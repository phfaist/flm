//! The package in a registry, the crate's manifest, and the macros in running
//! text.

mod common;

use std::sync::Arc;

use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, RegistryBuildError, RegistryBuilder,
    RegistryInputs,
};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::mint::{ElementDef, MintError, SyntaxDef};
use flm_core::lang::LangBuildError;
use flm_core::model::element::ElementSpec;
use flm_core::techy::latexlike::Mode;
use flm_core::stdlib::{self, StdlibConfig};

use common::{factory, html_of, latex_of, package, registry, run, text_of, FEATURE, PACKAGE};

#[test]
fn a_registry_holding_the_standard_package_and_phfpack_builds_without_a_diagnostic() {
    let registry = registry();
    assert!(registry.has_feature(FEATURE));
    let diagnostics: Vec<String> =
        registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn a_registry_holding_phfqit_without_userdefs_does_not_build() {
    let mut settings = PackageSettings::new();
    settings.switch_off("userdefs");
    let standard = stdlib::add_features_from_settings(PackageEntry::new("std"), &settings);
    let mut builder = RegistryBuilder::new();
    builder.use_package(standard);
    builder.use_package(package());
    let error = builder.build(&RegistryInputs::new()).expect_err("phfqit needs userdefs");
    let undeclared = error.errors.iter().any(|error| {
        matches!(error, BuildError::Undeclared { name, .. } if name.as_str() == "flm.userdefs.always-expanded")
    });
    assert!(undeclared, "{error}");
}

#[test]
fn a_document_that_uses_no_macro_of_the_package_renders_as_without_it() {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    let without = builder.build(&RegistryInputs::new()).expect("the registry builds");
    let with = registry();
    let document = "A `quoted' text, \\(x+y\\), and \\(\\alpha_1\\).\n\n\\section{Two}\nMore.";
    for render in [text_of, html_of, latex_of] {
        assert_eq!(render(&without, document), render(&with, document));
    }
}

#[test]
fn the_factory_names_the_package_and_its_feature_and_no_package_it_uses() {
    let factory = factory();
    assert_eq!(factory.name().as_str(), PACKAGE);
    assert!(factory.used().is_empty());
    assert_eq!(factory.features().len(), 1);
    assert_eq!(factory.features()[0].name, FEATURE);
    assert!(factory.settings(FEATURE).is_some());
}

// --- running text --------------------------------------------------------------------------

#[test]
fn in_running_text_a_backtick_is_a_quotation_mark_as_without_the_package() {
    let (text, diagnostics) = text_of(&registry(), "A `quoted' word.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A \u{2018}quoted\u{2019} word.\n");
}

#[test]
fn in_running_text_ident_and_bit_mean_what_their_replacement_means() {
    let (text, diagnostics) = text_of(&registry(), r"One \Ident{} and \bit{0}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "One \u{1D7D9} and 0.\n");
}

#[test]
fn in_running_text_a_macro_whose_replacement_is_not_valid_there_is_an_error_at_the_use_and_the_document_renders() {
    let source = r"A \ket{x} and \(\ket{y}\).";
    let (text, diagnostics) = text_of(&registry(), source);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    let run = run(&registry(), flm_core::stdlib::backends::text::WORKFLOW, source);
    flm_core::dump::diag::expect(run.diagnostics().iter(), "flm.userdefs.bad-definition")
        .at(r"\ket{x}")
        .data(serde_json::json!({ "name": r"\ket", "problem": { "kind": "not-in-force-in-mode", "mode": "text" } }))
        .once();
    // The use in the formula still gives its replacement, and the use in
    // running text stands as written.
    assert!(text.contains(r"\ket{x}"), "{text}");
    assert!(text.contains(r"\lvert{y}\rangle"), "{text}");
}

// --- the manifest --------------------------------------------------------------------------

/// The dependencies the manifest's table `[dependencies]` declares: each
/// one's name and the text after the `=`.
fn dependencies(manifest: &str) -> Vec<(String, String)> {
    let mut found = Vec::new();
    let mut in_dependencies = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_dependencies = line == "[dependencies]";
            continue;
        }
        if !in_dependencies || line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((name, value)) = line.split_once('=') {
            found.push((name.trim().to_string(), value.trim().to_string()));
        }
    }
    found
}

/// The members of the workspace, by the folder names its manifest lists.
fn workspace_members(manifest: &str) -> Vec<String> {
    let line = manifest.lines().find(|line| line.trim_start().starts_with("members")).unwrap_or("");
    line.split('"').skip(1).step_by(2).map(String::from).collect()
}

