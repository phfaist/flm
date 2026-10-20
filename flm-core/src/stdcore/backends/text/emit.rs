//! The text emitter: it walks the text backend's lowered trees and lowered
//! pieces, builds one layout document from them, and prints it into a
//! [`TextOutput`].
//!
//! An emitter is a backend's emit stage: it walks the lowered trees and writes
//! the output's bytes into a table, from which the backend's final stage makes
//! the artifact set (see [backends, lowering, and
//! outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
//! [`emit()`] is this module's entry point; the emit stage
//! ([`TextEmitStage`](super::stages::TextEmitStage)) calls it once per run.
//! The [layout engine](crate::layout) breaks the lines, indents and pads; this
//! module maps each presentation element onto the engine's constructors and
//! lets the engine produce the characters.
//!
//! # What each element prints
//!
//! In a **run of blocks** (a tree's top level, a block's body, a list item),
//! blocks are separated by one blank line:
//!
//! | lowered element | printed as |
//! |---|---|
//! | `par` | its inline content, filled word by word; decorations printed at their decoration marks |
//! | `block-heading` | the heading text, then a rule under it (see below) |
//! | `block` | its body as a run of blocks; the role and the kinds are dropped; a decoration whose decoration mark is among its blocks prints at that decoration mark, on a line of its own when a block follows |
//! | `list` | one entry per label/item pair, the item hanging after the label (see below) |
//! | `cells` | one table of the layout engine: each cell's blocks in its column, in the cell's own alignment, a spanned cell's content at the first position of its span; the leading rows of column headers above a rule (see below) |
//! | `lines` / `line` | one printed line per `line`, indented by its `indent_left` converted to columns ([`TextRenderOptions::columns`]) |
//! | `verbatim` (block) | the content byte for byte, indented like its surroundings |
//! | `math-passthrough` (display) | the source as a block indented by two columns (see below) |
//! | `unsupported` | `[unsupported: <element>]` |
//! | `dropped` | `[dropped: <detail>]` |
//! | `unknown` (environment) | its begin and end lines around its body blocks (see below) |
//! | `image` (block) | `[image: <alt or src>]` |
//! | `subtree` | the placed tree's lowered blocks, printed at this node |
//! | `flm.container`, `flm.group` | their children: as a run of blocks when a block-shaped node is among their descendants, otherwise as inline content filled with the text around them |
//! | `flm.comment`, `flm.parbreak` | nothing; the blocks on either side of them are separate |
//!
//! The rows that need more room:
//!
//! - An unattributed environment (an environment no feature declares) prints
//!   `\begin{name}` on a line of its own, then its lowered body as a run of
//!   blocks, then `\end{name}` on a line of its own.
//! - A `block-heading` prints the heading text, then a rule as wide as that
//!   text: `=` for a heading at the root heading level
//!   ([`TextRenderOptions::root_heading_level`], 1 by default), `-` for a
//!   heading one level deeper. A heading deeper still has no rule; it is
//!   preceded by `#` characters and a space: three `#` two levels below the
//!   root, one more for each further level. A heading above the root level is
//!   reported as `flm.pres.heading-above-root`
//!   ([`HeadingAboveRoot`](crate::stdcore::backends::pres::HeadingAboveRoot))
//!   and printed like one at the root level.
//! - A `list` prints, for each entry, the label's text (a superscript or
//!   subscript span in it written as in running text, below), padded to the
//!   width of the widest label plus one, and the item's blocks hanging at that
//!   column; when every label of the list is empty, the items start at the
//!   margin. An entry with an empty item prints its label alone. Entries are
//!   one blank line apart.
//! - A display `math-passthrough` prints the source inside `\[…\]` (or
//!   `\begin{env}…\end{env}` for an environment), as a block indented by two
//!   columns.
//! - A `cells` table is laid out as one table of the layout engine
//!   ([`DocBuf::table`]): one column per column of the grid, the column widths
//!   computed from their cells' content, columns two spaces apart, each cell's
//!   blocks printed at its column's width and padded in the cell's own
//!   alignment (left where it has none). The positions covered by a span,
//!   other than the span's first, print nothing. The leading rows whose cells
//!   all head their columns are the header rows, with a rule of `-` under them.
//!   Row headers, rules at the edges of cells and custom cell styles are not
//!   shown in this output.
//!
//! In **inline content**, text is split into words at breakable whitespace
//! ([`is_breakable_space`]: every whitespace character except the
//! non-breaking spaces); each word is an atom of the layout engine, each
//! space between two words a place where the line may break, and nothing is
//! escaped:
//!
//! | lowered element | printed as |
//! |---|---|
//! | `span` | its content; a superscript or subscript style converts its text (see below); every other style, the role, the document role and the kinds are dropped |
//! | `runin-heading` | its content; the payload is dropped |
//! | `link` | its content, then ` <address>` for a link to an address outside the document, as the render options say (below) |
//! | `math-passthrough` (inline) | one atom, `\(source\)`, each run of whitespace replaced by one space |
//! | `verbatim` (inline) | one atom; content holding a newline or a tab is printed as a block instead |
//! | `deco-mark` | the decoration, printed at its decoration mark (see below) |
//! | `unknown` (macro) | its source spelling, `\name`, as one atom |
//! | `image` (inline), `unsupported`, `dropped` | as in a run of blocks, as one atom |
//! | a `flm.parbreak` | a space |
//! | any block-shaped node inside a paragraph | on lines of its own |
//!
//! ## Decorations
//!
//! A decoration is a node whose element carries the
//! [`Decoration`](crate::stdcore::backends::pres::Decoration) aspect (a run-in
//! heading, a margin note, a closing mark); the lowering driver moves it out of
//! the content it was written in into an attached slot of the enclosing
//! presentation node, and leaves a decoration mark (a `flm.pres.deco-mark`
//! node) where it was (see [decorations, roles and
//! kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)). The
//! emitter prints each decoration at its decoration mark, as one atom made of
//! its text:
//!
//! - a run-in decoration (`flm.pres.deco-runin`) as its text followed by the
//!   stop [`TextRenderOptions::run_in_stop`] (a period by default), then a
//!   breakable space. The stop is added only where the decoration's content
//!   does not already end in sentence punctuation (`.`, `,`, `:`, `;`, `?` or
//!   `!`, as [`ends_in_sentence_punctuation`] tests): a feature that wants
//!   another stop, for instance a colon after a defined term, writes it in the
//!   content, and the emitter adds nothing;
//! - a closing decoration (`flm.pres.deco-final`) after a non-breaking space
//!   (U+00A0), so that it never starts a line by itself;
//! - a margin decoration (`flm.pres.deco-margin`) as `[text]`, between
//!   breakable spaces.
//!
//! A decoration whose content holds no text prints nothing. A decoration that
//! was not moved (because the content it is written in belongs to no
//! presentation node, for instance at the top level of a fragment laid out as
//! inline content) prints the same way where it is written.
//!
//! ## Links
//!
//! Under the default render options, a `link` to an address outside the
//! document prints the address after its content, as ` <address>`; it prints
//! its content alone when [`TextRenderOptions::link_addresses`] is off. A link
//! whose content already shows its address (the payload's
//! [`shows_target`](crate::stdcore::backends::pres::LinkPayload::shows_target),
//! set by `\url` and `\email`) prints its content alone unless
//! [`TextRenderOptions::link_addresses_when_shown`] is on as well. A link to a
//! place inside the document (an address beginning with `#`) prints its
//! content alone. A link with no content prints `<address>` where the address
//! would be printed, and nothing otherwise.
//!
//! # Position marks: where a printed byte came from
//!
//! The emitter wraps every presentation node it prints in a mark of the layout
//! engine, and records which lowered node each mark stands for: a
//! [`LoweredAddr`], which gives the document tree the node was lowered from
//! and the node's position in that tree's lowered form. [`TextOutput::marks`]
//! holds the result ([`TextMark`]): a range of printed bytes leads to a
//! lowered node, which [`LoweredAddr::resolve`] finds in the lowered-trees
//! table; that node's span and provenance lead to the source the text was
//! written from. A position mark never refers to a node of a [frozen
//! tree](crate::guide::concepts_overview#frozen-trees-and-computed-data),
//! because a lowered node may stand for no single node of the document. The bytes
//! printed for a piece carry no mark.
//!
//! # Further rules of the mapping
//!
//! - **A group inside verbatim content prints without its braces.** A parse
//!   never produces a group inside a [verbatim
//!   region](crate::guide::concepts_overview#verbatim-regions), since `{` is an
//!   ordinary character there. A content group found inside one is therefore
//!   the wrapper of an argument that a user definition's template
//!   ([`template`](crate::lang::rewrites::userdefs::template)) substituted
//!   there, and its
//!   braces belong to the syntax of the call, not to the content: its children
//!   are printed and its braces are not. Every other node inside verbatim
//!   content is printed as its source spelling, braces included, since a
//!   macro's braces are part of what the author wrote.
//! - **Mathematics prints as its source, with delimiters**: `\(…\)` for inline
//!   mathematics, `\[…\]` for display mathematics, `\begin{env}…\end{env}` for
//!   a math environment. The delimiters the author wrote are not part of the
//!   payload, so every way of writing inline mathematics prints as `\(…\)`.
//! - **A superscript span prints its text raised, a subscript span lowered**:
//!   each character of the span's text is written in its Unicode superscript or
//!   subscript form ([`superscript_form`], [`subscript_form`]), and a character
//!   that has none is written unchanged, so `10` is `¹⁰`, `a` is `ᵃ`, and `q`
//!   stays `q`. No marker such as `^` is added, since the output is read by a
//!   person. The conversion applies to the text of everything nested inside the
//!   span (a link, another span), and a nested span of the other of the two
//!   styles converts its own text its own way. What is not text of the document
//!   is written unchanged: the address printed after a link, mathematics and
//!   verbatim content (printed as their source), a decoration moved out of the
//!   span, a stand-in such as `[dropped: …]`.
//! - **What plain text cannot express is dropped without a diagnostic**: a
//!   span's style other than superscript and subscript; the document role of a
//!   block, a span or a link; a link's accessible name; the role and kinds of a
//!   block or a span; a list's kinds; every anchor; an image's width and
//!   alternative sources; a line's right indentation and its alignment; a
//!   verbatim node's kind and language; a link's kind and a target inside the
//!   document. Presentation is dropped that way; content never is.
//! - **A lowered tree need not be structured into blocks.** Consecutive inline
//!   nodes in a run of blocks are filled together as one paragraph, split at
//!   block-shaped nodes and at paragraph breaks, so a tree that went through no
//!   block-structure pass still prints.
//! - **The walk is bounded** by [`DEPTH_LIMIT`]. A tree that nests deeper,
//!   counting the trees it places, prints nothing and is reported as
//!   `flm.text.nesting-too-deep` ([`NestingTooDeep`]).
//!
//! # Diagnostics
//!
//! The emitter reports, at error severity: [`MissingSubtree`] and
//! [`SubtreeCycle`] for a tree placed inside another that cannot be printed;
//! [`PiecePlacement`] for a piece that places a tree; [`NestingTooDeep`] for a
//! tree nested beyond [`DEPTH_LIMIT`]; [`NoWorkerThread`] when it cannot run at
//! all; and the presentation condition `flm.pres.heading-above-root`
//! ([`HeadingAboveRoot`](crate::stdcore::backends::pres::HeadingAboveRoot))
//! for a heading above the root heading level. A [condition](crate::guide::concepts_overview#diagnostics) is
//! the typed body of a diagnostic; each condition's documentation states what
//! the emitter prints instead.

