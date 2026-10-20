//! Reading presentation nodes: the functions a backend's emitter reads a
//! lowered tree with.
//!
//! A presentation node is a callable node of the presentation form
//! ([`is_presentation`]); its typed payload is read with [`payload_of`]. Its
//! children are divided, in order, among its slots:
//!
//! - **Content slots** come first: on most elements a single slot named `body`
//!   and marked as the body ([`body_of`]); on a list node, a `label` slot and
//!   an `item` slot per entry, read by position because the two names repeat
//!   ([`list_entries`]); on a table of cells, one `cell` slot per entry of its
//!   payload ([`cells_of`]).
//! - **Attached slots** follow: one per decoration the lowering driver moved
//!   out of the node's content slots, named after the decoration's element
//!   ([`attached_slots`]). A decoration is a node whose element carries the
//!   [`Decoration`](super::Decoration) aspect; the decoration mark
//!   (`flm.pres.deco-mark`) left at its former position gives the index of
//!   its attached slot, and [`decoration_at`] returns the decoration a mark
//!   refers to. See [decorations, roles and
//!   kinds](crate::guide::concepts_overview#decorations-roles-and-kinds).
//!
//! Two functions read the text of a run of nodes, for different purposes:
//! [`plain_text`] returns the characters of the run with every construct
//! left out (what a heading is measured by and a list label is padded from),
//! and [`recompose_source`] returns the run's source text, byte for byte
//! (what an emitter writes for the opaque content of a math or verbatim
//! node). [`spelling_identity`] builds on the second to decide whether two
//! written runs are the same.
//!
//! [`holds_text`] and [`ends_in_sentence_punctuation`] are the two tests every
//! backend makes before it adds a stop after a run-in heading: whether there
//! is a title at all, and whether the title already ends in sentence
//! punctuation. Every backend calls these two functions, so two outputs of
//! the same document cannot disagree about which titles get a stop.
//! [`heading_depth`] likewise computes, for every backend, how deep a heading
//! is below the render's root heading level.

use techy::core::node::{BodySlotExt, NodeRef, NodeSlice, SlotRole};
use techy::error::DiagnosticInfo;
use techy::latexlike::{EnvironmentSyntax, InvocationSyntaxData, LatexlikeCallableType};
use techy::recompose::{
    ComposePiece, ConcatPieces, Recompose, RecomposeContext, Recomposer, core_source_instruction,
};

use crate::lang::copy::{ValueContent, copy_run_into_value};
use crate::lang::guard::{self, GuardPolicy};
use crate::lang::{Flm, FlmCallableType, PresPayload};
use crate::layout::is_breakable_space;

use super::payload::{CellsPayload, DecoMarkPayload};

/// Whether `node` is a presentation node: a callable node of the presentation
/// form, which is the form every element of [the vocabulary](super) is
/// registered with.
pub fn is_presentation(node: &NodeRef<'_, Flm>) -> bool {
    node.callable_type() == Some(FlmCallableType::Presentation)
}

/// The node's typed payload as a `P`: `None` when the node carries no payload
/// — most nodes of a document — or carries one of another type.
///
/// Three kinds of node carry a payload
/// ([`FlmNodeExt::payload`](crate::lang::FlmNodeExt::payload)): a presentation
/// node, a synthesized node carrying data of its own, and a node a rewrite
/// rebuilt with data of its own, such as one part of a [block-transparent
/// element](crate::lang::rewrites::blocks::transparent) cut at a paragraph
/// break, which carries its element's record. The type to ask for is the one
/// the element uses: [`ParPayload`](super::ParPayload) for `flm.pres.par`,
/// [`LinkPayload`](super::LinkPayload) for `flm.pres.link`, the element's own
/// record type for the parts of a block-transparent element, and so on.
pub fn payload_of<'t, P: PresPayload>(node: &NodeRef<'t, Flm>) -> Option<&'t P> {
    node.ext().payload.as_deref().and_then(|payload| payload.downcast_ref::<P>())
}

/// The content of the node's body slot — the content slot marked as the body,
/// named `body` on every element of the vocabulary that has one. `None` for a
/// node without a body slot, such as `flm.pres.image` or a list node.
pub fn body_of<'t>(node: &NodeRef<'t, Flm>) -> Option<NodeSlice<'t, Flm>> {
    node.body()
}

/// The content of the slot at `index` in the node's slot list, counting
/// content slots and attached slots alike; `None` when the node has no slot
/// there.
pub fn slot_of<'t>(node: &NodeRef<'t, Flm>, index: usize) -> Option<NodeSlice<'t, Flm>> {
    node.slot_content_nodes(index)
}

