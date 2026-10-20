//! The reader of a table of cells: [`read_grid`] reads the body of a `cells`
//! environment into the table's grid, at lowering.
//!
//! Every rule that lowers a table calls [`read_grid`], and nothing stores
//! its result. It reads the table in two passes. The first pass collects the
//! names of the rows and columns, then reads the instructions in order,
//! keeping a current row and column, and places every cell whose rectangle
//! is closed, which fixes the table's size. The second pass gives each cell
//! spanning "to the end" (`\merge{2-}`) the end of the table as finally
//! built. Then the cells are checked for overlaps, and the later of two
//! overlapping cells is left out.
//!
//! # The result
//!
//! A [`Grid`]: the table's model, which is the payload of the
//! `flm.pres.cells` node the rule stages ([`CellsPayload`]); the content of
//! each cell, in the order of the model's entries ([`CellContent`]); every
//! problem found, each with the dropped mark for what it left out, where it
//! left something out ([`Problem`]); and the cell styles the table's tokens
//! refer to.

// The placement algorithm follows the Python implementation of FLM step for
// step: the current row and column, the order in which the cells are
// placed, and the four parsers of an index, a range, a placement and a
// mapping.

use std::collections::BTreeMap;

use techy::core::node::{NodeRef, NodeSlice};
use techy::error::{Diagnostic, DiagnosticInfo};

use crate::core::lowering::{LowerError, chars_at_lowering};
use crate::lang::structural;
use crate::lang::{Flm, FlmDiagnostic, FlmGroupType};
use crate::model::element::element_of;
use crate::model::names::Name;
use crate::stdcore::backends::lowering::is_blank;
use crate::stdcore::backends::pres::{
    CellSpec, CellsPayload, DroppedPayload, MAX_CELLS_POSITIONS,
};

use super::diagnostics::{
    BadNames, BadPlacement, BadStyleValue, BadStyles, ContentOutsideCell, Empty, Overlap, TooLarge,
    UnknownStyle,
};
use super::styles::{Effect, StyleForm, Styles};
use super::{
    CELL, COLNAMES, COLUMN, CONTENT_ARG, DATA, DATA_ARG, MAPPING_ARG, MERGE, NAMES_ARG, PLACEMENT_ARG,
    RANGE_ARG, ROW, ROWNAMES, STYLES_ARG,
};

/// The content of one cell of a [`Grid`]: the nodes of the document that
/// hold it.
#[derive(Debug, Clone, Copy)]
pub enum CellContent<'t> {
    /// The content argument of a `\cell`, laid out as blocks.
    Blocks(NodeSlice<'t, Flm>),
    /// One entry of a `\celldata`'s data: inline content, which the rule sets
    /// as one paragraph so that a cell holds blocks on every backend.
    Inline(NodeSlice<'t, Flm>),
}

/// One problem the reader found in a table: the Error to report and, where
/// the reader left something out, the dropped mark to place for it.
#[derive(Debug, Clone)]
pub struct Problem {
    /// The Error, at the node it concerns.
    pub diagnostic: FlmDiagnostic,
    /// The payload of the dropped mark (`flm.pres.dropped`) for what was
    /// left out, where something was; the rule stages the marks before the
    /// table, in the order of the problems.
    pub dropped: Option<DroppedPayload>,
}

/// A table of cells as its reader reads it ([`read_grid`]).
#[derive(Debug, Clone)]
pub struct Grid<'t> {
    /// The table's model: its size and one entry per cell, in row-major order
    /// of the cells' top-left positions — the payload of the `flm.pres.cells`
    /// node the rule stages.
    pub payload: CellsPayload,
    /// The content of each cell, in the order of the model's entries.
    pub contents: Vec<CellContent<'t>>,
    /// Every problem found, in the order of the body.
    pub problems: Vec<Problem>,
    /// Every cell style that a token of the table refers to, once each, in
    /// the order of first use. On the backend it runs on, the rule requires
    /// (records a requirement for) what each of these cell styles declares
    /// for that backend ([`CellStyle::requires`](super::CellStyle::requires)):
    /// style definitions on HTML, definition pieces or package requests on
    /// the recomposed LaTeX.
    pub styles: Vec<Name>,
}

