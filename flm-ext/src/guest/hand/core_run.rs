//! The guest's own type of `flm:core/run`: [`WalkEntry`], a node of a walk
//! with its address.

use serde_json::{json, Value};

use super::{get, object};
use crate::adapter::values::JsonForm;
use crate::guest::core::lowering::NodeAddr;
use crate::guest::techy::node::Node;
use crate::runtime::ExtError;

/// Mirrors [`WalkEntry`](crate::api::core::run::WalkEntry): a node of the
/// trees a stage walks, with its address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalkEntry {
    /// The node's address.
    pub addr: NodeAddr,
    /// The node.
    pub node: Node,
}

/// `{"addr": {"tree", "node"}, "node": handle}`; the host's
/// `walk_entry_to_value`.
impl JsonForm for WalkEntry {
    fn to_value(&self) -> Value {
        json!({ "addr": self.addr.to_value(), "node": self.node.to_value() })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a node of a walk";
        let map = object(WHAT, value)?;
        Ok(WalkEntry { addr: get(WHAT, map, "addr")?, node: get(WHAT, map, "node")? })
    }
}
