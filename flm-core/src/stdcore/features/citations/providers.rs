//! The citation providers: the services through which a host gives the
//! library the citation entries of the cited works.
//!
//! A citation provider ([`CitationProvider`]) is a service (a value one
//! feature, or the embedding program, supplies for another feature to use;
//! see [services](crate::core::services)) that the host contributes to the
//! service slot [`PROVIDERS_SLOT`]. The retrieve stage
//! ([`RETRIEVE`](super::RETRIEVE)) hands it the cited keys
//! ([`CitationRequest`]) with a context ([`ProviderCx`]), and it returns for
//! each citation key a [`CitationResult`]: an answer ([`CitationAnswer`],
//! holding the [`CitationEntry`] and the work's identity), "not mine", or a
//! [`CitationProviderError`]. A citation provider builds the citation entry
//! from FLM source ([`ProviderCx::parse_entry`]) or from parts
//! ([`EntryPiece`], [`ProviderCx::build_entry`]).

use std::fmt;
use std::sync::Arc;

use techy::core::node::{NodeRef, NodeTree};
use techy::error::Severity;

use crate::lang::copy::{copy_pieces_into_value, CommandPiece, ValueContent, ValuePiece};
use crate::lang::{parse_fragment, BuiltLang, Flm, FlmOrigin, FlmSource, ParseOptions};
use crate::model::doc::{Fragment, FragmentLayout};
use crate::stdcore::features::href::HrefCommand;
use crate::stdcore::features::textstyles::TextStyle;

/// The name of the service slot of the citation providers,
/// `flm.citations.providers` (declared with
/// [`service_slot`](crate::core::registry::FeatureBuilder::service_slot)). It
/// takes any number of [`CitationProvider`] values, asked in the order of
/// precedence
/// ([`Services::services_by_precedence_named`](crate::core::services::Services::services_by_precedence_named)):
/// the providers of the feature placed last in the feature order first, each
/// feature's in the order it contributed them, and the host's last. The
/// feature declares it and never requires a
/// contribution: a registry with no citation provider builds, every cited key
/// of its documents then has no citation entry, and the retrieve stage
/// reports [`NoProvider`](super::NoProvider) once per document that cites a
/// work.
///
/// A host contributes a citation provider with
/// [`RegistryBuilder::contribute_to`](crate::core::registry::RegistryBuilder::contribute_to)
/// (see the example on [`CitationProvider`]), which places it after every
/// feature's contribution, or from a feature with
/// [`FeatureBuilder::contribute_to`](crate::core::registry::FeatureBuilder::contribute_to)
/// or, where the `citations` feature may be absent from the registry,
/// [`FeatureBuilder::peer_slot`](crate::core::registry::FeatureBuilder::peer_slot).
pub const PROVIDERS_SLOT: &str = "flm.citations.providers";

/// What the retrieve stage asks a citation provider about one cited key.
///
/// New fields may be added: a citation provider reads the fields it needs,
/// and only the retrieve stage creates a request.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct CitationRequest {
    /// The citation key, as the author wrote it between the braces of
    /// `\cite`, recomposed from the parsed nodes, with every run of whitespace
    /// made one space and the ends trimmed: `knuth84`, `arxiv:1234.5678`. The
    /// library reads no meaning into it; the citation provider decides what a
    /// prefix such as `arxiv:` means.
    pub key: String,
}

impl CitationRequest {
    /// Creates the request for the citation key `key`.
    pub(super) fn new(key: impl Into<String>) -> CitationRequest {
        CitationRequest { key: key.into() }
    }
}

/// The citation entry of one cited work: what the list of cited works shows
/// for it, and what a citation mark shows for it under the in-place placement
/// ([`CitationPlacement::InPlace`](super::CitationPlacement::InPlace)).
///
/// Its content is a [synthesized
/// value](crate::guide::concepts_overview#synthesized-values) (a small tree
/// held outside the document's trees) of inline content in the document's own
/// vocabulary: text, `\emph`, a link, a formula. It must state no fact about
/// the document: a citation entry that declares a label is refused where it
/// is shown, as any synthesized value stating such a fact is (see [value
/// catch-up](crate::guide::concepts_overview#value-catch-up)). The retrieve
/// stage reads each citation entry as the document would show it, with user
/// definitions expanded, and refuses one that holds a citation (`\cite`,
/// `\R`) or block content (a paragraph break, a list, a heading); the citation
/// key then has no citation entry.
///
/// New fields may be added; build one with [`ProviderCx::parse_entry`],
/// [`ProviderCx::build_entry`] or [`CitationEntry::new`].
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CitationEntry {
    /// The citation entry's content: a synthesized value's tree, as
    /// [`copy_run_into_value`](crate::lang::copy::copy_run_into_value) makes
    /// one, whose root's children are the content.
    pub content: Arc<NodeTree<Flm>>,
}

