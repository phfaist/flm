//! The escaping of document text for LaTeX: ten special ASCII characters are
//! written as commands, every non-ASCII character is written according to
//! one of two **Unicode modes** through the Unicode-to-LaTeX table of
//! [`untechxt`], with per-character overrides, and the [`Declared`] record
//! collects what the style file must declare because of the escaped text.
//!
//! Escaping is the only transformation document text undergoes on its way
//! into LaTeX output. It happens where the bytes are written (in the
//! emitter, through the sink's `text` method,
//! [`super::sink::LatexSink::text`]) and never earlier, so the lowered tree
//! holds the characters the document denotes and a text node is escaped
//! exactly once. [`escape_into`] does the escaping; it is public so that a
//! test or a host can see exactly what the sink writes. The escaping is
//! configured by an [`EscapeConfig`], which the `latex-recomposed` backend
//! builds from its render options `unicode` and `unicode_overrides`
//! ([`LatexRecomposedRenderOptions`](crate::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions)).
//!
//! # The special characters
//!
//! Ten ASCII characters mean something to LaTeX and are written as a
//! command: `$ & # _ %` as `\$ \& \# \_ \%`, and `^ { } \ ~` as
//! `\textasciicircum{}`, `\{`, `\}`, `\textbackslash{}`,
//! `\textasciitilde{}` ([`escape_special`]). The caret is written as
//! `\textasciicircum{}` and never as `\^`: `\^` is the circumflex accent
//! command, which takes the character after it as its argument, so `2^10`
//! written as `2\^10` would set an accent over the `1` instead of printing
//! a caret. Any of the ten can reach a text node: a literal-character command
//! such as `\{` or `\%` lowers to the character it represents, and a
//! recovered parse or a user definition's template can put any of them into
//! text. This escaping keeps the recomposed LaTeX compiling regardless of
//! what reaches a text node. Where an author wrote a literal-character
//! command, the recomposition rule of the `literals` feature writes the
//! author's spelling instead, so the escaping applies only where no such rule
//! did. Every other ASCII
//! character, newlines and tabs included, is written as it is.
//!
//! # The Unicode modes
//!
//! Every non-ASCII character has a *spelling*: the LaTeX that prints it
//! (`\'e` for `é`, `\ensuremath{\alpha}` for `α`), from the Unicode-to-LaTeX
//! table of [`untechxt`] ([`spelling_for`]) unless an override replaces it.
//! [`UnicodeMode`] says what is written for such a character:
//!
//! - [`Declare`](UnicodeMode::Declare), the default: the character itself,
//!   as UTF-8, and the style file declares it once with
//!   `\newunicodechar{<char>}{<spelling>}` so that the document compiles on
//!   any engine. The recomposed LaTeX stays what the author typed. The characters
//!   to declare, each with its spelling, are collected in [`Declared`].
//! - [`Replace`](UnicodeMode::Replace): the spelling, in place, protected
//!   exactly as untechxt's standard strategy for text output protects a value
//!   ([`StandardProtection`]): braced when it ends with a command name, so that
//!   it cannot merge with the letters after it.
//!
//! In both modes, what a spelling needs in the preamble is recorded in the
//! [`Declared`] record, so that the style file holds it. The Unicode-to-LaTeX
//! table gives, for each entry, what it needs as a set of *chunks of the
//! table* ([`PreambleNeeds`], made of [`Chunk`](untechxt::preamble::Chunk)
//! values; see [the packages of the Unicode-to-LaTeX
//! table](super::packages#the-packages-of-the-unicode-to-latex-table)). A
//! chunk of the table is either a package with the options it is loaded with
//! (`\ensuremath{\mathbb{C}}` needs `amssymb`, `\ensuremath{\mathds{1}}` needs
//! `dsfont`, a Cyrillic letter needs `fontenc` with the encoding that declares
//! its command) or a block of declarations no package makes
//! (`\ensuremath{\UnxTBbold{0}}` needs the `bbold` family declared as a math
//! alphabet). [`Declared::packages`] returns the first kind as package
//! identifiers and [`Declared::pieces`] the second as identifiers of
//! definition pieces ([`LatexPreambleDef`](super::LatexPreambleDef)).
//! [`packages::is_package_chunk`] says which of the two a chunk is: a package
//! under every engine, or one block of declarations. A
//! package chunk's request can differ by engine — `fontenc` loads `T2A`
//! beside `T1` under pdfLaTeX and beside `TU` under LuaLaTeX and XeLaTeX — and
//! the style file's package block writes such lines inside a test of the
//! engine ([`style_body`](super::style::style_body)); the escaped text itself
//! is the same under every engine.
//!
//! A character no spelling is known for is *unknown*: under `Declare` it is
//! written through as it is, under `Replace` it is replaced by a spelling
//! that shows its code point
//! (`\ensuremath{\langle}\texttt{U+0E18}\ensuremath{\rangle}`,
//! [`unknown_unihex`]). In both modes [`escape_into`] returns it, and the
//! `latex-recomposed` emitter reports each unknown character once per
//! lowered tree as a Warning
//! ([`UnknownCharacter`](crate::stdcore::backends::latex_recomposed::UnknownCharacter)).
//!
//! # Overrides
//!
//! An override replaces the table's spelling for one character, in either
//! mode: the **user's overrides** in [`EscapeConfig::overrides`], a map from a
//! character to the spelling to write for it. Such a spelling is written as
//! the user gave it and nothing is recorded for it here: the user provides
//! what it needs in the preamble (for example through the render option
//! `extra_packages`), not the style file. This library corrects no spelling
//! on its own: a spelling the Unicode-to-LaTeX table gets wrong is corrected in
//! that table.
//!
//! Text is composed to Unicode's canonical composed form ([`nfc`]) before any
//! of this, so that a letter followed by a combining accent is the accented
//! letter the table knows and the declared character is the composed one.
//!
//! Mathematics and verbatim content never pass through here: they are
//! written byte for byte in every mode.
//!
//! # Addresses
//!
//! An address — the target of a link to somewhere outside the document — is
//! not document text, and is written through an escaping of its own,
//! [`escape_url`]: it is handed to `hyperref`'s `\href` inside an ordinary
//! argument of one of the `href` feature's LaTeX hooks (`\flmHref`,
//! `\flmUrl`, `\flmEmail`), so the three characters LaTeX would otherwise read as
//! syntax there are each preceded by a backslash, and every other character,
//! non-ASCII ones included, is written as it is.

