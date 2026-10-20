//! The block constructs of `flm_core::layout`: `prefix` and the generalized
//! indentation stack, the blank-line longest-common-prefix rule, `raw`, and
//! `table`.

mod common;

use common::layout::{assert_max_width, assert_well_formed};
use flm_core::layout::{print, ColAlign, ColSpec, DocBuf, DocId, Profile, RawPolicy, TableCell};

fn render(buf: &DocBuf, root: DocId, width: Option<u32>) -> String {
    let out = print(buf, root, &Profile::new(width));
    assert_well_formed(&out);
    out
}

// ============================================================================
// prefix
// ============================================================================

/// A prefix costs budget: the same paragraph wraps earlier inside
/// `prefix("> ")`.
#[test]
fn a_prefix_narrows_the_wrap_budget() {
    let mut b = DocBuf::new();
    let bare = b.fill_words("aaa bbb ccc ddd");
    let quoted = b.prefix("> ", bare);

    assert_eq!(render(&b, bare, Some(12)), "aaa bbb ccc\nddd\n");
    // Budget is 12 - 2 = 10 columns of content.
    assert_eq!(render(&b, quoted, Some(12)), "> aaa bbb\n> ccc ddd\n");
    assert_max_width(&render(&b, quoted, Some(12)), 12);
}

/// Prefixes compose by nesting.
#[test]
fn nested_prefixes_compose() {
    let mut b = DocBuf::new();
    let body = b.fill_words("aaa bbb ccc ddd");
    let inner = b.prefix("> ", body);
    let outer = b.prefix("> ", inner);

    // Budget is 12 - 4 = 8.
    assert_eq!(render(&b, outer, Some(12)), "> > aaa bbb\n> > ccc ddd\n");
}

/// The stack is emitted in nesting order, so a prefix inside a `nest`
/// indents then quotes, and a `nest` inside a prefix quotes then indents.
#[test]
fn the_indentation_stack_is_emitted_in_nesting_order() {
    let mut b = DocBuf::new();
    let body = {
        let a = b.text("aaa");
        let hl = b.hardline();
        let c = b.text("bbb");
        b.concat(&[a, hl, c])
    };

    let quote_in_list = {
        let q = b.prefix("> ", body);
        b.nest(4, q)
    };
    assert_eq!(render(&b, quote_in_list, Some(80)), "    > aaa\n    > bbb\n");

    let list_in_quote = {
        let n = b.nest(4, body);
        b.prefix("> ", n)
    };
    assert_eq!(render(&b, list_in_quote, Some(80)), ">     aaa\n>     bbb\n");
}

/// Entering a prefix scope mid-line only affects the following lines;
/// entering it at a line start prefixes that line too.
#[test]
fn a_prefix_entered_mid_line_starts_at_the_next_line() {
    let mut b = DocBuf::new();
    let body = {
        let a = b.text("aaa");
        let hl = b.hardline();
        let c = b.text("bbb");
        b.concat(&[a, hl, c])
    };
    let q = b.prefix("> ", body);

    // Line start: the prefix applies from the first line.
    assert_eq!(render(&b, q, Some(80)), "> aaa\n> bbb\n");

    // Mid-line: the first line was already opened without the prefix.
    let mid = {
        let head = b.text("head ");
        b.concat(&[head, q])
    };
    assert_eq!(render(&b, mid, Some(80)), "head aaa\n> bbb\n");
}

/// `align` inside a prefix hangs relative to the prefixed column.
#[test]
fn align_inside_a_prefix_hangs_under_the_label() {
    let mut b = DocBuf::new();
    let label = b.text("1. ");
    let body = b.fill_words("alpha beta gamma delta");
    let hung = b.align(body);
    let item = b.concat(&[label, hung]);
    let quoted = b.prefix("> ", item);

    // "> 1. " is 5 columns; continuation lines hang at column 5, under the
    // prefix + label, and wrap against the same 16-column limit.
    let out = render(&b, quoted, Some(16));
    assert_eq!(out, "> 1. alpha beta\n>    gamma delta\n");
    assert_max_width(&out, 16);
}

// ============================================================================
// blank lines: the longest-common-prefix rule
// ============================================================================

