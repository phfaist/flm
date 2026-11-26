//! Settings declared from data: a scripted
//! feature's `settings` become its settings declaration, its `register`
//! receives its final configuration, and its stages read the option types it
//! declares by their names; a declaration that contradicts itself, or an
//! option named outside the extension's stem, fails the load.

mod common;

use std::sync::{Arc, Mutex};

use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageFactory, PackageResolver};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::{FeatureSettings, Mark, PackageSettings};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::options::{Bind, OptionKey, OptionValues};
use flm_ext::adapter::table::ExtTable;
use flm_ext::api::core::registry::FeatureBuilder;
use flm_ext::api::core::settings::ReadError;
use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_ext::engine::{ConfigReport, SettingsReport};
use flm_ext::{
    Engine, ExtError, ExtRuntime, ExtensionFiles, ExtensionIdentity, ExtensionLoader, FeatureReport, LoadError,
    Loaded, Manifest, MemoryFiles, MemorySource, PackageReport,
};
use serde_json::{json, Map, Value};

use common::double::{stage, Double};
use common::json_double::JsonDouble;
use common::registry_with;

// --- fixtures ----------------------------------------------------------------------------------

/// An object schema whose properties are `keys`, each of any value.
fn object_schema(keys: &[&str]) -> Value {
    let properties: Map<String, Value> = keys.iter().map(|key| (key.to_string(), json!({}))).collect();
    json!({ "type": "object", "properties": properties })
}

/// The JSON object `value` as a map.
fn map(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("not a map: {other}"),
    }
}

/// The settings of the feature `feature` of `factory`, read from `data` with
/// the factory's declaration.
fn read(factory: &PackageFactory, feature: &str, data: Value) -> FeatureSettings {
    let decl = factory.settings(feature).expect("the feature declares settings");
    decl.read(&map(data)).expect("the settings read")
}

