//! Mirrors [`flm_core::stdcore::features::math`]: the names of the `math`
//! feature and the library a processing feature builds its stages and rules
//! with ([`frame`](flm_core::stdcore::features::math::frame),
//! [`latex`](flm_core::stdcore::features::math::latex),
//! [`equation_lines`](flm_core::stdcore::features::math::lines::equation_lines)).
//!
//! A *processing feature* reads mathematical content and lowers formulas. The
//! library computes what the `math` feature knows of a formula without reading
//! its content (whether a group is displayed, an environment's equation lines,
//! each line's anchor and what it shows at its tag's place), writes the LaTeX
//! that surrounds a formula's content on the recomposed LaTeX backend and an
//! environment's lines there, and holds the newline rule that every LaTeX
//! writer of an environment's lines applies ([`normalize_line_newlines`]).
//! Its functions are parts called in order, never a callback.
//!
//! What differs from `flm-core`, for every item:
//!
//! - nodes are the extension API's [`Node`]s, and what a function finds in the
//!   tree of a node it was given (a line's content, a tag's content) is
//!   answered as nodes of the same kind;
//! - a line's frame ([`LineFrame`]) names its environment and its index, so
//!   that the parts that take a frame read the line again from the tree; a
//!   number's segments are the roots of their synthesized values;
//! - [`line_frames`] reads the two tables itself, through the stage's context
//!   or the rule's lowerer it is given ([`FrameReader`]), since a script reads
//!   another feature's table as its projection, which cannot be read back;
//! - the parts that stage answer the extension API's [`Staged`] nodes, staged
//!   by the rule's call, and fail with [`ExtError`].
//!
//! Every function is callable from a stage and from a rule body alike where
//! it reads only, and from a rule body where it stages; the dispatcher offers
//! each under its name in the module `stdcore/features/math`.

use std::sync::Arc;

use flm_core::core::lowering::LowerError;
use flm_core::lang::Flm;
use flm_core::model::element::element_value;
use flm_core::stdcore::features::math::{self as core_math, frame as core_frame, latex as core_latex, lines as core_lines};
use flm_core::stdcore::features::numbering::{self, NumberSegment};
use flm_core::stdcore::features::refs;
use flm_core::techy::core::node::{NodeRef, NodeTree};

use crate::adapter::node::NodeFacade;
use crate::api::core::lowering::{LowerCx, Lowerer, Staged};
use crate::api::core::run::StageCx;
use crate::api::techy::node::{Node, Span};
use crate::runtime::ExtError;
use crate::sealed::Token;

// --- the names ---------------------------------------------------------------------------------

/// Mirrors [`MATH`](flm_core::lang::structural::MATH): the element of a
/// math group (`\(…\)`, `\[…\]`), `flm.math`, which the language layer
/// declares in every registry. What differs: it is listed here, beside the
/// names of the `math` feature, since a processing feature's rules are rules of
/// that element.
pub const MATH: &str = flm_core::lang::structural::MATH;

/// Mirrors [`ENVIRONMENT`](flm_core::stdcore::features::math::ENVIRONMENT):
/// the element of the display-math environments, `flm.math.environment`.
pub const ENVIRONMENT: &str = core_math::ENVIRONMENT;

/// Mirrors [`ENVIRONMENTS`](flm_core::stdcore::features::math::ENVIRONMENTS):
/// the display-math environments the feature declares, as `(name, numbered)`.
pub const ENVIRONMENTS: [(&str, bool); 6] = core_math::ENVIRONMENTS;

/// Mirrors [`CONTENT`](flm_core::stdcore::features::math::CONTENT): the
/// capability `flm.math.content`, which a processing feature's check stage
/// provides.
pub const CONTENT: &str = core_math::CONTENT;

/// Mirrors [`RENDERED`](flm_core::stdcore::features::math::RENDERED): the
/// capability `flm.math.rendered`, which a table of rendered formulas refines
/// ([`FeatureBuilder::table_refining`](crate::api::core::registry::FeatureBuilder::table_refining)).
pub const RENDERED: &str = core_math::RENDERED;

