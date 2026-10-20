//! The façades over two tables of the standard package that a stage's
//! context and a rule's lowerer answer typed: the numbering feature's
//! [`Counters`](flm_core::stdcore::features::numbering::Counters)
//! (`CountersFacade`) and the references feature's
//! [`Anchors`](flm_core::stdcore::features::refs::Anchors)
//! (`AnchorsFacade`); and the reading of a point's identifier from data.
//!
//! Both tables are keyed by a [`PointId`], whose node cannot be made from
//! data in general (techy makes a `NodeId` only for a node of a tree). Each
//! façade therefore holds, beside the table, the trees its reader can see
//! (`Seen`): for a rule, the tree being lowered and its id; for a stage,
//! the document's frozen trees through the context. A point written as data
//! `{ addr: { tree, node }, index }` is read back against those trees
//! ([`PointsCore::point_from_value`]): the tree by its id's display text, the
//! node by its index through `NodeTree::slice(index..index + 1)`.

use flm_core::lang::Flm;
use flm_core::model::addr::{NodeAddr, TreeId};
use flm_core::core::run::StageCx as CoreStageCx;
use flm_core::stdcore::features::numbering::{Counters as CoreCounters, FormatOptions};
use flm_core::stdcore::features::refs::Anchors as CoreAnchors;
use flm_core::techy::core::node::NodeTree;
use serde_json::Value;

use crate::adapter::stage::{root_of, tree_id_of};
use crate::adapter::values::{field, fields};
use crate::api::model::aspects::PointId;
use crate::api::stdcore::features::numbering::{CounterRecord, Counters, NumberSegment};
use crate::api::stdcore::features::refs::Anchors;
use crate::runtime::ExtError;
use crate::sealed::{Sealed, Token};

/// The seal of [`Counters`] and [`Anchors`], with the reading of a point's
/// identifier from data against the trees the façade's reader can see.
pub trait PointsCore {
    /// The point `value` names, `{ addr: { tree, node }, index }`, read back
    /// against the trees the reader can see; `method` names the method in
    /// the errors.
    ///
    /// # Errors
    ///
    /// The value is not of that shape ([`ExtError::Value`]); its tree is
    /// not one the reader can see, or its node index is past the tree's end
    /// ([`ExtError::Argument`]).
    fn point_from_value(&self, token: Token, method: &str, value: &Value) -> Result<PointId, ExtError>;
}

/// The document's frozen trees, as a stage's context holds them.
pub(crate) trait FrozenTrees {
    /// The frozen tree `id`, if the document holds it.
    fn frozen(&self, id: TreeId) -> Option<&NodeTree<Flm>>;
}

impl FrozenTrees for CoreStageCx<'_> {
    fn frozen(&self, id: TreeId) -> Option<&NodeTree<Flm>> {
        self.frozen_tree(id).map(|tree| &**tree)
    }
}

/// The trees a table's reader can see.
#[derive(Clone, Copy)]
pub(crate) enum Seen<'t> {
    /// A rule's: the tree being lowered, and its id.
    Rule {
        /// The tree's id.
        id: TreeId,
        /// The tree.
        tree: &'t NodeTree<Flm>,
    },
    /// A stage's: the document's frozen trees.
    Stage(&'t (dyn FrozenTrees + 't)),
}

