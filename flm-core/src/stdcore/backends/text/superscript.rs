//! The Unicode superscript and subscript forms of characters, the only table of
//! these forms in the library: the text backend prints a superscript or a
//! subscript span with them, and the numbering feature's number formats
//! `#{unicodesuperscript}` and `#{unicodesubscript}` write their digits with
//! them.
//!
//! Plain text has no raised or lowered characters of its own, but Unicode
//! encodes a raised and a lowered form of some characters: the ten digits, five
//! signs, and a number of letters, most of them taken from phonetic notation.
//! [`superscript_form`] and [`subscript_form`] return that form for one
//! character, and `None` for a character that has none. The text emitter
//! writes a character that has no form unchanged, beside its converted
//! neighbors: `10a` in a superscript span is `¹⁰ᵃ`, while `q` stays `q`, since
//! this table holds no superscript `q`.
//!
//! A program that chooses the number template of a [counter
//! style](crate::stdcore::features::numbering::CounterStyleDecl) whose numbers
//! the text output shows raised (for instance a footnote's mark numbered with
//! `#{alph}`) can check here which of the characters that template writes
//! have a superscript form.
//!
//! # The superscript forms
//!
//! Each row pairs the characters of its first column, in order, with the forms
//! of its second column.
//!
//! | characters | forms |
//! |---|---|
//! | digits: `0123456789` | `⁰¹²³⁴⁵⁶⁷⁸⁹` |
//! | signs: `+-=()` | `⁺⁻⁼⁽⁾` |
//! | lowercase letters: `abcdefghijklmnoprstuvwxyz` | `ᵃᵇᶜᵈᵉᶠᵍʰⁱʲᵏˡᵐⁿᵒᵖʳˢᵗᵘᵛʷˣʸᶻ` |
//! | capital letters: `ABDEGHIJKLMNOPRTUVW` | `ᴬᴮᴰᴱᴳᴴᴵᴶᴷᴸᴹᴺᴼᴾᴿᵀᵁⱽᵂ` |
//!
//! The lowercase letters lack `q`; the capitals lack `C`, `F`, `Q`, `S`, `X`,
//! `Y` and `Z`.
//!
//! # The subscript forms
//!
//! | characters | forms |
//! |---|---|
//! | digits: `0123456789` | `₀₁₂₃₄₅₆₇₈₉` |
//! | signs: `+-=()` | `₊₋₌₍₎` |
//! | lowercase letters: `aehijklmnoprstuvx` | `ₐₑₕᵢⱼₖₗₘₙₒₚᵣₛₜᵤᵥₓ` |
//!
//! No capital letter has a subscript form.
//!
//! # Forms the table leaves out
//!
//! The forms are those of the Unicode blocks that fonts have covered for a
//! long time (Latin-1, Spacing Modifier Letters, the two Phonetic Extensions
//! blocks, Superscripts and Subscripts, Latin Extended-C). The few later
//! additions (a superscript `q` and the superscript capitals `C`, `F` and `Q`)
//! are left out, because a reader's font is unlikely to show them. The Greek
//! modifier letters are left out too, because they are raised forms of Greek
//! characters, not of the Latin ones a number format writes.

// See decision rationale at [§flm-dd-dr:rend-text-superscript].
const SUPERSCRIPTS: [(char, char); 59] = [
    ('0', '\u{2070}'), ('1', '\u{00b9}'), ('2', '\u{00b2}'), ('3', '\u{00b3}'), ('4', '\u{2074}'),
    ('5', '\u{2075}'), ('6', '\u{2076}'), ('7', '\u{2077}'), ('8', '\u{2078}'), ('9', '\u{2079}'),
    ('+', '\u{207a}'), ('-', '\u{207b}'), ('=', '\u{207c}'), ('(', '\u{207d}'), (')', '\u{207e}'),
    ('a', '\u{1d43}'), ('b', '\u{1d47}'), ('c', '\u{1d9c}'), ('d', '\u{1d48}'), ('e', '\u{1d49}'),
    ('f', '\u{1da0}'), ('g', '\u{1d4d}'), ('h', '\u{02b0}'), ('i', '\u{2071}'), ('j', '\u{02b2}'),
    ('k', '\u{1d4f}'), ('l', '\u{02e1}'), ('m', '\u{1d50}'), ('n', '\u{207f}'), ('o', '\u{1d52}'),
    ('p', '\u{1d56}'), ('r', '\u{02b3}'), ('s', '\u{02e2}'), ('t', '\u{1d57}'), ('u', '\u{1d58}'),
    ('v', '\u{1d5b}'), ('w', '\u{02b7}'), ('x', '\u{02e3}'), ('y', '\u{02b8}'), ('z', '\u{1dbb}'),
    ('A', '\u{1d2c}'), ('B', '\u{1d2e}'), ('D', '\u{1d30}'), ('E', '\u{1d31}'), ('G', '\u{1d33}'),
    ('H', '\u{1d34}'), ('I', '\u{1d35}'), ('J', '\u{1d36}'), ('K', '\u{1d37}'), ('L', '\u{1d38}'),
    ('M', '\u{1d39}'), ('N', '\u{1d3a}'), ('O', '\u{1d3c}'), ('P', '\u{1d3e}'), ('R', '\u{1d3f}'),
    ('T', '\u{1d40}'), ('U', '\u{1d41}'), ('V', '\u{2c7d}'), ('W', '\u{1d42}'),
];

