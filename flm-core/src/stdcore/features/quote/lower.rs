//! The general lowering rules of the `quote` feature, the defaults on every
//! backend: [`QuoteRule`] lowers a quotation to one `flm.pres.block` of role
//! [`QUOTE_ROLE`] holding one block per quotation section, and
//! [`SectionRule`] lowers a quotation-section command outside section
//! position to its argument's content, reporting it unless it is part of a
//! quotation body's bare content.
//!
//! The rules read the body each time they lower a quotation and store
//! nothing: the quotation sections are the body's direct children in section
//! position, and a lines section is split into lines while the rule stages
//! it. The reading of one line ([`read_line`]) and the tests of the two
//! commands of a lines section are shared with the recomposition rule of the
//! `latex-recomposed` backend ([`latex`](super::latex)).

use techy::core::node::{BuildId, NodeRef};
use techy::error::Diagnostic;

use crate::core::lowering::{Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl};
use crate::lang::Flm;
use crate::lang::mint::is_field_named;
use crate::lang::rewrites::blocks::argument_holds_blocks;
use crate::model::element::element_value;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::lowering::{
    is_blank, lower_paragraph, lower_run, stage_trimmed, stage_trimmed_runs,
};
use crate::stdcore::backends::pres::{self, BlockPayload, Length, LinePayload, LinesPayload};

use super::config::{QuoteSection, english_list};
use super::diagnostics::{BareContent, IndentMidLine, NestedSection};
use super::html::STYLE;
use super::elements::{QuoteElement, SectionElement, is_in_quote_body, section_position};
use super::{INDENT_COMMAND, LINEBREAK_COMMAND, QUOTE_ROLE};

/// The implementation of the lowering rule [`QUOTE_RULE`](super::QUOTE_RULE),
/// the default on every backend: a quotation lowers to one `flm.pres.block`
/// of role [`QUOTE_ROLE`] whose one kind token is the environment's name,
/// holding one `flm.pres.block` per quotation section in the order written,
/// of the role of the section's kind ([`QuoteSection::role`]):
///
/// - a text or an attribution section holds one paragraph over its content,
///   the whitespace at both ends of it trimmed;
/// - a block section holds its content's paragraphs and blocks, or one
///   paragraph where its content is written as inline content;
/// - a lines section holds one `flm.pres.lines` with one `flm.pres.line` per
///   line: the content is split at every `\\`, each line's leading `\indent`s
///   are counted into its left indentation as that many tabs, and the
///   whitespace at both ends of each line is trimmed.
///
/// The rule reads the body's direct children as a sequence of
/// quotation-section commands and **bare runs**. A quotation-section command
/// in *section position*, written directly in the body of an environment that
/// accepts it, opens a quotation section; each maximal run of other content
/// between two of them is one bare run. Whitespace and source comments at a
/// run's edges stage nothing, and a run that holds nothing else stages
/// nothing. A bare run is staged exactly as a quotation section of the
/// environment's bare-content kind would be staged over the same content. In
/// an environment that reads no bare content, a bare run is reported once
/// ([`BareContent`]) and lowered in place inside the quotation's block,
/// outside any quotation section. A quotation section that holds nothing,
/// such as `\attributed{}`, is an empty block of its role. An `\indent`
/// after the first content of its line is reported ([`IndentMidLine`]) and
/// left out.
///
/// A quotation-section command that the environment does not accept, written
/// in its body, can only be a command of an enclosing quotation, available
/// there because the body is inside one of that quotation's quotation
/// sections. It is part of a bare run like any other content, and its own
/// rule ([`SectionRule`]) lowers it to its content without reporting it a
/// second time.
///
/// At every quotation the rule requires the feature's style identifier
/// [`STYLE`] (`flm.quote.style`), which each backend resolves as it resolves
/// requirements: the stylesheet on the `html` backend, the definition pieces
/// delivering it on the `latex-rendered` backend, nothing on the `text`
/// backend.
///
/// # Errors
///
/// [`LowerError`] when the node carries no [`QuoteElement`], and any error
/// from lowering the content.
#[derive(Debug, Default, Clone, Copy)]
pub struct QuoteRule;

