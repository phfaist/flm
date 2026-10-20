//! The printer: it turns a layout document into text, and reports where each
//! marked part of the document was printed.
//!
//! [`print()`] and [`print_full`] take a layout document ([`DocBuf`]),
//! the id of its root node and a [`Profile`] (the target width). They return
//! the text; [`print_full`] also returns one [`MarkSpan`] per mark
//! ([`PrintOutput`]).
//!
//! The output depends only on the document, the root node and the profile:
//! the printer makes one pass over the document with one explicit work stack,
//! uses no hash-map iteration and no floating point, and keeps only a record
//! of the table cells it has already printed, which depends on nothing but
//! the cell and the width, so two runs give the same bytes. The printer calls itself only for
//! table cells, which it prints in separate runs.
//!
//! The rest of this documentation describes the five mechanisms that make up
//! the printer's behavior, for a reader who needs to predict the output
//! exactly or to maintain the printer. The names in code font below that no
//! link leads to (`Frame`, `Flat`, `Break`, `fits`, `IndentId`, `IndentUnit`,
//! `Spaces`, `Prefix`) are the printer's internal names, given for
//! maintainers; the lowercase ones (`group`, `line`, `nest`, …) are the
//! constructors of [`DocBuf`].
//!
//! # 1. The work stack and the two modes
//!
//! Each entry of the work stack is a `Frame`: the indentation stack that
//! applies, a *mode*, and the work to do (a node to print, or "close this
//! mark"). The mode says what a break opportunity prints: in `Flat` mode a
//! `line` prints as a space and a `softbreak` as nothing; in `Break` mode both
//! print as a line break. The root frame starts in `Break` mode when the
//! profile has a width, and in `Flat` mode when it has none ("no width" means
//! "never wrap", so a `line` outside any group prints as a space).
//!
//! A `group` reached in `Flat` mode stays flat: once a group has been found to
//! fit on the line, everything inside it is flat too, and nothing more is
//! tested. A `group` reached in `Break` mode is tested by the fitting scan of
//! section 2, and its content is printed in `Flat` mode if it fits, in `Break`
//! mode otherwise.
//!
//! # 2. The fitting scan, including what follows the group
//!
//! The fitting scan (`fits`) decides whether a group fits. It walks the
//! group's content in `Flat` mode, adding up widths against the room left on
//! the current line, **and then keeps walking the rest of the work stack**,
//! each remaining frame in the mode it was recorded with. It answers *yes* at
//! the first break opportunity it reaches in a `Break`-mode frame (the line
//! ends there in any case, so nothing after it can make this line too long),
//! or when the whole rest of the document has been walked. It answers *no* as
//! soon as the total exceeds the room left, or when it reaches an
//! unconditional break (`hardline`, `blocksep`, a `raw` block of several
//! lines, a table that has columns and rows) inside the group being tested, since such a group cannot
//! print on one line at all.
//!
//! Walking on past the group makes prose wrap correctly. Each space between
//! two words of [`fill_words`](super::DocBuf::fill_words) is its own group;
//! when the scan for the last such group reaches a trailing text after the
//! prose, a text with no break opportunity of its own, it finds that the word
//! *plus* that text would be too long, and the group breaks. A scan that
//! stopped at the group's end would instead keep the word on the line, and the
//! trailing text would make the line too long.
//!
//! **A group reached *during* the scan keeps the mode of the frame it came
//! from**: flat inside the group being tested, `Break` in the rest of the
//! work stack. A group further along the work stack has not been decided yet,
//! so its own break opportunities count as places where the line may end.
//! Prose filled word by word therefore looks exactly one word ahead, instead of
//! requiring that the rest of the paragraph fit.
//!
//! The scan does not count indentation: `nest`, `align` and `prefix` change
//! where *later* lines start, and the scan stops at the first line break, so
//! the scan's stack holds only `(mode, node)` pairs, and the indentation
//! affects the scan only through the current column, where it starts. This is
//! a known limitation: an indentation node inside a group at the start of a
//! line does change that group's first line, and the scan does not count it,
//! so `group(nest(4, ..))` printed right after a line break can be found to
//! fit when its indented first line does not. Emitters keep indentation
//! outside groups ([`fill_words`](super::DocBuf::fill_words) puts groups only
//! around single break opportunities), which avoids the case.
//!
//! # 3. The indentation stack
//!
//! A frame's indentation is not a number of columns but an `IndentId`: a
//! reference into a persistent stack of `IndentUnit`s whose stacks share
//! their common outer part. Each unit is either `Spaces(n)` (from `nest`, and
//! from `align`, which pads to the column where it starts) or a `Prefix` (from
//! `prefix`). At each line start, the stack is written in nesting order, so a
//! list inside a quotation gives `"> "` then the item's indentation, and a
//! quotation inside a list gives the item's indentation then `"> "`. The
//! printer stores the display width of each stack, and the width available
//! for wrapping is measured from it, so content inside `prefix("> ", ..)`
//! wraps two columns earlier.
//!
//! Because a line's indentation is written only when its first content
//! character is (section 4), the rule "a prefix applies to the current line
//! if it starts at a line start, and from the next line otherwise" needs no
//! special case: a prefix that starts in the middle of a line cannot change
//! the line's indentation, which has already been written. Adjacent `Spaces`
//! units are merged into one, so `nest(2, nest(3, ..))` uses one unit and
//! prints like `nest(5, ..)`; the merge keeps the outer unit, so it never
//! hides a prefix from the blank-line rule below.
//!
//! # 4. Writing a line only when content arrives: the pending break and the spaces not yet written
//!
//! The printer never writes whitespace ahead of time:
//!
//! - A request for a line break (`line` or `softbreak` in `Break` mode,
//!   `hardline`, `blocksep`) writes no newline. It records a *pending break*
//!   with a number of blank lines (`blocksep(n)` asks for `n`, everything else
//!   for `0`). A request made while another is pending is merged with it, and
//!   the larger number of blank lines is kept; the numbers are never added. So
//!   `hardline` next to `blocksep(1)` gives one blank line, and `blocksep(2)`
//!   next to `blocksep(1)` gives two.
//! - The indentation stack, and the space of a flat `line`, are written only
//!   when a content character follows on that line. A *content character* is
//!   a character of the layout document's own text that the printer writes,
//!   as opposed to the indentation, the prefixes, the spaces and the line
//!   breaks the printer adds.
//!
//! Therefore no line ends in whitespace the printer added; a pending break at
//! the very start of the output is dropped; a pending break at the very end is
//! replaced by exactly one final newline; and a blank line can only come from
//! `blocksep` or from a newline inside `raw` content. Runs of newlines cannot
//! arise, so no pass removes them afterwards.
//!
//! **What a blank line contains.** A blank line written for a pending break
//! holds the *longest common outer part* of two indentation stacks (the stack
//! that applied at the last content character and the stack that applies
//! where the break is written), trimmed of trailing whitespace. The two stacks
//! are compared by **identity of their units**, not by their text: each time
//! a `prefix` node is printed it pushes a unit of its own, so the common part
//! is exactly the units that the two stacks share. Consequences: a blank line
//! between two paragraphs inside one `prefix("> ")` prints `>`, so the two
//! paragraphs remain one quoted passage; a blank line between two *sibling*
//! `prefix("> ")` nodes prints nothing, so they remain two separate
//! quotations (the emitter decides whether two quotations are one, never the
//! printer); and stacks made only of spaces trim to an empty line.
//!
//! # 5. Marks
//!
//! A `mark` changes nothing in the output; it only records a position. Its
//! span **starts at the first content character of its child**, after any
//! pending break, its blank lines and the line's indentation, and **ends
//! immediately after the child's last content character**, before any space
//! or line break that is written later. The start is found when that
//! character is written: opening a mark only records it as waiting, and the
//! next content character gives every waiting mark its start. A prefix written
//! on a blank line (a `>` on a blank quoted line) is not content for this
//! purpose.
//!
//! A child that prints nothing gives an empty span placed at the next content
//! character or, if nothing follows, at the end of the text before the final
//! newline. Marks may be nested to any depth, have no width for the fitting
//! scan, and are reported in the order they are opened (the order in which
//! the printer reaches their nodes). Lines and columns are counted from 0,
//! columns are display columns, and the end position is excluded from the
//! span. A marked document and the same document without marks print the same
//! bytes.
//!
//! Known limitation: table cells are printed by separate runs of the printer,
//! and the marks inside them are not reported; a mark around a whole table is.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ops::Range;
use std::rc::Rc;

