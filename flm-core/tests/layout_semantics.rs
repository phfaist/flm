//! Printer semantics of `flm_core::layout`: groups, nest and align, fill,
//! breaks, and determinism. The one case that needs the crate-internal
//! measurement seam lives as a unit test beside the printer instead.

mod common;

use common::layout::{assert_max_width, assert_no_trailing_ws, assert_well_formed};
use flm_core::layout::{print, DocBuf, DocId, Profile};

/// Render at a given width, checking the invariants that must hold for
/// *every* output.
fn render(buf: &DocBuf, root: DocId, width: Option<u32>) -> String {
    let out = print(buf, root, &Profile::new(width));
    assert_well_formed(&out);
    if let Some(w) = width {
        // Only valid for docs whose atoms all fit; every doc in this file that
        // goes through `render` does.
        assert_max_width(&out, w);
    }
    out
}

// -- groups ------------------------------------------------------------------

/// "hello world" is 11 columns: flat at 11, broken at 10.
#[test]
fn group_flat_vs_broken_at_the_boundary() {
    let mut b = DocBuf::new();
    let hello = b.text("hello");
    let l = b.line();
    let world = b.text("world");
    let c = b.concat(&[hello, l, world]);
    let g = b.group(c);

    assert_eq!(render(&b, g, Some(11)), "hello world\n");
    assert_eq!(render(&b, g, Some(10)), "hello\nworld\n");
}

/// A `hardline` anywhere inside a group makes the group unflattenable, so its
/// sibling `line`s break too — even at a width where everything would fit.
#[test]
fn hardline_forces_the_enclosing_group_to_break() {
    let mut b = DocBuf::new();
    let a = b.text("aaa");
    let l = b.line();
    let bb = b.text("bbb");
    let hl = b.hardline();
    let cc = b.text("ccc");
    let c = b.concat(&[a, l, bb, hl, cc]);
    let g = b.group(c);

    assert_eq!(render(&b, g, Some(80)), "aaa\nbbb\nccc\n");
}

/// Groups nested inside a flattened group stay flat without any scanning.
#[test]
fn inner_group_stays_flat_inside_a_flat_group() {
    let mut b = DocBuf::new();
    let x = b.text("x");
    let l1 = b.line();
    let y = b.text("y");
    let inner_c = b.concat(&[x, l1, y]);
    let inner = b.group(inner_c);
    let l2 = b.line();
    let z = b.text("z");
    let outer_c = b.concat(&[inner, l2, z]);
    let outer = b.group(outer_c);

    assert_eq!(render(&b, outer, Some(80)), "x y z\n");
    // At width 4 the outer group breaks but the inner one still fits.
    assert_eq!(render(&b, outer, Some(4)), "x y\nz\n");
}

/// `softbreak` is a break opportunity that costs nothing when flat.
#[test]
fn softbreak_is_a_zero_width_break_opportunity() {
    let mut b = DocBuf::new();
    let a = b.text("aaaa");
    let sb = b.softbreak();
    let bb = b.text("bbbb");
    let c = b.concat(&[a, sb, bb]);
    let g = b.group(c);

    assert_eq!(render(&b, g, Some(8)), "aaaabbbb\n");
    assert_eq!(render(&b, g, Some(7)), "aaaa\nbbbb\n");
}

/// A flat `line` produces *deferred* whitespace, so a flat break opportunity
/// at the very end of the output never becomes trailing whitespace.
#[test]
fn a_flat_line_at_the_end_leaves_no_trailing_space() {
    let mut b = DocBuf::new();
    let a = b.text("a");
    let l = b.line();
    let c = b.concat(&[a, l]);
    let g = b.group(c);

    assert_eq!(render(&b, g, Some(80)), "a\n");
}

