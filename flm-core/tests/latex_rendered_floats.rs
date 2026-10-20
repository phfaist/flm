//! Floats on the `latex-rendered` backend, end to end over the standard
//! package: the documents `tests/floats_html.rs` renders (the four figures,
//! the unnumbered ones, a configured float type, a note and a reference in a
//! caption) and a table float, each a golden under
//! `tests/goldens/latex-rendered/` holding the document and the style file,
//! which `tests/latex_rendered_compile.rs` compiles with `lualatex`; and the
//! floats feature's definition piece, which the feature's style identifier
//! brings into the style file.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::floats::latex_rendered::PIECE;
use flm_core::stdcore::features::floats::{FloatNumbering, FloatType, FloatsConfig};

use common::latex_rendered::{assert_golden, document, render, style};

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The four figures of the HTML tests: no caption, a caption, a label, a
/// caption before the image.
const FOUR: &str = "\\begin{figure}\\includegraphics{a.png}\\end{figure}\n\n\
                    \\begin{figure}\\includegraphics{b.png}\\caption{A photo}\\end{figure}\n\n\
                    \\begin{figure}\\label{figure:c}\\includegraphics{c.png}\\end{figure}\n\n\
                    \\begin{figure}\\caption{Last}\\label{figure:d}\\includegraphics[alt=A plot]{d.png}\\end{figure}\n\n\
                    See \\ref{figure:c} and \\ref{figure:d}.\n";

#[test]
fn the_four_figures_are_figure_environments_with_the_line_under_the_content() {
    let run = render(&std(), &[FOUR], &Overrides::none());
    let written = document(&run);
    assert!(
        written.contains("\\begin{flmxBlock}[role=flm.floats.float, kinds={figure numbered}, anchor=figure-2]"),
        "{written}"
    );
    assert!(
        written.contains("\\flmxSpan[role=flm.floats.float.number]{Figure\u{a0}2}: Last"),
        "{written}"
    );
    let (text, _) = style(&run);
    assert!(text.contains(&format!("% {PIECE} --")), "{text}");
    assert!(text.contains("\\flmxDefineBlock{figure}%"), "{text}");
    assert_golden("floats-figures.tex", &run);
}

#[test]
fn unnumbered_figures_and_a_table_float() {
    let run = render(
        &std(),
        &["\\begin{figure}\\includegraphics{a.png}\\caption{A photo}\\end{figure}\n\n\
           \\begin{figure}\\nonumber\\label{figure:b}\\includegraphics{b.png}\\end{figure}\n\n\
           \\begin{table}\\begin{cells}\\celldata<H>{Name & Value}\\celldata{a & 1}\\end{cells}\
           \\caption{Some values}\\label{table:v}\\end{table}\n\n\
           See \\ref{table:v}."],
        &Overrides::none(),
    );
    let written = document(&run);
    assert!(written.contains("[role=flm.floats.float, kinds={figure unnumbered}]"), "{written}");
    assert!(
        written.contains("[role=flm.floats.float, kinds={table numbered}, anchor=table-1]"),
        "{written}"
    );
    assert_golden("floats-unnumbered-table.tex", &run);
}

#[test]
fn a_configured_type_is_written_with_the_block_s_default() {
    let config = FloatsConfig::new()
        .float_type(FloatType::new("diagram", ["any"]).numbering(FloatNumbering::AutoNumbered));
    let registry = common::stdcore::registry(StdcoreConfig::new().floats(config));
    let run = render(
        &registry,
        &["\\begin{diagram}\\label{diagram:x}Boxes.\\caption{Arrows}\\end{diagram}"],
        &Overrides::none(),
    );
    let written = document(&run);
    assert!(
        written.contains("[role=flm.floats.float, kinds={diagram numbered}, anchor=diagram-1]"),
        "{written}"
    );
    assert_golden("floats-configured-type.tex", &run);
}

#[test]
fn a_note_and_a_reference_inside_a_caption() {
    let run = render(
        &std(),
        &["\\section{Results}\\label{sec:results}\n\n\
           \\begin{figure}\\includegraphics{plot.png}\\caption{A plot\\footnote{Measured twice.} of \
           \\ref{sec:results}.}\\label{figure:plot}\\end{figure}\n\n\
           See \\ref{figure:plot}.\n"],
        &Overrides::none(),
    );
    let written = document(&run);
    assert!(written.contains("role=flm.xnotes.xnote.mark"), "{written}");
    assert_golden("floats-caption-note.tex", &run);
}

#[test]
fn a_document_without_floats_holds_no_piece() {
    let run = render(&std(), &["No float here."], &Overrides::none());
    let (text, _) = style(&run);
    assert!(!text.contains(PIECE), "{text}");
}