/// A blank line between two paragraphs of one blockquote keeps the ">" —
/// Markdown then reads one blockquote, not two.
#[test]
fn a_blank_line_inside_a_quote_keeps_the_marker() {
    let mut b = DocBuf::new();
    let p1 = b.fill_words("first paragraph");
    let sep = b.blocksep(1);
    let p2 = b.fill_words("second paragraph");
    let body = b.concat(&[p1, sep, p2]);
    let q = b.prefix("> ", body);

    assert_eq!(
        render(&b, q, Some(80)),
        "> first paragraph\n>\n> second paragraph\n"
    );
}

/// A blank line between a plain paragraph and a following blockquote is
/// empty: the quote starts fresh.
#[test]
fn a_blank_line_before_a_quote_is_empty() {
    let mut b = DocBuf::new();
    let p1 = b.fill_words("plain paragraph");
    let sep = b.blocksep(1);
    let inner = b.fill_words("quoted paragraph");
    let q = b.prefix("> ", inner);
    let doc = b.concat(&[p1, sep, q]);

    assert_eq!(
        render(&b, doc, Some(80)),
        "plain paragraph\n\n> quoted paragraph\n"
    );

    // ...and symmetrically on the way out of the quote.
    let out = {
        let p = b.fill_words("after the quote");
        let s = b.blocksep(1);
        b.concat(&[q, s, p])
    };
    assert_eq!(
        render(&b, out, Some(80)),
        "> quoted paragraph\n\nafter the quote\n"
    );
}

/// Inside nested quotes, a scope-internal blank line carries both markers.
#[test]
fn a_blank_line_inside_nested_quotes_keeps_both_markers() {
    let mut b = DocBuf::new();
    let p1 = b.fill_words("first");
    let sep = b.blocksep(1);
    let p2 = b.fill_words("second");
    let body = b.concat(&[p1, sep, p2]);
    let inner = b.prefix("> ", body);
    let outer = b.prefix("> ", inner);

    assert_eq!(render(&b, outer, Some(80)), "> > first\n> >\n> > second\n");

    // Leaving the inner quote drops back to the common "> ".
    let mixed = {
        let a = b.fill_words("inner");
        let q = b.prefix("> ", a);
        let s = b.blocksep(1);
        let c = b.fill_words("outer");
        let body = b.concat(&[q, s, c]);
        b.prefix("> ", body)
    };
    assert_eq!(render(&b, mixed, Some(80)), "> > inner\n>\n> outer\n");
}

/// Pure-`Spaces` stacks trim away, so indentation-only behavior is preserved
/// exactly.
#[test]
fn blank_lines_under_pure_indentation_are_truly_empty() {
    let mut b = DocBuf::new();
    let a = b.text("a");
    let sep = b.blocksep(1);
    let c = b.text("b");
    let body = b.concat(&[a, sep, c]);
    let doc = b.nest(4, body);

    assert_eq!(render(&b, doc, Some(80)), "    a\n\n    b\n");
}

/// Two **sibling** `prefix("> ")` scopes are two different scopes, so the
/// blank line between them is empty and Markdown reads two blockquotes.
#[test]
fn a_blank_line_between_sibling_quotes_is_empty() {
    let mut b = DocBuf::new();
    let q1 = {
        let p = b.fill_words("first quote");
        b.prefix("> ", p)
    };
    let sep = b.blocksep(1);
    let q2 = {
        let p = b.fill_words("second quote");
        b.prefix("> ", p)
    };
    let doc = b.concat(&[q1, sep, q2]);

    assert_eq!(
        render(&b, doc, Some(80)),
        "> first quote\n\n> second quote\n"
    );

    // Scope identity is per *visit*, so even re-using one `prefix` node twice
    // gives two scopes.
    let twice = b.concat(&[q1, sep, q1]);
    assert_eq!(render(&b, twice, Some(80)), "> first quote\n\n> first quote\n");
}