/// ...and a flat `line` at the *start* of a line is dropped rather than
/// printed in front of the indentation.
#[test]
fn a_flat_line_at_a_line_start_is_dropped() {
    let mut b = DocBuf::new();
    let a = b.text("a");
    let hl = b.hardline();
    let l = b.line();
    let bb = b.text("b");
    let inner = b.concat(&[l, bb]);
    let g = b.group(inner);
    let d = b.concat(&[a, hl, g]);

    assert_eq!(render(&b, d, Some(80)), "a\nb\n");
}

/// A limitation of the printer, pinned as it is: the fitting scan treats
/// indentation as inert. An indentation node entered *inside* a
/// group at a line start does move that group's first line, but the scan does
/// not charge it: `group(nest(4, "aaaa bbbb"))` right after a break is judged
/// to fit at width 9 and prints 13 columns, whereas the same content with the
/// `nest` outside the group breaks correctly. Emitters keep indentation
/// outside groups; a change here must be deliberate.
#[test]
fn indentation_entered_inside_a_group_at_a_line_start_is_not_charged() {
    let mut b = DocBuf::new();
    let x = b.text("x");
    let hl = b.hardline();
    let a = b.text("aaaa");
    let l = b.line();
    let bb = b.text("bbbb");
    let body = b.concat(&[a, l, bb]);

    let nest_inside = {
        let n = b.nest(4, body);
        let g = b.group(n);
        b.concat(&[x, hl, g])
    };
    let out = print(&b, nest_inside, &Profile::new(Some(9)));
    assert_no_trailing_ws(&out);
    assert_eq!(out, "x\n    aaaa bbbb\n");

    let nest_outside = {
        let g = b.group(body);
        let n = b.nest(4, g);
        b.concat(&[x, hl, n])
    };
    assert_eq!(render(&b, nest_outside, Some(9)), "x\n    aaaa\n    bbbb\n");
}

// -- nest vs align -----------------------------------------------------------

/// `align` picks up the column reached at entry; `nest` adds a fixed offset
/// to the enclosing indent.
#[test]
fn align_hangs_under_the_label_nest_uses_a_fixed_offset() {
    let mut b = DocBuf::new();
    let body = {
        let first = b.text("first");
        let hl = b.hardline();
        let second = b.text("second");
        b.concat(&[first, hl, second])
    };

    let aligned = {
        let label = b.text("note: ");
        let a = b.align(body);
        b.concat(&[label, a])
    };
    let nested = {
        let label = b.text("note: ");
        let n = b.nest(2, body);
        b.concat(&[label, n])
    };

    // "note: " is 6 columns, so the continuation lands at column 6.
    assert_eq!(render(&b, aligned, Some(80)), "note: first\n      second\n");
    // nest is relative to the *enclosing* indent (0 here), not to the column.
    assert_eq!(render(&b, nested, Some(80)), "note: first\n  second\n");
}

/// When a break is already pending, "the current column" is the column the
/// next character will land on — the indent in effect — not the stale column
/// of the finished line.
#[test]
fn align_after_a_pending_break_uses_the_indent_not_the_stale_column() {
    let mut b = DocBuf::new();
    let head = b.text("abcdef");
    let hl = b.hardline();
    let p = b.text("p");
    let hl2 = b.hardline();
    let q = b.text("q");
    let pq = b.concat(&[p, hl2, q]);
    let a = b.align(pq);
    let n = b.nest(2, a);
    let d = b.concat(&[head, hl, n]);

    assert_eq!(render(&b, d, Some(80)), "abcdef\n  p\n  q\n");
}

// -- fill --------------------------------------------------------------------

/// Per-separator groups give greedy word wrapping.
#[test]
fn fill_words_wraps_greedily() {
    let mut b = DocBuf::new();
    let d = b.fill_words(
        "the quick brown fox jumps over the lazy dog and then some more words here",
    );

    let out = render(&b, d, Some(30));
    assert_eq!(
        out,
        "the quick brown fox jumps over\n\
         the lazy dog and then some\n\
         more words here\n"
    );
    // The first line is exactly at the limit.
    assert_eq!(out.lines().next().unwrap().chars().count(), 30);
}

