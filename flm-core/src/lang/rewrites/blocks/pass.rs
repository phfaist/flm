//! The block-structure pass: [`structure`] and [`structure_with_guard`], and
//! the rules by which the layout step lays out each content region and
//! resolves its text.
//!
//! The pass makes two walks over a tree (see the [module documentation](super)).
//! The first, the **cut step**, replaces every node of a
//! [block-transparent](super::transparent) element whose content crosses a
//! paragraph break or holds a block by the cut parts its element builds. The
//! second, the **layout step** described on this page, runs over the cut tree as
//! over any tree: a cut part is laid out by its block role, like any node.
//!
//! [`structure`] runs the pass under the default recursion-guard policy,
//! [`GuardPolicy::StackBudget`] (a fixed budget of stack for the walk; the
//! other policy, which counts nesting levels, serves tests);
//! [`structure_with_guard`] runs it under a
//! [guard policy](crate::lang::guard::GuardPolicy) the caller chooses. Both are
//! plain functions over a tree, which need no pipeline around them. The rewrite
//! stage of the [`blocks` feature](crate::stdcore::features::blocks) places the
//! pass in a plan.
//!
//! # The three region layouts
//!
//! The layout step works on content regions (runs of sibling nodes laid out
//! together, such as the top level of the tree, an environment body's content,
//! an argument's content, or the children of a `{…}` group), not on single
//! nodes. It lays out each content region in one of three ways.
//!
//! **As blocks.** The run is divided at its paragraph-break markers (nodes of
//! the `flm.parbreak` element, the nodes a blank line leaves, recognized by the
//! element value that the parsing state's [structural
//! elements](crate::lang::structural::StructuralElements) hold) and at its
//! nodes of block role [`Block`](BlockRole::Block):
//!
//! - every maximal stretch of inline content becomes a paragraph, a group node
//!   of the paragraph group class, created by the pass;
//! - each node of block role `Block` is placed on its own;
//! - the paragraph-break markers are removed;
//! - a comment stays where it is: inside a paragraph when it is among the
//!   paragraph's content, and between the blocks otherwise; a comment never
//!   starts a paragraph;
//! - whitespace between blocks, text that is only whitespace and
//!   [whitespace nodes](#whitespace-nodes) alike, is removed;
//! - a block decorator declared [`Placement::Leads`] starts a new paragraph, and
//!   one declared [`Placement::Trails`] ends the paragraph it is in.
//!
//! **Inline.** No paragraph is built. The text is resolved (merged and
//! collapsed), and the whitespace at the edges of the content region is kept. A
//! node of block role `Block` is kept where it is and reported
//! ([`BlockInInline`]); a paragraph break is read as one space and reported
//! ([`ParbreakInInline`]).
//!
//! **Opaque.** An [opaque
//! region](crate::guide::concepts_overview#opaque-regions) is copied unchanged,
//! node for node: the content of a mathematics group, of a verbatim group or
//! of an opaque wrapper (the group without delimiters that minting places
//! around the content of an argument or a body declared with one, and of every
//! characters-only group), a verbatim body, and any run whose nodes were
//! parsed in math mode (the body of a math-mode environment, the root of a
//! mathematics snippet). Opacity is a property of the content region, not of
//! the command that opened it, so the different ways of writing mathematics
//! behave alike. Inside an opaque region,
//! no text is resolved, no paragraph-break marker is removed or reported (what
//! a blank line inside mathematics means is checked by the mathematics feature)
//! and no block-structure hook is called.
//!
//! **Text inside mathematics.** Inside an opaque region, the layout step still
//! lays out one kind of content region: an argument or a body that a
//! construct of mathematics reads in *text* mode, such as the argument of a
//! text command (`\text{…}`). Such a content region is document content, so it
//! is laid out by its declaration (an argument by its declared layout, a body
//! by its body declaration), with whitespace resolved, typographic substitutions
//! applied, and a paragraph break reported as in any content region laid out
//! inline. Everything else in the mathematics around it stays byte for byte
//! what the author wrote. The parsing mode of a run decides, in both
//! directions: a run parsed in math mode is opaque wherever it is, and a run
//! parsed in text mode inside mathematics is content wherever it is. The
//! exception does not reach inside a verbatim group or an opaque wrapper:
//! there nothing is laid out, in any parsing mode. Reading verbatim content
//! changes no mode, and an opaque wrapper keeps everything under it as
//! written, a formula included: in an argument placed under an opaque wrapper
//! that holds `$\text{x}$`, the argument of `\text` stays as typed.
//!
//! # Which layout each content region gets
//!
//! - The top level of the tree is laid out by the fragment's
//!   [`FragmentLayout`], which the caller passes;
//!   [`InlineOrBlocks`](FragmentLayout::InlineOrBlocks) lays it out as blocks
//!   when it holds a paragraph break or a node of block role `Block`, and
//!   inline otherwise.
//! - An environment's body is laid out by its body declaration
//!   ([`BodyDef`](crate::lang::mint::BodyDef)): as blocks or inline where the
//!   declaration says so; where the declaration follows the element's block
//!   role ([`BodyLayout::FromRole`](crate::lang::mint::BodyLayout), the
//!   default), as blocks for an element whose role is `Block` and inline
//!   otherwise; and opaque when its content stands in a verbatim group or an
//!   opaque wrapper (a verbatim body, a body declared with an opaque wrapper)
//!   or the body is read in math mode. A callable with no FLM body declaration
//!   follows its block role in the same way.
//! - An argument is laid out by its declared layout
//!   ([`ArgLayout`](crate::lang::mint::ArgLayout), read with
//!   [`arg_layout_of`](crate::lang::mint::arg_layout_of)): inline by default;
//!   as blocks where declared so, exactly as a body laid out as blocks; and,
//!   for [`InlineOrBlocks`](crate::lang::mint::ArgLayout::InlineOrBlocks), as
//!   blocks exactly when the content holds a paragraph break or a node of block
//!   role `Block`, which is the same test as for the top level and the result
//!   of [`argument_holds_blocks`](super::argument_holds_blocks). A
//!   [`BlockStructure`] hook takes precedence over this default, as it does
//!   for a body. In an argument laid out inline, a paragraph break and a
//!   block-level node are reported with the context `argument`. An argument
//!   whose content stands in a verbatim group or an opaque wrapper (a verbatim
//!   argument, an argument placed under an opaque wrapper) is opaque, whatever
//!   its declared layout.
//! - The content of a `{…}` group is laid out inline.
//!
//! In every case, a run parsed in math mode is opaque.
//!
//! # Text
//!
//! Adjacent text nodes read in text mode outside opaque regions are merged into
//! one and resolved once: every run of whitespace becomes one space, the
//! whitespace at the edges of a paragraph is removed, and the typographic
//! substitutions that [`BlocksConfig`] enables are applied. An inline node
//! whose element absorbs the whitespace beside it
//! ([`InlineSpacing`](crate::model::aspects::InlineSpacing)) has that
//! whitespace removed, whitespace nodes included (below), even when comments
//! separate the node from the whitespace.
//!
//! The merged node records where its text came from. When its text is a
//! byte-for-byte copy of one range of one source, its span alone says so and
//! it carries no map. Otherwise it holds its own text and a
//! [`TextSourceMap`], which records the source range of each part of the text.
//! A text node whose text does not change is copied with its extension, source
//! map included.
//!
//! ## Whitespace nodes
//!
//! A **whitespace node** is a node whose element carries the
//! [`Whitespace`](crate::model::aspects::Whitespace) aspect, such as the
//! literals `~` and `\ `. It is whitespace to every rule of the layout step,
//! and stays a node of its own:
//!
//! - the whitespace removed beside an absorbing node and at the edges of a
//!   paragraph is the whole stretch of whitespace there, up to the nearest
//!   content: whitespace-only text and whitespace nodes alike are removed, and
//!   the comments among them are kept;
//! - a whitespace node is not paragraph content: a stretch holding only
//!   whitespace and comments starts no paragraph;
//! - a straight quote right after a whitespace node opens, as it does after a
//!   space;
//! - a whitespace node is never merged into the text beside it and never
//!   collapsed with the whitespace around it: `a~~b` keeps its two nodes, and
//!   `a~ b` the node and the space. A whitespace node that is not removed is
//!   staged like any other inline node, and lowered by its element's own rule.
//!
//! # Block-structure hooks
//!
//! Before the default handling of a node, the layout step calls the
//! [`BlockStructure`] hook of the node's element, if it has one, and hands
//! itself to the hook as the [`BlockStructurer`]. The hook runs while the
//! paragraph around its node is being built, so it lays out its own node's
//! content and never the paragraph around it; this is why cutting a
//! block-transparent node is done by the cut step, before any paragraph exists.
//! A node that a hook creates and never places is reported
//! ([`DecoratorWithoutHome`]) and dropped. A block decorator that was in the
//! pass's *input* tree is laid out by its block role like any other node.
//!
//! # The recursion guard and the worker thread
//!
//! Both walks run under the same [recursion guard](crate::lang::guard), so that
//! content nested too deeply is refused with [`StructureError::NestingTooDeep`]
//! (`flm.blocks.nesting-too-deep`, and no tree) instead of overflowing the
//! stack. The guard limits how much stack the walk uses, which is safe only on
//! a thread with a large enough stack, so both entry points run the pass on a
//! [worker thread](crate::lang::guard::on_worker_thread), and create one when
//! the caller is not on one.
//!
//! # Further rules of the layout step
//!
//! - A tree whose root is not a container node (a snippet, a synthesized
//!   value) is laid out as a content region of one node; where that yields
//!   several nodes, they are wrapped in a container node created by the pass,
//!   so that the result still has one root.
//! - Every node slot of a callable (techy's `ParsedSlot`, a part of its content
//!   outside its arguments, such as an environment's body) is laid out with
//!   the layout of the element's body declaration (or the block-role
//!   default); the pass lays out every slot the
//!   same way, whether or not it is the body, and regardless of its
//!   [slot role](techy::core::node::SlotRole) (content, attached or hidden).
//! - A paragraph's span is the union of its children's spans in the source of
//!   its first child with a non-empty span, so that a paragraph that begins
//!   with a created decorator (whose span is empty, on a synthesized source)
//!   still points at real content. Its parsing state is that of its first
//!   child that is kept: blank text and whitespace nodes at the head are
//!   removed, a comment is kept.
//! - A [`Leads`](Placement::Leads) or [`Trails`](Placement::Trails) decorator
//!   in a content region laid out inline (a `{…}` group, an argument, a
//!   fragment laid out inline) is placed like any inline node and is not
//!   reported, because such a placement has legitimate uses.
//! - A stretch of content whose layout yields no node produces no paragraph,
//!   so the pass never builds an empty paragraph, which the
//!   [checker](super::check) would reject.

