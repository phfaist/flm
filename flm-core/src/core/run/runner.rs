//! The [`Run`], one execution of one plan over one document: how it is
//! constructed, how it executes, and the diagnostics it reports.
//!
//! A run is one execution of one plan over one document; it owns the frozen
//! trees, the store and the diagnostics ([`Run`]). Constructing a run checks its
//! inputs; executing it runs the plan's stages batch by batch. See also
//! [running a plan](crate::guide::pipeline#running-a-plan) and, for an
//! embedding program, [running](crate::guide::embedding#running).
//!
//! # Constructing a run
//!
//! [`Run::new`] takes the document, the plan and the [`Externals`] that the
//! caller installs. It first checks the option values set on the document
//! against the registry's declarations. It then checks each installed value:
//! the registry must declare the table, the table must be declared as external
//! ([`FeatureBuilder::external_table`](crate::core::registry::FeatureBuilder::external_table)),
//! the value must be of the table's declared type, and a value that
//! carries a [`Stamp`] must have been produced under the same registry, rewrite
//! stages and option values as this run. Finally, every external table that the
//! plan needs must have a value. Each failure is a [`RunError`]. `Run::new` also
//! resolves the option values of every scheduled stage once, for the whole
//! run. Nothing is executed.
//!
//! # Executing a run
//!
//! Execution runs the rewrite stages first, then the plan's batches in order. A
//! batch is a group of stages of which none depends on another stage of the
//! same batch, so they may run at the same time
//! ([`Plan::batches`](crate::core::plan::Plan::batches)).
//!
//! 1. **The rewrite stages.** Each tree of the document goes through every
//!    rewrite stage of the plan, in the plan's order, each one's output feeding
//!    the next; the trees are rewritten independently of each other. Each
//!    rewrite stage's output for every tree is published as that rewrite
//!    stage's own table, and the last output is the tree's frozen tree. A
//!    rewrite stage that fails on a tree records its one Error diagnostic, and
//!    that tree has no frozen tree: no later stage reads it, the tables of the
//!    rewrite stages that ran before keep their version of it, and the
//!    document's other trees are unaffected.
//!
//!    When the rewrite stages are over, the run computes its document order
//!    ([`crate::core::run::order`]): the document's walk applied to the frozen
//!    trees, a placed tree entered at the node that places it. Every later
//!    stage reads this order. The run's trees are the trees that this walk
//!    reaches: a tree whose placing node a rewrite stage removed, or whose
//!    including tree failed its rewrite, is read by no later stage and appears
//!    in no output, its content dropped with the node that placed it.
//! 2. **Each batch of stages.** A per-tree stage (a stage of
//!    [`Scope::PerTree`]) runs once for each tree of the run; a document-scope
//!    stage (a stage of [`Scope::Document`]) runs once for the whole document. Every stage of a batch reads the
//!    store as it was when the batch began, so the stages of one batch, and the
//!    calls of one per-tree stage, may run at the same time. When the batch is
//!    over, the runner folds the table pieces into the store: each stage's
//!    table pieces in the run's order of the trees (the order in which the walk
//!    enters them), the stages in the plan's order, and each stage's tables in
//!    the registry's table order. Nothing of this depends on the order in which
//!    the work finished, so a parallel and a sequential run produce the same
//!    bytes.
//! 3. **The error gate.** Before the first batch that holds a stage with
//!    effects outside the store, the error gate stops a run that has recorded
//!    an Error diagnostic, from parsing or from the stages, unless
//!    [`RunOptions::continue_with_errors`] is set. A document with errors
//!    therefore produces no effects outside the store unless the caller asks
//!    for them; see [`RunOptions`] and [the error
//!    gate](crate::guide::embedding#the-error-gate).
//!
//! [`Run::execute`] runs to the end; [`Run::step`] runs one step, the rewrite
//! stages first and then one batch per call. Stepping changes when the work
//! happens, never what is computed: the batches are fixed and the folds do not
//! depend on timing.
//!
//! # Worker threads
//!
//! The rewrite stages and every batch run on a worker thread. A worker thread is
//! a thread created with at least
//! [`WORKER_STACK_SIZE`](crate::lang::guard::WORKER_STACK_SIZE) bytes of stack;
//! every guarded run (a parse, a rewrite, an emit) must execute on one
//! ([`on_worker_thread`](crate::lang::guard::on_worker_thread)). Under the Cargo
//! feature `parallel` of `flm-core`, unless [`RunOptions::sequential`] is set,
//! the work runs on the threads of the runner's own pool, which are worker
//! threads.
//! Otherwise it runs on the calling thread when that thread is marked as a
//! worker thread, and on a thread created for the purpose when it is not.
//!
//! # Diagnostics
//!
//! [`Run::diagnostics`] returns a [`DiagnosticsView`]: each fragment's parse
//! diagnostics first, in the order in which the document's walk reaches the
//! fragments and once per fragment however many times that fragment is placed,
//! then the diagnostics that the run produced. [`Run::pipeline_diagnostics`]
//! returns the second part alone. The run produces its diagnostics in this
//! order: the warnings about the preferences of the plan request that the plan
//! could not follow ([`Plan::warnings`](crate::core::plan::Plan::warnings)), the diagnostics of
//! the rewrite stages tree by tree, the [`PlacedTwice`] diagnostics, and then
//! the diagnostics of the stages, batch by batch.
//!
//! [`PlacedTwice`]: crate::core::run::PlacedTwice

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use techy::core::node::NodeTree;
use techy::error::{Diagnostic, Diagnostics};
use techy::source::{Source, SourceSpan};

