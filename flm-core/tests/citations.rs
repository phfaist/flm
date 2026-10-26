//! The `citations` feature on the text output: chunks, the numbering by
//! first appearance, the marks in both forms, attached and standalone, the
//! word of `\R`, the configuration, and the mistakes of a citation's writing;
//! and `~`, whitespace beside a citation mark and at a paragraph's edges.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::core::run::Run;
use flm_core::dump::project::{ProjectOptions, Projector};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::FragmentLayout;
use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::stdcore::backends::pres::{payload_of, DroppedPayload, LinkPayload, SpanPayload, SpanStyle};
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::citations::{
    read_chunk, CitationsConfig, MarkForm, Reading, ATTACHED, CITATIONS, CITE, MARK_ROLE,
    STANDALONE, WORD_ROLE,
};
use flm_core::stdcore::features::numbering::{Delimiters, PluralWords, PrefixWords};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::{package, StdcoreConfig};

use common::citations::{Asked, Counting, EveryKey};
use common::lowering::{lowered, text as printed};
use common::stdcore::{identifiers, projection, run_workflow};

// --- helpers ---------------------------------------------------------------------------------

/// The standard package under `config`.
fn registry_of(config: CitationsConfig) -> Arc<FlmRegistry> {
    common::citations::registry(StdcoreConfig::new().citations(config))
}

/// The standard package, the default configuration.
fn registry() -> Arc<FlmRegistry> {
    registry_of(CitationsConfig::new())
}

