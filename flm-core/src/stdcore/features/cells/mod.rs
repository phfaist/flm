//! The `cells` feature ([`Cells`]): tables of cells. A table of cells is a
//! `cells` environment, a grid of rows and columns that an author fills one
//! cell at a time with `\cell`, or by rows with `\celldata`.
//!
//! ```text
//! \begin{cells}
//!   \celldata<H>{Name & City}          % a row of two column headers
//!   \celldata<l, r>{John & Berlin \\ Anna & Zurich}
//!   \cell<green>[\merge{1-2};3]{Both}  % rows 1 to 2, column 3
//! \end{cells}
//! ```
//!
//! The [language guide](crate::guide::language_syntax#tables-of-cells)
//! describes tables of cells for authors.
//!
//! # Where the environment can be written
//!
//! The `cells` environment is declared in the scoped package
//! [`ENVIRONMENT_PACKAGE`] (`flm.cells.environment`). A scoped package
//! ([`ScopedPackageDef`](crate::lang::mint::ScopedPackageDef)) is a set of
//! element declarations that can be written only inside the bodies and
//! arguments whose declarations refer to it (see
//! [scoped packages](crate::guide::concepts_overview#scoped-packages)). The
//! default `table` float type of the
//! [`floats`](crate::stdcore::features::floats) feature refers to this
//! package, so by default a table of cells can be written inside a `table`
//! float and nowhere else. Under the switch [`CellsConfig::global`], the same
//! element is also declared as an ordinary environment, so that a table can
//! be written wherever blocks can. Anywhere else, `\begin{cells}` is an
//! unknown environment, reported as any unknown environment is. A registry
//! that holds this feature without the `floats` feature holds, by default,
//! an element no document can write; this is not an error.
//!
//! # The body
//!
//! The environment's body is a sequence of instructions, laid out as blocks,
//! and holds nothing else; whitespace, blank lines and comments between the
//! instructions are ignored.
//!
//! - `\cell<styles>[placement]{content}` adds one cell. Its content is laid
//!   out as blocks: paragraphs, a list, a displayed formula, a nested table.
//!   By default the cell goes in the next column of the current row.
//! - `\celldata<styles>[mapping]{data}` adds a cell per entry of its data:
//!   rows ended by `\\`, entries separated by `&`, each entry inline content.
//!   An entry whose only content is a `\cell` is that cell, whose own styles
//!   and placement take precedence over the entry's; an empty entry adds no
//!   cell and leaves its position empty.
//! - `\\` ends the current row: the next cell starts the next row.
//! - `\rownames{…}` and `\colnames{…}` give the rows and the columns names:
//!   a comma-separated list whose *k*-th entry is the name of row or column
//!   *k*. A name is an ASCII letter followed by letters, digits or
//!   underscores, and can be written wherever a row or column number can,
//!   anywhere in the table.
//!
//! These commands can be written only in the body and, through it, in the
//! data of a `\celldata`; `&` only in that data; `\merge` only in a
//! placement or a mapping. Each set is a scoped package of its own
//! ([`BODY_PACKAGE`], [`DATA_PACKAGE`], [`MERGE_PACKAGE`]).
//!
//! # Placements, mappings and ranges
//!
//! Rows and columns are numbered from 1.
//!
//! - A **placement** (the optional argument of `\cell`) is `[COL]` or
//!   `[ROW;COL]`, each a number, a name, `.` or nothing for the current row
//!   or column, or `\merge{RANGE}` for a cell spanning several.
//! - A **range** is `2-4` (both included), `2+3` (three from 2), or
//!   `1,2,3` (a union, which must be contiguous). In `\merge`, a start left
//!   out is the first index, and an end left out is the last index of the
//!   table as finally built.
//! - A **mapping** (the optional argument of `\celldata`) is `[COLS]` or
//!   `[ROWS;COLS]`, each a comma-separated list of indices, ranges and
//!   `\merge{…}`, assigned to the data's entries in order. An open range
//!   (`2-`) continues one index per entry; with no rows given, the data's
//!   rows go from the current row on.
//!
//! # Style tokens
//!
//! The angle brackets hold style tokens separated by whitespace: a name, or
//! a name with a value in braces. Each token refers to a cell style
//! ([`CellStyle`], see the [`styles`] module) contributed to the service slot
//! [`STYLES_SLOT`]. In a `\celldata`, a comma separates the token lists of
//! successive entries of a row, the last list repeated. A cell's own tokens
//! come before its `\celldata`'s. Of the tokens that give an alignment, the
//! first decides, and likewise for the tokens that make the cell a header;
//! every edge line (a rule: `rule{…}`, `lvert`, `rvert`, a line drawn along
//! an edge of the cell) and every custom style is kept.
//!
//! # What the feature registers
//!
//! The feature ([`Cells`], under a [`CellsConfig`]) registers:
//!
//! - the eight elements of the tables of cells ([`ENVIRONMENT`], [`CELL`],
//!   [`DATA`], [`ROW`], [`COLUMN`], [`MERGE`], [`ROWNAMES`], [`COLNAMES`])
//!   and its four scoped packages; under [`CellsConfig::global`], the
//!   environment also as an ordinary environment;
//! - the content handler [`CellsContent`], as a peer contribution to the
//!   `floats` feature's service slot of content handlers ([`handler`]);
//! - the service slot [`STYLES_SLOT`] of cell styles
//!   ([`service_slot`](crate::core::registry::FeatureBuilder::service_slot)),
//!   with the cell styles it ships ([`shipped`]), and a build hook
//!   ([`Finalizer`](crate::core::registry::Finalizer)) that collects every
//!   cell style once the slot is assembled;
//! - the general lowering rules ([`lower`]) and the recomposition rule of the
//!   `latex-recomposed` backend ([`latex`]);
//! - the style definition [`STYLE`] for HTML ([`html`]) and the definition
//!   piece [`LATEX_PIECE`] for the recomposed LaTeX ([`latex`]); the
//!   definition piece of the rendered LaTeX, which delivers [`STYLE`]
//!   ([`latex_rendered`]);
//! - its settings: the fields of [`CellsConfig`] as data
//!   ([`crate::core::settings`]).
//!
//! # How a table is lowered
//!
//! The reader [`read_grid`] reads a table at lowering. Every rule that
//! lowers a table calls it, and nothing stores its result. Its result is the
//! payload of the presentation element
//! [`flm.pres.cells`](crate::stdcore::backends::pres::CELLS)
//! ([`CellsPayload`](crate::stdcore::backends::pres::CellsPayload)), the
//! grid's size and one entry per cell with the facts its tokens state,
//! together with each cell's content and the problems it found.
//!
//! - On every backend except `latex-recomposed`, the general rule
//!   ([`lower::CellsRule`]) stages one dropped mark (`flm.pres.dropped`) per
//!   thing left out, then the `flm.pres.cells` node, with one `cell` content
//!   slot per entry: a `\cell`'s blocks, or a `\celldata` entry's content as
//!   one paragraph with the whitespace at its edges trimmed. It requires, on
//!   the backend it runs on, the style definitions or definition pieces that
//!   the table's cell styles declare for that backend
//!   ([`CellStyle::requires`]). The text backend lays the table out in
//!   aligned columns; the HTML backend writes a `<table>` as the
//!   [HTML contract](crate::stdcore::backends::html::CONVENTIONS) states
//!   ([`html`]; see also the
//!   [rendering guide](crate::guide::rendering#the-html-contract)).
//! - On the `latex-recomposed` backend, the recomposition rule writes one
//!   call of the LaTeX hook `\flmCellsTable`, over the `tabularray` package
//!   ([`latex`]).
//! - A command of the tables of cells written outside its place is lowered
//!   by [`lower::OutsideRule`] on every backend.
//!
//! # Interactions with other features
//!
//! - The [`floats`](crate::stdcore::features::floats) feature's `table`
//!   float type refers to the scoped package [`ENVIRONMENT_PACKAGE`] and to
//!   the content handler `cells` ([`handler`]); without the `floats`
//!   feature, the handler is dropped with one Note, and the environment can
//!   be written only under [`CellsConfig::global`].
//! - Any feature may contribute cell styles of its own ([`CellStyle`]); the
//!   [guide on extending FLM](crate::guide::extending#a-cell-style-of-your-own)
//!   shows how.
//! - The LaTeX family (the [`latex`](crate::stdcore::backends::latex)
//!   helpers and the `latex-recomposed` backend) owns the constructs
//!   the recomposition rule stages; without them, that rule is set aside
//!   (excluded from rule selection; see
//!   [`PeerName`](crate::core::registry::PeerName)) and the registry still
//!   builds.
//!
//! # Diagnostics
//!
//! Every mistake is an Error with a recovery ([`diagnostics`]), reported each
//! time a rule lowers the table, and never under the `check` workflow
//! ([`CHECK_WORKFLOW`](crate::stdcore::CHECK_WORKFLOW)), which lowers
//! nothing. A dropped mark (`flm.pres.dropped`; see
//! [recovery](crate::stdcore::backends::lowering::error)) shows where
//! something was left out:
//!
//! | condition | recovery |
//! |---|---|
//! | `flm.cells.content-outside-cell` | the content is left out; a dropped mark is placed before the table |
//! | `flm.cells.bad-placement` | a placement is ignored, and the cell goes where the current row and column put it; a mapping is ignored whole |
//! | `flm.cells.bad-styles` | the styles argument is ignored |
//! | `flm.cells.unknown-style`, `flm.cells.bad-style-value` | the token is ignored, the others kept |
//! | `flm.cells.bad-names` | the entry is ignored, the others kept |
//! | `flm.cells.overlap` | the later cell is left out whole; a dropped mark is placed before the table |
//! | `flm.cells.too-large` | the cell that would make the table larger than a million positions is left out; a dropped mark is placed before the table |
//! | `flm.cells.empty` | nothing is written for the table, except the dropped marks of what was left out |
//! | `flm.cells.outside-table` | a `\cell`'s or a `\celldata`'s content is shown where the command is written; a command without content is left out, and a dropped mark is placed where it was |
//!
//! # Items of this module
//!
//! - The feature: [`Cells`], [`feature`], [`FEATURE`]; its configuration
//!   [`CellsConfig`].
//! - Names: the elements [`ENVIRONMENT`], [`CELL`], [`DATA`], [`ROW`],
//!   [`COLUMN`], [`MERGE`], [`ROWNAMES`], [`COLNAMES`]; the scoped packages
//!   [`ENVIRONMENT_PACKAGE`], [`BODY_PACKAGE`], [`MERGE_PACKAGE`],
//!   [`DATA_PACKAGE`]; the arguments [`STYLES_ARG`], [`PLACEMENT_ARG`],
//!   [`CONTENT_ARG`], [`MAPPING_ARG`], [`DATA_ARG`], [`RANGE_ARG`],
//!   [`NAMES_ARG`]; the service slot [`STYLES_SLOT`]; the definitions
//!   [`STYLE`] and [`LATEX_PIECE`].
//! - Reading a table: [`read_grid`], [`Grid`], [`CellContent`], [`Problem`]
//!   ([`reader`]); the block role by position: [`stands_in_body`],
//!   [`InBody`]; the styles argument's parser: [`CellStylesParser`]
//!   ([`parser`]).
//! - Cell styles: [`CellStyle`], [`Effect`], [`Parameter`], [`StyleForm`],
//!   [`StyleRequirement`], [`Styles`], [`shipped`] ([`styles`]).
//! - Lowering: [`lower`]; HTML: [`html`]; the recomposed LaTeX: [`latex`];
//!   the rendered LaTeX: [`latex_rendered`]; the floats content handler:
//!   [`CellsContent`] ([`handler`]).
//! - Conditions: [`diagnostics`].