use crate::model::addr::TreeId;
use crate::model::doc::Doc;
use crate::model::names::{CapName, StageName};
use crate::model::options::{BindSite, OptionError, ResolvedOptions, validate_values};
use crate::core::plan::{Plan, PlanWarning};
use crate::core::registry::StageKind;
use crate::core::registry::frozen::StageIdx;
use crate::core::run::cx::{
    IgnoredPreference, PreferredRuleNotCandidate, PreferredRuleSetAside, RewriteCx, RunView,
    StageCx,
};
use crate::core::run::externals::{Externals, Stamp};
use crate::core::run::order::RunOrder;
use crate::core::stage::{Scope, TreesTable};
use crate::core::data::store::{
    AccessError, Computed, ComputedView, FoldTarget, StageOut, StoreError, fold_outs,
};
use crate::core::data::table::{TableId, TableKey};
use crate::lang::guard;
use crate::lang::{Flm, FlmDiagnostic, FlmDiagnostics, FlmOrigin, FlmSource};

/// How a run executes: on one thread or on several, whether the error gate may
/// stop it, and how many diagnostics it keeps.
///
/// The error gate stops a run that has recorded an Error diagnostic, from
/// parsing or from the stages, before the first batch that holds a stage with
/// effects outside the store ([`Effect::Io`](crate::core::stage::Effect::Io);
/// [`Plan::first_io_batch`](crate::core::plan::Plan::first_io_batch)), unless
/// [`continue_with_errors`](RunOptions::continue_with_errors) is set. A run
/// that the error gate stopped reports it through
/// [`RunOutcome::halted_at_gate`] and [`Run::halted_at_gate`]. Apart from the
/// error gate, these options do not change what is computed.
///
/// Pass the same options to every [`Run::step`]; [`Run::execute`] passes them
/// to each step it takes.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RunOptions {
    /// Runs all the work on one thread, even when the Cargo feature `parallel`
    /// of `flm-core` is compiled in.
    ///
    /// The result is the same either way. The sequential path is always
    /// compiled, and it is the only one when `flm-core` is built without its
    /// default Cargo features.
    pub sequential: bool,
    /// Runs the stages with effects outside the store even when the run has
    /// recorded Error diagnostics, instead of stopping at the error gate.
    pub continue_with_errors: bool,
    /// The number of diagnostics that the run keeps of those it produces
    /// (the parse diagnostics have their own limit, set when parsing with
    /// [`ParseOptions`](crate::lang::ParseOptions)). `None` keeps the parser
    /// toolkit's default limit
    /// ([`Diagnostics::DEFAULT_LIMIT`](techy::error::Diagnostics::DEFAULT_LIMIT)).
    ///
    /// Diagnostics past the limit are counted but not kept;
    /// [`DiagnosticsView::any_recorded`], [`DiagnosticsView::has_errors`] and
    /// [`DiagnosticsView::error_count`] count them. The run reads this value
    /// from the options of the first step only.
    pub max_diagnostics: Option<usize>,
}

impl RunOptions {
    /// Creates the default options: parallel when `flm-core` is built with the
    /// Cargo feature `parallel`, stopping at the error gate, and the parser
    /// toolkit's default diagnostic limit.
    pub fn new() -> RunOptions {
        RunOptions::default()
    }

    /// Creates the default options, except that all the work runs on one
    /// thread.
    pub fn sequential() -> RunOptions {
        RunOptions { sequential: true, ..RunOptions::default() }
    }
}

/// What one call of [`Run::step`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// A step ran, and more steps remain.
    More,
    /// The run is over: it either finished or stopped at the error gate.
    Done,
}

/// How a run ended, as [`Run::execute`] reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunOutcome {
    /// Whether the error gate stopped the run before the first stage with
    /// effects outside the store, rather than the run finishing.
    pub halted_at_gate: bool,
}

