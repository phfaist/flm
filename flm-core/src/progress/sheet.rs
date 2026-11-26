//! The sheet: its rows, the host's half and the snapshot.

use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, PoisonError};
use std::time::Duration;

use super::handles::Task;
use super::kind::{Kind, SharedText};
use super::log::LogSink;

/// The id of a row: a number given when the row is declared, in the order of
/// declaration, starting at 0 for the root row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct RowId(pub u64);

/// The state of a row.
///
/// A state only moves forward: `Pending` to `Running` to `Finished` or
/// `Failed`, or `Pending` to `NotRun`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RowState {
    /// Declared, not started.
    Pending,
    /// Started, not ended.
    Running,
    /// Ended, its work completed.
    Finished,
    /// Ended without completing its work: an error, a panic or a
    /// cancellation.
    Failed,
    /// Declared and never started.
    NotRun,
}

impl RowState {
    fn from_u8(value: u8) -> RowState {
        match value {
            0 => RowState::Pending,
            1 => RowState::Running,
            2 => RowState::Finished,
            3 => RowState::Failed,
            _ => RowState::NotRun,
        }
    }

    fn as_u8(self) -> u8 {
        match self {
            RowState::Pending => 0,
            RowState::Running => 1,
            RowState::Finished => 2,
            RowState::Failed => 3,
            RowState::NotRun => 4,
        }
    }
}

/// What the count of a row counts. A display names no unit; the estimate of a
/// percentage keeps the units of different kinds apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Unit {
    /// Things: trees, images, files, formulas.
    Items,
    /// Bytes.
    Bytes,
}

/// The count of a row: its unit, its total and the number done.
///
/// The total is given when the row starts and never changes; the number done
/// only grows, and never exceeds the total.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
pub struct Count {
    /// What is counted.
    pub unit: Unit,
    /// How many units the row's work holds.
    pub total: u64,
    /// How many of them are done.
    pub done: u64,
}

/// One row of a [`Snapshot`], as plain data.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RowSnapshot {
    /// The row's id.
    pub id: RowId,
    /// The id of the row under which it was declared; `None` for the root
    /// row.
    pub parent: Option<RowId>,
    /// The name of the row's kind.
    pub kind: SharedText,
    /// The title of the row's kind, if it has one.
    pub title: Option<SharedText>,
    /// The row's label, if its owner gave one.
    pub label: Option<SharedText>,
    /// The row's state.
    pub state: RowState,
    /// The row's count, for a row started with one.
    pub count: Option<Count>,
    /// The time at which the row started, as the host's observer returned it.
    pub started: Option<Duration>,
    /// The time at which the row ended, as the host's observer returned it.
    pub ended: Option<Duration>,
}

/// The rows of a sheet at one moment, as plain data, from
/// [`Sheet::snapshot`].
///
/// The list of rows is taken at one moment, and the counts are then read one
/// after the other: a snapshot is for a display and for an estimate, never for
/// a decision of the work.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Snapshot {
    /// Whether the build was asked to stop.
    pub cancelled: bool,
    /// The rows, in the order of their ids; the first is the root row.
    pub rows: Vec<RowSnapshot>,
}

/// The function of an [`Observer`] that tells of a row starting or ending, and
/// returns the time to store on it.
pub type RowFn = dyn Fn(&RowSnapshot) -> Option<Duration> + Send + Sync;

/// The function of an [`Observer`] called before each question about the
/// cancellation.
pub type CheckpointFn = dyn Fn() + Send + Sync;

/// The host's three functions, which a sheet calls while the work runs.
///
/// Each is optional: a function the host did not set costs one test where it
/// would be called, and no call. These are the only functions of the host's
/// that a sheet calls, besides its [`LogSink`].
///
/// ```
/// use std::time::Duration;
/// use flm_core::progress::Observer;
///
/// // A host with a clock of its own stamps the rows, and sets no checkpoint.
/// let clock = || Some(Duration::from_millis(0)); // the host's clock
/// let observer = Observer {
///     row_started: Some(Box::new(move |_row| clock())),
///     row_ended: Some(Box::new(move |_row| clock())),
///     ..Observer::default()
/// };
/// # let _ = observer;
/// ```
///
/// The contract of the three functions:
///
/// - They are called on the thread doing the work, possibly from several
///   threads at once, and hold the work up until they return.
/// - They are called outside any lock of the sheet. Inside one, the host may
///   take a [`Sheet::snapshot`] and may call [`Sheet::cancel`]; a cancellation
///   asked for inside `checkpoint` is seen by the question that follows it.
/// - Each row that starts gives exactly one call of `row_started` and, when it
///   ends, one call of `row_ended`. A row that is only declared, or that ends
///   as [`RowState::NotRun`], gives none.
/// - No function is called when a count advances.
///
/// A host with one thread (the library run as WebAssembly) does its display
/// inside these functions: in `checkpoint` it reads its own clock and, when
/// enough time has passed, takes a snapshot and shows it, and it may decide
/// there to cancel.
#[derive(Default)]
pub struct Observer {
    /// A row has started. The time returned, if any, is stored on the row as
    /// the time it started: the time elapsed since an origin the host chooses.
    pub row_started: Option<Box<RowFn>>,
    /// A row that had started has ended, as [`RowState::Finished`] or
    /// [`RowState::Failed`]. The time returned, if any, is stored as the time
    /// it ended.
    pub row_ended: Option<Box<RowFn>>,
    /// The work is about to ask whether the build is cancelled
    /// ([`Scope::cancelled`](super::Scope::cancelled)).
    pub checkpoint: Option<Box<CheckpointFn>>,
}

