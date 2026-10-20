//! The layout questions that the block-structure pass and the tree-contract
//! checker share: whether a node is paragraph content, whether a run of nodes
//! holds blocks, and which layout a content region gets.
//!
//! A **content region** is a run of sibling nodes that is laid out together: a
//! fragment's top level, an environment body's content, an argument's content,
//! or the children of a `{…}` group. [`Region`] lists the three ways a content
//! region can be laid out; [`slot_layout`] returns the layout that a callable's
//! node slots (techy's `ParsedSlot`, the parts of its content outside its
//! arguments, such as an environment's body) take from the element's body declaration,
//! and [`argument_layout`] the
//! layout that an argument takes from its declared layout.
//!
//! The pass ([`super::pass`]) builds the tree that the checker
//! ([`super::check`]) checks, so both must decide these questions in exactly
//! the same way; this module decides them for both. Four of the functions are
//! public, so that an element or a lowering rule asks the same questions
//! instead of deciding them again: [`run_holds_blocks`],
//! [`argument_holds_blocks`], [`slot_holds_blocks`] and [`stands_in_blocks`].

use techy::core::node::NodeRef;
use techy::latexlike::Mode;

use super::typography::is_ascii_space;
use crate::model::aspects::{BlockRole, block_role_of};
use crate::model::element::{ElementSpec, element_of};
use crate::lang::flm::Flm;
use crate::lang::mint::{ArgLayout, BodyLayout, arg_layout_of, body_def_of};
use crate::model::doc::FragmentLayout;

/// How a content region is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Region {
    /// As blocks: paragraphs are built here.
    Blocks,
    /// Inline: no paragraph is built here.
    Inline,
    /// Opaque: an [opaque
    /// region](crate::guide::concepts_overview#opaque-regions) (mathematics or
    /// verbatim content), copied node for node.
    Raw,
}

/// Whether `text` is empty or ASCII whitespace only.
pub(crate) fn is_blank_text(text: &str) -> bool {
    text.bytes().all(is_ascii_space)
}

/// Whether a node counts as paragraph content: every node except a comment and
/// text that is only whitespace (a decorator counts). A run holding no content
/// starts no paragraph.
pub(crate) fn is_content(node: &NodeRef<'_, Flm>) -> bool {
    if node.is_comment() {
        return false;
    }
    match node.chars() {
        Some(text) => !is_blank_text(text),
        None => true,
    }
}

/// The layout of a callable's node slots (techy's `ParsedSlot`, the parts of
/// its content outside its arguments, such as an environment's body), taken from
/// the element's body declaration:
/// blocks or inline where the declaration says so; the element's block role
/// where the declaration follows it (`BodyLayout::FromRole`, the default) or
/// where the callable has no FLM body declaration at all (blocks for an element
/// whose role is `Block`, inline otherwise). A verbatim body and a body read in
/// math mode are [`Region::Raw`].
pub(crate) fn slot_layout(node: &NodeRef<'_, Flm>) -> Region {
    let from_role = || match block_role_of(node) {
        BlockRole::Block => Region::Blocks,
        BlockRole::Inline(_) | BlockRole::BlockDecorator(_) => Region::Inline,
    };
    match body_def_of(node) {
        Some(body) if !body.holds_content() || body.mode == Some(Mode::Math) => Region::Raw,
        Some(body) => match body.layout {
            BodyLayout::Blocks => Region::Blocks,
            BodyLayout::Inline => Region::Inline,
            BodyLayout::FromRole => from_role(),
        },
        None => from_role(),
    }
}

/// Whether a node is a paragraph-break marker (the node a blank line leaves in
/// the parsed tree), recognized by comparing its element value with the
/// paragraph-break element of the structural elements its parsing state
/// carries.
pub(crate) fn is_parbreak(node: &NodeRef<'_, Flm>) -> bool {
    ElementSpec::same_value(element_of(node), &node.parsing_state().ext().structural.parbreak)
}

