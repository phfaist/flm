//! Mirrors [`flm_core::stdcore::backends::lowering`]: the library's lowering
//! stage, which a backend declares as its own
//! ([`BackendDef`](crate::api::core::registry::BackendDef)), the name of the
//! table it writes, and what it writes, read by the backend's emit stage.

use std::sync::Arc;

use crate::api::core::registry::LibraryStage;
use crate::api::techy::node::Node;
use crate::sealed::Sealed;

/// Mirrors [`LoweredTrees`](flm_core::stdcore::backends::lowering::LoweredTrees):
/// the lowered trees of one backend, the table `flm.lowered.<backend>`, each
/// with the fields of its [`LoweredTree`](flm_core::stdcore::backends::lowering::LoweredTree).
///
/// What differs: a façade over the table, answered by a read of the table by
/// name ([`TableValue::Lowered`](crate::api::core::data::table::TableValue::Lowered));
/// a document tree is named by its id's display text (`tree#0`), and a
/// lowered tree is answered as its root node, whose nodes are the
/// presentation nodes the emitter walks ([`Node::element_name`],
/// [`Node::payload`], [`Node::slot_content_nodes`]). A document tree whose
/// lowering failed has no entry. The order of the document is the order of
/// [`StageCx::trees`](crate::api::core::run::StageCx::trees); the ids here
/// are in id order. `'t` is the lifetime of the trees, as for [`Node`].
///
/// The trait is sealed: the adapter alone implements it.
pub trait LoweredTrees<'t>: Sealed {
    /// Mirrors the field [`LoweredTrees::trees`](flm_core::stdcore::backends::lowering::LoweredTrees::trees):
    /// every lowered tree, by the id of the document tree it was lowered
    /// from, in id order. What differs: a list of pairs, each tree its root.
    fn trees(&self) -> Vec<(String, Box<dyn Node<'t> + 't>)>;

    /// Mirrors [`LoweredTrees::get`](flm_core::stdcore::backends::lowering::LoweredTrees::get)
    /// and the field [`LoweredTree::tree`](flm_core::stdcore::backends::lowering::LoweredTree::tree):
    /// the root of the lowered form of the document tree `id`, or `None` when
    /// the backend has none for it (or `id` names no tree).
    fn tree(&self, id: &str) -> Option<Box<dyn Node<'t> + 't>>;

    /// Mirrors the field [`LoweredTree::requires`](flm_core::stdcore::backends::lowering::LoweredTree::requires)
    /// of the lowered form of the document tree `id`: the identifiers of the
    /// definitions its output needs, in order. `None` as for
    /// [`tree`](LoweredTrees::tree).
    fn requires(&self, id: &str) -> Option<Vec<String>>;

    /// Mirrors the field [`LoweredTree::rounds`](flm_core::stdcore::backends::lowering::LoweredTree::rounds)
    /// of the lowered form of the document tree `id`: how many rounds of
    /// lowering ran over it, at least one. `None` as for
    /// [`tree`](LoweredTrees::tree).
    fn rounds(&self, id: &str) -> Option<u32>;
}

fn _object_safe(_: &dyn LoweredTrees<'_>) {}

/// Mirrors [`LoweringStage`](flm_core::stdcore::backends::lowering::LoweringStage):
/// the library's lowering stage for one backend, `flm.<backend>.lower`, a
/// stage computing per tree that lowers every frozen tree with the rules
/// the plan selected for the backend and writes the table
/// `flm.lowered.<backend>` ([`lowered_table_name`]). What differs: only its
/// constructor, which answers the stage as a [`LibraryStage`]; its
/// `backend`, `name` and `table` readers are not mirrored.
pub struct LoweringStage;

impl LoweringStage {
    /// Mirrors [`LoweringStage::new`](flm_core::stdcore::backends::lowering::LoweringStage::new):
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
        LibraryStage(Arc::new(flm_core::stdcore::backends::lowering::LoweringStage::new(backend)))
    }
}

/// Mirrors [`lowered_table_name`](flm_core::stdcore::backends::lowering::lowered_table_name):
/// the name of the table of the backend `backend`'s lowered trees,
/// `flm.lowered.<backend>`. What differs: the backend is a `&str`, and the
/// name an owned `String`.
pub fn lowered_table_name(backend: &str) -> String {
    flm_core::stdcore::backends::lowering::lowered_table_name(&flm_core::model::names::BackendName::from(backend)).to_string()
}
