//! Diagnostics laid out as text for a person, or as data for a program that shows them
//! another way.
//!
//! A diagnostic reports a problem with a document: a severity (Error, Warning or Note), a
//! position, and a typed condition with its data; its message is written for a person and
//! is not a stable interface ([`FlmDiagnostic`]). [`diagnostic`] lays out one diagnostic as
//! lines of text:
//!
//! ```text
//! error: <the message>
//!   at paper.flm:12:5
//!   inside paper.flm:3:1: <what the parser had entered there>
//!   in content made from it: <the label of that content>:1:3
//! ```
//!
//! - The `at` line gives the *leading position*: a place in a file the author wrote, which
//!   the author can act on. It is the diagnostic's own place when the problem is in the
//!   author's text. For a problem in content the library made (the expansion of a macro
//!   the author defined, a text given in the settings), it is the place in the author's file
//!   from which that content was made. [`lead_position`] decides which position leads.
//! - Each `inside` line is one frame of the diagnostic's traceback, with its place and its
//!   title. A frame is either what the parser had entered when it found the problem (a
//!   group, an argument; the first frame is often at the diagnostic's own place and repeats
//!   the `at` line), or a place where the content that holds the problem is shown again as
//!   a copy (for example the content that a `\ref` displays; see [`flm_core::lang::copy`]),
//!   with the title `a copy of this content, shown here`.
//! - The last line, `in content made from it`, gives the position in the content the library
//!   made, when the problem is there.
//!
//! [`report`] lays out every diagnostic of a build, in the order the build reported them,
//! duplicates included, and ends with the number of errors and of warnings ([`counts`]).
//! [`problem`] lays out a [`Problem`], a problem that stopped
//! [`prepare`](crate::session::prepare) or [`build`](crate::session::build) before the
//! document was processed. This module returns text and prints nothing; the `flm` program
//! prints the text on standard error.
//!
//! [`Reported`] holds a diagnostic or a [`Problem`] as data, for a program that shows it
//! another way: the live preview's page lists each with its severity, its text and its
//! place. It holds the same text as [`diagnostic`] and [`problem`] give, and the leading
//! position as a file, a line, a column and a range of bytes ([`Position`]).
//!
//! # Records
//!
//! A [`Record`] holds one thing a program reports, as data that serializes to JSON, for a
//! program that reads the reports of another (an editor, a script): a diagnostic, a
//! problem outside a document's diagnostics (one that stopped the work, or an error or a
//! warning of the program itself), the counts that end a report, the start of a build of
//! the live preview, or a line that tells what the program does. A program writes the
//! records as a stream, one JSON value per line, and writes first the header of that
//! stream, [`Record::meta`], which gives the version of FLM and of the records' format.
//!
//! A diagnostic's record holds the condition's identifier and data, as
//! [`flm_core::dump::project`] shows them, and the message; its places are those that the
//! lines of [`diagnostic`] give: the leading position, each frame's, and the position in
//! the content the library made. A place in a file the author wrote ([`Place`]) gives the
//! file, the line counted from 1 and the column counted from 1 in bytes.
//!
//! - A place in the file's content, the text below its front matter, also gives the byte
//!   offset counted from the top of the file, and the same three numbers for the end of the
//!   range it covers. The library parses the content alone and counts byte offsets from
//!   its start, so the record adds the number of bytes before the content
//!   ([`ContentOffsets`]); [`session::build`](crate::session::build) gives these for the
//!   input and for each included file ([`Built::content_offsets`](crate::session::Built::content_offsets)).
//! - Only a place in a file's parsed content has a byte offset. A place in a file's front
//!   matter, or in an extension's manifest, gives its line and its column alone: a problem
//!   with a value of the front matter or of the manifest's `flm.use`, a warning about a
//!   setting (written in the front matter or in an extension's package), and a problem in a
//!   text that the library parses from the front matter (the document's title, author or
//!   date, or the title of a file included with `\input` that opens with a heading).
//! - A place in content the library made (the expansion of a macro the author defined, a
//!   text given in the settings) gives the label of that content and nothing else.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use flm_core::dump::{Meta, ProjectOptions, project_diagnostic};
use flm_core::lang::{FlmDiagnostic, FlmOrigin, FlmSource, FlmSpan};
use flm_core::techy::error::Severity;
use flm_core::techy::serialize::SerialValue;
use flm_core::techy::source::{LineIndexCache, SourceOrigin, SourceProvenance};
use schemars::JsonSchema;
use serde::Serialize;
use serde_json::Value;

