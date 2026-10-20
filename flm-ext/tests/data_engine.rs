//! The data engine (plan A, 3.8): an extension written as data provides one
//! package, `_`, whose block is its manifest's `flm` and whose templates are
//! its `templates/` folder.

mod common;

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use flm_core::core::registry::{PackageEntry, PackageResolver, ResolveError};
use flm_core::core::settings::PackageSettings;
use flm_core::stdcore::features::compose::{
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

/// A content-only template description for the output `html`.
fn description() -> Value {
    json!({ "output": "html", "engine": "content-only", "accepts": "all" })
}

#[test]
fn the_data_engine_provides_one_package_whose_block_is_flm() {
    let manifest = json!({ "flm": {
        "use": "stdcore",
        "features": { "toc": false, "text": { "width": 60 } },
        "parsing": { "dollar_math": true },
        "workflows": { "draft": { "extends": "html" } },
    } });
    let loader = loader_of(manifest, MemoryFiles::new());
    let factory = loader.resolve("myext").unwrap().unwrap();
    assert_eq!(factory.name().as_str(), format!("{ID}#_"));
    // No template, no scripted feature: a package of settings with no feature.
    assert!(factory.features().is_empty());
    assert!(matches!(loader.resolve("myext#core"), Err(errors) if errors[0].detail.ends_with("it provides `_`")));
    let blocks = loader.blocks();
    assert_eq!(blocks.len(), 1);
    let block = &blocks[0].block;
    assert_eq!(block.uses, [UseEntry::parse("stdcore").unwrap()]);
    assert_eq!(block.features[0], ("toc".to_string(), FeatureBlock::Off));
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
        .with("a", "a.yaml", json!({ "flm": { "use": ["b", "stdcore"] } }), MemoryFiles::new())
        .with("b", "b.yaml", json!({ "flm": { "use": "a#_" } }), MemoryFiles::new());
    let source = Arc::new(Counting { inner, asked: Mutex::new(BTreeMap::new()), total: AtomicUsize::new(0) });
    let loader = Arc::new(ExtensionLoader::new(vec![Arc::clone(&source) as Arc<dyn ExtensionSource>]));
    let catalog = catalog_with(&loader);
    let found = catalog.resolve_all(&["a", "b", "a"]).expect("resolved");
    let names: Vec<&str> = found.iter().map(|f| f.name().as_str()).collect();
    assert_eq!(names, ["b.yaml#_", "stdcore", "a.yaml#_"]);
    // Asked again (the reach, the schema): answered from the loader alone.
    catalog.resolve_all(&["a", "b"]).expect("resolved");
    assert_eq!(*source.asked.lock().unwrap(), BTreeMap::from([("a".to_string(), 1), ("b".to_string(), 1)]));
    assert_eq!(source.total.load(Ordering::Relaxed), 2);
}

#[test]
fn a_compiled_name_in_its_use_list_reaches_the_compiled_package() {
    let loader = loader_of(json!({ "flm": { "use": ["stdcore"] } }), MemoryFiles::new());
    let catalog = catalog_with(&loader);
    let factory = catalog.resolve("myext").unwrap().unwrap();
    assert_eq!(factory.used().iter().map(|n| n.as_str()).collect::<Vec<_>>(), ["stdcore"]);
    let found = catalog.resolve_all(&["myext"]).expect("resolved");
    assert_eq!(found[0].name().as_str(), "stdcore");
}

#[test]
fn its_templates_reach_the_compose_slot_and_are_chosen_by_name() {
    let mut fancy = description();
    fancy["main"] = json!("fancy.html");
    let files = MemoryFiles::new()
        .with_value("templates/html/fancy/flmtemplate.yaml", fancy)
        .with_text("templates/html/fancy/fancy.html", "<main>{{ content }}</main>")
        .with_value("templates/html/bare.yaml", description())
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
fn a_templates_entry_that_is_not_a_folder_fails_the_load() {
    let files = MemoryFiles::new()
        .with_text("templates/readme.txt", "")
        .with_value("templates/html/fancy.yaml", json!({ "output": "html" }));
    let loader = loader_of(Value::Null, files);
    let errors = loader.resolve("myext").expect_err("the load fails");
    // Every problem is reported, and none is placed in the manifest's file.
    assert!(errors.iter().all(|error| error.file.is_none()));
    let details = details(errors);
    assert_eq!(details.len(), 2, "{details:?}");
    assert!(
        details[0].starts_with(
            "in the extension `myext`, `templates/html/fancy.yaml`: the description of the compose template \
             `fancy` cannot be read: "
        ),
        "{}",
        details[0]
    );
    assert_eq!(
        details[1],
        "`templates/readme.txt` of the extension `myext` must be a folder holding the templates of one output \
         (`templates/OUTPUT/`)"
    );
}

/// The compose template `t` of a data extension whose description
/// `templates/html/t/flmtemplate.yaml` lists the artifact `icons` described by `icons`,
/// with the files `files` of the extension.
fn shipping_folder(icons: Value, files: MemoryFiles) -> Result<Template, Vec<String>> {
    let description = json!({ "output": "html", "engine": "content-only", "accepts": "all", "artifacts": { "icons": icons } });
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
    let refused = |icons: Value, files: MemoryFiles| shipping_folder(icons, files).expect_err("the load fails");
    let at = "in the extension `myext`, `templates/html/t/flmtemplate.yaml`: ";
    assert_eq!(
        refused(json!({ "dir": "icons" }), MemoryFiles::new()),
        [format!("{at}the shipped folder `icons` of the compose template `t` is not a folder of the extension")]
    );
    // A file is not a folder.
    assert_eq!(
        refused(json!({ "dir": "icons" }), MemoryFiles::new().with_bytes("templates/html/t/icons", b"x")),
        [format!("{at}the shipped folder `icons` of the compose template `t` is not a folder of the extension")]
    );
    let hidden_only = MemoryFiles::new()
        .with_bytes("templates/html/t/icons/.DS_Store", b"x")
        .with_bytes("templates/html/t/icons/.git/HEAD", b"x");
    assert_eq!(
        refused(json!({ "dir": "icons" }), hidden_only),
        [format!("{at}the shipped folder `icons` of the compose template `t` holds no file")]
    );
    assert_eq!(
        refused(json!({ "dir": "../../../../icons" }), MemoryFiles::new()),
        [format!("{at}the shipped folder `../../../../icons` of the compose template `t` leaves the extension's folder")]
    );
    assert_eq!(
        refused(json!({ "dir": "/icons" }), MemoryFiles::new()),
        [format!("{at}the shipped folder `/icons` of the compose template `t` must be a relative path")]
    );
    assert_eq!(
        refused(json!({ "dir": "icons", "file": "x.svg" }), MemoryFiles::new()),
        [format!("{at}the artifact `icons` of the compose template `t` gives both `file` and `dir`; give one")]
    );
    assert_eq!(
        refused(json!({ "dir": "icons", "template": true }), MemoryFiles::new()),
        [format!(
            "{at}the artifact `icons` of the compose template `t` gives `dir` together with `template`, which applies \
             to a file only"
        )]
    );
}

#[test]
fn a_template_written_both_as_a_file_and_as_a_folder_fails_the_load() {
    // Neither form is read: the file's description, which `Template::from_data`
    // would refuse, adds no problem of its own.
    let files = MemoryFiles::new()
        .with_value("templates/html/fancy.yaml", json!({ "output": "html" }))
        .with_value("templates/html/fancy/flmtemplate.yaml", description())
        .with_value("templates/html/plain.yaml", description());
    let loader = loader_of(Value::Null, files);
    let details = details(loader.resolve("myext").expect_err("the load fails"));
    assert_eq!(details.len(), 1, "{details:?}");
    assert!(details[0].contains("`fancy`"), "{details:?}");
}

#[test]
fn layout_in_an_extension_gets_the_message_of_every_key_the_block_does_not_read() {
    let loader = loader_of(json!({ "flm": { "layout": "inline", "default_profile": "html", "docs": "x" } }), MemoryFiles::new());
    let errors = loader.resolve("myext").expect_err("refused");
    assert!(errors.iter().all(|error| error.file.as_deref() == Some(ID)));
    assert_eq!(
        details(errors),
        [
            "`flm.layout` is not a key of the `flm` block of the extension `myext`; the keys are `use`, `features`, \
             `parsing`, `workflows`, `feature_order`",
            "`flm.default_profile` is not a key of the `flm` block of the extension `myext`; the keys are `use`, \
             `features`, `parsing`, `workflows`, `feature_order`",
            "`flm.docs` is not a key of the `flm` block of the extension `myext`; the keys are `use`, `features`, \
             `parsing`, `workflows`, `feature_order`",
        ]
    );
}

#[test]
fn the_chains_of_feature_order_reach_the_package_entry_whatever_the_settings_switch_off() {
    let manifest = json!({ "flm": { "feature_order": ["a < b", "headings < x < toc"], "features": { "a": false } } });
    let files = MemoryFiles::new().with_value("templates/html/fancy.yaml", description());
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
    assert_eq!(
        details(errors),
        ["in the manifest of the extension `myext`, the chain `a, b` must have the form `NAME < NAME < … < NAME`, with \
          two or more feature names"]
    );
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
        error.to_string().contains(
            "Construct `\\foo` from `b` clashes with `\\foo` defined in `a` in text mode; either set `flm: \
             feature_order: ['a < b']` in your front matter to choose `b`'s definition, or have `b` declare `\\foo` \
             as an explicit override."
        ),
        "{error}"
    );
    let with = ordering(json!({ "flm": { "feature_order": "a < b" } })).build(&PackageSettings::new()).expect("the package");
    let registry = common::try_registry_with([two_definers_of_foo(), with]).unwrap_or_else(|error| panic!("{error}"));
    assert!(registry.build_diagnostics().is_empty());
}

#[test]
fn the_description_is_the_templates_feature_s_title() {
    let files = MemoryFiles::new().with_value("templates/html/fancy.yaml", description());
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
    assert!(details[0].starts_with("in the manifest of the extension `myext`, `features` must be a map"), "{}", details[0]);
    assert_eq!(
        details[1..],
        [
            "`flm.layout` is not a key of the `flm` block of the extension `myext`; the keys are `use`, `features`, \
             `parsing`, `workflows`, `feature_order`",
            "`flm.bogus` is not a key of the `flm` block of the extension `myext`; the keys are `use`, `features`, \
             `parsing`, `workflows`, `feature_order`",
        ]
    );
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
    let template = json!({ "output": "html", "engine": "minijinja", "main": "fancy.html", "accepts": "all" });
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
            json!({ "output": "html", "engine": "minijinja", "main": "fancy.html", "accepts": ["body"] }),
        )
        .with_text("templates/html/fancy/fancy.html", "one");
    assert_ne!(base, registry_digest(manifest("a"), other));
}

