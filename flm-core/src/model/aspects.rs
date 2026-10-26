//! The aspects that the standard features implement and read: block roles,
//! reference points, reference sites, headings, content outside the
//! surrounding text, what replaces a node in a copy of content, and whether a
//! node is whitespace.
//!
//! An aspect is a piece of behavior attached to an element when a feature
//! registers the element and read back through a trait object; it is evaluated per
//! occurrence of the element and reads no options (see [elements and
//! aspects](crate::guide::concepts_overview#elements-and-aspects)). An element
//! holds its aspects in its [`AspectMap`](crate::model::element::AspectMap).
//! A stage or a backend asks an element what it does by looking up an aspect
//! ("is this a block?", "which reference points does this node carry?"), never
//! by matching the element's name against a fixed list. A stage written today
//! therefore works on an element that a feature written later declares.
//!
//! An aspect method has access to the element's own data, which its
//! implementation holds, and receives the node it is asked about; [`InCopy`]
//! also receives the purpose of the copy. No aspect method receives option
//! values or anything else about the document as a whole or about the run. Two
//! rules follow, and hold for every aspect of this module:
//!
//! - **An aspect is evaluated per occurrence.** The node is a parameter of every
//!   method, because the same element may give a different result at each
//!   place it occurs: one heading element declares a different reference point
//!   at every heading.
//! - **An aspect reads no [options](crate::model::options).** A fact that
//!   varies from document to document is an option of the stage that uses the
//!   aspect's result, not of the aspect. A fact fixed by a feature's
//!   configuration is held by the element's data or by the aspect's
//!   implementation, both built when the registry is built.
//!
//! # The aspects defined here
//!
//! - [`BlockRoleAspect`] — whether an occurrence is a block, inline content or a
//!   block decorator ([`BlockRole`]); read with [`block_role_of`].
//! - [`Referenceable`] — the [reference points](RefPoint) an occurrence
//!   carries: the places in the document that a reference can point at, each
//!   declaring which [label types](AcceptedRefTypes) may be written for it.
//! - [`Referencing`] — the [reference sites](RefSite) an occurrence carries:
//!   the places that point at a reference point.
//! - [`Sectioning`] — the heading level of an occurrence, whether it is
//!   listed in the table of contents, and what an entry of the table of
//!   contents shows for it.
//! - [`ApartFromText`] — the content of an occurrence that is not part of the
//!   text around it, such as the words of a footnote written in a heading's
//!   title; read with [`apart_from_text_of`].
//! - [`InCopy`] — what replaces an occurrence in a copy of content shown a
//!   second time, away from where it is written ([`CopiedAs`]): the occurrence
//!   as written, nothing, or a new node of the element's own; read with
//!   [`copied_as`].
//! - [`Whitespace`] — that every occurrence is whitespace, such as the
//!   non-breaking space `~`: a marker with no method and no data, which the
//!   block-structure pass, the trimming of paragraphs at lowering and the
//!   features' tests for blank content read; read with [`is_whitespace_of`].
//!
//! The functions [`block_role_of`], [`apart_from_text_of`], [`copied_as`] and
//! [`is_whitespace_of`] supply the result for an element that does not declare
//! the aspect, so a caller reads those aspects through them rather than
//! through the aspect map.
//!
//! This module holds the aspects that many features implement, and the
//! aspects that state what an element is ([`Whitespace`]). An aspect that only
//! one rewrite pass reads and that describes what that pass does with a node
//! is declared beside that pass instead
//! ([`Expand`](crate::lang::rewrites::userdefs::Expand),
//! [`BlockStructure`](crate::lang::rewrites::blocks::BlockStructure)), since it
//! has no meaning apart from that pass.
//!
//! # Content an aspect returns
//!
//! An aspect that returns document content returns a [`ContentRef`]: a
//! reference to an argument or the body *of the node the aspect was asked
//! about*, or to one node of that node's tree, rather than a copy of a tree.
//! A reference into the node's own tree always designates content of that tree
//! and is cheap to carry, whereas a copy would have to be stored and kept up to
//! date.
//!
//! The exception is content that no node of the tree holds by itself: a label
//! that a feature's configuration supplies, such as the bullet beside the items
//! of a list, or a part of a run of characters, such as the value `Custom` of
//! `tag=Custom`, which shares its node with the key. The aspect then builds
//! that content as a small tree of its own and returns it
//! ([`ContentRef::Synthesized`]).
//!
//! Labels and anchor identifiers are owned [`String`]s rather than shared
//! names: a stage computes them per node while it scans the document and uses
//! them as keys of the tables it writes, so every consumer needs plain
//! comparable data.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use techy::core::node::{NodeId, NodeRef, NodeTree};

use crate::model::addr::NodeAddr;
use crate::model::names::{Name, RefKind};
use crate::lang::copy::ValueContent;
use crate::lang::mint::FlmSynthesizedSpec;
use crate::lang::text::collapse_whitespace;
use crate::lang::{Flm, FlmSpan, PresPayload};

// --- block roles --------------------------------------------------------------------

/// How an inline element treats the whitespace immediately before and after
/// it: the data of [`BlockRole::Inline`].
///
/// Setting either field removes the whitespace on that side entirely, so that
/// an element such as a citation mark can control its own spacing. The
/// whitespace removed is every piece of whitespace between the element and the
/// nearest content on that side, the nodes that are whitespace
/// ([`Whitespace`], such as `~` and `\ `) included: before an element that
/// absorbs the whitespace before it, `word~`, `word ~ ` and `word~~` all leave
/// `word` alone. Source comments between the whitespace and the element do not
/// prevent the removal, and are kept: in `word %% note` followed on the next
/// line by the element, the space after `word` is removed too. Ordinary inline
/// content keeps the spaces around it: [`InlineSpacing::CONTENT`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InlineSpacing {
    /// Remove the whitespace immediately before the element.
    pub absorb_before: bool,
    /// Remove the whitespace immediately after the element.
    pub absorb_after: bool,
}

impl InlineSpacing {
    /// Ordinary inline content: the spaces on both sides are kept.
    pub const CONTENT: InlineSpacing = InlineSpacing { absorb_before: false, absorb_after: false };
}

/// Where a block decorator is placed within its block: the data of
/// [`BlockRole::BlockDecorator`].
///
/// A block decorator is an element attached to the block it occurs in, such as
/// a run-in heading or the mark that closes a proof. The block-structure pass
/// ([`crate::lang::rewrites::blocks`]) places it accordingly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum Placement {
    /// First in its block: a new block begins at the element, and the inline
    /// content that follows it belongs to that block, as for a run-in heading.
    Leads,
    /// Last in its block: the block ends after the element, as for the mark
    /// that closes a proof.
    Trails,
    /// Anywhere in its block; the element begins or ends no block.
    Free,
}

