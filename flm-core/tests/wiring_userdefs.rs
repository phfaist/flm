//! The `userdefs` feature at registry build
//! (`flm_core::stdcore::features::userdefs`): definitions as elements carrying
//! `Expand`, their names, the templates compiled in the late-init hook, the
//! build diagnostics, overrides, the digest, and one feature value serving
//! several registries.

mod common;

use std::sync::Arc;

use flm_core::model::element::{ElementSpec, element_of};
use flm_core::core::registry::{BuildError, BuildPhase, FlmFeature, FlmRegistry};
use flm_core::dump::ExpectDiag;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::blocks::BlocksConfig;
use flm_core::lang::rewrites::userdefs::{Definition, Expand, UserDefinition};
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::userdefs::{Userdefs, UserdefsConfig, element_name};

use common::pipeline::{feature, registry as build, registry_error};
use common::wiring::{registry, registry_with};
use common::{callables, macro_def, parse};

/// The definition value the element named `name` carries.
fn definition<'r>(registry: &'r FlmRegistry, name: &str) -> &'r UserDefinition {
    registry
        .element_value(name)
        .expect("one value carries the name")
        .expect("registered")
        .value_as::<UserDefinition>()
        .expect("a user definition")
}

/// The `userdefs` feature over one definition, as a registry feature.
fn userdefs(definition: Definition) -> Arc<dyn FlmFeature> {
    userdefs_over(vec![definition])
}

/// The `userdefs` feature over `definitions`, as a registry feature.
fn userdefs_over(definitions: Vec<Definition>) -> Arc<dyn FlmFeature> {
    Arc::new(Userdefs::new(UserdefsConfig::new().definitions(definitions)))
}

// --- elements ------------------------------------------------------------------------------------

#[test]
fn a_definition_registers_an_element_carrying_expand() {
    let registry = registry(vec![Definition::macro_("foo", Vec::new()).text("hello")]);
    let element = registry.element_value("flm.userdefs.macro.foo").unwrap().expect("registered");
    assert!(element.has_aspect::<dyn Expand>());
    assert!(element.value_as::<UserDefinition>().is_some());
}

#[test]
fn a_use_site_carries_the_registered_element_by_identity() {
    let registry = registry(vec![Definition::macro_("foo", Vec::new()).text("hello")]);
    let fragment = parse(registry.lang(), r"\foo");
    let node = &callables(&fragment)[0];
    let registered = registry.element_value("flm.userdefs.macro.foo").unwrap().unwrap();
    assert!(ElementSpec::same_value(element_of(node), registered));
}

#[test]
fn a_macro_and_an_environment_of_one_name_are_two_elements() {
    let definitions = vec![
        Definition::macro_("foo", Vec::new()).text("m"),
        Definition::environment("foo", Vec::new()).text("e"),
    ];
    let registry = registry(definitions.clone());
    for definition in &definitions {
        assert!(registry.element_value(&element_name(definition)).unwrap().is_some());
    }
    assert_eq!(element_name(&definitions[0]), "flm.userdefs.macro.foo");
    assert_eq!(element_name(&definitions[1]), "flm.userdefs.environment.foo");
}

#[test]
fn a_specials_definition_registers_under_its_trigger() {
    let registry = registry(vec![Definition::specials("~", Vec::new()).text(" ")]);
    let fragment = parse(registry.lang(), "a~b");
    let elements: Vec<String> =
        callables(&fragment).iter().map(|node| element_of(node).name.to_string()).collect();
    assert_eq!(elements, ["flm.userdefs.specials.~"]);
}

