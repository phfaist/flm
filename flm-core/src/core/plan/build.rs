//! The [`Plan`] and how it is built and checked.
//!
//! A plan is the list of stages that satisfies a plan request, in dependency order
//! and grouped in batches; it is a pure value of the registry and the request, with
//! nothing of any document in it. [`Plan::build`] builds the plan of a
//! [`PlanRequest`]; [`Plan::for_workflow`] first turns a registered workflow into a
//! plan request ([`workflow_request`]) and then builds it. Every reason a plan
//! cannot be built is a [`PlanError`] that identifies the stages, capabilities,
//! elements or names involved; a preference the plan cannot honor is a
//! [`PlanWarning`] kept in the plan. See [`crate::core::plan`] for the whole
//! process.
//!
//! # What a plan holds
//!
//! - Its **order**: the scheduled stages that run after the rewrite stages, sorted
//!   so that every stage follows the stages it depends on, with registration
//!   order breaking ties. Two registries built from the same features,
//!   registered in the same order, therefore give the same order.
//! - Its **batches**: consecutive groups of stages in that order, none of which
//!   depends on another stage of the same group. The batches are computed here,
//!   once. Whether the runner runs a batch's stages one after another or at the
//!   same time (the `parallel` cargo feature), it runs exactly these batches, so
//!   both produce identical results.
//! - Its **rewrites**: the rewrite stages that will run, in the total order the
//!   plan fixes for them. A rewrite stage runs before the document's trees are
//!   frozen and turns one tree into another
//!   ([`RewriteStage`](crate::core::stage::RewriteStage)); every rewrite stage
//!   runs before every other stage.
//! - A [`StagePlan`] for every registered stage: the stage's manifest (its
//!   declaration together with what it needs for this plan) and its reads and
//!   writes resolved against the registry. The runner and every check read these
//!   records instead of asking the stage again, so a stage cannot state one thing
//!   while the plan is built and another while it runs.
//! - The record of the choices: the lowering rules chosen per backend, the
//!   backends that take part, the stages that were ruled out, the external tables
//!   the run needs (tables no stage writes, whose values the caller installs;
//!   [`Externals`](crate::core::run::Externals)), and the warnings.
//!
//! # The steps of building a plan
//!
//! 1. **The plan request is checked.** Every demanded capability, preferred
//!    stage, backend, preferred rule, accepted unrenderable element and render
//!    option value must be something the registry declares. Each failure
//!    identifies what was written and, except for an unknown element, lists what
//!    is registered.
//! 2. **A lowering rule is chosen per element, for every registered backend**
//!    ([`SelectedRules`]). Which backends take part is not known yet, because it
//!    follows from which stages are scheduled. When the rules of a backend cannot
//!    be chosen, the failure is kept and returned only if that backend takes part.
//! 3. **Every registered stage is asked what it needs**, and the needs are
//!    resolved against the registry into [`StagePlan`] records.
//! 4. **One stage is chosen to provide each needed capability**, by the provider
//!    selection of [`crate::core::plan::selection`].
//! 5. **The stages are ordered, and the plan is checked**, as described below.
//!
//! # What a plan refuses
//!
//! Besides names the registry does not declare, a plan refuses:
//!
//! - a stage that needs a capability, table or option type no feature declares,
//!   or reads a table under the wrong value type;
//! - a stage that reads an option type whose value is fixed when the registry is
//!   built;
//! - a rewrite stage that declares that it reads a table, or that requires the
//!   capability of a table that a stage running after the rewrite stages writes:
//!   every rewrite stage runs before any table is computed;
//! - a stage that requires a collection, which can only be demanded;
//! - a stage that requires a capability that scheduling the stage itself would
//!   rule out ([`PlanError::SelfExclusion`]);
//! - a needed capability with no provider left, or with several and nothing to
//!   choose between them;
//! - several lowering rules able to lower one element on one backend, with
//!   nothing to choose between them;
//! - a cycle among the scheduled stages;
//! - two scheduled stages that write one table;
//! - two rewrite stages with nothing that orders them relative to each other;
//! - a stage without effects outside the store (the run's collection of
//!   computed tables, [`Computed`](crate::core::data::store::Computed)) that
//!   depends on a stage with such effects;
//! - a cycle among the elements that the chosen lowering rules produce;
//! - a backend that cannot render every element a document may contain.
//!
//! Each case is a variant of [`PlanError`], which documents it.
//!
//! # Two placement rules
//!
//! **Stages with effects outside the store run last.** A stage's effect
//! ([`Effect`]) is `Compute` (it writes data and diagnostics), `Check` (it writes
//! diagnostics only) or `Io` (it has effects outside the store, such as writing
//! the output a caller asked for). The scheduled stages are ordered as two
//! blocks: first every [`Effect::Compute`] and [`Effect::Check`] stage, then
//! every [`Effect::Io`] stage, each block in its own dependency order. A
//! dependency in the other direction, from a `Compute` or `Check` stage on an
//! `Io` stage, is refused ([`PlanError::IoProductReadByPure`]). No batch mixes
//! the two blocks, so a run that stops before its first `Io` stage because of
//! errors stops at a batch boundary ([`Plan::first_io_batch`]).
//!
//! **Ordering edges between a rewrite stage and a later stage are ignored.** An
//! ordering edge places one stage before or after another without making either
//! one run ([`OrderingDef`]). Every rewrite stage runs before every other stage,
//! so such an edge cannot change any placement. The same holds for a rewrite
//! stage's requirement on a capability without a table that a later stage
//! provides. A rewrite stage that requires the capability of a table computed by
//! a later stage is a different case, and is refused
//! ([`PlanError::RewriteNeedsComputed`]).

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use crate::model::names::{
    BackendName, CapName, ElementName, Name, OutputName, RuleName, StageName, WorkflowName,
};
use crate::model::options::{BindSite, OptionError, OptionRef};
use crate::core::plan::cx::PlanCx;
use crate::core::plan::request::{Overrides, PlanKey, PlanRequest};
use crate::core::plan::selection::{self, Excluded, SelectedRules};
use crate::core::registry::{StageKind, WorkflowDef};
use crate::model::digest::Fnv1a64;
use crate::core::registry::entities::{OrderingDef, PeerName};
use crate::core::registry::frozen::{CapId, FlmRegistry, StageIdx};
use crate::core::registry::order::sort_by_priority;
use crate::core::stage::{Effect, Manifest, Needs, Scope};
use crate::core::data::store::{ReadEntry, ReadSet, WriteEntry, WriteSet};
use crate::core::data::table::{TableId, TableRef};

