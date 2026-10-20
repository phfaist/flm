//! The emitter of the `latex-rendered` backend: one lowered tree written as
//! LaTeX through the contract ([`super::contract`]), one command or
//! environment per presentation element.
//!
//! [`emit_tree`] walks the tree on a worker thread under a depth limit
//! ([`DEPTH_LIMIT`]) and writes through the LaTeX family's typed sink
//! ([`LatexSink`]), document text escaped in the mode `Declare` with no
//! overrides: the ten special characters escaped, every other character
//! written as UTF-8, nothing declared (LuaLaTeX with `fontspec` takes the
//! characters from the font).
//!
//! # What each element writes
//!
//! The element's row gives its form and its keys; the key-value argument is
//! written by one function, so that every element writes it the same way:
//! `[key=value, key={value}, …]`, a name bare, kind tokens braced and
//! separated by a space, content, text and addresses braced, a length spelled
//! for LaTeX ([`super::length::latex_length`]), a number bare, a boolean
//! `true` or `false`. A key whose value is absent, empty or false is left out,
//! except a heading's `toc` and `plain`, which are always written. A name made
//! of characters other than letters, digits and `. - _ : / + @ *` is braced
//! and written as escaped text; an anchor so made, here and in
//! `\flmxTarget{…}`, is braced and spelled as an address is (`~` written
//! `\string~`), so that it stays a valid label name. The construct elements of the LaTeX family
//! are written natively; a definition construct is recorded for the style
//! file ([`StagedDefinition`]) and written nowhere in the flow.
//!
//! # Layout
//!
//! A block-level element (a block, a list, a table of cells, lines, a verbatim
//! block, a heading, an `\flmxItem`, an image shown as a block) begins on a
//! line of its own and its end stands on one; one blank line separates two
//! paragraphs, and a paragraph from a block-level element; inline content
//! flows, and the emitter breaks a line at a space of document text once the
//! line is longer than [`LINE_WIDTH`] bytes, never inside a key-value
//! argument, a verbatim argument or the source of a formula. A run of white
//! space in document text is written as one space, or as one newline where it
//! holds one, so that no blank line, which LaTeX reads as a paragraph break,
//! comes from inside a paragraph. Every tree ends with a newline.
//!
//! # Files
//!
//! This file holds the emitter's state and its writing of bytes and layout;
//! `keys.rs` the key-value argument; `walk.rs` the walk and the paragraphs,
//! blocks and spans; `elements.rs` the other elements; `constructs.rs` the
//! construct elements of the LaTeX family. Mathematics is written in the
//! backend's `math.rs`.

use std::collections::BTreeSet;
use std::sync::Arc;

use techy::core::node::{NodeKind, NodeRef, NodeTree};
use techy::error::{Diagnostic, DiagnosticInfo};

use crate::lang::guard;
use crate::lang::structural;
use crate::lang::{Flm, FlmDiagnostics, FlmGroupType, FlmOrigin, FlmSpan, nowhere};
use crate::model::addr::TreeId;
use crate::model::element::element_of;
use crate::model::names::Name;
use crate::stdcore::backends::latex::construct::{
    self, BEGIN_SLOT, END_SLOT, LatexArgShape, LatexDefinitionKind, LatexDefinitionPayload,
    LatexEnvironmentPayload, LatexMacroPayload, LatexRawPayload, arg_slots, slot_named,
};
use crate::stdcore::backends::latex::{EscapeConfig, LatexRaw, LatexSink, UnicodeMode, escape_url};
use crate::stdcore::backends::pres::{
    self, BlockPayload, DroppedPayload, GridPosition, HeadingPayload, ImageDisplay,
    ImagePayload, Length, LinePayload, LinkPayload, ListPayload, ParPayload, SpanPayload,
    SubtreePayload, UnknownKind, UnknownPayload, UnsupportedPayload, VerbatimDisplay,
    VerbatimPayload, cells_of, decoration_at, ends_in_sentence_punctuation, holds_text,
    is_presentation, list_entries, payload_of, plain_text, recompose_source,
};

