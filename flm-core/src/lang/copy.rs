//! Building a synthesized value from nodes of an existing tree:
//! [`copy_run_into_value`] and [`copy_pieces_into_value`].
//!
//! A synthesized value is a small tree that a stage builds while a run is under
//! way, held outside the document's trees (see [synthesized
//! values](crate::guide::concepts_overview#synthesized-values)). Examples are
//! the content a `\ref` displays, a formatted citation entry, and the exact
//! source text an emitter has to print back. A synthesized value (a *value*
//! for short in this module) is lowered and rendered like document content,
//! but no stage that scans the document reads it.
//!
//! This module copies nodes *out of* a tree into a new value. Copying nodes
//! *into* a tree being built, as a lowering rule does when it adds a copy to
//! the presentation tree, belongs to the lowering driver and does not use this
//! module.
//!
//! # Copying a run of nodes
//!
//! [`copy_run_into_value`] builds a value from a run of sibling nodes of an
//! existing tree. Every copied node keeps its own span and its own source, so a
//! diagnostic raised about the value still points at the place in the document
//! where the content was written. The copies are the children of a new list
//! node (a container node, element
//! [`flm.container`](crate::lang::structural::CONTAINER)) spanning the run,
//! which is the value's root.
//!
//! # Copying value pieces
//!
//! [`copy_pieces_into_value`] builds a value from a list of value pieces
//! ([`ValuePiece`]), each of one of four kinds:
//!
//! - a node, copied with its subtree;
//! - characters that are not a node of their own, which become a new text
//!   node. An example is the value `Custom` of `tag=Custom`, which shares its
//!   text node with the key. [`key_value_pieces`] reads the value of a
//!   key-value entry into such pieces ([`WrittenPiece`]);
//! - a new node of a synthesized element around further pieces
//!   ([`SynthesizedPiece`]), for content that needs a structure of its own. Such
//!   a node may carry data of its own (a [payload](crate::lang::PresPayload))
//!   and may hold no piece at all. For example, an
//!   [xnote](crate::stdcore::features::xnotes) is a note set apart from the
//!   running text and shown elsewhere or in place (a footnote, an endnote, a
//!   side note); the list of an xnote kind is made of such nodes, each entry
//!   holding a node whose payload refers to the xnote whose content it shows,
//!   instead of a copy of that content;
//! - a new node of a command of the language around further pieces
//!   ([`CommandPiece`]), for content that needs a command such as emphasis or
//!   a link. The node is the one a parse of the command would produce, so it
//!   is read and lowered like the written command.
//!
//! # Content shown again and content of the value's own
//!
//! Every copy is made for a purpose ([`ValueContent`]): to show content of the
//! document a second time, away from where it is written (what a reference
//! shows of the heading it refers to), or to hold content as the value's own.
//!
//! - A copy of the value's own ([`ValueContent::Own`]) copies every node as it
//!   is.
//! - A copy shown again ([`ValueContent::Shown`]) asks each node it copies, at
//!   every depth, what to put in its place: the element's
//!   [`InCopy`](crate::model::aspects::InCopy) aspect, read through
//!   [`copied_as`]. The aspect returns the node itself over the copies of its
//!   children (the default, for every element without the aspect), nothing, or
//!   one new node of the element's own, at the node's span and without
//!   children. An xnote, for example, is replaced by such a node, which the
//!   xnote's lowering rules write as the xnote's mark: a note shown again is
//!   not a second note.
//!
//! # Copies must run on a worker thread
//!
//! The copy is a guarded traversal ([`crate::lang::guard`]) that does not check
//! its thread: call these functions on a worker thread, for example inside
//! [`on_worker_thread`](crate::lang::guard::on_worker_thread).

use std::sync::Arc;

use techy::core::node::{
    BodySlotExt, BuildId, CallableData, ChildRegion, ContentNodes, NodeKind, NodeRef, NodeSlice,
    NodeTree, ParsedArgument, ParsedArguments, ParsedSlot, ParsedSlots, SlotRole,
};
use techy::core::specs::{CallableQuery, CallableSpec, CallableSyntax};
use techy::core::{Lang, ParsingState};
use techy::latexlike::{BodyMarker, InvocationSyntaxData};
use techy::source::{SourceSpan, TextContent};
use techy::transform::{Restage, RestageContext, RestageError, RestageVisitor};

use crate::lang::guard::{self, GuardPolicy};
use crate::lang::mint::specs::element_of_spec;
use crate::lang::mint::FlmSynthesizedSpec;
use crate::lang::structural::UNKNOWN_MACRO;
use crate::lang::text::{is_blank, is_blank_text, is_space};
use crate::lang::{Flm, FlmCallableType, FlmSpan, PresPayload};
use crate::model::aspects::{CopiedAs, copied_as};

/// The reasons a run of nodes or a list of value pieces ([`ValuePiece`]) could
/// not be copied into a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CopyError {
    /// The [recursion guard](crate::lang::guard) refused to descend further: the
    /// content is nested too deeply.
    NestingTooDeep {
        /// techy's description of the limit that was reached.
        detail: String,
    },
    /// Building the copy failed for another reason. This is an error in the
    /// program (for example, the nodes passed to [`copy_run_into_value`] did
    /// not belong to the tree of its `anchor` argument, or a node extension
    /// could not be built), not something a document causes.
    Failed {
        /// techy's description of what failed.
        detail: String,
    },
}

impl std::fmt::Display for CopyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CopyError::NestingTooDeep { detail } => {
                write!(f, "the content nests too deeply to copy: {detail}")
            }
            CopyError::Failed { detail } => write!(f, "copying the content failed: {detail}"),
        }
    }
}

impl std::error::Error for CopyError {}

