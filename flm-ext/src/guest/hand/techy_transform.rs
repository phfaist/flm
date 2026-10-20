//! The guest's own type of `flm:techy/transform`: [`Restage`], the answer
//! of a rewrite handler.

use serde_json::Value;

use super::{emit_from_value, emit_to_value};
use crate::adapter::values::JsonForm;
use crate::guest::lang::rewrites::userdefs::Staged;
use crate::runtime::ExtError;

/// Mirrors [`Restage`](crate::api::techy::transform::Restage): what a rewrite
/// handler answers for a node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Restage {
    /// The rewrite descends into the node.
    Descend,
    /// The node is replaced by these staged nodes.
    Emit(Vec<Staged>),
}

/// `"descend"` or `{"emit": [staged, …]}`; the host's `restage_from_value`
/// and `restage_to_value`.
impl JsonForm for Restage {
    fn to_value(&self) -> Value {
        match self {
            Restage::Descend => Value::String("descend".to_string()),
            Restage::Emit(staged) => emit_to_value(staged),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        Ok(emit_from_value("a rewrite handler's answer", "descend", value)?.map_or(Restage::Descend, Restage::Emit))
    }
}
