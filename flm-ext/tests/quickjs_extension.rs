//! The engine `js` loading extensions through a loader (plan B, section 2):
//! the default export and its package objects, the main module's path, the
//! module rules at load and on other threads, the files read at load only,
//! the refusal of every dynamic `import()`, the queued jobs, the rehearsal
//! with each configuration, the digest, one load's engine instances, and
//! case 3 (`tests/js/twopack/`), an extension of two packages.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageEntry, PackageFactory, PackageResolver};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::features::compose::{ComposeFeature, Template, TEMPLATES_SLOT};
use flm_ext::adapter::table::ExtTable;
use flm_ext::block::FeatureBlock;
use flm_ext::quickjs::{JsEngine, JsHost, JsSettings, ENGINE_VERSION};
use flm_ext::source::{FilesError, SourceError};
use flm_ext::{
    Engine, ExtensionFiles, ExtensionIdentity, ExtensionLoader, ExtensionSource, FoundExtension, Locator, Manifest,
    MemoryFiles, MemorySource,
};
use serde_json::{json, Value};

use common::{factory_of, js_extension, js_loader, js_manifest, load_problems, registry_with, try_registry_with};

// --- helpers -----------------------------------------------------------------------------------

/// A default host.
fn host() -> JsHost {
    JsHost::new(JsSettings::default())
}

/// The loader of the extension `wc` of the files `files`, the main module
/// `main.js`.
fn loader(files: &[(&str, &str)]) -> Arc<ExtensionLoader> {
    js_loader(&host(), js_extension("wc", files, js_manifest("main.js")))
}

/// The problems of loading the extension `wc` of the one module `main.js`.
fn problems(main: &str) -> Vec<String> {
    load_problems(&loader(&[("main.js", main)]), "wc")
}

/// `factory` built from empty settings.
fn built(factory: &PackageFactory) -> PackageEntry {
    factory.build(&PackageSettings::new()).expect("the package builds")
}

/// A main module of the extension `wc` declaring the package `_` with one
/// feature `see`, whose `register(b, settings)` declares the table `wc.seen`
/// and a document stage `wc.see` running `run` (with `cx`, `out` and
/// `settings` in scope), after `prelude` (imports and module code).
fn seeing(prelude: &str, run: &str) -> String {
    format!(
        r#"import {{ StageDecl, Effect, Scope }} from "flm:core/stage";
{prelude}
export default function (ext) {{
  ext.package("_", {{ features: {{ see: {{ register(b, settings) {{
    b.table("wc.seen", "what the stage saw");
    b.stage({{
      decl: () => new StageDecl("wc.see", Effect.Compute, Scope.Document).writes("wc.seen"),
      run(cx, out) {{ {run} }},
    }});
  }} }} }} }});
}}
"#
    )
}

/// The run of a plan demanding `demand` over a one-tree document, sequential
/// (the stages run on the calling thread when it is a marked worker).
fn run_demanding(registry: &Arc<FlmRegistry>, demand: &str) -> Run {
    let source = Arc::new(FlmSource::new("Some text.").with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the tree parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Arc::new(Plan::build(registry, &PlanRequest::new().demand(demand)).expect("the plan builds"));
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

/// The items of `wc.seen` after one run of `registry`.
fn seen(registry: &Arc<FlmRegistry>) -> Vec<Value> {
    let run = run_demanding(registry, "wc.seen");
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    items(&run, "wc.seen")
}

/// `f` run on a thread marked as a worker of `flm-core`, on which a run
/// executes its stages itself.
fn on_worker<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    flm_core::lang::guard::run_with_worker_stack(f).expect("the worker thread starts")
}

/// `f` run on a marked worker thread of a new thread, which has no engine
/// instance yet.
fn on_other_thread<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn_scoped(scope, || on_worker(f))
            .expect("the thread starts")
            .join()
            .expect("the thread runs")
    })
}

/// The loader of case 3 (`tests/js/twopack/`), its main module's text
/// changed by `edit`.
fn twopack(edit: impl Fn(&str) -> String) -> Arc<ExtensionLoader> {
    let main = edit(common::TWOPACK_FILES[0].1);
    let files = [("main.js", main.as_str()), common::TWOPACK_FILES[1]];
    js_loader(&host(), js_extension("twopack", &files, js_manifest("main.js")))
}