use crate::seed::Split;
use crate::session::Problem;

/// A position in a file, as a person counts it: the file's name, and the line and column
/// counted from 1.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Position {
    /// The file's name, or the label of the content the library made; `None` when the
    /// source has no label.
    pub file: Option<String>,
    /// The line and the column, counted from 1, when known.
    pub line_column: Option<(usize, usize)>,
}

impl Position {
    fn of(span: &FlmSpan, cache: &mut LineIndexCache<FlmOrigin>) -> Position {
        let source = span.source();
        Position { file: source.origin().label().map(|label| label.into_owned()), line_column: cache.line_col(source, span.start()) }
    }

    /// The position of `place` in the file `file`.
    fn at(file: Option<String>, place: Option<crate::seed::Place>) -> Position {
        Position { file, line_column: place.map(|place| (place.line as usize, place.column as usize)) }
    }

    fn text(&self) -> String {
        match (&self.file, self.line_column) {
            (Some(file), Some((line, column))) => format!("{file}:{line}:{column}"),
            (Some(file), None) => file.clone(),
            (None, Some((line, column))) => format!("line {line}, column {column}"),
            (None, None) => String::from("an unknown place"),
        }
    }
}

/// Whether `span` points into a file the author wrote: its source has a file path.
fn in_authors_file(span: &FlmSpan) -> bool {
    span.source().origin().canonical_path().is_some()
}

/// Returns the leading position of a diagnostic whose span is `span`: `span` itself when it
/// points into a file the author wrote (its source has a path), else the first place in
/// such a file, found by following the source's provenance chain, from which the content
/// holding `span` was made; `None` when no such place is known.
pub fn lead_position(span: &FlmSpan) -> Option<FlmSpan> {
    if in_authors_file(span) {
        return Some(span.clone());
    }
    span.source().provenance_chain().find_map(|provenance| match provenance {
        SourceProvenance::Resolved { triggered_at, .. } | SourceProvenance::Synthesized { triggered_at, .. }
            if in_authors_file(triggered_at) =>
        {
            Some(triggered_at.clone())
        }
        _ => None,
    })
}

/// The word for a severity.
fn severity_word(severity: Severity) -> &'static str {
    match severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
        Severity::Note => "note",
    }
}

/// Whether a span points nowhere: an empty source without a file path, the span of a
/// problem with no place in any document.
fn is_nowhere(span: &FlmSpan) -> bool {
    span.source().content().is_empty() && span.source().origin().canonical_path().is_none()
}

/// Returns `diagnostic` laid out as text, without a final line break: the severity and the
/// message on the first line, then the `at`, `inside` and `in content made from it` lines
/// described in the [module documentation](self).
pub fn diagnostic(diagnostic: &FlmDiagnostic) -> String {
    let mut cache = LineIndexCache::<FlmOrigin>::new();
    let mut text = format!("{}: {}", severity_word(diagnostic.severity()), diagnostic.message());
    let span = diagnostic.span();
    let lead = lead_position(span);
    if let Some(lead) = &lead {
        text.push_str(&format!("\n  at {}", Position::of(lead, &mut cache).text()));
    }
    for frame in diagnostic.frames() {
        text.push_str(&format!("\n  inside {}: {}", Position::of(frame.span(), &mut cache).text(), frame.title()));
    }
    if shows_made_from(span, lead.as_ref()) {
        text.push_str(&format!("\n  in content made from it: {}", Position::of(span, &mut cache).text()));
    }
    text
}

/// Whether a diagnostic whose span is `span` and whose leading position is `lead` shows the
/// position in the content the library made: when `span` is not the leading position
/// itself and points somewhere.
fn shows_made_from(span: &FlmSpan, lead: Option<&FlmSpan>) -> bool {
    let own = lead.is_some_and(|lead| lead.same_source(span) && lead.start() == span.start());
    !own && !is_nowhere(span)
}