use std::collections::{BTreeMap, BTreeSet};

use untechxt::builtin::DEFAULT_TABLE;
use untechxt::lookuptable::LookupTable;
use untechxt::normalizer::nfc;
use untechxt::preamble::{PreambleNeeds, Profile};
use untechxt::protection::{
    MacroNameProtection, ProtectInput, ReplacementProtection, ReplacementProtectionHint,
    StandardProtection,
};
use untechxt::report::NoReport;
use untechxt::unknown_unihex;
use crate::model::names::Name;

use super::{packages, preamble};

/// What is written for a non-ASCII character that has a spelling (the LaTeX
/// that prints it, [`spelling_for`]).
///
/// See [the module documentation](self) for the two modes; the default is
/// `Declare`. The `latex-recomposed` backend takes the mode from its render
/// option `unicode`, written `declare` or `replace` in settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum UnicodeMode {
    /// The character itself, as UTF-8, declared once in the style file.
    #[default]
    Declare,
    /// The spelling, in place.
    Replace,
}

impl UnicodeMode {
    /// The mode's stable name: `declare` or `replace`.
    pub fn as_str(self) -> &'static str {
        match self {
            UnicodeMode::Declare => "declare",
            UnicodeMode::Replace => "replace",
        }
    }

    /// The mode of a stable name, if any.
    pub fn parse(name: &str) -> Option<UnicodeMode> {
        match name {
            "declare" => Some(UnicodeMode::Declare),
            "replace" => Some(UnicodeMode::Replace),
            _ => None,
        }
    }
}

/// How document text is escaped: the Unicode mode and the user's overrides.
/// A [`LatexSink`](super::LatexSink) escapes all the text it writes under one
/// configuration.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EscapeConfig {
    /// What is written for a non-ASCII character.
    pub mode: UnicodeMode,
    /// The user's spelling for a character, replacing the Unicode-to-LaTeX
    /// table's, in either mode. Only a non-ASCII character
    /// is looked up; an entry for an ASCII character is ignored, since the
    /// ten special characters are escaped and the rest of ASCII is written
    /// directly.
    pub overrides: BTreeMap<char, String>,
}

