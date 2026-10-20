//! The configuration of the `quote` feature: which quote environments exist,
//! which quotation-section commands each one accepts, and which kind of
//! quotation section its bare content (content written outside those
//! commands) is read as. The configuration is the value the feature is
//! constructed from, fixed when the registry is built
//! ([configuration](crate::guide::concepts_overview#settings-options-and-configuration)).

use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};

use crate::stdcore::backends::pres::LIST_RESERVED_KINDS;

/// The kind of one quotation section of a quote environment, and the
/// quotation-section command that writes it.
///
/// - [`Text`](QuoteSection::Text) — a quoted passage, one paragraph:
///   `\text{…}`.
/// - [`Lines`](QuoteSection::Lines) — content set line by line, each `\\`
///   ending a line and each `\indent` at the start of a line indenting it by
///   one tab: `\lines{…}`.
/// - [`Attributed`](QuoteSection::Attributed) — the attribution, an author or
///   a source: `\attributed{…}`.
/// - [`Block`](QuoteSection::Block) — ordinary paragraphs and blocks set as a
///   quotation: `\block{…}`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum QuoteSection {
    /// A quoted passage, one paragraph, written `\text{…}`.
    Text,
    /// Lines set as written, written `\lines{…}`.
    Lines,
    /// The attribution, written `\attributed{…}`.
    Attributed,
    /// Paragraphs and blocks, written `\block{…}`.
    Block,
}

impl QuoteSection {
    /// Every section kind, in declaration order.
    pub const ALL: [QuoteSection; 4] =
        [QuoteSection::Text, QuoteSection::Lines, QuoteSection::Attributed, QuoteSection::Block];

    /// Returns the kind's stable name, which is also the name of the
    /// quotation-section command that writes it: `text`, `lines`,
    /// `attributed` or `block`. Settings write a kind by this name.
    pub fn as_str(self) -> &'static str {
        match self {
            QuoteSection::Text => "text",
            QuoteSection::Lines => "lines",
            QuoteSection::Attributed => "attributed",
            QuoteSection::Block => "block",
        }
    }

    /// Returns the kind whose stable name ([`as_str`](QuoteSection::as_str))
    /// is `name`, or `None` when no kind has that name.
    pub fn parse(name: &str) -> Option<QuoteSection> {
        QuoteSection::ALL.into_iter().find(|section| section.as_str() == name)
    }

    /// Returns the role of the block a quotation section of this kind is
    /// staged as:
    /// [`TEXT_ROLE`](super::TEXT_ROLE), [`LINES_ROLE`](super::LINES_ROLE),
    /// [`ATTRIBUTED_ROLE`](super::ATTRIBUTED_ROLE) or
    /// [`BLOCK_ROLE`](super::BLOCK_ROLE).
    pub fn role(self) -> &'static str {
        match self {
            QuoteSection::Text => super::TEXT_ROLE,
            QuoteSection::Lines => super::LINES_ROLE,
            QuoteSection::Attributed => super::ATTRIBUTED_ROLE,
            QuoteSection::Block => super::BLOCK_ROLE,
        }
    }

    /// Returns whether a quotation section of this kind holds inline content
    /// (a passage, lines or an attribution) rather than paragraphs and blocks.
    pub fn is_inline(self) -> bool {
        !matches!(self, QuoteSection::Block)
    }
}

/// One quote environment: the quotation-section commands its body accepts,
/// and the kind of quotation section its **bare content** (content written in
/// the body outside any quotation-section command) is read as.
///
/// An environment accepts exactly the quotation-section commands of
/// [`sections`](QuoteEnvironment::sections), a set: the other commands are
/// not declared in its body, and the order of the set has no meaning (an
/// author writes the quotation sections in any order). Where
/// [`bare`](QuoteEnvironment::bare) holds a kind, each run of bare content is
/// read as one quotation section of that kind; where it holds none, bare
/// content is an error (the condition
/// [`BareContent`](super::BareContent)). An environment must accept a
/// quotation-section command or read bare content, or
/// [`QuoteConfig::validate`] refuses the configuration when the registry is
/// built.
///
/// The bare-content kind also decides how the body is laid out
/// ([`holds_blocks`](QuoteEnvironment::holds_blocks)): as paragraphs and
/// blocks when it is [`Block`](QuoteSection::Block) or when there is none, so
/// that each quotation-section command is a block of its own; as inline
/// content when it is [`Text`](QuoteSection::Text),
/// [`Lines`](QuoteSection::Lines) or [`Attributed`](QuoteSection::Attributed),
/// so that the quotation sections and the bare content form one run.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::quote::{QuoteEnvironment, QuoteSection};
///
/// // A quotation of one passage and its attribution, nothing else.
/// let epigraph = QuoteEnvironment::new([QuoteSection::Text, QuoteSection::Attributed]);
/// assert!(epigraph.enables(QuoteSection::Attributed));
/// assert_eq!(epigraph.bare, None);
///
/// // An environment whose whole content is lines, as `address` is.
/// let verse = QuoteEnvironment::new([]).bare(QuoteSection::Lines);
/// assert!(!verse.holds_blocks());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuoteEnvironment {
    /// The quotation-section commands the body accepts.
    pub sections: BTreeSet<QuoteSection>,
    /// The kind of quotation section bare content is read as; `None` makes
    /// bare content an error.
    pub bare: Option<QuoteSection>,
}

