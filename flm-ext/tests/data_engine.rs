//! The data engine: an extension written as data provides one
//! package, `_`, whose block is its manifest's `flm` and whose templates are
//! its `templates/` folder.

mod common;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use flm_core::core::registry::{BuildError, PackageEntry, PackageResolver, ResolveError};
use flm_core::lang::mint::MintError;
use flm_core::lang::LangBuildError;
use flm_core::core::settings::PackageSettings;
use flm_core::stdlib::features::compose::{
    chosen_template, ComposeFeature, ComposeOptions, Template, TemplateArtifact, TemplateChoice, TEMPLATES_SLOT,
};
use flm_ext::api::core::registry::FeatureBuilder;
use flm_ext::api::lang::mint::{ElementDef, SyntaxDef};
use flm_ext::api::model::element::ElementSpec;
use flm_ext::block::FeatureBlock;
use flm_ext::source::SourceError;
use flm_ext::{ExtensionLoader, ExtensionSource, FoundExtension, Locator, MemoryFiles, MemorySource, UseEntry};
use serde_json::{json, Value};

use common::double::Double;
use common::{catalog_with, memory_loader};

const ID: &str = "_flmextensions/myext/flmextension.yaml";

fn loader_of(manifest: Value, files: MemoryFiles) -> Arc<ExtensionLoader> {
    memory_loader(MemorySource::new().with("myext", ID, manifest, files))
}

fn details(errors: Vec<ResolveError>) -> Vec<String> {
    errors.into_iter().map(|error| error.detail).collect()
}

/// The description of the content-only template `name` for the output `html`.
fn description(name: &str) -> Value {
    json!({ "name": name, "output": "html", "engine": "content-only", "accepts": "all" })
}

#[test]
fn the_data_engine_provides_one_package_whose_block_is_flm() {
    let manifest = json!({ "flm": {
        "use": "phfpack",
        "features": { "toc": false, "text": { "width": 60 } },
        "parsing": { "dollar_math": true },
        "workflows": { "draft": { "extends": "html" } },
    } });
    let loader = loader_of(manifest, MemoryFiles::new());
    let factory = loader.resolve("myext").unwrap().unwrap();
    assert_eq!(factory.name().as_str(), format!("{ID}#_"));
    // No template, no scripted feature: a package of settings with no feature.
    assert!(factory.features().is_empty());
    assert!(matches!(loader.resolve("myext#core"), Err(errors) if errors.len() == 1));
    let blocks = loader.blocks();
    assert_eq!(blocks.len(), 1);
    let block = &blocks[0].block;
    assert_eq!(block.uses, [UseEntry::parse("phfpack").unwrap()]);
    assert_eq!(block.features[0], ("toc".to_string(), FeatureBlock::off()));
    assert_eq!(block.parsing.as_ref().map(|p| p.len()), Some(1));
    assert_eq!(block.workflows[0].name, "draft");
}

/// A source over `inner` that counts how often each locator is asked.
struct Counting {
    inner: MemorySource,
    asked: Mutex<BTreeMap<String, usize>>,
    total: AtomicUsize,
}

impl ExtensionSource for Counting {
    fn find(&self, locator: &Locator) -> Result<Option<FoundExtension>, SourceError> {
        self.total.fetch_add(1, Ordering::Relaxed);
        *self.asked.lock().unwrap().entry(locator.to_string()).or_default() += 1;
        self.inner.find(locator)
    }
}

