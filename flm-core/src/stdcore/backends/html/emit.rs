//! The HTML emitter: writes the HTML of one lowered tree through the
//! [HTML sink](super::sink).
//!
//! An emitter is a backend's emit stage: it walks the lowered trees and writes
//! the output's bytes into a table, from which the backend's final stage makes
//! the artifact set (see [backends, lowering, and
//! outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)).
//! On this backend, [`emit_tree`] writes one tree; the emit stage
//! ([`HtmlEmitStage`](super::stages::HtmlEmitStage)) calls it once per
//! document tree, and the stage `flm.html.emit-pieces` once per
//! [piece](crate::core::piece::Piece) the render places. It
//! returns an [`EmittedTree`]: the tree's HTML in segments, the identifiers of
//! the style and script definitions the tree requires, and the diagnostics
//! reported along the way.
//!
//! # What is written for each element
//!
//! The tag, the classes and the attributes of every presentation element come
//! from that element's row of the [conventions table](super::conventions), the
//! HTML contract written down as data: the emitter reads the row and never
//! spells a class or an attribute name of its own. The row also gives the
//! style definition the element needs, which the emitter **requires** (records
//! as a requirement of the tree) when it writes the element. The [rendering
//! guide](crate::guide::rendering#the-html-contract) prints the table.
//!
//! A **paragraph with nothing to show** (its content empty, or only spaces,
//! source comments and paragraph breaks) is not written at all: the text
//! backend prints nothing for one, and an empty `<p>` would only add space on
//! the page. An element inside it, even an empty one, counts as something to
//! show. A paragraph carrying an anchor, which a link can lead to, is written
//! even when it is empty.
//!
//! Beyond the rows, these elements are written with more than one tag:
//!
//! - a **list** writes `<li>` per entry, with a rich label as a
//!   `<span class="flm-pres-list-label">` at its start; a description list
//!   (`<dl>`) writes the label as a `<dt>` carrying the same class and the
//!   entry as a `<dd>`, because those are the only children a `<dl>` may have.
//!   An entry's anchor is the `id` of its `<li>`, or of its `<dt>`;
//! - a **table of cells** writes a `<tr>` per row of its grid and, in it, a
//!   `<td>` (a `<th>` carrying `scope` for a header cell) per position that no
//!   span covers: a cell at its top-left position, with `colspan` and
//!   `rowspan` where it spans, its alignment, its rules and its custom styles
//!   as `data-flm-…` attributes and its content as blocks; an empty `<td>`
//!   where no cell is placed;
//! - **verbatim content** as a block is `<pre><code>…</code></pre>`, with the
//!   class and the attributes on the `<pre>` and the content written as text,
//!   character for character;
//! - an **unattributed macro** (one no feature declares) is a `<span>` around
//!   the text the author wrote; an **unattributed environment** is a `<div>`
//!   holding a `\begin{…}` line, the lowered body, and an `\end{…}` line, each
//!   line a `<span class="flm-pres-unknown-marker">`;
//! - a [**decoration**](crate::guide::concepts_overview#decorations-roles-and-kinds)
//!   of every kind is a `<span>` written at the position of its decoration mark,
//!   in the flow of the text. A margin decoration is one too, which the core
//!   stylesheet (`flm.html.style.core`) sets beside the text, because a mark can be inside a paragraph
//!   and an `<aside>` inside a `<p>` is not valid HTML;
//! - a **link inside another link's content** is a `<span>` carrying the
//!   link's classes and kind but no `href`, role or accessible name, because
//!   an `<a>` inside an `<a>` is not valid HTML; the outer link is the one a
//!   reader follows. Nothing is reported, since a document may legitimately
//!   nest links this way (a term link inside the text of an `\href`).
//!
//! # Mathematics
//!
//! Passthrough mathematics (a formula whose source the
//! [`math-passthrough`](crate::math_passthrough) feature
//! carries unread, as a `flm.pres.math-passthrough` node) is not translated
//! into HTML: it is handed, as the LaTeX the author wrote, to a **formula
//! renderer**, a script in the reader's browser that reads
//! LaTeX out of the page and typesets it. FLM does not inspect what a formula
//! says ([`mod@super::mathjax`] states what that means for a host). Every
//! formula, inline or displayed, is a `<span>` carrying the class, the display
//! (`data-flm-display`) and the environment's name; the stylesheet
//! of passthrough mathematics (`flm.html.style.math-passthrough`) gives a
//! displayed formula the appearance of a block. One tag serves both
//! because a displayed formula can be inside a paragraph, and HTML does not
//! allow a block element there. The emitter writes the delimiters (`\(…\)`,
//! `\[…\]` or `\begin{…}…\end{…}`), and the document's own source between
//! them as text. It requires the script definition
//! [`flm.html.mathjax`](super::conventions::SCRIPT_MATHJAX) at the first
//! passthrough formula it writes in a tree; a page that does not carry that
//! script shows
//! the LaTeX as characters.
//!
//! **Content inside a formula** (the text of `\text{…}`, a link, a point a
//! reference can lead to) is a `flm.pres.math-text` node among the formula's
//! source. The emitter writes it as HTML inside the formula, through the
//! formula renderer's `<tex-html>` element:
//! `<tex-html><span class="flm-pres-math--text" …>…</span></tex-html>`.
//! The renderer searches the whole formula for its closing delimiter and for
//! that element, so the emitter breaks up any such sequence in the text it
//! writes inside a formula; an attribute value holding one is left out and
//! reported as [`RefusedInMath`].
//!
//! The repair of the text is about the `<tex-html>` element, not about who
//! staged it: the text written inside a `flm.pres.html.tag` construct whose
//! tag is `tex-html` (which a rule stages to put HTML inside a formula it
//! writes itself, as a processing feature's rule does) is repaired too, for
//! every closing delimiter (`\)`, `\]`, `\end{…}`) and the two spellings of
//! the tag, since the emitter does not know which formula the rule wrote
//! around it.
//!
//! # The trust guarantee
//!
//! Every byte goes through the [sink](super::HtmlSink): document text through
//! its text entry point, every attribute value through its attribute entry
//! points, each escaped where it is written. This module builds no HTML by
//! formatting strings and writes no unescaped byte.
//!
//! Two kinds of value are checked before they are written, because escaping
//! does not make them safe:
//!
//! - **an address** (a link's `href`, an image's `src` and the candidates of
//!   its `srcset`) must pass the [address policy](super::check_address). A refused
//!   address is not written and is reported as the Error
//!   `flm.html.refused-address` ([`AddressRefused`]); the link keeps its text
//!   and the image its alternative text, because content is never dropped.
//! - **a tag or attribute name** of a [construct node](super::construct),
//!   which a lowering rule wrote, must pass the [name check](super::names). A
//!   node whose name fails it is written as the unsupported stand-in (an empty
//!   `<span class="flm-pres-unsupported">`) and
//!   reported as `flm.html.invalid-name` ([`InvalidName`]).
//!
//! A third check applies to the two names a rule writes into a node of a
//! presentation element: a **role**, written as one class, and a **kind**, written
//! as one token of `data-flm-kind` (see [decorations, roles and
//! kinds](crate::guide::concepts_overview#decorations-roles-and-kinds)). Both
//! attributes separate their tokens by whitespace, so a name holding
//! whitespace would be read as several; such a name is left out (that one
//! class, that one token) and reported as `flm.html.invalid-name`, and the rest
//! of the node is written as usual.
//!
//! A construct node is outside the HTML contract and outside the address
//! policy: a lowering rule that stages an HTML tag node (builds it into the
//! lowered tree; see [`Lowerer`](crate::core::lowering::Lowerer)) is
//! responsible for the attributes it writes.
//!
//! # Trees placed in other trees
//!
//! A `flm.pres.subtree` node
//! ([`SUBTREE`](crate::stdcore::backends::pres::SUBTREE), [`SubtreePayload`])
//! places another document tree at its position. This emitter sees one tree
//! only, so it ends the segment it is writing and
//! records the placed tree ([`HtmlSegment::Tree`]); the regions stage, which
//! sees every tree, writes the placed tree's HTML there.
//!
//! # Limits on nesting
//!
//! The walk recurses as deeply as the tree nests, so it must run on a worker
//! thread, a thread created with at least
//! [`WORKER_STACK_SIZE`](crate::lang::guard::WORKER_STACK_SIZE) bytes of stack
//! ([`on_worker_thread`](crate::lang::guard::on_worker_thread); see [the
//! recursion guard and worker
//! threads](crate::guide::concepts_overview#the-recursion-guard-and-worker-threads)).
//! [`emit_tree`] runs it on one. The walk also refuses a tree that nests deeper
//! than [`DEPTH_LIMIT`], writing nothing for it and reporting
//! `flm.html.nesting-too-deep` ([`NestingTooDeep`]).

use std::collections::BTreeSet;
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

use techy::core::node::{NodeKind, NodeRef, NodeTree};
use techy::error::{Diagnostic, DiagnosticInfo};

use crate::lang::guard;
use crate::layout::is_breakable_space;
use crate::lang::structural;
use crate::lang::{Flm, FlmDiagnostics, FlmGroupType, FlmOrigin, FlmSpan, nowhere};
use crate::model::addr::TreeId;
use crate::model::element::element_of;
use crate::model::names::Name;
use crate::stdcore::backends::pres::{
    self, BlockPayload, CellHeader, CellSpec, DroppedPayload, GridPosition, HeadingPayload,
    ImageDisplay, ImagePayload, Length, LinePayload, LinkPayload, ListPayload, MathDisplay,
    MathPassthroughPayload, MathTextPayload, ParPayload, SpanPayload, SpanStyle, SubtreePayload,
    UnknownKind, UnknownPayload, UnsupportedPayload, VerbatimDisplay, VerbatimPayload, cells_of,
    decoration_at, ends_in_sentence_punctuation, heading_depth, holds_text, is_presentation,
    list_entries, payload_of, recompose_source,
};

use super::address::{AddressUse, check_address};
use super::construct::{HtmlTagPayload, TagBody};
use super::conventions::{
    Convention, LIST_LABEL_CLASS, SCRIPT_MATHJAX, STYLE_ATTRIBUTE, UNKNOWN_MARKER_CLASS, Written,
    block_tag, css_length, css_ratio, doc_role_value, row,
};
use super::escape::json_string_into;
use super::feature::HtmlRenderOptions;
use super::names::{class_of_element, class_of_role, data_attribute, is_valid_attribute_name,
    is_valid_tag_name, is_valid_token, style_property};
use super::sink::HtmlSink;
use super::stages::{HtmlSegment, StageFailed};

// --- conditions ---------------------------------------------------------------------------

/// Condition `flm.html.invalid-name` (Error): a name the emitter would write
/// into the page is not a valid name of its kind, and writing it unchanged
/// could produce markup nobody asked for.
///
/// Four kinds of name are checked, and each has its own recovery:
///
/// - the **tag** or an **attribute** name a [construct node](super::construct)
///   carries. Such a name is writable when it is an ASCII letter followed by
///   ASCII letters, digits and `-` (an attribute name also `_`, `:` and `.`);
///   anything else could end the tag early or begin something the rule did not
///   write. Recovery: the node is written as the unsupported stand-in, an
///   empty `<span class="flm-pres-unsupported">` naming the element.
/// - a **role** or a **kind** a presentation node's payload carries, which is
///   written as one class or as one token of `data-flm-kind`. Such a name is
///   writable when it is one token — not empty and holding no whitespace
///   ([`is_valid_token`]), since both of those attributes separate their
///   tokens by whitespace. Recovery: that one class or that one token is left
///   out (and when no kind is left, the `data-flm-kind` attribute is not
///   written at all), while everything else about the node is written as
///   usual. The condition's `recovery` field says which of those happened.
/// - a **tag** a host maps a heading kind to, in
///   [`HtmlRenderOptions::heading_tags`]. Recovery: the heading is written with
///   the tag of its level counted from the root heading level, `<h1>` to
///   `<h6>`.
/// - the name of a **custom style** a cell of a table of cells carries, which
///   is written as the attribute `data-flm-style-<name>` on the cell. Such a
///   name is writable when the whole attribute name is, as for a construct
///   node's attribute above. Recovery: that one attribute is left out, and
///   the cell is written otherwise as usual.
///
/// None of these names is document text: the feature whose lowering rule
/// staged the node wrote it, or the host gave it in its render options, so this
/// condition reports a mistake in a feature or a host rather than in a
/// document. It is reported at the node's position.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.html.invalid-name",
    message = "the {what} name `{name}` cannot be written into the HTML page: a tag name must be \
               an ASCII letter followed by ASCII letters, digits and `-`, an attribute name may \
               also contain `_`, `:` and `.`, and a role or a kind must be one word, not empty \
               and without whitespace"
)]
pub struct InvalidName {
    /// The kind of name that was refused: `tag`, `attribute`, `role` or
    /// `kind`.
    pub what: String,
    /// The name, as the payload or the render options gave it.
    pub name: String,
    /// What the emitter wrote instead, in words (data for a reader of the
    /// diagnostic; the message does not include it).
    pub recovery: String,
}

