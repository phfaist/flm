//! The guest's own types of `flm:stdcore/features/citations`
//! ([`CitationRequest`], [`CitationAnswer`], [`CitationResult`]), and the
//! handler trait [`CitationProvider`].

use serde_json::{json, Value};

use super::{get, object};
use crate::adapter::values::JsonForm;
use crate::api::stdcore::features::citations::CitationProviderError;
use crate::guest::stdcore::features::citations::{CitationEntry, ProviderCx};
use crate::runtime::ExtError;

/// Mirrors [`CitationProvider`](crate::api::stdcore::features::citations::CitationProvider):
/// what answers the citations of a document. A guest's provider is always
/// asked through `answer_all`, whose default runs in the guest.
pub trait CitationProvider: Send + Sync {
    /// Mirrors [`CitationProvider::name`](crate::api::stdcore::features::citations::CitationProvider::name).
    fn name(&self) -> String;

    /// Mirrors [`CitationProvider::answer`](crate::api::stdcore::features::citations::CitationProvider::answer):
    /// the answer to one request.
    ///
    /// # Errors
    ///
    /// What the provider answers.
    fn answer(&self, request: &CitationRequest, cx: &ProviderCx) -> Result<CitationResult, ExtError>;

    /// Mirrors [`CitationProvider::answer_all`](crate::api::stdcore::features::citations::CitationProvider::answer_all):
    /// the answers to every request, by default each from [`answer`](CitationProvider::answer).
    ///
    /// # Errors
    ///
    /// What the provider answers.
    fn answer_all(&self, requests: &[CitationRequest], cx: &ProviderCx) -> Result<Vec<CitationResult>, ExtError> {
        requests.iter().map(|request| self.answer(request, cx)).collect()
    }
}

/// Mirrors `flm-core`'s `CitationRequest`: a citation key a document asks
/// for. The guest's own: the api's has no constructor outside `flm-core`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CitationRequest {
    /// The key cited.
    pub key: String,
}

/// `{"key"}`; the host's `citation_requests_to_value` writes each.
impl JsonForm for CitationRequest {
    fn to_value(&self) -> Value {
        json!({ "key": self.key })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a citation request";
        Ok(CitationRequest { key: get(WHAT, object(WHAT, value)?, "key")? })
    }
}

/// Mirrors [`CitationAnswer`](crate::api::stdcore::features::citations::CitationAnswer):
/// a provider's answer for one key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationAnswer {
    /// The entry.
    pub entry: CitationEntry,
    /// The entry's identity, when two keys may name one entry.
    pub identity: Option<String>,
    /// The warnings for the reader.
    pub warnings: Vec<String>,
}

impl CitationAnswer {
    /// Mirrors [`CitationAnswer::new`](crate::api::stdcore::features::citations::CitationAnswer::new).
    pub fn new(entry: CitationEntry) -> CitationAnswer {
        CitationAnswer { entry, identity: None, warnings: Vec::new() }
    }

    /// Mirrors [`CitationAnswer::with_identity`](crate::api::stdcore::features::citations::CitationAnswer::with_identity).
    pub fn with_identity(mut self, identity: impl Into<String>) -> CitationAnswer {
        self.identity = Some(identity.into());
        self
    }

    /// Mirrors [`CitationAnswer::with_warning`](crate::api::stdcore::features::citations::CitationAnswer::with_warning).
    pub fn with_warning(mut self, text: impl Into<String>) -> CitationAnswer {
        self.warnings.push(text.into());
        self
    }
}

/// `{"entry": handle, "identity", "warnings"}`; read by the host's
/// `citation_results_from_value`.
impl JsonForm for CitationAnswer {
    fn to_value(&self) -> Value {
        json!({ "entry": self.entry.to_value(), "identity": self.identity, "warnings": self.warnings })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a citation answer";
        let map = object(WHAT, value)?;
        Ok(CitationAnswer {
            entry: get(WHAT, map, "entry")?,
            identity: get(WHAT, map, "identity")?,
            warnings: get::<Option<Vec<String>>>(WHAT, map, "warnings")?.unwrap_or_default(),
        })
    }
}

/// Mirrors [`CitationResult`](crate::api::stdcore::features::citations::CitationResult):
/// a provider's result for one request, an answer, none, or the provider's
/// failure.
pub type CitationResult = Result<Option<CitationAnswer>, CitationProviderError>;

/// `null`, `{"error"}` or `{"entry", "identity", "warnings"}`; read by the
/// host's `citation_results_from_value`.
impl JsonForm for CitationResult {
    fn to_value(&self) -> Value {
        match self {
            Ok(None) => Value::Null,
            Ok(Some(answer)) => answer.to_value(),
            Err(error) => error.to_value(),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        match value {
            Value::Null => Ok(Ok(None)),
            Value::Object(map) if map.contains_key("error") => Ok(Err(CitationProviderError::from_value(value)?)),
            _ => Ok(Ok(Some(CitationAnswer::from_value(value)?))),
        }
    }
}