/// Mirrors [`KIND`](flm_core::stdcore::features::math::KIND): the kind of the
/// reference points of the equation lines, `equation`.
pub const KIND: &str = core_math::KIND;

/// Mirrors [`COUNTER`](flm_core::stdcore::features::math::COUNTER): the
/// counter of the equation lines, `flm.math.equation`.
pub const COUNTER: &str = core_math::COUNTER;

/// Mirrors [`ANCHORS`](flm_core::stdcore::features::refs::ANCHORS): the name
/// of the references feature's anchors table, `flm.anchors`, which
/// [`line_frames`] reads when asked; the constant of
/// [`refs`](crate::api::stdcore::features::refs), re-exported. What differs:
/// the name alone.
pub use crate::api::stdcore::features::refs::ANCHORS;

/// Mirrors [`COUNTERS`](flm_core::stdcore::features::numbering::COUNTERS):
/// the name of the numbering feature's counter table, `flm.counters`, which
/// [`line_frames`] reads when asked; the constant of
/// [`numbering`](crate::api::stdcore::features::numbering), re-exported.
/// What differs: the name alone.
pub use crate::api::stdcore::features::numbering::COUNTERS;

/// Mirrors [`PACKAGE`](flm_core::stdcore::features::math::lines::PACKAGE): the
/// scoped package of the commands of an environment's body, `flm.math.lines`.
pub const LINES_PACKAGE: &str = core_lines::PACKAGE;

/// Mirrors [`LINEBREAK_COMMAND`](flm_core::stdcore::features::math::lines::LINEBREAK_COMMAND):
/// the command that ends an equation line, `\\`, by its name.
pub const LINEBREAK_COMMAND: &str = core_lines::LINEBREAK_COMMAND;

/// Mirrors [`TAG_COMMAND`](flm_core::stdcore::features::math::lines::TAG_COMMAND):
/// the command that tags an equation line, `tag`.
pub const TAG_COMMAND: &str = core_lines::TAG_COMMAND;

/// Mirrors [`NONUMBER_COMMAND`](flm_core::stdcore::features::math::lines::NONUMBER_COMMAND):
/// the command that leaves an equation line unnumbered, `nonumber`.
pub const NONUMBER_COMMAND: &str = core_lines::NONUMBER_COMMAND;

/// Mirrors [`TEXT`](flm_core::stdcore::features::math::text::TEXT): the
/// element of the text commands of mathematics, `flm.math.text`.
pub const TEXT: &str = core_math::text::TEXT;

/// Mirrors [`COMMANDS`](flm_core::stdcore::features::math::text::COMMANDS):
/// the twelve text commands of mathematics (`text`, `textrm`, …), whose
/// argument is read in text mode.
pub const TEXT_COMMANDS: [&str; 12] = core_math::text::COMMANDS;

/// Mirrors [`ENSUREMATH`](flm_core::stdcore::features::math::ensuremath::ENSUREMATH):
/// the element of `\ensuremath`, `flm.math.ensuremath`. In text its argument's
/// content is an inline formula, a math group (`flm.math`); inside a formula,
/// a writer of the formula's TeX for a renderer writes the argument's content
/// in place of the command.
pub const ENSUREMATH: &str = core_math::ensuremath::ENSUREMATH;

// --- equation lines ----------------------------------------------------------------------------

// The labels written on an equation line are the references feature's
// `LabelUse`, as in `flm-core`.
use crate::api::stdcore::features::refs::LabelUse;

/// Mirrors [`Tag`](flm_core::stdcore::features::math::lines::Tag): the
/// `\tag` of an equation line. What differs: the `\tag` node itself is left
/// out; `star` is named `starred`, as in a frame's [`LineShows::Tag`].
pub struct Tag<'t> {
    /// Whether the star was written (`\tag*{…}`).
    pub starred: bool,
    /// The nodes of the tag's argument, document content.
    pub content: Vec<Box<dyn Node<'t> + 't>>,
}

