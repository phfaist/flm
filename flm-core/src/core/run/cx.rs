//! The contexts through which a stage and a rewrite stage read what they need
//! during one call, and value catch-up, which brings a synthesized value up to
//! date.
//!
//! The runner creates a [`StageCx`] for each call of a stage that runs after the
//! trees are frozen, and a [`RewriteCx`] for each call of a rewrite stage
//! ([`RewriteStage`](crate::core::stage::RewriteStage)), which runs before. Each
//! context exists only for that one call.
//!
//! # What a stage reads through its context
//!
//! A stage context ([`StageCx`]) gives a stage, during one call:
//!
//! - the document, the registry and the plan;
//! - the **frozen trees**. A frozen tree is a document tree after the rewrite
//!   stages have run: it is never changed again, and everything a later stage
//!   computes about it is stored in tables beside it, keyed by node address
//!   ([frozen trees and computed
//!   data](crate::guide::concepts_overview#frozen-trees-and-computed-data)).
//!   [`StageCx::tree`] returns one by identifier; a tree whose rewrite failed
//!   has no frozen tree. A stage receives the frozen trees through its
//!   context, not by reading the rewrite stages' tables;
//! - the **run's document order**, the document's walk applied to the frozen
//!   trees ([`crate::core::run::order`]): [`StageCx::walk`] visits the nodes in
//!   that order, [`StageCx::position`] returns a node's position in it, and
//!   [`StageCx::placement_of`] the tree that a node places;
//! - the tables the stage declared as reads, through a view that refuses a read
//!   the stage did not declare ([`StageCx::get`]);
//! - the values of the option types the stage declared, resolved once for the
//!   run: the value set on the plan request, else the value set on the
//!   document, else the type's default ([`StageCx::option`]);
//! - the services the registry holds ([`StageCx::service`]);
//! - node addresses, by which computed data is keyed ([`StageCx::addr`]);
//! - value catch-up ([`StageCx::resolve_value`]), described below.
//!
//! A stage writes nothing through its context: it writes what it produces into
//! the [`StageOut`] that it receives together with the context.
//!
//! A rewrite stage reads much less through its [`RewriteCx`]: the identifier and
//! the layout of the tree being rewritten, the registry, and the option types
//! it declared. A `RewriteCx` gives no access to any table, so the rule that a
//! rewrite stage reads no computed data is enforced by the types.
//!
//! # Value catch-up
//!
//! A synthesized value is a small tree that a stage builds while a run is under
//! way, held outside the document's trees and numbered per stage invocation; a
//! consumer brings it up to date through value catch-up before reading it as
//! document content ([synthesized
//! values](crate::guide::concepts_overview#synthesized-values)). An example is
//! the content that a stage assembles from a user definition's template and
//! stores in a table. No rewrite stage has run over a synthesized value, and no
//! stage has computed anything about it.
//!
//! [Value catch-up](crate::guide::concepts_overview#value-catch-up) does that
//! work when something comes to render the synthesized value:
//! [`StageCx::resolve_value`] runs the plan's rewrite stages over it, then the
//! per-tree stages (stages of [`Scope::PerTree`], run once for each tree) that
//! the consumer depends on, over the synthesized value
//! alone. What these stages compute goes into a catch-up store that belongs to
//! that one catch-up and is returned in [`CaughtUp`]; the run's store is never
//! written. Stages that run once for the whole document are not run again, and
//! a table piece that states a fact about the document as a whole is refused
//! ([`PieceClass`](crate::core::data::table::PieceClass)).
//!
//! A table that the catch-up store holds is read whole from it, never entry by
//! entry. A feature author must know the consequence, which
//! [`ComputedView::with_overlay`] states. Reading one table from both stores at
//! once would need a merged copy of a type-erased table, which the
//! [`Table`](crate::core::data::table::Table) trait does not offer, and it would
//! weaken the rule that keeps a synthesized value from stating a fact about the
//! document.
//!
//! The diagnostics that the stages report during value catch-up are returned in
//! [`CaughtUp`], every one of them: a stage reports over a synthesized value
//! exactly as it does over a document tree. Content that a synthesized value
//! copies from the document is therefore reported again wherever the value is
//! brought up to date (a heading's title holding an unresolved reference
//! reports it where the heading is written, and again at every reference that
//! shows that title), and each such report still points at the place where the
//! content was written. When a lowering rule splices the synthesized value into
//! its output ([`Lowerer::splice`](crate::core::lowering::Lowerer::splice)), the
//! lowering adds to each report one traceback frame for each synthesized value
//! being spliced, each pointing at the node where that copy is shown.
//!
//! # The identifiers of synthesized values
//!
//! Synthesized values are numbered per stage call: the values that one stage
//! call brings up to date, including those of the catch-ups nested inside it,
//! are numbered from zero. Two calls therefore assign the same identifiers to
//! different trees. Two rules follow, stated in full at
//! [`StageCx::resolve_value`]: never store such an identifier, or a node address
//! inside such a tree, in a table that the runner folds into the run's store;
//! and never write such a tree by its identifier when a table serializes itself
//! ([`Table::project`](crate::core::data::table::Table::project)).
//!
//! During value catch-up, [`StageCx::walk`] still walks the document's trees,
//! not the synthesized value.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::fmt;
use std::rc::Rc;
use std::sync::Arc;

