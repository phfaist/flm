//! The HTML emitter (`flm_core::stdcore::backends::html`) over hand-built
//! presentation trees: one golden per area of the presentation vocabulary
//! under `tests/goldens/html/`, the contract tests that keep the conventions
//! table, the emitter and the printed table in step, the source-range option,
//! the requirement set of a tree, the refusals (an address, a name), a tree
//! placed in another tree, and the depth limit.
//!
//! Nothing here parses FLM source: every tree is built by hand, so that a
//! change in a lowering rule cannot move a golden of the emitter.

mod common;

use std::sync::Arc;

use techy::core::node::{BuildId, NodeTree};
use techy::error::Severity;

use flm_core::core::registry::FlmRegistry;
use flm_core::dump::diag::expect;
use flm_core::lang::{Flm, PresPayload};
use flm_core::model::addr::TreeId;
use flm_core::model::names::Name;
use flm_core::stdcore::backends::html::conventions::{
    CONVENTIONS, SCRIPT_MATHJAX, STYLE_ATTRIBUTE, Written, row,
};
use flm_core::stdcore::backends::html::{
    DEPTH_LIMIT, EmittedTree, HtmlRenderOptions, HtmlSegment, HtmlTagPayload, Markup,
    TAG, markdown_table,
};
use flm_core::stdcore::backends::pres::{
    self, HeadingPayload, BlockPayload, DecoMarkPayload, DecorationPayload, DocRole,
    ImageDisplay, ImagePayload, Length, LineAlign, LinePayload, LinesPayload, LinkPayload, ListPayload,
    MathDisplay,
    MathPassthroughPayload, MathTextPayload, ParPayload, SpanPayload, SpanStyle, SrcsetCandidate,
    SubtreePayload, UnknownKind, UnknownPayload, UnsupportedPayload, VerbatimDisplay,
    VerbatimKind, VerbatimPayload,
};

use common::html::{assert_golden, emit, emit_with, html};
use common::lowering::PresSite;
use common::pipeline;
use common::stdcore as std_package;

/// A registry holding the presentation vocabulary and the HTML backend, to
/// stage presentation nodes against: the `lowering` feature for the
/// vocabulary, the `html` feature for the backend and the construct element,
/// `check` for the collection the workflow demands, and the three features the
/// shared lowering rules require of a registry (`blocks`, `userdefs`, `math`).
fn vocabulary() -> Arc<FlmRegistry> {
    use flm_core::stdcore::features::blocks::Blocks;
    use flm_core::stdcore::features::math;
    use flm_core::stdcore::features::userdefs::{Userdefs, UserdefsConfig};

    pipeline::registry(vec![
        flm_core::stdcore::backends::lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        flm_core::stdcore::backends::html::feature(),
        flm_core::stdcore::check_feature(),
        Arc::new(Blocks::default()),
        Arc::new(Userdefs::new(UserdefsConfig::new())),
        math::feature(),
        flm_core::math_passthrough::feature(),
    ])
}

/// A site over [`vocabulary`].
fn site() -> PresSite {
    PresSite::new(&vocabulary())
}

/// The HTML one hand-built tree is written as.
fn written(tree: Arc<NodeTree<Flm>>) -> String {
    html(&emit(tree))
}

/// A paragraph over one text.
fn par(site: &mut PresSite, text: &str) -> BuildId {
    let text = site.text(text);
    site.pres(pres::PAR, ParPayload::default(), vec![text])
}

/// A span of `style` over one text.
fn span(site: &mut PresSite, style: Option<SpanStyle>, text: &str) -> BuildId {
    let text = site.text(text);
    site.pres(pres::SPAN, SpanPayload { style, ..SpanPayload::default() }, vec![text])
}

/// A name.
fn name(text: &str) -> Name {
    Name::from(text)
}

// --- paragraphs and inline roots --------------------------------------------------------

#[test]
fn paragraphs_are_p_elements_and_an_anchor_is_the_id() {
    let mut site = site();
    let first = par(&mut site, "One.");
    let anchored = {
        let text = site.text("Two.");
        site.pres(
            pres::PAR,
            ParPayload { anchor: Some(String::from("par-two")) },
            vec![text],
        )
    };
    let tree = site.finish(vec![first, anchored]);
    assert_golden("emit-paragraphs.html", &written(tree));
}

#[test]
fn a_root_of_inline_content_is_written_bare_with_no_paragraph_invented() {
    // What a fragment parsed with the inline layout reaches the emitter as:
    // no `flm.pres.par` node, so no `<p>` in the output.
    let mut site = site();
    let lead = site.text("A ");
    let emphasis = span(&mut site, Some(SpanStyle::Emphasis), "title");
    let tail = site.text(" here.");
    let tree = site.finish(vec![lead, emphasis, tail]);
    let out = written(tree);
    assert!(!out.contains("<p"), "{out}");
    assert_golden("emit-inline-root.html", &out);
}

#[test]
fn a_paragraph_holding_only_an_empty_element_is_still_written() {
    // An element is something shown even when it holds no text: a
    // stylesheet may draw it, so its paragraph stays.
    let mut site = site();
    let empty = site.pres(pres::SPAN, SpanPayload::default(), vec![]);
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![empty]);
    let tree = site.finish(vec![paragraph]);
    let out = written(tree);
    assert_eq!(out.trim(), "<p class=\"flm-pres-par\"><span class=\"flm-pres-span\"></span></p>", "{out}");
}

#[test]
fn an_empty_paragraph_carrying_an_anchor_is_still_written() {
    // A link may lead to it, so the paragraph stays for its identifier.
    let mut site = site();
    let space = site.text(" ");
    let anchored =
        site.pres(pres::PAR, ParPayload { anchor: Some(String::from("here")) }, vec![space]);
    let empty = site.pres(pres::PAR, ParPayload::default(), vec![]);
    let tree = site.finish(vec![anchored, empty]);
    let out = written(tree);
    assert!(out.contains("<p class=\"flm-pres-par\" id=\"here\">"), "{out}");
    assert_eq!(out.matches("<p").count(), 1, "{out}");
}

// --- blocks, headings, spans -------------------------------------------------------------

#[test]
fn a_block_carries_its_role_as_a_class_and_its_kinds_as_an_attribute() {
    let mut site = site();
    let inner = par(&mut site, "Inside.");
    let block = site.pres(
        pres::BLOCK,
        BlockPayload {
            role: Some(name("flm.theorem")),
            kinds: vec![name("numbered"), name("proof")],
            anchor: Some(String::from("thm-1")),
            doc_role: None,
        },
        vec![inner],
    );
    let plain = {
        let inner = par(&mut site, "Plain.");
        site.pres(pres::BLOCK, BlockPayload::default(), vec![inner])
    };
    let tree = site.finish(vec![block, plain]);
    assert_golden("emit-blocks.html", &written(tree));
}

#[test]
fn a_block_of_the_role_flm_toc_is_a_nav_carrying_the_document_role_doc_toc() {
    // The tag table's row for a table of contents, and its document role.
    let mut site = site();
    let inner = par(&mut site, "Entries.");
    let block = site.pres(
        pres::BLOCK,
        BlockPayload {
            role: Some(name("flm.toc")),
            kinds: vec![name("toc")],
            anchor: None,
            doc_role: Some(DocRole::Toc),
        },
        vec![inner],
    );
    let tree = site.finish(vec![block]);
    let out = written(tree);
    assert!(
        out.starts_with("<nav class=\"flm-pres-block flm-toc\" role=\"doc-toc\" data-flm-kind=\"toc\">"),
        "{out}"
    );
    assert_golden("emit-toc-block.html", &out);
}

#[test]
fn a_role_or_a_kind_that_is_not_one_token_is_left_out_and_reported() {
    // A class and a `data-flm-kind` value are both token lists separated by
    // whitespace, so a name holding whitespace would be read as several
    // tokens. Such a name is a lowering rule's, never a document's, and the
    // library's name type accepts any text, so the emitter checks it here.
    let mut site = site();
    let inner = par(&mut site, "Inside.");
    let block = site.pres(
        pres::BLOCK,
        BlockPayload {
            role: Some(name("flm.a b")),
            kinds: vec![name("numbered"), name("two words"), name("")],
            anchor: None,
            doc_role: None,
        },
        vec![inner],
    );
    let tree = site.finish(vec![block]);
    let emitted = emit(tree);
    expect(emitted.diags.iter(), "flm.html.invalid-name").severity(Severity::Error).count(3);
    let out = html(&emitted);
    // The element, its own class and the kinds that are one token are written;
    // the three refused names are not in the output at all.
    assert!(out.contains("<div class=\"flm-pres-block\" data-flm-kind=\"numbered\">"), "{out}");
    assert!(!out.contains("flm-a"), "{out}");
    assert!(!out.contains("two words"), "{out}");
    // The content is never lost over a name the emitter could not write.
    assert!(out.contains("Inside."), "{out}");
}

#[test]
fn a_heading_takes_its_tag_from_its_level_and_keeps_the_true_level_above_six() {
    let mut site = site();
    let mut blocks = Vec::new();
    for level in [1u8, 2, 3, 4, 5, 6, 7, 9] {
        let text = site.text(&format!("Level {level}"));
        blocks.push(site.pres(
            pres::BLOCK_HEADING,
            HeadingPayload {
                name: name("section"),
                level,
                in_toc: true,
                anchor: Some(format!("h-{level}")),
                number: None,
            },
            vec![text],
        ));
    }
    let tree = site.finish(blocks);
    assert_golden("emit-headings.html", &written(tree));
}

