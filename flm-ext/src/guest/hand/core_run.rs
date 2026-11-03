//! The guest's own type of `flm:core/run`: [`WalkEntry`], a node of a walk
//! with its address.

use serde_json::{json, Value};

use super::{get, object};
#[cfg(feature = "native")]
use crate::adapter::handles::CallTable;
#[cfg(feature = "native")]
use crate::adapter::typed::FromApi;
use crate::adapter::values::JsonForm;
#[cfg(feature = "native")]
use crate::api;
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

/// The api's `WalkEntry`, its node lent; the host's `walk_entry_to_value`.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, api::core::run::WalkEntry<'c>> for WalkEntry {
    fn from_api(api: api::core::run::WalkEntry<'c>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        Ok(WalkEntry { addr: FromApi::from_api(api.addr, table)?, node: FromApi::from_api(api.node, table)? })
    }
}
