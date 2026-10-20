//! Display width: how many columns a string occupies when printed.
//!
//! The engine never counts columns with `str::len()` or `chars().count()`. The
//! width of a string is the **sum, over its extended grapheme clusters, of the
//! display width of each cluster**, where a grapheme cluster is what a reader
//! sees as one character (`e` followed by a combining accent, or a whole family
//! emoji joined by zero-width joiners). The clusters come from the crate
//! `unicode-segmentation`; each cluster's width comes from the crate
//! `unicode-width`.
//!
//! The width is summed cluster by cluster, rather than computed by
//! `unicode-width` on the whole string, because the whole-string function
//! applies rules that depend on the neighboring characters across cluster
//! boundaries: with it, the width of two strings joined can differ from the
//! sum of their widths, and a longer string can measure narrower than its
//! beginning. The printer keeps a running column, and the fitting scan adds up
//! widths node by node; both need the width of a join to be the sum of the
//! widths, and a longer string never to be narrower. Measuring one cluster at
//! a time and adding the results gives both properties, and agrees with the
//! whole-string width on ordinary text.
//!
//! Control characters, including `'\t'` and `'\n'`, count as one column each
//! under `unicode-width`, which is one reason a text atom may not contain them
//! (see [`DocBuf::text`](super::DocBuf::text)).
//!
//! # The measure the engine uses
//!
//! The engine measures with [`UnicodeMeasure`], which counts the East-Asian
//! *Ambiguous* characters (`…`, `→`, `•`, the em dash) as one column, as
//! terminals configured for Western languages draw them.
//! [`Profile`](super::Profile) has no setting for the measure: a caller needs
//! the measure to align text by hand ([`display_width`]) but not the ability
//! to replace it, and keeping the measure's interface ([`WidthMeasure`])
//! inside the crate leaves the choice of measure free to change. The
//! [`WidthMeasure`] trait and two other measures are kept for the crate's own
//! use and for the tests: for output read where ambiguous characters are drawn
//! two columns wide, the crate could switch the printer to
//! [`UnicodeCjkMeasure`] with a change of one line, and the tests check
//! that the measure changes the layout (the same list document hangs its item
//! at column 4 under one measure and at column 5 under the other).

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

/// Returns the display width of `s` in columns, as the layout engine measures
/// it: the sum, over the extended grapheme clusters of `s`, of each cluster's
/// Unicode display width, with East-Asian *Ambiguous* characters counted as
/// one column.
///
/// A *grapheme cluster* is what a reader sees as one character (`e` followed
/// by a combining accent, or a whole family emoji joined by zero-width
/// joiners), as Unicode's text segmentation rules delimit it. *East-Asian
/// Ambiguous* characters are those a terminal configured for Western languages
/// draws one column wide and one configured for East-Asian languages two (`…`,
/// `→`, `•`, the em dash); the engine counts them as one column.
///
/// The printer wraps against this width, and an emitter uses it for anything
/// it aligns by hand (a rule of `=` under a heading, a label padded to a
/// column). `"Über Bäume — 日本語"` measures 19 (the three East-Asian
/// characters are two columns each); a family emoji measures 2; a combining
/// accent adds nothing.
pub fn display_width(s: &str) -> u32 {
    UnicodeMeasure.str_width(s)
}

/// A width measure: how many columns a string occupies where the output is
/// read.
///
/// An implementation gives the width of one grapheme cluster; the width of a
/// string is the provided sum, which an implementation must not override,
/// because the printer's column arithmetic relies on the width of a join being
/// the sum of the widths and on a longer string never being narrower.
pub(crate) trait WidthMeasure {
    /// The width of one extended grapheme cluster.
    fn cluster_width(&self, cluster: &str) -> u32;

    /// The width of any string: the sum of
    /// [`cluster_width`](Self::cluster_width) over its extended grapheme
    /// clusters.
    fn str_width(&self, s: &str) -> u32 {
        s.graphemes(true).map(|g| self.cluster_width(g)).sum()
    }
}

/// Unicode display width, with East-Asian *Ambiguous* characters counted as
/// **narrow** (one column): the measure the engine uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct UnicodeMeasure;

impl WidthMeasure for UnicodeMeasure {
    #[inline]
    fn cluster_width(&self, cluster: &str) -> u32 {
        UnicodeWidthStr::width(cluster) as u32
    }
}

/// Unicode display width, with East-Asian *Ambiguous* characters counted as
/// **wide** (two columns): the measure for output read where `—`, `…`, `°` and
/// box-drawing characters are drawn two columns wide. Used by the tests; not
/// reachable from the public interface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct UnicodeCjkMeasure;

impl WidthMeasure for UnicodeCjkMeasure {
    #[inline]
    fn cluster_width(&self, cluster: &str) -> u32 {
        UnicodeWidthStr::width_cjk(cluster) as u32
    }
}

/// The length in bytes as the width: not a Unicode measure. It stands for a
/// reader that counts bytes (or an ASCII-only output, where bytes are
/// columns) and gives the tests a third, simple measure. Used by the tests;
/// not reachable from the public interface.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) struct AsciiMeasure;

