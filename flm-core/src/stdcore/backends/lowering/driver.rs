//! The lowering driver: [`lower_tree`] and [`lower_value`], which turn one
//! tree into one lowered tree, and the implementation of the [`Lowerer`]
//! every lowering rule works through.
//!
//! A **lowering rule** is a named registry entity written for one element: it
//! declares what it requires, reads and produces, and implements `Lower`,
//! which stages (builds into the lowered tree) the element's presentation for
//! one backend ([`LoweringRule`](crate::core::lowering::LoweringRule)). The
//! [`Lowerer`] is the only interface a lowering rule has to the run: through
//! it the rule lowers parts of its node, stages presentation nodes, splices a
//! synthesized value and lowers a node region of another node. This module
//! implements it.
//!
//! The lowering stage ([`super::stage`]) calls the driver once per frozen
//! tree; the pieces stage calls it once per piece. Everything here runs on
//! the caller's thread, which the runner has made a worker thread (a thread
//! with a large stack; see
//! [`on_worker_thread`](crate::lang::guard::on_worker_thread)).
//!
//! # Rounds
//!
//! **One round** is one pass over the tree, from the root down, each parent
//! before its children. At each node, in this order:
//!
//! 1. If the backend writes the node's element natively, the node is copied
//!    and its children are lowered in turn. If the element's content is
//!    opaque source text ([`OpaqueContent`]), the subtree is copied unchanged
//!    and nothing inside it is lowered, except a node carrying
//!    [`ContentInOpaque`], which holds document content rather than source
//!    text and is handled like any other node.
//! 2. Otherwise, if the plan selected a rule for the element, that rule runs.
//!    It returns [`Lowering::Default`] (make the copy just described) or
//!    [`Lowering::Emit`] with the nodes it staged; an empty list removes the
//!    node.
//! 3. Otherwise the node lowers to a childless `flm.pres.unsupported`
//!    stand-in and one Error diagnostic is reported ([`super::error`]).
//!
//! A rule may stage nodes of elements the backend does not write natively,
//! so one round is not always enough: if the round's output still holds such
//! an element (outside opaque content, which is examined only for the
//! content of a [`ContentInOpaque`] node), another round runs over that
//! output. The number of rounds allowed is [`rank`](super::rank), computed
//! when the plan is built; beyond it the tree fails with
//! `flm.lowering.rounds-exceeded` and gets no lowered form.
//!
//! # Provenance
//!
//! A copied node keeps its span. Everything a rule stages at a node is placed
//! on one synthesized source per pair of visited node and rule, created at
//! that node, so following the provenance of any lowered node leads back to
//! the document. The lowered tree stores no map from its nodes back to the
//! frozen nodes; provenance is the only link. For this reason
//! [`LoweredAddr`](super::LoweredAddr) gives a position rather than a frozen
//! node.
//!
//! # What the `Lowerer` enforces
//!
//! Everything a rule reads, lowers or stages goes through the [`Lowerer`].
//! The driver's implementation decides the following:
//!
//! - **Decorations are moved into attached slots, always in the same way.** A
//!   decoration is a node whose element carries the [`Decoration`] aspect.
//!   When a rule stages a presentation node ([`Lowerer::pres`],
//!   [`Lowerer::pres_slots`]), every decoration among the direct children of
//!   one of its content slots is moved into an attached slot named after the
//!   decoration's element, and a `flm.pres.deco-mark` node (the decoration
//!   mark) giving the index of that slot is left at its former position (see
//!   [decorations, roles and
//!   kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)).
//!   The driver stages the mark itself, so a rule need not declare it in
//!   `produces`. A backend that writes any decoration element natively must
//!   write `flm.pres.deco-mark` natively too; the registry build checks this
//!   through the native-set implication the `lowering` feature declares
//!   ([`native_implications`](super::native_implications)).
//! - **A rule must declare what it produces.** The `Lowerer` refuses to stage
//!   any presentation element the rule's `produces` declaration does not
//!   list. The only exception is `flm.text`: a rule may add text to its
//!   output without declaring it when the backend writes `flm.text` natively.
//! - **Inside a splice** ([`Lowerer::splice`]), [`Lowerer::addr`] returns the
//!   address of the node of the spliced synthesized value, since the tables
//!   that the rule may read there hold that value's results. The nodes staged
//!   inside the splice are still recorded as coming from the splicing rule's
//!   own node, so a later round never sees an address of the value.
//! - **Inside a region lowering** ([`Lowerer::lower_region`]), the opposite
//!   holds: the content of a node region of a node of another tree (a
//!   document tree, when a synthesized value is being lowered) is lowered in
//!   that tree's own terms. Its nodes return their own addresses and are
//!   recorded with them as their origins, so every later round reads the
//!   tables at the document's addresses too. A table read returns a
//!   synthesized value's own results only at that value's addresses, in
//!   every round.
//!
//! # Failure and recovery
//!
//! A rule's failure at one node (an undeclared table or option read, an
//! undeclared element in `produces`, a node shape it did not expect, a failed
//! [value catch-up](crate::guide::concepts_overview#value-catch-up)) is
//! reported as `flm.lowering.rule-failed` at that node, and the node lowers
//! to the childless `flm.pres.unsupported` stand-in. A rule that refuses the
//! node with a diagnostic of its own ([`LowerError::Refused`]) gets the same
//! stand-in, and its diagnostic is reported in place of
//! `flm.lowering.rule-failed`, at every node it refuses. The driver stages
//! that stand-in itself, without any rule declaring it, so every backend
//! must write `flm.pres.unsupported` natively; the registry build checks
//! this as well, through the second native-set implication the `lowering`
//! feature declares.
//!
//! Two failures end the lowering of the whole tree instead: a node that could
//! not be staged at all (`flm.lowering.failed`), and the recursion guard's
//! refusal to descend further (`flm.lowering.nesting-too-deep`).
//!
//! One outcome is not a failure of the node and is passed on, unreported,
//! from rule to rule up to the rule that spliced the synthesized value it
//! arose in: a rule needing the characters of a template's placeholder
//! ([`LowerError::PlaceholderCharacters`]), which the user-definitions
//! feature handles by expanding the definition's use sites. The splice it
//! passes through discards the value's diagnostics and requirements with it.
//! If it reaches the lowering of a whole tree, it ends that lowering as
//! `flm.lowering.failed`.
//!
//! A rule is expected to return every error the `Lowerer` gives it. The
//! driver does not define what happens when a rule continues after one; for
//! example, a refusal of the recursion guard that the rule ignored is still
//! recorded, and if the tree's lowering later fails, that failure is reported
//! as `flm.lowering.nesting-too-deep`.
//!
//! [`Lowering::Default`]: crate::core::lowering::Lowering::Default
//! [`Lowering::Emit`]: crate::core::lowering::Lowering::Emit
//! [`Lowerer`]: crate::core::lowering::Lowerer
//! [`Lowerer::pres`]: crate::core::lowering::Lowerer::pres
//! [`Lowerer::pres_slots`]: crate::core::lowering::Lowerer::pres_slots
//! [`Lowerer::splice`]: crate::core::lowering::Lowerer::splice
//! [`Lowerer::lower_region`]: crate::core::lowering::Lowerer::lower_region
//! [`Lowerer::addr`]: crate::core::lowering::Lowerer::addr

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::convert::Infallible;
use std::sync::Arc;

use techy::core::node::{
    BodySlotExt, BuildId, ChildRegion, ContentNodes, NodeKind, NodeRef, NodeSlice, NodeTree,
    ParsedArguments, ParsedSlot, ParsedSlots, SlotRole,
};
use techy::core::{Lang, ParsingState};
use techy::error::{Diagnostic, TraceFrame};
use techy::latexlike::BodyMarker;
use techy::source::{Source, SourceSpan};
use techy::transform::{Restage, RestageContext, RestageError, RestageVisitor, RestagedArgument};

use crate::model::addr::{NodeAddr, NodeId, TreeId};
use crate::model::element::{ElementSpec, element_of};
use crate::model::doc::FragmentLayout;
use crate::core::lowering::{
    LowerCx, LowerError, Lowerer, Lowering, NodeRegion, RegionShape, chars_at_lowering,
};
use crate::model::names::{BackendName, CapName, ElementName, Name, RuleName, StageName};
use crate::model::options::{OptionError, OptionKey};
use crate::core::plan::SelectedRules;
use crate::core::registry::frozen::{FlmRegistry, RuleIdx};
use crate::core::run::{CatchUpError, StageCx};
use crate::core::data::store::{AccessError, Computed, StoreError};
use crate::lang::copy::WrittenPiece;
use crate::lang::guard::{self, GuardPolicy};
use crate::lang::mint::FlmPresentationSpec;
use crate::lang::{
    BuiltLang, Flm, FlmCallableType, FlmDiagnostic, FlmDiagnostics, FlmOrigin, FlmSource, FlmSpan,
    PresPayload,
};
use crate::stdcore::backends::pres::{
    self, ContentInOpaque, DecoMarkPayload, Decoration, OpaqueContent, UnsupportedPayload,
    recompose_source,
};