use super::doc::{ColSpec, DocBuf, DocId, MarkId, Node, PoolRange, RawPolicy, TableCell, TableDef};
use super::table::{self, RenderedCell};
use super::width::{UnicodeMeasure, WidthMeasure};

/// The printer's option: the target line width.
///
/// Every other behavior of the printer is fixed: lines end in `\n` (an
/// emitter that needs `\r\n` converts the text afterwards); a non-empty text
/// ends in exactly one newline; an atom wider than the line is written whole,
/// never split; and widths are measured by [`display_width`](super::display_width),
/// which counts the East-Asian *Ambiguous* characters (such as `…`, `→` and
/// `•`, which a terminal configured for Western languages draws one column
/// wide and one configured for East-Asian languages two) as one column.
///
/// The default profile (`Profile::default()`) has the width 80.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Profile {
    /// The target line width in columns, or `None` to never wrap.
    ///
    /// With `None`, every group prints flat (every `line` a space, every
    /// `softbreak` nothing) and the fitting scan never runs; this is a separate, faster path, not a very large width. The
    /// widths of atoms are still measured, because `align` needs the current
    /// column. `hardline` and `blocksep` still end lines, because they are
    /// part of the document's structure, and every table column gets its
    /// maximum width.
    pub width: Option<u32>,
}

impl Profile {
    /// Creates a profile with the target width `width` (`None` to never
    /// wrap).
    pub const fn new(width: Option<u32>) -> Self {
        Profile { width }
    }
}

impl Default for Profile {
    /// A profile with the width 80.
    fn default() -> Self {
        Profile { width: Some(80) }
    }
}

/// A position in the printed text: a line number and a display column on
/// that line, both counted from 0.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
pub struct LineCol {
    /// The line number, counted from 0.
    pub line: u32,
    /// The display column, counted from 0 and measured with
    /// [`display_width`](super::display_width).
    pub col: u32,
}

/// Where the child of one [`mark`](super::DocBuf::mark) was printed: the
/// mark's identifier, the byte range of the child's content characters in the
/// printed text, and their first and last line and column.
///
/// [`print_full`] returns one `MarkSpan` per occurrence of a mark. The end
/// positions are excluded from the span, so `bytes` slices
/// [`PrintOutput::text`] directly. The [module documentation](self), section
/// 5, gives the exact rules for where a span starts and ends.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct MarkSpan {
    /// The identifier the caller gave the mark.
    pub id: MarkId,
    /// The byte range of [`PrintOutput::text`] that holds the child's
    /// content characters.
    pub bytes: Range<usize>,
    /// The line and column of the child's first content character.
    pub start: LineCol,
    /// The line and column just after the child's last content character.
    pub end: LineCol,
}

