//! The `math` feature without a feature that processes mathematics, and
//! mathematics without the `math` feature: the errors follow from what the
//! registry and the plan already check. With the standard package `stdcore`
//! alone, every standard workflow plans and runs a document without formulas
//! without a diagnostic, and each formula, a math group or a display-math
//! environment, is one `flm.math.unprocessed` Error; two processing features
//! contest the capability, or the default rule of `flm.math`. A registry
//! without `math` lowers a math group through the `lowering` feature's rule of
//! last resort: one `flm.math.unprocessed` Error, and the content in a span of
//! role `flm.math`.

mod common;

use std::sync::Arc;

use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::core::plan::{Overrides, Plan, PlanError};
use flm_core::core::registry::{BuildError, FlmFeature, FlmRegistry};
use flm_core::core::run::StageCx;
use flm_core::core::data::store::StageOut;
use flm_core::core::plan::PlanCx;
use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_core::lang::Flm;
use flm_core::lang::structural::MATH;
use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::stdcore::backends::pres::{self, SpanPayload, payload_of, plain_text};
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::stdcore::features::math::{self, CONTENT};
use flm_core::math_passthrough;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig, package};
use flm_core::techy::core::node::NodeRef;

use common::lowering::{lowered, render};
use common::pipeline::{feature, registry, registry_error};

/// The features of the standard package, in its order, but those named in
/// `left_out`.
fn standard_but(left_out: &[&str]) -> Vec<Arc<dyn FlmFeature>> {
    package(StdcoreConfig::default())
        .features
        .into_iter()
        .filter(|feature| !left_out.contains(&feature.info().name.as_str()))
        .collect()
}

/// The standard package `stdcore` alone, as a program registers it.
fn stdcore_alone() -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.build(&RegistryInputs::new()).expect("stdcore alone builds")
}

/// The features of the standard package and the feature of the package
/// `math-passthrough`.
fn with_passthrough() -> Vec<Arc<dyn FlmFeature>> {
    let mut features = standard_but(&[]);
    features.push(math_passthrough::feature());
    features
}

/// The four standard workflows.
const WORKFLOWS: [&str; 4] =
    [CHECK_WORKFLOW, text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW];

#[test]
fn without_a_processing_feature_every_standard_workflow_runs_a_document_without_formulas() {
    let registry = stdcore_alone();
    for workflow in WORKFLOWS {
        let run = common::stdcore::run_workflow(
            &registry,
            workflow,
            &["No formula here."],
            &Overrides::none(),
        );
        assert_eq!(
            run.diagnostics().iter().count(),
            0,
            "`{workflow}`: {}",
            run.diagnostics().render_all()
        );
    }
}

#[test]
fn without_a_processing_feature_each_formula_is_an_error() {
    let run = render(&stdcore_alone(), &["a \\(x\\) b\n\n\\begin{align}y \\\\ z\\end{align}"]);
    assert_eq!(unprocessed(&run), ["\\(x\\)", "\\begin{align}y \\\\ z\\end{align}"]);
    assert_eq!(run.diagnostics().iter().count(), 2, "{}", run.diagnostics().render_all());
    // The spans in the lowered tree's storage order, which is not the document's.
    let mut kinds: Vec<Vec<String>> = math_spans(&run).into_iter().map(|span| span.0).collect();
    kinds.sort();
    assert_eq!(kinds, [["display"], ["inline"]]);
}

// --- a second processing feature ----------------------------------------------------------

/// A check stage of the test feature, providing `flm.math.content` beside the
/// passthrough's own check.
struct SecondCheck;

impl Stage for SecondCheck {
    fn decl(&self) -> StageDecl {
        StageDecl::new("t.second.check", Effect::Check, Scope::PerTree)
            .provides(CONTENT)
            .docs("a second check of the mathematics")
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new()
    }

    fn run(&self, _cx: &StageCx<'_>, _out: &mut StageOut<'_>) {}
}

/// A rule of `flm.math` that writes nothing.
struct Nothing;

impl Lower for Nothing {
    fn lower(
        &self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut LowerCx<'_>,
        _lw: &mut dyn Lowerer,
    ) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(Vec::new()))
    }
}

/// A second processing feature: a check providing `flm.math.content` and, when
/// `with_rule`, a rule of `flm.math` marked as the default on every backend.
fn second_processor(with_rule: bool) -> Arc<dyn FlmFeature> {
    feature("t-second", move |b| {
        b.stage(Arc::new(SecondCheck))?;
        if with_rule {
            b.lowering_rule(LoweringRule::new(
                "flm.math.lower.t-second",
                MATH,
                LoweringDecl::new(),
                Arc::new(Nothing),
                DefaultMark::Everywhere,
            ))?;
        }
        Ok(())
    })
}

