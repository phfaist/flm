//! Case 1 compiled from Rust to WebAssembly: the extension `wordcount` of
//! `tests/wasm/wordcount/` (the twin of `tests/js/wordcount/`, written against
//! `flm_ext::guest`) and the failing guest of `tests/wasm/failing/`, built for
//! `wasm32-unknown-unknown` and loaded through the engine `wasm`. The assertions
//! both forms of the crates share are in `tests/case1/` (the JavaScript case's
//! goldens, the settings, the hook's recovery, the rehearsal on a worker thread,
//! the hook answering an error); here also what is the engine `wasm`'s own: the
//! digest of the module's bytes, the trap of the hook that panics, a stage that
//! loops (the fuel limit), a stage that grows its memory (the memory limit), and
//! the time of the count against JavaScript. Gated on `FLM_TESTS_WASM_GUEST=1`
//! and the target being installed (`common::guest`); otherwise each test writes
//! one `skipped:` line.

#![cfg(all(feature = "wasm", feature = "guest"))]

mod case1;
mod common;

use std::path::Path;
use std::sync::Arc;

use flm_core::core::registry::{FlmRegistry, PackageEntry, PackageFactory};
use flm_core::core::settings::PackageSettings;
use flm_ext::wasm::{WasmEngine, WasmHost, WasmSettings, ENGINE_VERSION};
use flm_ext::{Engine, ExtensionIdentity, Manifest, MemoryFiles, MemorySource};
use serde_json::{json, Value};

use case1::{data_of, on_worker, run_demanding, Loads, PANIC_PREFIX};
use common::registry_with;

// --- helpers -----------------------------------------------------------------------------------

/// The path of the module in the extension's files; the manifests name it `./NAME.wasm`,
/// as `tests/wasm/*/flmextension.yaml` do.
fn module_path(name: &str) -> String {
    format!("{name}.wasm")
}

/// The module of the guest crate `name`, or `None` (a `skipped:` line written) when the
/// gated tests cannot run here.
fn module(name: &str) -> Option<Vec<u8>> {
    common::guest::module(Path::new(env!("CARGO_MANIFEST_DIR")), name)
}

/// The source of the extension `stem` whose module is `bytes`, found as `stem`, with the
/// manifest of `tests/wasm/STEM/flmextension.yaml` as JSON.
fn source(stem: &str, bytes: &[u8]) -> MemorySource {
    let files = MemoryFiles::new().with_bytes(&module_path(stem), bytes);
    MemorySource::new().with(stem, stem, common::wasm_manifest(&format!("./{}", module_path(stem))), files)
}

/// The factory of the extension `stem` of the module `bytes`, loaded with `host`.
fn factory(host: &WasmHost, stem: &str, bytes: &[u8]) -> PackageFactory {
    common::factory_of(&common::wasm_loader(host, source(stem, bytes)), stem)
}

/// The package of the extension `stem` of the module `bytes`, loaded with `host`, no
/// setting written.
fn package(host: &WasmHost, stem: &str, bytes: &[u8]) -> PackageEntry {
    factory(host, stem, bytes).build(&PackageSettings::new()).expect("the package builds")
}

/// The two guest crates built for WebAssembly, each loaded with a default host.
struct WasmCase;

impl Loads for WasmCase {
    fn factory(&self, stem: &str) -> PackageFactory {
        let bytes = module(stem).expect("the guest crates build when the case runs");
        factory(&WasmHost::new(WasmSettings::default()), stem, &bytes)
    }
}

/// The case, or `None` (a `skipped:` line written) when the gated tests cannot run here.
fn case() -> Option<WasmCase> {
    module("wordcount").map(|_| WasmCase)
}

// --- the assertions both forms share -------------------------------------------------------------

#[test]
fn case_one_renders_through_the_text_workflow() {
    let Some(case) = case() else { return };
    case1::case_one_renders_through_the_text_workflow(&case);
}

#[test]
fn case_one_renders_through_the_html_workflow() {
    let Some(case) = case() else { return };
    case1::case_one_renders_through_the_html_workflow(&case);
}

#[test]
fn case_one_s_tables_project_as_under_javascript() {
    let Some(case) = case() else { return };
    case1::case_one_s_tables_project_as_under_javascript(&case);
}

