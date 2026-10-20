//! Stages: the units of work a plan schedules, and the values that describe
//! them.
//!
//! A stage is one unit of work of the pipeline: it declares the capabilities
//! it provides and requires, the tables it reads and writes, its effect
//! (compute, check, or input/output) and its scope (once per tree, or once per
//! document), and the plan decides whether and when it runs. A capability is a
//! registered dotted name for a state of processing that a stage brings about,
//! such as `flm.blocks.structured`
//! ([`CapabilityDef`](crate::core::registry::CapabilityDef)); a caller demands
//! the capabilities it wants (asks the plan for stages that provide them,
//! [`PlanRequest`](crate::core::plan::PlanRequest)), and the plan works out
//! which stages have to run
//! ([stages, capabilities, and
//! plans](crate::guide::concepts_overview#stages-capabilities-and-plans)).
//!
//! This module defines what a stage is to the rest of the library:
//!
//! - the two traits a stage author implements, [`Stage`] and
//!   [`RewriteStage`];
//! - the values that describe a stage: [`StageDecl`] and [`RewriteDecl`] (what
//!   a stage declares when it is registered), [`Needs`] (what it needs for one
//!   plan) and [`Manifest`] (the plan's record joining the two);
//! - the two properties a [`StageDecl`] states, [`Effect`] and [`Scope`];
//! - [`RewriteError`], the failure of a rewrite stage over one tree, and
//!   [`TreesTable`], the table a rewrite stage publishes its output in.
//!
//! A feature registers a stage with
//! [`FeatureBuilder::stage`](crate::core::registry::FeatureBuilder::stage) and
//! a rewrite stage with
//! [`FeatureBuilder::rewrite_stage`](crate::core::registry::FeatureBuilder::rewrite_stage).
//! Plan construction ([`crate::core::plan`]) reads the describing values to
//! decide which stages run and in which order; the runner
//! ([`crate::core::run`]) calls the traits.
//!
//! # The two kinds of stage
//!
//! A [`RewriteStage`] runs before the document's trees are frozen and turns
//! one tree into another: expanding user definitions, or laying content out in
//! paragraphs and blocks. It runs once per tree. The runner applies the
//! rewrite stages the plan schedules
//! ([`Plan::rewrites`](crate::core::plan::Plan::rewrites)) in the order the plan
//! fixes, and passes each one the tree the previous one returned. A rewrite
//! stage reads no computed data: the signature of
//! [`rewrite`](RewriteStage::rewrite) gives it no access to the store. It publishes its output as a [`TreesTable`] named after the
//! capability it provides, so that a later stage can read a tree in the state
//! that rewrite left it.
//!
//! A [`Stage`] runs after the trees are frozen. It reads the frozen trees and
//! the tables it declared, and writes tables of its own. Its [`Effect`] says
//! what kind of work it does (computed data, diagnostics only, or effects
//! outside the store), and its [`Scope`] says how often it runs: once per tree,
//! or once for the whole document.
//!
//! # Declaration and needs
//!
//! A stage describes itself in two parts, because the two are fixed at
//! different moments.
//!
//! - The **declaration** ([`StageDecl`], or [`RewriteDecl`] for a rewrite
//!   stage) is fixed when a feature registers the stage: its name, its effect,
//!   its scope, the capabilities it provides and the tables it writes. The
//!   choice of the stage that provides a capability, and the refusal of a
//!   table with two writers, rely on these values being the same in every
//!   plan.
//! - The **needs** ([`Needs`]) are computed once per plan by [`Stage::needs`]
//!   from the plan request: the capabilities the stage requires, the tables
//!   and option types it reads, and the edges that only order it. Most stages
//!   return a constant. A stage whose inputs depend on the plan request
//!   computes them here; a backend's lowering stage, for example, reads the
//!   tables that the lowering rules selected for that backend read.
//!
//! The plan joins both parts into the stage's [`Manifest`] and keeps it. From
//! then on the runner and every check read the plan's manifest and never call
//! the trait methods again, so that one plan never acts on two different
//! results of one method.
//!
//! Declaring a table written also provides that table's capability, and
//! declaring a table read also requires that table's capability. The registry
//! and the plan derive both, so a stage author lists reads and writes and
//! nothing else for them.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use techy::core::node::NodeTree;
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::serialize::SerialValue;