// Why the run's parsing mode alone decides whether a region is opaque, in both
// directions, and why a text region inside mathematics is not a property the
// element declares: see decision rationale at
// [§flm-dd-dr:lang-blocks-text-in-opaque].

// Why the pass drives its own descent: techy's restager has no exit callback, so
// a region context cannot ride `Restage::Descend`; the visitor almost always
// emits, laying out the region's content itself and restaging the subtrees it
// wants through the context. Region *wrappers* (an argument's `{…}` group, a
// body's list) are never restaged directly — techy carries a record's content
// designation verbatim through a whole-node emit and refuses a changed child
// count (`ContentOutOfBounds`, `ContentParentDropped`); the pass builds a
// region's new content and hands it to `restage_argument_with_content` /
// `restage_slot_with_content`, then reassembles the callable with
// `restage_invocation`. `Descend` survives only inside opaque regions.

// The guard and the content-swap copies: techy's `_with_content` helpers copy a
// region's wrapper and its noise verbatim, and that copy recurses between two
// guard checks (techy's documented caveat). In FLM's grammar what they copy is
// leaves only: an argument's pre-space, the whitespace between arguments, the
// newline techy gobbles after a verbatim environment's begin, the wrapper group
// or body list itself, and the opaque wrapper over it. The content subtree is
// replaced, never copied, and the interiors of math groups, verbatim groups and
// opaque wrappers descend through `restage_children`, which the guard checks
// per level. So no document input can exhaust the stack
// inside such a copy; revisit only if a construct ever designates content below
// a deep wrapper chain. See the design rationale at
// [§flm-dd-dr:lang-nested-restage-budget].

use std::ops::Range;
use std::sync::Arc;

