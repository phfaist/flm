//! The general lowering rules of the xnote and of a list entry, and the
//! public staging helpers that every rule writing an xnote calls.
//!
//! A lowering rule ([`Lower`]) stages (builds into the lowered tree, through
//! the [`Lowerer`]) the presentation of one element for the backend being
//! rendered; its default mark says on which backends it is chosen. The
//! general rules of this module are marked default on every backend:
//!
//! - [`XnoteRule`] ([`XNOTE_RULE`](super::XNOTE_RULE)) writes an xnote where
//!   it is written. For a listed xnote kind it writes the xnote's **mark**, a
//!   superscript span linked to the xnote; for an in-place kind it writes the
//!   xnote **in place**: one span holding the mark and the content where the
//!   content is inline, one block holding the content's paragraphs where the
//!   content holds blocks.
//! - [`EntryRule`] ([`ENTRY_RULE`](super::ENTRY_RULE)) is the rule of an entry
//!   of a list of xnotes lowered outside its list: it writes nothing and
//!   reports it.
//!
//! The shapes are staged by public helpers, [`stage_mark`],
//! [`stage_in_place`], [`stage_mark_text`] and [`append_to_last_paragraph`].
//! The feature's own rules call them, and so does the rule of another
//! feature's element that carries xnotes, so that every xnote is written in
//! the same way; each helper states what it stages. The
//! [module documentation of the feature](super) describes xnotes, xnote kinds
//! and marks.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{Lower, LowerCx, LowerError, Lowerer, Lowering, NodeRegion};
use crate::core::run::MissingEntry;
use crate::lang::rewrites::blocks::{argument_holds_blocks, slot_holds_blocks, stands_in_blocks};
use crate::lang::text::{is_blank, plain_text};
use crate::lang::{Flm, FlmGroupType};
use crate::model::aspects::{BlockRole, PointId, block_role_of};
use crate::model::element::element_value;
use crate::model::names::{CapName, ElementName, Name};
use crate::stdcore::backends::lowering::{lower_blocks_with_edges, lower_run};
use crate::stdcore::backends::pres::{
    self, BlockPayload, DocRole, LinkPayload, SpanPayload, SpanStyle,
};
use crate::stdcore::features::numbering::{self, COUNTERS, FormatOptions};
use crate::stdcore::features::refs::{ANCHORS, CAPITAL_VARIANT};

use super::config::{XnoteKind, XnotePlacement};
use super::diagnostics::{BadLabelArgument, Empty, EntryOutsideList};
use super::stages::{XNOTES, XnoteRecord};
use super::{
    contents_of_kind, labels_of, mark_style_name, XnoteElement, INPLACE_ROLE, KIND_XNOTES, LINK_KIND, MARK_ROLE,
    STYLE,
};

/// How the brackets around an in-place xnote are written: the only
/// difference between an in-place xnote written by the HTML rule and one
/// written by the general rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Brackets {
    /// `[` and `]` are staged as text around the in-place xnote, before its
    /// mark and after its content. Used by the general rule (the plain-text
    /// output) and by any rule that writes the brackets as text.
    Text,
    /// No bracket is staged, and the output draws its own: the HTML page's
    /// stylesheet draws them, so that a page's own stylesheet can replace
    /// them.
    Drawn,
}

/// The record that the table of xnotes holds for the `nth` xnote of the xnote
/// kind `kind` that `node` carries, counting from 0, found among that kind's
/// records by the node's address
/// ([`Xnotes::records_at`](super::Xnotes::records_at)); `None` where the table
/// holds no record for it.
///
/// # Errors
///
/// [`LowerError`] when `node` is not a node of the tree being lowered, when
/// the calling rule did not declare that it reads [`XNOTES`], and when the
/// table records the node's xnotes of another kind only: the caller passed
/// the wrong kind.
pub(super) fn find(
    lw: &dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    kind: &XnoteKind,
    nth: usize,
) -> Result<Option<XnoteRecord>, LowerError> {
    let addr = lw.addr_of(node)?;
    let Some(xnotes) = lw.get(XNOTES)? else {
        return Ok(None);
    };
    let mut of_kind = xnotes.records_at(kind.name.as_str(), addr).peekable();
    if of_kind.peek().is_some() {
        return Ok(of_kind.nth(nth).cloned());
    }
    if let Some((other, _)) = xnotes.record_at(addr) {
        return Err(LowerError::structure(format!(
            "the xnote is recorded of the kind `{other}`, and was written as one of the kind `{}`",
            kind.name
        )));
    }
    Ok(None)
}

