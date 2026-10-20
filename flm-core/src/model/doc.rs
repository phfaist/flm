//! Documents and fragments: the parsed source a document is made of, how its
//! trees are placed, and the walk that defines document order.
//!
//! A fragment is one piece of source as the parser delivered it (its tree, its
//! source, its parse diagnostics); a tree is one placement of a fragment's node
//! tree in a document, numbered by the document's arena; a document is its
//! content (the tree arena, the traversal list and the resolution map) and the
//! option values bound on it, and nothing else. See [documents, fragments, and
//! the corpus](crate::guide::concepts_overview#documents-fragments-and-the-corpus).
//!
//! A [`Fragment`] also records how the content at its top level is laid out
//! ([`FragmentLayout`]). It is shared as `Arc<Fragment>` and has no identifier
//! of its own: its identity is the shared pointer, so one fragment may be used
//! by several documents at once.
//!
//! A document ([`Doc`]) holds its content ([`DocContent`]) and the option values
//! set on it ([`OptionValues`]). The content has exactly three parts:
//!
//! - the **tree arena**, which numbers every tree added to the document with a
//!   [`TreeId`] in the order the trees are added; adding one `Arc<Fragment>`
//!   twice makes two trees, each with its own computed data;
//! - the **traversal list**, the trees the walk visits in their own right, in
//!   order;
//! - the **resolution map**, which records the node at which each placed tree
//!   is entered: the node *places* that tree.
//!
//! A document has no structure beyond that: no front matter, no chapters, no
//! named parts. The structure of a document comes from the headings in its
//! content and is computed by a stage, so that there is only one account of how
//! a document nests.
//!
//! The usual way to build a document's content is [`assemble`], which parses a
//! primary file and the files it includes; [`DocContent::place`],
//! [`DocContent::add_tree`] and [`DocContent::resolve`] build one tree by tree.
//!
//! # The walk
//!
//! [`DocContent::walk`] defines document order, and it is the only definition
//! the library has. The walk visits the trees of the traversal list in order,
//! enters each tree at most once, and enters a tree that a node places at that
//! node: the placed tree's nodes come right after the placing node, before the
//! placing node's own children. Everything that comes before the placed tree in
//! document order is therefore the placing node and everything that comes
//! before it. Every notion of "earlier in the document" (numbering, the order of
//! notes, the order of citations) uses this one operation, so a placed tree
//! needs no special handling anywhere else. [`DocContent::walk_nodes`] and
//! [`DocContent::walk_trees`] keep the nodes alone and the trees alone.
//!
//! The walk ([`Walk`]) runs over any [`WalkSource`]: something that gives trees
//! by identifier, the trees of its traversal list, and which node places which
//! tree. [`DocContent`] is the source of the trees as parsed; a run is the
//! source of its frozen trees, what each tree became after the rewrites
//! ([`StageCx::walk`](crate::core::run::StageCx::walk)). Both use [`Walk`], so
//! document order is the same for both.
//!
//! Document order is neither the order of the arena nor the order in which node
//! addresses sort; see [`crate::model::addr`].
//!
//! # What a document does not hold
//!
//! The data that stages compute, and the diagnostics they report, belong to
//! the run that computed them, not to the document: one document may be
//! rendered to several formats, or checked and then rendered, and nothing
//! about it may depend on which plan runs. The diagnostics of the parse are
//! kept in the fragment ([`Fragment::diags`]). Synthesized values (small trees
//! a stage builds while a run is under way) are not part of the arena either;
//! the run numbers them (see [`TreeId`]).
//!
//! [`assemble`]: crate::model::assemble::assemble

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use techy::core::node::{Descendants, NodeRef, NodeTree};

use crate::model::addr::{NodeAddr, TreeId};
use crate::model::options::OptionValues;
use crate::lang::flm::Flm;
use crate::lang::origin::{FlmDiagnostics, FlmSource};

/// How the content at the top level of a fragment is laid out when a run
/// builds block structure: as blocks, as inline content, or as blocks only
/// when it holds a block.
///
/// The block-structure rewrite ([`crate::lang::rewrites::blocks`]) reads it.
/// It is recorded on the fragment rather than on the document because it is a
/// property of the text: a title is inline content in every document that
/// uses it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FragmentLayout {
    /// The top level holds blocks: paragraphs are built from its content.
    Blocks,
    /// Everything at the top level is inline content. An element whose block
    /// role is not inline is reported as an error there, and is left where it
    /// is written.
    Inline,
    /// Inline content, unless the top level holds a paragraph break or an
    /// element whose block role is
    /// [`Block`](crate::model::aspects::BlockRole::Block); then the whole top
    /// level is laid out as blocks. This is the default.
    #[default]
    InlineOrBlocks,
}