/// How many slots the node has, content slots and attached slots together.
pub fn slot_count(node: &NodeRef<'_, Flm>) -> usize {
    node.slots().map_or(0, |slots| slots.len())
}

/// One attached slot of a presentation node: the slot that holds a decoration
/// the lowering driver moved out of the node's content, with the slot's
/// position and name.
#[derive(Debug, Clone)]
pub struct AttachedSlot<'t> {
    /// The slot's index in the node's slot list: the number the payload of
    /// the corresponding `flm.pres.deco-mark` node carries.
    pub index: usize,
    /// The slot's name, which is the decoration element's name (for instance
    /// `flm.pres.deco.runin`).
    pub name: Option<&'t str>,
    /// The slot's content: the decoration node it holds.
    pub content: NodeSlice<'t, Flm>,
}

/// The attached slots of a presentation node, in slot order: one per
/// decoration the lowering driver moved out of the node's content slots.
/// Empty for a node that carries no decoration.
pub fn attached_slots<'t>(node: &NodeRef<'t, Flm>) -> Vec<AttachedSlot<'t>> {
    let Some(slots) = node.slots() else { return Vec::new() };
    slots
        .iter()
        .enumerate()
        .filter(|(_, slot)| slot.role == SlotRole::Attached)
        .filter_map(|(index, slot)| {
            Some(AttachedSlot {
                index,
                name: slot.name(),
                content: node.slot_content_nodes(index)?,
            })
        })
        .collect()
}

/// The decoration that the decoration mark `mark` (a `flm.pres.deco-mark`
/// node) refers to: the first node of the attached slot of `carrier` whose
/// index the mark's payload gives. `carrier` is the presentation node whose
/// content slot holds the mark.
///
/// Returns `None` when `mark` carries no [`DecoMarkPayload`] or `carrier` has
/// no slot at the index the payload gives. Only a tree built by hand can have
/// that shape; in a tree the lowering driver produced, every mark has its
/// decoration.
pub fn decoration_at<'t>(
    carrier: &NodeRef<'t, Flm>,
    mark: &NodeRef<'t, Flm>,
) -> Option<NodeRef<'t, Flm>> {
    let payload = payload_of::<DecoMarkPayload>(mark)?;
    carrier.slot_content_nodes(payload.slot as usize)?.first()
}

/// The `(label, item)` content pairs of a `flm.pres.list` node, in order.
///
/// The slots are read by position rather than by name, since `label` and
/// `item` repeat once per entry. A last slot with no partner is ignored, so a
/// malformed list still reads as the entries it does have.
pub fn list_entries<'t>(node: &NodeRef<'t, Flm>) -> Vec<(NodeSlice<'t, Flm>, NodeSlice<'t, Flm>)> {
    let count = slot_count(node);
    let mut entries = Vec::with_capacity(count / 2);
    let mut index = 0;
    while index + 1 < count {
        if let (Some(label), Some(item)) =
            (node.slot_content_nodes(index), node.slot_content_nodes(index + 1))
        {
            entries.push((label, item));
        }
        index += 2;
    }
    entries
}

/// A `flm.pres.cells` node's payload and the content of each of its cells, in
/// the order of the payload's entries: the slot at index *i* is entry *i*'s.
///
/// Returns `None` when the node carries no [`CellsPayload`]. A slot missing
/// for an entry (a shape a tree built by hand can have, never one the `cells`
/// feature stages) is returned as `None` in the list, so the list always has
/// one item per entry.
pub fn cells_of<'t>(
    node: &NodeRef<'t, Flm>,
) -> Option<(&'t CellsPayload, Vec<Option<NodeSlice<'t, Flm>>>)> {
    let payload = payload_of::<CellsPayload>(node)?;
    let contents = (0..payload.cells.len()).map(|index| node.slot_content_nodes(index)).collect();
    Some((payload, contents))
}

/// The plain text of a run: the characters of every text node among the given
/// nodes and their descendants, concatenated in document order.
///
/// Everything that is not text is left out, with nothing put in its place, so
/// the result measures the run's characters; it is not a rendering of the
/// run. An emitter uses it where it needs a width: a heading's underline, the
/// column at which a list's items start.
pub fn plain_text<'t>(nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>) -> String {
    let mut out = String::new();
    for node in nodes {
        if let Some(text) = node.chars() {
            out.push_str(text);
        }
        for child in node.descendants() {
            if let Some(text) = child.chars() {
                out.push_str(text);
            }
        }
    }
    out
}

