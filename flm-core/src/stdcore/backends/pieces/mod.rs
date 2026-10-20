//! Pieces: document-level synthesized values a backend places beside the
//! document's content, and what every backend shares to render them.
//!
//! A **piece** is a document-level synthesized value that a backend places
//! before or after the document's content (the title, the list of footnotes,
//! the bibliography), built once by its builder stage into a table named for
//! the piece and rendered by each backend with its own rules
//! ([`Piece`](crate::core::piece::Piece); see
//! [pieces](crate::guide::concepts_overview#pieces)). Its content is a small
//! tree of document elements, so each backend lowers it with the lowering
//! rules the offering feature wrote for its elements, and one builder serves
//! every backend.
//!
//! A feature offers a piece by contributing a [`PieceDecl`] (the piece's
//! name, its table, its place and whether a render places it by default) to
//! the service slot [`SLOT`] (`flm.pieces`). The render option
//! [`PiecesRenderOptions`] chooses which declared pieces a render places, and
//! [`selected_pieces`] applies it. Each backend that places pieces registers
//! a [`PiecesStage`] for itself, which lowers the selected pieces for that
//! backend into its table `flm.lowered.<backend>.pieces` ([`LoweredPieces`]).
//!
//! # Module map
//!
//! - [`mod@feature`] — the `pieces` feature: the service slot [`SLOT`], the
//!   render option [`PiecesRenderOptions`], and [`selected_pieces`], the
//!   only function that decides which pieces a render places.
//! - [`stage`] — [`PiecesStage`], the per-backend stage `flm.<backend>.pieces`
//!   that lowers the selected pieces, and the table it writes,
//!   [`LoweredPieces`] (`flm.lowered.<backend>.pieces`).
//! - [`error`] — the conditions `flm.pieces.unknown-piece` and
//!   `flm.pieces.lowering-failed`.
//!
//! # What a backend does with its pieces
//!
//! The `pieces` feature registers no stage and no table: a backend that
//! places pieces registers [`PiecesStage::new`] for itself and declares its
//! lowered-pieces table, and its own stages write the result into its
//! output. The HTML backend and the recomposed LaTeX write one output region
//! (a named part of the output's entry artifact;
//! [`Regions`](crate::core::region::Regions)) per piece; the text backend
//! writes the pieces inside the single text it prints. The recomposed LaTeX's
//! pieces stage places only the pieces a render selects for it
//! ([`PiecesStage::placing_only_named`]), because its final stage places no
//! piece. See also [the rendering guide](crate::guide::rendering#pieces).
//!
//! [`PieceDecl`]: crate::core::piece::PieceDecl

pub mod error;
pub mod feature;
pub mod stage;

pub use error::{PieceLoweringFailed, UnknownPiece};
pub use feature::{
    FEATURE, PiecesFeature, PiecesRenderOptions, SLOT, declared_pieces, feature, selected_pieces,
    unknown_pieces,
};
pub use stage::{
    LoweredPiece, LoweredPieces, PiecesStage, lowered_pieces_table, lowered_pieces_table_name,
};
