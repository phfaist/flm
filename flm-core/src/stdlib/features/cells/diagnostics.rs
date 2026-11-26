//! This module defines the diagnostic conditions of the [`cells`](super)
//! feature, which provides tables of cells. Their identifiers have the form
//! `flm.cells.<condition>`. Every condition is an error.
//!
//! The lowering rule of the `cells` environment reads the table with
//! [`read_grid`](super::read_grid), which finds the mistakes in the table.
//! Both the general rule [`CellsRule`](super::lower::CellsRule) and the rule
//! [`CellsLatexRule`](super::latex::CellsLatexRule) of the `latex-recomposed`
//! backend report these mistakes each time they lower the table, so a
//! document gets the same reports on every backend. Each mistake causes only
//! a part of the table to be ignored or left out, and the rest of the table
//! is written as usual. The lowering rule [`OutsideRule`](super::lower::OutsideRule)
//! reports [`OutsideTable`] for a command of a table of cells that is written
//! outside of its place.
//!
//! These conditions are reported only when a table is lowered. The `check`
//! workflow ([`CHECK_WORKFLOW`](crate::stdlib::CHECK_WORKFLOW)) lowers no
//! table, and therefore does not report them.
//!
//! When a part of the document is left out of the output, the lowering rule
//! places a *dropped mark* in the output: a `flm.pres.dropped` node
//! ([`DroppedPayload`](crate::stdlib::backends::pres::DroppedPayload)),
//! which shows the reader of the output that something was left out. The
//! dropped marks for the mistakes in a table are placed immediately before
//! the table.

use techy::error::DiagnosticInfo;

/// The error that is reported when the body of a `cells` environment contains
/// content outside of a cell (`flm.cells.content-outside-cell`).
///
/// The body of a `cells` environment may contain only the commands `\cell`,
/// `\celldata`, `\\`, `\rownames` and `\colnames`, together with whitespace,
/// blank lines and comments. Text and other content belong in a cell: in the
/// content of a `\cell`, as in `\cell{Some text}`, or in an entry of the data
/// of a `\celldata`, as in `\celldata{Some text & More text}`.
///
/// Reported by the lowering rule of the `cells` environment, on every
/// backend, at the content. The content is left out of the output, and a
/// dropped mark (`flm.pres.dropped`) is placed immediately before the table.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.cells.content-outside-cell",
    message = "A ‘cells’ environment may hold only ‘\\cell’, ‘\\celldata’, ‘\\\\’, ‘\\rownames’ and ‘\\colnames’"
)]
pub struct ContentOutsideCell;

/// The error that is reported when the placement of a `\cell` or the mapping
/// of a `\celldata` is not valid (`flm.cells.bad-placement`).
///
/// The placement of a `\cell` is its optional argument `[…]`. It has the form
/// `[COL]` or `[ROW;COL]`. Each part is a number, a name declared with
/// `\rownames` or `\colnames`, `.` or nothing for the current row or column,
/// or `\merge{RANGE}` for a cell that spans several rows or columns, as in
/// `\cell[2;3]{…}` or `\cell[1;\merge{3-4}]{…}`.
///
/// The mapping of a `\celldata` is its optional argument `[…]`. It has the
/// form `[COLS]` or `[ROWS;COLS]`. Each part is a comma-separated list of
/// numbers, names, ranges and `\merge{RANGE}`, as in
/// `\celldata[3,2,1]{A & B & C}` or `\celldata[2-5]{A & B & C & D}`.
///
/// A range has the form `2-4`, or `2+3` for three rows or columns starting
/// at 2. Inside `\merge`, a range can also be a contiguous union such as
/// `1,2,3`.
/// Rows and columns are numbered from 1, and a number may be at most
/// [`MAX_CELLS_POSITIONS`](crate::stdlib::backends::pres::MAX_CELLS_POSITIONS).
/// The [documentation of the `cells` feature](super#placements-mappings-and-ranges)
/// describes placements, mappings and ranges in full.
///
/// Reported by the lowering rule of the `cells` environment, on every
/// backend, at the `\cell` or the `\celldata`. The message states what is
/// wrong. An invalid placement is ignored: the cell goes to the current row
/// and column, or, for a `\cell` that is an entry of a `\celldata`, to the
/// position of that entry. An invalid mapping is ignored as a whole, and the
/// data of the `\celldata` is placed from the current row and column on.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.cells.bad-placement", message = "{detail}")]
pub struct BadPlacement {
    /// The message of the diagnostic. It states what is wrong with the
    /// placement or the mapping.
    pub detail: String,
}

