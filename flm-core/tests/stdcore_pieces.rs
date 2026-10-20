//! Pieces end to end over the text backend: what a feature declares to offer a
//! piece, which pieces a render places, and where the text output prints them.
//!
//! The tests build a registry from the `stdcore` package plus the test
//! feature of `common::pieces`. For each piece it is given, that feature
//! declares the piece's table (of type `Piece`), a document-scope builder
//! stage writing it, and the piece's declaration, contributed to the slot
//! `flm.pieces` as a peer. What the builder writes is fixed per piece: a value
//! parsed from a given source, a copy of the first document tree's content,
//! nothing at all.

mod common;

use std::sync::Arc;

use techy::error::Severity;

use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::pieces::{self, PiecesRenderOptions};
use flm_core::stdcore::backends::text::{self, TEXT_OUTPUT};
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::{StdcoreConfig, check_feature};

use common::pieces::{Content, TestPiece, registry};
use common::stdcore::reports;

// ---------------------------------------------------------------------------------------------
// running a document
// ---------------------------------------------------------------------------------------------

/// Overrides setting the pieces option.
fn with_options(options: PiecesRenderOptions) -> Overrides {
    Overrides { render_options: OptionValues::new().with(options), ..Overrides::none() }
}

/// The text plan under `overrides`.
fn text_plan(registry: &Arc<FlmRegistry>, overrides: &Overrides) -> Arc<Plan> {
    common::stdcore::plan(registry, text::WORKFLOW, overrides)
}

/// A text run of one tree per text under `overrides`, executed `options`'s way.
fn render_under(
    registry: &Arc<FlmRegistry>,
    texts: &[&str],
    overrides: &Overrides,
    options: &RunOptions,
) -> Run {
    let doc = common::pipeline::doc(registry, texts);
    let plan = text_plan(registry, overrides);
    common::pipeline::run_with(&doc, &plan, Externals::none(), options)
}

/// A sequential text run of one tree per text under `overrides`.
fn render(registry: &Arc<FlmRegistry>, texts: &[&str], overrides: &Overrides) -> Run {
    render_under(registry, texts, overrides, &RunOptions::sequential())
}

/// The printed text of a run.
fn printed(run: &Run) -> String {
    run.table(&TEXT_OUTPUT)
        .expect("the text output is declared")
        .expect("the text output was written")
        .text
        .clone()
}

/// The printed text of one tree per text under the given pieces, with the
/// default options.
fn print(pieces: Vec<TestPiece>, texts: &[&str]) -> String {
    let run = render(&registry(pieces), texts, &Overrides::none());
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    printed(&run)
}

// ---------------------------------------------------------------------------------------------
// placing a piece
// ---------------------------------------------------------------------------------------------

#[test]
fn a_piece_on_by_default_is_printed_after_the_content() {
    let out = print(vec![TestPiece::after("notes", Content::Source("Some notes."))], &["Body."]);
    assert_eq!(out, "Body.\n\nSome notes.\n");
}

#[test]
fn an_excluded_piece_is_left_out_and_its_builder_is_not_scheduled() {
    let piece = TestPiece::after("notes", Content::Source("Some notes."));
    let registry = registry(vec![piece.clone()]);
    let placed = text_plan(&registry, &Overrides::none());
    assert!(placed.stage_names().iter().any(|stage| stage.as_str() == piece.builder()));
    let overrides = with_options(PiecesRenderOptions::default().exclude(piece.name()));
    let left_out = text_plan(&registry, &overrides);
    assert!(!left_out.stage_names().iter().any(|stage| stage.as_str() == piece.builder()));
    let run = render(&registry, &["Body."], &overrides);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    assert_eq!(printed(&run), "Body.\n");
}

#[test]
fn a_piece_off_by_default_is_placed_only_when_included() {
    let piece = TestPiece::after("toc", Content::Source("Contents.")).off();
    let registry = registry(vec![piece.clone()]);
    let plan = text_plan(&registry, &Overrides::none());
    assert!(!plan.stage_names().iter().any(|stage| stage.as_str() == piece.builder()));
    assert_eq!(printed(&render(&registry, &["Body."], &Overrides::none())), "Body.\n");
    let overrides = with_options(PiecesRenderOptions::default().include(piece.name()));
    assert_eq!(printed(&render(&registry, &["Body."], &overrides)), "Body.\n\nContents.\n");
}

#[test]
fn a_piece_holding_no_content_places_nothing_and_reports_nothing() {
    let out = print(vec![TestPiece::after("notes", Content::Nothing)], &["Body."]);
    assert_eq!(out, "Body.\n");
}

#[test]
fn a_piece_whose_value_is_empty_places_nothing_and_reports_nothing() {
    let out = print(vec![TestPiece::after("notes", Content::Source(""))], &["Body."]);
    assert_eq!(out, "Body.\n");
}

