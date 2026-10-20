//! The typed payloads of the presentation elements, their projections and
//! their readers.
//!
//! Every presentation node (a node of a presentation element; see
//! [`super`]) carries exactly one payload value, of the type its element
//! declares, and that value holds every fact the node states beyond its
//! element and its children. This module defines one payload type per
//! element, together with the small enumerations they use ([`SpanStyle`],
//! [`DocRole`], [`VerbatimKind`], [`VerbatimDisplay`], [`MathDisplay`],
//! [`ImageDisplay`], [`LineAlign`], [`UnknownKind`], and for a table of cells
//! [`CellAlign`], [`CellHeader`], [`CellEdge`] and [`GridPosition`]).
//!
//! # Roles, kinds and document roles
//!
//! A **role** is a feature-owned dotted name on a presentation payload that
//! says what a container is (a theorem's heading, a quotation's attribution),
//! and **kinds** (kind tokens) are feature-owned names that classify it
//! further, so that a stylesheet or a host's lowering rule can select on
//! them. Both are [`Name`]s owned by the feature that introduced them, so two
//! features can choose their own roles without colliding. A **document
//! role** ([`DocRole`]) is a different thing: one of a small fixed set of
//! structural roles (a list of notes, a link to a cited work, a table of
//! contents) that a backend
//! with such a notion renders (the HTML backend writes it as the `role`
//! attribute) and every other backend ignores. See [decorations, roles and
//! kinds](crate::guide::concepts_overview#decorations-roles-and-kinds).
//!
//! # Anchors
//!
//! A payload's anchor field is a plain `String`: the anchor of the node, that
//! is, the identifier a link points at when the node is a link target. A
//! stage computed it before lowering (for reference points, the refs
//! feature's anchors stage writes it into the table
//! [`ANCHORS`](crate::stdcore::features::refs::ANCHORS)); the payload holds
//! the final identifier, not a reference to be resolved later.
//!
//! # Projections and readers
//!
//! Every payload type implements [`PresPayload`]. Its
//! [`project`](PresPayload::project) method returns the payload's
//! **projection**: the serialized form in which a lowered tree's payloads are
//! printed ([`crate::dump::project`]) and stored ([`crate::dump::session`]).
//! The reader registered for the element ([`payload_readers`]) rebuilds the
//! typed value from exactly that projection, through the `from_projection`
//! function of each type below. A projection is owned data made of names, numbers and strings, never a span
//! or a node identifier, so a lowered tree can be written out and read back
//! without a tree to resolve identifiers against.
//!
//! A projection is one map per payload, its keys in a fixed order. An
//! optional field is a key present only when it has a value, and a list
//! field a key present only when the list is not empty, which keeps stored
//! projections short. The keys per element:
//!
//! - `par`: `{anchor?}`; `block`: `{role?, kinds?, anchor?, doc_role?}`;
//!   `block-heading` and `runin-heading`: `{name, level, in_toc, anchor?}`;
//!   `span`: `{style?, role?, kinds?, anchor?, doc_role?}`; `list`: `{kinds?,
//!   anchor?, entry_anchors?}`;
//! - `cells`: `{rows, cols, cells}`, the cells a list of maps `{row, col,
//!   row_span?, col_span?, align?, header?, rules?, styles?}` (a span present
//!   only when it is not one);
//! - `lines`: `{}`; `line`: `{indent_left, indent_right, align}`, the two
//!   indentations each a [`Length`]'s text (`2tab`);
//! - `verbatim`: `{kind, display, lang?}`; `math-passthrough`: `{display, env?}`;
//!   `math-text`: `{command?, anchor?, at_tag?}` (`at_tag` present only when
//!   true);
//! - `link`: `{href, kind, doc_role?, label?, shows_target?}`
//!   (`shows_target` present only when true); `image`: `{src, display, alt?,
//!   width?, height?, srcset?, anchor?}`;
//! - `unsupported`: `{element, message}`; `dropped`: `{condition, detail}`;
//!   `unknown`: `{kind, name, spelling}`;
//! - the three decorations: `{}`; `deco-mark`: `{slot}`; `subtree`: `{tree}`
//!   (the tree identifier's display form, `tree#3`).

use std::fmt;
use std::sync::Arc;

use techy::serialize::{DeserializeContext, DeserializeError, SerialValue};

use crate::model::addr::TreeId;
use crate::model::names::{ElementName, Name};
use crate::dump::session::PayloadReaders;
use crate::lang::{Flm, PresPayload};

use super::length::Length;
use super::names;

// --- projection helpers -----------------------------------------------------------------
//
// The six that carry no knowledge of these payloads — `text`, `map`, `bad`,
// `entries`, `field` and `string` — are `pub(crate)` because the construct
// elements of the backend families (`crate::stdcore::backends::latex::construct`,
// `crate::stdcore::backends::html::construct`) project and read their payloads
// the same way and share them.

/// A string as serial data.
pub(crate) fn text(value: impl Into<String>) -> SerialValue {
    SerialValue::Str(value.into())
}

/// A projection's map, its keys in the order given.
pub(crate) fn map(entries: Vec<(&str, SerialValue)>) -> SerialValue {
    SerialValue::Map(entries.into_iter().map(|(key, value)| (String::from(key), value)).collect())
}

/// A list of names as a list of strings.
fn names(list: &[Name]) -> SerialValue {
    SerialValue::List(list.iter().map(|name| text(name.as_str())).collect())
}

/// Pushes `key: value` when the option holds a value.
fn push_opt(
    entries: &mut Vec<(&'static str, SerialValue)>,
    key: &'static str,
    value: Option<SerialValue>,
) {
    if let Some(value) = value {
        entries.push((key, value));
    }
}

/// The reading side's failure for a payload of `element`.
pub(crate) fn bad(element: &str, detail: impl fmt::Display) -> DeserializeError {
    DeserializeError::failed(format!("the payload of `{element}`: {detail}"))
}

/// The entries of a projected payload: a map, or the failure mentioning the
/// element.
pub(crate) fn entries<'v>(
    element: &str,
    value: &'v SerialValue,
) -> Result<&'v [(String, SerialValue)], DeserializeError> {
    match value {
        SerialValue::Map(entries) => Ok(entries),
        other => Err(bad(element, format!("expected a map, found {other:?}"))),
    }
}

/// The value stored under `key`, if the projection has that key.
pub(crate) fn field<'v>(
    entries: &'v [(String, SerialValue)],
    key: &str,
) -> Option<&'v SerialValue> {
    entries.iter().find(|(k, _)| k == key).map(|(_, v)| v)
}

/// An optional string field: absent and null read alike as `None`.
fn opt_string(
    element: &str,
    entries: &[(String, SerialValue)],
    key: &str,
) -> Result<Option<String>, DeserializeError> {
    match field(entries, key) {
        None | Some(SerialValue::Null) => Ok(None),
        Some(SerialValue::Str(s)) => Ok(Some(s.clone())),
        Some(other) => Err(bad(element, format!("`{key}` must be a string, found {other:?}"))),
    }
}

/// A required string field.
pub(crate) fn string(
    element: &str,
    entries: &[(String, SerialValue)],
    key: &str,
) -> Result<String, DeserializeError> {
    opt_string(element, entries, key)?.ok_or_else(|| bad(element, format!("`{key}` is missing")))
}

/// An optional integer field: absent and null read alike as `None`.
fn opt_int(
    element: &str,
    entries: &[(String, SerialValue)],
    key: &str,
) -> Result<Option<i64>, DeserializeError> {
    match field(entries, key) {
        None | Some(SerialValue::Null) => Ok(None),
        Some(SerialValue::Int(n)) => Ok(Some(*n)),
        Some(other) => Err(bad(element, format!("`{key}` must be an integer, found {other:?}"))),
    }
}

/// A required integer field.
fn int(
    element: &str,
    entries: &[(String, SerialValue)],
    key: &str,
) -> Result<i64, DeserializeError> {
    opt_int(element, entries, key)?.ok_or_else(|| bad(element, format!("`{key}` is missing")))
}

/// A list-of-names field; an absent or null key reads as the empty list.
fn name_list(
    element: &str,
    entries: &[(String, SerialValue)],
    key: &str,
) -> Result<Vec<Name>, DeserializeError> {
    match field(entries, key) {
        None | Some(SerialValue::Null) => Ok(Vec::new()),
        Some(SerialValue::List(items)) => items
            .iter()
            .map(|item| match item {
                SerialValue::Str(s) => Ok(Name::from(s.as_str())),
                other => Err(bad(element, format!("`{key}` holds a non-string {other:?}"))),
            })
            .collect(),
        Some(other) => Err(bad(element, format!("`{key}` must be a list, found {other:?}"))),
    }
}

/// An optional name field.
fn name_opt(
    element: &str,
    entries: &[(String, SerialValue)],
    key: &str,
) -> Result<Option<Name>, DeserializeError> {
    Ok(opt_string(element, entries, key)?.map(Name::from))
}

/// An optional document-role field, read by its stable name
/// ([`DocRole::parse`]).
fn doc_role_opt(
    element: &str,
    entries: &[(String, SerialValue)],
) -> Result<Option<DocRole>, DeserializeError> {
    match opt_string(element, entries, "doc_role")? {
        None => Ok(None),
        Some(name) => DocRole::parse(&name)
            .map(Some)
            .ok_or_else(|| bad(element, format!("unknown document role `{name}`"))),
    }
}

/// An integer field narrowed to `0..=max`, refused outside that range.
fn small(element: &str, key: &str, value: i64, max: i64) -> Result<u16, DeserializeError> {
    if (0..=max).contains(&value) {
        Ok(value as u16)
    } else {
        Err(bad(element, format!("`{key}` = {value} is outside 0..={max}")))
    }
}

// --- the enumerations -------------------------------------------------------------------

/// The display style of a `flm.pres.span` node: how the span is set, not what
/// it means. A span with no style still groups its content and may carry a
/// role and kinds; the style only designates a visual form.
///
/// The set is fixed and small. It covers the base formatting a text-like
/// markup language needs, plus [semantic emphasis](SpanStyle::Emphasis), which
/// is kept apart from plain italics because output formats that distinguish
/// the two (HTML's `<em>` beside `<i>`) need both. An emitter that cannot
/// express a style ignores it and writes the content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpanStyle {
    /// Semantic emphasis (`\emph`; HTML `<em>`).
    Emphasis,
    /// Bold weight (`\textbf`).
    Bold,
    /// Italic shape (`\textit`).
    Italic,
    /// A monospaced face (`\texttt`).
    Monospace,
    /// Underlined.
    Underline,
    /// Small capitals.
    SmallCaps,
    /// Raised, smaller.
    Superscript,
    /// Lowered, smaller.
    Subscript,
    /// The display of a reference whose target could not be resolved.
    UnresolvedReference,
}

