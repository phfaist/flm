//! The work's four handles: `Scope`, `Pending`, `Task` and `Counted`.

use std::fmt;
use std::mem;
use std::sync::Arc;

use super::kind::Kind;
use super::log::{LogLevel, LogRecord};
use super::sheet::{Row, RowState, Shared, Unit};

/// What a handle has behind it. Every operation of a handle is one match on
/// this value.
#[derive(Clone)]
enum Behind {
    /// No sheet: the build reports nowhere.
    Nothing,
    /// A sheet but no row of the handle's own: the handle is past a depth
    /// limit, or was declared under a row that had ended. `nearest` is the
    /// nearest row that exists, to which log lines are attributed.
    Above { sheet: Arc<Shared>, nearest: Arc<Row> },
    /// The handle's own row. `below` is how many levels of rows may exist
    /// below it (`None`: no limit).
    Row { sheet: Arc<Shared>, row: Arc<Row>, below: Option<u32> },
}

impl Behind {
    /// The same handle, with no more than `levels` levels of rows below its
    /// row.
    fn limited(mut self, levels: u32) -> Behind {
        if let Behind::Row { below, .. } = &mut self {
            *below = Some(below.map_or(levels, |below| below.min(levels)));
        }
        self
    }

    fn fmt_row(&self, name: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Behind::Row { row, .. } => write!(f, "{name}({})", row.id.0),
            _ => write!(f, "{name}(no row)"),
        }
    }
}

/// The handle lent to the code that works under a running row.
///
/// Through a scope the work declares rows under that row
/// ([`child`](Scope::child)), asks whether the build is cancelled
/// ([`cancelled`](Scope::cancelled)), and writes log lines
/// ([`log`](Scope::log)). It cannot start, count or end anything: the owner of
/// the row does that through its [`Task`] or [`Counted`].
///
/// A scope is cheap to clone, so that a value that outlives a call can keep
/// one. [`Scope::none()`] is the scope of a build that reports nowhere; the
/// work calls the same methods on it, and each does nothing.
#[derive(Clone)]
pub struct Scope(Behind);

impl Scope {
    /// The scope of a build that reports nowhere: it declares no row,
    /// [`cancelled`](Scope::cancelled) is `false`, and a log line goes
    /// nowhere.
    pub fn none() -> Scope {
        Scope(Behind::Nothing)
    }

    /// Declares a row of kind `kind`, labeled `label`, under this scope's row,
    /// and returns it, not started.
    ///
    /// The returned handle has no row behind it, and does nothing, when this
    /// scope has none, when the depth limit allows no row below this scope's
    /// row ([`Pending::with_depth_limit`]), or when this scope's row has
    /// ended.
    ///
    /// The row declared stands for a part of this row's work; under a row
    /// that has a count, it stands for one of the counted units, shown in
    /// detail, and the owner of the counted row advances it when that unit is
    /// done.
    pub fn child(&self, kind: &Kind, label: Option<&str>) -> Pending {
        match &self.0 {
            Behind::Nothing => Pending(Behind::Nothing),
            Behind::Above { sheet, nearest } => {
                Pending(Behind::Above { sheet: Arc::clone(sheet), nearest: Arc::clone(nearest) })
            }
            Behind::Row { sheet, row, below } => {
                if *below == Some(0) || row.state() != RowState::Running {
                    Pending(Behind::Above { sheet: Arc::clone(sheet), nearest: Arc::clone(row) })
                } else {
                    let child = sheet.declare(Some(row.id), kind, label);
                    Pending(Behind::Row {
                        sheet: Arc::clone(sheet),
                        row: child,
                        below: below.map(|below| below - 1),
                    })
                }
            }
        }
    }

    /// The same scope, with no more than `levels` levels of rows declared
    /// below its row through it: 0 for none. A limit already in place that is
    /// lower stays.
    pub fn with_depth_limit(&self, levels: u32) -> Scope {
        Scope(self.0.clone().limited(levels))
    }

    /// Whether the build was asked to stop.
    ///
    /// The host's [`checkpoint`](super::Observer::checkpoint) function is
    /// called first, when it set one, and the flag is read after it. The work
    /// asks at the grain of its units (once for a tree, an image, a formula),
    /// not inside an inner loop. A build that was asked to stop produces
    /// nothing from the step under way, so the work that sees `true` returns
    /// as soon as it can, and what it returns need not be meaningful.
    pub fn cancelled(&self) -> bool {
        let sheet = match &self.0 {
            Behind::Nothing => return false,
            Behind::Above { sheet, .. } | Behind::Row { sheet, .. } => sheet,
        };
        if let Some(checkpoint) = &sheet.observer.checkpoint {
            checkpoint();
        }
        sheet.cancelled()
    }

    /// Whether the build was asked to stop, read from the flag alone, without
    /// a call of the host's checkpoint: the run reads it to choose the state in
    /// which it ends a stage's row, which is not a question about the
    /// cancellation.
    pub(crate) fn cancel_requested(&self) -> bool {
        match &self.0 {
            Behind::Nothing => false,
            Behind::Above { sheet, .. } | Behind::Row { sheet, .. } => sheet.cancelled(),
        }
    }

    /// Whether the host's log sink wants the lines of `level`: a caller tests
    /// it before building the text of a line.
    pub fn log_enabled(&self, level: LogLevel) -> bool {
        match &self.0 {
            Behind::Nothing => false,
            Behind::Above { sheet, .. } | Behind::Row { sheet, .. } => {
                sheet.log_sink.as_ref().is_some_and(|sink| sink.enabled(level))
            }
        }
    }