#[test]
fn a_piece_placed_before_the_content_prints_before_the_trees() {
    let out = print(
        vec![TestPiece::before("title", Content::Source("A title."))],
        &["First tree.", "Second tree."],
    );
    assert_eq!(out, "A title.\n\nFirst tree.\n\nSecond tree.\n");
}

#[test]
fn two_pieces_print_in_the_order_they_were_declared() {
    let out = print(
        vec![
            TestPiece::after("second", Content::Source("Declared first.")),
            TestPiece::after("first", Content::Source("Declared second.")),
            TestPiece::before("head", Content::Source("Before.")),
        ],
        &["Body."],
    );
    assert_eq!(out, "Before.\n\nBody.\n\nDeclared first.\n\nDeclared second.\n");
}

// ---------------------------------------------------------------------------------------------
// what a piece's value may hold
// ---------------------------------------------------------------------------------------------

#[test]
fn a_reference_inside_a_piece_resolves_to_a_label_of_the_document() {
    let document = "\\section{Intro}\\label{sec:a}\n\nSee \\ref{sec:a}.";
    let body = print(Vec::new(), &[document]);
    let with_piece =
        print(vec![TestPiece::after("notes", Content::Source("Also \\ref{sec:a}."))], &[document]);
    let shown = body.lines().last().expect("a last line").trim_start_matches("See ");
    assert_eq!(with_piece, format!("{body}\nAlso {shown}\n"));
}

#[test]
fn a_piece_stating_a_labeled_point_is_refused_and_the_run_goes_on() {
    let registry = registry(vec![TestPiece::after(
        "notes",
        Content::Source("\\section{Inside}\\label{sec:inside}"),
    )]);
    let run = render(&registry, &["Body."], &Overrides::none());
    assert!(
        reports(&run).contains(&("flm.pieces.lowering-failed".to_string(), Severity::Error)),
        "{}",
        run.diagnostics().render_all()
    );
    assert_eq!(printed(&run), "Body.\n");
}

#[test]
fn a_piece_holding_another_features_elements_is_lowered_by_that_features_rules() {
    let out = print(
        vec![TestPiece::after(
            "paragraphs",
            Content::Source("First paragraph.\n\nSecond \\emph{one}, \\textbf{bold}."),
        )],
        &["Body."],
    );
    assert_eq!(out, "Body.\n\nFirst paragraph.\n\nSecond one, bold.\n");
}

/// For a piece holding `source`: the text the run prints, and the table each
/// diagnostic names, in the order reported, each with its identifier and
/// severity.
fn print_list_piece(source: &'static str) -> (String, Vec<(String, Severity, String)>) {
    let registry = registry(vec![TestPiece::after("list", Content::Source(source))]);
    let run = render(&registry, &["Body."], &Overrides::none());
    let reported = run
        .diagnostics()
        .iter()
        .map(|d| {
            let message = d.to_string();
            let table = message
                .split("in table `")
                .nth(1)
                .and_then(|rest| rest.split('`').next())
                .unwrap_or_default()
                .to_string();
            (d.identifier().to_string(), d.severity(), table)
        })
        .collect();
    (printed(&run), reported)
}

/// A missing entry of `table`, as [`print_list_piece`] lists it.
fn missing_entry(table: &str) -> (String, Severity, String) {
    ("flm.run.missing-entry".to_string(), Severity::Error, table.to_string())
}

#[test]
fn a_bulleted_list_inside_a_piece_has_no_anchors_and_reports_the_missing_entry() {
    // A list item is a reference point. A rule inside a value looks its entry
    // up by the value's own address, which the document-scope anchors table
    // does not hold: the items print, and the list reports the missing entry
    // once. (Content of the document shown in a piece — an xnote's in its
    // list — is pointed at rather than copied, and finds its entries.)
    let (out, reported) =
        print_list_piece("\\begin{itemize}\\item One\\item Two\\end{itemize}");
    assert_eq!(out, "Body.\n\n• One\n\n• Two\n");
    assert_eq!(reported, [missing_entry("flm.anchors")]);
}

#[test]
fn a_numbered_list_inside_a_piece_has_no_numbers_and_reports_both_missing_entries() {
    // As above, and the counters table holds no number for the value's items
    // either: the items print unnumbered, and the list reports each table's
    // missing entry once.
    let (out, reported) =
        print_list_piece("\\begin{enumerate}\\item One\\item Two\\end{enumerate}");
    assert_eq!(out, "Body.\n\nOne\n\nTwo\n");
    assert_eq!(reported, [missing_entry("flm.counters"), missing_entry("flm.anchors")]);
}

#[test]
fn a_piece_placing_a_tree_prints_nothing_there_and_is_reported() {
    // `\tpplace` stages a node placing the first tree; inside a piece it
    // places nothing, and the tree is printed where it stands, once.
    let registry = registry(vec![TestPiece::after("notes", Content::Source("Before.\n\n\\tpplace"))]);
    let run = render(&registry, &["Body."], &Overrides::none());
    assert_eq!(reports(&run), [("flm.text.piece-placement".to_string(), Severity::Error)]);
    assert_eq!(printed(&run), "Body.\n\nBefore.\n");
}

