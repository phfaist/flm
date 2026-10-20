//! The escaping of document text for HTML: the three functions that rewrite
//! the characters a browser would otherwise read as markup, one for text
//! between tags and two for attribute values.
//!
//! **Escaping** means writing a character as a *character reference* — the
//! spelling `&name;` or `&#number;` that HTML reads back as that one character
//! — so that the character reaches the reader as itself instead of being taken
//! for syntax. HTML needs it in two places, and they are not the same place:
//! the text between tags, where `<` would start a tag, and the value of an
//! attribute, where the quotation mark closing the value ends it.
//!
//! The three escaping functions here are the whole of the HTML backend's
//! escaping. [`escape_text_into`] is what
//! [`HtmlSink::text`](super::HtmlSink::text) writes through,
//! [`escape_attr_into`] what
//! [`TagBuilder::attr`](super::TagBuilder::attr) writes through, and
//! [`escape_attr_single_quoted_into`] what
//! [`TagBuilder::attr_json`](super::TagBuilder::attr_json) writes through; an
//! emitter calls the sink and never these functions, which are public so that
//! a test or a host can check exactly what the sink does. All three are total:
//! every string has a result, nothing is refused, and none of them ever reads
//! output back ([the sink](super::sink) explains why that matters).
//!
//! # What is escaped, and what is not
//!
//! In **text**, three characters become references — `&`, `<` and `>` — and
//! eight space characters are written as numeric references (below).
//! Nothing else is rewritten: the output is UTF-8 and every other character,
//! accented or not, is written as itself. A quotation mark needs no escaping
//! between tags, and is written as itself.
//!
//! In an **attribute value written inside double quotation marks**, four
//! characters become references — `&`, `<`, `>` and `"` — always, regardless
//! of what the value holds and where it came from. This is how almost every
//! attribute of this backend is written, so escaping the quotation mark keeps a
//! value from ending its attribute early. `&`, `<` and `>` are escaped there as
//! well, so that the value is valid regardless of what it holds, and because
//! this library never reads written output back to find out whether a value
//! was escaped already.
//!
//! In an **attribute value written inside single quotation marks**, two
//! characters become references — `&` and `'` — and nothing else is rewritten:
//! a double quotation mark, a `<` and a `>` are written as themselves, which HTML
//! allows inside a quoted attribute value. This is how an attribute whose
//! value is JSON is written, so that the raw page shows the JSON as
//! JSON — `data-flm-source-span='["book/ch1.flm",0,8]'` — instead of a line of
//! `&quot;`. It escapes as unconditionally as the other two: text a document
//! wrote cannot leave the attribute through it.
//!
//! A further function, `json_string_into`, is not one of the three: it is not
//! HTML escaping and is not public, being used by this backend's emitter
//! alone. It writes a string the way JSON writes one, for the only attribute
//! whose *value* is a JSON value rather than a plain word: the attribute
//! `data-flm-source-span`, which records where an element's node is located
//! in its source. The two escapings then apply one after the other: the value
//! is built as JSON, and the sink escapes that value for HTML with
//! [`escape_attr_single_quoted_into`], so a browser returns the JSON
//! intact.
//!
//! Escaping is the same inside `<pre>` and `<code>` as everywhere else. Those
//! elements change how a browser lays text out, not how it reads markup: a `<`
//! inside them still starts a tag.
//!
//! # The eight spaces
//!
//! Eight Unicode space characters are written as *numeric* character
//! references ([`SPACE_REFERENCES`]) — the spelling `&#x` followed by the
//! character's number in hexadecimal — because a space character that is not
//! the ordinary one cannot be told from an ordinary space by looking at the
//! output: a reviewer comparing two pages, or reading a diff, would see
//! nothing.
//!
//! The spelling is numeric rather than a name such as `&nbsp;` because a
//! numeric reference is read the same way wherever the output is embedded,
//! XML included, while those names are defined by HTML alone and an XML parser
//! refuses every one of them. A numeric reference shows in a comparison of two
//! outputs as clearly as a named one, and that is the purpose of the
//! rewriting.

/// The eight space characters written as numeric character references, each
/// with the reference written for it: the no-break space, the hair space,
/// the thin space, the punctuation space, the en space, the em space, the
/// figure space and the narrow no-break space.
///
/// The table is public so that a test or a host can refer to the same eight
/// characters without repeating them. It is used by [`escape_text_into`]
/// alone: an attribute value is not rewritten this way, since a reader does
/// not compare the spaces inside one.
pub const SPACE_REFERENCES: [(char, &str); 8] = [
    ('\u{00A0}', "&#xA0;"),
    ('\u{200A}', "&#x200A;"),
    ('\u{2009}', "&#x2009;"),
    ('\u{2008}', "&#x2008;"),
    ('\u{2002}', "&#x2002;"),
    ('\u{2003}', "&#x2003;"),
    ('\u{2007}', "&#x2007;"),
    ('\u{202F}', "&#x202F;"),
];