/// Reads the table of cells `node` — a `cells` environment — against the
/// style declarations `styles`.
///
/// The names of the rows and the columns are read first, then every
/// instruction of the body, in order. This function reports nothing: each
/// problem it finds is returned in [`Grid::problems`] with the Error to
/// report and, where something was left out, the dropped mark to place, and
/// the rule that calls it reports them; [diagnostics](super::diagnostics)
/// states the recovery for each. The table itself is never refused. A node
/// with no body gives a grid with no cells, whose only problem is
/// [`Empty`]. Characters are read through [`chars_at_lowering`], the only
/// way a rule reads characters at lowering.
///
/// # Errors
///
/// [`LowerError::PlaceholderCharacters`] where a placement, a mapping, a
/// styles argument, a `\merge` range or a list of names holds a placeholder
/// of a user definition's template that is lowered without being expanded:
/// its characters are not known until the template is expanded.
pub fn read_grid<'t>(node: &NodeRef<'t, Flm>, styles: &Styles) -> Result<Grid<'t>, LowerError> {
    let body: Vec<NodeRef<'t, Flm>> = node.body().map(|body| body.iter().collect()).unwrap_or_default();
    let mut reader = Reader::new(styles);
    // The first pass: the names, then every instruction in order.
    for child in &body {
        let name = element_of(child).name.as_str();
        if name == ROWNAMES || name == COLNAMES {
            reader.names(child, if name == ROWNAMES { Axis::Row } else { Axis::Col })?;
        }
    }
    for child in &body {
        reader.instruction(child)?;
    }
    // The second pass, and the grid.
    Ok(reader.finish(node))
}

// --- the reader's state ---------------------------------------------------------------------

/// A row or a column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Row,
    Col,
}

impl Axis {
    fn word(self) -> &'static str {
        match self {
            Axis::Row => "row",
            Axis::Col => "column",
        }
    }

    fn index(self) -> usize {
        match self {
            Axis::Row => 0,
            Axis::Col => 1,
        }
    }
}

/// A range of indices along one axis, from zero: `start` included, `end`
/// excluded; an `end` of `None` spans to the end of the table.
type Span = (u64, Option<u64>);

/// Where a cell goes: its rows and its columns.
#[derive(Debug, Clone, Copy)]
struct Rect {
    row: Span,
    col: Span,
}

/// A placement or a mapping that cannot be read: what is wrong with it.
struct Bad(String);

/// What an index that is left out or written `.` means.
#[derive(Clone, Copy)]
enum IndexDefault {
    /// The current row or column.
    Current,
    /// The value given: `None` for "left open".
    Value(Option<u64>),
}

/// One piece of a placement or a mapping: characters, or a `\merge`.
#[derive(Clone)]
enum Piece<'t> {
    Text(String),
    Merge(NodeRef<'t, Flm>),
}

impl Piece<'_> {
    fn is_blank(&self) -> bool {
        matches!(self, Piece::Text(text) if text.trim().is_empty())
    }
}

/// One style token: its name, its value, and the node it was read from.
struct Token<'t> {
    name: String,
    value: Option<String>,
    at: NodeRef<'t, Flm>,
}

/// A style token resolved against its declaration.
type Resolved = (Effect, Name);

/// A cell placed by the first pass.
struct Placed<'t> {
    rect: Rect,
    styles: Vec<Resolved>,
    content: CellContent<'t>,
    at: NodeRef<'t, Flm>,
}

/// A problem, with the place in the body it belongs to.
struct Found {
    order: u64,
    problem: Problem,
}

/// The mapping of a `\celldata`: the rows and the columns its data goes to.
#[derive(Default)]
struct Mapping {
    rows: Vec<Span>,
    cols: Vec<Span>,
}

impl Mapping {
    /// The range of the `j`-th entry along one axis: the mapping's own, the
    /// open range continued one per entry past its start, the last range
    /// repeated past the end, or the current index where the mapping gives
    /// none.
    fn get(spans: &[Span], j: u64, current: u64) -> Span {
        let Some(&last) = spans.last() else { return (current, Some(current + 1)) };
        let len = spans.len() as u64;
        if j < len - 1 {
            return spans[j as usize];
        }
        match last {
            (start, None) => {
                let index = start + (j + 1 - len);
                (index, Some(index + 1))
            }
            closed => closed,
        }
    }
}

struct Reader<'s, 't> {
    styles: &'s Styles,
    current_row: u64,
    current_col: u64,
    /// The size so far: rows, columns.
    size: [u64; 2],
    /// The names of the rows and of the columns.
    names: [BTreeMap<String, u64>; 2],
    cells: Vec<(u64, Placed<'t>)>,
    problems: Vec<Found>,
    /// The cell styles referred to, once each.
    used: Vec<Name>,
    /// The place in the body of the instruction being read, counted.
    order: u64,
}

impl<'s, 't> Reader<'s, 't> {
    fn new(styles: &'s Styles) -> Reader<'s, 't> {
        Reader {
            styles,
            current_row: 0,
            current_col: 0,
            size: [0, 0],
            names: [BTreeMap::new(), BTreeMap::new()],
            cells: Vec::new(),
            problems: Vec::new(),
            used: Vec::new(),
            order: 0,
        }
    }

    fn report(&mut self, at: &NodeRef<'t, Flm>, info: impl DiagnosticInfo + 'static, dropped: Option<DroppedPayload>) {
        let diagnostic = Diagnostic::error(info, at.span().clone());
        self.problems.push(Found { order: self.order, problem: Problem { diagnostic, dropped } });
    }

    fn current(&self, axis: Axis) -> u64 {
        match axis {
            Axis::Row => self.current_row,
            Axis::Col => self.current_col,
        }
    }