#[test]
fn its_uses_are_resolved_and_each_extension_read_once_loops_included() {
    let inner = MemorySource::new()
        .with("a", "a.yaml", json!({ "flm": { "use": ["b", "std"] } }), MemoryFiles::new())
        .with("b", "b.yaml", json!({ "flm": { "use": "a#_" } }), MemoryFiles::new());
    let source = Arc::new(Counting { inner, asked: Mutex::new(BTreeMap::new()), total: AtomicUsize::new(0) });
    let loader = Arc::new(ExtensionLoader::new(vec![Arc::clone(&source) as Arc<dyn ExtensionSource>]));
    let catalog = catalog_with(&loader);
    let found = catalog.resolve_all(&["a", "b", "a"]).expect("resolved");
    let names: Vec<&str> = found.iter().map(|f| f.name().as_str()).collect();
    assert_eq!(names, ["b.yaml#_", "std", "a.yaml#_"]);
    // Asked again (the reach, the schema): answered from the loader alone.
    catalog.resolve_all(&["a", "b"]).expect("resolved");
    assert_eq!(*source.asked.lock().unwrap(), BTreeMap::from([("a".to_string(), 1), ("b".to_string(), 1)]));
    assert_eq!(source.total.load(Ordering::Relaxed), 2);
}

#[test]
fn a_compiled_name_in_its_use_list_reaches_the_compiled_package() {
    let loader = loader_of(json!({ "flm": { "use": ["std"] } }), MemoryFiles::new());
    let catalog = catalog_with(&loader);
    let factory = catalog.resolve("myext").unwrap().unwrap();
    assert_eq!(factory.used().iter().map(|n| n.as_str()).collect::<Vec<_>>(), ["std"]);
    let found = catalog.resolve_all(&["myext"]).expect("resolved");
    assert_eq!(found[0].name().as_str(), "std");
}

#[test]
fn its_templates_reach_the_compose_slot_and_are_chosen_by_name() {
    let mut fancy = description("fancy");
    fancy["main"] = json!("fancy.html");
    let files = MemoryFiles::new()
        .with_value("templates/html/fancy/flmtemplate.yaml", fancy)
        .with_text("templates/html/fancy/fancy.html", "<main>{{ content }}</main>")
        .with_value("templates/html/bare.yaml", description("bare"))
        // Neither a folder without a description nor a file not ending in `.yaml` is a template.
        .with_text("templates/html/partials/head.yaml", "not a description")
        .with_text("templates/html/bare.yaml~", "not a description");
    let loader = loader_of(json!({ "extension": { "description": "Fancy pages." } }), files);
    let factory = loader.resolve("myext").unwrap().unwrap();
    let package = factory.build(&PackageSettings::new()).expect("the package");
    let compose = PackageEntry::new("compose").feature(Arc::new(ComposeFeature::new()));
    let registry = common::registry_with([compose, package]);
    let templates: Vec<&Template> = registry.services().services_named::<Template>(TEMPLATES_SLOT).collect();
    let mut names: Vec<(&str, &str)> = templates.iter().map(|t| (t.name.as_str(), t.package.as_str())).collect();
    names.sort();
    let package_name = format!("{ID}#_");
    assert_eq!(names, [("bare", package_name.as_str()), ("fancy", package_name.as_str())]);
    let options = ComposeOptions { template: TemplateChoice::Name("fancy".to_string()), ..ComposeOptions::default() };
    let chosen = chosen_template(&registry, &options, "html").expect("chosen");
    assert_eq!((chosen.name.as_str(), chosen.text.as_str()), ("fancy", "<main>{{ content }}</main>"));
    let bare = ComposeOptions { template: TemplateChoice::Name("bare".to_string()), ..ComposeOptions::default() };
    assert_eq!(chosen_template(&registry, &bare, "html").expect("chosen").text, "");
}

#[test]
fn a_file_of_the_templates_folder_is_left_out_and_the_templates_load() {
    let files = MemoryFiles::new()
        .with_text("templates/readme.txt", "The templates.")
        .with_value("templates/html/fancy.yaml", description("fancy"));
    let loader = loader_of(Value::Null, files);
    let factory = loader.resolve("myext").expect("the load").expect("found");
    let package = factory.build(&PackageSettings::new()).expect("the package");
    let compose = PackageEntry::new("compose").feature(Arc::new(ComposeFeature::new()));
    let registry = common::registry_with([compose, package]);
    let names: Vec<&str> =
        registry.services().services_named::<Template>(TEMPLATES_SLOT).map(|template| template.name.as_str()).collect();
    assert_eq!(names, ["fancy"]);
}

