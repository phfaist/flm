//! The recomposition rule of a table of cells on the `latex-recomposed`
//! backend ([`LATEX_RULE`], [`CellsLatexRule`]): the table is written as one
//! call of the LaTeX hook `\flmCellsTable`
//! ([LaTeX hooks](crate::stdcore::backends::latex::preamble)), which the
//! feature's definition piece ([`definition_piece`]) defines over the
//! `tabularray` package.
//!
//! A definition piece is a unit of LaTeX definitions that a feature registers
//! under an identifier `flm.latex.def.<name>`
//! ([`LatexPreambleDef`]); the style file holds every piece the document
//! requires. The rendering guide describes the output for LaTeX authors in
//! [*Tables of cells in the recomposed LaTeX*](crate::guide::rendering#tables-of-cells-in-the-recomposed-latex),
//! and the LaTeX hooks in general in
//! [*The LaTeX hooks*](crate::guide::rendering#the-latex-hooks).
//!
//! | written | LaTeX |
//! |---|---|
//! | `\begin{cells}\celldata<H>{One & Two}\celldata{A & B}\end{cells}` | `\flmCellsTable{cc}{X[-1]X[-1]}{hspan=minimal,`<br>`  cell{1}{1}={}{m, font={\flmCellsHeaderFont}},`<br>`  cell{1}{2}={}{m, font={\flmCellsHeaderFont}},`<br>`  hline{2}={1}{.4pt,solid},`<br>`  hline{2}={2}{.4pt,solid}}{\toprule`<br>`One&Two\\`<br>`A&B\\`<br>`\bottomrule`<br>`}` |
//! | `\cell<r green>[\merge{1-2};1]{X}` | the entry `cell{1}{1}={r=2}{r}` and the cell's text `\flmCellsSet{green}X` |
//!
//! # The call
//!
//! A table is written `\flmCellsTable{<natural>}{<fitted>}{<options>}{<rows>}`,
//! from the table's model as the feature's reader ([`read_grid`]) builds it
//! and from the positions of the grid that each cell occupies
//! ([`grid`](crate::stdcore::backends::pres::CellsPayload::grid)):
//!
//! - `<natural>` is the column specification of `tabularray`'s `tblr`
//!   environment with which the table is first set, `c` for each column, and
//!   `<fitted>` the one with which it is set where it is too wide, `X[-1]`
//!   for each column;
//! - `<options>` is `hspan=minimal`, then one `cell{<row>}{<column>}=
//!   {<span>}{<keys>}` entry for each cell that spans rows or columns
//!   (`r=2`, `c=3`) or has an alignment or a header kind (its keys: `l`, `c`
//!   or `r`, else `m`, then `font={\flmCellsHeaderFont}` for a header), then the
//!   lines: `hline{<row>}={<columns>}{.4pt,solid}` under a cell that heads its
//!   column (unless it is in the table's last row, which `\bottomrule`
//!   closes) and at a cell's top or bottom edge where a cell style draws a
//!   line, and `vline{<column>}={<rows>}{.4pt,solid}` at a cell's left or
//!   right edge where a cell style draws one; one entry per cell and edge,
//!   rows and columns counted from 1 and a range written `2-3`;
//! - `<rows>` is `\toprule`, then the rows, each its cells' texts separated
//!   by `&` and ended by `\\`, with an empty text at an empty position and at
//!   a position that a spanning cell covers, then `\bottomrule`.
//!
//! A cell's text is one `\flmCellsSet{<name>}` for each custom style it
//! carries, in the order of its tokens, then its content, recomposed through
//! the ordinary rules: a `\cell`'s content with its blank lines, a
//! `\celldata` entry's inline content with the whitespace at its edges
//! trimmed ([`stage_trimmed`]), as on every backend. `\flmCellsSet` is a
//! *table command*, a command that `tabularray` reads only at the start of a
//! cell's text. What a cell style states is written from the model's facts,
//! never from a token as written. Each cell is first read into what it
//! writes around its content on this backend (the span and keys of its
//! `cell{..}` option entry, the lines it draws, its table commands: the
//! rendering model of a cell), and
//! then written: the table commands, then its content.
//!
//! # What the rule requires, reports and leaves out
//!
//! The rule requires (records a requirement for) the feature's definition
//! piece ([`LATEX_PIECE`], [`definition_piece`]), which requests the LaTeX
//! packages `tabularray` and `xcolor`, and, for every cell style the table
//! uses, what the cell style requires on this backend
//! ([`CellStyle::requirements_on`](super::CellStyle::requirements_on)): the
//! shipped colors require the feature's piece, and a cell style of another
//! feature requires that feature's own piece, which registers its keys. The
//! rule reports every problem the reader found, as the feature's general
//! rule does, and stages one dropped mark (`flm.pres.dropped`) for each
//! thing the reader left out, immediately before the call; the
//! `latex-recomposed` backend's emitter writes each dropped mark as a comment
//! line. An empty table writes nothing but
//! its dropped marks. A command of the tables of cells written out of place
//! is lowered on this backend too by the feature's own rule for every
//! backend ([`OutsideRule`](super::lower::OutsideRule)).
//!
//! A float inside a cell is left to LaTeX and the author: a cell is a box,
//! and LaTeX cannot place a float in one.
//!
//! This backend reads the tree before block structure is built, so a blank
//! line inside a `\celldata`'s data is written as the document wrote it and
//! not reported, as for every other construct on this backend.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::latex::construct::{ARG_SLOT, MACRO, RAW};
use crate::stdcore::backends::latex::{
    LatexArgShape, LatexMacroPayload, LatexPreambleDef, LatexRawPayload, packages, preamble,
};
use crate::stdcore::backends::latex_recomposed::BACKEND;
use crate::stdcore::backends::lowering::stage_trimmed;
use crate::stdcore::backends::pres::{self, CellAlign, CellEdge, CellHeader, CellSpec, GridPosition};