/// Returns `problem`, which stopped the work before the document was processed, laid out
/// as text: `error: ` and the problem's text on the first line, then an `at` line with the
/// file, the line and the column, when they are known.
pub fn problem(problem: &Problem) -> String {
    let mut text = format!("error: {}", problem.detail);
    let place = match (&problem.file, problem.place) {
        (Some(file), Some(place)) => Some(format!("{file}:{}:{}", place.line, place.column)),
        (Some(file), None) => Some(file.clone()),
        (None, Some(place)) => Some(format!("line {}, column {}", place.line, place.column)),
        (None, None) => None,
    };
    if let Some(place) = place {
        text.push_str(&format!("\n  at {place}"));
    }
    text
}

/// Returns the last line of a report, given the number of errors and of warnings:
/// `1 error, 0 warnings`.
pub fn counts(errors: usize, warnings: usize) -> String {
    let plural = |count: usize, word: &str| {
        if count == 1 { format!("1 {word}") } else { format!("{count} {word}s") }
    };
    format!("{}, {}", plural(errors, "error"), plural(warnings, "warning"))
}

/// Returns every diagnostic of `diagnostics` laid out as [`diagnostic`] does, in order,
/// each followed by a line break, then the line of [`counts`], which counts the Errors and
/// the Warnings (Notes are not counted).
pub fn report(diagnostics: &[FlmDiagnostic]) -> String {
    let mut text = String::new();
    for entry in diagnostics {
        text.push_str(&diagnostic(entry));
        text.push('\n');
    }
    let (errors, warnings) = tally(diagnostics);
    text.push_str(&counts(errors, warnings));
    text
}

/// The number of Errors and the number of Warnings among `diagnostics`.
fn tally(diagnostics: &[FlmDiagnostic]) -> (usize, usize) {
    let errors = diagnostics.iter().filter(|entry| entry.severity() == Severity::Error).count();
    let warnings = diagnostics.iter().filter(|entry| entry.severity() == Severity::Warning).count();
    (errors, warnings)
}

/// A diagnostic, or a [`Problem`] that stopped the work before the document was processed,
/// as data: see the [module documentation](self).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reported {
    /// The severity: error, warning or note. A problem that stopped the work is an error.
    pub severity: Severity,
    /// The text for a person, as [`diagnostic`] or [`problem`] lays it out: the severity
    /// and the message on the first line, the places on the lines after it.
    pub text: String,
    /// The leading position ([`lead_position`]), when one is known.
    pub position: Option<Position>,
    /// The range of bytes the leading position covers in its source, when known. It is
    /// counted in the text the library parsed, which, for a file with a front matter, is
    /// the content below the front matter. A [`Problem`] has none, nor has a diagnostic
    /// placed in a file by its line and column alone, with no text of the file (a warning
    /// about a setting, [`warning_diagnostic`](crate::settings::warning_diagnostic)).
    pub bytes: Option<Range<usize>>,
}

impl Reported {
    /// Returns `diagnostic` as data.
    pub fn of_diagnostic(diagnostic: &FlmDiagnostic) -> Reported {
        let lead = lead_position(diagnostic.span());
        let mut cache = LineIndexCache::<FlmOrigin>::new();
        Reported {
            severity: diagnostic.severity(),
            text: self::diagnostic(diagnostic),
            position: lead.as_ref().map(|lead| Position::of(lead, &mut cache)),
            bytes: lead.filter(|lead| !lead.source().content().is_empty()).map(|lead| lead.start()..lead.end()),
        }
    }

    /// Returns `problem` as data, with the severity Error and the position of its file and
    /// place, when either is known.
    pub fn of_problem(problem: &Problem) -> Reported {
        let position = (problem.file.is_some() || problem.place.is_some())
            .then(|| Position::at(problem.file.clone(), problem.place));
        Reported { severity: Severity::Error, text: self::problem(problem), position, bytes: None }
    }

    /// Returns the word for the severity, `error`, `warning` or `note`, as the text begins
    /// with it.
    pub fn severity_word(&self) -> &'static str {
        severity_word(self.severity)
    }
}

