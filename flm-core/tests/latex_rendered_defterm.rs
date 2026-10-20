//! Term definitions on the `latex-rendered` backend, end to end over the
//! standard package: the documents `tests/stdcore_defterm_backends.rs`
//! renders (a definition with its lead-in, term links to it, a term link
//! inside its own definition and inside another link's text, a richer
//! lead-in, no lead-in), each a golden under `tests/goldens/latex-rendered/`
//! holding the document and the style file, which
//! `tests/latex_rendered_compile.rs` compiles with `lualatex`; and the
//! defterm feature's definition piece, which the feature's style identifier
//! brings into the style file where a term is used inside its own
//! definition.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::defterm::DeftermConfig;
use flm_core::stdcore::features::defterm::latex_rendered::PIECE;

use common::latex_rendered::{assert_golden, document, render, style};

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// A definition with a label and an alias, using its own term; term links to
/// it by its term, by a given term, by its alias; a reference to it; a term
/// link inside an `\href`'s text.
const DOCUMENT: &str = "\\section{Terms}

\\begin{defterm}{Entropy}\\label{topic:entropy}\\aliasterm{Entropies}
A measure of disorder: the \\term{Entropy} of a system never decreases.
\\end{defterm}

The \\term{Entropy} of a gas, \\term[Entropy]{its entropy}, all \\term{Entropies}; see
\\ref{topic:entropy} and \\href{https://example.com/}{a page on \\term{Entropy}}.
";

/// A definition of `term` using its own term.
fn definition_of(term: &str) -> String {
    format!("\\begin{{defterm}}{{{term}}}\nWhat \\term{{{term}}} means.\n\\end{{defterm}}\n")
}

#[test]
fn a_definition_and_term_links_and_the_style_file_holds_the_piece() {
    let run = render(&std(), &[DOCUMENT], &Overrides::none());
    let written = document(&run);
    assert!(written.contains("\\begin{flmxBlock}[role=flm.defterm, anchor="), "{written}");
    assert!(written.contains("\\flmxSpan[role=flm.defterm.term]{Entropy}"), "{written}");
    assert!(written.contains("\\flmxLink[kind=term"), "{written}");
    let (text, _) = style(&run);
    assert!(text.contains(&format!("% {PIECE} --")), "{text}");
    assert_golden("defterm.tex", &run);
}

#[test]
fn a_richer_lead_in_and_none() {
    let richer = common::stdcore::registry(StdcoreConfig::new().defterm(
        DeftermConfig::new().lead_in(Some(String::from("\\textbf{Definition} (\\emph{#{term}}): "))),
    ));
    let run = render(&richer, &[&definition_of("Entropy")], &Overrides::none());
    assert!(document(&run).contains("[role=flm.defterm.leadin]"), "{}", document(&run));
    assert_golden("defterm-lead-in-richer.tex", &run);
    let none = common::stdcore::registry(StdcoreConfig::new().defterm(DeftermConfig::new().lead_in(None)));
    let run = render(&none, &[&definition_of("Entropy")], &Overrides::none());
    assert!(!document(&run).contains("flm.defterm.leadin"), "{}", document(&run));
    assert_golden("defterm-lead-in-none.tex", &run);
}

#[test]
fn a_definition_without_its_own_term_holds_the_piece_and_a_document_without_one_does_not() {
    // The definition's rule requires the feature's style identifier at every
    // definition; a document with neither a definition nor a term link inside
    // its own definition requires nothing of the feature.
    let run = render(&std(), &["\\begin{defterm}{Entropy}\nA measure.\n\\end{defterm}\n"], &Overrides::none());
    assert!(style(&run).0.contains(PIECE), "{}", style(&run).0);
    let run = render(&std(), &["No definition.\n"], &Overrides::none());
    assert!(!style(&run).0.contains(PIECE), "{}", style(&run).0);
}
