//! The guest's own types of `flm:stdlib/features/math`: the lines of an
//! equation and their frames, which hold nodes ([`EquationLine`], [`Tag`],
//! [`LineBreak`], [`LineFrame`], [`LineShows`]), the TeX of a formula, whose
//! holes hold nodes ([`TexPart`], [`TextHole`], [`LineTex`]), and
//! [`FrameReader`].

use serde_json::{json, Value};

use super::{get, not_shape, object, single};
use crate::adapter::handles::HandleKind;
#[cfg(feature = "native")]
use crate::adapter::handles::{CallTable, Handle};
#[cfg(feature = "native")]
use crate::adapter::typed::{FromApi, IntoApi};
#[cfg(feature = "native")]
use crate::adapter::values::line_frame_of;
use crate::adapter::values::{handle_from_value, JsonForm};
#[cfg(feature = "native")]
use crate::api;
use crate::api::stdlib::features::refs::LabelUse;
use crate::api::techy::node::Span;
use crate::guest::core::lowering::Lowerer;
use crate::guest::core::run::StageCx;
use crate::guest::techy::node::{Node, Nodes, Tree};
use crate::runtime::ExtError;

/// Mirrors [`Tag`](crate::api::stdlib::features::math::Tag): a line's tag.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tag {
    /// Whether it is starred.
    pub starred: bool,
    /// Its content.
    pub content: Nodes,
}

/// `{"starred", "content": nodes}`, the nodes a sequence; the host's
/// `equation_line_to_value`.
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

/// Mirrors [`LineBreak`](crate::api::stdlib::features::math::LineBreak): the
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

/// Mirrors [`EquationLine`](crate::api::stdlib::features::math::EquationLine):
/// a line of an equation. The guest's own holds the line's span, which the
/// crossing form carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EquationLine {
    /// The line's span: from its first node to its last, or its break's.
    pub span: Option<Span>,
    /// Its content.
    pub content: Nodes,
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

/// The api's `EquationLine`, its nodes lent, its span from its first content
/// node's start to its last one's end (else the span of its line break),
/// its line break without its span; the host's `equation_line_to_value`.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, api::stdlib::features::math::EquationLine<'c>> for EquationLine {
    fn from_api(api: api::stdlib::features::math::EquationLine<'c>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        let span = match (api.content.first(), api.content.last(), &api.linebreak) {
            (Some(first), Some(last), _) => Some(Span { start: first.span().start, end: last.span().end }),
            (_, _, Some(brk)) => Some(brk.span),
            _ => None,
        };
        let tag = match api.tag {
            Some(tag) => Some(Tag { starred: tag.starred, content: FromApi::from_api(tag.content, table)? }),
            None => None,
        };
        Ok(EquationLine {
            span,
            content: FromApi::from_api(api.content, table)?,
            labels: api.labels,
            tag,
            nonumber: api.nonumber,
            linebreak: api.linebreak.map(|brk| LineBreak { spacing: brk.spacing }),
        })
    }
}

/// Mirrors [`LineShows`](crate::api::stdlib::features::math::LineShows):
/// what a line shows at its end.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineShows {
    /// Nothing.
    Nothing,
    /// Its number.
    Number {
        /// The number's segments, each the tree of a synthesized value.
        segments: Vec<Tree>,
        /// The number as a text.
        text: String,
    },
    /// Its tag.
    Tag {
        /// Whether the tag is starred.
        starred: bool,
        /// The tag's content.
        content: Nodes,
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

/// Mirrors [`LineFrame`](crate::api::stdlib::features::math::LineFrame): a
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

/// The api's `LineFrame`: its environment, index, anchor and what it shows,
/// its nodes and trees copied out of the table, its line read again from
/// the environment at the index; the host's `line_frame_from_value`.
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for &LineFrame {
    type Api = api::stdlib::features::math::LineFrame<'c>;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        use api::stdlib::features::math::LineShows as Api;
        let environment = (&self.environment).into_api(table)?;
        let shows = match &self.shows {
            LineShows::Nothing => Api::Nothing,
            LineShows::Number { segments, text } => Api::Number {
                segments: segments.iter().map(|tree| tree.into_api(table)).collect::<Result<_, _>>()?,
                text: text.clone(),
            },
            LineShows::Tag { starred, content } => Api::Tag { starred: *starred, content: content.into_api(table)? },
        };
        Ok(line_frame_of(environment, self.index, self.anchor.clone(), shows))
    }
}

/// A list of lines' frames, each as [`LineFrame`]'s conversion.
#[cfg(feature = "native")]
impl<'c> IntoApi<'c> for &[LineFrame] {
    type Api = Vec<api::stdlib::features::math::LineFrame<'c>>;

    fn into_api(self, table: &mut CallTable<'c>) -> Result<Self::Api, ExtError> {
        self.iter().map(|frame| frame.into_api(table)).collect()
    }
}

