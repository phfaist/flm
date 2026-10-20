//! The text emitter (`flm_core::stdcore::backends::text`) over hand-built
//! presentation trees: every element of the presentation vocabulary against a
//! golden under `tests/goldens/text/`, a full document at width 40, subtree
//! splicing and its two conditions, the interpolated group inside verbatim
//! content, marks resolving to lowered nodes, the empty tree, the unbounded
//! width, and the order of several trees.

mod common;

use std::sync::Arc;

use serde_json::json;
use techy::core::node::{BuildId, NodeTree};
use techy::error::Severity;

use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::core::registry::FlmRegistry;
use flm_core::dump::diag::expect;
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::Flm;
use flm_core::stdcore::backends::pres::{
    self, HeadingPayload, BlockPayload, DecoMarkPayload, DecorationPayload, DocRole, ImagePayload,
    Length, LineAlign, LinePayload, LinesPayload, LinkPayload, ListPayload, MathDisplay, MathPassthroughPayload,
    MathTextPayload, ParPayload, SpanPayload, SpanStyle, SubtreePayload, UnsupportedPayload,
    VerbatimDisplay, VerbatimKind, VerbatimPayload,
};
use flm_core::stdcore::backends::lowering::LoweredAddr;
use flm_core::stdcore::backends::text::{DEPTH_LIMIT, Emitted, TextOutput};

use common::layout::assert_well_formed;
use common::lowering::{PresSite, emit_tree, emit_trees, golden_path, registry};

/// A registry to stage presentation nodes against.
fn vocabulary() -> Arc<FlmRegistry> {
    registry(Vec::new(), Vec::new())
}

/// A paragraph over one text.
fn par(site: &mut PresSite, text: &str) -> BuildId {
    let text = site.text(text);
    site.pres(pres::PAR, ParPayload::default(), vec![text])
}

fn heading(site: &mut PresSite, level: u8, text: &str) -> BuildId {
    let text = site.text(text);
    site.pres(pres::BLOCK_HEADING, HeadingPayload { name: flm_core::model::names::Name::from("section"), level, in_toc: true, anchor: None, number: None }, vec![text])
}

fn verbatim(site: &mut PresSite, display: VerbatimDisplay, text: &str) -> BuildId {
    let text = site.text(text);
    site.pres(
        pres::VERBATIM,
        VerbatimPayload { kind: VerbatimKind::Text, display, lang: None },
        vec![text],
    )
}

fn math(site: &mut PresSite, display: MathDisplay, env: Option<&str>, source: &str) -> BuildId {
    let text = site.text(source);
    let payload = MathPassthroughPayload { display, env: env.map(String::from) };
    site.pres(pres::MATH_PASSTHROUGH, payload, vec![text])
}

fn link(site: &mut PresSite, href: &str, text: &str) -> BuildId {
    let text = site.text(text);
    let payload = LinkPayload {
        href: href.into(),
        kind: "url".into(),
        doc_role: None,
        label: None,
        shows_target: false,
    };
    site.pres(pres::LINK, payload, vec![text])
}

fn decoration(site: &mut PresSite, element: &str, text: &str) -> BuildId {
    let text = site.text(text);
    site.pres(element, DecorationPayload, vec![text])
}

fn mark(site: &mut PresSite, slot: u32) -> BuildId {
    site.pres(pres::DECO_MARK, DecoMarkPayload { slot }, Vec::new())
}

/// The output of an emission that ran.
fn out(emitted: &Emitted) -> &TextOutput {
    emitted.output.as_ref().expect("the emission ran")
}

fn text_of(emitted: &Emitted) -> String {
    assert!(emitted.diags.is_empty(), "{}", emitted.diags.render_all());
    out(emitted).text.clone()
}

fn check(name: &str, emitted: &Emitted) {
    let text = text_of(emitted);
    assert_well_formed(&text);
    assert_text_golden(golden_path(name), &text);
}

// --- the full reference document -----------------------------------------------------------------

/// The expected rendering of the full reference document at width 40: a
/// level-1 heading, a paragraph with a reference and an emphasis, a theorem
/// paragraph with a run-in title and a QED, a two-item list whose items
/// wrap, and a verbatim block.
const DOCUMENT_AT_40: &str = "\
Introduction
============