#[test]
fn a_piece_copied_from_the_document_prints_the_copy() {
    let out = print(vec![TestPiece::before("again", Content::CopyOfFirstTree)], &["Body."]);
    assert_eq!(out, "Body.\n\nBody.\n");
}

// ---------------------------------------------------------------------------------------------
// the option, the registry, the execution
// ---------------------------------------------------------------------------------------------

#[test]
fn a_name_in_the_option_no_feature_declares_is_a_warning_and_ignored() {
    let registry = registry(vec![TestPiece::after("notes", Content::Source("Notes."))]);
    let overrides = with_options(PiecesRenderOptions::default().include("tp.nothing"));
    let run = render(&registry, &["Body."], &overrides);
    assert_eq!(reports(&run), [("flm.pieces.unknown-piece".to_string(), Severity::Warning)]);
    assert_eq!(printed(&run), "Body.\n\nNotes.\n");
}

#[test]
fn a_registry_with_a_backend_and_without_the_pieces_feature_fails_every_plan() {
    let registry = common::stdcore::registry_over(vec![
        flm_core::stdcore::backends::lowering::feature(),
        text::feature(),
        flm_core::stdcore::backends::html::feature(),
        Arc::new(Blocks::default()),
        check_feature(),
    ]);
    // Every registered stage is asked what it needs whatever the plan
    // schedules, so the check workflow fails as the two backends' do.
    for workflow in
        [text::WORKFLOW, flm_core::stdcore::backends::html::WORKFLOW, flm_core::stdcore::CHECK_WORKFLOW]
    {
        let error = common::stdcore::try_plan(&registry, workflow, &Overrides::none())
            .expect_err("the plan must fail without the pieces feature");
        assert!(error.to_string().contains("PiecesRenderOptions"), "{workflow}: {error}");
    }
}

#[test]
fn the_check_workflow_schedules_no_pieces_stage_and_no_builder() {
    let piece = TestPiece::after("notes", Content::Source("Notes."));
    let registry = registry(vec![piece.clone()]);
    let plan = common::stdcore::plan(&registry, flm_core::stdcore::CHECK_WORKFLOW, &Overrides::none());
    for stage in plan.stage_names() {
        assert!(!stage.as_str().ends_with(".pieces"), "{stage}");
        assert_ne!(stage.as_str(), piece.builder());
    }
}

#[test]
fn the_parallel_and_the_sequential_runs_print_the_same_bytes_with_pieces() {
    let registry = registry(vec![
        TestPiece::before("head", Content::Source("Before \\ref{sec:a}.")),
        TestPiece::after("notes", Content::Source("After.\n\nAnd \\emph{more}.")),
        TestPiece::after("again", Content::CopyOfFirstTree),
    ]);
    // The first tree, which one piece copies, states no labeled point.
    let texts = ["One.", "\\section{A}\\label{sec:a}\n\nTwo \\ref{sec:a}.", "Three."];
    let sequential = render_under(&registry, &texts, &Overrides::none(), &RunOptions::sequential());
    let parallel = render_under(&registry, &texts, &Overrides::none(), &RunOptions::new());
    assert_eq!(printed(&sequential), printed(&parallel));
    // Every table the run computed, the lowered pieces among them, and every
    // diagnostic.
    let expected = common::spine::snapshot(&sequential);
    assert_eq!(common::spine::snapshot(&parallel), expected);
    assert!(expected.contains("flm.lowered.text.pieces = "), "{expected}");
    assert!(!sequential.diagnostics().any_recorded(), "{}", sequential.diagnostics().render_all());
}

#[test]
fn a_registry_declaring_no_piece_prints_what_it_printed_before() {
    let texts = ["\\section{A}\\label{sec:a}\n\nOne \\ref{sec:a}.", "Two."];
    // The standard package without the three features declaring pieces.
    let without = common::stdcore::registry_over(
        common::stdcore::package_features_but(StdcoreConfig::new(), "xnotes")
            .into_iter()
            .filter(|feature| !["citations", "meta"].contains(&feature.info().name.as_str()))
            .collect(),
    );
    let run = render(&without, &texts, &Overrides::none());
    let with = render(&registry(Vec::new()), &texts, &Overrides::none());
    assert_eq!(printed(&run), printed(&with));
    assert!(pieces::selected_pieces(&without, &PiecesRenderOptions::default(), &"text".into(), false).is_empty());
    // The standard package itself declares the list of footnotes, the
    // bibliography and the metadata's three pieces, which a document holding
    // no footnote, no citation and no metadata leaves empty: nothing is placed.
    let standard = render(&common::stdcore::registry(StdcoreConfig::new()), &texts, &Overrides::none());
    assert_eq!(printed(&standard), printed(&with));
}
