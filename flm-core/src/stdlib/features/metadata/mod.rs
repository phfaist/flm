//! The `metadata` feature: the document's metadata (its title, its authors, its date, and
//! any other key the author writes about the document) and the three pieces that show
//! the title, the authors and the date before the document's content.
//!
//! # What an author writes, and what it produces
//!
//! The title, the date and each author's name, affiliations and notes are FLM text,
//! not plain text: a title may be `The \emph{Art} of \(x^2\)`. An author may have,
//! beside a name, affiliations, an email address (a plain text) and notes
//! ([`MetaAuthor`]). The author of the document does not write them with commands in
//! the document's text; the program that processes the document sets them. The `flm`
//! program reads them from the root keys `title`, `author` and `date` of a file's
//! front matter (the block of YAML at the top of the file). Each FLM text is parsed as
//! inline content when the document is processed, and a problem in it is reported at
//! its place in the file that holds it.
//!
//! A *piece* is a document-level synthesized value that a backend places before or
//! after the document's content ([`Piece`]; see
//! [pieces](crate::guide::concepts_overview#pieces)). The feature builds three pieces,
//! each placed before the content, and placed by every render unless the render
//! options leave it out
//! ([`PieceDecl::default_on`](crate::core::piece::PieceDecl::default_on)): [`TITLE`]
//! (`flm.metadata.title`), [`AUTHOR`] (`flm.metadata.author`) and [`DATE`]
//! (`flm.metadata.date`). On each output:
//!
//! - on the plain-text, HTML and rendered-LaTeX outputs, each piece is a block of its
//!   own whose role is its element's name and whose kind token is `metadata`
//!   ([`KIND_METADATA`]; a `<div>` on HTML). The title and the date hold one
//!   paragraph. The authors hold each author, one after the other, as a group of
//!   lines: the name, each affiliation, the email address (a link to `mailto:` and
//!   the address) and each note, each on a line of its own in a span whose role is
//!   the part's element name ([`rules`]). The HTML page also uses the words of the
//!   title piece as its `<title>`;
//! - on the recomposed LaTeX, each piece is what a LaTeX document passes to `\title`,
//!   `\author` and `\date`: the title and the date are their content written back as
//!   LaTeX; each author is the name, `\thanks{…}` for each note, then each affiliation
//!   and the email address after `\\`, the authors joined by `\and` ([`latex`]). That
//!   backend places a piece only where the render options of the `pieces` feature
//!   list it ([pieces in the recomposed
//!   LaTeX](crate::guide::rendering#pieces-in-the-recomposed-latex)).
//!
//! A document without metadata places nothing: each piece holds no content. The other
//! keys of the metadata ([`DocMeta::other`]) and of an author ([`MetaAuthor::other`]) are
//! data the library does not show; the program may pass them to the tool it builds the
//! final output with, such as a template.
//!
//! # What a program provides
//!
//! The metadata is an option value of the option type [`DocMeta`], bound per document:
//! an option type is a plain Rust struct a feature declares once, whose values are set
//! on a document or on a render request ([`crate::model::options`]). A program sets it
//! on the document it builds
//! ([`Doc::with_options`](crate::model::doc::Doc::with_options)). A program that holds
//! the metadata as data (for example the root keys of a front matter) reads it with
//! [`DocMeta::from_data`], whose form ([`data`]) is the same for every program: `title`
//! and `date` one text each, `author` one author or a list of authors, an author being
//! a text (the name) or a map (`name`, `affiliation`, `email`, `note`). Each text is
//! kept as the author wrote it, with the place it was written at ([`MetaText`],
//! [`MetaPlace`]). See
//! [the document's metadata](crate::guide::embedding#the-documents-metadata) in the
//! embedding guide.
//!
//! # What the feature registers
//!
//! - the option type [`DocMeta`], bound per document;
//! - the stage [`BUILD`] (`flm.metadata.build`, [`BuildMetadata`]), which parses the
//!   title, the authors and the date, numbers the distinct affiliations of the document
//!   ([`AffiliationPayload`]), and writes the tables of the three pieces. It provides
//!   the capability [`READ`] (`flm.metadata.read`), a member of the collection
//!   [`flm.validated`](crate::stdlib::VALIDATED) that every standard workflow
//!   demands, so that checking a document reports the problems the parse of its
//!   metadata finds. A problem found only when a piece is lowered for an output, such
//!   as a footnote in the title, is reported by a render only;
//! - the three pieces [`TITLE`], [`AUTHOR`] and [`DATE`], each a table and a piece
//!   declaration contributed to the service slot of the `pieces` feature
//!   ([`pieces::SLOT`]);
//! - the synthesized elements of the three pieces, of one author ([`AUTHOR_ENTRY`],
//!   `flm.metadata.author.entry`) and of its parts ([`AUTHOR_NAME`],
//!   [`AUTHOR_AFFILIATION`], [`AUTHOR_EMAIL`], [`AUTHOR_NOTE`]); no document can write
//!   them. Their default lowering rules are in [`rules`], and their rules on the
//!   recomposed LaTeX in [`latex`];
//! - the diagnostic `flm.metadata.failed` ([`MetadataFailed`]), for a part of the
//!   metadata that could not be built.
//!
//! # Interactions with other features
//!
//! - `pieces` ([`pieces`]): the feature contributes its three piece declarations to
//!   that feature's service slot as peer contributions (values for a service slot
//!   another feature declares; see [services and service
//!   slots](crate::guide::concepts_overview#services-and-service-slots)); without the
//!   `pieces` feature no backend places the metadata.
//! - The LaTeX family ([`latex`](crate::stdlib::backends::latex)): the rules on the
//!   recomposed LaTeX are registered with peer declarations of the backend and of the
//!   macro construct, so that a registry without the LaTeX features still builds, with
//!   those rules set aside (excluded from rule selection; see
//!   [`PeerName`](crate::core::registry::PeerName)).
//! - `check` ([`CheckWorkflow`](crate::stdlib::CheckWorkflow)): the capability
//!   [`READ`] joins the collection `flm.validated`.
//!
//! # Items
//!
//! - The feature: [`Metadata`], [`feature`], [`FEATURE`].
//! - The metadata: [`DocMeta`], [`MetaAuthor`], [`MetaText`], [`MetaPlace`]; written as
//!   data: [`data`], [`DocMeta::from_data`], [`metadata_schema`], [`TITLE_KEY`],
//!   [`AUTHOR_KEY`], [`DATE_KEY`], [`NAME_KEY`], [`AFFILIATION_KEY`], [`EMAIL_KEY`],
//!   [`NOTE_KEY`], [`MetaPath`], [`MetaStep`], [`MetaDataError`].
//! - The pieces and their elements: [`TITLE`], [`AUTHOR`], [`AUTHOR_ENTRY`],
//!   [`AUTHOR_NAME`], [`AUTHOR_AFFILIATION`], [`AUTHOR_EMAIL`], [`AUTHOR_NOTE`], [`DATE`],
//!   [`KIND_METADATA`]; the payloads [`AffiliationPayload`] and [`EmailPayload`].
//! - The stage: [`BUILD`], [`READ`], [`BuildMetadata`], [`MetadataFailed`].
//! - The lowering rules: [`rules`] ([`MetaTextRule`], [`AuthorRule`], [`EntryRule`],
//!   [`PartRule`], [`EmailRule`]) and [`latex`].
//! - The style identifier [`STYLE`] ([`stylesheet`]), which the general rules
//!   require at every piece they stage, and the `latex-rendered` definition
//!   piece delivering it, [`latex_rendered`].

