//! Mirrors [`flm_core::stdlib::backends::lowering`]: the library's lowering
//! stage, which a backend declares as its own
//! ([`BackendDef`](crate::api::core::registry::BackendDef)), the name of the
//! table it writes, and what it writes, read by the backend's emit stage;
//! and the blank test and the trimmed staging of the content a rule cuts out
//! of a longer run itself.

use std::sync::Arc;

use flm_core::lang::Flm;
use flm_core::techy::core::node::NodeRef;

use crate::api::core::lowering::{LowerCx, Lowerer, Staged};
use crate::api::core::registry::LibraryStage;
use crate::api::core::run::Trees;
use crate::api::techy::node::{Node, Tree};
use crate::runtime::ExtError;
use crate::sealed::{Sealed, Token};

/// Mirrors [`LoweredTrees`](flm_core::stdlib::backends::lowering::LoweredTrees):
/// the lowered trees of one backend, the table `flm.lowered.<backend>`, each
/// with the fields of its [`LoweredTree`](flm_core::stdlib::backends::lowering::LoweredTree).
///
/// What differs: a façade over the table, answered by a read of the table by
/// name ([`TableValue::Lowered`](crate::api::core::data::table::TableValue::Lowered));
/// a document tree is named by its id's display text (`tree#0`); the nodes
/// of a lowered tree are the presentation nodes the emitter walks
/// ([`Node::element_name`], [`Node::payload`], [`Node::slot_content_nodes`]).
/// A document tree whose lowering failed has no entry. The order of the
/// document is the order of
/// [`StageCx::trees`](crate::api::core::run::StageCx::trees); the ids here
/// are in id order.
///
/// The trait is sealed: the adapter alone implements it.
pub trait LoweredTrees: Sealed {
    /// Mirrors the field [`LoweredTrees::trees`](flm_core::stdlib::backends::lowering::LoweredTrees::trees):
    /// every lowered tree, by the id of the document tree it was lowered
    /// from, in id order. What differs: a [`Trees`] of pairs.
    fn trees(&self) -> Trees;

    /// Mirrors [`LoweredTrees::get`](flm_core::stdlib::backends::lowering::LoweredTrees::get)
    /// and the field [`LoweredTree::tree`](flm_core::stdlib::backends::lowering::LoweredTree::tree):
    /// the lowered form of the document tree `id`, or `None` when the backend
    /// has none for it (or `id` names no tree).
    fn tree(&self, id: &str) -> Option<Box<dyn Tree>>;

    /// Mirrors the field [`LoweredTree::requires`](flm_core::stdlib::backends::lowering::LoweredTree::requires)
    /// of the lowered form of the document tree `id`: the identifiers of the
    /// definitions its output needs, in order. `None` as for
    /// [`tree`](LoweredTrees::tree).
    fn requires(&self, id: &str) -> Option<Vec<String>>;

    /// Mirrors the field [`LoweredTree::rounds`](flm_core::stdlib::backends::lowering::LoweredTree::rounds)
    /// of the lowered form of the document tree `id`: how many rounds of
    /// lowering ran over it, at least one. `None` as for
    /// [`tree`](LoweredTrees::tree).
    fn rounds(&self, id: &str) -> Option<u32>;

    /// Mirrors [`LoweredAddr::resolve`](flm_core::stdlib::backends::lowering::LoweredAddr::resolve):
    /// the node at the address `addr` of a lowered tree, looked up in these
    /// trees. `None` when this table holds no lowered form of the address's
    /// document tree, or when that lowered tree has no node at this position.
    ///
    /// What differs: the address is its display text, as the projection of
    /// the text output's position marks writes it, `tree#0/5` (the document
    /// tree's id, then the node's index in the lowered tree's storage order);
    /// a text of another form names no node and answers `None`. The node is a
    /// façade that shares the tree, as [`Tree::root`] answers.
    fn node_at<'s>(&self, addr: &str) -> Option<Box<dyn Node<'s> + 's>>;
}

