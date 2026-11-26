//! The progress of a build of `flm render` and `flm check`, shown while the build runs: as
//! one line on standard error, redrawn in place, when standard error is a terminal and the
//! program writes text; as a `progress` record every 500 ms under `--json-messages`. The
//! builds of `flm watch` show no progress line; under `--json-messages`, their progress is
//! written as records too ([`WatchRecords`]).
//!
//! A thread of [`Display`] reads the build's progress sheet, and estimates the fraction done
//! with the host's table of expected durations ([`expected_durations`]) as the host's
//! displays do, the highest fraction since the build started ([`ShownFraction`]), so that
//! what it shows never goes back. The text is the host's ([`progress_line`],
//! [`Record::progress`]).
//!
//! The reporter stays the one writer of standard error: every line of text it writes goes
//! through [`write_line`], which removes the progress line first and draws it again after.

use std::io::Write;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use flm_core::progress::{Sheet, Snapshot};
use flm_host::report::{Record, ShownFraction, expected_durations, progress_line};
use flm_host::watch::{Event, Handle};

use crate::Reporter;

/// How long a build runs before its progress is first shown.
const FIRST: Duration = Duration::from_millis(500);

/// The time between two drawings of the progress line: at most ten a second.
const LINE_EVERY: Duration = Duration::from_millis(100);

/// The time between two `progress` records.
const RECORD_EVERY: Duration = Duration::from_millis(500);

/// The width of the whole progress line, in columns, the bar and the text together.
const WIDTH: usize = 80;

/// The number of cells of the bar, between its brackets.
const BAR: usize = 20;

/// How a build's progress is shown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shown {
    /// The progress line on standard error, a terminal.
    Line,
    /// A `progress` record every 500 ms, when it differs from the last one written.
    Records,
}

/// The progress line as it stands on standard error.
struct Line {
    /// Whether the line may be drawn: from the start of a [`Display`] that draws it, until
    /// the display stops or [`close_line`] removes it.
    open: bool,
    /// The text of the line drawn, while one is on the screen.
    drawn: Option<String>,
}

/// The progress line of the program, shared by the drawing thread and the reporter.
static LINE: Mutex<Line> = Mutex::new(Line { open: false, drawn: None });

/// Writes `text` and a newline on standard error, removing the progress line before it
/// when one is drawn and drawing the line again after it.
pub(crate) fn write_line(text: &str) {
    let line = LINE.lock().unwrap_or_else(PoisonError::into_inner);
    let mut stderr = std::io::stderr().lock();
    match &line.drawn {
        None => {
            let _ = writeln!(stderr, "{text}");
        }
        Some(drawn) => {
            let _ = write!(stderr, "\r\x1b[K{text}\n{drawn}\x1b[K");
            let _ = stderr.flush();
        }
    }
}

/// Removes the progress line, when one is drawn, and draws it no more for this build. The
/// program calls it before it writes a document on standard output, which may be the same
/// terminal.
pub(crate) fn close_line() {
    let mut line = LINE.lock().unwrap_or_else(PoisonError::into_inner);
    line.open = false;
    if line.drawn.take().is_some() {
        let mut stderr = std::io::stderr().lock();
        let _ = write!(stderr, "\r\x1b[K");
        let _ = stderr.flush();
    }
}

/// Draws `text` as the progress line, in place of the one drawn, while the line is open and
/// when the text differs from the one drawn.
fn draw_line(text: String) {
    let mut line = LINE.lock().unwrap_or_else(PoisonError::into_inner);
    if !line.open || line.drawn.as_ref() == Some(&text) {
        return;
    }
    let mut stderr = std::io::stderr().lock();
    let _ = write!(stderr, "\r{text}\x1b[K");
    let _ = stderr.flush();
    line.drawn = Some(text);
}

/// The text of the progress line: a bar of [`BAR`] cells that fills with `fraction`, then
/// the host's text ([`progress_line`]), cut so that the whole is at most [`WIDTH`] columns.
fn line_text(snapshot: &Snapshot, fraction: f64) -> String {
    let filled = ((fraction.clamp(0.0, 1.0) * BAR as f64).floor() as usize).min(BAR);
    let mut text = format!("[{}{}] ", "#".repeat(filled), "-".repeat(BAR - filled));
    let room = WIDTH - (BAR + 3);
    text.extend(progress_line(snapshot, fraction).chars().take(room));
    text
}

/// The display of one build's progress: a thread that shows it, from [`FIRST`] after the
/// build started until the display is dropped. Dropping it stops the thread and removes
/// the progress line.
pub(crate) struct Display {
    /// The thread and the sender that stops it, by being dropped; `None` when nothing is
    /// shown.
    running: Option<(Sender<()>, JoinHandle<()>)>,
}

