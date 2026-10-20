//! The `citations` feature's entries on the text output: the providers, the
//! retrieve stage and its table, the check, the list of the cited works and
//! the in-place placement.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::lang::text::plain_text;
use flm_core::core::piece::Piece;
use flm_core::stdcore::backends::pieces;
use flm_core::stdcore::backends::pres::{
    self, payload_of, BlockPayload, DroppedPayload, HeadingPayload, LinkPayload, ListPayload, SpanPayload,
};
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::citations::{
    bibliography_table, CitationAnswer, CitationEntry, CitationPlacement, CitationProvider,
    CitationRequest, CitationResult, CitationsConfig, MarkForm, MissingEntry, ProviderCx, Retrieval, ENTRIES, ENTRY_NUMBER_ROLE, IN_PLACE, MARK_ROLE,
};
use flm_core::stdcore::{StdcoreConfig, CHECK_WORKFLOW};

use common::citations::{registry_with, Asked, Counting, EveryKey, Failing, FromMap};
use common::lowering::text as printed;
use common::stdcore::{identifiers, run_workflow};

// --- helpers ---------------------------------------------------------------------------------

/// The standard package, the default configuration, with `providers`.
fn with(providers: Vec<Arc<dyn CitationProvider>>) -> Arc<FlmRegistry> {
    registry_with(StdcoreConfig::new(), providers)
}

/// The standard package under `config`, with `providers`.
fn with_config(config: CitationsConfig, providers: Vec<Arc<dyn CitationProvider>>) -> Arc<FlmRegistry> {
    registry_with(StdcoreConfig::new().citations(config), providers)
}

/// Renders one document to text under `registry`.
fn render(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, text::WORKFLOW, &[document], &Overrides::none())
}

/// The text a run wrote, its trailing whitespace trimmed.
fn text_of(run: &Run) -> String {
    printed(run).trim_end().to_string()
}

/// The entries table of a run, each key with its entry's text or its
/// reason's name.
fn entries(run: &Run) -> Vec<(String, Result<String, MissingEntry>)> {
    let table = run.table(&ENTRIES).expect("declared").expect("written");
    table
        .iter()
        .map(|(key, found)| {
            let found = match found {
                Retrieval::Found(answer) => Ok(plain_text(&answer.entry.content)),
                Retrieval::Missing(missing) => Err(missing.clone()),
            };
            (key.to_string(), found)
        })
        .collect()
}

/// The provider of the entries `a` (`Knuth, 1984`) and `b` (`Lamport,
/// 1994`).
fn knuth_lamport() -> Arc<dyn CitationProvider> {
    Arc::new(FromMap::of(&[("a", "Knuth, 1984"), ("b", "Lamport, 1994")]))
}

/// The bibliography piece a run wrote.
fn bibliography(run: &Run) -> Piece {
    run.table(&bibliography_table()).expect("declared").expect("written").clone()
}

/// The nodes of the lowered bibliography the text backend placed, of the
/// presentation element `element`, with their payloads of type `P`.
fn placed<P: flm_core::lang::PresPayload + Clone>(run: &Run, element: &str) -> Vec<P> {
    let pieces = run.table(&pieces::lowered_pieces_table(&text::BACKEND.into())).expect("declared").expect("written");
    let piece = pieces.pieces.iter().find(|piece| piece.name.as_str() == "flm.citations.bibliography").expect("placed");
    piece
        .tree
        .tree
        .root()
        .descendants()
        .filter(|node| node.ext().element.name.as_str() == element)
        .filter_map(|node| payload_of::<P>(&node).cloned())
        .collect()
}

/// Every span of the lowered first tree of a run.
fn spans(run: &Run) -> Vec<SpanPayload> {
    common::lowering::lowered(run, flm_core::model::addr::TreeId::tree(0))
        .tree
        .root()
        .descendants()
        .filter_map(|node| payload_of::<SpanPayload>(&node).cloned())
        .collect()
}