/// The content of one xnote of a node, as the node's
/// [`XnoteSource`](super::XnoteSource) aspect returns it.
pub(super) enum Content<'t> {
    /// A region of the node, and its content nodes: `None` for an argument a
    /// parse recovery left absent, or a region the node does not have.
    Region(NodeRegion, Option<Vec<NodeRef<'t, Flm>>>),
    /// The node's aspect returns no xnote at that position.
    Nothing,
}

impl<'t> Content<'t> {
    /// The content of the `nth` xnote of the xnote kind `kind` that `node`
    /// carries, counting from 0: the `nth` of that kind that the node's
    /// [`XnoteSource`](super::XnoteSource) aspect returns, under the pairing
    /// that aspect states.
    pub(super) fn of(node: &NodeRef<'t, Flm>, kind: &XnoteKind, nth: usize) -> Content<'t> {
        let Some(region) = contents_of_kind(node, kind.name.as_str()).get(nth).copied() else {
            return Content::Nothing;
        };
        let nodes = if region.is_absent(node) { None } else { region.content_nodes(node) };
        Content::Region(region, nodes)
    }

    /// The content nodes: none for no content, and for an argument a parse
    /// recovery left absent.
    pub(super) fn nodes(&self) -> &[NodeRef<'t, Flm>] {
        match self {
            Content::Region(_, Some(nodes)) => nodes,
            Content::Region(_, None) | Content::Nothing => &[],
        }
    }

    /// Whether the content holds nothing but whitespace (`~` and `\ `
    /// included) and comments. An argument a parse recovery left absent is
    /// not empty: the parse has reported it.
    fn is_empty(&self) -> bool {
        match self {
            Content::Region(_, None) => false,
            Content::Region(_, Some(nodes)) => !nodes.iter().any(is_content),
            Content::Nothing => true,
        }
    }

    /// Whether the block-structure pass laid the content out as blocks: for an
    /// argument as [`argument_holds_blocks`] says, for a slot as
    /// [`slot_holds_blocks`] says.
    pub(super) fn holds_blocks(&self, node: &NodeRef<'_, Flm>) -> bool {
        match self {
            Content::Region(NodeRegion::Argument(index), _) => argument_holds_blocks(node, *index),
            Content::Region(NodeRegion::Slot(index), _) => slot_holds_blocks(node, *index),
            Content::Nothing => false,
        }
    }
}

/// Whether `node` is content: anything but a comment and a blank node
/// ([`is_blank`]: whitespace-only text, and a node that is whitespace, such as
/// `~` and `\ `), so `\footnote{~}` is as empty as `\footnote{ }`.
fn is_content(node: &NodeRef<'_, Flm>) -> bool {
    !(node.is_comment() || is_blank(node))
}

/// Whether `node` is a paragraph that the block-structure pass built.
fn is_paragraph(node: &NodeRef<'_, Flm>) -> bool {
    node.group_type() == Some(FlmGroupType::Paragraph)
}

/// Stages (builds into the lowered tree, through `lw`; see [`Lowerer`]) the
/// mark of one xnote that the node `node` carries, of the xnote kind `kind`: the shape a listed kind writes where the xnote was written.
/// It also reports the problems of the xnote: [`Empty`](super::Empty) and
/// `flm.run.missing-entry`, in the cases listed below.
///
/// It writes the mark alone, never the content: the content is shown, and
/// lowered once, where the kind's list shows it.
///
/// **What the caller passes.** `node` is the node being lowered: a node
/// carrying xnotes, whose element implements
/// [`XnoteSource`](super::XnoteSource) and returns its reference points with
/// [`ref_points`](super::ref_points), so that the table [`XNOTES`] records
/// it. `kind` is the kind of the xnote written, the one the caller declared
/// through [`contribute_kind`](super::contribute_kind). `nth` is the xnote's
/// position among the node's xnotes of that kind, counting from 0: the record
/// and the content of that xnote are the ones paired as
/// [`XnoteSource`](super::XnoteSource) states. An element carrying one xnote
/// of the kind passes `0`; an element carrying several calls the helper once
/// for each, with `0`, `1`, …, and places the marks where it writes them,
/// side by side in that order or apart.
///
/// **What the calling rule must declare.** It must declare that it reads
/// [`XNOTES`] and that it produces `flm.pres.span` and `flm.pres.link`. It may
/// declare optional reads of the numbering feature's counter table
/// [`COUNTERS`] and of the references feature's anchors table [`ANCHORS`]:
/// without the first, the mark has no number; without the second, it has no
/// anchor and no link.
///
/// **What it stages.** For an xnote that the table records, it stages one
/// `flm.pres.span { style: Superscript, role: flm.xnotes.xnote.mark, anchor:
/// <the mark's anchor> }` holding a
/// `flm.pres.link { href: "#<the xnote's anchor>", kind: xnote, doc_role:
/// NoteRef, label: "<Word> <n>" }`, which holds the mark text
/// ([`stage_mark_text`]). The link's label is its accessible name, the name
/// that assistive technology reads in place of the link's text: the kind's
/// singular word (its capital variant, where one is declared) with its
/// non-breaking space made a space, then the xnote's number as a reference
/// writes it without the word (`Footnote 1`); the word alone where there is
/// no number. The word is read as plain text: `~` and the non-breaking space
/// become spaces, and any other markup in it appears as written. The span is
/// staged regardless of the mark text, so that the mark's anchor exists for
/// the link back to it; an empty mark text gives an empty span. Where the
/// xnote has no anchor (in a registry without the references feature, or for
/// a numbered xnote with no number and no label), there is no link, and the
/// span holds the mark text alone.
///
/// **Other cases.** An xnote written inside another xnote's content is an
/// xnote like any other: its mark is written where it is written, which is
/// where the other xnote's content is shown. A node for which the table holds
/// no record is written as nothing and reported as the Error
/// `flm.run.missing-entry`: a node of the document that the collect stage did
/// not reach, or an xnote in content that the run builds itself, apart from
/// the document's trees (for instance a list's own label template, whose
/// xnote is not one the document wrote at that place). A position `nth`
/// beyond the node's recorded xnotes of the kind is likewise written as
/// nothing and reported as `flm.run.missing-entry`. Content shown again (a
/// heading's title that a reference shows) holds no xnote, only the node that
/// represents it ([`MarkOf`](super::MarkOf)), which
/// [`MarkOfRule`](super::MarkOfRule) writes as the mark.
///
/// An xnote with no content is still written, and is reported as the Warning
/// [`Empty`](super::Empty).
///
/// # Errors
///
/// [`LowerError`] when `node` is not a node of the tree being lowered, when
/// the calling rule did not declare what it reads or produces, when `kind` is
/// not the kind the table records for `node`, and from the staging of the
/// nodes.
pub fn stage_mark(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    kind: &XnoteKind,
    nth: usize,
) -> Result<Vec<BuildId>, LowerError> {
    let Some(record) = find(lw, node, kind, nth)? else {
        report_missing(lw, node);
        return Ok(Vec::new());
    };
    report_empty(lw, node, kind, nth)?;
    let mark = Mark { anchored: true, linked: true };
    Ok(vec![write_mark(cx, lw, kind, &record, mark)?])
}

/// Stages (builds into the lowered tree, through `lw`; see [`Lowerer`]) one
/// xnote that the node `node` carries, of the xnote kind `kind`, in place: the shape an in-place kind writes where the xnote was written. It
/// also reports the problems of the xnote: [`Empty`](super::Empty) and
/// `flm.run.missing-entry`, in the cases that [`stage_mark`] lists.
///
/// **What the caller passes** is what [`stage_mark`] states, `nth` included:
/// an element carrying several xnotes of an in-place kind calls the helper
/// once for each, and each xnote is written in place with its own mark and
/// content. A position beyond the node's recorded xnotes of the kind is
/// written as nothing and reported as `flm.run.missing-entry`. `brackets`
/// says how the brackets around the xnote are written: as text
/// ([`Brackets::Text`], the general rule) or not at all
/// ([`Brackets::Drawn`], the HTML page, whose stylesheet draws them); nothing
/// else differs.
///
/// **What the calling rule must declare.** It must declare that it reads
/// [`XNOTES`] and that it produces `flm.pres.span`, `flm.pres.block` and
/// `flm.pres.par`; it may declare optional reads of [`COUNTERS`] and
/// [`ANCHORS`], as for [`stage_mark`].
///
/// **What it stages.** For an xnote that the table records and whose content
/// is inline, it stages
///
/// ```text
/// flm.pres.span { role: flm.xnotes.xnote.inplace, kinds: [xnotes, <kind>],
///                 anchor: <the xnote's anchor>, doc_role: Footnote }
///   ├ "["
///   ├ flm.pres.span { style: Superscript, role: flm.xnotes.xnote.mark,
///   │                 anchor: <the mark's anchor> }  [ the mark text ]
///   ├ the content, lowered through the rules
///   └ "]"
/// ```
///
/// The mark span holds no link, since the xnote is right beside it. Where the
/// xnote is a block (its content holds paragraphs, and its element's block
/// role makes it a block of its own), it stages instead a `flm.pres.block` of
/// the same role, kinds, anchor and document role, whose children are the
/// content's paragraphs and blocks lowered as they are, with `[` and the mark
/// span at the head of the first paragraph and `]` at the end of the last
/// ([`append_to_last_paragraph`]). A block that the block-structure pass kept
/// where it was written in an inline region (a heading's title, `\text{…}`,
/// the argument of `\textbf{…}`) is written in the inline shape, its
/// paragraphs' runs joined by a space: the block-structure pass has reported
/// it (`flm.blocks.block-in-inline`), and nothing more is reported.
///
/// An xnote inside another xnote's content is written in the same shapes,
/// where that content is shown; the other cases are the ones [`stage_mark`]
/// states.
///
/// # Errors
///
/// As [`stage_mark`], and from the lowering of the content.
pub fn stage_in_place(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    kind: &XnoteKind,
    nth: usize,
    brackets: Brackets,
) -> Result<Vec<BuildId>, LowerError> {
    let Some(record) = find(lw, node, kind, nth)? else {
        report_missing(lw, node);
        return Ok(Vec::new());
    };
    report_empty(lw, node, kind, nth)?;
    in_place(cx, lw, node, kind, nth, &record, brackets)
}

/// Stages (builds into the lowered tree, through `lw`; see [`Lowerer`]) the
/// mark text of the xnote of the xnote kind `kind` whose own
/// reference point is `point`: what a mark shows, in the running text and
/// beside the xnote's entry in a list alike.
///
/// For a numbered kind, the mark text is the xnote's own number (never the
/// parts of the counters it is numbered within), written in the kind's mark
/// style ([`mark_style_name`](super::mark_style_name), `#{arabic}` by
/// default; a declared counter style that a host replaces through the
/// numbering feature's configuration), without word and delimiters: `1`. It
/// is empty where the counter table is absent (a registry without the
/// numbering feature, which the collect stage reports) or holds no number for
/// the point (which the numbering feature reports). For an unnumbered kind,
/// the mark text is the content of the kind's [`mark`](XnoteKind::mark)
/// template, which holds no number placeholder: FLM text compiled when the
/// registry was built, with the compiler of a numbered kind's mark, and read
/// from the table of xnotes ([`Xnotes::fixed_mark`](super::Xnotes::fixed_mark)).
/// `\dag` writes what the macro `\dag` writes, `~*` a non-breaking space and
/// `*`, and the empty template nothing.
///
/// The calling rule must declare that it reads [`XNOTES`], and may declare an
/// optional read of the numbering feature's counter table [`COUNTERS`].
///
/// # Errors
///
/// [`LowerError`] when the calling rule did not declare its reads of
/// [`XNOTES`] and [`COUNTERS`], when the counter table holds no mark style
/// for a numbered kind, when the number cannot be written in the mark style,
/// and from the staging of the content.
pub fn stage_mark_text(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    kind: &XnoteKind,
    point: PointId,
) -> Result<Vec<BuildId>, LowerError> {
    if !kind.numbered {
        // See decision rationale at [§flm-dd-dr:std-xnote-fixed-mark].
        let Some(xnotes) = lw.get(XNOTES)? else { return Ok(Vec::new()) };
        let trees = xnotes.fixed_mark(kind.name.as_str()).to_vec();
        let mut staged = Vec::new();
        for tree in &trees {
            staged.extend(lw.splice(cx, tree)?);
        }
        return Ok(staged);
    }
    let segments = {
        let Some(counters) = lw.get(COUNTERS)? else { return Ok(Vec::new()) };
        let Some(record) = counters.record(point) else { return Ok(Vec::new()) };
        let name = mark_style_name(kind.name.as_str());
        let style = counters.style(&name).ok_or_else(|| {
            LowerError::structure(format!("the counter table carries no mark style `{name}`"))
        })?;
        // The own number alone: no parent, so no part of a counter the kind
        // is numbered within.
        // See decision rationale at [§flm-dd-dr:std-xnote-mark-own-number].
        let options = FormatOptions::new().without_word().without_delimiters();
        numbering::format(&record.value, style, None, &options)
            .map_err(|error| LowerError::structure(error.to_string()))?
    };
    let mut staged = Vec::new();
    for segment in &segments {
        staged.extend(lw.splice(cx, segment.content())?);
    }
    Ok(staged)
}

/// Lowers `content`, a run of sibling nodes holding paragraphs and blocks or
/// holding inline content, as blocks, with `appended` at the end of the last
/// paragraph's inline run, and returns the blocks.
///
/// This is how something is added after the last words of content that may
/// hold paragraphs: the closing bracket of an in-place xnote, or the link
/// back from an entry of a list of xnotes to its mark. `appended` holds nodes
/// that the caller staged and placed nowhere else.
///
/// - Inline content (no paragraph and no block among the nodes) becomes one
///   `flm.pres.par` holding the content lowered through the rules, then
///   `appended`.
/// - Content holding blocks is lowered block by block: each paragraph becomes
///   a `flm.pres.par` of its inline run lowered through the rules, with
///   `appended` at the end of the last one; every other block is lowered by
///   its rule. Content whose last block is not a paragraph (a list, for
///   example) gets one more `flm.pres.par`, holding `appended` alone.
/// - Empty content with nothing appended gives nothing.
///
/// The function is the lowering feature's
/// [`lower_blocks_with_edges`](crate::stdcore::backends::lowering::lower_blocks_with_edges)
/// with nothing at the head. The calling rule stages every paragraph through
/// [`lower_paragraph`](crate::stdcore::backends::lowering::lower_paragraph),
/// the only function that stages a paragraph, so a rule that replaces the
/// paragraph rule is not consulted for these paragraphs. The calling rule
/// must declare that it produces `flm.pres.par`.
pub fn append_to_last_paragraph(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    content: &[NodeRef<'_, Flm>],
    appended: Vec<BuildId>,
) -> Result<Vec<BuildId>, LowerError> {
    lower_blocks_with_edges(cx, lw, content, Vec::new(), appended)
}

/// Reports `flm.run.missing-entry` at `node`, a node for which the table of
/// xnotes holds no record, with the table, the stage that writes it and the
/// node's element.
pub(super) fn report_missing(lw: &mut dyn Lowerer, node: &NodeRef<'_, Flm>) {
    let table = CapName::from(XNOTES.name());
    let producer = lw.producer_of(&table).map(|stage| stage.to_string()).unwrap_or_else(|| "nobody".to_string());
    let element = node.ext().element.name.to_string();
    lw.error(MissingEntry { table: table.to_string(), producer, element }, node.span().clone());
}

/// Reports [`Empty`] for the `nth` xnote of the xnote kind `kind` that `node`
/// carries, when its content holds nothing.
pub(super) fn report_empty(
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    kind: &XnoteKind,
    nth: usize,
) -> Result<(), LowerError> {
    if Content::of(node, kind, nth).is_empty() {
        lw.warning(Empty { kind: kind.name.to_string() }, node.span().clone());
    }
    Ok(())
}

/// The anchors of the mark and of the xnote of `record`, from the anchors
/// table where the plan computes it.
///
/// # Errors
///
/// [`LowerError`] when the calling rule did not declare its read of
/// [`ANCHORS`].
pub(super) fn identifiers(
    lw: &dyn Lowerer,
    record: &XnoteRecord,
) -> Result<(Option<String>, Option<String>), LowerError> {
    let Some(anchors) = lw.get(ANCHORS)? else { return Ok((None, None)) };
    let id = |point| anchors.lookup(point).flatten().map(String::from);
    Ok((id(record.mark), id(record.point)))
}

/// The accessible name of the link from the mark of the xnote whose point is
/// `point` to the xnote: the kind's singular word (its capital variant, where
/// one is declared) with its non-breaking spaces made spaces, then the
/// xnote's number as a reference writes it without the word (`Footnote 1`);
/// the word alone where there is no number.
// The number a reference shows, not the mark's: the name identifies the
// xnote as a reference does, whatever style the mark is written in.
// See decision rationale at [§flm-dd-dr:std-xnote-link-label].
fn link_label(lw: &dyn Lowerer, kind: &XnoteKind, point: PointId) -> Result<String, LowerError> {
    let words = kind.words.variants.get(CAPITAL_VARIANT).unwrap_or(&kind.words.base);
    accessible_name(lw, kind, &words.one, point)
}

/// The accessible name of the xnote of the xnote kind `kind` whose point is
/// `point`, spelled with `word`: `word` with its non-breaking spaces made
/// spaces, then the xnote's number as a reference writes it without the word
/// (`Footnote 1`); the word alone where there is no number; the number alone
/// where the word is empty. Every link to or from an xnote takes its
/// accessible name from this function.
///
/// # Errors
///
/// [`LowerError`] when the calling rule did not declare its read of
/// [`COUNTERS`], and when the number cannot be written.
pub(super) fn accessible_name(
    lw: &dyn Lowerer,
    kind: &XnoteKind,
    word: &str,
    point: PointId,
) -> Result<String, LowerError> {
    let word = word.replace(['~', '\u{a0}'], " ");
    let word = word.trim();
    let number = if kind.numbered {
        match lw.get(COUNTERS)? {
            None => None,
            Some(counters) => counters
                .format(point, &FormatOptions::new().without_word().without_delimiters())
                .transpose()
                .map_err(|error| LowerError::structure(error.to_string()))?
                .map(|segments| segments.iter().map(|segment| plain_text(segment.content())).collect::<String>()),
        }
    } else {
        None
    };
    Ok(match number.as_deref().map(str::trim) {
        Some(number) if !number.is_empty() && !word.is_empty() => format!("{word} {number}"),
        Some(number) if !number.is_empty() => number.to_string(),
        _ => word.to_string(),
    })
}

/// How a mark is written, beyond its text: whether its span carries the
/// mark's anchor, and whether its text is a link to the xnote.
///
/// The mark written where the xnote is written carries the anchor, which the
/// entry of a listed xnote links back to; a mark written in content shown
/// again carries none, so that the anchor stays where the xnote is written
/// and the page's anchors stay unique. A listed xnote's mark links to the
/// xnote, and so does a mark in content shown again; the mark of an in-place
/// xnote written where the xnote is written does not, because its content is
/// written right beside it.
#[derive(Debug, Clone, Copy)]
pub(super) struct Mark {
    /// Whether the span carries the mark's anchor.
    pub(super) anchored: bool,
    /// Whether the mark text is a link to the xnote.
    pub(super) linked: bool,
}

/// Stages the mark of the xnote of the xnote kind `kind` whose record is
/// `record`, shaped as `mark` says: one
/// `flm.pres.span { style: Superscript, role: flm.xnotes.xnote.mark }`,
/// carrying the mark's anchor where `mark` is anchored and the plan computes
/// one, and holding the mark text ([`stage_mark_text`]), inside a
/// `flm.pres.link` to the xnote, with the document role `NoteRef` and the
/// accessible name that [`stage_mark`] documents, where `mark` is linked and
/// the xnote has an anchor. Every mark of an xnote in the running text is
/// staged by this function.
///
/// # Errors
///
/// [`LowerError`] when the calling rule did not declare what it reads or
/// produces, and from the staging of the mark text.
pub(super) fn write_mark(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    kind: &XnoteKind,
    record: &XnoteRecord,
    mark: Mark,
) -> Result<BuildId, LowerError> {
    let (mark_anchor, xnote_anchor) = identifiers(lw, record)?;
    let text = stage_mark_text(cx, lw, kind, record.point)?;
    let body = match xnote_anchor.filter(|_| mark.linked) {
        Some(anchor) => {
            let link = LinkPayload {
                href: format!("#{anchor}"),
                kind: Name::from(LINK_KIND),
                doc_role: Some(DocRole::NoteRef),
                label: Some(link_label(lw, kind, record.point)?),
                shows_target: false,
            };
            vec![lw.pres_with(cx, &ElementName::from(pres::LINK), link, text)?]
        }
        None => text,
    };
    let payload = SpanPayload {
        style: Some(SpanStyle::Superscript),
        role: Some(Name::from(MARK_ROLE)),
        anchor: mark_anchor.filter(|_| mark.anchored),
        ..SpanPayload::default()
    };
    lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)
}

/// Stages the `nth` xnote of the xnote kind `kind` that `node` carries,
/// recorded as `record`, in place, in the shape that [`stage_in_place`]
/// documents, with its brackets as `brackets` says.
fn in_place(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    kind: &XnoteKind,
    nth: usize,
    record: &XnoteRecord,
    brackets: Brackets,
) -> Result<Vec<BuildId>, LowerError> {
    let content = Content::of(node, kind, nth);
    let kinds = vec![Name::from(KIND_XNOTES), kind.name.clone()];
    let (_, anchor) = identifiers(lw, record)?;
    let mut leading = Vec::new();
    if brackets == Brackets::Text {
        leading.push(lw.text(cx, "[")?);
    }
    leading.push(write_mark(cx, lw, kind, record, Mark { anchored: true, linked: false })?);
    let trailing = match brackets {
        Brackets::Text => vec![lw.text(cx, "]")?],
        Brackets::Drawn => Vec::new(),
    };
    // The block the language layer made the xnote, where it stands among
    // blocks; the inline shape wherever else it stands.
    if matches!(block_role_of(node), BlockRole::Block) && stands_in_blocks(node, lw.layout()) {
        let children = lower_blocks_with_edges(cx, lw, content.nodes(), leading, trailing)?;
        let payload = BlockPayload {
            role: Some(Name::from(INPLACE_ROLE)),
            kinds,
            anchor,
            doc_role: Some(DocRole::Footnote),
        };
        return Ok(vec![lw.pres_with(cx, &ElementName::from(pres::BLOCK), payload, children)?]);
    }
    let run = if content.holds_blocks(node) {
        lower_joined(cx, lw, content.nodes())?
    } else {
        lower_run(cx, lw, content.nodes())?
    };
    let payload = SpanPayload {
        role: Some(Name::from(INPLACE_ROLE)),
        kinds,
        anchor,
        doc_role: Some(DocRole::Footnote),
        ..SpanPayload::default()
    };
    let body = [leading, run, trailing].concat();
    Ok(vec![lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)?])
}

