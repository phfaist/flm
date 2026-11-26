//! This module defines the diagnostic conditions that the [`theorems`](super)
//! feature reports (see [diagnostics](crate::guide::concepts_overview#diagnostics)).
//!
//! The conditions [`MissingProof`] and [`BadRelationRef`] concern the theorems
//! written in a document. The check stage [`CHECK_PROOFS`](super::CHECK_PROOFS)
//! reports the warning [`MissingProof`] for a theorem that has no proof
//! although its kind expects one, and the error [`BadRelationRef`] for a title
//! of a proof that begins with a star but is not a valid back-reference.
//!
//! The two other conditions indicate an error in the code of a feature, and a
//! document cannot cause them. The lowering rules of the nodes of a theorem
//! heading report [`PointerRefused`] for a node that does not refer to the
//! title of a theorem. The check stage reports [`StageFailed`] when it fails to
//! read a table.
//!
//! The references feature reports two other conditions that concern theorems,
//! as it does for any label and reference: a back-reference to a label that is
//! not defined is reported as `flm.refs.unresolved`, and a `\label` on a proof
//! is reported as `flm.refs.label-type-not-accepted`.

use techy::error::DiagnosticInfo;

/// The warning that is reported when a theorem has no proof although its kind
/// expects one (id `flm.theorems.missing-proof`).
///
/// A theorem kind expects a proof when its
/// [`expects_proof`](super::TheoremKind::expects_proof) setting is `true`. In
/// the default configuration, these kinds are `theorem`, `proposition`,
/// `lemma` and `corollary`. A theorem has a proof when the title of a proof
/// is a back-reference to a label of the theorem, as in
/// `\begin{proof}[*thm:main]` or `\begin{proof}[**thm:main]`. The warning is
/// not reported for a theorem that carries the field `\noproofref`, as in
/// `\begin{theorem}\label{thm:main}\noproofref`.
///
/// Reported by the check stage [`CHECK_PROOFS`](super::CHECK_PROOFS) at the
/// theorem, under every workflow (see [`check`](super::check)). The output of
/// the document is not affected. The warning is not reported when the registry
/// does not contain the references feature, which resolves back-references.
///
/// The message suggests three fixes: writing a proof whose title refers to the
/// theorem, adding `\noproofref` to the theorem, or setting `expects_proof` to
/// `false` for the kind of the theorem.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.theorems.missing-proof")]
pub struct MissingProof {
    /// The kind of the theorem, which is also the name of its environment,
    /// e.g. `lemma`.
    pub kind: String,
    /// The first label of the theorem whose type the kind accepts, written as
    /// `type:label`. The labels are taken in the order in which they are
    /// written. `None` when the theorem has no such label. A label of another
    /// type does not define anything, so a proof cannot refer to the theorem
    /// through it.
    pub label: Option<String>,
    /// The label type that the message suggests when the theorem has no label
    /// (`label` is `None`): the first label type that the kind accepts, which
    /// is the first entry of
    /// [`TheoremsConfig::label_types`](super::TheoremsConfig::label_types),
    /// e.g. `thm`.
    pub label_type: String,
    /// The theorem kind that the message suggests for writing the proof, e.g.
    /// `proof`. It is the first kind, in the alphabetical order of the kind
    /// names, whose class reads back-references
    /// ([`relation_ref`](super::TheoremClass::relation_ref)). `None` when no
    /// class reads back-references; the message then does not suggest writing
    /// a proof.
    pub proof_kind: Option<String>,
}

