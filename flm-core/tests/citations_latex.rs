//! The `citations` feature on the recomposed LaTeX: a `\cite` chunk written
//! as one space hook and one `\flmCite` per call, the keys raw as typed and
//! the note in the optional argument; an `\R` chunk as `\flmRefCite` over the
//! style's word, with no space hook; the definition piece's three hooks and
//! the case-change exclusion line; the configuration's placement and form
//! changing nothing; a placeholder among the keys of a user definition
//! expanding its use sites; the mistakes reported. The goldens this file
//! writes (`citations-*.tex`) are compiled by `tests/latex_compile.rs`, each
//! by a test of its own that appends a `thebibliography` environment.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::stdcore::backends::latex_recomposed;
use flm_core::stdcore::features::citations::latex::{
    definition_piece, CITE_COMMAND, PIECE, PRE_SPACE_COMMAND, REF_CITE_COMMAND,
};
use flm_core::stdcore::features::citations::{CitationPlacement, CitationsConfig, MarkForm};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::StdcoreConfig;

use common::stdcore::{identifiers, run_workflow};

// --- helpers ---------------------------------------------------------------------------------

/// The user definitions of these tests: `\arxiv{k}` for `\cite{arxiv:#1}`, a
/// placeholder among the keys; `\seethm{n}` for `\cite[see #1]{knuth_84}`, a
/// placeholder in the note alone; `\knuth` for `\cite{knuth_84}`, none.
fn userdefs() -> UserdefsConfig {
    UserdefsConfig::new()
        .definition(Definition::macro_("arxiv", vec![ArgDef::mandatory("key")]).content("\\cite{arxiv:#1}"))
        .definition(Definition::macro_("seethm", vec![ArgDef::mandatory("note")]).content("\\cite[see #1]{knuth_84}"))
        .definition(Definition::macro_("knuth", vec![]).content("\\cite{knuth_84}"))
}

/// The standard package under `config`, with [`userdefs`], every key
/// answered by the test provider.
fn registry_of(config: CitationsConfig) -> Arc<FlmRegistry> {
    common::citations::registry(StdcoreConfig::new().citations(config).userdefs(userdefs()))
}

/// The recomposition of `document` under `registry`.
fn recompose_on(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, latex_recomposed::WORKFLOW, &[document], &Overrides::none())
}

/// The recomposition of `document` under the default configuration; the run
/// must report nothing.
fn recompose(document: &str) -> Run {
    let run = recompose_on(&registry_of(CitationsConfig::new()), document);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    run
}

/// The recomposed LaTeX of `document`, without the line naming the style
/// file and without the `%%` line ending the backend writes last.
fn written(document: &str) -> String {
    let output = common::latex::output(&recompose(document));
    output.strip_suffix("%%\n").unwrap_or(&output).to_string()
}

// --- the rules (test 17) ------------------------------------------------------------------------

#[test]
fn a_chunk_is_one_space_hook_then_one_hook_per_call_each_with_its_own_keys_and_note() {
    assert_eq!(
        written("text\\cite[Thm 4]{a,b}\\cite{c}"),
        "text\\flmCitePreSpace\\flmCite[Thm 4]{a,b}\\flmCite{c}"
    );
    assert_eq!(written("text\\cite{a,b,c}"), "text\\flmCitePreSpace\\flmCite{a,b,c}");
    // Nothing is sorted, merged or compressed: each call as written.
    assert_eq!(
        written("text\\cite{c}\\cite[Thm 4]{a}\\cite{b} \\cite[Thm 4]{d}"),
        "text\\flmCitePreSpace\\flmCite{c}\\flmCite[Thm 4]{a}\\flmCite{b}\\flmCite[Thm 4]{d}"
    );
}

#[test]
fn a_note_holding_a_command_is_braced_inside_the_brackets() {
    assert_eq!(
        written("text\\cite[\\emph{Thm} 4]{a}"),
        "text\\flmCitePreSpace\\flmCite[{\\emph{Thm} 4}]{a}"
    );
}

#[test]
fn a_key_holding_braces_is_written_as_read() {
    assert_eq!(
        written("text\\cite{manual:{X et al., Journal of Future Results (2034)}}"),
        "text\\flmCitePreSpace\\flmCite{manual:{X et al., Journal of Future Results (2034)}}"
    );
}