/// A fragment: one piece of source as the parser delivered it, with its tree,
/// the source it came from, the diagnostics of the parse, and the layout of its
/// top level ([`FragmentLayout`]).
///
/// The tree is exactly what the parse delivered, source comments included.
/// Expanding user definitions and building block structure are rewrite stages
/// of a run, and the trees they produce are stored by the run, apart from the
/// fragment. Because the fragment keeps the parse as written, one shared
/// fragment can be used by several documents with different configurations,
/// and a tool can recover the author's own text from it.
///
/// A fragment is shared as `Arc<Fragment>` and has no identifier: its
/// identity is the pointer, and within one document each placement of it is
/// identified by a [`TreeId`]. Create one with
/// [`parse_fragment`](crate::lang::parse_fragment), or with [`Fragment::new`]
/// from a tree obtained some other way.
pub struct Fragment {
    source: Arc<FlmSource>,
    tree: Arc<NodeTree<Flm>>,
    diags: FlmDiagnostics,
    layout: FragmentLayout,
}

impl Fragment {
    /// A fragment holding the parsed tree `tree`, the source it was parsed
    /// from, the diagnostics of the parse and the layout of its top level.
    ///
    /// [`parse_fragment`](crate::lang::parse_fragment) is the usual way to
    /// obtain a fragment; this constructor is for a tree built or read back
    /// from storage elsewhere. No check is made that the parts belong
    /// together.
    pub fn new(
        source: Arc<FlmSource>,
        tree: Arc<NodeTree<Flm>>,
        diags: FlmDiagnostics,
        layout: FragmentLayout,
    ) -> Fragment {
        Fragment { source, tree, diags, layout }
    }

    /// The source the tree was parsed from; its origin (such as a file name)
    /// is what diagnostics report.
    pub fn source(&self) -> &Arc<FlmSource> {
        &self.source
    }

    /// The tree, exactly as the parse produced it.
    pub fn tree(&self) -> &Arc<NodeTree<Flm>> {
        &self.tree
    }

    /// The diagnostics the parse produced. Diagnostics reported later, while
    /// stages run, are kept by the run instead.
    pub fn diags(&self) -> &FlmDiagnostics {
        &self.diags
    }

    /// How the top level is laid out when block structure is built.
    pub fn layout(&self) -> FragmentLayout {
        self.layout
    }
}

impl fmt::Debug for Fragment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Fragment")
            .field("source", &self.source.origin())
            .field("nodes", &self.tree.node_count())
            .field("diags", &self.diags.len())
            .field("layout", &self.layout)
            .finish()
    }
}

/// The error of [`DocContent::resolve`]: why a placement was not recorded.
///
/// Each variant holds the trees and nodes involved, so a caller can report the
/// problem without computing them again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacementError {
    /// Another node already places the target tree. The walk enters each tree
    /// once, so one node at most may place a tree.
    AlreadyPlaced {
        /// The tree that would have been placed twice.
        target: TreeId,
        /// The node that already places it.
        first: NodeAddr,
        /// The node that tried to place it as well.
        second: NodeAddr,
    },
    /// The node already places a tree. A node may place one tree at most.
    SiteTaken {
        /// The node that would have placed a second tree.
        site: NodeAddr,
        /// The tree it already places.
        target: TreeId,
    },
    /// The identifier is not that of a tree of this document: it identifies a
    /// synthesized value, or its index is past the end of the arena. Returned
    /// for the target tree and for the tree of the placing node alike.
    UnknownTree(TreeId),
    /// The placing node belongs to the target tree itself, and the walk could
    /// not enter a tree from inside itself.
    SelfPlacement(TreeId),
}

