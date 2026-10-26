//! Block-transparent elements: the [`BlockTransparent`] aspect, the result it
//! returns, and the [`BlockCutter`] operations it calls during the cut step of
//! the [block-structure pass](super).
//!
//! A **block-transparent** element is one whose content is laid out as if it
//! were written directly in the content around it (see [block-transparent
//! elements](crate::guide::concepts_overview#block-transparent-elements)). An
//! ordinary element whose argument is laid out according to its content
//! ([`ArgLayout::InlineOrBlocks`](crate::lang::mint::ArgLayout::InlineOrBlocks))
//! becomes a block of its own when that argument holds a paragraph break or a
//! list and the element's [block role](crate::model::aspects::BlockRoleAspect)
//! follows that content (the element tests its argument with
//! [`argument_holds_blocks`](super::argument_holds_blocks) to choose its role);
//! the argument's content is then laid out as paragraphs inside the node. An
//! argument declared inline reports the paragraph break instead. A
//! block-transparent element never becomes a block of its own: a paragraph
//! break inside its content is a paragraph break of the content around it, and
//! a list inside its content is a block of the content around it. A mark that an author places over a passage is the typical case
//! (the `annotations` feature's highlight, [`crate::stdcore::features::annotations`]):
//! the passage may cross a paragraph boundary, and the mark continues across
//! it.
//!
//! # The cut step
//!
//! A paragraph holds whole nodes, so the layout step cannot place one node
//! across two paragraphs. Before any paragraph is built, the
//! **cut step** of the pass therefore replaces every block-transparent node
//! whose content crosses a paragraph break or holds a block by **cut parts**:
//! nodes that each hold one stretch of the content (a stretch of inline content
//! between two paragraph breaks, or one block), with the paragraph breaks
//! between them. The layout step that follows places each inline cut part in
//! its paragraph and each block cut part among the blocks, as it places any
//! node.
//!
//! # What the element does, and what the cut step provides
//!
//! Only the element's own code can say what its arguments mean, which of them
//! to cut, and what a cut part of it is: the element itself rebuilt around the
//! cut part's content, a node of another element, or a block passed through as
//! it is. So
//! the cut step calls the element's aspect with the whole parsed node, and the
//! aspect returns the staged nodes that replace the node ([`Cutting`]). The
//! cut step provides, through [`BlockCutter`], what an element cannot do
//! alone:
//!
//! - **classifying while restaging**
//!   ([`restage_and_classify`](BlockCutter::restage_and_classify)): a staged
//!   node cannot be read back, so its block role cannot be asked once it is
//!   staged. A run of nodes is therefore classified from its input nodes while
//!   they are staged. A block-transparent node inside the run is cut at the
//!   same time, which makes the cut recursive.
//! - **rebuilding the node** ([`rebuild`](BlockCutter::rebuild)) around new
//!   content, with data of the element's own attached to it. techy's operation
//!   for rebuilding a node copies the node's extension unchanged, so a rebuild
//!   that attaches data goes through techy's lower-level tree builder; the cut
//!   step implements that rebuild once, for every element.
//! - **the cut number** ([`new_cut_number`](BlockCutter::new_cut_number)): a
//!   number unique within the tree, which an element may write into its cut
//!   parts so that the parts of one cut can be told apart from those of
//!   another. Uniqueness is a property of the whole walk, which only the cut
//!   step sees.
//!
//! Everything else is the element's choice: which arguments or body to scan, how to group
//! the classified nodes into cut parts, and which cut parts carry data about
//! where they came from, and of what type.
//!
//! # The continuation record
//!
//! An element whose cut parts must know where they came from (for example, the
//! first cut part shows a label and the others do not) attaches a value of a
//! type of its own to each cut part, through [`rebuild`](BlockCutter::rebuild).
//! This value is the *continuation record*. It is held in the node's
//! [`FlmNodeExt::payload`](crate::lang::FlmNodeExt::payload), is copied with
//! the node when the node is copied into a synthesized value, and is read by
//! code that knows its type. The cut step defines no record type and keeps no
//! table of cuts. A node the cut step left alone carries no record.
//!
//! A feature attaches the aspect to its element by the trait-object type, like
//! every aspect: `.with::<dyn BlockTransparent>(Arc::new(implementation))`.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use super::error::StructureError;
use super::hook::{StructureCx, SynthesizedNode};
use crate::lang::ext::PresPayload;
use crate::lang::flm::Flm;
use crate::lang::origin::FlmDiagnostic;
use crate::model::element::ElementSpec;

