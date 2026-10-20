//! The configuration of the [`floats`](super) feature: the float types
//! ([`FloatType`]), collected in a [`FloatsConfig`], each with its numbering
//! policy ([`FloatNumbering`]), its caption name (written before its number
//! in the line under its content), its number format, the words a reference
//! writes before its number, the content handlers it accepts, the scoped
//! packages available in its body and the label types it accepts.
//!
//! A feature's configuration is the plain Rust value the feature is
//! constructed from; it is fixed when the registry is built and cannot vary
//! per document or per render ([settings, options and
//! configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//!
//! The module also holds the names by which the two shipped float types refer
//! to the `graphics` and `cells` features: [`INCLUDEGRAPHICS_PACKAGE`],
//! [`INCLUDEGRAPHICS_HANDLER`], [`CELLS_PACKAGE`] and [`CELLS_HANDLER`]. The
//! `floats` feature does not depend on those features: a name that no
//! registered feature declares has no effect.

use std::hash::{Hash, Hasher};

use crate::core::settings::named::{self, NamedEntry};
use crate::model::names::Name;
use crate::stdcore::features::numbering::{NumberTemplate, PluralWords, PrefixWords};
use crate::stdcore::features::refs::CAPITAL_VARIANT;

use super::{ANY_HANDLER, RESERVED_KINDS};

/// The name of the scoped package holding `\includegraphics`,
/// `flm.graphics.includegraphics`, which the `graphics` feature declares. The
/// shipped types [`FloatType::figure`] and [`FloatType::table`] list it among
/// their [`scoped_packages`](FloatType::scoped_packages), so that
/// `\includegraphics` can be written in their bodies.
pub const INCLUDEGRAPHICS_PACKAGE: &str = "flm.graphics.includegraphics";

/// The name of the content handler that accepts exactly one image,
/// `includegraphics`, which the `graphics` feature contributes. The shipped
/// types [`FloatType::figure`] and [`FloatType::table`] list it among their
/// [`handlers`](FloatType::handlers).
pub const INCLUDEGRAPHICS_HANDLER: &str = "includegraphics";

/// The name of the scoped package holding the `cells` environment,
/// `flm.cells.environment`, which the `cells` feature declares. The shipped
/// type [`FloatType::table`] lists it among its
/// [`scoped_packages`](FloatType::scoped_packages), so that a table of cells
/// can be written in its body.
pub const CELLS_PACKAGE: &str = "flm.cells.environment";

/// The name of the content handler that accepts exactly one table of cells,
/// `cells`, which the `cells` feature contributes. The shipped type
/// [`FloatType::table`] lists it among its [`handlers`](FloatType::handlers).
pub const CELLS_HANDLER: &str = "cells";

/// The **numbering policy** of a float type: when the floats of the type are
/// numbered.
///
/// A float is numbered in its type's counter or not at all. The policy and the
/// float's own `\label` and `\nonumber` decide which
/// ([`numbers`](FloatNumbering::numbers)); where the numbering feature is not
/// registered, a float that the policy numbers is reported
/// ([`NoNumbering`](super::NoNumbering)) and shown without a number, except on
/// the recomposed LaTeX, where LaTeX numbers it. In settings, the policy is written
/// `numbered`, `unnumbered` or `auto-numbered`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum FloatNumbering {
    /// Every float of the type is numbered, unless it carries `\nonumber`, as
    /// a displayed equation is.
    Numbered,
    /// No float of the type is numbered, regardless of its `\label` and
    /// `\nonumber`; a `\nonumber` in one changes nothing and is reported
    /// with a Note.
    Unnumbered,
    /// A float of the type is numbered when it carries a `\label` and no
    /// `\nonumber`; a `\nonumber` in a float with no `\label` changes nothing
    /// and is reported with a Note. The default.
    #[default]
    AutoNumbered,
}

impl FloatNumbering {
    /// Returns the policy's stable name, as settings write it: `numbered`,
    /// `unnumbered` or `auto-numbered`.
    pub fn as_str(self) -> &'static str {
        match self {
            FloatNumbering::Numbered => "numbered",
            FloatNumbering::Unnumbered => "unnumbered",
            FloatNumbering::AutoNumbered => "auto-numbered",
        }
    }

    /// Returns whether a float under this policy is numbered, given whether it
    /// carries a `\label` (`labeled`) and whether it carries a `\nonumber`
    /// (`nonumber`).
    pub fn numbers(self, labeled: bool, nonumber: bool) -> bool {
        match self {
            FloatNumbering::Numbered => !nonumber,
            FloatNumbering::Unnumbered => false,
            FloatNumbering::AutoNumbered => labeled && !nonumber,
        }
    }
}

