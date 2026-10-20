//! The `meta` feature: the document's metadata (its title, its authors, its date, and
//! any other key the author writes about the document) and the three pieces that show
//! the title, the authors and the date before the document's content.
//!
//! # What an author writes, and what it produces
//!
//! The title, each author and the date are FLM text, not plain text: a title may be
//! `The \emph{Art} of $x^2$`. The author does not write them with commands in the
//! document's text; the program that processes the document sets them. The `flm`
//! program reads them from the root keys `title`, `author` and `date` of a file's
//! front matter (the block of YAML at the top of the file). Each text is parsed as
//! inline content when the document is processed, and a problem in it is reported at
//! its place in the file that holds it.
//!
//! A *piece* is a document-level synthesized value that a backend places before or
//! after the document's content ([`Piece`]; see
//! [pieces](crate::guide::concepts_overview#pieces)). The feature builds three pieces,
//! each placed before the content, and placed by every render unless the render
//! options leave it out
//! ([`PieceDecl::default_on`](crate::core::piece::PieceDecl::default_on)): [`TITLE`]
//! (`flm.meta.title`), [`AUTHOR`] (`flm.meta.author`) and [`DATE`] (`flm.meta.date`).
//! On each output:
//!
//! - on the plain-text and HTML outputs, each piece is a block of its own whose role is
//!   its element's name and whose kind token is `meta` ([`KIND_META`]; a `<div>` on
//!   HTML), holding one paragraph; the authors are written one after the other,
//!   separated by `, `, each in a span of its own ([`rules`]). The HTML page also uses the words of the title
//!   piece as its `<title>`;
//! - on the recomposed LaTeX, each piece is its content written back as LaTeX, the
//!   authors joined by `\and`: what a LaTeX document passes to `\title`, `\author` and
//!   `\date` ([`latex`]). That backend places a piece only where the render options of
//!   the `pieces` feature list it ([pieces in the recomposed
//!   LaTeX](crate::guide::rendering#pieces-in-the-recomposed-latex)).
//!
//! A document without metadata places nothing: each piece holds no content. The other
//! keys of the metadata ([`DocMeta::other`]) are data the library does not show; the
//! program may pass them to the tool it builds the final output with, such as a
//! template.
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
//! and `date` one text each, `author` one text or a list of texts. Each text is kept as
//! the author wrote it, with the place it was written at ([`MetaText`], [`MetaPlace`]). See
//! [the document's metadata](crate::guide::embedding#the-documents-metadata) in the
//! embedding guide.
//!
//! # What the feature registers
//!
//! - the option type [`DocMeta`], bound per document;
//! - the stage [`BUILD`] (`flm.meta.build`, [`BuildMeta`]), which parses the title,
//!   the authors and the date and writes the tables of the three pieces. It provides
//!   the capability [`READ`] (`flm.meta.read`), a member of the collection
//!   [`flm.validated`](crate::stdcore::VALIDATED) that every standard workflow
//!   demands, so that checking a document reports the problems the parse of its
//!   metadata finds. A problem found only when a piece is lowered for an output, such
//!   as a footnote in the title, is reported by a render only;
//! - the three pieces [`TITLE`], [`AUTHOR`] and [`DATE`], each a table and a piece
//!   declaration contributed to the service slot of the `pieces` feature
//!   ([`pieces::SLOT`]);
//! - the synthesized elements of the three pieces and [`AUTHOR_NAME`]
//!   (`flm.meta.author.name`), one author among several; no document can write them.
//!   Their default lowering rules are in [`rules`], and their rules on the recomposed
//!   LaTeX in [`latex`];
//! - the diagnostic `flm.meta.failed` ([`MetaFailed`]), for a part of the metadata that
//!   could not be built.
//!
//! # Interactions with other features
//!
//! - `pieces` ([`pieces`]): the feature contributes its three piece declarations to
//!   that feature's service slot as peer contributions (values for a service slot
//!   another feature declares; see [services and service
//!   slots](crate::guide::concepts_overview#services-and-service-slots)); without the
//!   `pieces` feature no backend places the metadata.
//! - The LaTeX family ([`latex`](crate::stdcore::backends::latex)): the rules on the
//!   recomposed LaTeX are registered with peer declarations of the backend and of the
//!   macro construct, so that a registry without the LaTeX features still builds, with
//!   those rules set aside (excluded from rule selection; see
//!   [`PeerName`](crate::core::registry::PeerName)).
//! - `check` ([`CheckWorkflow`](crate::stdcore::CheckWorkflow)): the capability
//!   [`READ`] joins the collection `flm.validated`.
//!
//! # Items
//!
//! - The feature: [`Meta`], [`feature`], [`FEATURE`].
//! - The metadata: [`DocMeta`], [`MetaText`], [`MetaPlace`]; written as data:
//!   [`data`], [`DocMeta::from_data`], [`metadata_schema`], [`TITLE_KEY`],
//!   [`AUTHOR_KEY`], [`DATE_KEY`], [`MetaPath`], [`MetaDataError`].
//! - The pieces and their elements: [`TITLE`], [`AUTHOR`], [`AUTHOR_NAME`], [`DATE`],
//!   [`KIND_META`].
//! - The stage: [`BUILD`], [`READ`], [`BuildMeta`], [`MetaFailed`].
//! - The lowering rules: [`rules`] ([`MetaTextRule`], [`AuthorRule`]) and [`latex`].
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

