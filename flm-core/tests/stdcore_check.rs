//! The block-contract checking stage `flm.blocks.check-contract`
//! (`flm_core::stdcore::features::blocks`) and what the `check` workflow
//! collects: the workflow plans and runs over a registry with and without user
//! definitions, a plan that demands no check never schedules the stage, the
//! check fits beside either no-op alternative and then checks the part of the
//! contract that alternative defines, and a violation the block-structure pass
//! leaves behind is reported as the checker's own diagnostic.

mod common;

use std::sync::Arc;

use flm_core::model::element::ElementSpec;
use flm_core::model::names::{ElementName, StageName};
use flm_core::core::plan::{Overrides, Plan, PlanRequest};
use flm_core::core::registry::FlmRegistry;
use flm_core::dump::diag::expect;
use flm_core::lang::Flm;
use flm_core::lang::mint::BodyDef;
use flm_core::lang::rewrites::blocks::{
    BlockStructure, BlockStructurer, ContractViolated, StructureCx, StructureError, Structuring,
};
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::stdcore::features::blocks::{Blocks, CONTRACT_CHECK, CONTRACT_CHECKED, RAW};
use flm_core::stdcore::features::userdefs::{KEPT, Userdefs, UserdefsConfig};
use flm_core::stdcore::{CHECK_WORKFLOW, VALIDATED, check_feature};
use techy::core::node::NodeRef;

use common::blocks::env_of;
use common::stdcore::{identifiers, plan, registry_over, run_workflow, try_plan};

fn names(stages: &[StageName]) -> Vec<String> {
    stages.iter().map(|name| name.to_string()).collect()
}

/// A registry over `blocks`, `userdefs` (one `per-plan` macro whose template
/// holds a paragraph break) and the `check` feature.
fn with_userdefs() -> Arc<FlmRegistry> {
    let definition = Definition::macro_("foo", Vec::new()).text("a\n\nb");
    let config = UserdefsConfig::new().definition(definition);
    registry_over(vec![
        Arc::new(Blocks::default()),
        Arc::new(Userdefs::new(config)),
        check_feature(),
    ])
}

/// A registry over `blocks` and the `check` feature alone.
fn without_userdefs() -> Arc<FlmRegistry> {
    registry_over(vec![Arc::new(Blocks::default()), check_feature()])
}