/// Renders `texts`, one tree each, to text under `registry`.
fn render_on(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Run {
    run_workflow(registry, text::WORKFLOW, texts, &Overrides::none())
}

/// Renders one document to text under the default configuration.
fn render(document: &str) -> Run {
    render_on(&registry(), &[document])
}

/// The text a run wrote before the list of the cited works, its trailing
/// whitespace trimmed: the marks these tests look at stand in the content,
/// and the list is `citations_entries.rs`'s.
fn text_of(run: &Run) -> String {
    let text = printed(run);
    let content = text.split("\n\nReferences\n----------\n").next().unwrap_or_default();
    content.trim_end().to_string()
}

/// The text of `document` under the default configuration; the run must
/// report nothing.
fn clean(document: &str) -> String {
    let run = render(document);
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    text_of(&run)
}

/// The text of `document` under `config`; the run must report nothing.
fn clean_under(config: CitationsConfig, document: &str) -> String {
    let run = render_on(&registry_of(config), &[document]);
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    text_of(&run)
}

/// Every span of the lowered first tree whose role is `role`.
fn spans(run: &Run, role: &str) -> Vec<SpanPayload> {
    lowered(run, TreeId::tree(0))
        .tree
        .root()
        .descendants()
        .filter_map(|node| payload_of::<SpanPayload>(&node).cloned())
        .filter(|span| span.role.as_ref().is_some_and(|name| name.as_str() == role))
        .collect()
}

/// Every link of the lowered first tree.
fn links(run: &Run) -> Vec<LinkPayload> {
    lowered(run, TreeId::tree(0))
        .tree
        .root()
        .descendants()
        .filter_map(|node| payload_of::<LinkPayload>(&node).cloned())
        .collect()
}

/// Every dropped mark of the lowered first tree.
fn dropped(run: &Run) -> Vec<DroppedPayload> {
    lowered(run, TreeId::tree(0))
        .tree
        .root()
        .descendants()
        .filter_map(|node| payload_of::<DroppedPayload>(&node).cloned())
        .collect()
}

const NBSP: char = '\u{a0}';

/// A projection as compact JSON, with no whitespace outside strings that
/// hold none.
fn json(value: &techy::serialize::SerialValue) -> String {
    flm_core::dump::json::to_json(value).expect("the projection writes").replace([' ', '\n'], "")
}

// --- test 1: a chunk's items -----------------------------------------------------------------

/// The items `(key, note)` of the first `\cite` of `document`, parsed under
/// the standard package.
fn items(document: &str) -> (Vec<(String, Option<String>)>, usize) {
    let registry = registry();
    let fragment = parse_fragment(
        registry.lang(),
        Arc::new(FlmSource::new(document).with_origin(FlmOrigin::none())),
        FragmentLayout::default(),
        &ParseOptions::default(),
    )
    .expect("the document parses");
    let node = fragment
        .tree()
        .root()
        .descendants()
        .find(|node| element_of(node).name.as_str() == CITE)
        .expect("a \\cite");
    let chunk = read_chunk(&node, Reading::InStage).expect("read in a stage");
    let items = chunk
        .items
        .iter()
        .map(|item| {
            let note = item.note.as_ref().map(|note| {
                note.iter().map(|node| node.chars().unwrap_or("?").to_string()).collect::<String>()
            });
            (item.key.clone(), note)
        })
        .collect();
    (items, chunk.mistakes.len())
}

fn keys(list: &[&str]) -> Vec<(String, Option<String>)> {
    list.iter().map(|key| (key.to_string(), None)).collect()
}

#[test]
fn a_chunk_is_read_as_its_keys_split_trimmed_and_the_empty_ones_skipped() {
    assert_eq!(items("\\cite{a}"), (keys(&["a"]), 0));
    assert_eq!(items("\\cite{ a , b }"), (keys(&["a", "b"]), 0));
    assert_eq!(items("\\cite{a,,b}"), (keys(&["a", "b"]), 0));
    assert_eq!(items("\\cite{}"), (Vec::new(), 1));
    // A command among the keys is kept as written.
    assert_eq!(items("\\cite{a\\b}"), (keys(&["a\\b"]), 0));
}

#[test]
fn a_key_is_flm_code_and_a_comma_inside_braces_belongs_to_it() {
    // The keys argument is parsed FLM code: a group, a command and a formula
    // are kept whole, and only a comma outside them separates two keys.
    assert_eq!(
        items("\\cite{manual:{X et al., Journal of Future Results (2034)}}"),
        (keys(&["manual:{X et al., Journal of Future Results (2034)}"]), 0)
    );
    assert_eq!(items("\\cite{a,{b,c},d}"), (keys(&["a", "{b,c}", "d"]), 0));
    assert_eq!(items("\\cite{a,\\emph{b,c}}"), (keys(&["a", "\\emph{b,c}"]), 0));
    assert_eq!(items("\\cite{manual:\\emph{Title}}"), (keys(&["manual:\\emph{Title}"]), 0));
    // A piece of characters cut before a group keeps its spelling.
    assert_eq!(items("\\cite{ab, manual:{X, Y}cd,ef}"), (keys(&["ab", "manual:{X, Y}cd", "ef"]), 0));
}

#[test]
fn a_key_s_whitespace_is_one_space_and_its_ends_trimmed() {
    assert_eq!(items("\\cite{ manual:{A   B} ,\n x }"), (keys(&["manual:{A B}", "x"]), 0));
    assert_eq!(items("\\cite{ , }"), (Vec::new(), 1));
}

#[test]
fn a_key_s_quotes_and_dashes_are_recorded_as_typed() {
    let run = render("See \\cite{manual:{A \"T\" -- x}}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let table = run.table(&CITATIONS).expect("declared").expect("written");
    assert_eq!(table.keys().map(|(key, _)| key).collect::<Vec<_>>(), ["manual:{A \"T\" -- x}"]);
}

#[test]
fn two_spellings_of_a_key_differing_only_in_whitespace_are_one_key() {
    let asked = Asked::default();
    let counting = Counting { asked: asked.clone(), inner: Arc::new(EveryKey) };
    let registry = common::citations::registry_with(StdcoreConfig::new(), vec![Arc::new(counting)]);
    let run = render_on(&registry, &["A\\cite{manual:{A   B}} and B\\cite{manual:{A\nB}}."]);
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(asked.keys(), ["manual:{A B}"]);
    assert_eq!(text_of(&run), format!("A{NBSP}[1] and B{NBSP}[1]."));
}

#[test]
fn a_key_synthesized_by_a_user_definition_is_recomposed_from_its_nodes() {
    let asked = Asked::default();
    let counting = Counting { asked: asked.clone(), inner: Arc::new(EveryKey) };
    let userdefs = UserdefsConfig::new()
        .definition(Definition::macro_("man", vec![ArgDef::mandatory("x")]).content("\\cite{manual:\\emph{#1}}"))
        .definition(Definition::macro_("arxiv", vec![ArgDef::mandatory("key")]).content("\\cite{arxiv:#1}"));
    let registry = common::citations::registry_with(StdcoreConfig::new().userdefs(userdefs), vec![Arc::new(counting)]);
    let run = render_on(&registry, &["A\\man{Title} and B\\arxiv{1234}."]);
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(asked.keys(), ["manual:\\emph{Title}", "arxiv:1234"]);
}

#[test]
fn a_note_belongs_to_every_key_of_its_call() {
    let note = Some("n".to_string());
    assert_eq!(
        items("\\cite[n]{a,b}\\cite{c}").0,
        [("a".to_string(), note.clone()), ("b".to_string(), note), ("c".to_string(), None)]
    );
}

#[test]
fn a_chunk_naming_no_key_is_an_error_and_the_document_renders() {
    for document in ["See \\cite{}.", "See \\cite{ , }."] {
        let run = render(document);
        assert_eq!(identifiers(&run), ["flm.citations.no-key"]);
        assert_eq!(dropped(&run).len(), 1);
        assert!(text_of(&run).starts_with("See"), "{}", text_of(&run));
    }
}

#[test]
fn a_key_holding_a_command_reaches_the_providers() {
    // The library reads no meaning into a key: one holding a command, here
    // passed in by a user definition's use site, is a key like any.
    let asked = Asked::default();
    let counting = Counting { asked: asked.clone(), inner: Arc::new(EveryKey) };
    let userdefs = UserdefsConfig::new()
        .definition(Definition::macro_("arxiv", vec![ArgDef::mandatory("key")]).content("\\cite{arxiv:#1}"));
    let registry = common::citations::registry_with(StdcoreConfig::new().userdefs(userdefs), vec![Arc::new(counting)]);
    let run = render_on(&registry, &["See \\cite{a}\\arxiv{\\emph{b}}."]);
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(asked.keys(), ["a", "arxiv:\\emph{b}"]);
    assert_eq!(text_of(&run), format!("See{NBSP}[1]{NBSP}[2]."));
}

/// The standard package with the user definitions `\arxiv{x}` for
/// `\cite{arxiv:#1}` and `\mynote{n}{k}` for `\cite[see #1]{#2}`.
fn with_arxiv() -> Arc<FlmRegistry> {
    let userdefs = UserdefsConfig::new()
        .definition(Definition::macro_("arxiv", vec![ArgDef::mandatory("key")]).content("\\cite{arxiv:#1}"))
        .definition(
            Definition::macro_("mynote", vec![ArgDef::mandatory("note"), ArgDef::mandatory("key")])
                .content("\\cite[see #1]{#2}"),
        );
    common::citations::registry(StdcoreConfig::new().userdefs(userdefs))
}

#[test]
fn a_user_definition_s_placeholder_in_a_key_or_a_note_substitutes() {
    let run = render_on(&with_arxiv(), &["A\\arxiv{1234}, B\\mynote{Thm 4}{k} and \\cite{arxiv:1234}."]);
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(text_of(&run), format!("A{NBSP}[1], B{NBSP}[2; see Thm 4] and{NBSP}[1]."));
}

#[test]
fn a_user_definition_named_cite_replaces_the_standard_one_only_when_it_says_so() {
    let definition = || Definition::macro_("cite", vec![ArgDef::mandatory("key")]).content("<#1>");
    let config = StdcoreConfig::new().userdefs(UserdefsConfig::new().definition(definition().overriding()));
    let run = render_on(&common::citations::registry(config), &["See \\cite{a}."]);
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(text_of(&run), "See <a>.");
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().userdefs(UserdefsConfig::new().definition(definition()))));
    builder.use_package(flm_core::math_passthrough::package());
    let refused = builder.build(&RegistryInputs::new());
    assert!(refused.is_err(), "a second `\\cite` without `overrides` must fail the build");
}

