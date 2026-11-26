//! [`NodeFacade`], the extension API's [`Node`] over a node of any tree the
//! adapter holds; [`TreeFacade`], its [`Tree`] over a tree shared through an
//! `Arc`; and the key-value façades [`KeyValsFacade`] and
//! [`KeyValEntryFacade`].
//!
//! Two rules shape the types:
//!
//! - `dyn Node<'t>` is invariant in `'t`, so [`NodeFacade<'t>`] implements the
//!   trait, and its seal, for every lifetime shorter than its tree's:
//!   `impl<'t: 's, 's> Node<'s> for NodeFacade<'t>`. A node over a tree the
//!   façade owns (an `Arc` or an `Rc`) can then be lent into a call table of any
//!   lifetime. The key-value façades hold a node of the bracket they were
//!   read from, so they implement `KeyVals<'t>` and `KeyValEntry<'t>` for
//!   the lifetime `'t` of the bracket's tree alone.
//! - Every node a method answers is a new façade that owns what it points at —
//!   the `Copy` `NodeRef` of a lent tree, a clone of the `Arc` or of the `Rc` —
//!   never a borrow of the façade it was asked on, and it is of the same variant
//!   as that façade (a child of a shared node is shared, with the same `Arc`).

use std::rc::Rc;
use std::sync::Arc;

use flm_core::dump::project::project_payload;
use flm_core::lang::{Flm, PresPayload};
use flm_core::model::element::{element_of, element_value};
use flm_core::techy::core::node::{NamedAccessError, NodeId, NodeRef, NodeSlice, NodeTree};
use flm_core::techy::extract::KeyVals as CoreKeyVals;
use serde_json::Value;

use crate::adapter::stage::projection_value;
use crate::api::lang::copy::WrittenPiece;
use crate::api::lang::mint::Mode;
use crate::api::techy::extract::{KeyValEntry, KeyVals};
use crate::api::techy::node::{Descendants, Node, NodeKindName, Span, Tree};
use crate::runtime::ExtError;
use crate::sealed::{NodeCore, Sealed, Token, TreeCore};

/// A node of any tree the adapter can hold onto: the one implementation of
/// [`Node`].
///
/// `NodeFacade` is `Clone`, and cheap to clone: each variant holds a `Copy`
/// `NodeRef`, or a reference-counted pointer to its tree and the node's id.
///
/// The ids a façade holds are ids its tree minted; a façade built with an id
/// of another tree panics when it is read, as techy's
/// [`NodeTree::node`](flm_core::techy::core::node::NodeTree::node) does. Only
/// Rust code of the host can build one that way: every façade an extension
/// receives is built by the adapter from the tree's own ids.
#[derive(Clone)]
pub enum NodeFacade<'t> {
    /// A node of a tree lent to the call (the use site of an expansion hook,
    /// and its relatives).
    #[non_exhaustive]
    Lent(NodeRef<'t, Flm>),
    /// A node of a tree held by an `Arc`: the root of a [`TreeFacade`] and
    /// its relatives, a node of a stage's walk over the frozen trees.
    #[non_exhaustive]
    Shared {
        /// The tree.
        tree: Arc<NodeTree<Flm>>,
        /// The node, in `tree`.
        id: NodeId,
    },
    /// A node of the tree a key-value list owns, kept alive by the `Rc` (a
    /// key-value list never leaves the thread of the call).
    #[non_exhaustive]
    KeyVals {
        /// The key-value list, which owns the tree its values are nodes of.
        tree: Rc<CoreKeyVals<Flm>>,
        /// The node, in the list's tree.
        id: NodeId,
    },
}

impl<'c> NodeFacade<'c> {
    /// The façade of a node lent for the call: its relatives borrow the same
    /// tree.
    pub fn lent(node: NodeRef<'c, Flm>) -> NodeFacade<'c> {
        NodeFacade::Lent(node)
    }

    /// The `flm-core` node behind the façade, borrowing the façade.
    pub fn node_ref(&self) -> NodeRef<'_, Flm> {
        match self {
            NodeFacade::Lent(node) => *node,
            NodeFacade::Shared { tree, id } => tree.node(*id),
            NodeFacade::KeyVals { tree, id } => tree.tree().node(*id),
        }
    }