impl SpanStyle {
    /// Every style, in declaration order.
    pub const ALL: [SpanStyle; 9] = [
        SpanStyle::Emphasis,
        SpanStyle::Bold,
        SpanStyle::Italic,
        SpanStyle::Monospace,
        SpanStyle::Underline,
        SpanStyle::SmallCaps,
        SpanStyle::Superscript,
        SpanStyle::Subscript,
        SpanStyle::UnresolvedReference,
    ];

    /// The style's stable name: its text in the projection, and the value of
    /// the `data-flm-style` attribute of the HTML contract.
    pub fn as_str(self) -> &'static str {
        match self {
            SpanStyle::Emphasis => "emph",
            SpanStyle::Bold => "bold",
            SpanStyle::Italic => "italic",
            SpanStyle::Monospace => "monospace",
            SpanStyle::Underline => "underline",
            SpanStyle::SmallCaps => "small-caps",
            SpanStyle::Superscript => "superscript",
            SpanStyle::Subscript => "subscript",
            SpanStyle::UnresolvedReference => "unresolved-reference",
        }
    }

    /// The style of a stable name, if any.
    pub fn parse(name: &str) -> Option<SpanStyle> {
        SpanStyle::ALL.into_iter().find(|style| style.as_str() == name)
    }
}

impl fmt::Display for SpanStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The verbatim kind of a `flm.pres.verbatim` node: plain characters,
/// computer code, or an identifier. A backend chooses from it how to render
/// the content: an HTML backend can give each kind its own class, while the
/// text backend writes the content alone, since plain text cannot show the
/// distinction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum VerbatimKind {
    /// Characters typeset as they are, with no formatting of their own
    /// (`\verbtext`, `{verbatimtext}`); also the kind of a bare verbatim
    /// group that no element of a feature owns.
    #[default]
    Text,
    /// Computer code (`\verbcode`, `{verbatimcode}`), possibly in the
    /// language the payload's `lang` field gives.
    Code,
    /// An identifier: a keyword or a name, typically set in italics rather
    /// than highlighted as code (`\verba`). Its stable name is `a`, after the
    /// macro that writes it.
    Identifier,
}

impl VerbatimKind {
    /// Every kind, in the order the feature declares its callables.
    pub const ALL: [VerbatimKind; 3] =
        [VerbatimKind::Text, VerbatimKind::Code, VerbatimKind::Identifier];

    /// The stable name.
    pub fn as_str(self) -> &'static str {
        match self {
            VerbatimKind::Text => "text",
            VerbatimKind::Code => "code",
            VerbatimKind::Identifier => "a",
        }
    }

    /// The kind of a stable name, if any.
    pub fn parse(name: &str) -> Option<VerbatimKind> {
        VerbatimKind::ALL.into_iter().find(|kind| kind.as_str() == name)
    }
}

impl fmt::Display for VerbatimKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether verbatim content is a block of its own or is part of a
/// paragraph's flow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VerbatimDisplay {
    /// Inside a paragraph's flow.
    Inline,
    /// A block of its own.
    Block,
}

impl VerbatimDisplay {
    /// The stable name.
    pub fn as_str(self) -> &'static str {
        match self {
            VerbatimDisplay::Inline => "inline",
            VerbatimDisplay::Block => "block",
        }
    }

    /// The display of a stable name, if any.
    pub fn parse(name: &str) -> Option<VerbatimDisplay> {
        match name {
            "inline" => Some(VerbatimDisplay::Inline),
            "block" => Some(VerbatimDisplay::Block),
            _ => None,
        }
    }
}

/// Whether an image is a block of its own or is inline, like a word of the
/// text around it.
///
/// An image is a block where it is a direct child of a float's body, and
/// inline content anywhere else
/// ([`GraphicsImage`](crate::stdcore::features::graphics::GraphicsImage)).
/// The default is [`Inline`](ImageDisplay::Inline).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum ImageDisplay {
    /// Inside a paragraph's flow.
    #[default]
    Inline,
    /// A block of its own.
    Block,
}

impl ImageDisplay {
    /// The stable name: `inline` or `block`.
    pub fn as_str(self) -> &'static str {
        match self {
            ImageDisplay::Inline => "inline",
            ImageDisplay::Block => "block",
        }
    }

    /// The display of a stable name, if any.
    pub fn parse(name: &str) -> Option<ImageDisplay> {
        match name {
            "inline" => Some(ImageDisplay::Inline),
            "block" => Some(ImageDisplay::Block),
            _ => None,
        }
    }
}

/// Whether math is inline or displayed (`\(…\)` versus `\[…\]` and the math
/// environments).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MathDisplay {
    /// Inline math.
    Inline,
    /// Display math.
    Display,
}

impl MathDisplay {
    /// The stable name.
    pub fn as_str(self) -> &'static str {
        match self {
            MathDisplay::Inline => "inline",
            MathDisplay::Display => "display",
        }
    }

    /// The display of a stable name, if any.
    pub fn parse(name: &str) -> Option<MathDisplay> {
        match name {
            "inline" => Some(MathDisplay::Inline),
            "display" => Some(MathDisplay::Display),
            _ => None,
        }
    }
}

/// The horizontal alignment of a line of a `lines` node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum LineAlign {
    /// Flush left (the default).
    #[default]
    Left,
    /// Centered.
    Center,
    /// Flush right.
    Right,
}

impl LineAlign {
    /// The stable name.
    pub fn as_str(self) -> &'static str {
        match self {
            LineAlign::Left => "left",
            LineAlign::Center => "center",
            LineAlign::Right => "right",
        }
    }

    /// The alignment of a stable name, if any.
    pub fn parse(name: &str) -> Option<LineAlign> {
        match name {
            "left" => Some(LineAlign::Left),
            "center" => Some(LineAlign::Center),
            "right" => Some(LineAlign::Right),
            _ => None,
        }
    }
}

/// A document role: a role in the structure of a document, taken from the
/// digital-publishing accessibility roles (`doc-endnotes`, `doc-footnote`,
/// …). It says what a block, a span or a link *is* among the notes, the
/// cited works and the table of contents of a document, beyond what it looks
/// like.
///
/// A payload carries it in its `doc_role` field ([`BlockPayload::doc_role`],
/// [`SpanPayload::doc_role`], [`LinkPayload::doc_role`]). A backend that has
/// such a notion renders it — the HTML backend writes the `role` attribute,
/// `doc-` followed by the role's [stable name](DocRole::as_str), so that
/// assistive technology such as a screen reader can announce what the element
/// is; a backend that has none, the text backend among them, ignores it. It is
/// distinct from the feature-owned role of [`BlockPayload::role`], which says
/// what kind of container a node is and becomes a CSS class on HTML.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DocRole {
    /// A container holding the list of a document's notes, shown apart from
    /// the running text (`doc-endnotes`).
    Endnotes,
    /// A note shown where it is written, or one entry of a list of notes
    /// (`doc-footnote`).
    Footnote,
    /// A link from the running text to a note (`doc-noteref`).
    NoteRef,
    /// A link from a note back to where it is referenced (`doc-backlink`).
    BackLink,
    /// A container holding the list of the works a document cites
    /// (`doc-bibliography`). The entries of the list carry no document role:
    /// the role once defined for one, `doc-biblioentry`, is deprecated by
    /// version 1.1 of the digital-publishing accessibility roles, because a
    /// list item inside the bibliography already says what it is.
    Bibliography,
    /// A link from the running text to a cited work's entry in the
    /// bibliography (`doc-biblioref`).
    BiblioRef,
    /// A container holding a document's table of contents (`doc-toc`).
    Toc,
}

impl DocRole {
    /// Every document role, in declaration order.
    pub const ALL: [DocRole; 7] = [
        DocRole::Endnotes,
        DocRole::Footnote,
        DocRole::NoteRef,
        DocRole::BackLink,
        DocRole::Bibliography,
        DocRole::BiblioRef,
        DocRole::Toc,
    ];

    /// The role's stable name: its text in the projection, and what the HTML
    /// backend writes after `doc-`: `endnotes`, `footnote`, `noteref`,
    /// `backlink`, `bibliography`, `biblioref`, `toc`.
    pub fn as_str(self) -> &'static str {
        match self {
            DocRole::Endnotes => "endnotes",
            DocRole::Footnote => "footnote",
            DocRole::NoteRef => "noteref",
            DocRole::BackLink => "backlink",
            DocRole::Bibliography => "bibliography",
            DocRole::BiblioRef => "biblioref",
            DocRole::Toc => "toc",
        }
    }

    /// The role of a stable name, if any.
    pub fn parse(name: &str) -> Option<DocRole> {
        DocRole::ALL.into_iter().find(|role| role.as_str() == name)
    }
}

impl fmt::Display for DocRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

// --- the payloads -----------------------------------------------------------------------

/// The payload of `flm.pres.par`, a laid-out paragraph: its anchor when it
/// is a link target.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParPayload {
    /// The anchor (see the [module documentation](self#anchors)), when the
    /// paragraph is a link target.
    pub anchor: Option<String>,
}

impl PresPayload for ParPayload {
    fn project(&self) -> SerialValue {
        let mut entries = Vec::new();
        push_opt(&mut entries, "anchor", self.anchor.as_deref().map(text));
        map(entries)
    }
}