use techy::core::node::{
    BodySlotExt, BuildId, GroupData, NodeKind, NodeRef, NodeTree, ParsedArgument,
};
use techy::core::{Lang, ParsingState};
use techy::error::Diagnostic;
use techy::latexlike::Mode;
use techy::source::SourceSpan;
use techy::transform::{Restage, RestageVisitor, RestagedArgument, RestagedSlot};

use super::config::BlocksConfig;
use super::cut;
use super::error::{BlockInInline, DecoratorWithoutHome, ParbreakInInline, StructureError};
use super::hook::{
    BlockStructure, BlockStructurer, RegionLayout, StructureCx, Structuring, SynthesizedNode,
};
use super::predicates::{
    Opacity, Region, argument_layout, group_opacity, is_content, is_math_run, is_opaque_group,
    is_parbreak, run_holds_blocks, run_opacity, slot_layout,
};
use super::text::{Edges, Tape, resolve};
use crate::model::aspects::{BlockRole, Placement, block_role_of, is_whitespace_of};
use crate::model::doc::FragmentLayout;
use crate::model::element::{ElementSpec, element_of};
use crate::lang::ext::TextSourceMap;
use crate::lang::flm::{Flm, FlmGroupType};
use crate::lang::guard::{self, GuardPolicy};
use crate::lang::mint::reads_text_in_math;
use crate::lang::origin::{FlmDiagnostic, FlmDiagnostics, FlmSpan};
use crate::lang::text::is_blank_text;

// --- entry points ---------------------------------------------------------------------

/// Runs the block-structure pass over `tree` and returns the structured tree,
/// under the default recursion-guard policy, [`GuardPolicy::StackBudget`] (a
/// fixed budget of stack for the walk).
///
/// `layout` is the layout of the tree's top level, normally the layout of the
/// fragment the tree was parsed from
/// ([`Fragment::layout`](crate::model::doc::Fragment::layout)). `config` holds
/// the options of text resolution. The conditions the pass reports and
/// recovers from ([`BlockInInline`], [`ParbreakInInline`],
/// [`DecoratorWithoutHome`], and those the elements' aspects report) are
/// pushed onto `diags`, and the tree is still returned.
///
/// The pass is a plain function over a tree, so it can be called on any tree:
/// a fragment's parsed or expanded tree, a snippet, a synthesized value, or a
/// tree the pass itself produced. It
/// runs on a [worker thread](crate::lang::guard::on_worker_thread): on the
/// calling thread when that thread is already one, otherwise on a thread
/// created for the call.
///
/// # Errors
///
/// A [`StructureError`] means that there is no structured tree:
/// [`NestingTooDeep`](StructureError::NestingTooDeep) when the content is
/// nested more deeply than the guard allows,
/// [`WorkerThread`](StructureError::WorkerThread) when no worker thread could be
/// created, and the other variants when the pass or an element's aspect broke
/// an invariant of techy's tree builder or reported a failure of its own. The
/// caller records [`StructureError::diagnostic`] in place of the tree.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::model::doc::FragmentLayout;
/// use flm_core::lang::rewrites::blocks::{structure, BlocksConfig};
/// use flm_core::lang::{
///     build_language, parse_fragment, FlmDiagnostics, FlmSource, LangInputs, ParseOptions,
/// };
///
/// let lang = build_language(LangInputs::new());
/// let source = Arc::new(FlmSource::new("First `paragraph\'.\n\nSecond one."));
/// let fragment = parse_fragment(
///     &lang, source, FragmentLayout::Blocks, &ParseOptions::default(),
/// ).expect("the input is shallow enough to parse");
///
/// let mut diags = FlmDiagnostics::new();
/// let tree = structure(
///     fragment.tree(), FragmentLayout::Blocks, &BlocksConfig::new(), &mut diags,
/// ).expect("the input is shallow enough to structure");
///
/// // The blank line became a paragraph boundary, and the marker is gone.
/// assert_eq!(tree.root().child_count(), 2);
/// // Text was resolved in the same pass: the TeX quotes are real quotes now.
/// let texts: Vec<String> = tree.root().descendants()
///     .filter_map(|node| node.chars().map(str::to_string))
///     .collect();
/// assert!(texts.iter().any(|text| text == "First \u{2018}paragraph\u{2019}."));
/// assert!(!diags.has_errors());
/// ```
///
/// See also [`structure_with_guard`], to choose the guard policy, and
/// [`check_block_contract`](super::check::check_block_contract), which checks
/// a structured tree against the rules this pass establishes.
pub fn structure(
    tree: &NodeTree<Flm>,
    layout: FragmentLayout,
    config: &BlocksConfig,
    diags: &mut FlmDiagnostics,
) -> Result<Arc<NodeTree<Flm>>, StructureError> {
    structure_with_guard(tree, layout, config, GuardPolicy::StackBudget, diags)
}

/// Runs the block-structure pass as [`structure`] does, under the
/// recursion-guard policy `guard`.
///
/// Pass the built language's policy
/// ([`BuiltLang::guard`](crate::lang::BuiltLang::guard)), as the `blocks`
/// feature's rewrite stage does, or a depth limit
/// ([`GuardPolicy::DepthLimit`]), which refuses at the same nesting depth on
/// every platform and so makes a test of the refusal reproducible.
///
/// # Errors
///
/// The same as [`structure`].
pub fn structure_with_guard(
    tree: &NodeTree<Flm>,
    layout: FragmentLayout,
    config: &BlocksConfig,
    guard: GuardPolicy,
    diags: &mut FlmDiagnostics,
) -> Result<Arc<NodeTree<Flm>>, StructureError> {
    let run = || structure_on_worker_thread(tree, layout, config, guard, diags);
    match guard::on_worker_thread(run) {
        Ok(result) => result,
        Err(error) => Err(StructureError::WorkerThread {
            detail: error.to_string(),
        }),
    }
}

/// The pass itself, on a thread that meets the worker-thread requirement.
fn structure_on_worker_thread(
    tree: &NodeTree<Flm>,
    layout: FragmentLayout,
    config: &BlocksConfig,
    policy: GuardPolicy,
    diags: &mut FlmDiagnostics,
) -> Result<Arc<NodeTree<Flm>>, StructureError> {
    let cut = cut::cut(tree, policy, diags)?;
    let mut pass = Pass::new(layout, config, diags);
    match guard::restager(&mut pass, policy).restage(&cut) {
        Ok(out) => Ok(Arc::new(out)),
        Err(error) => Err(StructureError::from(error)),
    }
}

