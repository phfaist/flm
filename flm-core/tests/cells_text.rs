//! The `cells` feature over the text workflow: the lowered tree (test 7), the
//! text output through the layout engine's table (test 8), the Errors and the
//! marks as the rules stage and print them (test 6), a registry without
//! `numbering` and `refs` or without `floats` (test 14), a user definition
//! writing a cell (test 17), where a table may stand under the switch (test
//! 19) and documents that must neither panic nor loop (test 15).

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::lang::PresPayload;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::pres::{
    self, CellAlign, CellHeader, CellsPayload, DroppedPayload, cells_of, payload_of,
};
use flm_core::stdcore::backends::text::{self, TextRenderOptions};
use flm_core::stdcore::features::cells::CellsConfig;
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::StdcoreConfig;

use common::lowering::{lowered, text as printed};
use common::stdcore::{frames, identifiers, run_workflow};

fn global() -> StdcoreConfig {
    StdcoreConfig::new().cells(CellsConfig::new().global(true))
}

fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(global())
}

fn render_with(registry: &Arc<FlmRegistry>, document: &str, overrides: &Overrides) -> Run {
    run_workflow(registry, text::WORKFLOW, &[document], overrides)
}

fn render(document: &str) -> Run {
    render_with(&registry(), document, &Overrides::none())
}

/// The text `document` renders to, its trailing newlines dropped.
fn text_of(run: &Run) -> String {
    printed(run).trim_end().to_string()
}

fn width(columns: u32) -> Overrides {
    Overrides {
        render_options: OptionValues::new()
            .with(TextRenderOptions { width: Some(columns), ..TextRenderOptions::default() }),
        ..Overrides::none()
    }
}

/// The payloads of every `flm.pres.cells` node of the lowered first tree, in
/// document order, with each node's number of slots.
fn tables(run: &Run) -> Vec<(CellsPayload, usize)> {
    let tree = &lowered(run, TreeId::tree(0)).tree;
    tree.root()
        .descendants()
        .filter(|node| element_of(node).name.as_str() == pres::CELLS)
        .map(|node| {
            let (payload, contents) = cells_of(&node).expect("a cells payload");
            (payload.clone(), contents.len())
        })
        .collect()
}

/// The payloads of every `flm.pres.dropped` mark of the lowered first tree.
fn marks(run: &Run) -> Vec<DroppedPayload> {
    let tree = &lowered(run, TreeId::tree(0)).tree;
    tree.root().descendants().filter_map(|node| payload_of::<DroppedPayload>(&node).cloned()).collect()
}

// --- test 7: the lowered tree -------------------------------------------------------------

#[test]
fn a_table_lowers_to_one_node_whose_payload_is_the_model() {
    let run = render(
        "\\begin{cells}\\celldata<H>{A & B}\\cell[\\merge{1-2}]{Wide}\\\\\\cell<r>{x}\\end{cells}",
    );
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let found = tables(&run);
    assert_eq!(found.len(), 1);
    let (payload, slots) = &found[0];
    assert_eq!((payload.rows, payload.cols), (3, 2));
    assert_eq!(*slots, payload.cells.len());
    let positions: Vec<(u32, u32, u32, u32)> =
        payload.cells.iter().map(|cell| (cell.row, cell.col, cell.row_span, cell.col_span)).collect();
    assert_eq!(positions, [(0, 0, 1, 1), (0, 1, 1, 1), (1, 0, 1, 2), (2, 0, 1, 1)]);
    assert_eq!(payload.cells[0].header, Some(CellHeader::Column));
    // The projection writes out and reads back equal.
    assert_eq!(&CellsPayload::from_projection(&payload.project()).expect("reads back"), payload);
    // A `\celldata` entry's slot holds one paragraph, and so does a `\cell`
    // holding one paragraph.
    let tree = &lowered(&run, TreeId::tree(0)).tree;
    let node = tree.root().descendants().find(|node| element_of(node).name.as_str() == pres::CELLS).unwrap();
    let (_, contents) = cells_of(&node).unwrap();
    for content in contents {
        let content = content.expect("a slot per entry");
        let names: Vec<String> = content.iter().map(|node| element_of(&node).name.to_string()).collect();
        assert_eq!(names, [pres::PAR]);
    }
}

#[test]
fn a_cell_holds_blocks_of_every_kind() {
    let run = render(
        "\\begin{cells}\\cell{One.\n\nTwo.}\\cell{\\begin{itemize}\\item a\\end{itemize}}\\\\\
         \\cell{\\[ x \\]}\\cell{A note\\footnote{B.}}\\\\\\cell{\\begin{cells}\\cell{inner}\\end{cells}}\
         \\end{cells}",
    );
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let found = tables(&run);
    assert_eq!(found.len(), 2, "the table and the one nested in a cell");
}

// --- test 8: the text output ----------------------------------------------------------------

