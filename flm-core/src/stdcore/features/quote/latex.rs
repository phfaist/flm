//! The recomposition rule of the [`quote`](super) feature on the
//! `latex-recomposed` backend: a quotation is written back as the environment
//! the author wrote, each quotation section through a LaTeX hook. A LaTeX
//! hook is a macro that the recomposed LaTeX writes and the [style
//! file](crate::stdcore::backends::latex::style) defines with a plain
//! default, which a user redefines in the document's preamble to change what
//! the recomposed LaTeX does at that point
//! ([LaTeX hooks](crate::stdcore::backends::latex::preamble)).
//!
//! | written | LaTeX |
//! |---|---|
//! | `\begin{quote}\text{To be.}\attributed{Shakespeare}\end{quote}` | `\begin{quote}\flmQuoteText{To be.}\flmQuoteAttributed{Shakespeare}\end{quote}` |
//! | `\lines{First\\ \indent Second}` | `\flmQuoteLines{First\\\relax`, then on the next line `\flmIndent{1\flmTabWidth}Second}` |
//! | `\block{One.⏎⏎Two.}` | `\flmQuoteBlock{One.⏎⏎Two.}` |
//! | `\begin{blockquote}One.⏎⏎Two.\end{blockquote}` | `\begin{blockquote}\flmQuoteBlock{One.⏎⏎Two.}\end{blockquote}` |
//! | `\begin{address}123 Main St\\ City\end{address}` | `\begin{address}\flmQuoteLines{123 Main St\\\relax`, then `City}\end{address}` |
//!
//! The rendering guide describes the same output for readers of the
//! recomposed LaTeX
//! ([Quotations in the recomposed LaTeX](crate::guide::rendering#quotations-in-the-recomposed-latex)).
//!
//! # The environment and its quotation sections
//!
//! The rule [`LATEX_RULE`] ([`QuoteLatexRule`]) writes a quotation as the
//! environment of its name. Inside the environment, it writes the body as the
//! author wrote it (the whitespace, blank lines and source comments between
//! the quotation sections unchanged, the nodes that are whitespace, such as
//! `~` and `\ `, left out), and each quotation section through the LaTeX hook
//! of its kind ([`hook_of`]):
//!
//! - `\flmQuoteText{<passage>}`, `\flmQuoteAttributed{<attribution>}` and
//!   `\flmQuoteBlock{<paragraphs>}`, over the argument of the
//!   quotation-section command, lowered through the ordinary rules;
//! - `\flmQuoteLines{<lines>}`, over the lines: the argument is split at
//!   every `\\`, the whitespace at both ends of each line is trimmed, and the
//!   lines are joined by `\\\relax` and a line break. A line's indentation
//!   (its leading `\indent` commands, a length of as many tabs) is written as
//!   `\flmIndent{<length>}` before its content, the length written as the
//!   [LaTeX family](crate::stdcore::backends::latex) writes every length ([`stage_length`]):
//!   `\flmIndent{1\flmTabWidth}` for one tab, `\flmIndent{2\flmTabWidth}` for
//!   two. The `\relax` after every `\\` ends that command, so that a line
//!   beginning with `[` or `*` is read as the line's text and not as an
//!   argument or a star of `\\`, regardless of what begins the line (a
//!   source comment included). The line break after `\relax` is ignored, as
//!   a space after a command name is, so no space follows.
//!
//! The rule reads the body as every backend reads it (see the [feature's
//! documentation](super)): a quotation-section command opens a quotation
//! section where it is in *section position*, written directly in the body
//! of an environment that accepts it, and each maximal run of other content
//! between two quotation sections is a run of **bare content**. The
//! whitespace, blank lines and source comments at the edges of a run are
//! written unchanged, outside any LaTeX hook, except the nodes that are
//! whitespace, such as `~` and `\ `, which are left out there, as every other
//! backend leaves them out. The rest of the run, with the
//! whitespace at both of its ends trimmed, is written through the LaTeX hook
//! of the environment's bare-content kind (a `blockquote`'s content inside
//! `\flmQuoteBlock`, an `address`'s inside `\flmQuoteLines`). Where the
//! environment reads no bare content, the run is reported
//! ([`BareContent`](super::BareContent)) and written in place, inside the
//! environment and outside any LaTeX hook. The two other errors are reported
//! as on every other backend: an `\indent` after the first content of its
//! line ([`IndentMidLine`](super::IndentMidLine)), which is left out, and a
//! quotation-section command written anywhere other than directly in a quote
//! environment's body ([`NestedSection`](super::NestedSection)), reported by the rule of the
//! command ([`SectionRule`](super::SectionRule)), which writes its content.
//!
//! # The definitions in the style file
//!
//! The feature's definition piece ([`definition_piece`],
//! `flm.latex.def.quote`; a unit of LaTeX definitions a feature registers,
//! [`LatexPreambleDef`]) defines the LaTeX hooks of the four kinds of
//! quotation section and `\flmIndent`,
//! each with `\ProvideDocumentCommand`, so that a definition that the user's
//! document class or preamble made first is kept; the user redefines any of
//! them after loading the style file. The rule requires the definition piece
//! at every quotation. The length `\flmTabWidth`, the width of one tab,
//! belongs to the LaTeX family
//! ([`TAB_WIDTH`](crate::stdcore::backends::latex::preamble::TAB_WIDTH));
//! [`stage_length`] requires the definition piece that declares it wherever
//! a line's indentation is written in tabs.
//!
//! `quote` is an environment of standard LaTeX. The environments `blockquote`
//! and `address`, which standard LaTeX does not define, are defined by the
//! same definition piece, each with `\ProvideDocumentEnvironment`, so that an
//! environment of that name defined first by the document class, a package or
//! the author's preamble is kept: `blockquote` as the one item of a list
//! indented `4em`, as `\flmQuoteBlock` sets its paragraphs, and `address` as a
//! group set in `\small`. Their text does not depend on the configuration, so
//! it belongs in a definition piece; the family's definition construct
//! ([`DEFINITION`](crate::stdcore::backends::latex::construct::DEFINITION))
//! is for text a feature builds from its configuration. An environment of any
//! other name that a host configures is written under its name, and the
//! author's LaTeX must define it.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};
use techy::error::Diagnostic;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::lang::structural::PARBREAK;
use crate::model::aspects::is_whitespace_of;
use crate::model::element::{element_of, element_value};
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{
    ARG_SLOT, ENVIRONMENT as ENVIRONMENT_CONSTRUCT, MACRO, RAW,
};
use crate::stdcore::backends::latex::preamble::stage_length;
use crate::stdcore::backends::latex::{
    LatexArgShape, LatexEnvironmentPayload, LatexMacroPayload, LatexPreambleDef, LatexRawPayload,
    preamble,
};
use crate::stdcore::backends::latex_recomposed::BACKEND;
use crate::stdcore::backends::lowering::{is_blank, lower_run, stage_trimmed_runs};
use crate::stdcore::backends::pres::BODY_SLOT;

