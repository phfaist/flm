//! The names of the presentation elements and of their slots.
//!
//! This module holds one constant per presentation element, spelled
//! `flm.pres.<name>`; the list [`ALL`] of every element, in the order
//! [`register`](super::register()) declares them; three groups of elements;
//! the names of the content slots ([`BODY_SLOT`], [`LABEL_SLOT`],
//! [`ITEM_SLOT`], [`CELL_SLOT`], [`ALT_SLOT`]); and the four kind tokens (classifying names
//! on a payload) with which a list states its shape and its labels
//! ([`LIST_RESERVED_KINDS`]).
//!
//! The three groups are [`DECORATIONS`], the elements whose nodes the
//! lowering driver moves out of a presentation node's content into an
//! attached slot; [`OPAQUE`], the elements whose content is copied as source
//! text; and [`CONTENT_IN_OPAQUE`], the elements whose content is document
//! content even inside one of those. Registration attaches an aspect to the
//! members of each group ([`super::aspects`]), and the driver and the
//! emitters read the aspect, never these lists.

/// `flm.pres.par` — a laid-out paragraph; its body is an inline run.
pub const PAR: &str = "flm.pres.par";
/// `flm.pres.block` — a block container with a role and kinds (see [roles,
/// kinds and document roles](super::payload#roles-kinds-and-document-roles));
/// its body is a run of blocks.
pub const BLOCK: &str = "flm.pres.block";
/// `flm.pres.block-heading` — a heading placed on its own between blocks,
/// with its heading kind's name, a level and whether it is listed in a table
/// of contents ([`HeadingPayload`](super::HeadingPayload)); its body is
/// inline.
pub const BLOCK_HEADING: &str = "flm.pres.block-heading";
/// `flm.pres.runin-heading` — the title of a run-in heading (a heading set at
/// the start of the paragraph it introduces), placed inside the run-in
/// decoration at the start of that paragraph. It has the same payload as a
/// [`BLOCK_HEADING`]: its heading kind's name, a level, whether it is listed
/// in a table of contents, and its anchor; its body is inline.
pub const RUNIN_HEADING: &str = "flm.pres.runin-heading";
/// `flm.pres.span` — inline content with a display style, a role and kinds
/// (see [roles, kinds and document
/// roles](super::payload#roles-kinds-and-document-roles)).
pub const SPAN: &str = "flm.pres.span";
/// `flm.pres.list` — a list: one node for the whole list, holding per entry a
/// `label` content slot followed by an `item` content slot. The slot names
/// repeat, so the slots are read by index ([`list_entries`](super::list_entries)).
pub const LIST: &str = "flm.pres.list";
/// `flm.pres.cells` — a table of cells: one node for the whole table, whose
/// payload ([`CellsPayload`](super::CellsPayload)) is the table's grid — its
/// size and one entry per cell — and which holds one `cell` content slot per
/// entry, in the same order ([`cells_of`](super::cells_of)).
pub const CELLS: &str = "flm.pres.cells";
/// `flm.pres.lines` — a sequence of `line` nodes, each printed on its own line.
pub const LINES: &str = "flm.pres.lines";
/// `flm.pres.line` — one line of a `lines` node, with its indentation and
/// alignment; its body is inline.
pub const LINE: &str = "flm.pres.line";
/// `flm.pres.verbatim` — byte-exact content of a verbatim kind
/// ([`VerbatimKind`](super::VerbatimKind): `text`, `code` or `a`), inline or
/// as a block, with an optional language tag.
pub const VERBATIM: &str = "flm.pres.verbatim";
/// `flm.pres.math-passthrough` — mathematics passed through as source text:
/// its body is the formula's content as written, which the
/// [`math-passthrough`](crate::math_passthrough) feature
/// stages and nothing in FLM has read (an [opaque
/// region](crate::guide::concepts_overview#opaque-regions)).
pub const MATH_PASSTHROUGH: &str = "flm.pres.math-passthrough";
/// `flm.pres.math-text` — a run of ordinary inline content inside a formula:
/// the argument of `\text{…}` and of the other text commands, or, holding
/// nothing and carrying an anchor, a link target placed at a point of the
/// formula. It occurs only inside a [`MATH_PASSTHROUGH`] node, and it is the only
/// thing inside such a node that is document content rather than source text.
pub const MATH_TEXT: &str = "flm.pres.math-text";
/// `flm.pres.link` — a link around inline display content. Its content may
/// hold another link; a backend that cannot write a link inside a link writes
/// the inner one as its text (the HTML page does).
pub const LINK: &str = "flm.pres.link";
/// `flm.pres.image` — an image; no content.
pub const IMAGE: &str = "flm.pres.image";
/// `flm.pres.unsupported` — the childless stand-in the lowering driver stages
/// for a node it cannot lower: a node of an accepted unrenderable element (an
/// element the plan request accepts that the backend cannot render; see
/// [`PlanRequest::unsupported`](crate::core::plan::PlanRequest#structfield.unsupported)),
/// a node whose lowering rule failed, and a node no rule lowers.
pub const UNSUPPORTED: &str = "flm.pres.unsupported";
/// `flm.pres.dropped` — the childless mark a lowering rule stages where it
/// left out something it could not place, carrying the condition of the
/// diagnostic it reported and a short statement of it
/// ([`DroppedPayload`](super::DroppedPayload)). Any rule that leaves something
/// out may stage one.
pub const DROPPED: &str = "flm.pres.dropped";
/// `flm.pres.unknown` — a macro or an environment the parse could not attribute
/// to any feature, carried into the output as the document content it is. The
/// payload holds the kind, the name and the source spelling of the invocation;
/// an environment's children are its lowered body, a macro's node is childless.
/// The output shows what the author wrote and nothing more: pointing the
/// mistake out to a reader is done from the run's diagnostics, not from this
/// node.
pub const UNKNOWN: &str = "flm.pres.unknown";
/// `flm.pres.deco.runin` — a decoration at the start of its block (a
/// theorem's heading). Like every decoration, the lowering driver moves it
/// into an attached slot.
pub const DECO_RUNIN: &str = "flm.pres.deco.runin";
/// `flm.pres.deco.final` — a decoration closing its block (a proof's closing
/// mark).
pub const DECO_FINAL: &str = "flm.pres.deco.final";
/// `flm.pres.deco.margin` — a decoration set beside the text, level with the
/// position of its decoration mark.
pub const DECO_MARGIN: &str = "flm.pres.deco.margin";
/// `flm.pres.deco-mark` — the decoration mark: the childless node the
/// lowering driver leaves where it moved a decoration out of a content slot.
/// Its payload ([`DecoMarkPayload`](super::DecoMarkPayload)) gives the index
/// of the attached slot that holds the decoration.
pub const DECO_MARK: &str = "flm.pres.deco-mark";
/// `flm.pres.subtree` — the position at which another document tree is
/// placed; its payload ([`SubtreePayload`](super::SubtreePayload)) identifies
/// that tree. An emitter that writes one output document inserts that tree's
/// own lowered form at this node.
pub const SUBTREE: &str = "flm.pres.subtree";

