//! Mirrors techy's extraction helpers, reached as [`flm_core::techy::extract`]:
//! key-value lists.

use crate::adapter::node::KeyValsFacade;
use crate::api::lang::copy::WrittenPiece;
use crate::api::techy::node::Node;
use crate::runtime::ExtError;
use crate::sealed::{Sealed, Token};

/// Mirrors [`parse_keyval_drop_annotations`](flm_core::techy::extract::parse_keyval_drop_annotations):
/// reads `nodes` as a key-value list. What differs: the nodes are the
/// extension API's, and must be consecutive siblings of one tree; an
/// `ExtractError` is [`ExtError::Core`].
///
/// # Errors
///
/// The nodes are not consecutive siblings of one tree
/// ([`ExtError::Argument`]), or they do not read as a key-value list
/// ([`ExtError::Core`]).
pub fn parse_keyval_drop_annotations<'t>(nodes: &[Box<dyn Node<'t> + 't>]) -> Result<Box<dyn KeyVals<'t> + 't>, ExtError> {
    let method = "parse_keyval_drop_annotations";
    let not_a_run = || ExtError::Argument {
        method: method.to_string(),
        detail: "the nodes must be consecutive children of one node".to_string(),
    };
    let refs: Vec<_> = nodes.iter().map(|node| node.core_node_ref(Token::new())).collect();
    let Some(first) = refs.first() else {
        return Ok(Box::new(KeyValsFacade::empty()));
    };
    let tree = first.tree();
    let start = first.id().index();
    let consecutive = refs
        .iter()
        .enumerate()
        .all(|(i, node)| std::ptr::eq(node.tree(), tree) && node.id().index() == start + i);
    if !consecutive {
        return Err(not_a_run());
    }
    let range = u32::try_from(start).ok().zip(u32::try_from(start + refs.len()).ok()).ok_or_else(not_a_run)?;
    let slice = tree.slice(range.0..range.1).ok_or_else(not_a_run)?;
    let list = flm_core::techy::extract::parse_keyval_drop_annotations(slice)
        .map_err(|error| ExtError::Core { kind: "extract".to_string(), detail: error.to_string(), data: serde_json::Value::Null })?;
    Ok(Box::new(KeyValsFacade::new(list, slice, nodes[0].clone_box(Token::new()))))
}

/// Mirrors [`KeyVals`](flm_core::techy::extract::KeyVals): a key-value list.
/// What differs: a façade; [`iter`](KeyVals::iter) answers a list. `'t` is the
/// lifetime of the tree of the nodes it was read from, as for [`Node`].
///
/// The trait is sealed: the adapter alone implements it.
pub trait KeyVals<'t>: Sealed {
    /// Mirrors [`KeyVals::len`](flm_core::techy::extract::KeyVals::len).
    fn len(&self) -> usize;

    /// Mirrors [`KeyVals::is_empty`](flm_core::techy::extract::KeyVals::is_empty).
    fn is_empty(&self) -> bool;

    /// Mirrors [`KeyVals::keyval`](flm_core::techy::extract::KeyVals::keyval):
    /// the entry at position `i`.
    fn keyval(&self, i: usize) -> Option<Box<dyn KeyValEntry<'t> + 't>>;

    /// Mirrors [`KeyVals::get`](flm_core::techy::extract::KeyVals::get): the
    /// entry of key `key`.
    fn get(&self, key: &str) -> Option<Box<dyn KeyValEntry<'t> + 't>>;

    /// Mirrors [`KeyVals::iter`](flm_core::techy::extract::KeyVals::iter).
    /// What differs: a list.
    fn iter(&self) -> Vec<Box<dyn KeyValEntry<'t> + 't>>;
}

/// Mirrors [`KeyValEntry`](flm_core::techy::extract::KeyValEntry): one entry
/// of a key-value list. What differs: a façade; its value nodes are nodes of
/// the tree the key-value list owns, kept alive as long as a node of it is;
/// `'t` is the lifetime of the tree of the nodes the list was read from,
/// whose nodes [`value_pieces`](KeyValEntry::value_pieces) answers.
///
/// The trait is sealed: the adapter alone implements it.
pub trait KeyValEntry<'t>: Sealed {
    /// Mirrors [`KeyValEntry::key`](flm_core::techy::extract::KeyValEntry::key).
    /// What differs: owned text.
    fn key(&self) -> String;

    /// Mirrors [`KeyValEntry::value`](flm_core::techy::extract::KeyValEntry::value).
    /// What differs: a list.
    fn value(&self) -> Option<Vec<Box<dyn Node<'t> + 't>>>;

    /// Mirrors [`KeyValEntry::value_content`](flm_core::techy::extract::KeyValEntry::value_content).
    /// What differs: a list.
    fn value_content(&self) -> Option<Vec<Box<dyn Node<'t> + 't>>>;

    /// Mirrors [`key_value_pieces`](flm_core::lang::copy::key_value_pieces)
    /// of [`flm_core::lang::copy`], called with the bracket the list was read
    /// from and this entry's value: the value as pieces of the document,
    /// whitespace at either end dropped. It answers `None` for an entry
    /// without a value, as [`value`](KeyValEntry::value) does. A group or a callable of the value is answered as the node of
    /// the bracket it was read from, which a rule lowers as document content;
    /// characters split off a text node that also holds the key are answered
    /// as text. What differs: one method of the entry, since the list keeps
    /// the nodes it was read from.
    fn value_pieces(&self) -> Option<Vec<WrittenPiece<'t>>>;
}

fn _object_safe(_: &dyn KeyVals<'_>, _: &dyn KeyValEntry<'_>) {}
