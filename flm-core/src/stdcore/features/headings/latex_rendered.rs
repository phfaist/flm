//! The definition piece of the [`headings`](super) feature on the
//! `latex-rendered` backend, `flm.latex.def.flmx.headings.number` ([`piece`]).
//!
//! The piece is the LaTeX text of the fragment `sty/headings.sty`, included at
//! compile time. It registers, with the backend's definition commands
//! (`\flmxDefineBlock`, `\flmxDefineSpan`, ...), the default look of the span
//! role `flm.heading.number`, a heading's number inside its title, written as
//! it stands. Its comment block says what each role and kind token is and how a
//! template registers another look.
//!
//! It delivers the feature's style identifier [`STYLE`](super::STYLE), which
//! the heading rule requires at every heading it stages: a `latex-rendered`
//! document holding a heading of the headings feature holds the piece in its
//! style file, and a heading another feature writes needs nothing of it. Its
//! identifier is not `flm.latex.def.flmx.headings`, which is the backend's
//! headings chunk's.
//!
//! The feature contributes the piece to the LaTeX family's preamble slot
//! ([`preamble::SLOT`]) with a peer declaration, so that a registry
//! without the LaTeX backends still builds.

use std::sync::Arc;

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::stdcore::backends::latex::{LatexPreambleDef, preamble};
use crate::stdcore::backends::latex_rendered::contract::CORE_CHUNK;

/// The piece's identifier, `flm.latex.def.flmx.headings.number`.
pub const PIECE: &str = "flm.latex.def.flmx.headings.number";

/// The feature's definition piece on the `latex-rendered` backend, [`PIECE`]:
/// the text of `sty/headings.sty`, requiring the backend's chunks it calls
/// into. See the [module documentation](self).
pub fn piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "flmx.headings.number",
        include_str!("sty/headings.sty"),
        "the number of a heading, as it stands",
    )
    .requires([CORE_CHUNK])
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

    #[test]
    fn the_piece_names_itself_and_requires_the_core_chunk() {
        let piece = piece();
        assert_eq!(piece.id.as_str(), PIECE);
        assert!(piece.requires.iter().any(|id| id.as_str() == CORE_CHUNK));
        assert_eq!(piece.delivers, [crate::model::names::Name::from(super::super::STYLE)]);
        assert!(piece.content.starts_with(&format!("% {PIECE} -- ")), "{}", piece.content);
    }
}
