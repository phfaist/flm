//! The colors of the images drawn in the printed text, which the stage `tty.paint` writes:
//! [`cells`] finds each drawn image's grid of glyphs in the text and gives the colors of its
//! cells.
//!
//! The rule `tty.graphics.image` draws an image shown as a block as a verbatim block of
//! glyph rows inside a block of the role [`ROLE`](image_rule::ROLE); the text output prints
//! the glyphs and records the verbatim block's range twice, as a style mark of the style
//! `verbatim` and as a position mark, which names the node that printed it. The colors are
//! not printed. The painter finds them again:
//!
//! 1. For each style mark of the style `verbatim`, the position mark of the same range names
//!    a node of the lowered trees (the table `flm.lowered.text`). The mark is a drawn image
//!    when that node is a verbatim node whose parent is a block of the role `tty.graphics`;
//!    a verbatim block anywhere else, one an author wrote, is never colored.
//! 2. The grid's lines are the mark's lines: its first line from the mark's start, each
//!    other line from the mark's start column on, since the indentation that precedes a
//!    verbatim block's lines is the same on each of them. The grid has as many columns as
//!    its first line has characters, and as many rows as lines.
//! 3. Each image of the table `flm.graphics.content.text` is rendered at the grid's size
//!    (the module `graphics`), and the first rendering whose glyphs are the printed ones
//!    gives the colors. A grid that no rendering matches stays uncolored; its characters
//!    are printed all the same.

use std::ops::Range;

use flm_ext::ExtError;
use flm_ext::guest::core::data::table::TableValue;
use flm_ext::guest::core::run::StageCx;
use flm_ext::guest::stdlib::backends::lowering::LoweredTrees;
use flm_ext::guest::stdlib::backends::pres;
use image::DynamicImage;
use serde_json::Value;

use crate::profile_tty::graphics::{self, Graphics};
use crate::profile_tty::image_rule;
use crate::profile_tty::paint::{Cell, Style, StyledText};

/// The table of the text backend's lowered trees, whose nodes the position marks name.
pub const LOWERED: &str = "flm.lowered.text";

/// The cells of the images drawn in `styled`, the printed text and its style marks, with
/// their colors; `projection` is the projection of the table `flm.output.text`, whose
/// position marks name the nodes that printed each range. The stage `cx` reads the lowered
/// trees ([`LOWERED`]) and the images' content (the table [`CONTENT`](image_rule::CONTENT)).
pub fn cells(cx: &StageCx, projection: &Value, styled: &StyledText) -> Result<Vec<Cell>, ExtError> {
    let lowered = match cx.get(LOWERED)? {
        None => return Ok(Vec::new()),
        Some(TableValue::Lowered(lowered)) => lowered,
        Some(_) => {
            return Err(ExtError::Value { detail: format!("the table `{LOWERED}` must be read as lowered trees") });
        }
    };
    let positions = position_marks(projection);
    let mut grids = Vec::new();
    for mark in &styled.styles {
        if mark.style != Style::Verbatim {
            continue;
        }
        let mut drawing = false;
        for (bytes, addr) in &positions {
            if *bytes == mark.bytes && is_drawing(&lowered, addr)? {
                drawing = true;
                break;
            }
        }
        if !drawing {
            continue;
        }
        grids.extend(grid(&styled.text, mark.bytes.clone()));
    }
    if grids.is_empty() {
        return Ok(Vec::new());
    }
    // The images that decode: the rule drew no other.
    let mut images: Vec<DynamicImage> = Vec::new();
    for row in image_rule::content_rows(cx.get(image_rule::CONTENT)?)? {
        let (media_type, content) = image_rule::row_content(&row)?;
        images.extend(graphics::decode(&media_type, &content).ok());
    }
    let mut cells = Vec::new();
    for grid in &grids {
        let columns = grid[0].1.chars().count();
        let (Ok(columns), Ok(rows)) = (u32::try_from(columns), u32::try_from(grid.len())) else { continue };
        let printed = |graphics: &Graphics| graphics.glyphs.iter().map(String::as_str).eq(grid.iter().map(|line| line.1));
        let Some(graphics) = images.iter().map(|image| graphics::render(image, columns, rows)).find(printed) else {
            continue;
        };
        for ((start, line), colors) in grid.iter().zip(&graphics.colors) {
            cells.extend(line.char_indices().zip(colors).map(|((at, _), colors)| Cell { at: start + at, colors: *colors }));
        }
    }
    Ok(cells)
}