pub mod build;
pub mod data;
pub mod latex;
pub mod latex_rendered;
pub mod rules;

use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use techy::core::node::NodeRef;
use techy::serialize::SerialValue;

use crate::core::piece::{Piece, PieceDecl, PiecePlace};
use crate::core::registry::{
    BuildError, CapabilityDef, DefContent, FeatureBuilder, FeatureInfo, FlmFeature, StyleDef,
};
use crate::lang::{Flm, PresPayload};
use crate::model::names::Name;
use crate::model::options::Bind;
use crate::stdlib::backends::pieces;
use crate::stdlib::VALIDATED;

pub use build::{BuildMetadata, MetadataFailed};
pub use data::{
    AFFILIATION_KEY, AUTHOR_KEY, DATE_KEY, EMAIL_KEY, MetaDataError, MetaPath, MetaStep, NAME_KEY, NOTE_KEY,
    TITLE_KEY, metadata_schema,
};
pub use rules::{AuthorRule, EmailRule, EntryRule, MetaTextRule, PartRule};

/// The feature's registered name, `metadata`.
pub const FEATURE: &str = "metadata";

/// The name of the title piece, `flm.metadata.title`. It is also the name of the
/// piece's table and of the synthesized element at the root of the piece's
/// content.
pub const TITLE: &str = "flm.metadata.title";

