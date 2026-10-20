//! The engine limits an extension declares in its manifest
//! (`extension.js.limits`) reach every engine instance of the extension: a
//! recursion in a stage that exceeds the host's default JavaScript stack runs
//! under `stack_mib: 16`.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;

use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageEntry};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_ext::adapter::table::ExtTable;
use flm_ext::quickjs::{JsHost, JsSettings};
use serde_json::{json, Value};

use common::{factory_of, js_extension, js_loader, registry_with};

/// The depth of the recursion. Measured in the test profile (unoptimized) on
/// a worker thread of this test, with a recursion that records its depth
/// until the `RangeError`: about 110 calls under the default 1 MiB of
/// JavaScript stack, about 1,800 under 16 MiB. A thousand sits between the
/// two with room on either side (an optimized build allows deeper
/// recursions under both, so the first half of the test is the one a faster
/// build could break; it would then need a larger depth).
const DEPTH: u32 = 1_000;

/// The main module: a document stage that recurses `DEPTH` calls deep and
/// pushes the depth it reached into `deep.depth`.
fn main_module() -> String {
    format!(
        r#"import {{ StageDecl, Effect, Scope }} from "flm:core/stage";
function down(n) {{ return n === 0 ? 0 : 1 + down(n - 1); }}
export default function (ext) {{
  ext.package("_", {{ features: {{ deep: {{ register(b, settings) {{
    b.table("deep.depth", "how deep the stage recursed");
    b.stage({{
      decl: () => new StageDecl("deep.recurse", Effect.Compute, Scope.Document).writes("deep.depth"),
      run(cx, out) {{ out.table("deep.depth").push(down({DEPTH})); }},
    }});
  }} }} }} }});
}}
"#
    )
}

/// The package of the extension `deep` whose manifest's `extension.js`
/// holds `limits` (none when `null`), loaded with a default host.
fn package(limits: Value) -> PackageEntry {
    let mut js = json!({ "main": "main.js" });
    if !limits.is_null() {
        js["limits"] = limits;
    }
    let manifest = json!({ "extension": { "engine": "js", "js": js } });
    let main = main_module();
    let host = JsHost::new(JsSettings::default());
    let loader = js_loader(&host, js_extension("deep", &[("main.js", main.as_str())], manifest));
    factory_of(&loader, "deep").build(&PackageSettings::new()).expect("the package builds")
}

/// The sequential run, on this thread, of a plan demanding `deep.depth`: the
/// diagnostics and the depth pushed.
fn run_plan(registry: &Arc<FlmRegistry>) -> (Vec<String>, Vec<Value>) {
    let source = Arc::new(FlmSource::new("Text.").with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Arc::new(Plan::build(registry, &PlanRequest::new().demand("deep.depth")).expect("the plan builds"));
    let mut run: Run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::sequential()).expect("the run executes");
    let items = run
        .table(TableHandle::<ExtTable>::new("deep.depth"))
        .expect("declared")
        .map(|table| table.items().to_vec())
        .unwrap_or_default();
    (common::diagnostics(&run), items)
}

/// `f` on a new worker thread (64 MiB of real stack, no engine instance yet).
fn on_new_worker<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn_scoped(scope, || flm_core::lang::guard::run_with_worker_stack(f).expect("the worker thread starts"))
            .expect("the thread starts")
            .join()
            .expect("the thread runs")
    })
}

/// The run of a plan demanding `deep.depth`, on a new worker thread: the
/// diagnostics and the depth pushed.
fn recurse(registry: &Arc<FlmRegistry>) -> (Vec<String>, Vec<Value>) {
    on_new_worker(|| run_plan(registry))
}

#[test]
fn a_recursion_that_fails_under_the_default_stack_runs_under_the_extension_s_limit() {
    let (diagnostics, items) = recurse(&registry_with([package(Value::Null)]));
    assert!(items.is_empty(), "{items:?}");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].starts_with("flm.ext.handler-failed: the stage `deep.recurse` failed"), "{diagnostics:?}");

    let (diagnostics, items) = recurse(&registry_with([package(json!({ "stack_mib": 16 }))]));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(items, [json!(DEPTH)]);
}

/// The same thread loads the extension, builds the registry and runs the
/// document (a sequential run on a worker thread): the engine instance created
/// at load is the one every stage runs on, and it has the extension's stack.
#[test]
fn the_extension_s_stack_limit_holds_when_one_worker_thread_loads_and_runs() {
    let (diagnostics, items) = on_new_worker(|| run_plan(&registry_with([package(json!({ "stack_mib": 16 }))])));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(items, [json!(DEPTH)]);

    let (diagnostics, items) = on_new_worker(|| run_plan(&registry_with([package(Value::Null)])));
    assert!(items.is_empty(), "{items:?}");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
}

#[test]
fn a_stack_limit_at_the_workers_stack_is_a_manifest_problem() {
    let manifest = json!({ "extension": { "engine": "js", "js": { "main": "main.js", "limits": { "stack_mib": 64 } } } });
    let main = main_module();
    let loader = js_loader(&JsHost::new(JsSettings::default()), js_extension("deep", &[("main.js", main.as_str())], manifest));
    let problems = common::load_problems(&loader, "deep");
    assert_eq!(
        problems,
        ["`extension.js.limits.stack_mib` of the extension `deep` must be below 64, the stack in MiB of the threads \
          that run the extension's handlers"]
    );
}

#[test]
fn the_limits_replace_the_host_s_settings_each_alone() {
    let host = JsSettings::default();
    let limits = flm_ext::manifest::JsLimits { memory_mib: Some(256), stack_mib: None, time_s: Some(60) };
    let settings = host.with_limits(&limits);
    assert_eq!(settings.memory_limit, Some(256 << 20));
    assert_eq!(settings.stack_size, host.stack_size);
    assert_eq!(settings.time_budget, Some(std::time::Duration::from_secs(60)));
}