#[test]
fn every_span_style_has_its_tag_and_its_attribute() {
    let mut site = site();
    let mut nodes = Vec::new();
    for style in SpanStyle::ALL {
        nodes.push(span(&mut site, Some(style), style.as_str()));
    }
    nodes.push(span(&mut site, None, "no style"));
    let roled = {
        let text = site.text("a term");
        site.pres(
            pres::SPAN,
            SpanPayload {
                style: Some(SpanStyle::Bold),
                role: Some(name("flm.defterm")),
                kinds: vec![name("primary")],
                anchor: Some(String::from("term-1")),
                doc_role: None,
            },
            vec![text],
        )
    };
    nodes.push(roled);
    let paragraph = site.pres(pres::PAR, ParPayload::default(), nodes);
    let tree = site.finish(vec![paragraph]);
    assert_golden("emit-spans.html", &written(tree));
}

// --- decorations ---------------------------------------------------------------------------

#[test]
fn a_decoration_of_every_kind_is_written_at_its_mark() {
    let mut site = site();
    let runin = {
        let text = site.text("Theorem 1.");
        site.pres(pres::DECO_RUNIN, DecorationPayload, vec![text])
    };
    let closing = {
        let text = site.text("□");
        site.pres(pres::DECO_FINAL, DecorationPayload, vec![text])
    };
    let margin = {
        let text = site.text("a note");
        site.pres(pres::DECO_MARGIN, DecorationPayload, vec![text])
    };
    let lead = site.pres(pres::DECO_MARK, DecoMarkPayload { slot: 1 }, Vec::new());
    let middle = site.text("The statement.");
    let beside = site.pres(pres::DECO_MARK, DecoMarkPayload { slot: 3 }, Vec::new());
    let trail = site.pres(pres::DECO_MARK, DecoMarkPayload { slot: 2 }, Vec::new());
    let paragraph = site.pres_attached(
        pres::PAR,
        ParPayload::default(),
        vec![lead, middle, beside, trail],
        vec![
            (pres::DECO_RUNIN, runin),
            (pres::DECO_FINAL, closing),
            (pres::DECO_MARGIN, margin),
        ],
    );
    // A block carrying a margin decoration: the mark stands at the start of
    // the block's body, before its first paragraph.
    let block_note = {
        let text = site.text("a block note");
        site.pres(pres::DECO_MARGIN, DecorationPayload, vec![text])
    };
    let block_mark = site.pres(pres::DECO_MARK, DecoMarkPayload { slot: 1 }, Vec::new());
    let inner = {
        let text = site.text("Inside the block.");
        site.pres(pres::PAR, ParPayload::default(), vec![text])
    };
    let block = site.pres_attached(
        pres::BLOCK,
        BlockPayload::default(),
        vec![block_mark, inner],
        vec![(pres::DECO_MARGIN, block_note)],
    );
    let tree = site.finish(vec![paragraph, block]);
    let out = written(tree);
    // The margin decoration is a `<span>` inside the paragraph, where its
    // mark stands, and at the start of the block's `<div>`: never an
    // `<aside>`, which a `<p>` cannot hold.
    assert!(!out.contains("<aside"), "{out}");
    assert!(
        out.contains("The statement.<span class=\"flm-pres-deco-margin\">a note</span>"),
        "{out}"
    );
    let block_at = out.find("<div").expect("the block is written");
    let note_at = out.find("a block note").expect("the block's note is written");
    let inner_at = out.find("Inside the block.").expect("the block's paragraph is written");
    assert!(block_at < note_at && note_at < inner_at, "{out}");
    assert_golden("emit-decorations.html", &out);
}

#[test]
fn a_decoration_met_with_no_mark_standing_for_it_is_written_where_it_stands() {
    // An inline-layout fragment has no paragraph for the driver to attach a
    // decoration to, so the decoration stands in the run itself.
    let mut site = site();
    let text = site.text("Note.");
    let runin = site.pres(pres::DECO_RUNIN, DecorationPayload, vec![text]);
    let rest = site.text(" The statement.");
    let tree = site.finish(vec![runin, rest]);
    assert_eq!(
        written(tree),
        "<span class=\"flm-pres-deco-runin\">Note.</span> The statement."
    );
}

// --- links and images ------------------------------------------------------------------------

#[test]
fn a_link_carries_its_kind_and_an_address_the_policy_refuses_is_left_out() {
    let mut site = site();
    // The target the first link points at, so that the golden is a fragment
    // that stands on its own: every address of the form `#x` in it is
    // answered by an element carrying `id="x"`.
    let target = {
        let text = site.text("Theorem 1.");
        site.pres(
            pres::SPAN,
            SpanPayload { anchor: Some(String::from("thm-1")), ..SpanPayload::default() },
            vec![text],
        )
    };
    let reference = {
        let text = site.text("Theorem 1");
        site.pres(
            pres::LINK,
            LinkPayload {
                href: String::from("#thm-1"),
                kind: name("ref"),
                doc_role: None,
                label: None,
                shows_target: false,
            },
            vec![text],
        )
    };
    let external = {
        let text = site.text("the page");
        site.pres(
            pres::LINK,
            LinkPayload {
                href: String::from("https://example.org/a?b=1&c=2"),
                kind: name("url"),
                doc_role: None,
                label: None,
                shows_target: false,
            },
            vec![text],
        )
    };
    let refused = {
        let text = site.text("do not run this");
        site.pres(
            pres::LINK,
            LinkPayload {
                href: String::from("javascript:alert(1)"),
                kind: name("url"),
                doc_role: None,
                label: None,
                shows_target: false,
            },
            vec![text],
        )
    };
    let paragraph =
        site.pres(pres::PAR, ParPayload::default(), vec![target, reference, external, refused]);
    let tree = site.finish(vec![paragraph]);
    let emitted = emit(tree);
    expect(emitted.diags.iter(), "flm.html.refused-address").severity(Severity::Error).once();
    assert!(emitted.diags.render_all().contains("javascript"), "{}", emitted.diags.render_all());
    let out = html(&emitted);
    // The content stays: presentation may be dropped, content never is.
    assert!(out.contains("do not run this"), "{out}");
    assert!(!out.contains("javascript"), "{out}");
    // The fragment answers its own internal address.
    assert!(out.contains("id=\"thm-1\""), "{out}");
    assert_golden("emit-links.html", &out);
}

#[test]
fn document_roles_and_a_links_accessible_name_are_written_as_role_and_aria_label() {
    // The shapes a list of notes and its marks take: a block of role
    // `doc-endnotes`, a note shown in place as a span of role `doc-footnote`,
    // a link of role `doc-noteref` with an accessible name, and a link back
    // whose name holds the two characters an attribute value must escape.
    let mut site = site();
    let mark = {
        let text = site.text("1");
        let link = site.pres(
            pres::LINK,
            LinkPayload {
                href: String::from("#fn-1"),
                kind: name("xnote"),
                doc_role: Some(DocRole::NoteRef),
                label: Some(String::from("Footnote 1")),
                shows_target: false,
            },
            vec![text],
        );
        site.pres(
            pres::SPAN,
            SpanPayload {
                style: Some(SpanStyle::Superscript),
                anchor: Some(String::from("fnref-1")),
                ..SpanPayload::default()
            },
            vec![link],
        )
    };
    let in_place = {
        let text = site.text("An aside.");
        site.pres(
            pres::SPAN,
            SpanPayload {
                kinds: vec![name("xnotes")],
                doc_role: Some(DocRole::Footnote),
                ..SpanPayload::default()
            },
            vec![text],
        )
    };
    let lead = site.text("Claim.");
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![lead, mark, in_place]);
    let back = {
        let text = site.text("\u{21a9}");
        site.pres(
            pres::LINK,
            LinkPayload {
                href: String::from("#fnref-1"),
                kind: name("xnote-back"),
                doc_role: Some(DocRole::BackLink),
                label: Some(String::from("Back to \"1\" <here>")),
                shows_target: false,
            },
            vec![text],
        )
    };
    let entry = {
        let text = site.text("The note. ");
        site.pres(pres::PAR, ParPayload { anchor: Some(String::from("fn-1")) }, vec![text, back])
    };
    let list = site.pres(
        pres::BLOCK,
        BlockPayload {
            kinds: vec![name("xnotes")],
            doc_role: Some(DocRole::Endnotes),
            ..BlockPayload::default()
        },
        vec![entry],
    );
    let tree = site.finish(vec![paragraph, list]);
    let emitted = emit(tree);
    assert!(emitted.diags.is_empty(), "{}", emitted.diags.render_all());
    let out = html(&emitted);
    assert!(
        out.contains(
            "<a class=\"flm-pres-link\" href=\"#fn-1\" role=\"doc-noteref\" \
             aria-label=\"Footnote 1\" data-flm-kind=\"xnote\">1</a>"
        ),
        "{out}"
    );
    assert!(
        out.contains(
            "<span class=\"flm-pres-span\" role=\"doc-footnote\" data-flm-kind=\"xnotes\">"
        ),
        "{out}"
    );
    assert!(
        out.contains(
            "<div class=\"flm-pres-block\" role=\"doc-endnotes\" data-flm-kind=\"xnotes\">"
        ),
        "{out}"
    );
    // The accessible name is escaped as every attribute value is; the
    // content stays what a reader sees.
    assert!(
        out.contains("role=\"doc-backlink\" aria-label=\"Back to &quot;1&quot; &lt;here&gt;\""),
        "{out}"
    );
    assert!(out.contains("\u{21a9}</a>"), "{out}");
    // No payload without a document role or a name writes either attribute.
    let mut other = self::site();
    let plain = span(&mut other, Some(SpanStyle::Bold), "bold");
    let out = written(other.finish(vec![plain]));
    assert!(!out.contains("role=") && !out.contains("aria-label"), "{out}");
}