/// Mirrors [`LineBreak`](flm_core::stdcore::features::math::lines::LineBreak):
/// the `\\` that ends an equation line. What differs: the `\\` node itself
/// is left out; its span is kept.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineBreak {
    /// The optional argument's content as written (`1ex` for `\\[1ex]`).
    pub spacing: Option<String>,
    /// No flm-core counterpart: the span of the `\\` node, on the source
    /// of the environment.
    pub span: Span,
}

/// Mirrors [`EquationLine`](flm_core::stdcore::features::math::lines::EquationLine):
/// one equation line of a display-math environment, with the fields of
/// `flm-core` by their names. What differs: as for [`Tag`], [`LineBreak`]
/// and [`LabelUse`].
pub struct EquationLine<'t> {
    /// The line's content, in order.
    pub content: Vec<Box<dyn Node<'t> + 't>>,
    /// The labels written on the line, in order.
    pub labels: Vec<LabelUse>,
    /// The tag, when one was written.
    pub tag: Option<Tag<'t>>,
    /// Whether `\nonumber` was written on the line.
    pub nonumber: bool,
    /// The `\\` that ends the line, when another line follows.
    pub linebreak: Option<LineBreak>,
}

/// The mirror of a node `core` of the tree of `site`.
fn relative<'t>(site: &dyn Node<'t>, core: &NodeRef<'_, Flm>) -> Box<dyn Node<'t> + 't> {
    site.relative(Token::new(), core.id())
}

/// The mirrors of `nodes`, nodes of the tree of `site`.
fn relatives<'t>(site: &dyn Node<'t>, nodes: &[NodeRef<'_, Flm>]) -> Vec<Box<dyn Node<'t> + 't>> {
    nodes.iter().map(|node| relative(site, node)).collect()
}

/// The mirror of `line`, a line of the environment `site`.
fn line_of<'t>(site: &dyn Node<'t>, line: &core_lines::EquationLine<'_>) -> EquationLine<'t> {
    EquationLine {
        content: relatives(site, &line.content),
        labels: line.labels.iter().map(LabelUse::of).collect(),
        tag: line.tag.as_ref().map(|tag| Tag { starred: tag.star, content: relatives(site, &tag.content) }),
        nonumber: line.nonumber,
        linebreak: line.linebreak.as_ref().map(|brk| {
            let range = brk.node.span().range();
            LineBreak { spacing: brk.spacing.clone(), span: Span { start: range.start, end: range.end } }
        }),
    }
}

/// Mirrors [`equation_lines`](flm_core::stdcore::features::math::lines::equation_lines):
/// the equation lines of the display-math environment `node`, in order (none
/// for another node). What differs: see [`EquationLine`].
pub fn equation_lines<'t>(node: &dyn Node<'t>) -> Vec<EquationLine<'t>> {
    let core = node.core_node_ref(Token::new());
    core_lines::equation_lines(&core).iter().map(|line| line_of(node, line)).collect()
}

/// Mirrors [`formula_display`](flm_core::stdcore::features::math::frame::formula_display):
/// whether the math group `node` is displayed (`\[…\]`); `false` for `\(…\)`
/// and for a node that is not a math group. Nothing differs.
pub fn formula_display(node: &dyn Node<'_>) -> bool {
    core_frame::formula_display(&node.core_node_ref(Token::new()))
}

/// No single `flm-core` counterpart: whether the lines of the display-math
/// environment `node` can be numbered, read from its element value
/// ([`MathEnvironment`](flm_core::stdcore::features::math::MathEnvironment)):
/// an unstarred environment in a registry that numbers equations
/// (`numbered` and `numbered_equations`), as
/// [`line_frames`](flm_core::stdcore::features::math::frame::line_frames)
/// reads it. `false` for a node that carries no such value.
pub fn environment_numbered(node: &dyn Node<'_>) -> bool {
    element_value::<core_math::MathEnvironment>(&node.core_node_ref(Token::new()))
        .is_some_and(|value| value.numbered && value.numbered_equations)
}

// --- the frame ---------------------------------------------------------------------------------

