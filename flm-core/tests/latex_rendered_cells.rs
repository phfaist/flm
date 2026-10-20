//! Tables of cells on the `latex-rendered` backend: the documents of
//! `tests/cells_latex.rs` written as `flmxCells` environments, with one
//! braced spec per cell in the key `cells` and the rows of the grid, every
//! grid position a column: spans, alignments, column and row headers, rules
//! at the four edges, the shipped styles and a host's styles, an empty
//! position, a cell holding two paragraphs and one holding a table, a list
//! followed by a paragraph and a paragraph that begins with a bracket, a
//! paragraph that is a link target, a table wider than the line, a table in a
//! float, a user definition writing a table, and what the reader leaves out.
//!
//! Goldens live under `tests/goldens/latex-rendered/`, in the shape of
//! `tests/latex_rendered.rs` (the document, then the style file, each under a
//! marker line); `tests/latex_rendered_compile.rs` compiles every one with
//! `lualatex`.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry};
use flm_core::core::run::Run;
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::latex::{LatexPreambleDef, preamble};
use flm_core::stdcore::backends::latex_rendered::{
    BACKEND, DOCUMENT, LatexRenderedRenderOptions, LatexSegment, WORKFLOW, emit_tree, style_artifact,
};
use flm_core::stdcore::backends::pres::{self, CellSpec, CellsPayload, ParPayload};
use flm_core::stdcore::features::cells::{CellStyle, CellsConfig, STYLES_SLOT};
use flm_core::stdcore::features::userdefs::UserdefsConfig;

use common::lowering::PresSite;
use common::stdcore::identifiers;

// --- helpers -------------------------------------------------------------------------------

fn global() -> StdcoreConfig {
    StdcoreConfig::new().cells(CellsConfig::new().global(true))
}

fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(global())
}

fn run(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    common::stdcore::run_workflow(registry, WORKFLOW, &[document], &Overrides::none())
}

/// Runs the workflow over `document` with `registry`, asserting that nothing
/// was reported.
fn render_with(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    let run = run(registry, document);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    run
}

fn render(document: &str) -> Run {
    render_with(&registry(), document)
}

/// The entry artifact's text.
fn document(run: &Run) -> String {
    let set = run.table(DOCUMENT).expect("declared").unwrap_or_else(|| {
        panic!("no latex-rendered output; diagnostics:\n{}", run.diagnostics().render_all())
    });
    let entry = set.entry().expect("the entry artifact");
    String::from_utf8(entry.as_bytes().expect("bytes").to_vec()).expect("UTF-8")
}

/// The content of the document: what stands between `\begin{document}` and
/// `\end{document}`.
fn content(run: &Run) -> String {
    let text = document(run);
    let start = text.find("\\begin{document}\n").expect("the skeleton") + "\\begin{document}\n".len();
    let end = text.rfind("\\end{document}").expect("the skeleton");
    String::from(&text[start..end])
}

/// The style file's text.
fn style(run: &Run) -> String {
    let set = run.table(DOCUMENT).expect("declared").expect("written");
    let style = style_artifact(set).expect("the style file");
    String::from_utf8(style.as_bytes().expect("bytes").to_vec()).expect("UTF-8")
}

/// Asserts the run's document and style file against
/// `tests/goldens/latex-rendered/<name>`, each under a marker line.
fn assert_golden(name: &str, run: &Run) {
    assert_golden_of(name, run, &document(run));
}

/// [`assert_golden`] with `document` in place of the run's document.
fn assert_golden_of(name: &str, run: &Run, document: &str) {
    let set = run.table(DOCUMENT).expect("declared").expect("written");
    let style = style_artifact(set).expect("the style file");
    let path = style.fixed_path.as_ref().map(|path| path.as_str().to_string()).unwrap_or_default();
    let text = format!(
        "% ===== latex-rendered:document =====\n{}% ===== latex-rendered:style {path} =====\n{}",
        document,
        self::style(run)
    );
    let mut golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    golden.push("tests/goldens/latex-rendered");
    golden.push(name);
    assert_text_golden(golden, &text);
}

// --- the tables ------------------------------------------------------------------------------

/// A header row and two rows of data.
const SIMPLE: &str = "\\begin{cells}\n  \\celldata<H>{One & Two}\n  \\celldata{A & B \\\\ C & D}\n\\end{cells}\n";

