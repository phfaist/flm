//! The rules of the three elements of the table of contents on the
//! `latex-recomposed` backend: each writes nothing.
//!
//! The recomposed LaTeX never shows FLM's table of contents: LaTeX builds its
//! own from the headings (`\tableofcontents`, written by the author or by a
//! template). The piece is not placed on that output unless a render request
//! or a template names it for that backend, and where it is, it writes
//! nothing. The rules exist because a plan checks that its backend can lower
//! every element the registry holds, as the list of xnotes has a rule on that
//! backend that writes nothing
//! ([`ListLatexRule`](crate::stdcore::features::xnotes::latex::ListLatexRule)).
//!
//! The rules are marked default on the `latex-recomposed` backend only, under
//! a peer declaration of that backend, so that a registry without it still
//! builds, with the rules set aside. They read nothing and stage nothing.

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::stdcore::backends::latex_recomposed::BACKEND;

use super::{ENTRY, LIST, PIECE};

/// Returns the name of the rule of the element `element` on the
/// `latex-recomposed` backend, `<element>.lower.latex-recomposed`.
pub fn rule_name(element: &str) -> String {
    format!("{element}.lower.latex-recomposed")
}

/// The rule of each element of the table of contents on the
/// `latex-recomposed` backend: it writes nothing (see the [module
/// documentation](self)).
#[derive(Debug, Default, Clone, Copy)]
pub struct NothingRule;

impl Lower for NothingRule {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        _lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(Vec::new()))
    }
}

/// Registers the rule [`NothingRule`] of each of the three elements, marked
/// default on the `latex-recomposed` backend only, under the peer declaration
/// of that backend.
pub(super) fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(BACKEND, "the recomposed-LaTeX backend, on which the table of contents writes nothing")?;
    for element in [PIECE, LIST, ENTRY] {
        b.lowering_rule(
            LoweringRule::new(
                rule_name(element),
                element,
                LoweringDecl::new(),
                Arc::new(NothingRule),
                DefaultMark::OnBackends(vec![BACKEND.into()]),
            )
            .docs("the table of contents writes nothing in the recomposed LaTeX: LaTeX builds its own"),
        )?;
    }
    Ok(())
}
