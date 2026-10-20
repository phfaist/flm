//! Including another file in a document: the command `\input{reference}`, the
//! interface through which the library asks the embedding program to read the
//! file ([`InputResolver`]), and the parser of the command.
//!
//! # What the embedding program provides, and what the library keeps
//!
//! The library performs no input or output, so it cannot read a file. The
//! embedding program supplies an **input resolver**, an object implementing
//! [`InputResolver`]. For each `\input`, the library calls it with the
//! reference as the document wrote it, the name of the file that holds the
//! command, and the parsing settings that apply there. The resolver finds the
//! file, reads it, splits off its front matter, reads from the front matter
//! the keys the library needs, and returns the file's content and those facts
//! as an [`IncludedFile`]. The embedding program sets the resolver for every
//! parse of a registry with
//! [`RegistryInputs::input_resolver`](crate::core::registry::RegistryInputs::input_resolver),
//! or for one parse with
//! [`ParseOptions::input_resolver`](crate::lang::ParseOptions::input_resolver);
//! the parse driver holds it
//! ([`FlmDriver::with_input_resolver`](crate::lang::FlmDriver::with_input_resolver)).
//! The `flm` program supplies a resolver that reads files from disk.
//!
//! The library keeps every fact about the document itself: which file
//! includes which (and therefore whether a file includes itself or a chain of
//! inclusions is too deep); the parsing settings of each file, which it passes
//! to the resolver with each call, and the parsing state the included content
//! is read under; and where the content goes in the tree.
//!
//! # Content inputs and heading inputs
//!
//! An included file is one of two kinds, as its front matter declares
//! ([`IncludedFile::heading`]):
//!
//! - A **content input** continues the text around the command. Its content is
//!   parsed where the command is written and becomes part of the including
//!   file's tree.
//! - A **heading input** opens with a heading, declared in its front matter
//!   ([`InputHeading`]). It is a part of the document with a place of its own.
//!   Only the assembly of a whole document ([`crate::model::assemble`]) can
//!   include it: the assembly parses the file as a tree of its own and places
//!   that tree at the command. A heading input may be included only once in a
//!   document.
//!
//! # What a parse makes of `\input`
//!
//! The [`input` feature](crate::stdcore::features::input) of the library's
//! standard features declares the command as an inclusion macro
//! ([`SyntaxDef::Inclusion`](crate::lang::mint::SyntaxDef::Inclusion)), which [minting](crate::lang::mint) builds as an [`InputSpec`]. Its parser
//! reads the argument named [`REFERENCE_ARGUMENT`] as plain characters, calls
//! the input resolver, and parses the content of a content input **where the
//! command is written**: under the parsing state that applies there, with the
//! file's own parsing settings ([`IncludedFile::parsing`]) on top. The state
//! records which settings it was derived from
//! ([`FlmStateExt::parsing`](crate::lang::FlmStateExt::parsing)), so that an
//! `\input` inside the included file passes the resolver the settings that
//! apply there. No change the included content makes to the parsing state
//! applies after the command.
//!
//! The parsed content becomes the node slot named [`ATTACHED_SLOT`] of the
//! command's node. A node slot (techy's `ParsedSlot`) is a part of a node's
//! content outside its arguments, such as an environment's body. The nodes in
//! that slot carry spans into the included file, and the command's node keeps
//! its span in the including file. Before any other
//! rewrite pass runs, the input rewrite pass ([`crate::lang::rewrites::input`])
//! replaces the node by that content, so that a paragraph begun in one file
//! may end in the other, and later passes and stages need not know that the
//! content came from another file.
//!
//! The node of a heading input's command has an empty slot named
//! [`PLACED_SLOT`] instead ([`is_placing`]), and the assembly places the file's
//! tree there.
//!
//! # When the file cannot be included
//!
//! The parser reports the problem at the command, and the command's node
//! includes nothing, when:
//!
//! - the reference is not plain characters ([`InputReferenceNotPlain`]);
//! - no input resolver is set ([`InputUnavailable`]);
//! - the resolver could not find or read the file ([`InputUnreadable`]);
//! - the file is already being included further up the chain
//!   ([`InputIncludesItself`]);
//! - the chain of inclusions is deeper than [`DEPTH_LIMIT`] ([`InputTooDeep`]);
//! - the command is written in a [user
//!   definition](crate::lang::rewrites::userdefs::Definition)'s own text (its
//!   template or an argument's default value) ([`InputInDefinition`]);
//! - for a heading input: the parse is not the assembly of a whole document
//!   ([`InputHeadingOutsideDocument`]), the heading command is not a command
//!   name ([`InputHeadingCommandInvalid`]), or the file was already included
//!   in the document ([`InputPlacedTwice`]).
//!
//! Two problems are reported and the file is still included: a problem the
//! resolver found in the file's front matter, reported at its place in the
//! included file ([`IncludedFrontMatterProblem`]), and, for a heading input, a
//! label holding a brace ([`InputHeadingLabelInvalid`]), after which the
//! heading carries no label.

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use techy::core::constructs::{
    parse_declared_arguments, ChildStateSpec, ConstructParser, ConstructParserResult, Invocation,
    ParseContext, PlainCharsError, StopSpec,
};
use techy::core::node::{
    BuildId, ChildRegion, ContentNodes, NodeKind, NodeRef, ParsedArguments, ParsedSlot,
    ParsedSlots, SlotRole,
};
use techy::core::specs::{ArgumentSpec, CallableSpec};
use techy::core::token::TokenEdge;
use techy::core::{ParseDriver, ParsingState, ParsingStateDelta};
use techy::error::{DiagnosticInfo, ParseError};
use techy::latexlike::BodyMarker;
use techy::serialize::SerializableObject;
use techy::source::SourceSpan;

