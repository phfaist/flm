//! Mirrors [`flm_core::stdcore::features::citations`]: the citation provider
//! as an extension writes one, contributed to [`PROVIDERS_SLOT`], and the
//! context the retrieve stage lends it ([`ProviderCx`]).
//!
//! An extension contributes a [`CitationProvider`] with
//! [`FeatureBuilder::contribute_to`](crate::api::core::registry::FeatureBuilder::contribute_to)
//! or [`peer_slot`](crate::api::core::registry::FeatureBuilder::peer_slot)
//! and the variant
//! [`Contributed::CitationProvider`](crate::api::core::registry::Contributed::CitationProvider);
//! the adapter wraps it in `flm-core`'s trait. The retrieve stage asks each
//! provider, in the order of the slot, about the cited keys no earlier
//! provider claimed, in one call of [`CitationProvider::answer_all`].
//!
//! Across a binding, a provider is an object with `name` and `answer(request,
//! cx)` or `answer_all(requests, cx)`; when the object has both, `answer_all`
//! is called, and when it has `answer` alone, the binding calls it once per
//! request (the default of [`CitationProvider::answer_all`], which the
//! binding computes itself: the one computation of the prelude beyond
//! forwarding). A request is `{ key }`; the answer is a list with one item
//! per request, each `null` ("not mine"), `{ error: text }`, or `{ entry,
//! identity, warnings }` (`entry` a handle of kind `citation_entry`, the two
//! others optional).

use crate::api::host::HostHandle;
use crate::runtime::ExtError;
use crate::sealed::{CitationEntryCore, Sealed};

pub use flm_core::stdcore::features::citations::{CitationProviderError, CitationRequest};

/// Mirrors [`PROVIDERS_SLOT`](flm_core::stdcore::features::citations::PROVIDERS_SLOT):
/// the service slot of the citation providers, `flm.citations.providers`.
/// Nothing differs.
pub const PROVIDERS_SLOT: &str = flm_core::stdcore::features::citations::PROVIDERS_SLOT;

/// Mirrors [`CitationProvider`](flm_core::stdcore::features::citations::CitationProvider):
/// a source of bibliography entries, asked about the cited keys of a
/// document.
///
/// What differs: [`name`](CitationProvider::name), which `flm-core`'s trait
/// does not have; the context is the façade [`ProviderCx`]; an answer's
/// entry is a [`CitationEntry`] the context made; a failure of the handler
/// itself is an [`ExtError`], beside the [`CitationProviderError`] a result
/// carries for one key.
///
/// The adapter runs a provider on the thread of the retrieve stage, a
/// worker thread with a large stack. A provider whose call fails (an
/// [`ExtError`]) answers, for every key of the call, the error
/// `flm.ext.handler-failed: the citation provider ... failed: ...`, which the
/// retrieve stage reports at each key's first citation; those keys then have
/// no entry, and no later provider is asked about them. An answer with fewer
/// or more results than requests is refused by the retrieve stage, which
/// reports it for every key of the call.
pub trait CitationProvider: Send + Sync {
    /// No `flm-core` counterpart: the provider's name, which must begin with
    /// the extension's stem and a dot. The adapter names the provider by it
    /// in messages, and a binding finds the object again by it on another
    /// thread's engine.
    fn name(&self) -> String;

    /// Mirrors [`CitationProvider::answer`](flm_core::stdcore::features::citations::CitationProvider::answer):
    /// the answer for the work cited as `request.key`: `Ok(Some(answer))`,
    /// `Ok(None)` for a key this provider does not serve, or an error for a
    /// key it claims and could not answer. What differs: the context is the
    /// façade [`ProviderCx`], and the result is wrapped in the handler's own
    /// `Result`.
    ///
    /// # Errors
    ///
    /// The handler's own failure ([`ExtError`]): the adapter then answers an
    /// error for every key of the call.
    fn answer(&self, request: &CitationRequest, cx: &dyn ProviderCx) -> Result<CitationResult, ExtError>;

    /// Mirrors [`CitationProvider::answer_all`](flm_core::stdcore::features::citations::CitationProvider::answer_all):
    /// the answers for `requests`, one per request, in the same order. The
    /// default calls [`answer`](CitationProvider::answer) for each request in
    /// turn, as `flm-core`'s does. What differs: as
    /// [`answer`](CitationProvider::answer).
    ///
    /// # Errors
    ///
    /// As [`answer`](CitationProvider::answer).
    fn answer_all(&self, requests: &[CitationRequest], cx: &dyn ProviderCx) -> Result<Vec<CitationResult>, ExtError> {
        requests.iter().map(|request| self.answer(request, cx)).collect()
    }
}

/// Mirrors [`CitationEntry`](flm_core::stdcore::features::citations::CitationEntry):
/// the entry of one cited work. What differs: an opaque value, made by
/// [`ProviderCx::parse_entry`] or [`ProviderCx::build_entry`] and handed back
/// in a [`CitationAnswer`]; it has no method. Across a binding, a handle of
/// kind `citation_entry`, valid in the call that made it.
///
/// The trait is sealed: the adapter alone implements it.
pub trait CitationEntry: CitationEntryCore {}