/// Lowers `content`, a run of paragraphs and blocks, as inline content: each
/// paragraph's inline run, a space between two of them; every other node as
/// the rules lower it.
// See decision rationale at [§flm-dd-dr:std-xnote-flattened].
fn lower_joined(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    content: &[NodeRef<'_, Flm>],
) -> Result<Vec<BuildId>, LowerError> {
    let mut out = Vec::new();
    let mut after_block = false;
    for node in content {
        if node.is_comment() {
            out.extend(lower_run(cx, lw, std::slice::from_ref(node))?);
            continue;
        }
        if after_block {
            out.push(lw.text(cx, " ")?);
        }
        if is_paragraph(node) {
            out.extend(lw.lower_children(cx, node)?);
        } else {
            out.extend(lower_run(cx, lw, std::slice::from_ref(node))?);
        }
        after_block = true;
    }
    Ok(out)
}

/// Reports the first thing other than a `\label` that the optional argument
/// of the xnote `node` holds, as [`BadLabelArgument`], at the xnote, wherever
/// the xnote is lowered.
pub(super) fn report_labels(lw: &mut dyn Lowerer, node: &NodeRef<'_, Flm>, kind: &XnoteKind) {
    if let Some(first) = labels_of(node).1.into_iter().next() {
        lw.error(BadLabelArgument { kind: kind.name.to_string(), found: first.found }, node.span().clone());
    }
}

/// Lowers the xnote `node` of the element [`XNOTE`](super::XNOTE): reports
/// the refused content of its optional argument, then stages its mark
/// ([`stage_mark`]) for a listed xnote kind or the xnote in place
/// ([`stage_in_place`]) for an in-place one, with its brackets as `brackets`
/// says. The general rule and the HTML rule of the element call it, so that
/// what they write differs in the brackets alone, and so does the element's
/// rule on the `latex-rendered` backend
/// ([`XnoteRenderedRule`](super::latex_rendered::XnoteRenderedRule)) for a
/// listed xnote kind.
///
/// It first requires the feature's style identifier [`STYLE`]
/// (`flm.xnotes.style`), which each backend resolves as it resolves
/// requirements: the stylesheet on the `html` backend, the definition pieces
/// delivering it on the `latex-rendered` backend, nothing on the `text`
/// backend.
pub(super) fn lower_xnote(
    node: &NodeRef<'_, Flm>,
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    brackets: Brackets,
) -> Result<Lowering, LowerError> {
    lw.require(&Name::from(STYLE));
    let value = element_value::<XnoteElement>(node)
        .ok_or_else(|| LowerError::structure("an xnote without its element's data"))?;
    let kind = Arc::clone(&value.kind);
    report_labels(lw, node, &kind);
    // The element carries one xnote, the first of its kind.
    let staged = match kind.placement {
        XnotePlacement::List => stage_mark(cx, lw, node, &kind, 0)?,
        XnotePlacement::InPlace => stage_in_place(cx, lw, node, &kind, 0, brackets)?,
    };
    Ok(Lowering::Emit(staged))
}

/// The general lowering rule of the element [`XNOTE`](super::XNOTE),
/// [`XNOTE_RULE`](super::XNOTE_RULE), marked default on every backend.
///
/// It reports the refused content of the xnote's optional argument
/// ([`BadLabelArgument`]), then writes the xnote's mark, as [`stage_mark`]
/// stages it, for a listed xnote kind, and the xnote in place, as
/// [`stage_in_place`] stages it with the brackets written as text, for an
/// in-place kind. The plain-text backend uses this rule; the `html` backend
/// uses [`XnoteHtmlRule`](super::html::XnoteHtmlRule), the
/// `latex-recomposed` backend [`XnoteLatexRule`](super::latex::XnoteLatexRule)
/// and the `latex-rendered` backend
/// [`XnoteRenderedRule`](super::latex_rendered::XnoteRenderedRule).
#[derive(Debug, Default)]
pub struct XnoteRule;

impl Lower for XnoteRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lower_xnote(&node, cx, lw, Brackets::Text)
    }
}

/// The lowering rule of the element [`ENTRY`](super::ENTRY),
/// [`ENTRY_RULE`](super::ENTRY_RULE), marked default on every backend.
///
/// The rule of a list lowers the content of its entries itself and never
/// lowers an entry, so an entry that reaches a lowering of its own was not
/// written by its list: this rule writes nothing for it and reports it as
/// [`EntryOutsideList`](super::EntryOutsideList).
#[derive(Debug, Default)]
pub struct EntryRule;

impl Lower for EntryRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lw.error(EntryOutsideList, node.span().clone());
        Ok(Lowering::Emit(Vec::new()))
    }
}