/// The name of the authors piece, `flm.metadata.author`. It is also the name of the
/// piece's table and of the synthesized element at the root of the piece's
/// content, which holds one [`AUTHOR_ENTRY`] node per author.
pub const AUTHOR: &str = "flm.metadata.author";

/// The name of the synthesized element of one author in the authors piece,
/// `flm.metadata.author.entry`. It holds the author's parts, in this order: one
/// [`AUTHOR_NAME`] node, one [`AUTHOR_AFFILIATION`] node per affiliation, one
/// [`AUTHOR_EMAIL`] node when the author has an email address, and one
/// [`AUTHOR_NOTE`] node per note.
pub const AUTHOR_ENTRY: &str = "flm.metadata.author.entry";

/// The name of the synthesized element of an author's name,
/// `flm.metadata.author.name`: it holds the name as inline content.
pub const AUTHOR_NAME: &str = "flm.metadata.author.name";

/// The name of the synthesized element of one affiliation of an author,
/// `flm.metadata.author.affiliation`: it holds the affiliation as inline content,
/// and carries the affiliation's number ([`AffiliationPayload`]).
pub const AUTHOR_AFFILIATION: &str = "flm.metadata.author.affiliation";

/// The name of the synthesized element of an author's email address,
/// `flm.metadata.author.email`: it holds no content, and carries the address
/// ([`EmailPayload`]).
pub const AUTHOR_EMAIL: &str = "flm.metadata.author.email";

/// The name of the synthesized element of one note about an author,
/// `flm.metadata.author.note`: it holds the note as inline content.
pub const AUTHOR_NOTE: &str = "flm.metadata.author.note";

/// The name of the date piece, `flm.metadata.date`. It is also the name of the
/// piece's table and of the synthesized element at the root of the piece's
/// content.
pub const DATE: &str = "flm.metadata.date";

/// The name of the stage that parses the metadata and builds the three pieces,
/// `flm.metadata.build` ([`BuildMetadata`]).
pub const BUILD: &str = "flm.metadata.build";

/// The name of the capability `flm.metadata.read`: the document's metadata has
/// been parsed, and every problem the parse found has been reported. It is a member
/// of the collection [`flm.validated`](crate::stdlib::VALIDATED), so every
/// standard workflow runs the stage [`BUILD`].
pub const READ: &str = "flm.metadata.read";

/// The kind token that the blocks staged by the rules of the three pieces carry,
/// `metadata`: a stylesheet or a program's lowering rule can select on it (see
/// [`pres::payload`](crate::stdlib::backends::pres::payload)).
pub const KIND_METADATA: &str = "metadata";

/// The feature's style identifier, `flm.metadata.style`, which the general rules
/// ([`MetaTextRule`], [`AuthorRule`]) require at every piece they stage. On
/// the `html` backend it names the feature's stylesheet ([`stylesheet`]);
/// on the `latex-rendered` backend
/// the feature's definition piece ([`latex_rendered::piece`]) delivers it.
pub const STYLE: &str = "flm.metadata.style";

/// The style definition [`STYLE`] on the `html` backend: one cascade layer that
/// sets the title large and bold, and the authors and the date indented under the
/// title, the authors one below the other, an author's affiliations, email
/// address and notes smaller than the name; the sizes, the indentation and the
/// spaces are custom properties it declares (`--flm-metadata-title-size`,
/// `--flm-metadata-line-indent`, `--flm-metadata-author-detail-size`, …). It
/// selects on the blocks and spans the general rules stage (their role and the
/// kind token [`KIND_METADATA`]), so it applies whatever compose template places
/// the pieces. It sets no rule between the pieces and the content: that belongs to
/// the page that places them.
pub fn stylesheet() -> StyleDef {
    StyleDef {
        id: Name::from(STYLE),
        content: vec![DefContent::Inline(String::from(include_str!("css/metadata.css")))],
        docs: String::from(
            "the document's title large and bold, and the authors and the date indented under the title, \
             each author's affiliations, email address and notes smaller than the name",
        ),
    }
}

