//! This module provides file inclusion with the `\input{file}` command: the
//! [`InputResolver`] trait, which a program that embeds FLM implements to read
//! the included files, and the specification of inclusion macros
//! ([`InputSpec`]), along with the error conditions that are reported for an
//! inclusion.
//!
//! # Reading the included files
//!
//! FLM performs no input or output, so the library cannot read a file itself. A
//! program that embeds FLM provides an *input resolver*, an object that
//! implements [`InputResolver`]. For each `\input` command, the library calls
//! the input resolver with the reference written in the document, for instance
//! `parts/intro.flm`. The input resolver finds and reads the file, reads the
//! front matter of the file, and returns the content and the information from
//! the front matter as an [`IncludedFile`].
//!
//! You set the input resolver in the inputs of a registry, with
//! [`RegistryInputs::input_resolver`](crate::core::registry::RegistryInputs::input_resolver),
//! or for a single parse, with
//! [`ParseOptions::input_resolver`](crate::lang::ParseOptions::input_resolver).
//! The `flm` program provides an input resolver that reads files relative to
//! the folder of the including file.
//!
//! The library keeps track of which file includes which. It reports a file
//! that includes itself and a chain of inclusions that is too deep, so the
//! input resolver does not need to remember earlier calls.
//!
//! # Content inputs and heading inputs
//!
//! An included file is either a content input or a heading input. The input
//! resolver selects the kind with the field [`IncludedFile::heading`], usually
//! according to the front matter of the file.
//!
//! - A *content input* continues the text around the `\input` command. Its
//!   content is parsed at the place of the command, as if it were written
//!   there. For instance, a paragraph can begin in the including file and end
//!   in the included file.
//! - A *heading input* is a part of the document with a heading of its own,
//!   such as a chapter kept in a file. The input resolver provides the heading
//!   as an [`InputHeading`]. The library parses the heading input as a separate
//!   tree that begins with the heading, and places this tree in the document at
//!   the `\input` command. Only the assembly of a whole document
//!   ([`crate::model::assemble`]) places such trees. A heading input can
//!   therefore be included only in a whole document, and only once.
//!
//! # The content of a content input in the tree
//!
//! The content of a content input is parsed under the parsing state in effect
//! at the `\input` command, with the parsing settings of the file
//! ([`IncludedFile::parsing`]) applied. A change that the included content
//! makes to the parsing state does not apply after the command.
//!
//! The parsed content is stored in the node slot [`ATTACHED_SLOT`] of the node
//! of the `\input` command. A *node slot* (techy's `ParsedSlot`) is a part of
//! the content of a node that is not an argument, such as the body of an
//! environment. The nodes of the included content have spans in the included
//! file, and the node of the command has its span in the including file.
//! [`attached_content`] returns the included content of such a node.
//!
//! The splicing pass ([`crate::lang::rewrites::input`]) runs before every
//! other rewrite pass, and replaces the node of each `\input` command with the
//! included content. The later passes and stages therefore process the
//! included content as if it were written in the including file.
//!
//! For a heading input, the node of the `\input` command has the empty node
//! slot [`PLACED_SLOT`] instead, and [`is_placing`] returns `true` for this
//! node.
//!
//! # Errors
//!
//! When a file cannot be included, the parser of the `\input` command reports
//! an error at the command, and the command includes nothing. With the standard
//! [`input` feature](crate::stdlib::features::input), the command then expands
//! to a dropped mark (`flm.pres.dropped`). The following errors prevent the
//! inclusion:
//!
//! - [`InputReferenceNotPlain`]: the reference is not plain text.
//! - [`InputUnavailable`]: no input resolver is set.
//! - [`InputUnreadable`]: the input resolver failed to provide the file.
//! - [`InputIncludesItself`]: the file includes itself.
//! - [`InputTooDeep`]: the chain of inclusions is too deep.
//! - [`InputInDefinition`]: the command is written in a user definition.
//! - [`InputHeadingOutsideDocument`], [`InputHeadingCommandInvalid`] and
//!   [`InputPlacedTwice`]: a heading input cannot be included.
//!
//! The file is still included after the errors [`IncludedFrontMatterProblem`]
//! and [`InputHeadingLabelInvalid`].

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

