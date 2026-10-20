//! The stylesheets and the script this backend ships, and the stage that
//! decides how each style or script definition a document requires reaches the
//! page.
//!
//! A style or script definition ([`StyleDef`], [`ScriptDef`](crate::core::registry::ScriptDef))
//! is a stylesheet or a script that a feature's output depends on, recorded in
//! the registry under an identifier; a lowering rule or the emitter *requires*
//! it by identifier, and the backend that writes the output decides its
//! *delivery*: written into the output, loaded from an address, or left to the
//! host (see [style and script
//! definitions](crate::guide::concepts_overview#style-and-script-definitions)).
//! On this backend the delivery is decided here:
//!
//! 1. Each tree's emission and the emission of each
//!    [piece](crate::core::piece::Piece) (a document-level value placed beside
//!    the content) record the identifiers
//!    their HTML requires (`flm.output.html`, `flm.output.html.pieces`).
//! 2. The stage [`ResolveDefinitionsStage`] collects them, adds the identifiers
//!    the host requires for the whole page (the render option
//!    [`required_definitions`](super::HtmlRenderOptions::required_definitions))
//!    and the page's own stylesheet [`STYLE_PAGE`], and looks each identifier up
//!    among the registry's style and script definitions.
//! 3. For each one, the delivery is the definition's declared content, unless
//!    the render option [`definitions`](super::HtmlRenderOptions::definitions)
//!    gives another ([`DefinitionOverride`]). A part of the declared content
//!    supplied by a table ([`DefContent::Table`]) is replaced by the text that
//!    table holds for the document, as an [`Inline`](DefContent::Inline) part.
//! 4. The stage writes the result into one table, [`DEFINITIONS_TABLE`]
//!    (`flm.html.definitions`): every definition the document requires, once
//!    each, with its kind, the feature that declared it and its delivery.
//!
//! The regions stage reads that table, and nothing else, to write the page's
//! stylesheets and scripts ([`super::page`]). A host that assembles a page of
//! its own reads the same table, and a bundler reads its serialized form
//! ([`Table::project`]).
//!
//! # What this backend ships
//!
//! Seven stylesheets, one per concern, so that a page carries only what its
//! content needs (a document with no mathematics loads no rules for
//! mathematics):
//!
//! | identifier | what it styles |
//! |---|---|
//! | `flm.html.style.core` | paragraphs, blocks, spans, links, images, decorations |
//! | `flm.html.style.headings` | the six heading sizes |
//! | `flm.html.style.verbatim` | content set character for character |
//! | `flm.html.style.math-passthrough` | a displayed formula, and content written inside a formula |
//! | `flm.html.style.lists` | lists and their labels; a run of lines |
//! | `flm.html.style.errors` | what the page shows where something could not be produced |
//! | `flm.html.style.page` | the page's own typography and the width of its content |
//!
//! Which element requires which stylesheet is stated by the [conventions
//! table](super::conventions). No element requires `flm.html.style.page`: the
//! resolve stage adds it, because the page this backend assembles needs it. A
//! host that builds a page of its own around the HTML of each tree can leave
//! that row out.
//!
//! **The first line of every text this backend ships is a comment holding the
//! identifier of the definition it belongs to**: the line
//! `/* flm.html.style.core */`, the line `/* flm.html.mathjax */`, before any
//! other comment and before the first rule. A page written by this backend
//! also records that identifier as an attribute of the `<style>` or `<script>`
//! element holding the text ([`DEF_ATTRIBUTE`](super::page::DEF_ATTRIBUTE)),
//! but a text passed on by itself carries no attribute. The comment lets a
//! reader that sees only the text (a tool that reads a finished page back, a
//! host's build step, a person reading the bytes) tell which definition it
//! has, and in particular tell `flm.html.style.page`, whose selectors are not
//! all classes of the [HTML contract](super::CONVENTIONS), from the others.
//!
//! Each stylesheet is one `@layer flm { … }` block. A **cascade layer** is a
//! named group of CSS rules; a rule outside every layer wins over a rule inside
//! one, regardless of their selectors. A host therefore restyles this backend's
//! output with ordinary rules and never needs a more specific selector than the
//! shipped one. Inside the layer, each stylesheet declares on
//! `:root` the custom properties (`--flm-…`) it uses; a host that only wants a
//! different value sets that property instead of replacing a rule. Every
//! selector begins with a class of the HTML contract, so embedding the output
//! in a host's page restyles nothing of the host's; `flm.html.style.page` is
//! the only exception, and says so in its own text.
//!
//! One script, `flm.html.mathjax` ([`mathjax_script`]): the formula renderer,
//! in two parts, its configuration and then the address its program is loaded
//! from. It is declared in [its own module](super::mathjax), which also says
//! what a host that does not trust the authors of the mathematics it renders
//! must configure for itself.
//!
//! # Conditions
//!
//! The resolve stage reports [`UnknownRequirement`] for a required identifier
//! that no feature declares, [`UnknownDefinition`] for an entry of the
//! `definitions` option that no feature declares, and [`DefinitionPartMissing`]
//! for a part supplied by a table that holds no text. The regions stage reports
//! [`DefinitionNotInlinable`] for a part of a definition it cannot write into
//! the page.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use techy::error::{Diagnostic, DiagnosticInfo, DiagnosticValue, ToDiagnosticValue};
use techy::serialize::SerialValue;

