//! The two choices made while a plan is built: provider selection, which chooses
//! the stage that provides each needed capability, and lowering rule selection,
//! which chooses the lowering rule for each element on each backend.
//!
//! Both choices are made before anything runs, from what the registry holds and
//! what the plan request says; see [`crate::core::plan`] for the whole process.
//! The public results are [`SelectedRules`], the rules chosen for one backend,
//! and [`Excluded`], a stage that provider selection ruled out. Every choice is
//! made in name order, so the outcome and every message are the same regardless
//! of the order in which the features registered.
//!
//! # Lowering rule selection
//!
//! Lowering rule selection chooses one rule per element and backend in a fixed
//! order: the rule the request prefers by name, else the backend's native
//! emission, else a rule marked default on that backend, else a rule marked
//! default everywhere, else the sole candidate. A lowering rule is a named
//! registry entity written for one element, which stages (builds into the lowered
//! tree) the element's presentation for one backend
//! ([`LoweringRule`](crate::core::lowering::LoweringRule)); its
//! [`DefaultMark`] says when it is the rule to choose.
//!
//! The candidates on a backend are the registered rules for the element, except
//! two kinds, which are never candidates, not even as the only rule for their
//! element and not even when the request prefers them by name:
//!
//! - a rule that is set aside (excluded from rule selection; see
//!   [`PeerName`](crate::core::registry::PeerName)): a peer declaration the rule
//!   depends on (a backend it is marked as the default on, an element it produces
//!   or a table it reads other than as an optional read, which the rule's feature
//!   expects another feature to declare) resolved to nothing
//!   ([`RuleEntry::set_aside`](crate::core::registry::RuleEntry::set_aside));
//! - a rule marked as the default on other backends only
//!   ([`DefaultMark::OnBackends`] without this backend). This mark says where the
//!   rule belongs, not only where it ranks first, because such a rule builds
//!   elements that only the backend it was written for writes natively. A rule marked
//!   [`DefaultMark::Never`] remains a candidate on every backend: that mark
//!   restricts the rule's rank, not where it may be used.
//!
//! Among the candidates, the rule is, in this order:
//!
//! 1. the first candidate the request prefers by name for this backend, even when
//!    the backend would otherwise write the element natively;
//! 2. otherwise no rule at all, when the backend writes the element natively;
//! 3. otherwise the rule marked as the default on this backend
//!    ([`DefaultMark::OnBackends`]);
//! 4. otherwise the rule marked as the default everywhere
//!    ([`DefaultMark::Everywhere`]);
//! 5. otherwise the only candidate, when there is exactly one.
//!
//! When several candidates remain and none of these decides (for example two
//! rules marked as the default on this backend), the plan build fails with
//! [`PlanError::RuleContest`]. An element with no candidate gets no rule; the
//! plan's completeness check then requires that the backend writes the element
//! natively or that the plan request accepts it as unrenderable
//! ([`PlanError::Incomplete`] otherwise). An element whose only rule is marked
//! for one backend therefore makes the plan fail on every other backend that
//! takes part in the plan and does not write the element natively, unless the
//! plan request accepts the element as unrenderable there.
//!
//! A preferred rule that cannot be used, because it is set aside or because it
//! is not a candidate on the backend, becomes a [`PlanWarning`], which the
//! runner reports as a Warning diagnostic at the start of the run.
//!
//! # Provider selection
//!
//! Several stages may provide one capability, and the plan schedules exactly one
//! of them. Provider selection chooses one stage for each capability the plan
//! needs: the forced choices are made first and followed to a fixed point, then
//! one tie is broken by the request's preferences or a default refinement, and
//! the forced choices are followed again, until nothing is left to decide. In
//! detail:
//!
//! 1. The needed capabilities start as the demanded ones. A **candidate** of a
//!    capability is a stage that provides it, directly or by providing a
//!    capability that refines it ([refinement](crate::guide::pipeline#refinement)),
//!    and that has not been ruled out.
//! 2. The forced choices are followed to a fixed point. A needed capability with
//!    exactly one candidate schedules that stage. The stage's required
//!    capabilities become needed too, and **every other stage that provides any of
//!    the same capabilities is ruled out**. A stage is therefore indivisible:
//!    scheduling it commits the plan to everything it provides, so no second
//!    stage may provide any part of that.
//! 3. A needed capability whose candidates have all been ruled out makes the
//!    plan build fail with [`PlanError::Unsatisfiable`], which lists each provider
//!    and the stage that ruled it out. A capability of an external table that
//!    no registered stage provides is the exception: no stage writes such a
//!    table, and the caller installs its value when the run is constructed
//!    ([`Externals`](crate::core::run::Externals)).
//! 4. When no choice is forced, the remaining ties are decided one at a time. The
//!    first stage in the request's preferences that is a candidate of a contested
//!    capability is scheduled. A preferred stage that is already ruled out is
//!    skipped and reported as [`PlanWarning::IgnoredPreference`]. Without such a
//!    preference, the first contested capability in name order is decided by its
//!    default refinement (the capability that refines it and is marked as the one
//!    a demand for it selects), following the chain of defaults down while the
//!    default itself has several candidates. A tie that neither decides makes the plan build fail
//!    with [`PlanError::Contested`], which lists the candidates. After each
//!    choice, selection resumes at item 2.
//!
//! A preferred stage that was ruled out is reported as
//! [`PlanWarning::IgnoredPreference`] even when no tie ever involved it.

