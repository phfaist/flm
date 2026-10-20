//! The `index` feature: the element index, a table that lists, for every
//! frozen tree of a run, the nodes of that tree and the element each node
//! carries, so that a stage can find the nodes it works on without walking
//! the trees itself.
//!
//! The feature declares no command or environment; it serves the other stages
//! of the pipeline. Many of them look for "every node whose element carries
//! the aspect `A`" (an [aspect](crate::guide::concepts_overview#elements-and-aspects)
//! is a piece of behavior attached to an element): the scan of the
//! [`refpoints`](crate::stdcore::features::refpoints) feature, for instance,
//! looks for every node that declares a reference point. One stage of this
//! feature walks each tree once and records its nodes, and every other stage
//! filters that record instead of walking the tree again.
//!
//! # What the feature registers
//!
//! - **A stage.** [`BUILD`] (`flm.index.build`) computes the index, once per
//!   tree, and requires nothing. It walks the frozen tree in document order
//!   (the root, then its descendants in preorder) and records every node whose
//!   element value a feature registered, as the node's identifier together with
//!   the number the registry assigned to that element value
//!   ([`FlmRegistry::elem_idx`]).
//! - **A table.** [`ELEMENT_INDEX`] (`flm.element-index`), an
//!   [`ElementIndex`], is what the stage writes. A stage that reads the table
//!   requires the capability of the same name.
//!
//! The stage requires nothing because it indexes the tree the run froze,
//! whether its paragraphs and blocks were laid out or not; a stage that needs
//! the laid-out tree requires
//! [`flm.blocks`](crate::stdcore::features::blocks::BLOCKS) itself. A node
//! whose element value no feature registered (an element built by hand) is
//! not recorded.
//!
//! # Reading the index
//!
//! - [`ElementIndex::nodes_with`] returns the nodes of one tree whose element
//!   carries a given aspect: a stage of per-tree scope calls it.
//! - [`walk_nodes_with`] returns the same nodes for every frozen tree of the
//!   run, in the run's document order ([`StageCx::walk`]), the nodes of a
//!   placed tree where that tree is placed: a stage of document scope calls it.
//! - [`ElementIndex::values_of`] and [`walk_values_of`] are the same two reads
//!   filtered by the type of the element's per-value data instead of by an
//!   aspect; they test every recorded node with a downcast.
//!
//! Each of the four returns, per node found, the node's
//! [address](crate::model::addr::NodeAddr), the node itself, and the aspect
//! implementation or the per-value data.
//!
//! The table holds each tree's nodes apart, in tree order (the root, then its
//! descendants in preorder), because the place of a tree in the document is known only once the run
//! holds every tree. The two walks put the nodes of all trees in document
//! order, by the position the run gives each node ([`StageCx::position`]).
//!
//! # The index records elements, not aspects
//!
//! The nodes whose element carries the aspect `A` are found by filtering the
//! recorded list through the bit set the registry computes for `A` when it is
//! built; no list is kept per aspect. One list per aspect would take storage
//! in proportion to the number of aspects times the number of nodes, and the
//! indexing stage would need to know every aspect. The index never changes a
//! result: the [element](crate::model::element) carried by the node decides,
//! and walking the tree gives the same answer, so reading the index only saves
//! the walk.
//!
//! # Synthesized values
//!
//! A [synthesized value](crate::guide::concepts_overview#synthesized-values)
//! is a small tree that a stage builds during a run. When a consumer brings
//! one up to date ([value catch-up](crate::guide::concepts_overview#value-catch-up)),
//! this stage runs again over that tree, and a stage reading the index there
//! reads the index of that tree.

// The reading helpers live here, beside the table, rather than on the stage
// context: they need the registry's aspect bit sets and the table, and a stage
// already holds both. See design rationale at [§flm-dd-dr:std-index] and
// [§flm-dd-dr:dm-element-index].

use std::any::Any;
use std::collections::BTreeMap;
use std::sync::Arc;

use techy::core::node::{NodeId, NodeRef, NodeTree};
use techy::serialize::SerialValue;

use crate::model::addr::{NodeAddr, TreeId};
use crate::model::element::{ElemIdx, element_of};
use crate::core::plan::PlanCx;
use crate::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry,
};
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::core::data::store::StageOut;
use crate::core::data::table::{ProjectCx, SingletonTable, Table, TableName};
use crate::lang::Flm;

/// The feature's registered name, `index`.
pub const FEATURE: &str = "index";

/// The name of the stage that builds the element index, `flm.index.build`.
pub const BUILD: &str = "flm.index.build";

/// The typed name of the element index table, `flm.element-index`, an
/// [`ElementIndex`]. The name is also the capability that a stage reading the
/// table requires.
pub const ELEMENT_INDEX: TableName<ElementIndex> = TableName::new("flm.element-index");

