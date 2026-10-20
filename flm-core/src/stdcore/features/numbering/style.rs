//! Counter styles: the declaration ([`CounterStyleDecl`] and its parts), the
//! compiled form ([`CounterStyle`]), and the writing of one value with it
//! ([`format()`]), with the errors of compiling ([`StyleError`]) and of
//! writing ([`FormatError`]).
//!
//! The module also holds what the writing shares with the rest of the
//! numbering module: the counters a value is numbered within ([`Ancestor`],
//! [`Parent`]), the options of a request ([`FormatOptions`]), the segments of
//! the result ([`Segment`]), the assembly of content on a worker thread, and the
//! join texts ([`JoinDecl`]), which the refs feature also uses for a
//! reference to several targets.

use std::collections::BTreeMap;
use std::convert::Infallible;
use std::fmt;
use std::sync::Arc;

use techy::core::node::{BuildId, NodeKind, NodeRef, NodeTree};
use techy::core::{Lang, ParsingState};
use techy::error::Severity;
use techy::source::{SourceSpan, TextContent};
use techy::transform::{Restage, RestageContext, RestageError, RestageVisitor};

use crate::model::addr::NodeAddr;
use crate::model::doc::FragmentLayout;
use crate::model::names::Name;
use crate::lang::guard::{self, GuardPolicy};
use crate::lang::{
    parse_fragment, BuiltLang, Flm, FlmDiagnostic, FlmOrigin, FlmSource, FlmSpan, ParseOptions,
};

use crate::lang::rewrites::userdefs::template::TemplateLanguage;

use super::formats::{CompiledNumber, NumberFormat, NumberPart, NumberTemplate};
use super::joiner::{CompiledWithinJoiner, JoinCx, Part};
use super::template::{instantiate_template, Binding, Bindings};
use super::value::CounterValue;
use super::writer::{
    default_writer, CompiledNumberWriter, JoinText, NumberOut, NumberWriter, NumberedValue,
    WriteCx, WriterCx,
};

// --- errors ---------------------------------------------------------------------------------

/// Why a counter style, a within-joiner or a number writer could not be
/// compiled.
///
/// The numbering feature compiles once, when the registry is built, so each of
/// these errors reveals a mistake in a declaration or in configuration. The
/// numbering stage also compiles the within-joiners of a document's numbering
/// options and a point's own number template; there, an error is reported as
/// a diagnostic of the document.
#[derive(Clone, Debug)]
pub enum StyleError {
    /// A template did not compile: it is not valid FLM, it holds a `#{…}`
    /// whose name the template does not accept (a name that is not a number
    /// format, a counter no feature declares), or it puts keys in brackets on
    /// a name that accepts none.
    Template {
        /// The template, as it was declared.
        text: String,
        /// What the compilation reported.
        diagnostics: Vec<FlmDiagnostic>,
    },
    /// A `#[…]{counter}` of a within template carries a key that is neither
    /// `bare` nor the name of a number format.
    UnknownKey {
        /// The template that holds the key.
        text: String,
        /// The key, as it was written.
        key: String,
    },
    /// A counter is numbered within another through a within-joiner name that
    /// no feature registers.
    UnknownJoiner {
        /// The name the counter asked for.
        name: Name,
    },
    /// A counter style asks for a number writer that no feature registers.
    UnknownWriter {
        /// The name the counter style asked for.
        name: Name,
    },
    /// A `${…}` of a join text refers to a name that is not a join key.
    UnknownVariable {
        /// The join text that holds the substitution.
        text: String,
        /// The name written between the braces.
        name: String,
    },
    /// A text of the counter style is not valid FLM: the parse reported
    /// errors.
    Parse {
        /// The text, as it was declared. A single-letter number template is
        /// cut at its letter and parsed in pieces, and the error gives the
        /// whole template, as its author wrote it.
        text: String,
        /// The parse's errors.
        diagnostics: Vec<FlmDiagnostic>,
    },
    /// A text of the counter style could not be parsed at all: it nests too
    /// deeply, or no worker thread was available.
    ParseFailed {
        /// The text, as it was declared, given whole as in
        /// [`Parse`](StyleError::Parse).
        text: String,
        /// What the parse reported.
        detail: String,
    },
}

impl StyleError {
    /// Returns the same error, with `text` (a text as its author declared it)
    /// in place of the piece of it that was being parsed. A single-letter
    /// number template is cut at its letter, and nobody wrote the piece on
    /// either side of the letter on its own.
    pub(super) fn in_text(self, text: &str) -> StyleError {
        let text = text.to_string();
        match self {
            StyleError::Template { diagnostics, .. } => StyleError::Template { text, diagnostics },
            StyleError::UnknownKey { key, .. } => StyleError::UnknownKey { text, key },
            StyleError::UnknownJoiner { name } => StyleError::UnknownJoiner { name },
            StyleError::UnknownWriter { name } => StyleError::UnknownWriter { name },
            StyleError::UnknownVariable { name, .. } => StyleError::UnknownVariable { text, name },
            StyleError::Parse { diagnostics, .. } => StyleError::Parse { text, diagnostics },
            StyleError::ParseFailed { detail, .. } => StyleError::ParseFailed { text, detail },
        }
    }
}

impl fmt::Display for StyleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StyleError::Template { text, diagnostics } => {
                write!(f, "the template `{text}` is not valid")?;
                for diagnostic in diagnostics {
                    write!(f, ": {}", diagnostic.message())?;
                }
                Ok(())
            }
            StyleError::UnknownKey { text, key } => {
                write!(
                    f,
                    "the template `{text}` writes the key `{key}` in brackets, and a key there \
                     must be `bare` or the name of a number format"
                )
            }
            StyleError::UnknownJoiner { name } => {
                write!(f, "no feature registers a within-joiner named `{name}`")
            }
            StyleError::UnknownWriter { name } => {
                write!(f, "no feature registers a number writer named `{name}`")
            }
            StyleError::UnknownVariable { text, name } => {
                write!(
                    f,
                    "the join text `{text}` writes `${{{name}}}`, and `{name}` is not one of \
                     the join keys"
                )
            }
            StyleError::Parse { text, diagnostics } => {
                write!(f, "the text `{text}` is not valid FLM")?;
                for diagnostic in diagnostics {
                    write!(f, ": {}", diagnostic.message())?;
                }
                Ok(())
            }
            StyleError::ParseFailed { text, detail } => {
                write!(f, "the text `{text}` could not be parsed: {detail}")
            }
        }
    }
}

impl std::error::Error for StyleError {}

/// Why a number could not be written.
///
/// The first two variants say that a value and the counter style or the
/// ancestors it is written with do not fit each other; the numbering stage
/// refuses such a record when it makes it, so for a record of the counter
/// table they reveal a fault in the program. The last two are failures of the
/// writing itself, not of any input.
#[derive(Clone, Debug)]
pub enum FormatError {
    /// A value has a sub-number at a level for which its counter style
    /// declares no format.
    UndeclaredSubLevel {
        /// The counter style asked to write the value.
        style: Name,
        /// The sub-level, counted from zero for the first sub-number.
        level: usize,
        /// The sub-number at that level.
        number: i64,
    },
    /// A within-joiner refers to a counter that is not among the ancestors it
    /// was given. The within-joiner may be the point's own or, since an
    /// ancestor is written with its own ancestors before it, an ancestor's.
    UnknownAncestor {
        /// The within-joiner that refers to the counter, as it was declared: a
        /// joining text, a within template, or the description a registered
        /// within-joiner gives of itself.
        joiner: String,
        /// The counter referred to.
        counter: Name,
    },
    /// The content nests too deeply to be copied into the result.
    NestingTooDeep {
        /// What the recursion guard reported.
        detail: String,
    },
    /// Building the result failed, or no worker thread was available. This
    /// reveals a fault in the program or a refusal of the operating system,
    /// not a problem of any input.
    Failed {
        /// What failed.
        detail: String,
    },
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FormatError::UndeclaredSubLevel { style, level, number } => write!(
                f,
                "the counter style `{style}` must declare a format for sub-level {level} \
                 (counting the first sub-level as 0), since the value has the sub-number \
                 {number} there"
            ),
            FormatError::UnknownAncestor { joiner, counter } => write!(
                f,
                "the within-joiner `{joiner}` may write only counters this number is numbered \
                 within, but it writes the counter `{counter}`"
            ),
            FormatError::NestingTooDeep { detail } => {
                write!(f, "the content nests too deeply to write a number with: {detail}")
            }
            FormatError::Failed { detail } => write!(f, "writing the number failed: {detail}"),
        }
    }
}

impl std::error::Error for FormatError {}

// --- what a feature declares ----------------------------------------------------------------

/// The delimiters of a counter style: the text before and the text after a
/// number, such as `(` and `)` for an equation number shown as `(3)`.
///
/// Both are FLM, and both are left out when a number is asked for without
/// delimiters ([`FormatOptions::without_delimiters`]).
#[derive(Clone, PartialEq, Eq, Debug, Default, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct Delimiters {
    /// The text written before the number.
    pub open: String,
    /// The text written after the number.
    pub close: String,
}