#[test]
fn a_simple_table_is_one_environment_with_a_spec_per_cell() {
    let run = render(SIMPLE);
    assert_eq!(
        content(&run),
        "\\begin{flmxCells}[rows=3, cols=2, cells={{row=1, col=1, header=column}, \
         {row=1, col=2, header=column}, {row=2, col=1}, {row=2, col=2}, {row=3, col=1}, \
         {row=3, col=2}}]\n\
         {One} & {Two} \\\\\n\
         {A} & {B} \\\\\n\
         {C} & {D} \\\\\n\
         \\end{flmxCells}\n"
    );
    let style = style(&run);
    assert!(style.contains("% flm.latex.def.flmx.cells\n"), "{style}");
    assert!(style.contains("\\RequirePackage{tabularray}"), "{style}");
    assert!(style.contains("\\UseTblrLibrary{booktabs}"), "{style}");
    assert_golden("cells-simple.tex", &run);
}

#[test]
fn a_document_holding_no_table_holds_neither_the_chunk_nor_tabularray() {
    for text in ["No table here.\n", "\\begin{cells}\\end{cells}\n"] {
        let run = run(&registry(), text);
        let style = style(&run);
        assert!(!style.contains("flm.latex.def.flmx.cells") && !style.contains("tabularray"), "{text}: {style}");
        assert!(!document(&run).contains("flmxCells"), "{text}");
    }
}

/// Every token: spans, alignments, a column and a row header, the shipped
/// colors, rules at the four edges, an empty position.
const TOKENS: &str = "\\begin{cells}\n  \
                      \\cell<H l>{Name}\\cell<H c>{City}\\cell<H r>{Height}\\\\\n  \
                      \\cell<rH>{John}\\cell<green>[\\merge{2-3};2]{Berlin}\\cell<red blue>{180}\\\\\n  \
                      \\cell<rH lvert>{Anna}\\cell<yellow rvert>[3]{170}\\\\\n  \
                      \\cell<rule{above} rule{below}>[\\merge{1-3}]{Together}\n\
                      \\end{cells}\n";

#[test]
fn every_token_reaches_the_cells_spec() {
    let run = render(TOKENS);
    let written = content(&run);
    for expected in [
        "\\begin{flmxCells}[rows=4, cols=3, cells={",
        "{row=1, col=1, align=left, header=column}",
        "{row=1, col=2, align=center, header=column}",
        "{row=1, col=3, align=right, header=column}",
        "{row=2, col=1, header=row}",
        "{row=2, col=2, row-span=2, styles={green}}",
        "{row=2, col=3, styles={red blue}}",
        "{row=3, col=1, header=row, rules={left}}",
        "{row=3, col=3, rules={right}, styles={yellow}}",
        "{row=4, col=1, col-span=3, rules={above below}}",
        // The covered position (3, 2) and the positions the last cell covers
        // hold nothing.
        "{John} & {Berlin} & {180} \\\\\n",
        "{Anna} &  & {170} \\\\\n",
        "{Together} &  &  \\\\\n",
    ] {
        assert!(written.contains(expected), "{expected}\n{written}");
    }
    assert_golden("cells-tokens.tex", &run);
}

#[test]
fn a_cell_holds_a_formula_a_reference_two_paragraphs_a_table_and_a_list() {
    let text = "\\section{Numbers}\\label{sec:n}\n\n\
                \\begin{cells}\n  \
                \\cell{\\(x^2\\), see \\ref{sec:n}}\n  \
                \\cell{First paragraph.\n\nSecond paragraph.}\\\\\n  \
                \\cell[2]{\\begin{cells}\\celldata{a & b}\\end{cells}}\\\\\n  \
                \\cell{\\begin{itemize}\\item A list, then\\end{itemize}a paragraph.}\n  \
                \\cell{One.\n\n[1] A paragraph that begins with a bracket.}\n\
                \\end{cells}\n";
    let run = render(text);
    let written = content(&run);
    for expected in [
        "{\\(x^2\\), see \\flmxLink[kind=ref, href={\\#sec-n}, target=sec-n]{",
        // A cell's blocks are separated by `\par`: a `\\` would have no line
        // to end after the list, and would read `[1]` as a length.
        " & {First paragraph.\\par Second\nparagraph.} \\\\\n",
        "\\end{flmxList}\n\\par a paragraph.} & {One.\\par [1] A paragraph",
        // The table in the cell, braced so that its & stays inside the cell.
        " & {\n\\begin{flmxCells}[rows=1, cols=2, cells={{row=1, col=1}, {row=1, col=2}}]\n{a} & {b} \\\\\n\\end{flmxCells}\n} \\\\\n",
    ] {
        assert!(written.contains(expected), "{expected}\n{written}");
    }
    assert_golden("cells-content.tex", &run);
}