/// The run of a plan demanding `demand`, with the render options `render`,
/// over a one-tree document with the document options `document`.
fn run_with(registry: &Arc<FlmRegistry>, demand: &str, document: OptionValues, render: OptionValues) -> Run {
    let source = Arc::new(FlmSource::new("Some text.").with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the tree parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let request = PlanRequest::new().demand(demand).render_options(render);
    let plan = Arc::new(Plan::build(registry, &request).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::with_options(content, document)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The items of the extension's table `name` after `run`.
fn items(run: &Run, name: &str) -> Vec<Value> {
    run.table(TableHandle::<ExtTable>::new(name))
        .expect("the table is declared")
        .map(|table| table.items().to_vec())
        .unwrap_or_default()
}

/// The declaration of a document-scope stage `wc.read` writing `wc.seen`.
fn read_decl() -> StageDecl {
    StageDecl::new("wc.read", Effect::Compute, Scope::Document).writes("wc.seen")
}

// --- the declaration ---------------------------------------------------------------------------

#[test]
fn a_feature_s_settings_become_its_settings_declaration() {
    let mut options = Map::new();
    options.insert(
        "wc.display".to_string(),
        json!({ "bind": "document", "docs": "How the count is shown.", "schema": object_schema(&["mode"]) }),
    );
    let mut schema = object_schema(&["min_length", "kinds", "file"]);
    schema["properties"]["kinds"] = json!({
        "x-flm-marks": ["merge-object-key-values", "remove-key-when-value-is-false"],
        "default": { "word": { "label": "words" } },
        "additionalProperties": { "properties": { "label": { "x-flm-marks": ["markup"] } } },
    });
    schema["properties"]["file"] = json!({ "x-flm-marks": ["path"] });
    let report = SettingsReport {
        config: Some(ConfigReport { docs: "The count.".to_string(), schema }),
        options,
        read: false,
    };
    let factory = Double::new("wc").feature("count", |_| Ok(())).settings("count", report).factory();
    let decl = factory.settings("count").expect("the feature declares settings");
    let keys: Vec<(String, Bind)> = decl.keys().map(|key| (key.name, key.bind)).collect();
    assert_eq!(
        keys,
        [
            ("min_length".to_string(), Bind::Registry),
            ("kinds".to_string(), Bind::Registry),
            ("file".to_string(), Bind::Registry),
            ("mode".to_string(), Bind::Document),
        ]
    );
    assert_eq!(
        decl.option_types().collect::<Vec<_>>(),
        [(OptionKey::named("count"), Bind::Registry), (OptionKey::named("wc.display"), Bind::Document)]
    );
    let schema = decl.json_schema();
    assert_eq!(schema["properties"]["min_length"]["x-flm-bind"], json!("registry"));
    assert_eq!(schema["properties"]["mode"]["x-flm-bind"], json!("document"));
    assert_eq!(schema["properties"]["kinds"]["default"], json!({ "word": { "label": "words" } }));
    let marks: Vec<(&str, bool)> =
        decl.marks().iter().map(|mark| (mark.path.as_str(), mark.has(Mark::MergeObjectKeyValues))).collect();
    assert_eq!(marks, [("kinds", true), ("kinds.*.label", false), ("file", false)]);

    // The registry declares the option type, owned by the feature.
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package")]);
    let display = registry.option_decl(&OptionKey::named("wc.display")).expect("declared");
    assert_eq!((display.bind, display.owner.as_str(), display.docs.as_str()), (Bind::Document, "count", "How the count is shown."));
}

#[test]
fn register_receives_the_final_configuration() {
    let seen: Arc<Mutex<Vec<(String, Value)>>> = Arc::default();
    let record = |feature: &'static str, seen: &Arc<Mutex<Vec<(String, Value)>>>| {
        let seen = Arc::clone(seen);
        move |_: &mut dyn FeatureBuilder, configuration: &Value| {
            seen.lock().unwrap().push((feature.to_string(), configuration.clone()));
            Ok(())
        }
    };
    let report = SettingsReport {
        config: Some(ConfigReport { docs: String::new(), schema: object_schema(&["min_length", "max_length"]) }),
        ..SettingsReport::default()
    };
    let factory = Double::new("wc")
        .configured("count", record("count", &seen))
        .configured("plain", record("plain", &seen))
        .settings("count", report)
        .factory();

    // The values written, as written.
    let mut settings = PackageSettings::new();
    settings.insert(read(&factory, "count", json!({ "min_length": 3 })));
    registry_with([factory.build(&settings).expect("the package")]);
    // None written, and no settings at all: the empty map.
    registry_with([factory.build(&PackageSettings::new()).expect("the package")]);
    assert_eq!(
        *seen.lock().unwrap(),
        [
            ("count".to_string(), json!({ "min_length": 3 })),
            ("plain".to_string(), json!({})),
            ("count".to_string(), json!({})),
            ("plain".to_string(), json!({})),
        ]
    );
}

// --- the reads by name -------------------------------------------------------------------------

#[test]
fn a_stage_reads_a_document_bound_option_by_name() {
    let mut options = Map::new();
    options.insert("wc.display".to_string(), json!({ "bind": "document", "schema": object_schema(&["mode"]) }));
    let report = SettingsReport { options, ..SettingsReport::default() };
    let factory = JsonDouble::new("wc")
        .feature("count", |js, b| {
            js.call(b, "table", json!(["wc.seen", "What the stage read."]))?;
            let read = js.lend_stage(
                &json!({ "name": "wc.read", "effect": "compute", "scope": "document", "writes": ["wc.seen"] }),
                &json!({ "options": ["wc.display"] }),
                |js, cx, out| {
                    let display = js.call(cx, "option", json!(["wc.display"]))?;
                    let piece = js.call(out, "table", json!(["wc.seen"]))?;
                    js.call(&piece, "push", json!([display]))?;
                    Ok(())
                },
            )?;
            js.call(b, "stage", json!([read]))?;
            Ok(())
        })
        .settings("count", report)
        .factory();
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package")]);

    let document = read(&factory, "count", json!({ "mode": "words" })).document_options().clone();
    let run = run_with(&registry, "wc.seen", document, OptionValues::new());
    assert_eq!(items(&run, "wc.seen"), [json!({ "mode": "words" })]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));

    // Nothing set: the empty map.
    let run = run_with(&registry, "wc.seen", OptionValues::new(), OptionValues::new());
    assert_eq!(items(&run, "wc.seen"), [json!({})]);
}

#[test]
fn a_render_bound_option_set_on_the_request_reaches_the_stage() {
    let mut options = Map::new();
    options.insert("wc.width".to_string(), json!({ "bind": "render", "schema": object_schema(&["w"]) }));
    let report = SettingsReport { options, ..SettingsReport::default() };
    let reading = stage(read_decl(), Needs::new().option("wc.width"), |cx, out| {
        let width = cx.option("wc.width")?;
        // A name the stage did not declare is refused.
        let undeclared = match cx.option("wc.other") {
            Err(ExtError::Core { kind, .. }) => json!([kind]),
            other => json!(format!("{other:?}")),
        };
        let piece = out.table("wc.seen")?;
        piece.push(width)?;
        piece.push(undeclared)
    });
    let factory = Double::new("wc")
        .feature("count", move |b| {
            b.table("wc.seen", "What the stage read.")?;
            b.stage(Arc::clone(&reading))
        })
        .settings("count", report)
        .factory();
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package")]);

    let render = read(&factory, "count", json!({ "w": 7 })).render_options().clone();
    let run = run_with(&registry, "wc.seen", OptionValues::new(), render);
    assert_eq!(
        items(&run, "wc.seen"),
        [json!({ "w": 7 }), json!(["option"])]
    );
}

// --- what fails the load -----------------------------------------------------------------------

/// An engine whose one package `_` holds the one feature `feature`.
struct OneFeature {
    feature: FeatureReport,
}

/// A runtime whose features register nothing.
struct Nothing;

impl ExtRuntime for Nothing {
    fn register(&self, _: &str, _: &str, _: &Value, _: &mut dyn FeatureBuilder) -> Result<(), ExtError> {
        Ok(())
    }

    fn read_settings(&self, _: &str, feature: &str, _: &Value) -> Result<Result<Value, ReadError>, ExtError> {
        Err(ExtError::Handler { detail: format!("the feature `{feature}` has no configuration reader"), data: Value::Null })
    }

    fn digest_input(&self) -> Vec<u8> {
        Vec::new()
    }
}

impl Engine for OneFeature {
    fn name(&self) -> &str {
        "test"
    }

    fn load(&self, _: &ExtensionIdentity, _: &Manifest, _: Box<dyn ExtensionFiles>) -> Result<Loaded, Vec<LoadError>> {
        let package = PackageReport {
            name: "_".to_string(),
            uses: Vec::new(),
            block: Default::default(),
            templates: Vec::new(),
            features: vec![self.feature.clone()],
            host: Vec::new(),
            caches: Default::default(),
        };
        Ok(Loaded { packages: vec![package], runtime: Arc::new(Nothing) })
    }
}

/// The problems of loading the extension `wc`, whose one feature `count`
/// declares `settings`.
fn load_problems(settings: Value) -> Vec<String> {
    let feature: FeatureReport =
        serde_json::from_value(json!({ "name": "count", "settings": settings })).expect("a feature report");
    let source = MemorySource::new().with(
        "wc",
        "_flmextensions/wc.yaml",
        json!({ "extension": { "engine": "test" } }),
        MemoryFiles::new(),
    );
    let loader = ExtensionLoader::new(vec![Arc::new(source)]).with_engine(Arc::new(OneFeature { feature }));
    let errors = loader.resolve("wc").expect_err("the load fails");
    errors.into_iter().map(|error| error.detail).collect()
}

#[test]
fn an_option_name_outside_the_stem_fails_the_load() {
    let problems = load_problems(json!({ "options": {
        "display": { "bind": "document", "schema": object_schema(&["mode"]) },
        "wcx.size": { "bind": "render", "schema": object_schema(&["size"]) },
        "wc.fine": { "bind": "either", "schema": object_schema(&["fine"]) },
    } }));
    // One problem for each of the two options outside the stem.
    assert_eq!(problems.len(), 2, "{problems:#?}");
}

#[test]
fn a_contradictory_declaration_fails_the_load() {
    let problems = load_problems(json!({
        "config": { "schema": object_schema(&["width"]) },
        "options": {
            "wc.size": { "bind": "render", "schema": object_schema(&["width"]) },
            "wc.bare": { "bind": "document", "schema": { "type": "object" } },
            "wc.fixed": { "bind": "registry", "schema": object_schema(&["fixed"]) },
            "wc.merged": { "bind": "render", "schema": { "properties": {
                "merged": { "x-flm-marks": ["merge-object-key-values"], "default": 3 },
            } } },
            "wc.outside": { "bind": "render", "schema": { "properties": { "outside": {} }, "x-flm-marks": ["markup"] } },
        },
    }));
    // One problem for each of the five contradictions: `width` declared twice,
    // a schema without `properties`, a binding that is not one, default
    // entries that are not a map, a mark at the root of a schema.
    assert_eq!(problems.len(), 5, "{problems:#?}");
}