use super::config::QuoteSection;
use super::elements::{QuoteElement, section_position};
use super::lower::{bare_content, is_content, is_line_command, is_linebreak, read_line};
use super::QUOTE;

/// The name of the lowering rule of a quotation on the `latex-recomposed`
/// backend, `flm.quote.lower.latex-recomposed` ([`QuoteLatexRule`]).
pub const LATEX_RULE: &str = "flm.quote.lower.latex-recomposed";

/// The name of the feature's definition piece, `quote`; its identifier, under
/// which a rule requires it, is `flm.latex.def.quote` ([`PIECE`]).
pub const PIECE_NAME: &str = "quote";

/// The identifier of the feature's definition piece, `flm.latex.def.quote`
/// ([`definition_piece`]).
pub const PIECE: &str = "flm.latex.def.quote";

/// The name of the LaTeX hook a text section (a quoted passage) is written
/// through, `flmQuoteText`.
pub const TEXT_HOOK: &str = "flmQuoteText";

/// The name of the LaTeX hook a lines section is written through,
/// `flmQuoteLines`.
pub const LINES_HOOK: &str = "flmQuoteLines";

/// The name of the LaTeX hook an attribution is written through,
/// `flmQuoteAttributed`.
pub const ATTRIBUTED_HOOK: &str = "flmQuoteAttributed";

