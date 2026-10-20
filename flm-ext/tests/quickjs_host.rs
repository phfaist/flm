//! The QuickJS engine host: engine instances per thread, the module names
//! and how a specifier resolves, the host's modules, the sandbox, the limits,
//! strict mode, and the nested entry into a live instance. The scripts are
//! the tests' own; no API module is involved.
#![cfg(feature = "quickjs")]

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use flm_ext::quickjs::{eval_for_test, set_test_packages, ConsoleLevel, HostModuleError, JsHost, JsSettings, TestPackage};
use flm_ext::ExtError;
use serde_json::json;

/// A main module with a module-level counter: `bump()` answers 1, 2, 3, … for
/// as long as its engine lives.
const COUNTER: &str = "let n = 0; export function bump() { n += 1; return n; }";

fn host(settings: JsSettings) -> JsHost {
    JsHost::new(settings)
}

/// The test extension of the main module `main.js` (its text `main`) and the
/// modules `modules`, by path.
fn package(host: &JsHost, main: &str, modules: &[(&str, &str)]) -> TestPackage {
    let mut all = vec![("main.js", main)];
    all.extend_from_slice(modules);
    match TestPackage::new(host, &all) {
        Ok(package) => package,
        Err(error) => panic!("the test package is refused: {error}"),
    }
}

fn eval(host: &JsHost, package: &TestPackage, source: &str) -> String {
    match eval_for_test(host, package, source) {
        Ok(text) => text,
        Err(error) => panic!("the script fails: {error} ({})", error.data()),
    }
}

/// A host offering `modules`.
fn host_with(modules: &[(&str, &str)]) -> JsHost {
    modules.iter().fold(host(JsSettings::default()), |host, (name, source)| {
        host.with_module(name, source).unwrap_or_else(|error| panic!("the host module is refused: {error}"))
    })
}

/// The prefix of a creation's failure on a thread.
const NOT_CREATED: &str = "the engine of the extension `test` could not be created on this thread: ";

/// A handler failure's text and data; for a failure followed by the drain's
/// texts, the whole text and the data of `ExtError::data`.
fn eval_err(host: &JsHost, package: &TestPackage, source: &str) -> (String, serde_json::Value) {
    match eval_for_test(host, package, source) {
        Ok(text) => panic!("the script succeeds with {text:?}"),
        Err(ExtError::Handler { detail, data }) => (detail, data),
        Err(error @ ExtError::Followed { .. }) => (error.to_string(), error.data()),
        Err(other) => panic!("not a handler failure: {other:?}"),
    }
}

#[test]
fn an_engine_is_created_on_first_use_and_kept_on_its_thread() {
    let host = host(JsSettings::default());
    let package = package(&host, COUNTER, &[]);
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
    assert_eq!(eval(&host, &package, "return main.bump();"), "2");
    host.discard_engines_on_this_thread();
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
}

#[test]
fn a_module_imports_another_by_its_path() {
    let host = host(JsSettings::default());
    let package = package(
        &host,
        r#"import { twice } from "./helpers.js"; export const v = twice(21);"#,
        &[("helpers.js", "export function twice(x) { return 2 * x; }")],
    );
    assert_eq!(eval(&host, &package, "return main.v;"), "42");
}