/// The name of the argument of an inclusion macro that contains the reference
/// to the included file (`reference`).
///
/// An argument with this name must be declared for every inclusion macro
/// ([`InputSpec::new`]).
pub const REFERENCE_ARGUMENT: &str = "reference";

/// The name of the node slot that contains the included content of a content
/// input (`attached`).
///
/// The node of an `\input` command has this slot when the command included a
/// content input. A node slot is a part of the content of a node that is not
/// an argument; see [the module
/// documentation](self#the-content-of-a-content-input-in-the-tree).
pub const ATTACHED_SLOT: &str = "attached";

/// The name of the empty node slot that marks the node of an `\input` command
/// that included a heading input (`placed`).
///
/// The tree of the heading input is placed in the document at this node
/// ([`is_placing`]).
pub const PLACED_SLOT: &str = "placed";

/// The maximum depth of an included file (32).
///
/// The primary file has depth zero, a file that the primary file includes has
/// depth one, and so on. A file at a greater depth is not included
/// ([`InputTooDeep`]).
pub const DEPTH_LIMIT: usize = 32;

// --- what the library asks of a host ----------------------------------------------------

/// An `IncludedFile` contains the content of an included file and the
/// information that the input resolver read from the front matter of the file.
///
/// An [`InputResolver`] returns an `IncludedFile` for each `\input` command.
/// The fields have defaults, so a resolver that reads no front matter only
/// sets [`name`](Self::name), [`text`](Self::text) and
/// [`first_line`](Self::first_line).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IncludedFile {
    /// The name that identifies the file, for instance its canonical path.
    ///
    /// Diagnostics refer to the file by this name. The library also compares
    /// names to detect a file that includes itself ([`InputIncludesItself`])
    /// and a heading input that is included twice ([`InputPlacedTwice`]).
    /// Therefore, two references to the same file must give the same name, and
    /// two different files must have different names. The name of the primary
    /// file is the canonical path of its [origin](crate::lang::FlmOrigin).
    pub name: String,
    /// The content of the file, without its front matter.
    pub text: String,
    /// The line of the file on which [`text`](Self::text) begins, counted from
    /// one.
    ///
    /// The library uses this number to report a problem in the content at its
    /// line in the file. For a file without front matter, the value is one.
    /// The value zero counts as one.
    pub first_line: usize,
    /// The parsing settings of the content, if parsing settings are written in
    /// the file itself, for instance in its front matter.
    ///
    /// The value is complete: it contains the `parsing` settings that the
    /// library passes to [`InputResolver::resolve`], with the settings of the
    /// file applied over them. With `None`, the content is parsed under the
    /// parsing state in effect at the `\input` command.
    pub parsing: Option<FlmParsing>,
    /// The problems that the input resolver found in the front matter of the
    /// file.
    ///
    /// The library reports each problem as an error
    /// ([`IncludedFrontMatterProblem`]) and still includes the file.
    pub problems: Vec<IncludedFileProblem>,
    /// The heading of a heading input, or `None` for a content input.
    ///
    /// See [content inputs and heading
    /// inputs](self#content-inputs-and-heading-inputs) for the two kinds of
    /// included files.
    pub heading: Option<InputHeading>,
}

/// An `InputHeading` describes the heading of a heading input
/// ([`IncludedFile::heading`]).
///
/// The library writes the heading as FLM source before the content of the
/// file, and parses it with the content. For instance, the heading command
/// `section`, the title `Methods` and the label `sec:methods` give
/// `\section{Methods}\label{sec:methods}`. An unnumbered heading uses the
/// starred form of the command, as in `\section*{Methods}`.
///
/// The command is parsed like any command written in the document. The library
/// does not check that the command is a heading command. It only checks that
/// [`command`](Self::command) is a valid command name
/// ([`InputHeadingCommandInvalid`]) and that [`label`](Self::label) contains
/// no brace ([`InputHeadingLabelInvalid`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputHeading {
    /// The name of the heading command, without the backslash, for instance
    /// `section`.
    pub command: String,
    /// The title, as FLM source, for instance `The \emph{Art}`.
    pub title: String,
    /// The line and the column of the title in the file, both counted from
    /// one, or `None` if the title is not written in the file.
    ///
    /// The library uses this position to report a problem in the title at its
    /// place in the file.
    pub title_at: Option<(usize, usize)>,
    /// The label of the heading, if the heading has one.
    pub label: Option<String>,
    /// Whether the heading is numbered. If `false`, the library writes the
    /// starred form of the heading command. The default is `false`.
    pub numbered: bool,
}