/// The role of an occurrence in the block and paragraph structure: a block,
/// inline content, or a block decorator.
///
/// The block-structure rewrite pass ([`crate::lang::rewrites::blocks`]) reads
/// it to cut content into blocks: the pass begins a new block at each `Block`
/// node and at each paragraph break, keeps `Inline` nodes inside the
/// surrounding block, and attaches a `BlockDecorator` to the block it occurs
/// in. When the pass trims the whitespace at the edges of a block, it looks
/// past a decorator, so a decorator at an edge does not keep the whitespace
/// beside it.
///
/// An element that declares no [`BlockRoleAspect`] has [`BlockRole::DEFAULT`];
/// read an occurrence's role with [`block_role_of`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum BlockRole {
    /// A block of its own, between paragraphs.
    Block,
    /// Inline content inside a block, with the whitespace beside it treated as
    /// the [`InlineSpacing`] says.
    Inline(InlineSpacing),
    /// Attached to the block it occurs in, at the [`Placement`] given.
    BlockDecorator(Placement),
}

impl BlockRole {
    /// The role of an element that declares no [`BlockRoleAspect`]: ordinary
    /// inline content, which keeps the whitespace around it.
    pub const DEFAULT: BlockRole = BlockRole::Inline(InlineSpacing::CONTENT);
}

