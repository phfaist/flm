//! The guest's own types of `flm:core/lowering` ([`Lowering`], [`PresTree`],
//! and [`NodeAddr`], which the host answers here first), the handler trait
//! [`Lower`], and the record-and-lend constructor `LoweringRule::new`, the
//! twin of the prelude's fragment `core-lowering.js`: it records the rule's
//! body under the rule's name and lends it, in every run of `register`.

use std::sync::Arc;

use serde_json::{json, Value};

use super::{emit_from_value, emit_to_value, get, not_shape, object};
#[cfg(feature = "native")]
use crate::adapter::handles::CallTable;
#[cfg(feature = "native")]
use crate::adapter::typed::{FromApi, IntoApi};
#[cfg(not(feature = "native"))]
use crate::adapter::values::handle_to_value;
use crate::adapter::values::JsonForm;
#[cfg(feature = "native")]
use crate::api;
use crate::api::core::lowering::{DefaultMark, LoweringDecl};
#[cfg(feature = "native")]
use crate::binding::natives::{self, Act};
use crate::guest::abi;
#[cfg(not(feature = "native"))]
use crate::guest::abi::ToArg;
use crate::guest::core::lowering::{LowerCx, Lowerer, LoweringRule};
use crate::guest::registry;
use crate::guest::lang::rewrites::userdefs::StagedNodes;
use crate::guest::techy::node::Node;
use crate::runtime::ExtError;

/// Mirrors [`Lower`](crate::api::core::lowering::Lower): the body of a
/// lowering rule.
pub trait Lower: Send + Sync {
    /// Mirrors [`Lower::lower`](crate::api::core::lowering::Lower::lower):
    /// the lowering of `node`.
    ///
    /// # Errors
    ///
    /// What the body answers.
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError>;
}

impl LoweringRule {
    /// Mirrors [`LoweringRule::new`](crate::api::core::lowering::LoweringRule::new):
    /// the rule `name`, which lowers `element` through `body`, with the
    /// declaration `decl`, the default mark `mark` and no description. The
    /// body is recorded under `name`, so that the instance of the thread that
    /// lowers finds it again, and lent; the rule is made while a feature's
    /// `register` runs.
    ///
    /// # Errors
    ///
    /// No `register` runs ([`ExtError::Handler`]), or the host's refusal.
    pub fn new(
        name: impl Into<String>,
        element: impl Into<String>,
        decl: LoweringDecl,
        body: Arc<dyn Lower>,
        mark: DefaultMark,
    ) -> Result<LoweringRule, ExtError> {
        const METHOD: &str = "LoweringRule::new";
        let name = name.into();
        let element = element.into();
        registry::record_lower(&name, body)?;
        #[cfg(not(feature = "native"))]
        {
            let lent = registry::lend_lower(&name)?;
            let args = vec![ToArg::arg(name.as_str()), ToArg::arg(element.as_str()), ToArg::arg(&decl), handle_to_value(lent), ToArg::arg(&mark)];
            JsonForm::from_value(&abi::call_module("core/lowering", METHOD, args)?)
        }
        #[cfg(feature = "native")]
        {
            abi::in_frame(Act::Lower(&name), METHOD, |view| {
                let body = natives::lend_lower(view, name.clone());
                let rule = api::core::lowering::LoweringRule::new(name.clone(), element, decl, body, mark);
                FromApi::from_api(rule, view.table)
            })
        }
    }
}

/// Mirrors [`Lowering`](crate::api::core::lowering::Lowering): what a rule's
/// body answers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lowering {
    /// The rule's default lowering.
    Default,
    /// The node is lowered to these staged nodes.
    Emit(StagedNodes),
}

/// `"default"` or `{"emit": staged}`; the host's `lowering_from_value`
/// and `lowering_to_value`.
impl JsonForm for Lowering {
    fn to_value(&self) -> Value {
        match self {
            Lowering::Default => Value::String("default".to_string()),
            Lowering::Emit(staged) => emit_to_value(staged),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        Ok(emit_from_value("a lowering", "default", value)?.map_or(Lowering::Default, Lowering::Emit))
    }
}

/// The api's `Lowering`, each staged node the entry of its handle; the
/// host's `lowering_from_value`.
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for Lowering {
    type Api = api::core::lowering::Lowering;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        Ok(match self {
            Lowering::Default => api::core::lowering::Lowering::Default,
            Lowering::Emit(staged) => api::core::lowering::Lowering::Emit(staged.into_api(table)?),
        })
    }
}