See Section 1 and this emphasized bit
here in a paragraph that is long enough
to wrap at forty columns.

Theorem. Every finite thing is finite,
and this sentence is long enough to wrap
somewhere.\u{a0}□

1. First item, with enough words in it
   to wrap across two lines nicely.

2. Second item text that also wraps a
   little.

let x = 1;
let y = 2;
";

fn session_06_document(registry: &Arc<FlmRegistry>) -> Arc<NodeTree<Flm>> {
    let mut site = PresSite::new(registry);
    let title = heading(&mut site, 1, "Introduction");

    let see = site.text("See ");
    let reference = link(&mut site, "#x", "Section 1");
    let and = site.text(" and ");
    let emphasized = site.text("this emphasized bit");
    let emphasis = site.pres(
        pres::SPAN,
        SpanPayload { style: Some(SpanStyle::Emphasis), ..SpanPayload::default() },
        vec![emphasized],
    );
    let rest = site.text(" here in a paragraph that is long enough to wrap at forty columns.");
    let intro =
        site.pres(pres::PAR, ParPayload::default(), vec![see, reference, and, emphasis, rest]);

    let runin = decoration(&mut site, pres::DECO_RUNIN, "Theorem");
    let qed = decoration(&mut site, pres::DECO_FINAL, "□");
    let first_mark = mark(&mut site, 1);
    let statement = site
        .text("Every finite thing is finite, and this sentence is long enough to wrap somewhere.");
    let second_mark = mark(&mut site, 2);
    let theorem = site.pres_attached(
        pres::PAR,
        ParPayload::default(),
        vec![first_mark, statement, second_mark],
        vec![(pres::DECO_RUNIN, runin), (pres::DECO_FINAL, qed)],
    );

    let label_one = site.text("1.");
    let item_one =
        par(&mut site, "First item, with enough words in it to wrap across two lines nicely.");
    let label_two = site.text("2.");
    let item_two = par(&mut site, "Second item text that also wraps a little.");
    let list = site.list(
        ListPayload::default(),
        vec![(vec![label_one], vec![item_one]), (vec![label_two], vec![item_two])],
    );

    let code = verbatim(&mut site, VerbatimDisplay::Block, "let x = 1;\nlet y = 2;\n");
    site.finish(vec![title, intro, theorem, list, code])
}

#[test]
fn session_06s_document_prints_as_the_probe_expected_at_width_40() {
    let registry = vocabulary();
    let emitted = emit_tree(session_06_document(&registry), Some(40));
    let text = text_of(&emitted);
    assert_eq!(text, DOCUMENT_AT_40, "\n--- got ---\n{text}\n--- end ---");
    assert_text_golden(golden_path("emit-document.txt"), &text);
}

#[test]
fn width_changes_only_the_wrapping() {
    let registry = vocabulary();
    let wide = text_of(&emit_tree(session_06_document(&registry), Some(200)));
    let narrow = text_of(&emit_tree(session_06_document(&registry), Some(40)));
    let words = |text: &str| text.split_whitespace().map(String::from).collect::<Vec<_>>();
    assert_eq!(words(&wide), words(&narrow));
    assert!(wide.lines().count() < narrow.lines().count());
}

// --- block context -----------------------------------------------------------------------------

#[test]
fn paragraphs_fill_and_stand_one_blank_line_apart() {
    let mut site = PresSite::new(&vocabulary());
    let first = par(&mut site, "one two three four five six");
    let second = par(&mut site, "seven eight");
    let tree = site.finish(vec![first, second]);
    check("emit-paragraphs.txt", &emit_tree(tree, Some(12)));
}

#[test]
fn headings_are_ruled_by_level() {
    let mut site = PresSite::new(&vocabulary());
    let one = heading(&mut site, 1, "Wide heading");
    let two = heading(&mut site, 2, "Narrower");
    let three = heading(&mut site, 3, "Third");
    let four = heading(&mut site, 4, "Fourth");
    let tree = site.finish(vec![one, two, three, four]);
    check("emit-headings.txt", &emit_tree(tree, Some(40)));
}