use crate::core::data::store::StageOut;
use crate::core::data::table::{
    PieceClass, ProjectCx, SingletonTable, Table, TableHandle, TableName,
};
use crate::core::plan::PlanCx;
use crate::core::registry::{
    BuildError, DefContent, DefinitionText, FeatureBuilder, FlmRegistry, StyleDef,
};
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::{FlmOrigin, nowhere};
use crate::model::names::{CapName, FeatureName, Name};

use super::conventions::{
    STYLE_CORE, STYLE_ERRORS, STYLE_HEADINGS, STYLE_LISTS, STYLE_MATH_PASSTHROUGH, STYLE_VERBATIM,
};
use super::emit::NOWHERE_LABEL;
use super::feature::{DefinitionOverride, HtmlRenderOptions};
use super::mathjax::mathjax_script;
use super::stages::{OUTPUT_TABLE, PIECES_TABLE, stage_failed};

/// The name of the stage that resolves the definitions,
/// `flm.html.resolve-definitions` ([`ResolveDefinitionsStage`]).
pub const RESOLVE_STAGE: &str = "flm.html.resolve-definitions";

/// The typed name of the definitions table, `flm.html.definitions`: every
/// style and script definition the document requires, each with its delivery
/// ([`ResolvedDefinitions`]).
pub const DEFINITIONS_TABLE: TableName<ResolvedDefinitions> =
    TableName::new("flm.html.definitions");

/// The style definition of the page's own typography and the width of its
/// content, `flm.html.style.page`.
///
/// No element requires it: it styles the page this backend assembles, and the
/// resolve stage ([`ResolveDefinitionsStage`]) adds it to what the document's
/// content required. It is the only stylesheet whose selectors are not all
/// classes of the [HTML contract](super::CONVENTIONS), because a page has a
/// `body` and a document does not.
pub const STYLE_PAGE: &str = "flm.html.style.page";

// --- what this backend ships ------------------------------------------------------------

