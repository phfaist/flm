//! The conditions that the [`defterm`](super) feature reports, each under
//! the identifier `flm.defterm.<condition>` and each an Error. A condition is
//! the typed body of a diagnostic (see
//! [diagnostics](crate::guide::concepts_overview#diagnostics)); each
//! condition's documentation states the recovery the feature applies (see
//! [recoveries](crate::stdcore::backends::lowering::error)). The lowering
//! rules report them where a term definition or a term link is written on an
//! output.
//!
//! An unresolved term link is not among them: the references feature reports
//! it as `flm.refs.unresolved`, as for any reference.

use techy::error::DiagnosticInfo;

/// Condition `flm.defterm.empty-term`, an Error: a term definition or a term
/// link whose term is empty or only whitespace (`\begin{defterm}{}`,
/// `\term{ }`), so that it refers to no term. `~` and `\ ` are part of a term
/// as written ([the identity of a
/// term](crate::stdcore::features::defterm::elements#the-identity-of-a-term)):
/// `\term{~}` is not empty. Reported at the node, by the lowering rules of
/// every backend. Recovery: a term definition is written with an empty term
/// in its lead-in and has no label computed from its term, so no term link
/// finds it (its `\label`s and aliases still label it); a term link shows its
/// text marked as an unresolved reference (on the recomposed LaTeX,
/// `\flmTerm` with an empty label).
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.defterm.empty-term", message = "the term of this {what} must not be empty or blank")]
pub struct EmptyTerm {
    /// What has the empty term: `term definition` or `term link`.
    pub what: String,
}

/// Condition `flm.defterm.pointer-refused`, an Error: a term pointer
/// ([`TERM_POINTER`](super::TERM_POINTER)) refers to something other than
/// the term argument of a term definition. No document can cause it: it is
/// the mistake of a rule that built a lead-in, which would otherwise show
/// content that is not a term definition's term. Reported at the pointer.
/// Recovery: the pointer writes nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.defterm.pointer-refused",
    message = "a term pointer in a term definition's lead-in must point at the term argument \
               of a `defterm` environment, but it points at the {region} of a node of \
               `{element}`"
)]
pub struct PointerRefused {
    /// The element of the node the pointer refers to.
    pub element: String,
    /// The region of that node the pointer refers to, as text (for example
    /// `argument 0`).
    pub region: String,
}
