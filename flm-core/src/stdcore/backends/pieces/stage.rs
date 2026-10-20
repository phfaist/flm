//! A backend's pieces stage, [`PiecesStage`], and the table it writes,
//! [`LoweredPieces`].
//!
//! A **piece** is a document-level synthesized value that a backend places
//! before or after the document's content
//! ([`Piece`](crate::core::piece::Piece)). A backend that places pieces
//! registers one stage of this kind, named `flm.<backend>.pieces`, beside its
//! lowering stage, and declares the table `flm.lowered.<backend>.pieces` that
//! the stage writes ([`lowered_pieces_table_name`]). It is a `Compute` stage
//! of `Document` scope: it runs once for the whole document, after the
//! builder stage of every piece it places.
//!
//! # What the stage declares it needs
//!
//! When the plan is built, the stage asks [`selected_pieces`] which pieces the
//! render places on its backend, under the plan request's
//! [`PiecesRenderOptions`] (the default value when the request sets none). It
//! then declares three needs:
//!
//! - what the lowering rules selected for the backend need, the same union
//!   the lowering stage declares ([`rules_needs`]), since it lowers the pieces
//!   with those rules;
//! - a read of every selected piece's table, which schedules each piece's
//!   builder stage (or the stage the request prefers over it) and nothing
//!   else: a piece the render leaves out schedules no builder;
//! - the option type [`PiecesRenderOptions`] itself, which a registry without
//!   the `pieces` feature does not declare, so that such a plan fails with an
//!   error that identifies the option type.
//!
//! # What the stage does
//!
//! For each selected piece, in slot order, the stage reads the piece's table.
//! A piece holding no content places nothing and reports nothing. Otherwise
//! its value is lowered by [`lower_value`]: brought up to date (the plan's
//! rewrites, then the per-tree stages this stage depends on, over the value
//! alone) and lowered with the backend's selected rules.
//!
//! A rule inside the value reads the document's document-scope tables
//! (anchors, counters, the label index) unchanged, so a reference inside a
//! piece resolves to the document's labels. It looks up an entry by the
//! value's own address, which those tables do not hold, so a reference point
//! the value states itself (an item of a list the builder synthesized) has no
//! number and no anchor, which the standard rules report as
//! `flm.run.missing-entry`. Content of the document that a piece shows (an
//! xnote's content in the list of its xnote kind) is not copied into the
//! value but referred to by a [content
//! pointer](crate::stdcore::backends::lowering::ContentPointer), and lowered
//! where the pointer is placed as the document's own
//! ([`Lowerer::lower_region`](crate::core::lowering::Lowerer::lower_region)),
//! so its reference points find the entries the document's stages computed
//! for them.
//!
//! The lowered tree is recorded with the piece's name and place. A value that
//! could not be brought up to date (one stating a labeled reference point,
//! for example) or lowered is reported as `flm.pieces.lowering-failed` and
//! left out; the run goes on.

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
use crate::stdcore::backends::lowering::stage::BackendLowering;
use crate::stdcore::backends::lowering::{LoweredTree, lower_value, rules_needs};

use super::error::{PieceLoweringFailed, UnknownPiece};
use super::feature::{PiecesRenderOptions, selected_pieces, unknown_pieces};

/// The label of the empty source a condition of this stage is reported at
/// when it concerns no position in any document.
const NOWHERE_LABEL: &str = "pieces";

/// One piece lowered for one backend: its name, where it is placed, and the
/// lowered tree the backend's emitter writes for it.
#[derive(Debug, Clone)]
pub struct LoweredPiece {
    /// The piece's name.
    pub name: Name,
    /// Where the backend places it.
    pub place: PiecePlace,
    /// The lowered tree, with the definitions its rules required and the
    /// rounds lowering took, as for a document tree.
    pub tree: LoweredTree,
}

/// The pieces one backend places, lowered, in slot order: the table
/// `flm.lowered.<backend>.pieces`, which the backend's [`PiecesStage`]
/// writes and the backend's own stages read.
///
/// A piece holding no content, and a piece whose lowering failed, has no
/// entry. The projection lists `{name, place, rounds, requires}` per piece,
/// not the lowered trees themselves.
#[derive(Debug, Clone, Default)]
pub struct LoweredPieces {
    /// The lowered pieces, in slot order.
    pub pieces: Vec<LoweredPiece>,
}