use super::contract::{CORE_CHUNK, Form, Row, row};
use super::feature::LatexRenderedRenderOptions;
use super::length::latex_length;
use super::stages::{LatexSegment, StageFailed};
use super::verbatim::escape_verbatim;

mod constructs;
mod elements;
mod keys;
mod walk;

const LEVEL_STACK_ESTIMATE: usize = 12 * 1024;

/// The nesting depth past which the emitter refuses a tree and reports
/// `flm.latex-rendered.too-deep` ([`NestingTooDeep`]): the recursion guard's
/// stack budget divided by the estimated stack use of one level.
pub const DEPTH_LIMIT: usize = guard::STACK_BUDGET / LEVEL_STACK_ESTIMATE;

/// The length in bytes past which a line of flowing text is broken at the
/// next space of document text, 100.
pub const LINE_WIDTH: usize = 100;

/// What a diagnostic shows in place of a position for a condition this backend
/// reports without a position in any document.
pub(crate) const NOWHERE_LABEL: &str = "latex-rendered backend";

/// Condition `flm.latex-rendered.too-deep` (Error): a lowered tree nests
/// deeper than the emitter's depth limit ([`DEPTH_LIMIT`]). Recovery: nothing
/// is written for that tree, and one Error is reported at its root.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-rendered.too-deep",
    message = "the content nests too deeply to be written as LaTeX: {detail}"
)]
pub struct NestingTooDeep {
    /// The limit that was exceeded, in words.
    pub detail: String,
}

/// Condition `flm.latex-rendered.worker-thread` (Error): the emitter needs a
/// thread with a large stack, and the operating system refused to create one.
/// Recovery: nothing is written for the tree.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-rendered.worker-thread",
    message = "writing LaTeX needs a thread with a large stack, and the operating system \
               refused to create one: {detail}"
)]
pub struct NoWorkerThread {
    /// The operating system's error, as text.
    pub detail: String,
}

/// A definition a rule staged as a definition construct
/// (`flm.pres.latex.definition`), written once by name in the style file and
/// nowhere in the flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StagedDefinition {
    /// The name defined: `\name` for a macro, the name for an environment or
    /// a declaration.
    pub name: String,
    /// The definition's LaTeX, ending with a newline.
    pub text: String,
}

/// The result of [`emit_tree`]: one tree's LaTeX, its requirements, the
/// definitions it staged, where it places other trees, and the diagnostics.
#[derive(Debug, Default)]
pub struct Emitted {
    /// The tree's LaTeX in order, as segments of bytes and placed trees.
    pub segments: Vec<LatexSegment>,
    /// The identifiers the LaTeX requires: the core chunk, and the chunk of
    /// every row written.
    pub requires: BTreeSet<Name>,
    /// The definitions the tree staged, each once by name, in order.
    pub definitions: Vec<StagedDefinition>,
    /// The position of each `flm.pres.subtree` node, one per
    /// [`LatexSegment::Tree`] of `segments`, in the same order.
    pub placed_at: Vec<FlmSpan>,
    /// The diagnostics reported, in order.
    pub diags: FlmDiagnostics,
}

/// Writes one lowered tree as rendered LaTeX under `options`.
///
/// The tree is a lowered tree of this backend: every element in it is one of
/// the backend's native set ([`native_elements`](super::native_elements));
/// another element is reported as [`StageFailed`] and writes nothing. The walk
/// runs on a worker thread ([`on_worker_thread`](crate::lang::guard::on_worker_thread));
/// a tree that cannot be written ([`NoWorkerThread`], [`NestingTooDeep`])
/// comes back with no segments and the Error.
pub fn emit_tree(tree: &Arc<NodeTree<Flm>>, options: &LatexRenderedRenderOptions) -> Emitted {
    match guard::on_worker_thread(|| emit_on_worker_thread(tree, options)) {
        Ok(emitted) => emitted,
        Err(error) => {
            let mut diags = FlmDiagnostics::new();
            diags.push(Diagnostic::error(
                NoWorkerThread { detail: error.to_string() },
                nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
            ));
            Emitted { diags, ..Emitted::default() }
        }
    }
}