/// An `IncludedFileProblem` describes a problem that an input resolver found
/// in the front matter of an included file ([`IncludedFile::problems`]).
///
/// The library reports the problem as the error
/// [`IncludedFrontMatterProblem`], at the given line and column of the
/// included file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncludedFileProblem {
    /// The line of the problem, counted from one.
    pub line: usize,
    /// The column of the problem, counted from one.
    pub column: usize,
    /// The message of the diagnostic. Write it like the messages of the
    /// library: state directly what is wrong, as in `Unknown key ‘title2’`.
    pub message: String,
}

/// The error that an input resolver returns when it fails to provide the file
/// of an `\input` command, for instance because the file does not exist.
///
/// The library reports the failure as the error [`InputUnreadable`], whose
/// message contains [`message`](Self::message).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputError {
    /// The description of the failure. It should give the file that the input
    /// resolver tried to read.
    pub message: String,
}

impl InputError {
    /// Returns an error with the description `message`.
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

/// An input resolver finds and reads the files that `\input` commands refer
/// to. A program that embeds FLM implements this trait, because the library
/// performs no input or output itself.
///
/// You set the input resolver in the inputs of a registry, with
/// [`RegistryInputs::input_resolver`](crate::core::registry::RegistryInputs::input_resolver),
/// or for a single parse, with
/// [`ParseOptions::input_resolver`](crate::lang::ParseOptions::input_resolver).
/// If no input resolver is set, the library reports each `\input` command as
/// an error ([`InputUnavailable`]) and includes nothing. See the [module
/// documentation](self) for how included files are parsed.
///
/// The library calls [`resolve`](Self::resolve) once for each `\input` command
/// that it parses. The method receives three arguments:
///
/// - `reference`: the reference to the file, as written in the document. The
///   input resolver decides how to find the file from the reference, usually
///   relative to the including file.
/// - `including`: the [name](IncludedFile::name) of the file that contains
///   the `\input` command, or `None` if the content does not come from a file.
///   For the primary file, the name is the canonical path of its
///   [origin](crate::lang::FlmOrigin).
/// - `parsing`: the parsing settings in effect at the `\input` command. In an
///   included file, these are the settings under which the library parses the
///   file.
///
/// If parsing settings are written in the file itself, the input resolver
/// applies them over `parsing` and returns the result in
/// [`IncludedFile::parsing`].
///
/// The library keeps track of which file includes which, and of the parsing
/// settings of each file. An input resolver therefore does not need to
/// remember earlier calls.
///
/// An input resolver must be `Send` and `Sync`, because the library can call
/// it from several threads. If your input resolver records the files that it
/// reads, store the record behind a lock, for instance a `Mutex`.
///
/// # Examples
///
/// The following input resolver provides files stored in memory. It sets the
/// name, the content and the first line of each file. The other fields of
/// [`IncludedFile`] keep their defaults, so each file is a content input
/// without parsing settings of its own.
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
    /// content and the information from its front matter. The [trait
    /// documentation](InputResolver) describes the arguments.
    ///
    /// # Errors
    ///
    /// Returns an [`InputError`] if the file cannot be found or read. The
    /// library reports the error at the `\input` command ([`InputUnreadable`])
    /// and includes nothing.
    fn resolve(
        &self,
        reference: &str,
        including: Option<&str>,
        parsing: &FlmParsing,
    ) -> Result<IncludedFile, InputError>;
}

// --- the conditions -------------------------------------------------------------------

/// The error that is reported when the argument of `\input` is not plain text
/// (`flm.input.reference-not-plain`).
///
/// The argument must consist of characters only, as in
/// `\input{parts/intro.flm}`.
///
/// Reported by the parser of `\input` when it parses the command. The command
/// includes no file. With the standard [`input`
/// feature](crate::stdlib::features::input), the command expands to a dropped
/// mark (`flm.pres.dropped`).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.reference-not-plain",
    message = "Invalid argument to ‘\\input’, expected plain text only"
)]
pub struct InputReferenceNotPlain;

