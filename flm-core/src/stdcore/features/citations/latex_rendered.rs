//! The definition piece of the [`citations`](super) feature on the
//! `latex-rendered` backend, `flm.latex.def.flmx.citations` ([`piece`]).
//!
//! The piece is the LaTeX text of the fragment `sty/citations.sty`, included
//! at compile time. It registers, with the backend's definition commands,
//! the default look of the span roles `flm.citations.space`,
//! `flm.citations.word` and `flm.citations.entry.number` (as they stand),
//! of the list of cited works, the block role `flm.citations.bibliography`
//! (a block of paragraphs: its heading and its list keep their own looks),
//! and of the link kind `cite` (the links chunk's default link, in the color
//! `flmxCiteColor`). The citation mark's span, `flm.citations.mark`, is left
//! to the span's default, so that the superscript form keeps its style. Its
//! comment block says what each role is and how a template registers
//! another look.
//!
//! It delivers the feature's style identifier [`STYLE`](super::html::STYLE),
//! which the feature's general rules require at every citation and at the
//! list of cited works: a `latex-rendered` document holding a citation holds
//! the piece in its style file, and one that does not, does not.
//!
//! The feature contributes the piece to the LaTeX family's preamble slot
//! ([`preamble::SLOT`]) with a peer declaration, so that a registry
//! without the LaTeX backends still builds.

use std::sync::Arc;

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::stdcore::backends::latex::{LatexPreambleDef, preamble};
use crate::stdcore::backends::latex_rendered::contract::{CORE_CHUNK, LINKS_CHUNK};

/// The piece's identifier, `flm.latex.def.flmx.citations`.
pub const PIECE: &str = "flm.latex.def.flmx.citations";

/// The feature's definition piece on the `latex-rendered` backend, [`PIECE`]:
/// the text of `sty/citations.sty`, requiring the links chunk, whose default
/// link the kind `cite` uses. See the [module documentation](self).
pub fn piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "flmx.citations",
        include_str!("sty/citations.sty"),
        "citations and the list of cited works",
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
    fn the_piece_names_itself_and_requires_the_links_chunk() {
        let piece = piece();
        assert_eq!(piece.id.as_str(), PIECE);
        assert!(piece.requires.iter().any(|id| id.as_str() == LINKS_CHUNK));
        assert_eq!(piece.delivers, [Name::from(super::super::html::STYLE)]);
        assert!(piece.content.starts_with(&format!("% {PIECE} -- ")), "{}", piece.content);
    }
}