/// A table wider than the line, which the default sets again with `X`
/// columns at `\flmxCellsMaxWidth`, a cell of its last row holding two
/// paragraphs.
const WIDE: &str = "\\begin{cells}\n  \\celldata<H>{Term & Meaning}\n  \\celldata{Cell & \
                    A table of cells is a grid of rows and columns in which each cell occupies a \
                    rectangle of grid positions, one position or several when the cell spans rows or \
                    columns, and its content is laid out as blocks.}\n  \\cell{Blocks}\\cell{A cell of the \
                    wide table holds two paragraphs, which the table set again with its columns wrapped \
                    still separates.\n\nThe second paragraph.}\n\\end{cells}\n";

#[test]
fn a_table_wider_than_the_line_is_one_environment_like_any() {
    let run = render(WIDE);
    assert!(content(&run).starts_with("\\begin{flmxCells}[rows=3, cols=2, cells="), "{}", content(&run));
    assert_golden("cells-wide.tex", &run);
}

#[test]
fn a_table_in_a_table_float_stands_in_the_float_with_its_caption() {
    let registry = common::stdcore::registry(StdcoreConfig::new());
    let run = render_with(
        &registry,
        "\\begin{table}\\begin{cells}\\celldata<H>{A & B}\\celldata{1 & 2}\\end{cells}\
         \\caption{Numbers}\\label{table:x}\\end{table}\n\n\
         See \\ref{table:x} and \\ref[S]{table:x}.\n",
    );
    assert!(content(&run).contains("\\begin{flmxCells}[rows=2, cols=2, "), "{}", content(&run));
    assert_golden("cells-float.tex", &run);
}

#[test]
fn a_user_definition_writing_a_table_is_expanded_where_it_is_used() {
    let onecell = Definition::macro_(
        "onecell",
        vec![ArgDef::mandatory("styles"), ArgDef::mandatory("placement"), ArgDef::mandatory("content")],
    )
    .text("\\begin{cells}\\cell<#1>[#2]{#3}\\cell{x}\\end{cells}");
    let registry = common::stdcore::registry(global().userdefs(UserdefsConfig::new().definitions(vec![onecell])));
    let run = render_with(&registry, "\\onecell{r H}{2}{right}\n");
    // The cell stands in column 2, right-aligned, a header; column 1 is an
    // empty position.
    let written = content(&run);
    assert!(written.contains("{row=1, col=2, align=right, header=column"), "{written}");
    assert!(written.contains("\n & {right} & {x} \\\\\n"), "{written}");
}

#[test]
fn what_the_reader_leaves_out_is_a_dropped_mark_before_the_table_or_where_it_stood() {
    let run = run(
        &registry(),
        "\\begin{cells}\nstray\n\\cell{A}\\cell[1]{B}\\\\\n\\celldata{x & \\emph{y \\\\ z}}\n\\end{cells}\n",
    );
    let reported = identifiers(&run);
    for condition in ["flm.cells.content-outside-cell", "flm.cells.overlap", "flm.cells.outside-table"] {
        assert!(reported.iter().any(|id| id == condition), "{condition}: {reported:?}");
    }
    let written = content(&run);
    assert!(written.contains("\\flmxDropped"), "{written}");
    assert_golden("cells-marks.tex", &run);
}

// --- a host's custom styles --------------------------------------------------------------------

/// The identifier of the host's piece, `flm.latex.def.host-cells`.
const HOST_PIECE: &str = "flm.latex.def.host-cells";

/// A host contributing `orange` and `bold`, each requiring on this backend
/// the host's piece, which registers their tabularray keys.
struct HostStyles;

impl FlmFeature for HostStyles {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("host", "A host's cell styles")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        for (name, docs) in [("orange", "an orange background"), ("bold", "the cell's text in bold")] {
            let style = CellStyle::custom(name, docs).requires(BACKEND, [HOST_PIECE]);
            b.peer_slot::<CellStyle>(STYLES_SLOT, Arc::new(style))?;
        }
        let piece = LatexPreambleDef::new(
            "host-cells",
            "\\flmxDefineCellStyle{orange}{bg=orange!40}\n\\flmxDefineCellStyle{bold}{font=\\bfseries}\n",
            "the keys of the host's cell styles",
        )
        .requires(["flm.latex.def.flmx.cells"]);
        b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(piece))
    }
}

