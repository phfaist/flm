//! The render stage `math-techxt.render`, on the text workflow, whose rules read
//! its table: the rows of its table `math-techxt.rendered`, and its
//! diagnostics placed at the formulas, beside those of the check
//! `math-techxt.check` (an empty equation line).

mod common;

use flm_core::core::data::table::TableHandle;
use flm_core::core::run::Run;
use flm_core::stdlib::backends::text;
use flm_ext::adapter::table::ExtTable;
use serde_json::{Value, json};

use common::{registry, run};

/// The rows of the table `math-techxt.rendered` after `run`, each without its
/// address.
fn rows(run: &Run) -> Vec<Value> {
    let table = run.table(TableHandle::<ExtTable>::new("math-techxt.rendered")).expect("the table is declared");
    let items = table.map(|table| table.items().to_vec()).unwrap_or_default();
    items
        .into_iter()
        .map(|mut row| {
            assert!(row["addr"]["tree"].is_string() && row["addr"]["node"].is_u64(), "a row has an address: {row}");
            row.as_object_mut().expect("a row is a map").remove("addr");
            row
        })
        .collect()
}

/// Each diagnostic of the extension's in `run`: its identifier, its severity,
/// and the source text it is placed at.
fn placed(run: &Run) -> Vec<(String, String, String)> {
    run.diagnostics()
        .iter()
        .filter(|d| d.identifier().starts_with("math-techxt."))
        .map(|d| (d.identifier().to_string(), format!("{:?}", d.severity()), d.span().content().to_string()))
        .collect()
}

/// The rows and the placed diagnostics of the text workflow's run over
/// `source`.
fn rendered(source: &str) -> (Vec<Value>, Vec<(String, String, String)>) {
    let run = run(&registry(), text::WORKFLOW, source);
    (rows(&run), placed(&run))
}

/// A placed diagnostic, as [`placed`] lists it.
fn at(id: &str, severity: &str, source: &str) -> (String, String, String) {
    (id.to_string(), severity.to_string(), source.to_string())
}

#[test]
fn a_math_group_gets_a_row_with_its_holes_and_lines() {
    let (rows, diagnostics) = rendered("Let \\(\\alpha + \\mbox{b} = c\\) and \\[x \\textbf{y}\\] hold.\n");
    assert_eq!(diagnostics, []);
    assert_eq!(rows, [
        json!({ "kind": "group", "display": "inline",
                "formula": { "holes": ["mbox"], "lines": [["α + ", 0, " = 𝑐"]] } }),
        json!({ "kind": "group", "display": "display",
                "formula": { "holes": ["textbf"], "lines": [["𝑥 ", 0]] } }),
    ]);
}

#[test]
fn an_environment_is_rendered_one_equation_line_at_a_time() {
    let source = "\\begin{align}\n  a &= \\begin{pmatrix} 1 & 2 \\\\ 30 & 4 \\end{pmatrix} \\label{eq:a} \\\\\n  b &\\le \\text{c} \\tag{T}\n\\end{align}\n";
    let (rows, diagnostics) = rendered(source);
    assert_eq!(diagnostics, []);
    assert_eq!(rows, [json!({ "kind": "environment", "environment": "align", "lines": [
        { "holes": [], "lines": [["𝑎 = ⎛  1  2 ⎞"], ["    ⎝ 30  4 ⎠"]] },
        { "holes": ["text"], "lines": [["𝑏 ≤ ", 0]] },
    ] })]);
}

#[test]
fn a_formula_inside_a_text_command_of_a_formula_gets_a_row_of_its_own() {
    let (rows, diagnostics) = rendered("\\(a \\text{for all \\(b^2\\)}\\)\n");
    assert_eq!(diagnostics, []);
    let lines: Vec<&Value> = rows.iter().map(|row| &row["formula"]["lines"]).collect();
    assert_eq!(lines, [&json!([["𝑎 ", 0]]), &json!([["𝑏²"]])]);
}

#[test]
fn a_diagnostic_of_techxt_is_reported_at_the_formula() {
    let (rows, diagnostics) = rendered("A \\(\\frac{a}\\) b.\n");
    assert_eq!(diagnostics, [at("math-techxt.conversion", "Error", "\\(\\frac{a}\\)")]);
    // The formula is rendered as techxt could.
    assert_eq!(rows.len(), 1);
}

#[test]
fn a_diagnostic_of_techxt_in_an_equation_line_is_reported_at_the_line() {
    let (_, diagnostics) = rendered("\\begin{align}\n  a \\\\\n  \\frac{b} \\label{eq:b}\n\\end{align}\n");
    // At the line's first node of mathematics: the command, its arguments
    // being techxt's to read.
    assert_eq!(diagnostics, [at("math-techxt.conversion", "Error", "\\frac")]);
}

#[test]
fn a_private_use_character_of_a_formula_is_an_error_and_the_formula_gets_no_row() {
    let (rows, diagnostics) = rendered("A \\(a + \u{E001}\\) b.\n");
    assert_eq!(diagnostics, [at("math-techxt.private-use-character", "Error", "\\(a + \u{E001}\\)")]);
    assert_eq!(rows, Vec::<Value>::new());
    // Inside a text command, the character is text.
    let (rows, diagnostics) = rendered("\\(a + \\text{\u{E001}}\\)\n");
    assert_eq!(diagnostics, []);
    assert_eq!(rows[0]["formula"]["lines"], json!([["𝑎 + ", 0]]));
}

#[test]
fn an_equation_line_without_mathematics_is_an_error() {
    let (rows, diagnostics) = rendered("\\begin{gather}\n  a \\\\ \\label{eq:x}\n\\end{gather}\n");
    assert_eq!(diagnostics, [at("math-techxt.empty-line", "Error", " ")]);
    assert_eq!(rows.len(), 1);
}
