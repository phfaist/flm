//! Whole-layout golden tests of `flm_core::layout`: four layouts that the
//! Python implementation's text and Markdown renderers each get wrong, rendered
//! against the goldens under `tests/goldens/layout/`, and rendered twice to show
//! that each is deterministic.
//!
//! The documents here are built the way a text emitter builds them — list
//! labels as `text` atoms, bodies through `fill_words`, `align` for hanging
//! indents, `blocksep` for paragraph separation. Nothing post-processes the
//! output, and a golden pins the pair of document and width and nothing else.
//!
//! `UPDATE_GOLDEN=1 cargo test` rewrites `tests/goldens/layout/*.txt`.

mod common;

use common::layout::{assert_max_width, assert_well_formed, golden_path};
use flm_core::dump::golden::assert_text_golden;
use flm_core::layout::{print, ColSpec, DocBuf, DocId, Profile, RawPolicy};

/// (a) Paragraph wrap with a hanging indent: continuation lines line up
/// under the first character of the item text, at column 4, not at column 0.
#[test]
fn hanging_indent() {
    let mut b = DocBuf::new();
    let bullet = b.text("  • ");
    let body = b.fill_words(
        "This is a fairly long paragraph of text inside a list item which really would \
         like to be wrapped at some sensible width like 72 or 80 columns, and whose \
         continuation lines ought to be aligned under the first character of the item \
         text rather than at column zero.",
    );
    let hung = b.align(body);
    let doc = b.concat(&[bullet, hung]);

    let out = print(&b, doc, &Profile::new(Some(72)));
    assert_well_formed(&out);
    assert_max_width(&out, 72);
    // Every continuation line starts at column 4.
    for line in out.lines().skip(1) {
        assert!(line.starts_with("    ") && !line.starts_with("     "), "{line:?}");
    }
    assert_text_golden(golden_path("hanging_indent.txt"), &out);
}

/// (b) A nested list at width 40: an outer numbered list whose first item
/// has a wrapping body *and* a nested bullet list. Aligned continuations at
/// both levels; inner items separated by a plain `hardline`, outer items by
/// `blocksep(1)`.
#[test]
fn nested_list() {
    let mut b = DocBuf::new();

    let inner = {
        let mut items = Vec::new();
        for text in [
            "the first inner bullet, whose text is long enough that it has to wrap",
            "a second inner bullet, shorter but still wrapping once",
        ] {
            if !items.is_empty() {
                items.push(b.hardline());
            }
            let label = b.text("  - ");
            let body = b.fill_words(text);
            let hung = b.align(body);
            items.push(b.concat(&[label, hung]));
        }
        b.concat(&items)
    };

    let item1 = {
        let label = b.text("  1. ");
        let body = b.fill_words(
            "The first outer item has a body that is comfortably longer than forty \
             columns, and it is followed by a nested list.",
        );
        let hl = b.hardline();
        let content = b.concat(&[body, hl, inner]);
        let hung = b.align(content);
        b.concat(&[label, hung])
    };

    let item2 = {
        let label = b.text("  2. ");
        let body = b.fill_words(
            "The second outer item is separated from the first by one blank line.",
        );
        let hung = b.align(body);
        b.concat(&[label, hung])
    };

    let sep = b.blocksep(1);
    let doc = b.concat(&[item1, sep, item2]);

    let out = print(&b, doc, &Profile::new(Some(40)));
    assert_well_formed(&out);
    assert_max_width(&out, 40);
    assert_text_golden(golden_path("nested_list.txt"), &out);
}