#[test]
fn case_one_s_registry_builds_without_a_diagnostic() {
    let Some(case) = case() else { return };
    case1::case_one_s_registry_builds_without_a_diagnostic(&case);
}

#[test]
fn a_shout_of_a_command_recovers_the_use_site_with_the_guest_s_diagnostic() {
    let Some(case) = case() else { return };
    case1::a_shout_of_a_command_recovers_the_use_site_with_the_guest_s_diagnostic(&case);
}

#[test]
fn the_rehearsal_declares_the_same_objects() {
    let Some(case) = case() else { return };
    case1::the_rehearsal_declares_the_same_objects(&case);
}

#[test]
fn register_receives_the_configuration_and_a_hook_on_a_worker_sees_the_same() {
    let Some(case) = case() else { return };
    case1::register_receives_the_configuration_and_a_hook_on_a_worker_sees_the_same(&case);
}

#[test]
fn the_count_stage_reads_both_options() {
    let Some(case) = case() else { return };
    case1::the_count_stage_reads_both_options(&case);
}

#[test]
fn the_count_s_note_appears_under_the_plain_workflows() {
    let Some(case) = case() else { return };
    case1::the_count_s_note_appears_under_the_plain_workflows(&case);
}

#[test]
fn a_hook_answering_an_error_fails_with_the_guest_s_error() {
    let Some(case) = case() else { return };
    case1::a_hook_answering_an_error_fails_with_the_guest_s_error(&case);
}

// --- the engine `wasm`'s own ---------------------------------------------------------------------

#[test]
fn the_digest_changes_with_the_module_s_bytes() {
    let Some(bytes) = module("wordcount") else { return };
    let digest = |bytes: &[u8]| {
        let files = MemoryFiles::new().with_bytes("wordcount.wasm", bytes);
        let identity = ExtensionIdentity { identity: "wordcount".to_string(), stem: "wordcount".to_string() };
        let manifest = Manifest::read("wordcount", &common::wasm_manifest("./wordcount.wasm")).expect("the manifest reads");
        let Ok(loaded) = WasmEngine::new(WasmHost::new(WasmSettings::default())).load(&identity, &manifest, Box::new(files))
        else {
            panic!("the extension loads")
        };
        loaded.runtime.digest_input()
    };
    let first = digest(&bytes);
    let mut expected = format!("flm-ext wasm {ENGINE_VERSION}").into_bytes();
    expected.push(0);
    expected.extend_from_slice(&bytes);
    expected.push(0);
    assert_eq!(first, expected);
    // The same module with a custom section appended (id 0, five bytes: the name `note`
    // of four bytes), which changes nothing it does.
    let mut noted = bytes.clone();
    noted.extend_from_slice(&[0, 5, 4, b'n', b'o', b't', b'e']);
    assert_ne!(digest(&noted), first);
}

#[test]
fn a_hook_that_panics_fails_with_the_panic_s_message() {
    let Some(case) = case() else { return };
    let (diagnostics, data) = case1::the_panicking_hook_s_failure(&case);
    // The failure as the expansion path words every binding's failure of a hook (the
    // JavaScript binding's time budget reads the same way), with the panic's message and
    // place appended.
    let expected = format!(
        "{PANIC_PREFIX}trap: wasm `unreachable` instruction executed; the guest panicked: the hook is not ready either, \
         at src/lib.rs:"
    );
    assert!(diagnostics.len() == 1 && diagnostics[0].starts_with(&expected), "{diagnostics:?}");
    assert_eq!(data["trap"], "wasm `unreachable` instruction executed");
    assert!(data["panic"].as_str().is_some_and(|panic| panic.starts_with("the hook is not ready either, at src/lib.rs:")));
}

/// The registry of the standard package and the failing guest's package, loaded with a
/// host of `settings`.
fn failing_registry(bytes: &[u8], settings: WasmSettings) -> (WasmHost, Arc<FlmRegistry>) {
    let host = WasmHost::new(settings);
    let registry = registry_with([package(&host, "failing", bytes)]);
    (host, registry)
}

/// The failure of the failing guest's stage that writes `table`, run on a worker thread
/// with a host of `settings`: the data of its diagnostic, and the instances left on the
/// thread.
fn failed_stage(bytes: &[u8], settings: WasmSettings, table: &str) -> (Value, usize) {
    let (host, registry) = failing_registry(bytes, settings);
    on_worker(|| {
        let run = run_demanding(&registry, &[table], &["One tree."]);
        (data_of(&run, "flm.ext.handler-failed"), host.instances_on_this_thread())
    })
}