impl ParPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.par` when the
    /// projection is not a map, or when `anchor` is present and is not a string.
    pub fn from_projection(value: &SerialValue) -> Result<ParPayload, DeserializeError> {
        let entries = entries(names::PAR, value)?;
        Ok(ParPayload { anchor: opt_string(names::PAR, entries, "anchor")? })
    }
}

/// The payload of `flm.pres.block`, a block container: its role and kinds
/// (what container it is), its anchor when it is a link target, and its
/// document role. See the [module documentation](self#roles-kinds-and-document-roles).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BlockPayload {
    /// The role: what the container is, as a dotted, feature-owned name. By
    /// convention it is the source element's name for the block that holds
    /// the element as a whole, and `<element>.<part>` for a block inside it
    /// that the element's rule builds.
    pub role: Option<Name>,
    /// The kind tokens that classify the role further (`theoremlike`,
    /// `lemma`). A backend or a stylesheet uses them to choose among
    /// renderings of the same role.
    pub kinds: Vec<Name>,
    /// The anchor, when the block is a link target.
    pub anchor: Option<String>,
    /// The block's document role, when it has one: [`DocRole::Endnotes`] for
    /// the container of a list of notes, [`DocRole::Footnote`] for a note
    /// shown where it is written, [`DocRole::Bibliography`] for the container
    /// of the list of cited works, [`DocRole::Toc`] for the container of a
    /// table of contents. A backend without the notion ignores it.
    pub doc_role: Option<DocRole>,
}

impl PresPayload for BlockPayload {
    fn project(&self) -> SerialValue {
        let mut entries = Vec::new();
        push_opt(&mut entries, "role", self.role.as_ref().map(|r| text(r.as_str())));
        if !self.kinds.is_empty() {
            entries.push(("kinds", names(&self.kinds)));
        }
        push_opt(&mut entries, "anchor", self.anchor.as_deref().map(text));
        push_opt(&mut entries, "doc_role", self.doc_role.map(|r| text(r.as_str())));
        map(entries)
    }
}

impl BlockPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.block` when the
    /// projection is not a map, when `role` or `anchor` is present and is not a
    /// string, when `kinds` is present and is not a list of strings, or when
    /// `doc_role` is present and matches no document role ([`DocRole::parse`]).
    pub fn from_projection(value: &SerialValue) -> Result<BlockPayload, DeserializeError> {
        let e = entries(names::BLOCK, value)?;
        Ok(BlockPayload {
            role: name_opt(names::BLOCK, e, "role")?,
            kinds: name_list(names::BLOCK, e, "kinds")?,
            anchor: opt_string(names::BLOCK, e, "anchor")?,
            doc_role: doc_role_opt(names::BLOCK, e)?,
        })
    }
}

/// The payload of `flm.pres.block-heading` and of `flm.pres.runin-heading`:
/// the heading kind's name and level, whether the heading is listed in a
/// table of contents, its anchor, and its number as plain text.
///
/// A heading carries the same facts whether it is placed on its own between
/// blocks or set run in, at the start of the paragraph it introduces, so the
/// two elements share this payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingPayload {
    /// The name of the heading kind, such as `section`
    /// ([`HeadingDecl::name`](crate::stdcore::features::headings::HeadingDecl::name)).
    pub name: Name,
    /// The heading level: a number counted from zero, the smaller the outer,
    /// every value valid ([`Sectioning::level`](crate::model::aspects::Sectioning::level)).
    pub level: u8,
    /// Whether the heading is listed in a table of contents
    /// ([`Sectioning::in_toc`](crate::model::aspects::Sectioning::in_toc)).
    pub in_toc: bool,
    /// The anchor, when the heading is a link target.
    pub anchor: Option<String>,
    /// The heading's number as plain text (`2.1`), where the document numbered
    /// the heading; `None` otherwise. The number also stands in the heading's
    /// content, in a span of role `flm.heading.number`; this field gives it to
    /// a backend that writes it apart from the title (the `latex-rendered`
    /// backend's key `number`). The HTML and text backends do not read it.
    pub number: Option<String>,
}

impl PresPayload for HeadingPayload {
    fn project(&self) -> SerialValue {
        let mut entries = vec![
            ("name", text(self.name.as_str())),
            ("level", SerialValue::Int(i64::from(self.level))),
            ("in_toc", SerialValue::Bool(self.in_toc)),
        ];
        push_opt(&mut entries, "anchor", self.anchor.as_deref().map(text));
        push_opt(&mut entries, "number", self.number.as_deref().map(text));
        map(entries)
    }
}

impl HeadingPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced, for the heading element
    /// `element` — `flm.pres.block-heading` or `flm.pres.runin-heading`, which
    /// share this payload type. `element` is used only to name the element in
    /// a failure.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `element` when the projection is
    /// not a map, when `name` is missing or is not a string, when `level` is
    /// missing, is not an integer or lies outside `0..=255`, when `in_toc` is
    /// missing or is not a boolean, or when `anchor` or `number` is present and
    /// is not a string.
    pub fn from_projection(
        element: &str,
        value: &SerialValue,
    ) -> Result<HeadingPayload, DeserializeError> {
        let e = entries(element, value)?;
        let name = Name::from(string(element, e, "name")?.as_str());
        let level = int(element, e, "level")?;
        let level = small(element, "level", level, i64::from(u8::MAX))? as u8;
        let in_toc = match field(e, "in_toc") {
            Some(SerialValue::Bool(value)) => *value,
            None | Some(SerialValue::Null) => {
                return Err(bad(element, "`in_toc` is missing"));
            }
            Some(other) => {
                return Err(bad(element, format!("`in_toc` must be a boolean, found {other:?}")));
            }
        };
        Ok(HeadingPayload {
            name,
            level,
            in_toc,
            anchor: opt_string(element, e, "anchor")?,
            number: opt_string(element, e, "number")?,
        })
    }
}

/// The payload of `flm.pres.span`, inline content: its display style, its
/// role and kinds (what it is), its anchor when it is a link target, and its
/// document role.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SpanPayload {
    /// The display style, if the span designates one.
    pub style: Option<SpanStyle>,
    /// The role (see [`BlockPayload::role`]).
    pub role: Option<Name>,
    /// The kind tokens (see [`BlockPayload::kinds`]).
    pub kinds: Vec<Name>,
    /// The anchor, when the span is a link target.
    pub anchor: Option<String>,
    /// The span's document role, when it has one: [`DocRole::Footnote`] for
    /// a note shown where it is written. A backend without the notion ignores
    /// it.
    pub doc_role: Option<DocRole>,
}

impl PresPayload for SpanPayload {
    fn project(&self) -> SerialValue {
        let mut entries = Vec::new();
        push_opt(&mut entries, "style", self.style.map(|s| text(s.as_str())));
        push_opt(&mut entries, "role", self.role.as_ref().map(|r| text(r.as_str())));
        if !self.kinds.is_empty() {
            entries.push(("kinds", names(&self.kinds)));
        }
        push_opt(&mut entries, "anchor", self.anchor.as_deref().map(text));
        push_opt(&mut entries, "doc_role", self.doc_role.map(|r| text(r.as_str())));
        map(entries)
    }
}

impl SpanPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.span` when the
    /// projection is not a map, when `style` is present and matches no style
    /// ([`SpanStyle::parse`]), when `role` or `anchor` is present and is not a
    /// string, when `kinds` is present and is not a list of strings, or when
    /// `doc_role` is present and matches no document role ([`DocRole::parse`]).
    pub fn from_projection(value: &SerialValue) -> Result<SpanPayload, DeserializeError> {
        let e = entries(names::SPAN, value)?;
        let style = match opt_string(names::SPAN, e, "style")? {
            None => None,
            Some(name) => Some(
                SpanStyle::parse(&name)
                    .ok_or_else(|| bad(names::SPAN, format!("unknown style `{name}`")))?,
            ),
        };
        Ok(SpanPayload {
            style,
            role: name_opt(names::SPAN, e, "role")?,
            kinds: name_list(names::SPAN, e, "kinds")?,
            anchor: opt_string(names::SPAN, e, "anchor")?,
            doc_role: doc_role_opt(names::SPAN, e)?,
        })
    }
}

/// The payload of `flm.pres.list`: the list's kind tokens, its anchor, and
/// the anchor of each of its entries.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListPayload {
    /// The kind tokens: first at most one structural token,
    /// [`LIST_NUMBERED`](super::LIST_NUMBERED),
    /// [`LIST_UNNUMBERED`](super::LIST_UNNUMBERED) or
    /// [`LIST_TERMS`](super::LIST_TERMS) (by which an HTML emitter chooses
    /// `<ol>`, `<ul>` or `<dl>`), then any others, such as the name of the
    /// environment that produced the list and
    /// [`LIST_LABELED`](super::LIST_LABELED).
    pub kinds: Vec<Name>,
    /// The anchor, when the list as a whole is a link target.
    pub anchor: Option<String>,
    /// The anchor of each entry, in the order of the entries
    /// ([`list_entries`](super::list_entries)): the identifier a link to that
    /// entry points at, `None` for an entry that is not a link target. An
    /// entry past the end of this list has no anchor, and anchors beyond the
    /// last entry are ignored.
    pub entry_anchors: Vec<Option<String>>,
}

impl PresPayload for ListPayload {
    fn project(&self) -> SerialValue {
        let mut entries = Vec::new();
        if !self.kinds.is_empty() {
            entries.push(("kinds", names(&self.kinds)));
        }
        push_opt(&mut entries, "anchor", self.anchor.as_deref().map(text));
        if !self.entry_anchors.is_empty() {
            let anchors = self
                .entry_anchors
                .iter()
                .map(|anchor| anchor.as_deref().map_or(SerialValue::Null, text))
                .collect();
            entries.push(("entry_anchors", SerialValue::List(anchors)));
        }
        map(entries)
    }
}

impl ListPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.list` when the
    /// projection is not a map, when `kinds` is present and is not a list of
    /// strings, when `anchor` is present and is not a string, or when
    /// `entry_anchors` is present and is not a list of strings and nulls.
    pub fn from_projection(value: &SerialValue) -> Result<ListPayload, DeserializeError> {
        let e = entries(names::LIST, value)?;
        let entry_anchors = match field(e, "entry_anchors") {
            None | Some(SerialValue::Null) => Vec::new(),
            Some(SerialValue::List(items)) => items
                .iter()
                .map(|item| match item {
                    SerialValue::Null => Ok(None),
                    SerialValue::Str(s) => Ok(Some(s.clone())),
                    other => Err(bad(
                        names::LIST,
                        format!("`entry_anchors` holds a non-string {other:?}"),
                    )),
                })
                .collect::<Result<Vec<_>, _>>()?,
            Some(other) => {
                return Err(bad(names::LIST, format!("`entry_anchors` must be a list, found {other:?}")));
            }
        };
        Ok(ListPayload {
            kinds: name_list(names::LIST, e, "kinds")?,
            anchor: opt_string(names::LIST, e, "anchor")?,
            entry_anchors,
        })
    }
}

// --- a table of cells ---------------------------------------------------------------------

/// The largest number of grid positions — rows times columns — a
/// [`CellsPayload`] may describe: one million.
///
/// A table's size is written by its author (`\merge{1-1000000}` spans a
/// million columns), and every emitter visits every position of the grid
/// ([`CellsPayload::grid`]), so the size is bounded wherever a table is read:
/// the `cells` feature refuses a cell that would make its table larger, and
/// [`CellsPayload::from_projection`] refuses a projection that describes a
/// larger one.
pub const MAX_CELLS_POSITIONS: u64 = 1_000_000;