impl From<RestageError<CopyError>> for CopyError {
    fn from(error: RestageError<CopyError>) -> CopyError {
        match error {
            RestageError::Visitor(error) => error,
            RestageError::DescentLimitExceeded { detail } => CopyError::NestingTooDeep { detail },
            other => CopyError::Failed { detail: other.to_string() },
        }
    }
}

/// A visitor that copies every node unchanged: the copy of the value's own
/// ([`ValueContent::Own`]).
struct Verbatim;

impl RestageVisitor<Flm, (), ()> for Verbatim {
    type Error = CopyError;

    fn restage(
        &mut self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, CopyError> {
        Ok(Restage::Descend(()))
    }
}

/// A visitor for a copy shown again ([`ValueContent::Shown`]): it asks each
/// node's element what to put in the node's place ([`copied_as`]) and adds
/// that to the copy: the node over the copies of its children, nothing, or one
/// new node of the element's own, at the node's span and under its parsing
/// state, without children.
struct AskEach;

impl RestageVisitor<Flm, (), ()> for AskEach {
    type Error = CopyError;

    fn restage(
        &mut self,
        node: NodeRef<'_, Flm>,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, CopyError> {
        match copied_as(&node, ValueContent::Shown) {
            CopiedAs::Verbatim => Ok(Restage::Descend(())),
            CopiedAs::Nothing => Ok(Restage::Emit(Vec::new())),
            CopiedAs::StandIn(stand_in) => {
                let kind = NodeKind::callable(stand_in.spec.callable_data());
                let span = node.span().clone();
                let state = Arc::clone(node.parsing_state());
                let staged = stage_node_with(cx, kind, span, state, Vec::new(), stand_in.payload)?;
                Ok(Restage::Emit(vec![staged]))
            }
        }
    }
}

/// The purpose of a copy: whether the copied content shows document content a
/// second time or belongs to the value. The code that builds the value passes it
/// to [`copy_run_into_value`] or [`copy_pieces_into_value`].
///
/// The purpose decides how each node is copied: a copy of the value's own
/// copies every node as it is, and a copy shown again asks each node's element
/// what to put in its place ([`copied_as`]); see the [module
/// documentation](self#content-shown-again-and-content-of-the-values-own).
///
/// The purpose affects the copy only: the value records nothing of it, and
/// every diagnostic its content raises when the value is brought up to date
/// and lowered is reported, regardless of the purpose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueContent {
    /// Content of the document shown a second time, away from where it is
    /// written: what a reference shows of the point it refers to (a heading's
    /// title, a list item's tag, the alternative text of a float's caption).
    /// Each node is copied as its element's
    /// [`InCopy`](crate::model::aspects::InCopy) aspect says.
    Shown,
    /// Content the value holds as its own: a structure the value builds
    /// around content lowered nowhere else (the list of an
    /// [xnote](crate::stdcore::features::xnotes) kind, the
    /// lead-in of a term definition), the use site of a user definition copied
    /// to be expanded in place, or content copied only to be read. Every node
    /// is copied as it is, regardless of what its element would put in its
    /// place for a copy shown again, because a use site copied to be expanded
    /// in place *is* the document's content there, and content copied to be
    /// read must be what was written.
    Own,
}

