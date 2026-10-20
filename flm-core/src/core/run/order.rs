//! The run's document order: the document's walk applied to the run's frozen
//! trees, computed once when the rewrite stages are over.
//!
//! Document order is defined once, by the document's walk
//! ([`DocContent::walk`](crate::model::doc::DocContent::walk)): the listed trees
//! in order, each tree entered once, and a placed tree (the tree of an included
//! file that opens with a heading) entered at the node that places it, its nodes
//! following that node immediately and coming before the node's own children.
//! The rewrite stages produce new trees, with new node identities, so the run
//! applies the walk to the frozen trees, the trees as the last rewrite stage
//! left them, and records three things:
//!
//! - **the placements of the run's trees**: which node of a frozen tree places
//!   which tree. The document records each placement by the placing node as
//!   parsed; the run finds that node again in the frozen tree by the source
//!   span it was written at, which every rewrite stage keeps;
//! - **the run's order of the trees**: the order in which the walk enters them;
//! - **the walk itself**: every node of those trees in document order, and each
//!   node's position in it ([`DocPosition`]).
//!
//! The run's trees are the trees that the walk reaches. A tree that the walk
//! does not reach is not a tree of the run: no later stage reads it and no
//! output contains it, and its content is dropped together with the node that
//! placed it. This happens when a rewrite stage removed the placing node (for
//! example, a user definition whose template does not use the argument that
//! held the `\input`), or when the tree that includes it failed its rewrite.
//!
//! A rewrite stage may also copy a placing node, so that it appears at two
//! places of the frozen trees. The first copy in document order then places the
//! tree, and the run reports [`PlacedTwice`] at every other copy.
//!
//! A stage reads this order through its context: [`StageCx::walk`],
//! [`StageCx::trees`], [`StageCx::position`] and [`StageCx::placement_of`]. An
//! embedding program reads the run's order of the trees with
//! [`Run::tree_order`].
//!
//! [`StageCx::walk`]: crate::core::run::StageCx::walk
//! [`StageCx::trees`]: crate::core::run::StageCx::trees
//! [`StageCx::position`]: crate::core::run::StageCx::position
//! [`StageCx::placement_of`]: crate::core::run::StageCx::placement_of
//! [`Run::tree_order`]: crate::core::run::Run::tree_order

// The walk is not implemented here: the run is a `WalkSource` for its frozen
// trees and the placements it found again, and `Walk` walks it as it walks the
// document's trees as parsed; the run records the steps.

use std::collections::BTreeMap;
use std::sync::Arc;

use techy::core::node::NodeTree;
use techy::error::{Diagnostic, DiagnosticInfo};

use crate::lang::input::is_placing;
use crate::lang::{Flm, FlmDiagnostics};
use crate::model::addr::{NodeAddr, TreeId};
use crate::model::doc::{DocContent, Walk, WalkSource, WalkStep};

/// A node's position in the run's document order: of two nodes of the run's
/// trees, the one with the smaller position comes first in the document.
///
/// Positions compare nodes of one run only and mean nothing outside it.
/// [`StageCx::position`](crate::core::run::StageCx::position) returns the
/// position of a node of a frozen tree; a document-scope stage (a stage of
/// [`Scope::Document`](crate::core::stage::Scope::Document), run once for the
/// whole document) that gathers nodes from several trees sorts them by
/// position to put them in document order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocPosition(usize);

/// Diagnostic condition `flm.run.placed-twice`, reported at Error severity: a
/// node that places a tree of the document appears at two places of the run's
/// trees, because a rewrite stage copied it.
///
/// This happens when a user definition's template writes twice the argument
/// that holds the `\input` of a file that opens with a heading. A tree is
/// placed by one node only: the first copy in document order places it, and
/// the run reports this condition at each other copy, which places nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.run.placed-twice",
    message = "a file that opens with a heading can be included only once: `{file}` is \
               included here a second time, because a macro or environment definition uses the \
               argument that holds its `\\input` more than once"
)]
pub struct PlacedTwice {
    /// The canonical path of the placed tree's file, or the tree's identifier
    /// when its source has no file path.
    pub file: String,
}

/// The run's document order. See the [module documentation](self).
#[derive(Debug, Default)]
pub(crate) struct RunOrder {
    /// The tree each placing node of the run's trees places, by the node's
    /// address in its frozen tree.
    placements: BTreeMap<NodeAddr, TreeId>,
    /// The trees in the order the walk enters them.
    trees: Vec<TreeId>,
    /// Every node of `trees`, in document order.
    walk: Vec<NodeAddr>,
    /// For each tree, the position in `walk` of each node, by the node's index
    /// in its tree's storage; `usize::MAX` for a node the walk does not visit.
    positions: BTreeMap<TreeId, Vec<usize>>,
}

