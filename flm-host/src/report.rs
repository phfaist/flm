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
//! # Records
//!
//! A [`Record`] holds one thing a program reports, as data that serializes to JSON, for a
//! program that reads the reports of another (an editor, a script): a diagnostic, a
//! problem outside a document's diagnostics (one that stopped the work, or an error or a
//! warning of the program itself), the counts that end a report, the link at which the
//! live preview is viewed, the start of a build of the live preview, a line that tells what
//! the program does, or the state of a build's progress. A program writes the records as a stream, one JSON value per line, and writes
//! first the header of that stream, [`Record::meta`], which gives the version of FLM and of
//! the records' format.
//!
//! A line's record gives its level (`error`, `warn` or `info`), its source (a short name of
//! who writes it) and its text. A progress record gives the state of a build as the
//! progress line shows it: the fraction done, the text of the line, and the row of the
//! progress sheet ([`flm_core::progress`]) that the text shows, with the name of its kind,
//! its title, its label and its count.
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
//!
//! # The progress of a build
//!
//! A program that shows the progress of a build reads the build's progress sheet
//! ([`flm_core::progress`]) from time to time, estimates the fraction done with
//! [`flm_core::progress::estimate`] and the table of expected durations shipped with this
//! crate ([`expected_durations`]), and shows the text that [`progress_line`] gives, or
//! writes the record that [`Record::progress`] makes. Both show the same row of the sheet:
//! the most detailed running row that has a title.
//!
//! # The preview's problems
//!
//! The live preview lists the problems of each build beside the document, and marks the
//! elements of the page in which each lies. [`Reported`] holds one of them: its record,
//! made by [`Record::of_diagnostic`] or [`Record::of_problem`] as for any other program,
//! and two things more that the preview needs.
//!
//! - The range of bytes of the leading position in the text the library parsed, counted
//!   as the HTML backend counts the place it writes into each element of the page
//!   (`data-flm-source-span`), so that the page finds the elements of a problem.
//! - The *excerpt*: the line of the file on which the leading position starts, and the part
//!   of it that the problem covers ([`Excerpt`]). A problem that covers several lines gives
//!   the first, and the part of it from the problem's start to the end of the line. Only a
//!   place in a file's content gives one: the text of the line is the source's own, the
//!   text the build parsed.

use std::collections::BTreeMap;
use std::ops::Range;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use flm_core::dump::{Meta, ProjectOptions, project_diagnostic};
use flm_core::lang::{FlmDiagnostic, FlmOrigin, FlmSource, FlmSpan};
use flm_core::progress::{Count, Expected, LogLevel, RowId, RowSnapshot, RowState, Snapshot, estimate};
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
struct Position {
    /// The file's name, or the label of the content the library made; `None` when the
    /// source has no label.
    file: Option<String>,
    /// The line and the column, counted from 1, when known.
    line_column: Option<(usize, usize)>,
}

impl Position {
    fn of(span: &FlmSpan, cache: &mut LineIndexCache<FlmOrigin>) -> Position {
        let source = span.source();
        Position { file: source.origin().label().map(|label| label.into_owned()), line_column: cache.line_col(source, span.start()) }
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
/// as the live preview's stream carries it: see "The preview's problems" in the [module
/// documentation](self). It serializes to the JSON map of its [`Record`], with the keys
/// `bytes` and `excerpt` added when they have a value.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reported {
    /// The record, as [`Record::of_diagnostic`] or [`Record::of_problem`] makes it: a
    /// [`Record::Diagnostic`] or a [`Record::Problem`].
    #[serde(flatten)]
    pub record: Record,
    /// The range of bytes the leading position covers in its source, when known. It is
    /// counted in the text the library parsed, which, for a file with a front matter, is
    /// the content below the front matter, as the HTML backend counts the place it writes
    /// into an element. A [`Problem`] has none, nor has a diagnostic placed in a file by
    /// its line and column alone, with no text of the file (a warning about a setting,
    /// [`warning_diagnostic`](crate::settings::warning_diagnostic)).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bytes: Option<Range<usize>>,
    /// The line of the leading position, when that position is in a file's content
    /// ([`Excerpt`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<Excerpt>,
}

impl Reported {
    /// Returns `diagnostic` as the preview's stream carries it: its record
    /// ([`Record::of_diagnostic`], with the byte offsets that `offsets` gives), the range of
    /// bytes of its leading position, and, when the record places that position in a
    /// file's content, the excerpt of its line.
    pub fn of_diagnostic(diagnostic: &FlmDiagnostic, offsets: &ContentOffsets) -> Reported {
        let record = Record::of_diagnostic(diagnostic, offsets);
        let lead = lead_position(diagnostic.span());
        let in_content = matches!(&record, Record::Diagnostic { place: Some(Place { offset: Some(_), .. }), .. });
        let excerpt = lead.as_ref().filter(|_| in_content).and_then(Excerpt::of);
        let bytes = lead.filter(|lead| !lead.source().content().is_empty()).map(|lead| lead.start()..lead.end());
        Reported { record, bytes, excerpt }
    }

