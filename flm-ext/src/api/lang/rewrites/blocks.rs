//! Mirrors [`flm_core::lang::rewrites::blocks`]: block structure predicates.

use crate::api::techy::node::Node;
use crate::sealed::Token;

/// Mirrors [`argument_holds_blocks`](flm_core::lang::rewrites::blocks::argument_holds_blocks):
/// whether the argument at `index` of `node` is laid out as blocks. What
/// differs: the node is the extension API's.
pub fn argument_holds_blocks(node: &dyn Node<'_>, index: usize) -> bool {
    flm_core::lang::rewrites::blocks::argument_holds_blocks(&node.core_node_ref(Token::new()), index)
}
