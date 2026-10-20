//! Two jobs of the [`refs`](super) feature: copying the content a reference
//! shows out of the frozen tree, and encoding the anchor of a reference
//! point.
//!
//! **Copying content.** The content a reference to a point shows (a heading's
//! title, for instance) is stored as a [synthesized
//! value](crate::guide::concepts_overview#synthesized-values), a small tree
//! held in a table rather than in the document. [`ContentRef::nodes`] finds
//! the nodes a content reference designates on the node that carries it,
//! [`value_of`] copies a run of nodes into a synthesized value through the
//! language's copy function
//! ([`crate::lang::copy::copy_run_into_value`]), and [`content_value`] does
//! both in one call. The language's [`crate::lang::text`] module reads the
//! characters of such a tree.
//!
//! **Encoding anchors.** An anchor is the identifier a link points at for a
//! reference point, computed once by the anchors stage ([`ANCHORS`](super::ANCHORS)
//! defines the term). [`encode_anchor`] encodes one text for an anchor, and
//! four functions build an anchor from it: [`label_anchor`] from a label,
//! [`title_anchor`] from a title, [`auto_anchor`] from a point's position, and
//! [`value_anchor`] from the number the document gave the point.

use std::sync::Arc;

use techy::core::node::{NodeRef, NodeTree};
use unicode_normalization::UnicodeNormalization;

use crate::model::aspects::{ContentRef, Label};
use crate::lang::copy::{ValueContent, copy_run_into_value};
use crate::lang::guard::GuardPolicy;
use crate::lang::Flm;

pub use crate::lang::copy::CopyError;

/// Copies `nodes` into a synthesized value under the recursion guard policy
/// `guard`, as content of the document shown a second time
/// ([`ValueContent::Shown`]): the content that a reference shows of the point
/// it designates. `anchor` is a node of the tree that holds `nodes` (usually
/// the node that carries the point); `nodes` must belong to `anchor`'s tree,
/// and when `nodes` is empty, the copy's root takes its span and parsing
/// state from `anchor`. It calls [`copy_run_into_value`].
///
/// # Errors
///
/// Returns the [`CopyError`] that the copy raises: `guard` refused to descend
/// further into the run, or the copy could not be built.
pub fn value_of(
    nodes: &[NodeRef<'_, Flm>],
    anchor: &NodeRef<'_, Flm>,
    guard: GuardPolicy,
) -> Result<Arc<NodeTree<Flm>>, CopyError> {
    copy_run_into_value(nodes, anchor, ValueContent::Shown, guard)
}

/// The synthesized value of the content reference `content` on `node`: the
/// nodes [`ContentRef::nodes`] designates, copied through [`value_of`];
/// `Ok(None)` when the reference designates nothing on `node`.
///
/// A [`ContentRef::Synthesized`] tree already has the shape of a synthesized
/// value and is returned unchanged, shared rather than copied, so that the
/// tree an element built once serves every reference to its points.
///
/// # Errors
///
/// Returns the [`CopyError`] that [`value_of`] raises for the designated
/// nodes.
pub fn content_value(
    node: &NodeRef<'_, Flm>,
    content: &ContentRef,
    guard: GuardPolicy,
) -> Result<Option<Arc<NodeTree<Flm>>>, CopyError> {
    if let ContentRef::Synthesized(tree) = content {
        return Ok(Some(Arc::clone(tree)));
    }
    match content.nodes(node) {
        Some(nodes) => value_of(&nodes, node, guard).map(Some),
        None => Ok(None),
    }
}

/// The FNV-1a 64-bit offset basis, the standard constant of the algorithm the
/// anchor tag is taken from.
const FNV_OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;

/// The FNV-1a 64-bit prime, the standard constant of the algorithm.
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

/// FNV-1a over 64 bits: start at the offset basis, and for every byte take the
/// exclusive or with the byte and multiply by the prime, wrapping on overflow.
/// Written out here rather than taken from a crate, because the tag it
/// produces is part of the published form of an anchor and must never change.
fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}

