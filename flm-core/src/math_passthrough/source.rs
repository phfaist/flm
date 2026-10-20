//! The source walk of the [`math-passthrough`](super) feature and the bodies
//! of its rules that stage `flm.pres.math-passthrough`: the rule of the math
//! groups (`\(…\)`, `\[…\]`), the rule of the display-math environments, and
//! the rule of an opaque command lowered on its own.
//!
//! Each rule stages (builds into the lowered tree; see [`Lowerer`]) one
//! [`flm.pres.math-passthrough`](crate::stdcore::backends::pres::MATH_PASSTHROUGH)
//! node whose body is the source the author wrote, in order and unchanged,
//! except that every text command becomes content: a `flm.pres.math-text` node
//! holding the command's argument lowered as inline content, and that
//! `\ensuremath{…}` inside a formula is written as its argument's content, the
//! command left out ([`ensuremath`](crate::stdcore::features::math::ensuremath)).
//! A backend that writes `flm.pres.math-passthrough` natively (the three
//! standard backends do) needs no rule of its own; on the `latex-recomposed`
//! backend the feature's rules of [`super::latex`] are chosen instead.
//!
//! The rule of the display-math environments reads the line anchors and the
//! line numbers through the library of the
//! [`math`](crate::stdcore::features::math) feature
//! ([`line_frames`], [`stage_tag_content`]), which owns the equation lines,
//! their reference points and their numbering, and writes each line's content
//! under that library's newline rule (the module documentation of
//! [`latex`](crate::stdcore::features::math::latex), *The newline rule*), so
//! that a `\label`, `\tag` or `\nonumber` taken off a line of its own leaves
//! no blank line in the formula's source: the run of whitespace that ends a
//! line's content is written after the line's tag and its `\\`.

use techy::core::node::{BuildId, NodeRef};
use techy::latexlike::{InvocationSyntaxData, MathGroupForm};

use crate::core::lowering::{Lower, LowerCx, LowerError, Lowerer, Lowering};
use crate::lang::Flm;
use crate::lang::mint::is_math_text_command;
use crate::model::element::{element_of, element_value};
use crate::model::names::ElementName;
use crate::stdcore::backends::lowering::{copy_subtree_verbatim, lower_run};
use crate::stdcore::backends::pres::{
    self, MathDisplay, MathPassthroughPayload, MathTextPayload, recompose_source,
};
use crate::stdcore::features::math::MathEnvironment;
use crate::stdcore::features::math::ensuremath::ENSUREMATH;
use crate::stdcore::features::math::frame::{formula_display, line_frames, stage_tag_content};
use crate::stdcore::features::math::latex::NewlineRule;
use crate::stdcore::features::numbering::COUNTERS;
use crate::stdcore::features::refs::ANCHORS;

/// The display form of a node, taken from the nearest math group that
/// contains it: display inside a display group, inline inside any other math
/// group and when no math group contains the node.
fn enclosing_display(node: &NodeRef<'_, Flm>) -> MathDisplay {
    let mut ancestor = node.parent();
    while let Some(up) = ancestor {
        if let Some(form) = up.math_form() {
            return match form {
                MathGroupForm::Display => MathDisplay::Display,
                _ => MathDisplay::Inline,
            };
        }
        ancestor = up.parent();
    }
    MathDisplay::Inline
}

/// The body of [`OPAQUE_RULE`](super::OPAQUE_RULE): it stages a
/// [`flm.pres.math-passthrough`](crate::stdcore::backends::pres::MATH_PASSTHROUGH) node
/// over a copy of the opaque node, displayed when the nearest math group that
/// contains the node is displayed and inline otherwise.
///
/// The rule runs only when an opaque node is lowered on its own; see the
/// [feature's module documentation](super), *The rules*. It
/// returns a [`LowerError`] when the copy or the node could not be staged.
#[derive(Debug, Default)]
pub struct OpaqueRule;

impl Lower for OpaqueRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let display = enclosing_display(&node);
        let body = vec![copy_subtree_verbatim(cx, node)?];
        let payload = MathPassthroughPayload { display, env: None };
        let math = lw.pres_with(cx, &ElementName::from(pres::MATH_PASSTHROUGH), payload, body)?;
        Ok(Lowering::Emit(vec![math]))
    }
}