/// The characters a run-in heading's title may already end in, in which case a
/// backend adds no stop of its own: the period, the comma, the colon, the
/// semicolon, the question mark and the exclamation mark.
///
/// The test a backend makes is [`ends_in_sentence_punctuation`], which reads
/// a run of nodes; this constant is the character set that test uses, for a
/// caller that already has the text.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::pres::RUN_IN_STOPS;
///
/// assert!("Why?".ends_with(RUN_IN_STOPS));
/// assert!(!"Note".ends_with(RUN_IN_STOPS));
/// ```
pub const RUN_IN_STOPS: [char; 6] = ['.', ',', ':', ';', '?', '!'];

/// The stop a backend writes after a run-in heading's title when the render
/// request sets none: a period. It is the default of the `run_in_stop` render
/// option of every backend that has this option.
pub const DEFAULT_RUN_IN_STOP: &str = ".";

/// The heading level a backend writes as its first-level heading when the
/// render request sets none: 1, the level of a section. It is the default of
/// the `root_heading_level` render option of every backend that writes a
/// heading by its level
/// ([`HtmlRenderOptions::root_heading_level`](crate::stdcore::backends::html::HtmlRenderOptions::root_heading_level),
/// [`TextRenderOptions::root_heading_level`](crate::stdcore::backends::text::TextRenderOptions::root_heading_level)).
///
/// A heading's level is a number counted from zero, the smaller the outer, and
/// every value is a level ([`HeadingPayload::level`](super::HeadingPayload::level));
/// the root says which of them a render writes first.
pub const DEFAULT_ROOT_HEADING_LEVEL: u8 = 1;

/// How deep a heading of level `level` is below the root heading level
/// `root`: 0 for a heading at the root's level (the first-level heading,
/// `<h1>` on an HTML page), 1 for one level deeper, and so on. Every backend
/// that writes a heading by its level computes the depth with this function,
/// so two outputs of one document cannot disagree about a heading's depth.
///
/// # Errors
///
/// [`HeadingAboveRoot`] when `level` is less than `root`: such a heading has
/// no depth below the root. A backend reports it as an Error and writes the
/// heading at depth 0, the root's level.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::pres::heading_depth;
///
/// assert_eq!(heading_depth(1, 1), Ok(0));
/// assert_eq!(heading_depth(3, 1), Ok(2));
/// assert_eq!(heading_depth(0, 0), Ok(0));
/// assert!(heading_depth(0, 1).is_err());
/// ```
pub fn heading_depth(level: u8, root: u8) -> Result<u8, HeadingAboveRoot> {
    level.checked_sub(root).ok_or(HeadingAboveRoot { level, root })
}

/// Condition `flm.pres.heading-above-root`: a heading's level is less than
/// the render's root heading level (the `root_heading_level` render option),
/// so the heading is above the first-level heading the render writes and has
/// no depth below it ([`heading_depth`]).
///
/// Both the HTML backend and the text backend report this one condition, for
/// every block heading they write; on HTML it is reported regardless of the
/// tag the render options map the heading's kind to
/// ([`HtmlRenderOptions::heading_tags`](crate::stdcore::backends::html::HtmlRenderOptions::heading_tags)).
///
/// Recovery: the heading is written at the root's level, as a first-level
/// heading — on the page, a heading whose kind is mapped keeps its mapped tag
/// — and on the page its attribute `data-flm-level` still says its own level.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.pres.heading-above-root",
    message = "a heading's level must be at least the root heading level {root} (the render \
               option `root_heading_level`); this heading's level is {level}"
)]
pub struct HeadingAboveRoot {
    /// The heading's level.
    pub level: u8,
    /// The render's root heading level.
    pub root: u8,
}

/// Whether the content of a run-in decoration already ends in sentence
/// punctuation — one of [`RUN_IN_STOPS`] — so that a backend writing such a
/// decoration should not add a stop of its own.
///
/// The test is the run's [`plain_text`], trimmed of trailing whitespace (which
/// includes a non-breaking space). Every backend that adds a stop asks this one
/// function, so that two outputs of the same document cannot disagree about
/// which titles get one.
///
/// **This test is a heuristic.** It looks only at the characters the title
/// ends in, not at what the title's last element is: a title ending in a
/// formula, or in a box drawn by an author's own command, ends in no
/// character of the list and gets the stop after it. An extension that needs
/// headings treated another way registers a lowering rule or a stage of its
/// own; [the extension guide](crate::guide::extending#lowering-rules)
/// explains how.
///
/// See [`holds_text`], which tests whether there is a title for the stop to
/// follow at all.
pub fn ends_in_sentence_punctuation<'t>(
    nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>,
) -> bool {
    plain_text(nodes).trim_end().ends_with(RUN_IN_STOPS)
}