#[test]
fn a_relative_specifier_is_joined_to_the_importing_module_s_folder() {
    let host = host(JsSettings::default());
    let package = package(
        &host,
        r#"import { twice } from "./lib/helpers.js"; export const v = twice(5);"#,
        &[
            ("lib/helpers.js", r#"import { one } from "../util.js"; export function twice(x) { return 2 * x + one; }"#),
            ("util.js", "export const one = 1;"),
        ],
    );
    assert_eq!(eval(&host, &package, "return main.v;"), "11");
}

#[test]
fn an_unknown_import_names_the_module_and_the_specifier() {
    let host = host_with(&[("lib", "export const k = 1;")]);
    let cases = [
        // J2: no file; the extension written in the specifier is required.
        (r#"import "./nope.js";"#, "the module `main.js` of the extension `test` imports `./nope.js`, and the extension holds no file `nope.js`"),
        (r#"import "./helpers";"#, "the module `main.js` of the extension `test` imports `./helpers`, and the extension holds no file `helpers`"),
        // J3: a climb.
        (r#"import "../x.js";"#, "the module `main.js` of the extension `test` imports `../x.js`, which leaves the extension's folder"),
        // J3b: an absolute path.
        (
            r#"import "/x.js";"#,
            "the module `main.js` of the extension `test` imports `/x.js`, which is an absolute path; a module of the \
             extension is imported by a path beginning with `./` or `../`",
        ),
        // J4: a bare name no host module has (a bare name is never a file of the extension).
        (
            r#"import "helpers.js";"#,
            "the module `main.js` of the extension `test` imports `helpers.js`, which is neither a module of the \
             extension (`./…`), nor of the extension API (`flm:…`), nor a module this program offers",
        ),
        (
            r#"import "node:fs";"#,
            "the module `main.js` of the extension `test` imports `node:fs`, which is neither a module of the \
             extension (`./…`), nor of the extension API (`flm:…`), nor a module this program offers",
        ),
    ];
    for (main, expected) in cases {
        let package = package(&host, main, &[("helpers.js", "export {};")]);
        let (detail, data) = eval_err(&host, &package, "return 1;");
        assert_eq!(detail, format!("{NOT_CREATED}{expected}"), "{main}");
        assert_eq!(data["refusals"], json!([expected]), "{main}");
    }
    // J5: a module of the extension API that does not exist, from a module
    // that is not the main one.
    let nested = package(&host, r#"import "./sub/a.js";"#, &[("sub/a.js", r#"import "flm:core/nothing";"#)]);
    let (detail, _) = eval_err(&host, &nested, "return 1;");
    assert!(
        detail.starts_with(&format!(
            "{NOT_CREATED}the module `sub/a.js` of the extension `test` imports `flm:core/nothing`, which is not a \
             module of the extension API; its modules are `flm:core/artifact`, `flm:core/data/store`, "
        )),
        "{detail}"
    );
    assert_eq!(detail.matches("`flm:").count(), 34, "{detail}");
    // J4 worded for a host module.
    let host = host_with(&[("lib", r#"import "other";"#)]);
    let from_host = package(&host, r#"import "lib";"#, &[]);
    let (detail, _) = eval_err(&host, &from_host, "return 1;");
    assert_eq!(
        detail,
        format!("{NOT_CREATED}the module `lib` this program offers imports `other`, which is not a module this program offers")
    );
}

#[test]
fn a_host_module_name_that_breaks_the_rules_is_refused() {
    let cases = [
        ("", "the name of a module this program offers must not be empty"),
        ("a\0b", "the module name `a\\0b` this program offers must not hold a NUL character"),
        ("./x", "the module name `./x` this program offers must be a bare name, not a path"),
        ("/x", "the module name `/x` this program offers must be a bare name, not a path"),
        ("flm:x", "the module name `flm:x` this program offers must not hold `:`, which marks the extension API (`flm:`) and other schemes"),
        ("node:fs", "the module name `node:fs` this program offers must not hold `:`, which marks the extension API (`flm:`) and other schemes"),
    ];
    for (name, detail) in cases {
        match host(JsSettings::default()).with_module(name, "export {};") {
            Err(error) => assert_eq!(error, HostModuleError { name: name.to_string(), detail: detail.to_string() }),
            Ok(_) => panic!("the host module name {name:?} is accepted"),
        }
    }
    match host_with(&[("lib", "")]).with_module("lib", "export {};") {
        Err(error) => assert_eq!(error.to_string(), "this program offers two modules named `lib`"),
        Ok(_) => panic!("a host module is added twice"),
    }
    // `main` is a name like any other.
    host_with(&[("main", "export {};")]);
}

#[test]
fn an_extension_module_and_a_host_module_of_one_name_are_two_modules() {
    let host = host_with(&[("helpers", "export const k = 'host';")]);
    let package = package(
        &host,
        r#"import { k as a } from "helpers"; import { k as b } from "./helpers"; export const v = a + "," + b;"#,
        &[("helpers", "export const k = 'extension';")],
    );
    assert_eq!(eval(&host, &package, "return main.v;"), "host,extension");
}

#[test]
fn a_host_module_is_importable_by_every_extension() {
    let host = host_with(&[("shared", "export const k = 7;")]);
    assert_eq!(host.modules().collect::<Vec<_>>(), [("shared", "export const k = 7;")]);
    let first = package(&host, r#"import { k } from "shared"; export const v = k + 1;"#, &[]);
    let second = package(&host, r#"import { k } from "shared"; export const v = k + 2;"#, &[]);
    assert_eq!(eval(&host, &first, "return main.v;"), "8");
    assert_eq!(eval(&host, &second, "return main.v;"), "9");
}

#[test]
fn a_host_module_imports_another_by_a_relative_path() {
    let host = host_with(&[
        ("mathjax/core", r#"import { u } from "./util"; import { w } from "other"; export const k = u + w;"#),
        ("mathjax/util", "export const u = 3;"),
        ("other", "export const w = 4;"),
    ]);
    let package = package(&host, r#"import { k } from "mathjax/core"; export const v = k;"#, &[]);
    assert_eq!(eval(&host, &package, "return main.v;"), "7");
}

#[test]
fn a_test_package_path_that_leaves_the_folder_is_refused() {
    let host = host(JsSettings::default());
    match TestPackage::new(&host, &[("../main.js", "")]) {
        Err(ExtError::Argument { detail, .. }) => assert_eq!(detail, "the path `../main.js` leaves the extension's folder"),
        Err(other) => panic!("not an argument error: {other:?}"),
        Ok(_) => panic!("the path is accepted"),
    }
}

#[test]
fn no_timer_file_network_or_process_object_is_reachable() {
    let host = host(JsSettings::default());
    let package = package(&host, COUNTER, &[]);
    let kinds = eval(
        &host,
        &package,
        "return [typeof setTimeout, typeof setInterval, typeof fetch, typeof XMLHttpRequest, \
         typeof process, typeof require, typeof os, typeof std].join(',');",
    );
    assert_eq!(kinds, ["undefined"; 8].join(","));
}

#[test]
fn date_now_and_new_date_are_the_fixed_instant() {
    let host = host(JsSettings::default());
    let package = package(&host, COUNTER, &[]);
    let dates = eval(
        &host,
        &package,
        "return [Date.now(), new Date().getTime(), new Date(5).getTime(), \
         new Date(Date.UTC(2020, 0, 1)).toISOString(), Date.parse('1970-01-02T00:00:00Z')].join(',');",
    );
    assert_eq!(dates, "0,0,5,2020-01-01T00:00:00.000Z,86400000");
    // The real clock is not reachable through the replaced `Date`.
    let reached = eval(
        &host,
        &package,
        "class Later extends Date {} \
         return [typeof Object.getPrototypeOf(Date).now, Date.prototype.constructor === Date, \
         new (Date.prototype.constructor)().getTime(), new (Object.getPrototypeOf(new Date()).constructor)().getTime(), \
         Object.getPrototypeOf(new Date()).constructor.now(), Date() === new Date(0).toString(), \
         new Later().getTime(), new Later() instanceof Date].join(',');",
    );
    assert_eq!(reached, "undefined,true,0,0,0,true,0,true");
}

#[test]
fn math_random_restarts_at_every_call() {
    let host = host(JsSettings::default());
    let package = package(&host, COUNTER, &[]);
    let first = eval(&host, &package, "return [Math.random(), Math.random()].join(',');");
    let second = eval(&host, &package, "return [Math.random(), Math.random()].join(',');");
    assert_eq!(first, second);
    let mut parts = first.split(',');
    assert_ne!(parts.next(), parts.next());
}

#[test]
fn performance_now_is_fixed() {
    let host = host(JsSettings::default());
    let package = package(&host, COUNTER, &[]);
    let now = eval(
        &host,
        &package,
        "let x = 0; for (let i = 0; i < 100000; i++) x += i; return performance.now();",
    );
    assert_eq!(now, "0");
}

#[test]
fn console_output_reaches_the_host_sink_with_its_level() {
    let lines: Arc<Mutex<Vec<(ConsoleLevel, String)>>> = Arc::default();
    let sink_lines = Arc::clone(&lines);
    let host = host(JsSettings::default()).with_console(Arc::new(move |level, text| {
        if let Ok(mut lines) = sink_lines.lock() {
            lines.push((level, text.to_string()));
        }
    }));
    let package = package(&host, r#"console.info("loaded");"#, &[]);
    eval(
        &host,
        &package,
        r#"console.log("a", 1, null); console.debug("d"); console.warn({}); console.error("e"); return "";"#,
    );
    let lines = lines.lock().map(|lines| lines.clone()).unwrap_or_default();
    assert_eq!(
        lines,
        [
            (ConsoleLevel::Info, "loaded".to_string()),
            (ConsoleLevel::Log, "a 1 null".to_string()),
            (ConsoleLevel::Debug, "d".to_string()),
            (ConsoleLevel::Warn, "[object Object]".to_string()),
            (ConsoleLevel::Error, "e".to_string()),
        ]
    );
}

#[test]
fn a_handler_over_its_time_budget_is_interrupted_and_the_engine_is_replaced() {
    let host = host(JsSettings { time_budget: Some(Duration::from_millis(50)), ..JsSettings::default() });
    let package = package(&host, COUNTER, &[]);
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
    assert_eq!(eval(&host, &package, "return main.bump();"), "2");
    let (detail, data) = eval_err(&host, &package, "try { while (true) {} } catch (e) { return 'caught'; }");
    assert_eq!(detail, "the handler must finish within its time budget of 50 ms");
    assert_eq!(data, json!({ "limit": "time", "budget_ms": 50 }));
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
}

#[test]
fn a_module_that_does_not_finish_within_the_time_budget_fails_the_engine_s_creation() {
    let host = host(JsSettings { time_budget: Some(Duration::from_millis(50)), ..JsSettings::default() });
    let package = package(&host, r#"import "./helpers.js";"#, &[("helpers.js", "while (true) {}")]);
    let (detail, data) = eval_err(&host, &package, "return 1;");
    assert_eq!(
        detail,
        format!("{NOT_CREATED}the modules of the extension `test` must finish evaluating within the time budget of 50 ms")
    );
    assert_eq!(data["limit"], "time");
}

#[test]
fn a_handler_over_the_memory_limit_fails_and_the_engine_is_replaced() {
    let host = host(JsSettings { memory_limit: Some(4 << 20), ..JsSettings::default() });
    let package = package(&host, COUNTER, &[]);
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
    assert_eq!(eval(&host, &package, "return main.bump();"), "2");
    let (detail, data) =
        eval_err(&host, &package, "const a = []; while (true) a.push(new Array(1000).fill('x')); return 'never';");
    assert_eq!(detail, format!("the handler must stay within the memory limit of {} bytes", 4 << 20));
    assert_eq!(data, json!({ "limit": "memory", "limit_bytes": 4 << 20 }));
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
}

#[test]
fn a_queued_job_over_the_memory_limit_fails_the_call_and_the_engine_is_replaced() {
    let host = host(JsSettings { memory_limit: Some(4 << 20), ..JsSettings::default() });
    let package = package(&host, COUNTER, &[]);
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
    assert_eq!(eval(&host, &package, "return main.bump();"), "2");
    let (detail, data) = eval_err(
        &host,
        &package,
        "Promise.resolve().then(() => { const a = []; for (;;) a.push(new Array(100000)); }); return 1;",
    );
    assert_eq!(detail, format!("the handler must stay within the memory limit of {} bytes", 4 << 20));
    assert_eq!(data, json!({ "limit": "memory", "limit_bytes": 4 << 20 }));
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
}

#[test]
fn a_thrown_null_under_a_memory_limit_is_an_ordinary_exception() {
    let host = host(JsSettings { memory_limit: Some(4 << 20), ..JsSettings::default() });
    let package = package(&host, COUNTER, &[]);
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
    let (detail, data) = eval_err(&host, &package, "throw null;");
    assert_eq!(detail, "null");
    assert!(data.get("limit").is_none(), "{data}");
    // The instance is kept.
    assert_eq!(eval(&host, &package, "return main.bump();"), "2");
    // A memory limit reached in a call whose script caught the failure does
    // not make a later call's `throw null` the limit.
    let caught = "try { const a = []; for (;;) a.push(new Array(100000)); } catch (e) { return 'caught'; }";
    assert_eq!(eval(&host, &package, caught), "caught");
    let (detail, data) = eval_err(&host, &package, "throw null;");
    assert_eq!(detail, "null");
    assert!(data.get("limit").is_none(), "{data}");
}

#[test]
fn a_promise_rejected_with_null_under_a_memory_limit_is_an_ordinary_rejection() {
    let host = host(JsSettings { memory_limit: Some(4 << 20), ..JsSettings::default() });
    let package = package(&host, COUNTER, &[]);
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
    let (detail, data) = eval_err(&host, &package, "Promise.reject(null); return 1;");
    assert_eq!(detail, "the handler must leave no promise rejected without a handler: null");
    assert!(data.get("limit").is_none(), "{data}");
    assert_eq!(eval(&host, &package, "return main.bump();"), "2");
}

#[test]
fn an_instance_of_a_dropped_extension_is_swept_at_the_next_entry() {
    let host = host(JsSettings::default());
    let a = package(&host, COUNTER, &[]);
    assert_eq!(eval(&host, &a, "return main.bump();"), "1");
    assert_eq!(host.engines_on_this_thread(), 1);
    drop(a);
    // Kept until the next entry into an instance on this thread.
    assert_eq!(host.engines_on_this_thread(), 1);
    let b = package(&host, COUNTER, &[]);
    assert_eq!(eval(&host, &b, "return main.bump();"), "1");
    assert_eq!(host.engines_on_this_thread(), 1);
    // Swept at an entry into another extension's instance that already exists.
    let c = package(&host, COUNTER, &[]);
    assert_eq!(eval(&host, &c, "return main.bump();"), "1");
    assert_eq!(host.engines_on_this_thread(), 2);
    drop(c);
    assert_eq!(eval(&host, &b, "return main.bump();"), "2");
    assert_eq!(host.engines_on_this_thread(), 1);
}

#[test]
fn a_handler_over_the_stack_fails_and_the_engine_is_kept() {
    let host = host(JsSettings { stack_size: 256 << 10, ..JsSettings::default() });
    let package = package(&host, COUNTER, &[]);
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
    let (detail, data) = eval_err(&host, &package, "function f() { return f() + 1; } return f();");
    assert_eq!(detail, "Maximum call stack size exceeded");
    assert_eq!(data["name"], "RangeError");
    // The script may catch it.
    let caught = eval(
        &host,
        &package,
        "function f() { return f() + 1; } try { return f(); } catch (e) { return e.name; }",
    );
    assert_eq!(caught, "RangeError");
    assert_eq!(eval(&host, &package, "return main.bump();"), "2");
}

#[test]
fn a_thrown_error_is_rendered_with_name_message_and_stack() {
    let host = host(JsSettings::default());
    let package = package(
        &host,
        "export function fail() { throw new TypeError('the word must be plain text'); }",
        &[],
    );
    let (detail, data) = eval_err(&host, &package, "main.fail();");
    assert_eq!(detail, "the word must be plain text");
    assert_eq!(data["name"], "TypeError");
    assert_eq!(data["flm_error"], json!(null));
    let stack = data["stack"].as_str().unwrap_or_default();
    assert!(stack.contains("fail"), "{stack}");
    assert!(stack.contains("main.js"), "{stack}");

    let (detail, data) = eval_err(&host, &package, "throw { reason: 'no' };");
    assert_eq!(detail, "[object Object]");
    assert_eq!(data, json!({ "value": { "reason": "no" } }));
    let (detail, data) = eval_err(&host, &package, "throw 'plain';");
    assert_eq!(detail, "plain");
    assert_eq!(data, json!({ "value": "plain" }));

    let (_, data) = eval_err(
        &host,
        &package,
        "class FlmError extends Error { constructor(d) { super(d.detail); this.name = 'FlmError'; this.data = d; } } \
         throw new FlmError({ kind: 'stale_handle', detail: 'old' });",
    );
    assert_eq!(data["flm_error"], json!({ "kind": "stale_handle", "detail": "old" }));
}

#[test]
fn a_main_module_that_throws_fails_the_engine_s_creation() {
    let host = host(JsSettings::default());
    let throwing = package(&host, "throw new Error('the module refuses to load');", &[]);
    let (detail, data) = eval_err(&host, &throwing, "return 1;");
    assert_eq!(detail, format!("{NOT_CREATED}the modules of the extension `test` must evaluate without an error: the module refuses to load"));
    assert_eq!(data["name"], "Error");
    let awaiting = package(&host, "await new Promise(() => {});", &[]);
    let (detail, _) = eval_err(&host, &awaiting, "return 1;");
    assert_eq!(
        detail,
        format!("{NOT_CREATED}the modules of the extension `test` must finish evaluating without waiting: a top-level `await` is not supported")
    );
    let invalid = package(&host, "export const = 1;", &[]);
    let (detail, _) = eval_err(&host, &invalid, "return 1;");
    assert!(detail.starts_with(&format!("{NOT_CREATED}the modules of the extension `test` must be valid JavaScript: ")), "{detail}");
    assert!(detail.contains("main.js:1"), "{detail}");
}

#[test]
fn strict_mode_starts_every_call_from_a_fresh_engine() {
    let host = host(JsSettings { strict: true, ..JsSettings::default() });
    let package = package(&host, COUNTER, &[]);
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
    // A nested call runs on the outermost call's engine.
    assert_eq!(
        eval(&host, &package, "main.bump(); return __flm_test_nested('return main.bump();');"),
        "2"
    );
}

#[test]
fn each_thread_has_its_own_engine() {
    let host = host(JsSettings::default());
    let package = package(&host, COUNTER, &[]);
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
    assert_eq!(eval(&host, &package, "return main.bump();"), "2");
    let answers: Vec<Vec<String>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let (host, package) = (&host, &package);
                std::thread::Builder::new()
                    .stack_size(64 << 20)
                    .spawn_scoped(scope, move || {
                        (0..2).map(|_| eval(host, package, "return main.bump();")).collect::<Vec<_>>()
                    })
                    .unwrap_or_else(|error| panic!("cannot spawn: {error}"))
            })
            .collect();
        handles.into_iter().map(|handle| handle.join().unwrap_or_default()).collect()
    });
    assert_eq!(answers, [["1", "2"], ["1", "2"]]);
    assert_eq!(eval(&host, &package, "return main.bump();"), "3");
}

#[test]
fn a_nested_entry_reuses_the_live_context() {
    let host = host(JsSettings::default());
    let package = package(&host, COUNTER, &[]);
    let answer = eval(
        &host,
        &package,
        "const inner = __flm_test_nested('return main.bump();'); \
         const twice = __flm_test_nested(\"return __flm_test_nested('return main.bump();');\"); \
         const failed = __flm_test_nested(\"throw new Error('inner failure');\"); \
         return [inner, twice, main.bump(), failed].join('|');",
    );
    assert_eq!(answer, "1|2|3|error: inner failure");
    // The engine is the same afterwards.
    assert_eq!(eval(&host, &package, "return main.bump();"), "4");
}

#[test]
fn a_nested_call_never_runs_the_callbacks_of_the_call_it_interrupts() {
    let host = host(JsSettings::default());
    let package = package(&host, COUNTER, &[]);
    let answer = eval(
        &host,
        &package,
        "globalThis.order = []; \
         Promise.resolve().then(() => order.push('outer callback')); \
         const inner = __flm_test_nested(\"Promise.resolve().then(() => order.push('inner callback')); return order.join(',');\"); \
         return inner + '|' + order.join(',');",
    );
    // Neither callback ran inside the nested call, nor before the outer call
    // returned its value; both ran when the outermost call ended.
    assert_eq!(answer, "|");
    assert_eq!(eval(&host, &package, "return order.join(',');"), "outer callback,inner callback");
    // A refusal met in a nested call fails the outermost call.
    let (detail, _) = eval_err(
        &host,
        &package,
        "const inner = __flm_test_nested('import(\"./x.js\").catch(() => {}); return 1;'); return inner;",
    );
    assert_eq!(
        detail,
        "the module `eval_script` calls `import(\"./x.js\")`; every module must be imported by an `import` \
         declaration at the top of a module"
    );
}

#[test]
fn a_promise_rejected_with_no_handler_fails_the_call() {
    let host = host(JsSettings::default());
    let package = package(&host, COUNTER, &[]);
    let (detail, data) = eval_err(&host, &package, "Promise.reject(new Error('lost')); return 1;");
    assert_eq!(detail, "the handler must leave no promise rejected without a handler: lost");
    assert_eq!(data, json!({ "rejections": [detail] }));
    let (detail, _) = eval_err(&host, &package, "(async () => { throw 'later'; })(); return 1;");
    assert_eq!(detail, "the handler must leave no promise rejected without a handler: later");
    // Handled, even after the rejection, it is no failure.
    assert_eq!(eval(&host, &package, "const p = Promise.reject(new Error('x')); p.catch(() => {}); return 1;"), "1");
    // A call that throws keeps the rejections it left in its text.
    let (detail, _) = eval_err(&host, &package, "Promise.reject(new Error('left')); throw new Error('thrown');");
    assert_eq!(detail, "thrown; the handler must leave no promise rejected without a handler: left");
    assert_eq!(eval(&host, &package, "return 2;"), "2");
}

#[test]
fn a_failed_call_s_queued_jobs_run_at_its_own_end_and_never_in_the_next_call() {
    let host = host(JsSettings::default());
    let package = package(&host, COUNTER, &[]);
    // The failure of a queued job is the failed call's own, after its text.
    let (detail, _) = eval_err(
        &host,
        &package,
        "Promise.resolve().then(() => { throw new Error('from first'); }); throw new Error('first');",
    );
    assert_eq!(detail, "first; the handler must leave no promise rejected without a handler: from first");
    // The next call did nothing wrong, and succeeds.
    assert_eq!(eval(&host, &package, "return 2;"), "2");
    // A harmless job of a failed call runs before that call returns.
    let (detail, _) = eval_err(
        &host,
        &package,
        "globalThis.k = 0; Promise.resolve().then(() => { globalThis.k = 1; }); throw new Error('again');",
    );
    assert_eq!(detail, "again");
    assert_eq!(eval(&host, &package, "return globalThis.k;"), "1");
}

#[test]
fn a_rejection_whose_reason_has_no_text_is_reported_by_another_form() {
    let host = host(JsSettings::default());
    let package = package(&host, COUNTER, &[]);
    const PREFIX: &str = "the handler must leave no promise rejected without a handler: ";
    let (detail, _) = eval_err(&host, &package, "Promise.reject({ toString() { throw new Error('no'); } }); return 1;");
    assert_eq!(detail, format!("{PREFIX}a value without a text form"));
    let (detail, _) = eval_err(&host, &package, "Promise.reject({ toString() { return ''; }, n: 3 }); return 1;");
    assert_eq!(detail, format!("{PREFIX}{{\"n\":3}}"));
    let (detail, _) = eval_err(&host, &package, "Promise.reject(new TypeError()); return 1;");
    assert_eq!(detail, format!("{PREFIX}TypeError"));
}

#[test]
fn a_nested_call_over_its_time_budget_poisons_the_engine_and_the_outer_call_continues() {
    let host = host(JsSettings { time_budget: Some(Duration::from_millis(50)), ..JsSettings::default() });
    let package = package(&host, COUNTER, &[]);
    let answer = eval(
        &host,
        &package,
        "main.bump(); const inner = __flm_test_nested('while (true) {}'); return main.bump() + '|' + inner;",
    );
    assert_eq!(answer, "2|error: the handler must finish within its time budget of 50 ms");
    // Discarded when the outer call ended.
    assert_eq!(eval(&host, &package, "return main.bump();"), "1");
}

#[test]
fn a_nested_call_of_another_package_creates_its_engine_and_reenters_the_first() {
    let host = host(JsSettings::default());
    let a = package(&host, COUNTER, &[]);
    let b = package(&host, COUNTER, &[]);
    set_test_packages(&[a.clone(), b.clone()]);
    // A -> B (B's engine created inside A's call) -> A (A's live context
    // found below B's frame).
    let answer = eval(
        &host,
        &a,
        "const x = main.bump(); \
         const y = __flm_test_nested_in(1, \"return main.bump() + ',' + __flm_test_nested_in(0, 'return main.bump();');\"); \
         return [x, y, main.bump()].join('|');",
    );
    assert_eq!(answer, "1|1,2|3");
    // Both engines are kept.
    assert_eq!(eval(&host, &a, "return main.bump();"), "4");
    assert_eq!(eval(&host, &b, "return main.bump();"), "2");
}

#[test]
fn a_nested_call_of_another_package_pauses_the_outer_call_s_budget() {
    const SPIN: &str = "let s = 0; for (let i = 0; i < 300000; i++) { s += i; } return 'spun';";
    // B has no budget of its own (another host), so that only A's counts.
    let free = host(JsSettings { time_budget: None, ..JsSettings::default() });
    let b = package(&free, COUNTER, &[]);
    // How long one call of SPIN takes here.
    eval(&free, &b, SPIN);
    let start = Instant::now();
    eval(&free, &b, SPIN);
    let spin = start.elapsed().max(Duration::from_millis(1));
    let budget = (spin * 3).max(Duration::from_millis(100));
    let calls = (2 * budget).as_millis() / spin.as_millis().max(1) + 1;
    // A calls B for twice A's budget in all, then works a little itself:
    // only A's own time counts against A's budget.
    let host = host(JsSettings { time_budget: Some(budget), ..JsSettings::default() });
    let a = package(&host, COUNTER, &[]);
    set_test_packages(&[a.clone(), b]);
    let source = format!(
        "const r = []; for (let k = 0; k < {calls}; k++) {{ r.push(__flm_test_nested_in(1, {SPIN:?})); }} \
         let s = 0; for (let i = 0; i < 100000; i++) {{ s += i; }} \
         return r.every(x => x === 'spun') ? 'ok' : r.join('|');"
    );
    assert_eq!(eval(&host, &a, &source), "ok");
}

#[test]
fn a_time_budget_past_the_clock_s_range_is_no_limit() {
    let host = host(JsSettings { time_budget: Some(Duration::MAX), ..JsSettings::default() });
    let package = package(&host, COUNTER, &[]);
    assert_eq!(eval(&host, &package, "return __flm_test_nested('return main.bump();');"), "1");
}

#[test]
fn the_call_native_answers_the_err_envelope() {
    let host = host(JsSettings::default());
    let package = package(&host, r#"export const at_load = __flm.call("{}", "chars", "[]");"#, &[]);
    let at_load: serde_json::Value =
        serde_json::from_str(&eval(&host, &package, "return main.at_load;")).unwrap_or_default();
    assert_eq!(at_load["err"]["kind"], "handler");
    assert_eq!(
        at_load["err"]["detail"],
        "the extension `test` must call `chars` of the extension API from a feature's `register` or from a handler \
         that `register` declared"
    );
    let in_call: serde_json::Value =
        serde_json::from_str(&eval(&host, &package, r#"return __flm.call("{}", "chars", "[]");"#))
            .unwrap_or_default();
    // Inside a handler call, the dispatcher answers: `{}` is not a handle.
    assert_eq!(in_call["err"]["kind"], "argument");
    assert_eq!(in_call["err"]["method"], "chars");
}

#[test]
fn the_host_and_a_package_may_be_shared_between_threads() {
    fn shareable<T: Send + Sync>() {}
    shareable::<JsHost>();
    shareable::<TestPackage>();
    let defaults = JsSettings::default();
    assert_eq!(
        (defaults.memory_limit, defaults.time_budget, defaults.stack_size, defaults.strict),
        (Some(64 << 20), Some(Duration::from_secs(10)), 1 << 20, false)
    );
}
