//! The typographic substitutions applied while text is resolved: the TeX
//! ligatures for dashes, ellipses and quotes, and the straight quotes whose
//! direction is decided by their position. Each family is switched on or off by
//! the options of text resolution ([`BlocksConfig`]).
//!
//! The substitutions are applied inside the normalization scan over a merged
//! text run (the private `text` module), after the merge, so a quote at the
//! start of a run can look at the node before the run (the `\(A_j\)'s` case).
//! Mathematics, verbatim content and the content of an opaque wrapper never
//! reach this module: text resolution applies only to text-mode content
//! outside the [opaque regions](crate::guide::concepts_overview#opaque-regions).
//!
//! When both quote families are on, the ligature family takes the TeX spellings
//! ``` `` ```, `''` and `` ` ``, and the position test decides a **single**
//! straight `'` (`don't` → `don’t`, `'hi'` → `‘hi’`, `\(A_j\)'s` → `’s`). A
//! single `'` becomes the TeX `’` only when the position family is off and the
//! ligature family on.
//!
//! The position test has three rules beyond "opening after whitespace":
//!
//! - a quote right after a dash (`-`, –, —) **opens** (`a--'q'` → `a–‘q’`),
//!   for the straight `'` and the straight `"` alike;
//! - a straight `'` right before an ASCII digit **opens** (`x'9` → `x‘9`);
//! - except in a decade abbreviation, a `'` between a word character and two
//!   digits followed by `s`, which **closes** (`x'90s` → `x’90s`).
//!
//! Every offset this module receives is a character boundary: the scan advances
//! by whole characters ([`substitution_at`] asserts it in debug builds), so
//! accented letters, curly quotes and East Asian text are copied unchanged.

use super::config::BlocksConfig;
use crate::lang::text::is_space;

/// Whether the character `prev`, immediately before a straight quote, puts that
/// quote in an opening position: whitespace ([`is_space`]), an opening bracket,
/// or a dash. The dash rule holds for the straight `"` as for the straight `'`,
/// so that `a--'q'` opens rather than closes.
fn opens_after(prev: char) -> bool {
    is_space(prev) || matches!(prev, '(' | '[' | '{' | '-' | '\u{2013}' | '\u{2014}')
}

