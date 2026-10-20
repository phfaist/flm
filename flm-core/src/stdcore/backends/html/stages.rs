//! The stages of the HTML backend after lowering, the tables they write and
//! read, the artifact they produce, and the conditions they report.
//!
//! An `html` run goes through these stages after the lowering stage
//! (`flm.html.lower`, which writes the lowered trees `flm.lowered.html`):
//!
//! 1. The **emit stage** `flm.html.emit` ([`HtmlEmitStage`], once per tree)
//!    writes each lowered tree's HTML ([`emit_tree`]) into the per-tree output
//!    table `flm.output.html` ([`HtmlFragments`]): one [`HtmlFragment`] per
//!    tree, holding the tree's HTML in **segments** (bytes, or another tree
//!    placed at that point) and the identifiers of the style and script
//!    definitions the tree requires. Trees are emitted independently, possibly
//!    at the same time on different threads; the table holds them by tree, so
//!    the result does not depend on how the run was executed.
//! 2. The **pieces stages** handle the [pieces](crate::core::piece::Piece) the
//!    render places: document-level synthesized values such as the title or a list of
//!    notes. The shared pieces stage
//!    ([`PiecesStage`](crate::stdcore::backends::pieces::PiecesStage), named
//!    `flm.html.pieces` here) lowers them into `flm.lowered.html.pieces`; the
//!    stage `flm.html.emit-pieces` ([`HtmlEmitPiecesStage`], once per
//!    document) writes each lowered piece's HTML, with the same emitter, into
//!    `flm.output.html.pieces` ([`HtmlPieces`]), with the definitions it
//!    requires.
//! 3. The stage `flm.html.resolve-definitions`
//!    ([`ResolveDefinitionsStage`](super::defs::ResolveDefinitionsStage), once
//!    per document) decides how each required definition reaches the page and
//!    writes the definitions table `flm.html.definitions`.
//! 4. The **regions stage** `flm.html.regions` ([`HtmlRegionsStage`], once per
//!    document) makes the page's [output regions](crate::core::region::Regions)
//!    ([`page::regions`]): the content, with each placed
//!    tree written where it was placed, each piece, the stylesheets, the
//!    scripts, the words of the page's title and its language. It writes them
//!    into `flm.output.html.regions` ([`REGIONS`]). It reads the per-tree
//!    table, the pieces table, the definitions table and the render options;
//!    because it reads the definitions table, the plan schedules the resolve
//!    stage.
//! 5. The **final stage** `flm.html.final.singlefile` ([`HtmlFinalStage`], once
//!    per document) is the backend's final stage: it assembles the page from
//!    the regions ([`page::assemble`]) and writes it as
//!    the entry artifact `html:document`, of media type `text/html`, of the
//!    artifact set in `flm.output.html.document` ([`DOCUMENT`]). That table is
//!    the `html` output's table. A host that assembles the regions through a
//!    template of its own provides the same table from the same regions, and
//!    the plan runs one of the two (see [output regions and the final
//!    stage](crate::guide::concepts_overview#output-regions-and-the-final-stage)).
//!
//! The plan derives this order from the tables each stage reads and writes; no
//! stage declares it.
//!
//! # Artifacts beside the page
//!
//! A page may link to files that belong with it, for instance a copy of each
//! image it shows. Those files are made while a document is rendered, so a
//! stage of the plan writes them into the **artifact table**
//! `flm.output.html.artifacts` ([`ARTIFACTS`]), an [`ArtifactSet`] that the
//! backend declares and that another feature's stage writes, for instance the
//! graphics feature's publish stage `flm.graphics.publish.html`, which writes
//! the copies of image files.
//! When a stage of the plan wrote that table, the final stage adds every
//! artifact it holds to the page's set, after the page itself: the output is
//! then the page as the entry artifact and the files it needs beside it. An
//! artifact whose identity is the page's, or that claims to be the entry
//! artifact, is recorded among the set's
//! [violations](ArtifactSet::violations) and is not added as such. A table has
//! one writer per plan, so a single stage writes every artifact of the render.
//!
//! # The capabilities of the output
//!
//! The name of the artifact-set table, `flm.output.html.document`, is also the
//! [capability](crate::core::registry::CapabilityDef) "the document has been
//! written as HTML". The capability
//! [`SINGLEFILE`] refines it and says how: as one HTML file. It is the default
//! among the refinements, and the final stage of this module provides it. A
//! multi-file output (not yet implemented) would be a second final stage
//! providing a second refinement and writing the same table; because a plan
//! schedules one writer per table, a request for both would be refused by the
//! plan builder rather than produce two pages.
//!
//! # Trees placed in other trees
//!
//! A `flm.pres.subtree` node
//! ([`SUBTREE`](crate::stdcore::backends::pres::SUBTREE),
//! [`SubtreePayload`](crate::stdcore::backends::pres::SubtreePayload)) places
//! another document tree at its position. The emit stage sees one tree only,
//! so it ends the segment it is writing and
//! records the placed tree ([`HtmlSegment::Tree`]). The regions stage, which
//! sees every tree, writes the placed tree's own segments at that point, and
//! reports [`MissingSubtree`] or [`SubtreeCycle`] where it cannot, as the text
//! backend does.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use techy::error::{Diagnostic, DiagnosticInfo};
use techy::serialize::SerialValue;