impl Delimiters {
    /// Returns the delimiters `open` and `close`.
    pub fn new(open: impl Into<String>, close: impl Into<String>) -> Delimiters {
        Delimiters { open: open.into(), close: close.into() }
    }
}

/// The word written before a number, in two forms: one for a single number
/// and one for several.
///
/// The fields are named after the plural categories `one` and `other` of
/// natural-language pluralization: `Eq.~` for one equation, `Eqs.~` for
/// several. Both are FLM, so `~` and a text style mean in them what they mean
/// in a document.
#[derive(Clone, PartialEq, Eq, Debug, Default, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PluralWords {
    /// The word written before a single number.
    pub one: String,
    /// The word written before several numbers.
    pub other: String,
}

impl PluralWords {
    /// Returns the words `one`, for a single number, and `other`, for
    /// several.
    pub fn new(one: impl Into<String>, other: impl Into<String>) -> PluralWords {
        PluralWords { one: one.into(), other: other.into() }
    }

    /// Returns the same word for one number and for several.
    pub fn same(word: impl Into<String>) -> PluralWords {
        let word = word.into();
        PluralWords { one: word.clone(), other: word }
    }
}

/// The prefix words of a counter style: the words written before a number by
/// default, and the named variants that a request may ask for instead.
///
/// A variant is a complete pair of words under a name of its own, such as
/// `capital`, holding `Equation~` and `Equations~` where the default words are
/// `Eq.~` and `Eqs.~`. A request for a variant the counter style does not
/// declare ([`FormatOptions::variant`]) gets the default words; the refs
/// feature reports such a request at the reference as an Error.
#[derive(Clone, PartialEq, Eq, Debug, Default, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PrefixWords {
    /// The words used when no variant is asked for, and when the variant asked
    /// for is not declared.
    pub base: PluralWords,
    /// The variants, by name.
    pub variants: BTreeMap<String, PluralWords>,
}

impl PrefixWords {
    /// Returns the default words `one` and `other`, with no variant.
    pub fn new(one: impl Into<String>, other: impl Into<String>) -> PrefixWords {
        PrefixWords { base: PluralWords::new(one, other), variants: BTreeMap::new() }
    }

    /// Returns these prefix words with the variant `variant` added under
    /// `name`.
    pub fn with_variant(mut self, name: impl Into<String>, variant: PluralWords) -> PrefixWords {
        self.variants.insert(name.into(), variant);
        self
    }
}

/// A sub-level of a counter style: how the sub-numbers at one level are
/// written, and the separator written before each.
///
/// A counter style with one sub-level, declared as letters after a full stop,
/// writes the value `(3, 2)` as `3.b`. The separator is FLM; it is left out
/// when nothing has been written before the sub-number, so that the end of a
/// range is written `b` and not `.b`.
#[derive(Clone, PartialEq, Eq, Debug, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubLevelDecl {
    /// The text written before this level's number, when something is
    /// written before it.
    pub separator: String,
    /// How this level's number is written.
    pub number_format: NumberTemplate,
}

impl SubLevelDecl {
    /// Returns a sub-level written with `number_format` after `separator`.
    pub fn new(separator: impl Into<String>, number_format: NumberTemplate) -> SubLevelDecl {
        SubLevelDecl { separator: separator.into(), number_format }
    }
}

/// The join texts of a counter style: the texts that join several numbers
/// into one text, such as `1–3, 7, and 9`.
///
/// Every field is a text written between or around the numbers, and each may
/// be left unset, in which case the default text is used (the defaults are
/// given per field below). The texts are FLM. A `${name}` in one of them is
/// replaced by the text of the key `name`, in one pass (a replaced text is not
/// scanned again); the keys meant for this use are the four variables `and`,
/// `sep`, `endash` and `empty`, so that a caller changes the word `and` once
/// instead of in every join text that uses it. A `${name}` whose name is not a key fails the
/// compilation of the counter style ([`StyleError::UnknownVariable`]).
///
/// Which fields are used depends on how many *runs* the numbers form, a run
/// being one number or a sequence of consecutive numbers:
///
/// - one run: `one_pre`, the run, `one_post`;
/// - two runs: `pair_pre`, the first, `pair_mid`, the second, `pair_post`;
/// - more: `list_pre`, the runs separated by `list_mid`, `list_midlast`
///   before the last one, `list_post`.
///
/// A run of several numbers is written as `range_pre`, the first number, then
/// `range_pairmid` for a run of exactly two or `range_mid` for a longer one,
/// then the last number, then `range_post`.
///
/// The refs feature uses the same declaration for the texts that join the
/// targets of a reference to several targets.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::numbering::JoinDecl;
///
/// // The default: `1–3, 7, and 9`.
/// let spelled_out = JoinDecl::default();
/// // The compact preset: `1–3,7,9`.
/// let compact = JoinDecl::compact();
/// // The default with another conjunction.
/// let mut german = JoinDecl::default();
/// german.and = Some("und".to_string());
/// # let _ = (spelled_out, compact, german);
/// ```
#[derive(Clone, PartialEq, Eq, Debug, Default, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct JoinDecl {
    /// Before a single run. Default: empty.
    pub one_pre: Option<String>,
    /// After a single run. Default: empty.
    pub one_post: Option<String>,
    /// Before two runs. Default: empty.
    pub pair_pre: Option<String>,
    /// Between two runs. Default: ` ${and} `; compact: `,`.
    pub pair_mid: Option<String>,
    /// After two runs. Default: empty.
    pub pair_post: Option<String>,
    /// Before a run of several numbers. Default: empty.
    pub range_pre: Option<String>,
    /// Between the ends of a run of three or more. Default: `${endash}`.
    pub range_mid: Option<String>,
    /// Between the ends of a run of exactly two. Default: `${sep}`; compact:
    /// `,`.
    pub range_pairmid: Option<String>,
    /// After a run of several numbers. Default: empty.
    pub range_post: Option<String>,
    /// Before three runs or more. Default: empty.
    pub list_pre: Option<String>,
    /// Between the runs of a list, except before the last. Default: `${sep} `;
    /// compact: `,`.
    pub list_mid: Option<String>,
    /// Before the last run of a list. Default: `${sep} ${and} `; compact: `,`.
    pub list_midlast: Option<String>,
    /// After three runs or more. Default: empty.
    pub list_post: Option<String>,
    /// The variable `and`. Default: `and`.
    pub and: Option<String>,
    /// The variable `sep`. Default: `,`.
    pub sep: Option<String>,
    /// The variable `endash`. Default: `–`.
    pub endash: Option<String>,
    /// The text written when there is no number at all, and the variable
    /// `empty`. Default: `(empty)`.
    pub empty: Option<String>,
}

impl JoinDecl {
    /// Returns every text the declaration carries, in the order of the fields
    /// above, each `None` where the default text is used.
    ///
    /// A caller that must write out a whole declaration reads it through this
    /// method, for example for a registry digest, which must change when any
    /// one of these texts changes.
    pub fn texts(&self) -> [&Option<String>; 17] {
        [
            &self.one_pre,
            &self.one_post,
            &self.pair_pre,
            &self.pair_mid,
            &self.pair_post,
            &self.range_pre,
            &self.range_mid,
            &self.range_pairmid,
            &self.range_post,
            &self.list_pre,
            &self.list_mid,
            &self.list_midlast,
            &self.list_post,
            &self.and,
            &self.sep,
            &self.endash,
            &self.empty,
        ]
    }

    /// Writes every text of the declaration into `out`, for a registry
    /// digest: per key whether it is set, then the text preceded by its
    /// length, so that two different declarations never write the same bytes.
    pub(crate) fn digest_into(&self, out: &mut Vec<u8>) {
        for text in self.texts() {
            out.push(u8::from(text.is_some()));
            super::decl::digest_text(out, text.as_deref().unwrap_or(""));
        }
    }

    /// Returns the compact join texts: the numbers separated by commas, with
    /// no word between the last two and no space after a comma, `1–3,7,9`.
    pub fn compact() -> JoinDecl {
        JoinDecl {
            pair_mid: Some(",".to_string()),
            range_pairmid: Some(",".to_string()),
            list_mid: Some(",".to_string()),
            list_midlast: Some(",".to_string()),
            ..JoinDecl::default()
        }
    }
}