/// One **float type**: one environment an author writes, such as `figure`,
/// `table`, or a `diagram` that a host configures. A float type has a
/// numbering policy, a caption name (written before its number in the line
/// under its content), a number format, the words a reference writes before
/// its number, the content handlers it accepts, the scoped packages available
/// in its body and the label types it accepts.
///
/// [`FloatType::new`] gives a type the defaults its name implies: numbered
/// when it carries a `\label` ([`FloatNumbering::AutoNumbered`]), the number
/// written in arabic digits, the caption name and the words derived from the
/// capitalized name (`diagram` gives the caption name `Diagram` and the
/// words `Diagram~`, `Diagrams~`, with the same pair as the variant
/// `capital`), no scoped package, and one accepted label type, the type's own
/// name (`\label{diagram:x}`). The
/// builder methods change each field. [`FloatType::figure`] and
/// [`FloatType::table`] are the two shipped types.
///
/// A float type is declared through the configuration ([`FloatsConfig`]) or by
/// another feature through [`contribute_type`](super::contribute_type). Its
/// name must consist of ASCII letters and must not be one of the reserved kind
/// tokens ([`validate`](FloatType::validate)).
///
/// In settings, a float type is one entry of the map `types`, under its name;
/// `handlers` must be written, and every other field takes the default that
/// [`FloatType::new`] gives for that name and those handlers.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::floats::{FloatNumbering, FloatType};
///
/// // A diagram holding anything, every one numbered.
/// let diagram = FloatType::new("diagram", ["any"]).numbering(FloatNumbering::Numbered);
/// assert_eq!(diagram.caption_name, "Diagram");
/// assert_eq!(diagram.words.base.one, "Diagram~");
/// assert_eq!(diagram.label_types, ["diagram"]);
///
/// // A figure also accepting labels written `fig:…`.
/// let figure = FloatType::figure().label_types(["figure", "fig"]);
/// assert_eq!(figure.words.base.one, "Fig.~");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(transform = float_type_schema)]
pub struct FloatType {
    /// The type's name: the environment's name, the reference kind of the
    /// float's reference point, the prefix of its anchor (`figure-1`) and one
    /// of the kind tokens of the lowered float. ASCII letters only. In
    /// settings, the name is the entry's key, not a field.
    #[schemars(skip)]
    pub name: Name,
    /// The word written before the number in the line under a numbered
    /// float's content: `Figure` in `Figure 1: A photo`. Plain text, shown as
    /// written.
    pub caption_name: String,
    /// When a float of the type is numbered: `numbered`, `unnumbered` or
    /// `auto-numbered` (numbered when it carries a `\label`; the default).
    pub numbering: FloatNumbering,
    /// How the number is written, as a number template of the numbering
    /// feature (a text in the template language, such as `#{roman}`);
    /// `#{arabic}` by default.
    pub number_format: NumberTemplate,
    /// The words a reference writes before the number (`Fig.~` in `Fig.~1`),
    /// singular and plural, with their variants (`Figure~` in `Figure~1`,
    /// written by `\ref[S]{…}`). FLM markup.
    pub words: PrefixWords,
    /// The names of the content handlers the type accepts, tried in this
    /// order: the first that accepts a float's content decides. A handler that
    /// no registered feature contributes accepts nothing. At least one name;
    /// `any` accepts any content.
    pub handlers: Vec<String>,
    /// The names of the scoped packages available in the body of a float of
    /// the type, besides the package of `\caption`, `\label` and
    /// `\nonumber`: they make the commands of the type's content available
    /// there (`\includegraphics` in a `figure`). A package that no registered
    /// feature declares makes nothing available.
    pub scoped_packages: Vec<Name>,
    /// The label types a `\label` in a float of the type may have (the part
    /// before the colon); by default the type's own name.
    pub label_types: Vec<String>,
}