/// The anchor tag of `text`: the first six digits of the sixteen lowercase
/// hexadecimal digits of [`fnv1a_64`] over the text's UTF-8 bytes — that is,
/// the hash's top twenty-four bits.
fn anchor_tag(text: &str) -> String {
    format!("{:06x}", fnv1a_64(text.as_bytes()) >> 40)
}

/// How many letters [`anchor_letters`] writes.
const ANCHOR_LETTERS: u32 = 4;

/// The four letters ending a position-derived anchor: the top thirty-two bits
/// of [`fnv1a_64`] over `text`, reduced modulo twenty-six to the fourth power
/// and written as four base-twenty-six digits, `a` for zero through `z` for
/// twenty-five, the most significant digit first.
///
/// The top bits are the ones taken, as they are for [`anchor_tag`]: the
/// multiplication of the hash carries every bit upwards, so the high bits
/// depend on the whole text.
fn anchor_letters(text: &str) -> String {
    let mut value = (fnv1a_64(text.as_bytes()) >> 32) % 26u64.pow(ANCHOR_LETTERS);
    let mut letters = vec![b'a'; ANCHOR_LETTERS as usize];
    for slot in letters.iter_mut().rev() {
        *slot = b'a' + u8::try_from(value % 26).expect("a remainder below twenty-six");
        value /= 26;
    }
    String::from_utf8(letters).expect("ASCII lowercase letters")
}

/// Whether `ch` is written into an anchor as itself: an ASCII letter, an
/// ASCII digit, `.` or `-`.
fn passes_through(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '.' || ch == '-'
}

/// Whether `text` takes the *exact path* of [`encode_anchor`]: it is not
/// empty, it neither begins nor ends with a hyphen, every character passes
/// through or is a space, and no space is leading, trailing or doubled.
fn is_exact(text: &str) -> bool {
    !text.is_empty()
        && !text.starts_with('-')
        && !text.ends_with('-')
        && !text.starts_with(' ')
        && !text.ends_with(' ')
        && !text.contains("  ")
        && text.chars().all(|ch| passes_through(ch) || ch == ' ')
}