/// The horizontal alignment of one cell of a table of cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellAlign {
    /// Flush left.
    Left,
    /// Centered.
    Center,
    /// Flush right.
    Right,
}

impl CellAlign {
    /// The stable name: `left`, `center` or `right`.
    pub fn as_str(self) -> &'static str {
        match self {
            CellAlign::Left => "left",
            CellAlign::Center => "center",
            CellAlign::Right => "right",
        }
    }

    /// The alignment of a stable name, if any.
    pub fn parse(name: &str) -> Option<CellAlign> {
        match name {
            "left" => Some(CellAlign::Left),
            "center" => Some(CellAlign::Center),
            "right" => Some(CellAlign::Right),
            _ => None,
        }
    }
}

/// What kind of header a cell of a table of cells is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellHeader {
    /// The cell heads its column (the HTML `<th scope="col">`).
    Column,
    /// The cell heads its row (the HTML `<th scope="row">`).
    Row,
}

impl CellHeader {
    /// The stable name: `column` or `row`.
    pub fn as_str(self) -> &'static str {
        match self {
            CellHeader::Column => "column",
            CellHeader::Row => "row",
        }
    }

    /// The header kind of a stable name, if any.
    pub fn parse(name: &str) -> Option<CellHeader> {
        match name {
            "column" => Some(CellHeader::Column),
            "row" => Some(CellHeader::Row),
            _ => None,
        }
    }
}

/// One edge of a cell of a table of cells, at which a rule is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CellEdge {
    /// The top edge.
    Above,
    /// The bottom edge.
    Below,
    /// The left edge.
    Left,
    /// The right edge.
    Right,
}

impl CellEdge {
    /// The stable name: `above`, `below`, `left` or `right`.
    pub fn as_str(self) -> &'static str {
        match self {
            CellEdge::Above => "above",
            CellEdge::Below => "below",
            CellEdge::Left => "left",
            CellEdge::Right => "right",
        }
    }

    /// The edge of a stable name, if any.
    pub fn parse(name: &str) -> Option<CellEdge> {
        match name {
            "above" => Some(CellEdge::Above),
            "below" => Some(CellEdge::Below),
            "left" => Some(CellEdge::Left),
            "right" => Some(CellEdge::Right),
            _ => None,
        }
    }
}

/// One cell of a [`CellsPayload`]: its position in the grid, how many rows
/// and columns it spans, and the facts its style tokens state about it.
///
/// Rows and columns are counted from zero here (the syntax counts from one).
/// A cell occupies the rectangle of grid positions from (`row`, `col`) to
/// (`row + row_span - 1`, `col + col_span - 1`); its **top-left position** is
/// (`row`, `col`). Every fact is backend-neutral: a backend renders what it can
/// express and ignores the rest (the plain-text backend shows the alignment and
/// a column header only).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CellSpec {
    /// The row of the cell's top-left position, from zero.
    pub row: u32,
    /// The column of the cell's top-left position, from zero.
    pub col: u32,
    /// How many rows the cell spans; at least one.
    pub row_span: u32,
    /// How many columns the cell spans; at least one.
    pub col_span: u32,
    /// The cell's alignment, when its style tokens give one.
    pub align: Option<CellAlign>,
    /// Whether the cell is a header, and of what.
    pub header: Option<CellHeader>,
    /// The edges at which a rule is drawn, each once, in the order the style
    /// tokens list them.
    pub rules: Vec<CellEdge>,
    /// The cell styles the cell carries, by name, in the order the style
    /// tokens list them. A backend renders each by a convention of its own (an
    /// attribute on HTML, a table command on the recomposed LaTeX), and the
    /// host's definitions for that backend give the name its meaning.
    pub styles: Vec<Name>,
}

impl CellSpec {
    /// A cell spanning one row and one column at (`row`, `col`), with no
    /// style fact.
    pub fn at(row: u32, col: u32) -> CellSpec {
        CellSpec {
            row,
            col,
            row_span: 1,
            col_span: 1,
            align: None,
            header: None,
            rules: Vec::new(),
            styles: Vec::new(),
        }
    }

    /// The row one past the cell's last.
    pub fn row_end(&self) -> u64 {
        u64::from(self.row) + u64::from(self.row_span)
    }

    /// The column one past the cell's last.
    pub fn col_end(&self) -> u64 {
        u64::from(self.col) + u64::from(self.col_span)
    }

    fn project(&self) -> SerialValue {
        let mut e: Vec<(&'static str, SerialValue)> = vec![
            ("row", SerialValue::Int(i64::from(self.row))),
            ("col", SerialValue::Int(i64::from(self.col))),
        ];
        if self.row_span != 1 {
            e.push(("row_span", SerialValue::Int(i64::from(self.row_span))));
        }
        if self.col_span != 1 {
            e.push(("col_span", SerialValue::Int(i64::from(self.col_span))));
        }
        push_opt(&mut e, "align", self.align.map(|align| text(align.as_str())));
        push_opt(&mut e, "header", self.header.map(|header| text(header.as_str())));
        if !self.rules.is_empty() {
            let rules = self.rules.iter().map(|edge| text(edge.as_str())).collect();
            e.push(("rules", SerialValue::List(rules)));
        }
        if !self.styles.is_empty() {
            e.push(("styles", names(&self.styles)));
        }
        map(e)
    }

    fn from_projection(value: &SerialValue) -> Result<CellSpec, DeserializeError> {
        const E: &str = names::CELLS;
        let e = entries(E, value)?;
        let count = |key: &str, value: i64, min: i64| -> Result<u32, DeserializeError> {
            if (min..=i64::from(u32::MAX)).contains(&value) {
                Ok(value as u32)
            } else {
                Err(bad(E, format!("a cell's `{key}` = {value} is outside {min}..={}", u32::MAX)))
            }
        };
        let align = match opt_string(E, e, "align")? {
            None => None,
            Some(name) => Some(
                CellAlign::parse(&name)
                    .ok_or_else(|| bad(E, format!("unknown cell alignment `{name}`")))?,
            ),
        };
        let header = match opt_string(E, e, "header")? {
            None => None,
            Some(name) => Some(
                CellHeader::parse(&name)
                    .ok_or_else(|| bad(E, format!("unknown cell header kind `{name}`")))?,
            ),
        };
        let rules = name_list(E, e, "rules")?
            .iter()
            .map(|name| {
                CellEdge::parse(name.as_str())
                    .ok_or_else(|| bad(E, format!("unknown cell edge `{name}`")))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(CellSpec {
            row: count("row", int(E, e, "row")?, 0)?,
            col: count("col", int(E, e, "col")?, 0)?,
            row_span: count("row_span", opt_int(E, e, "row_span")?.unwrap_or(1), 1)?,
            col_span: count("col_span", opt_int(E, e, "col_span")?.unwrap_or(1), 1)?,
            align,
            header,
            rules,
            styles: name_list(E, e, "styles")?,
        })
    }
}

/// What occupies one position of a table's grid ([`CellsPayload::grid`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GridPosition {
    /// No cell covers the position.
    Empty,
    /// The top-left position of the cell at this index of
    /// [`CellsPayload::cells`]: the position at which an emitter writes the
    /// cell.
    TopLeft(usize),
    /// A position the cell at this index covers by its span, other than its
    /// top-left one: a position at which an emitter writes nothing.
    Covered(usize),
}

/// The payload of `flm.pres.cells`, a whole table of cells: its size and one
/// entry per cell. The cells' content is held in the node's content slots.
///
/// The node carries one content slot named `cell`
/// ([`CELL_SLOT`](super::CELL_SLOT)) per entry of [`cells`](Self::cells), in the
/// same order, so the slot at index *i* holds the content of entry *i*, a run
/// of blocks ([`cells_of`](super::cells_of) reads the two together). A grid
/// position no cell covers has no entry and no slot. The node has no anchor and
/// no role: a table of cells becomes a link target only through the node that
/// holds it, such as a float of the `table` float type.
///
/// An emitter visits the grid through [`grid`](Self::grid) and never
/// computes the occupancy from the entries itself.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CellsPayload {
    /// The number of rows of the grid.
    pub rows: u32,
    /// The number of columns of the grid.
    pub cols: u32,
    /// The cells, in row-major order of their top-left positions: by row, and
    /// within a row by column.
    pub cells: Vec<CellSpec>,
}

impl PresPayload for CellsPayload {
    fn project(&self) -> SerialValue {
        map(vec![
            ("rows", SerialValue::Int(i64::from(self.rows))),
            ("cols", SerialValue::Int(i64::from(self.cols))),
            ("cells", SerialValue::List(self.cells.iter().map(CellSpec::project).collect())),
        ])
    }
}