impl QuoteEnvironment {
    /// Returns an environment that accepts the quotation-section commands of
    /// `sections` (a kind given twice is accepted once) and whose bare content
    /// is an error. [`bare`](QuoteEnvironment::bare()) sets the kind bare
    /// content is read as.
    pub fn new(sections: impl IntoIterator<Item = QuoteSection>) -> QuoteEnvironment {
        QuoteEnvironment { sections: sections.into_iter().collect(), bare: None }
    }

    /// Builder: sets the kind of quotation section bare content is read as to
    /// `kind`, and returns the environment.
    pub fn bare(mut self, kind: QuoteSection) -> QuoteEnvironment {
        self.bare = Some(kind);
        self
    }

    /// Returns whether the body accepts the quotation-section command of the
    /// kind `section`.
    pub fn enables(&self, section: QuoteSection) -> bool {
        self.sections.contains(&section)
    }

    /// Returns whether the body is laid out as paragraphs and blocks: true
    /// when its bare content is read as a [`Block`](QuoteSection::Block)
    /// section or is an error. A body whose bare content is read as a passage,
    /// lines or an attribution is laid out as inline content.
    pub fn holds_blocks(&self) -> bool {
        self.bare.is_none_or(|kind| !kind.is_inline())
    }
}

/// The configuration of the [`Quote`](super::Quote) feature: the quote
/// environments, each environment's name mapped to its [`QuoteEnvironment`].
///
/// The default ([`QuoteConfig::new`]) declares three environments:
///
/// | environment | quotation-section commands | bare content read as |
/// |---|---|---|
/// | `quote` | `\text`, `\lines`, `\attributed`, `\block` | an error |
/// | `blockquote` | none | one block section |
/// | `address` | none | one lines section |
///
/// A host declares others, or changes these, through the same type:
///
/// ```
/// use flm_core::stdcore::features::quote::{QuoteConfig, QuoteEnvironment, QuoteSection};
///
/// let config = QuoteConfig::new()
///     .environment("epigraph", QuoteEnvironment::new([QuoteSection::Text, QuoteSection::Attributed]))
///     .environment("aside", QuoteEnvironment::new([]).bare(QuoteSection::Block));
/// assert_eq!(config.environments.len(), 5);
/// assert!(config.validate().is_ok());
/// ```
///
/// The value is what [`Quote::new`](super::Quote::new) takes; a host using
/// the standard package passes it with
/// [`StdcoreConfig::quote`](crate::stdcore::StdcoreConfig::quote). The
/// configuration is fixed when the registry is built, before any document is
/// parsed
/// ([configuration](crate::guide::concepts_overview#settings-options-and-configuration)),
/// because the environments and their quotation-section commands are part of
/// the language. The feature therefore feeds
/// [`digest_bytes`](QuoteConfig::digest_bytes) into the [registry
/// digest](crate::model::digest), the number that changes whenever something
/// about the registry that can change what processing produces changes
/// ([`FeatureBuilder::digest_input`](crate::core::registry::FeatureBuilder::digest_input)).
///
/// The feature checks the configuration when the registry is built
/// ([`validate`](QuoteConfig::validate)), and the registry build fails, with
/// a message that identifies the environment, on any of these: a name that
/// is empty or holds whitespace, a brace, a backslash or `%`; a name equal to
/// one of the kind tokens a presentation list reserves ([`LIST_RESERVED_KINDS`]), since an
/// environment's name becomes the kind token of the block it is staged as;
/// and an environment that accepts no quotation-section command and reads no
/// bare content. A name that another feature also declares, such as `itemize`
/// or `theorem`, makes the registry build fail as any two declarations of one
/// name do.
///
/// The configuration refers to no backend, HTML tag or stylesheet: the
/// lowering rules and stylesheets of each backend decide how a quotation is
/// presented.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct QuoteConfig {
    /// The quote environments, by name.
    pub environments: BTreeMap<String, QuoteEnvironment>,
}