/// The element index: for every tree, every node whose element value a
/// feature registered, in tree order (the root, then its descendants in
/// preorder), each with the number
/// the registry assigned to its element value ([`FlmRegistry::elem_idx`]).
///
/// The table [`ELEMENT_INDEX`] holds one value of this type. A stage reads it
/// through [`ElementIndex::nodes_with`] or [`ElementIndex::values_of`] (one
/// tree), or through [`walk_nodes_with`] or [`walk_values_of`] (every tree of
/// the run, in document order). See the module documentation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ElementIndex {
    /// The indexed nodes of each tree, in tree order (the root, then its
    /// descendants in preorder), keyed by the tree's identifier.
    pub per_tree: BTreeMap<TreeId, Vec<(NodeId, ElemIdx)>>,
}

impl ElementIndex {
    /// The recorded nodes of the tree `tree`, in tree order, each with the
    /// number of its element value; empty for a tree the
    /// index does not hold.
    pub fn entries(&self, tree: TreeId) -> &[(NodeId, ElemIdx)] {
        self.per_tree.get(&tree).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Every recorded node of `tree` (the frozen tree whose identifier is
    /// `id`) whose element carries the aspect `A`, in tree order, each with the
    /// node's address and the aspect
    /// implementation.
    ///
    /// The aspect is given as a trait object in the type argument, as in
    /// `index.nodes_with::<dyn Referenceable>(registry, id, &tree)`.
    /// `registry` supplies the bit set of the elements that carry `A`; it must
    /// be the registry that numbered the elements. With another registry, the
    /// result can miss nodes, but every node it returns does carry `A`. A
    /// recorded node that `tree` does not
    /// hold is skipped. For every tree of the run, in document order, call
    /// [`walk_nodes_with`].
    pub fn nodes_with<'t, A: ?Sized + Any>(
        &self,
        registry: &FlmRegistry,
        id: TreeId,
        tree: &'t NodeTree<Flm>,
    ) -> impl Iterator<Item = (NodeAddr, NodeRef<'t, Flm>, &'t A)> {
        let bits = registry.aspect_bitset::<A>();
        self.entries(id).iter().filter_map(move |(node_id, idx)| {
            if !bits.contains(*idx) {
                return None;
            }
            let node = tree.get(*node_id)?;
            let aspect = element_of(&node).aspect::<A>()?;
            Some((NodeAddr::new(id, *node_id), node, aspect))
        })
    }

    /// Every recorded node of `tree` (the frozen tree whose identifier is
    /// `id`) whose element carries per-value data of type `V`, in tree order,
    /// each with the node's address and the per-value
    /// data, as in `index.values_of::<HeadingElement>(id, &tree)`.
    ///
    /// This is the counterpart of [`ElementIndex::nodes_with`] for a data
    /// type instead of an aspect. It tests every recorded node with a downcast
    /// rather than a bit set, so prefer [`ElementIndex::nodes_with`] where an
    /// aspect identifies the nodes wanted. For every tree of the run, in
    /// document order, call [`walk_values_of`].
    pub fn values_of<'t, V: Any + Send + Sync>(
        &self,
        id: TreeId,
        tree: &'t NodeTree<Flm>,
    ) -> impl Iterator<Item = (NodeAddr, NodeRef<'t, Flm>, &'t V)> {
        self.entries(id).iter().filter_map(move |(node_id, _)| {
            let node = tree.get(*node_id)?;
            let value = element_of(&node).value_as::<V>()?;
            Some((NodeAddr::new(id, *node_id), node, value))
        })
    }
}

impl Table for ElementIndex {
    fn absorb(&mut self, piece: ElementIndex) {
        self.per_tree.extend(piece.per_tree);
    }

    /// Projects the table as `{ "tree#0": [node references in the order the
    /// tree writes them], … }`.
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::Map(
            self.per_tree
                .iter()
                .map(|(tree, entries)| {
                    let nodes = entries
                        .iter()
                        .map(|(node, _)| cx.node_ref(NodeAddr::new(*tree, *node)))
                        .collect();
                    (tree.to_string(), SerialValue::List(nodes))
                })
                .collect(),
        )
    }
}

impl SingletonTable for ElementIndex {
    const NAME: &'static str = "flm.element-index";
}

/// Every recorded node of every frozen tree of the run whose element carries
/// the aspect `A`, in the run's document order, each with the node's address
/// and the aspect implementation, as in
/// `walk_nodes_with::<dyn Referenceable>(cx, index)`.
///
/// Document order is the order of the run's walk ([`StageCx::walk`]): the
/// nodes of a tree placed in another tree come where that tree is placed,
/// between the nodes of the tree that places it. A stage of document scope
/// calls this; a stage of per-tree scope calls [`ElementIndex::nodes_with`]
/// for its own tree.
pub fn walk_nodes_with<'a, A: ?Sized + Any>(
    cx: &StageCx<'a>,
    index: &'a ElementIndex,
) -> impl Iterator<Item = (NodeAddr, NodeRef<'a, Flm>, &'a A)> + 'a {
    let registry = cx.registry();
    let found = cx.trees().flat_map(|(id, tree)| index.nodes_with::<A>(registry, id, tree));
    in_document_order(cx, found)
}