fn emit_on_worker_thread(tree: &Arc<NodeTree<Flm>>, options: &LatexRenderedRenderOptions) -> Emitted {
    let mut emitter = Emitter::new(options);
    emitter.require(CORE_CHUNK);
    let root = tree.root();
    let children: Vec<NodeRef<'_, Flm>> = root.children().iter().collect();
    if root.is_list() {
        emitter.run(None, &children);
    } else {
        emitter.run(None, &[root]);
    }
    emitter.end_flow();
    emitter.line_start();
    let Emitter { sink, placements, requires, definitions, refused, mut diags, .. } = emitter;
    let latex = sink.finish().text;
    if refused {
        diags.push(Diagnostic::error(
            NestingTooDeep { detail: format!("more than {DEPTH_LIMIT} levels") },
            root.span().clone(),
        ));
        return Emitted { diags, ..Emitted::default() };
    }
    let placed_at = placements.iter().map(|(_, _, span)| span.clone()).collect();
    Emitted { segments: segments_of(latex, &placements), requires, definitions, placed_at, diags }
}

/// The written LaTeX cut into segments where a tree was placed.
fn segments_of(latex: String, placements: &[(usize, TreeId, FlmSpan)]) -> Vec<LatexSegment> {
    let mut segments = Vec::new();
    let mut at = 0usize;
    for (offset, tree, _) in placements {
        if *offset > at {
            segments.push(LatexSegment::Latex(String::from(&latex[at..*offset])));
        }
        segments.push(LatexSegment::Tree(*tree));
        at = *offset;
    }
    if at < latex.len() {
        segments.push(LatexSegment::Latex(String::from(&latex[at..])));
    }
    segments
}

// --- values of the key-value argument ------------------------------------------------------

/// One value of a key-value argument, written as its row's key says (see
/// [`contract::Value`](super::contract::Value)).
pub(super) enum KeyValue<'t> {
    /// A name, bare.
    Name(String),
    /// Kind tokens, braced, separated by a space.
    Words(Vec<String>),
    /// Lowered content, braced.
    Content(Vec<NodeRef<'t, Flm>>),
    /// Document text, braced.
    Text(String),
    /// An address, braced.
    Address(String),
    /// An anchor (a target's name), bare where it may stand bare
    /// ([`is_bare_name`]), else braced and spelled as an address is
    /// ([`anchor_spelling`]), so that it stays a valid label name.
    Anchor(String),
    /// A length.
    Length(Length),
    /// A number.
    Number(u64),
    /// A boolean.
    Bool(bool),
    /// One braced key-value list per cell, already written.
    Specs(Vec<String>),
}

/// Whether `name` may be written bare as a value: letters, digits and
/// `. - _ : / + @ *`, and not empty.
pub(crate) fn is_bare_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().all(|ch| ch.is_ascii_alphanumeric() || ".-_:/+@*".contains(ch))
}

/// An address or an anchor as LaTeX reads it back as itself, in a key's value
/// or the argument of `\flmxTarget`: the LaTeX family's address escaping
/// ([`escape_url`]: `#`, `%` and `\` after a backslash), and `~` written
/// `\string~`, so that neither `\href` nor a label name sees an active
/// character. An anchor is never written as escaped text: `\_` or
/// `\textasciitilde` would not survive in a label's name.
pub(crate) fn anchor_spelling(address: &str) -> String {
    escape_url(address).replace('~', "\\string~")
}

/// Whether `ch` is white space document text may break or collapse at.
fn is_space(ch: char) -> bool {
    matches!(ch, ' ' | '\t' | '\n' | '\r')
}

/// The children of a node's body slot.
fn body<'t>(node: &NodeRef<'t, Flm>) -> Vec<NodeRef<'t, Flm>> {
    node.body().map(|slice| slice.iter().collect()).unwrap_or_default()
}

/// Whether `node` is a node of the element `name`.
fn is_element(node: &NodeRef<'_, Flm>, name: &str) -> bool {
    element_of(node).name == name
}