/// The result of [`print_full`]: the printed text, and where each mark's
/// child was printed in it.
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct PrintOutput {
    /// The printed text: empty when the document prints nothing; otherwise it
    /// ends with exactly one `'\n'`.
    pub text: String,
    /// One span per occurrence of a mark, in the order the marks are opened
    /// (the order in which the printer reaches the `mark` nodes).
    pub marks: Vec<MarkSpan>,
}

/// Prints the layout document `buf` from its node `root`, at the width of
/// `profile`, and returns the text.
///
/// The result is the [`text`](PrintOutput::text) of [`print_full`] with the
/// same arguments; use [`print_full`] to know where marked parts were printed.
///
/// # Panics
///
/// `root` must have been created by `buf`, and not before a call to
/// [`DocBuf::clear`]: an id beyond the arena's nodes panics with an index
/// error. An id from another arena that is within range refers to the node at
/// that index instead, a caller error the engine cannot detect.
pub fn print(buf: &DocBuf, root: DocId, profile: &Profile) -> String {
    print_full(buf, root, profile).text
}

/// Prints the layout document `buf` from its node `root`, at the width of
/// `profile`, and returns the text together with one [`MarkSpan`] per mark
/// occurrence.
///
/// # Panics
///
/// As for [`print()`]: `root` must have been created by `buf`, and not before
/// a call to [`DocBuf::clear`].
pub fn print_full(buf: &DocBuf, root: DocId, profile: &Profile) -> PrintOutput {
    print_with_measure(buf, root, profile.width, &UnicodeMeasure)
}

/// Prints with the width measure `measure` instead of the engine's; for the
/// crate's own use.
///
/// It lets the tests check the printer under other measures, and would let
/// the crate select a measure that counts East-Asian *Ambiguous* characters as
/// two columns with a change of one line. Callers outside the crate use
/// [`print()`] and [`print_full`].
pub(crate) fn print_with_measure(
    buf: &DocBuf,
    root: DocId,
    width: Option<u32>,
    measure: &dyn WidthMeasure,
) -> PrintOutput {
    let cells = CellRenders::default();
    let mut printer = Printer::new(buf, measure, width, &cells);
    printer.run(root);
    printer.finish()
}

/// The table cells one print has printed, by cell and width.
///
/// A cell is printed by a separate run of the printer, at width 1 and with no
/// width limit to measure it, and at its column's width to show it; a table
/// inside a cell does the same for its own cells within each of those runs.
/// Without this record, the innermost cells of a table nested *n* deep would
/// be printed three to the *n*-th power times. A cell's printed text depends
/// only on the cell and the width (the separate run starts from an empty
/// indentation stack and reports no marks), so one printing serves every
/// later request for the same cell and width, and the output is the same.
// See decision rationale at [§flm-dd-dr:layout-table].
#[derive(Default)]
struct CellRenders(RefCell<BTreeMap<CellAtWidth, Rc<str>>>);

/// A table cell and the width it is printed at (`None` for no limit).
type CellAtWidth = (DocId, Option<u32>);

/// What a break opportunity prints in the frame from which the printer
/// reaches it.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    /// `line` is a space, `softbreak` is nothing.
    Flat,
    /// Both are line breaks.
    Break,
}

/// The work of one entry of the work stack.
#[derive(Clone, Copy, Debug)]
enum Work {
    /// Print this node.
    Doc(DocId),
    /// Close the mark at this index of the mark list (once the mark's child
    /// has been printed completely).
    CloseMark(u32),
}

/// One entry of the work stack.
#[derive(Clone, Copy, Debug)]
struct Frame {
    indent: IndentId,
    mode: Mode,
    work: Work,
}

/// One unit of the indentation stack.
#[derive(Clone, Copy, Debug)]
enum IndentUnit {
    /// Plain columns, from `nest` and from `align`'s padding.
    Spaces(u32),
    /// A per-line prefix: a range into the arena's text pool.
    Prefix(PoolRange),
}

/// A node of the persistent indentation stack. Stacks share their outer
/// part, so a frame holds only an `IndentId`, and two stacks are the same when
/// their ids are equal.
#[derive(Clone, Copy, Debug)]
struct IndentNode {
    unit: IndentUnit,
    parent: IndentId,
    /// The display width of the whole stack ending at this node, stored so
    /// that the width available for wrapping is not recomputed.
    width: u32,
}

/// Index into the printer's indentation-stack pool; [`IndentId::EMPTY`] is
/// the empty stack.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct IndentId(u32);

impl IndentId {
    const EMPTY: IndentId = IndentId(u32::MAX);
}

/// A position in the output text.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct Pos {
    byte: usize,
    line: u32,
    col: u32,
}

/// A mark being resolved.
struct MarkState {
    id: MarkId,
    start: Option<Pos>,
    end: Option<Pos>,
    /// The child printed no content character: the end must equal the
    /// start, wherever the start is eventually found.
    empty: bool,
}

struct Printer<'a> {
    buf: &'a DocBuf,
    measure: &'a dyn WidthMeasure,
    width: Option<u32>,

    out: String,
    stack: Vec<Frame>,
    indents: Vec<IndentNode>,

    /// The column the next content character would occupy on the current
    /// line, counting `pending_space`. Meaningful only while `line_open`.
    col: u32,
    /// Index of the line currently being written (0-based).
    line: u32,
    /// Whether the current line has been started (its indentation stack
    /// written).
    line_open: bool,
    /// Deferred spaces on the current line, from flat `line`s.
    pending_space: u32,
    /// A requested line break with this many blank lines after it, not yet
    /// written. `None` = no break pending.
    pending_break: Option<u8>,
    /// The indentation stack that applied at the last content character: one
    /// of the two stacks the blank-line rule compares.
    last_indent: IndentId,

    /// Marks, in open order.
    marks: Vec<MarkState>,
    /// The indices into `marks` of the marks whose start waits for the next
    /// content character.
    pending_starts: Vec<u32>,
    /// The position immediately after the last content character written.
    last_end: Pos,
    /// The table cells printed so far by this print and its separate runs.
    cells: &'a CellRenders,
}