/// Every recorded node of every frozen tree of the run whose element carries
/// per-value data of type `V`, in the run's document order, each with the
/// node's address and the per-value data, as in
/// `walk_values_of::<HeadingElement>(cx, index)`.
///
/// This is the counterpart of [`walk_nodes_with`] for a data type instead of
/// an aspect, as [`ElementIndex::values_of`] is of
/// [`ElementIndex::nodes_with`].
pub fn walk_values_of<'a, V: Any + Send + Sync>(
    cx: &StageCx<'a>,
    index: &'a ElementIndex,
) -> impl Iterator<Item = (NodeAddr, NodeRef<'a, Flm>, &'a V)> + 'a {
    let found = cx.trees().flat_map(|(id, tree)| index.values_of::<V>(id, tree));
    in_document_order(cx, found)
}

/// Sorts the nodes `found`, gathered tree after tree, into the run's document
/// order. The sort is stable, so the entries of one node keep their order.
fn in_document_order<'a, T>(
    cx: &StageCx<'a>,
    found: impl Iterator<Item = (NodeAddr, NodeRef<'a, Flm>, T)>,
) -> std::vec::IntoIter<(NodeAddr, NodeRef<'a, Flm>, T)> {
    // See decision rationale at [§flm-dd-dr:dm-run-order-position].
    // Every node of a tree of the run has a position; only a synthesized
    // value's nodes have none, and the stable sort keeps their order (first).
    let mut found: Vec<_> = found.collect();
    found.sort_by_key(|(addr, _, _)| cx.position(*addr));
    found.into_iter()
}

/// The `index` feature ([`FlmFeature`]): it registers the element index table
/// [`ELEMENT_INDEX`] and the stage [`BUILD`] that writes it. See the module
/// documentation.
///
/// The features [`refpoints`](crate::stdcore::features::refpoints) and
/// [`refs`](crate::stdcore::features::refs), among others, read the table, so
/// a registry that holds them must hold this feature too.
#[derive(Debug, Default, Clone, Copy)]
pub struct Index;

impl FlmFeature for Index {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Element index").docs(
            "The element index `flm.element-index`: for every tree, the nodes of the \
             tree and the element each carries, in document order, so that a stage finds \
             the nodes whose element carries an aspect without walking the tree.",
        )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.table(
            ELEMENT_INDEX,
            "For every tree, every node whose element a feature registered, in document \
             order, with the number the registry assigned to that element.",
        )?;
        b.stage(Arc::new(BuildIndex))?;
        Ok(())
    }
}

/// The [`Index`] feature, ready to be listed in a package entry
/// ([`PackageEntry`](crate::core::registry::PackageEntry)).
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Index)
}

/// The stage [`BUILD`] (`flm.index.build`): it walks the frozen tree once in
/// document order and writes the tree's part of the element index. See the
/// module documentation.
struct BuildIndex;

impl Stage for BuildIndex {
    fn decl(&self) -> StageDecl {
        StageDecl::new(BUILD, Effect::Compute, Scope::PerTree).writes(ELEMENT_INDEX).docs(
            "Walks the frozen tree once in document order and records every node whose \
             element a feature registered into `flm.element-index`.",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let (Some(id), Some(tree)) = (cx.tree_id(), cx.my_tree()) else { return };
        let registry = cx.registry();
        let root = tree.root();
        let entries: Vec<(NodeId, ElemIdx)> = std::iter::once(root)
            .chain(root.descendants())
            .filter_map(|node| registry.elem_idx(element_of(&node)).map(|idx| (node.id(), idx)))
            .collect();
        // The write is declared above, so the piece is always obtainable;
        // the runner refuses an undeclared write before a stage ever runs.
        if let Ok(index) = out.table(ELEMENT_INDEX) {
            index.per_tree.insert(id, entries);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stage_writes_the_index_per_tree_and_needs_nothing() {
        let decl = BuildIndex.decl();
        assert_eq!(decl.name, BUILD);
        assert_eq!(decl.effect, Effect::Compute);
        assert_eq!(decl.scope, Scope::PerTree);
        assert_eq!(decl.writes[0].name, ELEMENT_INDEX.name());
        assert_eq!(ElementIndex::NAME, ELEMENT_INDEX.name());
    }

    #[test]
    fn absorbing_pieces_keeps_every_tree() {
        let mut index = ElementIndex::default();
        let mut piece = ElementIndex::default();
        piece.per_tree.insert(TreeId::tree(1), Vec::new());
        index.per_tree.insert(TreeId::tree(0), Vec::new());
        index.absorb(piece);
        assert_eq!(index.per_tree.len(), 2);
        assert!(index.entries(TreeId::tree(2)).is_empty());
    }
}