use crate::model::addr::TreeId;
use crate::model::names::{CapName, Name, StageName};
use crate::model::options::{OptionRef, Options};
use crate::core::plan::PlanCx;
use crate::core::run::{RewriteCx, StageCx};
use crate::core::data::store::StageOut;
use crate::core::data::table::{ProjectCx, Table, TableKey, TableRef};
use crate::lang::{Flm, FlmDiagnostic, FlmDiagnostics, FlmSpan};

/// The kind of work a stage does: computed data, diagnostics only, or effects
/// outside the store. A plan orders stages and a run stops before some of
/// them according to their effect.
///
/// Every [`Stage`] declares one effect in its [`StageDecl`]. A
/// [`RewriteStage`] declares none: it runs before the trees are frozen and
/// always produces a tree (the registry records it as a `Compute` stage).
///
/// See also [`Scope`], which says how often a stage runs.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Effect {
    /// Produces computed data, and diagnostics along with it.
    Compute,
    /// Produces diagnostics only. The registry build refuses a `Check` stage
    /// that declares a table written.
    Check,
    /// Has effects outside the store, such as handing an output's files to
    /// the program that writes them.
    ///
    /// The plan orders every `Io` stage after every `Compute` and `Check`
    /// stage, and only another `Io` stage may read what an `Io` stage writes.
    /// A run that has recorded an Error diagnostic stops before the first
    /// batch holding an `Io` stage (the run's *error gate*), so a document with
    /// errors produces no effects outside the store unless the caller asks for
    /// them with
    /// [`RunOptions::continue_with_errors`](crate::core::run::RunOptions::continue_with_errors).
    ///
    /// A `Compute` stage that calls a service which reaches the network, a
    /// cache file or another program is still a `Compute` stage: what matters
    /// is that later `Compute` stages read what it writes.
    Io,
}

impl Effect {
    /// The effect's name as it appears in messages: `Compute`, `Check` or
    /// `Io`.
    pub fn label(self) -> &'static str {
        match self {
            Effect::Compute => "Compute",
            Effect::Check => "Check",
            Effect::Io => "Io",
        }
    }
}

impl fmt::Display for Effect {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// How often a stage runs over one document: once per tree, or once for the
/// whole document.
///
/// A document is made of one or more trees, and a stage reads either one tree
/// at a time or the whole document at once. Every [`Stage`] declares one scope
/// in its [`StageDecl`]; a [`RewriteStage`] always runs once per tree.
///
/// See also [`Effect`], which says what kind of work a stage does.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Scope {
    /// Once per tree of the document. Each invocation writes a table piece
    /// (the part of a table one stage invocation produced,
    /// [`PieceClass`](crate::core::data::table::PieceClass)), and the runner
    /// combines the table pieces in the run's order of the trees
    /// ([`Run::tree_order`](crate::core::run::Run::tree_order)), so the
    /// result does not depend on the order in which the invocations
    /// finished. Value catch-up runs a per-tree stage again over a [synthesized
    /// value](crate::guide::concepts_overview#synthesized-values) ([value
    /// catch-up](crate::guide::concepts_overview#value-catch-up)).
    PerTree,
    /// Once for the whole document, which the stage reads through the walk
    /// over the frozen trees ([`StageCx::walk`](crate::core::run::StageCx::walk)).
    Document,
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Scope::PerTree => "PerTree",
            Scope::Document => "Document",
        })
    }
}