use techy::core::node::{NodeRef, NodeTree};
use techy::error::{Diagnostic, DiagnosticInfo};

use crate::model::addr::{NodeAddr, TreeId};
use crate::model::doc::{Doc, Fragment, FragmentLayout};
use crate::model::names::{CapName, ElementName, StageName};
use crate::model::options::{OptionError, Options, ResolvedOptions};
use serde_json::Value;
use crate::core::plan::Plan;
use crate::core::plan::build::request_subject;
use crate::core::registry::StageKind;
use crate::core::registry::frozen::{FlmRegistry, StageIdx};
use crate::core::services::Services;
use crate::core::stage::{RewriteError, Scope};
use crate::core::data::store::{
    AccessError, Computed, ComputedView, FoldTarget, StageOut, StoreError, fold_outs,
};
use crate::core::data::table::{SingletonTable, TableKey};
use crate::core::run::order::{DocPosition, RunOrder};
use crate::lang::structural::StructuralElements;
use crate::lang::{Flm, FlmDiagnostic, FlmDiagnostics, FlmSpan};

/// Diagnostic condition `flm.run.missing-entry`, reported at Error severity: a
/// table holds no entry for a node inside a synthesized value, where a node of
/// the document would have one.
///
/// A synthesized value states no fact about the document, so no stage numbered
/// a heading inside one or counted a footnote there. The code that finds the
/// missing entry reports this condition instead of guessing the missing data;
/// [`StageCx::missing_entry`] builds the diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.run.missing-entry",
    message = "this `{element}` element has no entry in table `{table}` (written by \
               `{producer}`), because it is inside content generated during processing, for \
               which no such data is recorded"
)]
pub struct MissingEntry {
    /// The name of the table that was read.
    pub table: String,
    /// The name of the stage that the plan scheduled to write the table, or
    /// `nobody` when no stage of the plan writes it.
    pub producer: String,
    /// The name of the element of the node that the data was wanted for.
    pub element: String,
}

/// Diagnostic condition `flm.synth.nesting-too-deep`: during value catch-up, a
/// rewrite stage found a synthesized value nested more deeply than the
/// recursion guard allows ([the recursion guard and worker
/// threads](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads)).
///
/// Value catch-up then returns [`CatchUpError::NestingTooDeep`], which carries
/// this diagnostic, and the synthesized value is left unchanged.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.synth.nesting-too-deep",
    message = "in content generated during processing, {detail}"
)]
pub struct SynthNestingTooDeep {
    /// The message of the diagnostic that the rewrite stage reported, which
    /// states how deep the content nests.
    pub detail: String,
}

/// Diagnostic condition `flm.run.ignored-preference`, reported at Warning
/// severity: the plan request (named by its workflow when it was made from one)
/// preferred a stage that the plan could not schedule, because scheduling
/// another stage that provides the same capability had already ruled it out.
///
/// The run reports it once, before any other diagnostic of the run: the plan
/// was built, so the run goes ahead, but the caller did not get what it asked
/// for. It is the run-time counterpart of
/// [`PlanWarning::IgnoredPreference`](crate::core::plan::PlanWarning::IgnoredPreference).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.run.ignored-preference")]
pub struct IgnoredPreference {
    /// The name of the stage that the plan request preferred.
    pub stage: String,
    /// The name of the stage scheduled instead, which ruled the preferred
    /// stage out.
    pub excluded_by: String,
    /// A capability that both stages provide.
    pub over: String,
    /// The name of the workflow the plan request was made from, or `None` for
    /// a request built by hand
    /// ([`PlanRequest::workflow`](crate::core::plan::PlanRequest::workflow)).
    pub workflow: Option<String>,
}

impl fmt::Display for IgnoredPreference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} prefers the stage `{}`, but that stage cannot be scheduled: the scheduled \
             stage `{}` also provides `{}`, and only one scheduled stage may provide each \
             capability",
            request_subject(self.workflow.as_deref()),
            self.stage,
            self.excluded_by,
            self.over
        )
    }
}

/// Diagnostic condition `flm.run.preferred-rule-set-aside`, reported at
/// Warning severity: the plan request (named by its workflow when it was made
/// from one) preferred a lowering rule that is set aside, so no plan can choose
/// it.
///
/// A peer declaration is a feature's forward declaration of something that
/// another feature owns, such as a backend, a presentation element or a table
/// ([`PeerName`](crate::core::registry::PeerName)). A lowering rule is set
/// aside when a peer declaration it depends on resolves to nothing: the rule is
/// excluded from rule selection. The run reports this condition once, before
/// any other diagnostic of the run, and goes ahead with the rule that the rule
/// selection order ([`SelectedRules`](crate::core::plan::SelectedRules)) chose
/// instead. It is the run-time counterpart of
/// [`PlanWarning::PreferredRuleSetAside`](crate::core::plan::PlanWarning::PreferredRuleSetAside).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.run.preferred-rule-set-aside")]
pub struct PreferredRuleSetAside {
    /// The name of the lowering rule that the plan request preferred.
    pub rule: String,
    /// The name of the backend that the preference was stated for.
    pub backend: String,
    /// The peer declarations that did not resolve, each written as its kind
    /// followed by its name, in the order in which the rule declares them.
    pub unresolved: Vec<String>,
    /// The name of the workflow the plan request was made from, or `None` for
    /// a request built by hand
    /// ([`PlanRequest::workflow`](crate::core::plan::PlanRequest::workflow)).
    pub workflow: Option<String>,
}

