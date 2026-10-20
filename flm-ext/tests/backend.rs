//! A backend and an output declared by a scripted feature
//! (`FeatureBuilder::backend`, `FeatureBuilder::output`): the test backend
//! `tb`, whose lowering stage is the library's (`LoweringStage::new`), whose
//! emit stage walks the lowered trees `flm.lowered.tb` into a text and whose
//! final stage writes the output's artifact set, through the Rust façade and
//! through the dispatcher; the name rule for a backend's and an output's name.

mod common;

use std::sync::{Arc, Mutex};

use flm_core::core::artifact::{Artifact, ArtifactId, ArtifactSet};
use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{Overrides, Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageEntry, RegistryBuildError, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::cascade::workflow_def;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::{self, StdcoreConfig};
use flm_ext::api::core::data::store::StageOut;
use flm_ext::api::core::data::table::TableValue;
use flm_ext::api::core::lowering::{DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule};
use flm_ext::api::core::registry::{BackendDef, BackendStage, FeatureBuilder, OutputDef, StandardTable};
use flm_ext::api::core::run::StageCx;
use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_ext::api::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_ext::api::lang::structural;
use flm_ext::api::model::element::ElementSpec;
use flm_ext::api::stdcore::backends::lowering::{lowered_table_name, LoweringStage};
use flm_ext::api::stdcore::backends::pres;
use flm_ext::api::techy::node::Node;
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::double::{stage, Double};
use common::json_double::{module, JsonDouble};

/// The media type the test's output declares (plan section 4.3).
const MEDIA_TYPE: &str = "application/x-typst";

/// The presentation element of the backend `tb` with a data payload, which
/// `tb` writes natively.
const NOTE: &str = "tb.pres.note";

/// What the handlers of a test observed, in order.
type Log = Arc<Mutex<Vec<String>>>;

/// Records `line` in `log`.
fn note(log: &Log, line: impl Into<String>) {
    log.lock().expect("the log").push(line.into());
}

/// The lines of `log`.
fn lines(log: &Log) -> Vec<String> {
    log.lock().expect("the log").clone()
}

// --- the backend `tb` through the Rust façade -------------------------------------------------

/// A rule's body as a closure.
type BodyFn = dyn Fn(&dyn Node<'_>, &mut dyn LowerCx, &mut dyn Lowerer) -> Result<Lowering, ExtError> + Send + Sync;

/// A [`Lower`] whose work is a closure.
struct LowerFn(Box<BodyFn>);

impl Lower for LowerFn {
    fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
        (self.0)(node, cx, lw)
    }
}

/// The presentation vocabulary and the five structural elements the text
/// backend writes.
fn vocabulary() -> Vec<String> {
    let mut native: Vec<String> = pres::ALL.iter().map(ToString::to_string).collect();
    native.extend(
        [structural::TEXT, structural::GROUP, structural::COMMENT, structural::CONTAINER, structural::PARBREAK].map(String::from),
    );
    native
}

/// The native set of `tb`: the [`vocabulary`] and the data element `note`.
fn native_with(note: &str) -> Vec<String> {
    let mut native = vocabulary();
    native.push(note.to_string());
    native
}

/// The native set of `tb`: the [`vocabulary`] and `tb`'s own data element.
fn native() -> Vec<String> {
    native_with(NOTE)
}

/// A rule of the package `stem` lowering the standard package's image
/// element on `tb` into a text: the library's image rule is registered for
/// its own backends only (`flm-core`'s `graphics::image_rule`), so every
/// other backend lowers the element itself.
fn image_rule(stem: &str) -> LoweringRule {
    let lower = LowerFn(Box::new(|_, cx, lw| Ok(Lowering::Emit(vec![lw.text(cx, "[image]")?]))));
    let decl = LoweringDecl::new().produces(structural::TEXT);
    LoweringRule::new(format!("{stem}.image.lower"), "flm.graphics.image", decl, Arc::new(lower), DefaultMark::OnBackends(vec!["tb".into()]))
}

/// Writes the presentation node `node` and its content as text: characters as
/// they are, a paragraph followed by a line break, the data element `NOTE` as
/// `[kind: content]`, its kind read from its payload (ruling S8).
fn emit<'t>(node: &dyn Node<'t>, out: &mut String, log: &Log) -> Result<(), ExtError> {
    if node.is_chars() {
        out.push_str(&node.chars().unwrap_or_default());
        return Ok(());
    }
    let element = node.element_name();
    if element == NOTE {
        let payload = node.payload()?.unwrap_or(Value::Null);
        note(log, format!("payload {payload}"));
        out.push_str(&format!("[{}: ", payload["kind"].as_str().unwrap_or("?")));
        emit_content(node, out, log)?;
        out.push(']');
        return Ok(());
    }
    emit_content(node, out, log)?;
    if element == pres::PAR {
        out.push('\n');
    }
    Ok(())
}

