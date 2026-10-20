//! The package in a registry (acceptance test 6), the crate's manifest
//! (test 5), and the macros in running text (test 10).

mod common;

use std::sync::Arc;

use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, RegistryBuilder, RegistryInputs};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::mint::{ElementDef, SyntaxDef};
use flm_core::model::element::ElementSpec;
use flm_core::techy::latexlike::Mode;
use flm_core::stdcore::{self, StdcoreConfig};
use flm_phfpack::settings::PhfqitConfig;

use common::{html_of, latex_of, registry, text_of};

#[test]
fn a_registry_holding_the_standard_package_and_phfpack_builds_without_a_diagnostic() {
    let registry = registry();
    assert!(registry.has_feature(flm_phfpack::FEATURE));
    let diagnostics: Vec<String> =
        registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn a_registry_holding_phfqit_without_userdefs_does_not_build() {
    let mut settings = PackageSettings::new();
    settings.switch_off("userdefs");
    let standard = stdcore::factory().expect("the factory").build(&settings).expect("the package");
    let mut builder = RegistryBuilder::new();
    builder.use_package(standard);
    builder.use_package(flm_phfpack::package(PhfqitConfig::default()));
    let error = builder.build(&RegistryInputs::new()).expect_err("phfqit needs userdefs");
    assert!(error.to_string().contains("flm.userdefs.always-expanded"), "{error}");
}

#[test]
fn a_document_that_uses_no_macro_of_the_package_renders_as_without_it() {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    let without = builder.build(&RegistryInputs::new()).expect("the registry builds");
    let with = registry();
    let document = "A `quoted' text, \\(x+y\\), and \\(\\alpha_1\\).\n\n\\section{Two}\nMore.";
    for render in [text_of, html_of, latex_of] {
        assert_eq!(render(&without, document), render(&with, document));
    }
}

#[test]
fn the_factory_names_the_package_its_feature_and_the_package_it_uses() {
    let factory = flm_phfpack::factory().expect("the factory");
    assert_eq!(factory.name().as_str(), flm_phfpack::PACKAGE);
    assert_eq!(factory.used().iter().map(|n| n.as_str()).collect::<Vec<_>>(), [stdcore::PACKAGE]);
    assert_eq!(factory.features().len(), 1);
    assert_eq!(factory.features()[0].name, flm_phfpack::FEATURE);
    assert!(factory.settings(flm_phfpack::FEATURE).is_some());
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
    let (text, diagnostics) = text_of(&registry(), r"A \ket{x} and \(\ket{y}\).");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].starts_with("flm.userdefs.bad-definition"), "{diagnostics:?}");
    assert!(diagnostics[0].contains(r"\ket"), "{diagnostics:?}");
    // The use in the formula still gives its replacement, and the use in
    // running text stands as written.
    assert!(text.contains(r"\ket{x}"), "{text}");
    assert!(text.contains(r"\lvert{y}\rangle"), "{text}");
}

// --- the manifest --------------------------------------------------------------------------

/// The names of the dependencies a manifest declares, in every table of
/// dependencies.
fn dependencies(manifest: &str) -> Vec<String> {
    let mut names = Vec::new();
    let mut in_dependencies = false;
    for line in manifest.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_dependencies = line.ends_with("dependencies]");
            continue;
        }
        if in_dependencies && !line.is_empty() && !line.starts_with('#') {
            if let Some((name, _)) = line.split_once('=') {
                names.push(name.trim().to_string());
            }
        }
    }
    names
}

/// The members of the workspace, by the folder names its manifest lists.
fn workspace_members(manifest: &str) -> Vec<String> {
    let line = manifest.lines().find(|line| line.trim_start().starts_with("members")).unwrap_or("");
    line.split('"').skip(1).step_by(2).map(String::from).collect()
}

#[test]
fn the_manifest_names_no_crate_of_the_workspace_but_the_library_and_not_the_toolkit() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest = std::fs::read_to_string(root.join("Cargo.toml")).expect("the manifest");
    let workspace = std::fs::read_to_string(root.join("../Cargo.toml")).expect("the workspace");
    let members = workspace_members(&workspace);
    assert!(members.iter().any(|m| m == "flm-core"), "{members:?}");
    let names = dependencies(&manifest);
    assert!(names.iter().any(|n| n == "flm-core"), "{names:?}");
    for name in &names {
        let other_member = members.iter().any(|m| m == name) && name != "flm-core";
        assert!(!other_member, "the crate depends on the workspace's `{name}`");
        assert!(!name.starts_with("techy"), "the crate names the toolkit `{name}`");
    }
}

#[test]
fn the_package_entry_holds_the_one_feature() {
    let entry = flm_phfpack::package(PhfqitConfig::default());
    assert_eq!(entry.name.as_str(), flm_phfpack::PACKAGE);
    let names: Vec<String> = entry.features.iter().map(|f| f.info().name.to_string()).collect();
    assert_eq!(names, [flm_phfpack::FEATURE]);
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
fn with_mathjax(package: flm_core::core::registry::PackageEntry) -> Result<Arc<flm_core::core::registry::FlmRegistry>, String> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(Arc::new(MathJaxStandIn));
    builder.use_package(package);
    builder.build(&RegistryInputs::new()).map_err(|error| error.to_string())
}

#[test]
fn the_package_places_phfqit_after_math_mathjax() {
    let entry = flm_phfpack::package(PhfqitConfig::default());
    assert_eq!(entry.feature_order.len(), 1);
    let names: Vec<&str> = entry.feature_order[0].names().iter().map(|name| name.as_str()).collect();
    assert_eq!(names, ["math-mathjax", "phfqit"]);
    assert_eq!(entry.feature_order[0].origin(), "package phfpack");
    // The factory builds the same entry.
    let built = flm_phfpack::factory().expect("the factory").build(&PackageSettings::new()).expect("the package");
    assert_eq!(built.feature_order.len(), 1);
    // With `math-mathjax` loaded, `\Im` of both is allowed: `phfqit`'s applies.
    assert!(with_mathjax(entry.clone()).is_ok());
    // Without the chain, the two definitions of `\Im` clash.
    let mut bare = entry;
    bare.feature_order.clear();
    let error = with_mathjax(bare).expect_err("the definitions clash");
    assert!(
        error.contains("Construct `\\Im` from `phfqit` clashes with `\\Im` defined in `math-mathjax` in math mode"),
        "{error}"
    );
}
