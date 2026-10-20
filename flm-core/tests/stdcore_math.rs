//! The `math` feature (`flm_core::stdcore::features::math`): an unknown macro
//! or environment in math mode parses silently as `flm.math-passthrough.opaque` while text
//! mode stays strict, a command of another standard feature is opaque inside a
//! formula (every declaration but the `math` feature's text commands and
//! equation-line commands is text-only), the opaque element carries through a
//! math group into the text output and owns the rule that plan completeness
//! relies on, the display-math environments lower to a display `math-passthrough`
//! naming them, the passthrough check provides `flm.math.content` and joins
//! `flm.validated`, and `flm.math.rendered` is declared with no provider.

mod common;

use std::sync::Arc;

use flm_core::model::element::element_of;
use flm_core::core::plan::{Overrides, Plan, PlanError, PlanRequest};
use flm_core::core::registry::FlmRegistry;
use flm_core::dump::golden::assert_json_golden;
use flm_core::lang::structural;
use flm_core::stdcore::backends::pres;
use flm_core::stdcore::backends::{lowering, text};
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::math::{
    self, CONTENT, ENVIRONMENT, ENVIRONMENTS, MathEnvironment, RENDERED,
};
use flm_core::math_passthrough::{CHECK, OPAQUE, OPAQUE_RULE};
use flm_core::stdcore::{VALIDATED, check_feature};

use common::lowering::{lowered_projection, text as text_of};
use common::stdcore::{golden_path, identifiers, registry_over, run_workflow};

/// A registry over the `math` feature, the lowering machinery, the text
/// backend, the `blocks` feature and the `check` feature (whose collection
/// the content check joins: the membership tests read it).
fn registry() -> Arc<FlmRegistry> {
    registry_over(vec![
        lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        Arc::new(Blocks::default()),
        check_feature(),
        math::feature(),
        flm_core::math_passthrough::feature(),
    ])
}

/// The element names of the callable nodes of a parse, in document order.
fn callable_elements(registry: &FlmRegistry, text: &str) -> Vec<String> {
    let fragment = common::parse(registry.lang(), text);
    common::callables(&fragment).iter().map(|node| element_of(node).name.to_string()).collect()
}

#[test]
fn an_unknown_macro_in_math_mode_parses_silently_as_the_opaque_element() {
    let registry = registry();
    let fragment = common::parse(registry.lang(), "\\(\\alpha + x\\)");
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    assert_eq!(callable_elements(&registry, "\\(\\alpha + x\\)"), [OPAQUE]);
}

#[test]
fn an_unknown_environment_in_math_mode_parses_silently_as_the_opaque_element() {
    let registry = registry();
    let text = "\\[\\begin{aligned}a &= b\\end{aligned}\\]";
    let fragment = common::parse(registry.lang(), text);
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    assert_eq!(callable_elements(&registry, text), [OPAQUE]);
}

#[test]
fn a_command_of_another_standard_feature_is_opaque_inside_a_formula() {
    // Every declaration of a feature other than `math` is text-only: inside a
    // formula `\section` and `\href` are unknown commands, kept as written.
    let registry = common::stdcore::registry(flm_core::stdcore::StdcoreConfig::new());
    for text in ["\\(\\section{x}\\)", "\\(\\href{u}{x}\\)"] {
        let fragment = common::parse(registry.lang(), text);
        assert!(fragment.diags().is_empty(), "{text}: {:?}", fragment.diags());
        assert_eq!(callable_elements(&registry, text), [OPAQUE], "{text}");
    }
}

#[test]
fn an_unknown_macro_in_text_mode_stays_strict() {
    let registry = registry();
    let fragment = common::parse(registry.lang(), "\\alpha");
    assert!(fragment.diags().has_errors());
    // The math feature's fallback answers in math mode only; in text mode the
    // language layer's own recovery answers.
    assert_eq!(callable_elements(&registry, "\\alpha"), [structural::UNKNOWN_MACRO]);
}

#[test]
fn opaque_math_carries_through_a_math_group_into_the_text() {
    let document = "see \\(\\alpha_i\\) here";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(text_of(&run), "see \\(\\alpha_i\\) here\n");
}

#[test]
fn the_opaque_element_owns_the_rule_completeness_relies_on() {
    let registry = registry();
    let rules: Vec<&str> = registry
        .rules_for(OPAQUE)
        .iter()
        .map(|idx| registry.rule(*idx).rule.name.as_str())
        .collect();
    assert_eq!(rules, [OPAQUE_RULE]);
    let rule = &registry.rule(registry.rules_for(OPAQUE)[0]).rule;
    assert_eq!(rule.decl.produces, [flm_core::model::names::ElementName::from(pres::MATH_PASSTHROUGH)]);
    assert!(!registry.backend(text::BACKEND).unwrap().native.contains(OPAQUE));
}