impl Default for QuoteConfig {
    fn default() -> QuoteConfig {
        QuoteConfig::new()
    }
}

impl QuoteConfig {
    /// Returns the default configuration: the environments `quote`,
    /// `blockquote` and `address` (see the table above).
    // The three environments are the Python product's defaults.
    pub fn new() -> QuoteConfig {
        QuoteConfig::empty()
            .environment("quote", QuoteEnvironment::new(QuoteSection::ALL))
            .environment("blockquote", QuoteEnvironment::new([]).bare(QuoteSection::Block))
            .environment("address", QuoteEnvironment::new([]).bare(QuoteSection::Lines))
    }

    /// Returns a configuration that declares no quote environment. The
    /// feature then registers no quote environment, no quotation-section
    /// command and no lowering rule.
    pub fn empty() -> QuoteConfig {
        QuoteConfig { environments: BTreeMap::new() }
    }

    /// Builder: declares the environment `name` as `environment`, replacing
    /// any environment of that name, and returns the configuration.
    pub fn environment(mut self, name: impl Into<String>, environment: QuoteEnvironment) -> QuoteConfig {
        self.environments.insert(name.into(), environment);
        self
    }

    /// Builder: removes the environment `name`, if there is one, and returns
    /// the configuration.
    pub fn without_environment(mut self, name: &str) -> QuoteConfig {
        self.environments.remove(name);
        self
    }

    /// Returns the environment `name`, or `None` when none is declared.
    pub fn get(&self, name: &str) -> Option<&QuoteEnvironment> {
        self.environments.get(name)
    }

    /// Checks the configuration, as the feature does when it registers.
    ///
    /// # Errors
    ///
    /// Returns a sentence that states the first problem found: an environment
    /// name that is empty or holds whitespace, a brace, a backslash or the
    /// comment character `%`; a name equal to one of the kind tokens a
    /// presentation list reserves ([`LIST_RESERVED_KINDS`]); an environment
    /// that accepts no quotation-section command and reads no bare content.
    pub fn validate(&self) -> Result<(), String> {
        for (name, environment) in &self.environments {
            if name.is_empty()
                || name.chars().any(|ch| ch.is_whitespace() || matches!(ch, '{' | '}' | '\\' | '%'))
            {
                return Err(format!(
                    "the quote environment name `{name}` must not be empty or contain \
                     whitespace, a brace, a backslash or `%`"
                ));
            }
            if LIST_RESERVED_KINDS.contains(&name.as_str()) {
                let reserved = LIST_RESERVED_KINDS.iter().map(|kind| format!("`{kind}`"));
                return Err(format!(
                    "the quote environment name `{name}` is reserved: a quote environment must \
                     not be named {}",
                    english_list(reserved, "or")
                ));
            }
            if environment.sections.is_empty() && environment.bare.is_none() {
                let sections: Vec<String> = QuoteSection::ALL
                    .iter()
                    .map(|section| format!("`\\{}`", section.as_str()))
                    .collect();
                return Err(format!(
                    "the quote environment `{name}` must accept at least one of {}, or content \
                     outside them",
                    sections.join(", ")
                ));
            }
        }
        Ok(())
    }

    /// Returns the canonical byte form of the configuration: what the feature
    /// feeds into the registry digest, and what `Hash` hashes.
    ///
    /// The bytes are: the number of environments, then each environment in
    /// the order of the names: its name preceded by its length, the number of
    /// its quotation-section commands and the commands in the order of
    /// [`QuoteSection`], and its bare-content kind (0 for none, otherwise one
    /// more than the kind's position in that order, counted from zero). Two
    /// configurations therefore have the same byte form exactly when they are equal.
    pub fn digest_bytes(&self) -> Vec<u8> {
        // The fields are taken apart by name, so that a field added to a type
        // and not written here fails to compile.
        let QuoteConfig { environments } = self;
        let mut out = Vec::new();
        push_len(&mut out, environments.len());
        for (name, environment) in environments {
            let QuoteEnvironment { sections, bare } = environment;
            push_len(&mut out, name.len());
            out.extend_from_slice(name.as_bytes());
            push_len(&mut out, sections.len());
            out.extend(sections.iter().map(|section| section_byte(*section)));
            out.push(bare.map_or(0, |kind| section_byte(kind) + 1));
        }
        out
    }
}