    /// The façade of the node `id` of the same tree, owning what this one
    /// owns.
    fn at(&self, id: NodeId) -> NodeFacade<'c> {
        match self {
            NodeFacade::Lent(node) => NodeFacade::Lent(node.tree().node(id)),
            NodeFacade::Shared { tree, .. } => NodeFacade::Shared { tree: Arc::clone(tree), id },
            NodeFacade::KeyVals { tree, .. } => NodeFacade::KeyVals { tree: Rc::clone(tree), id },
        }
    }

    /// `node`, a node of this façade's tree, as a boxed façade.
    fn one<'s>(&self, node: NodeRef<'_, Flm>) -> Box<dyn Node<'s> + 's>
    where
        'c: 's,
    {
        Box::new(self.at(node.id()))
    }

    /// The nodes of `nodes`, a run of this façade's tree, as boxed façades.
    fn slice<'s>(&self, nodes: NodeSlice<'_, Flm>) -> Vec<Box<dyn Node<'s> + 's>>
    where
        'c: 's,
    {
        nodes.iter().map(|node| self.one(node)).collect()
    }
}

impl NodeFacade<'static> {
    /// The façade of the node `id` of `tree`, a tree shared through an `Arc`:
    /// the façade and its relatives keep the tree alive.
    ///
    /// `id` must be a node of `tree` (see the type's documentation).
    pub fn shared(tree: Arc<NodeTree<Flm>>, id: NodeId) -> NodeFacade<'static> {
        NodeFacade::Shared { tree, id }
    }
}

/// A `NamedAccessError` as the extension API reports it.
fn access(error: &NamedAccessError) -> ExtError {
    ExtError::Core { kind: "access".to_string(), detail: error.to_string(), data: serde_json::Value::Null }
}

impl<'t: 's, 's> NodeCore<'s> for NodeFacade<'t> {
    fn core_node_ref(&self, _: Token) -> NodeRef<'_, Flm> {
        self.node_ref()
    }

    fn clone_box(&self, _: Token) -> Box<dyn Node<'s> + 's> {
        Box::new(self.clone())
    }

    fn relative(&self, _: Token, id: NodeId) -> Box<dyn Node<'s> + 's> {
        Box::new(self.at(id))
    }
}

