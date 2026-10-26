//! The default lowering rules of the structural elements and of the field
//! element, and the helpers other rules use to stage paragraphs and trimmed
//! inline content.
//!
//! The **structural elements** are the elements of the node shapes no
//! feature declares: text, comments, containers, groups of each class,
//! paragraphs, paragraph breaks, template placeholders, and the macros and
//! environments a parse could not attribute to any feature
//! ([`StructuralElements`](crate::lang::structural::StructuralElements); see
//! [structural elements](crate::guide::concepts_overview#structural-elements)).
//! The field element ([`FIELD`]), the element of every field node, has its
//! rule here too.
//!
//! Most structural elements need no rule: `flm.text`, `flm.group`,
//! `flm.opaque-wrapper`, `flm.comment`, `flm.container` and `flm.parbreak` are
//! elements a backend writes natively, so nothing lowers them; the `text`
//! backend of this crate writes all six natively
//! ([`FLOW_THROUGH`](crate::stdcore::backends::text::FLOW_THROUGH)). An
//! opaque wrapper has no delimiters, so a backend writes it through its
//! children, and its text comes out as typed. The
//! paragraph-break element is among them because it remains after block
//! structuring inside opaque math content. The six rules of this module cover
//! the elements that do need a rule, and [`default_rules`] is the list the
//! `lowering` feature registers:
//!
//! | element | lowers to | rule |
//! |---|---|---|
//! | `flm.paragraph` | `flm.pres.par` | [`ParagraphRule`] |
//! | `flm.math` | `flm.pres.span` of role `flm.math`, and an Error | [`MathGroupRule`] |
//! | `flm.verbatim` | `flm.pres.verbatim` | [`VerbatimRule`] |
//! | `flm.unknown.macro` | `flm.pres.unknown` | [`UnknownMacroRule`] |
//! | `flm.unknown.environment` | `flm.pres.unknown` | [`UnknownEnvironmentRule`] |
//! | `flm.field` | nothing, and an Error | [`FieldRule`] |
//!
//! Each is registered as the default rule for its element on every backend
//! ([`DefaultMark::Everywhere`]), except the rule of `flm.math`, which is never
//! a default ([`DefaultMark::Never`]): it is chosen only where no feature that
//! processes mathematics has a rule for `flm.math` on the backend at hand. A backend uses it unless the plan request
//! prefers another rule by name, the backend writes the element natively, or
//! another rule is marked as the default on that backend
//! ([`DefaultMark::OnBackends`]); see the rule selection order at
//! [`SelectedRules`](crate::core::plan::SelectedRules).
//!
//! # What the default rules do and require
//!
//! **The paragraph rule requires the capability `flm.blocks.structured`**
//! ([`BLOCKS_STRUCTURED`]). A paragraph group exists only in a tree the
//! block-structure rewrite has processed, and a rule states what must have
//! happened before it runs by requiring a capability. Since this rule is
//! selected for every backend that does not write `flm.paragraph` natively,
//! every such plan structures its trees.
//!
//! **Verbatim content is copied unchanged.** It is an [opaque
//! region](crate::guide::concepts_overview#opaque-regions): nothing inside it
//! is lowered, and the nodes are copied unchanged into the presentation
//! node's body ([`super::copy_run_verbatim`]). Verbatim content is first read
//! as characters, as every rule that stages verbatim content reads it
//! ([`super::stage_verbatim_content`]): content that a template substituted
//! there and that is not characters is reported as
//! [`ContentNotCharacters`](super::ContentNotCharacters) at the command or
//! environment owning the group, and shown as its source text.
//!
//! **A math group is an Error until a feature processes mathematics.** The
//! math groups and the display-math environments are lowered by the rules of
//! the feature that processes mathematics (for example the feature of the
//! package [`math_passthrough`](crate::math_passthrough)). Where no
//! such feature has a rule for `flm.math` on the backend at hand, the rule of
//! last resort [`MathGroupRule`] reports
//! [`MathUnprocessed`] (`flm.math.unprocessed`) at the
//! group and lowers the group's content through the rules as ordinary content,
//! inside a `flm.pres.span` of role `flm.math` and kind `inline` or `display`.
//! The `math` feature's own rule of last resort does the same for a
//! display-math environment
//! ([`unprocessed`](crate::stdcore::features::math::unprocessed)).
//!
//! **A verbatim group's display and kind are inferred.** Its display is
//! `Block` when the group is the body of a block-role element declared with a
//! verbatim body and `Inline` otherwise; its verbatim kind is `text`, the kind
//! that applies no formatting of its own, because a bare group states no
//! kind. An element that needs another kind, another display or a language
//! tag builds the payload in its own rule from the group's children, as the
//! two rules of the `verbatim` feature do.
//!
//! **An unattributed macro or environment is lowered as content, not as an
//! error mark.** `flm.unknown.macro` and `flm.unknown.environment` are the
//! elements the parse gives a macro or an environment no feature declares.
//! They lower to `flm.pres.unknown` carrying the kind, the name and the
//! source text of the invocation, so a backend writes what the author wrote.
//! The run's diagnostics report the mistake; the output itself says nothing
//! about it.
//!
//! **A field reaching a backend is an error.** A *field* is a macro that a
//! construct absorbs as data rather than as content, such as the `\label`
//! written after a heading or the `\tag` on an equation line (see [fields:
//! data a construct
//! reads](crate::guide::concepts_overview#fields-data-a-construct-reads)),
//! and every field node carries the element [`FIELD`]. The construct that
//! declared a field reads it, and nothing is written for the field itself,
//! so a field node that reaches the lowering means that construct did not
//! read its data. [`FieldRule`] writes nothing for it and reports
//! [`FieldNotRead`] at the node, on every backend. The rule also makes every
//! plan complete for the element: a document may contain a field, so every
//! backend needs a lowering for it.
//!
//! **A mandatory argument that the parse left absent after a parse error is
//! not a second mistake.** The node lowers to the content that does exist (an
//! empty span, a heading with an empty title), and the parse's own diagnostic
//! remains the only report: no `flm.lowering.rule-failed`, no
//! `flm.pres.unsupported` stand-in. [`lower_arg_or_nothing`] and
//! [`lower_arg_named_or_nothing`] behave this way, and a rule lowering a
//! mandatory argument should call them rather than the strict
//! [`Lowerer::lower_arg`] and [`Lowerer::lower_arg_named`], which fail on an
//! absent argument. None of the rules of this module lowers an argument.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};
use techy::error::Diagnostic;
use techy::latexlike::{InvocationSyntaxData, MathGroupForm};

