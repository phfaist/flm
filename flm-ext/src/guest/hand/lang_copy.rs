//! The guest's own type of `flm:lang/copy`: a piece of the value of a
//! key-value entry, which holds a node ([`WrittenPiece`]).

use serde_json::{json, Value};

use super::{not_shape, single};
#[cfg(feature = "native")]
use crate::adapter::handles::CallTable;
#[cfg(feature = "native")]
use crate::adapter::typed::FromApi;
use crate::adapter::values::JsonForm;
#[cfg(feature = "native")]
use crate::api;
use crate::guest::techy::node::Node;
use crate::runtime::ExtError;

/// Mirrors [`WrittenPiece`](crate::api::lang::copy::WrittenPiece): one piece
/// of the value of a key-value entry, as the author wrote it in a bracket.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WrittenPiece {
    /// A node of the tree that holds the bracket, shown as the content it is.
    Node(Node),
    /// Characters of the document, as the tree holds them.
    Text(String),
}

/// `{"node": handle}` or `{"text": text}`; the host's
/// `written_piece_to_value`.
impl JsonForm for WrittenPiece {
    fn to_value(&self) -> Value {
        match self {
            WrittenPiece::Node(node) => json!({ "node": node.to_value() }),
            WrittenPiece::Text(text) => json!({ "text": text }),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a piece of the value of a key-value entry";
        match single(value, &["node", "text"]) {
            Some(("node", node)) => Ok(WrittenPiece::Node(JsonForm::from_value(node)?)),
            Some((_, text)) => Ok(WrittenPiece::Text(JsonForm::from_value(text)?)),
            None => Err(not_shape(WHAT, "`{\"node\": node}` or `{\"text\": text}`", value)),
        }
    }
}

/// The api's `WrittenPiece`, its node lent; the host's
/// `written_piece_to_value`.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, api::lang::copy::WrittenPiece<'c>> for WrittenPiece {
    fn from_api(api: api::lang::copy::WrittenPiece<'c>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        use api::lang::copy::WrittenPiece as Api;
        Ok(match api {
            Api::Node(node) => WrittenPiece::Node(FromApi::from_api(node, table)?),
            Api::Text(text) => WrittenPiece::Text(text),
        })
    }
}