use crate::lang::flm::Flm;
use crate::lang::mint::specs::FlmCallable;
use crate::lang::origin::{FlmOrigin, FlmSource, FlmSpan};
use crate::lang::rules::FlmParsing;

/// The name of the argument from which an inclusion macro reads the
/// reference: `reference`.
pub const REFERENCE_ARGUMENT: &str = "reference";

/// The name of the node slot (techy's `ParsedSlot`, a part of a node's content
/// outside its arguments, such as an environment's body) that holds the
/// included content on the node of an `\input` of a content input: `attached`.
pub const ATTACHED_SLOT: &str = "attached";

/// The name of the node slot (techy's `ParsedSlot`, a part of a node's content
/// outside its arguments, such as an environment's body), always empty, that
/// marks the node of an `\input` whose file is a heading input placed as a
/// tree of its own: `placed` ([`is_placing`]).
pub const PLACED_SLOT: &str = "placed";

/// The maximum depth of a chain of inclusions: 32. The primary file is at depth
/// zero and a file it includes at depth one; a file at a greater depth than
/// this is not included ([`InputTooDeep`]).
pub const DEPTH_LIMIT: usize = 32;

// --- what the library asks of a host ----------------------------------------------------

/// What an input resolver returns for one `\input`: the content of the
/// included file and what the resolver read about it.
///
/// An [`InputResolver`] builds it. Every field is a fact about the file that the
/// library cannot find out itself, since it reads no file and no front
/// matter.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IncludedFile {
    /// The name under which the file is known: what messages print, and what
    /// tells two files apart when the library checks that no file includes
    /// itself. Two references to the same file must give it the same name.
    pub name: String,
    /// The file's content, its front matter removed.
    pub text: String,
    /// The line of the file, counted from one, on which
    /// [`text`](Self::text) begins (one when the file has no front matter), so
    /// that a problem in the content is reported at its line in the file.
    pub first_line: usize,
    /// The parsing settings the content is read under, when the file sets any
    /// of its own: the settings that apply where the command is written (which
    /// the library passes to [`InputResolver::resolve`]), with the keys the file
    /// sets replaced, as one complete value. `None` when the file sets none:
    /// the content is then read under the parsing state that applies where the
    /// command is written, unchanged.
    pub parsing: Option<FlmParsing>,
    /// The problems the resolver found in the file's front matter, each at its
    /// place in the file. They are reported, and the content is still
    /// included.
    pub problems: Vec<IncludedFileProblem>,
    /// The heading the file opens with, when its front matter declares it a
    /// **heading input**; `None` for a **content input**, whose content
    /// continues the text around the command. The [module
    /// documentation](self#content-inputs-and-heading-inputs) describes both
    /// kinds.
    pub heading: Option<InputHeading>,
}