/// Stages a run of a formula's nodes for the body of a
/// `flm.pres.math-passthrough` node: every text command, at any depth, is lowered
/// through the rules and becomes a `flm.pres.math-text` node, and everything
/// else is written back as its source.
///
/// The walk goes through the whole formula, since a text command may be
/// written at any depth: a group is written as its delimiters around the walk
/// of its children, an environment as `\begin{…}` and `\end{…}` around the
/// walk of its arguments and body, `\ensuremath{…}` as the walk of its
/// argument's content alone, any other command with arguments as the call as
/// written followed by the walk of each argument, and any other node as a
/// copy. [`MathRule`] uses it; [`EnvironmentRule`] walks each node of a line
/// that holds a text command or `\ensuremath{…}` the same way.
///
/// # Errors
///
/// Returns [`LowerError`] when a copy or a text node could not be staged, or
/// when a text command could not be lowered.
pub(crate) fn stage_source(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    nodes: &[NodeRef<'_, Flm>],
) -> Result<Vec<BuildId>, LowerError> {
    let mut body = Vec::with_capacity(nodes.len());
    for node in nodes {
        stage_node(cx, lw, *node, &mut body)?;
    }
    Ok(body)
}

/// Stages one node of a formula and appends what it staged to `body` (see
/// [`stage_source`] for the cases).
// This walk carries no depth limit of its own, and needs none, because the
// parse that made the tree stops first. A parse holds far more on the stack per
// level of nesting than this walk does, and the parser's stack guard refuses a
// formula long before the depth this walk would need to run out of the worker
// thread's stack: measured, a formula of nested braces is refused at about two
// thousand levels, and a formula at that depth walks through here without
// trouble. Keep the margin in mind if the parser's stack budget is raised or if
// this function grows much larger frames.
fn stage_node(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: NodeRef<'_, Flm>,
    body: &mut Vec<BuildId>,
) -> Result<(), LowerError> {
    if is_math_text_command(&node) {
        body.extend(lower_run(cx, lw, std::slice::from_ref(&node))?);
        return Ok(());
    }
    if let Some((open, close)) = node.group_delimiters() {
        body.push(lw.text(cx, open)?);
        body.extend(stage_source(cx, lw, &children_of(&node))?);
        body.push(lw.text(cx, close)?);
        return Ok(());
    }
    if node.is_list() {
        body.extend(stage_source(cx, lw, &children_of(&node))?);
        return Ok(());
    }
    if let Some(inner) = node.body() {
        // An environment inside the formula. The name it was written with is
        // written back around the walk of its arguments and its body.
        let name = node.name().unwrap_or_default();
        body.push(lw.text(cx, &format!("\\begin{{{name}}}"))?);
        stage_arguments(cx, lw, &node, body)?;
        let inner: Vec<NodeRef<'_, Flm>> = inner.iter().collect();
        body.extend(stage_source(cx, lw, &inner)?);
        body.push(lw.text(cx, &format!("\\end{{{name}}}"))?);
        return Ok(());
    }
    if element_of(&node).name == ENSUREMATH {
        // Inside a formula, `\ensuremath{X}` stands for `X`: the argument's
        // content is written in place of the command.
        let content: Vec<NodeRef<'_, Flm>> =
            node.argument_content_nodes(0).map(|slice| slice.iter().collect()).unwrap_or_default();
        body.extend(stage_source(cx, lw, &content)?);
        return Ok(());
    }
    if node.callable().is_some_and(|callable| !callable.arguments.is_empty()) {
        body.push(lw.text(cx, &macro_call(&node))?);
        return stage_arguments(cx, lw, &node, body);
    }
    body.push(copy_subtree_verbatim(cx, node)?);
    Ok(())
}

/// The children of a node, collected into a list so that the walk can borrow
/// the [`Lowerer`] mutably while it goes through them.
fn children_of<'t>(node: &NodeRef<'t, Flm>) -> Vec<NodeRef<'t, Flm>> {
    node.children().iter().collect()
}

/// The beginning of a macro call as written: the escape character, the name,
/// and the whitespace after the name, which belongs to the call and not to
/// the argument that follows it.
fn macro_call(node: &NodeRef<'_, Flm>) -> String {
    let name = node.name().unwrap_or_default();
    match node.invocation_syntax() {
        Some(InvocationSyntaxData::Macro { escape_char, post_space }) => {
            format!("{escape_char}{name}{}", post_space.resolve(node.span().source()))
        }
        _ => format!("\\{name}"),
    }
}