use std::borrow::Cow;
use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use techy::core::node::{NodeKind, NodeRef, NodeTree, SlotRole};
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::serialize::SerialValue;

use crate::model::addr::TreeId;
use crate::model::element::element_of;
use crate::core::data::table::{PieceClass, ProjectCx, SingletonTable, Table, TableName};
use crate::lang::guard;
use crate::lang::structural;
use crate::lang::{nowhere, Flm, FlmDiagnostics, FlmGroupType, FlmOrigin};
use crate::layout::{
    ColAlign, ColSpec, DocBuf, DocId, LineCol, MarkId, Profile, RawPolicy, TableCell,
    display_width, is_breakable_space, print_full,
};
use crate::core::piece::PiecePlace;
use crate::model::names::Name;
use crate::stdcore::backends::lowering::{LoweredAddr, LoweredTrees};
use crate::stdcore::backends::pieces::{LoweredPiece, LoweredPieces};
use crate::stdcore::backends::pres::{
    self, CellAlign, CellHeader, CellsPayload, DroppedPayload, GridPosition, HeadingPayload, ImageDisplay, ImagePayload, LinePayload, LinkPayload, MathDisplay,
    MathPassthroughPayload, MathTextPayload, SpanPayload, SpanStyle, SubtreePayload, UnknownKind,
    UnknownPayload, UnsupportedPayload, VerbatimDisplay, VerbatimPayload, cells_of, decoration_at,
    ends_in_sentence_punctuation, heading_depth, holds_text, is_presentation, list_entries,
    payload_of, plain_text, recompose_source,
};

use super::stages::TextRenderOptions;
use super::superscript::{subscript_form, superscript_form};

// --- the output table --------------------------------------------------------------------

/// A position mark of the text output: where the text printed for one lowered
/// node is in [`TextOutput::text`].
///
/// It holds the layout engine's mark identifier, the lowered node it stands
/// for, and the byte range and the line and column range of that node's
/// printed content characters. [`TextOutput::marks`] holds one per printed
/// presentation node, and [`TextOutput::slice`] returns the text it covers.
///
/// Two runs over the same document produce equal marks, because a lowered node
/// is addressed by its position rather than by an identifier assigned during
/// the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextMark {
    /// The layout engine's identifier of the mark. The emitter assigns
    /// identifiers in the order it builds the marked parts, so the mark of an
    /// enclosing node has a larger identifier than the marks inside it; the
    /// order of [`TextOutput::marks`] is the order in which the printer opens
    /// the marks.
    pub id: MarkId,
    /// The lowered node this mark stands for. Find the node in the backend's
    /// lowered-trees table with
    /// [`LoweredAddr::resolve`](crate::stdcore::backends::lowering::LoweredAddr::resolve);
    /// the address refers to no node of a [frozen
    /// tree](crate::guide::concepts_overview#frozen-trees-and-computed-data).
    pub addr: LoweredAddr,
    /// The byte range of [`TextOutput::text`] that holds the node's printed
    /// content characters.
    pub bytes: Range<usize>,
    /// The line and column of the first content character printed for the
    /// node, counted from 0.
    pub start: LineCol,
    /// The line and column just after the last content character printed for
    /// the node.
    pub end: LineCol,
}

/// The value of the text backend's output table `flm.output.text`
/// ([`TEXT_OUTPUT`]): the printed document and its position marks.
///
/// The emit stage ([`TextEmitStage`](super::stages::TextEmitStage)) writes it
/// once per run, since the printed text is one document; the regions stage
/// and the final stage then publish its text as the `text` output's artifact
/// set (`flm.output.text.document`). A caller that wants the characters and
/// where each part of the document was printed reads this table; a caller
/// that wants the file reads the artifact set.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextOutput {
    /// The printed text: empty when nothing is printed, otherwise ending in
    /// exactly one newline.
    pub text: String,
    /// The position marks, in the order the printer reaches their nodes in
    /// the printed text (a node's mark before the marks of its descendants).
    pub marks: Vec<TextMark>,
}

impl TextOutput {
    /// Returns the printed text that `mark` covers.
    ///
    /// # Panics
    ///
    /// Panics if the byte range of `mark` lies outside this output's text or
    /// does not start and end on character boundaries. A mark taken from this
    /// output's own [`marks`](Self::marks), with the output unchanged since
    /// printing, never panics; a mark from another output, a mark built or
    /// changed by hand, or an output whose `text` was changed after printing
    /// can.
    pub fn slice(&self, mark: &TextMark) -> &str {
        &self.text[mark.bytes.clone()]
    }

    /// Returns the marks that stand for the lowered node at `addr`, in the order
    /// of [`marks`](Self::marks). A node printed several times (for instance in a
    /// tree placed twice) has several marks; a node for which nothing was
    /// printed has none.
    pub fn marks_at(&self, addr: LoweredAddr) -> impl Iterator<Item = &TextMark> + '_ {
        self.marks.iter().filter(move |mark| mark.addr == addr)
    }
}

