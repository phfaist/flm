//! The progress sheet of `flm_core::progress`: the rows, their states and
//! counts, the handles with no row, several threads writing one sheet, and the
//! host's observer.

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

use flm_core::progress::{
    Kind, LogLevel, LogRecord, LogSink, Observer, RowId, RowSnapshot, RowState, Scope, Sheet,
    Snapshot, Unit,
};

const ROOT: Kind = Kind::new("test.build");
const WORK: Kind = Kind::titled("test.work", "Working");
const PART: Kind = Kind::new("test.part");

/// The row of `snapshot` with the id `id`.
fn row(snapshot: &Snapshot, id: u64) -> &RowSnapshot {
    &snapshot.rows[id as usize]
}

/// An observer whose two row functions return the times 1, 2, 3, … in the
/// order of their calls.
fn stamping() -> Observer {
    let clock = Arc::new(AtomicU64::new(0));
    let tick = move |_: &RowSnapshot| {
        Some(Duration::from_secs(clock.fetch_add(1, Ordering::Relaxed) + 1))
    };
    Observer {
        row_started: Some(Box::new(tick.clone())),
        row_ended: Some(Box::new(tick)),
        checkpoint: None,
    }
}

#[test]
fn rows_keep_their_total_move_forward_and_end_when_dropped() {
    let (sheet, root) = Sheet::start(&ROOT, None, stamping(), None);
    let scope = root.scope();

    let counted = scope.child(&WORK, Some("a.flm")).start_counted(Unit::Items, 3);
    counted.advance(2);
    counted.advance(5);
    let failed = counted.scope().child(&PART, None).start();
    let not_run = counted.scope().child(&PART, None);
    drop(failed);
    drop(not_run);
    counted.finish();
    root.finish();

    let snapshot = sheet.snapshot();
    let ids: Vec<RowId> = snapshot.rows.iter().map(|row| row.id).collect();
    assert_eq!(ids, [RowId(0), RowId(1), RowId(2), RowId(3)]);
    let parents: Vec<Option<RowId>> = snapshot.rows.iter().map(|row| row.parent).collect();
    assert_eq!(parents, [None, Some(RowId(0)), Some(RowId(1)), Some(RowId(1))]);

    let work = row(&snapshot, 1);
    assert_eq!(work.kind, "test.work");
    assert_eq!(work.title.as_deref(), Some("Working"));
    assert_eq!(work.label.as_deref(), Some("a.flm"));
    let count = work.count.expect("the row was started with a count");
    assert_eq!((count.unit, count.total, count.done), (Unit::Items, 3, 3));

    let states: Vec<RowState> = snapshot.rows.iter().map(|row| row.state).collect();
    assert_eq!(
        states,
        [RowState::Finished, RowState::Finished, RowState::Failed, RowState::NotRun]
    );

    // The observer returned 1 for the root's start, then 2 and 3 for the
    // starts of the two rows that ran, 4 when the failed row ended, 5 for the
    // counted row and 6 for the root.
    let times = |id| (row(&snapshot, id).started, row(&snapshot, id).ended);
    let secs = |n| Some(Duration::from_secs(n));
    assert_eq!(times(0), (secs(1), secs(6)));
    assert_eq!(times(1), (secs(2), secs(5)));
    assert_eq!(times(2), (secs(3), secs(4)));
    assert_eq!(times(3), (None, None));

    // A state only moves forward: a row declared under a row that has ended
    // has no row behind it, and nothing of the sheet changes.
    let late = scope.child(&PART, None).start();
    late.finish();
    assert_eq!(sheet.snapshot(), snapshot);
}

/// A record that [`Kept`] keeps, as (source, row, label, text).
type KeptRecord = (String, Option<RowId>, Option<String>, String);

/// A log sink that keeps every record it receives.
#[derive(Default)]
struct Kept(Mutex<Vec<KeptRecord>>);

impl LogSink for Kept {
    fn enabled(&self, _level: LogLevel) -> bool {
        true
    }

    fn log(&self, record: &LogRecord<'_>) {
        self.0.lock().unwrap().push((
            record.source.to_string(),
            record.row,
            record.label.map(str::to_string),
            record.text.to_string(),
        ));
    }
}

#[test]
fn handles_with_no_row_add_no_row_and_still_answer() {
    // No sheet: nothing is declared, nothing is cancelled, and the closure
    // that computes a total is not called.
    let none = Scope::none();
    let counted = none
        .child(&WORK, Some("a.flm"))
        .start_counted_with(Unit::Items, || panic!("no row is behind the handle"));
    counted.advance(1);
    assert!(!counted.scope().cancelled());
    assert!(!none.log_enabled(LogLevel::Error));
    none.log(LogLevel::Error, "goes nowhere");
    counted.finish();

    // Past a depth limit: the limited row exists, the rows below it do not.
    let kept = Arc::new(Kept::default());
    let (sheet, root) = Sheet::start(&ROOT, None, Observer::default(), Some(kept.clone()));
    let limited = root.scope().child(&WORK, Some("b.flm")).with_depth_limit(0).start();
    let past = limited.scope().child(&PART, Some("unseen")).start_counted(Unit::Bytes, 10);
    past.advance(4);
    let deeper = past.scope().child(&PART, None).start();
    assert!(!deeper.scope().cancelled());
    sheet.cancel();
    assert!(deeper.scope().cancelled());
    assert!(deeper.scope().log_enabled(LogLevel::Info));
    deeper.scope().log(LogLevel::Info, "a line");
    deeper.finish();
    past.finish();
    limited.finish();

    // A depth limit of one keeps one level below the limited row.
    let one = root.scope().with_depth_limit(1);
    let kept_row = one.child(&WORK, None).start();
    kept_row.scope().child(&PART, None).start().finish();
    kept_row.finish();
    root.finish();

    let snapshot = sheet.snapshot();
    let kinds: Vec<&str> = snapshot.rows.iter().map(|row| &*row.kind).collect();
    assert_eq!(kinds, ["test.build", "test.work", "test.work"]);
    assert!(snapshot.cancelled);
    assert_eq!(
        *kept.0.lock().unwrap(),
        [(
            "test.work".to_string(),
            Some(RowId(1)),
            Some("b.flm".to_string()),
            "a line".to_string()
        )]
    );
}