/// The registry of the standard package and every package of `found` but
/// `stdcore`.
fn registry_of(found: &[PackageFactory]) -> Arc<FlmRegistry> {
    registry_with(found.iter().filter(|factory| factory.name().as_str() != "stdcore").map(built))
}

// --- the default export and the package objects ------------------------------------------------

#[test]
fn the_default_export_declares_the_packages_and_the_loader_answers_their_factories() {
    let loader = twopack(str::to_string);
    assert_eq!(factory_of(&loader, "twopack").name().as_str(), "twopack#_");
    let proofs = factory_of(&loader, "twopack#proofs");
    assert_eq!(proofs.name().as_str(), "twopack#proofs");
    assert_eq!(proofs.used().iter().map(|name| name.as_str()).collect::<Vec<_>>(), ["twopack#_"]);
    assert_eq!(proofs.features().iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["proofs"]);
    let mut listed: Vec<String> = loader.list().unwrap_or_default().iter().map(|name| name.as_str().to_string()).collect();
    listed.sort();
    assert_eq!(listed, ["twopack#_", "twopack#proofs"]);
}

#[test]
fn a_package_object_s_uses_flm_templates_and_features_reach_its_report() {
    let main = r#"
        export default function (ext) {
          ext.package("_", {
            uses: ["stdcore"],
            flm: { features: { text: { width: 3 } } },
            templates: ["./templates/html/fancy/flmtemplate.yaml", "templates/html/bare.yaml"],
            features: { words: { title: "Words", register(b) {} } },
          });
        }
    "#;
    let files = MemoryFiles::new()
        .with_text("main.js", main)
        .with_value(
            "templates/html/fancy/flmtemplate.yaml",
            json!({ "output": "html", "engine": "content-only", "main": "fancy.html", "accepts": "all" }),
        )
        .with_text("templates/html/fancy/fancy.html", "<main>{{ content }}</main>")
        .with_value("templates/html/bare.yaml", json!({ "output": "html", "engine": "content-only", "accepts": "all" }));
    let source = MemorySource::new().with("wc", "wc", js_manifest("main.js"), files);
    let loader = js_loader(&host(), source);
    let factory = factory_of(&loader, "wc");
    assert_eq!(factory.used().iter().map(|name| name.as_str()).collect::<Vec<_>>(), ["stdcore"]);
    // The feature, then the templates feature.
    assert_eq!(factory.features().iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["words", "wc#_"]);
    let blocks = loader.blocks();
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0].package.as_str(), "wc#_");
    let map = json!({ "width": 3 }).as_object().cloned().unwrap_or_default();
    assert_eq!(blocks[0].block.features, [("text".to_string(), FeatureBlock::Settings(map))]);
    let compose = PackageEntry::new("compose").feature(Arc::new(ComposeFeature::new()));
    let registry = registry_with([compose, built(&factory)]);
    let templates: Vec<(&str, &str, &str)> = registry
        .services()
        .services_named::<Template>(TEMPLATES_SLOT)
        .map(|t| (t.name.as_str(), t.package.as_str(), t.text.as_str()))
        .collect();
    assert_eq!(templates, [("fancy", "wc#_", "<main>{{ content }}</main>"), ("bare", "wc#_", "")]);
}

#[test]
fn flm_use_in_a_package_object_is_refused() {
    let main = r#"export default function (ext) { ext.package("_", { flm: { use: ["stdcore"] } }); }"#;
    assert_eq!(
        problems(main),
        ["`flm.use` of the package `_` of the extension `wc` must be written as the package's `uses` list"]
    );
}

