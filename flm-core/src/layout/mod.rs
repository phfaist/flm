//! The layout engine: it lays text out in lines of a given width, for any
//! output written as characters in fixed-width columns (plain text, a
//! terminal).
//!
//! The engine takes two inputs and produces one output:
//!
//! - **A layout document**, built by the caller in a [`DocBuf`]. It describes
//!   the intended text without deciding where the lines end: the words as
//!   *atoms* (texts printed whole), the places where a line may end and where
//!   it must end, indentation, per-line prefixes such as a quotation's `> `,
//!   verbatim blocks, tables, and *marks* (requests to report where a part of
//!   the document is printed). The constructors of [`DocBuf`] are the whole set
//!   of things a layout document can hold; the [`doc`] module lists them.
//! - **A profile** ([`Profile`]): the target line width in columns, or `None`
//!   for no wrapping. It is the engine's only option.
//! - **The output**: the printer, [`print()`] or [`print_full`], decides the
//!   line breaks for the profile's width and returns the text; [`print_full`]
//!   also returns, for every mark, the byte range and the line and column range
//!   of the marked part in that text ([`PrintOutput`], [`MarkSpan`]).
//!
//! An emitter (a backend's emit stage, which writes the output's bytes; see
//! [backends, lowering, and
//! outputs](crate::guide::concepts_overview#backends-lowering-and-outputs))
//! uses the engine when its output is text in columns: it builds one layout
//! document while it walks its input, then prints it once. In this library
//! the text backend's emitter ([`crate::stdlib::backends::text`]) is the only
//! caller of the printer; the HTML emitter and the presentation helpers use
//! [`is_breakable_space`] so that they split words at the same characters.
//! [The plain-text backend](crate::guide::rendering#the-plain-text-backend)
//! and [the layout engine](crate::guide::rendering#the-layout-engine) sections
//! of the rendering guide describe both.
//!
//! This module does not depend on anything else in the crate (neither on techy
//! nor on the document model), so any program that writes text in columns can
//! use it.
//!
//! # What the engine guarantees
//!
//! - **Breaking by width**: a [`group`](DocBuf::group) prints on one line if
//!   it fits together with everything that follows it on that line; otherwise
//!   every break opportunity directly inside it becomes a line break. Prose is
//!   filled with [`fill_words`](DocBuf::fill_words), which puts one group
//!   around each space between two words; each space then decides on its own
//!   whether to stay a space, which gives correct greedy wrapping, even before
//!   a trailing text that has no break opportunity of its own.
//! - **Display width by grapheme cluster**: the width of a text in columns is
//!   the sum, over its grapheme clusters (what a reader sees as one
//!   character), of each cluster's Unicode width. The width of two texts
//!   joined is therefore the sum of their widths, and a longer text never
//!   measures narrower than its beginning. `Über Bäume — 日本語` is 19 columns,
//!   a family emoji 2. [`display_width`] gives the same measure to an emitter
//!   that aligns text by hand.
//! - **One indentation stack**: [`nest`](DocBuf::nest),
//!   [`align`](DocBuf::align) and [`prefix`](DocBuf::prefix) all push units
//!   that are written at the start of every line, in nesting order, so a list
//!   inside a quotation and a quotation inside a list are both laid out
//!   correctly, and a prefix's width is subtracted from the width available
//!   for wrapping.
//! - **Blank lines only from [`blocksep`](DocBuf::blocksep)**: adjacent
//!   requests for a line break are merged into one, which keeps the larger
//!   number of blank lines; a line break at the start or at the end of the
//!   output is dropped; a blank line is written with the part of the
//!   indentation stack that the lines around it have in common, trimmed of
//!   trailing whitespace. The blank line between two paragraphs of one
//!   `> `-prefixed quotation therefore prints `>`, and the blank line between
//!   two such quotations prints nothing.
//! - **No trailing whitespace added by the engine**: indentation and the
//!   space of an unbroken break opportunity are written only when a content
//!   character follows on the same line.
//! - **[`raw`](DocBuf::raw) blocks are never reflowed**: their lines, blank
//!   lines included, are written byte for byte, either under the indentation
//!   stack or at column 0, as their [`RawPolicy`] says.
//! - **[`table`](DocBuf::table) columns are computed** from each column's
//!   narrowest and widest content: the width left over is shared out in
//!   proportion to each column's room to grow, in whole columns, after the
//!   gutters or the frame are subtracted from the width; cells are wrapped at
//!   their column's width and drawn as the table's [`TableFrame`] says: two
//!   spaces apart with a rule of `-` under the header rows (`Gutter`), or
//!   inside a frame of box-drawing characters with a rule of the frame under
//!   the header rows (`Box`). A table given a border mark reports what it
//!   draws under that mark, one span per run of drawn characters on a line.
//! - **[`mark`](DocBuf::mark) positions**: [`print_full`] reports, for every
//!   occurrence of a mark, the byte range and the 0-based line and column
//!   range of the marked part's content characters, in the order the marks
//!   are opened; a table's border runs and the marks inside its cells follow
//!   the marks opened before the table, the cells' marks row by row and
//!   column by column, each placed where its cell's text lands in the table.
//!   A mark inside a cell over several lines of the cell gives one span per
//!   line of the cell, because the table interleaves the cell's lines with
//!   the rest of the row. A mark never changes the layout.
//! - **Determinism**: the output depends only on the layout document, the root
//!   node passed to the printer and the profile. The printer uses no hash-map
//!   iteration and no floating point, and the record it keeps of the table
//!   cells it has already printed depends only on each cell and the width, so
//!   two runs give the same bytes.
//!
//! # Example
//!
//! ```
//! use flm_core::layout::{DocBuf, Profile, print};
//!
//! let mut b = DocBuf::new();
//! let label = b.text("  • ");
//! let body = b.fill_words("a list item whose text wraps under its own first word");
//! let hung = b.align(body);
//! let item = b.concat(&[label, hung]);
//!
//! let out = print(&b, item, &Profile::new(Some(32)));
//! assert_eq!(
//!     out,
//!     "  • a list item whose text wraps\n    under its own first word\n"
//! );
//! ```
//!
//! [`Profile`] carries the target width and nothing else: `None` means "never
//! wrap". Every other behavior is fixed: lines end in `\n`, a non-empty output
//! ends in exactly one `\n`, and an atom is never split. The width measure
//! cannot be changed either. The [`printer`] module's documentation explains
//! how the printer works.
//!
//! # Items of this module
//!
//! - [`doc`]: the arena [`DocBuf`] and its constructors; [`DocId`],
//!   [`MarkId`], [`RawPolicy`], [`ColSpec`], [`ColAlign`], [`TableCell`],
//!   [`is_breakable_space`].
//! - [`printer`]: [`Profile`], [`print()`], [`print_full`], [`PrintOutput`],
//!   [`MarkSpan`], [`LineCol`], and the printer itself.
//! - `table` (crate-internal): the column solver and the assembly of rows into
//!   lines.
//! - `width` (crate-internal): the width measures; [`display_width`] is
//!   re-exported here.
//!
//! # Properties of the interface
//!
//! - **The constructors of [`DocBuf`] are the whole public interface for
//!   building a layout document.** The nodes of the arena and the functions
//!   that read them are crate-internal: a caller only builds layout documents,
//!   and keeping their representation private leaves it free to change.
//! - **The width measure is public, and it cannot be replaced.** [`Profile`]
//!   has no setting for the measure. [`display_width`] is the measure the
//!   printer wraps against, offered for an emitter that aligns text by hand (a
//!   rule of `=` as wide as a heading, a label padded to a column).
//! - **The requirements on the constructors' input are checked in debug builds
//!   only.** An atom or a prefix must not contain `'\n'` or `'\t'`. These are
//!   requirements on the emitter's code, not on a document's content, and a
//!   violation has to be fixed in the emitter, so a release build does not
//!   spend time on the check.
//! - **A table that prints nothing is not a block.** A table with no columns,
//!   with no rows, or whose cells all print nothing leaves the surrounding line
//!   and any requested line break as they are, because output that does not
//!   exist should not force a line break. The fitting scan (the printer's test
//!   of whether a group fits on the line, see the [`printer`] module) agrees
//!   for the two cases it can decide without printing the cells (no columns, no
//!   rows); it still treats a table whose rows all print nothing as a block.
//!
//! # Known limitations
//!
//! - The fitting scan does not count indentation, so an indentation node
//!   inside a group at the start of a line is not counted against that group's
//!   first line. Emitters keep indentation outside groups, which avoids the
//!   case.
//! - A [`raw`](DocBuf::raw) block of one line is measured like an atom by the
//!   fitting scan, even under [`RawPolicy::Exempt`], where it starts a new
//!   line.
//! - The fitting scan stops once the width is used up by content characters,
//!   but it can pass through many nodes of zero width in a layout document
//!   whose nodes are shared many times; it has no limit on its number of
//!   steps.

// Maintainer notes on the points above. The trait behind the width measure
// stays crate-internal so that the choice of measure remains free to change.
// The limitations listed above are kept as they are, each pinned by a test, so
// that a deliberate change to the layout can be told apart from an accidental
// one. Correcting the indentation limitation is not local: the fitting scan
// would have to track the indentation accumulated up to a group's first
// visible character.
// The scan needs no step limit as long as emitters build layout documents as
// trees rather than as heavily shared graphs, since its walk is then
// proportional to the size of the document.

pub mod doc;
pub mod printer;
mod table;
mod width;

pub use doc::{
    is_breakable_space, ColAlign, ColSpec, DocBuf, DocId, MarkId, RawPolicy, TableCell, TableFrame,
};
pub use printer::{print, print_full, LineCol, MarkSpan, PrintOutput, Profile};
pub use width::display_width;