/// One value piece: a part of the content [`copy_pieces_into_value`] copies: a
/// node, copied with its subtree; characters that are not a node of their own;
/// or a new node of a synthesized element or of a command, holding pieces of
/// its own.
#[derive(Debug, Clone, Copy)]
pub enum ValuePiece<'n, 't> {
    /// A node of any tree, copied with its subtree as the copy's purpose says
    /// ([`ValueContent`]).
    Node(NodeRef<'t, Flm>),
    /// Characters, added as one new text node with an empty span at the
    /// anchor's start (or at the start of the enclosing piece's span).
    Text(&'n str),
    /// A new node of a synthesized element, holding the copy of its own pieces
    /// ([`SynthesizedPiece`]).
    Synthesized(&'n SynthesizedPiece<'n, 't>),
    /// A new node of a command of the language, whose arguments hold the
    /// copies of pieces ([`CommandPiece`]).
    Command(&'n CommandPiece<'n, 't>),
}

/// A new node that [`copy_pieces_into_value`] creates around value pieces
/// ([`ValuePiece`]): a node of a [synthesized
/// element](crate::lang::mint::SyntaxDef::Synthesized) (an element no document
/// writes) with one content slot, which holds the copy of `body`.
///
/// `body` is copied like any other pieces, except that a text piece in it gets
/// an empty span at the start of `span` instead of at the anchor's start. The
/// node carries `payload`, when there is one, as its own data
/// ([`FlmNodeExt::payload`](crate::lang::FlmNodeExt::payload)).
///
/// This is how a value gets a structure of its own. For example, the list of an
/// [xnote](crate::stdcore::features::xnotes) kind (a kind of footnote, endnote
/// or side note) is one list node holding one entry node per xnote, each
/// holding a node whose payload refers to the xnote whose content the entry
/// shows.
#[derive(Clone)]
pub struct SynthesizedPiece<'n, 't> {
    /// The specification of the node, which carries its synthesized element.
    pub spec: Arc<FlmSynthesizedSpec>,
    /// The name of the node's content slot, which holds the copy of `body`.
    pub slot: &'n str,
    /// The node's span: where a diagnostic about the node points.
    pub span: FlmSpan,
    /// The parsing state the node is added under.
    pub state: Arc<ParsingState<Flm>>,
    /// The pieces the node holds, in order.
    pub body: Vec<ValuePiece<'n, 't>>,
    /// The node's own data, set in its extension; `None` for a node that
    /// carries none.
    pub payload: Option<Arc<dyn PresPayload>>,
}

impl std::fmt::Debug for SynthesizedPiece<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SynthesizedPiece")
            .field("element", &self.spec.element().name)
            .field("slot", &self.slot)
            .field("span", &self.span)
            .field("body", &self.body)
            .finish_non_exhaustive()
    }
}

/// A new node that [`copy_pieces_into_value`] creates for a command of the
/// language (`\emph{…}`, `\href{…}{…}`) whose arguments hold value pieces
/// ([`ValuePiece`]): the node a parse of the command written with those
/// arguments would produce, carrying the command's element, so that it is read,
/// rewritten and lowered like the written command.
///
/// Build one with [`CommandPiece::new`], which looks the command up in the
/// language as a parsing state defines it, then fill
/// [`arguments`](CommandPiece::arguments). The node is recorded as written with
/// a backslash and nothing after the name. Each provided argument's content is
/// the copies of its pieces, as direct children of the node; a text piece gets
/// an empty span at the start of the node's span.
#[derive(Clone)]
pub struct CommandPiece<'n, 't> {
    /// The command's name without its backslash: `emph`.
    name: String,
    /// The command's specification in the language.
    spec: Arc<dyn CallableSpec<Flm>>,
    /// The node's span: where a diagnostic about the node points.
    pub span: FlmSpan,
    /// The parsing state the node is added under, and in which the command
    /// was looked up.
    pub state: Arc<ParsingState<Flm>>,
    /// The pieces of each argument, in the order in which the command declares
    /// its arguments; `None`, or no entry past the end of the list, for an
    /// argument not provided. More entries than the command has arguments make
    /// the copy fail ([`CopyError::Failed`]).
    pub arguments: Vec<Option<Vec<ValuePiece<'n, 't>>>>,
}

impl<'n, 't> CommandPiece<'n, 't> {
    /// Returns the command `\name` as the language defines it under `state`,
    /// at `span`, with no argument provided yet: `Ok(Some(_))` for a command
    /// the language defines there; `Ok(None)` where it defines no such command
    /// (the feature that defines it is not in the registry, and a parse would
    /// treat the command as unknown, [`UNKNOWN_MACRO`]).
    ///
    /// # Errors
    ///
    /// [`CopyError::Failed`] when looking the command up fails: a provider of
    /// command definitions on the state's scope stack returned an error.
    pub fn new(name: &str, span: FlmSpan, state: Arc<ParsingState<Flm>>) -> Result<Option<CommandPiece<'n, 't>>, CopyError> {
        let query = CallableQuery::new(FlmCallableType::Macro, name, CallableSyntax::Command { escape_char: '\\' });
        // A source of command definitions that cannot answer is a failure,
        // never a command the language does not define.
        let spec = match state.scopes().retrieve_spec(&query, &state) {
            Ok(Some(spec)) => spec,
            Ok(None) => return Ok(None),
            Err(error) => {
                return Err(CopyError::Failed { detail: format!("looking up the command `\\{name}` failed: {error}") })
            }
        };
        match element_of_spec(&*spec) {
            Some(element) if element.name.as_str() != UNKNOWN_MACRO => {
                Ok(Some(CommandPiece { name: name.to_string(), spec, span, state, arguments: Vec::new() }))
            }
            _ => Ok(None),
        }
    }

    /// Returns the command's name without its backslash.
    pub fn name(&self) -> &str {
        &self.name
    }
}

impl std::fmt::Debug for CommandPiece<'_, '_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandPiece")
            .field("name", &self.name)
            .field("span", &self.span)
            .field("arguments", &self.arguments)
            .finish_non_exhaustive()
    }
}

/// One piece of the value of a key-value entry, as the author wrote it in a
/// bracket: a node of the document, or characters split off a text node that
/// also holds the key (`Custom` in `tag=Custom`), which are not a node of their
/// own.
///
/// [`key_value_pieces`] returns the value of an entry as such pieces; examples
/// are the tag of an `\item`, a list's own label, the alternative text of a
/// float's `\caption`. Unlike a [`ValuePiece`], it owns its characters, so a
/// reader can pass it on; a copy takes each piece as a [`ValuePiece`]
/// ([`WrittenPiece::as_value_piece`]).
#[derive(Debug, Clone)]
pub enum WrittenPiece<'t> {
    /// A node of the tree that holds the bracket, shown as the content it is.
    Node(NodeRef<'t, Flm>),
    /// Characters of the document, as the tree holds them.
    Text(String),
}

impl<'t> WrittenPiece<'t> {
    /// Returns the piece as [`copy_pieces_into_value`] takes it: the node, or
    /// the characters.
    pub fn as_value_piece(&self) -> ValuePiece<'_, 't> {
        match self {
            WrittenPiece::Node(node) => ValuePiece::Node(*node),
            WrittenPiece::Text(text) => ValuePiece::Text(text),
        }
    }
}

/// Returns the value of a key-value entry as value pieces ([`ValuePiece`]) of
/// document content: `value` is the entry's value as techy's key-value reader
/// read it from the bracket whose content is `content`.
///
/// The key-value reader builds a tree of its own, whose nodes carry the spans
/// they were read from. A group or a callable of the value is returned as the
/// node of the document it was read from, found in `content` by its span;
/// characters split off a text node that also holds the key are returned as
/// text. Whitespace at either end is dropped, a node that is whitespace (such
/// as `~`) included.
///
/// The features whose brackets hold such values (lists, floats) all read them
/// through this function.
pub fn key_value_pieces<'t>(
    content: &NodeSlice<'t, Flm>,
    value: NodeSlice<'_, Flm>,
) -> Vec<WrittenPiece<'t>> {
    let pieces: Vec<WrittenPiece<'t>> = value
        .iter()
        .map(|piece| {
            let found = whole_node(content, &piece);
            match (found, piece.chars()) {
                (Some(node), _) => WrittenPiece::Node(node),
                (None, Some(text)) => WrittenPiece::Text(text.to_string()),
                // A derived node that is not characters always stands for a
                // whole node of the bracket; nothing else can be answered.
                (None, None) => WrittenPiece::Text(String::new()),
            }
        })
        .collect();
    trim_pieces(pieces)
}

