//! The `cells` feature on the HTML backend: every table of the reader's tests
//! written as a `<table>` under the backend's contract (test 10), the Python
//! product's one HTML test of a table rendered under that contract (test 18),
//! the marks a rule leaves where it dropped something (test 6's HTML part),
//! and a host's own custom styles (test 4's HTML part). The goldens are
//! parsed by `tests/html_validity.rs` with no excuse.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{
    BuildError, DefContent, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry, StyleDef,
};
use flm_core::core::run::Run;
use flm_core::model::names::Name;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::html::{BACKEND as HTML_BACKEND, WORKFLOW as HTML_WORKFLOW};
use flm_core::stdcore::features::cells::{self, CellStyle, CellsConfig, STYLES_SLOT};

use common::stdcore::{identifiers, run_workflow};

// ---------------------------------------------------------------------------------------------
// registries and runs
// ---------------------------------------------------------------------------------------------

/// The standard package with the `cells` environment wherever blocks stand.
fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().cells(CellsConfig::new().global(true)))
}

/// The run of the `html` workflow over `document`.
fn html_run(document: &str) -> Run {
    run_workflow(&registry(), HTML_WORKFLOW, &[document], &Overrides::none())
}

/// The HTML fragment of `document`; the run must report nothing.
fn html_output(document: &str) -> String {
    let run = html_run(document);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    common::html::fragment(&run)
}

/// The opening tag of a table.
const TABLE: &str = "<table class=\"flm-pres-cells\">";

/// Every table of the reader's tests of the Python product's cases (test 3),
/// in the order `tests/cells_reader.rs` gives them.
const TEST_3_TABLES: [&str; 9] = [
    "\\begin{cells}\n\\cell{A} \\cell{B}\n\\\\\n\\cell{C} \\cell{D}\n\\end{cells}",
    "\\begin{cells}\n\\celldata{\nA & B & C \\\\\nD & E & F\n}\n\\end{cells}",
    "\\begin{cells}\n\\celldata<H>{Name & City & Height}\n\\celldata{John & Berlin & 1m80}\n\\end{cells}",
    "\\begin{cells}\n\\celldata{\nA & B & C \\\\\n\\cell[\\merge{1-2}]{DE} & F\n}\n\\end{cells}",
    "\\begin{cells}\n\\cell<H>{Name} \\cell<H>{City}\n\\\\\n\\cell{John} \\cell<c>{Berlin}\n\\end{cells}",
    "\\begin{cells}\n\\celldata<H>{A & B & C}\n\\celldata[-]{\nD & E & F \\\\\nG & H & I\n}\n\\end{cells}",
    "\\begin{cells}\n\\celldata[-]{\nA & B & C \\\\\n\\cell[\\merge{1-2}]{DE} & F \\\\\n\
     \\cell[\\merge{+2}]{XY} & Z\n}\n\\end{cells}",
    "\\begin{cells}\n\\cell{One}\n\\\\\n\\cell{Two}\n\\\\\n\\cell{Three}\n\\end{cells}",
    "\\begin{cells}\n\\celldata<H>{One & Two & Three}\n\\celldata<c>[-]{\nA & B & C \\\\\n\
     \\cell<red>[\\merge{1-2}]{DE} & F\n}\n\\cell<green>{X}\n\\cell<yellow>[\\merge{+2}]{YZ}\n\\end{cells}",
];

// ---------------------------------------------------------------------------------------------
// test 10: the tables of test 3
// ---------------------------------------------------------------------------------------------

#[test]
fn every_table_of_the_reader_s_tests_is_a_table_of_rows_and_cells() {
    let document = TEST_3_TABLES.join("\n\n");
    let html = html_output(&document);
    assert_eq!(html.matches(TABLE).count(), TEST_3_TABLES.len(), "{html}");
    assert_eq!(html.matches("</table>").count(), TEST_3_TABLES.len(), "{html}");
    // Rows: 2, 2, 2, 2, 2, 3, 3, 3, 4.
    assert_eq!(html.matches("<tr>").count(), 23, "{html}");
    // A cell's content is written as blocks inside it: a `\cell`'s paragraph,
    // a `\celldata` entry's one paragraph.
    assert!(html.contains("<td>\n      <p class=\"flm-pres-par\">A</p>\n    </td>"), "{html}");
    // Column headers carry their scope.
    assert!(
        html.contains("<th scope=\"col\">\n      <p class=\"flm-pres-par\">Name</p>\n    </th>"),
        "{html}"
    );
    // A spanned cell carries its span, and the position it covers is written
    // nothing: the row of `DE` and `F` holds two cells.
    assert!(
        html.contains(
            "<tr>\n    <td colspan=\"2\" data-flm-align=\"center\" data-flm-style-red=\"\">\n      \
             <p class=\"flm-pres-par\">DE</p>\n    </td>\n    <td data-flm-align=\"center\">"
        ),
        "{html}"
    );
    assert!(html.contains("<td data-flm-style-green=\"\">"), "{html}");
    assert!(html.contains("<td colspan=\"2\" data-flm-style-yellow=\"\">"), "{html}");
    // No class of the feature's own, no edge class, no row group.
    for absent in ["celltbledge", "cellstyle", "class=\"cell", "<thead", "<tbody", "data-flm-kind"] {
        assert!(!html.contains(absent), "{absent}: {html}");
    }
    common::html::assert_golden("cells-test-3.html", &html);
}