use crate::model::aspects::{BlockRole, block_role_of};
use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::model::names::{ElementName, Name};
use crate::lang::mint::{FIELD, field_name};
use crate::lang::structural::{MATH, PARAGRAPH, UNKNOWN_ENVIRONMENT, UNKNOWN_MACRO, VERBATIM};
use crate::lang::text::is_space;
use crate::lang::{Flm, FlmGroupType};
use crate::stdcore::backends::pres::{
    self, ParPayload, SpanPayload, UnknownKind, UnknownPayload, VerbatimDisplay, VerbatimKind,
    VerbatimPayload,
};

use super::driver::{lower_run, stage_verbatim_content};
use super::error::{FieldNotRead, MathUnprocessed};

/// The registered name of the paragraph rule, [`ParagraphRule`].
pub const PARAGRAPH_RULE: &str = "flm.paragraph.lower.default";
/// The registered name of the rule of last resort of the math groups,
/// `flm.math.lower.default` ([`MathGroupRule`]).
pub const MATH_RULE: &str = "flm.math.lower.default";
/// The registered name of the verbatim rule, [`VerbatimRule`].
pub const VERBATIM_RULE: &str = "flm.verbatim.lower.default";
/// The registered name of the unattributed-macro rule, [`UnknownMacroRule`].
pub const UNKNOWN_MACRO_RULE: &str = "flm.unknown.macro.lower.default";
/// The registered name of the unattributed-environment rule,
/// [`UnknownEnvironmentRule`].
pub const UNKNOWN_ENVIRONMENT_RULE: &str = "flm.unknown.environment.lower.default";
/// The registered name of the field rule, [`FieldRule`].
pub const FIELD_RULE: &str = "flm.field.lower.default";

