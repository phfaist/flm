//! The layout document: the description of intended text that an emitter
//! builds and hands to the printer.
//!
//! A *layout document* describes output text without deciding where the lines
//! end. It holds the words as text atoms, the places where a line *may* end
//! ([`line`](DocBuf::line), [`softbreak`](DocBuf::softbreak)), the places
//! where a line *must* end ([`hardline`](DocBuf::hardline),
//! [`blocksep`](DocBuf::blocksep)), and the structure that sets indentation,
//! per-line prefixes, verbatim blocks and tables. The printer
//! ([`super::print`]) decides the line breaks for a given width.
//!
//! An emitter creates a [`DocBuf`], calls its constructors from the innermost
//! parts outwards (each returns the [`DocId`] of the node it creates), and
//! hands the id of the outermost node, the root, to the printer.
//!
//! # The constructors
//!
//! The constructors of [`DocBuf`] are the only way to build a layout
//! document; every one but [`fill_words`](DocBuf::fill_words) creates one kind
//! of node:
//!
//! | constructor | meaning |
//! |---|---|
//! | [`text`](DocBuf::text) | an atom: printed whole, never split, never changed |
//! | [`line`](DocBuf::line) | a break opportunity: one space when it does not break |
//! | [`softbreak`](DocBuf::softbreak) | a break opportunity: nothing when it does not break |
//! | [`hardline`](DocBuf::hardline) | an unconditional line break |
//! | [`blocksep`](DocBuf::blocksep) | a line break followed by *n* blank lines |
//! | [`nest`](DocBuf::nest) | indent the child by a fixed number of columns |
//! | [`align`](DocBuf::align) | indent the child to the column where it starts (a hanging indent) |
//! | [`group`](DocBuf::group) | a unit whose break opportunities all stay unbroken (flat), or all break |
//! | [`concat`](DocBuf::concat) | a sequence |
//! | [`prefix`](DocBuf::prefix) | write a string before every line of the child (`> `) |
//! | [`raw`](DocBuf::raw) | verbatim lines, never reflowed |
//! | [`table`](DocBuf::table) | rows of cells laid out in computed columns |
//! | [`mark`](DocBuf::mark) | ask the printer to report where the child is printed |
//! | [`fill_words`](DocBuf::fill_words) | prose: the words of a text, with one group around each space between two words |
//!
//! Each constructor's documentation states how the printer treats its node;
//! the [printer](super::printer)'s module documentation explains the
//! mechanisms the constructors share.
//!
//! # Representation: a flat arena
//!
//! A [`DocBuf`] is a set of growable pools and nothing else:
//!
//! - `nodes`: every node, as one fixed-size `Node` value;
//! - `text`: every text atom, every line prefix and every raw block, copied
//!   into one shared `String`; a node holds only a byte range into it;
//! - `children`: the children of every sequence, appended into one shared
//!   pool; a `Concat` node holds only a range into it;
//! - `rows`, `cells`, `cols`, `tables`: the data of table nodes, held outside
//!   the nodes.
//!
//! Building a document therefore makes no heap allocation per node, and a
//! [`DocId`] is a plain `u32`, cheap to copy onto the printer's work stack.
//! [`DocBuf::clear`] keeps the allocated capacity, so one arena can be reused
//! for one render after another.
//!
//! Ids are given out bottom-up: a node's children exist before the node
//! itself, so the arena cannot contain a cycle. A node may be shared: the same
//! [`DocId`] may appear in several sequences, and the printer prints it at
//! each occurrence (the arena is a directed acyclic graph, not a tree).

use std::ops::Range;

/// The identifier of a node of one [`DocBuf`]: its index in the arena.
///
/// Every constructor of [`DocBuf`] returns the `DocId` of the node it creates.
/// A `DocId` is meaningful only for the arena that created it. Used with
/// another arena, it refers to the node at the same index there, or makes
/// the printer panic if the index is out of range (see [`print`](super::print)).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct DocId(u32);