/// Every presentation element, in registration order.
pub const ALL: [&str; 22] = [
    PAR,
    BLOCK,
    BLOCK_HEADING,
    RUNIN_HEADING,
    SPAN,
    LIST,
    CELLS,
    LINES,
    LINE,
    VERBATIM,
    MATH_PASSTHROUGH,
    MATH_TEXT,
    LINK,
    IMAGE,
    UNSUPPORTED,
    DROPPED,
    UNKNOWN,
    DECO_RUNIN,
    DECO_FINAL,
    DECO_MARGIN,
    DECO_MARK,
    SUBTREE,
];

/// The decoration elements of the vocabulary: the elements whose nodes the
/// lowering driver moves out of a content slot into attached slots, leaving a
/// [`DECO_MARK`] node at each one's former position. The registration
/// attaches the [`Decoration`](super::Decoration) aspect to exactly these.
pub const DECORATIONS: [&str; 3] = [DECO_RUNIN, DECO_FINAL, DECO_MARGIN];

/// The elements whose content is opaque source text: the lowering driver
/// copies it unchanged and lowers nothing inside it, and the emitters write it
/// again byte for byte. The registration attaches the
/// [`OpaqueContent`](super::OpaqueContent) aspect to exactly these.
pub const OPAQUE: [&str; 2] = [VERBATIM, MATH_PASSTHROUGH];

/// The elements whose content is document content even when their node is
/// inside opaque content: the only exception to [`OPAQUE`]. The registration attaches the
/// [`ContentInOpaque`](super::ContentInOpaque) aspect to exactly these.
pub const CONTENT_IN_OPAQUE: [&str; 1] = [MATH_TEXT];