/// Mirrors [`CitationAnswer`](flm_core::stdcore::features::citations::CitationAnswer):
/// a provider's answer for a key it knows. What differs: `entry` is the
/// façade [`CitationEntry`]; across a binding, an answer is the plain object
/// `{ entry, identity, warnings }`, the two last optional.
pub struct CitationAnswer {
    /// The work's citation entry.
    pub entry: Box<dyn CitationEntry>,
    /// The work's identity, or `None` to use the key as written.
    pub identity: Option<String>,
    /// What the provider tells the author about the key, each reported as a
    /// Warning at the key's first citation.
    pub warnings: Vec<String>,
}

impl CitationAnswer {
    /// Mirrors [`CitationAnswer::new`](flm_core::stdcore::features::citations::CitationAnswer::new):
    /// the answer that shows `entry`, with no identity and no warning.
    /// Nothing differs.
    pub fn new(entry: Box<dyn CitationEntry>) -> CitationAnswer {
        CitationAnswer { entry, identity: None, warnings: Vec::new() }
    }

    /// Mirrors [`CitationAnswer::with_identity`](flm_core::stdcore::features::citations::CitationAnswer::with_identity):
    /// sets the work's identity to `identity`. Nothing differs.
    pub fn with_identity(mut self, identity: impl Into<String>) -> CitationAnswer {
        self.identity = Some(identity.into());
        self
    }

    /// Mirrors [`CitationAnswer::with_warning`](flm_core::stdcore::features::citations::CitationAnswer::with_warning):
    /// adds the warning `text`. Nothing differs.
    pub fn with_warning(mut self, text: impl Into<String>) -> CitationAnswer {
        self.warnings.push(text.into());
        self
    }
}

/// Mirrors [`CitationResult`](flm_core::stdcore::features::citations::CitationResult):
/// what a provider answers for one key. What differs: over the mirror's
/// [`CitationAnswer`]. Across a binding, `null`, `{ error: text }` or the
/// answer.
pub type CitationResult = Result<Option<CitationAnswer>, CitationProviderError>;

/// Mirrors [`EntryPiece`](flm_core::stdcore::features::citations::EntryPiece):
/// one part of the content of a citation entry that
/// [`ProviderCx::build_entry`] builds. What differs: [`Source`](EntryPiece::Source)
/// holds the text of the FLM source (the adapter gives it the provider's name
/// as its origin); the enum has a serde form, which is its form across a
/// binding: `{ "text": … }`, `{ "source": … }`, `{ "emphasized": [parts] }`,
/// `{ "strong": [parts] }`, `{ "link": { "address": …, "content": [parts] } }`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum EntryPiece {
    /// Characters, shown exactly as they are.
    Text(String),
    /// FLM source, parsed as inline content under the registry's language.
    Source(String),
    /// Emphasized content (`\emph`).
    Emphasized(Vec<EntryPiece>),
    /// Content of strong importance (`\textbf`).
    Strong(Vec<EntryPiece>),
    /// A link to `address` around content (`\href`).
    Link {
        /// The address the link goes to.
        address: String,
        /// What the link shows.
        content: Vec<EntryPiece>,
    },
}

/// Mirrors [`ProviderCx`](flm_core::stdcore::features::citations::ProviderCx):
/// the context the retrieve stage lends a provider for one call.
///
/// What differs: [`parse_entry`](ProviderCx::parse_entry) takes the text and
/// the origin's label, the adapter making the source; [`host`](ProviderCx::host)
/// has no counterpart; `lang()` is left out, since the extension API has no
/// façade of the built language and a provider makes its entries through the
/// two methods below.
///
/// The trait is sealed: the adapter alone implements it.
pub trait ProviderCx: Sealed {
    /// Mirrors [`ProviderCx::parse_entry`](flm_core::stdcore::features::citations::ProviderCx::parse_entry):
    /// the citation entry whose content is `source`, FLM parsed as inline
    /// content under the registry's language. What differs: `source` is the
    /// text and `origin` the label of where it came from (a file's name),
    /// which a diagnostic about the entry's content shows.
    ///
    /// # Errors
    ///
    /// The first Error the parse reports, or a failure of the copy into a
    /// synthesized value ([`ExtError::Core`] of kind `entry`).
    fn parse_entry(&self, source: &str, origin: &str) -> Result<Box<dyn CitationEntry>, ExtError>;

    /// Mirrors [`ProviderCx::build_entry`](flm_core::stdcore::features::citations::ProviderCx::build_entry):
    /// the citation entry made of `pieces`, in order. What differs: the
    /// mirror's [`EntryPiece`], a source part parsed under the provider's
    /// name as its origin.
    ///
    /// # Errors
    ///
    /// As [`parse_entry`](ProviderCx::parse_entry).
    fn build_entry(&self, pieces: &[EntryPiece]) -> Result<Box<dyn CitationEntry>, ExtError>;

    /// No `flm-core` counterpart: the host service `name`, which the
    /// provider's package declared, as a handle valid for the call; as
    /// [`StageCx::host`](crate::api::core::run::StageCx::host).
    ///
    /// # Errors
    ///
    /// The package did not declare `name` ([`ExtError::Argument`]).
    fn host(&self, name: &str) -> Result<HostHandle, ExtError>;
}

fn _object_safe(_: &dyn CitationProvider, _: &dyn ProviderCx, _: &dyn CitationEntry) {}