impl DocId {
    /// The index of this node in its arena, for debugging output only.
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

/// The identifier of a [`mark`](DocBuf::mark), chosen by the caller.
///
/// The engine does not interpret the number and does **not** require it to
/// be unique: an emitter may mark several nodes with the same id, and
/// [`print_full`](super::print_full) reports one [`MarkSpan`](super::MarkSpan)
/// per marked node, each carrying the id. The emitter keeps its own table from
/// ids to the parts of its input that the marks refer to (the text backend's
/// emitter maps them to lowered nodes), so this module does not depend on the
/// document model.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct MarkId(pub u32);

/// Whether the enclosing indentation and prefixes apply to the lines of a
/// [`raw`](DocBuf::raw) block.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RawPolicy {
    /// Every raw line is written after the current indentation stack, so a
    /// code block inside a list item or a quotation gets the item's indentation
    /// or the `> ` on every line, and its own bytes are unchanged. The text
    /// backend uses this policy.
    Indented,
    /// Every raw line starts at column 0 and the indentation stack is not
    /// written at all, the way a LaTeX `verbatim` environment behaves. No
    /// backend of this library uses this policy.
    Exempt,
}

/// Horizontal alignment of the cells in one table column.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ColAlign {
    /// Cells start at the column's left edge; padding goes on the right.
    Left,
    /// Cells end at the column's right edge; padding goes on the left.
    Right,
    /// Padding is split between both sides; when it is an odd number of
    /// columns, the extra column goes on the right.
    Center,
}

/// The specification of one column of a [table](DocBuf::table): the
/// alignment of its cells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ColSpec {
    /// How the column's cells are aligned within the column's computed width.
    pub align: ColAlign,
}

impl ColSpec {
    /// A left-aligned column.
    pub fn left() -> Self {
        ColSpec {
            align: ColAlign::Left,
        }
    }

    /// A right-aligned column.
    pub fn right() -> Self {
        ColSpec {
            align: ColAlign::Right,
        }
    }

    /// A centered column.
    pub fn center() -> Self {
        ColSpec {
            align: ColAlign::Center,
        }
    }
}

/// One cell of a [table](DocBuf::table): its content, a node of the same
/// arena, and optionally an alignment of its own.
///
/// A cell with no alignment of its own is aligned as its column's [`ColSpec`]
/// says. A cell with one is padded to the column's computed width in that
/// alignment instead, regardless of the other cells of the column. The
/// alignment changes only where the cell's lines are placed inside the
/// column: the column's width is computed from its cells' content alone.
///
/// A [`DocId`] converts into a cell with no alignment of its own, so the rows
/// of a table whose cells all follow their columns can be given as ids.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct TableCell {
    /// The cell's content: any node of the same arena.
    pub doc: DocId,
    /// The cell's own alignment; `None` uses the column's.
    pub align: Option<ColAlign>,
}

impl TableCell {
    /// A cell whose content is `doc`, aligned as its column is.
    pub fn new(doc: DocId) -> Self {
        TableCell { doc, align: None }
    }

    /// A cell whose content is `doc`, aligned as `align` says, regardless of
    /// its column.
    pub fn aligned(doc: DocId, align: ColAlign) -> Self {
        TableCell {
            doc,
            align: Some(align),
        }
    }
}

impl From<DocId> for TableCell {
    fn from(doc: DocId) -> Self {
        TableCell::new(doc)
    }
}

/// A range of bytes or of entries in one of the pools of a [`DocBuf`].
///
/// The node that holds the range determines which pool it refers to; the
/// reading methods of [`DocBuf`] return what the range covers.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct PoolRange {
    start: u32,
    end: u32,
}

