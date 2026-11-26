//! This module defines the check stage [`CHECK_PROOFS`]
//! (`flm.theorems.check-proofs`) of the [`theorems`](super) feature. The stage
//! checks that each theorem whose kind expects a proof has a proof, and that
//! the back-references in the titles of proofs are valid. In this module, a
//! *theorem* is an environment of any theorem kind, such as `lemma` or `proof`.
//!
//! The stage reports two conditions, in document order:
//!
//! - the warning [`MissingProof`] for each theorem that has no proof although
//!   its kind [expects a proof](super::TheoremKind::expects_proof), unless the
//!   theorem carries the field `\noproofref`;
//! - the error [`BadRelationRef`] for each title of a proof that begins with a
//!   star but is not a valid back-reference.
//!
//! # How proofs are matched with theorems
//!
//! A proof refers to the theorem that it proves with a back-reference in its
//! title, as in `\begin{proof}[*lem:key]` or `\begin{proof}[**lem:key]`.
//! Back-references are read in the titles of the theorem kinds whose class has
//! [`relation_ref`](super::TheoremClass::relation_ref) set. In the default
//! configuration, `proof` is the only such kind.
//!
//! A back-reference is a reference to a label, and the references feature
//! resolves it as it resolves `\ref{lem:key}`. The stage reads these
//! resolutions from the table [`REF_RESOLUTIONS`] of the references feature. A
//! theorem has a proof when at least one back-reference resolves to this
//! theorem. A `\ref` to a theorem does not count as a proof.
//!
//! In a registry without the references feature, no back-reference is
//! resolved, and the stage does not report [`MissingProof`]. The stage still
//! reports [`BadRelationRef`], which depends only on the title.
//!
//! # Workflows that run the check
//!
//! The stage provides the capability [`CHECK_PROOFS`], which is a member of the
//! collection [`flm.validated`](crate::stdlib::VALIDATED). Every standard
//! workflow demands this collection: the [`check`
//! workflow](crate::stdlib::CHECK_WORKFLOW) and each rendering workflow. The
//! stage therefore runs under every standard workflow, with the same reports.
//!
//! # Theorems in kept user definitions
//!
//! The stage reads the document trees. A plan can keep the use sites of a user
//! definition in the tree, instead of expanding them (see [kept
//! definitions](crate::stdlib::features::userdefs::kept)). The stage then
//! does not see the content of the definition: a theorem in the template of
//! the definition is not checked, a proof there does not count as the proof of
//! the theorem that it refers to, and a malformed back-reference there is not
//! reported.
//!
//! Every plan for the `latex-recomposed` backend keeps the use sites of the
//! user definitions with the expansion policy `per-plan`. On any backend, when
//! a plan request selects the `instantiate-at-lowering` lowering rule of a
//! definition, the plan keeps the use sites of this definition.

use std::collections::BTreeSet;

use techy::error::Diagnostic;
use techy::core::node::NodeRef;

use crate::core::data::store::StageOut;
use crate::core::plan::PlanCx;
use crate::core::registry::FlmRegistry;
use crate::core::run::StageCx;
use crate::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use crate::lang::{Flm, FlmOrigin, FlmSpan, nowhere};
use crate::model::aspects::Referenceable;
use crate::stdlib::features::index::{ELEMENT_INDEX, walk_values_of};
use crate::stdlib::features::refs::{REF_RESOLUTIONS, Resolution};

use super::config::TheoremsConfig;
use super::diagnostics::{BadRelationRef, MissingProof, StageFailed};
use super::elements::{
    TITLE_ARG, TheoremElement, TitleReading, has_noproofref, relation_site_id, theorem_point,
    title_reading,
};

/// The name of the check stage of the [`theorems`](super) feature,
/// `flm.theorems.check-proofs`.
///
/// The stage provides a capability of the same name, which is a member of the
/// collection [`flm.validated`](crate::stdlib::VALIDATED). A plan request can
/// also demand this capability directly. See the [module documentation](self)
/// for the conditions that the stage reports.
pub const CHECK_PROOFS: &str = "flm.theorems.check-proofs";

/// The check stage [`CHECK_PROOFS`] of the theorems feature (see the [module
/// documentation](self)).
pub(super) struct CheckProofs {
    /// The theorem kind that the message of [`MissingProof`] suggests for
    /// writing a proof: the first kind, in the alphabetical order of the kind
    /// names, whose class reads back-references. `None` when no class reads
    /// back-references.
    proof_kind: Option<String>,
}

impl CheckProofs {
    /// Returns the check stage for the theorems configuration `config`.
    pub(super) fn new(config: &TheoremsConfig) -> CheckProofs {
        let proof_kind = config
            .kinds
            .iter()
            .find(|(_, kind)| config.classes.get(&kind.class).is_some_and(|class| class.relation_ref))
            .map(|(name, _)| name.to_string());
        CheckProofs { proof_kind }
    }
}