    fn move_next_row(&mut self) {
        self.current_row += 1;
        self.current_col = 0;
    }

    // --- the names --------------------------------------------------------------------------

    fn names(&mut self, node: &NodeRef<'t, Flm>, axis: Axis) -> Result<(), LowerError> {
        let Ok(Some(content)) = node.argument_content_nodes_named(NAMES_ARG) else { return Ok(()) };
        let text = match chars_at_lowering(content.iter())? {
            Ok(text) => text,
            Err(_) => {
                let detail = format!("the {} names may hold only names separated by commas", axis.word());
                self.report(node, BadNames { detail }, None);
                return Ok(());
            }
        };
        let word = axis.word();
        for (k, entry) in text.split(',').enumerate() {
            let entry = entry.trim();
            let index = k as u64;
            if entry.is_empty() {
                continue;
            }
            let detail = if !is_name(entry) {
                format!(
                    "‘{entry}’ is not a {word} name: a name begins with a letter and holds only letters, \
                     digits and underscores"
                )
            } else if self.names[axis.index()].contains_key(entry) {
                format!("the {word} name ‘{entry}’ may be declared once only")
            } else if self.names[axis.index()].values().any(|&named| named == index) {
                format!("{word} {} may have one name only", index + 1)
            } else {
                self.names[axis.index()].insert(entry.to_string(), index);
                continue;
            };
            self.report(node, BadNames { detail }, None);
        }
        Ok(())
    }

    // --- the body's instructions -------------------------------------------------------------

    fn instruction(&mut self, node: &NodeRef<'t, Flm>) -> Result<(), LowerError> {
        if is_comment_or_blank(node) {
            return Ok(());
        }
        self.order += 1;
        let element = element_of(node).name.as_str();
        match element {
            CELL => {
                self.cell(node, None, &[])?;
            }
            DATA => self.celldata(node)?,
            ROW => self.move_next_row(),
            ROWNAMES | COLNAMES => {}
            structural::PARBREAK => {}
            _ => {
                let dropped = DroppedPayload {
                    condition: Name::from("flm.cells.content-outside-cell"),
                    detail: String::from("content outside \\cell"),
                };
                self.report(node, ContentOutsideCell, Some(dropped));
            }
        }
        Ok(())
    }

    /// A `\cell`, placed where its own placement says, else at `default` (the
    /// `\celldata` entry it stands for), else at the current row and column;
    /// its own style tokens come before `default_styles`. Returns the number
    /// of columns it spans, by which a `\celldata` advances.
    fn cell(
        &mut self,
        node: &NodeRef<'t, Flm>,
        default: Option<Rect>,
        default_styles: &[Resolved],
    ) -> Result<u64, LowerError> {
        let mut styles = match node.argument_content_nodes_named(STYLES_ARG) {
            Ok(Some(content)) => {
                let lists = self.style_lists(node, content, false)?;
                self.resolve(lists.into_iter().next().unwrap_or_default())
            }
            _ => Vec::new(),
        };
        styles.extend_from_slice(default_styles);
        let fallback = default.unwrap_or(Rect {
            row: (self.current_row, Some(self.current_row + 1)),
            col: (self.current_col, Some(self.current_col + 1)),
        });
        let rect = match node.argument_content_nodes_named(PLACEMENT_ARG) {
            Ok(Some(content)) => match self.placement(content)? {
                Ok(rect) => rect,
                Err(Bad(detail)) => {
                    self.report(node, BadPlacement { detail }, None);
                    fallback
                }
            },
            _ => fallback,
        };
        let content = match node.argument_content_nodes_named(CONTENT_ARG) {
            Ok(Some(content)) => content,
            _ => empty_slice(node),
        };
        Ok(self.add(rect, styles, CellContent::Blocks(content), node))
    }

    /// Places one cell, and advances the current column past it. Returns the
    /// number of columns it spans.
    fn add(&mut self, rect: Rect, styles: Vec<Resolved>, content: CellContent<'t>, at: &NodeRef<'t, Flm>) -> u64 {
        let [rows, cols] = self.size;
        // A span to the end of the table covers its first index at least now,
        // and is given its end by the second pass.
        let row_end = rect.row.1.unwrap_or(rect.row.0 + 1).max(rect.row.0 + 1);
        let col_end = rect.col.1.unwrap_or((rect.col.0 + 1).max(cols));
        let new_rows = rows.max(row_end);
        let new_cols = cols.max(col_end.max(rect.col.0 + 1));
        let span = col_end.saturating_sub(rect.col.0);
        self.current_col = col_end;
        if new_rows.saturating_mul(new_cols) > MAX_CELLS_POSITIONS {
            let dropped = DroppedPayload {
                condition: Name::from("flm.cells.too-large"),
                detail: String::from("cell making the table too large"),
            };
            self.report(at, TooLarge { limit: MAX_CELLS_POSITIONS, rows: new_rows, cols: new_cols }, Some(dropped));
            return span;
        }
        self.size = [new_rows, new_cols];
        self.cells.push((self.order, Placed { rect, styles, content, at: *at }));
        span
    }