/// The position marks of `projection`, the projection of `flm.output.text`: each one's byte
/// range and the address of the node that printed it. A mark of another shape is left out.
fn position_marks(projection: &Value) -> Vec<(Range<usize>, &str)> {
    let marks = projection["marks"].as_array().map(Vec::as_slice).unwrap_or_default();
    marks
        .iter()
        .filter_map(|mark| {
            let [start, end] = mark["bytes"].as_array()?.as_slice() else { return None };
            let start = usize::try_from(start.as_u64()?).ok()?;
            let end = usize::try_from(end.as_u64()?).ok()?;
            Some((start..end, mark["addr"].as_str()?))
        })
        .collect()
}

/// Whether the node of `lowered` at `addr` is the drawing of an image: a verbatim node whose
/// parent is a block of the role [`ROLE`](image_rule::ROLE).
fn is_drawing(lowered: &LoweredTrees, addr: &str) -> Result<bool, ExtError> {
    let Some(node) = lowered.node_at(addr)? else { return Ok(false) };
    if node.element_name()? != pres::VERBATIM {
        return Ok(false);
    }
    let Some(parent) = node.parent()? else { return Ok(false) };
    if parent.element_name()? != pres::BLOCK {
        return Ok(false);
    }
    Ok(parent.payload()?.is_some_and(|payload| payload["role"] == image_rule::ROLE))
}

/// The lines of the grid that `text` prints over the byte range `bytes`, each with the byte
/// offset of its first character: the first line from the range's start, each other line
/// from the start's column on. `None` when the lines do not form a grid: a range outside
/// the text or off a character boundary, a line shorter than the column, or lines that are
/// empty or of different lengths in characters.
fn grid(text: &str, bytes: Range<usize>) -> Option<Vec<(usize, &str)>> {
    let slice = text.get(bytes.clone())?;
    let column = bytes.start - text.get(..bytes.start)?.rfind('\n').map_or(0, |newline| newline + 1);
    let mut lines = Vec::new();
    let mut offset = bytes.start;
    for (index, line) in slice.split('\n').enumerate() {
        let skipped = if index == 0 { 0 } else { column };
        lines.push((offset + skipped, line.get(skipped..)?));
        offset += line.len() + 1;
    }
    let columns = lines[0].1.chars().count();
    (columns > 0 && lines.iter().all(|(_, line)| line.chars().count() == columns)).then_some(lines)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_grid_s_lines_start_at_the_mark_s_column() {
        let text = "label\n  > ▀▄\n  > █ \nafter\n";
        let start = text.find('▀').unwrap();
        let end = text.find(" \nafter").unwrap() + 1;
        let lines = grid(text, start..end).unwrap();
        assert_eq!(lines, [(start, "▀▄"), (text.find('█').unwrap(), "█ ")]);
    }

    #[test]
    fn lines_that_do_not_form_a_grid_give_none() {
        let text = "ab\nabc\n";
        // Lines of different lengths.
        assert_eq!(grid(text, 0..6), None);
        // An empty range, a range outside the text, off a character boundary.
        assert_eq!(grid(text, 0..0), None);
        assert_eq!(grid(text, 0..60), None);
        assert_eq!(grid("▀▀", 1..3), None);
        // A line shorter than the column.
        assert_eq!(grid("  ab\n\n", 2..5), None);
    }

    #[test]
    fn the_position_marks_are_read_with_their_ranges_and_addresses() {
        let projection = serde_json::json!({
            "marks": [
                { "addr": "tree#0/3", "bytes": [2, 8] },
                { "addr": "tree#0/4", "bytes": [2] },
                { "bytes": [0, 1] },
            ],
        });
        assert_eq!(position_marks(&projection), [(2..8, "tree#0/3")]);
        assert_eq!(position_marks(&Value::Null), []);
    }
}