/// The capability [`ParagraphRule`] requires: the table of trees the
/// block-structure rewrite writes, declared by the `blocks` feature. Because
/// the rule requires it, every plan whose backend lowers paragraphs includes
/// that rewrite.
pub const BLOCKS_STRUCTURED: &str = "flm.blocks.structured";

/// Lowers the content of the argument at `index` (in invocation order) of
/// `node`, or produces nothing when the parse left that argument absent after
/// a parse error.
///
/// A rule should use this function for a mandatory argument: the parse has
/// already reported a missing argument, and lowering reports nothing more
/// about it (see the module documentation). Use [`Lowerer::lower_arg`]
/// instead when an absent argument really is a failure of the rule.
///
/// # Errors
///
/// Returns any error that lowering the argument's content returns: a [`LowerError`]
/// from a rule that ran inside it, or a staging failure.
pub fn lower_arg_or_nothing(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    index: usize,
) -> Result<Vec<BuildId>, LowerError> {
    if node.argument_content_nodes(index).is_none() {
        return Ok(Vec::new());
    }
    lw.lower_arg(cx, node, index)
}

/// [`lower_arg_or_nothing`] for the argument named `name` instead of one at a
/// position.
///
/// # Errors
///
/// As [`lower_arg_or_nothing`], and additionally: a `name` the node's element
/// does not declare is a failure of the rule, returned by
/// [`Lowerer::lower_arg_named`], not treated as an absent argument.
pub fn lower_arg_named_or_nothing(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    name: &str,
) -> Result<Vec<BuildId>, LowerError> {
    if let Ok(None) = node.argument_content_nodes_named(name) {
        return Ok(Vec::new());
    }
    lw.lower_arg_named(cx, node, name)
}

/// Lowers `flm.field` to nothing and reports [`FieldNotRead`]: a field that
/// reaches a backend on its own is a fault of the construct that absorbed it.
///
/// See the module documentation, *A field reaching a backend is an error*.
///
/// # Errors
///
/// Returns [`LowerError`] for a node of the field element carrying no field
/// name. Every field node carries one (the name is part of the element
/// value, given when the field is declared), so only a caller that builds a
/// node of the field element without a field name can cause this; nothing in
/// this crate does.
#[derive(Debug, Default)]
pub struct FieldRule;

impl Lower for FieldRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let field = field_name(&node)
            .ok_or_else(|| {
                LowerError::structure(
                    "a field node must carry its field name, and this one carries none",
                )
            })?;
        let condition =
            FieldNotRead { backend: lw.backend().to_string(), field: field.to_string() };
        lw.diagnose(Diagnostic::error(condition, node.span().clone()));
        Ok(Lowering::Emit(Vec::new()))
    }
}

/// Stages one `flm.pres.par` holding `leading`, then the inline content of
/// the paragraph `node` lowered through the rules, then `trailing`, and
/// returns it.
///
/// Every paragraph of this crate is staged through this function.
/// [`ParagraphRule`] calls it with nothing at either edge; a rule that
/// writes a paragraph of the document with something added at its start or
/// its end (the bracket and the mark of an xnote shown in place, the link
/// back from an entry of a list of xnotes) calls it with the nodes to add,
/// which it staged and placed nowhere else. `node` is a paragraph the
/// block-structure pass built (`flm.paragraph`), or `None` for a paragraph
/// that holds no content of the document, only `leading` then `trailing`.
///
/// The [`Lowerer`] moves any decoration in the paragraph into an attached
/// slot and leaves a `flm.pres.deco-mark` at its former position. The
/// paragraph is staged by the calling rule, at that rule's node: a rule that
/// replaces [`ParagraphRule`] is not used for it.
///
/// The calling rule must declare that it produces `flm.pres.par`.
///
/// # Errors
///
/// [`LowerError`] from the lowering of the paragraph's run, and when the
/// calling rule did not declare that it produces `flm.pres.par` or the node
/// could not be staged.
// See decision rationale at [§flm-dd-dr:std-lower-paragraph].
pub fn lower_paragraph(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: Option<&NodeRef<'_, Flm>>,
    leading: Vec<BuildId>,
    trailing: Vec<BuildId>,
) -> Result<BuildId, LowerError> {
    let mut body = leading;
    if let Some(node) = node {
        body.extend(lw.lower_children(cx, node)?);
    }
    body.extend(trailing);
    lw.pres_with(cx, &ElementName::from(pres::PAR), ParPayload::default(), body)
}

