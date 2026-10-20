//! The eight elements of the tables of cells, their element declarations,
//! the feature's four scoped packages
//! ([`ScopedPackageDef`]; see
//! [scoped packages](crate::guide::concepts_overview#scoped-packages)), and
//! the predicate that decides the block role of four of the commands
//! ([`stands_in_body`], [`InBody`]).

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::lang::mint::{ArgDef, ArgLayout, ArgStdKind, BodyDef, ElementDef, ScopedPackageDef, SyntaxDef};
use crate::lang::Flm;
use crate::model::aspects::{BlockRole, BlockRoleAspect, FixedBlockRole};
use crate::model::element::{ElementSpec, element_of};
use crate::model::names::{FeatureName, Name};
use crate::stdcore::backends::lowering::rules::owning_callable;

use super::parser::CellStylesParser;
use super::{
    BODY_PACKAGE, CELL, COLNAMES, COLUMN, CONTENT_ARG, DATA, DATA_ARG, DATA_PACKAGE, ENVIRONMENT,
    ENVIRONMENT_PACKAGE, MAPPING_ARG, MERGE, MERGE_PACKAGE, NAMES_ARG, PLACEMENT_ARG, RANGE_ARG, ROW,
    ROWNAMES, STYLES_ARG,
};

/// Whether `node` is a direct child of the body of a `cells` environment:
/// its owner, the command or environment of whose body it is a direct child,
/// is an element named [`ENVIRONMENT`] (`flm.cells.cells`).
///
/// This predicate decides the block role of `\cell`, `\\`, `\rownames` and
/// `\colnames` ([`InBody`]). The table's reader
/// ([`read_grid`](super::read_grid)) does not call it, but reads the body's
/// direct children as its instructions, which are exactly the nodes for
/// which this predicate is true, so the block role and the reading cannot
/// disagree. The predicate reads the tree each time; nothing is stored.
pub fn stands_in_body(node: &NodeRef<'_, Flm>) -> bool {
    owning_callable(node).is_some_and(|owner| element_of(&owner).name.as_str() == ENVIRONMENT)
}

/// The block role of `\cell`, `\\`, `\rownames` and `\colnames`, decided by
/// position. The command is a block where it is a direct child of the body
/// of a `cells` environment ([`stands_in_body`]), as one of the table's
/// instructions. It is inline content everywhere else: inside the data of a
/// `\celldata`, where it is an entry's cell or ends a row, and inside a
/// cell's content, where it is out of place and reported when lowered
/// ([`OutsideTable`](super::diagnostics::OutsideTable)).
#[derive(Debug, Default, Clone, Copy)]
pub struct InBody;

impl BlockRoleAspect for InBody {
    fn block_role(&self, node: &NodeRef<'_, Flm>) -> BlockRole {
        if stands_in_body(node) { BlockRole::Block } else { BlockRole::DEFAULT }
    }
}

/// An element value whose block role is decided by position ([`InBody`]).
fn by_position(name: &str, docs: &str) -> Arc<ElementSpec> {
    Arc::new(ElementSpec::new(name).docs(docs).with::<dyn BlockRoleAspect>(Arc::new(InBody)))
}

/// An element value that is always a block.
fn block(name: &str, docs: &str) -> Arc<ElementSpec> {
    Arc::new(
        ElementSpec::new(name)
            .docs(docs)
            .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::Block))),
    )
}

/// An element value that is always inline content, the default block
/// role.
fn inline(name: &str, docs: &str) -> Arc<ElementSpec> {
    Arc::new(ElementSpec::new(name).docs(docs))
}

/// The element value of the environment, shared by its two declarations
/// (in the scoped package, and as an ordinary environment under
/// [`CellsConfig::global`](super::CellsConfig::global)): a block.
pub(super) fn environment_spec() -> Arc<ElementSpec> {
    block(ENVIRONMENT, "`\\begin{cells}…\\end{cells}`: a table of cells.")
}

/// One declaration of the environment `cells`, carrying `spec`: no argument,
/// and a body laid out as blocks that refers to the scoped package
/// [`BODY_PACKAGE`].
pub(super) fn environment_def(owner: &FeatureName, spec: &Arc<ElementSpec>) -> ElementDef {
    ElementDef::new(
        owner.clone(),
        ["cells"],
        SyntaxDef::Environment { args: Vec::new(), body: BodyDef::new().blocks().scoped(BODY_PACKAGE) },
        Arc::clone(spec),
    )
    .docs(
        "`\\begin{cells}…\\end{cells}`: a table of cells, filled with `\\cell`, `\\celldata` and `\\\\`, \
         its rows and columns optionally named with `\\rownames` and `\\colnames`.",
    )
}

/// The styles argument of `\cell` and `\celldata`: an optional `<…>` group
/// ([`CellStylesParser`]).
fn styles_arg() -> ArgDef {
    ArgDef::named(STYLES_ARG, CellStylesParser::new())
        .docs("Style tokens, separated by whitespace: a name, or a name with a value in braces.")
}