/// The content of each file the library parsed, by the file's name: where the content starts
/// in the file, and its text. [`Record::of_diagnostic`] reads it to count the byte offsets of
/// a diagnostic's places from the top of each file.
///
/// The library parses a file's content, the text below its front matter, into a source of
/// its own, and counts the byte offsets of a place in that source from the start of the
/// content. The content starts after the front matter's closing `---` line
/// ([`Split::content_offset`]). The library also parses, under the file's name, texts that
/// are not the file's content: the document's title, author or date, written in the front
/// matter, and the heading that a file included with `\input` opens with, made from the
/// file's title. A place in such a text gets no byte offset, since the library counts its
/// byte offsets from the start of that text ([`ContentOffsets::offset_of`]).
/// [`session::build`](crate::session::build) records the content of the input and of every
/// file it included ([`Built::content_offsets`](crate::session::Built::content_offsets)).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContentOffsets {
    files: BTreeMap<String, FileContent>,
}

/// The content of one file, as [`ContentOffsets`] records it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct FileContent {
    /// The byte offset in the file at which the content starts.
    offset: usize,
    /// The line of the file at which the content starts, counted from 1.
    line: usize,
    /// The content's text.
    text: Arc<str>,
}

impl ContentOffsets {
    /// Returns an empty list, with which no place gets a byte offset.
    pub fn new() -> ContentOffsets {
        ContentOffsets::default()
    }

    /// Records the content of the file `file` as `split`, the file's text split by
    /// [`seed::split`](crate::seed::split), gives it: the byte offset and the line at which
    /// the content starts, and its text. It replaces what was recorded for that file. `file`
    /// is the file's name as a diagnostic shows it: the canonical path of the source the
    /// library parses the content into.
    pub fn insert(&mut self, file: impl Into<String>, split: &Split<'_>) {
        let content =
            FileContent { offset: split.content_offset(), line: split.content_line as usize, text: Arc::from(split.content) };
        self.files.insert(file.into(), content);
    }

    /// Records every file of `other`, replacing what was recorded for the same files.
    pub fn extend(&mut self, other: ContentOffsets) {
        self.files.extend(other.files);
    }

    /// Returns the byte offset in its file at which the text of `source` starts, when
    /// `source` holds the content recorded for that file: its canonical path is the file's
    /// name, its text is the content, and its first character is at the line and the column
    /// where the content starts (the content's first line, column 1). The byte offset in the
    /// file of a place in `source` is then its byte offset in `source` plus the offset
    /// returned, and it agrees with the place's line and column. Returns `None` for any
    /// other source.
    pub fn offset_of(&self, source: &FlmSource) -> Option<usize> {
        let content = self.files.get(source.origin().canonical_path()?)?;
        let holds = source.line_number_offset() == content.line
            && source.column_number_offset() == 1
            && source.content() == &*content.text;
        holds.then_some(content.offset)
    }
}

/// The severity of a [`Record`]: `error`, `warning` or `note`, as the diagnostic's
/// severity is written in the [projection](flm_core::dump::project) and at the head of the
/// text of [`diagnostic`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RecordSeverity {
    /// An error: the document is wrong, or the work could not be done.
    Error,
    /// A warning: something suspicious that did not stop the work.
    Warning,
    /// A note: information.
    Note,
}

impl From<Severity> for RecordSeverity {
    fn from(severity: Severity) -> RecordSeverity {
        match severity {
            Severity::Error => RecordSeverity::Error,
            Severity::Warning => RecordSeverity::Warning,
            Severity::Note => RecordSeverity::Note,
        }
    }
}

/// A place that a [`Record`] reports: in a file the author wrote, its name, the line counted
/// from 1 and the column counted from 1 in bytes, and, in the file's content (the text
/// below its front matter), the byte offset counted from the top of the file and the end of
/// the range it covers; in content the library made, the label of that content alone. A key
/// with no value is left out of the JSON.
///
/// Only a place in a file's parsed content has a byte offset and an end. A place in a file's
/// front matter or in an extension's manifest has neither: a problem with a value of the
/// front matter or of the manifest's `flm.use`, a warning about a setting, a problem in the
/// document's title, author or date, or in the title of a file included with `\input` that
/// opens with a heading (see "Records" in the [module documentation](self)).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Place {
    /// The file's name, as the author wrote it or as the host resolved it (a file included
    /// with `\input`); absent for content the library made.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// The label of the content the library made, when the place is in such content.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The byte offset of the place, counted from 0 at the top of the file; given for a
    /// place in the file's content only.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offset: Option<usize>,
    /// The line, counted from 1 at the top of the file.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// The column, counted from 1 at the start of the line, in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
    /// The end of the range of bytes the place covers, exclusive; equal to the place itself
    /// for an empty range.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<PlaceEnd>,
}