impl Table for TextOutput {
    /// The table is written once per run, so it receives one table piece (the
    /// part of a table one stage invocation produced); a non-empty table piece
    /// replaces the value held so far.
    fn absorb(&mut self, piece: TextOutput) {
        if !piece.text.is_empty() || !piece.marks.is_empty() {
            *self = piece;
        }
    }

    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        let position = |at: LineCol| SerialValue::Str(format!("{}:{}", at.line, at.col));
        SerialValue::Map(vec![
            (String::from("text"), SerialValue::Str(self.text.clone())),
            (
                String::from("marks"),
                SerialValue::List(
                    self.marks
                        .iter()
                        .map(|mark| {
                            SerialValue::Map(vec![
                                (String::from("addr"), SerialValue::Str(mark.addr.to_string())),
                                (
                                    String::from("bytes"),
                                    SerialValue::List(vec![
                                        SerialValue::Int(mark.bytes.start as i64),
                                        SerialValue::Int(mark.bytes.end as i64),
                                    ]),
                                ),
                                (String::from("start"), position(mark.start)),
                                (String::from("end"), position(mark.end)),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
    }

    fn piece_class(&self) -> PieceClass {
        if self.text.is_empty() && self.marks.is_empty() {
            PieceClass::Empty
        } else {
            PieceClass::DocumentFact
        }
    }
}

impl SingletonTable for TextOutput {
    const NAME: &'static str = "flm.output.text";
}

/// The typed name of the text output table, `flm.output.text`, which holds a
/// [`TextOutput`].
pub const TEXT_OUTPUT: TableName<TextOutput> = TableName::new("flm.output.text");

// --- conditions ---------------------------------------------------------------------------

/// Condition `flm.text.missing-subtree`: a `flm.pres.subtree` node places a
/// tree that has no lowered form for the text backend, because its lowering
/// failed or because the document has no tree with that identifier.
///
/// A `flm.pres.subtree` node stands for a tree placed inside another one (a
/// fragment placed inside another fragment); the emitter prints the placed
/// tree at that node. Reported at error severity, at the node. Recovery:
/// nothing is printed for the node.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.text.missing-subtree",
    message = "the document tree `{tree}` is placed inside another tree, but it has no text to \
               print there: no tree has that identifier, or that tree could not be converted to \
               text"
)]
pub struct MissingSubtree {
    /// The placed tree's identifier.
    pub tree: String,
}

/// Condition `flm.text.subtree-cycle`: a `flm.pres.subtree` node places a
/// tree that is being printed already, because it contains this node,
/// directly or through other placed trees (a cycle of placements).
///
/// Reported at error severity, at the node. Recovery: nothing is printed for
/// the node; every tree of the cycle is still printed once.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.text.subtree-cycle",
    message = "the document tree `{tree}` must not be placed inside itself, but it is, \
               directly or through other trees"
)]
pub struct SubtreeCycle {
    /// The placed tree's identifier.
    pub tree: String,
}

/// Condition `flm.text.piece-placement`: a `flm.pres.subtree` node is inside
/// a piece.
///
/// A piece ([`Piece`](crate::core::piece::Piece)) is a document-level value
/// that a backend places before or after the document's content (the title,
/// the list of footnotes); it is never placed inside a tree, and it must not
/// place a tree inside itself. Reported at error severity, at the node.
/// Recovery: nothing is printed for the node.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.text.piece-placement",
    message = "the document tree `{tree}` cannot be placed inside `{piece}`, which the text \
               output places beside the document's content"
)]
pub struct PiecePlacement {
    /// The piece's name.
    pub piece: String,
    /// The identifier of the tree the node places.
    pub tree: String,
}

/// Condition `flm.text.worker-thread-unavailable`: the emitter could not run
/// on a worker thread, so it was not run.
///
/// A worker thread is a thread created with at least
/// [`WORKER_STACK_SIZE`](crate::lang::guard::WORKER_STACK_SIZE) bytes of stack
/// and marked as such ([`mark_worker_thread`](crate::lang::guard::mark_worker_thread));
/// [`on_worker_thread`](crate::lang::guard::on_worker_thread) creates one when
/// the calling thread is not one (see [the recursion guard and worker
/// threads](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads)). The emitter
/// needs one because its walk recurses as deeply as the trees nest. The
/// calling thread was not a worker thread, and the operating system refused to
/// create one. Reported at error severity, with no position in the document.
/// Recovery: the run has no text output.
// The refusal is the operating system's and cannot be provoked without
// injecting a failure, so no test covers this arm; the spawning half of the
// contract is what the emitter tests exercise. See the design rationale at
// [§flm-dd-dr:lang-worker-stack].
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.text.worker-thread-unavailable",
    message = "printing the text output needs a thread with a large stack, and the operating \
               system refused to create one: {detail}"
)]
pub struct NoWorkerThread {
    /// The operating system's error message.
    pub detail: String,
}

/// The emitter's estimate of the stack one level of nesting of its walk uses:
/// an upper bound on two frames of the inline or of the block recursion,
/// measured under 4 KiB in a debug build (a thousand nested spans print on a
/// 4 MiB stack and overflow a 2 MiB one). It is three times the measurement,
/// so that the limit holds in every build profile.
const LEVEL_STACK_ESTIMATE: usize = 12 * 1024;

/// The nesting depth, 4096 levels, past which the emitter does not print a
/// tree and reports `flm.text.nesting-too-deep` ([`NestingTooDeep`]).
///
/// It is the recursion guard's stack budget
/// ([`STACK_BUDGET`](crate::lang::guard::STACK_BUDGET)) divided by the
/// emitter's estimate of the stack one level uses. A tree that came from a
/// parsed document never comes near it, since the parse refuses far shallower
/// nesting. A lowered tree can, when its lowering rules stage (build into the
/// lowered tree) levels of their own, or when placed trees form a long chain;
/// the levels of the placed trees count toward the depth of the tree that
/// places them.
pub const DEPTH_LIMIT: usize = guard::STACK_BUDGET / LEVEL_STACK_ESTIMATE;

/// Condition `flm.text.nesting-too-deep`: a lowered tree, counting the trees
/// it places, nests deeper than the emitter's depth limit ([`DEPTH_LIMIT`]).
///
/// Every recursive walk of a tree in this library is bounded in this way, and
/// each reports its own condition, so a diagnostic tells which step reached
/// its limit. Reported at error severity, once, at the root of the top-level
/// tree (or piece). Recovery: that tree prints nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.text.nesting-too-deep",
    message = "the content nests too deeply to be printed as text: {detail}"
)]
pub struct NestingTooDeep {
    /// The limit that was exceeded, as text.
    pub detail: String,
}

/// The label `text backend` that a diagnostic shows in place of a position,
/// for a condition this backend reports that has no position in the document:
/// the label of the empty source that [`nowhere`] spans.
pub(crate) const NOWHERE_LABEL: &str = "text backend";

// --- the entry point ----------------------------------------------------------------------

/// The result of [`emit()`]: the output, and the diagnostics reported.
///
/// When the emitter cannot print at all, [`output`](Self::output) is `None`
/// and a diagnostic says why; [`emit()`] never returns an error.
#[derive(Debug)]
pub struct Emitted {
    /// The printed text and its position marks; `None` when the emitter could
    /// not run.
    pub output: Option<TextOutput>,
    /// The diagnostics, in the order they were reported.
    pub diags: FlmDiagnostics,
}

/// Prints the lowered trees of `trees` listed in `order`, in that order, with
/// the lowered `pieces` placed before and after them, under the render options
/// `options`, and returns the printed text with its position marks and the
/// diagnostics.
///
/// The pieces placed before the content come first, in their order, then the
/// trees, then the pieces placed after the content; any two of them are
/// separated by one blank line. Position marks are recorded for the bytes
/// printed for the trees, not for a piece's bytes. A `flm.pres.subtree` node
/// inside a piece prints nothing and is reported as [`PiecePlacement`].
///
/// A tree placed by a `flm.pres.subtree` node of another tree is printed at
/// that node, and is not printed again at the top level even when `order`
/// lists it. Trees that place each other in a cycle are printed after the
/// others, so that every tree of `order` is printed once; the cycle is
/// reported ([`SubtreeCycle`]). A placed tree that has no lowered form is
/// reported ([`MissingSubtree`]) and prints nothing. A tree of `order` that
/// has no lowered form in `trees` is left out without a diagnostic; when its
/// lowering failed, the lowering stage reported the failure. The module
/// documentation
/// lists what each element prints.
///
/// The walk recurses as deeply as the trees nest, so it must run on a worker
/// thread, a thread with a large stack
/// ([`on_worker_thread`](crate::lang::guard::on_worker_thread)): `emit` runs it
/// on the calling thread when that thread is a worker thread, and on a new
/// worker thread otherwise. A problem in the trees never makes `emit` fail or
/// panic: a tree that cannot be printed is left out and reported, and when the
/// emitter cannot run at all, [`Emitted::output`] is `None` beside an Error
/// diagnostic ([`NoWorkerThread`]).
pub fn emit(
    trees: &LoweredTrees,
    order: &[TreeId],
    pieces: &LoweredPieces,
    options: &TextRenderOptions,
) -> Emitted {
    match guard::on_worker_thread(|| emit_on_worker_thread(trees, order, pieces, options)) {
        Ok(emitted) => emitted,
        Err(error) => {
            let mut diags = FlmDiagnostics::new();
            diags.push(Diagnostic::error(
                NoWorkerThread { detail: error.to_string() },
                nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
            ));
            Emitted { output: None, diags }
        }
    }
}

