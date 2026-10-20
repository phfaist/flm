//! The definition piece of the [`meta`](super) feature on the
//! `latex-rendered` backend, `flm.latex.def.flmx.meta` ([`piece`]).
//!
//! The piece is the LaTeX text of the fragment `sty/meta.sty`, included at
//! compile time. It registers, with the backend's definition commands
//! (`\flmxDefineBlock`, `\flmxDefineSpan`, ...), the default look of the block
//! roles `flm.meta.title`, `flm.meta.author` and `flm.meta.date` (kind token
//! `meta`) as LaTeX's `\maketitle` sets the title block, centered, with its
//! spacing, the title in `\LARGE\bfseries`, the authors in `\large`, the date
//! in `\normalsize`; the span role `flm.meta.author.name` as it stands. Its
//! comment block says what each role and kind token is and how a template
//! registers another look.
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
use crate::stdcore::backends::latex_rendered::contract::CORE_CHUNK;

/// The piece's identifier, `flm.latex.def.flmx.meta`.
pub const PIECE: &str = "flm.latex.def.flmx.meta";

/// The feature's definition piece on the `latex-rendered` backend, [`PIECE`]:
/// the text of `sty/meta.sty`, requiring the backend's chunks it calls into.
/// See the [module documentation](self).
pub fn piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "flmx.meta",
        include_str!("sty/meta.sty"),
        "the document's title, authors and date, as a title block",
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