/// The payload of an [`AUTHOR_AFFILIATION`] node: the affiliation's number.
///
/// The stage [`BUILD`] numbers the distinct affiliations of the document from 1,
/// in the order in which they first appear. Two affiliations whose texts are the
/// same once the whitespace at both ends is trimmed are the same affiliation,
/// and carry the same number. No rule of this feature writes the number: it is
/// there for a lowering rule that marks each author with the numbers of the
/// author's affiliations and lists the affiliations once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AffiliationPayload {
    /// The number, counted from 1.
    pub number: usize,
}

impl PresPayload for AffiliationPayload {
    /// `{number}`.
    fn project(&self) -> SerialValue {
        SerialValue::Map(vec![(String::from("number"), SerialValue::Int(self.number as i64))])
    }
}

impl AffiliationPayload {
    /// The payload of the affiliation node `node`, or `None` for a node that
    /// carries none.
    pub fn of<'n>(node: &'n NodeRef<'_, Flm>) -> Option<&'n AffiliationPayload> {
        node.ext().payload.as_deref()?.downcast_ref::<AffiliationPayload>()
    }
}

/// The payload of an [`AUTHOR_EMAIL`] node: the address, as the author wrote it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailPayload {
    /// The address: `ada@example.com`.
    pub address: String,
}

impl PresPayload for EmailPayload {
    /// `{address}`.
    fn project(&self) -> SerialValue {
        SerialValue::Map(vec![(String::from("address"), SerialValue::Str(self.address.clone()))])
    }
}

impl EmailPayload {
    /// The payload of the email node `node`, or `None` for a node that carries
    /// none.
    pub fn of<'n>(node: &'n NodeRef<'_, Flm>) -> Option<&'n EmailPayload> {
        node.ext().payload.as_deref()?.downcast_ref::<EmailPayload>()
    }
}

/// A text of the metadata as the author wrote it, and where it was written. The
/// stage [`BUILD`] parses it as inline FLM content, except an author's email
/// address ([`MetaAuthor::email`]), which is a plain text.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MetaText {
    /// The text: `The \emph{Art} of \(x^2\)`.
    pub text: String,
    /// Where the text begins, when it was read from a file. A problem in the
    /// text is then reported at its line and column in that file.
    pub place: Option<MetaPlace>,
}

impl MetaText {
    /// Returns the text `text`, with no place.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdlib::features::metadata::{MetaPlace, MetaText};
    ///
    /// let title = MetaText::new(r"The \emph{Art}").at(MetaPlace::new("paper.flm", 2, 8));
    /// assert_eq!(title.place.map(|place| place.line), Some(2));
    /// ```
    pub fn new(text: impl Into<String>) -> MetaText {
        MetaText { text: text.into(), place: None }
    }

    /// Builder: the text begins at `place`.
    pub fn at(mut self, place: MetaPlace) -> MetaText {
        self.place = Some(place);
        self
    }
}

/// Where a text of the metadata begins: the file, the line and the column, the
/// line and the column counted from 1.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MetaPlace {
    /// The name of the file, as a diagnostic's position shows it.
    pub file: String,
    /// The line, counted from 1 at the top of the file.
    pub line: u32,
    /// The column, counted from 1 at the start of the line, in bytes.
    pub column: u32,
}

impl MetaPlace {
    /// Returns the place at `line` and `column` of the file named `file`.
    pub fn new(file: impl Into<String>, line: u32, column: u32) -> MetaPlace {
        MetaPlace { file: file.into(), line, column }
    }
}

