//! The condition the `annotations` feature reports, identified as
//! `flm.annotations.<condition>`, with the recovery its documentation states
//! ([recovery](crate::stdcore::backends::lowering::error)). A condition is the
//! typed body of a diagnostic
//! ([diagnostics](crate::guide::concepts_overview#diagnostics)).
//!
//! The parser of an annotator's argument
//! ([`AnnotationParser`](super::AnnotationParser)) reports [`Unterminated`]
//! while the document is parsed. Every other error in an annotation is
//! reported by the language itself: an unknown command (a stray `\endphf`, or
//! `\phf` with no annotator configured) and an unclosed group.

use techy::error::DiagnosticInfo;

/// Condition `flm.annotations.unterminated`, an Error: a passage opened by an
/// annotator's command in its third form, `\phf …`, is not closed by the
/// closing command `\endphf` before the input ends, or before the group or
/// environment enclosing the annotation ends. Reported over the passage read.
/// Recovery: the passage is what was read up to there, highlighted as if it
/// had been closed.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.annotations.unterminated",
    message = "\\{command} must be closed by \\{end_command} within the same group or environment"
)]
pub struct Unterminated {
    /// The name of the annotator's command, `phf`.
    pub command: String,
    /// The name of the closing command that was expected, `endphf`.
    pub end_command: String,
}
