//! Citations on the `latex-rendered` backend, end to end over the standard
//! package: the documents `tests/citations_html.rs` renders (the worked
//! example, the superscript form, the in-place placement), each a golden
//! under `tests/goldens/latex-rendered/` holding the document and the style
//! file, which `tests/latex_rendered_compile.rs` compiles with `lualatex`;
//! and the citations feature's definition piece, which the feature's style
//! identifier brings into the style file.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::citations::latex_rendered::PIECE;
use flm_core::stdcore::features::citations::{
    CitationPlacement, CitationProvider, CitationsConfig, MarkForm,
};

use common::citations::FromMap;
use common::latex_rendered::{assert_golden, document, render, style};

/// The works the test provider knows.
const WORKS: &[(&str, &str)] = &[
    ("knuth84", "D. Knuth, \\emph{The Art of Computer Programming}, 1984"),
    ("lamport94", "L. Lamport, \\emph{LaTeX: A Document Preparation System}, 1994"),
    ("shor95", "P. Shor, Scheme for reducing decoherence, 1995"),
    ("gottesman97", "D. Gottesman, Stabilizer codes, 1997"),
];

/// The standard package under `config`, the provider answering the keys of
/// [`WORKS`].
fn registry(config: CitationsConfig) -> Arc<FlmRegistry> {
    let provider: Arc<dyn CitationProvider> = Arc::new(FromMap::of(WORKS));
    common::citations::registry_with(StdcoreConfig::new().citations(config), vec![provider])
}

/// The worked example of the HTML tests, every work known.
const WORKED: &str = "\\section{Results \\cite{shor95}}\\label{sec:results}\n\n\
Codes of this family are well studied \\cite{knuth84,lamport94}. The bound is tight \
\\cite{shor95}\\cite[Thm 4]{knuth84}\\cite{lamport94}\\cite[Thm 4]{gottesman97}. \
A work first cited in a footnote.\\footnote{See \\cite{gottesman97}.}\n\n\
As shown in \\R\\cite{lamport94} and \\R[S]\\cite{knuth84,shor95}, the claim holds (see, for \
instance, \\R[I]\\cite{lamport94,shor95}); a mark standing after a parenthesis shows its \
space (\\cite{knuth84}). See \\ref{sec:results}.";

#[test]
fn the_worked_example_writes_the_marks_and_the_list_of_cited_works() {
    let run = render(&registry(CitationsConfig::new()), &[WORKED], &Overrides::none());
    let written = document(&run);
    assert!(written.contains("\\flmxSpan[role=flm.citations.space]"), "{written}");
    assert!(
        written.contains("\\flmxSpan[role=flm.citations.mark, kinds={attached}]{[\\flmxLink[kind=cite"),
        "{written}"
    );
    assert!(written.contains("\\flmxSpan[role=flm.citations.word]"), "{written}");
    assert!(
        written.contains("\\begin{flmxBlock}[role=flm.citations.bibliography, kinds={citations}, doc-role=bibliography]"),
        "{written}"
    );
    assert!(
        written.contains("\\flmxItem[label={\\flmxSpan[role=flm.citations.entry.number]{[1]}}"),
        "{written}"
    );
    let (text, _) = style(&run);
    assert!(text.contains(&format!("% {PIECE} --")), "{text}");
    assert_golden("citations-worked.tex", &run);
}

#[test]
fn under_the_superscript_form_the_mark_keeps_its_style() {
    let config = CitationsConfig { mark: MarkForm::Superscript, ..CitationsConfig::new() };
    let run = render(
        &registry(config),
        &["Codes of this family are well studied \\cite{knuth84,lamport94,shor95}. The bound is \
           tight \\cite{shor95}\\cite[Thm 4]{knuth84}. As shown in \\R\\cite{knuth84,lamport94} \
           (see, for instance, \\R[I]\\cite{lamport94,shor95})."],
        &Overrides::none(),
    );
    let written = document(&run);
    assert!(written.contains("\\flmxSpan[style=superscript, role=flm.citations.mark"), "{written}");
    assert_golden("citations-superscript.tex", &run);
}

#[test]
fn in_place_the_mark_holds_the_entries_and_no_list_is_placed() {
    let config = CitationsConfig {
        placement: CitationPlacement::InPlace,
        ..CitationsConfig::new()
    };
    let run = render(
        &registry(config),
        &["The bound is tight \\cite{knuth84}\\cite[Thm 4]{lamport94}. As shown in \
           \\R\\cite{shor95,gottesman97}."],
        &Overrides::none(),
    );
    let written = document(&run);
    assert!(written.contains("kinds={attached in-place}"), "{written}");
    assert!(!written.contains("flm.citations.bibliography"), "{written}");
    assert_golden("citations-in-place.tex", &run);
}

#[test]
fn a_document_without_citations_holds_no_piece() {
    let run = render(&registry(CitationsConfig::new()), &["No citation here."], &Overrides::none());
    let (text, _) = style(&run);
    assert!(!text.contains(PIECE), "{text}");
}