impl PoolRange {
    #[inline]
    fn as_range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

/// The `u32` form of a pool length, checked against the arena's documented
/// limit (see the `# Panics` section of [`DocBuf`]).
#[inline]
fn pool_len(len: usize, pool: &str) -> u32 {
    assert!(
        len <= u32::MAX as usize,
        "the {pool} pool of the layout document must hold at most u32::MAX entries (bytes, \
         for the text pool)"
    );
    len as u32
}

/// One node of the arena, as the printer reads it.
///
/// Every variant is `Copy` and holds no pointer; its data are ranges into the
/// pools of the [`DocBuf`] that holds it, read through that arena's reading
/// methods. Callers build nodes through the constructors of [`DocBuf`]; this
/// enum is the printer's input and is crate-internal, so that the arena's
/// representation can change.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Node {
    /// An atom (see [`DocBuf::text`]): a range into the text pool.
    Text(PoolRange),
    /// A break opportunity that is one space when it does not break.
    Line,
    /// A break opportunity that is nothing when it does not break.
    SoftBreak,
    /// An unconditional line break.
    HardLine,
    /// Indent `child` by `indent` extra columns.
    Nest { indent: u16, child: DocId },
    /// Indent `child` to the column reached when it is entered.
    Align { child: DocId },
    /// A unit whose break opportunities all break or all stay flat.
    Group { child: DocId },
    /// A sequence: a range into the children pool.
    Concat(PoolRange),
    /// A line break followed by `blank_lines` blank lines.
    BlockSep { blank_lines: u8 },
    /// A prefix written before every line of `child`; `prefix` is a range
    /// into the text pool.
    Prefix { prefix: PoolRange, child: DocId },
    /// Verbatim content. `content` is a range into the text pool and is the
    /// only text that may contain `'\n'` and `'\t'`.
    Raw { content: PoolRange, policy: RawPolicy },
    /// A table; `def` is its index in the arena's table pool.
    Table { def: u32 },
    /// A mark around `child`.
    Mark { id: MarkId, child: DocId },
}

/// The data of a [`Node::Table`], held in the arena's table pool.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) struct TableDef {
    /// A range into the rows pool; each row is itself a range into the cells
    /// pool, holding that row's cells. Rows may have different numbers of
    /// cells.
    pub(crate) rows: PoolRange,
    /// A range into the column pool.
    pub(crate) cols: PoolRange,
    /// The number of leading rows that form the header; a rule is drawn
    /// after them. `0` means no header.
    pub(crate) header_rows: u8,
}

/// A layout document: an arena of layout nodes, with its pools of text,
/// children, rows, cells, columns and tables.
///
/// An emitter builds a layout document by calling the constructors
/// ([`text`](Self::text), [`line`](Self::line), [`group`](Self::group), ...),
/// innermost parts first. Each returns the [`DocId`] of the node it creates,
/// which the emitter passes to the constructors of the enclosing nodes; the
/// id of the outermost node is the root that [`print`](super::print) and
/// [`print_full`](super::print_full) take. The [module documentation](self)
/// lists the constructors and describes the representation.
///
/// # Requirements
///
/// An id is meaningful only for the arena that created it, and
/// [`clear`](Self::clear) makes every id given out before it meaningless;
/// giving such an id to the printer is a caller error (see
/// [`print`](super::print)).
///
/// # Panics
///
/// Every constructor panics if the arena or one of its pools would grow past
/// `u32::MAX` entries (about four billion nodes, children, rows, cells,
/// columns or tables, or four gigabytes of text), a size no document reaches;
/// the panic message gives the pool.
#[derive(Clone, Default, Debug)]
pub struct DocBuf {
    nodes: Vec<Node>,
    text: String,
    children: Vec<DocId>,
    rows: Vec<PoolRange>,
    cells: Vec<TableCell>,
    cols: Vec<ColSpec>,
    tables: Vec<TableDef>,
}

impl DocBuf {
    /// Creates an empty layout document.
    pub fn new() -> Self {
        Self::default()
    }