/// The heading a heading input opens with, as the input resolver read it from
/// the file's front matter ([`IncludedFile::heading`]).
///
/// Before the file's content, the library writes the heading command given
/// here applied to the title (`\section{Title}`, or `\section*{Title}` when
/// the heading is not numbered), followed by `\label{label}` when a label is
/// given. The library checks only that this text forms one command with its
/// arguments: the command must be a command name
/// ([`InputHeadingCommandInvalid`]), and the label must hold no brace
/// ([`InputHeadingLabelInvalid`]). The command decides what it makes of the
/// heading (its level, its number); the library checks nothing of it. A problem
/// in the title is reported at its place in the file, given by
/// [`title_at`](Self::title_at).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputHeading {
    /// The name of the heading command, without its backslash: `section`.
    pub command: String,
    /// The title, as FLM source: `The \emph{Art}`.
    pub title: String,
    /// Where the title is written in the file — its line and column, counted
    /// from one — so that a problem in it is reported there; `None` when the
    /// file writes no title.
    pub title_at: Option<(usize, usize)>,
    /// The label the heading carries, when one is given.
    pub label: Option<String>,
    /// Whether the heading is numbered; `false` writes the command's starred
    /// form.
    pub numbered: bool,
}

/// A problem an input resolver found in the front matter of an included file,
/// at its place in that file ([`IncludedFile::problems`]). The library reports
/// it as the condition [`IncludedFrontMatterProblem`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludedFileProblem {
    /// The line, counted from one.
    pub line: usize,
    /// The column, counted from one.
    pub column: usize,
    /// The message that reports the problem. Like the library's own messages,
    /// it should state the violated condition directly.
    pub message: String,
}

/// The error an input resolver returns when it cannot provide the file of an
/// `\input`: the file does not exist or cannot be read. The library reports it as the
/// condition [`InputUnreadable`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputError {
    /// What went wrong, including the file that was looked for.
    pub message: String,
}

impl InputError {
    /// Returns an error with `message`.
    pub fn new(message: impl Into<String>) -> InputError {
        InputError { message: message.into() }
    }
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for InputError {}

/// An input resolver: the object the embedding program implements so that the
/// library can include files with `\input`. It finds a file from a reference,
/// reads it, and reads the facts of [`IncludedFile`] from its front matter.
///
/// Set it for every parse of a registry with
/// [`RegistryInputs::input_resolver`](crate::core::registry::RegistryInputs::input_resolver),
/// or for one parse with
/// [`ParseOptions::input_resolver`](crate::lang::ParseOptions::input_resolver).
/// Without one, every `\input` is reported ([`InputUnavailable`]) and includes
/// nothing. The [module documentation](self) describes the whole exchange.
///
/// The library calls [`resolve`](Self::resolve) once for each `\input` it
/// parses, with three arguments:
///
/// - `reference`, exactly as the document wrote it. The resolver alone decides
///   how it is turned into a path (usually relative to the including file,
///   which `including` identifies).
/// - `including`, the [name](IncludedFile::name) of the file that holds the
///   command: for the primary file, the canonical path of its source's
///   [origin](crate::lang::FlmOrigin); `None` for content that comes from no
///   file.
/// - `parsing`, the parsing settings that apply where the command is written:
///   the document's in the primary file, and in an included file the settings
///   it is read under, whether it set settings of its own or not.
///
/// The library keeps track of which file includes which, and of the settings
/// each file is read under, so the resolver needs no memory of earlier calls.
/// When the file sets parsing settings of its own, the resolver returns them
/// applied over `parsing`, as one complete value ([`IncludedFile::parsing`]).
///
/// A resolver is shared across threads, so it must be `Send` and `Sync`; a
/// resolver that records the files it read keeps the record behind a lock.
///
/// # Examples
///
/// A resolver that serves files held in memory. It fills in the name, the
/// content and the first line of each file, and leaves the other facts of
/// [`IncludedFile`] at their defaults (no parsing settings of its own, no
/// front-matter problem, a content input):
///
/// ```
/// use std::collections::HashMap;
///
/// use flm_core::lang::{FlmParsing, IncludedFile, InputError, InputResolver};
///
/// struct InMemory {
///     files: HashMap<String, String>,
/// }
///
/// impl InputResolver for InMemory {
///     fn resolve(
///         &self,
///         reference: &str,
///         _including: Option<&str>,
///         _parsing: &FlmParsing,
///     ) -> Result<IncludedFile, InputError> {
///         let text = self
///             .files
///             .get(reference)
///             .ok_or_else(|| InputError::new(format!("no file named ‘{reference}’")))?;
///         Ok(IncludedFile {
///             name: reference.to_string(),
///             text: text.clone(),
///             first_line: 1,
///             ..Default::default()
///         })
///     }
/// }
///
/// let resolver = InMemory {
///     files: HashMap::from([(String::from("intro.flm"), String::from("Hello."))]),
/// };
/// let file = resolver.resolve("intro.flm", None, &FlmParsing::default()).unwrap();
/// assert_eq!(file.text, "Hello.");
/// assert!(resolver.resolve("missing.flm", None, &FlmParsing::default()).is_err());
/// ```
pub trait InputResolver: Send + Sync {
    /// Finds and reads the file that `reference` refers to, and returns its
    /// content and front-matter facts.
    ///
    /// # Errors
    ///
    /// An [`InputError`] when no file is found or it cannot be read; the
    /// library reports it at the command ([`InputUnreadable`]).
    fn resolve(
        &self,
        reference: &str,
        including: Option<&str>,
        parsing: &FlmParsing,
    ) -> Result<IncludedFile, InputError>;
}

// --- the conditions -------------------------------------------------------------------

/// Condition `flm.input.reference-not-plain`: the reference of an `\input`
/// holds something other than plain characters (a group, a command, a
/// comment).
///
/// Reported at error severity at the command. Recovery: no file is included.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.reference-not-plain",
    message = "the file name given to \\input must be plain text, without commands, groups \
               or comments"
)]
pub struct InputReferenceNotPlain;