#[test]
fn an_image_writes_its_sources_and_leaves_out_the_ones_the_policy_refuses() {
    let mut site = site();
    let plain = site.pres(
        pres::IMAGE,
        ImagePayload {
            src: String::from("figures/plot.png"),
            display: ImageDisplay::Inline,
            alt: Some(String::from("a \"plot\" of x & y")),
            width: Some(Length::parse("5cm").unwrap()),
            height: Some(Length::parse("2in").unwrap()),
            srcset: Vec::new(),
            anchor: Some(String::from("fig-1")),
        },
        Vec::new(),
    );
    let responsive = site.pres(
        pres::IMAGE,
        ImagePayload {
            src: String::from("figures/plot.png"),
            display: ImageDisplay::Inline,
            alt: None,
            width: Some(Length::parse("300pt").unwrap()),
            height: None,
            srcset: vec![
                SrcsetCandidate {
                    src: String::from("figures/plot@2x.png"),
                    descriptor: String::from("2x"),
                },
                SrcsetCandidate {
                    src: String::from("javascript:alert(1)"),
                    descriptor: String::from("3x"),
                },
            ],
            anchor: None,
        },
        Vec::new(),
    );
    // A height alone is written alone, with no ratio; so is a pair of lengths
    // with no fixed size, which have no ratio to write.
    let tall = site.pres(
        pres::IMAGE,
        ImagePayload {
            src: String::from("figures/tall.png"),
            height: Some(Length::parse("3cm").unwrap()),
            ..ImagePayload::default()
        },
        Vec::new(),
    );
    let relative = site.pres(
        pres::IMAGE,
        ImagePayload {
            src: String::from("figures/wide.png"),
            width: Some(Length::parse("20em").unwrap()),
            height: Some(Length::parse("5em").unwrap()),
            ..ImagePayload::default()
        },
        Vec::new(),
    );
    let paragraph =
        site.pres(pres::PAR, ParPayload::default(), vec![plain, responsive, tall, relative]);
    let tree = site.finish(vec![paragraph]);
    let emitted = emit(tree);
    expect(emitted.diags.iter(), "flm.html.refused-address").severity(Severity::Error).once();
    let out = html(&emitted);
    assert!(
        out.contains(
            "style=\"--flm-element-width: 5cm; --flm-element-height: 2in; \
             --flm-element-aspect-ratio: 141.73228 / 144\""
        ),
        "{out}"
    );
    assert!(out.contains("style=\"--flm-element-width: 300pt\""), "{out}");
    assert!(out.contains("style=\"--flm-element-height: 3cm\""), "{out}");
    assert!(out.contains("style=\"--flm-element-width: 20em; --flm-element-height: 5em\""), "{out}");
    assert!(!out.contains(" width=") && !out.contains(" height="), "{out}");
    assert_golden("emit-image.html", &out);
}

// --- lists and lines ---------------------------------------------------------------------------

#[test]
fn a_list_takes_its_tag_from_its_kind_and_writes_a_rich_label_of_its_own() {
    // The tag comes from the structural kind token alone: the name after it
    // is a token for a stylesheet, and a list holding no structural token —
    // the last one, whatever its name says — is a `<ul>`. Each entry's anchor
    // is the identifier of the element that begins the entry.
    let mut site = site();
    let mut lists = Vec::new();
    for kinds in [
        vec![pres::LIST_NUMBERED, "enumerate"],
        vec![pres::LIST_UNNUMBERED, "itemize"],
        vec![pres::LIST_TERMS, "description"],
        vec!["enumerate"],
    ] {
        let mut entries = Vec::new();
        for (index, (label, item)) in [("1.", "First."), ("2.", "Second.")].iter().enumerate() {
            let label_nodes = if kinds[0] == pres::LIST_UNNUMBERED {
                // A marker a browser draws itself: no label content at all.
                Vec::new()
            } else if index == 0 {
                // A plain label: text and nothing else.
                vec![site.text(label)]
            } else {
                // A rich label: content with markup of its own.
                let styled = span(&mut site, Some(SpanStyle::Bold), label);
                let tail = site.text(" (b)");
                vec![styled, tail]
            };
            let text = site.text(item);
            let item_node = site.pres(pres::PAR, ParPayload::default(), vec![text]);
            entries.push((label_nodes, vec![item_node]));
        }
        let anchor = format!("{}-first", kinds.join("-"));
        lists.push(site.list(
            ListPayload {
                kinds: kinds.iter().map(|kind| name(kind)).collect(),
                anchor: None,
                entry_anchors: vec![Some(anchor)],
            },
            entries,
        ));
    }
    let tree = site.finish(lists);
    assert_golden("emit-lists.html", &written(tree));
}

#[test]
fn a_run_of_lines_writes_one_div_per_line_with_its_indentation_and_alignment() {
    let mut site = site();
    let mut lines = Vec::new();
    for (indent_left, indent_right, align, text) in [
        (0, 0, LineAlign::Left, "first"),
        (4, 0, LineAlign::Center, "second"),
        (0, 2, LineAlign::Right, "third"),
    ] {
        let content = site.text(text);
        let (indent_left, indent_right) = (Length::tabs(indent_left), Length::tabs(indent_right));
        lines.push(site.pres(
            pres::LINE,
            LinePayload { indent_left, indent_right, align },
            vec![content],
        ));
    }
    let node = site.pres(pres::LINES, LinesPayload, lines);
    let tree = site.finish(vec![node]);
    assert_golden("emit-lines.html", &written(tree));
}

// --- verbatim ------------------------------------------------------------------------------------

#[test]
fn verbatim_content_is_written_byte_for_byte_and_escaped_where_it_is_written() {
    let mut site = site();
    let inline = {
        let text = site.text("a < b & c");
        site.pres(
            pres::VERBATIM,
            VerbatimPayload {
                kind: VerbatimKind::Code,
                display: VerbatimDisplay::Inline,
                lang: None,
            },
            vec![text],
        )
    };
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![inline]);
    let block = {
        // A content beginning with a line break: an HTML parser drops the one
        // that directly follows `<pre>`, so the emitter writes one more.
        let text = site.text("\nif a < b & \\(x\\) {\n    print(\"<hi>\")\n}\n");
        site.pres(
            pres::VERBATIM,
            VerbatimPayload {
                kind: VerbatimKind::Code,
                display: VerbatimDisplay::Block,
                lang: Some(String::from("python")),
            },
            vec![text],
        )
    };
    let tree = site.finish(vec![paragraph, block]);
    let out = written(tree);
    assert!(out.contains("<pre class=\"flm-pres-verbatim\""), "{out}");
    assert!(out.contains("&lt;hi&gt;"), "{out}");
    assert_golden("emit-verbatim.html", &out);
}

// --- what the output shows as an error -------------------------------------------------------

#[test]
fn an_unattributed_macro_and_environment_are_shown_as_the_author_wrote_them() {
    let mut site = site();
    let text = site.text("Before ");
    let macro_node = site.pres(
        pres::UNKNOWN,
        UnknownPayload {
            kind: UnknownKind::Macro,
            name: String::from("foo"),
            spelling: String::from("\\foo"),
        },
        Vec::new(),
    );
    let after = site.text(" after.");
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![text, macro_node, after]);
    let inner = par(&mut site, "Body.");
    let environment = site.pres(
        pres::UNKNOWN,
        UnknownPayload {
            kind: UnknownKind::Environment,
            name: String::from("bar"),
            spelling: String::from("bar"),
        },
        vec![inner],
    );
    let tree = site.finish(vec![paragraph, environment]);
    assert_golden("emit-unknown.html", &written(tree));
}

#[test]
fn an_element_this_backend_cannot_write_is_an_empty_span_naming_it() {
    let mut site = site();
    let unsupported = site.pres(
        pres::UNSUPPORTED,
        UnsupportedPayload {
            element: flm_core::model::names::ElementName::from("thm.theorem"),
            message: String::from("no rule produced it"),
        },
        Vec::new(),
    );
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![unsupported]);
    let tree = site.finish(vec![paragraph]);
    assert_golden("emit-unsupported.html", &written(tree));
}

// --- mathematics -------------------------------------------------------------------------

/// A formula over one run of source text.
fn formula(
    site: &mut PresSite,
    display: MathDisplay,
    env: Option<&str>,
    source: &str,
) -> BuildId {
    let text = site.text(source);
    site.pres(
        pres::MATH_PASSTHROUGH,
        MathPassthroughPayload { display, env: env.map(String::from) },
        vec![text],
    )
}

/// A piece of content standing inside a formula: the command it was written
/// in, an anchor, and its content.
fn math_text(
    site: &mut PresSite,
    command: Option<&str>,
    anchor: Option<&str>,
    body: Vec<BuildId>,
) -> BuildId {
    site.pres(
        pres::MATH_TEXT,
        MathTextPayload {
            command: command.map(String::from),
            anchor: anchor.map(String::from),
            at_tag: false,
        },
        body,
    )
}

/// A `flm.pres.math-text` node marked as standing where an equation line's tag
/// stands, which is what makes this emitter write a pair of dollar signs
/// around it.
fn math_text_at_tag(site: &mut PresSite, body: Vec<BuildId>) -> BuildId {
    site.pres(
        pres::MATH_TEXT,
        MathTextPayload { at_tag: true, ..MathTextPayload::default() },
        body,
    )
}

