//! The order of features in the registry build: chains given by packages and
//! by the host, the final order (a sort of the chains, ties broken by the
//! order the features were added), the cycle error, and the rule that decides
//! when two features may define one name.

mod common;

use std::sync::Arc;

use techy::latexlike::Mode;

use flm_core::core::registry::{
    BuildError, BuildPhase, FeatureChain, FlmFeature, FlmRegistry, PackageEntry,
    RegistryBuildError, RegistryBuilder, RegistryInputs,
};
use flm_core::lang::mint::{BodyDef, CallableKind, Symbol};

use common::pipeline::feature;
use common::{env_def, macro_def};

/// A feature `name` that defines the macros `names` in math mode.
fn math_macros(name: &'static str, names: &'static [&'static str]) -> Arc<dyn FlmFeature> {
    feature(name, move |b| {
        for macro_name in names {
            b.element(macro_def(name, macro_name, 0).modes([Mode::Math]))?;
        }
        Ok(())
    })
}

/// A feature `name` that defines the macro `\Im` in math mode and declares it
/// as an override.
fn overriding_im(name: &'static str) -> Arc<dyn FlmFeature> {
    feature(name, move |b| {
        b.element(macro_def(name, "Im", 0).modes([Mode::Math]))?;
        b.override_callable(CallableKind::Macro, "Im")
    })
}

fn chain(text: &str, origin: &str) -> FeatureChain {
    FeatureChain::parse(text, origin).expect("a chain")
}

/// Builds `features`, added in this order, with the host's `chains`.
fn build(features: Vec<Arc<dyn FlmFeature>>, chains: &[&str]) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    let mut builder = RegistryBuilder::new();
    for feature in features {
        builder.feature(feature);
    }
    for text in chains {
        builder.feature_order(chain(text, "front matter of doc.flm, line 4"));
    }
    builder.build(&RegistryInputs::new())
}

fn built(features: Vec<Arc<dyn FlmFeature>>, chains: &[&str]) -> Arc<FlmRegistry> {
    build(features, chains).unwrap_or_else(|error| panic!("the registry builds:\n{error}"))
}

fn language_error(features: Vec<Arc<dyn FlmFeature>>, chains: &[&str]) -> String {
    match build(features, chains) {
        Ok(_) => panic!("the registry build was expected to fail"),
        Err(error) => {
            assert_eq!(error.phase, BuildPhase::Language, "{error}");
            error.render()
        }
    }
}

fn feature_names(registry: &FlmRegistry) -> Vec<String> {
    registry.features().iter().map(|info| info.name.to_string()).collect()
}

/// The owner whose definition of the macro `name` applies.
fn owner_of(registry: &FlmRegistry, name: &str) -> String {
    let symbol = Symbol { kind: CallableKind::Macro, name: name.to_string() };
    registry.lang().symbols()[&symbol].to_string()
}

// --- the final order ----------------------------------------------------------------------------

#[test]
fn the_chains_of_packages_and_of_the_host_decide_the_final_order() {
    let base = PackageEntry::new("base")
        .feature(math_macros("x", &[]))
        .feature(math_macros("y", &[]))
        .feature_order(chain("z < x", "package base"));
    let top = PackageEntry::new("top")
        .uses(base)
        .feature(math_macros("z", &[]))
        .feature(math_macros("w", &[]))
        .feature_order(chain("w < y", "package top"));
    let mut builder = RegistryBuilder::new();
    builder.use_package(top);
    builder.feature(math_macros("v", &[]));
    builder.feature_order(chain("v < z", "front matter of doc.flm, line 4"));
    let registry = builder.build(&RegistryInputs::new()).expect("the registry builds");
    // Implicit order x, y, z, w, v; the chains place z before x, w before y
    // and v before z. Only w and v have no predecessor: w comes first in the
    // implicit order, then y is free and comes before v.
    assert_eq!(feature_names(&registry), ["w", "y", "v", "z", "x"]);
}

#[test]
fn without_chains_the_final_order_is_the_order_the_features_were_added() {
    let registry = built(vec![math_macros("c", &[]), math_macros("a", &[]), math_macros("b", &[])], &[]);
    assert_eq!(feature_names(&registry), ["c", "a", "b"]);
}

#[test]
fn a_chain_naming_features_not_loaded_is_ignored() {
    let registry = built(vec![math_macros("x", &[]), math_macros("y", &[])], &["nosuch < y < x", "y < misspelled"]);
    assert_eq!(feature_names(&registry), ["y", "x"]);
}

#[test]
fn a_package_used_twice_states_its_chains_once() {
    let shared = PackageEntry::new("shared")
        .feature(math_macros("a", &[]))
        .feature(math_macros("b", &[]))
        .feature_order(chain("b < a", "package shared"));
    let one = PackageEntry::new("one").uses(shared.clone());
    let two = PackageEntry::new("two").uses(shared);
    let mut builder = RegistryBuilder::new();
    builder.use_package(one).use_package(two);
    let registry = builder.build(&RegistryInputs::new()).expect("the registry builds");
    assert_eq!(feature_names(&registry), ["b", "a"]);
}