/// The reason a plan could not be built, returned by [`Plan::build`],
/// [`Plan::for_workflow`] and [`workflow_request`].
///
/// Every variant holds the names of the stages, capabilities, elements or other
/// entities involved. A variant about a name that is not declared also holds the
/// list of the declared names of that kind, except `UnknownElement`, so that its
/// message can list the alternatives. The `Display` text is a message for a person; match on the
/// variant to act on the error. The [module documentation](self) gives the order
/// in which the checks run.
///
/// Every variant that [`Plan::build`] returns also holds the workflow the plan
/// request was made from ([`PlanRequest::workflow`]), and its message names
/// that workflow: as the subject of the sentence where the request asks for
/// something ("the plan request for workflow `html` demands …"; "the plan
/// request demands …" for a request built by hand), and as an opening "for
/// workflow `html`, " where the sentence is about a stage, a backend or the
/// plan. The request of a workflow also holds the caller's overrides, so the
/// subject is the request made for the workflow, not the workflow itself.
/// `RenderOption` carries the name inside its [`OptionError`].
/// `UnknownWorkflow` and `UnknownOutput` hold the name of the workflow they
/// are about.
#[derive(Debug, Clone)]
pub enum PlanError {
    /// A demanded name is neither a declared capability nor a declared
    /// collection.
    UnknownDemand {
        /// The demanded name.
        name: CapName,
        /// Every declared capability and collection, in name order.
        roster: Vec<Name>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A preferred stage is not registered.
    UnknownPreference {
        /// The preferred stage name.
        stage: StageName,
        /// Every registered stage, in name order.
        roster: Vec<Name>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// The plan request refers to a backend that is not registered, in its
    /// lowering selections or in its accepted unrenderable elements.
    UnknownBackend {
        /// The backend name written in the plan request.
        backend: BackendName,
        /// Every registered backend, in name order.
        roster: Vec<Name>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A preferred lowering rule is not registered.
    UnknownRule {
        /// The backend the preference was written for.
        backend: BackendName,
        /// The preferred rule name.
        rule: RuleName,
        /// Every registered rule, in name order.
        roster: Vec<Name>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// An element the plan request accepts as unrenderable is not registered.
    UnknownElement {
        /// The backend on which the element is accepted as unrenderable.
        backend: BackendName,
        /// The element name written in the plan request.
        element: ElementName,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// The plan request sets an option value that the render request may not
    /// set: the option type is not declared, or its binding does not admit the
    /// render request ([`Bind`](crate::model::options::Bind)). The option error
    /// holds the workflow the request was made from, if any.
    RenderOption(OptionError),
    /// A stage needs a capability, a table or an option type that no feature
    /// declares.
    NeedsUndeclared {
        /// The stage.
        stage: StageName,
        /// The kind of the undeclared thing: `capability`, `table` or
        /// `option type`.
        kind: &'static str,
        /// The undeclared name the stage needs.
        name: Name,
        /// Every declared name of that kind, in name order.
        roster: Vec<Name>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A stage reads a table as a different value type than the one the table is
    /// declared to hold.
    NeedsTableType {
        /// The stage.
        stage: StageName,
        /// The table.
        table: CapName,
        /// The value type the table is declared to hold.
        declared: &'static str,
        /// The value type the stage's read expects.
        referenced: &'static str,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A stage declares a read of an option type whose value is fixed when the
    /// registry is built ([`Bind::Registry`](crate::model::options::Bind::Registry)).
    /// No stage may read such an option type: the feature that owns it holds the
    /// value in its configuration, and a read could only return the type's
    /// default.
    RegistryBoundRead {
        /// The option error, an [`OptionError::RegistryBoundRead`].
        error: OptionError,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A rewrite stage declares that it reads a table. A rewrite stage may not
    /// read tables: it runs before any table is computed.
    RewriteReads {
        /// The rewrite stage.
        stage: StageName,
        /// The table it declared it reads.
        table: CapName,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A rewrite stage requires the capability of a table that a scheduled stage
    /// other than a rewrite stage provides. Such a stage runs after every rewrite
    /// stage, so the rewrite stage would wait for data computed after it.
    RewriteNeedsComputed {
        /// The rewrite stage.
        stage: StageName,
        /// The capability it requires.
        capability: CapName,
        /// The scheduled stage that provides that capability.
        provider: StageName,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A stage requires a collection. A collection is a name for a set of
    /// capabilities ([`CollectionDef`](crate::core::registry::CollectionDef)); a
    /// plan request may demand one, but a stage may not require one.
    CollectionRequired {
        /// The stage.
        stage: StageName,
        /// The collection it requires.
        collection: CapName,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A stage requires a capability that scheduling the stage itself would
    /// rule out.
    ///
    /// Scheduling a stage rules out every other stage that provides any of the
    /// same capabilities, including the capabilities that what it provides
    /// refines ([refinement](crate::guide::pipeline#refinement)). Suppose a stage
    /// requires a capability that is related by refinement to one it provides:
    /// one refines the other, or both refine a common capability. Every provider
    /// of the required capability then provides that common capability too, so
    /// scheduling the stage would rule out every provider of what it requires.
    /// The check runs over every registered stage, scheduled or not. Requiring
    /// exactly a capability the stage itself declares as provided is allowed:
    /// the stage satisfies that requirement itself.
    SelfExclusion {
        /// The stage.
        stage: StageName,
        /// The capability it requires.
        requires: CapName,
        /// The capability it provides that is related by refinement to the
        /// required one.
        provides: CapName,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A needed capability has no provider left: no registered stage provides
    /// it, or provider selection ruled out every stage that provides it.
    Unsatisfiable {
        /// The capability.
        capability: CapName,
        /// Each stage that provides the capability, with the scheduled stage that
        /// ruled it out; empty when no registered stage provides the capability.
        blocked: Vec<Excluded>,
        /// When no registered stage provides the capability, the text its
        /// declaration gives for that case
        /// ([`CapabilityDef::unprovided`](crate::core::registry::CapabilityDef::unprovided)),
        /// which the message gives in place of the generic one; `None` otherwise.
        unprovided: Option<String>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A needed capability has several possible providers, and neither a
    /// preferred stage nor a default refinement chooses one of them. A default
    /// refinement is the refinement that a demand for its parent capability
    /// selects when nothing else decides
    /// ([refinement](crate::guide::pipeline#refinement)).
    Contested {
        /// The capability.
        capability: CapName,
        /// The stages that could provide it, in name order.
        candidates: Vec<StageName>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// Several lowering rules could lower one element on one backend, and
    /// neither a preference nor a default mark
    /// ([`DefaultMark`](crate::core::lowering::DefaultMark)) chooses one of them.
    RuleContest {
        /// The backend.
        backend: BackendName,
        /// The element.
        element: ElementName,
        /// The rules that could lower it, in name order.
        candidates: Vec<RuleName>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// The scheduled stages depend on one another in a cycle.
    Cycle {
        /// The cycle, in dependency order, starting at the stage whose name comes
        /// first and ending with that stage again.
        stages: Vec<StageName>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// Two scheduled stages write one table. At most one scheduled stage may
    /// write a table.
    TwoWriters {
        /// The table.
        table: CapName,
        /// The two stages, in registration order.
        stages: Vec<StageName>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// Two scheduled rewrite stages have no order relative to each other.
    ///
    /// Each rewrite stage works on the trees the previous one produced, so the
    /// rewrite stages of a plan need one total order. An ordering declaration
    /// ([`OrderingDef`]) between two of them supplies it.
    RewriteTie {
        /// Every scheduled rewrite stage, in dependency order, with registration
        /// order breaking ties.
        active: Vec<StageName>,
        /// One of the two unordered rewrites.
        first: StageName,
        /// The other one.
        second: StageName,
        /// Ordering declarations that would order the two; registering either one
        /// is enough.
        resolving: Vec<OrderingDef>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A stage whose effect is `Compute` or `Check` depends on a stage whose
    /// effect is [`Effect::Io`] (a stage that acts outside the run's computed
    /// data, for example by writing files).
    ///
    /// Every `Io` stage runs after every other stage, so only another `Io` stage
    /// may depend on one.
    IoProductReadByPure {
        /// The `Compute` or `Check` stage.
        reader: StageName,
        /// The `Io` stage it depends on.
        io_stage: StageName,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// The elements that the chosen lowering rules produce lead back to an
    /// element already on the path, so lowering would never finish.
    ///
    /// Each lowering rule declares the elements it produces; lowering an element
    /// with its rule gives nodes of those elements, which are lowered in turn.
    ProducesCycle {
        /// The backend.
        backend: BackendName,
        /// The elements of the cycle, starting at the one whose name comes first;
        /// the rule of the last one produces the first.
        cycle: Vec<ElementName>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A backend that takes part in the plan cannot render every element a
    /// document may contain.
    ///
    /// An element passes when the backend writes it natively (and the request
    /// does not prefer a lowering rule for it instead), or when the lowering rule
    /// chosen for it produces only elements that pass. Every element a document
    /// may contain must pass, except the elements the plan request accepts as
    /// unrenderable on that backend. An accepted element still does not pass
    /// when another element's rule produces it.
    Incomplete {
        /// The backend.
        backend: BackendName,
        /// The elements that do not pass.
        missing: Vec<ElementName>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// The requested workflow is not registered.
    UnknownWorkflow {
        /// The requested workflow name.
        workflow: WorkflowName,
        /// Every registered workflow, in name order.
        roster: Vec<Name>,
    },
    /// A workflow lists an output that is not registered.
    UnknownOutput {
        /// The workflow listing it.
        workflow: WorkflowName,
        /// The output it lists.
        output: OutputName,
        /// Every registered output, in name order.
        roster: Vec<Name>,
    },
}

fn join<'a>(names: impl IntoIterator<Item = &'a Name>) -> String {
    names.into_iter().map(|name| name.as_str()).collect::<Vec<_>>().join(", ")
}

/// The subject of a sentence about what a plan request asks for: "the plan
/// request for workflow `w`" when the request was made from the workflow `w`
/// ([`PlanRequest::workflow`]), and "the plan request" for a request built by
/// hand. The request of a workflow merges the workflow's fields with the
/// caller's overrides, so what it asks for may come from either. The messages of [`PlanError`] and [`PlanWarning`], and those of the
/// run's diagnostics built from the warnings, take their subject from here.
pub(crate) fn request_subject(workflow: Option<&str>) -> String {
    match workflow {
        Some(workflow) => format!("the plan request for workflow `{workflow}`"),
        None => "the plan request".to_string(),
    }
}

/// The opening of a message whose subject is not the plan request itself (a
/// stage, a backend, the plan): "for workflow `w`, " when the request was made
/// from the workflow `w`, and nothing for a request built by hand.
fn workflow_opening(workflow: Option<&str>) -> String {
    match workflow {
        Some(workflow) => format!("for workflow `{workflow}`, "),
        None => String::new(),
    }
}

fn name_of(workflow: &Option<WorkflowName>) -> Option<&str> {
    workflow.as_ref().map(|workflow| workflow.as_str())
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let subject = |workflow: &Option<WorkflowName>| request_subject(name_of(workflow));
        let opening = |workflow: &Option<WorkflowName>| workflow_opening(name_of(workflow));
        match self {
            PlanError::UnknownDemand { name, roster, workflow } => write!(
                f,
                "{} demands `{name}`, but no feature declares a capability or a collection of \
                 that name; declared capabilities and collections: [{}]",
                subject(workflow),
                join(roster)
            ),
            PlanError::UnknownPreference { stage, roster, workflow } => {
                write!(
                    f,
                    "{} prefers the stage `{stage}`, but no stage of that name is registered; \
                     registered stages: [{}]",
                    subject(workflow),
                    join(roster)
                )
            }
            PlanError::UnknownBackend { backend, roster, workflow } => {
                write!(f, "{} refers to the backend `{backend}`, but no backend of that name is \
                           registered; registered backends: [{}]", subject(workflow), join(roster))
            }
            PlanError::UnknownRule { backend, rule, roster, workflow } => write!(
                f,
                "{} prefers the lowering rule `{rule}` on backend `{backend}`, but no lowering \
                 rule of that name is registered; registered lowering rules: [{}]",
                subject(workflow),
                join(roster)
            ),
            PlanError::UnknownElement { backend, element, workflow } => {
                write!(
                    f,
                    "{} accepts the element `{element}` as unrenderable on backend `{backend}`, \
                     but no element of that name is registered",
                    subject(workflow)
                )
            }
            PlanError::RenderOption(error) => fmt::Display::fmt(error, f),
            PlanError::RegistryBoundRead { error, workflow } => {
                write!(f, "{}{error}", opening(workflow))
            }
            PlanError::NeedsUndeclared { stage, kind, name, roster, workflow } => write!(
                f,
                "{}stage `{stage}` needs the {kind} `{name}`, but no feature declares this \
                 {kind}; declared {kind} names: [{}]",
                opening(workflow),
                join(roster)
            ),
            PlanError::NeedsTableType { stage, table, declared, referenced, workflow } => write!(
                f,
                "{}stage `{stage}` reads the table `{table}` as `{referenced}`, but the table \
                 is declared to hold `{declared}`",
                opening(workflow)
            ),
            PlanError::RewriteReads { stage, table, workflow } => {
                write!(
                    f,
                    "{}rewrite stage `{stage}` declares a read of the table `{table}`, but a \
                     rewrite stage must not read tables: it runs before any table is computed",
                    opening(workflow)
                )
            }
            PlanError::RewriteNeedsComputed { stage, capability, provider, workflow } => write!(
                f,
                "{}rewrite stage `{stage}` requires `{capability}`, the capability of a table \
                 that stage `{provider}` computes, but a rewrite stage runs before every other \
                 stage and must not require a table another stage computes",
                opening(workflow)
            ),
            PlanError::CollectionRequired { stage, collection, workflow } => {
                write!(
                    f,
                    "{}stage `{stage}` requires the collection `{collection}`, but a collection \
                     can only be demanded in a plan request, not required by a stage",
                    opening(workflow)
                )
            }
            PlanError::SelfExclusion { stage, requires, provides, workflow } => write!(
                f,
                "{}stage `{stage}` requires `{requires}` and provides `{provides}`, but one of \
                 `{requires}` and `{provides}` refines the other or both refine a common \
                 capability, so scheduling the stage would rule out every provider of \
                 `{requires}`",
                opening(workflow)
            ),
            PlanError::Unsatisfiable { capability, blocked, unprovided, workflow } => {
                if let (true, Some(text)) = (blocked.is_empty(), unprovided) {
                    write!(f, "{}{text}", opening(workflow))
                } else if blocked.is_empty() {
                    write!(
                        f,
                        "{}the plan needs the capability `{capability}`, but no registered stage \
                         provides it",
                        opening(workflow)
                    )
                } else {
                    let detail: Vec<String> = blocked
                        .iter()
                        .map(|b| {
                            format!(
                                "stage `{}` was ruled out by the scheduled stage `{}`, which \
                                 also provides `{}`",
                                b.stage, b.excluded_by, b.over
                            )
                        })
                        .collect();
                    write!(f, "{}the plan needs the capability `{capability}`, but every stage \
                               that provides it was ruled out, because only one scheduled stage \
                               may provide each capability: {}", opening(workflow),
                               detail.join("; "))
                }
            }
            PlanError::Contested { capability, candidates, workflow } => write!(
                f,
                "{}the plan needs the capability `{capability}`, which several stages provide \
                 ([{}]), and neither a preference of the plan request nor a capability marked \
                 as the default among those that refine `{capability}` chooses one of these \
                 stages; prefer one of them in the plan request",
                opening(workflow),
                candidates.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")
            ),
            PlanError::RuleContest { backend, element, candidates, workflow } => write!(
                f,
                "{}several lowering rules can lower `{element}` on backend `{backend}` ([{}]), \
                 and none of them is preferred or marked as the default",
                opening(workflow),
                candidates.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")
            ),
            PlanError::Cycle { stages, workflow } => write!(
                f,
                "{}the scheduled stages depend on one another in a cycle: {}",
                opening(workflow),
                stages.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(" -> ")
            ),
            PlanError::TwoWriters { table, stages, workflow } => write!(
                f,
                "{}only one scheduled stage may write the table `{table}`, but two do: {}",
                opening(workflow),
                stages.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")
            ),
            PlanError::RewriteTie { active, first, second, resolving, workflow } => write!(
                f,
                "{}the scheduled rewrite stages [{}] include `{first}` and `{second}`, which \
                 must run in a defined order, but nothing orders them; an ordering declaration \
                 such as {} would order them",
                opening(workflow),
                active.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", "),
                resolving
                    .iter()
                    .map(|edge| format!(
                        "`{}` after `{}`",
                        edge.stage,
                        join(edge.after.iter().map(|c| c.name()))
                    ))
                    .collect::<Vec<_>>()
                    .join(" or ")
            ),
            PlanError::IoProductReadByPure { reader, io_stage, workflow } => write!(
                f,
                "{}stage `{reader}` depends on stage `{io_stage}`, whose effect is `Io` (it acts \
                 outside the run's computed data, for example by writing output), but only \
                 another stage with effect `Io` may depend on it",
                opening(workflow)
            ),
            PlanError::ProducesCycle { backend, cycle, workflow } => write!(
                f,
                "{}on backend `{backend}`, the selected lowering rules produce one another's \
                 elements in a cycle, so lowering would never finish: {}",
                opening(workflow),
                cycle.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(" -> ")
            ),
            PlanError::Incomplete { backend, missing, workflow } => write!(
                f,
                "backend `{backend}` cannot render the elements [{}]: it does not write them \
                 natively, the selected lowering rules do not lower them to elements it writes \
                 natively, and {} does not accept them as unrenderable",
                missing.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", "),
                subject(workflow)
            ),
            PlanError::UnknownWorkflow { workflow, roster } => {
                write!(f, "the workflow `{workflow}` is requested, but no workflow of that name \
                           is registered; registered workflows: [{}]", join(roster))
            }
            PlanError::UnknownOutput { workflow, output, roster } => write!(
                f,
                "workflow `{workflow}` lists the output `{output}`, but no output of that name \
                 is registered; registered outputs: [{}]",
                join(roster)
            ),
        }
    }
}

impl std::error::Error for PlanError {}

/// A preference of the plan request that the plan could not honor.
///
/// A plan request may prefer something the registry cannot give: a lowering rule
/// that is set aside (excluded from rule selection; see [`PeerName`]), a lowering
/// rule that belongs to
/// other backends, or a stage that provider selection had to rule out. The plan
/// is still built, without that preference, and the plan keeps one warning per
/// such preference, because the caller asked for something it did not get.
///
/// Read the warnings from [`Plan::warnings`]. The runner reports each one as a
/// diagnostic of Warning severity at the start of every run of the plan
/// ([`Run`](crate::core::run::Run)). The `Display` text describes the warning
/// for a person; the runner's diagnostic carries its own message. Each variant
/// holds the workflow the plan request was made from ([`PlanRequest::workflow`]),
/// and both messages name it in the subject of the sentence ("the plan request
/// for workflow `html` prefers …"; "the plan request prefers …" for a request
/// built by hand).
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum PlanWarning {
    /// A preferred lowering rule is set aside: a peer declaration the rule
    /// depends on (a backend it is marked as the default on, an element it
    /// produces or a table it reads other than as an optional read, which its
    /// feature expects another feature to declare; see [`PeerName`]) resolves to nothing in this registry, so no
    /// plan can choose the rule.
    PreferredRuleSetAside {
        /// The backend the preference was stated for.
        backend: BackendName,
        /// The rule the request preferred.
        rule: RuleName,
        /// The peer declarations the rule depends on that did not resolve, in
        /// the order of the rule's dependencies: the backends it is marked as the
        /// default on, then the elements it produces, then the tables it reads.
        unresolved: Vec<PeerName>,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A preferred lowering rule is not a candidate on the backend the preference
    /// was stated for: the rule is marked as the default on other backends only
    /// ([`DefaultMark::OnBackends`](crate::core::lowering::DefaultMark::OnBackends)),
    /// and that mark also restricts the rule to those backends.
    PreferredRuleNotCandidate {
        /// The backend the preference was stated for.
        backend: BackendName,
        /// The rule the request preferred.
        rule: RuleName,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
    /// A preferred stage could not be scheduled, because provider selection had
    /// ruled it out when it scheduled another stage that provides a capability
    /// in common.
    IgnoredPreference {
        /// The stage the request preferred.
        stage: StageName,
        /// The stage that was scheduled instead, which ruled it out.
        excluded_by: StageName,
        /// A capability both of them provide.
        over: CapName,
        /// The workflow the plan request was made from, or `None` for a request
        /// built by hand ([`PlanRequest::workflow`]).
        workflow: Option<WorkflowName>,
    },
}

impl fmt::Display for PlanWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanWarning::PreferredRuleSetAside { backend, rule, unresolved, workflow } => write!(
                f,
                "{} prefers the lowering rule `{rule}` on backend `{backend}`, but that rule \
                 cannot be used: it depends on {}, which no registered feature declares",
                request_subject(name_of(workflow)),
                unresolved.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ")
            ),
            PlanWarning::PreferredRuleNotCandidate { backend, rule, workflow } => write!(
                f,
                "{} prefers the lowering rule `{rule}` on backend `{backend}`, but that rule is \
                 marked as the default on other backends only, which restricts it to those \
                 backends",
                request_subject(name_of(workflow))
            ),
            PlanWarning::IgnoredPreference { stage, excluded_by, over, workflow } => write!(
                f,
                "{} prefers the stage `{stage}`, but that stage cannot be scheduled: the \
                 scheduled stage `{excluded_by}` also provides `{over}`, and only one scheduled \
                 stage may provide each capability",
                request_subject(name_of(workflow))
            ),
        }
    }
}

/// The record a plan keeps about one registered stage: the stage's manifest,
/// and its reads, writes and requirements resolved against the registry.
///
/// A stage manifest ([`Manifest`]) is the stage's declaration, fixed when it was
/// registered, together with the needs it stated for this plan
/// ([`Needs`]). The plan keeps one record for every registered stage, whether or
/// not the stage will run; [`Plan::manifest`] returns one and
/// [`Plan::manifests`] all of them. The runner reads these records instead of
/// asking the stage again.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct StagePlan {
    /// The stage's number in the registry.
    pub idx: StageIdx,
    /// The stage's declaration, together with the needs it stated for this
    /// plan.
    #[serde(serialize_with = "serialize_manifest")]
    pub manifest: Arc<Manifest>,
    /// The tables the stage reads, optional reads included, each with the name
    /// of the scheduled stage that writes it, when there is one.
    pub reads: ReadSet,
    /// The tables the stage writes, resolved.
    pub writes: WriteSet,
    /// The option types the stage declared that it reads.
    pub options: Vec<OptionRef>,
    /// Every capability the stage requires, including the capabilities of the
    /// tables it reads (not those of its optional reads).
    pub requires: BTreeSet<CapId>,
    /// Capabilities whose providers this stage runs after, when those providers
    /// are scheduled. These order the stage without causing anything to be
    /// scheduled.
    pub after: BTreeSet<CapId>,
    /// Capabilities whose providers this stage runs before, when those providers
    /// are scheduled. These order the stage without causing anything to be
    /// scheduled.
    pub before: BTreeSet<CapId>,
}

/// Writes the manifest a stage plan shares, for [`StagePlan`]'s serialized
/// form.
fn serialize_manifest<S: serde::Serializer>(manifest: &Arc<Manifest>, serializer: S) -> Result<S::Ok, S::Error> {
    serde::Serialize::serialize(&**manifest, serializer)
}

/// A plan: the list of stages that satisfies a plan request, in dependency order
/// and grouped in batches.
///
/// A plan is a pure value of the registry and the request, with nothing of any
/// document in it; one plan therefore serves every document rendered the same
/// way. Build one with [`Plan::for_workflow`] from the name of a registered
/// workflow, or with [`Plan::build`] from a [`PlanRequest`]; then run it over a
/// document with [`Run::new`](crate::core::run::Run::new), which takes it in an
/// `Arc`. The [module documentation](self) describes what a plan holds and how
/// it is checked.
///
/// Besides the order of the stages, a plan exposes the record of how it was
/// built: [`Plan::warnings`] for the preferences it could not honor,
/// [`Plan::excluded`] and [`Plan::commit_order`] for the provider choices,
/// [`Plan::selections`] for the lowering rules, and [`Plan::render`] for all of
/// it as stable text.
///
/// # Examples
///
/// ```
/// use flm_core::core::plan::{Overrides, Plan};
/// use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
/// use flm_core::stdcore::{self, StdcoreConfig};
///
/// let mut builder = RegistryBuilder::new();
/// builder.use_package(stdcore::package(StdcoreConfig::new()));
/// builder.use_package(flm_core::math_passthrough::package());
/// let registry = builder.build(&RegistryInputs::new())?;
///
/// let plan = Plan::for_workflow(&registry, "text", &Overrides::none())?;
///
/// // The rewrite stages run first, then the other stages, in order.
/// assert!(!plan.rewrite_names().is_empty());
/// assert!(plan.active_backends().iter().any(|name| name.as_str() == "text"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct Plan {
    registry: Arc<FlmRegistry>,
    request: PlanRequest,
    demands: BTreeSet<CapName>,
    manifests: BTreeMap<StageIdx, StagePlan>,
    rewrites: Vec<StageIdx>,
    order: Vec<StageIdx>,
    batches: Vec<Range<usize>>,
    first_io_batch: Option<usize>,
    selections: BTreeMap<BackendName, SelectedRules>,
    active_backends: BTreeSet<BackendName>,
    needed: BTreeSet<CapId>,
    externals: BTreeSet<TableId>,
    key: PlanKey,
    warnings: Vec<PlanWarning>,
    excluded: Vec<Excluded>,
    commit_order: Vec<StageIdx>,
    preds: BTreeMap<StageIdx, BTreeSet<StageIdx>>,
}

impl Plan {
    /// Builds the plan that satisfies `request` against `registry`.
    ///
    /// The [module documentation](self) describes the steps and every check.
    /// The plan keeps the registry (an `Arc` clone) and a copy of `request`.
    ///
    /// # Errors
    ///
    /// Returns a [`PlanError`] for a name in `request` that the registry does
    /// not declare, a contradiction between what the stages declare, a choice
    /// that cannot be made, or a backend taking part in the plan that cannot
    /// render every element a document may contain. The variant holds the
    /// names of the entities involved.
    ///
    /// # Panics
    ///
    /// Building a plan asks every registered stage what it needs
    /// ([`Stage::needs`](crate::core::stage::Stage::needs) or
    /// [`RewriteStage::needs`](crate::core::stage::RewriteStage::needs)); a
    /// panic in that method propagates out of this call.
    pub fn build(registry: &Arc<FlmRegistry>, request: &PlanRequest) -> Result<Plan, PlanError> {
        let registry = Arc::clone(registry);
        let reg: &FlmRegistry = &registry;

        // --- the request is checked -----------------------------------------------------------
        let mut demands: BTreeSet<CapName> = BTreeSet::new();
        for demand in &request.demands {
            if reg.cap_id(demand).is_some() {
                demands.insert(demand.clone());
            } else if let Some(collection) = reg.collection(demand) {
                for member in &collection.members {
                    demands.insert(reg.capability(*member).name.clone());
                }
            } else {
                let mut roster: Vec<Name> =
                    reg.capabilities().iter().map(|c| c.name.name().clone()).collect();
                roster.extend(reg.collections().map(|c| c.name.name().clone()));
                roster.sort();
                return Err(PlanError::UnknownDemand {
                    name: demand.clone(),
                    roster,
                    workflow: request.workflow.clone(),
                });
            }
        }
        let stage_roster = || -> Vec<Name> {
            let mut names: Vec<Name> = reg.stages().iter().map(|s| s.name.name().clone()).collect();
            names.sort();
            names
        };
        let backend_roster =
            || -> Vec<Name> { reg.backends().map(|b| b.name.name().clone()).collect() };
        let mut prefer: Vec<StageIdx> = Vec::new();
        for stage in &request.prefer {
            match reg.stage_idx(stage) {
                Some(idx) => prefer.push(idx),
                None => {
                    return Err(PlanError::UnknownPreference {
                        stage: stage.clone(),
                        roster: stage_roster(),
                        workflow: request.workflow.clone(),
                    });
                }
            }
        }
        for (backend, selection) in &request.lowering {
            if reg.backend(backend).is_none() {
                return Err(PlanError::UnknownBackend {
                    backend: backend.clone(),
                    roster: backend_roster(),
                    workflow: request.workflow.clone(),
                });
            }
            for rule in &selection.prefer {
                if reg.rule_idx(rule).is_none() {
                    let mut roster: Vec<Name> =
                        reg.rules().iter().map(|r| r.rule.name.name().clone()).collect();
                    roster.sort();
                    return Err(PlanError::UnknownRule {
                        backend: backend.clone(),
                        rule: rule.clone(),
                        roster,
                        workflow: request.workflow.clone(),
                    });
                }
            }
        }
        for (backend, holes) in &request.unsupported {
            if reg.backend(backend).is_none() {
                return Err(PlanError::UnknownBackend {
                    backend: backend.clone(),
                    roster: backend_roster(),
                    workflow: request.workflow.clone(),
                });
            }
            for element in holes {
                if reg.elements_named(element).is_empty() {
                    return Err(PlanError::UnknownElement {
                        backend: backend.clone(),
                        element: element.clone(),
                        workflow: request.workflow.clone(),
                    });
                }
            }
        }
        let option_roster: Vec<_> = reg.options().keys().cloned().collect();
        if let Some(error) = crate::model::options::validate_values(
            &request.render_options,
            BindSite::Render,
            |key| reg.option_decl(key),
            &option_roster,
            request.workflow.as_ref(),
        )
        .into_iter()
        .next()
        {
            return Err(PlanError::RenderOption(error));
        }

        // --- a lowering rule is chosen per element, for every backend ---------------------------
        let mut selections: BTreeMap<BackendName, SelectedRules> = BTreeMap::new();
        let mut selection_errors: BTreeMap<BackendName, PlanError> = BTreeMap::new();
        // Per backend, the preferences that backend cannot honor. Only the
        // backends that take part are reported on, for the same reason that a
        // failure to choose their rules matters only for those.
        let mut selection_warnings: BTreeMap<BackendName, Vec<PlanWarning>> = BTreeMap::new();
        for backend in reg.backends() {
            let prefer =
                request.lowering.get(&backend.name).map(|s| s.prefer.as_slice()).unwrap_or(&[]);
            match selection::select_rules(reg, backend, prefer, &request.workflow) {
                Ok((selected, warnings)) => {
                    selections.insert(backend.name.clone(), selected);
                    selection_warnings.insert(backend.name.clone(), warnings);
                }
                Err(error) => {
                    selection_errors.insert(backend.name.clone(), error);
                }
            }
        }

        // --- every stage is asked what it needs -------------------------------------------------
        let cx = PlanCx::new(reg, request, &demands, &selections);
        let mut manifests: BTreeMap<StageIdx, StagePlan> = BTreeMap::new();
        for entry in reg.stages() {
            let needs = match &entry.kind {
                StageKind::Pipeline(stage) => stage.needs(&cx),
                StageKind::Rewrite(stage) => stage.needs(&cx),
            };
            let plan = resolve_needs(reg, entry.idx, &entry.decl, needs, &request.workflow)?;
            manifests.insert(entry.idx, plan);
        }

        // A stage may not require what its own scheduling would rule out. The
        // check runs over every registered stage and does not depend on the
        // demands, but it lives here rather than in the registry build because
        // what a stage requires is only known once it has been asked, which is
        // a per-plan question. A requirement that is exactly one of the stage's
        // own direct provides is allowed; any other requirement whose
        // refinement closure meets the closure of a direct provide would rule
        // out every provider of that requirement, the stage's own commitment
        // included. Requiring the parent of a capability the stage provides
        // falls under that too. The reported pair is the name-least one, so the
        // message does not depend on registration order. See the design
        // rationale at [§flm-dd-dr:pipe-self-exclusion] and
        // [§flm-dd-dr:pipe-stage-atomicity].
        for entry in reg.stages() {
            let plan = &manifests[&entry.idx];
            let direct: Vec<CapId> = entry
                .decl
                .provides
                .iter()
                .filter_map(|c| reg.cap_id(c))
                .chain(entry.writes.iter().map(|t| reg.table(*t).cap))
                .collect();
            let mut offending: Option<(CapName, CapName)> = None;
            for req in &plan.requires {
                if direct.contains(req) {
                    continue;
                }
                let req_closure: BTreeSet<CapId> =
                    reg.capability(*req).ancestors().iter().copied().collect();
                for prov in &direct {
                    let shares_an_ancestor =
                        reg.capability(*prov).ancestors().iter().any(|a| req_closure.contains(a));
                    if !shares_an_ancestor {
                        continue;
                    }
                    let pair =
                        (reg.capability(*req).name.clone(), reg.capability(*prov).name.clone());
                    if offending.as_ref().is_none_or(|least| pair < *least) {
                        offending = Some(pair);
                    }
                }
            }
            if let Some((requires, provides)) = offending {
                return Err(PlanError::SelfExclusion {
                    stage: entry.name.clone(),
                    requires,
                    provides,
                    workflow: request.workflow.clone(),
                });
            }
        }

        // --- a provider is chosen for each needed capability ------------------------------------
        let demand_ids: Vec<CapId> = demands.iter().filter_map(|name| reg.cap_id(name)).collect();
        let requires_of = |idx: StageIdx| -> &BTreeSet<CapId> { &manifests[&idx].requires };
        let selected = selection::select_providers(reg, demand_ids, &prefer, &requires_of, &request.workflow)?;

        // A backend takes part exactly when its lowering stage is scheduled, so
        // a failure to choose its rules matters only now.
        let mut active_backends: BTreeSet<BackendName> = BTreeSet::new();
        for backend in reg.backends() {
            if selected.scheduled.contains(&backend.lowering) {
                if let Some(error) = selection_errors.remove(&backend.name) {
                    return Err(error);
                }
                active_backends.insert(backend.name.clone());
            }
        }

        // --- the stages are ordered and the plan is checked -------------------------------------
        // Who writes each table and who provides each capability, among the
        // stages that will run.
        let mut writers: BTreeMap<TableId, Vec<StageIdx>> = BTreeMap::new();
        let mut providers: BTreeMap<CapId, Vec<StageIdx>> = BTreeMap::new();
        for idx in &selected.scheduled {
            let entry = reg.stage(*idx);
            for table in &entry.writes {
                writers.entry(*table).or_default().push(*idx);
            }
            for cap in &entry.provides {
                providers.entry(*cap).or_default().push(*idx);
            }
        }
        for (table, stages) in &writers {
            if stages.len() > 1 {
                return Err(PlanError::TwoWriters {
                    table: reg.table(*table).def.name.clone(),
                    stages: stages.iter().map(|idx| reg.stage(*idx).name.clone()).collect(),
                    workflow: request.workflow.clone(),
                });
            }
        }
        for plan in manifests.values_mut() {
            let names: Vec<CapName> = plan.reads.names();
            for name in names {
                let entry = plan.reads.get(&name).cloned();
                if let Some(mut entry) = entry {
                    entry.producer = writers
                        .get(&entry.id)
                        .and_then(|w| w.first())
                        .map(|idx| reg.stage(*idx).name.clone());
                    plan.reads.insert(name, entry);
                }
            }
        }

        // The dependency edges: an edge from b to a means a runs after b.
        let mut preds: BTreeMap<StageIdx, BTreeSet<StageIdx>> =
            selected.scheduled.iter().map(|idx| (*idx, BTreeSet::new())).collect();
        let mut add_edge = |from: StageIdx, to: StageIdx| {
            if from == to {
                return;
            }
            if let Some(set) = preds.get_mut(&to) {
                set.insert(from);
            }
        };
        let providers_of = |cap: CapId| -> Vec<StageIdx> {
            // Every scheduled stage that provides `cap`, whether directly or
            // by providing something that refines it.
            selected
                .scheduled
                .iter()
                .copied()
                .filter(|idx| reg.stage(*idx).provides.contains(&cap))
                .collect()
        };
        for idx in &selected.scheduled {
            let plan = &manifests[idx];
            for cap in &plan.requires {
                for provider in providers_of(*cap) {
                    add_edge(provider, *idx);
                }
            }
            // Reads declared as optional are in `plan.reads` as well, so a
            // scheduled writer of one orders its reader even though the read
            // caused nothing to be scheduled.
            for table in plan.reads.iter().map(|(_, e)| e.id) {
                if let Some(writer) = writers.get(&table).and_then(|w| w.first()) {
                    add_edge(*writer, *idx);
                }
            }
            for cap in &plan.after {
                for provider in providers_of(*cap) {
                    add_edge(provider, *idx);
                }
            }
            for cap in &plan.before {
                for provider in providers_of(*cap) {
                    add_edge(*idx, provider);
                }
            }
        }
        for ordering in reg.orderings() {
            let Some(idx) = reg.stage_idx(&ordering.stage) else { continue };
            if !selected.scheduled.contains(&idx) {
                continue;
            }
            for cap in ordering.after.iter().filter_map(|c| reg.cap_id(c)) {
                for provider in providers_of(cap) {
                    add_edge(provider, idx);
                }
            }
            for cap in ordering.before.iter().filter_map(|c| reg.cap_id(c)) {
                for provider in providers_of(cap) {
                    add_edge(idx, provider);
                }
            }
        }

        // The rewrites: refuse a need for computed data, drop the edges that
        // cross between rewrites and pipeline stages, and fix one total order.
        let is_rewrite = |idx: StageIdx| reg.stage(idx).is_rewrite();
        let rewrites_scheduled: Vec<StageIdx> =
            selected.scheduled.iter().copied().filter(|idx| is_rewrite(*idx)).collect();
        for idx in &rewrites_scheduled {
            for cap in &manifests[idx].requires {
                if reg.capability(*cap).table.is_some() {
                    if let Some(provider) = providers_of(*cap).into_iter().find(|p| !is_rewrite(*p))
                    {
                        return Err(PlanError::RewriteNeedsComputed {
                            stage: reg.stage(*idx).name.clone(),
                            capability: reg.capability(*cap).name.clone(),
                            provider: reg.stage(provider).name.clone(),
                            workflow: request.workflow.clone(),
                        });
                    }
                }
            }
        }
        for (idx, set) in preds.iter_mut() {
            let rewrite = is_rewrite(*idx);
            set.retain(|pred| is_rewrite(*pred) == rewrite);
        }
        let rewrite_preds: BTreeMap<StageIdx, BTreeSet<StageIdx>> =
            rewrites_scheduled.iter().map(|idx| (*idx, preds[idx].clone())).collect();
        let rewrites = topo_order(reg, &rewrites_scheduled, &rewrite_preds, &request.workflow)?;
        {
            // Every pair of rewrites must be ordered relative to each other,
            // directly or through others: they feed each other's output.
            let reach = |from: StageIdx, to: StageIdx| -> bool {
                let mut stack = vec![to];
                let mut seen = BTreeSet::new();
                while let Some(node) = stack.pop() {
                    if node == from {
                        return true;
                    }
                    if seen.insert(node) {
                        stack.extend(rewrite_preds[&node].iter().copied());
                    }
                }
                false
            };
            for (i, a) in rewrites.iter().enumerate() {
                for b in &rewrites[i + 1..] {
                    if !reach(*a, *b) && !reach(*b, *a) {
                        let first = reg.stage(*a);
                        let second = reg.stage(*b);
                        let cap_of = |idx: StageIdx| {
                            reg.stage(idx).rewrite.as_ref().map(|r| r.publishes.clone())
                        };
                        let mut resolving = Vec::new();
                        if let Some(cap) = cap_of(*a) {
                            resolving.push(OrderingDef::new(second.name.clone()).after(cap));
                        }
                        if let Some(cap) = cap_of(*b) {
                            resolving.push(OrderingDef::new(first.name.clone()).after(cap));
                        }
                        return Err(PlanError::RewriteTie {
                            active: rewrites
                                .iter()
                                .map(|idx| reg.stage(*idx).name.clone())
                                .collect(),
                            first: first.name.clone(),
                            second: second.name.clone(),
                            resolving,
                            workflow: request.workflow.clone(),
                        });
                    }
                }
            }
        }

        // The pipeline stages: dependency order, with the stages that have
        // effects outside the store placed after every stage that has none.
        let pipeline: Vec<StageIdx> =
            selected.scheduled.iter().copied().filter(|idx| !is_rewrite(*idx)).collect();
        let pipeline_preds: BTreeMap<StageIdx, BTreeSet<StageIdx>> =
            pipeline.iter().map(|idx| (*idx, preds[idx].clone())).collect();
        let topo = topo_order(reg, &pipeline, &pipeline_preds, &request.workflow)?;
        let is_io = |idx: StageIdx| reg.stage(idx).decl.effect == Effect::Io;
        for idx in &topo {
            if !is_io(*idx) {
                if let Some(io) = pipeline_preds[idx].iter().copied().find(|p| is_io(*p)) {
                    return Err(PlanError::IoProductReadByPure {
                        reader: reg.stage(*idx).name.clone(),
                        io_stage: reg.stage(io).name.clone(),
                        workflow: request.workflow.clone(),
                    });
                }
            }
        }
        let mut order: Vec<StageIdx> = topo.iter().copied().filter(|idx| !is_io(*idx)).collect();
        let pure_len = order.len();
        order.extend(topo.iter().copied().filter(|idx| is_io(*idx)));
        let batches = batch(&order, &pipeline_preds, pure_len);
        let first_io_batch = if pure_len < order.len() {
            batches.iter().position(|range| range.start == pure_len)
        } else {
            None
        };

        // Every backend that takes part must be able to render every element.
        for backend in &active_backends {
            let entry = reg.backend(backend).expect("an active backend is registered");
            let selected_rules = &selections[backend];
            let holes: BTreeSet<ElementName> =
                request.unsupported.get(backend).into_iter().flatten().cloned().collect();
            check_completeness(reg, entry, selected_rules, &holes, &request.workflow)?;
        }

        let mut warnings: Vec<PlanWarning> = Vec::new();
        for backend in &active_backends {
            warnings.extend(selection_warnings.remove(backend).into_iter().flatten());
        }
        warnings.extend(selected.warnings);

        let key = plan_key(reg, request, &demands);
        // Name order, so the plan's shape does not depend on the order features
        // registered in.
        let mut excluded: Vec<Excluded> = selected.excluded.into_values().collect();
        excluded.sort();
        Ok(Plan {
            registry: Arc::clone(&registry),
            request: request.clone(),
            demands,
            manifests,
            rewrites,
            order,
            batches,
            first_io_batch,
            selections,
            active_backends,
            needed: selected.needed,
            externals: selected.externals,
            key,
            warnings,
            excluded,
            commit_order: selected.commit_order,
            preds,
        })
    }

    /// Builds the plan of the registered workflow `name`, with `overrides`
    /// applied.
    ///
    /// This is the usual way to get a plan. A workflow ([`WorkflowDef`]) is a
    /// named set of demands together with the choices that go with them, held
    /// in the registry as a complete plan request, and a caller changes only
    /// what it needs to through [`Overrides`]. This
    /// method turns the workflow into a [`PlanRequest`] with
    /// [`workflow_request`], which documents exactly how, and builds that
    /// request with [`Plan::build`].
    ///
    /// # Errors
    ///
    /// Returns [`PlanError::UnknownWorkflow`] when no workflow named `name` is
    /// registered, [`PlanError::UnknownOutput`] when the workflow lists an
    /// output that is not registered, and any error [`Plan::build`] returns.
    ///
    /// # Panics
    ///
    /// As [`Plan::build`]: a panic in a registered stage's
    /// [`Stage::needs`](crate::core::stage::Stage::needs) or
    /// [`RewriteStage::needs`](crate::core::stage::RewriteStage::needs)
    /// propagates out of this call.
    pub fn for_workflow(
        registry: &Arc<FlmRegistry>,
        name: &str,
        overrides: &Overrides,
    ) -> Result<Plan, PlanError> {
        let request = workflow_request(registry, name, overrides)?;
        Plan::build(registry, &request)
    }

    /// The registry the plan was built against. Every stage, capability, table
    /// and rule number the plan holds is a number of this registry.
    pub fn registry(&self) -> &Arc<FlmRegistry> {
        &self.registry
    }

    /// The plan request the plan was built from; for a plan built with
    /// [`Plan::for_workflow`], the request the workflow was turned into, with the
    /// overrides applied.
    pub fn request(&self) -> &PlanRequest {
        &self.request
    }

    /// Every demanded capability, with collections replaced by their members, in
    /// name order.
    pub fn demands(&self) -> &BTreeSet<CapName> {
        &self.demands
    }

    /// The number that identifies the plan by everything it was built from: see
    /// [`PlanKey`].
    pub fn key(&self) -> PlanKey {
        self.key
    }

    /// The record the plan keeps about the stage `idx`, or `None` when `idx` is
    /// not a stage number of this plan's registry.
    ///
    /// Every registered stage has a record, whether or not it will run;
    /// [`Plan::is_scheduled`] says whether it will.
    pub fn manifest(&self, idx: StageIdx) -> Option<&StagePlan> {
        self.manifests.get(&idx)
    }

    /// The records the plan keeps about every registered stage, in registration
    /// order.
    pub fn manifests(&self) -> &BTreeMap<StageIdx, StagePlan> {
        &self.manifests
    }

    /// The rewrite stages that will run, in the order they run.
    pub fn rewrites(&self) -> &[StageIdx] {
        &self.rewrites
    }

    /// The names of the rewrite stages that will run, in the order they run.
    pub fn rewrite_names(&self) -> Vec<StageName> {
        self.rewrites.iter().map(|idx| self.registry.stage(*idx).name.clone()).collect()
    }

    /// The stages that will run after the rewrite stages, in the order they run.
    pub fn order(&self) -> &[StageIdx] {
        &self.order
    }

    /// The names of the stages that will run after the rewrite stages, in the
    /// order they run.
    pub fn stage_names(&self) -> Vec<StageName> {
        self.order.iter().map(|idx| self.registry.stage(*idx).name.clone()).collect()
    }

    /// Whether the stage `idx` will run, as a rewrite stage or as a later stage.
    pub fn is_scheduled(&self, idx: StageIdx) -> bool {
        self.rewrites.contains(&idx) || self.order.contains(&idx)
    }

    /// The names of every stage that will run, rewrite stages included, in name
    /// order.
    pub fn scheduled_names(&self) -> Vec<StageName> {
        let mut names: Vec<StageName> = self.rewrite_names();
        names.extend(self.stage_names());
        names.sort();
        names
    }

    /// The batches the runner executes, as ranges of positions in
    /// [`Plan::order`].
    ///
    /// No stage of a batch depends on another stage of the same batch, so the
    /// stages of a batch may run at the same time. See the [module
    /// documentation](self).
    pub fn batches(&self) -> &[Range<usize>] {
        &self.batches
    }

    /// The batches, each as the names of its stages in order.
    pub fn batch_names(&self) -> Vec<Vec<StageName>> {
        self.batches
            .iter()
            .map(|range| {
                self.order[range.clone()]
                    .iter()
                    .map(|idx| self.registry.stage(*idx).name.clone())
                    .collect()
            })
            .collect()
    }

    /// The index of the first batch that holds a stage with effect
    /// [`Effect::Io`] (a stage that acts outside the run's computed data, for
    /// example by writing the output), or `None` when the plan schedules no such
    /// stage.
    ///
    /// A run that has recorded an error diagnostic stops before this batch,
    /// unless the caller sets
    /// [`RunOptions::continue_with_errors`](crate::core::run::RunOptions::continue_with_errors).
    pub fn first_io_batch(&self) -> Option<usize> {
        self.first_io_batch
    }

    /// The lowering rules chosen for every registered backend whose rules could
    /// be chosen, whether or not that backend takes part in this plan.
    pub fn selections(&self) -> &BTreeMap<BackendName, SelectedRules> {
        &self.selections
    }

    /// The lowering rules chosen for `backend`, or `None` when `backend` is not
    /// registered or its rules could not be chosen (which makes the plan build
    /// fail only when that backend takes part).
    pub fn selection(&self, backend: &str) -> Option<&SelectedRules> {
        self.selections.get(backend)
    }

    /// The backends that take part in this plan, in name order: those whose
    /// lowering stage is scheduled.
    pub fn active_backends(&self) -> &BTreeSet<BackendName> {
        &self.active_backends
    }

    /// The names of every capability the plan needs, in name order: the demanded
    /// capabilities, together with every capability the scheduled stages
    /// require.
    pub fn needed_names(&self) -> Vec<CapName> {
        let mut names: Vec<CapName> =
            self.needed.iter().map(|id| self.registry.capability(*id).name.clone()).collect();
        names.sort();
        names
    }

    /// The external tables of this plan. An external table is a table that no
    /// stage of the run writes: the caller installs its value when the run is
    /// constructed ([`Externals`](crate::core::run::Externals)).
    pub fn externals(&self) -> &BTreeSet<TableId> {
        &self.externals
    }

    /// The names of the external tables of this plan, in the registry's table
    /// order.
    pub fn external_names(&self) -> Vec<CapName> {
        self.externals.iter().map(|id| self.registry.table(*id).def.name.clone()).collect()
    }

    /// The preferences this plan could not honor ([`PlanWarning`]): first the
    /// lowering rule preferences of the backends that take part, in backend name
    /// order, then the stage preferences.
    pub fn warnings(&self) -> &[PlanWarning] {
        &self.warnings
    }

    /// The stages that provider selection ruled out ([`Excluded`]), sorted by
    /// the name of the stage ruled out, then by the name of the stage that ruled
    /// it out.
    pub fn excluded(&self) -> &[Excluded] {
        &self.excluded
    }

    /// The names of the scheduled stages in the order in which provider
    /// selection chose them, which is not the order in which they run. It shows
    /// how the plan was built.
    pub fn commit_order(&self) -> Vec<StageName> {
        self.commit_order.iter().map(|idx| self.registry.stage(*idx).name.clone()).collect()
    }

    /// The scheduled stages that the stage `idx` depends on directly, or `None`
    /// when `idx` is not scheduled.
    ///
    /// The dependencies come from the stage's requirements, its reads, its
    /// ordering edges and the registry's ordering declarations. A rewrite stage's
    /// dependencies are rewrite stages only, and a later stage's dependencies are
    /// later stages only.
    pub fn predecessors(&self, idx: StageIdx) -> Option<&BTreeSet<StageIdx>> {
        self.preds.get(&idx)
    }

    /// The stages that run once per tree and that the stage `idx` depends on,
    /// directly or through other stages, in the order they run; `idx` itself is
    /// not among them, and neither is any stage with effect [`Effect::Io`].
    ///
    /// [Value catch-up](crate::guide::concepts_overview#value-catch-up) runs
    /// these stages over a synthesized value, so that the stage that renders the
    /// value finds the same kinds of computed data it would find for a tree of
    /// the document. A synthesized value is a small tree that a stage builds while
    /// a run is under way, held outside the document's trees
    /// ([synthesized values](crate::guide::concepts_overview#synthesized-values)).
    pub fn per_tree_closure(&self, idx: StageIdx) -> Vec<StageIdx> {
        let mut seen: BTreeSet<StageIdx> = BTreeSet::new();
        let mut stack = vec![idx];
        while let Some(node) = stack.pop() {
            if let Some(preds) = self.preds.get(&node) {
                for pred in preds {
                    if seen.insert(*pred) {
                        stack.push(*pred);
                    }
                }
            }
        }
        self.order
            .iter()
            .copied()
            .filter(|stage| seen.contains(stage))
            .filter(|stage| {
                let decl = &self.registry.stage(*stage).decl;
                decl.scope == Scope::PerTree && decl.effect != Effect::Io
            })
            .collect()
    }

    /// The plan as five lines of text: its rewrite stages, its order, the order
    /// in which provider selection chose the stages, the stages that were ruled
    /// out, and the external tables.
    ///
    /// The format is fixed and lists stage, capability and table names only, so a report
    /// can print it and a test can compare the text of two plans, for example to
    /// check that a plan does not depend on the order in which the features
    /// registered.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!(
            "rewrites: [{}]\n",
            join(self.rewrite_names().iter().map(|n| n.name()))
        ));
        out.push_str(&format!("order: [{}]\n", join(self.stage_names().iter().map(|n| n.name()))));
        out.push_str(&format!(
            "committed: [{}]\n",
            join(self.commit_order().iter().map(|n| n.name()))
        ));
        let excluded: Vec<String> = self
            .excluded
            .iter()
            .map(|e| format!("{} by {} over {}", e.stage, e.excluded_by, e.over))
            .collect();
        out.push_str(&format!("excluded: [{}]\n", excluded.join("; ")));
        out.push_str(&format!(
            "externals: [{}]",
            join(self.external_names().iter().map(|n| n.name()))
        ));
        out
    }
}

impl fmt::Debug for Plan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Plan")
            .field("rewrites", &self.rewrite_names())
            .field("order", &self.stage_names())
            .field("batches", &self.batch_names())
            .field("active_backends", &self.active_backends)
            .field("key", &self.key)
            .finish()
    }
}

/// Resolves one stage's declaration and needs against the registry into the
/// record the plan keeps for it.
///
/// Every name is looked up here. This function therefore refuses an undeclared
/// capability, table or option type, a table read as the wrong value type, a
/// rewrite stage declaring a read, a stage requiring a collection, and a stage
/// reading an option type whose value is fixed when the registry is built.
fn resolve_needs(
    reg: &FlmRegistry,
    idx: StageIdx,
    decl: &crate::core::stage::StageDecl,
    needs: Needs,
    workflow: &Option<WorkflowName>,
) -> Result<StagePlan, PlanError> {
    let entry = reg.stage(idx);
    let stage = entry.name.clone();
    let cap_roster = || -> Vec<Name> {
        let mut names: Vec<Name> =
            reg.capabilities().iter().map(|c| c.name.name().clone()).collect();
        names.sort();
        names
    };
    let table_roster = || -> Vec<Name> {
        let mut names: Vec<Name> = reg.tables().iter().map(|t| t.def.name.name().clone()).collect();
        names.sort();
        names
    };
    let resolve_cap = |name: &CapName, kind: &str| -> Result<CapId, PlanError> {
        if let Some(id) = reg.cap_id(name) {
            return Ok(id);
        }
        if reg.collection(name).is_some() && kind == "requires" {
            return Err(PlanError::CollectionRequired {
                stage: stage.clone(),
                collection: name.clone(),
                workflow: workflow.clone(),
            });
        }
        Err(PlanError::NeedsUndeclared {
            stage: stage.clone(),
            kind: "capability",
            name: name.name().clone(),
            roster: cap_roster(),
            workflow: workflow.clone(),
        })
    };
    let resolve_table = |table: &TableRef| -> Result<TableId, PlanError> {
        let Some(entry) = reg.table_named(&table.name) else {
            return Err(PlanError::NeedsUndeclared {
                stage: stage.clone(),
                kind: "table",
                name: table.name.name().clone(),
                roster: table_roster(),
                workflow: workflow.clone(),
            });
        };
        if entry.def.ty.type_id() != table.type_id {
            return Err(PlanError::NeedsTableType {
                stage: stage.clone(),
                table: table.name.clone(),
                declared: entry.def.ty.short_name(),
                referenced: table.short_type_name(),
                workflow: workflow.clone(),
            });
        }
        Ok(entry.id)
    };

    let mut requires: BTreeSet<CapId> = BTreeSet::new();
    for cap in &needs.requires {
        requires.insert(resolve_cap(cap, "requires")?);
    }
    let mut reads = ReadSet::new();
    for table in &needs.reads {
        if entry.is_rewrite() {
            return Err(PlanError::RewriteReads {
                stage: stage.clone(),
                table: table.name.clone(),
                workflow: workflow.clone(),
            });
        }
        let id = resolve_table(table)?;
        requires.insert(reg.table(id).cap);
        reads.insert(
            table.name.clone(),
            ReadEntry { id, ty: reg.table(id).def.ty, optional: false, producer: None },
        );
    }
    for table in &needs.reads_opt {
        if entry.is_rewrite() {
            return Err(PlanError::RewriteReads {
                stage: stage.clone(),
                table: table.name.clone(),
                workflow: workflow.clone(),
            });
        }
        let id = resolve_table(table)?;
        reads.insert(
            table.name.clone(),
            ReadEntry { id, ty: reg.table(id).def.ty, optional: true, producer: None },
        );
    }
    let mut writes = WriteSet::new();
    for table in &decl.writes {
        let id = resolve_table(table)?;
        writes.insert(table.name.clone(), WriteEntry { id, ty: reg.table(id).def.ty });
    }
    for option in &needs.options {
        match reg.option_decl(option.key()) {
            None => {
                let mut roster: Vec<Name> =
                    reg.options().keys().map(|k| Name::from(k.short_name())).collect();
                roster.sort();
                return Err(PlanError::NeedsUndeclared {
                    stage: stage.clone(),
                    kind: "option type",
                    name: Name::from(option.key().short_name()),
                    roster,
                    workflow: workflow.clone(),
                });
            }
            Some(decl) if decl.bind == crate::model::options::Bind::Registry => {
                return Err(PlanError::RegistryBoundRead {
                    error: OptionError::RegistryBoundRead {
                        stage: stage.clone(),
                        option: option.key().short_name().to_string(),
                        owner: decl.owner.clone(),
                    },
                    workflow: workflow.clone(),
                });
            }
            Some(_) => {}
        }
    }
    let mut after: BTreeSet<CapId> = BTreeSet::new();
    for cap in &needs.after {
        after.insert(resolve_cap(cap, "after")?);
    }
    let mut before: BTreeSet<CapId> = BTreeSet::new();
    for cap in &needs.before {
        before.insert(resolve_cap(cap, "before")?);
    }
    let options = needs.options.clone();
    Ok(StagePlan {
        idx,
        manifest: Arc::new(Manifest { decl: decl.clone(), needs }),
        reads,
        writes,
        options,
        requires,
        after,
        before,
    })
}

/// Sorts `stages` so that every stage follows the stages it depends on,
/// breaking ties by registration order so that the result depends on nothing
/// else. The registry build sorts features with the same function
/// ([`crate::core::registry::order`]).
///
/// `preds` gives, for each stage, the stages it depends on. Returns
/// [`PlanError::Cycle`] when the dependencies form a cycle, listing the cycle
/// from the stage whose name comes first and repeating that stage at the end.
fn topo_order(
    reg: &FlmRegistry,
    stages: &[StageIdx],
    preds: &BTreeMap<StageIdx, BTreeSet<StageIdx>>,
    workflow: &Option<WorkflowName>,
) -> Result<Vec<StageIdx>, PlanError> {
    let cycle = match sort_by_priority(stages, preds) {
        Ok(order) => return Ok(order),
        Err(cycle) => cycle,
    };
    let names: Vec<StageName> = cycle.iter().map(|idx| reg.stage(*idx).name.clone()).collect();
    let least =
        names.iter().enumerate().min_by(|(_, a), (_, b)| a.cmp(b)).map(|(i, _)| i).unwrap_or(0);
    let mut rotated = names;
    rotated.rotate_left(least);
    let first = rotated[0].clone();
    rotated.push(first);
    Err(PlanError::Cycle { stages: rotated, workflow: workflow.clone() })
}

/// Cuts an ordered list of stages into batches: a stage joins the current batch
/// when it depends on no stage already in it, and starts a new batch otherwise.
///
/// A batch always ends at `pure_len`, the position where the stages with
/// effect [`Effect::Io`] begin, so that no batch holds both kinds and a run
/// that stops before the first `Io` stage stops at a batch boundary.
fn batch(
    order: &[StageIdx],
    preds: &BTreeMap<StageIdx, BTreeSet<StageIdx>>,
    pure_len: usize,
) -> Vec<Range<usize>> {
    let mut batches: Vec<Range<usize>> = Vec::new();
    let mut start = 0usize;
    for (pos, stage) in order.iter().copied().enumerate() {
        let independent = order[start..pos].iter().all(|other| !preds[&stage].contains(other));
        if pos > start && (!independent || pos == pure_len) {
            batches.push(start..pos);
            start = pos;
        }
    }
    if start < order.len() {
        batches.push(start..order.len());
    }
    batches
}

/// Checks that `backend` can render every element a document may contain.
///
/// The elements checked are those a document may contain
/// ([`FlmRegistry::occurrable_elements`]). An element passes if the backend
/// writes it natively and no preferred rule replaces that, or if the rule
/// chosen for it produces only elements that pass in turn. Every element must
/// pass except the elements of `holes`, the elements the plan request accepts as
/// unrenderable on this backend; an element of `holes` that a rule produces
/// still does not pass.
///
/// Returns [`PlanError::Incomplete`] listing the elements that do not pass,
/// and [`PlanError::ProducesCycle`] when following what the rules produce leads
/// back to an element already on the path.
fn check_completeness(
    reg: &FlmRegistry,
    backend: &crate::core::registry::frozen::BackendEntry,
    selected: &SelectedRules,
    holes: &BTreeSet<ElementName>,
    workflow: &Option<WorkflowName>,
) -> Result<(), PlanError> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum State {
        Visiting,
        Reachable,
        Unreachable,
    }
    let mut states: BTreeMap<ElementName, State> = BTreeMap::new();
    fn reach(
        reg: &FlmRegistry,
        backend: &crate::core::registry::frozen::BackendEntry,
        selected: &SelectedRules,
        element: &ElementName,
        states: &mut BTreeMap<ElementName, State>,
        path: &mut Vec<ElementName>,
    ) -> Result<bool, Vec<ElementName>> {
        if let Some(state) = states.get(element) {
            return match state {
                State::Reachable => Ok(true),
                State::Unreachable => Ok(false),
                State::Visiting => {
                    let start = path.iter().position(|e| e == element).unwrap_or(0);
                    Err(path[start..].to_vec())
                }
            };
        }
        if backend.native.contains(element) && !selected.overridden_native.contains(element) {
            states.insert(element.clone(), State::Reachable);
            return Ok(true);
        }
        let Some(rule) = selected.rule_for(element) else {
            states.insert(element.clone(), State::Unreachable);
            return Ok(false);
        };
        states.insert(element.clone(), State::Visiting);
        path.push(element.clone());
        let mut all = true;
        for produced in &reg.rule(rule).rule.decl.produces {
            if !reach(reg, backend, selected, produced, states, path)? {
                all = false;
            }
        }
        path.pop();
        states.insert(element.clone(), if all { State::Reachable } else { State::Unreachable });
        Ok(all)
    }
    let mut missing: Vec<ElementName> = Vec::new();
    for element in reg.occurrable_elements() {
        if holes.contains(element) {
            continue;
        }
        let mut path = Vec::new();
        match reach(reg, backend, selected, element, &mut states, &mut path) {
            Ok(true) => {}
            Ok(false) => missing.push(element.clone()),
            Err(cycle) => {
                let least = cycle
                    .iter()
                    .enumerate()
                    .min_by(|(_, a), (_, b)| a.cmp(b))
                    .map(|(i, _)| i)
                    .unwrap_or(0);
                let mut rotated = cycle;
                rotated.rotate_left(least);
                return Err(PlanError::ProducesCycle {
                    backend: backend.name.clone(),
                    cycle: rotated,
                    workflow: workflow.clone(),
                });
            }
        }
    }
    if missing.is_empty() {
        Ok(())
    } else {
        Err(PlanError::Incomplete { backend: backend.name.clone(), missing, workflow: workflow.clone() })
    }
}

/// Computes the plan's key: a digest over the registry's own digest together
/// with exactly what plan building consults (see [`PlanKey`]).
fn plan_key(reg: &FlmRegistry, request: &PlanRequest, demands: &BTreeSet<CapName>) -> PlanKey {
    let mut hasher = Fnv1a64::new();
    hasher.write_u64(reg.digest());
    for demand in demands {
        hasher.write_str(demand.as_str());
    }
    hasher.write_str("|prefer");
    for stage in &request.prefer {
        hasher.write_str(stage.as_str());
    }
    hasher.write_str("|lowering");
    for (backend, selection) in &request.lowering {
        hasher.write_str(backend.as_str());
        for rule in &selection.prefer {
            hasher.write_str(rule.as_str());
        }
    }
    hasher.write_str("|unsupported");
    for (backend, holes) in &request.unsupported {
        hasher.write_str(backend.as_str());
        for element in holes {
            hasher.write_str(element.as_str());
        }
    }
    hasher.write_str("|options");
    hasher.write_u64(request.render_options.digest());
    PlanKey(hasher.value())
}

/// Turns the registered workflow `name` into a plan request, with `overrides`
/// applied, without building the plan.
///
/// A workflow ([`WorkflowDef`]) is a named set of demands together with the
/// choices that go with them, held in the registry as a complete plan request.
/// [`Plan::for_workflow`] calls this function and builds the result;
/// call it directly to inspect or adjust the request before building it with
/// [`Plan::build`].
///
/// A workflow may extend another. The request is then built from the fields
/// combined along the chain of workflows it extends
/// ([`FlmRegistry::combined_workflow`]): each field a workflow declares replaces
/// the one it inherits. The request's demands are, in this order, the
/// capability of each output the workflow lists
/// ([`OutputDef`](crate::core::registry::OutputDef)), the workflow's own
/// demands, and the demands of `overrides`. [`Overrides`] documents how each of
/// its other fields combines with the workflow's. The request's
/// [`workflow`](PlanRequest#structfield.workflow) is `name`, so that the errors
/// and warnings of the plan name the workflow, including for what `overrides`
/// adds.
///
/// # Errors
///
/// Returns [`PlanError::UnknownWorkflow`] when no workflow named `name` is
/// registered, and [`PlanError::UnknownOutput`] when the workflow lists an
/// output that is not registered. The other names of the request are checked
/// only when the plan is built.
pub fn workflow_request(
    registry: &Arc<FlmRegistry>,
    name: &str,
    overrides: &Overrides,
) -> Result<PlanRequest, PlanError> {
    let reg: &FlmRegistry = registry;
    let workflow_roster =
        || -> Vec<Name> { reg.workflows().map(|w| w.def.name.name().clone()).collect() };
    let Some(combined) = reg.combined_workflow(name) else {
        return Err(PlanError::UnknownWorkflow {
            workflow: WorkflowName::from(name),
            roster: workflow_roster(),
        });
    };
    let WorkflowDef { outputs, demands, prefer, lowering, unsupported, render_options, .. } = combined;
    let output_roster =
        || -> Vec<Name> { reg.outputs().map(|o| o.def.name.name().clone()).collect() };
    let mut request = PlanRequest::new();
    request.workflow = Some(WorkflowName::from(name));
    for output in &outputs {
        let Some(output_entry) = reg.output(output) else {
            return Err(PlanError::UnknownOutput {
                workflow: WorkflowName::from(name),
                output: output.clone(),
                roster: output_roster(),
            });
        };
        request.demands.push(output_entry.def.capability.clone());
    }
    request.demands.extend(demands);
    request.demands.extend(overrides.demands.iter().cloned());
    request.prefer.extend(overrides.prefer.iter().cloned());
    request.prefer.extend(prefer);
    request.lowering = lowering;
    for (backend, selection) in &overrides.lowering {
        request.lowering.insert(backend.clone(), selection.clone());
    }
    request.unsupported = unsupported;
    for (backend, holes) in &overrides.unsupported {
        request.unsupported.entry(backend.clone()).or_default().extend(holes.iter().cloned());
    }
    // Render-time values: the workflow's, with the host's replacing them one
    // option type at a time.
    let mut merged = render_options;
    merged.merge_from(&overrides.render_options);
    request.render_options = merged;
    Ok(request)
}
