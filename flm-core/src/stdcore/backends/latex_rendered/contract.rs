//! The contract of the `latex-rendered` backend: what LaTeX is written for
//! each element of the [presentation vocabulary](crate::stdcore::backends::pres),
//! as data.
//!
//! One command or environment per presentation element, named `flmx` and the
//! element's name in camel case ([`Row::form`]); every field of the
//! element's payload is a key of an optional key-value argument written first
//! ([`Row::keys`]), or a mandatory argument ([`Row::args`]), or listed as not
//! written with the reason ([`Row::not_written`]). The emitter reads the row of
//! every element it writes, writes the keys in the row's order, and requires
//! the row's definition piece ([`Row::chunk`]), so that the style file holds
//! the definitions of every command the document uses. [`print_rows`] prints
//! the table for the documentation; a test pins the rows against the payload
//! types.
//!
//! The style file defines each command and environment once
//! (the chunks [`CORE_CHUNK`] to [`MARKS_CHUNK`]); a definition dispatches on
//! the element's keys, so that a template registers a look per role, kind
//! token, style, heading kind or level, or link kind (see the header comment of
//! the core chunk, `sty/flmx-core.sty`).

use crate::stdcore::backends::pres;

// --- the chunks -------------------------------------------------------------------------

/// `flm.latex.def.flmx.core`: the dispatch machinery, every `\flmxDefine…`
/// command, `\flmxKey`, `\flmxIfKey`, `\flmxTarget`, the lengths
/// `\flmxTabWidth` and `\flmxRuninSep`, the colors, `flmxBlock`, `\flmxSpan`
/// with its nine style defaults, and the three decorations. Every style file
/// holds it.
pub const CORE_CHUNK: &str = "flm.latex.def.flmx.core";
/// `flm.latex.def.flmx.headings`: `\flmxHeading` and `\flmxRuninHeading`.
pub const HEADINGS_CHUNK: &str = "flm.latex.def.flmx.headings";
/// `flm.latex.def.flmx.links`: `\flmxLink`.
pub const LINKS_CHUNK: &str = "flm.latex.def.flmx.links";
/// `flm.latex.def.flmx.lists`: `flmxList` and `\flmxItem`.
pub const LISTS_CHUNK: &str = "flm.latex.def.flmx.lists";
/// `flm.latex.def.flmx.lines`: `flmxLines` and `\flmxLine`.
pub const LINES_CHUNK: &str = "flm.latex.def.flmx.lines";
/// `flm.latex.def.flmx.verbatim`: `\flmxVerbatim`, `flmxVerbatimBlock` and
/// `\flmxVerbLine`.
pub const VERBATIM_CHUNK: &str = "flm.latex.def.flmx.verbatim";
/// `flm.latex.def.flmx.cells`: `flmxCells`.
pub const CELLS_CHUNK: &str = "flm.latex.def.flmx.cells";
/// `flm.latex.def.flmx.images`: `\flmxImage`.
pub const IMAGES_CHUNK: &str = "flm.latex.def.flmx.images";
/// `flm.latex.def.flmx.marks`: `\flmxDropped`, `\flmxUnsupported` and
/// `\flmxUnknown`.
pub const MARKS_CHUNK: &str = "flm.latex.def.flmx.marks";

/// Every chunk of the backend, the core chunk first.
pub const CHUNKS: [&str; 9] = [
    CORE_CHUNK,
    HEADINGS_CHUNK,
    LINKS_CHUNK,
    LISTS_CHUNK,
    LINES_CHUNK,
    VERBATIM_CHUNK,
    CELLS_CHUNK,
    IMAGES_CHUNK,
    MARKS_CHUNK,
];

// --- the row's parts ----------------------------------------------------------------------