#[test]
fn a_block_lays_out_its_body_as_blocks() {
    let mut site = PresSite::new(&vocabulary());
    let before = par(&mut site, "before");
    let first = par(&mut site, "inside one");
    let second = par(&mut site, "inside two");
    let payload = BlockPayload {
        role: Some("t.quote".into()),
        kinds: vec!["kind".into()],
        anchor: None,
        doc_role: None,
    };
    let block = site.pres(pres::BLOCK, payload, vec![first, second]);
    let after = par(&mut site, "after");
    let tree = site.finish(vec![before, block, after]);
    check("emit-block.txt", &emit_tree(tree, Some(40)));
}

#[test]
fn a_list_hangs_its_items_at_the_label_column() {
    let mut site = PresSite::new(&vocabulary());
    let a = site.text("a)");
    let a_item = par(&mut site, "the first item wraps onto a second line here");
    let iii = site.text("iii)");
    let iii_first = par(&mut site, "two paragraphs");
    let inner_label = site.text("-");
    let inner_item = par(&mut site, "a nested item that also wraps around");
    let inner = site.list(ListPayload::default(), vec![(vec![inner_label], vec![inner_item])]);
    let tree_list = site.list(
        ListPayload { kinds: vec!["enumerate".into()], ..ListPayload::default() },
        vec![(vec![a], vec![a_item]), (vec![iii], vec![iii_first, inner])],
    );
    let tree = site.finish(vec![tree_list]);
    check("emit-list.txt", &emit_tree(tree, Some(30)));
}

#[test]
fn lines_keep_their_breaks_and_indents() {
    let mut site = PresSite::new(&vocabulary());
    let mut lines = Vec::new();
    // No indentation, one em (two columns) and one tab (four columns by
    // default).
    for (indent, text) in [("0tab", "first line"), ("1em", "second, indented"), ("1tab", "third, more so")] {
        let content = site.text(text);
        let indent_left = Length::parse(indent).expect("a length");
        let payload =
            LinePayload { indent_left, indent_right: Length::default(), align: LineAlign::Left };
        lines.push(site.pres(pres::LINE, payload, vec![content]));
    }
    let block = site.pres(pres::LINES, LinesPayload, lines);
    let tree = site.finish(vec![block]);
    check("emit-lines.txt", &emit_tree(tree, Some(40)));
}

#[test]
fn a_verbatim_block_is_raw_and_indented_like_its_surroundings() {
    let mut site = PresSite::new(&vocabulary());
    let label = site.text("*");
    let lead = par(&mut site, "code:");
    let code = verbatim(
        &mut site,
        VerbatimDisplay::Block,
        "fn main() {\n    a  very   long line that would wrap\n}\n",
    );
    let list = site.list(ListPayload::default(), vec![(vec![label], vec![lead, code])]);
    let tree = site.finish(vec![list]);
    check("emit-verbatim.txt", &emit_tree(tree, Some(20)));
}

#[test]
fn math_prints_as_source_with_its_delimiters() {
    let mut site = PresSite::new(&vocabulary());
    let before = site.text("Let ");
    let inline = math(&mut site, MathDisplay::Inline, None, "x  =\n 1");
    let after = site.text(" hold.");
    let first = site.pres(pres::PAR, ParPayload::default(), vec![before, inline, after]);
    let display = math(&mut site, MathDisplay::Display, None, " y = 2 ");
    let env = math(&mut site, MathDisplay::Display, Some("align"), "\n a &= b \\\\\n c &= d\n");
    let tree = site.finish(vec![first, display, env]);
    check("emit-math.txt", &emit_tree(tree, Some(40)));
}