/// Mirrors [`LineFrame`](flm_core::stdcore::features::math::frame::LineFrame):
/// the frame of one equation line. What differs: it names its environment
/// and its index among the environment's lines, by which [`line_closing`]
/// reads the line again from the tree.
pub struct LineFrame<'t> {
    /// The display-math environment the line belongs to.
    pub environment: Box<dyn Node<'t> + 't>,
    /// The line's index among the environment's lines, from 0.
    pub index: usize,
    /// The line, as [`equation_lines`] reads it.
    pub line: EquationLine<'t>,
    /// The line's anchor; `None` where the anchors table was not read or is
    /// absent, or holds no anchor for the line.
    pub anchor: Option<String>,
    /// What the line shows at its tag's place.
    pub shows: LineShows<'t>,
}

/// Mirrors [`LineShows`](flm_core::stdcore::features::math::frame::LineShows):
/// what an equation line shows at its tag's place. What differs: a number
/// carries its plain text ([`number_text`]) beside its segments, and each
/// segment is the root node of its synthesized value (the segment's link to a
/// reference point is left out; nothing of the mirror reads it).
pub enum LineShows<'t> {
    /// Nothing.
    Nothing,
    /// The line's number.
    Number {
        /// The segments, each the root of a synthesized value; empty where the
        /// counter table was not read or is absent.
        segments: Vec<Box<dyn Node<'t> + 't>>,
        /// The segments' plain text joined, the counter style's delimiters
        /// included (`(3)`); empty for an empty number.
        text: String,
    },
    /// The author's `\tag`.
    Tag {
        /// Whether the star was written.
        starred: bool,
        /// The nodes of the tag's argument.
        content: Vec<Box<dyn Node<'t> + 't>>,
    },
}

/// The context [`line_frames`] reads the tables and the environment's address
/// through: a running stage's context, or a rule's lowerer.
pub enum FrameReader<'a> {
    /// A stage's context ([`StageCx`]).
    Stage(&'a dyn StageCx),
    /// A rule's lowerer ([`Lowerer`]).
    Rule(&'a dyn Lowerer),
}

/// An error of the library as a script sees it.
fn library_error(kind: &str, detail: impl ToString) -> ExtError {
    ExtError::Core { kind: kind.to_string(), detail: detail.to_string(), data: serde_json::Value::Null }
}

/// The mirror of the frames `frames` of the environment `site`.
fn frames_of<'t>(site: &dyn Node<'t>, frames: &[core_frame::LineFrame<'_>]) -> Vec<LineFrame<'t>> {
    frames
        .iter()
        .enumerate()
        .map(|(index, frame)| LineFrame {
            environment: site.clone_box(Token::new()),
            index,
            line: line_of(site, &frame.line),
            anchor: frame.anchor.clone(),
            shows: match &frame.shows {
                core_frame::LineShows::Nothing => LineShows::Nothing,
                core_frame::LineShows::Number(segments) => LineShows::Number {
                    segments: segments
                        .iter()
                        .map(|segment| {
                            let tree = Arc::clone(segment.content());
                            let root = tree.root().id();
                            Box::new(NodeFacade::shared(tree, root)) as Box<dyn Node<'t> + 't>
                        })
                        .collect(),
                    text: core_frame::number_text(frame).unwrap_or_default(),
                },
                core_frame::LineShows::Tag { starred, content } => {
                    LineShows::Tag { starred: *starred, content: relatives(site, content) }
                }
            },
        })
        .collect()
}

/// Mirrors [`line_frames`](flm_core::stdcore::features::math::frame::line_frames):
/// the frame of each equation line of the display-math environment `node`,
/// in order.
///
/// What differs: `flm-core` takes the two tables as values and the node's
/// address; the mirror takes the context it reads them through (`reader`)
/// and, for each table, whether to read it: `anchors` reads the anchors table
/// ([`ANCHORS`]) and `counters` the counter table ([`COUNTERS`]), each as an
/// optional read the caller declared (an absent table is read as `flm-core`
/// reads `None`); `false` leaves the table out, as a caller does that numbers
/// the lines itself (the recomposed LaTeX). The node's address is the one the
/// reader answers for it. A stage and a rule call it alike.
///
/// # Errors
///
/// A table asked for whose read the caller did not declare ([`ExtError::Core`]
/// of kind `access`); `node` not in a tree of the run ([`ExtError::Argument`]);
/// a number that could not be written ([`ExtError::Core`] of kind `format`).
pub fn line_frames<'t>(
    reader: FrameReader<'_>,
    anchors: bool,
    counters: bool,
    node: &dyn Node<'t>,
) -> Result<Vec<LineFrame<'t>>, ExtError> {
    const METHOD: &str = "line_frames";
    let core = node.core_node_ref(Token::new());
    let mut answer: Option<Result<Vec<LineFrame<'t>>, ExtError>> = None;
    let outside = || ExtError::Argument { method: METHOD.to_string(), detail: "`node` must be a node of a tree of the run".to_string() };
    match reader {
        FrameReader::Stage(cx) => cx.with_core(Token::new(), &mut |cx| {
            answer = Some((|| {
                let addr = cx.addr(&core).ok_or_else(outside)?;
                let anchors = if anchors { cx.get(refs::ANCHORS).map_err(|e| library_error("access", e))? } else { None };
                let counters =
                    if counters { cx.get(numbering::COUNTERS).map_err(|e| library_error("access", e))? } else { None };
                let frames = core_frame::line_frames(anchors, counters, addr, &core).map_err(|e| library_error("format", e))?;
                Ok(frames_of(node, &frames))
            })());
        }),
        FrameReader::Rule(lw) => lw.with_core(Token::new(), &mut |lw| {
            answer = Some((|| {
                let addr = lw.addr_of(&core).map_err(|_| outside())?;
                let anchors = if anchors { lw.get(refs::ANCHORS).map_err(|e| library_error("access", e))? } else { None };
                let counters =
                    if counters { lw.get(numbering::COUNTERS).map_err(|e| library_error("access", e))? } else { None };
                let frames = core_frame::line_frames(anchors, counters, addr, &core).map_err(|e| library_error("format", e))?;
                Ok(frames_of(node, &frames))
            })());
        }),
    }
    // `with_core` calls its closure exactly once (the seal's contract), so the
    // answer is always set.
    answer.expect("`with_core` calls its closure")
}

