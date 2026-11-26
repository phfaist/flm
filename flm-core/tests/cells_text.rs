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
use flm_core::stdlib::backends::lowering::LoweredAddr;
use flm_core::model::element::element_of;
use flm_core::model::options::OptionValues;
use flm_core::stdlib::backends::pres::{
    self, CellAlign, CellHeader, CellsPayload, DroppedPayload, cells_of, payload_of,
};
use flm_core::stdlib::backends::text::{
    self, TEXT_OUTPUT, TableFrame, TextOutput, TextRenderOptions, TextStyle,
};
use flm_core::stdlib::features::cells::CellsConfig;
use flm_core::stdlib::features::userdefs::UserdefsConfig;
use flm_core::stdlib::StdlibConfig;

use common::lowering::{lowered, text as printed};
use common::stdlib::{frames, identifiers, run_workflow};

fn global() -> StdlibConfig {
    StdlibConfig::new().cells(CellsConfig::new().global(true))
}

fn registry() -> Arc<FlmRegistry> {
    common::stdlib::registry(global())
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

// --- the frame of a table and the marks on what the table draws ----------------------------

fn framed(frame: TableFrame, columns: Option<u32>) -> Overrides {
    Overrides {
        render_options: OptionValues::new().with(TextRenderOptions {
            width: columns,
            table_frame: frame,
            ..TextRenderOptions::default()
        }),
        ..Overrides::none()
    }
}

fn output(run: &Run) -> &TextOutput {
    run.table(TEXT_OUTPUT).expect("declared").unwrap_or_else(|| {
        panic!("no text output; diagnostics:\n{}", run.diagnostics().render_all())
    })
}

/// The text each `TableBorder` style mark covers.
fn borders(output: &TextOutput) -> Vec<&str> {
    output
        .styles
        .iter()
        .filter(|mark| mark.style == TextStyle::TableBorder)
        .map(|mark| output.style_slice(mark))
        .collect()
}

const HEADED: &str = "\\begin{cells}\n\\celldata<H>{Name & Count}\n\\celldata<l, r>{alpha & 1 \\\\ be & 22}\n\
     \\cell<c>{mid}\\end{cells}";

#[test]
fn table_frame_box_frames_the_table_with_its_header_rule_and_alignments() {
    let run = render_with(&registry(), HEADED, &framed(TableFrame::Box, Some(80)));
    assert_eq!(
        text_of(&run),
        "┌───────┬───────┐
│ Name  │ Count │
├───────┼───────┤
│ alpha │     1 │
│ be    │    22 │
│  mid  │       │
└───────┴───────┘"
    );
    let output = output(&run);
    let lines = ["┌───────┬───────┐", "├───────┼───────┤", "└───────┴───────┘"];
    let mut expected = vec![lines[0], "│", "│", "│", lines[1]];
    for _ in 0..3 {
        expected.extend(["│", "│", "│"]);
    }
    expected.push(lines[2]);
    assert_eq!(borders(output), expected);
    assert_eq!(TextStyle::TableBorder.name(), "table-border");
}

#[test]
fn table_frame_none_keeps_the_gutter_and_marks_its_rule() {
    let run = render_with(&registry(), HEADED, &framed(TableFrame::None, Some(80)));
    assert_eq!(text_of(&run), "Name   Count\n-----  -----\nalpha      1\nbe        22\n mid");
    assert_eq!(borders(output(&run)), ["-----  -----"]);
}

#[test]
fn a_boxed_table_wraps_within_the_width_less_its_frame() {
    let run = render_with(
        &registry(),
        "\\begin{cells}\\cell{first words here\n\nsecond}\\cell{right column text}\\end{cells}",
        &framed(TableFrame::Box, Some(20)),
    );
    let text = text_of(&run);
    assert!(text.lines().all(|line| line.chars().count() <= 20), "{text}");
    assert!(text.lines().all(|line| line.ends_with(['│', '┐', '┤', '┘'])), "{text}");
    assert!(text.contains("second"), "{text}");
}

#[test]
fn a_nested_boxed_table_is_framed_inside_its_cell() {
    let run = render_with(
        &registry(),
        "\\begin{cells}\\cell{\\begin{cells}\\cell{a}\\cell{b}\\end{cells}}\\cell{c}\\end{cells}",
        &framed(TableFrame::Box, Some(80)),
    );
    assert_eq!(
        text_of(&run),
        "┌───────────┬───┐
│ ┌───┬───┐ │ c │
│ │ a │ b │ │   │
│ └───┴───┘ │   │
└───────────┴───┘"
    );
}

#[test]
fn a_boxed_table_in_a_list_item_is_indented_with_the_item() {
    let run = render_with(
        &registry(),
        "\\begin{itemize}\\item Text\n\n\\begin{cells}\\cell{a}\\cell{b}\\end{cells}\\end{itemize}",
        &framed(TableFrame::Box, Some(80)),
    );
    let text = text_of(&run);
    let frame: Vec<&str> = text.lines().skip_while(|line| !line.contains('┌')).collect();
    let indent = frame[0].find('┌').expect("the top line");
    assert!(indent > 0, "{text}");
    assert_eq!(frame.len(), 3, "{text}");
    for (line, drawn) in frame.iter().zip(["┌───┬───┐", "│ a │ b │", "└───┴───┘"]) {
        assert_eq!(&line[..indent], " ".repeat(indent), "{text}");
        assert_eq!(&line[indent..], drawn, "{text}");
    }
}

// --- the marks inside the cells ----------------------------------------------------------------

/// A table with a bold span and a link in a right-aligned cell that wraps,
/// and a paragraph in a cell of its own.
const MARKED: &str = "\\begin{cells}\\celldata<H>{Key & Value}\\cell{Paragraph}\
     \\cell<r>{some \\textbf{bold words} and \\href{https://example.com}{the site}}\\end{cells}";

/// The style marks other than the table's border, each with the text it
/// covers.
fn content_styles(output: &TextOutput) -> Vec<(TextStyle, &str)> {
    output
        .styles
        .iter()
        .filter(|mark| mark.style != TextStyle::TableBorder)
        .map(|mark| (mark.style.clone(), output.style_slice(mark)))
        .collect()
}

/// The text each position mark of a node of `element` in the lowered first
/// tree covers, in storage order.
fn position_slices<'r>(run: &'r Run, element: &str) -> Vec<&'r str> {
    let tree = &lowered(run, TreeId::tree(0)).tree;
    let out = output(run);
    tree.iter_storage_order()
        .filter(|node| element_of(node).name.as_str() == element)
        .flat_map(|node| out.marks_at(LoweredAddr::new(TreeId::tree(0), node.id())))
        .map(|mark| out.slice(mark))
        .collect()
}