    fn celldata(&mut self, node: &NodeRef<'t, Flm>) -> Result<(), LowerError> {
        let lists = match node.argument_content_nodes_named(STYLES_ARG) {
            Ok(Some(content)) => self.style_lists(node, content, true)?,
            _ => vec![Vec::new()],
        };
        let lists: Vec<Vec<Resolved>> = lists.into_iter().map(|list| self.resolve(list)).collect();
        let mapping = match node.argument_content_nodes_named(MAPPING_ARG) {
            Ok(Some(content)) => match self.mapping(content)? {
                Ok(mapping) => mapping,
                Err(Bad(detail)) => {
                    self.report(node, BadPlacement { detail }, None);
                    Mapping::default()
                }
            },
            _ => Mapping::default(),
        };
        let data = match node.argument_content_nodes_named(DATA_ARG) {
            Ok(Some(content)) => content,
            _ => empty_slice(node),
        };
        self.current_row = Mapping::get(&mapping.rows, 0, self.current_row).0;
        self.current_col = Mapping::get(&mapping.cols, 0, self.current_col).0;
        let mut rows = split(data, ROW);
        // A trailing `\\` ends the last row and adds none: an empty last part
        // after a final `\\` is dropped, while a `\\` on its own inside the
        // data still makes an empty row. See decision rationale at
        // [§flm-dd-dr:std-cells-reader]
        if rows.len() > 1 && rows.last().is_some_and(|row| row.iter().all(|node| is_comment_or_blank(&node))) {
            rows.pop();
        }
        for (row_j, row) in rows.into_iter().enumerate() {
            let mut col_j: u64 = 0;
            for entry in split(row, COLUMN) {
                self.order += 1;
                let rect = Rect {
                    row: Mapping::get(&mapping.rows, row_j as u64, self.current_row),
                    col: Mapping::get(&mapping.cols, col_j, self.current_col),
                };
                let styles = lists[(col_j as usize).min(lists.len() - 1)].clone();
                let shown: Vec<NodeRef<'t, Flm>> = entry
                    .iter()
                    .filter(|node| !is_comment_or_blank(node))
                    .collect();
                match shown.as_slice() {
                    [one] if element_of(one).name.as_str() == CELL => {
                        col_j += self.cell(one, Some(rect), &styles)?;
                    }
                    [] => {
                        self.current_col = rect.col.1.unwrap_or(rect.col.0 + 1);
                        col_j += 1;
                    }
                    [first, ..] => {
                        self.add(rect, styles, CellContent::Inline(entry), first);
                        col_j += 1;
                    }
                }
            }
            self.move_next_row();
        }
        Ok(())
    }

    // --- the styles ---------------------------------------------------------------------------

    /// The token lists of a styles argument: one, or, where `per_entry`, one
    /// per comma-separated list. An argument holding something other than
    /// characters and groups is reported, and reads as one empty list.
    fn style_lists(
        &mut self,
        node: &NodeRef<'t, Flm>,
        content: NodeSlice<'t, Flm>,
        per_entry: bool,
    ) -> Result<Vec<Vec<Token<'t>>>, LowerError> {
        let mut lists: Vec<Vec<Token<'t>>> = vec![Vec::new()];
        let mut pending_values: Vec<(NodeRef<'t, Flm>, String)> = Vec::new();
        for child in content.iter() {
            if child.is_comment() {
                continue;
            }
            if child.is_group() && child.group_type() == Some(FlmGroupType::Content) {
                let value = match chars_at_lowering(child.children().iter())? {
                    Ok(value) => value,
                    Err(_) => {
                        self.report(node, BadStyles, None);
                        return Ok(vec![Vec::new()]);
                    }
                };
                let list = lists.last_mut().expect("at least one list");
                match list.last_mut() {
                    Some(token) if token.value.is_none() => token.value = Some(value),
                    _ => pending_values.push((child, value)),
                }
                continue;
            }
            let text = match chars_at_lowering(std::iter::once(child))? {
                Ok(text) => text,
                Err(_) => {
                    self.report(node, BadStyles, None);
                    return Ok(vec![Vec::new()]);
                }
            };
            let pieces: Vec<&str> = if per_entry { text.split(',').collect() } else { vec![text.as_str()] };
            for (k, piece) in pieces.into_iter().enumerate() {
                if k > 0 {
                    lists.push(Vec::new());
                }
                let list = lists.last_mut().expect("at least one list");
                list.extend(
                    piece
                        .split_whitespace()
                        .map(|name| Token { name: name.to_string(), value: None, at: child }),
                );
            }
        }
        for (at, value) in pending_values {
            let detail = format!("the value ‘{{{value}}}’ must follow the name of a cell style directly, once");
            self.report(&at, BadStyleValue { detail }, None);
        }
        Ok(lists)
    }