/// The name of the body slot of a presentation node: the content slot an
/// emitter treats as the node's content ([`is_body_slot`](super::is_body_slot)).
pub const BODY_SLOT: &str = "body";
/// The name of the content slot holding the label of a list entry; in a
/// [`LIST`] node it alternates with [`ITEM_SLOT`].
pub const LABEL_SLOT: &str = "label";
/// The name of the content slot holding the item of a list entry; in a
/// [`LIST`] node it follows each [`LABEL_SLOT`].
pub const ITEM_SLOT: &str = "item";
/// The name of the content slot holding one cell of a table of cells: a
/// [`CELLS`] node holds one per entry of its payload, in the same order, each
/// holding that cell's content as a run of blocks.
pub const CELL_SLOT: &str = "cell";
/// The name of the optional content slot of a heading ([`BLOCK_HEADING`],
/// [`RUNIN_HEADING`]) holding its alternative title, the lowered value of the
/// heading's `alt=` option, beside the [`BODY_SLOT`] holding the title. A
/// heading written without `alt=` has no such slot. A backend that sets a
/// running head or a short title reads it; the HTML and text backends read the
/// body alone.
pub const ALT_SLOT: &str = "alt";

/// The kind token of a [`LIST`] whose entries are numbered items: `numbered`.
///
/// Three **structural kind tokens** say what shape of list a [`LIST`] node
/// is — this one, [`LIST_UNNUMBERED`] and [`LIST_TERMS`] — so that a backend
/// can choose how to write it without knowing the name of the environment
/// that produced it: the HTML backend writes `<ol>`, `<ul>` or `<dl>` by
/// them. A list carries at most one of them, before its other kind tokens.
/// They are structure, not styling: they say what the list is, and the
/// output format decides what that looks like. These three words and
/// [`LIST_LABELED`] are reserved ([`LIST_RESERVED_KINDS`]).
pub const LIST_NUMBERED: &str = "numbered";
/// The kind token of a [`LIST`] whose entries are items that are not
/// numbered, each with the same label or one of its own: `unnumbered`. One of
/// the three structural kind tokens ([`LIST_NUMBERED`]).
pub const LIST_UNNUMBERED: &str = "unnumbered";
/// The kind token of a [`LIST`] whose entries are terms, each with its
/// description — the label is the term: `terms`. One of the three structural
/// kind tokens ([`LIST_NUMBERED`]).
pub const LIST_TERMS: &str = "terms";
/// The kind token of a [`LIST`] every entry of which carries its label as the
/// content of its `label` slot — a number, a bullet, a term — so that a
/// backend draws no marker of its own beside an entry: `labeled`. An entry
/// whose label slot is empty then shows no label at all.
///
/// Only the feature that produced the list knows whether every entry carries
/// its own label, so a token on the list states it; a backend does not decide
/// it.
pub const LIST_LABELED: &str = "labeled";
/// The kind tokens of a [`LIST`] this vocabulary gives a meaning to:
/// [`LIST_NUMBERED`], [`LIST_UNNUMBERED`], [`LIST_TERMS`] and
/// [`LIST_LABELED`] — `numbered`, `unnumbered`, `terms`, `labeled`.
///
/// These four words are **reserved**: a backend or a stylesheet that finds
/// one of them among a list's kind tokens takes it in the meaning this
/// vocabulary gives it. A feature that stages a list therefore uses none of
/// them for anything else, in particular not as the name of the list
/// environment the list comes from, which is staged as a kind token too and
/// which a stylesheet selecting `terms` would otherwise match. The `lists`
/// feature refuses to build a registry whose configuration gives a list
/// environment one of these names. Any other feature that stages lists must
/// keep them apart itself: a list is staged while a document is lowered, and
/// no check made when the registry is built applies to it.
pub const LIST_RESERVED_KINDS: [&str; 4] =
    [LIST_NUMBERED, LIST_UNNUMBERED, LIST_TERMS, LIST_LABELED];

/// Whether `name` is one of the presentation elements of this vocabulary.
///
/// This is a test on a name alone. To test whether a node of a tree is a
/// presentation node, use [`is_presentation`](super::is_presentation), which
/// reads the node's form rather than its element's name.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::pres::{PAR, is_presentation_name};
///
/// assert!(is_presentation_name(PAR));
/// assert!(!is_presentation_name("flm.paragraph"));
/// ```
pub fn is_presentation_name(name: &str) -> bool {
    ALL.contains(&name)
}
