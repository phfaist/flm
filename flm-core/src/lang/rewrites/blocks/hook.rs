//! The block-structure hook: the aspect by which an element lays out its own
//! content during the layout step of the [block-structure pass](super).
//!
//! An aspect is a piece of behavior attached to an element when the feature
//! registers it, and read back through a trait object (see [elements and
//! aspects](crate::guide::concepts_overview#elements-and-aspects)). The
//! block-structure hook, [`BlockStructure`], is the aspect for an element whose
//! content has a block layout that its declarations cannot express. Such an
//! element could, for example, split its body at markers and build blocks for
//! each part, or create nodes that are never written in a document (decorators,
//! placed at the start of its first paragraph or at the end of its last).
//!
//! No standard element uses the hook. The decorations of the standard features,
//! such as a theorem's heading and its closing mark, are placed when the
//! document is lowered for a backend, not by this hook.
//!
//! # How the hook is called
//!
//! The layout step calls the hook once for every node of the element that it
//! reaches outside an [opaque
//! region](crate::guide::concepts_overview#opaque-regions), from the root
//! downwards, before its own default handling of the node. The hook either lays
//! out the node itself and returns the nodes it staged (built into the new
//! tree), or returns [`Structuring::Default`] to leave the node to the default
//! handling. The hook runs while the paragraph around its node is being built,
//! so it lays out its node's own content and never the paragraph around it.
//!
//! The hook works through two values it receives:
//!
//! - [`StructureCx`], the context through which the pass stages nodes;
//! - [`BlockStructurer`], the operations of the layout step that the hook may
//!   call (lay out a run of nodes as blocks or as inline content, apply the
//!   default handling to a node, rebuild a node around a new body, create a
//!   node, report a diagnostic). A staged node cannot be read back, so the
//!   hook builds its result from these operations rather than by editing a
//!   tree.
//!
//! A node the hook creates is a [`SynthesizedNode`]: a one-node tree of its own,
//! so that it is a real node reference whose block role can be read like any
//! node's, and which can be placed in a run of nodes given to
//! [`BlockStructurer::build_blocks`] or [`BlockStructurer::build_inline`]. Those
//! two operations stage it when they lay out the run. A created node that is
//! never placed has no place in the structured tree: the pass reports it
//! ([`DecoratorWithoutHome`](super::error::DecoratorWithoutHome)) and drops it.
//! [`RegionLayout`] tells the hook whether its node is placed between blocks or
//! inside inline content.
//!
//! A feature attaches the hook to its element by the trait-object type, like
//! every aspect: `.with::<dyn BlockStructure>(Arc::new(implementation))`.

use std::fmt;
use std::sync::Arc;

use techy::core::Lang;
use techy::core::node::{BuildId, NodeKind, NodeRef, NodeTree, NodeTreeBuilder};
use techy::source::{Source, SourceSpan};
use techy::transform::RestageContext;

use super::error::StructureError;
use crate::model::element::ElementSpec;
use crate::lang::flm::Flm;
use crate::lang::mint::FlmSynthesizedSpec;
use crate::lang::origin::{FlmDiagnostic, FlmSource};

/// The context through which the block-structure pass, a
/// [`BlockTransparent`](super::transparent::BlockTransparent) aspect and a
/// [`BlockStructure`] hook stage nodes: techy's restage context
/// ([`RestageContext`]) over the FLM language.
///
/// To *stage* a node is to build it into the new tree that the walk is
/// producing; a staged node is known by its [`BuildId`] and cannot be read
/// back. The two unit type parameters say that FLM trees carry no
/// annotations.
pub type StructureCx<'t> = RestageContext<'t, Flm, (), ()>;

/// What a [`BlockStructure`] hook returns for its node: leave it to the
/// default handling, or replace it by nodes the hook staged.
#[derive(Debug)]
pub enum Structuring {
    /// Leave the node to the default handling of the layout step, the same as
    /// [`BlockStructurer::default`]: each argument laid out by its declared
    /// layout, the body by the body declaration, and the node rebuilt around
    /// them.
    Default,
    /// The hook staged the node's replacement itself, through the
    /// [`BlockStructurer`] or the context: these staged nodes take the node's
    /// place among its siblings, in this order. An empty list removes the
    /// node.
    Emit(Vec<BuildId>),
}