    /// The tokens of one list, each resolved against its declaration; a token
    /// no declaration covers, or whose value its declaration does not take,
    /// is reported and left out.
    fn resolve(&mut self, tokens: Vec<Token<'t>>) -> Vec<Resolved> {
        let mut resolved = Vec::new();
        for token in tokens {
            let Some(style) = self.styles.get(&token.name) else {
                self.report(&token.at, UnknownStyle { name: token.name }, None);
                continue;
            };
            let effect = match (style.form, token.value) {
                (StyleForm::Fixed(effect), None) => effect,
                (StyleForm::Fixed(_), Some(value)) => {
                    let detail = format!("the cell style ‘{}’ takes no value, not ‘{{{value}}}’", token.name);
                    self.report(&token.at, BadStyleValue { detail }, None);
                    continue;
                }
                (StyleForm::Structural(parameter), None) => {
                    let detail = format!(
                        "the cell style ‘{}’ must be given a value in braces, one of {}",
                        token.name,
                        parameter.values().join(", ")
                    );
                    self.report(&token.at, BadStyleValue { detail }, None);
                    continue;
                }
                (StyleForm::Structural(parameter), Some(value)) => match parameter.effect(value.trim()) {
                    Some(effect) => effect,
                    None => {
                        let detail = format!(
                            "the cell style ‘{}’ takes one of {}, not ‘{value}’",
                            token.name,
                            parameter.values().join(", ")
                        );
                        self.report(&token.at, BadStyleValue { detail }, None);
                        continue;
                    }
                },
            };
            if !self.used.contains(&style.name) {
                self.used.push(style.name.clone());
            }
            resolved.push((effect, style.name.clone()));
        }
        resolved
    }

    // --- placements, mappings, ranges, indices -----------------------------------------------

    /// A placement: `COL` or `ROW;COL`.
    fn placement(&self, content: NodeSlice<'t, Flm>) -> Result<Result<Rect, Bad>, LowerError> {
        let pieces = match pieces(content)? {
            Ok(pieces) => pieces,
            Err(bad) => return Ok(Err(bad)),
        };
        let parts = split_pieces(pieces, ';');
        let (row, col) = match parts.as_slice() {
            [col] => (Vec::new(), col.clone()),
            [row, col] => (row.clone(), col.clone()),
            _ => return Ok(Err(Bad(String::from("a cell's placement must be written COL or ROW;COL")))),
        };
        let row = match self.placement_index(&row, Axis::Row)? {
            Ok(span) => span,
            Err(bad) => return Ok(Err(bad)),
        };
        let col = match self.placement_index(&col, Axis::Col)? {
            Ok(span) => span,
            Err(bad) => return Ok(Err(bad)),
        };
        Ok(Ok(Rect { row, col }))
    }