impl<'a> Printer<'a> {
    fn new(
        buf: &'a DocBuf,
        measure: &'a dyn WidthMeasure,
        width: Option<u32>,
        cells: &'a CellRenders,
    ) -> Self {
        Printer {
            buf,
            measure,
            cells,
            width,
            out: String::new(),
            stack: Vec::new(),
            indents: Vec::new(),
            col: 0,
            line: 0,
            line_open: false,
            pending_space: 0,
            pending_break: None,
            last_indent: IndentId::EMPTY,
            marks: Vec::new(),
            pending_starts: Vec::new(),
            last_end: Pos::default(),
        }
    }

    // ========================================================================
    // the main loop
    // ========================================================================

    #[inline]
    fn push_doc(&mut self, indent: IndentId, mode: Mode, id: DocId) {
        self.stack.push(Frame {
            indent,
            mode,
            work: Work::Doc(id),
        });
    }

    fn run(&mut self, root: DocId) {
        // Root mode by width: an unbounded profile starts flat, so a bare
        // `line` outside any group is a space, not a break.
        let mode = if self.width.is_some() {
            Mode::Break
        } else {
            Mode::Flat
        };
        self.push_doc(IndentId::EMPTY, mode, root);

        let buf = self.buf;
        while let Some(f) = self.stack.pop() {
            let id = match f.work {
                Work::Doc(id) => id,
                Work::CloseMark(idx) => {
                    self.close_mark(idx);
                    continue;
                }
            };
            match buf.node(id) {
                Node::Text(range) => {
                    let s = buf.text_at(range);
                    if !s.is_empty() {
                        self.emit(f.indent, s);
                    }
                }
                Node::Line => match f.mode {
                    Mode::Flat => self.flat_space(),
                    Mode::Break => self.request_break(0),
                },
                Node::SoftBreak => match f.mode {
                    Mode::Flat => {}
                    Mode::Break => self.request_break(0),
                },
                Node::HardLine => self.request_break(0),
                Node::BlockSep { blank_lines } => self.request_break(blank_lines),
                Node::Nest { indent, child } => {
                    let id = self.push_spaces(f.indent, u32::from(indent));
                    self.push_doc(id, f.mode, child);
                }
                Node::Align { child } => {
                    // Pad the current stack out to the column reached at
                    // entry. Saturating: entering an `align` mid-line from
                    // inside a deeper stack (`text("a") ++ nest(50, align(..))`)
                    // can put the column behind the stack; the stack wins.
                    let pad = self
                        .column(f.indent)
                        .saturating_sub(self.stack_width(f.indent));
                    let id = self.push_spaces(f.indent, pad);
                    self.push_doc(id, f.mode, child);
                }
                Node::Prefix { prefix, child } => {
                    // Each *visit* pushes its own unit: scope identity, which
                    // is what the blank-line rule compares.
                    let id = self.push_unit(f.indent, IndentUnit::Prefix(prefix));
                    self.push_doc(id, f.mode, child);
                }
                Node::Group { child } => {
                    // Unbounded, or already inside a flattened group: flat
                    // with no scanning (the `||` short-circuits before the
                    // scan). Otherwise the fitting scan decides.
                    let flat = self.width.is_none()
                        || f.mode == Mode::Flat
                        || self.fits(f.indent, child);
                    let mode = if flat { Mode::Flat } else { Mode::Break };
                    self.push_doc(f.indent, mode, child);
                }
                Node::Concat(range) => {
                    // Push in reverse so that children pop in document order.
                    for &child in buf.children_at(range).iter().rev() {
                        self.push_doc(f.indent, f.mode, child);
                    }
                }
                Node::Raw { content, policy } => {
                    self.emit_raw(f.indent, buf.text_at(content), policy);
                }
                Node::Table { def } => {
                    self.emit_table(f.indent, buf.table_def(def));
                }
                Node::Mark { id: mark, child } => {
                    let idx = self.open_mark(mark);
                    // Close after the child: the stack is LIFO, so push the
                    // closer first.
                    self.stack.push(Frame {
                        indent: f.indent,
                        mode: f.mode,
                        work: Work::CloseMark(idx),
                    });
                    self.push_doc(f.indent, f.mode, child);
                }
            }
        }
    }

    // ========================================================================
    // marks
    // ========================================================================

    fn open_mark(&mut self, id: MarkId) -> u32 {
        let idx = self.marks.len() as u32;
        self.marks.push(MarkState {
            id,
            start: None,
            end: None,
            empty: false,
        });
        self.pending_starts.push(idx);
        idx
    }

    fn close_mark(&mut self, idx: u32) {
        let mark = &mut self.marks[idx as usize];
        if mark.start.is_some() {
            mark.end = Some(self.last_end);
        } else {
            // Nothing visible came out of the child: an empty span, anchored
            // wherever the start eventually resolves.
            mark.empty = true;
        }
    }

    /// The next content character is about to be written at `pos`: every
    /// mark still waiting for its start gets `pos`.
    fn resolve_pending_starts(&mut self, pos: Pos) {
        for &idx in &self.pending_starts {
            let mark = &mut self.marks[idx as usize];
            mark.start = Some(pos);
            if mark.empty {
                mark.end = Some(pos);
            }
        }
        self.pending_starts.clear();
    }