/// THE regression this engine exists for.
///
/// The fill's last separator must see the trailing atom that follows the fill
/// — which has no break opportunity of its own — and break in front of "hhh"
/// so that "hhh]]]]]]]]]]" fits. A fits scan that stops at the end of the
/// group puts "hhh" on the first line (it fits by itself) and then overflows
/// by 9 columns. That is precisely the Python text renderer's failure mode.
#[test]
fn fill_accounts_for_text_that_follows_the_group() {
    let mut b = DocBuf::new();
    let fill = b.fill_words("aaa bbb ccc ddd eee fff ggg hhh");
    let tail = b.text("]]]]]]]]]]");
    let d = b.concat(&[fill, tail]);

    let out = render(&b, d, Some(32));
    assert_eq!(out, "aaa bbb ccc ddd eee fff ggg\nhhh]]]]]]]]]]\n");
    assert_max_width(&out, 32);
    // Sanity: the naive answer would have been 41 columns wide.
    assert_eq!("aaa bbb ccc ddd eee fff ggg hhh]]]]]]]]]]".len(), 41);
}

/// An atom wider than the line is emitted whole: atoms are never split.
#[test]
fn overlong_atom_overflows_rather_than_splitting() {
    let mut b = DocBuf::new();
    let d = b.fill_words("ab supercalifragilistic cd");

    let out = print(&b, d, &Profile::new(Some(10)));
    assert_no_trailing_ws(&out);
    assert_eq!(out, "ab\nsupercalifragilistic\ncd\n");
}

// -- unbounded width ---------------------------------------------------------

/// `width: None` flattens every group with no scanning, but hard structure
/// still breaks.
#[test]
fn unbounded_width_flattens_everything_except_hard_structure() {
    let mut b = DocBuf::new();
    let a = b.text("aaa");
    let l = b.line();
    let sb = b.softbreak();
    let bb = b.text("bbb");
    let hl = b.hardline();
    let cc = b.text("ccc");
    let bs = b.blocksep(1);
    let dd = b.text("ddd");
    let c = b.concat(&[a, l, bb, sb, hl, cc, bs, dd]);
    let g = b.group(c);

    let out = render(&b, g, None);
    assert_eq!(out, "aaa bbb\nccc\n\nddd\n");

    // Even a very long fill stays on one line.
    let mut b2 = DocBuf::new();
    let f = b2.fill_words("one two three four five six seven eight nine ten");
    assert_eq!(
        render(&b2, f, None),
        "one two three four five six seven eight nine ten\n"
    );
}

// -- breaks and blocksep -----------------------------------------------------

/// Adjacent break requests merge by max of their blank-line counts.
#[test]
fn adjacent_breaks_merge_by_max() {
    let mut b = DocBuf::new();
    let a = b.text("a");
    let bb = b.text("b");

    let two_then_one = {
        let s2 = b.blocksep(2);
        let s1 = b.blocksep(1);
        b.concat(&[a, s2, s1, bb])
    };
    assert_eq!(render(&b, two_then_one, Some(80)), "a\n\n\nb\n");

    let one_then_two = {
        let s1 = b.blocksep(1);
        let s2 = b.blocksep(2);
        b.concat(&[a, s1, s2, bb])
    };
    assert_eq!(render(&b, one_then_two, Some(80)), "a\n\n\nb\n");
}