// --- test 2: adjacent calls ------------------------------------------------------------------

/// How many `\cite` nodes the parse of `document` holds.
fn chunk_count(document: &str) -> usize {
    let registry = registry();
    let fragment = parse_fragment(
        registry.lang(),
        Arc::new(FlmSource::new(document).with_origin(FlmOrigin::none())),
        FragmentLayout::default(),
        &ParseOptions::default(),
    )
    .expect("the document parses");
    fragment.tree().root().descendants().filter(|node| element_of(node).name.as_str() == CITE).count()
}

#[test]
fn adjacent_calls_are_one_chunk_and_a_paragraph_break_or_text_ends_it() {
    assert_eq!(chunk_count("\\cite{a}\\cite[n]{b}"), 1);
    assert_eq!(chunk_count("\\cite{a} \\cite{b}"), 1);
    assert_eq!(chunk_count("\\cite{a}\n\\cite{b}"), 1);
    assert_eq!(chunk_count("\\cite{a}%% a comment\n\\cite{b}"), 1);
    assert_eq!(chunk_count("\\cite{a}\n\n\\cite{b}"), 2);
    assert_eq!(chunk_count("\\cite{a} and \\cite{b}"), 2);
    assert_eq!(clean("Text \\cite{a} \\cite{b}\\cite{c}."), format!("Text{NBSP}[1–3]."));
}

#[test]
fn the_projection_shows_the_further_calls_of_a_chunk() {
    let registry = registry();
    let fragment = parse_fragment(
        registry.lang(),
        Arc::new(FlmSource::new("\\cite{a}\\cite[n]{b}").with_origin(FlmOrigin::none())),
        FragmentLayout::default(),
        &ParseOptions::default(),
    )
    .expect("the document parses");
    let projected = json(&Projector::new(ProjectOptions::new()).tree(fragment.tree()));
    assert!(projected.contains("flm.citations.cite"), "{projected}");
    assert!(projected.contains("\"more\""), "{projected}");
    assert!(projected.contains("flm.field"), "{projected}");
    assert!(projected.contains("\"b\""), "{projected}");
    assert!(projected.contains("\"n\""), "{projected}");
}