/// Joins `items` as an English list, with no serial comma: `a`, `a or b`,
/// `a, b or c` (with `conjunction` `"or"`).
pub(super) fn english_list(items: impl IntoIterator<Item = String>, conjunction: &str) -> String {
    let mut items: Vec<String> = items.into_iter().collect();
    let last = items.pop().unwrap_or_default();
    if items.is_empty() { last } else { format!("{} {conjunction} {last}", items.join(", ")) }
}

/// A section kind as one byte: its position in [`QuoteSection::ALL`].
fn section_byte(section: QuoteSection) -> u8 {
    match section {
        QuoteSection::Text => 0,
        QuoteSection::Lines => 1,
        QuoteSection::Attributed => 2,
        QuoteSection::Block => 3,
    }
}

/// Writes a count as eight little-endian bytes.
fn push_len(out: &mut Vec<u8>, len: usize) {
    out.extend_from_slice(&(len as u64).to_le_bytes());
}

impl Hash for QuoteConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shipped_environments_are_the_three_of_the_python_product() {
        let config = QuoteConfig::new();
        let names: Vec<&str> = config.environments.keys().map(String::as_str).collect();
        assert_eq!(names, ["address", "blockquote", "quote"]);
        let quote = config.get("quote").expect("shipped");
        assert_eq!(quote.sections, BTreeSet::from(QuoteSection::ALL));
        assert_eq!(quote.bare, None);
        assert!(quote.holds_blocks());
        let blockquote = config.get("blockquote").expect("shipped");
        assert!(blockquote.sections.is_empty());
        assert_eq!(blockquote.bare, Some(QuoteSection::Block));
        assert!(blockquote.holds_blocks());
        let address = config.get("address").expect("shipped");
        assert!(address.sections.is_empty());
        assert_eq!(address.bare, Some(QuoteSection::Lines));
        assert!(!address.holds_blocks());
        assert_eq!(config.validate(), Ok(()));
    }

    #[test]
    fn a_section_kind_is_named_and_parsed_as_its_command() {
        for section in QuoteSection::ALL {
            assert_eq!(QuoteSection::parse(section.as_str()), Some(section));
            assert!(section.role().ends_with(section.as_str()));
        }
        assert_eq!(QuoteSection::parse("quote"), None);
    }

    #[test]
    fn the_refusals_name_the_environment() {
        let nothing = QuoteConfig::empty().environment("empty", QuoteEnvironment::new([]));
        let refused = nothing.validate().expect_err("refused");
        assert!(refused.contains("`empty`"), "{refused}");
        for reserved in LIST_RESERVED_KINDS {
            let config = QuoteConfig::empty().environment(reserved, QuoteEnvironment::new(QuoteSection::ALL));
            let refused = config.validate().expect_err("refused");
            // The name blamed stands before the last colon; the reserved list follows it.
            let blamed = refused.rsplit_once(':').map_or("", |(blamed, _)| blamed);
            assert!(blamed.contains(&format!("`{reserved}`")), "{refused}");
        }
        let spaced = QuoteConfig::empty().environment("my quote", QuoteEnvironment::new(QuoteSection::ALL));
        assert!(spaced.validate().expect_err("refused").contains("`my quote`"));
    }

    #[test]
    fn the_digest_bytes_change_with_every_field() {
        let base = QuoteConfig::new();
        let changed = [
            base.clone().environment("aside", QuoteEnvironment::new([]).bare(QuoteSection::Block)),
            base.clone().without_environment("address"),
            base.clone().environment("quote", QuoteEnvironment::new([QuoteSection::Text])),
            base.clone().environment("quote", QuoteEnvironment::new(QuoteSection::ALL).bare(QuoteSection::Text)),
            base.clone().environment("address", QuoteEnvironment::new([]).bare(QuoteSection::Text)),
        ];
        for other in &changed {
            assert_ne!(other.digest_bytes(), base.digest_bytes(), "{other:?}");
        }
        assert_eq!(QuoteConfig::new().digest_bytes(), base.digest_bytes());
    }

    #[test]
    fn the_accepted_sections_are_a_set() {
        // Named in another order, or twice, the sections are the same set:
        // the configurations are equal and digest alike.
        let listed = QuoteConfig::empty().environment(
            "quote",
            QuoteEnvironment::new([QuoteSection::Lines, QuoteSection::Text, QuoteSection::Text]),
        );
        let canonical = QuoteConfig::empty()
            .environment("quote", QuoteEnvironment::new([QuoteSection::Text, QuoteSection::Lines]));
        assert_eq!(listed, canonical);
        assert_eq!(listed.digest_bytes(), canonical.digest_bytes());
        assert_eq!(listed.validate(), Ok(()));
    }
}