#[test]
fn content_inside_mathematics_is_written_as_its_command_around_its_content() {
    // A `math-text` node stands among the source of a `math-passthrough` body: the
    // emitter writes its command around its content, and nothing at all for
    // one with no content — the shape of a link target placed in a formula.
    let mut site = PresSite::new(&vocabulary());
    let head = site.text("x = ");
    let word = site.text("one half");
    let content = site.pres(
        pres::MATH_TEXT,
        MathTextPayload { command: Some(String::from("text")), anchor: None, at_tag: false },
        vec![word],
    );
    let tail = site.text(" + y");
    let payload = MathPassthroughPayload { display: MathDisplay::Inline, env: None };
    let inline = site.pres(pres::MATH_PASSTHROUGH, payload, vec![head, content, tail]);
    let before = site.text("Let ");
    let after = site.text(" hold.");
    let first = site.pres(pres::PAR, ParPayload::default(), vec![before, inline, after]);

    let anchor = site.pres(
        pres::MATH_TEXT,
        MathTextPayload { command: None, anchor: Some(String::from("eq-1")), at_tag: false },
        Vec::new(),
    );
    let line = site.text("a &= b");
    let payload =
        MathPassthroughPayload { display: MathDisplay::Display, env: Some(String::from("align")) };
    let env = site.pres(pres::MATH_PASSTHROUGH, payload, vec![anchor, line]);

    let tree = site.finish(vec![first, env]);
    check("emit-math-text.txt", &emit_tree(tree, Some(40)));
}

#[test]
fn content_at_a_tags_place_is_written_as_it_stands() {
    // The mark that makes the HTML backend write a pair of dollar signs around
    // a tag's content changes nothing here: this backend writes the formula's
    // source, and the content stands in it as an author would have typed it.
    let mut site = PresSite::new(&vocabulary());
    let line = site.text(" a = b \\tag*{");
    let number = site.text("(1)");
    let tag = site.pres(
        pres::MATH_TEXT,
        MathTextPayload { at_tag: true, ..MathTextPayload::default() },
        vec![number],
    );
    let close = site.text("}");
    let payload =
        MathPassthroughPayload { display: MathDisplay::Display, env: Some(String::from("equation")) };
    let env = site.pres(pres::MATH_PASSTHROUGH, payload, vec![line, tag, close]);
    let tree = site.finish(vec![env]);
    let out = text_of(&emit_tree(tree, Some(40)));
    assert!(out.contains("\\tag*{(1)}"), "{out}");
    assert!(!out.contains('$'), "{out}");
}

#[test]
fn display_math_inside_a_paragraph_takes_lines_of_its_own() {
    let mut site = PresSite::new(&vocabulary());
    let before = site.text("Consider");
    let display = math(&mut site, MathDisplay::Display, None, "z");
    let after = site.text("and then more text after it.");
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![before, display, after]);
    let tree = site.finish(vec![paragraph]);
    check("emit-math-in-par.txt", &emit_tree(tree, Some(40)));
}

// --- inline context ----------------------------------------------------------------------------

#[test]
fn inline_verbatim_is_one_atom_and_a_multiline_one_becomes_a_block() {
    let mut site = PresSite::new(&vocabulary());
    let before = site.text("see ");
    let code = verbatim(&mut site, VerbatimDisplay::Inline, "a  b   c");
    let between = site.text(" and ");
    let multiline = verbatim(&mut site, VerbatimDisplay::Inline, "p\nq");
    let after = site.text(" end");
    let paragraph =
        site.pres(pres::PAR, ParPayload::default(), vec![before, code, between, multiline, after]);
    let tree = site.finish(vec![paragraph]);
    check("emit-verbatim-inline.txt", &emit_tree(tree, Some(10)));
}

#[test]
fn links_show_external_targets_only() {
    let mut site = PresSite::new(&vocabulary());
    let internal = link(&mut site, "#sec", "Section 2");
    let space = site.text(" and ");
    let external = link(&mut site, "https://example.org/", "the site");
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![internal, space, external]);
    let tree = site.finish(vec![paragraph]);
    check("emit-links.txt", &emit_tree(tree, Some(80)));
}

#[test]
fn images_and_unsupported_elements_print_as_bracketed_notes() {
    let mut site = PresSite::new(&vocabulary());
    let with_alt = site.pres(
        pres::IMAGE,
        ImagePayload { src: "a.png".into(), alt: Some("A cat".into()), ..ImagePayload::default() },
        Vec::new(),
    );
    let space = site.text(" ");
    let without_alt = site.pres(
        pres::IMAGE,
        ImagePayload { src: "b.png".into(), ..ImagePayload::default() },
        Vec::new(),
    );
    let space_two = site.text(" ");
    let hole = site.pres(
        pres::UNSUPPORTED,
        UnsupportedPayload { element: "x.box".into(), message: "no rule".into() },
        Vec::new(),
    );
    let paragraph = site.pres(
        pres::PAR,
        ParPayload::default(),
        vec![with_alt, space, without_alt, space_two, hole],
    );
    let tree = site.finish(vec![paragraph]);
    check("emit-notes.txt", &emit_tree(tree, Some(80)));
}

