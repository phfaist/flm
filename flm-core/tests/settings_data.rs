//! Settings declared from data (`SettingsDecl::config_data`, `options_data`,
//! `merge_entries_data`) and the reads by name that go with them
//! (`OptionKey::Data`, `Needs::option_named`, `StageCx::option_named`,
//! `RewriteCx::option_named`, `LoweringDecl::option_named`, the lowering
//! façade's `option_named`): the keys a schema gives, the values kept as
//! written, the bindings, the declaration's own mistakes, the schema, and the
//! reads through a run.

mod common;

use std::sync::Arc;

use serde_json::{Map, Value, json};

use flm_core::core::lowering::{DefaultMark, Lowering};
use flm_core::core::plan::{Overrides, Plan, PlanError, PlanRequest};
use flm_core::core::registry::{BuildError, BuildPhase, FeatureBuilder, FeatureInfo, FlmFeature};
use flm_core::core::run::RunOptions;
use flm_core::core::settings::{SettingsDecl, SettingsError, SettingsErrorKind};
use flm_core::core::stage::{Effect, Needs, RewriteDecl, Scope, StageDecl};
use flm_core::model::addr::TreeId;
use flm_core::model::options::{Bind, DataValue, OptionKey, OptionValues};
use flm_core::stdcore::backends::{lowering, pieces, text};
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::math;
use flm_core::stdcore::features::userdefs::{Userdefs, UserdefsConfig};

use common::lowering::{lowered, produces, render_with, rule};
use common::macro_def;
use common::pipeline::{
    LOG, Log, Note, doc_with_options, plan_for, registry, registry_error, rewrite, run, span, stage,
};

// --- fixtures ----------------------------------------------------------------------------------

type RegisterFn = dyn Fn(&mut FeatureBuilder<'_>) -> Result<(), BuildError> + Send + Sync;

/// A feature whose settings are the declaration `decl` and whose registration
/// the test supplies.
struct DataFeature {
    name: String,
    decl: SettingsDecl,
    register: Box<RegisterFn>,
}

impl FlmFeature for DataFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(self.name.as_str(), self.name.as_str())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        (self.register)(b)
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(self.decl.clone())
    }
}