#[test]
fn two_definitions_of_one_name_and_kind_refuse_the_build() {
    let twice = vec![
        Definition::macro_("foo", Vec::new()).text("a"),
        Definition::macro_("foo", Vec::new()).text("b"),
    ];
    let error = registry_error(vec![userdefs_over(twice)]);
    assert_eq!(error.phase, BuildPhase::Collect);
    assert_eq!(error.errors.len(), 1, "{}", error.render());
    match &error.errors[0] {
        BuildError::Feature { feature, detail } => {
            assert_eq!(feature, "userdefs");
            // Both apply in every mode, so only one of them could ever be in
            // force; a pair whose modes do not overlap is accepted instead.
            assert!(detail.contains("the same parsing mode"), "{detail}");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

/// The identifiers of the build diagnostics the `userdefs` feature reported
/// (a registry over `blocks` alone carries the registry's own warning about
/// the contract check's collection, which is not the feature's).
fn userdefs_diagnostics(registry: &FlmRegistry) -> Vec<&str> {
    registry
        .build_diagnostics()
        .iter()
        .map(|d| d.identifier())
        .filter(|id| id.starts_with("flm.userdefs."))
        .collect()
}

#[test]
fn a_macro_and_an_environment_of_one_name_do_not_clash() {
    let both = vec![
        Definition::macro_("foo", Vec::new()).text("m"),
        Definition::environment("foo", Vec::new()).text("e"),
    ];
    // A macro and an environment of one name are two definitions in the
    // recomposed output too, each under the prefix of its kind, so neither is
    // a reason for anything. What the test pins is that the registry accepts
    // both — no clash — with no `flm.userdefs.*` diagnostic at all.
    assert_eq!(userdefs_diagnostics(&registry(both)), [] as [&str; 0]);
}

// --- the late-init hook and the build diagnostics ---------------------------------------------

#[test]
fn templates_compile_in_the_late_init_hook() {
    let registry = registry(vec![Definition::macro_("foo", Vec::new()).text("hello")]);
    assert!(definition(&registry, "flm.userdefs.macro.foo").compiled().is_some());
    assert!(userdefs_diagnostics(&registry).is_empty());
}

#[test]
fn a_bad_template_is_a_build_diagnostic_that_refuses_its_definition() {
    let bad = Definition::macro_("foo", vec![ArgDef::mandatory("x")]).text("#2");
    let registry = registry(vec![bad]);
    registry.build_diagnostics().expect("flm.userdefs.bad-placeholder").at("#2").once();
    assert!(definition(&registry, "flm.userdefs.macro.foo").compiled().is_none());
}

#[test]
fn a_refused_definition_leaves_the_others_compiled() {
    let definitions = vec![
        Definition::macro_("bad", vec![ArgDef::mandatory("x")]).text("#2"),
        Definition::macro_("good", Vec::new()).text("hello"),
    ];
    let registry = registry(definitions);
    assert!(definition(&registry, "flm.userdefs.macro.bad").compiled().is_none());
    assert!(definition(&registry, "flm.userdefs.macro.good").compiled().is_some());
}

// --- overrides -----------------------------------------------------------------------------------

#[test]
fn an_overriding_definition_replaces_another_features_macro() {
    let other: Arc<dyn FlmFeature> = feature("t", |b| b.element(macro_def("t", "foo", 0)));
    // The declaration being replaced is text-only (a declaration's default); the
    // definition replacing it applies in both modes (a definition's default),
    // which covers it.
    let mine = userdefs(Definition::macro_("foo", Vec::new()).content("hello").overriding());
    let registry = build(vec![other, mine]);
    let fragment = parse(registry.lang(), r"\foo");
    assert_eq!(element_of(&callables(&fragment)[0]).name, "flm.userdefs.macro.foo");
}

#[test]
fn a_clashing_definition_without_override_is_a_language_phase_error() {
    let other: Arc<dyn FlmFeature> = feature("t", |b| b.element(macro_def("t", "foo", 0)));
    let mine = userdefs(Definition::macro_("foo", Vec::new()).text("hello"));
    assert_eq!(registry_error(vec![other, mine]).phase, BuildPhase::Language);
}

// --- the digest ----------------------------------------------------------------------------------

#[test]
fn the_template_text_enters_the_digest() {
    let a = registry(vec![Definition::macro_("foo", Vec::new()).text("hello")]);
    let b = registry(vec![Definition::macro_("foo", Vec::new()).text("hello!")]);
    assert_ne!(a.digest(), b.digest());
}

#[test]
fn the_depth_limit_enters_the_digest() {
    let a = registry_with(Blocks::default(), UserdefsConfig::new());
    let b = registry_with(Blocks::default(), UserdefsConfig::new().recursion_limit(3));
    assert_ne!(a.digest(), b.digest());
}

#[test]
fn the_text_options_enter_the_digest() {
    let a = registry_with(Blocks::default(), UserdefsConfig::new());
    let b = registry_with(Blocks::new(BlocksConfig::no_typography()), UserdefsConfig::new());
    assert_ne!(a.digest(), b.digest());
}

// --- a feature value across registries ---------------------------------------------------------

#[test]
fn a_feature_value_serves_two_registries_with_their_own_compiled_templates() {
    let shared = userdefs(Definition::macro_("foo", Vec::new()).text("hello"));
    let first = build(vec![Arc::clone(&shared)]);
    let second = build(vec![shared]);
    let a = definition(&first, "flm.userdefs.macro.foo");
    let b = definition(&second, "flm.userdefs.macro.foo");
    assert!(!std::ptr::eq(a, b));
    assert!(a.compiled().is_some() && b.compiled().is_some());
}