#[test]
fn decorations_print_at_their_marks() {
    let mut site = PresSite::new(&vocabulary());
    let runin = decoration(&mut site, pres::DECO_RUNIN, "Lemma");
    let margin = decoration(&mut site, pres::DECO_MARGIN, "aside");
    let last = decoration(&mut site, pres::DECO_FINAL, "□");
    let first_mark = mark(&mut site, 1);
    let start = site.text("Text before the margin note");
    let second_mark = mark(&mut site, 2);
    let end = site.text("and after it.");
    let third_mark = mark(&mut site, 3);
    let paragraph = site.pres_attached(
        pres::PAR,
        ParPayload::default(),
        vec![first_mark, start, second_mark, end, third_mark],
        vec![(pres::DECO_RUNIN, runin), (pres::DECO_MARGIN, margin), (pres::DECO_FINAL, last)],
    );
    let tree = site.finish(vec![paragraph]);
    check("emit-decorations.txt", &emit_tree(tree, Some(30)));
}

#[test]
fn spans_are_transparent() {
    let mut site = PresSite::new(&vocabulary());
    let inner_text = site.text("inner");
    let inner = site.pres(
        pres::SPAN,
        SpanPayload {
            style: Some(SpanStyle::Bold),
            role: Some("r".into()),
            kinds: vec!["k".into()],
            anchor: Some("a".into()),
            doc_role: Some(DocRole::Footnote),
        },
        vec![inner_text],
    );
    let before = site.text("outer ");
    let outer = site.pres(pres::SPAN, SpanPayload::default(), vec![before, inner]);
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![outer]);
    let tree = site.finish(vec![paragraph]);
    assert_eq!(text_of(&emit_tree(tree, Some(80))), "outer inner\n");
}

/// A span of `style` holding `children`.
fn styled(site: &mut PresSite, style: SpanStyle, children: Vec<BuildId>) -> BuildId {
    site.pres(pres::SPAN, SpanPayload { style: Some(style), ..SpanPayload::default() }, children)
}

/// What one paragraph holding `children` prints as.
fn printed(site: PresSite, children: Vec<BuildId>) -> String {
    let mut site = site;
    let paragraph = site.pres(pres::PAR, ParPayload::default(), children);
    let tree = site.finish(vec![paragraph]);
    text_of(&emit_tree(tree, Some(80)))
}

#[test]
fn a_superscript_span_writes_its_text_in_the_superscript_forms() {
    let mut site = PresSite::new(&vocabulary());
    let lead = site.text("Note");
    let text = site.text("10 a iv (n+1) A Q q S");
    let raised = styled(&mut site, SpanStyle::Superscript, vec![text]);
    let tail = site.text(" ends.");
    // A character with no superscript form is written as it stands beside
    // its converted neighbors, and no marker is added.
    assert_eq!(printed(site, vec![lead, raised, tail]), "Note¹⁰ ᵃ ⁱᵛ ⁽ⁿ⁺¹⁾ ᴬ Q q S ends.\n");
}

#[test]
fn a_subscript_span_writes_its_text_in_the_subscript_forms() {
    let mut site = PresSite::new(&vocabulary());
    let lead = site.text("x");
    let text = site.text("1b");
    let lowered = styled(&mut site, SpanStyle::Subscript, vec![text]);
    assert_eq!(printed(site, vec![lead, lowered]), "x₁b\n");
}

#[test]
fn every_other_span_style_leaves_the_text_as_it_stands() {
    for style in SpanStyle::ALL {
        if matches!(style, SpanStyle::Superscript | SpanStyle::Subscript) {
            continue;
        }
        let mut site = PresSite::new(&vocabulary());
        let text = site.text("a1");
        let span = styled(&mut site, style, vec![text]);
        assert_eq!(printed(site, vec![span]), "a1\n", "{style}");
    }
}

