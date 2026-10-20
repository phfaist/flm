//! The stages of the `latex-rendered` backend after lowering, the tables they
//! write, the resolution of requirements, and the conditions they report.
//!
//! 1. The **emit stage** `flm.latex-rendered.emit` ([`LatexRenderedEmitStage`],
//!    once per tree) writes each lowered tree's LaTeX ([`emit_tree`]) into
//!    `flm.output.latex-rendered` ([`RenderedFragments`]): the LaTeX as
//!    segments (bytes, or another tree placed there), the requirement
//!    identifiers (the tree's lowering requirements and the emitter's) and the
//!    definitions the tree staged.
//! 2. The **pieces** the render places are lowered by the shared pieces stage
//!    (every piece placed by default) and written by
//!    `flm.latex-rendered.emit-pieces` ([`LatexRenderedEmitPiecesStage`]) into
//!    `flm.output.latex-rendered.pieces` ([`RenderedPieces`]), each with its
//!    words.
//! 3. The **regions stage** `flm.latex-rendered.regions`
//!    ([`LatexRenderedRegionsStage`]) writes each tree placed in another where
//!    it is placed, resolves every requirement ([`resolve`]), assembles the
//!    style file ([`super::style`]), names it through the program's naming of
//!    files, and writes the regions `content`, one per piece, and
//!    `style-package` into `flm.output.latex-rendered.regions` ([`REGIONS`]),
//!    the style file into `flm.output.latex-rendered.style` ([`STYLE_FILE`]).
//! 4. The **final stage** `flm.latex-rendered.final`
//!    ([`LatexRenderedFinalStage`]) writes the fixed skeleton, a complete
//!    `article` document loading the style file, as the entry artifact
//!    `latex-rendered:document` of `flm.output.latex-rendered.document`
//!    ([`DOCUMENT`]), followed by the style file and the files of
//!    `flm.output.latex-rendered.artifacts` ([`ARTIFACTS`]).
//!
//! # The resolution of requirements
//!
//! Every identifier a tree, a piece or the emitter requires is resolved
//! ([`resolve`]): `flm.latex.pkg.*` through the LaTeX family's package slot,
//! `flm.latex.def.*` through its preamble slot, closed over the pieces each
//! piece requires in turn. Any other identifier (a feature's style
//! identifier, such as `flm.theorems.style`) brings in every piece whose
//! [`delivers`](crate::stdcore::backends::latex::LatexPreambleDef::delivers)
//! lists it; where no piece delivers it and the registry holds it as a style
//! or script definition, it is another backend's and writes nothing; an
//! identifier known nowhere is reported as [`UnknownRequirement`] and left
//! out. The core chunk is in every style file.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use techy::error::{Diagnostic, DiagnosticInfo};
use techy::serialize::SerialValue;

use crate::core::artifact::{
    Artifact, ArtifactId, ArtifactNamer, ArtifactRef, ArtifactSet, NamingScope, RelPath,
};
use crate::core::data::store::StageOut;
use crate::core::data::table::{PieceClass, ProjectCx, Table, TableName};
use crate::core::piece::PiecePlace;
use crate::core::plan::PlanCx;
use crate::core::region::{CONTENT, Region, Regions, STYLE_PACKAGE};
use crate::core::registry::FlmRegistry;
use crate::core::services::Services;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::{FlmDiagnostics, FlmOrigin, nowhere};
use crate::model::addr::TreeId;
use crate::model::names::{BackendName, Name};
use crate::stdcore::backends::latex::style::content_hash;
use crate::stdcore::backends::latex::{
    LatexPackageDef, LatexPreambleDef, PackageRequest, packages, preamble,
};
use crate::stdcore::backends::lowering::{LoweringStage, NAMER_SLOT, lowered_table};
use crate::stdcore::backends::pieces::lowered_pieces_table;

use super::contract::CORE_CHUNK;
use super::emit::{NOWHERE_LABEL, StagedDefinition, emit_tree};
use super::feature::{BACKEND, LatexRenderedRenderOptions};
use super::style::{StyleParts, assemble, style_body};