/// The error that is reported when the style argument `<…>` of a `\cell` or
/// a `\celldata` contains content other than text (`flm.cells.bad-styles`).
///
/// The style argument contains style tokens separated by whitespace. A style
/// token is the name of a cell style, optionally followed by a value in
/// braces, as in `\cell<H align{left}>{…}`. In a `\celldata`, commas separate
/// the tokens of successive entries of a row, as in
/// `\celldata<H l, H c>{One & Two}`. A token that is not the name of a cell
/// style is reported as [`UnknownStyle`].
///
/// Reported by the lowering rule of the `cells` environment, on every
/// backend, at the `\cell` or the `\celldata`. The whole style argument is
/// ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.cells.bad-styles",
    message = "The styles of a cell may hold only style names, each with an optional value in braces"
)]
pub struct BadStyles;

/// The error that is reported when a style token is not the name of a cell
/// style declared in the registry (`flm.cells.unknown-style`).
///
/// Each style token of a `\cell` or a `\celldata` refers to a cell style
/// ([`CellStyle`](super::CellStyle)) by its name, as in `\cell<H l>{…}`. The
/// cell styles of a registry are the ones that the `cells` feature ships
/// ([`shipped`](super::shipped)), and the ones that other features contribute
/// to the service slot [`STYLES_SLOT`](super::STYLES_SLOT).
///
/// Reported by the lowering rule of the `cells` environment, on every
/// backend, at the token. The token is ignored, and the other tokens of the
/// cell are used.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.cells.unknown-style", message = "Unknown cell style ‘{name}’")]
pub struct UnknownStyle {
    /// The name of the style token, as written.
    pub name: String,
}

/// The error that is reported when a style token has a missing or an invalid
/// value (`flm.cells.bad-style-value`).
///
/// A value is written in braces directly after the name of a cell style, and
/// each style name takes at most one value. A cell style of the structural
/// form ([`StyleForm::Structural`](super::StyleForm::Structural)) requires
/// one of the values of its [`Parameter`](super::Parameter), as in
/// `align{left}`, `head{column}` or `rule{below}`. A cell style of the fixed
/// form ([`StyleForm::Fixed`](super::StyleForm::Fixed)), such as `l`, `H` or
/// `green`, takes no value.
///
/// Reported by the lowering rule of the `cells` environment, on every
/// backend, at the token. The token is ignored, and the other tokens of the
/// cell are used. A value that does not follow a style name is reported at
/// the value, and ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.cells.bad-style-value", message = "{detail}")]
pub struct BadStyleValue {
    /// The message of the diagnostic. It states what is wrong with the value.
    pub detail: String,
}

/// The error that is reported when an entry of `\rownames` or `\colnames` is
/// not valid (`flm.cells.bad-names`).
///
/// The argument of `\rownames` or `\colnames` is a comma-separated list of
/// names. The *k*-th entry is the name of row or column *k*, and an empty
/// entry gives no name, as in `\colnames{name,,city}`. A name begins with an
/// ASCII letter and contains only ASCII letters, digits and underscores. Each
/// row name and each column name may be declared only once, and each row or
/// column may have only one name.
///
/// Reported by the lowering rule of the `cells` environment, on every
/// backend, at the `\rownames` or the `\colnames`, once for each invalid
/// entry. The message states what is wrong. The invalid entry is ignored, and
/// the other entries are used.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.cells.bad-names", message = "{detail}")]
pub struct BadNames {
    /// The message of the diagnostic. It states what is wrong with the entry.
    pub detail: String,
}

