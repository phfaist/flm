//! The `stdcore` package end to end (`flm_core::stdcore::package`): a host
//! builds its registry from the package alone and runs the `text` workflow over
//! a small document holding a labeled heading, a reference to it, an unresolved
//! reference, an `\emph`, a display-math environment and a user-defined macro —
//! the lowered tree and the printed text as goldens — and the `check` workflow
//! over the same document.

mod common;

use std::sync::Arc;

use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::model::options::OptionValues;
use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::dump::diag::expect;
use flm_core::dump::golden::{assert_json_golden, assert_text_golden};
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::blocks::CONTRACT_CHECK;
use flm_core::math_passthrough::CHECK as MATH_CHECK;
use flm_core::stdcore::features::refs::{CHECK_LABELS, RefsRenderOptions, UnresolvedRefs};
use flm_core::stdcore::features::userdefs::{UserdefsConfig, element_name};
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig};
use techy::error::Severity;

use common::lowering::{lowered, lowered_projection, text as text_of};
use common::stdcore::{
    golden_path, identifiers, plan, registry, run_workflow, try_plan,
};

/// The document: a labeled heading, formatting, a user macro, display math,
/// a resolved and an unresolved reference.
const DOCUMENT: &str = "\\section{Introduction}\\label{sec:intro}\n\n\
                        This is \\emph{emphasized} text by \\myname.\n\n\
                        \\begin{equation}\n  E = mc^2\n\\end{equation}\n\n\
                        See \\ref{sec:intro} and \\ref{sec:missing}.\n";

/// The user macro `\myname`, expanding to `FLM`.
fn myname() -> Definition {
    Definition::macro_("myname", Vec::new()).text("FLM")
}

/// The host's registry: the package over the one macro.
fn host_registry() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new().userdefs(UserdefsConfig::new().definition(myname())))
}

/// The host's overrides for the `text` workflow: the unresolved reference
/// tolerated or not. No hole is needed for the user macro's element: its
/// `expand-in-tree` rule covers it (the seam the integration work package
/// closed; `the_text_plan_is_complete_without_a_hole_for_the_user_macro`).
fn text_overrides(unresolved: UnresolvedRefs) -> Overrides {
    Overrides {
        render_options: OptionValues::new().with(RefsRenderOptions { unresolved }),
        ..Overrides::none()
    }
}

/// The overrides tolerating the unresolved reference.
fn allow_unresolved() -> Overrides {
    text_overrides(UnresolvedRefs::Allow)
}

#[test]
fn the_package_builds_a_registry_offering_the_text_and_check_workflows() {
    let registry = host_registry();
    assert!(registry.workflow(text::WORKFLOW).is_some());
    assert!(registry.workflow(CHECK_WORKFLOW).is_some());
    for element in ["flm.heading", "flm.ref", "flm.field", "flm.textstyle", "flm.math-passthrough.opaque"] {
        assert!(!registry.elements_named(element).is_empty(), "{element} is missing");
    }
    assert!(!registry.elements_named(element_name(&myname()).as_str()).is_empty());
}

#[test]
fn the_text_workflow_lowers_and_prints_the_document() {
    let run = run_workflow(&host_registry(), text::WORKFLOW, &[DOCUMENT], &allow_unresolved());
    assert_eq!(identifiers(&run), ["flm.refs.unresolved"]);
    assert_json_golden(golden_path("package-lowered.json"), &lowered_projection(&run));
    assert_text_golden(golden_path("package-text.txt"), &text_of(&run));
}

#[test]
fn the_user_macro_is_expanded_before_lowering() {
    let run = run_workflow(&host_registry(), text::WORKFLOW, &[DOCUMENT], &allow_unresolved());
    let lowered = lowered(&run, TreeId::tree(0));
    let macro_element = element_name(&myname());
    assert!(lowered.tree.root().descendants().all(|node| element_of(&node).name != macro_element));
    assert!(text_of(&run).contains("by FLM."));
}

#[test]
fn the_unresolved_reference_is_an_error_unless_the_request_allows_it() {
    let overrides = text_overrides(UnresolvedRefs::Error);
    let run = run_workflow(&host_registry(), text::WORKFLOW, &[DOCUMENT], &overrides);
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.unresolved").once();
    assert_eq!(diagnostic.severity(), Severity::Error);
}