#[test]
fn marks_inside_cells_cover_their_text_in_both_frames() {
    for frame in [TableFrame::Box, TableFrame::None] {
        let run = render_with(&registry(), MARKED, &framed(frame, Some(40)));
        let text = text_of(&run);
        let output = output(&run);
        let styles = content_styles(output);
        let href = TextStyle::Link { href: String::from("https://example.com") };
        // A mark over several lines of a cell gives one mark per line, each a
        // piece of the cell's own text on that line, the bold pieces
        // consecutive and before the link's.
        let bold = styles.iter().take_while(|(style, _)| *style == TextStyle::Bold).count();
        assert!(bold >= 1, "{frame:?}: {text}\n{styles:?}");
        assert!(styles[bold..].iter().all(|(style, _)| *style == href), "{frame:?}: {styles:?}");
        let joined = |pieces: &[(TextStyle, &str)]| {
            pieces.iter().map(|(_, slice)| *slice).collect::<Vec<_>>().join(" ")
        };
        assert_eq!(joined(&styles[..bold]), "bold words", "{frame:?}: {styles:?}\n{text}");
        assert_eq!(joined(&styles[bold..]), "the site", "{frame:?}: {styles:?}\n{text}");
        assert!(styles.len() > 2, "{frame:?}: the width wraps a mark\n{text}");
        for mark in output.styles.iter().filter(|mark| mark.style != TextStyle::TableBorder) {
            assert_eq!(mark.start.line, mark.end.line, "{frame:?}: {mark:?}");
        }
        let paragraphs = position_slices(&run, pres::PAR);
        assert!(paragraphs.contains(&"Paragraph"), "{frame:?}: {paragraphs:?}\n{text}");
        // The marks are reported in the order the printer opens them: the
        // table's border before the content of its cells.
        let last_border =
            output.styles.iter().rposition(|mark| mark.style == TextStyle::TableBorder).expect("a border");
        let first_content =
            output.styles.iter().position(|mark| mark.style != TextStyle::TableBorder).expect("a style");
        assert!(last_border < first_content, "{frame:?}");
    }
}

