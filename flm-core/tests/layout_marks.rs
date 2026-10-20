//! Marks in `flm_core::layout`: where a subtree landed in the output.
//!
//! Positions are 0-based lines and 0-based display columns, and the end
//! position is exclusive. Spans are reported in the order the marks opened.

mod common;

use common::layout::assert_well_formed;
use flm_core::layout::{
    print, print_full, ColSpec, DocBuf, DocId, MarkId, PrintOutput, Profile, RawPolicy,
};

/// `(id, start_byte, end_byte, start_line, start_col, end_line, end_col)`
type Span = (u32, u32, u32, u32, u32, u32, u32);

fn spans(out: &PrintOutput) -> Vec<Span> {
    out.marks
        .iter()
        .map(|m| {
            (
                m.id.0,
                m.bytes.start as u32,
                m.bytes.end as u32,
                m.start.line,
                m.start.col,
                m.end.line,
                m.end.col,
            )
        })
        .collect()
}

fn run(buf: &DocBuf, root: DocId, width: Option<u32>) -> PrintOutput {
    let out = print_full(buf, root, &Profile::new(width));
    assert_well_formed(&out.text);
    // `print` is exactly `print_full(..).text`.
    assert_eq!(out.text, print(buf, root, &Profile::new(width)));
    // Every span is a valid slice of the output.
    for m in &out.marks {
        assert!(m.bytes.start <= m.bytes.end);
        assert!(out.text.is_char_boundary(m.bytes.start));
        assert!(out.text.is_char_boundary(m.bytes.end));
    }
    out
}

/// The span starts at the child's first visible character — after the line's
/// indentation bytes.
#[test]
fn a_mark_excludes_the_lines_indentation() {
    let mut b = DocBuf::new();
    let head = b.text("aa ");
    let word = b.text("bb");
    let marked = b.mark(MarkId(7), word);
    let tail = b.text(" cc");
    let body = b.concat(&[head, marked, tail]);
    let doc = b.nest(4, body);

    let out = run(&b, doc, Some(80));
    assert_eq!(out.text, "    aa bb cc\n");
    // "    aa " is 7 bytes and 7 columns.
    assert_eq!(spans(&out), vec![(7, 7, 9, 0, 7, 0, 9)]);
    assert_eq!(&out.text[out.marks[0].bytes.clone()], "bb");
}

/// A span may cover wrapped content: it ends on the last line the child
/// reached.
#[test]
fn a_mark_can_span_several_lines() {
    let mut b = DocBuf::new();
    let body = b.fill_words("aaa bbb ccc ddd");
    let doc = b.mark(MarkId(1), body);

    let out = run(&b, doc, Some(7));
    assert_eq!(out.text, "aaa bbb\nccc ddd\n");
    assert_eq!(spans(&out), vec![(1, 0, 15, 0, 0, 1, 7)]);
    assert_eq!(&out.text[0..15], "aaa bbb\nccc ddd");
}

/// The end excludes deferred whitespace and breaks that flush later: a mark
/// ending right before a `blocksep` does not swallow the blank line.
#[test]
fn a_mark_ends_before_a_following_blocksep() {
    let mut b = DocBuf::new();
    let a = b.text("aa");
    let marked = b.mark(MarkId(2), a);
    let sep = b.blocksep(1);
    let c = b.text("bb");
    let doc = b.concat(&[marked, sep, c]);

    let out = run(&b, doc, Some(80));
    assert_eq!(out.text, "aa\n\nbb\n");
    assert_eq!(spans(&out), vec![(2, 0, 2, 0, 0, 0, 2)]);
}

/// ...and symmetrically, a mark *after* a `blocksep` starts after the blank
/// lines.
#[test]
fn a_mark_starts_after_the_blank_lines_of_a_break() {
    let mut b = DocBuf::new();
    let a = b.text("a");
    let sep = b.blocksep(1);
    let c = b.text("b");
    let marked = b.mark(MarkId(3), c);
    let doc = b.concat(&[a, sep, marked]);

    let out = run(&b, doc, Some(80));
    assert_eq!(out.text, "a\n\nb\n");
    assert_eq!(spans(&out), vec![(3, 3, 4, 2, 0, 2, 1)]);
}