/// The seven stylesheets this backend ships: the identifier, the text, and the
/// description recorded with the definition.
///
/// The order is the order in which they are registered, which is the order in
/// which the definitions table lists them and a page writes them.
const STYLESHEETS: [(&str, &str, &str); 7] = [
    (
        STYLE_CORE,
        include_str!("css/core.css"),
        "paragraphs, blocks, the display styles of a span, links, images and the three \
         decorations",
    ),
    (STYLE_HEADINGS, include_str!("css/headings.css"), "the six heading sizes"),
    (
        STYLE_VERBATIM,
        include_str!("css/verbatim.css"),
        "content set character for character, inline and as a block",
    ),
    (
        STYLE_MATH_PASSTHROUGH,
        include_str!("css/math-passthrough.css"),
        "a displayed formula, and content written inside a formula",
    ),
    (
        STYLE_LISTS,
        include_str!("css/lists.css"),
        "lists with their entries and labels, and a run of lines with its alignment",
    ),
    (
        STYLE_ERRORS,
        include_str!("css/errors.css"),
        "what the page shows where the document asked for something that could not be \
         produced",
    ),
    (
        STYLE_PAGE,
        include_str!("css/page.css"),
        "the page's own typography and the width the content is held to; required by the \
         page this backend assembles and by no element",
    ),
];

/// Returns the seven style definitions this backend ships, in the order in
/// which they are registered, which is the order in which the definitions table
/// lists them and a page writes them.
///
/// The content of each is one part, its text ([`DefContent::Inline`]). The
/// [module documentation](self#what-this-backend-ships) lists them and the
/// rules their texts follow.
pub fn stylesheets() -> Vec<StyleDef> {
    STYLESHEETS
        .iter()
        .map(|(id, text, docs)| StyleDef {
            id: Name::from(*id),
            content: vec![DefContent::Inline(String::from(*text))],
            docs: String::from(*docs),
        })
        .collect()
}

/// Registers this backend's seven style definitions, the formula renderer's
/// script definition, the definitions table and the resolve stage. The `html`
/// feature ([`super::HtmlBackend`]) calls it from its own registration.
///
/// # Errors
///
/// Never fails: the feature builder accepts each of these registrations, and a
/// name clash is reported when the registry is built. The `Result` lets the
/// `html` feature's registration call this function with `?`.
pub fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    for style in stylesheets() {
        b.style_def(style)?;
    }
    b.script_def(mathjax_script())?;
    b.table(
        DEFINITIONS_TABLE,
        "the stylesheets and scripts the page requires, each with how it is delivered",
    )?;
    b.stage(Arc::new(ResolveDefinitionsStage))?;
    Ok(())
}

// --- the table --------------------------------------------------------------------------

/// Whether a row of [`ResolvedDefinitions`] is a stylesheet or a script.
///
/// The kind decides where the page writes the definition: a stylesheet in the
/// head, a script at the end of the body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DefinitionKind {
    /// A stylesheet.
    Style,
    /// A script.
    Script,
}

impl DefinitionKind {
    /// Returns the kind's stable name, `style` or `script`, as the table's
    /// projection writes it.
    pub fn as_str(self) -> &'static str {
        match self {
            DefinitionKind::Style => "style",
            DefinitionKind::Script => "script",
        }
    }
}

/// One row of the definitions table: a style or script definition the document
/// requires, with the delivery it ended up with.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedDefinition {
    /// The definition's identifier, as the requirement gave it.
    pub id: Name,
    /// Whether it is a stylesheet or a script.
    pub kind: DefinitionKind,
    /// The feature that declared it.
    pub owner: FeatureName,
    /// How it reaches the page: the parts written for it, in order, or the
    /// statement that the host's own page or bundle supplies it and the page
    /// writes nothing. This is the declared content unless the render option
    /// [`definitions`](super::HtmlRenderOptions::definitions) put something
    /// else in its place.
    pub delivery: DefinitionOverride,
}

/// The definitions table, `flm.html.definitions` ([`DEFINITIONS_TABLE`]):
/// every style and script definition the document requires, once each, with
/// its delivery.
///
/// A requirement is only an identifier; this table records, for each
/// identifier, how the definition reaches the page. The regions stage reads it
/// and reads nothing else about definitions. A host that assembles its own page
/// around the per-tree HTML of `flm.output.html` reads it too, including the
/// rows whose delivery is [`ProvidedByHost`](DefinitionOverride::ProvidedByHost),
/// so that it knows what the document needs; a bundler works from its
/// serialized form ([`Table::project`]). The stage [`ResolveDefinitionsStage`]
/// writes it once per document.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResolvedDefinitions {
    /// The rows: the stylesheets first, in the order in which the features
    /// registered them, then the scripts in the same way.
    pub definitions: Vec<ResolvedDefinition>,
}