/// The aspect that gives the [`BlockRole`] of an occurrence.
///
/// The method receives the node, so an element that is a block in some places
/// and inline content in others decides from the node: from its own content,
/// or from where it is placed. A section command of a quotation (such as
/// `\lines` or `\attributed`) is a block directly in the quotation's body and
/// inline content elsewhere; an image is a block as a direct child of a
/// float's body and inline content elsewhere.
/// [`BlockRole`] therefore has no "either" variant: an element whose role
/// depends on its content or its position computes the role in this method.
///
/// Most elements have one fixed role and declare [`FixedBlockRole`]. Read the
/// role of a node with [`block_role_of`], which supplies
/// [`BlockRole::DEFAULT`] for an element that does not declare this aspect.
pub trait BlockRoleAspect: Send + Sync {
    /// The block role of this occurrence.
    fn block_role(&self, node: &NodeRef<'_, Flm>) -> BlockRole;
}

/// A [`BlockRoleAspect`] that gives the same role for every occurrence.
///
/// Most elements declare this one: `FixedBlockRole(BlockRole::Block)` for a
/// heading, `FixedBlockRole(BlockRole::DEFAULT)` for an emphasis command.
///
/// # Examples
///
/// ```
/// use flm_core::model::aspects::{BlockRole, FixedBlockRole, InlineSpacing};
///
/// let heading = FixedBlockRole(BlockRole::Block);
/// let emphasis = FixedBlockRole(BlockRole::DEFAULT);
/// assert_eq!(heading.0, BlockRole::Block);
/// assert_eq!(emphasis.0, BlockRole::Inline(InlineSpacing::CONTENT));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedBlockRole(pub BlockRole);

impl BlockRoleAspect for FixedBlockRole {
    fn block_role(&self, _node: &NodeRef<'_, Flm>) -> BlockRole {
        self.0
    }
}

/// The block role of the node `node`: the role its element's
/// [`BlockRoleAspect`] gives, or [`BlockRole::DEFAULT`] when its element does
/// not declare that aspect.
///
/// Use this function to read a node's role. Reading the aspect map directly
/// would leave every caller to supply the default for the many elements, such
/// as plain text or an emphasis command, that do not declare the aspect.
pub fn block_role_of(node: &NodeRef<'_, Flm>) -> BlockRole {
    match node.ext().element.aspect::<dyn BlockRoleAspect>() {
        Some(aspect) => aspect.block_role(node),
        None => BlockRole::DEFAULT,
    }
}

/// The aspect of an element each of whose nodes is whitespace, such as the
/// non-breaking space `~` and the space `\ `; read with [`is_whitespace_of`].
///
/// Such a node is whitespace to the layout of paragraphs, and to the readers
/// listed below, which ignore whitespace. The block-structure pass
/// ([`crate::lang::rewrites::blocks`]) treats such a node as it treats text
/// that is only whitespace, wherever it removes whitespace:
///
/// - beside an inline element that absorbs the whitespace on that side
///   ([`InlineSpacing`]), the node is removed with the rest of the whitespace
///   between the element and the nearest content;
/// - at the start and at the end of a paragraph, the node is removed with the
///   rest of the whitespace there;
/// - the node is not paragraph content: a stretch holding only whitespace,
///   such nodes and comments starts no paragraph, and between blocks it is
///   removed;
/// - a straight quote right after the node opens, as it does after a space
///   (in `a~"x"`, the quote before `x` opens).
///
/// Everywhere else the pass keeps the node where it is written, as a node of
/// its own: it is never merged into the text beside it and never collapsed
/// with the whitespace around it (`a~~b` keeps its two nodes, `a~ b` the node
/// and the space). A node that stays is lowered by its element's own lowering
/// rule.
///
/// The other readers of the aspect treat such a node as they treat text that
/// is only whitespace, and each makes the blank test of the first item below:
///
/// - the lowering helpers' blank test
///   ([`is_blank`](crate::stdcore::backends::lowering::is_blank)), through
///   which the helper that trims a paragraph a rule writes over content it
///   cuts out itself
///   ([`stage_trimmed`](crate::stdcore::backends::lowering::stage_trimmed):
///   a quotation's passage, attribution and lines, a `\celldata` entry)
///   removes the node at that paragraph's edges, and the quotation's and the
///   `\celldata` reader's tests find a section or an entry holding only such
///   nodes empty;
/// - the readers of a list's body and of a float's body
///   ([`list_items`](crate::stdcore::features::lists::list_items),
///   [`float_body`](crate::stdcore::features::floats::float_body)), and the
///   recomposed LaTeX of a list and of a quotation, which leaves the node out
///   where it writes the layout itself;
/// - an xnote's test for empty content
///   ([`Empty`](crate::stdcore::features::xnotes::Empty): `\footnote{~}` is
///   reported empty) and the reader of an xnote's labels argument
///   ([`labels_of`](crate::stdcore::features::xnotes::labels_of));
/// - the readers of a list's, an item's and a caption's bracket
///   ([`list_options`](crate::stdcore::features::lists::list_options),
///   [`item_options`](crate::stdcore::features::lists::item_options),
///   [`read_caption`](crate::stdcore::features::floats::read_caption)): a
///   bracket holding only whitespace is as if it were not written;
/// - the trimming of a key-value entry's value
///   ([`key_value_pieces`](crate::lang::copy::key_value_pieces): `tag=~x~`
///   gives `x`);
/// - the value tests of a user definition's template, which read the one
///   placeholder of their test argument, whitespace aside
///   ([`userdefs`](crate::lang::rewrites::userdefs));
/// - the test for a star of a theorem's title that is not one run of
///   characters
///   ([`title_reading`](crate::stdcore::features::theorems::title_reading)),
///   and its copy in the title's reader: `[~*\emph{x}]` begins with a star,
///   as `[ *\emph{x}]` does.
///
/// A reader of text taken as typed or with its markup keeps such a node as
/// part of the text, even where it removes the whitespace at the text's ends:
/// a citation key, a term (`\term{~}` is not empty) and a theorem's
/// back-reference (`[~*lem:a]` is malformed, while `[ *lem:a]` refers to
/// `lem:a`). A label holds no such node: it is read from a characters-only
/// group ([`crate::lang::chars_group`]), in which `~` and the backslash of
/// `\ ` are ordinary characters.
///
/// The no-break space U+00A0 typed directly in the text is no such node: it is
/// a character of the text, and the pass and every reader above count it as
/// content, as they count every character other than the space, the tab, the
/// carriage return and the line feed ([the whitespace of FLM
/// text](crate::lang::text#whitespace)). The one exception is the key of a
/// `key=value` entry, whose ends techy's key-value reader trims of every
/// Unicode whitespace character, U+00A0 included.
///
/// The aspect has no method: its presence is the whole statement, the same
/// for every node of the element. It is meant for an inline element, of the
/// block role [`BlockRole::Inline`]. Attach it by its trait-object type, with
/// its implementation [`IsWhitespace`]:
/// `.with::<dyn Whitespace>(Arc::new(IsWhitespace))` on an
/// [`ElementSpec`](crate::model::element::ElementSpec).
pub trait Whitespace: Send + Sync {}

/// The implementation of [`Whitespace`]. It carries no data: the presence of
/// the aspect is the whole statement.
#[derive(Debug, Clone, Copy, Default)]
pub struct IsWhitespace;

impl Whitespace for IsWhitespace {}

/// Whether the node `node` is whitespace: whether its element carries the
/// [`Whitespace`] aspect. `false` for an element that does not declare that
/// aspect.
pub fn is_whitespace_of(node: &NodeRef<'_, Flm>) -> bool {
    node.ext().element.has_aspect::<dyn Whitespace>()
}

// --- content references ---------------------------------------------------------------

/// Document content that an aspect returns: a reference to content *of the
/// node the aspect was asked about* (one of its arguments, its body, or one
/// node of the same tree) or, for content that no node holds by itself, a
/// small tree the aspect built.
///
/// An aspect refers to content wherever it can, for the reason given in the
/// [module documentation](self#content-an-aspect-returns). A consumer reads a
/// reference on the node that the aspect was asked about, with
/// [`ContentRef::nodes`]; a [`ContentRef::Synthesized`] tree is read as it is.
///
/// # Comparing two references
///
/// Two references are equal when they designate the same content. Two
/// synthesized trees are equal only when they are the *same* tree, shared
/// through one pointer (as the lists feature shares the one bullet tree it
/// builds per configured bullet with every item), and never because two trees
/// built separately hold the same text: a tree holds spans, sources and
/// parsing states that cannot be compared, and nothing in the library compares
/// content by its text. The hash of a [`ContentRef::Synthesized`] covers the
/// variant alone, so that it depends on no memory address and is the same on
/// every run.
#[derive(Debug, Clone)]
pub enum ContentRef {
    /// The content of the argument at this position, counted from zero in the
    /// order the arguments are written at the call.
    Argument(usize),
    /// The content of the argument with this name.
    ArgumentNamed(String),
    /// The node's body: the content of an environment, or of a command that
    /// takes a body.
    Body,
    /// One node of the same tree, by its identifier: the node itself, one of
    /// its descendants, or any other node of that tree.
    Node(NodeId),
    /// Content that no node of the tree holds by itself, built by the aspect as
    /// a small tree of its own: the bullet that a list's configuration places
    /// beside its items, or the value `Custom` of `tag=Custom`, which shares a
    /// run of characters with its key.
    ///
    /// The tree has the shape of a synthesized value built by copying
    /// ([`crate::lang::copy`]): its root is a list node that holds the content
    /// and adds nothing of its own (no delimiters, no text), so the content is
    /// read as written inside it. A consumer uses the tree like a synthesized
    /// value copied out of the document (the references feature shows it
    /// where a reference to the point shows content); the tree is not part of
    /// the document's own trees.
    ///
    /// An aspect that returns the same content for many points builds it once
    /// and shares it: the lists feature builds one tree per configured bullet
    /// when the registry is built. See [comparing two
    /// references](ContentRef#comparing-two-references) for how two such trees
    /// compare.
    Synthesized(Arc<NodeTree<Flm>>),
}

impl PartialEq for ContentRef {
    fn eq(&self, other: &ContentRef) -> bool {
        match (self, other) {
            (ContentRef::Argument(a), ContentRef::Argument(b)) => a == b,
            (ContentRef::ArgumentNamed(a), ContentRef::ArgumentNamed(b)) => a == b,
            (ContentRef::Body, ContentRef::Body) => true,
            (ContentRef::Node(a), ContentRef::Node(b)) => a == b,
            // The same tree, never two trees holding the same text.
            // See decision rationale at [§flm-dd-dr:dm-content-synthesized].
            (ContentRef::Synthesized(a), ContentRef::Synthesized(b)) => Arc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for ContentRef {}

impl Hash for ContentRef {
    fn hash<H: Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            ContentRef::Argument(index) => index.hash(state),
            ContentRef::ArgumentNamed(name) => name.hash(state),
            ContentRef::Body => {}
            ContentRef::Node(id) => id.hash(state),
            // The variant alone: equal trees are one pointer, and hashing the
            // pointer would make a hash differ from run to run.
            ContentRef::Synthesized(_) => {}
        }
    }
}

impl ContentRef {
    /// The nodes this reference designates, read on `node`, the node whose
    /// aspect returned the reference: the content of an argument, the body, or
    /// one node of the tree; for [`ContentRef::Synthesized`], the children of
    /// the root of the tree it carries.
    ///
    /// Returns `None` when the reference does not apply to `node`: an argument
    /// that is absent or does not exist, no body, or no node with that
    /// identifier in the tree.
    pub fn nodes<'t>(&'t self, node: &NodeRef<'t, Flm>) -> Option<Vec<NodeRef<'t, Flm>>> {
        match self {
            ContentRef::Argument(index) => Some(node.argument_content_nodes(*index)?.iter().collect()),
            ContentRef::ArgumentNamed(name) => {
                Some(node.argument_content_nodes_named(name).ok()??.iter().collect())
            }
            ContentRef::Body => Some(node.body()?.iter().collect()),
            ContentRef::Node(id) => Some(vec![node.tree().get(*id)?]),
            ContentRef::Synthesized(tree) => Some(tree.root().children().iter().collect()),
        }
    }
}

// --- content apart from the text around it ----------------------------------------------

/// The aspect that gives the content of an occurrence that is not part of the
/// text of the content around it.
///
/// The **text** of a run of content is its words, read out of the tree
/// ([`plain_text_of`](crate::lang::text::plain_text_of)); for example, a
/// heading without a label gets an identifier encoded from the text of its
/// title. Some content is written inside a run without being part of its
/// words: a footnote written in a heading's title is shown at the foot of the
/// page, and its words are not the title's; the label that a `\ref` refers to
/// is not a word of the text. An element that holds such content returns it
/// from this aspect, as [content references](ContentRef) on the node, and
/// every reading of the text of an enclosing run leaves that content out,
/// with everything inside it. The rest of the element, such as its other
/// arguments, is read as usual. A [`ContentRef::Synthesized`] designates no
/// content of the tree, and the reading ignores it.
///
/// An element that does not declare this aspect has no such content. Read the
/// result with [`apart_from_text_of`], which returns an empty list for an
/// element that does not declare the aspect.
pub trait ApartFromText: Send + Sync {
    /// The content of this occurrence that is not part of the text around it,
    /// as references on `node`.
    fn apart_from_text(&self, node: &NodeRef<'_, Flm>) -> Vec<ContentRef>;
}

/// The content of the node `node` that is not part of the text around it: the
/// result of its element's [`ApartFromText`] aspect, or an empty list when its
/// element does not declare that aspect.
pub fn apart_from_text_of(node: &NodeRef<'_, Flm>) -> Vec<ContentRef> {
    match node.ext().element.aspect::<dyn ApartFromText>() {
        Some(aspect) => aspect.apart_from_text(node),
        None => Vec::new(),
    }
}

// --- what stands for a node in a copy ----------------------------------------------------

/// What replaces a node in a copy of content shown a second time: the result
/// of the element's [`InCopy`] aspect, read with [`copied_as`].
///
/// A copy shown again is content of the document copied into a synthesized
/// value ([`crate::lang::copy`]) to be shown a second time, away from where it
/// is written, such as a heading's title that a reference prints
/// ([`ValueContent::Shown`]). The copy operation reads this for every node it
/// copies, at every depth, and stages (builds into the copy) what it says in
/// the node's place.
#[derive(Debug, Clone)]
pub enum CopiedAs {
    /// The node itself, copied as written with its span and source; its
    /// children are copied as their own elements say. This is the result for
    /// an element that does not declare the [`InCopy`] aspect.
    Verbatim,
    /// Nothing: the copy holds nothing in the node's place, and nothing of the
    /// node's descendants.
    Nothing,
    /// One new node in the node's place, described by the [`StandIn`]; the
    /// node's descendants are not copied.
    StandIn(StandIn),
}

/// A new node that a copy stages in place of a node of the document
/// ([`CopiedAs::StandIn`]): a node of a [synthesized
/// element](crate::lang::mint::SyntaxDef::Synthesized) (an element that no
/// document writes), carrying data of its own and holding no children.
///
/// The copy stages it with the span and the parsing state of the node it
/// replaces, as it does for every copied node, so that a diagnostic about it
/// points where the document wrote that node. The feature whose aspect returns
/// the stand-in owns its element and its payload: the feature declares the
/// element, with a lowering rule on every backend, and puts in the payload
/// what that rule needs to know about the node it replaces. For example, the
/// stand-in of an xnote (a note set apart from the running text, such as a
/// footnote; see [`xnotes`](crate::stdcore::features::xnotes)) identifies the
/// xnote, and its lowering rules write the xnote's mark.
#[derive(Clone)]
pub struct StandIn {
    /// The specification of the new node, which carries its synthesized
    /// element.
    pub spec: Arc<FlmSynthesizedSpec>,
    /// The node's own data, attached to its node extension
    /// ([`FlmNodeExt::payload`](crate::lang::FlmNodeExt::payload)), or `None`
    /// for a node without data.
    pub payload: Option<Arc<dyn PresPayload>>,
}

impl fmt::Debug for StandIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StandIn")
            .field("element", &self.spec.element().name)
            .field("payload", &self.payload)
            .finish()
    }
}

/// The aspect that says what replaces an occurrence in a copy of content
/// shown a second time ([`CopiedAs`]): the node as written, nothing, or a new
/// node of the element's own.
///
/// Content of the document is copied into a synthesized value
/// ([`crate::lang::copy`]) to be shown a second time, away from where it is
/// written: a heading's title that a reference prints, the caption of an
/// unnumbered float, a defined term, the tag of a list item. Most content is
/// shown again as it was written, and an element that does not declare this
/// aspect is copied as written. An element that must not be repeated returns
/// something else. For example, an xnote shown again would be a second note
/// with the same number and the same labels, so the xnotes feature returns a
/// new node of its own in the xnote's place, which its lowering rules write as
/// the xnote's mark.
///
/// The copy operation reads the aspect when the copy is made, once per node
/// and per copy, for every node it copies at every depth: the children of a
/// node that gives [`CopiedAs::Verbatim`] are read in turn, and the copy does
/// not descend into a node that gives anything else. Like every aspect, it is
/// evaluated per occurrence, from the element's data and the node.
///
/// Only a copy shown again ([`ValueContent::Shown`]) reads the aspect. A copy
/// that the synthesized value holds as its own content ([`ValueContent::Own`])
/// is copied as written regardless of the aspect: a use site of a user
/// definition copied to be expanded *is* the document's content at that
/// place, and content copied to be read must be what was written.
///
/// The library has no built-in stand-in element: the element, its payload and
/// its lowering rules belong to the feature that returns the stand-in. Read the
/// result with [`copied_as`], which gives [`CopiedAs::Verbatim`] for an
/// element that does not declare the aspect and for every copy of
/// [`ValueContent::Own`] content.
pub trait InCopy: Send + Sync {
    /// What replaces this occurrence in a copy made for `purpose`.
    ///
    /// Only a copy shown again reads the aspect, so `purpose` is currently
    /// always [`ValueContent::Shown`]; it is passed so that an element can
    /// distinguish a purpose added later.
    fn in_copy(&self, node: &NodeRef<'_, Flm>, purpose: ValueContent) -> CopiedAs;
}

/// What replaces the node `node` in a copy made for `purpose`: for a copy
/// shown again ([`ValueContent::Shown`]), the result of its element's
/// [`InCopy`] aspect; [`CopiedAs::Verbatim`] for a copy of
/// [`ValueContent::Own`] content and for an element that does not declare the
/// aspect.
pub fn copied_as(node: &NodeRef<'_, Flm>, purpose: ValueContent) -> CopiedAs {
    if purpose == ValueContent::Own {
        return CopiedAs::Verbatim;
    }
    match node.ext().element.aspect::<dyn InCopy>() {
        Some(aspect) => aspect.in_copy(node, purpose),
        None => CopiedAs::Verbatim,
    }
}

// --- reference points ---------------------------------------------------------------

/// A label, as written in `\label{type:label}`: the label type before the
/// first colon, and the label proper after it.
///
/// A label is the text an author writes as `\label{type:label}` after a
/// construct, which the construct reads as a field and attaches to its
/// reference point (see [reference points, labels and
/// anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors)).
/// The reference point carries its labels in [`RefPoint::labels`], and a
/// reference site gives the label it points at in [`RefSite::target`]. The
/// label type is part of the label and does not depend on the [reference
/// kind](RefPoint::kind) of the point, so one point may carry labels of several
/// types, among those the point [accepts](AcceptedRefTypes), and each label is
/// looked up as a whole.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Label {
    /// The label type (`sec` in `sec:intro`).
    pub ref_type: String,
    /// The label proper (`intro` in `sec:intro`).
    pub ref_label: String,
}

impl Label {
    /// A label made of the label type `ref_type` and the label proper
    /// `ref_label`, taken as given (no trimming, no check).
    pub fn new(ref_type: impl Into<String>, ref_label: impl Into<String>) -> Label {
        Label { ref_type: ref_type.into(), ref_label: ref_label.into() }
    }