/// One author of the document: the name, the affiliations, the email address,
/// the notes, and every other key the author's entry holds. Each text keeps the
/// place it was written at, so that a problem in an affiliation is reported at
/// the affiliation.
///
/// The name, each affiliation and each note are FLM text, which the stage
/// [`BUILD`] parses as inline content. The email address is a plain text: it is
/// not parsed, and nothing checks its form.
///
/// # Examples
///
/// ```
/// use flm_core::stdlib::features::metadata::{MetaAuthor, MetaText};
///
/// let ada = MetaAuthor::new(MetaText::new("Ada Lovelace"))
///     .affiliation(MetaText::new(r"University of \emph{X}"))
///     .email(MetaText::new("ada@example.com"))
///     .note(MetaText::new("Corresponding author"));
/// assert_eq!(ada.affiliations.len(), 1);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct MetaAuthor {
    /// The author's name.
    pub name: MetaText,
    /// The author's affiliations, in the order written.
    pub affiliations: Vec<MetaText>,
    /// The author's email address, a plain text.
    pub email: Option<MetaText>,
    /// The notes about the author, in the order written.
    pub notes: Vec<MetaText>,
    /// Every other key of the author's entry, with its value as data. The
    /// library does not show these values.
    pub other: BTreeMap<String, serde_json::Value>,
}

impl MetaAuthor {
    /// Returns the author named `name`, with nothing else.
    pub fn new(name: MetaText) -> MetaAuthor {
        MetaAuthor { name, affiliations: Vec::new(), email: None, notes: Vec::new(), other: BTreeMap::new() }
    }

    /// Builder: adds an affiliation after those already given.
    pub fn affiliation(mut self, affiliation: MetaText) -> MetaAuthor {
        self.affiliations.push(affiliation);
        self
    }

    /// Builder: sets the email address.
    pub fn email(mut self, email: MetaText) -> MetaAuthor {
        self.email = Some(email);
        self
    }

    /// Builder: adds a note after those already given.
    pub fn note(mut self, note: MetaText) -> MetaAuthor {
        self.notes.push(note);
        self
    }

    /// Builder: sets the value of another key.
    pub fn other(mut self, key: impl Into<String>, value: serde_json::Value) -> MetaAuthor {
        self.other.insert(key.into(), value);
        self
    }
}

// A data value has no hash of its own; its written form stands for it, one
// written form per value.
impl Hash for MetaAuthor {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.name.hash(state);
        self.affiliations.hash(state);
        self.email.hash(state);
        self.notes.hash(state);
        hash_data(&self.other, state);
    }
}

/// Feeds `other`, a map of data values, to `state`: its length, then each key
/// and the written form of its value.
fn hash_data<H: Hasher>(other: &BTreeMap<String, serde_json::Value>, state: &mut H) {
    other.len().hash(state);
    for (key, value) in other {
        key.hash(state);
        value.to_string().hash(state);
    }
}

/// The document's metadata: the option type the `metadata` feature declares, bound
/// per document (see the [module documentation](self)). A program sets its value
/// on the document ([`Doc::with_options`](crate::model::doc::Doc::with_options)),
/// or reads it from data with [`DocMeta::from_data`].
///
/// The default holds nothing, and a document whose metadata holds nothing
/// places none of the three pieces.
///
/// # Examples
///
/// ```
/// use flm_core::model::doc::{Doc, DocContent};
/// use flm_core::model::options::OptionValues;
/// use flm_core::stdlib::features::metadata::{DocMeta, MetaAuthor, MetaText};
///
/// let meta = DocMeta::default()
///     .title(MetaText::new(r"On \emph{flexible} markup"))
///     .author(MetaAuthor::new(MetaText::new("Ada")).affiliation(MetaText::new("University of X")))
///     .author(MetaAuthor::new(MetaText::new("Grace")));
/// let doc = Doc::with_options(DocContent::new(), OptionValues::new().with(meta));
/// # let _ = doc;
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocMeta {
    /// The title.
    pub title: Option<MetaText>,
    /// The authors, in the order written.
    pub authors: Vec<MetaAuthor>,
    /// The date, as the author wrote it.
    pub date: Option<MetaText>,
    /// Every other key of the metadata, with its value as data. The library
    /// does not show these values.
    pub other: BTreeMap<String, serde_json::Value>,
}

impl DocMeta {
    /// Builder: sets the title.
    pub fn title(mut self, title: MetaText) -> DocMeta {
        self.title = Some(title);
        self
    }