/// The name of the LaTeX hook a block section is written through,
/// `flmQuoteBlock`.
pub const BLOCK_HOOK: &str = "flmQuoteBlock";

/// The name of the LaTeX hook a line's indentation is written through,
/// `flmIndent`, before the line's content: `\flmIndent{<length>}`.
pub const INDENT_HOOK: &str = "flmIndent";

/// Returns the name of the LaTeX hook a quotation section of the kind
/// `section` is written through: [`TEXT_HOOK`], [`LINES_HOOK`],
/// [`ATTRIBUTED_HOOK`] or [`BLOCK_HOOK`].
pub fn hook_of(section: QuoteSection) -> &'static str {
    match section {
        QuoteSection::Text => TEXT_HOOK,
        QuoteSection::Lines => LINES_HOOK,
        QuoteSection::Attributed => ATTRIBUTED_HOOK,
        QuoteSection::Block => BLOCK_HOOK,
    }
}

/// Returns the feature's definition piece, `flm.latex.def.quote`: the LaTeX
/// hooks of the four kinds of quotation section and the LaTeX hook
/// `\flmIndent`, each defined with
/// `\ProvideDocumentCommand`, and the two environments of the feature's
/// default configuration that standard LaTeX does not define:
///
/// - `\flmQuoteText{<passage>}` — the passage, ending a paragraph:
///   `#1\par`;
/// - `\flmQuoteLines{<lines>}` — the lines, each ended by `\\\relax`, set
///   with no paragraph indentation, in a group of their own, the paragraph
///   started before the first line, so that an empty first line has a line
///   to end:
///   `\par\begingroup\setlength{\parindent}{0pt}\leavevmode#1\par\endgroup`;
/// - `\flmQuoteAttributed{<attribution>}` — the attribution, on a line of
///   its own set to the right after a dash:
///   `\par\begingroup\raggedleft---~#1\par\endgroup`;
/// - `\flmQuoteBlock{<paragraphs>}` — paragraphs and blocks, indented as the
///   one item of a list with no label, its left margin `4em` and its first
///   line indented as a paragraph: `\par\list{}{\itemindent\parindent
///   \leftmargin=4em\relax}\item\relax #1\endlist`;
/// - `\flmIndent{<length>}` — space of that length at the start of a line
///   that no line break removes: `\hspace*{#1}`;
/// - the environments `blockquote` and `address`, defined with
///   `\ProvideDocumentEnvironment`: `blockquote` as the list that
///   `\flmQuoteBlock` sets,
///   `address` as a group set `\small`
///   (`\ProvideDocumentEnvironment{address}{}{\par\begingroup\small}{\par\endgroup}`).
///
/// The environments' text does not depend on the configuration, so it
/// belongs to the definition piece, which is written into the style file of every
/// document holding a quotation; the family's definition construct is for
/// text a feature builds from its configuration. A definition of the same
/// name made before the style file is loaded is kept.
///
/// The LaTeX hooks of the four kinds of quotation section take a long
/// argument (`+m`), so that content holding a paragraph break is still read
/// as the argument instead of stopping the compilation. Redefine a LaTeX
/// hook, before loading the style file
/// or after it, to set that quotation section differently. See the [module
/// documentation](self).
pub fn definition_piece() -> LatexPreambleDef {
    // The definitions of `blockquote` and `address` are the two of the Python
    // implementation's recomposer.
    LatexPreambleDef::new(
        PIECE_NAME,
        "\
% Quotations: one command for each kind of quotation section (a passage,
% lines, an attribution, a block), written between the environment's \\begin
% and \\end lines.
% \\flmQuoteText{<passage>}: a quoted passage, ending a paragraph.
% \\flmQuoteLines{<lines>}: lines set as written, each ended by \\\\\\relax, with no
%   paragraph indentation; a line's indentation is \\flmIndent{<length>} at
%   its start, a tab being \\flmTabWidth.
% \\flmQuoteAttributed{<attribution>}: the attribution, on a line of its own
%   set to the right, after a dash.
% \\flmQuoteBlock{<paragraphs>}: paragraphs and blocks, indented as the one
%   item of a list with no label, its left margin 4em.
% \\flmIndent{<length>}: space of <length> at the start of a line.
% Redefine one of these commands to set that kind of quotation section
% differently.
% The environments blockquote (set as \\flmQuoteBlock sets its paragraphs) and
% address (a group set in \\small), which standard LaTeX does not define, are
% defined below; a definition made before this file is loaded is kept.
\\ProvideDocumentCommand\\flmQuoteText{+m}{#1\\par}
\\ProvideDocumentCommand\\flmQuoteLines{+m}{\\par\\begingroup\\setlength{\\parindent}{0pt}\\leavevmode#1\\par\\endgroup}
\\ProvideDocumentCommand\\flmQuoteAttributed{+m}{\\par\\begingroup\\raggedleft---~#1\\par\\endgroup}
\\ProvideDocumentCommand\\flmQuoteBlock{+m}{\\par\\list{}{\\itemindent\\parindent\\leftmargin=4em\\relax}\\item\\relax #1\\endlist}
\\ProvideDocumentCommand\\flmIndent{m}{\\hspace*{#1}}
\\ProvideDocumentEnvironment{blockquote}{}{\\par\\list{}{\\itemindent\\parindent\\leftmargin=4em\\relax}\\item\\relax}{\\endlist}
\\ProvideDocumentEnvironment{address}{}{\\par\\begingroup\\small}{\\par\\endgroup}
",
        "`\\flmQuoteText`, `\\flmQuoteLines`, `\\flmQuoteAttributed`, `\\flmQuoteBlock`: the \
         LaTeX hooks of the four kinds of quotation section; `\\flmIndent{length}`: a line's \
         indentation; the environments `blockquote` and `address`",
    )
}

/// The implementation of the lowering rule [`LATEX_RULE`], the default on the
/// `latex-recomposed` backend: it requires the definition piece [`PIECE`]
/// and stages the environment of the quotation's name around its body, each
/// quotation section through its LaTeX hook. See the [module
/// documentation](self).
///
/// # Errors
///
/// [`LowerError`] when the node carries no [`QuoteElement`], and any error from lowering the
/// content.
#[derive(Debug, Default, Clone, Copy)]
pub struct QuoteLatexRule;

impl Lower for QuoteLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<QuoteElement>(&node)
            .ok_or_else(|| LowerError::structure("a quotation without its value"))?;
        lw.require(&Name::from(PIECE));
        // The body read as the general rule reads it: sections in section
        // position, and the maximal runs of other content between them.
        let body: Vec<NodeRef<'_, Flm>> =
            node.body().map(|body| body.iter().collect()).unwrap_or_default();
        let mut content = Vec::new();
        let mut start = 0;
        for (at, child) in body.iter().enumerate() {
            if let Some(position) = section_position(child) {
                bare_run(cx, lw, value, &body[start..at], &mut content)?;
                let section = match position.section {
                    QuoteSection::Lines => {
                        let nodes: Vec<NodeRef<'_, Flm>> = child
                            .argument_content_nodes(0)
                            .map(|nodes| nodes.iter().collect())
                            .unwrap_or_default();
                        stage_lines(cx, lw, &nodes)?
                    }
                    _ => lw.lower_arg(cx, child, 0)?,
                };
                content.push(hook(cx, lw, position.section, section)?);
                start = at + 1;
            }
        }
        bare_run(cx, lw, value, &body[start..], &mut content)?;
        let environment = lw.pres_slots_with(
            cx,
            &ElementName::from(ENVIRONMENT_CONSTRUCT),
            LatexEnvironmentPayload::new(value.name.as_str(), Vec::new()),
            vec![(String::from(BODY_SLOT), content)],
        )?;
        Ok(Lowering::Emit(vec![environment]))
    }
}

/// Stages `run`, a maximal run of bare content between two quotation
/// sections of the quotation `value`: the whitespace, blank lines and source
/// comments at its edges unchanged, the nodes that are whitespace there left
/// out ([`lower_edge`]), and the rest through the LaTeX hook of
/// the environment's bare-content kind, with the whitespace at both of its
/// ends trimmed as a quotation section's content is; or, where the
/// environment reads no bare content, reported once and written in place.
fn bare_run(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    value: &QuoteElement,
    run: &[NodeRef<'_, Flm>],
    out: &mut Vec<BuildId>,
) -> Result<(), LowerError> {
    let Some(first) = run.iter().position(|node| !is_edge(node)) else {
        out.extend(lower_edge(cx, lw, run)?);
        return Ok(());
    };
    let last = run.iter().rposition(|node| !is_edge(node)).unwrap_or(first);
    out.extend(lower_edge(cx, lw, &run[..first])?);
    let core = &run[first..=last];
    match value.environment.bare {
        Some(QuoteSection::Lines) => {
            let lines = stage_lines(cx, lw, core)?;
            out.push(hook(cx, lw, QuoteSection::Lines, lines)?);
        }
        Some(kind) => {
            let content = stage_trimmed_runs(cx, lw, &[core], is_line_command)?;
            out.push(hook(cx, lw, kind, content)?);
        }
        None => {
            lw.diagnose(Diagnostic::error(bare_content(value), core[0].span().clone()));
            out.extend(lower_run(cx, lw, core)?);
        }
    }
    out.extend(lower_edge(cx, lw, &run[last + 1..])?);
    Ok(())
}

/// Returns whether `node`, at the edge of a bare run, stands outside any
/// LaTeX hook: whitespace (`~` and `\ ` included, [`is_blank`]), a source
/// comment, or the paragraph-break node that a blank line leaves in the tree
/// this backend lowers, which has not been through the block-structure pass.
fn is_edge(node: &NodeRef<'_, Flm>) -> bool {
    node.is_comment() || is_blank(node) || element_of(node).name.as_str() == PARBREAK
}

/// Lowers `edge`, nodes at the edge of a bare run ([`is_edge`]), as written,
/// except the nodes that are whitespace, such as `~` and `\ `
/// ([`is_whitespace_of`]), which are left out: LaTeX would typeset them
/// there, and every other backend removes them.
fn lower_edge(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    edge: &[NodeRef<'_, Flm>],
) -> Result<Vec<BuildId>, LowerError> {
    let mut out = Vec::new();
    for run in edge.split(is_whitespace_of) {
        out.extend(lower_run(cx, lw, run)?);
    }
    Ok(out)
}

/// Stages the lines of a lines section over `content`: split at every `\\`,
/// each line's indentation as `\flmIndent{<length>}` before its content, the
/// whitespace at both ends of the line trimmed, the lines joined by
/// `\\\relax` and a line break. A lines section that holds neither content
/// nor a `\\` has no line.
fn stage_lines(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    content: &[NodeRef<'_, Flm>],
) -> Result<Vec<BuildId>, LowerError> {
    let mut out = Vec::new();
    if !content.iter().any(|node| is_content(node) || is_linebreak(node)) {
        return Ok(out);
    }
    for (index, line) in content.split(is_linebreak).enumerate() {
        if index > 0 {
            out.push(raw(cx, lw, "\\\\\\relax\n")?);
        }
        let (indent, runs) = read_line(lw, line);
        if !indent.is_zero() {
            let length = stage_length(cx, lw, &indent)?;
            out.push(lw.pres_slots_with(
                cx,
                &ElementName::from(MACRO),
                LatexMacroPayload::new(INDENT_HOOK, [LatexArgShape::Mandatory]),
                vec![(String::from(ARG_SLOT), vec![length])],
            )?);
        }
        out.extend(stage_trimmed_runs(cx, lw, &runs, is_line_command)?);
    }
    Ok(out)
}

/// Stages a call of the LaTeX hook of the kind `section` with `content` as
/// its argument.
fn hook(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    section: QuoteSection,
    content: Vec<BuildId>,
) -> Result<BuildId, LowerError> {
    lw.pres_slots_with(
        cx,
        &ElementName::from(MACRO),
        LatexMacroPayload::new(hook_of(section), [LatexArgShape::Mandatory]),
        vec![(String::from(ARG_SLOT), content)],
    )
}

/// Stages a raw node (`flm.pres.latex.raw`): LaTeX text written byte for
/// byte.
fn raw(cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer, text: &str) -> Result<BuildId, LowerError> {
    lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(text), Vec::new())
}

/// Returns the lowering rule [`LATEX_RULE`], ready to be registered: the
/// default on the `latex-recomposed` backend only.
pub fn latex_rule() -> LoweringRule {
    LoweringRule::new(
        LATEX_RULE,
        QUOTE,
        LoweringDecl::new()
            .produces(ENVIRONMENT_CONSTRUCT)
            .produces(MACRO)
            .produces(RAW),
        Arc::new(QuoteLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "a quotation lowers to the environment of its name, each quotation section through the \
         LaTeX hook of its kind, and a line's indentation through `\\flmIndent`",
    )
}

/// Registers what the feature contributes to the `latex-recomposed` backend:
/// a peer declaration ([`PeerName`](crate::core::registry::PeerName)) of the
/// backend and of each construct element the rule stages, the definition
/// piece ([`definition_piece`]) as a contribution to the service slot of
/// definition pieces ([`preamble::SLOT`]), then [`latex_rule`]. A registry
/// without the LaTeX features still builds, with the rule set aside (excluded
/// from rule selection; see [`PeerName`](crate::core::registry::PeerName)).
///
/// # Errors
///
/// The [`BuildError`] of the registry builder.
pub(super) fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the quotation's rule \
         `flm.quote.lower.latex-recomposed` is the default",
    )?;
    b.peer_element::<LatexEnvironmentPayload>(
        ENVIRONMENT_CONSTRUCT,
        "the LaTeX family's environment construct, which the recomposition rule stages",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro-call construct, which the recomposition rule stages for the \
         LaTeX hooks of the quotation sections and of a line's indentation",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw node construct, in which the recomposition rule writes the \
         line breaks between lines and the length of a line's indentation",
    )?;
    b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(definition_piece()))?;
    b.lowering_rule(latex_rule())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_piece_provides_the_four_hooks_and_the_indentation() {
        let piece = definition_piece();
        assert_eq!(piece.id.as_str(), PIECE);
        for hook in [TEXT_HOOK, LINES_HOOK, ATTRIBUTED_HOOK, BLOCK_HOOK, INDENT_HOOK] {
            assert!(
                piece.content.contains(&format!("\\ProvideDocumentCommand\\{hook}{{")),
                "{hook}"
            );
        }
        assert!(piece.requires.is_empty());
    }

    #[test]
    fn the_piece_provides_only_the_two_environments_latex_lacks() {
        let piece = definition_piece();
        for name in ["blockquote", "address"] {
            let definition = format!("\\ProvideDocumentEnvironment{{{name}}}{{}}");
            assert_eq!(piece.content.matches(&definition).count(), 1, "{name}");
        }
        assert!(!piece.content.contains("Environment{quote}"));
    }
}