impl EscapeConfig {
    /// The configuration under `mode` with no overrides.
    pub fn new(mode: UnicodeMode) -> EscapeConfig {
        EscapeConfig { mode, overrides: BTreeMap::new() }
    }

    /// The same configuration with one more override: `ch` is spelled
    /// `spelling`.
    pub fn with_override(mut self, ch: char, spelling: impl Into<String>) -> EscapeConfig {
        self.overrides.insert(ch, spelling.into());
        self
    }

    /// The same configuration with the given overrides added.
    pub fn with_overrides(mut self, overrides: BTreeMap<char, String>) -> EscapeConfig {
        self.overrides.extend(overrides);
        self
    }
}

/// The ten special characters and their escaped spellings, in the order
/// `$ & # ^ _ % { } \ ~`.
pub const SPECIALS: [(char, &str); 10] = [
    ('$', "\\$"),
    ('&', "\\&"),
    ('#', "\\#"),
    ('^', "\\textasciicircum{}"),
    ('_', "\\_"),
    ('%', "\\%"),
    ('{', "\\{"),
    ('}', "\\}"),
    ('\\', "\\textbackslash{}"),
    ('~', "\\textasciitilde{}"),
];

/// The escaped spelling of a special character, or `None` for any other
/// character.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::escape::escape_special;
///
/// assert_eq!(escape_special('%'), Some("\\%"));
/// assert_eq!(escape_special('^'), Some("\\textasciicircum{}"));
/// assert_eq!(escape_special('\\'), Some("\\textbackslash{}"));
/// assert_eq!(escape_special('a'), None);
/// ```
pub fn escape_special(ch: char) -> Option<&'static str> {
    SPECIALS.iter().find(|(special, _)| *special == ch).map(|(_, spelling)| *spelling)
}

/// The characters [`escape_url`] precedes with a backslash: `#`, `%` and
/// `\`.
///
/// These are the characters a LaTeX argument cannot hold as themselves — a
/// parameter marker, a comment and an escape — and which `hyperref`'s `\href`
/// reads back as the character they stand for, in its address and in the
/// argument of `\nolinkurl`, when they are written `\#`, `\%` and `\\`. Braces
/// need no escaping: an address the parser read without an error holds its
/// braces balanced, and `\href` keeps a balanced pair as it is, where `\{`
/// would reach the address as a backslash and a brace. (After a missing
/// closing brace, which the parser reports, the address it recovers may hold
/// an unbalanced one, and the LaTeX written from it does not compile.) The other special characters
/// of document text (`$ & ^ _ ~`) are written as they are: `\href` takes them
/// as the characters they are.
pub const URL_SPECIALS: [char; 3] = ['#', '%', '\\'];