/// What a stage declares about itself when a feature registers it: its name,
/// its effect, its scope, the capabilities it provides and the tables it
/// writes.
///
/// [`Stage::decl`] returns it, and the registry reads it once, when the
/// feature registers the stage. It is the part of a stage's description that
/// is the same in every plan (see the [module documentation](self)); the other
/// part is [`Needs`], and the plan joins the two into a [`Manifest`].
///
/// Build one with [`StageDecl::new`] and the methods that take and return it.
/// A table listed in [`writes`](StageDecl::writes) must also be declared by a
/// feature ([`FeatureBuilder::table`](crate::core::registry::FeatureBuilder::table));
/// the registry build reports a write of a table nobody declares.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
///
/// # Examples
///
/// ```
/// use flm_core::core::stage::{Effect, Scope, StageDecl};
/// use flm_core::core::data::table::{Table, TableName};
///
/// // The value the stage writes; a table type combines the table pieces
/// // that several invocations of a stage write.
/// #[derive(Debug, Default)]
/// struct WordCount(usize);
///
/// impl Table for WordCount {
///     fn absorb(&mut self, piece: WordCount) {
///         self.0 += piece.0;
///     }
/// }
///
/// const WORDS: TableName<WordCount> = TableName::new("example.words");
///
/// let decl = StageDecl::new("example.words.count", Effect::Compute, Scope::PerTree)
///     .writes(WORDS)
///     .docs("counts the words of each tree");
///
/// assert_eq!(decl.name.as_str(), "example.words.count");
/// assert_eq!(decl.writes.len(), 1);
/// // Writing a table provides its capability, so nothing else is declared.
/// assert!(decl.provides.is_empty());
/// ```
#[derive(Debug, Clone)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct StageDecl {
    /// The stage's name: `flm.<feature>.<verb>` for a stage of the standard
    /// features, such as `flm.refs.resolve`. No two stages of a registry have
    /// the same name.
    pub name: StageName,
    /// What kind of work the stage does.
    pub effect: Effect,
    /// How often the stage runs.
    pub scope: Scope,
    /// The capabilities the stage provides besides the capabilities of the
    /// tables it writes: a capability that no table represents, such as one
    /// stating that a check has run.
    pub provides: Vec<CapName>,
    /// The tables the stage writes. Writing a table also provides that
    /// table's capability, and no two scheduled stages may write one table.
    pub writes: Vec<TableRef>,
    /// A one-line description of the stage for a person reading the registry.
    pub docs: String,
}

impl StageDecl {
    /// Returns the declaration of the stage `name` with the given effect and
    /// scope, providing no capability and writing no table.
    pub fn new(name: impl Into<StageName>, effect: Effect, scope: Scope) -> StageDecl {
        StageDecl {
            name: name.into(),
            effect,
            scope,
            provides: Vec::new(),
            writes: Vec::new(),
            docs: String::new(),
        }
    }

    /// Adds a capability the stage provides, and returns the declaration.
    pub fn provides(mut self, capability: impl Into<CapName>) -> StageDecl {
        self.provides.push(capability.into());
        self
    }

    /// Adds a table the stage writes, which also provides that table's
    /// capability, and returns the declaration.
    pub fn writes(mut self, table: impl TableKey) -> StageDecl {
        self.writes.push(table.table_ref());
        self
    }

    /// Sets the stage's one-line description, and returns the declaration.
    pub fn docs(mut self, docs: impl Into<String>) -> StageDecl {
        self.docs = crate::kept_docs(docs);
        self
    }
}