use super::reader::{CellContent, read_grid};
use super::styles::Styles;
use super::{ENVIRONMENT, LATEX_PIECE};

/// The name of the recomposition rule, `flm.cells.cells.lower.latex-recomposed`
/// ([`CellsLatexRule`]).
pub const LATEX_RULE: &str = "flm.cells.cells.lower.latex-recomposed";

/// The name of the feature's definition piece, `cells`; its identifier is
/// `flm.latex.def.cells` ([`LATEX_PIECE`], [`definition_piece`]).
pub const PIECE_NAME: &str = "cells";

/// The name of the LaTeX hook `\flmCellsTable`, which the recomposition
/// rule calls once for each table of cells:
/// `\flmCellsTable{<natural colspec>}{<fitted colspec>}{<options>}{<rows>}`.
pub const TABLE_COMMAND: &str = "flmCellsTable";

/// The name of the table command as which a cell's custom style is written,
/// `flmCellsSet`: `\flmCellsSet{<name>}` at the start of the cell's text.
pub const SET_COMMAND: &str = "flmCellsSet";

/// The name of the command that registers the `tabularray` cell keys of a
/// custom style, `flmCellsStyleKeys`: `\flmCellsStyleKeys{<name>}{<keys>}`.
/// The definition piece of a feature that declares a custom style calls it
/// to give that style its keys; the first registration of a name is kept, as
/// `\providecommand` keeps the first definition.
pub const STYLE_KEYS_COMMAND: &str = "flmCellsStyleKeys";

/// The prefix of the name of the command that holds the cell keys
/// registered for a custom style, `flmCellsKeys`: `\flmCellsKeysgreen` holds
/// `bg=flmCellsColorgreen`. An author defines this command before loading
/// the style file, or redefines it after, to give a style other keys; for a
/// name holding a digit, the command is written
/// `\csname flmCellsKeys<name>\endcsname`, since a command name written
/// after a backslash holds letters only.
pub const KEYS_PREFIX: &str = "flmCellsKeys";

/// The prefix of the name of a shipped style's background color,
/// `flmCellsColor`: `flmCellsColorgreen` and the three others.
pub const COLOR_PREFIX: &str = "flmCellsColor";

/// The shipped background colors, each with its value in the RGB model, from
/// 0 to 255: the colors of the custom styles `green`, `blue`, `yellow` and
/// `red`.
pub const COLORS: [(&str, &str); 4] =
    [("green", "200,255,200"), ("blue", "200,220,255"), ("yellow", "255,255,200"), ("red", "255,200,200")];