// --- the pass -------------------------------------------------------------------------

/// Whether a run's edges are trimmed: yes in a paragraph, no in a content region
/// laid out inline
/// (`x\emph{ a }y` keeps its edge spaces).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Trim {
    head: bool,
    tail: bool,
}

impl Trim {
    const BOTH: Trim = Trim {
        head: true,
        tail: true,
    };
    const NONE: Trim = Trim {
        head: false,
        tail: false,
    };
}

/// A node a hook created through
/// [`BlockStructurer::synthesize_callable`], waiting to be placed in a run.
struct Synthesized {
    id: techy::core::node::NodeId,
    element: Arc<ElementSpec>,
    placed: bool,
}

/// The layout step: the visitor of the restage, and the [`BlockStructurer`]
/// the hooks receive.
struct Pass<'a> {
    layout: FragmentLayout,
    config: &'a BlocksConfig,
    diags: &'a mut FlmDiagnostics,
    region: Region,
    /// Whether the next visited node is the tree's root (the driver's first
    /// call); consumed by that call, so a root laid out as a content region of one
    /// node is not treated as the root again when that run is staged.
    at_root: bool,
    synthesized: Vec<Synthesized>,
}

impl<'a> Pass<'a> {
    fn new(
        layout: FragmentLayout,
        config: &'a BlocksConfig,
        diags: &'a mut FlmDiagnostics,
    ) -> Pass<'a> {
        Pass {
            layout,
            config,
            diags,
            region: Region::Blocks,
            at_root: true,
            synthesized: Vec::new(),
        }
    }
}

impl RestageVisitor<Flm, (), ()> for Pass<'_> {
    type Error = StructureError;

    fn restage(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut StructureCx<'_>,
    ) -> Result<Restage<()>, StructureError> {
        if self.at_root {
            self.at_root = false;
            return Ok(Restage::Emit(self.root(cx, node)?));
        }
        self.note_placed(&node);
        // Opaque regions are carried over verbatim: no hook, no layout, no text
        // resolution. The one exception is a construct of mathematics that
        // reads a region of its own in text mode — a text command, whose
        // argument is document content: that region is laid out like any
        // other, and the rest of the node is carried over unchanged. The
        // exception holds in mathematics only; a verbatim group or an opaque
        // wrapper met there opens a region opaque in any mode, where it does
        // not apply.
        match self.region {
            Region::Raw(Opacity::AnyMode) => return Ok(Restage::Descend(())),
            Region::Raw(Opacity::Math) => {
                if is_opaque_group(&node) {
                    return Ok(Restage::Emit(self.group_node(cx, node)?));
                }
                if reads_text_in_math(&node) {
                    return Ok(Restage::Emit(self.callable_node(cx, node)?));
                }
                return Ok(Restage::Descend(()));
            }
            Region::Blocks | Region::Inline => {}
        }
        if let Some(hook) = element_of(&node).aspect::<dyn BlockStructure>() {
            let mark = self.synthesized.len();
            let ids = match hook.structure(node, cx, self)? {
                Structuring::Emit(ids) => ids,
                Structuring::Default => self.default_node(cx, node)?,
            };
            self.report_homeless(mark, &node);
            return Ok(Restage::Emit(ids));
        }
        Ok(Restage::Emit(self.default_node(cx, node)?))
    }
}