use crate::core::artifact::{Artifact, ArtifactSet};
use crate::core::region::Regions;
use crate::core::data::store::StageOut;
use crate::core::data::table::{PieceClass, ProjectCx, SingletonTable, Table, TableName};
use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::{FlmOrigin, nowhere};
use crate::core::piece::PiecePlace;
use crate::model::addr::TreeId;
use crate::model::names::{BackendName, Name};
use crate::stdcore::backends::lowering::{LoweringStage, lowered_table};
use crate::stdcore::backends::pieces::lowered_pieces_table;

use super::defs::{DEFINITIONS_TABLE, ResolvedDefinitions};
use super::emit::{NOWHERE_LABEL, emit_tree};
use super::feature::{BACKEND, HtmlRenderOptions};
use super::page;

/// The name of the emit stage, `flm.html.emit` ([`HtmlEmitStage`]).
pub const EMIT_STAGE: &str = "flm.html.emit";
/// The name of the stage that writes the pieces' HTML, `flm.html.emit-pieces`
/// ([`HtmlEmitPiecesStage`]).
pub const EMIT_PIECES_STAGE: &str = "flm.html.emit-pieces";
/// The name of the regions stage, `flm.html.regions` ([`HtmlRegionsStage`]).
pub const REGIONS_STAGE: &str = "flm.html.regions";
/// The name of the backend's final stage, `flm.html.final.singlefile`
/// ([`HtmlFinalStage`]), which writes the page.
pub const FINAL_STAGE: &str = "flm.html.final.singlefile";
/// The owner part of the page's artifact identifier `html:document`: `html`.
pub const ARTIFACT_OWNER: &str = "html";
/// The name part of the page's artifact identifier `html:document`:
/// `document`.
pub const ARTIFACT_NAME: &str = "document";
/// The media type of the page, `text/html`.
pub const MEDIA_TYPE: &str = "text/html";
/// The file extension, `html`, that a host writing the page to a file should
/// use.
pub const EXTENSION: &str = "html";

/// The typed name of the per-tree output table, `flm.output.html`
/// ([`HtmlFragments`]).
pub const OUTPUT_TABLE: TableName<HtmlFragments> = TableName::new("flm.output.html");

/// The typed name of the pieces' output table, `flm.output.html.pieces`
/// ([`HtmlPieces`]).
pub const PIECES_TABLE: TableName<HtmlPieces> = TableName::new("flm.output.html.pieces");

/// The typed name of the regions table, `flm.output.html.regions`: the output
/// regions the page is made of ([`Regions`]), written by the regions stage and
/// read by the final stage.
pub const REGIONS: TableName<Regions> = TableName::new("flm.output.html.regions");

/// The typed name of the artifact-set table, `flm.output.html.document`: the
/// `html` output's table, whose entry artifact is the page. Its name is also the
/// capability "the document has been written as HTML".
pub const DOCUMENT: TableName<ArtifactSet> = TableName::new("flm.output.html.document");