/// Writes the content of `node`: its slots in order, or its children.
fn emit_content<'t>(node: &dyn Node<'t>, out: &mut String, log: &Log) -> Result<(), ExtError> {
    if node.slot_count() > 0 {
        for i in 0..node.slot_count() {
            for child in node.slot_content_nodes(i).unwrap_or_default() {
                emit(&*child, out, log)?;
            }
        }
    } else {
        for child in node.children() {
            emit(&*child, out, log)?;
        }
    }
    Ok(())
}

/// The emit stage `tb.emit`: reads `flm.lowered.tb` and writes the text of
/// every lowered tree, in id order, as one item of `tb.output`.
fn emit_stage(log: &Log) -> Arc<dyn flm_ext::api::core::stage::Stage> {
    let log = Arc::clone(log);
    let decl = StageDecl::new("tb.emit", Effect::Compute, Scope::Document).writes("tb.output").docs("Writes the text.");
    stage(decl, Needs::new().reads("flm.lowered.tb"), move |cx: &dyn StageCx, out: &mut dyn StageOut| {
        let Some(TableValue::Lowered(lowered)) = cx.get("flm.lowered.tb")? else {
            note(&log, "not the lowered trees");
            return Ok(());
        };
        let mut text = String::new();
        for (_, root) in lowered.trees() {
            emit(&*root, &mut text, &log)?;
        }
        out.table("tb.output")?.push(json!(text))
    })
}

/// The final stage `tb.final`: reads the text of `tb.output` and writes it as
/// the entry artifact `tb:document` of the artifact set `tb.document`.
fn final_stage() -> Arc<dyn flm_ext::api::core::stage::Stage> {
    let decl = StageDecl::new("tb.final", Effect::Compute, Scope::Document).writes("tb.document").docs("Writes the file.");
    stage(decl, Needs::new().reads("tb.output"), |cx, out| {
        let Some(TableValue::Ext(read)) = cx.get("tb.output")? else { return Ok(()) };
        let text: String = read.items().iter().filter_map(Value::as_str).collect();
        let artifact = json!({
            "id": { "owner": "tb", "name": "document" },
            "media_type": MEDIA_TYPE,
            "entry": true,
            "data": { "bytes": text },
            "extension": "typ",
        });
        out.table("tb.document")?.push(artifact)
    })
}

/// Declares the macro `\tbnote{content}` (the element `tb.note`), the data
/// element `NOTE`, a rule lowering `tb.note` on `tb` into `NOTE` with the
/// payload `{ kind: "tip" }`, the tables of `tb`, the backend `tb` (the
/// library's lowering stage, [`emit_stage`], [`final_stage`]) of native set
/// `native`, and the output `tb`.
fn register_tb(b: &mut dyn FeatureBuilder, log: &Log, native: Vec<String>) -> Result<(), ExtError> {
    b.element(ElementDef::new(
        b.feature(),
        ["tbnote"],
        SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] },
        ElementSpec::new("tb.note"),
    ))?;
    b.presentation_element(NOTE, "A note, with its kind as data.")?;
    let lower = LowerFn(Box::new(|node, cx, lw| {
        let content = lw.lower_arg(cx, node, 0)?;
        Ok(Lowering::Emit(vec![lw.pres(cx, NOTE, &json!({ "kind": "tip" }), content)?]))
    }));
    let decl = LoweringDecl::new().produces(NOTE);
    let mark = DefaultMark::OnBackends(vec!["tb".into()]);
    b.lowering_rule(LoweringRule::new("tb.note.lower", "tb.note", decl, Arc::new(lower), mark))?;
    b.lowering_rule(image_rule("tb"))?;
    b.table_named(&lowered_table_name("tb"), StandardTable::LoweredTrees, "The lowered trees of `tb`.")?;
    b.table("tb.output", "The text `tb` writes.")?;
    b.table_named("tb.document", StandardTable::ArtifactSet, "The files of the output `tb`.")?;
    let def = BackendDef::new(
        "tb",
        native,
        BackendStage::Library(LoweringStage::new("tb")),
        BackendStage::Own(emit_stage(log)),
        BackendStage::Own(final_stage()),
    )
    .docs("A test backend.");
    b.backend(def)?;
    b.output(OutputDef::new("tb", "tb.document", "tb.document", MEDIA_TYPE, "typ").docs("The document as `tb` writes it."))
}

