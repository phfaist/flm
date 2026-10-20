//! The view of a plan request that a stage consults while it states what it needs
//! for one plan: [`PlanCx`].
//!
//! While it builds a plan, [`Plan::build`](crate::core::plan::Plan::build) asks
//! every registered stage what it needs for that plan, through
//! [`Stage::needs`](crate::core::stage::Stage::needs) or
//! [`RewriteStage::needs`](crate::core::stage::RewriteStage::needs), and hands it a
//! [`PlanCx`]. A stage whose needs depend on the plan request (for example on the
//! lowering rules chosen for a backend, or on whether another feature is
//! registered) works them out from it. See [`crate::core::plan`] for the whole
//! process.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use crate::model::names::{BackendName, CapName, ElementName, RuleName, StageName};
use crate::model::options::Options;
use crate::core::plan::request::PlanRequest;
use crate::core::plan::selection::SelectedRules;
use crate::core::registry::frozen::FlmRegistry;

/// The view of a plan request that a stage consults while it states what it
/// needs, in [`Stage::needs`](crate::core::stage::Stage::needs) or
/// [`RewriteStage::needs`](crate::core::stage::RewriteStage::needs).
///
/// It gives the registry the plan is built against, for asking what exists; the
/// parts of the plan request ([`PlanRequest`]): the demands with collections
/// replaced by their members, the preferred stages, the preferred lowering rules,
/// the accepted unrenderable elements and the option values of the render
/// request; and the lowering rules chosen for each backend ([`SelectedRules`]).
/// Only
/// [`Plan::build`](crate::core::plan::Plan::build) creates one.
///
/// Two kinds of information are absent, each for a reason:
///
/// - **Nothing about any document**, including the option values a document
///   sets. A plan is built once and run over many documents; needs that varied
///   with the document would be wrong for all but one of them.
/// - **Nothing about which stages provide the needed capabilities.** The plan
///   chooses those stages from the needs the stages state, so a stage whose needs
///   depended on that choice would make the choice circular. The lowering rules
///   chosen for each backend are available, because the plan chooses them before
///   it asks any stage for its needs.
///
/// Everything available here is part of what the plan's key is computed over
/// ([`PlanKey`](crate::core::plan::PlanKey)). Two plan requests with the same key
/// against the same registry therefore give every stage the same view.
pub struct PlanCx<'a> {
    registry: &'a FlmRegistry,
    request: &'a PlanRequest,
    demands: &'a BTreeSet<CapName>,
    selections: &'a BTreeMap<BackendName, SelectedRules>,
}

impl<'a> PlanCx<'a> {
    pub(crate) fn new(
        registry: &'a FlmRegistry,
        request: &'a PlanRequest,
        demands: &'a BTreeSet<CapName>,
        selections: &'a BTreeMap<BackendName, SelectedRules>,
    ) -> PlanCx<'a> {
        PlanCx { registry, request, demands, selections }
    }

    /// The registry the plan is being built against, for asking which features,
    /// backends, rules or tables exist.
    pub fn registry(&self) -> &'a FlmRegistry {
        self.registry
    }

    /// Whether a feature named `name` is registered.
    ///
    /// A stage uses this to make its needs depend on the other features of the
    /// registry, for example to read another feature's table only when that
    /// feature is registered.
    pub fn has_feature(&self, name: &str) -> bool {
        self.registry.has_feature(name)
    }

    /// Every demanded capability, with collections replaced by their members, in
    /// name order.
    pub fn demands(&self) -> &'a BTreeSet<CapName> {
        self.demands
    }

    /// Whether `capability` is demanded, by its own name or as a member of a
    /// demanded collection.
    pub fn demands_include(&self, capability: &str) -> bool {
        self.demands.contains(capability)
    }

    /// The stages the plan request prefers, most preferred first.
    pub fn prefer(&self) -> &'a [StageName] {
        &self.request.prefer
    }

    /// The lowering rules chosen for `backend`, one per element.
    ///
    /// A backend's lowering stage reads this to work out its own needs, which are
    /// the union of what the chosen rules declare. Returns `None` when `backend`
    /// is not registered, or when the rules of `backend` could not be chosen; the
    /// plan reports that failure only if the backend takes part in the plan.
    pub fn lowering_selection(&self, backend: &str) -> Option<&'a SelectedRules> {
        self.selections.get(backend)
    }

    /// The lowering rules the plan request prefers for `backend`, most preferred
    /// first; empty when it prefers none.
    pub fn lowering_preferences(&self, backend: &str) -> &'a [RuleName] {
        self.request.lowering.get(backend).map(|s| s.prefer.as_slice()).unwrap_or(&[])
    }

    /// The accepted unrenderable elements of `backend`: the elements the plan
    /// request lists as ones `backend` need not render; empty when it lists none.
    /// See
    /// [`PlanRequest::unsupported`](crate::core::plan::PlanRequest#structfield.unsupported).
    pub fn unsupported(&self, backend: &str) -> &'a [ElementName] {
        self.request.unsupported.get(backend).map(Vec::as_slice).unwrap_or(&[])
    }

    /// The value the render request sets for the option type `T`, or `None` when
    /// it sets none.
    ///
    /// Only the value set on the render request is returned. A value that a
    /// document sets for an option type is never returned here, since no
    /// document exists while the plan is built; what a stage needs therefore
    /// never varies with the document the plan runs over.
    pub fn render_option<T: Options>(&self) -> Option<&'a T> {
        self.request.render_options.get::<T>()
    }
}

impl fmt::Debug for PlanCx<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PlanCx")
            .field("demands", self.demands)
            .field("prefer", &self.request.prefer)
            .field("selections", &self.selections.keys().collect::<Vec<_>>())
            .finish()
    }
}
