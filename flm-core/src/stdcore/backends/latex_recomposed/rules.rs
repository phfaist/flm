//! The two structural lowering rules ([`LoweringRule`]) of the
//! `latex-recomposed` backend, for the paragraph break and the paragraph:
//! the only lowering rules that the
//! backend's feature registers itself ([`rules`]).
//!
//! Both rules write a blank line, which LaTeX reads as a paragraph break, and
//! both require the raw tree:
//!
//! | element | rule | writes |
//! |---|---|---|
//! | `flm.parbreak` | [`PARBREAK_RULE`] ([`ParbreakRule`]) | one blank line, however many blank lines the author wrote at that place |
//! | `flm.paragraph` | [`PARAGRAPH_RULE`] ([`ParagraphRule`]) | the paragraph's content, lowered, then one blank line |
//!
//! # The raw tree
//!
//! The recomposed LaTeX keeps the document as the author typed it, so it is
//! written from the *raw* tree: the [frozen
//! tree](crate::guide::concepts_overview#frozen-trees-and-computed-data) that
//! the [block-structure pass](crate::lang::rewrites::blocks) has not
//! processed, in which no paragraphs were built, no typographic
//! substitution was made, the text is as parsed, and the paragraph-break
//! markers (`flm.parbreak`) are still in place. A backend states the tree
//! state it needs through the requirements of its lowering rules, not through
//! a setting: both rules here require the capability `flm.blocks.raw`
//! ([`RAW`]), which the [`blocks`](crate::stdcore::features::blocks) feature
//! provides through a stage that does nothing and only marks this tree
//! state. The lowering stage of a backend requires
//! every capability that the backend's selected rules require
//! ([`rules_needs`](crate::stdcore::backends::lowering::rules_needs)), so
//! every plan for this backend keeps its trees raw, regardless of what the
//! document holds.
//!
//! The paragraph rule never applies on the raw tree, where no paragraph group
//! exists. It is registered because `flm.paragraph` is a structural element
//! present in every registry, and building a plan for a backend fails
//! ([`PlanError::Incomplete`](crate::core::plan::PlanError::Incomplete)) when
//! some element that a document may contain has neither a native emission
//! nor a rule on that backend. Both rules stage (build into the lowered
//! tree; see [`Lowerer`]) a `flm.pres.latex.raw` node holding the blank line
//! ([`LatexRawPayload`]): a blank line is LaTeX syntax that no other construct
//! element expresses.
//!
//! Both rules are marked as the default on this backend only
//! ([`DefaultMark::OnBackends`]), so that they do not affect the `text`
//! backend, which writes `flm.parbreak` natively and lowers `flm.paragraph`
//! through the general rule of the `lowering` feature
//! ([`PARAGRAPH_RULE`](crate::stdcore::backends::lowering::PARAGRAPH_RULE)).
//!
//! # Where the other recomposition rules are registered
//!
//! Each standard feature registers the lowering rule that writes its own
//! elements back as LaTeX (its *recomposition rule*), in a `latex` module of
//! its own, beside the feature's general rule. Such a rule refers to things
//! the feature does not register: this backend, in its default mark; the
//! LaTeX construct elements, in what it produces; and for some features a
//! table another feature owns, such as the references feature's table of
//! safe labels. The feature makes a peer declaration ([`PeerName`](crate::core::registry::PeerName)) of each:
//! a forward declaration that the registry build resolves once every feature
//! has registered. When a peer declaration resolves to nothing, the rule is
//! set aside (excluded from rule selection) and the build records one Note.
//! A host that registers the `textstyles` feature without the `latex`
//! feature and this backend therefore still builds its registry; a host that
//! registers this backend gets every feature's recomposition rule selected
//! like any other rule. This backend's feature declares no content feature as
//! a dependency: a registry that holds it needs only the `latex`, `lowering`
//! and `check` features.

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::lang::structural::{PARAGRAPH, PARBREAK};
use crate::lang::Flm;
use crate::model::names::ElementName;
use crate::stdcore::backends::latex::LatexRawPayload;
use crate::stdcore::backends::latex::construct::RAW as LATEX_RAW;
use crate::stdcore::features::blocks::RAW;

use super::feature::BACKEND;

/// The registered name of the paragraph-break rule ([`ParbreakRule`]),
/// `flm.parbreak.lower.latex-recomposed`.
pub const PARBREAK_RULE: &str = "flm.parbreak.lower.latex-recomposed";