/// The reason why a run could not be constructed, or could not go on.
///
/// Apart from [`DocumentOption`](RunError::DocumentOption), which reports an
/// option value set on the document where it may not be set, none of these
/// errors concerns the document's content: a problem with the content is
/// reported as a diagnostic. A `RunError` means that the run was asked for
/// something it cannot do. [`Run::new`] returns every variant except
/// [`Worker`](RunError::Worker), which [`Run::step`] and [`Run::execute`]
/// return together with [`Store`](RunError::Store).
#[derive(Debug, Clone)]
pub enum RunError {
    /// The document sets a value for an option type that is not bound at
    /// document level ([option types and values](crate::model::options)).
    DocumentOption(OptionError),
    /// The plan needs an external table for which no value is installed.
    MissingExternal {
        /// The name of the table.
        table: CapName,
        /// The table's description
        /// ([`TableDef::docs`](crate::core::data::table::TableDef::docs)),
        /// which says what belongs in it.
        docs: String,
    },
    /// A value is installed for a table that the registry does not declare.
    UnknownExternal {
        /// The table name given with the installed value.
        table: CapName,
        /// The names of every external table that the registry declares.
        roster: Vec<CapName>,
    },
    /// A value is installed for a table that is not declared as external
    /// ([`FeatureBuilder::external_table`](crate::core::registry::FeatureBuilder::external_table));
    /// only an external table can receive an installed value.
    NotExternal {
        /// The name of the table.
        table: CapName,
    },
    /// An installed value is not of the value type that the table is declared
    /// with.
    ExternalType {
        /// The name of the table.
        table: CapName,
        /// The short name of the declared value type.
        declared: &'static str,
        /// The short name of the type of the installed value.
        installed: &'static str,
    },
    /// An installed value was produced under another registry, other rewrite
    /// stages or other option values than this run uses; see [`Stamp`].
    StampMismatch {
        /// The name of the table.
        table: CapName,
        /// The stamp of the installed value.
        installed: Stamp,
        /// The stamp of this run.
        expected: Stamp,
    },
    /// No worker thread could be created to run the work on.
    Worker {
        /// The error message of the failed thread creation.
        detail: String,
    },
    /// An invariant of the store failed, for example a table written twice.
    /// This reports a defect in the runner or in a stage, never a problem with
    /// the document.
    Store(StoreError),
}

impl fmt::Display for RunError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RunError::DocumentOption(error) => fmt::Display::fmt(error, f),
            RunError::MissingExternal { table, docs } => {
                write!(f, "the plan needs the external table `{table}`, but no value is installed for it")?;
                // A registry built without the cargo feature `docs` keeps no
                // description of the table.
                if !docs.is_empty() {
                    write!(f, "; the table holds: {docs}")?;
                }
                Ok(())
            }
            RunError::UnknownExternal { table, roster } => write!(
                f,
                "a value is installed for the table `{table}`, but no feature declares a table \
                 of that name; declared external tables: [{}]",
                roster.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")
            ),
            RunError::NotExternal { table } => {
                write!(
                    f,
                    "table `{table}` is not declared as an external table, so it cannot receive \
                     an installed value"
                )
            }
            RunError::ExternalType { table, declared, installed } => write!(
                f,
                "the value installed for table `{table}` has type `{installed}`, but the table is \
                 declared with value type `{declared}`"
            ),
            RunError::StampMismatch { table, installed, expected } => write!(
                f,
                "the value installed for table `{table}` was produced under another registry, \
                 other rewrite stages or other option values than this run: ({installed}) \
                 instead of ({expected})"
            ),
            RunError::Worker { detail } => {
                write!(f, "no thread could be created to execute the run: {detail}")
            }
            RunError::Store(error) => fmt::Display::fmt(error, f),
        }
    }
}

impl std::error::Error for RunError {}

impl From<StoreError> for RunError {
    fn from(error: StoreError) -> RunError {
        RunError::Store(error)
    }
}

/// What the rewrites did to one tree: the tree after each rewrite, in the
/// plan's order and `None` from the first failure onwards, and everything the
/// rewrites reported.
type RewriteOuts = (Vec<Option<Arc<NodeTree<Flm>>>>, FlmDiagnostics);

/// How far a run has got.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    Rewrites,
    Batch(usize),
    Done { halted: bool },
}

/// A run is one execution of one plan over one document; it owns the frozen
/// trees, the store and the diagnostics.
///
/// Because a run owns everything it
/// computes, one document may be run several times, under different plans,
/// without one run seeing another's results.
///
/// Construct a run with [`Run::new`], then [`Run::execute`] it to the end or
/// [`Run::step`] through it. Read what it computed with [`Run::table`],
/// [`Run::frozen_tree`] and [`Run::diagnostics`]. The [module
/// documentation](self) describes what happens in between; [reading
/// results](crate::guide::embedding#reading-results) shows how an embedding
/// program reads an output.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
/// use flm_core::core::plan::{Overrides, Plan};
/// use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
/// use flm_core::core::run::{Externals, Run, RunOptions};
/// use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
/// use flm_core::stdcore::backends::text::TEXT_DOCUMENT;
/// use flm_core::stdcore::{self, StdcoreConfig};
///
/// let mut builder = RegistryBuilder::new();
/// builder.use_package(stdcore::package(StdcoreConfig::new()));
/// builder.use_package(flm_core::math_passthrough::package());
/// let registry = Arc::new(builder.build(&RegistryInputs::new())?);
///
/// let source = Arc::new(
///     FlmSource::new("Hello \\textbf{world}.").with_origin(FlmOrigin::new("hello.flm")),
/// );
/// let fragment = parse_fragment(
///     registry.lang(), source, FragmentLayout::default(), &ParseOptions::default(),
/// )?;
/// let mut content = DocContent::new();
/// content.place(Arc::new(fragment));
/// let doc = Arc::new(Doc::new(content));
///
/// let plan = Arc::new(Plan::for_workflow(&registry, "text", &Overrides::none())?);
/// let mut run = Run::new(doc, plan, Externals::none())?;
/// let outcome = run.execute(&RunOptions::new())?;
///
/// assert!(!outcome.halted_at_gate);
/// let artifacts = run.table(TEXT_DOCUMENT)?.expect("the text workflow writes it");
/// let bytes = artifacts.entry().and_then(|a| a.as_bytes()).expect("one entry artifact");
/// assert_eq!(String::from_utf8_lossy(bytes).trim_end(), "Hello world.");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct Run {
    doc: Arc<Doc>,
    plan: Arc<Plan>,
    computed: Computed,
    diagnostics: FlmDiagnostics,
    frozen: BTreeMap<TreeId, Arc<NodeTree<Flm>>>,
    /// The run's document order, computed when the rewrites are over; empty
    /// before.
    order: RunOrder,
    tree_index: BTreeMap<usize, TreeId>,
    options: BTreeMap<StageIdx, Arc<ResolvedOptions>>,
    phase: Phase,
    plan_source: Arc<FlmSource>,
    sink_sized: bool,
}