/// What a stage needs for one plan: the capabilities it requires, the tables
/// and option types it reads, and the edges that only order it.
///
/// [`Stage::needs`] and [`RewriteStage::needs`] return it, once per plan,
/// given the plan request through a [`PlanCx`]. It is the part of a stage's
/// description that may depend on the plan request; the other part is
/// [`StageDecl`], and the plan joins the two into a [`Manifest`].
///
/// Requiring a capability makes the plan schedule a stage that provides it.
/// An ordering-only edge does not: it only constrains the order of two stages
/// that are scheduled anyway.
///
/// Build one with [`Needs::new`] and the methods that take and return it.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
///
/// # Examples
///
/// ```
/// use flm_core::core::stage::Needs;
///
/// let needs = Needs::new()
///     .requires("flm.blocks.structured")
///     .after("flm.labels");
///
/// assert_eq!(needs.requires.len(), 1);
/// assert!(!needs.is_empty());
/// assert!(Needs::new().is_empty());
/// ```
#[derive(Debug, Clone, Default)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct Needs {
    /// The capabilities the stage requires. The plan schedules a stage that
    /// provides each of them, and orders that stage before this one.
    pub requires: Vec<CapName>,
    /// The tables the stage reads. Reading a table also requires that table's
    /// capability, so the plan schedules a stage that writes it.
    pub reads: Vec<TableRef>,
    /// The tables the stage reads if the plan provides them. An optional read
    /// requires nothing, so it never causes a stage to be scheduled; when no
    /// scheduled stage writes the table, the read returns `None`.
    pub reads_opt: Vec<TableRef>,
    /// The option types the stage reads. A read of an option type the stage
    /// did not declare here returns an error where the read is made.
    pub options: Vec<OptionRef>,
    /// Ordering only: the stage runs after the stage that provides each of
    /// these capabilities, if that stage is scheduled.
    pub after: Vec<CapName>,
    /// Ordering only: the stage runs before the stage that provides each of
    /// these capabilities, if that stage is scheduled.
    pub before: Vec<CapName>,
}

impl Needs {
    /// Returns needs that hold nothing: no requirement, no read, no option
    /// type and no ordering edge.
    pub fn new() -> Needs {
        Needs::default()
    }

    /// Adds a required capability, and returns the needs.
    pub fn requires(mut self, capability: impl Into<CapName>) -> Needs {
        self.requires.push(capability.into());
        self
    }

    /// Adds a table the stage reads, which also requires that table's
    /// capability, and returns the needs.
    pub fn reads(mut self, table: impl TableKey) -> Needs {
        self.reads.push(table.table_ref());
        self
    }

    /// Adds a table the stage reads if the plan provides it, requiring
    /// nothing (see [`reads_opt`](Needs::reads_opt)), and returns the needs.
    pub fn reads_opt(mut self, table: impl TableKey) -> Needs {
        self.reads_opt.push(table.table_ref());
        self
    }

    /// Adds the option type `T` to the option types the stage reads, and
    /// returns the needs.
    pub fn option<T: Options>(mut self) -> Needs {
        self.options.push(OptionRef::of::<T>());
        self
    }

    /// Adds the option type declared from data under the name `name` to the
    /// option types the stage reads, and returns the needs. The stage reads it
    /// with [`StageCx::option_named`](crate::core::run::StageCx::option_named).
    pub fn option_named(mut self, name: impl Into<Name>) -> Needs {
        self.options.push(OptionRef::named(name));
        self
    }

    /// Adds an ordering-only edge that places the stage after the stage
    /// providing `capability`, and returns the needs.
    pub fn after(mut self, capability: impl Into<CapName>) -> Needs {
        self.after.push(capability.into());
        self
    }

    /// Adds an ordering-only edge that places the stage before the stage
    /// providing `capability`, and returns the needs.
    pub fn before(mut self, capability: impl Into<CapName>) -> Needs {
        self.before.push(capability.into());
        self
    }

    /// Returns whether the needs hold nothing: no requirement, no read, no
    /// option type and no ordering edge.
    pub fn is_empty(&self) -> bool {
        self.requires.is_empty()
            && self.reads.is_empty()
            && self.reads_opt.is_empty()
            && self.options.is_empty()
            && self.after.is_empty()
            && self.before.is_empty()
    }
}

/// The stage manifest: everything one plan records about one stage, its
/// [`StageDecl`] together with the [`Needs`] it returned for that plan.
///
/// The plan builds a manifest for every registered stage and keeps it. The
/// runner and every check of the plan read the manifest and never call the
/// stage's trait methods, so a stage cannot give one answer to the plan and
/// another to the runner.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct Manifest {
    /// What the stage declared when it was registered.
    pub decl: StageDecl,
    /// What the stage returned as its needs for this plan.
    pub needs: Needs,
}

impl Manifest {
    /// Returns the stage's name.
    pub fn name(&self) -> &StageName {
        &self.decl.name
    }