/// A mark's end excludes a trailing flat space that has not been flushed.
#[test]
fn a_mark_excludes_trailing_deferred_space() {
    let mut b = DocBuf::new();
    let a = b.text("aa");
    let l = b.line();
    let inner = b.concat(&[a, l]);
    let marked = b.mark(MarkId(4), inner);
    let c = b.text("bb");
    let outer = b.concat(&[marked, c]);
    let doc = b.group(outer);

    let out = run(&b, doc, Some(80));
    assert_eq!(out.text, "aa bb\n");
    // The space belongs to neither the mark's content nor its span.
    assert_eq!(spans(&out), vec![(4, 0, 2, 0, 0, 0, 2)]);
}

// -- empty and invisible children --------------------------------------------

/// An empty child gives an empty span anchored at the next visible character.
#[test]
fn an_empty_mark_anchors_at_the_next_character() {
    let mut b = DocBuf::new();
    let nothing = b.concat(&[]);
    let marked = b.mark(MarkId(5), nothing);
    let x = b.text("x");
    let doc = b.concat(&[marked, x]);

    let out = run(&b, doc, Some(80));
    assert_eq!(out.text, "x\n");
    assert_eq!(spans(&out), vec![(5, 0, 0, 0, 0, 0, 0)]);
}

/// An *invisible* child (structure only) behaves the same: the anchor skips
/// over the break it caused.
#[test]
fn an_invisible_mark_anchors_after_the_break_it_caused() {
    let mut b = DocBuf::new();
    let a = b.text("a");
    let hl = b.hardline();
    let marked = b.mark(MarkId(6), hl);
    let c = b.text("b");
    let doc = b.concat(&[a, marked, c]);

    let out = run(&b, doc, Some(80));
    assert_eq!(out.text, "a\nb\n");
    assert_eq!(spans(&out), vec![(6, 2, 2, 1, 0, 1, 0)]);
}

/// With nothing following, an empty mark anchors at the end of the content —
/// before the final newline.
#[test]
fn an_empty_mark_at_the_end_anchors_before_the_final_newline() {
    let mut b = DocBuf::new();
    let x = b.text("xy");
    let nothing = b.concat(&[]);
    let marked = b.mark(MarkId(8), nothing);
    let doc = b.concat(&[x, marked]);

    let out = run(&b, doc, Some(80));
    assert_eq!(out.text, "xy\n");
    assert_eq!(spans(&out), vec![(8, 2, 2, 0, 2, 0, 2)]);
}

/// A mark on a completely empty document anchors at the origin.
#[test]
fn a_mark_on_an_empty_document() {
    let mut b = DocBuf::new();
    let nothing = b.concat(&[]);
    let doc = b.mark(MarkId(9), nothing);

    let out = run(&b, doc, Some(80));
    assert_eq!(out.text, "");
    assert_eq!(spans(&out), vec![(9, 0, 0, 0, 0, 0, 0)]);
}

// -- nesting, prefixes, raw, tables ------------------------------------------

/// Marks nest arbitrarily and are reported in open order.
#[test]
fn nested_marks_report_in_open_order() {
    let mut b = DocBuf::new();
    let inner_text = b.text("cd");
    let inner = b.mark(MarkId(20), inner_text);
    let head = b.text("ab");
    let body = b.concat(&[head, inner]);
    let outer = b.mark(MarkId(10), body);
    let tail = b.text("ef");
    let doc = b.concat(&[outer, tail]);

    let out = run(&b, doc, Some(80));
    assert_eq!(out.text, "abcdef\n");
    // Outer opens first, so it is reported first even though it closes last.
    assert_eq!(
        spans(&out),
        vec![(10, 0, 4, 0, 0, 0, 4), (20, 2, 4, 0, 2, 0, 4)]
    );
}

/// Two marks with the same id are both reported: ids need not be unique.
#[test]
fn mark_ids_need_not_be_unique() {
    let mut b = DocBuf::new();
    let x = b.text("xx");
    let mx = b.mark(MarkId(1), x);
    let y = b.text("yy");
    let my = b.mark(MarkId(1), y);
    let doc = b.concat(&[mx, my]);

    let out = run(&b, doc, Some(80));
    assert_eq!(out.text, "xxyy\n");
    assert_eq!(
        spans(&out),
        vec![(1, 0, 2, 0, 0, 0, 2), (1, 2, 4, 0, 2, 0, 4)]
    );
}

