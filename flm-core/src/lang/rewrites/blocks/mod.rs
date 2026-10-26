//! Block structure: the rewrite pass that divides a tree's content into
//! paragraphs and blocks and resolves its text, and the checker of the tree
//! contract.
//!
//! The **block-structure pass** ([`structure`]) takes a parsed tree (or a tree
//! an earlier rewrite produced) and returns a new tree, called the *structured*
//! tree. It lays out the content of every content region, and resolves the text
//! once.
//!
//! A **content region** is a run of sibling nodes that is laid out together: a
//! fragment's top level, an environment's body, an argument's content, or the
//! children of a `{…}` group. The pass lays out each content region in one of
//! three ways, its *region layout*:
//!
//! - **as blocks**: every maximal stretch of inline content becomes a
//!   paragraph (a group node of the paragraph group class,
//!   [`FlmGroupType::Paragraph`](crate::lang::FlmGroupType::Paragraph)); every
//!   node whose element has the block role
//!   [`Block`](crate::model::aspects::BlockRole::Block) is placed on its own
//!   between the paragraphs; the paragraph-break markers (the nodes a blank line
//!   leaves) are removed;
//! - **inline**: no paragraph is built; a block-level node or a blank line found
//!   there is reported;
//! - **opaque**: the content is copied unchanged; see [opaque
//!   regions](crate::guide::concepts_overview#opaque-regions). The opaque
//!   regions are the content of a mathematics group, of a verbatim group and
//!   of an opaque wrapper (the group without delimiters that minting places
//!   around an argument's or a body's content to keep it as written), a
//!   verbatim body, and any run parsed in mathematics mode. Inside
//!   mathematics, an argument or a body read in text mode (the argument of
//!   `\text{…}`) is content again and is laid out; the exception does not
//!   reach inside a verbatim group or an opaque wrapper, where nothing is laid
//!   out, whatever its parsing mode.
//!
//! **Text resolution** happens in the same pass, in every content region that
//! is not opaque: adjacent text nodes are merged into one, every run of
//! whitespace becomes one space, the whitespace at the edges of a paragraph is
//! removed, and the typographic substitutions that [`BlocksConfig`] enables
//! are applied (`--` becomes an en dash, for example). An inline node whose
//! element absorbs the whitespace beside it
//! ([`InlineSpacing`](crate::model::aspects::InlineSpacing), as a citation mark
//! does) has that whitespace removed, whitespace nodes included: a node whose
//! element carries the [`Whitespace`](crate::model::aspects::Whitespace)
//! aspect, such as `~`, is whitespace to these rules, removed beside such a
//! node and at the edges of a paragraph, and kept as a node of its own
//! elsewhere (see [`pass`]). A merged text node whose text no longer matches
//! its source byte for byte records where each part came from
//! ([`TextSourceMap`](crate::lang::TextSourceMap)).
//!
//! # What decides the layout
//!
//! Nothing about paragraphs and blocks is decided while a document is parsed.
//! The pass decides everything from declarations:
//!
//! - the **block role** of each node's element
//!   ([`BlockRole`](crate::model::aspects::BlockRole), read with
//!   [`block_role_of`](crate::model::aspects::block_role_of)): a block of its
//!   own, inline content, or a block decorator attached to the paragraph it
//!   occurs in ([`Placement`](crate::model::aspects::Placement));
//! - the layout the element declares for its **body**
//!   ([`BodyLayout`](crate::lang::mint::BodyLayout)) and for each of its
//!   **arguments** ([`ArgLayout`](crate::lang::mint::ArgLayout));
//! - the layout of the **fragment's top level**
//!   ([`FragmentLayout`](crate::model::doc::FragmentLayout)), which the caller
//!   passes to [`structure`].
//!
//! [`pass`] gives the full rules for which content region gets which layout.
//!
//! # How the pass works: the cut step and the layout step
//!
//! The pass makes two walks over the tree. Each walk is a *restage*: techy's
//! tree transformation ([`RestageContext`](techy::transform::RestageContext))
//! that builds a new tree from the old one node by node. To *stage* a node is
//! to build it into that new tree.
//!
//! 1. The **cut step** handles the
//!    [block-transparent elements](crate::guide::concepts_overview#block-transparent-elements):
//!    elements whose content is laid out as if it were written directly in the
//!    content around them. Every node of such an element whose content crosses
//!    a paragraph break or holds a block is replaced by *cut parts*: nodes that
//!    each hold one stretch of inline content or one block, with the paragraph
//!    breaks between them. The element's [`BlockTransparent`] aspect builds the
//!    cut parts, through the operations of [`BlockCutter`].
//! 2. The **layout step** then lays out every content region of the cut tree
//!    and resolves the text. A cut part is laid out by its block role, like any
//!    other node. An element's [block-structure hook](BlockStructure) runs
//!    during this step: it lays out the content of its own node, while the
//!    paragraph around that node is being built.
//!
//! # Calling the pass
//!
//! [`structure`] and [`structure_with_guard`] are plain functions over a
//! tree: they need no registry, plan or run, so they can be called on a
//! fragment's tree, on a snippet, or on a synthesized value. They run on a
//! [worker thread](crate::lang::guard::on_worker_thread), and create one when
//! the caller is not on one. The problems found in the document are pushed as
//! diagnostics onto the collection the caller passes, and the structured tree
//! is still returned; a failure that leaves no tree is a [`StructureError`]
//! (see [`error`]).
//!
//! The `blocks` feature ([`crate::stdcore::features::blocks`]) places the
//! pass in a plan as the rewrite stage `flm.blocks.structure`, which publishes
//! the structured trees as `flm.blocks.structured`; a plan can instead keep the
//! trees unstructured through the alternative `flm.blocks.raw` (both refine the
//! capability `flm.blocks`). The feature's check stage runs the tree-contract
//! checker over every frozen tree, its block-structure part only over the trees
//! the plan structured.
//!
//! # Making an element take part
//!
//! An element takes part in block structure through its declarations (above)
//! and, where those are not enough, through one of two aspects (behavior
//! attached to an element when a feature registers it; see [elements and
//! aspects](crate::guide::concepts_overview#elements-and-aspects)):
//!
//! - [`BlockTransparent`], for an element whose content crosses paragraph
//!   boundaries, such as a highlight of a passage;
//! - [`BlockStructure`], the block-structure hook, for an element that lays out
//!   its own content; no standard element uses it.
//!
//! A node slot (techy's `ParsedSlot`) is a part of a node's content outside
//! its arguments, such as an environment's body. Four public functions decide
//! layout questions exactly as the pass and the checker decide them, so that
//! an element or a lowering rule never repeats the decision:
//! [`argument_holds_blocks`] and [`slot_holds_blocks`] (whether an argument of
//! a node, or one of its node slots, holds block content), [`stands_in_blocks`]
//! (whether a node is placed in a content region laid out as blocks), and
//! [`run_holds_blocks`] (whether a run of nodes holds a paragraph break or a
//! block, which is also the test for a cut point of a block-transparent
//! element).
//!
//! # The tree contract
//!
//! The **tree contract** is the set of rules that every tree satisfies after
//! parsing, together with the further rules that a structured tree satisfies.
//! [`check`] states the rules and checks them: [`check_parse_contract`] and
//! [`check_block_contract`] return every violation as a typed
//! [`ContractViolation`].
//!
//! # Map of the module
//!
//! - [`config`]: [`BlocksConfig`], the options of text resolution.
//! - [`error`]: the conditions the pass reports and recovers from
//!   ([`BlockInInline`], [`ParbreakInInline`], [`DecoratorWithoutHome`]), the
//!   failure type [`StructureError`], and the conditions of its diagnostics
//!   ([`StructureFailed`], [`NoWorkerThread`]).
//! - [`transparent`]: the [`BlockTransparent`] aspect, the result it returns
//!   ([`Cutting`], [`CutItem`]), the [`BlockCutter`] operations it calls, and
//!   [`CutPart`], which designates the argument or body of a node that
//!   [`BlockCutter::rebuild`] gives new content.
//! - [`hook`]: the [`BlockStructure`] hook, the result it returns
//!   ([`Structuring`]), the
//!   [`BlockStructurer`] operations it calls, [`SynthesizedNode`] (a node an
//!   aspect or a hook creates), [`RegionLayout`] (whether the hook's node is
//!   placed between blocks or inside inline content), and [`StructureCx`], the
//!   context both aspects stage nodes through.
//! - [`pass`]: [`structure`], [`structure_with_guard`], and the rules of the
//!   layout step.
//! - [`check`]: the tree-contract checker.
//!
//! Four private submodules hold the rest: `cut`, the cut step, which
//! implements [`BlockCutter`]; `predicates`, the layout questions the pass and
//! the checker share, including the four public functions above; `text`, the
//! merge of adjacent text and its source map; `typography`, the typographic
//! substitutions.

pub mod check;
pub mod config;
mod cut;
pub mod error;
pub mod hook;
pub mod pass;
mod predicates;
mod text;
pub mod transparent;
mod typography;

pub use check::{
    ContractViolated, ContractViolation, ContractViolationKind, check_block_contract,
    check_parse_contract,
};
pub use config::BlocksConfig;
pub use error::{
    BlockInInline, DecoratorWithoutHome, NoWorkerThread, ParbreakInInline, StructureError,
    StructureFailed,
};
pub use hook::{
    BlockStructure, BlockStructurer, RegionLayout, StructureCx, Structuring, SynthesizedNode,
};
pub use pass::{structure, structure_with_guard};
pub use predicates::{argument_holds_blocks, run_holds_blocks, slot_holds_blocks, stands_in_blocks};
pub use transparent::{BlockCutter, BlockTransparent, CutItem, CutPart, Cutting};