/// The seam the userdefs `expand-in-tree` rule closes: a
/// registry holding a user definition plans against `text` under no
/// overrides at all — the definition's element is reachable through its
/// rule, so plan completeness lists nothing.
#[test]
fn the_text_plan_is_complete_without_a_hole_for_the_user_macro() {
    let plan = try_plan(&host_registry(), text::WORKFLOW, &Overrides::none())
        .unwrap_or_else(|error| panic!("the text plan must be complete: {error}"));
    let selected = plan.selection(text::BACKEND).expect("the text backend is active");
    let rule = selected.by_element.get(&element_name(&myname())).expect("the macro's rule");
    assert_eq!(
        plan.registry().rule(*rule).rule.name,
        "flm.userdefs.macro.myname.lower.expand-in-tree"
    );
}

/// Stage names as plain strings, in the plan's order.
fn names(plan_names: &[flm_core::model::names::StageName]) -> Vec<String> {
    plan_names.iter().map(|name| name.to_string()).collect()
}

#[test]
fn the_text_plan_over_the_package_schedules_the_refs_chain_before_lowering() {
    let plan = plan(&host_registry(), text::WORKFLOW, &allow_unresolved());
    // The tree-contract check the render demands requires the `always`
    // expansion, which is why that rewrite heads the chain with nothing to do.
    assert_eq!(
        names(&plan.rewrite_names()),
        ["flm.input.splice", "flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]
    );
    assert_eq!(
        names(&plan.stage_names()),
        [
            "flm.blocks.check-contract",
            "flm.index.build",
            "flm.refpoints.scan",
            "flm.numbering.number",
            "flm.refs.scan",
            "flm.refs.anchors",
            "flm.refs.resolve",
            "flm.refs.check-labels",
            "flm.xnotes.collect",
            "flm.xnotes.build-list.footnote",
            "flm.theorems.check-proofs",
            "flm.graphics.inspect",
            "flm.graphics.publish.text",
            "flm.floats.check-handlers",
            "flm.citations.collect",
            "flm.citations.retrieve",
            "flm.citations.check",
            "flm.citations.build-bibliography",
            "flm.meta.build",
            "flm.text.pieces",
            "flm.text.lower",
            "flm.text.emit",
            "flm.text.regions",
            "flm.text.final",
            "flm.math-passthrough.check",
        ]
    );
}

#[test]
fn the_heading_rules_optional_read_of_the_anchors_table_is_a_required_read_here() {
    // The heading rule reads the anchors table optionally, so that a registry
    // without the references feature still renders headings. Here that feature
    // is registered and a stage writes the table, so the lowering stage takes
    // the read as a required one and the plan names the stage that writes it.
    // The anchors stage was scheduled for this workflow in any case, which is
    // why the stage lists above are unchanged.
    let plan = plan(&host_registry(), text::WORKFLOW, &allow_unresolved());
    let registry = plan.registry();
    let idx = registry.stage_idx("flm.text.lower").expect("the lowering stage");
    let read = plan
        .manifest(idx)
        .expect("its manifest")
        .reads
        .get("flm.anchors")
        .expect("the anchors table is read");
    assert!(!read.optional);
    assert_eq!(
        read.producer.as_ref().map(ToString::to_string),
        Some("flm.refs.anchors".to_string())
    );
}

#[test]
fn the_check_plan_over_the_package_schedules_both_expansions_and_every_check() {
    let plan = plan(&host_registry(), CHECK_WORKFLOW, &Overrides::none());
    assert_eq!(
        names(&plan.rewrite_names()),
        ["flm.input.splice", "flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]
    );
    assert_eq!(
        names(&plan.stage_names()),
        [
            "flm.blocks.check-contract",
            "flm.index.build",
            "flm.refpoints.scan",
            "flm.numbering.number",
            "flm.refs.scan",
            "flm.refs.anchors",
            "flm.refs.resolve",
            "flm.refs.check-labels",
            "flm.theorems.check-proofs",
            "flm.floats.check-handlers",
            "flm.citations.collect",
            "flm.citations.retrieve",
            "flm.citations.check",
            "flm.meta.build",
            "flm.math-passthrough.check",
        ]
    );
}

#[test]
fn the_check_workflow_plans_every_contributed_check() {
    let plan = plan(&host_registry(), CHECK_WORKFLOW, &Overrides::none());
    let mut stages: Vec<String> = plan.stage_names().iter().map(|name| name.to_string()).collect();
    stages.sort();
    for check in [CONTRACT_CHECK, MATH_CHECK, CHECK_LABELS] {
        assert!(stages.iter().any(|name| name == check), "{check} is not planned: {stages:?}");
    }
    assert!(plan.active_backends().is_empty());
}

#[test]
fn the_check_workflow_reports_the_unresolved_reference_and_nothing_else() {
    let run = run_workflow(&host_registry(), CHECK_WORKFLOW, &[DOCUMENT], &Overrides::none());
    assert_eq!(identifiers(&run), ["flm.refs.unresolved"]);
}
