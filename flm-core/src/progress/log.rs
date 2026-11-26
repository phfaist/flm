//! The log sink: lines of text for a person about what a build does.

use super::RowId;

/// The level of a log line.
///
/// The levels are ordered from the most to the least severe: `Error <
/// Warn < Info`, so that a sink that shows the lines up to a chosen level
/// tests `level <= chosen`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Something went wrong in the work, such as a program that could not be
    /// started.
    Error,
    /// Something a person should know, such as a fetch that was retried.
    Warn,
    /// What the work is doing, for a person who asked for detail.
    Info,
}

/// One log line, as a [`LogSink`] receives it.
///
/// The record borrows its texts from the writer: building one allocates
/// nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LogRecord<'a> {
    /// The level of the line.
    pub level: LogLevel,
    /// A short name of who writes the line. For a line written through a
    /// [`Scope`](super::Scope), the name of the kind of its row.
    pub source: &'a str,
    /// The text of the line.
    pub text: &'a str,
    /// For a line written through a scope, the id of its row.
    pub row: Option<RowId>,
    /// For a line written through a scope, the label of its row, if the row
    /// has one.
    pub label: Option<&'a str>,
}

impl<'a> LogRecord<'a> {
    /// A record of the line `text` at `level`, written by `source`, outside
    /// any row.
    pub fn new(level: LogLevel, source: &'a str, text: &'a str) -> LogRecord<'a> {
        LogRecord { level, source, text, row: None, label: None }
    }
}

/// Where the log lines of a build, or of any other work, go.
///
/// A host gives a sink when it starts a [`Sheet`](super::Sheet), and the work
/// writes to it through [`Scope::log`](super::Scope::log). The trait stands on
/// its own: a host may give the same sink to code that runs outside any
/// build.
///
/// A log line is text for a person about what the work does; a problem of the
/// document is a diagnostic, never a log line.
///
/// The sink is called on the thread that writes the line, possibly from
/// several threads at once, and holds that work up until it returns.
pub trait LogSink: Send + Sync {
    /// Whether the sink wants the lines of `level`. A writer calls it before
    /// building the text of a line, to skip building a text nobody wants. A
    /// sink may still receive a record of a level it does not want, and then
    /// ignores it.
    fn enabled(&self, level: LogLevel) -> bool;

    /// Receives one line.
    fn log(&self, record: &LogRecord<'_>);
}
