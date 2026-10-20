//! The general lowering rules of the tables of cells.
//!
//! [`CellsRule`] (the rule [`RULE`]) is the rule of the `cells` environment
//! on every backend except `latex-recomposed`. It reads the table through
//! [`read_grid`], reports every problem found, and stages (builds into the
//! lowered tree; see [`Lowerer`]) one dropped mark (`flm.pres.dropped`) per
//! thing left out, then one
//! [`flm.pres.cells`](crate::stdcore::backends::pres::CELLS) node. The
//! node's payload is the table's model
//! ([`CellsPayload`](crate::stdcore::backends::pres::CellsPayload)), and its
//! `cell` content slots hold the cells' content: a `\cell`'s blocks, or a
//! `\celldata` entry's inline content as one paragraph with the whitespace
//! at its edges trimmed ([`stage_trimmed`]), so that a cell holds blocks on
//! every backend. An empty table stages nothing but its dropped marks. On
//! the backend it runs on, the rule requires (records a requirement for) what
//! each cell style used in the table declares for that backend
//! ([`CellStyle::requires`](super::CellStyle::requires)): style definitions
//! on HTML, definition pieces or package requests on the recomposed LaTeX.
//!
//! What each backend makes of the `flm.pres.cells` node:
//!
//! - **HTML**: a `<table>` written as the
//!   [HTML contract](crate::stdcore::backends::html::CONVENTIONS) states for
//!   `flm.pres.cells`, a cell's facts as attributes, with the feature's
//!   style definition ([`html`](super::html));
//! - **Text**: a table of the layout engine, its columns aligned, the rows
//!   of column headers followed by a horizontal line; row headers, edge
//!   lines and custom styles are not shown;
//! - **`latex-recomposed`**: this rule is not used; the recomposition rule
//!   ([`latex`](super::latex)) writes one call of the LaTeX hook
//!   `\flmCellsTable`.
//!
//! [`OutsideRule`] is the rule of a `\cell`, `\celldata`, `\\`, `&`,
//! `\rownames` or `\colnames` that the lowering reaches on its own, outside
//! the table's reader, on every backend. It reports the command
//! ([`OutsideTable`]); it shows the content of a command that has content
//! where the command is written, and places a dropped mark where a command
//! without content was.

use std::sync::Arc;

use techy::core::node::{BuildId, NodeRef};
use techy::error::Diagnostic;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::lang::Flm;
use crate::model::element::element_of;
use crate::model::names::{ElementName, Name};
use crate::stdcore::backends::lowering::{lower_paragraph, stage_trimmed};
use crate::stdcore::backends::pres::{self, CELL_SLOT, DroppedPayload};

use super::diagnostics::OutsideTable;
use super::reader::{CellContent, read_grid};
use super::styles::Styles;
use super::{CELL, COLNAMES, COLUMN, CONTENT_ARG, DATA, DATA_ARG, ENVIRONMENT, MERGE, ROW, ROWNAMES};

/// The name of the general rule of the `cells` environment,
/// `flm.cells.cells.lower.default` ([`CellsRule`]).
pub const RULE: &str = "flm.cells.cells.lower.default";

/// The lowering rule [`RULE`] of the `cells` environment, the default on
/// every backend except `latex-recomposed`: it stages the table's dropped
/// marks, then one `flm.pres.cells` node. See the
/// [module documentation](self).
#[derive(Debug)]
pub struct CellsRule {
    styles: Arc<Styles>,
}

impl CellsRule {
    /// The rule, reading style tokens against the cell style declarations
    /// `styles`.
    pub fn new(styles: Arc<Styles>) -> CellsRule {
        CellsRule { styles }
    }
}

impl Lower for CellsRule {
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
        if grid.payload.cells.is_empty() {
            return Ok(Lowering::Emit(staged));
        }
        let backend = lw.backend().clone();
        for name in &grid.styles {
            if let Some(style) = self.styles.get(name.as_str()) {
                for id in style.requirements_on(backend.as_str()) {
                    lw.require(id);
                }
            }
        }
        let mut slots: Vec<(String, Vec<BuildId>)> = Vec::with_capacity(grid.contents.len());
        for content in grid.contents {
            let blocks = match content {
                CellContent::Blocks(slice) => lw.lower_nodes(cx, slice)?,
                CellContent::Inline(slice) => {
                    let nodes: Vec<NodeRef<'_, Flm>> = slice.iter().collect();
                    let run = stage_trimmed(cx, lw, &nodes, |_| false)?;
                    vec![lower_paragraph(cx, lw, None, run, Vec::new())?]
                }
            };
            slots.push((CELL_SLOT.to_string(), blocks));
        }
        staged.push(lw.pres_slots_with(cx, &ElementName::from(pres::CELLS), grid.payload, slots)?);
        Ok(Lowering::Emit(staged))
    }
}