/// The LaTeX form an element is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// A LaTeX paragraph between blank lines, with no command of its own.
    Paragraph,
    /// An environment `\begin{<name>}[…]` … `\end{<name>}`.
    Environment(&'static str),
    /// A command `\<name>[…]{…}`.
    Command(&'static str),
    /// An inline command or, for a block, an environment, chosen by the
    /// payload's `display`.
    CommandOrEnvironment {
        /// The inline command's name.
        command: &'static str,
        /// The block environment's name.
        environment: &'static str,
    },
    /// The mathematics delimiters around the source, byte for byte.
    Formula,
    /// Content written inside a formula's source.
    FormulaText,
    /// The decoration the mark refers to, written at the mark's place.
    AtMark,
    /// The placed tree's LaTeX, written in the node's place.
    Placed,
}

/// Where a key's value comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// A field of the element's payload.
    Field(&'static str),
    /// A field of the payload holding one value per list entry, written as a
    /// key of that entry's `\flmxItem`.
    EntryField(&'static str),
    /// A content slot of the node, lowered content.
    Slot(&'static str),
    /// A content slot of each list entry, written as a key of that entry's
    /// `\flmxItem`.
    EntrySlot(&'static str),
    /// Computed by the emitter (a heading's plain title, a link's target, a
    /// list's depth, the stop after a run-in title).
    Computed,
}

/// How a key's value is written (see [`super::emit`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Value {
    /// A name (a role, a kind, an anchor, a word), written bare.
    Name,
    /// Kind tokens, separated by a space, braced.
    Words,
    /// Lowered content, braced.
    Content,
    /// Document text computed by the emitter, braced.
    Text,
    /// An address, escaped for LaTeX, braced.
    Address,
    /// A length, spelled for LaTeX ([`super::length::latex_length`]).
    Length,
    /// A number, bare.
    Number,
    /// `true` or `false`.
    Bool,
    /// One braced key-value list per cell.
    Specs,
}

/// One key of an element's key-value argument.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    /// The key as written (`doc-role`).
    pub key: &'static str,
    /// Where its value comes from.
    pub origin: Origin,
    /// How its value is written.
    pub value: Value,
}

impl Key {
    /// The payload field the key is written from, where it comes from one.
    pub fn field(&self) -> Option<&'static str> {
        match self.origin {
            Origin::Field(field) | Origin::EntryField(field) => Some(field),
            _ => None,
        }
    }

    /// Whether the key belongs to each entry of a list (`\flmxItem`) rather
    /// than to the element itself.
    pub fn is_entry(&self) -> bool {
        matches!(self.origin, Origin::EntryField(_) | Origin::EntrySlot(_))
    }

    /// Whether the key is not the field's name with `_` replaced by `-`: a
    /// heading's `name` written `kind` and its `in_toc` written `toc`, and a
    /// list entry's anchor.
    pub fn is_renamed(&self) -> bool {
        self.field().is_some_and(|field| field.replace('_', "-") != self.key)
    }
}

/// One row of the contract: everything this backend writes for one
/// presentation element.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    /// The presentation element's name (`flm.pres.span`).
    pub element: &'static str,
    /// The LaTeX form.
    pub form: Form,
    /// The keys of the key-value argument, in the order they are written.
    pub keys: &'static [Key],
    /// The payload fields written as mandatory arguments, in order, as text.
    pub args: &'static [&'static str],
    /// The payload fields written as nothing, each with the reason.
    pub not_written: &'static [(&'static str, &'static str)],
    /// The definition piece the emitter requires where it writes the
    /// element; `None` for a node that writes no command of its own.
    pub chunk: Option<&'static str>,
}

impl Row {
    /// The key named `key`, where the row has one.
    pub fn key(&self, key: &str) -> Option<&Key> {
        self.keys.iter().find(|entry| entry.key == key)
    }
}

const fn field(key: &'static str, field: &'static str, value: Value) -> Key {
    Key { key, origin: Origin::Field(field), value }
}

const fn computed(key: &'static str, value: Value) -> Key {
    Key { key, origin: Origin::Computed, value }
}

/// The keys of both heading elements, block and run in.
const HEADING_KEYS: [Key; 7] = [
    field("kind", "name", Value::Name),
    field("level", "level", Value::Number),
    field("toc", "in_toc", Value::Bool),
    field("anchor", "anchor", Value::Name),
    field("number", "number", Value::Text),
    computed("plain", Value::Text),
    Key { key: "alt", origin: Origin::Slot(pres::ALT_SLOT), value: Value::Content },
];

/// The keys of a run-in heading: [`HEADING_KEYS`] and the stop.
const RUNIN_HEADING_KEYS: [Key; 8] = [
    HEADING_KEYS[0],
    HEADING_KEYS[1],
    HEADING_KEYS[2],
    HEADING_KEYS[3],
    HEADING_KEYS[4],
    HEADING_KEYS[5],
    HEADING_KEYS[6],
    computed("stop", Value::Text),
];

