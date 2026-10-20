//! The table of contents on the `latex-rendered` backend, end to end over
//! the standard package: the documents `tests/toc.rs` places a table of
//! contents in (numbered sections and subsections, an unnumbered section, a
//! section left out, headings without numbering), each a golden under
//! `tests/goldens/latex-rendered/` holding the document and the style file,
//! which `tests/latex_rendered_compile.rs` compiles with `lualatex` (an
//! entry's page number is a `\pageref*` to the heading's label); and the toc
//! feature's definition piece, which the feature's style identifier brings
//! into the style file.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::pieces::PiecesRenderOptions;
use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};
use flm_core::stdcore::features::toc;
use flm_core::stdcore::features::toc::latex_rendered::PIECE;

use common::latex_rendered::{assert_golden, document, render, style};

/// The standard package with every heading numbered.
fn numbered() -> Arc<FlmRegistry> {
    common::stdcore::registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered)),
    )
}

/// The render request placing the table of contents.
fn with_toc() -> Overrides {
    Overrides {
        render_options: OptionValues::new().with(PiecesRenderOptions::default().include(toc::PIECE)),
        ..Overrides::none()
    }
}

/// Sections and subsections, numbered, one unnumbered and one left out of
/// the table of contents, each holding a paragraph long enough that the
/// headings stand on several pages ([`sections`]).
const SECTIONS: &str = "\\section{Introduction}

Text.

\\subsection{Background}

Text.

\\subsection{Salt \\& pepper}

Text.

\\section[toc=false]{Hidden}

Text.

\\section*{Unnumbered}

Text.

\\section{Results}

Text.

\\subsection{A first result}

Text.

\\subsubsection{A detail}

Text.";

/// [`SECTIONS`] with each `Text.` a paragraph of some twenty lines.
fn sections() -> String {
    let filler = "A sentence that fills the page, written again and again. ".repeat(30);
    SECTIONS.replace("Text.", filler.trim_end())
}

#[test]
fn the_table_of_contents_lists_the_headings_with_page_numbers() {
    let run = render(&numbered(), &[&sections()], &with_toc());
    let written = document(&run);
    assert!(written.contains("\\begin{flmxBlock}[role=flm.toc, kinds={toc}"), "{written}");
    assert!(written.contains("\\begin{flmxList}[kinds={unnumbered toc}, depth=2]"), "{written}");
    assert!(written.contains("\\flmxLink[kind=toc, href={\\#sec--Introduction}, target=sec--Introduction"), "{written}");
    assert!(!written[..written.find("\\end{flmxBlock}").expect("closed")].contains("Hidden"), "{written}");
    let (text, _) = style(&run);
    assert!(text.contains(&format!("% {PIECE} --")), "{text}");
    assert!(text.contains("\\pageref* { flmx:\\flmxKey { target } }"), "{text}");
    // The kind toc is registered by the toc piece alone: the links chunk,
    // written after the features' pieces, must not register it again over
    // the piece's leaders and page number.
    assert_eq!(text.matches("\\flmxDefineLink { toc }").count(), 1, "{text}");
    assert!(!text.contains("xnote-back, toc, url }"), "{text}");
    assert_golden("toc.tex", &run);
}

#[test]
fn without_numbering_an_entry_has_no_number_span() {
    let registry = common::stdcore::registry(StdcoreConfig::new());
    let run = render(&registry, &["\\section{One}\n\n\\subsection{Two}"], &with_toc());
    assert!(!document(&run).contains("flm.toc.entry.number"), "{}", document(&run));
    assert_golden("toc-unnumbered.tex", &run);
}

#[test]
fn a_document_whose_table_of_contents_is_not_placed_does_not_hold_the_piece() {
    let run = render(&numbered(), &["\\section{One}"], &Overrides::none());
    assert!(!style(&run).0.contains(PIECE), "{}", style(&run).0);
}