/// `nest(2, nest(3, ..))` renders the same 5 columns as `nest(5, ..)`:
/// adjacent `Spaces` units are merged into one. The merge keeps the parent,
/// so it never hides a `Prefix` ancestor from the blank-line rule.
#[test]
fn adjacent_space_units_are_merged() {
    let mut b = DocBuf::new();
    let inner = {
        let a = b.text("a");
        let n3 = b.nest(3, a);
        b.nest(2, n3)
    };
    let sep = b.blocksep(1);
    let outer = {
        let c = b.text("b");
        let q = b.prefix("| ", c);
        b.nest(5, q)
    };
    let doc = b.concat(&[inner, sep, outer]);

    // Different scopes, so nothing is shared; and the indentation would trim
    // away on a blank line in any case.
    assert_eq!(render(&b, doc, Some(80)), "     a\n\n     | b\n");

    // An indented quote *within one scope* keeps both the indent and the
    // marker on its internal blank line.
    let one_scope = {
        let p1 = b.text("x");
        let s = b.blocksep(1);
        let p2 = b.text("y");
        let body = b.concat(&[p1, s, p2]);
        let q = b.prefix("> ", body);
        b.nest(4, q)
    };
    assert_eq!(render(&b, one_scope, Some(80)), "    > x\n    >\n    > y\n");
}

// ============================================================================
// raw
// ============================================================================

/// Blank-line runs inside raw survive exactly. (Python-Markdown's renderer
/// runs a `re.sub` over the finished string and collapses these.)
#[test]
fn raw_preserves_runs_of_blank_lines() {
    let mut b = DocBuf::new();
    let r = b.raw("one\n\n\ntwo", RawPolicy::Indented);

    assert_eq!(render(&b, r, Some(80)), "one\n\n\ntwo\n");

    // ...and under a prefix, the blank code lines keep the quote marker.
    let q = b.prefix("> ", r);
    assert_eq!(render(&b, q, Some(80)), "> one\n>\n>\n> two\n");
}

/// Raw is never reflowed or split, however long its lines are.
#[test]
fn raw_is_never_reflowed() {
    let mut b = DocBuf::new();
    let r = b.raw(
        "a very long verbatim line that would otherwise wrap",
        RawPolicy::Indented,
    );
    let out = render(&b, r, Some(10));
    assert_eq!(out, "a very long verbatim line that would otherwise wrap\n");
}

/// `Indented` raw sits under the whole stack; `Exempt` raw escapes it
/// entirely.
#[test]
fn exempt_raw_starts_at_column_zero() {
    let mut b = DocBuf::new();
    let head = b.text("head");
    let hl = b.hardline();
    let tail = b.text("tail");

    let indented = {
        let r = b.raw("L1\nL2", RawPolicy::Indented);
        let body = b.concat(&[head, hl, r, hl, tail]);
        let q = b.prefix("> ", body);
        b.nest(4, q)
    };
    assert_eq!(
        render(&b, indented, Some(80)),
        "    > head\n    > L1\n    > L2\n    > tail\n"
    );

    let exempt = {
        let r = b.raw("L1\nL2", RawPolicy::Exempt);
        let body = b.concat(&[head, hl, r, hl, tail]);
        let q = b.prefix("> ", body);
        b.nest(4, q)
    };
    assert_eq!(
        render(&b, exempt, Some(80)),
        "    > head\nL1\nL2\n    > tail\n"
    );
}

/// A trailing newline in raw folds into the pending-break mechanism, so a
/// following `blocksep` still merges by max instead of stacking a blank line.
#[test]
fn a_trailing_newline_in_raw_merges_with_the_next_break() {
    let mut b = DocBuf::new();
    let r = b.raw("code\n", RawPolicy::Indented);
    let sep = b.blocksep(1);
    let p = b.text("after");
    let doc = b.concat(&[r, sep, p]);

    assert_eq!(render(&b, doc, Some(80)), "code\n\nafter\n");
}

/// After a raw that does not end in a newline, the column continues after
/// its last line, so a following group wraps against the right budget.
#[test]
fn the_column_continues_after_a_raw_without_a_trailing_newline() {
    let mut b = DocBuf::new();
    let r = b.raw("xx\nyyyy", RawPolicy::Indented);
    let l = b.line();
    let t = b.text("zzzz");
    let c = b.concat(&[l, t]);
    let g = b.group(c);
    let doc = b.concat(&[r, g]);

    // The last raw line is 4 columns; " zzzz" would reach 9 > 8, so it
    // breaks.
    assert_eq!(render(&b, doc, Some(8)), "xx\nyyyy\nzzzz\n");
    assert_eq!(render(&b, doc, Some(9)), "xx\nyyyy zzzz\n");
}

/// Raw content's own trailing spaces are preserved: atoms (and raw) are
/// sacred.
#[test]
fn raw_keeps_its_own_trailing_spaces() {
    let mut b = DocBuf::new();
    let r = b.raw("trailing   \nnext", RawPolicy::Indented);
    let out = print(&b, r, &Profile::new(Some(80)));
    assert_eq!(out, "trailing   \nnext\n");
}