impl CitationEntry {
    /// Creates the citation entry that shows `content`, a synthesized value's
    /// tree whose root's children are inline content. A citation provider
    /// usually calls [`ProviderCx::parse_entry`] or [`ProviderCx::build_entry`]
    /// instead, which make such a tree.
    pub fn new(content: Arc<NodeTree<Flm>>) -> CitationEntry {
        CitationEntry { content }
    }
}

/// A citation provider's answer for a citation key it knows: the work's
/// citation entry, the work's identity, and warnings for the author.
///
/// The **identity** of a cited work is one spelling of its citation key that
/// every citation key referring to the same work shares: `arxiv:1211.1037`
/// for both `arXiv:1211.1037` and `arxiv:1211.1037`. Citation keys answered
/// with the same identity are one cited work: one number, one identifier, one
/// entry in the list of cited works. Identities are compared as characters,
/// across the answers of every citation provider, so a citation provider
/// writes a work's identity in the spelling that any citation provider
/// answering for the same work would use. Only a citation provider knows which
/// citation keys refer to one work: the library reads no meaning into a
/// citation key.
///
/// An answer that gives no identity has the citation key as written as its
/// identity: the work is then a work of its own, unless another citation
/// key's answer gives that citation key as its identity.
///
/// An answer may also carry **warnings**: what the citation provider has to
/// tell the author about the citation key, for example that two citation keys
/// cited as two works refer to the same work. Each warning is reported as a
/// Warning ([`ProviderWarning`](super::ProviderWarning),
/// `flm.citations.provider-warning`) at the citation key's first citation,
/// with the citation provider's text as its message; the citation entry is
/// shown and the numbering is unchanged.
///
/// New fields may be added; build one with [`CitationAnswer::new`] and the
/// builders [`with_identity`](CitationAnswer::with_identity) and
/// [`with_warning`](CitationAnswer::with_warning).
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct CitationAnswer {
    /// The work's citation entry.
    pub entry: CitationEntry,
    /// The work's identity, or `None` to use the citation key as written.
    pub identity: Option<String>,
    /// What the citation provider tells the author about the citation key,
    /// each a sentence in the citation provider's own words, reported as a
    /// Warning at the citation key's first citation.
    pub warnings: Vec<String>,
}

impl CitationAnswer {
    /// Creates the answer that shows `entry`, with no identity (the citation
    /// key as written is the work's identity) and no warning.
    pub fn new(entry: CitationEntry) -> CitationAnswer {
        CitationAnswer { entry, identity: None, warnings: Vec::new() }
    }

    /// Builder: sets the work's identity to `identity`. Every citation key
    /// answered with the same identity refers to the same work.
    pub fn with_identity(mut self, identity: impl Into<String>) -> CitationAnswer {
        self.identity = Some(identity.into());
        self
    }

    /// Builder: adds the warning `text`, a sentence stating what the author
    /// should know about the citation key, reported as a Warning at the
    /// citation key's first citation. Warnings are reported in the order they
    /// were added.
    pub fn with_warning(mut self, text: impl Into<String>) -> CitationAnswer {
        self.warnings.push(text.into());
        self
    }

    /// Returns the identity of the work this answer gives for `key`: its
    /// identity where it has one, else `key` itself.
    pub fn identity_for<'a>(&'a self, key: &'a str) -> &'a str {
        self.identity.as_deref().unwrap_or(key)
    }
}

/// What a citation provider returns for one citation key: `Ok(Some(answer))`
/// for a citation key it serves, `Ok(None)` ("not mine") for a citation key it
/// does not serve, so that the next citation provider is asked, or an error for
/// a citation key it claims and could not answer.
pub type CitationResult = Result<Option<CitationAnswer>, CitationProviderError>;