/// Writes the message of the warning. The first suggested fix depends on
/// whether the theorem has a label and whether a kind for proofs exists, so
/// the message has four forms.
impl std::fmt::Display for MissingProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = &self.kind;
        let ty = &self.label_type;
        match (&self.label, &self.proof_kind) {
            (Some(label), Some(proof)) => write!(
                f,
                "Missing proof for {kind} ‘{label}’: write its proof as \
                 ‘\\begin{{{proof}}}[*{label}]’; or add ‘\\noproofref’ to the {kind}; or set \
                 ‘expects_proof’ to false for theorem kind ‘{kind}’ in the configuration of \
                 feature ‘theorems’"
            ),
            (None, Some(proof)) => write!(
                f,
                "Missing proof for this {kind}: label the {kind} with ‘\\label{{{ty}:…}}’ and \
                 write its proof as ‘\\begin{{{proof}}}[*{ty}:…]’; or add ‘\\noproofref’ to the \
                 {kind}; or set ‘expects_proof’ to false for theorem kind ‘{kind}’ in the \
                 configuration of feature ‘theorems’"
            ),
            (Some(label), None) => write!(
                f,
                "Missing proof for {kind} ‘{label}’: add ‘\\noproofref’ to the {kind}; or set \
                 ‘expects_proof’ to false for theorem kind ‘{kind}’ in the configuration of \
                 feature ‘theorems’"
            ),
            (None, None) => write!(
                f,
                "Missing proof for this {kind}: add ‘\\noproofref’ to the {kind}; or set \
                 ‘expects_proof’ to false for theorem kind ‘{kind}’ in the configuration of \
                 feature ‘theorems’"
            ),
        }
    }
}

/// The error that is reported when the title of a proof begins with a star but
/// is not a valid back-reference (id `flm.theorems.bad-relation-ref`).
///
/// A back-reference consists of one or two stars followed by a label, as in
/// `\begin{proof}[*lem:x]` or `\begin{proof}[**lem:x]`. Back-references are
/// read in the titles of the theorem kinds whose class has
/// [`relation_ref`](super::TheoremClass::relation_ref) set. In the default
/// configuration, `proof` is the only such kind.
///
/// Reported by the check stage [`CHECK_PROOFS`](super::CHECK_PROOFS) at the
/// title, under every workflow. The title is written to the output as it is
/// typed, and the proof does not refer to any theorem.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.theorems.bad-relation-ref",
    message = "Invalid back-reference in the title of ‘{environment}’: back-references \
               must have the form ‘*type:label’ or ‘**type:label’"
)]
pub struct BadRelationRef {
    /// The name of the environment, e.g. `proof`.
    pub environment: String,
}

/// Error representing a node of a theorem heading whose content pointer does
/// not refer to the title of a theorem (id `flm.theorems.pointer-refused`).
///
/// The lowering rule of a theorem writes the heading of the theorem from a
/// template (see [`heading`](super::heading)). In this heading, the title is
/// represented by a synthesized node of the element
/// [`TITLE_POINTER`](super::TITLE_POINTER), or of the element
/// [`RELATION`](super::RELATION) for a back-reference that a proof shows in
/// its heading. This node contains a content pointer
/// ([`ContentPointer`](crate::stdlib::backends::lowering::ContentPointer)),
/// through which the lowering rule of the node reads the title. The content
/// pointer must refer to the title argument of a theorem.
///
/// A document cannot cause this error: it is caused by the faulty code that
/// created the node. Reported by the lowering rules
/// [`TitlePointerRule`](super::TitlePointerRule) and
/// [`RelationRule`](super::RelationRule). The node expands to nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.theorems.pointer-refused",
    message = "Invalid content pointer to {region} of a node of ‘{element}’: content \
               pointers in a theorem heading must refer to the title argument of a theorem"
)]
pub struct PointerRefused {
    /// The name of the element of the node that the content pointer refers to.
    pub element: String,
    /// The region of this node that the content pointer refers to, as text,
    /// e.g. `argument 0`.
    pub region: String,
}

/// Error raised when the check stage of the theorems feature fails to read a
/// table (id `flm.theorems.stage-failed`).
///
/// The check stage [`CHECK_PROOFS`](super::CHECK_PROOFS) reads the element
/// index and, when the registry contains the references feature, the table of
/// resolved references. A table read fails only when the code of a feature is
/// faulty: when a stage reads a table that it did not declare as a read, or
/// when the value type of the table differs from the declared one (see
/// [`AccessError`](crate::core::data::store::AccessError)). A document cannot
/// cause this error.
///
/// Reported with no position in the source. The check stage then stops, and
/// reports no other condition for the document.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.theorems.stage-failed", message = "Stage ‘{stage}’ failed: {detail}")]
pub struct StageFailed {
    /// The name of the stage, `flm.theorems.check-proofs`.
    pub stage: String,
    /// The error message of the failed table read.
    pub detail: String,
}