pub mod diagnostics;
mod elements;
pub mod handler;
pub mod html;
pub mod latex;
pub mod latex_rendered;
pub mod lower;
pub mod parser;
pub mod reader;
pub mod styles;

use std::sync::Arc;

use crate::core::settings::SettingsDecl;
use crate::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use crate::core::services::Arity;
use crate::model::names::FeatureName;
use crate::stdcore::features::floats::{CONTENT_SLOT, FloatContentHandler};

pub use elements::{InBody, stands_in_body};
pub use handler::CellsContent;
pub use parser::CellStylesParser;
pub use reader::{CellContent, Grid, Problem, read_grid};
pub use styles::{CellStyle, Effect, Parameter, StyleForm, StyleRequirement, Styles, shipped};

/// The feature's registered name, `cells`.
pub const FEATURE: &str = "cells";

/// The declaration of the feature's settings: the fields of [`CellsConfig`]
/// are its keys. See [`crate::core::settings`].
fn settings() -> SettingsDecl {
    SettingsDecl::new(FEATURE)
        .config::<CellsConfig>(
            "Whether the `cells` environment can be written wherever blocks can (`global`, false \
            by default), in addition to the bodies that refer to the scoped package \
            `flm.cells.environment`, such as a `table` float's. Fixed when the registry is \
            built: it cannot vary per document or per render.",
        )
}