/// A counter style, as a feature declares it.
///
/// A counter style says how a counter value is shown: the number template, the
/// sub-levels, the prefix words with their variants, the delimiters and the
/// join texts, each text written in FLM and parsed once, when the counter
/// style is compiled against the built language
/// ([`compile`](CounterStyleDecl::compile)) into a [`CounterStyle`]. It also
/// holds three choices about links and ranges, and may ask for a number writer
/// other than the default one. The concepts overview presents counters and
/// counter styles together ([counters and counter
/// styles](crate::guide::concepts_overview#counters-and-counter-styles)).
///
/// A feature contributes the declaration to the service slot
/// [`STYLES_SLOT`](super::STYLES_SLOT); a counter refers to its default counter
/// style by name ([`CounterDecl::default_style`](super::CounterDecl::default_style)),
/// and a reference point gives the counter style of its number in its number
/// source. A host replaces a declared counter style by name through
/// [`NumberingConfig`](super::NumberingConfig).
///
/// A counter style's name is a dotted name owned by the feature that declares
/// it, such as `flm.math.style.equation`, so that two features may each
/// declare a counter style for the same counter without a clash.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::numbering::{
///     CounterStyleDecl, Delimiters, JoinDecl, NumberTemplate, PrefixWords,
/// };
///
/// let mut style = CounterStyleDecl::new("flm.math.style.equation");
/// style.number_format = NumberTemplate::formats("#{arabic}");
/// style.delimiters = Delimiters::new("(", ")");
/// style.prefix_words = PrefixWords::new("Eq.~", "Eqs.~")
///     .with_variant("capital", flm_core::stdcore::features::numbering::PluralWords::new(
///         "Equation~", "Equations~",
///     ));
/// style.join = JoinDecl::compact();
/// ```
///
/// In data, no field is required: each field the data gives replaces that
/// field of a base counter style, and every other field keeps the base's
/// value. The base of an entry of
/// [`NumberingConfig::styles`](super::NumberingConfig::styles) is the counter
/// style that [`CounterStyleDecl::new`] returns for the entry's key, which is
/// its name, not the declared counter style it replaces; the counter style in
/// the citations feature's configuration takes that feature's default counter
/// style as its base. The name is never written as a field.
#[derive(Clone, PartialEq, Eq, Debug, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(transform = style_schema)]
pub struct CounterStyleDecl {
    /// The counter style's name: dotted, owned by the feature that declares
    /// it.
    #[schemars(skip)]
    pub name: Name,
    /// The number template: how the number itself is written.
    pub number_format: NumberTemplate,
    /// The sub-levels, outermost first. A value cannot be written with a
    /// sub-number at a level the counter style does not declare.
    pub sub_levels: Vec<SubLevelDecl>,
    /// The prefix words: the word written before the number.
    pub prefix_words: PrefixWords,
    /// The delimiters: the texts around the number.
    pub delimiters: Delimiters,
    /// The join texts: the texts that join several numbers.
    pub join: JoinDecl,
    /// Whether the word before the numbers is part of the link to the first
    /// number written. With one number written by [`format()`], the whole
    /// text is one link regardless of this setting. On by default.
    pub name_in_link: bool,
    /// Whether the end of a range repeats the part of the number that comes
    /// from the counters it is numbered within: `A.I–A.III` instead of
    /// `A.I–III`. Off by default.
    pub repeat_parents_in_range: bool,
    /// Whether consecutive numbers collapse into a range: `1–3` when on,
    /// `1,2,3` when off. Several numbers are sorted in both cases, and when
    /// off every number is written separately, each with a link of its own.
    /// On by default.
    pub ranges: bool,
    /// The name of the [number writer](super::NumberWriter) that writes this
    /// counter style's numbers, registered under that name in
    /// [`NUMBER_WRITERS_SLOT`](super::NUMBER_WRITERS_SLOT), or `None` for the
    /// default writer. A name no feature registers fails the registry build.
    pub number_writer: Option<Name>,
}

/// Adjusts the schema of a counter style in data: no field is required, as
/// the counter style is read over a base counter style.
fn style_schema(schema: &mut schemars::Schema) {
    crate::core::settings::named::require_only(schema, &[]);
}

impl CounterStyleDecl {
    /// Returns a counter style named `name` that writes the number in decimal
    /// digits (`#{arabic}`), with no sub-level, no word, no delimiters, the
    /// default join texts, the word inside the link, no repeated ancestors'
    /// part in a range, consecutive numbers collapsed into ranges, and the
    /// default number writer.
    pub fn new(name: impl Into<Name>) -> CounterStyleDecl {
        CounterStyleDecl {
            name: name.into(),
            number_format: NumberTemplate::formats("#{arabic}"),
            sub_levels: Vec::new(),
            prefix_words: PrefixWords::default(),
            delimiters: Delimiters::default(),
            join: JoinDecl::default(),
            name_in_link: true,
            repeat_parents_in_range: false,
            ranges: true,
            number_writer: None,
        }
    }

    /// Builder: this counter style's numbers are written by the number writer
    /// registered under `name`.
    ///
    /// A counter style that asks for no writer is written by the default
    /// writer, which writes what the counter style's own data describes. A
    /// name no feature registers fails the registry build.
    pub fn with_number_writer(mut self, name: impl Into<Name>) -> CounterStyleDecl {
        self.number_writer = Some(name.into());
        self
    }

    /// Returns this declaration compiled against the built language: every
    /// text parsed once, every number template compiled, every join variable
    /// replaced.
    ///
    /// A caller that writes numbers of its own, outside the numbering feature,
    /// compiles a counter style with this method; the numbering feature
    /// compiles every declared counter style itself when the registry is
    /// built. This method knows no registered number writer, so it refuses a
    /// declaration that asks for one ([`compile_with`](CounterStyleDecl::compile_with)
    /// accepts them).
    ///
    /// # Errors
    ///
    /// The [`StyleError`] of the first text that is not valid FLM or that
    /// refers to something that does not exist, and
    /// [`StyleError::UnknownWriter`] for a declaration that asks for a number
    /// writer.
    pub fn compile(&self, lang: &BuiltLang) -> Result<CounterStyle, StyleError> {
        let language = TemplateLanguage::new(lang).map_err(|errors| StyleError::ParseFailed {
            text: self.number_format.text().to_string(),
            detail: errors.iter().map(ToString::to_string).collect::<Vec<_>>().join("; "),
        })?;
        self.compile_with(lang, &language, &[])
    }

    /// Returns this declaration compiled as [`compile`](CounterStyleDecl::compile)
    /// does, with an already built template language and with the number
    /// writers that features registered.
    ///
    /// A caller that compiles many counter styles builds the template language
    /// once and passes it to each call. A declaration that asks for a writer
    /// that none of `writers` has is refused with
    /// [`StyleError::UnknownWriter`].
    ///
    /// # Errors
    ///
    /// The same as [`compile`](CounterStyleDecl::compile), and
    /// [`StyleError::UnknownWriter`].
    pub fn compile_with(
        &self,
        lang: &BuiltLang,
        language: &TemplateLanguage,
        writers: &[Arc<dyn NumberWriter>],
    ) -> Result<CounterStyle, StyleError> {
        let mut warnings = Vec::new();
        let blank = parse_text("", lang, &mut warnings)?;
        let number =
            CompiledNumber::compile(&self.number_format, &self.name, lang, language, &mut warnings)?;
        let mut sub_levels = Vec::with_capacity(self.sub_levels.len());
        for level in &self.sub_levels {
            sub_levels.push(CompiledSubLevel {
                separator: parse_text(&level.separator, lang, &mut warnings)?,
                number: CompiledNumber::compile(
                    &level.number_format,
                    &self.name,
                    lang,
                    language,
                    &mut warnings,
                )?,
            });
        }
        let writer = match &self.number_writer {
            None => Arc::clone(default_writer()),
            Some(name) => {
                let Some(declared) = writers.iter().find(|writer| writer.name() == name) else {
                    return Err(StyleError::UnknownWriter { name: name.clone() });
                };
                declared.compile(&WriterCx::new(lang, &self.name))?
            }
        };
        let words = CompiledWords {
            base: compile_words(&self.prefix_words.base, lang, &mut warnings)?,
            variants: self
                .prefix_words
                .variants
                .iter()
                .map(|(name, words)| {
                    compile_words(words, lang, &mut warnings).map(|words| (name.clone(), words))
                })
                .collect::<Result<BTreeMap<_, _>, _>>()?,
        };
        Ok(CounterStyle {
            name: self.name.clone(),
            number,
            sub_levels: Arc::new(sub_levels),
            words: Arc::new(words),
            open: parse_text(&self.delimiters.open, lang, &mut warnings)?,
            close: parse_text(&self.delimiters.close, lang, &mut warnings)?,
            join: Arc::new(CompiledJoin::compile(
                &self.join,
                STYLE_TEXT_ORIGIN,
                lang,
                &mut warnings,
            )?),
            name_in_link: self.name_in_link,
            repeat_parents_in_range: self.repeat_parents_in_range,
            ranges: self.ranges,
            writer,
            state: Arc::clone(lang.initial_state()),
            blank,
            warnings,
        })
    }
}

// --- what a declaration compiles to ---------------------------------------------------------

/// One sub-level, compiled.
struct CompiledSubLevel {
    separator: Arc<NodeTree<Flm>>,
    number: CompiledNumber,
}

/// One pair of words, compiled.
struct CompiledPlural {
    one: Arc<NodeTree<Flm>>,
    other: Arc<NodeTree<Flm>>,
}

/// The words of a counter style, compiled.
struct CompiledWords {
    base: CompiledPlural,
    variants: BTreeMap<String, CompiledPlural>,
}

