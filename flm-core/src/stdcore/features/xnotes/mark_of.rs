//! An xnote in content shown again: the element [`MARK_OF`] of the node that
//! represents the xnote in the copy, its payload [`MarkOf`], and its general
//! lowering rule [`MarkOfRule`].
//!
//! Content of the document that holds an xnote is shown a second time, away
//! from where it is written, when a reference prints a heading's title, an
//! unnumbered float's caption or a defined term, or a list item's tag. The
//! xnote there is not a second xnote: its number, its labels and its place in
//! its kind's list are the ones it has where it was written. So when such
//! content is copied to be shown again, the xnote's element returns, through
//! its [`InCopy`] aspect, which the copy consults, a new node that represents
//! the xnote: a node of the element [`MARK_OF`], with no children,
//! carrying a [`MarkOf`] payload that identifies the node carrying the
//! xnote. Nothing of the xnote itself is copied: no labels, no content. A
//! node carrying several xnotes becomes one node of the copy that represents
//! all of them, written as their marks side by side, in the node's order.
//! Another feature's element that carries xnotes returns the same node,
//! through [`copied_as`]. A copy made for content that a [synthesized
//! value](crate::guide::concepts_overview#synthesized-values) holds as its
//! own ([`ValueContent::Own`]: the use site of a user definition copied to be
//! expanded in place, content copied only to be read) holds the xnote as
//! written.
//!
//! The general rule writes that node as the xnote's mark: the superscript mark
//! text in the kind's mark style, inside a link to the xnote that carries the
//! accessible name (`Footnote 1`), as the xnote's own mark is written where
//! the xnote is written ([`stage_mark`](super::stage_mark)), with one
//! difference: it carries **no anchor**. The mark's anchor is the one where
//! the xnote is written, which the xnote's entry in its list links back to,
//! and a page holds each anchor once. The mark of an in-place xnote kind is
//! written in the same way, as a link to the xnote where it was written.
//! Content shown again is usually inside a link of its own (a reference is a
//! link to its target), so the mark's link is then inside another link; a
//! backend writes that as it writes any link inside a link (the HTML page
//! writes the inner one as its text). Nothing is added to any list, and
//! nothing is reported: the xnote reports its problems where it was written.
//!
//! On the recomposed LaTeX the node writes nothing
//! ([`MarkOfLatexRule`](super::latex::MarkOfLatexRule)), because no rule of
//! the standard package lowers content shown again on that backend.

use std::sync::{Arc, OnceLock};

use techy::core::node::NodeRef;
use techy::serialize::SerialValue;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::lang::copy::ValueContent;
use crate::lang::mint::FlmSynthesizedSpec;
use crate::lang::{Flm, FlmSpan, PresPayload};
use crate::model::aspects::{CopiedAs, InCopy, StandIn};
use crate::model::element::ElementSpec;
use crate::stdcore::backends::pres;
use crate::stdcore::features::numbering::COUNTERS;
use crate::stdcore::features::refs;

use super::config::XnoteKind;
use super::lower::{report_missing, write_mark, Mark};
use super::stages::{XnoteRecord, XNOTES};
use super::{XnoteElement, MARK_OF, MARK_OF_RULE};

/// The payload of a node of the element [`MARK_OF`]: the xnotes that the
/// node represents in content shown again, which are every xnote of the given
/// xnote kinds that the original node (the node the copy was made from)
/// carries.
///
/// The payload identifies the xnotes by their kinds and by the source span of
/// their node (the source and the range where it was written), which the copy
/// keeps for every node it copies: the rule finds the xnotes' records among
/// the kinds' records written at that span
/// ([`Xnotes::records_written_at`](super::Xnotes::records_written_at)). A node
/// of a copy has no address among the document's, so the span is what
/// identifies the original node.
#[derive(Debug, Clone)]
pub struct MarkOf {
    /// The xnote kinds of the xnotes that the node represents: the kinds of
    /// the xnotes its original node carries, each once.
    pub kinds: Vec<Arc<XnoteKind>>,
    /// The source span of the xnotes' node, where it was written.
    pub original: FlmSpan,
}

impl PresPayload for MarkOf {
    /// `{kinds: [<each kind's name>], original: "@<start>-<end>"}`: the kinds,
    /// and the byte range where the xnotes' node was written.
    fn project(&self) -> SerialValue {
        SerialValue::Map(vec![
            (
                String::from("kinds"),
                SerialValue::List(
                    self.kinds.iter().map(|kind| SerialValue::Str(kind.name.to_string())).collect(),
                ),
            ),
            (
                String::from("original"),
                SerialValue::Str(format!("@{}-{}", self.original.start(), self.original.end())),
            ),
        ])
    }
}

impl MarkOf {
    /// The payload of the node `node`, or `None` for a node that carries no
    /// payload or a payload of another type.
    pub fn of<'n>(node: &'n NodeRef<'_, Flm>) -> Option<&'n MarkOf> {
        node.ext().payload.as_deref()?.downcast_ref::<MarkOf>()
    }
}