    /// Reads a label from its written form `type:label`: the text before the
    /// first colon is the label type, everything after it (further colons
    /// included) is the label proper. In each part, every run of whitespace
    /// (the space, the tab, the carriage return and the line feed: [the
    /// whitespace of FLM text](crate::lang::text#whitespace)) is made one space
    /// and the whitespace at both ends is removed, as in a citation key
    /// ([`spelling_identity`](crate::stdcore::backends::pres::spelling_identity));
    /// every other character counts as written, quotes, dashes and the no-break
    /// space U+00A0 included. A label written with a doubled space, or broken
    /// over two lines, is therefore the label written with one space.
    ///
    /// The standard features read every label a document writes with this
    /// function (a `\label`, the target of a `\ref`, a `\hyperref` or an
    /// `\eqref`, a proof's back-reference), so that a label and a reference to
    /// it agree.
    ///
    /// Returns `None` when the text holds no colon, or when either part is
    /// empty after the whitespace is removed. A caller that reads a label from
    /// a document reports that case with a diagnostic at the label's span (see
    /// [`MalformedLabel`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::model::aspects::Label;
    ///
    /// let label = Label::parse("sec:intro").unwrap();
    /// assert_eq!(label.ref_type, "sec");
    /// assert_eq!(label.ref_label, "intro");
    /// assert_eq!(label.to_string(), "sec:intro");
    /// assert_eq!(Label::parse(" sec : a \n  b "), Some(Label::new("sec", "a b")));
    /// assert!(Label::parse("no-colon-here").is_none());
    /// ```
    pub fn parse(text: &str) -> Option<Label> {
        let (ref_type, ref_label) = text.split_once(':')?;
        let (ref_type, ref_label) = (collapse_whitespace(ref_type), collapse_whitespace(ref_label));
        if ref_type.is_empty() || ref_label.is_empty() {
            return None;
        }
        Some(Label::new(ref_type, ref_label))
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.ref_type, self.ref_label)
    }
}

