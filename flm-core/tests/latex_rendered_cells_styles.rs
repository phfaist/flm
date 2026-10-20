//! The cells feature's definition piece on the `latex-rendered` backend: the
//! four custom styles the feature ships (`green`, `blue`, `yellow`, `red`)
//! registered with `\flmxDefineCellStyle`, the piece in the style file of a
//! document whose table uses one of them and in no other; and a table whose
//! cell holds a footnote, under the footnote kind's two placements. (The
//! tables themselves are the backend's: `tests/latex_rendered_cells.rs`.)
//! Each document is a golden under `tests/goldens/latex-rendered/`, which
//! `tests/latex_rendered_compile.rs` compiles with `lualatex`.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::cells::CellsConfig;
use flm_core::stdcore::features::cells::latex_rendered::PIECE;
use flm_core::stdcore::features::xnotes::{XnoteKind, XnotePlacement, XnotesConfig};

use common::latex_rendered::{assert_golden, document, render, style};

/// The standard package, tables of cells allowed anywhere.
fn global() -> StdcoreConfig {
    StdcoreConfig::new().cells(CellsConfig::new().global(true))
}

/// The standard package under [`global`].
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(global())
}

#[test]
fn the_four_shipped_colors_are_registered_and_the_piece_is_in_the_style_file() {
    let run = render(
        &std(),
        &["\\begin{cells}\n  \\celldata<H>{Green & Blue & Yellow & Red}\n  \
           \\cell<green>{g}\\cell<blue>{b}\\cell<yellow>{y}\\cell<red>{r}\n\\end{cells}\n"],
        &Overrides::none(),
    );
    assert!(document(&run).contains("styles={green}"), "{}", document(&run));
    let (text, _) = style(&run);
    assert!(text.contains(&format!("% {PIECE} --")), "{text}");
    for name in ["green", "blue", "yellow", "red"] {
        let line = format!("\\flmxDefineCellStyle{{{name}}}{{bg=flmxCells");
        assert!(text.contains(&line), "{name}: {text}");
    }
    assert_golden("cells-shipped-styles.tex", &run);
}

#[test]
fn a_table_without_a_custom_style_holds_no_piece() {
    let run = render(
        &std(),
        &["\\begin{cells}\\celldata<H>{a & b}\\cell<c>{1}\\cell<rvert>{2}\\end{cells}"],
        &Overrides::none(),
    );
    let (text, _) = style(&run);
    assert!(!text.contains(PIECE), "{text}");
}

/// The table of `tests/cells_latex.rs` whose cell holds a formula, a
/// reference and a footnote, two paragraphs and a table.
const NOTE_IN_A_CELL: &str = "\\section{Numbers}\\label{sec:n}\n\n\
                              \\begin{cells}\n  \
                              \\cell{\\(x^2\\), see \\ref{sec:n}\\footnote{A note.}}\n  \
                              \\cell{First paragraph.\n\nSecond paragraph.}\\\\\n  \
                              \\cell[2]{\\begin{cells}\\celldata{a & b}\\end{cells}}\n\
                              \\end{cells}\n";

#[test]
fn a_listed_footnote_in_a_cell_is_its_mark_and_an_entry_of_the_list() {
    let run = render(&std(), &[NOTE_IN_A_CELL], &Overrides::none());
    let written = document(&run);
    assert!(written.contains("role=flm.xnotes.xnote.mark"), "{written}");
    assert!(written.contains("role=flm.xnotes.footnote.list"), "{written}");
    assert_golden("cells-footnote-listed.tex", &run);
}

#[test]
fn an_in_place_footnote_in_a_cell_is_the_note_command() {
    let xnotes = XnotesConfig {
        kinds: vec![XnoteKind::footnote().placement(XnotePlacement::InPlace)],
    };
    let registry = common::stdcore::registry(global().xnotes(xnotes));
    let run = render(&registry, &[NOTE_IN_A_CELL], &Overrides::none());
    let written = document(&run);
    assert!(
        written.contains("\\flmxXnote[kind=footnote, anchor=fn-1, doc-role=footnote]"),
        "{written}"
    );
    assert_golden("cells-footnote-in-place.tex", &run);
}