#[test]
fn a_cell_spanning_rows_carries_rowspan_and_every_fact_its_own_attribute() {
    let html = html_output(
        "\\begin{cells}\\celldata[\\merge{1,2},\\merge{+2};4,6]{A & AA \\\\ B & BB}\\end{cells}\n\n\
         \\begin{cells}\\cell<rH>{R}\\cell<rule{above} lvert r>{X}\\cell<align{left} green blue>{Y}\
         \\end{cells}",
    );
    // Rows 1–2 and 3–4, columns 4 and 6: four cells each spanning two rows,
    // the positions under them written nothing, every other position empty.
    assert_eq!(html.matches("<td rowspan=\"2\">").count(), 4, "{html}");
    let first = html.split("</table>").next().expect("the first table");
    assert_eq!(first.matches("<td></td>").count(), 4 + 4 + 4 + 4, "{html}");
    assert!(html.contains("<th scope=\"row\">\n      <p class=\"flm-pres-par\">R</p>"), "{html}");
    assert!(html.contains("<td data-flm-align=\"right\" data-flm-rules=\"above left\">"), "{html}");
    assert!(
        html.contains("<td data-flm-align=\"left\" data-flm-style-green=\"\" data-flm-style-blue=\"\">"),
        "{html}"
    );
    common::html::assert_golden("cells-attributes.html", &html);
}

#[test]
fn an_empty_position_and_an_empty_entry_are_empty_cells() {
    // Row 1: `A`, an empty entry, `C`; row 2: one cell, then two positions
    // no cell covers.
    let html = html_output("\\begin{cells}\\celldata{A & & C \\\\ D}\\end{cells}");
    let rows: Vec<&str> = html.split("<tr>").skip(1).collect();
    assert_eq!(rows.len(), 2, "{html}");
    assert_eq!(rows[0].matches("<td></td>").count(), 1, "{html}");
    assert_eq!(rows[1].matches("<td></td>").count(), 2, "{html}");
    assert_eq!(html.matches("<p ").count(), 3, "{html}");
}

#[test]
fn the_stylesheet_is_in_a_page_holding_a_table_once_and_in_no_other() {
    let run = html_run(
        "\\begin{cells}\\celldata<H>{A & B}\\cell<green>{C}\\cell<green>{D}\\end{cells}\n\n\
         \\begin{cells}\\cell{E}\\end{cells}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let page = common::html::page(&run);
    let marker = format!("data-flm-def=\"{}\"", cells::STYLE);
    assert_eq!(page.matches(&marker).count(), 1, "{page}");
    common::html::assert_golden("cells.page.html", &page);

    let run = html_run("A paragraph.\n");
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(common::html::page(&run).matches(&marker).count(), 0);
}

// ---------------------------------------------------------------------------------------------
// test 4, the HTML part: a host's own custom styles
// ---------------------------------------------------------------------------------------------

/// The identifier of the host's stylesheet chunk.
const HOST_STYLE: &str = "host.cells.style";

/// A host's feature: three custom styles, `orange`, `bold` and `warn`, each
/// requiring the host's stylesheet chunk on HTML, and that chunk.
struct HostStyles;

impl FlmFeature for HostStyles {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("host", "A host's cell styles and their stylesheet")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.style_def(StyleDef {
            id: Name::from(HOST_STYLE),
            content: vec![DefContent::Inline(String::from(
                "/* host.cells.style */\n.flm-pres-cells [data-flm-style-orange] { background: orange; }\n",
            ))],
            docs: String::from("the host's cell styles"),
        })?;
        for name in ["orange", "bold", "warn"] {
            let style = CellStyle::custom(name, "a host's style").requires(HTML_BACKEND, [HOST_STYLE]);
            b.peer_slot::<CellStyle>(STYLES_SLOT, Arc::new(style))?;
        }
        Ok(())
    }
}

/// The run of the `html` workflow over `document` under the standard package
/// and the host's feature.
fn host_run(document: &str) -> Run {
    let mut features =
        common::stdcore::package_features_but(StdcoreConfig::new().cells(CellsConfig::new().global(true)), "");
    features.push(Arc::new(HostStyles));
    let registry = common::stdcore::registry_over(features);
    run_workflow(&registry, HTML_WORKFLOW, &[document], &Overrides::none())
}