#[test]
fn two_processing_features_contest_the_check() {
    let mut features = with_passthrough();
    features.push(second_processor(false));
    let registry = registry(features);
    let error = Plan::for_workflow(&registry, CHECK_WORKFLOW, &Overrides::none())
        .expect_err("two stages provide the check");
    match &error {
        PlanError::Contested { capability, candidates, .. } => {
            assert_eq!(capability.as_str(), CONTENT);
            assert_eq!(
                candidates.iter().map(|name| name.as_str()).collect::<Vec<_>>(),
                [math_passthrough::CHECK, "t.second.check"]
            );
        }
        other => panic!("unexpected plan error {other:?}"),
    }
}

#[test]
fn two_processing_features_with_default_rules_fail_the_registry_build() {
    let mut features = with_passthrough();
    features.push(second_processor(true));
    let error = registry_error(features);
    assert!(
        error.errors.iter().any(|error| matches!(
            error,
            BuildError::TwoDefaultRules { element, backend: None, .. } if element.as_str() == MATH
        )),
        "{}",
        error.render()
    );
}

// --- mathematics without the `math` feature -----------------------------------------------

/// The standard package without `math` (and no feature that processes
/// mathematics).
fn without_math() -> Arc<FlmRegistry> {
    registry(standard_but(&[math::FEATURE]))
}

/// The spans of role `flm.math` of the first tree's lowered form, each with its
/// kinds, the element names below it and its plain text.
fn math_spans(run: &flm_core::core::run::Run) -> Vec<(Vec<String>, Vec<String>, String)> {
    let lowered = lowered(run, TreeId::tree(0));
    lowered
        .tree
        .iter_storage_order()
        .filter(|node| element_of(node).name == pres::SPAN)
        .filter_map(|node| {
            let payload = payload_of::<SpanPayload>(&node)?;
            (payload.role.as_ref().map(|role| role.as_str()) == Some(MATH)).then(|| {
                let kinds = payload.kinds.iter().map(|kind| kind.to_string()).collect();
                let below =
                    node.descendants().map(|node| element_of(&node).name.to_string()).collect();
                let text = plain_text(node.body().expect("a body").iter());
                (kinds, below, text)
            })
        })
        .collect()
}

/// The diagnostics of `run` of the identifier `flm.math.unprocessed`, each as
/// the text its span covers.
fn unprocessed(run: &flm_core::core::run::Run) -> Vec<String> {
    run.diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.identifier() == "flm.math.unprocessed")
        .map(|diagnostic| {
            assert_eq!(diagnostic.severity(), flm_core::techy::error::Severity::Error);
            assert_eq!(
                diagnostic.message(),
                "mathematics cannot be rendered: no loaded feature processes it; rendering it needs \
                 the feature `math` and a package that processes mathematics, for example \
                 `math-passthrough` or `math-mathjax`"
            );
            diagnostic.span().content().to_string()
        })
        .collect()
}

#[test]
fn without_math_an_inline_formula_is_an_error_and_its_content_a_span() {
    let run = render(&without_math(), &["a \\(x\\) b"]);
    assert_eq!(unprocessed(&run), ["\\(x\\)"]);
    let spans = math_spans(&run);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].0, ["inline"]);
    assert_eq!(spans[0].2, "x");
}

#[test]
fn without_math_a_displayed_formula_lowers_its_unknown_command_inside_the_span() {
    let run = render(&without_math(), &["a \\[\\alpha\\] b"]);
    assert_eq!(unprocessed(&run), ["\\[\\alpha\\]"]);
    let spans = math_spans(&run);
    assert_eq!(spans.len(), 1);
    assert_eq!(spans[0].0, ["display"]);
    assert!(spans[0].1.iter().any(|name| name == pres::UNKNOWN), "{:?}", spans[0].1);
}

#[test]
fn without_math_the_recomposed_latex_writes_the_content_of_a_formula() {
    let overrides = Overrides::none();
    let run = common::stdcore::run_workflow(
        &without_math(),
        latex_recomposed::WORKFLOW,
        &["a \\(x\\) b \\[\\alpha\\] c \\foo\\(y\\)"],
        &overrides,
    );
    assert_eq!(unprocessed(&run), ["\\(x\\)", "\\[\\alpha\\]", "\\(y\\)"]);
    let text = common::latex::body(&run);
    assert!(text.contains("a x b \\alpha c"), "{text}");
    // A control word followed by the content of a formula that begins with a
    // letter keeps its name apart from that letter.
    assert!(!text.contains("\\fooy"), "{text}");
}

#[test]
fn without_math_the_recomposed_latex_of_a_document_without_formulas_builds() {
    let run = common::stdcore::run_workflow(
        &without_math(),
        latex_recomposed::WORKFLOW,
        &["No formula here."],
        &Overrides::none(),
    );
    assert_eq!(run.diagnostics().iter().count(), 0, "{}", run.diagnostics().render_all());
    assert!(common::latex::body(&run).contains("No formula here."));
}