/// Condition `flm.html.refused-address` (Error): a link's or an image's address
/// has a scheme that the [address policy](super::check_address) does not allow, such
/// as `javascript`.
///
/// Reported at the link's or the image's position. Recovery: the address is
/// not written. The link keeps its text and the image its alternative text, so
/// the content stays and only the address is lost.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.html.refused-address",
    message = "the address `{address}` in the `{attribute}` attribute uses the scheme `{scheme}`, which is \
               not allowed; the allowed addresses are those with the scheme `http`, `https`, \
               `mailto`, `ftp` or `tel`, those without a scheme, and, for an image, \
               `data:image/…`"
)]
pub struct AddressRefused {
    /// The address as the link or image gives it.
    pub address: String,
    /// The scheme, in lower case and without its colon.
    pub scheme: String,
    /// The attribute the address would have been written in: `href`, `src` or
    /// `srcset`.
    pub attribute: String,
}

/// Condition `flm.html.refused-in-math` (Error): an attribute value the
/// emitter would write inside a formula contains a sequence that would break
/// the formula.
///
/// Content inside a formula is written as HTML inside the formula's LaTeX
/// ([the module documentation](self#mathematics)). The formula renderer
/// searches all the characters of the formula, attribute values included, for
/// the closing delimiter the emitter wrote and for its `<tex-html>` tag. Text
/// holding one of those sequences is broken up with an empty element where it
/// is written; an attribute value cannot be broken up that way, because an
/// element written into it would reach the reader as characters.
///
/// Reported at the node's position, or at the formula's when the element
/// belongs to no node of its own. Recovery: that one attribute is left out,
/// and everything else about the element, its content above all, is written
/// as usual.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.html.refused-in-math",
    message = "an attribute value inside a formula must not contain `{sequence}`, which the \
               formula renderer would read as the end of the formula or as its own tag: the \
               value of the `{attribute}` attribute contains it"
)]
pub struct RefusedInMath {
    /// The attribute that was left out, by the name it would have been
    /// written under: `id`, `class`, a `data-flm-…` attribute, or an attribute
    /// a lowering rule asked for.
    pub attribute: String,
    /// The characters in the value that the formula renderer would read as
    /// the end of the formula or as its own tag: the closing delimiter of the
    /// surrounding formula (`\)`, `\]` or `\end{<env>}`), `<tex-html` or
    /// `</tex-html`.
    pub sequence: String,
}

/// Condition `flm.html.worker-thread-unavailable` (Error): the emitter could
/// not run, because the calling thread is not a worker thread and the
/// operating system refused to create one.
///
/// A worker thread is a thread created with at least
/// [`WORKER_STACK_SIZE`](crate::lang::guard::WORKER_STACK_SIZE) bytes of stack
/// ([`on_worker_thread`](crate::lang::guard::on_worker_thread)); the emitter
/// needs one because the walk recurses as deeply as the tree nests. Reported at
/// no position in a document. Recovery: nothing is written for the tree.
// The refusal is the operating system's and cannot be provoked without
// injecting a failure, so no test covers this arm. See decision rationale at
// [§flm-dd-dr:lang-worker-stack].
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.html.worker-thread-unavailable",
    message = "writing HTML needs a thread with a large stack, and the operating system \
               refused to create one: {detail}"
)]
pub struct NoWorkerThread {
    /// The operating system's error.
    pub detail: String,
}

/// The emitter's per-level stack estimate: an upper bound on the stack one
/// level of nesting of the walk uses, taken from the text emitter's
/// measurement of the same shape of recursion and tripled, so that the limit
/// holds in every build profile.
const LEVEL_STACK_ESTIMATE: usize = 12 * 1024;

/// The nesting depth past which the emitter refuses a tree and reports
/// `flm.html.nesting-too-deep` ([`NestingTooDeep`]): the recursion guard's
/// stack budget ([`STACK_BUDGET`](crate::lang::guard::STACK_BUDGET)) divided by
/// the estimated stack use of one level, which is 4096 levels.
///
/// A tree that came from a parsed document never comes near it, since the
/// parse refuses far shallower nesting. A lowered tree can, when its lowering
/// rules add levels of their own.
pub const DEPTH_LIMIT: usize = guard::STACK_BUDGET / LEVEL_STACK_ESTIMATE;

/// Condition `flm.html.nesting-too-deep` (Error): a lowered tree nests deeper
/// than the emitter's depth limit ([`DEPTH_LIMIT`]).
///
/// Every recursive tree walk in this library is bounded in this way, and each
/// reports its own condition, so a diagnostic says which step ran out of room.
/// Recovery: nothing is written for that tree, and one Error is reported at
/// its root.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.html.nesting-too-deep",
    message = "the content nests too deeply to be written as HTML: {detail}"
)]
pub struct NestingTooDeep {
    /// The limit that was exceeded, in words.
    pub detail: String,
}

/// What a diagnostic shows in place of a position for a condition this backend
/// reports without a position in any document: the label of the empty source
/// that [`nowhere`](crate::lang::nowhere) spans.
pub(crate) const NOWHERE_LABEL: &str = "html backend";

// --- the entry point ----------------------------------------------------------------------

/// The result of [`emit_tree`]: one tree's HTML, its requirements, where it
/// places other trees, and the diagnostics reported.
#[derive(Debug, Default)]
pub struct EmittedTree {
    /// The tree's HTML in order, as segments of bytes and placed trees. Empty
    /// when nothing was written.
    pub segments: Vec<HtmlSegment>,
    /// The identifiers of the style and script definitions the tree's HTML
    /// requires, as an ordered set.
    pub requires: BTreeSet<Name>,
    /// The position of each `flm.pres.subtree` node, one for each
    /// [`HtmlSegment::Tree`] of `segments`, in the same order, so that a caller
    /// reporting a placement can give its position.
    pub placed_at: Vec<FlmSpan>,
    /// The diagnostics reported, in order.
    pub diags: FlmDiagnostics,
}

/// Writes one lowered tree as HTML under the render options `options`, and
/// returns the HTML with the tree's requirements and diagnostics
/// ([`EmittedTree`]).
///
/// The tree is a lowered tree of the HTML backend: every element in it is one
/// the backend writes natively ([`native_elements`](super::native_elements));
/// another element is reported as
/// [`StageFailed`] and writes nothing. The walk
/// recurses as deeply as the tree nests, so it must run on a worker thread:
/// this function runs it on the calling thread when that thread is a worker
/// thread, and on a new worker thread otherwise
/// ([`on_worker_thread`](crate::lang::guard::on_worker_thread)).
///
/// A tree that cannot be written (no worker thread available, nesting deeper
/// than [`DEPTH_LIMIT`]) comes back with no segments and an Error diagnostic
/// ([`NoWorkerThread`], [`NestingTooDeep`]). The emit stages
/// ([`HtmlEmitStage`](super::stages::HtmlEmitStage),
/// [`HtmlEmitPiecesStage`](super::stages::HtmlEmitPiecesStage)) call it; a host
/// may call it directly on a lowered tree.
pub fn emit_tree(tree: &Arc<NodeTree<Flm>>, options: &HtmlRenderOptions) -> EmittedTree {
    match guard::on_worker_thread(|| emit_on_worker_thread(tree, options)) {
        Ok(emitted) => emitted,
        Err(error) => {
            let mut diags = FlmDiagnostics::new();
            diags.push(Diagnostic::error(
                NoWorkerThread { detail: error.to_string() },
                nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
            ));
            EmittedTree { diags, ..EmittedTree::default() }
        }
    }
}

/// The emission itself, once the caller is known to be on a worker thread.
fn emit_on_worker_thread(tree: &Arc<NodeTree<Flm>>, options: &HtmlRenderOptions) -> EmittedTree {
    let mut emitter = Emitter {
        sink: HtmlSink::new(),
        options,
        placements: Vec::new(),
        requires: BTreeSet::new(),
        depth: 0,
        refused: false,
        math: None,
        in_tex_html: false,
        tex_html_constructs: 0,
        construct_sequences: Arc::from(sequences_in_construct()),
        piece: None,
        links_open: 0,
        diags: FlmDiagnostics::new(),
    };
    let root = tree.root();
    let children: Vec<NodeRef<'_, Flm>> = root.children().iter().collect();
    if root.is_list() {
        emitter.run(None, &children);
    } else {
        emitter.run(None, &[root]);
    }
    let Emitter { sink, placements, requires, refused, mut diags, .. } = emitter;
    let html = sink.finish();
    if refused {
        diags.push(Diagnostic::error(
            NestingTooDeep {
                detail: format!("more than {DEPTH_LIMIT} levels"),
            },
            root.span().clone(),
        ));
        return EmittedTree { diags, ..EmittedTree::default() };
    }
    let placed_at = placements.iter().map(|(_, _, span)| span.clone()).collect();
    EmittedTree { segments: segments_of(html, &placements), requires, placed_at, diags }
}

/// The written HTML cut into segments at the byte offsets where a tree was
/// placed, with the placed trees between the parts.
///
/// An empty part — two trees placed side by side, or one placed at the very
/// beginning or the very end — contributes no segment, so a fragment never
/// holds an empty string.
fn segments_of(html: String, placements: &[(usize, TreeId, FlmSpan)]) -> Vec<HtmlSegment> {
    let mut segments = Vec::new();
    let mut at = 0usize;
    for (offset, tree, _) in placements {
        if *offset > at {
            segments.push(HtmlSegment::Html(String::from(&html[at..*offset])));
        }
        segments.push(HtmlSegment::Tree(*tree));
        at = *offset;
    }
    if at < html.len() {
        segments.push(HtmlSegment::Html(String::from(&html[at..])));
    }
    segments
}

// --- mathematics: what the formula renderer reads, and the two repairs --------------------

/// The tag of the mechanism by which HTML is put inside a formula:
/// `tex-html`.
///
/// The formula renderer reads the HTML between a `<tex-html>` and its closing
/// tag **out of the characters of the formula**, before any LaTeX is parsed.
/// Escaping does not hide the tag from it: the renderer reads the text a
/// browser built from the page, in which `&lt;` is already `<` again. So the
/// emitter writes this tag itself, and keeps a document from writing it (see
/// [`neutralized_source`]).
const TEX_HTML: &str = "tex-html";

/// The empty TeX group written into a document's own mathematics source where
/// the tag of [`TEX_HTML`] would otherwise be: `{}`.
///
/// It takes no space, it typesets nothing, and — unlike a macro that would do
/// the same — a document cannot redefine it.
const EMPTY_GROUP: &str = "{}";

/// A sequence of characters that must not reach the formula renderer's search
/// whole, held as the two parts the repair is written between.
///
/// Three such sequences exist while a formula is being written: the closing
/// delimiter of that formula ([`closing_delimiter`]), and the two spellings of
/// the tag of [`TEX_HTML`], `<tex-html` and `</tex-html`. Where the repair
/// is placed inside the sequence does not matter to the renderer — any break
/// stops the search from matching — so it is chosen to fall at the sequence's
/// own joint: after the backslash of `\)` and `\]`, after the `\end` of
/// `\end{align}`, after the `<` of a tag.
///
/// Inside a `tex-html` construct tag a rule staged, every closing delimiter is
/// broken, since the emitter does not know the formula around it
/// ([`sequences_in_construct`]).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Sequence {
    /// The characters, unbroken.
    text: String,
    /// The byte offset inside `text` at which the repair is written.
    break_at: usize,
}

impl Sequence {
    /// The sequence `before` followed by `after`, broken between the two.
    fn new(before: &str, after: &str) -> Sequence {
        Sequence { text: format!("{before}{after}"), break_at: before.len() }
    }
}

/// The two spellings of the tag of [`TEX_HTML`] the formula renderer searches
/// for: the opening `<tex-html` and the closing `</tex-html`, each broken
/// after its `<`.
fn tex_html_sequences() -> Vec<Sequence> {
    vec![Sequence::new("<", TEX_HTML), Sequence::new("<", &format!("/{TEX_HTML}"))]
}

/// The LaTeX the emitter writes **before** a formula's own source: `\(` for
/// inline mathematics, `\[` for displayed mathematics, and `\begin{<env>}`
/// for a formula written in a mathematics environment.
fn opening_delimiter(display: MathDisplay, env: Option<&str>) -> String {
    match (display, env) {
        (_, Some(env)) => format!("\\begin{{{env}}}"),
        (MathDisplay::Inline, None) => String::from("\\("),
        (MathDisplay::Display, None) => String::from("\\["),
    }
}

/// The LaTeX the emitter writes **after** a formula's own source, as the
/// sequence that must not reach the formula renderer's search whole from
/// inside the formula: `\)`, `\]` or `\end{<env>}`.
fn closing_delimiter(display: MathDisplay, env: Option<&str>) -> Sequence {
    match (display, env) {
        (_, Some(env)) => Sequence::new("\\end", &format!("{{{env}}}")),
        (MathDisplay::Inline, None) => Sequence::new("\\", ")"),
        (MathDisplay::Display, None) => Sequence::new("\\", "]"),
    }
}

/// Every sequence that must not reach the formula renderer's search whole out
/// of the HTML written inside a formula whose closing delimiter is `closing`:
/// that delimiter, then the two tag spellings.
fn sequences_in_math(closing: &Sequence) -> Vec<Sequence> {
    let mut sequences = vec![closing.clone()];
    sequences.extend(tex_html_sequences());
    sequences
}