/// The run's trees as the walk's source: the document's listed trees, the
/// frozen trees, and the placements the run found again in them.
struct FrozenSource<'a> {
    listed: &'a [TreeId],
    frozen: &'a BTreeMap<TreeId, Arc<NodeTree<Flm>>>,
    placements: &'a BTreeMap<NodeAddr, TreeId>,
}

impl WalkSource for FrozenSource<'_> {
    fn listed(&self) -> &[TreeId] {
        self.listed
    }

    fn node_tree(&self, tree: TreeId) -> Option<&NodeTree<Flm>> {
        self.frozen.get(&tree).map(|tree| &**tree)
    }

    fn placement(&self, site: NodeAddr) -> Option<TreeId> {
        self.placements.get(&site).copied()
    }
}

impl RunOrder {
    /// The order of the frozen trees `frozen` of the document `content`, and
    /// the diagnostics of a tree placed twice.
    pub(crate) fn new(
        content: &DocContent,
        frozen: &BTreeMap<TreeId, Arc<NodeTree<Flm>>>,
    ) -> (RunOrder, FlmDiagnostics) {
        let mut diags = FlmDiagnostics::new();
        let placements = placements(content, frozen, &mut diags);
        let source = FrozenSource { listed: content.traversal(), frozen, placements: &placements };
        let mut trees = Vec::new();
        let mut walk = Vec::new();
        let mut positions: BTreeMap<TreeId, Vec<usize>> = BTreeMap::new();
        for step in Walk::new(&source) {
            match step {
                WalkStep::EnterTree(tree) => {
                    trees.push(tree);
                    positions.insert(tree, vec![usize::MAX; frozen[&tree].node_count()]);
                }
                WalkStep::Node(addr, _) => {
                    if let Some(slot) =
                        positions.get_mut(&addr.tree).and_then(|p| p.get_mut(addr.node.index()))
                    {
                        *slot = walk.len();
                    }
                    walk.push(addr);
                }
                WalkStep::ExitTree(_) => {}
            }
        }
        (RunOrder { placements, trees, walk, positions }, diags)
    }

    /// Whether the walk reaches the tree `tree`, that is whether it is a tree
    /// of the run.
    pub(crate) fn has_tree(&self, tree: TreeId) -> bool {
        self.positions.contains_key(&tree)
    }

    /// The trees in the order the walk enters them.
    pub(crate) fn trees(&self) -> &[TreeId] {
        &self.trees
    }

    /// Every node of the run's trees, in document order.
    pub(crate) fn walk(&self) -> &[NodeAddr] {
        &self.walk
    }

    /// The position of a node of the run's trees in document order, or `None`
    /// for a node the walk does not visit.
    pub(crate) fn position(&self, addr: NodeAddr) -> Option<DocPosition> {
        let position = *self.positions.get(&addr.tree)?.get(addr.node.index())?;
        (position != usize::MAX).then_some(DocPosition(position))
    }

    /// The tree a node of the run's trees places, if it places one.
    pub(crate) fn placement_of(&self, addr: NodeAddr) -> Option<TreeId> {
        self.placements.get(&addr).copied()
    }
}

/// The placements of the run's trees: for each placement of the document, the
/// node of the frozen tree that places the tree — a placing node written at
/// the same place as the placing node as parsed. A second such node is
/// reported, and places nothing.
fn placements(
    content: &DocContent,
    frozen: &BTreeMap<TreeId, Arc<NodeTree<Flm>>>,
    diags: &mut FlmDiagnostics,
) -> BTreeMap<NodeAddr, TreeId> {
    // The placing nodes of each frozen tree that places anything, in the order
    // they are written.
    let mut placing: BTreeMap<TreeId, Vec<NodeAddr>> = BTreeMap::new();
    for site in content.resolution().keys() {
        let Some(tree) = frozen.get(&site.tree) else { continue };
        placing.entry(site.tree).or_insert_with(|| {
            let root = tree.root();
            std::iter::once(root)
                .chain(root.descendants())
                .filter(is_placing)
                .map(|node| NodeAddr::new(site.tree, node.id()))
                .collect()
        });
    }
    let mut found = BTreeMap::new();
    for (site, target) in content.resolution() {
        let Some(parsed) = content.tree(site.tree).and_then(|tree| tree.get(site.node)) else {
            continue;
        };
        let (Some(tree), Some(candidates)) = (frozen.get(&site.tree), placing.get(&site.tree))
        else {
            continue;
        };
        // See decision rationale at [§flm-dd-dr:std-input-placing-node-span].
        let mut copies = candidates.iter().filter_map(|addr| {
            let node = tree.get(addr.node)?;
            (node.span() == parsed.span()).then_some((*addr, node))
        });
        let Some((first, _)) = copies.next() else { continue };
        found.insert(first, *target);
        for (_, node) in copies {
            let file = content
                .fragment(*target)
                .and_then(|fragment| fragment.source().origin().canonical_path().map(String::from))
                .unwrap_or_else(|| target.to_string());
            diags.push(Diagnostic::error(PlacedTwice { file }, node.span().clone()));
        }
    }
    found
}
