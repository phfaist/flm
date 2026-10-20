//! The engine `wasm` over scripted guests (`common::wat`): the load and its
//! refusals, `define` and the reports, each of the thirteen handler rows
//! (the entry and its arguments as the guest received them, its reply
//! converted, the lending of its object from a scripted `register`), the
//! rehearsal on another thread, a nested entry and the outer call's fuel, the
//! limits and the failures, strict mode, the instances per thread, the
//! digest, the guest's own errors, the requests the host refuses, and the
//! module page.

#![cfg(feature = "wasm")]

mod common;

use std::sync::{Arc, Mutex};

use flm_core::core::plan::{Overrides, Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageFactory};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::backends::html;
use flm_core::stdcore::features::compose::{stage_name, ComposeOptions, TemplateChoice};
use flm_ext::adapter::handles::module_handle;
use flm_ext::adapter::values::{diagnostic_value_to_json, handle_to_value};
use flm_ext::wasm::{WasmEngine, WasmHost, WasmSettings, ENGINE_VERSION};
use flm_ext::{Engine, ExtError, ExtensionIdentity, Manifest, MemoryFiles, MemorySource};
use serde_json::{json, Value};

use common::wat::{wat_bytes, Script, Step};
use common::{compose_registry, toc_entries};

/// The extension's stem (and identity).
const STEM: &str = "wc";
/// The module's path.
const MODULE: &str = "guest.wasm";
/// The package objects of every script but the reports' own.
const PACKAGES: &str = r#"[{"name":"_","features":[{"name":"words","object":{}}]}]"#;

/// The lines the guests sent to `console`.
type Log = Arc<Mutex<Vec<String>>>;

/// A host with `settings` whose console is kept in the log.
fn host_with(settings: WasmSettings) -> (WasmHost, Log) {
    let log: Log = Arc::default();
    let sink = Arc::clone(&log);
    let host = WasmHost::new(settings).with_console(Arc::new(move |_, text| {
        sink.lock().expect("the log").push(text.to_string());
    }));
    (host, log)
}

/// A host with the default settings.
fn host() -> (WasmHost, Log) {
    host_with(WasmSettings::default())
}

/// A source of the extension `wc` holding `files`, its manifest `manifest`.
fn source(files: MemoryFiles, manifest: Value) -> MemorySource {
    MemorySource::new().with(STEM, STEM, manifest, files)
}

/// The loader of the extension `wc` of the module `bytes`.
fn loader(host: &WasmHost, bytes: &[u8]) -> Arc<flm_ext::ExtensionLoader> {
    common::wasm_loader(host, source(MemoryFiles::new().with_bytes(MODULE, bytes), common::wasm_manifest(MODULE)))
}

/// The problems of loading the module `bytes`.
fn problems(bytes: &[u8]) -> Vec<String> {
    common::load_problems(&loader(&host().0, bytes), STEM)
}

/// The package `_` of the scripted extension `script`, loaded with `host`.
fn factory(host: &WasmHost, script: &Script) -> PackageFactory {
    common::factory_of(&loader(host, &script.bytes()), STEM)
}

/// The registry of the standard package and the package of `script`.
fn registry(host: &WasmHost, script: &Script) -> Arc<FlmRegistry> {
    common::registry_with([factory(host, script).build(&PackageSettings::new()).expect("the package builds")])
}

/// A script of `define`, `packages` (one feature `words`), `register` as
/// given, `rehearse` (answering every call), then `others`.
fn script(register: Step, others: Vec<Step>) -> Script {
    let mut steps = vec![Step::new("define"), Step::new("packages").ok(PACKAGES), register, Step::new("rehearse").repeat()];
    steps.extend(others);
    Script::new(steps)
}

/// The entry calls the guests received, in order: the entry and its
/// arguments.
fn entries(log: &Log) -> Vec<(String, Value)> {
    let lines = log.lock().expect("the log").clone();
    lines
        .iter()
        .filter_map(|line| line.strip_prefix("entry "))
        .map(|text| {
            let request: Value = serde_json::from_str(text).expect("an entry request is JSON");
            (request["entry"].as_str().unwrap_or_default().to_string(), request["args"].clone())
        })
        .collect()
}

/// The names of the entry calls the guests received, in order.
fn names(log: &Log) -> Vec<String> {
    entries(log).into_iter().map(|(name, _)| name).collect()
}

/// The arguments of the first call of `entry`.
fn args_of(log: &Log, entry: &str) -> Value {
    entries(log).into_iter().find(|(name, _)| name == entry).map(|(_, args)| args).unwrap_or(Value::Null)
}

/// The replies the guests received to their requests, in order.
fn replies(log: &Log) -> Vec<Value> {
    let lines = log.lock().expect("the log").clone();
    lines.iter().filter_map(|line| line.strip_prefix("reply ")).map(|text| serde_json::from_str(text).expect("JSON")).collect()
}

