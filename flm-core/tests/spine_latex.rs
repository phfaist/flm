//! The whole pipeline through the `latex-recomposed` workflow, over a registry
//! built from `flm_core::stdcore::package` alone: the recomposed LaTeX and style
//! file of the document exercising every standard feature and of the
//! session-06 document as goldens, the lowered projection and the tables of
//! the former, the plan the workflow builds, and the two-backends proof — one
//! document, one registry, the `text` plan and the `latex-recomposed` plan run
//! one after the other with the frozen trees differing as the plans say, and
//! one request demanding both outputs refused, since the two tree states are
//! exclusive providers. The text-workflow counterpart is `spine_text.rs`; the
//! backend's own suite, one property per golden, is `latex_recomposed.rs`.
//!
//! The LaTeX goldens hold the recomposed LaTeX, then the style file, each under a
//! marker line that is itself a LaTeX comment (`common::latex::golden_text`),
//! and `latex_compile.rs` compiles them under `pdflatex` and `lualatex` when
//! `FLM_TESTS_LATEX_COMPILE` is set.

mod common;

use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan, PlanError, PlanRequest};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::golden::{assert_json_golden, assert_text_golden};
use flm_core::dump::project::{ProjectOptions, project_tree};
use flm_core::model::addr::TreeId;
use flm_core::model::names::BackendName;
use flm_core::stdcore::backends::latex_recomposed::{
    self, BACKEND, OUTPUT_TABLE, PACKAGES_TABLE, PREAMBLE_TABLE, WORKFLOW,
};
use flm_core::stdcore::backends::lowering::lowered_table;
use flm_core::stdcore::backends::text::{self, TEXT_DOCUMENT};
use flm_core::stdcore::features::refs::{SAFE_LABELS, UnresolvedRefs};
use techy::serialize::SerialValue;

use common::latex::{body, golden_text, style, output};
use common::lowering::text as text_of;
use common::pipeline;
use common::spine::{
    FEATURES_DOCUMENT, SESSION06_DOCUMENT, golden_path, overrides, registry, tables_projection,
    text_plan,
};
use common::stdcore::{identifiers, plan, run_workflow};
use common::wiring::count_element;

/// The features document recomposed by the `latex-recomposed` workflow over
/// the spine's registry, sequentially, with no overrides.
fn recomposed(document: &str) -> Run {
    run_workflow(&registry(), WORKFLOW, &[document], &Overrides::none())
}

/// The projection of the first tree's lowered form for the LaTeX backend.
fn lowered_projection(run: &Run) -> SerialValue {
    let lowered = run
        .table(lowered_table(&BackendName::from(BACKEND)))
        .expect("the lowered table is declared")
        .and_then(|trees| trees.get(TreeId::tree(0)))
        .unwrap_or_else(|| {
            panic!("the tree was not lowered; diagnostics:\n{}", run.diagnostics().render_all())
        });
    project_tree(&lowered.tree, &ProjectOptions::new())
}

// --- the plan --------------------------------------------------------------------------------

#[test]
fn the_workflow_plans_the_raw_tree_the_safe_labels_the_kept_definitions_and_the_backend() {
    // The spine's registry holds two per-plan definitions and one `always`
    // definition, so the plan keeps the former and expands the latter first.
    let plan = plan(&registry(), WORKFLOW, &Overrides::none());
    let names = |names: &[flm_core::model::names::StageName]| {
        names.iter().map(|name| name.to_string()).collect::<Vec<_>>()
    };
    assert_eq!(names(&plan.rewrite_names()), ["flm.input.splice", "flm.userdefs.expand-always"]);
    // The workflow demands `flm.validated` beside its output, so the three
    // checks and the stages they read are planned with the backend's own.
    assert_eq!(
        names(&plan.stage_names()),
        [
            "flm.blocks.raw",
            "flm.index.build",
            "flm.refpoints.scan",
            "flm.numbering.number",
            "flm.refs.scan",
            "flm.refs.anchors",
            "flm.refs.resolve",
            "flm.refs.check-labels",
            "flm.refs.safe-labels",
            "flm.refs.value-safe-labels",
            "flm.theorems.check-proofs",
            "flm.graphics.inspect",
            "flm.graphics.publish.latex-recomposed",
            "flm.floats.check-handlers",
            "flm.citations.collect",
            "flm.citations.retrieve",
            "flm.citations.check",
            "flm.userdefs.keep",
            "flm.blocks.check-contract",
            "flm.meta.build",
            "flm.latex-recomposed.pieces",
            "flm.latex-recomposed.lower",
            "flm.latex-recomposed.emit",
            "flm.latex-recomposed.regions",
            "flm.latex-recomposed.final",
            "flm.math-passthrough.check",
        ]
    );
    assert_eq!(plan.active_backends().iter().map(|b| b.to_string()).collect::<Vec<_>>(), [BACKEND]);
}