impl CellsPayload {
    /// What occupies every position of the grid: `grid()[row][col]`, the
    /// rows from zero, each row holding [`cols`](Self::cols) positions.
    ///
    /// Every emitter of a table of cells uses this, and none computes the
    /// occupancy itself. It is built from the entries in their order: an entry
    /// marks its top-left position and covers the rest of its rectangle. A
    /// payload the `cells` feature stages has no two entries overlapping and
    /// none outside the grid, and a payload read back from a projection has
    /// none either ([`from_projection`](Self::from_projection) refuses any
    /// other). For a payload built by hand otherwise, a position already held
    /// keeps its first holder and a position outside the grid is ignored.
    ///
    /// The result holds [`rows`](Self::rows) times [`cols`](Self::cols)
    /// positions, at most [`MAX_CELLS_POSITIONS`] for a payload the feature or
    /// the projection reader produced.
    pub fn grid(&self) -> Vec<Vec<GridPosition>> {
        let rows = self.rows as usize;
        let cols = self.cols as usize;
        let mut grid = vec![vec![GridPosition::Empty; cols]; rows];
        for (index, cell) in self.cells.iter().enumerate() {
            let row_end = cell.row_end().min(rows as u64) as usize;
            let col_end = cell.col_end().min(cols as u64) as usize;
            for (r, row) in grid.iter_mut().enumerate().take(row_end).skip(cell.row as usize) {
                for (c, position) in row.iter_mut().enumerate().take(col_end).skip(cell.col as usize) {
                    if *position != GridPosition::Empty {
                        continue;
                    }
                    *position = if r == cell.row as usize && c == cell.col as usize {
                        GridPosition::TopLeft(index)
                    } else {
                        GridPosition::Covered(index)
                    };
                }
            }
        }
        grid
    }

    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.cells` when the
    /// projection is not a map; when `rows`, `cols` or `cells` is missing or of
    /// the wrong type; when an entry is not a map, lacks `row` or `col`, gives
    /// a span below one or a number outside the unsigned 32-bit range, or gives
    /// an alignment, a header kind or an edge that does not exist; and when the
    /// payload is not one the `cells` feature could stage: a grid of more than
    /// [`MAX_CELLS_POSITIONS`] positions, an entry reaching outside the grid,
    /// two entries overlapping, or the entries out of row-major order.
    pub fn from_projection(value: &SerialValue) -> Result<CellsPayload, DeserializeError> {
        const E: &str = names::CELLS;
        let e = entries(E, value)?;
        let size = |key: &str| -> Result<u32, DeserializeError> {
            let value = int(E, e, key)?;
            u32::try_from(value).map_err(|_| bad(E, format!("`{key}` = {value} is out of range")))
        };
        let rows = size("rows")?;
        let cols = size("cols")?;
        if u64::from(rows) * u64::from(cols) > MAX_CELLS_POSITIONS {
            return Err(bad(
                E,
                format!("a grid of {rows} by {cols} exceeds {MAX_CELLS_POSITIONS} positions"),
            ));
        }
        let cells = match field(e, "cells") {
            Some(SerialValue::List(items)) => {
                items.iter().map(CellSpec::from_projection).collect::<Result<Vec<_>, _>>()?
            }
            Some(other) => return Err(bad(E, format!("`cells` must be a list, found {other:?}"))),
            None => return Err(bad(E, "`cells` is missing")),
        };
        let payload = CellsPayload { rows, cols, cells };
        payload.check().map_err(|detail| bad(E, detail))?;
        Ok(payload)
    }

    /// The consistency [`from_projection`](Self::from_projection) requires:
    /// every entry inside the grid, in row-major order, none overlapping.
    fn check(&self) -> Result<(), String> {
        for (index, cell) in self.cells.iter().enumerate() {
            if cell.row_end() > u64::from(self.rows) || cell.col_end() > u64::from(self.cols) {
                return Err(format!("the cell at index {index} reaches outside the grid"));
            }
        }
        if self.cells.windows(2).any(|pair| (pair[0].row, pair[0].col) >= (pair[1].row, pair[1].col)) {
            return Err("the cells are not in row-major order of their top-left positions".into());
        }
        let held = self.grid();
        for (index, cell) in self.cells.iter().enumerate() {
            for (r, row) in held.iter().enumerate().take(cell.row_end() as usize).skip(cell.row as usize) {
                for position in row.iter().take(cell.col_end() as usize).skip(cell.col as usize) {
                    let holder = match *position {
                        GridPosition::TopLeft(i) | GridPosition::Covered(i) => i,
                        GridPosition::Empty => index,
                    };
                    if holder != index {
                        return Err(format!(
                            "the cell at index {index} overlaps the cell at index {holder} in row {r}"
                        ));
                    }
                }
            }
        }
        Ok(())
    }
}

/// The payload of `flm.pres.dropped`, the mark a lowering rule stages where it
/// left out something it could not place: the condition of the diagnostic the
/// rule reported, and a short statement of it.
///
/// The node is always childless. What was left out does not reach the output;
/// the mark shows a reader of the output that something is missing, and
/// where, while the run's diagnostics say why. Each backend writes it as
/// prominently as it writes [`flm.pres.unsupported`](super::UNSUPPORTED).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DroppedPayload {
    /// The condition of the diagnostic reported for what was dropped, such as
    /// `flm.cells.overlap`.
    pub condition: Name,
    /// A short statement of what was dropped, in the diagnostic's plain words,
    /// such as `cell overlapping another`.
    pub detail: String,
}

impl PresPayload for DroppedPayload {
    fn project(&self) -> SerialValue {
        map(vec![("condition", text(self.condition.as_str())), ("detail", text(&self.detail))])
    }
}

impl DroppedPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.dropped` when the
    /// projection is not a map, or when `condition` or `detail` is missing or
    /// is not a string.
    pub fn from_projection(value: &SerialValue) -> Result<DroppedPayload, DeserializeError> {
        let e = entries(names::DROPPED, value)?;
        Ok(DroppedPayload {
            condition: Name::from(string(names::DROPPED, e, "condition")?),
            detail: string(names::DROPPED, e, "detail")?,
        })
    }
}

/// The payload of `flm.pres.lines`, a sequence of lines: it holds no data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinesPayload;

impl PresPayload for LinesPayload {
    fn project(&self) -> SerialValue {
        map(Vec::new())
    }
}

impl LinesPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.lines` when the
    /// projection is not a map.
    pub fn from_projection(value: &SerialValue) -> Result<LinesPayload, DeserializeError> {
        entries(names::LINES, value)?;
        Ok(LinesPayload)
    }
}

/// The payload of `flm.pres.line`, one line of a `flm.pres.lines` node: the
/// line's indentation from the left and from the right, each a [`Length`],
/// and its alignment.
///
/// An indentation is a length, never a bare number: its unit is part of the
/// value, so that each backend applies it the same way, a
/// [tab](super::LengthUnit::Tabs) at the width that backend sets for it, any
/// other unit at its own size (see [`Length`]). Both default to the zero
/// length.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LinePayload {
    /// Indentation from the left.
    pub indent_left: Length,
    /// Indentation from the right.
    pub indent_right: Length,
    /// The alignment.
    pub align: LineAlign,
}

impl PresPayload for LinePayload {
    fn project(&self) -> SerialValue {
        map(vec![
            ("indent_left", text(self.indent_left.to_string())),
            ("indent_right", text(self.indent_right.to_string())),
            ("align", text(self.align.as_str())),
        ])
    }
}

impl LinePayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.line` when the
    /// projection is not a map, when `indent_left` or `indent_right` is
    /// present and is not the text of a length ([`Length::parse`]), or when
    /// `align` is present and matches no alignment ([`LineAlign::parse`]).
    pub fn from_projection(value: &SerialValue) -> Result<LinePayload, DeserializeError> {
        let e = entries(names::LINE, value)?;
        let length = |key: &str| -> Result<Length, DeserializeError> {
            match opt_string(names::LINE, e, key)? {
                None => Ok(Length::default()),
                Some(written) => Length::parse(&written)
                    .map_err(|error| bad(names::LINE, format!("`{key}`: {error}"))),
            }
        };
        let align = match opt_string(names::LINE, e, "align")? {
            None => LineAlign::Left,
            Some(name) => LineAlign::parse(&name)
                .ok_or_else(|| bad(names::LINE, format!("unknown alignment `{name}`")))?,
        };
        Ok(LinePayload {
            indent_left: length("indent_left")?,
            indent_right: length("indent_right")?,
            align,
        })
    }
}

/// The payload of `flm.pres.verbatim`: the verbatim kind, whether the content
/// is a block, and the language the content is written in when one was given.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerbatimPayload {
    /// The verbatim kind.
    pub kind: VerbatimKind,
    /// Block or inline.
    pub display: VerbatimDisplay,
    /// A language tag, when one was given (`python`).
    pub lang: Option<String>,
}

impl PresPayload for VerbatimPayload {
    fn project(&self) -> SerialValue {
        let mut entries =
            vec![("kind", text(self.kind.as_str())), ("display", text(self.display.as_str()))];
        push_opt(&mut entries, "lang", self.lang.as_deref().map(text));
        map(entries)
    }
}

impl VerbatimPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.verbatim` when the
    /// projection is not a map, when `kind` or `display` is missing or matches
    /// no kind ([`VerbatimKind::parse`]) or display ([`VerbatimDisplay::parse`]),
    /// or when `lang` is present and is not a string.
    pub fn from_projection(value: &SerialValue) -> Result<VerbatimPayload, DeserializeError> {
        let e = entries(names::VERBATIM, value)?;
        let kind = string(names::VERBATIM, e, "kind")?;
        let kind = VerbatimKind::parse(&kind)
            .ok_or_else(|| bad(names::VERBATIM, format!("unknown kind `{kind}`")))?;
        let display = string(names::VERBATIM, e, "display")?;
        let display = VerbatimDisplay::parse(&display)
            .ok_or_else(|| bad(names::VERBATIM, format!("unknown display `{display}`")))?;
        Ok(VerbatimPayload { kind, display, lang: opt_string(names::VERBATIM, e, "lang")? })
    }
}

/// The payload of `flm.pres.math-passthrough`, mathematics given to the backend as
/// source text: the display form, and the environment name when the
/// mathematics is written as an environment.
///
/// The mathematics itself is not in the payload: it is the node's content,
/// copied as source text, which a backend gives to a formula renderer or
/// writes out again. The only part of that content that is not source text
/// is a [`flm.pres.math-text`](names::MATH_TEXT) node ([`MathTextPayload`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathPassthroughPayload {
    /// Inline or display.
    pub display: MathDisplay,
    /// The math environment's name (`align`), when the mathematics is written
    /// as an environment.
    pub env: Option<String>,
}

impl PresPayload for MathPassthroughPayload {
    fn project(&self) -> SerialValue {
        let mut entries = vec![("display", text(self.display.as_str()))];
        push_opt(&mut entries, "env", self.env.as_deref().map(text));
        map(entries)
    }
}

impl MathPassthroughPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.math-passthrough` when the
    /// projection is not a map, when `display` is missing or matches no display
    /// form ([`MathDisplay::parse`]), or when `env` is present and is not a
    /// string.
    pub fn from_projection(value: &SerialValue) -> Result<MathPassthroughPayload, DeserializeError> {
        let e = entries(names::MATH_PASSTHROUGH, value)?;
        let display = string(names::MATH_PASSTHROUGH, e, "display")?;
        let display = MathDisplay::parse(&display)
            .ok_or_else(|| bad(names::MATH_PASSTHROUGH, format!("unknown display `{display}`")))?;
        Ok(MathPassthroughPayload { display, env: opt_string(names::MATH_PASSTHROUGH, e, "env")? })
    }
}

