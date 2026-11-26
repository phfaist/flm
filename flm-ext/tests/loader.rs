//! The loader: entries resolved through the host's sources to
//! the factories of the extensions' packages, each extension loaded once, its
//! problems answered where it is reached.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use flm_core::core::registry::{PackageResolver, ResolveError};
use flm_core::core::settings::PackageSettings;
use flm_ext::api::core::registry::FeatureBuilder;
use flm_ext::api::core::settings::ReadError;
use flm_ext::block::Block;
use flm_ext::{
    Engine, ExtError, ExtRuntime, ExtensionFiles, ExtensionIdentity, ExtensionLoader, FeatureReport, LoadError, Loaded,
    Manifest, MemoryFiles, MemorySource, PackageReport, UseEntry,
};
use serde_json::{json, Value};

use common::{catalog_with, memory_loader};

/// An engine that reports the packages it was given, each with one feature
/// `count` declaring a table under the stem, and counts its loads.
struct TestEngine {
    packages: Vec<(&'static str, Vec<&'static str>)>,
    bytes: Vec<u8>,
    loads: Arc<AtomicUsize>,
    /// The host services every package declares.
    host: Vec<&'static str>,
}

impl TestEngine {
    fn new(packages: Vec<(&'static str, Vec<&'static str>)>) -> TestEngine {
        TestEngine { packages, bytes: b"test 1".to_vec(), loads: Arc::new(AtomicUsize::new(0)), host: Vec::new() }
    }
}

struct TestRuntime {
    bytes: Vec<u8>,
}

impl ExtRuntime for TestRuntime {
    fn register(&self, _: &str, _: &str, _: &Value, b: &mut dyn FeatureBuilder) -> Result<(), ExtError> {
        b.table("myext.words", "")
    }

    fn read_settings(&self, _: &str, feature: &str, _: &Value) -> Result<Result<Value, ReadError>, ExtError> {
        Err(ExtError::Handler { detail: format!("the feature `{feature}` has no configuration reader"), data: Value::Null })
    }

    fn digest_input(&self) -> Vec<u8> {
        self.bytes.clone()
    }
}

impl Engine for TestEngine {
    fn name(&self) -> &str {
        "test"
    }

    fn load(&self, _: &ExtensionIdentity, _: &Manifest, _: Box<dyn ExtensionFiles>) -> Result<Loaded, Vec<LoadError>> {
        self.loads.fetch_add(1, Ordering::Relaxed);
        let packages = self
            .packages
            .iter()
            .map(|(name, uses)| PackageReport {
                name: name.to_string(),
                uses: uses.iter().map(|entry| UseEntry::parse(entry).expect("an entry")).collect(),
                block: Block { other: vec![("mark".to_string(), json!(name))], ..Block::default() },
                templates: Vec::new(),
                features: vec![FeatureReport {
                    name: format!("count-{name}"),
                    title: String::new(),
                    docs: String::new(),
                    version: None,
                    settings: None,
                    templates: Default::default(),
                }],
                host: self.host.iter().map(|name| name.to_string()).collect(),
                caches: Default::default(),
            })
            .collect();
        Ok(Loaded { packages, runtime: Arc::new(TestRuntime { bytes: self.bytes.clone() }) })
    }
}

const ID: &str = "_flmextensions/myext.yaml";

fn data(manifest: Value) -> MemorySource {
    MemorySource::new().with("myext", ID, manifest, MemoryFiles::new())
}

fn test_manifest() -> Value {
    json!({ "extension": { "engine": "test" } })
}

fn errors(result: Result<Option<flm_core::core::registry::PackageFactory>, Vec<ResolveError>>) -> Vec<ResolveError> {
    result.expect_err("the resolve fails")
}

/// The place of each problem of a failed resolve: its file, line and column.
fn places(
    result: Result<Option<flm_core::core::registry::PackageFactory>, Vec<ResolveError>>,
) -> Vec<(Option<String>, Option<u32>, Option<u32>)> {
    errors(result).into_iter().map(|error| (error.file, error.line, error.column)).collect()
}

/// The place of a problem in no file.
const NOWHERE: (Option<String>, Option<u32>, Option<u32>) = (None, None, None);

/// The place of a problem in the manifest's file, at no line.
fn in_manifest() -> (Option<String>, Option<u32>, Option<u32>) {
    (Some(ID.to_string()), None, None)
}

