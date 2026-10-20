//! The `annotations` feature's lowering and its text output: the highlight
//! and the comment as spans or blocks of their roles with the initials a
//! margin decoration, the cut annotation's pieces lowered, the hidden render
//! option, the text backend's rows, annotations in every place inline and
//! block content may stand, and the text the same sequentially and in
//! parallel.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{Flm, FlmSource, ParseOptions, parse_fragment};
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::model::addr::TreeId;
use flm_core::model::doc::FragmentLayout;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::pres::read::payload_of;
use flm_core::stdcore::backends::pres::{BlockPayload, SpanPayload};
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::annotations::{
    ANNOTATION, AnnotationsConfig, AnnotationsRenderOptions, Annotator,
};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::StdcoreConfig;
use techy::core::node::NodeRef;

use common::stdcore::{identifiers, plan, run_workflow};

/// The two annotators these tests use: `phf` with the initials `PhF`,
/// annotator 0, and `abc` with none, annotator 1.
fn config() -> StdcoreConfig {
    StdcoreConfig::new().annotations(
        AnnotationsConfig::new()
            .annotator(Annotator::new("phf").initials("PhF"))
            .annotator(Annotator::new("abc")),
    )
}

/// The standard package under [`config`].
fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(config())
}

/// The render request hiding every annotation.
fn hidden() -> Overrides {
    Overrides {
        render_options: OptionValues::new().with(AnnotationsRenderOptions { hidden: true }),
        ..Overrides::none()
    }
}

/// `document` rendered through the `text` workflow under `overrides`.
fn render_under(registry: &Arc<FlmRegistry>, document: &str, overrides: &Overrides) -> Run {
    run_workflow(registry, text::WORKFLOW, &[document], overrides)
}

/// `document` rendered through the `text` workflow; any diagnostic is a test
/// failure.
fn render(document: &str) -> Run {
    let run = render_under(&registry(), document, &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    run
}

/// The printed text of `document`.
fn printed(document: &str) -> String {
    common::lowering::text(&render(document))
}

/// The printed text of `document`, hidden.
fn printed_hidden(document: &str) -> String {
    let run = render_under(&registry(), document, &hidden());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    common::lowering::text(&run)
}

/// A compact picture of a lowered node: text quoted; a presentation node by
/// its short name, a span or a block with its role and kinds in angle
/// brackets, its body in square brackets, and each attached slot as
/// `{name: …}`.
fn pshape(node: &NodeRef<'_, Flm>) -> String {
    if let Some(text) = node.chars() {
        return format!("{text:?}");
    }
    let element = node.ext().element.name.to_string();
    let short = element.strip_prefix("flm.pres.").unwrap_or(&element).to_string();
    let payload = if let Some(span) = payload_of::<SpanPayload>(node) {
        summary(span.role.as_ref().map(|r| r.as_str()), &span.kinds)
    } else if let Some(block) = payload_of::<BlockPayload>(node) {
        summary(block.role.as_ref().map(|r| r.as_str()), &block.kinds)
    } else {
        String::new()
    };
    let mut out = format!("{short}{payload}");
    let Some(slots) = node.slots() else {
        let children: Vec<String> = node.children().iter().map(|child| pshape(&child)).collect();
        return format!("{out}[{}]", children.join(" "));
    };
    for (index, slot) in slots.iter().enumerate() {
        let content: Vec<String> = node
            .slot_content_nodes(index)
            .map(|nodes| nodes.iter().map(|child| pshape(&child)).collect())
            .unwrap_or_default();
        match slot.name.as_deref() {
            Some("body") | None => out.push_str(&format!("[{}]", content.join(" "))),
            Some(name) => out.push_str(&format!("{{{name}: {}}}", content.join(" "))),
        }
    }
    out
}

/// A span's or a block's role and kinds, `<role|kind,kind>`.
fn summary(role: Option<&str>, kinds: &[flm_core::model::names::Name]) -> String {
    let kinds: Vec<&str> = kinds.iter().map(|kind| kind.as_str()).collect();
    format!("<{}|{}>", role.unwrap_or(""), kinds.join(","))
}

/// The shapes of the lowered tree's top-level nodes.
fn lowered(document: &str) -> Vec<String> {
    lowered_run(&render(document))
}

/// The shapes of a run's first lowered tree's top-level nodes.
fn lowered_run(run: &Run) -> Vec<String> {
    let tree = &common::lowering::lowered(run, TreeId::tree(0)).tree;
    tree.root().children().iter().map(|node| pshape(&node)).collect()
}

/// The initials decoration of annotator 0 as the shape shows it.
const PHF_INITIALS: &str =
    "{flm.pres.deco.margin: deco.margin[span<flm.annotations.initials|phf,annotator-0>[\"PhF\"]]}";

// ---------------------------------------------------------------------------------------------
// the presentation tree (P11, P12; test 11)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_highlight_is_a_span_of_its_role_with_the_initials_a_margin_decoration_marked_first() {
    // A fragment of one paragraph lowers to that paragraph's run.
    let span = format!(
        "span<flm.annotations.highlight|phf,annotator-0>[deco-mark[] \"passage\"]{PHF_INITIALS}"
    );
    assert_eq!(lowered("a \\phf{passage} b"), ["\"a \"".to_string(), span.clone(), "\" b\"".to_string()]);
    assert_eq!(lowered("a \\phf passage\\endphf, b"), ["\"a \"".to_string(), span, "\", b\"".to_string()]);
}