/// The element of the `cells` environment, `flm.cells.cells`: a table of
/// cells.
pub const ENVIRONMENT: &str = "flm.cells.cells";

/// The element of `\cell`, `flm.cells.cell`: one cell.
pub const CELL: &str = "flm.cells.cell";

/// The element of `\celldata`, `flm.cells.data`: a cell per entry of its data.
pub const DATA: &str = "flm.cells.data";

/// The element of `\\` in a table of cells, `flm.cells.row`: the end of a
/// row.
pub const ROW: &str = "flm.cells.row";

/// The element of `&` in the data of `\celldata`, `flm.cells.column`: the end
/// of an entry.
pub const COLUMN: &str = "flm.cells.column";

/// The element of `\merge`, `flm.cells.merge`: a span of rows or of columns.
pub const MERGE: &str = "flm.cells.merge";

/// The element of `\rownames`, `flm.cells.rownames`.
pub const ROWNAMES: &str = "flm.cells.rownames";

/// The element of `\colnames`, `flm.cells.colnames`.
pub const COLNAMES: &str = "flm.cells.colnames";

/// The scoped package that holds the `cells` environment,
/// `flm.cells.environment`: a table of cells can be written in a body whose
/// declaration refers to it. A float type of the `floats` feature refers to
/// it in its configuration; the feature declares the package under every
/// configuration.
pub const ENVIRONMENT_PACKAGE: &str = "flm.cells.environment";