#[test]
fn a_stage_that_loops_fails_with_the_fuel_limit() {
    let Some(bytes) = module("failing") else { return };
    let settings = WasmSettings { fuel: Some(20_000_000), ..WasmSettings::default() };
    let (data, instances) = failed_stage(&bytes, settings, "failing.looped");
    assert_eq!(
        data["detail"],
        "the stage `failing.loop` of the feature `failing` of the package `_` of the extension `failing` must finish \
         within its budget of 20000000 fuel units"
    );
    assert_eq!(data["data"], json!({ "limit": "fuel", "fuel": 20_000_000 }));
    assert_eq!(instances, 0, "the instance is dropped after the call");
}

#[test]
fn a_stage_that_grows_its_memory_fails_with_the_memory_limit() {
    let Some(bytes) = module("failing") else { return };
    let settings = WasmSettings { memory_limit: Some(16 << 20), ..WasmSettings::default() };
    let (data, instances) = failed_stage(&bytes, settings, "failing.grown");
    assert_eq!(
        data["detail"],
        "the stage `failing.grow` of the feature `failing` of the package `_` of the extension `failing` must stay \
         within the memory limit of 16777216 bytes"
    );
    assert_eq!(data["data"], json!({ "limit": "memory", "limit_bytes": 16_777_216 }));
    assert_eq!(instances, 0, "the instance is dropped after the call");
}

// --- the time of the count, against JavaScript (the report's figure) ---------------------------

/// The time of `rounds` runs of the count over the case document, on one worker thread,
/// after one run that creates the instance and rehearses `register` there; the items of
/// the last run.
#[cfg(feature = "quickjs")]
fn time_the_count(registry: &Arc<FlmRegistry>, rounds: u32) -> (std::time::Duration, Vec<Value>) {
    use flm_core::core::plan::{Plan, PlanRequest};
    use flm_core::core::run::{Externals, Run, RunOptions};
    use flm_core::model::options::OptionValues;
    let plan = Arc::new(Plan::build(registry, &PlanRequest::new().demand(case1::WORDS)).expect("the plan builds"));
    let doc = case1::doc(registry, &[case1::DOCUMENT], OptionValues::new());
    on_worker(|| {
        let once = || {
            let mut run = Run::new(Arc::clone(&doc), Arc::clone(&plan), Externals::none()).expect("a run");
            run.execute(&RunOptions::sequential()).expect("the run executes");
            case1::items(&run, case1::WORDS)
        };
        once();
        let start = std::time::Instant::now();
        let mut last = Vec::new();
        for _ in 0..rounds {
            last = once();
        }
        (start.elapsed() / rounds, last)
    })
}

/// The count's table is the JavaScript case's; the time of a run of the count under each
/// engine is written for the record (`--nocapture`), with the build it was measured in:
/// the checks and the battery run the debug build, in which `wasmi` itself is unoptimized,
/// so that its figure is not an optimized build's.
#[cfg(feature = "quickjs")]
#[test]
fn the_count_answers_as_under_javascript_and_its_time_is_written() {
    let Some(case) = case() else { return };
    const ROUNDS: u32 = 50;
    let (wasm, wasm_items) = time_the_count(&case1::registry_of(&case, "wordcount"), ROUNDS);
    let js_host = flm_ext::quickjs::JsHost::new(flm_ext::quickjs::JsSettings::default());
    let js_source = common::js_extension("wordcount", &common::WORDCOUNT_FILES, common::js_manifest("main.js"));
    let js_factory = common::factory_of(&common::js_loader(&js_host, js_source), "wordcount");
    let js_registry = registry_with([js_factory.build(&PackageSettings::new()).expect("the package builds")]);
    let (js, js_items) = time_the_count(&js_registry, ROUNDS);
    assert_eq!(wasm_items, js_items);
    let build = if cfg!(debug_assertions) { "the debug build" } else { "an optimized build" };
    eprintln!(
        "timing: a run of the stage `wordcount.count` over the case document in {build}, the mean of {ROUNDS}: \
         WebAssembly {wasm:?}, JavaScript {js:?}"
    );
}
