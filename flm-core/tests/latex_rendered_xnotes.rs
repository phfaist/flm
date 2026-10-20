//! Notes on the `latex-rendered` backend, end to end over the standard
//! package: the documents `tests/stdcore_xnotes_html.rs` renders, under the
//! kind's two placements. A kind placed in a list (the default `footnote`)
//! writes the mark in the text and the notes as a block after the content; a
//! kind placed in place writes `\flmxXnote[<keys>]{<mark>}{<content>}`
//! through the xnotes feature's rule for this backend, which the default of
//! the feature's definition piece sets as a footnote at the bottom of the
//! page carrying FLM's mark. Each document is a golden under
//! `tests/goldens/latex-rendered/`, which `tests/latex_rendered_compile.rs`
//! compiles with `lualatex`.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::numbering::{NumberTemplate, PrefixWords};
use flm_core::stdcore::features::xnotes::latex_rendered::{PIECE, RULE};
use flm_core::stdcore::features::xnotes::{XnoteKind, XnotePlacement, XnotesConfig};

use common::latex_rendered::{assert_golden, document, render, style};

/// The standard package, its xnotes feature under `xnotes`.
fn registry(xnotes: XnotesConfig) -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().xnotes(xnotes))
}

/// The configuration whose `footnote` kind is placed in place.
fn in_place() -> XnotesConfig {
    XnotesConfig { kinds: vec![XnoteKind::footnote().placement(XnotePlacement::InPlace)] }
}

/// The worked example of the HTML tests: a note of one paragraph and a note
/// of two.
const WORKED: &str = "The claim holds in every case.\\footnote{Except the empty one, which we exclude \
    by convention.} The next sentence goes on.\n\n\
    Two paragraphs.\\footnote{First paragraph.\n\n\
    Second paragraph.} And on.";

#[test]
fn a_listed_kind_writes_the_mark_and_the_list_and_the_style_file_holds_the_piece() {
    let run = render(&registry(XnotesConfig::new()), &[WORKED], &Overrides::none());
    let written = document(&run);
    assert!(
        written.contains("case.\\flmxSpan[style=superscript, role=flm.xnotes.xnote.mark, anchor="),
        "{written}"
    );
    assert!(
        written.contains("\\flmxLink[kind=xnote, href={\\#fn-1}, target=fn-1, doc-role=noteref"),
        "{written}"
    );
    assert!(
        written.contains("\\begin{flmxBlock}[role=flm.xnotes.footnote.list, kinds={xnotes footnote}, doc-role=endnotes]"),
        "{written}"
    );
    assert!(!written.contains("\\flmxXnote"), "{written}");
    let (text, _) = style(&run);
    assert!(text.contains(&format!("% {PIECE} --")), "{text}");
    assert_golden("xnotes-list.tex", &run);
}

#[test]
fn an_in_place_kind_writes_the_note_command_with_the_mark_and_the_content_apart() {
    let run = render(
        &registry(in_place()),
        &[&format!(
            "{WORKED}\n\nCode in a note.\\footnote{{The code:\n\
             \\begin{{verbatimcode}}\nlet x = 1;  // {{x}}\n\\end{{verbatimcode}}\nAnd after.}} Done."
        )],
        &Overrides::none(),
    );
    let written = document(&run);
    // The keys, the mark unlinked, then the content; no bracket.
    assert!(
        written.contains("case.\\flmxXnote[kind=footnote, anchor=fn-1, doc-role=footnote]{\\flmxSpan[style=superscript, \
                          role=flm.xnotes.xnote.mark, anchor=fnref--"),
        "{written}"
    );
    assert!(written.contains("]{1}}{Except"), "{written}");
    assert!(written.contains("by convention.} The next"), "{written}");
    assert!(!written.contains("\\flmxLink[kind=xnote"), "{written}");
    assert!(!written.contains("{[") && !written.contains("]}"), "{written}");
    // A note of two paragraphs holds them as paragraphs.
    assert!(written.contains("{\n\nFirst paragraph.\n\nSecond paragraph.}"), "{written}");
    assert!(written.contains("\\begin{flmxVerbatimBlock}"), "{written}");
    // Nothing is placed after the content.
    assert!(!written.contains("flm.xnotes.footnote.list"), "{written}");
    let (text, _) = style(&run);
    assert!(text.contains(&format!("% {PIECE} --")), "{text}");
    assert_golden("xnotes-in-place.tex", &run);
}

#[test]
fn an_in_place_kind_with_a_fixed_mark_writes_that_mark() {
    let aside = XnoteKind::unnumbered("aside", PrefixWords::new("Aside~", "Asides~"))
        .commands(["aside"])
        .label_type("as")
        .mark(NumberTemplate::formats("*"))
        .placement(XnotePlacement::InPlace);
    let run = render(
        &registry(XnotesConfig::new().kind(aside)),
        &["An aside stands here.\\aside{An aside.} A footnote too.\\footnote{Listed.}"],
        &Overrides::none(),
    );
    let written = document(&run);
    assert!(written.contains("\\flmxXnote[kind=aside, anchor="), "{written}");
    assert!(written.contains("]{*}}{An"), "{written}");
    // The listed kind beside it keeps its mark and its list.
    assert!(written.contains("flm.xnotes.footnote.list"), "{written}");
    assert_golden("xnotes-fixed-mark.tex", &run);
}

#[test]
fn a_document_without_notes_holds_no_piece_and_the_rule_is_registered() {
    let registry = registry(in_place());
    let run = render(&registry, &["No note here."], &Overrides::none());
    let (text, _) = style(&run);
    assert!(!text.contains(PIECE), "{text}");
    assert!(registry.rules().iter().any(|entry| entry.rule.name.as_str() == RULE));
}