    /// Called immediately before content characters are written.
    fn note_visible_start(&mut self) {
        let pos = Pos {
            byte: self.out.len(),
            line: self.line,
            col: self.col,
        };
        self.resolve_pending_starts(pos);
    }

    /// Called immediately after content characters are written.
    fn note_visible_end(&mut self) {
        self.last_end = Pos {
            byte: self.out.len(),
            line: self.line,
            col: self.col,
        };
    }

    // ========================================================================
    // the indentation stack
    // ========================================================================

    #[inline]
    fn stack_width(&self, id: IndentId) -> u32 {
        if id == IndentId::EMPTY {
            0
        } else {
            self.indents[id.0 as usize].width
        }
    }

    fn unit_width(&self, unit: IndentUnit) -> u32 {
        match unit {
            IndentUnit::Spaces(n) => n,
            IndentUnit::Prefix(range) => self.measure.str_width(self.buf.text_at(range)),
        }
    }

    fn push_unit(&mut self, parent: IndentId, unit: IndentUnit) -> IndentId {
        let width = self.stack_width(parent).saturating_add(self.unit_width(unit));
        self.indents.push(IndentNode {
            unit,
            parent,
            width,
        });
        IndentId((self.indents.len() - 1) as u32)
    }

    /// Pushes `n` columns, merged with an adjacent `Spaces` unit, so that
    /// `nest(2, nest(3, ..))` and `nest(5, ..)` print the same and use one
    /// node. The merge keeps the outer unit's parent, so it never hides a
    /// `Prefix` unit from the blank-line rule, which compares units by
    /// identity; the merge changes only which `Spaces` unit is which, and
    /// spaces are trimmed from a blank line anyway.
    fn push_spaces(&mut self, parent: IndentId, n: u32) -> IndentId {
        if n == 0 {
            return parent;
        }
        if parent != IndentId::EMPTY {
            let node = self.indents[parent.0 as usize];
            if let IndentUnit::Spaces(m) = node.unit {
                return self.push_unit(node.parent, IndentUnit::Spaces(m.saturating_add(n)));
            }
        }
        self.push_unit(parent, IndentUnit::Spaces(n))
    }

    /// The ids along a stack, outermost first.
    fn chain(&self, id: IndentId) -> Vec<IndentId> {
        let mut v = Vec::new();
        let mut cur = id;
        while cur != IndentId::EMPTY {
            v.push(cur);
            cur = self.indents[cur.0 as usize].parent;
        }
        v.reverse();
        v
    }

    fn write_unit(&self, out: &mut String, unit: IndentUnit) {
        match unit {
            IndentUnit::Spaces(n) => {
                for _ in 0..n {
                    out.push(' ');
                }
            }
            IndentUnit::Prefix(range) => out.push_str(self.buf.text_at(range)),
        }
    }

    /// The full rendering of a stack, for a line start.
    fn stack_string(&self, id: IndentId) -> String {
        let mut s = String::new();
        for unit_id in self.chain(id) {
            self.write_unit(&mut s, self.indents[unit_id.0 as usize].unit);
        }
        s
    }

    /// The text of a blank line between two stacks: their longest common
    /// outer part (units compared by identity, not by text), written out and
    /// trimmed of trailing whitespace (see the module documentation, section
    /// 4).
    fn blank_line_text(&self, a: IndentId, b: IndentId) -> String {
        let ca = self.chain(a);
        let cb = self.chain(b);
        let mut s = String::new();
        for (x, y) in ca.iter().zip(cb.iter()) {
            if x != y {
                break;
            }
            self.write_unit(&mut s, self.indents[x.0 as usize].unit);
        }
        s.truncate(s.trim_end().len());
        s
    }

    // ========================================================================
    // line mechanics
    // ========================================================================

    /// The column at which the next content character will be written, when
    /// it is printed under the stack `indent`.
    ///
    /// If a break is pending, or the line has not been started yet, the next
    /// character is written right after the stack; otherwise at the current
    /// column.
    #[inline]
    fn column(&self, indent: IndentId) -> u32 {
        if self.pending_break.is_some() || !self.line_open {
            self.stack_width(indent)
        } else {
            self.col
        }
    }

    /// Requests a line break followed by `blanks` blank lines. A request made
    /// while another is pending is merged with it, keeping the larger number.
    #[inline]
    fn request_break(&mut self, blanks: u8) {
        self.pending_break = Some(match self.pending_break {
            Some(pending) => pending.max(blanks),
            None => blanks,
        });
    }

    /// A `line` printed flat: one space, written only when content follows.
    ///
    /// Dropped at a line start (it would be whitespace before the
    /// indentation, and trailing whitespace if the line then ended). The
    /// fitting scan still counts one column for such a `line`; that extra
    /// column can only make the scan break a line that would have fit.
    #[inline]
    fn flat_space(&mut self) {
        if self.pending_break.is_none() && self.line_open {
            self.pending_space += 1;
            self.col += 1;
        }
    }

    fn push_nl(&mut self) {
        self.out.push('\n');
        self.line += 1;
    }

    /// Writes the pending break, if any: the newline that ends the current
    /// line, then its blank lines (each holding the longest common outer part
    /// of the two stacks). The next line is left unstarted.
    fn flush_break(&mut self, indent: IndentId) {
        let Some(blanks) = self.pending_break.take() else {
            return;
        };
        self.pending_space = 0;
        if !self.out.is_empty() {
            // A break at the very start of the output is dropped.
            self.push_nl();
            if blanks > 0 {
                let blank = self.blank_line_text(self.last_indent, indent);
                for _ in 0..blanks {
                    self.out.push_str(&blank);
                    self.push_nl();
                }
            }
        }
        self.line_open = false;
        self.col = 0;
    }