/// Lowers `content` (a run of sibling nodes holding paragraphs and blocks,
/// or inline content) as blocks, with `leading` at the start of the first
/// paragraph's inline content and `trailing` at the end of the last one, and
/// returns the blocks.
///
/// A rule uses this function to place something before the first words, or
/// after the last words, of content that may hold paragraphs: the opening
/// and the closing bracket of an xnote shown in place, the link back from an
/// entry of a list of xnotes to its mark, the lead-in of a term definition,
/// the heading and the closing mark of a theorem.
/// `leading` and `trailing` hold nodes the caller staged and placed nowhere
/// else; either may be empty.
///
/// - Inline content (no paragraph and no block among the nodes) becomes one
///   `flm.pres.par` holding `leading`, the content lowered through the rules,
///   then `trailing`.
/// - Content holding blocks is lowered block by block: each paragraph becomes
///   a `flm.pres.par` of its inline content lowered through the rules, with
///   `leading` at the start of the first one and `trailing` at the end of the
///   last one; every other block is lowered by the rules. Content whose first
///   block is not a paragraph (a list, for example) gets one more
///   `flm.pres.par` before it, holding `leading` alone, and content whose
///   last block is not a paragraph one more after it, holding `trailing`
///   alone. Comments at either end are skipped when the first and the last
///   block are found.
/// - Empty content with something to place is treated as inline content
///   too: one `flm.pres.par` holding `leading` then `trailing`, so a theorem
///   with an empty body shows its heading and its closing mark in one
///   paragraph. Empty content with nothing to place gives nothing.
///
/// Every paragraph is staged through [`lower_paragraph`], by the calling
/// rule: a rule that replaces the paragraph rule is not used for these
/// paragraphs. The calling rule must declare that it produces
/// `flm.pres.par`.
///
/// # Errors
///
/// [`LowerError`] when `content` is not a run of siblings of the tree being
/// lowered, and from the lowering of the content and the staging of the
/// paragraphs.
pub fn lower_blocks_with_edges(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    content: &[NodeRef<'_, Flm>],
    leading: Vec<BuildId>,
    trailing: Vec<BuildId>,
) -> Result<Vec<BuildId>, LowerError> {
    let is_paragraph = |node: &NodeRef<'_, Flm>| node.group_type() == Some(FlmGroupType::Paragraph);
    let is_block = |node: &NodeRef<'_, Flm>| {
        is_paragraph(node) || matches!(block_role_of(node), BlockRole::Block)
    };
    if !content.iter().any(is_block) {
        let run = lower_run(cx, lw, content)?;
        if run.is_empty() && leading.is_empty() && trailing.is_empty() {
            return Ok(Vec::new());
        }
        return Ok(vec![lower_paragraph(cx, lw, None, [leading, run].concat(), trailing)?]);
    }
    // See decision rationale at [§flm-dd-dr:std-xnote-edge-paragraphs].
    let first = content.iter().position(|node| !node.is_comment());
    let last = content.iter().rposition(|node| !node.is_comment());
    let into_first = first.is_some_and(|at| is_paragraph(&content[at]));
    let into_last = last.is_some_and(|at| is_paragraph(&content[at]));
    let mut leading = Some(leading);
    let mut trailing = Some(trailing);
    let mut out = Vec::new();
    if !into_first {
        let leading = leading.take().unwrap_or_default();
        if !leading.is_empty() {
            out.push(lower_paragraph(cx, lw, None, leading, Vec::new())?);
        }
    }
    for (at, node) in content.iter().enumerate() {
        if !is_paragraph(node) {
            out.extend(lower_run(cx, lw, std::slice::from_ref(node))?);
            continue;
        }
        let head = if into_first && Some(at) == first { leading.take() } else { None };
        let tail = if into_last && Some(at) == last { trailing.take() } else { None };
        let (head, tail) = (head.unwrap_or_default(), tail.unwrap_or_default());
        out.push(lower_paragraph(cx, lw, Some(node), head, tail)?);
    }
    let trailing = trailing.take().unwrap_or_default();
    if !trailing.is_empty() {
        out.push(lower_paragraph(cx, lw, None, Vec::new(), trailing)?);
    }
    Ok(out)
}