use super::error::{
    ContentNotCharacters, LoweringFailed, NestingTooDeep, NoRule, RegionNotFound, RoundsExceeded, RuleFailed,
    UnsupportedElement,
};
use super::table::LoweredTree;

/// The name the `Lowerer` returns from `rule()` outside any rule, inside the
/// driver's own default handling, which no rule observes.
const DRIVER_RULE: &str = "flm.lowering.driver";

/// The title of the frame added to a diagnostic raised inside a spliced
/// synthesized value: the frame's span is the node at which the value is
/// shown, as the title says.
const SHOWN_HERE: &str = "a copy of this content, shown here";

/// The decorations moved out of a content slot: each one's element (the
/// attached slot's name) and its staged node.
type Hoisted = Vec<(ElementName, BuildId)>;

/// Everything one call of [`lower_tree`] or [`lower_value`] needs besides the
/// tree itself: what the plan decided for one backend, and the limits the run
/// imposes.
///
/// The lowering stage and the pieces stage build it from the plan and the
/// registry; a caller that runs lowering by hand builds it the same way, with
/// [`rank`](super::rank) and [`selection_requires`](super::selection_requires)
/// for the computed fields.
#[derive(Debug, Clone)]
pub struct LoweringInputs<'r> {
    /// The backend being lowered for.
    pub backend: &'r BackendName,
    /// The registry the selection's rule indices refer to.
    pub registry: &'r FlmRegistry,
    /// The rules the plan selected for this backend, by element.
    pub selected: &'r SelectedRules,
    /// The elements this backend writes natively under this plan: its native
    /// set, minus any element for which the plan request prefers a rule over
    /// the native writing.
    pub native: &'r BTreeSet<ElementName>,
    /// The accepted unrenderable elements of this backend: the elements the
    /// plan request accepts that the backend cannot render
    /// ([`PlanRequest::unsupported`](crate::core::plan::PlanRequest#structfield.unsupported)).
    /// Each node of one lowers to a childless `flm.pres.unsupported` stand-in
    /// with one Error diagnostic.
    pub holes: &'r BTreeSet<ElementName>,
    /// The recursion guard's policy, which every round over the tree follows.
    pub guard: GuardPolicy,
    /// How many rounds lowering may run; see [`rank`](super::rank). Values
    /// below one are treated as one.
    pub rank: u32,
    /// The identifiers of the style and script definitions (on LaTeX, package
    /// requests and definition pieces) that the selected rules require
    /// whenever they are selected
    /// ([`selection_requires`](super::selection_requires)). Every lowered
    /// tree's requirements contain them, in addition to what the rules record
    /// at their nodes.
    pub selection_requires: &'r BTreeSet<Name>,
}

/// What lowering one tree produced: the lowered tree, if any, and the
/// diagnostics.
///
/// A failed lowering returns no tree and an Error diagnostic that says why; no
/// error is returned to the caller.
#[derive(Debug)]
pub struct LoweringOutcome {
    /// The lowered tree; `None` when the tree's lowering failed.
    pub lowered: Option<LoweredTree>,
    /// The diagnostics raised, in the order they were raised.
    pub diags: FlmDiagnostics,
}

/// Lowers one frozen tree for one backend: runs the rounds, calls the
/// selected rules, and returns the lowered tree with the diagnostics raised
/// (see the module documentation).
///
/// `cx` is the calling stage's context: the [`Lowerer`] reads the rules'
/// declared tables and options through it, and brings a spliced synthesized
/// value up to date through it. `tree_id` identifies the document tree of
/// which `tree` is the frozen form; it becomes the tree part of every
/// [`LoweredAddr`](super::LoweredAddr) into the result.
///
/// It never returns an error: a tree that cannot be lowered comes back with
/// [`LoweringOutcome::lowered`] set to `None` and an Error diagnostic. Call it
/// on a worker thread (the runner's stages already run on one), since the walk
/// recurses as deeply as the tree nests.
///
/// # Panics
///
/// Panics if `inputs.selected` was built against a registry other than
/// `inputs.registry`, so that a selected rule's index is out of range for that
/// registry's rule list. The lowering stage and the pieces stage always pass a
/// matching pair; only inputs assembled by hand from two registries can reach
/// this.
pub fn lower_tree(
    cx: &StageCx<'_>,
    inputs: &LoweringInputs<'_>,
    tree_id: TreeId,
    tree: &Arc<NodeTree<Flm>>,
) -> LoweringOutcome {
    let mut pass = Pass::new(cx, inputs, tree_id, tree.root().id());
    lower_rounds(&mut pass, inputs, tree)
}

/// Lowers a synthesized value for one backend: brings it up to date, then
/// runs the rounds over it exactly as [`lower_tree`] runs them over a document
/// tree.
///
/// A **synthesized value** is a small tree that a stage builds while a run is
/// under way, held outside the document's trees (see [synthesized
/// values](crate::guide::concepts_overview#synthesized-values)). This
/// function does for a whole value what a rule's [`Lowerer::splice`] does for
/// a value inside the node it lowers; a backend's pieces stage lowers each
/// [piece](crate::core::piece::Piece) with it.
///
/// The value is brought up to date by [value
/// catch-up](crate::guide::concepts_overview#value-catch-up)
/// ([`StageCx::resolve_value`]): the plan's rewrites run over it, then the
/// per-tree stages the calling stage depends on, and what they compute goes
/// into a catch-up store of the value's own. Every table read a rule makes at
/// an address of the value reads that catch-up store first, so a per-tree
/// table holds the value's own entries, and every table that no re-entered
/// stage writes (the document-scope tables: anchors, counters, the label
/// index) is the document's, unchanged. A reference inside a value therefore
/// resolves to the document's labels. Content of the document that a rule
/// inside the value lowers through [`Lowerer::lower_region`] is read at the
/// document's addresses, in the run's store, never in the value's catch-up
/// store.
///
/// The diagnostics the catch-up raised come first in the outcome, then the
/// lowering's own, as a splice records them. A value whose lowering fails
/// comes back with [`LoweringOutcome::lowered`] set to `None` and an Error
/// diagnostic, as a tree does.
///
/// `cx` is the calling stage's context. The catch-up runs again the per-tree
/// stages *that* stage depends on, so a stage calling this function must
/// declare the needs of the rules it lowers with
/// ([`rules_needs`](super::rules_needs)). Call it on a worker thread, as
/// [`lower_tree`].
///
/// # Errors
///
/// Returns the [`CatchUpError`] when the value could not be brought up to
/// date: a rewrite refused it, it nests too deeply, or a re-entered stage
/// produced a fact about the document as a whole (a labeled reference point,
/// for example), which a synthesized value may not state. Nothing was
/// lowered then; the caller records the error's
/// [diagnostic](CatchUpError::diagnostic), when it has one.
///
/// # Panics
///
/// Panics if `inputs.selected` was built against a registry other than
/// `inputs.registry`, so that a selected rule's index is out of range for that
/// registry's rule list. The lowering stage and the pieces stage always pass a
/// matching pair; only inputs assembled by hand from two registries can reach
/// this.
///
/// A rewrite or a re-entered stage that panics propagates out of this call,
/// as it does out of [`StageCx::resolve_value`].
pub fn lower_value(
    cx: &StageCx<'_>,
    inputs: &LoweringInputs<'_>,
    value: &Arc<NodeTree<Flm>>,
) -> Result<LoweringOutcome, CatchUpError> {
    let caught = cx.resolve_value(value)?;
    // The value is lowered as a tree of its own: its nodes answer the value's
    // addresses from the first round on, and its scratch store stays the
    // overlay of every read for all the rounds. See decision rationale at
    // [§flm-dd-dr:rend-pieces-composer].
    let mut pass = Pass::new(cx, inputs, caught.id, caught.tree.root().id());
    pass.stores.push((caught.id, Arc::clone(&caught.tree), caught.scratch));
    let lowered = lower_rounds(&mut pass, inputs, &caught.tree);
    let mut diags = caught.diags;
    for diagnostic in lowered.diags {
        diags.push(diagnostic);
    }
    Ok(LoweringOutcome { lowered: lowered.lowered, diags })
}

/// The rounds over one tree, shared by [`lower_tree`] and [`lower_value`].
fn lower_rounds(
    pass: &mut Pass<'_>,
    inputs: &LoweringInputs<'_>,
    tree: &Arc<NodeTree<Flm>>,
) -> LoweringOutcome {
    let bound = inputs.rank.max(1);
    let mut current = Arc::clone(tree);
    let mut rounds: u32 = 0;
    loop {
        rounds += 1;
        pass.mode = if rounds == 1 { Mode::Frozen } else { Mode::Chain };
        pass.round_tree = Arc::as_ptr(&current) as usize;
        let output = match guard::restager(&mut *pass, inputs.guard).restage(&current) {
            Ok(output) => output,
            Err(error) => {
                let failure = pass.failure_of(error);
                let mut diags = pass.take_diags();
                diags.push(failure_diagnostic(failure, current.root().span().clone()));
                return LoweringOutcome { lowered: None, diags };
            }
        };
        pass.previous = std::mem::take(&mut pass.round_origins);
        pass.origins.clear();
        let remaining = non_native(&output, inputs.native);
        if remaining.is_empty() {
            let lowered =
                LoweredTree { tree: Arc::new(output), requires: pass.requires.clone(), rounds };
            return LoweringOutcome { lowered: Some(lowered), diags: pass.take_diags() };
        }
        if rounds >= bound {
            let mut diags = pass.take_diags();
            diags.push(Diagnostic::error(
                RoundsExceeded {
                    backend: inputs.backend.to_string(),
                    rank: bound,
                    remaining: remaining.iter().map(|e| e.as_str()).collect::<Vec<_>>().join(", "),
                },
                current.root().span().clone(),
            ));
            return LoweringOutcome { lowered: None, diags };
        }
        current = Arc::new(output);
    }
}