    /// Starts the current line by writing its indentation stack.
    fn open_line(&mut self, indent: IndentId) {
        debug_assert!(self.pending_break.is_none());
        if !self.line_open {
            let s = self.stack_string(indent);
            self.out.push_str(&s);
            self.col = self.stack_width(indent);
            self.line_open = true;
        }
    }

    /// Writes a line that has no content: the stack, trimmed of trailing
    /// whitespace (`"> "` keeps its `">"`; indentation made only of spaces
    /// disappears).
    ///
    /// The stack written on such a line is not content for marks: a mark
    /// waiting for its start gets the next content character instead.
    fn open_blank_line(&mut self, indent: IndentId) {
        debug_assert!(self.pending_break.is_none());
        if !self.line_open {
            let s = self.stack_string(indent);
            let t = &s[..s.trim_end().len()];
            self.out.push_str(t);
            self.col = self.measure.str_width(t);
            self.line_open = true;
        }
    }

    /// Writes a newline that is part of the content, as *inside* `raw`
    /// content and between the lines of a table. It is never merged with a
    /// pending break.
    fn hard_nl(&mut self) {
        self.push_nl();
        self.line_open = false;
        self.col = 0;
        self.pending_space = 0;
    }

    fn flush_spaces(&mut self) {
        for _ in 0..self.pending_space {
            self.out.push(' ');
        }
        self.pending_space = 0;
    }

    /// Writes content characters, starting the line first if needed.
    ///
    /// Atoms are never split: an atom wider than the room left is written
    /// whole, and the line is longer than the width. Lines break only at
    /// break opportunities.
    fn emit(&mut self, indent: IndentId, s: &str) {
        self.flush_break(indent);
        self.open_line(indent);
        self.flush_spaces();
        self.note_visible_start();
        self.out.push_str(s);
        self.col += self.measure.str_width(s);
        self.note_visible_end();
        self.last_indent = indent;
    }

    /// Ends the print: a pending break at the end of the output is dropped, as
    /// is any space not yet written; a non-empty output ends with exactly one
    /// `'\n'`.
    fn finish(mut self) -> PrintOutput {
        // Marks never resolved: anchor them at the end of the content, before
        // the final newline.
        let end = self.last_end;
        self.resolve_pending_starts(end);
        for mark in &mut self.marks {
            if mark.end.is_none() {
                mark.end = Some(end);
            }
        }
        if !self.out.is_empty() && !self.out.ends_with('\n') {
            self.out.push('\n');
        }
        let marks = self
            .marks
            .iter()
            .map(|mark| {
                let s = mark.start.unwrap_or_default();
                let e = mark.end.unwrap_or_default();
                MarkSpan {
                    id: mark.id,
                    bytes: s.byte..e.byte,
                    start: LineCol {
                        line: s.line,
                        col: s.col,
                    },
                    end: LineCol {
                        line: e.line,
                        col: e.col,
                    },
                }
            })
            .collect();
        PrintOutput {
            text: self.out,
            marks,
        }
    }

    // ========================================================================
    // raw
    // ========================================================================

    /// Writes verbatim content.
    ///
    /// Newlines inside the content are written as they are, so runs of blank
    /// lines inside a code block are kept exactly. A newline at the *end* of
    /// the content becomes a pending break instead of being written, so that
    /// a following `blocksep` is merged with it (the larger number of blank
    /// lines kept) rather than adding a blank line.
    fn emit_raw(&mut self, indent: IndentId, content: &str, policy: RawPolicy) {
        if content.is_empty() {
            return;
        }
        // Verbatim-exempt content escapes the scope entirely, including for
        // the blank lines of the break being flushed.
        let scope = match policy {
            RawPolicy::Indented => indent,
            RawPolicy::Exempt => IndentId::EMPTY,
        };
        self.flush_break(scope);
        if policy == RawPolicy::Exempt && self.line_open {
            // Exempt raw must start at column 0, so it ends any open line.
            self.hard_nl();
        }

        let mut segments = content.split('\n').peekable();
        let mut first = true;
        while let Some(seg) = segments.next() {
            let last = segments.peek().is_none();
            if !first {
                if last && seg.is_empty() {
                    // Trailing newline: hand it to the pending-break
                    // mechanism.
                    self.request_break(0);
                    break;
                }
                self.hard_nl();
            }
            first = false;
            if seg.is_empty() {
                match policy {
                    RawPolicy::Indented => self.open_blank_line(indent),
                    // Truly empty; mark the line materialized so that the next
                    // `hard_nl` terminates it.
                    RawPolicy::Exempt => self.line_open = true,
                }
            } else {
                match policy {
                    RawPolicy::Indented => self.open_line(indent),
                    RawPolicy::Exempt => {
                        if !self.line_open {
                            self.line_open = true;
                            self.col = 0;
                        }
                    }
                }
                self.flush_spaces();
                self.note_visible_start();
                self.out.push_str(seg);
                // Best effort: the column after a raw that does not end in a
                // newline is measured from its last line with the engine
                // measure (tabs included, whatever the measure says they
                // are).
                self.col += self.measure.str_width(seg);
                self.note_visible_end();
            }
        }
        self.last_indent = scope;
    }

    // ========================================================================
    // tables
    // ========================================================================