/// The scoped package of the environment's body, `flm.cells.body`: `\cell`,
/// `\celldata`, `\\`, `\rownames` and `\colnames`, which can be written
/// only there and, through it, in the data of a `\celldata`.
pub const BODY_PACKAGE: &str = "flm.cells.body";

/// The scoped package of a placement and a mapping, `flm.cells.merge`:
/// `\merge`.
pub const MERGE_PACKAGE: &str = "flm.cells.merge";

/// The scoped package of the data of `\celldata`, `flm.cells.data`: `&`.
pub const DATA_PACKAGE: &str = "flm.cells.data";

/// The name of the styles argument of `\cell` and `\celldata`, `styles`.
pub const STYLES_ARG: &str = "styles";

/// The name of the placement argument of `\cell`, `placement`.
pub const PLACEMENT_ARG: &str = "placement";

/// The name of the content argument of `\cell`, `content`.
pub const CONTENT_ARG: &str = "content";

/// The name of the mapping argument of `\celldata`, `mapping`.
pub const MAPPING_ARG: &str = "mapping";

/// The name of the data argument of `\celldata`, `data`.
pub const DATA_ARG: &str = "data";

/// The name of the range argument of `\merge`, `range`.
pub const RANGE_ARG: &str = "range";

/// The name of the argument of `\rownames` and `\colnames`, `names`.
pub const NAMES_ARG: &str = "names";

/// The name of the service slot of cell styles, `flm.cells.styles`: every
/// [`CellStyle`] that a table's tokens may refer to, in the order of
/// contribution ([`Arity::OrderedMany`]). A service slot is the declaration, by
/// the feature that owns an interaction, of a dotted name, the type of the
/// values the slot takes and its arity; any feature may contribute values to
/// it (see [services](crate::core::services)). The
/// `cells` feature contributes the cell styles it ships ([`shipped`]); any
/// feature may contribute its own, as a peer contribution.
pub const STYLES_SLOT: &str = "flm.cells.styles";

/// The identifier of the feature's style definition for HTML,
/// `flm.cells.style` ([`html::stylesheet`]). The HTML contract's row for
/// `flm.pres.cells` refers to it, and the shipped custom styles require it on
/// HTML.
pub const STYLE: &str = "flm.cells.style";

/// The identifier of the feature's definition piece for the recomposed
/// LaTeX, `flm.latex.def.cells` ([`latex::definition_piece`]), which the
/// recomposition rule of a table requires, and which the shipped custom
/// styles require on that backend.
pub const LATEX_PIECE: &str = "flm.latex.def.cells";

/// The configuration of the `cells` feature: whether the `cells` environment
/// is also an ordinary environment of the document.
///
/// Under the default, `global: false`, the environment can be written only
/// inside the bodies whose declarations refer to the scoped package
/// [`ENVIRONMENT_PACKAGE`]: a `table` float's, where the `floats` feature is
/// registered. Under `global: true` it is also an ordinary environment,
/// written wherever blocks can be: between paragraphs, in a list item, in a
/// footnote.
///
/// The value is the feature's configuration (see
/// [configuration](crate::guide::concepts_overview#settings-options-and-configuration)):
/// fixed when the registry is built, since it decides which environments the
/// registry's parsing language ([`BuiltLang`](crate::lang::BuiltLang))
/// holds, and fed into the registry digest
/// ([`digest_bytes`](CellsConfig::digest_bytes)). The standard package
/// carries it as [`StdcoreConfig::cells`](crate::stdcore::StdcoreConfig::cells).
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::cells::CellsConfig;
///
/// assert!(!CellsConfig::new().global);
/// assert!(CellsConfig::new().global(true).global);
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, serde::Deserialize)]
#[derive(schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct CellsConfig {
    /// Whether the `cells` environment is an ordinary environment of the
    /// document, in addition to its declaration in the scoped package
    /// [`ENVIRONMENT_PACKAGE`]. `false` by default.
    pub global: bool,
}

