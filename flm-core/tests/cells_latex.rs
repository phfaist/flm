//! The `cells` feature on the recomposed LaTeX
//! (`flm_core::stdcore::features::cells::latex`): one call of the hook
//! `\flmCellsTable` over the two column specifications, the `tabularray`
//! options and the rows, the definition piece and the two packages it
//! requests (test 11); a table in a `table` float with its caption and a
//! reference to it (test 12, the LaTeX part); a user definition writing a
//! table (test 17, the LaTeX part); the marks of what the reader left out as
//! comment lines (test 6, the LaTeX part); a host's custom styles as table
//! commands requiring the host's piece. The goldens are compiled under both
//! engines by `tests/latex_compile.rs`, which also compiles the author's
//! redefinition of the hook.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry};
use flm_core::core::run::Run;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::latex::{LatexPreambleDef, packages, preamble};
use flm_core::stdcore::backends::latex_recomposed;
use flm_core::stdcore::features::cells::latex::definition_piece;
use flm_core::stdcore::features::cells::{CellStyle, CellsConfig, LATEX_PIECE, STYLES_SLOT};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::model::names::Name;

use common::latex::{assert_golden, output, style, style_if_any};
use common::stdcore::identifiers;

fn global() -> StdcoreConfig {
    StdcoreConfig::new().cells(CellsConfig::new().global(true))
}

fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(global())
}

fn run(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    common::stdcore::run_workflow(registry, latex_recomposed::WORKFLOW, &[document], &Overrides::none())
}

/// The recomposed LaTeX and the style file of `document` under `registry`;
/// the run must report nothing.
fn written(registry: &Arc<FlmRegistry>, document: &str) -> (String, String, Run) {
    let run = run(registry, document);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let written = output(&run);
    let style = style(&run).0;
    (written, style, run)
}

// --- test 11 -----------------------------------------------------------------------------------

/// A header row and two rows of data, as the Python implementation's simple
/// table.
const SIMPLE: &str = "\\begin{cells}\n  \\celldata<H>{One & Two}\n  \\celldata{A & B \\\\ C & D}\n\\end{cells}\n";

#[test]
fn a_simple_table_is_one_call_of_the_hook() {
    let (written, style, run) = written(&registry(), SIMPLE);
    assert_eq!(
        written,
        "\\flmCellsTable{cc}{X[-1]X[-1]}{hspan=minimal,\n  \
         cell{1}{1}={}{m, font={\\flmCellsHeaderFont}},\n  \
         cell{1}{2}={}{m, font={\\flmCellsHeaderFont}},\n  \
         hline{2}={1}{.4pt,solid},\n  \
         hline{2}={2}{.4pt,solid}}{\\toprule\n\
         One&Two\\\\\n\
         A&B\\\\\n\
         C&D\\\\\n\
         \\bottomrule\n}\n%%\n"
    );
    // The piece, and the two packages it requests.
    assert!(style.contains(&format!("% {LATEX_PIECE}\n")), "{style}");
    for package in [packages::TABULARRAY, packages::XCOLOR] {
        assert!(style.contains(&format!("% {package}\n")), "{package}: {style}");
    }
    assert!(style.contains("\\RequirePackage{tabularray}") && style.contains("\\RequirePackage{xcolor}"), "{style}");
    assert_golden("cells-simple.tex", &run);
}

#[test]
fn a_document_holding_no_table_holds_neither_the_piece_nor_the_packages() {
    for document in ["No table here.\n", "\\begin{cells}\\end{cells}\n"] {
        let run = run(&registry(), document);
        let style = style_if_any(&run).map(|(text, _)| text).unwrap_or_default();
        assert!(!style.contains(LATEX_PIECE) && !style.contains("tabularray"), "{document}: {style}");
        assert!(!output(&run).contains("flmCellsTable"), "{document}");
    }
}

/// Every token: spans, alignments, a column and a row header, the shipped
/// colors, rules at the four edges.
const TOKENS: &str = "\\begin{cells}\n  \
                      \\cell<H l>{Name}\\cell<H c>{City}\\cell<H r>{Height}\\\\\n  \
                      \\cell<rH>{John}\\cell<green>[\\merge{2-3};2]{Berlin}\\cell<red blue>{180}\\\\\n  \
                      \\cell<rH lvert>{Anna}\\cell<yellow rvert>[3]{170}\\\\\n  \
                      \\cell<rule{above} rule{below}>[\\merge{1-3}]{Together}\n\
                      \\end{cells}\n";

