//! `ExtCondition`, the diagnostic condition whose identifier, message and
//! data are values, for the conditions an extension defines and for
//! `flm-ext`'s own.

use std::fmt;

use flm_core::techy::error::{DiagnosticInfo, DiagnosticValue};
use serde_json::Value;

use crate::adapter::values::diagnostic_value;
use crate::runtime::ExtError;

/// The identifier of a handler's failure: an `Err` from a `register`, `run`
/// or `expand` handler, a stale handle, a bad argument, a value that cannot
/// cross.
pub const HANDLER_FAILED: &str = "flm.ext.handler-failed";

/// The identifier of a rewrite stage of an extension that could not produce a
/// tree: the output builder or techy's driver refused what the handler
/// staged, so the rewrite of that tree was abandoned.
pub const REWRITE_FAILED: &str = "flm.ext.rewrite-failed";

/// A condition defined on the other side of the boundary, or one of
/// `flm-ext`'s own: its identifier, its message and its data are values, as
/// techy's [`DiagnosticInfo`] foresees for a language binding ("an adapter
/// type … stores each one's identifier in a field").
///
/// The type's [`IDENTIFIER`](DiagnosticInfo::IDENTIFIER) is
/// `flm.ext.condition`; every value answers its own identifier through
/// [`identifier`](DiagnosticInfo::identifier).
#[derive(Debug, Clone)]
pub struct ExtCondition {
    id: String,
    message: String,
    data: DiagnosticValue,
}

impl ExtCondition {
    /// The condition `id`, with the text `message` and the data `data`.
    ///
    /// # Errors
    ///
    /// `data` cannot cross (a floating-point number, a reserved map key:
    /// [`ExtError::Value`]).
    pub fn new(id: impl Into<String>, message: impl Into<String>, data: Value) -> Result<ExtCondition, ExtError> {
        Ok(ExtCondition { id: id.into(), message: message.into(), data: diagnostic_value(&data)? })
    }

    /// The condition [`HANDLER_FAILED`] reporting `error` from the handler
    /// `handler`, named in words ("the stage `wc.count`", "the expansion hook
    /// of `wc.macro.shout`", "the registration of the feature `wc`"): its
    /// message is `<handler> failed: <the error's text>`, its data is
    /// [`ExtError::data`].
    pub fn handler_failed(error: &ExtError, handler: &str) -> ExtCondition {
        // `ExtError::data` holds what the error was given; a handler's own
        // data may hold a value that cannot cross, which is then left out.
        let data = diagnostic_value(&error.data()).unwrap_or_else(|_| {
            let mut data = error.data();
            if let Value::Object(map) = &mut data {
                map.remove("data");
            }
            diagnostic_value(&data).unwrap_or_else(|_| DiagnosticValue::empty_map())
        });
        ExtCondition { id: HANDLER_FAILED.to_string(), message: format!("{handler} failed: {error}"), data }
    }

    /// The condition [`REWRITE_FAILED`]: the rewrite stage `stage` of an
    /// extension produced no tree from the tree `tree` (its display text),
    /// for the reason `detail` (techy's text of the refusal). Its message is
    /// ``the rewrite stage `{stage}` must produce one tree from the tree
    /// `{tree}`: {detail}``; its data holds the three texts.
    pub fn rewrite_failed(stage: &str, tree: &str, detail: &str) -> ExtCondition {
        ExtCondition {
            id: REWRITE_FAILED.to_string(),
            message: format!("the rewrite stage `{stage}` must produce one tree from the tree `{tree}`: {detail}"),
            data: DiagnosticValue::Map(vec![
                ("stage".to_string(), DiagnosticValue::Str(stage.to_string())),
                ("tree".to_string(), DiagnosticValue::Str(tree.to_string())),
                ("detail".to_string(), DiagnosticValue::Str(detail.to_string())),
            ]),
        }
    }

    /// The condition's identifier.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The condition's message.
    pub fn message(&self) -> &str {
        &self.message
    }

    /// The condition's data.
    pub fn data(&self) -> &DiagnosticValue {
        &self.data
    }
}

impl DiagnosticInfo for ExtCondition {
    const IDENTIFIER: &'static str = "flm.ext.condition";

    fn identifier(&self) -> &str {
        &self.id
    }

    fn serializable_data(&self) -> DiagnosticValue {
        self.data.clone()
    }
}

impl fmt::Display for ExtCondition {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_condition_answers_its_own_identifier_message_and_data() {
        let condition = ExtCondition::new("wc.shout.empty", "the argument must not be empty", json!({ "n": 2 }))
            .expect("the data crosses");
        assert_eq!(condition.identifier(), "wc.shout.empty");
        assert_eq!(condition.to_string(), "the argument must not be empty");
        assert_eq!(
            condition.serializable_data(),
            DiagnosticValue::Map(vec![("n".to_string(), DiagnosticValue::Int(2))])
        );
    }

    #[test]
    fn a_condition_refuses_data_that_cannot_cross() {
        let error = ExtCondition::new("wc.x", "m", json!({ "ratio": 0.5 })).expect_err("a float");
        assert_eq!(error.to_string(), "a number must be an integer, not `0.5`");
        let error = ExtCondition::new("wc.x", "m", json!({ "$type": 1 })).expect_err("a reserved key");
        assert_eq!(error.to_string(), "a map key must not begin with `$`, as `$type` does");
    }

    #[test]
    fn a_failed_rewrite_names_the_stage_and_the_tree() {
        let condition = ExtCondition::rewrite_failed("sd.dash", "tree#0", "staging the restaged tree failed: x");
        assert_eq!(condition.identifier(), REWRITE_FAILED);
        assert_eq!(
            condition.to_string(),
            "the rewrite stage `sd.dash` must produce one tree from the tree `tree#0`: staging the restaged tree failed: x"
        );
    }

    #[test]
    fn a_handler_failure_carries_the_error_s_data() {
        let error = ExtError::Handler { detail: "TypeError: x".to_string(), data: json!({ "line": 3 }) };
        let condition = ExtCondition::handler_failed(&error, "the stage `wc.count`");
        assert_eq!(condition.identifier(), HANDLER_FAILED);
        assert_eq!(condition.to_string(), "the stage `wc.count` failed: TypeError: x");
        let with_float = ExtError::Handler { detail: "x".to_string(), data: json!({ "t": 1.5 }) };
        let data = ExtCondition::handler_failed(&with_float, "the stage `wc.count`").serializable_data();
        assert_eq!(
            data,
            DiagnosticValue::Map(vec![
                ("kind".to_string(), DiagnosticValue::Str("handler".to_string())),
                ("detail".to_string(), DiagnosticValue::Str("x".to_string())),
            ])
        );
    }
}