/// Encodes one text for an anchor (the identifier a link points at; see
/// [`ANCHORS`](super::ANCHORS)): a label's type, the label after the colon,
/// or a heading title's plain text.
///
/// The encoding depends on the text alone: no document-order counter takes
/// part, so one text always yields one encoding, in this document and in any
/// other.
///
/// There are two paths.
///
/// - The **exact path**, taken when the text holds only ASCII letters, ASCII
///   digits, `.`, `-` and single spaces (none leading, trailing or doubled),
///   is not empty and neither begins nor ends with a hyphen: the text is
///   reproduced character for character, case preserved, with every space
///   written `_`.
///   `intro` stays `intro`; `quantum error-correcting code` becomes
///   `quantum_error-correcting_code`; `main.v2` stays `main.v2`.
/// - The **tagged path**, taken for every other text: the text is
///   NFKD-decomposed (a compatibility decomposition, which splits an accented
///   letter into its base letter and a combining accent), the combining marks
///   are dropped, and what is left is reduced: an ASCII letter, an ASCII
///   digit, `.` and `-` are written as themselves, and every maximal run of
///   other characters becomes one `_` when the run is whitespace only and one
///   `-` otherwise; `-` and `_` are trimmed from both ends. A `~` and a tag of
///   six hexadecimal digits computed from the text as written (before the
///   decomposition) are appended, so that two texts that reduce alike still
///   differ. `Éléments 2` becomes
///   `Elements_2~<tag>`; `\(\mathcal{C}_\alpha^\infty\)` becomes
///   `mathcal-C-alpha-infty~<tag>`; the empty text becomes `~<tag>`.
///
/// A `~` never appears on the exact path and always on the tagged one, so the
/// two paths cannot produce the same encoding.
///
/// **Why a hyphen at either end leaves the exact path.** An anchor made from
/// a label joins its two encoded parts with one hyphen ([`label_anchor`]),
/// and one made from a title or a position joins its parts with two
/// ([`title_anchor`], [`auto_anchor`]), so the separators tell the two kinds
/// apart, unless a hyphen of the encoded text falls beside the joining one.
/// That happens at both ends: a label whose own text begins with a hyphen
/// would turn `sec:-Foo` into `sec--Foo`, the anchor of an unlabeled heading
/// titled `Foo`, and `eq:-7` into `eq--7…`, the shape of an unlabeled
/// equation line's anchor; a label whose *type* ends with a hyphen would turn
/// `sec-:Foo` into the same `sec--Foo`. On the tagged path the hyphen is
/// trimmed and a tag is appended, so the anchor of a label never takes the
/// shape of the other kinds, regardless of what the label says. No standard
/// construct accepts such a label type, since each accepts a fixed list of
/// types; a reference point that accepts any type can receive one.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::refs::encode_anchor;
///
/// assert_eq!(encode_anchor("intro"), "intro");
/// assert_eq!(encode_anchor("main.v2"), "main.v2");
/// assert_eq!(encode_anchor("quantum error-correcting code"),
///            "quantum_error-correcting_code");
///
/// // Anything else takes the tagged path.
/// let tagged = encode_anchor("Éléments 2");
/// assert!(tagged.starts_with("Elements_2~"));
/// assert_eq!(tagged, encode_anchor("Éléments 2"));
/// ```
pub fn encode_anchor(text: &str) -> String {
    if is_exact(text) {
        return text.replace(' ', "_");
    }
    let folded: String =
        text.nfkd().filter(|ch| !unicode_normalization::char::is_combining_mark(*ch)).collect();
    // A separator is written only before the next character that passes
    // through, so a leading or a trailing run writes nothing: that is the
    // trimming of the ends.
    let mut slug = String::with_capacity(folded.len() + 7);
    let mut run: Option<bool> = None;
    for ch in folded.chars() {
        if passes_through(ch) {
            if let Some(whitespace_only) = run.take() {
                if !slug.is_empty() {
                    slug.push(if whitespace_only { '_' } else { '-' });
                }
            }
            slug.push(ch);
        } else {
            run = Some(run.unwrap_or(true) && ch.is_whitespace());
        }
    }
    let mut anchor = slug.trim_matches(|ch| ch == '-' || ch == '_').to_string();
    anchor.push('~');
    anchor.push_str(&anchor_tag(text));
    anchor
}

/// The anchor formed from a label: `<type>-<label>`, each part through
/// [`encode_anchor`].
///
/// The anchors stage ([`ANCHORS_STAGE`](super::ANCHORS_STAGE)) gives it to a
/// point whose anchor policy is
/// [`AnchorPolicy::FromFirstLabel`](crate::model::aspects::AnchorPolicy), from
/// the first label the point keeps, and to a labeled point of the other
/// policies that fall back to a label (`Auto`, and `FromValue` for a point
/// the document did not number).
///
/// # Examples
///
/// ```
/// use flm_core::model::aspects::Label;
/// use flm_core::stdcore::features::refs::label_anchor;
///
/// assert_eq!(label_anchor(&Label::new("sec", "intro")), "sec-intro");
/// assert!(label_anchor(&Label::new("sec", "Éléments 2"))
///     .starts_with("sec-Elements_2~"));
/// ```
pub fn label_anchor(label: &Label) -> String {
    format!("{}-{}", encode_anchor(&label.ref_type), encode_anchor(&label.ref_label))
}