#[test]
fn the_text_of_content_nested_in_a_superscript_span_is_converted_too() {
    // A link inside the span: its content is converted, its address and its
    // accessible name and document role are not (the address is the
    // emitter's own text; the other two are not written at all).
    let mut site = PresSite::new(&vocabulary());
    let number = site.text("2");
    let target = site.pres(
        pres::LINK,
        LinkPayload {
            href: String::from("https://example.org/n1"),
            kind: "xnote".into(),
            doc_role: Some(DocRole::NoteRef),
            label: Some(String::from("Footnote 2")),
            shows_target: false,
        },
        vec![number],
    );
    let mark = styled(&mut site, SpanStyle::Superscript, vec![target]);
    assert_eq!(printed(site, vec![mark]), "² <https://example.org/n1>\n");

    // An in-document link prints its content alone.
    let mut site = PresSite::new(&vocabulary());
    let lead = site.text("claim");
    let number = site.text("3");
    let target = site.pres(
        pres::LINK,
        LinkPayload {
            href: String::from("#fn-3"),
            kind: "xnote".into(),
            doc_role: Some(DocRole::NoteRef),
            label: Some(String::from("Footnote 3")),
            shows_target: false,
        },
        vec![number],
    );
    let mark = styled(&mut site, SpanStyle::Superscript, vec![target]);
    assert_eq!(printed(site, vec![lead, mark]), "claim³\n");

    // A nested span of another style inherits the conversion; a nested span
    // of the other of the two styles converts its own text its own way.
    let mut site = PresSite::new(&vocabulary());
    let bold_text = site.text("ab");
    let bold = styled(&mut site, SpanStyle::Bold, vec![bold_text]);
    let low_text = site.text("2");
    let low = styled(&mut site, SpanStyle::Subscript, vec![low_text]);
    let after = site.text("n");
    let raised = styled(&mut site, SpanStyle::Superscript, vec![bold, low, after]);
    assert_eq!(printed(site, vec![raised]), "ᵃᵇ₂ⁿ\n");

    // Mathematics inside the span is its source, not text of the document.
    let mut site = PresSite::new(&vocabulary());
    let formula = math(&mut site, MathDisplay::Inline, None, "n+1");
    let raised = styled(&mut site, SpanStyle::Superscript, vec![formula]);
    assert_eq!(printed(site, vec![raised]), "\\(n+1\\)\n");

    // Verbatim content inside the span is its source too.
    let mut site = PresSite::new(&vocabulary());
    let code = verbatim(&mut site, VerbatimDisplay::Inline, "x1");
    let raised = styled(&mut site, SpanStyle::Superscript, vec![code]);
    assert_eq!(printed(site, vec![raised]), "x1\n");

    // A block standing inside the span is not converted: it takes lines of
    // its own, beside the converted text of the span.
    let mut site = PresSite::new(&vocabulary());
    let lead = site.text("1");
    let block = par(&mut site, "1");
    let raised = styled(&mut site, SpanStyle::Superscript, vec![lead, block]);
    assert_eq!(printed(site, vec![raised]), "¹\n1\n");
}

/// What a list of one entry per label, each item the paragraph `x`, prints as.
fn list_printed(site: PresSite, labels: Vec<BuildId>) -> String {
    let mut site = site;
    let entries: Vec<(Vec<BuildId>, Vec<BuildId>)> =
        labels.into_iter().map(|label| (vec![label], vec![par(&mut site, "x")])).collect();
    let list = site.list(ListPayload::default(), entries);
    let tree = site.finish(vec![list]);
    text_of(&emit_tree(tree, Some(80)))
}

