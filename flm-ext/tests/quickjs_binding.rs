//! The QuickJS binding: an extension loaded from JavaScript through the
//! engine `js`, the package objects its default export declares, its
//! `register` declaring tables and stages through the API modules, its stages
//! running through the glue and the dispatcher on the building thread and on
//! a worker thread, strict mode, and the failures.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::{Arc, Mutex};

use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageEntry, PackageFactory};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_ext::adapter::table::ExtTable;
use flm_ext::adapter::values::diagnostic_value_to_json;
use flm_ext::quickjs::{ConsoleLevel, JsHost, JsSettings};
use serde_json::{json, Value};

use common::{registry_with, try_registry_with};

// --- helpers -----------------------------------------------------------------------------------

/// The probe of plan B 8.2.
const COUNTER: &str = include_str!("js/counter.js");

/// A host with `settings`.
fn host(settings: JsSettings) -> JsHost {
    JsHost::new(settings)
}

/// The package `_` of the extension `wc` of the main module `main.js` (its
/// text `main`) and the modules `modules` (path, text), loaded with `host`;
/// or the load's problems.
fn try_load(host: &JsHost, main: &str, modules: &[(&str, &str)]) -> Result<PackageFactory, Vec<String>> {
    load_as(host, "wc", main, modules)
}

/// As [`try_load`], the extension's stem `stem`.
fn load_as(host: &JsHost, stem: &str, main: &str, modules: &[(&str, &str)]) -> Result<PackageFactory, Vec<String>> {
    let mut files = vec![("main.js", main)];
    files.extend_from_slice(modules);
    let loader = common::js_loader(host, common::js_extension(stem, &files, common::js_manifest("main.js")));
    match flm_core::core::registry::PackageResolver::resolve(&*loader, stem) {
        Ok(found) => Ok(found.expect("the extension is found")),
        Err(errors) => Err(errors.into_iter().map(|error| error.detail).collect()),
    }
}

/// The problems of loading the extension `wc` of the main module `main`.
fn problems(main: &str) -> Vec<String> {
    match try_load(&host(JsSettings::default()), main, &[]) {
        Ok(factory) => panic!("the extension loads: {}", factory.name()),
        Err(problems) => problems,
    }
}

/// The package of `main`, loaded with a default host and built from empty
/// settings.
fn package(main: &str) -> PackageEntry {
    package_with(&host(JsSettings::default()), main)
}

/// The package of `main`, loaded with `host` and built from empty settings.
fn package_with(host: &JsHost, main: &str) -> PackageEntry {
    let factory = try_load(host, main, &[]).unwrap_or_else(|error| panic!("the package loads: {error:?}"));
    factory.build(&PackageSettings::new()).expect("the package builds")
}

/// The counter probe's package (the extension `counter`), loaded with
/// `host` and built from empty settings.
fn counter_package(host: &JsHost) -> PackageEntry {
    let factory = load_as(host, "counter", COUNTER, &[]).unwrap_or_else(|error| panic!("the package loads: {error:?}"));
    factory.build(&PackageSettings::new()).expect("the package builds")
}