impl fmt::Display for PreferredRuleSetAside {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} prefers the lowering rule `{}` on backend `{}`, but that rule cannot be used: \
             it depends on {}, which no registered feature declares",
            request_subject(self.workflow.as_deref()),
            self.rule,
            self.backend,
            self.unresolved.join(", ")
        )
    }
}

/// Diagnostic condition `flm.run.preferred-rule-not-candidate`, reported at
/// Warning severity: the plan request (named by its workflow when it was made
/// from one) preferred, for a backend, a lowering rule that is not a candidate
/// for any element on that backend.
///
/// A rule marked as the default for named backends
/// ([`DefaultMark::OnBackends`](crate::core::lowering::DefaultMark::OnBackends))
/// belongs to those backends and is never chosen on any other. The run reports
/// this condition once, before any other diagnostic of the run, and goes ahead
/// with the rule that the rule selection order
/// ([`SelectedRules`](crate::core::plan::SelectedRules)) chose instead. It is
/// the run-time counterpart of
/// [`PlanWarning::PreferredRuleNotCandidate`](crate::core::plan::PlanWarning::PreferredRuleNotCandidate).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.run.preferred-rule-not-candidate")]
pub struct PreferredRuleNotCandidate {
    /// The name of the lowering rule that the plan request preferred.
    pub rule: String,
    /// The name of the backend that the preference was stated for.
    pub backend: String,
    /// The name of the workflow the plan request was made from, or `None` for
    /// a request built by hand
    /// ([`PlanRequest::workflow`](crate::core::plan::PlanRequest::workflow)).
    pub workflow: Option<String>,
}

impl fmt::Display for PreferredRuleNotCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} prefers the lowering rule `{}` on backend `{}`, but that rule is marked as the \
             default on other backends only, which restricts it to those backends",
            request_subject(self.workflow.as_deref()),
            self.rule,
            self.backend
        )
    }
}

/// The reason why value catch-up could not bring a synthesized value up to
/// date.
///
/// Returned by [`StageCx::resolve_value`] and
/// [`StageCx::resolve_value_with`]. The caller of value catch-up decides what to
/// do. [`Lowerer::splice`](crate::core::lowering::Lowerer::splice), for example,
/// turns the store's refusal of a fact about the document into
/// [`LowerError::DocumentFactInValue`](crate::core::lowering::LowerError::DocumentFactInValue)
/// and any other failure into
/// [`LowerError::CatchUp`](crate::core::lowering::LowerError::CatchUp).
#[derive(Debug)]
pub enum CatchUpError {
    /// A rewrite stage found the synthesized value nested more deeply than the
    /// recursion guard allows. The diagnostic's condition is
    /// [`SynthNestingTooDeep`].
    NestingTooDeep(FlmDiagnostic),
    /// A rewrite stage failed over the synthesized value.
    Rewrite {
        /// The rewrite stage that failed.
        stage: StageName,
        /// The one diagnostic it reported.
        diagnostic: FlmDiagnostic,
    },
    /// A stage run during value catch-up produced a table piece that states a
    /// fact about the document as a whole
    /// ([`StoreError::DocumentFactInValue`]), which a synthesized value may not
    /// do, or an invariant of the catch-up store failed
    /// ([`StoreError::SecondWriter`]).
    Store(StoreError),
}

impl CatchUpError {
    /// Returns the one diagnostic that describes the failure, when the failure
    /// has one.
    ///
    /// [`CatchUpError::Store`] has none; its `Display` text describes the
    /// failure.
    pub fn diagnostic(&self) -> Option<&FlmDiagnostic> {
        match self {
            CatchUpError::NestingTooDeep(d) => Some(d),
            CatchUpError::Rewrite { diagnostic, .. } => Some(diagnostic),
            CatchUpError::Store(_) => None,
        }
    }
}

impl fmt::Display for CatchUpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CatchUpError::NestingTooDeep(d) => write!(f, "{}", d.message()),
            CatchUpError::Rewrite { stage, diagnostic } => {
                write!(
                    f,
                    "rewrite stage `{stage}` failed on content generated during processing: {}",
                    diagnostic.message()
                )
            }
            CatchUpError::Store(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl std::error::Error for CatchUpError {}

impl From<StoreError> for CatchUpError {
    fn from(error: StoreError) -> CatchUpError {
        CatchUpError::Store(error)
    }
}

/// What one run shares with every stage context it creates.
pub(crate) struct RunView<'r> {
    /// The document being processed.
    pub doc: &'r Doc,
    /// The registry that the plan was built from.
    pub registry: &'r FlmRegistry,
    /// The plan being run.
    pub plan: &'r Plan,
    /// The frozen tree of each of the document's trees that has one.
    pub frozen: &'r BTreeMap<TreeId, Arc<NodeTree<Flm>>>,
    /// The run's document order: the trees, their nodes, the placements.
    pub order: &'r RunOrder,
    /// From a frozen tree's address in memory to its identifier, so that a node
    /// handed to a stage can be placed.
    pub tree_index: &'r BTreeMap<usize, TreeId>,
    /// The store as it was when the current batch began; every stage of a
    /// batch reads the same version.
    pub computed: &'r Computed,
    /// The resolved option values of every scheduled stage.
    pub options: &'r BTreeMap<StageIdx, Arc<ResolvedOptions>>,
}

