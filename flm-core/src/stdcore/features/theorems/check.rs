//! The check stage of the [`theorems`](super) feature,
//! `flm.theorems.check-proofs` ([`CHECK_PROOFS`]): it reports every statement
//! that expects a proof and has none, and every title that begins with a star
//! but is not a well-formed back-reference.
//!
//! # What the check reports
//!
//! The stage visits every theorem of the document in document order and
//! reports two conditions:
//!
//! - [`MissingProof`] (`flm.theorems.missing-proof`), a **Warning** at the
//!   environment: the theorem is of a kind that
//!   [expects a proof](super::TheoremKind::expects_proof), no proof's
//!   back-reference (shown, `[*lem:x]`, or not, `[**lem:x]`) refers to it,
//!   and it does not carry the field `\noproofref`. The message identifies the
//!   theorem by its kind and its first label of a type the kind accepts, and
//!   gives the author's possible fixes (see [`MissingProof`]). The condition
//!   is a Warning because the output is complete as it is: it reminds the
//!   author of a missing proof and reports no fault in what is written.
//! - [`BadRelationRef`] (`flm.theorems.bad-relation-ref`), an **Error** at
//!   the title: a title that begins with a star, on a kind that reads
//!   back-references, but is not a well-formed back-reference
//!   ([`TitleReading::Malformed`]). The theorem's rules show such a title as
//!   written and report nothing themselves, so that the condition is reported
//!   once per document, regardless of the workflow.
//!
//! # What the check reads
//!
//! The check reads the document's trees, so it reaches a theorem written
//! inside a user definition's template only where the plan expands the
//! definition in the tree before the stages run. A plan that keeps a user
//! definition (the `flm.userdefs.kept` alternative; see
//! [kept definitions](crate::stdcore::features::userdefs::kept)) leaves the
//! call in the tree, not the definition's content. Two kinds of plan do so:
//! every plan for the recomposed LaTeX, where the default lowering rule of a
//! user definition that a plan may keep is `emit-call` (it keeps the call in
//! the tree even where it then instantiates the definition at lowering), and
//! any plan that selects the definition's `instantiate-at-lowering` lowering
//! rule, on any backend. Under such a plan the check sees nothing inside the
//! definition: a statement there is not
//! checked, a proof there does not count as a proof of the statement it
//! refers to, and a malformed back-reference there is not reported. On the
//! recomposed LaTeX, LaTeX itself numbers the theorems that a kept
//! definition holds, resolves their references and sets their headings, and
//! the check leaves them to LaTeX as well.
//!
//! # How the check decides that a proof refers to a statement
//!
//! The check matches no label itself. A back-reference is a reference site
//! that the references feature resolves as it resolves `\ref`. The
//! resolutions table ([`REF_RESOLUTIONS`]) says which reference point each
//! theorem's site resolved to, and a statement has a proof when one of those
//! points is its own. A plain `\ref` to a statement is not a proof of it; a
//! proof of a statement that expects none, such as a definition, is not
//! reported.
//!
//! # Where the check runs
//!
//! The stage provides the capability of its own name, [`CHECK_PROOFS`],
//! which the feature contributes as a member of the
//! [`flm.validated`](crate::stdcore::VALIDATED) collection. Every standard
//! workflow demands that collection, the
//! [`check` workflow](crate::stdcore::CHECK_WORKFLOW) and each rendering
//! workflow alike, so the check runs, with the same reports, under every
//! workflow.
//!
//! In a registry without the references feature no back-reference resolves,
//! and the stage reports no missing proof; it still reports a malformed
//! back-reference, which depends on the title alone.

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
use crate::stdcore::features::index::{ELEMENT_INDEX, walk_values_of};
use crate::stdcore::features::refs::{REF_RESOLUTIONS, Resolution};

use super::config::TheoremsConfig;
use super::diagnostics::{BadRelationRef, MissingProof, StageFailed};
use super::elements::{
    TITLE_ARG, TheoremElement, TitleReading, has_noproofref, relation_site_id, theorem_point,
    title_reading,
};

/// The name of the feature's check stage, and of the capability it provides,
/// `flm.theorems.check-proofs`: a member of the
/// [`flm.validated`](crate::stdcore::VALIDATED) collection; a plan request
/// may also demand it directly. See the [module documentation](self).
pub const CHECK_PROOFS: &str = "flm.theorems.check-proofs";

/// The check stage, [`CHECK_PROOFS`]: a document-scoped stage of the effect
/// `check`. See the [module documentation](self).
pub(super) struct CheckProofs {
    /// The environment that the missing-proof message gives as the one that
    /// refers back to a statement: the first kind, in the configuration's
    /// order, of a class that reads back-references; `None` where no class
    /// does.
    proof_kind: Option<String>,
}

impl CheckProofs {
    /// The stage of the feature configured by `config`.
    pub(super) fn new(config: &TheoremsConfig) -> CheckProofs {
        let proof_kind = config
            .kinds
            .iter()
            .find(|(_, kind)| config.classes.get(&kind.class).is_some_and(|class| class.relation_ref))
            .map(|(name, _)| name.to_string());
        CheckProofs { proof_kind }
    }
}

/// Whether `registry` declares the references feature's resolutions table;
/// the stage reads it only then.
fn resolutions_declared(registry: &FlmRegistry) -> bool {
    registry.cap_id(REF_RESOLUTIONS.name()).is_some()
}

/// Where a malformed back-reference is reported: the title's first node, or
/// the theorem where the title holds none.
fn title_span(node: &NodeRef<'_, Flm>) -> FlmSpan {
    node.argument_content_nodes_named(TITLE_ARG)
        .ok()
        .flatten()
        .and_then(|title| title.iter().next().map(|first| first.span().clone()))
        .unwrap_or_else(|| node.span().clone())
}

/// The first label of the theorem `node` whose type its kind accepts, written
/// `type:label`, read from the reference point the theorem declares and
/// tested as the references feature tests it: a label of another type defines
/// nothing, so no proof's title could refer to the theorem through it.
fn accepted_label(value: &TheoremElement, node: &NodeRef<'_, Flm>) -> Option<String> {
    let point = value.points(node).into_iter().next()?;
    let label = point.labels.iter().find(|label| point.accepts.accepts(&label.ref_type))?;
    Some(label.to_string())
}

/// Reports a table the stage could not read, as [`StageFailed`].
fn stage_failed(out: &mut StageOut<'_>, detail: impl ToString) {
    out.diags.push(Diagnostic::error(
        StageFailed { stage: CHECK_PROOFS.to_string(), detail: detail.to_string() },
        nowhere(FlmOrigin::none()),
    ));
}

impl Stage for CheckProofs {
    fn decl(&self) -> StageDecl {
        StageDecl::new(CHECK_PROOFS, Effect::Check, Scope::Document).provides(CHECK_PROOFS).docs(
            "Reports every statement of a kind expecting a proof that no proof's \
             back-reference resolves to and that carries no `\\noproofref` (a Warning), and \
             every title beginning with a star that is not a well-formed back-reference (an \
             Error).",
        )
    }

    /// Reads the element index, to find the theorems, and, where the
    /// registry holds the references feature, the resolutions table, so that
    /// the plan schedules the references feature's resolve stage before this
    /// one.
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