/// Returns the numeric character reference of one of the eight spaces
/// ([`SPACE_REFERENCES`]), or `None` for every other character.
fn space_reference(ch: char) -> Option<&'static str> {
    SPACE_REFERENCES.iter().find(|(space, _)| *space == ch).map(|(_, reference)| *reference)
}

/// Appends `text` to `out` escaped as document text: `&`, `<` and `>` as their
/// character references, the eight spaces of [`SPACE_REFERENCES`] as their
/// numeric references, every other character as itself.
///
/// This is what [`HtmlSink::text`](super::HtmlSink::text) writes through; see
/// [the module documentation](self) for what is escaped where and why.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::escape::escape_text_into;
///
/// let mut out = String::new();
/// escape_text_into(&mut out, "a < b & \"c\"");
/// assert_eq!(out, "a &lt; b &amp; \"c\"");
///
/// // A string that already holds a character reference is escaped again: the
/// // text a document wrote is text, never markup.
/// let mut out = String::new();
/// escape_text_into(&mut out, "&amp;");
/// assert_eq!(out, "&amp;amp;");
///
/// // A no-break space, which nobody could see in the output otherwise.
/// let mut out = String::new();
/// escape_text_into(&mut out, "10\u{00A0}km");
/// assert_eq!(out, "10&#xA0;km");
/// ```
pub fn escape_text_into(out: &mut String, text: &str) {
    for ch in text.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ if ch.is_ascii() => out.push(ch),
            _ => match space_reference(ch) {
                Some(reference) => out.push_str(reference),
                None => out.push(ch),
            },
        }
    }
}

/// Appends `value` to `out` escaped as the value of an attribute written
/// inside double quotation marks: `&`, `<`, `>` and `"` as their character
/// references, every other character as itself — the eight spaces included,
/// since an attribute value is not read by a person the way text is.
///
/// This is what [`TagBuilder::attr`](super::TagBuilder::attr) writes through;
/// see [the module documentation](self) for why the escaping is unconditional.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::escape::escape_attr_into;
///
/// let mut out = String::new();
/// escape_attr_into(&mut out, "a \"quoted\" <value>");
/// assert_eq!(out, "a &quot;quoted&quot; &lt;value&gt;");
///
/// // An address keeps every character it needs; only the four are rewritten.
/// let mut out = String::new();
/// escape_attr_into(&mut out, "https://example.org/p?a=1&b=2");
/// assert_eq!(out, "https://example.org/p?a=1&amp;b=2");
/// ```
pub fn escape_attr_into(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(ch),
        }
    }
}

/// Appends `value` to `out` escaped as the value of an attribute written
/// inside single quotation marks: `&` as `&amp;` and `'` as `&#39;`, every
/// other character as itself — a double quotation mark, a `<` and a `>`
/// included, which HTML allows inside a quoted attribute value.
///
/// This is what [`TagBuilder::attr_json`](super::TagBuilder::attr_json) writes
/// through, and it is meant for an attribute whose value is a JSON value: a
/// JSON value is full of double quotation marks, and writing it between single
/// quotation marks leaves them as themselves, so the raw page shows the value
/// as JSON. See [the module documentation](self) for what is escaped where.
///
/// Escaping the two characters keeps a value from ending its attribute early.
/// The escaping is unconditional: regardless of what the value holds and where
/// it came from, the result cannot leave the attribute.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::html::escape::escape_attr_single_quoted_into;
///
/// let mut out = String::new();
/// escape_attr_single_quoted_into(&mut out, "[\"book/ch1.flm\",0,8]");
/// assert_eq!(out, "[\"book/ch1.flm\",0,8]");
///
/// // The two characters that are rewritten, and only those.
/// let mut out = String::new();
/// escape_attr_single_quoted_into(&mut out, "it's <a & b>");
/// assert_eq!(out, "it&#39;s <a &amp; b>");
/// ```
pub fn escape_attr_single_quoted_into(out: &mut String, value: &str) {
    for ch in value.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
}

