//! A collection loaded by a program: the module of the guest crate
//! `tests/wasm/collection/`, which holds the extensions `keep` and `show` and the
//! manifest of each in its section `flmextensions`, read with `Collection::read` and
//! given to the engine `wasm`, its rows found by a source the test makes from them. The
//! two extensions run in one instance on a thread: a value that one keeps in the
//! module's memory is read by the other; after a handler of one traps, a handler of the
//! other answers on its next call; the memory limit of the instance is the sum of the
//! limits of the two. The module of a collection, named by a folder's manifest, fails its
//! load. Gated on `FLM_TESTS_WASM_GUEST=1` and the target being installed
//! (`common::guest`); otherwise each test writes one `skipped:` line.

#![cfg(feature = "wasm")]

mod common;

use std::path::Path;
use std::sync::Arc;

use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_ext::adapter::values::diagnostic_value_to_json;
use flm_ext::wasm::{Collection, WasmEngine, WasmHost, WasmSettings};
use flm_ext::{ExtensionLoader, Manifest, MemoryFiles, MemorySource};
use serde_json::{json, Value};

use common::{factory_of, registry_with};

/// The bytes of the module of the guest crate `tests/wasm/collection/`, or `None` (a
/// `skipped:` line written) when the gated tests cannot run here.
fn module() -> Option<Vec<u8>> {
    common::guest::module(Path::new(env!("CARGO_MANIFEST_DIR")), "collection")
}

/// The manifest of the extension `stem` of the collection, as JSON: that of
/// `tests/wasm/collection/STEM/flmextension.yaml`.
fn manifest(stem: &str) -> Value {
    let mut wasm = json!({ "module": "./collection.wasm" });
    if stem == "keep" {
        wasm["limits"] = json!({ "memory_mib": 8 });
    }
    json!({ "extension": { "engine": "wasm", "wasm": wasm } })
}

/// The registry of the packages of the extensions `stems` of `collection`, found by a
/// source made from its rows and loaded with `host`.
fn registry(host: &WasmHost, collection: &Collection, stems: &[&str]) -> Arc<FlmRegistry> {
    let source = collection.extensions().iter().fold(MemorySource::new(), |source, row| {
        let files = row.files().iter().fold(MemoryFiles::new(), |files, file| files.with_bytes(file.path(), file.bytes()));
        source.with(row.stem(), row.identity(), manifest(row.stem()), files)
    });
    let engine = WasmEngine::new(host.clone())
        .with_collection(collection.clone(), |row| Manifest::read(row.stem(), &manifest(row.stem())).ok());
    let loader = Arc::new(ExtensionLoader::new(vec![Arc::new(source)]).with_engine(Arc::new(engine)));
    let packages = stems.iter().map(|stem| factory_of(&loader, stem).build(&PackageSettings::new()).expect("the package builds"));
    registry_with(packages)
}

/// The collection of the guest crate, read under the label `tests`, and the registry of
/// its extensions `stems` loaded with a host of `settings`; `None` when the gated tests
/// cannot run here.
fn loaded(settings: WasmSettings, stems: &[&str]) -> Option<(WasmHost, Arc<FlmRegistry>)> {
    let collection = Collection::read("tests", module()?).expect("the collection reads");
    let identities: Vec<&str> = collection.extensions().iter().map(|row| row.identity()).collect();
    assert_eq!(identities, ["tests:rust/keep", "tests:rust/show"]);
    let host = WasmHost::new(settings);
    let registry = registry(&host, &collection, stems);
    Some((host, registry))
}

/// The run of a plan demanding the table `demand` over a document of one paragraph, its
/// stages run on the calling thread.
fn run_demanding(registry: &Arc<FlmRegistry>, demand: &str) -> Run {
    let plan = Arc::new(Plan::build(registry, &PlanRequest::new().demand(demand)).expect("the plan builds"));
    let source = Arc::new(FlmSource::new("One tree.").with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the tree parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::sequential()).expect("the run executes");
    run
}