use std::collections::{BTreeMap, BTreeSet};

use crate::core::lowering::DefaultMark;
use crate::model::names::{CapName, ElementName, RuleName, StageName, WorkflowName};
use crate::core::plan::build::{PlanError, PlanWarning};
use crate::core::registry::frozen::{BackendEntry, CapId, FlmRegistry, RuleIdx, StageIdx};
use crate::core::data::table::TableId;

/// The lowering rules one backend uses in one plan, one rule per element.
///
/// Lowering rule selection chooses one rule per element and backend in a fixed
/// order: the rule the request prefers by name, else the backend's native
/// emission, else a rule marked default on that backend, else a rule marked
/// default everywhere, else the sole candidate. The [module documentation](self)
/// describes which rules are candidates and when the choice fails.
///
/// A backend's lowering stage reads this value to work out what it needs, and the
/// lowering driver reads it at every node to find the rule to call. Read it from
/// [`Plan::selection`](crate::core::plan::Plan::selection) or, while the plan is
/// being built, from
/// [`PlanCx::lowering_selection`](crate::core::plan::PlanCx::lowering_selection).
///
/// The rule indices ([`RuleIdx`]) are numbers of the registry the plan was built
/// against, and mean nothing with any other registry.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SelectedRules {
    /// The rule chosen for each element. An element the backend writes natively
    /// has no entry, unless the request preferred a rule for it.
    pub by_element: BTreeMap<ElementName, RuleIdx>,
    /// The elements the backend writes natively for which the request preferred
    /// a rule, which is used instead of the native emission.
    pub overridden_native: BTreeSet<ElementName>,
}

impl SelectedRules {
    /// The rule chosen for `element`, or `None` when no rule was chosen for it on
    /// this backend.
    pub fn rule_for(&self, element: &str) -> Option<RuleIdx> {
        self.by_element.get(element).copied()
    }

    /// Every chosen rule, in element-name order.
    pub fn rules(&self) -> impl Iterator<Item = RuleIdx> + '_ {
        self.by_element.values().copied()
    }
}

/// Chooses one rule per element for `backend`, under the rule preferences
/// `prefer` (most preferred first), as the module documentation describes.
///
/// Returns the choice and one [`PlanWarning`] per preference that cannot be
/// honored: a preferred rule that is set aside, and a preferred rule that is
/// not a candidate on this backend. `workflow` is the workflow the plan request
/// was made from, which the error and the warnings name.
///
/// # Errors
///
/// Returns [`PlanError::RuleContest`] when several rules could lower an element
/// and nothing chooses between them.
pub(crate) fn select_rules(
    registry: &FlmRegistry,
    backend: &BackendEntry,
    prefer: &[RuleName],
    workflow: &Option<WorkflowName>,
) -> Result<(SelectedRules, Vec<PlanWarning>), PlanError> {
    let mut selected = SelectedRules::default();
    let warnings = unhonored_rule_preferences(registry, backend, prefer, workflow);
    let mut elements: BTreeSet<ElementName> =
        registry.rules().iter().map(|entry| entry.rule.element.clone()).collect();
    elements.extend(backend.native.iter().cloned());
    for element in elements {
        let candidates: Vec<RuleIdx> = registry
            .rules_for(&element)
            .iter()
            .copied()
            .filter(|idx| !registry.rule(*idx).is_set_aside())
            .filter(|idx| registry.rule(*idx).rule.default.takes_part_on(&backend.name))
            .collect();
        let preferred = prefer.iter().find_map(|name| {
            candidates.iter().copied().find(|idx| &registry.rule(*idx).rule.name == name)
        });
        if let Some(rule) = preferred {
            if backend.native.contains(&element) {
                selected.overridden_native.insert(element.clone());
            }
            selected.by_element.insert(element, rule);
            continue;
        }
        if backend.native.contains(&element) {
            continue;
        }
        let specific: Vec<RuleIdx> = candidates
            .iter()
            .copied()
            .filter(|idx| registry.rule(*idx).rule.default.is_specific_to(&backend.name))
            .collect();
        let general: Vec<RuleIdx> = candidates
            .iter()
            .copied()
            .filter(|idx| matches!(registry.rule(*idx).rule.default, DefaultMark::Everywhere))
            .collect();
        let chosen = match (specific.as_slice(), general.as_slice(), candidates.as_slice()) {
            ([rule], _, _) | ([], [rule], _) | ([], [], [rule]) => *rule,
            ([], [], []) => continue,
            _ => {
                let mut names: Vec<RuleName> =
                    candidates.iter().map(|idx| registry.rule(*idx).rule.name.clone()).collect();
                names.sort();
                return Err(PlanError::RuleContest {
                    backend: backend.name.clone(),
                    element,
                    candidates: names,
                    workflow: workflow.clone(),
                });
            }
        };
        selected.by_element.insert(element, chosen);
    }
    Ok((selected, warnings))
}

