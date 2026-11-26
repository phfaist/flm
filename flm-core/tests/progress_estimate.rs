//! The estimate of the overall percentage of `flm_core::progress`, over
//! snapshots written by hand: the fractions of the rows, a table measured from
//! a finished build, and one table laid over another.

use std::time::Duration;

use flm_core::progress::{
    estimate, measure, Count, Expected, KindDurations, RowId, RowSnapshot, RowState, Snapshot,
    Unit,
};

fn secs(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

/// A row with no count and no times.
fn row(id: u64, parent: Option<u64>, kind: &'static str, state: RowState) -> RowSnapshot {
    RowSnapshot {
        id: RowId(id),
        parent: parent.map(RowId),
        kind: kind.into(),
        title: None,
        label: None,
        state,
        count: None,
        started: None,
        ended: None,
    }
}

fn counted(mut row: RowSnapshot, total: u64, done: u64) -> RowSnapshot {
    row.count = Some(Count { unit: Unit::Items, total, done });
    row
}

fn timed(mut row: RowSnapshot, started: u64, ended: Option<u64>) -> RowSnapshot {
    row.started = Some(secs(started));
    row.ended = ended.map(secs);
    row
}

fn snapshot(rows: Vec<RowSnapshot>) -> Snapshot {
    Snapshot { cancelled: false, rows }
}

fn per_row(seconds: u64) -> KindDurations {
    KindDurations { per_unit: None, per_row: Some(secs(seconds)) }
}

fn assert_close(actual: f64, expected: f64) {
    assert!((actual - expected).abs() < 1e-9, "{actual} is not {expected}");
}

#[test]
fn estimate_follows_the_table_of_fractions() {
    use RowState::*;
    let none = Expected::new();
    let mut table = Expected::new();
    table.insert("t.slow", per_row(10));

    // Ended rows count as done, a pending row as not started; the two
    // children of equal weight average.
    for (state, fraction) in [(Finished, 1.0), (Failed, 1.0), (Pending, 0.0)] {
        let rows = vec![row(0, None, "t.root", Running), row(1, Some(0), "t.a", state)];
        assert_close(estimate(&snapshot(rows), &none, secs(0)), fraction);
    }
    let rows = vec![
        row(0, None, "t.root", Running),
        row(1, Some(0), "t.a", Finished),
        row(2, Some(0), "t.b", Pending),
    ];
    assert_close(estimate(&snapshot(rows), &none, secs(0)), 0.5);

    // A row that is not run is left out of its parent's fraction.
    let rows = vec![
        row(0, None, "t.root", Running),
        row(1, Some(0), "t.a", Finished),
        row(2, Some(0), "t.b", NotRun),
    ];
    assert_close(estimate(&snapshot(rows), &none, secs(0)), 1.0);

    // A running row with a count; a running child of it counts as one unit,
    // with its own fraction, and a finished child is already in `done`.
    let rows = vec![
        counted(row(0, None, "t.root", Running), 4, 1),
        row(1, Some(0), "t.unit", Finished),
        row(2, Some(0), "t.unit", Running),
        row(3, Some(2), "t.part", Finished),
        row(4, Some(2), "t.part", Pending),
    ];
    assert_close(estimate(&snapshot(rows), &none, secs(0)), 1.5 / 4.0);

    // A running row with no count and no children: the time since it started
    // over the duration of its kind, at most 0.95, and 0 when either is
    // unknown.
    let running = timed(row(0, None, "t.slow", Running), 2, None);
    assert_close(estimate(&snapshot(vec![running.clone()]), &table, secs(6)), 0.4);
    assert_close(estimate(&snapshot(vec![running.clone()]), &table, secs(40)), 0.95);
    assert_close(estimate(&snapshot(vec![running]), &none, secs(6)), 0.0);
    let unstarted = row(0, None, "t.slow", Running);
    assert_close(estimate(&snapshot(vec![unstarted]), &table, secs(6)), 0.0);

    // Weights: an ended row weighs the time it took (rule 1), a counted row
    // its total times the duration per unit (rule 2), another row the
    // duration of its kind (rule 3), and a row of an unknown kind the median
    // of its siblings' weights (rule 4).
    let mut table = Expected::new();
    table.insert("t.slow", per_row(10));
    table.insert("t.images", KindDurations { per_unit: Some(secs(5)), per_row: Some(secs(1)) });
    let rows = vec![
        row(0, None, "t.root", Running),
        timed(row(1, Some(0), "t.slow", Finished), 0, Some(30)),
        counted(row(2, Some(0), "t.images", Pending), 4, 0),
        row(3, Some(0), "t.slow", Pending),
        row(4, Some(0), "t.unknown", Pending),
    ];
    // Weights 30, 20, 10 and the median 20; only the first is done.
    assert_close(estimate(&snapshot(rows), &table, secs(30)), 30.0 / 80.0);
}

#[test]
fn a_measured_table_gives_one_half_of_the_same_rows_half_done() {
    use RowState::*;
    let finished = snapshot(vec![
        timed(row(0, None, "t.build", Finished), 0, Some(30)),
        timed(row(1, Some(0), "t.parse", Finished), 0, Some(10)),
        timed(counted(row(2, Some(0), "t.images", Finished), 4, 4), 10, Some(30)),
    ]);
    let expected = measure(&finished);
    let images = expected.get("t.images").unwrap();
    assert_eq!(images.per_unit, Some(secs(5)));
    assert_eq!(images.per_row, Some(secs(20)));

    let half_done = snapshot(vec![
        timed(row(0, None, "t.build", Running), 0, None),
        timed(row(1, Some(0), "t.parse", Finished), 0, Some(10)),
        timed(counted(row(2, Some(0), "t.images", Running), 4, 1), 10, None),
    ]);
    // The parse took 10 s of an expected 30 s, and one image of four is
    // done: (10 + 20 / 4) / 30.
    assert_close(estimate(&half_done, &expected, secs(15)), 0.5);
}

#[test]
fn the_first_table_answers_first() {
    let mut first = Expected::new();
    first.insert("t.a", KindDurations { per_unit: Some(secs(1)), per_row: None });
    let mut base = Expected::new();
    base.insert("t.a", KindDurations { per_unit: Some(secs(2)), per_row: Some(secs(3)) });
    base.insert("t.b", per_row(4));

    let table = first.over(base);
    assert_eq!(
        table.get("t.a"),
        Some(&KindDurations { per_unit: Some(secs(1)), per_row: Some(secs(3)) })
    );
    assert_eq!(table.get("t.b"), Some(&per_row(4)));
    assert_eq!(table.get("t.c"), None);
}