    /// Prints `cell` on its own, at `width`, with this printer's measure and
    /// an empty indentation stack (the table's own stack applies to the
    /// assembled lines). **Marks inside the cell are not reported.**
    ///
    /// Each printing of a table asks for each of its cells at three widths
    /// (width 1, no limit, and the column's computed width), and each cell is
    /// printed once per width for the whole print: the text is recorded in
    /// [`CellRenders`], and a later request at the same width reads the
    /// record, so a nested table does not multiply the printings of its
    /// innermost cells by three per level.
    fn render_cell(&self, cell: DocId, width: Option<u32>) -> Rc<str> {
        if let Some(done) = self.cells.0.borrow().get(&(cell, width)) {
            return Rc::clone(done);
        }
        let mut printer = Printer::new(self.buf, self.measure, width, self.cells);
        printer.run(cell);
        let text: Rc<str> = Rc::from(printer.finish().text);
        self.cells.0.borrow_mut().insert((cell, width), Rc::clone(&text));
        text
    }

    fn max_line_width(&self, s: &str) -> u32 {
        s.lines()
            .map(|l| self.measure.str_width(l))
            .max()
            .unwrap_or(0)
    }

    /// A cell's **minimum** width: the width below which breaking more lines
    /// makes the cell no narrower. It is computed as the widest line of the
    /// cell printed at width 1 (every break opportunity broken), which is the
    /// widest atom (or raw line) plus the indentation around it.
    fn cell_min(&self, cell: DocId) -> u32 {
        self.max_line_width(&self.render_cell(cell, Some(1)))
    }

    /// A cell's **maximum** width: the widest line of the cell printed with no
    /// width limit, where only its unconditional breaks (`hardline`,
    /// `blocksep`, newlines of raw content) end lines.
    fn cell_max(&self, cell: DocId) -> u32 {
        self.max_line_width(&self.render_cell(cell, None))
    }

    /// Lays out and writes a table: measure the cells, compute the column
    /// widths, print the cells, assemble the lines, write them.
    ///
    /// A table is a block on both sides: it requests a line break before and
    /// after itself, and every assembled line is written under the table's
    /// indentation stack, so a table inside a list or a quotation is indented
    /// and prefixed correctly. A table that prints nothing (no columns, no
    /// rows, or only empty cells) is not a block: it leaves the current line
    /// and any pending break as they are (see the [`layout`](super) module
    /// documentation).
    fn emit_table(&mut self, indent: IndentId, def: TableDef) {
        let buf = self.buf;
        let cols: Vec<ColSpec> = buf.cols_at(def.cols).to_vec();
        let ncols = cols.len();
        if ncols == 0 {
            return;
        }
        let rows: Vec<PoolRange> = buf.rows_at(def.rows).to_vec();

        // -- measure ---------------------------------------------------------
        let mut mins = vec![0u32; ncols];
        let mut maxs = vec![0u32; ncols];
        let mut grid: Vec<Vec<Option<TableCell>>> = Vec::with_capacity(rows.len());
        for &row_range in &rows {
            let cells = buf.cells_at(row_range);
            let mut row = vec![None; ncols];
            for (i, &cell) in cells.iter().take(ncols).enumerate() {
                row[i] = Some(cell);
                mins[i] = mins[i].max(self.cell_min(cell.doc));
                maxs[i] = maxs[i].max(self.cell_max(cell.doc));
            }
            grid.push(row);
        }
        // A cell whose indentation grows when it breaks can measure "wider
        // when narrow"; keep max >= min so that the solver's spans stay sane.
        for (max, &min) in maxs.iter_mut().zip(&mins) {
            *max = (*max).max(min);
        }

        // -- solve -----------------------------------------------------------
        let gutters = table::GUTTER_WIDTH * (ncols as u32 - 1);
        let available = self.width.map(|w| {
            w.saturating_sub(self.stack_width(indent))
                .saturating_sub(gutters)
        });
        let widths = table::solve_columns(&mins, &maxs, available);

        // -- render cells ----------------------------------------------------
        let mut rendered: Vec<Vec<RenderedCell>> = Vec::with_capacity(grid.len());
        for row in &grid {
            let mut out_row = Vec::with_capacity(ncols);
            for (i, cell) in row.iter().enumerate() {
                // With an unbounded profile every column already has its max
                // width; render the cell unbounded too, so that the assignment
                // is exactly the unbounded layout.
                let cell_width = self.width.map(|_| widths[i]);
                let lines: Vec<String> = match cell {
                    None => Vec::new(),
                    Some(cell) => {
                        let s = self.render_cell(cell.doc, cell_width);
                        let s = s.strip_suffix('\n').unwrap_or(&s);
                        if s.is_empty() {
                            Vec::new()
                        } else {
                            s.split('\n').map(|l| l.to_string()).collect()
                        }
                    }
                };
                // A cell's own alignment replaces its column's.
                let align = cell.and_then(|cell| cell.align).unwrap_or(cols[i].align);
                out_row.push(RenderedCell { lines, align });
            }
            rendered.push(out_row);
        }

        // -- assemble --------------------------------------------------------
        let lines = table::assemble_lines(&rendered, &widths, def.header_rows, self.measure);

        // -- emit ------------------------------------------------------------
        // A table that prints nothing is not a block: it leaves the current
        // line and any pending break untouched. See design rationale at
        // [§flm-dd-dr:layout-empty-table].
        if lines.is_empty() {
            return;
        }
        // Otherwise a table is a block: it starts on its own line and ends
        // one.
        if self.line_open {
            self.request_break(0);
        }
        self.flush_break(indent);
        for (k, line) in lines.iter().enumerate() {
            if k > 0 {
                self.hard_nl();
            }
            if line.is_empty() {
                self.open_blank_line(indent);
            } else {
                self.open_line(indent);
                self.note_visible_start();
                self.out.push_str(line);
                self.col += self.measure.str_width(line);
                self.note_visible_end();
            }
        }
        self.last_indent = indent;
        self.request_break(0);
    }