/// Condition `flm.input.unavailable`: an `\input` was parsed without an input
/// resolver, so no file can be read.
///
/// Reported at error severity at the command. Recovery: no file is included.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.unavailable",
    message = "\\input{{{reference}}} requires a program that reads files, and the program \
               processing this document reads none"
)]
pub struct InputUnavailable {
    /// The reference, as written.
    pub reference: String,
}

/// Condition `flm.input.unreadable`: the input resolver found no file for the
/// reference, or could not read it ([`InputError`]).
///
/// Reported at error severity at the command. Recovery: no file is included.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.unreadable",
    message = "the file that \\input{{{reference}}} refers to must exist and be readable: \
               {detail}"
)]
pub struct InputUnreadable {
    /// The reference, as written.
    pub reference: String,
    /// The resolver's message, which includes the file that was looked for.
    pub detail: String,
}

/// Condition `flm.input.includes-itself`: the file an `\input` refers to is
/// already being included further up the chain of inclusions, so including it
/// again would never end.
///
/// Reported at error severity at the command. Recovery: the file is not
/// included again.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.includes-itself",
    message = "a file must not include itself, directly or through other files: {file} is \
               already being included"
)]
pub struct InputIncludesItself {
    /// The name of the file.
    pub file: String,
}

/// Condition `flm.input.too-deep`: the chain of inclusions that leads to this
/// `\input` is already [`DEPTH_LIMIT`] files deep.
///
/// Reported at error severity at the command. Recovery: no file is included.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.too-deep",
    message = "files may be included at most {limit} levels deep, and including {file} here \
               would exceed that limit"
)]
pub struct InputTooDeep {
    /// The limit, [`DEPTH_LIMIT`].
    pub limit: usize,
    /// The name of the file that this `\input` would include.
    pub file: String,
}

/// Condition `flm.input.in-definition`: an `\input` is written in a [user
/// definition](crate::lang::rewrites::userdefs::Definition)'s own text (its
/// template or an argument's default value), where no file can be included.
///
/// Reported at error severity at the command. Recovery: no file is included.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.in-definition",
    message = "\\input must not appear in the template or default argument values of a \
               user-defined command or environment"
)]
pub struct InputInDefinition;

/// Condition `flm.input.front-matter`: a problem the input resolver found in
/// the front matter of an included file ([`IncludedFileProblem`]), reported at
/// its place in that file. The message is the resolver's.
///
/// Reported at error severity. Recovery: the file is still included.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.input.front-matter", message = "{message}")]
pub struct IncludedFrontMatterProblem {
    /// The resolver's message.
    pub message: String,
}

/// Condition `flm.input.placed-twice`: a heading input is included a second
/// time in one document. A heading input is a part of the document with a
/// place of its own, so it can be placed only once.
///
/// Reported at error severity at the second command. Recovery: the second
/// command includes nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.placed-twice",
    message = "a file that opens with a heading can be included only once: {file} is already \
               included"
)]
pub struct InputPlacedTwice {
    /// The name of the file.
    pub file: String,
}

/// Condition `flm.input.heading-outside-document`: a file that opens with a
/// heading is included by a parse that does not assemble a whole document
/// ([`crate::model::assemble`]), so there is no document to place it in.
///
/// Reported at error severity at the command. Recovery: no file is included.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.heading-outside-document",
    message = "a file that opens with a heading can be included only in a whole document: \
               {file} opens with a heading"
)]
pub struct InputHeadingOutsideDocument {
    /// The name of the file.
    pub file: String,
}