/// The name of the emit stage, `flm.latex-rendered.emit`.
pub const EMIT_STAGE: &str = "flm.latex-rendered.emit";
/// The name of the stage writing the pieces, `flm.latex-rendered.emit-pieces`.
pub const EMIT_PIECES_STAGE: &str = "flm.latex-rendered.emit-pieces";
/// The name of the regions stage, `flm.latex-rendered.regions`.
pub const REGIONS_STAGE: &str = "flm.latex-rendered.regions";
/// The name of the final stage, `flm.latex-rendered.final`.
pub const FINAL_STAGE: &str = "flm.latex-rendered.final";
/// The owner of the backend's artifacts, `latex-rendered`: the document is
/// `latex-rendered:document` and the style file `latex-rendered:<stem>.<hash>`.
pub const ARTIFACT_OWNER: &str = "latex-rendered";
/// The name of the entry artifact, `document`.
pub const DOCUMENT_ARTIFACT: &str = "document";
/// The media type of the document and of the style file, `application/x-latex`.
pub const MEDIA_TYPE: &str = "application/x-latex";
/// The extension of the document, `tex`.
pub const EXTENSION: &str = "tex";
/// The extension of the style file, `sty`.
pub const STYLE_EXTENSION: &str = "sty";

/// The per-tree output table, `flm.output.latex-rendered`.
pub const OUTPUT_TABLE: TableName<RenderedFragments> = TableName::new("flm.output.latex-rendered");
/// The pieces' output table, `flm.output.latex-rendered.pieces`.
pub const PIECES_TABLE: TableName<RenderedPieces> =
    TableName::new("flm.output.latex-rendered.pieces");
/// The output regions, `flm.output.latex-rendered.regions`: `content`, one
/// region per piece with its words, and `style-package`, the style file's
/// LaTeX package name.
pub const REGIONS: TableName<Regions> = TableName::new("flm.output.latex-rendered.regions");
/// The style file, `flm.output.latex-rendered.style`: an artifact set holding
/// the one style file.
pub const STYLE_FILE: TableName<ArtifactSet> = TableName::new("flm.output.latex-rendered.style");
/// The files beside the document that another feature's stage writes (copies
/// of images), `flm.output.latex-rendered.artifacts`.
pub const ARTIFACTS: TableName<ArtifactSet> =
    TableName::new("flm.output.latex-rendered.artifacts");
/// The `latex-rendered` output's table, `flm.output.latex-rendered.document`:
/// the document as the entry artifact, the style file and the files beside it.
pub const DOCUMENT: TableName<ArtifactSet> =
    TableName::new("flm.output.latex-rendered.document");

// --- the tables -----------------------------------------------------------------------------

/// One segment of a tree's LaTeX: bytes, or another tree placed there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LatexSegment {
    /// LaTeX this tree's emission wrote.
    Latex(String),
    /// Another document tree, placed here by a `flm.pres.subtree` node.
    Tree(TreeId),
}

/// One tree's LaTeX, its requirements and the definitions it staged.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderedFragment {
    /// The tree's LaTeX, in order.
    pub segments: Vec<LatexSegment>,
    /// The identifiers the LaTeX requires: what the tree's lowering rules
    /// required and what the emitter required.
    pub requires: BTreeSet<Name>,
    /// The definitions the tree's rules staged, each once by name.
    pub definitions: Vec<StagedDefinition>,
}

impl RenderedFragment {
    /// The tree's own LaTeX, with nothing in place of the trees it places.
    pub fn latex(&self) -> String {
        self.segments
            .iter()
            .filter_map(|segment| match segment {
                LatexSegment::Latex(latex) => Some(latex.as_str()),
                LatexSegment::Tree(_) => None,
            })
            .collect()
    }
}

/// Projects a list of names.
fn names<'n>(names: impl Iterator<Item = &'n Name>) -> SerialValue {
    SerialValue::List(names.map(|name| SerialValue::Str(name.to_string())).collect())
}

/// The per-tree output table `flm.output.latex-rendered` ([`OUTPUT_TABLE`]):
/// one [`RenderedFragment`] per document tree.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderedFragments {
    /// Each tree's LaTeX, by tree.
    pub trees: BTreeMap<TreeId, RenderedFragment>,
}

impl Table for RenderedFragments {
    fn absorb(&mut self, piece: RenderedFragments) {
        self.trees.extend(piece.trees);
    }