impl Run {
    /// Constructs a run of `plan` over `doc`, with the values of `externals`
    /// installed into their external tables.
    ///
    /// Nothing is executed here: this checks the inputs and resolves the
    /// option values of every scheduled stage, so that every constructed run
    /// has sound inputs. The [module documentation](self) lists the checks.
    /// Execute the run with [`Run::execute`] or [`Run::step`].
    ///
    /// # Errors
    ///
    /// Returns [`RunError::DocumentOption`] when the document sets a value for
    /// an option type that is not bound at document level, and
    /// [`RunError::MissingExternal`], [`RunError::UnknownExternal`],
    /// [`RunError::NotExternal`], [`RunError::ExternalType`] or
    /// [`RunError::StampMismatch`] for a problem with the installs.
    pub fn new(doc: Arc<Doc>, plan: Arc<Plan>, externals: Externals) -> Result<Run, RunError> {
        let registry = plan.registry();
        let option_roster: Vec<_> = registry.options().keys().cloned().collect();
        if let Some(error) = validate_values(
            doc.options(),
            BindSite::Document,
            |key| registry.option_decl(key),
            &option_roster,
            None,
        )
        .into_iter()
        .next()
        {
            return Err(RunError::DocumentOption(error));
        }

        let mut options: BTreeMap<StageIdx, Arc<ResolvedOptions>> = BTreeMap::new();
        for idx in plan.rewrites().iter().chain(plan.order()) {
            let stage_plan = plan.manifest(*idx).expect("a scheduled stage has a manifest");
            let resolved = ResolvedOptions::resolve(
                registry.stage(*idx).name.clone(),
                &stage_plan.options,
                &plan.request().render_options,
                doc.options(),
            );
            options.insert(*idx, Arc::new(resolved));
        }
        let expected = stamp_of(&plan, &options);

        let mut computed = Computed::new();
        let roster: Vec<CapName> = registry.external_tables().map(|t| t.def.name.clone()).collect();
        for (name, install) in externals.iter() {
            let Some(entry) = registry.table_named(name) else {
                return Err(RunError::UnknownExternal {
                    table: name.clone(),
                    roster: roster.clone(),
                });
            };
            if !entry.external {
                return Err(RunError::NotExternal { table: name.clone() });
            }
            if entry.def.ty.type_id() != install.table.type_id {
                return Err(RunError::ExternalType {
                    table: name.clone(),
                    declared: entry.def.ty.short_name(),
                    installed: install.table.short_type_name(),
                });
            }
            if let Some(stamp) = &install.stamp {
                if *stamp != expected {
                    return Err(RunError::StampMismatch {
                        table: name.clone(),
                        installed: stamp.clone(),
                        expected: expected.clone(),
                    });
                }
            }
            computed.install(entry.id, Arc::clone(&install.value))?;
        }
        for id in plan.externals() {
            if !computed.contains(*id) {
                let entry = registry.table(*id);
                return Err(RunError::MissingExternal {
                    table: entry.def.name.clone(),
                    docs: entry.def.docs.clone(),
                });
            }
        }

        Ok(Run {
            doc,
            plan,
            computed,
            diagnostics: FlmDiagnostics::new(),
            frozen: BTreeMap::new(),
            order: RunOrder::default(),
            tree_index: BTreeMap::new(),
            options,
            phase: Phase::Rewrites,
            plan_source: Arc::new(Source::new("").with_origin(FlmOrigin::none())),
            sink_sized: false,
        })
    }

    /// The document being processed.
    pub fn doc(&self) -> &Arc<Doc> {
        &self.doc
    }

    /// The plan being run.
    pub fn plan(&self) -> &Arc<Plan> {
        &self.plan
    }

    /// Returns the store: the values installed into external tables, and the
    /// tables that the steps run so far have produced.
    ///
    /// [`Run::table`] reads one table of it, typed.
    pub fn computed(&self) -> &Computed {
        &self.computed
    }