#[test]
fn a_list_label_writes_a_superscript_or_subscript_span_in_its_forms() {
    // A label is printed as one piece of text; a superscript or subscript
    // span in it converts its characters as it does in running text.
    let mut site = PresSite::new(&vocabulary());
    let one = site.text("12");
    let raised = styled(&mut site, SpanStyle::Superscript, vec![one]);
    let lead = site.text("a");
    let two = site.text("2");
    let lowered = styled(&mut site, SpanStyle::Subscript, vec![two]);
    let low = site.pres(pres::SPAN, SpanPayload::default(), vec![lead, lowered]);
    // Nested and mixed: a bold span inherits the superscript conversion, a
    // subscript span inside converts its own text its own way, and a
    // character with no form is written as it stands.
    let bold_text = site.text("n");
    let bold = styled(&mut site, SpanStyle::Bold, vec![bold_text]);
    let inner_text = site.text("3");
    let inner = styled(&mut site, SpanStyle::Subscript, vec![inner_text]);
    let plain = site.text("Q)");
    let mixed = styled(&mut site, SpanStyle::Superscript, vec![bold, inner, plain]);
    assert_eq!(
        list_printed(site, vec![raised, low, mixed]),
        "\u{b9}\u{b2}   x\n\na\u{2082}   x\n\n\u{207f}\u{2083}Q\u{207e} x\n"
    );
}

#[test]
fn a_superscript_span_with_empty_or_no_text_prints_nothing() {
    let mut site = PresSite::new(&vocabulary());
    let lead = site.text("a");
    let empty = site.text("");
    let raised = styled(&mut site, SpanStyle::Superscript, vec![empty]);
    let childless = styled(&mut site, SpanStyle::Subscript, Vec::new());
    let tail = site.text("b");
    assert_eq!(printed(site, vec![lead, raised, childless, tail]), "ab\n");
}

#[test]
fn the_unicodesuperscript_format_writes_the_shared_tables_digits() {
    use flm_core::stdcore::backends::text::superscript::{subscript_form, superscript_form};
    use flm_core::stdcore::features::numbering::NumberFormat;

    assert_eq!(NumberFormat::Superscript.text(17), "¹⁷");
    assert_eq!(NumberFormat::Subscript.text(17), "₁₇");
    for digit in '0'..='9' {
        let number = i64::from(digit.to_digit(10).expect("a digit")) + 10;
        let written: Vec<char> = NumberFormat::Superscript.text(number).chars().collect();
        assert_eq!(written, [superscript_form('1'), superscript_form(digit)].map(Option::unwrap));
        let written: Vec<char> = NumberFormat::Subscript.text(number).chars().collect();
        assert_eq!(written, [subscript_form('1'), subscript_form(digit)].map(Option::unwrap));
    }
    assert_eq!(NumberFormat::Superscript.text(0), "");
}

#[test]
fn an_interpolated_group_inside_verbatim_prints_without_its_delimiters() {
    let mut site = PresSite::new(&vocabulary());
    let a = site.text("a");
    let b = site.text("b");
    let group = site.group(vec![b]);
    let c = site.text("c");
    let code = site.pres(
        pres::VERBATIM,
        VerbatimPayload { kind: VerbatimKind::Text, display: VerbatimDisplay::Block, lang: None },
        vec![a, group, c],
    );
    let tree = site.finish(vec![code]);
    assert_eq!(text_of(&emit_tree(tree, Some(80))), "abc\n");
}

// --- subtrees ----------------------------------------------------------------------------------

#[test]
fn a_subtree_splices_the_placed_tree_where_it_stands() {
    let registry = vocabulary();
    let mut outer = PresSite::new(&registry);
    let before = par(&mut outer, "before");
    let placed = outer.pres(pres::SUBTREE, SubtreePayload { tree: TreeId::tree(1) }, Vec::new());
    let after = par(&mut outer, "after");
    let outer_tree = outer.finish(vec![before, placed, after]);
    let mut inner = PresSite::new(&registry);
    let inside = par(&mut inner, "inside");
    let inner_tree = inner.finish(vec![inside]);
    check("emit-subtree.txt", &emit_trees(vec![outer_tree, inner_tree], Some(40)));
}

#[test]
fn a_missing_subtree_is_diagnosed_and_prints_nothing() {
    let mut site = PresSite::new(&vocabulary());
    let before = par(&mut site, "before");
    let placed = site.pres(pres::SUBTREE, SubtreePayload { tree: TreeId::tree(5) }, Vec::new());
    let tree = site.finish(vec![before, placed]);
    let emitted = emit_tree(tree, Some(40));
    expect(emitted.diags.iter(), "flm.text.missing-subtree")
        .severity(Severity::Error)
        .data(json!({ "tree": "tree#5" }))
        .once();
    assert_eq!(out(&emitted).text, "before\n");
}