/// Whether a run holds text beyond breakable whitespace: its [`plain_text`] has
/// at least one character that is not a space a line may be broken at
/// ([`is_breakable_space`]).
///
/// A run that fails this test is blank (`\paragraph{}` and `\paragraph{ }`
/// both reach a backend that way), and a blank run-in title has nothing for a
/// stop to follow, so no backend writes one after it. Both the text and the
/// HTML backend call this function, as they call
/// [`ends_in_sentence_punctuation`], so that they cannot disagree.
///
/// A non-breaking space is text the author asked for and not breakable
/// whitespace, so a title holding one is not blank.
pub fn holds_text<'t>(nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>) -> bool {
    plain_text(nodes).chars().any(|c| !is_breakable_space(c))
}

/// The source text of a run of nodes, as techy's source recomposer writes it
/// again from what the nodes recorded of their invocations: byte for byte
/// for a run parsed from one source. An emitter writes this for an opaque
/// body: the content of a math node, or a verbatim run holding a construct a
/// template substituted into it.
///
/// The nodes may belong to any tree and come from several sources, and the
/// result then **means what the nodes mean**: a control word written with
/// nothing after it, such as `\alpha`, is followed by a space when the next
/// node's text begins with a letter. For example, a user definition's
/// template `\alpha#1` used with the argument `x` gives `\alpha x`, where the
/// two texts put together would make the single command `\alphax`. A run
/// parsed from one source never holds two such nodes, so its text is
/// unchanged.
///
/// A run techy cannot write again (a tree built by hand whose invocation
/// records are not coherent) is not an error here: the run's [`plain_text`]
/// is returned instead.
///
/// Call this on a worker thread (see
/// [`on_worker_thread`](crate::lang::guard::on_worker_thread)): copying the
/// run and recomposing it are both recursive walks, guarded by a budget sized
/// for a worker thread's stack, and a thread with a smaller stack can
/// overflow before the guard refuses. Every stage of a run already executes
/// on such a thread, and the text emitter's entry point
/// ([`emit`](crate::stdcore::backends::text::emit())) creates one for itself.
pub fn recompose_source(nodes: &[NodeRef<'_, Flm>]) -> String {
    let Some(first) = nodes.first() else { return String::new() };
    // The run copied verbatim into a value tree (the language's one copy
    // helper), then recomposed from the copies' recorded payloads.
    let copied = match copy_run_into_value(nodes, first, ValueContent::Own, GuardPolicy::StackBudget) {
        Ok(tree) => tree,
        Err(_) => return plain_text(nodes.iter().copied()),
    };
    match guard::recomposer(&mut Spelling, GuardPolicy::StackBudget).recompose(&copied, ()) {
        Ok(text) => text.0,
        Err(_) => plain_text(nodes.iter().copied()),
    }
}

/// Source text being put together from the spellings of nodes, which keeps a
/// control word at its end apart from a letter that follows (see
/// [`recompose_source`]).
#[derive(Debug, Clone, Default)]
struct SourceText(String);

impl From<&str> for SourceText {
    fn from(text: &str) -> SourceText {
        SourceText(String::from(text))
    }
}

impl From<String> for SourceText {
    fn from(text: String) -> SourceText {
        SourceText(text)
    }
}

impl ComposePiece for SourceText {
    fn empty() -> SourceText {
        SourceText::default()
    }

    fn append(&mut self, other: SourceText) {
        if ends_with_control_word(&self.0) && other.0.starts_with(|c: char| c.is_ascii_alphabetic()) {
            self.0.push(' ');
        }
        self.0.push_str(&other.0);
    }
}

/// Whether `text` ends with a control word: letters after a backslash that is
/// not itself escaped.
fn ends_with_control_word(text: &str) -> bool {
    let head = text.trim_end_matches(|c: char| c.is_ascii_alphabetic());
    if head.len() == text.len() {
        return false;
    }
    let backslashes = head.len() - head.trim_end_matches('\\').len();
    backslashes % 2 == 1
}

/// The recomposer of [`recompose_source`]: each node's recorded spelling, as
/// techy's own source recomposer writes it, put together as [`SourceText`].
struct Spelling;

/// The failure of [`Spelling`]: a callable whose invocation record does not
/// match the form of its callable, which a parse never produces.
#[derive(Debug)]
struct IncoherentInvocation;

impl<A> Recomposer<Flm, A> for Spelling {
    type State = ();
    type Piece = SourceText;
    type Error = IncoherentInvocation;

    fn recompose_node(
        &mut self,
        node: NodeRef<'_, Flm, A>,
        _state: &(),
        _cx: &mut RecomposeContext<'_, Flm, A>,
    ) -> Result<Recompose<SourceText, ()>, IncoherentInvocation> {
        if let Some(instruction) = core_source_instruction(node) {
            return Ok(instruction);
        }
        let data = node.callable().ok_or(IncoherentInvocation)?;
        let source = node.span().source();
        let name: &str = &data.name;
        // The invocation as it was written, then the argument regions as
        // children, as techy's source recomposer writes it.
        let (head, tail) = match &data.invocation_syntax {
            InvocationSyntaxData::Macro { escape_char, post_space } if data.callable_type.is_macro() => {
                (format!("{escape_char}{name}{}", post_space.resolve(source)), String::new())
            }
            InvocationSyntaxData::Environment(env) if data.callable_type.is_environment() => {
                (env.write_begin(name, source), env.write_end(name, source))
            }
            InvocationSyntaxData::Specials if data.callable_type.is_specials() => {
                (String::from(name), String::new())
            }
            _ => return Err(IncoherentInvocation),
        };
        Ok(Recompose::Concat(
            ConcatPieces::children().wrap(SourceText(head), SourceText(tail)),
        ))
    }
}

/// The identity of the run written as `nodes` (the top-level nodes of an
/// argument), computed from its source text: each node's own source text, in
/// order, with comments left out, every run of whitespace made one space and
/// the ends trimmed. The result is the empty text when the run is empty or
/// holds only whitespace. Two runs written alike (the same text and the same
/// markup, however the whitespace is laid out) have the same identity; case
/// and markup count.
///
/// This is the only function that decides whether two written runs are the
/// same. It computes a term's identity, which a term definition, its aliases
/// and a term link use ([the identity of a
/// term](crate::stdcore::features::defterm::elements#the-identity-of-a-term));
/// whether two citation notes are one note
/// ([`stage_mark`](crate::stdcore::features::citations::stage_mark)); and a
/// citation key, each part of a `\cite`'s keys between two commas
/// ([`read_chunk`](crate::stdcore::features::citations::read_chunk)).
///
/// Call this on a worker thread, as every stage and every lowering rule runs:
/// writing the nodes' source text again is a recursive walk guarded by a
/// budget sized for such a thread ([`recompose_source`]).
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
/// use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
/// use flm_core::model::doc::FragmentLayout;
/// use flm_core::stdcore::{self, StdcoreConfig};
/// use flm_core::stdcore::backends::pres::spelling_identity;
///
/// let mut builder = RegistryBuilder::new();
/// builder.use_package(stdcore::package(StdcoreConfig::new()));
/// builder.use_package(flm_core::math_passthrough::package());
/// let registry = builder.build(&RegistryInputs::new())?;
/// let source = FlmSource::new("See \\term{\\emph{Markov}\n   chain}.");
/// let fragment = parse_fragment(
///     registry.lang(),
///     Arc::new(source.with_origin(FlmOrigin::none())),
///     FragmentLayout::default(),
///     &ParseOptions::default(),
/// )?;
/// let link = fragment.tree().root().descendants()
///     .find(|node| node.name() == Some("term"))
///     .expect("the term link");
/// let shown = link.argument_content_nodes_named("shown")?.expect("written");
/// assert_eq!(spelling_identity(shown.iter()), "\\emph{Markov} chain");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn spelling_identity<'t>(nodes: impl IntoIterator<Item = NodeRef<'t, Flm>>) -> String {
    let nodes: Vec<NodeRef<'t, Flm>> = nodes.into_iter().filter(|node| !node.is_comment()).collect();
    // Each node re-emitted from what it records of its own spelling, never a
    // slice of a source: see decision rationale at [§flm-dd-dr:std-defterm-identity].
    recompose_source(&nodes).split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Whether the slot at `index` is the node's body slot: the content slot an
/// emitter treats as the node's content ([`BODY_SLOT`](super::BODY_SLOT) on
/// every element of the vocabulary that has one).
pub fn is_body_slot(node: &NodeRef<'_, Flm>, index: usize) -> bool {
    node.slots().and_then(|slots| slots.get(index)).is_some_and(|slot| slot.ext.is_body())
}