    /// One entry per tree: `{tree, latex, requires: [id], definitions: [name]}`.
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.trees
                .iter()
                .map(|(id, fragment)| {
                    SerialValue::Map(vec![
                        (String::from("tree"), cx.tree_ref(*id)),
                        (String::from("latex"), SerialValue::Str(fragment.latex())),
                        (String::from("requires"), names(fragment.requires.iter())),
                        (
                            String::from("definitions"),
                            SerialValue::List(
                                fragment
                                    .definitions
                                    .iter()
                                    .map(|definition| SerialValue::Str(definition.name.clone()))
                                    .collect(),
                            ),
                        ),
                    ])
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        if self.trees.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

/// One piece's LaTeX: its name, its place, its LaTeX with its requirements,
/// and its words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPiece {
    /// The piece's name.
    pub name: Name,
    /// Where the document places it.
    pub place: PiecePlace,
    /// Its LaTeX; a piece places no tree, so its segments are bytes only.
    pub fragment: RenderedFragment,
    /// Its words: the plain text of its lowered tree, white space collapsed
    /// ([`piece_words`](crate::core::region::piece_words)).
    pub words: String,
}

/// The pieces' output table `flm.output.latex-rendered.pieces`
/// ([`PIECES_TABLE`]): one [`RenderedPiece`] per piece placed, in declaration
/// order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RenderedPieces {
    /// The pieces, in declaration order.
    pub pieces: Vec<RenderedPiece>,
}

impl Table for RenderedPieces {
    fn absorb(&mut self, piece: RenderedPieces) {
        self.pieces.extend(piece.pieces);
    }

    /// One entry per piece: `{name, place, latex, words, requires: [id]}`.
    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.pieces
                .iter()
                .map(|piece| {
                    SerialValue::Map(vec![
                        (String::from("name"), SerialValue::Str(piece.name.to_string())),
                        (String::from("place"), SerialValue::Str(piece.place.as_str().into())),
                        (String::from("latex"), SerialValue::Str(piece.fragment.latex())),
                        (String::from("words"), SerialValue::Str(piece.words.clone())),
                        (String::from("requires"), names(piece.fragment.requires.iter())),
                    ])
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        if self.pieces.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

// --- conditions -----------------------------------------------------------------------------

/// Condition `flm.latex-rendered.stage-failed` (Error): a stage of this
/// backend could not read or write a table or an option it declared, or the
/// emitter met an element it does not write. Both report a mistake in the
/// library or an extension. Recovery: the stage's output is absent, or the
/// node writes nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.latex-rendered.stage-failed", message = "the stage `{stage}` failed: {detail}")]
pub struct StageFailed {
    /// The name of the stage.
    pub stage: String,
    /// What failed, as text.
    pub detail: String,
}

/// Condition `flm.latex-rendered.unknown-requirement` (Error): a rule, a piece
/// or the emitter required an identifier that names no LaTeX package
/// request, no definition piece, and no style or script definition.
/// Recovery: the identifier is left out of the style file.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-rendered.unknown-requirement",
    message = "the requirement `{id}` names no LaTeX package request, no definition piece, and \
               no style or script definition"
)]
pub struct UnknownRequirement {
    /// The requirement identifier.
    pub id: String,
}

/// Condition `flm.latex-rendered.conflicting-package-options` (Error): two
/// requests for one LaTeX package give one option two values, which the one
/// `\RequirePackage` line cannot hold. Recovery: the package is not
/// requested.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-rendered.conflicting-package-options",
    message = "two requests for the LaTeX package `{package}` give the option `{key}` the values \
               `{first}` and `{second}`"
)]
pub struct ConflictingPackageOptions {
    /// The package's name.
    pub package: String,
    /// The option key the requests disagree on.
    pub key: String,
    /// The option as the first request wrote it.
    pub first: String,
    /// The option as the second request wrote it.
    pub second: String,
}

/// Condition `flm.latex-rendered.style-path` (Error): the program's naming of
/// files placed the style file at a path that is not a bare file name ending
/// in `.sty`, which the document cannot load by its package name. Recovery:
/// the style file is named `<stem>.<hash>.sty`.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-rendered.style-path",
    message = "the naming of files places the style file at `{path}`, which is not a bare file \
               name ending in `.sty`; the document loads the style file by its package name"
)]
pub struct StylePathRefused {
    /// The path the naming of files gave.
    pub path: String,
}

/// Condition `flm.latex-rendered.missing-subtree` (Error): a
/// `flm.pres.subtree` node places a tree that has no LaTeX for this backend.
/// Recovery: nothing is written in the node's place.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-rendered.missing-subtree",
    message = "the document tree `{tree}` is placed inside another tree, but it has no LaTeX to \
               write there: no tree has that identifier, or that tree could not be converted to \
               LaTeX"
)]
pub struct MissingSubtree {
    /// The placed tree.
    pub tree: String,
}

/// Condition `flm.latex-rendered.subtree-cycle` (Error): a tree would be
/// placed inside itself, directly or through other trees. Recovery: nothing
/// is written in the node's place.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-rendered.subtree-cycle",
    message = "the document tree `{tree}` must not be placed inside itself, but it is, \
               directly or through other trees"
)]
pub struct SubtreeCycle {
    /// The placed tree.
    pub tree: String,
}