/// The block-structure hook: the aspect an element carries to lay out its own
/// content during the layout step of the [block-structure pass](super).
///
/// [`structure`](BlockStructure::structure) is called once for every node of
/// the element that the layout step reaches outside an opaque region, from the
/// root downwards, before the default handling of that node. The hook lays out
/// the node through the operations of `bs` and returns the nodes it staged, or
/// returns [`Structuring::Default`] to leave the node to the default handling.
/// See the [module documentation](self) for what a hook is for.
///
/// A hook must not give its *own* node to a run it lays out, because the pass
/// would call the hook again on it. It lays out the node's parts instead, with
/// [`body_run`](BlockStructurer::body_run),
/// [`default`](BlockStructurer::default) and
/// [`with_body`](BlockStructurer::with_body).
///
/// The [tree contract](super::check) binds what a hook produces: a body or an
/// argument that its declaration lays out as blocks must hold only paragraphs,
/// block-level nodes and comments, and one laid out inline must hold no
/// paragraph.
pub trait BlockStructure: Send + Sync {
    /// Lays out `node`, or leaves it to the default handling.
    ///
    /// `cx` is the context to stage nodes through, and `bs` the operations of
    /// the layout step.
    ///
    /// # Errors
    ///
    /// The hook returns
    /// [`StructureError::hook`](super::error::StructureError::hook) for a
    /// failure of its own, and passes on unchanged the error of a
    /// [`BlockStructurer`] operation that failed. Either error ends the pass and
    /// leaves no tree, so a problem in the *document* is reported as a
    /// diagnostic through [`diagnose`](BlockStructurer::diagnose) instead.
    fn structure(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
        bs: &mut dyn BlockStructurer,
    ) -> Result<Structuring, StructureError>;
}

/// Where a hook's node is placed in the content being laid out, as
/// [`BlockStructurer::region`] returns it.
///
/// This is the position of the node, not the region layout of the [content
/// region](super) that holds it (both are defined in the module
/// documentation): a node inside a paragraph gets `Inline` even when the paragraph is part of a
/// content region laid out as blocks, and a hook is never called inside an
/// opaque region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RegionLayout {
    /// The node is placed directly in a content region laid out as blocks,
    /// on its own between the paragraphs. This is where a node whose block
    /// role is [`Block`](crate::model::aspects::BlockRole::Block) goes.
    Blocks,
    /// The node is inside inline content: inside a paragraph, or in a content
    /// region laid out inline.
    Inline,
}

/// A node created by a block-structure hook through
/// [`BlockStructurer::synthesize_callable`], or by a
/// [`BlockTransparent`](super::transparent::BlockTransparent) aspect through
/// [`BlockCutter::synthesize_callable`](super::transparent::BlockCutter::synthesize_callable):
/// a callable node with no arguments, no node slots (techy's `ParsedSlot`, the
/// parts of a node's content outside its arguments, such as an environment's
/// body) and no content, normally of an
/// element declared with
/// [`SyntaxDef::Synthesized`](crate::lang::mint::SyntaxDef::Synthesized), which
/// no document can write (the operation does not check this). Its span is an empty span on a
/// synthesized source created at the span of the node the hook or aspect was
/// called on.
///
/// The node is held in a one-node tree of its own until it is staged. A hook
/// gives [`node`](SynthesizedNode::node) to a run passed to
/// [`BlockStructurer::build_blocks`] or [`BlockStructurer::build_inline`]; an
/// aspect stages it with [`BlockCutter::carry`](super::transparent::BlockCutter::carry)
/// or in a run passed to
/// [`BlockCutter::restage_and_classify`](super::transparent::BlockCutter::restage_and_classify).
pub struct SynthesizedNode {
    tree: NodeTree<Flm>,
}