    /// Returns the stage's effect: the kind of work it does.
    pub fn effect(&self) -> Effect {
        self.decl.effect
    }

    /// Returns the stage's scope: how often it runs.
    pub fn scope(&self) -> Scope {
        self.decl.scope
    }
}

/// A stage that runs after the document's trees are frozen: a `Compute`,
/// `Check` or `Io` stage ([`Effect`]).
///
/// A stage is one unit of work of the pipeline: it declares the capabilities
/// it provides and requires, the tables it reads and writes, its effect
/// (compute, check, or input/output) and its scope (once per tree, or once per
/// document), and the plan decides whether and when it runs. A feature
/// registers an implementation of this trait with
/// [`FeatureBuilder::stage`](crate::core::registry::FeatureBuilder::stage).
///
/// The implementation is registered once and may run many times over many
/// documents, so it holds only its own configuration and does not modify its
/// own state when it runs. Its result must depend only on the tables and option
/// types it declared, the content of the trees and the registry: the runner
/// may run the invocations of a per-tree stage in parallel, and the result
/// must not depend on that.
///
/// See also [`RewriteStage`], the other kind of stage, which runs before the
/// trees are frozen.
pub trait Stage: Send + Sync {
    /// Returns what the stage declares about itself. The registry calls it
    /// once, when the feature registers the stage, and keeps the result for
    /// the life of the registry.
    fn decl(&self) -> StageDecl;

    /// Returns what the stage needs for one plan, given the plan request
    /// through `cx`. Plan construction calls it once per plan.
    ///
    /// Most stages return a constant. A stage whose inputs depend on the plan
    /// request computes them here from what [`PlanCx`] provides: the demanded
    /// capabilities, the stage preferences, the lowering rules selected per
    /// backend and the option values set on the render request. Nothing about
    /// any document is available here, so one plan serves every document.
    fn needs(&self, cx: &PlanCx<'_>) -> Needs;

    /// Runs the stage once: reads through `cx`, and writes its table pieces
    /// and its diagnostics into `out`.
    ///
    /// The runner calls this once per tree for a [`Scope::PerTree`] stage and
    /// once for a [`Scope::Document`] stage. There is no error return: a
    /// problem with the document is reported as a diagnostic through `out`.
    /// A panic is a bug in the stage, not a way to report a problem with the
    /// document, and it propagates out of the run
    /// ([`Run::step`](crate::core::run::Run::step)).
    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>);
}

/// What a rewrite stage declares about itself when a feature registers it: its
/// name, the capability it publishes, and the capability that one refines.
///
/// [`RewriteStage::decl`] returns it. The capability in [`publishes`] has
/// three uses: a caller demands it to have the rewrite stage scheduled; it is
/// the name of the [`TreesTable`] in which the rewrite stage publishes its
/// output; and the registry declares that table and its capability from this
/// declaration alone
/// ([`FeatureBuilder::rewrite_stage`](crate::core::registry::FeatureBuilder::rewrite_stage)).
///
/// A capability may **refine** a parent capability: providing the refinement
/// also provides the parent. `flm.blocks.structured` refines `flm.blocks`, so a
/// caller that demands `flm.blocks` is satisfied by the rewrite stage that
/// structures blocks. The refinement is stated here because the table of a
/// rewrite stage is declared from this declaration, and a table declaration
/// says nothing about refinement. When several refinements of one parent are
/// candidates and nothing else decides between them, the plan chooses the one
/// marked [`default`](RewriteDecl::default).
///
/// See also [`StageDecl`], the declaration of a stage that runs after the
/// trees are frozen.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
///
/// [`publishes`]: RewriteDecl::publishes
#[derive(Debug, Clone)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct RewriteDecl {
    /// The rewrite stage's name: `flm.<feature>.<verb>` for a rewrite stage of
    /// the standard features.
    pub name: StageName,
    /// The capability the rewrite stage provides, which is also the name of
    /// the [`TreesTable`] in which its output is published.
    pub publishes: CapName,
    /// The parent capability that [`publishes`](RewriteDecl::publishes)
    /// refines, if any.
    pub refines: Option<CapName>,
    /// Whether [`publishes`](RewriteDecl::publishes) is the refinement the
    /// plan chooses among its parent's refinements when nothing else decides.
    pub default: bool,
    /// A one-line description of the rewrite stage, which is also the
    /// description of the trees table it publishes.
    pub docs: String,
}

