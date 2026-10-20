//! The lowering rules of the `annotations` feature that lower an annotation
//! into the presentation vocabulary: the general rule, the default on every
//! backend that has no rule of its own, and the `text` backend's rule.
//!
//! **The general rule** ([`RULE`](super::RULE), [`AnnotationRule`]) lowers an
//! annotation, a node of an annotator's command or a cut part of one, to a
//! `flm.pres.span` around inline content, or to a `flm.pres.block` around the
//! one block that a cut part holds where the cut part is placed among blocks.
//! The span or block has the role
//! [`HIGHLIGHT_ROLE`](super::HIGHLIGHT_ROLE) or
//! [`COMMENT_ROLE`](super::COMMENT_ROLE), according to the annotation's
//! [form](super::form_of), and the two kind tokens `<command>` and
//! `annotator-<n>`:
//!
//! ```text
//! flm.pres.span { role: flm.annotations.highlight, kinds: [phf, annotator-0] }
//!   ├ flm.pres.deco.margin
//!   │   └ flm.pres.span { role: flm.annotations.initials, kinds: [phf, annotator-0] }
//!   │       └ "PhF"
//!   └ the passage, lowered
//! ```
//!
//! A **cut part** of a cut annotation also carries the kind token
//! `mark-continues` when it is not the last cut part, and `mark-continued`
//! when it is not the first
//! ([`MARK_CONTINUES_KIND`](super::MARK_CONTINUES_KIND),
//! [`MARK_CONTINUED_KIND`](super::MARK_CONTINUED_KIND)), for a highlight and
//! a comment alike; the initials' span does not carry them. A comment cut in
//! two:
//!
//! ```text
//! flm.pres.span { role: flm.annotations.comment, kinds: [phf, annotator-0, mark-continues] }
//!   ├ flm.pres.deco.margin …
//!   └ the first paragraph's part of the remark
//! flm.pres.span { role: flm.annotations.comment, kinds: [phf, annotator-0, mark-continued] }
//!   └ the second paragraph's part of the remark
//! ```
//!
//! The margin decoration that holds the **initials** is the first node of
//! the span's or the block's own content. The lowering driver therefore
//! moves it into the node's attached slot and leaves its decoration mark at
//! the start of the content, where the annotation starts
//! ([decorations](crate::guide::concepts_overview#decorations-roles-and-kinds)).
//! The rule stages it when the annotator has initials and the node is a first
//! cut part: a node that was not cut, or a cut part whose record says it is
//! the first. Every cut part carries the annotator's kind tokens, so that an
//! output colors every cut part alike.
//!
//! **The text backend's rule** ([`TEXT_RULE`](super::TEXT_RULE),
//! [`AnnotationTextRule`]) lowers a highlight as the general rule does, and a
//! comment with its brackets written as text and no decoration: `[`, the
//! initials and `: ` before the remark, and `]` after it: `[PhF: fix this]`,
//! or `[fix this]` for an annotator with no initials. A cut comment opens its
//! bracket on its first cut part and closes it on its last.
//!
//! **Hidden.** When [`AnnotationsRenderOptions::hidden`] is true, both rules stage
//! nothing of the annotation's own: a highlight lowers to its passage's
//! lowered nodes, and a comment to nothing.
//!
//! **The style identifier.** Unless annotations are hidden, both rules
//! require the feature's style identifier [`STYLE`]
//! (`flm.annotations.style`) at every annotation they write; each backend
//! resolves it as it resolves requirements (the stylesheet on the `html`
//! backend, the definition pieces delivering it on the `latex-rendered`
//! backend, nothing on the `text` backend).

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl};
use crate::lang::rewrites::blocks::stands_in_blocks;
use crate::lang::Flm;
use crate::model::aspects::{block_role_of, BlockRole};
use crate::model::element::element_value;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::pres::{self, BlockPayload, DecorationPayload, SpanPayload};
use crate::stdcore::backends::pres::read::payload_of;