#[test]
fn the_check_workflow_over_a_registry_with_userdefs_plans_both_expansions_and_the_check() {
    let plan = plan(&with_userdefs(), CHECK_WORKFLOW, &Overrides::none());
    assert_eq!(
        names(&plan.rewrite_names()),
        ["flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]
    );
    assert_eq!(names(&plan.stage_names()), [CONTRACT_CHECK]);
}

#[test]
fn the_check_workflow_over_a_registry_without_userdefs_plans_structure_and_the_check() {
    let plan = plan(&without_userdefs(), CHECK_WORKFLOW, &Overrides::none());
    assert_eq!(names(&plan.rewrite_names()), ["flm.blocks.structure"]);
    assert_eq!(names(&plan.stage_names()), [CONTRACT_CHECK]);
}

#[test]
fn a_conforming_document_passes_the_check_with_userdefs() {
    let run = run_workflow(&with_userdefs(), CHECK_WORKFLOW, &[r"x \foo."], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

#[test]
fn a_conforming_document_passes_the_check_without_userdefs() {
    let run = run_workflow(&without_userdefs(), CHECK_WORKFLOW, &["a\n\nb"], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

/// A kept, structured plan that does not demand the check: no default pulls
/// the contract check in (demanding it beside `kept` is the atomicity error
/// the next test pins).
#[test]
fn no_default_pulls_the_contract_check_into_a_plan_that_did_not_demand_it() {
    let request = PlanRequest::new().demand(KEPT).demand("flm.blocks.structured");
    let plan = Plan::build(&with_userdefs(), &request).expect("a kept, structured plan");
    assert!(!plan.stage_names().iter().any(|name| name == CONTRACT_CHECK));
}

/// The check fits into a plan that keeps the per-plan definitions: it requires
/// no alternative of `flm.userdefs`, only reads which one ran. The surviving
/// use sites are then what the plan asked for, so the check does not report
/// them; everything else the block-structure contract says still holds.
#[test]
fn the_check_fits_beside_kept_definitions_and_lets_the_use_sites_stand() {
    let request = PlanRequest::new().demand(VALIDATED).demand(KEPT);
    let plan = Plan::build(&with_userdefs(), &request).expect("a kept plan");
    assert!(plan.stage_names().iter().any(|name| name == CONTRACT_CHECK));
    let overrides = Overrides { demands: vec![KEPT.into()], ..Overrides::none() };
    let run = run_workflow(&with_userdefs(), CHECK_WORKFLOW, &[r"x oo."], &overrides);
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

/// The check fits into a plan that keeps the trees raw: it requires only that
/// a decision about block structure was made. The block-structure part of the
/// contract is not defined over raw trees, so only the parse part runs — and a
/// raw tree, which still holds its paragraph-break markers, passes.
#[test]
fn the_check_fits_beside_raw_trees_and_checks_the_parse_contract_alone() {
    let overrides = Overrides { demands: vec![RAW.into()], ..Overrides::none() };
    let plan = try_plan(&without_userdefs(), CHECK_WORKFLOW, &overrides).expect("a raw plan");
    assert_eq!(names(&plan.rewrite_names()), Vec::<String>::new());
    assert_eq!(names(&plan.stage_names()), [RAW, CONTRACT_CHECK]);
    let run = run_workflow(&without_userdefs(), CHECK_WORKFLOW, &["a\n\nb"], &overrides);
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

/// A registry holding `blocks` but not the `check` feature builds: the
/// contract check's membership is dropped silently, with nothing in the
/// build diagnostics, and a structured plan still needs no check.
#[test]
fn a_check_member_without_the_collection_s_declarer_builds_silently() {
    let registry = registry_over(vec![Arc::new(Blocks::default())]);
    let identifiers: Vec<String> =
        registry.build_diagnostics().iter().map(|d| d.identifier().to_string()).collect();
    assert_eq!(identifiers, Vec::<String>::new());
    assert!(registry.collection(VALIDATED).is_none());
    let request = PlanRequest::new().demand("flm.blocks.structured");
    let plan = Plan::build(&registry, &request).expect("a structured plan");
    assert!(!plan.stage_names().iter().any(|name| name == CONTRACT_CHECK));
}

#[test]
fn the_contract_check_is_a_member_of_the_validated_collection() {
    let registry = without_userdefs();
    let collection = registry.collection(VALIDATED).expect("declared by the check feature");
    let member = registry.cap_id(CONTRACT_CHECKED).expect("declared by blocks");
    assert!(collection.members.contains(&member));
}

/// An inline environment whose hook lays its body out as blocks although its
/// declaration makes the body inline — a violation the pass leaves in the
/// frozen tree. The checker's own suite pins the violation kind; this test
/// pins the stage that reports it.
struct BlocksInInlineBody;

impl BlockStructure for BlocksInInlineBody {
    fn structure(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
        bs: &mut dyn BlockStructurer,
    ) -> Result<Structuring, StructureError> {
        let run = bs.body_run(&node)?;
        let body = bs.build_blocks(cx, &run)?;
        Ok(Structuring::Emit(vec![bs.with_body(cx, &node, body)?]))
    }
}

#[test]
fn a_violation_is_reported_as_the_checkers_diagnostic() {
    let element = Arc::new(
        ElementSpec::new(ElementName::from("t.oddenv"))
            .with::<dyn BlockStructure>(Arc::new(BlocksInInlineBody)),
    );
    let oddenv = env_of("oddenv", BodyDef::new(), element);
    let toy = common::pipeline::feature("t", move |b| b.element(oddenv.clone()));
    let registry = registry_over(vec![Arc::new(Blocks::default()), check_feature(), toy]);
    let run = run_workflow(
        &registry,
        CHECK_WORKFLOW,
        &["\\begin{oddenv}a\\end{oddenv}"],
        &Overrides::none(),
    );
    let diagnostic = expect(run.diagnostics().iter(), "flm.blocks.contract-violated").once();
    let condition = diagnostic.data().downcast_ref::<ContractViolated>().expect("typed");
    assert_eq!(condition.rule, "ParagraphInInlineContainer");
}