/// The package `tb`, its one feature `backend` declared by [`register_tb`].
fn tb(log: &Log) -> PackageEntry {
    let log = Arc::clone(log);
    Double::new("tb").feature("backend", move |b| register_tb(b, &log, native())).package()
}

/// The registry holding the standard package, `packages` and, when `workflow`
/// holds its fields, a workflow `tb` registered by the host
/// (`RegistryBuilder::workflow_fields`), as a program registers the
/// workflows of an extension's `flm` block.
fn build(packages: Vec<PackageEntry>, workflow: Option<&Value>) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    for package in packages {
        builder.use_package(package);
    }
    if let Some(Value::Object(fields)) = workflow {
        builder.workflow_fields(workflow_def("tb", fields.iter().map(|(key, value)| (key.as_str(), value))));
    }
    builder.build(&RegistryInputs::new())
}

/// The run of `plan` over the one-tree document `text`.
fn run(plan: Plan, text: &str) -> Run {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(plan.registry().lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the tree parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let mut run = Run::new(Arc::new(Doc::new(content)), Arc::new(plan), Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The artifact set `tb.document` after `run`.
fn documents(run: &Run) -> ArtifactSet {
    run.table(TableHandle::<ArtifactSet>::new("tb.document")).expect("the table is declared").cloned().unwrap_or_default()
}

/// The document of the tests.
const DOCUMENT: &str = "Hello \\tbnote{dear \\emph{world}}.\n\nBye.";

/// What `tb` writes for [`DOCUMENT`].
const EMITTED: &str = "Hello [tip: dear world].\nBye.\n";

/// Checks the one entry artifact of `run`'s `tb.document`.
fn check_document(run: &Run) {
    assert!(common::diagnostics(run).is_empty(), "{:?}", common::diagnostics(run));
    let set = documents(run);
    assert_eq!(set.len(), 1);
    let entry: &Artifact = set.entry().expect("an entry artifact");
    assert_eq!(entry.id, ArtifactId::new("tb", "document"));
    assert_eq!(entry.media_type, MEDIA_TYPE);
    assert_eq!(entry.extension.as_deref(), Some("typ"));
    assert_eq!(entry.as_bytes().map(|bytes| String::from_utf8_lossy(bytes).into_owned()).as_deref(), Some(EMITTED));
}

#[test]
fn a_backend_declared_through_the_facade_runs_on_a_demand_for_its_output() {
    let log: Log = Arc::default();
    let registry = build(vec![tb(&log)], None).expect("the registry builds");
    // The registry holds the backend, its three stages and the output.
    let backend = registry.backend("tb").expect("the backend `tb`");
    assert!(backend.native.iter().any(|name| name.as_str() == NOTE));
    // The three stages, in the order lowering, emit, final.
    assert_eq!(registry.stage_idx("flm.tb.lower"), Some(backend.lowering));
    assert_eq!(registry.stage_idx("tb.emit"), Some(backend.emit));
    assert_eq!(registry.stage_idx("tb.final"), Some(backend.final_stage));
    let output = registry.output("tb").expect("the output `tb`");
    assert_eq!(output.def.entry.extension, "typ");
    assert_eq!(output.def.entry.media_type, MEDIA_TYPE);
    let plan = Plan::build(&registry, &PlanRequest::new().demand("tb.document")).expect("the plan builds");
    let ran = run(plan, DOCUMENT);
    check_document(&ran);
    // The emit stage read the data element's payload from the lowered tree
    // (ruling S8).
    assert_eq!(lines(&log), [r#"payload {"kind":"tip"}"#]);
}

#[test]
fn a_backend_runs_through_a_workflow_the_host_registers() {
    let log: Log = Arc::default();
    let fields = json!({ "outputs": ["tb"], "demands": ["flm.validated"], "docs": "The document as `tb` writes it." });
    let registry = build(vec![tb(&log)], Some(&fields)).expect("the registry builds");
    let plan = Plan::for_workflow(&registry, "tb", &Overrides::none()).expect("the plan builds");
    check_document(&run(plan, DOCUMENT));
    assert_eq!(lines(&log), [r#"payload {"kind":"tip"}"#]);
}

#[test]
fn the_registry_reports_a_native_element_no_feature_declares() {
    let log: Log = Arc::default();
    let mut names = native();
    names.push("tb.pres.never".to_string());
    let package = Double::new("tb").feature("backend", move |b| register_tb(b, &log, names.clone())).package();
    let error = build(vec![package], None).expect_err("the build fails").to_string();
    assert!(error.contains("the element `tb.pres.never`, but no feature declares this element"), "{error}");
}

// --- the name rule ----------------------------------------------------------------------------

/// The error of the registry build of a package `tb` whose feature does
/// `register`, rendered.
fn build_error(register: impl Fn(&mut dyn FeatureBuilder) -> Result<(), ExtError> + Send + Sync + 'static) -> String {
    let package = Double::new("tb").feature("backend", register).package();
    build(vec![package], None).expect_err("the build fails").to_string()
}

/// A backend named `name` whose three stages are the library's lowering
/// stage for it and two quiet stages of the extension, named after it.
fn quiet_backend(name: &str, stages: &str) -> BackendDef {
    let quiet = |stage_name: String| {
        stage(StageDecl::new(stage_name, Effect::Compute, Scope::Document), Needs::new(), |_, _| Ok(()))
    };
    BackendDef::new(
        name,
        vocabulary(),
        BackendStage::Library(LoweringStage::new(name)),
        BackendStage::Own(quiet(format!("{stages}.emit"))),
        BackendStage::Own(quiet(format!("{stages}.final"))),
    )
}

#[test]
fn a_backend_or_an_output_named_outside_the_stem_is_refused() {
    let error = build_error(|b| b.backend(quiet_backend("other", "tb.other")));
    assert!(
        error.contains("the backend `other` declared by the package `_` of the extension `tb` must be `tb` or begin with `tb.`"),
        "{error}"
    );
    let error = build_error(|b| b.output(OutputDef::new("tbx", "tb.document", "tb.document", MEDIA_TYPE, "typ")));
    assert!(
        error.contains("the output `tbx` declared by the package `_` of the extension `tb` must be `tb` or begin with `tb.`"),
        "{error}"
    );
    // An extension's own stage of a backend keeps the stem rule.
    let error = build_error(|b| b.backend(quiet_backend("tb", "other")));
    assert!(
        error.contains("the stage `other.emit` declared by the package `_` of the extension `tb` must begin with `tb.`"),
        "{error}"
    );
}

#[test]
fn a_backend_under_the_stem_and_its_lowered_table_are_accepted() {
    let package = Double::new("tb")
        .feature("backend", |b| {
            b.table_named(&lowered_table_name("tb.sub"), StandardTable::LoweredTrees, "The lowered trees of `tb.sub`.")?;
            b.backend(quiet_backend("tb.sub", "tb.sub"))?;
            b.table_named("tb.sub.document", StandardTable::ArtifactSet, "The files.")?;
            b.output(OutputDef::new("tb.sub", "tb.sub.document", "tb.sub.document", MEDIA_TYPE, "typ"))
        })
        .package();
    let registry = build(vec![package], None).expect("the registry builds");
    assert!(registry.backend("tb.sub").is_some());
    assert!(registry.output("tb.sub").is_some());
    assert!(registry.stage_named("flm.tb.sub.lower").is_some());
    assert_eq!(lowered_table_name("tb.sub"), "flm.lowered.tb.sub");
}

#[test]
fn a_lowered_table_is_accepted_for_a_backend_of_the_extension_only() {
    let log: Log = Arc::default();
    let seen = Arc::clone(&log);
    let package = Double::new("tb")
        .feature("tables", move |b| {
            for name in ["flm.lowered.tb", "flm.lowered.tb.a", "flm.lowered.other", "flm.lowered.", "flm.output.tb"] {
                let answer = b.table_named(name, StandardTable::LoweredTrees, "Lowered trees.");
                note(&seen, format!("{name}: {}", answer.map_or_else(|error| error.to_string(), |()| "ok".to_string())));
            }
            Ok(())
        })
        .package();
    build(vec![package], None).expect("the registry builds");
    assert_eq!(
        lines(&log),
        [
            "flm.lowered.tb: ok",
            "flm.lowered.tb.a: ok",
            "flm.lowered.other: the table `flm.lowered.other` declared by the package `_` of the extension `tb` must begin with `tb.`",
            "flm.lowered.: the table `flm.lowered.` declared by the package `_` of the extension `tb` must begin with `tb.`",
            "flm.output.tb: the table `flm.output.tb` declared by the package `_` of the extension `tb` must begin with `tb.`",
        ]
    );
}

// --- the dispatcher ---------------------------------------------------------------------------

/// The package `tb` declared through the dispatcher alone: the tables, the
/// library's lowering stage by `LoweringStage::new` on the module handle, two
/// stage objects lent in, the backend and the output in their data forms.
/// What the handlers see is logged.
fn tb_by_json(log: &Log) -> PackageEntry {
    let log = Arc::clone(log);
    JsonDouble::new("tb")
        .feature("backend", move |js, b| {
            let lowering = module("stdcore/backends/lowering");
            let lowered = js.call(&lowering, "lowered_table_name", json!(["tb"]))?;
            note(&log, format!("lowered_table_name {lowered}"));
            js.call(b, "table_named", json!([lowered, "lowered-trees", "The lowered trees of `tb`."]))?;
            js.call(b, "table", json!(["tb.output", "The text."]))?;
            js.call(b, "table_named", json!(["tb.document", "artifact-set", "The files."]))?;
            let library = js.call(&lowering, "LoweringStage::new", json!(["tb"]))?;
            note(&log, format!("library {}", library["@handle"]["kind"]));
            let emit_log = Arc::clone(&log);
            let emit = js.lend_stage(
                &json!({ "name": "tb.emit", "effect": "compute", "scope": "document", "writes": ["tb.output"] }),
                &json!({ "reads": ["flm.lowered.tb"] }),
                move |js, cx, out| {
                    let read = js.call(cx, "get", json!(["flm.lowered.tb"]))?;
                    let trees = js.call(&read["lowered"], "trees", json!([]))?;
                    let mut text = String::new();
                    for pair in trees.as_array().into_iter().flatten() {
                        for node in js.call(&pair[1], "descendants", json!([]))?.as_array().into_iter().flatten() {
                            if js.call(node, "element_name", json!([]))? == json!(TBN_NOTE) {
                                note(&emit_log, format!("payload {}", js.call(node, "payload", json!([]))?));
                            }
                            if js.call(node, "is_chars", json!([]))? == json!(true) {
                                text.push_str(js.call(node, "chars", json!([]))?.as_str().unwrap_or_default());
                            }
                        }
                    }
                    let piece = js.call(out, "table", json!(["tb.output"]))?;
                    js.call(&piece, "push", json!([text]))?;
                    Ok(())
                },
            )?;
            let final_stage = js.lend_stage(
                &json!({ "name": "tb.final", "effect": "compute", "scope": "document", "writes": ["tb.document"] }),
                &json!({ "reads": ["tb.output"] }),
                |js, cx, out| {
                    let read = js.call(cx, "get", json!(["tb.output"]))?;
                    let items = js.call(&read["ext"], "items", json!([]))?;
                    let artifact = json!({
                        "id": { "owner": "tb", "name": "document" },
                        "media_type": MEDIA_TYPE,
                        "entry": true,
                        "data": { "bytes": items[0] },
                        "extension": "typ",
                    });
                    let piece = js.call(out, "table", json!(["tb.document"]))?;
                    js.call(&piece, "push", json!([artifact]))?;
                    Ok(())
                },
            )?;
            // A stage of another kind than `library_stage` or `stage`.
            let wrong = json!({
                "name": "tb", "native": native_with(TBN_NOTE), "lowering": library, "emit": b, "final_stage": final_stage,
            });
            let refused = js.call(b, "backend", json!([wrong])).map_err(|error| error.to_string());
            note(&log, format!("wrong kind {}", refused.err().unwrap_or_default()));
            let backend = json!({
                "name": "tb", "native": native_with(TBN_NOTE), "lowering": library, "emit": emit, "final_stage": final_stage,
                "docs": "A test backend.",
            });
            js.call(b, "backend", json!([backend]))?;
            let output = json!({
                "name": "tb", "capability": "tb.document", "table": "tb.document", "media_type": MEDIA_TYPE, "extension": "typ",
            });
            js.call(b, "output", json!([output]))?;
            let refused = js.call(b, "output", json!([{ "name": "other", "capability": "tb.document", "table": "tb.document", "media_type": "", "extension": "x" }]));
            note(&log, format!("output other {}", refused.err().map(|error| error.to_string()).unwrap_or_default()));
            Ok(())
        })
        .package()
}

/// The data element of the extension `tbn`.
const TBN_NOTE: &str = "tbn.pres.note";

/// The extension `tbn`: the macro `\tbnote`, its data element `TBN_NOTE` and
/// the rules lowering it and the image element on `tb`, declared through the
/// façade, so that the dispatcher's backend `tb` writes the data element of
/// another extension natively.
fn notes() -> PackageEntry {
    Double::new("tbn")
        .feature("notes", |b| {
            b.element(ElementDef::new(
                b.feature(),
                ["tbnote"],
                SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] },
                ElementSpec::new("tbn.note"),
            ))?;
            b.presentation_element(TBN_NOTE, "A note, with its kind as data.")?;
            let lower = LowerFn(Box::new(|node, cx, lw| {
                let content = lw.lower_arg(cx, node, 0)?;
                Ok(Lowering::Emit(vec![lw.pres(cx, TBN_NOTE, &json!({ "kind": "tip" }), content)?]))
            }));
            let decl = LoweringDecl::new().produces(TBN_NOTE);
            let mark = DefaultMark::OnBackends(vec!["tb".into()]);
            b.lowering_rule(LoweringRule::new("tbn.note.lower", "tbn.note", decl, Arc::new(lower), mark))?;
            b.lowering_rule(image_rule("tbn"))
        })
        .package()
}

#[test]
fn the_dispatcher_declares_a_backend_and_an_output() {
    let log: Log = Arc::default();
    let registry = build(vec![notes(), tb_by_json(&log)], None).expect("the registry builds");
    let plan = Plan::build(&registry, &PlanRequest::new().demand("tb.document")).expect("the plan builds");
    let ran = run(plan, DOCUMENT);
    assert!(common::diagnostics(&ran).is_empty(), "{:?}", common::diagnostics(&ran));
    let set = documents(&ran);
    let entry = set.entry().expect("an entry artifact");
    assert_eq!(entry.media_type, MEDIA_TYPE);
    assert_eq!(entry.as_bytes(), Some(&b"Hello dear world.Bye."[..]));
    let registered = lines(&log);
    // `register` runs once for the build; the stages run once each.
    assert_eq!(
        registered,
        [
            r#"lowered_table_name "flm.lowered.tb""#,
            r#"library "library_stage""#,
            "wrong kind `backend`: an object of the kind `library_stage` or `stage` is expected here, not one of the kind `feature_builder`",
            "output other the output `other` declared by the package `_` of the extension `tb` must be `tb` or begin with `tb.`",
            r#"payload {"kind":"tip"}"#,
        ]
    );
}