/// Returns whether the resolutions table [`REF_RESOLUTIONS`] of the references
/// feature is declared in `registry`. The stage reads this table only in that
/// case.
fn resolutions_declared(registry: &FlmRegistry) -> bool {
    registry.cap_id(REF_RESOLUTIONS.name()).is_some()
}

/// Returns the span at which a malformed back-reference in the title of the
/// theorem `node` is reported: the span of the first node of the title, or the
/// span of the theorem when the title is absent or empty.
fn title_span(node: &NodeRef<'_, Flm>) -> FlmSpan {
    node.argument_content_nodes_named(TITLE_ARG)
        .ok()
        .flatten()
        .and_then(|title| title.iter().next().map(|first| first.span().clone()))
        .unwrap_or_else(|| node.span().clone())
}

/// Returns the first label of the theorem `node` whose type the kind of the
/// theorem accepts, written as `type:label`, or `None` when the theorem has no
/// such label. The labels and the accepted types are read from the reference
/// point of the theorem. A label of another type does not define anything, so
/// a proof cannot refer to the theorem through it.
fn accepted_label(value: &TheoremElement, node: &NodeRef<'_, Flm>) -> Option<String> {
    let point = value.points(node).into_iter().next()?;
    let label = point.labels.iter().find(|label| point.accepts.accepts(&label.ref_type))?;
    Some(label.to_string())
}

/// Reports the error [`StageFailed`] for a failed table read, with `detail` as
/// the error message of the read.
fn stage_failed(out: &mut StageOut<'_>, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: CHECK_PROOFS.to_string(), detail: detail.to_string() },
        nowhere(FlmOrigin::none()),
    ));
}

impl Stage for CheckProofs {
    fn decl(&self) -> StageDecl {
        StageDecl::new(CHECK_PROOFS, Effect::Check, Scope::Document).provides(CHECK_PROOFS).docs(
            "Reports a warning for each theorem that has no proof although its kind expects \
             one, unless the theorem carries `\\noproofref`. Reports an error for each title \
             of a proof that begins with a star but is not a valid back-reference.",
        )
    }

    /// Returns the reads of the stage. The stage reads the element index
    /// [`ELEMENT_INDEX`] to find the theorems of the document. When the
    /// resolutions table [`REF_RESOLUTIONS`] of the references feature is
    /// declared in the registry, the stage also reads this table; the plan then
    /// runs the resolve stage of the references feature before this stage.
    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        let needs = Needs::new().reads(ELEMENT_INDEX);
        if resolutions_declared(cx.registry()) { needs.reads(REF_RESOLUTIONS) } else { needs }
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let index = match cx.get(ELEMENT_INDEX) {
            Ok(Some(index)) => index,
            Ok(None) => return,
            Err(error) => return stage_failed(out, error),
        };
        let resolutions = if resolutions_declared(cx.registry()) {
            match cx.get(REF_RESOLUTIONS) {
                Ok(resolutions) => resolutions,
                Err(error) => return stage_failed(out, error),
            }
        } else {
            None
        };
        // First every theorem, and the points the back-references resolved
        // to; then the reports, in document order.
        let mut theorems = Vec::new();
        let mut proved = BTreeSet::new();
        for (addr, node, value) in walk_values_of::<TheoremElement>(cx, index) {
            let site = relation_site_id(addr);
            if let Some(Resolution::Resolved { point: Some(point), .. }) =
                resolutions.and_then(|resolutions| resolutions.site(site))
            {
                proved.insert(*point);
            }
            theorems.push((addr, node, value));
        }
        for (addr, node, value) in theorems {
            if title_reading(&node) == Some(TitleReading::Malformed) {
                out.diags.push(Diagnostic::error(
                    BadRelationRef { environment: value.name.to_string() },
                    title_span(&node),
                ));
            }
            let point = theorem_point(addr);
            if resolutions.is_some()
                && value.kind.expects_proof
                && !proved.contains(&point)
                && !has_noproofref(&node)
            {
                // A kind that expects a proof accepts a label: the
                // configuration was checked.
                let Some(label_type) = value.label_types.first() else { continue };
                let missing = MissingProof {
                    kind: value.name.to_string(),
                    label: accepted_label(value, &node),
                    label_type: label_type.clone(),
                    proof_kind: self.proof_kind.clone(),
                };
                out.diags.push(Diagnostic::warning(missing, node.span().clone()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_stage_is_a_document_check_providing_its_capability() {
        let decl = CheckProofs::new(&TheoremsConfig::new()).decl();
        assert_eq!(decl.name, CHECK_PROOFS);
        assert_eq!(decl.effect, Effect::Check);
        assert_eq!(decl.scope, Scope::Document);
        assert_eq!(decl.provides, [crate::model::names::CapName::from(CHECK_PROOFS)]);
        assert!(decl.writes.is_empty());
    }

    #[test]
    fn the_proof_kind_is_the_first_kind_of_a_class_reading_back_references() {
        assert_eq!(CheckProofs::new(&TheoremsConfig::new()).proof_kind.as_deref(), Some("proof"));
        assert_eq!(CheckProofs::new(&TheoremsConfig::new().without_kind("proof")).proof_kind, None);
    }
}
