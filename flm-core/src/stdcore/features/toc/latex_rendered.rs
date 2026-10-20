//! The definition piece of the [`toc`](super) feature on the
//! `latex-rendered` backend, `flm.latex.def.flmx.toc` ([`piece`]).
//!
//! The piece is the LaTeX text of the fragment `sty/toc.sty`, included at
//! compile time. It registers, with the backend's definition commands
//! (`\flmxDefineBlock`, `\flmxDefineSpan`, ...), the default look of the block
//! role `flm.toc` as the title `\contentsname`, an unnumbered heading of level
//! 0, followed by the lists; the list kind `toc` without markers, indented by
//! the list's depth; the link kind `toc` as the title linked to the heading,
//! leaders and the heading's page number (`\pageref*`); the span role
//! `flm.toc.entry.number` as the number followed by `\enspace`. Its comment
//! block says what each role and kind token is and how a template registers
//! another look.
//!
//! It delivers the feature's style identifier [`STYLE`](super::STYLE), which
//! the feature's general rules require: a `latex-rendered` document whose
//! content requires it holds the piece in its style file, and one that does
//! not, does not.
//!
//! The feature contributes the piece to the LaTeX family's preamble slot
//! ([`preamble::SLOT`]) with a peer declaration, so that a registry
//! without the LaTeX backends still builds.

use std::sync::Arc;

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::stdcore::backends::latex::{LatexPreambleDef, preamble};
use crate::stdcore::backends::latex_rendered::contract::{
    CORE_CHUNK, HEADINGS_CHUNK, LINKS_CHUNK, LISTS_CHUNK,
};

/// The piece's identifier, `flm.latex.def.flmx.toc`.
pub const PIECE: &str = "flm.latex.def.flmx.toc";

/// The feature's definition piece on the `latex-rendered` backend, [`PIECE`]:
/// the text of `sty/toc.sty`, requiring the backend's chunks it calls into.
/// See the [module documentation](self).
pub fn piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "flmx.toc",
        include_str!("sty/toc.sty"),
        "the table of contents: its title, its entries with leaders and page numbers",
    )
    .requires([CORE_CHUNK, HEADINGS_CHUNK, LISTS_CHUNK, LINKS_CHUNK])
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
    fn the_piece_names_itself_and_requires_the_core_chunk() {
        let piece = piece();
        assert_eq!(piece.id.as_str(), PIECE);
        assert!(piece.requires.iter().any(|id| id.as_str() == CORE_CHUNK));
        assert_eq!(piece.delivers, [Name::from(super::super::STYLE)]);
        assert!(piece.content.starts_with(&format!("% {PIECE} -- ")), "{}", piece.content);
    }
}
