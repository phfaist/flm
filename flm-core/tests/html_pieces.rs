//! Pieces over the HTML backend: where the page places them, what their
//! content requires, and what a piece may not hold.
//!
//! The tests build a registry from the `stdcore` package plus the test
//! feature of `common::pieces`, which declares each piece it is given — a
//! table, a builder stage, a declaration — and the command `\tpplace`, whose
//! rule stages a node placing the document's first tree.

mod common;

use std::sync::Arc;

use techy::error::Severity;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::stdcore::backends::html::{self, DEFINITIONS_TABLE, PIECES_TABLE};

use common::html::{assert_golden, page};
use common::pieces::{Content, TestPiece, registry};
use common::stdcore::{reports, run_workflow};

/// An `html` run of one tree per text.
fn render(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Run {
    run_workflow(registry, html::WORKFLOW, texts, &Overrides::none())
}

/// The part of `page` inside `<main …>` and `</main>`.
fn main_of(page: &str) -> &str {
    let start = page.find("<main").expect("the page has a <main>");
    let start = start + page[start..].find('>').expect("the <main> tag closes") + 1;
    let end = page.find("</main>").expect("the page closes its <main>");
    &page[start..end]
}

#[test]
fn the_page_places_a_piece_after_the_trees_inside_main_as_block_markup() {
    let registry = registry(vec![
        TestPiece::after("notes", Content::Source("Some notes.\n\nMore notes.")),
        TestPiece::before("title", Content::Source("A title.")),
    ]);
    let run = render(&registry, &["First tree.", "Second tree."]);
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
    let page = page(&run);
    let main = main_of(&page);
    let at = |text: &str| main.find(text).unwrap_or_else(|| panic!("{text} in {main}"));
    assert!(at("A title.") < at("First tree."));
    assert!(at("First tree.") < at("Second tree."));
    assert!(at("Second tree.") < at("Some notes."));
    // Block markup, as a tree's: each piece starts a line of its own, and a
    // piece holding paragraphs writes them as a tree's are written.
    let lines: Vec<&str> = main.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    assert_eq!(lines, ["A title.", "First tree.", "Second tree.", "<p class=\"flm-pres-par\">Some notes.</p>", "<p class=\"flm-pres-par\">More notes.</p>"], "{main}");
    assert_golden("pieces.page.html", &page);
}

#[test]
fn the_pieces_html_is_its_own_table() {
    let registry = registry(vec![TestPiece::after("notes", Content::Source("Some notes."))]);
    let run = render(&registry, &["Body."]);
    let pieces = run.table(&PIECES_TABLE).expect("declared").expect("written");
    assert_eq!(pieces.pieces.len(), 1);
    assert_eq!(pieces.pieces[0].name.as_str(), "tp.notes");
    assert!(pieces.pieces[0].fragment.html().contains("Some notes."));
}

#[test]
fn a_pieces_requirements_reach_the_definitions_table() {
    let registry = registry(vec![TestPiece::after("notes", Content::Source("Math \\(x^2\\)."))]);
    let without = render(&registry_without_pieces(), &["Body."]);
    let with = render(&registry, &["Body."]);
    let ids = |run: &Run| -> Vec<String> {
        run.table(&DEFINITIONS_TABLE)
            .expect("declared")
            .expect("written")
            .definitions
            .iter()
            .map(|definition| definition.id.to_string())
            .collect()
    };
    assert!(!ids(&without).iter().any(|id| id == "flm.html.mathjax"), "{:?}", ids(&without));
    assert!(ids(&with).iter().any(|id| id == "flm.html.mathjax"), "{:?}", ids(&with));
    assert!(ids(&with).iter().any(|id| id == "flm.html.style.math-passthrough"), "{:?}", ids(&with));
    assert!(page(&with).contains("data-flm-def=\"flm.html.mathjax\""));
}

#[test]
fn a_subtree_node_inside_a_piece_writes_nothing_and_is_reported() {
    let registry =
        registry(vec![TestPiece::after("notes", Content::Source("Before.\n\n\\tpplace"))]);
    let run = render(&registry, &["Body."]);
    assert_eq!(reports(&run), [("flm.html.piece-placement".to_string(), Severity::Error)]);
    // The report points at the placing command, on the value's third line.
    let rendered = run.diagnostics().render_all();
    assert!(rendered.contains("synthesized from @ (line 3, col 1)"), "{rendered}");
    let page = page(&run);
    // The first tree is written once, where it stands.
    assert_eq!(main_of(&page).matches("Body.").count(), 1);
    assert!(main_of(&page).contains("Before."));
}

/// The registry of the standard package and no piece.
fn registry_without_pieces() -> Arc<FlmRegistry> {
    registry(Vec::new())
}