/// One part of the content of a citation entry that a citation provider
/// builds with [`ProviderCx::build_entry`]: characters, FLM source, or a
/// group of parts with a meaning (emphasis, strong importance, a link).
///
/// Each group is written as the command of the feature that owns its
/// meaning: `\emph` and `\textbf` (the `textstyles` feature), `\href` (the
/// `href` feature). A citation entry built from parts is therefore shown on
/// every output exactly as the same content written in FLM. A group whose
/// command the registry's language does not define shows its content alone.
///
/// New kinds of parts may be added.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum EntryPiece {
    /// Characters, shown exactly as they are, `%` and `\` included.
    Text(String),
    /// FLM source, parsed as inline content under the registry's language, as
    /// [`ProviderCx::parse_entry`] parses it: a formula in a title.
    Source(Arc<FlmSource>),
    /// Emphasized content (`\emph`).
    Emphasized(Vec<EntryPiece>),
    /// Content of strong importance, set in bold (`\textbf`).
    Strong(Vec<EntryPiece>),
    /// A link to `address` around content (`\href`). The address is passed on
    /// unchanged; the feature that owns links decides what each output does
    /// with it.
    Link {
        /// The address the link goes to: `https://doi.org/10.1103/…`.
        address: String,
        /// What the link shows.
        content: Vec<EntryPiece>,
    },
}

impl EntryPiece {
    /// Creates a part of characters: `EntryPiece::Text(text.into())`.
    pub fn text(text: impl Into<String>) -> EntryPiece {
        EntryPiece::Text(text.into())
    }
}

/// Why a citation provider could not answer for a citation key it claims, in
/// the citation provider's own words. The retrieve stage reports it in the
/// Error [`ProviderFailed`](super::ProviderFailed), and no later citation
/// provider is asked about that citation key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationProviderError {
    /// What went wrong, in the citation provider's words: `the server did not
    /// answer`.
    pub detail: String,
}

impl CitationProviderError {
    /// Creates the error that says `detail`.
    pub fn new(detail: impl Into<String>) -> CitationProviderError {
        CitationProviderError { detail: detail.into() }
    }
}

impl fmt::Display for CitationProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for CitationProviderError {}

/// The context the retrieve stage gives a citation provider together with the
/// requests: the built language of the registry, through which the citation
/// provider makes its citation entries into content of the document's
/// vocabulary ([`parse_entry`](ProviderCx::parse_entry),
/// [`build_entry`](ProviderCx::build_entry)).
#[derive(Debug, Clone, Copy)]
pub struct ProviderCx<'a> {
    lang: &'a BuiltLang,
}

impl<'a> ProviderCx<'a> {
    /// Creates the context of a request under the built language `lang`.
    pub(super) fn new(lang: &'a BuiltLang) -> ProviderCx<'a> {
        ProviderCx { lang }
    }

    /// Returns the built language of the registry the document is processed
    /// under.
    pub fn lang(&self) -> &'a BuiltLang {
        self.lang
    }

    /// Returns the citation entry whose content is `source`, FLM parsed as
    /// inline content under the registry's language: `Knuth, \emph{The Art of
    /// Computer Programming}`. The source's origin says where the text came
    /// from, so a diagnostic about the citation entry's content points there.
    ///
    /// The source must be inline content: a citation entry holding a paragraph
    /// break or a block (a list, a heading) is returned, and the retrieve stage
    /// then refuses it.
    ///
    /// The retrieve stage calls the citation providers on a worker thread (a
    /// thread with a large enough stack; see
    /// [`on_worker_thread`](crate::lang::guard::on_worker_thread)); a citation
    /// provider that calls this method on another thread of its own runs it on
    /// a worker thread too, as the copy into a synthesized value requires.
    ///
    /// # Errors
    ///
    /// A [`CitationProviderError`] holding the message of the first Error the
    /// parse reports (an unknown command, a brace left open), or the failure
    /// of the parse or of the copy into a synthesized value (content nesting
    /// too deeply).
    pub fn parse_entry(&self, source: Arc<FlmSource>) -> Result<CitationEntry, CitationProviderError> {
        let fragment = self.parse_inline(source)?;
        let root = fragment.tree().root();
        let nodes: Vec<ValuePiece<'_, '_>> = root.children().iter().map(ValuePiece::Node).collect();
        self.copy(&nodes, &root).map(CitationEntry::new)
    }

