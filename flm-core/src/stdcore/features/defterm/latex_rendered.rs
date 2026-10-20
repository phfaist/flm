//! The definition piece of the [`defterm`](super) feature on the
//! `latex-rendered` backend, `flm.latex.def.flmx.defterm` ([`piece`]).
//!
//! The piece is the LaTeX text of the fragment `sty/defterm.sty`, included at
//! compile time. It registers, with the backend's definition commands
//! (`\flmxDefineBlock`, `\flmxDefineSpan`, ...), the default look of the block
//! role `flm.defterm` as a block of paragraphs, the span role
//! `flm.defterm.leadin` of its lead-in as it stands, the span role
//! `flm.defterm.term` in `\bfseries`, and the link kind `term` as the links
//! chunk's default link. Its comment block says what each role and kind token
//! is and how a template registers another look.
//!
//! It delivers the feature's style identifier [`STYLE`](super::html::STYLE),
//! which the feature's general rules require: a `latex-rendered` document whose
//! content requires it holds the piece in its style file, and one that does
//! not, does not.
//!
//! The feature contributes the piece to the LaTeX family's preamble slot
//! ([`preamble::SLOT`]) with a peer declaration, so that a registry
//! without the LaTeX backends still builds.

use std::sync::Arc;

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::stdcore::backends::latex::{LatexPreambleDef, preamble};
use crate::stdcore::backends::latex_rendered::contract::{CORE_CHUNK, LINKS_CHUNK};

/// The piece's identifier, `flm.latex.def.flmx.defterm`.
pub const PIECE: &str = "flm.latex.def.flmx.defterm";

/// The feature's definition piece on the `latex-rendered` backend, [`PIECE`]:
/// the text of `sty/defterm.sty`, requiring the backend's chunks it calls into.
/// See the [module documentation](self).
pub fn piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "flmx.defterm",
        include_str!("sty/defterm.sty"),
        "term definitions, their lead-ins and the terms they define",
    )
    .requires([CORE_CHUNK, LINKS_CHUNK])
    .delivering([super::html::STYLE])
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
    fn the_piece_names_itself_and_requires_the_core_chunk() {
        let piece = piece();
        assert_eq!(piece.id.as_str(), PIECE);
        assert!(piece.requires.iter().any(|id| id.as_str() == CORE_CHUNK));
        assert_eq!(piece.delivers, [Name::from(super::super::html::STYLE)]);
        assert!(piece.content.starts_with(&format!("% {PIECE} -- ")), "{}", piece.content);
    }
}