impl fmt::Display for PlacementError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlacementError::AlreadyPlaced { target, first, second } => write!(
                f,
                "a tree can be placed at one node only: {target} is already placed at \
                 {first:?} and cannot also be placed at {second:?}"
            ),
            PlacementError::SiteTaken { site, target } => {
                write!(f, "a node can place one tree only: {site:?} already places {target}")
            }
            PlacementError::UnknownTree(tree) => write!(f, "{tree} is not a tree of this document"),
            PlacementError::SelfPlacement(tree) => write!(f, "{tree} cannot be placed at one of its own nodes"),
        }
    }
}

impl std::error::Error for PlacementError {}

/// The content of a document: its tree arena, its traversal list and its
/// resolution map.
///
/// It is built once, by adding trees and recording placements, and then only
/// read: several runs may share one document, so nothing about it changes while
/// a run is under way. The [module documentation](self) describes the three
/// parts and the walk that they define.
///
/// [`assemble`](crate::model::assemble::assemble) builds the content of a
/// document from its files; [`DocContent::place`], [`DocContent::add_tree`]
/// and [`DocContent::resolve`] build it tree by tree; [`Doc::new`] and
/// [`Doc::with_options`] make a document of it.
#[derive(Default)]
pub struct DocContent {
    arena: Vec<Arc<Fragment>>,
    traversal: Vec<TreeId>,
    resolution: BTreeMap<NodeAddr, TreeId>,
    placed: BTreeMap<TreeId, NodeAddr>,
}

impl DocContent {
    /// Content with no tree.
    pub fn new() -> DocContent {
        DocContent::default()
    }

    /// Adds a tree for `fragment` to the arena and appends it to the traversal
    /// list, so that the walk visits it after the trees listed before it;
    /// returns the tree's [`TreeId`].
    ///
    /// The same `Arc<Fragment>` added twice gives two distinct trees with
    /// separate computed data, because each tree is a separate placement in the
    /// document.
    ///
    /// See [`DocContent::add_tree`] for a tree that the walk reaches through a
    /// node that places it, rather than through the traversal list.
    ///
    /// # Panics
    ///
    /// Panics under the same condition as [`DocContent::add_tree`].
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Arc;
    ///
    /// use flm_core::model::doc::{DocContent, Fragment, FragmentLayout};
    /// use flm_core::lang::{
    ///     build_language, parse_fragment, FlmOrigin, LangInputs, ParseOptions,
    /// };
    /// use flm_core::techy::source::Source;
    ///
    /// let lang = build_language(LangInputs::new());
    /// let source = Arc::new(Source::new("hello").with_origin(FlmOrigin::none()));
    /// let fragment = parse_fragment(
    ///     &lang,
    ///     source,
    ///     FragmentLayout::default(),
    ///     &ParseOptions::default(),
    /// )?;
    ///
    /// let mut content = DocContent::new();
    /// let shared = Arc::new(fragment);
    /// let first = content.place(Arc::clone(&shared));
    /// let second = content.place(shared);
    /// assert_ne!(first, second);
    /// assert_eq!(content.walk_trees().collect::<Vec<_>>(), [first, second]);
    /// # Ok::<(), flm_core::lang::FragmentError>(())
    /// ```
    pub fn place(&mut self, fragment: Arc<Fragment>) -> TreeId {
        let id = self.add_tree(fragment);
        self.traversal.push(id);
        id
    }

    /// Adds a tree for `fragment` to the arena without appending it to the
    /// traversal list; returns the tree's [`TreeId`].
    ///
    /// Use this for a tree that the walk is to reach through a node that places
    /// it ([`DocContent::resolve`]). A tree that is neither listed nor placed
    /// is part of the document but is never walked.
    ///
    /// # Panics
    ///
    /// Panics if the arena already holds 2³¹ trees, the largest number of
    /// trees a [`TreeId`] can tell apart from synthesized values.
    pub fn add_tree(&mut self, fragment: Arc<Fragment>) -> TreeId {
        // The bound is `TreeId::tree`'s own (the tag bit): checked here so the
        // message names the real limit.
        let index = u32::try_from(self.arena.len())
            .ok()
            .filter(|index| index >> 31 == 0)
            .expect("a document holds fewer than 2^31 trees");
        self.arena.push(fragment);
        TreeId::tree(index)
    }

