//! This module defines the `index` feature, which provides the *element index*.
//! The element index is a table that lists, for each frozen tree of a run, the
//! nodes of the tree together with their elements. A stage reads the element
//! index to find the nodes that it works on, without walking the trees itself.
//!
//! Many stages look for the nodes whose element carries a given
//! [aspect](crate::guide::concepts_overview#elements-and-aspects), that is, a
//! piece of behavior attached to an element. For instance, the scan stage of
//! the [`refpoints`](crate::stdlib::features::refpoints) feature looks for
//! every node whose element carries the
//! [`Referenceable`](crate::model::aspects::Referenceable) aspect. With the
//! element index, each tree is walked once, by the stage [`BUILD`] of the
//! `index` feature, and these stages filter the result.
//!
//! # What the feature registers
//!
//! The `index` feature offers no command or environment. It registers the
//! following items:
//!
//! - The table [`ELEMENT_INDEX`] (`flm.element-index`), whose value is an
//!   [`ElementIndex`].
//! - The stage [`BUILD`] (`flm.index.build`), which writes this table. The
//!   stage runs once for each tree. It records every node of the tree whose
//!   element is registered in the run's registry, in *tree order*: the root,
//!   then its descendants in preorder. A node whose element is not registered,
//!   for instance an element that a program created itself, is not recorded.
//!
//! # Reading the element index
//!
//! To read the element index in a stage, declare the read in the stage's
//! [`Stage::needs`] method with [`Needs::reads`]`(ELEMENT_INDEX)`. This
//! declaration requires the table's capability, which the stage [`BUILD`]
//! provides. Then obtain the table in the stage's [`Stage::run`] method with
//! [`StageCx::get`].
//!
//! The following functions find nodes in the element index:
//!
//! - [`ElementIndex::nodes_with`] returns the nodes of one tree whose element
//!   carries a given aspect. A stage that runs once for each tree calls this
//!   method for its own tree.
//! - [`walk_nodes_with`] returns the nodes of all trees of the run whose
//!   element carries a given aspect, in document order. A stage that runs once
//!   for the whole document calls this function.
//! - [`ElementIndex::values_of`] and [`walk_values_of`] are the same two reads,
//!   but they select the nodes by the type of the element's data instead of by
//!   an aspect.
//!
//! Each of these functions returns, for each node found, the node's
//! [address](crate::model::addr::NodeAddr), the node, and either the aspect
//! implementation or the element's data.
//!
//! The table stores the nodes of each tree separately, in tree order. The
//! [document
//! order](crate::guide::concepts_overview#documents-fragments-and-the-corpus)
//! is the order of [`StageCx::walk`]: when another tree is placed at a node,
//! the nodes of that tree come immediately after this node.
//! [`walk_nodes_with`] and [`walk_values_of`] sort the nodes of all trees into
//! document order.
//!
//! # Element numbers and aspects
//!
//! The element index stores the element number of each node
//! ([`FlmRegistry::elem_idx`]), and no separate list of nodes for each aspect.
//! To find the nodes with an aspect, [`ElementIndex::nodes_with`] tests each
//! element number against the set of elements that carry the aspect. This set
//! is computed when the registry is built ([`FlmRegistry::aspect_bitset`]).
//! Therefore the `index` feature does not depend on the aspects defined in
//! other features.
//!
//! A read of the element index finds the same nodes as a walk of the tree that
//! inspects the element of each node. Reading the element index only saves the
//! walk.
//!
//! # Synthesized values
//!
//! A [*synthesized
//! value*](crate::guide::concepts_overview#synthesized-values) is a small tree
//! that a stage builds during a run, outside the document's trees, for instance
//! the text displayed for a reference. Before a stage renders a synthesized
//! value, it calls [`StageCx::resolve_value`], which performs [value
//! catch-up](crate::guide::concepts_overview#value-catch-up): the stages that
//! run once for each tree and that the calling stage depends on run over the
//! synthesized value. The synthesized value then has the same kinds of computed
//! data as a tree of the document.
//!
//! The stage [`BUILD`] is among these stages if the calling stage depends on
//! it, directly or through other stages. This is the case, for instance, when
//! the calling stage reads [`ELEMENT_INDEX`]. The element index computed during
//! the catch-up contains only the nodes of the synthesized value.
//!
//! During the catch-up, a stage that runs over the synthesized value obtains
//! the identifier of the value with [`StageCx::tree_id`] and the value itself
//! with [`StageCx::my_tree`]. With these, [`ElementIndex::nodes_with`] returns
//! nodes of the synthesized value. [`walk_nodes_with`] and [`walk_values_of`]
//! return only nodes of the document's trees, never nodes of a synthesized
//! value.