    /// One side of a placement: empty for the current index, else an index or
    /// a `\merge`.
    fn placement_index(&self, part: &[Piece<'t>], axis: Axis) -> Result<Result<Span, Bad>, LowerError> {
        if part.is_empty() {
            let current = self.current(axis);
            return Ok(Ok((current, Some(current + 1))));
        }
        let shown: Vec<&Piece<'t>> = part.iter().filter(|piece| !piece.is_blank()).collect();
        let [one] = shown.as_slice() else {
            return Ok(Err(Bad(format!(
                "a cell's {} must be one number, name, ‘.’ or \\merge{{…}}",
                axis.word()
            ))));
        };
        Ok(match one {
            Piece::Merge(merge) => {
                let text = match merge_range(merge)? {
                    Ok(text) => text,
                    Err(bad) => return Ok(Err(bad)),
                };
                match self.range(&text, axis, IndexDefault::Current, Some(0), None) {
                    Ok((Some(start), end)) => Ok((start, end)),
                    Ok((None, _)) => Err(Bad(format!("the range ‘{text}’ must give its start"))),
                    Err(bad) => Err(bad),
                }
            }
            Piece::Text(text) => match self.index(text, axis, IndexDefault::Current) {
                Ok(Some(index)) => Ok((index, Some(index + 1))),
                Ok(None) => Err(Bad(format!("the {} ‘{}’ must be given", axis.word(), text.trim()))),
                Err(bad) => Err(bad),
            },
        })
    }

    /// A mapping: `COLS` or `ROWS;COLS`.
    fn mapping(&self, content: NodeSlice<'t, Flm>) -> Result<Result<Mapping, Bad>, LowerError> {
        let pieces = match pieces(content)? {
            Ok(pieces) => pieces,
            Err(bad) => return Ok(Err(bad)),
        };
        let parts = split_pieces(pieces, ';');
        let (rows, cols) = match parts.as_slice() {
            [cols] => (Vec::new(), cols.clone()),
            [rows, cols] => (rows.clone(), cols.clone()),
            _ => return Ok(Err(Bad(String::from("the mapping of \\celldata must be written COLS or ROWS;COLS")))),
        };
        let rows = match self.mapping_axis(rows, Axis::Row)? {
            Ok(spans) => spans,
            Err(bad) => return Ok(Err(bad)),
        };
        let cols = match self.mapping_axis(cols, Axis::Col)? {
            Ok(spans) => spans,
            Err(bad) => return Ok(Err(bad)),
        };
        Ok(Ok(Mapping { rows, cols }))
    }

    /// One side of a mapping: a comma-separated list of indices, ranges and
    /// `\merge`s, each range one entry per index but a `\merge`, which is one
    /// entry spanning them; nothing for "from the current index on".
    fn mapping_axis(&self, part: Vec<Piece<'t>>, axis: Axis) -> Result<Result<Vec<Span>, Bad>, LowerError> {
        if part.is_empty() {
            return Ok(Ok(Vec::new()));
        }
        let mut spans: Vec<Span> = Vec::new();
        let mut current: Option<u64> = Some(0);
        for entry in split_pieces(part, ',') {
            if spans.last().is_some_and(|last| last.1.is_none()) {
                return Ok(Err(Bad(format!("only the last {} range of a mapping may be left open", axis.word()))));
            }
            let shown: Vec<&Piece<'t>> = entry.iter().filter(|piece| !piece.is_blank()).collect();
            let [one] = shown.as_slice() else {
                return Ok(Err(Bad(format!(
                    "each {} of a mapping must be one index, range or \\merge{{…}}",
                    axis.word()
                ))));
            };
            let (text, merged) = match one {
                Piece::Merge(merge) => match merge_range(merge)? {
                    Ok(text) => (text, true),
                    Err(bad) => return Ok(Err(bad)),
                },
                Piece::Text(text) => (text.clone(), false),
            };
            let (start, end) = match self.range(&text, axis, IndexDefault::Value(current), current, None) {
                Ok((Some(start), end)) => (start, end),
                Ok((None, _)) => return Ok(Err(Bad(format!("the range ‘{}’ must give its start", text.trim())))),
                Err(bad) => return Ok(Err(bad)),
            };
            // The length is counted from the range before any entry is
            // listed: a mapping may name ranges whose indices, listed one per
            // entry, would not fit in memory.
            let entries = match end {
                Some(end) if !merged => end - start,
                _ => 1,
            };
            if spans.len() as u64 + entries > MAX_CELLS_POSITIONS {
                return Ok(Err(Bad(format!(
                    "the mapping of \\celldata may list at most {MAX_CELLS_POSITIONS} {}s",
                    axis.word()
                ))));
            }
            match end {
                Some(end) if !merged => {
                    spans.extend((start..end).map(|index| (index, Some(index + 1))));
                }
                end => spans.push((start, end)),
            }
            current = end;
        }
        Ok(Ok(spans))
    }

    /// A range: `2-4`, `2+3`, `1,2,3`, or a single index; an end of `None` is
    /// left open.
    fn range(
        &self,
        text: &str,
        axis: Axis,
        default: IndexDefault,
        default_start: Option<u64>,
        default_end: Option<u64>,
    ) -> Result<(Option<u64>, Option<u64>), Bad> {
        if text.contains(',') {
            let mut covered: Vec<(u64, u64)> = Vec::new();
            for part in text.split(',') {
                match self.range(part, axis, default, None, None)? {
                    (Some(start), Some(end)) => covered.push((start, end)),
                    _ => return Err(Bad(format!("each part of the range ‘{}’ must give both its ends", text.trim()))),
                }
            }
            covered.sort_unstable();
            let start = covered[0].0;
            let mut end = covered[0].1;
            for &(part_start, part_end) in &covered[1..] {
                if part_start > end {
                    return Err(Bad(format!("the range ‘{}’ must be contiguous", text.trim())));
                }
                end = end.max(part_end);
            }
            return Ok((Some(start), Some(end)));
        }
        if let Some((start, end)) = text.split_once('-') {
            let start = self.index(start, axis, IndexDefault::Value(default_start))?;
            let end = match self.index(end, axis, IndexDefault::Value(None))? {
                Some(last) => Some(last + 1),
                None => default_end,
            };
            if matches!((start, end), (Some(start), Some(end)) if end <= start) {
                return Err(Bad(format!("the range ‘{}’ must not run backwards", text.trim())));
            }
            return Ok((start, end));
        }
        if let Some((start, count)) = text.split_once('+') {
            let start = self.index(start, axis, default)?;
            let count = count.trim();
            if count.is_empty() || !count.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(Bad(format!("in the range ‘{}’, ‘{count}’ must be a number", text.trim())));
            }
            let count: u64 = count.parse().unwrap_or(u64::MAX);
            if count == 0 {
                return Err(Bad(format!("the range ‘{}’ must span one {} at least", text.trim(), axis.word())));
            }
            let Some(start) = start else {
                return Err(Bad(format!("the range ‘{}’ must give its start", text.trim())));
            };
            let end = start.saturating_add(count);
            if end > MAX_CELLS_POSITIONS {
                return Err(Bad(too_far(axis)));
            }
            return Ok((Some(start), Some(end)));
        }
        let index = self.index(text, axis, default)?;
        Ok((index, index.map(|index| index + 1)))
    }