/// The exclusive end of the range of bytes a [`Place`] covers, counted as the place is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct PlaceEnd {
    /// The byte offset, counted from 0 at the top of the file.
    pub offset: usize,
    /// The line, counted from 1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// The column, counted from 1, in bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<usize>,
}

/// What the places of one [`Record`] are worked out with: the files' contents
/// ([`ContentOffsets`]), the line index of each source met ([`LineIndexCache`]), and, for
/// each source met, the byte offset in its file at which its text starts, when it has one,
/// so that each source is indexed, and compared with its file's content, once.
struct Placer<'o> {
    offsets: &'o ContentOffsets,
    lines: LineIndexCache<FlmOrigin>,
    starts: Vec<(Arc<FlmSource>, Option<usize>)>,
}

impl<'o> Placer<'o> {
    fn new(offsets: &'o ContentOffsets) -> Placer<'o> {
        Placer { offsets, lines: LineIndexCache::new(), starts: Vec::new() }
    }

    /// The byte offset in its file at which the text of `source` starts, when `source`
    /// holds a file's content ([`ContentOffsets::offset_of`]).
    fn start(&mut self, source: &Arc<FlmSource>) -> Option<usize> {
        if let Some((_, start)) = self.starts.iter().find(|(known, _)| Arc::ptr_eq(known, source)) {
            return *start;
        }
        let start = self.offsets.offset_of(source);
        self.starts.push((Arc::clone(source), start));
        start
    }

    /// The place of `span`: in a file the author wrote (its source has a canonical path),
    /// the file, the line and the column, and, when the source holds the file's content,
    /// the byte offsets of the start and of the end counted from the top of the file; else
    /// the source's label alone.
    fn place(&mut self, span: &FlmSpan) -> Place {
        let source = span.source();
        let origin = source.origin();
        let Some(file) = origin.canonical_path() else {
            return Place { label: origin.label().map(|label| label.into_owned()), ..Place::default() };
        };
        let (line, column) = self.lines.line_col(source, span.start()).unzip();
        let mut place = Place { file: Some(file.to_string()), line, column, ..Place::default() };
        if let Some(start) = self.start(source) {
            let (line, column) = self.lines.line_col(source, span.end()).unzip();
            place.offset = Some(span.start() + start);
            place.end = Some(PlaceEnd { offset: span.end() + start, line, column });
        }
        place
    }
}

/// One frame of a diagnostic's traceback, in a [`Record`]: its place and its title, as an
/// `inside` line of [`diagnostic`] gives them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct Frame {
    /// Where the frame is.
    pub place: Place,
    /// What the parser had entered there, or `a copy of this content, shown here`.
    pub title: String,
}

/// One thing a program reports, as data: see "Records" in the [module
/// documentation](self). It serializes to a JSON map whose key `kind` gives the variant,
/// in lowercase (`diagnostic`, `problem`, `summary`, `build`, `log`); a key with no value
/// is left out.
// A record is made and written one at a time, never kept in numbers, so the size of its
// largest variant costs nothing; boxing it would only complicate its fields.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum Record {
    /// A diagnostic ([`Record::of_diagnostic`]).
    Diagnostic {
        /// The severity.
        severity: RecordSeverity,
        /// The identifier of the condition (`flm.refs.bad-label`), as the projection gives
        /// it.
        id: String,
        /// The data of the condition, as the projection gives it.
        data: Value,
        /// The message, written for a person; not a stable interface.
        message: String,
        /// The leading position ([`lead_position`]), when one is known.
        #[serde(skip_serializing_if = "Option::is_none")]
        place: Option<Place>,
        /// The frames of the traceback, in order; empty when there are none.
        frames: Vec<Frame>,
        /// The position in the content the library made, when the problem is in such
        /// content: the place of the `in content made from it` line of [`diagnostic`].
        #[serde(skip_serializing_if = "Option::is_none")]
        made_from: Option<Place>,
    },
    /// A problem outside a document's diagnostics: a problem that stopped the work before
    /// the document was processed ([`Record::of_problem`], an Error with the place of the
    /// problem when one is known), or a message of the program itself that no diagnostic
    /// carries, an Error (a wrong argument, an output that cannot be written) or a Warning
    /// that does not stop the work, made with no place.
    Problem {
        /// The severity: [`Error`](RecordSeverity::Error) for a problem that stopped the
        /// work or an error of the program, [`Warning`](RecordSeverity::Warning) for a
        /// warning of the program that does not stop the work.
        severity: RecordSeverity,
        /// The message, written for a person.
        message: String,
        /// The file and the place in it, when either is known.
        #[serde(skip_serializing_if = "Option::is_none")]
        place: Option<Place>,
    },
    /// The counts that end a report ([`Record::summary`]).
    Summary {
        /// The number of Errors.
        errors: usize,
        /// The number of Warnings (Notes are not counted).
        warnings: usize,
    },
    /// The start of a build of the live preview ([`Record::build`]).
    Build {
        /// The number of the build, counted from 1 for the first build.
        number: u64,
        /// The time at which the build started, in milliseconds since the Unix epoch
        /// (1970-01-01 00:00 UTC).
        time: u64,
    },
    /// A line that tells what the program does, such as which files it read.
    Log {
        /// The line's text.
        text: String,
    },
}