/// An address written as the argument of one of the LaTeX family's hyperlink
/// hooks, `\flmHref`, `\flmUrl` and `\flmEmail`: every character of
/// [`URL_SPECIALS`] preceded by a backslash, every other character as it is.
///
/// It is the only escaping of an address in this family. `\href` reads the
/// three escaped characters back as themselves, and so do `\flmUrl` and
/// `\flmEmail` when they derive the displayed text from the address: they drop
/// the escaping backslashes before they remove a leading `http://`,
/// `https://` or `mailto:` and the trailing run of the characters `/`, `#`
/// and `?`. The three hooks
/// are defined by the `href` feature's definition piece
/// ([`href::latex`](crate::stdcore::features::href::latex)).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::escape::escape_url;
///
/// assert_eq!(escape_url("https://x.org/a%20b#c"), "https://x.org/a\\%20b\\#c");
/// assert_eq!(escape_url("x.org/{a}_b~c\\d"), "x.org/{a}_b~c\\\\d");
/// ```
pub fn escape_url(address: &str) -> String {
    let mut out = String::with_capacity(address.len());
    for ch in address.chars() {
        if URL_SPECIALS.contains(&ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// How a spelling the style file declares is written: with nothing around it
/// (`\textemdash`); the argument of `\newunicodechar` is a group of its own.
const DECLARE_NAMES: MacroNameProtection = MacroNameProtection::NoProtection;

/// How a spelling written in place under `Replace` mode is kept from merging
/// with the letters after it: braced when it ends with a command name
/// (`{\textemdash}`), which is untechxt's standard protection for text.
const REPLACE_NAMES: MacroNameProtection = MacroNameProtection::BracesAround;

/// Appends `encoded` to `out` as untechxt's standard strategy for text output
/// ([`StandardProtection::text_mode`]) writes a value under the macro-name
/// protection `names`: wrapped in `\ensuremath{…}` when `hint` says the value
/// is valid in math mode alone, and otherwise as it is, with `names` applied
/// when it ends with a command name.
fn write_spelling(
    out: &mut String,
    encoded: &str,
    hint: ReplacementProtectionHint,
    names: MacroNameProtection,
) {
    let strategy = StandardProtection { protect_names: names, ..StandardProtection::text_mode() };
    // A `String` takes every write, so the strategy cannot fail into one.
    let _ = strategy.write_protected(out, &mut NoReport, ProtectInput::new(encoded, hint));
}

/// The spelling of a non-ASCII character under `config`, with what it needs in
/// the preamble: the user's override, else the Unicode-to-LaTeX table's
/// spelling; `None` when neither knows the character (an *unknown* character).
///
/// The spelling is what the style file declares, and what `Replace` mode
/// protects before writing: the table's LaTeX wrapped for text where it is
/// valid in math mode alone (`\ensuremath{\alpha}`) and as it is otherwise
/// (`\'e`, `\textemdash`), with nothing around it; an override exactly as the
/// user gave it. What it needs is the table entry's [`Profile`] (what a
/// document must have in its preamble to print it), or `None` for a spelling
/// the LaTeX kernel prints by itself and for a user's override, whose needs
/// the user provides. The character is looked up as given; [`escape_into`]
/// composes its text first.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::escape::{EscapeConfig, UnicodeMode, spelling_for};
///
/// let config = EscapeConfig::new(UnicodeMode::Replace).with_override('α', "\\alpha");
/// assert_eq!(spelling_for('é', &config).unwrap().0, "\\'e");
/// assert_eq!(spelling_for('α', &config).unwrap(), (String::from("\\alpha"), None));
/// assert_eq!(spelling_for('𝟙', &config).unwrap().0, "\\ensuremath{\\mathds{1}}");
/// assert_eq!(spelling_for('ธ', &config), None);
/// ```
pub fn spelling_for(ch: char, config: &EscapeConfig) -> Option<(String, Option<&'static Profile>)> {
    if let Some(spelling) = config.overrides.get(&ch) {
        return Some((spelling.clone(), None));
    }
    DEFAULT_TABLE.lookup(ch).map(|entry| {
        let mut spelling = String::new();
        write_spelling(&mut spelling, entry.encoded, entry.hint, DECLARE_NAMES);
        (spelling, entry.needs)
    })
}

/// What the style file must declare because of the text escaped so far: the
/// characters written through under `Declare` mode, each with the spelling
/// to declare it with, and what the spellings written in either mode need in
/// the preamble.
///
/// One record accumulates across every text the emitter writes for one tree;
/// records of several trees are joined with [`merge`](Self::merge). The
/// characters are in character order and the chunks in the order first
/// needed, and what the style file reads of the record
/// ([`packages`](Self::packages), [`pieces`](Self::pieces)) is sorted, so the
/// style file built from it is the same regardless of the order in which the
/// texts were escaped.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Declared {
    chars: BTreeMap<char, String>,
    needs: PreambleNeeds,
}

impl Declared {
    /// An empty record.
    pub fn new() -> Declared {
        Declared::default()
    }

    /// The characters to declare, each with its spelling, in character
    /// order. Empty unless `Declare` mode wrote a non-ASCII character that
    /// has a spelling.
    pub fn chars(&self) -> &BTreeMap<char, String> {
        &self.chars
    }

    /// What the spellings written or declared need in the preamble, as the
    /// Unicode-to-LaTeX table states it ([`PreambleNeeds`]). A user's override
    /// contributes nothing here: the user requests what it needs separately.
    pub fn needs(&self) -> &PreambleNeeds {
        &self.needs
    }

    /// The package identifiers the escaped text needs, in identifier order:
    /// one per chunk of [`needs`](Self::needs) that is a **package**
    /// ([`packages::is_package_chunk`]), under the identifier
    /// [`packages::chunk_package_id`] gives the chunk's identifier, plus
    /// [`packages::NEWUNICODECHAR`] whenever a character is declared, since
    /// declaring one is what `\newunicodechar` is for.
    ///
    /// Every identifier here resolves to the package definitions derived from
    /// the chunk ([`packages::chunk_packages`]), one per set of engines the
    /// chunk differs between, and the style file writes its `\RequirePackage`
    /// lines from those definitions. A chunk that is declarations rather than
    /// a package is a definition piece instead: see [`pieces`](Self::pieces).
    pub fn packages(&self) -> BTreeSet<Name> {
        let mut ids: BTreeSet<Name> = self
            .needs
            .chunks()
            .filter(|chunk| packages::is_package_chunk(chunk))
            .map(|chunk| packages::chunk_package_id(chunk.id()))
            .collect();
        if !self.chars.is_empty() {
            ids.insert(Name::from(packages::NEWUNICODECHAR));
        }
        ids
    }

    /// The definition-piece identifiers the escaped text needs, in identifier
    /// order: one per chunk of [`needs`](Self::needs) that is
    /// **declarations** no package makes ([`packages::is_package_chunk`]),
    /// under the identifier [`preamble::chunk_piece_id`] gives the chunk's
    /// identifier.
    ///
    /// Every identifier here resolves to one of the family's standard
    /// definition pieces ([`preamble::chunk_pieces`]), which the style file
    /// writes after its package requests, so that the packages the
    /// declarations call into are already loaded.
    pub fn pieces(&self) -> BTreeSet<Name> {
        self.needs
            .chunks()
            .filter(|chunk| !packages::is_package_chunk(chunk))
            .map(|chunk| preamble::chunk_piece_id(chunk.id()))
            .collect()
    }

    /// Whether nothing was recorded.
    pub fn is_empty(&self) -> bool {
        self.chars.is_empty() && self.needs.is_empty()
    }

    /// Adds everything `other` recorded. A character both records declare
    /// keeps this record's spelling; since every record of one render
    /// escapes under one configuration, the two spellings are the same.
    pub fn merge(&mut self, other: &Declared) {
        for (ch, spelling) in &other.chars {
            self.chars.entry(*ch).or_insert_with(|| spelling.clone());
        }
        self.needs.merge(&other.needs);
    }

    /// Records `needs`, when the spelling needed anything.
    fn include(&mut self, needs: Option<&Profile>) {
        if let Some(needs) = needs {
            self.needs.include(needs);
        }
    }

    fn declare(&mut self, ch: char, spelling: &str, needs: Option<&Profile>) {
        self.include(needs);
        self.chars.entry(ch).or_insert_with(|| String::from(spelling));
    }
}

/// Appends `text` to `out`, escaped for LaTeX under `config`, recording in
/// `declared` what the style file must declare, and returns the unknown
/// characters found (each once, in order of first occurrence) for the caller
/// to report. See [the module documentation](self) for the rules.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex::escape::{
///     Declared, EscapeConfig, UnicodeMode, escape_into,
/// };
///
/// let mut out = String::new();
/// let mut declared = Declared::new();
/// let unknown = escape_into(
///     &mut out,
///     "50% of café",
///     &EscapeConfig::new(UnicodeMode::Replace),
///     &mut declared,
/// );
/// assert_eq!(out, "50\\% of caf\\'e");
/// assert!(unknown.is_empty());
/// assert!(declared.chars().is_empty());
///
/// let mut out = String::new();
/// escape_into(&mut out, "café", &EscapeConfig::new(UnicodeMode::Declare), &mut declared);
/// assert_eq!(out, "café");
/// assert_eq!(declared.chars().get(&'é').map(String::as_str), Some("\\'e"));
/// ```
pub fn escape_into(
    out: &mut String,
    text: &str,
    config: &EscapeConfig,
    declared: &mut Declared,
) -> Vec<char> {
    let composed = nfc(text);
    let mut unknown: Vec<char> = Vec::new();
    for ch in composed.chars() {
        if let Some(spelling) = escape_special(ch) {
            out.push_str(spelling);
        } else if ch.is_ascii() {
            out.push(ch);
        } else if let Some((spelling, needs)) = spelling_for(ch, config) {
            match config.mode {
                UnicodeMode::Declare => {
                    out.push(ch);
                    declared.declare(ch, &spelling, needs);
                }
                UnicodeMode::Replace => {
                    let hint = ReplacementProtectionHint::any_mode(&spelling);
                    write_spelling(out, &spelling, hint, REPLACE_NAMES);
                    declared.include(needs);
                }
            }
        } else {
            if !unknown.contains(&ch) {
                unknown.push(ch);
            }
            match config.mode {
                UnicodeMode::Declare => out.push(ch),
                UnicodeMode::Replace => out.push_str(&unknown_unihex(ch)),
            }
        }
    }
    unknown
}

/// [`escape_into`] into a fresh string, for the callers that want the
/// escaped text alone; the unknown characters are dropped, so use
/// [`escape_into`] where they must be reported.
pub fn escape(text: &str, config: &EscapeConfig, declared: &mut Declared) -> String {
    let mut out = String::new();
    escape_into(&mut out, text, config, declared);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The study's probe string: an accented letter, a dash, curly quotes,
    /// a vulgar fraction, a relation, a Greek letter.
    const PROBE: &str = "Café — naïve “quotes” ½ ≤ α";

    fn run(text: &str, config: &EscapeConfig) -> (String, Declared, Vec<char>) {
        let mut out = String::new();
        let mut declared = Declared::new();
        let unknown = escape_into(&mut out, text, config, &mut declared);
        (out, declared, unknown)
    }

    #[test]
    fn the_ten_specials_are_escaped_and_other_ascii_passes_through() {
        let (out, declared, unknown) =
            run("$&#^_%{}\\~ a-z 0-9 \"'<>|\n\t", &EscapeConfig::new(UnicodeMode::Replace));
        assert_eq!(
            out,
            "\\$\\&\\#\\textasciicircum{}\\_\\%\\{\\}\\textbackslash{}\\textasciitilde{} \
             a-z 0-9 \"'<>|\n\t"
        );
        assert!(declared.is_empty());
        assert!(unknown.is_empty());
    }

    #[test]
    fn replace_mode_writes_protected_spellings_in_place() {
        let (out, declared, unknown) = run(PROBE, &EscapeConfig::new(UnicodeMode::Replace));
        assert_eq!(
            out,
            "Caf\\'e {\\textemdash} na\\\"ive {\\textquotedblleft}quotes{\\textquotedblright} \
             {\\textonehalf} \\ensuremath{\\leq} \\ensuremath{\\alpha}"
        );
        assert!(unknown.is_empty());
        assert!(declared.chars().is_empty());
        // Every spelling here is one the LaTeX kernel prints by itself.
        assert!(declared.needs().is_empty());
        assert!(declared.packages().is_empty());
    }

    #[test]
    fn declare_mode_writes_the_characters_and_records_their_spellings() {
        let (out, declared, unknown) = run(PROBE, &EscapeConfig::new(UnicodeMode::Declare));
        assert_eq!(out, PROBE);
        assert!(unknown.is_empty());
        let chars: Vec<(char, &str)> =
            declared.chars().iter().map(|(c, s)| (*c, s.as_str())).collect();
        // In character order, which is the record's order.
        assert_eq!(chars, [
            ('½', "\\textonehalf"),
            ('é', "\\'e"),
            ('ï', "\\\"i"),
            ('α', "\\ensuremath{\\alpha}"),
            ('—', "\\textemdash"),
            ('“', "\\textquotedblleft"),
            ('”', "\\textquotedblright"),
            ('≤', "\\ensuremath{\\leq}"),
        ]);
        let ids = declared.packages();
        let packages: Vec<&str> = ids.iter().map(Name::as_str).collect();
        assert_eq!(packages, [packages::NEWUNICODECHAR]);
    }

    #[test]
    fn the_table_s_needs_are_recorded_in_both_modes() {
        let (out, declared, _) = run("𝟙", &EscapeConfig::new(UnicodeMode::Replace));
        assert_eq!(out, "\\ensuremath{\\mathds{1}}");
        assert_eq!(declared.needs().chunks().map(|c| c.id()).collect::<Vec<_>>(), ["dsfont"]);
        assert!(declared.packages().contains(&packages::chunk_package_id("dsfont")));
        let (out, declared, _) = run("𝟙", &EscapeConfig::new(UnicodeMode::Declare));
        assert_eq!(out, "𝟙");
        assert_eq!(declared.chars()[&'𝟙'], "\\ensuremath{\\mathds{1}}");
        assert!(declared.packages().contains(&packages::chunk_package_id("dsfont")));
        // The style file also needs the package the declaration is made with.
        assert!(declared.packages().contains(packages::NEWUNICODECHAR));
    }

    #[test]
    fn a_user_override_wins_over_the_table_and_needs_nothing_of_its_own() {
        let config = EscapeConfig::new(UnicodeMode::Replace)
            .with_override('𝟙', "\\mathbb{1}")
            .with_override('é', "\\'{e}");
        let (out, declared, _) = run("𝟙é", &config);
        assert_eq!(out, "\\mathbb{1}\\'{e}");
        // What an override's spelling needs is the user's own template's to
        // provide: nothing is recorded, and nothing is guessed from its
        // commands.
        assert!(declared.needs().is_empty());
        assert!(declared.packages().is_empty());
    }

    #[test]
    fn an_unknown_character_is_returned_once_and_written_by_mode() {
        let (out, _, unknown) = run("ธ x ธ", &EscapeConfig::new(UnicodeMode::Declare));
        assert_eq!(out, "ธ x ธ");
        assert_eq!(unknown, ['ธ']);
        let (out, declared, unknown) = run("ธ", &EscapeConfig::new(UnicodeMode::Replace));
        assert_eq!(out, "\\ensuremath{\\langle}\\texttt{U+0E18}\\ensuremath{\\rangle}");
        assert_eq!(unknown, ['ธ']);
        assert!(declared.is_empty());
    }

    #[test]
    fn a_cyrillic_spelling_carries_its_font_encoding_and_needs_it() {
        let (out, declared, _) = run("я", &EscapeConfig::new(UnicodeMode::Replace));
        assert_eq!(out, "{\\fontencoding{T2A}\\selectfont\\cyrya}");
        assert_eq!(declared.needs().chunks().map(|c| c.id()).collect::<Vec<_>>(), ["fontenc-t2a"]);
        assert!(declared.packages().contains(&packages::chunk_package_id("fontenc-t2a")));
    }

    #[test]
    fn a_combining_mark_is_composed_when_it_can_be_and_unknown_otherwise() {
        // `e` + combining acute composes to `é`.
        let (out, _, unknown) = run("e\u{0301}", &EscapeConfig::new(UnicodeMode::Replace));
        assert_eq!(out, "\\'e");
        assert!(unknown.is_empty());
        // `q` + combining dot above has no composed form; the table's
        // free-standing accent is not used.
        let (out, _, unknown) = run("q\u{0307}", &EscapeConfig::new(UnicodeMode::Declare));
        assert_eq!(out, "q\u{0307}");
        assert_eq!(unknown, ['\u{0307}']);
    }

    #[test]
    fn merging_two_records_keeps_every_character_and_package() {
        let (_, mut first, _) = run("é", &EscapeConfig::new(UnicodeMode::Declare));
        let (_, second, _) = run("α 𝟙", &EscapeConfig::new(UnicodeMode::Declare));
        first.merge(&second);
        assert_eq!(first.chars().len(), 3);
        assert!(first.packages().contains(&packages::chunk_package_id("dsfont")));
        assert!(first.packages().contains(packages::NEWUNICODECHAR));
    }

    #[test]
    fn the_mode_names_round_trip() {
        for mode in [UnicodeMode::Declare, UnicodeMode::Replace] {
            assert_eq!(UnicodeMode::parse(mode.as_str()), Some(mode));
        }
        assert_eq!(UnicodeMode::default(), UnicodeMode::Declare);
    }
}