/// Every link of the lowered first tree of a run.
fn links(run: &Run) -> Vec<LinkPayload> {
    common::lowering::lowered(run, flm_core::model::addr::TreeId::tree(0))
        .tree
        .root()
        .descendants()
        .filter_map(|node| payload_of::<LinkPayload>(&node).cloned())
        .collect()
}

/// A counting provider around `inner`, and what it records.
fn counting(inner: Arc<dyn CitationProvider>) -> (Arc<dyn CitationProvider>, Asked) {
    let asked = Asked::default();
    (Arc::new(Counting { asked: asked.clone(), inner }), asked)
}

// --- test 8: the providers -------------------------------------------------------------------

#[test]
fn a_provider_answering_from_a_map_gives_each_cited_key_its_entry_once() {
    let (provider, asked) = counting(Arc::new(FromMap::of(&[("a", "Knuth, 1984"), ("b", "Lamport, 1994")])));
    let run = render(&with(vec![provider]), "X \\cite{b,a}. Y \\cite{a}\\cite{b}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(
        entries(&run),
        [("b".to_string(), Ok("Lamport, 1994".to_string())), ("a".to_string(), Ok("Knuth, 1984".to_string()))]
    );
    assert_eq!(asked.keys(), ["b", "a"], "once per distinct key, in order of first appearance");
}

#[test]
fn the_second_provider_answers_what_the_first_declines() {
    let (first, first_asked) = counting(Arc::new(FromMap::of(&[("a", "First A")])));
    let (second, second_asked) = counting(Arc::new(FromMap::of(&[("a", "Second A"), ("b", "Second B")])));
    let run = render(&with(vec![first, second]), "X \\cite{a,b}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(
        entries(&run),
        [("a".to_string(), Ok("First A".to_string())), ("b".to_string(), Ok("Second B".to_string()))]
    );
    assert_eq!(first_asked.keys(), ["a", "b"]);
    assert_eq!(second_asked.keys(), ["b"]);
}

#[test]
fn a_provider_s_error_leaves_the_key_missing_and_the_next_provider_unasked() {
    let (failing, _) = counting(Arc::new(Failing(vec!["a".to_string()])));
    let (next, next_asked) = counting(Arc::new(FromMap::of(&[("a", "A"), ("b", "B")])));
    let run = render(&with(vec![failing, next]), "X \\cite{a,b}.");
    assert_eq!(
        identifiers(&run),
        ["flm.citations.provider-failed", "flm.citations.no-entry"],
        "{}",
        run.diagnostics().render_all()
    );
    let rendered = run.diagnostics().render_all();
    assert!(rendered.contains("the bibliography entry of the key `a` could not be retrieved: offline"), "{rendered}");
    assert!(rendered.contains("the cited key `a` must have a bibliography entry; retrieving it failed: offline"), "{rendered}");
    assert_eq!(next_asked.keys(), ["b"]);
    let found = entries(&run);
    assert!(matches!(&found[0].1, Err(MissingEntry::Failed(error)) if error.detail == "offline"));
    assert_eq!(found[1].1, Ok("B".to_string()));
}