#[test]
fn an_entry_resolves_through_a_memory_source_to_a_factory() {
    let loader = memory_loader(data(json!({ "flm": { "use": ["std"] } })));
    let factory = loader.resolve("myext").expect("no problem").expect("found");
    assert_eq!(factory.name().as_str(), "_flmextensions/myext.yaml#_");
    assert_eq!(factory.used().iter().map(|n| n.as_str()).collect::<Vec<_>>(), ["std"]);
    assert_eq!(loader.resolve("other").expect("no problem").map(|f| f.name().clone()), None);
    let catalog = catalog_with(&loader);
    let found = catalog.resolve_all(&["myext"]).expect("resolved");
    let names: Vec<&str> = found.iter().map(|f| f.name().as_str()).collect();
    assert_eq!(names, ["std", "_flmextensions/myext.yaml#_"]);
}

#[test]
fn myext_and_myext_hash_underscore_are_one_package() {
    let loader = memory_loader(data(Value::Null));
    let short = loader.resolve("myext").unwrap().unwrap();
    let long = loader.resolve("myext#_").unwrap().unwrap();
    assert_eq!(short.name(), long.name());
}

#[test]
fn a_factory_name_is_answered_before_any_parsing() {
    let loader = memory_loader(data(Value::Null));
    // The identity is no locator: before a load, the name is not this loader's.
    assert!(UseEntry::parse("_flmextensions/myext.yaml#_").is_err());
    assert!(loader.resolve("_flmextensions/myext.yaml#_").unwrap().is_none());
    loader.resolve("myext").unwrap().unwrap();
    let exact = loader.resolve("_flmextensions/myext.yaml#_").unwrap().expect("the factory name is found");
    assert_eq!(exact.name().as_str(), "_flmextensions/myext.yaml#_");
    assert_eq!(loader.list().unwrap().iter().map(|n| n.as_str()).collect::<Vec<_>>(), ["_flmextensions/myext.yaml#_"]);
}

#[test]
fn a_dot_entry_asked_of_the_loader_is_refused() {
    let loader = memory_loader(data(Value::Null));
    assert_eq!(places(loader.resolve(".#core")), [NOWHERE]);
}

#[test]
fn a_dot_in_a_package_s_uses_is_the_extension_itself() {
    let engine = TestEngine::new(vec![("_", vec![".#core", "std"]), ("core", vec![])]);
    let loader = Arc::new(ExtensionLoader::new(vec![Arc::new(data(test_manifest()))]).with_engine(Arc::new(engine)));
    let factory = loader.resolve("myext").unwrap().unwrap();
    assert_eq!(
        factory.used().iter().map(|n| n.as_str()).collect::<Vec<_>>(),
        ["_flmextensions/myext.yaml#core", "std"]
    );
    let catalog = catalog_with(&loader);
    let found = catalog.resolve_all(&["myext"]).expect("resolved");
    let names: Vec<&str> = found.iter().map(|f| f.name().as_str()).collect();
    assert_eq!(names, ["_flmextensions/myext.yaml#core", "std", "_flmextensions/myext.yaml#_"]);
}

#[test]
fn two_locators_reaching_one_identity_load_it_once() {
    let engine = TestEngine::new(vec![("_", vec![])]);
    let loads = Arc::clone(&engine.loads);
    let source = MemorySource::new()
        .with("myext", ID, test_manifest(), MemoryFiles::new())
        .with("@acme/myext", ID, test_manifest(), MemoryFiles::new());
    let loader = ExtensionLoader::new(vec![Arc::new(source)]).with_engine(Arc::new(engine));
    let a = loader.resolve("myext").unwrap().unwrap();
    let b = loader.resolve("@acme/myext").unwrap().unwrap();
    let c = loader.resolve("myext#_").unwrap().unwrap();
    assert_eq!((a.name(), b.name()), (c.name(), c.name()));
    assert_eq!(loads.load(Ordering::Relaxed), 1);
}

#[test]
fn the_first_source_that_answers_wins() {
    let first = MemorySource::new().with("other", "first/other.yaml", Value::Null, MemoryFiles::new());
    let second = MemorySource::new()
        .with("myext", "second/myext.yaml", Value::Null, MemoryFiles::new())
        .with("other", "second/other.yaml", Value::Null, MemoryFiles::new());
    let third = MemorySource::new().with("myext", "third/myext.yaml", Value::Null, MemoryFiles::new());
    let loader = ExtensionLoader::new(vec![Arc::new(first), Arc::new(second), Arc::new(third)]);
    assert_eq!(loader.resolve("myext").unwrap().unwrap().name().as_str(), "second/myext.yaml#_");
    assert_eq!(loader.resolve("other").unwrap().unwrap().name().as_str(), "first/other.yaml#_");
}