/// Adjusts the JSON Schema of a float type: only `handlers` is required,
/// since an entry is read over the type [`FloatType::new`] makes from the
/// entry's name and its `handlers`.
fn float_type_schema(schema: &mut schemars::Schema) {
    crate::core::settings::named::require_only(schema, &["handlers"]);
}
impl FloatType {
    /// Returns the float type `name` that accepts the content handlers
    /// `handlers`, with the defaults its name implies (see the type's
    /// documentation).
    pub fn new(
        name: impl Into<Name>,
        handlers: impl IntoIterator<Item = impl Into<String>>,
    ) -> FloatType {
        let name = name.into();
        let caption_name = capitalized(name.as_str());
        let one = format!("{caption_name}~");
        let other = format!("{caption_name}s~");
        let words = PrefixWords::new(one.clone(), other.clone())
            .with_variant(CAPITAL_VARIANT, PluralWords::new(one, other));
        FloatType {
            label_types: vec![name.to_string()],
            name,
            caption_name,
            numbering: FloatNumbering::default(),
            number_format: NumberTemplate::formats("#{arabic}"),
            words,
            handlers: handlers.into_iter().map(Into::into).collect(),
            scoped_packages: Vec::new(),
        }
    }

    /// Returns the shipped type `figure`: numbered when it carries a `\label`,
    /// caption name `Figure`, the words `Fig.~`, `Figs.~` with the variant
    /// `capital` `Figure~`, `Figures~`, accepting exactly one image (the handler
    /// [`INCLUDEGRAPHICS_HANDLER`]), with `\includegraphics` available in its
    /// body (the package [`INCLUDEGRAPHICS_PACKAGE`]).
    pub fn figure() -> FloatType {
        FloatType::new("figure", [INCLUDEGRAPHICS_HANDLER])
            .words(
                PrefixWords::new("Fig.~", "Figs.~")
                    .with_variant(CAPITAL_VARIANT, PluralWords::new("Figure~", "Figures~")),
            )
            .scoped_packages([INCLUDEGRAPHICS_PACKAGE])
    }

    /// Returns the shipped type `table`: numbered when it carries a `\label`,
    /// caption name `Table`, the words `Tab.~`, `Tabs.~` with the variant
    /// `capital` `Table~`, `Tables~`, accepting exactly one table of cells (the handler [`CELLS_HANDLER`]) or
    /// exactly one image ([`INCLUDEGRAPHICS_HANDLER`]), with the `cells`
    /// environment and `\includegraphics` available in its body (the packages
    /// [`CELLS_PACKAGE`] and [`INCLUDEGRAPHICS_PACKAGE`]).
    pub fn table() -> FloatType {
        FloatType::new("table", [CELLS_HANDLER, INCLUDEGRAPHICS_HANDLER])
            .words(
                PrefixWords::new("Tab.~", "Tabs.~")
                    .with_variant(CAPITAL_VARIANT, PluralWords::new("Table~", "Tables~")),
            )
            .scoped_packages([CELLS_PACKAGE, INCLUDEGRAPHICS_PACKAGE])
    }

    /// Builder: sets the caption name.
    pub fn caption_name(mut self, caption_name: impl Into<String>) -> FloatType {
        self.caption_name = caption_name.into();
        self
    }

    /// Builder: sets the numbering policy.
    pub fn numbering(mut self, numbering: FloatNumbering) -> FloatType {
        self.numbering = numbering;
        self
    }

    /// Builder: sets the number format.
    pub fn number_format(mut self, number_format: NumberTemplate) -> FloatType {
        self.number_format = number_format;
        self
    }

    /// Builder: sets the words a reference writes before the number.
    pub fn words(mut self, words: PrefixWords) -> FloatType {
        self.words = words;
        self
    }

    /// Builder: sets the content handlers, in the order they are tried.
    pub fn handlers(mut self, handlers: impl IntoIterator<Item = impl Into<String>>) -> FloatType {
        self.handlers = handlers.into_iter().map(Into::into).collect();
        self
    }

    /// Builder: sets the scoped packages available in the body.
    pub fn scoped_packages(
        mut self,
        scoped_packages: impl IntoIterator<Item = impl Into<Name>>,
    ) -> FloatType {
        self.scoped_packages = scoped_packages.into_iter().map(Into::into).collect();
        self
    }

    /// Builder: sets the accepted label types.
    pub fn label_types(mut self, label_types: impl IntoIterator<Item = impl Into<String>>) -> FloatType {
        self.label_types = label_types.into_iter().map(Into::into).collect();
        self
    }