/// Condition `flm.input.placement-lost`: the assembly of a document found no
/// node for the `\input` of a heading input that its parse recorded, so the
/// file's tree has no place. The parse adds that node to the tree right after
/// recording the file, so this reports an error in the library, not in the
/// document.
///
/// Reported at error severity by the assembly ([`crate::model::assemble`]), at
/// the command. Recovery: the file's tree is not placed.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.placement-lost",
    message = "the \\input that includes {file}, a file that opens with a heading, must \
               remain in the parsed document for the file to be placed there; this is an \
               error in the program processing the document"
)]
pub struct InputPlacementLost {
    /// The name of the file.
    pub file: String,
}

/// Condition `flm.input.heading-command`: the heading command that a heading
/// input specifies ([`InputHeading::command`], which the input resolver reads
/// from the file's front matter) is not a command name: it is empty, or it holds a character a command name cannot hold.
///
/// Reported at error severity at the command. Recovery: no file is included.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.heading-command",
    message = "the heading command of {file} must be a command name without its backslash: \
               ‘{command}’ is not a command name"
)]
pub struct InputHeadingCommandInvalid {
    /// The name of the file.
    pub file: String,
    /// The heading command as written.
    pub command: String,
}

/// Condition `flm.input.heading-label`: the label that a heading input gives
/// its heading ([`InputHeading::label`], which the input resolver reads from
/// the file's front matter) holds a brace.
///
/// Reported at error severity at the command. Recovery: the file is included,
/// and its heading carries no label.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.heading-label",
    message = "the heading label of {file} must not hold a brace: ‘{label}’"
)]
pub struct InputHeadingLabelInvalid {
    /// The name of the file.
    pub file: String,
    /// The label as written.
    pub label: String,
}

// --- the record of heading inputs -------------------------------------------------------

/// One heading input found by a parse: what the assembly of the document needs
/// to parse the file as a tree of its own and place it at its command.
#[derive(Debug, Clone)]
pub(crate) struct PlacedInput {
    /// The span of the `\input` command, in the including file: the assembly
    /// finds the node that places the tree by this span.
    pub(crate) site: FlmSpan,
    /// The name the input resolver gave the file.
    pub(crate) name: String,
    /// The file's content, as a source included from the command.
    pub(crate) source: Arc<FlmSource>,
    /// The heading the file opens with.
    pub(crate) heading: InputHeading,
    /// The parsing state the file is read under: the state at the command,
    /// with the file's own parsing settings on top.
    pub(crate) state: Arc<ParsingState<Flm>>,
}

/// The heading inputs found while one document is assembled, in the order the
/// parses found them. The assembly sets one on the parse driver
/// ([`FlmDriver::with_placements`](crate::lang::FlmDriver)) and reads it back
/// after each parse.
#[derive(Debug, Default)]
pub(crate) struct Placements {
    records: std::sync::Mutex<Vec<PlacedInput>>,
}

impl Placements {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<PlacedInput>> {
        self.records.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Returns how many heading inputs were found so far.
    pub(crate) fn len(&self) -> usize {
        self.lock().len()
    }

    /// Returns the heading inputs found from the `from`-th on, up to the
    /// `to`-th.
    pub(crate) fn range(&self, from: usize, to: usize) -> Vec<PlacedInput> {
        self.lock().get(from..to).map(<[PlacedInput]>::to_vec).unwrap_or_default()
    }