/// The anchor formed from a title, for an unlabeled heading:
/// `<prefix>--<title>`, each part through [`encode_anchor`].
///
/// The anchors stage ([`ANCHORS_STAGE`](super::ANCHORS_STAGE)) gives it to a
/// point whose anchor policy is
/// [`AnchorPolicy::Slug`](crate::model::aspects::AnchorPolicy); the policy
/// carries the prefix. A heading states `sec`, the label type it accepts, so
/// that a heading's anchor begins the same way whether it was labeled or not.
/// The separator is a double hyphen, so that an anchor made from a title
/// cannot equal one made from a label, whose separator is a single hyphen;
/// [`encode_anchor`] sends a label text that could produce a double hyphen
/// down its tagged path for that reason.
///
/// This function only encodes; the anchors stage makes the result unique. An
/// anchor made from a title that another point already holds takes the next
/// free number (`sec--Intro-2`), so it is never refused.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::refs::title_anchor;
///
/// assert_eq!(title_anchor("sec", "Intro"), "sec--Intro");
/// ```
pub fn title_anchor(prefix: &str, title: &str) -> String {
    format!("{}--{}", encode_anchor(prefix), encode_anchor(title))
}

/// The anchor formed from a point's position, for an unlabeled point such as
/// an equation line: `<prefix>--<position><four letters>`, as in `eq--8otbc`.
///
/// The anchors stage ([`ANCHORS_STAGE`](super::ANCHORS_STAGE)) gives it to a
/// point whose anchor policy is
/// [`AnchorPolicy::Auto`](crate::model::aspects::AnchorPolicy) and that has
/// no label. The policy carries the prefix, as for an anchor made from a
/// title, so that a labeled line (`eq-euler`) and an unlabeled one begin the
/// same way; the separator is a double hyphen, for the reason
/// [`title_anchor`] gives. `position` is the point's place among the points
/// of its kind in the document, counting from one, and makes the anchor
/// unique.
///
/// **The four letters** keep `eq--8` from reading as equation (8), which it
/// is not as soon as one unnumbered equation comes before it: they show that
/// the anchor is not the document's own number. They are computed from the
/// prefix and the position alone: the FNV-1a hash of the text before them,
/// its top thirty-two bits reduced modulo twenty-six to the fourth power and
/// written as four base-twenty-six digits, `a` for zero through `z` for
/// twenty-five, the most significant first. The same position therefore
/// gives the same anchor in every document, on every run and on every
/// platform. They are letters rather than hexadecimal digits because a hash
/// made only of digits would read as one long number again.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::refs::auto_anchor;
///
/// let eighth = auto_anchor("eq", 8);
/// assert!(eighth.starts_with("eq--8"));
/// assert_eq!(eighth.len(), "eq--8".len() + 4);
/// // The same position always gives the same identifier.
/// assert_eq!(eighth, auto_anchor("eq", 8));
/// assert_ne!(eighth, auto_anchor("eq", 9));
/// ```
pub fn auto_anchor(prefix: &str, position: u32) -> String {
    let base = format!("{}--{position}", encode_anchor(prefix));
    let letters = anchor_letters(&base);
    format!("{base}{letters}")
}

