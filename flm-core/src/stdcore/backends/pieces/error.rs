//! The conditions a backend's pieces stage reports, `flm.pieces.*`.
//!
//! A **condition** is the typed body of a diagnostic: a value that says what
//! happened and holds the facts describing it, identified as
//! `flm.<feature>.<condition>` (see
//! [diagnostics](crate::guide::concepts_overview#diagnostics)). Neither
//! condition stops a run: an unknown piece in the render options is ignored,
//! and a piece that cannot be rendered is left out of the output while the
//! rest of the document is written.

use techy::error::DiagnosticInfo;

/// Condition `flm.pieces.unknown-piece`: the render option
/// [`PiecesRenderOptions`](super::PiecesRenderOptions) lists, in its
/// `include` or `exclude` set, a piece that no feature of the registry
/// declares. Each backend's pieces stage reports it as a Warning, since the
/// output does not change.
///
/// Recovery: the name is ignored.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.pieces.unknown-piece",
    message = "the render options of the `pieces` feature list the piece `{piece}` under \
               `include` or `exclude`, but no registered feature declares a piece of that name"
)]
pub struct UnknownPiece {
    /// The piece name the option gave.
    pub piece: String,
}

/// Condition `flm.pieces.lowering-failed`: a piece's content could not be
/// brought up to date or could not be lowered for a backend. Causes include a
/// synthesized value that states a fact about the document as a whole (a
/// labeled reference point, for example), content nested too deeply, and a
/// lowering that failed. That backend's pieces stage reports it as an Error.
///
/// Recovery: the piece is left out of that backend's output; the run
/// continues and the document's content is written.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.pieces.lowering-failed",
    message = "the piece `{piece}` could not be rendered for the backend `{backend}`: {detail}"
)]
pub struct PieceLoweringFailed {
    /// The name of the piece.
    pub piece: String,
    /// The name of the backend.
    pub backend: String,
    /// What failed, as text.
    pub detail: String,
}