#[test]
fn a_template_reads_its_includes_through_the_extension_s_files() {
    let page = "{% include \"partials/head.html\" %}{{ include_raw(\"../../../dist/x.js\") }}";
    let files = MemoryFiles::new()
        .with_value(
            "templates/html/t/flmtemplate.yaml",
            json!({ "output": "html", "engine": "minijinja", "main": "t.html", "accepts": "all" }),
        )
        .with_text("templates/html/t/t.html", page)
        .with_text("templates/html/t/partials/head.html", "<head>{{ regions.title }}</head>")
        .with_text("dist/x.js", "let a = \"{{ x }}\";")
        .with_bytes("templates/html/t/logo.png", &[0x89, b'P', 0xff])
        .with_text("outside.js", "");
    let loader = loader_of(Value::Null, files);
    let factory = loader.resolve("myext").unwrap().expect("found");
    let package = factory.build(&PackageSettings::new()).expect("the package");
    let compose = PackageEntry::new("compose").feature(Arc::new(ComposeFeature::new()));
    let registry = common::registry_with([compose, package]);
    let options = ComposeOptions { template: TemplateChoice::Name("t".to_string()), ..ComposeOptions::default() };
    let template = chosen_template(&registry, &options, "html").expect("chosen");
    let included = template.files.as_ref().expect("the template's files");
    // By a path relative to the description's folder, the template's folder, read when asked.
    assert_eq!(included.read_text("partials/head.html"), Ok(Some(String::from("<head>{{ regions.title }}</head>"))));
    assert_eq!(included.read_text("../../../dist/x.js"), Ok(Some(String::from("let a = \"{{ x }}\";"))));
    assert_eq!(included.read_text("./partials/../partials/head.html").map(|text| text.is_some()), Ok(true));
    // No such file.
    assert_eq!(included.read_text("partials/none.html"), Ok(None));
    // The refusals.
    assert_eq!(
        included.read_text("../../../../outside.js"),
        Err(String::from("the compose template `t` includes `../../../../outside.js`, which leaves the extension's folder"))
    );
    assert_eq!(
        included.read_text("/dist/x.js"),
        Err(String::from("the compose template `t` includes `/dist/x.js`, which must be a relative path"))
    );
    assert_eq!(
        included.read_text("logo.png"),
        Err(String::from("the compose template `t` includes `logo.png`, and the file is not UTF-8 text"))
    );
    // A value held as data cannot be read as bytes: the files' own text follows.
    assert_eq!(
        included.read_text("flmtemplate.yaml"),
        Err(String::from(
            "the compose template `t` includes `flmtemplate.yaml`, and the file cannot be read: the file \
             `templates/html/t/flmtemplate.yaml` must be a file of bytes, and it is held as a data value"
        ))
    );
}