/// What a [`BlockTransparent`] aspect returns for its node: leave it as it is,
/// or replace it by staged items.
#[derive(Debug)]
pub enum Cutting {
    /// Leave the node as it is, to the default handling of the cut step: the
    /// cut step continues into the node's arguments and node slots (techy's
    /// `ParsedSlot`, the parts of its content outside the arguments, such as an
    /// environment's body), so a
    /// block-transparent node inside them is still cut, and the node itself is
    /// copied once, with its extension (and any payload) unchanged.
    ///
    /// Any node the aspect staged before returning this is absent from the
    /// tree, so an aspect that may leave its node decides so before staging
    /// anything.
    Default,
    /// These staged nodes replace the node, in this order, among the node's
    /// siblings and without a wrapper, so a paragraph-break marker among them
    /// ends the paragraph around the node once the layout step runs. An empty
    /// list removes the node.
    Emit(Vec<CutItem>),
}

/// One staged node of the cut step, classified by what it is to the layout
/// step that follows.
///
/// [`BlockCutter::restage_and_classify`] returns a run as these items, and a
/// [`BlockTransparent`] aspect returns the replacement of its node as these
/// items ([`Cutting::Emit`]). The variant of an item (`Inline`, `Blank`,
/// `Marker` or `Block`) matters when the node being cut is inside the content
/// of another block-transparent node: the outer node's aspect receives the
/// inner node's items, variants included, from its
/// own call to [`restage_and_classify`](BlockCutter::restage_and_classify).
/// An aspect therefore classifies each cut part by what it is:
/// [`Inline`](CutItem::Inline) for a cut part holding inline content,
/// [`Block`](CutItem::Block) for one that is a block, and the markers and
/// blank items it passes through with the variant they came with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CutItem {
    /// Inline content: a node that goes in a paragraph.
    Inline(BuildId),
    /// A comment, text that is only whitespace, or a node whose element
    /// carries the [`Whitespace`](crate::model::aspects::Whitespace) aspect
    /// (such as `~`): a node that goes inline but is not content, so a run of
    /// such nodes alone makes no paragraph. An aspect that groups items into
    /// cut parts makes no cut part of a stretch of such items alone and passes
    /// them through as they are: the layout step keeps the comments, and
    /// removes, collapses or keeps the whitespace as it does in the content
    /// around the node.
    Blank(BuildId),
    /// A paragraph-break marker: the node a blank line leaves in the parsed
    /// tree. Two inline items with a marker between them go in two
    /// paragraphs.
    Marker(BuildId),
    /// A node whose block role is
    /// [`Block`](crate::model::aspects::BlockRole::Block): it is placed on its
    /// own between the paragraphs.
    Block(BuildId),
}

impl CutItem {
    /// The staged node of this item.
    pub fn id(self) -> BuildId {
        match self {
            CutItem::Inline(id) | CutItem::Blank(id) | CutItem::Marker(id) | CutItem::Block(id) => id,
        }
    }
}

/// Designates an argument or the body of a callable node, to which
/// [`BlockCutter::rebuild`] gives new content.
///
/// A `CutPart` is not a cut part (a node a cut produces, see the [module
/// documentation](self)): it says which argument or body of the node being
/// rebuilt receives the content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CutPart {
    /// The argument at this position, counting from zero in the order the
    /// arguments are declared (written or not).
    Argument(usize),
    /// The argument declared under this name.
    ArgumentNamed(String),
    /// The body: the node slot (techy's `ParsedSlot`, a part of a node's content
    /// outside its arguments) marked by its body marker as the node's body, such
    /// as an environment's body (see
    /// [`FlmNodeExts`](crate::lang::flm::FlmNodeExts)).
    Body,
}

