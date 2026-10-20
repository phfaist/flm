//! The recomposition rules of the [`literals`](super) feature for the
//! `latex-recomposed` backend: every literal, and `\@`, written back as the
//! author wrote it.
//!
//! Both rules stage (build into the lowered tree; see
//! [`Lowerer`]) a raw node,
//! [`flm.pres.latex.raw`](crate::stdcore::backends::latex::construct::RAW): a
//! construct element whose text the emitter writes byte for byte, with no
//! escaping ([`LatexRawPayload`]). A literal's spelling is LaTeX syntax that must
//! reach LaTeX exactly as written; a raw node carries such text.
//!
//! - [`LITERAL_LATEX_RULE`] (`flm.literal.lower.latex-recomposed`) writes the
//!   literal's spelling ([`Literal::source`](super::Literal::source)): `\%`,
//!   `\#`, `\&`, `\$`, `\ `, `\{`, `\}`, `\textbackslash` or `~`, which LaTeX
//!   reads as the character the author meant.
//! - [`SENTENCE_SPACING_LATEX_RULE`] (`flm.sentence-spacing.lower.latex-recomposed`)
//!   writes `\@`, LaTeX's hint about the space after a period.
//!
//! After the spelling, each rule writes the whitespace that the parse took after
//! the command's name, exactly as written. Only a command whose name is made of
//! letters takes such whitespace (`\textbackslash `), and it keeps the command
//! apart from a letter written next (`\textbackslash x`); LaTeX reads it the same
//! way the parse did.
//!
//! Without these rules, the backend would apply the default rules of the
//! [`literals`](super) feature, and the recomposed LaTeX would differ from what
//! the author wrote: each literal would become its character as text, so that
//! the space of `\ ` would reach LaTeX as an ordinary space, which LaTeX merges
//! with the spaces around it, `~` would be written as the character U+00A0 (or
//! its LaTeX spelling, under the backend's `Replace` Unicode mode), a
//! brace or a backslash would be written in the escaped form the backend
//! chooses, and `\@` would be dropped. Both rules are the default on the
//! `latex-recomposed` backend only.
//!
//! The feature does not own the backend or the raw construct. [`register`]
//! declares both as peer declarations
//! ([`PeerName`](crate::core::registry::PeerName)), so that a registry without
//! the LaTeX features still builds; the two rules are then set aside (excluded
//! from rule selection; see [`PeerName`](crate::core::registry::PeerName)).

use std::sync::Arc;

use techy::core::node::NodeRef;
use techy::latexlike::InvocationSyntaxData;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::core::registry::{BuildError, FeatureBuilder};
use crate::lang::Flm;
use crate::model::element::element_value;
use crate::model::names::ElementName;
use crate::stdcore::backends::latex::LatexRawPayload;
use crate::stdcore::backends::latex::construct::RAW;
use crate::stdcore::backends::latex_recomposed::BACKEND;

use super::{LITERAL, LiteralElement, SENTENCE_SPACING};

/// The name of the recomposition rule of `flm.literal`,
/// `flm.literal.lower.latex-recomposed`, whose body is
/// [`LiteralLatexRule`].
pub const LITERAL_LATEX_RULE: &str = "flm.literal.lower.latex-recomposed";

/// The name of the recomposition rule of `\@` (`flm.sentence-spacing`),
/// `flm.sentence-spacing.lower.latex-recomposed`, whose body is
/// [`SentenceSpacingLatexRule`].
pub const SENTENCE_SPACING_LATEX_RULE: &str = "flm.sentence-spacing.lower.latex-recomposed";

/// The LaTeX written for `\@`: `\@`.
pub const SENTENCE_SPACING_SOURCE: &str = "\\@";