    /// Removes every node and pool entry and keeps the allocated capacity, so
    /// that the arena can be reused for the next render. Every [`DocId`] given
    /// out before the call becomes meaningless.
    pub fn clear(&mut self) {
        self.nodes.clear();
        self.text.clear();
        self.children.clear();
        self.rows.clear();
        self.cells.clear();
        self.cols.clear();
        self.tables.clear();
    }

    /// The number of nodes in the arena.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the arena holds no nodes.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    // -- reader side (the printer) -------------------------------------------

    /// The node an id refers to.
    #[inline]
    pub(crate) fn node(&self, id: DocId) -> Node {
        self.nodes[id.0 as usize]
    }

    /// The text a range of the text pool covers (an atom, a prefix, a raw
    /// block).
    #[inline]
    pub(crate) fn text_at(&self, range: PoolRange) -> &str {
        &self.text[range.as_range()]
    }

    /// The children a range of the children pool covers (a sequence's
    /// children).
    #[inline]
    pub(crate) fn children_at(&self, range: PoolRange) -> &[DocId] {
        &self.children[range.as_range()]
    }

    /// The data of the table at index `def` of the table pool.
    #[inline]
    pub(crate) fn table_def(&self, def: u32) -> TableDef {
        self.tables[def as usize]
    }

    /// The rows of a table, each a range into the cells pool.
    #[inline]
    pub(crate) fn rows_at(&self, range: PoolRange) -> &[PoolRange] {
        &self.rows[range.as_range()]
    }

    /// The cells of one table row.
    #[inline]
    pub(crate) fn cells_at(&self, range: PoolRange) -> &[TableCell] {
        &self.cells[range.as_range()]
    }

    /// The column specifications of a table.
    #[inline]
    pub(crate) fn cols_at(&self, range: PoolRange) -> &[ColSpec] {
        &self.cols[range.as_range()]
    }

    // -- builder side --------------------------------------------------------

    #[inline]
    fn push(&mut self, node: Node) -> DocId {
        let id = pool_len(self.nodes.len(), "node");
        self.nodes.push(node);
        DocId(id)
    }

    /// Copies `s` into the shared text pool and returns its byte range.
    fn intern(&mut self, s: &str) -> PoolRange {
        let start = pool_len(self.text.len(), "text");
        self.text.push_str(s);
        let end = pool_len(self.text.len(), "text");
        PoolRange { start, end }
    }

    /// Creates an atom: a text, already in its final form (escaped if the
    /// output format needs it), that the printer writes as one unit.
    ///
    /// The printer never splits an atom at the width limit (an atom wider than
    /// the room left on the line is written whole, and the line is longer than
    /// the width) and never changes it. An atom's width is measured in display
    /// columns ([`display_width`](super::display_width)).
    ///
    /// # Requirements
    ///
    /// An atom must not contain `'\n'`: a line break is expressed with
    /// [`hardline`](Self::hardline) or [`blocksep`](Self::blocksep). It must
    /// not contain `'\t'` either, which has no fixed display width: an emitter
    /// expands tabs into spaces when it builds atoms, or writes text holding
    /// tabs through [`raw`](Self::raw).
    ///
    /// # Panics
    ///
    /// In a debug build, if `s` contains `'\n'` or `'\t'`: the requirements
    /// above are checked by a debug assertion. A release build writes such an
    /// atom as it is, and the column count of that line is then wrong. In any
    /// build, if the arena's node or text pool would overflow (see
    /// [`DocBuf`]).
    pub fn text(&mut self, s: &str) -> DocId {
        debug_assert!(
            !s.contains('\n'),
            "text atoms must not contain newlines; use hardline()/blocksep()"
        );
        debug_assert!(
            !s.contains('\t'),
            "text atoms must not contain tabs; expand them or use raw()"
        );
        let range = self.intern(s);
        self.push(Node::Text(range))
    }