/// The typed name of the artifact table, `flm.output.html.artifacts`: files a
/// stage produces for the page to link to, which the final stage adds to the
/// page's set after the page (see the [module
/// documentation](self#artifacts-beside-the-page)). The HTML backend declares
/// it but does not write it; another feature's stage writes it, for instance
/// the graphics feature's publish stage.
pub const ARTIFACTS: TableName<ArtifactSet> = TableName::new("flm.output.html.artifacts");

/// The capability `flm.output.html.singlefile`: the document written as one
/// HTML page. It refines the capability named after [`DOCUMENT`] and is the
/// default among its refinements, so a caller who asks for the `html` output
/// and says nothing more gets this one. [`HtmlFinalStage`] provides it.
pub const SINGLEFILE: &str = "flm.output.html.singlefile";

// --- the tables -------------------------------------------------------------------------

/// One segment of a document tree's HTML: bytes, or another tree placed at
/// that point.
///
/// A tree's HTML is a list of segments rather than one string because another
/// document tree may be placed inside it: the emitter, which sees one tree,
/// records the placement, and the regions stage writes the placed tree's own
/// HTML there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HtmlSegment {
    /// HTML that this tree's emission wrote.
    Html(String),
    /// Another document tree, placed at this point by a `flm.pres.subtree`
    /// node.
    Tree(TreeId),
}

/// One document tree's HTML and the definitions it requires: the record the
/// emit stage writes per tree into [`HtmlFragments`]. A piece's HTML has the
/// same form ([`HtmlPiece::fragment`]).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HtmlFragment {
    /// The tree's HTML, in order. Empty when nothing was written for the tree.
    pub segments: Vec<HtmlSegment>,
    /// The identifiers of the style and script definitions this tree
    /// requires: what its lowering rules required, together with what the
    /// emitter required while writing. An ordered set, so that two runs over
    /// one document produce the same list.
    ///
    /// A requirement is only an identifier; the delivery of each definition is
    /// decided by [`ResolveDefinitionsStage`](super::defs::ResolveDefinitionsStage).
    pub requires: BTreeSet<Name>,
}

impl HtmlFragment {
    /// Returns the tree's own HTML with nothing in place of the trees it
    /// places: the bytes of its [`HtmlSegment::Html`] segments, concatenated.
    ///
    /// A test, or an embedder that does not use placed trees, reads this; the
    /// regions stage reads the segments instead.
    pub fn html(&self) -> String {
        let mut out = String::new();
        for segment in &self.segments {
            if let HtmlSegment::Html(html) = segment {
                out.push_str(html);
            }
        }
        out
    }
}

/// The per-tree output table, `flm.output.html` ([`OUTPUT_TABLE`]): one
/// [`HtmlFragment`] per document tree.
///
/// An embedder reads this table when it wants the HTML of a document's content
/// without the page around it. Each entry's
/// [`requires`](HtmlFragment::requires) lists the definitions that tree's HTML
/// requires, and the definitions table
/// ([`DEFINITIONS_TABLE`]) gives their
/// delivery. The emit stage [`HtmlEmitStage`] writes it, one tree per
/// invocation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HtmlFragments {
    /// Each document tree's HTML and requirements, by tree.
    pub trees: BTreeMap<TreeId, HtmlFragment>,
}

impl Table for HtmlFragments {
    fn absorb(&mut self, piece: HtmlFragments) {
        self.trees.extend(piece.trees);
    }