    /// Records `placed`, unless a heading input of the same name was found
    /// before; returns whether it was recorded.
    fn record(&self, placed: PlacedInput) -> bool {
        let mut records = self.lock();
        if records.iter().any(|known| known.name == placed.name) {
            return false;
        }
        records.push(placed);
        true
    }
}

/// Returns the source of the heading a heading input opens with, with the
/// file's name as its origin, included from the command that includes the
/// file: `\COMMAND` (`\COMMAND*` when not numbered) on a first line, then
/// `{title}`, `\label{label}` and a line end on a second, so that the title's
/// first character is reported at the title's line and column in the file (the
/// line and column offsets of a source apply to every one of its lines).
///
/// The command name and the label were checked when the file was recorded
/// ([`attach`]), so the text holds exactly one command and one argument each.
pub(crate) fn heading_source(placed: &PlacedInput) -> Arc<FlmSource> {
    let heading = &placed.heading;
    let star = if heading.numbered { "" } else { "*" };
    let mut text = format!("\\{}{star}\n{{{}}}", heading.command, heading.title);
    if let Some(label) = &heading.label {
        text.push_str(&format!("\\label{{{label}}}"));
    }
    // The heading ends its line, as it would in a file.
    text.push('\n');
    // The title stands on the source's second line, after the one-byte `{`.
    let (line, column) = match heading.title_at {
        Some((line, column)) => (line.saturating_sub(1), column.saturating_sub(1)),
        None => (1, 1),
    };
    let source = FlmSource::resolved(text, placed.name.as_str(), placed.site.clone())
        .with_origin(FlmOrigin::new(placed.name.clone()))
        .with_line_column_number_offsets(line, column);
    Arc::new(source)
}

/// Returns whether `name` is read whole as a command name by the command rule
/// with the escape character `\` under `state`: not empty, and made only of
/// characters a command name may contain.
fn is_command_name(state: &ParsingState<Flm>, name: &str) -> bool {
    !name.is_empty()
        && state
            .rules()
            .command_rules()
            .iter()
            .any(|rule| rule.escape_char == '\\' && name.chars().all(|c| rule.name_chars.contains(c)))
}

/// The root parser of a heading input's own tree: the heading first, from
/// [`heading_source`], then the file's content, each through techy's parse of
/// an included source, so that a problem in either is reported with the chain
/// of inclusions; the root spans the file's content.
pub(crate) struct PlacedRoot {
    pub(crate) heading: Arc<FlmSource>,
}

impl ConstructParser<Flm> for PlacedRoot {
    type Output = BuildId;

    fn parse(
        &mut self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, (BuildId, Option<Box<ParsingStateDelta<Flm>>>)> {
        let entry = Arc::clone(&cx.state);
        let source = Arc::clone(cx.here().source());
        let driver = cx.driver;
        let mut parser = driver
            .make_nodes_parser(StopSpec::none(), ChildStateSpec::inherit())
            .map_err(|error| cx.attach_hook_frames(error))?;
        let mut nodes = cx.parse_attached_source(Arc::clone(&self.heading), Arc::clone(&entry), &mut *parser)?.nodes;
        nodes.extend(cx.parse_attached_source(Arc::clone(&source), Arc::clone(&entry), &mut *parser)?.nodes);
        let whole = SourceSpan::entire(&source);
        let root = cx
            .stage_node(NodeKind::list(), whole.clone(), entry, nodes)
            .map_err(|error| cx.implementation_error(error, whole))?;
        Ok((root, None))
    }
}

// --- the specification ----------------------------------------------------------------

/// The specification of an inclusion macro: its declared arguments, one of
/// them named [`REFERENCE_ARGUMENT`], and the parser described in the [module
/// documentation](self).
///
/// [Minting](crate::lang::mint) builds it from
/// [`SyntaxDef::Inclusion`](crate::lang::mint::SyntaxDef::Inclusion) and wraps
/// it, like every macro, in an [`FlmCallable`] carrying the element. A feature
/// declares an inclusion macro and never builds this specification itself.
#[derive(Debug)]
pub struct InputSpec {
    arguments: Vec<Arc<ArgumentSpec<Flm>>>,
}

impl InputSpec {
    /// Returns the specification over `arguments`, in the order they are
    /// written.
    ///
    /// # Errors
    ///
    /// A description of the declared arguments when none of them is named
    /// [`REFERENCE_ARGUMENT`].
    pub fn new(arguments: Vec<Arc<ArgumentSpec<Flm>>>) -> Result<InputSpec, String> {
        if arguments.iter().any(|argument| argument.name.as_deref() == Some(REFERENCE_ARGUMENT)) {
            return Ok(InputSpec { arguments });
        }
        let names: Vec<String> = arguments
            .iter()
            .map(|argument| argument.name.as_deref().map_or_else(|| String::from("<unnamed>"), |n| format!("`{n}`")))
            .collect();
        Err(if names.is_empty() {
            String::from("it declares no argument")
        } else {
            format!("it declares {}", names.join(", "))
        })
    }
}

impl SerializableObject<Flm> for InputSpec {}

impl CallableSpec<Flm> for InputSpec {
    fn arguments(&self) -> &[Arc<ArgumentSpec<Flm>>] {
        &self.arguments
    }