/// Condition `flm.latex-rendered.piece-placement` (Error): a
/// `flm.pres.subtree` node is inside a piece, which is placed beside the
/// document's content and never places a tree. Recovery: nothing is written
/// in the node's place.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.latex-rendered.piece-placement",
    message = "the document tree `{tree}` cannot be placed inside `{piece}`, which the document \
               places beside its content"
)]
pub struct PiecePlacement {
    /// The name of the piece.
    pub piece: String,
    /// The tree the node places.
    pub tree: String,
}

/// The position of a condition reported at no position in any document.
fn here() -> crate::lang::FlmSpan {
    nowhere(FlmOrigin::synthetic(NOWHERE_LABEL))
}

/// Reports [`StageFailed`] at no position in any document.
fn stage_failed(out: &mut StageOut<'_>, stage: &str, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: String::from(stage), detail: detail.to_string() },
        here(),
    ));
}

// --- the resolution of requirements -----------------------------------------------------------

/// What the requirements of a document resolve to: the definition pieces and
/// the merged package requests the style file writes, and whether `hyperref`
/// is loaded.
#[derive(Debug, Clone)]
pub struct Resolved<'s> {
    /// The definition pieces, the core chunk first, then the others in the
    /// order the registry holds them.
    pub pieces: Vec<&'s LatexPreambleDef>,
    /// The merged package requests, in the order first met: the pieces'
    /// packages in piece order, then the packages the trees and pieces
    /// required directly, then the author's own requests.
    pub packages: Vec<PackageRequest>,
    /// Whether the style file loads `hyperref` at the end of the preamble:
    /// unless the render option `skip_packages` lists it.
    pub hyperref: bool,
}

/// The package `hyperref`, which the style file loads through the kernel hook
/// rather than in its package block.
const HYPERREF: &str = "hyperref";

/// Resolves the identifiers `required` (see the [module
/// documentation](self#the-resolution-of-requirements)), reporting each
/// identifier known nowhere ([`UnknownRequirement`]) and each package whose
/// requests conflict ([`ConflictingPackageOptions`]) into `diags`.
///
/// The core chunk is always among the pieces. A package listed in the render
/// option `skip_packages`, and `hyperref`, which the style file loads through
/// the kernel hook, are left out of the library's requests; the author's own
/// requests (`extra_packages`) are merged after them as given.
pub fn resolve<'s>(
    services: &'s Services,
    registry: &FlmRegistry,
    required: &BTreeSet<Name>,
    options: &'s LatexRenderedRenderOptions,
    diags: &mut FlmDiagnostics,
) -> Resolved<'s> {
    // Of the pieces with one identifier, the one of the feature placed last
    // defines it (see decision rationale at [§flm-dd-dr:feat-order]).
    let definitions = preamble::definitions(services);
    // The pieces wanted, closed over what each requires in turn.
    let mut wanted: BTreeSet<Name> = BTreeSet::from([Name::from(CORE_CHUNK)]);
    let mut direct_packages: BTreeSet<Name> = BTreeSet::new();
    for id in required {
        let text = id.as_str();
        if text.starts_with(preamble::ID_PREFIX) {
            wanted.insert(id.clone());
        } else if text.starts_with(packages::ID_PREFIX) {
            direct_packages.insert(id.clone());
        } else {
            // A style identifier: every piece that delivers it stands for it.
            let delivering: Vec<Name> = definitions
                .iter()
                .filter(|piece| piece.delivers.contains(id))
                .map(|piece| piece.id.clone())
                .collect();
            if !delivering.is_empty() {
                wanted.extend(delivering);
            } else if registry.styles().iter().any(|(_, style)| style.id == *id)
                || registry.scripts().iter().any(|(_, script)| script.id == *id)
            {
                // Another backend's stylesheet or script: nothing to write here.
            } else {
                diags.push(Diagnostic::error(UnknownRequirement { id: id.to_string() }, here()));
            }
        }
    }
    let mut added = true;
    while added {
        added = false;
        for piece in &definitions {
            if !wanted.contains(&piece.id) {
                continue;
            }
            for id in preamble::split_requirements(&piece.requires).0 {
                added |= wanted.insert(id);
            }
        }
    }
    let mut pieces: Vec<&LatexPreambleDef> = Vec::new();
    for piece in definitions {
        let first = piece.id.as_str() == CORE_CHUNK;
        if wanted.contains(&piece.id) {
            if first {
                pieces.insert(0, piece);
            } else {
                pieces.push(piece);
            }
        }
    }
    for id in &wanted {
        if !pieces.iter().any(|piece| piece.id == *id) {
            diags.push(Diagnostic::error(UnknownRequirement { id: id.to_string() }, here()));
        }
    }

    // The packages, in the order first met.
    let mut package_ids: Vec<Name> = Vec::new();
    for piece in &pieces {
        for id in &piece.requires {
            if !id.as_str().starts_with(preamble::ID_PREFIX) && !package_ids.contains(id) {
                package_ids.push(id.clone());
            }
        }
    }
    for id in direct_packages {
        if !package_ids.contains(&id) {
            package_ids.push(id);
        }
    }
    let skipped = |package: &str| package == HYPERREF || options.skip_packages.contains(package);
    let mut defs: Vec<&LatexPackageDef> = Vec::new();
    for id in &package_ids {
        let named: Vec<&LatexPackageDef> = packages::packages_named(services, id.as_str()).collect();
        if named.is_empty() {
            diags.push(Diagnostic::error(UnknownRequirement { id: id.to_string() }, here()));
        }
        defs.extend(named.into_iter().filter(|def| !skipped(def.package.as_ref())));
    }
    defs.extend(options.extra_packages.iter());
    let (packages, conflicts) = packages::merge_requests(defs);
    for conflict in conflicts {
        diags.push(Diagnostic::error(
            ConflictingPackageOptions {
                package: conflict.package,
                key: conflict.key,
                first: conflict.first,
                second: conflict.second,
            },
            here(),
        ));
    }
    Resolved { pieces, packages, hyperref: !options.skip_packages.contains(HYPERREF) }
}

