//! The `expand-in-tree` lowering rule that every user definition registers:
//! the rule of the definition's element on every backend where the plan
//! expands the definition's use sites before lowering.
//!
//! A lowering rule ([`LoweringRule`]) is written for one element and stages
//! (builds into the lowered tree) that element's presentation for one backend.
//! Every element an author can write needs a rule on each backend of a plan,
//! unless the backend writes the element natively or the plan request accepts
//! it as unrenderable; otherwise the plan build fails
//! ([`PlanError::Incomplete`](crate::core::plan::PlanError::Incomplete)). A user
//! definition's element is one an author can write, so it has a rule like
//! any other: this one, the default on every backend
//! ([`DefaultMark::Everywhere`]). The rule exists for two reasons.
//!
//! - **The element has a rule on every backend.** With this rule registered, a
//!   plan can be built for any backend over a registry that holds user
//!   definitions with no element left without a rule, although the expansion
//!   removes every use site before lowering.
//! - **Selecting the rule puts the expansion into the plan.** The rule requires
//!   the trees table of the rewrite stage that expands its definition:
//!   [`EXPANDED`] for a definition whose expansion policy is `per-plan`,
//!   [`ALWAYS_EXPANDED`] for an `always` one. Every plan that lowers a
//!   registry holding an `always` definition therefore holds
//!   `flm.userdefs.expand-always`. A plan that demands `flm.userdefs.kept` on
//!   a backend where this rule is the one selected for a `per-plan` definition
//!   is refused when the plan is built, since the two alternatives of
//!   `flm.userdefs` exclude each other. On the `latex-recomposed` backend the
//!   rule selected for a `per-plan` definition is `emit-call` instead, which
//!   requires the other alternative ([`super::kept`]).
//!
//! # The rule never lowers a use site
//!
//! Every tree that the lowering stage of a backend lowers (a document tree, or
//! a [synthesized value](crate::guide::concepts_overview#synthesized-values)
//! brought up to date by [value
//! catch-up](crate::guide::concepts_overview#value-catch-up)) has had the
//! plan's rewrite stages run over it, the expansion this rule requires among
//! them. No use site of the definition is left in such a tree: the expansion replaced each
//! one by its instantiated template or, where it could not, by its literal
//! source text with a diagnostic. The body of the rule ([`ExpandInTree`])
//! therefore never receives a node. If it did, the expansion would have failed
//! to keep that guarantee, and the rule returns a structure error
//! ([`LowerError::structure`], reported as `flm.lowering.rule-failed`) rather
//! than expanding the use site at lowering.
//!
//! # One rule per definition
//!
//! A rule targets one element, and every definition is an element of its own,
//! so each definition has its own rule, named
//! `flm.userdefs.<kind>.<name>.lower.expand-in-tree` ([`rule_name`]). The rule
//! declares no presentation element, since it stages nothing.
//! [`expand_in_tree_rule`] builds the rule of a definition of the `userdefs`
//! feature; [`expand_in_tree_rule_for`] builds it for an element that another
//! crate's feature registers for a definition of its own.

use std::sync::Arc;

use techy::core::node::NodeRef;

use crate::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use crate::model::names::{ElementName, RuleName};
use crate::lang::Flm;
use crate::lang::rewrites::userdefs::{Definition, ExpandPolicy};

use super::{ALWAYS_EXPANDED, EXPANDED, element_name};

/// The last segment of the name of every `expand-in-tree` rule,
/// `expand-in-tree`.
pub const EXPAND_IN_TREE: &str = "expand-in-tree";

/// Returns the name of the `expand-in-tree` rule of `definition`:
/// `flm.userdefs.<kind>.<name>.lower.expand-in-tree`, the definition's element
/// name ([`element_name`]) followed by `.lower.` and
/// [`EXPAND_IN_TREE`].
///
/// # Examples
///
/// ```
/// use flm_core::stdcore::features::userdefs::{Definition, rule_name};
///
/// let definition = Definition::macro_("foo", Vec::new());
/// assert_eq!(rule_name(&definition), "flm.userdefs.macro.foo.lower.expand-in-tree");
/// ```
pub fn rule_name(definition: &Definition) -> RuleName {
    rule_name_of(&element_name(definition))
}

/// Returns the name of the `expand-in-tree` rule of the element `element`.
fn rule_name_of(element: &ElementName) -> RuleName {
    RuleName::from(format!("{element}.lower.{EXPAND_IN_TREE}"))
}