/// Whether a run of nodes shows nothing: only white space, comments and
/// paragraph breaks, at any depth of groups.
fn shows_nothing(content: &[NodeRef<'_, Flm>]) -> bool {
    let mut pending: Vec<NodeRef<'_, Flm>> = content.to_vec();
    while let Some(node) = pending.pop() {
        if let Some(text) = node.chars() {
            if !text.chars().all(is_space) {
                return false;
            }
        } else if node.is_comment() || is_element(&node, structural::PARBREAK) {
            continue;
        } else if is_presentation(&node) {
            return false;
        } else {
            pending.extend(node.children().iter());
        }
    }
    true
}

/// Whitespace waiting to be written before the next thing written.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Pending {
    None,
    Space,
    Newline,
}

/// What the current container wrote last, for the blank lines between
/// paragraphs and blocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Last {
    Nothing,
    Paragraph,
    Block,
}

/// The emitter's state over one tree.
pub(super) struct Emitter<'o> {
    pub(super) sink: LatexSink,
    pub(super) options: &'o LatexRenderedRenderOptions,
    pub(super) requires: BTreeSet<Name>,
    pub(super) definitions: Vec<StagedDefinition>,
    placements: Vec<(usize, TreeId, FlmSpan)>,
    depth: usize,
    refused: bool,
    pub(super) diags: FlmDiagnostics,
    /// Bytes written since the last newline.
    col: usize,
    /// Whether a run of inline content (a paragraph) is open.
    pub(super) flow: bool,
    /// Whether the open run has written nothing yet: the blank line before
    /// it waits for its first byte, so that a run holding nothing writes
    /// nothing.
    opening: bool,
    last: Last,
    pending: Pending,
    /// Above zero inside a key-value argument, a verbatim argument or a
    /// formula: no line is broken there.
    pub(super) no_break: u32,
    links_open: u32,
    lists_open: u32,
    /// Whether the last bytes written are a control word (`\name`, the name
    /// ending in a letter) that no argument followed: a letter written next
    /// is preceded by one space, so that it does not extend the name.
    control_word: bool,
}