#[test]
fn a_formula_is_written_whole_as_latex_with_the_delimiters_the_emitter_chose() {
    // Inline mathematics is a `<span>`, displayed mathematics a `<div>`, and a
    // formula written in a mathematics environment keeps the environment: the
    // renderer in the reader's browser reads the LaTeX between the
    // delimiters, which is the document's source untouched.
    let mut site = site();
    let inline = formula(&mut site, MathDisplay::Inline, None, "x^2 + y");
    let lead = site.text("Let ");
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![lead, inline]);
    let displayed = formula(&mut site, MathDisplay::Display, None, " a = b ");
    let environment =
        formula(&mut site, MathDisplay::Display, Some("align"), "a &= b \\\\\nc &= d");
    let tree = site.finish(vec![paragraph, displayed, environment]);
    let emitted = emit(tree);
    assert!(emitted.diags.iter().next().is_none(), "{}", emitted.diags.render_all());
    assert_golden("emit-math.html", &html(&emitted));
}

#[test]
fn content_inside_a_formula_is_written_as_html_inside_the_formula() {
    // A text command's content, and an empty anchored node at the start of
    // each line of an environment: a point in the formula a link can name,
    // which shows nothing.
    let mut site = site();
    let lead = site.text("x = ");
    let word = site.text("one half");
    let content = math_text(&mut site, Some("text"), None, vec![word]);
    let tail = site.text(" + y");
    let inline = site.pres(
        pres::MATH_PASSTHROUGH,
        MathPassthroughPayload { display: MathDisplay::Inline, env: None },
        vec![lead, content, tail],
    );
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![inline]);

    let first_anchor = math_text(&mut site, None, Some("eq:one"), Vec::new());
    let first_line = site.text("a &= b \\\\\n");
    let second_anchor = math_text(&mut site, None, Some("eq:two"), Vec::new());
    let second_line = site.text("c &= d");
    let environment = site.pres(
        pres::MATH_PASSTHROUGH,
        MathPassthroughPayload { display: MathDisplay::Display, env: Some(String::from("align")) },
        vec![first_anchor, first_line, second_anchor, second_line],
    );
    let tree = site.finish(vec![paragraph, environment]);
    let emitted = emit(tree);
    assert!(emitted.diags.iter().next().is_none(), "{}", emitted.diags.render_all());
    let out = html(&emitted);
    assert_eq!(out.matches("<tex-html>").count(), 3, "{out}");
    assert_golden("emit-math-text.html", &out);
}

#[test]
fn content_at_a_tags_place_is_written_between_dollar_signs() {
    // The dollar signs are this backend's, not the rule's: the rule marks the
    // node as standing where a line's tag stands, and this emitter writes the
    // pair around the whole node — once, not once per piece.
    let mut site = site();
    let line = site.text("a = b \\tag*{");
    let number = site.text("(1)");
    let tag = math_text_at_tag(&mut site, vec![number]);
    let close = site.text("}");
    let environment = site.pres(
        pres::MATH_PASSTHROUGH,
        MathPassthroughPayload { display: MathDisplay::Display, env: Some(String::from("equation")) },
        vec![line, tag, close],
    );
    let tree = site.finish(vec![environment]);
    let emitted = emit(tree);
    assert!(emitted.diags.iter().next().is_none(), "{}", emitted.diags.render_all());
    let out = html(&emitted);
    assert!(out.contains("\\tag*{$<tex-html>"), "{out}");
    assert!(out.contains("</tex-html>$}"), "{out}");
    assert_eq!(out.matches('$').count(), 2, "{out}");
}

#[test]
fn the_tag_that_puts_html_in_a_formula_cannot_be_typed_by_a_document() {
    // Both spellings in a document's own mathematics source get an empty TeX
    // group after their `<`, which typesets nothing: the reader sees the
    // characters and the renderer does not read them as its own tag.
    let mut site = site();
    let inline = formula(
        &mut site,
        MathDisplay::Inline,
        None,
        "a < b <tex-html><b id=\"x\">p</b></tex-html> c",
    );
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![inline]);
    let tree = site.finish(vec![paragraph]);
    let out = written(tree);
    assert!(out.contains("&lt;{}tex-html"), "{out}");
    assert!(out.contains("&lt;{}/tex-html"), "{out}");
    assert_golden("emit-math-neutralized.html", &out);
}

#[test]
fn text_inside_the_html_of_a_formula_breaks_the_delimiter_that_would_end_it() {
    // The renderer searches the characters of the whole formula — the HTML
    // written inside it included — for the closing delimiter, so text that
    // holds it would end the formula early. An empty element written inside
    // the sequence shows nothing and stops the search from matching.
    let mut site = site();
    let word = site.text("write \\) here");
    let content = math_text(&mut site, Some("text"), None, vec![word]);
    let inline = site.pres(
        pres::MATH_PASSTHROUGH,
        MathPassthroughPayload { display: MathDisplay::Inline, env: None },
        vec![content],
    );
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![inline]);

    let word = site.text("stop at \\end{align} here");
    let content = math_text(&mut site, Some("text"), None, vec![word]);
    let line = site.text("a &= b");
    let environment = site.pres(
        pres::MATH_PASSTHROUGH,
        MathPassthroughPayload { display: MathDisplay::Display, env: Some(String::from("align")) },
        vec![line, content],
    );
    let tree = site.finish(vec![paragraph, environment]);
    let emitted = emit(tree);
    assert!(emitted.diags.iter().next().is_none(), "{}", emitted.diags.render_all());
    let out = html(&emitted);
    assert!(out.contains("\\<span></span>)"), "{out}");
    assert!(out.contains("\\end<span></span>{align}"), "{out}");
    assert_golden("emit-math-breaks.html", &out);
}

#[test]
fn a_formula_inside_a_text_command_splits_the_content_it_stands_in() {
    // `\text{if \(x>0\) holds}`: the renderer does not read the HTML it
    // inserts a second time, so the inner formula's source goes into the
    // formula around it, with no delimiters of its own, and the content is
    // written in two pieces. The identifier and the command stand on the
    // first piece; the class on each.
    let mut site = site();
    let lead = site.text("if ");
    let inner = formula(&mut site, MathDisplay::Inline, None, "x > 0");
    let tail = site.text(" holds");
    let content = math_text(&mut site, Some("text"), Some("eq:cond"), vec![lead, inner, tail]);
    let before = site.text("y = ");
    let outer = site.pres(
        pres::MATH_PASSTHROUGH,
        MathPassthroughPayload { display: MathDisplay::Inline, env: None },
        vec![before, content],
    );
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![outer]);
    let tree = site.finish(vec![paragraph]);
    let emitted = emit(tree);
    assert!(emitted.diags.iter().next().is_none(), "{}", emitted.diags.render_all());
    let out = html(&emitted);
    assert_eq!(out.matches("<tex-html>").count(), 2, "{out}");
    assert_eq!(out.matches("id=\"eq:cond\"").count(), 1, "{out}");
    assert_eq!(out.matches("data-flm-command=\"text\"").count(), 1, "{out}");
    assert_eq!(out.matches("class=\"flm-pres-math--text\"").count(), 2, "{out}");
    // The inner formula is no element of its own and keeps no delimiters of
    // its own: its characters already stand in mathematics.
    assert_eq!(out.matches("flm-pres-math--passthrough").count(), 1, "{out}");
    assert_eq!(out.matches("\\(").count(), 1, "{out}");
    assert_golden("emit-math-split.html", &out);
}

#[test]
fn an_attribute_inside_the_html_of_a_formula_that_would_end_it_is_left_out() {
    // An anchor built from a label the author wrote: an attribute value
    // cannot carry the empty element that repairs text, so the attribute is
    // left out and the content stays. The text of the content holds no
    // sequence of its own here, so the golden shows the dropped attribute and
    // nothing else.
    let mut site = site();
    let word = site.text("a word");
    let content = math_text(&mut site, Some("text"), Some("eq:\\)7"), vec![word]);
    let inline = site.pres(
        pres::MATH_PASSTHROUGH,
        MathPassthroughPayload { display: MathDisplay::Inline, env: None },
        vec![content],
    );
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![inline]);
    let tree = site.finish(vec![paragraph]);
    let emitted = emit(tree);
    expect(emitted.diags.iter(), "flm.html.refused-in-math").severity(Severity::Error).once();
    let out = html(&emitted);
    assert!(!out.contains("id="), "{out}");
    assert!(out.contains(">a word</span>"), "{out}");
    assert_golden("emit-math-refused-attribute.html", &out);
}

#[test]
fn a_construct_nodes_attribute_inside_a_formula_passes_the_same_check() {
    // A tag a lowering rule staged, standing inside `\text{…}`: its attribute
    // values are written into the characters the renderer searches, exactly as
    // the emitter's own are, so they are checked the same way. The rule that
    // staged the tag could not make this check itself: only the emitter knows
    // which delimiter it wrote around this formula.
    let mut site = site();
    let word = site.text("x");
    let tag = site.pres(
        TAG,
        HtmlTagPayload::new("abbr").attributes([("title", "x \\) y"), ("lang", "en")]),
        vec![word],
    );
    let content = math_text(&mut site, Some("text"), None, vec![tag]);
    let inline = site.pres(
        pres::MATH_PASSTHROUGH,
        MathPassthroughPayload { display: MathDisplay::Inline, env: None },
        vec![content],
    );
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![inline]);
    let tree = site.finish(vec![paragraph]);
    let emitted = emit(tree);
    expect(emitted.diags.iter(), "flm.html.refused-in-math").severity(Severity::Error).once();
    let out = html(&emitted);
    assert!(!out.contains("title="), "{out}");
    assert!(out.contains("lang=\"en\""), "{out}");
    assert_golden("emit-math-construct-attribute.html", &out);
}