    /// Reads one table of the store through a typed table key.
    ///
    /// `Ok(None)` means that no value of the table is present yet: no stage of
    /// the plan writes it, or the batch that writes it has not run yet.
    ///
    /// Unlike a stage's read, this read is not restricted to declared reads: a
    /// caller may read any table of the registry.
    ///
    /// # Errors
    ///
    /// Returns [`AccessError::UndeclaredRead`] when the registry declares no
    /// table of that name (the error then gives `_host` as the stage and every
    /// table of the registry as the declared reads), and
    /// [`AccessError::TypeMismatch`] when the value type of `key` is not the
    /// table's declared value type.
    pub fn table<K: TableKey>(&self, key: K) -> Result<Option<&K::Table>, AccessError> {
        let table = key.table_ref();
        let host: StageName = StageName::from(crate::core::registry::HOST_FEATURE);
        let Some(entry) = self.plan.registry().table_named(&table.name) else {
            return Err(AccessError::UndeclaredRead {
                stage: host,
                table: table.name.clone(),
                declared: self
                    .plan
                    .registry()
                    .tables()
                    .iter()
                    .map(|t| t.def.name.clone())
                    .collect(),
            });
        };
        if entry.def.ty.type_id() != table.type_id {
            return Err(AccessError::TypeMismatch {
                stage: host,
                table: table.name.clone(),
                declared: entry.def.ty.short_name(),
                requested: table.short_type_name(),
            });
        }
        Ok(self.computed.get_as::<K::Table>(entry.id))
    }

    /// Returns the frozen tree of one of the document's trees: what the tree
    /// became after the last rewrite stage.
    ///
    /// `None` before the rewrite stages have run, for a tree whose rewrite
    /// failed, for a tree that the run's walk does not reach
    /// ([`Run::tree_order`]), and for an identifier that the document does not
    /// use.
    pub fn frozen_tree(&self, id: TreeId) -> Option<&Arc<NodeTree<Flm>>> {
        self.frozen.get(&id)
    }