impl<'t: 's, 's> Node<'s> for NodeFacade<'t> {
    fn kind(&self) -> NodeKindName {
        let node = self.node_ref();
        if node.is_chars() {
            NodeKindName::Chars
        } else if node.is_group() {
            NodeKindName::Group
        } else if node.is_callable() {
            NodeKindName::Callable
        } else if node.is_comment() {
            NodeKindName::Comment
        } else {
            NodeKindName::List
        }
    }

    fn is_chars(&self) -> bool {
        self.node_ref().is_chars()
    }

    fn is_group(&self) -> bool {
        self.node_ref().is_group()
    }

    fn is_callable(&self) -> bool {
        self.node_ref().is_callable()
    }

    fn is_comment(&self) -> bool {
        self.node_ref().is_comment()
    }

    fn is_list(&self) -> bool {
        self.node_ref().is_list()
    }

    fn span(&self) -> Span {
        let range = self.node_ref().span().range();
        Span { start: range.start, end: range.end }
    }

    fn span_content(&self) -> String {
        self.node_ref().span_content().to_string()
    }

    fn parent(&self) -> Option<Box<dyn Node<'s> + 's>> {
        self.node_ref().parent().map(|parent| self.one(parent))
    }

    fn child_count(&self) -> usize {
        self.node_ref().child_count()
    }

    fn child(&self, i: usize) -> Option<Box<dyn Node<'s> + 's>> {
        self.node_ref().child(i).map(|child| self.one(child))
    }

    fn children(&self) -> Vec<Box<dyn Node<'s> + 's>> {
        self.slice(self.node_ref().children())
    }

    fn descendants(&self) -> Descendants<'s> {
        // The ids are collected with techy's iterator, which defines the
        // order; the traversal then owns them and a copy of this façade.
        let ids: Vec<NodeId> = self.node_ref().descendants().map(|node| node.id()).collect();
        let root = self.clone();
        Descendants::new(ids.into_iter().map(move |id| -> Box<dyn Node<'s> + 's> { Box::new(root.at(id)) }))
    }

    fn chars(&self) -> Option<String> {
        self.node_ref().chars().map(str::to_string)
    }

    fn name(&self) -> Option<String> {
        self.node_ref().name().map(str::to_string)
    }

    fn group_delimiters(&self) -> Option<(String, String)> {
        self.node_ref().group_delimiters().map(|(open, close)| (open.to_string(), close.to_string()))
    }

    fn argument_nodes(&self, i: usize) -> Option<Vec<Box<dyn Node<'s> + 's>>> {
        self.node_ref().argument_nodes(i).map(|nodes| self.slice(nodes))
    }

    fn argument_content_nodes(&self, i: usize) -> Option<Vec<Box<dyn Node<'s> + 's>>> {
        self.node_ref().argument_content_nodes(i).map(|nodes| self.slice(nodes))
    }

    fn argument_nodes_named(&self, name: &str) -> Result<Option<Vec<Box<dyn Node<'s> + 's>>>, ExtError> {
        let nodes = self.node_ref().argument_nodes_named(name).map_err(|error| access(&error))?;
        Ok(nodes.map(|nodes| self.slice(nodes)))
    }

    fn argument_content_nodes_named(&self, name: &str) -> Result<Option<Vec<Box<dyn Node<'s> + 's>>>, ExtError> {
        let nodes = self.node_ref().argument_content_nodes_named(name).map_err(|error| access(&error))?;
        Ok(nodes.map(|nodes| self.slice(nodes)))
    }

    fn argument_is_provided(&self, name: &str) -> Result<bool, ExtError> {
        let arguments = self.node_ref().arguments().ok_or_else(|| access(&NamedAccessError::NotACallable))?;
        let argument = arguments
            .get_named(name)
            .ok_or_else(|| access(&NamedAccessError::UnknownArgumentName { name: name.into() }))?;
        Ok(argument.is_provided())
    }

    fn body(&self) -> Option<Vec<Box<dyn Node<'s> + 's>>> {
        self.node_ref().body().map(|nodes| self.slice(nodes))
    }

    fn slot_content_nodes_named(&self, name: &str) -> Result<Vec<Box<dyn Node<'s> + 's>>, ExtError> {
        let nodes = self.node_ref().slot_content_nodes_named(name).map_err(|error| access(&error))?;
        Ok(self.slice(nodes))
    }

    fn element_name(&self) -> String {
        element_of(&self.node_ref()).name.to_string()
    }

    fn element_value(&self) -> Option<Value> {
        // A scripted element's data is the JSON value itself, as
        // `ElementSpec::with_value` stored it.
        element_value::<Value>(&self.node_ref()).cloned()
    }

    fn mode(&self) -> Mode {
        self.node_ref().parsing_state().mode()
    }

    /// The payload's projection, converted as a table's projection is
    /// (`payload_value`).
    fn payload(&self) -> Result<Option<Value>, ExtError> {
        let node = self.node_ref();
        node.ext().payload.as_ref().map(|payload| payload_value(&**payload)).transpose()
    }

    fn slot_count(&self) -> usize {
        self.node_ref().slots().map_or(0, |slots| slots.len())
    }

    fn slot_name(&self, i: usize) -> Option<String> {
        self.node_ref().slots()?.get(i)?.name().map(str::to_string)
    }

    fn slot_content_nodes(&self, i: usize) -> Option<Vec<Box<dyn Node<'s> + 's>>> {
        self.node_ref().slot_content_nodes(i).map(|nodes| self.slice(nodes))
    }
}

/// A tree shared through an `Arc`: the one implementation of [`Tree`].
///
/// `TreeFacade` is `Clone`, and cheap to clone: it holds the tree's `Arc`.
/// Its root, and every node reached from it, is a [`NodeFacade::Shared`]
/// holding the same `Arc`.
#[derive(Clone)]
pub struct TreeFacade {
    tree: Arc<NodeTree<Flm>>,
}