impl SynthesizedNode {
    /// Builds the one-node tree of a callable of `element` at `node`: no
    /// arguments, no slots, no content, an empty span on a source synthesized
    /// at `node`'s span, and `node`'s parsing state. Only the two walks of the
    /// pass call this; an element obtains the value from
    /// [`BlockStructurer::synthesize_callable`] or
    /// [`BlockCutter::synthesize_callable`](super::BlockCutter::synthesize_callable).
    pub(crate) fn mint(
        node: &NodeRef<'_, Flm>,
        element: &Arc<ElementSpec>,
    ) -> Result<SynthesizedNode, StructureError> {
        let spec = Arc::new(FlmSynthesizedSpec::new(Arc::clone(element)));
        let description = format!("{} (synthesized by block structuring)", element.name);
        let source: Arc<FlmSource> = Arc::new(Source::synthesized(
            String::new(),
            description,
            node.span().clone(),
        ));
        let span = SourceSpan::new(&source, 0..0);
        let state = Arc::clone(node.parsing_state());
        let mut builder: NodeTreeBuilder<Flm> = NodeTreeBuilder::new();
        let kind = NodeKind::callable(spec.callable_data());
        let ext = Flm::make_node_ext(&kind, &span, &state, builder.staged_children(&[]))?;
        let id = builder.add(kind, span, state, Vec::new(), ext, ())?;
        let tree = builder.finish(id)?;
        Ok(SynthesizedNode { tree })
    }

    /// The created node, as a reference to place in a run given to
    /// [`BlockStructurer::build_blocks`] or [`BlockStructurer::build_inline`].
    pub fn node(&self) -> NodeRef<'_, Flm> {
        self.tree.root()
    }

    /// The element the node carries.
    pub fn element(&self) -> &Arc<ElementSpec> {
        &self.tree.root().ext().element
    }
}

impl fmt::Debug for SynthesizedNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SynthesizedNode")
            .field("element", &self.element().name)
            .finish()
    }
}