/// Walks the arguments of `node` in the order they were written and appends
/// what it stages to `body`. The delimiters of an argument are nodes of the
/// argument, so the walk writes them back with it.
fn stage_arguments(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    body: &mut Vec<BuildId>,
) -> Result<(), LowerError> {
    let count = node.callable().map_or(0, |callable| callable.arguments.len());
    for index in 0..count {
        if let Some(region) = node.argument_nodes(index) {
            let nodes: Vec<NodeRef<'_, Flm>> = region.iter().collect();
            body.extend(stage_source(cx, lw, &nodes)?);
        }
    }
    Ok(())
}

/// Stages one equation line's tag: the text `\tag*{`, a
/// [`flm.pres.math-text`](crate::stdcore::backends::pres::MATH_TEXT) node
/// holding `content`, and `}`; nothing when `content` is empty.
///
/// The same function writes an author's tag and a line's number: `content` is
/// the lowered content of the author's `\tag`, with the parentheses of the
/// unstarred form already added, or the number the document gave the line.
/// The form `\tag*` writes its content as given, so nothing is added to it.
/// The content node is marked
/// [`at_tag`](crate::stdcore::backends::pres::MathTextPayload::at_tag), so that
/// each backend can write it as a tag's content requires.
///
/// # Errors
///
/// Returns [`LowerError`] when a node could not be staged, or when the calling
/// rule does not declare that it produces `flm.pres.math-text`.
fn stage_tag(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    content: Vec<BuildId>,
) -> Result<Vec<BuildId>, LowerError> {
    if content.is_empty() {
        return Ok(Vec::new());
    }
    let payload = MathTextPayload { command: None, anchor: None, at_tag: true };
    let text = lw.pres_with(cx, &ElementName::from(pres::MATH_TEXT), payload, content)?;
    Ok(vec![lw.text(cx, "\\tag*{")?, text, lw.text(cx, "}")?])
}

/// Whether the walk ([`stage_node`]) must write `node` itself rather than
/// copy its source: a text command or `\ensuremath{…}`, or a node that holds
/// one.
fn needs_walk(node: &NodeRef<'_, Flm>) -> bool {
    let walked =
        |node: &NodeRef<'_, Flm>| is_math_text_command(node) || element_of(node).name == ENSUREMATH;
    walked(node) || node.descendants().any(|descendant| walked(&descendant))
}