#[test]
fn several_threads_advance_one_count_and_declare_rows_under_one_scope() {
    const THREADS: u64 = 8;
    const EACH: u64 = 100;
    let (sheet, root) = Sheet::start(&ROOT, None, Observer::default(), None);
    let counted = root.scope().child(&WORK, None).start_counted(Unit::Items, THREADS * EACH);
    let scope = counted.scope();

    thread::scope(|threads| {
        for _ in 0..THREADS {
            threads.spawn(|| {
                for _ in 0..EACH {
                    counted.advance(1);
                    scope.child(&PART, None).start().finish();
                }
            });
        }
    });
    counted.finish();
    root.finish();

    let snapshot = sheet.snapshot();
    assert_eq!(snapshot.rows[1].count.map(|count| count.done), Some(THREADS * EACH));
    let parts = snapshot.rows.iter().filter(|row| row.kind == "test.part");
    assert_eq!(parts.clone().count() as u64, THREADS * EACH);
    assert!(parts.clone().all(|row| row.parent == Some(RowId(1))));
    assert!(parts.clone().all(|row| row.state == RowState::Finished));
    let ids: Vec<u64> = snapshot.rows.iter().map(|row| row.id.0).collect();
    assert_eq!(ids, (0..ids.len() as u64).collect::<Vec<_>>());
}

/// What an observer saw: the rows it was told of, as (id, state), and the
/// number of checkpoints.
#[derive(Default)]
struct Seen {
    started: Mutex<Vec<(RowId, RowState)>>,
    ended: Mutex<Vec<(RowId, RowState)>>,
    checkpoints: AtomicUsize,
    snapshots: AtomicUsize,
}

#[test]
fn the_observer_is_called_when_a_row_starts_ends_and_at_each_question() {
    let seen = Arc::new(Seen::default());
    // The sheet, reachable from inside the observer's functions once started.
    let sheet_cell: Arc<OnceLock<Sheet>> = Arc::new(OnceLock::new());
    let snap = {
        let (seen, cell) = (seen.clone(), sheet_cell.clone());
        move || {
            if let Some(sheet) = cell.get() {
                let _ = sheet.snapshot();
                seen.snapshots.fetch_add(1, Ordering::Relaxed);
            }
        }
    };
    let observer = Observer {
        row_started: Some(Box::new({
            let (seen, snap) = (seen.clone(), snap.clone());
            move |row| {
                seen.started.lock().unwrap().push((row.id, row.state));
                snap();
                None
            }
        })),
        row_ended: Some(Box::new({
            let (seen, snap) = (seen.clone(), snap.clone());
            move |row| {
                seen.ended.lock().unwrap().push((row.id, row.state));
                snap();
                None
            }
        })),
        checkpoint: Some(Box::new({
            let (seen, cell, snap) = (seen.clone(), sheet_cell.clone(), snap.clone());
            move || {
                // The second question of the work cancels the build.
                if seen.checkpoints.fetch_add(1, Ordering::Relaxed) == 1 {
                    cell.get().expect("the sheet is set before the work asks").cancel();
                }
                snap();
            }
        })),
    };
    let (sheet, root) = Sheet::start(&ROOT, None, observer, None);
    sheet_cell.set(sheet.clone()).expect("the cell is empty");
    let scope = root.scope();

    let counted = scope.child(&WORK, None).start_counted(Unit::Items, 2);
    counted.advance(1);
    counted.advance(1);
    assert!(!scope.cancelled());
    assert!(scope.cancelled(), "a cancellation asked for in the checkpoint is seen at once");
    counted.finish();
    drop(scope.child(&PART, None));
    root.finish();

    assert_eq!(seen.checkpoints.load(Ordering::Relaxed), 2);
    assert_eq!(
        *seen.started.lock().unwrap(),
        [(RowId(0), RowState::Running), (RowId(1), RowState::Running)],
        "the dropped row never started"
    );
    assert_eq!(
        *seen.ended.lock().unwrap(),
        [(RowId(1), RowState::Finished), (RowId(0), RowState::Finished)]
    );
    // A snapshot was taken inside each call but the root's start.
    assert_eq!(seen.snapshots.load(Ordering::Relaxed), 5);

    // A function that is not set is not called while the two others are.
    let checkpoints = Arc::new(AtomicUsize::new(0));
    let observer = Observer {
        row_started: None,
        row_ended: Some(Box::new(|_| None)),
        checkpoint: Some(Box::new({
            let checkpoints = checkpoints.clone();
            move || {
                checkpoints.fetch_add(1, Ordering::Relaxed);
            }
        })),
    };
    let (sheet, root) = Sheet::start(&ROOT, None, observer, None);
    let task = root.scope().child(&WORK, None).start();
    assert!(!task.scope().cancelled());
    task.finish();
    root.finish();
    assert_eq!(checkpoints.load(Ordering::Relaxed), 1);
    assert!(sheet.snapshot().rows.iter().all(|row| row.started.is_none() && row.ended.is_none()));
}