#[test]
fn a_formula_standing_deeper_than_the_contents_own_nodes_closes_and_opens_again() {
    // `\text{\emph{\(x\)}}`: the formula stands inside a styled span inside
    // the content. The emphasis and the piece are closed, the formula's source
    // goes into the formula around it, and the piece and the emphasis are
    // opened again — the identifier on the first half alone. Nothing is
    // dropped and nothing is reported.
    let mut site = site();
    let before = site.text("if ");
    let inner = formula(&mut site, MathDisplay::Inline, None, "x");
    let after = site.text(" holds");
    let emphasis = site.pres(
        pres::SPAN,
        SpanPayload {
            style: Some(SpanStyle::Emphasis),
            anchor: Some(String::from("em-1")),
            ..SpanPayload::default()
        },
        vec![before, inner, after],
    );
    let content = math_text(&mut site, Some("text"), Some("eq:deep"), vec![emphasis]);
    let lead = site.text("y = ");
    let outer = site.pres(
        pres::MATH_PASSTHROUGH,
        MathPassthroughPayload { display: MathDisplay::Inline, env: None },
        vec![lead, content],
    );
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![outer]);
    let tree = site.finish(vec![paragraph]);
    let emitted = emit(tree);
    assert!(emitted.diags.iter().next().is_none(), "{}", emitted.diags.render_all());
    let out = html(&emitted);
    assert_eq!(out.matches("<tex-html>").count(), 2, "{out}");
    assert_eq!(out.matches("<em").count(), 2, "{out}");
    // An identifier stands once: on the first half of the piece and on the
    // first half of the emphasis.
    assert_eq!(out.matches("id=\"eq:deep\"").count(), 1, "{out}");
    assert_eq!(out.matches("id=\"em-1\"").count(), 1, "{out}");
    // The inner formula keeps no delimiters and is no element of its own.
    assert_eq!(out.matches("flm-pres-math--passthrough").count(), 1, "{out}");
    assert_eq!(out.matches("\\(").count(), 1, "{out}");
    assert_golden("emit-math-deeper.html", &out);
}

#[test]
fn a_formula_two_levels_deeper_closes_and_opens_every_element_around_it() {
    // `\text{\emph{\textbf{\(x\)}}}`: two elements stand between the piece
    // and the formula, and both are closed and opened again, outermost first.
    let mut site = site();
    let before = site.text("a");
    let inner = formula(&mut site, MathDisplay::Display, Some("align"), "x");
    let after = site.text("b");
    let bold = site.pres(
        pres::SPAN,
        SpanPayload { style: Some(SpanStyle::Bold), ..SpanPayload::default() },
        vec![before, inner, after],
    );
    let emphasis = site.pres(
        pres::SPAN,
        SpanPayload { style: Some(SpanStyle::Emphasis), ..SpanPayload::default() },
        vec![bold],
    );
    let content = math_text(&mut site, Some("text"), None, vec![emphasis]);
    let line = site.text("u &= v \\\\\n");
    let outer = site.pres(
        pres::MATH_PASSTHROUGH,
        MathPassthroughPayload { display: MathDisplay::Display, env: Some(String::from("align")) },
        vec![line, content],
    );
    let tree = site.finish(vec![outer]);
    let emitted = emit(tree);
    assert!(emitted.diags.iter().next().is_none(), "{}", emitted.diags.render_all());
    let out = html(&emitted);
    assert_eq!(out.matches("<tex-html>").count(), 2, "{out}");
    assert_eq!(out.matches("<em").count(), 2, "{out}");
    assert_eq!(out.matches("<strong").count(), 2, "{out}");
    // The inner formula's environment is its own: the emitter writes no
    // delimiters around a formula that already stands in mathematics, so the
    // characters `\begin{align}` of the inner one never appear.
    assert_eq!(out.matches("\\begin{align}").count(), 1, "{out}");
    assert_golden("emit-math-deeper-twice.html", &out);
}

#[test]
fn content_inside_a_formula_met_outside_one_is_the_span_alone() {
    // A hand-built tree, or a rule that staged the node elsewhere: there is no
    // formula to put HTML into, so nothing is written around the span.
    let mut site = site();
    let word = site.text("one half");
    let content = math_text(&mut site, Some("textbf"), Some("a-1"), vec![word]);
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![content]);
    let tree = site.finish(vec![paragraph]);
    let out = written(tree);
    assert!(!out.contains("tex-html"), "{out}");
    assert!(
        out.contains(
            "<span class=\"flm-pres-math--text\" id=\"a-1\" data-flm-command=\"textbf\">one \
             half</span>"
        ),
        "{out}"
    );
}

// --- the construct element -------------------------------------------------------------------

#[test]
fn a_construct_node_is_written_as_its_payload_states_it_and_the_contract_adds_nothing() {
    let mut site = site();
    let inner = site.text("inside & out");
    let children = site.pres(
        TAG,
        HtmlTagPayload::new("figure")
            .attributes([("class", "picture-figure"), ("data-picture", "a \"quoted\" value")])
            .block(),
        vec![inner],
    );
    let markup = site.pres(
        TAG,
        HtmlTagPayload::new("span").markup(Markup::trusted("<svg viewBox=\"0 0 1 1\"></svg>")),
        Vec::new(),
    );
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![markup]);
    let tree = site.finish(vec![children, paragraph]);
    assert_golden("emit-construct.html", &written(tree));
}

#[test]
fn the_text_inside_a_tex_html_construct_breaks_every_delimiter_and_tag() {
    // A rule writing a formula itself (a processing feature's rule) stages its
    // holes as `tex-html` constructs; the browser's renderer reads their
    // characters as it reads the emitter's own `<tex-html>`, so their text is
    // repaired the same way, for every closing delimiter, since the emitter
    // does not know the formula around them. Text outside such a construct is
    // written unchanged.
    let mut site = site();
    let inside = site.text("a \\(y\\) \\] \\end{align} <tex-html> </tex-html> b");
    let hole = site.pres(TAG, HtmlTagPayload::new("tex-html"), vec![inside]);
    let before = site.text("\\(");
    let after = site.text("\\)");
    let formula = site.pres(TAG, HtmlTagPayload::new("span"), vec![before, hole, after]);
    let tree = site.finish(vec![formula]);
    let emitted = emit(tree);
    assert!(emitted.diags.iter().next().is_none(), "{}", emitted.diags.render_all());
    assert_eq!(
        html(&emitted),
        "<span>\\(<tex-html>a \\(y\\<span></span>) \\<span></span>] \\end<span></span>{align} \
         &lt;<span></span>tex-html&gt; &lt;<span></span>/tex-html&gt; b</tex-html>\\)</span>"
    );
}

#[test]
fn a_construct_node_whose_name_is_not_writable_is_refused_and_stands_in() {
    for payload in [
        HtmlTagPayload::new("p><script>alert(1)</script"),
        HtmlTagPayload::new("span").attributes([("onclick=\"x\" y", "1")]),
    ] {
        let mut site = site();
        let node = site.pres(TAG, payload, Vec::new());
        let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![node]);
        let tree = site.finish(vec![paragraph]);
        let emitted = emit(tree);
        expect(emitted.diags.iter(), "flm.html.invalid-name").severity(Severity::Error).once();
        let out = html(&emitted);
        assert!(out.contains("data-flm-element=\"flm.pres.html.tag\""), "{out}");
        assert!(!out.contains("alert"), "{out}");
        assert!(!out.contains("onclick"), "{out}");
    }
}

// --- a tree placed in another tree ---------------------------------------------------------

#[test]
fn a_placed_tree_closes_the_segment_it_stands_in_and_is_recorded() {
    let mut site = site();
    let before = par(&mut site, "Before.");
    let placed = site.pres(pres::SUBTREE, SubtreePayload { tree: TreeId::tree(1) }, Vec::new());
    let after = par(&mut site, "After.");
    let tree = site.finish(vec![before, placed, after]);
    let emitted = emit(tree);
    assert_eq!(emitted.segments.len(), 3);
    assert!(matches!(emitted.segments[0], HtmlSegment::Html(_)));
    assert_eq!(emitted.segments[1], HtmlSegment::Tree(TreeId::tree(1)));
    assert!(matches!(emitted.segments[2], HtmlSegment::Html(_)));
    let HtmlSegment::Html(first) = &emitted.segments[0] else { unreachable!() };
    assert!(first.contains("Before."), "{first}");
    assert!(!first.contains("After."), "{first}");
}

// --- the depth limit --------------------------------------------------------------------------

/// A paragraph holding `depth` nested spans.
fn nested_spans(depth: usize) -> Arc<NodeTree<Flm>> {
    let mut site = site();
    let mut node = site.text("x");
    for _ in 0..depth {
        node = site.pres(pres::SPAN, SpanPayload::default(), vec![node]);
    }
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![node]);
    site.finish(vec![paragraph])
}

#[test]
fn a_tree_nesting_past_the_depth_limit_is_refused_with_its_condition() {
    let emitted = emit(nested_spans(DEPTH_LIMIT + 8));
    expect(emitted.diags.iter(), "flm.html.nesting-too-deep").severity(Severity::Error).once();
    assert!(emitted.segments.is_empty(), "the tree writes nothing");
}

#[test]
fn a_tree_nesting_under_the_depth_limit_is_written() {
    let emitted = emit(nested_spans(DEPTH_LIMIT / 2));
    assert!(emitted.diags.iter().next().is_none(), "{}", emitted.diags.render_all());
    assert!(html(&emitted).contains(">x<"));
}

// --- the source-span attribute -------------------------------------------------------------

/// A tree of two paragraphs, parsed from a source whose canonical path is
/// `origin` and lowered for this backend, so that its nodes have a position a
/// host can name.
fn tree_on_a_named_source(origin: &str) -> Arc<NodeTree<Flm>> {
    parsed_on_a_named_source("One two.\n\nThree four.\n", origin)
}