impl WidthMeasure for AsciiMeasure {
    #[inline]
    fn cluster_width(&self, cluster: &str) -> u32 {
        cluster.len() as u32
    }
}

#[cfg(test)]
mod tests {
    //! Grapheme clusters, East-Asian width, and the additivity the engine
    //! relies on.

    use unicode_segmentation::UnicodeSegmentation;

    use super::{display_width, AsciiMeasure, UnicodeCjkMeasure, UnicodeMeasure, WidthMeasure};

    /// A string with combining-capable Latin, an East-Asian *Ambiguous*
    /// character (the em dash) and three East-Asian *Wide* characters.
    const MIXED: &str = "Über Bäume — 日本語";

    /// A ZWJ emoji sequence: four emoji joined by three zero-width joiners,
    /// which renders as a single glyph.
    const FAMILY: &str = "👨‍👩‍👧‍👦";

    fn measures() -> Vec<Box<dyn WidthMeasure>> {
        vec![
            Box::new(UnicodeMeasure),
            Box::new(UnicodeCjkMeasure),
            Box::new(AsciiMeasure),
        ]
    }

    #[test]
    fn mixed_string_widths_across_the_three_measures() {
        // 11 narrow columns ("Über Bäume " = 11), the em dash, a space, then
        // 3 wide CJK characters.
        assert_eq!(UnicodeMeasure.str_width(MIXED), 19);
        // U+2014 EM DASH is East-Asian Ambiguous: wide in a CJK context.
        assert_eq!(UnicodeCjkMeasure.str_width(MIXED), 20);
        // AsciiMeasure counts bytes: 4 two-byte and 1 three-byte character
        // among 17 characters.
        assert_eq!(MIXED.len(), 26);
        assert_eq!(AsciiMeasure.str_width(MIXED), 26);
    }

    #[test]
    fn ambiguous_width_ellipsis() {
        assert_eq!(UnicodeMeasure.str_width("…"), 1);
        assert_eq!(UnicodeCjkMeasure.str_width("…"), 2);
        assert_eq!(UnicodeMeasure.cluster_width("…"), 1);
        assert_eq!(UnicodeCjkMeasure.cluster_width("…"), 2);
    }

    #[test]
    fn zwj_family_emoji_is_one_cluster_of_width_two() {
        let clusters: Vec<&str> = FAMILY.graphemes(true).collect();
        assert_eq!(clusters.len(), 1, "clusters: {clusters:?}");
        assert_eq!(clusters[0], FAMILY);

        assert_eq!(UnicodeMeasure.cluster_width(FAMILY), 2);
        assert_eq!(UnicodeMeasure.str_width(FAMILY), 2);
        // Chars would say 7, bytes 25 — neither is a column count.
        assert_eq!(FAMILY.chars().count(), 7);
    }

    #[test]
    fn combining_marks_do_not_add_width() {
        // "e" + U+0301 COMBINING ACUTE ACCENT is one cluster, one column.
        let decomposed = "e\u{301}";
        assert_eq!(decomposed.graphemes(true).count(), 1);
        assert_eq!(UnicodeMeasure.str_width(decomposed), 1);
        assert_eq!(UnicodeMeasure.str_width("é"), 1);
    }

    /// The provided `str_width` is exactly the per-cluster accumulator the
    /// printer runs, so measuring a whole atom and accumulating cluster by
    /// cluster agree — the measure is additive.
    #[test]
    fn str_width_is_the_per_cluster_accumulator() {
        for s in [MIXED, FAMILY, "…", "e\u{301}", "plain ascii", ""] {
            for m in measures() {
                let summed: u32 = s.graphemes(true).map(|g| m.cluster_width(g)).sum();
                assert_eq!(m.str_width(s), summed, "{s:?}");
            }
        }
    }

    /// Additivity at an arbitrary cluster boundary: split anywhere between
    /// clusters and the parts still sum to the whole.
    #[test]
    fn width_is_additive_across_cluster_boundaries() {
        for m in measures() {
            let clusters: Vec<&str> = MIXED.graphemes(true).collect();
            for k in 0..=clusters.len() {
                let head: String = clusters[..k].concat();
                let tail: String = clusters[k..].concat();
                assert_eq!(
                    m.str_width(&head) + m.str_width(&tail),
                    m.str_width(MIXED),
                    "split at cluster {k}"
                );
            }
        }
    }

    /// The measure is part of the layout contract, not a cosmetic setting:
    /// U+2022 BULLET is East-Asian *Ambiguous*, so the same list item hangs at
    /// column 4 under [`UnicodeMeasure`] and column 5 under
    /// [`UnicodeCjkMeasure`].
    #[test]
    fn bullet_is_ambiguous_width() {
        assert_eq!(UnicodeMeasure.str_width("  • "), 4);
        assert_eq!(UnicodeCjkMeasure.str_width("  • "), 5);
    }

    /// The public function is the engine's measure.
    #[test]
    fn display_width_is_the_unicode_measure() {
        for s in [MIXED, FAMILY, "…", "e\u{301}", "  • ", ""] {
            assert_eq!(display_width(s), UnicodeMeasure.str_width(s), "{s:?}");
        }
        assert_eq!(display_width(MIXED), 19);
    }
}