/// The commands of the body: `\cell`, `\celldata`, `\\`, `\rownames` and
/// `\colnames`.
fn body_commands(owner: &FeatureName) -> Vec<ElementDef> {
    let cell = ElementDef::new(
        owner.clone(),
        ["cell"],
        SyntaxDef::Macro {
            args: vec![
                styles_arg(),
                ArgDef::named(PLACEMENT_ARG, ArgStdKind::Optional).scoped(MERGE_PACKAGE).docs(
                    "Where the cell goes: `COL` or `ROW;COL`, each a number, a name, `.` or empty for \
                     the current one, or `\\merge{RANGE}` for a span.",
                ),
                ArgDef::named(CONTENT_ARG, ArgStdKind::Mandatory)
                    .layout(ArgLayout::Blocks)
                    .docs("The cell's content: paragraphs and blocks."),
            ],
        },
        by_position(CELL, "`\\cell<styles>[placement]{content}`: one cell of a table of cells."),
    )
    .docs("`\\cell<styles>[placement]{content}`: a cell, in the next column of the current row by default.");
    let data = ElementDef::new(
        owner.clone(),
        ["celldata"],
        SyntaxDef::Macro {
            args: vec![
                styles_arg(),
                ArgDef::named(MAPPING_ARG, ArgStdKind::Optional).scoped(MERGE_PACKAGE).docs(
                    "Where the data goes: `COLS` or `ROWS;COLS`, each a comma-separated list of \
                     indices, ranges and `\\merge{RANGE}`.",
                ),
                ArgDef::named(DATA_ARG, ArgStdKind::Mandatory)
                    .layout(ArgLayout::Inline)
                    .scoped(DATA_PACKAGE)
                    .docs("Rows ended by `\\\\`, entries separated by `&`; each entry inline content."),
            ],
        },
        block(DATA, "`\\celldata<styles>[mapping]{data}`: several cells of a table of cells, by rows."),
    )
    .docs("`\\celldata<styles>[mapping]{data}`: a cell per entry of the data, row by row.");
    let row = ElementDef::new(
        owner.clone(),
        ["\\"],
        SyntaxDef::Macro { args: Vec::new() },
        by_position(ROW, "`\\\\` in a table of cells: the next cell starts a new row."),
    )
    .docs("`\\\\`: the next cell starts a new row.");
    let names = |command: &str, element: &str, what: &str| {
        ElementDef::new(
            owner.clone(),
            [command],
            SyntaxDef::Macro {
                args: vec![ArgDef::named(NAMES_ARG, ArgStdKind::CharsGroup).docs(format!(
                    "Comma-separated names; the k-th is the name of {what} k, and an empty entry \
                     gives no name."
                ))],
            },
            by_position(element, &format!("`\\{command}{{names}}`: names for the {what}s of a table of cells.")),
        )
        .docs(format!(
            "`\\{command}{{names}}`: names that can be written wherever a {what} number can."
        ))
    };
    vec![cell, data, row, names("rownames", ROWNAMES, "row"), names("colnames", COLNAMES, "column")]
}

/// The feature's four scoped packages: the environment's
/// ([`ENVIRONMENT_PACKAGE`]), the body's ([`BODY_PACKAGE`]), the placements'
/// and mappings' ([`MERGE_PACKAGE`]) and the data's ([`DATA_PACKAGE`]).
pub(super) fn packages(owner: &FeatureName, environment: &Arc<ElementSpec>) -> Vec<ScopedPackageDef> {
    let merge = ElementDef::new(
        owner.clone(),
        ["merge"],
        SyntaxDef::Macro {
            args: vec![ArgDef::named(RANGE_ARG, ArgStdKind::CharsGroup).docs(
                "The rows or columns the cell spans: `2-4`, `2+3`, `1,2,3`, a start or an end left out.",
            )],
        },
        inline(MERGE, "`\\merge{range}`: a span of rows or of columns, in a cell's placement."),
    )
    .docs("`\\merge{range}`: the rows or the columns a cell spans.");
    let column = ElementDef::new(
        owner.clone(),
        Vec::<Name>::new(),
        SyntaxDef::Specials { trigger: String::from("&"), args: Vec::new() },
        inline(COLUMN, "`&` in the data of `\\celldata`: the end of an entry."),
    )
    .docs("`&`: ends an entry of the data.");
    vec![
        ScopedPackageDef {
            owner: owner.clone(),
            name: Name::from(ENVIRONMENT_PACKAGE),
            elements: vec![environment_def(owner, environment)],
            docs: String::from("The `cells` environment, in the bodies that accept a table of cells."),
        },
        ScopedPackageDef {
            owner: owner.clone(),
            name: Name::from(BODY_PACKAGE),
            elements: body_commands(owner),
            docs: String::from(
                "The instructions of a table of cells: `\\cell`, `\\celldata`, `\\\\`, `\\rownames` and \
                 `\\colnames`.",
            ),
        },
        ScopedPackageDef {
            owner: owner.clone(),
            name: Name::from(MERGE_PACKAGE),
            elements: vec![merge],
            docs: String::from("`\\merge`, in the placement of a cell and the mapping of `\\celldata`."),
        },
        ScopedPackageDef {
            owner: owner.clone(),
            name: Name::from(DATA_PACKAGE),
            elements: vec![column],
            docs: String::from("`&`, in the data of `\\celldata`."),
        },
    ]
}
