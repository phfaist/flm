//! The workflows ([`Environment::workflows`], [`Environment::workflow`]) and the plan in
//! view ([`PlanView`]).

use flm_core::core::plan::{Excluded, Plan, PlanWarning};
use flm_core::core::registry::{StageIdx, WorkflowDef, WorkflowEntry};
use serde::Serialize;

use super::{Environment, is_false, summary_of};

impl Environment {
    /// Every workflow of the registry, in name order.
    pub fn workflows(&self) -> Vec<WorkflowView> {
        self.registry.workflows().map(|entry| self.workflow_view(entry)).collect()
    }

    /// The workflow called `name`, or `None` when the registry has no such workflow. When
    /// the plan in view ([`plan`](Environment::plan)) was built from this workflow, the view
    /// holds it.
    pub fn workflow(&self, name: &str) -> Option<WorkflowView> {
        self.registry.workflow(name).map(|entry| self.workflow_view(entry))
    }

    fn workflow_view(&self, entry: &WorkflowEntry) -> WorkflowView {
        let name = entry.def.name.as_str();
        let chain = self
            .registry
            .workflow_chain(name)
            .map(|chain| chain.iter().map(|def| def.name.to_string()).collect())
            .unwrap_or_default();
        let plan = self
            .plan
            .as_ref()
            .filter(|plan| plan.request().workflow.as_ref().is_some_and(|workflow| workflow == name))
            .map(|plan| plan_view(plan));
        WorkflowView {
            name: name.to_string(),
            owner: entry.owner.to_string(),
            docs: entry.def.docs.clone(),
            summary: summary_of(&entry.def.docs).to_string(),
            chain,
            declared: entry.def.clone(),
            combined: self.registry.combined_workflow(name).unwrap_or_else(|| entry.def.clone()),
            plan,
        }
    }
}

