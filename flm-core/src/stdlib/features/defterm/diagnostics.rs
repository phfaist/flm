//! This module defines the diagnostic conditions that the [`defterm`](super)
//! feature reports (see
//! [diagnostics](crate::guide::concepts_overview#diagnostics)). Both
//! conditions are errors.
//!
//! The lowering rules of a term definition (the `defterm` environment) and of
//! a term link (the `\term` command) report [`EmptyTerm`] when the term of the
//! environment or of the command is empty.
//!
//! A term pointer is a synthesized node in the lead-in of a term definition.
//! The lowering rule of a term pointer reports [`PointerRefused`] when the
//! term pointer does not refer to the term of a term definition. A document
//! cannot cause this error.
//!
//! Both conditions are reported only when a document is lowered. The `check`
//! workflow ([`CHECK_WORKFLOW`](crate::stdlib::CHECK_WORKFLOW)) lowers
//! nothing, and therefore does not report them.
//!
//! A term link is a reference, and a term definition is a reference point.
//! The [references](crate::stdlib::features::refs) feature therefore reports
//! their other problems. For instance, a term link that matches no term
//! definition is reported as `flm.refs.unresolved`, and two term definitions
//! of the same term are reported as `flm.refs.duplicate-label`.

use techy::error::DiagnosticInfo;

/// The error that is reported when the term of a `defterm` environment or of
/// a `\term` command is empty (`flm.defterm.empty-term`).
///
/// The term of a `defterm` environment is its argument, as in
/// `\begin{defterm}{Markov chain}`. The term of a `\term` command is its
/// optional argument, as in `\term[Markov chain]{such a process}`, or its
/// mandatory argument when the optional argument is absent or empty, as in
/// `\term{Markov chain}`. A term counts as empty when it contains nothing but
/// whitespace and comments. The space commands `~` and `\ ` count as content.
///
/// Reported on every backend by the lowering rule of the environment or of
/// the command, when the rule is invoked. A `defterm` environment with an
/// empty term is otherwise written as usual. A `\term` command with an empty
/// term expands to the text of its mandatory argument, marked as an
/// unresolved reference. On the `latex-recomposed` backend, the `\term`
/// command expands to `\flmTerm` with an empty label.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.defterm.empty-term", message = "Missing term in ‘{what}’")]
pub struct EmptyTerm {
    /// The name of the environment or of the command whose term is empty:
    /// `defterm` or `\term`.
    pub what: String,
}

/// Error representing a term pointer ([`TERM_POINTER`](super::TERM_POINTER))
/// that does not refer to the term of a term definition (id
/// `flm.defterm.pointer-refused`).
///
/// A term definition opens with a *lead-in*, such as `Markov chain: `. The
/// lead-in is written from the lead-in template of the feature
/// ([`DeftermConfig`](super::DeftermConfig)), in which `#{term}` stands for
/// the term being defined. At each term definition, `#{term}` is replaced by
/// a term pointer: a synthesized node that refers to the term argument of the
/// `defterm` environment. The lowering rule of the term pointer lowers the
/// term argument at the place of the pointer. A term pointer must refer to
/// the term argument of a `defterm` environment. This requirement ensures
/// that no content is written twice: any other content is already written
/// where its node is placed.
///
/// A document cannot cause this error: it is caused by the faulty code that
/// created the term pointer. Reported by the lowering rule of the term pointer
/// ([`PointerRule`](super::PointerRule)) when it is invoked. The term pointer
/// expands to nothing. The rest of the lead-in and the term definition are
/// written as usual.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.defterm.pointer-refused",
    message = "Invalid term pointer to {region} of a node of ‘{element}’: the \
               pointer must refer to the term argument of a ‘defterm’ environment"
)]
pub struct PointerRefused {
    /// The name of the element of the node that the term pointer refers to.
    pub element: String,
    /// The node region that the term pointer refers to: an argument or a
    /// content slot of the node
    /// ([`NodeRegion`](crate::core::lowering::NodeRegion)), as text, e.g.
    /// `argument 0`.
    pub region: String,
}