// --- test 3: the numbers ---------------------------------------------------------------------

#[test]
fn works_are_numbered_by_first_appearance_across_the_trees() {
    let run = render_on(
        &registry(),
        &["A \\cite{x}\\cite{y}. B\\footnote{Note \\cite{z}.} \\cite{x}.", "C \\cite{w,x}."],
    );
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let table = json(&projection(&run, CITATIONS));
    for (key, number) in [("x", 1), ("y", 2), ("z", 3), ("w", 4)] {
        let entry = format!("\"key\":\"{key}\",\"number\":{number},\"id\":\"citation-{number}\"");
        assert!(table.contains(&entry), "{entry} in {table}");
    }
    let hrefs: Vec<String> =
        links(&run).into_iter().filter(|link| link.kind.as_str() == "cite").map(|link| link.href).collect();
    // The footnote's mark stands in the list of footnotes, not in the tree.
    assert_eq!(hrefs, ["#citation-1", "#citation-2", "#citation-1"]);
}

#[test]
fn a_key_first_cited_in_a_footnote_is_numbered_at_the_footnote_s_place() {
    // The footnote comes before any other citation of `a`, and its key is
    // numbered there, not after the running text.
    let run = render("A\\footnote{B \\cite{a}.} C \\cite{b,a}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let table = json(&projection(&run, CITATIONS));
    for (key, number) in [("a", 1), ("b", 2)] {
        let entry = format!("\"key\":\"{key}\",\"number\":{number},\"id\":\"citation-{number}\"");
        assert!(table.contains(&entry), "{entry} in {table}");
    }
    let text = text_of(&run);
    assert!(text.contains(&format!("C{NBSP}[1,2].")), "{text}");
    assert!(text.contains(&format!("B{NBSP}[1].")), "{text}");
}

// --- test 4: the grammar ---------------------------------------------------------------------

#[test]
fn the_items_without_a_note_come_first_then_one_group_per_note() {
    let document = "\\cite{a,b,c,d,e}. X\\cite{c,b}\\cite[Thm 4]{a}\\cite{d}\\cite[Thm 4]{e}.";
    // Three consecutive numbers are a range under the default style.
    assert_eq!(clean(document), format!("{NBSP}[1–5]. X{NBSP}[2–4][1,5; Thm 4]."));
    let mut config = CitationsConfig::new();
    config.style.ranges = false;
    assert_eq!(clean_under(config, document), format!("{NBSP}[1,2,3,4,5]. X{NBSP}[2,3,4][1,5; Thm 4]."));
    assert_eq!(clean("\\cite{a,b,c,d,e}. X\\cite{e,b,a}."), format!("{NBSP}[1–5]. X{NBSP}[1,2,5]."));
}

#[test]
fn two_notes_differing_in_whitespace_are_one_group() {
    assert_eq!(
        clean("\\cite{a,b}. X\\cite[Thm  4]{a}\\cite[Thm\n4]{b}."),
        format!("{NBSP}[1,2]. X{NBSP}[1,2; Thm 4].")
    );
}

#[test]
fn two_different_notes_are_two_groups_in_order_of_first_appearance() {
    assert_eq!(
        clean("\\cite{a,b,c}. X\\cite[p. 5]{c}\\cite[Thm 4]{a}\\cite{b}\\cite[p. 5]{a}."),
        format!("{NBSP}[1–3]. X{NBSP}[2][1,3; p. 5][1; Thm 4].")
    );
}

#[test]
fn a_note_is_lowered_content_with_its_edges_trimmed() {
    let run = render("X\\cite[ see \\emph{Thm} \\(x\\) ]{a}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(text_of(&run), format!("X{NBSP}[1; see Thm \\(x\\)]."));
    let registry = registry();
    let run = run_workflow(&registry, "check", &["X\\cite[ see \\emph{Thm} ]{a}."], &Overrides::none());
    assert!(identifiers(&run).is_empty());
}

#[test]
fn the_separator_and_the_delimiters_are_the_configuration_s() {
    let mut config = CitationsConfig::new().note_separator(", ");
    config.style.delimiters = Delimiters::new("(", ")");
    assert_eq!(clean_under(config, "X\\cite[p. 3]{a}\\cite{b}."), format!("X{NBSP}(2)(1, p. 3)."));
}

#[test]
fn a_style_without_ranges_writes_every_number_alone() {
    let mut config = CitationsConfig::new();
    config.style.ranges = false;
    let run = render_on(&registry_of(config), &["X\\cite{a,b,c,d,e}."]);
    assert_eq!(text_of(&run), format!("X{NBSP}[1,2,3,4,5]."));
    assert_eq!(links(&run).len(), 5);
    let run = render("X\\cite{a,b,c,d,e}.");
    assert_eq!(text_of(&run), format!("X{NBSP}[1–5]."));
    assert_eq!(links(&run).len(), 2);
}

// --- test 5: the space before a mark ---------------------------------------------------------

#[test]
fn a_cite_removes_the_author_s_space_and_writes_a_no_break_space() {
    assert_eq!(clean("text \\cite{a}"), format!("text{NBSP}[1]"));
    assert_eq!(clean("text\\cite{a}"), format!("text{NBSP}[1]"));
    assert_eq!(clean("text, \\cite{a}"), format!("text,{NBSP}[1]"));
    assert_eq!(clean("(\\cite{a})"), format!("({NBSP}[1])"));
    assert_eq!(clean("(\\R[I]\\cite{a})"), "([1])");
    assert_eq!(clean("\\cite{a} starts"), format!("{NBSP}[1] starts"));
}

#[test]
fn a_cite_after_a_formula_or_a_footnote_mark_writes_its_space() {
    let text = clean("A \\(x\\) \\cite{a}. B\\footnote{n} \\cite{a}.");
    assert!(text.contains(&format!("{NBSP}[1]. B")), "{text}");
    assert!(text.contains(&format!("{NBSP}[1].\n")) || text.contains(&format!("{NBSP}[1].")), "{text}");
    assert_eq!(text.matches(NBSP).count(), 2, "{text}");
}

#[test]
fn a_cite_starting_a_cell_or_a_line_shows_its_space() {
    let config = StdcoreConfig::new()
        .cells(flm_core::stdcore::features::cells::CellsConfig::new().global(true));
    let run = render_on(
        &common::citations::registry(config),
        &["\\begin{cells}\\cell{\\cite{a}}\\celldata{x & \\cite{b}}\\end{cells}"],
    );
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let text = text_of(&run);
    assert!(text.contains(&format!("{NBSP}[1]")) && text.contains(&format!("{NBSP}[2]")), "{text}");
}

#[test]
fn a_cite_starting_a_line_of_lines_shows_its_space() {
    let text = clean("\\begin{quote}\\lines{First \\cite{a}\\\\ \\cite{b} second}\\end{quote}");
    assert!(text.contains(&format!("First{NBSP}[1]\n")), "{text}");
    assert!(text.contains(&format!("{NBSP}[2] second")), "{text}");
}

#[test]
fn a_comment_between_the_text_and_a_cite_leaves_no_space_before_the_mark() {
    for document in ["text %% c\n\\cite{a} more", "text %% c\n  %% d\n  \\cite{a} more"] {
        let run = render(document);
        assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
        let written: String = lowered(&run, TreeId::tree(0))
            .tree
            .root()
            .descendants()
            .filter_map(|node| node.chars().map(str::to_string))
            .collect();
        assert!(written.starts_with("text") && !written.starts_with("text "), "{written:?}");
        assert!(written.contains(&format!("{NBSP}[1] more")), "{written:?}");
        assert!(!written.contains(&format!(" {NBSP}")), "{written:?}");
    }
}

#[test]
fn a_cite_removes_a_no_break_space_written_before_it_as_any_space() {
    let documents = ["word~\\cite{a}", "word ~ \\cite{a}", "word~~\\cite{a}", "word\\ \\cite{a}"];
    for document in documents {
        assert_eq!(clean(document), format!("word{NBSP}[1]"), "{document}");
    }
}

// `~` is whitespace to the layout of paragraphs wherever it stands, beside a
// citation mark or not.

#[test]
fn a_no_break_space_at_a_paragraph_s_edge_is_removed_and_one_inside_is_kept() {
    // The blank line lays the document out as paragraphs.
    let run = render("~a~b~\n\nc");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let tree = run.frozen_tree(TreeId::tree(0)).expect("the first tree is frozen");
    let paragraph = tree.root().children().iter().next().expect("the first paragraph");
    let children: Vec<String> = paragraph
        .children()
        .iter()
        .map(|child| match child.chars() {
            Some(text) => text.to_string(),
            None => element_of(&child).name.to_string(),
        })
        .collect();
    assert_eq!(children, ["a", "flm.literal", "b"]);
}

#[test]
fn a_straight_quote_after_a_no_break_space_opens() {
    // A document of one line is laid out inline, where the `~` is kept.
    assert_eq!(clean("~\"x\""), format!("{NBSP}\u{201c}x\u{201d}"));
    assert_eq!(clean("a~\"x\""), format!("a{NBSP}\u{201c}x\u{201d}"));
}

#[test]
fn an_r_keeps_the_author_s_space_and_writes_none() {
    assert_eq!(clean("(see \\R\\cite{a})"), format!("(see ref.{NBSP}[1])"));
    assert_eq!(clean("(see\\R\\cite{a})"), format!("(seeref.{NBSP}[1])"));
}

#[test]
fn under_the_superscript_form_a_cite_writes_no_space_and_is_raised() {
    let config = CitationsConfig::new().mark(MarkForm::Superscript);
    assert_eq!(clean_under(config.clone(), "text \\cite{a,b}."), "text¹,².");
    let run = render_on(&registry_of(config), &["text \\cite{a,b,c}."]);
    let marks = spans(&run, MARK_ROLE);
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].style, Some(SpanStyle::Superscript));
    assert_eq!(marks[0].kinds.iter().map(|kind| kind.as_str()).collect::<Vec<_>>(), [ATTACHED]);
}

