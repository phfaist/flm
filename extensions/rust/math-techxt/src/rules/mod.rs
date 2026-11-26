//! The feature's lowering rules.
//!
//! - **The commands lowered on their own** ([`register_command_rules`]): a
//!   macro or an environment of techxt's reached outside a formula is written
//!   as its source, on every backend, so that its element is lowerable wherever
//!   it occurs. Inside a formula the rule of the formula writes it.
//! - **The formulas on `text` and `html`** ([`register_formula_rules`]): a
//!   math group's row of the table [`RENDERED_TABLE`] is staged as the
//!   feature's element [`RENDERED_AS_TEXT`] ([`Rendered`]), which carries the
//!   rendered lines and, in a content slot `hole-<i>` per text command, that
//!   command lowered through the rules. A display-math environment is staged
//!   as one such element whose body holds one element per equation line, each
//!   carrying the line's anchor and, in its slot `tag`, what the line shows at
//!   its tag. A formula without a row (the stage reported why) is staged as a
//!   `flm.pres.math-passthrough` node over its TeX instead.
//! - **The element** ([`text`], [`html`]): another round of lowering writes
//!   the element with the rule of the backend at hand, in that backend's own
//!   elements.
//! - **The formulas on `latex-rendered` and `latex-recomposed`** ([`latex`]):
//!   the library's own formula rules, registered under names of the feature;
//!   techxt plays no part there.

pub mod html;
pub mod latex;
pub mod text;

use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::lowering::{
    DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::stdlib::backends::pres::{BODY_SLOT, MATH_PASSTHROUGH, MATH_TEXT};
use flm_ext::guest::stdlib::features::math::{
    ANCHORS, COUNTERS, ENVIRONMENT as MATH_ENVIRONMENT, FrameReader, LineFrame, MATH, TexPart, formula_display, group_tex,
    line_frames, line_tex, stage_tag_content,
};
use flm_ext::guest::lang::rewrites::userdefs::StagedNodes;
use flm_ext::guest::techy::node::{Node, Nodes};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::math_techxt::render::{RENDERED_TABLE, RenderedFormula, RenderedNode, Row};
use crate::math_techxt::{ENVIRONMENT, MACRO, RENDERED_AS_TEXT};

/// The name of the content slot of the element [`RENDERED_AS_TEXT`] that holds
/// hole `index`'s content: `hole-<index>`.
pub fn hole_slot(index: usize) -> String {
    format!("hole-{index}")
}

/// The name of the content slot of an equation line's element
/// [`RENDERED_AS_TEXT`] that holds what the line shows at its tag: `tag`.
pub const TAG_SLOT: &str = "tag";

/// The payload of the element [`RENDERED_AS_TEXT`]: a formula rendered by
/// techxt, its holes marked, written in JSON with its kind under `kind`
/// (`formula`, `environment` or `line`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Rendered {
    /// A math group: its display and its rendered lines, its holes' content
    /// in its slots [`hole_slot`].
    Formula {
        /// Whether the formula is inline or displayed.
        display: Display,
        /// The rendered lines, each a list of segments.
        lines: Vec<Vec<Segment>>,
    },
    /// A display-math environment: its name. Its body (the slot `body`)
    /// holds one element per equation line, in order, each a
    /// [`Rendered::Line`].
    Environment {
        /// The environment's name.
        env: String,
    },
    /// An equation line, displayed, in an environment's body: its rendered
    /// lines and its anchor, its holes' content in its slots [`hole_slot`],
    /// and what it shows at its tag in its slot [`TAG_SLOT`], when it shows
    /// something.
    Line {
        /// The rendered lines, each a list of segments.
        lines: Vec<Vec<Segment>>,
        /// The line's anchor, when it has one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        anchor: Option<String>,
    },
}

/// How a formula stands in the text: within a line, or displayed. Its JSON
/// form, `inline` or `display`, is also the `display` that the payload of
/// `flm.pres.math-passthrough` holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Display {
    /// Within a line.
    Inline,
    /// Displayed on lines of its own.
    Display,
}

/// A segment of a rendered line: a text, or the index of a hole, whose
/// content is in the slot [`hole_slot`] of that index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Segment {
    /// Rendered text.
    Text(String),
    /// The index of a hole.
    Hole(usize),
}

