//! The configuration of the `citations` feature ([`CitationsConfig`]): where
//! the citation entries are shown, the form of a citation mark, the citation
//! counter style, the heading of the list of cited works, and the text written
//! before a note.
//!
//! The doc comments of the configuration's types and fields are also the
//! descriptions of the feature's settings in the settings schema.

use std::hash::{Hash, Hasher};

use serde::Deserialize;

use crate::core::settings::named;

use crate::stdcore::features::numbering::{
    digest_style, CounterStyleDecl, Delimiters, JoinDecl, PluralWords, PrefixWords,
};

/// The name of the citation counter style, `flm.citations.style.citation`
/// ([`default_style`]). The citation counter style is the feature's own counter
/// style; a configuration read from settings data keeps this name.
pub const STYLE_NAME: &str = "flm.citations.style.citation";

/// The name of the citation counter style's variant of words that the flag `S` of
/// `\R` selects, `capital`: `Ref.~` and `Refs.~` rather than `ref.~` and
/// `refs.~`.
///
/// The `refs` feature gives the same name to its capitalized variant
/// ([`refs::CAPITAL_VARIANT`](crate::stdcore::features::refs::CAPITAL_VARIANT)),
/// so one counter style can serve both features.
pub const CAPITAL_VARIANT: &str = "capital";

/// Where the citation entries of the cited works are shown. In settings data,
/// `listed` or `in-place`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum CitationPlacement {
    /// Listed after the content, under a heading, one citation entry per cited
    /// work in the order of the numbers; a citation mark shows the numbers of
    /// its works, as its form ([`MarkForm`]) says. The default.
    #[default]
    Listed,
    /// In place: a citation mark shows the citation entries themselves, one
    /// group per item between the citation counter style's delimiters (`[Knuth,
    /// 1984][Lamport, 1994; Thm 4]`), and no list is placed. The form of a
    /// citation mark has no effect.
    InPlace,
}

impl CitationPlacement {
    /// Returns the byte the placement writes into the registry digest.
    fn digest_byte(self) -> u8 {
        match self {
            CitationPlacement::Listed => 0,
            CitationPlacement::InPlace => 1,
        }
    }
}

/// How a citation mark shows the numbers of the works its citation chunk
/// cites, where the citation entries are listed after the content
/// ([`CitationPlacement::Listed`]); it has no effect under the in-place
/// placement. In settings data, `bracketed` or `superscript`.
///
/// The form decides the delimiters wherever numbers are shown. Under the
/// bracketed form, a citation mark and an entry's label in the list of cited
/// works carry the citation counter style's delimiters; under the superscript
/// form they carry none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum MarkForm {
    /// The numbers inline, between the citation counter style's delimiters:
    /// `text [1,3–5]`, with a no-break space written before the citation mark
    /// of a `\cite`. The default.
    #[default]
    Bracketed,
    /// The numbers raised, with no delimiters: `text¹˒³⁻⁵`, with nothing
    /// written before the citation mark of a `\cite`. A citation written with
    /// `\R` is never raised: its numbers are shown inline, with no delimiters
    /// (`refs.~1,2`).
    Superscript,
}

impl MarkForm {
    /// Returns the byte the form writes into the registry digest.
    fn digest_byte(self) -> u8 {
        match self {
            MarkForm::Bracketed => 0,
            MarkForm::Superscript => 1,
        }
    }
}

/// The configuration of the `citations` feature: where the citation entries are
/// shown, the form of a citation mark, the citation counter style, the heading
/// of the list of cited works, and the text between the numbers and a note.
///
/// It is the plain Rust value the feature is constructed from
/// ([`Citations::new`](super::Citations::new)), fixed when the registry is
/// built ([configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
/// It enters the [registry digest](crate::model::digest)
/// ([`digest_bytes`](CitationsConfig::digest_bytes)), and its texts are
/// compiled when the registry is built: a text that is not valid FLM fails
/// the build with a message that identifies the text. The standard package
/// takes it as
/// [`StdcoreConfig::citations`](crate::stdcore::StdcoreConfig::citations).
///
/// The citation counter style is the feature's own: the counter styles a host
/// sets in the numbering feature's configuration
/// ([`NumberingConfig::styles`](crate::stdcore::features::numbering::NumberingConfig::styles))
/// do not replace it, and a host changes it here.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::citations::{CitationsConfig, MarkForm};
/// use flm_core::stdcore::features::numbering::Delimiters;
///
/// let mut config = CitationsConfig::new().mark(MarkForm::Superscript);
/// config.style.delimiters = Delimiters::new("(", ")");
/// assert_eq!(config.note_separator, "; ");
/// assert!(config.style.ranges);
/// assert_eq!(config.heading_title.as_deref(), Some("References"));
///
/// // The list with no heading above it.
/// assert_eq!(CitationsConfig::new().without_heading().heading_title, None);
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CitationsConfig {
    /// Where the citation entries are shown: listed after the content, the
    /// default, or in the citation marks in place of the numbers
    /// ([`CitationPlacement`]).
    pub placement: CitationPlacement,
    /// How a citation mark shows its numbers under the listed placement, and
    /// how the list of cited works labels its entries: [`MarkForm::Bracketed`]
    /// by default. It has no effect under [`CitationPlacement::InPlace`].
    pub mark: MarkForm,
    /// The citation counter style, a counter style
    /// ([`CounterStyleDecl`]): how a number is written, the delimiters, the
    /// texts joining several numbers, whether consecutive numbers make a
    /// range, and the words `\R` writes before a citation mark. By default
    /// ([`default_style`]): arabic numbers, `[` and `]`, the compact join,
    /// ranges on, the words `ref.~` and `refs.~`, and the variant `capital`
    /// with the words `Ref.~` and `Refs.~`.
    ///
    /// In settings data, the fields written replace those of the default
    /// citation counter style, and the name is not written: it is always
    /// [`STYLE_NAME`].
    #[serde(deserialize_with = "read_style")]
    pub style: CounterStyleDecl,
    /// The title of the heading above the list of cited works, as FLM text:
    /// `References` by default; `None` for a list with no heading above it
    /// ([`without_heading`](CitationsConfig::without_heading)).
    pub heading_title: Option<String>,
    /// The level of the heading above the list of cited works, counted from
    /// zero like every heading level; 2 by default, the level of the heading
    /// above a list of footnotes. The feature passes it to the heading
    /// unchanged, and accepts every value. It has no effect on a list with no
    /// heading.
    pub heading_level: u8,
    /// The text written between a group's numbers and its note, as FLM text:
    /// `; ` by default, as in `[1; Thm 4]`.
    pub note_separator: String,
}