#[test]
fn a_cycle_among_loaded_features_fails_the_build_naming_the_chains_and_their_origins() {
    let package = PackageEntry::new("phfpack")
        .feature(math_macros("phfqit", &[]))
        .feature_order(chain("math-mathjax < phfqit", "package phfpack"));
    let mut builder = RegistryBuilder::new();
    builder.feature(math_macros("math-mathjax", &[]));
    builder.use_package(package);
    builder.feature_order(chain("phfqit < unloaded < math-mathjax", "front matter of doc.flm, line 4"));
    let error = match builder.build(&RegistryInputs::new()) {
        Ok(_) => panic!("a cycle is an error"),
        Err(error) => error,
    };
    assert_eq!(error.phase, BuildPhase::Collect);
    assert_eq!(error.errors.len(), 1, "{}", error.render());
    assert!(matches!(&error.errors[0], BuildError::FeatureOrderCycle { cycle, chains }
        if cycle.len() == 2 && chains.len() == 2));
    assert_eq!(
        error.render(),
        "the feature order must not contain a cycle, but the chains `math-mathjax < phfqit` \
         (package phfpack) and `phfqit < unloaded < math-mathjax` (front matter of doc.flm, line 4) \
         order the features in the cycle `math-mathjax < phfqit < math-mathjax`"
    );
}

#[test]
fn a_cycle_is_reported_with_the_other_errors_of_the_collect_phase() {
    let failing = feature("failing", |_| {
        Err(BuildError::Feature { feature: "failing".into(), detail: "cannot register".into() })
    });
    let error = match build(vec![math_macros("a", &[]), math_macros("b", &[]), failing], &["a < b", "b < a"]) {
        Ok(_) => panic!("a cycle is an error"),
        Err(error) => error,
    };
    assert_eq!(error.phase, BuildPhase::Collect);
    assert_eq!(error.errors.len(), 2, "{}", error.render());
    assert!(matches!(&error.errors[0], BuildError::FeatureOrderCycle { .. }), "{}", error.render());
    assert_eq!(error.errors[1].to_string(), "feature `failing`: cannot register");
}

#[test]
fn a_cycle_only_through_a_feature_not_loaded_is_no_error() {
    let registry = built(vec![math_macros("a", &[]), math_macros("b", &[])], &["a < x", "x < b", "b < a"]);
    assert_eq!(feature_names(&registry), ["b", "a"]);
}

// --- collisions --------------------------------------------------------------------------------

#[test]
fn with_the_chain_the_definition_of_the_feature_placed_last_applies_and_nothing_is_reported() {
    // Added with `phfqit` first; the chain places it last.
    let registry = built(
        vec![math_macros("phfqit", &["Im"]), math_macros("math-mathjax", &["Im"])],
        &["math-mathjax < phfqit"],
    );
    assert_eq!(feature_names(&registry), ["math-mathjax", "phfqit"]);
    assert_eq!(owner_of(&registry, "Im"), "phfqit");
    assert!(registry.build_diagnostics().is_empty());
    // The scope stack follows the final order: `phfqit`'s package is innermost.
    let packages: Vec<&str> = registry
        .lang()
        .providers()
        .iter()
        .map(|provider| provider.name())
        .filter(|name| ["phfqit", "math-mathjax"].contains(name))
        .collect();
    assert_eq!(packages, ["math-mathjax", "phfqit"]);
}

#[test]
fn without_a_chain_the_message_names_the_one_other_definer_and_the_fix() {
    let error = language_error(vec![math_macros("math-mathjax", &["Im"]), math_macros("phfqit", &["Im"])], &[]);
    assert_eq!(
        error,
        "Construct `\\Im` from `phfqit` clashes with `\\Im` defined in `math-mathjax` in math mode; \
         either set `flm: feature_order: ['math-mathjax < phfqit']` in your front matter to choose \
         `phfqit`'s definition, or have `phfqit` declare `\\Im` as an explicit override."
    );
}

#[test]
fn the_message_names_several_other_definers_and_one_chain_in_the_stack_order() {
    let features = vec![
        math_macros("Z", &["Im"]),
        math_macros("X", &["Im"]),
        math_macros("math-mathjax", &["Im"]),
        math_macros("phfqit", &["Im"]),
    ];
    assert_eq!(
        language_error(features, &[]),
        "Construct `\\Im` from `phfqit` clashes with `\\Im` defined in `math-mathjax`, `X`, and `Z` \
         in math mode; either set `flm: feature_order: ['Z < X < math-mathjax < phfqit']` in your \
         front matter to choose `phfqit`'s definition, or have `phfqit` declare `\\Im` as an \
         explicit override."
    );
}