    /// Creates a break opportunity that prints as one space if the enclosing
    /// group stays on one line (prints *flat*), and as a line break if the
    /// group breaks.
    ///
    /// The space is written only when a content character follows it on the
    /// same line: an unbroken `line` at the end of a line leaves no trailing
    /// whitespace, and an unbroken `line` at the start of a line is dropped.
    ///
    /// # Panics
    ///
    /// If the arena's node pool would overflow (see [`DocBuf`]).
    pub fn line(&mut self) -> DocId {
        self.push(Node::Line)
    }

    /// Creates a break opportunity that prints as nothing if the enclosing
    /// group stays on one line, and as a line break if the group breaks.
    ///
    /// # Panics
    ///
    /// If the arena's node pool would overflow (see [`DocBuf`]).
    pub fn softbreak(&mut self) -> DocId {
        self.push(Node::SoftBreak)
    }

    /// Creates an unconditional line break. It also makes every enclosing
    /// group break, because a group that contains a line break cannot print
    /// on one line.
    ///
    /// Two adjacent `hardline`s produce a single newline: blank lines come only
    /// from [`blocksep`](Self::blocksep).
    ///
    /// # Panics
    ///
    /// If the arena's node pool would overflow (see [`DocBuf`]).
    pub fn hardline(&mut self) -> DocId {
        self.push(Node::HardLine)
    }

    /// Indents `child` by `columns` columns more than the enclosing
    /// indentation.
    ///
    /// The indentation applies from the next line start on: it is a unit of
    /// the printer's indentation stack, written at the start of every line
    /// that begins inside `child`.
    ///
    /// # Panics
    ///
    /// If the arena's node pool would overflow (see [`DocBuf`]).
    pub fn nest(&mut self, columns: u16, child: DocId) -> DocId {
        self.push(Node::Nest {
            indent: columns,
            child,
        })
    }

    /// Indents `child` to the column at which it starts: a hanging indent.
    ///
    /// The second and later lines of `child` start under its first character.
    /// `text("  • ")` followed by `align(body)` lays out a list item whose
    /// wrapped lines start under the item's first word. If that column is to
    /// the left of the enclosing indentation (which can happen when a deeper
    /// `nest` starts in the middle of a line), the enclosing indentation
    /// applies instead.
    ///
    /// # Panics
    ///
    /// If the arena's node pool would overflow (see [`DocBuf`]).
    pub fn align(&mut self, child: DocId) -> DocId {
        self.push(Node::Align { child })
    }

    /// Creates a group: a unit whose break opportunities all stay unbroken or
    /// all break.
    ///
    /// The printer prints `child` flat (every `line` a space, every
    /// `softbreak` nothing) if `child`, *together with everything that follows
    /// it on the same line*, fits within the width; otherwise every break
    /// opportunity directly inside `child` becomes a line break (the break
    /// opportunities inside a nested group follow that group's own decision).
    /// The [printer](super::printer)'s module documentation describes the
    /// exact test.
    ///
    /// # Panics
    ///
    /// If the arena's node pool would overflow (see [`DocBuf`]).
    pub fn group(&mut self, child: DocId) -> DocId {
        self.push(Node::Group { child })
    }

    /// Creates a sequence of the nodes `ids`, printed in that order.
    ///
    /// # Panics
    ///
    /// If the arena's node or children pool would overflow (see [`DocBuf`]).
    pub fn concat(&mut self, ids: &[DocId]) -> DocId {
        let start = pool_len(self.children.len(), "children");
        self.children.extend_from_slice(ids);
        let end = pool_len(self.children.len(), "children");
        self.push(Node::Concat(PoolRange { start, end }))
    }

    /// Creates a separation between two blocks: a line break followed by
    /// `blank_lines` blank lines.
    ///
    /// Adjacent requests for a line break are merged into one, which keeps the
    /// larger number of blank lines (the numbers are never added), so nested
    /// containers can each ask for the separation they need without producing
    /// runs of blank lines. A line break at the very start or end of the
    /// output is dropped.
    ///
    /// # Panics
    ///
    /// If the arena's node pool would overflow (see [`DocBuf`]).
    pub fn blocksep(&mut self, blank_lines: u8) -> DocId {
        self.push(Node::BlockSep { blank_lines })
    }