/// The error that is reported when a document uses `\input` and no input
/// resolver is set (`flm.input.unavailable`).
///
/// The library reads included files through an [`InputResolver`], which the
/// program that processes the document must provide. The [`InputResolver`]
/// documentation describes how to set one.
///
/// Reported by the parser of `\input` when it parses the command. The command
/// includes no file. With the standard [`input`
/// feature](crate::stdlib::features::input), the command expands to a dropped
/// mark (`flm.pres.dropped`).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.unavailable",
    message = "Cannot process ‘\\input{{{reference}}}’: No input source resolver configured for this document"
)]
pub struct InputUnavailable {
    /// The reference, as written.
    pub reference: String,
}

/// The error that is reported when the input resolver fails to provide the
/// file of an `\input` command (`flm.input.unreadable`).
///
/// The input resolver returns an [`InputError`], for instance because the file
/// does not exist or cannot be read. The message of the diagnostic contains
/// the description of the failure from the [`InputError`].
///
/// Reported by the parser of `\input` when it parses the command. The command
/// includes no file. With the standard [`input`
/// feature](crate::stdlib::features::input), the command expands to a dropped
/// mark (`flm.pres.dropped`).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.unreadable",
    message = "Source input ‘{reference}’ nonexistent or unreadable: {detail}"
)]
pub struct InputUnreadable {
    /// The reference, as written.
    pub reference: String,
    /// The description of the failure ([`InputError::message`]).
    pub detail: String,
}

/// The error that is reported when a file includes itself, directly or
/// through other files (`flm.input.includes-itself`).
///
/// The library compares the [name](IncludedFile::name) of the file that an
/// `\input` command refers to with the names of the including file, the file
/// that includes it, and so on up to the primary file.
///
/// Reported by the parser of `\input` at the command that would include the
/// file again. The command includes no file. With the standard [`input`
/// feature](crate::stdlib::features::input), the command expands to a dropped
/// mark (`flm.pres.dropped`).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.includes-itself",
    message = "Refusing recursive inclusion of ‘{file}’"
)]
pub struct InputIncludesItself {
    /// The [name](IncludedFile::name) of the file.
    pub file: String,
}

/// The error that is reported when an included file would have a depth
/// greater than [`DEPTH_LIMIT`] (`flm.input.too-deep`).
///
/// The primary file has depth zero, a file that the primary file includes has
/// depth one, and so on.
///
/// Reported by the parser of `\input` when it parses the command. The command
/// includes no file. With the standard [`input`
/// feature](crate::stdlib::features::input), the command expands to a dropped
/// mark (`flm.pres.dropped`).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.too-deep",
    message = "Input depth limit ({limit} levels) reached while processing inclusion of ‘{file}’"
)]
pub struct InputTooDeep {
    /// The maximum depth, [`DEPTH_LIMIT`].
    pub limit: usize,
    /// The [name](IncludedFile::name) of the file that the `\input` command
    /// refers to.
    pub file: String,
}

/// The error that is reported when `\input` is written in the definition of a
/// user-defined command or environment (`flm.input.in-definition`).
///
/// This is the case when `\input` is written in the template of a [user
/// definition](crate::lang::rewrites::userdefs::Definition), or in the default
/// value of one of its arguments.
///
/// Reported by the parser of `\input` when it parses the command. The command
/// includes no file. With the standard [`input`
/// feature](crate::stdlib::features::input), the command expands to a dropped
/// mark (`flm.pres.dropped`).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.in-definition",
    message = "‘\\input’ must not appear in the template or default argument values of a \
               user-defined command or environment"
)]
pub struct InputInDefinition;

/// The error that is reported for a problem that the input resolver found in
/// the front matter of an included file (`flm.input.front-matter`).
///
/// The input resolver returns the problem as an [`IncludedFileProblem`]. The
/// message of the diagnostic is the message of the [`IncludedFileProblem`].
///
/// Reported by the parser of `\input`, at the line and column of the problem
/// in the included file. The file is still included.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.input.front-matter", message = "{message}")]
pub struct IncludedFrontMatterProblem {
    /// The message from the input resolver ([`IncludedFileProblem::message`]).
    pub message: String,
}