impl TreeFacade {
    /// The façade of `tree`.
    pub fn new(tree: Arc<NodeTree<Flm>>) -> TreeFacade {
        TreeFacade { tree }
    }
}

impl TreeCore for TreeFacade {
    fn shared(&self, _: Token) -> Arc<NodeTree<Flm>> {
        Arc::clone(&self.tree)
    }

    fn clone_box(&self, _: Token) -> Box<dyn Tree> {
        Box::new(self.clone())
    }
}

impl Tree for TreeFacade {
    fn root<'s>(&self) -> Box<dyn Node<'s> + 's> {
        Box::new(NodeFacade::shared(Arc::clone(&self.tree), self.tree.root().id()))
    }
}

/// The key-value list
/// [`parse_keyval_drop_annotations`](crate::api::techy::extract::parse_keyval_drop_annotations)
/// read: the one implementation of [`KeyVals`].
///
/// It owns the list behind an `Rc`, which its entries and their value nodes
/// share, and the bracket it was read from (a node of it, and the value of
/// each entry as pieces of it), which its entries share too. An empty run of
/// nodes reads as the empty list, which holds no tree.
#[derive(Clone)]
pub struct KeyValsFacade<'t> {
    list: Option<(Rc<CoreKeyVals<Flm>>, Rc<Bracket<'t>>)>,
}

/// The bracket a key-value list was read from, as its entries reach it: a
/// node of the bracket, and the value of each entry as pieces of the bracket,
/// which [`key_value_pieces`](flm_core::lang::copy::key_value_pieces)
/// answered when the list was read.
struct Bracket<'t> {
    /// A node of the bracket: the pieces' nodes are its relatives.
    node: Box<dyn Node<'t> + 't>,
    /// The value of each entry of the list, by position: `None` for an entry
    /// without a value.
    values: Vec<Option<Vec<BracketPiece>>>,
}

/// A piece of the value of an entry, as [`Bracket`] keeps it: the node by
/// its id in the bracket's tree.
enum BracketPiece {
    /// A node of the bracket's tree.
    Node(NodeId),
    /// Characters of the document.
    Text(String),
}

impl<'t> KeyValsFacade<'t> {
    /// The façade of `list`, read from `content`, a run of nodes of which
    /// `node` is one.
    pub(crate) fn new(list: CoreKeyVals<Flm>, content: NodeSlice<'_, Flm>, node: Box<dyn Node<'t> + 't>) -> KeyValsFacade<'t> {
        let values = list
            .iter()
            .map(|entry| {
                let value = entry.value()?;
                let pieces = flm_core::lang::copy::key_value_pieces(&content, value);
                Some(
                    pieces
                        .into_iter()
                        .map(|piece| match piece {
                            flm_core::lang::copy::WrittenPiece::Node(node) => BracketPiece::Node(node.id()),
                            flm_core::lang::copy::WrittenPiece::Text(text) => BracketPiece::Text(text),
                        })
                        .collect(),
                )
            })
            .collect();
        KeyValsFacade { list: Some((Rc::new(list), Rc::new(Bracket { node, values }))) }
    }

    /// The empty list.
    pub(crate) fn empty() -> KeyValsFacade<'t> {
        KeyValsFacade { list: None }
    }

    /// The façade of the entry at position `index`, when there is one.
    fn entry(&self, index: usize) -> Option<Box<dyn KeyValEntry<'t> + 't>> {
        let (list, bracket) = self.list.as_ref()?;
        list.keyval(index)?;
        Some(Box::new(KeyValEntryFacade { list: Rc::clone(list), bracket: Rc::clone(bracket), index }))
    }
}

impl Sealed for KeyValsFacade<'_> {}

