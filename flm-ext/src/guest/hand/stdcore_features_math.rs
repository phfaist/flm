//! The guest's own types of `flm:stdcore/features/math`: the lines of an
//! equation and their frames, which hold nodes ([`EquationLine`], [`Tag`],
//! [`LineBreak`], [`LineFrame`], [`LineShows`]), and [`FrameReader`].

use serde_json::{json, Value};

use super::{get, not_shape, object};
use crate::adapter::handles::HandleKind;
use crate::adapter::values::{handle_from_value, JsonForm};
use crate::api::stdcore::features::refs::LabelUse;
use crate::api::techy::node::Span;
use crate::guest::core::lowering::Lowerer;
use crate::guest::core::run::StageCx;
use crate::guest::techy::node::Node;
use crate::runtime::ExtError;

/// Mirrors [`Tag`](crate::api::stdcore::features::math::Tag): a line's tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// Whether it is starred.
    pub starred: bool,
    /// Its content.
    pub content: Vec<Node>,
}

/// `{"starred", "content": [handle, …]}`; the host's `equation_line_to_value`.
impl JsonForm for Tag {
    fn to_value(&self) -> Value {
        json!({ "starred": self.starred, "content": self.content.to_value() })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a tag";
        let map = object(WHAT, value)?;
        Ok(Tag { starred: get(WHAT, map, "starred")?, content: get(WHAT, map, "content")? })
    }
}

/// Mirrors [`LineBreak`](crate::api::stdcore::features::math::LineBreak): the
/// break that ends a line. The guest's own: the crossing form leaves out the
/// break's span, which the line's span stands for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineBreak {
    /// The break's spacing argument, if any.
    pub spacing: Option<String>,
}

/// `{"spacing"}`; the host's `equation_line_to_value`.
impl JsonForm for LineBreak {
    fn to_value(&self) -> Value {
        json!({ "spacing": self.spacing })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a line break";
        Ok(LineBreak { spacing: get(WHAT, object(WHAT, value)?, "spacing")? })
    }
}

/// Mirrors [`EquationLine`](crate::api::stdcore::features::math::EquationLine):
/// a line of an equation. The guest's own holds the line's span, which the
/// crossing form carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquationLine {
    /// The line's span: from its first node to its last, or its break's.
    pub span: Option<Span>,
    /// Its content.
    pub content: Vec<Node>,
    /// Its labels.
    pub labels: Vec<LabelUse>,
    /// Its tag, if any.
    pub tag: Option<Tag>,
    /// Whether it is not numbered.
    pub nonumber: bool,
    /// The break that ends it, if any.
    pub linebreak: Option<LineBreak>,
}

/// `{"span", "content", "labels", "tag", "nonumber", "linebreak"}`; the
/// host's `equation_line_to_value`.
impl JsonForm for EquationLine {
    fn to_value(&self) -> Value {
        json!({
            "span": self.span.as_ref().map_or(Value::Null, JsonForm::to_value),
            "content": self.content.to_value(),
            "labels": self.labels.to_value(),
            "tag": self.tag.as_ref().map_or(Value::Null, JsonForm::to_value),
            "nonumber": self.nonumber,
            "linebreak": self.linebreak.as_ref().map_or(Value::Null, JsonForm::to_value),
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "an equation line";
        let map = object(WHAT, value)?;
        Ok(EquationLine {
            span: get(WHAT, map, "span")?,
            content: get(WHAT, map, "content")?,
            labels: get(WHAT, map, "labels")?,
            tag: get(WHAT, map, "tag")?,
            nonumber: get(WHAT, map, "nonumber")?,
            linebreak: get(WHAT, map, "linebreak")?,
        })
    }
}

/// Mirrors [`LineShows`](crate::api::stdcore::features::math::LineShows):
/// what a line shows at its end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineShows {
    /// Nothing.
    Nothing,
    /// Its number.
    Number {
        /// The number's segments.
        segments: Vec<Node>,
        /// The number as a text.
        text: String,
    },
    /// Its tag.
    Tag {
        /// Whether the tag is starred.
        starred: bool,
        /// The tag's content.
        content: Vec<Node>,
    },
}

/// `{"kind": "nothing"}`, `{"kind": "number", "segments", "text"}` or
/// `{"kind": "tag", "starred", "content"}`; the host's `line_frame_to_value`
/// and `line_frame_from_value`.
impl JsonForm for LineShows {
    fn to_value(&self) -> Value {
        match self {
            LineShows::Nothing => json!({ "kind": "nothing" }),
            LineShows::Number { segments, text } => json!({ "kind": "number", "segments": segments.to_value(), "text": text }),
            LineShows::Tag { starred, content } => json!({ "kind": "tag", "starred": starred, "content": content.to_value() }),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "what a line shows";
        let map = object(WHAT, value)?;
        match map.get("kind").and_then(Value::as_str) {
            Some("nothing") => Ok(LineShows::Nothing),
            Some("number") => Ok(LineShows::Number { segments: get(WHAT, map, "segments")?, text: get(WHAT, map, "text")? }),
            Some("tag") => Ok(LineShows::Tag { starred: get(WHAT, map, "starred")?, content: get(WHAT, map, "content")? }),
            _ => Err(not_shape(WHAT, "a map whose `kind` is `nothing`, `number` or `tag`", value)),
        }
    }
}

/// Mirrors [`LineFrame`](crate::api::stdcore::features::math::LineFrame): a
/// line of an equation environment with what it shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineFrame {
    /// The environment.
    pub environment: Node,
    /// The line's index in it.
    pub index: usize,
    /// The line.
    pub line: EquationLine,
    /// The line's anchor, if any.
    pub anchor: Option<String>,
    /// What the line shows.
    pub shows: LineShows,
}

/// `{"environment": handle, "index", "line", "anchor", "shows"}`; the host's
/// `line_frame_to_value` and `line_frame_from_value` (which reads the line
/// again from the environment).
impl JsonForm for LineFrame {
    fn to_value(&self) -> Value {
        json!({
            "environment": self.environment.to_value(),
            "index": self.index,
            "line": self.line.to_value(),
            "anchor": self.anchor,
            "shows": self.shows.to_value(),
        })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a line's frame";
        let map = object(WHAT, value)?;
        Ok(LineFrame {
            environment: get(WHAT, map, "environment")?,
            index: get(WHAT, map, "index")?,
            line: get(WHAT, map, "line")?,
            anchor: get(WHAT, map, "anchor")?,
            shows: get(WHAT, map, "shows")?,
        })
    }
}

/// Mirrors [`FrameReader`](crate::api::stdcore::features::math::FrameReader):
/// what reads the frames of a line, a stage or a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameReader {
    /// A stage's context.
    Stage(StageCx),
    /// A rule's lowerer.
    Rule(Lowerer),
}

/// The handle itself, of the kind `stage_cx` or `lowerer`; read by the
/// host's `line_frames`.
impl JsonForm for FrameReader {
    fn to_value(&self) -> Value {
        match self {
            FrameReader::Stage(cx) => cx.to_value(),
            FrameReader::Rule(lw) => lw.to_value(),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        let handle = handle_from_value(value)?;
        match handle.kind {
            HandleKind::StageCx => Ok(FrameReader::Stage(StageCx { handle })),
            HandleKind::Lowerer => Ok(FrameReader::Rule(Lowerer { handle })),
            other => Err(ExtError::Argument {
                method: String::new(),
                detail: format!("an object of the kind `stage_cx` or `lowerer` is expected here, not one of the kind `{}`", other.name()),
            }),
        }
    }
}