/// The payload of `node`, an element [`RENDERED_AS_TEXT`].
///
/// # Errors
///
/// The node carries no payload, or one that is not a rendered formula
/// ([`ExtError::Value`]).
pub(crate) fn rendered(node: &Node) -> Result<Rendered, ExtError> {
    let payload = node.payload()?.ok_or_else(|| ExtError::Value {
        detail: format!("Misconfigured element ‘{RENDERED_AS_TEXT}’: the element must carry its payload"),
    })?;
    serde_json::from_value(payload).map_err(|error| ExtError::Value {
        detail: format!("Failed to read the payload of an element ‘{RENDERED_AS_TEXT}’ as a rendered formula: {error}"),
    })
}

/// The rendered lines and the anchor of `node`, an element
/// [`RENDERED_AS_TEXT`] of a display-math environment's body.
///
/// # Errors
///
/// The node's payload is not an equation line's ([`ExtError::Value`]), or
/// [`rendered`]'s errors.
pub(crate) fn equation_line(node: &Node) -> Result<(Vec<Vec<Segment>>, Option<String>), ExtError> {
    match rendered(node)? {
        Rendered::Line { lines, anchor } => Ok((lines, anchor)),
        Rendered::Formula { .. } | Rendered::Environment { .. } => Err(ExtError::Value {
            detail: format!(
                "Misconfigured element ‘{RENDERED_AS_TEXT}’: the body of an environment's element must hold equation \
                 lines"
            ),
        }),
    }
}

/// The error of an equation line's element [`RENDERED_AS_TEXT`] found
/// outside an environment's body.
pub(crate) fn stray_line() -> ExtError {
    ExtError::Value {
        detail: format!(
            "Misconfigured element ‘{RENDERED_AS_TEXT}’: an equation line's element must stand in an environment's \
             body"
        ),
    }
}

/// The content of the slot `name` of `node`, or `None` when the node has no
/// slot of that name.
pub(crate) fn slot_named(node: &Node, name: &str) -> Result<Option<Nodes>, ExtError> {
    for index in 0..node.slot_count()? {
        if node.slot_name(index)?.as_deref() == Some(name) {
            return node.slot_content_nodes(index);
        }
    }
    Ok(None)
}

/// What the equation line `line` shows at its tag: its slot [`TAG_SLOT`]'s
/// content, none when the line has no such slot.
pub(crate) fn tag_content(line: &Node) -> Result<Nodes, ExtError> {
    Ok(slot_named(line, TAG_SLOT)?.unwrap_or_default())
}

/// Registers the rules of the commands lowered on their own: one for the
/// element [`MACRO`] and one for [`ENVIRONMENT`], each the default on every
/// backend, writing the node's source as text.
///
/// # Errors
///
/// The host's refusal.
pub fn register_command_rules(b: &mut FeatureBuilder) -> Result<(), ExtError> {
    let rules = [
        (MACRO, "math-techxt.macro.lower.source", "A macro of techxt's lowers to its source; the rule of the formula around it writes it otherwise."),
        (ENVIRONMENT, "math-techxt.environment.lower.source", "An environment of techxt's lowers to its source; the rule of the formula around it writes it otherwise."),
    ];
    for (element, name, docs) in rules {
        let rule = LoweringRule::new(name, element, LoweringDecl::new(), Arc::new(Source), DefaultMark::Everywhere)?;
        b.lowering_rule(rule.docs(docs)?)?;
    }
    Ok(())
}

/// Declares the two tables the rules of the display-math environments read
/// optionally: the anchors table of the references feature and the counter
/// table of the numbering feature.
///
/// # Errors
///
/// The host's refusal.
pub fn declare_peer_tables(b: &mut FeatureBuilder) -> Result<(), ExtError> {
    b.peer_table(ANCHORS, "the references feature's anchors, read for the anchor of each equation line")?;
    b.peer_table(COUNTERS, "the numbering feature's counters, read for the number of each equation line")
}

/// Registers the two formula rules of the backend `backend`: the rule of
/// `flm.math`, `math-techxt.math.lower.<backend>`, and the rule of
/// `flm.math.environment`, `math-techxt.math-environment.lower.<backend>`,
/// each marked for that backend alone. Both read the table
/// [`RENDERED_TABLE`] and stage the element [`RENDERED_AS_TEXT`], or
/// `flm.pres.math-passthrough` for a formula without a row.
///
/// # Errors
///
/// The host's refusal.
pub fn register_formula_rules(b: &mut FeatureBuilder, backend: &str) -> Result<(), ExtError> {
    let mark = || DefaultMark::OnBackends(vec![backend.into()]);
    let decl = || LoweringDecl::new().reads(RENDERED_TABLE).produces(RENDERED_AS_TEXT).produces(MATH_PASSTHROUGH);
    let formula = LoweringRule::new(format!("math-techxt.math.lower.{backend}"), MATH, decl(), Arc::new(Formula), mark())?;
    b.lowering_rule(formula.docs(
        "Inline or displayed mathematics lowers to `math-techxt.rendered-as-text`: the formula rendered by \
         techxt, each text command lowered through the rules into a slot of its own.",
    )?)?;
    let environment = LoweringRule::new(
        format!("math-techxt.math-environment.lower.{backend}"),
        MATH_ENVIRONMENT,
        decl().produces(MATH_TEXT).reads_opt(ANCHORS).reads_opt(COUNTERS),
        Arc::new(Environment),
        mark(),
    )?;
    b.lowering_rule(environment.docs(
        "A display-math environment lowers to `math-techxt.rendered-as-text` holding one such element per \
         equation line: the line rendered by techxt, each text command lowered into a slot of its own, the \
         line's anchor, and its number or tag in a slot of its own.",
    )?)
}

