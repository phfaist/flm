//! The layout of a table of the layout engine: computing the column widths
//! and assembling the printed cells into lines.
//!
//! Here a table is only rows of cells (each cell any node of the layout
//! document, with an optional alignment of its own), one [`ColSpec`] per
//! column, an optional number of header rows, and how the table is drawn
//! around and between its cells ([`TableFrame`]); what the table means is the
//! caller's concern. The printer measures every cell twice (its narrowest and
//! its widest printing), [`solve_columns`] turns those measurements into column
//! widths, the printer prints every cell at its column's width, and
//! [`assemble_lines`] assembles the printed cells into output lines.
//!
//! Everything here is integer arithmetic on plain values, with no floating
//! point and no hashing, so the same inputs always give the same widths.

use std::ops::Range;

use super::doc::{ColAlign, TableFrame};
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
/// left for the columns themselves, the table's gutters or frame already
/// subtracted ([`frame_width`]), or `None` when the profile has no width.
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

/// One output line of a table, as [`assemble_lines`] assembles it.
pub(crate) struct AssembledLine {
    /// The line's text, without its line ending and without the enclosing
    /// indentation.
    pub(crate) text: String,
    /// The byte ranges of `text` that the table draws itself (its rule and
    /// its frame), one per maximal run of drawn characters, in order. The
    /// padding around and between the cells is not part of any run.
    pub(crate) borders: Vec<Range<usize>>,
    /// Where the cells' lines stand in `text`, for a line of a row; `None`
    /// for a line the table draws whole.
    pub(crate) row_line: Option<RowLine>,
}

/// Where the cells of one line of a row stand in an [`AssembledLine`].
pub(crate) struct RowLine {
    /// The index of the row, counted from 0.
    pub(crate) row: usize,
    /// The index of the line within the row, counted from 0: the line of
    /// each cell printed on it.
    pub(crate) line: usize,
    /// For each column, the byte offset in the line's text at which the
    /// cell's line starts, after the frame or the gutter and after the
    /// padding before it. Under [`TableFrame::Gutter`], an offset past the
    /// end of the trimmed line is the line's length.
    pub(crate) starts: Vec<usize>,
}

/// The display width the table's own drawing takes on a line beside its
/// columns: the gutters between the columns under [`TableFrame::Gutter`], and
/// under [`TableFrame::Box`] one bar and two spaces of padding per column and
/// the closing bar.
pub(crate) fn frame_width(frame: TableFrame, ncols: usize) -> u32 {
    let ncols = ncols as u32;
    match frame {
        TableFrame::Gutter => GUTTER_WIDTH * ncols.saturating_sub(1),
        TableFrame::Box => 3 * ncols + 1,
    }
}

/// The vertical bar of a [`TableFrame::Box`] frame, before, between and after
/// the cells of a content line.
const BAR: char = '\u{2502}';

/// The horizontal line of a [`TableFrame::Box`] frame.
const HORIZONTAL: char = '\u{2500}';

/// The three corner or junction characters of one horizontal line of a
/// [`TableFrame::Box`] frame: at its left end, where it crosses a vertical
/// bar between two columns, and at its right end.
#[derive(Clone, Copy)]
struct FrameLine {
    left: char,
    join: char,
    right: char,
}

/// The line above the first row.
const TOP: FrameLine = FrameLine {
    left: '\u{250c}',
    join: '\u{252c}',
    right: '\u{2510}',
};

/// The rule under the header rows.
const HEADER_RULE: FrameLine = FrameLine {
    left: '\u{251c}',
    join: '\u{253c}',
    right: '\u{2524}',
};

/// The line below the last row.
const BOTTOM: FrameLine = FrameLine {
    left: '\u{2514}',
    join: '\u{2534}',
    right: '\u{2518}',
};

/// Assembles printed cells into output lines and returns the lines.
///
/// `rows[r][c]` holds the cell at row `r`, column `c`, already printed at
/// `widths[c]`; every row holds one cell per column. Rows are assembled line by
/// line, with every cell at the top of its row: a row is as tall as its
/// tallest cell, and a shorter cell contributes empty lines. Each cell is
/// padded to its column's width in the cell's alignment.
///
/// - Under [`TableFrame::Gutter`], columns are separated by [`GUTTER`],
///   trailing whitespace is removed from every line (the padding, and any
///   trailing whitespace of the last cell's own text), and after the first
///   `header_rows` rows a rule of `-` across every column is inserted. The
///   rule line is the one drawn run of the table.
/// - Under [`TableFrame::Box`], every content line is `│ cell │ cell │`, one
///   space on each side of each padded cell, and it ends with its closing
///   bar; a line `┌─…─┬─…─┐` comes before the first content line, a line
///   `└─…─┴─…─┘` after the last one, and a rule `├─…─┼─…─┤` after the first
///   `header_rows` rows when content lines follow them. Each bar of a content
///   line, and each whole horizontal line, is one drawn run. A table with no
///   content line gets no frame.
///
/// Each line of a row says where each cell's line starts in it
/// ([`AssembledLine::row_line`]), so that the printer can place the marks
/// printed inside the cells.
pub(crate) fn assemble_lines(
    rows: &[Vec<RenderedCell>],
    widths: &[u32],
    header_rows: u8,
    frame: TableFrame,
    measure: &dyn WidthMeasure,
) -> Vec<AssembledLine> {
    let mut lines: Vec<AssembledLine> = Vec::new();
    // Under `Box`, the header rule waits for the next content line, so that
    // it is never drawn directly above the bottom line.
    let mut rule_pending = false;
    for (r, row) in rows.iter().enumerate() {
        let height = row.iter().map(|cell| cell.lines.len()).max().unwrap_or(0);
        for k in 0..height {
            match frame {
                TableFrame::Gutter => lines.push(gutter_line(row, r, k, widths, measure)),
                TableFrame::Box => {
                    if lines.is_empty() {
                        lines.push(frame_line(TOP, widths));
                    }
                    if rule_pending {
                        lines.push(frame_line(HEADER_RULE, widths));
                        rule_pending = false;
                    }
                    lines.push(box_line(row, r, k, widths, measure));
                }
            }
        }
        if usize::from(header_rows) == r + 1 {
            match frame {
                TableFrame::Gutter => lines.push(gutter_rule(widths)),
                TableFrame::Box => rule_pending = true,
            }
        }
    }
    if frame == TableFrame::Box && !lines.is_empty() {
        lines.push(frame_line(BOTTOM, widths));
    }
    lines
}