/// The error that is reported when a heading input is included a second time
/// in a document (`flm.input.placed-twice`).
///
/// The tree of a [heading input](self#content-inputs-and-heading-inputs) is
/// placed in the document at the `\input` command, so a heading input can be
/// included only once. Two `\input` commands include the same file when the
/// input resolver returns the same [name](IncludedFile::name) for both.
///
/// Reported by the parser of `\input` at the second command. The second
/// command includes no file. With the standard [`input`
/// feature](crate::stdlib::features::input), it expands to a dropped mark
/// (`flm.pres.dropped`).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.placed-twice",
    message = "Duplicate heading-mode inclusion of ‘{file}’"
)]
pub struct InputPlacedTwice {
    /// The [name](IncludedFile::name) of the file.
    pub file: String,
}

/// The error that is reported when a heading input is included outside the
/// assembly of a whole document (`flm.input.heading-outside-document`).
///
/// The tree of a [heading input](self#content-inputs-and-heading-inputs) is
/// placed in the document at the `\input` command. Only the assembly of a
/// whole document ([`crate::model::assemble`]) places such trees. Content that
/// is parsed on its own, for instance with
/// [`parse_fragment`](crate::lang::parse_fragment), is not part of a document.
/// A site generator, for instance, parses each field of its data in this way.
///
/// Reported by the parser of `\input` when it parses the command. The command
/// includes no file. With the standard [`input`
/// feature](crate::stdlib::features::input), the command expands to a dropped
/// mark (`flm.pres.dropped`).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.heading-outside-document",
    message = "Heading-mode inclusion of ‘{file}’ is only possible in a whole document"
)]
pub struct InputHeadingOutsideDocument {
    /// The [name](IncludedFile::name) of the file.
    pub file: String,
}

/// The error that is reported when the assembly of a document does not find
/// the node of the `\input` command of a heading input
/// (`flm.input.placement-lost`).
///
/// When the parser of `\input` includes a heading input, it records the file
/// for the assembly ([`crate::model::assemble`]) and adds the node of the
/// command to the tree. The assembly then places the tree of the heading input
/// at this node. A document cannot cause this error: a missing node indicates
/// a fault in the library or in an extension.
///
/// Reported by the assembly, at the `\input` command. The tree of the heading
/// input is not placed in the document.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.placement-lost",
    message = "Misconfigured input processing: The node of a heading-mode ‘\\input’ for ‘{file}’ was not found in the final document tree"
)]
pub struct InputPlacementLost {
    /// The [name](IncludedFile::name) of the file.
    pub file: String,
}

/// The error that is reported when the heading command of a heading input is
/// not a valid command name (`flm.input.heading-command`).
///
/// The input resolver provides the heading command in
/// [`InputHeading::command`], usually from the front matter of the included
/// file. The heading command must be the name of a command without the
/// backslash, as in `section`. It may contain only the characters that the
/// parsing settings in effect at the `\input` command allow in a command name.
///
/// Reported by the parser of `\input` when it parses the command. The command
/// includes no file. With the standard [`input`
/// feature](crate::stdlib::features::input), the command expands to a dropped
/// mark (`flm.pres.dropped`).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.heading-command",
    message = "Invalid heading command ‘\\{command}’ configured for heading-mode input of ‘{file}’"
)]
pub struct InputHeadingCommandInvalid {
    /// The [name](IncludedFile::name) of the file.
    pub file: String,
    /// The heading command, as provided by the input resolver.
    pub command: String,
}


// ### Why do we have to check that the label doesn't hold a brace??? How is this even a condition??? What's the deal here????