/// A quirk of the printer, pinned as it is: in the fitting scan a
/// single-line raw measures like an atom whatever its policy, so a group
/// holding an `Exempt` single-line raw is judged flat — yet the raw still
/// starts its own line at column 0, and the flat content after it continues
/// on the raw's line. Text and Markdown output never use `Exempt`; a change
/// here must be deliberate.
#[test]
fn a_single_line_exempt_raw_inside_a_flat_group_still_starts_a_line() {
    let mut b = DocBuf::new();
    let a = b.text("aaaa");
    let l1 = b.line();
    let r = b.raw("b", RawPolicy::Exempt);
    let l2 = b.line();
    let c = b.text("c");
    let inner = b.concat(&[l1, r, l2, c]);
    let g = b.group(inner);
    let doc = b.concat(&[a, g]);

    assert_eq!(render(&b, doc, Some(80)), "aaaa\nb c\n");
}

// ============================================================================
// table
// ============================================================================

fn cell(b: &mut DocBuf, s: &str) -> DocId {
    b.fill_words(s)
}

/// When everything fits, columns take their max width.
#[test]
fn table_columns_take_their_max_when_everything_fits() {
    let mut b = DocBuf::new();
    let a1 = cell(&mut b, "alpha");
    let b1 = cell(&mut b, "x");
    let a2 = cell(&mut b, "b");
    let b2 = cell(&mut b, "yy");
    let t = b.table(&[ColSpec::left(), ColSpec::left()], &[&[a1, b1], &[a2, b2]], 0);

    // maxes are 5 and 2; available is 40 - 2 = 38.
    assert_eq!(render(&b, t, Some(40)), "alpha  x\nb      yy\n");
}

/// The proportional distribution, with the arithmetic spelled out.
#[test]
fn table_distributes_the_surplus_proportionally() {
    let mut b = DocBuf::new();
    let a = cell(&mut b, "aaaa bb");
    let c = cell(&mut b, "ccccccc dddddddddd");
    let t = b.table(&[ColSpec::left(), ColSpec::left()], &[&[a, c]], 0);

    // available = 20 - 2 gutters = 18.  mins = [4, 10] (widest atom),
    // maxs = [7, 18] (flat width).  Sum(max) = 25 > 18, so:
    //   surplus = 18 - 14 = 4, spans = [3, 8], total = 11
    //   col0: 4*3/11 = 1 rem 1 ; col1: 4*8/11 = 2 rem 10
    //   leftover 1 goes to the largest remainder (col1)
    //   => widths = [4+1, 10+3] = [5, 13], total 5 + 2 + 13 = 20.
    let out = render(&b, t, Some(20));
    assert_eq!(out, "aaaa   ccccccc\nbb     dddddddddd\n");
    // Column 1 starts at column 7 = width(col0) + 2 gutter: the solved widths
    // are observable in the output.
    assert_eq!(out.lines().next().unwrap().find("ccccccc"), Some(7));
    assert_eq!(out.lines().nth(1).unwrap().find("dddddddddd"), Some(7));
}

/// When even the minima do not fit, columns are emitted at their minimum and
/// the lines overflow — atoms are never split.
#[test]
fn table_falls_back_to_minimum_widths_and_overflows() {
    let mut b = DocBuf::new();
    let a = cell(&mut b, "aaaaaaaa");
    let c = cell(&mut b, "bbbbbbbb");
    let t = b.table(&[ColSpec::left(), ColSpec::left()], &[&[a, c]], 0);

    let out = print(&b, t, &Profile::new(Some(6)));
    assert_eq!(out, "aaaaaaaa  bbbbbbbb\n");
}

/// Column alignment and the header rule.
#[test]
fn table_header_rule_and_alignment() {
    let mut b = DocBuf::new();
    let h1 = cell(&mut b, "ab");
    let h2 = cell(&mut b, "n");
    let d1 = cell(&mut b, "c");
    let d2 = cell(&mut b, "1234");
    let t = b.table(
        &[ColSpec::left(), ColSpec::right()],
        &[&[h1, h2], &[d1, d2]],
        1,
    );

    // widths = maxes = [2, 4].
    assert_eq!(render(&b, t, Some(40)), "ab     n\n--  ----\nc   1234\n");
}