/// The text of line `k` of `row`'s cell in column `i`; empty below the cell's
/// last line.
fn cell_line(row: &[RenderedCell], i: usize, k: usize) -> &str {
    row[i].lines.get(k).map(|s| s.as_str()).unwrap_or("")
}

/// Line `k` of `row`, the row at index `r`, under [`TableFrame::Gutter`].
fn gutter_line(
    row: &[RenderedCell],
    r: usize,
    k: usize,
    widths: &[u32],
    measure: &dyn WidthMeasure,
) -> AssembledLine {
    let mut text = String::new();
    let mut starts = Vec::with_capacity(widths.len());
    for (i, &width) in widths.iter().enumerate() {
        if i > 0 {
            text.push_str(GUTTER);
        }
        starts.push(pad_into(&mut text, cell_line(row, i, k), width, row[i].align, measure));
    }
    // Engine-introduced padding never survives at a line end.
    text.truncate(text.trim_end().len());
    for start in &mut starts {
        *start = (*start).min(text.len());
    }
    AssembledLine {
        text,
        borders: Vec::new(),
        row_line: Some(RowLine { row: r, line: k, starts }),
    }
}

/// The rule of `-` under the header rows, under [`TableFrame::Gutter`]: one
/// drawn run, the whole line.
fn gutter_rule(widths: &[u32]) -> AssembledLine {
    let mut text = String::new();
    for (i, &width) in widths.iter().enumerate() {
        if i > 0 {
            text.push_str(GUTTER);
        }
        for _ in 0..width {
            text.push('-');
        }
    }
    whole_run(text)
}

/// Line `k` of `row`, the row at index `r`, under [`TableFrame::Box`]: each
/// bar is a run of its own.
fn box_line(
    row: &[RenderedCell],
    r: usize,
    k: usize,
    widths: &[u32],
    measure: &dyn WidthMeasure,
) -> AssembledLine {
    let mut text = String::new();
    let mut borders = Vec::with_capacity(widths.len() + 1);
    let mut starts = Vec::with_capacity(widths.len());
    push_run(&mut text, &mut borders, BAR);
    for (i, &width) in widths.iter().enumerate() {
        text.push(' ');
        starts.push(pad_into(&mut text, cell_line(row, i, k), width, row[i].align, measure));
        text.push(' ');
        push_run(&mut text, &mut borders, BAR);
    }
    AssembledLine {
        text,
        borders,
        row_line: Some(RowLine { row: r, line: k, starts }),
    }
}

/// A horizontal line of a [`TableFrame::Box`] frame, its horizontal stretch
/// over each column as wide as the column and its two spaces of padding: one
/// drawn run, the whole line.
fn frame_line(chars: FrameLine, widths: &[u32]) -> AssembledLine {
    let mut text = String::new();
    text.push(chars.left);
    for (i, &width) in widths.iter().enumerate() {
        if i > 0 {
            text.push(chars.join);
        }
        for _ in 0..width + 2 {
            text.push(HORIZONTAL);
        }
    }
    text.push(chars.right);
    whole_run(text)
}

/// A line drawn whole by the table: one run over all of it, or none for an
/// empty line.
fn whole_run(text: String) -> AssembledLine {
    let mut borders = Vec::new();
    if !text.is_empty() {
        borders.push(0..text.len());
    }
    AssembledLine {
        text,
        borders,
        row_line: None,
    }
}

/// Appends the drawn character `c` to `text`, and its byte range to `borders`.
fn push_run(text: &mut String, borders: &mut Vec<Range<usize>>, c: char) {
    let start = text.len();
    text.push(c);
    borders.push(start..text.len());
}

/// Appends `cell` to `out`, padded with spaces to `width` columns as `align`
/// says, and returns the byte offset in `out` at which `cell` starts. A cell
/// wider than `width` is appended without padding.
fn pad_into(
    out: &mut String,
    cell: &str,
    width: u32,
    align: ColAlign,
    measure: &dyn WidthMeasure,
) -> usize {
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
    let start = out.len();
    out.push_str(cell);
    for _ in 0..right {
        out.push(' ');
    }
    start
}

#[cfg(test)]
mod tests {
    use super::super::width::{UnicodeMeasure, WidthMeasure};
    use super::{solve_columns, BAR, BOTTOM, HEADER_RULE, HORIZONTAL, TOP};

    /// Every character of the box frame is one display column wide for the
    /// engine's measure: the solver subtracts one column per bar, and the
    /// lines of a framed table align only if each frame character is one
    /// column.
    #[test]
    fn frame_characters_are_one_column_wide() {
        let mut chars = vec![BAR, HORIZONTAL];
        for line in [TOP, HEADER_RULE, BOTTOM] {
            chars.extend([line.left, line.join, line.right]);
        }
        assert_eq!(chars.len(), 11);
        for c in chars {
            assert_eq!(UnicodeMeasure.str_width(&c.to_string()), 1, "{c:?}");
        }
    }

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