#[test]
fn every_token_reaches_its_entry_its_rule_or_its_table_command() {
    let (written, style, run) = written(&registry(), TOKENS);
    for expected in [
        "\\flmCellsTable{ccc}{X[-1]X[-1]X[-1]}{hspan=minimal,",
        "cell{1}{1}={}{l, font={\\flmCellsHeaderFont}}",
        "cell{1}{2}={}{c, font={\\flmCellsHeaderFont}}",
        "cell{1}{3}={}{r, font={\\flmCellsHeaderFont}}",
        "cell{2}{1}={}{m, font={\\flmCellsHeaderFont}}",
        "cell{2}{2}={r=2}{m}",
        "cell{4}{1}={c=3}{m}",
        "hline{2}={1}{.4pt,solid}",
        "hline{2}={3}{.4pt,solid}",
        "vline{1}={3}{.4pt,solid}",
        "vline{4}={3}{.4pt,solid}",
        "hline{4}={1-3}{.4pt,solid}",
        "hline{5}={1-3}{.4pt,solid}",
        "John&\\flmCellsSet{green}Berlin&\\flmCellsSet{red}\\flmCellsSet{blue}180\\\\\n",
        "Anna&&\\flmCellsSet{yellow}170\\\\\n",
        "Together&&\\\\\n",
    ] {
        assert!(written.contains(expected), "{expected}\n{written}");
    }
    // A row header draws no rule under it.
    assert!(!written.contains("hline{3}"), "{written}");
    assert!(style.contains("\\flmCellsStyleKeys{green}{bg=flmCellsColorgreen}"), "{style}");
    assert_golden("cells-tokens.tex", &run);
}

#[test]
fn a_cell_holds_a_formula_a_reference_a_footnote_two_paragraphs_and_a_table() {
    let document = "\\section{Numbers}\\label{sec:n}\n\n\
                    \\begin{cells}\n  \
                    \\cell{\\(x^2\\), see \\ref{sec:n}\\footnote{A note.}}\n  \
                    \\cell{First paragraph.\n\nSecond paragraph.}\\\\\n  \
                    \\cell[2]{\\begin{cells}\\celldata{a & b}\\end{cells}}\n\
                    \\end{cells}\n";
    let (written, _, run) = written(&registry(), document);
    for expected in [
        "\\(x^2\\), see \\flmRef{ref1}\\flmXNfootnote{A note.}&",
        "First paragraph.\n\nSecond paragraph.\\\\\n",
        "&\\flmCellsTable{cc}{X[-1]X[-1]}{hspan=minimal}{\\toprule\na&b\\\\\n\\bottomrule\n}\\\\\n",
    ] {
        assert!(written.contains(expected), "{expected}\n{written}");
    }
    assert_golden("cells-content.tex", &run);
}

/// A table wider than the line, which the hook sets again with the fitted
/// columns.
const WIDE: &str = "\\begin{cells}\n  \\celldata<H>{Term & Meaning}\n  \\celldata{Cell & \
                        A table of cells is a grid of rows and columns in which each cell occupies a \
                        rectangle of grid positions, one position or several when the cell spans rows or \
                        columns, and its content is laid out as blocks.}\n\\end{cells}\n";

#[test]
fn a_table_wider_than_the_line_is_one_call_like_any() {
    let (written, _, run) = written(&registry(), WIDE);
    assert!(written.starts_with("\\flmCellsTable{cc}{X[-1]X[-1]}{hspan=minimal,"), "{written}");
    assert_golden("cells-wide.tex", &run);
}

// --- test 12, the LaTeX part -------------------------------------------------------------------

#[test]
fn a_table_in_a_table_float_stands_in_the_float_hook_with_its_caption() {
    let registry = common::stdcore::registry(StdcoreConfig::new());
    let (written, _, run) = written(
        &registry,
        "\\begin{table}\\begin{cells}\\celldata<H>{A & B}\\celldata{1 & 2}\\end{cells}\
         \\caption{Numbers}\\label{table:x}\\end{table}\n\n\
         See \\ref{table:x} and \\ref[S]{table:x}.\n",
    );
    assert!(
        written.contains("\\begin{flmFloat}{table}{NumCap}\\flmCellsTable{cc}{X[-1]X[-1]}{hspan=minimal,"),
        "{written}"
    );
    assert!(written.contains("\\bottomrule\n}\\caption{Numbers}\\flmLabels{\\label{ref1}}\\end{flmFloat}"), "{written}");
    assert!(written.contains("See \\flmRef{ref1} and \\flmRef[S]{ref1}."), "{written}");
    assert_golden("cells-float.tex", &run);
}

// --- test 17, the LaTeX part -------------------------------------------------------------------

