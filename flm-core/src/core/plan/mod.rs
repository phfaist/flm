//! Building a plan: working out which stages run, and in which order, from what a
//! caller asks for.
//!
//! A plan is the list of stages that satisfies a plan request, in dependency order
//! and grouped in batches; it is a pure value of the registry and the request, with
//! nothing of any document in it ([`Plan`]). One plan therefore serves every
//! document rendered the same way, and [`crate::core::run`] runs it over one
//! document at a time.
//!
//! A caller never lists stages itself. It *demands* capabilities (asks the plan for
//! stages that provide them), usually by choosing a workflow, a named set of demands
//! together with the choices that go with them
//! ([`WorkflowDef`](crate::core::registry::WorkflowDef)). A capability is a
//! registered dotted name for a state of processing that a stage brings about
//! ([`CapabilityDef`](crate::core::registry::CapabilityDef)). This module derives
//! the rest from what the registered stages declare: which stage provides each
//! needed capability, which lowering rule
//! ([`LoweringRule`](crate::core::lowering::LoweringRule)) lowers each element on
//! each backend ([`BackendDef`](crate::core::registry::BackendDef)), and
//! the order of the chosen stages. The guide sections [stages, capabilities, and
//! plans](crate::guide::concepts_overview#stages-capabilities-and-plans) and
//! [how a plan is built](crate::guide::pipeline#how-a-plan-is-built) describe the
//! whole picture.
//!
//! # Getting a plan
//!
//! - [`Plan::for_workflow`] builds the plan of a registered workflow, with
//!   [`Overrides`] applied: the usual way.
//! - [`workflow_request`] turns a workflow into a [`PlanRequest`] without building
//!   it, for a caller that inspects or adjusts the request first.
//! - [`Plan::build`] builds the plan of any [`PlanRequest`].
//!
//! Each returns a [`PlanError`] when the plan cannot be built. A preference the plan
//! cannot honor does not stop the build: it is kept as a [`PlanWarning`].
//!
//! # Module map
//!
//! - [`request`] — [`PlanRequest`], everything a plan is built from;
//!   [`Overrides`], what a caller changes about a workflow's request; [`PlanKey`],
//!   the number that identifies a plan.
//! - [`cx`] — [`PlanCx`], what a stage can consult while it states what it needs
//!   for one plan.
//! - [`selection`] — how one lowering rule is chosen per element and backend
//!   ([`SelectedRules`]) and how one stage is chosen to provide each needed
//!   capability ([`Excluded`] records a stage that this choice ruled out).
//! - [`build`] — [`Plan`] and how it is built and checked, with [`StagePlan`],
//!   what a plan records about one stage, [`PlanError`] and [`PlanWarning`].
//!
//! [`LoweringSelection`], the lowering rules a request prefers for one backend, is
//! defined in the registry module and re-exported here.
//!
//! # Examples
//!
//! Building the plan of a registered workflow, with nothing changed about it:
//!
//! ```
//! use flm_core::core::plan::{Overrides, Plan};
//! use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
//! use flm_core::stdcore::{self, StdcoreConfig};
//!
//! let mut builder = RegistryBuilder::new();
//! builder.use_package(stdcore::package(StdcoreConfig::new()));
//! builder.use_package(flm_core::math_passthrough::package());
//! let registry = builder.build(&RegistryInputs::new())?;
//!
//! let plan = Plan::for_workflow(&registry, "text", &Overrides::none())?;
//!
//! // The plan lists the stages that run after the rewrite stages, in the order
//! // the runner runs them.
//! assert!(plan.stage_names().iter().any(|name| name.as_str() == "flm.text.emit"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub mod build;
pub mod cx;
pub mod request;
pub mod selection;

pub use crate::core::registry::entities::LoweringSelection;
pub use build::{Plan, PlanError, PlanWarning, StagePlan, workflow_request};
pub use cx::PlanCx;
pub use request::{Overrides, PlanKey, PlanRequest};
pub use selection::{Excluded, SelectedRules};
