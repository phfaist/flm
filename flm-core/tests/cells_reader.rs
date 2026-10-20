//! The reader of a table of cells (`flm_core::stdcore::features::cells::read_grid`):
//! the Python implementation's tests and their expected grids (test 3), the
//! index, range, placement and mapping forms and the names (test 3), the style
//! tokens (test 4), a span to the end of the table as finally built (test 5),
//! the reader's Errors and recoveries (test 6, the reader's part), the grid's
//! occupancy on every test-3 table (test 7, `grid()`), and inputs that must
//! neither panic nor loop (test 15, the reader's part).

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::lang::Flm;
use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::stdcore::backends::pres::{CellsPayload, GridPosition};
use flm_core::stdcore::features::cells::{
    self, CellContent, CellStyle, CellsConfig, Grid, Styles, read_grid, shipped,
};
use flm_core::stdcore::StdcoreConfig;
use techy::core::node::NodeRef;

use common::stdcore::run_workflow;

fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().cells(CellsConfig::new().global(true)))
}

/// The shipped styles, and three custom styles of a host's.
fn styles() -> Styles {
    let mut all = shipped();
    for name in ["orange", "bold", "warn", "clr"] {
        all.push(CellStyle::custom(name, "a host's style"));
    }
    Styles::of(all)
}

/// Parses `document` (the `check` workflow, which lays it out as blocks) and
/// reads its first table.
fn read(document: &str) -> (Run, Summary) {
    let registry = registry();
    let run = run_workflow(&registry, "check", &[document], &Overrides::none());
    let summary = {
        let tree = run.frozen_tree(TreeId::tree(0)).expect("the first tree is frozen");
        let table = tree
            .root()
            .descendants()
            .find(|node| element_of(node).name.as_str() == cells::ENVIRONMENT)
            .expect("a table");
        let grid = read_grid(&table, &styles()).expect("no placeholder");
        Summary::of(&grid)
    };
    (run, summary)
}

/// What a test compares of a grid: the payload, each cell in a short form, and
/// the problems' identifiers and the details of their marks.
struct Summary {
    payload: CellsPayload,
    cells: Vec<String>,
    problems: Vec<String>,
    marks: Vec<String>,
}

impl Summary {
    fn of(grid: &Grid<'_>) -> Summary {
        let cells = grid
            .payload
            .cells
            .iter()
            .zip(&grid.contents)
            .map(|(spec, content)| {
                let mut out = format!("{},{}", spec.row + 1, spec.col + 1);
                if spec.row_span != 1 || spec.col_span != 1 {
                    out.push_str(&format!(" {}x{}", spec.row_span, spec.col_span));
                }
                if let Some(align) = spec.align {
                    out.push_str(&format!(" {}", align.as_str()));
                }
                if let Some(header) = spec.header {
                    out.push_str(&format!(" {}-header", header.as_str()));
                }
                for edge in &spec.rules {
                    out.push_str(&format!(" rule-{}", edge.as_str()));
                }
                for style in &spec.styles {
                    out.push_str(&format!(" +{style}"));
                }
                let (kind, slice) = match content {
                    CellContent::Blocks(slice) => ("", slice),
                    CellContent::Inline(slice) => ("~", slice),
                };
                out.push_str(&format!(": {kind}{}", text_of(slice.iter())));
                out
            })
            .collect();
        let problems =
            grid.problems.iter().map(|problem| problem.diagnostic.identifier().to_string()).collect();
        let marks = grid
            .problems
            .iter()
            .filter_map(|problem| problem.dropped.as_ref())
            .map(|mark| format!("{}: {}", mark.condition, mark.detail))
            .collect();
        Summary { payload: grid.payload.clone(), cells, problems, marks }
    }
}

