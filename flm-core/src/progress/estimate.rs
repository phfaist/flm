//! The estimate of the overall percentage: the table of expected durations,
//! [`measure`] and [`estimate`].

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use super::kind::SharedText;
use super::sheet::{RowId, RowSnapshot, RowState, Snapshot};

/// The expected durations of the rows of one kind, an entry of an
/// [`Expected`] table.
///
/// In the serialized form, each duration is a number of seconds, and a
/// duration that is not known is left out.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct KindDurations {
    /// The duration of one unit of the count, for the rows of the kind that
    /// have a count.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "seconds")]
    pub per_unit: Option<Duration>,
    /// The duration of a whole row of the kind.
    #[serde(default, skip_serializing_if = "Option::is_none", with = "seconds")]
    pub per_row: Option<Duration>,
}

/// The table of expected durations, by the name of a row's [`Kind`]: what
/// [`estimate`] weighs the rows of a sheet with.
///
/// A host measures the table of a build with [`measure`], stores it, and lays
/// it over a table of defaults with [`Expected::over`] for the next build. A
/// table serializes as a map from the name of a kind to its
/// [`KindDurations`]:
///
/// ```
/// use std::time::Duration;
/// use flm_core::progress::{Expected, KindDurations};
///
/// let mut defaults = Expected::new();
/// defaults.insert(
///     "flm.graphics.convert",
///     KindDurations { per_unit: None, per_row: Some(Duration::from_millis(1500)) },
/// );
/// let measured = Expected::new();
///
/// let expected = measured.over(defaults);
/// let durations = expected.get("flm.graphics.convert").unwrap();
/// assert_eq!(durations.per_row, Some(Duration::from_millis(1500)));
/// ```
///
/// [`Kind`]: super::Kind
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct Expected {
    kinds: BTreeMap<SharedText, KindDurations>,
}

impl Expected {
    /// An empty table.
    pub fn new() -> Expected {
        Expected::default()
    }

    /// Sets the durations of the kind named `kind`, replacing those it had.
    pub fn insert(&mut self, kind: impl Into<SharedText>, durations: KindDurations) {
        self.kinds.insert(kind.into(), durations);
    }

    /// The durations of the kind named `kind`, if the table has any.
    pub fn get(&self, kind: &str) -> Option<&KindDurations> {
        self.kinds.get(kind)
    }

    /// The kinds of the table and their durations, in the order of their
    /// names.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &KindDurations)> {
        self.kinds.iter().map(|(kind, durations)| (kind.as_str(), durations))
    }

    /// The table that answers from `self` first, and from `base` for what
    /// `self` lacks: a kind that `self` does not hold, or one of the two
    /// durations of a kind that `self` does not know.
    pub fn over(self, base: Expected) -> Expected {
        let mut kinds = base.kinds;
        for (kind, durations) in self.kinds {
            let entry = kinds.entry(kind).or_default();
            entry.per_unit = durations.per_unit.or(entry.per_unit);
            entry.per_row = durations.per_row.or(entry.per_row);
        }
        Expected { kinds }
    }
}

/// Measures the table of expected durations from the last snapshot of a build
/// whose rows carry their times.
///
/// For each kind, over the rows of that kind that ended as
/// [`RowState::Finished`] with both times: the duration of a whole row is the
/// mean of their durations, and the duration of one unit is the sum of their
/// durations divided by the sum of their totals, over those that have a count
/// with a total above zero. A kind with no such row is not in the table.
///
/// The function keeps no memory and reads no clock.
pub fn measure(snapshot: &Snapshot) -> Expected {
    #[derive(Default)]
    struct Sums {
        rows: u32,
        seconds: f64,
        counted_seconds: f64,
        units: f64,
    }

    let mut sums: BTreeMap<&SharedText, Sums> = BTreeMap::new();
    for row in &snapshot.rows {
        let Some(took) = took(row).filter(|_| row.state == RowState::Finished) else {
            continue;
        };
        let sums = sums.entry(&row.kind).or_default();
        sums.rows += 1;
        sums.seconds += took;
        if let Some(count) = row.count.filter(|count| count.total > 0) {
            sums.counted_seconds += took;
            sums.units += count.total as f64;
        }
    }

    let kinds = sums
        .into_iter()
        .map(|(kind, sums)| {
            let per_row = duration(sums.seconds / f64::from(sums.rows));
            let per_unit =
                if sums.units > 0.0 { duration(sums.counted_seconds / sums.units) } else { None };
            (kind.clone(), KindDurations { per_unit, per_row })
        })
        .collect();
    Expected { kinds }
}