    /// One entry per tree: `{tree, segments: [{html} | {tree}], requires:
    /// [id]}`.
    fn project(&self, cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.trees
                .iter()
                .map(|(id, fragment)| {
                    SerialValue::Map(vec![
                        (String::from("tree"), cx.tree_ref(*id)),
                        (
                            String::from("segments"),
                            SerialValue::List(
                                fragment
                                    .segments
                                    .iter()
                                    .map(|segment| match segment {
                                        HtmlSegment::Html(html) => SerialValue::Map(vec![(
                                            String::from("html"),
                                            SerialValue::Str(html.clone()),
                                        )]),
                                        HtmlSegment::Tree(placed) => SerialValue::Map(vec![(
                                            String::from("tree"),
                                            cx.tree_ref(*placed),
                                        )]),
                                    })
                                    .collect(),
                            ),
                        ),
                        (
                            String::from("requires"),
                            SerialValue::List(
                                fragment
                                    .requires
                                    .iter()
                                    .map(|name| SerialValue::Str(name.as_str().to_string()))
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

impl SingletonTable for HtmlFragments {
    const NAME: &'static str = "flm.output.html";
}

/// One piece's HTML: the piece's name, where the page places it, its HTML with
/// the definitions it requires, and its words.
///
/// A piece places no document tree, so its HTML holds [`HtmlSegment::Html`]
/// segments only.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HtmlPiece {
    /// The piece's name.
    pub name: Name,
    /// Where the page places it.
    pub place: PiecePlace,
    /// Its HTML and the definitions that HTML requires: what its lowering
    /// rules required, together with what the emitter required while writing.
    pub fragment: HtmlFragment,
    /// Its words: the plain text of its lowered tree, with every run of white
    /// space replaced by one space
    /// ([`piece_words`](crate::core::region::piece_words)).
    pub words: String,
}

/// The pieces' output table, `flm.output.html.pieces` ([`PIECES_TABLE`]): one
/// [`HtmlPiece`] per piece the render places, in the order in which the pieces
/// were declared.
///
/// The regions stage makes one output region per piece from it, which the page
/// places before and after the content; a host assembling its own page reads
/// it beside `flm.output.html`. The stage [`HtmlEmitPiecesStage`] writes it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HtmlPieces {
    /// The pieces' HTML, in the order in which the pieces were declared.
    pub pieces: Vec<HtmlPiece>,
}

impl HtmlPieces {
    /// Returns the pieces placed at `place`, in order.
    pub fn at(&self, place: PiecePlace) -> impl Iterator<Item = &HtmlPiece> + '_ {
        self.pieces.iter().filter(move |piece| piece.place == place)
    }
}

impl Table for HtmlPieces {
    fn absorb(&mut self, piece: HtmlPieces) {
        self.pieces.extend(piece.pieces);
    }

    /// One entry per piece: `{name, place, html, requires: [id]}`.
    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.pieces
                .iter()
                .map(|piece| {
                    SerialValue::Map(vec![
                        (String::from("name"), SerialValue::Str(piece.name.to_string())),
                        (String::from("place"), SerialValue::Str(piece.place.as_str().into())),
                        (String::from("html"), SerialValue::Str(piece.fragment.html())),
                        (
                            String::from("requires"),
                            SerialValue::List(
                                piece
                                    .fragment
                                    .requires
                                    .iter()
                                    .map(|name| SerialValue::Str(name.to_string()))
                                    .collect(),
                            ),
                        ),
                    ])
                })
                .collect(),
        )
    }

    fn piece_class(&self) -> PieceClass {
        if self.pieces.is_empty() { PieceClass::Empty } else { PieceClass::DocumentFact }
    }
}

impl SingletonTable for HtmlPieces {
    const NAME: &'static str = "flm.output.html.pieces";
}

// --- conditions -------------------------------------------------------------------------

/// Condition `flm.html.stage-failed` (Error): a stage of this backend could not
/// read or write a table or an option it declared, or the emitter met an
/// element it does not write.
///
/// The first case means the stage runs in a plan its declarations do not
/// describe; the second means an element outside the backend's native set
/// reached the emitter. Both report a mistake in the library or in an
/// extension, never anything a document can write. Recovery: the stage's
/// output is absent, or the node writes nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.html.stage-failed",
    message = "the stage `{stage}` failed: {detail}"
)]
pub struct StageFailed {
    /// The name of the stage.
    pub stage: String,
    /// What failed, as text.
    pub detail: String,
}

/// Condition `flm.html.missing-subtree` (Error): a `flm.pres.subtree` node
/// places a document tree that has no HTML for this backend, because its
/// lowering or its emission failed, or because the identifier refers to no
/// tree.
///
/// Reported by the regions stage, at no position in a document. Recovery:
/// nothing is written in the node's place.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.html.missing-subtree",
    message = "the document tree `{tree}` is placed inside another tree, but it has no HTML to \
               write there: no tree has that identifier, or that tree could not be converted to \
               HTML"
)]
pub struct MissingSubtree {
    /// The placed tree.
    pub tree: String,
}