/// The compose template `t` of a data extension whose description
/// `templates/html/t/flmtemplate.yaml` lists the artifact `icons` described by `icons`,
/// with the files `files` of the extension.
fn shipping_folder(icons: Value, files: MemoryFiles) -> Result<Template, Vec<String>> {
    let description =
        json!({ "name": "t", "output": "html", "engine": "content-only", "accepts": "all", "artifacts": { "icons": icons } });
    let loader = loader_of(Value::Null, files.with_value("templates/html/t/flmtemplate.yaml", description));
    let factory = loader.resolve("myext").map_err(details)?.expect("found");
    let package = factory.build(&PackageSettings::new()).expect("the package");
    let compose = PackageEntry::new("compose").feature(Arc::new(ComposeFeature::new()));
    let registry = common::registry_with([compose, package]);
    let options = ComposeOptions { template: TemplateChoice::Name("t".to_string()), ..ComposeOptions::default() };
    Ok(chosen_template(&registry, &options, "html").expect("chosen").clone())
}

#[test]
fn a_shipped_folder_is_listed_at_every_depth_without_the_names_beginning_with_a_dot() {
    let files = MemoryFiles::new()
        .with_bytes("templates/html/t/icons/check.svg", b"<svg/>")
        .with_bytes("templates/html/t/icons/.DS_Store", b"x")
        .with_bytes("templates/html/t/icons/sub/x.png", &[0x89, b'P'])
        .with_bytes("templates/html/t/icons/sub/.hidden/y.svg", b"y")
        .with_text("templates/html/t/icons/a.txt", "text")
        .with_bytes("templates/html/t/elsewhere.svg", b"no");
    let template = shipping_folder(json!({ "dir": "icons" }), files).expect("the template");
    assert_eq!(
        template.artifacts,
        [TemplateArtifact::Folder {
            name: String::from("icons"),
            files: vec![
                (String::from("a.txt"), b"text".to_vec()),
                (String::from("check.svg"), b"<svg/>".to_vec()),
                (String::from("sub/x.png"), vec![0x89, b'P']),
            ],
        }]
    );
    // A folder named by a path that climbs, inside the extension.
    let files = MemoryFiles::new().with_bytes("assets/icons/check.svg", b"<svg/>");
    let template = shipping_folder(json!({ "dir": "../../../assets/icons" }), files).expect("the template");
    assert_eq!(template.artifacts[0], TemplateArtifact::Folder { name: String::from("icons"), files: vec![(String::from("check.svg"), b"<svg/>".to_vec())] });
}

#[test]
fn a_shipped_folder_that_is_missing_empty_or_outside_the_extension_fails_the_load() {
    // Each case is one problem of the load.
    let refused = |icons: Value, files: MemoryFiles| shipping_folder(icons, files).expect_err("the load fails").len();
    assert_eq!(refused(json!({ "dir": "icons" }), MemoryFiles::new()), 1);
    // A file is not a folder.
    assert_eq!(refused(json!({ "dir": "icons" }), MemoryFiles::new().with_bytes("templates/html/t/icons", b"x")), 1);
    let hidden_only = MemoryFiles::new()
        .with_bytes("templates/html/t/icons/.DS_Store", b"x")
        .with_bytes("templates/html/t/icons/.git/HEAD", b"x");
    assert_eq!(refused(json!({ "dir": "icons" }), hidden_only), 1);
    assert_eq!(refused(json!({ "dir": "../../../../icons" }), MemoryFiles::new()), 1);
    assert_eq!(refused(json!({ "dir": "/icons" }), MemoryFiles::new()), 1);
    assert_eq!(refused(json!({ "dir": "icons", "file": "x.svg" }), MemoryFiles::new()), 1);
    assert_eq!(refused(json!({ "dir": "icons", "template": true }), MemoryFiles::new()), 1);
}

#[test]
fn two_templates_of_one_name_and_output_in_one_package_fail_the_load() {
    let files = MemoryFiles::new()
        .with_value("templates/html/fancy.yaml", description("fancy"))
        .with_value("templates/html/plain/flmtemplate.yaml", description("fancy"));
    loader_of(Value::Null, files).resolve("myext").expect_err("two templates of one name");
}