impl Seen<'_> {
    /// The point of `value` (see [`PointsCore::point_from_value`]).
    fn point(&self, method: &str, value: &Value) -> Result<PointId, ExtError> {
        const WHAT: &str = "a point's identifier";
        const ADDR: &str = "a point's address";
        let refuse = |detail: String| ExtError::Argument { method: method.to_string(), detail };
        let map = fields(WHAT, value, &["addr", "index"])?;
        let index = number(WHAT, "index", field(&map, "index"))?;
        let addr = fields(ADDR, field(&map, "addr"), &["tree", "node"])?;
        let node = number(ADDR, "node", field(&addr, "node"))?;
        let text = match field(&addr, "tree") {
            Value::String(text) => text.as_str(),
            other => {
                return Err(ExtError::Value { detail: format!("the field `tree` of {ADDR} must be a text, not `{other}`") });
            }
        };
        let (id, tree) = match *self {
            Seen::Rule { id, tree } => {
                if id.to_string() != text {
                    return Err(refuse(format!(
                        "the address of the point must be in the tree being lowered (`{id}`), not in `{text}`"
                    )));
                }
                (id, tree)
            }
            Seen::Stage(trees) => {
                match tree_id_of(text).and_then(|id| trees.frozen(id).map(|tree| (id, tree))) {
                    Some(found) => found,
                    None => {
                        return Err(refuse(format!("the address of the point must be in a tree of the document, not in `{text}`")));
                    }
                }
            }
        };
        let found = tree.slice(node..node.saturating_add(1)).and_then(|slice| slice.first());
        let Some(found) = found.filter(|_| node < u32::MAX) else {
            return Err(refuse(format!(
                "the address of the point must name a node of the tree `{id}`: it has {} nodes, not a node {node}",
                tree.node_count()
            )));
        };
        Ok(PointId { addr: NodeAddr::new(id, found.id()), index })
    }
}

/// A number field from 0 up, below 2³².
fn number(what: &str, key: &str, value: &Value) -> Result<u32, ExtError> {
    value.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| match value {
        Value::Null => ExtError::Value { detail: format!("{what} must have the field `{key}`") },
        other => ExtError::Value { detail: format!("the field `{key}` of {what} must be a number from 0 up, not `{other}`") },
    })
}

// --- the counters ------------------------------------------------------------------------------

/// The extension API's [`Counters`] over `flm-core`'s table, for a reader
/// that sees the trees `seen`.
pub(crate) struct CountersFacade<'t> {
    table: &'t CoreCounters,
    seen: Seen<'t>,
}

impl<'t> CountersFacade<'t> {
    /// The façade over `table`, read by a reader that sees `seen`.
    pub(crate) fn new(table: &'t CoreCounters, seen: Seen<'t>) -> CountersFacade<'t> {
        CountersFacade { table, seen }
    }
}

impl Sealed for CountersFacade<'_> {}

impl PointsCore for CountersFacade<'_> {
    fn point_from_value(&self, _: Token, method: &str, value: &Value) -> Result<PointId, ExtError> {
        self.seen.point(method, value)
    }
}

impl Counters for CountersFacade<'_> {
    fn record(&self, point: PointId) -> Option<CounterRecord> {
        let record = self.table.record(point)?;
        Some(CounterRecord {
            counter: record.counter.to_string(),
            value: record.value.components().to_vec(),
            style: record.style.name().to_string(),
            instance: record.instance,
        })
    }

    fn format<'s>(&self, point: PointId, options: &FormatOptions) -> Result<Option<Vec<NumberSegment<'s>>>, ExtError> {
        match self.table.format(point, options) {
            None => Ok(None),
            Some(Err(error)) => {
                Err(ExtError::Core { kind: "format".to_string(), detail: error.to_string(), data: Value::Null })
            }
            Some(Ok(segments)) => Ok(Some(
                segments
                    .iter()
                    .map(|segment| NumberSegment { content: root_of(segment.content()), link: segment.link() })
                    .collect(),
            )),
        }
    }
}

// --- the anchors -------------------------------------------------------------------------------

/// The extension API's [`Anchors`] over `flm-core`'s table, for a reader
/// that sees the trees `seen`.
pub(crate) struct AnchorsFacade<'t> {
    table: &'t CoreAnchors,
    seen: Seen<'t>,
}

impl<'t> AnchorsFacade<'t> {
    /// The façade over `table`, read by a reader that sees `seen`.
    pub(crate) fn new(table: &'t CoreAnchors, seen: Seen<'t>) -> AnchorsFacade<'t> {
        AnchorsFacade { table, seen }
    }
}

impl Sealed for AnchorsFacade<'_> {}

impl PointsCore for AnchorsFacade<'_> {
    fn point_from_value(&self, _: Token, method: &str, value: &Value) -> Result<PointId, ExtError> {
        self.seen.point(method, value)
    }
}

impl Anchors for AnchorsFacade<'_> {
    fn id(&self, point: PointId) -> Option<String> {
        self.table.id(point).map(str::to_string)
    }
}