/// The cargo features that the feature `name` of the manifest turns on, as written
/// between its brackets.
fn feature(manifest: &str, name: &str) -> Vec<String> {
    let line = manifest.lines().find(|line| line.split_once('=').is_some_and(|(key, _)| key.trim() == name));
    let line = line.unwrap_or_else(|| panic!("the manifest has the feature `{name}`"));
    line.split('"').skip(1).step_by(2).map(String::from).collect()
}

#[test]
fn the_manifest_names_no_crate_of_the_workspace_but_the_guest_client_and_not_the_toolkit() {
    // The extension is written against the guest client alone: its feature
    // turns on `flm-ext`'s feature `guest` and no other, and the crate
    // depends on `flm-ext` without features. The tests' own dependencies
    // (`[dev-dependencies]`) are not the extension's.
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("the manifest");
    let workspace = std::fs::read_to_string(root.join("../Cargo.toml")).expect("the workspace");
    let members = workspace_members(&workspace);
    assert!(members.iter().any(|m| m == "flm-ext"), "{members:?}");
    let found = dependencies(&manifest);
    let guest = found.iter().find(|(name, _)| name == "flm-ext").map(|(_, value)| value.as_str());
    let guest = guest.unwrap_or_else(|| panic!("the crate depends on `flm-ext`: {found:?}"));
    assert!(guest.contains("default-features = false") && !guest.contains("features = ["), "{guest}");
    let turned_on = feature(&manifest, "phfpack");
    let flm_ext: Vec<&String> = turned_on.iter().filter(|name| name.starts_with("flm-ext/")).collect();
    assert_eq!(flm_ext, ["flm-ext/guest"], "{turned_on:?}");
    for name in turned_on.iter().filter_map(|name| name.strip_prefix("dep:")) {
        let other_member = members.iter().any(|m| m == name) && name != "flm-ext";
        assert!(!other_member, "the extension depends on the workspace's `{name}`");
        assert!(!name.starts_with("techy"), "the extension names the toolkit `{name}`");
    }
}

#[test]
fn the_package_entry_holds_the_one_feature() {
    let entry = package();
    assert_eq!(entry.name.as_str(), PACKAGE);
    let names: Vec<String> = entry.features.iter().map(|f| f.info().name.to_string()).collect();
    assert_eq!(names, [FEATURE]);
}

/// A stand-in for the feature `math-mathjax` of the extension that processes
/// mathematics with MathJax: it defines `\Im` in math mode, as MathJax does.
struct MathJaxStandIn;

impl FlmFeature for MathJaxStandIn {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("math-mathjax", "A stand-in for math-mathjax")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let element = Arc::new(ElementSpec::new("math-mathjax.Im"));
        b.element(ElementDef::new("math-mathjax", ["Im"], SyntaxDef::Macro { args: Vec::new() }, element).modes([Mode::Math]))
    }
}

/// Builds the standard package, `math-passthrough`, the stand-in for
/// `math-mathjax` and then `package`, as the initial list `std`, the math
/// processor `mathjax` and `use: [phfpack]` register them.
fn with_mathjax(
    package: flm_core::core::registry::PackageEntry,
) -> Result<Arc<flm_core::core::registry::FlmRegistry>, RegistryBuildError> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(Arc::new(MathJaxStandIn));
    builder.use_package(package);
    builder.build(&RegistryInputs::new())
}

#[test]
fn the_package_places_phfqit_after_math_mathjax() {
    let entry = package();
    assert_eq!(entry.feature_order.len(), 1);
    let names: Vec<&str> = entry.feature_order[0].names().iter().map(|name| name.as_str()).collect();
    assert_eq!(names, ["math-mathjax", "phfqit"]);
    assert_eq!(entry.feature_order[0].origin(), format!("package {PACKAGE}"));
    // The factory builds the same chain without any settings.
    let built = factory().build(&PackageSettings::new()).expect("the package");
    assert_eq!(built.feature_order.len(), 1);
    // With `math-mathjax` loaded, `\Im` of both is allowed: `phfqit`'s applies.
    assert!(with_mathjax(entry.clone()).is_ok());
    // Without the chain, the two definitions of `\Im` clash.
    let mut bare = entry;
    bare.feature_order.clear();
    let error = with_mathjax(bare).expect_err("the definitions clash");
    let clash = error.errors.iter().any(|error| {
        matches!(error, BuildError::Lang(LangBuildError::Mint(MintError::Clash { symbol, modes, owners, .. }))
            if symbol.name == "Im"
                && modes.as_slice() == [Mode::Math]
                && owners.iter().any(|owner| owner.as_str() == "phfqit")
                && owners.iter().any(|owner| owner.as_str() == "math-mathjax"))
    });
    assert!(clash, "{}", error.render());
}
