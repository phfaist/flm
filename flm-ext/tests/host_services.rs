//! Host services (E6, plan section 4), through the Rust test double and a
//! memory host: a stage reaches a service its package declared through
//! `cx.host`, the `files` service in its two scopes (the extension's own files
//! served by the adapter, the document's folder by the host's object), any
//! other service through the generic handle; the service sees its caller
//! (ruling S4).

mod common;

use std::sync::{Arc, Mutex};

use flm_ext::adapter::dispatch;
use flm_ext::adapter::handles::{CallTable, Entry};
use flm_ext::api::core::run::StageCx;
use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_ext::api::host::{FilesScope, HostHandle};
use flm_ext::host::{HostAnswer, HostCaller, HostService, HostServiceError};
use flm_ext::{ExtError, MemoryFiles};
use serde_json::{json, Value};

use common::double::{stage, Double};
use common::{ext_items, registry_with, run_demanding, MemoryHostFiles};

/// The package `wc`, whose stage `wc.read` writes to `wc.seen` what `probe`
/// answers, built by `edit` from the double.
fn reading(
    edit: impl FnOnce(Double) -> Double,
    probe: impl Fn(&dyn StageCx) -> Value + Send + Sync + 'static,
) -> Vec<Value> {
    let read = stage(
        StageDecl::new("wc.read", Effect::Compute, Scope::Document).writes("wc.seen"),
        Needs::new(),
        move |cx, out| out.table("wc.seen")?.push(probe(cx)),
    );
    let double = edit(Double::new("wc").digest(b"the code")).feature("read", move |b| {
        b.table("wc.seen", "")?;
        b.stage(Arc::clone(&read))
    });
    let registry = registry_with([double.package()]);
    let run = run_demanding(&registry, &["wc.seen"], &["x"]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    ext_items(&run, "wc.seen")
}

/// An answer as data: the value, or `{error, kind}`.
fn answer<T: serde::Serialize>(result: Result<T, ExtError>) -> Value {
    match result {
        Ok(value) => serde_json::to_value(value).expect("data"),
        Err(error) => json!({ "kind": error.kind(), "error": error.to_string() }),
    }
}

/// The `files` handle of `cx`.
fn files_of(cx: &dyn StageCx) -> Box<dyn flm_ext::api::host::Files> {
    match cx.host("files").expect("declared") {
        HostHandle::Files(files) => files,
        HostHandle::Service(_) => panic!("`files` is lent as the `files` handle"),
    }
}

fn the_extension_s_files() -> MemoryFiles {
    MemoryFiles::new().with_text("data/entries.json", "[1, 2]")
}

fn the_document_s_folder() -> MemoryHostFiles {
    MemoryHostFiles::new(MemoryFiles::new().with_text("bib.json", "{}")).with_link("outside.json")
}

#[test]
fn a_stage_reads_a_declared_files_service_in_both_scopes() {
    let host = Arc::new(the_document_s_folder());
    let seen = reading(
        |double| double.host("files").offering(Arc::clone(&host) as Arc<dyn HostService>).files(the_extension_s_files()),
        |cx| {
            let files = files_of(cx);
            let read = |scope, path: &str| answer(files.read_text(scope, path));
            json!([
                read(FilesScope::Extension, "data/entries.json"),
                read(FilesScope::Extension, "./data/../data/entries.json"),
                read(FilesScope::Extension, "absent.json"),
                read(FilesScope::Extension, "../x.json"),
                read(FilesScope::Extension, "data\\entries.json"),
                read(FilesScope::Document, "bib.json"),
                read(FilesScope::Document, "absent.json"),
                read(FilesScope::Document, "../x.json"),
                read(FilesScope::Document, "outside.json"),
            ])
        },
    );
    assert_eq!(
        seen,
        [json!([
            "[1, 2]",
            "[1, 2]",
            null,
            { "kind": "host", "error": "`files.read_text`: the path `../x.json` leaves the extension's folder" },
            {
                "kind": "host",
                "error": "`files.read_text`: the path `data\\entries.json` must use `/` as its separator, not `\\`",
            },
            "{}",
            null,
            {
                "kind": "host",
                "error": "`files.read_text`: the path `../x.json` must stay inside the document's folder `doc`",
            },
            {
                "kind": "host",
                "error": "`files.read_text`: the file `outside.json` leads outside the document's folder `doc` through \
                          a symbolic link",
            },
        ])]
    );
    // The host's object saw the document scope alone.
    let calls = host.calls.lock().expect("the calls");
    let paths: Vec<&Value> = calls.iter().map(|(_, _, args)| &args["path"]).collect();
    assert_eq!(paths, [&json!("bib.json"), &json!("absent.json"), &json!("../x.json"), &json!("outside.json")]);
    assert!(calls.iter().all(|(_, method, args)| method == "read_text" && args["scope"] == "document"));
}

#[test]
fn an_undeclared_service_is_refused_even_when_the_host_offers_it() {
    let host: Arc<dyn HostService> = Arc::new(the_document_s_folder());
    let seen = reading(
        |double| double.offering(host),
        |cx| json!([answer(cx.host("files").map(|_| ())), answer(cx.host("clock").map(|_| ()))]),
    );
    assert_eq!(
        seen,
        [json!([
            {
                "kind": "argument",
                "error": "`host`: the host service `files` must be declared in the `host` list of the package `_` of \
                          the extension `wc` before a handler calls it",
            },
            {
                "kind": "argument",
                "error": "`host`: the host service `clock` must be declared in the `host` list of the package `_` of \
                          the extension `wc` before a handler calls it; this program offers no host service `clock`",
            },
        ])]
    );
}

/// A service `echo` answering its arguments (`say`), bytes (`bytes`), or a
/// failure (any other method), recording its callers.
#[derive(Default)]
struct Echo {
    callers: Mutex<Vec<HostCaller>>,
}

impl HostService for Echo {
    fn name(&self) -> &str {
        "echo"
    }

    fn call(&self, caller: &HostCaller, method: &str, args: &Value) -> Result<HostAnswer, HostServiceError> {
        self.callers.lock().expect("the callers").push(caller.clone());
        match method {
            "say" => Ok(HostAnswer::Data(args.clone())),
            "bytes" => Ok(HostAnswer::Bytes(vec![1, 2])),
            other => Err(HostServiceError { detail: format!("the service `echo` has no method `{other}`") }),
        }
    }
}

#[test]
fn a_service_is_called_through_the_generic_handle_and_sees_its_caller() {
    let echo = Arc::new(Echo::default());
    let seen = reading(
        |double| double.host("echo").offering(Arc::clone(&echo) as Arc<dyn HostService>),
        |cx| {
            let HostHandle::Service(echo) = cx.host("echo").expect("declared") else { panic!("a generic handle") };
            json!([
                echo.name(),
                answer(echo.call("say", json!({ "a": [1] }))),
                answer(echo.call("bytes", Value::Null)),
                answer(echo.call("shout", Value::Null)),
            ])
        },
    );
    assert_eq!(
        seen,
        [json!([
            "echo",
            { "a": [1] },
            {
                "kind": "argument",
                "error": "`call`: the method `bytes` of the host service `echo` answers bytes, which a script cannot \
                          receive",
            },
            { "kind": "host", "error": "`echo.shout`: the service `echo` has no method `shout`" },
        ])]
    );
    let callers = echo.callers.lock().expect("the callers");
    assert_eq!(callers.len(), 3);
    let caller = &callers[0];
    assert_eq!((caller.extension.identity.as_str(), caller.extension.stem.as_str()), ("wc", "wc"));
    assert_eq!(caller.package, "_");
    assert_eq!(&*caller.digest_input, b"the code");
}

#[test]
fn the_dispatcher_lends_the_handles_and_refuses_an_unknown_scope_by_name() {
    let host: Arc<dyn HostService> = Arc::new(the_document_s_folder());
    let seen = reading(
        |double| double.host("files").offering(host).files(the_extension_s_files()),
        |cx| {
            let mut table = CallTable::new(7);
            let cx_handle = table.lend(Entry::StageCx(cx));
            let call = |table: &mut CallTable<'_>, handle, method: &str, args: Value| {
                let handle = serde_json::from_value(handle).expect("a handle");
                answer(dispatch::call(table, handle, method, &args))
            };
            let files = call(&mut table, json!(cx_handle), "host", json!(["files"]));
            let handle = files["@handle"].clone();
            json!([
                handle["kind"],
                call(&mut table, handle.clone(), "read_text", json!(["extension", "data/entries.json"])),
                call(&mut table, handle, "read_text", json!(["elsewhere", "x"])),
            ])
        },
    );
    assert_eq!(
        seen,
        [json!([
            "files",
            "[1, 2]",
            {
                "kind": "argument",
                "error": "`read_text`: the scope of a file read must be `extension` or `document`, not `elsewhere`",
            },
        ])]
    );
}
