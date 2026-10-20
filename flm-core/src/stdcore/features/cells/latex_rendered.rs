//! The definition piece of the [`cells`](super) feature on the
//! `latex-rendered` backend, `flm.latex.def.flmx.cells.styles` ([`piece`]).
//!
//! The piece is the LaTeX text of the fragment `sty/cells.sty`, included at
//! compile time. It registers, with the core chunk's `\flmxDefineCellStyle`,
//! the look of the four custom styles the feature ships, `green`, `blue`,
//! `yellow` and `red`: a light background, a color of its own
//! (`flmxCellsGreen` and the three others). The structural styles are written
//! as the cell's keys `align`, `header` and `rules`, and need no registration.
//! Its comment block says how a template registers another look.
//!
//! The piece is named apart from the backend's cells chunk
//! (`flm.latex.def.flmx.cells`), which defines the table itself.
//!
//! It delivers the feature's style identifier [`STYLE`](super::STYLE), which
//! each of the four custom styles requires on this backend
//! ([`CellStyle::requirements_on`](super::CellStyle::requirements_on)): a
//! `latex-rendered` document with a cell of one of these styles holds the
//! piece in its style file, and one without, does not.
//!
//! The feature contributes the piece to the LaTeX family's preamble slot
//! ([`preamble::SLOT`]) with a peer declaration, so that a registry
//! without the LaTeX backends still builds.

use std::sync::Arc;

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::stdcore::backends::latex::{LatexPreambleDef, preamble};
use crate::stdcore::backends::latex_rendered::contract::{CELLS_CHUNK, CORE_CHUNK};

/// The piece's identifier, `flm.latex.def.flmx.cells.styles`.
pub const PIECE: &str = "flm.latex.def.flmx.cells.styles";

/// The feature's definition piece on the `latex-rendered` backend, [`PIECE`]:
/// the text of `sty/cells.sty`, requiring the core chunk, whose
/// `\flmxDefineCellStyle` it calls, and the cells chunk, whose cell styles it
/// registers. See the [module documentation](self).
pub fn piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "flmx.cells.styles",
        include_str!("sty/cells.sty"),
        "the cell styles the cells feature ships",
    )
    .requires([CORE_CHUNK, CELLS_CHUNK])
    .delivering([super::STYLE])
}

/// Contributes [`piece`] to the preamble slot, under a peer declaration.
///
/// # Errors
///
/// The [`BuildError`] of the registry builder when a declaration is made
/// outside the registration round that accepts it.
pub(super) fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_slot::<LatexPreambleDef>(preamble::SLOT, Arc::new(piece()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::names::Name;

    #[test]
    fn the_piece_names_itself_and_requires_the_cells_chunk() {
        let piece = piece();
        assert_eq!(piece.id.as_str(), PIECE);
        assert!(piece.requires.iter().any(|id| id.as_str() == CELLS_CHUNK));
        assert_eq!(piece.delivers, [Name::from(super::super::STYLE)]);
        assert!(piece.content.starts_with(&format!("% {PIECE} -- ")), "{}", piece.content);
        for name in ["green", "blue", "yellow", "red"] {
            let line = format!("\\flmxDefineCellStyle{{{name}}}{{bg=");
            assert!(piece.content.contains(&line), "{name}");
        }
    }
}