    /// Writes `prefix` before every line of `child` (`> ` for a quotation,
    /// `// ` for a comment block).
    ///
    /// The prefix is a unit of the indentation stack, like an indentation
    /// written as text rather than spaces: it combines with `nest` and `align`
    /// in nesting order, and its width is subtracted from the width available
    /// for wrapping (text inside `prefix("> ", ..)` wraps two columns earlier).
    /// A blank line inside `child` is written with the prefix trimmed of
    /// trailing whitespace (`>`), so a quoted passage remains one quotation
    /// across its paragraphs.
    ///
    /// # Examples
    ///
    /// Two paragraphs inside one prefix: the blank line between them is
    /// written with the prefix trimmed of its trailing space.
    ///
    /// ```
    /// use flm_core::layout::{DocBuf, Profile, print};
    ///
    /// let mut b = DocBuf::new();
    /// let first = b.text("one");
    /// let gap = b.blocksep(1);
    /// let second = b.text("two");
    /// let body = b.concat(&[first, gap, second]);
    /// let quoted = b.prefix("> ", body);
    ///
    /// assert_eq!(print(&b, quoted, &Profile::new(Some(40))), "> one\n>\n> two\n");
    /// ```
    ///
    /// # Requirements
    ///
    /// Like an atom, a prefix must not contain `'\n'` or `'\t'`.
    ///
    /// # Panics
    ///
    /// In a debug build, if `prefix` contains `'\n'` or `'\t'`: the
    /// requirement above is checked by a debug assertion. In any build, if the
    /// arena's node or text pool would overflow (see [`DocBuf`]).
    pub fn prefix(&mut self, prefix: &str, child: DocId) -> DocId {
        debug_assert!(
            !prefix.contains('\n'),
            "a line prefix must not contain newlines"
        );
        debug_assert!(!prefix.contains('\t'), "a line prefix must not contain tabs");
        let range = self.intern(prefix);
        self.push(Node::Prefix {
            prefix: range,
            child,
        })
    }

    /// Creates a verbatim block: text written as it is, never reflowed, never
    /// split and never escaped.
    ///
    /// Unlike an atom, `content` **may** contain `'\n'` and `'\t'`. A newline
    /// inside `content` is part of the content, not a request for a line
    /// break, so runs of blank lines inside the block are kept exactly. A
    /// newline at the end of `content` is treated as a request for a line
    /// break and merged with the requests around it, so `raw("code\n")`
    /// followed by `blocksep(1)` gives one blank line, not two. An empty
    /// `content` prints nothing. `policy` says whether the enclosing
    /// indentation applies to the lines of the block.
    ///
    /// A block of several lines makes its enclosing groups break; a block of
    /// one line is measured like an atom. Tabs are written unchanged and
    /// counted as one column each. The printer never breaks a raw line,
    /// regardless of its width: a raw line longer than the line width is written
    /// as it is.
    ///
    /// # Panics
    ///
    /// If the arena's node or text pool would overflow (see [`DocBuf`]).
    pub fn raw(&mut self, content: &str, policy: RawPolicy) -> DocId {
        let range = self.intern(content);
        self.push(Node::Raw {
            content: range,
            policy,
        })
    }

    /// Marks `child`, so that [`print_full`](super::print_full) reports where
    /// it is printed, under the identifier `id`.
    ///
    /// A mark has no width and does not change the layout: the printer prints
    /// `child` as if the mark were not there, and records one
    /// [`MarkSpan`](super::MarkSpan) in the result of
    /// [`print_full`](super::print_full). Marks may be nested; a marked node
    /// that appears several times in the document (a shared node) is reported
    /// once per occurrence.
    ///
    /// # Panics
    ///
    /// If the arena's node pool would overflow (see [`DocBuf`]).
    pub fn mark(&mut self, id: MarkId, child: DocId) -> DocId {
        self.push(Node::Mark { id, child })
    }