#[test]
fn a_subtree_cycle_is_diagnosed_and_prints_nothing() {
    let mut site = PresSite::new(&vocabulary());
    let before = par(&mut site, "before");
    let placed = site.pres(pres::SUBTREE, SubtreePayload { tree: TreeId::tree(0) }, Vec::new());
    let tree = site.finish(vec![before, placed]);
    let emitted = emit_tree(tree, Some(40));
    expect(emitted.diags.iter(), "flm.text.subtree-cycle").data(json!({ "tree": "tree#0" })).once();
    assert_eq!(out(&emitted).text, "before\n");
}

// --- the depth limit ------------------------------------------------------------------------

/// A paragraph holding `depth` nested spans around one word.
fn nested_spans(registry: &Arc<FlmRegistry>, depth: usize) -> Arc<NodeTree<Flm>> {
    let mut site = PresSite::new(registry);
    let mut node = site.text("x");
    for _ in 0..depth {
        node = site.pres(pres::SPAN, SpanPayload::default(), vec![node]);
    }
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![node]);
    site.finish(vec![paragraph])
}

#[test]
fn a_tree_nesting_past_the_depth_limit_is_refused_with_its_condition() {
    let emitted = emit_tree(nested_spans(&vocabulary(), DEPTH_LIMIT + 8), Some(40));
    expect(emitted.diags.iter(), "flm.text.nesting-too-deep").severity(Severity::Error).once();
    assert_eq!(out(&emitted).text, "", "the tree prints nothing");
}

#[test]
fn a_tree_nesting_under_the_depth_limit_prints() {
    // Deep enough to prove the per-level estimate holds on the worker stack.
    let emitted = emit_tree(nested_spans(&vocabulary(), DEPTH_LIMIT - 8), Some(40));
    assert_eq!(text_of(&emitted), "x\n");
}

// --- marks, edges, several trees ---------------------------------------------------------------

#[test]
fn marks_resolve_to_the_lowered_nodes() {
    let mut site = PresSite::new(&vocabulary());
    let first = par(&mut site, "first");
    let second = par(&mut site, "second");
    let tree = site.finish(vec![first, second]);
    let second_id = tree
        .iter_storage_order()
        .filter(|node| element_of(node).name == pres::PAR)
        .nth(1)
        .expect("the second par")
        .id();
    let emitted = emit_tree(Arc::clone(&tree), Some(40));
    let marks: Vec<_> =
        out(&emitted).marks_at(LoweredAddr::new(TreeId::tree(0), second_id)).collect();
    assert_eq!(marks.len(), 1);
    assert_eq!(out(&emitted).slice(marks[0]), "second");
    assert_eq!((marks[0].start.line, marks[0].start.col), (2, 0));
}

#[test]
fn an_empty_tree_prints_nothing() {
    let site = PresSite::new(&vocabulary());
    let tree = site.finish(Vec::new());
    let emitted = emit_tree(tree, Some(40));
    assert_eq!(out(&emitted).text, "");
    assert!(out(&emitted).marks.is_empty());
}

#[test]
fn no_width_never_wraps() {
    let mut site = PresSite::new(&vocabulary());
    let long = par(&mut site, "word ".repeat(50).trim_end());
    let tree = site.finish(vec![long]);
    let text = text_of(&emit_tree(tree, None));
    assert_eq!(text.lines().count(), 1);
}

#[test]
fn trees_print_in_order_one_blank_line_apart() {
    let registry = vocabulary();
    let mut first = PresSite::new(&registry);
    let one = par(&mut first, "one");
    let first_tree = first.finish(vec![one]);
    let mut second = PresSite::new(&registry);
    let two = par(&mut second, "two");
    let second_tree = second.finish(vec![two]);
    assert_eq!(text_of(&emit_trees(vec![first_tree, second_tree], Some(40))), "one\n\ntwo\n");
}
