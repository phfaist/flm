//! This module defines [`PiecesStage`], the stage that lowers the pieces of a
//! document for one backend, along with the table [`LoweredPieces`] that this
//! stage writes.
//!
//! Each backend that places pieces registers its own pieces stage, named
//! `flm.<backend>.pieces`, and the table `flm.lowered.<backend>.pieces`
//! ([`lowered_pieces_table_name`]). The pieces
//! stage runs once for the whole document. The other stages of the backend
//! read the lowered pieces from this table and write them into the output.
//!
//! # Building the plan
//!
//! When the plan is built, the pieces stage determines which pieces are placed
//! on its backend during the render. It calls [`selected_pieces`] with the
//! [`PiecesRenderOptions`] of the render request, or with the default options
//! if the request sets none. The stage then declares the following needs:
//!
//! - A read of the table of each selected piece. The *builder* of each
//!   selected piece is therefore scheduled in the plan. The builder of a piece
//!   is the stage that builds the content of the piece and writes it into this
//!   table. The builder of a piece that is left out of the render is not
//!   scheduled.
//! - The needs of the lowering rules selected for the backend
//!   ([`rules_needs`]). The pieces stage lowers the pieces with these rules.
//! - The option type [`PiecesRenderOptions`]. This option type is declared
//!   only in the `pieces` feature. If a registry contains a pieces stage but
//!   not the `pieces` feature, building a plan from this registry fails with
//!   [`PlanError::NeedsUndeclared`](crate::core::plan::PlanError::NeedsUndeclared).
//!
//! # Lowering the pieces
//!
//! When the pieces stage runs, it processes the selected pieces in the order
//! of the service slot [`SLOT`](super::SLOT). For each piece, it reads the
//! content of the piece from the table of the piece. If the table contains no
//! content, the piece is skipped, and no diagnostic is reported. Otherwise,
//! the stage brings the content up to date ([value
//! catch-up](crate::guide::concepts_overview#value-catch-up)) and lowers it
//! with the lowering rules selected for the backend ([`lower_value`]).
//!
//! Bringing the content up to date or lowering it can fail. For instance,
//! content that defines a label cannot be brought up to date. In such a case,
//! the stage reports the error `flm.pieces.lowering-failed`
//! ([`PieceLoweringFailed`]) and leaves the piece out. The other pieces and
//! the rest of the document are processed as usual.
//!
//! A lowering rule that runs inside a piece reads the tables of the document.
//! A reference inside a piece therefore resolves to a label of the document.
//! The [rendering guide](crate::guide::rendering#pieces) describes which
//! table entries a lowering rule finds inside a piece.

use techy::error::Diagnostic;
use techy::serialize::SerialValue;

use crate::core::data::store::StageOut;
use crate::core::data::table::{PieceClass, ProjectCx, Table, TableHandle, TableKey};
use crate::core::piece::PiecePlace;
use crate::core::plan::PlanCx;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::{FlmOrigin, FlmSpan, nowhere};
use crate::model::names::{BackendName, CapName, Name};
use crate::stdlib::backends::lowering::stage::BackendLowering;
use crate::stdlib::backends::lowering::{LoweredTree, lower_value, rules_needs};

use super::error::{PieceLoweringFailed, UnknownPiece};
use super::feature::{PiecesRenderOptions, selected_pieces, unknown_pieces};

/// The label of the synthetic source at which this stage reports a condition
/// that does not concern a position in a document.
const NOWHERE_LABEL: &str = "pieces";

/// A piece lowered for one backend, as stored in the table [`LoweredPieces`].
#[derive(Debug, Clone)]
pub struct LoweredPiece {
    /// The name of the piece.
    pub name: Name,
    /// The place of the piece in the output: before or after the document's
    /// content.
    pub place: PiecePlace,
    /// The lowered content of the piece, in the same form as a lowered
    /// document tree.
    pub tree: LoweredTree,
}

/// This struct contains the pieces placed on one backend during a render,
/// lowered for this backend.
///
/// It is the value of the table `flm.lowered.<backend>.pieces`. The
/// [`PiecesStage`] of the backend writes this table, and the other stages of
/// the backend read it to write the pieces into the output.
///
/// The pieces are listed in the order of the service slot
/// [`SLOT`](super::SLOT). A piece without content and a piece whose lowering
/// failed have no entry. Use [`at`](Self::at) to iterate over the pieces at
/// one place.
///
/// The serialized form of the table lists the name, the place, the number of
/// lowering rounds and the requirements of each piece. It does not contain
/// the lowered trees.
#[derive(Debug, Clone, Default)]
pub struct LoweredPieces {
    /// The lowered pieces, in the order of the service slot.
    pub pieces: Vec<LoweredPiece>,
}

impl LoweredPieces {
    /// Returns the lowered pieces placed at `place`, in the order of the
    /// service slot [`SLOT`](super::SLOT).
    pub fn at(&self, place: PiecePlace) -> impl Iterator<Item = &LoweredPiece> + '_ {
        self.pieces.iter().filter(move |piece| piece.place == place)
    }
}