    /// Returns the run's trees, each with its frozen tree, in the run's order
    /// of the trees ([`Run::tree_order`]).
    pub fn frozen_trees(&self) -> impl Iterator<Item = (TreeId, &Arc<NodeTree<Flm>>)> + '_ {
        self.order.trees().iter().filter_map(|id| self.frozen.get(id).map(|tree| (*id, tree)))
    }

    /// Returns the run's trees in the run's order of the trees: the trees that
    /// the document's walk, applied to the frozen trees, reaches, in the order
    /// in which it enters them. Empty before the rewrite stages have run.
    ///
    /// A tree that the walk does not reach is not a tree of the run: a rewrite
    /// stage removed its placing node (for example, a user definition whose
    /// template does not use the argument that held it), or the tree that
    /// includes it failed its rewrite. Its content is dropped with the node
    /// that placed it, and no diagnostic reports it.
    ///
    /// A placed tree is entered at the node that places it, so its nodes come
    /// in the middle of the tree that places it: this order says which tree
    /// comes first, not which node. Document order across trees is the order
    /// of the walk ([`StageCx::walk`](crate::core::run::StageCx::walk)).
    pub fn tree_order(&self) -> &[TreeId] {
        self.order.trees()
    }

    /// Returns only the diagnostics that the run produced, without the parse
    /// diagnostics.
    ///
    /// [`Run::diagnostics`] is usually what a caller wants: it returns both.
    pub fn pipeline_diagnostics(&self) -> &FlmDiagnostics {
        &self.diagnostics
    }

    /// Returns every diagnostic of the run: each fragment's parse diagnostics,
    /// in the order in which the document's walk reaches the fragments and
    /// once per fragment, then the diagnostics that the run produced.
    pub fn diagnostics(&self) -> DiagnosticsView<'_> {
        DiagnosticsView { doc: &self.doc, pipeline: &self.diagnostics }
    }

    /// Returns this run's stamp: the registry digest, the rewrite stages it
    /// runs, and a digest of the option values those rewrite stages read.
    ///
    /// To install a value that this run produced into an external table of
    /// another run, pass this stamp to [`Externals::install`]; the other run
    /// checks it. See [`Stamp`].
    pub fn stamp(&self) -> Stamp {
        stamp_of(&self.plan, &self.options)
    }

    /// Whether the run is over, whether because it finished or because the
    /// error gate stopped it.
    pub fn is_done(&self) -> bool {
        matches!(self.phase, Phase::Done { .. })
    }

    /// Whether the error gate stopped the run, rather than the run finishing.
    pub fn halted_at_gate(&self) -> bool {
        matches!(self.phase, Phase::Done { halted: true })
    }

    /// Runs one step: the rewrite stages on the first call, then one batch of
    /// the plan per call, in order.
    ///
    /// Returns [`Step::More`] while more steps remain and [`Step::Done`] once
    /// the run is over; calling it again after that does nothing and returns
    /// [`Step::Done`]. Before the first batch that holds a stage with effects
    /// outside the store, the error gate may end the run (see
    /// [`RunOptions`]). The first call also sets the diagnostic limit from
    /// [`RunOptions::max_diagnostics`]. Stepping changes only when the work
    /// happens, never what is computed.
    ///
    /// # Errors
    ///
    /// Returns [`RunError::Worker`] when no thread could be created to run the
    /// batch on, and [`RunError::Store`] when an invariant of the store failed.
    ///
    /// # Panics
    ///
    /// A stage or a rewrite stage that panics propagates the panic out of this
    /// call. A panic means a defect in that stage; a problem with the document
    /// is reported as a diagnostic. The whole batch is collected before
    /// anything is folded into the store, so when a panic propagates, the store
    /// is untouched and the run has not advanced.
    pub fn step(&mut self, options: &RunOptions) -> Result<Step, RunError> {
        if !self.sink_sized {
            if let Some(limit) = options.max_diagnostics {
                self.diagnostics = Diagnostics::with_limit(limit);
            }
            self.sink_sized = true;
        }
        match self.phase {
            Phase::Done { .. } => Ok(Step::Done),
            Phase::Rewrites => {
                self.on_worker(options, |run| run.run_rewrites(options))?;
                self.phase = if self.plan.batches().is_empty() {
                    Phase::Done { halted: false }
                } else {
                    Phase::Batch(0)
                };
                Ok(if self.is_done() { Step::Done } else { Step::More })
            }
            Phase::Batch(index) => {
                if self.plan.first_io_batch() == Some(index)
                    && !options.continue_with_errors
                    && self.diagnostics().has_errors()
                {
                    self.phase = Phase::Done { halted: true };
                    return Ok(Step::Done);
                }
                self.on_worker(options, |run| run.run_batch(index, options))?;
                self.phase = if index + 1 < self.plan.batches().len() {
                    Phase::Batch(index + 1)
                } else {
                    Phase::Done { halted: false }
                };
                Ok(if self.is_done() { Step::Done } else { Step::More })
            }
        }
    }

    /// Runs every remaining step, and returns how the run ended.
    ///
    /// # Errors
    ///
    /// The same as [`Run::step`], which this calls until the run is over.
    ///
    /// # Panics
    ///
    /// A stage or a rewrite stage that panics propagates the panic out of this
    /// call; see [`Run::step`].
    pub fn execute(&mut self, options: &RunOptions) -> Result<RunOutcome, RunError> {
        while self.step(options)? == Step::More {}
        Ok(RunOutcome { halted_at_gate: self.halted_at_gate() })
    }

    /// Runs `f` on a worker thread: a thread of the runner's pool on the
    /// parallel path; otherwise the current thread when it is marked as a
    /// worker thread, and a thread created for `f` when it is not.
    fn on_worker(
        &mut self,
        options: &RunOptions,
        f: impl FnOnce(&mut Run) -> Result<(), RunError> + Send,
    ) -> Result<(), RunError> {
        #[cfg(feature = "parallel")]
        {
            if !options.sequential {
                let pool = pool()?;
                return pool.install(move || f(self));
            }
        }
        let _ = options;
        match guard::on_worker_thread(move || f(self)) {
            Ok(result) => result,
            Err(error) => Err(RunError::Worker { detail: error.to_string() }),
        }
    }

    /// Runs every rewrite over every tree, publishes each rewrite's table, and
    /// records the frozen tree of each tree that came through. See the module
    /// documentation.
    fn run_rewrites(&mut self, options: &RunOptions) -> Result<(), RunError> {
        let plan = Arc::clone(&self.plan);
        let registry = plan.registry();
        for warning in plan.warnings() {
            let span = SourceSpan::new(&self.plan_source, 0..0);
            let diagnostic = match warning {
                PlanWarning::PreferredRuleSetAside { backend, rule, unresolved, workflow } => {
                    Diagnostic::warning(
                        PreferredRuleSetAside {
                            rule: rule.to_string(),
                            backend: backend.to_string(),
                            unresolved: unresolved.iter().map(ToString::to_string).collect(),
                            workflow: workflow.as_ref().map(ToString::to_string),
                        },
                        span,
                    )
                }
                PlanWarning::PreferredRuleNotCandidate { backend, rule, workflow } => {
                    Diagnostic::warning(
                        PreferredRuleNotCandidate {
                            rule: rule.to_string(),
                            backend: backend.to_string(),
                            workflow: workflow.as_ref().map(ToString::to_string),
                        },
                        span,
                    )
                }
                PlanWarning::IgnoredPreference { stage, excluded_by, over, workflow } => {
                    Diagnostic::warning(
                        IgnoredPreference {
                            stage: stage.to_string(),
                            excluded_by: excluded_by.to_string(),
                            over: over.to_string(),
                            workflow: workflow.as_ref().map(ToString::to_string),
                        },
                        span,
                    )
                }
            };
            self.diagnostics.push(diagnostic);
        }
        let rewrites: Vec<StageIdx> = plan.rewrites().to_vec();
        let doc = Arc::clone(&self.doc);
        let content = doc.content();
        let inputs: Vec<(TreeId, Arc<NodeTree<Flm>>, crate::model::doc::FragmentLayout)> =
            content
            .walk_trees()
            .filter_map(|id| {
                content
                    .fragment(id)
                    .map(|fragment| (id, Arc::clone(fragment.tree()), fragment.layout()))
            })
            .collect();
        let stage_options = &self.options;
        let results: Vec<RewriteOuts> = map_indexed(inputs.len(), options, |i| {
            let (id, tree, layout) = &inputs[i];
            let mut local = FlmDiagnostics::new();
            let mut outs: Vec<Option<Arc<NodeTree<Flm>>>> = Vec::with_capacity(rewrites.len());
            let mut current = Arc::clone(tree);
            for idx in &rewrites {
                let entry = registry.stage(*idx);
                let StageKind::Rewrite(stage) = &entry.kind else { continue };
                let cx = RewriteCx::new(registry, *id, *layout, &stage_options[idx]);
                match stage.rewrite(*id, &current, &cx, &mut local) {
                    Ok(next) => {
                        outs.push(Some(Arc::clone(&next)));
                        current = next;
                    }
                    Err(error) => {
                        local.push(error.diagnostic().clone());
                        while outs.len() < rewrites.len() {
                            outs.push(None);
                        }
                        break;
                    }
                }
            }
            (outs, local)
        });

        let mut tables: Vec<(TableId, TreesTable)> = rewrites
            .iter()
            .map(|idx| {
                let table = registry
                    .stage(*idx)
                    .writes
                    .first()
                    .copied()
                    .expect("a rewrite writes its trees table");
                (table, TreesTable::default())
            })
            .collect();
        for (i, (outs, local)) in results.into_iter().enumerate() {
            let (id, tree, _) = &inputs[i];
            let mut last: Option<Arc<NodeTree<Flm>>> = Some(Arc::clone(tree));
            let mut failed = false;
            for (position, out) in outs.into_iter().enumerate() {
                match out {
                    Some(next) => {
                        tables[position].1.trees.insert(*id, Arc::clone(&next));
                        last = Some(next);
                    }
                    None => {
                        failed = true;
                        break;
                    }
                }
            }
            if !failed {
                if let Some(tree) = last {
                    self.tree_index.insert(Arc::as_ptr(&tree) as usize, *id);
                    self.frozen.insert(*id, tree);
                }
            }
            for diagnostic in local {
                self.diagnostics.push(diagnostic);
            }
        }
        tables.sort_by_key(|(id, _)| *id);
        for (id, table) in tables {
            self.computed.install(id, Arc::new(table))?;
        }
        let (order, diags) = RunOrder::new(content, &self.frozen);
        // A tree the walk does not reach is not a tree of the run.
        self.frozen.retain(|id, _| order.has_tree(*id));
        self.tree_index.retain(|_, id| order.has_tree(*id));
        self.order = order;
        for diagnostic in diags {
            self.diagnostics.push(diagnostic);
        }
        Ok(())
    }

    /// Runs the stages of one batch and folds their pieces into the store. See
    /// the module documentation.
    fn run_batch(&mut self, index: usize, options: &RunOptions) -> Result<(), RunError> {
        let plan = Arc::clone(&self.plan);
        let registry = plan.registry();
        let range = plan.batches()[index].clone();
        let stages: Vec<StageIdx> = plan.order()[range].to_vec();
        let base = self.computed.clone();
        let alive: Vec<TreeId> = self.order.trees().to_vec();
        let view = RunView {
            doc: &self.doc,
            registry,
            plan: &plan,
            frozen: &self.frozen,
            order: &self.order,
            tree_index: &self.tree_index,
            computed: &base,
            options: &self.options,
        };
        let outs: Vec<Vec<StageOut<'_>>> = map_indexed(stages.len(), options, |k| {
            let idx = stages[k];
            let entry = registry.stage(idx);
            let StageKind::Pipeline(stage) = &entry.kind else { return Vec::new() };
            let stage_plan = plan.manifest(idx).expect("a scheduled stage has a manifest");
            match entry.decl.scope {
                Scope::PerTree => map_indexed(alive.len(), options, |j| {
                    let cx = StageCx::new(
                        &view,
                        idx,
                        Some(alive[j]),
                        ComputedView::new(&entry.name, &stage_plan.reads, &base),
                    );
                    let mut out = StageOut::new(&entry.name, &stage_plan.writes);
                    stage.run(&cx, &mut out);
                    out
                }),
                Scope::Document => {
                    let cx = StageCx::new(
                        &view,
                        idx,
                        None,
                        ComputedView::new(&entry.name, &stage_plan.reads, &base),
                    );
                    let mut out = StageOut::new(&entry.name, &stage_plan.writes);
                    stage.run(&cx, &mut out);
                    vec![out]
                }
            }
        });
        // The whole batch is collected before anything is folded in, so a stage
        // that panics leaves the store untouched and the run unadvanced. See the
        // design rationale at [§flm-dd-dr:pipe-panic].
        for (k, outs) in outs.into_iter().enumerate() {
            let idx = stages[k];
            let entry = registry.stage(idx);
            let stage_plan = plan.manifest(idx).expect("a scheduled stage has a manifest");
            fold_outs(
                &entry.name,
                &stage_plan.writes,
                outs,
                FoldTarget::Main,
                &mut self.computed,
                &mut self.diagnostics,
            )?;
        }
        Ok(())
    }
}