/// The data of the diagnostic of `run` whose identifier is `id`, as JSON (`null` when
/// it has none).
fn data_of(run: &Run, id: &str) -> Value {
    run.diagnostics()
        .iter()
        .find(|d| d.identifier() == id)
        .map(|d| diagnostic_value_to_json(&d.data().serializable_data()))
        .unwrap_or(Value::Null)
}

/// The note of the stage `show.read` after `run`.
fn shown(run: &Run) -> Option<String> {
    common::diagnostics(run).into_iter().find(|line| line.starts_with("show.value: "))
}

/// `f` run on a thread marked as a worker of `flm-core`, on which a run executes its
/// stages itself.
fn on_worker<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    flm_core::lang::guard::run_with_worker_stack(f).expect("the worker thread starts")
}

#[test]
fn two_extensions_of_one_collection_run_in_one_instance() {
    let Some((host, registry)) = loaded(WasmSettings::default(), &["keep", "show"]) else { return };
    let (note, instances) = on_worker(|| {
        let kept = run_demanding(&registry, "keep.kept");
        assert!(common::diagnostics(&kept).iter().all(|line| !line.starts_with("flm.ext.")), "{:?}", common::diagnostics(&kept));
        let run = run_demanding(&registry, "show.shown");
        (shown(&run), host.instances_on_this_thread())
    });
    assert_eq!(note.as_deref(), Some("show.value: the value kept is ‘42’"));
    assert_eq!(instances, 1, "one instance runs both extensions");
}

#[test]
fn after_a_handler_of_one_extension_traps_the_other_answers_on_its_next_call() {
    let Some((host, registry)) = loaded(WasmSettings::default(), &["keep", "show"]) else { return };
    let (trap, instances, note) = on_worker(|| {
        run_demanding(&registry, "keep.kept");
        assert!(shown(&run_demanding(&registry, "show.shown")).is_some());
        let trapped = run_demanding(&registry, "keep.trapped");
        let trap = data_of(&trapped, "flm.ext.handler-failed");
        let instances = host.instances_on_this_thread();
        let run = run_demanding(&registry, "show.shown");
        (trap, instances, shown(&run))
    });
    assert!(trap["data"]["panic"].as_str().is_some_and(|panic| panic.starts_with("the stage traps")), "{trap}");
    assert_eq!(instances, 0, "the poisoned instance is dropped after the call");
    // A fresh instance, in which `show` is defined and rehearsed again.
    assert_eq!(note.as_deref(), Some("show.value: the value kept is ‘0’"));
}

#[test]
fn the_memory_limit_of_a_collection_s_instance_is_the_sum_of_the_limits_of_its_extensions() {
    // `keep`'s manifest sets 8 MiB; `show`'s sets none, so the host's 16 MiB is its limit.
    // `show` counts although only `keep` is loaded.
    let settings = WasmSettings { memory_limit: Some(16 << 20), ..WasmSettings::default() };
    let Some((_host, registry)) = loaded(settings, &["keep"]) else { return };
    let data = on_worker(|| data_of(&run_demanding(&registry, "keep.grown"), "flm.ext.handler-failed"));
    assert_eq!(data["data"], json!({ "limit": "memory", "limit_bytes": 24 << 20 }));
}

#[test]
fn a_module_that_holds_the_section_fails_its_load_when_a_folder_s_manifest_names_it() {
    let Some(bytes) = module() else { return };
    let files = MemoryFiles::new().with_bytes("collection.wasm", &bytes);
    let source = MemorySource::new().with("keep", "keep", common::wasm_manifest("./collection.wasm"), files);
    let loader = common::wasm_loader(&WasmHost::new(WasmSettings::default()), source);
    let problems = common::load_problems(&loader, "keep");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("flmextensions"), "{problems:?}");
}