impl Lower for QuoteRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        lw.require(&Name::from(STYLE));
        let value = element_value::<QuoteElement>(&node)
            .ok_or_else(|| LowerError::structure("a quotation without its element's data"))?;
        let body: Vec<NodeRef<'_, Flm>> =
            node.body().map(|body| body.iter().collect()).unwrap_or_default();
        let mut children = Vec::new();
        let mut start = 0;
        for (at, child) in body.iter().enumerate() {
            if let Some(position) = section_position(child) {
                bare_run(cx, lw, value, &body[start..at], &mut children)?;
                children.push(stage_section_node(cx, lw, position.section, child)?);
                start = at + 1;
            }
        }
        bare_run(cx, lw, value, &body[start..], &mut children)?;
        let payload = BlockPayload {
            role: Some(Name::from(QUOTE_ROLE)),
            kinds: vec![Name::from(value.name.as_str())],
            anchor: None,
            doc_role: None,
        };
        let block = lw.pres_with(cx, &ElementName::from(pres::BLOCK), payload, children)?;
        Ok(Lowering::Emit(vec![block]))
    }
}

/// Returns the declaration shared by the rules that lower a quotation into
/// the presentation vocabulary ([`QuoteRule`], and the `html` backend's rule
/// that calls it): they produce blocks, paragraphs, and a lines section's
/// `flm.pres.lines` and `flm.pres.line`.
pub(super) fn quote_decl() -> LoweringDecl {
    LoweringDecl::new()
        .produces(pres::BLOCK)
        .produces(pres::PAR)
        .produces(pres::LINES)
        .produces(pres::LINE)
}

