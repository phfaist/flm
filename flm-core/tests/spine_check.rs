//! The `check` workflow of `flm_core::stdcore` over whole documents: validation
//! with no output — a clean document reports nothing, a duplicate label is an
//! error, and an unresolved reference is a warning (a check plan renders
//! nothing, so a reference only might fail to resolve) — each condition is
//! reported once, every contributed check is scheduled, and no backend is
//! active.

mod common;

use flm_core::core::plan::Overrides;
use flm_core::dump::diag::expect;
use flm_core::stdcore::CHECK_WORKFLOW;
use flm_core::stdcore::backends::text::TEXT_OUTPUT;
use flm_core::stdcore::features::blocks::CONTRACT_CHECK;
use flm_core::math_passthrough::CHECK as MATH_CHECK;
use flm_core::stdcore::features::refs::CHECK_LABELS;
use techy::error::Severity;

use common::spine::{FEATURES_DOCUMENT, SESSION06_DOCUMENT, registry};
use common::stdcore::{identifiers, plan, run_workflow};

/// A document declaring one label twice.
const DUPLICATE: &str = "\\section{A}\\label{sec:a}\n\nText.\n\n\\section{B}\\label{sec:a}\n";

#[test]
fn a_clean_document_passes_the_check_workflow_with_no_diagnostic() {
    let run = run_workflow(&registry(), CHECK_WORKFLOW, &[SESSION06_DOCUMENT], &Overrides::none());
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

#[test]
fn the_check_workflow_reports_an_unresolved_reference_as_a_warning() {
    let run = run_workflow(&registry(), CHECK_WORKFLOW, &[FEATURES_DOCUMENT], &Overrides::none());
    assert_eq!(identifiers(&run), ["flm.refs.unresolved"]);
    let diagnostic =
        expect(run.diagnostics().iter(), "flm.refs.unresolved").at("\\ref{sec:missing}").once();
    assert_eq!(diagnostic.severity(), Severity::Warning);
    assert!(!run.diagnostics().has_errors());
}

#[test]
fn the_check_workflow_reports_a_duplicate_label_as_an_error() {
    let run = run_workflow(&registry(), CHECK_WORKFLOW, &[DUPLICATE], &Overrides::none());
    assert_eq!(identifiers(&run), ["flm.refs.duplicate-label"]);
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.duplicate-label").once();
    assert_eq!(diagnostic.severity(), Severity::Error);
    assert!(run.diagnostics().has_errors());
}

#[test]
fn the_check_workflow_reports_each_condition_of_a_document_once() {
    let document = format!("{DUPLICATE}\nSee \\ref{{sec:nowhere}}.\n");
    let run = run_workflow(&registry(), CHECK_WORKFLOW, &[&document], &Overrides::none());
    let mut found = identifiers(&run);
    found.sort();
    assert_eq!(found, ["flm.refs.duplicate-label", "flm.refs.unresolved"]);
}

#[test]
fn the_check_workflow_writes_no_output_and_activates_no_backend() {
    let plan = plan(&registry(), CHECK_WORKFLOW, &Overrides::none());
    assert!(plan.active_backends().is_empty());
    let run = run_workflow(&registry(), CHECK_WORKFLOW, &[SESSION06_DOCUMENT], &Overrides::none());
    assert!(run.table(TEXT_OUTPUT).expect("declared by the package").is_none());
}

#[test]
fn the_check_plan_over_the_spine_registry_schedules_every_contributed_check() {
    let plan = plan(&registry(), CHECK_WORKFLOW, &Overrides::none());
    let stages: Vec<String> = plan.stage_names().iter().map(|name| name.to_string()).collect();
    for check in [CONTRACT_CHECK, MATH_CHECK, CHECK_LABELS] {
        assert!(stages.iter().any(|name| name == check), "{check} is not planned: {stages:?}");
    }
    // The spine's `always` definition schedules its expansion here too.
    let rewrites: Vec<String> = plan.rewrite_names().iter().map(|name| name.to_string()).collect();
    assert_eq!(
        rewrites,
        ["flm.input.splice", "flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]
    );
}