impl fmt::Debug for Observer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Observer")
            .field("row_started", &self.row_started.is_some())
            .field("row_ended", &self.row_ended.is_some())
            .field("checkpoint", &self.checkpoint.is_some())
            .finish()
    }
}

/// One row, shared by its handle and the sheet.
pub(super) struct Row {
    pub(super) id: RowId,
    parent: Option<RowId>,
    pub(super) kind: Kind,
    pub(super) label: Option<SharedText>,
    state: AtomicU8,
    count: OnceLock<(Unit, u64)>,
    pub(super) done: AtomicU64,
    started: OnceLock<Duration>,
    ended: OnceLock<Duration>,
}

impl Row {
    pub(super) fn state(&self) -> RowState {
        RowState::from_u8(self.state.load(Ordering::Acquire))
    }

    fn set_state(&self, state: RowState) {
        self.state.store(state.as_u8(), Ordering::Release);
    }

    fn snapshot(&self) -> RowSnapshot {
        RowSnapshot {
            id: self.id,
            parent: self.parent,
            kind: self.kind.shared_name().clone(),
            title: self.kind.shared_title().cloned(),
            label: self.label.clone(),
            state: self.state(),
            count: self.count.get().map(|&(unit, total)| Count {
                unit,
                total,
                done: self.done.load(Ordering::Relaxed).min(total),
            }),
            started: self.started.get().copied(),
            ended: self.ended.get().copied(),
        }
    }
}

/// What the two halves of a sheet share.
pub(super) struct Shared {
    cancelled: AtomicBool,
    pub(super) observer: Observer,
    pub(super) log_sink: Option<Arc<dyn LogSink>>,
    rows: Mutex<Vec<Arc<Row>>>,
}

impl Shared {
    pub(super) fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }

    /// Declares a row as `Pending`.
    pub(super) fn declare(&self, parent: Option<RowId>, kind: &Kind, label: Option<&str>) -> Arc<Row> {
        let label = label.map(|label| SharedText::from(Arc::<str>::from(label)));
        let mut rows = self.rows.lock().unwrap_or_else(PoisonError::into_inner);
        let row = Arc::new(Row {
            id: RowId(rows.len() as u64),
            parent,
            kind: kind.clone(),
            label,
            state: AtomicU8::new(RowState::Pending.as_u8()),
            count: OnceLock::new(),
            done: AtomicU64::new(0),
            started: OnceLock::new(),
            ended: OnceLock::new(),
        });
        rows.push(Arc::clone(&row));
        row
    }

    /// Starts a `Pending` row, with its count if it has one.
    pub(super) fn start(&self, row: &Row, count: Option<(Unit, u64)>) {
        if let Some(count) = count {
            let _ = row.count.set(count);
        }
        row.set_state(RowState::Running);
        if let Some(row_started) = &self.observer.row_started {
            if let Some(time) = row_started(&row.snapshot()) {
                let _ = row.started.set(time);
            }
        }
    }

    /// Ends a `Running` row as `Finished` or `Failed`.
    pub(super) fn end(&self, row: &Row, state: RowState) {
        row.set_state(state);
        if let Some(row_ended) = &self.observer.row_ended {
            if let Some(time) = row_ended(&row.snapshot()) {
                let _ = row.ended.set(time);
            }
        }
    }

    /// Ends a `Pending` row as `NotRun`.
    pub(super) fn not_run(&self, row: &Row) {
        row.set_state(RowState::NotRun);
    }
}

/// The progress sheet of one build, as the host holds it: it reads the rows
/// and asks the build to stop.
///
/// [`Sheet::start`] creates a sheet and gives its two halves: this one, and
/// the root row as a [`Task`], which the host lends to the work and ends when
/// the build is over. A `Sheet` is cheap to clone and may be read from any
/// thread.
#[derive(Clone)]
pub struct Sheet {
    shared: Arc<Shared>,
}

impl Sheet {
    /// Creates a sheet whose root row, of kind `root` and labeled `label`, is
    /// running, and returns the sheet and that row.
    ///
    /// `observer` holds the host's functions that the sheet calls while the
    /// work runs, and `log_sink` the sink of the log lines that the work
    /// writes through its scopes (none: the lines go nowhere). The root row's
    /// start is told to the observer before this returns.
    pub fn start(
        root: &Kind,
        label: Option<&str>,
        observer: Observer,
        log_sink: Option<Arc<dyn LogSink>>,
    ) -> (Sheet, Task) {
        let shared = Arc::new(Shared {
            cancelled: AtomicBool::new(false),
            observer,
            log_sink,
            rows: Mutex::new(Vec::new()),
        });
        let row = shared.declare(None, root, label);
        shared.start(&row, None);
        let task = Task::running(Arc::clone(&shared), row);
        (Sheet { shared }, task)
    }

    /// The rows of the sheet, as plain data.
    pub fn snapshot(&self) -> Snapshot {
        let rows: Vec<Arc<Row>> =
            self.shared.rows.lock().unwrap_or_else(PoisonError::into_inner).clone();
        Snapshot {
            cancelled: self.shared.cancelled(),
            rows: rows.iter().map(|row| row.snapshot()).collect(),
        }
    }

    /// Asks the build to stop. The work sees it the next time it asks
    /// ([`Scope::cancelled`](super::Scope::cancelled)), and ends what it was
    /// doing as [`RowState::Failed`].
    pub fn cancel(&self) {
        self.shared.cancelled.store(true, Ordering::Release);
    }

    /// Whether the build was asked to stop.
    pub fn cancelled(&self) -> bool {
        self.shared.cancelled()
    }
}

impl fmt::Debug for Sheet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sheet").field("cancelled", &self.cancelled()).finish_non_exhaustive()
    }
}