impl<'o> Emitter<'o> {
    pub(super) fn new(options: &'o LatexRenderedRenderOptions) -> Emitter<'o> {
        Emitter {
            sink: LatexSink::new(EscapeConfig::new(UnicodeMode::Declare)),
            options,
            requires: BTreeSet::new(),
            definitions: Vec::new(),
            placements: Vec::new(),
            depth: 0,
            refused: false,
            diags: FlmDiagnostics::new(),
            col: 0,
            flow: false,
            opening: false,
            last: Last::Nothing,
            pending: Pending::None,
            no_break: 0,
            links_open: 0,
            lists_open: 0,
            control_word: false,
        }
    }

    fn descend(&mut self) -> bool {
        if self.depth >= DEPTH_LIMIT {
            self.refused = true;
            return false;
        }
        self.depth += 1;
        true
    }

    fn ascend(&mut self) {
        self.depth -= 1;
    }

    /// Records that the LaTeX requires the identifier `id`.
    pub(super) fn require(&mut self, id: &str) {
        self.requires.insert(Name::from(id));
    }

    /// Requires the chunk of `row`, where it has one.
    fn require_row(&mut self, row: &Row) {
        if let Some(chunk) = row.chunk {
            self.require(chunk);
        }
    }

    /// Reports an element the emitter does not write, reaching it anyway.
    pub(super) fn unexpected(&mut self, node: &NodeRef<'_, Flm>, what: &str) {
        self.diags.push(Diagnostic::error(
            StageFailed {
                stage: String::from(super::stages::EMIT_STAGE),
                detail: format!(
                    "{what} `{}` reached the latex-rendered emitter, which cannot write it",
                    element_of(node).name
                ),
            },
            node.span().clone(),
        ));
    }

    // --- writing bytes ----------------------------------------------------------------------

    /// Advances the column over `written`.
    fn advance(&mut self, written: &str) {
        match written.rfind('\n') {
            Some(at) => self.col = written.len() - at - 1,
            None => self.col += written.len(),
        }
    }

    /// Writes the white space waiting, if any.
    fn flush(&mut self) {
        if std::mem::replace(&mut self.opening, false) {
            self.newline();
        }
        let pending = std::mem::replace(&mut self.pending, Pending::None);
        let newline = match pending {
            Pending::None => return,
            Pending::Space => self.no_break == 0 && self.col > LINE_WIDTH,
            Pending::Newline => self.no_break == 0,
        };
        if newline {
            self.control_word = false;
            self.sink.newline();
            self.col = 0;
        } else {
            self.syntax(" ");
        }
    }

    /// Writes the emitter's own LaTeX syntax, which never holds a `%`.
    pub(super) fn put(&mut self, syntax: &str) {
        self.flush();
        self.syntax(syntax);
    }

    fn syntax(&mut self, syntax: &str) {
        self.end_control_word(syntax);
        let raw = LatexRaw::from_raw_payload(&LatexRawPayload::new(syntax));
        if self.sink.raw(&raw).is_ok() {
            self.advance(syntax);
        }
    }

    /// Writes bytes a rule or the document gave, reporting the sink's
    /// refusal of a `%` that would start a comment at `node`.
    pub(super) fn put_raw(&mut self, node: &NodeRef<'_, Flm>, raw: &LatexRaw) {
        self.flush();
        self.end_control_word(raw.as_str());
        match self.sink.raw(raw) {
            Ok(()) => self.advance(raw.as_str()),
            Err(refused) => self.diags.push(Diagnostic::error(
                StageFailed {
                    stage: String::from(super::stages::EMIT_STAGE),
                    detail: refused.to_string(),
                },
                node.span().clone(),
            )),
        }
    }

    /// Writes one space where a control word was just written and `next`,
    /// the bytes about to be written, begins with a letter (under LuaTeX, any
    /// Unicode letter would extend the name).
    fn end_control_word(&mut self, next: &str) {
        if std::mem::take(&mut self.control_word) && next.starts_with(char::is_alphabetic) {
            self.syntax(" ");
        }
    }

    /// Ends the current line, dropping the white space waiting.
    pub(super) fn newline(&mut self) {
        self.control_word = false;
        self.pending = Pending::None;
        self.sink.newline();
        self.col = 0;
    }

    /// Ends the current line unless it is empty.
    pub(super) fn line_start(&mut self) {
        if self.col != 0 {
            self.newline();
        } else {
            self.pending = Pending::None;
        }
    }

    /// Writes document text, escaped, with its white space collapsed and the
    /// line broken where it is long.
    pub(super) fn text(&mut self, text: &str) {
        let mut text = text;
        if !self.flow {
            if text.chars().all(is_space) {
                return;
            }
            self.begin_flow();
            text = text.trim_start_matches(is_space);
        }
        let mut rest = text;
        while !rest.is_empty() {
            let space = rest.find(|ch: char| !is_space(ch)).unwrap_or(rest.len());
            if space > 0 {
                let run = &rest[..space];
                let wanted = if run.contains('\n') { Pending::Newline } else { Pending::Space };
                self.pending = self.pending.max(wanted);
                rest = &rest[space..];
                continue;
            }
            let word = rest.find(is_space).unwrap_or(rest.len());
            self.flush();
            self.end_control_word(&rest[..word]);
            let before = self.sink.position();
            self.sink.text(&rest[..word]);
            self.col += self.sink.position() - before;
            rest = &rest[word..];
        }
    }

    // --- layout ------------------------------------------------------------------------

    /// Opens a run of inline content where none is open: a new paragraph,
    /// after a blank line where the container wrote something before.
    pub(super) fn inline(&mut self) {
        if !self.flow {
            self.begin_flow();
        }
    }

    fn begin_flow(&mut self) {
        self.line_start();
        self.opening = self.last != Last::Nothing;
        self.flow = true;
    }

    /// Closes the run of inline content, if one is open.
    pub(super) fn end_flow(&mut self) {
        if self.flow {
            self.flow = false;
            if !self.opening {
                self.last = Last::Paragraph;
            }
            self.opening = false;
            self.pending = Pending::None;
        }
    }

    /// Starts a block-level element on a line of its own, after a blank line
    /// where a paragraph precedes it.
    pub(super) fn begin_block(&mut self) {
        let after_paragraph = (self.flow && !self.opening) || self.last == Last::Paragraph;
        self.end_flow();
        self.line_start();
        if after_paragraph {
            self.newline();
        }
    }

    /// Ends a block-level element: its line is ended.
    pub(super) fn end_block(&mut self) {
        self.line_start();
        self.flow = false;
        self.last = Last::Block;
    }

    /// Writes `nodes` as the blocks of a container: a fresh run of
    /// paragraphs and blocks, ended on a line of its own.
    fn container(&mut self, carrier: Option<&NodeRef<'_, Flm>>, nodes: &[NodeRef<'_, Flm>]) {
        let last = std::mem::replace(&mut self.last, Last::Nothing);
        self.flow = false;
        self.run(carrier, nodes);
        self.end_flow();
        self.line_start();
        self.last = last;
    }