// --- the emit stages ----------------------------------------------------------------------

/// The emit stage, `flm.latex-rendered.emit` ([`EMIT_STAGE`]): one lowered
/// tree's LaTeX into [`RenderedFragments`], once per tree.
#[derive(Debug, Default, Clone, Copy)]
pub struct LatexRenderedEmitStage;

impl Stage for LatexRenderedEmitStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(EMIT_STAGE, Effect::Compute, Scope::PerTree).writes(OUTPUT_TABLE).docs(
            "writes each lowered tree's LaTeX, its requirements and the definitions it staged \
             into `flm.output.latex-rendered`",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
            .reads(lowered_table(&BackendName::from(BACKEND)))
            .option::<LatexRenderedRenderOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let Some(id) = cx.tree_id() else { return };
        let trees = match cx.get(lowered_table(&BackendName::from(BACKEND))) {
            Ok(Some(trees)) => trees,
            Ok(None) => return,
            Err(error) => return stage_failed(out, EMIT_STAGE, error),
        };
        let Some(lowered) = trees.get(id) else { return };
        let options = match cx.option::<LatexRenderedRenderOptions>() {
            Ok(options) => options,
            Err(error) => return stage_failed(out, EMIT_STAGE, error),
        };
        let emitted = emit_tree(&lowered.tree, options);
        for diagnostic in emitted.diags.iter() {
            out.diags.push(diagnostic.clone());
        }
        let mut fragment = RenderedFragment {
            segments: emitted.segments,
            requires: emitted.requires,
            definitions: emitted.definitions,
        };
        fragment.requires.extend(lowered.requires.iter().cloned());
        match out.table(OUTPUT_TABLE) {
            Ok(table) => {
                table.trees.insert(id, fragment);
            }
            Err(error) => stage_failed(out, EMIT_STAGE, error),
        }
    }
}

/// The stage `flm.latex-rendered.emit-pieces` ([`EMIT_PIECES_STAGE`]): each
/// lowered piece's LaTeX into [`RenderedPieces`], once per document. A
/// `flm.pres.subtree` node inside a piece writes nothing and is reported as
/// [`PiecePlacement`].
#[derive(Debug, Default, Clone, Copy)]
pub struct LatexRenderedEmitPiecesStage;

impl Stage for LatexRenderedEmitPiecesStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(EMIT_PIECES_STAGE, Effect::Compute, Scope::Document)
            .writes(PIECES_TABLE)
            .docs("writes each lowered piece's LaTeX into `flm.output.latex-rendered.pieces`")
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
            .reads(lowered_pieces_table(&BackendName::from(BACKEND)))
            .option::<LatexRenderedRenderOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let lowered = match cx.get(lowered_pieces_table(&BackendName::from(BACKEND))) {
            Ok(Some(lowered)) => lowered,
            Ok(None) => return,
            Err(error) => return stage_failed(out, EMIT_PIECES_STAGE, error),
        };
        let options = match cx.option::<LatexRenderedRenderOptions>() {
            Ok(options) => options,
            Err(error) => return stage_failed(out, EMIT_PIECES_STAGE, error),
        };
        let mut pieces = RenderedPieces::default();
        for piece in &lowered.pieces {
            let emitted = emit_tree(&piece.tree.tree, options);
            for diagnostic in emitted.diags.iter() {
                out.diags.push(diagnostic.clone());
            }
            let mut segments = Vec::with_capacity(emitted.segments.len());
            let mut placed_at = emitted.placed_at.into_iter();
            for segment in emitted.segments {
                match segment {
                    LatexSegment::Latex(latex) => segments.push(LatexSegment::Latex(latex)),
                    LatexSegment::Tree(tree) => out.diags.push(Diagnostic::error(
                        PiecePlacement { piece: piece.name.to_string(), tree: tree.to_string() },
                        placed_at.next().unwrap_or_else(here),
                    )),
                }
            }
            let mut requires = emitted.requires;
            requires.extend(piece.tree.requires.iter().cloned());
            pieces.pieces.push(RenderedPiece {
                name: piece.name.clone(),
                place: piece.place,
                fragment: RenderedFragment { segments, requires, definitions: emitted.definitions },
                words: crate::core::region::piece_words(&piece.tree.tree),
            });
        }
        match out.table(PIECES_TABLE) {
            Ok(table) => *table = pieces,
            Err(error) => stage_failed(out, EMIT_PIECES_STAGE, error),
        }
    }
}

// --- the regions stage --------------------------------------------------------------------

/// The trees placed in a tree, written in place: what [`write_tree`] gathers.
#[derive(Default)]
struct Written {
    /// The trees written so far, at the top level or where placed.
    done: BTreeSet<TreeId>,
    missing: Vec<TreeId>,
    cycles: Vec<TreeId>,
}

/// Writes the LaTeX of the tree `id` into `out`, each tree it places written
/// where it is placed. `entered` holds the trees being written.
fn write_tree(
    fragments: &RenderedFragments,
    id: TreeId,
    out: &mut String,
    entered: &mut Vec<TreeId>,
    written: &mut Written,
) {
    let Some(fragment) = fragments.trees.get(&id) else {
        written.missing.push(id);
        return;
    };
    if entered.contains(&id) {
        written.cycles.push(id);
        return;
    }
    entered.push(id);
    written.done.insert(id);
    for segment in &fragment.segments {
        match segment {
            LatexSegment::Latex(latex) => out.push_str(latex),
            LatexSegment::Tree(tree) => write_tree(fragments, *tree, out, entered, written),
        }
    }
    entered.pop();
}

/// Returns the path of the style file whose identity is `id` and whose name
/// without a naming of files is `default` (`<stem>.<hash>`): the path the
/// registry's naming of files gives, where it is a bare file name ending in
/// `.sty`, else `<default>.sty`, reporting [`StylePathRefused`].
fn style_path(cx: &StageCx<'_>, id: &ArtifactId, default: &str, out: &mut StageOut<'_>) -> String {
    let fallback = format!("{default}.{STYLE_EXTENSION}");
    let Some(namer) = cx.services_all().service_named::<dyn ArtifactNamer>(NAMER_SLOT) else {
        return fallback;
    };
    let artifact = ArtifactRef {
        id,
        entry: false,
        media_type: MEDIA_TYPE,
        extension: Some(STYLE_EXTENSION),
        output: None,
        folder: false,
    };
    let path = namer.url_of(&artifact, &NamingScope::of(cx.registry(), cx.plan().demands()));
    let path = path.as_str();
    let loadable = path
        .strip_suffix(".sty")
        .is_some_and(|name| !name.is_empty() && !name.contains(['/', '\\']));
    if loadable {
        return path.to_string();
    }
    out.diags.push(Diagnostic::error(StylePathRefused { path: path.to_string() }, here()));
    fallback
}

/// The regions stage, `flm.latex-rendered.regions` ([`REGIONS_STAGE`]; see
/// the [module documentation](self)).
#[derive(Debug, Default, Clone, Copy)]
pub struct LatexRenderedRegionsStage;