/// Stages `content`, a run of inline content, with the whitespace at both of
/// its ends trimmed, and returns the staged nodes. The whitespace and the
/// comments before its first content and after its last are left out, and
/// the text at either edge is staged without its leading or its trailing
/// whitespace. Everything between the two edges is lowered through the rules
/// unchanged. Content holding only whitespace and comments stages nothing.
///
/// A rule uses this function to write a paragraph over inline content whose
/// edges the block-structure pass did not trim, because the rule cuts that
/// content out of a longer run itself: the `quote` feature's passage,
/// attribution and lines, and an entry of the `cells` feature's
/// `\celldata`, each written without the spaces around it. Whitespace here
/// is what [`is_blank`] counts as blank: the space, the tab, the carriage
/// return and the line feed, and the nodes that are whitespace, such as `~`
/// and `\ ` ([`Whitespace`](crate::model::aspects::Whitespace)), the same
/// whitespace the block-structure pass removes at a paragraph's edges. The
/// character U+00A0 typed in text is content.
///
/// `passed_over` selects further nodes that are not content (commands that
/// write nothing, which the calling rule reads in place, such as the `\\`
/// and the `\indent` of a quotation's lines section); they are left out at
/// the edges as whitespace is. A caller with none passes `|_| false`.
///
/// # Errors
///
/// [`LowerError`] from the lowering of the content and the staging of the
/// trimmed text.
pub fn stage_trimmed(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    content: &[NodeRef<'_, Flm>],
    passed_over: impl Fn(&NodeRef<'_, Flm>) -> bool,
) -> Result<Vec<BuildId>, LowerError> {
    stage_trimmed_runs(cx, lw, &[content], passed_over)
}

/// [`stage_trimmed`] over several runs of siblings read as one run of
/// content, the edges being those of the whole. The `quote` feature uses it
/// for the lines of a quotation's lines section, which are cut into runs at
/// each `\indent`.
///
/// # Errors
///
/// As [`stage_trimmed`].
// A trimmed edge is staged as a text node of its own: the block-structuring
// pass trims a paragraph's edges while it builds the paragraph, over text it
// is resolving, and nothing of it can be called on a structured tree at
// lowering, so the edge's trimmed text is staged through the façade.
// See decision rationale at [§flm-dd-dr:std-quote-sections].
pub fn stage_trimmed_runs(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    runs: &[&[NodeRef<'_, Flm>]],
    passed_over: impl Fn(&NodeRef<'_, Flm>) -> bool,
) -> Result<Vec<BuildId>, LowerError> {
    let is_content =
        |node: &NodeRef<'_, Flm>| !(node.is_comment() || is_blank(node) || passed_over(node));
    let nodes: Vec<(usize, usize)> = runs
        .iter()
        .enumerate()
        .flat_map(|(run, nodes)| (0..nodes.len()).map(move |at| (run, at)))
        .collect();
    let node_at = |(run, at): (usize, usize)| runs[run][at];
    let Some(first) = nodes.iter().position(|&place| is_content(&node_at(place))) else {
        return Ok(Vec::new());
    };
    let last = nodes.iter().rposition(|&place| is_content(&node_at(place))).unwrap_or(first);
    let mut out = Vec::new();
    // The nodes from `first` to `last`, each run's share lowered as one run;
    // the two edge nodes, when they are text, staged trimmed.
    let mut pending: Option<(usize, usize, usize)> = None;
    for (index, &(run, at)) in nodes.iter().enumerate().take(last + 1).skip(first) {
        let node = runs[run][at];
        let trimmed = (index == first || index == last)
            .then(|| node.chars())
            .flatten()
            .map(|text| {
                let text = if index == first { text.trim_start_matches(is_space) } else { text };
                if index == last { text.trim_end_matches(is_space) } else { text }
            })
            .filter(|trimmed| Some(*trimmed) != node.chars());
        match trimmed {
            Some(text) => {
                if let Some((run, from, to)) = pending.take() {
                    out.extend(lower_run(cx, lw, &runs[run][from..to])?);
                }
                out.push(lw.text(cx, text)?);
            }
            None => {
                pending = match pending {
                    Some((pending_run, from, to)) if pending_run == run && to == at => {
                        Some((run, from, at + 1))
                    }
                    Some((pending_run, from, to)) => {
                        out.extend(lower_run(cx, lw, &runs[pending_run][from..to])?);
                        Some((run, at, at + 1))
                    }
                    None => Some((run, at, at + 1)),
                };
            }
        }
    }
    if let Some((run, from, to)) = pending {
        out.extend(lower_run(cx, lw, &runs[run][from..to])?);
    }
    Ok(out)
}

/// Whether `node` is blank: a node that is whitespace, such as `~` and `\ `
/// (its element carries the aspect
/// [`Whitespace`](crate::model::aspects::Whitespace)), or text made of
/// whitespace alone: the space, the tab, the carriage return and the line
/// feed. Text holding the character U+00A0 is not blank. [`stage_trimmed`]
/// leaves blank nodes out at the edges of a run.
///
/// This is the one blank test of every reader of FLM content (see [the
/// whitespace of FLM text](crate::lang::text#whitespace)): the
/// block-structure pass and the features' readers make the same test.
pub fn is_blank(node: &NodeRef<'_, Flm>) -> bool {
    crate::lang::text::is_blank(node)
}

/// Lowers `flm.paragraph` to a `flm.pres.par` holding the paragraph's
/// lowered inline content, through [`lower_paragraph`].
///
/// The [`Lowerer`] moves any decoration in that content into an attached slot
/// and leaves a `flm.pres.deco-mark` at its former position.
#[derive(Debug, Default)]
pub struct ParagraphRule;

impl Lower for ParagraphRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let par = lower_paragraph(cx, lw, Some(&node), Vec::new(), Vec::new())?;
        Ok(Lowering::Emit(vec![par]))
    }
}

/// The rule of last resort of the math groups: it reports
/// [`MathUnprocessed`] (`flm.math.unprocessed`, an Error) at the group's span,
/// and lowers the group's content through the rules as ordinary content,
/// inside a `flm.pres.span` of role `flm.math` ([`MATH`]) and kind `inline`
/// or `display` (see the module documentation, *A math group is an Error
/// until a feature processes mathematics*).
///
/// It is marked [`DefaultMark::Never`], so it is chosen on a backend only
/// where it is the sole candidate for `flm.math`: in a registry without a
/// feature that processes mathematics, or whose processing feature has no
/// rule for `flm.math` on that backend. The content is lowered as the rules
/// lower it anywhere: a character run is text, a comment a comment, an
/// unknown command its `flm.pres.unknown`, a text command its
/// `flm.pres.math-text`.
///
/// # Errors
///
/// Returns [`LowerError`] when a child could not be lowered or the span could
/// not be staged.
#[derive(Debug, Default)]
pub struct MathGroupRule;

impl Lower for MathGroupRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lw.diagnose(Diagnostic::error(MathUnprocessed, node.span().clone()));
        let kind = match node.group_type() {
            Some(FlmGroupType::Math(MathGroupForm::Display)) => MATH_DISPLAY_KIND,
            _ => MATH_INLINE_KIND,
        };
        let content = lw.lower_children(cx, &node)?;
        let payload = SpanPayload {
            role: Some(Name::from(MATH)),
            kinds: vec![Name::from(kind)],
            ..SpanPayload::default()
        };
        let span = lw.pres_with(cx, &ElementName::from(pres::SPAN), payload, content)?;
        Ok(Lowering::Emit(vec![span]))
    }
}

/// The kind of the span [`MathGroupRule`] stages for an inline math group
/// (`\(…\)`, `$…$`), `inline`.
pub const MATH_INLINE_KIND: &str = "inline";

/// The kind of the span [`MathGroupRule`] stages for a displayed math group
/// (`\[…\]`, `$$…$$`), `display`.
pub const MATH_DISPLAY_KIND: &str = "display";

/// Lowers `flm.verbatim` to `flm.pres.verbatim`, copying the group's content
/// unchanged once it is read as characters ([`stage_verbatim_content`];
/// content that is not characters is reported at the command or environment
/// owning the group, and shown as its source text).
///
/// The display is `Block` when the group is the body of a block-role element
/// declared with a verbatim body, and `Inline` otherwise. The verbatim kind
/// is always `text`, the kind that applies no formatting of its own, because
/// a bare verbatim group states no kind (see the module documentation).
#[derive(Debug, Default)]
pub struct VerbatimRule;

/// The callable whose argument or body directly holds `node` (a verbatim
/// group, a quote environment's section command), when there is one. An
/// opaque wrapper over the argument or the body
/// ([`FlmGroupType::OpaqueWrapper`]) is looked through.
pub(crate) fn owning_callable<'t>(node: &NodeRef<'t, Flm>) -> Option<NodeRef<'t, Flm>> {
    // The body slot's content nodes are the callable's own children; a
    // transparent container in between (a hook's) is looked through. An
    // opaque wrapper stands over an argument's nodes, or over the body's
    // container under a container of its own.
    let parent_past_list = |node: &NodeRef<'t, Flm>| {
        let parent = node.parent()?;
        if parent.is_list() { parent.parent() } else { Some(parent) }
    };
    let mut owner = parent_past_list(node)?;
    if owner.group_type() == Some(FlmGroupType::OpaqueWrapper) {
        owner = parent_past_list(&owner)?;
    }
    owner.is_callable().then_some(owner)
}