/// What the element of `node`, a node carrying xnotes of the xnote kinds
/// `kinds`, returns from its [`InCopy`] aspect, for a copy made for
/// `purpose`: in content copied to be shown again ([`ValueContent::Shown`]),
/// one node of the element [`MARK_OF`] that represents every xnote of those
/// kinds the node carries ([`MarkOf`]), written as their marks; in a copy of
/// content that a synthesized value holds as its own ([`ValueContent::Own`]),
/// the node copied as written.
///
/// The element of `\footnote` passes its one xnote kind; another feature's
/// element carrying xnotes passes the kinds of its xnotes, each once, so that
/// its xnotes are shown in a copy as those of `\footnote` are. The element
/// [`MARK_OF`] belongs to the `xnotes` feature: in a registry without that
/// feature, no rule writes the node. Its element specification is a single
/// one, shared by every registry, because an element's [`InCopy`] aspect has
/// no access to a registry to take it from; the specification holds nothing a
/// registry configures, and would have to become each registry's own if it
/// came to depend on a configuration.
pub fn copied_as(node: &NodeRef<'_, Flm>, purpose: ValueContent, kinds: Vec<Arc<XnoteKind>>) -> CopiedAs {
    match purpose {
        ValueContent::Own => CopiedAs::Verbatim,
        ValueContent::Shown => {
            let payload = MarkOf { kinds, original: node.span().clone() };
            CopiedAs::StandIn(StandIn { spec: Arc::clone(mark_of_spec()), payload: Some(Arc::new(payload)) })
        }
    }
}

/// In content copied to be shown again, an xnote becomes one node of the
/// element [`MARK_OF`] that identifies it ([`MarkOf`]), written as its mark;
/// in a copy of content that a synthesized value holds as its own
/// ([`ValueContent::Own`]), it is copied as written ([`copied_as`]).
impl InCopy for XnoteElement {
    fn in_copy(&self, node: &NodeRef<'_, Flm>, purpose: ValueContent) -> CopiedAs {
        copied_as(node, purpose, vec![Arc::clone(&self.kind)])
    }
}

/// The specification of the nodes that represent xnotes in content shown
/// again, and its element, the one that the feature registers as
/// [`MARK_OF`]: inline content, keeping the whitespace around it as the copy
/// holds it. It is the same for every registry, since it holds nothing a
/// registry configures.
pub(super) fn mark_of_spec() -> &'static Arc<FlmSynthesizedSpec> {
    static SPEC: OnceLock<Arc<FlmSynthesizedSpec>> = OnceLock::new();
    SPEC.get_or_init(|| {
        let element = ElementSpec::new(MARK_OF)
            .docs("The mark of an xnote, written in its place in content shown again.");
        Arc::new(FlmSynthesizedSpec::new(Arc::new(element)))
    })
}

/// The general rule of the element [`MARK_OF`], [`MARK_OF_RULE`], marked
/// default on every backend; the recomposed LaTeX uses a rule of its own that
/// writes nothing.
pub(super) fn mark_of_rule() -> LoweringRule {
    LoweringRule::new(
        MARK_OF_RULE,
        MARK_OF,
        LoweringDecl::new()
            .reads(XNOTES)
            .reads_opt(COUNTERS)
            .reads_opt(refs::ANCHORS)
            .produces(pres::SPAN)
            .produces(pres::LINK),
        Arc::new(MarkOfRule),
        DefaultMark::Everywhere,
    )
    .docs(
        "an xnote in content shown again lowers to the xnote's mark, a superscript \
         `flm.pres.span` holding the mark text inside a `flm.pres.link` to the xnote, with no \
         anchor",
    )
}

/// The general lowering rule of the element [`MARK_OF`], [`MARK_OF_RULE`]:
/// the marks of the xnotes that the node represents ([`MarkOf`]).
///
/// Each mark is one `flm.pres.span { style: Superscript, role:
/// flm.xnotes.xnote.mark }` holding the mark text
/// ([`stage_mark_text`](super::stage_mark_text)) inside a `flm.pres.link` to
/// the xnote, as the xnote's own mark is written where the xnote is written
/// ([`stage_mark`](super::stage_mark)) but with no anchor, regardless of the
/// kind's placement: the mark's anchor is the one where the xnote is written.
///
/// The rule finds the xnotes among their kinds' records by the span where
/// their node was written: every record of those kinds written there, each
/// written as its mark, side by side, in the order of the node's points (the
/// order in which the node's element returns its xnotes). Where the table of
/// xnotes holds none, the rule writes nothing and reports
/// `flm.run.missing-entry`, as the xnote's own rule does. It reports nothing
/// else: the problems of an xnote (an empty content, a refused optional
/// argument) are reported where it was written.
#[derive(Debug, Default, Clone, Copy)]
pub struct MarkOfRule;

impl Lower for MarkOfRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let mark_of =
            MarkOf::of(&node).ok_or_else(|| LowerError::structure("a mark of an xnote without its payload"))?;
        let mut found: Vec<(Arc<XnoteKind>, XnoteRecord)> = match lw.get(XNOTES)? {
            None => Vec::new(),
            Some(xnotes) => mark_of
                .kinds
                .iter()
                .flat_map(|kind| {
                    xnotes
                        .records_written_at(kind.name.as_str(), &mark_of.original)
                        .map(|record| (Arc::clone(kind), record.clone()))
                })
                .collect(),
        };
        if found.is_empty() {
            report_missing(lw, &node);
            return Ok(Lowering::Emit(Vec::new()));
        }
        // Every xnote of the kinds written at the span, in the node's order.
        // See decision rationale at [§flm-dd-dr:std-xnote-mark-of-several].
        found.sort_by_key(|(_, record)| record.point);
        let mut marks = Vec::with_capacity(found.len());
        for (kind, record) in &found {
            // No identifier, which is the mark's where the xnote stands.
            marks.push(write_mark(cx, lw, kind, record, Mark { anchored: false, linked: true })?);
        }
        Ok(Lowering::Emit(marks))
    }
}