/// (c) A table with a header row and cells that must wrap, at width 44.
///
/// The content is chosen so the proportional solver is actually exercised:
/// Σmin = 20 < available = 40 < Σmax = 54.
#[test]
fn table_wrapped() {
    let mut b = DocBuf::new();

    let cell = |b: &mut DocBuf, s: &str| b.fill_words(s);
    let h0 = cell(&mut b, "Name");
    let h1 = cell(&mut b, "City of residence");
    let h2 = cell(&mut b, "Height");
    let a0 = cell(&mut b, "John Doe the third");
    let a1 = cell(&mut b, "Berlin, the capital of Germany");
    let a2 = cell(&mut b, "1m80");
    let b0 = cell(&mut b, "Al");
    let b1 = cell(&mut b, "Zurich");
    let b2 = cell(&mut b, "1m72");

    let doc = b.table(
        &[ColSpec::left(), ColSpec::left(), ColSpec::right()],
        &[&[h0, h1, h2], &[a0, a1, a2], &[b0, b1, b2]],
        1,
    );

    let out = print(&b, doc, &Profile::new(Some(44)));
    assert_well_formed(&out);
    assert_max_width(&out, 44);
    // mins  = [5, 9, 6]  (widest atom: "third", "residence", "Height")
    // maxs  = [18, 30, 6] (flat width of the widest cell)
    // available = 44 - 2*2 = 40; surplus 20 over spans [13, 21, 0] (total 34)
    //   => [7 r22, 12 r12, 0]; leftover 1 to col0 => widths [13, 21, 6].
    assert_eq!(out.lines().next().unwrap().find("City"), Some(15));
    assert_text_golden(golden_path("table_wrapped.txt"), &out);
}

/// (d) A fenced code block inside a blockquote, at width 40 — the case the
/// Python Markdown renderer mangles in three separate ways.
#[test]
fn blockquote_code() {
    let mut b = DocBuf::new();

    let intro = b.fill_words(
        "Here is a paragraph inside a blockquote; it has to wrap two columns \
         earlier than usual because of the quote marker.",
    );
    let s1 = b.blocksep(1);
    let code = b.raw(
        "```\ndef f(x):\n\n    return x + 1\n```",
        RawPolicy::Indented,
    );
    let s2 = b.blocksep(1);
    let outro = b.fill_words("And a closing paragraph after the code block.");
    let body = b.concat(&[intro, s1, code, s2, outro]);
    let doc = b.prefix("> ", body);

    let out = print(&b, doc, &Profile::new(Some(40)));
    assert_well_formed(&out);
    assert_max_width(&out, 40);
    // Every line belongs to the quote; the blank lines carry a bare ">".
    for line in out.lines() {
        assert!(line == ">" || line.starts_with("> "), "{line:?}");
    }
    // The blank line inside the fenced code survived.
    assert!(out.contains("> def f(x):\n>\n>     return x + 1"));
    assert_text_golden(golden_path("blockquote_code.txt"), &out);
}

/// The goldens are stable across runs and across a rebuilt arena.
#[test]
fn goldens_are_deterministic() {
    let mut b = DocBuf::new();
    let bullet = b.text("  • ");
    let body = b.fill_words("determinism is a hard requirement for this engine, always");
    let hung = b.align(body);
    let doc = b.concat(&[bullet, hung]);

    let p = Profile::new(Some(30));
    let a = print(&b, doc, &p);
    let b2 = print(&b, doc, &p);
    assert_eq!(a, b2);
}

/// The table and blank-line goldens are stable too: the table solver
/// (integer largest-remainder, ties by column index) and the blank-line rule
/// are pure.
#[test]
fn table_and_blank_line_goldens_are_deterministic() {
    fn build(b: &mut DocBuf) -> DocId {
        let h0 = b.fill_words("Name");
        let h1 = b.fill_words("City of residence");
        let t = b.table(&[ColSpec::left(), ColSpec::left()], &[&[h0, h1]], 1);
        let s = b.blocksep(1);
        let r = b.raw("```\nx\n\ny\n```", RawPolicy::Indented);
        let body = b.concat(&[t, s, r]);
        b.prefix("> ", body)
    }

    let mut b = DocBuf::new();
    let doc = build(&mut b);
    let p = Profile::new(Some(44));
    let first = print(&b, doc, &p);
    assert_eq!(first, print(&b, doc, &p));

    // Same document built in a fresh (and in a cleared) arena.
    let mut b2 = DocBuf::new();
    let doc2 = build(&mut b2);
    assert_eq!(first, print(&b2, doc2, &p));

    b2.clear();
    let doc3 = build(&mut b2);
    assert_eq!(first, print(&b2, doc3, &p));
}