/// Whether the verbatim group is the body of a block-role element: the group
/// is one of the content nodes of the owning callable's body slot (the body
/// of an environment declared with a verbatim body), and the callable's block
/// role is `Block`.
fn is_block_verbatim(node: &NodeRef<'_, Flm>) -> bool {
    owning_callable(node).is_some_and(|owner| {
        block_role_of(&owner) == BlockRole::Block
            && owner.body().is_some_and(|body| body.iter().any(|child| child.id() == node.id()))
    })
}

impl Lower for VerbatimRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let display =
            if is_block_verbatim(&node) { VerbatimDisplay::Block } else { VerbatimDisplay::Inline };
        let owner = owning_callable(&node).unwrap_or(node);
        let body = stage_verbatim_content(&owner, node.children(), cx, lw)?;
        let payload = VerbatimPayload { kind: VerbatimKind::Text, display, lang: None };
        let verbatim = lw.pres_with(cx, &ElementName::from(pres::VERBATIM), payload, body)?;
        Ok(Lowering::Emit(vec![verbatim]))
    }
}

/// The name of an unattributed callable as written, and the escape character
/// that introduced it when the node records one (a macro).
fn spelling_of(node: &NodeRef<'_, Flm>) -> (String, Option<char>) {
    let name = node.name().unwrap_or_default().to_string();
    let escape = match node.invocation_syntax() {
        Some(InvocationSyntaxData::Macro { escape_char, .. }) => Some(*escape_char),
        _ => None,
    };
    (name, escape)
}