/// The payload of `flm.pres.math-text`: the text command the content was
/// written in, and the anchor when the node is a link target.
///
/// The node's body is the content itself (ordinary inline content, lowered
/// like content anywhere else), and the node is inside a
/// [`flm.pres.math-passthrough`](names::MATH_PASSTHROUGH) node, among that node's source
/// text, at the content's position in the formula.
///
/// A node with no content at all and an anchor is a link target placed at a
/// point of the formula and showing nothing: such a node marks the start of
/// a numbered equation line.
///
/// A node marked [`at_tag`](MathTextPayload::at_tag) is at the position of a
/// line's tag, inside the argument of the `\tag*` the tag is written as. The
/// mark is needed because the HTML and text backends write such a position
/// differently: the HTML backend wraps the content in a pair of dollar signs,
/// and the text backend writes the content unchanged, as an author would have
/// typed it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MathTextPayload {
    /// The text command the content was written in, without the leading
    /// backslash (`text`, `textbf`, `mbox`); `None` when no command was
    /// written, which is the case for a node that only carries an anchor.
    pub command: Option<String>,
    /// The anchor, when the node is a link target.
    pub anchor: Option<String>,
    /// Whether this content is at the position of an equation line's tag: the
    /// argument of the `\tag*` that writes a line's tag, whether the tag is
    /// the one its author wrote or the number the document gave the line.
    ///
    /// The HTML backend writes a pair of dollar signs around the content of
    /// such a node and nothing around any other; every other backend writes
    /// the content the same way wherever it is.
    pub at_tag: bool,
}

impl PresPayload for MathTextPayload {
    fn project(&self) -> SerialValue {
        let mut entries = Vec::new();
        push_opt(&mut entries, "command", self.command.as_deref().map(text));
        push_opt(&mut entries, "anchor", self.anchor.as_deref().map(text));
        if self.at_tag {
            entries.push(("at_tag", SerialValue::Bool(true)));
        }
        map(entries)
    }
}

impl MathTextPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.math-text` when the
    /// projection is not a map, when `command` or `anchor` is present and is
    /// not a string, or when `at_tag` is present and is not a boolean.
    pub fn from_projection(value: &SerialValue) -> Result<MathTextPayload, DeserializeError> {
        let e = entries(names::MATH_TEXT, value)?;
        let at_tag = match field(e, "at_tag") {
            None | Some(SerialValue::Null) => false,
            Some(SerialValue::Bool(value)) => *value,
            Some(_) => return Err(bad(names::MATH_TEXT, "`at_tag` is not a boolean")),
        };
        Ok(MathTextPayload {
            command: opt_string(names::MATH_TEXT, e, "command")?,
            anchor: opt_string(names::MATH_TEXT, e, "anchor")?,
            at_tag,
        })
    }
}

/// The payload of `flm.pres.link`: its target and kind, its role in the
/// document's structure, and its accessible name.
///
/// The link's content (the children of the node) is always what a reader
/// sees. [`label`](LinkPayload::label) is an alternative a reader does not
/// see: the name assistive technology speaks in place of the content, for a
/// link whose content alone says little (a note's number, an arrow back to
/// where the note was referenced).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkPayload {
    /// The target: `#` followed by the anchor for a target in the document, a
    /// URL otherwise.
    pub href: String,
    /// The link kind (`ref`, `cite`, `url`, `term`), which cannot be derived
    /// from the target.
    pub kind: Name,
    /// The link's document role, when it has one: [`DocRole::NoteRef`] for a
    /// link to a note, [`DocRole::BackLink`] for a link back from one,
    /// [`DocRole::BiblioRef`] for a link to a cited work. A backend without
    /// the notion ignores it.
    pub doc_role: Option<DocRole>,
    /// The link's accessible name (`Footnote 1`), spoken in place of its
    /// content by assistive technology, when it has one; the HTML backend
    /// writes it as `aria-label`. A backend without the notion ignores it,
    /// and the content stays what a reader sees either way.
    pub label: Option<String>,
    /// Whether the content is the target's own display: text derived from the
    /// target by the feature that staged the link, rather than words an
    /// author wrote for it. `\url{https://example.com/}` shows `example.com`,
    /// derived from its address, whereas
    /// `\href{https://example.com}{example.com}` shows the words its author
    /// wrote, which happen to match.
    ///
    /// The feature that staged the link states this fact; it is never the
    /// result of comparing the content with the target. Any backend may read it: the text backend
    /// prints the target after a link's content, and not after a link whose
    /// content already is the target's display unless asked to
    /// ([`TextRenderOptions`](crate::stdcore::backends::text::TextRenderOptions)).
    /// `false` for every link whose content was written by an author or is a
    /// reference's text, which is every link but a `\url` or an `\email`.
    pub shows_target: bool,
}

impl PresPayload for LinkPayload {
    fn project(&self) -> SerialValue {
        let mut entries = vec![("href", text(&self.href)), ("kind", text(self.kind.as_str()))];
        push_opt(&mut entries, "doc_role", self.doc_role.map(|r| text(r.as_str())));
        push_opt(&mut entries, "label", self.label.as_deref().map(text));
        if self.shows_target {
            entries.push(("shows_target", SerialValue::Bool(true)));
        }
        map(entries)
    }
}

impl LinkPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.link` when the
    /// projection is not a map, when `href` or `kind` is missing or is not a
    /// string, when `doc_role` is present and matches no document role
    /// ([`DocRole::parse`]), when `label` is present and is not a string, or
    /// when `shows_target` is present and is not a boolean. An absent
    /// `shows_target` is `false`, the value the projection leaves out.
    pub fn from_projection(value: &SerialValue) -> Result<LinkPayload, DeserializeError> {
        let e = entries(names::LINK, value)?;
        let shows_target = match field(e, "shows_target") {
            None | Some(SerialValue::Null) => false,
            Some(SerialValue::Bool(value)) => *value,
            Some(_) => return Err(bad(names::LINK, "`shows_target` is not a boolean")),
        };
        Ok(LinkPayload {
            href: string(names::LINK, e, "href")?,
            kind: Name::from(string(names::LINK, e, "kind")?),
            doc_role: doc_role_opt(names::LINK, e)?,
            label: opt_string(names::LINK, e, "label")?,
            shows_target,
        })
    }
}

/// One candidate of an image's `srcset`: a source and its descriptor
/// (`2x`, `640w`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SrcsetCandidate {
    /// The image source.
    pub src: String,
    /// The candidate's descriptor.
    pub descriptor: String,
}

/// The payload of `flm.pres.image`: where the image is, and how it is to be
/// presented. The node has no content.
///
/// The display says whether the image is a block of its own (as the content
/// of a float) or inline in the text around it ([`ImageDisplay`]).
///
/// The two dimensions are the size the image is shown at, each a [`Length`]
/// kept in the unit it was given in; each backend writes them its own way (the
/// page as two custom properties its stylesheet reads, LaTeX through its
/// length writer) and none converts them. A dimension left out is the image's
/// natural size in that direction, the image keeping its proportions; where
/// both are set, the image is shown at exactly that size.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ImagePayload {
    /// The image source, as the backend's published graphics table resolved it.
    pub src: String,
    /// Block or inline.
    pub display: ImageDisplay,
    /// The alternative text, when given.
    pub alt: Option<String>,
    /// The width the image is shown at, when set.
    pub width: Option<Length>,
    /// The height the image is shown at, when set.
    pub height: Option<Length>,
    /// Alternative sources.
    pub srcset: Vec<SrcsetCandidate>,
    /// The anchor, when the image is a link target.
    pub anchor: Option<String>,
}

impl PresPayload for ImagePayload {
    fn project(&self) -> SerialValue {
        let mut entries =
            vec![("src", text(&self.src)), ("display", text(self.display.as_str()))];
        push_opt(&mut entries, "alt", self.alt.as_deref().map(text));
        push_opt(&mut entries, "width", self.width.map(|length| text(length.to_string())));
        push_opt(&mut entries, "height", self.height.map(|length| text(length.to_string())));
        if !self.srcset.is_empty() {
            entries.push((
                "srcset",
                SerialValue::List(
                    self.srcset
                        .iter()
                        .map(|c| {
                            map(vec![("src", text(&c.src)), ("descriptor", text(&c.descriptor))])
                        })
                        .collect(),
                ),
            ));
        }
        push_opt(&mut entries, "anchor", self.anchor.as_deref().map(text));
        map(entries)
    }
}

impl ImagePayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.image` when the
    /// projection is not a map, when `src` is missing, when `display` is
    /// missing or matches no display ([`ImageDisplay::parse`]), when `alt` or `anchor`
    /// is present and is not a string, when `width` or `height` is present
    /// and is not the text of a length ([`Length::parse`]), or when `srcset`
    /// is present and is not a list of maps each holding a `src` and a
    /// `descriptor` string.
    pub fn from_projection(value: &SerialValue) -> Result<ImagePayload, DeserializeError> {
        let e = entries(names::IMAGE, value)?;
        let length = |key: &str| -> Result<Option<Length>, DeserializeError> {
            opt_string(names::IMAGE, e, key)?
                .map(|written| {
                    Length::parse(&written)
                        .map_err(|error| bad(names::IMAGE, format!("`{key}`: {error}")))
                })
                .transpose()
        };
        let srcset = match field(e, "srcset") {
            None | Some(SerialValue::Null) => Vec::new(),
            Some(SerialValue::List(items)) => items
                .iter()
                .map(|item| {
                    let c = entries(names::IMAGE, item)?;
                    Ok(SrcsetCandidate {
                        src: string(names::IMAGE, c, "src")?,
                        descriptor: string(names::IMAGE, c, "descriptor")?,
                    })
                })
                .collect::<Result<Vec<_>, DeserializeError>>()?,
            Some(other) => {
                return Err(bad(names::IMAGE, format!("`srcset` must be a list, found {other:?}")));
            }
        };
        let display = string(names::IMAGE, e, "display")?;
        let display = ImageDisplay::parse(&display)
            .ok_or_else(|| bad(names::IMAGE, format!("unknown display `{display}`")))?;
        Ok(ImagePayload {
            src: string(names::IMAGE, e, "src")?,
            display,
            alt: opt_string(names::IMAGE, e, "alt")?,
            width: length("width")?,
            height: length("height")?,
            srcset,
            anchor: opt_string(names::IMAGE, e, "anchor")?,
        })
    }
}

/// The payload of `flm.pres.unsupported`: the element the backend could not
/// render and a message saying why.
///
/// The node is always childless. What could not be rendered is described in
/// the run's diagnostics and does not reach the output; a backend writes a
/// mark in its place (the text backend writes `[unsupported: <element>]`).
/// The lowering driver stages the node for a
/// node of an accepted unrenderable element, for a node whose rule failed,
/// and for a node no rule lowers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnsupportedPayload {
    /// The element that could not be rendered.
    pub element: ElementName,
    /// Why the element could not be rendered, as a short statement.
    pub message: String,
}

impl PresPayload for UnsupportedPayload {
    fn project(&self) -> SerialValue {
        map(vec![("element", text(self.element.as_str())), ("message", text(&self.message))])
    }
}

