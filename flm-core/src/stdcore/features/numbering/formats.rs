//! The eight number formats ([`NumberFormat`]) and the number template
//! ([`NumberTemplate`]), the text with which a counter style writes a number,
//! in either of its two languages; and their compilation against the built
//! language.
//!
//! The superscript and subscript digits are read from the text backend's
//! table of Unicode forms ([`superscript_form`], [`subscript_form`]), so that
//! the library holds a single table of those forms.

use std::fmt;
use std::sync::Arc;

use techy::core::node::NodeTree;

use crate::lang::rewrites::userdefs::template::{CompiledTemplate, TemplateLanguage};
use crate::lang::{BuiltLang, Flm, FlmDiagnostic};
use crate::model::names::Name;
// A feature may read a backend module: the library's layering rules constrain
// only the crate's top-level modules.
use crate::stdcore::backends::text::superscript::{subscript_form, superscript_form};

use super::style::{parse_text, StyleError};
use super::template::{compile_text, number_vocabulary, occurrences_of};

/// A number format: one way of writing a number as characters, as decimal
/// digits, letters, roman numerals, footnote symbols, or superscript or
/// subscript digits.
///
/// A number template refers to a format by the name that
/// [`written_name`](NumberFormat::written_name) returns: `#{arabic}` writes the
/// number in [`Arabic`](NumberFormat::Arabic). The settings use the same
/// names, so these names are part of the settings format and stay fixed.
///
/// Every format except `Arabic` writes the empty text for a number it cannot
/// write: zero, a negative number, and a number so large that its text would
/// be longer than [`MAX_CHARS`](NumberFormat::MAX_CHARS) characters. `Arabic`
/// writes the decimal digits of any number, including `0` and a leading minus
/// sign.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::numbering::NumberFormat;
///
/// assert_eq!(NumberFormat::Arabic.text(12), "12");
/// assert_eq!(NumberFormat::Letters.text(27), "aa");
/// assert_eq!(NumberFormat::CapitalRoman.text(2099), "MMXCIX");
/// assert_eq!(NumberFormat::Roman.text(0), "");
/// assert_eq!(NumberFormat::from_written_name("fnsymbol"), Some(NumberFormat::Symbols));
/// ```
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum NumberFormat {
    /// The decimal digits: `1`, `2`, `12`. Written `arabic`.
    Arabic,
    /// Lowercase letters, repeated instead of counted in base twenty-six:
    /// `a`, …, `z`, `aa`, `bb`, …, `zz`, `aaa`. Written `alph`.
    Letters,
    /// [`Letters`](NumberFormat::Letters) in uppercase: `A`, …, `AA`. Written
    /// `Alph`.
    CapitalLetters,
    /// Lowercase roman numerals: `i`, `iv`, `mmxcix`. Written `roman`.
    Roman,
    /// [`Roman`](NumberFormat::Roman) in uppercase: `I`, `IV`, `MMXCIX`.
    /// Written `Roman`.
    CapitalRoman,
    /// The footnote symbols `*`, `†`, `‡`, `§`, `¶`, `‖`, doubled from the
    /// seventh, tripled from the thirteenth. Written `fnsymbol`.
    Symbols,
    /// The decimal digits as superscript characters: `¹⁷`. Written
    /// `unicodesuperscript`.
    Superscript,
    /// The decimal digits as subscript characters: `₁₇`. Written
    /// `unicodesubscript`.
    Subscript,
}

/// The footnote symbols, in order.
const SYMBOLS: [char; 6] = ['*', '\u{2020}', '\u{2021}', '\u{00a7}', '\u{00b6}', '\u{2016}'];

/// The values of the roman numeral pieces, largest first.
const ROMAN: [(i64, &str); 13] = [
    (1000, "M"),
    (900, "CM"),
    (500, "D"),
    (400, "CD"),
    (100, "C"),
    (90, "XC"),
    (50, "L"),
    (40, "XL"),
    (10, "X"),
    (9, "IX"),
    (5, "V"),
    (4, "IV"),
    (1, "I"),
];