#[test]
fn layout_in_an_extension_is_refused_as_every_key_the_block_does_not_read() {
    let loader = loader_of(json!({ "flm": { "layout": "inline", "default_profile": "html", "docs": "x" } }), MemoryFiles::new());
    let errors = loader.resolve("myext").expect_err("refused");
    assert!(errors.iter().all(|error| error.file.as_deref() == Some(ID)));
    assert_eq!(errors.len(), 3, "{errors:?}");
}

#[test]
fn the_chains_of_feature_order_reach_the_package_entry_whatever_the_settings_switch_off() {
    let manifest = json!({ "flm": { "feature_order": ["a < b", "headings < x < toc"], "features": { "a": false } } });
    let files = MemoryFiles::new().with_value("templates/html/fancy.yaml", description("fancy"));
    let factory = loader_of(manifest, files).resolve("myext").unwrap().unwrap();
    let mut settings = PackageSettings::new();
    settings.switch_off("a");
    settings.switch_off(format!("{ID}#_").as_str());
    let package = factory.build(&settings).expect("the package");
    // Its one feature, the templates', is switched off; the chains stay.
    assert!(package.features.is_empty());
    let chains: Vec<(String, String)> =
        package.feature_order.iter().map(|chain| (chain.to_string(), chain.origin().to_string())).collect();
    let origin = format!("package {ID}#_");
    assert_eq!(chains, [("a < b".to_string(), origin.clone()), ("headings < x < toc".to_string(), origin)]);
}

#[test]
fn a_chain_that_is_not_one_fails_the_load_at_its_key() {
    let loader = loader_of(json!({ "flm": { "feature_order": ["a < b", "a, b"] } }), MemoryFiles::new());
    let errors = loader.resolve("myext").expect_err("refused");
    assert_eq!(errors[0].file.as_deref(), Some(ID));
    assert_eq!(errors.len(), 1, "{errors:?}");
}

/// The package of the extension `dbl` (a double), whose features `a` and `b`
/// both define the macro `\foo`, in this order.
fn two_definers_of_foo() -> PackageEntry {
    let defining = |element: &'static str| {
        move |b: &mut dyn FeatureBuilder| {
            b.element(ElementDef::new(b.feature(), ["foo"], SyntaxDef::Macro { args: Vec::new() }, ElementSpec::new(element)))
        }
    };
    Double::new("dbl").feature("a", defining("dbl.a")).feature("b", defining("dbl.b")).package()
}

#[test]
fn a_chain_of_a_data_extension_orders_two_features_and_allows_their_collision() {
    let ordering = |manifest: Value| loader_of(manifest, MemoryFiles::new()).resolve("myext").unwrap().unwrap();
    let without = ordering(Value::Null).build(&PackageSettings::new()).expect("the package");
    let error = common::try_registry_with([two_definers_of_foo(), without]).expect_err("the collision is an error");
    assert!(
        error.errors.iter().any(|e| matches!(e, BuildError::Lang(LangBuildError::Mint(MintError::Clash { symbol, owners, .. }))
            if symbol.name == "foo" && owners.iter().map(|owner| owner.as_str()).collect::<Vec<_>>() == ["a", "b"])),
        "{error:?}"
    );
    let with = ordering(json!({ "flm": { "feature_order": "a < b" } })).build(&PackageSettings::new()).expect("the package");
    let registry = common::try_registry_with([two_definers_of_foo(), with]).unwrap_or_else(|error| panic!("{error}"));
    assert!(registry.build_diagnostics().is_empty());
}