/// The join texts of a counter style, compiled: the variables replaced and
/// every text parsed.
///
/// The refs feature compiles the texts that join the targets of a reference
/// to several targets into this same form, through the same
/// [`compile`](CompiledJoin::compile), so that one [`JoinDecl`] means the same
/// in both places.
pub(crate) struct CompiledJoin {
    pub(super) one_pre: Arc<NodeTree<Flm>>,
    pub(super) one_post: Arc<NodeTree<Flm>>,
    pub(super) pair_pre: Arc<NodeTree<Flm>>,
    pub(super) pair_mid: Arc<NodeTree<Flm>>,
    pub(super) pair_post: Arc<NodeTree<Flm>>,
    pub(super) range_pre: Arc<NodeTree<Flm>>,
    pub(super) range_mid: Arc<NodeTree<Flm>>,
    pub(super) range_pairmid: Arc<NodeTree<Flm>>,
    pub(super) range_post: Arc<NodeTree<Flm>>,
    pub(super) list_pre: Arc<NodeTree<Flm>>,
    pub(super) list_mid: Arc<NodeTree<Flm>>,
    pub(super) list_midlast: Arc<NodeTree<Flm>>,
    pub(super) list_post: Arc<NodeTree<Flm>>,
    pub(super) empty: Arc<NodeTree<Flm>>,
}

/// A counter style compiled against the built language: what a number is
/// written with.
///
/// It is built from a [`CounterStyleDecl`] by
/// [`compile`](CounterStyleDecl::compile) or
/// [`compile_with`](CounterStyleDecl::compile_with), holds the parsed form of
/// every text of the declaration and the compiled number writer, and is
/// passed to [`format()`] and [`format_many`](super::format_many). The counter
/// table holds the compiled counter styles of a registry
/// ([`Counters::style`](super::Counters::style)).
pub struct CounterStyle {
    name: Name,
    number: CompiledNumber,
    // Every compiled part but the number is shared, so that a style derived
    // with another number format costs one compiled template.
    sub_levels: Arc<Vec<CompiledSubLevel>>,
    words: Arc<CompiledWords>,
    open: Arc<NodeTree<Flm>>,
    close: Arc<NodeTree<Flm>>,
    join: Arc<CompiledJoin>,
    name_in_link: bool,
    repeat_parents_in_range: bool,
    ranges: bool,
    /// The writer of this counter style's numbers: the default writer, or the
    /// one the declaration asks for.
    writer: Arc<dyn CompiledNumberWriter>,
    /// The language's initial parsing state, with which the text nodes of a
    /// number's characters are built.
    state: Arc<ParsingState<Flm>>,
    /// The parse of the empty text: the tree a result is assembled against,
    /// and the source the characters of a number are attributed to.
    blank: Arc<NodeTree<Flm>>,
    warnings: Vec<FlmDiagnostic>,
}

impl fmt::Debug for CounterStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CounterStyle").field("name", &self.name.as_str()).finish_non_exhaustive()
    }
}

impl CounterStyle {
    /// Returns the counter style's name, as the declaration gave it.
    pub fn name(&self) -> &Name {
        &self.name
    }

    /// Returns the diagnostics that the counter style's texts raised when
    /// they were parsed. None of them is an error, since a text that parses
    /// with an error is refused instead. The numbering feature adds them to
    /// the registry's build diagnostics.
    pub fn warnings(&self) -> &[FlmDiagnostic] {
        &self.warnings
    }

    /// Returns a derived counter style: this counter style with its number
    /// format replaced by `template`, with the same name, sub-levels, words,
    /// delimiters, join texts, choices and number writer.
    ///
    /// Only `template` is compiled; every other part is shared with this
    /// counter style. The numbering stage uses it for a point that carries a
    /// number template of its own
    /// ([`NumberSource::number_format`](crate::model::aspects::NumberSource::number_format)):
    /// the point's record holds the derived counter style, so that the point's
    /// number and every reference to it are written in it. The lists feature
    /// also writes an item's label through a derived counter style
    /// ([`Counters::format_in_style`](super::Counters::format_in_style)). The
    /// [`warnings`](CounterStyle::warnings) of the derived counter style are
    /// this counter style's, followed by those `template` raised.
    ///
    /// # Errors
    ///
    /// The [`StyleError`] of a template that does not compile, as
    /// [`CounterStyleDecl::compile_with`] reports it for a declaration's
    /// number template.
    pub fn with_number_format(
        &self,
        template: &NumberTemplate,
        lang: &BuiltLang,
        language: &TemplateLanguage,
    ) -> Result<CounterStyle, StyleError> {
        let mut warnings = self.warnings.clone();
        let number = CompiledNumber::compile(template, &self.name, lang, language, &mut warnings)?;
        Ok(CounterStyle {
            name: self.name.clone(),
            number,
            sub_levels: Arc::clone(&self.sub_levels),
            words: Arc::clone(&self.words),
            open: Arc::clone(&self.open),
            close: Arc::clone(&self.close),
            join: Arc::clone(&self.join),
            name_in_link: self.name_in_link,
            repeat_parents_in_range: self.repeat_parents_in_range,
            ranges: self.ranges,
            writer: Arc::clone(&self.writer),
            state: Arc::clone(&self.state),
            blank: Arc::clone(&self.blank),
            warnings,
        })
    }

    /// Returns whether the counter style declares the variant `name` of the
    /// prefix words ([`PrefixWords::variants`]).
    ///
    /// A number asked for in a variant the counter style does not declare is
    /// written with the default words; a caller that reports such a request,
    /// as the refs feature does, checks it with this method.
    pub fn declares_variant(&self, name: &str) -> bool {
        self.words.variants.contains_key(name)
    }

    /// Returns how many sub-levels the counter style declares: the depth of
    /// the deepest sub-number it can write, counting the first sub-level as 1.
    ///
    /// A value that carries a non-zero sub-number deeper than that cannot be
    /// written in this counter style; the numbering stage checks this when it
    /// makes a record, not when the number is shown.
    pub fn sub_level_count(&self) -> usize {
        self.sub_levels.len()
    }

    /// Returns `text` as a small tree of content, built as the characters of
    /// a number are: one text node attributed to the counter style's source.
    ///
    /// A caller that writes several numbers itself builds the text between two
    /// of them with this method: everything a reader sees is content, not
    /// characters, and this module provides the only way to build content from
    /// computed characters. The call runs on a worker thread.
    ///
    /// # Errors
    ///
    /// [`FormatError::NestingTooDeep`] or [`FormatError::Failed`] when the
    /// content could not be built.
    pub fn literal(&self, text: &str) -> Result<Arc<NodeTree<Flm>>, FormatError> {
        let pieces = [Piece::Digits(text.to_string())];
        on_worker(|| assemble_pieces(self, &pieces))
    }

    /// Returns the empty span to which a number's computed content is
    /// attributed: the start of the counter style's own source, where a
    /// diagnostic about a number points.
    pub(super) fn site(&self) -> FlmSpan {
        let at: &FlmSpan = self.blank.root().span();
        SourceSpan::new(at.source(), at.start()..at.start())
    }

    /// Returns the word to write before `count` numbers, `variant` being the
    /// variant asked for. A variant the counter style does not declare gets
    /// the default words; the refs feature reports such a request at the
    /// reference as an Error.
    fn word(&self, count: usize, variant: Option<&str>) -> &Arc<NodeTree<Flm>> {
        let words = variant
            .and_then(|variant| self.words.variants.get(variant))
            .unwrap_or(&self.words.base);
        if count == 1 {
            &words.one
        } else {
            &words.other
        }
    }
}

// --- what a caller passes in ------------------------------------------------------------------

/// An ancestor: one counter within which a number is numbered, as the
/// numbering feature records it: the counter's name, its value at the
/// numbered point, the counter style in which that value is written, and the
/// within-joiner that writes that counter's own ancestors before it.
///
/// Because each ancestor carries its own within-joiner, a chain of counters is
/// written one step at a time: a within-joiner that refers to
/// `flm.headings.subsection` writes the
/// subsection's number preceded by what the subsection's own within-joiner
/// writes, `2.3.`, so a within-joiner refers only to the counter immediately
/// above.
///
/// An ancestor counted in a scoped instance also records which instance, for
/// example the enclosing list of an item of a nested list
/// ([`instance`](Ancestor::instance)).
#[derive(Clone, Debug)]
pub struct Ancestor {
    /// The counter's name.
    pub counter: Name,
    /// Its value at the numbered point.
    pub value: CounterValue,
    /// The counter style in which its value is written.
    pub style: Arc<CounterStyle>,
    /// The within-joiner that writes this counter's own ancestors before its
    /// number, or `None` for a counter numbered within nothing. Those
    /// ancestors are the ones before it in the list a [`Parent`] carries.
    pub joiner: Option<Arc<dyn CompiledWithinJoiner>>,
    /// The scoped instance this value was counted in, as the address of the
    /// node the instance is attached to, or `None` for a value of the
    /// counter's single sequence through the document. See
    /// [`NumberSource::instances`](crate::model::aspects::NumberSource::instances).
    pub instance: Option<NodeAddr>,
}