impl NumberFormat {
    /// Every format, in the order of the variants above.
    pub const ALL: [NumberFormat; 8] = [
        NumberFormat::Arabic,
        NumberFormat::Letters,
        NumberFormat::CapitalLetters,
        NumberFormat::Roman,
        NumberFormat::CapitalRoman,
        NumberFormat::Symbols,
        NumberFormat::Superscript,
        NumberFormat::Subscript,
    ];

    /// The length, in characters, of the longest text a format other than
    /// [`Arabic`](NumberFormat::Arabic) writes. A number whose text would be
    /// longer gets the empty text, so that the work of writing a number is
    /// bounded for every number.
    pub const MAX_CHARS: usize = 1024;

    /// Returns the name by which a number template and the settings refer to
    /// this format: `arabic`, `alph`, `Alph`, `roman`, `Roman`, `fnsymbol`,
    /// `unicodesuperscript` or `unicodesubscript`.
    pub fn written_name(self) -> &'static str {
        match self {
            NumberFormat::Arabic => "arabic",
            NumberFormat::Letters => "alph",
            NumberFormat::CapitalLetters => "Alph",
            NumberFormat::Roman => "roman",
            NumberFormat::CapitalRoman => "Roman",
            NumberFormat::Symbols => "fnsymbol",
            NumberFormat::Superscript => "unicodesuperscript",
            NumberFormat::Subscript => "unicodesubscript",
        }
    }

    /// Returns the format whose written name is `name`, or `None` when no
    /// format has that name. The inverse of
    /// [`written_name`](NumberFormat::written_name).
    pub fn from_written_name(name: &str) -> Option<NumberFormat> {
        NumberFormat::ALL.into_iter().find(|format| format.written_name() == name)
    }

    /// Returns `number` written in this format. The type's documentation says
    /// what a format writes for a number it cannot write.
    pub fn text(self, number: i64) -> String {
        match self {
            NumberFormat::Arabic => number.to_string(),
            NumberFormat::Letters => repeated_letter(number, 'a'),
            NumberFormat::CapitalLetters => repeated_letter(number, 'A'),
            NumberFormat::Roman => roman(number).to_lowercase(),
            NumberFormat::CapitalRoman => roman(number),
            NumberFormat::Symbols => repeated_symbol(number),
            NumberFormat::Superscript => in_forms(number, superscript_form),
            NumberFormat::Subscript => in_forms(number, subscript_form),
        }
    }
}

impl fmt::Display for NumberFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.written_name())
    }
}

/// Returns the letter at `number`'s place in the alphabet, repeated once per
/// completed round of twenty-six: 1 is `a`, 26 is `z`, 27 is `aa`, 53 is
/// `aaa`. `first` is the first letter of the alphabet, which chooses the case.
fn repeated_letter(number: i64, first: char) -> String {
    let Some(index) = positive_index(number) else { return String::new() };
    let repeats = 1 + index / 26;
    if repeats > NumberFormat::MAX_CHARS as i64 {
        return String::new();
    }
    let letter = char::from(first as u8 + (index % 26) as u8);
    std::iter::repeat_n(letter, repeats as usize).collect()
}

/// Returns the footnote symbol at `number`'s place, repeated once per
/// completed round of the six symbols.
fn repeated_symbol(number: i64) -> String {
    let Some(index) = positive_index(number) else { return String::new() };
    let repeats = 1 + index / SYMBOLS.len() as i64;
    if repeats > NumberFormat::MAX_CHARS as i64 {
        return String::new();
    }
    let symbol = SYMBOLS[(index % SYMBOLS.len() as i64) as usize];
    std::iter::repeat_n(symbol, repeats as usize).collect()
}

/// Returns `number` written as an uppercase roman numeral: the largest
/// numeral that fits is written as often as it fits, then the next with the
/// remainder.
fn roman(number: i64) -> String {
    if number <= 0 {
        return String::new();
    }
    // Every thousand takes one character, and what is left of a number below a
    // thousand takes at most fifteen.
    if number / 1000 + 15 > NumberFormat::MAX_CHARS as i64 {
        return String::new();
    }
    let mut left = number;
    let mut out = String::new();
    for (value, piece) in ROMAN {
        while left >= value {
            out.push_str(piece);
            left -= value;
        }
    }
    out
}