    /// Returns `problem` as the preview's stream carries it: its record
    /// ([`Record::of_problem`]) alone.
    pub fn of_problem(problem: &Problem) -> Reported {
        Reported { record: Record::of_problem(problem), bytes: None, excerpt: None }
    }
}

/// The line of a file that a problem's leading position is on, in a [`Reported`]: the
/// line's number, counted from 1 at the top of the file, and its text, without its line
/// break, cut in three at the start and at the end of the range of bytes the problem
/// covers. A range that goes on past the end of the line is cut at the end of the line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Excerpt {
    /// The line's number, counted from 1.
    pub line: usize,
    /// The text of the line before the range.
    pub before: String,
    /// The text of the line in the range.
    pub covered: String,
    /// The text of the line after the range.
    pub after: String,
}

impl Excerpt {
    /// Returns the excerpt of the line on which `span` starts, in the text of the span's
    /// source; `None` when no line is known for it.
    fn of(span: &FlmSpan) -> Option<Excerpt> {
        let source = span.source();
        let content = source.content();
        // Every source is indexed, whatever its length: the index's default limit is for
        // the display of a position in a message, where a byte position stands in.
        let mut lines = LineIndexCache::<FlmOrigin>::new();
        lines.set_max_scan_len(content.len());
        let (line, range) = lines.line_of(source, span.start())?;
        let text = &content[range.clone()];
        let text = text.strip_suffix('\r').unwrap_or(text);
        let start = (span.start() - range.start).min(text.len());
        let end = (span.end() - range.start).clamp(start, text.len());
        Some(Excerpt {
            line,
            before: text[..start].to_string(),
            covered: text[start..end].to_string(),
            after: text[end..].to_string(),
        })
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
/// in lowercase (`diagnostic`, `problem`, `summary`, `preview-link`, `build`, `log`,
/// `progress`); a key with no value is left out.
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
    /// The link at which the live preview is viewed in a browser, written once, when the
    /// preview starts: the address of its tools page.
    #[serde(rename = "preview-link")]
    PreviewLink {
        /// The address (`http://127.0.0.1:18910/.flm-preview/`).
        url: String,
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
        /// The line's level.
        level: LogLevel,
        /// A short name of who writes the line: the program, or the kind of the row of the
        /// progress sheet under which the line was written
        /// ([`LogRecord::source`](flm_core::progress::LogRecord::source)).
        source: String,
        /// The line's text.
        text: String,
    },
    /// The state of a build, as the progress line shows it: see "Records" in the [module
    /// documentation](self).
    Progress {
        /// The fraction of the build done, from 0 to 1, as estimated from the progress
        /// sheet ([`flm_core::progress::estimate`]).
        fraction: f64,
        /// The text of the progress line: the percentage, then the row it shows.
        text: String,
        /// The row of the progress sheet that the text shows, when it shows one.
        #[serde(skip_serializing_if = "Option::is_none")]
        row: Option<ProgressRow>,
    },
}

/// The row of the progress sheet that a [`Record::Progress`] shows: the most detailed
/// running row that has a title. A key with no value is left out of the JSON.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ProgressRow {
    /// The name of the row's kind (`flm.graphics.convert`).
    pub kind: String,
    /// The title of the row's kind (`Converting graphics`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// The row's label, the item it tells apart from the other rows of its kind (a file, a
    /// citation key).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    /// The row's count, for a row that counts its work.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub count: Option<Count>,
}