/// The error that is reported when the label of the heading of a heading input
/// contains a brace (`flm.input.heading-label`).
///
/// The input resolver provides the label in [`InputHeading::label`], usually
/// from the front matter of the included file. The library writes the heading
/// as FLM source, as in `\section{Methods}\label{sec:methods}`, and parses it. A
/// brace in the label would change where the argument of `\label` ends.
/// Therefore, the label must not contain `{` or `}`.
///
/// Reported by the parser of `\input` when it parses the command. The file is
/// still included, and its heading has no label.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.input.heading-label",
    message = "Invalid label ‘{label}’ specified in heading-mode inclusion of ‘{file}’"
)]
pub struct InputHeadingLabelInvalid {
    /// The [name](IncludedFile::name) of the file.
    pub file: String,
    /// The label, as provided by the input resolver.
    pub label: String,
}

// --- the record of heading inputs -------------------------------------------------------

/// A `PlacedInput` records a heading input that the parser of `\input`
/// included. It contains what the assembly of the document needs to parse the
/// file as a separate tree and to place this tree at the `\input` command.
#[derive(Debug, Clone)]
pub(crate) struct PlacedInput {
    /// The span of the `\input` command in the including file. The assembly
    /// finds the node of the command by this span.
    pub(crate) site: FlmSpan,
    /// The [name](IncludedFile::name) of the file.
    pub(crate) name: String,
    /// The content of the file, as a source included from the `\input`
    /// command.
    pub(crate) source: Arc<FlmSource>,
    /// The heading of the heading input.
    pub(crate) heading: InputHeading,
    /// The parsing state under which the file is parsed: the state at the
    /// `\input` command, with the parsing settings of the file applied.
    pub(crate) state: Arc<ParsingState<Flm>>,
}

/// `Placements` collects the heading inputs that are included while one
/// document is assembled, in the order in which they are parsed.
///
/// The assembly ([`crate::model::assemble`]) sets a `Placements` on the parse
/// driver (`FlmDriver::with_placements`), and reads the new records after each
/// parse. The parser of `\input` reports [`InputHeadingOutsideDocument`] when
/// the parse driver has no `Placements`.
#[derive(Debug, Default)]
pub(crate) struct Placements {
    records: std::sync::Mutex<Vec<PlacedInput>>,
}