// The reading helpers live here, beside the table, rather than on the stage
// context: they need the registry's aspect bit sets and the table, and a stage
// already holds both.

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

/// The name of the element index table, `flm.element-index`, whose value is an
/// [`ElementIndex`]. To read the table in a stage, declare the read with
/// [`Needs::reads`]`(ELEMENT_INDEX)`.
pub const ELEMENT_INDEX: TableName<ElementIndex> = TableName::new("flm.element-index");

/// An `ElementIndex` lists the nodes of each frozen tree of a run, together
/// with the element number of each node. It is the value of the table
/// [`ELEMENT_INDEX`].
///
/// For each tree, the list contains every node whose element is registered in
/// the registry, in tree order: the root, then its descendants in preorder. The
/// element number of a node is the number of the node's element in the
/// registry ([`FlmRegistry::elem_idx`]).
///
/// To find nodes in the element index, call [`ElementIndex::nodes_with`] or
/// [`ElementIndex::values_of`] for one tree, or [`walk_nodes_with`] or
/// [`walk_values_of`] for all trees of the run, in document order. The [module
/// documentation](crate::stdlib::features::index) explains how a stage reads
/// the element index.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ElementIndex {
    /// The list of indexed nodes of each tree, keyed by the identifier of the
    /// tree. Each list is in tree order, and contains each node with its
    /// element number.
    pub per_tree: BTreeMap<TreeId, Vec<(NodeId, ElemIdx)>>,
}

impl ElementIndex {
    /// Returns the indexed nodes of the tree `tree`, in tree order, each with
    /// its element number. Returns an empty slice if the element index contains
    /// no list for `tree`.
    pub fn entries(&self, tree: TreeId) -> &[(NodeId, ElemIdx)] {
        self.per_tree.get(&tree).map(Vec::as_slice).unwrap_or(&[])
    }

    /// Returns the indexed nodes of one tree whose element carries the aspect
    /// `A`, in tree order.
    ///
    /// `id` is the identifier of the tree, and `tree` is the frozen tree
    /// itself. The aspect is given as a trait-object type, as in
    /// `index.nodes_with::<dyn Referenceable>(registry, id, &tree)`. Each item
    /// contains the address of the node, the node, and the element's
    /// implementation of `A`.
    ///
    /// `registry` must be the registry of the run ([`StageCx::registry`]),
    /// because the element numbers in the element index are numbers of that
    /// registry. With another registry, the result can miss nodes.
    ///
    /// To find the nodes of all trees of the run, in document order, call
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

    /// Returns the indexed nodes of one tree whose element has data of type
    /// `V`, in tree order.
    ///
    /// Same as [`ElementIndex::nodes_with`], but selects the nodes by the type
    /// of the element's data
    /// ([`ElementSpec::value_as`](crate::model::element::ElementSpec::value_as))
    /// instead of by an aspect, as in `index.values_of::<HeadingElement>(id,
    /// &tree)`. Each item contains the address of the node, the node, and the
    /// element's data.
    ///
    /// This method inspects the element of every indexed node of the tree,
    /// whereas [`ElementIndex::nodes_with`] tests the element number against a
    /// precomputed set. Use [`ElementIndex::nodes_with`] where an aspect
    /// identifies the nodes that you want. To find the nodes of all trees of
    /// the run, in document order, call [`walk_values_of`].
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