#[test]
fn a_host_s_custom_styles_are_written_as_attributes_and_require_its_chunk() {
    let run = host_run("\\begin{cells}\\cell<orange bold>{A}\\cell<warn>{B}\\cell<orange>{C}\\end{cells}");
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let page = common::html::page(&run);
    // Each style an attribute of its own, in the order of the tokens, the
    // feature writing no code of its own for any of them.
    assert!(page.contains("<td data-flm-style-orange=\"\" data-flm-style-bold=\"\">"), "{page}");
    assert!(page.contains("<td data-flm-style-warn=\"\">"), "{page}");
    // The host's chunk is in the page once, however many cells require it.
    let marker = format!("data-flm-def=\"{HOST_STYLE}\"");
    assert_eq!(page.matches(&marker).count(), 1, "{page}");
    common::html::assert_golden("cells-host-styles.page.html", &page);

    // A table using none of the host's styles requires nothing of the host.
    let run = host_run("\\begin{cells}\\cell<green>{A}\\end{cells}");
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(common::html::page(&run).matches(&marker).count(), 0);
}

// ---------------------------------------------------------------------------------------------
// test 18: the Python product's one HTML test of a table
// ---------------------------------------------------------------------------------------------

/// The Python product writes, for this table, `<table class="cells">` with
/// `<th class="cell cellstyle-H celltbledge-top celltbledge-left"><p>A</p></th>`
/// and the like: a class per style token and per edge of the table the cell
/// touches. Under this backend's contract the table carries its element's
/// class, a header cell its `scope` instead of a class for its token, and no
/// cell an edge class — the table's outline is drawn on the `<table>` by the
/// stylesheet — and a paragraph carries its element's class. An entry of
/// `\celldata` is written without the spaces around it (`A`, not `A `), as
/// the Python product writes it.
#[test]
fn the_python_product_s_simple_table_under_the_contract() {
    let html = html_output("\\begin{cells}\\celldata<H>{A & B}\\celldata{1 & 2}\\end{cells}");
    let expected = "\
<table class=\"flm-pres-cells\">
  <tr>
    <th scope=\"col\">
      <p class=\"flm-pres-par\">A</p>
    </th>
    <th scope=\"col\">
      <p class=\"flm-pres-par\">B</p>
    </th>
  </tr>
  <tr>
    <td>
      <p class=\"flm-pres-par\">1</p>
    </td>
    <td>
      <p class=\"flm-pres-par\">2</p>
    </td>
  </tr>
</table>";
    assert_eq!(html.trim_end(), expected);
}

// ---------------------------------------------------------------------------------------------
// test 6, the HTML part: the marks
// ---------------------------------------------------------------------------------------------

/// The mark for what was left out under the condition `condition`.
fn mark(condition: &str, detail: &str) -> String {
    format!(
        "<span class=\"flm-pres-dropped\" data-flm-condition=\"flm.cells.{condition}\">dropped: {detail}</span>"
    )
}

#[test]
fn the_marks_of_a_table_stand_before_it_one_per_thing_left_out() {
    let run = html_run("\\begin{cells}\nHello world\n\\cell{A}\\cell[1]{B}\n\\end{cells}");
    assert_eq!(
        identifiers(&run),
        ["flm.cells.content-outside-cell", "flm.cells.overlap"],
        "{}",
        run.diagnostics().render_all()
    );
    let html = common::html::fragment(&run);
    let stray = mark("content-outside-cell", "content outside \\cell");
    let overlap = mark("overlap", "cell overlapping another");
    let (stray_at, overlap_at, table_at) = (
        html.find(&stray).expect("the mark of the stray content"),
        html.find(&overlap).expect("the mark of the overlapping cell"),
        html.find(TABLE).expect("the table"),
    );
    assert!(stray_at < overlap_at && overlap_at < table_at, "{html}");
    assert!(!html.contains("Hello") && !html.contains(">B<"), "{html}");
    common::html::assert_golden("cells-marks.html", &html);
    let page = common::html::page(&run);
    // The marks take the error look, and two side by side stand apart.
    assert_eq!(page.matches("data-flm-def=\"flm.html.style.errors\"").count(), 1, "{page}");
    assert!(page.contains(".flm-pres-unsupported,\n  .flm-pres-dropped {"), "{page}");
    assert!(page.contains(".flm-pres-dropped + .flm-pres-dropped {"), "{page}");
}

#[test]
fn a_mark_stands_where_a_command_out_of_place_stood() {
    let run = html_run("\\begin{cells}\\celldata{\\emph{a \\\\ b} & c}\\end{cells}");
    assert_eq!(identifiers(&run), ["flm.cells.outside-table"], "{}", run.diagnostics().render_all());
    let html = common::html::fragment(&run);
    let inside = format!("<em class=\"flm-pres-span\" data-flm-style=\"emph\">a {} b</em>", mark("outside-table", "\\\\ out of place"));
    assert!(html.contains(&inside), "{html}");
}