#[test]
fn an_annotator_with_no_initials_stages_no_decoration() {
    assert_eq!(lowered("\\abc{x}"), ["span<flm.annotations.highlight|abc,annotator-1>[\"x\"]"]);
}

/// The shapes of the top-level nodes `document` lowers to on the `html`
/// plan: the general rules, on a backend with no annotation rule of its own —
/// the shape the HTML page and every other output start from.
fn lowered_general(document: &str) -> Vec<String> {
    let html = run_workflow(&registry(), "html", &[document], &Overrides::none());
    assert_eq!(identifiers(&html), Vec::<String>::new(), "{}", html.diagnostics().render_all());
    let tree = &html
        .table(flm_core::stdcore::backends::lowering::lowered_table(&"html".into()))
        .expect("declared")
        .expect("written")
        .get(TreeId::tree(0))
        .expect("lowered")
        .tree;
    tree.root().children().iter().map(|node| pshape(&node)).collect()
}

#[test]
fn a_comment_is_a_span_of_its_role_shaped_as_a_highlight() {
    assert_eq!(
        lowered_general("\\phf[note]"),
        [format!("span<flm.annotations.comment|phf,annotator-0>[deco-mark[] \"note\"]{PHF_INITIALS}")]
    );
}

#[test]
fn a_bracket_after_a_highlight_is_text() {
    assert_eq!(
        lowered("\\phf{x}[y]"),
        [format!("span<flm.annotations.highlight|phf,annotator-0>[deco-mark[] \"x\"]{PHF_INITIALS}"), String::from("\"[y]\"")]
    );
}

#[test]
fn the_pieces_of_a_cut_highlight_are_spans_the_first_with_the_decoration() {
    let top = lowered("Some text \\phf{A comment.\n\nWith multiple paragraphs.} More text");
    assert_eq!(
        top,
        [
            format!("par[\"Some text \" span<flm.annotations.highlight|phf,annotator-0,mark-continues>[deco-mark[] \"A comment.\"]{PHF_INITIALS}]"),
            String::from("par[span<flm.annotations.highlight|phf,annotator-0,mark-continued>[\"With multiple paragraphs.\"] \" More text\"]"),
        ]
    );
}

#[test]
fn the_pieces_of_a_cut_comment_are_spans_the_first_with_the_decoration() {
    // Under the general rule: the text plan folds a comment between brackets
    // and stages no decoration.
    let top = lowered_general("Some text \\phf[A comment.\n\nWith multiple paragraphs.] More text");
    assert_eq!(
        top,
        [
            format!("par[\"Some text \" span<flm.annotations.comment|phf,annotator-0,mark-continues>[deco-mark[] \"A comment.\"]{PHF_INITIALS}]"),
            String::from("par[span<flm.annotations.comment|phf,annotator-0,mark-continued>[\"With multiple paragraphs.\"] \" More text\"]"),
        ]
    );
}

#[test]
fn a_block_piece_is_a_block_of_the_role_holding_the_list() {
    let top = lowered("\\phf see \\begin{itemize}\\item a\\end{itemize} here \\endphf");
    assert_eq!(top.len(), 3, "{top:?}");
    // Whitespace at the edges of a piece stays inside it, as parsed.
    assert_eq!(top[0], format!("par[span<flm.annotations.highlight|phf,annotator-0,mark-continues>[deco-mark[] \"see \"]{PHF_INITIALS}]"));
    assert!(top[1].starts_with("block<flm.annotations.highlight|phf,annotator-0,mark-continues,mark-continued>[list"), "{top:?}");
    assert!(!top[1].contains("deco"), "{top:?}");
    assert_eq!(top[2], "par[span<flm.annotations.highlight|phf,annotator-0,mark-continued>[\" here \"]]");
}