/// The synthesized values that one stage call brought up to date, each with
/// its address in memory and the identifier it was given.
type ValueTrees = Vec<(usize, TreeId, Arc<NodeTree<Flm>>)>;

/// The numbering of the synthesized values of one stage call, and the trees
/// themselves, so that the address of a node inside one can be found.
struct ValueSpace {
    next: Cell<u32>,
    trees: RefCell<ValueTrees>,
}

/// A synthesized value brought up to date by value catch-up: the tree after the
/// rewrite stages, what the stages run over it computed, and the diagnostics
/// they reported.
///
/// Returned by [`StageCx::resolve_value`]. Read the computed data through
/// [`CaughtUp::view`], and report [`CaughtUp::diags`] yourself, for example by
/// appending them to [`StageOut::diags`](crate::core::data::store::StageOut::diags):
/// value catch-up writes nothing into the run.
pub struct CaughtUp {
    /// The identifier given to the synthesized value.
    ///
    /// It means something only inside the stage call that requested the value
    /// catch-up, because synthesized values are numbered per stage call. Never
    /// store it, or a node address inside this tree, in a table that the runner
    /// folds into the run's store, and never write this tree by its identifier
    /// when a table serializes itself. The rules are stated in full at
    /// [`StageCx::resolve_value`].
    pub id: TreeId,
    /// The synthesized value as the rewrite stages left it.
    pub tree: Arc<NodeTree<Flm>>,
    /// The catch-up store: what the stages run over the tree computed, in a
    /// store of its own.
    pub scratch: Computed,
    /// The diagnostics reported while the tree was brought up to date.
    pub diags: FlmDiagnostics,
}

impl CaughtUp {
    /// Returns a view of the tables that the stage of `cx` declared as reads,
    /// reading this catch-up store before the run's store for every read,
    /// whichever node the data is wanted for.
    ///
    /// A table present in the catch-up store is read whole from it; see
    /// [`ComputedView::with_overlay`]. The lowering stage of a backend
    /// ([`crate::stdcore::backends::lowering`]) does not read through this
    /// view: it keeps each synthesized value's catch-up store by
    /// the value's identifier and reads it only for an address inside that
    /// value, so that content of another tree, lowered while the value is
    /// lowered, reads its own tree's tables.
    pub fn view<'a>(&'a self, cx: &StageCx<'a>) -> ComputedView<'a> {
        cx.view.with_overlay(&self.scratch)
    }
}

impl fmt::Debug for CaughtUp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CaughtUp").field("id", &self.id).field("scratch", &self.scratch).finish()
    }
}

/// The stage context: everything a stage may read during one call.
///
/// A stage context gives a stage the frozen trees, the run's document order,
/// the tables it declared as reads, the values of the option types it
/// declared, the registry's services, node addresses and value catch-up; the
/// [module documentation](self) describes each.
///
/// The runner creates one stage context for each call: once for each tree of
/// the run for a [`Scope::PerTree`] stage, once for a [`Scope::Document`]
/// stage. It passes the context to [`Stage::run`](crate::core::stage::Stage::run)
/// together with a [`StageOut`], into which the stage writes what it produces.
/// During value catch-up, a per-tree stage receives a context for the
/// synthesized value.
pub struct StageCx<'a> {
    run: &'a RunView<'a>,
    stage: StageIdx,
    /// For a per-tree call, which tree it is for: a document tree, or a
    /// synthesized one being brought up to date. `None` for a document-scope
    /// call.
    tree: Option<TreeId>,
    /// The tree itself, when this call is over a synthesized one. A document
    /// tree is reached through `run.frozen` instead.
    value_tree: Option<Arc<NodeTree<Flm>>>,
    view: ComputedView<'a>,
    values: Rc<ValueSpace>,
}

