//! The guest's own type of `flm:techy/error`: [`Diagnostic`], placed at a
//! node by the node's handle.

use serde_json::{json, Value};

use super::{get, object};
use crate::adapter::values::JsonForm;
use crate::api::techy::error::Severity;
use crate::guest::techy::node::Node;
use crate::runtime::ExtError;

/// Mirrors [`Diagnostic`](crate::api::techy::error::Diagnostic): a message
/// for the reader of a document. In the guest, `at` is the node the
/// diagnostic is placed at (the host takes its span), where the api holds the
/// span itself.
#[derive(Debug, Clone, PartialEq)]
pub struct Diagnostic {
    /// The diagnostic's identifier.
    pub id: String,
    /// Its severity.
    pub severity: Severity,
    /// Its message.
    pub message: String,
    /// Its data, a map (`{}` when none was given).
    pub data: Value,
    /// The node it is placed at, if any.
    pub at: Option<Node>,
}

impl Diagnostic {
    /// Mirrors [`Diagnostic::new`](crate::api::techy::error::Diagnostic::new):
    /// a diagnostic of `severity`, with no data and no place.
    pub fn new(severity: Severity, id: impl Into<String>, message: impl Into<String>) -> Diagnostic {
        Diagnostic { id: id.into(), severity, message: message.into(), data: json!({}), at: None }
    }

    /// Mirrors [`Diagnostic::error`](crate::api::techy::error::Diagnostic::error).
    pub fn error(id: impl Into<String>, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(Severity::Error, id, message)
    }

    /// Mirrors [`Diagnostic::warning`](crate::api::techy::error::Diagnostic::warning).
    pub fn warning(id: impl Into<String>, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(Severity::Warning, id, message)
    }

    /// Mirrors [`Diagnostic::note`](crate::api::techy::error::Diagnostic::note).
    pub fn note(id: impl Into<String>, message: impl Into<String>) -> Diagnostic {
        Diagnostic::new(Severity::Note, id, message)
    }

    /// Mirrors [`Diagnostic::with_data`](crate::api::techy::error::Diagnostic::with_data).
    pub fn with_data(mut self, data: Value) -> Diagnostic {
        self.data = data;
        self
    }

    /// Mirrors [`Diagnostic::at`](crate::api::techy::error::Diagnostic::at):
    /// the diagnostic placed at `node`.
    pub fn at(mut self, node: &Node) -> Diagnostic {
        self.at = Some(*node);
        self
    }
}

/// `{"id", "severity", "message", "data", "at"}`, `at` the node's handle or
/// `null`; read by the host's `diagnostic_from_value`.
impl JsonForm for Diagnostic {
    fn to_value(&self) -> Value {
        json!({
            "id": self.id,
            "severity": self.severity.to_value(),
            "message": self.message,
            "data": self.data,
            "at": self.at.as_ref().map_or(Value::Null, JsonForm::to_value),
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a diagnostic";
        let map = object(WHAT, value)?;
        let data = match map.get("data") {
            None | Some(Value::Null) => json!({}),
            Some(data) => data.clone(),
        };
        Ok(Diagnostic {
            id: get(WHAT, map, "id")?,
            severity: get(WHAT, map, "severity")?,
            message: get(WHAT, map, "message")?,
            data,
            at: get(WHAT, map, "at")?,
        })
    }
}