impl ResolvedDefinitions {
    /// Returns the row of the definition `id`, or `None` when the document
    /// does not require it.
    pub fn get(&self, id: &str) -> Option<&ResolvedDefinition> {
        self.definitions.iter().find(|definition| definition.id.as_str() == id)
    }

    /// Returns the rows of one kind, in order: what the page writes in the
    /// head (stylesheets) or at the end of the body (scripts).
    pub fn of_kind(&self, kind: DefinitionKind) -> impl Iterator<Item = &ResolvedDefinition> {
        self.definitions.iter().filter(move |definition| definition.kind == kind)
    }
}

impl Table for ResolvedDefinitions {
    /// The stage writes the table once, for the whole document, so a
    /// non-empty table piece replaces what the table held.
    fn absorb(&mut self, piece: ResolvedDefinitions) {
        if !piece.definitions.is_empty() {
            self.definitions = piece.definitions;
        }
    }

    /// One entry per row: `{id, kind, owner, provided_by_host, parts}`, each
    /// part one of `{inline}`, `{file}`, `{url}` or `{table}`. A bundler or a site
    /// generator works from this list.
    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.definitions
                .iter()
                .map(|definition| {
                    let (provided, parts) = match &definition.delivery {
                        DefinitionOverride::ProvidedByHost => (true, Vec::new()),
                        DefinitionOverride::Parts(parts) => (false, parts.clone()),
                    };
                    SerialValue::Map(vec![
                        (String::from("id"), SerialValue::Str(definition.id.to_string())),
                        (
                            String::from("kind"),
                            SerialValue::Str(String::from(definition.kind.as_str())),
                        ),
                        (String::from("owner"), SerialValue::Str(definition.owner.to_string())),
                        (String::from("provided_by_host"), SerialValue::Bool(provided)),
                        (
                            String::from("parts"),
                            SerialValue::List(parts.iter().map(project_part).collect()),
                        ),
                    ])
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        if self.definitions.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

impl SingletonTable for ResolvedDefinitions {
    const NAME: &'static str = "flm.html.definitions";
}

/// One part of a delivery, projected: `{inline}`, `{file}`, `{url}` or
/// `{table}`. The stage puts a table's text in the place of a `table` part, so
/// a row it wrote holds none; a table built by other means may.
fn project_part(part: &DefContent) -> SerialValue {
    let (key, value) = match part {
        DefContent::Inline(text) => ("inline", text.clone()),
        DefContent::File(path) => ("file", path.clone()),
        DefContent::Url(address) => ("url", address.clone()),
        DefContent::Table(table) => ("table", table.to_string()),
    };
    SerialValue::Map(vec![(String::from(key), SerialValue::Str(value))])
}

// --- conditions -------------------------------------------------------------------------

/// Condition `flm.html.unknown-requirement` (Error): an identifier that a
/// lowering rule or the emitter required, or that the render option
/// [`required_definitions`](super::HtmlRenderOptions::required_definitions)
/// lists, is not the identifier of any style or script definition in the
/// registry.
///
/// It reports a mistake in a feature, in the host's render option
/// `required_definitions`, or in the way the registry was assembled, and never
/// anything a document can write. Reported by
/// [`ResolveDefinitionsStage`], at no position in a document. Recovery: the
/// requirement is dropped (nothing is written for it, and it is left out of the
/// definitions table) and the rest of the page is produced as usual.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.html.unknown-requirement",
    message = "the HTML page requires the stylesheet or script `{definition}` (for the document's \
               content, for the page itself, or through the render option \
               `required_definitions`), but no registered feature declares a stylesheet or \
               script with that identifier"
)]
pub struct UnknownRequirement {
    /// The identifier that was required.
    pub definition: String,
}

/// Condition `flm.html.unknown-definition` (Warning): an entry of the render
/// option [`definitions`](super::HtmlRenderOptions::definitions) has an
/// identifier that is not the identifier of any style or script definition in
/// the registry.
///
/// Reported by [`ResolveDefinitionsStage`], at no position in a document.
/// Recovery: that entry of the option is ignored. It is a Warning rather than
/// an Error because nothing about the page changes: the likely cause is a
/// misspelled identifier, or a host carrying settings for a feature the
/// registry does not hold.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.html.unknown-definition",
    message = "the render option `definitions` lists `{definition}`, but no registered feature \
               declares a stylesheet or script with that identifier"
)]
pub struct UnknownDefinition {
    /// The identifier the option listed.
    pub definition: String,
}

