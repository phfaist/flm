//! Mirrors [`flm_core::lang::text`]: reading the characters of a run of nodes.

use crate::api::techy::node::Node;
use crate::sealed::Token;

pub use flm_core::lang::text::NotCharacters;

/// Mirrors [`chars_text`](flm_core::lang::text::chars_text): the characters of
/// `nodes`, or `None` when they hold anything but characters. What differs: the
/// nodes are the extension API's.
pub fn chars_text<'t>(nodes: &[Box<dyn Node<'t> + 't>]) -> Option<String> {
    flm_core::lang::text::chars_text(nodes.iter().map(|node| node.core_node_ref(Token::new())))
}

/// Mirrors [`read_chars`](flm_core::lang::text::read_chars): the characters of
/// `nodes`, placeholders included. What differs: the nodes are the extension
/// API's; the content is answered as its two readings, [`CharsContent`].
///
/// # Errors
///
/// A node is not characters ([`NotCharacters`]).
pub fn read_chars<'t>(nodes: &[Box<dyn Node<'t> + 't>]) -> Result<CharsContent, NotCharacters> {
    let read = flm_core::lang::text::read_chars(nodes.iter().map(|node| node.core_node_ref(Token::new())))?;
    Ok(CharsContent { text: read.text(), has_placeholder: read.has_placeholder() })
}

/// Mirrors [`CharsContent`](flm_core::lang::text::CharsContent): the content
/// [`read_chars`] read. What differs: its two readings as data, not its
/// segments.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct CharsContent {
    /// Mirrors [`CharsContent::text`](flm_core::lang::text::CharsContent::text):
    /// the characters, or `None` when a placeholder stands among them.
    pub text: Option<String>,
    /// Mirrors [`CharsContent::has_placeholder`](flm_core::lang::text::CharsContent::has_placeholder).
    pub has_placeholder: bool,
}