/// One paragraph holding an inline element of each kind the option writes the
/// attribute on — a link, a styled span, inline verbatim, an inline formula
/// and text inside that formula — parsed from a source named `origin`.
fn inline_tree_on_a_named_source(origin: &str) -> Arc<NodeTree<Flm>> {
    parsed_on_a_named_source(
        "\\section{Intro}\\label{sec:intro}\n\n\
         See \\ref{sec:intro} and \\emph{this bit}, \
         \\verbcode+let x = 1;+ and \\(\\text{a word} + 1\\).\n",
        origin,
    )
}

/// `document`, parsed from a source whose canonical path is `origin` and
/// lowered for this backend, so that its nodes have a position a host can
/// name.
fn parsed_on_a_named_source(document: &str, origin: &str) -> Arc<NodeTree<Flm>> {
    use flm_core::core::plan::Overrides;
    use flm_core::core::run::RunOptions;
    use flm_core::lang::{FlmOrigin, ParseOptions, parse_fragment};
    use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
    use flm_core::model::names::BackendName;
    use flm_core::stdcore::StdcoreConfig;
    use flm_core::stdcore::backends::html;
    use flm_core::stdcore::backends::lowering::lowered_table;
    use techy::source::Source;

    let registry = std_package::registry(StdcoreConfig::new());
    let source = Arc::new(Source::new(document).with_origin(FlmOrigin::new(origin)));
    let fragment =
        parse_fragment(registry.lang(), source, FragmentLayout::Blocks, &ParseOptions::default())
            .expect("the parse yields a tree");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let doc = Arc::new(Doc::new(content));
    let plan = std_package::plan(&registry, html::WORKFLOW, &Overrides::none());
    let run = pipeline::run(&doc, &plan, &RunOptions::sequential());
    let trees = run
        .table(lowered_table(&BackendName::from(html::BACKEND)))
        .expect("declared")
        .expect("written");
    Arc::clone(&trees.get(TreeId::tree(0)).expect("the tree was lowered").tree)
}

/// The attribute the render option adds, written out once here so that every
/// test below reads the same spelling.
///
/// Its value is JSON, so the sink writes it between *single* quotation marks:
/// a double quotation mark then stands as itself and the raw page shows the
/// JSON as JSON.
const SOURCE_SPAN: &str = " data-flm-source-span='";

#[test]
fn the_source_span_attribute_is_the_only_difference_the_option_makes() {
    let tree = tree_on_a_named_source("book/ch1.flm");
    let off = html(&emit(Arc::clone(&tree)));
    let on = html(&emit_with(
        Arc::clone(&tree),
        &HtmlRenderOptions { source_spans: true, ..HtmlRenderOptions::default() },
    ));
    // With the option off nothing says where a block stands.
    assert!(!off.contains("data-flm-source-span"), "{off}");
    assert!(on.contains(SOURCE_SPAN), "{on}");
    // The bytes with the option off are the bytes with it on minus the attribute.
    assert_eq!(strip_source_span(&on), off);
    assert_golden("emit-source-spans.html", &on);

    // The same holds of inline content, which carries the attribute too.
    let tree = inline_tree_on_a_named_source("book/ch1.flm");
    let off = html(&emit(Arc::clone(&tree)));
    let on = html(&emit_with(
        tree,
        &HtmlRenderOptions { source_spans: true, ..HtmlRenderOptions::default() },
    ));
    assert!(!off.contains("data-flm-source-span"), "{off}");
    assert_eq!(strip_source_span(&on), off);
}

#[test]
fn an_inline_element_written_for_a_node_carries_its_position_too() {
    // The ruling is that every element written for a presentation node that
    // has a position in a named source carries the attribute, inline or block
    // alike. One paragraph here holds one of each inline kind, and each of
    // them carries it: the link, the styled span, the inline verbatim, the
    // inline formula, and the text written inside that formula.
    let on = html(&emit_with(
        inline_tree_on_a_named_source("book/ch1.flm"),
        &HtmlRenderOptions { source_spans: true, ..HtmlRenderOptions::default() },
    ));
    for opening in [
        "<a class=\"flm-pres-link\"",
        "<em class=\"flm-pres-span\"",
        "<code class=\"flm-pres-verbatim\"",
        "<span class=\"flm-pres-math--passthrough\"",
        "<span class=\"flm-pres-math--text\"",
    ] {
        let at = on.find(opening).unwrap_or_else(|| panic!("{opening} stands in: {on}"));
        let tag = &on[at..at + on[at..].find('>').expect("the tag ends")];
        assert!(tag.contains(SOURCE_SPAN), "{tag}");
    }
    // Every range names the source the document was parsed from and holds
    // something.
    for value in decoded_source_span_values(&on) {
        let (source, start, end) = json_span(&value);
        assert_eq!(source, "book/ch1.flm");
        assert!(start < end, "a range that holds something: {value}");
    }
}

#[test]
fn a_node_written_as_several_pieces_carries_its_position_on_the_first_alone() {
    // Text inside a formula is written as one `<span>`, unless a formula
    // stands inside that text: the text is then written as one piece before
    // the inner formula and one after it, and a styled span around the inner
    // formula is closed and opened again around the pieces. A node's position
    // stands once, on its own first piece, exactly as its identifier does.
    let on = html(&emit_with(
        parsed_on_a_named_source(
            "A \\(\\text{one \\(y\\) two}\\) and \\(\\text{\\emph{\\(z\\)}}\\).\n",
            "book/ch1.flm",
        ),
        &HtmlRenderOptions { source_spans: true, ..HtmlRenderOptions::default() },
    ));
    // Six nodes have a position here — the paragraph, the two formulas, the
    // text inside each of them, and the styled span inside the second — and
    // the attribute stands six times, although the text of the first formula
    // is written as two pieces and the styled span of the second is written
    // twice.
    assert_eq!(on.matches(SOURCE_SPAN).count(), 6, "{on}");
}

#[test]
fn a_source_name_that_would_end_a_formula_is_refused_inside_it_like_any_value() {
    // Inside a formula, an attribute whose value holds the delimiter that
    // closes that formula would end the formula where the attribute stands,
    // so the emitter leaves the attribute out and reports it. The source
    // position is an attribute like any other there: a source named
    // `a\)b.flm` costs the elements inside the formula their position, and
    // nothing else.
    let on = html(&emit_with(
        parsed_on_a_named_source("A \\(\\text{a word}\\).\n", "a\\)b.flm"),
        &HtmlRenderOptions { source_spans: true, ..HtmlRenderOptions::default() },
    ));
    // The paragraph and the formula stand outside the formula's own HTML and
    // keep their position; the text inside it loses it.
    assert_eq!(on.matches(SOURCE_SPAN).count(), 2, "{on}");
    assert!(on.contains("<span class=\"flm-pres-math--text\" data-flm-command=\"text\">"), "{on}");
}

/// A site over [`vocabulary`] whose nodes stand on a source named
/// `book/ch1.flm`, so that a hand-built tree has a position a host can name.
///
/// The source is empty, so every node's range is `0..0`: what these trees give
/// is the source's name and the attribute's spelling, not a range.
fn named_site() -> PresSite {
    PresSite::named(&vocabulary(), "book/ch1.flm")
}

/// `tree`, written with the source-position option on.
fn with_spans(tree: Arc<NodeTree<Flm>>) -> String {
    html(&emit_with(
        tree,
        &HtmlRenderOptions { source_spans: true, ..HtmlRenderOptions::default() },
    ))
}

#[test]
fn the_stand_in_for_an_element_this_backend_cannot_write_carries_its_position() {
    // `flm.pres.unsupported` is a presentation element of its own, and the
    // empty span written for it is the one element written for that node, so
    // it carries the node's position like any other.
    let mut site = named_site();
    let unsupported = site.pres(
        pres::UNSUPPORTED,
        UnsupportedPayload {
            element: flm_core::model::names::ElementName::from("thm.theorem"),
            message: String::from("no rule produced it"),
        },
        Vec::new(),
    );
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![unsupported]);
    let on = with_spans(site.finish(vec![paragraph]));
    let at = on.find("<span class=\"flm-pres-unsupported\"").unwrap_or_else(|| panic!("{on}"));
    let tag = &on[at..at + on[at..].find('>').expect("the tag ends")];
    assert!(tag.contains(SOURCE_SPAN), "{tag}");
}

#[test]
fn a_block_image_carries_the_class_of_a_block_before_its_own() {
    let mut site = site();
    let image = |display| ImagePayload {
        src: String::from("a.png"),
        display,
        ..ImagePayload::default()
    };
    let block = site.pres(pres::IMAGE, image(ImageDisplay::Block), Vec::new());
    let inline = site.pres(pres::IMAGE, image(ImageDisplay::Inline), Vec::new());
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![inline]);
    let out = html(&emit(site.finish(vec![block, paragraph])));
    assert!(out.contains("<img class=\"flm-pres-block flm-pres-image\" src=\"a.png\">"), "{out}");
    assert!(out.contains("<p class=\"flm-pres-par\"><img class=\"flm-pres-image\" src=\"a.png\"></p>"), "{out}");
}