#[test]
fn a_hosts_custom_styles_are_names_in_the_spec_requiring_the_hosts_piece() {
    let mut features = common::stdcore::package_features_but(global(), "");
    features.push(Arc::new(HostStyles));
    let registry = common::stdcore::registry_over(features);
    let run = render_with(&registry, "\\begin{cells}\\cell<r orange bold>{A}\\cell<bold green>{B}\\cell{C}\\end{cells}\n");
    let written = content(&run);
    assert!(written.contains("{row=1, col=1, align=right, styles={orange bold}}"), "{written}");
    assert!(written.contains("{row=1, col=2, styles={bold green}}"), "{written}");
    let style = style(&run);
    assert!(style.contains(&format!("% {HOST_PIECE}\n")), "{style}");
    // The backend's chunk stands before the host's piece, which calls it.
    let (cells, host) = (style.find("% flm.latex.def.flmx.cells\n"), style.find(&format!("% {HOST_PIECE}\n")));
    assert!(cells.is_some() && cells < host, "{style}");
    assert_golden("cells-host-styles.tex", &run);
    // A table without the host's styles does not hold its piece.
    let run = render_with(&registry, "\\begin{cells}\\cell<green>{A}\\end{cells}\n");
    assert!(!self::style(&run).contains(HOST_PIECE));
}

#[test]
fn the_shipped_styles_require_the_feature_s_piece_where_they_color_the_cell() {
    // The four custom styles require the cells feature's style identifier,
    // which its definition piece delivers on this backend; the structural
    // ones require nothing.
    for style in flm_core::stdcore::features::cells::styles::shipped() {
        let custom = ["green", "blue", "yellow", "red"].contains(&style.name.as_str());
        let expected: &[&str] = if custom {
            &[flm_core::stdcore::features::cells::STYLE]
        } else {
            &[]
        };
        let found: Vec<&str> = style.requirements_on(BACKEND).map(|id| id.as_str()).collect();
        assert_eq!(found, expected, "{}", style.name);
    }
}

#[test]
fn a_target_in_a_cell_is_reached_by_a_reference() {
    let run = render(
        "\\begin{cells}\\cell{Before \\begin{equation}\\label{eq:a} x = 1\\end{equation}}\
         \\cell{Plain.}\\end{cells}\n\nSee \\eqref{eq:a}.\n",
    );
    let written = content(&run);
    assert!(written.contains("\\flmxTarget{"), "{written}");
    assert_golden("cells-target.tex", &run);
}

#[test]
fn an_anchored_paragraph_in_a_cell_writes_its_target_first() {
    // No feature of the standard package anchors a paragraph, so the table is
    // built by hand and written into the document of a rendered table, in
    // place of that table, so that the golden compiles.
    let registry = registry();
    let mut site = PresSite::new(&registry);
    let first = site.text("Plain.");
    let first = site.pres(pres::PAR, ParPayload::default(), vec![first]);
    let lead = site.text("Before.");
    let lead = site.pres(pres::PAR, ParPayload::default(), vec![lead]);
    let anchored = site.text("Anchored.");
    let anchored = site.pres(pres::PAR, ParPayload { anchor: Some(String::from("par-a")) }, vec![anchored]);
    let payload = CellsPayload { rows: 1, cols: 2, cells: vec![CellSpec::at(0, 0), CellSpec::at(0, 1)] };
    let table = site.cells(payload, vec![vec![first], vec![lead, anchored]]);
    let tree = site.finish(vec![table]);
    let emitted = emit_tree(&tree, &LatexRenderedRenderOptions::default());
    assert!(emitted.diags.is_empty(), "{}", emitted.diags.render_all());
    let latex: String = emitted
        .segments
        .iter()
        .map(|segment| match segment {
            LatexSegment::Latex(latex) => latex.clone(),
            LatexSegment::Tree(tree) => format!("<tree {tree}>"),
        })
        .collect();
    assert!(latex.contains("{Plain.} & {Before.\\par \\flmxTarget{par-a}Anchored.} \\\\\n"), "{latex}");

    let run = render("\\begin{cells}\\cell{Plain.}\\cell{Before.}\\end{cells}\n");
    let shell = document(&run);
    let rendered = content(&run);
    assert!(shell.contains(rendered.trim_end()), "{shell}");
    assert_golden_of("cells-par-anchor.tex", &run, &shell.replacen(rendered.trim_end(), latex.trim_end(), 1));
}
