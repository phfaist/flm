//! The frame of a formula: what the [`math`](super) feature knows of a
//! formula without reading its content, computed by plain functions that a
//! processing feature's stages and rules call.
//!
//! A *processing feature* is a feature that reads mathematical content and
//! lowers formulas (the module documentation of [`math`](super), *Processing
//! mathematics*). Whatever it does with the mathematics itself, it writes the
//! same frame around it: whether a math group is displayed, and for each
//! equation line of a display-math environment its anchor and what it shows at
//! its tag's place (a number, the author's `\tag`, or nothing). This module
//! computes that frame; [`super::latex`] holds the parts that write it as LaTeX.
//!
//! The functions are *parts called in order*, never a callback: a rule calls
//! [`line_frames`], then, for each line, writes the line's content its own way
//! and calls [`stage_tag_content`] for what goes at the tag's place. The
//! functions that read tables take the tables as values, what
//! [`StageCx::get`](crate::core::run::StageCx::get) or
//! [`Lowerer::get_erased`] answered (through the typed `get` on `dyn Lowerer`),
//! so that a stage and a rule call them alike.
//!
//! - [`formula_display`] — whether a math group is displayed.
//! - [`line_frames`], [`LineFrame`], [`LineShows`] — the frame of each
//!   equation line of an environment.
//! - [`number_text`] — a line's number as plain characters.
//! - [`stage_tag_content`] — what to stage at a line's tag's place.
//!
//! The reader of equation lines, [`equation_lines`],
//! stays in [`lines`](super::lines).

use techy::core::node::{BuildId, NodeRef};
use techy::latexlike::MathGroupForm;

use crate::core::lowering::{LowerCx, LowerError, Lowerer};
use crate::lang::text::plain_text;
use crate::lang::{Flm, FlmGroupType};
use crate::model::addr::NodeAddr;
use crate::model::aspects::PointId;
use crate::model::element::element_value;
use crate::stdcore::backends::lowering::lower_run;
use crate::stdcore::features::numbering::{Counters, FormatError, FormatOptions, NumberSegment};
use crate::stdcore::features::refs::Anchors;

use super::MathEnvironment;
use super::lines::{EquationLine, equation_lines};

/// Whether the math group `node` is displayed: `true` for `\[…\]` (and
/// `$$…$$`), `false` for `\(…\)` (and `$…$`) and for a node that is not a math
/// group. Read from the node's group class
/// ([`FlmGroupType::Math`]).
///
/// A processing feature's rule of `flm.math` calls it to choose between its
/// inline and its displayed output.
pub fn formula_display(node: &NodeRef<'_, Flm>) -> bool {
    matches!(node.group_type(), Some(FlmGroupType::Math(MathGroupForm::Display)))
}

/// The frame of one equation line of a display-math environment, as
/// [`line_frames`] computes it: the line, its anchor, and what it shows at its
/// tag's place.
#[derive(Debug, Clone)]
pub struct LineFrame<'t> {
    /// The line as [`equation_lines`] read it.
    pub line: EquationLine<'t>,
    /// The line's anchor from the anchors table; `None` where no references
    /// feature is registered (the table is absent), or where the table holds
    /// no anchor for the line.
    pub anchor: Option<String>,
    /// What the line shows at its tag's place.
    pub shows: LineShows<'t>,
}

/// What an equation line shows at its tag's place ([`LineFrame::shows`]).
#[derive(Debug, Clone)]
pub enum LineShows<'t> {
    /// Nothing: a line of a starred environment, a line marked `\nonumber`, or
    /// any line where the feature's configuration numbers no equations
    /// ([`MathConfig::numbered_equations`](super::MathConfig::numbered_equations)),
    /// when the line carries no `\tag`.
    Nothing,
    /// The line's number, as the numbering feature formats it, with the
    /// counter style's delimiters (`(3)`) and without the words a reference
    /// writes before it; empty where no numbering feature is registered (the
    /// counter table is absent), or where the table holds no number for the
    /// line.
    Number(Vec<NumberSegment>),
    /// The author's `\tag`: its content nodes, and whether the star was
    /// written (`\tag*{…}`). A line shows the tag written on it even where it
    /// cannot be numbered (that case is reported when the body is parsed, as
    /// [`LabelWithoutNumber`](super::lines::LabelWithoutNumber)).
    Tag {
        /// Whether the star was written: `\tag*{A}` shows `A`, and `\tag{A}`
        /// shows `(A)`.
        starred: bool,
        /// The nodes of the tag's argument, document content.
        content: Vec<NodeRef<'t, Flm>>,
    },
}