/// The error that is reported when a cell covers a position that is already
/// covered by a cell earlier in the body of the table (`flm.cells.overlap`).
///
/// Each position of a table belongs to at most one cell. A cell covers more
/// than one position when it spans several rows or columns with `\merge`, as
/// in `\cell[1;\merge{3-4}]{…}`. When the data of a `\celldata` has more
/// entries than its mapping lists, the remaining entries are placed at the
/// last row or column of the mapping, where they overlap. An open last range,
/// as in `\celldata[2-]{…}`, instead continues one row or column per entry.
///
/// Reported by the lowering rule of the `cells` environment, on every
/// backend, at the later cell. The message gives the position of the earlier
/// cell. The later cell is left out of the output together with its content,
/// and a dropped mark (`flm.pres.dropped`) is placed immediately before the
/// table.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.cells.overlap",
    message = "Two cells may not share a position: this cell overlaps the cell at row {row}, column {col}"
)]
pub struct Overlap {
    /// The first row of the earlier cell, numbered from 1.
    pub row: u64,
    /// The first column of the earlier cell, numbered from 1.
    pub col: u64,
}

/// The error that is reported when a cell would make its table larger than
/// the largest size allowed (`flm.cells.too-large`).
///
/// The size of a table is its number of rows times its number of columns,
/// counted in positions. A table may have at most
/// [`MAX_CELLS_POSITIONS`](crate::stdlib::backends::pres::MAX_CELLS_POSITIONS)
/// positions (one million). This limit exists because each backend visits
/// every position of a table when it writes the table, and a single `\merge`
/// can span a very large number of rows or columns.
///
/// Reported by the lowering rule of the `cells` environment, on every
/// backend, at the cell. The cell is left out of the output together with its
/// content, and a dropped mark (`flm.pres.dropped`) is placed immediately
/// before the table.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.cells.too-large",
    message = "A ‘cells’ environment may have at most {limit} positions (rows times columns): \
               this cell would make it {rows} rows by {cols} columns"
)]
pub struct TooLarge {
    /// The largest number of positions that a table may have.
    pub limit: u64,
    /// The number of rows that the table would have with the cell.
    pub rows: u64,
    /// The number of columns that the table would have with the cell.
    pub cols: u64,
}

/// The error that is reported when a `cells` environment contains no cell
/// (`flm.cells.empty`).
///
/// An empty entry of a `\celldata` adds no cell. A table also counts as empty
/// when every one of its cells is left out because it would make the table
/// too large ([`TooLarge`]).
///
/// Reported by the lowering rule of the `cells` environment, on every
/// backend, at the environment. Nothing is written for the table, except the
/// dropped marks for the parts of the table that were left out.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.cells.empty",
    message = "A ‘cells’ environment must hold at least one cell"
)]
pub struct Empty;

/// The error that is reported when a command of a table of cells is written
/// outside of its place (`flm.cells.outside-table`).
///
/// The commands of a table of cells, and the character `&`, must be written
/// in the following places:
///
/// - `\cell` directly in the body of a `cells` environment, or as an entry of
///   the data of a `\celldata`, as in `\celldata{A & \cell<H>{B}}`;
/// - `\celldata`, `\rownames` and `\colnames` directly in the body of a
///   `cells` environment;
/// - `\\` directly in the body of a `cells` environment, or directly in the
///   data of a `\celldata`;
/// - `&` directly in the data of a `\celldata`, as in `\celldata{A & B}`;
/// - `\merge` in the placement of a `\cell` or in the mapping of a
///   `\celldata`, as in `\cell[1;\merge{3-4}]{…}`.
///
/// Reported by the lowering rule [`OutsideRule`](super::lower::OutsideRule)
/// of the command, on every backend, at the command. A `\cell` expands to its
/// content, and a `\celldata` expands to its data as inline content. The
/// other commands expand to a dropped mark (`flm.pres.dropped`), and so does
/// a `\cell` or a `\celldata` whose last argument is missing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.cells.outside-table", message = "{detail}")]
pub struct OutsideTable {
    /// The message of the diagnostic. It states where the command must be
    /// written ([`OUTSIDE_RULES`](super::lower::OUTSIDE_RULES)).
    pub detail: String,
}