fn _object_safe(_: &dyn LoweredTrees) {}

/// Mirrors [`LoweringStage`](flm_core::stdlib::backends::lowering::LoweringStage):
/// the library's lowering stage for one backend, `flm.<backend>.lower`, a
/// stage computing per tree that lowers every frozen tree with the rules
/// the plan selected for the backend and writes the table
/// `flm.lowered.<backend>` ([`lowered_table_name`]). What differs: only its
/// constructor, which answers the stage as a [`LibraryStage`]; its
/// `backend`, `name` and `table` readers are not mirrored.
pub struct LoweringStage;

impl LoweringStage {
    /// Mirrors [`LoweringStage::new`](flm_core::stdlib::backends::lowering::LoweringStage::new):
    /// the lowering stage for the backend `backend`. What differs: it
    /// answers a [`LibraryStage`], the value a
    /// [`BackendStage::Library`](crate::api::core::registry::BackendStage::Library)
    /// holds. The feature that declares the backend declares the table the
    /// stage writes, `flm.lowered.<backend>`, with
    /// [`FeatureBuilder::table_named`](crate::api::core::registry::FeatureBuilder::table_named)
    /// and [`StandardTable::LoweredTrees`](crate::api::core::registry::StandardTable::LoweredTrees).
    // The mirror of `flm-core`'s constructor, which answers the stage; here it
    // is answered as the opaque value a backend declaration takes.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(backend: &str) -> LibraryStage {
        LibraryStage(Arc::new(flm_core::stdlib::backends::lowering::LoweringStage::new(backend)))
    }
}

/// Mirrors [`lowered_table_name`](flm_core::stdlib::backends::lowering::lowered_table_name):
/// the name of the table of the backend `backend`'s lowered trees,
/// `flm.lowered.<backend>`. What differs: the backend is a `&str`, and the
/// name an owned `String`.
pub fn lowered_table_name(backend: &str) -> String {
    flm_core::stdlib::backends::lowering::lowered_table_name(&flm_core::model::names::BackendName::from(backend)).to_string()
}

/// Mirrors [`is_blank`](flm_core::stdlib::backends::lowering::is_blank):
/// whether `node` is blank, a node that is whitespace, such as `~` and `\ `,
/// or text made of whitespace alone (the space, the tab, the carriage return
/// and the line feed); the one blank test of every reader of FLM content.
/// What differs: the node is the extension API's.
pub fn is_blank(node: &dyn Node<'_>) -> bool {
    flm_core::stdlib::backends::lowering::is_blank(&node.core_node_ref(Token::new()))
}

/// Mirrors [`stage_trimmed`](flm_core::stdlib::backends::lowering::stage_trimmed):
/// stages `content`, inline content that the rule cuts out of a longer run
/// itself, without the whitespace at its edges: the blank nodes
/// ([`is_blank`]) and the comments before its first content and after its
/// last are left out, and the text at either edge is staged without its
/// leading or its trailing whitespace. Content holding only whitespace and
/// comments stages nothing. What differs: the nodes are the extension API's,
/// and no further node is passed over (no `passed_over` function), as
/// [`Lowerer::lower_region`] has no shape function.
///
/// # Errors
///
/// `cx` is not the call's context ([`ExtError::Argument`]), or `flm-core`'s
/// refusal ([`ExtError::Core`]) from the lowering of the content and the
/// staging of the trimmed text.
pub fn stage_trimmed<'t>(cx: &mut dyn LowerCx, lw: &mut dyn Lowerer, content: &[Box<dyn Node<'t> + 't>]) -> Result<Vec<Staged>, ExtError> {
    let nodes: Vec<NodeRef<'_, Flm>> = content.iter().map(|node| node.core_node_ref(Token::new())).collect();
    lw.stage_core(Token::new(), "stage_trimmed", cx, &mut |cx, lw| {
        flm_core::stdlib::backends::lowering::stage_trimmed(cx, lw, &nodes, |_| false)
    })
}