/// The frame of each equation line of the display-math environment `node`, in
/// order: the line ([`equation_lines`]), its anchor and what it shows at its
/// tag's place.
///
/// `anchors` and `counters` are the anchors table of the references feature
/// ([`ANCHORS`](crate::stdcore::features::refs::ANCHORS)) and the counter table of the numbering feature
/// ([`COUNTERS`](crate::stdcore::features::numbering::COUNTERS)), as the caller read them, or `None` where
/// the table is absent; `addr` is the address of `node` in its tree
/// ([`Lowerer::addr`] in a rule, [`StageCx::addr`](crate::core::run::StageCx::addr)
/// in a stage), under which both tables key the lines' reference points. The
/// caller's feature declares the two tables as optional reads and as peer
/// tables of its own.
///
/// A processing feature's rule (or stage) of `flm.math.environment` calls it
/// first, then writes each line around its frame. A line shows its `\tag` when
/// one is written; otherwise its number when the line can be numbered (an
/// unstarred environment, no `\nonumber`, and a configuration that numbers
/// equations), empty when the counter table is absent; otherwise nothing. A
/// node that carries no [`MathEnvironment`] value is read as an environment
/// whose lines cannot be numbered.
///
/// # Errors
///
/// Returns [`FormatError`] when a number could not be written: a counter
/// record that does not fit the counter style it was made with, which the
/// numbering stage prevents.
pub fn line_frames<'t>(
    anchors: Option<&Anchors>,
    counters: Option<&Counters>,
    addr: NodeAddr,
    node: &NodeRef<'t, Flm>,
) -> Result<Vec<LineFrame<'t>>, FormatError> {
    let numbered = element_value::<MathEnvironment>(node)
        .is_some_and(|value| value.numbered && value.numbered_equations);
    // The word before a number belongs to a reference to the line, not to the
    // line itself: a formula shows `(3)`, and `Eq.~(3)` is what a `\ref` to it
    // shows.
    let options = FormatOptions::new().without_word();
    let mut frames = Vec::new();
    for (index, line) in equation_lines(node).into_iter().enumerate() {
        let point = PointId { addr, index: index as u32 };
        let anchor = anchors.and_then(|anchors| anchors.id(point)).map(String::from);
        let shows = match &line.tag {
            Some(tag) => LineShows::Tag { starred: tag.star, content: tag.content.clone() },
            None if numbered && !line.nonumber => {
                let number = match counters.and_then(|counters| counters.format(point, &options)) {
                    None => Vec::new(),
                    Some(written) => written?,
                };
                LineShows::Number(number)
            }
            None => LineShows::Nothing,
        };
        frames.push(LineFrame { line, anchor, shows });
    }
    Ok(frames)
}

/// The number a line shows, as plain characters: the segments of
/// [`LineShows::Number`] joined, the counter style's delimiters included
/// (`(3)`); an empty text where the number is empty. `None` where the line
/// shows nothing or a tag.
///
/// A processing feature calls it where it hands the number to a renderer as
/// text rather than staging its segments.
pub fn number_text(frame: &LineFrame<'_>) -> Option<String> {
    match &frame.shows {
        LineShows::Number(segments) => {
            Some(segments.iter().map(|segment| plain_text(segment.content())).collect())
        }
        LineShows::Nothing | LineShows::Tag { .. } => None,
    }
}

/// Stages what an equation line shows at its tag's place, and returns the
/// nodes to emit: the number's segments spliced (synthesized values, each
/// brought up to date and lowered); the author's tag lowered through the
/// rules, between `(` and `)` when it is not starred; nothing when the line
/// shows nothing.
///
/// A processing feature's rule of `flm.math.environment` calls it for each
/// line, after the line's content, and wraps the result in whatever its output
/// writes a tag as.
///
/// # Errors
///
/// Returns [`LowerError`] when a node could not be lowered or staged.
pub fn stage_tag_content(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    frame: &LineFrame<'_>,
) -> Result<Vec<BuildId>, LowerError> {
    let mut content = Vec::new();
    match &frame.shows {
        LineShows::Nothing => {}
        LineShows::Number(segments) => {
            for segment in segments {
                content.extend(lw.splice(cx, segment.content())?);
            }
        }
        LineShows::Tag { starred, content: nodes } => {
            if !starred {
                content.push(lw.text(cx, "(")?);
            }
            content.extend(lower_run(cx, lw, nodes)?);
            if !starred {
                content.push(lw.text(cx, ")")?);
            }
        }
    }
    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(shows: LineShows<'static>) -> LineFrame<'static> {
        LineFrame { line: EquationLine::default(), anchor: None, shows }
    }

    #[test]
    fn a_line_showing_nothing_or_a_tag_has_no_number_text() {
        assert_eq!(number_text(&frame(LineShows::Nothing)), None);
        let tag = LineShows::Tag { starred: true, content: Vec::new() };
        assert_eq!(number_text(&frame(tag)), None);
    }

    #[test]
    fn an_empty_number_is_an_empty_text() {
        assert_eq!(number_text(&frame(LineShows::Number(Vec::new()))), Some(String::new()));
    }
}