    /// Creates a table: one [`ColSpec`] per column in `cols`, and the rows of
    /// cells in `rows`.
    ///
    /// Each cell is a [`TableCell`]: any node of the same arena and,
    /// optionally, an alignment of its own that replaces its column's for that
    /// cell alone. A row may be given as [`DocId`]s, which are cells that
    /// follow their columns. Rows may have fewer cells than there are columns
    /// (the missing cells at the end of a row are empty); cells beyond
    /// `cols.len()` are ignored. `header_rows` is the number of leading rows
    /// that form the header; a rule of `-` across every column is drawn after
    /// them (`0` means no header).
    ///
    /// # How the columns are laid out
    ///
    /// The printer lays a table out in four steps:
    ///
    /// 1. **Measure.** Every cell is printed twice on its own: at width 1,
    ///    where every break opportunity breaks, and with no width limit, where
    ///    only unconditional line breaks break. A column's *minimum* is the
    ///    widest line of its cells printed at width 1 (its widest part that
    ///    cannot be broken, indentation included); its *maximum* is the widest
    ///    line printed with no limit. A cell can be wider at width 1 than with
    ///    no limit (when its indentation grows as it breaks), so the maximum is
    ///    raised to the minimum where it is smaller.
    /// 2. **Compute the widths.** The available width is the profile's width
    ///    minus the enclosing indentation and the gutters (two spaces between
    ///    adjacent columns). If the maxima of all columns fit, or the profile
    ///    has no width, each column gets its maximum. Otherwise each column
    ///    starts at its minimum, and the remaining width is shared out in
    ///    proportion to each column's room to grow (maximum minus minimum), in
    ///    whole columns: each column first gets the whole part of its share,
    ///    then the columns with the largest fractional parts get one more
    ///    column each until the width is used up (on equal fractional parts,
    ///    the column further left first), and no column grows past its
    ///    maximum. If even the minima do not fit, every column gets its minimum
    ///    and the lines are longer than the width, because atoms are never
    ///    split.
    /// 3. **Print the cells.** Every cell is printed at its column's width with
    ///    an empty indentation stack; the table's own indentation applies to
    ///    the assembled lines, so a table inside a list or a quotation is
    ///    indented and prefixed correctly.
    /// 4. **Assemble.** The rows are assembled line by line, with every cell
    ///    at the top of its row: a row is as tall as its tallest cell. Each
    ///    cell is padded to its column's width in its own alignment, or in its
    ///    column's where it has none; columns are separated by the gutter, and
    ///    trailing whitespace is removed from every line (the padding, and any
    ///    trailing whitespace of the last cell's own text).
    ///    The rule follows the header rows.
    ///
    /// A table is a block: it starts on a line of its own and ends its last
    /// line. A table that prints nothing (no columns, no rows, or only empty
    /// cells) is not a block and leaves the surrounding line as it is.
    ///
    /// Known limitation: marks inside cells are not reported, because each
    /// cell is printed by a separate run of the printer; a mark around the
    /// whole table is reported.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::layout::{ColSpec, DocBuf, Profile, print};
    ///
    /// let mut b = DocBuf::new();
    /// let name = b.text("name");
    /// let count = b.text("count");
    /// let alpha = b.text("alpha");
    /// let three = b.text("3");
    /// let t = b.table(
    ///     &[ColSpec::left(), ColSpec::right()],
    ///     &[&[name, count], &[alpha, three]],
    ///     1,
    /// );
    ///
    /// assert_eq!(
    ///     print(&b, t, &Profile::new(Some(40))),
    ///     "name   count\n-----  -----\nalpha      3\n"
    /// );
    /// ```
    ///
    /// A cell with an alignment of its own, in a left-aligned column:
    ///
    /// ```
    /// use flm_core::layout::{ColAlign, ColSpec, DocBuf, Profile, TableCell, print};
    ///
    /// let mut b = DocBuf::new();
    /// let wide = b.text("a wide cell");
    /// let right = b.text("right");
    /// let t = b.table(
    ///     &[ColSpec::left()],
    ///     &[&[TableCell::new(wide)], &[TableCell::aligned(right, ColAlign::Right)]],
    ///     0,
    /// );
    ///
    /// assert_eq!(print(&b, t, &Profile::new(Some(40))), "a wide cell\n      right\n");
    /// ```
    ///
    /// # Panics
    ///
    /// If the arena's node, column, row, cell or table pool would overflow
    /// (see [`DocBuf`]).
    pub fn table<C>(&mut self, cols: &[ColSpec], rows: &[&[C]], header_rows: u8) -> DocId
    where
        C: Copy + Into<TableCell>,
    {
        let cols_start = pool_len(self.cols.len(), "column");
        self.cols.extend_from_slice(cols);
        let cols_end = pool_len(self.cols.len(), "column");

        let rows_start = pool_len(self.rows.len(), "row");
        // Two passes so that the rows pool stays contiguous for this table.
        let mut ranges = Vec::with_capacity(rows.len());
        for row in rows {
            let start = pool_len(self.cells.len(), "cell");
            self.cells.extend(row.iter().map(|&cell| cell.into()));
            let end = pool_len(self.cells.len(), "cell");
            ranges.push(PoolRange { start, end });
        }
        self.rows.extend_from_slice(&ranges);
        let rows_end = pool_len(self.rows.len(), "row");

        let def = pool_len(self.tables.len(), "table");
        self.tables.push(TableDef {
            rows: PoolRange {
                start: rows_start,
                end: rows_end,
            },
            cols: PoolRange {
                start: cols_start,
                end: cols_end,
            },
            header_rows,
        });
        self.push(Node::Table { def })
    }