impl<'a> StageCx<'a> {
    pub(crate) fn new(
        run: &'a RunView<'a>,
        stage: StageIdx,
        tree: Option<TreeId>,
        view: ComputedView<'a>,
    ) -> StageCx<'a> {
        StageCx {
            run,
            stage,
            tree,
            value_tree: None,
            view,
            values: Rc::new(ValueSpace { next: Cell::new(0), trees: RefCell::new(Vec::new()) }),
        }
    }

    /// The document being processed.
    pub fn doc(&self) -> &'a Doc {
        self.run.doc
    }

    /// The registry that the plan was built from.
    pub fn registry(&self) -> &'a FlmRegistry {
        self.run.registry
    }

    /// The plan being run.
    pub fn plan(&self) -> &'a Plan {
        self.run.plan
    }

    /// The name of the stage that this call is for.
    pub fn stage_name(&self) -> &'a StageName {
        &self.run.registry.stage(self.stage).name
    }

    /// The stage's scope: once per tree, or once for the whole document.
    pub fn scope(&self) -> Scope {
        self.run.registry.stage(self.stage).decl.scope
    }

    /// The identifier of the tree that this call is for (a tree of the run, or
    /// a synthesized value during value catch-up), or `None` when the stage runs once for
    /// the whole document.
    pub fn tree_id(&self) -> Option<TreeId> {
        self.tree
    }

    /// Returns the tree with this identifier: the frozen tree of a tree of the
    /// run, or a synthesized value that this stage call has brought up to
    /// date.
    ///
    /// `None` when the identifier is unknown, when the document tree's rewrite
    /// failed so that it has no frozen tree, or when the tree is not a tree of
    /// the run ([`Run::tree_order`](crate::core::run::Run::tree_order)).
    pub fn tree(&self, id: TreeId) -> Option<Arc<NodeTree<Flm>>> {
        if id.is_value() {
            if self.tree == Some(id) {
                return self.value_tree.clone();
            }
            return self
                .values
                .trees
                .borrow()
                .iter()
                .find(|(_, tree_id, _)| *tree_id == id)
                .map(|(_, _, tree)| Arc::clone(tree));
        }
        self.run.frozen.get(&id).cloned()
    }

    /// Returns the frozen tree of a tree of the run, borrowed rather than
    /// shared.
    ///
    /// Unlike [`StageCx::tree`], this never returns a synthesized value.
    pub fn frozen_tree(&self, id: TreeId) -> Option<&'a Arc<NodeTree<Flm>>> {
        self.run.frozen.get(&id)
    }

    /// Returns the tree that this call is for, if the call is for one: a
    /// frozen tree, or a synthesized value during value catch-up.
    pub fn my_tree(&self) -> Option<Arc<NodeTree<Flm>>> {
        self.tree.and_then(|id| self.tree(id))
    }

    /// Returns the fragment that a document tree was parsed from
    /// ([`Fragment`]: one piece of source as the parser delivered it).
    pub fn fragment(&self, id: TreeId) -> Option<&'a Arc<Fragment>> {
        self.run.doc.content().fragment(id)
    }

    /// Returns how a document tree's content is laid out: as inline content, as
    /// blocks, or as inline content followed by blocks ([`FragmentLayout`]).
    pub fn layout(&self, id: TreeId) -> Option<FragmentLayout> {
        self.fragment(id).map(|fragment| fragment.layout())
    }

    /// Returns the run's trees, each with its identifier and its frozen tree,
    /// in the run's order of the trees: the trees that the walk
    /// ([`StageCx::walk`]) reaches, in the order in which it enters them. A
    /// tree that the walk does not reach is not a tree of the run
    /// ([`Run::tree_order`]).
    ///
    /// A placed tree is entered at the node that places it, in the middle of
    /// the tree that places it, so reading the trees one after the other is not
    /// reading the document in order. A document-scope stage that needs
    /// document order across trees uses [`StageCx::walk`], or sorts what it
    /// gathers from several trees by [`StageCx::position`].
    ///
    /// [`Run::tree_order`]: crate::core::run::Run::tree_order
    pub fn trees(&self) -> impl Iterator<Item = (TreeId, &'a Arc<NodeTree<Flm>>)> + 'a {
        let run = self.run;
        run.order.trees().iter().filter_map(move |id| run.frozen.get(id).map(|tree| (*id, tree)))
    }

    /// Returns every node of every tree of the run, with its address, in
    /// document order.
    ///
    /// This is the document's walk ([`DocContent::walk`]) applied to the frozen
    /// trees: the listed trees in order, each tree once, within a tree the root
    /// and then its descendants in the order in which they are written, and a
    /// placed tree entered at the node that places it, its nodes following that
    /// node immediately and coming before the node's own children. A tree that
    /// the walk does not reach (its placing node removed by a rewrite stage, or
    /// the tree that includes it failed its rewrite) is not a tree of the run
    /// ([`Run::tree_order`]). A document-scope stage reads the whole document
    /// this way. During value catch-up, the walk still covers the document's
    /// trees, not the synthesized value.
    ///
    /// [`DocContent::walk`]: crate::model::doc::DocContent::walk
    /// [`Run::tree_order`]: crate::core::run::Run::tree_order
    pub fn walk(&self) -> FrozenWalk<'a> {
        FrozenWalk::new(self.run)
    }

    /// Returns the position of the node at `addr` in the run's document order,
    /// the order of [`StageCx::walk`]: of two nodes, the one with the smaller
    /// position comes first in the document.
    ///
    /// A document-scope stage that gathers nodes, or records keyed by nodes,
    /// from several trees sorts them by position. `None` for a node of none of
    /// the run's trees, for example a node of a synthesized value.
    pub fn position(&self, addr: NodeAddr) -> Option<DocPosition> {
        self.run.order.position(addr)
    }

    /// Returns the tree of the document that the node at `addr`, a node of a
    /// frozen tree, places, or `None` when it places none.
    ///
    /// The document records its placements by the placing node as parsed
    /// ([`DocContent::placement_of`]); the run finds each placing node again
    /// in the frozen trees, where a stage reaches it. When a rewrite stage
    /// copied a placing node, only its first copy in document order places the
    /// tree ([`PlacedTwice`](crate::core::run::PlacedTwice)).
    ///
    /// [`DocContent::placement_of`]: crate::model::doc::DocContent::placement_of
    pub fn placement_of(&self, addr: NodeAddr) -> Option<TreeId> {
        self.run.order.placement_of(addr)
    }

    /// Returns the view of the store that this call reads through, which
    /// refuses a read the stage did not declare. During value catch-up, the
    /// view reads the catch-up store before the run's store.
    pub fn view(&self) -> &ComputedView<'a> {
        &self.view
    }

    /// Reads a table that the stage declared as a read
    /// ([`Needs::reads`](crate::core::stage::Needs::reads)), through a typed
    /// table key.
    ///
    /// `Ok(None)` means that no stage of the plan provides the table, which
    /// can only happen for a table declared as an optional read
    /// ([`Needs::reads_opt`](crate::core::stage::Needs::reads_opt)).
    ///
    /// # Errors
    ///
    /// Returns [`AccessError`] when the stage did not declare this read, or
    /// declared it under another value type.
    pub fn get<K: TableKey>(&self, key: K) -> Result<Option<&'a K::Table>, AccessError> {
        self.view.get(key)
    }

    /// Reads the one table of a value type `T` that exactly one table uses
    /// ([`SingletonTable`]).
    ///
    /// The same as [`StageCx::get`] with the table name `T::NAME`.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError`] when the stage did not declare this read, or
    /// declared it under another value type.
    pub fn get_singleton<T: SingletonTable>(&self) -> Result<Option<&'a T>, AccessError> {
        self.view.get_singleton::<T>()
    }

    /// Returns the value of an option type that the stage declared as a read:
    /// the value set on the plan request, else the value set on the document,
    /// else the type's default ([option types and
    /// values](crate::model::options)). The value is resolved once for the
    /// whole run.
    ///
    /// # Errors
    ///
    /// Returns [`OptionError`] when the stage did not declare that option
    /// type.
    pub fn option<T: Options>(&self) -> Result<&'a T, OptionError> {
        self.run.options[&self.stage].get::<T>()
    }

    /// Returns the value of the option type declared from data under the name
    /// `name`, which the stage declared as a read
    /// ([`Needs::option_named`](crate::core::stage::Needs::option_named)): the
    /// data as written on the plan request, else on the document, else the
    /// empty map. Otherwise as [`StageCx::option`].
    ///
    /// # Errors
    ///
    /// Returns [`OptionError::UndeclaredRead`], with the name, when the stage
    /// did not declare that read.
    pub fn option_named(&self, name: &str) -> Result<&'a Value, OptionError> {
        self.run.options[&self.stage].get_named(name)
    }

    /// Returns every option type that the stage declared, each with its
    /// resolved value.
    ///
    /// [`StageCx::option`] reads one value. A stage that reads option values
    /// on behalf of other code (a lowering stage, for its lowering rules)
    /// passes this whole set on.
    pub fn options(&self) -> &'a ResolvedOptions {
        &self.run.options[&self.stage]
    }

    /// Returns every service that the registry holds ([`Services`]; see
    /// [services](crate::core::services) for what a service is).
    pub fn services_all(&self) -> &'a Services {
        self.run.registry.services()
    }

    /// Returns one service of type `S`: of the service slots whose values have
    /// type `S`, taken in name order, the first that holds a contribution gives
    /// the one that takes precedence in it (between features, the contribution
    /// of the feature placed last in the feature order); `None` when no such
    /// slot holds a contribution. See [`Services::service`].
    pub fn service<S: ?Sized + Any + Send + Sync>(&self) -> Option<&'a S> {
        self.run.registry.services().service::<S>()
    }

    /// Returns every service of type `S`: the contributions of every service
    /// slot whose values have type `S`, the slots in name order. See
    /// [`Services::services`].
    pub fn services<S: ?Sized + Any + Send + Sync>(&self) -> impl Iterator<Item = &'a S> + 'a {
        self.run.registry.services().services::<S>()
    }

    /// Returns the address of `node`, by which computed data about the node is
    /// keyed.
    ///
    /// `None` when the node belongs to neither a frozen tree of the document
    /// nor a synthesized tree this stage call has brought up to date.
    pub fn addr(&self, node: &NodeRef<'_, Flm>) -> Option<NodeAddr> {
        let key = node.tree() as *const NodeTree<Flm> as usize;
        if let Some(id) = self.run.tree_index.get(&key) {
            return Some(NodeAddr::new(*id, node.id()));
        }
        if let (Some(id), Some(tree)) = (self.tree, &self.value_tree) {
            if Arc::as_ptr(tree) as usize == key {
                return Some(NodeAddr::new(id, node.id()));
            }
        }
        self.values
            .trees
            .borrow()
            .iter()
            .find(|(ptr, _, _)| *ptr == key)
            .map(|(_, id, _)| NodeAddr::new(*id, node.id()))
    }

    /// Builds the Error diagnostic [`MissingEntry`], for a node inside a
    /// synthesized value for which the table `table` holds no entry.
    ///
    /// The diagnostic identifies the table, the stage that the plan scheduled
    /// to write it (or `nobody`), and `element`, the element of the node; it is
    /// reported at `span`. This method only builds the diagnostic: the caller
    /// reports it, for example by appending it to
    /// [`StageOut::diags`](crate::core::data::store::StageOut::diags).
    pub fn missing_entry(
        &self,
        table: &CapName,
        element: &ElementName,
        span: FlmSpan,
    ) -> FlmDiagnostic {
        let producer = self
            .view
            .producer_of(table)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "nobody".to_string());
        Diagnostic::error(
            MissingEntry { table: table.to_string(), producer, element: element.to_string() },
            span,
        )
    }

    /// Brings a synthesized value up to date through value catch-up, so that it
    /// can be rendered like a tree of the document.
    ///
    /// A synthesized value is a small tree that a stage builds while a run is
    /// under way ([synthesized
    /// values](crate::guide::concepts_overview#synthesized-values)). The
    /// plan's rewrite stages run over `value`, then the per-tree stages that
    /// this stage depends on
    /// ([`Plan::per_tree_closure`](crate::core::plan::Plan::per_tree_closure))
    /// run over it alone; what they compute and report is returned in
    /// [`CaughtUp`]. The run's store is not written. See [value
    /// catch-up](crate::guide::concepts_overview#value-catch-up).
    ///
    /// The value's content is laid out with the default [`FragmentLayout`]:
    /// inline content, followed by blocks if a block appears.
    /// [`StageCx::resolve_value_with`] sets the layout explicitly.
    ///
    /// # Tables read during value catch-up
    ///
    /// A table that the stages run during value catch-up write is read from
    /// the catch-up store, whole: a stage run over the value reads the value's
    /// contribution to that table and nothing of the document's. A stage that
    /// needs data about the whole document must therefore read a table that no
    /// per-tree stage run during the catch-up writes; see
    /// [`ComputedView::with_overlay`].
    ///
    /// # The identifiers of the synthesized value
    ///
    /// Synthesized values are numbered per stage call, so this value's
    /// identifier, and every node address inside it, means something only
    /// inside this call; another call assigns the same identifiers to a
    /// different tree. Two rules follow:
    ///
    /// - **Never put such an address into a table piece that the runner folds
    ///   into the run's store.** Entries belonging to two different trees would
    ///   collide there without anything noticing.
    /// - **Never write such a tree by its identifier when a table serializes
    ///   itself** ([`Table::project`](crate::core::data::table::Table::project)).
    ///   A reader of the serialized table cannot resolve the identifier; a
    ///   table writes a stored synthesized value in full instead
    ///   ([`ProjectCx::tree`](crate::core::data::table::ProjectCx::tree)).
    ///
    /// # Errors
    ///
    /// Returns [`CatchUpError`] when a rewrite stage refused or failed over the
    /// value, or when a stage run over the value produced a table piece that
    /// states a fact about the document as a whole.
    ///
    /// # Panics
    ///
    /// A rewrite stage or another stage that panics while it runs over the
    /// value propagates the panic out of this call, as out of
    /// [`Run::step`](crate::core::run::Run::step). A panic means a defect in
    /// that stage; a problem with the document is reported as a diagnostic.
    pub fn resolve_value(&self, value: &Arc<NodeTree<Flm>>) -> Result<CaughtUp, CatchUpError> {
        self.resolve_value_with(value, FragmentLayout::default())
    }

    /// Brings a synthesized value up to date through value catch-up, laying out
    /// its content with `layout`.
    ///
    /// The same as [`StageCx::resolve_value`] in every other respect, including
    /// the rules about what the stages read and about the value's identifiers,
    /// except that `layout` says whether the value's content is inline content,
    /// blocks, or inline content followed by blocks.
    ///
    /// # Errors
    ///
    /// Returns [`CatchUpError`], as [`StageCx::resolve_value`] does.
    ///
    /// # Panics
    ///
    /// As [`StageCx::resolve_value`]: a rewrite stage or another stage that
    /// panics while it runs over the value propagates the panic out of this
    /// call.
    pub fn resolve_value_with(
        &self,
        value: &Arc<NodeTree<Flm>>,
        layout: FragmentLayout,
    ) -> Result<CaughtUp, CatchUpError> {
        let run = self.run;
        let index = self.values.next.get();
        self.values.next.set(index + 1);
        let id = TreeId::value(index);
        let mut diags = FlmDiagnostics::new();
        let mut current = Arc::clone(value);
        for rewrite in run.plan.rewrites() {
            let entry = run.registry.stage(*rewrite);
            let StageKind::Rewrite(stage) = &entry.kind else { continue };
            let cx = RewriteCx::new(run.registry, id, layout, &run.options[rewrite]);
            match stage.rewrite(id, &current, &cx, &mut diags) {
                Ok(next) => current = next,
                Err(RewriteError::NestingTooDeep(diagnostic)) => {
                    let detail = diagnostic.message();
                    return Err(CatchUpError::NestingTooDeep(Diagnostic::error(
                        SynthNestingTooDeep { detail },
                        diagnostic.span().clone(),
                    )));
                }
                Err(error) => {
                    return Err(CatchUpError::Rewrite {
                        stage: entry.name.clone(),
                        diagnostic: error.diagnostic().clone(),
                    });
                }
            }
        }
        self.values.trees.borrow_mut().push((
            Arc::as_ptr(&current) as usize,
            id,
            Arc::clone(&current),
        ));
        let mut scratch = Computed::new();
        for idx in run.plan.per_tree_closure(self.stage) {
            let entry = run.registry.stage(idx);
            let StageKind::Pipeline(stage) = &entry.kind else { continue };
            let plan = run.plan.manifest(idx).expect("a scheduled stage has a manifest");
            let mut out = StageOut::new(&entry.name, &plan.writes);
            {
                let view = ComputedView::new(&entry.name, &plan.reads, run.computed)
                    .with_overlay(&scratch);
                let cx = StageCx {
                    run,
                    stage: idx,
                    tree: Some(id),
                    value_tree: Some(Arc::clone(&current)),
                    view,
                    values: Rc::clone(&self.values),
                };
                stage.run(&cx, &mut out);
            }
            fold_outs(
                &entry.name,
                &plan.writes,
                vec![out],
                FoldTarget::Scratch,
                &mut scratch,
                &mut diags,
            )?;
        }
        Ok(CaughtUp { id, tree: current, scratch, diags })
    }
}

