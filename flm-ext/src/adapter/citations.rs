//! `ExtCitationProvider`, `flm-core`'s citation provider over the extension
//! API's [`CitationProvider`], and the façades a call of it lends: the
//! context ([`ProviderCxFacade`]) and the entries the context makes
//! ([`CitationEntryFacade`]).
//!
//! # One call
//!
//! The retrieve stage calls [`ExtCitationProvider::answer_all`] once per
//! document with the keys no earlier provider claimed. The adapter lends the
//! handler a [`ProviderCxFacade`] on its own stack, over `flm-core`'s
//! context, and calls the handler's
//! [`answer_all`](CitationProvider::answer_all). Each entry the façade makes
//! owns the `flm-core` entry (an `Arc` of its tree), so an answer hands it
//! back as it is.
//!
//! # What a failure leaves behind
//!
//! A handler that fails answers, for every key of the call, the error
//! [`HANDLER_FAILED`]`: the citation provider ... failed: ...`, which the
//! retrieve stage reports at each key's first citation
//! (`flm.citations.provider-failed`); those keys then have no entry, and no
//! later provider is asked about them. An answer with fewer or more results
//! than requests is passed on as it is, and the retrieve stage refuses it
//! with its own message.

use std::sync::Arc;

use flm_core::lang::{FlmOrigin, FlmSource};
use flm_core::stdcore::features::citations::{
    CitationAnswer as CoreCitationAnswer, CitationEntry as CoreCitationEntry,
    CitationProvider as CoreCitationProvider, CitationProviderError, CitationRequest,
    CitationResult as CoreCitationResult, EntryPiece as CoreEntryPiece, ProviderCx as CoreProviderCx,
};
use serde_json::Value;

use crate::adapter::diag::HANDLER_FAILED;
use crate::adapter::names::Owner;
use crate::adapter::services::DeclaredServices;
use crate::api::host::HostHandle;
use crate::api::stdcore::features::citations::{
    CitationAnswer, CitationEntry, CitationProvider, CitationResult, EntryPiece, ProviderCx,
};
use crate::runtime::ExtError;
use crate::sealed::{CitationEntryCore, Sealed, Token};

/// `flm-core`'s citation provider over an extension's [`CitationProvider`]:
/// what a contribution to `flm.citations.providers` registers.
///
/// It runs on the thread of the retrieve stage (a worker thread with a large
/// stack, so that an entry can be parsed and copied); a binding runs the
/// handler on that thread's engine. A failure of the handler answers an
/// error for every key of the call (the [module documentation](self)).
pub struct ExtCitationProvider {
    /// The extension's provider.
    pub(crate) provider: Arc<dyn CitationProvider>,
    /// Its name, read once at registration.
    pub(crate) name: String,
    /// The extension and the package that contributed it.
    pub(crate) owner: Owner,
    /// The feature that contributed it.
    pub(crate) feature: String,
    /// The host services the package declared, which the context lends.
    pub(crate) services: Arc<DeclaredServices>,
}

impl ExtCitationProvider {
    /// The provider `provider`, contributed by the feature `feature` of
    /// `owner`'s package, lending the host services `services`.
    pub(crate) fn new(
        provider: Arc<dyn CitationProvider>,
        owner: &Owner,
        feature: &str,
        services: &Arc<DeclaredServices>,
    ) -> ExtCitationProvider {
        let name = provider.name();
        ExtCitationProvider {
            provider,
            name,
            owner: owner.clone(),
            feature: feature.to_string(),
            services: Arc::clone(services),
        }
    }

    /// The error every key of a failed call answers.
    fn failed(&self, error: &ExtError) -> CitationProviderError {
        let (name, feature, package, stem) = (&self.name, &self.feature, &self.owner.package, &self.owner.stem);
        CitationProviderError::new(format!(
            "{HANDLER_FAILED}: the citation provider `{name}` of the feature `{feature}` of the package `{package}` of \
             the extension `{stem}` failed: {error}"
        ))
    }
}

impl CoreCitationProvider for ExtCitationProvider {
    /// [`answer_all`](CoreCitationProvider::answer_all) for the one request;
    /// an answer that is not one result is an error for the key.
    fn answer(&self, request: &CitationRequest, cx: &CoreProviderCx<'_>) -> CoreCitationResult {
        let mut results = self.answer_all(std::slice::from_ref(request), cx);
        if results.len() == 1 {
            return results.remove(0);
        }
        let error = ExtError::Value {
            detail: format!("the answer must hold one result for the one key given, and it holds {}", results.len()),
        };
        Err(self.failed(&error))
    }