    fn make_invocation_parser<'a>(
        &'a self,
        invocation: Invocation<'a, Flm>,
    ) -> Result<Box<dyn ConstructParser<Flm, Output = BuildId> + 'a>, ParseError<FlmOrigin>> {
        Ok(Box::new(InputParser { invocation }))
    }
}

/// Returns whether `node` is the node of an inclusion macro (an `\input`),
/// whether or not its file was included.
pub fn is_input(node: &NodeRef<'_, Flm>) -> bool {
    let Some(spec) = node.spec() else { return false };
    (&**spec as &dyn Any)
        .downcast_ref::<FlmCallable>()
        .is_some_and(|callable| (&**callable.syntax() as &dyn Any).is::<InputSpec>())
}

/// Returns whether `node` is the node of an `\input` whose file is a heading
/// input placed as a tree of its own: the node at which that tree is placed
/// ([`DocContent::resolve`](crate::model::doc::DocContent::resolve)).
pub fn is_placing(node: &NodeRef<'_, Flm>) -> bool {
    is_input(node) && node.slot_content_nodes_named(PLACED_SLOT).is_ok()
}

/// Returns the included content of `node`, the node of an `\input`: the nodes
/// of its [`ATTACHED_SLOT`], or `None` when `node` is not an `\input` or no
/// content input was included.
pub fn attached_content<'t>(node: &NodeRef<'t, Flm>) -> Option<Vec<NodeRef<'t, Flm>>> {
    if !is_input(node) {
        return None;
    }
    node.slot_content_nodes_named(ATTACHED_SLOT).ok().map(|nodes| nodes.iter().collect())
}

// --- the parser -----------------------------------------------------------------------

/// The parser of one `\input` invocation.
struct InputParser<'a> {
    invocation: Invocation<'a, Flm>,
}

impl ConstructParser<Flm> for InputParser<'_> {
    type Output = BuildId;

    fn parse(
        &mut self,
        cx: &mut ParseContext<'_, '_, Flm>,
    ) -> ConstructParserResult<Flm, (BuildId, Option<Box<ParsingStateDelta<Flm>>>)> {
        let token = self.invocation.token;
        let name = cx.tokens.source_span_between(token, TokenEdge::Start, TokenEdge::End);
        let (mut children, arguments) = parse_declared_arguments(cx, self.invocation.spec, &name)?;
        let end = cx.tokens.position_here();
        let at = cx.source_span_within(&cx.tokens.position_at(token, TokenEdge::Start), &end)?;

        let reference_argument = arguments
            .iter()
            .find(|argument| argument.name() == Some(REFERENCE_ARGUMENT))
            .ok_or_else(|| {
                cx.implementation_error("an inclusion macro without a reference argument", at.clone())
            })?;
        let reference = match cx.argument_content_as_plain_chars(reference_argument, &children) {
            Ok(text) => text,
            Err(PlainCharsError::NotPlainCharacters { .. }) => {
                cx.recover(InputReferenceNotPlain, at.clone())?;
                None
            }
            Err(error) => return Err(cx.implementation_error(error.to_string(), at.clone())),
        };

        let attached = match reference {
            Some(reference) => attach(cx, &reference, &at)?,
            None => Attached::Nothing,
        };
        let slots = match attached {
            Attached::Placed => {
                let offset = children.len() as u32;
                ParsedSlots::new(vec![ParsedSlot::new(
                    ChildRegion::new(offset..offset, ContentNodes::InRegion(0..0)),
                    PLACED_SLOT,
                    SlotRole::Hidden,
                    BodyMarker::not_body(),
                )])
            }
            Attached::Content(nodes) => {
                let offset = children.len() as u32;
                let count = nodes.len() as u32;
                children.extend(nodes);
                ParsedSlots::new(vec![ParsedSlot::new(
                    ChildRegion::new(offset..offset + count, ContentNodes::InRegion(0..count)),
                    ATTACHED_SLOT,
                    SlotRole::Attached,
                    BodyMarker::not_body(),
                )])
            }
            Attached::Nothing => ParsedSlots::empty(),
        };
        // `Some(&end)`: the node's span is the command in the including file;
        // the rule of the last child would reach into the included file.
        let id = cx.stage_invocation(
            &self.invocation,
            ParsedArguments::from(arguments),
            slots,
            children,
            Some(&end),
        )?;
        // No after-effect: nothing the included content does to the parsing
        // state reaches past the command.
        Ok((id, None))
    }
}