impl Ancestor {
    /// Returns an ancestor numbered within nothing and counted in no scoped
    /// instance. Its own within-joiner, if it has one, is added with
    /// [`within`](Ancestor::within).
    pub fn new(counter: impl Into<Name>, value: CounterValue, style: Arc<CounterStyle>) -> Ancestor {
        Ancestor { counter: counter.into(), value, style, joiner: None, instance: None }
    }

    /// Returns this ancestor as a value counted in the scoped instance
    /// attached to the node `instance`.
    pub fn in_instance(mut self, instance: NodeAddr) -> Ancestor {
        self.instance = Some(instance);
        self
    }

    /// Returns this ancestor with `joiner` as the within-joiner that writes
    /// its own ancestors' numbers before its own.
    pub fn within(mut self, joiner: Arc<dyn CompiledWithinJoiner>) -> Ancestor {
        self.joiner = Some(joiner);
        self
    }
}

/// What a number is numbered within: the within-joiner that writes the
/// ancestors' numbers, and the ancestors with their values.
///
/// The ancestors are the **whole chain**, outermost first:
/// `flm.headings.section` before `flm.headings.subsection` before
/// `flm.headings.subsubsection`. A caller must keep this order, because it
/// says which counters each ancestor may itself be written within: the
/// ancestors of `ancestors[i]` are `ancestors[..i]`. A within-joiner that
/// refers to a counter which is not before it in the chain fails with
/// [`FormatError::UnknownAncestor`]. Every step of the writing therefore has
/// strictly fewer counters before it than the step before, so a chain of n
/// ancestors takes at most n steps, regardless of what a caller passes.
#[derive(Clone, Copy, Debug)]
pub struct Parent<'a> {
    /// The within-joiner that writes the ancestors' numbers before the
    /// point's own. It usually refers only to the counter immediately above,
    /// which is written with its own ancestors.
    pub joiner: &'a dyn CompiledWithinJoiner,
    /// The ancestors, as the numbering feature recorded them: the whole chain,
    /// outermost first.
    pub ancestors: &'a [Ancestor],
}

impl<'a> Parent<'a> {
    /// Returns the within-joiner `joiner` with the chain `ancestors`,
    /// outermost first.
    pub fn new(
        joiner: &'a dyn CompiledWithinJoiner,
        ancestors: &'a [Ancestor],
    ) -> Parent<'a> {
        Parent { joiner, ancestors }
    }
}

/// How a number is asked for: which variant of the prefix words, and whether
/// the word and the delimiters are written.
///
/// The default asks for the number as the counter style shows it: the
/// default words, with the word and the delimiters.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FormatOptions {
    /// The variant of the prefix words to use. A variant the counter style
    /// does not declare gets the default words.
    pub variant: Option<String>,
    /// Whether the word before the number is written.
    pub with_word: bool,
    /// Whether the delimiters around the number are written.
    pub with_delimiters: bool,
}

impl Default for FormatOptions {
    fn default() -> FormatOptions {
        FormatOptions { variant: None, with_word: true, with_delimiters: true }
    }
}

impl FormatOptions {
    /// Returns the default options: the default words, with the word and the
    /// delimiters.
    pub fn new() -> FormatOptions {
        FormatOptions::default()
    }

    /// Returns these options with the variant `variant` of the prefix words.
    pub fn variant(mut self, variant: impl Into<String>) -> FormatOptions {
        self.variant = Some(variant.into());
        self
    }

    /// Returns these options without the word before the number.
    pub fn without_word(mut self) -> FormatOptions {
        self.with_word = false;
        self
    }

    /// Returns these options without the delimiters around the number.
    pub fn without_delimiters(mut self) -> FormatOptions {
        self.with_delimiters = false;
        self
    }
}

// --- what comes out ---------------------------------------------------------------------------

/// A segment: a small tree of FLM content that is one stretch of a written
/// number, together with the link mark of the value it links to.
///
/// A segment's content is a synthesized value, content computed by the
/// library instead of parsed from a document ([synthesized
/// values](crate::guide::concepts_overview#synthesized-values)). It is made of
/// the counter style's texts, parsed when the counter style was compiled, and
/// the characters of the numbers. A lowering rule shows it with
/// [`Lowerer::splice`](crate::core::lowering::Lowerer::splice), which brings
/// it up to date first ([value
/// catch-up](crate::guide::concepts_overview#value-catch-up)), so the content
/// may hold a text style or a reference.
///
/// The **link mark** is the position, among the values the caller passed in,
/// of the value the segment links to: `0` for the only value of [`format()`],
/// and the position in the caller's list for
/// [`format_many`](super::format_many), regardless of how the values were
/// sorted and grouped. A segment with no link mark is shown without a link:
/// the texts between two numbers have none. These functions do not depend on
/// reference points; the caller turns a position into a link target (the
/// counter table does so in [`NumberSegment`](super::NumberSegment)).
pub struct Segment {
    content: Arc<NodeTree<Flm>>,
    link: Option<usize>,
}

impl fmt::Debug for Segment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Segment").field("link", &self.link).finish_non_exhaustive()
    }
}

impl Segment {
    /// Returns the segment's content, a synthesized value whose root is a list
    /// node over the content's nodes.
    pub fn content(&self) -> &Arc<NodeTree<Flm>> {
        &self.content
    }

    /// Returns the link mark: the position of the value this segment links
    /// to, or `None` when the segment is not part of a link.
    pub fn link(&self) -> Option<usize> {
        self.link
    }
}

// --- writing one value --------------------------------------------------------------------------

/// Writes one counter value in `style` and returns the segments of the
/// result: under the default writer, the word, the delimiters and the number,
/// as one segment that links to the value.
///
/// `parent` gives the within-joiner and the counters the value is numbered
/// within, and is `None` for a counter numbered within nothing. A
/// counter that the within-joiner refers to is written with its own ancestors
/// before it, so a within-joiner refers only to the counter immediately above
/// ([`Parent`]). `options` says which variant of the prefix words to use and
/// whether the word and the delimiters are written.
///
/// Under the default writer, the result is one segment with the link mark
/// `0`: one link covers the whole text, the word included. A counter style
/// written by another [number writer](super::NumberWriter) may return other
/// segments. The call runs on a worker thread ([`on_worker_thread`](guard::on_worker_thread)).
///
/// # Errors
///
/// [`FormatError::UndeclaredSubLevel`] when the value carries a sub-number at
/// a level for which `style` declares no format, [`FormatError::UnknownAncestor`]
/// when a within-joiner refers to a counter that is not among the ancestors,
/// and [`FormatError::NestingTooDeep`] or [`FormatError::Failed`] when the
/// content could not be built.
///
/// # Examples
///
/// ```
/// use flm_core::lang::{build_language, LangInputs};
/// use flm_core::stdcore::features::numbering::{
///     format, CounterStyleDecl, CounterValue, Delimiters, FormatOptions, PrefixWords,
/// };
///
/// let lang = build_language(LangInputs::new());
/// let mut decl = CounterStyleDecl::new("example.style.equation");
/// decl.delimiters = Delimiters::new("(", ")");
/// decl.prefix_words = PrefixWords::new("Eq.~", "Eqs.~");
/// let style = decl.compile(&lang).expect("the style compiles");
///
/// let segments = format(&CounterValue::new(3), &style, None, &FormatOptions::new())
///     .expect("the number is written");
/// assert_eq!(segments.len(), 1);
/// assert_eq!(segments[0].link(), Some(0));
/// ```
pub fn format(
    value: &CounterValue,
    style: &CounterStyle,
    parent: Option<Parent<'_>>,
    options: &FormatOptions,
) -> Result<Vec<Segment>, FormatError> {
    // Building the pieces walks the chain of ancestors and assembling a
    // segment copies content: both descend recursively, so the whole call runs
    // under the fixed stack budget, on the worker thread the recursion guard's
    // contract asks for. See decision rationale at
    // [§flm-dd-dr:lang-stack-budget].
    on_worker(|| {
        let mut numbered = NumberedValue::new(value);
        if let Some(parent) = parent {
            numbered = numbered.within(parent);
        }
        let cx = WriteCx::new(style);
        let mut out = NumberOut::new();
        style.writer().write_one(&numbered, options, &cx, &mut out)?;
        segments(style, out.into_items())
    })
}

/// Writes one value as the default writer does: the word, the opening
/// delimiter, the parts the value's within-joiner writes, the number with its
/// sub-numbers, and the closing delimiter, as one item that links to the
/// value.
///
/// [`WriteCx::default_one`] calls it, so a registered writer that hands one
/// value to the default writer gets this result.
///
/// # Errors
///
/// The same as [`format()`].
pub(super) fn default_one<'a>(
    value: &NumberedValue<'a>,
    options: &FormatOptions,
    cx: &WriteCx<'a>,
    out: &mut NumberOut<'a>,
) -> Result<(), FormatError> {
    let style = cx.style();
    let mut pieces = Vec::new();
    if options.with_word {
        pieces.push(Piece::Flm(style.word(1, options.variant.as_deref())));
    }
    if options.with_delimiters {
        pieces.push(Piece::Flm(&style.open));
    }
    let own = value.value();
    let sub_numbers: Vec<Option<i64>> = own.sub_numbers().iter().map(|n| Some(*n)).collect();
    number_pieces(style, Some(own.number()), &sub_numbers, Some((value, cx)), &mut pieces)?;
    if options.with_delimiters {
        pieces.push(Piece::Flm(&style.close));
    }
    out.item(Item { pieces, mark: Mark::Value(value.index()) });
    Ok(())
}