/// Every sequence that must not reach the formula renderer's search whole out
/// of the text written inside a `flm.pres.html.tag` construct whose tag is
/// [`TEX_HTML`]: the three closing delimiters `\)`, `\]` and `\end{` (any
/// environment's), then the two tag spellings.
///
/// The rule that staged the construct wrote the formula around it; the
/// emitter does not know which delimiter closes it, so it breaks all three.
fn sequences_in_construct() -> Vec<Sequence> {
    let mut sequences = vec![Sequence::new("\\", ")"), Sequence::new("\\", "]"), Sequence::new("\\end", "{")];
    sequences.extend(tex_html_sequences());
    sequences
}

/// A document's own mathematics source with an [empty TeX group](EMPTY_GROUP)
/// written into every `<tex-html` and `</tex-html` it holds, which keeps a
/// document from reaching the mechanism that puts HTML inside a formula.
///
/// The reader sees the same characters: the group typesets nothing and carries
/// the spacing the `<` had. It closes the case where a document's author typed
/// the characters; an author who can define a macro inside mathematics can
/// still build the tag out of parts, which no substitution over the source
/// can catch; a host closes that case in the formula renderer's own
/// configuration ([`super::mathjax`]).
fn neutralized_source(source: &str) -> String {
    let sequences = tex_html_sequences();
    let mut out = String::with_capacity(source.len());
    let mut at = 0;
    while at < source.len() {
        let rest = &source[at..];
        match sequences.iter().find(|sequence| rest.starts_with(&sequence.text)) {
            Some(sequence) => {
                out.push_str(&sequence.text[..sequence.break_at]);
                out.push_str(EMPTY_GROUP);
                at += sequence.break_at;
            }
            None => {
                let ch = rest.chars().next().expect("a character starts at a character boundary");
                out.push(ch);
                at += ch.len_utf8();
            }
        }
    }
    out
}

/// The byte offsets of `text` at which an empty element must be written so
/// that none of `sequences` reaches the formula renderer's search whole, in
/// ascending order.
///
/// This is the repair for text written **inside** the HTML of a `<tex-html>`,
/// where the empty TeX group of [`neutralized_source`] would be shown to the
/// reader as two characters: an empty element shows nothing and breaks the
/// search just as well.
fn break_points(text: &str, sequences: &[Sequence]) -> Vec<usize> {
    let mut points = Vec::new();
    let mut at = 0;
    while at < text.len() {
        let rest = &text[at..];
        match sequences.iter().find(|sequence| rest.starts_with(&sequence.text)) {
            Some(sequence) => {
                at += sequence.break_at;
                points.push(at);
            }
            None => {
                at += rest
                    .chars()
                    .next()
                    .map_or(1, |ch| ch.len_utf8());
            }
        }
    }
    points
}

/// The first of `sequences` that `value` holds; such a sequence makes an
/// attribute value impossible to write inside the HTML of a `<tex-html>`.
/// `None` when it holds none.
///
/// An attribute value cannot carry the repair of [`break_points`]: an
/// attribute value is text, so an element written into it reaches the
/// reader as its characters. Such an attribute is left out instead
/// ([`RefusedInMath`]).
fn refused_sequence(value: &str, sequences: &[Sequence]) -> Option<String> {
    sequences
        .iter()
        .filter_map(|sequence| value.find(&sequence.text).map(|at| (at, sequence.text.clone())))
        .min_by(|(left, _), (right, _)| left.cmp(right))
        .map(|(_, text)| text)
}

/// An element left open inside the HTML written into a formula, held so
/// that it can be written a second time.
///
/// A formula placed deeper inside that HTML than the content's own nodes —
/// `\text{\emph{\(x\)}}` — is written by closing everything open inside the
/// formula's HTML, writing the inner formula into the formula's own
/// characters, and opening the same elements again
/// ([`math_text`](Emitter::math_text) says why). This is what "the same
/// elements" means: each one's tag, the classes and attributes it was written
/// with, and whether it is block-level or preformatted.
#[derive(Debug, Clone)]
struct OpenInMath {
    /// The tag the element was opened with.
    tag: String,
    /// Whether it was opened as a block-level element.
    block: bool,
    /// The classes written on it, empty when none were.
    classes: Vec<String>,
    /// The attributes written on it, without an `id` and without the
    /// source-position attribute: each of those appears once — an identifier
    /// once in a page, a node's position once on the node's own element — so
    /// both stay on the element's first half.
    attributes: Vec<(String, String)>,
    /// Whether it was opened as preformatted content.
    preformatted: bool,
}

/// One part of content being written as HTML inside a formula — one
/// `<tex-html>` element — while it is being written.
#[derive(Debug, Clone)]
struct Piece {
    /// The classes of the `<span>` the part is written as, which every part
    /// of the same content carries.
    classes: Vec<String>,
    /// The elements open inside that `<span>`, outermost first.
    open: Vec<OpenInMath>,
}

/// The formula the emitter is writing, while it is inside one.
#[derive(Debug, Clone)]
struct MathContext {
    /// The sequences that must not reach the formula renderer's search whole
    /// out of the HTML written inside this formula.
    sequences: Arc<[Sequence]>,
    /// The formula's position, which is the position a diagnostic about a
    /// part of it carries when that part has no position of its own.
    span: FlmSpan,
}

// --- the emitter ----------------------------------------------------------------------------

/// The walk in progress.
struct Emitter<'o> {
    sink: HtmlSink,
    options: &'o HtmlRenderOptions,
    /// Where a tree was placed: the byte offset in what has been written, the
    /// tree, and the placing node's span, in the order encountered.
    placements: Vec<(usize, TreeId, FlmSpan)>,
    /// The definitions the tree's HTML needs.
    requires: BTreeSet<Name>,
    /// The open levels of the walk.
    depth: usize,
    /// Whether the depth limit was hit anywhere in this tree.
    refused: bool,
    /// The formula being written, while the walk is inside one.
    math: Option<MathContext>,
    /// Whether the bytes being written now are inside a `<tex-html>`, which
    /// is the HTML the emitter puts inside a formula.
    in_tex_html: bool,
    /// How many `flm.pres.html.tag` constructs whose tag is `tex-html` the
    /// walk is inside: their text is repaired as the text inside a formula's
    /// `<tex-html>` is ([`sequences_in_construct`]).
    tex_html_constructs: usize,
    /// The sequences of [`sequences_in_construct`], built once for the walk.
    construct_sequences: Arc<[Sequence]>,
    /// The part of content being written as HTML inside a formula, while the
    /// walk is inside one and past its `<span>`.
    piece: Option<Piece>,
    /// How many links the walk is inside: a link reached inside another is
    /// written without an address of its own ([`Emitter::link`]).
    links_open: usize,
    diags: FlmDiagnostics,
}

impl fmt::Debug for Emitter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Emitter")
            .field("depth", &self.depth)
            .field("placements", &self.placements.len())
            .finish_non_exhaustive()
    }
}

/// The children of a node's body slot, or none when it has no body.
fn body<'t>(node: &NodeRef<'t, Flm>) -> Vec<NodeRef<'t, Flm>> {
    node.body().map(|slice| slice.iter().collect()).unwrap_or_default()
}

/// Whether `node` is a node of the element `name`.
fn is_element(node: &NodeRef<'_, Flm>, name: &str) -> bool {
    element_of(node).name == name
}