/// One part of an equation line's content, as [`stage_line_content`] writes
/// it.
enum LinePart<'t> {
    /// The source of a run of nodes that need no walk, written as text under
    /// the newline rule.
    Text(String),
    /// A source comment, copied.
    Comment(NodeRef<'t, Flm>),
    /// A node written by the walk.
    Walked(NodeRef<'t, Flm>),
}

/// Stages the content of one equation line and appends what it staged to
/// `body`, under the `math` library's newline rule (`rule`, carried across
/// the environment's lines): each run of nodes that need no walk as the text
/// of its source, each source comment as a copy, and every other node by the
/// walk ([`stage_node`]). Returns the run of whitespace the rule took off the
/// content's end, to be written after the line's tag and its `\\`.
fn stage_line_content(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    content: &[NodeRef<'_, Flm>],
    rule: &mut NewlineRule,
    body: &mut Vec<BuildId>,
) -> Result<String, LowerError> {
    let mut parts = Vec::new();
    let mut run: Vec<NodeRef<'_, Flm>> = Vec::new();
    for node in content {
        if node.is_comment() || needs_walk(node) {
            if !run.is_empty() {
                parts.push(LinePart::Text(recompose_source(&run)));
                run.clear();
            }
            parts.push(if node.is_comment() { LinePart::Comment(*node) } else { LinePart::Walked(*node) });
        } else {
            run.push(*node);
        }
    }
    if !run.is_empty() {
        parts.push(LinePart::Text(recompose_source(&run)));
    }
    let count = parts.len();
    let mut trailing = String::new();
    for (index, part) in parts.into_iter().enumerate() {
        match part {
            LinePart::Text(text) => {
                let line = rule.text(&text, index + 1 == count);
                if !line.text.is_empty() {
                    body.push(lw.text(cx, &line.text)?);
                }
                trailing = line.trailing;
            }
            LinePart::Comment(node) => {
                rule.written(&recompose_source(&[node]));
                body.push(copy_subtree_verbatim(cx, node)?);
            }
            LinePart::Walked(node) => {
                stage_node(cx, lw, node, body)?;
                rule.piece(false);
            }
        }
    }
    Ok(trailing)
}

/// The body of [`ENVIRONMENT_RULE`](super::ENVIRONMENT_RULE): it stages a displayed
/// [`flm.pres.math-passthrough`](crate::stdcore::backends::pres::MATH_PASSTHROUGH)
/// node that carries the environment's name, over the environment's equation
/// lines: for each line, an empty `flm.pres.math-text` node carrying the
/// line's anchor, the line's source, the line's tag or number, its `\\` when
/// another line follows, and the run of whitespace that ended the line's
/// source. A line's `\label` and `\nonumber` are not written: they are data
/// of the line, which the `math` feature has already read.
///
/// The line's source is written under the `math` library's newline rule (see
/// the module documentation): the source of the nodes that hold no text
/// command is written as text, so that the rule applies to it.
///
/// The rule reads the anchors table and the counter table when they are
/// present; without them the lines carry no anchor and no number. It returns
/// a [`LowerError`] when the node carries no [`MathEnvironment`] (a node that a
/// parse produced for one of the six environments always carries one), when a
/// table cannot be read, or when a node could not be staged.
#[derive(Debug, Default, Clone, Copy)]
pub struct EnvironmentRule;

impl Lower for EnvironmentRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<MathEnvironment>(&node)
            .ok_or_else(|| LowerError::structure("a math environment without its value"))?;
        // The frames are computed before anything is staged, because reading
        // a table borrows the façade the staging needs.
        let frames = line_frames(lw.get(ANCHORS)?, lw.get(COUNTERS)?, lw.addr(), &node)
            .map_err(|error| LowerError::structure(error.to_string()))?;
        let mut body = Vec::new();
        let mut rule = NewlineRule::default();
        for frame in &frames {
            if let Some(anchor) = frame.anchor.clone() {
                let payload =
                    MathTextPayload { command: None, anchor: Some(anchor), at_tag: false };
                body.push(lw.pres_with(
                    cx,
                    &ElementName::from(pres::MATH_TEXT),
                    payload,
                    Vec::new(),
                )?);
            }
            let trailing = stage_line_content(cx, lw, &frame.line.content, &mut rule, &mut body)?;
            let content = stage_tag_content(cx, lw, frame)?;
            let tag = stage_tag(cx, lw, content)?;
            let closed = !tag.is_empty() || frame.line.linebreak.is_some();
            body.extend(tag);
            if let Some(brk) = &frame.line.linebreak {
                body.push(copy_subtree_verbatim(cx, brk.node)?);
            }
            rule.line_end(closed, &trailing);
            if !trailing.is_empty() {
                body.push(lw.text(cx, &trailing)?);
            }
        }
        let payload = MathPassthroughPayload {
            display: MathDisplay::Display,
            env: Some(value.name.to_string()),
        };
        let math = lw.pres_with(cx, &ElementName::from(pres::MATH_PASSTHROUGH), payload, body)?;
        Ok(Lowering::Emit(vec![math]))
    }
}

/// The body of [`MATH_RULE`](super::MATH_RULE): it stages a `flm.pres.math-passthrough` node
/// over the math group's nodes, displayed for `\[…\]` and inline for `\(…\)`.
/// Every text command inside the group, at any depth, becomes a
/// `flm.pres.math-text` node (see the module documentation).
///
/// The rule returns a [`LowerError`] when a node could not be staged or a text
/// command could not be lowered.
#[derive(Debug, Default, Clone, Copy)]
pub struct MathRule;

impl Lower for MathRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let display =
            if formula_display(&node) { MathDisplay::Display } else { MathDisplay::Inline };
        let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
        let body = stage_source(cx, lw, &children)?;
        let payload = MathPassthroughPayload { display, env: None };
        let math = lw.pres_with(cx, &ElementName::from(pres::MATH_PASSTHROUGH), payload, body)?;
        Ok(Lowering::Emit(vec![math]))
    }
}
