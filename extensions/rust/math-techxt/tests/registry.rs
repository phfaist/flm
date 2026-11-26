//! The package in a registry beside the `math` feature, the commands it
//! declares in math mode, the rules of those commands lowered on their own,
//! and the plans its check and render stages are in.

mod common;

use std::sync::Arc;

use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, RegistryBuilder, RegistryInputs,
};
use flm_core::lang::mint::{ElementDef, SyntaxDef};
use flm_core::model::element::ElementSpec;
use flm_core::stdlib::backends::{html, latex_recomposed, latex_rendered, text};
use flm_core::stdlib::{self, CHECK_WORKFLOW, StdlibConfig};
use flm_core::techy::latexlike::Mode;

use common::{FEATURE, PACKAGE, package, registry, selected_rule, workflow_plan};

/// The element of the macros the feature declares.
const MACRO: &str = "math-techxt.macro";

/// The element of the environments the feature declares.
const ENVIRONMENT: &str = "math-techxt.environment";

/// The names the feature declares under the element `element`, each with the
/// parsing modes of its declaration.
fn declared(element: &str) -> Vec<(String, Vec<Mode>)> {
    let registry = registry();
    registry
        .declarations()
        .elements_of(FEATURE)
        .filter(|def| def.element.name.as_str() == element)
        .flat_map(|def| def.names.iter().map(|name| (name.to_string(), def.modes.clone())))
        .collect()
}

#[test]
fn the_package_entry_holds_the_one_feature() {
    let entry = package();
    assert_eq!(entry.name.as_str(), PACKAGE);
    let names: Vec<String> = entry.features.iter().map(|f| f.info().name.to_string()).collect();
    assert_eq!(names, [FEATURE]);
}

#[test]
fn the_feature_registers_beside_the_math_feature_without_a_clash_or_a_diagnostic() {
    // The standard package holds the `math` feature, which declares its text
    // commands and `\ensuremath` in math mode, and `\\`, `\tag`, `\nonumber`
    // and `\label` in a display-math environment's body: a name declared by
    // both features in math mode would fail the build. The feature's `\\`, in
    // math mode, stands beside the `math` feature's in the body.
    let registry = registry();
    assert!(registry.has_feature("math"));
    assert!(registry.has_feature(FEATURE));
    let diagnostics: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

/// A feature that declares `\frac` in math mode, as a second processor of
/// mathematics might.
struct FracStandIn;

impl FlmFeature for FracStandIn {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("standin", "A stand-in declaring \\frac in math mode")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let element = Arc::new(ElementSpec::new("standin.frac"));
        b.element(ElementDef::new("standin", ["frac"], SyntaxDef::Macro { args: Vec::new() }, element).modes([Mode::Math]))
    }
}

#[test]
fn a_command_declared_by_another_feature_in_math_mode_fails_the_build() {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
    builder.feature(Arc::new(FracStandIn));
    builder.use_package(package());
    let error = builder.build(&RegistryInputs::new()).expect_err("the two declarations of `\\frac` clash");
    assert!(error.to_string().contains("`\\frac`"), "{error}");
}

#[test]
fn the_notation_is_declared_in_math_mode_and_the_document_library_is_not() {
    let macros = declared(MACRO);
    let environments = declared(ENVIRONMENT);
    assert!(macros.len() > 500, "{}", macros.len());
    for (name, modes) in macros.iter().chain(&environments) {
        assert_eq!(modes, &[Mode::Math], "{name}");
    }
    let has = |list: &[(String, Vec<Mode>)], name: &str| list.iter().any(|(declared, _)| declared == name);
    for name in ["frac", "alpha", "mathbb", "hat", "quad", "overline", "{", "}", "$", "&", "#", "_", "%"] {
        assert!(has(&macros, name), "{name}");
    }
    assert!(has(&environments, "pmatrix"));
    // The line break, which ends a row of a matrix in a math group.
    assert!(has(&macros, "\\"));
    for name in ["eqref", "ref", "cite", "footnote", "section"] {
        assert!(!has(&macros, name) && !has(&environments, name), "{name}");
    }
    // The `math` feature's names.
    for name in ["text", "mbox", "tag", "nonumber", "label", "ensuremath"] {
        assert!(!has(&macros, name), "{name}");
    }
    // No specials: `^`, `_`, `&` and `~` are characters of the formula.
    let registry = registry();
    let specials = registry
        .declarations()
        .elements_of(FEATURE)
        .filter(|def| !matches!(def.syntax, SyntaxDef::Macro { .. } | SyntaxDef::Environment { .. }))
        .count();
    assert_eq!(specials, 0);
}

#[test]
fn a_command_of_techxt_outside_a_formula_lowers_to_its_source_on_every_backend() {
    let registry = registry();
    for (workflow, backend) in [(text::WORKFLOW, "text"), (html::WORKFLOW, "html"), (latex_recomposed::WORKFLOW, "latex-recomposed")] {
        let rule = |element| selected_rule(&registry, workflow, backend, element);
        assert_eq!(rule(MACRO).as_deref(), Some("math-techxt.macro.lower.source"), "{backend}");
        assert_eq!(rule(ENVIRONMENT).as_deref(), Some("math-techxt.environment.lower.source"), "{backend}");
    }
}

#[test]
fn the_check_is_in_every_plan_and_the_render_stage_only_where_a_rule_reads_its_table() {
    let registry = registry();
    for workflow in [text::WORKFLOW, html::WORKFLOW, latex_rendered::WORKFLOW, latex_recomposed::WORKFLOW, CHECK_WORKFLOW] {
        let stages: Vec<String> = workflow_plan(&registry, workflow).stage_names().iter().map(ToString::to_string).collect();
        let has = |stage: &str| stages.iter().any(|name| name == stage);
        assert!(has("math-techxt.check"), "{workflow}: {stages:?}");
        let rendered = workflow == text::WORKFLOW || workflow == html::WORKFLOW;
        assert_eq!(has("math-techxt.render"), rendered, "{workflow}: {stages:?}");
    }
}