    /// Returns the citation entry made of `pieces`, in order: characters shown
    /// as they are, FLM source parsed as [`parse_entry`](ProviderCx::parse_entry)
    /// parses it, and groups written as the commands of the features that own
    /// their meaning ([`EntryPiece`]). A citation provider that holds a work's
    /// citation entry as structured parts (a formatter's italic and bold runs,
    /// a link) uses it to make content with no text to escape and nothing to
    /// parse back.
    ///
    /// The characters of a text part, and the groups, come from no source
    /// text: a diagnostic about them has no position.
    ///
    /// The retrieve stage calls the citation providers on a worker thread (a
    /// thread with a large enough stack; see
    /// [`on_worker_thread`](crate::lang::guard::on_worker_thread)); a citation
    /// provider that calls this method on another thread of its own runs it on
    /// a worker thread too, as the copy into a synthesized value requires.
    ///
    /// # Errors
    ///
    /// A [`CitationProviderError`] holding the message of the first Error a
    /// source part's parse reports, or the failure of a copy (content nesting
    /// too deeply, or a group's command that could not be looked up in the
    /// language).
    pub fn build_entry(&self, pieces: &[EntryPiece]) -> Result<CitationEntry, CitationProviderError> {
        let anchor = self.parse_inline(Arc::new(FlmSource::new("").with_origin(FlmOrigin::none())))?;
        self.build(pieces, &anchor.tree().root()).map(CitationEntry::new)
    }

    /// Parses `source` as inline content; refuses it with the first Error the
    /// parse reports.
    fn parse_inline(&self, source: Arc<FlmSource>) -> Result<Fragment, CitationProviderError> {
        let fragment =
            parse_fragment(self.lang, source, FragmentLayout::Inline, &ParseOptions::default())
                .map_err(|error| CitationProviderError::new(error.to_string()))?;
        if let Some(error) = fragment.diags().iter().find(|d| d.severity() == Severity::Error) {
            return Err(CitationProviderError::new(error.message()));
        }
        Ok(fragment)
    }

    /// Copies `pieces` into a synthesized value's tree at `anchor`, as the
    /// citation entry's own content.
    fn copy(&self, pieces: &[ValuePiece<'_, '_>], anchor: &NodeRef<'_, Flm>) -> Result<Arc<NodeTree<Flm>>, CitationProviderError> {
        copy_pieces_into_value(pieces, anchor, ValueContent::Own, self.lang.guard())
            .map_err(|error| CitationProviderError::new(error.to_string()))
    }

    /// Builds the synthesized value's tree of `pieces`: each group is built
    /// first into a tree of its own, whose nodes the whole then copies.
    fn build(&self, pieces: &[EntryPiece], anchor: &NodeRef<'_, Flm>) -> Result<Arc<NodeTree<Flm>>, CitationProviderError> {
        enum Part<'p> {
            Text(&'p str),
            Tree(Arc<NodeTree<Flm>>),
        }
        let mut parts = Vec::with_capacity(pieces.len());
        for piece in pieces {
            let part = match piece {
                EntryPiece::Text(text) => Part::Text(text),
                EntryPiece::Source(source) => Part::Tree(Arc::clone(self.parse_inline(Arc::clone(source))?.tree())),
                EntryPiece::Emphasized(content) => {
                    Part::Tree(self.group(TextStyle::Emphasis.command(), None, content, anchor)?)
                }
                EntryPiece::Strong(content) => Part::Tree(self.group(TextStyle::Bold.command(), None, content, anchor)?),
                EntryPiece::Link { address, content } => {
                    Part::Tree(self.group(HrefCommand::Href.name(), Some(address), content, anchor)?)
                }
            };
            parts.push(part);
        }
        let values: Vec<ValuePiece<'_, '_>> = parts
            .iter()
            .flat_map(|part| match part {
                Part::Text(text) => vec![ValuePiece::Text(text)],
                Part::Tree(tree) => tree.root().children().iter().map(ValuePiece::Node).collect(),
            })
            .collect();
        self.copy(&values, anchor)
    }

    /// Builds the tree of the command `\name` around `content`, with the
    /// characters of `first` as its first argument where there is one; builds
    /// the tree of `content` alone where the language defines no such
    /// command.
    fn group(
        &self,
        name: &str,
        first: Option<&str>,
        content: &[EntryPiece],
        anchor: &NodeRef<'_, Flm>,
    ) -> Result<Arc<NodeTree<Flm>>, CitationProviderError> {
        let inner = self.build(content, anchor)?;
        let command = CommandPiece::new(name, anchor.span().clone(), Arc::clone(anchor.parsing_state()))
            .map_err(|error| CitationProviderError::new(error.to_string()))?;
        let Some(mut command) = command else {
            return Ok(inner);
        };
        let root = inner.root();
        let nodes: Vec<ValuePiece<'_, '_>> = root.children().iter().map(ValuePiece::Node).collect();
        if let Some(first) = first {
            command.arguments.push(Some(vec![ValuePiece::Text(first)]));
        }
        command.arguments.push(Some(nodes));
        self.copy(&[ValuePiece::Command(&command)], anchor)
    }
}

