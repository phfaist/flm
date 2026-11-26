//! Mirrors [`flm_core::lang::copy`]: the value of a key-value entry as pieces
//! of the document it was read from. Its function
//! [`key_value_pieces`](flm_core::lang::copy::key_value_pieces) is mirrored as
//! the method [`KeyValEntry::value_pieces`](crate::api::techy::extract::KeyValEntry::value_pieces)
//! of an entry, which keeps the bracket it was read from.

use crate::api::techy::node::Node;

/// Mirrors [`WrittenPiece`](flm_core::lang::copy::WrittenPiece): one piece of
/// the value of a key-value entry, as the author wrote it in a bracket: a node
/// of the document, or characters split off a text node that also holds the
/// key (`Custom` in `tag=Custom`), which are not a node of their own. What
/// differs: the node is the extension API's [`Node`], of the tree of the
/// bracket's content; across a binding `{"node": node}` or `{"text": text}`.
pub enum WrittenPiece<'t> {
    /// A node of the tree that holds the bracket, shown as the content it is.
    Node(Box<dyn Node<'t> + 't>),
    /// Characters of the document, as the tree holds them.
    Text(String),
}