/// Returns the parts written before a point's own number: what `parent`'s
/// within-joiner writes of its chain of ancestors.
///
/// The list is the one [`format()`] writes before a number, returned on its
/// own: a caller that writes several numbers itself reads the parts to find
/// which of them two numbers share. `style` is the counter style of the
/// point's own number, to which the within-joiner's own texts are
/// attributed. The call runs on a worker thread.
///
/// # Errors
///
/// [`FormatError::UnknownAncestor`] when the within-joiner refers to a
/// counter that is not in the chain, and [`FormatError::NestingTooDeep`] or
/// [`FormatError::Failed`] when the content could not be built.
pub fn parts(style: &CounterStyle, parent: Parent<'_>) -> Result<Vec<Part>, FormatError> {
    on_worker(|| {
        let cx = JoinCx::new(style);
        parent.joiner.parts(parent.ancestors, &cx)
    })
}

// --- the pieces a number is made of ---------------------------------------------------------

/// One piece of a segment's content before it is assembled: FLM parsed when
/// the counter style was compiled, content built while writing, or characters
/// the library computed.
pub(super) enum Piece<'a> {
    /// A tree of FLM content; its top-level nodes are copied into the segment.
    Flm(&'a Arc<NodeTree<Flm>>),
    /// A tree of content built while writing, which nothing else holds: an
    /// instantiated template for this number, or what a within-joiner wrote.
    Tree(Arc<NodeTree<Flm>>),
    /// Characters, which become one text node.
    Digits(String),
}

/// How a piece of the result is linked.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Mark {
    /// Never part of a link.
    Never,
    /// Part of the link of its neighbors, if they are part of one.
    Either,
    /// Part of the link to the value at this position among the caller's.
    Value(usize),
}

/// One item of the result, before neighboring items with the same mark are
/// merged into a segment.
pub(super) struct Item<'a> {
    pub(super) pieces: Vec<Piece<'a>>,
    pub(super) mark: Mark,
}

impl<'a> Item<'a> {
    /// Returns an item holding one tree of FLM content.
    pub(super) fn text(tree: &'a Arc<NodeTree<Flm>>, mark: Mark) -> Item<'a> {
        Item { pieces: vec![Piece::Flm(tree)], mark }
    }
}

/// Appends the pieces of one number to `out`: the ancestors' parts, the
/// number written with the counter style's number template, and every
/// sub-number.
///
/// `number` is `None` for a number that is not written (the end of a range
/// whose start has the same number), and so is each entry of `sub_numbers`
/// that is not written; a sub-number of zero is written as nothing too. The
/// separator of a sub-level is written only when something is written before
/// it.
///
/// `within` is the value whose ancestors are written before the number,
/// with the writing context through which its parts are requested, and is
/// `None` where nothing of the ancestors is written. The parts are requested
/// only where they are written, so a counter style whose number template
/// writes no ancestors' part never runs the within-joiner.
///
/// # Errors
///
/// [`FormatError::UndeclaredSubLevel`] for a sub-number at a level for which
/// the counter style declares no format, and [`FormatError::UnknownAncestor`]
/// from the ancestors' parts.
pub(super) fn number_pieces<'a>(
    style: &'a CounterStyle,
    number: Option<i64>,
    sub_numbers: &[Option<i64>],
    within: Option<(&NumberedValue<'a>, &WriteCx<'a>)>,
    out: &mut Vec<Piece<'a>>,
) -> Result<(), FormatError> {
    let mut wrote_something = false;
    if let Some(number) = number {
        if style.number.with_parent() {
            if let Some((value, cx)) = within {
                for part in cx.parts(value)? {
                    out.push(Piece::Tree(part.content));
                }
            }
        }
        template_pieces(style, &style.number, number, out)?;
        wrote_something = true;
    }
    sub_level_pieces(style, sub_numbers, None, &mut wrote_something, out)
}

/// Appends the pieces of the sub-numbers of one number: for each, the
/// separator its level declares, then the sub-number itself.
///
/// Every sub-number is written by this function. A sub-number that is `None`
/// or zero is written as nothing; the separator is written only when something
/// is written before it, which `wrote_something` says and which this function
/// sets as soon as it writes anything. `format`, when given, writes every
/// sub-number in that number format instead of with its level's own number
/// template.
///
/// # Errors
///
/// [`FormatError::UndeclaredSubLevel`] for a sub-number at a level for which
/// the counter style declares no format.
fn sub_level_pieces<'a>(
    style: &'a CounterStyle,
    sub_numbers: &[Option<i64>],
    format: Option<NumberFormat>,
    wrote_something: &mut bool,
    out: &mut Vec<Piece<'a>>,
) -> Result<(), FormatError> {
    for (level, sub_number) in sub_numbers.iter().enumerate() {
        let Some(sub_number) = *sub_number else { continue };
        if sub_number == 0 {
            continue;
        }
        let Some(declared) = style.sub_levels.get(level) else {
            return Err(FormatError::UndeclaredSubLevel {
                style: style.name.clone(),
                level,
                number: sub_number,
            });
        };
        if *wrote_something {
            out.push(Piece::Flm(&declared.separator));
        }
        match format {
            Some(format) => out.push(Piece::Digits(format.text(sub_number))),
            None => template_pieces(style, &declared.number, sub_number, out)?,
        }
        *wrote_something = true;
    }
    Ok(())
}

/// Appends the pieces of a compiled number template written for `number`.
///
/// A template written in the template language is instantiated here: each
/// `#{…}` is replaced by the number written in the format it gives, and the
/// result is one tree of content. A single-letter number template contributes
/// its parsed literal parts with the number between them.
///
/// # Errors
///
/// [`FormatError`] when the content could not be built.
fn template_pieces<'a>(
    style: &CounterStyle,
    template: &'a CompiledNumber,
    number: i64,
    out: &mut Vec<Piece<'a>>,
) -> Result<(), FormatError> {
    match template {
        CompiledNumber::Initial { parts, .. } => {
            for part in parts {
                match part {
                    NumberPart::Literal(tree) => out.push(Piece::Flm(tree)),
                    NumberPart::Number(format) => out.push(Piece::Digits(format.text(number))),
                }
            }
            Ok(())
        }
        CompiledNumber::Written { template, formats } => {
            let mut entries = Vec::with_capacity(formats.len());
            for format in formats {
                entries.push(Binding {
                    name: format.written_name().to_string(),
                    keys: String::new(),
                    content: assemble_pieces(style, &[Piece::Digits(format.text(number))])?,
                });
            }
            let bindings = Bindings::new(entries);
            out.push(Piece::Tree(instantiate_template(&style.site(), template, &bindings)?));
            Ok(())
        }
    }
}

/// Returns `value` written in `style` as a bare number: no word, no
/// delimiters, and nothing of any counter it is numbered within.
///
/// `format`, when given, writes the number and every sub-number in that format
/// instead of with the counter style's own number templates, as the key of
/// `#[Roman]{…}` asks; the counter style's sub-level separators are used in
/// both cases.
///
/// # Errors
///
/// [`FormatError::UndeclaredSubLevel`] for a sub-number at a level for which
/// the counter style declares no format, and a failure to build the content.
pub(super) fn number_tree(
    style: &CounterStyle,
    value: &CounterValue,
    format: Option<NumberFormat>,
) -> Result<Arc<NodeTree<Flm>>, FormatError> {
    let sub_numbers: Vec<Option<i64>> = value.sub_numbers().iter().map(|n| Some(*n)).collect();
    let mut pieces = Vec::new();
    match format {
        None => number_pieces(style, Some(value.number()), &sub_numbers, None, &mut pieces)?,
        Some(format) => {
            pieces.push(Piece::Digits(format.text(value.number())));
            let mut wrote_something = true;
            sub_level_pieces(style, &sub_numbers, Some(format), &mut wrote_something, &mut pieces)?;
        }
    }
    assemble_pieces(style, &pieces)
}

// --- from items to segments --------------------------------------------------------------------