#[test]
fn the_description_is_the_templates_feature_s_title() {
    let files = MemoryFiles::new().with_value("templates/html/fancy.yaml", description("fancy"));
    let loader = loader_of(json!({ "extension": { "description": "Fancy pages." } }), files);
    let factory = loader.resolve("myext").unwrap().unwrap();
    let package = factory.build(&PackageSettings::new()).expect("the package");
    let infos: Vec<(String, String)> =
        package.features.iter().map(|f| (f.info().name.to_string(), f.info().title.clone())).collect();
    assert_eq!(infos, [(format!("{ID}#_"), "Fancy pages.".to_string())]);
    // Switched off by its name, it is left out.
    let mut settings = PackageSettings::new();
    settings.switch_off(format!("{ID}#_").as_str());
    assert!(factory.build(&settings).expect("the package").features.is_empty());
}

#[test]
fn a_block_problem_leaves_the_refusal_of_the_other_keys_in_place() {
    let loader = loader_of(json!({ "flm": { "features": 3, "layout": "blocks", "bogus": 1 } }), MemoryFiles::new());
    let errors = loader.resolve("myext").expect_err("refused");
    let paths: Vec<Option<String>> = errors.iter().map(|error| error.file.clone()).collect();
    assert!(paths.iter().all(|file| file.as_deref() == Some(ID)), "{paths:?}");
    let details = details(errors);
    assert_eq!(details.len(), 3, "{details:?}");
}

/// The registry digest of a registry holding the package of a data
/// extension whose manifest is `manifest` and whose files are `files`.
fn registry_digest(manifest: Value, files: MemoryFiles) -> String {
    let factory = loader_of(manifest, files).resolve("myext").unwrap().unwrap();
    let registry = common::registry_with([factory.build(&PackageSettings::new()).expect("the package")]);
    format!("{:?}", registry.digest())
}

#[test]
fn a_template_and_the_manifest_count_in_the_registry_digest() {
    let template = json!({ "name": "fancy", "output": "html", "engine": "minijinja", "main": "fancy.html", "accepts": "all" });
    let files = |text: &str| {
        MemoryFiles::new()
            .with_value("templates/html/fancy/flmtemplate.yaml", template.clone())
            .with_text("templates/html/fancy/fancy.html", text)
    };
    let manifest = |description: &str| json!({ "extension": { "description": description } });
    let base = registry_digest(manifest("a"), files("one"));
    assert_eq!(base, registry_digest(manifest("a"), files("one")));
    assert_ne!(base, registry_digest(manifest("a"), files("two")));
    assert_ne!(base, registry_digest(manifest("b"), files("one")));
    let other = MemoryFiles::new()
        .with_value(
            "templates/html/fancy/flmtemplate.yaml",
            json!({ "name": "fancy", "output": "html", "engine": "minijinja", "main": "fancy.html", "accepts": ["body"] }),
        )
        .with_text("templates/html/fancy/fancy.html", "one");
    assert_ne!(base, registry_digest(manifest("a"), other));
}

#[test]
fn a_template_states_the_place_of_its_text_under_the_root_of_its_extension() {
    let page = "{% include \"partials/head.html\" %}{{ include_raw(\"../../../dist/x.js\") }}";
    let files = MemoryFiles::new()
        .with_value(
            "templates/html/t/flmtemplate.yaml",
            json!({ "name": "t", "output": "html", "engine": "minijinja", "main": "t.html", "accepts": "all" }),
        )
        .with_text("templates/html/t/t.html", page)
        .with_text("templates/html/t/partials/head.html", "<head>{{ regions.title }}</head>")
        .with_text("dist/x.js", "let a = \"{{ x }}\";");
    let loader = loader_of(Value::Null, files);
    let factory = loader.resolve("myext").unwrap().expect("found");
    let package = factory.build(&PackageSettings::new()).expect("the package");
    let compose = PackageEntry::new("compose").feature(Arc::new(ComposeFeature::new()));
    let registry = common::registry_with([compose, package]);
    let options = ComposeOptions { template: TemplateChoice::Name("t".to_string()), ..ComposeOptions::default() };
    let template = chosen_template(&registry, &options, "html").expect("chosen");
    // The place of the text, under the root of the use name that reached the extension, from
    // which a template engine reads the files the text includes.
    assert_eq!(template.text, page);
    assert_eq!(template.source.as_deref(), Some("ext+myext:templates/html/t/t.html"));
}
