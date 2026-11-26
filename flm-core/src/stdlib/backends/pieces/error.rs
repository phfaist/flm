//! This module defines the diagnostic conditions that the pieces stage of a
//! backend ([`PiecesStage`](super::PiecesStage)) reports.

use techy::error::DiagnosticInfo;

/// The warning that is reported when a piece listed in the setting `include` or
/// `exclude` of the `pieces` feature is not defined in any feature (id
/// `flm.pieces.unknown-piece`).
///
/// The settings `include` and `exclude` are the fields of
/// [`PiecesRenderOptions`](super::PiecesRenderOptions). They accept the names
/// of the pieces declared in the registry
/// ([`declared_pieces`](super::declared_pieces)), as in
/// `pieces.include=[flm.toc]`.
///
/// Reported by the pieces stage of each backend when it runs. The unknown name
/// is ignored, and the output is the same as without it.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.pieces.unknown-piece",
    message = "Unknown piece ‘{piece}’ in setting ‘pieces.include’ or \
               ‘pieces.exclude’"
)]
pub struct UnknownPiece {
    /// The piece name, as written in the setting.
    pub piece: String,
}

/// The error that is reported when a piece cannot be rendered for a backend
/// (id `flm.pieces.lowering-failed`).
///
/// The pieces stage of a backend renders a piece in two steps. First, it
/// brings the content of the piece up to date ([value
/// catch-up](crate::guide::concepts_overview#value-catch-up)). Then, it lowers
/// the content with the lowering rules selected for the backend. Either step
/// can fail. For instance, no label may be defined in the content of a piece.
/// If a label is defined there, the first step fails.
///
/// Reported by the pieces stage of the backend when it runs, once for each
/// piece that fails. The piece is left out of the output of this backend, and
/// the rest of the document is written. The diagnostics that describe the
/// cause of the failure, if any, are reported as well.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.pieces.lowering-failed",
    message = "Failed to render piece ‘{piece}’ for backend \
               ‘{backend}’: {detail}"
)]
pub struct PieceLoweringFailed {
    /// The name of the piece.
    pub piece: String,
    /// The name of the backend.
    pub backend: String,
    /// A description of the cause of the failure.
    pub detail: String,
}