#[test]
fn the_keys_are_written_raw_as_typed_and_trimmed() {
    assert_eq!(
        written("text\\cite{ arxiv:1234_5 ,knuth_84,, }"),
        "text\\flmCitePreSpace\\flmCite{arxiv:1234_5,knuth_84}"
    );
}

#[test]
fn the_authors_whitespace_before_a_cite_stands_and_the_hook_removes_it() {
    assert_eq!(
        written("text\\cite{a} and text \\cite{a}"),
        "text\\flmCitePreSpace\\flmCite{a} and text \\flmCitePreSpace\\flmCite{a}"
    );
}

#[test]
fn an_r_chunk_is_the_word_and_its_calls_with_no_space_hook() {
    assert_eq!(written("See \\R\\cite{a,b,c}."), "See \\flmRefCite{refs.~}{\\flmCite{a,b,c}}.");
    assert_eq!(written("\\R[S]\\cite{a,b,c}"), "\\flmRefCite{Refs.~}{\\flmCite{a,b,c}}");
    assert_eq!(written("\\R[S]\\cite{a}\\cite{b}"), "\\flmRefCite{Refs.~}{\\flmCite{a}\\flmCite{b}}");
    assert_eq!(written("\\R\\cite{a}"), "\\flmRefCite{ref.~}{\\flmCite{a}}");
    // The plural counts every item, a note's included.
    assert_eq!(written("\\R[S]\\cite[Thm 4]{a}"), "\\flmRefCite{Ref.~}{\\flmCite[Thm 4]{a}}");
    // Under the flag `I`, the word is empty and the hook is written all the same.
    assert_eq!(written("(\\R[I]\\cite{a,b})"), "(\\flmRefCite{}{\\flmCite{a,b}})");
    assert!(!written("\\R\\cite{a} \\R[I]\\cite{b}").contains(PRE_SPACE_COMMAND));
}

#[test]
fn the_placement_and_the_form_change_nothing() {
    let document = "text\\cite[Thm 4]{a,b}\\cite{c} and \\R[S]\\cite{a}\\cite{d}.";
    let default = common::latex::golden_text(&recompose(document));
    for config in [
        CitationsConfig::new().placement(CitationPlacement::InPlace),
        CitationsConfig::new().mark(MarkForm::Superscript),
        CitationsConfig::new().placement(CitationPlacement::InPlace).mark(MarkForm::Superscript),
    ] {
        let run = recompose_on(&registry_of(config.clone()), document);
        assert_eq!(identifiers(&run), Vec::<String>::new(), "{config:?}");
        assert_eq!(common::latex::golden_text(&run), default, "{config:?}");
    }
}

#[test]
fn no_bibliography_is_written() {
    // The document is the citation alone: the list of the cited works is
    // LaTeX's own, and the style file holds the piece and nothing else.
    let run = recompose("text\\cite{a}");
    assert_eq!(common::latex::output(&run), "text\\flmCitePreSpace\\flmCite{a}%%\n");
    let (style, _) = common::latex::style(&run);
    assert!(!style.contains("References"), "{style}");
}

// --- the definition piece ------------------------------------------------------------------------

#[test]
fn the_style_file_holds_the_three_hooks_and_the_exclusion_line_and_requests_no_package() {
    let (style, _) = common::latex::style(&recompose("text\\cite{a}"));
    let piece = definition_piece();
    assert!(style.contains(piece.content.as_str()), "{style}");
    assert!(!style.contains("\\RequirePackage"), "{style}");
    for line in [
        "\\ProvideDocumentCommand\\flmCite{o m}{\\IfValueTF{#1}{\\cite[{#1}]{#2}}{\\cite{#2}}}\n",
        "\\ProvideDocumentCommand\\flmCitePreSpace{}{\\unskip~}\n",
        "\\ProvideDocumentCommand\\flmRefCite{m m}{#1#2}\n",
        "\\tl_if_in:NnF \\l_text_case_exclude_arg_tl { \\flmCite } \
         { \\tl_put_right:Nn \\l_text_case_exclude_arg_tl { \\flmCite } }\n",
    ] {
        assert!(style.contains(line), "{line}\n{style}");
    }
    assert_eq!(PIECE, "flm.latex.def.citations");
    assert_eq!([CITE_COMMAND, PRE_SPACE_COMMAND, REF_CITE_COMMAND], ["flmCite", "flmCitePreSpace", "flmRefCite"]);
}