/// Condition `flm.html.definition-part-missing` (Error): a part of a style or
/// script definition the document requires is supplied by a table
/// ([`DefContent::Table`]), and the run holds no text for it: the table is
/// absent (no stage of the plan wrote it) or holds an empty text.
///
/// It reports a mistake in the feature that declares the definition (its stage
/// did not run or wrote nothing), never anything a document can write.
/// Reported by [`ResolveDefinitionsStage`], at no position in a document.
/// Recovery: the part is empty; the definition's other parts, and the rest of
/// the page, are written as usual.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.html.definition-part-missing",
    message = "the {kind} `{definition}` takes a part from the table `{table}`, but {condition}"
)]
pub struct DefinitionPartMissing {
    /// What the definition is, `stylesheet` or `script`.
    pub kind: String,
    /// The definition's identifier.
    pub definition: String,
    /// The table the part names.
    pub table: String,
    /// What the run found: `no stage wrote that table` where the table is
    /// absent, or ``the table `<name>` holds no text`` where it holds an
    /// empty text.
    pub condition: String,
}

/// The part of a style or script definition that could not be written into the
/// page, and why: the reason carried by [`DefinitionNotInlinable`].
///
/// Three kinds of part cannot be written into the page: a file, a text
/// holding the closing tag of the element it would be written into, and a part
/// still naming the table that supplies its text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NotInlinablePart {
    /// A part that is a **file** ([`DefContent::File`]). This library opens no
    /// file: the host reads it and delivers its content itself, through the
    /// render option [`definitions`](super::HtmlRenderOptions::definitions).
    File {
        /// The file, as the definition's part gives it.
        path: String,
    },
    /// A part of **text** holding the closing tag of the element it would be
    /// written into (`</style` or `</script`), which would end that element
    /// early and leave the rest of the definition in the page as markup.
    ClosingTag {
        /// The tag of the element the text would have gone into: `style` or
        /// `script`.
        tag: String,
        /// The byte offset of the closing tag in the text.
        at: usize,
    },
    /// A part supplied by a **table** ([`DefContent::Table`]) that reached
    /// the page as such: the resolve stage puts the table's text in its place,
    /// so only a definitions table built by other means holds one.
    Table {
        /// The table the part names.
        table: String,
    },
}

impl NotInlinablePart {
    /// Returns the stable token that identifies the case in the diagnostic's
    /// data: `file` for [`File`](Self::File), `closing-tag` for
    /// [`ClosingTag`](Self::ClosingTag), `table` for [`Table`](Self::Table).
    ///
    /// A test or a tool matches on this token rather than on the message,
    /// whose wording is not a stable interface.
    pub fn kind(&self) -> &'static str {
        match self {
            NotInlinablePart::File { .. } => "file",
            NotInlinablePart::ClosingTag { .. } => "closing-tag",
            NotInlinablePart::Table { .. } => "table",
        }
    }
}

impl std::fmt::Display for NotInlinablePart {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NotInlinablePart::File { path } => {
                write!(
                    f,
                    "the part is the file `{path}`, and this library does not read files; in the \
                     render option `definitions`, give the file's content as text, or an address \
                     the page loads, or mark the stylesheet or script as provided by the host's \
                     own page"
                )
            }
            NotInlinablePart::ClosingTag { tag, at } => write!(
                f,
                "the part's text contains the closing tag `</{tag}` (in any letter case) at byte \
                 {at}, which would end the `<{tag}>` element it is written into"
            ),
            NotInlinablePart::Table { table } => write!(
                f,
                "the part must be the text of the table `{table}`, and only the stage that \
                 resolves the page's stylesheets and scripts reads that table"
            ),
        }
    }
}