/// One [`PlanWarning`] per preference in `prefer` that this backend cannot
/// honor, in the order the preferences are written.
///
/// A preference is honored whenever the rule it refers to is a candidate for its
/// element on this backend, because the rule selection order consults the
/// preferences first. The two ways it cannot be honored are the two warnings:
/// the rule is set aside, or it is marked for other backends and so is not a
/// candidate here. A preference passed over because an earlier preference chose
/// a rule for the same element counts as honored and gives no warning, because
/// the caller ranked the two itself.
///
/// A name that no rule of this registry carries is not one of these cases: the
/// plan refuses such a request before choosing anything
/// ([`PlanError::UnknownRule`]). The last match arm below, which would report
/// it, only makes the function return a result for every input instead of
/// panicking.
fn unhonored_rule_preferences(
    registry: &FlmRegistry,
    backend: &BackendEntry,
    prefer: &[RuleName],
    workflow: &Option<WorkflowName>,
) -> Vec<PlanWarning> {
    let mut warnings: Vec<PlanWarning> = Vec::new();
    for name in prefer {
        let entry = registry.rule_idx(name).map(|idx| registry.rule(idx));
        match entry {
            Some(entry) if entry.is_set_aside() => {
                warnings.push(PlanWarning::PreferredRuleSetAside {
                    backend: backend.name.clone(),
                    rule: name.clone(),
                    unresolved: entry.set_aside.clone(),
                    workflow: workflow.clone(),
                });
            }
            Some(entry) if entry.rule.default.takes_part_on(&backend.name) => {}
            _ => warnings.push(PlanWarning::PreferredRuleNotCandidate {
                backend: backend.name.clone(),
                rule: name.clone(),
                workflow: workflow.clone(),
            }),
        }
    }
    warnings
}

/// A record of a stage that provider selection ruled out, with the scheduled stage
/// that ruled it out and a capability both provide.
///
/// Scheduling a stage rules out every other stage that provides any of the same
/// capabilities (see the [module documentation](self)). Each record lets a
/// message say which stage was ruled out, by which stage and over which
/// capability. [`Plan::excluded`](crate::core::plan::Plan::excluded) lists every
/// record of a plan, and [`PlanError::Unsatisfiable`] lists the ones that left a
/// needed capability without a provider.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct Excluded {
    /// The stage that was ruled out.
    pub stage: StageName,
    /// The stage that was scheduled, which ruled it out.
    pub excluded_by: StageName,
    /// A capability both stages provide; the first in name order when there are
    /// several, so that the record does not depend on the order in which the
    /// features registered.
    pub over: CapName,
}

/// The outcome of provider selection.
#[derive(Debug, Default)]
pub(crate) struct Selection {
    /// The stages that will run.
    pub scheduled: BTreeSet<StageIdx>,
    /// The scheduled stages in the order they were chosen, kept so that a
    /// reader can follow how the choice went.
    pub commit_order: Vec<StageIdx>,
    /// The stages that were ruled out, and why.
    pub excluded: BTreeMap<StageIdx, Excluded>,
    /// Every capability needed once provider selection has finished.
    pub needed: BTreeSet<CapId>,
    /// The external tables that must be installed when the run is constructed.
    pub externals: BTreeSet<TableId>,
    /// The stage preferences that could not be honored.
    pub warnings: Vec<PlanWarning>,
}