    /// Checks the type on its own, as [`contribute_type`](super::contribute_type)
    /// does before declaring anything.
    ///
    /// # Errors
    ///
    /// A sentence that describes the first problem found: a name that is empty
    /// or holds anything but ASCII letters; a name that is one of the kind
    /// tokens a float reserves ([`RESERVED_KINDS`]); an empty list of content
    /// handlers.
    pub fn validate(&self) -> Result<(), String> {
        let name = self.name.as_str();
        if name.is_empty() || !name.chars().all(|ch| ch.is_ascii_alphabetic()) {
            return Err(format!("the float type name `{name}` must consist of ASCII letters only"));
        }
        if RESERVED_KINDS.contains(&name) {
            let reserved: Vec<String> = RESERVED_KINDS.iter().map(|kind| format!("`{kind}`")).collect();
            return Err(format!(
                "a float type may not be named `{name}`: the names {} are reserved",
                reserved.join(" and ")
            ));
        }
        if self.handlers.is_empty() {
            return Err(format!(
                "the float type `{name}` must list at least one content handler (`{ANY_HANDLER}` \
                 accepts any content)"
            ));
        }
        Ok(())
    }

    /// Writes every field of the type into `out`, each text preceded by its
    /// length: the type's part of [`FloatsConfig::digest_bytes`].
    fn push_digest(&self, out: &mut Vec<u8>) {
        let FloatType {
            name,
            caption_name,
            numbering,
            number_format,
            words,
            handlers,
            scoped_packages,
            label_types,
        } = self;
        push_text(out, name.as_str());
        push_text(out, caption_name);
        push_text(out, numbering.as_str());
        match number_format {
            NumberTemplate::Formats(text) => {
                out.push(0);
                push_text(out, text);
            }
            NumberTemplate::Initial(text) => {
                out.push(1);
                push_text(out, text);
            }
        }
        push_plural(out, &words.base);
        push_len(out, words.variants.len());
        for (variant, plural) in &words.variants {
            push_text(out, variant);
            push_plural(out, plural);
        }
        push_len(out, handlers.len());
        for handler in handlers {
            push_text(out, handler);
        }
        push_len(out, scoped_packages.len());
        for package in scoped_packages {
            push_text(out, package.as_str());
        }
        push_len(out, label_types.len());
        for label_type in label_types {
            push_text(out, label_type);
        }
    }
}

/// The configuration of the [`Floats`](super::Floats) feature: the float
/// types it declares, in order.
///
/// The default ([`FloatsConfig::new`]) declares the two shipped types,
/// [`figure`](FloatType::figure) and [`table`](FloatType::table), both
/// numbered. A caller replaces a type by declaring one of the same name
/// ([`float_type`](FloatsConfig::float_type)), removes one
/// ([`without_type`](FloatsConfig::without_type)), adds its own, or starts
/// from [`FloatsConfig::empty`]:
///
/// ```
/// use flm_core::stdcore::features::floats::{FloatNumbering, FloatType, FloatsConfig};
///
/// let config = FloatsConfig::new()
///     .float_type(FloatType::figure().numbering(FloatNumbering::Numbered))
///     .float_type(FloatType::new("diagram", ["any"]));
/// assert_eq!(config.types.len(), 3);
/// assert!(config.validate().is_ok());
/// ```
///
/// A float type is document structure (a numbered series and the word under
/// its content), not a property of the content that fills it. The two shipped
/// types are therefore declared here, and they refer to the content handlers
/// they accept by name. A feature that brings a float type together with its
/// own content declares the type through
/// [`contribute_type`](super::contribute_type) instead.
///
/// The value is what [`Floats::new`](super::Floats::new) takes; the standard
/// package takes it through
/// [`StdcoreConfig::floats`](crate::stdcore::StdcoreConfig::floats). It is a
/// configuration, fixed when the registry is built: the environments are part
/// of the language, and the counters and counter styles are built from it. The
/// feature therefore adds [`digest_bytes`](FloatsConfig::digest_bytes) to the
/// registry digest. The feature checks it when it registers
/// ([`validate`](FloatsConfig::validate)).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct FloatsConfig {
    /// The float types, in order; no two have the same name. In settings, a
    /// map from each type's name to its fields.
    #[serde(deserialize_with = "crate::core::settings::named::deserialize")]
    #[schemars(
        transform = crate::core::settings::named::map_schema,
        with = "std::collections::BTreeMap<String, FloatType>"
    )]
    pub types: Vec<FloatType>,
}

