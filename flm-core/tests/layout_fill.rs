//! What `flm_core::layout::DocBuf::fill_words` treats as the space between two
//! words: a line may break at ordinary whitespace, and never inside a word two
//! non-breaking spaces join.

mod common;

use common::layout::assert_well_formed;
use flm_core::layout::{DocBuf, DocId, Profile, print};

/// Renders at `width`, checking the invariants that hold for every output.
fn render(buf: &DocBuf, root: DocId, width: Option<u32>) -> String {
    let out = print(buf, root, &Profile::new(width));
    assert_well_formed(&out);
    out
}

#[test]
fn a_non_breaking_space_keeps_its_two_words_on_one_line() {
    let mut b = DocBuf::new();
    let d = b.fill_words("aaaa bbbb cccc dddd eeee Theorem\u{00A0}1 ffff");
    // The joined word measures nine columns and does not fit on the first
    // line, so the whole of it moves to the next one.
    assert_eq!(
        render(&b, d, Some(33)),
        "aaaa bbbb cccc dddd eeee\nTheorem\u{00A0}1 ffff\n"
    );
}

#[test]
fn a_non_breaking_space_is_printed_as_itself() {
    let mut b = DocBuf::new();
    let d = b.fill_words("Figure\u{00A0}1");
    assert_eq!(render(&b, d, None), "Figure\u{00A0}1\n");
}

#[test]
fn the_figure_space_and_the_narrow_no_break_space_are_not_break_opportunities() {
    for space in ['\u{2007}', '\u{202F}'] {
        let mut b = DocBuf::new();
        let d = b.fill_words(&format!("aaaa bbbb cccc Theorem{space}1 dddd"));
        let out = render(&b, d, Some(20));
        assert!(out.contains(&format!("Theorem{space}1")), "{out:?}");
    }
}

#[test]
fn ordinary_whitespace_is_a_break_opportunity_and_is_dropped() {
    let mut b = DocBuf::new();
    let d = b.fill_words("  one\ttwo\nthree  four \u{2003}five  ");
    assert_eq!(render(&b, d, Some(9)), "one two\nthree\nfour five\n");
}