    /// Creates prose: the words of `s`, separated by spaces that each break
    /// independently of the others.
    ///
    /// This is a combination of the other constructors, and its exact shape
    /// matters: each space between two words is its own `group(line())`, so
    /// each space decides on its own whether to stay a space or become a line
    /// break. Because the printer's test for a group looks past the group to
    /// what follows on the line, this gives correct greedy wrapping, including
    /// before a trailing text that has no break opportunity of its own. One
    /// group around a whole paragraph would instead give one line longer than
    /// the width, or a line break at every space.
    ///
    /// `s` is split at breakable whitespace ([`is_breakable_space`]), which is
    /// dropped; the words become atoms. A non-breaking space (U+00A0 no-break
    /// space, U+2007 figure space, U+202F narrow no-break space) stays inside
    /// the word it joins, so an author's request not to break there is kept.
    /// An `s` holding only breakable whitespace gives an empty sequence.
    ///
    /// # Panics
    ///
    /// If the arena's node, text or children pool would overflow (see
    /// [`DocBuf`]).
    pub fn fill_words(&mut self, s: &str) -> DocId {
        let mut parts: Vec<DocId> = Vec::new();
        for word in s.split(is_breakable_space).filter(|word| !word.is_empty()) {
            if !parts.is_empty() {
                let l = self.line();
                parts.push(self.group(l));
            }
            parts.push(self.text(word));
        }
        self.concat(&parts)
    }
}

/// Returns whether `c` is whitespace at which a line may break: every Unicode
/// whitespace character except the non-breaking spaces (U+00A0 no-break
/// space, U+2007 figure space, U+202F narrow no-break space), which join the
/// words on either side of them into one word.
///
/// [`DocBuf::fill_words`] splits its text into words with this function. An
/// emitter that splits text into words itself, before building atoms, uses
/// the same function, so that both split at the same characters.
pub fn is_breakable_space(c: char) -> bool {
    c.is_whitespace() && !matches!(c, '\u{00A0}' | '\u{2007}' | '\u{202F}')
}