/// Stages `run`, a maximal run of bare content between two quotation
/// sections of the quotation `value`, without the whitespace and source
/// comments at its edges: as one quotation section of the environment's
/// bare-content kind, or, where the environment reads no bare content,
/// reported once and lowered in place. A run that holds nothing but
/// whitespace and source comments stages nothing.
fn bare_run(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    value: &QuoteElement,
    run: &[NodeRef<'_, Flm>],
    out: &mut Vec<BuildId>,
) -> Result<(), LowerError> {
    let is_edge = |node: &NodeRef<'_, Flm>| node.is_comment() || is_blank(node);
    let Some(first) = run.iter().position(|node| !is_edge(node)) else { return Ok(()) };
    let last = run.iter().rposition(|node| !is_edge(node)).unwrap_or(first);
    let run = &run[first..=last];
    let holds_blocks = value.environment.holds_blocks();
    match value.environment.bare {
        Some(kind) => out.push(stage_section(cx, lw, kind, run, holds_blocks)?),
        None => {
            lw.diagnose(Diagnostic::error(bare_content(value), run[0].span().clone()));
            // An environment reading no bare content has a body of blocks.
            out.extend(lower_blocks(cx, lw, run)?);
        }
    }
    Ok(())
}

/// Returns the condition reporting bare content in the quote environment
/// `value`, listing the quotation-section commands it accepts.
pub(super) fn bare_content(value: &QuoteElement) -> BareContent {
    let sections =
        value.environment.sections.iter().map(|section| format!("\\{}", section.as_str()));
    BareContent { environment: value.name.clone(), sections: english_list(sections, "or") }
}

/// Stages the quotation section that the quotation-section command `node`
/// writes, over the command's argument.
fn stage_section_node(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    section: QuoteSection,
    node: &NodeRef<'_, Flm>,
) -> Result<BuildId, LowerError> {
    let content: Vec<NodeRef<'_, Flm>> =
        node.argument_content_nodes(0).map(|nodes| nodes.iter().collect()).unwrap_or_default();
    stage_section(cx, lw, section, &content, argument_holds_blocks(node, 0))
}

/// Stages one quotation section of the kind `section` over `content`, which
/// holds paragraphs and blocks when `holds_blocks` is true and inline content
/// otherwise.
fn stage_section(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    section: QuoteSection,
    content: &[NodeRef<'_, Flm>],
    holds_blocks: bool,
) -> Result<BuildId, LowerError> {
    let staged = match section {
        QuoteSection::Lines => vec![stage_lines(cx, lw, content)?],
        QuoteSection::Block if holds_blocks => lower_blocks(cx, lw, content)?,
        QuoteSection::Text | QuoteSection::Attributed | QuoteSection::Block => {
            let run = stage_trimmed(cx, lw, content, is_line_command)?;
            if run.is_empty() {
                Vec::new()
            } else {
                vec![lower_paragraph(cx, lw, None, run, Vec::new())?]
            }
        }
    };
    sub_block(cx, lw, section, staged)
}

/// Stages a `flm.pres.block` of the role of the kind `section`, holding
/// `content`.
fn sub_block(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    section: QuoteSection,
    content: Vec<BuildId>,
) -> Result<BuildId, LowerError> {
    let payload = BlockPayload {
        role: Some(Name::from(section.role())),
        kinds: Vec::new(),
        anchor: None,
        doc_role: None,
    };
    lw.pres_with(cx, &ElementName::from(pres::BLOCK), payload, content)
}

/// Lowers `content`, paragraphs and blocks, one block at a time, leaving out
/// the source comments and whitespace among them.
fn lower_blocks(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    content: &[NodeRef<'_, Flm>],
) -> Result<Vec<BuildId>, LowerError> {
    let mut out = Vec::new();
    for node in content {
        if !(node.is_comment() || is_blank(node)) {
            out.extend(lower_run(cx, lw, std::slice::from_ref(node))?);
        }
    }
    Ok(out)
}

/// Stages a lines section over `content`: one `flm.pres.lines` holding one
/// `flm.pres.line` per line, the content split at every `\\`. A lines
/// section that holds neither content nor a `\\` has no line at all.
fn stage_lines(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    content: &[NodeRef<'_, Flm>],
) -> Result<BuildId, LowerError> {
    let mut lines = Vec::new();
    if content.iter().any(|node| is_content(node) || is_linebreak(node)) {
        for line in content.split(is_linebreak) {
            lines.push(stage_line(cx, lw, line)?);
        }
    }
    lw.pres_with(cx, &ElementName::from(pres::LINES), LinesPayload, lines)
}

/// Stages one line: its leading `\indent` commands (whitespace and source
/// comments before and between them are skipped) set its left indentation
/// to as many tabs, a later `\indent` is reported and left out, and the rest
/// of the line is staged with the whitespace at both of its ends trimmed.
fn stage_line(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    line: &[NodeRef<'_, Flm>],
) -> Result<BuildId, LowerError> {
    let (indent_left, runs) = read_line(lw, line);
    let staged = stage_trimmed_runs(cx, lw, &runs, is_line_command)?;
    let payload = LinePayload { indent_left, ..LinePayload::default() };
    lw.pres_with(cx, &ElementName::from(pres::LINE), payload, staged)
}

/// Reads one line of a lines section, the nodes between two `\\`, and
/// returns its left indentation, as many tabs as its leading `\indent`
/// commands (whitespace and source comments before and between them are
/// skipped), and the rest of its nodes as runs of siblings, every `\indent`
/// left out. An `\indent` after the first content of the line is reported
/// ([`IndentMidLine`]).
///
/// Every rule that lowers a lines section reads its lines with this
/// function, so that the indentation and the report are the same on every
/// backend.
pub(super) fn read_line<'l, 't>(
    lw: &mut dyn Lowerer,
    line: &'l [NodeRef<'t, Flm>],
) -> (Length, Vec<&'l [NodeRef<'t, Flm>]>) {
    let mut indents: i32 = 0;
    let mut started = false;
    let mut runs: Vec<&[NodeRef<'_, Flm>]> = Vec::new();
    let mut run_start = 0;
    for (at, node) in line.iter().enumerate() {
        if is_field_named(node, INDENT_COMMAND) {
            if started {
                lw.diagnose(Diagnostic::error(IndentMidLine, node.span().clone()));
            } else {
                indents = indents.saturating_add(1);
            }
            runs.push(&line[run_start..at]);
            run_start = at + 1;
        } else if is_content(node) {
            started = true;
        }
    }
    runs.push(&line[run_start..]);
    (Length::tabs(indents), runs)
}

/// Returns whether `node` is content of a quotation section: neither a
/// source comment, nor whitespace, nor one of the two commands of a lines
/// section.
pub(super) fn is_content(node: &NodeRef<'_, Flm>) -> bool {
    !(node.is_comment() || is_blank(node) || is_line_command(node))
}

/// Returns whether `node` is one of the two commands of a lines section,
/// `\\` or `\indent`, which write nothing in place.
pub(super) fn is_line_command(node: &NodeRef<'_, Flm>) -> bool {
    is_linebreak(node) || is_field_named(node, INDENT_COMMAND)
}

/// Returns whether `node` is the `\\` that ends a line of a lines section.
pub(super) fn is_linebreak(node: &NodeRef<'_, Flm>) -> bool {
    is_field_named(node, LINEBREAK_COMMAND)
}

/// The implementation of the lowering rule
/// [`SECTION_RULE`](super::SECTION_RULE), the default on every backend: a
/// quotation-section command outside section position is lowered as its
/// argument's content, with a lines section's `\\` and `\indent` left out.
/// (A command in section position never reaches this rule: the lowering rule
/// of its quotation stages its quotation section and does not lower the
/// command node itself.)
///
/// A command written inside another quotation section's argument
/// (`\lines{\text{x}}`), or anywhere else outside a quotation's body, is
/// reported ([`NestedSection`]). A command written directly in the body of a
/// quote environment that does not accept it (a command of an enclosing
/// quotation, available in a nested quotation's body) is part of that body's
/// bare content, which the quotation's rule reports once per bare run where
/// the environment reads none; this rule does not report it a second time.
///
/// # Errors
///
/// [`LowerError`] when the node carries no [`SectionElement`], and any error
/// from lowering the content.
#[derive(Debug, Default, Clone, Copy)]
pub struct SectionRule;

impl Lower for SectionRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let section = element_value::<SectionElement>(&node)
            .ok_or_else(|| LowerError::structure(
                "a quotation-section command without its element's data"
            ))?
            .section;
        if !is_in_quote_body(&node) {
            lw.diagnose(Diagnostic::error(
                NestedSection { section: section.as_str().to_string() },
                node.span().clone(),
            ));
        }
        let content: Vec<NodeRef<'_, Flm>> =
            node.argument_content_nodes(0).map(|nodes| nodes.iter().collect()).unwrap_or_default();
        let mut out = Vec::new();
        for run in content.split(|node| is_linebreak(node) || is_field_named(node, INDENT_COMMAND)) {
            out.extend(lower_run(cx, lw, run)?);
        }
        Ok(Lowering::Emit(out))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::features::quote::QuoteEnvironment;

    #[test]
    fn the_bare_content_report_lists_the_accepted_sections_as_english_does() {
        let listed = |sections: Vec<QuoteSection>| {
            let environment = QuoteEnvironment::new(sections);
            bare_content(&QuoteElement { name: "quote".into(), environment }).sections
        };
        assert_eq!(listed(vec![QuoteSection::Block]), "\\block");
        assert_eq!(listed(vec![QuoteSection::Block, QuoteSection::Text]), "\\text or \\block");
        assert_eq!(listed(QuoteSection::ALL.to_vec()), "\\text, \\lines, \\attributed or \\block");
    }
}