    /// Builder: adds an author after those already given.
    pub fn author(mut self, author: MetaAuthor) -> DocMeta {
        self.authors.push(author);
        self
    }

    /// Builder: sets the date.
    pub fn date(mut self, date: MetaText) -> DocMeta {
        self.date = Some(date);
        self
    }

    /// Builder: sets the value of another key.
    pub fn other(mut self, key: impl Into<String>, value: serde_json::Value) -> DocMeta {
        self.other.insert(key.into(), value);
        self
    }

    /// Returns whether the metadata holds nothing.
    pub fn is_empty(&self) -> bool {
        self.title.is_none() && self.authors.is_empty() && self.date.is_none() && self.other.is_empty()
    }
}

// A data value has no hash of its own; its written form stands for it, one
// written form per value.
impl Hash for DocMeta {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.title.hash(state);
        self.authors.hash(state);
        self.date.hash(state);
        hash_data(&self.other, state);
    }
}

/// The `metadata` feature: the option type [`DocMeta`], the stage [`BUILD`], the
/// three pieces and their elements and lowering rules (see the [module
/// documentation](self)).
///
/// Use [`feature()`] to obtain it as a value a package entry can list.
#[derive(Debug, Default, Clone, Copy)]
pub struct Metadata;

impl FlmFeature for Metadata {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Document metadata").docs(
            "The document's metadata (its title, its authors, its date and any other key) as \
             an option set per document, and the three pieces that show the title, the authors \
             and the date before the content.",
        )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.options_with_docs::<DocMeta>(
            Bind::Document,
            "the document's metadata: its title, its authors (each a name, with affiliations, an \
             email address and notes where given) and its date, each text with the place it was \
             written at, and every other key as data",
        )?;
        rules::declare(b)?;
        latex::register(b)?;
        b.style_def(stylesheet())?;
        latex_rendered::register(b)?;
        for (name, docs) in [
            (TITLE, "The document's title, parsed: its content, or nothing to place."),
            (AUTHOR, "The document's authors, parsed: their content, or nothing to place."),
            (DATE, "The document's date, parsed: its content, or nothing to place."),
        ] {
            b.table_named::<Piece>(name, docs)?;
        }
        b.capability(CapabilityDef::new(READ).docs(
            "The document's metadata has been parsed, and every problem the parse found has \
             been reported. A member of `flm.validated`.",
        ))?;
        // One stage parses the three texts, read from one value at one time; it
        // joins `flm.validated` so that checking a document reports a problem in
        // its title as rendering it does.
        b.stage(Arc::new(BuildMetadata))?;
        b.collection_member(VALIDATED, READ)?;
        // The pieces feature owns the slot, and may be absent: the metadata is
        // then placed by no backend.
        for (name, docs) in [
            (TITLE, "the document's title, before the content"),
            (AUTHOR, "the document's authors, before the content"),
            (DATE, "the document's date, before the content"),
        ] {
            b.peer_slot(pieces::SLOT, Arc::new(PieceDecl::new(name, PiecePlace::BeforeContent, docs)))?;
        }
        Ok(())
    }
}

/// Returns the [`Metadata`] feature, ready to be listed in a package entry.
///
/// # Examples
///
/// ```
/// use flm_core::stdlib::features::metadata;
///
/// assert_eq!(metadata::feature().info().name.as_str(), metadata::FEATURE);
/// ```
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Metadata)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    fn digest(meta: &DocMeta) -> u64 {
        let mut hasher = DefaultHasher::new();
        meta.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn the_hash_follows_every_field_the_data_included() {
        let base = DocMeta::default().title(MetaText::new("A"));
        assert_eq!(digest(&base), digest(&base.clone()));
        assert_ne!(digest(&base), digest(&base.clone().author(MetaAuthor::new(MetaText::new("B")))));
        assert_ne!(
            digest(&base.clone().other("n", serde_json::json!(1))),
            digest(&base.clone().other("n", serde_json::json!(2)))
        );
    }

    #[test]
    fn the_default_holds_nothing() {
        assert!(DocMeta::default().is_empty());
        assert!(!DocMeta::default().date(MetaText::new("today")).is_empty());
    }
}