/// Whether a paragraph's content shows nothing on the page, so that no `<p>`
/// is written for it: every node of it is text made only of spaces a line may
/// be broken at ([`is_breakable_space`]), a comment, a paragraph-break marker,
/// or a group or container holding only such nodes — an empty content
/// included. A presentation element — a span, a link, an image, a formula, a
/// decoration's mark — counts as something shown, even with no text inside:
/// it is written as an element, which a stylesheet may draw or a link may
/// reach.
///
/// This is the criterion the text backend reaches for a paragraph it prints
/// nothing for, stated over the tree: that backend decides while it lays the
/// words out, and there an empty span also prints nothing, while on the page
/// it is nevertheless an element. The walk is iterative, so a deep nesting of
/// groups cannot exhaust the stack.
fn shows_nothing(content: &[NodeRef<'_, Flm>]) -> bool {
    let mut pending: Vec<NodeRef<'_, Flm>> = content.to_vec();
    while let Some(node) = pending.pop() {
        if let Some(text) = node.chars() {
            if !text.chars().all(is_breakable_space) {
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

/// The values a heading's attributes are written from, block or run in: the
/// anchor, the kind's name, the level (1 where the payload is missing) and
/// whether the heading is in the table of contents, by payload field.
fn heading_values(payload: Option<&HeadingPayload>) -> Vec<(&'static str, String)> {
    let mut values: Vec<(&'static str, String)> = Vec::new();
    if let Some(anchor) = payload.and_then(|payload| payload.anchor.clone()) {
        values.push(("anchor", anchor));
    }
    if let Some(payload) = payload {
        values.push(("name", payload.name.to_string()));
    }
    values.push(("level", payload.map_or(1, |payload| payload.level).to_string()));
    if let Some(payload) = payload {
        values.push(("in_toc", payload.in_toc.to_string()));
    }
    values
}

/// The tag a heading `depth` levels below the root heading level is written
/// with ([`heading_depth`]): `<h1>` at the root, `<h2>` to `<h6>` below it,
/// `<h6>` for every deeper one.
fn heading_tag(depth: u8) -> &'static str {
    match depth {
        0 => "h1",
        1 => "h2",
        2 => "h3",
        3 => "h4",
        4 => "h5",
        _ => "h6",
    }
}

/// The tag a span of `style` is written with. A style HTML has an element for
/// gets that element; every other style, and a span with no style at all, gets
/// `<span>` and is told apart by `data-flm-style`.
fn span_tag(style: Option<SpanStyle>) -> &'static str {
    match style {
        Some(SpanStyle::Emphasis) => "em",
        Some(SpanStyle::Bold) => "strong",
        Some(SpanStyle::Italic) => "i",
        Some(SpanStyle::Monospace) => "code",
        Some(SpanStyle::Underline) => "u",
        Some(SpanStyle::Superscript) => "sup",
        Some(SpanStyle::Subscript) => "sub",
        Some(SpanStyle::SmallCaps) | Some(SpanStyle::UnresolvedReference) | None => "span",
    }
}

/// The first structural kind token among a list's kinds —
/// [`LIST_NUMBERED`](pres::LIST_NUMBERED),
/// [`LIST_UNNUMBERED`](pres::LIST_UNNUMBERED) or
/// [`LIST_TERMS`](pres::LIST_TERMS) — and the tag it asks for: `<ol>`, `<ul>`
/// or `<dl>`. A list whose kinds hold none of them is written `<ul>`.
///
/// The tag is chosen by what the list is, never by the name of the
/// environment that produced it, so that a list environment a host adds is
/// written as its structure says.
fn list_tag(kinds: &[Name]) -> &'static str {
    for kind in kinds {
        match kind.as_str() {
            pres::LIST_NUMBERED => return "ol",
            pres::LIST_UNNUMBERED => return "ul",
            pres::LIST_TERMS => return "dl",
            _ => {}
        }
    }
    "ul"
}

/// The attributes of `row` for the values present, in the order the contract
/// fixes: the element's own attributes in the row's order, then the `style`
/// attribute holding the custom properties of the length fields and the
/// ratios in the row's order, then the `data-flm-…` attributes in the row's order.
///
/// `values` holds the text of the fields the row writes as attributes, and
/// `lengths` the value of the fields it writes as custom properties
/// ([`Written::Style`]). This is the only writer of those properties: it
/// spells each length as CSS ([`css_length`]) and writes nothing for a zero
/// length, the stylesheet's fallback being zero, so an element whose lengths
/// are all zero carries no `style` attribute. It writes the ratio of two of
/// those lengths the same way ([`Written::Ratio`], [`css_ratio`]) where both
/// are set and it has one.
///
/// A row lists the field that becomes `id` first, so `id` comes first among
/// the element's own attributes, as the order requires.
fn ordered_attributes(
    convention: &Convention,
    values: &[(&'static str, String)],
    lengths: &[(&'static str, &Length)],
) -> Vec<(String, String)> {
    let mut own: Vec<(String, String)> = Vec::new();
    let mut properties: Vec<String> = Vec::new();
    let mut data: Vec<(String, String)> = Vec::new();
    for field in convention.fields {
        let length_of =
            |wanted: &str| lengths.iter().find(|(name, _)| *name == wanted).map(|(_, length)| *length);
        match field.written {
            Written::Style => {
                if let Some(length) = length_of(field.field).filter(|length| !length.is_zero()) {
                    properties
                        .push(format!("{}: {}", style_property(field.field), css_length(length)));
                }
                continue;
            }
            Written::Ratio(first, second) => {
                if let Some(ratio) = length_of(first)
                    .zip(length_of(second))
                    .and_then(|(first, second)| css_ratio(first, second))
                {
                    properties.push(format!("{}: {ratio}", style_property(field.field)));
                }
                continue;
            }
            _ => {}
        }
        let Some(value) = values.iter().find(|(name, _)| *name == field.field) else { continue };
        match field.written {
            Written::Own(attribute) => own.push((String::from(attribute), value.1.clone())),
            Written::Data(stem) => data.push((data_attribute(stem), value.1.clone())),
            Written::Style
            | Written::Ratio(..)
            | Written::RoleClass
            | Written::BlockClass
            | Written::Entry(_)
            | Written::Dropped(_) => {}
        }
    }
    if !properties.is_empty() {
        own.push((String::from(STYLE_ATTRIBUTE), properties.join("; ")));
    }
    own.extend(data);
    own
}

/// The attribute `row` writes its field `field` under on the element of each
/// entry of a list or each cell of a table of cells; `None` where the row
/// writes that field nowhere there.
fn entry_attribute(convention: &Convention, field: &str) -> Option<&'static str> {
    convention.field(field).and_then(|field| field.entry_attribute())
}

/// Where a lowered node is located in a source a host can name, by following the
/// chain of sources back from the node's own span: the source's canonical path
/// and the byte range in it.
///
/// A lowering rule stages its nodes on a source the library made for the
/// occasion, which points back at the node the rule fired at; walking those
/// back-references leads to the document. `None` when the chain reaches no
/// source with a canonical path — content the library made that stands for
/// nothing a document holds, or a fragment a host parsed without naming it.
fn source_position(span: &FlmSpan) -> Option<(String, Range<usize>)> {
    let mut span = span.clone();
    loop {
        if let Some(path) = span.source().origin().canonical_path() {
            return Some((String::from(path), span.range()));
        }
        let triggered = span.source().provenance().triggered_at()?.clone();
        span = triggered;
    }
}

/// The attribute that says where an element is located in its source.
///
/// It is the only attribute of this backend whose value is JSON, so it is the
/// only one written through [`TagBuilder::attr_json`](super::TagBuilder::attr_json),
/// in single quotation marks. The name is spelled out here because every
/// element written is checked against it; a unit test holds it to
/// [`data_attribute`], the function that derives it.
const SOURCE_SPAN_ATTRIBUTE: &str = "data-flm-source-span";

/// The value of the attribute that says where an element is located in its
/// source: a JSON array of three items — the source's name as a JSON
/// string, the first byte of the range, and the byte just past its end.
///
/// The value is JSON so that a script reading it needs no parsing of its own,
/// and the sink writes it between single quotation marks, so the raw page
/// shows it as JSON.
fn source_span_value(source: &str, range: &Range<usize>) -> String {
    let mut value = String::from("[");
    json_string_into(&mut value, source);
    value.push(',');
    value.push_str(&range.start.to_string());
    value.push(',');
    value.push_str(&range.end.to_string());
    value.push(']');
    value
}

/// The rendering model of one cell of a table of cells: what the emitter
/// writes for it, filled from the cell's entry of the payload
/// ([`Emitter::cell_rendering`]) and then written ([`Emitter::cell`]).
///
/// The only place where a cell's facts become HTML, so that how a cell is written
/// can be changed in one place.
// A seam a later step may put behind a trait a host implements; nothing of
// it is public. See decision rationale at [§flm-dd-dr:rend-html-cells].
#[derive(Debug, Clone, PartialEq, Eq)]
struct CellRendering {
    /// `td`, or `th` for a header cell.
    tag: &'static str,
    /// The `scope` of a header cell: `col` for a column header, `row` for a
    /// row header.
    scope: Option<&'static str>,
    /// The number of columns the cell spans, where it is not one.
    colspan: Option<u32>,
    /// The number of rows the cell spans, where it is not one.
    rowspan: Option<u32>,
    /// The cell's other attributes, in the row's order: its alignment, the
    /// edges it has a rule at, one attribute per custom style it carries.
    attributes: Vec<(String, String)>,
}

impl CellRendering {
    /// An empty position of the grid: an empty `<td>`.
    fn empty() -> CellRendering {
        CellRendering { tag: "td", scope: None, colspan: None, rowspan: None, attributes: Vec::new() }
    }
}

impl<'o> Emitter<'o> {
    /// Opens one level of the walk; `false`, with the refusal recorded, past
    /// the depth limit.
    fn descend(&mut self) -> bool {
        if self.depth >= DEPTH_LIMIT {
            self.refused = true;
            return false;
        }
        self.depth += 1;
        true
    }

    /// Closes one level of the walk.
    fn ascend(&mut self) {
        self.depth -= 1;
    }

    /// Records that the tree's HTML requires the definition `identifier`.
    fn require(&mut self, identifier: &str) {
        self.requires.insert(Name::from(identifier));
    }

    /// Reports an internal invariant broken at `node`: an element this backend
    /// neither claims nor expects reached the emitter.
    fn unexpected(&mut self, node: &NodeRef<'_, Flm>) {
        self.diags.push(Diagnostic::error(
            StageFailed {
                stage: String::from(super::stages::EMIT_STAGE),
                detail: format!(
                    "the element `{}` reached the HTML emitter, which cannot write it",
                    element_of(node).name
                ),
            },
            node.span().clone(),
        ));
    }

    // --- writing one element ---------------------------------------------------------------

    /// Opens an element for `node`: its classes, the attributes the row asks
    /// for, and — under the option that asks for them — where the node is
    /// located in its source.
    fn open(
        &mut self,
        node: Option<&NodeRef<'_, Flm>>,
        tag: &str,
        block: bool,
        classes: &[String],
        attributes: Vec<(String, String)>,
        preformatted: bool,
    ) {
        let attributes = self.with_source_span(node, attributes);
        let attributes = self.writable_in_math(node, attributes);
        let with_class = !classes.is_empty() && self.class_writable_in_math(node, classes);
        let classes: Vec<String> = if with_class { classes.to_vec() } else { Vec::new() };
        self.write_open(OpenInMath {
            tag: String::from(tag),
            block,
            classes,
            attributes,
            preformatted,
        });
    }

    /// Opens `element` on the sink and, inside the HTML written into a
    /// formula, records it as open there.
    ///
    /// Every element the emitter opens goes through here, so that a formula
    /// placed deeper inside that HTML can close what is open and open it
    /// again ([`math_text`](Emitter::math_text)). The element is written
    /// exactly as it is given: the checks an element's attributes and classes
    /// pass ([`writable_in_math`](Emitter::writable_in_math)) are the caller's,
    /// and are not made a second time when the same element is opened again.
    fn write_open(&mut self, element: OpenInMath) {
        let mut builder = if element.block {
            self.sink.open_block(&element.tag)
        } else {
            self.sink.open_inline(&element.tag)
        };
        if !element.classes.is_empty() {
            builder = builder.class(&element.classes);
        }
        // The one attribute whose value is JSON is picked out by its name.
        // Both branches escape, each for the quotation mark it writes between,
        // so a name that did not match would still reach the page as a valid
        // attribute; the spelling decides how the value looks, not whether it
        // is safe.
        for (name, value) in &element.attributes {
            builder = if name == SOURCE_SPAN_ATTRIBUTE {
                builder.attr_json(name, value)
            } else {
                builder.attr(name, value)
            };
        }
        if element.preformatted {
            builder = builder.preformatted();
        }
        builder.finish();
        if self.in_tex_html {
            if let Some(piece) = self.piece.as_mut() {
                // An identifier may stand once in a page, so the second half
                // of an element written around a formula carries none; the
                // source position stands once per node too, on the element's
                // first half, so it goes the same way.
                let mut element = element;
                element
                    .attributes
                    .retain(|(name, _)| name != "id" && name != SOURCE_SPAN_ATTRIBUTE);
                piece.open.push(element);
            }
        }
    }

    /// Closes an element opened through [`open`](Emitter::open) or
    /// [`write_open`](Emitter::write_open).
    ///
    /// Inside the HTML written into a formula this is also what takes the
    /// element off the list of those open there, so the two must stay
    /// paired: an element opened through those two is closed through this one
    /// and through no other. A debug assertion holds the pair together —
    /// the element taken off the list must be the one the sink is about to
    /// close — so that an arm closing an element inside a formula by another
    /// route fails a test run instead of quietly reopening the wrong element
    /// around a deeper formula.
    fn close(&mut self) {
        if self.in_tex_html {
            if let Some(piece) = self.piece.as_mut() {
                let popped = piece.open.pop();
                debug_assert_eq!(
                    popped.as_ref().map(|element| element.tag.as_str()),
                    self.sink.open_tag(),
                    "an element inside a formula was opened or closed \
                     without going through the emitter's own pair"
                );
            }
        }
        self.sink.close();
    }

    /// `attributes` with the source-position attribute added when the option
    /// asks for it and `node` is located in a source a host can name.
    ///
    /// Every element written for a presentation node carries it, inline or
    /// block alike; an element that stands for no node of its own is written
    /// with `node` as `None` and carries none.
    fn with_source_span(
        &self,
        node: Option<&NodeRef<'_, Flm>>,
        attributes: Vec<(String, String)>,
    ) -> Vec<(String, String)> {
        let mut attributes = attributes;
        if self.options.source_spans {
            if let Some((source, range)) = node.and_then(|node| source_position(node.span())) {
                attributes.push((
                    String::from(SOURCE_SPAN_ATTRIBUTE),
                    source_span_value(&source, &range),
                ));
            }
        }
        attributes
    }

    /// Writes an element that has no content and is never closed (`<img>`).
    fn void(
        &mut self,
        node: Option<&NodeRef<'_, Flm>>,
        tag: &str,
        classes: &[String],
        attributes: Vec<(String, String)>,
    ) {
        let attributes = self.with_source_span(node, attributes);
        let attributes = self.writable_in_math(node, attributes);
        let with_class = !classes.is_empty() && self.class_writable_in_math(node, classes);
        let mut builder = self.sink.void_inline(tag);
        if with_class {
            builder = builder.class(classes);
        }
        // The one attribute whose value is JSON is picked out by its name,
        // exactly as [`write_open`](Emitter::write_open) picks it out.
        for (name, value) in &attributes {
            builder = if name == SOURCE_SPAN_ATTRIBUTE {
                builder.attr_json(name, value)
            } else {
                builder.attr(name, value)
            };
        }
        builder.finish();
    }

    // --- writing inside a formula -----------------------------------------------------------

    /// `attributes` with every value left out that cannot be written where
    /// this element is being written, which is only ever inside the HTML of a
    /// `<tex-html>`; each one left out is reported.
    ///
    /// Outside a formula every attribute is writable and the list comes back
    /// as it went in.
    fn writable_in_math(
        &mut self,
        node: Option<&NodeRef<'_, Flm>>,
        attributes: Vec<(String, String)>,
    ) -> Vec<(String, String)> {
        if !self.in_tex_html {
            return attributes;
        }
        let Some(sequences) = self.math.as_ref().map(|math| Arc::clone(&math.sequences)) else {
            return attributes;
        };
        let mut written = Vec::with_capacity(attributes.len());
        for (name, value) in attributes {
            match refused_sequence(&value, &sequences) {
                Some(sequence) => self.refused_in_math(node, &name, &sequence),
                None => written.push((name, value)),
            }
        }
        written
    }

    /// Whether the `class` attribute built from `classes` may be written
    /// where this element is being written, reporting it when it may not.
    ///
    /// A class is derived from an element's name or from a role's, and a role
    /// is a name a lowering rule wrote, so a class holding one of the
    /// sequences is a mistake in a feature rather than in a document. The
    /// whole attribute is left out, as any other refused one is.
    fn class_writable_in_math(
        &mut self,
        node: Option<&NodeRef<'_, Flm>>,
        classes: &[String],
    ) -> bool {
        if !self.in_tex_html {
            return true;
        }
        let attribute = vec![(String::from("class"), classes.join(" "))];
        !self.writable_in_math(node, attribute).is_empty()
    }

    /// Reports [`RefusedInMath`] at `node`, or at the formula being written
    /// when the refused element belongs to no node of its own.
    ///
    /// The severity is an Error: what the page shows is not what the document
    /// asked for, even though the element and its content are kept, which is
    /// how this backend reports every refusal of something a document asked
    /// for.
    fn refused_in_math(
        &mut self,
        node: Option<&NodeRef<'_, Flm>>,
        attribute: &str,
        sequence: &str,
    ) {
        let span = match node {
            Some(node) => node.span().clone(),
            None => match self.math.as_ref() {
                Some(math) => math.span.clone(),
                None => nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
            },
        };
        self.diags.push(Diagnostic::error(
            RefusedInMath {
                attribute: String::from(attribute),
                sequence: String::from(sequence),
            },
            span,
        ));
    }

    /// Writes document text through the sink, repaired where it is written inside
    /// the HTML of a `<tex-html>`, the emitter's own inside a passthrough
    /// formula or a `tex-html` construct a rule staged: an empty
    /// `<span></span>` is written wherever a sequence would otherwise end the
    /// formula early ([`break_points`]).
    fn write_text(&mut self, text: &str) {
        let in_math = match (self.in_tex_html, self.math.as_ref()) {
            (true, Some(math)) => Some(Arc::clone(&math.sequences)),
            _ => None,
        };
        let in_construct = (self.tex_html_constructs > 0).then(|| Arc::clone(&self.construct_sequences));
        // The set that applies, shared where only one does; the two merged,
        // the formula's first, where both do.
        let sequences: Arc<[Sequence]> = match (in_math, in_construct) {
            (None, None) => {
                self.sink.text(text);
                return;
            }
            (Some(sequences), None) | (None, Some(sequences)) => sequences,
            (Some(math), Some(construct)) => math.iter().chain(construct.iter()).cloned().collect(),
        };
        let mut at = 0;
        for point in break_points(text, &sequences) {
            self.sink.text(&text[at..point]);
            // The empty element written into a sequence is syntax of the
            // repair, not an element of the page's structure: it opens and
            // closes here, so it never stands open and is never recorded
            // among the elements a deeper formula closes and opens again.
            self.sink.open_inline("span").finish();
            self.sink.close();
            at = point;
        }
        self.sink.text(&text[at..]);
    }

    /// Writes characters into the formula itself — a delimiter the emitter
    /// chose, or a document's own mathematics source — with the tag that puts
    /// HTML inside a formula neutralized ([`neutralized_source`]).
    fn write_math_source(&mut self, source: &str) {
        let neutralized = neutralized_source(source);
        self.sink.text(&neutralized);
    }

    /// The classes of a node: the element's own, then the role's when the row
    /// says the payload carries one and the payload does.
    ///
    /// A role that is not one token is left out and reported: the `class`
    /// attribute separates its classes by whitespace, so a role holding
    /// whitespace would add classes that represent nothing.
    fn classes(
        &mut self,
        node: &NodeRef<'_, Flm>,
        convention: &Convention,
        role: Option<&Name>,
    ) -> Vec<String> {
        let mut classes = vec![class_of_element(convention.element)];
        if convention.has_role() {
            if let Some(role) = role {
                if is_valid_token(role.as_str()) {
                    classes.push(class_of_role(role.as_str()));
                } else {
                    self.refused_name(
                        node,
                        "role",
                        role.as_str(),
                        "the class the role would have added was left out",
                    );
                }
            }
        }
        classes
    }

    /// The names of `kinds` as the whitespace-separated token list a
    /// `data-flm-kind` attribute holds; `None` when nothing is left to write,
    /// in which case the caller writes no attribute at all.
    ///
    /// A name that is not one token is left out and reported, for the reason a
    /// role is ([`classes`](Emitter::classes)). The same walk serves a payload
    /// holding a list of kinds and one holding a single kind, which are written
    /// under the same attribute; a payload holding a single kind therefore
    /// loses the whole attribute when that one name is refused, and the report
    /// says so.
    fn kind_tokens(&mut self, node: &NodeRef<'_, Flm>, kinds: &[Name]) -> Option<String> {
        let mut tokens: Vec<&str> = Vec::new();
        let mut refused: Vec<&str> = Vec::new();
        for kind in kinds {
            if is_valid_token(kind.as_str()) {
                tokens.push(kind.as_str());
            } else {
                refused.push(kind.as_str());
            }
        }
        let written = (!tokens.is_empty()).then(|| tokens.join(" "));
        // What a refusal costs depends on what is left: the attribute holds
        // the names that passed, and is not written at all when none did.
        let recovery = if written.is_some() {
            "the token was left out of the `data-flm-kind` attribute"
        } else {
            "the `data-flm-kind` attribute was not written"
        };
        for name in refused {
            self.refused_name(node, "kind", name, recovery);
        }
        written
    }

    /// Reports `flm.html.invalid-name` for a name the emitter would have
    /// written at `node` and could not.
    fn refused_name(
        &mut self,
        node: &NodeRef<'_, Flm>,
        what: &str,
        name: &str,
        recovery: &str,
    ) {
        self.diags.push(Diagnostic::error(
            InvalidName {
                what: String::from(what),
                name: String::from(name),
                recovery: String::from(recovery),
            },
            node.span().clone(),
        ));
    }

    // --- the walk ---------------------------------------------------------------------------

    /// A run of sibling nodes, in order.
    ///
    /// `carrier` is the presentation node whose content run this is, which is
    /// what a decoration mark resolves its decoration against.
    fn run(&mut self, carrier: Option<&NodeRef<'_, Flm>>, nodes: &[NodeRef<'_, Flm>]) {
        for node in nodes {
            if !self.descend() {
                return;
            }
            self.node(carrier, node);
            self.ascend();
        }
    }

    /// One node.
    fn node(&mut self, carrier: Option<&NodeRef<'_, Flm>>, node: &NodeRef<'_, Flm>) {
        if let Some(text) = node.chars() {
            self.write_text(text);
            return;
        }
        if node.is_comment() {
            // A comment is dropped: an HTML comment would let document text
            // close it, and nothing in the page shows it.
            return;
        }
        if is_element(node, structural::PARBREAK) {
            // A paragraph break separates blocks, and block-level elements
            // separate themselves: nothing is written for it.
            return;
        }
        if !is_presentation(node) {
            // A group, a container, a text node's wrapper: its children, and
            // nothing of its own.
            let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
            self.run(Some(node), &children);
            return;
        }
        let name = element_of(node).name.clone();
        if name == super::construct::TAG {
            self.construct(node);
            return;
        }
        let Some(convention) = row(name.as_str()) else {
            self.unexpected(node);
            return;
        };
        match name.as_str() {
            pres::PAR => self.par(node, convention),
            pres::BLOCK => self.block(node, convention),
            pres::BLOCK_HEADING => self.heading(node, convention),
            pres::RUNIN_HEADING => self.runin_heading(node, convention),
            pres::SPAN => self.span(node, convention),
            pres::LIST => self.list(node, convention),
            pres::CELLS => self.cells(node, convention),
            pres::LINES => self.lines(node, convention),
            pres::LINE => self.line(node, convention),
            pres::VERBATIM => self.verbatim(node, convention),
            pres::MATH_PASSTHROUGH => self.math_passthrough(node, convention),
            pres::MATH_TEXT => self.math_text(node, convention),
            pres::LINK => self.link(node, convention),
            pres::IMAGE => self.image(node, convention),
            pres::UNSUPPORTED => self.unsupported(node),
            pres::DROPPED => self.dropped(node, convention),
            pres::UNKNOWN => self.unknown(node, convention),
            pres::DECO_RUNIN | pres::DECO_FINAL | pres::DECO_MARGIN => {
                self.decoration(node, convention)
            }
            pres::DECO_MARK => self.deco_mark(carrier, node),
            pres::SUBTREE => self.subtree(node),
            _ => self.unexpected(node),
        }
    }

    /// A `flm.pres.par`: a `<p>` around its content; nothing at all for a
    /// paragraph that carries no anchor and whose content shows nothing
    /// ([`shows_nothing`]).
    fn par(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        let payload = payload_of::<ParPayload>(node);
        let anchor = payload.and_then(|payload| payload.anchor.clone());
        let content = body(node);
        if anchor.is_none() && shows_nothing(&content) {
            return;
        }
        self.require_style(convention);
        let mut values: Vec<(&'static str, String)> = Vec::new();
        if let Some(anchor) = anchor {
            values.push(("anchor", anchor));
        }
        let classes = self.classes(node, convention, None);
        self.open(Some(node), "p", true, &classes, ordered_attributes(convention, &values, &[]), false);
        self.run(Some(node), &content);
        self.close();
    }

    /// A `flm.pres.block`: the tag its role maps to ([`block_tag`], a `<div>`
    /// for every role the table does not name) with the role's class, around
    /// its blocks.
    fn block(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        self.require_style(convention);
        let payload = payload_of::<BlockPayload>(node);
        let mut values: Vec<(&'static str, String)> = Vec::new();
        if let Some(anchor) = payload.and_then(|payload| payload.anchor.clone()) {
            values.push(("anchor", anchor));
        }
        if let Some(role) = payload.and_then(|payload| payload.doc_role) {
            values.push(("doc_role", doc_role_value(role)));
        }
        let kinds = payload.map(|payload| payload.kinds.clone()).unwrap_or_default();
        if let Some(kinds) = self.kind_tokens(node, &kinds) {
            values.push(("kinds", kinds));
        }
        let role = payload.and_then(|payload| payload.role.as_ref());
        let tag = block_tag(role.map(Name::as_str));
        let classes = self.classes(node, convention, role);
        self.open(
            Some(node),
            tag,
            true,
            &classes,
            ordered_attributes(convention, &values, &[]),
            false,
        );
        let content = body(node);
        self.run(Some(node), &content);
        self.close();
    }

    /// A `flm.pres.block-heading`: the tag the render options map the
    /// heading kind to ([`HtmlRenderOptions::heading_tags`]), else `<h1>` to
    /// `<h6>` by its level counted from the root heading level
    /// ([`HtmlRenderOptions::root_heading_level`]), with the kind's name, the
    /// true level and whether the heading is in the table of contents in
    /// `data-flm-heading`, `data-flm-level` and `data-flm-toc` regardless of
    /// the tag. A heading above the root is reported regardless of its tag, and one
    /// with no mapped tag is written `<h1>`.
    fn heading(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        self.require_style(convention);
        let payload = payload_of::<HeadingPayload>(node);
        let level = payload.map_or(self.options.root_heading_level, |payload| payload.level);
        let values = heading_values(payload);
        let depth = match heading_depth(level, self.options.root_heading_level) {
            Ok(depth) => depth,
            Err(above) => {
                self.diags.push(Diagnostic::error(above, node.span().clone()));
                0
            }
        };
        // A mapped tag is the host's text, so it is checked before it is
        // written; one that is not writable falls back to the level's tag.
        let mapped = payload.and_then(|payload| self.options.heading_tags.get(&payload.name));
        let tag = match mapped {
            Some(tag) if is_valid_tag_name(tag) => tag.clone(),
            Some(tag) => {
                self.diags.push(Diagnostic::error(
                    InvalidName {
                        what: String::from("tag"),
                        name: tag.clone(),
                        recovery: String::from("the heading was written with the tag of its level"),
                    },
                    node.span().clone(),
                ));
                heading_tag(depth).to_string()
            }
            None => heading_tag(depth).to_string(),
        };
        let classes = self.classes(node, convention, None);
        self.open(
            Some(node),
            &tag,
            true,
            &classes,
            ordered_attributes(convention, &values, &[]),
            false,
        );
        let content = body(node);
        self.run(Some(node), &content);
        self.close();
    }

    /// A `flm.pres.runin-heading`: a `<span>` carrying the same attributes
    /// as a block heading — the anchor as `id`, then `data-flm-heading`,
    /// `data-flm-level` and `data-flm-toc` — around the title. The run-in
    /// decoration it is in writes the stop after it.
    fn runin_heading(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        self.require_style(convention);
        let values = heading_values(payload_of::<HeadingPayload>(node));
        let classes = self.classes(node, convention, None);
        self.open(
            Some(node),
            "span",
            false,
            &classes,
            ordered_attributes(convention, &values, &[]),
            false,
        );
        let content = body(node);
        self.run(Some(node), &content);
        self.close();
    }

    /// A `flm.pres.span`: the element HTML has for its style, or a `<span>`.
    fn span(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        self.require_style(convention);
        let payload = payload_of::<SpanPayload>(node);
        let style = payload.and_then(|payload| payload.style);
        let mut values: Vec<(&'static str, String)> = Vec::new();
        if let Some(anchor) = payload.and_then(|payload| payload.anchor.clone()) {
            values.push(("anchor", anchor));
        }
        if let Some(role) = payload.and_then(|payload| payload.doc_role) {
            values.push(("doc_role", doc_role_value(role)));
        }
        if let Some(style) = style {
            values.push(("style", String::from(style.as_str())));
        }
        let kinds = payload.map(|payload| payload.kinds.clone()).unwrap_or_default();
        if let Some(kinds) = self.kind_tokens(node, &kinds) {
            values.push(("kinds", kinds));
        }
        let classes = self.classes(node, convention, payload.and_then(|payload| payload.role.as_ref()));
        self.open(
            Some(node),
            span_tag(style),
            false,
            &classes,
            ordered_attributes(convention, &values, &[]),
            false,
        );
        let content = body(node);
        self.run(Some(node), &content);
        self.close();
    }

    /// A `flm.pres.list`: `<ol>`, `<ul>` or `<dl>` by kind, one entry each,
    /// each entry's element carrying that entry's anchor as its identifier.
    fn list(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        self.require_style(convention);
        let payload = payload_of::<ListPayload>(node);
        let kinds: Vec<Name> = payload.map(|payload| payload.kinds.clone()).unwrap_or_default();
        let entry_anchors: Vec<Option<String>> =
            payload.map(|payload| payload.entry_anchors.clone()).unwrap_or_default();
        let tag = list_tag(&kinds);
        let mut values: Vec<(&'static str, String)> = Vec::new();
        if let Some(anchor) = payload.and_then(|payload| payload.anchor.clone()) {
            values.push(("anchor", anchor));
        }
        if let Some(tokens) = self.kind_tokens(node, &kinds) {
            values.push(("kinds", tokens));
        }
        let classes = self.classes(node, convention, None);
        self.open(Some(node), tag, true, &classes, ordered_attributes(convention, &values, &[]), false);
        // A description list's only valid children are `<dt>` and `<dd>`, so a
        // label is a `<dt>` carrying the class a rich label carries elsewhere.
        // See decision rationale at [§flm-dd-dr:rend-html-conventions-data].
        let description = tag == "dl";
        for (index, (label, item)) in list_entries(node).into_iter().enumerate() {
            let label: Vec<NodeRef<'_, Flm>> = label.iter().collect();
            let item: Vec<NodeRef<'_, Flm>> = item.iter().collect();
            let label_class = [String::from(LIST_LABEL_CLASS)];
            // An entry's anchor is the identifier of the element that begins
            // the entry: its `<li>`, or a description list's `<dt>`, which is
            // what a link to the entry lands on.
            let id: Vec<(String, String)> =
                match (entry_attribute(convention, "entry_anchors"), entry_anchors.get(index)) {
                    (Some(attribute), Some(Some(anchor))) => {
                        vec![(String::from(attribute), anchor.clone())]
                    }
                    _ => Vec::new(),
                };
            if description {
                self.open(None, "dt", true, &label_class, id, false);
                self.run(Some(node), &label);
                self.close();
                self.open(None, "dd", true, &[], Vec::new(), false);
                self.run(Some(node), &item);
                self.close();
            } else {
                self.open(None, "li", true, &[], id, false);
                if !label.is_empty() {
                    self.open(None, "span", false, &label_class, Vec::new(), false);
                    self.run(Some(node), &label);
                    self.close();
                }
                self.run(Some(node), &item);
                self.close();
            }
        }
        self.close();
    }

    /// A `flm.pres.cells`: a `<table>` holding one `<tr>` per row of the
    /// grid ([`CellsPayload::grid`](pres::CellsPayload::grid)), and in it one
    /// element per position no span covers: a cell at its top-left position,
    /// written from its rendering model ([`CellRendering`]) around its `cell`
    /// slot's blocks, and an empty `<td>` where no cell is placed. Nothing is
    /// written for a position a span covers; the spanning cell's `colspan` and
    /// `rowspan` account for it.
    ///
    /// The table carries the element's class and nothing of the grid: its
    /// size is its shape. No `<thead>` or `<tbody>` is written, HTML implying
    /// the body. A node carrying no payload is an empty table.
    fn cells(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        self.require_style(convention);
        let classes = self.classes(node, convention, None);
        self.open(Some(node), "table", true, &classes, Vec::new(), false);
        if let Some((payload, contents)) = cells_of(node) {
            for row in payload.grid() {
                self.open(None, "tr", true, &[], Vec::new(), false);
                for position in row {
                    match position {
                        GridPosition::Empty => {
                            self.cell(node, convention, &CellRendering::empty(), &[]);
                        }
                        GridPosition::TopLeft(index) => {
                            let rendering = self.cell_rendering(node, convention, &payload.cells[index]);
                            let content: Vec<NodeRef<'_, Flm>> = contents
                                .get(index)
                                .and_then(|slice| slice.as_ref())
                                .map(|slice| slice.iter().collect())
                                .unwrap_or_default();
                            self.cell(node, convention, &rendering, &content);
                        }
                        GridPosition::Covered(_) => {}
                    }
                }
                self.close();
            }
        }
        self.close();
    }

    /// The rendering model of the cell `spec` of the table `node`: its tag,
    /// its `scope`, its spans and its facts as the attributes the row writes
    /// on each cell ([`Written::Entry`]).
    ///
    /// A custom style whose name cannot end an attribute name is left out and
    /// reported as `flm.html.invalid-name`, as a role or a kind is
    /// ([`classes`](Emitter::classes)).
    fn cell_rendering(
        &mut self,
        node: &NodeRef<'_, Flm>,
        convention: &Convention,
        spec: &CellSpec,
    ) -> CellRendering {
        let mut attributes: Vec<(String, String)> = Vec::new();
        if let (Some(attribute), Some(align)) = (entry_attribute(convention, "align"), spec.align) {
            attributes.push((String::from(attribute), String::from(align.as_str())));
        }
        if let Some(attribute) = entry_attribute(convention, "rules").filter(|_| !spec.rules.is_empty()) {
            let edges: Vec<&str> = spec.rules.iter().map(|edge| edge.as_str()).collect();
            attributes.push((String::from(attribute), edges.join(" ")));
        }
        if let Some(family) = entry_attribute(convention, "styles") {
            for style in &spec.styles {
                let attribute = format!("{family}{style}");
                if is_valid_attribute_name(&attribute) {
                    attributes.push((attribute, String::new()));
                } else {
                    self.refused_name(
                        node,
                        "attribute",
                        &attribute,
                        "the attribute of the custom style was left out",
                    );
                }
            }
        }
        CellRendering {
            tag: if spec.header.is_some() { "th" } else { "td" },
            scope: spec.header.map(|header| match header {
                CellHeader::Column => "col",
                CellHeader::Row => "row",
            }),
            colspan: (spec.col_span != 1).then_some(spec.col_span),
            rowspan: (spec.row_span != 1).then_some(spec.row_span),
            attributes,
        }
    }

    /// Writes one cell of the table `node` from its rendering model, around
    /// `content`: the element's own attributes in the row's order, then the
    /// `data-flm-…` ones.
    fn cell(
        &mut self,
        node: &NodeRef<'_, Flm>,
        convention: &Convention,
        rendering: &CellRendering,
        content: &[NodeRef<'_, Flm>],
    ) {
        let mut attributes: Vec<(String, String)> = Vec::new();
        for field in convention.fields {
            let value = match field.field {
                "row_span" => rendering.rowspan.map(|span| span.to_string()),
                "col_span" => rendering.colspan.map(|span| span.to_string()),
                "header" => rendering.scope.map(String::from),
                _ => None,
            };
            if let (Some(attribute), Some(value)) = (field.entry_attribute(), value) {
                attributes.push((String::from(attribute), value));
            }
        }
        attributes.extend(rendering.attributes.iter().cloned());
        self.open(None, rendering.tag, true, &[], attributes, false);
        self.run(Some(node), content);
        self.close();
    }

    /// A `flm.pres.lines`: a `<div>` holding one `<div>` per line.
    fn lines(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        self.require_style(convention);
        let classes = self.classes(node, convention, None);
        self.open(Some(node), "div", true, &classes, Vec::new(), false);
        let content = body(node);
        self.run(Some(node), &content);
        self.close();
    }

    /// A `flm.pres.line`: one `<div>` carrying its alignment, and its
    /// indentation from the left and from the right as the custom properties
    /// of its `style` attribute, a zero indentation writing nothing
    /// ([`ordered_attributes`]).
    fn line(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        self.require_style(convention);
        let payload = payload_of::<LinePayload>(node);
        let mut values: Vec<(&'static str, String)> = Vec::new();
        let mut lengths: Vec<(&'static str, &Length)> = Vec::new();
        if let Some(payload) = payload {
            lengths.push(("indent_left", &payload.indent_left));
            lengths.push(("indent_right", &payload.indent_right));
            values.push(("align", String::from(payload.align.as_str())));
        }
        let classes = self.classes(node, convention, None);
        self.open(
            Some(node),
            "div",
            true,
            &classes,
            ordered_attributes(convention, &values, &lengths),
            false,
        );
        let content = body(node);
        self.run(Some(node), &content);
        self.close();
    }

    /// The characters of a run of verbatim nodes: text runs unchanged, an
    /// interpolated content group's children without its braces, and anything
    /// else as its source spelling.
    ///
    /// Inside a verbatim region a document parse never produces a group, since
    /// `{` is an ordinary character there; a content group found in one is
    /// therefore the wrapper of an argument a template substituted, and its
    /// braces are syntax of the invocation rather than content.
    fn verbatim_text(&mut self, nodes: &[NodeRef<'_, Flm>]) -> String {
        let mut out = String::new();
        for node in nodes {
            match node.kind() {
                NodeKind::Chars { .. } => out.push_str(node.chars().unwrap_or("")),
                NodeKind::Group(_) if node.group_type() == Some(FlmGroupType::Content) => {
                    if self.descend() {
                        let children: Vec<NodeRef<'_, Flm>> = node.children().iter().collect();
                        let text = self.verbatim_text(&children);
                        out.push_str(&text);
                        self.ascend();
                    }
                }
                _ => out.push_str(&recompose_source(&[*node])),
            }
        }
        out
    }

    /// A `flm.pres.verbatim`: `<code>` inline, `<pre><code>` as a block.
    fn verbatim(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        let payload = payload_of::<VerbatimPayload>(node);
        let block =
            payload.is_some_and(|payload| payload.display == VerbatimDisplay::Block);
        self.require_style(convention);
        let mut values: Vec<(&'static str, String)> = Vec::new();
        if let Some(payload) = payload {
            values.push(("kind", String::from(payload.kind.as_str())));
            values.push(("display", String::from(payload.display.as_str())));
            if let Some(lang) = payload.lang.as_deref() {
                values.push(("lang", String::from(lang)));
            }
        }
        let classes = self.classes(node, convention, None);
        let attributes = ordered_attributes(convention, &values, &[]);
        let content = self.verbatim_text(&body(node));
        if block {
            self.open(Some(node), "pre", true, &classes, attributes, true);
            self.open(None, "code", false, &[], Vec::new(), false);
            self.write_text(&content);
            self.close();
            self.close();
        } else {
            self.open(Some(node), "code", false, &classes, attributes, false);
            self.write_text(&content);
            self.close();
        }
    }

    /// A `flm.pres.math-passthrough`: one formula of passthrough mathematics,
    /// written whole as the LaTeX a formula renderer in the reader's browser
    /// reads.
    ///
    /// The element is a `<span>` for every formula, inline and displayed
    /// alike, carrying the class, the display (`data-flm-display`, which tells
    /// the two apart) and the environment's name, and no identifier of its
    /// own. A displayed formula is opened as block-level, so that line breaks
    /// surround it; the default stylesheet gives it the appearance of a block.
    /// A `<div>` is not written because HTML does not allow a block element
    /// inside a paragraph, and a displayed formula is an inline node of its
    /// paragraph's text. The element holds the delimiters the emitter chose and
    /// the formula's own source between them, written as text, with the content
    /// inside the formula written as HTML at its position
    /// ([`math_text`](Emitter::math_text)).
    ///
    /// A formula reached **inside the HTML written into another formula** is
    /// never written there, because the renderer does not read the HTML it
    /// inserts a second time; its source goes into the surrounding formula
    /// instead ([`math_text`](Emitter::math_text) describes how).
    fn math_passthrough(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        if self.math.is_some() {
            self.formula_deeper_in_a_piece(node);
            return;
        }
        let payload = payload_of::<MathPassthroughPayload>(node);
        let display = payload.map_or(MathDisplay::Inline, |payload| payload.display);
        let env: Option<String> = payload.and_then(|payload| payload.env.clone());
        let block = display == MathDisplay::Display;
        self.require_style(convention);
        // The one definition the emitter requires that no row names: the
        // script that reads the LaTeX written here.
        self.require(SCRIPT_MATHJAX);
        let mut values: Vec<(&'static str, String)> =
            vec![("display", String::from(display.as_str()))];
        if let Some(env) = &env {
            values.push(("env", env.clone()));
        }
        let classes = self.classes(node, convention, None);
        // The tag is a `<span>` for a displayed formula too, although the
        // element is opened as block-level: see the rustdoc above.
        self.open(
            Some(node),
            "span",
            block,
            &classes,
            ordered_attributes(convention, &values, &[]),
            false,
        );
        let closing = closing_delimiter(display, env.as_deref());
        self.math = Some(MathContext {
            sequences: Arc::from(sequences_in_math(&closing)),
            span: node.span().clone(),
        });
        let opening = opening_delimiter(display, env.as_deref());
        self.write_math_source(&opening);
        self.math_stream(node);
        let closing = closing.text.clone();
        self.write_math_source(&closing);
        self.math = None;
        self.close();
    }

    /// The nodes of a formula's body, in order: the source unchanged, written
    /// as text, and each `flm.pres.math-text` node among it written as HTML at
    /// its position.
    ///
    /// A formula nested in content inside a formula is written the same way:
    /// its source alone, into the formula around it, with no delimiters of its
    /// own, since its characters are already inside mathematics.
    fn math_stream(&mut self, node: &NodeRef<'_, Flm>) {
        let body = body(node);
        let mut source: Vec<NodeRef<'_, Flm>> = Vec::new();
        for child in &body {
            if !is_element(child, pres::MATH_TEXT) {
                source.push(*child);
                continue;
            }
            let text = recompose_source(&source);
            self.write_math_source(&text);
            source.clear();
            if !self.descend() {
                return;
            }
            if let Some(convention) = row(pres::MATH_TEXT) {
                self.math_text(child, convention);
            }
            self.ascend();
        }
        let text = recompose_source(&source);
        self.write_math_source(&text);
    }

    /// A `flm.pres.math-text`: content inside a formula, written as HTML
    /// inside the formula's LaTeX.
    ///
    /// Inside a formula the content is written as
    /// `<tex-html><span class="flm-pres-math--text">…</span></tex-html>`. The
    /// `<tex-html>` element is the renderer's way of putting HTML inside a
    /// formula, and the renderer takes exactly one element out of it, so the
    /// attributes are on the `<span>` inside and never on the `<tex-html>`
    /// itself. A node with an anchor and no content is that same `<span>`,
    /// empty: a point in the formula that a link can lead to, which marks the
    /// start of an equation line. The
    /// [stylesheet](super::conventions::STYLE_MATH_PASSTHROUGH) keeps an empty one at zero
    /// size.
    ///
    /// A formula among the content's own nodes **splits** the content: the
    /// part being written (one `<tex-html>`) is closed, the inner formula's
    /// source goes into the surrounding formula with no delimiters of its own,
    /// and what follows it opens a new part. The identifier, the command and
    /// the node's source position are on the first part only, since each
    /// appears once per node; the class is on each part.
    ///
    /// A formula deeper inside the content (inside a styled span inside it, as
    /// `\text{\emph{\(x\)}}` gives) is handled by closing and opening again
    /// ([`formula_deeper_in_a_piece`](Emitter::formula_deeper_in_a_piece)).
    /// Nothing is lost and nothing is reported.
    ///
    /// Outside a formula (a hand-built tree, or a rule that staged the node
    /// elsewhere), the node is the `<span>` alone, and its content is ordinary
    /// content.
    fn math_text(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        self.require_style(convention);
        let payload = payload_of::<MathTextPayload>(node);
        let content = body(node);
        if self.math.is_none() {
            let values = self.math_text_values(payload);
            let classes = self.classes(node, convention, None);
            self.open(
                Some(node),
                "span",
                false,
                &classes,
                ordered_attributes(convention, &values, &[]),
                false,
            );
            self.run(Some(node), &content);
            self.close();
            return;
        }
        // Content standing where a line's tag stands is written between a
        // pair of dollar signs, which is this backend's own: the formula's
        // source carries the content at that place, and the dollar signs are
        // what this backend writes around it there. Every other position is
        // written without them. They stand once around the whole node, not
        // around each piece, so that a formula inside the content keeps the
        // one pair it stands in.
        let at_tag = payload.is_some_and(|payload| payload.at_tag);
        if at_tag {
            self.write_math_source("$");
        }
        let mut written = false;
        let mut piece: Vec<NodeRef<'_, Flm>> = Vec::new();
        for child in &content {
            if !is_element(child, pres::MATH_PASSTHROUGH) {
                piece.push(*child);
                continue;
            }
            if !written || !piece.is_empty() {
                self.tex_html(node, convention, payload, !written, &piece);
                written = true;
            }
            piece.clear();
            if self.descend() {
                self.math_stream(child);
                self.ascend();
            }
        }
        if !written || !piece.is_empty() {
            self.tex_html(node, convention, payload, !written, &piece);
        }
        if at_tag {
            self.write_math_source("$");
        }
    }

    /// The attributes a `flm.pres.math-text` node's payload asks for: the
    /// anchor as the identifier, and the command it was written in.
    fn math_text_values(
        &self,
        payload: Option<&MathTextPayload>,
    ) -> Vec<(&'static str, String)> {
        let mut values: Vec<(&'static str, String)> = Vec::new();
        if let Some(anchor) = payload.and_then(|payload| payload.anchor.clone()) {
            values.push(("anchor", anchor));
        }
        if let Some(command) = payload.and_then(|payload| payload.command.clone()) {
            values.push(("command", command));
        }
        values
    }

    /// One part of a `flm.pres.math-text` node's content, written as HTML
    /// inside the formula: a `<tex-html>` holding one `<span>` holding
    /// `content`.
    ///
    /// `first` says that this is the node's first part, the one that carries
    /// the identifier, the command and the node's source position: each of
    /// those appears once per node, and the later parts carry the class alone.
    /// Only the first part is therefore written for the node itself; a later
    /// part is written for no node of its own, so a class refused inside the
    /// formula is reported there at the surrounding formula, the first part
    /// having already reported it at the node.
    fn tex_html(
        &mut self,
        node: &NodeRef<'_, Flm>,
        convention: &Convention,
        payload: Option<&MathTextPayload>,
        first: bool,
        content: &[NodeRef<'_, Flm>],
    ) {
        let values = if first { self.math_text_values(payload) } else { Vec::new() };
        let classes = self.classes(node, convention, None);
        // The piece this one stands inside, when a formula inside this
        // content holds content of its own: each `<tex-html>` keeps its own
        // list of what is open in it.
        let outer = self.piece.take();
        let was_in_tex_html = self.in_tex_html;
        self.sink.open_inline(TEX_HTML).finish();
        self.in_tex_html = true;
        self.open(
            first.then_some(node),
            "span",
            false,
            &classes,
            ordered_attributes(convention, &values, &[]),
            false,
        );
        self.piece = Some(Piece { classes, open: Vec::new() });
        self.run(Some(node), content);
        self.piece = None;
        self.sink.close();
        self.in_tex_html = was_in_tex_html;
        self.sink.close();
        self.piece = outer;
    }

    /// A formula placed deeper inside the HTML of a part (one `<tex-html>`)
    /// than the content's own nodes, written by closing what is open and
    /// opening it again.
    ///
    /// The renderer never reads the HTML it inserts a second time, so a
    /// formula written inside that HTML would reach the reader as characters;
    /// its source must go into the surrounding formula's own characters. Where
    /// the formula is among the content's own nodes, that is a boundary
    /// between parts ([`math_text`](Emitter::math_text)); deeper, every element
    /// open inside the part (a styled span, a link, a tag a lowering rule
    /// staged) is closed first and opened again afterwards, so that the content
    /// around the formula keeps the elements it had. Each one is written again
    /// with the classes and attributes it carried, without its identifier and
    /// without its source position: an identifier may appear once in a page
    /// and a node's position once on the node's own element, so both stay on
    /// the element's first half. The part is opened again as a later part,
    /// carrying the class alone.
    fn formula_deeper_in_a_piece(&mut self, node: &NodeRef<'_, Flm>) {
        let Some(piece) = self.piece.take() else {
            // A formula inside a formula stands, by construction, inside a
            // piece: nothing else writes HTML into a formula.
            self.unexpected(node);
            return;
        };
        for _ in &piece.open {
            self.sink.close();
        }
        self.sink.close();
        self.in_tex_html = false;
        self.sink.close();
        self.math_stream(node);
        self.sink.open_inline(TEX_HTML).finish();
        self.in_tex_html = true;
        self.write_open(OpenInMath {
            tag: String::from("span"),
            block: false,
            classes: piece.classes.clone(),
            attributes: Vec::new(),
            preformatted: false,
        });
        self.piece = Some(Piece { classes: piece.classes, open: Vec::new() });
        for element in piece.open {
            self.write_open(element);
        }
    }

    /// A `flm.pres.link`: an `<a>` whose address passed the policy.
    ///
    /// A link may be inside the content of another link: a term link in the
    /// text of an `\href`, a `\ref` in the text of a `\hyperref`, an xnote's
    /// mark in a heading's title that a reference shows. HTML does not allow
    /// an `<a>` inside another (a browser closes the outer one early), so the
    /// inner link is written as a `<span>` carrying the same classes
    /// and kind, and no address, document role or accessible name, since it
    /// leads nowhere; the outer one, which the author wrote around the whole
    /// text, is the one a reader follows. Its content is written as usual.
    /// Nothing is reported: this is the page's way of writing such a link.
    // See decision rationale at [§flm-dd-dr:rend-html-nested-link].
    fn link(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        self.require_style(convention);
        let nested = self.links_open > 0;
        let payload = payload_of::<LinkPayload>(node);
        let mut values: Vec<(&'static str, String)> = Vec::new();
        if let Some(payload) = payload {
            if !nested {
                match check_address(&payload.href, AddressUse::Link) {
                    Ok(()) => values.push(("href", payload.href.clone())),
                    Err(refused) => self.diags.push(Diagnostic::error(
                        AddressRefused {
                            address: payload.href.clone(),
                            scheme: refused.scheme,
                            attribute: String::from("href"),
                        },
                        node.span().clone(),
                    )),
                }
                if let Some(role) = payload.doc_role {
                    values.push(("doc_role", doc_role_value(role)));
                }
                if let Some(label) = &payload.label {
                    values.push(("label", label.clone()));
                }
            }
            // A link's kind is a single name written under the same attribute
            // a list of kinds is written under, so it passes the same check.
            if let Some(kind) = self.kind_tokens(node, std::slice::from_ref(&payload.kind)) {
                values.push(("kind", kind));
            }
        }
        let classes = self.classes(node, convention, None);
        self.open(
            Some(node),
            if nested { "span" } else { "a" },
            false,
            &classes,
            ordered_attributes(convention, &values, &[]),
            false,
        );
        let content = body(node);
        self.links_open += 1;
        self.run(Some(node), &content);
        self.links_open -= 1;
        self.close();
    }

    /// A `flm.pres.image`: an `<img>` whose sources passed the policy; a
    /// block image carries the class of a block before its own
    /// ([`Written::BlockClass`]).
    fn image(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        self.require_style(convention);
        let payload = payload_of::<ImagePayload>(node);
        let mut values: Vec<(&'static str, String)> = Vec::new();
        if let Some(payload) = payload {
            if let Some(anchor) = payload.anchor.clone() {
                values.push(("anchor", anchor));
            }
            match check_address(&payload.src, AddressUse::Image) {
                Ok(()) => values.push(("src", payload.src.clone())),
                Err(refused) => self.diags.push(Diagnostic::error(
                    AddressRefused {
                        address: payload.src.clone(),
                        scheme: refused.scheme,
                        attribute: String::from("src"),
                    },
                    node.span().clone(),
                )),
            }
            if let Some(alt) = payload.alt.clone() {
                values.push(("alt", alt));
            }
            let mut candidates: Vec<String> = Vec::new();
            for candidate in &payload.srcset {
                match check_address(&candidate.src, AddressUse::Image) {
                    Ok(()) => candidates.push(
                        if candidate.descriptor.is_empty() {
                            candidate.src.clone()
                        } else {
                            format!("{} {}", candidate.src, candidate.descriptor)
                        },
                    ),
                    Err(refused) => self.diags.push(Diagnostic::error(
                        AddressRefused {
                            address: candidate.src.clone(),
                            scheme: refused.scheme,
                            attribute: String::from("srcset"),
                        },
                        node.span().clone(),
                    )),
                }
            }
            if !candidates.is_empty() {
                values.push(("srcset", candidates.join(", ")));
            }
        }
        let mut lengths: Vec<(&'static str, &Length)> = Vec::new();
        if let Some(payload) = payload {
            lengths.extend(payload.width.as_ref().map(|width| ("width", width)));
            lengths.extend(payload.height.as_ref().map(|height| ("height", height)));
        }
        let mut classes = self.classes(node, convention, None);
        if payload.is_some_and(|payload| payload.display == ImageDisplay::Block) {
            classes.insert(0, class_of_element(pres::BLOCK));
        }
        self.void(Some(node), "img", &classes, ordered_attributes(convention, &values, &lengths));
    }

    /// A `flm.pres.unsupported`: an empty span naming the element, which the
    /// stylesheet shows as an error.
    fn unsupported(&mut self, node: &NodeRef<'_, Flm>) {
        let element = payload_of::<UnsupportedPayload>(node)
            .map_or_else(String::new, |payload| payload.element.to_string());
        self.stand_in(node, &element);
    }

    /// A `flm.pres.dropped`: a `<span>` carrying the condition of the
    /// diagnostic reported for what was left out, and holding `dropped: `
    /// followed by the payload's detail as its text, so that the mark shows
    /// on a page without a stylesheet. The row's style definition shows it as
    /// an error.
    fn dropped(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        self.require_style(convention);
        let payload = payload_of::<DroppedPayload>(node);
        let mut values: Vec<(&'static str, String)> = Vec::new();
        if let Some(payload) = payload {
            values.push(("condition", payload.condition.to_string()));
        }
        let classes = self.classes(node, convention, None);
        self.open(Some(node), "span", false, &classes, ordered_attributes(convention, &values, &[]), false);
        let detail = payload.map_or("", |payload| payload.detail.as_str());
        self.write_text(&format!("dropped: {detail}"));
        self.close();
    }

    /// The stand-in for something this backend cannot write: an empty span
    /// naming the element.
    ///
    /// `node` is the node the stand-in is written for, and the stand-in is the
    /// one element written for it, so it carries that node's position in its
    /// source under the option that asks for positions. That holds of the node
    /// whose element this backend cannot write and of a staged tag the emitter
    /// refuses to write: in the second case no staged tag reaches the page at
    /// all, so what the page holds there is the emitter's own stand-in — carrying the
    /// emitter's own class and its own attribute — and not the tag the rule
    /// asked for.
    fn stand_in(&mut self, node: &NodeRef<'_, Flm>, element: &str) {
        let convention = row(pres::UNSUPPORTED).expect("the vocabulary has a row for every element");
        self.require_style(convention);
        let values = vec![("element", String::from(element))];
        // The stand-in stands for a node whose own row it is not, so it takes
        // the class of the element it is — and no role, which is why it does
        // not go through `classes`, the one place a role is checked.
        let classes = vec![class_of_element(convention.element)];
        self.open(
            Some(node),
            "span",
            false,
            &classes,
            ordered_attributes(convention, &values, &[]),
            false,
        );
        self.close();
    }

    /// A `flm.pres.unknown`: what the author wrote, shown as the error it is.
    fn unknown(&mut self, node: &NodeRef<'_, Flm>, convention: &Convention) {
        let payload = payload_of::<UnknownPayload>(node);
        let kind = payload.map_or(UnknownKind::Macro, |payload| payload.kind);
        self.require_style(convention);
        let mut values: Vec<(&'static str, String)> = Vec::new();
        if let Some(payload) = payload {
            values.push(("kind", String::from(payload.kind.as_str())));
            values.push(("name", payload.name.clone()));
        }
        let classes = self.classes(node, convention, None);
        let attributes = ordered_attributes(convention, &values, &[]);
        let spelling = payload.map_or_else(String::new, |payload| payload.spelling.clone());
        match kind {
            UnknownKind::Macro => {
                self.open(Some(node), "span", false, &classes, attributes, false);
                self.write_text(&spelling);
                self.close();
            }
            UnknownKind::Environment => {
                self.open(Some(node), "div", true, &classes, attributes, false);
                self.marker(&format!("\\begin{{{spelling}}}"));
                let content = body(node);
                self.run(Some(node), &content);
                self.marker(&format!("\\end{{{spelling}}}"));
                self.close();
            }
        }
    }

    /// One marker line of an unattributed environment.
    fn marker(&mut self, text: &str) {
        let classes = [String::from(UNKNOWN_MARKER_CLASS)];
        self.open(None, "span", true, &classes, Vec::new(), false);
        self.write_text(text);
        self.close();
    }

    /// A decoration, written as a `<span>` at its position: at its decoration
    /// mark, or, in a tree no lowering driver built, where the decoration itself
    /// is in the run.
    ///
    /// A margin decoration is written like the other two kinds, inside the
    /// flow of the text at the position its mark keeps — inside a paragraph,
    /// inside a heading's title, at the start of the block that carries it —
    /// and the stylesheet sets it beside the text. It is a `<span>` because
    /// HTML allows a `<span>` at every one of those places, while an `<aside>`
    /// inside a `<p>` is not valid HTML.
    ///
    /// A run-in decoration — the title of a heading set at the start of the
    /// paragraph it introduces — takes a stop after its title, inside the same
    /// span: the render option
    /// [`HtmlRenderOptions::run_in_stop`], written as ordinary escaped text,
    /// unless the option is empty, the title is blank ([`holds_text`]) or the
    /// title already ends in sentence punctuation
    /// ([`ends_in_sentence_punctuation`]). Both tests are the shared ones over
    /// the content tree, which this emitter still holds when it reaches the
    /// decoration; nothing here reads back what was written.
    fn decoration(
        &mut self,
        node: &NodeRef<'_, Flm>,
        convention: &Convention,
    ) {
        self.require_style(convention);
        let classes = self.classes(node, convention, None);
        self.open(Some(node), "span", false, &classes, Vec::new(), false);
        let content = body(node);
        self.run(Some(node), &content);
        if is_element(node, pres::DECO_RUNIN)
            && !self.options.run_in_stop.is_empty()
            && holds_text(content.iter().copied())
            && !ends_in_sentence_punctuation(content.iter().copied())
        {
            let stop = self.options.run_in_stop.clone();
            self.write_text(&stop);
        }
        self.close();
    }

    /// A `flm.pres.deco-mark`: the decoration it stands for, at its position,
    /// regardless of its kind. A mark whose decoration cannot be resolved — a tree
    /// no lowering driver built — writes nothing.
    fn deco_mark(&mut self, carrier: Option<&NodeRef<'_, Flm>>, node: &NodeRef<'_, Flm>) {
        let Some(carrier) = carrier else { return };
        let Some(deco) = decoration_at(carrier, node) else { return };
        let Some(convention) = row(element_of(&deco).name.as_str()) else { return };
        self.decoration(&deco, convention);
    }

    /// A `flm.pres.subtree`: the segment being written ends here, and the
    /// placed tree is recorded for the regions stage to write in its place.
    fn subtree(&mut self, node: &NodeRef<'_, Flm>) {
        let Some(payload) = payload_of::<SubtreePayload>(node) else { return };
        // A count of the bytes written, never the bytes: the sink stays
        // unreadable, and the count is what lets the page assembly write the
        // placed tree where it belongs.
        // See decision rationale at [§flm-dd-dr:rend-html-page-skeleton].
        let at = self.sink.len();
        self.placements.push((at, payload.tree, node.span().clone()));
    }

    /// A `flm.pres.html.tag`: the tag a lowering rule staged, written as its
    /// payload states it.
    ///
    /// The HTML contract does not apply to it (the emitter adds no class and
    /// no attribute of its own), and neither does the address policy: the rule
    /// that staged the tag is responsible for what it writes. The names are
    /// checked, because a name is written unchanged and one that is not
    /// writable would produce markup the rule did not ask for.
    ///
    /// Inside the HTML written into a formula the attribute *values* are
    /// checked as well, exactly as every other attribute written there is
    /// ([`writable_in_math`](Emitter::writable_in_math)): a value holding the
    /// closing delimiter of the surrounding formula would end that formula
    /// early, and only the emitter knows which delimiter it wrote. A body given
    /// as [`Markup`] is written unchanged, there as anywhere else: the code
    /// that staged the node created those bytes and is responsible for them
    /// ([`HtmlTagPayload`] says what that means inside a formula).
    ///
    /// The text of the children of a tag named `tex-html` is repaired as the
    /// text inside a formula's `<tex-html>` is ([`Emitter::write_text`],
    /// [`sequences_in_construct`]): MathJax reads that element's characters
    /// whoever staged it.
    fn construct(&mut self, node: &NodeRef<'_, Flm>) {
        let Some(payload) = payload_of::<HtmlTagPayload>(node) else {
            self.unexpected(node);
            return;
        };
        if !is_valid_tag_name(&payload.tag) {
            self.diags.push(Diagnostic::error(
                InvalidName {
                    what: String::from("tag"),
                    name: payload.tag.clone(),
                    recovery: String::from("the node was written as the unsupported stand-in"),
                },
                node.span().clone(),
            ));
            self.stand_in(node, super::construct::TAG);
            return;
        }
        for (name, _) in &payload.attributes {
            if !is_valid_attribute_name(name) {
                self.diags.push(Diagnostic::error(
                    InvalidName {
                        what: String::from("attribute"),
                        name: name.clone(),
                        recovery: String::from(
                            "the node was written as the unsupported stand-in",
                        ),
                    },
                    node.span().clone(),
                ));
                self.stand_in(node, super::construct::TAG);
                return;
            }
        }
        let attributes: Vec<(String, String)> = payload
            .attributes
            .iter()
            .map(|(name, value)| (name.clone(), value.clone()))
            .collect();
        let attributes = self.writable_in_math(Some(node), attributes);
        self.write_open(OpenInMath {
            tag: payload.tag.clone(),
            block: payload.block,
            classes: Vec::new(),
            attributes,
            preformatted: false,
        });
        match &payload.body {
            TagBody::Markup(markup) => self.sink.markup(markup),
            TagBody::Children => {
                let content = body(node);
                let tex_html = payload.tag == TEX_HTML;
                self.tex_html_constructs += usize::from(tex_html);
                self.run(Some(node), &content);
                self.tex_html_constructs -= usize::from(tex_html);
            }
        }
        self.close();
    }

    /// Records the style definition the row gives as a requirement, when it
    /// gives one.
    fn require_style(&mut self, convention: &Convention) {
        if let Some(style) = convention.style {
            self.require(style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The sequences that stand around inline mathematics: `\)` and the two
    /// tag spellings.
    fn inline_sequences() -> Vec<Sequence> {
        sequences_in_math(&closing_delimiter(MathDisplay::Inline, None))
    }

    /// `text` with an empty element written at every break point, spelled the
    /// way the emitter writes it.
    fn broken(text: &str, sequences: &[Sequence]) -> String {
        let mut out = String::new();
        let mut at = 0;
        for point in break_points(text, sequences) {
            out.push_str(&text[at..point]);
            out.push_str("<span></span>");
            at = point;
        }
        out.push_str(&text[at..]);
        out
    }

    #[test]
    fn the_style_writer_writes_nothing_for_a_zero_length() {
        let line = row(pres::LINE).expect("the line row");
        let zero = Length::tabs(0);
        let two = Length::tabs(2);
        let align = [("align", String::from("left"))];
        let attributes =
            ordered_attributes(line, &align, &[("indent_left", &zero), ("indent_right", &zero)]);
        assert!(attributes.iter().all(|(name, _)| name != STYLE_ATTRIBUTE), "{attributes:?}");
        let attributes =
            ordered_attributes(line, &align, &[("indent_left", &two), ("indent_right", &zero)]);
        assert_eq!(
            attributes.iter().find(|(name, _)| name == STYLE_ATTRIBUTE).map(|(_, value)| value.as_str()),
            Some("--flm-element-indent-left: calc(2 * var(--flm-tab))")
        );
    }

    #[test]
    fn the_source_span_attribute_is_the_name_the_naming_function_derives() {
        assert_eq!(SOURCE_SPAN_ATTRIBUTE, data_attribute("source_span"));
        assert!(is_valid_attribute_name(SOURCE_SPAN_ATTRIBUTE));
    }

    #[test]
    fn the_value_of_the_source_span_attribute_is_a_json_array_of_three_items() {
        assert_eq!(source_span_value("book/ch1.flm", &(0..8)), "[\"book/ch1.flm\",0,8]");
    }

    #[test]
    fn the_delimiters_are_the_three_shapes_a_formula_is_written_in() {
        assert_eq!(opening_delimiter(MathDisplay::Inline, None), "\\(");
        assert_eq!(closing_delimiter(MathDisplay::Inline, None).text, "\\)");
        assert_eq!(opening_delimiter(MathDisplay::Display, None), "\\[");
        assert_eq!(closing_delimiter(MathDisplay::Display, None).text, "\\]");
        assert_eq!(
            opening_delimiter(MathDisplay::Display, Some("align")),
            "\\begin{align}"
        );
        assert_eq!(
            closing_delimiter(MathDisplay::Display, Some("align")).text,
            "\\end{align}"
        );
        // The environment's name decides the closing sequence, so a formula in
        // one environment is not cut short by another's end.
        assert_eq!(closing_delimiter(MathDisplay::Display, Some("gather")).text, "\\end{gather}");
    }

    #[test]
    fn source_holding_the_tag_gets_an_empty_group_after_its_opening_angle() {
        assert_eq!(neutralized_source("a < b"), "a < b");
        assert_eq!(neutralized_source("x <tex-html> y"), "x <{}tex-html> y");
        assert_eq!(neutralized_source("x </tex-html> y"), "x <{}/tex-html> y");
        // Both spellings in one run, and the substitution of one never builds
        // the other.
        assert_eq!(
            neutralized_source("<tex-html><b>p</b></tex-html>"),
            "<{}tex-html><b>p</b><{}/tex-html>"
        );
        // A `<` that begins neither spelling is left alone, and so is a
        // spelling that is only begun.
        assert_eq!(neutralized_source("<tex-htm"), "<tex-htm");
        assert_eq!(neutralized_source("a < tex-html"), "a < tex-html");
    }

    #[test]
    fn source_that_holds_no_tag_is_unchanged_whatever_it_holds() {
        for source in ["", "x^2 + y", "a \\le b", "\\text{if}", "α < β", "&\\\\"] {
            assert_eq!(neutralized_source(source), source, "{source}");
        }
    }

    #[test]
    fn text_inside_the_html_breaks_the_closing_delimiter_and_the_two_tags() {
        let sequences = inline_sequences();
        assert_eq!(broken("write \\) here", &sequences), "write \\<span></span>) here");
        assert_eq!(
            broken("write <tex-html> here", &sequences),
            "write <<span></span>tex-html> here"
        );
        assert_eq!(
            broken("write </tex-html> here", &sequences),
            "write <<span></span>/tex-html> here"
        );
        // The delimiter of another formula is not this formula's end and is
        // left alone.
        assert_eq!(broken("write \\] here", &sequences), "write \\] here");
    }

    #[test]
    fn the_environment_name_is_part_of_the_sequence_that_is_broken() {
        let sequences = sequences_in_math(&closing_delimiter(MathDisplay::Display, Some("align")));
        assert_eq!(
            broken("stop at \\end{align} here", &sequences),
            "stop at \\end<span></span>{align} here"
        );
        assert_eq!(broken("\\end{gather} is not this end", &sequences), "\\end{gather} is not this end");
    }

    #[test]
    fn a_text_that_holds_no_sequence_is_written_as_it_stands() {
        let sequences = inline_sequences();
        for text in ["", "one half", "a < b", "\\alpha", "( ) \\ ]"] {
            assert_eq!(broken(text, &sequences), text, "{text}");
            assert!(break_points(text, &sequences).is_empty(), "{text}");
        }
    }

    #[test]
    fn every_occurrence_of_a_sequence_is_broken_including_two_in_a_row() {
        let sequences = inline_sequences();
        assert_eq!(broken("\\)\\)", &sequences), "\\<span></span>)\\<span></span>)");
        assert_eq!(break_points("\\)\\)", &sequences), vec![1, 3]);
        // A backslash before the sequence does not hide it: the renderer
        // searches the characters and would find the delimiter at the second
        // backslash.
        assert_eq!(broken("\\\\)", &sequences), "\\\\<span></span>)");
    }

    #[test]
    fn an_attribute_value_is_refused_for_the_first_sequence_it_holds() {
        let sequences = inline_sequences();
        assert_eq!(refused_sequence("eq:one", &sequences), None);
        assert_eq!(refused_sequence("eq:\\)one", &sequences).as_deref(), Some("\\)"));
        assert_eq!(
            refused_sequence("a </tex-html> b", &sequences).as_deref(),
            Some("</tex-html")
        );
        // The earliest occurrence names the refusal, whichever sequence it is.
        assert_eq!(
            refused_sequence("<tex-html \\)", &sequences).as_deref(),
            Some("<tex-html")
        );
        assert_eq!(
            refused_sequence("\\) <tex-html", &sequences).as_deref(),
            Some("\\)")
        );
    }

    #[test]
    fn a_break_point_falls_at_the_sequences_own_joint() {
        // The three shapes of a closing delimiter, and the two tag spellings,
        // each broken where the sequence joins: this is what the goldens hold.
        assert_eq!(closing_delimiter(MathDisplay::Inline, None).break_at, 1);
        assert_eq!(closing_delimiter(MathDisplay::Display, None).break_at, 1);
        assert_eq!(closing_delimiter(MathDisplay::Display, Some("align")).break_at, 4);
        for sequence in tex_html_sequences() {
            assert_eq!(sequence.break_at, 1, "{}", sequence.text);
        }
    }
}
