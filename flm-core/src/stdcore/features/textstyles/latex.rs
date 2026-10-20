//! The recomposition rule of the [`textstyles`](super) feature for the
//! `latex-recomposed` backend: each formatting command is written back as the
//! LaTeX macro of the same name.
//!
//! [`LATEX_RULE`] (`flm.textstyle.lower.latex-recomposed`) is the default rule of
//! `flm.textstyle` on the `latex-recomposed` backend only. It stages (builds into
//! the lowered tree; see [`Lowerer`]) a macro
//! construct, [`flm.pres.latex.macro`](crate::stdcore::backends::latex::construct::MACRO),
//! named after the command the author wrote (`\emph`, `\textbf`, `\textit` or
//! `\texttt`; [`TextStyle::command`](super::TextStyle::command)), with one
//! mandatory argument that holds the command's argument lowered through the
//! rules. A construct element is a presentation element that mirrors one
//! construct of LaTeX, here a macro call ([backends, lowering and
//! outputs](crate::guide::concepts_overview#backends-lowering-and-outputs)). The
//! LaTeX kernel defines the four commands, so the rule requests no LaTeX package.
//!
//! When the argument is missing (a parse recovery left it absent and the parse
//! reported it), the macro is written with an empty argument, `\emph{}`, and
//! nothing more is reported.
//!
//! The feature does not own the backend or the macro construct. [`register`]
//! declares both as peer declarations ([`PeerName`](crate::core::registry::PeerName)),
//! so that a registry without the LaTeX features still builds; the rule is then
//! set aside (excluded from rule selection; see
//! [`PeerName`](crate::core::registry::PeerName)).

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::element::element_value;
use crate::model::names::ElementName;
use crate::stdcore::backends::latex::construct::{ARG_SLOT, MACRO};
use crate::stdcore::backends::latex::{LatexArgShape, LatexMacroPayload};
use crate::stdcore::backends::latex_recomposed::BACKEND;
use crate::stdcore::backends::lowering::lower_arg_or_nothing;

use super::{TEXT_STYLE, TextStyleElement};

/// The name of the recomposition rule of `flm.textstyle`,
/// `flm.textstyle.lower.latex-recomposed`, whose body is
/// [`TextStyleLatexRule`].
pub const LATEX_RULE: &str = "flm.textstyle.lower.latex-recomposed";

/// The body of [`LATEX_RULE`]: it stages the formatting command as a macro
/// construct of the same name, over the command's argument lowered through
/// the rules (see the module documentation).
///
/// The rule returns a [`LowerError`] when the node carries no
/// [`TextStyleElement`], or when the argument or the construct could not be
/// lowered or staged.
#[derive(Debug, Default, Clone, Copy)]
pub struct TextStyleLatexRule;

impl Lower for TextStyleLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<TextStyleElement>(&node)
            .ok_or_else(|| LowerError::structure("a text-style node without its element's data"))?;
        let content = lower_arg_or_nothing(cx, lw, &node, 0)?;
        let payload = LatexMacroPayload::new(value.style.command(), [LatexArgShape::Mandatory]);
        let macro_call = lw.pres_slots_with(
            cx,
            &ElementName::from(MACRO),
            payload,
            vec![(String::from(ARG_SLOT), content)],
        )?;
        Ok(Lowering::Emit(vec![macro_call]))
    }
}

/// The recomposition rule [`LATEX_RULE`], ready to be registered: it targets
/// `flm.textstyle`, produces `flm.pres.latex.macro` and is the default on the
/// `latex-recomposed` backend only.
pub fn latex_rule() -> LoweringRule {
    LoweringRule::new(
        LATEX_RULE,
        TEXT_STYLE,
        LoweringDecl::new().produces(MACRO),
        Arc::new(TextStyleLatexRule),
        DefaultMark::OnBackends(vec![BACKEND.into()]),
    )
    .docs("a formatting command lowers to the LaTeX macro of the same name over its argument")
}

/// Registers what the `textstyles` feature contributes to the
/// `latex-recomposed` backend: a peer declaration
/// ([`PeerName`](crate::core::registry::PeerName)) of the backend and of the
/// macro construct, with [`LatexMacroPayload`] as its payload type, then
/// [`latex_rule`].
///
/// In a registry without the LaTeX features the two peer declarations resolve
/// to nothing and the rule is set aside (excluded from rule selection); the
/// registry still builds.
///
/// # Errors
///
/// Returns the [`BuildError`] the feature builder raises, for instance for a
/// rule already registered under the same name.
pub fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the recomposition rule of the formatting \
         commands is the default",
    )?;
    b.peer_element::<LatexMacroPayload>(
        MACRO,
        "the LaTeX family's macro construct, which the recomposition rule of the formatting \
         commands stages",
    )?;
    b.lowering_rule(latex_rule())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rule_is_the_default_on_the_recomposed_backend_only() {
        let rule = latex_rule();
        assert_eq!(rule.name, LATEX_RULE);
        assert_eq!(rule.element, TEXT_STYLE);
        assert_eq!(rule.decl.produces, [ElementName::from(MACRO)]);
        assert!(rule.default.is_specific_to(&BACKEND.into()));
    }
}