#[test]
fn a_package_the_extension_lacks_is_refused() {
    let engine = TestEngine::new(vec![("_", vec![]), ("core", vec![])]);
    let loader = ExtensionLoader::new(vec![Arc::new(data(test_manifest()))]).with_engine(Arc::new(engine));
    assert_eq!(places(loader.resolve("myext#svg")), [NOWHERE]);
}

#[test]
fn an_unknown_engine_fails_the_load() {
    let loader = ExtensionLoader::new(vec![Arc::new(data(json!({ "extension": { "engine": "python" } })))])
        .with_engine(Arc::new(TestEngine::new(vec![])));
    assert_eq!(loader.engines(), ["data", "test"]);
    assert_eq!(places(loader.resolve("myext")), [in_manifest()]);
}

#[test]
fn an_unmet_flm_version_fails_the_load() {
    let loader = memory_loader(data(json!({ "extension": { "flm_version": ">=999" } })));
    assert_eq!(places(loader.resolve("myext")), [in_manifest()]);
}

#[test]
fn a_manifest_problem_is_placed_in_the_manifest_s_file() {
    let loader = memory_loader(data(json!({ "extension": { "engines": "js" }, "flm": { "use": 3 } })));
    assert_eq!(places(loader.resolve("myext")), [in_manifest()]);
    // A problem of the data engine's block is a manifest problem too.
    let loader = memory_loader(data(json!({ "flm": { "use": 3 } })));
    assert_eq!(places(loader.resolve("myext")), [in_manifest()]);
}

#[test]
fn a_failed_extension_answers_its_problems_to_every_entry() {
    let source = MemorySource::new()
        .with("myext", ID, json!({ "extension": { "engine": "python" } }), MemoryFiles::new())
        .with("again", ID, Value::Null, MemoryFiles::new());
    let loader = ExtensionLoader::new(vec![Arc::new(source)]);
    let first = errors(loader.resolve("myext"));
    assert_eq!(first.len(), 1);
    assert_eq!(errors(loader.resolve("myext#_")), first);
    assert_eq!(errors(loader.resolve("myext#core")), first);
    assert_eq!(errors(loader.resolve("again")), first);
    assert!(loader.blocks().is_empty());
}

#[test]
fn blocks_lists_each_loaded_package_with_its_block() {
    let engine = TestEngine::new(vec![("z", vec![]), ("_", vec![]), ("a", vec![])]);
    let source = data(test_manifest()).with("plain", "plain.yaml", json!({ "flm": { "features": { "toc": false } } }), MemoryFiles::new());
    let loader = ExtensionLoader::new(vec![Arc::new(source)]).with_engine(Arc::new(engine));
    loader.resolve("plain").unwrap().unwrap();
    loader.resolve("myext#a").unwrap().unwrap();
    let blocks = loader.blocks();
    let listed: Vec<(&str, &str)> = blocks.iter().map(|b| (b.package.as_str(), b.extension.stem.as_str())).collect();
    assert_eq!(
        listed,
        [
            ("plain.yaml#_", "plain"),
            ("_flmextensions/myext.yaml#z", "myext"),
            ("_flmextensions/myext.yaml#_", "myext"),
            ("_flmextensions/myext.yaml#a", "myext"),
        ]
    );
    assert_eq!(blocks[0].block.features, [("toc".to_string(), flm_ext::block::FeatureBlock::off())]);
    assert_eq!(blocks[2].block.other, [("mark".to_string(), json!("_"))]);
    assert_eq!(blocks[1].extension.identity, ID);
}