/// Diagnostic data: a map holding the [`kind`](NotInlinablePart::kind) token
/// and that case's own facts (`path`, or `tag` and `at`), never the message
/// text, so that a test or a tool reading the data depends on no wording.
impl ToDiagnosticValue for NotInlinablePart {
    fn to_diagnostic_value(&self) -> DiagnosticValue {
        let mut fields = vec![(String::from("kind"), self.kind().to_diagnostic_value())];
        match self {
            NotInlinablePart::File { path } => {
                fields.push((String::from("path"), path.to_diagnostic_value()));
            }
            NotInlinablePart::ClosingTag { tag, at } => {
                fields.push((String::from("tag"), tag.to_diagnostic_value()));
                fields.push((String::from("at"), at.to_diagnostic_value()));
            }
            NotInlinablePart::Table { table } => {
                fields.push((String::from("table"), table.to_diagnostic_value()));
            }
        }
        DiagnosticValue::Map(fields)
    }
}

/// Condition `flm.html.definition-not-inlinable` (Error): a part of a style or
/// script definition the document requires cannot be written into the page,
/// for the reason [`NotInlinablePart`] gives.
///
/// It reports a mistake in a feature's declaration or in the host's settings,
/// never anything a document can write. Reported by the regions stage
/// ([`HtmlRegionsStage`](super::stages::HtmlRegionsStage)), at no position in a
/// document. Recovery: nothing is written for that part; the definition's other
/// parts, and the rest of the page, are written as usual.
///
/// One condition serves both cases because the recovery is the same;
/// [`part`](Self::part) says which case occurred and carries its facts, so that
/// a host reads them rather than the message.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.html.definition-not-inlinable",
    message = "a part of the stylesheet or script `{definition}` cannot be written into the \
               page: {part}"
)]
pub struct DefinitionNotInlinable {
    /// The definition's identifier.
    pub definition: String,
    /// The part that could not be written, and why.
    pub part: NotInlinablePart,
}

// --- the stage --------------------------------------------------------------------------

/// The tables named by a part of a registered style or script definition
/// ([`DefContent::Table`]), each once, in the order the definitions name them.
fn part_tables(registry: &FlmRegistry) -> Vec<CapName> {
    let parts = registry
        .styles()
        .iter()
        .flat_map(|(_, style)| &style.content)
        .chain(registry.scripts().iter().flat_map(|(_, script)| &script.content));
    let mut tables: Vec<CapName> = Vec::new();
    for part in parts {
        if let DefContent::Table(table) = part {
            if !tables.contains(table) {
                tables.push(table.clone());
            }
        }
    }
    tables
}

/// One part of a definition's declared content, as the page receives it: a
/// [`DefContent::Table`] part becomes the table's text as an
/// [`Inline`](DefContent::Inline) part, empty with
/// [`DefinitionPartMissing`] reported where the table is absent or holds no
/// text, and with [`StageFailed`](super::stages::StageFailed) reported where
/// it cannot be read; any other part is kept.
fn resolve_part(
    cx: &StageCx<'_>,
    out: &mut StageOut<'_>,
    kind: DefinitionKind,
    definition: &str,
    part: &DefContent,
) -> DefContent {
    let DefContent::Table(table) = part else { return part.clone() };
    let found = match cx.get(TableHandle::<DefinitionText>::new(table.clone())) {
        Ok(found) => found.map(|found| found.text.clone()),
        Err(error) => {
            stage_failed(out, RESOLVE_STAGE, error);
            return DefContent::Inline(String::new());
        }
    };
    let condition = match &found {
        None => String::from("no stage wrote that table"),
        Some(text) if text.is_empty() => format!("the table `{table}` holds no text"),
        Some(_) => String::new(),
    };
    if !condition.is_empty() {
        let kind = match kind {
            DefinitionKind::Style => "stylesheet",
            DefinitionKind::Script => "script",
        };
        out.diags.push(Diagnostic::error(
            DefinitionPartMissing {
                kind: String::from(kind),
                definition: definition.to_string(),
                table: table.to_string(),
                condition,
            },
            nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
        ));
    }
    DefContent::Inline(found.unwrap_or_default())
}