// --- test 6 and 7: `\R` ----------------------------------------------------------------------

#[test]
fn an_r_under_the_superscript_form_shows_bare_inline_numbers() {
    let config = CitationsConfig::new().mark(MarkForm::Superscript);
    assert_eq!(clean_under(config.clone(), "See \\R\\cite{a,b}."), format!("See refs.{NBSP}1,2."));
    assert_eq!(clean_under(config.clone(), "(see \\R[I]\\cite{a,b})"), "(see 1,2)");
    let run = render_on(&registry_of(config), &["See \\R\\cite{a,b}."]);
    let marks = spans(&run, MARK_ROLE);
    assert_eq!(marks[0].style, None);
    assert_eq!(marks[0].kinds.iter().map(|kind| kind.as_str()).collect::<Vec<_>>(), [STANDALONE]);
}

#[test]
fn a_raised_mark_in_a_theorem_s_title_is_raised_on_the_text_output() {
    // A theorem's title is a run-in decoration, printed as one piece of text:
    // its raised mark is written in the superscript forms as in running text.
    let config = CitationsConfig::new().mark(MarkForm::Superscript);
    let run = render_on(
        &registry_of(config),
        &["\\begin{theorem}[Main \\cite{a}]\\noproofref\nText \\cite{b}.\n\\end{theorem}"],
    );
    let text = text_of(&run);
    assert!(text.contains("(Main\u{b9}).") && text.contains("Text\u{b2}."), "{text}");
}