#[test]
fn manifest_gives_the_manifest_of_each_loaded_extension_as_read() {
    let source = data(json!({ "extension": { "description": "My words.", "author": "Ada Lovelace", "engine": "test" } }))
        .with("broken", "broken.yaml", json!({ "extension": { "engine": "python" } }), MemoryFiles::new());
    let loader = ExtensionLoader::new(vec![Arc::new(source)]).with_engine(Arc::new(TestEngine::new(vec![("_", vec![])])));
    // Before a load, the loader has no manifest.
    assert_eq!(loader.manifest(ID), None);
    loader.resolve("myext").unwrap().unwrap();
    let manifest = loader.manifest(ID).expect("the manifest of the loaded extension");
    assert_eq!(manifest.description.as_deref(), Some("My words."));
    assert_eq!(manifest.author.as_deref(), Some("Ada Lovelace"));
    assert_eq!(manifest.engine, "test");
    // An extension that failed to load has none; the identity is not the locator.
    errors(loader.resolve("broken"));
    assert_eq!(loader.manifest("broken.yaml"), None);
    assert_eq!(loader.manifest("myext"), None);
}

/// The digest of the registry holding the standard features and the package
/// `myext` of an extension of the test engine, whose manifest is `manifest`
/// and whose engine's bytes are `bytes`.
fn digest(manifest: Value, bytes: &[u8]) -> String {
    let mut engine = TestEngine::new(vec![("_", vec![])]);
    engine.bytes = bytes.to_vec();
    let loader = ExtensionLoader::new(vec![Arc::new(data(manifest))]).with_engine(Arc::new(engine));
    let factory = loader.resolve("myext").unwrap().unwrap();
    let registry = common::registry_with([factory.build(&PackageSettings::new()).expect("the package")]);
    format!("{:?}", registry.digest())
}

#[test]
fn the_digest_changes_with_the_manifest_and_the_engine_s_bytes() {
    let manifest = |description: &str| json!({ "extension": { "engine": "test", "description": description } });
    let base = digest(manifest("a"), b"one");
    assert_eq!(base, digest(manifest("a"), b"one"));
    // The same manifest, its keys in another order: the canonical form is one.
    assert_eq!(base, digest(json!({ "extension": { "description": "a", "engine": "test" } }), b"one"));
    assert_ne!(base, digest(manifest("b"), b"one"));
    assert_ne!(base, digest(manifest("a"), b"two"));
}

// ---- host services, places, the `\` rule ----

/// A host service named `name` that answers nothing.
struct Named(&'static str);

impl flm_ext::host::HostService for Named {
    fn name(&self) -> &str {
        self.0
    }

    fn call(
        &self,
        _: &flm_ext::host::HostCaller,
        _: &str,
        _: &Value,
        _: &flm_core::progress::Scope,
    ) -> Result<flm_ext::host::HostAnswer, flm_ext::host::HostServiceError> {
        Ok(flm_ext::host::HostAnswer::Data(Value::Null))
    }
}

/// A loader over the test engine whose packages `_` and `extra` declare the
/// host services `host`, offering `offered`.
fn declaring(host: Vec<&'static str>, offered: &[&'static str]) -> ExtensionLoader {
    let mut engine = TestEngine::new(vec![("_", vec![]), ("extra", vec![])]);
    engine.host = host;
    let loader = ExtensionLoader::new(vec![Arc::new(data(test_manifest()))]).with_engine(Arc::new(engine));
    offered.iter().fold(loader, |loader, name| loader.with_host_service(Arc::new(Named(name))))
}

#[test]
fn a_package_declaring_a_service_the_host_lacks_fails_the_load() {
    // One problem for each of the two packages.
    let loader = declaring(vec!["files", "programs"], &["files", "echo"]);
    assert_eq!(loader.host_services(), ["echo", "files"]);
    assert_eq!(places(loader.resolve("myext")), [NOWHERE, NOWHERE]);
    let none = declaring(vec!["files"], &[]);
    assert_eq!(places(none.resolve("myext")), [NOWHERE, NOWHERE]);
    // Declared and offered: the extension loads.
    assert!(declaring(vec!["files"], &["files"]).resolve("myext").expect("loads").is_some());
}

#[test]
fn a_second_service_of_one_name_replaces_the_first() {
    let loader = ExtensionLoader::new(Vec::new())
        .with_host_service(Arc::new(Named("files")))
        .with_host_service(Arc::new(Named("files")));
    assert_eq!(loader.host_services(), ["files"]);
}

