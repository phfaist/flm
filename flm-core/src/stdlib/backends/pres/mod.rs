//! The presentation vocabulary: the elements every lowering rule stages and
//! every backend's emitter reads.
//!
//! A **presentation element** is an element that never occurs in a source
//! document. A [lowering rule](crate::core::lowering::LoweringRule) stages
//! (builds into the lowered tree) nodes of presentation elements while it
//! rewrites a frozen tree, a backend writes these elements natively, and that
//! backend's emitter turns them into output. The presentation elements of
//! this module form the vocabulary every backend shares, so a lowering rule
//! that stages only these elements works for every backend. See [backends, lowering,
//! and outputs](crate::guide::concepts_overview#backends-lowering-and-outputs).
//!
//! A **presentation node** is a node of a presentation element: a callable
//! node of the presentation form
//! ([`FlmCallableType::Presentation`](crate::lang::FlmCallableType)).
//! Everything such a node states beyond its element and its children is one
//! **typed payload** held in the node extension
//! ([`FlmNodeExt::payload`](crate::lang::FlmNodeExt::payload)): a span's
//! display style, a block's role and kinds, a link's target, the anchor of a
//! node that is a link target. A node has no open set of properties and no
//! handler per backend. An emitter that cannot express a field of a payload
//! ignores it, so an output format may lose a detail of presentation; it
//! never loses content.
//!
//! A presentation node's children are grouped into named **content slots**
//! (the `body`, a list's `label` and `item` pairs, the `cell` slots of a
//! table of cells),
//! followed by one **attached slot** per decoration the lowering driver moved
//! out of the node's content. A decoration is a node whose element carries
//! the [`Decoration`] aspect; see [decorations, roles and
//! kinds](crate::guide::concepts_overview#decorations-roles-and-kinds) and
//! [`read`].
//!
//! The vocabulary is `par`, `block`, `block-heading`, `runin-heading`, `span`,
//! `list`, `cells`, `lines`/`line`, `verbatim`, `math-passthrough`, `math-text`,
//! `link`, `image`, `unsupported`, `dropped`, `unknown`, `deco.runin`,
//! `deco.final`, `deco.margin`, `deco-mark` and `subtree`, each spelled
//! `flm.pres.<name>` ([`ALL`]). It has no catch-all element: an extension that
//! needs another presentation element registers it, with its own payload
//! type, through
//! [`FeatureBuilder::presentation_element`](crate::core::registry::FeatureBuilder::presentation_element).
//!
//! # Module map
//!
//! - [`names`] — the element names, the slot names, the kind tokens of a
//!   list, and the three groups of elements to which registration attaches
//!   an aspect ([`DECORATIONS`], [`OPAQUE`], [`CONTENT_IN_OPAQUE`]).
//! - [`payload`] — the payload types, their projections (the serialized form
//!   a test or a dump reads) and the readers that rebuild them
//!   ([`payload_readers`]).
//! - [`length`] — the [`Length`] value, an exact amount and a unit: how a
//!   payload states a dimension.
//! - [`aspects`] — the [`Decoration`], [`OpaqueContent`] and
//!   [`ContentInOpaque`] aspects.
//! - [`mod@register`] — the registration of the whole vocabulary.
//! - [`read`] — reading presentation nodes: payloads, slots, decorations, list
//!   entries, a table's cells, plain text, byte-exact source text, heading
//!   depth, and the two tests a backend makes before it writes a stop after a
//!   run-in heading.

pub mod aspects;
pub mod length;
pub mod names;
pub mod payload;
pub mod read;
pub mod register;

pub use aspects::{
    ContentInOpaque, Decoration, IsContentInOpaque, IsDecoration, IsOpaque, OpaqueContent,
};
pub use length::{I48F16, Length, LengthConversionError, LengthError, LengthUnit};
pub use names::{
    ALL, ALT_SLOT, BLOCK, BLOCK_HEADING, BODY_SLOT, CELL_SLOT, CELLS, CONTENT_IN_OPAQUE, DROPPED, DECO_FINAL, DECO_MARGIN, DECO_MARK,
    DECO_RUNIN, DECORATIONS, IMAGE, ITEM_SLOT, LABEL_SLOT, LINE, LINES, LINK, LIST, LIST_LABELED,
    LIST_NUMBERED, LIST_RESERVED_KINDS, LIST_TERMS, LIST_UNNUMBERED, MATH_PASSTHROUGH, MATH_TEXT,
    OPAQUE, PAR, RUNIN_HEADING, SPAN, SUBTREE, UNKNOWN, UNSUPPORTED, VERBATIM,
    is_presentation_name,
};
pub use payload::{
    BlockPayload, CellAlign, CellEdge, CellHeader, CellSpec, CellsPayload, DecoMarkPayload,
    DecorationPayload, DocRole, DroppedPayload, GridPosition, HeadingPayload, ImageDisplay,
    MAX_CELLS_POSITIONS,
    ImagePayload,
    LineAlign, LinePayload, LinesPayload, LinkPayload, ListPayload, MathDisplay, MathPassthroughPayload,
    MathTextPayload, ParPayload, SpanPayload, SpanStyle, SrcsetCandidate, SubtreePayload,
    UnknownKind, UnknownPayload, UnsupportedPayload, VerbatimDisplay, VerbatimKind, VerbatimPayload,
    payload_readers,
};
pub use read::{
    AttachedSlot, DEFAULT_ROOT_HEADING_LEVEL, DEFAULT_RUN_IN_STOP, HeadingAboveRoot, RUN_IN_STOPS,
    attached_slots, body_of, cells_of, decoration_at, heading_depth,
    ends_in_sentence_punctuation, holds_text, is_body_slot, is_presentation, list_entries,
    payload_of, plain_text, recompose_source, slot_count, slot_of, spelling_identity,
};
pub use register::register;