/// The kind of the handle `value`.
fn kind(value: &Value) -> &str {
    value["@handle"]["kind"].as_str().unwrap_or_default()
}

/// The handle of the API module `path`, as JSON text.
fn module(path: &str) -> String {
    handle_to_value(module_handle(path).expect("a module of the API")).to_string()
}

/// `f` run on a thread marked as a worker of `flm-core`, on which a
/// sequential run executes its stages itself.
fn on_worker<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    flm_core::lang::guard::run_with_worker_stack(f).expect("the worker thread starts")
}

/// The sequential run of a plan demanding `demands` over a document of one
/// tree, `text`.
fn run_demanding(registry: &Arc<FlmRegistry>, demands: &[&str], text: &str) -> Run {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the tree parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let request = demands.iter().fold(PlanRequest::new(), |request, demand| request.demand(*demand));
    let plan = Arc::new(Plan::build(registry, &request).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::sequential()).expect("the run executes");
    run
}

/// The data of the first diagnostic of `run` whose identifier is `id`.
fn data_of(run: &Run, id: &str) -> Value {
    run.diagnostics()
        .iter()
        .find(|d| d.identifier() == id)
        .map(|d| diagnostic_value_to_json(&d.data().serializable_data()))
        .unwrap_or(Value::Null)
}