#[test]
fn the_digest_changes_with_the_host_list() {
    let with = |host: Vec<&'static str>| {
        let mut engine = TestEngine::new(vec![("_", vec![])]);
        engine.host = host;
        let loader = ExtensionLoader::new(vec![Arc::new(data(test_manifest()))])
            .with_engine(Arc::new(engine))
            .with_host_service(Arc::new(Named("files")))
            .with_host_service(Arc::new(Named("echo")));
        let factory = loader.resolve("myext").unwrap().unwrap();
        let registry = common::registry_with([factory.build(&PackageSettings::new()).expect("the package")]);
        format!("{:?}", registry.digest())
    };
    let base = with(Vec::new());
    assert_eq!(base, with(Vec::new()));
    assert_ne!(base, with(vec!["files"]));
    assert_ne!(with(vec!["files"]), with(vec!["files", "echo"]));
    assert_ne!(with(vec!["files", "echo"]), with(vec!["echo", "files"]));
}

/// A source answering one extension, `myext`, whose manifest's keys are
/// placed by `places` (a key path and its line); or failing with `error`.
struct Placed {
    manifest: Value,
    places: Vec<(&'static str, u32)>,
    error: Option<flm_ext::source::SourceError>,
}

impl flm_ext::ExtensionSource for Placed {
    fn find(&self, _: &flm_ext::Locator) -> Result<Option<flm_ext::FoundExtension>, flm_ext::source::SourceError> {
        if let Some(error) = &self.error {
            return Err(error.clone());
        }
        let places = self.places.clone();
        let place_of: flm_ext::source::PlaceOf = Arc::new(move |path: &str| {
            places.iter().find(|(key, _)| *key == path).map(|(_, line)| flm_ext::source::Place { line: *line, column: 3 })
        });
        Ok(Some(flm_ext::FoundExtension {
            manifest: self.manifest.clone(),
            identity: ID.to_string(),
            files: Box::new(MemoryFiles::new()),
            place_of: Some(place_of),
        }))
    }
}

fn placed_loader(manifest: Value, places: Vec<(&'static str, u32)>) -> ExtensionLoader {
    ExtensionLoader::new(vec![Arc::new(Placed { manifest, places, error: None })])
}

#[test]
fn a_manifest_problem_by_key_path_carries_the_place_the_resolver_gives() {
    let loader = placed_loader(json!({ "extension": { "engine": "lua" } }), vec![("extension.engine", 4)]);
    assert_eq!(places(loader.resolve("myext")), [(Some(ID.to_string()), Some(4), Some(3))]);
    // The manifest's reader names the key path; the data engine's block too.
    let loader = placed_loader(json!({ "extension": { "engines": "js" } }), vec![("extension.engines", 2)]);
    let found = errors(loader.resolve("myext"));
    assert_eq!((found[0].line, found[0].column), (Some(2), Some(3)), "{found:?}");
    let loader = placed_loader(json!({ "flm": { "use": [1] } }), vec![("flm.use[0]", 7), ("flm.use", 6)]);
    let found = errors(loader.resolve("myext"));
    assert!(found[0].line.is_some(), "{found:?}");
    // A path the resolver does not place: in the file, at no line.
    let loader = placed_loader(json!({ "extension": { "engine": "lua" } }), Vec::new());
    assert_eq!(places(loader.resolve("myext")), [in_manifest()]);
}

#[test]
fn a_source_error_with_a_place_is_placed() {
    let error = flm_ext::source::SourceError {
        locator: "myext".to_string(),
        path: Some(ID.to_string()),
        place: Some(flm_ext::source::Place { line: 5, column: 9 }),
        detail: "the manifest `_flmextensions/myext.yaml` is not YAML".to_string(),
    };
    let loader = ExtensionLoader::new(vec![Arc::new(Placed { manifest: Value::Null, places: Vec::new(), error: Some(error) })]);
    assert_eq!(
        errors(loader.resolve("myext")),
        [ResolveError::new("the manifest `_flmextensions/myext.yaml` is not YAML").in_file(ID).at(5, 9)]
    );
}

#[test]
fn a_backslash_in_a_template_description_path_belongs_to_a_name() {
    // `templates\html\x.yaml` is one name, not a path under `templates/`.
    let extension = ExtensionIdentity { identity: ID.to_string(), stem: "myext".to_string(), name: "myext".to_string() };
    let description = json!({ "name": "x", "output": "html", "engine": "content-only", "accepts": "all" });
    let files = MemoryFiles::new().with_value("templates\\html\\x.yaml", description);
    let root = flm_ext::paths::Root::Ext(String::from("myext"));
    let read = |path: &str| flm_ext::templates::read(&files, &root, path, &format!("{ID}#_"), &extension);
    assert_eq!(read("templates\\html\\x.yaml").expect("the template").name, "x");
    assert!(read("templates/html/x.yaml").is_err());
}