/// The body of the rules of the commands lowered on their own: the node's
/// source, as text.
struct Source;

impl Lower for Source {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        Ok(Lowering::Emit(vec![lw.text(cx, &node.span_content()?)?].into()))
    }
}

/// The row of the table [`RENDERED_TABLE`] for `node`, if the stage wrote
/// one.
fn row_of(lw: &Lowerer, node: &Node) -> Result<Option<Row>, ExtError> {
    let Some(row) = lw.get_at(RENDERED_TABLE, node)?.unwrap_or_default().into_iter().next() else { return Ok(None) };
    serde_json::from_value(row).map(Some).map_err(|error| ExtError::Value {
        detail: format!("Failed to read a row of the table ‘{RENDERED_TABLE}’ as a rendered math group or environment: {error}"),
    })
}

/// The payload `rendered` as data.
fn payload(rendered: &Rendered) -> Result<Value, ExtError> {
    serde_json::to_value(rendered)
        .map_err(|error| ExtError::Value { detail: format!("Failed to serialize a rendered formula: {error}") })
}

/// The text command node of each hole of `parts`, in order.
fn hole_nodes(parts: Vec<TexPart>) -> impl Iterator<Item = Node> {
    parts.into_iter().filter_map(|part| match part {
        TexPart::Hole(hole) => Some(hole.node),
        TexPart::Tex(_) => None,
    })
}

/// The slots `hole-<i>` of the text command nodes `holes`, each lowered
/// through the rules.
fn hole_slots(
    cx: &mut LowerCx,
    lw: &mut Lowerer,
    holes: impl Iterator<Item = Node>,
) -> Result<Vec<(String, StagedNodes)>, ExtError> {
    let mut slots = Vec::new();
    for (index, hole) in holes.enumerate() {
        slots.push((hole_slot(index), lw.lower_nodes(cx, &Nodes::from(vec![hole]))?));
    }
    Ok(slots)
}

/// Stages the writer's parts `parts` as the body of a
/// `flm.pres.math-passthrough` node, appended to `body`: each TeX part as
/// text, each hole's text command lowered through the rules.
fn stage_parts(cx: &mut LowerCx, lw: &mut Lowerer, parts: Vec<TexPart>, body: &mut StagedNodes) -> Result<(), ExtError> {
    for part in parts {
        match part {
            TexPart::Tex(text) => body.push(lw.text(cx, &text)?),
            TexPart::Hole(hole) => body.push_all(&lw.lower_nodes(cx, &Nodes::from(vec![hole.node]))?),
        }
    }
    Ok(())
}

/// The body of the rule of `flm.math`: the math group's row of the table
/// [`RENDERED_TABLE`], staged as the element [`RENDERED_AS_TEXT`] with each
/// hole's text command, found again through the writer, lowered into its
/// slot. A math group without a row is staged as a
/// `flm.pres.math-passthrough` node over its TeX.
struct Formula;

impl Lower for Formula {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let Some(row) = row_of(lw, node)? else {
            let display = if formula_display(node)? { Display::Display } else { Display::Inline };
            let mut body = StagedNodes::default();
            stage_parts(cx, lw, group_tex(node)?, &mut body)?;
            let math = lw.pres(cx, MATH_PASSTHROUGH, &json!({ "display": display }), body)?;
            return Ok(Lowering::Emit(vec![math].into()));
        };
        let RenderedNode::Group { display, formula: RenderedFormula { lines, .. } } = row.rendered else {
            return Err(ExtError::Value {
                detail: format!(
                    "Misconfigured table ‘{RENDERED_TABLE}’: the row at a math group must be a math group's row"
                ),
            });
        };
        let payload = payload(&Rendered::Formula { display, lines })?;
        let slots = hole_slots(cx, lw, hole_nodes(group_tex(node)?))?;
        Ok(Lowering::Emit(vec![lw.pres_slots(cx, RENDERED_AS_TEXT, &payload, slots)?].into()))
    }
}