/// Mirrors [`number_text`](flm_core::stdcore::features::math::frame::number_text):
/// the number a line shows, as plain characters; `None` where the line shows
/// nothing or a tag. What differs: read from the frame's
/// [`LineShows::Number`], which carries it.
pub fn number_text(frame: &LineFrame<'_>) -> Option<String> {
    match &frame.shows {
        LineShows::Number { text, .. } => Some(text.clone()),
        LineShows::Nothing | LineShows::Tag { .. } => None,
    }
}

/// `flm-core`'s frame of what `frame` shows, its line left empty: what
/// [`stage_tag_content`] reads.
fn core_shows<'a>(frame: &'a LineFrame<'_>) -> Result<core_frame::LineShows<'a>, ExtError> {
    Ok(match &frame.shows {
        LineShows::Nothing => core_frame::LineShows::Nothing,
        LineShows::Number { segments, .. } => {
            let mut core = Vec::with_capacity(segments.len());
            for segment in segments {
                let root = segment.core_node_ref(Token::new());
                if root.parent().is_some() {
                    return Err(ExtError::Argument {
                        method: "stage_tag_content".to_string(),
                        detail: "each segment of a number must be the root node of its value".to_string(),
                    });
                }
                let tree: Arc<NodeTree<Flm>> =
                    segment.shared_tree(Token::new()).unwrap_or_else(|| Arc::new(root.tree().clone()));
                core.push(NumberSegment::new(tree, None));
            }
            core_frame::LineShows::Number(core)
        }
        LineShows::Tag { starred, content } => core_frame::LineShows::Tag {
            starred: *starred,
            content: content.iter().map(|node| node.core_node_ref(Token::new())).collect(),
        },
    })
}