#[test]
fn the_header_row_stands_above_its_rule_and_a_cells_alignment_is_its_own() {
    let run = render(
        "\\begin{cells}\n\\celldata<H>{Name & Count}\n\\celldata<l, r>{alpha & 1 \\\\ be & 22}\n\
         \\cell<c>{mid}\\end{cells}",
    );
    assert_eq!(text_of(&run), "Name   Count\n-----  -----\nalpha      1\nbe        22\n mid");
}

#[test]
fn a_spanned_cells_content_stands_in_its_first_position() {
    let run = render("\\begin{cells}\\cell[\\merge{1-2}]{Wide}\\\\\\cell{a}\\cell{b}\\end{cells}");
    assert_eq!(text_of(&run), "Wide\na     b");
}

#[test]
fn a_cell_of_two_paragraphs_wraps_in_its_column_under_the_width() {
    let run = render_with(
        &registry(),
        "\\begin{cells}\\cell{first words here\n\nsecond}\\cell{right column text}\\end{cells}",
        &width(20),
    );
    let text = text_of(&run);
    assert!(text.lines().all(|line| line.chars().count() <= 20), "{text}");
    assert!(text.contains("second"), "{text}");
    assert!(text.lines().count() > 3, "{text}");
}

#[test]
fn a_nested_table_is_laid_out_in_its_cell() {
    let run = render("\\begin{cells}\\cell{\\begin{cells}\\cell{a}\\cell{b}\\end{cells}}\\cell{c}\\end{cells}");
    assert_eq!(text_of(&run), "a  b  c");
}

// --- test 6: the Errors, the marks, their text form ---------------------------------------------

#[test]
fn stray_content_leaves_a_mark_before_the_table_and_nothing_of_it() {
    let run = render("\\begin{cells}\nHello world\n\\cell{A}\n\\end{cells}");
    assert_eq!(identifiers(&run), ["flm.cells.content-outside-cell"]);
    assert_eq!(text_of(&run), "[dropped: content outside \\cell]\n\nA");
}

#[test]
fn an_overlapping_cell_is_dropped_whole_with_one_mark() {
    let run = render("\\begin{cells}\\cell{A}\\cell[1]{B}\\end{cells}");
    assert_eq!(identifiers(&run), ["flm.cells.overlap"]);
    assert_eq!(text_of(&run), "[dropped: cell overlapping another]\n\nA");
}

#[test]
fn an_empty_table_writes_nothing() {
    let run = render("Before.\n\n\\begin{cells}\\end{cells}\n\nAfter.");
    assert_eq!(identifiers(&run), ["flm.cells.empty"]);
    assert_eq!(text_of(&run), "Before.\n\nAfter.");
    assert!(marks(&run).is_empty());
}

#[test]
fn a_cell_inside_a_cells_content_shows_its_content_where_it_stands() {
    let run = render("\\begin{cells}\\cell{x \\cell{y} z}\\end{cells}");
    assert_eq!(identifiers(&run), ["flm.cells.outside-table"]);
    let text = text_of(&run);
    assert!(text.contains('x') && text.contains('y') && text.contains('z'), "{text}");
}

#[test]
fn a_celldata_inside_a_celldata_shows_its_data_and_marks_its_ampersand() {
    let run = render("\\begin{cells}\\celldata{a & \\celldata{b & c}}\\end{cells}");
    let found = identifiers(&run);
    assert!(found.iter().filter(|id| *id == "flm.cells.outside-table").count() == 2, "{found:?}");
    let marks = marks(&run);
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].detail, "& out of place");
    let text = text_of(&run);
    assert!(text.contains("[dropped: & out of place]"), "{text}");
}

#[test]
fn a_nested_row_end_and_a_colnames_in_data_are_dropped_with_a_mark() {
    let run = render("\\begin{cells}\\celldata{\\emph{a \\\\ b} & \\colnames{x}}\\end{cells}");
    let found = identifiers(&run);
    assert_eq!(found, ["flm.cells.outside-table", "flm.cells.outside-table"], "{found:?}");
    let details: Vec<String> = marks(&run).into_iter().map(|mark| mark.detail).collect();
    assert_eq!(details, ["\\\\ out of place", "\\colnames out of place"]);
}

#[test]
fn a_cell_out_of_place_missing_its_content_leaves_a_mark() {
    let run = render("\\begin{cells}\\cell{x \\cell}\\end{cells}");
    let found = identifiers(&run);
    assert!(found.iter().any(|id| id == "flm.cells.outside-table"), "{found:?}");
    let details: Vec<String> = marks(&run).into_iter().map(|mark| mark.detail).collect();
    assert_eq!(details, ["\\cell out of place"]);
}

#[test]
fn every_report_is_made_again_with_its_frame_inside_a_copy() {
    let document = "\\begin{cells}\\cell{\\section*{T \\\\ x}\\label{sec:a}}\\end{cells}\n\nSee \\ref{sec:a}.";
    let run = render(document);
    let found = identifiers(&run);
    assert_eq!(found, ["flm.cells.outside-table", "flm.cells.outside-table"], "{}", run.diagnostics().render_all());
    let reference = document.find("\\ref{sec:a}").expect("the reference");
    let mut shown = frames(&run);
    shown.sort();
    assert_eq!(shown, [vec![], vec![(String::from(common::stdcore::SHOWN_HERE), reference)]]);
}