#[test]
fn an_r_writes_the_style_s_word_singular_or_plural() {
    assert_eq!(clean("See \\R\\cite{a}."), format!("See ref.{NBSP}[1]."));
    assert_eq!(clean("See \\R\\cite{a,b}."), format!("See refs.{NBSP}[1,2]."));
    assert_eq!(clean("\\R[S]\\cite{a} and \\R[S]\\cite{a,b}."), format!("Ref.{NBSP}[1] and Refs.{NBSP}[1,2]."));
    assert_eq!(clean("See \\R[I]\\cite{a,b}."), "See [1,2].");
    assert_eq!(clean("See \\R\\cite[Thm 4]{a}."), format!("See ref.{NBSP}[1; Thm 4]."));
    assert_eq!(clean("See \\R\\cite{a}\\cite{b}."), format!("See refs.{NBSP}[1,2]."));
    let run = render("See \\R\\cite{a,b}.");
    assert_eq!(spans(&run, WORD_ROLE).len(), 1);
}

#[test]
fn an_unknown_flag_or_an_r_without_citation_is_an_error() {
    let run = render("See \\R[X]\\cite{a}.");
    assert_eq!(identifiers(&run), ["flm.citations.unknown-flag"]);
    assert_eq!(text_of(&run), format!("See ref.{NBSP}[1]."));
    let run = render("See \\R{\\cite{a}}.");
    assert_eq!(identifiers(&run), ["flm.citations.no-citation"]);
    assert_eq!(dropped(&run).len(), 1);
    assert!(text_of(&run).contains(&format!("{NBSP}[1]")), "{}", text_of(&run));
    let run = render("See \\R text.");
    assert_eq!(identifiers(&run), ["flm.citations.no-citation"]);
}

#[test]
fn the_style_s_words_are_the_configuration_s() {
    let mut config = CitationsConfig::new();
    config.style.prefix_words =
        PrefixWords::new("cit.~", "cits.~").with_variant("capital", PluralWords::new("Cit.~", "Cits.~"));
    assert_eq!(
        clean_under(config, "\\R[S]\\cite{a,b} and \\R\\cite{a}."),
        format!("Cits.{NBSP}[1,2] and cit.{NBSP}[1].")
    );
}

// --- test 13: copies ------------------------------------------------------------------------