use super::config::AnnotationsRenderOptions;
use super::cut::AnnotationPiece;
use super::html::STYLE;
use super::parser::{form_of, Form};
use super::{
    annotator_kind, AnnotationElement, COMMENT_ROLE, MARK_CONTINUED_KIND, MARK_CONTINUES_KIND, HIGHLIGHT_ROLE, INITIALS_ROLE,
    TEXT_ARG,
};

/// How a comment's brackets are written: the only difference between the
/// general rule and the text backend's rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Brackets {
    /// No bracket: the initials are a margin decoration. The general rule.
    None,
    /// `[`, the initials and `: ` as text before the remark, `]` after it,
    /// and no decoration. The text backend's rule.
    Text,
}

/// Returns the declaration shared by the rules that lower an annotation into
/// the presentation vocabulary: they read the render option
/// [`AnnotationsRenderOptions`], and produce the span, the block and the
/// margin decoration.
pub(super) fn decl() -> LoweringDecl {
    LoweringDecl::new()
        .option::<AnnotationsRenderOptions>()
        .produces(pres::SPAN)
        .produces(pres::BLOCK)
        .produces(pres::DECO_MARGIN)
}

/// The implementation of the general rule [`RULE`](super::RULE), the default
/// on every backend that has no rule of its own for an annotation.
///
/// It lowers an annotation, a node of an annotator's command or a cut part
/// of one, to a `flm.pres.span` around inline content, or to a
/// `flm.pres.block` around the one block that a cut part holds where the cut
/// part is placed among blocks. The span or block has the role
/// [`HIGHLIGHT_ROLE`] or [`COMMENT_ROLE`], according to the annotation's
/// [form](form_of), and the two kind tokens `<command>` and `annotator-<n>`:
///
/// ```text
/// flm.pres.span { role: flm.annotations.highlight, kinds: [phf, annotator-0] }
///   ├ flm.pres.deco.margin
///   │   └ flm.pres.span { role: flm.annotations.initials, kinds: [phf, annotator-0] }
///   │       └ "PhF"
///   └ the passage, lowered
/// ```
///
/// A cut part of a cut annotation also carries the kind token
/// [`MARK_CONTINUES_KIND`](super::MARK_CONTINUES_KIND) when it is not the
/// last cut part, and [`MARK_CONTINUED_KIND`](super::MARK_CONTINUED_KIND)
/// when it is not the first, so that a stylesheet can open a comment's
/// brackets on its first cut part and close them on its last; a node that
/// was not cut carries neither.
///
/// The margin decoration that holds the **initials** is the first node of the
/// span's or the block's own content, so that the lowering driver moves it
/// into the node's attached slot and leaves its decoration mark at the start
/// of the content, where the annotation starts. The rule stages it when the
/// annotator has initials and the node is a first cut part: a node that was
/// not cut, or a cut part whose [record](super::AnnotationPiece) says it is
/// the first. Every cut part carries the annotator's kind tokens, so that an
/// output shows every cut part alike.
///
/// When [`AnnotationsRenderOptions::hidden`] is true, it stages nothing of the
/// annotation's own: a highlight lowers to its passage's lowered nodes, and a
/// comment to nothing.
///
/// # Errors
///
/// [`LowerError`] when the node carries no [`AnnotationElement`] or no
/// argument group, and
/// any error from reading the render option or lowering the content.
#[derive(Debug, Default, Clone, Copy)]
pub struct AnnotationRule;

impl Lower for AnnotationRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lower_annotation(&node, cx, lw, Brackets::None)
    }
}

/// The implementation of the text backend's rule,
/// [`TEXT_RULE`](super::TEXT_RULE), the default on that backend only.
///
/// It lowers a highlight as [`AnnotationRule`] does, and a comment to the
/// same span or block, with its brackets written as text and no decoration:
/// `[`, the initials and `: ` before the remark, and `]` after it:
/// `[PhF: fix this]`, or `[fix this]` for an annotator with no initials. A
/// cut comment opens its bracket on its first cut part and closes it on its
/// last; the cut parts between them have no bracket. When
/// [`AnnotationsRenderOptions::hidden`] is true, it stages what
/// [`AnnotationRule`] stages.
///
/// # Errors
///
/// As [`AnnotationRule`].
#[derive(Debug, Default, Clone, Copy)]
pub struct AnnotationTextRule;

