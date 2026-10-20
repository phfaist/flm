//! The conditions the `cells` feature reports, each identified as
//! `flm.cells.<condition>`: every mistake in a table of cells is an Error.
//!
//! Each item below states the condition first, then the feature's recovery:
//! what it does to go on (see
//! [recovery](crate::stdcore::backends::lowering::error)). A diagnostic's
//! message states the problem only.
//!
//! The table's reader ([`read_grid`](super::read_grid)) finds the first
//! eight conditions each time a rule lowers the table: on each backend that
//! renders it, and again where the table is shown inside a copied
//! [synthesized value](crate::guide::concepts_overview#synthesized-values). The rule of a command written outside a table
//! ([`OutsideRule`](super::lower::OutsideRule)) reports the ninth. Where
//! something cannot be placed in the table, it is left out whole, and the
//! rule stages a dropped mark (a `flm.pres.dropped` node,
//! [`DroppedPayload`](crate::stdcore::backends::pres::DroppedPayload)) in its
//! place: before the table for the reader's conditions, where the command
//! was for [`OutsideTable`].

use techy::error::DiagnosticInfo;

/// Condition `flm.cells.content-outside-cell`, an Error: the body of a `cells`
/// environment holds something other than `\cell`, `\celldata`, `\\`,
/// `\rownames`, `\colnames`, whitespace and comments, such as a paragraph or
/// a stray `&`.
///
/// Recovery: the content is left out, and a dropped mark is placed before
/// the table.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.cells.content-outside-cell",
    message = "a `cells` environment may hold only \\cell, \\celldata, \\\\, \\rownames and \\colnames"
)]
pub struct ContentOutsideCell;

/// Condition `flm.cells.bad-placement`, an Error: a cell's placement or a
/// `\celldata`'s mapping cannot be read: it is not of the form `ROW;COL`, an
/// index is not a number, a declared name, `.` or empty, a range is not
/// contiguous or runs backwards, or a `\merge` holds a command.
///
/// Recovery: a placement is ignored, and the cell goes where the current row
/// and column put it; a mapping is ignored whole.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.cells.bad-placement", message = "{detail}")]
pub struct BadPlacement {
    /// What cannot be read, as a sentence.
    pub detail: String,
}

/// Condition `flm.cells.bad-styles`, an Error: a cell's styles argument holds
/// something other than style names and values in braces, such as a command.
///
/// Recovery: the whole styles argument is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.cells.bad-styles",
    message = "the styles of a cell may hold only style names, each with an optional value in braces"
)]
pub struct BadStyles;

/// Condition `flm.cells.unknown-style`, an Error: a style token refers to no
/// cell style declared in the registry ([`CellStyle`](super::CellStyle)):
/// a typo, or a style whose feature is not registered.
///
/// Recovery: the token is ignored, and the cell's other tokens are kept.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.cells.unknown-style", message = "there is no cell style named ‘{name}’")]
pub struct UnknownStyle {
    /// The token as written.
    pub name: String,
}

/// Condition `flm.cells.bad-style-value`, an Error: a value on a token that
/// takes none (`green{x}`), a token of the structural form with no value or
/// with a value it does not take (`align`, `align{middle}`), or a value that
/// follows no style name.
///
/// Recovery: the token is ignored, and the cell's other tokens are kept.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.cells.bad-style-value", message = "{detail}")]
pub struct BadStyleValue {
    /// What is wrong with the value, as a sentence.
    pub detail: String,
}

/// Condition `flm.cells.bad-names`, an Error: an entry of `\rownames` or
/// `\colnames` that is not a valid name, a name declared twice, or two names
/// for one row or column.
///
/// Recovery: the offending entries are ignored, and the others are kept.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.cells.bad-names", message = "{detail}")]
pub struct BadNames {
    /// What is wrong with the entry, as a sentence.
    pub detail: String,
}

/// Condition `flm.cells.overlap`, an Error: a cell covers a position that an
/// earlier cell already holds. Data that runs past the end of a mapping is
/// placed at the mapping's last position, and so overlaps there. Reported at
/// the later cell, giving the earlier cell's position.
///
/// Recovery: the later cell is left out whole, its content not shown, and a
/// dropped mark is placed before the table.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.cells.overlap",
    message = "two cells may not share a position: this cell overlaps the cell at row {row}, column {col}"
)]
pub struct Overlap {
    /// The earlier cell's first row, counted from 1.
    pub row: u64,
    /// The earlier cell's first column, counted from 1.
    pub col: u64,
}

/// Condition `flm.cells.too-large`, an Error: a cell would make its table
/// larger than [`MAX_CELLS_POSITIONS`](crate::stdcore::backends::pres::MAX_CELLS_POSITIONS)
/// positions, its rows times its columns.
///
/// Recovery: the cell is left out whole, and a dropped mark is placed before
/// the table.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.cells.too-large",
    message = "a `cells` environment may have at most {limit} positions, its rows times its columns, \
               and this cell would make it {rows} rows by {cols} columns"
)]
pub struct TooLarge {
    /// The largest number of positions.
    pub limit: u64,
    /// The number of rows the table would have with the cell.
    pub rows: u64,
    /// The number of columns the table would have with the cell.
    pub cols: u64,
}

/// Condition `flm.cells.empty`, an Error: a `cells` environment places no
/// cell.
///
/// Recovery: nothing is written for the table, except the dropped marks of
/// what was left out.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.cells.empty", message = "a `cells` environment must hold at least one cell")]
pub struct Empty;

/// Condition `flm.cells.outside-table`, an Error: a `\cell`, `\celldata`,
/// `\\`, `&`, `\rownames` or `\colnames` reaches the lowering on its own,
/// outside the reader of a table: inside a cell's content, a `\celldata` or a
/// `\colnames` inside a `\celldata`'s data, an `&` or a `\\` nested inside
/// an entry's inline content.
///
/// Recovery: a command with content shows it where the command is written (a
/// `\cell`'s content as blocks, a `\celldata`'s data as inline content); a
/// command without content is left out, and a dropped mark is placed where
/// it was.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.cells.outside-table", message = "{detail}")]
pub struct OutsideTable {
    /// Where the command must be written, as a sentence
    /// ([`OUTSIDE_RULES`](super::lower::OUTSIDE_RULES)).
    pub detail: String,
}
