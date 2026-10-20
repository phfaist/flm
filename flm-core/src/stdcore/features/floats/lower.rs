//! The general lowering rule of a float, [`FLOAT_RULE`], the default on every
//! backend: it lowers a float to a block holding the float's content and,
//! where the float is numbered or has a caption, the line under the content
//! (`Figure 1: A photo`).
//!
//! A lowering rule ([`LoweringRule`]) is a named registry entity written for
//! one element, which stages the element's presentation for one backend: to
//! stage a node is to build it into the tree that the rule is producing
//! ([`Lowerer`]). The plain-text output uses this rule; the `html` backend
//! uses a rule that lowers a float in the same way and requires the feature's
//! stylesheet ([`html`](super::html)); the `latex-recomposed` backend has a
//! rule of its own ([`latex`](super::latex)). [`float_decl`] gives what every
//! rule lowering a float declares, and [`float_number`] writes a float's
//! number.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::lang::Flm;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::lowering::{lower_paragraph, lower_run};
use crate::stdcore::backends::pres::{self, BlockPayload, SpanPayload};
use crate::stdcore::features::numbering::{COUNTERS, FormatOptions, NumberSegment};
use crate::stdcore::features::refs::ANCHORS;

use super::body::{FloatFields, is_float_field, is_paragraph, read_float};
use super::content::Handlers;
use super::elements::float_point;
use super::html::STYLE;
use super::{
    CAPTION_ROLE, CAPTION_TEXT_ARG, CHECK_HANDLERS, CONTENT_ROLE, FLOAT, FLOAT_ROLE, NUMBERED_KIND,
    NUMBER_ROLE, UNNUMBERED_KIND,
};

/// The name of the float's general rule, `flm.floats.float.lower.default`,
/// whose body is [`FloatRule`].
pub const FLOAT_RULE: &str = "flm.floats.float.lower.default";

/// The body of [`FLOAT_RULE`].
///
/// A role and its kind tokens are feature-owned names on a presentation
/// payload that say what a container is, so that a stylesheet or a host's
/// lowering rule can select on them
/// ([`pres::payload`](crate::stdcore::backends::pres::payload)). A float
/// lowers to one `flm.pres.block` of role [`FLOAT_ROLE`] (`flm.floats.float`),
/// whose kind tokens are the float type's name and [`NUMBERED_KIND`] or [`UNNUMBERED_KIND`], which carries the
/// anchor of the float's reference point where the references feature
/// computed one, and which holds, in this order regardless of the order in
/// the source:
///
/// - a `flm.pres.block` of role [`CONTENT_ROLE`]
///   (`flm.floats.float.content`): the float's content, lowered; a paragraph
///   that holds `\caption`, `\label` or `\nonumber` beside other content is
///   lowered without those commands;
/// - where the float is numbered or has a caption, a `flm.pres.block` of role
///   [`CAPTION_ROLE`] (`flm.floats.float.caption`), with the same kind tokens
///   as the float's block, holding one paragraph, **the line under the
///   content**. For a numbered float, the paragraph holds a `flm.pres.span` of
///   role [`NUMBER_ROLE`] with the type's caption name, a no-break space and
///   the number, followed, where the float has a caption, by `: ` and the
///   caption's full content (`Figure 1: A photo`). For an unnumbered float, it
///   holds the caption alone.
///
/// The float is **numbered** where its type's numbering policy numbers it,
/// given its `\label` and `\nonumber`, and the counter table holds its
/// number ([`float_number`]): the number is written from the table when the
/// float is lowered, in the type's counter style without its words. A float
/// that the run could not number, because the numbering feature is not
/// registered, is unnumbered; [`read_float`] reports it.
///
/// [`read_float`], which every rule lowering a float calls, reports every
/// problem of the float's body.
///
/// At every float the rule requires the feature's style identifier
/// [`STYLE`] (`flm.floats.style`), which each backend resolves as it
/// resolves requirements: the stylesheet on the `html` backend, the
/// definition pieces delivering it on the `latex-rendered` backend, nothing
/// on the `text` backend.
#[derive(Debug)]
pub struct FloatRule {
    handlers: Arc<Handlers>,
}

impl FloatRule {
    /// Returns the rule's body, which tests a float's content against the
    /// content handlers `handlers`.
    pub(super) fn new(handlers: Arc<Handlers>) -> FloatRule {
        FloatRule { handlers }
    }
}

/// Returns the number of the float whose reference point is `point`, written
/// from the counter table ([`COUNTERS`]) in the type's counter style, without
/// the words before it and without delimiters. Returns `None` where the table
/// holds no number for the point, and where the numbering feature is not
/// registered. The calling rule must declare a read of the counter table
/// ([`float_decl`] declares an optional one).
///
/// # Errors
///
/// [`LowerError`] when the table cannot be read, and when the number cannot
/// be written from its record, which does not happen in a correct run, since
/// the numbering stage refuses a record that its counter style cannot
/// write.
pub fn float_number(
    lw: &dyn Lowerer,
    point: crate::model::aspects::PointId,
) -> Result<Option<Vec<NumberSegment>>, LowerError> {
    let Some(counters) = lw.get(COUNTERS)? else { return Ok(None) };
    let options = FormatOptions::new().without_word().without_delimiters();
    match counters.format(point, &options) {
        None => Ok(None),
        Some(written) => written.map(Some).map_err(|error| LowerError::structure(error.to_string())),
    }
}