#[test]
fn with_three_definers_one_chain_leaves_the_collision_an_error() {
    let features = vec![
        math_macros("userdefs", &["Im"]),
        math_macros("math-mathjax", &["Im"]),
        math_macros("phfqit", &["Im"]),
    ];
    assert_eq!(
        language_error(features, &["math-mathjax < phfqit"]),
        "Construct `\\Im` from `phfqit` clashes with `\\Im` defined in `math-mathjax` and `userdefs` \
         in math mode; either set `flm: feature_order: ['userdefs < math-mathjax < phfqit']` in your \
         front matter to choose `phfqit`'s definition, or have `phfqit` declare `\\Im` as an \
         explicit override."
    );
    // Chains that place `phfqit` after both allow it, through `math-mathjax`.
    let features = vec![
        math_macros("userdefs", &["Im"]),
        math_macros("math-mathjax", &["Im"]),
        math_macros("phfqit", &["Im"]),
    ];
    let registry = built(features, &["math-mathjax < phfqit", "userdefs < math-mathjax"]);
    assert_eq!(owner_of(&registry, "Im"), "phfqit");
}

#[test]
fn the_message_writes_an_environment_by_its_name_and_each_mode_is_reported_on_its_own() {
    let both = |name: &'static str| {
        feature(name, move |b| b.element(env_def(name, "quote", BodyDef::new()).modes([Mode::Text, Mode::Math])))
    };
    let text = |name: &'static str| feature(name, move |b| b.element(macro_def(name, "foo", 0)));
    assert_eq!(
        language_error(vec![both("a"), both("b"), text("c"), text("d")], &[]),
        "Construct `\\foo` from `d` clashes with `\\foo` defined in `c` in text mode; either set \
         `flm: feature_order: ['c < d']` in your front matter to choose `d`'s definition, or have \
         `d` declare `\\foo` as an explicit override.\n\
         Construct `quote` from `b` clashes with `quote` defined in `a` in text mode; either set \
         `flm: feature_order: ['a < b']` in your front matter to choose `b`'s definition, or have \
         `b` declare `quote` as an explicit override.\n\
         Construct `quote` from `b` clashes with `quote` defined in `a` in math mode; either set \
         `flm: feature_order: ['a < b']` in your front matter to choose `b`'s definition, or have \
         `b` declare `quote` as an explicit override."
    );
}

#[test]
fn each_mode_is_decided_with_the_features_that_define_the_name_in_it() {
    // `a` defines `\Im` in both modes, `b` in text mode only, `c` in math mode
    // only: `b` and `c` never meet, so the chains need not order them.
    let both = |name: &'static str| {
        feature(name, move |b| b.element(macro_def(name, "Im", 0).modes([Mode::Text, Mode::Math])))
    };
    let text = |name: &'static str| feature(name, move |b| b.element(macro_def(name, "Im", 0)));
    let features = || vec![both("a"), text("b"), math_macros("c", &["Im"])];
    let registry = built(features(), &["a < b", "a < c"]);
    assert!(registry.build_diagnostics().is_empty());
    // Without the chain for math mode, that mode alone is reported.
    assert_eq!(
        language_error(features(), &["a < b"]),
        "Construct `\\Im` from `c` clashes with `\\Im` defined in `a` in math mode; either set \
         `flm: feature_order: ['a < c']` in your front matter to choose `c`'s definition, or have \
         `c` declare `\\Im` as an explicit override."
    );
}

#[test]
fn an_override_by_the_feature_placed_last_is_still_a_silent_override() {
    let registry = built(vec![math_macros("math-mathjax", &["Im"]), overriding_im("phfqit")], &[]);
    assert_eq!(owner_of(&registry, "Im"), "phfqit");
    assert!(registry.build_diagnostics().is_empty());
    // The chain may place the overriding feature last.
    let registry = built(vec![overriding_im("phfqit"), math_macros("math-mathjax", &["Im"])], &["math-mathjax < phfqit"]);
    assert_eq!(owner_of(&registry, "Im"), "phfqit");
}

#[test]
fn the_override_of_a_feature_not_placed_last_is_not_consulted() {
    // `phfqit` declares an override but is placed first: only the override of
    // the definition that applies counts, so this is a collision as any other.
    let error = language_error(vec![overriding_im("phfqit"), math_macros("math-mathjax", &["Im"])], &[]);
    assert_eq!(
        error,
        "Construct `\\Im` from `math-mathjax` clashes with `\\Im` defined in `phfqit` in math mode; \
         either set `flm: feature_order: ['phfqit < math-mathjax']` in your front matter to choose \
         `math-mathjax`'s definition, or have `math-mathjax` declare `\\Im` as an explicit override."
    );
    // A chain that places the overriding feature first allows the collision:
    // `math-mathjax`'s definition applies and nothing is reported.
    let registry = built(
        vec![math_macros("math-mathjax", &["Im"]), overriding_im("phfqit")],
        &["phfqit < math-mathjax"],
    );
    assert_eq!(owner_of(&registry, "Im"), "math-mathjax");
    assert!(registry.build_diagnostics().is_empty());
}