/// How the anchor of a reference point is formed: the identifier a link in the
/// output points at.
///
/// The element declares the policy for each point; the refs feature computes
/// the identifier from it in its anchors stage, in document order, and records
/// it in the table [`ANCHORS`](crate::stdcore::features::refs::ANCHORS), so that the same document always
/// gives the same identifiers. A point whose identifier would be one an earlier
/// point already has is reported and gets no anchor, except under
/// [`AnchorPolicy::Slug`], where the later point gets a number appended
/// instead.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AnchorPolicy {
    /// This identifier, used as given.
    Explicit(String),
    /// Encoded from the point's first remaining label: the first of its labels
    /// that no earlier point carries and whose type the point accepts
    /// ([`AcceptedRefTypes`]). A point with no such label gets no anchor.
    FromFirstLabel,
    /// Encoded from the given content, such as a heading's title, after a
    /// prefix and a double hyphen, as in `sec--The_Title`. An identifier
    /// encoded from the title rather than from a number does not change when
    /// text is inserted before the point, so a link from outside the document
    /// keeps working. Where several points encode to the same identifier, the
    /// later ones get `-2`, `-3`, … appended in document order
    /// (`sec--The_Title-2`).
    ///
    /// The element gives the prefix, so that an identifier encoded from the
    /// title and one encoded from a label can begin alike: a heading gives
    /// `sec`, the label type it accepts, and both `\label{sec:intro}` and an
    /// unlabeled title give an identifier beginning with `sec`.
    Slug {
        /// The text written before the double hyphen.
        prefix: String,
        /// The content the identifier is encoded from.
        content: ContentRef,
    },
    /// The prefix, a hyphen, and the number the document gave the point: the
    /// values of the counters it is numbered within, outermost first, then its
    /// own number, each written in decimal and joined by hyphens, as in
    /// `theorem-2-3`.
    ///
    /// The element gives the prefix, as under [`AnchorPolicy::Slug`] and
    /// [`AnchorPolicy::Auto`]. The numbers are written in decimal regardless of
    /// how the counter style shows them, so that the identifier does not change
    /// when the counter style does;
    /// [`value_anchor`](crate::stdcore::features::refs::value_anchor) writes
    /// them.
    ///
    /// **A point that the document did not number** (because a counter it
    /// depends on has no value yet, or because no numbering feature is
    /// registered) falls back to what [`AnchorPolicy::FromFirstLabel`] gives:
    /// the point's first remaining label if it has one, and no identifier
    /// otherwise. The identifier of a point under this policy therefore
    /// depends on the numbering.
    FromValue {
        /// The text written before the number.
        prefix: String,
    },
    /// The point's first remaining label if it has one; otherwise the prefix,
    /// a double hyphen, the point's position among the points of its reference
    /// kind, and four lowercase letters, as in `eq--8kqzt`.
    ///
    /// A point with a remaining label gets the identifier
    /// [`AnchorPolicy::FromFirstLabel`] would give, never one from its
    /// position, so that its identifier does not change when a point is
    /// inserted before it. A point without a label gets an identifier from its
    /// position, so that every point has an identifier that a link from
    /// outside the document can use.
    ///
    /// The element gives the prefix, as under [`AnchorPolicy::Slug`]: a
    /// display-math environment gives `eq`, the label type its lines accept,
    /// so that the identifiers of a labeled line (`eq-euler`) and of an
    /// unlabeled one begin alike.
    ///
    /// **The four letters** are computed from the prefix and the position
    /// alone, so the same position gives the same identifier in every document
    /// and on every run. The position alone already makes the identifier
    /// unique; the letters keep it from reading as a number of the document.
    /// Written as `eq--8`, the identifier would suggest equation (8), which it
    /// is not as soon as an unnumbered equation comes before it.
    /// [`auto_anchor`](crate::stdcore::features::refs::auto_anchor) computes
    /// the identifier and documents how the letters are derived.
    Auto {
        /// The text written before the double hyphen.
        prefix: String,
    },
}