/// Appends `value` to `out` as JSON spells a string: the quotation marks
/// around it, a backslash before every quotation mark and every backslash
/// inside it, and every control character (below `U+0020`) as the short escape
/// JSON gives it or as `\u00XX`.
///
/// Every other character is written as itself, which is valid JSON: a JSON
/// document is Unicode text, and this library's output is UTF-8.
///
/// This is not HTML escaping and does not replace it. The result is JSON text;
/// written as an attribute value it goes through
/// [`escape_attr_single_quoted_into`] afterwards, which leaves its quotation
/// marks as themselves and writes the value between single quotation marks.
/// See the module documentation.
///
/// It is not public: the only attribute whose value is JSON is written by this
/// backend's emitter, and nothing outside the backend has a use for it.
pub(super) fn json_string_into(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ if (ch as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", ch as u32)),
            _ => out.push(ch),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(input: &str) -> String {
        let mut out = String::new();
        escape_text_into(&mut out, input);
        out
    }

    fn attr(input: &str) -> String {
        let mut out = String::new();
        escape_attr_into(&mut out, input);
        out
    }

    #[test]
    fn text_escapes_three_characters_and_leaves_the_quotation_marks() {
        assert_eq!(text("a & b < c > d \"e\" 'f'"), "a &amp; b &lt; c &gt; d \"e\" 'f'");
    }

    #[test]
    fn an_attribute_value_escapes_four_characters_always() {
        assert_eq!(attr("a & b < c > d \"e\" 'f'"), "a &amp; b &lt; c &gt; d &quot;e&quot; 'f'");
    }

    #[test]
    fn the_eight_spaces_are_written_as_their_numeric_references_in_text_only() {
        let spaces: String = SPACE_REFERENCES.iter().map(|(ch, _)| *ch).collect();
        let expected: String = SPACE_REFERENCES.iter().map(|(_, r)| *r).collect();
        assert_eq!(text(&spaces), expected);
        assert_eq!(attr(&spaces), spaces);
        // The ordinary space is not one of them.
        assert_eq!(text(" "), " ");
    }

    fn attr_single(input: &str) -> String {
        let mut out = String::new();
        escape_attr_single_quoted_into(&mut out, input);
        out
    }

    #[test]
    fn a_single_quoted_attribute_value_escapes_two_characters_and_no_others() {
        assert_eq!(
            attr_single("a & b < c > d \"e\" 'f'"),
            "a &amp; b < c > d \"e\" &#39;f&#39;"
        );
    }

    #[test]
    fn a_json_array_is_written_as_json_between_single_quotation_marks() {
        let mut value = String::from("[");
        json_string_into(&mut value, "book/ch1.flm");
        value.push_str(",0,8]");
        assert_eq!(attr_single(&value), "[\"book/ch1.flm\",0,8]");
    }

    #[test]
    fn nothing_else_is_rewritten_and_the_output_is_utf8() {
        let probe = "Café — naïve “quotes” ½ ≤ α 😅";
        assert_eq!(text(probe), probe);
        assert_eq!(attr(probe), probe);
        assert_eq!(attr_single(probe), probe);
    }

    #[test]
    fn both_functions_append_to_what_the_buffer_already_holds() {
        let mut out = String::from("<p>");
        escape_text_into(&mut out, "a<b");
        assert_eq!(out, "<p>a&lt;b");
    }

    fn json(input: &str) -> String {
        let mut out = String::new();
        json_string_into(&mut out, input);
        out
    }

    #[test]
    fn a_json_string_carries_its_quotation_marks() {
        assert_eq!(json(""), "\"\"");
        assert_eq!(json("book/ch1.flm"), "\"book/ch1.flm\"");
    }

    #[test]
    fn a_name_holding_a_quotation_mark_a_backslash_and_a_control_character_survives() {
        assert_eq!(json("a\"b\\c\td\u{1}e"), "\"a\\\"b\\\\c\\td\\u0001e\"");
    }

    #[test]
    fn a_quotation_mark_and_a_backslash_are_written_with_a_backslash_before_them() {
        assert_eq!(json("a\"b"), "\"a\\\"b\"");
        assert_eq!(json("a\\b"), "\"a\\\\b\"");
        assert_eq!(json("\\\""), "\"\\\\\\\"\"");
    }

    #[test]
    fn a_control_character_is_written_as_an_escape() {
        assert_eq!(json("\u{8}\u{c}\n\r\t"), "\"\\b\\f\\n\\r\\t\"");
        assert_eq!(json("\u{0}\u{1}\u{1f}"), "\"\\u0000\\u0001\\u001f\"");
    }

    #[test]
    fn every_other_character_stands_as_itself() {
        let probe = "Café — naïve ½ α 😅 '";
        assert_eq!(json(probe), format!("\"{probe}\""));
        // `U+007F` is not a control character JSON asks to be escaped.
        assert_eq!(json("\u{7f}"), "\"\u{7f}\"");
    }

    #[test]
    fn a_json_string_appends_to_what_the_buffer_already_holds() {
        let mut out = String::from("[");
        json_string_into(&mut out, "a");
        assert_eq!(out, "[\"a\"");
    }
}
