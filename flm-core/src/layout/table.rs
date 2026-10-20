//! The layout of a table of the layout engine: computing the column widths
//! and assembling the printed cells into lines.
//!
//! Here a table is only rows of cells (each cell any node of the layout
//! document, with an optional alignment of its own), one [`ColSpec`] per
//! column, and an optional number of header rows; what the table means is the
//! caller's concern. The printer measures every cell twice (its narrowest and
//! its widest printing), [`solve_columns`] turns those measurements into column
//! widths, the printer prints every cell at its column's width, and
//! [`assemble_lines`] assembles the printed cells into output lines.
//!
//! Everything here is integer arithmetic on plain values, with no floating
//! point and no hashing, so the same inputs always give the same widths.

use super::doc::ColAlign;
#[cfg(doc)]
use super::doc::ColSpec;
use super::width::WidthMeasure;

/// The gutter written between two adjacent columns.
pub(crate) const GUTTER: &str = "  ";

/// The display width of [`GUTTER`]: its length in bytes, since it is made of
/// ASCII spaces.
pub(crate) const GUTTER_WIDTH: u32 = GUTTER.len() as u32;

/// Computes the column widths from the columns' minimum and maximum content
/// widths, and returns one width per column.
///
/// `mins[i]` is the narrowest width at which column `i` can be printed (the
/// widest part of its cells that cannot be broken), `maxs[i]` the width at
/// which nothing in it needs to break (its widest cell printed on unbroken
/// lines); the caller ensures `mins[i] <= maxs[i]`. `available` is the width
/// left for the columns themselves, gutters already subtracted, or `None` when
/// the profile has no width.
///
/// - No width, or the maxima fit: every column gets its maximum.
/// - Otherwise every column starts at its minimum, and the remaining width
///   `available − Σmin` is shared out in proportion to each column's room to
///   grow, `max − min`, in whole columns: each column first gets the whole
///   part of its share, then the columns with the largest fractional parts get
///   one more column each until the width is used up (on equal fractional
///   parts, the column with the lower index first), and no column grows past
///   its maximum.
/// - If even the minima do not fit, every column gets its minimum and the
///   lines are longer than the width, because atoms are never split.
pub(crate) fn solve_columns(mins: &[u32], maxs: &[u32], available: Option<u32>) -> Vec<u32> {
    let Some(available) = available else {
        return maxs.to_vec();
    };
    let available = u64::from(available);
    let sum_max: u64 = maxs.iter().map(|&m| u64::from(m)).sum();
    if sum_max <= available {
        return maxs.to_vec();
    }
    let mut widths = mins.to_vec();
    let sum_min: u64 = mins.iter().map(|&m| u64::from(m)).sum();
    if sum_min >= available {
        return widths;
    }
    let surplus = available - sum_min;
    let spans: Vec<u64> = mins
        .iter()
        .zip(maxs)
        .map(|(&min, &max)| u64::from(max - min))
        .collect();
    let total: u64 = spans.iter().sum();
    if total == 0 {
        return widths;
    }
    let mut remainders: Vec<(u64, usize)> = Vec::with_capacity(spans.len());
    let mut assigned = 0u64;
    for (i, (&span, width)) in spans.iter().zip(widths.iter_mut()).enumerate() {
        let num = surplus * span;
        let base = num / total;
        *width += base as u32;
        assigned += base;
        remainders.push((num - base * total, i));
    }
    // Largest remainder first, ties by lower column index: deterministic.
    remainders.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut leftover = surplus - assigned;
    for &(_, i) in &remainders {
        if leftover == 0 {
            break;
        }
        if widths[i] < maxs[i] {
            widths[i] += 1;
            leftover -= 1;
        }
    }
    widths
}

/// One cell printed at its column's width: its lines, and the alignment in
/// which it is padded (its own, or its column's where it has none).
pub(crate) struct RenderedCell {
    /// The printed lines, without line endings; empty for an empty cell.
    pub(crate) lines: Vec<String>,
    /// The alignment the lines are padded in.
    pub(crate) align: ColAlign,
}