impl fmt::Debug for Run {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Run")
            .field("phase", &self.phase)
            .field("frozen", &self.frozen.keys().collect::<Vec<_>>())
            .field("computed", &self.computed)
            .field("diagnostics", &self.diagnostics.len())
            .finish()
    }
}

/// The stamp of a plan's registry, the rewrite stages it schedules, and the
/// values of the options those rewrite stages read.
fn stamp_of(plan: &Plan, options: &BTreeMap<StageIdx, Arc<ResolvedOptions>>) -> Stamp {
    let mut hasher = crate::model::digest::Fnv1a64::new();
    for idx in plan.rewrites() {
        hasher.write_u64(options[idx].digest());
    }
    Stamp {
        digest: plan.registry().digest(),
        active_rewrites: plan.rewrite_names(),
        option_reads: hasher.value(),
    }
}

/// Applies `f` to every index below `n`, in parallel under the Cargo feature
/// `parallel` of `flm-core` when the options allow it.
///
/// The results always come back in index order, whichever path ran, so the
/// parallel and the sequential run produce the same output.
fn map_indexed<T, F>(n: usize, options: &RunOptions, f: F) -> Vec<T>
where
    T: Send,
    F: Fn(usize) -> T + Sync + Send,
{
    #[cfg(feature = "parallel")]
    {
        use rayon::prelude::*;
        if !options.sequential {
            return (0..n).into_par_iter().map(&f).collect();
        }
    }
    let _ = options;
    (0..n).map(&f).collect()
}