#[test]
fn a_marks_lines_and_columns_inside_a_framed_cell_match_its_bytes() {
    let run = render_with(&registry(), MARKED, &framed(TableFrame::Box, Some(40)));
    let output = output(&run);
    let text = &output.text;
    for mark in &output.styles {
        let line_start = |byte: usize| text[..byte].rfind('\n').map_or(0, |i| i + 1);
        let line = |byte: usize| text[..byte].matches('\n').count() as u32;
        let col = |byte: usize| text[line_start(byte)..byte].chars().count() as u32;
        assert_eq!((mark.start.line, mark.start.col), (line(mark.bytes.start), col(mark.bytes.start)));
        assert_eq!((mark.end.line, mark.end.col), (line(mark.bytes.end), col(mark.bytes.end)));
    }
    // The right-aligned cell is padded before its text, and the bold span
    // starts after that padding.
    let bold = output.styles.iter().find(|mark| mark.style == TextStyle::Bold).expect("a bold span");
    let line = text.lines().nth(bold.start.line as usize).expect("the line");
    assert!(line.contains("│    some "), "{text}");
    assert_eq!(&line[line.find("bold").expect("the word")..][..4], &text[bold.bytes.start..][..4]);
}

#[test]
fn marks_inside_a_nested_table_cover_their_text() {
    let run = render_with(
        &registry(),
        "\\begin{cells}\\cell{\\begin{cells}\\cell{a \\textbf{b}}\\cell{c}\\end{cells}}\\cell{\\emph{d}}\\end{cells}",
        &framed(TableFrame::Box, Some(80)),
    );
    assert_eq!(
        text_of(&run),
        "┌─────────────┬───┐
│ ┌─────┬───┐ │ d │
│ │ a b │ c │ │   │
│ └─────┴───┘ │   │
└─────────────┴───┘"
    );
    let output = output(&run);
    let marks: Vec<(&str, (u32, u32))> = output
        .styles
        .iter()
        .filter(|mark| mark.style != TextStyle::TableBorder)
        .map(|mark| (output.style_slice(mark), (mark.start.line, mark.start.col)))
        .collect();
    assert_eq!(marks, [("b", (2, 6)), ("d", (1, 16))]);
    // The inner table's frame is reported too, inside the outer cell.
    assert!(borders(output).contains(&"┌─────┬───┐"));
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
    assert_eq!(shown, [vec![], vec![reference]]);
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
    let mut features = common::stdlib::package_features_but(global(), "refs");
    features.retain(|feature| feature.info().name.as_str() != "numbering");
    let registry = common::stdlib::registry_over(features);
    let run = render_with(&registry, "\\begin{cells}\\cell{A \\ref{x}}\\end{cells}", &Overrides::none());
    assert!(text_of(&run).starts_with('A'), "{}", text_of(&run));
}

#[test]
fn a_table_renders_without_floats_under_global() {
    let registry = common::stdlib::registry_over(common::stdlib::package_features_but(global(), "floats"));
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
    let registry = common::stdlib::registry(config);
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