/// The characters under a run of nodes, whitespace runs collapsed and trimmed.
fn text_of<'t>(nodes: impl Iterator<Item = NodeRef<'t, Flm>>) -> String {
    let mut out = String::new();
    for node in nodes {
        if let Some(text) = node.chars() {
            out.push_str(text);
        }
        for child in node.descendants() {
            if let Some(text) = child.chars() {
                out.push_str(text);
            }
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Reads `document`'s table and checks its size, its cells and that nothing
/// was reported.
fn assert_grid(document: &str, rows: u32, cols: u32, expected: &[&str]) -> CellsPayload {
    let (run, summary) = read(document);
    assert!(summary.problems.is_empty(), "{document}: {:?}", summary.problems);
    assert!(common::stdcore::identifiers(&run).is_empty(), "{document}: {}", run.diagnostics().render_all());
    assert_eq!((summary.payload.rows, summary.payload.cols), (rows, cols), "{document}");
    assert_eq!(summary.cells, expected, "{document}");
    summary.payload
}

// --- test 3: the Python implementation's tables -----------------------------------------

#[test]
fn a_two_by_two_of_cells() {
    let payload = assert_grid(
        "\\begin{cells}\n\\cell{A} \\cell{B}\n\\\\\n\\cell{C} \\cell{D}\n\\end{cells}",
        2,
        2,
        &["1,1: A", "1,2: B", "2,1: C", "2,2: D"],
    );
    use GridPosition::TopLeft;
    assert_eq!(payload.grid(), vec![vec![TopLeft(0), TopLeft(1)], vec![TopLeft(2), TopLeft(3)]]);
}

#[test]
fn a_two_by_three_of_celldata() {
    assert_grid(
        "\\begin{cells}\n\\celldata{\nA & B & C \\\\\nD & E & F\n}\n\\end{cells}",
        2,
        3,
        &["1,1: ~A", "1,2: ~B", "1,3: ~C", "2,1: ~D", "2,2: ~E", "2,3: ~F"],
    );
}

#[test]
fn a_header_row_and_data() {
    assert_grid(
        "\\begin{cells}\n\\celldata<H>{Name & City & Height}\n\\celldata{John & Berlin & 1m80}\n\\end{cells}",
        2,
        3,
        &[
            "1,1 column-header: ~Name",
            "1,2 column-header: ~City",
            "1,3 column-header: ~Height",
            "2,1: ~John",
            "2,2: ~Berlin",
            "2,3: ~1m80",
        ],
    );
}

#[test]
fn celldata_with_a_merged_cell() {
    let payload = assert_grid(
        "\\begin{cells}\n\\celldata{\nA & B & C \\\\\n\\cell[\\merge{1-2}]{DE} & F\n}\n\\end{cells}",
        2,
        3,
        &["1,1: ~A", "1,2: ~B", "1,3: ~C", "2,1 1x2: DE", "2,3: ~F"],
    );
    use GridPosition::{Covered, TopLeft};
    assert_eq!(
        payload.grid(),
        vec![vec![TopLeft(0), TopLeft(1), TopLeft(2)], vec![TopLeft(3), Covered(3), TopLeft(4)]]
    );
}

#[test]
fn styles_on_cells() {
    assert_grid(
        "\\begin{cells}\n\\cell<H>{Name} \\cell<H>{City}\n\\\\\n\\cell{John} \\cell<c>{Berlin}\n\\end{cells}",
        2,
        2,
        &["1,1 column-header: Name", "1,2 column-header: City", "2,1: John", "2,2 center: Berlin"],
    );
}

#[test]
fn celldata_with_an_open_mapping() {
    assert_grid(
        "\\begin{cells}\n\\celldata<H>{A & B & C}\n\\celldata[-]{\nD & E & F \\\\\nG & H & I\n}\n\\end{cells}",
        3,
        3,
        &[
            "1,1 column-header: ~A",
            "1,2 column-header: ~B",
            "1,3 column-header: ~C",
            "2,1: ~D",
            "2,2: ~E",
            "2,3: ~F",
            "3,1: ~G",
            "3,2: ~H",
            "3,3: ~I",
        ],
    );
}

#[test]
fn celldata_with_several_merges() {
    assert_grid(
        "\\begin{cells}\n\\celldata[-]{\nA & B & C \\\\\n\\cell[\\merge{1-2}]{DE} & F \\\\\n\
         \\cell[\\merge{+2}]{XY} & Z\n}\n\\end{cells}",
        3,
        3,
        &["1,1: ~A", "1,2: ~B", "1,3: ~C", "2,1 1x2: DE", "2,3: ~F", "3,1 1x2: XY", "3,3: ~Z"],
    );
}

#[test]
fn three_by_one() {
    assert_grid(
        "\\begin{cells}\n\\cell{One}\n\\\\\n\\cell{Two}\n\\\\\n\\cell{Three}\n\\end{cells}",
        3,
        1,
        &["1,1: One", "2,1: Two", "3,1: Three"],
    );
}

#[test]
fn complex_header_styles_and_merges() {
    let payload = assert_grid(
        "\\begin{cells}\n\\celldata<H>{One & Two & Three}\n\\celldata<c>[-]{\nA & B & C \\\\\n\
         \\cell<red>[\\merge{1-2}]{DE} & F\n}\n\\cell<green>{X}\n\\cell<yellow>[\\merge{+2}]{YZ}\n\\end{cells}",
        4,
        3,
        &[
            "1,1 column-header: ~One",
            "1,2 column-header: ~Two",
            "1,3 column-header: ~Three",
            "2,1 center: ~A",
            "2,2 center: ~B",
            "2,3 center: ~C",
            "3,1 1x2 center +red: DE",
            "3,3 center: ~F",
            "4,1 +green: X",
            "4,2 1x2 +yellow: YZ",
        ],
    );
    use GridPosition::{Covered, TopLeft};
    assert_eq!(
        payload.grid(),
        vec![
            vec![TopLeft(0), TopLeft(1), TopLeft(2)],
            vec![TopLeft(3), TopLeft(4), TopLeft(5)],
            vec![TopLeft(6), Covered(6), TopLeft(7)],
            vec![TopLeft(8), TopLeft(9), Covered(9)],
        ]
    );
}

#[test]
fn a_style_list_of_several_tokens() {
    assert_grid(
        "\\begin{cells}\n\\celldata<H c>{One & Two}\n\\celldata{A & B}\n\\end{cells}",
        2,
        2,
        &["1,1 center column-header: ~One", "1,2 center column-header: ~Two", "2,1: ~A", "2,2: ~B"],
    );
}

// --- test 3: indices, ranges, placements, mappings, names ---------------------------------

#[test]
fn an_index_is_a_number_a_dot_or_nothing() {
    assert_grid("\\begin{cells}\\cell[5]{A}\\end{cells}", 1, 5, &["1,5: A"]);
    assert_grid("\\begin{cells}\\cell{A}\\\\\\cell[.;.]{B}\\end{cells}", 2, 1, &["1,1: A", "2,1: B"]);
    assert_grid("\\begin{cells}\\cell{A}\\cell[;]{B}\\cell[]{C}\\end{cells}", 1, 3, &["1,1: A", "1,2: B", "1,3: C"]);
}

#[test]
fn a_trailing_row_break_in_data_ends_the_last_row_and_adds_none() {
    let four = ["1,1: ~A", "1,2: ~B", "2,1: ~C", "2,2: ~D"];
    for data in ["A & B \\\\ C & D \\\\", "A & B \\\\ C & D \\\\ %% a comment\n  "] {
        let document = format!("\\begin{{cells}}\\celldata{{{data}}}\\cell{{E}}\\end{{cells}}");
        let mut expected = four.to_vec();
        expected.push("3,1: E");
        assert_grid(&document, 3, 2, &expected);
        assert_grid(&format!("\\begin{{cells}}\\celldata{{{data}}}\\end{{cells}}"), 2, 2, &four);
    }
    // A row break on its own inside the data still makes an empty row.
    assert_grid("\\begin{cells}\\celldata{A & B \\\\ \\\\ C}\\end{cells}", 3, 2, &["1,1: ~A", "1,2: ~B", "3,1: ~C"]);
    // Data of nothing but row breaks: the last one adds no row.
    assert_grid("\\begin{cells}\\celldata{}\\cell{E}\\end{cells}", 2, 1, &["2,1: E"]);
    assert_grid("\\begin{cells}\\celldata{\\\\}\\cell{E}\\end{cells}", 2, 1, &["2,1: E"]);
    assert_grid("\\begin{cells}\\celldata{\\\\\\\\}\\cell{E}\\end{cells}", 3, 1, &["3,1: E"]);
}

#[test]
fn a_no_break_space_is_content_and_not_blank() {
    // A no-break space after a final row break is content: the last part is a
    // row of its own.
    assert_grid("\\begin{cells}\\celldata{A & B \\\\ \u{a0}}\\cell{E}\\end{cells}", 3, 2, &[
        "1,1: ~A",
        "1,2: ~B",
        "2,1: ~",
        "3,1: E",
    ]);
    // Between two commands of a table's body, it is content outside a cell.
    let (_, summary) = read("\\begin{cells}\\cell{A}\u{a0}\\cell{B}\\end{cells}");
    assert_eq!(summary.problems, ["flm.cells.content-outside-cell"]);
    assert_eq!(summary.marks, ["flm.cells.content-outside-cell: content outside \\cell"]);
    assert_eq!(summary.cells, ["1,1: A", "1,2: B"]);
}

#[test]
fn a_range_in_a_merge() {
    assert_grid("\\begin{cells}\\cell[\\merge{2-4}]{A}\\end{cells}", 1, 4, &["1,2 1x3: A"]);
    assert_grid("\\begin{cells}\\cell[\\merge{2+3}]{A}\\end{cells}", 1, 4, &["1,2 1x3: A"]);
    assert_grid("\\begin{cells}\\cell[\\merge{1,2}]{A}\\end{cells}", 1, 2, &["1,1 1x2: A"]);
    // A start left out in a placement's `\merge` is the first index.
    assert_grid("\\begin{cells}\\cell{A}\\cell{B}\\\\\\cell[\\merge{-}]{C}\\end{cells}", 2, 2, &[
        "1,1: A",
        "1,2: B",
        "2,1 1x2: C",
    ]);
    for range in ["1,3", ".+abc"] {
        let (_, summary) = read(&format!("\\begin{{cells}}\\cell[\\merge{{{range}}}]{{A}}\\end{{cells}}"));
        assert_eq!(summary.problems, ["flm.cells.bad-placement"], "{range}");
        assert_eq!(summary.cells, ["1,1: A"], "{range}");
    }
}

#[test]
fn a_mapping_may_permute_the_columns() {
    assert_grid("\\begin{cells}\\celldata[3,2,1]{A & B & C}\\end{cells}", 1, 3, &["1,1: ~C", "1,2: ~B", "1,3: ~A"]);
}

#[test]
fn a_mapping_of_merges_over_rows_and_columns() {
    assert_grid(
        "\\begin{cells}\\celldata[\\merge{1,2},\\merge{+2};4,6]{A & AA \\\\ B & BB}\\end{cells}",
        4,
        6,
        &["1,4 2x1: ~A", "1,6 2x1: ~AA", "3,4 2x1: ~B", "3,6 2x1: ~BB"],
    );
}

#[test]
fn an_open_mapping_range_continues_one_column_per_entry() {
    assert_grid(
        "\\begin{cells}\\celldata[2-5]{A & B & C & D \\\\ E & F & G & H}\\end{cells}",
        2,
        5,
        &["1,2: ~A", "1,3: ~B", "1,4: ~C", "1,5: ~D", "2,2: ~E", "2,3: ~F", "2,4: ~G", "2,5: ~H"],
    );
    assert_grid(
        "\\begin{cells}\\celldata[2-]{A & B & C}\\end{cells}",
        1,
        4,
        &["1,2: ~A", "1,3: ~B", "1,4: ~C"],
    );
}

#[test]
fn names_stand_wherever_an_index_does() {
    let names = "\\colnames{name, city, height}\\rownames{h, john}";
    assert_grid(
        &format!("\\begin{{cells}}{names}\\cell[john;city]{{X}}\\end{{cells}}"),
        2,
        2,
        &["2,2: X"],
    );
    assert_grid(
        &format!("\\begin{{cells}}{names}\\celldata[;city-height]{{A & B}}\\end{{cells}}"),
        1,
        3,
        &["1,2: ~A", "1,3: ~B"],
    );
    assert_grid(
        &format!("\\begin{{cells}}{names}\\cell[\\merge{{name-city}}]{{A}}\\end{{cells}}"),
        1,
        2,
        &["1,1 1x2: A"],
    );
    assert_grid(
        &format!("\\begin{{cells}}{names}\\cell[\\merge{{city+2}}]{{A}}\\end{{cells}}"),
        1,
        3,
        &["1,2 1x2: A"],
    );
    // A `\colnames` written last applies to the whole table; an empty entry
    // names nothing.
    assert_grid("\\begin{cells}\\cell[b]{X}\\colnames{a,,b}\\end{cells}", 1, 3, &["1,3: X"]);
}

// --- test 4: the style tokens -------------------------------------------------------------

#[test]
fn a_cells_own_tokens_come_before_its_celldatas() {
    assert_grid(
        "\\begin{cells}\\celldata<c>{A & \\cell<r>{X}}\\end{cells}",
        1,
        2,
        &["1,1 center: ~A", "1,2 right: X"],
    );
    assert_grid("\\begin{cells}\\cell<H rH>{X}\\end{cells}", 1, 1, &["1,1 column-header: X"]);
    assert_grid("\\begin{cells}\\cell<H  l>{X}\\end{cells}", 1, 1, &["1,1 left column-header: X"]);
}

#[test]
fn a_structural_style_and_its_synonym_say_the_same() {
    assert_grid("\\begin{cells}\\cell<align{right}>{X}\\cell<r>{Y}\\end{cells}", 1, 2, &[
        "1,1 right: X",
        "1,2 right: Y",
    ]);
    assert_grid("\\begin{cells}\\cell<head{row}>{X}\\cell<rH>{Y}\\end{cells}", 1, 2, &[
        "1,1 row-header: X",
        "1,2 row-header: Y",
    ]);
    assert_grid("\\begin{cells}\\cell<rule{above} rule{left} lvert>{X}\\end{cells}", 1, 1, &[
        "1,1 rule-above rule-left: X",
    ]);
}

#[test]
fn custom_styles_are_carried_in_order() {
    assert_grid("\\begin{cells}\\cell<green red>{X}\\cell<orange bold warn>{Y}\\end{cells}", 1, 2, &[
        "1,1 +green +red: X",
        "1,2 +orange +bold +warn: Y",
    ]);
    // A comma inside a value's braces belongs to the value.
    let (_, summary) = read("\\begin{cells}\\celldata<H l, green clr{rgb(1,0.4,0.4)}>{A & B}\\end{cells}");
    assert_eq!(summary.problems, ["flm.cells.bad-style-value"]);
    assert_eq!(summary.cells, ["1,1 left column-header: ~A", "1,2 +green: ~B"]);
}

#[test]
fn a_bad_token_is_reported_and_ignored() {
    for (styles, problem) in [
        ("green{x} r", "flm.cells.bad-style-value"),
        ("align r", "flm.cells.bad-style-value"),
        ("align{middle} r", "flm.cells.bad-style-value"),
        ("foo r", "flm.cells.unknown-style"),
    ] {
        let (_, summary) = read(&format!("\\begin{{cells}}\\cell<{styles}>{{X}}\\end{{cells}}"));
        assert_eq!(summary.problems, [problem], "{styles}");
        assert_eq!(summary.cells, ["1,1 right: X"], "{styles}");
    }
    let (_, summary) = read("\\begin{cells}\\cell<\\emph{x} r>{X}\\end{cells}");
    assert_eq!(summary.problems, ["flm.cells.bad-styles"]);
    assert_eq!(summary.cells, ["1,1: X"]);
}

#[test]
fn a_celldatas_bad_token_is_reported_once() {
    let (_, summary) = read("\\begin{cells}\\celldata<foo>{A & B \\\\ C & D}\\end{cells}");
    assert_eq!(summary.problems, ["flm.cells.unknown-style"]);
}

// --- test 5: a span to the end of the table as finally built -------------------------------

#[test]
fn an_open_merge_spans_to_the_last_column_of_the_whole_table() {
    assert_grid(
        "\\begin{cells}\\cell[\\merge{2-}]{X}\\\\\\celldata{A & B & C & D}\\end{cells}",
        2,
        4,
        &["1,2 1x3: X", "2,1: ~A", "2,2: ~B", "2,3: ~C", "2,4: ~D"],
    );
}

// --- test 6: the Errors and their recoveries (the reader's part) ---------------------------

#[test]
fn stray_content_is_dropped_with_one_mark() {
    let (_, summary) = read("\\begin{cells}\nHello world\n\\cell{A}\n\\end{cells}");
    assert_eq!(summary.problems, ["flm.cells.content-outside-cell"]);
    assert_eq!(summary.marks, ["flm.cells.content-outside-cell: content outside \\cell"]);
    assert_eq!(summary.cells, ["1,1: A"]);
}

#[test]
fn a_placement_that_cannot_be_read_is_ignored() {
    for placement in ["a;b;c", "foo", "\\merge{\\merge{1}}", "0", ";nope"] {
        let (_, summary) = read(&format!("\\begin{{cells}}\\cell{{A}}\\cell[{placement}]{{B}}\\end{{cells}}"));
        assert_eq!(summary.problems, ["flm.cells.bad-placement"], "{placement}");
        assert_eq!(summary.cells, ["1,1: A", "1,2: B"], "{placement}");
    }
}

#[test]
fn a_mapping_that_cannot_be_read_is_ignored_whole() {
    let (_, summary) = read("\\begin{cells}\\celldata[\\merge{1,3}]{A & B}\\end{cells}");
    assert_eq!(summary.problems, ["flm.cells.bad-placement"]);
    assert_eq!(summary.cells, ["1,1: ~A", "1,2: ~B"]);
}

#[test]
fn the_later_of_two_overlapping_cells_is_dropped_with_a_mark() {
    let (_, summary) = read("\\begin{cells}\\cell{A}\\cell[1]{B}\\end{cells}");
    assert_eq!(summary.problems, ["flm.cells.overlap"]);
    assert_eq!(summary.marks, ["flm.cells.overlap: cell overlapping another"]);
    assert_eq!(summary.cells, ["1,1: A"]);
    // Data past the end of a mapping lands on its last position.
    let (_, summary) = read("\\begin{cells}\\celldata[1,2]{A & B & C}\\end{cells}");
    assert_eq!(summary.problems, ["flm.cells.overlap"]);
    assert_eq!(summary.cells, ["1,1: ~A", "1,2: ~B"]);
}

#[test]
fn bad_names_are_reported_and_ignored() {
    for (names, placement) in [("a,a", "a"), ("1x,b", "b"), ("a}\\colnames{b", "a")] {
        let (_, summary) =
            read(&format!("\\begin{{cells}}\\colnames{{{names}}}\\cell[{placement}]{{X}}\\end{{cells}}"));
        assert_eq!(summary.problems, ["flm.cells.bad-names"], "{names}");
        assert_eq!(summary.cells.len(), 1, "{names}");
    }
}

#[test]
fn an_empty_table_is_reported_with_no_mark() {
    let (_, summary) = read("\\begin{cells}\\end{cells}");
    assert_eq!(summary.problems, ["flm.cells.empty"]);
    assert!(summary.marks.is_empty());
    assert!(summary.cells.is_empty());
}

// --- test 15: inputs that must neither panic nor loop -----------------------------------------

#[test]
fn hard_inputs_are_read() {
    for body in [
        "",
        "%% a comment\n\n\n%% another",
        "\\\\",
        "\\\\\\\\\\\\",
        "\\celldata{}",
        "\\celldata{&&&}",
        "\\celldata{\\\\\\\\}",
        "\\cell[;]{A}",
        "\\cell[\\merge{}]{A}",
        "\\cell[\\merge{-}]{A}",
        "\\cell[\\merge{1-1000000}]{A}",
        "\\cell[\\merge{1-1000000};\\merge{1-1000000}]{A}",
        "\\cell[1000001]{A}",
        "\\cell[99999999999999999999999]{A}",
    ] {
        let _ = read(&format!("\\begin{{cells}}{body}\\end{{cells}}"));
    }
    let mapping: Vec<String> = (1..=10_000).map(|index| index.to_string()).collect();
    let entries = vec!["x"; 10_000].join(" & ");
    let (_, summary) = read(&format!("\\begin{{cells}}\\celldata[{}]{{{entries}}}\\end{{cells}}", mapping.join(",")));
    assert_eq!(summary.payload.cols, 10_000);
}

#[test]
fn a_mapping_listing_more_than_the_bound_is_ignored_before_it_is_listed() {
    // Twenty ranges of a million columns each: listed one entry per index,
    // they would ask for twenty million entries.
    let ranges = vec!["1-1000000"; 20].join(",");
    let (_, summary) = read(&format!("\\begin{{cells}}\\celldata[{ranges}]{{A & B}}\\end{{cells}}"));
    assert_eq!(summary.problems, ["flm.cells.bad-placement"]);
    assert_eq!(summary.cells, ["1,1: ~A", "1,2: ~B"]);
}

#[test]
fn a_table_larger_than_the_bound_drops_the_cell_with_a_mark() {
    let (_, summary) = read("\\begin{cells}\\cell{A}\\cell[\\merge{1-1000};\\merge{1-1001}]{B}\\end{cells}");
    assert_eq!(summary.problems, ["flm.cells.too-large"]);
    assert_eq!(summary.marks.len(), 1);
    assert_eq!(summary.cells, ["1,1: A"]);
}