impl Default for FloatsConfig {
    fn default() -> FloatsConfig {
        FloatsConfig::new()
    }
}

impl FloatsConfig {
    /// Returns the configuration with the two shipped types, `figure` then
    /// `table`.
    pub fn new() -> FloatsConfig {
        FloatsConfig { types: vec![FloatType::figure(), FloatType::table()] }
    }

    /// Returns a configuration that declares no float type.
    pub fn empty() -> FloatsConfig {
        FloatsConfig { types: Vec::new() }
    }

    /// Builder: declares `float_type`, replacing the type of that name in
    /// its place if there is one, and adding it at the end otherwise.
    pub fn float_type(mut self, float_type: FloatType) -> FloatsConfig {
        match self.types.iter_mut().find(|known| known.name == float_type.name) {
            Some(known) => *known = float_type,
            None => self.types.push(float_type),
        }
        self
    }

    /// Builder: removes the type `name`, if there is one.
    pub fn without_type(mut self, name: &str) -> FloatsConfig {
        self.types.retain(|known| known.name.as_str() != name);
        self
    }

    /// Returns the type `name`, if the configuration declares it.
    pub fn get(&self, name: &str) -> Option<&FloatType> {
        self.types.iter().find(|known| known.name.as_str() == name)
    }

    /// Checks the configuration, as the feature does when it registers.
    ///
    /// # Errors
    ///
    /// A sentence that describes the first problem found: a type that fails
    /// [`FloatType::validate`], or two types of one name.
    pub fn validate(&self) -> Result<(), String> {
        for (at, float_type) in self.types.iter().enumerate() {
            float_type.validate()?;
            if self.types[..at].iter().any(|earlier| earlier.name == float_type.name) {
                return Err(format!(
                    "the float type `{}` may not be declared twice",
                    float_type.name
                ));
            }
        }
        Ok(())
    }

    /// Returns the configuration written out as bytes: what the feature adds
    /// to the registry digest, and what `Hash` hashes.
    ///
    /// Every type is written out in order, with every one of its fields, each
    /// text preceded by its length. Two configurations give the same bytes
    /// exactly when they are equal. The code takes the fields apart by name,
    /// so a field added to a type and not written here fails to compile.
    pub fn digest_bytes(&self) -> Vec<u8> {
        let FloatsConfig { types } = self;
        let mut out = Vec::new();
        push_len(&mut out, types.len());
        for float_type in types {
            float_type.push_digest(&mut out);
        }
        out
    }
}

impl Hash for FloatsConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest_bytes());
    }
}