/// Assembles printed cells into output lines and returns the lines.
///
/// `rows[r][c]` holds the cell at row `r`, column `c`, already printed at
/// `widths[c]`; every row holds one cell per column. Rows are assembled line by
/// line, with every cell at the top of its row: a row is as tall as its
/// tallest cell, and a shorter cell contributes empty lines. Each cell is
/// padded to its column's width in the cell's alignment, columns are separated
/// by [`GUTTER`], and trailing whitespace is removed from every line (the
/// padding, and any trailing whitespace of the last cell's own text). After the first `header_rows` rows, a rule of `-` across every column
/// is inserted.
pub(crate) fn assemble_lines(
    rows: &[Vec<RenderedCell>],
    widths: &[u32],
    header_rows: u8,
    measure: &dyn WidthMeasure,
) -> Vec<String> {
    let ncols = widths.len();
    let mut lines: Vec<String> = Vec::new();
    for (r, row) in rows.iter().enumerate() {
        let height = row.iter().map(|cell| cell.lines.len()).max().unwrap_or(0);
        for k in 0..height {
            let mut line = String::new();
            for i in 0..ncols {
                if i > 0 {
                    line.push_str(GUTTER);
                }
                let cell = &row[i];
                let text = cell.lines.get(k).map(|s| s.as_str()).unwrap_or("");
                pad_into(&mut line, text, widths[i], cell.align, measure);
            }
            // Engine-introduced padding never survives at a line end.
            line.truncate(line.trim_end().len());
            lines.push(line);
        }
        if usize::from(header_rows) == r + 1 {
            let mut rule = String::new();
            for (i, &width) in widths.iter().enumerate() {
                if i > 0 {
                    rule.push_str(GUTTER);
                }
                for _ in 0..width {
                    rule.push('-');
                }
            }
            lines.push(rule);
        }
    }
    lines
}

/// Appends `cell` to `out`, padded with spaces to `width` columns as `align`
/// says. A cell wider than `width` is appended without padding.
fn pad_into(out: &mut String, cell: &str, width: u32, align: ColAlign, measure: &dyn WidthMeasure) {
    let w = measure.str_width(cell);
    let pad = width.saturating_sub(w);
    let (left, right) = match align {
        ColAlign::Left => (0, pad),
        ColAlign::Right => (pad, 0),
        ColAlign::Center => (pad / 2, pad - pad / 2),
    };
    for _ in 0..left {
        out.push(' ');
    }
    out.push_str(cell);
    for _ in 0..right {
        out.push(' ');
    }
}

#[cfg(test)]
mod tests {
    use super::solve_columns;

    #[test]
    fn unbounded_or_fitting_maxima_give_every_column_its_max() {
        assert_eq!(solve_columns(&[1, 2], &[5, 7], None), vec![5, 7]);
        assert_eq!(solve_columns(&[1, 2], &[5, 7], Some(12)), vec![5, 7]);
        assert_eq!(solve_columns(&[1, 2], &[5, 7], Some(100)), vec![5, 7]);
    }

    #[test]
    fn overflowing_minima_give_every_column_its_min() {
        assert_eq!(solve_columns(&[4, 6], &[8, 9], Some(10)), vec![4, 6]);
        assert_eq!(solve_columns(&[4, 6], &[8, 9], Some(3)), vec![4, 6]);
    }

    /// Surplus 20 over spans [13, 21, 0] (total 34) gives whole parts
    /// [7, 12, 0] with remainders [22, 12, 0], and the one leftover column of
    /// width goes to the largest remainder.
    #[test]
    fn surplus_is_distributed_by_largest_remainder() {
        assert_eq!(
            solve_columns(&[5, 9, 6], &[18, 30, 6], Some(40)),
            vec![13, 21, 6]
        );
    }

    /// Equal remainders are broken by the lower column index.
    #[test]
    fn remainder_ties_go_to_the_lower_column_index() {
        // spans [3, 3], surplus 1: whole parts 0, remainders equal (3 and 3).
        assert_eq!(solve_columns(&[2, 2], &[5, 5], Some(5)), vec![3, 2]);
    }
}