#[test]
fn an_image_carries_its_position_in_the_spelling_every_value_of_this_kind_takes() {
    // An `<img>` is written and never closed, through the emitter's one other
    // writer, so it is the one element whose attributes do not go through the
    // writer the tests above exercise. Its position is written there in the
    // same spelling: single quotation marks around the JSON.
    let mut site = named_site();
    let image = site.pres(
        pres::IMAGE,
        ImagePayload {
            src: String::from("figures/plot.png"),
            display: ImageDisplay::Inline,
            alt: None,
            width: None,
            height: None,
            srcset: Vec::new(),
            anchor: None,
        },
        Vec::new(),
    );
    let paragraph = site.pres(pres::PAR, ParPayload::default(), vec![image]);
    let on = with_spans(site.finish(vec![paragraph]));
    let at = on.find("<img class=\"flm-pres-image\"").unwrap_or_else(|| panic!("{on}"));
    let tag = &on[at..at + on[at..].find('>').expect("the tag ends")];
    assert!(tag.contains("data-flm-source-span='[\"book/ch1.flm\",0,0]'"), "{tag}");
}

#[test]
fn an_element_standing_for_no_node_of_its_own_carries_no_position() {
    // A block of verbatim is written as `<pre><code>`: the `<pre>` stands for
    // the node and carries the attribute, while the `<code>` inside it is a
    // piece of HTML's own spelling for preformatted text and stands for no
    // node, so it carries none.
    let on = html(&emit_with(
        parsed_on_a_named_source(
            "\\begin{verbatimcode}\nlet x = 1;\n\\end{verbatimcode}\n",
            "book/ch1.flm",
        ),
        &HtmlRenderOptions { source_spans: true, ..HtmlRenderOptions::default() },
    ));
    assert!(on.contains("<pre class=\"flm-pres-verbatim\""), "{on}");
    assert!(on.contains(SOURCE_SPAN), "{on}");
    assert!(on.contains("<code>"), "{on}");
}

#[test]
fn the_value_is_a_json_array_of_the_source_and_the_two_byte_offsets() {
    // The value is JSON, and it stands in the page as JSON: the sink writes it
    // between single quotation marks, where the quotation marks of the JSON
    // string need no rewriting. What a script reads with `JSON.parse` is
    // `["book/ch1.flm",0,8]`, and so is what the raw page shows.
    let on = html(&emit_with(
        tree_on_a_named_source("book/ch1.flm"),
        &HtmlRenderOptions { source_spans: true, ..HtmlRenderOptions::default() },
    ));
    assert!(on.contains("data-flm-source-span='[\"book/ch1.flm\",0,8]'"), "{on}");
    for value in decoded_source_span_values(&on) {
        let (source, start, end) = json_span(&value);
        assert_eq!(source, "book/ch1.flm");
        assert!(start < end, "a range that holds something: {value}");
    }
}

#[test]
fn a_source_name_holding_every_character_that_matters_stays_readable_as_json() {
    // The name holds all five characters that either escaping could rewrite: a
    // single quotation mark, which ends the attribute; a double quotation mark
    // and a backslash, which JSON escapes; an ampersand, which begins a
    // character reference; and a `<`, which begins a tag between tags but not
    // inside a quoted attribute value.
    let name = "a'b\"c&d<e\\f.flm";
    let on = html(&emit_with(
        tree_on_a_named_source(name),
        &HtmlRenderOptions { source_spans: true, ..HtmlRenderOptions::default() },
    ));
    // JSON writes the name as `"a'b\"c&d<e\\f.flm"`; the single-quoted HTML
    // escaping then rewrites the ampersand and the single quotation mark, and
    // nothing else.
    assert!(
        on.contains(
            "data-flm-source-span='[\"a&#39;b\\\"c&amp;d<e\\\\f.flm\",0,8]'"
        ),
        "{on}"
    );
    // And an HTML parser hands a script back the JSON that was written, which
    // parses to the name the document was given.
    let value = &decoded_source_span_values(&on)[0];
    assert_eq!(value, "[\"a'b\\\"c&d<e\\\\f.flm\",0,8]");
    assert_eq!(json_span(value), (String::from(name), 0, 8));
}

#[test]
fn a_node_with_no_position_in_a_named_source_carries_no_attribute() {
    // A hand-built tree stands on no source a host can name, so the option
    // finds nothing to write and the page is the page it would be without it.
    let mut site = site();
    let first = par(&mut site, "One.");
    let second = par(&mut site, "Two.");
    let tree = site.finish(vec![first, second]);
    let on = html(&emit_with(
        Arc::clone(&tree),
        &HtmlRenderOptions { source_spans: true, ..HtmlRenderOptions::default() },
    ));
    assert!(!on.contains("data-flm-source-span"), "{on}");
    assert_eq!(on, html(&emit(tree)));
}

/// `on` with every ` data-flm-source-span='…'` attribute removed.
fn strip_source_span(on: &str) -> String {
    let mut out = String::new();
    let mut rest = on;
    while let Some(at) = rest.find(SOURCE_SPAN) {
        out.push_str(&rest[..at]);
        let tail = &rest[at + SOURCE_SPAN.len()..];
        let close = tail.find('\'').expect("the attribute value ends");
        rest = &tail[close + 1..];
    }
    out.push_str(rest);
    out
}

/// The value of every source-position attribute of the fragment `on`, as an
/// HTML parser hands it to a script: the character references decoded, in the
/// order the attributes stand in.
///
/// The decoding is a real parser's and not this file's idea of one: the parser
/// is `html5ever`, the one `tests/html_validity.rs` parses every golden with.
/// A parse error there is a failure here too, so an attribute value that broke
/// out of its attribute could not pass unnoticed.
fn decoded_source_span_values(on: &str) -> Vec<String> {
    use html5ever::driver::{ParseOpts, parse_fragment};
    use html5ever::tendril::TendrilSink;
    use html5ever::{QualName, local_name, ns};
    use markup5ever_rcdom::{Handle, NodeData, RcDom};

    let dom = parse_fragment(
        RcDom::default(),
        ParseOpts::default(),
        QualName::new(None, ns!(html), local_name!("div")),
        Vec::new(),
        false,
    )
    .one(on);
    let errors: Vec<String> = dom.errors.borrow().iter().map(|e| e.to_string()).collect();
    assert!(errors.is_empty(), "the parser reported {errors:?} on: {on}");

    fn walk(node: &Handle, values: &mut Vec<String>) {
        if let NodeData::Element { attrs, .. } = &node.data {
            for attribute in attrs.borrow().iter() {
                if &*attribute.name.local == "data-flm-source-span" {
                    values.push(attribute.value.to_string());
                }
            }
        }
        for child in node.children.borrow().iter() {
            walk(child, values);
        }
    }

    let mut values = Vec::new();
    walk(&dom.document, &mut values);
    assert!(!values.is_empty(), "the option wrote no attribute: {on}");
    values
}

/// The attribute's decoded value read as JSON: the source's name and the two
/// byte offsets.
///
/// The reading is `serde_json`'s, so what these tests check is that the value
/// is JSON and holds what it is meant to hold — not that it is spelled one
/// particular way. It fails the test on anything else, which is how it checks
/// that the value really is that array.
fn json_span(value: &str) -> (String, usize, usize) {
    serde_json::from_str(value).unwrap_or_else(|e| panic!("a JSON array: {value}: {e}"))
}

// --- the requirement set -----------------------------------------------------------------------

/// The identifiers an emitted tree requires.
fn requires(emitted: &EmittedTree) -> Vec<String> {
    emitted.requires.iter().map(|name| name.as_str().to_string()).collect()
}

#[test]
fn a_tree_of_paragraphs_requires_the_core_definitions_alone() {
    let mut site = site();
    let first = par(&mut site, "One.");
    let second = par(&mut site, "Two.");
    let tree = site.finish(vec![first, second]);
    assert_eq!(requires(&emit(tree)), ["flm.html.style.core"]);
}

#[test]
fn a_tree_holding_code_requires_the_verbatim_definition_beside_the_core_one() {
    let mut site = site();
    let text = site.text("print(1)");
    let code = site.pres(
        pres::VERBATIM,
        VerbatimPayload {
            kind: VerbatimKind::Code,
            display: VerbatimDisplay::Block,
            lang: Some(String::from("python")),
        },
        vec![text],
    );
    let paragraph = par(&mut site, "Before.");
    let tree = site.finish(vec![paragraph, code]);
    assert_eq!(requires(&emit(tree)), ["flm.html.style.core", "flm.html.style.verbatim"]);
}

#[test]
fn a_tree_holding_mathematics_requires_the_formula_renderer_as_well() {
    // A requirement is a name. Mathematics needs two: the stylesheet that
    // gives a formula and the content inside one their look, and the script
    // that turns the LaTeX into mathematics in the reader's browser.
    let mut site = site();
    let paragraph = par(&mut site, "Before.");
    let inline = formula(&mut site, MathDisplay::Inline, None, "x^2");
    let tree = site.finish(vec![paragraph, inline]);
    assert_eq!(
        requires(&emit(tree)),
        ["flm.html.mathjax", "flm.html.style.core", "flm.html.style.math-passthrough"]
    );
    assert_eq!(SCRIPT_MATHJAX, "flm.html.mathjax");
}

// --- the contract ---------------------------------------------------------------------------

#[test]
fn every_element_of_the_vocabulary_has_exactly_one_row_and_no_other_name_does() {
    let mut elements: Vec<&str> = CONVENTIONS.iter().map(|row| row.element).collect();
    elements.sort();
    let mut all: Vec<&str> = pres::ALL.to_vec();
    all.sort();
    assert_eq!(elements, all);
}