#[test]
fn a_chunk_in_a_heading_title_shows_its_numbers_where_a_reference_shows_the_title() {
    let run = render("\\section{Results \\cite{b}}\\label{sec:r}\n\nFirst \\cite{a}. See \\ref{sec:r}.");
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let text = text_of(&run);
    assert!(text.contains(&format!("See Results{NBSP}[1]")), "{text}");
    let hrefs: Vec<String> =
        links(&run).into_iter().filter(|link| link.kind.as_str() == "cite").map(|link| link.href).collect();
    assert_eq!(hrefs, ["#citation-1", "#citation-2", "#citation-1"]);
}

#[test]
fn a_report_raised_in_a_copy_carries_the_copy_s_frame() {
    let run = render("\\section{Results \\cite{}}\\label{sec:r}\n\nSee \\ref{sec:r}.");
    assert_eq!(identifiers(&run), ["flm.citations.no-key", "flm.citations.no-key"]);
    let framed = common::stdcore::frames(&run)
        .iter()
        .filter(|frames| frames.iter().any(|(title, _)| title == common::stdcore::SHOWN_HERE))
        .count();
    assert_eq!(framed, 1);
}

// --- test 15: apart from the text -----------------------------------------------------------

/// The words of the content of `document`, read as a heading's identity is.
fn words(document: &str) -> String {
    let registry = registry();
    let fragment = parse_fragment(
        registry.lang(),
        Arc::new(FlmSource::new(document).with_origin(FlmOrigin::none())),
        FragmentLayout::default(),
        &ParseOptions::default(),
    )
    .expect("the document parses");
    flm_core::lang::text::plain_text_of(fragment.tree().root().children().iter())
}

#[test]
fn the_keys_are_no_words_of_the_text_and_the_notes_are() {
    assert_eq!(words("Results\\cite{a}"), "Results");
    assert_eq!(words("Results \\cite[Thm 4]{a}\\cite{b}\\cite[c]{d}"), "Results Thm 4c");
    assert_eq!(words("Results \\R[S]\\cite{a}"), "Results ");
}

// --- test 18: other rosters ------------------------------------------------------------------

/// The standard package, the feature `left_out` left out.
fn registry_without(left_out: &str) -> Arc<FlmRegistry> {
    common::citations::registry_over(common::stdcore::package_features_but(StdcoreConfig::new(), left_out))
}

const EVERY_FORM: &str = "A \\cite{a}\\cite[Thm 4]{b}, \\R\\cite{a,b} and \\R[S]\\cite{c}.";

#[test]
fn the_marks_are_the_same_without_numbering_refs_or_a_backend() {
    let expected = format!("A{NBSP}[1][2; Thm 4], refs.{NBSP}[1,2] and Ref.{NBSP}[3].");
    assert_eq!(clean(EVERY_FORM), expected);
    for left_out in ["numbering", "refs", "xnotes", "html", "latex-recomposed"] {
        let run = render_on(&registry_without(left_out), &[EVERY_FORM]);
        assert!(identifiers(&run).is_empty(), "{left_out}: {}", run.diagnostics().render_all());
        assert_eq!(text_of(&run), expected, "{left_out}");
    }
}

#[test]
fn without_the_feature_cite_is_an_unknown_command() {
    let features = common::stdcore::package_features_but(StdcoreConfig::new(), "citations");
    let run = render_on(&common::stdcore::registry_over(features), &["A \\cite{a}."]);
    assert_eq!(identifiers(&run), ["core.specs.unresolvable-command"]);
}

// --- test 19: the configuration --------------------------------------------------------------

#[test]
fn a_style_text_or_a_separator_that_does_not_compile_fails_the_build_naming_it() {
    let build = |config: CitationsConfig| {
        let mut builder = RegistryBuilder::new();
        builder.use_package(package(StdcoreConfig::new().citations(config)));
        builder.use_package(flm_core::math_passthrough::package());
        match builder.build(&RegistryInputs::new()) {
            Ok(_) => panic!("the registry build was expected to fail"),
            Err(error) => error.render(),
        }
    };
    let mut config = CitationsConfig::new();
    config.style.delimiters = Delimiters::new("{", "]");
    let message = build(config);
    assert!(message.contains("the option `style` of the feature `citations` is not a valid counter style"), "{message}");
    let message = build(CitationsConfig::new().note_separator("}"));
    assert!(message.contains("the option `note_separator` of the feature `citations`"), "{message}");
}