    // ========================================================================
    // the fitting scan
    // ========================================================================

    /// Returns whether `child`, printed flat under `indent`, keeps the rest of
    /// the current line within the width. The scan continues through the rest
    /// of the work stack; see the [module documentation](self), section 2.
    fn fits(&self, indent: IndentId, child: DocId) -> bool {
        let Some(max) = self.width else {
            return true;
        };
        // Room left on the current line. Saturating: if the line is already
        // past the width (an overflowing atom), nothing further fits.
        let mut rem = i64::from(max.saturating_sub(self.column(indent)));

        let buf = self.buf;
        // Indentation is inert here, so the scan stack is (mode, node) only.
        let mut scan: Vec<(Mode, DocId)> = vec![(Mode::Flat, child)];
        // Index into the work stack, walked from the top downwards once the
        // group's own content is exhausted.
        let mut cont = self.stack.len();

        loop {
            let (mode, id) = match scan.pop() {
                Some(item) => item,
                None => {
                    if cont == 0 {
                        // Nothing left in the document: the line ends here.
                        return true;
                    }
                    cont -= 1;
                    let f = self.stack[cont];
                    match f.work {
                        Work::Doc(id) => (f.mode, id),
                        // Mark closers have no width and must be stepped
                        // over, or the continuation walk stops short.
                        Work::CloseMark(_) => continue,
                    }
                }
            };
            match buf.node(id) {
                Node::Text(range) => {
                    rem -= i64::from(self.measure.str_width(buf.text_at(range)));
                    if rem < 0 {
                        return false;
                    }
                }
                Node::Line => match mode {
                    Mode::Flat => {
                        rem -= 1;
                        if rem < 0 {
                            return false;
                        }
                    }
                    // The line ends here regardless of this group's decision.
                    Mode::Break => return true,
                },
                Node::SoftBreak => match mode {
                    Mode::Flat => {}
                    Mode::Break => return true,
                },
                Node::HardLine | Node::BlockSep { .. } => match mode {
                    // A hard break inside the candidate group: it cannot be
                    // flattened.
                    Mode::Flat => return false,
                    // A hard break after it: the line ends there.
                    Mode::Break => return true,
                },
                Node::Nest { child, .. }
                | Node::Align { child }
                | Node::Prefix { child, .. }
                | Node::Mark { child, .. } => scan.push((mode, child)),
                // The load-bearing line: a group keeps the mode of the frame
                // it came from. Content of the candidate
                // group is flat, whereas a group further along the work stack
                // is still undecided and its own break opportunities count as
                // real ones. This is what makes per-word fill look exactly one
                // word ahead instead of demanding that the whole rest of the
                // paragraph fit.
                Node::Group { child } => scan.push((mode, child)),
                Node::Concat(range) => {
                    for &c in buf.children_at(range).iter().rev() {
                        scan.push((mode, c));
                    }
                }
                Node::Raw { content, .. } => {
                    let s = buf.text_at(content);
                    match s.find('\n') {
                        // Single-line raw measures like an atom.
                        None => {
                            rem -= i64::from(self.measure.str_width(s));
                            if rem < 0 {
                                return false;
                            }
                        }
                        Some(nl) => match mode {
                            // Multi-line raw is hard structure, like a
                            // hardline.
                            Mode::Flat => return false,
                            Mode::Break => {
                                rem -= i64::from(self.measure.str_width(&s[..nl]));
                                return rem >= 0;
                            }
                        },
                    }
                }
                Node::Table { def } => {
                    // A table is a block — unless it prints nothing. The two
                    // shapes decidable without rendering, no columns and no
                    // rows, count as zero width and force no break; a table
                    // whose rows all render empty is still a block to the scan.
                    // See design rationale at [§flm-dd-dr:layout-empty-table].
                    let def = buf.table_def(def);
                    let prints_nothing = buf.cols_at(def.cols).is_empty()
                        || buf.rows_at(def.rows).is_empty();
                    if !prints_nothing {
                        match mode {
                            Mode::Flat => return false,
                            Mode::Break => return true,
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    //! Tests that need to choose the width measure, which only crate-internal
    //! code can do. The rest of the printer's tests are integration tests of
    //! the crate.

    use super::super::doc::DocBuf;
    use super::super::width::UnicodeCjkMeasure;
    use super::{print, print_with_measure, Profile};

    /// The width measure is not a cosmetic setting: it moves break points *and*
    /// hanging-indent columns. U+2022 BULLET is East-Asian Ambiguous, so the
    /// same document hangs at column 4 under the shipped (ambiguous-narrow)
    /// measure and column 5 under an ambiguous-wide one.
    ///
    /// `Profile` exposes only the width; the measure is reached through the
    /// crate-internal print function, which is what an ambiguous-wide profile
    /// would change.
    #[test]
    fn the_measure_changes_the_layout() {
        let mut b = DocBuf::new();
        let bullet = b.text("  • ");
        let body = b.fill_words("alpha beta gamma");
        let hung = b.align(body);
        let d = b.concat(&[bullet, hung]);

        let narrow = print(&b, d, &Profile::new(Some(14)));
        let wide = print_with_measure(&b, d, Some(14), &UnicodeCjkMeasure).text;
        for out in [&narrow, &wide] {
            for line in out.split('\n') {
                assert_eq!(line.trim_end(), line, "trailing whitespace in {out:?}");
            }
        }

        assert_eq!(narrow, "  • alpha beta\n    gamma\n");
        // One extra column of label pushes both the hanging indent and every
        // break.
        assert_eq!(wide, "  • alpha\n     beta\n     gamma\n");
    }
}