impl LoweredPieces {
    /// The lowered pieces placed at `place`, in slot order.
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

/// The name of `backend`'s lowered-pieces table,
/// `flm.lowered.<backend>.pieces`: both the table's name and the capability
/// that writing the table provides.
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::pieces::lowered_pieces_table_name;
///
/// assert_eq!(lowered_pieces_table_name(&"text".into()).as_str(), "flm.lowered.text.pieces");
/// ```
pub fn lowered_pieces_table_name(backend: &BackendName) -> CapName {
    CapName::from(format!("flm.lowered.{backend}.pieces"))
}

/// The typed handle through which a stage reads or writes `backend`'s
/// lowered pieces.
///
/// The feature that registers the backend must have declared the table under
/// the name [`lowered_pieces_table_name`] returns; a handle alone does not
/// create it.
pub fn lowered_pieces_table(backend: &BackendName) -> TableHandle<LoweredPieces> {
    TableHandle::new(lowered_pieces_table_name(backend))
}

/// The pieces stage of one backend, `flm.<backend>.pieces`: it lowers every
/// piece the render places on that backend and writes the result into the
/// table `flm.lowered.<backend>.pieces` (see the module documentation).
///
/// One value is the stage of one backend. The feature that registers the
/// backend creates it with [`PiecesStage::new`], registers it with
/// [`FeatureBuilder::stage`](crate::core::registry::FeatureBuilder::stage),
/// and declares the table whose name [`lowered_pieces_table_name`] returns.
#[derive(Debug, Clone)]
pub struct PiecesStage {
    backend: BackendName,
    table: TableHandle<LoweredPieces>,
    only_named: bool,
}

impl PiecesStage {
    /// The pieces stage of `backend`, which writes the table
    /// `flm.lowered.<backend>.pieces`.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::pieces::PiecesStage;
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

    /// Builder: the backend places no piece unless the render lists the pieces
    /// it places ([`PiecesRenderOptions::backends`]). This is for a backend
    /// whose final stage places no piece, such as the recomposed LaTeX, whose
    /// pieces only a host's own template places.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::backends::pieces::PiecesStage;
    ///
    /// let stage = PiecesStage::new("latex-recomposed").placing_only_named();
    /// assert!(stage.places_only_named());
    /// assert!(!PiecesStage::new("text").places_only_named());
    /// ```
    pub fn placing_only_named(mut self) -> PiecesStage {
        self.only_named = true;
        self
    }

    /// Whether the backend places no piece unless the render lists the pieces
    /// it places ([`placing_only_named`](Self::placing_only_named)).
    pub fn places_only_named(&self) -> bool {
        self.only_named
    }

    /// The backend this stage lowers pieces for.
    pub fn backend(&self) -> &BackendName {
        &self.backend
    }

    /// The stage's name, `flm.<backend>.pieces`.
    pub fn name(&self) -> String {
        format!("flm.{}.pieces", self.backend)
    }

    /// The typed handle on the lowered-pieces table this stage writes.
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
                "lowers every piece that the render request places for the `{}` backend",
                self.backend
            ))
    }

    /// Declares what the stage reads: what the backend's selected rules need
    /// ([`rules_needs`]), the table of every piece the render places, and
    /// [`PiecesRenderOptions`] (see the module documentation).
    ///
    /// # Panics
    ///
    /// Panics under the precondition of [`rules_needs`]: a plan context whose
    /// rule selection was built against another registry than the one it
    /// holds, which only pairing values of two registries by hand can reach.
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

    /// Lowers every piece the render places and writes the result; a piece
    /// that cannot be lowered is reported and left out, and never causes a
    /// panic. A piece name in the render options that no feature declares is
    /// reported as a Warning (`flm.pieces.unknown-piece`).
    ///
    /// # Panics
    ///
    /// Panics under the precondition of [`needs`](Self::needs); and a rewrite
    /// or a re-entered stage that panics while a piece is brought up to date
    /// propagates, as out of
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
                            String::from(
                                "its content could not be lowered; the errors in that content \
                                 are reported separately",
                            ),
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