/// The emission itself, once the caller is known to be on a worker thread: the
/// pieces placed before the content, every tree of `order` that no other tree
/// places, in order, then the trees a placement cycle left unprinted, then the
/// pieces placed after the content, one blank line apart, printed at the
/// width of `options`.
fn emit_on_worker_thread(
    trees: &LoweredTrees,
    order: &[TreeId],
    pieces: &LoweredPieces,
    options: &TextRenderOptions,
) -> Emitted {
    let mut emitter = Emitter {
        buf: DocBuf::new(),
        trees,
        run_in_stop: &options.run_in_stop,
        link_addresses: options.link_addresses,
        link_addresses_when_shown: options.link_addresses_when_shown,
        options,
        tree: order.first().copied().unwrap_or(TreeId::tree(0)),
        placing: Vec::new(),
        printed: BTreeSet::new(),
        block_containers: Vec::new(),
        depth: 0,
        refused: false,
        piece: None,
        script: None,
        marks: Vec::new(),
        diags: FlmDiagnostics::new(),
    };
    let placed = emitter.placed_trees();
    let mut parts: Vec<DocId> = Vec::new();
    for piece in pieces.at(PiecePlace::BeforeContent) {
        emitter.piece_part(piece, &mut parts);
    }
    // First the trees no other tree places, in order — each splices what it
    // places; then whatever is still unprinted (trees placing each other in
    // a cycle), so that every tree of the order prints once and a cycle is
    // diagnosed rather than swallowed.
    for pass in 0..2 {
        for id in order {
            if emitter.printed.contains(id) || (pass == 0 && placed.contains(id)) {
                continue;
            }
            let Some(lowered) = trees.get(*id) else { continue };
            let tree = Arc::clone(&lowered.tree);
            if let Some(doc) = emitter.tree_doc(*id, &tree) {
                if !parts.is_empty() {
                    let separator = emitter.buf.blocksep(1);
                    parts.push(separator);
                }
                parts.push(doc);
            }
        }
    }
    for piece in pieces.at(PiecePlace::AfterContent) {
        emitter.piece_part(piece, &mut parts);
    }
    let root = emitter.buf.concat(&parts);
    let Emitter { buf, marks, diags, .. } = emitter;
    let printed = print_full(&buf, root, &Profile::new(options.width));
    let output = TextOutput {
        text: printed.text,
        marks: printed
            .marks
            .into_iter()
            .filter_map(|span| {
                let addr = *marks.get(span.id.0 as usize)?;
                Some(TextMark {
                    id: span.id,
                    addr,
                    bytes: span.bytes,
                    start: span.start,
                    end: span.end,
                })
            })
            .collect(),
    };
    Emitted { output: Some(output), diags }
}

// --- the emitter ---------------------------------------------------------------------------

/// One token of an inline run: an atom (possibly itself filled prose), a
/// breakable space (a *gap*), a non-breaking space joining the atoms on either
/// side of it (a *tie*, which, like a gap, prints nothing at the start of a
/// run), or a block-shaped node that takes lines of its own.
enum Tok {
    Gap,
    Tie,
    Atom(DocId),
    Block(DocId),
}

/// The two ways a span's text is written through other forms of its
/// characters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Script {
    /// Through the superscript forms ([`superscript_form`]).
    Raised,
    /// Through the subscript forms ([`subscript_form`]).
    Lowered,
}

impl Script {
    /// The way a span of `style` writes its text, if it has one of its own.
    fn of(style: Option<SpanStyle>) -> Option<Script> {
        match style {
            Some(SpanStyle::Superscript) => Some(Script::Raised),
            Some(SpanStyle::Subscript) => Some(Script::Lowered),
            _ => None,
        }
    }

    /// `word` with each character that has a form written in it; a
    /// character with none stays as it is.
    fn write(self, word: &str) -> String {
        let form = match self {
            Script::Raised => superscript_form,
            Script::Lowered => subscript_form,
        };
        word.chars().map(|c| form(c).unwrap_or(c)).collect()
    }
}

/// The walk in progress: the layout document being built, where in the trees
/// the walk is, and what it has collected.
struct Emitter<'t> {
    buf: DocBuf,
    trees: &'t LoweredTrees,
    /// The stop written after a run-in heading's title that does not already
    /// end in sentence punctuation; the empty text writes none.
    run_in_stop: &'t str,
    /// Whether a link to an address prints the address after its content
    /// ([`TextRenderOptions::link_addresses`]).
    link_addresses: bool,
    /// Whether a link showing its own address's display prints the address
    /// too ([`TextRenderOptions::link_addresses_when_shown`]).
    link_addresses_when_shown: bool,
    /// The render options, read for the conversion of lengths to columns
    /// ([`TextRenderOptions::columns`]) and for the root heading level.
    options: &'t TextRenderOptions,
    /// The document tree whose lowered form is being walked.
    tree: TreeId,
    /// The trees being printed, outermost first (the placement cycle guard).
    placing: Vec<TreeId>,
    /// The trees printed so far, at the top level or spliced.
    printed: BTreeSet<TreeId>,
    /// Per tree being printed (innermost last): the indices of its
    /// non-presentation containers holding a block-shaped node.
    block_containers: Vec<BTreeSet<usize>>,
    /// The open levels of the walk.
    depth: usize,
    /// Whether the depth limit was hit while printing the current top-level
    /// tree.
    refused: bool,
    /// The piece being printed, if any: its bytes carry no position mark,
    /// and it places no tree.
    piece: Option<Name>,
    /// How the text being walked is written: raised or lowered inside a
    /// superscript or subscript span, unchanged (`None`) elsewhere.
    script: Option<Script>,
    /// The mark table: index = mark id.
    marks: Vec<LoweredAddr>,
    diags: FlmDiagnostics,
}

impl fmt::Debug for Emitter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Emitter")
            .field("tree", &self.tree)
            .field("marks", &self.marks.len())
            .finish()
    }
}

/// Whether the element of `node` has the name `name`.
fn is_element(node: &NodeRef<'_, Flm>, name: &str) -> bool {
    element_of(node).name == name
}

/// Whether a node takes lines of its own in an inline run.
///
/// An image is block-shaped where its payload says it is a block (the content
/// of a float, where two images are two blocks) and an atom of the text where
/// it is inline; verbatim content and mathematics likewise follow their
/// payload.
fn is_block_shaped(node: &NodeRef<'_, Flm>) -> bool {
    let name = element_of(node).name.as_str();
    match name {
        pres::PAR
        | pres::BLOCK
        | pres::BLOCK_HEADING
        | pres::LIST
        | pres::CELLS
        | pres::LINES
        | pres::SUBTREE => true,
        pres::IMAGE => payload_of::<ImagePayload>(node)
            .is_some_and(|payload| payload.display == ImageDisplay::Block),
        pres::VERBATIM => payload_of::<VerbatimPayload>(node)
            .is_some_and(|payload| payload.display == VerbatimDisplay::Block),
        pres::MATH_PASSTHROUGH => payload_of::<MathPassthroughPayload>(node)
            .is_some_and(|payload| payload.display == MathDisplay::Display),
        // An unattributed environment prints its begin and end lines around its
        // body; an unattributed macro is one inline atom.
        pres::UNKNOWN => payload_of::<UnknownPayload>(node)
            .is_some_and(|payload| payload.kind == UnknownKind::Environment),
        _ => false,
    }
}

/// The layout engine's alignment for a cell's alignment.
fn col_align(align: CellAlign) -> ColAlign {
    match align {
        CellAlign::Left => ColAlign::Left,
        CellAlign::Center => ColAlign::Center,
        CellAlign::Right => ColAlign::Right,
    }
}

/// How many leading rows of a table of cells are rows of column headers: a
/// row counts when at least one of its positions holds a cell, and every cell
/// at its positions (at the cell's top-left or covered by its span) carries
/// [`CellHeader::Column`]; an empty position counts neither way. The engine
/// takes at most `u8::MAX` header rows.
fn header_rows(payload: &CellsPayload, grid: &[Vec<GridPosition>]) -> u8 {
    let is_header = |row: &Vec<GridPosition>| {
        let mut held = row.iter().filter_map(|position| match *position {
            GridPosition::TopLeft(index) | GridPosition::Covered(index) => Some(index),
            GridPosition::Empty => None,
        });
        let first = held.next();
        first.is_some()
            && first
                .into_iter()
                .chain(held)
                .all(|index| payload.cells[index].header == Some(CellHeader::Column))
    };
    let count = grid.iter().take_while(|row| is_header(row)).count();
    u8::try_from(count).unwrap_or(u8::MAX)
}

/// Whether a node is inert in a run (nothing is printed for it).
fn is_inert(node: &NodeRef<'_, Flm>) -> bool {
    node.is_comment() || is_element(node, structural::PARBREAK)
}

/// The containers of a tree that are not presentation nodes (lists, groups)
/// and hold a block-shaped node somewhere below them; they are laid out as
/// runs of blocks. Computed in one pass in reverse storage order (a child's
/// index is always greater than its parent's), so the walk looks the
/// container up in a set instead of walking its subtree again.
fn block_containers(tree: &NodeTree<Flm>) -> BTreeSet<usize> {
    let nodes: Vec<NodeRef<'_, Flm>> = tree.iter_storage_order().collect();
    let mut set = BTreeSet::new();
    for node in nodes.iter().rev() {
        if is_presentation(node) || !matches!(node.kind(), NodeKind::List | NodeKind::Group(_)) {
            continue;
        }
        let holds = node
            .children()
            .iter()
            .any(|child| is_block_shaped(&child) || set.contains(&child.id().index()));
        if holds {
            set.insert(node.id().index());
        }
    }
    set
}

/// `text` with every line break and tab replaced by a space: the newline, the
/// carriage return, the tab, the vertical tab, the form feed, the next-line
/// character and the Unicode line and paragraph separators (an atom may hold
/// none of them).
fn one_line(text: &str) -> String {
    text.replace(
        ['\n', '\r', '\t', '\u{000B}', '\u{000C}', '\u{0085}', '\u{2028}', '\u{2029}'],
        " ",
    )
}