impl Lower for AnnotationTextRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lower_annotation(&node, cx, lw, Brackets::Text)
    }
}

/// Lowers the annotation `node`, writing a comment's brackets as `brackets`
/// says. Both rules of this module call it, so that what they write differs
/// only in a comment's brackets.
fn lower_annotation(
    node: &NodeRef<'_, Flm>,
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    brackets: Brackets,
) -> Result<Lowering, LowerError> {
    let value = element_value::<AnnotationElement>(node)
        .ok_or_else(|| LowerError::structure("an annotation without its element's data"))?;
    let form = form_of(node).ok_or_else(|| LowerError::structure("an annotation without its argument"))?;
    let hidden = lw.option::<AnnotationsRenderOptions>()?.hidden;
    if hidden {
        return Ok(Lowering::Emit(match form {
            Form::Highlight => lw.lower_arg_named(cx, node, TEXT_ARG)?,
            Form::Comment => Vec::new(),
        }));
    }
    lw.require(&Name::from(STYLE));
    let record = payload_of::<AnnotationPiece>(node).copied();
    let first = record.is_none_or(|piece| piece.is_first());
    let last = record.is_none_or(|piece| piece.is_last());
    let kinds = vec![Name::from(value.annotator.command.as_str()), annotator_kind(value.number)];
    // The piece's own kinds: the annotator's, then where the piece stands in
    // the cut annotation; the initials' span carries the annotator's alone.
    let mut piece_kinds = kinds.clone();
    if !last {
        piece_kinds.push(Name::from(MARK_CONTINUES_KIND));
    }
    if !first {
        piece_kinds.push(Name::from(MARK_CONTINUED_KIND));
    }
    let initials = if first { value.annotator.initials.as_deref() } else { None };
    let mut body = Vec::new();
    let mut trailing = Vec::new();
    match (form, brackets) {
        (Form::Comment, Brackets::Text) => {
            if first {
                let opening = match initials {
                    Some(initials) => format!("[{initials}: "),
                    None => String::from("["),
                };
                body.push(lw.text(cx, &opening)?);
            }
            if last {
                trailing.push(lw.text(cx, "]")?);
            }
        }
        (Form::Highlight, _) | (Form::Comment, Brackets::None) => {
            if let Some(initials) = initials {
                body.push(stage_initials(cx, lw, initials, &kinds)?);
            }
        }
    }
    body.extend(lw.lower_arg_named(cx, node, TEXT_ARG)?);
    body.extend(trailing);
    let role = Some(Name::from(match form {
        Form::Highlight => HIGHLIGHT_ROLE,
        Form::Comment => COMMENT_ROLE,
    }));
    let staged = if matches!(block_role_of(node), BlockRole::Block) && stands_in_blocks(node, lw.layout()) {
        let payload = BlockPayload { role, kinds: piece_kinds, anchor: None, doc_role: None };
        lw.pres_with(cx, &ElementName::from(pres::BLOCK), payload, body)?
    } else {
        let payload = SpanPayload { role, kinds: piece_kinds, ..SpanPayload::default() };
        lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, body)?
    };
    Ok(Lowering::Emit(vec![staged]))
}

/// Stages the margin decoration that holds `initials`: a
/// `flm.pres.deco.margin` holding one `flm.pres.span` of role
/// [`INITIALS_ROLE`] with the annotation's kind tokens `kinds`, which holds
/// the initials as text.
fn stage_initials(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    initials: &str,
    kinds: &[Name],
) -> Result<BuildId, LowerError> {
    let text = lw.text(cx, initials)?;
    let payload = SpanPayload { role: Some(Name::from(INITIALS_ROLE)), kinds: kinds.to_vec(), ..SpanPayload::default() };
    let span = lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, vec![text])?;
    lw.pres_with(cx, &ElementName::from(pres::DECO_MARGIN), DecorationPayload, vec![span])
}