/// The api's `LineFrame`, its nodes and trees lent; the host's
/// `line_frame_to_value`.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, api::stdlib::features::math::LineFrame<'c>> for LineFrame {
    fn from_api(api: api::stdlib::features::math::LineFrame<'c>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        use api::stdlib::features::math::LineShows as Api;
        let environment = FromApi::from_api(api.environment, table)?;
        let line = FromApi::from_api(api.line, table)?;
        let shows = match api.shows {
            Api::Nothing => LineShows::Nothing,
            Api::Number { segments, text } => LineShows::Number { segments: FromApi::from_api(segments, table)?, text },
            Api::Tag { starred, content } => LineShows::Tag { starred, content: FromApi::from_api(content, table)? },
        };
        Ok(LineFrame { environment, index: api.index, line, anchor: api.anchor, shows })
    }
}

/// Mirrors [`TexPart`](crate::api::stdlib::features::math::TexPart): one part
/// of a formula's TeX.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TexPart {
    /// TeX text, never empty.
    Tex(String),
    /// The place of a text command.
    Hole(TextHole),
}

/// `{"tex": text}` or `{"hole": {"command", "node"}}`; the host's
/// `tex_part_to_value`.
impl JsonForm for TexPart {
    fn to_value(&self) -> Value {
        match self {
            TexPart::Tex(text) => json!({ "tex": text }),
            TexPart::Hole(hole) => json!({ "hole": hole.to_value() }),
        }
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a part of a formula's TeX";
        match single(value, &["tex", "hole"]) {
            Some(("tex", text)) => Ok(TexPart::Tex(JsonForm::from_value(text)?)),
            Some((_, hole)) => Ok(TexPart::Hole(JsonForm::from_value(hole)?)),
            None => Err(not_shape(WHAT, "`{\"tex\": text}` or `{\"hole\": {\"command\", \"node\"}}`", value)),
        }
    }
}

/// The api's `TexPart`, its hole's node lent; the host's `tex_part_to_value`.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, api::stdlib::features::math::TexPart<'c>> for TexPart {
    fn from_api(api: api::stdlib::features::math::TexPart<'c>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        use api::stdlib::features::math::TexPart as Api;
        Ok(match api {
            Api::Tex(text) => TexPart::Tex(text),
            Api::Hole(hole) => TexPart::Hole(TextHole { command: hole.command, node: FromApi::from_api(hole.node, table)? }),
        })
    }
}

/// Mirrors [`TextHole`](crate::api::stdlib::features::math::TextHole): a
/// text command inside a formula.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextHole {
    /// The command's name, without the escape character.
    pub command: String,
    /// The text command's node.
    pub node: Node,
}

/// `{"command", "node": handle}`; the host's `tex_part_to_value`.
impl JsonForm for TextHole {
    fn to_value(&self) -> Value {
        json!({ "command": self.command, "node": self.node.to_value() })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "a hole of a formula's TeX";
        let map = object(WHAT, value)?;
        Ok(TextHole { command: get(WHAT, map, "command")?, node: get(WHAT, map, "node")? })
    }
}

/// Mirrors [`LineTex`](crate::api::stdlib::features::math::LineTex): the TeX
/// of an equation line's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineTex {
    /// The content's TeX, in order.
    pub parts: Vec<TexPart>,
    /// The run of whitespace that ended the content.
    pub trailing: String,
}

impl LineTex {
    /// Mirrors [`LineTex::ends_line`](crate::api::stdlib::features::math::LineTex::ends_line):
    /// whether the trailing run is not empty.
    pub fn ends_line(&self) -> bool {
        !self.trailing.is_empty()
    }
}

/// `{"parts", "trailing"}`; the host's `line_tex_to_value`.
impl JsonForm for LineTex {
    fn to_value(&self) -> Value {
        json!({ "parts": self.parts.to_value(), "trailing": self.trailing })
    }

    fn from_value(value: &Value) -> Result<Self, ExtError> {
        const WHAT: &str = "an equation line's TeX";
        let map = object(WHAT, value)?;
        Ok(LineTex { parts: get(WHAT, map, "parts")?, trailing: get(WHAT, map, "trailing")? })
    }
}

/// The api's `LineTex`, its holes' nodes lent; the host's
/// `line_tex_to_value`.
#[cfg(feature = "native")]
impl<'c> FromApi<'c, api::stdlib::features::math::LineTex<'c>> for LineTex {
    fn from_api(api: api::stdlib::features::math::LineTex<'c>, table: &mut CallTable<'c>) -> Result<Self, ExtError> {
        Ok(LineTex { parts: FromApi::from_api(api.parts, table)?, trailing: api.trailing })
    }
}

/// Mirrors [`FrameReader`](crate::api::stdlib::features::math::FrameReader):
/// what reads the frames of a line, a stage or a rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameReader {
    /// A stage's context.
    Stage(StageCx),
    /// A rule's lowerer.
    Rule(Lowerer),
}

#[cfg(feature = "native")]
impl FrameReader {
    /// The reader's handle, of the kind `stage_cx` or `lowerer`, by which the
    /// host's `line_frames` takes the reader from the call's table.
    pub(crate) fn handle(&self) -> Handle {
        match self {
            FrameReader::Stage(cx) => cx.handle,
            FrameReader::Rule(lw) => lw.handle,
        }
    }
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