/// The `register` of a feature declaring the table `wc.seen` and the
/// document stage `wc.see` writing it.
fn stage_register() -> Step {
    Step::new("register")
        .request(r#"{"native":"call","handle":$H1,"method":"table","args":["wc.seen","what the stage saw"]}"#)
        .request(
            r#"{"native":"lend_stage","decl":{"name":"wc.see","effect":"compute","scope":"document","writes":["wc.seen"]},"needs":{}}"#,
        )
        .request(r#"{"native":"call","handle":$H1,"method":"stage","args":[$R]}"#)
}

/// The stage `wc.see`'s `run`, pushing `{"saw": "it"}` to `wc.seen`.
fn stage_run() -> Step {
    Step::new("run_stage")
        .request(r#"{"native":"call","handle":$H2,"method":"table","args":["wc.seen"]}"#)
        .request(r#"{"native":"call","handle":$R,"method":"push","args":[{"saw":"it"}]}"#)
}

// --- the load ----------------------------------------------------------------------------------

#[test]
fn a_module_that_does_not_compile_is_refused() {
    // The detail is `wasmi`'s, which writes the two byte arrays one element per
    // line; the text keeps it on one line.
    assert_eq!(
        problems(b"not a module"),
        ["the module `guest.wasm` of the extension `wc` must be a valid WebAssembly module: magic header not detected: \
          bad magic number - expected=[ 0x0, 0x61, 0x73, 0x6d, ] actual=[ 0x6e, 0x6f, 0x74, 0x20, ] (at offset 0x0)"]
    );
}

#[test]
fn a_module_lacking_an_export_or_with_an_export_of_another_type_is_refused() {
    let text = script(Step::new("register"), vec![]).wat();
    let lacking = text.replace(r#"(func (export "flm_free") (param i32 i32))"#, "");
    assert_eq!(
        problems(&wat_bytes(&lacking)),
        ["the module `guest.wasm` of the extension `wc` must export the functions `flm_entry`, `flm_alloc` and `flm_free` \
          and the memory `memory`; it lacks `flm_free`"]
    );
    let other = text.replace(r#"(func (export "flm_free") (param i32 i32))"#, r#"(func (export "flm_free") (param i32))"#);
    assert_eq!(
        problems(&wat_bytes(&other)),
        ["the export `flm_free` of the module `guest.wasm` of the extension `wc` must have the type `(func (param i32 i32))`"]
    );
}

#[test]
fn a_module_importing_what_the_host_does_not_offer_is_refused() {
    let text = script(Step::new("register"), vec![]).wat();
    let foreign = text.replacen("(module\n", "(module\n  (import \"env\" \"abort\" (func))\n", 1);
    assert_eq!(
        problems(&wat_bytes(&foreign)),
        ["the module `guest.wasm` of the extension `wc` imports `env.abort`, which this program does not offer; the \
          imports offered are `flm.call` and `flm.take`"]
    );
    let typed = text.replacen("(module\n", "(module\n  (import \"flm\" \"call\" (func (param i32)))\n", 1);
    assert_eq!(
        problems(&wat_bytes(&typed)),
        ["the import `flm.call` of the module `guest.wasm` of the extension `wc` must have the type `(func (param i32 \
          i32) (result i32))`"]
    );
}

#[test]
fn a_module_whose_initial_memory_is_above_the_limit_is_refused() {
    // 2,000 pages of 64 KiB: 125 MiB, above the default 64 MiB.
    let bytes = script(Step::new("register"), vec![]).pages(2000).bytes();
    assert_eq!(
        problems(&bytes),
        ["the initial memory of the module `guest.wasm` of the extension `wc` must be at most 67108864 bytes, its \
          memory limit"]
    );
}

#[test]
fn the_module_s_path_and_file_are_checked() {
    let (host, _) = host();
    let path_problems = |path: &str| {
        let files = MemoryFiles::new().with_bytes(MODULE, b"");
        common::load_problems(&common::wasm_loader(&host, source(files, common::wasm_manifest(path))), STEM)
    };
    assert_eq!(
        path_problems("../guest.wasm"),
        ["`extension.wasm.module` of the extension `wc` is `../guest.wasm`, which leaves the extension's folder"]
    );
    assert_eq!(
        path_problems("/guest.wasm"),
        ["`extension.wasm.module` of the extension `wc` is `/guest.wasm`, which is an absolute path; it must be a path \
          inside the extension's folder"]
    );
    assert_eq!(
        path_problems("a\\guest.wasm"),
        ["`extension.wasm.module` of the extension `wc` is `a\\guest.wasm`, which holds `\\`; a path inside the \
          extension uses `/`"]
    );
    assert_eq!(
        path_problems("other.wasm"),
        ["the extension `wc` names its module `other.wasm` (`extension.wasm.module`), and the extension holds no file \
          `other.wasm`"]
    );
    let held_as_data = MemoryFiles::new().with_value(MODULE, json!(1));
    let found = common::load_problems(&common::wasm_loader(&host, source(held_as_data, common::wasm_manifest(MODULE))), STEM);
    assert!(found[0].starts_with("the module `guest.wasm` of the extension `wc` cannot be read: "), "{found:?}");
}

#[test]
fn an_instance_that_cannot_start_and_failing_load_entries_are_load_errors() {
    let text = script(Step::new("register"), vec![]).wat();
    let starting = text.replacen("  (memory (export", "  (func $boom unreachable)\n  (start $boom)\n  (memory (export", 1);
    assert_eq!(
        problems(&wat_bytes(&starting)),
        ["the engine of the extension `wc` could not start: wasm `unreachable` instruction executed"]
    );
    let define = Script::new(vec![Step::new("define").trap()]);
    assert_eq!(
        problems(&define.bytes()),
        ["the entry `define` of the module `guest.wasm` of the extension `wc` must run without a trap: wasm \
          `unreachable` instruction executed"]
    );
    // A load entry's own error names the entry, the module and the extension.
    let error = ExtError::Value { detail: "the packages are not ready".to_string() };
    let envelope = json!({ "err": error.data(), "message": error.to_string() }).to_string();
    let packages = Script::new(vec![Step::new("define"), Step::new("packages").reply(&envelope)]);
    assert_eq!(
        problems(&packages.bytes()),
        ["the entry `packages` of the module `guest.wasm` of the extension `wc` must answer without an error: the \
          packages are not ready"]
    );
    let define = Script::new(vec![Step::new("define").reply(&envelope)]);
    assert_eq!(
        problems(&define.bytes()),
        ["the entry `define` of the module `guest.wasm` of the extension `wc` must answer without an error: the \
          packages are not ready"]
    );
}

#[test]
fn each_load_entry_has_one_call_s_fuel() {
    // One burn fits in one call's fuel, two do not: `packages` does not run
    // on what `define` left.
    let burn = 60_000;
    let (host, log) = host_with(WasmSettings { fuel: Some(1_000_000), ..WasmSettings::default() });
    let both = Script::new(vec![Step::new("define").burn(burn), Step::new("packages").burn(burn).ok(PACKAGES)]);
    factory(&host, &both);
    assert_eq!(names(&log), ["define", "packages"]);
    let twice = Script::new(vec![Step::new("define"), Step::new("packages").burn(burn).burn(burn).ok(PACKAGES)]);
    let (host, _) = host_with(WasmSettings { fuel: Some(1_000_000), ..WasmSettings::default() });
    assert_eq!(
        common::load_problems(&loader(&host, &twice.bytes()), STEM),
        ["the entry `packages` of the module `guest.wasm` of the extension `wc` must finish within its budget of \
          1000000 fuel units"]
    );
}

#[test]
fn define_and_the_reports_read_the_package_objects() {
    let packages = r#"[{"name":"_","uses":["stdcore"],"flm":{"features":{"text":{"width":3}}},"templates":["./templates/html/fancy.yaml"],"host":["files"],"features":[{"name":"words","object":{"title":"Words"}},{"name":"count","object":{}}]}]"#;
    let bytes = Script::new(vec![Step::new("define"), Step::new("packages").ok(packages)]).bytes();
    let files = MemoryFiles::new()
        .with_bytes(MODULE, &bytes)
        .with_value("templates/html/fancy.yaml", json!({ "output": "html", "engine": "content-only", "accepts": "all" }));
    let (host, log) = host();
    let identity = ExtensionIdentity { identity: STEM.to_string(), stem: STEM.to_string() };
    let manifest = Manifest::read(STEM, &common::wasm_manifest(MODULE)).expect("the manifest reads");
    let loaded = match WasmEngine::new(host.clone()).load(&identity, &manifest, Box::new(files)) {
        Ok(loaded) => loaded,
        Err(errors) => panic!("the extension loads: {:?}", errors.iter().map(|e| e.detail().to_string()).collect::<Vec<_>>()),
    };
    assert_eq!(names(&log), ["define", "packages"]);
    assert_eq!(args_of(&log, "define"), json!(["wc", "guest.wasm"]));
    assert_eq!(args_of(&log, "packages"), json!([]));
    let [report] = loaded.packages.as_slice() else { panic!("one package") };
    assert_eq!(report.name, "_");
    assert_eq!(report.uses.len(), 1);
    assert_eq!(report.templates.iter().map(|t| t.name.as_str()).collect::<Vec<_>>(), ["fancy"]);
    assert_eq!(report.host, ["files"]);
    assert_eq!(report.features.iter().map(|f| (f.name.as_str(), f.title.as_str())).collect::<Vec<_>>(), [("words", "Words"), ("count", "")]);
    assert!(!report.block.features.is_empty());
    // The instance created at load is kept on this thread.
    assert_eq!(host.instances_on_this_thread(), 1);
}

#[test]
fn the_digest_changes_with_the_module_s_bytes() {
    let digest = |script: &Script| {
        let bytes = script.bytes();
        let files = MemoryFiles::new().with_bytes(MODULE, &bytes);
        let identity = ExtensionIdentity { identity: STEM.to_string(), stem: STEM.to_string() };
        let manifest = Manifest::read(STEM, &common::wasm_manifest(MODULE)).expect("the manifest reads");
        let Ok(loaded) = WasmEngine::new(host().0).load(&identity, &manifest, Box::new(files)) else {
            panic!("the extension loads")
        };
        (bytes, loaded.runtime.digest_input())
    };
    let (bytes, first) = digest(&script(Step::new("register"), vec![]));
    let mut expected = format!("flm-ext wasm {ENGINE_VERSION}").into_bytes();
    expected.push(0);
    expected.extend_from_slice(&bytes);
    expected.push(0);
    assert_eq!(first, expected);
    let (_, second) = digest(&script(Step::new("register").burn(1), vec![]));
    assert_ne!(first, second);
}

// --- the thirteen handler rows -----------------------------------------------------------------

#[test]
fn rows_0_and_1_register_lends_a_stage_whose_run_writes_its_table() {
    let (host, log) = host();
    let registry = registry(&host, &script(stage_register(), vec![stage_run()]));
    let register = args_of(&log, "register");
    assert_eq!((&register[0], &register[1], &register[2]), (&json!("_"), &json!("words"), &json!({})));
    assert_eq!(kind(&register[3]), "feature_builder");
    assert!(replies(&log).iter().any(|reply| kind(&reply["ok"]) == "stage"), "{:?}", replies(&log));
    let run = run_demanding(&registry, &["wc.seen"], "One tree.");
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(common::ext_items(&run, "wc.seen"), [json!({ "saw": "it" })]);
    let run_stage = args_of(&log, "run_stage");
    assert_eq!((&run_stage[0], &run_stage[1], &run_stage[2]), (&json!("_"), &json!("words"), &json!("wc.see")));
    assert_eq!((kind(&run_stage[3]), kind(&run_stage[4])), ("stage_cx", "stage_out"));
}

#[test]
fn row_2_an_expansion_hook_lent_from_register_expands_its_element() {
    let register = Step::new("register")
        .request(r#"{"native":"module_handle","path":"stdcore/features/userdefs"}"#)
        .request(
            r#"{"native":"call","handle":$R,"method":"expand_in_tree_rule_for","args":["wc.badge",{"kind":"macro","name":"badge","expand":"always"}]}"#,
        )
        .request(r#"{"native":"call","handle":$H1,"method":"lowering_rule","args":[$R]}"#)
        .request(r#"{"native":"lend_expand","element":"wc.badge","policy":"always"}"#)
        .request(
            r#"{"native":"call","handle":$H1,"method":"element","args":[{"names":["badge"],"syntax":{"macro":{"args":[]}},"element":{"name":"wc.badge","aspects":[{"Expand":$R}]}}]}"#,
        );
    let expand = Step::new("expand")
        .repeat()
        .request(r#"{"native":"call","handle":$H2,"method":"stage_chars","args":["HI"]}"#)
        .ok(r#"{"emit":[$R]}"#);
    let (host, log) = host();
    let registry = registry(&host, &script(register, vec![expand]));
    let (text, diagnostics) = common::text_of(&registry, "A \\badge{} here.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A HI here.\n");
    let args = args_of(&log, "expand");
    assert_eq!(args[2], "wc.badge");
    assert_eq!((kind(&args[3]), kind(&args[4]), kind(&args[5])), ("node", "expand_cx", "expander"));
}

/// The `register` of `\wcbox{content}\label{…}`, its element `wc.box`
/// lowered to its content by a lent rule body, with a lent `Referenceable`
/// and a lent `Sectioning` attached.
fn box_register() -> Step {
    let refs = module("stdcore/features/refs");
    let lowering = module("core/lowering");
    Step::new("register")
        .request(&format!(r#"{{"native":"call","handle":{refs},"method":"label_fields_arg","args":[]}}"#))
        .request(
            r#"{"native":"call","handle":$H1,"method":"element","args":[{"names":["wcbox"],"syntax":{"macro":{"args":["{",$R]}},"element":{"name":"wc.box"}}]}"#,
        )
        .request(r#"{"native":"lend_lower","rule":"wc.box.lower.default"}"#)
        .request(&format!(
            r#"{{"native":"call","handle":{lowering},"method":"LoweringRule::new","args":["wc.box.lower.default","wc.box",{{}},$R,"everywhere"]}}"#
        ))
        .request(r#"{"native":"call","handle":$H1,"method":"lowering_rule","args":[$R]}"#)
        .request(r#"{"native":"lend_aspect","kind":"Referenceable","element":"wc.box","methods":["points"]}"#)
        .request(r#"{"native":"call","handle":$H1,"method":"attach","args":["wc.box",{"Referenceable":$R}]}"#)
        .request(r#"{"native":"lend_aspect","kind":"Sectioning","element":"wc.box","methods":["level","in_toc","toc_title"]}"#)
        .request(r#"{"native":"call","handle":$H1,"method":"attach","args":["wc.box",{"Sectioning":$R}]}"#)
}

/// The box element's handlers: the rule's body, `points`, the three
/// sectioning methods.
fn box_handlers() -> Vec<Step> {
    let refs = module("stdcore/features/refs");
    vec![
        Step::new("lower").repeat().request(r#"{"native":"call","handle":$H3,"method":"lower_arg","args":[$H2,$H1,0]}"#).ok(r#"{"emit":$R}"#),
        Step::new("points")
            .repeat()
            .request(&format!(r#"{{"native":"call","handle":{refs},"method":"label_uses","args":[$H1]}}"#))
            .request(&format!(r#"{{"native":"call","handle":{refs},"method":"point_labels","args":[$R]}}"#))
            .ok(r#"[{"kind":"wcbox","labels":$R0,"malformed":$R1,"anchor":"from_first_label","content":{"argument":0}}]"#),
        Step::new("level").repeat().ok("2"),
        Step::new("in_toc").repeat().ok("true"),
        Step::new("toc_title").repeat().ok(r#"{"argument":0}"#),
    ]
}

#[test]
fn rows_3_9_and_10_a_rule_s_body_points_and_sectioning_lent_from_register() {
    let (host, log) = host();
    let registry = registry(&host, &script(box_register(), box_handlers()));
    let (text, diagnostics) = common::text_of(&registry, "\\wcbox{Alpha}\\label{ex:one} See \\ref{ex:one}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Alpha See Alpha.\n");
    let lower = args_of(&log, "lower");
    assert_eq!(lower[2], "wc.box.lower.default");
    assert_eq!((kind(&lower[3]), kind(&lower[4]), kind(&lower[5])), ("node", "lower_cx", "lowerer"));
    let points = args_of(&log, "points");
    assert_eq!((&points[2], kind(&points[3])), (&json!("wc.box"), "node"));
    assert_eq!(toc_entries(&registry, "\\wcbox{Intro} text \\wcbox{More}"), [(2, true), (2, true)]);
    for method in ["level", "in_toc", "toc_title"] {
        let args = args_of(&log, method);
        assert_eq!((&args[2], kind(&args[3])), (&json!("wc.box"), "node"), "{method}");
    }
}

#[test]
fn row_4_a_rewrite_stage_lent_from_register_restages() {
    let register = Step::new("register")
        .request(
            r#"{"native":"lend_rewrite","decl":{"name":"wc.dash","publishes":"wc.dashed","docs":"Descends."},"needs":{"after":["flm.userdefs"],"before":["flm.blocks.structured"]}}"#,
        )
        .request(r#"{"native":"call","handle":$H1,"method":"rewrite_stage","args":[$R]}"#);
    let restage = Step::new("restage").repeat().ok(r#""descend""#);
    let (host, log) = host();
    let registry = registry(&host, &script(register, vec![restage]));
    let run = run_demanding(&registry, &["wc.dashed"], "One tree.");
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let args = args_of(&log, "restage");
    assert_eq!(args[2], "wc.dash");
    assert_eq!((kind(&args[3]), kind(&args[4]), kind(&args[5])), ("node", "rewrite_cx", "restager"));
}

#[test]
fn row_5_a_citation_provider_lent_from_register_answers_the_requests() {
    let register = Step::new("register")
        .request(r#"{"native":"lend_provider","name":"wc.bib"}"#)
        .request(r#"{"native":"call","handle":$H1,"method":"peer_slot","args":["flm.citations.providers",$R]}"#);
    let answer = Step::new("answer_all").repeat().ok(r#"[{"error":"the guest holds no entry"}]"#);
    let (host, log) = host();
    let registry = registry(&host, &script(register, vec![answer]));
    let (_, diagnostics) = common::text_of(&registry, "See \\cite{knuth84}.");
    assert!(diagnostics.iter().any(|d| d.contains("the guest holds no entry")), "{diagnostics:?} {:?} {:?}", names(&log), replies(&log));
    let args = args_of(&log, "answer_all");
    assert_eq!(args[2], "wc.bib");
    assert_eq!(args[3].as_array().map(Vec::len), Some(1), "{args}");
    assert_eq!(kind(&args[4]), "provider_cx");
}

#[test]
fn rows_6_7_and_8_a_template_module_describes_itself_inside_register_then_ships_and_renders() {
    use flm_core::model::options::OptionValues;
    let register = Step::new("register")
        .request(r#"{"native":"lend_template","name":"tiny"}"#)
        .request(r#"{"native":"call","handle":$H1,"method":"peer_slot","args":["flm.compose.templates",$R]}"#);
    let handlers = vec![
        Step::new("template_describe").repeat().ok(r#"{"output":"html","accepts":"all","requires":[],"config":{}}"#),
        Step::new("template_files").repeat().ok(r#"[{"name":"style","extension":"css","media_type":"text/css","text":"h1 {}\n"}]"#),
        Step::new("template_render").repeat().ok(r#""<html>tiny</html>\n""#),
    ];
    let (host, log) = host();
    let package = factory(&host, &script(register, handlers)).build(&PackageSettings::new()).expect("the package builds");
    let registry = compose_registry(package).expect("the registry builds");
    // The description runs inside `register`, on the loading thread's instance.
    assert_eq!(names(&log), ["define", "packages", "register", "template_describe"]);
    assert_eq!(args_of(&log, "template_describe"), json!(["_", "words", "tiny", {}]));
    let options = ComposeOptions {
        template: TemplateChoice::Name("tiny".to_string()),
        template_config: [("html".to_string(), [("tiny".to_string(), serde_json::Map::new())].into_iter().collect())]
            .into_iter()
            .collect(),
    };
    let overrides =
        Overrides { prefer: vec![stage_name("html").into()], render_options: OptionValues::new().with(options), ..Overrides::none() };
    let source = Arc::new(FlmSource::new("Some text.").with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Arc::new(Plan::for_workflow(&registry, html::WORKFLOW, &overrides).expect("a plan"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let page = run.table(html::DOCUMENT).expect("declared").and_then(|set| Some(String::from_utf8_lossy(set.entry()?.as_bytes()?).into_owned()));
    assert_eq!(page.as_deref(), Some("<html>tiny</html>\n"));
    for entry in ["template_files", "template_render"] {
        let args = args_of(&log, entry);
        assert_eq!(args[2], "tiny", "{entry}");
        assert!(args[3].is_object(), "{entry}: {args}");
        assert_eq!(kind(&args[4]), "template_cx", "{entry}");
    }
}

/// The `register` of a feature registering a late-initialization hook and a
/// finalizer, both lent.
fn hooks_register() -> Step {
    Step::new("register")
        .request(r#"{"native":"lend_late_init","index":0}"#)
        .request(r#"{"native":"call","handle":$H1,"method":"late_init","args":[$R]}"#)
        .request(r#"{"native":"lend_finalizer","index":0}"#)
        .request(r#"{"native":"call","handle":$H1,"method":"finalizer","args":[$R]}"#)
}

/// The hooks: the late-initialization hook pushes a warning; the finalizer
/// declares the table `wc.late` and the stage `wc.fill`, which fills it.
fn hooks_handlers() -> Vec<Step> {
    vec![
        Step::new("late_init").request(
            r#"{"native":"call","handle":$H2,"method":"push","args":[{"id":"wc.late","severity":"warning","message":"the language is built"}]}"#,
        ),
        Step::new("finalize")
            .request(r#"{"native":"call","handle":$H1,"method":"table","args":["wc.late","the second round's table"]}"#)
            .request(
                r#"{"native":"lend_stage","decl":{"name":"wc.fill","effect":"compute","scope":"document","writes":["wc.late"]},"needs":{}}"#,
            )
            .request(r#"{"native":"call","handle":$H1,"method":"stage","args":[$R]}"#),
        Step::new("run_stage")
            .request(r#"{"native":"call","handle":$H2,"method":"table","args":["wc.late"]}"#)
            .request(r#"{"native":"call","handle":$R,"method":"push","args":[{"filled":true}]}"#),
    ]
}

#[test]
fn rows_11_and_12_hooks_lent_from_register_and_the_rehearsal_on_a_worker_thread() {
    let (host, log) = host();
    let registry = registry(&host, &script(hooks_register(), hooks_handlers()));
    let found: Vec<String> =
        registry.build_diagnostics().iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect();
    assert_eq!(found, ["wc.late: the language is built"]);
    assert_eq!(names(&log), ["define", "packages", "register", "finalize", "late_init"]);
    let late_init = args_of(&log, "late_init");
    assert_eq!((&late_init[2], kind(&late_init[3]), kind(&late_init[4])), (&json!(0), "built_lang", "flm_diagnostics"));
    let finalize = args_of(&log, "finalize");
    assert_eq!((&finalize[2], kind(&finalize[3])), (&json!(0), "feature_builder"));
    // The stage the finalizer declared runs on a worker thread, whose own
    // instance runs the script from its start: `define`, then the rehearsal
    // with the registration's configuration, then the stage.
    let before = names(&log).len();
    let (items, instances) = on_worker(|| {
        let run = run_demanding(&registry, &["wc.late"], "One tree.");
        (common::ext_items(&run, "wc.late"), host.instances_on_this_thread())
    });
    assert_eq!(items, [json!({ "filled": true })]);
    assert_eq!(instances, 1);
    assert_eq!(names(&log)[before..], ["define", "rehearse", "run_stage"]);
    assert_eq!(args_of(&log, "rehearse"), json!(["_", "words", {}]));
}

// --- the nested entry, the limits, the failures -------------------------------------------------

#[test]
fn a_nested_entry_has_fuel_of_its_own_and_the_outer_call_s_is_restored_after_it() {
    // `register` contributes a template module, whose `describe` runs nested
    // inside `register`'s request and spends all its fuel. The outer call
    // then runs on with the fuel it had left, and finishes.
    let nesting = |register: Step| {
        register
            .request(r#"{"native":"lend_template","name":"tiny"}"#)
            .request(r#"{"native":"call","handle":$H1,"method":"peer_slot","args":["flm.compose.templates",$R]}"#)
    };
    let describe = Step::new("template_describe").spin();
    let (host, log) =
        host_with(WasmSettings { fuel: Some(1_000_000), ..WasmSettings::default() });
    let register = nesting(Step::new("register")).burn(1000);
    let package =
        factory(&host, &script(register, vec![describe.clone()])).build(&PackageSettings::new()).expect("the package builds");
    let built = compose_registry(package);
    assert_eq!(names(&log), ["define", "packages", "register", "template_describe"]);
    // The nested call's failure is answered to `register`'s request, which
    // the script ignores; `register` itself finishes.
    let reply = replies(&log).last().cloned().unwrap_or_default();
    assert!(
        reply["err"]["detail"].as_str().is_some_and(|detail| detail.contains("must finish within its budget of 1000000 fuel units")),
        "{reply}"
    );
    assert!(built.is_ok(), "{:?}", built.err());
    // The instance the nested call poisoned is dropped once the outermost
    // call leaves it.
    assert_eq!(host.instances_on_this_thread(), 0);
    // The outer call gets its remainder back, not a fresh budget: one burn
    // fits in one call's fuel and two do not, so a burn before the nested
    // call and one after it exhaust `register`'s fuel.
    let burn = 60_000;
    let (host, _) = host_with(WasmSettings { fuel: Some(1_000_000), ..WasmSettings::default() });
    let register = nesting(Step::new("register").burn(burn)).burn(burn);
    let package = factory(&host, &script(register, vec![describe])).build(&PackageSettings::new()).expect("the package builds");
    let error = compose_registry(package).expect_err("`register` runs out of fuel");
    assert!(
        error.contains(
            "`register` of the feature `words` of the package `_` of the extension `wc` must finish within its budget of \
             1000000 fuel units"
        ),
        "{error}"
    );
}

#[test]
fn the_limits_and_a_trap_fail_the_handler_with_their_text_and_data_and_drop_the_instance() {
    let what = "the stage `wc.see` of the feature `words` of the package `_` of the extension `wc`";
    let cases: Vec<(Step, WasmSettings, String, Value)> = vec![
        (
            Step::new("run_stage").spin(),
            WasmSettings { fuel: Some(100_000), ..WasmSettings::default() },
            format!("{what} must finish within its budget of 100000 fuel units"),
            json!({ "limit": "fuel", "fuel": 100_000 }),
        ),
        (
            Step::new("run_stage").grow(),
            WasmSettings { memory_limit: Some(2 << 20), ..WasmSettings::default() },
            format!("{what} must stay within the memory limit of 2097152 bytes"),
            json!({ "limit": "memory", "limit_bytes": 2_097_152 }),
        ),
        (
            Step::new("run_stage").trap(),
            WasmSettings::default(),
            format!("{what} must run without a trap: wasm `unreachable` instruction executed"),
            json!({ "trap": "wasm `unreachable` instruction executed", "panic": null }),
        ),
        (
            Step::new("run_stage").panic("index out of bounds"),
            WasmSettings::default(),
            format!(
                "{what} must run without a trap: wasm `unreachable` instruction executed; the guest panicked: index out \
                 of bounds"
            ),
            json!({ "trap": "wasm `unreachable` instruction executed", "panic": "index out of bounds" }),
        ),
    ];
    for (run_stage, settings, text, data) in cases {
        let (host, _) = host_with(settings);
        let registry = registry(&host, &script(stage_register(), vec![run_stage]));
        let (data_found, instances) = on_worker(|| {
            let run = run_demanding(&registry, &["wc.seen"], "One tree.");
            (data_of(&run, "flm.ext.handler-failed"), host.instances_on_this_thread())
        });
        assert_eq!(data_found["detail"], text);
        assert_eq!(data_found["data"], data, "{text}");
        assert_eq!(instances, 0, "{text}");
    }
}

#[test]
fn an_err_reply_is_answered_as_the_guest_s_own_error() {
    let error = ExtError::NameOutsideExtension {
        what: "table".to_string(),
        name: "seen".to_string(),
        package: "_".to_string(),
        extension: "wc".to_string(),
    };
    let envelope = json!({ "err": error.data(), "message": error.to_string() }).to_string();
    let (host, _) = host();
    let registry = registry(&host, &script(stage_register(), vec![Step::new("run_stage").reply(&envelope)]));
    let (data, instances) = on_worker(|| {
        let run = run_demanding(&registry, &["wc.seen"], "One tree.");
        (data_of(&run, "flm.ext.handler-failed"), host.instances_on_this_thread())
    });
    assert_eq!(data, error.data());
    // A handler's own error poisons nothing.
    assert_eq!(instances, 1);
}

#[test]
fn a_request_the_host_cannot_read_is_answered_with_an_err() {
    let deep = format!(r#"{}{}"#, "[".repeat(200), "]".repeat(200));
    let register = Step::new("register")
        .request(r#"{"native":"nope"}"#)
        .request("{")
        .request(&deep)
        .request(r#"{"native":"console","level":"info","text":"still here"}"#);
    let define = Step::new("define")
        .request(r#"{"native":"lend_stage","decl":{"name":"wc.see","effect":"compute","scope":"document"},"needs":{}}"#);
    let (host, log) = host();
    let steps = vec![define, Step::new("packages").ok(PACKAGES), register];
    registry(&host, &Script::new(steps));
    let details: Vec<String> =
        replies(&log).iter().map(|reply| reply["err"]["detail"].as_str().unwrap_or("ok").to_string()).collect();
    assert_eq!(
        details[0],
        "the extension `wc` must declare a stage from a feature's `register` or from a handler that `register` declared"
    );
    assert!(details[1].starts_with("a request of the extension `wc` must be one of the requests the host answers: "), "{details:?}");
    assert!(details[2].starts_with("a request of the extension `wc` must be JSON text: "), "{details:?}");
    assert_eq!(
        details[3],
        "a request of the extension `wc` must nest lists and maps at most 127 levels deep (a presentation tree about 63 \
         levels deep)"
    );
    assert_eq!(details[4], "ok");
    assert!(log.lock().expect("the log").iter().any(|line| line == "still here"));
}

#[test]
fn strict_mode_gives_every_outermost_call_a_fresh_instance() {
    let (host, log) = host_with(WasmSettings { strict: true, ..WasmSettings::default() });
    let registry = registry(&host, &script(stage_register(), vec![stage_run()]));
    // `register` ran on a fresh instance, which ran `define` first.
    assert_eq!(names(&log), ["define", "packages", "define", "register"]);
    assert_eq!(host.instances_on_this_thread(), 0);
    let before = names(&log).len();
    on_worker(|| run_demanding(&registry, &["wc.seen"], "One tree."));
    assert_eq!(names(&log)[before..], ["define", "rehearse", "run_stage"]);
}

#[test]
fn each_thread_has_its_own_instance_which_runs_the_script_from_its_start() {
    let (host, log) = host();
    let registry = registry(&host, &script(stage_register(), vec![stage_run()]));
    assert_eq!(host.instances_on_this_thread(), 1);
    for _ in 0..2 {
        let before = names(&log).len();
        let items = on_worker(|| common::ext_items(&run_demanding(&registry, &["wc.seen"], "One tree."), "wc.seen"));
        assert_eq!(items, [json!({ "saw": "it" })]);
        assert_eq!(names(&log)[before..], ["define", "rehearse", "run_stage"]);
    }
    host.discard_instances_on_this_thread();
    assert_eq!(host.instances_on_this_thread(), 0);
}

// --- the module page ---------------------------------------------------------------------------

#[test]
fn the_module_page_has_the_sections_of_the_quickjs_page() {
    let page = include_str!("../src/wasm/mod.rs");
    for heading in [
        "# What the binding is",
        "# An extension in WebAssembly",
        "# Loading and the instances",
        "# The mapping rules",
        "# The limits",
        "# Strict mode",
        "# The rehearsal",
        "# Failures",
        "# The digest",
    ] {
        assert!(page.contains(heading), "the module page lacks {heading:?}");
    }
}