impl fmt::Debug for StageCx<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StageCx")
            .field("stage", self.stage_name())
            .field("tree", &self.tree)
            .finish()
    }
}

/// An iterator over every node of every tree of the run, in document order,
/// with its address.
///
/// Created by [`StageCx::walk`], which describes the order: the document's walk
/// applied to the frozen trees, a placed tree entered at the node that places
/// it, over the run's trees only.
pub struct FrozenWalk<'a> {
    run: &'a RunView<'a>,
    position: usize,
}

impl<'a> FrozenWalk<'a> {
    fn new(run: &'a RunView<'a>) -> FrozenWalk<'a> {
        FrozenWalk { run, position: 0 }
    }
}

impl<'a> Iterator for FrozenWalk<'a> {
    type Item = (NodeAddr, NodeRef<'a, Flm>);

    fn next(&mut self) -> Option<(NodeAddr, NodeRef<'a, Flm>)> {
        let run = self.run;
        loop {
            let addr = *run.order.walk().get(self.position)?;
            self.position += 1;
            if let Some(node) = run.frozen.get(&addr.tree).and_then(|tree| tree.get(addr.node)) {
                return Some((addr, node));
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.run.order.walk().len().saturating_sub(self.position)))
    }
}

/// Everything a rewrite stage may read during one call: which tree it is
/// rewriting, how that tree's content is laid out, the registry, and the
/// option types the rewrite stage declared.
///
/// A rewrite stage runs before the document's trees are frozen, turns one tree
/// into another, reads no computed data, and publishes the trees it produced in
/// a table named after the capability it provides
/// ([`RewriteStage`](crate::core::stage::RewriteStage)). The runner creates one
/// `RewriteCx` for each call and passes it to
/// [`RewriteStage::rewrite`](crate::core::stage::RewriteStage::rewrite). A
/// `RewriteCx` gives no access to any table: rewrite stages run before anything
/// is computed, and the type enforces that they read no computed data.
pub struct RewriteCx<'a> {
    registry: &'a FlmRegistry,
    tree: TreeId,
    layout: FragmentLayout,
    options: &'a ResolvedOptions,
}