    fn answer_all(&self, requests: &[CitationRequest], cx: &CoreProviderCx<'_>) -> Vec<CoreCitationResult> {
        let facade = ProviderCxFacade { cx, provider: &self.name, services: &self.services };
        match self.provider.answer_all(requests, &facade) {
            Ok(results) => results.into_iter().map(result_to_core).collect(),
            Err(error) => {
                let failed = self.failed(&error);
                requests.iter().map(|_| Err(failed.clone())).collect()
            }
        }
    }
}

/// One result of the mirror as `flm-core`'s.
fn result_to_core(result: CitationResult) -> CoreCitationResult {
    let Some(answer) = result? else { return Ok(None) };
    let CitationAnswer { entry, identity, warnings } = answer;
    let mut core = CoreCitationAnswer::new(entry.core_entry(Token::new()));
    if let Some(identity) = identity {
        core = core.with_identity(identity);
    }
    for warning in warnings {
        core = core.with_warning(warning);
    }
    Ok(Some(core))
}

/// The extension API's [`ProviderCx`] over `flm-core`'s context of one call:
/// what [`ExtCitationProvider`] lends the handler.
pub struct ProviderCxFacade<'r, 'x> {
    cx: &'r CoreProviderCx<'x>,
    /// The provider's name: the origin of a source part of
    /// [`build_entry`](ProviderCx::build_entry).
    provider: &'r str,
    services: &'r Arc<DeclaredServices>,
}

impl Sealed for ProviderCxFacade<'_, '_> {}

/// A failure of `flm-core`'s parse or copy, as the handler sees it.
fn entry_error(error: CitationProviderError) -> ExtError {
    ExtError::Core { kind: "entry".to_string(), detail: error.detail, data: Value::Null }
}

impl ProviderCx for ProviderCxFacade<'_, '_> {
    fn parse_entry(&self, source: &str, origin: &str) -> Result<Box<dyn CitationEntry>, ExtError> {
        let source = Arc::new(FlmSource::new(source).with_origin(FlmOrigin::new(origin)));
        let entry = self.cx.parse_entry(source).map_err(entry_error)?;
        Ok(Box::new(CitationEntryFacade(entry)))
    }

    fn build_entry(&self, pieces: &[EntryPiece]) -> Result<Box<dyn CitationEntry>, ExtError> {
        let pieces: Vec<CoreEntryPiece> = pieces.iter().map(|piece| piece_to_core(piece, self.provider)).collect();
        let entry = self.cx.build_entry(&pieces).map_err(entry_error)?;
        Ok(Box::new(CitationEntryFacade(entry)))
    }

    fn host(&self, name: &str) -> Result<HostHandle, ExtError> {
        self.services.handle(name)
    }
}

/// The mirror's part as `flm-core`'s, a source part given the origin
/// `origin`.
fn piece_to_core(piece: &EntryPiece, origin: &str) -> CoreEntryPiece {
    let list = |pieces: &[EntryPiece]| pieces.iter().map(|piece| piece_to_core(piece, origin)).collect();
    match piece {
        EntryPiece::Text(text) => CoreEntryPiece::Text(text.clone()),
        EntryPiece::Source(text) => {
            CoreEntryPiece::Source(Arc::new(FlmSource::new(text.as_str()).with_origin(FlmOrigin::new(origin))))
        }
        EntryPiece::Emphasized(pieces) => CoreEntryPiece::Emphasized(list(pieces)),
        EntryPiece::Strong(pieces) => CoreEntryPiece::Strong(list(pieces)),
        EntryPiece::Link { address, content } => CoreEntryPiece::Link { address: address.clone(), content: list(content) },
    }
}

/// The extension API's [`CitationEntry`]: the `flm-core` entry, owned.
#[derive(Debug, Clone)]
pub struct CitationEntryFacade(pub(crate) CoreCitationEntry);

impl CitationEntryCore for CitationEntryFacade {
    fn core_entry(&self, _: Token) -> CoreCitationEntry {
        self.0.clone()
    }
}

impl CitationEntry for CitationEntryFacade {}