/// Condition `flm.html.subtree-cycle` (Error): a `flm.pres.subtree` node
/// places a document tree that is already being written, so the tree would be
/// placed inside itself, directly or through other trees (a placement cycle).
///
/// Reported by the regions stage, at no position in a document. Recovery:
/// nothing is written in the node's place.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.html.subtree-cycle",
    message = "the document tree `{tree}` must not be placed inside itself, but it is, \
               directly or through other trees"
)]
pub struct SubtreeCycle {
    /// The placed tree.
    pub tree: String,
}

/// Condition `flm.html.piece-placement` (Error): a `flm.pres.subtree` node is
/// inside a piece.
///
/// A [piece](crate::core::piece::Piece) is a document-level value placed
/// beside the document's content (the title, a list of notes); it is never
/// placed inside a tree and never places a tree. Reported by
/// [`HtmlEmitPiecesStage`] at the node's position. Recovery: nothing is written
/// in the node's place.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.html.piece-placement",
    message = "the document tree `{tree}` cannot be placed inside `{piece}`, which the page \
               places beside the document's content"
)]
pub struct PiecePlacement {
    /// The name of the piece.
    pub piece: String,
    /// The tree the node places.
    pub tree: String,
}

/// Reports [`StageFailed`] at no position in any document.
pub(super) fn stage_failed(out: &mut StageOut<'_>, stage: &str, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: String::from(stage), detail: detail.to_string() },
        nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
    ));
}

// --- the emit stage ---------------------------------------------------------------------

/// The emit stage, `flm.html.emit` ([`EMIT_STAGE`]): it writes one lowered
/// tree's HTML into [`HtmlFragments`], once per tree (see the [module
/// documentation](self)).
///
/// It reads the lowered-trees table `flm.lowered.html` and the render options
/// ([`HtmlRenderOptions`]), and writes `flm.output.html`. It calls
/// [`emit_tree`] and adds the tree's lowering requirements to what the
/// emitter required. When a table or the options cannot be read or written, it
/// reports [`StageFailed`] and writes nothing for the tree; the emitter's own
/// conditions are reported as they come.
#[derive(Debug, Default, Clone, Copy)]
pub struct HtmlEmitStage;

impl Stage for HtmlEmitStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(EMIT_STAGE, Effect::Compute, Scope::PerTree)
            .writes(OUTPUT_TABLE)
            .docs(
                "writes each lowered tree's HTML, and the stylesheets and scripts it requires, \
                 into `flm.output.html`",
            )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
            .reads(lowered_table(&BackendName::from(BACKEND)))
            .option::<HtmlRenderOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let Some(id) = cx.tree_id() else { return };
        let trees = match cx.get(lowered_table(&BackendName::from(BACKEND))) {
            Ok(Some(trees)) => trees,
            Ok(None) => return,
            Err(error) => return stage_failed(out, EMIT_STAGE, error),
        };
        let Some(lowered) = trees.get(id) else { return };
        let options = match cx.option::<HtmlRenderOptions>() {
            Ok(options) => options,
            Err(error) => return stage_failed(out, EMIT_STAGE, error),
        };
        let emitted = emit_tree(&lowered.tree, options);
        for diagnostic in emitted.diags.iter() {
            out.diags.push(diagnostic.clone());
        }
        let mut fragment =
            HtmlFragment { segments: emitted.segments, requires: emitted.requires };
        // What the tree's lowering rules required joins what the emitter
        // required while writing: one set per tree.
        fragment.requires.extend(lowered.requires.iter().cloned());
        match out.table(OUTPUT_TABLE) {
            Ok(table) => {
                table.trees.insert(id, fragment);
            }
            Err(error) => stage_failed(out, EMIT_STAGE, error),
        }
    }
}

// --- the pieces' emit stage -------------------------------------------------------------

/// The stage `flm.html.emit-pieces` ([`EMIT_PIECES_STAGE`]): it writes each
/// lowered piece's HTML into [`HtmlPieces`], once per document (see the
/// [module documentation](self)).
///
/// It reads the lowered-pieces table `flm.lowered.html.pieces` and the render
/// options, and writes `flm.output.html.pieces`. Each piece is written by the
/// emitter that writes a tree ([`emit_tree`]); a `flm.pres.subtree` node inside
/// a piece writes nothing and is reported as [`PiecePlacement`].
#[derive(Debug, Default, Clone, Copy)]
pub struct HtmlEmitPiecesStage;