    /// Records that the node `site` places the tree `target`: the walk enters
    /// `target` right after it reaches `site`.
    ///
    /// `site` is an address in a tree **as parsed**. Several runs with
    /// different rewrites may share one document, and a rewrite produces a new
    /// tree with new node identifiers, so an address taken after the rewrites
    /// would hold for one run at most; the resolution map of a document
    /// therefore uses the node identifiers that the parse produced.
    ///
    /// [`assemble`](crate::model::assemble::assemble) places the tree of every
    /// included file that opens with a heading at the node of its `\input`
    /// command. A program that embeds the library may also use this to build a
    /// document from several fragments.
    ///
    /// # Errors
    ///
    /// Returns a [`PlacementError`], and records nothing, when `target` or
    /// `site.tree` is not a tree of this document
    /// ([`UnknownTree`](PlacementError::UnknownTree)), when `site` belongs to
    /// `target` itself ([`SelfPlacement`](PlacementError::SelfPlacement)),
    /// when another node already places `target`
    /// ([`AlreadyPlaced`](PlacementError::AlreadyPlaced)), or when `site`
    /// already places a tree ([`SiteTaken`](PlacementError::SiteTaken)).
    pub fn resolve(&mut self, site: NodeAddr, target: TreeId) -> Result<(), PlacementError> {
        if self.fragment(target).is_none() {
            return Err(PlacementError::UnknownTree(target));
        }
        if self.fragment(site.tree).is_none() {
            return Err(PlacementError::UnknownTree(site.tree));
        }
        if site.tree == target {
            return Err(PlacementError::SelfPlacement(target));
        }
        if let Some(&first) = self.placed.get(&target) {
            return Err(PlacementError::AlreadyPlaced { target, first, second: site });
        }
        if let Some(&existing) = self.resolution.get(&site) {
            return Err(PlacementError::SiteTaken { site, target: existing });
        }
        self.resolution.insert(site, target);
        self.placed.insert(target, site);
        Ok(())
    }

    /// The fragment of the tree `tree` of this document.
    ///
    /// Returns `None` when `tree` identifies a synthesized value or its index
    /// is past the end of the arena.
    pub fn fragment(&self, tree: TreeId) -> Option<&Arc<Fragment>> {
        if tree.is_value() {
            return None;
        }
        self.arena.get(tree.index() as usize)
    }

    /// The node tree of the tree `tree` of this document: the tree of its
    /// [`fragment`](DocContent::fragment).
    ///
    /// Returns `None` under the same conditions as
    /// [`DocContent::fragment`].
    pub fn tree(&self, tree: TreeId) -> Option<&Arc<NodeTree<Flm>>> {
        self.fragment(tree).map(|fragment| fragment.tree())
    }

    /// Every tree of the arena with its fragment, in the order the trees were
    /// added. This is not document order; [`DocContent::walk_trees`] gives
    /// the trees in document order.
    pub fn trees(&self) -> impl Iterator<Item = (TreeId, &Arc<Fragment>)> + '_ {
        self.arena
            .iter()
            .enumerate()
            .map(|(index, fragment)| (TreeId::tree(index as u32), fragment))
    }

    /// The number of trees in the arena, walked or not.
    pub fn len(&self) -> usize {
        self.arena.len()
    }

    /// Returns `true` when the arena holds no tree.
    pub fn is_empty(&self) -> bool {
        self.arena.is_empty()
    }

    /// The traversal list: the trees the walk visits in their own right, in
    /// order. A tree that the walk reaches only through a node that places it
    /// is not in this list.
    pub fn traversal(&self) -> &[TreeId] {
        &self.traversal
    }

    /// The resolution map: for each node that places a tree, the tree it
    /// places. The keys are addresses in the trees as parsed.
    pub fn resolution(&self) -> &BTreeMap<NodeAddr, TreeId> {
        &self.resolution
    }

    /// The tree that the node `site` places, or `None` when it places none.
    pub fn placement_of(&self, site: NodeAddr) -> Option<TreeId> {
        self.resolution.get(&site).copied()
    }

    /// The node that places the tree `target`, or `None` when no node places
    /// it.
    pub fn placed_at(&self, target: TreeId) -> Option<NodeAddr> {
        self.placed.get(&target).copied()
    }

    /// The walk over the trees as parsed: the steps of the traversal in
    /// document order.
    ///
    /// The iterator yields a [`WalkStep`] when it enters a tree and when it
    /// leaves it, and one for every node in between, in pre-order (a node
    /// before its children), with the node's address. This is the library's
    /// definition of document order; see the [module
    /// documentation](self#the-walk) for what that order is.
    ///
    /// [`DocContent::walk_nodes`] and [`DocContent::walk_trees`] give the
    /// nodes alone and the trees alone.
    pub fn walk(&self) -> Walk<'_> {
        Walk::new(self)
    }

    /// The nodes of the walk: every node of every walked tree, in document
    /// order, with its address.
    pub fn walk_nodes(&self) -> impl Iterator<Item = (NodeAddr, NodeRef<'_, Flm>)> + '_ {
        self.walk().filter_map(|step| match step {
            WalkStep::Node(addr, node) => Some((addr, node)),
            _ => None,
        })
    }

    /// The trees of the walk, in document order, each once.
    pub fn walk_trees(&self) -> impl Iterator<Item = TreeId> + '_ {
        self.walk().filter_map(|step| match step {
            WalkStep::EnterTree(tree) => Some(tree),
            _ => None,
        })
    }
}