/// The rules of the mathematics elements over the whole standard package, in
/// registration order: each element's general default beside one rule per
/// backend that wants its own. A rule marked for named backends is no
/// candidate on any other, so adding one changes no other backend's plan.
///
/// `flm.math` is the language's own element: the `math-passthrough` feature's
/// two rules come first, in that feature's registration order, then the
/// `lowering` feature's rule of last resort (`flm.math.lower.default`, never a
/// default); the environment element's rule of last resort
/// (`flm.math.environment.lower.default`, never a default) comes first, as the
/// `math` feature registers before `math-passthrough`, then that feature's
/// general rule.
#[test]
fn each_mathematics_element_has_its_general_rule_and_one_rule_per_backend_that_wants_its_own() {
    use flm_core::stdcore::StdcoreConfig;

    let registry = common::stdcore::registry(StdcoreConfig::new());
    let rules = |element: &str| -> Vec<String> {
        registry
            .rules_for(element)
            .iter()
            .map(|idx| registry.rule(*idx).rule.name.to_string())
            .collect()
    };
    assert_eq!(
        rules(structural::MATH),
        ["flm.math.lower.default", "flm.math.lower.passthrough", "flm.math.lower.passthrough-latex-recomposed"]
    );
    assert_eq!(
        rules(ENVIRONMENT),
        [
            "flm.math.environment.lower.default",
            "flm.math.environment.lower.passthrough",
            "flm.math.environment.lower.passthrough-latex-recomposed"
        ],
        "the rule of last resort, the processing feature's own default, then one rule per \
         backend that wants its own"
    );
    // The text commands keep one rule on every backend but the recomposed
    // LaTeX: on `html` the general rule is what a formula's own rule delegates
    // a text command to.
    assert_eq!(
        rules(math::text::TEXT),
        ["flm.math.text.lower.default", "flm.math.text.lower.latex-recomposed"]
    );
}

/// In a registry without the recomposed LaTeX backend, the rules of the
/// `math` and `math-passthrough` features for it are set aside: a rule whose
/// feature's peer declaration nothing resolved is no candidate at all, so this
/// registry's plans are exactly what they were. The passthrough rules of
/// `flm.math` and of the display-math environments are not among them: they
/// are the defaults on every backend, so they depend on no backend's presence.
#[test]
fn the_rules_for_the_recomposed_latex_are_set_aside_where_that_backend_is_not_registered() {
    let registry = registry();
    let set_aside: Vec<String> =
        registry.set_aside_rules().map(|entry| entry.rule.name.to_string()).collect();
    assert_eq!(
        set_aside,
        [
            "flm.math.text.lower.latex-recomposed",
            "flm.math.eqref.lower.latex-recomposed",
            "flm.math.lower.passthrough-latex-recomposed",
            "flm.math.environment.lower.passthrough-latex-recomposed"
        ]
    );
}

#[test]
fn the_environments_are_six_block_values_of_one_element_type() {
    let registry = registry();
    let names: Vec<(String, bool)> = registry
        .elements_named(ENVIRONMENT)
        .iter()
        .map(|idx| {
            let value = registry.element(*idx).unwrap().element.value_as::<MathEnvironment>();
            let value = value.expect("valued");
            (value.name.to_string(), value.numbered)
        })
        .collect();
    let expected: Vec<(String, bool)> =
        ENVIRONMENTS.iter().map(|(name, numbered)| (name.to_string(), *numbered)).collect();
    assert_eq!(names, expected);
}

#[test]
fn a_display_environment_lowers_to_a_displayed_passthrough_formula_naming_it() {
    let run = run_workflow(
        &registry(),
        text::WORKFLOW,
        &["Before.\n\n\\begin{align*}\n  a &= \\alpha \\\\\n  b &= 2\n\\end{align*}\n\nAfter."],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_json_golden(golden_path("math-environment.json"), &lowered_projection(&run));
    assert_eq!(
        text_of(&run),
        "Before.\n\n  \\begin{align*}\n    a &= \\alpha \\\\\n    b &= 2\n  \\end{align*}\n\n\
         After.\n"
    );
}

#[test]
fn math_content_is_provided_by_the_passthrough_check() {
    let registry = registry();
    let plan = Plan::build(&registry, &PlanRequest::new().demand(CONTENT)).expect("plans");
    let names: Vec<String> = plan.stage_names().iter().map(|n| n.to_string()).collect();
    assert_eq!(names, [CHECK]);
    let run = run_workflow(&registry, "check", &["\\(a\n\nb\\)"], &Overrides::none());
    assert!(!identifiers(&run).iter().any(|id| id.starts_with("flm.math.")));
}

#[test]
fn the_content_check_is_a_member_of_the_validated_collection() {
    let registry = registry();
    let collection = registry.collection(VALIDATED).expect("declared");
    assert!(collection.members.contains(&registry.cap_id(CONTENT).expect("declared")));
}

#[test]
fn rendered_math_is_declared_with_no_provider() {
    let registry = registry();
    assert!(registry.capability_named(RENDERED).is_some());
    let error = Plan::build(&registry, &PlanRequest::new().demand(RENDERED)).expect_err("nobody");
    assert!(matches!(error, PlanError::Unsatisfiable { .. }), "{error}");
}

#[test]
fn the_text_plan_over_the_math_feature_is_complete() {
    let plan =
        Plan::for_workflow(&registry(), text::WORKFLOW, &Overrides::none()).expect("complete");
    assert!(plan.active_backends().contains(text::BACKEND));
}