#[test]
fn layout_and_unknown_keys_in_a_package_object_s_flm_are_refused() {
    let main = r#"export default function (ext) {
        ext.package("_", { flm: { layout: "page", default_profile: "html", colour: "red", parsing: { nope: 1 } } });
    }"#;
    let found = problems(main);
    assert_eq!(found.len(), 4, "{found:#?}");
    assert!(found[0].starts_with("in `flm` of the package `_` of the extension `wc`, "), "{found:#?}");
    assert_eq!(
        &found[1..],
        [
            "`flm.layout` is not a key of `flm` of the package `_` of the extension `wc`; the keys are `features`, \
             `parsing`, `workflows` and `feature_order`",
            "`flm.default_profile` is not a key of `flm` of the package `_` of the extension `wc`; the keys are \
             `features`, `parsing`, `workflows` and `feature_order`",
            "`flm.colour` is not a key of `flm` of the package `_` of the extension `wc`; the keys are `features`, \
             `parsing`, `workflows` and `feature_order`",
        ]
    );
}

/// A main module of the extension `wc` declaring the package `_` with the
/// features `a` and `b`, in this order, both defining the macro `\foo`, and
/// `flm` holding `flm`.
fn two_definers_of_foo(flm: &str) -> String {
    format!(
        r#"import {{ ElementDef, SyntaxDef }} from "flm:lang/mint";
import {{ ElementSpec }} from "flm:model/element";
function foo(element) {{
  return {{ register(b) {{ b.element(new ElementDef(b.feature(), ["foo"], SyntaxDef.Macro([]), new ElementSpec(element))); }} }};
}}
export default function (ext) {{
  ext.package("_", {{ flm: {flm}, features: {{ a: foo("wc.a"), b: foo("wc.b") }} }});
}}
"#
    )
}