impl Stage for HtmlEmitPiecesStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(EMIT_PIECES_STAGE, Effect::Compute, Scope::Document)
            .writes(PIECES_TABLE)
            .docs(
                "writes each lowered piece's HTML, and the stylesheets and scripts it requires, \
                 into `flm.output.html.pieces`",
            )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
            .reads(lowered_pieces_table(&BackendName::from(BACKEND)))
            .option::<HtmlRenderOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let lowered = match cx.get(lowered_pieces_table(&BackendName::from(BACKEND))) {
            Ok(Some(lowered)) => lowered,
            Ok(None) => return,
            Err(error) => return stage_failed(out, EMIT_PIECES_STAGE, error),
        };
        let options = match cx.option::<HtmlRenderOptions>() {
            Ok(options) => options,
            Err(error) => return stage_failed(out, EMIT_PIECES_STAGE, error),
        };
        let mut pieces = HtmlPieces::default();
        for piece in &lowered.pieces {
            let emitted = emit_tree(&piece.tree.tree, options);
            for diagnostic in emitted.diags.iter() {
                out.diags.push(diagnostic.clone());
            }
            let mut segments = Vec::with_capacity(emitted.segments.len());
            let mut placed_at = emitted.placed_at.into_iter();
            for segment in emitted.segments {
                match segment {
                    HtmlSegment::Html(html) => segments.push(HtmlSegment::Html(html)),
                    // A value is never placed and never places: the node
                    // writes nothing.
                    HtmlSegment::Tree(tree) => out.diags.push(Diagnostic::error(
                        PiecePlacement { piece: piece.name.to_string(), tree: tree.to_string() },
                        placed_at
                            .next()
                            .unwrap_or_else(|| nowhere(FlmOrigin::synthetic(NOWHERE_LABEL))),
                    )),
                }
            }
            let mut requires = emitted.requires;
            requires.extend(piece.tree.requires.iter().cloned());
            let words = crate::core::region::piece_words(&piece.tree.tree);
            pieces.pieces.push(HtmlPiece {
                name: piece.name.clone(),
                place: piece.place,
                fragment: HtmlFragment { segments, requires },
                words,
            });
        }
        match out.table(PIECES_TABLE) {
            Ok(table) => *table = pieces,
            Err(error) => stage_failed(out, EMIT_PIECES_STAGE, error),
        }
    }
}

// --- the regions stage -------------------------------------------------------------------

/// The regions stage, `flm.html.regions` ([`REGIONS_STAGE`]): it makes the
/// page's output regions from the trees' HTML, the pieces' HTML, the
/// definitions table and the render options ([`page::regions`]), and writes
/// them into [`REGIONS`], once per document (see the [module
/// documentation](self)).
///
/// When the pieces table or the definitions table is absent, it makes the
/// regions as if the table were empty. It reports [`MissingSubtree`],
/// [`SubtreeCycle`] and
/// [`DefinitionNotInlinable`](super::defs::DefinitionNotInlinable) for what the
/// regions could not hold, and [`StageFailed`] when a table or the options
/// cannot be read or written.
#[derive(Debug, Default, Clone, Copy)]
pub struct HtmlRegionsStage;