impl Record {
    /// Returns the header that opens a stream of records: the kind `diagnostics`, the
    /// version of FLM, and the version 1 of the records' format.
    pub fn meta() -> Meta {
        Meta::new("diagnostics", 1)
    }

    /// Returns `diagnostic` as a record: the identifier and the data of its condition, as
    /// [`project_diagnostic`] gives them, its message, its leading position, its frames, and
    /// the position in the content the library made, when [`diagnostic`] shows one.
    /// `offsets` holds the content of each file the library parsed; a place in one of those
    /// contents gets its byte offset counted from the top of its file ([`ContentOffsets`]).
    pub fn of_diagnostic(diagnostic: &FlmDiagnostic, offsets: &ContentOffsets) -> Record {
        // The identifier and the data, as the projection spells them.
        let projected = project_diagnostic(diagnostic, &ProjectOptions::new());
        let entry = |key: &str| match &projected {
            SerialValue::Map(entries) => entries.iter().find(|(name, _)| name == key).map(|(_, value)| value),
            _ => None,
        };
        let id = match entry("id") {
            Some(SerialValue::Str(id)) => id.clone(),
            _ => String::new(),
        };
        // Data holding a map key that begins with `$`, which techy reserves, has no JSON
        // form, and is recorded as `null`.
        let data = entry("data").and_then(|data| serde_json::to_value(data).ok()).unwrap_or(Value::Null);
        let mut placer = Placer::new(offsets);
        let span = diagnostic.span();
        let lead = lead_position(span);
        Record::Diagnostic {
            severity: diagnostic.severity().into(),
            id,
            data,
            message: diagnostic.message(),
            place: lead.as_ref().map(|lead| placer.place(lead)),
            frames: diagnostic
                .frames()
                .iter()
                .map(|frame| Frame { place: placer.place(frame.span()), title: frame.title().to_string() })
                .collect(),
            made_from: shows_made_from(span, lead.as_ref()).then(|| placer.place(span)),
        }
    }

    /// Returns `problem` as a record, with the severity Error and the place of its file and
    /// line and column, when either is known.
    pub fn of_problem(problem: &Problem) -> Record {
        let place = (problem.file.is_some() || problem.place.is_some()).then(|| Place {
            file: problem.file.clone(),
            line: problem.place.map(|place| place.line as usize),
            column: problem.place.map(|place| place.column as usize),
            ..Place::default()
        });
        Record::Problem { severity: RecordSeverity::Error, message: problem.detail.clone(), place }
    }

    /// Returns the counts that end the report of `diagnostics`: the number of Errors and of
    /// Warnings, as [`report`] counts them.
    pub fn summary(diagnostics: &[FlmDiagnostic]) -> Record {
        let (errors, warnings) = tally(diagnostics);
        Record::Summary { errors, warnings }
    }

    /// Returns the start of the build numbered `number` of the live preview, which started
    /// at `time`; a time before the Unix epoch is recorded as 0.
    pub fn build(number: u64, time: SystemTime) -> Record {
        let since = time.duration_since(UNIX_EPOCH).unwrap_or_default();
        Record::Build { number, time: u64::try_from(since.as_millis()).unwrap_or(u64::MAX) }
    }
}