// ---------------------------------------------------------------------------------------------
// the text output (P12, P16; test 13)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_text_rows() {
    assert_eq!(printed("word \\phf{passage} word"), "word [PhF] passage word\n");
    assert_eq!(printed("word \\phf passage\\endphf{} word"), "word [PhF] passage word\n");
    assert_eq!(printed("word \\phf[note] word"), "word [PhF: note] word\n");
    assert_eq!(printed("word \\abc[note] word"), "word [note] word\n");
    assert_eq!(printed("word \\abc{passage} word"), "word passage word\n");
    assert_eq!(printed("word \\phf{x}[y] word"), "word [PhF] x[y] word\n");
}

#[test]
fn a_cut_comment_opens_its_bracket_on_its_first_piece_and_closes_it_on_its_last() {
    assert_eq!(
        printed("Some text \\phf[A comment.\n\nWith multiple paragraphs.] More text"),
        "Some text [PhF: A comment.\n\nWith multiple paragraphs.] More text\n"
    );
    assert_eq!(
        printed("Some \\abc[a\n\nb\n\nc] text"),
        "Some [a\n\nb\n\nc] text\n"
    );
}

#[test]
fn a_cut_highlight_shows_the_initials_on_its_first_piece() {
    assert_eq!(printed("\\phf{this\n\nthat}"), "[PhF] this\n\nthat\n");
}

#[test]
fn whitespace_beside_a_cut_prints_as_the_paragraphs_trim_it() {
    assert_eq!(printed("x \\phf[a \n\n b] y"), "x [PhF: a\n\nb] y\n");
}

#[test]
fn an_annotation_holding_only_a_paragraph_break_makes_no_piece_and_prints_nothing() {
    // An empty argument is an annotation of no content, which prints its
    // initials; an argument of paragraph breaks only is cut, and its cut
    // yields no piece, so nothing of the annotation remains.
    assert_eq!(printed("a \\phf{} b"), "a [PhF] b\n");
    assert_eq!(printed("a \\phf{\n\n} b"), "a\n\nb\n");
}

#[test]
fn a_block_piece_prints_its_initials_on_a_line_of_their_own_above_the_block() {
    assert_eq!(
        printed("\\phf see \\begin{itemize}\\item a\\end{itemize} here \\endphf"),
        "[PhF] see\n\n• a\n\nhere\n"
    );
    assert_eq!(printed("\\phf{\\begin{itemize}\\item a\\end{itemize}}"), "[PhF]\n\n• a\n");
    // A comment around one block: its brackets are text beside the block,
    // each filled into a paragraph of its own.
    assert_eq!(printed("\\phf[\\begin{itemize}\\item a\\end{itemize}]"), "[PhF:\n\n• a\n\n]\n");
}

// ---------------------------------------------------------------------------------------------
// hidden (P9; test 12 on text)
// ---------------------------------------------------------------------------------------------

#[test]
fn hidden_a_highlight_is_its_passage_and_a_comment_is_nothing() {
    assert_eq!(printed_hidden("word \\phf{passage} word"), "word passage word\n");
    assert_eq!(printed_hidden("word \\phf passage\\endphf{} word"), "word passage word\n");
    assert_eq!(printed_hidden("word \\phf[note] word"), "word word\n");
    assert_eq!(printed_hidden("word \\abc[note] word"), "word word\n");
    assert_eq!(printed_hidden("\\phf{this\n\nthat}"), "this\n\nthat\n");
    assert_eq!(
        printed_hidden("\\phf see \\begin{itemize}\\item a\\end{itemize} here \\endphf"),
        "see\n\n• a\n\nhere\n"
    );
}

#[test]
fn hidden_a_comment_across_a_paragraph_break_leaves_the_break() {
    // The cut is part of the block structure, which the render option does
    // not change: the comment's pieces go, the two paragraphs stay.
    assert_eq!(printed_hidden("Some text \\phf[A comment.\n\nWith more.] More text"), "Some text\n\nMore text\n");
}

#[test]
fn hidden_nothing_of_the_annotations_own_is_staged() {
    let run = render_under(&registry(), "a \\phf{b} \\phf[c] d", &hidden());
    let top = lowered_run(&run);
    assert_eq!(top, ["\"a \"", "\"b\"", "\" \"", "\" d\""]);
}

// ---------------------------------------------------------------------------------------------
// in every place (test 19, the annotation half on text)
// ---------------------------------------------------------------------------------------------