/// What a reference point is numbered by: the counter it counts in, the
/// counter style its number is shown in, the scoped instances it counts
/// under, the levels of the counter it advances, and optionally a number
/// template of its own.
///
/// A counter is a dotted name under which things are counted in document
/// order ([`CounterDecl`](crate::stdcore::features::numbering::CounterDecl)),
/// and a counter style says how a counter value is shown
/// ([`CounterStyleDecl`](crate::stdcore::features::numbering::CounterStyleDecl)). A
/// point that is to carry a number gives a number source in
/// [`RefPoint::number`], and the numbering feature
/// ([`numbering`](crate::stdcore::features::numbering)) numbers every point
/// that gives one; no setting of the numbering feature turns numbering off for
/// a point. A point that is not to
/// carry a number gives none: a line of an `equation*` environment, a heading
/// written with a star, a heading of a kind that the headings configuration
/// does not number. The element decides from the node and from its feature's
/// configuration, fixed when the registry was built, since an aspect reads no
/// option; the same document is therefore numbered the same way on every
/// output.
///
/// The counter and the counter style are given by name. A point that gives a
/// counter or a counter style that no feature declares is reported by the
/// numbering stage, not here.
///
/// # Scoped instances
///
/// A point may count in a **scoped instance** of its counter rather than in
/// the counter's single sequence through the document. A scoped instance is an
/// independent sequence of the counter's values attached to a node, usually
/// the environment the point is written in, and it covers exactly that node's
/// content: two sibling environments are two instances, and each starts again
/// from the counter's initial value. The point lists the instances it counts
/// under in [`instances`](NumberSource::instances), outermost first, the last
/// being its own. An instance inside another is numbered within the current
/// value of the enclosing instance, so the item of a nested list reads `2.(b)`
/// or `(b)` depending on what the counter writes for its enclosing levels. The
/// [numbering guide](crate::guide::numbering) describes the whole mechanism.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct NumberSource {
    /// The name of the counter the point counts in, such as
    /// `flm.headings.section` or `flm.math.equation`. The feature that
    /// declares the counter owns its name; points of several reference kinds,
    /// from any feature, may count in one counter.
    pub counter: Name,
    /// The scoped instances of the counter that the point counts under,
    /// outermost first: nodes of the tree of the node the aspect was asked
    /// about, which contain the point's node and each hold one sequence of the
    /// counter's values. The last is the point's own instance; each one
    /// encloses the next. Empty for a point that counts in the counter's single
    /// sequence through the document, as nearly every point does.
    ///
    /// Each instance must be a node that contains the point's node (never the
    /// node that carries the point itself) and must enclose the next instance;
    /// otherwise the point is not numbered and the numbering stage reports the
    /// Error `flm.numbering.instance-not-enclosing`. An instance is given by
    /// its node's identifier, as in [`ContentRef::Node`], because an aspect is
    /// given a node of one tree and not the tree's place in the document; the
    /// numbering stage records the instance as the full [`NodeAddr`] of that
    /// node.
    pub instances: Vec<NodeId>,
    /// The name of the counter style the point's number is shown in, which
    /// gives the number format, the word before the number and the
    /// delimiters.
    pub style: Name,
    /// Which levels of the counter this point advances, in the order it
    /// advances them.
    ///
    /// A counter value is a number with optional sub-numbers: level 0 is the
    /// number, level 1 the first sub-number, and advancing a level resets the
    /// deeper levels to zero. Most points advance `[0]` and show `(3)`, then
    /// `(4)`. A group of sub-numbered lines is written by advancing two levels
    /// at once on the first line and the deeper level alone on the rest: the
    /// first line advances `[0, 1]` and shows `(3a)`, the lines after it
    /// advance `[1]` and show `(3b)`, `(3c)`, and the next equation advances
    /// `[0]` and shows `(4)`.
    pub advance: Vec<u32>,
    /// A number template of the point's own, or `None` to use the number
    /// format of the counter style ([`style`](NumberSource::style)) as
    /// declared.
    ///
    /// The text is a number template
    /// ([`NumberTemplate`](crate::stdcore::features::numbering::NumberTemplate)):
    /// every `#{name}` is the number in the format `name` (`#{arabic}`,
    /// `#{alph}`, `#{roman}`, …), every other character is written as given,
    /// and markup may surround the number: `(#{alph})`, `#{Roman}.`. The text
    /// is the author's input, like the text of a `\tag`; for example, a list's
    /// reference template written `ref=(#{alph})`, or `label=(#{alph})` alone.
    /// The aspect passes it on as written, without compiling it, since an
    /// aspect has no language to compile it with. The numbering stage compiles
    /// each distinct text once per run and per counter style, and records the
    /// point in the counter style [`style`](NumberSource::style) with its
    /// number format replaced by this template, so that the point's number and
    /// every reference to it use the template: `(b)`, `Item~(b)`. A template
    /// that does not compile is reported as the Error
    /// `flm.numbering.bad-number-format`, and the point is numbered with the
    /// counter style as declared.
    pub number_format: Option<String>,
}

/// The label types a reference point accepts.
///
/// A label is written `type:label`, and the part before the colon, the [label
/// type](Label::ref_type), says what sort of thing is labeled: `sec:intro`
/// labels a section, `eq:euler` an equation. A reference point declares which
/// label types may be written for it, so that a label of the wrong type is
/// reported instead of becoming a working link.
///
/// [`AcceptedRefTypes::Any`] accepts every label type. It suits a construct
/// whose label types are not fixed in advance, and it is the default.
///
/// The labels of a document are checked against this declaration when they
/// are collected: a written label whose type the point does not accept
/// defines nothing (no reference resolves to it), and the label check
/// ([`CHECK_LABELS`](crate::stdcore::features::refs::CHECK_LABELS)) reports it
/// as `flm.refs.label-type-not-accepted`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub enum AcceptedRefTypes {
    /// Every label type is accepted.
    #[default]
    Any,
    /// Only these label types, in the order the point declares them. An empty
    /// list accepts no label.
    These(Vec<String>),
}

impl AcceptedRefTypes {
    /// A point that accepts every label type.
    pub fn any() -> AcceptedRefTypes {
        AcceptedRefTypes::Any
    }

    /// A point that accepts these label types and no others.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::model::aspects::AcceptedRefTypes;
    ///
    /// let accepted = AcceptedRefTypes::these(["sec", "topic"]);
    /// assert!(accepted.accepts("sec"));
    /// assert!(!accepted.accepts("eq"));
    /// assert!(AcceptedRefTypes::any().accepts("eq"));
    /// ```
    pub fn these(types: impl IntoIterator<Item = impl Into<String>>) -> AcceptedRefTypes {
        AcceptedRefTypes::These(types.into_iter().map(Into::into).collect())
    }

    /// Returns `true` when a label of the type `ref_type` may be written for
    /// the point.
    pub fn accepts(&self, ref_type: &str) -> bool {
        match self {
            AcceptedRefTypes::Any => true,
            AcceptedRefTypes::These(types) => types.iter().any(|one| one == ref_type),
        }
    }

    /// The accepted label types, or `None` when every type is accepted. A
    /// report about a refused label lists these types.
    pub fn listed(&self) -> Option<&[String]> {
        match self {
            AcceptedRefTypes::Any => None,
            AcceptedRefTypes::These(types) => Some(types),
        }
    }
}