/// The body of the rule of `flm.math.environment`: the environment's row of
/// the table [`RENDERED_TABLE`], staged as the element [`RENDERED_AS_TEXT`]
/// whose body holds one such element per equation line, with the line's
/// anchor and what it shows at its tag, read from the line's frame. An
/// environment without a row is staged as a `flm.pres.math-passthrough` node
/// over its lines' TeX ([`environment_passthrough`]).
struct Environment;

impl Lower for Environment {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        let frames = line_frames(FrameReader::Rule(*lw), true, true, node)?;
        let name = node.name()?.unwrap_or_default();
        let Some(row) = row_of(lw, node)? else { return environment_passthrough(cx, lw, &name, &frames) };
        let RenderedNode::Environment { lines: formulas, .. } = row.rendered else {
            return Err(ExtError::Value {
                detail: format!(
                    "Misconfigured table ‘{RENDERED_TABLE}’: the row at a ‘{name}’ environment must be an \
                     environment's row"
                ),
            });
        };
        if formulas.len() != frames.len() {
            return Err(ExtError::Value {
                detail: format!(
                    "Misconfigured table ‘{RENDERED_TABLE}’: the row of a ‘{name}’ environment must hold one formula \
                     per equation line ({} lines, {} formulas)",
                    frames.len(),
                    formulas.len()
                ),
            });
        }
        let mut lines = Vec::new();
        let mut after_line_end = false;
        for (frame, RenderedFormula { lines: rendered, .. }) in frames.iter().zip(formulas) {
            let tex = line_tex(&frame.line.content, after_line_end)?;
            after_line_end = tex.ends_line();
            let mut slots = hole_slots(cx, lw, hole_nodes(tex.parts))?;
            let tag = stage_tag_content(cx, lw, frame)?;
            if !tag.is_empty() {
                slots.push((TAG_SLOT.to_string(), tag));
            }
            let line = Rendered::Line { lines: rendered, anchor: frame.anchor.clone() };
            lines.push(lw.pres_slots(cx, RENDERED_AS_TEXT, &payload(&line)?, slots)?);
        }
        let environment = Rendered::Environment { env: name };
        let staged = lw.pres_slots(cx, RENDERED_AS_TEXT, &payload(&environment)?, vec![(BODY_SLOT.to_string(), lines.into())])?;
        Ok(Lowering::Emit(vec![staged].into()))
    }
}

/// A display-math environment `name` without a row, staged as a displayed
/// `flm.pres.math-passthrough` node carrying its name over its equation
/// lines `frames`: for each line, an empty `flm.pres.math-text` node carrying
/// the line's anchor, the line's TeX ([`stage_parts`]), `\tag*{` around a
/// `flm.pres.math-text` node marked `at_tag` holding what the line shows at
/// its tag, the line's `\\` with its spacing, and the run of whitespace that
/// ended the line.
fn environment_passthrough(
    cx: &mut LowerCx,
    lw: &mut Lowerer,
    name: &str,
    frames: &[LineFrame],
) -> Result<Lowering, ExtError> {
    let mut body = StagedNodes::default();
    let mut after_line_end = false;
    for frame in frames {
        if let Some(anchor) = &frame.anchor {
            body.push(lw.pres(cx, MATH_TEXT, &json!({ "anchor": anchor }), Vec::new().into())?);
        }
        let tex = line_tex(&frame.line.content, after_line_end)?;
        after_line_end = tex.ends_line();
        stage_parts(cx, lw, tex.parts, &mut body)?;
        let tag = stage_tag_content(cx, lw, frame)?;
        if !tag.is_empty() {
            body.push(lw.text(cx, "\\tag*{")?);
            body.push(lw.pres(cx, MATH_TEXT, &json!({ "at_tag": true }), tag)?);
            body.push(lw.text(cx, "}")?);
        }
        if let Some(linebreak) = &frame.line.linebreak {
            let spacing = linebreak.spacing.as_ref().map(|spacing| format!("[{spacing}]")).unwrap_or_default();
            body.push(lw.text(cx, &format!("\\\\{spacing}"))?);
        }
        if !tex.trailing.is_empty() {
            body.push(lw.text(cx, &tex.trailing)?);
        }
    }
    let payload = json!({ "display": Display::Display, "env": name });
    Ok(Lowering::Emit(vec![lw.pres(cx, MATH_PASSTHROUGH, &payload, body)?].into()))
}