/// Returns `text` with its first letter capitalized.
fn capitalized(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Writes a count as eight little-endian bytes.
fn push_len(out: &mut Vec<u8>, len: usize) {
    out.extend_from_slice(&(len as u64).to_le_bytes());
}

/// Writes a text as its length and its bytes.
fn push_text(out: &mut Vec<u8>, text: &str) {
    push_len(out, text.len());
    out.extend_from_slice(text.as_bytes());
}

/// Writes a pair of words.
fn push_plural(out: &mut Vec<u8>, plural: &PluralWords) {
    push_text(out, &plural.one);
    push_text(out, &plural.other);
}

/// Reads a float type from settings: an entry of the map
/// [`FloatsConfig::types`], under the type's name. The entry is read over the
/// type that [`FloatType::new`] makes for that name and the entry's
/// `handlers`, which must be written.
impl NamedEntry for FloatType {
    fn read_entry(name: &str, data: serde_json::Value) -> Result<FloatType, String> {
        let handlers: Vec<String> = named::required(name, &data, "handlers")?;
        named::read_over(&FloatType::new(name, handlers), "name", name, data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_type_takes_its_defaults_from_its_name() {
        let diagram = FloatType::new("diagram", ["any"]);
        assert_eq!(diagram.caption_name, "Diagram");
        assert_eq!(diagram.words.base, PluralWords::new("Diagram~", "Diagrams~"));
        assert_eq!(diagram.words.variants[CAPITAL_VARIANT], PluralWords::new("Diagram~", "Diagrams~"));
        assert_eq!(diagram.numbering, FloatNumbering::AutoNumbered);
        assert_eq!(FloatType::figure().numbering, FloatNumbering::AutoNumbered);
        assert_eq!(FloatType::table().numbering, FloatNumbering::AutoNumbered);
        assert_eq!(diagram.number_format, NumberTemplate::formats("#{arabic}"));
        assert_eq!(diagram.label_types, ["diagram"]);
        assert!(diagram.scoped_packages.is_empty());
    }

    #[test]
    fn the_shipped_types_carry_the_words_handlers_and_package() {
        let figure = FloatType::figure();
        assert_eq!(figure.caption_name, "Figure");
        assert_eq!(figure.words.base, PluralWords::new("Fig.~", "Figs.~"));
        assert_eq!(figure.words.variants[CAPITAL_VARIANT], PluralWords::new("Figure~", "Figures~"));
        assert_eq!(figure.handlers, ["includegraphics"]);
        assert_eq!(figure.scoped_packages, [Name::from(INCLUDEGRAPHICS_PACKAGE)]);
        assert_eq!(
            FloatType::table().scoped_packages,
            [Name::from(CELLS_PACKAGE), Name::from(INCLUDEGRAPHICS_PACKAGE)]
        );
        let table = FloatType::table();
        assert_eq!(table.caption_name, "Table");
        assert_eq!(table.words.base, PluralWords::new("Tab.~", "Tabs.~"));
        assert_eq!(table.handlers, ["cells", "includegraphics"]);
        assert_eq!(FloatsConfig::new().validate(), Ok(()));
    }

    #[test]
    fn the_policy_decides_the_number_from_the_fields() {
        use FloatNumbering::*;
        assert!(Numbered.numbers(false, false) && !Numbered.numbers(true, true));
        assert!(!Unnumbered.numbers(true, false));
        assert!(AutoNumbered.numbers(true, false));
        assert!(!AutoNumbered.numbers(false, false) && !AutoNumbered.numbers(true, true));
    }

    #[test]
    fn a_name_an_empty_handler_list_and_a_repeated_type_are_refused() {
        let refused = |config: FloatsConfig| config.validate().expect_err("refused");
        assert!(refused(FloatsConfig::empty().float_type(FloatType::new("my-float", ["any"])))
            .contains("ASCII letters"));
        assert!(refused(FloatsConfig::empty().float_type(FloatType::new("", ["any"])))
            .contains("ASCII letters"));
        assert!(refused(FloatsConfig::empty().float_type(FloatType::new("numbered", ["any"])))
            .contains("reserved"));
        assert!(refused(FloatsConfig::empty().float_type(FloatType::new("unnumbered", ["any"])))
            .contains("may not"));
        assert!(refused(FloatsConfig::empty().float_type(FloatType::new("x", Vec::<String>::new())))
            .contains("must list at least one"));
        let twice = FloatsConfig { types: vec![FloatType::figure(), FloatType::figure()] };
        assert!(refused(twice).contains("twice"));
    }

    #[test]
    fn declaring_a_type_of_a_known_name_replaces_it_in_place() {
        let config = FloatsConfig::new().float_type(FloatType::figure().caption_name("Fig"));
        assert_eq!(config.types.len(), 2);
        assert_eq!(config.types[0].caption_name, "Fig");
        assert_eq!(FloatsConfig::new().without_type("figure").types.len(), 1);
    }

    #[test]
    fn the_digest_bytes_change_with_every_field() {
        let base = FloatsConfig::new();
        let figure = FloatType::figure;
        let changed = [
            base.clone().without_type("table"),
            base.clone().float_type(FloatType::new("diagram", ["any"])),
            base.clone().float_type(figure().caption_name("Fig")),
            base.clone().float_type(figure().numbering(FloatNumbering::Numbered)),
            base.clone().float_type(figure().numbering(FloatNumbering::Unnumbered)),
            base.clone().float_type(figure().number_format(NumberTemplate::formats("#{roman}"))),
            base.clone().float_type(figure().number_format(NumberTemplate::initial("#{arabic}"))),
            base.clone().float_type(figure().words(PrefixWords::new("Fig.~", "Figs.~"))),
            base.clone().float_type(figure().handlers(["any"])),
            base.clone().float_type(figure().scoped_packages(Vec::<Name>::new())),
            base.clone().float_type(figure().label_types(["figure", "fig"])),
        ];
        for other in &changed {
            assert_ne!(other.digest_bytes(), base.digest_bytes(), "{other:?}");
        }
        assert_eq!(FloatsConfig::new().digest_bytes(), base.digest_bytes());
    }
}