/// A text written as a label for a reference point that is not of the form
/// `type:label`, such as `\label{intro}`, with no colon.
///
/// Such a text defines no label: no reference resolves to it, and a point
/// whose labels are all of this kind is as unlabeled as one with no `\label`
/// at all. The construct that read the text records it on the point it
/// declares ([`RefPoint::malformed`]), and the refs feature
/// ([`crate::stdcore::features::refs`]) reports it once, as
/// `flm.refs.bad-label`, at the span of the field it was written in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MalformedLabel {
    /// The text as written, trimmed.
    pub text: String,
    /// The span of the field it was written in.
    pub span: FlmSpan,
    /// The position of this text among the point's labels, counted from zero
    /// over the labels ([`RefPoint::labels`], the computed ones first) and
    /// these texts together, in the order the document wrote them.
    ///
    /// The labels and these texts are kept apart because almost every reader
    /// wants the labels alone. This position lets a reader that needs the
    /// written order restore it, as the recomposed LaTeX output does when it
    /// gives each label a key of its own.
    pub position: usize,
}

/// A reference point: one place in a document that can be referred to,
/// numbered, or given an anchor, such as a heading, an equation line, a
/// theorem or a footnote.
///
/// An element declares the reference points of each occurrence through the
/// [`Referenceable`] aspect, and the `refpoints` feature records them for every
/// tree ([`REFPOINTS`](crate::stdcore::features::refpoints::REFPOINTS)). A node
/// carries zero or more points: a display-math environment has one point per
/// equation line, a heading has one, and an element that nothing can point at
/// does not declare the aspect. A point is identified by a [`PointId`]. See
/// [reference points, labels and
/// anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors).
///
/// # What a reference to the point shows
///
/// A point gives the two things it may show, and a reference shows the first
/// of these that exists: the point's **number**, shown in the counter style of
/// its [number source](RefPoint::number), when the numbering gave it one in
/// this document; otherwise its [content](RefPoint::content), such as a
/// heading's title; otherwise the label the reference was written with, as
/// written.
///
/// A point gives both because a point with a number source may still get no
/// number in a run (no numbering feature registered, a counter that no feature
/// declares, an enclosing counter with no value) and must then have something
/// else to show. The point's element decides whether the point has a number
/// source, from its feature's configuration fixed when the registry was
/// built, and every point that has one is numbered where possible. What a
/// reference shows is therefore decided by the target; the reference site
/// decides only how it is presented ([`RefDisplayOptions`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefPoint {
    /// The reference kind: what sort of thing the point is, such as
    /// `section`, `equation` or `theorem`. The set of kinds is open: a feature
    /// uses the kinds it needs.
    pub kind: RefKind,
    /// The labels of this point: first the [`computed`](Self::computed)
    /// ones, which the element computed, then the labels written for the point
    /// in the document, in the order they were written.
    pub labels: Vec<Label>,
    /// The number of the point's computed labels: the first `computed`
    /// [labels](Self::labels) were computed by the element, not written by the
    /// author, and the accepted label types ([`accepts`](Self::accepts)) do not
    /// apply to them. `0` for a point whose labels are all written, which is
    /// every point except one whose element derives labels from the document,
    /// as a term definition (the `defterm` environment) does from its term.
    pub computed: usize,
    /// The texts written as labels for this point that are not of the form
    /// `type:label`, in the order they were written, each with its
    /// [position](MalformedLabel::position) among the point's labels. They
    /// define nothing; the refs feature reports each one.
    pub malformed: Vec<MalformedLabel>,
    /// How the point's anchor, the identifier a link points at, is formed.
    pub anchor: AnchorPolicy,
    /// What the point is numbered by, or `None` for a point that carries no
    /// number. See [`NumberSource`] and [what a reference to the point
    /// shows](Self#what-a-reference-to-the-point-shows).
    pub number: Option<NumberSource>,
    /// The content a reference shows when the point has no number, such as a
    /// heading's title, the text of an explicit tag or the bullet beside a
    /// list's item: a reference into the frozen tree, or a tree the aspect
    /// built when no node holds the content by itself
    /// ([`ContentRef::Synthesized`]). `None` for a point with no content to
    /// show.
    pub content: Option<ContentRef>,
    /// The label types that may be written for this point. A written label of
    /// another type defines nothing and is reported; see [`AcceptedRefTypes`].
    /// The check applies to what the author wrote only: the first
    /// [`computed`](Self::computed) labels are not checked.
    pub accepts: AcceptedRefTypes,
}

/// The identifier of one reference point: the address of the node that
/// carries it and the point's index among that node's points.
///
/// A node's element returns the node's points in document order
/// ([`Referenceable::points`]), and the index is the point's position in that
/// list, counted from zero. The pair identifies the point within the whole
/// document; every table about reference points uses it as its key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PointId {
    /// The node carrying the point.
    pub addr: NodeAddr,
    /// The point's index among the node's points, counted from zero in
    /// document order.
    pub index: u32,
}

/// The aspect that declares the [reference points](RefPoint) a node carries:
/// the places in it that a reference can point at.
///
/// The `refpoints` feature reads this aspect on every node whose element
/// declares it, and the refs feature ([`crate::stdcore::features::refs`])
/// collects the labels written for each point and computes the anchors. Any
/// element may declare the aspect, so a construct that a third-party feature
/// declares is referred to exactly like a standard heading.
///
/// See [`Referencing`] for the places that point at a reference point.
pub trait Referenceable: Send + Sync {
    /// The reference points of this occurrence, in document order.
    ///
    /// Each point declares the label types it accepts
    /// ([`RefPoint::accepts`]); a construct with no fixed set of label types
    /// declares [`AcceptedRefTypes::Any`].
    fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<RefPoint>;
}

// --- reference sites ------------------------------------------------------------------

/// How one reference site presents the reference point it points at.
///
/// A prefix is the word a reference writes before the target's number or
/// title, such as `Section` or `Eq.`; these options say which form of the
/// prefix to use and whether to show it. Each site records them through
/// [`Referencing`], as the document wrote them in the brackets of
/// `\ref[…]{…}`, so that an output that leaves the formatting of references
/// to another program (such as the recomposed LaTeX) can write them back
/// unchanged.
///
/// The lowering rule that shows a reference reads them; the standard rule
/// translates them into the terms of the counter style when it writes a
/// target's number
/// ([`lower_reference`](crate::stdcore::features::refs::lower_reference)).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RefDisplayOptions {
    /// The name of the prefix form asked for at this site, or `None` to leave
    /// the choice to the target.
    pub variant: Option<String>,
    /// Whether the prefix is shown before the target's number or title.
    pub with_prefix: bool,
}

impl Default for RefDisplayOptions {
    fn default() -> RefDisplayOptions {
        RefDisplayOptions { variant: None, with_prefix: true }
    }
}