/// The contract, one row per element of the presentation vocabulary, in the
/// vocabulary's own order. Every name of [`pres::ALL`] has exactly one row,
/// which a test holds; [`row`] is the lookup the emitter uses.
pub static ROWS: &[Row] = &[
    Row {
        element: pres::PAR,
        form: Form::Paragraph,
        keys: &[],
        args: &[],
        not_written: &[("anchor", "written as `\\flmxTarget{<anchor>}` at the paragraph's start")],
        chunk: Some(CORE_CHUNK),
    },
    Row {
        element: pres::BLOCK,
        form: Form::Environment("flmxBlock"),
        keys: &[
            field("role", "role", Value::Name),
            field("kinds", "kinds", Value::Words),
            field("anchor", "anchor", Value::Name),
            field("doc-role", "doc_role", Value::Name),
        ],
        args: &[],
        not_written: &[],
        chunk: Some(CORE_CHUNK),
    },
    Row {
        element: pres::BLOCK_HEADING,
        form: Form::Command("flmxHeading"),
        keys: &HEADING_KEYS,
        args: &[],
        not_written: &[],
        chunk: Some(HEADINGS_CHUNK),
    },
    Row {
        element: pres::RUNIN_HEADING,
        form: Form::Command("flmxRuninHeading"),
        keys: &RUNIN_HEADING_KEYS,
        args: &[],
        not_written: &[],
        chunk: Some(HEADINGS_CHUNK),
    },
    Row {
        element: pres::SPAN,
        form: Form::Command("flmxSpan"),
        keys: &[
            field("style", "style", Value::Name),
            field("role", "role", Value::Name),
            field("kinds", "kinds", Value::Words),
            field("anchor", "anchor", Value::Name),
            field("doc-role", "doc_role", Value::Name),
        ],
        args: &[],
        not_written: &[],
        chunk: Some(CORE_CHUNK),
    },
    Row {
        element: pres::LIST,
        form: Form::Environment("flmxList"),
        keys: &[
            field("kinds", "kinds", Value::Words),
            field("anchor", "anchor", Value::Name),
            computed("depth", Value::Number),
            Key { key: "label", origin: Origin::EntrySlot(pres::LABEL_SLOT), value: Value::Content },
            Key {
                key: "anchor",
                origin: Origin::EntryField("entry_anchors"),
                value: Value::Name,
            },
        ],
        args: &[],
        not_written: &[],
        chunk: Some(LISTS_CHUNK),
    },
    Row {
        element: pres::CELLS,
        form: Form::Environment("flmxCells"),
        keys: &[
            field("rows", "rows", Value::Number),
            field("cols", "cols", Value::Number),
            field("cells", "cells", Value::Specs),
        ],
        args: &[],
        not_written: &[],
        chunk: Some(CELLS_CHUNK),
    },
    Row {
        element: pres::LINES,
        form: Form::Environment("flmxLines"),
        keys: &[],
        args: &[],
        not_written: &[],
        chunk: Some(LINES_CHUNK),
    },
    Row {
        element: pres::LINE,
        form: Form::Command("flmxLine"),
        keys: &[
            field("indent-left", "indent_left", Value::Length),
            field("indent-right", "indent_right", Value::Length),
            field("align", "align", Value::Name),
        ],
        args: &[],
        not_written: &[],
        chunk: Some(LINES_CHUNK),
    },
    Row {
        element: pres::VERBATIM,
        form: Form::CommandOrEnvironment {
            command: "flmxVerbatim",
            environment: "flmxVerbatimBlock",
        },
        keys: &[field("kind", "kind", Value::Name), field("lang", "lang", Value::Name)],
        args: &[],
        not_written: &[(
            "display",
            "chooses the form: the command `\\flmxVerbatim` inline, the environment \
             `flmxVerbatimBlock` for a block",
        )],
        chunk: Some(VERBATIM_CHUNK),
    },
    Row {
        element: pres::MATH_PASSTHROUGH,
        form: Form::Formula,
        keys: &[],
        args: &[],
        not_written: &[
            ("display", "chooses the delimiters: `\\(…\\)` inline, `\\[…\\]` or an environment displayed"),
            ("env", "the environment written, starred, around a displayed formula's source"),
        ],
        chunk: None,
    },
    Row {
        element: pres::MATH_TEXT,
        form: Form::FormulaText,
        keys: &[],
        args: &[],
        not_written: &[
            ("command", "the command written around the content inside the formula, `\\text` by default"),
            ("anchor", "written as `\\flmxTarget{<anchor>}` inside the formula"),
            ("at_tag", "at a tag the content is written alone, since the source holds `\\tag*{` around it"),
        ],
        chunk: None,
    },
    Row {
        element: pres::LINK,
        form: Form::Command("flmxLink"),
        keys: &[
            field("kind", "kind", Value::Name),
            field("href", "href", Value::Address),
            computed("target", Value::Name),
            field("doc-role", "doc_role", Value::Name),
            field("label", "label", Value::Text),
            field("shows-target", "shows_target", Value::Bool),
        ],
        args: &[],
        not_written: &[],
        chunk: Some(LINKS_CHUNK),
    },
    Row {
        element: pres::IMAGE,
        form: Form::Command("flmxImage"),
        keys: &[
            field("width", "width", Value::Length),
            field("height", "height", Value::Length),
            field("alt", "alt", Value::Text),
            field("display", "display", Value::Name),
            field("anchor", "anchor", Value::Name),
        ],
        args: &[],
        not_written: &[
            ("src", "the path is the command's mandatory argument"),
            ("srcset", "the candidates a browser chooses among; LaTeX includes the one file"),
        ],
        chunk: Some(IMAGES_CHUNK),
    },
    Row {
        element: pres::UNSUPPORTED,
        form: Form::Command("flmxUnsupported"),
        keys: &[],
        args: &["element", "message"],
        not_written: &[],
        chunk: Some(MARKS_CHUNK),
    },
    Row {
        element: pres::DROPPED,
        form: Form::Command("flmxDropped"),
        keys: &[],
        args: &["condition", "detail"],
        not_written: &[],
        chunk: Some(MARKS_CHUNK),
    },
    Row {
        element: pres::UNKNOWN,
        form: Form::Command("flmxUnknown"),
        keys: &[field("kind", "kind", Value::Name)],
        args: &["name", "spelling"],
        not_written: &[],
        chunk: Some(MARKS_CHUNK),
    },
    Row {
        element: pres::DECO_RUNIN,
        form: Form::Command("flmxDecoRunin"),
        keys: &[computed("stop", Value::Text)],
        args: &[],
        not_written: &[],
        chunk: Some(CORE_CHUNK),
    },
    Row {
        element: pres::DECO_FINAL,
        form: Form::Command("flmxDecoFinal"),
        keys: &[],
        args: &[],
        not_written: &[],
        chunk: Some(CORE_CHUNK),
    },
    Row {
        element: pres::DECO_MARGIN,
        form: Form::Command("flmxDecoMargin"),
        keys: &[],
        args: &[],
        not_written: &[],
        chunk: Some(CORE_CHUNK),
    },
    Row {
        element: pres::DECO_MARK,
        form: Form::AtMark,
        keys: &[],
        args: &[],
        not_written: &[("slot", "identifies the decoration written in the mark's place")],
        chunk: None,
    },
    Row {
        element: pres::SUBTREE,
        form: Form::Placed,
        keys: &[],
        args: &[],
        not_written: &[("tree", "identifies the document tree written in the node's place")],
        chunk: None,
    },
];