/// The content of a document is the walk's source for its trees as parsed: the
/// arena gives the trees, the traversal list the trees listed, and the
/// resolution map the placements.
impl WalkSource for DocContent {
    fn listed(&self) -> &[TreeId] {
        &self.traversal
    }

    fn node_tree(&self, tree: TreeId) -> Option<&NodeTree<Flm>> {
        self.tree(tree).map(|tree| &**tree)
    }

    fn placement(&self, site: NodeAddr) -> Option<TreeId> {
        self.placement_of(site)
    }
}

impl fmt::Debug for DocContent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DocContent")
            .field("arena", &self.arena)
            .field("traversal", &self.traversal)
            .field("resolution", &self.resolution)
            .finish()
    }
}

/// The trees a [`Walk`] visits: trees by identifier, the trees of the
/// traversal list in order, and which node places which tree.
///
/// [`Walk`] defines document order and reads its trees through this trait, so
/// that the same order applies to any set of trees that represents a
/// document's trees: [`DocContent`] is the source of the trees as parsed, and a
/// run is the source of its frozen trees, what each tree became after the
/// rewrites ([`StageCx::walk`](crate::core::run::StageCx::walk)). The [module
/// documentation](self#the-walk) describes the order.
pub trait WalkSource {
    /// The trees of the traversal list, in the order the walk visits them. A
    /// tree that the walk reaches only through a node that places it is not in
    /// this list.
    fn listed(&self) -> &[TreeId];

    /// The node tree of the tree `tree`, or `None` when the source holds no
    /// tree with that identifier; the walk then does not enter it.
    fn node_tree(&self, tree: TreeId) -> Option<&NodeTree<Flm>>;

    /// The tree that the node at `site`, a node of one of the source's trees,
    /// places, or `None` when it places none.
    fn placement(&self, site: NodeAddr) -> Option<TreeId>;
}

/// One step of the walk, yielded by [`Walk`].
pub enum WalkStep<'d> {
    /// The walk enters a tree, either at its position in the traversal list or
    /// right after the node that places it. Every step until the matching
    /// [`WalkStep::ExitTree`] belongs to that tree or to a tree placed in it.
    EnterTree(TreeId),
    /// A node, with its address. The nodes of one tree come in pre-order: a
    /// node before its children.
    Node(NodeAddr, NodeRef<'d, Flm>),
    /// The walk leaves a tree; every node of it has been yielded.
    ExitTree(TreeId),
}

impl fmt::Debug for WalkStep<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WalkStep::EnterTree(tree) => write!(f, "EnterTree({tree})"),
            WalkStep::Node(addr, node) => write!(f, "Node({addr:?}, {})", node.summary()),
            WalkStep::ExitTree(tree) => write!(f, "ExitTree({tree})"),
        }
    }
}

/// One open tree of the walk.
struct WalkFrame<'d> {
    tree: TreeId,
    root: Option<NodeRef<'d, Flm>>,
    descendants: Descendants<'d, Flm>,
}

/// The walk: an iterator over the steps of the traversal of a [`WalkSource`]
/// in document order.
///
/// [`DocContent::walk`] returns one for the trees as parsed; [`Walk::new`]
/// creates one over any other source. A tree that the source lists twice, or
/// that is both listed and placed, is entered once only, the first time the
/// walk reaches it.
pub struct Walk<'d, S: ?Sized = DocContent> {
    source: &'d S,
    list_index: usize,
    stack: Vec<WalkFrame<'d>>,
    /// Whether each tree was entered, by the tree's index.
    visited: Vec<bool>,
    /// A tree whose entry step is due next: the placing node was just yielded.
    pending: Option<(TreeId, &'d NodeTree<Flm>)>,
}