/// What became of the file of one `\input`.
enum Attached {
    /// A content input: its content, parsed where the command is written.
    Content(Vec<BuildId>),
    /// A heading input, recorded for the assembly of the document.
    Placed,
    /// Nothing was included; the problem was reported.
    Nothing,
}

/// Asks the input resolver for the file `reference` refers to and, for a
/// content input, parses its content where the command at `at` is written; a
/// heading input is recorded for the assembly of the document instead.
fn attach(
    cx: &mut ParseContext<'_, '_, Flm>,
    reference: &str,
    at: &FlmSpan,
) -> ConstructParserResult<Flm, Attached> {
    let including = at.source().origin();
    if including.is_definition() {
        cx.recover(InputInDefinition, at.clone())?;
        return Ok(Attached::Nothing);
    }
    let driver = cx.driver;
    let Some(resolver) = driver.input_resolver() else {
        cx.recover(InputUnavailable { reference: reference.to_string() }, at.clone())?;
        return Ok(Attached::Nothing);
    };
    let in_force = Arc::clone(&cx.state.ext().parsing);
    let file = match resolver.resolve(reference, including.canonical_path(), &in_force) {
        Ok(file) => file,
        Err(error) => {
            let condition =
                InputUnreadable { reference: reference.to_string(), detail: error.message };
            cx.recover(condition, at.clone())?;
            return Ok(Attached::Nothing);
        }
    };
    // See decision rationale at [§flm-dd-dr:lang-input-chain-check].
    // The chain: the including source, the source that included it, and so
    // on up to the primary file, which is at depth zero.
    let mut depth = 0;
    for source in at.source().including_sources() {
        if source.origin().canonical_path() == Some(file.name.as_str()) {
            cx.recover(InputIncludesItself { file: file.name.clone() }, at.clone())?;
            return Ok(Attached::Nothing);
        }
        depth += 1;
    }
    if depth > DEPTH_LIMIT {
        cx.recover(InputTooDeep { limit: DEPTH_LIMIT, file: file.name.clone() }, at.clone())?;
        return Ok(Attached::Nothing);
    }
    for problem in &file.problems {
        let place = FlmSource::resolved(String::new(), reference, at.clone())
            .with_origin(FlmOrigin::new(file.name.clone()))
            .with_line_column_number_offsets(problem.line, problem.column);
        let condition = IncludedFrontMatterProblem { message: problem.message.clone() };
        cx.recover(condition, SourceSpan::new(&Arc::new(place), 0..0))?;
    }
    let source = FlmSource::resolved(file.text, reference, at.clone())
        .with_origin(FlmOrigin::new(file.name.clone()))
        .with_line_column_number_offsets(file.first_line.max(1), 1);
    let name = file.name;
    // The file's settings are recorded on the state it is read under, so that
    // a command inside it hands them on in turn. See decision rationale at
    // [§flm-dd-dr:lang-input-parsing-over].
    let state: Arc<ParsingState<Flm>> = match file.parsing {
        Some(parsing) => {
            let ext = cx.state.ext().clone().with_parsing(parsing.clone());
            cx.derive_state(&parsing.state_delta().ext(ext))?
        }
        None => Arc::clone(&cx.state),
    };
    let source = Arc::new(source);
    if let Some(mut heading) = file.heading {
        let Some(placements) = driver.placements() else {
            cx.recover(InputHeadingOutsideDocument { file: name }, at.clone())?;
            return Ok(Attached::Nothing);
        };
        // The heading is composed as text from the command name and the label;
        // each must keep that text one command with its arguments.
        if !is_command_name(&cx.state, &heading.command) {
            cx.recover(InputHeadingCommandInvalid { file: name, command: heading.command }, at.clone())?;
            return Ok(Attached::Nothing);
        }
        if let Some(label) = heading.label.take_if(|label| label.contains(['{', '}'])) {
            cx.recover(InputHeadingLabelInvalid { file: name.clone(), label }, at.clone())?;
        }
        let placed = PlacedInput { site: at.clone(), name: name.clone(), source, heading, state };
        if placements.record(placed) {
            return Ok(Attached::Placed);
        }
        cx.recover(InputPlacedTwice { file: name }, at.clone())?;
        return Ok(Attached::Nothing);
    }
    let mut parser = driver
        .make_nodes_parser(StopSpec::none(), ChildStateSpec::inherit())
        .map_err(|error| cx.attach_hook_frames(error))?;
    let outcome = cx.parse_attached_source(source, state, &mut *parser)?;
    Ok(Attached::Content(outcome.nodes))
}