impl<'a> RewriteCx<'a> {
    pub(crate) fn new(
        registry: &'a FlmRegistry,
        tree: TreeId,
        layout: FragmentLayout,
        options: &'a ResolvedOptions,
    ) -> RewriteCx<'a> {
        RewriteCx { registry, tree, layout, options }
    }

    /// The registry that the plan was built from.
    pub fn registry(&self) -> &'a FlmRegistry {
        self.registry
    }

    /// Returns the structural elements of this registry: the elements of the
    /// node shapes that no feature declares, such as text, groups and
    /// paragraph breaks ([`StructuralElements`]).
    pub fn structural(&self) -> &'a Arc<StructuralElements> {
        self.registry.structural()
    }

    /// The identifier of the tree being rewritten: a document tree, or a
    /// synthesized value being brought up to date by value catch-up.
    pub fn tree_id(&self) -> TreeId {
        self.tree
    }

    /// How the tree's content is laid out: as inline content, as blocks, or as
    /// inline content followed by blocks ([`FragmentLayout`]).
    pub fn layout(&self) -> FragmentLayout {
        self.layout
    }

    /// Returns the value of an option type that the rewrite stage declared as
    /// a read: the value set on the plan request, else the value set on the
    /// document, else the type's default.
    ///
    /// # Errors
    ///
    /// Returns [`OptionError`] when the rewrite stage did not declare that
    /// option type.
    pub fn option<T: Options>(&self) -> Result<&'a T, OptionError> {
        self.options.get::<T>()
    }

    /// Returns the value of the option type declared from data under the name
    /// `name`, which the rewrite stage declared as a read: the data as written
    /// on the plan request, else on the document, else the empty map.
    ///
    /// # Errors
    ///
    /// Returns [`OptionError::UndeclaredRead`], with the name, when the rewrite
    /// stage did not declare that read.
    pub fn option_named(&self, name: &str) -> Result<&'a Value, OptionError> {
        self.options.get_named(name)
    }
}

impl fmt::Debug for RewriteCx<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RewriteCx").field("tree", &self.tree).field("layout", &self.layout).finish()
    }
}