impl UnsupportedPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.unsupported` when the
    /// projection is not a map, or when `element` or `message` is missing or
    /// is not a string.
    pub fn from_projection(value: &SerialValue) -> Result<UnsupportedPayload, DeserializeError> {
        let e = entries(names::UNSUPPORTED, value)?;
        Ok(UnsupportedPayload {
            element: ElementName::from(string(names::UNSUPPORTED, e, "element")?),
            message: string(names::UNSUPPORTED, e, "message")?,
        })
    }
}

/// The invocation form an [`UnknownPayload`] records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UnknownKind {
    /// A macro: `\foo`.
    Macro,
    /// An environment: `\begin{foo}…\end{foo}`.
    Environment,
}

impl UnknownKind {
    /// The stable name.
    pub fn as_str(self) -> &'static str {
        match self {
            UnknownKind::Macro => "macro",
            UnknownKind::Environment => "environment",
        }
    }

    /// The kind of a stable name, if any.
    pub fn parse(name: &str) -> Option<UnknownKind> {
        match name {
            "macro" => Some(UnknownKind::Macro),
            "environment" => Some(UnknownKind::Environment),
            _ => None,
        }
    }
}

/// The payload of `flm.pres.unknown`, a macro or an environment the parse
/// could not attribute to any feature, carried into the output as the
/// document content it is. An environment's node holds its lowered body as
/// children; a macro's node is childless.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownPayload {
    /// Which invocation form was written.
    pub kind: UnknownKind,
    /// The callable's name as written, without any delimiter (`foo`).
    pub name: String,
    /// The source spelling of the invocation: `\foo` for a macro, the bare name
    /// for an environment (a backend writes its own `\begin`/`\end` around it).
    pub spelling: String,
}

impl PresPayload for UnknownPayload {
    fn project(&self) -> SerialValue {
        map(vec![
            ("kind", text(self.kind.as_str())),
            ("name", text(&self.name)),
            ("spelling", text(&self.spelling)),
        ])
    }
}

impl UnknownPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.unknown` when the
    /// projection is not a map, when `kind` is missing or matches no invocation
    /// form ([`UnknownKind::parse`]), or when `name` or `spelling` is missing or
    /// is not a string.
    pub fn from_projection(value: &SerialValue) -> Result<UnknownPayload, DeserializeError> {
        let e = entries(names::UNKNOWN, value)?;
        let kind = string(names::UNKNOWN, e, "kind")?;
        let kind = UnknownKind::parse(&kind)
            .ok_or_else(|| bad(names::UNKNOWN, format!("unknown kind `{kind}`")))?;
        Ok(UnknownPayload {
            kind,
            name: string(names::UNKNOWN, e, "name")?,
            spelling: string(names::UNKNOWN, e, "spelling")?,
        })
    }
}

/// The payload of the three decoration elements (`flm.pres.deco.runin`,
/// `flm.pres.deco.final`, `flm.pres.deco.margin`): it holds no data, since
/// the element states which decoration it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DecorationPayload;

impl PresPayload for DecorationPayload {
    fn project(&self) -> SerialValue {
        map(Vec::new())
    }
}

impl DecorationPayload {
    /// The payload rebuilt from its projection, for the decoration element
    /// `element` — any of `flm.pres.deco.runin`, `flm.pres.deco.final` and
    /// `flm.pres.deco.margin`, which share this payload type. `element` is used
    /// only to name the element in a failure.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `element` when the projection is
    /// not a map.
    pub fn from_projection(
        element: &str,
        value: &SerialValue,
    ) -> Result<DecorationPayload, DeserializeError> {
        entries(element, value)?;
        Ok(DecorationPayload)
    }
}

/// The payload of `flm.pres.deco-mark`, the decoration mark: the index, in
/// the slot list of the presentation node that holds the mark, of the
/// attached slot holding the decoration the lowering driver moved out of this
/// position ([`decoration_at`](super::decoration_at)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecoMarkPayload {
    /// The attached slot's index.
    pub slot: u32,
}

impl PresPayload for DecoMarkPayload {
    fn project(&self) -> SerialValue {
        map(vec![("slot", SerialValue::Int(i64::from(self.slot)))])
    }
}

impl DecoMarkPayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.deco-mark` when the
    /// projection is not a map, or when `slot` is missing, is not an integer, or
    /// is not a slot index (negative, or beyond `u32`).
    pub fn from_projection(value: &SerialValue) -> Result<DecoMarkPayload, DeserializeError> {
        let e = entries(names::DECO_MARK, value)?;
        let slot = int(names::DECO_MARK, e, "slot")?;
        let slot = u32::try_from(slot)
            .map_err(|_| bad(names::DECO_MARK, format!("`slot` = {slot} is not a slot index")))?;
        Ok(DecoMarkPayload { slot })
    }
}

/// The payload of `flm.pres.subtree`: the document tree placed at this node.
/// An emitter that writes one output document inserts that tree's own
/// lowered form at this node.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubtreePayload {
    /// The tree placed here.
    pub tree: TreeId,
}

impl PresPayload for SubtreePayload {
    fn project(&self) -> SerialValue {
        map(vec![("tree", text(self.tree.to_string()))])
    }
}

impl SubtreePayload {
    /// The payload rebuilt from its projection, the value
    /// [`project`](PresPayload::project) produced.
    ///
    /// # Errors
    ///
    /// Returns a [`DeserializeError`] mentioning `flm.pres.subtree` when the
    /// projection is not a map, when `tree` is missing or is not a string, or
    /// when that string is not a document tree identifier — `tree#3`; a value
    /// identifier such as `value#1` is refused, since a value is never placed.
    pub fn from_projection(value: &SerialValue) -> Result<SubtreePayload, DeserializeError> {
        let e = entries(names::SUBTREE, value)?;
        let spelled = string(names::SUBTREE, e, "tree")?;
        let index = spelled
            .strip_prefix("tree#")
            .and_then(|n| n.parse::<u32>().ok())
            .filter(|n| n & (1 << 31) == 0)
            .ok_or_else(|| bad(names::SUBTREE, format!("`{spelled}` is not a document tree id")))?;
        Ok(SubtreePayload { tree: TreeId::tree(index) })
    }
}

// --- the readers -----------------------------------------------------------------------

/// Registers `parse` as the payload reader of `element`.
fn reader<P: PresPayload>(
    readers: &mut PayloadReaders,
    element: &'static str,
    parse: fn(&SerialValue) -> Result<P, DeserializeError>,
) {
    readers.register(element, move |value: &SerialValue, _cx: &mut DeserializeContext<'_, Flm>| {
        parse(value).map(|payload| Arc::new(payload) as Arc<dyn PresPayload>)
    });
}