/// Returns the segments that `items` become: neighboring items that belong
/// to one link (or to none) are merged, and each merged sequence is assembled
/// into one synthesized value.
///
/// A segment that holds no content and links to nothing is left out.
///
/// The function assembles content, which descends recursively, so it must be
/// called on a worker thread; the two entry points [`format()`] and
/// [`format_many`](super::format_many) run their whole work on one.
///
/// # Errors
///
/// [`FormatError::NestingTooDeep`] or [`FormatError::Failed`] from the
/// assembly of a value tree.
pub(super) fn segments(
    style: &CounterStyle,
    items: Vec<Item<'_>>,
) -> Result<Vec<Segment>, FormatError> {
    let mut merged: Vec<Item<'_>> = Vec::new();
    for item in items {
        match merged.last_mut() {
            Some(last) if last.mark == Mark::Never && item.mark == Mark::Never => {
                last.pieces.extend(item.pieces);
                continue;
            }
            Some(last)
                if last.mark != Mark::Never
                    && item.mark != Mark::Never
                    && (item.mark == Mark::Either
                        || last.mark == Mark::Either
                        || item.mark == last.mark) =>
            {
                if let (Mark::Value(_), Mark::Either) = (item.mark, last.mark) {
                    last.mark = item.mark;
                }
                last.pieces.extend(item.pieces);
                continue;
            }
            _ => {}
        }
        merged.push(item);
    }

    let mut out = Vec::with_capacity(merged.len());
    for item in merged {
        let link = match item.mark {
            Mark::Value(position) => Some(position),
            Mark::Never | Mark::Either => None,
        };
        let content = assemble_pieces(style, &item.pieces)?;
        if link.is_none() && content.root().child_count() == 0 {
            continue;
        }
        out.push(Segment { content, link });
    }
    Ok(out)
}

/// Runs `work` on a worker thread and returns its result; the operating
/// system's refusal to create the thread becomes [`FormatError::Failed`].
///
/// Every entry point that writes a number goes through this function: the
/// writing walks the chain of ancestors and copies content, both recursively,
/// and the recursion guard's fixed stack budget can be met only on a worker
/// thread ([`on_worker_thread`](guard::on_worker_thread)).
/// A call made from a thread that is already a worker thread, as every stage
/// of the pipeline is, runs `work` in place.
pub(super) fn on_worker<T: Send>(
    work: impl FnOnce() -> Result<T, FormatError> + Send,
) -> Result<T, FormatError> {
    match guard::on_worker_thread(work) {
        Ok(result) => result,
        Err(error) => Err(FormatError::Failed { detail: error.to_string() }),
    }
}

// --- assembling a value tree ---------------------------------------------------------------

/// The origin name under which a counter style's text is parsed: the name
/// that a diagnostic about that text gives as the text's origin.
pub(super) const STYLE_TEXT_ORIGIN: &str = "counter style text";

/// Parses `text`, a counter style's text, as FLM against the built language
/// and returns its tree: [`parse_text_from`] with the origin name
/// [`STYLE_TEXT_ORIGIN`].
///
/// # Errors
///
/// As [`parse_text_from`].
pub(super) fn parse_text(
    text: &str,
    lang: &BuiltLang,
    warnings: &mut Vec<FlmDiagnostic>,
) -> Result<Arc<NodeTree<Flm>>, StyleError> {
    parse_text_from(text, STYLE_TEXT_ORIGIN, lang, warnings)
}

/// Parses `text` as FLM against the built language and returns its tree;
/// `origin` is the origin name that the diagnostics of the parse give.
///
/// Warnings are added to `warnings`; an error refuses the text, since the text
/// is configuration and a mistake in it is reported when the registry is
/// built.
///
/// # Errors
///
/// [`StyleError::Parse`] when the parse reported errors, and
/// [`StyleError::ParseFailed`] when it could not run at all.
fn parse_text_from(
    text: &str,
    origin: &str,
    lang: &BuiltLang,
    warnings: &mut Vec<FlmDiagnostic>,
) -> Result<Arc<NodeTree<Flm>>, StyleError> {
    let origin = FlmOrigin::synthetic(origin);
    let source = Arc::new(FlmSource::new(text.to_string()).with_origin(origin));
    let fragment =
        parse_fragment(lang, source, FragmentLayout::default(), &ParseOptions::default())
            .map_err(|error| StyleError::ParseFailed {
                text: text.to_string(),
                detail: error.to_string(),
            })?;
    let errors: Vec<FlmDiagnostic> = fragment
        .diags()
        .iter()
        .filter(|diagnostic| diagnostic.severity() == Severity::Error)
        .cloned()
        .collect();
    if !errors.is_empty() {
        return Err(StyleError::Parse { text: text.to_string(), diagnostics: errors });
    }
    warnings.extend(
        fragment
            .diags()
            .iter()
            .filter(|diagnostic| diagnostic.severity() != Severity::Error)
            .cloned(),
    );
    Ok(Arc::clone(fragment.tree()))
}

/// Compiles one pair of words.
fn compile_words(
    words: &PluralWords,
    lang: &BuiltLang,
    warnings: &mut Vec<FlmDiagnostic>,
) -> Result<CompiledPlural, StyleError> {
    Ok(CompiledPlural {
        one: parse_text(&words.one, lang, warnings)?,
        other: parse_text(&words.other, lang, warnings)?,
    })
}

/// A visitor that carries every node over unchanged.
struct Verbatim;

impl RestageVisitor<Flm, (), ()> for Verbatim {
    type Error = Infallible;

    fn restage(
        &mut self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, Infallible> {
        Ok(Restage::Descend(()))
    }
}

/// The visitor that assembles a segment: called once, at the root of the
/// counter style's empty tree, it emits a list node that holds a copy of every
/// piece.
struct Assemble<'a, 'p> {
    style: &'a CounterStyle,
    pieces: &'p [Piece<'a>],
}

impl RestageVisitor<Flm, (), ()> for Assemble<'_, '_> {
    type Error = FormatError;

    fn restage(
        &mut self,
        root: NodeRef<'_, Flm>,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, FormatError> {
        let mut children: Vec<BuildId> = Vec::new();
        for piece in self.pieces {
            match piece {
                Piece::Flm(tree) => {
                    for node in tree.root().children().iter() {
                        let copied = cx
                            .restage_subtree(node, &mut Verbatim)
                            .map_err(|error| failed(&error))?;
                        children.extend(copied);
                    }
                }
                Piece::Tree(tree) => {
                    for node in tree.root().children().iter() {
                        let copied = cx
                            .restage_subtree(node, &mut Verbatim)
                            .map_err(|error| failed(&error))?;
                        children.extend(copied);
                    }
                }
                Piece::Digits(text) if !text.is_empty() => {
                    children.push(self.digits(cx, text)?);
                }
                Piece::Digits(_) => {}
            }
        }
        let span = SourceSpan::new(root.span().source(), root.span().start()..root.span().start());
        let state = Arc::clone(&self.style.state);
        let kind = NodeKind::list();
        let ext = Flm::make_node_ext(&kind, &span, &state, cx.builder().staged_children(&children))
            .map_err(|error| FormatError::Failed { detail: error.to_string() })?;
        let list = cx
            .builder()
            .add(kind, span, state, children, ext, ())
            .map_err(|error| FormatError::Failed { detail: error.to_string() })?;
        Ok(Restage::Emit(vec![list]))
    }
}

impl Assemble<'_, '_> {
    /// Stages (builds into the tree being assembled) one text node that holds
    /// the characters of a number. The node is attributed to the counter
    /// style's own source, where a diagnostic about it points.
    fn digits(
        &self,
        cx: &mut RestageContext<'_, Flm, (), ()>,
        text: &str,
    ) -> Result<BuildId, FormatError> {
        let at: &FlmSpan = self.style.blank.root().span();
        let span = SourceSpan::new(at.source(), at.start()..at.start());
        let state = Arc::clone(&self.style.state);
        let kind = NodeKind::chars(TextContent::Owned(Box::from(text)));
        let ext = Flm::make_node_ext(&kind, &span, &state, cx.builder().staged_children(&[]))
            .map_err(|error| FormatError::Failed { detail: error.to_string() })?;
        cx.builder()
            .add(kind, span, state, Vec::new(), ext, ())
            .map_err(|error| FormatError::Failed { detail: error.to_string() })
    }
}

/// Returns a restage failure as this module's [`FormatError`].
fn failed<E: fmt::Display>(error: &RestageError<E>) -> FormatError {
    match error {
        RestageError::DescentLimitExceeded { detail } => {
            FormatError::NestingTooDeep { detail: detail.clone() }
        }
        other => FormatError::Failed { detail: other.to_string() },
    }
}