/// A cell's own alignment replaces its column's for that cell alone; a cell
/// that gives none follows the column, and the column's width is solved from
/// the content alone.
#[test]
fn table_cell_alignment_replaces_the_columns() {
    let mut b = DocBuf::new();
    let wide = cell(&mut b, "wide cell");
    let own = cell(&mut b, "own");
    let follows = cell(&mut b, "col");
    let centered = cell(&mut b, "mid");
    let t = b.table(
        &[ColSpec::left()],
        &[
            &[TableCell::new(wide)],
            &[TableCell::aligned(own, ColAlign::Right)],
            &[TableCell::new(follows)],
            &[TableCell::aligned(centered, ColAlign::Center)],
        ],
        0,
    );

    // width 9: "own" padded 6 on the left, "col" left as its column says,
    // "mid" 3 on each side (trailing padding trimmed).
    assert_eq!(render(&b, t, Some(40)), "wide cell
      own
col
   mid
");
}

/// A right-aligned column in which one cell asks for the left: the other cells
/// of the column keep the column's alignment, and the header rule is drawn as
/// before.
#[test]
fn table_cell_alignment_under_a_header_row() {
    let mut b = DocBuf::new();
    let h1 = cell(&mut b, "name");
    let h2 = cell(&mut b, "count");
    let d1 = cell(&mut b, "a");
    let d2 = cell(&mut b, "1");
    let e1 = cell(&mut b, "b");
    let e2 = cell(&mut b, "2");
    let t = b.table(
        &[ColSpec::left(), ColSpec::right()],
        &[
            &[TableCell::new(h1), TableCell::aligned(h2, ColAlign::Left)],
            &[TableCell::new(d1), TableCell::new(d2)],
            &[TableCell::aligned(e1, ColAlign::Right), TableCell::new(e2)],
        ],
        1,
    );

    assert_eq!(
        render(&b, t, Some(40)),
        "name  count
----  -----
a         1
   b      2
"
    );
}

/// The layout a spanned cell takes: its content in the span's first position
/// and an empty document under the rest of the span. The empty positions print
/// nothing and take no width, and the spanned content widens its own column.
#[test]
fn table_an_empty_document_under_a_span_prints_nothing() {
    let mut b = DocBuf::new();
    let spanned = cell(&mut b, "spanning two");
    let empty = b.concat(&[]);
    let x = cell(&mut b, "x");
    let y = cell(&mut b, "y");
    let t = b.table(
        &[ColSpec::left(), ColSpec::left()],
        &[
            &[TableCell::aligned(spanned, ColAlign::Center), TableCell::new(empty)],
            &[TableCell::new(x), TableCell::new(y)],
        ],
        0,
    );

    assert_eq!(render(&b, t, Some(40)), "spanning two
x             y
");
}

/// A table nested thirty deep, each level one cell beside a word, prints in
/// time: every cell is rendered once per width it is asked at, whatever the
/// depth.
#[test]
fn table_nested_thirty_deep_prints() {
    let mut b = DocBuf::new();
    let mut inner = cell(&mut b, "core");
    for _ in 0..30 {
        let word = cell(&mut b, "w");
        inner = b.table(&[ColSpec::left(), ColSpec::left()], &[&[inner, word]], 0);
    }
    let out = render(&b, inner, Some(200));
    assert_eq!(out.trim_end(), format!("core{}", "  w".repeat(30)));
}

/// Ragged rows: a missing trailing cell is empty.
#[test]
fn table_rows_may_be_ragged() {
    let mut b = DocBuf::new();
    let a1 = cell(&mut b, "aa");
    let b1 = cell(&mut b, "bb");
    let a2 = cell(&mut b, "cc");
    let t = b.table(&[ColSpec::left(), ColSpec::left()], &[&[a1, b1], &[a2]], 0);

    assert_eq!(render(&b, t, Some(40)), "aa  bb\ncc\n");
}

/// A table is a block, and every one of its lines goes through the
/// indentation stack.
#[test]
fn a_table_inside_a_quote_is_prefixed() {
    let mut b = DocBuf::new();
    let a1 = cell(&mut b, "aa");
    let b1 = cell(&mut b, "bb");
    let a2 = cell(&mut b, "cc");
    let b2 = cell(&mut b, "dd");
    let t = b.table(&[ColSpec::left(), ColSpec::left()], &[&[a1, b1], &[a2, b2]], 0);

    let intro = b.text("before");
    let sep = b.blocksep(1);
    let after = b.text("after");
    let body = b.concat(&[intro, sep, t, sep, after]);
    let q = b.prefix("> ", body);

    assert_eq!(
        render(&b, q, Some(40)),
        "> before\n>\n> aa  bb\n> cc  dd\n>\n> after\n"
    );
}

/// With an unbounded profile every column gets its max width.
#[test]
fn table_columns_take_their_max_when_the_width_is_unbounded() {
    let mut b = DocBuf::new();
    let a = cell(&mut b, "aaaa bb");
    let c = cell(&mut b, "ccccccc dddddddddd");
    let t = b.table(&[ColSpec::left(), ColSpec::left()], &[&[a, c]], 0);

    assert_eq!(render(&b, t, None), "aaaa bb  ccccccc dddddddddd\n");
}

/// Entering an `align` mid-line from inside a deeper stack: the stack wins,
/// no underflow.
#[test]
fn align_never_falls_behind_its_stack() {
    let mut b = DocBuf::new();
    let a = b.text("a");
    let body = {
        let x = b.text("x");
        let hl = b.hardline();
        let y = b.text("y");
        let c = b.concat(&[x, hl, y]);
        let al = b.align(c);
        b.nest(6, al)
    };
    let doc = b.concat(&[a, body]);

    assert_eq!(render(&b, doc, Some(80)), "ax\n      y\n");
}

/// Cells are themselves documents: a cell may hold a nested table.
#[test]
fn table_cells_are_documents() {
    let mut b = DocBuf::new();
    let ia = cell(&mut b, "x");
    let ib = cell(&mut b, "y");
    let inner = b.table(&[ColSpec::left(), ColSpec::left()], &[&[ia, ib]], 0);
    let other = cell(&mut b, "z");
    let outer = b.table(&[ColSpec::left(), ColSpec::left()], &[&[inner, other]], 0);

    assert_eq!(render(&b, outer, Some(40)), "x  y  z\n");
}

// -- a table that prints nothing is not a block -------------------------------

/// A table with no columns prints nothing and is not a block: the text
/// around it stays on one line and no break is forced.
#[test]
fn a_table_with_no_columns_prints_nothing_and_is_not_a_block() {
    let mut b = DocBuf::new();
    let a = b.text("aaaa");
    let bb = b.text("bbbb");
    let t = b.table(&[], &[&[a]], 0);
    let doc = b.concat(&[a, t, bb]);

    assert_eq!(render(&b, doc, Some(40)), "aaaabbbb\n");
}

/// A table with columns but no rows prints nothing and is not a block
/// either: an empty `cells` environment is ordinary document input, so this
/// shape is reachable.
#[test]
fn a_table_with_no_rows_prints_nothing_and_is_not_a_block() {
    let mut b = DocBuf::new();
    let a = b.text("aaaa");
    let bb = b.text("bbbb");
    let t = b.table::<DocId>(&[ColSpec::left()], &[], 0);
    let doc = b.concat(&[a, t, bb]);

    assert_eq!(render(&b, doc, Some(40)), "aaaabbbb\n");
}

/// A break pending before an empty table flushes at the next content, as if
/// the table were not there.
#[test]
fn an_empty_table_leaves_a_pending_break_untouched() {
    let mut b = DocBuf::new();
    let a = b.text("aaaa");
    let sep = b.blocksep(1);
    let t = b.table::<DocId>(&[ColSpec::left()], &[], 0);
    let bb = b.text("bbbb");
    let doc = b.concat(&[a, sep, t, bb]);

    assert_eq!(render(&b, doc, Some(40)), "aaaa\n\nbbbb\n");
}

/// The fitting scan agrees: an empty table inside a group has no width and
/// forces no break, so the group still prints flat.
#[test]
fn an_empty_table_inside_a_group_does_not_break_the_group() {
    let mut b = DocBuf::new();
    let a = b.text("aaaa");
    let l = b.line();
    let t = b.table::<DocId>(&[ColSpec::left()], &[], 0);
    let bb = b.text("bbbb");
    let c = b.concat(&[a, l, t, bb]);
    let g = b.group(c);

    assert_eq!(render(&b, g, Some(40)), "aaaa bbbb\n");
}