impl<'t> KeyVals<'t> for KeyValsFacade<'t> {
    fn len(&self) -> usize {
        self.list.as_ref().map_or(0, |(list, _)| list.len())
    }

    fn is_empty(&self) -> bool {
        self.list.as_ref().is_none_or(|(list, _)| list.is_empty())
    }

    fn keyval(&self, i: usize) -> Option<Box<dyn KeyValEntry<'t> + 't>> {
        self.entry(i)
    }

    fn get(&self, key: &str) -> Option<Box<dyn KeyValEntry<'t> + 't>> {
        // The last entry of that key, as techy's `get` answers.
        let (list, _) = self.list.as_ref()?;
        let index = list.iter().enumerate().filter(|(_, entry)| entry.key() == key).map(|(i, _)| i).last()?;
        self.entry(index)
    }

    fn iter(&self) -> Vec<Box<dyn KeyValEntry<'t> + 't>> {
        (0..self.len()).filter_map(|i| self.entry(i)).collect()
    }
}

/// One entry of a [`KeyValsFacade`]: the one implementation of
/// [`KeyValEntry`]. It shares the list's `Rc`, and so do its value nodes, and
/// the list's bracket.
#[derive(Clone)]
pub struct KeyValEntryFacade<'t> {
    list: Rc<CoreKeyVals<Flm>>,
    bracket: Rc<Bracket<'t>>,
    index: usize,
}

impl KeyValEntryFacade<'_> {
    /// The value nodes `nodes` as façades sharing the list.
    fn nodes<'s>(&self, nodes: NodeSlice<'_, Flm>) -> Vec<Box<dyn Node<'s> + 's>> {
        nodes
            .iter()
            .map(|node| Box::new(NodeFacade::KeyVals { tree: Rc::clone(&self.list), id: node.id() }) as Box<dyn Node<'s> + 's>)
            .collect()
    }
}

impl Sealed for KeyValEntryFacade<'_> {}

impl<'t> KeyValEntry<'t> for KeyValEntryFacade<'t> {
    fn key(&self) -> String {
        self.list.keyval(self.index).map(|entry| entry.key().to_string()).unwrap_or_default()
    }

    fn value(&self) -> Option<Vec<Box<dyn Node<'t> + 't>>> {
        let value = self.list.keyval(self.index)?.value()?;
        Some(self.nodes(value))
    }

    fn value_content(&self) -> Option<Vec<Box<dyn Node<'t> + 't>>> {
        let value = self.list.keyval(self.index)?.value_content()?;
        Some(self.nodes(value))
    }

    fn value_pieces(&self) -> Option<Vec<WrittenPiece<'t>>> {
        let pieces = self.bracket.values.get(self.index)?.as_ref()?;
        Some(
            pieces
                .iter()
                .map(|piece| match piece {
                    BracketPiece::Node(id) => WrittenPiece::Node(self.bracket.node.relative(Token::new(), *id)),
                    BracketPiece::Text(text) => WrittenPiece::Text(text.clone()),
                })
                .collect(),
        )
    }
}

/// A presentation payload as data: its projection, converted as a table's
/// projection is ([`projection_value`]).
///
/// # Errors
///
/// The projection cannot cross as data ([`ExtError::Value`]).
fn payload_value(payload: &dyn PresPayload) -> Result<Value, ExtError> {
    projection_value(&project_payload(payload))
}

#[cfg(test)]
mod tests {
    use flm_core::techy::serialize::SerialValue;

    use super::*;

    /// A payload whose projection holds bytes, which cannot cross.
    #[derive(Debug)]
    struct Bytes;

    impl PresPayload for Bytes {
        fn project(&self) -> SerialValue {
            SerialValue::Bytes(vec![1, 2])
        }
    }

    /// A payload whose projection is a map of data.
    #[derive(Debug)]
    struct Level;

    impl PresPayload for Level {
        fn project(&self) -> SerialValue {
            SerialValue::Map(vec![("level".to_string(), SerialValue::Int(2))].into_iter().collect())
        }
    }

    #[test]
    fn a_payload_that_cannot_cross_is_an_error() {
        let error = payload_value(&Bytes).expect_err("bytes do not cross");
        assert_eq!(error.kind(), "value");
        assert_eq!(payload_value(&Level), Ok(serde_json::json!({ "level": 2 })));
    }
}