/// Stages through `flm-core`'s part `f`, as the call of `lw`.
fn stage(
    method: &str,
    cx: &mut dyn LowerCx,
    lw: &mut dyn Lowerer,
    mut f: impl FnMut(&mut flm_core::core::lowering::LowerCx<'_>, &mut dyn flm_core::core::lowering::Lowerer) -> Result<Vec<flm_core::techy::core::node::BuildId>, LowerError>,
) -> Result<Vec<Staged>, ExtError> {
    lw.stage_core(Token::new(), method, cx, &mut f)
}

/// Mirrors [`stage_tag_content`](flm_core::stdcore::features::math::frame::stage_tag_content):
/// stages what an equation line shows at its tag's place (the number's
/// segments spliced; the tag lowered, between `(` and `)` when unstarred;
/// nothing) and answers the nodes to emit. What differs: the frame's
/// [`shows`](LineFrame::shows) alone is read; the nodes are the call's own
/// staged nodes.
///
/// # Errors
///
/// A segment that is not the root of its value, `cx` not the call's context
/// ([`ExtError::Argument`]), or `flm-core`'s refusal ([`ExtError::Core`]).
pub fn stage_tag_content(cx: &mut dyn LowerCx, lw: &mut dyn Lowerer, frame: &LineFrame<'_>) -> Result<Vec<Staged>, ExtError> {
    let core = core_frame::LineFrame { line: core_lines::EquationLine::default(), anchor: None, shows: core_shows(frame)? };
    stage("stage_tag_content", cx, lw, |cx, lw| core_frame::stage_tag_content(cx, lw, &core))
}

// --- the LaTeX frame of a formula --------------------------------------------------------------

/// Mirrors [`environment_opening`](flm_core::stdcore::features::math::latex::environment_opening):
/// the opening part of a display-math environment in the recomposed LaTeX.
/// What differs: the nodes are the call's own staged nodes.
///
/// # Errors
///
/// `cx` not the call's context ([`ExtError::Argument`]), or `flm-core`'s
/// refusal ([`ExtError::Core`]): the rule does not declare the reads and
/// constructs the part needs (see the `flm-core` item).
pub fn environment_opening(cx: &mut dyn LowerCx, lw: &mut dyn Lowerer, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError> {
    let node = node.core_node_ref(Token::new());
    stage("environment_opening", cx, lw, |cx, lw| core_latex::environment_opening(cx, lw, &node))
}

/// `flm-core`'s frame of the line `frame` names, read again from the frame's
/// environment at its index, without an anchor and showing nothing: what the
/// parts that write a line in the recomposed LaTeX read (`method` names the
/// calling function in the error).
fn core_line_frame<'a>(frame: &'a LineFrame<'_>, method: &str) -> Result<core_frame::LineFrame<'a>, ExtError> {
    let environment = frame.environment.core_node_ref(Token::new());
    let line = core_lines::equation_lines(&environment).into_iter().nth(frame.index).ok_or_else(|| ExtError::Argument {
        method: method.to_string(),
        detail: format!("the frame's `index` must be the index of a line of its environment, not {}", frame.index),
    })?;
    Ok(core_frame::LineFrame { line, anchor: None, shows: core_frame::LineShows::Nothing })
}

/// Mirrors [`line_closing`](flm_core::stdcore::features::math::latex::line_closing):
/// the closing part of an equation line in the recomposed LaTeX. What
/// differs: the line is read again from the frame's environment, at its
/// index; the nodes are the call's own staged nodes.
///
/// # Errors
///
/// The frame's index is not that of a line of its environment, `cx` not the
/// call's context ([`ExtError::Argument`]), or `flm-core`'s refusal
/// ([`ExtError::Core`]).
pub fn line_closing(cx: &mut dyn LowerCx, lw: &mut dyn Lowerer, frame: &LineFrame<'_>) -> Result<Vec<Staged>, ExtError> {
    let core = core_line_frame(frame, "line_closing")?;
    stage("line_closing", cx, lw, |cx, lw| core_latex::line_closing(cx, lw, &core))
}

/// Mirrors [`stage_lines`](flm_core::stdcore::features::math::latex::stage_lines):
/// stages the equation lines of a display-math environment in the recomposed
/// LaTeX as they were written: for each line, its content written by the
/// mathematics writer of the `userdefs` feature under the newline rule, its
/// closing part ([`line_closing`]), and the run of whitespace the rule took
/// off the content's end. What differs: each line is read again from its
/// frame's environment, at its index; the nodes are the call's own staged
/// nodes. The calling rule declares what `flm-core`'s item names, including
/// that it produces the raw, macro and definition constructs
/// (`flm.pres.latex.raw`, `flm.pres.latex.macro`, `flm.pres.latex.definition`).
///
/// # Errors
///
/// A frame's index is not that of a line of its environment, `cx` not the
/// call's context ([`ExtError::Argument`]), or `flm-core`'s refusal
/// ([`ExtError::Core`]).
pub fn stage_lines(cx: &mut dyn LowerCx, lw: &mut dyn Lowerer, frames: &[LineFrame<'_>]) -> Result<Vec<Staged>, ExtError> {
    let core = frames.iter().map(|frame| core_line_frame(frame, "stage_lines")).collect::<Result<Vec<_>, _>>()?;
    stage("stage_lines", cx, lw, |cx, lw| core_latex::stage_lines(cx, lw, &core))
}

/// Mirrors `flm-core`'s [`NormalizedLine`]: one text part of a line's
/// content after the newline rule, what [`normalize_line_newlines`] answers.
/// Nothing differs: it is `flm-core`'s own, re-exported; across a binding
/// `{ text, trailing }`.
pub use flm_core::stdcore::features::math::latex::NormalizedLine;

/// Mirrors [`normalize_line_newlines`](flm_core::stdcore::features::math::latex::normalize_line_newlines):
/// applies the newline rule to one text part of a line's content: every run
/// of spaces, tabs and newlines that holds a newline becomes one newline and
/// the indentation after it; the run that begins `content` is dropped when
/// `after_line_end` (what was written just before the part ends a line); the
/// run that ends it is answered apart as the trailing run, to be written
/// after the line's closing part, unless `inner` (the part is not the last of
/// the line's content). A writer of an environment's lines that writes their
/// content as text applies it to each text part. Nothing differs.
pub fn normalize_line_newlines(content: &str, after_line_end: bool, inner: bool) -> NormalizedLine {
    core_latex::normalize_line_newlines(content, after_line_end, inner)
}

/// Mirrors [`environment_closing`](flm_core::stdcore::features::math::latex::environment_closing):
/// the closing part of a display-math environment in the recomposed LaTeX.
/// What differs: as for [`environment_opening`].
///
/// # Errors
///
/// As [`environment_opening`].
pub fn environment_closing(cx: &mut dyn LowerCx, lw: &mut dyn Lowerer, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError> {
    let node = node.core_node_ref(Token::new());
    stage("environment_closing", cx, lw, |cx, lw| core_latex::environment_closing(cx, lw, &node))
}

/// Mirrors [`group_opening`](flm_core::stdcore::features::math::latex::group_opening):
/// the opening part of a math group in the recomposed LaTeX. What differs: as
/// for [`environment_opening`].
///
/// # Errors
///
/// As [`environment_opening`].
pub fn group_opening(cx: &mut dyn LowerCx, lw: &mut dyn Lowerer, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError> {
    let node = node.core_node_ref(Token::new());
    stage("group_opening", cx, lw, |cx, lw| core_latex::group_opening(cx, lw, &node))
}

/// Mirrors [`group_closing`](flm_core::stdcore::features::math::latex::group_closing):
/// the closing part of a math group in the recomposed LaTeX. What differs: as
/// for [`environment_opening`].
///
/// # Errors
///
/// As [`environment_opening`].
pub fn group_closing(cx: &mut dyn LowerCx, lw: &mut dyn Lowerer, node: &dyn Node<'_>) -> Result<Vec<Staged>, ExtError> {
    let node = node.core_node_ref(Token::new());
    stage("group_closing", cx, lw, |cx, lw| core_latex::group_closing(cx, lw, &node))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_table_names_are_the_libraries() {
        assert_eq!(MATH, "flm.math");
        assert_eq!(ANCHORS, "flm.anchors");
        assert_eq!(COUNTERS, "flm.counters");
        assert_eq!(TEXT_COMMANDS.len(), 12);
    }
}