/// The positions, among `registrations` (each a feature and an identifier,
/// in registration order), of the registration that defines each identifier:
/// of the registrations of one identifier, the one of the feature placed last
/// in the feature order, and of one feature's, its first.
fn chosen_registrations<'r>(
    registry: &FlmRegistry,
    registrations: impl Iterator<Item = (&'r FeatureName, &'r str)>,
) -> BTreeSet<usize> {
    let mut chosen: BTreeMap<&str, (Option<usize>, usize)> = BTreeMap::new();
    for (index, (owner, id)) in registrations.enumerate() {
        let position = registry.feature_position(owner);
        match chosen.get(id) {
            Some((best, _)) if position <= *best => {}
            _ => {
                chosen.insert(id, (position, index));
            }
        }
    }
    chosen.into_values().map(|(_, index)| index).collect()
}

/// The stage `flm.html.resolve-definitions` ([`RESOLVE_STAGE`]): it turns the
/// identifiers that the document's trees and pieces required into the
/// definitions table [`DEFINITIONS_TABLE`] (the [module documentation](self)
/// describes the steps).
///
/// It runs once per document, after the HTML of the trees and of the pieces
/// has been written and before the regions are made. The plan derives that
/// order from the tables: this stage reads the tables the two emit stages write
/// (`flm.output.html` and `flm.output.html.pieces`), and the regions stage
/// reads the table this stage writes. It also reads the render options
/// ([`HtmlRenderOptions`]).
///
/// When several features register one identifier, the registration of the
/// feature placed last in the feature order is the definition (of one
/// feature's registrations, its first), and it stands at its own place in
/// registration order; two features that require the same stylesheet thus
/// require it once. Style definitions are looked up before script
/// definitions: an identifier registered as both resolves to the style
/// definition.
///
/// A part supplied by a table ([`DefContent::Table`]) is replaced by the text
/// the table holds, as an [`Inline`](DefContent::Inline) part; the stage reads
/// every table such a part of a registered definition names, as an optional
/// read computed over the registry when the plan is built. Conditions:
/// [`UnknownRequirement`], [`UnknownDefinition`], [`DefinitionPartMissing`], and
/// [`StageFailed`](super::stages::StageFailed) when a table or the options
/// cannot be read or the table cannot be written.
#[derive(Debug, Default, Clone, Copy)]
pub struct ResolveDefinitionsStage;

