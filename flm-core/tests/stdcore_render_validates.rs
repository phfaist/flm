//! One document, one verdict, whatever workflow runs it: every rendering
//! workflow of `flm_core::stdcore` demands the `flm.validated` collection
//! beside its output, so the checks that collection gathers run in a render
//! exactly as they run under the `check` workflow.
//!
//! The three documents here each break one rule the collection's members
//! check — a label of a type its element refuses, a label written twice, and
//! the mathematics content check's own stage — and each is run under all four
//! standard workflows: `check`, `text`, `latex-recomposed` and `html`. What
//! differs by workflow is one thing only, pinned at the end: an unresolved
//! reference is a Warning under `check` and an Error under a render.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::dump::diag::expect;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::math_passthrough::CHECK as MATH_CHECK;
use flm_core::stdcore::features::refs::CHECK_LABELS;
use flm_core::stdcore::features::blocks::CONTRACT_CHECK;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig};
use techy::error::Severity;

use common::stdcore::{identifiers, plan, registry, run_workflow};

/// The four workflows the standard package registers, in the order the tests
/// below report them.
const WORKFLOWS: [&str; 4] =
    [CHECK_WORKFLOW, text::WORKFLOW, latex_recomposed::WORKFLOW, html::WORKFLOW];

/// The registry of the standard package under its default configuration.
fn std_registry() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// Runs `document` under `workflow` over the standard package.
fn run(workflow: &str, document: &str) -> Run {
    run_workflow(&std_registry(), workflow, &[document], &Overrides::none())
}

/// A heading accepts the label types `sec` and `topic` alone, so `eq:` on a
/// section is refused; the refused label defines nothing, which leaves the
/// reference to it unresolved.
const REFUSED_TYPE: &str = "\\section{Sum}\\label{eq:sum}\n\nSee \\ref{eq:sum}.\n";

/// One label written on two headings.
const DUPLICATE: &str = "\\section{A}\\label{sec:a}\n\nText.\n\n\\section{B}\\label{sec:a}\n";

#[test]
fn every_workflow_reports_a_label_of_a_type_its_element_refuses() {
    for workflow in WORKFLOWS {
        let run = run(workflow, REFUSED_TYPE);
        let diagnostic =
            expect(run.diagnostics().iter(), "flm.refs.label-type-not-accepted").once();
        assert_eq!(diagnostic.severity(), Severity::Error, "under `{workflow}`");
    }
}

#[test]
fn every_workflow_reports_a_duplicate_label() {
    for workflow in WORKFLOWS {
        let run = run(workflow, DUPLICATE);
        assert_eq!(identifiers(&run), ["flm.refs.duplicate-label"], "under `{workflow}`");
        let diagnostic = expect(run.diagnostics().iter(), "flm.refs.duplicate-label").once();
        assert_eq!(diagnostic.severity(), Severity::Error, "under `{workflow}`");
    }
}

/// The three stages the collection gathers are scheduled by every workflow.
/// The mathematics content check of the standard `math` feature passes every
/// region through, so what a test can pin about it is that it is planned:
/// a feature replacing it with a checking stage of its own is then run by a
/// render as it is by `check`.
#[test]
fn every_workflow_schedules_every_member_of_the_validation_collection() {
    let registry = std_registry();
    for workflow in WORKFLOWS {
        let plan = plan(&registry, workflow, &Overrides::none());
        let names: Vec<String> =
            plan.scheduled_names().iter().map(|name| name.to_string()).collect();
        for check in [CONTRACT_CHECK, MATH_CHECK, CHECK_LABELS] {
            assert!(names.contains(&check.to_string()), "`{check}` is not in the `{workflow}` plan");
        }
    }
}

/// The one severity that depends on the workflow. A plan that renders nothing
/// cannot know whether a label would resolve in a plan that does — a render may
/// install an index of labels defined elsewhere — so `check` warns where a
/// render errs.
#[test]
fn an_unresolved_reference_is_a_warning_under_check_and_an_error_under_a_render() {
    let document = "See \\ref{sec:missing}.\n";
    let checked = run(CHECK_WORKFLOW, document);
    assert_eq!(
        expect(checked.diagnostics().iter(), "flm.refs.unresolved").once().severity(),
        Severity::Warning
    );
    for workflow in [text::WORKFLOW, latex_recomposed::WORKFLOW] {
        let rendered = run(workflow, document);
        assert_eq!(
            expect(rendered.diagnostics().iter(), "flm.refs.unresolved").once().severity(),
            Severity::Error,
            "under `{workflow}`"
        );
    }
}