/// The state of one run of provider selection.
struct Solver<'a> {
    registry: &'a FlmRegistry,
    /// Every stage by name, which is the order every scan iterates in.
    by_name: BTreeMap<StageName, StageIdx>,
    requires_of: &'a dyn Fn(StageIdx) -> &'a BTreeSet<CapId>,
    prefer: &'a [StageIdx],
    needed: BTreeMap<CapName, CapId>,
    committed: BTreeSet<StageIdx>,
    commit_order: Vec<StageIdx>,
    excluded: BTreeMap<StageIdx, Excluded>,
    externals: BTreeSet<TableId>,
    warnings: Vec<PlanWarning>,
    warned: BTreeSet<StageIdx>,
    /// The workflow the plan request was made from, which the errors and
    /// warnings name.
    workflow: &'a Option<WorkflowName>,
}

impl<'a> Solver<'a> {
    fn provides(&self, stage: StageIdx) -> &'a BTreeSet<CapId> {
        &self.registry.stage(stage).provides
    }

    fn name(&self, stage: StageIdx) -> StageName {
        self.registry.stage(stage).name.clone()
    }

    /// The stages still eligible as providers of `cap`, in name order.
    fn candidates(&self, cap: CapId) -> Vec<StageIdx> {
        self.by_name
            .values()
            .copied()
            .filter(|stage| !self.excluded.contains_key(stage))
            .filter(|stage| self.provides(*stage).contains(&cap))
            .collect()
    }

    /// Every stage providing `cap`, ruled out or not, in name order.
    fn providers(&self, cap: CapId) -> Vec<StageIdx> {
        self.by_name
            .values()
            .copied()
            .filter(|stage| self.provides(*stage).contains(&cap))
            .collect()
    }

    fn need(&mut self, cap: CapId) {
        self.needed.insert(self.registry.capability(cap).name.clone(), cap);
    }

    fn commit(&mut self, stage: StageIdx) {
        self.committed.insert(stage);
        self.commit_order.push(stage);
        for cap in (self.requires_of)(stage).iter().copied().collect::<Vec<_>>() {
            self.need(cap);
        }
        let closure = self.provides(stage);
        let others: Vec<StageIdx> = self.by_name.values().copied().collect();
        for other in others {
            if other == stage
                || self.committed.contains(&other)
                || self.excluded.contains_key(&other)
            {
                continue;
            }
            let shared = closure.intersection(self.provides(other));
            // The name-least shared capability, so that the recorded reason does
            // not depend on registration order.
            let over = shared.map(|cap| &self.registry.capability(*cap).name).min().cloned();
            if let Some(over) = over {
                self.excluded.insert(
                    other,
                    Excluded { stage: self.name(other), excluded_by: self.name(stage), over },
                );
            }
        }
    }

    /// Items 2 and 3 of provider selection as the [module documentation](self)
    /// describes it: follows the forced choices until none is left.
    fn propagate(&mut self) -> Result<(), PlanError> {
        loop {
            let mut progressed = false;
            let needed: Vec<CapId> = self.needed.values().copied().collect();
            for cap in needed {
                let candidates = self.candidates(cap);
                if candidates.is_empty() {
                    let entry = self.registry.capability(cap);
                    if entry.external && self.providers(cap).is_empty() {
                        if let Some(table) = entry.table {
                            self.externals.insert(table);
                        }
                        continue;
                    }
                    let blocked: Vec<Excluded> = self
                        .providers(cap)
                        .into_iter()
                        .filter_map(|stage| self.excluded.get(&stage).cloned())
                        .collect();
                    return Err(PlanError::Unsatisfiable {
                        capability: entry.name.clone(),
                        unprovided: if blocked.is_empty() { entry.unprovided.clone() } else { None },
                        blocked,
                        workflow: self.workflow.clone(),
                    });
                }
                if candidates.len() == 1 && !self.committed.contains(&candidates[0]) {
                    self.commit(candidates[0]);
                    progressed = true;
                    break; // `needed` may have grown: restart the sweep
                }
            }
            if !progressed {
                return Ok(());
            }
        }
    }

    /// The needed capabilities that still have several candidates and none of
    /// them scheduled, in name order.
    fn contested(&self) -> Vec<CapId> {
        self.needed
            .values()
            .copied()
            .filter(|cap| {
                let candidates = self.candidates(*cap);
                candidates.len() > 1
                    && !candidates.iter().any(|stage| self.committed.contains(stage))
            })
            .collect()
    }

    /// Item 4 of provider selection, first half: the first preferred stage that
    /// is a candidate of one of the `contested` capabilities. Records a warning
    /// for each preferred stage already ruled out.
    fn preferred(&mut self, contested: &[CapId]) -> Option<StageIdx> {
        for stage in self.prefer.iter().copied() {
            if self.committed.contains(&stage) {
                continue;
            }
            if let Some(excluded) = self.excluded.get(&stage) {
                if self.warned.insert(stage) {
                    self.warnings.push(PlanWarning::IgnoredPreference {
                        stage: excluded.stage.clone(),
                        excluded_by: excluded.excluded_by.clone(),
                        over: excluded.over.clone(),
                        workflow: self.workflow.clone(),
                    });
                }
                continue;
            }
            if contested.iter().any(|cap| self.candidates(*cap).contains(&stage)) {
                return Some(stage);
            }
        }
        None
    }

    /// Item 4 of provider selection, second half: decides one contested
    /// capability by its default refinement, following the chain of defaults down
    /// until exactly one candidate is left.
    fn by_default(&self, cap: CapId) -> Option<StageIdx> {
        let mut current = self.registry.default_refinement(cap)?;
        loop {
            let candidates = self.candidates(current);
            match candidates.as_slice() {
                [stage] => return Some(*stage),
                [] => return None,
                _ => current = self.registry.default_refinement(current)?,
            }
        }
    }

    fn solve(mut self) -> Result<Selection, PlanError> {
        loop {
            self.propagate()?;
            let contested = self.contested();
            if contested.is_empty() {
                break;
            }
            if let Some(stage) = self.preferred(&contested) {
                self.commit(stage);
                continue;
            }
            let cap = contested[0];
            match self.by_default(cap) {
                Some(stage) => self.commit(stage),
                None => {
                    let candidates =
                        self.candidates(cap).into_iter().map(|stage| self.name(stage)).collect();
                    return Err(PlanError::Contested {
                        capability: self.registry.capability(cap).name.clone(),
                        candidates,
                        workflow: self.workflow.clone(),
                    });
                }
            }
        }
        // A preference naming a stage the deduction ruled out is reported even
        // when no contest ever looked at that preference: the caller asked for
        // something that did not happen, and the reason is the same either way.
        // See the design rationale at [§flm-dd-dr:pipe-selection].
        for stage in self.prefer.iter().copied() {
            if let Some(excluded) = self.excluded.get(&stage) {
                if self.warned.insert(stage) {
                    self.warnings.push(PlanWarning::IgnoredPreference {
                        stage: excluded.stage.clone(),
                        excluded_by: excluded.excluded_by.clone(),
                        over: excluded.over.clone(),
                        workflow: self.workflow.clone(),
                    });
                }
            }
        }
        Ok(Selection {
            scheduled: self.committed,
            commit_order: self.commit_order,
            excluded: self.excluded,
            needed: self.needed.values().copied().collect(),
            externals: self.externals,
            warnings: self.warnings,
        })
    }
}