/// The anchor formed from the number the document gave a point:
/// `<prefix>-<numbers>`, as in `theorem-2-3`.
///
/// The anchors stage ([`ANCHORS_STAGE`](super::ANCHORS_STAGE)) gives it to a
/// point whose anchor policy is
/// [`AnchorPolicy::FromValue`](crate::model::aspects::AnchorPolicy) and that
/// the document numbered. The policy carries the prefix, as for an anchor
/// made from a title or a position. `numbers` is the text that
/// [`identifier_text`](crate::stdcore::features::numbering::identifier_text)
/// writes for the point's counter value: the values of the counters it is
/// numbered within, outermost first, then its own number, each in decimal and
/// joined by hyphens, regardless of how the counter style shows them. Both
/// parts go through [`encode_anchor`].
///
/// The separator is a single hyphen, the same as in an anchor made from a
/// label, because a number identifies a point as a label does: `theorem-2-3`
/// and the anchor of a label `\label{theorem:2-3}` are equal, and the anchors
/// stage reports that as a collision
/// ([`AnchorCollision`](super::AnchorCollision)).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::refs::value_anchor;
///
/// assert_eq!(value_anchor("theorem", "2-3"), "theorem-2-3");
/// ```
pub fn value_anchor(prefix: &str, numbers: &str) -> String {
    format!("{}-{}", encode_anchor(prefix), encode_anchor(numbers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_exact_path_reproduces_a_plain_label_with_spaces_as_underscores() {
        assert_eq!(encode_anchor("intro"), "intro");
        let phrase = "quantum error-correcting code";
        assert_eq!(encode_anchor(phrase), "quantum_error-correcting_code");
        assert_eq!(encode_anchor("main.v2"), "main.v2");
        assert_eq!(encode_anchor("Mixed-Case.42"), "Mixed-Case.42");
    }

    #[test]
    fn the_exact_path_stops_at_a_leading_a_trailing_or_a_doubled_space() {
        assert!(encode_anchor(" x").contains('~'));
        assert!(encode_anchor("x ").contains('~'));
        assert!(encode_anchor("a  b").contains('~'));
        assert!(!encode_anchor("a b").contains('~'));
    }

    /// A text beginning or ending with a hyphen takes the tagged path, so
    /// that a label's identifier can never take the shape of a title-derived
    /// or a position-derived one, whose separator is a double hyphen.
    #[test]
    fn the_exact_path_stops_at_a_hyphen_at_either_end() {
        assert!(encode_anchor("-7").contains('~'));
        assert!(encode_anchor("-Foo").contains('~'));
        assert!(encode_anchor("a-").contains('~'));
        // The hyphen is trimmed off the slug, and the tag is what keeps the
        // result apart from the same text without it.
        assert_eq!(encode_anchor("-7"), format!("7~{}", anchor_tag("-7")));
        assert_eq!(encode_anchor("a-"), format!("a~{}", anchor_tag("a-")));
        assert_eq!(encode_anchor("7"), "7");
        assert_eq!(encode_anchor("a"), "a");
        // A hyphen anywhere else still passes through.
        assert_eq!(encode_anchor("a-b"), "a-b");
    }

    /// The hole this closes, one test per prefix: `sec:-Foo` used to encode
    /// to `sec--Foo`, the identifier of an unlabeled heading titled `Foo`,
    /// and `eq:-7` to `eq--7`, the shape of an unlabeled equation line's.
    #[test]
    fn a_label_beginning_with_a_hyphen_cannot_take_a_double_hyphen_shape() {
        let section = label_anchor(&Label::new("sec", "-Foo"));
        assert!(!section.starts_with("sec--"), "{section}");
        assert_ne!(section, title_anchor("sec", "Foo"));
        let equation = label_anchor(&Label::new("eq", "-7"));
        assert!(!equation.starts_with("eq--"), "{equation}");
        assert_ne!(equation, auto_anchor("eq", 7));
    }

    /// The same hole at the other end: a label whose *type* ends with a
    /// hyphen, which no standard point accepts and a point accepting any type
    /// can be written with. `sec-:Foo` used to encode to `sec--Foo` as well.
    #[test]
    fn a_label_type_ending_in_a_hyphen_cannot_take_a_double_hyphen_shape() {
        let section = label_anchor(&Label::new("sec-", "Foo"));
        assert!(!section.starts_with("sec--"), "{section}");
        assert_ne!(section, title_anchor("sec", "Foo"));
    }

    #[test]
    fn the_exact_path_stops_at_a_character_it_cannot_write() {
        assert!(encode_anchor("a_b").contains('~'));
        assert!(encode_anchor("a:b").contains('~'));
        assert!(encode_anchor("é").contains('~'));
    }

    #[test]
    fn the_tagged_path_strips_the_accents_of_an_nfkd_decomposition() {
        let text = "Éléments 2";
        assert_eq!(encode_anchor(text), format!("Elements_2~{}", anchor_tag(text)));
    }

    #[test]
    fn the_tagged_path_writes_one_dash_per_run_and_trims_the_ends() {
        let raw = r"\(\mathcal{C}_\alpha^\infty\)";
        assert_eq!(encode_anchor(raw), format!("mathcal-C-alpha-infty~{}", anchor_tag(raw)));
    }

    #[test]
    fn the_tagged_path_allows_an_empty_slug() {
        assert_eq!(encode_anchor("!!"), format!("~{}", anchor_tag("!!")));
        assert_eq!(encode_anchor(""), format!("~{}", anchor_tag("")));
    }

    /// The tag is part of the published form of an anchor: these values are
    /// pinned so that a change of the hash is a test failure, not a silent
    /// change of everybody's anchors.
    #[test]
    fn the_tag_is_the_fixed_fnv_1a_64_hash_of_the_raw_text() {
        assert_eq!(fnv1a_64(b""), FNV_OFFSET_BASIS);
        assert_eq!(fnv1a_64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a_64(b"foobar"), 0x8594_4171_f739_67e8);
        assert_eq!(anchor_tag(""), "cbf29c");
        assert_eq!(anchor_tag("a"), "af63dc");
    }

    #[test]
    fn a_label_anchor_joins_the_type_and_the_label_with_one_hyphen() {
        assert_eq!(label_anchor(&Label::new("sec", "intro")), "sec-intro");
        assert_eq!(
            label_anchor(&Label::new("term", "quantum error-correcting code")),
            "term-quantum_error-correcting_code"
        );
        assert_eq!(label_anchor(&Label::new("eq", "main.v2")), "eq-main.v2");
    }

    /// The letters are part of the published form of an anchor, so these
    /// values are pinned: a change of the reduction is a test failure, not a
    /// silent change of everybody's identifiers.
    #[test]
    fn the_letters_are_the_fixed_base_twenty_six_reduction_of_the_hash() {
        assert_eq!(anchor_letters(""), "qvaa");
        assert_eq!(anchor_letters("eq--8"), "otbc");
        for text in ["", "eq--8", "eq--1", "sec--12"] {
            let letters = anchor_letters(text);
            assert_eq!(letters.len(), 4, "{text}: {letters}");
            assert!(letters.bytes().all(|b| b.is_ascii_lowercase()), "{text}: {letters}");
        }
    }

    #[test]
    fn a_position_anchor_is_the_prefix_the_position_and_four_letters() {
        assert_eq!(auto_anchor("eq", 8), format!("eq--8{}", anchor_letters("eq--8")));
        // Two positions differ in the position itself, whatever the letters.
        assert_ne!(auto_anchor("eq", 8), auto_anchor("eq", 9));
        assert_ne!(auto_anchor("eq", 8), auto_anchor("sec", 8));
        // The prefix is encoded, as it is for a title-derived anchor.
        assert_eq!(auto_anchor("a b", 1), format!("a_b--1{}", anchor_letters("a_b--1")));
    }

    #[test]
    fn a_value_anchor_joins_the_prefix_and_the_numbers_with_one_hyphen() {
        assert_eq!(value_anchor("theorem", "2-3"), "theorem-2-3");
        assert_eq!(value_anchor("theorem", "13"), "theorem-13");
        // The prefix is encoded, as it is everywhere else.
        assert_eq!(value_anchor("a b", "1"), "a_b-1");
        // A number is a name of the same standing as a label, so the two
        // shapes meet; the anchors stage reports that as a collision.
        assert_eq!(value_anchor("theorem", "2-3"), label_anchor(&Label::new("theorem", "2-3")));
    }

    #[test]
    fn a_title_anchor_joins_the_prefix_and_the_title_with_two_hyphens() {
        let title = "The Sum, of Things";
        assert_eq!(
            title_anchor("sec", title),
            format!("sec--The_Sum-of_Things~{}", anchor_tag(title))
        );
        assert_eq!(title_anchor("sec", "Intro"), "sec--Intro");
    }
}