/// Returns the default citation counter style, named [`STYLE_NAME`]: arabic
/// numbers, the delimiters `[` and `]`, the compact join (`1,2` and `1–3,5`),
/// ranges on, and the words `ref.~` and `refs.~`, with the variant
/// [`CAPITAL_VARIANT`] holding `Ref.~` and `Refs.~`.
pub fn default_style() -> CounterStyleDecl {
    let mut style = CounterStyleDecl::new(STYLE_NAME);
    style.delimiters = Delimiters::new("[", "]");
    style.join = JoinDecl::compact();
    style.prefix_words = PrefixWords::new("ref.~", "refs.~")
        .with_variant(CAPITAL_VARIANT, PluralWords::new("Ref.~", "Refs.~"));
    style
}

/// Reads the citation counter style from settings data over [`default_style`],
/// keeping its name; refuses a map that writes `name`.
fn read_style<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<CounterStyleDecl, D::Error> {
    use serde::de::Error;
    let written = match serde_json::Value::deserialize(deserializer)? {
        serde_json::Value::Object(written) => written,
        other => return Err(D::Error::custom(format!(
            "the option `style` of the feature `citations` must be a map, not `{other}`"
        ))),
    };
    if written.contains_key("name") {
        return Err(D::Error::custom(
            "the option `style` of the feature `citations` may not set `name`",
        ));
    }
    named::read_fields_over(&default_style(), written).map_err(D::Error::custom)
}

impl Default for CitationsConfig {
    fn default() -> CitationsConfig {
        CitationsConfig {
            placement: CitationPlacement::default(),
            mark: MarkForm::default(),
            style: default_style(),
            heading_title: Some("References".to_string()),
            heading_level: 2,
            note_separator: "; ".to_string(),
        }
    }
}

impl CitationsConfig {
    /// Returns the default configuration: the citation entries listed, the
    /// bracketed form, the default citation counter style ([`default_style`]), the
    /// title `References` at level 2, and `; ` before a note.
    pub fn new() -> CitationsConfig {
        CitationsConfig::default()
    }

    /// Builder: sets [`placement`](CitationsConfig::placement).
    pub fn placement(mut self, placement: CitationPlacement) -> CitationsConfig {
        self.placement = placement;
        self
    }

    /// Builder: sets [`mark`](CitationsConfig::mark).
    pub fn mark(mut self, mark: MarkForm) -> CitationsConfig {
        self.mark = mark;
        self
    }

    /// Builder: sets [`style`](CitationsConfig::style), the citation counter style.
    /// The name of `style` is used as given.
    pub fn style(mut self, style: CounterStyleDecl) -> CitationsConfig {
        self.style = style;
        self
    }

    /// Builder: sets [`heading_title`](CitationsConfig::heading_title), the
    /// title of the heading above the list of cited works.
    pub fn heading_title(mut self, title: impl Into<String>) -> CitationsConfig {
        self.heading_title = Some(title.into());
        self
    }

    /// Builder: no heading above the list of the cited works.
    pub fn without_heading(mut self) -> CitationsConfig {
        self.heading_title = None;
        self
    }

    /// Builder: sets [`heading_level`](CitationsConfig::heading_level).
    pub fn heading_level(mut self, level: u8) -> CitationsConfig {
        self.heading_level = level;
        self
    }

    /// Builder: sets [`note_separator`](CitationsConfig::note_separator).
    pub fn note_separator(mut self, separator: impl Into<String>) -> CitationsConfig {
        self.note_separator = separator.into();
        self
    }

    /// Returns the bytes the feature gives to the registry digest
    /// ([`FeatureBuilder::digest_input`](crate::core::registry::FeatureBuilder::digest_input)):
    /// the placement, the form, every field of the citation counter style as the
    /// numbering feature writes a counter style's, whether the list has a
    /// heading and its title, the note separator and the heading's level, each
    /// text preceded by its length.
    pub fn digest_bytes(&self) -> Vec<u8> {
        // Taken apart by name, so a field added and not written here fails
        // to compile.
        let CitationsConfig { placement, mark, style, heading_title, heading_level, note_separator } = self;
        let mut out = vec![placement.digest_byte(), mark.digest_byte()];
        digest_style(&mut out, style);
        let text = |out: &mut Vec<u8>, text: &String| {
            out.extend_from_slice(&(text.len() as u64).to_le_bytes());
            out.extend_from_slice(text.as_bytes());
        };
        match heading_title {
            Some(title) => {
                out.push(1);
                text(&mut out, title);
            }
            None => out.push(0),
        }
        text(&mut out, note_separator);
        out.push(*heading_level);
        out
    }
}

impl Hash for CitationsConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest_bytes());
    }
}