/// Returns the `expand-in-tree` rule of `definition`, a definition of the
/// `userdefs` feature: the rule targets the definition's element
/// ([`element_name`]), requires the trees table of the
/// rewrite stage that expands definitions of its expansion policy, and is the
/// default on every backend. See the module documentation.
pub fn expand_in_tree_rule(definition: &Definition) -> LoweringRule {
    expand_in_tree_rule_for(element_name(definition), definition)
}

/// Returns the `expand-in-tree` rule of `definition` for the element named
/// `element`, under the rule name `<element>.lower.expand-in-tree`.
/// [`expand_in_tree_rule`] calls it with the element name of the `userdefs`
/// feature; a feature of another crate that registers an element of its own
/// for a definition (named, for example, with
/// [`element_name_under`](super::element_name_under)) calls it with that
/// element's name.
///
/// The rule requires the trees table of the rewrite stage that expands
/// definitions of the definition's expansion policy: [`ALWAYS_EXPANDED`] for
/// an `always` definition, [`EXPANDED`] for a `per-plan` one. Selecting the
/// rule therefore puts that stage into the plan. Only the `userdefs` feature
/// declares these capabilities, so a registry that holds such a rule without
/// the `userdefs` feature fails to build.
///
/// # Examples
///
/// ```
/// use flm_core::lang::rewrites::userdefs::ExpandPolicy;
/// use flm_core::stdcore::features::userdefs::{
///     Definition, element_name_under, expand_in_tree_rule_for,
/// };
///
/// let definition = Definition::macro_("ket", Vec::new()).expand(ExpandPolicy::Always);
/// let rule = expand_in_tree_rule_for(element_name_under("mypack", &definition), &definition);
/// assert_eq!(rule.name, "mypack.macro.ket.lower.expand-in-tree");
/// ```
pub fn expand_in_tree_rule_for(
    element: impl Into<ElementName>,
    definition: &Definition,
) -> LoweringRule {
    let element = element.into();
    let (required, stage) = match definition.expand {
        ExpandPolicy::Always => (ALWAYS_EXPANDED.name(), "flm.userdefs.expand-always"),
        ExpandPolicy::PerPlan => (EXPANDED.name(), "flm.userdefs.expand"),
    };
    LoweringRule::new(
        rule_name_of(&element),
        element,
        LoweringDecl::new().requires(required),
        Arc::new(ExpandInTree),
        DefaultMark::Everywhere,
    )
    .docs(format!(
        "the use sites of `{}` are expanded in the tree by `{stage}` before lowering \
         (requires `{required}`), so this rule never lowers a node",
        definition.display_name()
    ))
}

/// The body of every `expand-in-tree` rule. It never receives a node, because
/// every use site was expanded before lowering (see the module
/// documentation); if it receives one, it returns a structure error
/// ([`LowerError::structure`]), since the expansion failed to keep that
/// guarantee.
#[derive(Debug, Default, Clone, Copy)]
pub struct ExpandInTree;

impl Lower for ExpandInTree {
    fn lower(
        &self,
        node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        _lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Err(LowerError::structure(format!(
            "a use site of `{}` reached lowering unexpanded, although the expansion the rule \
             requires runs over every tree lowered",
            node.name().unwrap_or_default()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::names::CapName;

    #[test]
    fn a_per_plan_definitions_rule_requires_the_expanded_table() {
        let rule = expand_in_tree_rule(&Definition::macro_("foo", Vec::new()).text("x"));
        assert_eq!(rule.name, "flm.userdefs.macro.foo.lower.expand-in-tree");
        assert_eq!(rule.element, "flm.userdefs.macro.foo");
        assert_eq!(rule.decl.requires, [CapName::from("flm.userdefs.expanded")]);
        assert!(rule.decl.produces.is_empty());
        assert_eq!(rule.default, DefaultMark::Everywhere);
    }

    #[test]
    fn an_always_definitions_rule_requires_the_always_expanded_table() {
        let definition =
            Definition::environment("foo", Vec::new()).text("x").expand(ExpandPolicy::Always);
        let rule = expand_in_tree_rule(&definition);
        assert_eq!(rule.name, "flm.userdefs.environment.foo.lower.expand-in-tree");
        assert_eq!(rule.decl.requires, [CapName::from("flm.userdefs.always-expanded")]);
    }
}