impl CellsConfig {
    /// The default configuration: the environment is declared in its scoped
    /// package only.
    pub fn new() -> CellsConfig {
        CellsConfig::default()
    }

    /// Builder: sets [`global`](CellsConfig::global).
    pub fn global(mut self, global: bool) -> CellsConfig {
        self.global = global;
        self
    }

    /// The bytes the feature feeds into the registry digest: one byte, `1`
    /// under `global` and `0` otherwise.
    pub fn digest_bytes(&self) -> Vec<u8> {
        // Taken apart by name, so a field added and not written here fails
        // to compile.
        let CellsConfig { global } = self;
        vec![u8::from(*global)]
    }
}

/// The `cells` feature ([`FlmFeature`]). Under a [`CellsConfig`], it
/// registers the `cells` environment in its scoped package and, under
/// [`global`](CellsConfig::global), as an ordinary environment; the commands
/// of its body and their scoped packages; the content handler for the
/// `floats` feature; the service slot of cell styles with the shipped ones;
/// the lowering rules; the HTML style definition and the LaTeX definition
/// piece. See the [module documentation](self#what-the-feature-registers).
#[derive(Debug, Default, Clone, Copy)]
pub struct Cells {
    config: CellsConfig,
}

impl Cells {
    /// The feature under `config`, which is fixed for the life of the
    /// registry it is built into (see
    /// [configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
    pub fn new(config: CellsConfig) -> Cells {
        Cells { config }
    }

    /// The configuration this feature was constructed with.
    pub fn config(&self) -> &CellsConfig {
        &self.config
    }
}

impl FlmFeature for Cells {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Tables of cells").docs(
            "The `cells` environment, inside a `table` float or, under `global`, wherever blocks \
             can be written: a grid filled with `\\cell` and `\\celldata`, cells spanning rows or \
             columns with `\\merge`, names for rows and columns with `\\rownames` and \
             `\\colnames`, and style tokens referring to the cell styles of the service slot \
             `flm.cells.styles`.",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(settings())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner: FeatureName = b.feature().clone();
        let environment = elements::environment_spec();
        for package in elements::packages(&owner, &environment) {
            b.scoped_package(package)?;
        }
        if self.config.global {
            // The same element, as an ordinary environment: which declaration
            // answers inside a body naming the package is immaterial.
            // See decision rationale at [§flm-dd-dr:std-cells-global].
            b.element(elements::environment_def(&owner, &environment))?;
        }
        // Where the floats feature is absent, nobody declares the slot and
        // the contribution is dropped with a Note.
        b.peer_slot::<dyn FloatContentHandler>(CONTENT_SLOT, Arc::new(CellsContent))?;
        b.service_slot::<CellStyle>(
            STYLES_SLOT,
            Arity::OrderedMany,
            "the cell styles a cell's style tokens may refer to: the ones the `cells` feature \
             ships and the ones other features contribute",
        )?;
        for style in shipped() {
            b.contribute_to::<CellStyle>(STYLES_SLOT, Arc::new(style))?;
        }
        let styles = Arc::new(Styles::default());
        b.finalizer(Arc::new(styles::CollectStyles { styles: Arc::clone(&styles) }))?;
        for rule in lower::rules(Arc::clone(&styles)) {
            b.lowering_rule(rule)?;
        }
        html::register(b)?;
        latex_rendered::register(b)?;
        latex::register(b, styles)?;
        b.digest_input(&self.config.digest_bytes())?;
        Ok(())
    }
}

/// The [`Cells`] feature under the default configuration, ready to be
/// listed in a package entry.
pub fn feature() -> Arc<dyn FlmFeature> {
    Arc::new(Cells::default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_configuration_digests_its_one_switch() {
        assert_eq!(CellsConfig::new().digest_bytes(), [0]);
        assert_eq!(CellsConfig::new().global(true).digest_bytes(), [1]);
    }

    #[test]
    fn the_styles_parser_answers_what_the_language_asks() {
        use crate::lang::mint::{DefinitionShape, FlmArgumentParser};
        use techy::core::constructs::ArgumentParser;
        let parser = CellStylesParser::new();
        assert!(parser.holds_content());
        assert_eq!(parser.identity(), b"flm.cells.styles");
        assert_eq!(parser.definition_shape(), DefinitionShape::NoShape { token: "cell-styles" });
        assert_eq!(parser.description(), "an optional `<…>` group of style tokens");
        assert!(parser.can_match_empty());
    }
}