/// Returns the synthesized value that the pieces make: a list node over a copy
/// of each piece's nodes, with one text node per piece of characters.
///
/// It descends recursively under the fixed stack budget, so it must be called
/// on a worker thread, which [`on_worker`] provides.
pub(super) fn assemble_pieces(
    style: &CounterStyle,
    pieces: &[Piece<'_>],
) -> Result<Arc<NodeTree<Flm>>, FormatError> {
    let mut visitor = Assemble { style, pieces };
    guard::restager(&mut visitor, GuardPolicy::StackBudget)
        .restage(&style.blank)
        .map_err(|error| failed(&error))
        .map(Arc::new)
}

// --- the join texts, resolved ------------------------------------------------------------------

/// The join keys, in the order of the fields of [`JoinDecl`], with the default
/// text of each.
const JOIN_DEFAULTS: [(&str, &str); 17] = [
    ("one_pre", ""),
    ("one_post", ""),
    ("pair_pre", ""),
    ("pair_mid", " ${and} "),
    ("pair_post", ""),
    ("range_pre", ""),
    ("range_mid", "${endash}"),
    ("range_pairmid", "${sep}"),
    ("range_post", ""),
    ("list_pre", ""),
    ("list_mid", "${sep} "),
    ("list_midlast", "${sep} ${and} "),
    ("list_post", ""),
    ("and", "and"),
    ("sep", ","),
    ("endash", "\u{2013}"),
    ("empty", "(empty)"),
];

impl CompiledJoin {
    /// Returns the join text `which`.
    pub(crate) fn text(&self, which: JoinText) -> &Arc<NodeTree<Flm>> {
        match which {
            JoinText::OnePre => &self.one_pre,
            JoinText::OnePost => &self.one_post,
            JoinText::PairPre => &self.pair_pre,
            JoinText::PairMid => &self.pair_mid,
            JoinText::PairPost => &self.pair_post,
            JoinText::RangePre => &self.range_pre,
            JoinText::RangeMid => &self.range_mid,
            JoinText::RangePairMid => &self.range_pairmid,
            JoinText::RangePost => &self.range_post,
            JoinText::ListPre => &self.list_pre,
            JoinText::ListMid => &self.list_mid,
            JoinText::ListMidLast => &self.list_midlast,
            JoinText::ListPost => &self.list_post,
            JoinText::Empty => &self.empty,
        }
    }

    /// Returns the declaration's texts compiled: the variables replaced and
    /// every text parsed; a key the declaration leaves `None` takes its
    /// default.
    ///
    /// `origin` is the origin name that the diagnostics of the parse give:
    /// [`STYLE_TEXT_ORIGIN`] for a counter style's texts, another name for the
    /// texts of another owner. Warnings are added to `warnings`.
    ///
    /// # Errors
    ///
    /// The [`StyleError`] of the first text that refers to a variable that is
    /// not a join key or that is not valid FLM.
    pub(crate) fn compile(
        decl: &JoinDecl,
        origin: &str,
        lang: &BuiltLang,
        warnings: &mut Vec<FlmDiagnostic>,
    ) -> Result<CompiledJoin, StyleError> {
        let declared: [(&str, &Option<String>); 17] = [
            ("one_pre", &decl.one_pre),
            ("one_post", &decl.one_post),
            ("pair_pre", &decl.pair_pre),
            ("pair_mid", &decl.pair_mid),
            ("pair_post", &decl.pair_post),
            ("range_pre", &decl.range_pre),
            ("range_mid", &decl.range_mid),
            ("range_pairmid", &decl.range_pairmid),
            ("range_post", &decl.range_post),
            ("list_pre", &decl.list_pre),
            ("list_mid", &decl.list_mid),
            ("list_midlast", &decl.list_midlast),
            ("list_post", &decl.list_post),
            ("and", &decl.and),
            ("sep", &decl.sep),
            ("endash", &decl.endash),
            ("empty", &decl.empty),
        ];
        let mut raw: BTreeMap<&str, &str> = JOIN_DEFAULTS.into_iter().collect();
        for (key, value) in declared {
            if let Some(value) = value {
                raw.insert(key, value);
            }
        }
        let mut parsed: BTreeMap<&str, Arc<NodeTree<Flm>>> = BTreeMap::new();
        for (key, _) in JOIN_DEFAULTS {
            let resolved = substitute_variables(raw[key], &raw)?;
            parsed.insert(key, parse_text_from(&resolved, origin, lang, warnings)?);
        }
        let mut take = |key: &str| parsed.remove(key).expect("every join key was parsed");
        Ok(CompiledJoin {
            one_pre: take("one_pre"),
            one_post: take("one_post"),
            pair_pre: take("pair_pre"),
            pair_mid: take("pair_mid"),
            pair_post: take("pair_post"),
            range_pre: take("range_pre"),
            range_mid: take("range_mid"),
            range_pairmid: take("range_pairmid"),
            range_post: take("range_post"),
            list_pre: take("list_pre"),
            list_mid: take("list_mid"),
            list_midlast: take("list_midlast"),
            list_post: take("list_post"),
            empty: take("empty"),
        })
    }
}

/// One piece of a list joined by the join texts: a join text, or the item at
/// a position of the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ListPiece {
    /// The join text of this key.
    Text(JoinText),
    /// The item at this position, counted from zero.
    Item(usize),
}

/// Returns the places of the join texts in a list of `count` items: one item
/// is `one_pre`, the item, `one_post`; two items are `pair_pre`, the first,
/// `pair_mid`, the second, `pair_post`; more are `list_pre`, the items with
/// `list_mid` between them and `list_midlast` before the last, `list_post`.
/// No item gives no piece.
///
/// The default number writer joins the runs of a counter style's numbers with
/// it, and the refs feature joins the items of a reference to several targets
/// with it, so that both follow the same rule.
pub(crate) fn list_shape(count: usize) -> Vec<ListPiece> {
    let mut pieces = Vec::new();
    match count {
        0 => {}
        1 => {
            pieces.push(ListPiece::Text(JoinText::OnePre));
            pieces.push(ListPiece::Item(0));
            pieces.push(ListPiece::Text(JoinText::OnePost));
        }
        2 => {
            pieces.push(ListPiece::Text(JoinText::PairPre));
            pieces.push(ListPiece::Item(0));
            pieces.push(ListPiece::Text(JoinText::PairMid));
            pieces.push(ListPiece::Item(1));
            pieces.push(ListPiece::Text(JoinText::PairPost));
        }
        _ => {
            pieces.push(ListPiece::Text(JoinText::ListPre));
            for index in 0..count - 1 {
                if index > 0 {
                    pieces.push(ListPiece::Text(JoinText::ListMid));
                }
                pieces.push(ListPiece::Item(index));
            }
            pieces.push(ListPiece::Text(JoinText::ListMidLast));
            pieces.push(ListPiece::Item(count - 1));
            pieces.push(ListPiece::Text(JoinText::ListPost));
        }
    }
    pieces
}

/// Returns the name of the variable at the start of `text`, and how many bytes
/// it takes, or `None` when `text` does not start with one.
///
/// A variable is `${`, then one or more ASCII letters, digits, `_` and `.`,
/// then `}`. This syntax belongs to the join texts only: a number template and
/// a within template are written in the template language, where a
/// substitution is `#{…}`.
fn substitution_at(text: &str) -> Option<(&str, usize)> {
    let rest = text.strip_prefix("${")?;
    let end = rest.find('}')?;
    let name = &rest[..end];
    if name.is_empty()
        || !name.chars().all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '.')
    {
        return None;
    }
    Some((name, 2 + end + 1))
}

/// Returns `text` with every `${name}` replaced by `values[name]`, in one
/// pass: a replaced text is not scanned again.
///
/// # Errors
///
/// [`StyleError::UnknownVariable`] for a name that `values` does not hold.
fn substitute_variables(
    text: &str,
    values: &BTreeMap<&str, &str>,
) -> Result<String, StyleError> {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find("${") {
        match substitution_at(&rest[start..]) {
            Some((name, length)) => {
                let Some(value) = values.get(name) else {
                    return Err(StyleError::UnknownVariable {
                        text: text.to_string(),
                        name: name.to_string(),
                    });
                };
                out.push_str(&rest[..start]);
                out.push_str(value);
                rest = &rest[start + length..];
            }
            None => {
                out.push_str(&rest[..start + 2]);
                rest = &rest[start + 2..];
            }
        }
    }
    out.push_str(rest);
    Ok(out)
}

// --- what `format_many` reads off a style -----------------------------------------------------

impl CounterStyle {
    /// Returns the word for `count` numbers in the variant `variant`, or
    /// `None` when that word is the empty text and nothing is written.
    pub(crate) fn word_text(
        &self,
        count: usize,
        variant: Option<&str>,
    ) -> Option<&Arc<NodeTree<Flm>>> {
        let word = self.word(count, variant);
        (word.root().child_count() > 0).then_some(word)
    }

    /// Returns the opening delimiter, the text before a number.
    pub(crate) fn open(&self) -> &Arc<NodeTree<Flm>> {
        &self.open
    }

    /// Returns the closing delimiter, the text after a number.
    pub(crate) fn close(&self) -> &Arc<NodeTree<Flm>> {
        &self.close
    }

    /// Returns whether the word before the numbers is part of the link to the
    /// first number written.
    pub(super) fn name_in_link(&self) -> bool {
        self.name_in_link
    }

    /// Returns whether the end of a range repeats the ancestors' parts.
    pub(super) fn repeat_parents_in_range(&self) -> bool {
        self.repeat_parents_in_range
    }

    /// Returns whether consecutive numbers collapse into a range.
    pub(super) fn ranges(&self) -> bool {
        self.ranges
    }

    /// Returns the number writer of this counter style.
    pub(super) fn writer(&self) -> &Arc<dyn CompiledNumberWriter> {
        &self.writer
    }

    /// Returns the counter style's compiled join texts.
    pub(crate) fn join(&self) -> &CompiledJoin {
        &self.join
    }
}
