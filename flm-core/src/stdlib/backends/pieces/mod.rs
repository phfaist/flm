//! This module provides what the backends share to place *pieces* in their
//! output, along with the `pieces` feature.
//!
//! A *piece* is a part of the output that a backend places before or after the
//! document's content, such as the document's title, the list of footnotes or
//! the bibliography ([`Piece`](crate::core::piece::Piece); see
//! [pieces](crate::guide::concepts_overview#pieces)). The author does not
//! write a piece in the document. Instead, a stage of the feature that offers
//! the piece, called the piece's *builder*, builds the content of the piece
//! once for the whole document. The builder writes this content into a table
//! named after the piece.
//!
//! The content of a piece consists of document elements. Each backend lowers
//! it with the lowering rules defined for these elements, as it lowers the
//! document's content. One builder therefore serves every backend.
//!
//! The items of this module are used in the following steps:
//!
//! 1. Each piece that a feature offers is declared with a [`PieceDecl`]. The
//!    feature contributes this declaration to the service slot [`SLOT`]
//!    (`flm.pieces`).
//! 2. The pieces placed during a render are selected with the render option
//!    [`PiecesRenderOptions`]. The function [`selected_pieces`] applies this
//!    selection for one backend.
//! 3. Each backend that places pieces registers its own [`PiecesStage`]. This
//!    stage lowers the selected pieces for the backend, and writes them into
//!    the table `flm.lowered.<backend>.pieces` ([`LoweredPieces`]).
//! 4. The backend's other stages write the lowered pieces into the output. The
//!    HTML backend and the recomposed LaTeX backend write each piece as an
//!    output region, a named part of the output
//!    ([`Regions`](crate::core::region::Regions)). The text backend prints the
//!    pieces before and after the document's text.
//!
//! The recomposed LaTeX backend places no piece by default. Its pieces stage
//! places only the pieces listed for this backend in the render option
//! ([`PiecesStage::placing_only_named`]).
//!
//! The submodule [`mod@feature`] defines the `pieces` feature and the
//! selection of pieces, the submodule [`stage`] defines the pieces stage and
//! the table it writes, and the submodule [`error`] defines the conditions
//! that the pieces stage reports. See also [the rendering
//! guide](crate::guide::rendering#pieces).
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