/// The aspect an element carries to be **block-transparent**: its content is
/// laid out as if it were written in the content around it, and its node is
/// cut into cut parts at the paragraph breaks and blocks that its content
/// holds, before any paragraph is built. See the [module documentation](self).
///
/// [`cut`](BlockTransparent::cut) is called once for every node of the element
/// that the cut step reaches outside an [opaque
/// region](crate::guide::concepts_overview#opaque-regions), from the root
/// downwards: an outer node's aspect is called before the nodes inside it, and
/// a node inside a run that the outer aspect gives to
/// [`restage_and_classify`](BlockCutter::restage_and_classify) is cut there,
/// its items returned in its place. The aspect returns [`Cutting::Default`]
/// when there is nothing to cut, or the staged nodes that replace the node.
///
/// # A typical implementation
///
/// 1. Read the content (a run of nodes) of each argument or body the element
///    cuts. Return `Default`, staging
///    nothing, when the node is one of the element's own cut parts (it carries
///    the element's continuation record), or when none of the runs holds a
///    cut point ([`run_holds_blocks`](super::run_holds_blocks)).
/// 2. Otherwise give each run to
///    [`restage_and_classify`](BlockCutter::restage_and_classify).
/// 3. Group each maximal stretch of inline items into a cut part, with
///    [`rebuild`](BlockCutter::rebuild): the stretch as the part's new content,
///    and a continuation record as the payload. Make each block item a cut part
///    of its own, and pass the markers through.
/// 4. Return the sequence ([`Cutting::Emit`]).
///
/// After the cut, no cut part may hold a paragraph-break marker at the top
/// level of an argument or body the element cut, nor a block beside other content: the
/// layout step lays out each cut part by its block role, a cut part around
/// inline content as inline content and a cut part around one block as a
/// block. [`rebuild`](BlockCutter::rebuild) does not check this.
///
/// # Cut parts must not be cut again
///
/// Within one cut step, the aspect is never called on a cut part it produced.
/// But the aspect is called at every node of the element, cut parts included,
/// when a tree is structured a second time, or when a cut part copied into a
/// synthesized value is structured again. A block cut part holds a block at
/// the top level of an argument or body, so [`run_holds_blocks`](super::run_holds_blocks)
/// returns `true` for it. Only the element can tell a cut part from a node
/// that was never cut, because the continuation record is of the element's own
/// type. The element therefore recognizes its cut parts by their record and
/// returns [`Cutting::Default`] for them, so that structuring a structured
/// tree changes nothing. An element that attaches no record to a block cut part
/// cuts it again into an identical cut part, which changes nothing either.
///
/// An aspect must not give its *own* node to
/// [`restage_and_classify`](BlockCutter::restage_and_classify), because the cut
/// step would call the aspect again on it. It gives the content of the node's
/// arguments or body instead.
pub trait BlockTransparent: Send + Sync {
    /// Cuts `node` into cut parts, or leaves it as it is.
    ///
    /// `cx` is the context to stage nodes through, and `cutter` the operations
    /// of the cut step.
    ///
    /// # Errors
    ///
    /// The aspect returns
    /// [`StructureError::cut`](super::error::StructureError::cut) for a failure
    /// of its own, and passes on unchanged the error of a [`BlockCutter`]
    /// operation that failed. Either error ends the pass and leaves no tree, so
    /// a problem in the *document* is reported as a diagnostic through
    /// [`diagnose`](BlockCutter::diagnose) instead.
    fn cut(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
        cutter: &mut dyn BlockCutter,
    ) -> Result<Cutting, StructureError>;
}