/// Whether `c` is a word character for the position test: a letter, a digit or
/// an underscore.
fn is_word(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Whether the straight `'` at byte offset `at` is the apostrophe of a decade
/// abbreviation (`x'90s`): a word character before it, then exactly two ASCII
/// digits and an `s`. Such a quote closes even though a digit follows it, which
/// no other rule of the position test would give.
fn is_decade_abbreviation(text: &str, at: usize, prev: Option<char>) -> bool {
    if !prev.is_some_and(is_word) {
        return false;
    }
    let mut after = text[at..].chars();
    after.next();
    let two_digits = after.next().is_some_and(|c| c.is_ascii_digit())
        && after.next().is_some_and(|c| c.is_ascii_digit());
    two_digits && after.next() == Some('s')
}

/// Whether a straight quote at byte offset `at` of `text` opens (rather than
/// closes): at the very start of the run when no content sibling precedes the
/// run, right after whitespace, an opening bracket or a dash, or (for a single
/// `'` only, when `single` is true) right before an ASCII digit that does not
/// begin a decade abbreviation.
fn opening_position(text: &str, at: usize, prev_is_content: bool, single: bool) -> bool {
    let prev = text[..at].chars().next_back();
    if single
        && text[at..].chars().nth(1).is_some_and(|c| c.is_ascii_digit())
        && !is_decade_abbreviation(text, at, prev)
    {
        return true;
    }
    match prev {
        None => !prev_is_content,
        Some(prev) => opens_after(prev),
    }
}

/// The substitution starting at byte offset `at` of `text` — a character
/// boundary — if the options enable one there: `(source byte length,
/// replacement)`. Longest match first within a family; the families are tried
/// in the order dashes, ellipses, quote ligatures (the pairs and the backtick),
/// the single `'` (decided by position, otherwise the ligature `’`), the `"`
/// decided by position. `prev_is_content` says whether a content sibling (not a
/// comment, and not a whitespace node such as `~`) precedes the run the text
/// belongs to.
pub(crate) fn substitution_at(
    text: &str,
    at: usize,
    config: &BlocksConfig,
    prev_is_content: bool,
) -> Option<(usize, &'static str)> {
    debug_assert!(
        text.is_char_boundary(at),
        "the normalization scan advances by whole characters"
    );
    let rest = &text[at..];
    if config.ligature_unicode_dashes {
        if rest.starts_with("---") {
            return Some((3, "\u{2014}"));
        }
        if rest.starts_with("--") {
            return Some((2, "\u{2013}"));
        }
    }
    if config.ligature_unicode_ellipses && rest.starts_with("...") {
        return Some((3, "\u{2026}"));
    }
    if config.ligature_unicode_quotes {
        if rest.starts_with("``") {
            return Some((2, "\u{201c}"));
        }
        if rest.starts_with("''") {
            return Some((2, "\u{201d}"));
        }
        if rest.starts_with('`') {
            return Some((1, "\u{2018}"));
        }
    }
    if rest.starts_with('\'') {
        if config.auto_unicode_quotes {
            let opening = opening_position(text, at, prev_is_content, true);
            return Some((1, if opening { "\u{2018}" } else { "\u{2019}" }));
        }
        if config.ligature_unicode_quotes {
            return Some((1, "\u{2019}"));
        }
    }
    if config.auto_unicode_quotes && rest.starts_with('"') {
        let opening = opening_position(text, at, prev_is_content, false);
        return Some((1, if opening { "\u{201c}" } else { "\u{201d}" }));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_longest_dash_wins() {
        let config = BlocksConfig::default();
        assert_eq!(
            substitution_at("a---b", 1, &config, false),
            Some((3, "\u{2014}"))
        );
        assert_eq!(
            substitution_at("a--b", 1, &config, false),
            Some((2, "\u{2013}"))
        );
    }

    #[test]
    fn a_closed_gate_leaves_its_family_alone() {
        let config = BlocksConfig {
            ligature_unicode_dashes: false,
            ..BlocksConfig::default()
        };
        assert_eq!(substitution_at("a--b", 1, &config, false), None);
    }

    #[test]
    fn a_double_quote_opens_after_whitespace_and_closes_after_a_letter() {
        let config = BlocksConfig::default();
        assert_eq!(
            substitution_at("a \"b\"", 2, &config, false),
            Some((1, "\u{201c}"))
        );
        assert_eq!(
            substitution_at("a \"b\"", 4, &config, false),
            Some((1, "\u{201d}"))
        );
    }

    #[test]
    fn a_lone_apostrophe_is_decided_by_its_position_when_both_families_are_on() {
        let config = BlocksConfig::default();
        assert_eq!(
            substitution_at("'hi'", 0, &config, false),
            Some((1, "\u{2018}"))
        );
        assert_eq!(
            substitution_at("'hi'", 3, &config, false),
            Some((1, "\u{2019}"))
        );
    }

    #[test]
    fn a_lone_apostrophe_starting_a_run_after_a_content_sibling_closes() {
        let config = BlocksConfig::default();
        assert_eq!(
            substitution_at("'s", 0, &config, true),
            Some((1, "\u{2019}"))
        );
    }

    #[test]
    fn the_ligature_family_keeps_the_pairs_and_the_backtick() {
        let config = BlocksConfig {
            auto_unicode_quotes: false,
            ..BlocksConfig::default()
        };
        assert_eq!(
            substitution_at("``a''", 0, &config, false),
            Some((2, "\u{201c}"))
        );
        assert_eq!(
            substitution_at("``a''", 3, &config, false),
            Some((2, "\u{201d}"))
        );
        assert_eq!(
            substitution_at("`a'", 0, &config, false),
            Some((1, "\u{2018}"))
        );
        assert_eq!(
            substitution_at("`a'", 2, &config, false),
            Some((1, "\u{2019}"))
        );
    }

    #[test]
    fn a_quote_after_a_dash_opens() {
        // Python's `(\s|--|[–—-])'` rule: the dash is opening context for the
        // lone `'` and for the straight `"` alike.
        let config = BlocksConfig::default();
        assert_eq!(substitution_at("a--'q'", 3, &config, false), Some((1, "\u{2018}")));
        assert_eq!(substitution_at("a–'q'", "a–".len(), &config, false), Some((1, "\u{2018}")));
        assert_eq!(substitution_at("a—'q'", "a—".len(), &config, false), Some((1, "\u{2018}")));
        assert_eq!(substitution_at("a--\"q\"", 3, &config, false), Some((1, "\u{201c}")));
    }

    #[test]
    fn a_lone_apostrophe_before_a_digit_opens() {
        let config = BlocksConfig::default();
        assert_eq!(substitution_at("x'9", 1, &config, false), Some((1, "\u{2018}")));
        // The rule is the lone `'`'s alone: a straight `"` before a digit closes.
        assert_eq!(substitution_at("x\"9", 1, &config, false), Some((1, "\u{201d}")));
    }

    #[test]
    fn a_decade_abbreviation_closes_before_its_digits() {
        let config = BlocksConfig::default();
        assert_eq!(substitution_at("x'90s", 1, &config, false), Some((1, "\u{2019}")));
        // Three digits are no decade abbreviation, so the digit rule decides.
        assert_eq!(substitution_at("x'900s", 1, &config, false), Some((1, "\u{2018}")));
        // No word character before the quote: the decade case does not arise.
        assert_eq!(substitution_at("the '90s", 4, &config, false), Some((1, "\u{2018}")));
    }

    #[test]
    fn a_non_ascii_character_before_the_offset_is_no_boundary_problem() {
        let config = BlocksConfig::default();
        let text = "é\"";
        assert_eq!(
            substitution_at(text, "é".len(), &config, false),
            Some((1, "\u{201d}"))
        );
    }
}
