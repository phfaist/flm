//! The rule of the element [`RENDERED_AS_TEXT`] on the backend `html`, and
//! its stylesheet [`STYLE`].
//!
//! The rule writes `<span>` elements (`flm.pres.html.tag`) with the
//! extension's own classes, which are not part of FLM's HTML contract; the
//! outermost span of a formula also carries the mark of a formula that the
//! `math` library gives (`formula_attributes`: the class `flm-math-formula`,
//! `data-flm-display`, and `data-flm-env` for an environment):
//!
//! | what | written as |
//! |---|---|
//! | an inline formula | `<span class="math-techxt flm-math-formula" data-flm-display="inline">` holding its lines |
//! | a displayed formula | `<span class="math-techxt math-techxt-display flm-math-formula" data-flm-display="display">` holding its lines |
//! | a display-math environment | `<span class="math-techxt math-techxt-display flm-math-formula" data-flm-display="display" data-flm-env="…">` holding its equation lines |
//! | an equation line | `<span class="math-techxt-line">`, its anchor as `id`, holding `<span class="math-techxt-text">` with its lines, then its tag in `<span class="math-techxt-tag">` |
//!
//! A formula's lines are written as text separated by newlines, each text
//! segment as given and each hole as its slot's content. The stylesheet keeps
//! the white space and sets the formulas in JuliaMono, so that a drawing over
//! several lines stays aligned. The rule requires the stylesheet and the
//! standard `flm.html.style.math-passthrough`, which sets the text inside a
//! formula (a `flm.pres.math-text` node) upright.

use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::lowering::{DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule, Staged};
use flm_ext::guest::core::registry::{DefContent, FeatureBuilder, StyleDef};
use flm_ext::guest::lang::rewrites::userdefs::StagedNodes;
use flm_ext::guest::stdlib::backends::html::TAG;
use flm_ext::guest::stdlib::backends::pres::BODY_SLOT;
use flm_ext::guest::stdlib::features::math::formula_attributes;
use flm_ext::guest::techy::node::Node;
use serde_json::{Value, json};

use super::{Display, Rendered, Segment, equation_line, hole_slot, rendered, stray_line, tag_content};
use crate::math_techxt::RENDERED_AS_TEXT;

/// The name of the HTML backend.
pub const BACKEND: &str = "html";

/// The stylesheet of the formulas, `math-techxt.style`.
pub const STYLE: &str = "math-techxt.style";

/// The standard stylesheet of the text inside formulas,
/// `flm.html.style.math-passthrough`.
const MATH_PASSTHROUGH_STYLE: &str = "flm.html.style.math-passthrough";

/// The class of every formula.
const FORMULA_CLASS: &str = "math-techxt";

/// The class of a displayed formula, added to [`FORMULA_CLASS`].
const DISPLAY_CLASS: &str = "math-techxt-display";

/// The class of an equation line.
const LINE_CLASS: &str = "math-techxt-line";

/// The class of an equation line's rendered lines.
const TEXT_CLASS: &str = "math-techxt-text";

/// The class of an equation line's tag.
const TAG_CLASS: &str = "math-techxt-tag";

/// The text of the stylesheet [`STYLE`], loading JuliaMono from `font_url`.
pub fn stylesheet_text(font_url: &str) -> String {
    let url = font_url.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        r#"/* {STYLE} */
/* The formulas the math-techxt extension writes as Unicode text: the font
   JuliaMono, the white space kept as written, a displayed formula as a block
   that scrolls sideways when it is wider than the text, and each equation
   line's tag at the right edge, beside the line's last printed line. */
@layer flm {{
  @font-face {{
    font-family: "JuliaMono";
    src: url("{url}") format("woff2");
  }}
  .{FORMULA_CLASS} {{
    font-family: "JuliaMono", ui-monospace, monospace;
    white-space: pre;
  }}
  .{DISPLAY_CLASS} {{
    display: block;
    margin: var(--flm-math-display-space-above) 0 var(--flm-math-display-space-below);
    overflow-x: auto;
  }}
  .{LINE_CLASS} {{
    display: flex;
    align-items: flex-end;
  }}
  .{LINE_CLASS}[id] {{
    scroll-margin-top: var(--flm-math-anchor-space-above);
  }}
  .{TAG_CLASS} {{
    margin-left: auto;
    padding-left: 2em;
  }}
}}
"#
    )
}