/// A reference site: one place in a document that points at a [reference
/// point](RefPoint), such as a `\ref`, a `\hyperref`, a proof's
/// back-reference to its theorem, or a link to a defined term.
///
/// An element declares the reference sites of each occurrence through the
/// [`Referencing`] aspect, and the refs feature
/// ([`crate::stdcore::features::refs`]) resolves them. A node carries zero or
/// more sites. See [reference points, labels and
/// anchors](crate::guide::concepts_overview#reference-points-labels-and-anchors).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RefSite {
    /// The label this site points at. The refs feature looks it up among the
    /// labels collected from every reference point of the document.
    pub target: Label,
    /// How the resolved target is presented here.
    pub display: RefDisplayOptions,
    /// Whether the site is shown in the output. A site that is not shown still
    /// records the connection between the two places, from which a list of
    /// what points at a target is built: a proof written `[**thm:a]` refers to
    /// the theorem without showing a reference in its heading. The element
    /// that declares the site reads this field where it writes the site (a
    /// theorem's heading shows a proof's back-reference only when the site is
    /// shown); the refs feature resolves every site in the same way.
    pub shown: bool,
}

/// The aspect that declares the [reference sites](RefSite) a node carries:
/// the places in it that point at a reference point. The reference points
/// themselves are declared by [`Referenceable`].
///
/// The refs feature ([`crate::stdcore::features::refs`]) resolves the sites of
/// every element that declares this aspect, so any construct can refer, not
/// only the reference command: a proof referring to its theorem, an equation
/// reference (`\eqref`), a link to a defined term. None of them has to look up labels or anchors, or
/// format the reference, by itself.
pub trait Referencing: Send + Sync {
    /// The reference sites of this occurrence, in document order.
    fn sites(&self, node: &NodeRef<'_, Flm>) -> Vec<RefSite>;
}

// --- sectioning -----------------------------------------------------------------------

/// The aspect that declares that an occurrence is a heading: its heading level,
/// whether it is listed in the table of contents, and what an entry of the
/// table of contents shows for it.
///
/// Every method takes the node, because whether a heading is listed in the
/// table of contents is a fact about the occurrence: the standard headings
/// feature ([`headings`](crate::stdcore::features::headings)) reads it from the
/// key `toc=false` written at the call, so two headings of the same kind may
/// differ. So is what an entry shows: the feature reads the alternative title
/// from the key `alt=` written at the call.
pub trait Sectioning: Send + Sync {
    /// The heading level of this occurrence: the smaller the number, the
    /// outer the heading. Every `u32`, `0` included, is a valid level; the
    /// standard heading kinds use 1 (`\section`) to 5 (`\subparagraph`).
    fn level(&self, node: &NodeRef<'_, Flm>) -> u32;

    /// Whether this occurrence is listed in the table of contents. The default
    /// implementation returns `true`.
    fn in_toc(&self, node: &NodeRef<'_, Flm>) -> bool {
        let _ = node;
        true
    }

    /// The content a table of contents shows for this occurrence, or `None`
    /// when it shows nothing but the occurrence's number. The default returns `None`.
    ///
    /// The standard headings return the alternative title written with the
    /// key `alt=`, copied into a synthesized value of its own
    /// ([`ContentRef::Synthesized`]), and the heading's title argument
    /// otherwise. A table of contents asks every heading through this method
    /// alone, so a heading kind of an extension decides what its entries show;
    /// the standard package's table of contents is the
    /// [`toc`](crate::stdcore::features::toc) feature.
    fn toc_title(&self, node: &NodeRef<'_, Flm>) -> Option<ContentRef> {
        let _ = node;
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_parses_at_the_first_colon() {
        let label = Label::parse("sec:a:b").unwrap();
        assert_eq!(label.ref_type, "sec");
        assert_eq!(label.ref_label, "a:b");
        assert_eq!(label.to_string(), "sec:a:b");
    }

    #[test]
    fn label_requires_both_parts() {
        assert!(Label::parse("nocolon").is_none());
        assert!(Label::parse(":x").is_none());
        assert!(Label::parse("sec: ").is_none());
    }

    #[test]
    fn the_default_display_options_show_the_prefix() {
        assert!(RefDisplayOptions::default().with_prefix);
    }

    #[test]
    fn the_default_block_role_is_inline_content() {
        assert_eq!(BlockRole::DEFAULT, BlockRole::Inline(InlineSpacing::CONTENT));
    }

    /// `\\gone{…}` answers nothing in a copy shown again, `\\held{…}` a
    /// stand-in, and every other node declares no [`InCopy`].
    mod in_copy {
        use super::*;
        use crate::lang::mint::{ArgDef, ElementDef, SyntaxDef};
        use crate::lang::{FlmSource, LangInputs, ParseOptions, build_language, parse_fragment};
        use crate::model::doc::FragmentLayout;
        use crate::model::element::ElementSpec;

        struct Answers(fn() -> CopiedAs);

        impl InCopy for Answers {
            fn in_copy(&self, _node: &NodeRef<'_, Flm>, _purpose: ValueContent) -> CopiedAs {
                (self.0)()
            }
        }

        fn stand_in() -> CopiedAs {
            let spec = FlmSynthesizedSpec::new(Arc::new(ElementSpec::new("t.stand-in")));
            CopiedAs::StandIn(StandIn { spec: Arc::new(spec), payload: None })
        }

        fn answers(purpose: ValueContent) -> Vec<(String, &'static str)> {
            let mut inputs = LangInputs::new();
            for (name, answer) in [("gone", (|| CopiedAs::Nothing) as fn() -> CopiedAs), ("held", stand_in)] {
                inputs.elements.push(ElementDef::new(
                    "t",
                    [name],
                    SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] },
                    Arc::new(
                        ElementSpec::new(format!("t.{name}"))
                            .with::<dyn InCopy>(Arc::new(Answers(answer)) as Arc<dyn InCopy>),
                    ),
                ));
            }
            let lang = build_language(inputs);
            let source = Arc::new(FlmSource::new("a\\gone{b}\\held{c}"));
            let fragment = parse_fragment(&lang, source, FragmentLayout::Inline, &ParseOptions::default())
                .expect("a parse");
            fragment
                .tree()
                .root()
                .children()
                .iter()
                .map(|node| {
                    let answer = match copied_as(&node, purpose) {
                        CopiedAs::Verbatim => "verbatim",
                        CopiedAs::Nothing => "nothing",
                        CopiedAs::StandIn(stand_in) => {
                            assert_eq!(stand_in.spec.element().name.as_str(), "t.stand-in");
                            "stand-in"
                        }
                    };
                    (node.ext().element.name.to_string(), answer)
                })
                .collect()
        }

        #[test]
        fn a_copy_shown_again_takes_each_element_s_answer_and_verbatim_by_default() {
            let answers = answers(ValueContent::Shown);
            let kinds: Vec<&str> = answers.iter().map(|(_, answer)| *answer).collect();
            assert_eq!(kinds, ["verbatim", "nothing", "stand-in"]);
            assert_eq!(answers[1].0, "t.gone");
        }

        #[test]
        fn a_copy_of_the_value_s_own_is_verbatim_whatever_the_element_answers() {
            let answers = answers(ValueContent::Own);
            assert!(answers.iter().all(|(_, answer)| *answer == "verbatim"));
            assert_eq!(answers.len(), 3);
        }
    }
}