/// The registered name of the paragraph rule ([`ParagraphRule`]),
/// `flm.paragraph.lower.latex-recomposed`.
pub const PARAGRAPH_RULE: &str = "flm.paragraph.lower.latex-recomposed";

/// The text of the `flm.pres.latex.raw` node that both rules stage: two
/// newlines, which LaTeX reads as one paragraph break.
pub const BLANK_LINE: &str = "\n\n";

/// Stages a `flm.pres.latex.raw` node holding one blank line ([`BLANK_LINE`])
/// and returns its identifier in the tree being built.
fn blank_line(cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer) -> Result<techy::core::node::BuildId, LowerError> {
    lw.pres_with(cx, &ElementName::from(LATEX_RAW), LatexRawPayload::new(BLANK_LINE), Vec::new())
}

/// The lowering rule registered as [`PARBREAK_RULE`]: it lowers a
/// paragraph-break marker (`flm.parbreak`) to one blank line.
///
/// The author may have written several blank lines at that place; LaTeX reads
/// any number of them as one paragraph break, and the recomposed LaTeX writes
/// one.
#[derive(Debug, Default, Clone, Copy)]
pub struct ParbreakRule;

impl Lower for ParbreakRule {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(vec![blank_line(cx, lw)?]))
    }
}

/// The lowering rule registered as [`PARAGRAPH_RULE`]: it lowers a paragraph
/// group (`flm.paragraph`) to its content, lowered, followed by one blank
/// line.
///
/// The rule never applies on the raw tree that this backend requires; it
/// exists so that the plan's completeness check
/// ([`PlanError::Incomplete`](crate::core::plan::PlanError::Incomplete)) finds
/// a rule for `flm.paragraph` (see the module documentation).
#[derive(Debug, Default, Clone, Copy)]
pub struct ParagraphRule;

impl Lower for ParagraphRule {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut LowerCx<'_>,
        lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        let mut nodes = lw.lower_children(cx, &node)?;
        nodes.push(blank_line(cx, lw)?);
        Ok(Lowering::Emit(nodes))
    }
}

/// The two structural lowering rules of this backend, [`PARBREAK_RULE`] then
/// [`PARAGRAPH_RULE`], ready to be registered; the [`LatexRecomposed`] feature
/// registers them.
///
/// Each rule requires the capability [`RAW`] (`flm.blocks.raw`), produces
/// `flm.pres.latex.raw` nodes, and is marked as the default on the
/// `latex-recomposed` backend only ([`DefaultMark::OnBackends`]).
///
/// [`LatexRecomposed`]: super::LatexRecomposed
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::backends::latex_recomposed::rules::{PARBREAK_RULE, rules};
///
/// let names: Vec<String> = rules().iter().map(|rule| rule.name.to_string()).collect();
/// assert_eq!(names[0], PARBREAK_RULE);
/// assert!(rules().iter().all(|rule| rule.decl.requires[0].as_str() == "flm.blocks.raw"));
/// ```
pub fn rules() -> Vec<LoweringRule> {
    let on_this_backend = || DefaultMark::OnBackends(vec![BACKEND.into()]);
    vec![
        LoweringRule::new(
            PARBREAK_RULE,
            PARBREAK,
            LoweringDecl::new().requires(RAW).produces(LATEX_RAW),
            Arc::new(ParbreakRule),
            on_this_backend(),
        )
        .docs("a paragraph-break marker lowers to a raw blank line; requires the raw tree"),
        LoweringRule::new(
            PARAGRAPH_RULE,
            PARAGRAPH,
            LoweringDecl::new().requires(RAW).produces(LATEX_RAW),
            Arc::new(ParagraphRule),
            on_this_backend(),
        )
        .docs(
            "a paragraph group lowers to its lowered content followed by a raw blank line; \
             requires the raw tree, in which no paragraph group occurs",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_rules_require_the_raw_tree_and_default_on_this_backend_only() {
        for rule in rules() {
            assert_eq!(rule.decl.requires, [crate::model::names::CapName::from(RAW)]);
            assert_eq!(rule.decl.produces, [ElementName::from(LATEX_RAW)]);
            assert!(rule.default.is_specific_to(&BACKEND.into()));
            assert!(!rule.default.is_specific_to(&"text".into()));
        }
        assert_eq!(rules()[0].element, PARBREAK);
        assert_eq!(rules()[1].element, PARAGRAPH);
    }
}