#[test]
fn a_chain_in_a_package_object_s_flm_orders_two_features_and_allows_their_collision() {
    let package = |flm: &str| built(&factory_of(&loader(&[("main.js", &two_definers_of_foo(flm))]), "wc"));
    let without = package("{}");
    assert!(without.feature_order.is_empty());
    let error = try_registry_with([without]).expect_err("the collision is an error");
    assert!(
        error.to_string().contains(
            "Construct `\\foo` from `b` clashes with `\\foo` defined in `a` in text mode; either set `flm: \
             feature_order: ['a < b']` in your front matter to choose `b`'s definition, or have `b` declare `\\foo` \
             as an explicit override."
        ),
        "{error}"
    );
    let with = package(r#"{ feature_order: ["a < b"] }"#);
    let chains: Vec<(String, String)> =
        with.feature_order.iter().map(|chain| (chain.to_string(), chain.origin().to_string())).collect();
    assert_eq!(chains, [("a < b".to_string(), "package wc#_".to_string())]);
    let registry = try_registry_with([with]).unwrap_or_else(|error| panic!("{error}"));
    assert!(registry.build_diagnostics().is_empty());
}

#[test]
fn a_chain_that_is_not_one_in_a_package_object_s_flm_fails_the_load() {
    assert_eq!(
        problems(&two_definers_of_foo(r#"{ feature_order: "a < b < a" }"#)),
        ["in `flm` of the package `_` of the extension `wc`, each feature must appear only once in the chain \
          `a < b < a`, but `a` appears more than once"]
    );
}

#[test]
fn a_bad_entry_of_uses_is_refused() {
    let main = r#"export default function (ext) { ext.package("_", { uses: ["stdcore", "./x", ""] }); }"#;
    let found = problems(main);
    assert_eq!(found.len(), 2, "{found:#?}");
    for problem in &found {
        assert!(problem.starts_with("in `uses` of the package `_` of the extension `wc`, "), "{problem}");
    }
    assert!(found[1].ends_with("an entry of `use` must not be empty"), "{found:#?}");
}

#[test]
fn two_packages_of_one_name_fail_the_load() {
    let main = r#"export default function (ext) { ext.package("core", {}); ext.package("core", {}); }"#;
    assert_eq!(problems(main), ["the extension `wc` (`wc`) declares the package `core` twice"]);
}

#[test]
fn a_default_export_that_throws_fails_the_load_naming_the_extension() {
    assert_eq!(
        problems(r#"export default function (ext) { throw new TypeError("not ready"); }"#),
        ["the default export of the main module `main.js` of the extension `wc` must return without an error: not ready"]
    );
    // A module that throws while evaluating.
    assert_eq!(
        problems(r#"throw new Error("broken"); export default function (ext) {}"#),
        ["the modules of the extension `wc` must evaluate without an error: broken"]
    );
}

#[test]
fn the_main_module_is_js_main_normalized() {
    let main = seeing(r#"import { WORD } from "../lib/word.js";"#, r#"out.table("wc.seen").push(WORD);"#);
    let files = [("src/main.js", main.as_str()), ("lib/word.js", r#"export const WORD = "found";"#)];
    let loader = js_loader(&host(), js_extension("wc", &files, js_manifest("./src/main.js")));
    let registry = registry_with([built(&factory_of(&loader, "wc"))]);
    assert_eq!(seen(&registry), [json!("found")]);
    assert_eq!(on_other_thread(|| seen(&registry)), [json!("found")]);
}

#[test]
fn a_main_module_the_extension_lacks_fails_the_load() {
    let cases = [
        ("lib/main.js", "the extension `wc` names its main module `lib/main.js` (`extension.js.main`), and the extension holds no file `lib/main.js`"),
        ("../main.js", "`extension.js.main` of the extension `wc` is `../main.js`, which leaves the extension's folder"),
        (
            "/main.js",
            "`extension.js.main` of the extension `wc` is `/main.js`, which is an absolute path; it must be a path \
             inside the extension's folder",
        ),
    ];
    for (main, expected) in cases {
        let loader = js_loader(&host(), js_extension("wc", &[("main.js", "")], js_manifest(main)));
        let errors = match loader.resolve("wc") {
            Err(errors) => errors,
            Ok(found) => panic!("{main} loads: {:?}", found.map(|f| f.name().clone())),
        };
        assert_eq!(errors.iter().map(|e| e.detail.as_str()).collect::<Vec<_>>(), [expected], "{main}");
        // A manifest problem: placed in the manifest's file.
        assert_eq!(errors[0].file.as_deref(), Some("wc"), "{main}");
    }
}

#[test]
fn a_module_imported_from_two_places_is_one_module() {
    let main = seeing(
        r#"import "./a.js"; import { again } from "./sub/b.js";"#,
        r#"out.table("wc.seen").push([globalThis.__a, again]);"#,
    );
    let files = [
        ("main.js", main.as_str()),
        ("a.js", "globalThis.__a = (globalThis.__a ?? 0) + 1;"),
        ("sub/b.js", r#"import "../a.js"; export const again = globalThis.__a;"#),
    ];
    let registry = registry_with([built(&factory_of(&loader(&files), "wc"))]);
    assert_eq!(seen(&registry), [json!([1, 1])]);
    assert_eq!(on_other_thread(|| seen(&registry)), [json!([1, 1])]);
}

/// The files of an extension, counting every read.
struct CountingFiles {
    inner: MemoryFiles,
    reads: Arc<AtomicUsize>,
}

impl ExtensionFiles for CountingFiles {
    fn read_text(&self, path: &str) -> Result<Option<String>, FilesError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.read_text(path)
    }

    fn read_bytes(&self, path: &str) -> Result<Option<Vec<u8>>, FilesError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.read_bytes(path)
    }

    fn read_value(&self, path: &str) -> Result<Option<Value>, FilesError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.read_value(path)
    }

    fn list(&self, path: &str) -> Result<Option<Vec<String>>, FilesError> {
        self.reads.fetch_add(1, Ordering::SeqCst);
        self.inner.list(path)
    }

    fn shared(&self) -> Arc<dyn ExtensionFiles + Send + Sync> {
        Arc::new(CountingFiles { inner: self.inner.clone(), reads: Arc::clone(&self.reads) })
    }
}

/// A source of one extension `wc` over counting files.
struct CountingSource {
    files: Vec<(&'static str, String)>,
    reads: Arc<AtomicUsize>,
}

impl ExtensionSource for CountingSource {
    fn find(&self, locator: &Locator) -> Result<Option<FoundExtension>, SourceError> {
        if locator.to_string() != "wc" {
            return Ok(None);
        }
        let inner = self.files.iter().fold(MemoryFiles::new(), |files, (path, text)| files.with_text(path, text));
        Ok(Some(FoundExtension {
            manifest: js_manifest("main.js"),
            identity: "wc".to_string(),
            files: Box::new(CountingFiles { inner, reads: Arc::clone(&self.reads) }),
            place_of: None,
        }))
    }
}

#[test]
fn the_extension_s_files_are_read_at_load_only() {
    let reads = Arc::new(AtomicUsize::new(0));
    let main = seeing(r#"import { k } from "./helpers.js";"#, r#"out.table("wc.seen").push(k);"#);
    let source = CountingSource {
        files: vec![("main.js", main), ("helpers.js", "export const k = 5;".to_string())],
        reads: Arc::clone(&reads),
    };
    let loader = Arc::new(ExtensionLoader::new(vec![Arc::new(source)]).with_js(host()));
    let registry = registry_with([built(&factory_of(&loader, "wc"))]);
    let at_load = reads.load(Ordering::SeqCst);
    assert_eq!(at_load, 2, "the main module and its import");
    assert_eq!(on_other_thread(|| seen(&registry)), [json!(5)]);
    assert_eq!(on_other_thread(|| seen(&registry)), [json!(5)]);
    assert_eq!(seen(&registry), [json!(5)]);
    assert_eq!(reads.load(Ordering::SeqCst), at_load);
}

// --- dynamic import and the queued jobs ----------------------------------------------------------

#[test]
fn a_dynamic_import_at_the_top_of_a_module_fails_the_load() {
    let files = [("main.js", r#"import "./a.js"; export default function (ext) {}"#), ("a.js", r#"import("./b.js");"#), ("b.js", "")];
    assert_eq!(
        load_problems(&loader(&files), "wc"),
        [
            "the module `a.js` of the extension `wc` calls `import(\"./b.js\")`; every module must be imported by an \
             `import` declaration at the top of a module"
        ]
    );
}

#[test]
fn a_dynamic_import_in_the_default_export_fails_the_load() {
    let main = r#"export default function (ext) { import("./b.js").catch(() => {}); }"#;
    assert_eq!(
        load_problems(&loader(&[("main.js", main), ("b.js", "")]), "wc"),
        [
            "the module `main.js` of the extension `wc` calls `import(\"./b.js\")`; every module must be imported by an \
             `import` declaration at the top of a module"
        ]
    );
}

#[test]
fn a_dynamic_import_in_a_handler_fails_the_handler_even_when_caught() {
    let main = seeing("", r#"import("./b.js").catch(() => {}); out.table("wc.seen").push("before");"#);
    let registry = registry_with([built(&factory_of(&loader(&[("main.js", main.as_str()), ("b.js", "")]), "wc"))]);
    for run in [run_demanding(&registry, "wc.seen"), on_other_thread(|| run_demanding(&registry, "wc.seen"))] {
        assert_eq!(
            common::diagnostics(&run),
            [
                "flm.ext.handler-failed: the stage `wc.see` failed: the module `main.js` of the extension `wc` calls \
                 `import(\"./b.js\")`; every module must be imported by an `import` declaration at the top of a module"
            ]
        );
        assert_eq!(items(&run, "wc.seen"), [json!("before")]);
    }
}

#[test]
fn a_handler_s_promise_callbacks_run_before_the_call_returns() {
    let main = seeing(
        "",
        r#"const table = out.table("wc.seen");
           Promise.resolve().then(() => table.push("in a callback"));
           table.push("in the call");"#,
    );
    let registry = registry_with([built(&factory_of(&loader(&[("main.js", main.as_str())]), "wc"))]);
    assert_eq!(seen(&registry), [json!("in the call"), json!("in a callback")]);
}

#[test]
fn a_refused_import_is_recognized_by_its_error_object_not_by_its_text() {
    const REFUSAL: &str = "the module `main.js` of the extension `wc` calls `import(\"./b.js\")`; every module must be \
                           imported by an `import` declaration at the top of a module";
    // The callback rejects its promise with another error of the same text,
    // after the refusal was recorded: it is a rejection of its own.
    let run_js = format!("import(\"./b.js\").catch(() => {{ throw new Error({REFUSAL:?}); }});");
    let main = seeing("", &run_js);
    let registry = registry_with([built(&factory_of(&loader(&[("main.js", main.as_str()), ("b.js", "")]), "wc"))]);
    let run = run_demanding(&registry, "wc.seen");
    assert_eq!(
        common::diagnostics(&run),
        [format!(
            "flm.ext.handler-failed: the stage `wc.see` failed: {REFUSAL}; the stage `wc.see` of the feature `see` of \
             the package `_` of the extension `wc` must leave no promise rejected without a handler: {REFUSAL}"
        )]
    );
}

#[test]
fn several_refused_imports_in_one_handler_call_are_one_error() {
    let main = seeing("", r#"import("./b.js").catch(() => {}); import("./c.js").catch(() => {});"#);
    let registry = registry_with([built(&factory_of(&loader(&[("main.js", main.as_str())]), "wc"))]);
    let run = run_demanding(&registry, "wc.seen");
    assert_eq!(
        common::diagnostics(&run),
        [
            "flm.ext.handler-failed: the stage `wc.see` failed: the module `main.js` of the extension `wc` calls \
             `import(\"./b.js\")`; every module must be imported by an `import` declaration at the top of a module; \
             the module `main.js` of the extension `wc` calls `import(\"./c.js\")`; every module must be imported by \
             an `import` declaration at the top of a module"
        ]
    );
}

#[test]
fn an_import_of_the_extension_s_folder_names_the_folder() {
    for spec in ["./", "."] {
        let main = format!(r#"import "{spec}"; export default function (ext) {{}}"#);
        assert_eq!(
            problems(&main),
            [format!("the module `main.js` of the extension `wc` imports `{spec}`, which names the extension's folder, not a file")],
            "{spec}"
        );
    }
}

#[test]
fn a_stage_whose_promise_throws_is_reported() {
    let main = seeing(
        "",
        r#"out.table("wc.seen").push("before");
           Promise.resolve().then(() => { throw new Error("lost"); });"#,
    );
    let registry = registry_with([built(&factory_of(&loader(&[("main.js", main.as_str())]), "wc"))]);
    let run = run_demanding(&registry, "wc.seen");
    assert_eq!(
        common::diagnostics(&run),
        [
            "flm.ext.handler-failed: the stage `wc.see` failed: the stage `wc.see` of the feature `see` of the package \
             `_` of the extension `wc` must leave no promise rejected without a handler: lost"
        ]
    );
    assert_eq!(items(&run, "wc.seen"), [json!("before")]);
    // A rejection the script handles is no failure.
    let main = seeing(
        "",
        r#"Promise.reject(new Error("handled")).catch(() => {}); out.table("wc.seen").push("ok");"#,
    );
    let registry = registry_with([built(&factory_of(&loader(&[("main.js", main.as_str())]), "wc"))]);
    assert_eq!(seen(&registry), [json!("ok")]);
}

#[test]
fn an_async_default_export_declares_every_package_and_its_failure_fails_the_load() {
    // A package declared after an `await` is read: the default export's
    // promise callbacks run before its packages are.
    let main = r#"export default async function (ext) { ext.package("_", {}); await null; ext.package("late", {}); }"#;
    let loader = loader(&[("main.js", main)]);
    built(&factory_of(&loader, "wc#_"));
    built(&factory_of(&loader, "wc#late"));
    let main = r#"export default async function (ext) { await null; throw new Error("too late"); }"#;
    assert_eq!(
        problems(main),
        [
            "the default export of the main module `main.js` of the extension `wc` must leave no promise rejected \
             without a handler: too late"
        ]
    );
    let main = r#"export default async function (ext) { await null; ext.package("", {}); }"#;
    assert_eq!(
        problems(main),
        [
            "the default export of the main module `main.js` of the extension `wc` must leave no promise rejected \
             without a handler: the extension `wc` calls `ext.package` with a package name that is not a non-empty text"
        ]
    );
}

#[test]
fn a_default_export_left_pending_fails_the_load() {
    // A package declared after an `await` that nothing settles would never
    // be read.
    let main = r#"export default async function (ext) { ext.package("_", {}); await new Promise(() => {}); ext.package("late", {}); }"#;
    assert_eq!(
        problems(main),
        [
            "the default export of the main module `main.js` of the extension `wc` must finish while the extension \
             loads; its promise is still pending after its queued jobs ran"
        ]
    );
}

#[test]
fn ext_package_called_after_the_packages_were_read_is_refused() {
    // The default export keeps `ext`; a feature's `register` calls it during
    // the registry build, after the load read the packages.
    let main = r#"
        let kept;
        export default function (ext) {
          kept = ext;
          ext.package("_", { features: { a: { register(b) { kept.package("late", {}); } } } });
        }
    "#;
    let entry = built(&factory_of(&loader(&[("main.js", main)]), "wc"));
    let error = try_registry_with([entry]).err().map(|error| error.to_string()).unwrap_or_default();
    assert!(
        error.contains(
            "the extension `wc` must call `ext.package` while the default export of its main module `main.js` runs, \
             before its packages are read"
        ),
        "{error}"
    );
}

// --- the rehearsal, the digest, the engine instances ---------------------------------------------

#[test]
fn a_registration_with_another_configuration_is_rehearsed_again() {
    let main = r#"
        import { StageDecl, Effect, Scope } from "flm:core/stage";
        export default function (ext) {
          ext.package("_", { features: { see: {
            settings: { config: { schema: { type: "object", properties: { n: { type: "integer" } } } } },
            register(b, settings) {
              b.table("wc.seen", "the configuration the stage saw");
              b.stage({
                decl: () => new StageDecl("wc.see", Effect.Compute, Scope.Document).writes("wc.seen"),
                run(cx, out) { out.table("wc.seen").push(settings.n ?? 0); },
              });
            },
          } } });
        }
    "#;
    let factory = factory_of(&loader(&[("main.js", main)]), "wc");
    let decl = factory.settings("see").expect("the feature declares settings");
    let build = |n: i64| {
        let mut settings = PackageSettings::new();
        let Value::Object(map) = json!({ "n": n }) else { unreachable!() };
        settings.insert(decl.read(&map).expect("the settings read"));
        registry_with([factory.build(&settings).expect("the package builds")])
    };
    let (one, two) = (build(1), build(2));
    // On this thread, each build ran `register` for real, last with 2.
    assert_eq!((seen(&one), seen(&two), seen(&one)), (vec![json!(1)], vec![json!(2)], vec![json!(1)]));
    // On another thread, each stage is rehearsed with its own configuration.
    let seen_there = on_other_thread(|| (seen(&one), seen(&two), seen(&one)));
    assert_eq!(seen_there, (vec![json!(1)], vec![json!(2)], vec![json!(1)]));
}

#[test]
fn the_digest_covers_the_manifest_every_recorded_module_and_the_engine_s_version() {
    let main = r#"import "./a.js"; export default function (ext) { ext.package("_", {}); }"#;
    let files = MemoryFiles::new().with_text("main.js", main).with_text("a.js", "export {};").with_text("unused.js", "");
    let manifest = Manifest::read("wc", &js_manifest("main.js")).expect("the manifest reads");
    let identity = ExtensionIdentity { identity: "wc".to_string(), stem: "wc".to_string() };
    let loaded = match JsEngine::new(host()).load(&identity, &manifest, Box::new(files)) {
        Ok(loaded) => loaded,
        Err(errors) => panic!("{errors:?}"),
    };
    let mut expected = format!("flm-ext quickjs {ENGINE_VERSION}").into_bytes();
    for (path, text) in [("a.js", "export {};"), ("main.js", main)] {
        expected.push(0);
        expected.extend_from_slice(path.as_bytes());
        expected.push(0);
        expected.extend_from_slice(text.as_bytes());
    }
    expected.push(0);
    assert_eq!(loaded.runtime.digest_input(), expected);
    // Through a loader: the manifest enters, and so does every recorded
    // module; a file no module imports does not.
    let digest = |manifest: Value, a: &str, unused: &str| {
        let files = [("main.js", main), ("a.js", a), ("unused.js", unused)];
        let main_feature = r#"import "./a.js"; export default function (ext) { ext.package("_", { features: { f: { register(b) {} } } }); }"#;
        let files = [("main.js", main_feature), files[1], files[2]];
        let loader = js_loader(&host(), js_extension("wc", &files, manifest));
        registry_with([built(&factory_of(&loader, "wc"))]).digest()
    };
    let first = digest(js_manifest("main.js"), "export {};", "");
    assert_eq!(first, digest(js_manifest("main.js"), "export {};", "changed"));
    assert_ne!(first, digest(js_manifest("main.js"), "export {}; // changed", ""));
    let described = json!({ "extension": { "engine": "js", "js": { "main": "main.js" }, "description": "Words." } });
    assert_ne!(first, digest(described, "export {};", ""));
}

#[test]
fn a_second_load_of_one_identity_gets_engine_instances_of_its_own() {
    let version = |v: &str| seeing(&format!("const V = {v:?};"), r#"out.table("wc.seen").push(V);"#);
    let (first, second) = (version("first"), version("second"));
    let host = host();
    let load = |main: &str| js_loader(&host, js_extension("wc", &[("main.js", main)], js_manifest("main.js")));
    let one = registry_with([built(&factory_of(&load(&first), "wc"))]);
    let second = factory_of(&load(&second), "wc");
    let two = registry_with([built(&second)]);
    assert_eq!((seen(&one), seen(&two)), (vec![json!("first")], vec![json!("second")]));
    let there = on_other_thread(|| (seen(&one), seen(&two), seen(&one)));
    assert_eq!(there, (vec![json!("first")], vec![json!("second")], vec![json!("first")]));
    // Once the first registry and its factory are dropped, the next entry on
    // this thread (the second's registry built again) drops the first load's
    // instance.
    assert_eq!(host.engines_on_this_thread(), 2);
    drop(one);
    let again = registry_with([built(&second)]);
    assert_eq!(host.engines_on_this_thread(), 1);
    assert_eq!(seen(&again), [json!("second")]);
}

// --- case 3: two packages of one extension -------------------------------------------------------

#[test]
fn two_packages_share_a_module_evaluated_once_per_engine_instance() {
    let loader = twopack(str::to_string);
    let found = common::catalog_with(&loader).resolve_all(&["twopack#proofs"]).expect("resolved");
    let registry = registry_of(&found);
    let loads = |registry: &Arc<FlmRegistry>| {
        let run = run_demanding(registry, "twopack.loads");
        assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
        items(&run, "twopack.loads")
    };
    // The loading thread's instance, and a fresh one on another thread.
    assert_eq!(loads(&registry), [json!({ "loads": 1 })]);
    assert_eq!(on_other_thread(|| loads(&registry)), [json!({ "loads": 1 })]);
}

#[test]
fn a_package_using_dot_hash_underscore_brings_the_other_package() {
    let loader = twopack(str::to_string);
    let found = common::catalog_with(&loader).resolve_all(&["twopack#proofs"]).expect("resolved");
    let names: Vec<&str> = found.iter().map(|factory| factory.name().as_str()).collect();
    assert_eq!(names, ["stdcore", "twopack#_", "twopack#proofs"]);
    let (text, diagnostics) = common::text_of(&registry_of(&found), "A \\tpmark{} and \\tpqed{}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A [mark] and [qed].\n");
}

#[test]
fn a_register_refused_by_the_name_rule_keeps_its_kind_and_the_rejected_promise_s_text() {
    let main = r#"export default function (ext) {
  ext.package("_", { features: { see: { register(b, settings) {
    Promise.reject(new Error("lost"));
    b.table("seen", "what the stage saw");
  } } } });
}
"#;
    let error = try_registry_with([built(&factory_of(&loader(&[("main.js", main)]), "wc"))])
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(
        error.contains(
            "flm.ext.name-outside-extension: the table `seen` declared by the package `_` of the extension `wc` must \
             begin with `wc.`; `register` of the feature `see` of the package `_` of the extension `wc` must leave \
             no promise rejected without a handler: lost"
        ),
        "{error}"
    );
}

#[test]
fn a_name_under_the_stem_passes_and_one_under_the_package_s_name_fails() {
    let loader = twopack(|main| main.replace(r#"b.table("twopack.proofs""#, r#"b.table("proofs.count""#));
    let error = try_registry_with([built(&factory_of(&loader, "twopack")), built(&factory_of(&loader, "twopack#proofs"))])
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default();
    assert!(
        error.contains(
            "flm.ext.name-outside-extension: the table `proofs.count` declared by the package `proofs` of the extension \
             `twopack` must begin with `twopack.`"
        ),
        "{error}"
    );
}
