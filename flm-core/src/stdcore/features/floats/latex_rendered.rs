//! The definition piece of the [`floats`](super) feature on the
//! `latex-rendered` backend, `flm.latex.def.flmx.floats` ([`piece`]).
//!
//! The piece is the LaTeX text of the fragment `sty/floats.sty`, included at
//! compile time. It registers, with the backend's definition commands, the
//! default look of a float by its type's kind token: `figure` and `table` as
//! LaTeX's `figure` or `table` environment placed `[htbp]`, centered; the
//! content block of role `flm.floats.float.content` as it stands; the
//! caption block of role `flm.floats.float.caption` as a paragraph in
//! `\small` after `\abovecaptionskip`; the number span of role
//! `flm.floats.float.number` in `\bfseries`. A float of a type the document
//! configures is written with the block's default. Its comment block says
//! what each role and kind token is and how a template registers another
//! look.
//!
//! It delivers the feature's style identifier [`STYLE`](super::html::STYLE),
//! which the feature's general rule requires at every float: a
//! `latex-rendered` document holding a float holds the piece in its style
//! file, and one that does not, does not.
//!
//! The feature contributes the piece to the LaTeX family's preamble slot
//! ([`preamble::SLOT`]) with a peer declaration, so that a registry
//! without the LaTeX backends still builds.

use std::sync::Arc;

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::stdcore::backends::latex::{LatexPreambleDef, preamble};
use crate::stdcore::backends::latex_rendered::contract::CORE_CHUNK;

/// The piece's identifier, `flm.latex.def.flmx.floats`.
pub const PIECE: &str = "flm.latex.def.flmx.floats";

/// The feature's definition piece on the `latex-rendered` backend, [`PIECE`]:
/// the text of `sty/floats.sty`, requiring the core chunk. See the [module
/// documentation](self).
pub fn piece() -> LatexPreambleDef {
    LatexPreambleDef::new(
        "flmx.floats",
        include_str!("sty/floats.sty"),
        "figures, tables and the other float types",
    )
    .requires([CORE_CHUNK])
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