/// Chooses one provider per needed capability, starting from `demands`, by the
/// provider selection described in the module documentation.
///
/// `prefer` is the request's stage preferences, most preferred first.
/// `requires_of` returns what one stage requires, already resolved against the
/// registry. `workflow` is the workflow the plan request was made from, which
/// the errors and the warnings name.
///
/// # Errors
///
/// Returns [`PlanError::Unsatisfiable`] when a needed capability has no
/// candidate left, and [`PlanError::Contested`] when several candidates remain
/// and neither a preference nor a marked default chooses between them.
pub(crate) fn select_providers<'a>(
    registry: &'a FlmRegistry,
    demands: impl IntoIterator<Item = CapId>,
    prefer: &'a [StageIdx],
    requires_of: &'a dyn Fn(StageIdx) -> &'a BTreeSet<CapId>,
    workflow: &'a Option<WorkflowName>,
) -> Result<Selection, PlanError> {
    let by_name: BTreeMap<StageName, StageIdx> =
        registry.stages().iter().map(|entry| (entry.name.clone(), entry.idx)).collect();
    let mut solver = Solver {
        registry,
        by_name,
        requires_of,
        prefer,
        needed: BTreeMap::new(),
        committed: BTreeSet::new(),
        commit_order: Vec::new(),
        excluded: BTreeMap::new(),
        externals: BTreeSet::new(),
        warnings: Vec::new(),
        warned: BTreeSet::new(),
        workflow,
    };
    for cap in demands {
        solver.need(cap);
    }
    solver.solve()
}