impl Pass<'_> {
    // -- the root ----------------------------------------------------------------------

    /// The top level: a container root's children laid out by the fragment
    /// layout and the root rebuilt over them; any other root laid out as a
    /// content region of one node and wrapped when needed.
    fn root(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, StructureError> {
        if node.is_list() {
            let run: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
            let layout = self.root_layout(&run);
            let out = self.lay_out(cx, &run, layout, "fragment")?;
            return Ok(vec![copy_over(cx, &node, out)?]);
        }
        let run = vec![node];
        let layout = self.root_layout(&run);
        let out = self.lay_out(cx, &run, layout, "fragment")?;
        if out.len() == 1 {
            return Ok(out);
        }
        let kind = NodeKind::list();
        let span = node.span().clone();
        let state = Arc::clone(node.parsing_state());
        Ok(vec![mint(cx, kind, span, state, out)?])
    }

    /// The layout of the top level: the fragment's layout, with
    /// `InlineOrBlocks` meaning blocks when the run holds a paragraph break or
    /// a block-level node, the same test as for an argument declared
    /// `InlineOrBlocks`.
    fn root_layout(&self, run: &[NodeRef<'_, Flm>]) -> Region {
        match self.layout {
            FragmentLayout::Blocks => Region::Blocks,
            FragmentLayout::Inline => Region::Inline,
            FragmentLayout::InlineOrBlocks if run_holds_blocks(run) => Region::Blocks,
            FragmentLayout::InlineOrBlocks => Region::Inline,
        }
    }

    // -- regions -------------------------------------------------------------------------

    /// Lays out a run with the given layout (opaque as mathematics, when
    /// `layout` is not opaque already and the run's nodes were parsed in math
    /// mode), and restores the current region layout afterwards. `context` is
    /// the kind of content region that a `parbreak-in-inline` diagnostic
    /// reports.
    ///
    /// A run opaque in any mode comes with that layout already: the layout of
    /// an argument or a slot says so ([`argument_layout`], [`slot_layout`]),
    /// a hook's run is tested ([`hook_layout`]), and the content of a `{…}`
    /// group is never opaque in any mode, since the walk enters no region
    /// under a verbatim group or an opaque wrapper.
    fn lay_out(
        &mut self,
        cx: &mut StructureCx<'_>,
        run: &[NodeRef<'_, Flm>],
        layout: Region,
        context: &'static str,
    ) -> Result<Vec<BuildId>, StructureError> {
        let layout = match layout {
            Region::Blocks | Region::Inline if is_math_run(run) => Region::Raw(Opacity::Math),
            layout => layout,
        };
        let saved = self.region;
        self.region = layout;
        let result = match layout {
            Region::Blocks => self.blocks(cx, run),
            Region::Inline => self.inline(cx, run, Trim::NONE, context),
            Region::Raw(_) => self.raw(cx, run),
        };
        self.region = saved;
        result
    }

    /// Copies a run unchanged, node for node (the current layout is already
    /// [`Region::Raw`], so every visit descends, except at the nodes that
    /// [`restage`](RestageVisitor::restage) singles out inside mathematics).
    fn raw(
        &mut self,
        cx: &mut StructureCx<'_>,
        run: &[NodeRef<'_, Flm>],
    ) -> Result<Vec<BuildId>, StructureError> {
        let mut out = Vec::new();
        for node in run {
            out.extend(cx.restage_subtree(*node, self)?);
        }
        Ok(out)
    }

    /// The default handling of one node reached through the visitor. It is
    /// never called while the current layout is raw (a raw visit descends
    /// before reaching it), so the `Raw` arms below only complete the match.
    fn default_node(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, StructureError> {
        match node.kind() {
            // Reached only at region level (argument noise): carried over.
            NodeKind::Chars { .. } | NodeKind::Comment(_) => Ok(vec![copy_leaf(cx, &node)?]),
            NodeKind::Group(_) => self.group_node(cx, node),
            NodeKind::List => {
                // A list that is not the root: a multi-node argument value, a body
                // reached outside the slot route — laid out in the current region.
                let run: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
                let layout = match self.region {
                    Region::Blocks => Region::Blocks,
                    Region::Inline | Region::Raw(_) => Region::Inline,
                };
                let out = self.lay_out(cx, &run, layout, "list")?;
                Ok(vec![copy_over(cx, &node, out)?])
            }
            NodeKind::Callable(_) => self.callable_node(cx, node),
        }
    }

    /// A group: the content of a verbatim group or of an opaque wrapper is an
    /// opaque region of the kind [`Opacity::AnyMode`], the content of a
    /// mathematics group one of the kind [`Opacity::Math`]
    /// ([`group_opacity`]); the content of any other group is laid out inline.
    fn group_node(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, StructureError> {
        let children = if let Some(opacity) = group_opacity(&node) {
            let saved = self.region;
            self.region = Region::Raw(opacity);
            let children = cx.restage_children(node, self);
            self.region = saved;
            children?
        } else {
            let run: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
            self.lay_out(cx, &run, Region::Inline, "group")?
        };
        Ok(vec![copy_over(cx, &node, children)?])
    }

    /// A callable: the arguments laid out by their declared layouts, the
    /// node slots (techy's `ParsedSlot`, the parts of its content outside the
    /// arguments) by the body declaration, and the node rebuilt around them.
    fn callable_node(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, StructureError> {
        let arguments = self.arguments(cx, node)?;
        let slots = self.slots(cx, node, None)?;
        Ok(vec![cx.restage_invocation(node, arguments, slots, ())?])
    }

    /// The staged arguments of a callable, one per declared argument: an
    /// argument that was not written stays absent, and a written one has its
    /// content laid out by its declared layout (or copied unchanged, when it
    /// is opaque) and put back between its original delimiters.
    fn arguments(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: NodeRef<'_, Flm>,
    ) -> Result<Vec<RestagedArgument<Flm>>, StructureError> {
        let records: Vec<&ParsedArgument<Flm>> = node
            .arguments()
            .map(|arguments| arguments.iter().collect())
            .unwrap_or_default();
        let mut out = Vec::with_capacity(records.len());
        for (i, record) in records.iter().enumerate() {
            if !record.is_provided() {
                out.push(RestagedArgument::absent(Arc::clone(&record.spec)));
                continue;
            }
            let content: Vec<NodeRef<'_, Flm>> = node
                .argument_content_nodes(i)
                .map(|slice| slice.iter().collect())
                .unwrap_or_default();
            let layout = argument_layout(&node, i, &content);
            let ids = self.lay_out(cx, &content, layout, "argument")?;
            out.push(cx.restage_argument_with_content(node, i, ids, ())?);
        }
        Ok(out)
    }

    /// The staged slots of a callable: every slot's content laid out with the
    /// body layout (or copied unchanged, when it is opaque), except the body
    /// slot when `body` supplies its new content already staged
    /// ([`BlockStructurer::with_body`]).
    fn slots(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: NodeRef<'_, Flm>,
        mut body: Option<Vec<BuildId>>,
    ) -> Result<Vec<RestagedSlot<Flm>>, StructureError> {
        let count = node.slots().map_or(0, |slots| slots.len());
        let mut out = Vec::with_capacity(count);
        for i in 0..count {
            let is_body = node
                .slots()
                .and_then(|slots| slots.get(i))
                .is_some_and(|slot| slot.ext.is_body());
            if is_body {
                if let Some(ids) = body.take() {
                    out.push(cx.restage_slot_with_content(node, i, ids, ())?);
                    continue;
                }
            }
            let content: Vec<NodeRef<'_, Flm>> = node
                .slot_content_nodes(i)
                .map(|slice| slice.iter().collect())
                .unwrap_or_default();
            let ids = self.lay_out(cx, &content, slot_layout(&node, i), "body")?;
            out.push(cx.restage_slot_with_content(node, i, ids, ())?);
        }
        if body.is_some() {
            return Err(StructureError::NoBody {
                element: element_of(&node).name.clone(),
            });
        }
        Ok(out)
    }

    // -- blocks layout -------------------------------------------------------------------

    /// Lays out a run as blocks: a paragraph for each maximal stretch of inline
    /// content, block-level nodes on their own, markers removed, comments kept,
    /// decorators placed by their `Placement`.
    fn blocks(
        &mut self,
        cx: &mut StructureCx<'_>,
        run: &[NodeRef<'_, Flm>],
    ) -> Result<Vec<BuildId>, StructureError> {
        let mut out: Vec<BuildId> = Vec::new();
        let mut current: Vec<NodeRef<'_, Flm>> = Vec::new();
        for node in run {
            if is_parbreak(node) {
                self.close_paragraph(cx, &mut current, &mut out)?;
                continue;
            }
            if node.is_comment() {
                current.push(*node);
                continue;
            }
            match block_role_of(node) {
                BlockRole::Block => {
                    self.close_paragraph(cx, &mut current, &mut out)?;
                    let saved = self.region;
                    self.region = Region::Blocks;
                    let ids = cx.restage_subtree(*node, self);
                    self.region = saved;
                    out.extend(ids?);
                }
                BlockRole::BlockDecorator(Placement::Leads) => {
                    // First in its paragraph: inline content before it closes the
                    // running paragraph (comments and whitespace are not content).
                    if current.iter().any(is_content) {
                        self.close_paragraph(cx, &mut current, &mut out)?;
                    }
                    current.push(*node);
                }
                BlockRole::BlockDecorator(Placement::Trails) => {
                    current.push(*node);
                    self.close_paragraph(cx, &mut current, &mut out)?;
                }
                BlockRole::BlockDecorator(Placement::Free) | BlockRole::Inline(_) => {
                    current.push(*node);
                }
            }
        }
        self.close_paragraph(cx, &mut current, &mut out)?;
        Ok(out)
    }

    /// Ends the pending paragraph: a run with content becomes one paragraph; a
    /// run of only whitespace (text and whitespace nodes) and comments adds its
    /// comments to the content region and removes the whitespace; a run whose
    /// layout yields nothing produces no paragraph.
    fn close_paragraph(
        &mut self,
        cx: &mut StructureCx<'_>,
        current: &mut Vec<NodeRef<'_, Flm>>,
        out: &mut Vec<BuildId>,
    ) -> Result<(), StructureError> {
        if current.is_empty() {
            return Ok(());
        }
        if !current.iter().any(is_content) {
            for node in current.drain(..) {
                if node.is_comment() {
                    out.push(copy_leaf(cx, &node)?);
                }
            }
            return Ok(());
        }
        let span = run_span(current);
        // The first kept child's state: a blank head run is dropped by the
        // trim, a comment or content is kept.
        let first_kept = current
            .iter()
            .find(|node| node.is_comment() || is_content(node))
            .unwrap_or(&current[0]);
        let state = Arc::clone(first_kept.parsing_state());
        let saved = self.region;
        self.region = Region::Inline;
        let children = self.inline(cx, current, Trim::BOTH, "paragraph");
        self.region = saved;
        let children = children?;
        current.clear();
        if children.is_empty() {
            return Ok(());
        }
        let kind = NodeKind::group(GroupData::new(FlmGroupType::Paragraph, "", ""));
        out.push(mint(cx, kind, span, state, children)?);
        Ok(())
    }

    // -- inline layout -------------------------------------------------------------------

    /// Lays out a run as inline content: adjacent text merged and resolved,
    /// whitespace nodes removed where the whitespace of their place is
    /// ([`whitespace_removed`]), comments and other nodes staged in place.
    fn inline(
        &mut self,
        cx: &mut StructureCx<'_>,
        run: &[NodeRef<'_, Flm>],
        trim: Trim,
        context: &'static str,
    ) -> Result<Vec<BuildId>, StructureError> {
        // 1. Classify.
        let mut items: Vec<Item<'_>> = Vec::with_capacity(run.len());
        for node in run {
            if is_parbreak(node) {
                self.diags.push(Diagnostic::error(
                    ParbreakInInline::new(context),
                    node.span().clone(),
                ));
                items.push(Item::Space(*node));
            } else if node.is_comment() {
                items.push(Item::Comment(*node));
            } else if node.is_chars() && !is_raw_chars(node) {
                items.push(Item::Text(*node));
            } else if is_whitespace_of(node) {
                items.push(Item::Whitespace(*node));
            } else {
                let role = block_role_of(node);
                if matches!(role, BlockRole::Block) {
                    let condition = BlockInInline::new(element_of(node).name.as_str());
                    self.diags
                        .push(Diagnostic::error(condition, node.span().clone()));
                }
                items.push(Item::Node(*node, role));
            }
        }

        // 2. Emit, merging every maximal run of text and recovery spaces. A
        // whitespace node is an item of its own, never merged into a run.
        let mut out: Vec<BuildId> = Vec::new();
        let mut i = 0usize;
        while i < items.len() {
            match &items[i] {
                Item::Text(_) | Item::Space(_) => {
                    let start = i;
                    while i < items.len() && matches!(items[i], Item::Text(_) | Item::Space(_)) {
                        i += 1;
                    }
                    if let Some(id) = self.emit_text(cx, &items, start..i, trim)? {
                        out.push(id);
                    }
                }
                Item::Comment(node) => {
                    out.push(copy_leaf(cx, node)?);
                    i += 1;
                }
                Item::Whitespace(_) if whitespace_removed(&items, i, trim) => i += 1,
                Item::Whitespace(node) | Item::Node(node, _) => {
                    let saved = self.region;
                    self.region = Region::Inline;
                    let ids = cx.restage_subtree(*node, self);
                    self.region = saved;
                    out.extend(ids?);
                    i += 1;
                }
            }
        }
        Ok(out)
    }

    /// Stages the merged text node of `items[range]`, or returns `None` when
    /// the text resolves to nothing.
    fn emit_text(
        &mut self,
        cx: &mut StructureCx<'_>,
        items: &[Item<'_>],
        range: Range<usize>,
        trim: Trim,
    ) -> Result<Option<BuildId>, StructureError> {
        let edges = edges_of(items, range.clone(), trim);

        // The tape: the run's text with its provenance.
        let mut tape = Tape::default();
        let mut single: Option<NodeRef<'_, Flm>> = None;
        let mut state: Option<&Arc<ParsingState<Flm>>> = None;
        for item in &items[range.clone()] {
            match item {
                Item::Text(node) => {
                    single = if tape.is_empty() { Some(*node) } else { None };
                    let text = node.chars().unwrap_or("");
                    // An already-resolved node lays its own pieces on the tape,
                    // so its exact provenance survives a merge or an edit.
                    let laid = node
                        .ext()
                        .source_map
                        .as_deref()
                        .is_some_and(|map| tape.push_mapped(text, map));
                    if !laid {
                        let exact = node.span().content() == text;
                        tape.push(text, node.span().clone(), exact);
                    }
                    state.get_or_insert(node.parsing_state());
                }
                Item::Space(node) => {
                    single = None;
                    // The recovery space of a paragraph break: one output byte
                    // standing for the whole marker.
                    tape.push(" ", node.span().clone(), false);
                    state.get_or_insert(node.parsing_state());
                }
                Item::Comment(_) | Item::Whitespace(_) | Item::Node(..) => {}
            }
        }
        let Some(state) = state else {
            return Ok(None);
        };

        let resolved = resolve(&tape, self.config, edges);
        if resolved.text.is_empty() {
            return Ok(None);
        }

        // A single node whose text did not change is carried over verbatim —
        // its content as it was (span-backed or owned) and its extension, source
        // map included, so re-resolving resolved text keeps its provenance.
        if let Some(node) = single {
            if resolved.text == node.chars().unwrap_or("") {
                return Ok(Some(copy_leaf(cx, &node)?));
            }
        }

        let Some(span) = resolved.merged_span() else {
            return Ok(None);
        };
        // One byte-identical image of a source range needs no map: the node's own
        // (narrowed) span is the account, and the content stays span-backed.
        let map_is_none = resolved.is_one_source_image();
        let kind = if map_is_none {
            NodeKind::chars(resolved.pieces[0].span.span())
        } else {
            NodeKind::chars(resolved.text.clone())
        };
        let state = Arc::clone(state);
        let builder = cx.builder();
        let mut ext = Flm::make_node_ext(&kind, &span, &state, builder.staged_children(&[]))?;
        if !map_is_none {
            ext.source_map = Some(Arc::new(TextSourceMap::new(resolved.pieces)));
        }
        Ok(Some(builder.add(kind, span, state, Vec::new(), ext, ())?))
    }

    // -- synthesized nodes ----------------------------------------------------------------

    /// Marks a created node as placed when the visitor reaches it.
    fn note_placed(&mut self, node: &NodeRef<'_, Flm>) {
        if self.synthesized.is_empty() {
            return;
        }
        let id = node.id();
        if let Some(entry) = self.synthesized.iter_mut().find(|entry| entry.id == id) {
            entry.placed = true;
        }
    }

    /// Reports every node that the hook of `hook_node` created since `mark` and
    /// never placed, then forgets all the nodes created since `mark`.
    fn report_homeless(&mut self, mark: usize, hook_node: &NodeRef<'_, Flm>) {
        let homeless: Vec<Arc<ElementSpec>> = self
            .synthesized
            .drain(mark..)
            .filter(|entry| !entry.placed)
            .map(|entry| entry.element)
            .collect();
        for element in homeless {
            let condition = DecoratorWithoutHome::new(
                element.name.as_str(),
                element_of(hook_node).name.as_str(),
            );
            self.diags
                .push(Diagnostic::error(condition, hook_node.span().clone()));
        }
    }
}

// --- the façade ---------------------------------------------------------------------------

impl BlockStructurer for Pass<'_> {
    fn build_blocks(
        &mut self,
        cx: &mut StructureCx<'_>,
        run: &[NodeRef<'_, Flm>],
    ) -> Result<Vec<BuildId>, StructureError> {
        self.lay_out(cx, run, hook_layout(run, Region::Blocks), "run")
    }

    fn build_inline(
        &mut self,
        cx: &mut StructureCx<'_>,
        run: &[NodeRef<'_, Flm>],
    ) -> Result<Vec<BuildId>, StructureError> {
        self.lay_out(cx, run, hook_layout(run, Region::Inline), "run")
    }

    fn default(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: &NodeRef<'_, Flm>,
    ) -> Result<Vec<BuildId>, StructureError> {
        self.default_node(cx, *node)
    }

    fn body_run<'t>(
        &mut self,
        node: &NodeRef<'t, Flm>,
    ) -> Result<Vec<NodeRef<'t, Flm>>, StructureError> {
        match node.body() {
            Some(slice) => Ok(slice.iter().collect()),
            None => Err(StructureError::NoBody {
                element: element_of(node).name.clone(),
            }),
        }
    }

    fn with_body(
        &mut self,
        cx: &mut StructureCx<'_>,
        node: &NodeRef<'_, Flm>,
        body: Vec<BuildId>,
    ) -> Result<BuildId, StructureError> {
        if node.slots().is_none() {
            return Err(StructureError::NoBody {
                element: element_of(node).name.clone(),
            });
        }
        let arguments = self.arguments(cx, *node)?;
        let slots = self.slots(cx, *node, Some(body))?;
        Ok(cx.restage_invocation(*node, arguments, slots, ())?)
    }

    fn synthesize_callable(
        &mut self,
        node: &NodeRef<'_, Flm>,
        element: &Arc<ElementSpec>,
    ) -> Result<SynthesizedNode, StructureError> {
        let minted = SynthesizedNode::mint(node, element)?;
        self.synthesized.push(Synthesized {
            id: minted.node().id(),
            element: Arc::clone(element),
            placed: false,
        });
        Ok(minted)
    }

    fn diagnose(&mut self, diagnostic: FlmDiagnostic) {
        self.diags.push(diagnostic);
    }

    /// A hook is never called inside an opaque region, so `Raw` maps to
    /// inline only to complete the match.
    fn region(&self) -> RegionLayout {
        match self.region {
            Region::Blocks => RegionLayout::Blocks,
            Region::Inline | Region::Raw(_) => RegionLayout::Inline,
        }
    }
}

/// The layout of a run that a hook asks to lay out as `layout`: raw when the
/// run is an opaque region ([`run_opacity`]: a run that stands in a verbatim
/// group or an opaque wrapper, or was parsed in math mode), `layout`
/// otherwise.
fn hook_layout(run: &[NodeRef<'_, Flm>], layout: Region) -> Region {
    run_opacity(run).map_or(layout, Region::Raw)
}

// --- run items and helpers ----------------------------------------------------------------

/// One item of an inline run, after classification.
enum Item<'t> {
    /// Text read in text mode, outside verbatim content and opaque wrappers:
    /// goes on the merge tape.
    Text(NodeRef<'t, Flm>),
    /// A paragraph-break marker found in inline content: one space on the
    /// tape.
    Space(NodeRef<'t, Flm>),
    /// A comment: kept as it is; it separates the text before it from the text
    /// after it, and trimming looks through it.
    Comment(NodeRef<'t, Flm>),
    /// A whitespace node ([`is_whitespace_of`]): blank, never merged into the
    /// text beside it, and staged in place unless the whitespace of its place
    /// is removed ([`whitespace_removed`]).
    Whitespace(NodeRef<'t, Flm>),
    /// Anything else, with its role.
    Node(NodeRef<'t, Flm>, BlockRole),
}

/// Whether an item is blank: text that is only whitespace, the space read for
/// a paragraph break, or a whitespace node.
fn is_blank(item: &Item<'_>) -> bool {
    match item {
        Item::Text(node) => node.chars().is_none_or(is_blank_text),
        Item::Space(_) | Item::Whitespace(_) => true,
        Item::Comment(_) | Item::Node(..) => false,
    }
}

/// How the whitespace at the edges of `items[range]` is treated, and what
/// precedes the range, for a run laid out under `trim`.
///
/// An edge's whitespace is dropped beside an inline node that absorbs the
/// whitespace toward the range, and at an edge of a paragraph that `trim`
/// trims. Both look past comments and blank items ([`is_blank`]), so every
/// blank item of the stretch between the edge and the nearest other item
/// goes, and the comments among them stay.
fn edges_of(items: &[Item<'_>], range: Range<usize>, trim: Trim) -> Edges {
    // Absorption: the neighbouring element asked for the whitespace beside it
    // to be removed outright. It sees through comments, and through the
    // blank items between a comment and the element, as the trimming of the
    // edges below does.
    let passed_over = |item: &&Item<'_>| matches!(item, Item::Comment(_)) || is_blank(item);
    let absorb_left = match items[..range.start].iter().rev().find(|item| !passed_over(item)) {
        Some(Item::Node(_, BlockRole::Inline(spacing))) => spacing.absorb_after,
        _ => false,
    };
    let absorb_right = match items[range.end..].iter().find(|item| !passed_over(item)) {
        Some(Item::Node(_, BlockRole::Inline(spacing))) => spacing.absorb_before,
        _ => false,
    };
    // Trimming sees through comments, through the decorator leading
    // (trailing) the paragraph, and through blank items — whitespace-only
    // text, recovery spaces and whitespace nodes between the edge and this
    // range are themselves dropped as edge whitespace.
    let transparent_before = |item: &Item<'_>| {
        is_blank(item)
            || matches!(
                item,
                Item::Comment(_) | Item::Node(_, BlockRole::BlockDecorator(Placement::Leads))
            )
    };
    let transparent_after = |item: &Item<'_>| {
        is_blank(item)
            || matches!(
                item,
                Item::Comment(_) | Item::Node(_, BlockRole::BlockDecorator(Placement::Trails))
            )
    };
    let trim_left = trim.head && items[..range.start].iter().all(transparent_before);
    let trim_right = trim.tail && items[range.end..].iter().all(transparent_after);
    // A whitespace node before the range is whitespace to the quote-position
    // test, not a content sibling.
    let prev_is_content = items[..range.start]
        .iter()
        .rev()
        .find(|item| !matches!(item, Item::Comment(_)))
        .is_some_and(|item| matches!(item, Item::Node(..)));
    Edges {
        drop_leading: trim_left || absorb_left,
        drop_trailing: trim_right || absorb_right,
        prev_is_content,
    }
}

/// Whether the whitespace node `items[index]` is removed. It is all
/// whitespace, so it is removed exactly where a run of whitespace-only text in
/// its place would be: when the whitespace at either edge of its place is
/// dropped ([`edges_of`]), beside an absorbing node or at a trimmed edge of a
/// paragraph.
fn whitespace_removed(items: &[Item<'_>], index: usize, trim: Trim) -> bool {
    let edges = edges_of(items, index..index + 1, trim);
    edges.drop_leading || edges.drop_trailing
}

/// Whether text resolution must leave this text node alone: text read in math
/// mode, or a child of a group opaque in any mode (a verbatim group, an opaque
/// wrapper).
///
/// The layout step never enters an opaque region (mathematics, verbatim groups
/// and opaque wrappers are walked raw, math-mode runs and the content of
/// verbatim and opaque-wrapped arguments and bodies are laid out raw), so a
/// text item that reaches the inline layout is in text mode and outside every
/// such group. The test remains for a run a hook assembles itself. It tests
/// the immediate parent only, which covers the flat content of a verbatim
/// group (the group's children are the raw text). The checker walks the
/// ancestors instead, because it checks whole trees, including trees built by
/// hand.
fn is_raw_chars(node: &NodeRef<'_, Flm>) -> bool {
    node.parsing_state().mode() != Mode::Text
        || node.parent().is_some_and(|parent| is_opaque_group(&parent))
}

/// The span of a run: the union of its nodes' spans in the source of the first
/// node with a non-empty span (the first node's source when every span is
/// empty).
///
/// # Panics
///
/// If `run` is empty. Every caller has already established that the run holds
/// content, so this is an internal invariant.
fn run_span(run: &[NodeRef<'_, Flm>]) -> FlmSpan {
    let anchor = run
        .iter()
        .find(|node| !node.span().is_empty())
        .unwrap_or(&run[0])
        .span();
    let mut start = anchor.start();
    let mut end = anchor.end();
    for node in run {
        let span = node.span();
        if span.same_source(anchor) {
            start = start.min(span.start());
            end = end.max(span.end());
        }
    }
    SourceSpan::new(anchor.source(), start..end)
}

/// Stages an unchanged copy of a node without children (text, comment): kind,
/// span, parsing state and extension copied.
pub(super) fn copy_leaf(
    cx: &mut StructureCx<'_>,
    node: &NodeRef<'_, Flm>,
) -> Result<BuildId, StructureError> {
    Ok(cx.builder().add(
        node.kind().clone(),
        node.span().clone(),
        Arc::clone(node.parsing_state()),
        Vec::new(),
        node.ext().clone(),
        (),
    )?)
}

/// Stages a copy of a node that has children (a container node or a group)
/// over new children: kind, span, parsing state and extension copied.
pub(super) fn copy_over(
    cx: &mut StructureCx<'_>,
    node: &NodeRef<'_, Flm>,
    children: Vec<BuildId>,
) -> Result<BuildId, StructureError> {
    Ok(cx.builder().add(
        node.kind().clone(),
        node.span().clone(),
        Arc::clone(node.parsing_state()),
        children,
        node.ext().clone(),
        (),
    )?)
}

/// Stages a node the pass creates (a paragraph, a wrapping container), with
/// its extension built by `Flm::make_node_ext`, so that every node of a tree
/// carries an extension built the same way.
pub(super) fn mint(
    cx: &mut StructureCx<'_>,
    kind: NodeKind<Flm>,
    span: FlmSpan,
    state: Arc<ParsingState<Flm>>,
    children: Vec<BuildId>,
) -> Result<BuildId, StructureError> {
    let builder = cx.builder();
    let ext = Flm::make_node_ext(&kind, &span, &state, builder.staged_children(&children))?;
    Ok(builder.add(kind, span, state, children, ext, ())?)
}