impl Display {
    /// Starts showing, as `shown` says, the progress of the build whose sheet is `sheet`,
    /// which started at `origin`, the origin of the times of its rows; with `shown` `None`,
    /// shows nothing.
    pub(crate) fn start(sheet: &Sheet, origin: Instant, shown: Option<Shown>) -> Display {
        let Some(shown) = shown else {
            return Display { running: None };
        };
        if shown == Shown::Line {
            let mut line = LINE.lock().unwrap_or_else(PoisonError::into_inner);
            line.open = true;
        }
        let (stop, stopped) = mpsc::channel::<()>();
        let sheet = sheet.clone();
        let thread = std::thread::spawn(move || {
            let expected = expected_durations();
            let every = match shown {
                Shown::Line => LINE_EVERY,
                Shown::Records => RECORD_EVERY,
            };
            let mut wait = FIRST.saturating_sub(origin.elapsed());
            let mut highest = ShownFraction::new();
            let mut last: Option<Record> = None;
            // Nothing is ever sent: the loop ends when the display drops the sender.
            while let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(wait) {
                let snapshot = sheet.snapshot();
                let fraction = highest.estimate(&snapshot, &expected, origin.elapsed());
                match shown {
                    Shown::Line => draw_line(line_text(&snapshot, fraction)),
                    Shown::Records => {
                        let record = Record::progress(&snapshot, fraction);
                        if last.as_ref() != Some(&record) {
                            Reporter::record(&record);
                            last = Some(record);
                        }
                    }
                }
                wait = every;
            }
        });
        Display { running: Some((stop, thread)) }
    }
}

impl Drop for Display {
    fn drop(&mut self) {
        if let Some((stop, thread)) = self.running.take() {
            drop(stop);
            let _ = thread.join();
            close_line();
        }
    }
}

/// The `progress` records of the builds of `flm watch`, under `--json-messages`: a thread
/// that reads the progress of the build under way through the watch loop's handle
/// ([`Handle::progress`]) every 500 ms, and writes its record when it differs from the last
/// one written for that build. Dropping it stops the thread.
pub(crate) struct WatchRecords {
    /// What was written of the build under way, shared with the thread.
    written: Arc<Mutex<Written>>,
    /// The thread and the sender that stops it, by being dropped.
    running: Option<(Sender<()>, JoinHandle<()>)>,
}

/// What was written of the progress of the build under way, from its start.
#[derive(Default)]
pub(crate) struct Written {
    /// The fraction of the build done, as the records show it.
    fraction: ShownFraction,
    /// The last record written; `None` before the first.
    last: Option<Record>,
}

impl WatchRecords {
    /// Starts writing the records of the builds of the watch loop of `handle`.
    pub(crate) fn start(handle: Handle) -> WatchRecords {
        let written = Arc::new(Mutex::new(Written::default()));
        let (stop, stopped) = mpsc::channel::<()>();
        let thread = {
            let written = Arc::clone(&written);
            std::thread::spawn(move || {
                // Nothing is ever sent: the loop ends when the records are dropped.
                while let Err(RecvTimeoutError::Timeout) = stopped.recv_timeout(RECORD_EVERY) {
                    // The build under way is read while what was written is locked, as
                    // `told` holds it: the watch loop forgets a build's progress before it
                    // hands the build's end, so no record of a build comes after the
                    // program has reported that end.
                    let mut written = written.lock().unwrap_or_else(PoisonError::into_inner);
                    let Some(progress) = handle.progress() else { continue };
                    let snapshot = progress.sheet.snapshot();
                    let fraction = written.fraction.estimate(&snapshot, &progress.expected, progress.origin.elapsed());
                    let record = Record::progress(&snapshot, fraction);
                    if written.last.as_ref() != Some(&record) {
                        Reporter::record(&record);
                        written.last = Some(record);
                    }
                }
            })
        };
        WatchRecords { written, running: Some((stop, thread)) }
    }

    /// Holds the records back while the program reports `event` of the watch loop, for as
    /// long as the guard returned lives; a build that starts is written from its start.
    pub(crate) fn told(&self, event: &Event) -> MutexGuard<'_, Written> {
        let mut written = self.written.lock().unwrap_or_else(PoisonError::into_inner);
        if let Event::Building { .. } = event {
            *written = Written::default();
        }
        written
    }
}

impl Drop for WatchRecords {
    fn drop(&mut self) {
        if let Some((stop, thread)) = self.running.take() {
            drop(stop);
            let _ = thread.join();
        }
    }
}