// See decision rationale at [§flm-dd-dr:rend-text-superscript].
const SUBSCRIPTS: [(char, char); 32] = [
    ('0', '\u{2080}'), ('1', '\u{2081}'), ('2', '\u{2082}'), ('3', '\u{2083}'), ('4', '\u{2084}'),
    ('5', '\u{2085}'), ('6', '\u{2086}'), ('7', '\u{2087}'), ('8', '\u{2088}'), ('9', '\u{2089}'),
    ('+', '\u{208a}'), ('-', '\u{208b}'), ('=', '\u{208c}'), ('(', '\u{208d}'), (')', '\u{208e}'),
    ('a', '\u{2090}'), ('e', '\u{2091}'), ('h', '\u{2095}'), ('i', '\u{1d62}'), ('j', '\u{2c7c}'),
    ('k', '\u{2096}'), ('l', '\u{2097}'), ('m', '\u{2098}'), ('n', '\u{2099}'), ('o', '\u{2092}'),
    ('p', '\u{209a}'), ('r', '\u{1d63}'), ('s', '\u{209b}'), ('t', '\u{209c}'), ('u', '\u{1d64}'),
    ('v', '\u{1d65}'), ('x', '\u{2093}'),
];

/// Returns the Unicode superscript form of `c` (`¹` for `1`, `ᵃ` for `a`), or
/// `None` when the [table](self) holds none for it.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::text::superscript::superscript_form;
///
/// assert_eq!(superscript_form('1'), Some('¹'));
/// assert_eq!(superscript_form('a'), Some('ᵃ'));
/// assert_eq!(superscript_form('q'), None);
/// ```
pub fn superscript_form(c: char) -> Option<char> {
    form_in(&SUPERSCRIPTS, c)
}

/// Returns the Unicode subscript form of `c` (`₁` for `1`, `ₐ` for `a`), or
/// `None` when the [table](self) holds none for it.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::text::superscript::subscript_form;
///
/// assert_eq!(subscript_form('1'), Some('₁'));
/// assert_eq!(subscript_form('b'), None);
/// ```
pub fn subscript_form(c: char) -> Option<char> {
    form_in(&SUBSCRIPTS, c)
}

/// The form that `table` pairs with `c`, if any.
fn form_in(table: &[(char, char)], c: char) -> Option<char> {
    table.iter().find(|(from, _)| *from == c).map(|(_, form)| *form)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rows of the table printed under `heading` in this module's
    /// documentation, as the characters of the first column and the forms of
    /// the second, all rows joined.
    fn printed(heading: &str) -> Vec<(char, char)> {
        let source = include_str!("superscript.rs");
        let section = source.split(heading).nth(1).expect("the heading is in the documentation");
        let mut pairs = Vec::new();
        for line in section.lines().skip_while(|line| !line.starts_with("//! |")) {
            let Some(row) = line.strip_prefix("//! |") else { break };
            let cells: Vec<&str> = row.split('`').collect();
            if cells.len() < 4 {
                continue;
            }
            pairs.extend(cells[1].chars().zip(cells[3].chars()));
        }
        pairs
    }

    #[test]
    fn the_printed_tables_are_the_tables() {
        assert_eq!(printed("# The superscript forms"), SUPERSCRIPTS.to_vec());
        assert_eq!(printed("# The subscript forms"), SUBSCRIPTS.to_vec());
    }

    #[test]
    fn every_character_has_at_most_one_form_and_no_form_is_shared() {
        for table in [&SUPERSCRIPTS[..], &SUBSCRIPTS[..]] {
            let mut from: Vec<char> = table.iter().map(|(c, _)| *c).collect();
            let mut to: Vec<char> = table.iter().map(|(_, f)| *f).collect();
            let count = table.len();
            from.sort_unstable();
            from.dedup();
            to.sort_unstable();
            to.dedup();
            assert_eq!((from.len(), to.len()), (count, count));
        }
    }

    #[test]
    fn the_digits_are_the_numbering_formats_digits() {
        let raised: String = ('0'..='9').filter_map(superscript_form).collect();
        assert_eq!(raised, "⁰¹²³⁴⁵⁶⁷⁸⁹");
        let lowered: String = ('0'..='9').filter_map(subscript_form).collect();
        assert_eq!(lowered, "₀₁₂₃₄₅₆₇₈₉");
    }

    #[test]
    fn a_character_with_no_form_answers_none() {
        for c in ['q', 'S', 'Q', ' ', '.', 'é', '\u{b9}'] {
            assert_eq!(superscript_form(c), None, "{c:?}");
        }
        for c in ['b', 'A', 'y', ' '] {
            assert_eq!(subscript_form(c), None, "{c:?}");
        }
    }
}