// --- the goldens: the step's deliverable ------------------------------------------------------

#[test]
fn the_features_document_is_recomposed_as_the_golden_shows() {
    let run = recomposed(FEATURES_DOCUMENT);
    let text = body(&run);
    // Every heading behind the hook, its labels in the labels hook, and every
    // reference through the reference hook — the one whose label nothing
    // defines too, which LaTeX reports in its turn. (The exact bytes
    // between the hook and the sectioning command are the golden's.)
    assert_eq!(text.matches("\\flmHeading").count(), 7, "{text}");
    assert!(text.contains("\\section{Introduction}{\\flmLabels{\\label{ref1}}}\n"), "{text}");
    assert!(text.contains("\\section*{Unnumbered}{\\flmLabels{}}\n"), "{text}");
    assert!(
        text.contains("See \\flmRef{ref1}, \\flmRef[noprefix]{ref2} and \\flmRef{ref3}.\n"),
        "{text}"
    );
    // The raw tree: the author's blank lines, comment and typography survive.
    assert!(text.contains("text --- with a dash, an ellipsis... and \"quotes\" resolved by typography.\n%% A comment vanishes"), "{text}");
    // The `always` definition is expanded; the per-plan ones are calls.
    assert!(text.contains("It is version 1.0.\n"), "{text}");
    assert!(
        text.contains(
            "\\flmUDhello{World} from a user macro, and \
             \\begin{flmUEnote}a user environment\\end{flmUEnote}\n"
        ),
        "{text}"
    );
    // Mathematics from its nodes, the environments rebuilt line by line.
    assert!(text.contains("\\begin{align*}\n  a &= b \\\\\n  c &= d\n\\end{align*}\n"), "{text}");
    // Verbatim through `\EscVerb` and the code environment.
    assert!(text.contains("Inline \\EscVerb[formatcom=\\flmFmtVRB{verbtext}]{a\\{b\\}\\\\c} text"), "{text}");
    assert!(text.contains("\\begin{flmVerbatimCode}[python]\ndef f(x):\n    return {x} + 1\n\\end{flmVerbatimCode}\n"), "{text}");
    let (style, path) = style(&run);
    assert!(path.starts_with("flmrecomposed.") && path.ends_with(".sty"), "{path}");
    for needed in [
        "\\RequirePackage{amsmath}",
        "\\RequirePackage{fvextra}",
        "\\RequirePackage{zref-clever}",
        "\\providecommand\\flmUDhello[1]{Hello, #1!}",
        "\\ProvideDocumentEnvironment{flmUEnote}{}{[Note: }{]}",
    ] {
        assert!(style.contains(needed), "{needed} is missing from the style file:\n{style}");
    }
    // The recomposed LaTeX validates like every render: the document's one deliberately
    // dangling reference is reported here, and changes no byte of the output.
    assert_eq!(identifiers(&run), ["flm.refs.unresolved"]);
    assert_text_golden(golden_path("features-recomposed.tex"), &golden_text(&run));
}