/// Estimates the fraction done of a build, from 0 to 1: the fraction of the
/// root row of `snapshot`, with its rows weighed by `expected`.
///
/// `now` is the host's time, from the same origin as the times the host's
/// observer returned; the caller reads it. The function keeps no memory and
/// reads no clock.
///
/// The fraction of a row:
///
/// | Row | Fraction |
/// |---|---|
/// | `Finished` or `Failed` | 1 |
/// | `Pending` | 0 |
/// | `NotRun` | left out of its parent's fraction |
/// | `Running`, with a count | (done + the fractions of its running children) / total, at most 1 |
/// | `Running`, no count, with children | the weighted average of its children's fractions |
/// | `Running`, no count, no children | the time since it started / the duration of a whole row of its kind, at most 0.95; 0 when either is unknown |
///
/// A child of a counted row stands for one of the counted units. A count
/// whose total is zero counts as no count.
///
/// The weight of a row, where its parent averages its children, is the first
/// of:
///
/// 1. for a row that has ended with both times, the time it took;
/// 2. for a row with a count, whose kind has a duration per unit, that
///    duration times its total;
/// 3. for a row whose kind has a duration for a whole row, that duration;
/// 4. otherwise the median of the weights that the first three rules gave
///    among its siblings, and 1 when they gave none.
///
/// A parent's own weight comes from these rules applied to it, never from the
/// sum of its children, so a row declared late changes the fraction of its
/// parent and the share of no other row.
///
/// Two consequences for a display:
///
/// - **The fraction can decrease** from one call to the next: the total of a
///   row, known when it starts, replaces the duration of a whole row in its
///   weight, and a row declared late lowers the fraction of its parent. A row
///   declared in advance and then not run moves the fraction forward instead.
///   A display keeps the highest value it has shown, and shows the whole only
///   when the root row has finished.
/// - **Under batches that run in parallel**, the fraction is representative
///   of the work done, not a measure of the time left.
pub fn estimate(snapshot: &Snapshot, expected: &Expected, now: Duration) -> f64 {
    let Some(root) = snapshot.rows.iter().position(|row| row.parent.is_none()) else {
        return 0.0;
    };
    let mut index: HashMap<RowId, usize> = HashMap::with_capacity(snapshot.rows.len());
    for (at, row) in snapshot.rows.iter().enumerate() {
        index.entry(row.id).or_insert(at);
    }
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); snapshot.rows.len()];
    for (at, row) in snapshot.rows.iter().enumerate() {
        if row.state == RowState::NotRun || index.get(&row.id) != Some(&at) {
            continue;
        }
        if let Some(&parent) = row.parent.and_then(|parent| index.get(&parent)) {
            children[parent].push(at);
        }
    }
    let tree = Tree { rows: &snapshot.rows, children: &children, expected, now };
    tree.fraction(root).clamp(0.0, 1.0)
}

/// The rows of a snapshot with the children of each, for [`estimate`].
struct Tree<'a> {
    rows: &'a [RowSnapshot],
    /// For each row, the positions of its children that are not `NotRun`.
    children: &'a [Vec<usize>],
    expected: &'a Expected,
    now: Duration,
}