impl Stage for HtmlRegionsStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(REGIONS_STAGE, Effect::Compute, Scope::Document).writes(REGIONS).docs(
            "makes the page's regions — the content, the pieces, the stylesheets, the \
             scripts, the title's words and the language — into `flm.output.html.regions`",
        )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
            .reads(OUTPUT_TABLE)
            .reads(PIECES_TABLE)
            .reads(DEFINITIONS_TABLE)
            .option::<HtmlRenderOptions>()
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let fragments = match cx.get(OUTPUT_TABLE) {
            Ok(Some(fragments)) => fragments,
            Ok(None) => return,
            Err(error) => return stage_failed(out, REGIONS_STAGE, error),
        };
        // A plan in which the pieces were not written is the same page with
        // no piece placed.
        let no_pieces = HtmlPieces::default();
        let pieces = match cx.get(PIECES_TABLE) {
            Ok(Some(pieces)) => pieces,
            Ok(None) => &no_pieces,
            Err(error) => return stage_failed(out, REGIONS_STAGE, error),
        };
        // A document requiring no definition at all leaves the table empty
        // rather than absent; an absent one is the same page with nothing in
        // it, so the regions are made from an empty table and the page is
        // still produced.
        let none = ResolvedDefinitions::default();
        let definitions = match cx.get(DEFINITIONS_TABLE) {
            Ok(Some(definitions)) => definitions,
            Ok(None) => &none,
            Err(error) => return stage_failed(out, REGIONS_STAGE, error),
        };
        let options = match cx.option::<HtmlRenderOptions>() {
            Ok(options) => options,
            Err(error) => return stage_failed(out, REGIONS_STAGE, error),
        };
        let order: Vec<TreeId> = cx.trees().map(|(id, _)| id).collect();
        let made = page::regions(fragments, pieces, definitions, &order, options);
        for refused in made.not_inlinable {
            out.diags.push(Diagnostic::error(
                refused,
                nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
            ));
        }
        for tree in made.missing {
            out.diags.push(Diagnostic::error(
                MissingSubtree { tree: tree.to_string() },
                nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
            ));
        }
        for tree in made.cycles {
            out.diags.push(Diagnostic::error(
                SubtreeCycle { tree: tree.to_string() },
                nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
            ));
        }
        match out.table(REGIONS) {
            Ok(table) => *table = made.regions,
            Err(error) => stage_failed(out, REGIONS_STAGE, error),
        }
    }
}

// --- the final stage --------------------------------------------------------------------

/// The backend's final stage, `flm.html.final.singlefile` ([`FINAL_STAGE`]): it
/// assembles the page from the output regions ([`page::assemble`]) and writes
/// it as the entry artifact of the set in `flm.output.html.document`
/// ([`DOCUMENT`]), followed by every artifact of the artifact table
/// `flm.output.html.artifacts` ([`ARTIFACTS`]) when a stage of the plan wrote
/// that table; otherwise the set holds the page alone. It provides the
/// capability [`SINGLEFILE`].
///
/// An artifact beside the page that collides with the page or claims to be the
/// entry artifact is recorded among the set's
/// [violations](ArtifactSet::violations) and reported.
#[derive(Debug, Default, Clone, Copy)]
pub struct HtmlFinalStage;

impl Stage for HtmlFinalStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(FINAL_STAGE, Effect::Compute, Scope::Document)
            .provides(SINGLEFILE)
            .writes(DOCUMENT)
            .docs(
                "assembles the page from the regions in the backend's own layout and writes it \
                 as the entry artifact of `flm.output.html.document`",
            )
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(REGIONS).reads_opt(ARTIFACTS)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let regions = match cx.get(REGIONS) {
            Ok(Some(regions)) => regions,
            Ok(None) => return,
            Err(error) => return stage_failed(out, FINAL_STAGE, error),
        };
        let beside = match cx.get(ARTIFACTS) {
            Ok(beside) => beside,
            Err(error) => return stage_failed(out, FINAL_STAGE, error),
        };
        let page = page::assemble(regions);
        let mut entry =
            Artifact::bytes(regions.entry.clone(), regions.media_type.clone(), page.into_bytes())
                .as_entry();
        entry.refs.extend(regions.refs.iter().cloned());
        match out.table(DOCUMENT) {
            // A collision among the artifacts beside the page is recorded
            // among the set's violations.
            // See decision rationale at [§flm-dd-dr:rend-html-artifacts].
            Ok(set) => match set.insert_entry_with(entry, beside.as_slice()) {
                Ok(reports) => reports.into_iter().for_each(|report| out.diags.push(report)),
                Err(violation) => stage_failed(out, FINAL_STAGE, violation),
            },
            Err(error) => stage_failed(out, FINAL_STAGE, error),
        }
    }
}

/// Returns the backend's three stages as shared values, in the order a backend
/// declaration ([`BackendDef::new`](crate::core::registry::BackendDef::new))
/// takes them: the lowering stage ([`LoweringStage`] for the `html` backend),
/// the emit stage ([`HtmlEmitStage`]) and the final stage ([`HtmlFinalStage`]).
pub fn stages() -> (Arc<dyn Stage>, Arc<dyn Stage>, Arc<dyn Stage>) {
    (Arc::new(LoweringStage::new(BACKEND)), Arc::new(HtmlEmitStage), Arc::new(HtmlFinalStage))
}