#[test]
fn a_document_citing_nothing_needs_no_piece() {
    assert!(common::latex::style_if_any(&recompose("No citation here.")).is_none());
}

// --- user definitions (test 14, the LaTeX half) -------------------------------------------------

#[test]
fn a_placeholder_among_the_keys_expands_the_use_sites() {
    let run = recompose("A\\arxiv{1234} and B\\arxiv{99_1}.");
    let output = common::latex::output(&run);
    assert!(output.starts_with("A\\flmCitePreSpace\\flmCite{arxiv:1234} and B\\flmCitePreSpace\\flmCite{arxiv:99_1}."), "{output}");
    let (style, _) = common::latex::style(&run);
    assert!(!style.contains("flmUDarxiv"), "{style}");
}

#[test]
fn a_definition_citing_fixed_keys_or_with_a_placeholder_in_the_note_is_kept_and_its_citations_are_latexs() {
    let run = recompose("A\\knuth, B\\seethm{Thm 4}.");
    let output = common::latex::output(&run);
    assert!(output.starts_with("A\\flmUDknuth, B\\flmUDseethm{Thm 4}."), "{output}");
    let (style, _) = common::latex::style(&run);
    assert!(style.contains("\\providecommand\\flmUDknuth{\\flmCitePreSpace\\flmCite{knuth_84}}"), "{style}");
    assert!(
        style.contains("\\providecommand\\flmUDseethm[1]{\\flmCitePreSpace\\flmCite[see #1]{knuth_84}}"),
        "{style}"
    );
    assert!(style.contains(definition_piece().content.as_str()), "{style}");
}

// --- mistakes ------------------------------------------------------------------------------------

#[test]
fn a_mistaken_call_is_reported_and_writes_no_hook_and_a_chunk_writing_none_is_a_dropped_mark() {
    let run = recompose_on(&registry_of(CitationsConfig::new()), "A\\cite{a}\\cite{} B\\cite{} C\\R D\\R[X]\\cite{a}.");
    assert_eq!(
        identifiers(&run),
        ["flm.citations.no-key", "flm.citations.no-key", "flm.citations.no-citation", "flm.citations.unknown-flag"]
    );
    let output = common::latex::output(&run);
    assert!(output.starts_with("A\\flmCitePreSpace\\flmCite{a} B\n%"), "{output}");
    assert_eq!(output.matches("\\flmCitePreSpace").count(), 1, "{output}");
    assert!(output.contains("D\\flmRefCite{ref.~}{\\flmCite{a}}."), "{output}");
}

#[test]
fn a_cited_key_with_no_entry_is_an_error_here_too() {
    let registry = common::citations::registry_with(StdcoreConfig::new(), Vec::new());
    let run = recompose_on(&registry, "text\\cite{a}");
    assert!(identifiers(&run).contains(&"flm.citations.no-entry".to_string()), "{:?}", identifiers(&run));
    assert!(common::latex::output(&run).starts_with("text\\flmCitePreSpace\\flmCite{a}"));
}

// --- the goldens compiled by `tests/latex_compile.rs` -------------------------------------------

/// The chunk golden: notes on several keys, a chunk of calls, keys holding
/// `_` and `:`, a `\cite` written with and without a space before it, and a
/// kept definition citing a fixed key.
const CHUNKS: &str = "\
A chunk with notes\\cite[Thm 4]{a,b}\\cite{c}.

Written with no space: text\\cite{a}. Written with one: text \\cite{a}.

Keys as typed\\cite{arxiv:1234_5, knuth_84}; through a definition\\knuth.
";

/// The golden of the `\R` forms.
const REFCITES: &str = "\
See \\R\\cite{a,b,c}, \\R[S]\\cite{a}\\cite{b}, \\R\\cite{a}, (\\R[I]\\cite{a,b}) and \\R[S]\\cite[Thm 4]{a}.
";

/// The golden of a citation in a heading's title.
const HEADING: &str = "\
\\section{Results\\cite[Thm 4]{knuth_84}}

Text.
";

#[test]
fn the_golden_of_a_chunk() {
    common::latex::assert_golden("citations-chunks.tex", &recompose(CHUNKS));
}

#[test]
fn the_golden_of_the_r_forms() {
    common::latex::assert_golden("citations-refcite.tex", &recompose(REFCITES));
}

#[test]
fn the_golden_of_a_citation_in_a_heading() {
    common::latex::assert_golden("citations-heading.tex", &recompose(HEADING));
}