/// The runner's thread pool, built the first time it is needed.
///
/// Its threads are created with the large stack the recursion guard expects,
/// and each is marked as such once when the pool starts it, so that guarded
/// work never has to create a thread of its own.
#[cfg(feature = "parallel")]
fn pool() -> Result<&'static rayon::ThreadPool, RunError> {
    use std::sync::OnceLock;
    static POOL: OnceLock<Result<rayon::ThreadPool, String>> = OnceLock::new();
    let pool = POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .stack_size(guard::WORKER_STACK_SIZE)
            .start_handler(|_| guard::mark_worker_thread())
            .build()
            .map_err(|error| error.to_string())
    });
    match pool {
        Ok(pool) => Ok(pool),
        Err(detail) => Err(RunError::Worker { detail: detail.clone() }),
    }
}

/// Every diagnostic of a run, as one sequence: each fragment's parse
/// diagnostics first, in the order in which the document's walk reaches the
/// fragments, then the diagnostics that the run produced.
///
/// Created by [`Run::diagnostics`]. A fragment placed in the document more
/// than once contributes its parse diagnostics once, at the first place the
/// walk reaches it, because they are facts about the text, not about a
/// placement. A diagnostic reports a problem with a document: a severity, a
/// position, and a typed condition with its data
/// ([diagnostics](crate::guide::concepts_overview#diagnostics)).
pub struct DiagnosticsView<'a> {
    doc: &'a Doc,
    pipeline: &'a FlmDiagnostics,
}

impl<'a> DiagnosticsView<'a> {
    /// The document's fragments in the order the document's walk reaches
    /// them, each appearing once however many times it is placed.
    fn fragments(&self) -> Vec<&'a crate::model::doc::Fragment> {
        let content = self.doc.content();
        let mut seen: BTreeSet<usize> = BTreeSet::new();
        let mut fragments = Vec::new();
        for id in content.walk_trees() {
            if let Some(fragment) = content.fragment(id) {
                if seen.insert(Arc::as_ptr(fragment) as usize) {
                    fragments.push(&**fragment);
                }
            }
        }
        fragments
    }

    /// Returns every kept diagnostic, in order.
    pub fn iter(&self) -> impl Iterator<Item = &'a FlmDiagnostic> + 'a {
        let pipeline = self.pipeline;
        self.fragments()
            .into_iter()
            .flat_map(|fragment| fragment.diags().iter())
            .chain(pipeline.iter())
    }

    /// The number of kept diagnostics.
    ///
    /// A run may record more diagnostics than it keeps (see
    /// [`RunOptions::max_diagnostics`]); the ones past the limit are not
    /// counted here. [`DiagnosticsView::any_recorded`] and
    /// [`DiagnosticsView::error_count`] do count them.
    pub fn retained_len(&self) -> usize {
        self.iter().count()
    }

    /// Whether any diagnostic was reported, kept or not.
    pub fn any_recorded(&self) -> bool {
        self.fragments().iter().any(|fragment| !fragment.diags().is_empty())
            || !self.pipeline.is_empty()
    }

    /// Whether any diagnostic of Error severity was reported, kept or not. The
    /// run's error gate checks this.
    pub fn has_errors(&self) -> bool {
        self.fragments().iter().any(|fragment| fragment.diags().has_errors())
            || self.pipeline.has_errors()
    }

    /// The number of diagnostics of Error severity that were reported, kept or
    /// not.
    pub fn error_count(&self) -> usize {
        self.fragments().iter().map(|fragment| fragment.diags().error_count()).sum::<usize>()
            + self.pipeline.error_count()
    }

    /// Renders every kept diagnostic as text, joined by line breaks; the
    /// diagnostics past the limit are not mentioned.
    pub fn render_all(&self) -> String {
        self.iter().map(|d| d.render()).collect::<Vec<_>>().join("\n")
    }
}

impl fmt::Debug for DiagnosticsView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter().map(|d| d.identifier().to_string())).finish()
    }
}