/// The text of a list label or of a decoration: its plain text
/// ([`plain_text`]), each character inside a superscript or subscript span
/// written in the forms of the innermost such span, as in running text. A
/// label and a decoration are printed as one text, not walked as a run.
fn label_text<'t>(nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>) -> String {
    let mut out = String::new();
    for node in nodes {
        for piece in std::iter::once(node).chain(node.descendants()) {
            let Some(text) = piece.chars() else { continue };
            let script = std::iter::successors(Some(piece), |at| {
                (at.id() != node.id()).then(|| at.parent()).flatten()
            })
            .filter(|at| is_element(at, pres::SPAN))
            .find_map(|at| Script::of(payload_of::<SpanPayload>(&at).and_then(|span| span.style)));
            match script {
                Some(script) => out.push_str(&script.write(text)),
                None => out.push_str(text),
            }
        }
    }
    out
}

/// The whitespace-normalized text of a run (one space between words). Words are
/// separated at breakable whitespace only ([`is_breakable_space`]), so a
/// non-breaking space stays in the text as written.
fn normalized(text: &str) -> String {
    text.split(is_breakable_space).filter(|word| !word.is_empty()).collect::<Vec<_>>().join(" ")
}

/// How this backend prints a decoration of the element `element`: the text to
/// print, what goes before it, and whether a breakable space goes after it.
///
/// A run-in decoration is its text followed by `stop`, with a space after it
/// and none before, since it starts the content it introduces. A closing
/// decoration follows a non-breaking space, so that it stays on the line of
/// the word before it (with nothing before it when nothing precedes it). A
/// margin decoration is written between brackets and spaces. The caller has
/// already chosen `stop`: the empty text when the title ends in sentence
/// punctuation, or when the render option asks for no stop.
fn deco_shape(element: &str, plain: String, stop: &str) -> (String, Before, bool) {
    match element {
        pres::DECO_RUNIN => (format!("{plain}{stop}"), Before::Nothing, true),
        pres::DECO_FINAL => (plain, Before::Tie, false),
        pres::DECO_MARGIN => (format!("[{plain}]"), Before::Gap, true),
        _ => (plain, Before::Gap, true),
    }
}

/// The children of `node` that hold its content: the nodes of every slot
/// except the attached slots (where the lowering driver moves decorations,
/// which are printed at their decoration marks instead), or every child of a
/// node with no slot.
fn content_children<'t>(node: &NodeRef<'t, Flm>) -> Vec<NodeRef<'t, Flm>> {
    match node.slots() {
        Some(slots) if !slots.is_empty() => slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.role != SlotRole::Attached)
            .flat_map(|(index, _)| {
                node.slot_content_nodes(index).map(|nodes| nodes.iter().collect::<Vec<_>>()).unwrap_or_default()
            })
            .collect(),
        _ => node.children().iter().collect(),
    }
}

/// What a decoration is preceded by ([`deco_shape`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Before {
    /// Nothing of its own.
    Nothing,
    /// A breakable space.
    Gap,
    /// A non-breaking space, joining it to the atom before it.
    Tie,
}

impl<'t> Emitter<'t> {
    /// The trees that a `subtree` node of another lowered tree places (a tree
    /// that places itself forms a cycle; it is printed at the top level and
    /// reported).
    fn placed_trees(&self) -> BTreeSet<TreeId> {
        let mut placed = BTreeSet::new();
        for (id, lowered) in &self.trees.trees {
            for node in lowered.tree.iter_storage_order() {
                if let Some(payload) = payload_of::<SubtreePayload>(&node) {
                    if payload.tree != *id {
                        placed.insert(payload.tree);
                    }
                }
            }
        }
        placed
    }

    /// Opens one level of the walk; returns `false`, and records the refusal,
    /// past the depth limit.
    fn descend(&mut self) -> bool {
        if self.depth >= DEPTH_LIMIT {
            self.refused = true;
            return false;
        }
        self.depth += 1;
        true
    }

    /// Closes one level of the walk.
    fn ascend(&mut self) {
        self.depth -= 1;
    }

    /// The blocks of one lowered tree; nothing, with the condition recorded
    /// at the root of the outermost tree, when the depth limit was reached.
    ///
    /// Inside a piece, `tree` is the piece's lowered value: it is not a
    /// document tree, so it is neither recorded as printed nor added to the
    /// chain of placing trees.
    fn tree_doc(&mut self, id: TreeId, tree: &Arc<NodeTree<Flm>>) -> Option<DocId> {
        if !self.descend() {
            return None;
        }
        let in_piece = self.piece.is_some();
        let saved = self.tree;
        if !in_piece {
            self.tree = id;
            self.placing.push(id);
            self.printed.insert(id);
        }
        self.block_containers.push(block_containers(tree));
        let root = tree.root();
        let children: Vec<NodeRef<'_, Flm>> = root.children().iter().collect();
        let doc = if root.is_list() { self.block_run(&children) } else { self.block_run(&[root]) };
        self.block_containers.pop();
        if !in_piece {
            self.placing.pop();
        }
        self.tree = saved;
        self.ascend();
        if !self.refused {
            return doc;
        }
        if self.placing.is_empty() {
            self.refused = false;
            self.diags.push(Diagnostic::error(
                NestingTooDeep {
                    detail: format!("more than {DEPTH_LIMIT} levels"),
                },
                root.span().clone(),
            ));
        }
        None
    }

    /// One piece at the top level, one block separation after what `parts`
    /// already holds; nothing when it prints nothing.
    fn piece_part(&mut self, piece: &LoweredPiece, parts: &mut Vec<DocId>) {
        let saved = self.piece.replace(piece.name.clone());
        let tree = Arc::clone(&piece.tree.tree);
        let doc = self.tree_doc(self.tree, &tree);
        self.piece = saved;
        if let Some(doc) = doc {
            if !parts.is_empty() {
                let separator = self.buf.blocksep(1);
                parts.push(separator);
            }
            parts.push(doc);
        }
    }

    /// Wraps `doc` in a new layout mark and records which lowered node that
    /// mark stands for; inside a piece, returns `doc` unchanged.
    fn marked(&mut self, node: &NodeRef<'_, Flm>, doc: DocId) -> DocId {
        if self.piece.is_some() {
            return doc;
        }
        let id = MarkId(self.marks.len() as u32);
        self.marks.push(LoweredAddr::new(self.tree, node.id()));
        self.buf.mark(id, doc)
    }

    // --- block context ---------------------------------------------------------------------

