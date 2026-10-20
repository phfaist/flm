//! The input of plan building: the plan request, the changes a caller makes to a
//! workflow's request, and the number that identifies a plan.
//!
//! - [`PlanRequest`] is everything a caller says about what it wants from a plan;
//!   [`Plan::build`](crate::core::plan::Plan::build) builds the plan that satisfies
//!   it.
//! - [`Overrides`] are the changes a caller makes to the request of a registered
//!   workflow. Most callers never write a plan request: they choose a workflow by
//!   name and let [`Plan::for_workflow`](crate::core::plan::Plan::for_workflow)
//!   build its request, with these changes applied.
//! - [`PlanKey`] identifies a plan by everything it was built from, so that a
//!   built plan can be cached and reused.
//!
//! A plan request holds nothing about any document. One plan therefore serves
//! every document rendered the same way. See [`crate::core::plan`] for the whole
//! process.
use std::collections::BTreeMap;
use std::fmt;

use crate::model::names::{BackendName, CapName, ElementName, StageName, WorkflowName};
use crate::model::options::OptionValues;
use crate::core::registry::entities::LoweringSelection;

/// A plan request: everything a caller says about what it wants from a plan.
///
/// To *demand* a capability is to ask the plan for a stage that provides it; the
/// demands of a plan request are the capabilities it asks for, and a workflow is a
/// named set of demands together with the choices that go with them
/// ([`WorkflowDef`](crate::core::registry::WorkflowDef)). A capability is a
/// registered dotted name for a state of processing that a stage brings about
/// ([`CapabilityDef`](crate::core::registry::CapabilityDef)).
///
/// The request has five parts. The demands say what is wanted; the other four
/// fields decide how: which stage provides a capability several stages
/// provide, which lowering rule lowers an element several rules lower, which
/// elements a backend need not render, and which option values the render
/// request sets. These five fields, together with the registry,
/// decide the whole plan. The same fields are therefore what a stage can consult
/// while it states its needs ([`PlanCx`](crate::core::plan::PlanCx)), and what the
/// [`PlanKey`] is computed over.
///
/// The request also records the workflow it was made from, if any
/// ([`workflow`](PlanRequest#structfield.workflow)). Plan building does not
/// consult that name; the errors and warnings of the plan name it, so that a
/// person reading them knows which workflow's request they are about.
///
/// Create a request with [`PlanRequest::new`] and add to it with the methods that
/// take and return it, or set the public fields directly. Build the plan with
/// [`Plan::build`](crate::core::plan::Plan::build), which checks that every name
/// in the request is registered. To start from a registered workflow instead, use
/// [`workflow_request`](crate::core::plan::workflow_request) or
/// [`Plan::for_workflow`](crate::core::plan::Plan::for_workflow).
///
/// # Examples
///
/// ```
/// use flm_core::core::plan::PlanRequest;
///
/// let request = PlanRequest::new()
///     .demand("flm.validated")
///     .prefer("flm.blocks.structure");
///
/// assert_eq!(request.demands.len(), 1);
/// ```
#[derive(Debug, Clone, Default)]
pub struct PlanRequest {
    /// The demanded capabilities. A name here may also be a collection, a name
    /// for a set of capabilities
    /// ([`CollectionDef`](crate::core::registry::CollectionDef)): building the
    /// plan replaces it with its members.
    pub demands: Vec<CapName>,
    /// The stages to schedule where several stages could provide one
    /// capability, most preferred first. A preference for a stage that provides
    /// nothing the plan needs schedules nothing. A preference for a stage that the
    /// plan had to rule out becomes a
    /// [`PlanWarning::IgnoredPreference`](crate::core::plan::PlanWarning::IgnoredPreference).
    /// See [`selection`](crate::core::plan::selection) for how a provider is
    /// chosen.
    pub prefer: Vec<StageName>,
    /// The lowering rules to use where several rules could lower one element,
    /// per backend, each list most preferred first. See
    /// [`SelectedRules`](crate::core::plan::SelectedRules) for how a rule is
    /// chosen.
    pub lowering: BTreeMap<BackendName, LoweringSelection>,
    /// The accepted unrenderable elements, per backend.
    ///
    /// An accepted unrenderable element is an element the plan request lists, per
    /// backend, as one the backend need not render: the plan builds anyway, and
    /// each occurrence lowers to a childless `flm.pres.unsupported` node with one
    /// Error diagnostic. Without this list, a backend that cannot render an element
    /// a document may contain makes the plan build fail
    /// ([`PlanError::Incomplete`](crate::core::plan::PlanError::Incomplete)).
    pub unsupported: BTreeMap<BackendName, Vec<ElementName>>,
    /// The option values set on the render request. An option type is a plain
    /// Rust struct a feature declares once, with a binding that says where its
    /// values may be set ([`crate::model::options`]). A value here for an option
    /// type whose binding does not admit the render request makes the plan build
    /// fail ([`PlanError::RenderOption`](crate::core::plan::PlanError::RenderOption)).
    pub render_options: OptionValues,
    /// The workflow the request was made from, or `None` for a request a
    /// program builds by hand. [`workflow_request`](crate::core::plan::workflow_request)
    /// sets it. Plan building does not consult it: the errors and warnings of
    /// the plan name the workflow in their messages, and the [`PlanKey`] leaves
    /// it out, so that two workflows that produce the same request share a plan.
    pub workflow: Option<WorkflowName>,
}