/// The lowering rule of a command of the tables of cells that is lowered on
/// its own, outside the reader of its table ([`OUTSIDE_RULES`]), the default
/// on every backend. It reports the command as [`OutsideTable`]. It shows a
/// `\cell`'s content as blocks, and a `\celldata`'s data as inline content,
/// where the command is written. It leaves out a `\\`, an `&`, a
/// `\rownames` or a `\colnames`, and a `\cell` or a `\celldata` missing
/// its content argument, and places a dropped mark (`flm.pres.dropped`)
/// where the command was. The rule also covers `\merge`, since every element
/// needs a rule, but no document reaches it there: the placement and mapping
/// arguments in which `\merge` can be written are read by the table's
/// reader and never lowered.
#[derive(Debug, Default)]
pub struct OutsideRule;

/// The commands [`OutsideRule`] covers, each with its element, the name of
/// its rule, the message saying where the command must be written (the
/// detail of [`OutsideTable`]), and how the command is written.
pub const OUTSIDE_RULES: [(&str, &str, &str, &str); 7] = [
    (
        CELL,
        "flm.cells.cell.lower.default",
        "\\cell must be written directly in the body of a cells environment, or as an entry of \\celldata",
        "\\cell",
    ),
    (
        DATA,
        "flm.cells.data.lower.default",
        "\\celldata must be written directly in the body of a cells environment",
        "\\celldata",
    ),
    (
        ROW,
        "flm.cells.row.lower.default",
        "\\\\ in a table of cells must be written directly in its body or in the data of \\celldata",
        "\\\\",
    ),
    (
        COLUMN,
        "flm.cells.column.lower.default",
        "& must be written directly in the data of \\celldata, between two entries",
        "&",
    ),
    (
        MERGE,
        "flm.cells.merge.lower.default",
        "\\merge must be written in the placement of \\cell or the mapping of \\celldata",
        "\\merge",
    ),
    (
        ROWNAMES,
        "flm.cells.rownames.lower.default",
        "\\rownames must be written directly in the body of a cells environment",
        "\\rownames",
    ),
    (
        COLNAMES,
        "flm.cells.colnames.lower.default",
        "\\colnames must be written directly in the body of a cells environment",
        "\\colnames",
    ),
];

impl Lower for OutsideRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let element = element_of(&node).name.clone();
        let Some(&(_, _, detail, spelling)) =
            OUTSIDE_RULES.iter().find(|(name, ..)| *name == element.as_str())
        else {
            return Err(LowerError::structure("a command of the tables of cells without its rule"));
        };
        lw.diagnose(Diagnostic::error(OutsideTable { detail: detail.to_string() }, node.span().clone()));
        let content = match element.as_str() {
            CELL => Some(CONTENT_ARG),
            DATA => Some(DATA_ARG),
            _ => None,
        };
        let shown =
            content.filter(|argument| matches!(node.argument_content_nodes_named(argument), Ok(Some(_))));
        if let Some(argument) = shown {
            return Ok(Lowering::Emit(lw.lower_arg_named(cx, &node, argument)?));
        }
        let mark = DroppedPayload {
            condition: Name::from("flm.cells.outside-table"),
            detail: format!("{spelling} out of place"),
        };
        Ok(Lowering::Emit(vec![lw.pres_with(cx, &ElementName::from(pres::DROPPED), mark, Vec::new())?]))
    }
}

/// The general rules of the feature: the rule of the environment, reading
/// style tokens against `styles`, and one [`OutsideRule`] per command it
/// covers.
pub(super) fn rules(styles: Arc<Styles>) -> Vec<LoweringRule> {
    let mut rules = vec![
        LoweringRule::new(
            RULE,
            ENVIRONMENT,
            LoweringDecl::new().produces(pres::CELLS).produces(pres::PAR).produces(pres::DROPPED),
            Arc::new(CellsRule::new(styles)),
            DefaultMark::Everywhere,
        )
        .docs(
            "a table of cells lowers to `flm.pres.cells`, its payload the table's model and its \
             `cell` content slots the cells' content, after one dropped mark \
             (`flm.pres.dropped`) per thing left out",
        ),
    ];
    for (element, rule, _, _) in OUTSIDE_RULES {
        rules.push(
            LoweringRule::new(
                rule,
                element,
                LoweringDecl::new().produces(pres::DROPPED),
                Arc::new(OutsideRule),
                DefaultMark::Everywhere,
            )
            .docs(
                "a command of the tables of cells lowered outside its table is reported; its \
                 content is shown where the command is written, or a dropped mark is placed there",
            ),
        );
    }
    rules
}