    /// Writes the log line `text` at `level` to the host's log sink.
    ///
    /// The line is attributed to this scope's row, or, for a scope past a
    /// depth limit, to the nearest row that exists: the record holds that
    /// row's id and label, and the name of its kind as the source.
    pub fn log(&self, level: LogLevel, text: &str) {
        let (sheet, row) = match &self.0 {
            Behind::Nothing => return,
            Behind::Above { sheet, nearest: row } | Behind::Row { sheet, row, .. } => (sheet, row),
        };
        if let Some(sink) = &sheet.log_sink {
            sink.log(&LogRecord {
                level,
                source: row.kind.name(),
                text,
                row: Some(row.id),
                label: row.label.as_deref(),
            });
        }
    }
}

impl fmt::Debug for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt_row("Scope", f)
    }
}

/// A declared row, not started, owned by the code that declared it.
///
/// The owner starts it with [`start`](Pending::start) or, for a row with a
/// count, [`start_counted`](Pending::start_counted). A `Pending` that is
/// dropped ends as [`RowState::NotRun`]: an owner declares a piece of work
/// that may be needed before it knows, and drops the handle when the work is
/// not needed.
#[must_use = "a declared row that is dropped ends as not run"]
pub struct Pending(Behind);

impl Pending {
    /// The same row, with no more than `levels` levels of rows below it: 0 for
    /// none. A handle past the limit has no row behind it; it still answers
    /// [`Scope::cancelled`], and its log lines are attributed to the nearest
    /// row that exists. A limit already in place that is lower stays.
    pub fn with_depth_limit(mut self, levels: u32) -> Pending {
        let behind = mem::replace(&mut self.0, Behind::Nothing);
        Pending(behind.limited(levels))
    }

    /// Starts the row, with nothing to count.
    pub fn start(mut self) -> Task {
        let behind = mem::replace(&mut self.0, Behind::Nothing);
        if let Behind::Row { sheet, row, .. } = &behind {
            sheet.start(row, None);
        }
        Task(behind)
    }

    /// Starts the row with a count of `total` units of `unit`. The total is
    /// fixed: it never changes after the row starts.
    pub fn start_counted(mut self, unit: Unit, total: u64) -> Counted {
        let behind = mem::replace(&mut self.0, Behind::Nothing);
        if let Behind::Row { sheet, row, .. } = &behind {
            sheet.start(row, Some((unit, total)));
        }
        Counted(behind)
    }

    /// Starts the row with a count of units of `unit`, whose total `total`
    /// computes. `total` is called only when a row is behind the handle.
    pub fn start_counted_with(mut self, unit: Unit, total: impl FnOnce() -> u64) -> Counted {
        let behind = mem::replace(&mut self.0, Behind::Nothing);
        if let Behind::Row { sheet, row, .. } = &behind {
            sheet.start(row, Some((unit, total())));
        }
        Counted(behind)
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if let Behind::Row { sheet, row, .. } = &self.0 {
            sheet.not_run(row);
        }
    }
}

impl fmt::Debug for Pending {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt_row("Pending", f)
    }
}

/// A running row with nothing to count, owned by the code that started it.
///
/// The owner lends its [`scope`](Task::scope) to the work under it and ends
/// it with [`finish`](Task::finish). A `Task` dropped without it, by an early
/// return or a panic, ends as [`RowState::Failed`].
#[must_use = "a running row that is dropped ends as failed"]
pub struct Task(Behind);

impl Task {
    /// The task of the root row of a sheet, which has just started.
    pub(super) fn running(sheet: Arc<Shared>, row: Arc<Row>) -> Task {
        Task(Behind::Row { sheet, row, below: None })
    }

    /// The scope of the row, for the work under it.
    pub fn scope(&self) -> Scope {
        Scope(self.0.clone())
    }

    /// Ends the row as [`RowState::Finished`].
    pub fn finish(mut self) {
        end(&mut self.0, RowState::Finished);
    }
}

impl Drop for Task {
    fn drop(&mut self) {
        end(&mut self.0, RowState::Failed);
    }
}

impl fmt::Debug for Task {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt_row("Task", f)
    }
}

/// A running row with a count, owned by the code that started it.
///
/// [`advance`](Counted::advance) adds to the number done; the rest is as for
/// a [`Task`]. Several threads may advance one `Counted` at the same time.
#[must_use = "a running row that is dropped ends as failed"]
pub struct Counted(Behind);

impl Counted {
    /// Adds `n` to the number done: one atomic addition. The number done never
    /// goes past the total: an owner that advances further has miscounted,
    /// and the count stays at the total.
    pub fn advance(&self, n: u64) {
        if let Behind::Row { row, .. } = &self.0 {
            row.done.fetch_add(n, std::sync::atomic::Ordering::Relaxed);
        }
    }

    /// The scope of the row, for the work under it.
    pub fn scope(&self) -> Scope {
        Scope(self.0.clone())
    }

    /// Ends the row as [`RowState::Finished`].
    pub fn finish(mut self) {
        end(&mut self.0, RowState::Finished);
    }
}

impl Drop for Counted {
    fn drop(&mut self) {
        end(&mut self.0, RowState::Failed);
    }
}

impl fmt::Debug for Counted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt_row("Counted", f)
    }
}

/// Ends the running row behind a handle as `state`, once: the handle is left
/// with nothing behind it.
fn end(behind: &mut Behind, state: RowState) {
    if let Behind::Row { sheet, row, .. } = mem::replace(behind, Behind::Nothing) {
        sheet.end(&row, state);
    }
}

// Every handle may be sent to and shared between threads.
const _: () = {
    const fn send_sync<T: Send + Sync>() {}
    send_sync::<Scope>();
    send_sync::<Pending>();
    send_sync::<Task>();
    send_sync::<Counted>();
    send_sync::<super::Sheet>();
};