/// Returns `number`'s decimal digits, each written in the form that `form`
/// gives it (every digit has a superscript and a subscript form). Zero and
/// every negative number get the empty text, as the superscript and subscript
/// formats write only positive numbers.
fn in_forms(number: i64, form: fn(char) -> Option<char>) -> String {
    if number <= 0 {
        return String::new();
    }
    number.to_string().chars().map(|digit| form(digit).unwrap_or(digit)).collect()
}

/// Returns `number`'s place counted from zero, or `None` when the number is
/// zero or negative and the format has nothing to write.
fn positive_index(number: i64) -> Option<i64> {
    (number >= 1).then(|| number - 1)
}

/// A number template: how a counter style writes the number itself, as a text
/// with a place for the number, written in one of the eight [`NumberFormat`]s;
/// a counter style holds one in its field `number_format`.
///
/// The text is FLM with a place for the number, in one of two languages, and
/// it is compiled once, when the counter style is compiled. Around the place
/// for the number the text is ordinary FLM: `\textbf` and `~` mean in a
/// number template what they mean in a document.
///
/// - [`Formats`](NumberTemplate::Formats) is written in the template language
///   that user definitions use ([the template language shared by
///   features](crate::guide::concepts_overview#the-template-language-shared-by-features)):
///   `#{name}` is the number written in the format `name`. The whole text is
///   parsed as one, so markup may surround the number: `\textbf{#{arabic}}`
///   writes the number in bold.
/// - [`Initial`](NumberTemplate::Initial) is the language of an author's own
///   item labels: the first of the characters `a`, `A`, `i`, `I` and `1` is
///   the number, written in the format that character shows by example, and
///   the rest is FLM: `(a)` writes `(c)` for the number three.
///
/// A counter style's number format
/// ([`CounterStyleDecl::number_format`](super::CounterStyleDecl::number_format)),
/// each of its sub-levels ([`SubLevelDecl`](super::SubLevelDecl)) and a
/// reference point's own number template
/// ([`NumberSource::number_format`](crate::model::aspects::NumberSource::number_format),
/// always in the `Formats` language) are number templates.
///
/// # Examples
///
/// The number in decimal digits followed by a full stop, and the number as a
/// lowercase letter between parentheses:
///
/// ```
/// use flm_core::stdcore::features::numbering::NumberTemplate;
///
/// let section = NumberTemplate::formats("#{arabic}.");
/// let item = NumberTemplate::initial("(a)");
/// # let _ = (section, item);
/// ```
///
/// Markup around the number. The language that the counter style is compiled
/// against here holds FLM's structural elements and no feature's commands, so
/// the example surrounds the number with a group instead of a command; a
/// command's braces are a group, and a command behaves the same way.
///
/// ```
/// use flm_core::lang::{build_language, LangInputs};
/// use flm_core::stdcore::features::numbering::{CounterStyleDecl, NumberTemplate};
///
/// let lang = build_language(LangInputs::new());
///
/// let mut around = CounterStyleDecl::new("example.style.equation");
/// around.number_format = NumberTemplate::formats("{#{arabic}}");
/// assert!(around.compile(&lang).is_ok());
///
/// // A name that is not one of the eight formats is refused.
/// let mut wrong = CounterStyleDecl::new("example.style.equation");
/// wrong.number_format = NumberTemplate::formats("#{nosuchformat}");
/// assert!(wrong.compile(&lang).is_err());
/// ```
#[derive(Clone, PartialEq, Eq, Debug, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum NumberTemplate {
    /// A number template in the template language: every `#{name}` is the
    /// number written in the format `name`, and the rest is written as given:
    /// `#{arabic}.` writes `3.` for the number three, and
    /// `\textbf{#{arabic}}` writes it in bold. A `#{…}` whose name is not a
    /// number format is refused when the counter style is compiled, and so is
    /// a key in brackets, `#[…]{…}`, which only a within template accepts.
    Formats(String),
    /// A number template in the single-letter language: the first character
    /// among `a`, `A`, `i`, `I` and `1` is the number, written in the format
    /// that character shows by example (letters, capital letters, roman
    /// numerals, capital roman numerals, decimal digits), and the rest is
    /// FLM written as given: `(a)` writes `(c)` for the number three. A text
    /// that holds none of those five characters writes itself for every
    /// number, and the parts of the counters it is numbered within are not
    /// written before it: it is a fixed mark, such as a bullet.
    ///
    /// This is the language of an author's own item labels, where the
    /// number's format is shown by an example instead of being named. The
    /// first of the five characters is replaced wherever it is, so `Part a.`
    /// writes `Pcrt a.` for the number three; a text that must hold one of
    /// these characters as a letter of its own is written in the `Formats`
    /// language.
    Initial(String),
}