/// A `hardline` next to a `blocksep(1)` is one blank line, never two — in
/// either order. Blank lines come only from `blocksep`.
#[test]
fn hardline_next_to_blocksep_is_one_blank_line() {
    let mut b = DocBuf::new();
    let a = b.text("a");
    let bb = b.text("b");

    let hl_then_sep = {
        let hl = b.hardline();
        let s1 = b.blocksep(1);
        b.concat(&[a, hl, s1, bb])
    };
    assert_eq!(render(&b, hl_then_sep, Some(80)), "a\n\nb\n");

    let sep_then_hl = {
        let s1 = b.blocksep(1);
        let hl = b.hardline();
        b.concat(&[a, s1, hl, bb])
    };
    assert_eq!(render(&b, sep_then_hl, Some(80)), "a\n\nb\n");

    // Two hardlines collapse to a single newline: `blocksep` is the only
    // source of blank lines.
    let two_hardlines = {
        let h1 = b.hardline();
        let h2 = b.hardline();
        b.concat(&[a, h1, h2, bb])
    };
    assert_eq!(render(&b, two_hardlines, Some(80)), "a\nb\n");
}

/// Leading and trailing separation is dropped; the output ends with exactly
/// one newline.
#[test]
fn breaks_are_dropped_at_the_start_and_end_of_the_output() {
    let mut b = DocBuf::new();
    let s2 = b.blocksep(2);
    let hl = b.hardline();
    let a = b.text("a");
    let s1 = b.blocksep(1);
    let hl2 = b.hardline();
    let d = b.concat(&[s2, hl, a, s1, hl2]);

    assert_eq!(render(&b, d, Some(80)), "a\n");
}

/// Blank lines are completely empty even under indentation, and no line
/// carries the indent as trailing whitespace.
#[test]
fn indentation_is_lazy_so_blank_lines_stay_empty() {
    let mut b = DocBuf::new();
    let a = b.text("a");
    let s1 = b.blocksep(1);
    let bb = b.text("b");
    let c = b.concat(&[a, s1, bb]);
    let d = b.nest(4, c);

    let out = render(&b, d, Some(80));
    assert_eq!(out, "    a\n\n    b\n");
    assert_no_trailing_ws(&out);
}

/// An empty document prints as the empty string (no stray newline).
#[test]
fn empty_document_prints_empty() {
    let mut b = DocBuf::new();
    let d = b.concat(&[]);
    assert_eq!(render(&b, d, Some(80)), "");

    let mut b2 = DocBuf::new();
    let e = b2.fill_words("   ");
    assert_eq!(render(&b2, e, Some(80)), "");
}

// -- purity ------------------------------------------------------------------

/// Same input + same profile ⇒ identical bytes.
#[test]
fn printing_is_deterministic() {
    let mut b = DocBuf::new();
    let d = {
        let label = b.text("  1. ");
        let body = b.fill_words(
            "deterministic output is a hard requirement here: golden tests and diffable \
             generated sites depend on it",
        );
        let a = b.align(body);
        b.concat(&[label, a])
    };

    let p = Profile::new(Some(40));
    let first = print(&b, d, &p);
    let second = print(&b, d, &p);
    assert_eq!(first, second);

    // A fresh profile value of the same shape gives the same bytes too.
    let third = print(&b, d, &Profile::new(Some(40)));
    assert_eq!(first, third);
}

/// The arena is reusable: `clear` then rebuild gives the same output.
#[test]
fn docbuf_clear_makes_the_arena_reusable() {
    let mut b = DocBuf::new();
    let d1 = b.fill_words("alpha beta gamma delta");
    let first = render(&b, d1, Some(12));

    b.clear();
    assert!(b.is_empty());
    let d2 = b.fill_words("alpha beta gamma delta");
    let second = render(&b, d2, Some(12));

    assert_eq!(first, second);
    assert_eq!(first, "alpha beta\ngamma delta\n");
}

/// A shared subtree may appear twice; it prints twice.
#[test]
fn shared_subtrees_are_expanded() {
    let mut b = DocBuf::new();
    let shared = b.fill_words("shared bit");
    let hl = b.hardline();
    let d = b.concat(&[shared, hl, shared]);
    assert_eq!(render(&b, d, Some(80)), "shared bit\nshared bit\n");
}