    /// Writes `nodes` as inline content, inside an argument.
    pub(super) fn inline_content(&mut self, carrier: Option<&NodeRef<'_, Flm>>, nodes: &[NodeRef<'_, Flm>]) {
        let flow = std::mem::replace(&mut self.flow, true);
        self.run(carrier, nodes);
        self.flow = flow;
    }
}

/// One cell's spec for the key `cells`: `{row=…, col=…, …}`, the position
/// counted from one. A style name
/// that may not stand bare is braced and spelled as an anchor is
/// ([`anchor_spelling`]), so that it reads back as itself.
fn cell_spec(cell: &pres::CellSpec) -> String {
    // The payload counts from zero; the spec counts from one.
    let mut parts = vec![format!("row={}", u64::from(cell.row) + 1), format!("col={}", u64::from(cell.col) + 1)];
    if cell.row_span > 1 {
        parts.push(format!("row-span={}", cell.row_span));
    }
    if cell.col_span > 1 {
        parts.push(format!("col-span={}", cell.col_span));
    }
    if let Some(align) = cell.align {
        parts.push(format!("align={}", align.as_str()));
    }
    if let Some(header) = cell.header {
        parts.push(format!("header={}", header.as_str()));
    }
    if !cell.rules.is_empty() {
        let rules: Vec<&str> = cell.rules.iter().map(|edge| edge.as_str()).collect();
        parts.push(format!("rules={{{}}}", rules.join(" ")));
    }
    if !cell.styles.is_empty() {
        let styles: Vec<String> = cell
            .styles
            .iter()
            .map(|style| {
                if is_bare_name(style.as_str()) {
                    style.to_string()
                } else {
                    format!("{{{}}}", anchor_spelling(style.as_str()))
                }
            })
            .collect();
        parts.push(format!("styles={{{}}}", styles.join(" ")));
    }
    format!("{{{}}}", parts.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_name_is_bare_only_when_every_character_may_stand_bare() {
        assert!(is_bare_name("flm.theorems.theorem"));
        assert!(is_bare_name("sec-intro_2:a"));
        assert!(!is_bare_name(""));
        assert!(!is_bare_name("a b"));
        assert!(!is_bare_name("a,b"));
        assert!(!is_bare_name("a]b"));
        assert!(!is_bare_name("a%b"));
        assert!(!is_bare_name("a#b"));
    }

    #[test]
    fn a_cell_spec_lists_only_what_the_cell_sets() {
        // The payload's position (0, 1) is the spec's (1, 2).
        let mut cell = pres::CellSpec::at(0, 1);
        assert_eq!(cell_spec(&cell), "{row=1, col=2}");
        cell.col_span = 2;
        cell.rules = vec![pres::CellEdge::Below];
        assert_eq!(cell_spec(&cell), "{row=1, col=2, col-span=2, rules={below}}");
        cell.styles = vec![Name::from("green"), Name::from("a b")];
        assert_eq!(cell_spec(&cell), "{row=1, col=2, col-span=2, rules={below}, styles={green {a b}}}");
    }
}