impl Record {
    /// Returns the header that opens a stream of records: the kind `messages`, the
    /// version of FLM, and the version 1 of the records' format.
    pub fn meta() -> Meta {
        Meta::new("messages", 1)
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

    /// Returns the state of the build whose progress sheet `snapshot` shows, estimated as
    /// `fraction` done (from 0 to 1): the fraction, the text of the progress line
    /// ([`progress_line`]), and the row that the text shows, when it shows one.
    pub fn progress(snapshot: &Snapshot, fraction: f64) -> Record {
        let row = shown_row(snapshot).map(|row| ProgressRow {
            kind: row.kind.to_string(),
            title: row.title.as_ref().map(|title| title.to_string()),
            label: row.label.as_ref().map(|label| label.to_string()),
            count: row.count,
        });
        Record::Progress { fraction: fraction.clamp(0.0, 1.0), text: progress_line(snapshot, fraction), row }
    }
}

/// Returns the text of the progress line of the build whose progress sheet `snapshot`
/// shows, estimated as `fraction` done (from 0 to 1): the percentage, a whole number
/// followed by `%`, then the row that the line shows, when a row is shown: its title, its
/// label, and its count written `done/total` when it has one.
///
/// ```text
/// 42% Converting graphics figure-3.pdf
/// 60% Fetching citation data 3/12
/// ```
///
/// The row shown is the most detailed running row that has a title: the deepest in the
/// tree of rows, and of several at that depth the one declared first. A row whose kind has
/// no title is passed over, so a row of internal work shows the titled row above it. The
/// percentage stays below 100 until the root row has finished.
///
/// The estimate of a fraction can decrease from one moment to the next
/// ([`flm_core::progress::estimate`]): a display passes the highest fraction it has
/// computed for the build ([`ShownFraction`]), so that the percentage it shows never
/// decreases.
pub fn progress_line(snapshot: &Snapshot, fraction: f64) -> String {
    let finished = snapshot.rows.iter().find(|row| row.parent.is_none()).is_some_and(|root| root.state == RowState::Finished);
    // The floor, so that a build is at 100% only when the estimate is whole.
    let percent = (fraction.clamp(0.0, 1.0) * 100.0).floor() as u32;
    let percent = if finished { percent } else { percent.min(99) };
    let mut text = format!("{percent}%");
    if let Some(row) = shown_row(snapshot) {
        for part in [&row.title, &row.label].into_iter().flatten() {
            text.push(' ');
            text.push_str(part.as_str());
        }
        if let Some(count) = row.count {
            text.push_str(&format!(" {}/{}", count.done, count.total));
        }
    }
    text
}

/// The fraction of one build done as a display shows it: the highest fraction estimated
/// since the build started, so that the percentage that [`progress_line`] and
/// [`Record::progress`] show never decreases, although an estimate can
/// ([`flm_core::progress::estimate`]). A display keeps one for each build, from its start.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ShownFraction(f64);

impl ShownFraction {
    /// Returns the fraction of a build that has done nothing.
    pub fn new() -> ShownFraction {
        ShownFraction(0.0)
    }