/// The elements of `tree` that are not in `native`, in name order and
/// without repetition. [`lower_tree`] runs another round while this list is
/// not empty.
///
/// A node of an element with opaque content ([`OpaqueContent`]) is counted
/// itself, but its content is not examined: everything inside a math or
/// verbatim region is copied as source text and never lowered. The only
/// exception is a node carrying [`ContentInOpaque`] (the argument of a text
/// command inside a formula): it holds document content, so it and
/// everything under it are counted like content anywhere else.
pub fn non_native(tree: &NodeTree<Flm>, native: &BTreeSet<ElementName>) -> Vec<ElementName> {
    let mut found: BTreeSet<ElementName> = BTreeSet::new();
    // Each entry is a node and whether it stands inside opaque content.
    let mut stack: Vec<(NodeRef<'_, Flm>, bool)> = vec![(tree.root(), false)];
    while let Some((node, in_opaque)) = stack.pop() {
        let element = element_of(&node);
        // A node holding content ends the opaque stretch at itself.
        let in_opaque = in_opaque && !element.has_aspect::<dyn ContentInOpaque>();
        if !in_opaque && !native.contains(&element.name) {
            found.insert(element.name.clone());
        }
        let below = in_opaque || element.has_aspect::<dyn OpaqueContent>();
        stack.extend(node.children().iter().map(|child| (child, below)));
    }
    found.into_iter().collect()
}

/// Whether `node` is, or holds among its descendants, a node carrying
/// [`ContentInOpaque`]: document content inside opaque content.
///
/// The walks that copy opaque content use it to decide whether they must
/// descend into a subtree at all: without one of these nodes in it, an
/// opaque subtree is entirely source text.
fn holds_content(node: &NodeRef<'_, Flm>) -> bool {
    element_of(node).has_aspect::<dyn ContentInOpaque>()
        || node.descendants().any(|d| element_of(&d).has_aspect::<dyn ContentInOpaque>())
}

/// A visitor carrying every node over unchanged, for the verbatim copies of
/// opaque content.
struct Verbatim;

impl RestageVisitor<Flm, (), ()> for Verbatim {
    type Error = Infallible;

    fn restage(
        &mut self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, Infallible> {
        Ok(Restage::Descend(()))
    }
}

/// Stages a copy of `node`'s whole subtree, every node copied unchanged with
/// its kind, span, parsing state and extension, and returns the staged copy.
///
/// A rule uses it to place the content of an [opaque
/// region](crate::guide::concepts_overview#opaque-regions) (a math group, a
/// verbatim group) in a presentation node: nothing inside it is lowered, and
/// an emitter writes it again as source text.
///
/// The copy must become the body of an element that carries the
/// [`OpaqueContent`] aspect, such as `flm.pres.math-passthrough` or
/// `flm.pres.verbatim`. Nothing checks this, and a mistake is reported far
/// from its cause: the check that ends the rounds examines nothing inside an
/// opaque subtree except the content of a [`ContentInOpaque`] node, and the
/// number of rounds allowed counts only what a rule declares in `produces`.
/// A copy of content the backend does not write natively, placed anywhere
/// else, therefore needs rounds that neither of them counts, and the tree
/// fails with `flm.lowering.rounds-exceeded`.
///
/// A rule that puts content inside such a body stages it as a node carrying
/// the [`ContentInOpaque`] aspect and does not copy it through here.
///
/// # Errors
///
/// Returns a [`LowerError::Build`] when the subtree cannot be staged, or when
/// staging it unexpectedly gives other than exactly one node.
pub fn copy_subtree_verbatim(
    cx: &mut LowerCx<'_>,
    node: NodeRef<'_, Flm>,
) -> Result<BuildId, LowerError> {
    let ids = cx.restage_subtree(node, &mut Verbatim).map_err(infallible_restage)?;
    match ids.as_slice() {
        [id] => Ok(*id),
        _ => Err(LowerError::Build {
            detail: format!(
                "a verbatim copy must give exactly one node, but it gave {}",
                ids.len()
            ),
        }),
    }
}

/// Stages unchanged copies of a run of nodes (a math group's children, a
/// verbatim group's children) by applying [`copy_subtree_verbatim`] to each,
/// and returns the copies in order.
///
/// The same restriction applies: the copies belong in the body of an
/// [`OpaqueContent`] element and nowhere else.
///
/// # Errors
///
/// Returns the first [`LowerError`] [`copy_subtree_verbatim`] returns.
pub fn copy_run_verbatim(
    cx: &mut LowerCx<'_>,
    nodes: NodeSlice<'_, Flm>,
) -> Result<Vec<BuildId>, LowerError> {
    let mut ids = Vec::with_capacity(nodes.len());
    for node in nodes.iter() {
        ids.push(copy_subtree_verbatim(cx, node)?);
    }
    Ok(ids)
}

/// The content of a verbatim region as [`read_verbatim_content`] returns it.
#[derive(Debug)]
pub enum VerbatimContent<'t> {
    /// Characters: the nodes, to be copied unchanged
    /// ([`copy_run_verbatim`]).
    Characters(NodeSlice<'t, Flm>),
    /// Content that is not characters, already reported: its source text,
    /// which the region shows instead.
    Spelled(String),
}

/// Reads the content `nodes` of a verbatim region as characters, and reports
/// [`ContentNotCharacters`] at `owner` when it is not characters.
///
/// A **verbatim region** is a run of raw text in which no markup character
/// means anything (see [verbatim
/// regions](crate::guide::concepts_overview#verbatim-regions)): the content
/// of a verbatim argument or of a verbatim environment body. What a document
/// writes there is characters by construction. A user definition's template
/// may place one of its arguments inside verbatim content, though, and the
/// expansion substitutes the argument's content there unchanged: a command,
/// a formula, anything the use site passed. Every rule that stages verbatim
/// content reads it through this function, so every verbatim region is
/// treated the same way: the default rule of a bare verbatim group
/// ([`VerbatimRule`](super::VerbatimRule)) and the rules of the `verbatim`
/// feature.
///
/// The content is read with [`chars_at_lowering`], the only reading of
/// characters at lowering: text gives its characters, a comment nothing, a
/// brace group its content without the braces. A command, an environment, a
/// special character sequence or mathematics has no characters to give; then
/// the Error is reported once, at `owner` (the command or environment that
/// owns the region, whose name the condition carries), and the result is the
/// whole content written as its source text ([`recompose_source`]), so that
/// `\emph{x}` is shown as those seven characters.
///
/// # Errors
///
/// [`LowerError::PlaceholderCharacters`] when the content holds a placeholder
/// of a template that is being lowered unexpanded: the recomposed LaTeX then
/// expands the use sites of the definition, where the placeholder's content
/// is known ([`chars_at_lowering`]).
pub fn read_verbatim_content<'t>(
    owner: &NodeRef<'_, Flm>,
    nodes: NodeSlice<'t, Flm>,
    lw: &mut dyn Lowerer,
) -> Result<VerbatimContent<'t>, LowerError> {
    // Content that is not characters, its reading and its recovery: see
    // decision rationale at [§flm-dd-dr:std-verbatim-not-characters].
    if chars_at_lowering(nodes.iter())?.is_ok() {
        return Ok(VerbatimContent::Characters(nodes));
    }
    let name = match owner.name() {
        Some(name) if owner.callable_type() == Some(FlmCallableType::Environment) => {
            format!("{{{name}}}")
        }
        Some(name) => format!("\\{name}"),
        None => String::from("a verbatim group"),
    };
    lw.diagnose(Diagnostic::error(ContentNotCharacters { name }, owner.span().clone()));
    let nodes: Vec<NodeRef<'t, Flm>> = nodes.iter().collect();
    Ok(VerbatimContent::Spelled(recompose_source(&nodes)))
}

/// Stages the content `nodes` of a verbatim region owned by `owner` for the
/// body of a `flm.pres.verbatim` node, and returns the staged nodes: the
/// nodes copied unchanged ([`copy_run_verbatim`]) or, for content that is not
/// characters, its source text as one text node ([`read_verbatim_content`],
/// which reports it).
///
/// # Errors
///
/// Returns the first [`LowerError`] staging the copies or the text node
/// returns.
pub fn stage_verbatim_content(
    owner: &NodeRef<'_, Flm>,
    nodes: NodeSlice<'_, Flm>,
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
) -> Result<Vec<BuildId>, LowerError> {
    match read_verbatim_content(owner, nodes, lw)? {
        VerbatimContent::Characters(nodes) => copy_run_verbatim(cx, nodes),
        VerbatimContent::Spelled(text) => Ok(vec![lw.text(cx, &text)?]),
    }
}

