//! The wired rewrite features at plan build
//! (`flm_core::stdcore::features::blocks` and
//! `flm_core::stdcore::features::userdefs`): block structure activates the
//! expansion decision, the marked defaults, the marker alternatives, the
//! atomicity error, the order of the two expansion stages, and the declared
//! configurations.

mod common;

use std::sync::Arc;

use flm_core::model::names::StageName;
use flm_core::model::options::{Bind, OptionKey};
use flm_core::core::plan::{Plan, PlanError, PlanRequest};
use flm_core::core::registry::FlmFeature;
use flm_core::lang::rewrites::blocks::BlocksConfig;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::userdefs::UserdefsConfig;

use common::pipeline::plan;
use common::wiring::registry;

fn names(names: &[StageName]) -> Vec<String> {
    names.iter().map(|n| n.to_string()).collect()
}

/// One `per-plan` macro `\foo`.
fn foo() -> Vec<Definition> {
    vec![Definition::macro_("foo", Vec::new()).text("hello")]
}

// --- activation and order ------------------------------------------------------------------------

#[test]
fn demanding_structured_schedules_expansion_then_structure() {
    let plan = plan(&registry(foo()), &["flm.blocks.structured"]);
    assert_eq!(names(&plan.rewrite_names()), ["flm.userdefs.expand", "flm.blocks.structure"]);
    assert!(plan.stage_names().is_empty());
}

#[test]
fn demanding_the_parents_takes_the_marked_defaults() {
    let plan = plan(&registry(foo()), &["flm.blocks", "flm.userdefs"]);
    assert_eq!(names(&plan.rewrite_names()), ["flm.userdefs.expand", "flm.blocks.structure"]);
}

#[test]
fn demanding_kept_and_structured_schedules_the_marker_and_structure() {
    let plan = plan(&registry(foo()), &["flm.userdefs.kept", "flm.blocks.structured"]);
    assert_eq!(names(&plan.rewrite_names()), ["flm.blocks.structure"]);
    assert_eq!(names(&plan.stage_names()), ["flm.userdefs.keep"]);
}

#[test]
fn demanding_raw_and_kept_schedules_no_rewrite() {
    let plan = plan(&registry(foo()), &["flm.blocks.raw", "flm.userdefs.kept"]);
    assert!(plan.rewrites().is_empty());
    let mut markers = names(&plan.stage_names());
    markers.sort();
    assert_eq!(markers, ["flm.blocks.raw", "flm.userdefs.keep"]);
}

#[test]
fn always_expansion_precedes_per_plan_expansion() {
    let demands = ["flm.userdefs.always-expanded", "flm.userdefs.expanded"];
    let plan = plan(&registry(foo()), &demands);
    assert_eq!(
        names(&plan.rewrite_names()),
        ["flm.userdefs.expand-always", "flm.userdefs.expand"]
    );
}

#[test]
fn the_after_edge_alone_never_activates_always_expansion() {
    let plan = plan(&registry(foo()), &["flm.userdefs.expanded"]);
    assert_eq!(names(&plan.rewrite_names()), ["flm.userdefs.expand"]);
}

#[test]
fn kept_with_always_expansion_orders_always_expansion_before_structure() {
    let demands = ["flm.userdefs.kept", "flm.userdefs.always-expanded", "flm.blocks.structured"];
    let plan = plan(&registry(foo()), &demands);
    assert_eq!(
        names(&plan.rewrite_names()),
        ["flm.userdefs.expand-always", "flm.blocks.structure"]
    );
    assert_eq!(names(&plan.stage_names()), ["flm.userdefs.keep"]);
}

#[test]
fn block_structure_alone_needs_no_expansion_decision() {
    let blocks: Arc<dyn FlmFeature> = Arc::new(Blocks::default());
    let registry = common::pipeline::registry(vec![blocks]);
    let plan = plan(&registry, &["flm.blocks.structured"]);
    assert_eq!(names(&plan.rewrite_names()), ["flm.blocks.structure"]);
}

// --- the alternatives ----------------------------------------------------------------------------

#[test]
fn demanding_expanded_and_kept_is_the_atomicity_error() {
    let request = PlanRequest::new().demand("flm.userdefs.expanded").demand("flm.userdefs.kept");
    match Plan::build(&registry(foo()), &request).expect_err("exclusive") {
        PlanError::Unsatisfiable { capability, blocked, .. } => {
            assert_eq!(capability, "flm.userdefs.kept");
            assert_eq!(blocked.len(), 1);
            assert_eq!(blocked[0].stage, "flm.userdefs.keep");
            assert_eq!(blocked[0].excluded_by, "flm.userdefs.expand");
            assert_eq!(blocked[0].over, "flm.userdefs");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn demanding_structured_and_raw_is_the_atomicity_error() {
    let request = PlanRequest::new().demand("flm.blocks.structured").demand("flm.blocks.raw");
    match Plan::build(&registry(foo()), &request).expect_err("exclusive") {
        PlanError::Unsatisfiable { capability, blocked, .. } => {
            assert_eq!(capability, "flm.blocks.structured");
            assert_eq!(blocked.len(), 1);
            assert_eq!(blocked[0].stage, "flm.blocks.structure");
            assert_eq!(blocked[0].excluded_by, "flm.blocks.raw");
            assert_eq!(blocked[0].over, "flm.blocks");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// --- the registry's shape ------------------------------------------------------------------------

#[test]
fn expanded_is_the_marked_default_of_userdefs() {
    let registry = registry(foo());
    let expanded = registry.capability_named("flm.userdefs.expanded").expect("declared");
    assert_eq!(expanded.refines, registry.cap_id("flm.userdefs"));
    assert!(expanded.default);
    assert!(expanded.table.is_some());
}

#[test]
fn kept_refines_userdefs_without_being_its_default() {
    let registry = registry(foo());
    let kept = registry.capability_named("flm.userdefs.kept").expect("declared");
    assert_eq!(kept.refines, registry.cap_id("flm.userdefs"));
    assert!(!kept.default);
    assert!(kept.table.is_none());
}

#[test]
fn always_expanded_refines_nothing() {
    let registry = registry(foo());
    let always = registry.capability_named("flm.userdefs.always-expanded").expect("declared");
    assert!(always.refines.is_none());
    assert!(always.table.is_some());
}

#[test]
fn structured_is_the_marked_default_of_blocks() {
    let registry = registry(foo());
    let structured = registry.capability_named("flm.blocks.structured").expect("declared");
    assert_eq!(structured.refines, registry.cap_id("flm.blocks"));
    assert!(structured.default);
    assert!(structured.table.is_some());
}

#[test]
fn raw_refines_blocks_without_being_its_default() {
    let registry = registry(foo());
    let raw = registry.capability_named("flm.blocks.raw").expect("declared");
    assert_eq!(raw.refines, registry.cap_id("flm.blocks"));
    assert!(!raw.default);
    assert!(raw.table.is_none());
}

#[test]
fn the_configurations_are_declared_registry_bound() {
    let registry = registry(foo());
    let blocks = registry.option_decl(&OptionKey::of::<BlocksConfig>()).expect("declared");
    assert_eq!(blocks.bind, Bind::Registry);
    assert_eq!(blocks.owner, "blocks");
    let userdefs = registry.option_decl(&OptionKey::of::<UserdefsConfig>()).expect("declared");
    assert_eq!(userdefs.bind, Bind::Registry);
    assert_eq!(userdefs.owner, "userdefs");
}