impl Stage for LatexRenderedRegionsStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(REGIONS_STAGE, Effect::Compute, Scope::Document)
            .writes(REGIONS)
            .writes(STYLE_FILE)
            .docs(
                "resolves the requirements, assembles the style file into \
                 `flm.output.latex-rendered.style`, and writes the content, the pieces and the \
                 style file's package name as the regions `flm.output.latex-rendered.regions`",
            )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
            .reads(OUTPUT_TABLE)
            .reads(PIECES_TABLE)
            .option::<LatexRenderedRenderOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let fragments = match cx.get(OUTPUT_TABLE) {
            Ok(Some(fragments)) => fragments,
            Ok(None) => return,
            Err(error) => return stage_failed(out, REGIONS_STAGE, error),
        };
        let no_pieces = RenderedPieces::default();
        let pieces = match cx.get(PIECES_TABLE) {
            Ok(Some(pieces)) => pieces,
            Ok(None) => &no_pieces,
            Err(error) => return stage_failed(out, REGIONS_STAGE, error),
        };
        let options = match cx.option::<LatexRenderedRenderOptions>() {
            Ok(options) => options,
            Err(error) => return stage_failed(out, REGIONS_STAGE, error),
        };

        // The content: the trees in document order, a tree placed in another
        // written where it is placed and not again in its own right.
        let placed: BTreeSet<TreeId> = fragments
            .trees
            .values()
            .flat_map(|fragment| fragment.segments.iter())
            .filter_map(|segment| match segment {
                LatexSegment::Tree(tree) => Some(*tree),
                LatexSegment::Latex(_) => None,
            })
            .collect();
        let mut content = String::new();
        let mut written = Written::default();
        let order: Vec<TreeId> = cx.trees().map(|(id, _)| id).collect();
        for id in order.iter().copied() {
            if !placed.contains(&id) && fragments.trees.contains_key(&id) {
                write_tree(fragments, id, &mut content, &mut Vec::new(), &mut written);
            }
        }
        // A tree every other tree's placement left unwritten (trees placing
        // each other in a cycle) is written at the top level after the
        // others, so that its content is kept and the cycle is reported.
        for id in order.iter().copied() {
            if !written.done.contains(&id) && fragments.trees.contains_key(&id) {
                write_tree(fragments, id, &mut content, &mut Vec::new(), &mut written);
            }
        }
        for tree in written.missing {
            out.diags.push(Diagnostic::error(MissingSubtree { tree: tree.to_string() }, here()));
        }
        for tree in written.cycles {
            out.diags.push(Diagnostic::error(SubtreeCycle { tree: tree.to_string() }, here()));
        }

        // Every requirement and every staged definition, trees and pieces.
        let mut required: BTreeSet<Name> = BTreeSet::new();
        let mut definitions: Vec<StagedDefinition> = Vec::new();
        let fragments_and_pieces = fragments
            .trees
            .values()
            .chain(pieces.pieces.iter().map(|piece| &piece.fragment));
        for fragment in fragments_and_pieces {
            required.extend(fragment.requires.iter().cloned());
            for definition in &fragment.definitions {
                if !definitions.iter().any(|seen| seen.name == definition.name) {
                    definitions.push(definition.clone());
                }
            }
        }
        let mut diags = FlmDiagnostics::new();
        let resolved = resolve(cx.services_all(), cx.registry(), &required, options, &mut diags);
        for diagnostic in diags.iter() {
            out.diags.push(diagnostic.clone());
        }

        // The style file, named through the naming of files.
        let body = style_body(&StyleParts {
            packages: &resolved.packages,
            hyperref: resolved.hyperref,
            pieces: &resolved.pieces,
            definitions: &definitions,
            extra: &options.extra_preamble,
        });
        let default = format!("{}.{}", options.style_stem, content_hash(&body));
        let style_id = ArtifactId::new(ARTIFACT_OWNER, default.clone());
        let path = style_path(cx, &style_id, &default, out);
        let name = path.strip_suffix(".sty").unwrap_or(&path).to_string();
        let mut style = Artifact::bytes(style_id.clone(), MEDIA_TYPE, assemble(&name, &body).into_bytes())
            .with_extension(STYLE_EXTENSION);
        style.fixed_path = Some(RelPath::new(path));
        let mut beside = ArtifactSet::new();
        if let Err(violation) = beside.insert(style) {
            return stage_failed(out, REGIONS_STAGE, violation);
        }

        let mut regions = Regions::new(ArtifactId::new(ARTIFACT_OWNER, DOCUMENT_ARTIFACT), MEDIA_TYPE);
        regions.regions.push(Region::new(CONTENT, vec![content]));
        for piece in &pieces.pieces {
            regions.regions.push(Region::piece(
                piece.name.clone(),
                piece.place,
                vec![piece.fragment.latex()],
                piece.words.clone(),
            ));
        }
        regions.regions.push(Region::new(STYLE_PACKAGE, vec![name]));
        regions.refs.push(style_id);
        let write = (|| -> Result<(), crate::core::data::store::AccessError> {
            *out.table(REGIONS)? = regions;
            *out.table(STYLE_FILE)? = beside;
            Ok(())
        })();
        if let Err(error) = write {
            stage_failed(out, REGIONS_STAGE, error);
        }
    }
}