/// A citation provider: a service that answers cited keys with their works'
/// citation entries and identities. A host contributes citation providers to
/// the service slot [`PROVIDERS_SLOT`].
///
/// The library performs no input or output: a citation provider that reads a
/// file or asks a server does so as a service of the host. The retrieve stage
/// hands the citation providers, in the order of the service slot, the
/// distinct citation keys of a document in order of first citation, each
/// citation provider in one call ([`answer_all`](CitationProvider::answer_all)).
/// The first citation provider receives every citation key, and each later
/// one receives the citation keys that every citation provider before it
/// answered "not mine". The citation provider that answers a citation key,
/// with an answer or with an error, decides it. A citation provider that
/// answers one citation key at a time implements
/// [`answer`](CitationProvider::answer) alone, and the default
/// [`answer_all`](CitationProvider::answer_all) calls it for each citation
/// key; a citation provider that fetches in batches implements both.
///
/// A citation provider is asked again in every run, under every workflow, the
/// `check` workflow included. A citation provider whose answers are costly to
/// obtain should therefore keep the answers it has given and return them
/// again, unchanged.
///
/// # Examples
///
/// A provider answering from a map of FLM texts:
///
/// ```
/// use std::collections::BTreeMap;
/// use std::sync::Arc;
///
/// use flm_core::core::registry::RegistryBuilder;
/// use flm_core::lang::{FlmOrigin, FlmSource};
/// use flm_core::stdcore::features::citations::{
///     CitationAnswer, CitationProvider, CitationRequest, CitationResult, ProviderCx,
///     PROVIDERS_SLOT,
/// };
///
/// struct FromMap(BTreeMap<String, String>);
///
/// impl CitationProvider for FromMap {
///     fn answer(&self, request: &CitationRequest, cx: &ProviderCx<'_>) -> CitationResult {
///         let Some(text) = self.0.get(&request.key) else { return Ok(None) };
///         let source = FlmSource::new(text.as_str()).with_origin(FlmOrigin::new("refs.flm"));
///         cx.parse_entry(Arc::new(source)).map(|entry| Some(CitationAnswer::new(entry)))
///     }
/// }
///
/// let entries = BTreeMap::from([("knuth84".to_string(), "Knuth, 1984".to_string())]);
/// let mut builder = RegistryBuilder::new();
/// builder.contribute_to(PROVIDERS_SLOT, Arc::new(FromMap(entries)) as Arc<dyn CitationProvider>);
/// ```
pub trait CitationProvider: Send + Sync {
    /// Returns the answer for the work cited as `request.key`:
    /// `Ok(Some(answer))`, with the citation entry and what this citation
    /// provider knows of the work ([`CitationAnswer`]); `Ok(None)` for a
    /// citation key this citation provider does not serve, so that the next
    /// citation provider is asked; or an error, for a citation key this
    /// citation provider claims and could not answer, which ends the lookup of
    /// that citation key.
    ///
    /// # Errors
    ///
    /// A [`CitationProviderError`] in the citation provider's own words; the
    /// citation key then has no citation entry, and no later citation provider
    /// is asked about it.
    fn answer(&self, request: &CitationRequest, cx: &ProviderCx<'_>) -> CitationResult;

    /// Returns the answers for `requests`, one per request and in the same
    /// order, each as [`answer`](CitationProvider::answer) describes. The
    /// default calls [`answer`](CitationProvider::answer) for each request in
    /// turn; a citation provider that fetches many citation keys at once
    /// overrides it.
    ///
    /// A citation provider must return exactly one result per request. When
    /// the number differs, every citation key of the call gets an error saying
    /// so, reported as [`ProviderFailed`](super::ProviderFailed), and no later
    /// citation provider is asked about these citation keys.
    fn answer_all(&self, requests: &[CitationRequest], cx: &ProviderCx<'_>) -> Vec<CitationResult> {
        requests.iter().map(|request| self.answer(request, cx)).collect()
    }
}