impl<'d, S: WalkSource + ?Sized> Walk<'d, S> {
    /// The walk over the trees of `source`, starting from the first tree of its
    /// traversal list.
    pub fn new(source: &'d S) -> Walk<'d, S> {
        Walk { source, list_index: 0, stack: Vec::new(), visited: Vec::new(), pending: None }
    }

    /// Whether the walk has entered the tree `tree`.
    fn visited(&self, tree: TreeId) -> bool {
        self.visited.get(tree.index() as usize).copied().unwrap_or(false)
    }

    fn enter(&mut self, tree: TreeId, nodes: &'d NodeTree<Flm>) -> WalkStep<'d> {
        let index = tree.index() as usize;
        if self.visited.len() <= index {
            self.visited.resize(index + 1, false);
        }
        self.visited[index] = true;
        let root = nodes.root();
        self.stack.push(WalkFrame { tree, root: Some(root), descendants: root.descendants() });
        WalkStep::EnterTree(tree)
    }
}

impl<'d, S: WalkSource + ?Sized> Iterator for Walk<'d, S> {
    type Item = WalkStep<'d>;

    fn next(&mut self) -> Option<WalkStep<'d>> {
        if let Some((tree, nodes)) = self.pending.take() {
            return Some(self.enter(tree, nodes));
        }
        let source = self.source;
        if let Some(frame) = self.stack.last_mut() {
            let next = frame.root.take().or_else(|| frame.descendants.next());
            return match next {
                Some(node) => {
                    let addr = NodeAddr::new(frame.tree, node.id());
                    if let Some(target) = source.placement(addr) {
                        if !self.visited(target) {
                            // The placed tree follows its placing node: its entry
                            // step is the next one yielded, then its nodes, then
                            // the rest of this tree.
                            self.pending = source.node_tree(target).map(|nodes| (target, nodes));
                        }
                    }
                    Some(WalkStep::Node(addr, node))
                }
                None => {
                    let frame = self.stack.pop().expect("a frame was just observed");
                    Some(WalkStep::ExitTree(frame.tree))
                }
            };
        }
        // Between listed trees: the next not-yet-visited listed tree the
        // source holds.
        let listed = source.listed();
        while self.list_index < listed.len() {
            let tree = listed[self.list_index];
            self.list_index += 1;
            if self.visited(tree) {
                continue;
            }
            if let Some(nodes) = source.node_tree(tree) {
                return Some(self.enter(tree, nodes));
            }
        }
        None
    }
}

/// A document: its [content](DocContent) together with the option values set
/// on it.
///
/// A `Doc` does not change once built, and several runs may share one, for
/// example to render the same document in two formats, or to check it and then
/// render it. What a run computes (tables of data, the diagnostics of its
/// stages) is kept by that run, not here. A run is constructed over a document
/// with [`Run::new`](crate::core::run::Run::new).
#[derive(Debug, Default)]
pub struct Doc {
    content: DocContent,
    options: OptionValues,
}

impl Doc {
    /// A document holding `content`, with no option value set on it. A stage
    /// then reads, for each option type, the value set on the render request,
    /// or else the type's default.
    pub fn new(content: DocContent) -> Doc {
        Doc { content, options: OptionValues::new() }
    }

    /// A document holding `content`, with the option values `options` set on
    /// it.
    ///
    /// The values are not checked here. Constructing a run over this document
    /// ([`Run::new`](crate::core::run::Run::new)) checks them against the
    /// option types the registry declares, and fails with
    /// [`RunError::DocumentOption`](crate::core::run::RunError::DocumentOption)
    /// when a value is set for an option type that no feature declares or that
    /// cannot be set on a document. See [`crate::model::options`].
    pub fn with_options(content: DocContent, options: OptionValues) -> Doc {
        Doc { content, options }
    }

    /// The document's content: its trees, their order, and which node places
    /// which tree.
    pub fn content(&self) -> &DocContent {
        &self.content
    }

    /// The option values set on this document.
    pub fn options(&self) -> &OptionValues {
        &self.options
    }
}