// --- the final stage ------------------------------------------------------------------------

/// The first line of the document the fixed skeleton writes, before the
/// style file's name: `% flm: latex-rendered; style file `.
pub const SKELETON_HEADER: &str = "% flm: latex-rendered; style file ";

/// The backend's final stage, `flm.latex-rendered.final` ([`FINAL_STAGE`]):
/// the fixed skeleton, a complete document of the class `article` loading the
/// style file, with the pieces placed before the content, the content, and
/// the pieces placed after it, as the entry artifact of [`DOCUMENT`],
/// followed by the style file and the files of [`ARTIFACTS`].
#[derive(Debug, Default, Clone, Copy)]
pub struct LatexRenderedFinalStage;

impl Stage for LatexRenderedFinalStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(FINAL_STAGE, Effect::Compute, Scope::Document).writes(DOCUMENT).docs(
            "writes the document in the backend's fixed skeleton, with its style file, as the \
             artifact set `flm.output.latex-rendered.document`",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(REGIONS).reads(STYLE_FILE).reads_opt(ARTIFACTS)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let regions = match cx.get(REGIONS) {
            Ok(Some(regions)) => regions,
            Ok(None) => return,
            Err(error) => return stage_failed(out, FINAL_STAGE, error),
        };
        let mut beside = Vec::new();
        for table in [STYLE_FILE, ARTIFACTS] {
            match cx.get(table) {
                Ok(set) => beside.extend(set),
                Err(error) => return stage_failed(out, FINAL_STAGE, error),
            }
        }
        let document = skeleton(regions);
        let mut entry =
            Artifact::bytes(regions.entry.clone(), regions.media_type.clone(), document.into_bytes())
                .as_entry();
        entry.refs.extend(regions.refs.iter().cloned());
        match out.table(DOCUMENT) {
            Ok(set) => match set.insert_entry_with(entry, &beside) {
                Ok(reports) => reports.into_iter().for_each(|report| out.diags.push(report)),
                Err(violation) => stage_failed(out, FINAL_STAGE, violation),
            },
            Err(error) => stage_failed(out, FINAL_STAGE, error),
        }
    }
}

/// The fixed skeleton over `regions`: a comment naming the style file, the
/// class `article`, `\usepackage` of the style file, then between
/// `\begin{document}` and `\end{document}` the pieces placed before the
/// content, the content and the pieces placed after it, a blank line after
/// each piece.
fn skeleton(regions: &Regions) -> String {
    let package = regions.get(STYLE_PACKAGE).map(Region::text).unwrap_or_default();
    let mut out = format!(
        "{SKELETON_HEADER}{package}.{STYLE_EXTENSION}\n\\documentclass{{article}}\n\\usepackage{{{package}}}\n\\begin{{document}}\n"
    );
    let piece = |out: &mut String, region: &Region| {
        let text = region.text();
        if !text.is_empty() {
            out.push_str(&text);
            out.push('\n');
        }
    };
    for region in regions.pieces_at(PiecePlace::BeforeContent) {
        piece(&mut out, region);
    }
    out.push_str(&regions.get(CONTENT).map(Region::text).unwrap_or_default());
    for region in regions.pieces_at(PiecePlace::AfterContent) {
        if !region.text().is_empty() {
            out.push('\n');
            piece(&mut out, region);
        }
    }
    out.push_str("\\end{document}\n");
    out
}

/// Returns the backend's three stages in the order a backend declaration
/// takes them: the lowering stage ([`LoweringStage`] for `latex-rendered`),
/// the emit stage ([`LatexRenderedEmitStage`]) and the final stage
/// ([`LatexRenderedFinalStage`]).
pub fn stages() -> (Arc<dyn Stage>, Arc<dyn Stage>, Arc<dyn Stage>) {
    (
        Arc::new(LoweringStage::new(BACKEND)),
        Arc::new(LatexRenderedEmitStage),
        Arc::new(LatexRenderedFinalStage),
    )
}

/// Returns the style file of the set `set`: its artifact of owner
/// `latex-rendered` other than the entry artifact.
pub fn style_artifact(set: &ArtifactSet) -> Option<&Artifact> {
    set.iter().find(|artifact| artifact.id.owner.as_str() == ARTIFACT_OWNER && !artifact.entry)
}