    /// A run of nodes in block context: block-shaped nodes as blocks, maximal
    /// runs of inline nodes filled together, one blank line between blocks.
    fn block_run(&mut self, nodes: &[NodeRef<'_, Flm>]) -> Option<DocId> {
        self.block_run_of(None, nodes)
    }

    /// [`block_run`](Self::block_run) over the content run of the
    /// presentation node `carrier`, when there is one: a decoration the
    /// lowering driver moved out of that run is printed at its decoration
    /// mark, as inline
    /// content, on a line of its own when a block follows it.
    fn block_run_of(&mut self, carrier: Option<&NodeRef<'_, Flm>>, nodes: &[NodeRef<'_, Flm>]) -> Option<DocId> {
        let mut parts: Vec<DocId> = Vec::new();
        let mut inline: Vec<Tok> = Vec::new();
        let flush = |this: &mut Emitter<'_>, inline: &mut Vec<Tok>, parts: &mut Vec<DocId>| {
            if inline.is_empty() {
                return;
            }
            let toks = std::mem::take(inline);
            if let Some(doc) = this.fill(toks) {
                parts.push(doc);
            }
        };
        for node in nodes {
            if is_inert(node) {
                flush(self, &mut inline, &mut parts);
                continue;
            }
            if is_block_shaped(node) || self.is_block_container(node) {
                flush(self, &mut inline, &mut parts);
                if let Some(doc) = self.block(node) {
                    parts.push(doc);
                }
            } else if let (Some(carrier), true) = (carrier, is_element(node, pres::DECO_MARK)) {
                inline.extend(self.decoration(carrier, node));
            } else {
                inline.extend(self.inline_tokens(node));
            }
        }
        flush(self, &mut inline, &mut parts);
        if parts.is_empty() {
            return None;
        }
        let mut separated: Vec<DocId> = Vec::with_capacity(parts.len() * 2);
        for (index, part) in parts.into_iter().enumerate() {
            if index > 0 {
                let separator = self.buf.blocksep(1);
                separated.push(separator);
            }
            separated.push(part);
        }
        Some(self.buf.concat(&separated))
    }

    /// Whether a container that is not a presentation node, in a run of
    /// blocks, holds a block-shaped node; it is then laid out as a run of
    /// blocks (the set [`block_containers`] computed on entering the tree).
    fn is_block_container(&self, node: &NodeRef<'_, Flm>) -> bool {
        self.block_containers.last().is_some_and(|set| set.contains(&node.id().index()))
    }

    /// One block-shaped node (or a container of blocks), marked; nothing past
    /// the depth limit.
    fn block(&mut self, node: &NodeRef<'_, Flm>) -> Option<DocId> {
        if !self.descend() {
            return None;
        }
        // A block standing inside a superscript or subscript span is not
        // raised or lowered with the span's text: it takes lines of its own.
        let script = self.script.take();
        let doc = self.block_at(node);
        self.script = script;
        self.ascend();
        doc
    }

    /// One block-shaped node at the level already opened: the element's own
    /// layout, marked; a node that is not a presentation node contributes its
    /// children as a run of blocks.
    fn block_at(&mut self, node: &NodeRef<'_, Flm>) -> Option<DocId> {
        if !is_presentation(node) {
            let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
            return self.block_run(&children);
        }
        let name = element_of(node).name.clone();
        let doc = match name.as_str() {
            pres::PAR => {
                let body: Vec<NodeRef<'_, Flm>> =
                    node.body().map(|b| b.iter().collect()).unwrap_or_default();
                let toks = self.inline_run(node, &body);
                self.fill(toks)?
            }
            pres::BLOCK_HEADING => self.heading(node)?,
            pres::BLOCK => {
                let body: Vec<NodeRef<'_, Flm>> =
                    node.body().map(|b| b.iter().collect()).unwrap_or_default();
                self.block_run_of(Some(node), &body)?
            }
            pres::LIST => self.list(node)?,
            pres::CELLS => self.cells(node)?,
            pres::LINES => self.lines(node)?,
            pres::VERBATIM => self.verbatim_block(node)?,
            pres::MATH_PASSTHROUGH => self.math_block(node)?,
            pres::SUBTREE => self.subtree(node)?,
            pres::UNKNOWN => self.unknown_environment(node)?,
            _ => {
                let toks = self.inline_tokens(node);
                return self.fill(toks);
            }
        };
        Some(self.marked(node, doc))
    }

    /// The address a `flm.pres.link` prints after its content, `<address>`,
    /// where the options ask for it: never for a link inside the document (a
    /// target beginning with `#`), and, for a link whose content already shows
    /// its address, only when
    /// [`link_addresses_when_shown`](TextRenderOptions::link_addresses_when_shown)
    /// is on. `None` where nothing is printed.
    fn printed_address(&self, node: &NodeRef<'_, Flm>) -> Option<String> {
        let payload = payload_of::<LinkPayload>(node)?;
        let printed = self.link_addresses
            && (!payload.shows_target || self.link_addresses_when_shown)
            && !payload.href.starts_with('#');
        printed.then(|| format!("<{}>", one_line(&payload.href)))
    }

    /// A `flm.pres.block-heading`: the heading text with a rule under it at
    /// depths 0 and 1 below the root heading level
    /// ([`TextRenderOptions::root_heading_level`], counted from 0 by
    /// [`heading_depth`]; a heading above the root is reported and written at
    /// depth 0), and `depth + 1` `#` characters in front of it below that — three
    /// for a heading two levels below the root. The rule is as wide as the
    /// words of the title (a decoration in it counted as it is printed,
    /// [`printed_words`](Self::printed_words)) plus the addresses its links
    /// print after them.
    fn heading(&mut self, node: &NodeRef<'_, Flm>) -> Option<DocId> {
        let level = payload_of::<HeadingPayload>(node)
            .map_or(self.options.root_heading_level, |payload| payload.level);
        let depth = match heading_depth(level, self.options.root_heading_level) {
            Ok(depth) => depth,
            Err(above) => {
                self.diags.push(Diagnostic::error(above, node.span().clone()));
                0
            }
        };
        let body: Vec<NodeRef<'_, Flm>> =
            node.body().map(|b| b.iter().collect()).unwrap_or_default();
        let mut plain = normalized(&self.printed_words(node, &body));
        for link in body
            .iter()
            .flat_map(|child| std::iter::once(*child).chain(child.descendants()))
            .filter(|descendant| is_element(descendant, pres::LINK))
        {
            if let Some(address) = self.printed_address(&link) {
                plain.push(' ');
                plain.push_str(&address);
            }
        }
        let toks = self.inline_run(node, &body);
        let text = self.fill(toks);
        match depth {
            0 | 1 => {
                let rule = if depth == 0 { "=" } else { "-" };
                let text = text?;
                let underline = rule.repeat(display_width(&plain) as usize);
                let bar = self.buf.text(&underline);
                let br = self.buf.hardline();
                Some(self.buf.concat(&[text, br, bar]))
            }
            _ => {
                let prefix = format!("{} ", "#".repeat(usize::from(depth) + 1));
                let prefix = self.buf.text(&prefix);
                match text {
                    Some(text) => Some(self.buf.concat(&[prefix, text])),
                    None => Some(prefix),
                }
            }
        }
    }

    /// A `flm.pres.list`: one entry per label/item pair, every item's blocks
    /// hanging at one column, the width of the widest label plus one, or at
    /// the margin when every label is empty.
    fn list(&mut self, node: &NodeRef<'_, Flm>) -> Option<DocId> {
        let entries = list_entries(node);
        if entries.is_empty() {
            return None;
        }
        let labels: Vec<String> =
            entries.iter().map(|(label, _)| normalized(&label_text(label.iter()))).collect();
        // A column is kept for the labels only where some label is written:
        // a list whose labels are all empty hangs its items at the margin.
        let column = match labels.iter().map(|label| display_width(label)).max() {
            None | Some(0) => 0,
            Some(widest) => widest + 1,
        };
        let mut parts: Vec<DocId> = Vec::new();
        for (index, (_, item)) in entries.iter().enumerate() {
            if !parts.is_empty() {
                let separator = self.buf.blocksep(1);
                parts.push(separator);
            }
            let items: Vec<NodeRef<'_, Flm>> = item.iter().collect();
            match self.block_run(&items) {
                Some(body) => {
                    let mut padded = labels[index].clone();
                    while display_width(&padded) < column {
                        padded.push(' ');
                    }
                    let label = self.buf.text(&padded);
                    let hung = self.buf.align(body);
                    parts.push(self.buf.concat(&[label, hung]));
                }
                // An empty item prints its label alone: padding it would end
                // the line in whitespace.
                None => parts.push(self.buf.text(&labels[index])),
            }
        }
        Some(self.buf.concat(&parts))
    }

    /// A `flm.pres.cells`: one table of the layout engine, built from the
    /// payload's grid ([`CellsPayload::grid`]): one column per column, one
    /// row per row, each cell's content as a run of blocks at its top-left
    /// position in the cell's own alignment, and an empty document at every
    /// other position (covered by a span, or empty). The leading rows of
    /// column headers are the engine's header rows, with its rule under them.
    /// Row headers, rules and custom styles are not shown in plain text.
    fn cells(&mut self, node: &NodeRef<'_, Flm>) -> Option<DocId> {
        let (payload, contents) = cells_of(node)?;
        let contents: Vec<Option<DocId>> = contents
            .iter()
            .map(|content| {
                let nodes: Vec<NodeRef<'_, Flm>> =
                    content.map(|slice| slice.iter().collect()).unwrap_or_default();
                self.block_run(&nodes)
            })
            .collect();
        let empty = self.buf.concat(&[]);
        let grid = payload.grid();
        let rows: Vec<Vec<TableCell>> = grid
            .iter()
            .map(|row| {
                row.iter()
                    .map(|position| match *position {
                        GridPosition::TopLeft(index) => TableCell {
                            doc: contents[index].unwrap_or(empty),
                            align: payload.cells[index].align.map(col_align),
                        },
                        GridPosition::Covered(_) | GridPosition::Empty => TableCell::new(empty),
                    })
                    .collect()
            })
            .collect();
        let row_refs: Vec<&[TableCell]> = rows.iter().map(Vec::as_slice).collect();
        let cols = vec![ColSpec::left(); payload.cols as usize];
        let header_rows = header_rows(payload, &grid);
        Some(self.buf.table(&cols, &row_refs, header_rows))
    }

    /// A `flm.pres.lines`: one printed line per `flm.pres.line` child,
    /// indented by that line's left indentation.
    fn lines(&mut self, node: &NodeRef<'_, Flm>) -> Option<DocId> {
        let body: Vec<NodeRef<'_, Flm>> =
            node.body().map(|b| b.iter().collect()).unwrap_or_default();
        let mut parts: Vec<DocId> = Vec::new();
        for line in &body {
            if is_inert(line) {
                continue;
            }
            let content: Vec<NodeRef<'_, Flm>> = if is_element(line, pres::LINE) {
                line.body().map(|b| b.iter().collect()).unwrap_or_default()
            } else {
                vec![*line]
            };
            let indent = payload_of::<LinePayload>(line)
                .map_or(0, |payload| self.options.columns(&payload.indent_left));
            // The layout engine indents by at most `u16::MAX` columns.
            let indent = u16::try_from(indent).unwrap_or(u16::MAX);
            let toks = self.inline_run(line, &content);
            let filled = self.fill(toks).unwrap_or_else(|| self.buf.text(""));
            let indented = if indent > 0 { self.buf.nest(indent, filled) } else { filled };
            let marked =
                if is_element(line, pres::LINE) { self.marked(line, indented) } else { indented };
            if !parts.is_empty() {
                let br = self.buf.hardline();
                parts.push(br);
            }
            parts.push(marked);
        }
        if parts.is_empty() {
            return None;
        }
        Some(self.buf.concat(&parts))
    }

    /// The text of verbatim content: text nodes as they are, the children of
    /// a group substituted into it without the group's braces (see the module
    /// documentation), anything else as its source spelling.
    fn verbatim_text(&mut self, nodes: &[NodeRef<'_, Flm>]) -> String {
        let mut out = String::new();
        for node in nodes {
            match node.kind() {
                NodeKind::Chars { .. } => out.push_str(node.chars().unwrap_or("")),
                NodeKind::Group(_) if node.group_type() == Some(FlmGroupType::Content) => {
                    if self.descend() {
                        let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
                        out.push_str(&self.verbatim_text(&children));
                        self.ascend();
                    }
                }
                _ => out.push_str(&recompose_source(&[*node])),
            }
        }
        out
    }

    /// A `flm.pres.verbatim` printed as a block: its content byte for byte,
    /// indented like its surroundings.
    fn verbatim_block(&mut self, node: &NodeRef<'_, Flm>) -> Option<DocId> {
        let body: Vec<NodeRef<'_, Flm>> =
            node.body().map(|b| b.iter().collect()).unwrap_or_default();
        let text = self.verbatim_text(&body);
        if text.is_empty() {
            return None;
        }
        Some(self.buf.raw(&text, RawPolicy::Indented))
    }

    /// The body of a math node written out: its nodes as their source, with
    /// each text node among them ([`flm.pres.math-text`](pres::MATH_TEXT))
    /// written as the command it was written with, around its content on one
    /// line, and nothing at all for one with no content (a link target, which
    /// shows nothing).
    ///
    /// A text node of this kind is one of the direct nodes of the body, where
    /// the rule that puts text inside mathematics stages (builds) it. Such a
    /// node reached anywhere else is printed by this emitter's ordinary inline
    /// path, which writes its content without the command: outside a formula,
    /// the command means nothing this backend can print.
    ///
    /// The content is flattened into one line of text here, so a construct
    /// inside it contributes its text alone: mathematics inside it loses its
    /// delimiters on this path. No rule of the standard package stages that
    /// nesting on this backend, and a backend that renders formulas reads the
    /// content's nodes rather than this string.
    fn math_body(&self, body: &[NodeRef<'_, Flm>]) -> String {
        let mut out = String::new();
        let mut source: Vec<NodeRef<'_, Flm>> = Vec::new();
        for node in body {
            if !is_element(node, pres::MATH_TEXT) {
                source.push(*node);
                continue;
            }
            out.push_str(&recompose_source(&source));
            source.clear();
            let content: Vec<NodeRef<'_, Flm>> =
                node.body().map(|b| b.iter().collect()).unwrap_or_default();
            let content = normalized(&plain_text(content.iter().copied()));
            if content.is_empty() {
                continue;
            }
            match payload_of::<MathTextPayload>(node).and_then(|p| p.command.as_deref()) {
                Some(command) => out.push_str(&format!("\\{command}{{{content}}}")),
                None => out.push_str(&content),
            }
        }
        out.push_str(&recompose_source(&source));
        out
    }

    /// The source of a math node, with its delimiters.
    fn math_text(&self, node: &NodeRef<'_, Flm>) -> (MathDisplay, String) {
        let body: Vec<NodeRef<'_, Flm>> =
            node.body().map(|b| b.iter().collect()).unwrap_or_default();
        let source = self.math_body(&body);
        let payload = payload_of::<MathPassthroughPayload>(node);
        let display = payload.map_or(MathDisplay::Inline, |payload| payload.display);
        let text = match (display, payload.and_then(|payload| payload.env.as_deref())) {
            (_, Some(env)) => format!("\\begin{{{env}}}{source}\\end{{{env}}}"),
            (MathDisplay::Inline, None) => format!("\\({source}\\)"),
            (MathDisplay::Display, None) => format!("\\[{source}\\]"),
        };
        (display, text)
    }

    /// A display `flm.pres.math-passthrough`: the math source with its delimiters,
    /// as a block indented by two columns.
    fn math_block(&mut self, node: &NodeRef<'_, Flm>) -> Option<DocId> {
        let (_, text) = self.math_text(node);
        let raw = self.buf.raw(&text, RawPolicy::Indented);
        Some(self.buf.nest(2, raw))
    }

    /// An unattributed environment: `\begin{name}` on a line of its own, the
    /// lowered body as a run of blocks, then `\end{name}` on a line of its own.
    /// What the author wrote is document content and is printed as such; the
    /// output does not say that the environment is unattributed (declared by
    /// no feature), which the run's diagnostics report instead.
    fn unknown_environment(&mut self, node: &NodeRef<'_, Flm>) -> Option<DocId> {
        let name = payload_of::<UnknownPayload>(node)
            .map_or_else(String::new, |payload| one_line(&payload.spelling));
        let begin = self.buf.text(&format!("\\begin{{{name}}}"));
        let body: Vec<NodeRef<'_, Flm>> =
            node.body().map(|b| b.iter().collect()).unwrap_or_default();
        let content = self.block_run(&body);
        let end = self.buf.text(&format!("\\end{{{name}}}"));
        let mut parts = vec![begin];
        if let Some(content) = content {
            let br = self.buf.hardline();
            parts.push(br);
            parts.push(content);
        }
        let br = self.buf.hardline();
        parts.push(br);
        parts.push(end);
        Some(self.buf.concat(&parts))
    }

    /// A `flm.pres.subtree`: the placed tree's own blocks, printed here.
    /// Nothing, with a diagnostic, inside a piece, or when that tree is being
    /// printed already or has no lowered form.
    fn subtree(&mut self, node: &NodeRef<'_, Flm>) -> Option<DocId> {
        let payload = payload_of::<SubtreePayload>(node)?;
        let id = payload.tree;
        if let Some(piece) = &self.piece {
            self.diags.push(Diagnostic::error(
                PiecePlacement { piece: piece.to_string(), tree: id.to_string() },
                node.span().clone(),
            ));
            return None;
        }
        if self.placing.contains(&id) {
            self.diags.push(Diagnostic::error(
                SubtreeCycle { tree: id.to_string() },
                node.span().clone(),
            ));
            return None;
        }
        let Some(lowered) = self.trees.get(id) else {
            self.diags.push(Diagnostic::error(
                MissingSubtree { tree: id.to_string() },
                node.span().clone(),
            ));
            return None;
        };
        let tree = Arc::clone(&lowered.tree);
        self.tree_doc(id, &tree)
    }

    // --- inline context ---------------------------------------------------------------------

    /// A token stream as filled prose: gaps become spaces that break
    /// independently of each other (leading, trailing and repeated gaps
    /// collapse), a tie a non-breaking space (leading and trailing ties are
    /// dropped, and a tie next to a gap replaces it, so that what it joins
    /// stays together), and block-shaped nodes take lines of their own.
    /// `None` when nothing is printed.
    fn fill(&mut self, toks: Vec<Tok>) -> Option<DocId> {
        let mut parts: Vec<DocId> = Vec::new();
        let mut pending_gap = false;
        let mut pending_tie = false;
        let mut after_block = false;
        for tok in toks {
            match tok {
                Tok::Gap => pending_gap = !parts.is_empty(),
                Tok::Tie => pending_tie = !parts.is_empty() && !after_block,
                Tok::Atom(doc) => {
                    if after_block {
                        let br = self.buf.hardline();
                        parts.push(br);
                    } else if pending_tie {
                        let tie = self.buf.text("\u{a0}");
                        parts.push(tie);
                    } else if pending_gap {
                        let line = self.buf.line();
                        let separator = self.buf.group(line);
                        parts.push(separator);
                    }
                    pending_gap = false;
                    pending_tie = false;
                    after_block = false;
                    parts.push(doc);
                }
                Tok::Block(doc) => {
                    if !parts.is_empty() {
                        let br = self.buf.hardline();
                        parts.push(br);
                    }
                    pending_gap = false;
                    pending_tie = false;
                    after_block = true;
                    parts.push(doc);
                }
            }
        }
        if parts.is_empty() {
            return None;
        }
        Some(self.buf.concat(&parts))
    }

    /// The inline run of `carrier`: its content, with the decorations moved
    /// out of it printed at their decoration marks.
    fn inline_run(&mut self, carrier: &NodeRef<'_, Flm>, nodes: &[NodeRef<'_, Flm>]) -> Vec<Tok> {
        let mut toks = Vec::new();
        for node in nodes {
            if is_element(node, pres::DECO_MARK) {
                toks.extend(self.decoration(carrier, node));
            } else {
                toks.extend(self.inline_tokens(node));
            }
        }
        toks
    }

    /// A decoration moved into an attached slot, at the position of its
    /// decoration mark.
    fn decoration(&mut self, carrier: &NodeRef<'_, Flm>, mark: &NodeRef<'_, Flm>) -> Vec<Tok> {
        match decoration_at(carrier, mark) {
            Some(deco) => self.deco_tokens(&deco),
            None => Vec::new(),
        }
    }

    /// A decoration as tokens: its content flattened to text, shaped as its
    /// element asks ([`deco_shape`]) and surrounded by the breakable spaces
    /// that shape asks for. A decoration whose content holds no text
    /// ([`holds_text`]) contributes nothing.
    ///
    /// Every decoration is printed through this function, so a decoration the
    /// driver moved into an attached slot and one reached where no mark
    /// stands for it are printed alike.
    ///
    /// Whether a run-in decoration takes a stop is decided from its content
    /// tree, by the two tests every backend shares ([`holds_text`] and
    /// [`ends_in_sentence_punctuation`]), and never from the text this emitter
    /// has flattened it to.
    fn deco_tokens(&mut self, deco: &NodeRef<'_, Flm>) -> Vec<Tok> {
        let Some((text, before, after)) = self.deco_text(deco) else { return Vec::new() };
        let atom = self.buf.text(&text);
        let marked = self.marked(deco, atom);
        let mut toks = Vec::new();
        match before {
            Before::Nothing => {}
            Before::Gap => toks.push(Tok::Gap),
            Before::Tie => toks.push(Tok::Tie),
        }
        toks.push(Tok::Atom(marked));
        if after {
            toks.push(Tok::Gap);
        }
        toks
    }

    /// What [`deco_tokens`](Self::deco_tokens) sets for the decoration
    /// `deco`: its text, what goes before it and whether a breakable space
    /// goes after it ([`deco_shape`]); `None` for a decoration whose content
    /// holds no text ([`holds_text`]).
    fn deco_text(&self, deco: &NodeRef<'_, Flm>) -> Option<(String, Before, bool)> {
        let body: Vec<NodeRef<'_, Flm>> =
            deco.body().map(|b| b.iter().collect()).unwrap_or_default();
        if !holds_text(body.iter().copied()) {
            return None;
        }
        let plain = normalized(&label_text(body.iter().copied()));
        let element = element_of(deco).name.as_str();
        let stop = if element == pres::DECO_RUNIN
            && !ends_in_sentence_punctuation(body.iter().copied())
        {
            self.run_in_stop
        } else {
            ""
        };
        Some(deco_shape(element, plain, stop))
    }

    /// The words of `nodes`, the content run of the presentation node
    /// `carrier`, as this backend prints them, for measuring: text as
    /// written, and a decoration the lowering driver moved out of a node's
    /// content as it is printed at its decoration mark
    /// ([`deco_text`](Self::deco_text)),
    /// not as its bare words in the attached slot.
    fn printed_words(&self, carrier: &NodeRef<'_, Flm>, nodes: &[NodeRef<'_, Flm>]) -> String {
        let mut out = String::new();
        for node in nodes {
            if let Some(text) = node.chars() {
                out.push_str(text);
            } else if is_element(node, pres::DECO_MARK) {
                let Some((text, before, after)) =
                    decoration_at(carrier, node).and_then(|deco| self.deco_text(&deco))
                else {
                    continue;
                };
                match before {
                    Before::Nothing => {}
                    Before::Gap => out.push(' '),
                    Before::Tie => out.push('\u{a0}'),
                }
                out.push_str(&text);
                if after {
                    out.push(' ');
                }
            } else {
                out.push_str(&self.printed_words(node, &content_children(node)));
            }
        }
        out
    }

    /// One node of an inline run, as tokens; none past the depth limit.
    fn inline_tokens(&mut self, node: &NodeRef<'_, Flm>) -> Vec<Tok> {
        if !self.descend() {
            return Vec::new();
        }
        let toks = self.inline_tokens_at(node);
        self.ascend();
        toks
    }

    /// One node of an inline run at the level already opened: text split into
    /// words and gaps, a presentation element as its own atom or block,
    /// anything else as its children.
    fn inline_tokens_at(&mut self, node: &NodeRef<'_, Flm>) -> Vec<Tok> {
        if let Some(text) = node.chars() {
            let mut toks = Vec::new();
            if text.starts_with(is_breakable_space) {
                toks.push(Tok::Gap);
            }
            for word in text.split(is_breakable_space).filter(|word| !word.is_empty()) {
                let word = match self.script {
                    Some(script) => Cow::Owned(script.write(word)),
                    None => Cow::Borrowed(word),
                };
                let atom = self.buf.text(&word);
                toks.push(Tok::Atom(atom));
                toks.push(Tok::Gap);
            }
            if !text.ends_with(is_breakable_space) && !text.is_empty() {
                toks.pop();
            }
            return toks;
        }
        if is_inert(node) {
            return if is_element(node, structural::PARBREAK) {
                vec![Tok::Gap]
            } else {
                Vec::new()
            };
        }
        if !is_presentation(node) {
            let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
            return self.inline_run(node, &children);
        }
        let name = element_of(node).name.clone();
        let doc = match name.as_str() {
            pres::SPAN | pres::RUNIN_HEADING => {
                let body: Vec<NodeRef<'_, Flm>> =
                    node.body().map(|b| b.iter().collect()).unwrap_or_default();
                // A superscript or subscript span writes the text of its
                // content through the forms of its style, nested content
                // included; a nested span of the other style writes its own
                // its own way.
                // See decision rationale at [§flm-dd-dr:rend-text-superscript].
                let outer = self.script;
                if name.as_str() == pres::SPAN {
                    let style = payload_of::<SpanPayload>(node).and_then(|payload| payload.style);
                    if let Some(script) = Script::of(style) {
                        self.script = Some(script);
                    }
                }
                let toks = self.inline_run(node, &body);
                self.script = outer;
                match self.fill(toks) {
                    Some(doc) => doc,
                    None => return Vec::new(),
                }
            }
            pres::LINK => {
                let body: Vec<NodeRef<'_, Flm>> =
                    node.body().map(|b| b.iter().collect()).unwrap_or_default();
                let toks = self.inline_run(node, &body);
                let content = self.fill(toks);
                match (content, self.printed_address(node)) {
                    (Some(content), Some(address)) => {
                        let tail = self.buf.text(&format!(" {address}"));
                        self.buf.concat(&[content, tail])
                    }
                    (Some(content), None) => content,
                    (None, Some(address)) => self.buf.text(&address),
                    (None, None) => return Vec::new(),
                }
            }
            pres::MATH_PASSTHROUGH => {
                let (display, text) = self.math_text(node);
                match display {
                    MathDisplay::Inline => self.buf.text(&normalized(&text)),
                    MathDisplay::Display => {
                        let raw = self.buf.raw(&text, RawPolicy::Indented);
                        let nested = self.buf.nest(2, raw);
                        let marked = self.marked(node, nested);
                        return vec![Tok::Block(marked)];
                    }
                }
            }
            pres::VERBATIM => {
                let body: Vec<NodeRef<'_, Flm>> =
                    node.body().map(|b| b.iter().collect()).unwrap_or_default();
                let text = self.verbatim_text(&body);
                if text.is_empty() {
                    return Vec::new();
                }
                let block = payload_of::<VerbatimPayload>(node)
                    .is_some_and(|payload| payload.display == VerbatimDisplay::Block);
                if block || text.contains(['\n', '\t']) {
                    let raw = self.buf.raw(&text, RawPolicy::Indented);
                    let marked = self.marked(node, raw);
                    return vec![Tok::Block(marked)];
                }
                self.buf.text(&text)
            }
            pres::IMAGE => {
                let label = payload_of::<ImagePayload>(node)
                    .map(|payload| {
                        let shown = payload.alt.as_deref().filter(|alt| !alt.trim().is_empty());
                        format!("[image: {}]", one_line(shown.unwrap_or(&payload.src)))
                    })
                    .unwrap_or_else(|| String::from("[image]"));
                self.buf.text(&label)
            }
            pres::UNSUPPORTED => {
                let element = payload_of::<UnsupportedPayload>(node)
                    .map_or_else(|| String::from("?"), |payload| payload.element.to_string());
                let text = format!("[unsupported: {element}]");
                self.buf.text(&text)
            }
            pres::DROPPED => {
                let detail = payload_of::<DroppedPayload>(node)
                    .map_or_else(|| String::from("?"), |payload| one_line(&payload.detail));
                let text = format!("[dropped: {detail}]");
                self.buf.text(&text)
            }
            // An unattributed macro, inline: its source spelling. The
            // environment kind is block-shaped and falls to the arm below.
            pres::UNKNOWN if !is_block_shaped(node) => {
                let text = payload_of::<UnknownPayload>(node)
                    .map_or_else(String::new, |payload| one_line(&payload.spelling));
                if text.is_empty() {
                    return Vec::new();
                }
                self.buf.text(&text)
            }
            pres::DECO_MARK => return Vec::new(),
            pres::DECO_RUNIN | pres::DECO_FINAL | pres::DECO_MARGIN => {
                // A decoration met in a run with no mark standing for it: it was
                // never hoisted, because the run it stands in is not the content
                // of a presentation node — an inline-layout fragment has no
                // paragraph to attach it to. It is set with the same idiom as a
                // hoisted one, so that one document reads alike under either
                // layout.
                return self.deco_tokens(node);
            }
            _ if is_block_shaped(node) => {
                return match self.block(node) {
                    Some(doc) => vec![Tok::Block(doc)],
                    None => Vec::new(),
                };
            }
            _ => {
                let body: Vec<NodeRef<'_, Flm>> =
                    node.body().map(|b| b.iter().collect()).unwrap_or_default();
                let toks = self.inline_run(node, &body);
                match self.fill(toks) {
                    Some(doc) => doc,
                    None => return Vec::new(),
                }
            }
        };
        let marked = self.marked(node, doc);
        vec![Tok::Atom(marked)]
    }
}