/// The operations of the layout step that a [`BlockStructure`] hook may call.
///
/// The layout step itself implements this trait and hands itself to the hook,
/// so a hook lays out content exactly as the pass does, without access to the
/// pass's internal state. Every operation that lays out a run of nodes stages
/// its result through the context and returns the staged nodes; the hook
/// assembles them into its result ([`Structuring::Emit`]).
///
/// # Errors
///
/// Every operation that stages fails with [`StructureError`] when an operation
/// of techy's tree builder fails, when the content is nested too deeply, or
/// when the hook of a node inside the laid-out content fails;
/// [`body_run`](BlockStructurer::body_run) and
/// [`with_body`](BlockStructurer::with_body) also fail with
/// [`StructureError::NoBody`] for a node with no body slot. A node's body slot
/// is the node slot marked by its body marker as the node's body, such as an
/// environment's body (see [`FlmNodeExts`](crate::lang::flm::FlmNodeExts)); a
/// node slot (techy's `ParsedSlot`) is a part of a node's content outside its
/// arguments.
pub trait BlockStructurer {
    /// Lays out `run`, a run of sibling nodes, as blocks and returns the staged
    /// nodes: every maximal stretch of inline content becomes a paragraph,
    /// each block-level node is placed on its own, the paragraph-break markers
    /// end paragraphs and are removed, comments are kept, block decorators are
    /// placed by their [`Placement`](crate::model::aspects::Placement), and the
    /// text is resolved with the whitespace at the edges of each paragraph
    /// removed. A run whose nodes were parsed in math mode, and a run that
    /// stands in a verbatim group or an opaque wrapper or begins with one, is
    /// an [opaque region](crate::guide::concepts_overview#opaque-regions) and
    /// is copied unchanged instead.
    ///
    /// The staged nodes are paragraphs, block-level nodes and comments. A
    /// container the hook builds to hold them must therefore be a node of an
    /// element whose block role is [`Block`](crate::model::aspects::BlockRole::Block):
    /// a `{…}` group holds inline content only, and the
    /// [tree-contract checker](super::check) reports a paragraph inside one.
    fn build_blocks(
        &mut self,
        cx: &mut StructureCx<'_>,
        run: &[NodeRef<'_, Flm>],
    ) -> Result<Vec<BuildId>, StructureError>;

    /// Lays out `run`, a run of sibling nodes, as inline content and returns
    /// the staged nodes: no paragraph is built, adjacent text is merged and its
    /// whitespace collapsed, and the whitespace at the edges of the run is kept.
    /// A block-level node in the run is kept where it is and reported
    /// ([`BlockInInline`](super::error::BlockInInline)); a paragraph break is
    /// read as one space and reported
    /// ([`ParbreakInInline`](super::error::ParbreakInInline), with the context
    /// `run`). A run parsed in math mode, and a run that stands in a verbatim
    /// group or an opaque wrapper or begins with one, is copied unchanged
    /// instead.
    fn build_inline(
        &mut self,
        cx: &mut StructureCx<'_>,
        run: &[NodeRef<'_, Flm>],
    ) -> Result<Vec<BuildId>, StructureError>;

    /// Applies the default handling of the layout step to `node` and returns
    /// the staged nodes: the handling a node receives when its element has no
    /// hook, or when the hook returns [`Structuring::Default`].
    ///
    /// The arguments of a callable node are laid out by their declared layouts
    /// ([`ArgLayout`](crate::lang::mint::ArgLayout); inline by default), and
    /// its body by the element's body declaration
    /// ([`BodyLayout`](crate::lang::mint::BodyLayout)): as blocks or inline
    /// where the declaration says so, and otherwise by the element's block role
    /// (as blocks for an element whose role is
    /// [`Block`](crate::model::aspects::BlockRole::Block), inline otherwise). A
    /// verbatim body, a body under an opaque wrapper and a body read in math
    /// mode are copied unchanged, and so is an argument whose content stands
    /// in a verbatim group or an opaque wrapper. The content of a `{…}` group
    /// is laid out inline. Mathematics groups, verbatim groups and opaque
    /// wrappers are copied unchanged.
    fn default(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: &NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, StructureError>;

    /// Returns the content nodes of `node`'s body slot, as a run to lay out
    /// with [`build_blocks`](BlockStructurer::build_blocks) or
    /// [`build_inline`](BlockStructurer::build_inline). Nothing is staged.
    ///
    /// # Errors
    ///
    /// [`StructureError::NoBody`] when the node has no body slot.
    fn body_run<'t>(
        &mut self,
        node: &NodeRef<'t, Flm>,
    ) -> Result<Vec<NodeRef<'t, Flm>>, StructureError>;

    /// Stages a copy of the callable `node` whose body slot holds `body`, nodes
    /// the hook staged, and returns it. The arguments and the other slots
    /// receive the default handling; what surrounds the body's content in the
    /// source (the delimiters of the slot and the text around them) is copied
    /// unchanged.
    ///
    /// # Errors
    ///
    /// [`StructureError::NoBody`] when the node has no body slot.
    fn with_body(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: &NodeRef<'_, Flm>,
        body: Vec<BuildId>,
    ) -> Result<BuildId, StructureError>;

    /// Creates a node of `element` at `node`: a callable node with no
    /// arguments, no slots and no content, with an empty span on a source
    /// synthesized at `node`'s span, and `node`'s parsing state.
    ///
    /// Place [`SynthesizedNode::node`] in a run given to
    /// [`build_blocks`](BlockStructurer::build_blocks) or
    /// [`build_inline`](BlockStructurer::build_inline); the element's block role
    /// then decides where the node goes in the paragraph:
    /// `BlockDecorator(Leads)` for a run-in heading, `BlockDecorator(Trails)`
    /// for a mark that ends a statement
    /// ([`Placement`](crate::model::aspects::Placement)). A created node that
    /// is never placed by one of those two operations is reported
    /// ([`DecoratorWithoutHome`](super::error::DecoratorWithoutHome)) and
    /// dropped when the hook returns; a copy the hook stages itself through the
    /// context does not count as placed.
    ///
    /// `element` should be declared with
    /// [`SyntaxDef::Synthesized`](crate::lang::mint::SyntaxDef::Synthesized)
    /// (no syntax), so that no document can write it. This operation cannot
    /// check that, because an element value does not carry its syntax
    /// declaration, and it creates a node of any element.
    fn synthesize_callable(
        &mut self,
        node: &NodeRef<'_, Flm>,
        element: &Arc<ElementSpec>,
    ) -> Result<SynthesizedNode, StructureError>;

    /// Adds `diagnostic` to the diagnostics the pass returns to its caller: a
    /// condition the hook found in the document, at a span of the hook's
    /// choice. This is how a hook reports a problem in the document without
    /// ending the pass.
    fn diagnose(&mut self, diagnostic: FlmDiagnostic);

    /// Returns where the hook's node is placed: directly between blocks, or
    /// inside inline content (a paragraph, or a content region laid out
    /// inline).
    fn region(&self) -> RegionLayout;
}