/// The feature's definition piece ([`LatexPreambleDef`]),
/// `flm.latex.def.cells`. It defines:
///
/// - the LaTeX hook `\flmCellsTable` and the four commands that change one
///   part of what it does: `\flmCellsHeaderFont`, `\flmCellsBeginCenter`,
///   `\flmCellsEndCenter` and `\flmCellsMaxWidth`;
/// - the table command `\flmCellsSet`, and `\flmCellsStyleKeys`, which
///   registers a custom style's cell keys;
/// - the four shipped colors, registered as the keys of the custom styles of
///   the same names;
/// - the loading of `tabularray`'s `booktabs` library, and of its `counter`
///   library, which restores LaTeX's counters after the passes in which
///   `tabularray` sets a cell only to measure it, so that a footnote in a
///   cell advances its number once, except in the cases that
///   [*Tables of cells in the recomposed LaTeX*](crate::guide::rendering#tables-of-cells-in-the-recomposed-latex)
///   describes.
///
/// It records package requests for `tabularray` and `xcolor`
/// ([`packages::TABULARRAY`], [`packages::XCOLOR`]).
///
/// By default the hook sets the table at its natural width with the natural
/// column specification. Where that is wider than `\flmCellsMaxWidth`
/// (`0.96\linewidth`), it sets the table again at that width with the fitted
/// column specification, whose columns share the width and break their
/// lines. It sets the table between `\flmCellsBeginCenter` and
/// `\flmCellsEndCenter` (a `center` environment), except where LaTeX sets a
/// single line, as in a cell of another table set at its natural width or
/// in an `\mbox`: no paragraph can begin there, and the table is set
/// without them.
pub fn definition_piece() -> LatexPreambleDef {
    let mut colors = String::new();
    for (name, rgb) in COLORS {
        colors.push_str(&format!("\\providecolor{{{COLOR_PREFIX}{name}}}{{RGB}}{{{rgb}}}\n"));
    }
    for (name, _) in COLORS {
        colors.push_str(&format!("\\{STYLE_KEYS_COMMAND}{{{name}}}{{bg={COLOR_PREFIX}{name}}}\n"));
    }
    let content = format!(
        "\
% Tables of cells. A table is written
% \\flmCellsTable{{<natural>}}{{<fitted>}}{{<options>}}{{<rows>}}: <natural> and
% <fitted> are two column specifications of the tblr environment of the
% tabularray package (<natural> has c for each column, <fitted> has X[-1]
% for each column),
% <options> are the table's other tblr options (the cells' spans, alignment
% and header font, and the lines), and <rows> are the rows, the cells
% separated by & and each row ended by \\\\.
% By default the table is set at its natural width with <natural>; where it
% is wider than \\flmCellsMaxWidth, it is set again at that width with
% <fitted>. It is set between \\flmCellsBeginCenter and \\flmCellsEndCenter,
% except where LaTeX sets a single line, as in a cell of another table set
% at its natural width, where no paragraph can begin. A header cell is set
% in \\flmCellsHeaderFont. Redefine \\flmCellsTable to set tables another way,
% or one of the four other commands to change one part of it.
% A cell's custom style <name> is written \\flmCellsSet{{<name>}} at the start
% of the cell's text: a tabularray table command that applies the tabularray
% cell keys held in \\flmCellsKeys<name>. \\flmCellsStyleKeys{{<name>}}{{<keys>}}
% sets those keys unless keys are already set for <name>: define
% \\flmCellsKeys<name> before loading this file, or redefine it after, to
% change how a style looks. The shipped styles green, blue, yellow and red
% set the background colors flmCellsColor<name>, which you can define before
% loading this file.
% tabularray's counter library restores LaTeX's counters after the passes in
% which tabularray sets a cell only to measure it.
% The macros with @ in their names are internals, not a customization point.
\\UseTblrLibrary{{booktabs}}
\\UseTblrLibrary{{counter}}
\\providecommand\\flmCellsHeaderFont{{\\bfseries}}
\\providecommand\\flmCellsBeginCenter{{\\begin{{center}}}}
\\providecommand\\flmCellsEndCenter{{\\end{{center}}}}
\\providecommand\\flmCellsMaxWidth{{0.96\\linewidth}}
\\ExplSyntaxOn
\\long\\def\\flm@cells@fit#1#2#3#4{{%
  \\setbox0=\\hbox{{\\begin{{tblr}}{{colspec={{#1}},#3}}#4\\end{{tblr}}}}%
  \\dim_compare:nNnTF {{\\wd0}} < {{\\flmCellsMaxWidth}}
    {{\\leavevmode\\box0}}
    {{\\begin{{tblr}}{{width=\\flmCellsMaxWidth,colspec={{#2}},#3}}#4\\end{{tblr}}}}}}
\\long\\def\\flm@cells@iflr#1#2{{\\mode_if_horizontal:TF {{\\mode_if_inner:TF {{#1}} {{#2}}}} {{#2}}}}
\\def\\flm@cells@set#1{{\\ifcsname flmCellsKeys#1\\endcsname
  \\exp_args:Nnv \\tblr_set_cell:nn {{}} {{flmCellsKeys#1}}\\fi}}
\\ExplSyntaxOff
\\ProvideDocumentCommand\\flmCellsTable{{+m +m +m +m}}{{%
  \\flm@cells@iflr
    {{\\flm@cells@fit{{#1}}{{#2}}{{#3}}{{#4}}}}%
    {{\\flmCellsBeginCenter\\flm@cells@fit{{#1}}{{#2}}{{#3}}{{#4}}\\flmCellsEndCenter}}}}
\\ProvideDocumentCommand\\flmCellsStyleKeys{{m m}}{{%
  \\expandafter\\providecommand\\csname flmCellsKeys#1\\endcsname{{#2}}}}
\\ifcsname flmCellsSet\\endcsname\\else
  \\NewTableCommand\\flmCellsSet[1]{{\\flm@cells@set{{#1}}}}
  \\ProvideDocumentCommand\\flmCellsSet{{m}}{{}}
\\fi
{colors}"
    );
    LatexPreambleDef::new(
        PIECE_NAME,
        content,
        "`\\flmCellsTable{natural}{fitted}{options}{rows}`: the LaTeX hook through which a \
         table of cells is set, with `\\flmCellsSet{name}` and \
         `\\flmCellsStyleKeys{name}{keys}` for a cell's custom styles, and the four shipped \
         colors",
    )
    .requires([packages::TABULARRAY, packages::XCOLOR])
}

/// The key-value pair that sets the line of a `hline` or `vline` option entry.
const RULE_LINE: &str = ".4pt,solid";

/// The rendering model of one cell of a table of cells: what the cell writes
/// around its content on this backend, filled from the cell's record in the
/// table's model ([`latex_cell`]) before anything is written. The
/// recomposition rule writes the cell's `cell{..}` option entry and its
/// `hline`/`vline` option entries among the table's options, and the cell's
/// text (the
/// table commands, then the content) with [`cell_text`]. The content is not
/// part of the model: the reader provides it, and it is lowered through the
/// ordinary rules.
///
/// This is the only place where a cell's facts become LaTeX, so that how a
/// cell is written can be changed in one place.
// A seam a later step may put behind a trait a host implements; nothing of
// it is public. See decision rationale at [§flm-dd-dr:std-cells-latex].
#[derive(Debug, Clone, PartialEq, Eq)]
struct LatexCell {
    /// The row and the column of the cell's top-left position, counted from 1.
    at: (u64, u64),
    /// The span keys of the cell's `cell{..}` option entry, `r=2` and `c=3`,
    /// where it spans more than one row or column.
    span: Vec<String>,
    /// The cell keys of its `cell{..}` option entry: its alignment, `l`, `c`
    /// or `r` (`m` where it has none), then `font={\flmCellsHeaderFont}` where it is a header.
    keys: Vec<String>,
    /// The table options that add the cell's lines: its `hline` and `vline`
    /// option entries, in order.
    rules: Vec<String>,
    /// The custom styles it carries, each written as a table command
    /// `\flmCellsSet{<name>}` at the start of its text, in order.
    commands: Vec<Name>,
}

impl LatexCell {
    /// The cell's `cell{<row>}{<column>}={<span>}{<keys>}` entry, where it
    /// needs one: a span, an alignment or a header kind.
    fn entry(&self) -> Option<String> {
        if self.span.is_empty() && self.keys == ["m"] {
            return None;
        }
        let (row, col) = self.at;
        Some(format!("cell{{{row}}}{{{col}}}={{{}}}{{{}}}", self.span.join(","), self.keys.join(", ")))
    }
}

/// A range of rows or columns counted from 1, `3` or `3-4`, from the first
/// and one past the last counted from 0.
fn range(start: u64, end: u64) -> String {
    if end == start + 1 { format!("{}", start + 1) } else { format!("{}-{end}", start + 1) }
}

/// The rendering model of the cell `spec` of a table of `rows` rows: what it
/// writes on this backend ([`LatexCell`]).
fn latex_cell(spec: &CellSpec, rows: u32) -> LatexCell {
    let (row, col) = (u64::from(spec.row), u64::from(spec.col));
    let (row_end, col_end) = (spec.row_end(), spec.col_end());
    let mut span = Vec::new();
    if spec.row_span > 1 {
        span.push(format!("r={}", spec.row_span));
    }
    if spec.col_span > 1 {
        span.push(format!("c={}", spec.col_span));
    }
    let align = match spec.align {
        Some(CellAlign::Left) => "l",
        Some(CellAlign::Center) => "c",
        Some(CellAlign::Right) => "r",
        None => "m",
    };
    let mut keys = vec![align.to_string()];
    if spec.header.is_some() {
        keys.push(String::from("font={\\flmCellsHeaderFont}"));
    }
    let mut rules = Vec::new();
    // The table's last row is closed by `\bottomrule`.
    if spec.header == Some(CellHeader::Column) && row_end < u64::from(rows) {
        rules.push(format!("hline{{{}}}={{{}}}{{{RULE_LINE}}}", row_end + 1, range(col, col_end)));
    }
    for edge in &spec.rules {
        rules.push(match edge {
            CellEdge::Above => format!("hline{{{}}}={{{}}}{{{RULE_LINE}}}", row + 1, range(col, col_end)),
            CellEdge::Below => format!("hline{{{}}}={{{}}}{{{RULE_LINE}}}", row_end + 1, range(col, col_end)),
            CellEdge::Left => format!("vline{{{}}}={{{}}}{{{RULE_LINE}}}", col + 1, range(row, row_end)),
            CellEdge::Right => format!("vline{{{}}}={{{}}}{{{RULE_LINE}}}", col_end + 1, range(row, row_end)),
        });
    }
    LatexCell { at: (row + 1, col + 1), span, keys, rules, commands: spec.styles.clone() }
}

/// Stages the text of `cell` from its rendering model: one table command
/// `\flmCellsSet{<name>}` per custom style, in order, then `content` lowered
/// through the ordinary rules: a `\cell`'s blocks as written, a `\celldata`
/// entry's inline content with the whitespace at its edges trimmed
/// ([`stage_trimmed`]).
fn cell_text(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    cell: &LatexCell,
    content: CellContent<'_>,
) -> Result<Vec<BuildId>, LowerError> {
    let mut text = Vec::new();
    for name in &cell.commands {
        let argument = vec![raw(cx, lw, name.as_str())?];
        text.push(command(cx, lw, SET_COMMAND, vec![argument])?);
    }
    match content {
        CellContent::Blocks(slice) => text.extend(lw.lower_nodes(cx, slice)?),
        CellContent::Inline(slice) => {
            let nodes: Vec<NodeRef<'_, Flm>> = slice.iter().collect();
            text.extend(stage_trimmed(cx, lw, &nodes, |_| false)?);
        }
    }
    Ok(text)
}

/// Stages a raw node (`flm.pres.latex.raw`) holding the LaTeX text `text`.
fn raw(cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer, text: &str) -> Result<BuildId, LowerError> {
    lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(text), Vec::new())
}

/// Stages a macro call `\<name>` with one mandatory argument per entry of
/// `slots`, each entry the staged content of one argument, in order.
fn command(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    name: &str,
    slots: Vec<Vec<BuildId>>,
) -> Result<BuildId, LowerError> {
    lw.pres_slots_with(
        cx,
        &ElementName::from(MACRO),
        LatexMacroPayload::new(name, vec![LatexArgShape::Mandatory; slots.len()]),
        slots.into_iter().map(|slot| (String::from(ARG_SLOT), slot)).collect(),
    )
}

/// The recomposition rule [`LATEX_RULE`] of a table of cells, the default on
/// the `latex-recomposed` backend: it stages the table's dropped marks, then
/// one call of the LaTeX hook `\flmCellsTable`. See the
/// [module documentation](self).
#[derive(Debug)]
pub struct CellsLatexRule {
    styles: Arc<Styles>,
}

impl CellsLatexRule {
    /// The rule, reading style tokens against the cell style declarations
    /// `styles`.
    pub fn new(styles: Arc<Styles>) -> CellsLatexRule {
        CellsLatexRule { styles }
    }
}

impl Lower for CellsLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let grid = read_grid(&node, &self.styles)?;
        // Every report is made, by each render that lowers the table.
        let mut staged = Vec::new();
        for problem in grid.problems {
            lw.diagnose(problem.diagnostic);
            if let Some(mark) = problem.dropped {
                staged.push(lw.pres_with(cx, &ElementName::from(pres::DROPPED), mark, Vec::new())?);
            }
        }
        let payload = grid.payload;
        if payload.cells.is_empty() {
            return Ok(Lowering::Emit(staged));
        }
        lw.require(&Name::from(LATEX_PIECE));
        for name in &grid.styles {
            if let Some(style) = self.styles.get(name.as_str()) {
                for id in style.requirements_on(BACKEND) {
                    lw.require(id);
                }
            }
        }
        let cells: Vec<LatexCell> = payload.cells.iter().map(|spec| latex_cell(spec, payload.rows)).collect();
        let mut options = String::from("hspan=minimal");
        for entry in cells.iter().filter_map(LatexCell::entry) {
            options.push_str(&format!(",\n  {entry}"));
        }
        for rule in cells.iter().flat_map(|cell| &cell.rules) {
            options.push_str(&format!(",\n  {rule}"));
        }
        let mut texts = Vec::with_capacity(cells.len());
        for (cell, content) in cells.iter().zip(grid.contents) {
            texts.push(Some(cell_text(cx, lw, cell, content)?));
        }
        let mut rows = vec![raw(cx, lw, "\\toprule\n")?];
        for row in payload.grid() {
            for (index, position) in row.iter().enumerate() {
                if index > 0 {
                    rows.push(raw(cx, lw, "&")?);
                }
                if let GridPosition::TopLeft(entry) = position {
                    rows.extend(texts.get_mut(*entry).and_then(Option::take).unwrap_or_default());
                }
            }
            rows.push(raw(cx, lw, "\\\\\n")?);
        }
        rows.push(raw(cx, lw, "\\bottomrule\n")?);
        let natural = "c".repeat(payload.cols as usize);
        let fitted = "X[-1]".repeat(payload.cols as usize);
        let slots = vec![
            vec![raw(cx, lw, &natural)?],
            vec![raw(cx, lw, &fitted)?],
            vec![raw(cx, lw, &options)?],
            rows,
        ];
        staged.push(command(cx, lw, TABLE_COMMAND, slots)?);
        Ok(Lowering::Emit(staged))
    }
}

/// The recomposition rule of a table of cells, [`LATEX_RULE`], reading
/// style tokens against `styles`: the default on the `latex-recomposed`
/// backend only.
fn latex_rule(styles: Arc<Styles>) -> LoweringRule {
    LoweringRule::new(
        LATEX_RULE,
        ENVIRONMENT,
        LoweringDecl::new().produces(MACRO).produces(RAW).produces(pres::DROPPED),
        Arc::new(CellsLatexRule::new(styles)),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs(
        "a table of cells lowers to one call of the LaTeX hook `\\flmCellsTable` over its \
         column specifications, its `tabularray` options and its rows, after one dropped mark \
         (`flm.pres.dropped`) per thing left out",
    )
}

/// Registers what the feature contributes to the `latex-recomposed` backend:
/// the peer declarations ([`PeerName`](crate::core::registry::PeerName)) of
/// the backend and of the constructs the rule stages, the definition piece
/// (a peer contribution to the service slot [`preamble::SLOT`]), then
/// [`latex_rule`], reading style tokens against `styles`. A registry without
/// the LaTeX features still builds, with the rule set aside (excluded from
/// rule selection; see [`PeerName`](crate::core::registry::PeerName)).
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made in
/// a registration round that does not accept it (see
/// [`FeatureBuilder`](crate::core::registry::FeatureBuilder)).
pub(super) fn register(b: &mut FeatureBuilder<'_>, styles: Arc<Styles>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the recomposition rule is the default",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro-call construct, which the recomposition rule stages for the \
         LaTeX hook `\\flmCellsTable` and a cell's table commands",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw node construct, in which the column specifications, options \
         and row separators of the LaTeX hook `\\flmCellsTable` are written",
    )?;
    b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(definition_piece()))?;
    b.lowering_rule(latex_rule(styles))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stdcore::backends::latex::preamble::piece_id;

    #[test]
    fn the_piece_is_named_as_its_identifier_says_and_requests_tabularray_and_xcolor() {
        let piece = definition_piece();
        assert_eq!(piece.id, piece_id(PIECE_NAME));
        assert_eq!(piece.id.as_str(), LATEX_PIECE);
        assert_eq!(piece.requires, [Name::from(packages::TABULARRAY), Name::from(packages::XCOLOR)]);
    }

    #[test]
    fn the_piece_defines_the_hook_its_knobs_the_table_command_and_the_colors() {
        let content = definition_piece().content;
        for expected in [
            "\\ProvideDocumentCommand\\flmCellsTable{+m +m +m +m}",
            "\\providecommand\\flmCellsHeaderFont{\\bfseries}",
            "\\providecommand\\flmCellsBeginCenter{\\begin{center}}",
            "\\providecommand\\flmCellsEndCenter{\\end{center}}",
            "\\providecommand\\flmCellsMaxWidth{0.96\\linewidth}",
            "\\NewTableCommand\\flmCellsSet[1]",
            "\\tblr_set_cell:nn",
            "\\ProvideDocumentCommand\\flmCellsStyleKeys{m m}",
            "\\UseTblrLibrary{booktabs}",
            "\\UseTblrLibrary{counter}",
            "\\providecolor{flmCellsColorgreen}{RGB}{200,255,200}",
            "\\providecolor{flmCellsColorred}{RGB}{255,200,200}",
            "\\flmCellsStyleKeys{yellow}{bg=flmCellsColoryellow}",
            "\\flmCellsStyleKeys{blue}{bg=flmCellsColorblue}",
        ] {
            assert!(content.contains(expected), "{expected}\n{content}");
        }
        // Every line is a whole line, and no line is a comment's continuation.
        assert!(content.ends_with('\n'));
    }

    #[test]
    fn the_rendering_model_of_a_cell_holds_its_entry_its_rules_and_its_commands() {
        let mut spec = CellSpec::at(1, 2);
        spec.row_span = 2;
        spec.align = Some(CellAlign::Right);
        spec.header = Some(CellHeader::Column);
        spec.rules = vec![CellEdge::Right, CellEdge::Above];
        spec.styles = vec![Name::from("green"), Name::from("bold")];
        let cell = latex_cell(&spec, 4);
        assert_eq!(
            cell,
            LatexCell {
                at: (2, 3),
                span: vec![String::from("r=2")],
                keys: vec![String::from("r"), String::from("font={\\flmCellsHeaderFont}")],
                rules: vec![
                    String::from("hline{4}={3}{.4pt,solid}"),
                    String::from("vline{4}={2-3}{.4pt,solid}"),
                    String::from("hline{2}={3}{.4pt,solid}"),
                ],
                commands: vec![Name::from("green"), Name::from("bold")],
            }
        );
        assert_eq!(cell.entry().as_deref(), Some("cell{2}{3}={r=2}{r, font={\\flmCellsHeaderFont}}"));
        // A plain cell needs no entry; a column header in the last row draws no
        // rule under it, `\bottomrule` closing the table.
        assert_eq!(latex_cell(&CellSpec::at(0, 0), 1).entry(), None);
        let mut last = CellSpec::at(0, 0);
        last.header = Some(CellHeader::Column);
        assert!(latex_cell(&last, 1).rules.is_empty());
    }
}