#[test]
fn every_error_is_reported_by_each_render() {
    let document = "\\begin{cells}\\cell<foo>[a;b;c]{A}\\cell[1]{B}stray\\end{cells}";
    for _ in 0..2 {
        let run = render(document);
        let mut found = identifiers(&run);
        found.sort();
        assert_eq!(
            found,
            [
                "flm.cells.bad-placement",
                "flm.cells.content-outside-cell",
                "flm.cells.overlap",
                "flm.cells.unknown-style"
            ]
        );
    }
}

// --- test 14: absence is ordinary -----------------------------------------------------------

#[test]
fn a_table_renders_without_numbering_and_refs() {
    let mut features = common::stdcore::package_features_but(global(), "refs");
    features.retain(|feature| feature.info().name.as_str() != "numbering");
    let registry = common::stdcore::registry_over(features);
    let run = render_with(&registry, "\\begin{cells}\\cell{A \\ref{x}}\\end{cells}", &Overrides::none());
    assert!(text_of(&run).starts_with('A'), "{}", text_of(&run));
}

#[test]
fn a_table_renders_without_floats_under_global() {
    let registry = common::stdcore::registry_over(common::stdcore::package_features_but(global(), "floats"));
    let run = render_with(&registry, "\\begin{cells}\\cell{A}\\cell{B}\\end{cells}", &Overrides::none());
    assert_eq!(text_of(&run), "A  B");
}

// --- test 17: a user definition writing a table ------------------------------------------------

/// A user definition's template is parsed where the definition is declared,
/// outside any table, where `\cell` is not a command: a template writes a
/// whole table, and the placeholders of a cell's styles and placement are read
/// at lowering, where the use site is expanded.
#[test]
fn a_user_definition_writing_a_table_places_its_cell_where_the_use_site_says() {
    let onecell = Definition::macro_(
        "onecell",
        vec![ArgDef::mandatory("styles"), ArgDef::mandatory("placement"), ArgDef::mandatory("content")],
    )
    .text("\\begin{cells}\\cell<#1>[#2]{#3}\\cell{x}\\end{cells}");
    let config = global().userdefs(UserdefsConfig::new().definitions(vec![onecell]));
    let registry = common::stdcore::registry(config);
    assert!(registry.build_diagnostics().is_empty(), "{}", registry.build_diagnostics().render_all());
    let run = render_with(&registry, "\\onecell{r H}{2}{right}", &Overrides::none());
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let (payload, _) = &tables(&run)[0];
    let first = &payload.cells[0];
    assert_eq!((first.row, first.col), (0, 1));
    assert_eq!(first.align, Some(CellAlign::Right));
    assert_eq!(first.header, Some(CellHeader::Column));
}

// --- test 19: where a table may stand under `global` ---------------------------------------------

#[test]
fn under_global_a_table_stands_wherever_blocks_stand() {
    for document in [
        "Before.\n\n\\begin{cells}\\cell{A}\\end{cells}\n\nAfter.",
        "\\begin{itemize}\\item \\begin{cells}\\cell{A}\\end{cells}\\end{itemize}",
        "Text\\footnote{\\begin{cells}\\cell{A}\\end{cells}}.",
        "\\begin{quote}\\block{\\begin{cells}\\cell{A}\\end{cells}}\\end{quote}",
    ] {
        let run = render(document);
        assert!(identifiers(&run).is_empty(), "{document}: {}", run.diagnostics().render_all());
        // A footnote's content is lowered in the list of footnotes, a piece of
        // its own: the table is in the text, not in the document's tree.
        assert!(text_of(&run).lines().any(|line| line.trim() == "A"), "{document}: {}", text_of(&run));
    }
}

// --- test 15: documents that must neither panic nor loop ---------------------------------------

#[test]
fn hard_documents_render() {
    let nested = (0..20).fold(String::from("x"), |inner, _| format!("\\begin{{cells}}\\cell{{{inner}}}\\end{{cells}}"));
    for document in [
        String::from("\\begin{cells}\\end{cells}"),
        String::from("\\begin{cells}%% only\n\n%% comments\n\\end{cells}"),
        String::from("\\begin{cells}\\\\\\\\\\\\\\end{cells}"),
        String::from("\\begin{cells}\\celldata{}\\celldata{&&&}\\celldata{\\\\\\\\}\\end{cells}"),
        String::from("\\begin{cells}\\cell[\\merge{1-1000000}]{A}\\end{cells}"),
        String::from("\\begin{cells}\\cell{A\\footnote{\\begin{cells}\\cell{B}\\end{cells}}}\\end{cells}"),
        nested,
    ] {
        let _ = render(&document);
    }
}