fn data_feature(
    name: &str,
    decl: SettingsDecl,
    register: impl Fn(&mut FeatureBuilder<'_>) -> Result<(), BuildError> + Send + Sync + 'static,
) -> Arc<dyn FlmFeature> {
    Arc::new(DataFeature { name: name.to_string(), decl, register: Box::new(register) })
}

/// An object schema whose properties are `keys`, each of any value.
fn object_schema(keys: &[&str]) -> Value {
    let properties: Map<String, Value> =
        keys.iter().map(|key| (key.to_string(), json!({ "type": "integer" }))).collect();
    json!({ "type": "object", "properties": properties })
}

fn map(value: Value) -> Map<String, Value> {
    match value {
        Value::Object(map) => map,
        other => panic!("not a map: {other}"),
    }
}

fn invalid(result: Result<SettingsDecl, SettingsError>) -> SettingsError {
    match result {
        Ok(_) => panic!("the declaration was expected to be refused"),
        Err(error) => error,
    }
}

fn logged(run: &flm_core::core::run::Run) -> Vec<String> {
    run.table(LOG).unwrap().map(|log: &Log| log.0.clone()).unwrap_or_default()
}

// --- reading --------------------------------------------------------------------------------------

#[test]
fn a_data_configuration_keeps_the_keys_written_and_reads_as_an_empty_map_otherwise() {
    let decl = SettingsDecl::new("wc")
        .config_data(object_schema(&["min_length", "max_length"]), "The word count.")
        .expect("a sound declaration");
    assert_eq!(decl.keys().map(|key| key.name).collect::<Vec<_>>(), ["min_length", "max_length"]);
    assert!(decl.keys().all(|key| key.bind == Bind::Registry));
    assert_eq!(decl.option_types().collect::<Vec<_>>(), [(OptionKey::named("wc"), Bind::Registry)]);

    // The value written is kept as written, its type unchecked.
    let settings = decl.read(&map(json!({ "min_length": "three" }))).expect("a known key");
    assert_eq!(settings.config::<DataValue>(), Some(&DataValue(json!({ "min_length": "three" }))));
    assert!(settings.document_options().is_empty());
    assert!(settings.render_options().is_empty());

    // Nothing written: the empty map, not the schema's defaults.
    let empty = decl.read(&Map::new()).expect("nothing to refuse");
    assert_eq!(empty.config::<DataValue>(), Some(&DataValue(json!({}))));
}

#[test]
fn a_key_outside_the_schema_s_properties_is_an_unknown_key() {
    let decl = SettingsDecl::new("wc")
        .config_data(object_schema(&["min_length"]), "")
        .and_then(|decl| decl.options_data("wc.display", Bind::Document, object_schema(&["size"]), ""))
        .expect("a sound declaration");
    let errors = decl
        .read(&map(json!({ "minlength": 3, "size": 2, "colour": 1 })))
        .expect_err("two unknown keys");
    let found: Vec<(&str, SettingsErrorKind, &str)> =
        errors.iter().map(|error| (error.path.as_str(), error.kind, error.detail.as_str())).collect();
    assert_eq!(
        found,
        [
            ("wc.minlength", SettingsErrorKind::UnknownKey, "the feature `wc` has no setting `minlength`"),
            ("wc.colour", SettingsErrorKind::UnknownKey, "the feature `wc` has no setting `colour`"),
        ]
    );
}

#[test]
fn a_data_option_goes_to_the_document_or_the_render_request_by_its_binding() {
    let decl = SettingsDecl::new("wc")
        .options_data("wc.document", Bind::Document, object_schema(&["a"]), "")
        .and_then(|decl| decl.options_data("wc.render", Bind::Render, object_schema(&["b"]), ""))
        .and_then(|decl| decl.options_data("wc.either", Bind::Either, object_schema(&["c"]), ""))
        .and_then(|decl| decl.options_data("wc.unwritten", Bind::Either, object_schema(&["d"]), ""))
        .expect("a sound declaration");
    let settings = decl.read(&map(json!({ "a": 1, "b": 2, "c": 3 }))).expect("known keys");
    let document = settings.document_options();
    let render = settings.render_options();
    assert_eq!(document.get_named("wc.document"), Some(&json!({ "a": 1 })));
    assert_eq!(document.get_named("wc.either"), Some(&json!({ "c": 3 })));
    assert_eq!(render.get_named("wc.render"), Some(&json!({ "b": 2 })));
    assert_eq!(render.len(), 1);
    // An option none of whose keys is written gets no value.
    assert_eq!(document.len(), 2);
    assert_eq!(document.get_named("wc.unwritten"), None);
    assert_eq!(settings.config::<DataValue>(), None);
}

// --- the declaration's own mistakes ---------------------------------------------------------------

#[test]
fn a_data_declaration_that_contradicts_itself_is_an_error() {
    let check = |error: SettingsError, path: &str, detail: &str| {
        assert_eq!(error.kind, SettingsErrorKind::InvalidDeclaration, "{detail}");
        assert_eq!(error.path, path);
        assert_eq!(error.detail, detail);
        assert_eq!(error.to_string(), detail);
    };
    // S1: a schema without `properties`, for the configuration and for an option.
    check(
        invalid(SettingsDecl::new("wc").config_data(json!({ "type": "object" }), "")),
        "wc",
        "the schema of the configuration of the feature `wc` must be an object schema with \
         `properties`, a map from each setting to its schema",
    );
    check(
        invalid(SettingsDecl::new("wc").options_data("wc.display", Bind::Document, json!(true), "")),
        "wc",
        "the schema of the option `wc.display` of the feature `wc` must be an object schema with \
         `properties`, a map from each setting to its schema",
    );
    // S2: a key declared twice, across a configuration and an option.
    let configured = SettingsDecl::new("wc").config_data(object_schema(&["a", "b"]), "").unwrap();
    check(
        invalid(configured.clone().options_data("wc.display", Bind::Render, object_schema(&["b"]), "")),
        "wc.b",
        "the feature `wc` declares the setting `b` twice",
    );
    // S3: a second configuration.
    check(
        invalid(configured.config_data(object_schema(&["c"]), "")),
        "wc",
        "the feature `wc` declares two configurations",
    );
    // S4: an option bound to the registry.
    check(
        invalid(SettingsDecl::new("wc").options_data("wc.fixed", Bind::Registry, object_schema(&["a"]), "")),
        "wc",
        "the feature `wc` declares the option `wc.fixed` bound to the registry, and a value fixed \
         when the registry is built belongs in the feature's configuration",
    );
    // S5: a merge mark on a path below a key.
    for path in ["kinds.words", "kinds.*"] {
        check(
            invalid(SettingsDecl::new("wc").merge_entries_data(path, Map::new())),
            &format!("wc.{path}"),
            &format!(
                "the feature `wc` marks `{path}` as merged entry by entry, and only a setting \
                 itself can be marked so, not a path below one"
            ),
        );
    }
}

// --- the schema -----------------------------------------------------------------------------------

#[test]
fn the_schema_of_a_data_part_is_the_schema_given_with_its_binding_marks_and_defaults() {
    let schema = json!({
        "type": "object",
        "properties": {
            "kinds": { "type": "object", "description": "The [kinds](crate::Kind) of words." },
            "label": { "$ref": "#/$defs/Label" },
            "anything": true,
        },
        "$defs": { "Label": { "type": "string", "description": "A label, in `markup`." } },
    });
    let defaults = map(json!({ "word": { "min": 1 } }));
    let decl = SettingsDecl::new("wc")
        .config_data(schema, "The word count's configuration.")
        .and_then(|decl| decl.merge_entries_data("kinds", defaults.clone()))
        .and_then(|decl| {
            decl.options_data("wc.display", Bind::Render, object_schema(&["size"]), "How it shows.")
        })
        .expect("a sound declaration")
        .markup("label");
    assert_eq!(decl.default_entries("kinds"), Some(&defaults));
    assert!(decl.mark_at("kinds").is_some_and(|mark| mark.merge_entries));
    // The schema's description is the documentation texts the declaration gives.
    #[cfg(feature = "docs")]
    assert_eq!(
        decl.json_schema(),
        json!({
            "description": "The word count's configuration.\n\nHow it shows.",
            "type": "object",
            "properties": {
                "kinds": {
                    "type": "object",
                    "description": "The kinds of words.",
                    "x-flm-bind": "registry",
                    "default": { "word": { "min": 1 } },
                },
                "label": { "$ref": "#/$defs/Label", "x-flm-bind": "registry" },
                "anything": { "x-flm-bind": "registry" },
                "size": { "type": "integer", "x-flm-bind": "render" },
            },
            "additionalProperties": false,
            "x-flm-marks": [
                { "path": "kinds", "merge_entries": true, "kind": "data" },
                { "path": "label", "merge_entries": false, "kind": "markup" },
            ],
            "$defs": { "Label": { "type": "string", "description": "A label, in `markup`." } },
        })
    );
}

// --- reads by name through a run ------------------------------------------------------------------

/// The feature `t`: the data option `t.display`, either-bound, and a stage
/// `t.report` that logs what it reads of it with `reads` declared.
fn display_feature(needs: Needs) -> Arc<dyn FlmFeature> {
    let decl = SettingsDecl::new("t")
        .options_data("t.display", Bind::Either, object_schema(&["size"]), "")
        .expect("a sound declaration");
    data_feature("t", decl, move |b| {
        b.table(LOG, "what the stage read")?;
        b.stage(stage(
            StageDecl::new("t.report", Effect::Compute, Scope::Document).writes(LOG),
            needs.clone(),
            |cx, out| {
                let seen = match cx.option_named("t.display") {
                    Ok(value) => value.to_string(),
                    Err(error) => error.to_string(),
                };
                out.table(LOG).expect("declared").0.push(seen);
            },
        ))
    })
}

fn display(size: u32) -> OptionValues {
    let mut values = OptionValues::new();
    values.set_named("t.display", json!({ "size": size }));
    values
}

#[test]
fn a_stage_reads_a_data_option_by_name_from_the_document_the_render_request_or_its_default() {
    let registry = registry(vec![display_feature(Needs::new().option_named("t.display"))]);
    let seen = |document: OptionValues, render: OptionValues| {
        let doc = doc_with_options(&registry, &["x"], document);
        let request = PlanRequest::new().demand("t.log").render_options(render);
        logged(&run(&doc, &plan_for(&registry, &request), &RunOptions::new()))
    };
    assert_eq!(seen(display(1), display(2)), [r#"{"size":2}"#]);
    assert_eq!(seen(display(1), OptionValues::new()), [r#"{"size":1}"#]);
    assert_eq!(seen(OptionValues::new(), OptionValues::new()), ["{}"]);
    // The value enters the plan's identity like any render option value.
    let key = |render: OptionValues| {
        plan_for(&registry, &PlanRequest::new().demand("t.log").render_options(render)).key()
    };
    assert_eq!(key(display(1)), key(display(1)));
    assert_ne!(key(display(1)), key(display(2)));
}

#[test]
fn a_read_by_name_the_stage_did_not_declare_is_refused_with_the_name() {
    // At run time, a read the stage did not declare.
    let undeclared = registry(vec![display_feature(Needs::new())]);
    let doc = doc_with_options(&undeclared, &["x"], display(1));
    let request = PlanRequest::new().demand("t.log");
    assert_eq!(
        logged(&run(&doc, &plan_for(&undeclared, &request), &RunOptions::new())),
        ["stage `t.report` must declare each option type it reads, but it reads `t.display` \
          without declaring it; it declares: []"]
    );
    // At plan time, a declared read of a name no feature declares.
    let registry = registry(vec![display_feature(Needs::new().option_named("t.nothing"))]);
    match Plan::build(&registry, &PlanRequest::new().demand("t.log")).expect_err("undeclared") {
        PlanError::NeedsUndeclared { stage, kind, name, roster, .. } => {
            assert_eq!(stage, "t.report");
            assert_eq!(kind, "option type");
            assert_eq!(name, "t.nothing");
            assert_eq!(roster, ["t.display"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_rewrite_stage_reads_a_data_option_by_name() {
    let decl = SettingsDecl::new("t")
        .options_data("t.display", Bind::Document, object_schema(&["size"]), "")
        .expect("a sound declaration");
    let registry = registry(vec![data_feature("t", decl, |b| {
        b.rewrite_stage(rewrite(
            RewriteDecl::new("t.rw", "t.rewritten"),
            Needs::new().option_named("t.display"),
            |_, tree, cx, diags| {
                let detail = cx.option_named("t.display").unwrap().to_string();
                diags.push(techy::error::Diagnostic::note(Note { detail }, span()));
                Ok(Arc::clone(tree))
            },
        ))
    })]);
    let doc = doc_with_options(&registry, &["x"], display(4));
    let run = run(&doc, &plan_for(&registry, &PlanRequest::new().demand("t.rewritten")), &RunOptions::new());
    let seen: Vec<String> =
        run.pipeline_diagnostics().conditions::<Note>().map(|note| note.detail.clone()).collect();
    assert_eq!(seen, [r#"{"size":4}"#]);
}

#[test]
fn two_features_declaring_one_data_option_name_are_a_duplicate() {
    let declaring = |feature: &str| {
        let decl = SettingsDecl::new(feature)
            .options_data("shared.display", Bind::Document, object_schema(&["size"]), "")
            .expect("a sound declaration");
        data_feature(feature, decl, |_| Ok(()))
    };
    let error = registry_error(vec![declaring("t.a"), declaring("t.b")]);
    assert_eq!(error.phase, BuildPhase::Validate);
    match &error.errors[0] {
        BuildError::Duplicate { kind, name, first, second } => {
            assert_eq!(*kind, "option type");
            assert_eq!(name, "shared.display");
            assert_eq!(first, "t.a");
            assert_eq!(second, "t.b");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_lowering_rule_reads_a_data_option_by_name() {
    let reading = rule(
        "t.foo.lower.default",
        "t.foo",
        produces(&[]).option_named("w.width"),
        DefaultMark::Everywhere,
        |_, cx, lw| {
            let width = lw.option_named("w.width")?["w"].to_string();
            Ok(Lowering::Emit(vec![lw.text(cx, &width)?]))
        },
    );
    let decl = SettingsDecl::new("w")
        .options_data("w.width", Bind::Render, object_schema(&["w"]), "")
        .expect("a sound declaration");
    // The registry of `common::lowering::registry`, with the feature `w`.
    let features: Vec<Arc<dyn FlmFeature>> = vec![
        lowering::feature(),
        pieces::feature(),
        text::feature(),
        flm_core::stdcore::check_feature(),
        Arc::new(Blocks::default()),
        Arc::new(Userdefs::new(UserdefsConfig::new())),
        math::feature(),
        flm_core::math_passthrough::feature(),
        common::pipeline::feature("t", move |b| {
            b.element(macro_def("t", "foo", 1))?;
            b.lowering_rule(reading.clone())
        }),
        data_feature("w", decl, |_| Ok(())),
    ];
    let registry = registry(features);
    let mut render_options = OptionValues::new();
    render_options.set_named("w.width", json!({ "w": 7 }));
    let run = render_with(&registry, &["\\foo{b}"], &Overrides { render_options, ..Overrides::none() });
    let lowered = &lowered(&run, TreeId::tree(0)).tree;
    let texts: Vec<String> =
        lowered.iter_storage_order().filter_map(|node| node.chars().map(String::from)).collect();
    assert_eq!(texts, ["7"]);
}

// --- both kinds of parts --------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Hash, serde::Deserialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
struct PageOptions {
    /// The page's width, in characters.
    width: u32,
}

#[test]
fn a_rust_part_and_a_data_part_live_in_one_declaration() {
    let decl = SettingsDecl::new("page")
        .options::<PageOptions>(Bind::Render, "The page.")
        .options_data("page.extra", Bind::Document, object_schema(&["note"]), "Extra.")
        .expect("a sound declaration");
    let keys: Vec<(String, Bind)> = decl.keys().map(|key| (key.name, key.bind)).collect();
    assert_eq!(keys, [("width".to_string(), Bind::Render), ("note".to_string(), Bind::Document)]);
    assert_eq!(
        decl.option_types().collect::<Vec<_>>(),
        [(OptionKey::of::<PageOptions>(), Bind::Render), (OptionKey::named("page.extra"), Bind::Document)]
    );
    let settings = decl.read(&map(json!({ "width": 72, "note": 5 }))).expect("known keys");
    assert_eq!(settings.render_options().get::<PageOptions>().map(|page| page.width), Some(72));
    assert_eq!(settings.document_options().get_named("page.extra"), Some(&json!({ "note": 5 })));
    let schema = decl.json_schema();
    assert_eq!(schema["properties"]["width"]["x-flm-bind"], json!("render"));
    assert_eq!(schema["properties"]["note"]["x-flm-bind"], json!("document"));

    // A data part may not declare a key of the Rust part.
    let error = invalid(
        SettingsDecl::new("page")
            .options::<PageOptions>(Bind::Render, "")
            .options_data("page.extra", Bind::Document, object_schema(&["width"]), ""),
    );
    assert_eq!(error.detail, "the feature `page` declares the setting `width` twice");

    // The registry declares both option types.
    let registry = registry(vec![data_feature("page", decl, |_| Ok(()))]);
    let extra = registry.option_decl(&OptionKey::named("page.extra")).expect("declared");
    assert_eq!((extra.bind, extra.owner.as_str()), (Bind::Document, "page"));
    #[cfg(feature = "docs")]
    assert_eq!(extra.docs, "Extra.");
    assert!(registry.option_decl(&OptionKey::of::<PageOptions>()).is_some());
}
