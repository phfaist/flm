//! The conditions that the [`theorems`](super) feature reports, each under
//! the identifier `flm.theorems.<condition>`. A condition is the typed body of
//! a diagnostic (see [diagnostics](crate::guide::concepts_overview#diagnostics));
//! each condition's documentation states its severity and the recovery the
//! feature applies (see [recoveries](crate::stdcore::backends::lowering::error)).
//!
//! - The feature's check ([`CHECK_PROOFS`](super::CHECK_PROOFS)) reports the
//!   two conditions a document can cause, [`MissingProof`] and
//!   [`BadRelationRef`], under every workflow, and [`StageFailed`] when it
//!   cannot read a table.
//! - The heading's lowering rules report [`PointerRefused`].
//!
//! Two problems with theorems are reported by the references feature, as for
//! any reference: a back-reference to a label that nothing declares is
//! `flm.refs.unresolved`, and a `\label` on a proof is
//! `flm.refs.label-type-not-accepted`.

use techy::error::DiagnosticInfo;

/// Condition `flm.theorems.missing-proof`, a Warning: a theorem of a kind
/// that [expects a proof](super::TheoremKind::expects_proof) has no proof
/// that refers back to it (no proof's title `[*label]` or `[**label]`
/// resolves to it), and it does not carry `\noproofref`. Reported at the
/// theorem by the feature's check. Recovery: none is needed; the output is
/// unchanged.
///
/// The message identifies the theorem by its kind and its first label of a
/// type the kind accepts, and gives the author's three possible fixes: a
/// proof whose title refers to the label, the field `\noproofref` on the
/// theorem, or the kind's [`expects_proof`](super::TheoremKind::expects_proof)
/// set to `false`:
///
/// ```text
/// theorem thm:main has no proof that refers back to it. Write its proof as
/// `\begin{proof}[*thm:main]`; or mark the theorem with `\noproofref`; or set
/// `expects_proof` to false for the kind `theorem` in the configuration of the
/// theorems feature.
/// ```
///
/// The environment of the first fix is [`proof_kind`](Self::proof_kind): the
/// first kind, in the configuration's order, of a class that [reads
/// back-references](super::TheoremClass::relation_ref) (`proof` under every
/// shipped configuration). Where no class reads them, no proof can refer
/// back, and the message leaves that fix out.
///
/// A theorem with no such label cannot be referred to by a proof's title, so
/// the message then says so, and its first fix is to give the theorem a
/// label of the first type its kind accepts ([`label_type`](Self::label_type)).
/// A kind that expects a proof always accepts a label: the configuration is
/// checked for it ([`TheoremsConfig::validate`](super::TheoremsConfig::validate)).
///
/// The message is written for a person; a program reads the fields, which
/// carry the same facts.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.theorems.missing-proof")]
pub struct MissingProof {
    /// The kind of the theorem, its environment's name (`theorem`, or a
    /// host's `exercise`).
    pub kind: String,
    /// The theorem's first label of a type its kind accepts, in the order the
    /// theorem's labels are written, as `type:label`; `None` for a theorem
    /// with no such label. A label of a type the kind does not accept defines
    /// nothing (the references feature reports it), so a proof's title could
    /// not refer to the theorem through it.
    pub label: Option<String>,
    /// The label type the message suggests for a theorem with no label: the
    /// first of the types its kind accepts.
    pub label_type: String,
    /// The environment that the message gives for a proof that refers back
    /// to the theorem: the first kind, in the configuration's order, of a
    /// class that reads back-references; `None` where no class does, and the
    /// message then gives only the two other fixes.
    pub proof_kind: Option<String>,
}

/// The message gives the three fixes, and the first depends on whether the
/// theorem has a label and whether a kind can refer back to it, so the
/// wordings are written here rather than as one format string.
impl std::fmt::Display for MissingProof {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let kind = &self.kind;
        match (&self.proof_kind, &self.label) {
            (Some(proof), Some(label)) => write!(
                f,
                "{kind} {label} has no proof that refers back to it. Write its proof as \
                 `\\begin{{{proof}}}[*{label}]`; or mark"
            )?,
            (Some(proof), None) => {
                let ty = &self.label_type;
                write!(
                    f,
                    "{kind} has no label and no proof that refers back to it. Give it a \
                     `\\label{{{ty}:...}}` and write its proof as `\\begin{{{proof}}}[*{ty}:...]`; \
                     or mark"
                )?
            }
            (None, Some(label)) => {
                write!(f, "{kind} {label} has no proof that refers back to it. Mark")?
            }
            (None, None) => write!(f, "{kind} has no proof that refers back to it. Mark")?,
        }
        write!(
            f,
            " the {kind} with `\\noproofref`; or set `expects_proof` to false for the kind \
             `{kind}` in the configuration of the theorems feature."
        )
    }
}

/// Condition `flm.theorems.bad-relation-ref`, an Error: the title of a
/// theorem whose class reads back-references begins with a star but is not a
/// well-formed back-reference. Either it is not one run of plain characters
/// (`[*\emph{x}]`), or the text after the stars is not of the form
/// `type:label` (`[*nolabel]`, `[*]`). Reported by the feature's check at the
/// title, under every workflow. Recovery: the title is shown as written, and
/// the theorem refers to nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.theorems.bad-relation-ref",
    message = "the title of this `{environment}` environment begins with `*`, so it must be written \
               `*type:label` or `**type:label`, in plain characters, with the label of the \
               statement it proves"
)]
pub struct BadRelationRef {
    /// The theorem environment's name, as the document writes it.
    pub environment: String,
}

/// Condition `flm.theorems.pointer-refused`, an Error: a node that shows a
/// theorem's title in its heading ([`TITLE_POINTER`](super::TITLE_POINTER)
/// or [`RELATION`](super::RELATION)) refers to something other than the
/// title argument of a theorem. No document can cause it: it is the mistake
/// of a rule that built a heading, which would otherwise show content that is
/// not a theorem's title. Reported at the node. Recovery: the node writes
/// nothing.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(
    id = "flm.theorems.pointer-refused",
    message = "a pointer in a theorem's heading must point at the title argument of a \
               theorem, but it points at the {region} of a node of `{element}`"
)]
pub struct PointerRefused {
    /// The element of the node the pointer refers to.
    pub element: String,
    /// The region of that node the pointer refers to, as text (for example
    /// `argument 0`).
    pub region: String,
}

/// Condition `flm.theorems.stage-failed`, an Error: the feature's check could
/// not read a table it declared, which means that the plan does not match the
/// stage's declarations. No document can cause it. Recovery: the check reports
/// nothing else.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "flm.theorems.stage-failed", message = "the stage `{stage}` failed: {detail}")]
pub struct StageFailed {
    /// The stage that failed.
    pub stage: String,
    /// What failed.
    pub detail: String,
}