/// Lowers a run of sibling nodes through the rules, in order, and returns the
/// nodes they became.
///
/// This is the counterpart of [`copy_run_verbatim`] for document content
/// rather than source text: where that function copies a run of nodes
/// unchanged, this one gives the run back to the lowering rules, so each node
/// becomes what its own rule stages for it. A rule uses it when it has
/// selected a run of nodes (the content of one argument, one node of an
/// opaque region that holds document content rather than source text) and
/// wants the ordinary treatment for it rather than a treatment of its own.
///
/// `nodes` must be **siblings of one tree**, in their order in the tree, with
/// no gap between them: the content nodes of one argument, or the direct
/// children of one node, or a single node. An empty run is allowed and
/// stages nothing. Nothing checks the sibling requirement in advance; a run
/// that does not meet it fails with the structure error below.
///
/// The run is lowered as a whole, so a rule that reads a slot's content and
/// passes it here gets exactly what the driver would have produced for that
/// content on its own.
///
/// # Errors
///
/// Returns [`LowerError::Structure`] when `nodes` is not a run of siblings of
/// one tree, or belongs to another tree than the one the [`Lowerer`] is
/// lowering ([`Lowerer::lower_nodes`]), and any error lowering the run
/// returned.
pub fn lower_run(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    nodes: &[NodeRef<'_, Flm>],
) -> Result<Vec<BuildId>, LowerError> {
    let (Some(first), Some(last)) = (nodes.first(), nodes.last()) else { return Ok(Vec::new()) };
    let from = first.id().index() as u32;
    let to = last.id().index() as u32;
    let Some(slice) = first.tree().slice(from..to + 1) else {
        return Err(LowerError::structure(
            "the nodes to lower must be siblings in one tree, and they are not",
        ));
    };
    lw.lower_nodes(cx, slice)
}

/// Lowers the pieces of a key-value entry's value as the author wrote them
/// ([`WrittenPiece`], as [`key_value_pieces`](crate::lang::copy::key_value_pieces)
/// returns them), as ordinary content: a node through the rules ([`lower_run`]),
/// characters as text; an empty text stages nothing.
///
/// The LaTeX rules that relay such a value use it: a float's reference to an
/// unnumbered float with a caption's alternative text, a heading's
/// alternative title.
///
/// # Errors
///
/// As [`lower_run`], and any error staging the text.
pub(crate) fn lower_written_pieces(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    pieces: &[WrittenPiece<'_>],
) -> Result<Vec<BuildId>, LowerError> {
    let mut staged = Vec::new();
    for piece in pieces {
        match piece {
            WrittenPiece::Node(node) => staged.extend(lower_run(cx, lw, std::slice::from_ref(node))?),
            WrittenPiece::Text(text) if text.is_empty() => {}
            WrittenPiece::Text(text) => staged.push(lw.text(cx, text)?),
        }
    }
    Ok(staged)
}

/// The plain shape of a node region's content ([`RegionShape`]): the content
/// nodes lowered unchanged, as one run of siblings ([`lower_run`]).
///
/// Pass it to [`Lowerer::lower_region`] to show a node region of another
/// node where the rule's own node is placed, with nothing added by the rule:
/// `lw.lower_region(cx, at, region, &mut lower_region_content)`. `_node`,
/// the node the region belongs to, is not read.
///
/// # Errors
///
/// As [`lower_run`].
pub fn lower_region_content(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    _node: &NodeRef<'_, Flm>,
    content: &[NodeRef<'_, Flm>],
) -> Result<Vec<BuildId>, LowerError> {
    lower_run(cx, lw, content)
}

fn infallible_restage(error: RestageError<Infallible>) -> LowerError {
    LowerError::Build { detail: error.to_string() }
}

/// How the visited node's address and origin are computed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// The first round, over the frozen tree: a node is its own address.
    Frozen,
    /// A later round, over the previous round's output: the node's origin is
    /// what the previous round recorded for it.
    Chain,
    /// Inside a splice, over a caught-up value: the address is the value's,
    /// the origin the splicing rule's node.
    Value(TreeId),
    /// Inside a region lowering ([`Lowerer::lower_region`]), over the tree the
    /// region belongs to: a node is its own address and its own origin in
    /// that tree.
    Pointed(TreeId),
}

/// The per-visit state saved around the descent (techy's restager has no
/// exit callback, so each visit restores what it changed).
struct Visit {
    current: Option<RuleIdx>,
    origin: Option<NodeAddr>,
    addr: Option<NodeAddr>,
    trigger: Option<(FlmSpan, Arc<ParsingState<Flm>>)>,
    synth: Option<Arc<FlmSource>>,
}

/// The pass: the visitor of every round and the [`Lowerer`] every rule
/// receives.
struct Pass<'r> {
    cx: &'r StageCx<'r>,
    backend: &'r BackendName,
    registry: &'r FlmRegistry,
    selected: &'r SelectedRules,
    native: &'r BTreeSet<ElementName>,
    holes: &'r BTreeSet<ElementName>,
    doc_tree: TreeId,
    /// The root of the document tree being lowered: the address every
    /// fallback returns.
    doc_root: NodeId,
    driver_rule: RuleName,
    specs: BTreeMap<ElementName, Arc<FlmPresentationSpec>>,
    mode: Mode,
    /// The tree the current round visits, as the address of its allocation:
    /// outside a splice and a region lowering, a node of any other tree is
    /// not the round's to lower.
    round_tree: usize,
    /// The origins of the previous round's output, by node index.
    previous: Vec<NodeAddr>,
    /// The origin recorded for nodes staged inside a splice.
    inherit: Option<NodeAddr>,
    depth: u32,
    /// Whether the node being visited is inside opaque content whose copy is
    /// under way: such a node is copied as the source text it is.
    in_opaque: bool,
    // per visit
    current: Option<RuleIdx>,
    origin: Option<NodeAddr>,
    addr: Option<NodeAddr>,
    trigger: Option<(FlmSpan, Arc<ParsingState<Flm>>)>,
    synth: Option<Arc<FlmSource>>,
    // values and entered trees
    /// The values this pass holds, each with its own results: the value
    /// [`lower_value`] lowers, and every value a splice is lowering, innermost
    /// last. A table read at an address of one of them reads its results
    /// first; a read at any other address never does.
    stores: Vec<(TreeId, Arc<NodeTree<Flm>>, Computed)>,
    /// The trees a splice or a region lowering has entered, innermost last:
    /// the last one is the tree the `Lowerer` is lowering.
    entered: Vec<(TreeId, Arc<NodeTree<Flm>>)>,
    /// Where each value being spliced is shown, innermost last: the span of
    /// the node whose rule spliced it. Every diagnostic added to the
    /// collection meanwhile gets one frame per entry, innermost first.
    shown_at: Vec<FlmSpan>,
    // outputs
    origins: HashMap<BuildId, NodeAddr>,
    round_origins: Vec<NodeAddr>,
    requires: BTreeSet<Name>,
    diags: FlmDiagnostics,
    nesting_refused: Option<String>,
}

impl<'r> Pass<'r> {
    fn new(
        cx: &'r StageCx<'r>,
        inputs: &LoweringInputs<'r>,
        doc_tree: TreeId,
        doc_root: NodeId,
    ) -> Pass<'r> {
        Pass {
            cx,
            backend: inputs.backend,
            registry: inputs.registry,
            selected: inputs.selected,
            native: inputs.native,
            holes: inputs.holes,
            doc_tree,
            doc_root,
            driver_rule: RuleName::from(DRIVER_RULE),
            specs: BTreeMap::new(),
            mode: Mode::Frozen,
            round_tree: 0,
            previous: Vec::new(),
            inherit: None,
            depth: 0,
            in_opaque: false,
            current: None,
            origin: None,
            addr: None,
            trigger: None,
            synth: None,
            stores: Vec::new(),
            entered: Vec::new(),
            shown_at: Vec::new(),
            origins: HashMap::new(),
            round_origins: Vec::new(),
            requires: inputs.selection_requires.clone(),
            diags: FlmDiagnostics::new(),
            nesting_refused: None,
        }
    }

    /// Adds `diagnostic` to the pass's diagnostics. Inside a splice it first
    /// gets one frame for each value being spliced, innermost first, titled
    /// `SHOWN_HERE` and spanning the node at which the value is shown; its own
    /// span, where the content was written, is unchanged.
    fn report(&mut self, diagnostic: FlmDiagnostic) {
        let frames = self.shown_at.iter().rev();
        let frames = frames.map(|span| TraceFrame::new(SHOWN_HERE, span.clone())).collect();
        self.diags.push(diagnostic.with_outer_frames(frames));
    }

    fn take_diags(&mut self) -> FlmDiagnostics {
        std::mem::replace(&mut self.diags, FlmDiagnostics::new())
    }

    /// Puts the diagnostics back as they were when `(len, suppressed,
    /// error_count)` was read from them: the diagnostics pushed since, stored
    /// or only counted, are dropped. The counts read from the collection are
    /// consistent with its first `len` diagnostics, so the rebuild succeeds.
    fn restore_diags(&mut self, (len, suppressed, error_count): (usize, usize, usize)) {
        let kept = self.diags.as_slice()[..len].to_vec();
        if let Ok(restored) =
            FlmDiagnostics::from_parts(kept, self.diags.limit(), suppressed, error_count)
        {
            self.diags = restored;
        }
    }

    /// The condition that reports a failed round.
    fn failure_of(&mut self, error: RestageError<LowerError>) -> FailedTree {
        let backend = self.backend.to_string();
        if let Some(detail) = self.nesting_refused.take() {
            return FailedTree::Nesting(NestingTooDeep { backend, detail });
        }
        match error {
            RestageError::DescentLimitExceeded { detail } => {
                FailedTree::Nesting(NestingTooDeep { backend, detail })
            }
            RestageError::Visitor(inner) => {
                FailedTree::Failed(LoweringFailed { backend, detail: inner.to_string() })
            }
            other => FailedTree::Failed(LoweringFailed { backend, detail: other.to_string() }),
        }
    }

    /// A context operation's failure as the `Lowerer`'s error: the visitor's
    /// own error unwrapped, the guard's refusal remembered, anything else a
    /// staging failure (fatal).
    fn convert(&mut self, error: RestageError<LowerError>) -> LowerError {
        match error {
            RestageError::Visitor(inner) => inner,
            RestageError::DescentLimitExceeded { detail } => {
                self.nesting_refused = Some(detail.clone());
                LowerError::Build { detail }
            }
            other => LowerError::Build { detail: other.to_string() },
        }
    }

    fn is_fatal(error: &LowerError) -> bool {
        matches!(error, LowerError::Build { .. })
    }

    // --- per-visit context ---------------------------------------------------------------

    fn rule_name(&self) -> &RuleName {
        match self.current {
            Some(idx) => &self.registry.rule(idx).rule.name,
            None => &self.driver_rule,
        }
    }

    /// The running rule's name as a stage name: the conditions raised where a
    /// read is checked are the store's and identify a stage, so a rule's
    /// undeclared read is shown as "stage `<rule>`". The data is right and the
    /// word is the store's; a variant that identifies a rule would have to be
    /// added to the store.
    fn rule_as_stage(&self) -> StageName {
        StageName::from(self.rule_name().as_str())
    }

    fn trigger(&self) -> Result<&(FlmSpan, Arc<ParsingState<Flm>>), LowerError> {
        self.trigger
            .as_ref()
            .ok_or_else(|| {
                LowerError::other(
                    "the lowering rule interface may be used only while a node is being lowered, \
                     but it was used outside that",
                )
            })
    }

    /// The synthesized source of the pair (visited node, rule): created once
    /// per visit, at the node, labeled with the rule's name.
    fn synth_source(&mut self) -> Result<Arc<FlmSource>, LowerError> {
        if let Some(source) = &self.synth {
            return Ok(Arc::clone(source));
        }
        let (span, _) = self.trigger()?;
        let label = format!("lowering rule `{}`", self.rule_name());
        let source = Arc::new(
            Source::synthesized(format!("«{}»", self.rule_name()), label.clone(), span.clone())
                .with_origin(FlmOrigin::synthetic(label)),
        );
        self.synth = Some(Arc::clone(&source));
        Ok(source)
    }

    fn spec_for(&mut self, element: &ElementName) -> Result<Arc<FlmPresentationSpec>, LowerError> {
        if let Some(spec) = self.specs.get(element) {
            return Ok(Arc::clone(spec));
        }
        let entry = self.registry.presentation_element(element).ok_or_else(|| {
            LowerError::other(format!("`{element}` is not a registered presentation element"))
        })?;
        let spec = Arc::new(FlmPresentationSpec::new(Arc::clone(&entry.element)));
        self.specs.insert(element.clone(), Arc::clone(&spec));
        Ok(spec)
    }

    fn record(&mut self, id: BuildId) -> BuildId {
        if let Some(origin) = self.origin {
            self.origins.insert(id, origin);
        }
        id
    }

    // --- dispatch --------------------------------------------------------------------------

    fn dispatch(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
    ) -> Result<Vec<BuildId>, LowerError> {
        let element = Arc::clone(element_of(&node));
        // Inside opaque content, a node is the source it is and is carried
        // over; a node carrying `ContentInOpaque` holds content instead, and
        // is dispatched like a node anywhere else — it and everything under
        // it.
        if self.in_opaque {
            if !element.has_aspect::<dyn ContentInOpaque>() {
                return self.copy(cx, node);
            }
            let saved = std::mem::replace(&mut self.in_opaque, false);
            let out = self.dispatch_content(node, cx, element);
            self.in_opaque = saved;
            return out;
        }
        self.dispatch_content(node, cx, element)
    }

    /// The dispatch itself, on a node that is document content: the three
    /// steps of a round (see the [module documentation](self)).
    fn dispatch_content(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        element: Arc<ElementSpec>,
    ) -> Result<Vec<BuildId>, LowerError> {
        if self.native.contains(&element.name) {
            if element.has_aspect::<dyn OpaqueContent>() {
                // Source through and through: the subtree is copied in one
                // step. Where content stands in it, the copy descends instead,
                // so that the content is lowered where it stands.
                if !node.children().iter().any(|child| holds_content(&child)) {
                    let id = copy_subtree_verbatim(cx, node)?;
                    return Ok(vec![self.record(id)]);
                }
                let saved = std::mem::replace(&mut self.in_opaque, true);
                let out = self.copy(cx, node);
                self.in_opaque = saved;
                return out;
            }
            return self.copy(cx, node);
        }
        if let Some(idx) = self.selected.rule_for(&element.name) {
            let registry = self.registry;
            let rule = &registry.rule(idx).rule;
            self.current = Some(idx);
            let outcome = rule.rule.lower(node, cx, self);
            self.current = None;
            return match outcome {
                Ok(Lowering::Emit(ids)) => Ok(ids),
                Ok(Lowering::Default) => self.copy(cx, node),
                Err(error) if Pass::is_fatal(&error) => Err(error),
                // Passed on unreported to the rule that spliced the template
                // the placeholder stands in, which acts on it.
                Err(LowerError::PlaceholderCharacters) => Err(LowerError::PlaceholderCharacters),
                Err(LowerError::Refused(diagnostic)) => {
                    self.report(*diagnostic);
                    self.unsupported(
                        cx,
                        node,
                        &element.name,
                        format!("rule `{}` failed", rule.name),
                    )
                }
                Err(LowerError::RegionNotFound { detail }) => {
                    self.report(Diagnostic::error(
                        RegionNotFound {
                            backend: self.backend.to_string(),
                            rule: rule.name.to_string(),
                            element: element.name.to_string(),
                            detail,
                        },
                        node.span().clone(),
                    ));
                    self.unsupported(
                        cx,
                        node,
                        &element.name,
                        format!("rule `{}` failed", rule.name),
                    )
                }
                Err(error) => {
                    self.report(Diagnostic::error(
                        RuleFailed {
                            backend: self.backend.to_string(),
                            rule: rule.name.to_string(),
                            element: element.name.to_string(),
                            detail: error.to_string(),
                        },
                        node.span().clone(),
                    ));
                    self.unsupported(
                        cx,
                        node,
                        &element.name,
                        format!("rule `{}` failed", rule.name),
                    )
                }
            };
        }
        if self.holes.contains(&element.name) {
            self.report(Diagnostic::error(
                UnsupportedElement {
                    backend: self.backend.to_string(),
                    element: element.name.to_string(),
                },
                node.span().clone(),
            ));
        } else {
            self.report(Diagnostic::error(
                NoRule { backend: self.backend.to_string(), element: element.name.to_string() },
                node.span().clone(),
            ));
        }
        let message = format!("backend `{}` cannot render `{}`", self.backend, element.name);
        self.unsupported(cx, node, &element.name, message)
    }

    /// The childless stand-in for a node the backend cannot render.
    fn unsupported(
        &mut self,
        cx: &mut LowerCx<'_>,
        node: NodeRef<'_, Flm>,
        element: &ElementName,
        message: String,
    ) -> Result<Vec<BuildId>, LowerError> {
        let payload: Arc<dyn PresPayload> =
            Arc::new(UnsupportedPayload { element: element.clone(), message });
        let name = ElementName::from(pres::UNSUPPORTED);
        let label = format!("unsupported `{element}`");
        let source = Arc::new(
            Source::synthesized(format!("«{element}»"), label.clone(), node.span().clone())
                .with_origin(FlmOrigin::synthetic(label)),
        );
        let state = Arc::clone(node.parsing_state());
        let body = vec![(String::from(pres::BODY_SLOT), Vec::new())];
        let id = self.stage_node(cx, &name, payload, body, Vec::new(), source, state).map_err(
            // A registry without the vocabulary cannot recover a hole: fatal.
            |error| LowerError::Build { detail: error.to_string() },
        )?;
        Ok(vec![id])
    }

    /// The driver's copy: the node added again over its children, lowered in
    /// turn. A leaf is copied unchanged; a group or a list over its lowered
    /// children; a callable is rebuilt argument by argument and slot by slot
    /// (techy's content-swap route, so the argument and slot records keep
    /// their meaning).
    fn copy(
        &mut self,
        cx: &mut LowerCx<'_>,
        node: NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, LowerError> {
        let id = match node.kind() {
            NodeKind::Chars { .. } | NodeKind::Comment(_) => cx
                .builder()
                .add(
                    node.kind().clone(),
                    node.span().clone(),
                    Arc::clone(node.parsing_state()),
                    Vec::new(),
                    node.ext().clone(),
                    (),
                )
                .map_err(|e| LowerError::Build { detail: e.to_string() })?,
            NodeKind::Group(_) | NodeKind::List => {
                let children = cx.restage_children(node, self).map_err(|e| self.convert(e))?;
                cx.builder()
                    .add(
                        node.kind().clone(),
                        node.span().clone(),
                        Arc::clone(node.parsing_state()),
                        children,
                        node.ext().clone(),
                        (),
                    )
                    .map_err(|e| LowerError::Build { detail: e.to_string() })?
            }
            NodeKind::Callable(data) => {
                let mut arguments = Vec::with_capacity(data.arguments.len());
                for (index, record) in data.arguments.iter().enumerate() {
                    if !record.is_provided() {
                        arguments.push(RestagedArgument::absent(Arc::clone(&record.spec)));
                        continue;
                    }
                    let content = self.lower_slice(cx, node.argument_content_nodes(index))?;
                    let bundle = cx
                        .restage_argument_with_content::<LowerError>(node, index, content, ())
                        .map_err(|e| self.convert(e))?;
                    arguments.push(bundle);
                }
                let mut slots = Vec::with_capacity(data.slots.len());
                for index in 0..data.slots.len() {
                    let content = self.lower_slice(cx, node.slot_content_nodes(index))?;
                    let bundle = cx
                        .restage_slot_with_content::<LowerError>(node, index, content, ())
                        .map_err(|e| self.convert(e))?;
                    slots.push(bundle);
                }
                cx.restage_invocation::<LowerError>(node, arguments, slots, ())
                    .map_err(|e| self.convert(e))?
            }
        };
        Ok(vec![self.record(id)])
    }

    /// Lowers the content nodes of an argument or a slot (nothing when
    /// absent).
    fn lower_slice(
        &mut self,
        cx: &mut LowerCx<'_>,
        content: Option<NodeSlice<'_, Flm>>,
    ) -> Result<Vec<BuildId>, LowerError> {
        let Some(content) = content else { return Ok(Vec::new()) };
        // Another tree's nodes are lowered through `lower_region` alone,
        // which enters their tree; lowered here they would answer the
        // addresses of the tree being lowered.
        if content.iter().next().is_some_and(|first| !self.is_input_tree(&first)) {
            return Err(LowerError::structure(
                "the nodes to lower must belong to the tree being lowered, but they belong to \
                 another tree; lower another tree's content through `Lowerer::lower_region`",
            ));
        }
        let mut ids = Vec::new();
        for child in content.iter() {
            ids.extend(cx.restage_subtree(child, self).map_err(|e| self.convert(e))?);
        }
        Ok(ids)
    }

    // --- synthesis -----------------------------------------------------------------------

    /// Stages a presentation node of `element` over its content slots (in
    /// order; the one named `body` marked as the body) and `attached` (one
    /// attached slot per decoration moved out of a content slot, after the
    /// content slots), on the given source and state. The node extension is
    /// created by `Flm::make_node_ext` and the payload set here. techy
    /// requires the slots to cover the children in order, without gaps.
    #[allow(clippy::too_many_arguments)]
    fn stage_node(
        &mut self,
        cx: &mut LowerCx<'_>,
        element: &ElementName,
        payload: Arc<dyn PresPayload>,
        content: Vec<(String, Vec<BuildId>)>,
        attached: Hoisted,
        source: Arc<FlmSource>,
        state: Arc<ParsingState<Flm>>,
    ) -> Result<BuildId, LowerError> {
        let spec = self.spec_for(element)?;
        let mut children: Vec<BuildId> = Vec::new();
        let mut slots = Vec::with_capacity(content.len() + attached.len());
        for (name, nodes) in content {
            let start = children.len() as u32;
            let len = nodes.len() as u32;
            children.extend(nodes);
            let marker = if name == pres::BODY_SLOT {
                BodyMarker::make_body()
            } else {
                BodyMarker::not_body()
            };
            slots.push(ParsedSlot::new(
                ChildRegion::new(start..start + len, ContentNodes::InRegion(0..len)),
                name.as_str(),
                SlotRole::Content,
                marker,
            ));
        }
        for (name, node) in attached {
            let start = children.len() as u32;
            children.push(node);
            slots.push(ParsedSlot::new(
                ChildRegion::new(start..start + 1, ContentNodes::InRegion(0..1)),
                name.as_str(),
                SlotRole::Attached,
                BodyMarker::not_body(),
            ));
        }
        let kind = NodeKind::callable(
            spec.callable_data(ParsedArguments::empty(), ParsedSlots::new(slots)),
        );
        let span = SourceSpan::entire(&source);
        let builder = cx.builder();
        let ext = Flm::make_node_ext(&kind, &span, &state, builder.staged_children(&children))
            .map_err(|e| LowerError::Build { detail: e.to_string() })?
            .with_payload(payload);
        let id = builder
            .add(kind, span, state, children, ext, ())
            .map_err(|e| LowerError::Build { detail: e.to_string() })?;
        Ok(self.record(id))
    }

    /// Splits the staged nodes of one content slot into its content, with a
    /// decoration mark at each decoration's former position, and moves the
    /// decorations into `attached`. A mark gives its decoration's attached
    /// slot by index in the node's slot list: the attached slots follow the
    /// `content_slots` content slots, in the order the decorations were
    /// moved.
    fn hoist(
        &mut self,
        cx: &mut LowerCx<'_>,
        nodes: Vec<BuildId>,
        content_slots: u32,
        attached: &mut Hoisted,
    ) -> Result<Vec<BuildId>, LowerError> {
        let decorations: Vec<(usize, ElementName)> = {
            let staged = cx.builder().staged_nodes();
            nodes
                .iter()
                .enumerate()
                .filter_map(|(index, id)| {
                    let element = &staged.get(*id)?.ext().element;
                    element.has_aspect::<dyn Decoration>().then(|| (index, element.name.clone()))
                })
                .collect()
        };
        if decorations.is_empty() {
            return Ok(nodes);
        }
        let mut content = Vec::with_capacity(nodes.len());
        for (index, id) in nodes.into_iter().enumerate() {
            match decorations.iter().find(|(at, _)| *at == index) {
                Some((_, name)) => {
                    let slot = content_slots + attached.len() as u32;
                    let origin = self.origins.get(&id).copied().or(self.origin);
                    attached.push((name.clone(), id));
                    content.push(self.deco_mark(cx, slot, origin)?);
                }
                None => content.push(id),
            }
        }
        Ok(content)
    }

    /// The `Lowerer`'s staging of a presentation node for the running rule:
    /// the `produces` check, the move of decorations out of every content
    /// slot, the shared synthesized source and the node's state.
    fn stage_pres(
        &mut self,
        cx: &mut LowerCx<'_>,
        element: &ElementName,
        payload: Arc<dyn PresPayload>,
        slots: Vec<(String, Vec<BuildId>)>,
    ) -> Result<BuildId, LowerError> {
        let Some(idx) = self.current else {
            return Err(LowerError::other(
                "`pres` may be called only by a lowering rule, but no lowering rule is running",
            ));
        };
        let decl = &self.registry.rule(idx).rule.decl;
        if !decl.produces.contains(element) {
            return Err(LowerError::UndeclaredProduces {
                rule: self.registry.rule(idx).rule.name.clone(),
                element: element.clone(),
                declared: decl.produces.clone(),
            });
        }
        let content_slots = slots.len() as u32;
        let mut attached = Vec::new();
        let mut content = Vec::with_capacity(slots.len());
        for (name, nodes) in slots {
            let nodes = self.hoist(cx, nodes, content_slots, &mut attached)?;
            content.push((name, nodes));
        }
        let source = self.synth_source()?;
        let (_, state) = self.trigger()?;
        let state = Arc::clone(state);
        self.stage_node(cx, element, payload, content, attached, source, state)
    }

    /// The childless decoration mark left where a decoration was moved out;
    /// its origin is the decoration's, not the block's.
    fn deco_mark(
        &mut self,
        cx: &mut LowerCx<'_>,
        slot: u32,
        origin: Option<NodeAddr>,
    ) -> Result<BuildId, LowerError> {
        let name = ElementName::from(pres::DECO_MARK);
        let source = self.synth_source()?;
        let (_, state) = self.trigger()?;
        let state = Arc::clone(state);
        let payload: Arc<dyn PresPayload> = Arc::new(DecoMarkPayload { slot });
        let saved = self.origin;
        self.origin = origin;
        let body = vec![(String::from(pres::BODY_SLOT), Vec::new())];
        let result = self.stage_node(cx, &name, payload, body, Vec::new(), source, state);
        self.origin = saved;
        result
    }

    // --- the origin chain ------------------------------------------------------------

    /// After the root's visit: the origin of every node of the round's output
    /// in the tree's storage order — techy's breadth-first layout replayed
    /// over the staged graph — a node without a recorded origin inheriting
    /// its parent's.
    fn finish_round(&mut self, cx: &mut LowerCx<'_>, root: BuildId) {
        let staged = cx.builder().staged_nodes();
        let root_origin = self
            .origins
            .get(&root)
            .copied()
            .or(self.origin)
            .unwrap_or(NodeAddr::new(self.doc_tree, self.doc_root));
        let mut order: Vec<(BuildId, NodeAddr)> = vec![(root, root_origin)];
        let mut position = 0;
        while position < order.len() {
            let (id, origin) = order[position];
            if let Some(view) = staged.get(id) {
                for child in view.children() {
                    let own = self.origins.get(child).copied().unwrap_or(origin);
                    order.push((*child, own));
                }
            }
            position += 1;
        }
        self.round_origins = order.into_iter().map(|(_, origin)| origin).collect();
    }

    /// The origin of a node of the tree being visited under the current mode.
    fn origin_of_input(&self, node: &NodeRef<'_, Flm>) -> Option<NodeAddr> {
        match self.mode {
            Mode::Frozen => Some(NodeAddr::new(self.doc_tree, node.id())),
            Mode::Chain => self.previous.get(node.id().index()).copied(),
            Mode::Value(_) => self.inherit,
            Mode::Pointed(id) => Some(NodeAddr::new(id, node.id())),
        }
    }

    /// The address at which a node of the tree being visited reads tables.
    fn addr_of_input(&self, node: &NodeRef<'_, Flm>) -> Option<NodeAddr> {
        match self.mode {
            Mode::Frozen => Some(NodeAddr::new(self.doc_tree, node.id())),
            Mode::Chain => self.previous.get(node.id().index()).copied(),
            Mode::Value(id) | Mode::Pointed(id) => Some(NodeAddr::new(id, node.id())),
        }
    }

    /// Whether `node` belongs to the tree currently being visited.
    fn is_input_tree(&self, node: &NodeRef<'_, Flm>) -> bool {
        let key = node.tree() as *const NodeTree<Flm> as usize;
        match self.mode {
            Mode::Value(id) | Mode::Pointed(id) => self
                .entered
                .last()
                .is_some_and(|(entered, tree)| *entered == id && Arc::as_ptr(tree) as usize == key),
            Mode::Frozen | Mode::Chain => self.round_tree == key,
        }
    }

    /// The tree with the identifier `id` that a region lowering may lower
    /// content of: a frozen document tree, or a synthesized value this pass
    /// holds.
    fn reachable_tree(&self, id: TreeId) -> Option<Arc<NodeTree<Flm>>> {
        if id.is_document_tree() {
            return self.cx.frozen_tree(id).cloned();
        }
        self.stores
            .iter()
            .rev()
            .find(|(value, _, _)| *value == id)
            .map(|(_, tree, _)| Arc::clone(tree))
    }
}

/// The condition that reports a tree whose lowering failed.
enum FailedTree {
    Nesting(NestingTooDeep),
    Failed(LoweringFailed),
}

/// The Error diagnostic of a failed tree, at the tree's root.
fn failure_diagnostic(failure: FailedTree, span: FlmSpan) -> FlmDiagnostic {
    match failure {
        FailedTree::Nesting(condition) => Diagnostic::error(condition, span),
        FailedTree::Failed(condition) => Diagnostic::error(condition, span),
    }
}

impl<'r> RestageVisitor<Flm, (), ()> for Pass<'r> {
    type Error = LowerError;

    fn restage(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, LowerError> {
        self.depth += 1;
        let saved = Visit {
            current: self.current.take(),
            origin: self.origin,
            addr: self.addr,
            trigger: self.trigger.take(),
            synth: self.synth.take(),
        };
        self.origin = self.origin_of_input(&node);
        self.addr = self.addr_of_input(&node);
        self.trigger = Some((node.span().clone(), Arc::clone(node.parsing_state())));

        let mut result = self.dispatch(node, cx);
        if self.depth == 1 {
            // The tree root is a container the backend claims natively, copied
            // as exactly one node; anything else would leave the next round's
            // origin chain empty, so it is a failure rather than a silent gap.
            let root = match result.as_deref() {
                Ok([root]) => Ok(Some(*root)),
                Ok(other) => Err(other.len()),
                Err(_) => Ok(None),
            };
            match root {
                Ok(Some(root)) => self.finish_round(cx, root),
                Ok(None) => {}
                Err(count) => {
                    result = Err(LowerError::Build {
                        detail: format!(
                            "the root of a tree must lower to exactly one node, but it lowered \
                             to {count} nodes"
                        ),
                    });
                }
            }
        }

        self.depth -= 1;
        self.current = saved.current;
        self.origin = saved.origin;
        self.addr = saved.addr;
        self.trigger = saved.trigger;
        self.synth = saved.synth;
        result.map(Restage::Emit)
    }
}

impl<'r> Pass<'r> {
    /// [`Lowerer::splice`] once the node the value is shown at is recorded.
    fn splice_shown(
        &mut self,
        cx: &mut LowerCx<'_>,
        value: &Arc<NodeTree<Flm>>,
    ) -> Result<Vec<BuildId>, LowerError> {
        // What the value's lowering reports and requests is discarded when it
        // ends in a placeholder's outcome: the rule that spliced the value
        // writes something else in its place, which reports and requests its
        // own.
        let diags_on_entry = (self.diags.len(), self.diags.suppressed(), self.diags.error_count());
        let requires_on_entry = self.requires.clone();
        let caught = match self.cx.resolve_value(value) {
            Ok(caught) => caught,
            Err(CatchUpError::Store(StoreError::DocumentFactInValue { stage, table })) => {
                return Err(LowerError::DocumentFactInValue { stage, table });
            }
            Err(error) => {
                if let Some(diagnostic) = error.diagnostic() {
                    self.report(diagnostic.clone());
                }
                return Err(LowerError::CatchUp { detail: error.to_string() });
            }
        };
        for diagnostic in caught.diags {
            self.report(diagnostic);
        }
        let tree = Arc::clone(&caught.tree);
        self.stores.push((caught.id, Arc::clone(&tree), caught.scratch));
        self.entered.push((caught.id, Arc::clone(&tree)));
        let saved_mode = self.mode;
        let saved_inherit = self.inherit;
        self.mode = Mode::Value(caught.id);
        self.inherit = self.origin;
        let result = (|| {
            let root = tree.root();
            let mut ids = Vec::new();
            if root.is_list() {
                for child in root.children().iter() {
                    ids.extend(cx.restage_subtree(child, self).map_err(|e| self.convert(e))?);
                }
            } else {
                ids.extend(cx.restage_subtree(root, self).map_err(|e| self.convert(e))?);
            }
            Ok(ids)
        })();
        self.mode = saved_mode;
        self.inherit = saved_inherit;
        self.entered.pop();
        self.stores.pop();
        if let Err(LowerError::PlaceholderCharacters) = result {
            self.restore_diags(diags_on_entry);
            self.requires = requires_on_entry;
        }
        result
    }
}

impl<'r> Lowerer for Pass<'r> {
    fn rule(&self) -> &RuleName {
        self.rule_name()
    }

    fn backend(&self) -> &BackendName {
        self.backend
    }

    fn comment_start(&self) -> Option<&str> {
        self.registry.lang().parsing().comment_start()
    }

    fn lang(&self) -> &BuiltLang {
        self.registry.lang()
    }

    fn tree_id(&self) -> TreeId {
        self.entered.last().map(|(id, _)| *id).unwrap_or(self.doc_tree)
    }

    fn layout(&self) -> FragmentLayout {
        let id = self.tree_id();
        // A value is brought up to date under the default layout, both by a
        // splice and by `lower_value`; a document tree answers its fragment's,
        // which every tree the run lowers has — the tree being lowered and a
        // tree a region lowering entered alike.
        if id.is_value() { FragmentLayout::default() } else { self.cx.layout(id).unwrap_or_default() }
    }

    fn addr(&self) -> NodeAddr {
        // The façade is handed to rules inside visits only, where the address
        // is set; the fallback (the document tree's root) is unreachable.
        self.addr.unwrap_or(NodeAddr::new(self.doc_tree, self.doc_root))
    }

    fn addr_of(&self, node: &NodeRef<'_, Flm>) -> Result<NodeAddr, LowerError> {
        if !self.is_input_tree(node) {
            return Err(LowerError::structure(
                "the node must belong to the tree being lowered, but it belongs to another tree",
            ));
        }
        self.addr_of_input(node).ok_or_else(|| {
            LowerError::structure("the node has no address in the tree being lowered")
        })
    }

    fn placement(&self) -> Option<TreeId> {
        self.cx.placement_of(self.addr())
    }

    fn get_erased(
        &self,
        table: &crate::core::data::table::TableRef,
    ) -> Result<Option<&(dyn std::any::Any + Send + Sync)>, AccessError> {
        let Some(idx) = self.current else {
            return Err(AccessError::UndeclaredRead {
                stage: self.rule_as_stage(),
                table: table.name.clone(),
                declared: Vec::new(),
            });
        };
        let decl = &self.registry.rule(idx).rule.decl;
        // The declared list is for the error arms only: a rule reading a
        // table at every node of its element does not pay for it.
        let declared = || {
            let mut names: Vec<CapName> =
                decl.reads.iter().chain(&decl.reads_opt).map(|r| r.name.clone()).collect();
            names.sort();
            names.dedup();
            names
        };
        let Some(read) = decl.reads.iter().chain(&decl.reads_opt).find(|r| r.name == table.name)
        else {
            return Err(AccessError::UndeclaredRead {
                stage: self.rule_as_stage(),
                table: table.name.clone(),
                declared: declared(),
            });
        };
        if read.type_id != table.type_id {
            return Err(AccessError::TypeMismatch {
                stage: self.rule_as_stage(),
                table: table.name.clone(),
                declared: read.short_type_name(),
                requested: table.short_type_name(),
            });
        }
        let Some(entry) = self.registry.table_named(&table.name) else {
            // A table no feature of this registry declares: the rule read it
            // optionally as a peer table that stayed unresolved, and sees it
            // as absent. The plan's stage does not name it either.
            return Ok(None);
        };
        // A value's results are read at that value's addresses only: a read
        // at a document address — a region of the document lowered inside a
        // value, in any round, which reads by the recorded origin — reads the
        // run's store. See decision rationale at [§flm-dd-dr:rend-lower-region].
        let at = self.addr.map(|addr| addr.tree);
        for (id, _, scratch) in self.stores.iter().rev() {
            if Some(*id) != at {
                continue;
            }
            if let Some(found) = scratch.get_erased(entry.id) {
                return Ok(Some(&**found));
            }
        }
        Ok(self.cx.view().get_erased(&table.name)?.map(|found| &**found))
    }

    fn producer_of(&self, table: &CapName) -> Option<StageName> {
        self.cx.view().producer_of(table).cloned()
    }

    fn option_erased(
        &self,
        key: OptionKey,
    ) -> Result<&(dyn std::any::Any + Send + Sync), OptionError> {
        // Outside a rule nothing is declared: refused, like `pres`.
        let Some(idx) = self.current else {
            return Err(OptionError::UndeclaredRead {
                stage: self.rule_as_stage(),
                option: key.short_name().to_string(),
                declared: Vec::new(),
            });
        };
        let decl = &self.registry.rule(idx).rule.decl;
        let declared: Vec<String> =
            decl.options.iter().map(|option| option.key().short_name().to_string()).collect();
        if !decl.options.iter().any(|option| option.key() == &key) {
            return Err(OptionError::UndeclaredRead {
                stage: self.rule_as_stage(),
                option: key.short_name().to_string(),
                declared,
            });
        }
        self.cx.options().get_erased(&key).ok_or_else(|| OptionError::UndeclaredRead {
            stage: self.rule_as_stage(),
            option: key.short_name().to_string(),
            declared,
        })
    }

    fn lower_children(
        &mut self,
        cx: &mut LowerCx<'_>,
        node: &NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, LowerError> {
        cx.restage_children(*node, self).map_err(|e| self.convert(e))
    }

    fn lower_body(
        &mut self,
        cx: &mut LowerCx<'_>,
        node: &NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, LowerError> {
        let body = node.body().ok_or_else(|| {
            LowerError::structure(format!("`{}` has no body", element_of(node).name))
        })?;
        self.lower_slice(cx, Some(body))
    }

    fn lower_arg(
        &mut self,
        cx: &mut LowerCx<'_>,
        node: &NodeRef<'_, Flm>,
        index: usize,
    ) -> Result<Vec<BuildId>, LowerError> {
        let content = node.argument_content_nodes(index).ok_or_else(|| {
            LowerError::structure(format!(
                "argument {index} of `{}` is absent",
                element_of(node).name
            ))
        })?;
        self.lower_slice(cx, Some(content))
    }

    fn lower_arg_named(
        &mut self,
        cx: &mut LowerCx<'_>,
        node: &NodeRef<'_, Flm>,
        name: &str,
    ) -> Result<Vec<BuildId>, LowerError> {
        let content = match node.argument_content_nodes_named(name) {
            Ok(Some(content)) => content,
            Ok(None) => {
                return Err(LowerError::structure(format!(
                    "argument `{name}` of `{}` is absent",
                    element_of(node).name
                )));
            }
            Err(error) => return Err(LowerError::structure(error.to_string())),
        };
        self.lower_slice(cx, Some(content))
    }

    fn lower_nodes(
        &mut self,
        cx: &mut LowerCx<'_>,
        nodes: NodeSlice<'_, Flm>,
    ) -> Result<Vec<BuildId>, LowerError> {
        self.lower_slice(cx, Some(nodes))
    }

    fn default(
        &mut self,
        cx: &mut LowerCx<'_>,
        node: &NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, LowerError> {
        self.copy(cx, *node)
    }

    fn pres(
        &mut self,
        cx: &mut LowerCx<'_>,
        element: &ElementName,
        payload: Arc<dyn PresPayload>,
        body: Vec<BuildId>,
    ) -> Result<BuildId, LowerError> {
        self.stage_pres(cx, element, payload, vec![(String::from(pres::BODY_SLOT), body)])
    }

    fn pres_slots(
        &mut self,
        cx: &mut LowerCx<'_>,
        element: &ElementName,
        payload: Arc<dyn PresPayload>,
        slots: Vec<(String, Vec<BuildId>)>,
    ) -> Result<BuildId, LowerError> {
        self.stage_pres(cx, element, payload, slots)
    }

    fn text(&mut self, cx: &mut LowerCx<'_>, text: &str) -> Result<BuildId, LowerError> {
        let text_element = ElementName::from(crate::lang::structural::TEXT);
        if let Some(idx) = self.current {
            let rule = &self.registry.rule(idx).rule;
            if !self.native.contains(&text_element) && !rule.decl.produces.contains(&text_element) {
                return Err(LowerError::UndeclaredProduces {
                    rule: rule.name.clone(),
                    element: text_element,
                    declared: rule.decl.produces.clone(),
                });
            }
        }
        let source = self.synth_source()?;
        let (_, state) = self.trigger()?;
        let state = Arc::clone(state);
        let kind = NodeKind::chars(text);
        let span = SourceSpan::new(&source, 0..0);
        let builder = cx.builder();
        let ext = Flm::make_node_ext(&kind, &span, &state, builder.staged_children(&[]))
            .map_err(|e| LowerError::Build { detail: e.to_string() })?;
        let id = builder
            .add(kind, span, state, Vec::new(), ext, ())
            .map_err(|e| LowerError::Build { detail: e.to_string() })?;
        Ok(self.record(id))
    }

    fn splice(
        &mut self,
        cx: &mut LowerCx<'_>,
        value: &Arc<NodeTree<Flm>>,
    ) -> Result<Vec<BuildId>, LowerError> {
        // Every diagnostic raised while the value is brought up to date and
        // lowered names the node it is shown at, the one being lowered.
        let (shown_at, _) = self.trigger()?;
        self.shown_at.push(shown_at.clone());
        let result = self.splice_shown(cx, value);
        self.shown_at.pop();
        result
    }

    fn lower_region(
        &mut self,
        cx: &mut LowerCx<'_>,
        at: NodeAddr,
        region: NodeRegion,
        shape: &mut RegionShape<'_>,
    ) -> Result<Vec<BuildId>, LowerError> {
        let not_found = |detail: String| LowerError::RegionNotFound { detail };
        let tree = self
            .reachable_tree(at.tree)
            .ok_or_else(|| not_found(format!("no tree {} is reachable", at.tree)))?;
        let node = tree
            .get(at.node)
            .ok_or_else(|| not_found(format!("the tree {} holds no node {:?}", at.tree, at.node)))?;
        let content = region.content_nodes(&node).ok_or_else(|| {
            not_found(format!("the node {at:?} (`{}`) has no {region}", element_of(&node).name))
        })?;
        // The pointed node is the one whose content is being lowered: until a
        // visit sets its own, the address and the origin are the node's.
        self.entered.push((at.tree, Arc::clone(&tree)));
        let saved_mode = std::mem::replace(&mut self.mode, Mode::Pointed(at.tree));
        let saved_addr = self.addr.replace(at);
        let saved_origin = self.origin.replace(at);
        let result = shape(cx, self, &node, &content);
        self.mode = saved_mode;
        self.addr = saved_addr;
        self.origin = saved_origin;
        self.entered.pop();
        result
    }

    fn require(&mut self, id: &Name) {
        self.requires.insert(id.clone());
    }

    fn diagnose(&mut self, diagnostic: FlmDiagnostic) {
        self.report(diagnostic);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_driver_rule_name_is_in_the_lowering_namespace() {
        assert!(DRIVER_RULE.starts_with("flm.lowering."));
    }
}
