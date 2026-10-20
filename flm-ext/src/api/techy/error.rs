//! Mirrors techy's diagnostics, reached as [`flm_core::techy::error`]: what a
//! handler reports.

use flm_core::lang::origin::FlmSpan;
use serde_json::{json, Value};

use crate::api::techy::node::Node;
use crate::sealed::Token;

pub use flm_core::techy::error::Severity;

/// Mirrors [`Diagnostic`](flm_core::techy::error::Diagnostic): one problem a
/// handler reports.
///
/// What differs: the condition is data — an identifier, a message, a JSON
/// value — where `flm-core` takes a typed condition; the place is a node's span
/// ([`at`](Diagnostic::at)), or nothing, in which case the adapter places the
/// diagnostic at the use site of an expansion hook or at the tree of a stage.
/// The identifier must begin with the extension's stem and a dot.
#[derive(Debug, Clone)]
pub struct Diagnostic {
    /// The condition's identifier (`wordcount.shout.not-characters`).
    pub id: String,
    /// The severity.
    pub severity: Severity,
    /// The message a reader sees.
    pub message: String,
    /// The condition's data; an empty map by default.
    pub data: Value,
    /// Where the diagnostic stands, if anywhere: the span of the node given to
    /// [`at`](Diagnostic::at).
    pub at: Option<FlmSpan>,
}

impl Diagnostic {
    /// Mirrors [`Diagnostic::new`](flm_core::techy::error::Diagnostic::new): a
    /// diagnostic at the given severity, with empty data and no place.
    pub fn new(severity: Severity, id: impl Into<String>, message: impl Into<String>) -> Diagnostic {
        Diagnostic { id: id.into(), severity, message: message.into(), data: json!({}), at: None }
    }

    /// Mirrors [`Diagnostic::error`](flm_core::techy::error::Diagnostic::error).
    pub fn error(id: impl Into<String>, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(Severity::Error, id, message)
    }

    /// Mirrors [`Diagnostic::warning`](flm_core::techy::error::Diagnostic::warning).
    pub fn warning(id: impl Into<String>, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(Severity::Warning, id, message)
    }

    /// Mirrors [`Diagnostic::note`](flm_core::techy::error::Diagnostic::note).
    pub fn note(id: impl Into<String>, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(Severity::Note, id, message)
    }

    /// No flm-core counterpart: sets the condition's data, which `flm-core`'s
    /// typed condition carries itself.
    pub fn with_data(mut self, data: Value) -> Diagnostic {
        self.data = data;
        self
    }

    /// No flm-core counterpart: places the diagnostic at `node`'s span, which
    /// `flm-core` takes as an argument where the diagnostic is reported.
    pub fn at(mut self, node: &dyn Node<'_>) -> Diagnostic {
        self.at = Some(node.core_node_ref(Token::new()).span().clone());
        self
    }
}