/// Mirrors [`PresTree`](crate::api::core::lowering::PresTree): a tree of
/// presentation elements a rule builds in one call.
#[derive(Debug, Clone, PartialEq)]
pub enum PresTree {
    /// A presentation element, its payload and its children.
    Node {
        /// The element's name.
        element: String,
        /// Its payload.
        payload: Value,
        /// Its children.
        children: Vec<PresTree>,
    },
    /// Nodes staged already, placed here in order.
    Build(StagedNodes),
    /// A text.
    Text(String),
}

/// `{"element", "payload", "children"}`, `{"build": staged}` (`staged` a
/// staged node, a sequence of staged nodes or a joined list) or `{"text":
/// text}`; read by the host's `pres_tree_from_value`.
impl JsonForm for PresTree {
    fn to_value(&self) -> Value {
        match self {
            PresTree::Node { element, payload, children } => {
                json!({ "element": element, "payload": payload, "children": children.iter().map(JsonForm::to_value).collect::<Vec<_>>() })
            }
            PresTree::Build(staged) => json!({ "build": staged.to_value() }),
            PresTree::Text(text) => json!({ "text": text }),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a presentation tree";
        let map = object(WHAT, value)?;
        if let (Some(build), 1) = (map.get("build"), map.len()) {
            return Ok(PresTree::Build(StagedNodes::from_value(build)?));
        }
        if let (Some(Value::String(text)), 1) = (map.get("text"), map.len()) {
            return Ok(PresTree::Text(text.clone()));
        }
        if !map.contains_key("element") {
            return Err(not_shape(WHAT, "`{\"element\", \"payload\", \"children\"}`, `{\"build\": staged}` or `{\"text\": text}`", value));
        }
        let payload = match map.get("payload") {
            None | Some(Value::Null) => json!({}),
            Some(payload) => payload.clone(),
        };
        let children: Option<Vec<PresTree>> = get(WHAT, map, "children")?;
        Ok(PresTree::Node { element: get(WHAT, map, "element")?, payload, children: children.unwrap_or_default() })
    }
}

/// The api's `PresTree`, each staged node the entry of its handle, a `null`
/// payload the empty map; the host's `pres_tree_from_value`.
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for &PresTree {
    type Api = api::core::lowering::PresTree;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        Ok(match self {
            PresTree::Node { element, payload, children } => api::core::lowering::PresTree::Node {
                element: element.clone(),
                payload: if payload.is_null() { json!({}) } else { payload.clone() },
                children: children.iter().map(|child| child.into_api(table)).collect::<Result<_, _>>()?,
            },
            PresTree::Build(staged) => api::core::lowering::PresTree::Build(staged.into_api(table)?),
            PresTree::Text(text) => api::core::lowering::PresTree::Text(text.clone()),
        })
    }
}

/// Mirrors `flm-core`'s `NodeAddr`: the address of a node, its tree's
/// identifier and the node's index in it. The guest's own: the api's holds
/// `flm-core`'s node identifier, which only a tree makes. Its serde form is
/// its data form, `{"tree", "node"}`, so that a guest's own data can hold an
/// address.
#[derive(Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct NodeAddr {
    /// The tree's identifier.
    pub tree: String,
    /// The node's index in the tree.
    pub node: usize,
}

/// `{"tree", "node"}`; the host's `node_addr_to_value`.
impl JsonForm for NodeAddr {
    fn to_value(&self) -> Value {
        json!({ "tree": self.tree, "node": self.node })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a node's address";
        let map = object(WHAT, value)?;
        Ok(NodeAddr { tree: get(WHAT, map, "tree")?, node: get(WHAT, map, "node")? })
    }
}

/// The api's address, its tree's identifier written and its node's index;
/// the host's `node_addr_to_value`.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, flm_core::model::addr::NodeAddr> for NodeAddr {
    fn from_api(api: flm_core::model::addr::NodeAddr, _: &mut CallTable<'c>) -> Result<Self, ExtError> {
        Ok(NodeAddr { tree: api.tree.to_string(), node: api.node.index() })
    }
}