#[test]
fn a_user_definition_writing_a_table_is_expanded_where_it_is_used() {
    let onecell = Definition::macro_(
        "onecell",
        vec![ArgDef::mandatory("styles"), ArgDef::mandatory("placement"), ArgDef::mandatory("content")],
    )
    .text("\\begin{cells}\\cell<#1>[#2]{#3}\\cell{x}\\end{cells}");
    let registry = common::stdcore::registry(global().userdefs(UserdefsConfig::new().definitions(vec![onecell])));
    let (written, _, _) = written(&registry, "\\onecell{r H}{2}{right}\n");
    // The placement and the styles are read where the use site is expanded:
    // the cell stands in column 2, right-aligned, a header.
    assert!(written.contains("cell{1}{2}={}{r, font={\\flmCellsHeaderFont}}"), "{written}");
    assert!(written.contains("\\toprule\n&right&x\\\\\n"), "{written}");
}

// --- test 6, the LaTeX part --------------------------------------------------------------------

#[test]
fn what_the_reader_leaves_out_is_a_comment_line_before_the_table_or_where_it_stood() {
    let run = run(
        &registry(),
        "\\begin{cells}\nstray\n\\cell{A}\\cell[1]{B}\\\\\n\\celldata{x & \\emph{y \\\\ z}}\n\\end{cells}\n",
    );
    let written = output(&run);
    assert!(
        written.starts_with(
            "% flm: dropped content outside \\cell\n% flm: dropped cell overlapping another\n\\flmCellsTable{"
        ),
        "{written}"
    );
    assert!(written.contains("\\emph{y \n% flm: dropped \\\\ out of place\n z}"), "{written}");
    let reported = identifiers(&run);
    for condition in ["flm.cells.content-outside-cell", "flm.cells.overlap", "flm.cells.outside-table"] {
        assert!(reported.iter().any(|id| id == condition), "{condition}: {reported:?}");
    }
    assert_golden("cells-marks.tex", &run);
}

// --- a host's custom styles --------------------------------------------------------------------

/// The identifier of the host's piece, `flm.latex.def.host-cells`.
const HOST_PIECE: &str = "flm.latex.def.host-cells";

/// A host contributing `orange` and `bold`, each requiring on the recomposed
/// LaTeX the host's piece, which registers their keys.
struct HostStyles;

impl FlmFeature for HostStyles {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("host", "A host's cell styles")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        for (name, docs) in [("orange", "an orange background"), ("bold", "the cell's text in bold")] {
            let style = CellStyle::custom(name, docs).requires(latex_recomposed::BACKEND, [HOST_PIECE]);
            b.peer_slot::<CellStyle>(STYLES_SLOT, Arc::new(style))?;
        }
        let piece = LatexPreambleDef::new(
            "host-cells",
            "\\flmCellsStyleKeys{orange}{bg=orange}\n\\flmCellsStyleKeys{bold}{cmd=\\bfseries}\n",
            "the keys of the host's cell styles",
        )
        .requires([LATEX_PIECE]);
        b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(piece))
    }
}

#[test]
fn a_hosts_custom_styles_are_table_commands_in_token_order_requiring_the_hosts_piece() {
    let mut features = common::stdcore::package_features_but(global(), "");
    features.push(Arc::new(HostStyles));
    let registry = common::stdcore::registry_over(features);
    let (out, style, run) =
        written(&registry, "\\begin{cells}\\cell<r orange bold>{A}\\cell<bold green>{B}\\cell{C}\\end{cells}\n");
    assert!(
        out.contains("\\toprule\n\\flmCellsSet{orange}\\flmCellsSet{bold}A&\\flmCellsSet{bold}\\flmCellsSet{green}B&C\\\\\n"),
        "{out}"
    );
    assert!(out.contains("cell{1}{1}={}{r}"), "{out}");
    assert!(style.contains(&format!("% {HOST_PIECE}\n")), "{style}");
    // The feature's piece stands before the host's, which calls it.
    let (cells, host) = (style.find(&format!("% {LATEX_PIECE}\n")), style.find(&format!("% {HOST_PIECE}\n")));
    assert!(cells.is_some() && cells < host, "{style}");
    assert_golden("cells-host-styles.tex", &run);
    // A table without the host's styles does not hold its piece.
    let (_, style, _) = written(&registry, "\\begin{cells}\\cell<green>{A}\\end{cells}\n");
    assert!(!style.contains(HOST_PIECE), "{style}");
}

#[test]
fn the_piece_requests_both_packages() {
    let piece = definition_piece();
    assert_eq!(piece.requires, [Name::from(packages::TABULARRAY), Name::from(packages::XCOLOR)]);
}