impl Stage for ResolveDefinitionsStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(RESOLVE_STAGE, Effect::Compute, Scope::Document)
            .writes(DEFINITIONS_TABLE)
            .docs(
                "resolves the stylesheets and scripts that the document's trees and pieces \
                 required, the ones the host requires, and the host's deliveries, into \
                 `flm.html.definitions`",
            )
    }

    /// The two tables of the emit stages and the render options, and, as
    /// optional reads, the tables that supply a part of a registered
    /// definition ([`DefContent::Table`]): computed over the registry when the
    /// plan is built.
    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        let mut needs =
            Needs::new().reads(OUTPUT_TABLE).reads(PIECES_TABLE).option::<HtmlRenderOptions>();
        for table in part_tables(cx.registry()) {
            needs = needs.reads_opt(TableHandle::<DefinitionText>::new(table));
        }
        needs
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let fragments = match cx.get(OUTPUT_TABLE) {
            Ok(Some(fragments)) => fragments,
            Ok(None) => return,
            Err(error) => return stage_failed(out, RESOLVE_STAGE, error),
        };
        let pieces = match cx.get(PIECES_TABLE) {
            Ok(pieces) => pieces,
            Err(error) => return stage_failed(out, RESOLVE_STAGE, error),
        };
        let options = match cx.option::<HtmlRenderOptions>() {
            Ok(options) => options,
            Err(error) => return stage_failed(out, RESOLVE_STAGE, error),
        };
        // What the document's content and the pieces the page places
        // required, plus the page's own stylesheet — the page is this
        // backend's output, and nothing in a document can ask for it — and
        // what the host requires for the page.
        let mut required: BTreeSet<Name> = options.required_definitions.clone();
        for fragment in fragments.trees.values() {
            required.extend(fragment.requires.iter().cloned());
        }
        for piece in pieces.iter().flat_map(|pieces| &pieces.pieces) {
            required.extend(piece.fragment.requires.iter().cloned());
        }
        required.insert(Name::from(STYLE_PAGE));

        let registry = cx.registry();
        // Of the registrations of one identifier, the one of the feature
        // placed last is the definition (of one feature's, its first), at its
        // own place: two features asking for the same stylesheet ask for it
        // once, and the two rosters are walked one after the other because
        // the registry keeps them apart and the page writes them in two
        // places. The choice therefore runs within a kind: where one
        // identifier names a style definition and a script definition, the
        // style one is the definition, whichever was registered last.
        // See decision rationale at [§flm-dd-dr:rend-html-definition-delivery]
        // and [§flm-dd-dr:feat-order].
        let mut declared: BTreeMap<&str, (DefinitionKind, &FeatureName, &Vec<DefContent>)> =
            BTreeMap::new();
        let mut order: Vec<&str> = Vec::new();
        let styles = chosen_registrations(registry, registry.styles().iter().map(|(owner, style)| (owner, style.id.as_str())));
        for (index, (owner, style)) in registry.styles().iter().enumerate() {
            if styles.contains(&index) {
                declared.insert(style.id.as_str(), (DefinitionKind::Style, owner, &style.content));
                order.push(style.id.as_str());
            }
        }
        let scripts = chosen_registrations(registry, registry.scripts().iter().map(|(owner, script)| (owner, script.id.as_str())));
        for (index, (owner, script)) in registry.scripts().iter().enumerate() {
            if scripts.contains(&index) && !declared.contains_key(script.id.as_str()) {
                declared
                    .insert(script.id.as_str(), (DefinitionKind::Script, owner, &script.content));
                order.push(script.id.as_str());
            }
        }

        for id in options.definitions.keys() {
            if !declared.contains_key(id.as_str()) {
                out.diags.push(Diagnostic::warning(
                    UnknownDefinition { definition: id.to_string() },
                    nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
                ));
            }
        }
        for id in &required {
            if !declared.contains_key(id.as_str()) {
                out.diags.push(Diagnostic::error(
                    UnknownRequirement { definition: id.to_string() },
                    nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
                ));
            }
        }

        let mut resolved = ResolvedDefinitions::default();
        for id in order {
            if !required.contains(&Name::from(id)) {
                continue;
            }
            let (kind, owner, content) = &declared[id];
            let name = Name::from(id);
            // An override decides the delivery and never the requirement: the
            // row stands here whatever the host answered.
            let delivery = match options.definitions.get(&name) {
                Some(over) => over.clone(),
                None => DefinitionOverride::Parts(
                    content.iter().map(|part| resolve_part(cx, out, *kind, id, part)).collect(),
                ),
            };
            resolved.definitions.push(ResolvedDefinition {
                id: name,
                kind: *kind,
                owner: (*owner).clone(),
                delivery,
            });
        }
        // The stylesheets first, then the scripts, each group in registration
        // order: the two places the page writes them in.
        resolved.definitions.sort_by_key(|definition| definition.kind);

        match out.table(DEFINITIONS_TABLE) {
            Ok(table) => *table = resolved,
            Err(error) => stage_failed(out, RESOLVE_STAGE, error),
        }
    }
}