/// Registers the stylesheet [`STYLE`] for `font_url` and the rule of the
/// element [`RENDERED_AS_TEXT`] on `html`,
/// `math-techxt.rendered-as-text.lower.html`.
///
/// # Errors
///
/// The host's refusal.
pub fn register_element_rule(b: &mut FeatureBuilder, font_url: &str) -> Result<(), ExtError> {
    b.style_def(StyleDef {
        id: STYLE.into(),
        content: vec![DefContent::Inline(stylesheet_text(font_url))],
        docs: "The formulas written as Unicode text: the font JuliaMono, the white space kept, the displayed \
               formulas, the tags."
            .to_string(),
    })?;
    let rule = LoweringRule::new(
        "math-techxt.rendered-as-text.lower.html",
        RENDERED_AS_TEXT,
        LoweringDecl::new().produces(TAG),
        Arc::new(RenderedAsHtml),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )?;
    b.lowering_rule(rule.docs(
        "A formula rendered as Unicode text lowers to a span of its rendered text, each text command's content \
         in its place; an environment's equation lines are spans of their own, with their anchors and tags.",
    )?)
}

/// The body of the rule. See the module documentation.
struct RenderedAsHtml;

impl Lower for RenderedAsHtml {
    fn lower(&self, node: &Node, cx: &mut LowerCx, lw: &mut Lowerer) -> Result<Lowering, ExtError> {
        lw.require(STYLE)?;
        lw.require(MATH_PASSTHROUGH_STYLE)?;
        let (display, env, children) = match rendered(node)? {
            Rendered::Formula { display, lines } => (display, None, text_of(cx, lw, node, &lines)?),
            Rendered::Environment { env } => {
                let mut lines = StagedNodes::default();
                for line in node.slot_content_nodes_named(BODY_SLOT)?.to_vec()? {
                    lines.push(line_span(cx, lw, &line)?);
                }
                (Display::Display, Some(env), lines)
            }
            Rendered::Line { .. } => return Err(stray_line()),
        };
        let class = match display {
            Display::Inline => FORMULA_CLASS.to_string(),
            Display::Display => format!("{FORMULA_CLASS} {DISPLAY_CLASS}"),
        };
        let attributes = formula_attributes(&class, display == Display::Display, env)?
            .into_iter()
            .map(|(name, value)| json!([name, value]))
            .collect();
        Ok(Lowering::Emit(vec![span(cx, lw, attributes, children)?].into()))
    }
}

/// A `<span>` of the attributes `attributes` holding `children`.
fn span(cx: &mut LowerCx, lw: &mut Lowerer, attributes: Vec<Value>, children: StagedNodes) -> Result<Staged, ExtError> {
    lw.pres(cx, TAG, &json!({ "tag": "span", "attributes": attributes }), children)
}

/// The equation line `line`, an element [`RENDERED_AS_TEXT`] of an
/// environment's body: its span, carrying its anchor, holding the span of its
/// lines and the span of its tag.
fn line_span(cx: &mut LowerCx, lw: &mut Lowerer, line: &Node) -> Result<Staged, ExtError> {
    let (lines, anchor) = equation_line(line)?;
    let text = text_of(cx, lw, line, &lines)?;
    let mut children = StagedNodes::from(span(cx, lw, vec![json!(["class", TEXT_CLASS])], text)?);
    let tag = lw.lower_nodes(cx, &tag_content(line)?)?;
    if !tag.is_empty() {
        children.push(span(cx, lw, vec![json!(["class", TAG_CLASS])], tag)?);
    }
    let mut attributes = vec![json!(["class", LINE_CLASS])];
    if let Some(anchor) = anchor {
        attributes.push(json!(["id", anchor]));
    }
    span(cx, lw, attributes, children)
}

/// The rendered lines `lines` of the element `node`, as text separated by
/// newlines, each hole as its slot's content.
fn text_of(cx: &mut LowerCx, lw: &mut Lowerer, node: &Node, lines: &[Vec<Segment>]) -> Result<StagedNodes, ExtError> {
    let mut staged = StagedNodes::default();
    for (number, line) in lines.iter().enumerate() {
        if number > 0 {
            staged.push(lw.text(cx, "\n")?);
        }
        for segment in line {
            match segment {
                Segment::Text(text) => staged.push(lw.text(cx, text)?),
                Segment::Hole(index) => {
                    let content = node.slot_content_nodes_named(&hole_slot(*index))?;
                    staged.push_all(&lw.lower_nodes(cx, &content)?);
                }
            }
        }
    }
    Ok(staged)
}