impl Placements {
    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<PlacedInput>> {
        self.records.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Returns the number of heading inputs recorded so far.
    pub(crate) fn len(&self) -> usize {
        self.lock().len()
    }

    /// Returns the records with the indices `from..to`, or an empty vector if
    /// this range is out of bounds.
    pub(crate) fn range(&self, from: usize, to: usize) -> Vec<PlacedInput> {
        self.lock().get(from..to).map(<[PlacedInput]>::to_vec).unwrap_or_default()
    }

    /// Records `placed` and returns `true`, or returns `false` without
    /// recording if a heading input with the same name was recorded before.
    fn record(&self, placed: PlacedInput) -> bool {
        let mut records = self.lock();
        if records.iter().any(|known| known.name == placed.name) {
            return false;
        }
        records.push(placed);
        true
    }
}

/// Returns the FLM source of the heading of a heading input.
///
/// The source has two lines: the heading command on the first line, as in
/// `\section` or `\section*`, and the title in braces followed by
/// `\label{label}` on the second line. The origin of the source is the name of
/// the file, and the source is included from the `\input` command. The line
/// and column offsets of the source place the title at its position in the
/// file ([`InputHeading::title_at`]), so that a problem in the title is
/// reported there. These offsets apply to every line of the source, which is
/// why the title is on a line of its own.
///
/// [`attach`] checks the command name and the label before it records the
/// heading input, so the source contains exactly one command and its argument.
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

/// Returns whether `name` is a valid command name under `state`: `name` is not
/// empty, and a command rule of `state` with the escape character `\` accepts
/// every character of `name` in a command name.
fn is_command_name(state: &ParsingState<Flm>, name: &str) -> bool {
    !name.is_empty()
        && state
            .rules()
            .command_rules()
            .iter()
            .any(|rule| rule.escape_char == '\\' && name.chars().all(|c| rule.name_chars.contains(c)))
}

/// The root parser of the tree of a heading input.
///
/// The parser parses the heading ([`heading_source`]) and then the content of
/// the file, both as included sources, so that a problem in either is reported
/// with the chain of inclusions. The root node contains the nodes of both and
/// spans the content of the file.
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

/// An `InputSpec` is the specification of an inclusion macro, such as
/// `\input`. It contains the arguments of the macro and provides the parser
/// that includes the file (see the [module documentation](self)).
///
/// To define an inclusion macro in a feature, declare an element with the
/// syntax [`SyntaxDef::Inclusion`](crate::lang::mint::SyntaxDef::Inclusion).
/// [Minting](crate::lang::mint) then creates the `InputSpec`, and wraps it in
/// an [`FlmCallable`] like the specification of every macro.
#[derive(Debug)]
pub struct InputSpec {
    arguments: Vec<Arc<ArgumentSpec<Flm>>>,
}

impl InputSpec {
    /// Returns the specification of an inclusion macro with the arguments
    /// `arguments`, in the order in which they are written.
    ///
    /// # Errors
    ///
    /// Returns an error if no argument is named [`REFERENCE_ARGUMENT`]. The
    /// error is a description of the declared arguments, which minting embeds
    /// in the message of its own error.
    pub fn new(arguments: Vec<Arc<ArgumentSpec<Flm>>>) -> Result<InputSpec, String> {
        if arguments.iter().any(|argument| argument.name.as_deref() == Some(REFERENCE_ARGUMENT)) {
            return Ok(InputSpec { arguments });
        }
        let names: Vec<String> = arguments
            .iter()
            .map(|argument| argument.name.as_deref().map_or_else(|| String::from("<unnamed>"), |n| format!("‘{n}’")))
            .collect();
        Err(if names.is_empty() {
            String::from("no argument declared")
        } else {
            format!("declared arguments: {}", names.join(", "))
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

/// Returns whether `node` is the node of an inclusion macro, such as `\input`.
///
/// The result does not depend on whether the file was included.
pub fn is_input(node: &NodeRef<'_, Flm>) -> bool {
    let Some(spec) = node.spec() else { return false };
    (&**spec as &dyn Any)
        .downcast_ref::<FlmCallable>()
        .is_some_and(|callable| (&**callable.syntax() as &dyn Any).is::<InputSpec>())
}

/// Returns whether `node` is the node of an inclusion macro that included a
/// heading input.
///
/// The tree of the heading input is placed in the document at this node
/// ([`DocContent::resolve`](crate::model::doc::DocContent::resolve)).
pub fn is_placing(node: &NodeRef<'_, Flm>) -> bool {
    is_input(node) && node.slot_content_nodes_named(PLACED_SLOT).is_ok()
}

/// Returns the included content of a content input: the nodes in the node
/// slot [`ATTACHED_SLOT`] of `node`.
///
/// Returns `None` if `node` is not the node of an inclusion macro, or if the
/// macro did not include a content input. The splicing pass
/// ([`crate::lang::rewrites::input`]) replaces the node of each inclusion macro
/// with its included content, so this function is useful only on a tree
/// before this pass.
pub fn attached_content<'t>(node: &NodeRef<'t, Flm>) -> Option<Vec<NodeRef<'t, Flm>>> {
    if !is_input(node) {
        return None;
    }
    node.slot_content_nodes_named(ATTACHED_SLOT).ok().map(|nodes| nodes.iter().collect())
}

// --- the parser -----------------------------------------------------------------------

/// The parser of one invocation of an inclusion macro.
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

/// The result of [`attach`] for one `\input` command.
enum Attached {
    /// A content input was included. The vector contains the parsed content.
    Content(Vec<BuildId>),
    /// A heading input was recorded for the assembly of the document.
    Placed,
    /// No file was included, and an error was reported.
    Nothing,
}

/// Includes the file that `reference` refers to, for the `\input` command at
/// the span `at`.
///
/// Calls the input resolver and checks the chain of inclusions. Parses the
/// content of a content input under the parsing state at the command, with the
/// parsing settings of the file applied. Records a heading input in the
/// [`Placements`] of the parse driver. Reports each problem with
/// [`ParseContext::recover`] and returns [`Attached::Nothing`] when the file
/// cannot be included.
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
    // The library checks the chain of inclusions, so that no host has to: which
    // file includes which is a fact of the document, and the host states only
    // each file's identity, as its name.
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
    // a command inside it hands them on in turn, and no host has to remember
    // them.
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