impl NumberTemplate {
    /// Returns a [`Formats`](NumberTemplate::Formats) number template with the
    /// text `text`.
    pub fn formats(text: impl Into<String>) -> NumberTemplate {
        NumberTemplate::Formats(text.into())
    }

    /// Returns an [`Initial`](NumberTemplate::Initial) number template with
    /// the text `text`.
    pub fn initial(text: impl Into<String>) -> NumberTemplate {
        NumberTemplate::Initial(text.into())
    }

    /// Returns the template's text, in the language it is written in.
    pub fn text(&self) -> &str {
        match self {
            NumberTemplate::Formats(text) | NumberTemplate::Initial(text) => text,
        }
    }
}

/// One part of a compiled single-letter number template: FLM parsed once, or
/// the number in a format.
pub(super) enum NumberPart {
    /// A literal part of the template, parsed when the counter style was
    /// compiled.
    Literal(Arc<NodeTree<Flm>>),
    /// The number, written in this format.
    Number(NumberFormat),
}

/// A number template compiled against the built language.
///
/// A compiled number template belongs to a compiled counter style
/// ([`CounterStyle`](super::CounterStyle)), which writes numbers with it.
pub(super) enum CompiledNumber {
    /// An [`Initial`](NumberTemplate::Initial) template: its literal parts
    /// parsed, and the single letter replaced by the format it shows.
    Initial {
        /// The parts, in the order they are written.
        parts: Vec<NumberPart>,
        /// Whether the ancestors' numbers are written before this template's
        /// own text. Only a template that holds none of the five characters
        /// leaves them out.
        with_parent: bool,
    },
    /// A [`Formats`](NumberTemplate::Formats) template, compiled by the
    /// template compiler of the language layer: every `#{…}` is the number in
    /// the format between its braces, and everything around it is content
    /// that may surround the number.
    Written {
        /// The compiled template.
        template: CompiledTemplate,
        /// The formats the template refers to, each once, in the order of
        /// their first use: the formats in which the number is written before
        /// the template is instantiated.
        formats: Vec<NumberFormat>,
    },
}

impl CompiledNumber {
    /// Returns whether the ancestors' numbers are written before this
    /// template's own text.
    pub(super) fn with_parent(&self) -> bool {
        match self {
            CompiledNumber::Initial { with_parent, .. } => *with_parent,
            CompiledNumber::Written { .. } => true,
        }
    }

    /// Returns `template` compiled against `lang`; `style` is the name of the
    /// counter style the template belongs to, which a diagnostic gives.
    ///
    /// # Errors
    ///
    /// [`StyleError::Template`] for a `Formats` template that does not compile
    /// (one that is not valid FLM, or whose `#{…}` is not one of the eight
    /// number formats), and the parse error of a literal part of an
    /// `Initial` template.
    pub(super) fn compile(
        template: &NumberTemplate,
        style: &Name,
        lang: &BuiltLang,
        language: &TemplateLanguage,
        warnings: &mut Vec<FlmDiagnostic>,
    ) -> Result<CompiledNumber, StyleError> {
        match template {
            NumberTemplate::Formats(text) => compile_formats(text, style, lang, language),
            NumberTemplate::Initial(text) => compile_initial(text, lang, warnings),
        }
    }
}