/// Stages a raw node holding `spelling` followed by the whitespace that the
/// parse took after `node`'s command name, exactly as written (see the module
/// documentation): what both rules stage.
fn spelling_as_written(
    cx: &mut LowerCx<'_>,
    lw: &mut dyn Lowerer,
    node: &NodeRef<'_, Flm>,
    spelling: &str,
) -> Result<Lowering, LowerError> {
    let mut written = String::from(spelling);
    if let Some(InvocationSyntaxData::Macro { post_space, .. }) = node.invocation_syntax() {
        written.push_str(post_space.resolve(node.span().source()));
    }
    let raw = lw.pres_with(cx, &ElementName::from(RAW), LatexRawPayload::new(written), Vec::new())?;
    Ok(Lowering::Emit(vec![raw]))
}

/// The body of [`LITERAL_LATEX_RULE`]: a raw node holding the literal's
/// spelling as written ([`Literal::source`](super::Literal::source)).
///
/// The rule returns a [`LowerError`] when the node carries no
/// [`LiteralElement`] or when the raw node could not be staged.
#[derive(Debug, Default, Clone, Copy)]
pub struct LiteralLatexRule;

impl Lower for LiteralLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let value = element_value::<LiteralElement>(&node)
            .ok_or_else(|| LowerError::structure("a literal node without its element's data"))?;
        spelling_as_written(cx, lw, &node, value.literal.source())
    }
}

/// The body of [`SENTENCE_SPACING_LATEX_RULE`]: a raw node holding `\@`.
#[derive(Debug, Default, Clone, Copy)]
pub struct SentenceSpacingLatexRule;

impl Lower for SentenceSpacingLatexRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        spelling_as_written(cx, lw, &node, SENTENCE_SPACING_SOURCE)
    }
}

/// The two recomposition rules, ready to be registered: each produces
/// `flm.pres.latex.raw` and is the default on the `latex-recomposed` backend
/// only.
pub fn latex_rules() -> Vec<LoweringRule> {
    let on_this_backend = || DefaultMark::OnBackends(vec![BACKEND.into()]);
    vec![
        LoweringRule::new(
            LITERAL_LATEX_RULE,
            LITERAL,
            LoweringDecl::new().produces(RAW),
            Arc::new(LiteralLatexRule),
            on_this_backend(),
        )
        .docs("a literal lowers to a raw node holding its spelling as written"),
        LoweringRule::new(
            SENTENCE_SPACING_LATEX_RULE,
            SENTENCE_SPACING,
            LoweringDecl::new().produces(RAW),
            Arc::new(SentenceSpacingLatexRule),
            on_this_backend(),
        )
        .docs("`\\@` lowers to a raw node holding `\\@`"),
    ]
}

/// Registers what the `literals` feature contributes to the
/// `latex-recomposed` backend: a peer declaration
/// ([`PeerName`](crate::core::registry::PeerName)) of the backend and of the
/// raw construct, with [`LatexRawPayload`] as its payload type, then
/// [`latex_rules`].
///
/// In a registry without the LaTeX features the two peer declarations resolve
/// to nothing and the rules are set aside (excluded from rule selection); the
/// registry still builds.
///
/// # Errors
///
/// Returns the [`BuildError`] the feature builder raises, for instance for a
/// rule already registered under the same name.
pub fn register(b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
    b.peer_backend(
        BACKEND,
        "the latex-recomposed backend, on which the recomposition rules of the literals are \
         the default",
    )?;
    b.peer_element::<LatexRawPayload>(
        RAW,
        "the LaTeX family's raw construct, which the recomposition rules of the literals \
         stage",
    )?;
    for rule in latex_rules() {
        b.lowering_rule(rule)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_rules_produce_raw_pieces_on_the_recomposed_backend_only() {
        let rules = latex_rules();
        assert_eq!(rules[0].element, LITERAL);
        assert_eq!(rules[1].element, SENTENCE_SPACING);
        for rule in &rules {
            assert_eq!(rule.decl.produces, [ElementName::from(RAW)]);
            assert!(rule.default.is_specific_to(&BACKEND.into()));
        }
    }
}