    /// An index: a number from 1, a declared name, or `.` or nothing for what
    /// `default` says.
    fn index(&self, text: &str, axis: Axis, default: IndexDefault) -> Result<Option<u64>, Bad> {
        let text = text.trim();
        if text.is_empty() || text == "." {
            return Ok(match default {
                IndexDefault::Current => Some(self.current(axis)),
                IndexDefault::Value(value) => value,
            });
        }
        if text.bytes().all(|byte| byte.is_ascii_digit()) {
            let number: u64 = text.parse().unwrap_or(u64::MAX);
            if number == 0 {
                return Err(Bad(format!("a {} number must be 1 or more, not 0", axis.word())));
            }
            if number > MAX_CELLS_POSITIONS {
                return Err(Bad(too_far(axis)));
            }
            return Ok(Some(number - 1));
        }
        match self.names[axis.index()].get(text) {
            Some(&index) => Ok(Some(index)),
            None => Err(Bad(format!(
                "‘{text}’ is not a {word} number or a declared {word} name",
                word = axis.word()
            ))),
        }
    }

    // --- the second pass and the grid --------------------------------------------------------

    fn finish(mut self, node: &NodeRef<'t, Flm>) -> Grid<'t> {
        let [rows, cols] = self.size;
        let mut held: Vec<Option<(u64, u64)>> = vec![None; (rows * cols) as usize];
        let mut accepted: Vec<(CellSpec, CellContent<'t>)> = Vec::new();
        let cells = std::mem::take(&mut self.cells);
        for (order, placed) in cells {
            let rect = placed.rect;
            let row = (rect.row.0, rect.row.1.unwrap_or(rows));
            let col = (rect.col.0, rect.col.1.unwrap_or(cols));
            let overlapped = (row.0..row.1)
                .flat_map(|r| (col.0..col.1).map(move |c| (r, c)))
                .find_map(|(r, c)| held[(r * cols + c) as usize]);
            if let Some((earlier_row, earlier_col)) = overlapped {
                self.order = order;
                let dropped = DroppedPayload {
                    condition: Name::from("flm.cells.overlap"),
                    detail: String::from("cell overlapping another"),
                };
                self.report(&placed.at, Overlap { row: earlier_row + 1, col: earlier_col + 1 }, Some(dropped));
                continue;
            }
            for r in row.0..row.1 {
                for c in col.0..col.1 {
                    held[(r * cols + c) as usize] = Some((row.0, col.0));
                }
            }
            let mut spec = CellSpec::at(row.0 as u32, col.0 as u32);
            spec.row_span = (row.1 - row.0) as u32;
            spec.col_span = (col.1 - col.0) as u32;
            apply(&mut spec, &placed.styles);
            accepted.push((spec, placed.content));
        }
        if accepted.is_empty() {
            self.order = u64::MAX;
            self.report(node, Empty, None);
        }
        accepted.sort_by_key(|(spec, _)| (spec.row, spec.col));
        let (specs, contents): (Vec<CellSpec>, Vec<CellContent<'t>>) = accepted.into_iter().unzip();
        let mut problems = self.problems;
        problems.sort_by_key(|found| found.order);
        Grid {
            payload: CellsPayload { rows: rows as u32, cols: cols as u32, cells: specs },
            contents,
            problems: problems.into_iter().map(|found| found.problem).collect(),
            styles: self.used,
        }
    }
}

/// Writes the facts of the resolved tokens into a cell's entry: the first
/// alignment and the first header kind decide; every edge line and every
/// custom style is kept once, in order.
fn apply(spec: &mut CellSpec, styles: &[Resolved]) {
    for (effect, name) in styles {
        match *effect {
            Effect::Align(align) => {
                spec.align.get_or_insert(align);
            }
            Effect::Header(header) => {
                spec.header.get_or_insert(header);
            }
            Effect::Rule(edge) => {
                if !spec.rules.contains(&edge) {
                    spec.rules.push(edge);
                }
            }
            Effect::Custom => {
                if !spec.styles.contains(name) {
                    spec.styles.push(name.clone());
                }
            }
        }
    }
}

/// The message of an index past the largest.
fn too_far(axis: Axis) -> String {
    format!("a {} number may be at most {MAX_CELLS_POSITIONS}", axis.word())
}

/// Whether `text` is a name of a row or a column: an ASCII letter, then ASCII
/// letters, digits and underscores.
fn is_name(text: &str) -> bool {
    let mut chars = text.chars();
    chars.next().is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

/// Whether `node` is a comment or blank text, in the lowering helpers' sense
/// ([`is_blank`]): a no-break space is content.
fn is_comment_or_blank(node: &NodeRef<'_, Flm>) -> bool {
    node.is_comment() || is_blank(node)
}

/// An empty run of nodes of `node`'s tree.
fn empty_slice<'t>(node: &NodeRef<'t, Flm>) -> NodeSlice<'t, Flm> {
    let at = node.id().index() as u32;
    node.tree().slice(at..at).expect("an empty range of the tree")
}

/// The runs of `content` between its top-level nodes of the element
/// `separator`, as str::split cuts a string: one more run than there are
/// separators.
fn split<'t>(content: NodeSlice<'t, Flm>, separator: &str) -> Vec<NodeSlice<'t, Flm>> {
    let range = content.range();
    let tree = content.first().map(|node| node.tree());
    let Some(tree) = tree else { return vec![content] };
    let mut runs = Vec::new();
    let mut start = range.start;
    for (k, node) in content.iter().enumerate() {
        let at = range.start + k as u32;
        if node.is_callable() && element_of(&node).name.as_str() == separator {
            runs.push(tree.slice(start..at).expect("a run of siblings"));
            start = at + 1;
        }
    }
    runs.push(tree.slice(start..range.end).expect("a run of siblings"));
    runs
}

/// The pieces of a placement or a mapping: runs of characters and `\merge`s,
/// comments skipped. Anything else is an argument that cannot be read.
fn pieces<'t>(content: NodeSlice<'t, Flm>) -> Result<Result<Vec<Piece<'t>>, Bad>, LowerError> {
    let mut pieces = Vec::new();
    let mut run: Vec<NodeRef<'t, Flm>> = Vec::new();
    let flush = |run: &mut Vec<NodeRef<'t, Flm>>, pieces: &mut Vec<Piece<'t>>| -> Result<Result<(), Bad>, LowerError> {
        if run.is_empty() {
            return Ok(Ok(()));
        }
        let read = chars_at_lowering(std::mem::take(run))?;
        Ok(match read {
            Ok(text) => {
                pieces.push(Piece::Text(text));
                Ok(())
            }
            Err(_) => Err(Bad(String::from(
                "a placement or a mapping may hold only numbers, names, ranges and \\merge{…}",
            ))),
        })
    };
    for node in content.iter() {
        if node.is_comment() {
            continue;
        }
        if node.is_callable() && element_of(&node).name.as_str() == MERGE {
            if let Err(bad) = flush(&mut run, &mut pieces)? {
                return Ok(Err(bad));
            }
            pieces.push(Piece::Merge(node));
        } else {
            run.push(node);
        }
    }
    if let Err(bad) = flush(&mut run, &mut pieces)? {
        return Ok(Err(bad));
    }
    Ok(Ok(pieces))
}