    /// Serializes the element index as a map from each tree identifier, such as
    /// `tree#0`, to the list of the indexed nodes of the tree, in tree order.
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

/// Returns the indexed nodes of all frozen trees of the run whose element
/// carries the aspect `A`, in document order.
///
/// The aspect is given as a trait-object type, as in `walk_nodes_with::<dyn
/// Referenceable>(cx, index)`. Each item contains the address of the node, the
/// node, and the element's implementation of `A`.
///
/// Document order is the order of [`StageCx::walk`]: when another tree is
/// placed at a node, the nodes of that tree come immediately after this node. A
/// stage that runs once for the whole document calls this function. A stage
/// that runs once for each tree calls [`ElementIndex::nodes_with`] for its own
/// tree.
pub fn walk_nodes_with<'a, A: ?Sized + Any>(
    cx: &StageCx<'a>,
    index: &'a ElementIndex,
) -> impl Iterator<Item = (NodeAddr, NodeRef<'a, Flm>, &'a A)> + 'a {
    let registry = cx.registry();
    let found = cx.trees().flat_map(|(id, tree)| index.nodes_with::<A>(registry, id, tree));
    in_document_order(cx, found)
}

/// Returns the indexed nodes of all frozen trees of the run whose element has
/// data of type `V`, in document order.
///
/// Same as [`walk_nodes_with`], but selects the nodes by the type of the
/// element's data instead of by an aspect, as in
/// `walk_values_of::<HeadingElement>(cx, index)`. Each item contains the
/// address of the node, the node, and the element's data. A stage that runs
/// once for each tree calls [`ElementIndex::values_of`] for its own tree.
pub fn walk_values_of<'a, V: Any + Send + Sync>(
    cx: &StageCx<'a>,
    index: &'a ElementIndex,
) -> impl Iterator<Item = (NodeAddr, NodeRef<'a, Flm>, &'a V)> + 'a {
    let found = cx.trees().flat_map(|(id, tree)| index.values_of::<V>(id, tree));
    in_document_order(cx, found)
}

/// Sorts the items `found`, gathered from several trees, into document order by
/// the position of each node ([`StageCx::position`]). The sort is stable: items
/// with the same position keep their order in `found`.
fn in_document_order<'a, T>(
    cx: &StageCx<'a>,
    found: impl Iterator<Item = (NodeAddr, NodeRef<'a, Flm>, T)>,
) -> std::vec::IntoIter<(NodeAddr, NodeRef<'a, Flm>, T)> {
    // A per-tree table holds no place in the document, so the entries gathered
    // from several trees are sorted by the position the run gives each node in
    // its one walk of the document.
    // Every node of a tree of the run has a position; only a synthesized
    // value's nodes have none, and the stable sort keeps their order (first).
    let mut found: Vec<_> = found.collect();
    found.sort_by_key(|(addr, _, _)| cx.position(*addr));
    found.into_iter()
}

/// The `index` feature, which registers the table [`ELEMENT_INDEX`] and the
/// stage [`BUILD`] that writes this table. The [module
/// documentation](crate::stdlib::features::index) explains how a stage reads
/// the element index.
///
/// The table [`ELEMENT_INDEX`] is read by stages of several features, among
/// them [`refpoints`](crate::stdlib::features::refpoints) and
/// [`refs`](crate::stdlib::features::refs). A registry that contains one of
/// these features must also contain the `index` feature.
#[derive(Debug, Default, Clone, Copy)]
pub struct Index;

impl FlmFeature for Index {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Element index").docs(
            "The element index (table `flm.element-index`), which lists the nodes of each \
             tree together with their elements. Stages read it to find the nodes whose \
             element carries a given aspect, without walking the trees.",
        )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.table(
            ELEMENT_INDEX,
            "For each tree, the nodes whose element is registered, in tree order, each \
             with the number of its element in the registry.",
        )?;
        b.stage(Arc::new(BuildIndex))?;
        Ok(())
    }
}

/// Returns the [`Index`] feature as a shared feature object, for instance to
/// list it in a [`PackageEntry`](crate::core::registry::PackageEntry).
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Index)
}

/// The stage [`BUILD`] (`flm.index.build`), which writes the element index. The
/// stage runs once for each tree. It walks the tree once and records each node
/// whose element is registered in the registry, in tree order.
struct BuildIndex;

impl Stage for BuildIndex {
    fn decl(&self) -> StageDecl {
        StageDecl::new(BUILD, Effect::Compute, Scope::PerTree).writes(ELEMENT_INDEX).docs(
            "Records in `flm.element-index` each node of the tree whose element is \
             registered, in tree order.",
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