/// Whether a run's nodes were parsed in math mode (a math-mode environment
/// body, a mathematics snippet's root, an argument read in math mode), which
/// makes the run an opaque region regardless of the layout it would otherwise
/// take. The nodes of a run share their content region's mode, so the first
/// node decides.
pub(crate) fn is_math_run(run: &[NodeRef<'_, Flm>]) -> bool {
    run.first().is_some_and(|node| node.parsing_state().mode() == Mode::Math)
}

/// Whether `run`, a run of sibling nodes, holds blocks: whether one of the
/// run's own nodes is a paragraph-break marker or a node whose block role is
/// [`Block`](BlockRole::Block) (a paragraph is one). A run parsed in math mode
/// is an opaque region and returns `false`.
///
/// This is the only test by which the block-structure pass decides a layout
/// from content: it lays out a fragment declared
/// [`InlineOrBlocks`](crate::model::doc::FragmentLayout::InlineOrBlocks), and
/// an argument declared [`ArgLayout::InlineOrBlocks`], as blocks exactly when
/// this function returns `true`. It reads the run as it is: before the pass,
/// a blank line is a paragraph-break marker; after the pass, the paragraphs it
/// built are nodes of block role `Block`. A run laid out as blocks therefore
/// still returns `true` after the pass.
///
/// It is also the test for a **cut point** of a
/// [block-transparent](super::transparent) element: when the runs of all the
/// parts an element cuts return `false`, there is nothing to cut, and the
/// element's aspect leaves the node as it is. Unlike
/// [`argument_holds_blocks`], it reads a run of nodes rather than an argument
/// and ignores every declared layout: a paragraph break with no content around
/// it is still a cut point.
pub fn run_holds_blocks(run: &[NodeRef<'_, Flm>]) -> bool {
    !is_math_run(run)
        && run.iter().any(|node| is_parbreak(node) || matches!(block_role_of(node), BlockRole::Block))
}

/// Whether the argument of `node` at position `index` (counting from zero, in
/// the order the arguments are declared, written or not) holds block content
/// (paragraphs, or block-level constructs such as a list) rather than inline
/// content.
///
/// The result follows the argument's declared [layout](ArgLayout)
/// ([`arg_layout_of`]):
///
/// - [`Inline`](ArgLayout::Inline), and an argument with no declared layout:
///   `false`, regardless of the content;
/// - [`Blocks`](ArgLayout::Blocks): `true`, even for an empty argument;
/// - [`InlineOrBlocks`](ArgLayout::InlineOrBlocks): whether the content holds a
///   node whose block role is [`Block`](BlockRole::Block) (a paragraph is one),
///   or a paragraph-break marker together with some content (text that is not
///   only whitespace, or any node but a comment). This is the test that the
///   top level of a fragment laid out
///   [`InlineOrBlocks`](crate::model::doc::FragmentLayout::InlineOrBlocks)
///   uses, and the block-structure pass lays out the argument by it, except for
///   an argument holding no content at all (below).
///
/// It is `false` when `node` has no argument at `index`, when the argument was
/// not written (an omitted optional argument), and when its content was parsed
/// in mathematics mode, which is an opaque region the pass never lays out.
///
/// The result is computed on the tree as it is, and it is the same before
/// the block-structure pass and after it: before, a blank line is a
/// paragraph-break marker in the content; after, the pass has built paragraphs,
/// which are nodes of block role `Block`. An element that must know the shape
/// of its argument (to choose its own block role, which the pass reads, and
/// again to choose its rendering, after the pass) therefore calls this
/// function both times and never decides the question itself. An argument
/// declared `InlineOrBlocks` that holds no content (only blank lines,
/// whitespace and comments) returns `false` both times: the pass lays such a
/// run out as an empty content region and reports nothing, and the argument
/// holds nothing either way, so it does not hold blocks.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::model::doc::FragmentLayout;
/// use flm_core::model::element::ElementSpec;
/// use flm_core::lang::mint::{ArgDef, ArgLayout, ElementDef, SyntaxDef};
/// use flm_core::lang::rewrites::blocks::{argument_holds_blocks, structure, BlocksConfig};
/// use flm_core::lang::{
///     build_language, parse_fragment, FlmDiagnostics, FlmSource, LangInputs, ParseOptions,
/// };
///
/// // `\note{…}`, whose one argument is inline or blocks as its content shows.
/// let mut inputs = LangInputs::new();
/// inputs.elements.push(ElementDef::new(
///     "t",
///     ["note"],
///     SyntaxDef::Macro {
///         args: vec![ArgDef::mandatory("text").layout(ArgLayout::InlineOrBlocks)],
///     },
///     Arc::new(ElementSpec::new("t.note")),
/// ));
/// let lang = build_language(inputs);
/// let source = Arc::new(FlmSource::new("\\note{one line} \\note{a\n\nb}"));
/// let fragment = parse_fragment(
///     &lang, source, FragmentLayout::Blocks, &ParseOptions::default(),
/// ).expect("the input is shallow enough to parse");
/// let answers = |tree: &flm_core::techy::core::node::NodeTree<flm_core::lang::Flm>| {
///     tree.root()
///         .descendants()
///         .filter(|node| node.name() == Some("note"))
///         .map(|node| argument_holds_blocks(&node, 0))
///         .collect::<Vec<bool>>()
/// };
/// assert_eq!(answers(fragment.tree()), [false, true]);
///
/// let mut diags = FlmDiagnostics::new();
/// let tree = structure(fragment.tree(), FragmentLayout::Blocks, &BlocksConfig::new(), &mut diags)
///     .expect("the input is shallow enough to structure");
/// assert_eq!(answers(&tree), [false, true]);
/// ```
pub fn argument_holds_blocks(node: &NodeRef<'_, Flm>, index: usize) -> bool {
    let Some(content) = node.argument_content_nodes(index) else {
        return false;
    };
    let run: Vec<NodeRef<'_, Flm>> = content.iter().collect();
    if is_math_run(&run) || argument_layout(node, index, &run) != Region::Blocks {
        return false;
    }
    // A run with no content — blank lines, whitespace and comments — is laid
    // out by the pass as an empty region, and holds nothing after it; only a
    // declared `Blocks` makes it blocks, so that the answer does not change
    // across the pass.
    arg_layout_of(node, index) == Some(ArgLayout::Blocks)
        || run.iter().any(|node| is_content(node) && !is_parbreak(node))
}

/// Whether the block-structure pass lays out the content of `node`'s node
/// slot at `index` as blocks (a sequence of
/// paragraphs and block-level nodes) rather than as inline content. This is the
/// counterpart of [`argument_holds_blocks`] for node slots.
///
/// A node slot (techy's `ParsedSlot`) is a part of a node's content outside
/// its arguments, such as an environment's body; a node slot may be unnamed.
/// `index` counts the node's slots from zero, in source order. Every slot of a node takes the
/// layout of the element's body declaration ([`BodyLayout`]): blocks or inline
/// where the declaration says so, and otherwise blocks for an element whose block role is
/// [`Block`](BlockRole::Block) and inline for any other. A verbatim body and a
/// body read in math mode are opaque, and return `false`. The result is the
/// one the pass acts on, and it is the same before and after the pass. It is
/// `false` when `node` is not a callable or has no slot at `index`.
pub fn slot_holds_blocks(node: &NodeRef<'_, Flm>, index: usize) -> bool {
    node.slot_content_nodes(index).is_some() && slot_layout(node) == Region::Blocks
}

/// The layout of `node`'s argument at `index`, whose content is `run`, taken
/// from the argument's declared layout ([`arg_layout_of`]): inline or blocks
/// where declared so, and for [`ArgLayout::InlineOrBlocks`] blocks exactly when
/// [`run_holds_blocks`] returns `true`. An argument with no declared layout is
/// inline. Regardless of the declaration, the pass lays out a run parsed in
/// math mode as [`Region::Raw`].
pub(crate) fn argument_layout(node: &NodeRef<'_, Flm>, index: usize, run: &[NodeRef<'_, Flm>]) -> Region {
    match arg_layout_of(node, index).unwrap_or_default() {
        ArgLayout::Inline => Region::Inline,
        ArgLayout::Blocks => Region::Blocks,
        ArgLayout::InlineOrBlocks if run_holds_blocks(run) => Region::Blocks,
        ArgLayout::InlineOrBlocks => Region::Inline,
    }
}

/// Whether `node` is placed in a content region laid out as blocks (among
/// paragraphs and other blocks) rather than in one laid out inline, in a tree
/// the block-structure pass has laid out.
///
/// A node of block role [`Block`](BlockRole::Block) is normally placed in a
/// content region laid out as blocks. The pass also keeps one where it was
/// written inside inline content (a heading's title, the argument of
/// `\textbf{…}`, `\text{…}` inside a formula) and reports it there
/// ([`BlockInInline`](super::BlockInInline)). A lowering rule that renders such
/// a node differently in inline content calls this function, so that it reads
/// the layout the pass gave the content region instead of deciding it again.
///
/// `root` is the layout the tree's top level was structured with: for a
/// fragment's tree, the fragment's [`layout`](crate::model::doc::Fragment::layout);
/// at lowering, [`Lowerer::layout`](crate::core::lowering::Lowerer::layout).
///
/// The content region is the one holding `node` as content:
///
/// - an argument's content is laid out as [`argument_holds_blocks`] says for
///   that argument;
/// - the content of a node slot (techy's `ParsedSlot`, a part of a node's
///   content outside its arguments, such as an environment's body) is laid out as the
///   element's body declaration says;
/// - the content of a `{…}` group or of a paragraph is inline;
/// - the content of a container node is laid out as the content region that
///   holds the container;
/// - the top level of the tree (the root's content, or the root itself when it
///   is not a container) is laid out as `root` says: blocks for
///   [`Blocks`](FragmentLayout::Blocks), inline for
///   [`Inline`](FragmentLayout::Inline), and for
///   [`InlineOrBlocks`](FragmentLayout::InlineOrBlocks) blocks exactly when the
///   top level holds a paragraph break or a block-level node (the pass's own
///   test, which gives the same result before and after the pass).
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
///
/// use flm_core::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
/// use flm_core::model::doc::FragmentLayout;
/// use flm_core::model::element::ElementSpec;
/// use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
/// use flm_core::lang::rewrites::blocks::{stands_in_blocks, structure, BlocksConfig};
/// use flm_core::lang::{
///     build_language, parse_fragment, FlmDiagnostics, FlmSource, LangInputs, ParseOptions,
/// };
///
/// // `\aside{…}`, a block of its own, and `\em{…}`, whose argument is inline.
/// let mut inputs = LangInputs::new();
/// inputs.elements.push(ElementDef::new(
///     "t",
///     ["aside"],
///     SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] },
///     Arc::new(
///         ElementSpec::new("t.aside")
///             .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block))),
///     ),
/// ));
/// inputs.elements.push(ElementDef::new(
///     "t",
///     ["em"],
///     SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] },
///     Arc::new(ElementSpec::new("t.em")),
/// ));
/// let lang = build_language(inputs);
/// let answers = |text: &str, layout: FragmentLayout| {
///     let source = Arc::new(FlmSource::new(text));
///     let fragment = parse_fragment(&lang, source, layout, &ParseOptions::default())
///         .expect("the input is shallow enough to parse");
///     let mut diags = FlmDiagnostics::new();
///     let tree = structure(fragment.tree(), layout, &BlocksConfig::new(), &mut diags)
///         .expect("the input is shallow enough to structure");
///     tree.root()
///         .descendants()
///         .filter(|node| node.name() == Some("aside"))
///         .map(|node| stands_in_blocks(&node, layout))
///         .collect::<Vec<bool>>()
/// };
/// // Among paragraphs, and inside the inline argument of `\em`.
/// assert_eq!(answers(r"x \aside{a} \em{y \aside{b}}", FragmentLayout::Blocks), [true, false]);
/// // Alone at the top level: the fragment's layout decides.
/// assert_eq!(answers(r"\aside{a}", FragmentLayout::Blocks), [true]);
/// assert_eq!(answers(r"\aside{a}", FragmentLayout::Inline), [false]);
/// ```
pub fn stands_in_blocks(node: &NodeRef<'_, Flm>, root: FragmentLayout) -> bool {
    // See decision rationale at [§flm-dd-dr:lang-stands-in-blocks].
    let Some(parent) = node.parent() else { return top_level_holds_blocks(std::slice::from_ref(node), root) };
    // The content of an argument or of a slot sits directly under the
    // callable, or under the group that delimits it.
    for owner in std::iter::successors(Some(parent), NodeRef::parent).take(2) {
        if let Some(region) = region_holding(&owner, node) {
            return region == Region::Blocks;
        }
    }
    if !parent.is_list() {
        return false;
    }
    match parent.parent() {
        // A list inside a region is laid out in that region.
        Some(_) => stands_in_blocks(&parent, root),
        None => {
            let run: Vec<NodeRef<'_, Flm>> = parent.children().iter().collect();
            top_level_holds_blocks(&run, root)
        }
    }
}

/// Whether the top level of a tree, the run `run`, is laid out as blocks
/// under the tree's layout `root`: as `root` says, or, for
/// [`InlineOrBlocks`](FragmentLayout::InlineOrBlocks), by the pass's test
/// [`run_holds_blocks`].
fn top_level_holds_blocks(run: &[NodeRef<'_, Flm>], root: FragmentLayout) -> bool {
    match root {
        FragmentLayout::Blocks => true,
        FragmentLayout::Inline => false,
        FragmentLayout::InlineOrBlocks => run_holds_blocks(run),
    }
}

/// The layout of the content region of the callable `owner` (one of its
/// arguments or one of its node slots, techy's `ParsedSlot`, the parts of its
/// content outside the arguments) that holds `node` as content, or `None` when
/// no content region of `owner` holds `node` (or `owner` is not a callable).
fn region_holding(owner: &NodeRef<'_, Flm>, node: &NodeRef<'_, Flm>) -> Option<Region> {
    let callable = owner.callable()?;
    let holds = |content: Option<techy::core::node::NodeSlice<'_, Flm>>| {
        content.is_some_and(|content| content.iter().any(|child| child.id() == node.id()))
    };
    if let Some(index) = (0..callable.arguments.len()).find(|&i| holds(owner.argument_content_nodes(i))) {
        return Some(if argument_holds_blocks(owner, index) { Region::Blocks } else { Region::Inline });
    }
    (0..callable.slots.len()).any(|i| holds(owner.slot_content_nodes(i))).then(|| slot_layout(owner))
}