    /// Estimates the fraction done of the build whose progress sheet `snapshot` shows, with
    /// the table `expected`, at `now` from the origin of the sheet's times
    /// ([`flm_core::progress::estimate`]), and returns the highest fraction estimated so far,
    /// this one included: the fraction to pass to [`progress_line`] and [`Record::progress`].
    pub fn estimate(&mut self, snapshot: &Snapshot, expected: &Expected, now: Duration) -> f64 {
        self.0 = self.0.max(estimate(snapshot, expected, now));
        self.0
    }
}

/// The row of `snapshot` that the progress line shows ([`progress_line`]): of the running
/// rows that have a title, the deepest, and of several at that depth the one declared
/// first; `None` when no running row has a title.
fn shown_row(snapshot: &Snapshot) -> Option<&RowSnapshot> {
    let depths = depths(snapshot);
    let mut shown: Option<(usize, &RowSnapshot)> = None;
    for (row, depth) in snapshot.rows.iter().zip(depths) {
        if row.state != RowState::Running || row.title.is_none() {
            continue;
        }
        // The rows come in the order of their ids: a row at the same depth as the one
        // kept was declared after it.
        if shown.is_none_or(|(kept, _)| depth > kept) {
            shown = Some((depth, row));
        }
    }
    shown.map(|(_, row)| row)
}

/// The depth of each row of `snapshot`, in the order of the rows: 0 for the root, and one
/// more than its parent's for every other row. A row's parent comes before it, being
/// declared before it; a row whose parent is not in the snapshot is counted at depth 0.
fn depths(snapshot: &Snapshot) -> Vec<usize> {
    let mut known: BTreeMap<RowId, usize> = BTreeMap::new();
    snapshot
        .rows
        .iter()
        .map(|row| {
            let depth = row.parent.and_then(|parent| known.get(&parent)).map_or(0, |depth| depth + 1);
            known.insert(row.id, depth);
            depth
        })
        .collect()
}

/// The table of expected durations shipped with this crate, as a host weighs the rows of a
/// build's progress sheet with it ([`flm_core::progress::estimate`]), alone or under a
/// table measured from an earlier build ([`Expected::over`]).
///
/// The table holds the kinds of the rows that the host's builds declare on sample
/// documents: the row of a build ([`BUILD_KIND`](crate::session::BUILD_KIND)), the parts
/// of [`prepare`](crate::session::prepare) and [`build`](crate::session::build), the
/// stages of the packages those documents use, and the slow work under them (a conversion
/// of graphics, a fetch of citation data, a program that an extension runs, the formulas
/// that the math processor `mathjax` renders). A kind missing from the table gets the
/// median weight of the rows beside it. Its durations were measured over those documents
/// on one machine, those of the work that needs a program or the network set by hand: they
/// give the parts of a build their proportions, not the time a build takes.
pub fn expected_durations() -> Expected {
    // The file is checked by the crate's tests; an unreadable table weighs nothing.
    serde_json::from_str(include_str!("expected_durations.json")).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use flm_core::lang::{FlmOrigin, FlmSource, FlmSpan};
    use flm_core::progress::Scope;
    use serde_json::{Value, json};

    use super::{Excerpt, Reported};
    use crate::files::MemoryFiles;
    use crate::session::{Built, Input, Request, build, prepare};

    /// A request for a text render of `doc/paper.flm`.
    fn paper() -> Request {
        let mut request = Request::new(Input::File(String::from("doc/paper.flm")));
        request.default_math = crate::seed::MathChoice::processor("passthrough");
        request.profile = Some(String::from("text"));
        request
    }

    /// The build of a text render of `doc/paper.flm` holding `text`.
    fn built(text: &str) -> Built {
        let prepared = prepare(paper(), Arc::new(MemoryFiles::new().with("doc/paper.flm", text)), &[], &Scope::none())
            .expect("prepared");
        build(&prepared, &Scope::none()).expect("built")
    }

    /// Every diagnostic of `built` as the preview's stream carries it, as JSON.
    fn reported(built: &Built) -> Vec<Value> {
        built
            .diagnostics
            .iter()
            .map(|entry| serde_json::to_value(Reported::of_diagnostic(entry, &built.content_offsets)).expect("JSON"))
            .collect()
    }

    #[test]
    fn a_diagnostic_with_frames_is_its_record_with_its_bytes_and_its_excerpt() {
        let text = "---\ntitle: T\n---\n\\section*{See \\ref{nowhere}}\\label{sec:e}\n\nSee \\ref{sec:e}.\n";
        let built = built(text);
        let found = reported(&built);
        let framed: Vec<_> = built
            .diagnostics
            .iter()
            .zip(&found)
            .filter(|(_, entry)| entry["frames"].as_array().is_some_and(|frames| !frames.is_empty()))
            .collect();
        assert_eq!(framed.len(), 1, "{found:?}");
        let (diagnostic, entry) = framed[0];
        let record = serde_json::to_value(super::Record::of_diagnostic(diagnostic, &built.content_offsets)).unwrap();
        // The record's own keys, as `--json-messages` writes them.
        for key in ["kind", "severity", "id", "data", "message", "place", "frames"] {
            assert_eq!(entry[key], record[key], "{key}: {entry}");
        }
        assert_eq!(entry["kind"], "diagnostic");
        assert_eq!(entry["place"]["line"], 4);
        assert_eq!(entry["place"]["column"], 15);
        assert_eq!(entry["frames"][0]["place"]["line"], 6);
        assert_eq!(entry["frames"][0]["place"]["column"], 5);
        assert!(entry["frames"][0]["title"].is_string(), "{entry}");
        // The bytes are counted in the content below the front matter, the offset from the
        // top of the file.
        let content = &text[text.find("\\section").unwrap()..];
        let start = entry["bytes"]["start"].as_u64().unwrap() as usize;
        let end = entry["bytes"]["end"].as_u64().unwrap() as usize;
        assert!(content[start..end].starts_with("\\ref{nowhere}"), "{entry}");
        assert_eq!(entry["place"]["offset"].as_u64().unwrap() as usize, start + text.find("\\section").unwrap());
        assert_eq!(entry["excerpt"]["line"], 4);
        assert_eq!(entry["excerpt"]["before"], "\\section*{See ");
        assert!(entry["excerpt"]["covered"].as_str().unwrap().starts_with("\\ref{nowhere}"), "{entry}");
        assert_eq!(
            format!("{}{}{}", entry["excerpt"]["before"].as_str().unwrap(), entry["excerpt"]["covered"].as_str().unwrap(), entry["excerpt"]["after"].as_str().unwrap()),
            "\\section*{See \\ref{nowhere}}\\label{sec:e}"
        );
    }

    #[test]
    fn a_problem_of_a_stopped_build_is_its_record_alone() {
        let files = MemoryFiles::new().with("doc/paper.flm", "---\ntitle: A title\nauthor: {affiliation: A}\n---\nText.\n");
        let error = prepare(paper(), Arc::new(files), &[], &Scope::none()).expect_err("a problem of the front matter");
        let entry = serde_json::to_value(Reported::of_problem(&error.problems[0])).expect("JSON");
        assert_eq!(
            entry,
            json!({
                "kind": "problem",
                "severity": "error",
                "message": error.problems[0].detail,
                "place": {"file": "doc/paper.flm", "line": 3, "column": 9}
            })
        );
    }

    #[test]
    fn the_excerpt_of_a_problem_on_part_of_a_line_cuts_the_line_around_it() {
        let found = reported(&built("---\ntitle: T\n---\nHello.\n\nA \\nosuchmacro here.\n"));
        let error = found.iter().find(|entry| entry["severity"] == "error").expect("an error");
        assert_eq!(error["excerpt"]["line"], 6, "{error}");
        assert_eq!(error["excerpt"]["before"], "A ", "{error}");
        assert_eq!(error["excerpt"]["covered"].as_str().unwrap().trim_end(), "\\nosuchmacro", "{error}");
        assert_eq!(
            format!("{}{}", error["excerpt"]["covered"].as_str().unwrap(), error["excerpt"]["after"].as_str().unwrap()),
            "\\nosuchmacro here."
        );
    }

    #[test]
    fn the_excerpt_of_a_problem_over_several_lines_is_its_first_line() {
        // A file's content, below a front matter of three lines: its first line is the
        // file's fourth.
        let source = Arc::new(
            FlmSource::new("First line.\nA {group\nover lines} here.\r\nLast.").with_origin(FlmOrigin::new("doc/paper.flm")).with_line_column_number_offsets(4, 1),
        );
        let start = source.content().find('{').unwrap();
        let end = source.content().find('}').unwrap() + 1;
        let excerpt = Excerpt::of(&FlmSpan::new(&source, start..end)).expect("an excerpt");
        assert_eq!(
            excerpt,
            Excerpt { line: 5, before: String::from("A "), covered: String::from("{group"), after: String::new() }
        );
        // A line that ends with a carriage return gives its text without it.
        let start = source.content().find("here").unwrap();
        let excerpt = Excerpt::of(&FlmSpan::new(&source, start..start + 4)).expect("an excerpt");
        assert_eq!(
            excerpt,
            Excerpt { line: 6, before: String::from("over lines} "), covered: String::from("here"), after: String::from(".") }
        );
    }
}