/// The view of `plan`.
fn plan_view(plan: &Plan) -> PlanView {
    let registry = plan.registry();
    let mut stages: Vec<PlannedStageView> =
        plan.rewrites().iter().map(|idx| planned_stage(plan, *idx, true, None)).collect();
    for (batch, range) in plan.batches().iter().enumerate() {
        stages.extend(plan.order()[range.clone()].iter().map(|idx| planned_stage(plan, *idx, false, Some(batch))));
    }
    let lowering = plan
        .active_backends()
        .iter()
        .map(|backend| LoweringView {
            backend: backend.to_string(),
            rules: plan
                .selection(backend.as_str())
                .map(|selected| {
                    selected
                        .by_element
                        .iter()
                        .map(|(element, rule)| RuleChoiceView {
                            element: element.to_string(),
                            rule: registry.rule(*rule).rule.name.to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();
    PlanView {
        workflow: plan.request().workflow.as_ref().map(ToString::to_string),
        demands: plan.demands().iter().map(ToString::to_string).collect(),
        stages,
        commit_order: plan.commit_order().iter().map(ToString::to_string).collect(),
        excluded: plan.excluded().to_vec(),
        warnings: plan.warnings().to_vec(),
        lowering,
        externals: plan.external_names().iter().map(ToString::to_string).collect(),
    }
}

/// The view of the scheduled stage `idx` of `plan`.
pub(super) fn planned_stage(plan: &Plan, idx: StageIdx, rewrite: bool, batch: Option<usize>) -> PlannedStageView {
    let registry = plan.registry();
    let entry = registry.stage(idx);
    let record = plan.manifest(idx);
    PlannedStageView {
        name: entry.name.to_string(),
        owner: entry.owner.to_string(),
        rewrite,
        batch,
        reads: record
            .map(|record| {
                record
                    .reads
                    .names()
                    .into_iter()
                    .map(|table| {
                        let read = record.reads.get(table.as_str());
                        ReadView {
                            optional: read.is_some_and(|read| read.optional),
                            writer: read.and_then(|read| read.producer.as_ref()).map(ToString::to_string),
                            table: table.to_string(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default(),
        writes: record.map(|record| record.writes.names().iter().map(ToString::to_string).collect()).unwrap_or_default(),
        requires: record
            .map(|record| record.requires.iter().map(|id| registry.capability(*id).name.to_string()).collect())
            .unwrap_or_default(),
        options: record
            .map(|record| record.options.iter().map(|option| option.key().short_name().to_string()).collect())
            .unwrap_or_default(),
    }
}

/// One workflow, as [`Environment::workflow`] and [`Environment::workflows`] answer it.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct WorkflowView {
    /// The workflow's name.
    pub name: String,
    /// The feature that registered it.
    pub owner: String,
    /// Its documentation text.
    #[serde(skip_serializing_if = "super::is_empty")]
    pub docs: String,
    /// The first paragraph of [`docs`](WorkflowView::docs).
    #[serde(skip_serializing_if = "super::is_empty")]
    pub summary: String,
    /// The workflow and those it extends, directly or not, from the one that extends none
    /// to this one, which comes last.
    pub chain: Vec<String>,
    /// The workflow as declared: its outputs, demands, preferences, the backends' rule
    /// preferences and unsupported elements, the workflow it extends, and its
    /// documentation text (the form of [`WorkflowDef`]).
    pub declared: WorkflowDef,
    /// The workflow with the fields it inherits along [`chain`](WorkflowView::chain)
    /// ([`FlmRegistry::combined_workflow`](flm_core::core::registry::FlmRegistry::combined_workflow)).
    pub combined: WorkflowDef,
    /// The plan in view, when it was built from this workflow.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<PlanView>,
}

/// A plan: the stages that run for a workflow, in order, with the record of how the plan
/// was built.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
pub struct PlanView {
    /// The workflow the plan was built from; absent for a plan built from a request made by
    /// hand.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
    /// Every demanded capability, collections replaced by their members, in name order.
    pub demands: Vec<String>,
    /// The scheduled stages in the order they run: the rewrite stages first, then the
    /// other stages, each with its batch.
    pub stages: Vec<PlannedStageView>,
    /// The scheduled stages in the order in which provider selection chose them.
    pub commit_order: Vec<String>,
    /// The stages that provider selection ruled out, each with the stage that ruled it out
    /// and the capability both provide.
    pub excluded: Vec<Excluded>,
    /// The preferences the plan could not honor.
    pub warnings: Vec<PlanWarning>,
    /// For each backend that takes part in the plan, in name order, the lowering rule
    /// chosen for each element.
    pub lowering: Vec<LoweringView>,
    /// The external tables: those no stage writes, whose values the caller installs.
    pub externals: Vec<String>,
}

/// One scheduled stage of a plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct PlannedStageView {
    /// The stage's name.
    pub name: String,
    /// The feature that registered it.
    pub owner: String,
    /// Whether it is a rewrite stage, which changes the trees before they are frozen.
    #[serde(skip_serializing_if = "is_false")]
    pub rewrite: bool,
    /// For a stage that runs after the rewrite stages, the number of its batch, counting
    /// from 0: the stages of one batch depend on none of each other.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch: Option<usize>,
    /// The tables it reads, in name order, each with the stage that writes it.
    pub reads: Vec<ReadView>,
    /// The tables it writes, in name order.
    pub writes: Vec<String>,
    /// Every capability it requires, in identifier order.
    pub requires: Vec<String>,
    /// The option types it reads.
    pub options: Vec<String>,
}

/// One table a scheduled stage reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ReadView {
    /// The table's name.
    pub table: String,
    /// Whether the table may be absent.
    #[serde(skip_serializing_if = "is_false")]
    pub optional: bool,
    /// The scheduled stage that writes the table, when there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub writer: Option<String>,
}

/// The lowering rules a plan chose for one backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct LoweringView {
    /// The backend's name.
    pub backend: String,
    /// The rule chosen for each element, in element name order.
    pub rules: Vec<RuleChoiceView>,
}

/// The lowering rule chosen for one element.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct RuleChoiceView {
    /// The element type's name.
    pub element: String,
    /// The rule's name.
    pub rule: String,
}