impl Tree<'_> {
    fn fraction(&self, at: usize) -> f64 {
        let row = &self.rows[at];
        match row.state {
            RowState::Finished | RowState::Failed => return 1.0,
            RowState::Pending | RowState::NotRun => return 0.0,
            RowState::Running => {}
        }
        let children = &self.children[at];
        if let Some(count) = row.count.filter(|count| count.total > 0) {
            let running: f64 = children
                .iter()
                .filter(|&&child| self.rows[child].state == RowState::Running)
                .map(|&child| self.fraction(child))
                .sum();
            return ((count.done as f64 + running) / count.total as f64).min(1.0);
        }
        if !children.is_empty() {
            return self.average(children);
        }
        let (Some(started), Some(per_row)) = (row.started, self.durations(row).per_row) else {
            return 0.0;
        };
        if per_row.is_zero() {
            return RUNNING_AT_MOST;
        }
        let elapsed = self.now.saturating_sub(started);
        (elapsed.as_secs_f64() / per_row.as_secs_f64()).min(RUNNING_AT_MOST)
    }

    /// The average of the fractions of `children`, weighted by their weights.
    fn average(&self, children: &[usize]) -> f64 {
        let known: Vec<Option<f64>> = children.iter().map(|&child| self.weight(child)).collect();
        let fallback = median(known.iter().flatten().copied().collect()).unwrap_or(1.0);
        let mut weighted = 0.0;
        let mut weights = 0.0;
        for (&child, weight) in children.iter().zip(&known) {
            let weight = weight.unwrap_or(fallback);
            weighted += weight * self.fraction(child);
            weights += weight;
        }
        if weights > 0.0 {
            weighted / weights
        } else {
            let sum: f64 = children.iter().map(|&child| self.fraction(child)).sum();
            sum / children.len() as f64
        }
    }

    /// The weight of a row by the first three rules, in seconds; `None` when
    /// none of them gives one.
    fn weight(&self, at: usize) -> Option<f64> {
        let row = &self.rows[at];
        if matches!(row.state, RowState::Finished | RowState::Failed) {
            if let Some(took) = took(row) {
                return Some(took);
            }
        }
        let durations = self.durations(row);
        if let (Some(count), Some(per_unit)) =
            (row.count.filter(|count| count.total > 0), durations.per_unit)
        {
            return Some(per_unit.as_secs_f64() * count.total as f64);
        }
        durations.per_row.map(|per_row| per_row.as_secs_f64())
    }

    fn durations(&self, row: &RowSnapshot) -> KindDurations {
        self.expected.get(&row.kind).copied().unwrap_or_default()
    }
}

/// The highest fraction of a running row that the time since it started
/// gives.
const RUNNING_AT_MOST: f64 = 0.95;

/// The time a row took, in seconds, when it carries both times.
fn took(row: &RowSnapshot) -> Option<f64> {
    let (started, ended) = (row.started?, row.ended?);
    Some(ended.checked_sub(started)?.as_secs_f64())
}

/// The median of `values`; the mean of the two middle values when their number
/// is even.
fn median(mut values: Vec<f64>) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    values.sort_by(f64::total_cmp);
    let middle = values.len() / 2;
    Some(if values.len() % 2 == 0 {
        (values[middle - 1] + values[middle]) / 2.0
    } else {
        values[middle]
    })
}

/// A duration of `seconds`, when it is one.
fn duration(seconds: f64) -> Option<Duration> {
    Duration::try_from_secs_f64(seconds).ok()
}

/// The serialized form of an optional duration: a number of seconds.
mod seconds {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S: Serializer>(
        value: &Option<Duration>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match value {
            Some(duration) => serializer.serialize_f64(duration.as_secs_f64()),
            None => serializer.serialize_none(),
        }
    }

    pub(super) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<Duration>, D::Error> {
        Option::<f64>::deserialize(deserializer)?
            .map(|seconds| {
                Duration::try_from_secs_f64(seconds).map_err(|_| {
                    serde::de::Error::custom(format!(
                        "Invalid duration ‘{seconds}’ (a duration is a number of seconds, zero or above)"
                    ))
                })
            })
            .transpose()
    }
}