/// Returns `pieces` (value pieces, [`ValuePiece`]) with the whitespace at
/// either end dropped: the blank pieces at each end (text holding only
/// whitespace, [`is_blank_text`], and a blank node, [`is_blank`]: a text node
/// holding only whitespace, or a node that is whitespace, such as `~` and
/// `\ `), and the whitespace ([`is_space`]) a text piece at either end begins
/// or ends with.
pub(crate) fn trim_pieces(mut pieces: Vec<WrittenPiece<'_>>) -> Vec<WrittenPiece<'_>> {
    let blank = |piece: &WrittenPiece<'_>| match piece {
        WrittenPiece::Text(text) => is_blank_text(text),
        WrittenPiece::Node(node) => is_blank(node),
    };
    while pieces.first().is_some_and(blank) {
        pieces.remove(0);
    }
    while pieces.last().is_some_and(blank) {
        pieces.pop();
    }
    if let Some(WrittenPiece::Text(text)) = pieces.first_mut() {
        *text = text.trim_start_matches(is_space).to_string();
    }
    if let Some(WrittenPiece::Text(text)) = pieces.last_mut() {
        *text = text.trim_end_matches(is_space).to_string();
    }
    pieces
}

/// Returns the node of the bracket `content` that `piece`, a node of the
/// key-value reader's own tree, was read from as a whole, if there is one.
///
/// The reader's nodes carry the spans they were read from: a group or a
/// callable of a value is a whole node of the document, found by its span. A
/// text node split at the key keeps, where the block-structure pass rewrote
/// it, the span of the whole text node, key included, so characters are
/// matched to a node only when they are also that node's characters.
fn whole_node<'t>(
    content: &NodeSlice<'t, Flm>,
    piece: &NodeRef<'_, Flm>,
) -> Option<NodeRef<'t, Flm>> {
    content.iter().find(|node| {
        node.span().start() == piece.span().start()
            && node.span().end() == piece.span().end()
            && node.chars() == piece.chars()
    })
}

/// The visitor that produces the value. It is called once, at the tree's root,
/// and returns a new list node holding the copies of the pieces; nothing else
/// is visited.
struct CopyRun<'n, 't> {
    pieces: &'n [ValuePiece<'n, 't>],
    anchor: NodeRef<'t, Flm>,
    content: ValueContent,
}

impl RestageVisitor<Flm, (), ()> for CopyRun<'_, '_> {
    type Error = CopyError;

    fn restage(
        &mut self,
        _root: NodeRef<'_, Flm>,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, CopyError> {
        let at = self.anchor.span();
        let text_at = SourceSpan::new(at.source(), at.start()..at.start());
        let children = stage_pieces(cx, self.pieces, self.content, &text_at, self.anchor.parsing_state())?;
        let (span, state) = where_pieces_stand(self.pieces).unwrap_or_else(|| {
            let at = self.anchor.span();
            (
                SourceSpan::new(at.source(), at.start()..at.start()),
                Arc::clone(self.anchor.parsing_state()),
            )
        });
        let root = stage_node(cx, NodeKind::list(), span, state, children)?;
        Ok(Restage::Emit(vec![root]))
    }
}

/// Adds the copy of each of `pieces`, in order: a node with its subtree, as
/// `purpose` says (unchanged for a copy of the value's own, as each element
/// says for a copy shown again, [`AskEach`]); a text as a new text node at the
/// empty span `text_at` under `text_state`; a synthesized or command piece as a
/// new node holding the copy of its own pieces, made the same way, whose texts
/// get the empty span at the piece's own start.
fn stage_pieces(
    cx: &mut RestageContext<'_, Flm, (), ()>,
    pieces: &[ValuePiece<'_, '_>],
    purpose: ValueContent,
    text_at: &FlmSpan,
    text_state: &Arc<ParsingState<Flm>>,
) -> Result<Vec<BuildId>, CopyError> {
    let failed = |error: &dyn std::fmt::Display| CopyError::Failed { detail: error.to_string() };
    let mut children = Vec::with_capacity(pieces.len());
    for piece in pieces {
        match piece {
            ValuePiece::Node(node) => {
                let copied = match purpose {
                    ValueContent::Own => cx.restage_subtree(*node, &mut Verbatim)?,
                    ValueContent::Shown => cx.restage_subtree(*node, &mut AskEach)?,
                };
                children.extend(copied);
            }
            ValuePiece::Text(text) => {
                let kind = NodeKind::chars(TextContent::Owned(Box::from(*text)));
                let state = Arc::clone(text_state);
                children.push(stage_node(cx, kind, text_at.clone(), state, Vec::new())?);
            }
            ValuePiece::Synthesized(piece) => {
                let start = SourceSpan::new(piece.span.source(), piece.span.start()..piece.span.start());
                let body = stage_pieces(cx, &piece.body, purpose, &start, &piece.state)?;
                let count = u32::try_from(body.len()).map_err(|error| failed(&error))?;
                let mut data = piece.spec.callable_data();
                data.slots = ParsedSlots::new(vec![ParsedSlot::new(
                    ChildRegion::new(0..count, ContentNodes::InRegion(0..count)),
                    piece.slot,
                    SlotRole::Content,
                    BodyMarker::make_body(),
                )]);
                let kind = NodeKind::callable(data);
                let state = Arc::clone(&piece.state);
                let payload = piece.payload.clone();
                children.push(stage_node_with(cx, kind, piece.span.clone(), state, body, payload)?);
            }
            ValuePiece::Command(piece) => children.push(stage_command(cx, piece, purpose)?),
        }
    }
    Ok(children)
}