/// The run of a plan demanding `demands` over a document of one tree per text
/// of `texts`, in order.
///
/// The run is sequential ([`RunOptions::sequential`]): with `flm-core`'s
/// `parallel` feature on, a run that is not sequential sends its stages to the
/// runner's thread pool, whatever thread calls it. Sequential, the stages run
/// on the calling thread when it is a marked worker (see [`on_worker`]) and
/// otherwise on one thread created per batch, under both feature sets; the
/// tests on engines per thread, stale handles and the console rely on this.
fn run_demanding(registry: &Arc<FlmRegistry>, demands: &[&str], texts: &[&str]) -> Run {
    let mut content = DocContent::new();
    for text in texts {
        let source = Arc::new(FlmSource::new(*text).with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .expect("the tree parses");
        content.place(Arc::new(fragment));
    }
    let request = demands.iter().fold(PlanRequest::new(), |request, demand| request.demand(*demand));
    let plan = Arc::new(Plan::build(registry, &request).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::sequential()).expect("the run executes");
    run
}

/// The items of the extension's table `name` after `run`.
fn items(run: &Run, name: &str) -> Vec<Value> {
    run.table(TableHandle::<ExtTable>::new(name))
        .expect("the table is declared")
        .map(|table| table.items().to_vec())
        .unwrap_or_default()
}

/// The data of the first diagnostic of `run` whose identifier is `id`, as
/// JSON.
fn data_of(run: &Run, id: &str) -> Value {
    run.diagnostics()
        .iter()
        .find(|d| d.identifier() == id)
        .map(|d| diagnostic_value_to_json(&d.data().serializable_data()))
        .unwrap_or(Value::Null)
}

/// The `calls` of the counter probe's items after one run over one tree.
fn counter_run(registry: &Arc<FlmRegistry>) -> Vec<Value> {
    let run = run_demanding(registry, &["counter.calls"], &["One tree."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    items(&run, "counter.calls").into_iter().map(|item| item["calls"].clone()).collect()
}

/// `f` run on a thread marked as a worker of `flm-core`, on which a run
/// executes its stages itself.
fn on_worker<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    flm_core::lang::guard::run_with_worker_stack(f).expect("the worker thread starts")
}

/// A main module of the extension `wc` declaring the package `_` with one
/// feature `words` whose `register` body is `body` (with `b` the builder),
/// after `imports`.
fn wc_main(imports: &str, body: &str) -> String {
    format!(
        "{imports}\nexport default function (ext) {{ ext.package(\"_\", {{ features: {{ words: {{ register(b) {{\n{body}\n}} }} }} }}); }}\n"
    )
}

/// The imports of the stage modules.
const STAGE_IMPORTS: &str = r#"import { StageDecl, Needs, Effect, Scope } from "flm:core/stage";"#;

// --- load and describe ---------------------------------------------------------------------------

#[test]
fn load_reports_the_package_and_the_factory_names_it_its_feature_and_what_it_uses() {
    let main = r#"
        export default function (ext) {
            ext.package("_", {
                uses: ["stdcore"],
                features: { words: { title: "Words", docs: "The count.", version: "1", register(b) {} } },
            });
        }
        export function unrelated() { return 1; }
    "#;
    let factory = try_load(&host(JsSettings::default()), main, &[]).expect("the package loads");
    assert_eq!(factory.name().as_str(), "wc#_");
    assert_eq!(factory.features().iter().map(|f| f.name.as_str().to_string()).collect::<Vec<_>>(), ["words"]);
    assert!(factory.features().iter().all(|f| f.settings.is_none()));
    assert_eq!(factory.used().iter().map(|n| n.as_str().to_string()).collect::<Vec<_>>(), ["stdcore"]);
    // The package registers: its feature's `register` runs through JavaScript.
    registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
}

#[test]
fn a_main_module_that_does_not_parse_is_a_load_error() {
    let found = problems("export default function (ext) { const = 1; }");
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("the modules of the extension `wc` must be valid JavaScript: "), "{found:?}");
    assert!(found[0].ends_with("(at main.js:1:39)"), "{found:?}");
}

#[test]
fn a_main_module_whose_import_is_unknown_is_a_load_error() {
    assert_eq!(
        problems(r#"import "helpers"; export default function (ext) {}"#),
        [
            "the module `main.js` of the extension `wc` imports `helpers`, which is neither a module of the extension \
             (`./…`), nor of the extension API (`flm:…`), nor a module this program offers"
        ]
    );
}

#[test]
fn a_package_object_that_is_not_one_is_a_load_error() {
    assert_eq!(
        problems("export const x = 1;"),
        ["the main module `main.js` of the extension `wc` must have a function as its default export (`export default function (ext) { … }`)"]
    );
    let prefix = "the default export of the main module `main.js` of the extension `wc` must return without an error: ";
    let cases = [
        (r#"ext.package("", {})"#, "the extension `wc` calls `ext.package` with a package name that is not a non-empty text"),
        (r#"ext.package("_", 3)"#, "the package `_` of the extension `wc` must be an object"),
        (r#"ext.package("_", { uses: "stdcore" })"#, "`uses` of the package `_` of the extension `wc` must be a list of texts"),
        (r#"ext.package("_", { templates: [1] })"#, "`templates` of the package `_` of the extension `wc` must be a list of texts"),
        (r#"ext.package("_", { flm: [] })"#, "`flm` of the package `_` of the extension `wc` must be a map"),
        (
            r#"ext.package("_", { features: [] })"#,
            "`features` of the package `_` of the extension `wc` must be a map from a feature's name to its object",
        ),
        (
            r#"ext.package("_", { features: { a: { title: "A" } } })"#,
            "the feature `a` of the package `_` of the extension `wc` must be an object with a `register` function",
        ),
    ];
    for (call, expected) in cases {
        let main = format!("export default function (ext) {{ {call}; }}");
        assert_eq!(problems(&main), [format!("{prefix}{expected}")], "{call}");
    }
}

#[test]
fn a_feature_object_with_an_unknown_key_is_a_load_error() {
    let main = r#"export default function (ext) { ext.package("_", { features: { a: { setings: {}, register() {} } } }); }"#;
    assert_eq!(
        problems(main),
        [
            "the feature `a` of the package `_` of the extension `wc` cannot be read: unknown field `setings`, expected \
             one of `title`, `docs`, `version`, `settings`"
        ]
    );
    // A settings part of the wrong shape names the shape, not a type of Rust.
    let main = r#"export default function (ext) { ext.package("_", { features: { a: { settings: 3, register() {} } } }); }"#;
    assert_eq!(
        problems(main),
        [
            "`settings` of the feature `a` of the package `_` of the extension `wc` must be a map with `config`, \
             `options`, `merge_entries`, `markup` and `path`, each optional"
        ]
    );
    let main = r#"export default function (ext) { ext.package("_", { features: { a: { settings: { config: [] }, register() {} } } }); }"#;
    assert_eq!(
        problems(main),
        ["`settings.config` of the feature `a` of the package `_` of the extension `wc` must be a map with `schema` and, optionally, `docs`"]
    );
    let main = r#"export default function (ext) { ext.package("_", { features: { a: { settings: { options: { "wc.x": 3, "wc.y": { bind: 1, schema: {} } } }, register() {} } } }); }"#;
    assert_eq!(
        problems(main),
        [
            "in the extension `wc`, the option `wc.x` of the feature `a` of the package `_` must be a map with `bind`, \
             `schema` and, optionally, `docs` (`settings.options.wc.x`)",
            "in the extension `wc`, `bind` of the option `wc.y` of the feature `a` of the package `_` must be \
             `document`, `render` or `either`, and it is `1` (`settings.options.wc.y.bind`)",
        ]
    );
}

#[test]
fn every_part_of_a_feature_s_settings_of_the_wrong_shape_names_its_key_path() {
    let main = r#"export default function (ext) { ext.package("_", { features: { a: { settings: {
        options: [], merge_entries: 3, markup: "x", path: [1], config: null }, register() {} } } }); }"#;
    let owner = "of the feature `a` of the package `_` of the extension `wc`";
    assert_eq!(
        problems(main),
        [
            format!("`settings.options` {owner} must be a map from an option's name to its declaration"),
            format!("`settings.merge_entries` {owner} must be a map from a key to its default entries"),
            format!("`settings.markup` {owner} must be a list of texts"),
            format!("`settings.path` {owner} must be a list of texts"),
        ]
    );
}

#[test]
fn a_feature_object_s_key_that_json_cannot_hold_is_refused() {
    let prefix = "the default export of the main module `main.js` of the extension `wc` must return without an error: ";
    for (key, value) in [("run", "() => {}"), ("count", "1n"), ("tag", "Symbol(\"t\")")] {
        let main = format!(
            r#"export default function (ext) {{ ext.package("_", {{ features: {{ a: {{ {key}: {value}, register() {{}} }} }} }}); }}"#
        );
        assert_eq!(
            problems(&main),
            [format!(
                "{prefix}`{key}` of the feature `a` of the package `_` of the extension `wc` must be a text, a number, \
                 a boolean, a list or a map; the one function of a feature object is `register`"
            )],
            "{key}"
        );
    }
}

#[test]
fn a_value_json_cannot_hold_is_refused_at_any_depth_naming_its_key_path() {
    let prefix = "the default export of the main module `main.js` of the extension `wc` must return without an error: ";
    let owner = "of the feature `a` of the package `_` of the extension `wc`";
    let shapes = "must be a text, a number, a boolean, a list or a map; the one function of a feature object is `register`";
    for (object, refused) in [
        ("settings: { options: { x: () => {} } }", format!("`settings.options.x` {owner} {shapes}")),
        ("settings: { options: { x: { default: Symbol(\"t\") } } }", format!("`settings.options.x.default` {owner} {shapes}")),
        ("settings: { markup: [\"a\", 1n] }", format!("`settings.markup[1]` {owner} {shapes}")),
        // A list with a hole: the walk reads the hole as undefined.
        ("settings: { options: [1, , () => {}] }", format!("`settings.options[2]` {owner} {shapes}")),
        ("settings: { [Symbol(\"t\")]: 1 }", format!("the key `Symbol(t)` of `settings` {owner} must be a text")),
        ("[Symbol(\"t\")]: 1", format!("the key `Symbol(t)` {owner} must be a text")),
    ] {
        let main = format!(
            r#"export default function (ext) {{ ext.package("_", {{ features: {{ a: {{ {object}, register() {{}} }} }} }}); }}"#
        );
        assert_eq!(problems(&main), [format!("{prefix}{refused}")], "{object}");
    }
    // Plan A, A56: a feature object that contains itself, refused at the key
    // path where the cycle closes; a value met twice without a cycle passes.
    for (object, refused) in [
        ("settings: s", "`settings.self`"),
        ("settings: { options: [s] }", "`settings.options[0].self`"),
    ] {
        let main = format!(
            r#"export default function (ext) {{ const s = {{}}; s.self = s; ext.package("_", {{ features: {{ a: {{ {object}, register() {{}} }} }} }}); }}"#
        );
        assert_eq!(problems(&main), [format!("{prefix}{refused} {owner} must not contain itself")], "{object}");
    }
    // Plan A, A62: the feature object itself is among the objects walked, so a
    // feature object that contains itself is refused as a cycle, not at its
    // `register` function.
    for (statement, refused) in [("f.self = f;", "`self`"), ("f.settings = { back: f };", "`settings.back`")] {
        let main = format!(
            r#"export default function (ext) {{ const f = {{ register() {{}} }}; {statement} ext.package("_", {{ features: {{ a: f }} }}); }}"#
        );
        assert_eq!(problems(&main), [format!("{prefix}{refused} {owner} must not contain itself")], "{statement}");
    }
    let main = r#"export default function (ext) { const d = { type: "string" }; ext.package("_", { features: { a: { docs: "A", settings: { config: { schema: { type: "object", properties: { x: d, y: d } } } }, register() {} } } }); }"#;
    assert!(try_load(&host(JsSettings::default()), main, &[]).is_ok());
}

#[test]
fn a_feature_object_s_own_name_key_is_refused_as_unknown() {
    let main = r#"export default function (ext) { ext.package("_", { features: { a: { name: "b", register() {} } } }); }"#;
    assert_eq!(
        problems(main),
        [
            "the feature `a` of the package `_` of the extension `wc` cannot be read: unknown field `name`, expected \
             one of `title`, `docs`, `version`, `settings`"
        ]
    );
}

#[test]
fn a_package_name_a_use_entry_cannot_write_is_refused() {
    for name in ["a#b", "x y", "a/b", "."] {
        let main = format!(r#"export default function (ext) {{ ext.package("{name}", {{}}); }}"#);
        assert_eq!(
            problems(&main),
            [format!("the package name `{name}` of the extension `wc` must be letters, digits, `-` and `_`")],
            "{name}"
        );
    }
}

// --- register and stages -------------------------------------------------------------------------

#[test]
fn register_declares_a_table_and_a_stage_through_javascript_and_the_registry_builds() {
    let main = wc_main(
        STAGE_IMPORTS,
        r#"
        if (b.feature() !== "words") throw new Error(`the feature is ${b.feature()}`);
        b.table("wc.seen", "What the stage saw.");
        b.stage({
            decl() { return new StageDecl("wc.see", Effect.Compute, Scope.Document).writes("wc.seen").docs("Sees."); },
            needs() { return new Needs(); },
            run(cx, out) { out.table("wc.seen").push({ scope: cx.scope(), tree: cx.tree_id() }); },
        });
        "#,
    );
    let registry = registry_with([package(&main)]);
    let run = run_demanding(&registry, &["wc.seen"], &["Hello."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(items(&run, "wc.seen"), [json!({ "scope": "document", "tree": null })]);
}

#[test]
fn a_per_tree_stage_writes_its_table_from_javascript() {
    let main = wc_main(
        STAGE_IMPORTS,
        r#"
        b.table("wc.chars", "The characters of each tree.");
        b.stage({
            decl() { return new StageDecl("wc.count", Effect.Compute, Scope.PerTree).writes("wc.chars"); },
            run(cx, out) {
                let count = 0;
                for (const node of cx.my_tree().descendants()) {
                    const chars = node.chars();
                    if (chars !== null) count += chars.length;
                }
                out.table("wc.chars").push({ tree: cx.tree_id(), chars: count });
            },
        });
        "#,
    );
    let registry = registry_with([package(&main)]);
    let run = run_demanding(&registry, &["wc.chars"], &["One \\emph{two}", "Three."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let mut found = items(&run, "wc.chars");
    found.sort_by_key(|item| item["tree"].as_str().map(str::to_string));
    assert_eq!(found, [json!({ "tree": "tree#0", "chars": 7 }), json!({ "tree": "tree#1", "chars": 6 })]);
}

#[test]
fn a_document_stage_reads_the_structured_trees_table() {
    let main = wc_main(
        STAGE_IMPORTS,
        r#"
        b.table("wc.trees", "The structured trees.");
        b.stage({
            decl() { return new StageDecl("wc.trees", Effect.Compute, Scope.Document).writes("wc.trees"); },
            needs() { return new Needs().reads("flm.blocks.structured"); },
            run(cx, out) {
                const table = cx.get("flm.blocks.structured");
                for (const [id, root] of table.trees.trees()) {
                    out.table("wc.trees").push({ id, children: root.children().length, count: root.child_count() });
                }
            },
        });
        "#,
    );
    let registry = registry_with([package(&main)]);
    let run = run_demanding(&registry, &["wc.trees"], &["One.\n\nTwo.", "Three."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let found = items(&run, "wc.trees");
    assert_eq!(found.iter().map(|item| item["id"].clone()).collect::<Vec<_>>(), [json!("tree#0"), json!("tree#1")]);
    for item in &found {
        assert_eq!(item["children"], item["count"], "{item}");
    }
    assert!(found[0]["count"].as_u64() > found[1]["count"].as_u64(), "{found:?}");
}

#[test]
fn a_stage_runs_on_a_worker_thread_with_that_thread_s_engine() {
    // A run started from a thread not marked as a worker runs its stages on a
    // worker thread of its own (`flm_core::lang::guard`); the scenario runs on
    // marked worker threads so that it chooses the threads.
    let registry = on_worker(|| {
        let registry = registry_with([counter_package(&host(JsSettings::default()))]);
        // The building thread's engine runs the stage: the module's count grows.
        assert_eq!(counter_run(&registry), [json!(1)]);
        assert_eq!(counter_run(&registry), [json!(2)]);
        registry
    });
    // Another thread has an engine of its own: the feature is rehearsed there,
    // and the module's count starts again.
    let shared = Arc::clone(&registry);
    let other = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || on_worker(|| (counter_run(&shared), counter_run(&shared))))
        .expect("the thread starts");
    assert_eq!(other.join().expect("the thread runs"), (vec![json!(1)], vec![json!(2)]));
    // From an unmarked thread, every run has a worker thread of its own.
    assert_eq!(counter_run(&registry), [json!(1)]);
    assert_eq!(counter_run(&registry), [json!(1)]);
}

#[test]
fn strict_mode_gives_every_handler_call_a_fresh_engine() {
    let host = host(JsSettings { strict: true, ..JsSettings::default() });
    // On one thread (see the test above), the engine would be kept.
    on_worker(|| {
        let registry = registry_with([counter_package(&host)]);
        assert_eq!(counter_run(&registry), [json!(1)]);
        assert_eq!(counter_run(&registry), [json!(1)]);
    });
}

// --- failures ------------------------------------------------------------------------------------

/// Plan A, A55: the refusal crosses the binding as a thrown `FlmError` and comes
/// back with its kind, so the build error carries the name rule's identifier.
#[test]
fn a_name_outside_the_extension_s_stem_fails_the_build_from_javascript() {
    let main = wc_main("", r#"b.table("words", "Outside the prefix.");"#);
    let error = try_registry_with([package(&main)]).err().map(|error| error.to_string()).unwrap_or_default();
    assert!(
        error.contains(
            "flm.ext.name-outside-extension: the table `words` declared by the package `_` of the extension `wc` must begin with `wc.`"
        ),
        "{error}"
    );
    assert!(!error.contains("flm.ext.handler-failed"), "{error}");
}

/// Plan A, A55: a script that catches the refusal and throws it again still
/// fails with the name rule's identifier; any other `FlmError` stays a
/// handler's failure.
#[test]
fn a_caught_and_rethrown_refusal_of_the_name_rule_keeps_its_kind() {
    let main = wc_main("", r#"try { b.table("words", "Outside the prefix."); } catch (e) { if (e.kind !== "name_outside_extension") throw new Error(`kind ${e.kind}`); throw e; }"#);
    let error = try_registry_with([package(&main)]).err().map(|error| error.to_string()).unwrap_or_default();
    assert!(error.contains("flm.ext.name-outside-extension: the table `words`"), "{error}");
    let main = wc_main("", r#"b.table({}, "Not a name.");"#);
    let error = try_registry_with([package(&main)]).err().map(|error| error.to_string()).unwrap_or_default();
    assert!(error.contains("flm.ext.handler-failed"), "{error}");
}

#[test]
fn a_throwing_register_fails_the_build_with_handler_failed() {
    let main = wc_main("", r#"throw new TypeError("the register is not ready");"#);
    let error = try_registry_with([package(&main)]).err().map(|error| error.to_string()).unwrap_or_default();
    assert!(error.contains("flm.ext.handler-failed"), "{error}");
    assert!(error.contains("the register is not ready"), "{error}");
}

#[test]
fn a_throwing_run_is_an_error_at_the_stage_and_the_run_completes() {
    let main = wc_main(
        STAGE_IMPORTS,
        r#"
        b.table("wc.seen", "What the stage saw.");
        b.stage({
            decl() { return new StageDecl("wc.see", Effect.Compute, Scope.Document).writes("wc.seen"); },
            run(cx, out) {
                out.table("wc.seen").push("before");
                throw new RangeError("the stage gave up");
            },
        });
        "#,
    );
    let registry = registry_with([package(&main)]);
    let run = run_demanding(&registry, &["wc.seen"], &["Hello."]);
    assert_eq!(common::diagnostics(&run), ["flm.ext.handler-failed: the stage `wc.see` failed: the stage gave up"]);
    assert_eq!(items(&run, "wc.seen"), [json!("before")]);
    let data = data_of(&run, "flm.ext.handler-failed");
    assert_eq!(data["kind"], "handler");
    assert_eq!(data["data"]["name"], "RangeError");
    assert!(data["data"]["stack"].as_str().is_some_and(|stack| stack.contains("run")), "{data}");
    assert_eq!(data["data"]["flm_error"], Value::Null);
}

#[test]
fn a_stale_handle_is_a_handler_failure_with_the_stale_handle_data() {
    let main = wc_main(
        STAGE_IMPORTS,
        r#"
        let kept = null;
        b.table("wc.seen", "What the stage saw.");
        b.stage({
            decl() { return new StageDecl("wc.see", Effect.Compute, Scope.PerTree).writes("wc.seen"); },
            run(cx, out) {
                if (kept === null) { kept = cx; out.table("wc.seen").push(cx.scope()); return; }
                kept.scope();
            },
        });
        "#,
    );
    let registry = registry_with([package(&main)]);
    let run = run_demanding(&registry, &["wc.seen"], &["One.", "Two."]);
    assert_eq!(common::identifiers(&run), ["flm.ext.handler-failed"]);
    let data = data_of(&run, "flm.ext.handler-failed");
    assert_eq!(data["data"]["name"], "FlmError");
    assert_eq!(data["data"]["flm_error"]["kind"], "stale_handle");
    assert_eq!(data["data"]["flm_error"]["handle"]["kind"], "stage_cx");
    assert!(data["detail"].as_str().is_some_and(|detail| detail.contains("belongs to a call that is not the one running")), "{data}");
    assert_eq!(items(&run, "wc.seen"), [json!("per-tree")]);
}

#[test]
fn a_float_pushed_from_javascript_is_refused() {
    let main = wc_main(
        STAGE_IMPORTS,
        r#"
        b.table("wc.seen", "What the stage saw.");
        b.stage({
            decl() { return new StageDecl("wc.see", Effect.Compute, Scope.Document).writes("wc.seen"); },
            run(cx, out) {
                try {
                    out.table("wc.seen").push({ ratio: 1.5 });
                } catch (error) {
                    out.diags({ id: "wc.refused", severity: "note", message: `${error.name} ${error.kind}`, data: { message: error.message } });
                }
            },
        });
        "#,
    );
    let registry = registry_with([package(&main)]);
    let run = run_demanding(&registry, &["wc.seen"], &["Hello."]);
    assert_eq!(common::diagnostics(&run), ["wc.refused: FlmError value"]);
    assert!(data_of(&run, "wc.refused")["message"].as_str().is_some_and(|m| !m.is_empty()));
    assert!(items(&run, "wc.seen").is_empty());
}

#[test]
fn console_log_inside_a_handler_reaches_the_sink() {
    let lines: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let sink_lines = Arc::clone(&lines);
    let host = host(JsSettings::default()).with_console(Arc::new(move |level: ConsoleLevel, text: &str| {
        if let Ok(mut lines) = sink_lines.lock() {
            lines.push(format!("{level} {text}"));
        }
    }));
    let main = wc_main(
        STAGE_IMPORTS,
        r#"
        console.info("registering", b.feature());
        b.table("wc.seen", "What the stage saw.");
        b.stage({
            decl() { return new StageDecl("wc.see", Effect.Compute, Scope.PerTree).writes("wc.seen"); },
            run(cx, out) { console.log("tree", cx.tree_id(), 2); },
        });
        "#,
    );
    on_worker(|| {
        let registry = registry_with([package_with(&host, &main)]);
        run_demanding(&registry, &["wc.seen"], &["Hello."]);
    });
    let lines = lines.lock().map(|lines| lines.clone()).unwrap_or_default();
    assert_eq!(lines, ["info registering words", "log tree tree#0 2"]);
}

#[test]
fn the_digest_changes_with_a_script() {
    let main = wc_main(r#"import { WORD } from "./helpers.js";"#, "b.digest_input(WORD);");
    let host = host(JsSettings::default());
    let digest = |helpers: &str| {
        let factory = try_load(&host, &main, &[("helpers.js", helpers)]).expect("the package loads");
        registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]).digest()
    };
    let first = digest("export const WORD = \"one\";");
    assert_eq!(first, digest("export const WORD = \"one\";"));
    // A change in a module the script imports, even one `register` does not
    // feed into the digest by itself.
    assert_ne!(first, digest("export const WORD = \"one\"; // changed"));
}

// --- an expansion hook, through the prelude's helpers ----------------------------------------------

/// A data-form element `\badge` whose hook, lent from JavaScript, stages the
/// text `text` (the expansion modules come later; the hook reaches the
/// context through the prelude's `call`).
fn badge_main(stage_text: &str) -> String {
    wc_main(
        r#"import { call, lend_expand, module } from "flm:_runtime";"#,
        &format!(
            r#"
        const hook = {{
            policy() {{ return "always"; }},
            expand(node, cx, xp) {{
                if (node.element_name() !== "wc.badge") throw new Error("not the badge");
                return {{ emit: [call(cx, "stage_chars", [{stage_text}])] }};
            }},
        }};
        const element = {{ name: "wc.badge", docs: "", aspects: [{{ Expand: lend_expand("wc.badge", hook) }}], value: null }};
        const definition = {{ kind: "macro", name: "badge", expand: "always" }};
        b.lowering_rule(call(module("stdcore/features/userdefs"), "expand_in_tree_rule_for", ["wc.badge", definition]));
        b.element({{ owner: b.feature(), names: ["badge"], syntax: {{ macro: {{ args: [] }} }}, element, docs: "", modes: null }});
        "#
        ),
    )
}

#[test]
fn a_hook_lent_from_javascript_expands_its_element_on_any_thread() {
    let registry = registry_with([package(&badge_main(r#""HI""#))]);
    // On a run's own worker thread: the feature is rehearsed there first.
    let (text, diagnostics) = common::text_of(&registry, "A \\badge{} here.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A HI here.\n");
}

#[test]
fn a_hook_that_fails_recovers_the_use_site_with_handler_failed() {
    // A float as an argument is refused: the `FlmError` escapes the hook.
    let registry = registry_with([package(&badge_main(r#"1.5"#))]);
    let (text, diagnostics) = common::text_of(&registry, "A \\badge{} here.");
    assert_eq!(text, "A \\badge here.\n");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].starts_with("flm.ext.handler-failed: the expansion hook of `wc.badge` failed: "), "{diagnostics:?}");
}

// --- WP-4a: handler aspects, late-init hooks and finalizers from JavaScript ---------------------

/// The imports of the box element's script.
const BOX_IMPORTS: &str = r#"
import { ArgDef, ElementDef, SyntaxDef } from "flm:lang/mint";
import { ElementSpec, Aspect } from "flm:model/element";
import { RefPoint, AnchorPolicy, ContentRef, AcceptedRefTypes } from "flm:model/aspects";
import * as refs from "flm:stdcore/features/refs";
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
"#;

/// A main module declaring `\wcbox{content}` with its `\label` fields, its
/// element `wc.box` carrying the aspects `aspects` (a JavaScript expression
/// of a list of aspects, `points` a function in scope), lowered to its
/// content.
fn box_main(aspects: &str) -> String {
    wc_main(
        BOX_IMPORTS,
        &format!(
            r#"
        function points(node) {{
            const [labels, malformed] = refs.point_labels(refs.label_uses(node));
            return [new RefPoint({{ kind: "wcbox", labels, malformed, anchor: AnchorPolicy.FromFirstLabel,
                content: ContentRef.Argument(0), accepts: AcceptedRefTypes.Any }})];
        }}
        let spec = new ElementSpec("wc.box");
        for (const aspect of {aspects}) spec = spec.with(aspect);
        b.element(new ElementDef(b.feature(), ["wcbox"], SyntaxDef.Macro([ArgDef.mandatory("content"), refs.label_fields_arg()]), spec));
        b.lowering_rule(new LoweringRule("wc.box.lower.default", "wc.box", new LoweringDecl(),
            {{ lower(node, cx, lw) {{ return Lowering.Emit(lw.lower_arg_named(cx, node, "content")); }} }}, DefaultMark.Everywhere));
        "#
        ),
    )
}

/// The document of the reference tests.
const REFERENCED: &str = "\\wcbox{Alpha}\\label{ex:one} See \\ref{ex:one}.";

/// The table of contents' entries of `text`, as `(level, whether the title
/// is the first argument)`.
fn toc_entries(registry: &Arc<FlmRegistry>, text: &str) -> Vec<(u32, bool)> {
    use flm_core::model::aspects::ContentRef as CoreContent;
    use flm_core::stdcore::features::toc;
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Plan::build(registry, &PlanRequest::new().demand(toc::ENTRIES.name())).expect("a plan");
    let mut run = Run::new(Arc::new(Doc::new(content)), Arc::new(plan), Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run.table(toc::ENTRIES)
        .expect("declared")
        .map(|table| {
            table.entries.iter().map(|entry| (entry.level, matches!(entry.title, Some(CoreContent::Argument(0))))).collect()
        })
        .unwrap_or_default()
}

#[test]
fn a_scripted_referenceable_and_sectioning_answer_on_a_worker_thread_after_the_rehearsal() {
    // The registry is built on the test thread; the runs execute their stages
    // on a worker thread of their own, whose engine instance rehearses the
    // feature and finds the aspects by the element's name.
    let main = box_main(
        "[Aspect.Referenceable({ points }), Aspect.Sectioning({ level(node) { return 2; }, toc_title(node) { return ContentRef.Argument(0); } })]",
    );
    let registry = registry_with([package(&main)]);
    let (text, diagnostics) = common::text_of(&registry, REFERENCED);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Alpha See Alpha.\n");
    // `in_toc` is left out: the section is listed, as the trait's default says.
    assert_eq!(toc_entries(&registry, "\\wcbox{Intro} text \\wcbox{More}"), [(2, true), (2, true)]);
}

#[test]
fn an_in_toc_and_a_toc_title_left_out_answer_the_defaults_and_in_toc_false_hides() {
    let bare = registry_with([package(&box_main("[Aspect.Sectioning({ level(node) { return 1; } })]"))]);
    assert_eq!(toc_entries(&bare, "\\wcbox{Intro}"), [(1, false)]);
    let hidden = registry_with([package(&box_main("[Aspect.Sectioning({ level(node) { return 1; }, in_toc(node) { return false; } })]"))]);
    assert!(toc_entries(&hidden, "\\wcbox{Intro}").is_empty());
}

#[test]
fn a_points_handler_that_throws_leaves_the_element_without_a_point() {
    let main = box_main(r#"[Aspect.Referenceable({ points(node) { throw new Error("no points today"); } })]"#);
    let registry = registry_with([package(&main)]);
    let (_, diagnostics) = common::text_of(&registry, REFERENCED);
    // The aspect answers no point: the `\ref` is unresolved, reported by the
    // references feature at the `\ref`; the handler's failure is reported
    // nowhere.
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].contains("ex:one"), "{diagnostics:?}");
    assert!(!diagnostics[0].contains("no points today"), "{diagnostics:?}");
}

#[test]
fn attach_lends_a_scripted_sectioning_to_an_element_by_name() {
    // The element carries no aspect of its own; `attach` adds one, recorded
    // under the element's name.
    let main = box_main("[]").replace(
        "b.lowering_rule(",
        "b.attach(\"wc.box\", Aspect.Sectioning({ level(node) { return 3; } }));\n        b.lowering_rule(",
    );
    let registry = registry_with([package(&main)]);
    assert_eq!(toc_entries(&registry, "\\wcbox{Intro}"), [(3, false)]);
}

#[test]
fn an_aspect_of_an_unknown_kind_is_refused_by_the_native() {
    let main = wc_main(
        r#"import { lend_aspect } from "flm:_runtime";"#,
        r#"lend_aspect("Bogus", "wc.box", { points() { return []; } });"#,
    );
    let factory = try_load(&host(JsSettings::default()), &main, &[]).expect("the extension loads");
    let error = match factory.build(&PackageSettings::new()) {
        Ok(entry) => common::try_registry_with([entry]).expect_err("the build fails").to_string(),
        Err(error) => error.to_string(),
    };
    assert!(error.contains("the kind of a handler aspect must be `Referenceable` or `Sectioning`, not `Bogus`"), "{error}");
}

/// A main module whose `register` registers a late-initialization hook
/// pushing a warning, and a finalizer declaring the table `wc.late` and a
/// stage filling it; with `only_when_dispatching`, both are registered only
/// when the builder dispatches (not in a rehearsal), so that an instance
/// that rehearses does not find them.
fn hooks_main(only_when_dispatching: bool) -> String {
    let guard = if only_when_dispatching { "if (b._token !== null)" } else { "" };
    wc_main(
        STAGE_IMPORTS,
        &format!(
            r#"
        {guard} b.late_init({{ late_init(lang, diags) {{
            diags.push({{ id: "wc.late", severity: "warning", message: "the language is built" }});
        }} }});
        {guard} b.finalizer({{ finalize(b) {{
            b.table("wc.late", "A table of the second round.");
            b.stage({{
                decl() {{ return new StageDecl("wc.fill", Effect.Compute, Scope.Document).writes("wc.late"); }},
                run(cx, out) {{ out.table("wc.late").push({{ filled: true }}); }},
            }});
        }} }});
        "#
        ),
    )
}

#[test]
fn late_init_and_finalizer_objects_are_found_by_position() {
    let registry = registry_with([package(&hooks_main(false))]);
    let found: Vec<String> =
        registry.build_diagnostics().iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect();
    assert_eq!(found, ["wc.late: the language is built"]);
    // The stage the finalizer declared runs on the run's own worker thread,
    // whose instance replays the finalizer in its rehearsal.
    let run = run_demanding(&registry, &["wc.late"], &["One tree."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(items(&run, "wc.late"), [json!({ "filled": true })]);
}

#[test]
fn a_late_init_hook_or_a_finalizer_not_found_after_the_rehearsal_fails_the_build() {
    // Under strict mode every handler call has a fresh instance, which
    // rehearses the feature; this one registers its hooks only when the
    // builder dispatches, so the rehearsal does not find them.
    let host = host(JsSettings { strict: true, ..JsSettings::default() });
    let factory = try_load(&host, &hooks_main(true), &[]).expect("the extension loads");
    let entry = factory.build(&PackageSettings::new()).expect("the package builds");
    let error = common::try_registry_with([entry]).expect_err("the build fails").to_string();
    let late = "the late-initialization hook 0 must be registered by `register` of the feature `words` of the package `_` of the extension `wc`";
    let finalizer = "the finalizer 0 must be registered by `register` of the feature `words` of the package `_` of the extension `wc`";
    assert!(error.contains("flm.ext.handler-failed: ") && (error.contains(late) || error.contains(finalizer)), "{error}");
    // Without the late-initialization hook, the finalizer is the one met.
    let only_finalizer = hooks_main(true).replacen("if (b._token !== null) b.late_init", "if (false) b.late_init", 1);
    let factory = try_load(&host, &only_finalizer, &[]).expect("the extension loads");
    let entry = factory.build(&PackageSettings::new()).expect("the package builds");
    let error = common::try_registry_with([entry]).expect_err("the build fails").to_string();
    assert!(error.contains(finalizer), "{error}");
}

#[test]
fn the_aspect_entries_name_what_is_not_found_after_the_rehearsal() {
    // The entries of `flm:_entry`, called for an element of a feature whose
    // `register` attached nothing on this instance.
    let main = r#"
import { points, level } from "flm:_entry";
export function probe() {
  const texts = [];
  for (const entry of [points, level]) {
    try { entry("_", "words", "wc.box", "null"); } catch (error) { texts.push(error.message); }
  }
  return JSON.stringify(texts);
}
"#;
    let host = host(JsSettings::default());
    let package = flm_ext::quickjs::TestPackage::new(&host, &[("main.js", main)]).expect("the test package");
    let answer = flm_ext::quickjs::eval_for_test(&host, &package, "return main.probe();").expect("the script runs");
    let texts: Vec<String> = serde_json::from_str(&answer).expect("a list of texts");
    assert_eq!(texts.len(), 2, "{texts:?}");
    assert!(
        texts[0].starts_with("the reference points of the element `wc.box` must be answered by an aspect attached by `register` of the feature `words` of the package `_` of the extension `"),
        "{texts:?}"
    );
    assert!(
        texts[1].starts_with("the sectioning of the element `wc.box` must be answered by an aspect attached by `register` of the feature `words` of the package `_` of the extension `"),
        "{texts:?}"
    );
}

// --- WP-4b: the durable token's class ------------------------------------------------------------

#[test]
fn a_durable_token_revives_as_its_registered_class_and_writes_itself_back() {
    // `revive` makes an instance of the class registered for the token's
    // kind: `TemplateLanguage` extends `Durable`; `CompiledTemplate` serves
    // both forms (a variant's template is a per-call handle, a compiled
    // template a durable token), each instance writing back the form it came
    // in. A kind no module registered revives as a `Durable`.
    let main = r#"
import { revive, Durable, Handle } from "flm:_runtime";
import { TemplateLanguage, CompiledTemplate } from "flm:lang/rewrites/userdefs";
export function probe() {
  const language = revive({ "@durable": { kind: "template_language", table: 7, index: 0 } });
  const template = revive({ "@durable": { kind: "compiled_template", table: 7, index: 1 } });
  const per_call = revive({ "@handle": { kind: "compiled_template", call: 3, index: 2 } });
  const unknown = revive({ "@durable": { kind: "bogus", table: 7, index: 4 } });
  return JSON.stringify({
    language: [language instanceof TemplateLanguage, language instanceof Durable, JSON.parse(JSON.stringify(language))],
    template: [template instanceof CompiledTemplate, JSON.parse(JSON.stringify(template))],
    per_call: [per_call instanceof CompiledTemplate, per_call instanceof Handle, JSON.parse(JSON.stringify(per_call))],
    unknown: [unknown instanceof Durable, JSON.parse(JSON.stringify(unknown))],
    nested: JSON.parse(JSON.stringify(revive([{ at: { "@durable": { kind: "template_language", table: 7, index: 0 } } }]))),
  });
}
"#;
    let host = host(JsSettings::default());
    let package = flm_ext::quickjs::TestPackage::new(&host, &[("main.js", main)]).expect("the test package");
    let answer = flm_ext::quickjs::eval_for_test(&host, &package, "return main.probe();").expect("the script runs");
    let answer: Value = serde_json::from_str(&answer).expect("JSON");
    assert_eq!(
        answer,
        json!({
            "language": [true, true, { "@durable": { "kind": "template_language", "table": 7, "index": 0 } }],
            "template": [true, { "@durable": { "kind": "compiled_template", "table": 7, "index": 1 } }],
            "per_call": [true, true, { "@handle": { "kind": "compiled_template", "call": 3, "index": 2 } }],
            "unknown": [true, { "@durable": { "kind": "bogus", "table": 7, "index": 4 } }],
            "nested": [{ "at": { "@durable": { "kind": "template_language", "table": 7, "index": 0 } } }],
        })
    );
}

#[test]
fn a_stale_durable_token_from_a_script_is_a_handler_failure_with_the_stale_durable_data() {
    // A rule passes a template token of a table that does not exist: the
    // dispatcher refuses it as stale, and the script sees an `FlmError` of
    // kind `stale_durable` whose data names the token.
    let main = wc_main(
        r#"
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import { ArgDef, ElementDef, SyntaxDef } from "flm:lang/mint";
import { ElementSpec } from "flm:model/element";
import { ValueBindings, instantiate } from "flm:lang/rewrites/userdefs";
import { revive } from "flm:_runtime";
import { SPAN } from "flm:stdcore/backends/pres";
"#,
        r#"
        b.element(new ElementDef(b.feature(), ["wcstale"], SyntaxDef.Macro([ArgDef.mandatory("content")]), new ElementSpec("wc.stale")));
        b.lowering_rule(new LoweringRule("wc.stale.lower.default", "wc.stale", new LoweringDecl().produces(SPAN), {
          lower(node, cx, lw) {
            const stale = revive({ "@durable": { kind: "compiled_template", table: 999999, index: 1 } });
            try { instantiate(node, stale, new ValueBindings()); }
            catch (error) {
              lw.diagnose({ id: "wc.caught", severity: "warning", message: `${error.kind}|${JSON.stringify(error.data.durable)}|${error.message}` });
            }
            return Lowering.Emit([]);
          },
        }, DefaultMark.Everywhere));
        "#,
    );
    let registry = registry_with([package(&main)]);
    let (_, diagnostics) = common::text_of(&registry, "A \\wcstale{}.");
    assert_eq!(
        diagnostics,
        [
            "wc.caught: stale_durable|{\"kind\":\"compiled_template\",\"table\":999999,\"index\":1}|the `compiled_template` object 1 must be one the feature `words` of the package `_` of the extension `wc` made under the registry in use"
        ]
    );
}