#[test]
fn the_digest_changes_with_the_form_the_style_and_the_separator() {
    let digest = |config: CitationsConfig| registry_of(config).digest();
    let plain = digest(CitationsConfig::new());
    assert_eq!(plain, digest(CitationsConfig::new()));
    assert_ne!(plain, digest(CitationsConfig::new().mark(MarkForm::Superscript)));
    assert_ne!(plain, digest(CitationsConfig::new().note_separator(", ")));
    assert_ne!(plain, digest(CitationsConfig::new().heading_level(3)));
    let mut config = CitationsConfig::new();
    config.style.prefix_words = PrefixWords::new("r.~", "rr.~");
    assert_ne!(plain, digest(config));
    let mut config = CitationsConfig::new();
    config.style.ranges = false;
    assert_ne!(plain, digest(config));
}

// --- test 20: hard inputs --------------------------------------------------------------------

#[test]
fn no_document_input_panics_or_loops() {
    let many: Vec<String> = (0..200).map(|index| format!("k{index}")).collect();
    let fifty = "\\cite{x}".repeat(50);
    let long = "k".repeat(1000);
    let deep = {
        let mut inner = "\\cite{deep}".to_string();
        for depth in 0..10 {
            inner = match depth % 3 {
                0 => format!("A\\footnote{{{inner}}}"),
                1 => format!("\\begin{{itemize}}\\item {inner}\\end{{itemize}}"),
                _ => format!("\\begin{{quote}}{inner}\\end{{quote}}"),
            };
        }
        inner
    };
    let documents = [
        format!("A \\cite{{{}}}.", many.join(",")),
        format!("A {fifty}."),
        deep,
        "\\begin{theorem}[On \\cite{t}]X\\end{theorem}".to_string(),
        "\\cite[see \\cite{b}]{a}".to_string(),
        "\\cite{a}".to_string(),
        format!("\\cite{{{long}}}"),
        "\\R\\R\\cite{a}".to_string(),
        "\\R[\\cite{a}]\\cite{b}".to_string(),
        "\\cite[\\R\\cite{a}]{b}".to_string(),
    ];
    for document in &documents {
        let run = render(document);
        let _ = text_of(&run);
    }
    let run = render(&format!("A \\cite{{{}}}.", many.join(",")));
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    assert_eq!(text_of(&run), format!("A{NBSP}[1–200]."));
    // The numbers of a group are a set: one key written fifty times shows
    // its number once.
    let run = render(&format!("A {fifty}."));
    assert_eq!(text_of(&run), format!("A{NBSP}[1]."));
}

#[test]
fn the_hard_inputs_render_as_expected() {
    // A theorem's title, a note holding a chunk, fifty tacked-on calls with
    // different keys, alone and after `\R`, and a table nested in a cell.
    // (The theorem asks for a label and a proof, a warning of its own.)
    let theorem = text_of(&render("\\begin{theorem}[On \\cite{t}]X \\cite{u}\\end{theorem}"));
    assert_eq!(theorem, format!("Theorem{NBSP}1 (On{NBSP}[1]). X{NBSP}[2]"));
    assert_eq!(clean("X \\cite[see \\cite{b}]{a}."), format!("X{NBSP}[1; see{NBSP}[2]]."));
    let fifty: String = (1..=50).map(|index| format!("\\cite{{k{index}}}")).collect();
    assert_eq!(clean(&format!("A {fifty}.")), format!("A{NBSP}[1–50]."));
    assert_eq!(clean(&format!("A \\R{fifty}.")), format!("A refs.{NBSP}[1–50]."));
    let config = StdcoreConfig::new()
        .cells(flm_core::stdcore::features::cells::CellsConfig::new().global(true));
    let run = render_on(
        &common::citations::registry(config),
        &["\\begin{cells}\\cell{\\begin{cells}\\cell{A \\cite{a}}\\cell{c}\\end{cells}}\\end{cells}"],
    );
    assert!(identifiers(&run).is_empty(), "{}", run.diagnostics().render_all());
    let text = text_of(&run);
    assert!(text.contains(&format!("A{NBSP}[1]")), "{text}");
}

// --- the guide's example ---------------------------------------------------------------------

#[test]
fn the_syntax_guide_s_example_reads_as_the_guide_says() {
    let text = clean(
        "Codes of this family are well studied \\cite{knuth84,lamport94}. The bound is\n\
         tight \\cite[Thm 4]{knuth84}\\cite{shor95}. As \\R\\cite{lamport94} shows, and as\n\
         \\R[S]\\cite{knuth84,shor95} confirm, the proof is short (see, for instance,\n\
         \\R[I]\\cite{shor95}).",
    );
    let text = text.split_ascii_whitespace().collect::<Vec<_>>().join(" ");
    assert_eq!(
        text,
        format!(
            "Codes of this family are well studied{NBSP}[1,2]. The bound is tight{NBSP}[3][1; Thm 4]. \
             As ref.{NBSP}[2] shows, and as Refs.{NBSP}[1,3] confirm, the proof is short (see, for \
             instance, [3])."
        )
    );
}