#[test]
fn annotations_in_a_heading_a_list_item_and_a_footnote() {
    // The rule under a heading counts the initials as they are printed.
    assert_eq!(printed("\\section{A \\phf{b} \\phf[c]}"), "A [PhF] b [PhF: c]\n==================\n");
    assert_eq!(printed("\\section{\\phf{b}}"), "[PhF] b\n=======\n");
    let item = printed("\\begin{itemize}\\item a \\phf{b} \\abc[c]\\end{itemize}");
    assert!(item.contains("a [PhF] b [c]"), "{item}");
    let footnote = printed("x\\footnote{a \\phf{b} \\phf[c]}");
    assert!(footnote.contains("a [PhF] b [PhF: c]"), "{footnote}");
}

#[test]
fn an_annotation_cut_inside_a_list_item_and_inside_a_footnote() {
    assert_eq!(printed("\\begin{itemize}\\item a \\phf[b\n\nc] d\\end{itemize}"), "• a [PhF: b\n\n  c] d\n");
    assert_eq!(
        printed("x\\footnote{a \\phf{b\n\nc} d}"),
        "x¹\n\nFootnotes\n---------\n\n¹ a [PhF] b\n\n  c d\n"
    );
}

#[test]
fn an_annotation_in_text_inside_a_formula_is_the_text_modes() {
    // The command is declared in text mode, and `\text{…}` is text mode
    // inside a formula: the annotation there is parsed as one like any. The
    // text output prints a formula as its source, the annotation with it.
    let document = "a \\(x \\text{ \\phf{y} }\\) b";
    let registry = registry();
    let source = Arc::new(FlmSource::new(document));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::Blocks, &ParseOptions::default())
        .expect("a parse");
    let count = fragment.tree().root().descendants().filter(|node| node.ext().element.name == ANNOTATION).count();
    assert_eq!(count, 1);
    assert_eq!(printed(document), "a \\(x \\text{yPhF}\\) b\n");
}

#[test]
fn an_annotation_in_a_user_definitions_template() {
    let definition =
        Definition::macro_("note", vec![ArgDef::mandatory("text")]).text("\\phf[#1]");
    let registry =
        common::stdcore::registry(config().userdefs(UserdefsConfig::new().definition(definition)));
    let run = render_under(&registry, "word \\note{check} word", &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(common::lowering::text(&run), "word [PhF: check] word\n");
}

// ---------------------------------------------------------------------------------------------
// execution settings, and documents that must neither panic nor loop (test 20)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_text_is_the_same_sequentially_and_in_parallel() {
    let registry = registry();
    let texts = [
        "a \\phf{b} \\phf[c\n\nd] e",
        "\\phf see \\begin{itemize}\\item a\\end{itemize} here \\endphf",
        "\\abc[x \\phf{y\n\nz}]",
    ];
    let doc = common::pipeline::doc(&registry, &texts);
    let plan = plan(&registry, text::WORKFLOW, &Overrides::none());
    let sequential = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    let parallel = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::new());
    assert_eq!(common::lowering::text(&sequential), common::lowering::text(&parallel));
    assert_eq!(identifiers(&sequential), Vec::<String>::new());
}

#[test]
fn hard_documents_neither_panic_nor_loop_through_the_text_output() {
    let many_commands = "\\phf{x} \\abc[y] \\phf z\\endphf ".repeat(1000);
    let many_breaks = format!("\\phf[{}]", "a\n\n".repeat(1000));
    for document in [
        "\\phf{a \\phf{b\n\nc} d}",
        "\\phf{a \\phf[b\n\nc] d}",
        "\\phf passage",
        "\\phf",
        "\\phf[x",
        "\\endphf",
        many_commands.as_str(),
        many_breaks.as_str(),
    ] {
        let _ = render_under(&registry(), document, &Overrides::none());
        let _ = render_under(&registry(), document, &hidden());
    }
}

#[test]
fn an_unterminated_passage_is_shown_and_reported() {
    let run = render_under(&registry(), "a \\phf passage", &Overrides::none());
    assert_eq!(identifiers(&run), ["flm.annotations.unterminated"]);
    assert_eq!(common::lowering::text(&run), "a [PhF] passage\n");
    // The passage ends where the list around it ends; the list stays whole.
    let run = render_under(&registry(), "\\begin{itemize}\\item \\phf a\\end{itemize} b", &Overrides::none());
    assert_eq!(identifiers(&run), ["flm.annotations.unterminated"]);
    assert_eq!(common::lowering::text(&run), "• [PhF] a\n\nb\n");
}


#[test]
fn the_examples_of_the_language_guide_print_as_it_says() {
    assert_eq!(
        printed("The result \\phf{holds in every case}, as shown in\n\\phf[cite the lemma here] the appendix."),
        "The result [PhF] holds in every case, as shown in [PhF: cite the lemma here] the\nappendix.\n"
    );
}