impl Table for LoweredPieces {
    fn absorb(&mut self, piece: LoweredPieces) {
        self.pieces.extend(piece.pieces);
    }

    fn project(&self, _cx: &dyn ProjectCx) -> SerialValue {
        SerialValue::List(
            self.pieces
                .iter()
                .map(|piece| {
                    SerialValue::Map(vec![
                        (String::from("name"), SerialValue::Str(piece.name.to_string())),
                        (String::from("place"), SerialValue::Str(piece.place.as_str().into())),
                        (String::from("rounds"), SerialValue::Int(i64::from(piece.tree.rounds))),
                        (
                            String::from("requires"),
                            SerialValue::List(
                                piece
                                    .tree
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

/// Returns the name of the table that contains the lowered pieces of
/// `backend`, `flm.lowered.<backend>.pieces`.
///
/// If you register a [`PiecesStage`] for `backend` in a feature, declare a
/// table of type [`LoweredPieces`] under this name in the same feature, with
/// [`FeatureBuilder::table_named`](crate::core::registry::FeatureBuilder::table_named).
///
/// # Examples
///
/// ```
/// use flm_core::stdlib::backends::pieces::lowered_pieces_table_name;
///
/// assert_eq!(lowered_pieces_table_name(&"text".into()).as_str(), "flm.lowered.text.pieces");
/// ```
pub fn lowered_pieces_table_name(backend: &BackendName) -> CapName {
    CapName::from(format!("flm.lowered.{backend}.pieces"))
}

/// Returns the typed handle through which a stage reads or writes the table
/// that contains the lowered pieces of `backend`.
///
/// Creating the handle does not declare the table. Declare the table in the
/// feature in which you register the backend, under the name that
/// [`lowered_pieces_table_name`] returns.
pub fn lowered_pieces_table(backend: &BackendName) -> TableHandle<LoweredPieces> {
    TableHandle::new(lowered_pieces_table_name(backend))
}

/// The stage that lowers the pieces placed on one backend during a render, and
/// writes them into the table `flm.lowered.<backend>.pieces`
/// ([`LoweredPieces`]).
///
/// The stage is named `flm.<backend>.pieces`. The [module
/// documentation](crate::stdlib::backends::pieces::stage) describes what the
/// stage reads and how it lowers the pieces.
///
/// If your backend places pieces, create its pieces stage with
/// [`PiecesStage::new`], and register the stage with
/// [`FeatureBuilder::stage`](crate::core::registry::FeatureBuilder::stage) in
/// the feature in which you register the backend. In the same feature, declare
/// the table of the stage under the name that [`lowered_pieces_table_name`]
/// returns.
#[derive(Debug, Clone)]
pub struct PiecesStage {
    backend: BackendName,
    table: TableHandle<LoweredPieces>,
    only_named: bool,
}

impl PiecesStage {
    /// Creates the pieces stage of `backend`.
    ///
    /// The stage is named `flm.<backend>.pieces` and writes the table
    /// `flm.lowered.<backend>.pieces`. It lowers every piece selected for
    /// `backend` with the render options; see
    /// [`placing_only_named`](Self::placing_only_named) for the alternative.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdlib::backends::pieces::PiecesStage;
    ///
    /// let stage = PiecesStage::new("text");
    /// assert_eq!(stage.name(), "flm.text.pieces");
    /// assert_eq!(stage.table().name().as_str(), "flm.lowered.text.pieces");
    /// ```
    pub fn new(backend: impl Into<BackendName>) -> PiecesStage {
        let backend = backend.into();
        let table = lowered_pieces_table(&backend);
        PiecesStage { backend, table, only_named: false }
    }

    /// Makes the backend place pieces only when pieces are listed for this
    /// backend in the render options, and returns the stage.
    ///
    /// The pieces placed on each backend during a render can be listed in the
    /// render options ([`PiecesRenderOptions::backends`]). By default, a
    /// backend without an entry in this list places every piece selected with
    /// `include` and `exclude`. With this setting, a backend without an entry
    /// places no piece.
    ///
    /// The recomposed LaTeX backend uses this setting. In a LaTeX document, the
    /// pieces are placed at specific positions, such as the title in
    /// `\title{…}`. The template of the document contains these positions, but
    /// the default output of the backend does not.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdlib::backends::pieces::PiecesStage;
    ///
    /// let stage = PiecesStage::new("latex-recomposed").placing_only_named();
    /// assert!(stage.places_only_named());
    /// assert!(!PiecesStage::new("text").places_only_named());
    /// ```
    pub fn placing_only_named(mut self) -> PiecesStage {
        self.only_named = true;
        self
    }

    /// Returns `true` if the backend places pieces only when pieces are listed
    /// for this backend in the render options
    /// ([`placing_only_named`](Self::placing_only_named)).
    pub fn places_only_named(&self) -> bool {
        self.only_named
    }

    /// Returns the backend for which this stage lowers pieces.
    pub fn backend(&self) -> &BackendName {
        &self.backend
    }

    /// Returns the name of the stage, `flm.<backend>.pieces`.
    pub fn name(&self) -> String {
        format!("flm.{}.pieces", self.backend)
    }

    /// Returns the typed handle of the table that this stage writes,
    /// `flm.lowered.<backend>.pieces`.
    pub fn table(&self) -> &TableHandle<LoweredPieces> {
        &self.table
    }

    fn failed(&self, out: &mut StageOut<'_>, piece: &Name, detail: String, span: FlmSpan) {
        out.diags.push(Diagnostic::error(
            PieceLoweringFailed {
                piece: piece.to_string(),
                backend: self.backend.to_string(),
                detail,
            },
            span,
        ));
    }
}

impl Stage for PiecesStage {
    fn decl(&self) -> StageDecl {
        StageDecl::new(self.name(), Effect::Compute, Scope::Document)
            .writes(self.table.clone())
            .docs(format!(
                "lowers every piece placed on the `{}` backend during the render",
                self.backend
            ))
    }

    /// Returns the needs of the stage: the needs of the lowering rules
    /// selected for the backend ([`rules_needs`]), a read of the table of each
    /// piece placed during the render, and the option type
    /// [`PiecesRenderOptions`].
    ///
    /// The [module documentation](crate::stdlib::backends::pieces::stage)
    /// explains the effect of these needs on the plan.
    ///
    /// # Panics
    ///
    /// Panics if the rule selection of the plan context was built against
    /// another registry than the one that the context contains, as
    /// [`rules_needs`] does. This happens only if values from two registries
    /// are combined by hand.
    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        let mut needs = rules_needs(cx, &self.backend);
        let default = PiecesRenderOptions::default();
        let options = cx.render_option::<PiecesRenderOptions>().unwrap_or(&default);
        for decl in selected_pieces(cx.registry(), options, &self.backend, self.only_named) {
            if !needs.reads.iter().any(|read| &read.name == decl.table.name()) {
                needs.reads.push(decl.table.table_ref());
            }
        }
        needs.option::<PiecesRenderOptions>()
    }

    /// Lowers each piece placed during the render, and writes the lowered
    /// pieces into the table of the stage.
    ///
    /// The stage reports the warning `flm.pieces.unknown-piece`
    /// ([`UnknownPiece`]) for each name in the options `include` and `exclude`
    /// that is not the name of a piece defined in any feature. It reports the
    /// error
    /// `flm.pieces.lowering-failed` ([`PieceLoweringFailed`]) for each piece
    /// that cannot be lowered, and leaves this piece out of the table. The
    /// diagnostics reported while a piece is lowered are reported as well.
    ///
    /// # Panics
    ///
    /// Panics under the condition described at [`needs`](Self::needs). A
    /// panic of a rewrite stage, or of a stage that runs again while the
    /// content of a piece is brought up to date, propagates out of this
    /// method, as it does out of
    /// [`StageCx::resolve_value`](crate::core::run::StageCx::resolve_value).
    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let registry = cx.registry();
        let options = match cx.option::<PiecesRenderOptions>() {
            Ok(options) => options,
            // The option was declared among the needs, so the plan could not
            // have been built without it.
            Err(_) => return,
        };
        for name in unknown_pieces(registry, options) {
            out.diags.push(Diagnostic::warning(
                UnknownPiece { piece: name.to_string() },
                nowhere(FlmOrigin::synthetic(NOWHERE_LABEL)),
            ));
        }
        let Some(setting) = BackendLowering::of(cx, &self.backend) else { return };
        let inputs = setting.inputs(&self.backend, registry);
        let mut lowered = LoweredPieces::default();
        for decl in selected_pieces(registry, options, &self.backend, self.only_named) {
            let here = || nowhere(FlmOrigin::synthetic(NOWHERE_LABEL));
            let piece = match cx.get(&decl.table) {
                Ok(Some(piece)) => piece,
                // A builder that ran and failed wrote nothing: nothing is
                // placed, and the builder reported why.
                Ok(None) => continue,
                Err(error) => {
                    self.failed(out, &decl.name, error.to_string(), here());
                    continue;
                }
            };
            let Some(content) = &piece.content else { continue };
            let span = content.root().span().clone();
            match lower_value(cx, &inputs, content) {
                Err(error) => {
                    if let Some(diagnostic) = error.diagnostic() {
                        out.diags.push(diagnostic.clone());
                    }
                    self.failed(out, &decl.name, error.to_string(), span);
                }
                Ok(outcome) => {
                    for diagnostic in outcome.diags {
                        out.diags.push(diagnostic);
                    }
                    match outcome.lowered {
                        Some(tree) => lowered.pieces.push(LoweredPiece {
                            name: decl.name.clone(),
                            place: decl.place,
                            tree,
                        }),
                        None => self.failed(
                            out,
                            &decl.name,
                            String::from("its content contains errors"),
                            span,
                        ),
                    }
                }
            }
        }
        match out.table(&self.table) {
            Ok(table) => *table = lowered,
            // The table is the stage's declared write, so this is a plan the
            // stage's declaration does not describe: every piece is then left
            // out, each reported.
            Err(error) => {
                for piece in &lowered.pieces {
                    let span = nowhere(FlmOrigin::synthetic(NOWHERE_LABEL));
                    self.failed(out, &piece.name, error.to_string(), span);
                }
            }
        }
    }
}