#[test]
fn the_session_06_document_is_recomposed_as_the_golden_shows() {
    let run = recomposed(SESSION06_DOCUMENT);
    let text = body(&run);
    assert!(text.contains("\\flmHeading"), "{text}");
    assert!(text.contains("\\section{Introduction}{\\flmLabels{\\label{ref1}}}\n"), "{text}");
    assert!(text.contains("See \\flmRef{ref1} and \\emph{this emphasized bit}"), "{text}");
    // The one non-ASCII character is written through and declared once.
    assert!(text.contains("long\nenough to wrap somewhere. \u{25a1}\n"), "{text}");
    let (style, _) = style(&run);
    assert!(style.contains("\\newunicodechar{\u{25a1}}{"), "{style}");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_text_golden(golden_path("session06-recomposed.tex"), &golden_text(&run));
}

#[test]
fn the_features_document_lowers_for_the_latex_backend_as_the_golden_shows() {
    let run = recomposed(FEATURES_DOCUMENT);
    assert_json_golden(golden_path("features-recomposed-lowered.json"), &lowered_projection(&run));
}

#[test]
fn the_tables_of_the_features_document_project_as_the_golden_shows() {
    // What `flm dump --table` prints for each of the backend's tables: the
    // safe labels both ways, the per-tree record (recomposed LaTeX, declared
    // characters, the preamble chunks their spellings need, the requirement
    // identifiers), the packages and the preamble pieces.
    let run = recomposed(FEATURES_DOCUMENT);
    let names = [
        SAFE_LABELS.name(),
        OUTPUT_TABLE.name(),
        PACKAGES_TABLE.name(),
        PREAMBLE_TABLE.name(),
    ];
    assert_json_golden(
        golden_path("features-recomposed-tables.json"),
        &tables_projection(&run, &names),
    );
}

// --- one document, two backends -----------------------------------------------------------------

#[test]
fn one_document_serves_both_backends_from_one_registry_in_two_runs() {
    let registry = registry();
    let doc = pipeline::doc(&registry, &[FEATURES_DOCUMENT]);
    let first = TreeId::tree(0);

    // The text plan: the trees structured into paragraphs, the definitions
    // expanded, the references resolved.
    let text_plan = text_plan(&registry, &overrides(Some(80), UnresolvedRefs::Allow));
    let text_run = pipeline::run_with(&doc, &text_plan, Externals::none(), &RunOptions::sequential());
    assert_text_golden(golden_path("features-80.txt"), &text_of(&text_run));
    let structured = text_run.frozen_tree(first).expect("the text run froze the tree");
    assert!(count_element(structured, "flm.paragraph") > 0);
    assert_eq!(count_element(structured, "flm.parbreak"), 0);
    assert_eq!(count_element(structured, "flm.userdefs.macro.hello"), 0, "expanded");

    // The LaTeX plan over the very same document: the raw tree, the
    // definitions kept, nothing resolved.
    let latex_plan = plan(&registry, WORKFLOW, &Overrides::none());
    let latex_run =
        pipeline::run_with(&doc, &latex_plan, Externals::none(), &RunOptions::sequential());
    assert_text_golden(golden_path("features-recomposed.tex"), &golden_text(&latex_run));
    let raw = latex_run.frozen_tree(first).expect("the LaTeX run froze the tree");
    assert_eq!(count_element(raw, "flm.paragraph"), 0);
    assert!(count_element(raw, "flm.parbreak") > 0);
    assert_eq!(count_element(raw, "flm.userdefs.macro.hello"), 1, "kept for the rule");
    assert!(!Arc::ptr_eq(structured, raw), "two plans, two frozen trees");

    // Each run holds its own output and not the other's.
    assert!(text_run.table(latex_recomposed::DOCUMENT).expect("declared").is_none());
    assert!(latex_run.table(TEXT_DOCUMENT).expect("declared").is_none());
}