/// Every payload of the vocabulary, fully populated, beside its element.
fn populated_payloads() -> Vec<(&'static str, Arc<dyn PresPayload>)> {
    let anchor = || Some(String::from("a-1"));
    vec![
        (pres::PAR, Arc::new(ParPayload { anchor: anchor() }) as Arc<dyn PresPayload>),
        (
            pres::BLOCK,
            Arc::new(BlockPayload {
                role: Some(name("flm.theorem")),
                kinds: vec![name("numbered")],
                anchor: anchor(),
                doc_role: Some(DocRole::Endnotes),
            }),
        ),
        (pres::BLOCK_HEADING, Arc::new(HeadingPayload {
                name: name("subsection"),
                level: 2,
                in_toc: true,
                anchor: anchor(),
                number: Some(String::from("2.1")),
            })),
        (
            pres::RUNIN_HEADING,
            Arc::new(HeadingPayload {
                name: name("paragraph"),
                level: 4,
                in_toc: false,
                anchor: anchor(),
                number: Some(String::from("2.1")),
            }),
        ),
        (
            pres::SPAN,
            Arc::new(SpanPayload {
                style: Some(SpanStyle::Bold),
                role: Some(name("flm.defterm")),
                kinds: vec![name("primary")],
                anchor: anchor(),
                doc_role: Some(DocRole::Footnote),
            }),
        ),
        (
            pres::LIST,
            Arc::new(ListPayload {
                kinds: vec![name("itemize")],
                anchor: anchor(),
                entry_anchors: vec![Some(String::from("item-a"))],
            }),
        ),
        (pres::LINES, Arc::new(LinesPayload)),
        (
            pres::LINE,
            Arc::new(LinePayload {
                indent_left: Length::tabs(1),
                indent_right: Length::tabs(2),
                align: LineAlign::Center,
            }),
        ),
        (
            pres::VERBATIM,
            Arc::new(VerbatimPayload {
                kind: VerbatimKind::Code,
                display: VerbatimDisplay::Block,
                lang: Some(String::from("python")),
            }),
        ),
        (
            pres::MATH_PASSTHROUGH,
            Arc::new(MathPassthroughPayload {
                display: MathDisplay::Display,
                env: Some(String::from("align")),
            }),
        ),
        (
            pres::MATH_TEXT,
            Arc::new(MathTextPayload {
                command: Some(String::from("text")),
                anchor: anchor(),
                at_tag: true,
            }),
        ),
        (
            pres::LINK,
            Arc::new(LinkPayload {
                href: String::from("#a-1"),
                kind: name("ref"),
                doc_role: Some(DocRole::NoteRef),
                label: Some(String::from("Footnote 1")),
                shows_target: false,
            }),
        ),
        (
            pres::IMAGE,
            Arc::new(ImagePayload {
                src: String::from("a.png"),
                display: ImageDisplay::Block,
                alt: Some(String::from("a")),
                width: Some(Length::parse("10pt").unwrap()),
                height: Some(Length::parse("5pt").unwrap()),
                srcset: vec![SrcsetCandidate {
                    src: String::from("b.png"),
                    descriptor: String::from("2x"),
                }],
                anchor: anchor(),
            }),
        ),
        (
            pres::UNSUPPORTED,
            Arc::new(UnsupportedPayload {
                element: flm_core::model::names::ElementName::from("thm.theorem"),
                message: String::from("why"),
            }),
        ),
        (
            pres::UNKNOWN,
            Arc::new(UnknownPayload {
                kind: UnknownKind::Macro,
                name: String::from("foo"),
                spelling: String::from("\\foo"),
            }),
        ),
        (pres::DECO_RUNIN, Arc::new(DecorationPayload)),
        (pres::DECO_FINAL, Arc::new(DecorationPayload)),
        (pres::DECO_MARGIN, Arc::new(DecorationPayload)),
        (pres::DECO_MARK, Arc::new(DecoMarkPayload { slot: 1 })),
        (pres::SUBTREE, Arc::new(SubtreePayload { tree: TreeId::tree(1) })),
    ]
}

#[test]
fn every_field_of_every_payload_is_written_or_the_row_says_why_not() {
    use techy::serialize::SerialValue;

    for (element, payload) in populated_payloads() {
        let convention = row(element).expect("a row for every element");
        let SerialValue::Map(entries) = payload.project() else {
            panic!("`{element}` projects as something other than a map");
        };
        let keys: Vec<&str> = entries.iter().map(|(key, _)| key.as_str()).collect();
        for key in &keys {
            assert!(
                convention.field(key).is_some(),
                "`{element}`'s field `{key}` is neither written nor listed as dropped"
            );
        }
        for field in convention.fields {
            // A ratio is named after its custom property and derived from two
            // length fields, which the payload projects.
            let projected: Vec<&str> = match field.written {
                Written::Ratio(first, second) => vec![first, second],
                _ => vec![field.field],
            };
            for name in projected {
                assert!(
                    keys.contains(&name),
                    "`{element}`'s row names `{name}`, which its payload does not project"
                );
            }
        }
    }
}

#[test]
fn the_attributes_the_emitter_writes_are_exactly_the_rows() {
    // Every element with a populated payload, written into a tree of its own,
    // and the attributes found in the output compared with the row's.
    for (element, _) in populated_payloads() {
        let convention = row(element).expect("a row for every element");
        // The order is fixed too: the class first, then the element's own
        // attributes in the row's order, then `style` where the row has a
        // length field, then the `data-flm-` attributes in the row's order. It
        // is not part of the contract a stylesheet may rely on, but it is what
        // the emitter promises its own goldens, and deriving it here keeps a
        // regenerated golden from changing it quietly.
        let written = |wanted: fn(&Written) -> bool| -> Vec<String> {
            convention
                .fields
                .iter()
                .filter(|field| wanted(&field.written))
                .filter_map(|field| field.attribute())
                .collect()
        };
        let mut expected = written(|written| matches!(written, Written::Own(_)));
        if convention.fields.iter().any(|field| field.style_property().is_some()) {
            expected.push(String::from(STYLE_ATTRIBUTE));
        }
        expected.extend(written(|written| matches!(written, Written::Data(_))));
        if expected.is_empty() {
            continue;
        }
        let out = one_node_of(element);
        let mut ordered = vec![String::from("class")];
        ordered.extend(expected.iter().cloned());
        assert_eq!(
            attributes_of_first_tag(&out),
            ordered,
            "`{element}` writes its attributes in another order: {out}"
        );
        for attribute in &expected {
            assert!(
                out.contains(&format!("{attribute}=\"")),
                "`{element}` should write `{attribute}`: {out}"
            );
        }
        for found in attributes_of(&out) {
            let known = expected.contains(&found) || found == "class";
            assert!(known, "`{element}` wrote `{found}`, which its row does not name: {out}");
        }
    }
}

/// The attribute names of the first tag of a piece of HTML, in the order
/// written: the element a node's own row is about, the elements it holds being
/// the emitter's own shapes.
fn attributes_of_first_tag(html: &str) -> Vec<String> {
    let end = html.find('>').map_or(html.len(), |at| at + 1);
    attributes_of(&html[..end])
}

/// Every attribute name occurring in a piece of HTML, in the order met.
fn attributes_of(html: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find('<') {
        let tail = &rest[at..];
        let end = tail.find('>').unwrap_or(tail.len());
        for piece in tail[..end].split(' ').skip(1) {
            if let Some((name, _)) = piece.split_once('=') {
                names.push(String::from(name));
            }
        }
        rest = &tail[end.min(tail.len() - 1) + 1..];
    }
    names
}

/// One node of `element`, with a populated payload, written on its own.
fn one_node_of(element: &str) -> String {
    let mut site = site();
    let payload = populated_payloads()
        .into_iter()
        .find(|(name, _)| *name == element)
        .map(|(_, payload)| payload)
        .expect("a payload for every element");
    let body = match element {
        pres::LINES => {
            let text = site.text("a");
            vec![site.pres(pres::LINE, LinePayload::default(), vec![text])]
        }
        pres::IMAGE | pres::UNSUPPORTED | pres::DECO_MARK | pres::SUBTREE => Vec::new(),
        _ => vec![site.text("content")],
    };
    let node = site.pres_payload(element, payload, body);
    let tree = site.finish(vec![node]);
    written(tree)
}

#[test]
fn the_printed_table_is_the_golden_the_guide_holds() {
    assert_golden("conventions.md", &markdown_table());
}

#[test]
fn the_rendering_guide_holds_the_same_table() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("docs")
        .join("rendering.md");
    let Ok(guide) = std::fs::read_to_string(&path) else {
        // A packaged crate ships no guide beside its sources.
        eprintln!("skipped: {} is not present", path.display());
        return;
    };
    assert!(
        guide.contains(markdown_table().trim_end()),
        "`docs/rendering.md` does not hold the conventions table as \
         `conventions::markdown_table()` prints it"
    );
}

#[test]
fn every_style_definition_a_row_names_is_registered() {
    // A row names the style definition an element needs; the identifier must
    // be one the standard package's `html` feature actually registers, or the
    // requirement would resolve to nothing and the element would reach a page
    // with no rules for it.
    let registry = std_package::registry(flm_core::stdcore::StdcoreConfig::new());
    let registered: Vec<&str> =
        registry.styles().iter().map(|(_, style)| style.id.as_str()).collect();
    for convention in CONVENTIONS {
        if let Some(style) = convention.style {
            assert!(
                registered.contains(&style),
                "`{}` names `{style}`, which no feature registers; registered: {registered:?}",
                convention.element
            );
        }
    }
    // The script definition the emitter requires at a formula is registered
    // the same way.
    let scripts: Vec<&str> =
        registry.scripts().iter().map(|(_, script)| script.id.as_str()).collect();
    assert!(scripts.contains(&SCRIPT_MATHJAX), "{scripts:?}");
}

#[test]
fn a_dropped_field_carries_the_reason_it_is_dropped() {
    for convention in CONVENTIONS {
        for field in convention.fields {
            if let Written::Dropped(why) = field.written {
                assert!(!why.is_empty(), "`{}`'s `{}`", convention.element, field.field);
            }
        }
    }
}
