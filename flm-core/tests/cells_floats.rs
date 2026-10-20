//! Tables of cells in `table` floats (test 12, the text part and the content
//! handler's cases; test 19, its float parts): the `cells` content handler
//! the feature contributes, the `table` type's default package naming the
//! environment, the float's caption and number around a table.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::cells::{self, CellsConfig};
use flm_core::stdcore::features::floats::{self, FloatType};
use flm_core::stdcore::StdcoreConfig;

use common::lowering::text as printed;
use common::stdcore::{identifiers, run_workflow};

fn registry(config: StdcoreConfig) -> Arc<FlmRegistry> {
    common::stdcore::registry(config)
}

fn render(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, text::WORKFLOW, &[document], &Overrides::none())
}

fn text_of(run: &Run) -> String {
    printed(run).trim_end().to_string()
}

/// The message of the first diagnostic `id` of the run.
fn message(run: &Run, id: &str) -> String {
    run.diagnostics().iter().find(|d| d.identifier() == id).expect("reported").to_string()
}

const TABLE: &str = "\\begin{table}\n\\begin{cells}\\celldata<H>{A & B}\\celldata{1 & 2}\\end{cells}\n\
                     \\caption{Numbers}\\label{table:x}\n\\end{table}\n\n\
                     See \\ref{table:x} and \\ref[S]{table:x}.";

#[test]
fn under_the_default_a_table_of_cells_renders_in_a_table_float() {
    let run = render(&registry(StdcoreConfig::new()), TABLE);
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(
        text_of(&run),
        "A  B\n-  -\n1  2\n\nTable\u{a0}1: Numbers\n\nSee Tab.\u{a0}1 and Table\u{a0}1."
    );
}

#[test]
fn a_table_nested_in_a_cell_of_a_table_float_renders() {
    let run = render(
        &registry(StdcoreConfig::new()),
        "\\begin{table}\\begin{cells}\\cell{\\begin{cells}\\cell{a}\\cell{b}\\end{cells}}\\cell{c}\
         \\end{cells}\\end{table}",
    );
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(text_of(&run).lines().next(), Some("a  b  c"));
}

#[test]
fn the_default_table_type_names_the_environments_package_and_the_handler() {
    let table = FloatType::table();
    assert!(table.scoped_packages.iter().any(|package| package.as_str() == cells::ENVIRONMENT_PACKAGE));
    assert_eq!(floats::CELLS_PACKAGE, cells::ENVIRONMENT_PACKAGE);
    assert_eq!(table.handlers.first().map(String::as_str), Some(cells::handler::HANDLER));
}

#[test]
fn the_handler_admits_exactly_one_table_of_cells() {
    let registry = registry(StdcoreConfig::new());
    for document in [
        "\\begin{table}\\begin{cells}\\cell{A}\\end{cells}\\begin{cells}\\cell{B}\\end{cells}\\end{table}",
        "\\begin{table}Some text.\n\n\\begin{cells}\\cell{A}\\end{cells}\\end{table}",
    ] {
        let run = render(&registry, document);
        assert_eq!(identifiers(&run), ["flm.floats.content-refused"], "{document}");
        let refused = message(&run, "flm.floats.content-refused");
        assert!(refused.contains("exactly one table of cells"), "{refused}");
        assert!(text_of(&run).contains('A'), "shown as it stands: {}", text_of(&run));
    }
}

#[test]
fn a_figure_holding_a_table_of_cells_is_the_floats_features_error() {
    let registry = registry(StdcoreConfig::new().cells(CellsConfig::new().global(true)));
    let run = render(&registry, "\\begin{figure}\\begin{cells}\\cell{A}\\end{cells}\\end{figure}");
    assert_eq!(identifiers(&run), ["flm.floats.content-refused"]);
    assert!(message(&run, "flm.floats.content-refused").contains("exactly one image"));
}

#[test]
fn under_global_a_table_float_holds_a_table_alike() {
    let run = render(&registry(StdcoreConfig::new().cells(CellsConfig::new().global(true))), TABLE);
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert!(text_of(&run).starts_with("A  B\n-  -\n1  2"), "{}", text_of(&run));
}