/// Lowers `content`, the float's content nodes, in order: a paragraph that
/// holds one of the float's three commands as a paragraph of the rest of its
/// content, and every other node through the rules.
fn lower_content(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    content: &[NodeRef<'_, Flm>],
) -> Result<Vec<BuildId>, LowerError> {
    let mut out = Vec::new();
    for node in content {
        if is_paragraph(node) && node.children().iter().any(|child| is_float_field(&child)) {
            let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
            let mut run = Vec::new();
            for piece in children.split(is_float_field) {
                run.extend(lower_run(cx, lw, piece)?);
            }
            out.push(lower_paragraph(cx, lw, None, run, Vec::new())?);
        } else {
            out.extend(lower_run(cx, lw, std::slice::from_ref(node))?);
        }
    }
    Ok(out)
}

/// Stages a `flm.pres.block` of role `role`, with the kind tokens `kinds` and
/// the anchor `anchor`, holding `body`.
fn block(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    role: &str,
    kinds: &[Name],
    anchor: Option<String>,
    body: Vec<BuildId>,
) -> Result<BuildId, LowerError> {
    let payload = BlockPayload { role: Some(Name::from(role)), kinds: kinds.to_vec(), anchor, doc_role: None };
    lw.pres_with(cx, &ElementName::from(pres::BLOCK), payload, body)
}

/// Stages the paragraph of the line under the content: the number's span and
/// the caption, or either alone. Returns `None` where the float has neither.
fn line(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    fields: &FloatFields<'_>,
    number: Option<&[NumberSegment]>,
) -> Result<Option<BuildId>, LowerError> {
    let mut run = Vec::new();
    if let Some(segments) = number {
        let mut body = vec![lw.text(cx, &format!("{}\u{a0}", fields.float_type.caption_name))?];
        for segment in segments {
            body.extend(lw.splice(cx, segment.content())?);
        }
        let payload = SpanPayload { role: Some(Name::from(NUMBER_ROLE)), ..SpanPayload::default() };
        run.push(lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)?);
    }
    if let Some(caption) = &fields.caption {
        if number.is_some() {
            run.push(lw.text(cx, ": ")?);
        }
        run.extend(lw.lower_arg_named(cx, &caption.node, CAPTION_TEXT_ARG)?);
    } else if number.is_none() {
        return Ok(None);
    }
    Ok(Some(lower_paragraph(cx, lw, None, run, Vec::new())?))
}

impl Lower for FloatRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lw.require(&Name::from(STYLE));
        let reading = read_float(&node, &self.handlers, lw)?;
        let fields = &reading.fields;
        let point = float_point(lw.addr());
        // The anchors table is absent in a registry holding no references
        // feature: the float is written with no anchor.
        let anchor = lw.get(ANCHORS)?.and_then(|anchors| anchors.id(point)).map(String::from);
        let number = if fields.numbered { float_number(lw, point)? } else { None };
        let numbered = if number.is_some() { NUMBERED_KIND } else { UNNUMBERED_KIND };
        let kinds = [Name::from(fields.float_type.name.as_str()), Name::from(numbered)];
        let content = lower_content(cx, lw, &fields.body.content)?;
        let mut children = vec![block(cx, lw, CONTENT_ROLE, &[], None, content)?];
        if let Some(line) = line(cx, lw, fields, number.as_deref())? {
            children.push(block(cx, lw, CAPTION_ROLE, &kinds, None, vec![line])?);
        }
        let float = block(cx, lw, FLOAT_ROLE, &kinds, anchor, children)?;
        Ok(Lowering::Emit(vec![float]))
    }
}

/// Returns what a rule that lowers a float declares: optional reads of the
/// anchors table ([`ANCHORS`]) and of the counter table ([`COUNTERS`]), which
/// [`read_float`] needs, the requirement of the stage
/// [`CHECK_HANDLERS`](super::CHECK_HANDLERS), and the production of blocks,
/// paragraphs and spans. The general rule and the `html` backend's rule
/// declare exactly this.
pub fn float_decl() -> LoweringDecl {
    LoweringDecl::new()
        .reads_opt(ANCHORS)
        .reads_opt(COUNTERS)
        .requires(CHECK_HANDLERS)
        .produces(pres::BLOCK)
        .produces(pres::PAR)
        .produces(pres::SPAN)
}

/// Returns the float's general rule, [`FLOAT_RULE`], the default on every
/// backend, which tests a float's content against the content handlers
/// `handlers`.
pub(super) fn float_rule(handlers: Arc<Handlers>) -> LoweringRule {
    LoweringRule::new(
        FLOAT_RULE,
        FLOAT,
        float_decl(),
        Arc::new(FloatRule::new(handlers)),
        DefaultMark::Everywhere,
    )
    .docs(
        "a float lowers to a `flm.pres.block` of role `flm.floats.float`, whose kind tokens are \
         the float type's name and `numbered` or `unnumbered` and which carries the float's \
         anchor; it holds a block of role `flm.floats.float.content` with the float's content \
         and, where the float is numbered or has a caption, a block of role \
         `flm.floats.float.caption` with the line under the content",
    )
}