#[test]
fn a_request_demanding_both_outputs_is_refused_naming_the_two_exclusive_providers() {
    // The text backend's rules require the structured tree and the LaTeX
    // backend's the raw one; the two states are exclusive refinements of the
    // block state, so no one plan provides both.
    let registry = registry();
    for (first, second) in [
        (TEXT_DOCUMENT.name(), latex_recomposed::DOCUMENT.name()),
        (latex_recomposed::DOCUMENT.name(), TEXT_DOCUMENT.name()),
    ] {
        let request = PlanRequest::new().demand(first).demand(second);
        match Plan::build(&registry, &request) {
            Err(PlanError::Unsatisfiable { capability, blocked, .. }) => {
                let providers: Vec<(&str, &str)> = blocked
                    .iter()
                    .map(|excluded| (excluded.stage.as_str(), excluded.excluded_by.as_str()))
                    .collect();
                assert!(
                    providers.contains(&("flm.blocks.raw", "flm.blocks.structure"))
                        || providers.contains(&("flm.blocks.structure", "flm.blocks.raw")),
                    "`{capability}` blocked by {providers:?}"
                );
            }
            other => panic!("expected the refusal, got {other:?}"),
        }
    }
    // Each output alone is planned.
    for demand in [TEXT_DOCUMENT.name(), latex_recomposed::DOCUMENT.name()] {
        Plan::build(&registry, &PlanRequest::new().demand(demand))
            .unwrap_or_else(|error| panic!("`{demand}` alone failed: {error}"));
    }
    // And the workflow of each name demands exactly that output.
    let demands = |workflow: &str| -> Vec<String> {
        plan(&registry, workflow, &Overrides::none())
            .demands()
            .iter()
            .map(|capability| capability.to_string())
            .collect()
    };
    // Each rendering workflow demands its own output and the members of the
    // `flm.validated` collection, which plan construction has already replaced
    // by their names.
    assert_eq!(
        demands(text::WORKFLOW),
        [
            "flm.blocks.contract-checked",
            "flm.citations.check",
            "flm.floats.check-handlers",
            "flm.input.spliced", "flm.math.content",
            "flm.meta.read",
            TEXT_DOCUMENT.name(),
            "flm.refs.check-labels",
            "flm.theorems.check-proofs",
        ]
    );
    assert_eq!(
        demands(WORKFLOW),
        [
            "flm.blocks.contract-checked",
            "flm.citations.check",
            "flm.floats.check-handlers",
            "flm.input.spliced", "flm.math.content",
            "flm.meta.read",
            latex_recomposed::DOCUMENT.name(),
            "flm.refs.check-labels",
            "flm.theorems.check-proofs",
        ]
    );
}

// --- determinism ------------------------------------------------------------------------------

#[test]
fn the_recomposed_latex_is_the_same_bytes_under_both_execution_paths() {
    // The goldens above are compared under every build setting of the
    // battery; this test compares the two runners inside one process over a
    // document of both spine documents.
    let registry = registry();
    let doc = pipeline::doc(&registry, &[FEATURES_DOCUMENT, SESSION06_DOCUMENT]);
    let plan = plan(&registry, WORKFLOW, &Overrides::none());
    let sequential = pipeline::run(&doc, &plan, &RunOptions::sequential());
    let parallel = pipeline::run(&doc, &plan, &RunOptions::new());
    let again = pipeline::run(&doc, &plan, &RunOptions::new());
    assert_eq!(golden_text(&sequential), golden_text(&parallel));
    assert_eq!(golden_text(&parallel), golden_text(&again));
    assert_eq!(identifiers(&sequential), identifiers(&parallel));
    // One safe-label count across the two trees, and one recomposed LaTeX per tree.
    let table = sequential.table(SAFE_LABELS).expect("declared").expect("written");
    assert_eq!(table.label_of("ref", "ref1"), Some("sec:intro"));
    assert_eq!(sequential.table(OUTPUT_TABLE).expect("declared").expect("written").trees.len(), 2);
    assert_eq!(output(&sequential).matches("%%\n").count(), 2);
}