/// The five characters by which an [`Initial`](NumberTemplate::Initial)
/// template shows a format, and the format each one shows.
const INITIALS: [(char, NumberFormat); 5] = [
    ('a', NumberFormat::Letters),
    ('A', NumberFormat::CapitalLetters),
    ('i', NumberFormat::Roman),
    ('I', NumberFormat::CapitalRoman),
    ('1', NumberFormat::Arabic),
];

/// Compiles a [`Formats`](NumberTemplate::Formats) template with the template
/// compiler of the language layer, and reads from it the formats it refers to.
fn compile_formats(
    text: &str,
    style: &Name,
    lang: &BuiltLang,
    language: &TemplateLanguage,
) -> Result<CompiledNumber, StyleError> {
    let what = format!("the counter style `{style}`");
    let template = compile_text(text, &what, &number_vocabulary(), lang, language)?;
    let mut formats = Vec::new();
    for (name, _) in occurrences_of(&template) {
        // The template compiled against the vocabulary of the eight formats,
        // so every name it writes is one of them.
        if let Some(format) = NumberFormat::from_written_name(&name) {
            if !formats.contains(&format) {
                formats.push(format);
            }
        }
    }
    Ok(CompiledNumber::Written { template, formats })
}

/// Compiles an [`Initial`](NumberTemplate::Initial) template: the text is cut
/// at the first of the five characters, which becomes the format, and the two
/// texts around it are parsed as FLM.
fn compile_initial(
    text: &str,
    lang: &BuiltLang,
    warnings: &mut Vec<FlmDiagnostic>,
) -> Result<CompiledNumber, StyleError> {
    let found = text.char_indices().find_map(|(at, ch)| {
        INITIALS.iter().find(|(initial, _)| *initial == ch).map(|(_, format)| (at, *format))
    });
    let Some((at, format)) = found else {
        let mut parts = Vec::new();
        let mut literal = text.to_string();
        push_literal(&mut parts, &mut literal, text, lang, warnings)?;
        return Ok(CompiledNumber::Initial { parts, with_parent: false });
    };
    let mut parts = Vec::new();
    let mut before = text[..at].to_string();
    push_literal(&mut parts, &mut before, text, lang, warnings)?;
    parts.push(NumberPart::Number(format));
    let mut after = text[at + 1..].to_string();
    push_literal(&mut parts, &mut after, text, lang, warnings)?;
    Ok(CompiledNumber::Initial { parts, with_parent: true })
}

/// Parses `literal` as FLM and appends it as a part, unless it is empty; then
/// empties it for the next text.
///
/// `declared` is the whole template the text comes from: a refused text is
/// reported with the whole template, because nobody wrote the text on either
/// side of the letter on its own.
fn push_literal(
    parts: &mut Vec<NumberPart>,
    literal: &mut String,
    declared: &str,
    lang: &BuiltLang,
    warnings: &mut Vec<FlmDiagnostic>,
) -> Result<(), StyleError> {
    if literal.is_empty() {
        return Ok(());
    }
    let parsed =
        parse_text(literal, lang, warnings).map_err(|error| error.in_text(declared))?;
    parts.push(NumberPart::Literal(parsed));
    literal.clear();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_format_is_written_by_its_own_name() {
        for format in NumberFormat::ALL {
            assert_eq!(NumberFormat::from_written_name(format.written_name()), Some(format));
        }
    }

    #[test]
    fn a_number_too_large_to_write_gets_the_empty_text() {
        for format in NumberFormat::ALL {
            let text = format.text(i64::MAX);
            assert!(
                text.chars().count() <= 20,
                "{format} wrote {} characters",
                text.chars().count()
            );
        }
        assert_eq!(NumberFormat::Arabic.text(i64::MAX), i64::MAX.to_string());
    }
}