/// The payload readers of the whole vocabulary, one per element: what a
/// reading session ([`reader_with`](crate::dump::session::reader_with)) needs
/// to read a stored lowered tree back into typed payloads. Each reader
/// rebuilds a payload from its projection through the type's
/// `from_projection` function.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::pres::{ALL, payload_readers};
///
/// assert_eq!(payload_readers().len(), ALL.len());
/// ```
pub fn payload_readers() -> PayloadReaders {
    let mut readers = PayloadReaders::new();
    reader(&mut readers, names::PAR, ParPayload::from_projection);
    reader(&mut readers, names::BLOCK, BlockPayload::from_projection);
    for element in [names::BLOCK_HEADING, names::RUNIN_HEADING] {
        readers.register(
            element,
            move |value: &SerialValue, _cx: &mut DeserializeContext<'_, Flm>| {
                HeadingPayload::from_projection(element, value)
                    .map(|payload| Arc::new(payload) as Arc<dyn PresPayload>)
            },
        );
    }
    reader(&mut readers, names::SPAN, SpanPayload::from_projection);
    reader(&mut readers, names::LIST, ListPayload::from_projection);
    reader(&mut readers, names::CELLS, CellsPayload::from_projection);
    reader(&mut readers, names::LINES, LinesPayload::from_projection);
    reader(&mut readers, names::LINE, LinePayload::from_projection);
    reader(&mut readers, names::VERBATIM, VerbatimPayload::from_projection);
    reader(&mut readers, names::MATH_PASSTHROUGH, MathPassthroughPayload::from_projection);
    reader(&mut readers, names::MATH_TEXT, MathTextPayload::from_projection);
    reader(&mut readers, names::LINK, LinkPayload::from_projection);
    reader(&mut readers, names::IMAGE, ImagePayload::from_projection);
    reader(&mut readers, names::UNSUPPORTED, UnsupportedPayload::from_projection);
    reader(&mut readers, names::DROPPED, DroppedPayload::from_projection);
    reader(&mut readers, names::UNKNOWN, UnknownPayload::from_projection);
    for element in names::DECORATIONS {
        readers.register(
            element,
            move |value: &SerialValue, _cx: &mut DeserializeContext<'_, Flm>| {
                DecorationPayload::from_projection(element, value)
                    .map(|payload| Arc::new(payload) as Arc<dyn PresPayload>)
            },
        );
    }
    reader(&mut readers, names::DECO_MARK, DecoMarkPayload::from_projection);
    reader(&mut readers, names::SUBTREE, SubtreePayload::from_projection);
    readers
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 2×3 table: a header spanning two columns, a cell spanning two rows,
    /// one empty position.
    fn table() -> CellsPayload {
        CellsPayload {
            rows: 2,
            cols: 3,
            cells: vec![
                CellSpec {
                    col_span: 2,
                    header: Some(CellHeader::Column),
                    align: Some(CellAlign::Center),
                    ..CellSpec::at(0, 0)
                },
                CellSpec {
                    row_span: 2,
                    rules: vec![CellEdge::Left, CellEdge::Below],
                    styles: vec![Name::from("green"), Name::from("red")],
                    ..CellSpec::at(0, 2)
                },
                CellSpec { header: Some(CellHeader::Row), ..CellSpec::at(1, 0) },
            ],
        }
    }

    #[test]
    fn a_table_of_cells_projects_its_entries_and_reads_back() {
        let payload = table();
        let projected = payload.project();
        let cell = |entries: Vec<(&str, SerialValue)>| map(entries);
        assert_eq!(
            projected,
            map(vec![
                ("rows", SerialValue::Int(2)),
                ("cols", SerialValue::Int(3)),
                (
                    "cells",
                    SerialValue::List(vec![
                        cell(vec![
                            ("row", SerialValue::Int(0)),
                            ("col", SerialValue::Int(0)),
                            ("col_span", SerialValue::Int(2)),
                            ("align", text("center")),
                            ("header", text("column")),
                        ]),
                        cell(vec![
                            ("row", SerialValue::Int(0)),
                            ("col", SerialValue::Int(2)),
                            ("row_span", SerialValue::Int(2)),
                            ("rules", SerialValue::List(vec![text("left"), text("below")])),
                            ("styles", SerialValue::List(vec![text("green"), text("red")])),
                        ]),
                        cell(vec![
                            ("row", SerialValue::Int(1)),
                            ("col", SerialValue::Int(0)),
                            ("header", text("row")),
                        ]),
                    ]),
                ),
            ])
        );
        assert_eq!(CellsPayload::from_projection(&projected).unwrap(), payload);
    }

    #[test]
    fn the_grid_answers_every_position() {
        use GridPosition::{Covered, Empty, TopLeft};
        assert_eq!(
            table().grid(),
            vec![vec![TopLeft(0), Covered(0), TopLeft(1)], vec![TopLeft(2), Empty, Covered(1)]]
        );
        assert_eq!(CellsPayload::default().grid(), Vec::<Vec<GridPosition>>::new());
        // A hand-built payload that overlaps or reaches outside: the first
        // holder keeps a position, and what lies outside is ignored.
        let odd = CellsPayload {
            rows: 1,
            cols: 2,
            cells: vec![CellSpec { col_span: 5, ..CellSpec::at(0, 0) }, CellSpec::at(0, 1)],
        };
        assert_eq!(odd.grid(), vec![vec![TopLeft(0), Covered(0)]]);
    }

    #[test]
    fn a_projection_the_feature_could_not_stage_is_refused() {
        let refused = |payload: CellsPayload| {
            CellsPayload::from_projection(&payload.project()).expect_err("refused")
        };
        let mut outside = table();
        outside.cells[1].row_span = 3;
        refused(outside);
        let mut overlapping = table();
        overlapping.cells[2] = CellSpec { col_span: 3, ..CellSpec::at(1, 0) };
        refused(overlapping);
        let mut unordered = table();
        unordered.cells.swap(0, 1);
        refused(unordered);
        refused(CellsPayload { rows: 1001, cols: 1000, cells: Vec::new() });
        let zero_span = map(vec![
            ("rows", SerialValue::Int(1)),
            ("cols", SerialValue::Int(1)),
            (
                "cells",
                SerialValue::List(vec![map(vec![
                    ("row", SerialValue::Int(0)),
                    ("col", SerialValue::Int(0)),
                    ("row_span", SerialValue::Int(0)),
                ])]),
            ),
        ]);
        assert!(CellsPayload::from_projection(&zero_span).is_err());
    }

    #[test]
    fn the_dropped_mark_projects_and_reads_back() {
        let mark = DroppedPayload {
            condition: Name::from("flm.cells.overlap"),
            detail: String::from("cell overlapping another"),
        };
        assert_eq!(
            mark.project(),
            map(vec![
                ("condition", text("flm.cells.overlap")),
                ("detail", text("cell overlapping another")),
            ])
        );
        assert_eq!(DroppedPayload::from_projection(&mark.project()).unwrap(), mark);
    }

    #[test]
    fn a_projection_omits_absent_optionals_and_empty_lists() {
        let block = BlockPayload::default();
        assert_eq!(block.project(), map(Vec::new()));
        let block = BlockPayload {
            role: Some(Name::from("t.thm")),
            kinds: vec![Name::from("lemma")],
            anchor: Some(String::from("lem-1")),
            doc_role: Some(DocRole::Endnotes),
        };
        assert_eq!(
            block.project(),
            map(vec![
                ("role", text("t.thm")),
                ("kinds", SerialValue::List(vec![text("lemma")])),
                ("anchor", text("lem-1")),
                ("doc_role", text("endnotes")),
            ])
        );
    }

    #[test]
    fn every_payload_reads_back_from_its_projection() {
        let readers = payload_readers();
        assert_eq!(readers.len(), names::ALL.len());
        let heading = HeadingPayload {
            name: Name::from("subsection"),
            level: 2,
            in_toc: false,
            anchor: Some(String::from("sec")),
            number: Some(String::from("1.2")),
        };
        for element in [names::BLOCK_HEADING, names::RUNIN_HEADING] {
            assert_eq!(HeadingPayload::from_projection(element, &heading.project()).unwrap(), heading);
        }
        let span = SpanPayload { style: Some(SpanStyle::SmallCaps), ..SpanPayload::default() };
        assert_eq!(SpanPayload::from_projection(&span.project()).unwrap(), span);
        let span = SpanPayload { doc_role: Some(DocRole::Footnote), ..SpanPayload::default() };
        assert_eq!(SpanPayload::from_projection(&span.project()).unwrap(), span);
        let block = BlockPayload { doc_role: Some(DocRole::Endnotes), ..BlockPayload::default() };
        assert_eq!(BlockPayload::from_projection(&block.project()).unwrap(), block);
        // A link's document role, accessible name and `shows_target` project
        // only when set, and read back as they were written.
        let link = LinkPayload {
            href: String::from("#fn-1"),
            kind: Name::from("xnote"),
            doc_role: None,
            label: None,
            shows_target: false,
        };
        assert_eq!(link.project(), map(vec![("href", text("#fn-1")), ("kind", text("xnote"))]));
        assert_eq!(LinkPayload::from_projection(&link.project()).unwrap(), link);
        let link = LinkPayload {
            doc_role: Some(DocRole::NoteRef),
            label: Some(String::from("Footnote \"1\" <a>")),
            ..link
        };
        assert_eq!(LinkPayload::from_projection(&link.project()).unwrap(), link);
        let link = LinkPayload {
            href: String::from("https://example.com/"),
            kind: Name::from("url"),
            doc_role: None,
            label: None,
            shows_target: true,
        };
        assert_eq!(
            link.project(),
            map(vec![
                ("href", text("https://example.com/")),
                ("kind", text("url")),
                ("shows_target", SerialValue::Bool(true)),
            ])
        );
        assert_eq!(LinkPayload::from_projection(&link.project()).unwrap(), link);
        let math = MathPassthroughPayload {
            display: MathDisplay::Display,
            env: Some(String::from("align")),
        };
        assert_eq!(MathPassthroughPayload::from_projection(&math.project()).unwrap(), math);
        let math_text = MathTextPayload {
            command: Some(String::from("textbf")),
            anchor: Some(String::from("eq-1")),
            at_tag: false,
        };
        assert_eq!(MathTextPayload::from_projection(&math_text.project()).unwrap(), math_text);
        assert_eq!(MathTextPayload::default().project(), map(Vec::new()));
        // The mark of a tag's position projects only when it is set, and reads
        // back as it was written.
        let at_tag = MathTextPayload { at_tag: true, ..MathTextPayload::default() };
        assert_eq!(MathTextPayload::from_projection(&at_tag.project()).unwrap(), at_tag);
        let subtree = SubtreePayload { tree: TreeId::tree(3) };
        assert_eq!(SubtreePayload::from_projection(&subtree.project()).unwrap(), subtree);
        let image = ImagePayload {
            src: String::from("a.png"),
            srcset: vec![SrcsetCandidate {
                src: String::from("a@2x.png"),
                descriptor: String::from("2x"),
            }],
            width: Some(Length::parse("5cm").unwrap()),
            height: Some(Length::parse("2.5in").unwrap()),
            display: ImageDisplay::Block,
            ..ImagePayload::default()
        };
        assert_eq!(ImagePayload::from_projection(&image.project()).unwrap(), image);
        let one_dimension =
            ImagePayload { height: Some(Length::parse("10pt").unwrap()), ..image.clone() };
        assert_eq!(
            ImagePayload::from_projection(&one_dimension.project()).unwrap(),
            one_dimension
        );
        let line = LinePayload {
            indent_left: Length::tabs(4),
            indent_right: Length::parse("1.5em").unwrap(),
            align: LineAlign::Right,
        };
        assert_eq!(LinePayload::from_projection(&line.project()).unwrap(), line);
        let unknown = UnknownPayload {
            kind: UnknownKind::Environment,
            name: String::from("nope"),
            spelling: String::from("nope"),
        };
        assert_eq!(UnknownPayload::from_projection(&unknown.project()).unwrap(), unknown);
        let verbatim = VerbatimPayload {
            kind: VerbatimKind::Identifier,
            display: VerbatimDisplay::Block,
            lang: Some(String::from("rust")),
        };
        assert_eq!(VerbatimPayload::from_projection(&verbatim.project()).unwrap(), verbatim);
        // A list's entry anchors keep their order and their gaps.
        let list = ListPayload {
            kinds: vec![Name::from(names::LIST_NUMBERED), Name::from("enumerate")],
            anchor: None,
            entry_anchors: vec![None, Some(String::from("item-a")), None],
        };
        assert_eq!(ListPayload::from_projection(&list.project()).unwrap(), list);
        assert_eq!(ListPayload::default().project(), map(Vec::new()));
    }

    #[test]
    fn every_verbatim_kind_round_trips_through_its_stable_name() {
        for kind in VerbatimKind::ALL {
            assert_eq!(VerbatimKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(VerbatimKind::Identifier.as_str(), "a");
        assert_eq!(VerbatimKind::parse("identifier"), None);
    }

    #[test]
    fn a_malformed_projection_is_a_typed_read_error_naming_the_element() {
        let error = LinkPayload::from_projection(&map(vec![("href", text("x"))])).unwrap_err();
        assert!(error.to_string().contains(names::LINK), "{error}");
        let error =
            SubtreePayload::from_projection(&map(vec![("tree", text("value#1"))])).unwrap_err();
        assert!(error.to_string().contains(names::SUBTREE), "{error}");
    }

    #[test]
    fn document_role_names_round_trip_and_an_unknown_one_is_refused() {
        for role in DocRole::ALL {
            assert_eq!(DocRole::parse(role.as_str()), Some(role));
        }
        assert_eq!(DocRole::NoteRef.as_str(), "noteref");
        let error = SpanPayload::from_projection(&map(vec![("doc_role", text("doc-noteref"))]))
            .unwrap_err();
        assert!(error.to_string().contains(names::SPAN), "{error}");
        let error = LinkPayload::from_projection(&map(vec![
            ("href", text("#a")),
            ("kind", text("ref")),
            ("label", SerialValue::Int(1)),
        ]))
        .unwrap_err();
        assert!(error.to_string().contains(names::LINK), "{error}");
    }

    #[test]
    fn style_names_round_trip() {
        for style in SpanStyle::ALL {
            assert_eq!(SpanStyle::parse(style.as_str()), Some(style));
        }
        assert_eq!(SpanStyle::parse("blink"), None);
    }
}