/// The operations of the cut step that a [`BlockTransparent`] aspect may call.
///
/// The cut step itself implements this trait and hands itself to the aspect.
/// Every operation that produces nodes stages them through the context `cx`
/// (builds them into the new tree) and returns their [`BuildId`]s. A staged
/// node cannot be read back, so the aspect keeps what it needs to know about
/// each one (the variants of the items that
/// [`restage_and_classify`](BlockCutter::restage_and_classify) returns tell it)
/// and assembles the staged nodes into its result ([`Cutting::Emit`]). A staged
/// node can be used once, in the result or inside a node the aspect rebuilds;
/// a staged node used nowhere is absent from the tree.
///
/// # Errors
///
/// Every operation that stages fails with [`StructureError`] when an operation
/// of techy's tree builder fails, when the content is nested more deeply than
/// the [recursion guard](crate::lang::guard) allows, or when the aspect of a
/// node inside the content fails; [`rebuild`](BlockCutter::rebuild) also fails
/// for an argument or body the node does not have.
pub trait BlockCutter {
    /// Stages the nodes of `run` (a run of sibling nodes, typically the content
    /// of one of the node's arguments) and returns them as classified items, in
    /// order.
    ///
    /// - A paragraph-break marker is copied and returned as a
    ///   [`Marker`](CutItem::Marker).
    /// - A block-transparent node is **cut here**: its element's aspect is
    ///   called, and the items it returns are returned in the node's place, with
    ///   the variants it gave them. A node that its aspect leaves as it is, is
    ///   copied once and classified like any other node.
    /// - Every other node is staged through the cut step (so a
    ///   block-transparent node deeper inside it is cut as usual) and
    ///   classified by the block role of its *input* node: a node whose role
    ///   is [`Block`](crate::model::aspects::BlockRole::Block) is a
    ///   [`Block`](CutItem::Block); a comment, text that is only whitespace or
    ///   a node whose element carries the
    ///   [`Whitespace`](crate::model::aspects::Whitespace) aspect a
    ///   [`Blank`](CutItem::Blank); anything else an [`Inline`](CutItem::Inline).
    ///
    /// A run parsed in mathematics mode is an [opaque
    /// region](crate::guide::concepts_overview#opaque-regions): its nodes are
    /// returned as inline and blank items only, never as a marker or a block,
    /// and no aspect is called on them. The run is copied unchanged, except an
    /// argument or a body that a construct of mathematics reads in text mode
    /// (such as `\text{…}`): the cut step walks that content like any other
    /// content, and cuts the block-transparent nodes inside it. A run that
    /// stands in a verbatim group or an opaque wrapper (the content of an
    /// argument placed under one), or begins with one, is an opaque region
    /// too, and is copied unchanged with no exception, whatever its parsing
    /// mode.
    ///
    /// The markers and blocks returned are the cut points: the layout step
    /// treats them as paragraph boundaries and blocks, by the same tests.
    ///
    /// A text node is never split: whitespace at the edge of a text node next
    /// to a marker or a block belongs to that text node's inline item, so it
    /// ends up inside the cut part that the aspect builds from that item. An
    /// aspect that wants otherwise splits the text itself.
    fn restage_and_classify(
        &mut self,
        cx: &mut StructureCx<'_>,
        run: &[NodeRef<'_, Flm>],
    ) -> Result<Vec<CutItem>, StructureError>;

    /// Stages a copy of the callable `node` in which each part listed in
    /// `replacements` holds the given staged nodes as its content, and returns
    /// the copy. When `payload` is given, it is attached as the copy's
    /// [`payload`](crate::lang::FlmNodeExt::payload) (the continuation record).
    ///
    /// The copy keeps the node's span, its parsing state, its element and the
    /// way it was written. A replaced part keeps what surrounds its content
    /// (the delimiters of an argument, the whitespace and comments around it),
    /// and only its content changes. Every part not replaced is staged through
    /// the cut step, as [`Cutting::Default`] would stage it; an argument that
    /// was not written stays unwritten. Without `payload`, the copy's extension
    /// is the node's own, payload included.
    ///
    /// The staged nodes given as a part's content must be inline content or
    /// blocks, as the part's layout expects; `rebuild` does not check them. The
    /// caller keeps the cut parts free of cut points in the parts it cut (see
    /// [`BlockTransparent`]).
    ///
    /// # Errors
    ///
    /// [`StructureError::Cut`] when `node` is not a callable, when a part is
    /// listed twice, or when a listed part does not exist: an argument
    /// position or name the node does not have, the body of a node without a
    /// body slot (a body slot is the node slot marked by its body marker as the
    /// node's body; a node slot, techy's `ParsedSlot`, is a part of a node's
    /// content outside its arguments), or an argument that was not written,
    /// which has no delimiters to hold content. Any other [`StructureError`]
    /// when staging fails.
    fn rebuild(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: &NodeRef<'_, Flm>,
        replacements: Vec<(CutPart, Vec<BuildId>)>,
        payload: Option<Arc<dyn PresPayload>>,
    ) -> Result<BuildId, StructureError>;

    /// Stages an unchanged copy of `node` and its whole subtree, and returns
    /// it: nothing inside it is cut or classified. This is for a node the
    /// aspect passes through as it is, such as a comment or a node created
    /// with [`synthesize_callable`](BlockCutter::synthesize_callable).
    fn carry(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: &NodeRef<'_, Flm>,
    ) -> Result<BuildId, StructureError>;

    /// Creates a node of `element` at `node`: a callable node with no
    /// arguments, no slots and no content, with an empty span on a source
    /// synthesized at `node`'s span, and `node`'s parsing state. It is the same
    /// node that
    /// [`BlockStructurer::synthesize_callable`](super::BlockStructurer::synthesize_callable)
    /// creates.
    ///
    /// The created node is held in a tree of its own. It enters the cut tree
    /// when the aspect stages it, with [`carry`](BlockCutter::carry) or in a
    /// run given to [`restage_and_classify`](BlockCutter::restage_and_classify);
    /// a created node that is never staged is absent from the tree, and nothing
    /// is reported. `element` should be declared with
    /// [`SyntaxDef::Synthesized`](crate::lang::mint::SyntaxDef::Synthesized)
    /// (no syntax), so that no document can write it; this operation cannot
    /// check that.
    fn synthesize_callable(
        &mut self,
        node: &NodeRef<'_, Flm>,
        element: &Arc<ElementSpec>,
    ) -> Result<SynthesizedNode, StructureError>;

    /// Adds `diagnostic` to the diagnostics the pass returns to its caller: a
    /// condition the aspect found in the document, at a span of the aspect's
    /// choice. This is how an aspect reports a problem in the document without
    /// ending the pass.
    fn diagnose(&mut self, diagnostic: FlmDiagnostic);

    /// Returns a new **cut number**: a number that no other call returns
    /// during the cut step over this tree. The numbers are 0, 1, 2, … in the
    /// order of the calls, which follows the order of the walk, so the same
    /// tree gets the same numbers every time it is structured.
    ///
    /// An aspect asks for one number for each node it cuts, and writes it into
    /// the continuation records of that node's cut parts, so that the cut parts
    /// of one cut can be told apart from those of another. The cut step gives
    /// the number no meaning and keeps no table of it.
    fn new_cut_number(&mut self) -> usize;
}