/// A shared marked subtree is reported once per occurrence, each at its own
/// position.
#[test]
fn a_shared_mark_is_reported_per_occurrence() {
    let mut b = DocBuf::new();
    let a = b.text("aaaa");
    let marked = b.mark(MarkId(1), a);
    let hl = b.hardline();
    let doc = b.concat(&[marked, hl, marked]);

    let out = run(&b, doc, Some(80));
    assert_eq!(out.text, "aaaa\naaaa\n");
    assert_eq!(
        spans(&out),
        vec![(1, 0, 4, 0, 0, 0, 4), (1, 5, 9, 1, 0, 1, 4)]
    );
}

/// Inside a prefix scope the span starts after the `"> "` bytes, on every
/// line.
#[test]
fn a_mark_inside_a_prefix_excludes_the_marker() {
    let mut b = DocBuf::new();
    let body = b.fill_words("aaa bbb");
    let marked = b.mark(MarkId(11), body);
    let doc = b.prefix("> ", marked);

    let out = run(&b, doc, Some(5));
    assert_eq!(out.text, "> aaa\n> bbb\n");
    assert_eq!(spans(&out), vec![(11, 2, 11, 0, 2, 1, 5)]);
    assert_eq!(&out.text[2..11], "aaa\n> bbb");
}

/// Marks around raw content work normally, including its literal newlines.
#[test]
fn a_mark_around_raw_content() {
    let mut b = DocBuf::new();
    let r = b.raw("x\ny", RawPolicy::Indented);
    let doc = b.mark(MarkId(12), r);

    let out = run(&b, doc, Some(80));
    assert_eq!(out.text, "x\ny\n");
    assert_eq!(spans(&out), vec![(12, 0, 3, 0, 0, 1, 1)]);
}

/// v1 limitation: a mark *inside* a table cell is dropped, because cells are
/// rendered by nested printer runs. A mark *around* the table works.
#[test]
fn marks_inside_table_cells_are_dropped() {
    let mut b = DocBuf::new();
    let c0 = b.text("aa");
    let c0m = b.mark(MarkId(30), c0);
    let c1 = b.text("bb");
    let t = b.table(&[ColSpec::left(), ColSpec::left()], &[&[c0m, c1]], 0);
    let doc = b.mark(MarkId(13), t);

    let out = run(&b, doc, Some(40));
    assert_eq!(out.text, "aa  bb\n");
    // Only the outer mark survives.
    assert_eq!(spans(&out), vec![(13, 0, 6, 0, 0, 0, 6)]);
}

/// Marks have no width and no layout effect: wrapping is identical with and
/// without them.
#[test]
fn marks_do_not_affect_layout() {
    let mut b = DocBuf::new();
    let plain = b.fill_words("aaa bbb ccc ddd eee");
    let inner = b.fill_words("aaa bbb ccc ddd eee");
    let marked = b.mark(MarkId(14), inner);

    for width in [5, 7, 11, 80] {
        assert_eq!(
            print(&b, plain, &Profile::new(Some(width))),
            print(&b, marked, &Profile::new(Some(width))),
            "width {width}"
        );
    }
}

// -- purity ------------------------------------------------------------------

/// Spans are as deterministic as the text.
#[test]
fn mark_reporting_is_deterministic() {
    let mut b = DocBuf::new();
    let label = b.text("  1. ");
    let w1 = b.text("alpha");
    let m1 = b.mark(MarkId(1), w1);
    let sp = b.text(" ");
    let w2 = b.fill_words("beta gamma delta epsilon");
    let m2 = b.mark(MarkId(2), w2);
    let body = b.concat(&[m1, sp, m2]);
    let hung = b.align(body);
    let doc = b.concat(&[label, hung]);

    let p = Profile::new(Some(20));
    let first = print_full(&b, doc, &p);
    let second = print_full(&b, doc, &p);
    assert_eq!(first, second);
    assert!(!first.marks.is_empty());
}