/// Adds the new node of a command piece, each provided argument holding the
/// copies of its pieces.
fn stage_command(
    cx: &mut RestageContext<'_, Flm, (), ()>,
    piece: &CommandPiece<'_, '_>,
    purpose: ValueContent,
) -> Result<BuildId, CopyError> {
    let failed = |detail: String| CopyError::Failed { detail };
    let specs = piece.spec.arguments();
    if piece.arguments.len() > specs.len() {
        return Err(failed(format!(
            "`\\{}` takes {} {}, not {}",
            piece.name,
            specs.len(),
            if specs.len() == 1 { "argument" } else { "arguments" },
            piece.arguments.len()
        )));
    }
    let start = SourceSpan::new(piece.span.source(), piece.span.start()..piece.span.start());
    let mut children: Vec<BuildId> = Vec::new();
    let mut arguments = Vec::with_capacity(specs.len());
    for (index, spec) in specs.iter().enumerate() {
        let Some(Some(pieces)) = piece.arguments.get(index) else {
            arguments.push(ParsedArgument::absent(Arc::clone(spec)));
            continue;
        };
        let staged = stage_pieces(cx, pieces, purpose, &start, &piece.state)?;
        let from = u32::try_from(children.len()).map_err(|error| failed(error.to_string()))?;
        let count = u32::try_from(staged.len()).map_err(|error| failed(error.to_string()))?;
        children.extend(staged);
        let region = ChildRegion::new(from..from + count, ContentNodes::InRegion(0..count));
        arguments.push(ParsedArgument::provided(Arc::clone(spec), region, ()));
    }
    let kind = NodeKind::callable(CallableData {
        callable_type: FlmCallableType::Macro,
        name: Box::from(piece.name.as_str()),
        spec: Arc::clone(&piece.spec),
        arguments: ParsedArguments::new(arguments),
        slots: ParsedSlots::empty(),
        invocation_syntax: InvocationSyntaxData::Macro {
            escape_char: '\\',
            post_space: TextContent::Owned(Box::from("")),
        },
    });
    stage_node(cx, kind, piece.span.clone(), Arc::clone(&piece.state), children)
}

/// Adds one new node of `kind` over the already added `children`.
fn stage_node(
    cx: &mut RestageContext<'_, Flm, (), ()>,
    kind: NodeKind<Flm>,
    span: FlmSpan,
    state: Arc<ParsingState<Flm>>,
    children: Vec<BuildId>,
) -> Result<BuildId, CopyError> {
    stage_node_with(cx, kind, span, state, children, None)
}

/// Adds one new node of `kind` over the already added `children`, carrying
/// `payload` when there is one.
fn stage_node_with(
    cx: &mut RestageContext<'_, Flm, (), ()>,
    kind: NodeKind<Flm>,
    span: FlmSpan,
    state: Arc<ParsingState<Flm>>,
    children: Vec<BuildId>,
    payload: Option<Arc<dyn PresPayload>>,
) -> Result<BuildId, CopyError> {
    let failed = |error: &dyn std::fmt::Display| CopyError::Failed { detail: error.to_string() };
    let mut ext = Flm::make_node_ext(&kind, &span, &state, cx.builder().staged_children(&children))
        .map_err(|error| failed(&error))?;
    if let Some(payload) = payload {
        ext = ext.with_payload(payload);
    }
    cx.builder().add(kind, span, state, children, ext, ()).map_err(|error| failed(&error))
}

/// Returns the span covering the pieces that have a place in a source (the
/// nodes, the synthesized pieces and the command pieces, from the first to the
/// last) and the parsing state of the first; `None` when there is none.
fn where_pieces_stand(pieces: &[ValuePiece<'_, '_>]) -> Option<(FlmSpan, Arc<ParsingState<Flm>>)> {
    let located = |piece: &ValuePiece<'_, '_>| match piece {
        ValuePiece::Node(node) => Some((node.span().clone(), Arc::clone(node.parsing_state()))),
        ValuePiece::Synthesized(piece) => Some((piece.span.clone(), Arc::clone(&piece.state))),
        ValuePiece::Command(piece) => Some((piece.span.clone(), Arc::clone(&piece.state))),
        ValuePiece::Text(_) => None,
    };
    let (first, state) = pieces.iter().find_map(located)?;
    let (last, _) = pieces.iter().rev().find_map(located)?;
    Some((covering(&first, &last), state))
}

/// Returns the span covering a run from `first` to `last`, or `first` alone
/// when the two are in different sources or in reverse order.
fn covering(first: &FlmSpan, last: &FlmSpan) -> FlmSpan {
    if first.same_source(last) && first.start() <= last.end() {
        SourceSpan::new(first.source(), first.start()..last.end())
    } else {
        first.clone()
    }
}