/// The pieces cut at every `separator` among their characters, as str::split
/// cuts a string.
fn split_pieces<'t>(pieces: Vec<Piece<'t>>, separator: char) -> Vec<Vec<Piece<'t>>> {
    let mut parts: Vec<Vec<Piece<'t>>> = vec![Vec::new()];
    for piece in pieces {
        match piece {
            Piece::Text(text) => {
                for (k, segment) in text.split(separator).enumerate() {
                    if k > 0 {
                        parts.push(Vec::new());
                    }
                    if !segment.is_empty() {
                        parts.last_mut().expect("a part").push(Piece::Text(segment.to_string()));
                    }
                }
            }
            merge => parts.last_mut().expect("a part").push(merge),
        }
    }
    parts
}

/// The range a `\merge` gives, read as characters.
fn merge_range(merge: &NodeRef<'_, Flm>) -> Result<Result<String, Bad>, LowerError> {
    let Ok(Some(content)) = merge.argument_content_nodes_named(RANGE_ARG) else { return Ok(Ok(String::new())) };
    Ok(match chars_at_lowering(content.iter())? {
        Ok(text) => Ok(text),
        Err(_) => Err(Bad(String::from("the range of \\merge may hold only numbers, names and range signs"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mapping_entry_is_its_own_the_open_range_continued_or_the_last_repeated() {
        let spans = [(0, Some(1)), (2, Some(3))];
        assert_eq!(Mapping::get(&spans, 0, 9), (0, Some(1)));
        assert_eq!(Mapping::get(&spans, 1, 9), (2, Some(3)));
        assert_eq!(Mapping::get(&spans, 5, 9), (2, Some(3)));
        let open = [(0, Some(1)), (1, Some(2)), (2, None)];
        assert_eq!(Mapping::get(&open, 2, 9), (2, Some(3)));
        assert_eq!(Mapping::get(&open, 4, 9), (4, Some(5)));
        assert_eq!(Mapping::get(&[], 3, 7), (7, Some(8)));
    }

    #[test]
    fn a_name_is_a_letter_then_letters_digits_and_underscores() {
        assert!(is_name("height"));
        assert!(is_name("row_2"));
        assert!(!is_name("1x"));
        assert!(!is_name("a-b"));
        assert!(!is_name(""));
    }
}
