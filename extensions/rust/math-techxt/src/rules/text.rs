//! The rule of the element [`RENDERED_AS_TEXT`] on the backend `text`.
//!
//! - An inline formula is its segments as text, which the backend fills into
//!   the line and may wrap at its spaces, each hole as its slot's content
//!   (techxt writes an inline formula on one line; lines it would write
//!   beyond the first follow the first after a space).
//! - A displayed formula is a `flm.pres.lines` node of one `flm.pres.line` per
//!   rendered line, each indented by two columns (`1em`), each text segment an
//!   inline `flm.pres.verbatim` atom, so that its runs of spaces and its
//!   leading spaces are kept and the drawing stays aligned, and each hole its
//!   slot's content, shown as the backend shows that content anywhere (a
//!   reference as its number).
//! - A display-math environment is one such `flm.pres.lines` node over its
//!   equation lines in order, each line's tag after the line's last printed
//!   line, separated from it by one space.

use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::lowering::{DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule, Staged};
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::lang::rewrites::userdefs::StagedNodes;
use flm_ext::guest::stdlib::backends::pres::{BODY_SLOT, LINE, LINES, VERBATIM};
use flm_ext::guest::techy::node::Node;
use serde_json::json;

use super::{Display, Rendered, Segment, equation_line, hole_slot, rendered, stray_line, tag_content};
use crate::math_techxt::RENDERED_AS_TEXT;

/// The name of the text backend.
pub const BACKEND: &str = "text";

/// The indentation of each line of a displayed formula: one em, which the
/// text backend sets as two columns.
const DISPLAY_INDENT: &str = "1em";

/// Registers the rule of the element [`RENDERED_AS_TEXT`] on `text`,
/// `math-techxt.rendered-as-text.lower.text`.
///
/// # Errors
///
/// The host's refusal.
pub fn register_element_rule(b: &mut FeatureBuilder) -> Result<(), ExtError> {
    let rule = LoweringRule::new(
        "math-techxt.rendered-as-text.lower.text",
        RENDERED_AS_TEXT,
        LoweringDecl::new().produces(LINES).produces(LINE).produces(VERBATIM),
        Arc::new(RenderedAsText),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )?;
    b.lowering_rule(rule.docs(
        "A formula rendered as Unicode text lowers to its rendered text, each text command's content in its \
         place: an inline formula within the line, a displayed one on lines of its own, its spaces kept.",
    )?)
}

/// The body of the rule. See the module documentation.
struct RenderedAsText;

impl Lower for RenderedAsText {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let (display, lines) = match rendered(node)? {
            Rendered::Formula { display, lines } => (display, lines),
            Rendered::Environment { .. } => {
                let mut lines = Vec::new();
                for line in node.slot_content_nodes_named(BODY_SLOT)?.to_vec()? {
                    let (rendered_lines, _) = equation_line(&line)?;
                    let mut printed = displayed_lines(cx, lw, &line, &rendered_lines)?;
                    let tag = lw.lower_nodes(cx, &tag_content(&line)?)?;
                    if !tag.is_empty() {
                        match printed.last_mut() {
                            Some(last) => {
                                last.push(lw.text(cx, " ")?);
                                last.push_all(&tag);
                            }
                            None => printed.push(tag),
                        }
                    }
                    lines.extend(printed);
                }
                return Ok(Lowering::Emit(vec![lines_node(cx, lw, lines)?].into()));
            }
            Rendered::Line { .. } => return Err(stray_line()),
        };
        match display {
            Display::Inline => {
                let mut staged = StagedNodes::default();
                for (number, line) in lines.iter().enumerate() {
                    if number > 0 {
                        staged.push(lw.text(cx, " ")?);
                    }
                    for segment in line {
                        match segment {
                            Segment::Text(text) => staged.push(lw.text(cx, text)?),
                            Segment::Hole(index) => staged.push_all(&hole(cx, lw, node, *index)?),
                        }
                    }
                }
                Ok(Lowering::Emit(staged))
            }
            Display::Display => {
                let lines = displayed_lines(cx, lw, node, &lines)?;
                Ok(Lowering::Emit(vec![lines_node(cx, lw, lines)?].into()))
            }
        }
    }
}

/// The content of hole `index` of the element `node`, lowered.
fn hole(cx: &mut LowerCx, lw: &mut Lowerer, node: &Node, index: usize) -> Result<StagedNodes, ExtError> {
    let content = node.slot_content_nodes_named(&hole_slot(index))?;
    lw.lower_nodes(cx, &content)
}

/// The content of each of `lines`, the rendered lines of the element `node`:
/// each text segment an inline `flm.pres.verbatim` atom, each hole its slot's
/// content.
fn displayed_lines(
    cx: &mut LowerCx,
    lw: &mut Lowerer,
    node: &Node,
    lines: &[Vec<Segment>],
) -> Result<Vec<StagedNodes>, ExtError> {
    let mut printed = Vec::new();
    for line in lines {
        let mut staged = StagedNodes::default();
        for segment in line {
            match segment {
                Segment::Text(text) => {
                    let text = lw.text(cx, text)?;
                    staged.push(lw.pres(cx, VERBATIM, &json!({ "kind": "text", "display": "inline" }), vec![text].into())?);
                }
                Segment::Hole(index) => staged.push_all(&hole(cx, lw, node, *index)?),
            }
        }
        printed.push(staged);
    }
    Ok(printed)
}

/// A `flm.pres.lines` node of one `flm.pres.line` per entry of `lines`, each
/// indented by [`DISPLAY_INDENT`].
fn lines_node(cx: &mut LowerCx, lw: &mut Lowerer, lines: Vec<StagedNodes>) -> Result<Staged, ExtError> {
    let mut staged = StagedNodes::default();
    for content in lines {
        staged.push(lw.pres(cx, LINE, &json!({ "indent_left": DISPLAY_INDENT }), content)?);
    }
    lw.pres(cx, LINES, &json!({}), staged)
}