/// The row of the presentation element `element`, or `None` for a name
/// outside the vocabulary.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex_rendered::contract::{Form, row};
/// use flm_core::stdcore::backends::pres;
///
/// let span = row(pres::SPAN).expect("a row");
/// assert_eq!(span.form, Form::Command("flmxSpan"));
/// assert_eq!(span.keys.iter().map(|key| key.key).collect::<Vec<_>>(),
///            ["style", "role", "kinds", "anchor", "doc-role"]);
/// ```
pub fn row(element: &str) -> Option<&'static Row> {
    ROWS.iter().find(|row| row.element == element)
}

/// The table printed for the documentation: one line per element, its LaTeX
/// form, its keys (a renamed key with its field, `kind (name)`), its
/// mandatory arguments and its chunk.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex_rendered::contract::print_rows;
///
/// let table = print_rows();
/// assert!(table.lines().any(|line| line.starts_with("flm.pres.span: \\flmxSpan[style, role")));
/// ```
pub fn print_rows() -> String {
    let mut out = String::new();
    for row in ROWS {
        let keys: Vec<String> = row
            .keys
            .iter()
            .map(|key| match key.origin {
                Origin::Field(field) if key.is_renamed() => format!("{} ({field})", key.key),
                Origin::EntryField(field) => format!("item {} ({field})", key.key),
                Origin::EntrySlot(slot) => format!("item {} (slot {slot})", key.key),
                Origin::Slot(slot) => format!("{} (slot {slot})", key.key),
                _ => String::from(key.key),
            })
            .collect();
        let keys = if keys.is_empty() { String::new() } else { format!("[{}]", keys.join(", ")) };
        let args: String = row.args.iter().map(|arg| format!("{{{arg}}}")).collect();
        let form = match row.form {
            Form::Paragraph => String::from("a paragraph"),
            Form::Environment(name) => format!("\\begin{{{name}}}{keys}{args} … \\end{{{name}}}"),
            Form::Command(name) => format!("\\{name}{keys}{args}"),
            Form::CommandOrEnvironment { command, environment } => {
                format!("\\{command}{keys} or \\begin{{{environment}}}{keys} … \\end{{{environment}}}")
            }
            Form::Formula => String::from("the formula's delimiters around its source"),
            Form::FormulaText => String::from("content inside the formula's source"),
            Form::AtMark => String::from("the decoration at the mark's place"),
            Form::Placed => String::from("the placed tree in place"),
        };
        let chunk = row.chunk.map(|chunk| format!(" ({chunk})")).unwrap_or_default();
        out.push_str(&format!("{}: {form}{chunk}\n", row.element));
    }
    out
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use techy::serialize::SerialValue;

    use super::*;
    use crate::lang::PresPayload;
    use crate::model::addr::TreeId;
    use crate::model::names::{ElementName, Name};
    use crate::stdcore::backends::pres::{
        BlockPayload, CellSpec, CellsPayload, DecoMarkPayload, DecorationPayload, DocRole,
        DroppedPayload, HeadingPayload, ImageDisplay, ImagePayload, Length, LineAlign,
        LinePayload, LinesPayload, LinkPayload, ListPayload, MathDisplay, MathPassthroughPayload,
        MathTextPayload, ParPayload, SpanPayload, SpanStyle, SrcsetCandidate, SubtreePayload,
        UnknownKind, UnknownPayload, UnsupportedPayload, VerbatimDisplay, VerbatimKind,
        VerbatimPayload,
    };

    fn name(text: &str) -> Name {
        Name::from(text)
    }

    /// Every payload of the vocabulary, fully populated, beside its element.
    fn populated_payloads() -> Vec<(&'static str, Arc<dyn PresPayload>)> {
        let anchor = || Some(String::from("a-1"));
        let heading = || HeadingPayload { name: name("section"), level: 1, in_toc: true, anchor: anchor(), number: Some(String::from("1")) };
        let mut cell = CellSpec::at(1, 1);
        cell.styles = vec![name("green")];
        vec![
            (pres::PAR, Arc::new(ParPayload { anchor: anchor() }) as Arc<dyn PresPayload>),
            (
                pres::BLOCK,
                Arc::new(BlockPayload {
                    role: Some(name("flm.theorems.theorem")),
                    kinds: vec![name("theorem")],
                    anchor: anchor(),
                    doc_role: Some(DocRole::Endnotes),
                }),
            ),
            (pres::BLOCK_HEADING, Arc::new(heading())),
            (pres::RUNIN_HEADING, Arc::new(heading())),
            (
                pres::SPAN,
                Arc::new(SpanPayload {
                    style: Some(SpanStyle::Bold),
                    role: Some(name("flm.defterm.term")),
                    kinds: vec![name("primary")],
                    anchor: anchor(),
                    doc_role: Some(DocRole::Footnote),
                }),
            ),
            (
                pres::LIST,
                Arc::new(ListPayload {
                    kinds: vec![name("numbered")],
                    anchor: anchor(),
                    entry_anchors: vec![Some(String::from("item-a"))],
                }),
            ),
            (pres::CELLS, Arc::new(CellsPayload { rows: 1, cols: 1, cells: vec![cell] })),
            (pres::LINES, Arc::new(LinesPayload)),
            (
                pres::LINE,
                Arc::new(LinePayload {
                    indent_left: Length::tabs(1),
                    indent_right: Length::tabs(2),
                    align: LineAlign::Center,
                }),
            ),
            (
                pres::VERBATIM,
                Arc::new(VerbatimPayload {
                    kind: VerbatimKind::Code,
                    display: VerbatimDisplay::Block,
                    lang: Some(String::from("rust")),
                }),
            ),
            (
                pres::MATH_PASSTHROUGH,
                Arc::new(MathPassthroughPayload { display: MathDisplay::Display, env: Some(String::from("align")) }),
            ),
            (
                pres::MATH_TEXT,
                Arc::new(MathTextPayload { command: Some(String::from("text")), anchor: anchor(), at_tag: true }),
            ),
            (
                pres::LINK,
                Arc::new(LinkPayload {
                    href: String::from("#a-1"),
                    kind: name("ref"),
                    doc_role: Some(DocRole::NoteRef),
                    label: Some(String::from("Footnote 1")),
                    shows_target: true,
                }),
            ),
            (
                pres::IMAGE,
                Arc::new(ImagePayload {
                    src: String::from("a.png"),
                    display: ImageDisplay::Block,
                    alt: Some(String::from("a")),
                    width: Some(Length::tabs(1)),
                    height: Some(Length::tabs(1)),
                    srcset: vec![SrcsetCandidate { src: String::from("b.png"), descriptor: String::from("2x") }],
                    anchor: anchor(),
                }),
            ),
            (
                pres::UNSUPPORTED,
                Arc::new(UnsupportedPayload { element: ElementName::from("x.box"), message: String::from("why") }),
            ),
            (
                pres::DROPPED,
                Arc::new(DroppedPayload { condition: name("x.left-out"), detail: String::from("what") }),
            ),
            (
                pres::UNKNOWN,
                Arc::new(UnknownPayload {
                    kind: UnknownKind::Macro,
                    name: String::from("foo"),
                    spelling: String::from("\\foo"),
                }),
            ),
            (pres::DECO_RUNIN, Arc::new(DecorationPayload)),
            (pres::DECO_FINAL, Arc::new(DecorationPayload)),
            (pres::DECO_MARGIN, Arc::new(DecorationPayload)),
            (pres::DECO_MARK, Arc::new(DecoMarkPayload { slot: 1 })),
            (pres::SUBTREE, Arc::new(SubtreePayload { tree: TreeId::tree(1) })),
        ]
    }

    #[test]
    fn every_element_of_the_vocabulary_has_exactly_one_row() {
        let mut elements: Vec<&str> = ROWS.iter().map(|row| row.element).collect();
        elements.sort_unstable();
        let mut all = pres::ALL.to_vec();
        all.sort_unstable();
        assert_eq!(elements, all);
        assert_eq!(populated_payloads().len(), ROWS.len());
    }

    #[test]
    fn every_payload_field_is_a_key_an_argument_or_not_written_with_a_reason() {
        for (element, payload) in populated_payloads() {
            let row = row(element).expect("a row for every element");
            let SerialValue::Map(entries) = payload.project() else {
                panic!("`{element}` projects as something other than a map");
            };
            let projected: Vec<&str> = entries.iter().map(|(key, _)| key.as_str()).collect();
            for key in &projected {
                let known = row.keys.iter().any(|entry| entry.field() == Some(key))
                    || row.args.contains(key)
                    || row.not_written.iter().any(|(field, _)| field == key);
                assert!(known, "`{element}`'s field `{key}` is not in its row");
            }
            let named = row
                .keys
                .iter()
                .filter_map(Key::field)
                .chain(row.args.iter().copied())
                .chain(row.not_written.iter().map(|(field, _)| *field));
            for field in named {
                assert!(projected.contains(&field), "`{element}`'s row names `{field}`, not projected");
            }
        }
    }

    #[test]
    fn the_keys_of_a_row_are_the_field_names_with_hyphens_but_the_two_heading_renames() {
        let renamed: Vec<(&str, &str)> = ROWS
            .iter()
            .flat_map(|row| row.keys.iter())
            .filter(|key| key.is_renamed())
            .map(|key| (key.key, key.field().unwrap_or_default()))
            .collect();
        assert_eq!(
            renamed,
            [("kind", "name"), ("toc", "in_toc"), ("kind", "name"), ("toc", "in_toc"), ("anchor", "entry_anchors")]
        );
    }

    #[test]
    fn the_printed_table_holds_one_line_per_element() {
        let table = print_rows();
        assert_eq!(table.lines().count(), ROWS.len());
        assert!(table.contains(
            "flm.pres.block-heading: \\flmxHeading[kind (name), level, toc (in_toc), anchor, number, plain, alt (slot alt)] (flm.latex.def.flmx.headings)"
        ), "{table}");
        assert!(table.contains("flm.pres.dropped: \\flmxDropped{condition}{detail} (flm.latex.def.flmx.marks)"), "{table}");
    }
}