impl RewriteDecl {
    /// Returns the declaration of the rewrite stage `name` publishing the
    /// capability `publishes`, refining no capability and not marked as the
    /// default.
    pub fn new(name: impl Into<StageName>, publishes: impl Into<CapName>) -> RewriteDecl {
        RewriteDecl {
            name: name.into(),
            publishes: publishes.into(),
            refines: None,
            default: false,
            docs: String::new(),
        }
    }

    /// Marks the published capability as a refinement of `parent`, so that
    /// providing it also provides `parent`, and returns the declaration.
    pub fn refines(mut self, parent: impl Into<CapName>) -> RewriteDecl {
        self.refines = Some(parent.into());
        self
    }

    /// Marks the published capability as the refinement the plan chooses among
    /// its parent's refinements when nothing else decides, and returns the
    /// declaration.
    pub fn default(mut self) -> RewriteDecl {
        self.default = true;
        self
    }

    /// Sets the rewrite stage's one-line description, and returns the
    /// declaration.
    pub fn docs(mut self, docs: impl Into<String>) -> RewriteDecl {
        self.docs = crate::kept_docs(docs);
        self
    }
}

/// The failure of a rewrite stage over one tree, carrying the one Error
/// diagnostic the run records for that tree.
///
/// [`RewriteStage::rewrite`] returns a tree or this error. A failure removes
/// the whole tree: the tree is absent from every later table and from the
/// frozen trees. The other trees of the document are unaffected.
///
/// The nesting variant is separate from the others because a failure of that
/// kind over a synthesized value is reported again under a condition of its
/// own; see [`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value).
#[derive(Debug, Clone)]
pub enum RewriteError {
    /// The tree nests more deeply than the recursion guard allows
    /// (`flm.blocks.nesting-too-deep`; see [the recursion guard and worker
    /// threads](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads)).
    NestingTooDeep(FlmDiagnostic),
    /// Any other failure, with the diagnostic that describes it.
    Failed(FlmDiagnostic),
}

impl RewriteError {
    /// Returns a [`Failed`](RewriteError::Failed) error carrying an Error
    /// diagnostic that reports `condition` at `span`.
    pub fn failed(condition: impl DiagnosticInfo, span: FlmSpan) -> RewriteError {
        RewriteError::Failed(Diagnostic::error(condition, span))
    }

    /// Returns the Error diagnostic the run records for the tree that is now
    /// absent.
    pub fn diagnostic(&self) -> &FlmDiagnostic {
        match self {
            RewriteError::NestingTooDeep(diagnostic) | RewriteError::Failed(diagnostic) => {
                diagnostic
            }
        }
    }
}

impl fmt::Display for RewriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.diagnostic().message())
    }
}

impl std::error::Error for RewriteError {}

/// A stage that runs before the document's trees are frozen and turns one
/// tree into another.
///
/// A rewrite stage runs before the document's trees are frozen, turns one tree
/// into another, reads no computed data, and publishes the trees it produced in
/// a table named after the capability it provides. A feature registers an
/// implementation of this trait with
/// [`FeatureBuilder::rewrite_stage`](crate::core::registry::FeatureBuilder::rewrite_stage);
/// the passes the standard features wrap this way are plain functions in
/// [`crate::lang::rewrites`].
///
/// The runner applies the rewrite stages the plan schedules to each tree, in
/// the order the plan fixes, and passes each one the tree the previous one
/// returned. A rewrite stage has no access to computed data: no argument of
/// [`rewrite`](RewriteStage::rewrite) reaches the store. The types therefore
/// guarantee that trees change only before they are frozen.
///
/// See also [`Stage`], the other kind of stage, which runs after the trees are
/// frozen.
pub trait RewriteStage: Send + Sync {
    /// Returns what the rewrite stage declares about itself. The registry
    /// calls it once, when the feature registers the stage, and keeps the
    /// result for the life of the registry.
    fn decl(&self) -> RewriteDecl;