#[test]
fn no_provider_is_an_error_and_every_key_is_then_missing() {
    let run = render(&with(Vec::new()), "X \\cite{a,b}. Y \\cite{a}.");
    assert_eq!(
        identifiers(&run),
        ["flm.citations.no-provider", "flm.citations.no-entry", "flm.citations.no-entry"],
        "{}",
        run.diagnostics().render_all()
    );
    assert!(entries(&run).iter().all(|(_, found)| *found == Err(MissingEntry::NoProvider)));
    // A document citing nothing reports nothing.
    let run = render(&with(Vec::new()), "Nothing cited.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
}

#[test]
fn a_provider_parses_flm_source_into_an_entry_through_the_request_context() {
    let source = "Knuth, \\emph{The Art of Computer Programming}, \\href{https://example.org}{online}";
    let run = render(&with(vec![Arc::new(FromMap::of(&[("knuth", source)]))]), "X \\cite{knuth}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let table = run.table(&ENTRIES).expect("declared").expect("written");
    let entry = table.entry("knuth").expect("an entry");
    let names: Vec<String> =
        entry.content.root().descendants().filter_map(|node| node.name().map(String::from)).collect();
    assert_eq!(names, ["emph", "href"]);
    assert_eq!(plain_text(&entry.content), "Knuth, The Art of Computer Programming, online");
}

#[test]
fn an_entry_that_does_not_parse_is_the_provider_s_error() {
    let run = render(&with(vec![Arc::new(FromMap::of(&[("a", "\\nosuchcommand")]))]), "X \\cite{a}.");
    assert_eq!(
        identifiers(&run),
        ["flm.citations.provider-failed", "flm.citations.no-entry"],
        "{}",
        run.diagnostics().render_all()
    );
}

// --- test 9: a missing entry -----------------------------------------------------------------

#[test]
fn a_missing_entry_is_an_error_the_key_numbered_and_its_mark_unchanged() {
    let run = render(&with(vec![Arc::new(FromMap::of(&[("a", "A")]))]), "X \\cite{a,b}. Y \\cite{b}.");
    assert_eq!(identifiers(&run), ["flm.citations.no-entry"], "{}", run.diagnostics().render_all());
    let rendered = run.diagnostics().render_all();
    assert!(
        rendered.contains("the cited key `b` must have a bibliography entry; no source of bibliography entries knows it"),
        "{rendered}"
    );
    let text = text_of(&run);
    assert!(text.starts_with("X\u{a0}[1,2]. Y\u{a0}[2]."), "{text}");
}

// --- test 10: the check workflow asks the providers ------------------------------------------

#[test]
fn the_check_workflow_asks_the_providers_and_reports_a_missing_entry_as_a_render_does() {
    let (provider, asked) = counting(Arc::new(FromMap::of(&[("a", "A")])));
    let registry = with(vec![provider]);
    let document = "X \\cite{a,b}.";
    let checked = run_workflow(&registry, CHECK_WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&checked), ["flm.citations.no-entry"], "{}", checked.diagnostics().render_all());
    assert_eq!(asked.keys(), ["a", "b"]);
    let rendered = render(&registry, document);
    assert_eq!(identifiers(&rendered), identifiers(&checked));
    assert_eq!(asked.keys(), ["a", "b", "a", "b"], "every run asks again");
}

#[test]
fn the_test_provider_answering_every_key_gives_each_key_its_own_text() {
    let run = render(&with(vec![Arc::new(EveryKey)]), "X \\cite{knuth84}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(entries(&run), [("knuth84".to_string(), Ok("knuth84".to_string()))]);
}


// --- test 11: the bibliography ---------------------------------------------------------------

#[test]
fn the_bibliography_lists_every_cited_work_in_citation_order_after_the_footnotes() {
    let registry = with(vec![Arc::new(FromMap::of(&[
        ("a", "Knuth, \\emph{TAOCP}, \\(x^2\\), \\href{https://e.org}{online}"),
        ("b", "Lamport, 1994"),
        ("c", "Shor, 1995"),
    ]))]);
    let run = render(&registry, "X \\cite{b,a}. Y\\footnote{Z \\cite{c}.} W \\cite{a}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(
        text_of(&run),
        "X\u{a0}[1,2]. Y\u{b9} W\u{a0}[2].\n\nFootnotes\n---------\n\n\u{b9} Z\u{a0}[3].\n\n\
         References\n----------\n\n[1] Lamport, 1994\n\n[2] Knuth, TAOCP, \\(x^2\\), online <https://e.org>\n\n\
         [3] Shor, 1995"
    );
}

#[test]
fn the_bibliography_is_a_block_holding_a_heading_and_a_labeled_list_identifying_each_entry() {
    let run = render(&with(vec![knuth_lamport()]), "X \\cite{b,a}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let blocks: Vec<BlockPayload> = placed(&run, pres::BLOCK);
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].role.as_ref().map(|role| role.as_str()), Some("flm.citations.bibliography"));
    assert_eq!(blocks[0].kinds.iter().map(|kind| kind.as_str()).collect::<Vec<_>>(), ["citations"]);
    let headings: Vec<HeadingPayload> = placed(&run, pres::BLOCK_HEADING);
    assert_eq!((headings[0].name.as_str(), headings[0].level, headings[0].in_toc), ("citations", 2, false));
    let lists: Vec<ListPayload> = placed(&run, pres::LIST);
    assert_eq!(
        lists[0].kinds.iter().map(|kind| kind.as_str()).collect::<Vec<_>>(),
        ["numbered", "citations", "labeled"]
    );
    assert_eq!(lists[0].entry_anchors, [Some("citation-1".to_string()), Some("citation-2".to_string())]);
    let labels: Vec<SpanPayload> = placed(&run, pres::SPAN);
    assert_eq!(labels.len(), 2);
    assert!(labels.iter().all(|label| label.role.as_ref().is_some_and(|role| role.as_str() == ENTRY_NUMBER_ROLE)));
}

#[test]
fn the_title_and_its_level_are_configured_and_the_superscript_form_labels_with_bare_numbers() {
    let config =
        CitationsConfig::new().heading_title("Bibliography").heading_level(1).mark(MarkForm::Superscript);
    let run = render(&with_config(config, vec![knuth_lamport()]), "X \\cite{a,b}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let text = text_of(&run);
    assert!(text.ends_with("Bibliography\n============\n\n1 Knuth, 1984\n\n2 Lamport, 1994"), "{text}");
}

#[test]
fn a_list_configured_with_no_heading_is_the_list_alone() {
    let config = CitationsConfig::new().without_heading();
    let run = render(&with_config(config, vec![knuth_lamport()]), "X \\cite{a,b}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let headings: Vec<HeadingPayload> = placed(&run, pres::BLOCK_HEADING);
    assert!(headings.is_empty(), "{headings:?}");
    let blocks: Vec<BlockPayload> = placed(&run, pres::BLOCK);
    assert_eq!(blocks[0].role.as_ref().map(|role| role.as_str()), Some("flm.citations.bibliography"));
    let lists: Vec<ListPayload> = placed(&run, pres::LIST);
    assert_eq!(lists[0].entry_anchors, [Some("citation-1".to_string()), Some("citation-2".to_string())]);
    let text = printed(&run);
    assert!(text.ends_with("[1] Knuth, 1984\n\n[2] Lamport, 1994\n"), "{text:?}");
    assert!(!text.contains("References"), "{text:?}");
}

#[test]
fn no_citation_places_no_bibliography() {
    let run = render(&with(vec![knuth_lamport()]), "Nothing cited.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert!(bibliography(&run).content.is_none());
    assert_eq!(text_of(&run), "Nothing cited.");
}

#[test]
fn an_entry_stating_a_document_fact_is_refused_with_the_pieces_error() {
    let provider = Arc::new(FromMap::of(&[("a", "Knuth\\footnote[\\label{fn:knuth}]{A note.}")]));
    let run = render(&with(vec![provider]), "X \\cite{a}.");
    assert_eq!(identifiers(&run), ["flm.pieces.lowering-failed"], "{}", run.diagnostics().render_all());
    assert_eq!(text_of(&run), "X\u{a0}[1].");
}

// --- tests 3, 9 and 13: what the list shows --------------------------------------------------

#[test]
fn a_key_first_cited_inside_a_footnote_is_listed_at_its_number_with_its_marks_in_the_list() {
    let run = render(&with(vec![knuth_lamport()]), "A\\footnote{B \\cite{a}.} C \\cite{b,a}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let text = text_of(&run);
    assert!(text.starts_with("A\u{b9} C\u{a0}[1,2]."), "{text}");
    assert!(text.contains("\u{b9} B\u{a0}[1]."), "{text}");
    assert!(text.ends_with("[1] Knuth, 1984\n\n[2] Lamport, 1994"), "{text}");
}

#[test]
fn the_bibliography_line_of_a_missing_entry_shows_that_it_is_missing() {
    let run = render(&with(vec![Arc::new(FromMap::of(&[("a", "Knuth, 1984")]))]), "X \\cite{a,b}.");
    assert_eq!(identifiers(&run), ["flm.citations.no-entry"], "{}", run.diagnostics().render_all());
    let text = text_of(&run);
    assert!(text.ends_with("[1] Knuth, 1984\n\n[2] [dropped: bibliography entry]"), "{text}");
    let dropped: Vec<DroppedPayload> = placed(&run, pres::DROPPED);
    assert_eq!(dropped[0].condition.as_str(), "flm.citations.no-entry");
}

#[test]
fn a_citation_inside_a_provider_s_entry_is_an_error_and_the_entry_is_refused() {
    let provider = Arc::new(FromMap::of(&[("a", "Knuth, see \\cite{b}"), ("b", "Lamport, 1994")]));
    let run = render(&with(vec![provider]), "X \\cite{a}.");
    assert_eq!(
        identifiers(&run),
        ["flm.citations.entry-cites", "flm.citations.no-entry"],
        "{}",
        run.diagnostics().render_all()
    );
    assert!(run.diagnostics().render_all().contains("the bibliography entry of the key `a` may not cite other works"));
    assert!(text_of(&run).ends_with("[1] [dropped: bibliography entry]"), "{}", text_of(&run));
    // The same entry under the in-place placement: refused, no loop.
    let provider = Arc::new(FromMap::of(&[("a", "Knuth, see \\cite{a}")]));
    let config = CitationsConfig::new().placement(CitationPlacement::InPlace);
    let run = render(&with_config(config, vec![provider]), "X \\cite{a}.");
    assert_eq!(identifiers(&run), ["flm.citations.entry-cites", "flm.citations.no-entry"]);
    assert_eq!(text_of(&run), "X\u{a0}[[dropped: bibliography entry]].");
}

// --- test 18: without the pieces feature -----------------------------------------------------

#[test]
fn a_registry_holding_citations_without_pieces_builds() {
    let features = common::stdcore::package_features_but(StdcoreConfig::new(), "pieces");
    let registry = common::citations::registry_over(features);
    assert!(registry.has_feature("citations"));
    assert!(!registry.has_feature("pieces"));
}

// --- test 12, and the in-place clauses of tests 5 and 19 -------------------------------------

/// The text of `document` under the in-place placement and `mark`, with the
/// entries of `a` and `b`, and the run.
fn in_place(mark: MarkForm, document: &str) -> (String, Run) {
    let config = CitationsConfig::new().placement(CitationPlacement::InPlace).mark(mark);
    let run = render(&with_config(config, vec![knuth_lamport()]), document);
    (text_of(&run), run)
}

#[test]
fn in_place_a_mark_shows_the_entries_one_group_per_item_with_no_link_and_no_list() {
    let (text, run) = in_place(MarkForm::Bracketed, "X \\cite{a}\\cite[Thm 4]{b}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(text, "X\u{a0}[Knuth, 1984][Lamport, 1994; Thm 4].");
    assert!(links(&run).is_empty());
    assert!(bibliography(&run).content.is_none());
    let marks: Vec<SpanPayload> =
        spans(&run).into_iter().filter(|span| span.role.as_ref().is_some_and(|role| role.as_str() == MARK_ROLE)).collect();
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].kinds.iter().map(|kind| kind.as_str()).collect::<Vec<_>>(), ["attached", IN_PLACE]);
    assert_eq!(marks[0].style, None);
}

#[test]
fn in_place_the_items_keep_their_order_and_the_word_of_r_precedes_the_entries() {
    let (text, run) = in_place(MarkForm::Bracketed, "X \\cite{b,a}. See \\R\\cite{a,b}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(text, "X\u{a0}[Lamport, 1994][Knuth, 1984]. See refs.\u{a0}[Knuth, 1984][Lamport, 1994].");
}

#[test]
fn in_place_a_missing_entry_is_shown_missing_inside_its_brackets() {
    let (text, run) = in_place(MarkForm::Bracketed, "X \\cite{a,z}.");
    assert_eq!(identifiers(&run), ["flm.citations.no-entry"], "{}", run.diagnostics().render_all());
    assert_eq!(text, "X\u{a0}[Knuth, 1984][[dropped: bibliography entry]].");
}

#[test]
fn in_place_the_superscript_form_changes_nothing_and_a_cite_writes_its_no_break_space() {
    let document = "X \\cite{a}\\cite[Thm 4]{b}. Y\\cite{b}.";
    let (bracketed, _) = in_place(MarkForm::Bracketed, document);
    let (superscript, run) = in_place(MarkForm::Superscript, document);
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(superscript, bracketed);
    assert!(superscript.contains("Y\u{a0}[Lamport, 1994]."), "{superscript}");
}

#[test]
fn the_digest_changes_with_the_placement_and_the_title() {
    let digest = |config: CitationsConfig| with_config(config, Vec::new()).digest();
    let plain = digest(CitationsConfig::new());
    assert_ne!(plain, digest(CitationsConfig::new().placement(CitationPlacement::InPlace)));
    assert_ne!(plain, digest(CitationsConfig::new().heading_title("Bibliography")));
    let without = digest(CitationsConfig::new().without_heading());
    assert_ne!(plain, without);
    // No heading is not an empty title.
    assert_ne!(without, digest(CitationsConfig::new().heading_title("")));
}

#[test]
fn a_title_that_does_not_compile_fails_the_build_naming_it() {
    let mut builder = flm_core::core::registry::RegistryBuilder::new();
    builder.use_package(flm_core::stdcore::package(
        StdcoreConfig::new().citations(CitationsConfig::new().heading_title("}")),
    ));
    builder.use_package(flm_core::math_passthrough::package());
    let message = match builder.build(&flm_core::core::registry::RegistryInputs::new()) {
        Ok(_) => panic!("the registry build was expected to fail"),
        Err(error) => error.render(),
    };
    assert!(message.contains("the option `heading_title` of the feature `citations`"), "{message}");
}

#[test]
fn in_place_an_entry_stating_a_document_fact_fails_the_chunk_s_rule() {
    let provider = Arc::new(FromMap::of(&[("a", "Knuth\\footnote[\\label{fn:knuth}]{A note.}")]));
    let config = CitationsConfig::new().placement(CitationPlacement::InPlace);
    let run = render(&with_config(config, vec![provider]), "X \\cite{a}. Y.");
    assert_eq!(identifiers(&run), ["flm.lowering.rule-failed"], "{}", run.diagnostics().render_all());
    assert!(text_of(&run).ends_with("Y."), "{}", text_of(&run));
}

/// The standard package with the user definition `\mycite{key}`, standing
/// for `\cite{key}`, under `citations`, with `provider`.
fn with_mycite(citations: CitationsConfig, provider: Arc<dyn CitationProvider>) -> Arc<FlmRegistry> {
    use flm_core::lang::mint::ArgDef;
    use flm_core::lang::rewrites::userdefs::Definition;
    use flm_core::stdcore::features::userdefs::UserdefsConfig;
    let definition = Definition::macro_("mycite", vec![ArgDef::mandatory("key")]).content("\\cite{#1}");
    let config = StdcoreConfig::new().userdefs(UserdefsConfig::new().definition(definition)).citations(citations);
    registry_with(config, vec![provider])
}

#[test]
fn an_entry_citing_through_a_user_definition_is_refused_under_both_placements() {
    let provider: Arc<dyn CitationProvider> = Arc::new(FromMap::of(&[("a", "Knuth, see \\mycite{a}")]));
    // In place, the entry citing its own key: refused, the document rendered.
    let config = CitationsConfig::new().placement(CitationPlacement::InPlace);
    let run = render(&with_mycite(config, Arc::clone(&provider)), "Before.\n\nX \\cite{a}. Y.");
    assert_eq!(
        identifiers(&run),
        ["flm.citations.entry-cites", "flm.citations.no-entry"],
        "{}",
        run.diagnostics().render_all()
    );
    assert_eq!(text_of(&run), "Before.\n\nX\u{a0}[[dropped: bibliography entry]]. Y.");
    // Listed: refused as the plain spelling is.
    let run = render(&with_mycite(CitationsConfig::new(), Arc::clone(&provider)), "X \\cite{a}.");
    assert_eq!(identifiers(&run), ["flm.citations.entry-cites", "flm.citations.no-entry"]);
    assert!(text_of(&run).ends_with("[1] [dropped: bibliography entry]"), "{}", text_of(&run));
    // The check workflow sees it.
    let registry = with_mycite(CitationsConfig::new(), provider);
    let checked = run_workflow(&registry, CHECK_WORKFLOW, &["X \\cite{a}."], &Overrides::none());
    assert_eq!(identifiers(&checked), ["flm.citations.entry-cites", "flm.citations.no-entry"]);
}

#[test]
fn an_entry_holding_a_paragraph_break_or_a_block_is_refused() {
    for content in ["one\n\ntwo", "\\begin{itemize}\\item one\\end{itemize}", "\\section{Oops}"] {
        for placement in [CitationPlacement::Listed, CitationPlacement::InPlace] {
            let provider: Arc<dyn CitationProvider> = Arc::new(FromMap::of(&[("a", content), ("b", "Lamport")]));
            let config = CitationsConfig::new().placement(placement);
            let run = render(&with_config(config, vec![provider]), "X \\cite{a,b}.");
            assert_eq!(
                identifiers(&run),
                ["flm.citations.entry-not-inline", "flm.citations.no-entry"],
                "{content}: {}",
                run.diagnostics().render_all()
            );
            assert_eq!(entries(&run)[0].1, Err(MissingEntry::NotInline));
            assert!(
                run.diagnostics().render_all().contains(
                    "the bibliography entry of the key `a` may not hold a paragraph break or a block such as a list or a heading"
                ),
                "{}",
                run.diagnostics().render_all()
            );
        }
    }
    // Built structurally, as a provider may: refused the same.
    struct Structural;
    impl CitationProvider for Structural {
        fn answer(&self, _request: &CitationRequest, cx: &ProviderCx<'_>) -> CitationResult {
            let source = Arc::new(flm_core::lang::FlmSource::new("one\n\ntwo"));
            let fragment = flm_core::lang::parse_fragment(
                cx.lang(),
                source,
                flm_core::model::doc::FragmentLayout::Blocks,
                &flm_core::lang::ParseOptions::default(),
            )
            .expect("a parse");
            let root = fragment.tree().root();
            let nodes: Vec<_> = root.children().iter().collect();
            let content = flm_core::lang::copy::copy_run_into_value(
                &nodes,
                &root,
                flm_core::lang::copy::ValueContent::Own,
                cx.lang().guard(),
            )
            .expect("a copy");
            Ok(Some(CitationAnswer::new(CitationEntry::new(content))))
        }
    }
    let run = render(&with(vec![Arc::new(Structural)]), "X \\cite{a}.");
    assert_eq!(identifiers(&run), ["flm.citations.entry-not-inline", "flm.citations.no-entry"]);
}