/// Lowers `flm.unknown.macro` (a macro no feature declares) to a childless
/// `flm.pres.unknown` carrying its source text, `\name`.
#[derive(Debug, Default)]
pub struct UnknownMacroRule;

impl Lower for UnknownMacroRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let (name, escape) = spelling_of(&node);
        let payload = UnknownPayload {
            kind: UnknownKind::Macro,
            spelling: format!("{}{name}", escape.unwrap_or('\\')),
            name,
        };
        let unknown =
            lw.pres_with(cx, &ElementName::from(pres::UNKNOWN), payload, Vec::new())?;
        Ok(Lowering::Emit(vec![unknown]))
    }
}

/// Lowers `flm.unknown.environment` (an environment no feature declares) to
/// a `flm.pres.unknown` holding its lowered body.
///
/// The payload's `spelling` is the bare environment name; each backend
/// writes its own `\begin` and `\end` around the body.
#[derive(Debug, Default)]
pub struct UnknownEnvironmentRule;

impl Lower for UnknownEnvironmentRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let (name, _) = spelling_of(&node);
        let body = lw.lower_body(cx, &node)?;
        let payload = UnknownPayload {
            kind: UnknownKind::Environment,
            spelling: name.clone(),
            name,
        };
        let unknown = lw.pres_with(cx, &ElementName::from(pres::UNKNOWN), payload, body)?;
        Ok(Lowering::Emit(vec![unknown]))
    }
}