impl PlanRequest {
    /// Creates a plan request that demands nothing and decides nothing.
    pub fn new() -> PlanRequest {
        PlanRequest::default()
    }

    /// Adds `capability` to the demands, after the ones already there, and returns
    /// the request. `capability` may also be the name of a collection, which
    /// demands each of its members.
    pub fn demand(mut self, capability: impl Into<CapName>) -> PlanRequest {
        self.demands.push(capability.into());
        self
    }

    /// Adds `stage` to the preferred stages, after the ones already there, and
    /// returns the request.
    pub fn prefer(mut self, stage: impl Into<StageName>) -> PlanRequest {
        self.prefer.push(stage.into());
        self
    }

    /// Sets the lowering rules preferred on `backend`, replacing any selection
    /// already set for that backend, and returns the request.
    pub fn lowering(
        mut self,
        backend: impl Into<BackendName>,
        selection: LoweringSelection,
    ) -> PlanRequest {
        self.lowering.insert(backend.into(), selection);
        self
    }

    /// Adds `element` to the accepted unrenderable elements of `backend` and
    /// returns the request: the plan builds even though `backend` cannot render
    /// `element`. See the [`unsupported`](PlanRequest#structfield.unsupported)
    /// field.
    pub fn unsupported(
        mut self,
        backend: impl Into<BackendName>,
        element: impl Into<ElementName>,
    ) -> PlanRequest {
        self.unsupported.entry(backend.into()).or_default().push(element.into());
        self
    }

    /// Sets the option values of the render request, replacing any already set,
    /// and returns the request.
    pub fn render_options(mut self, options: OptionValues) -> PlanRequest {
        self.render_options = options;
        self
    }
}

/// The changes a caller makes to the plan request of a registered workflow.
///
/// A workflow ([`WorkflowDef`](crate::core::registry::WorkflowDef)) is a named
/// set of demands together with the choices that go with them, held in the
/// registry as a complete plan request.
/// [`workflow_request`](crate::core::plan::workflow_request) turns it into a
/// [`PlanRequest`] and then applies these changes, and
/// [`Plan::for_workflow`](crate::core::plan::Plan::for_workflow) builds the result.
/// Each field combines with the workflow's own value as its documentation below
/// says, so that a caller adds to a workflow without restating it.
///
/// [`Overrides::none`] changes nothing.
#[derive(Debug, Clone, Default)]
pub struct Overrides {
    /// Capabilities demanded in addition to the workflow's demands.
    pub demands: Vec<CapName>,
    /// Stages preferred before the ones the workflow prefers.
    pub prefer: Vec<StageName>,
    /// Lowering selections, per backend. The selection given here for a backend
    /// replaces the workflow's whole selection for that backend; the workflow's
    /// selections for other backends stay.
    pub lowering: BTreeMap<BackendName, LoweringSelection>,
    /// Accepted unrenderable elements, per backend, added to the ones the
    /// workflow already accepts.
    pub unsupported: BTreeMap<BackendName, Vec<ElementName>>,
    /// Option values for the render request, applied one option type at a
    /// time: a value given here for an option type replaces the workflow's value
    /// for that type, and every other option type keeps the workflow's value.
    pub render_options: OptionValues,
}

impl Overrides {
    /// Creates overrides that change nothing about the workflow's request.
    pub fn none() -> Overrides {
        Overrides::default()
    }
}

/// The number that identifies a plan, computed over everything the plan was built
/// from.
///
/// The key is a digest of the registry's own digest, the demands (with
/// collections replaced by their members), the preferred stages, the lowering
/// selections, the accepted unrenderable elements and the option values of the
/// render request: everything that plan building consults, and nothing else. The
/// name of the workflow the request was made from is left out, since plan
/// building does not consult it: two workflows that produce the same request
/// share a plan. Two
/// plan requests with the same key against the same registry therefore build the
/// same plan, and a caller can cache a built plan under its key. Read a plan's key
/// with [`Plan::key`](crate::core::plan::Plan::key).
///
/// The key is displayed as sixteen hexadecimal digits.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlanKey(
    /// The digest. Only comparison between keys is meaningful; the value itself
    /// carries no other information.
    pub u64,
);

impl fmt::Debug for PlanKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "PlanKey({:016x})", self.0)
    }
}

impl fmt::Display for PlanKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:016x}", self.0)
    }
}