use crate::core::piece::{Piece, PieceDecl, PiecePlace};
use crate::core::registry::{
    BuildError, CapabilityDef, DefContent, FeatureBuilder, FeatureInfo, FlmFeature, StyleDef,
};
use crate::model::names::Name;
use crate::model::options::Bind;
use crate::stdcore::backends::pieces;
use crate::stdcore::VALIDATED;

pub use build::{BuildMeta, MetaFailed};
pub use data::{AUTHOR_KEY, DATE_KEY, MetaDataError, MetaPath, TITLE_KEY, metadata_schema};
pub use rules::{AuthorRule, MetaTextRule};

/// The feature's registered name, `meta`.
pub const FEATURE: &str = "meta";

/// The name of the title piece, `flm.meta.title`. It is also the name of the
/// piece's table and of the synthesized element at the root of the piece's
/// content.
pub const TITLE: &str = "flm.meta.title";

/// The name of the authors piece, `flm.meta.author`. It is also the name of the
/// piece's table and of the synthesized element at the root of the piece's
/// content, which holds one [`AUTHOR_NAME`] node per author.
pub const AUTHOR: &str = "flm.meta.author";

/// The name of the synthesized element of one author in the authors piece,
/// `flm.meta.author.name`: it holds the author's name as inline content.
pub const AUTHOR_NAME: &str = "flm.meta.author.name";

/// The name of the date piece, `flm.meta.date`. It is also the name of the
/// piece's table and of the synthesized element at the root of the piece's
/// content.
pub const DATE: &str = "flm.meta.date";

/// The name of the stage that parses the metadata and builds the three pieces,
/// `flm.meta.build` ([`BuildMeta`]).
pub const BUILD: &str = "flm.meta.build";

/// The name of the capability `flm.meta.read`: the document's metadata has been
/// parsed, and every problem the parse found has been reported. It is a member
/// of the collection [`flm.validated`](crate::stdcore::VALIDATED), so every
/// standard workflow runs the stage [`BUILD`].
pub const READ: &str = "flm.meta.read";

/// The kind token that the blocks staged by the rules of the three pieces carry,
/// `meta`: a stylesheet or a program's lowering rule can select on it (see
/// [`pres::payload`](crate::stdcore::backends::pres::payload)).
pub const KIND_META: &str = "meta";

/// The feature's style identifier, `flm.meta.style`, which the general rules
/// ([`MetaTextRule`], [`AuthorRule`]) require at every piece they stage. On
/// the `html` backend it names the feature's stylesheet ([`stylesheet`]);
/// on the `latex-rendered` backend
/// the feature's definition piece ([`latex_rendered::piece`]) delivers it.
pub const STYLE: &str = "flm.meta.style";

/// The style definition [`STYLE`] on the `html` backend: one cascade layer that
/// sets the title large and bold, and the authors and the date each on a line of
/// its own, indented under the title, the sizes and the indentation custom
/// properties it declares (`--flm-meta-title-size`, `--flm-meta-line-indent`, …).
/// It selects on the blocks the general rules stage (their role and the kind
/// token [`KIND_META`]), so it applies whatever compose template places the
/// pieces. It sets no rule between the pieces and the content: that belongs to
/// the page that places them.
pub fn stylesheet() -> StyleDef {
    StyleDef {
        id: Name::from(STYLE),
        content: vec![DefContent::Inline(String::from(include_str!("css/meta.css")))],
        docs: String::from(
            "the document's title large and bold, and the authors and the date each on a line of its own, \
             indented under the title",
        ),
    }
}

/// A text of the metadata as the author wrote it, in FLM, and where it was
/// written. The stage [`BUILD`] parses it as inline content.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MetaText {
    /// The text: `The \emph{Art} of $x^2$`.
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
    /// use flm_core::stdcore::features::meta::{MetaPlace, MetaText};
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

/// The document's metadata: the option type the `meta` feature declares, bound
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
/// use flm_core::stdcore::features::meta::{DocMeta, MetaText};
///
/// let meta = DocMeta::default()
///     .title(MetaText::new(r"On \emph{flexible} markup"))
///     .author(MetaText::new("Ada"))
///     .author(MetaText::new("Grace"));
/// let doc = Doc::with_options(DocContent::new(), OptionValues::new().with(meta));
/// # let _ = doc;
/// ```
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DocMeta {
    /// The title.
    pub title: Option<MetaText>,
    /// The authors, in the order written, each its own text.
    pub authors: Vec<MetaText>,
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
    pub fn author(mut self, author: MetaText) -> DocMeta {
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
        self.other.len().hash(state);
        for (key, value) in &self.other {
            key.hash(state);
            value.to_string().hash(state);
        }
    }
}

/// The `meta` feature: the option type [`DocMeta`], the stage [`BUILD`], the
/// three pieces and their elements and lowering rules (see the [module
/// documentation](self)).
///
/// Use [`feature()`] to obtain it as a value a package entry can list.
#[derive(Debug, Default, Clone, Copy)]
pub struct Meta;

impl FlmFeature for Meta {
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
            "the document's metadata: its title, its authors and its date, each an FLM text \
             with the place it was written at, and every other key as data",
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
        // See decision rationale at [§flm-dd-dr:std-meta-stage].
        b.stage(Arc::new(BuildMeta))?;
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

/// Returns the [`Meta`] feature, ready to be listed in a package entry.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::meta;
///
/// assert_eq!(meta::feature().info().name.as_str(), meta::FEATURE);
/// ```
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Meta)
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
        assert_ne!(digest(&base), digest(&base.clone().author(MetaText::new("B"))));
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