/// The six default rules of this module (five for structural elements, one
/// for the field element), as values a feature can register; the `lowering`
/// feature registers them.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::lowering::{PARAGRAPH_RULE, default_rules};
///
/// let names: Vec<String> =
///     default_rules().iter().map(|rule| rule.name.to_string()).collect();
/// assert_eq!(names.len(), 6);
/// assert!(names.contains(&PARAGRAPH_RULE.to_string()));
/// ```
pub fn default_rules() -> Vec<LoweringRule> {
    vec![
        LoweringRule::new(
            FIELD_RULE,
            FIELD,
            LoweringDecl::new(),
            Arc::new(FieldRule),
            DefaultMark::Everywhere,
        )
        .docs(
            "a field that reaches a backend on its own is an error: the construct it was \
             written after did not read its data",
        ),
        LoweringRule::new(
            PARAGRAPH_RULE,
            PARAGRAPH,
            LoweringDecl::new().requires(BLOCKS_STRUCTURED).produces(pres::PAR),
            Arc::new(ParagraphRule),
            DefaultMark::Everywhere,
        )
        .docs("a paragraph group lowers to `flm.pres.par` over its lowered inline run"),
        LoweringRule::new(
            MATH_RULE,
            MATH,
            LoweringDecl::new().produces(pres::SPAN),
            Arc::new(MathGroupRule),
            // The rule of last resort: never a default, so that the rule of a
            // feature that processes mathematics applies wherever one is
            // registered for the backend.
            DefaultMark::Never,
        )
        .docs(
            "a math group that no feature processes is an Error, and lowers to a \
             `flm.pres.span` of role `flm.math` over its content lowered as ordinary content",
        ),
        LoweringRule::new(
            VERBATIM_RULE,
            VERBATIM,
            LoweringDecl::new().produces(pres::VERBATIM),
            Arc::new(VerbatimRule),
            DefaultMark::Everywhere,
        )
        .docs("a verbatim group lowers to `flm.pres.verbatim` over its content, verbatim"),
        LoweringRule::new(
            UNKNOWN_MACRO_RULE,
            UNKNOWN_MACRO,
            LoweringDecl::new().produces(pres::UNKNOWN),
            Arc::new(UnknownMacroRule),
            DefaultMark::Everywhere,
        )
        .docs("an unattributed macro lowers to a childless `flm.pres.unknown` of its spelling"),
        LoweringRule::new(
            UNKNOWN_ENVIRONMENT_RULE,
            UNKNOWN_ENVIRONMENT,
            LoweringDecl::new().produces(pres::UNKNOWN),
            Arc::new(UnknownEnvironmentRule),
            DefaultMark::Everywhere,
        )
        .docs("an unattributed environment lowers to `flm.pres.unknown` over its lowered body"),
    ]
}
