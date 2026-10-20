//! The lowered-trees table, into which a backend's lowering stage writes its
//! result, and the address of a node inside a lowered tree.
//!
//! A **table** is typed data that one stage writes beside the frozen trees and
//! other stages read; its identity is its registered dotted name, and that
//! name is also a capability ([`Table`]; see [frozen trees and computed
//! data](crate::guide::concepts_overview#frozen-trees-and-computed-data)).
//!
//! Every backend uses the table type of this module, [`LoweredTrees`],
//! with an instance of its own named `flm.lowered.<backend>`
//! ([`lowered_table_name`], [`lowered_table`]). For each document tree, the
//! table holds the lowered tree ([`LoweredTree`]), the identifiers of the
//! definitions its rules required (style and script definitions on HTML,
//! package requests and definition pieces on LaTeX), and how many rounds
//! lowering ran. [`LoweredAddr`] is the address of one node inside a lowered
//! tree; the text backend's position marks use it.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use techy::core::node::{NodeRef, NodeTree};
use techy::serialize::SerialValue;

use crate::model::addr::{NodeId, TreeId};
use crate::model::names::{BackendName, CapName, Name};
use crate::core::data::table::{ProjectCx, Table, TableHandle};
use crate::lang::Flm;

/// The address of a node of a *lowered* tree: the document tree it was
/// lowered from, and the node's position in that tree's lowered form (its
/// index in storage order).
///
/// It is not a [`NodeAddr`](crate::model::addr::NodeAddr), which identifies a
/// node of a *frozen* tree, for two reasons. A lowered node may have no
/// frozen counterpart: a rule may synthesize nodes that represent no single
/// frozen node, and a lowered tree stores no map back to the frozen one. And
/// the node identifiers techy assigns carry a per-tree tag that differs from
/// run to run, whereas a position is the same in two runs over the same
/// document. An address is looked up in the lowered-trees table with
/// [`resolve`](LoweredAddr::resolve). The text backend's position marks
/// ([`TextMark::addr`](crate::stdcore::backends::text::TextMark::addr)) are
/// addresses of this kind.
///
/// # Examples
///
/// ```
/// use flm_core::model::addr::TreeId;
/// use flm_core::stdcore::backends::lowering::LoweredAddr;
///
/// let addr = LoweredAddr::at(TreeId::tree(0), 5);
/// assert_eq!(addr.to_string(), "tree#0/5");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LoweredAddr {
    /// The document tree whose lowered form holds the node.
    pub tree: TreeId,
    /// The node's index in the lowered tree's storage order.
    pub index: u32,
}

impl LoweredAddr {
    /// The address of the node `node` in the lowered form of the document tree
    /// `tree`.
    ///
    /// `node` must be a node of that lowered tree. Nothing checks it here: an
    /// identifier from another tree gives the address of a different
    /// position.
    pub fn new(tree: TreeId, node: NodeId) -> LoweredAddr {
        LoweredAddr { tree, index: node.index() as u32 }
    }

    /// The address of the node at `index` in storage order in the lowered form
    /// of the document tree `tree`.
    pub const fn at(tree: TreeId, index: u32) -> LoweredAddr {
        LoweredAddr { tree, index }
    }

    /// The node at this address, looked up in `trees`.
    ///
    /// Returns `None` when `trees` holds no lowered form of this address's
    /// document tree (its lowering failed, or the backend never lowered it), or
    /// when that lowered tree has no node at this position.
    pub fn resolve<'t>(&self, trees: &'t LoweredTrees) -> Option<NodeRef<'t, Flm>> {
        trees.get(self.tree)?.tree.iter_storage_order().nth(self.index as usize)
    }
}

impl fmt::Display for LoweredAddr {
    /// `tree#0/5`: the tree, then the index.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.tree, self.index)
    }
}

/// The lowered form of one document tree: the tree an emitter walks, the
/// requirements its rules recorded, and how many rounds lowering took.
#[derive(Debug, Clone)]
pub struct LoweredTree {
    /// The lowered tree.
    pub tree: Arc<NodeTree<Flm>>,
    /// The requirements of this tree: the identifiers of the definitions its
    /// output needs (style and script definitions on HTML, package requests
    /// and definition pieces on LaTeX; see [style and script
    /// definitions](crate::guide::concepts_overview#style-and-script-definitions)).
    /// Two sets, merged: what the selected rules require whenever they are
    /// selected
    /// ([`LoweringDecl::selection_requires`](crate::core::lowering::LoweringDecl::selection_requires),
    /// the same for every lowered tree of one plan), and what the rules
    /// recorded at their nodes in this tree, through
    /// [`Lowerer::require`](crate::core::lowering::Lowerer::require). The
    /// backend that writes the output resolves them.
    pub requires: BTreeSet<Name>,
    /// How many rounds ran over this tree; at least one. See
    /// [`lower_tree`](super::lower_tree).
    pub rounds: u32,
}

/// The lowered trees of one backend, keyed by the document tree each was
/// lowered from — the table `flm.lowered.<backend>`.
///
/// The backend's lowering stage ([`LoweringStage`](super::LoweringStage))
/// writes it, and the backend's emit stage reads it. A tree whose lowering
/// failed has no entry here; the failure is reported as an Error diagnostic
/// instead ([`super::error`]). The table's projection lists `{tree, rounds,
/// requires}` per entry, not the lowered trees themselves: a consumer that
/// needs them projects them itself ([`crate::dump::project`]).
#[derive(Debug, Clone, Default)]
pub struct LoweredTrees {
    /// Document tree → its lowered form.
    pub trees: BTreeMap<TreeId, LoweredTree>,
}

impl LoweredTrees {
    /// The lowered form of the document tree `id`, or `None` when the backend
    /// has none for it.
    pub fn get(&self, id: TreeId) -> Option<&LoweredTree> {
        self.trees.get(&id)
    }
}

impl Table for LoweredTrees {
    fn absorb(&mut self, piece: LoweredTrees) {
        self.trees.extend(piece.trees);
    }

    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.trees
                .iter()
                .map(|(id, lowered)| {
                    SerialValue::Map(vec![
                        (String::from("tree"), cx.tree_ref(*id)),
                        (String::from("rounds"), SerialValue::Int(i64::from(lowered.rounds))),
                        (
                            String::from("requires"),
                            SerialValue::List(
                                lowered
                                    .requires
                                    .iter()
                                    .map(|name| SerialValue::Str(name.to_string()))
                                    .collect(),
                            ),
                        ),
                    ])
                })
                .collect(),
        )
    }
}

/// The name of `backend`'s lowered-trees table, `flm.lowered.<backend>` — both
/// the table's name and the capability that writing it provides.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::lowering::lowered_table_name;
///
/// assert_eq!(lowered_table_name(&"text".into()).as_str(), "flm.lowered.text");
/// ```
pub fn lowered_table_name(backend: &BackendName) -> CapName {
    CapName::from(format!("flm.lowered.{backend}"))
}

/// The typed handle through which a stage reads or writes `backend`'s
/// lowered trees.
///
/// The feature that registers the backend must have declared the table under
/// the name [`lowered_table_name`] returns; a handle alone does not create
/// it.
pub fn lowered_table(backend: &BackendName) -> TableHandle<LoweredTrees> {
    TableHandle::new(lowered_table_name(backend))
}