/// Copies `nodes` into a new value tree and returns it: a list node, spanning
/// the whole run, holding a copy of each node. This is
/// [`copy_pieces_into_value`] with every piece a node.
///
/// `nodes` must be nodes of `anchor`'s tree. `anchor` also supplies the span
/// and the parsing state when `nodes` is empty: the root then gets an empty
/// span at `anchor`'s start. Each copy keeps the span, source and parsing state
/// of the node it was made from. `content` is the purpose of the copy
/// ([`ValueContent`]), which decides whether each node is copied unchanged or
/// as its element says.
///
/// The traversal runs under the recursion guard of `guard` and does not check
/// its thread: call this function on a worker thread
/// ([`on_worker_thread`](crate::lang::guard::on_worker_thread)), as the example
/// does.
///
/// # Errors
///
/// [`CopyError::NestingTooDeep`] when the content is nested deeper than `guard`
/// allows; [`CopyError::Failed`] when building the copy fails, which means the
/// arguments did not meet this function's requirements.
///
/// # Examples
///
/// The copy is guarded, so the example does its work on a worker thread, as
/// every caller must.
///
/// ```
/// use std::sync::Arc;
/// use flm_core::model::doc::FragmentLayout;
/// use flm_core::lang::copy::{copy_run_into_value, CopyError, ValueContent};
/// use flm_core::lang::guard::run_with_worker_stack;
/// use flm_core::lang::{
///     build_language, parse_fragment, FlmOrigin, FlmSource, GuardPolicy, LangInputs,
///     ParseOptions,
/// };
///
/// let lang = build_language(LangInputs::new());
/// let source = Arc::new(FlmSource::new("ab {cd}").with_origin(FlmOrigin::none()));
/// let fragment = parse_fragment(
///     &lang, source, FragmentLayout::default(), &ParseOptions::default(),
/// ).expect("a parse");
///
/// run_with_worker_stack(|| -> Result<(), CopyError> {
///     let root = fragment.tree().root();
///     let run: Vec<_> = root.children().iter().collect();
///     let value = copy_run_into_value(&run, &root, ValueContent::Own, GuardPolicy::StackBudget)?;
///
///     // A list root over one copy per node, each keeping its original span.
///     assert!(value.root().is_list());
///     assert_eq!(value.root().child_count(), run.len());
///     assert_eq!(value.root().children().iter().next().unwrap().span(), run[0].span());
///     Ok(())
/// }).expect("a worker thread")?;
/// # Ok::<(), CopyError>(())
/// ```
pub fn copy_run_into_value(
    nodes: &[NodeRef<'_, Flm>],
    anchor: &NodeRef<'_, Flm>,
    content: ValueContent,
    guard: GuardPolicy,
) -> Result<Arc<NodeTree<Flm>>, CopyError> {
    let pieces: Vec<ValuePiece<'_, '_>> = nodes.iter().copied().map(ValuePiece::Node).collect();
    copy_pieces_into_value(&pieces, anchor, content, guard)
}

/// Copies `pieces` (value pieces, [`ValuePiece`]) into a new value tree and
/// returns it: a list node holding, in order, a copy of each node with its
/// subtree, a new text node for each text, and a new node for each [synthesized
/// piece](SynthesizedPiece) and [command piece](CommandPiece), holding the copy
/// of that piece's own pieces.
///
/// The root spans the pieces that have a place in a source (nodes, synthesized
/// pieces and command pieces), from the first to the last, or is an empty span
/// at `anchor`'s start when there is none. A text piece has no place in the
/// document and gets an empty span at `anchor`'s start. The nodes may belong to
/// any tree (the tree a node was parsed in, or a tree derived from it, such as
/// the one techy's key-value reader builds), and each copy keeps the span,
/// source, parsing state and element of the node it was made from.
///
/// `content` is the purpose of the copy ([`ValueContent`]). For a copy of the
/// value's own, every node is copied unchanged. For a copy shown again, each
/// node (including the nodes inside the pieces of a synthesized or command
/// piece) is copied as its element says ([`copied_as`]): itself over the copies
/// of its children, nothing, or one new node in its place, at its span and
/// under its parsing state. The node of a synthesized or command piece itself
/// is added as given: the elements are asked only about nodes copied out of a
/// tree, not about the nodes the caller creates.
///
/// Call this function on a worker thread, as for [`copy_run_into_value`].
///
/// # Errors
///
/// [`CopyError::NestingTooDeep`] when the content is nested deeper than `guard`
/// allows; [`CopyError::Failed`] when building the copy fails, which is an
/// error in the program, not something a document causes.
pub fn copy_pieces_into_value(
    pieces: &[ValuePiece<'_, '_>],
    anchor: &NodeRef<'_, Flm>,
    content: ValueContent,
    guard: GuardPolicy,
) -> Result<Arc<NodeTree<Flm>>, CopyError> {
    let mut visitor = CopyRun { pieces, anchor: *anchor, content };
    let tree = guard::restager(&mut visitor, guard).restage(anchor.tree())?;
    Ok(Arc::new(tree))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::doc::FragmentLayout;
    use crate::lang::{FlmOrigin, LangInputs, ParseOptions, build_language, parse_fragment};
    use techy::source::Source;

    #[test]
    fn a_copied_run_becomes_a_list_root_over_copies_keeping_their_spans() {
        let lang = build_language(LangInputs::new());
        let source = Arc::new(Source::new("ab {cd}").with_origin(FlmOrigin::none()));
        let fragment =
            parse_fragment(&lang, source, FragmentLayout::default(), &ParseOptions::default())
                .expect("a parse");
        let root = fragment.tree().root();
        let nodes: Vec<_> = root.children().iter().collect();
        let value = copy_run_into_value(&nodes, &root, ValueContent::Own, GuardPolicy::StackBudget).expect("copied");
        let copied = value.root();
        assert!(copied.is_list());
        assert_eq!(copied.child_count(), nodes.len());
        let spans: Vec<_> = copied.children().iter().map(|n| n.span().clone()).collect();
        let originals: Vec<_> = nodes.iter().map(|n| n.span().clone()).collect();
        assert_eq!(spans, originals);
    }

    #[test]
    fn a_text_piece_becomes_a_text_node_beside_the_copied_nodes() {
        let lang = build_language(LangInputs::new());
        let source = Arc::new(Source::new("ab {cd}").with_origin(FlmOrigin::none()));
        let fragment =
            parse_fragment(&lang, source, FragmentLayout::default(), &ParseOptions::default())
                .expect("a parse");
        let root = fragment.tree().root();
        let group = root.children().iter().next_back().expect("the group");
        let pieces = [ValuePiece::Text("xy"), ValuePiece::Node(group)];
        let value = copy_pieces_into_value(&pieces, &root, ValueContent::Own, GuardPolicy::StackBudget).expect("copied");
        let children: Vec<_> = value.root().children().iter().collect();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].chars(), Some("xy"));
        assert!(children[1].is_group());
        assert_eq!(value.root().span(), group.span());
    }

    #[test]
    fn a_synthesized_piece_becomes_a_fresh_node_holding_its_own_pieces_in_its_slot() {
        use crate::model::element::ElementSpec;
        let lang = build_language(LangInputs::new());
        let source = Arc::new(Source::new("ab {cd}").with_origin(FlmOrigin::none()));
        let fragment =
            parse_fragment(&lang, source, FragmentLayout::default(), &ParseOptions::default())
                .expect("a parse");
        let root = fragment.tree().root();
        let group = root.children().iter().next_back().expect("the group");
        let spec = Arc::new(FlmSynthesizedSpec::new(Arc::new(ElementSpec::new("t.box"))));
        let span = SourceSpan::new(group.span().source(), 1..2);
        let boxed = SynthesizedPiece {
            spec,
            slot: "body",
            span: span.clone(),
            state: Arc::clone(root.parsing_state()),
            body: vec![ValuePiece::Text("xy"), ValuePiece::Node(group)],
            payload: None,
        };
        let value = copy_pieces_into_value(&[ValuePiece::Synthesized(&boxed)], &root, ValueContent::Own, GuardPolicy::StackBudget)
            .expect("copied");
        // The root spans the synthesized node, which holds the text — at its
        // own start, not the anchor's — and the copied group in its slot.
        assert_eq!(value.root().span(), &span);
        let node = value.root().children().iter().next().expect("the node");
        assert_eq!(node.ext().element.name.as_str(), "t.box");
        let body: Vec<_> = node.children().iter().collect();
        assert_eq!(body.len(), 2);
        assert_eq!(body[0].chars(), Some("xy"));
        assert_eq!(body[0].span().start(), 1);
        assert!(body[1].is_group());
        assert_eq!(body[1].span(), group.span());
    }

    #[test]
    fn a_synthesized_piece_carries_its_payload_and_may_hold_nothing() {
        use crate::model::element::ElementSpec;
        use techy::serialize::SerialValue;
        #[derive(Debug)]
        struct Named(&'static str);
        impl PresPayload for Named {
            fn project(&self) -> SerialValue {
                SerialValue::Str(self.0.to_string())
            }
        }
        let lang = build_language(LangInputs::new());
        let source = Arc::new(Source::new("ab").with_origin(FlmOrigin::none()));
        let fragment =
            parse_fragment(&lang, source, FragmentLayout::default(), &ParseOptions::default())
                .expect("a parse");
        let root = fragment.tree().root();
        let spec = Arc::new(FlmSynthesizedSpec::new(Arc::new(ElementSpec::new("t.mark"))));
        let marked = SynthesizedPiece {
            spec,
            slot: "body",
            span: root.span().clone(),
            state: Arc::clone(root.parsing_state()),
            body: Vec::new(),
            payload: Some(Arc::new(Named("here"))),
        };
        let value = copy_pieces_into_value(&[ValuePiece::Synthesized(&marked)], &root, ValueContent::Own, GuardPolicy::StackBudget)
            .expect("built");
        let node = value.root().children().iter().next().expect("the node");
        assert_eq!(node.child_count(), 0);
        let payload = node.ext().payload.as_deref().expect("a payload");
        assert_eq!(payload.downcast_ref::<Named>().map(|named| named.0), Some("here"));
    }

    #[test]
    fn an_empty_run_becomes_an_empty_list_at_the_anchor_s_start() {
        let lang = build_language(LangInputs::new());
        let source = Arc::new(Source::new("x").with_origin(FlmOrigin::none()));
        let fragment =
            parse_fragment(&lang, source, FragmentLayout::default(), &ParseOptions::default())
                .expect("a parse");
        let root = fragment.tree().root();
        let value = copy_run_into_value(&[], &root, ValueContent::Own, GuardPolicy::StackBudget).expect("copied");
        assert_eq!(value.root().child_count(), 0);
        assert_eq!(value.root().span().start(), root.span().start());
    }

    /// A language where `\gone{…}` answers nothing in a copy shown again and
    /// `\held{…}` a stand-in of the element `t.stand-in` carrying the payload
    /// `Named("held")`.
    mod asking {
        use super::*;
        use crate::lang::mint::{ArgDef, ElementDef, SyntaxDef};
        use crate::lang::BuiltLang;
        use crate::model::aspects::{InCopy, StandIn};
        use crate::model::element::ElementSpec;
        use techy::serialize::SerialValue;

        #[derive(Debug)]
        struct Named(&'static str);

        impl PresPayload for Named {
            fn project(&self) -> SerialValue {
                SerialValue::Str(self.0.to_string())
            }
        }

        struct Gone;

        impl InCopy for Gone {
            fn in_copy(&self, _node: &NodeRef<'_, Flm>, _purpose: ValueContent) -> CopiedAs {
                CopiedAs::Nothing
            }
        }

        struct Held;

        impl InCopy for Held {
            fn in_copy(&self, _node: &NodeRef<'_, Flm>, _purpose: ValueContent) -> CopiedAs {
                let spec = FlmSynthesizedSpec::new(Arc::new(ElementSpec::new("t.stand-in")));
                CopiedAs::StandIn(StandIn { spec: Arc::new(spec), payload: Some(Arc::new(Named("held"))) })
            }
        }

        fn language() -> BuiltLang {
            let mut inputs = LangInputs::new();
            let aspects: [(&str, Arc<dyn InCopy>); 2] = [("gone", Arc::new(Gone)), ("held", Arc::new(Held))];
            for (name, aspect) in aspects {
                inputs.elements.push(ElementDef::new(
                    "t",
                    [name],
                    SyntaxDef::Macro { args: vec![ArgDef::mandatory("text")] },
                    Arc::new(ElementSpec::new(format!("t.{name}")).with::<dyn InCopy>(aspect)),
                ));
            }
            build_language(inputs)
        }

        fn parse(lang: &BuiltLang, text: &str) -> crate::model::doc::Fragment {
            let source = Arc::new(Source::new(text).with_origin(FlmOrigin::none()));
            parse_fragment(lang, source, FragmentLayout::Inline, &ParseOptions::default()).expect("a parse")
        }

        /// `node` and its descendants, in document order.
        fn subtree<'t>(node: &NodeRef<'t, Flm>) -> Vec<NodeRef<'t, Flm>> {
            std::iter::once(*node).chain(node.descendants()).collect()
        }

        /// The element names of `node`'s subtree, `node` included, in document
        /// order.
        fn elements(node: &NodeRef<'_, Flm>) -> Vec<String> {
            subtree(node).iter().map(|node| node.ext().element.name.to_string()).collect()
        }

        #[test]
        fn a_copy_shown_again_copies_each_node_as_its_element_answers_at_every_depth() {
            let lang = language();
            let fragment = parse(&lang, r"a\gone{b}{c\held{d}}");
            let root = fragment.tree().root();
            let nodes: Vec<_> = root.children().iter().collect();
            let value = copy_run_into_value(&nodes, &root, ValueContent::Shown, GuardPolicy::StackBudget)
                .expect("copied");
            let copied: Vec<_> = value.root().children().iter().collect();
            // The text and the group are copied, keeping their spans; `\gone`
            // is not there at all.
            assert_eq!(copied.len(), 2);
            assert_eq!(copied[0].chars(), Some("a"));
            assert_eq!(copied[0].span(), nodes[0].span());
            assert!(copied[1].is_group());
            assert_eq!(copied[1].span(), nodes[2].span());
            // Inside the group, `\held` is one fresh node of the stand-in
            // element, at the original's span, holding no child and carrying
            // the payload; what `\held` held is not copied.
            let inner: Vec<_> = copied[1].children().iter().collect();
            assert_eq!(inner.len(), 2);
            let original = nodes[2].children().iter().nth(1).expect("the original");
            assert_eq!(inner[1].ext().element.name.as_str(), "t.stand-in");
            assert_eq!(inner[1].span(), original.span());
            assert_eq!(inner[1].child_count(), 0);
            let payload = inner[1].ext().payload.as_deref().expect("a payload");
            assert_eq!(payload.downcast_ref::<Named>().map(|named| named.0), Some("held"));
            assert!(!elements(&value.root()).iter().any(|name| name == "t.gone" || name == "t.held"));
        }

        #[test]
        fn a_copy_of_the_value_s_own_is_verbatim_whatever_the_elements_answer() {
            let lang = language();
            let fragment = parse(&lang, r"a\gone{b}{c\held{d}}");
            let root = fragment.tree().root();
            let nodes: Vec<_> = root.children().iter().collect();
            let value = copy_run_into_value(&nodes, &root, ValueContent::Own, GuardPolicy::StackBudget)
                .expect("copied");
            let originals: Vec<String> = nodes.iter().flat_map(elements).collect();
            let copied: Vec<String> = value.root().children().iter().flat_map(|node| elements(&node)).collect();
            assert_eq!(copied, originals);
            let spans: Vec<_> = value.root().descendants().map(|node| node.span().clone()).collect();
            let original_spans: Vec<_> =
                nodes.iter().flat_map(subtree).map(|node| node.span().clone()).collect();
            assert_eq!(spans, original_spans);
        }

        #[test]
        fn a_copy_shown_again_asks_the_pieces_of_a_synthesized_piece_and_stages_the_piece_as_given() {
            let lang = language();
            let fragment = parse(&lang, r"\held{x}\gone{y}");
            let root = fragment.tree().root();
            let nodes: Vec<_> = root.children().iter().collect();
            let spec = Arc::new(FlmSynthesizedSpec::new(Arc::new(ElementSpec::new("t.box"))));
            let boxed = SynthesizedPiece {
                spec,
                slot: "body",
                span: root.span().clone(),
                state: Arc::clone(root.parsing_state()),
                body: nodes.iter().copied().map(ValuePiece::Node).collect(),
                payload: None,
            };
            let value = copy_pieces_into_value(
                &[ValuePiece::Synthesized(&boxed)],
                &root,
                ValueContent::Shown,
                GuardPolicy::StackBudget,
            )
            .expect("copied");
            let node = value.root().children().iter().next().expect("the box");
            assert_eq!(node.ext().element.name.as_str(), "t.box");
            let body: Vec<String> = node.children().iter().map(|n| n.ext().element.name.to_string()).collect();
            assert_eq!(body, ["t.stand-in"]);
        }

        #[test]
        fn the_recursion_guard_answers_over_a_copy_shown_again() {
            let lang = language();
            let depth = 40;
            let text = format!("{}x{}", "{".repeat(depth), "}".repeat(depth));
            let fragment = parse(&lang, &text);
            let root = fragment.tree().root();
            let nodes: Vec<_> = root.children().iter().collect();
            let shown = copy_run_into_value(&nodes, &root, ValueContent::Shown, GuardPolicy::DepthLimit(10));
            assert!(matches!(shown, Err(CopyError::NestingTooDeep { .. })), "{shown:?}");
            let own = copy_run_into_value(&nodes, &root, ValueContent::Own, GuardPolicy::DepthLimit(10));
            assert!(matches!(own, Err(CopyError::NestingTooDeep { .. })), "{own:?}");
        }
    }
}