    /// Returns what the rewrite stage needs for one plan, given the plan
    /// request through `cx`.
    ///
    /// Only part of [`Needs`] applies to a rewrite stage: the required
    /// capabilities, which decide whether the plan schedules the rewrite
    /// stage, the ordering-only edges, and the option types it reads. A table
    /// read, plain or optional, is refused when the plan is built
    /// ([`PlanError::RewriteReads`](crate::core::plan::PlanError::RewriteReads)),
    /// since a rewrite stage reads no computed data. Requiring the capability
    /// of a table that a [`Stage`] writes is refused as well
    /// ([`PlanError::RewriteNeedsComputed`](crate::core::plan::PlanError::RewriteNeedsComputed)),
    /// since that table is computed after every rewrite stage has run.
    fn needs(&self, cx: &PlanCx<'_>) -> Needs;

    /// Rewrites the tree `tree`, whose identifier in the document is
    /// `tree_id`, and returns the tree that replaces it.
    ///
    /// A problem that still leaves the rewrite stage with a tree to return is
    /// reported as a diagnostic in `diags`.
    ///
    /// # Errors
    ///
    /// Returns [`RewriteError`] when the rewrite stage cannot produce a tree.
    /// The tree is then absent from every later table and from the frozen
    /// trees, and the run records the error's diagnostic for it.
    fn rewrite(
        &self,
        tree_id: TreeId,
        tree: &Arc<NodeTree<Flm>>,
        cx: &RewriteCx<'_>,
        diags: &mut FlmDiagnostics,
    ) -> Result<Arc<NodeTree<Flm>>, RewriteError>;
}

/// The table a rewrite stage publishes: each tree as that rewrite stage
/// returned it.
///
/// Every rewrite stage the plan schedules writes one of these under the
/// capability it publishes ([`RewriteDecl::publishes`]), so that a later stage
/// can read a tree in a particular state, for example with user definitions
/// expanded, rather than the frozen tree. A tree on which an earlier rewrite stage
/// failed is missing from this table too.
///
/// The serialized form of this table ([`Table::project`]) lists the tree
/// references only; the trees themselves belong to the document and are
/// reached by their identifier.
#[derive(Debug, Clone, Default)]
pub struct TreesTable {
    /// The output of the rewrite stage for each tree it ran on, by tree
    /// identifier.
    pub trees: BTreeMap<TreeId, Arc<NodeTree<Flm>>>,
}

impl Table for TreesTable {
    fn absorb(&mut self, piece: TreesTable) {
        self.trees.extend(piece.trees);
    }

    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(self.trees.keys().map(|id| cx.tree_ref(*id)).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::data::table::TableName;

    #[derive(Debug, Default, Hash)]
    struct Width(u32);

    const TREES: TableName<TreesTable> = TableName::new("t.trees");

    #[test]
    fn declarations_and_needs_record_what_the_builders_are_given() {
        let decl = StageDecl::new("t.scan", Effect::Compute, Scope::PerTree)
            .provides("t.marker")
            .writes(TREES);
        assert_eq!(decl.provides, [CapName::from("t.marker")]);
        assert_eq!(decl.writes[0].name, "t.trees");
        let needs =
            Needs::new().requires("a").reads(TREES).option::<Width>().after("b").before("c");
        assert_eq!(needs.requires, [CapName::from("a")]);
        assert_eq!(needs.reads[0].name, "t.trees");
        assert_eq!(needs.options.len(), 1);
        assert!(!needs.is_empty());
        assert!(Needs::new().is_empty());
    }

    #[test]
    fn a_rewrite_declaration_carries_its_refinement() {
        let decl = RewriteDecl::new("t.structure", "t.structured").refines("t.blocks").default();
        assert_eq!(decl.refines.as_deref().map(|n| n.as_str()), Some("t.blocks"));
        assert!(decl.default);
    }
}
