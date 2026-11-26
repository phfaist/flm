//! Host services, through the Rust test double and a
//! memory host: a stage reaches a service its package declared through
//! `cx.host`, the `files` service at a qualified path (the extension's own
//! files under `self` served by the adapter, the other roots by the host's
//! object), any
//! other service through the generic handle; the service sees its caller and
//! the scope of the handler call.

mod common;

use std::sync::{Arc, Mutex};

use flm_core::progress::{Kind, Observer, Scope as ProgressScope, Sheet};
use flm_ext::adapter::dispatch;
use flm_ext::adapter::handles::{CallTable, Entry};
use flm_ext::api::core::run::StageCx;
use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_ext::api::host::HostHandle;
use flm_ext::host::{HostAnswer, HostCaller, HostService, HostServiceError};
use flm_ext::{ExtError, MemoryFiles};
use serde_json::{json, Value};

use common::double::{stage, Double};
use common::{ext_items, registry_with, run_demanding_under, MemoryHostFiles};

/// The package `wc`, whose stage `wc.read` writes to `wc.seen` what `probe`
/// answers, built by `edit` from the double.
fn reading(
    edit: impl FnOnce(Double) -> Double,
    probe: impl Fn(&dyn StageCx) -> Value + Send + Sync + 'static,
) -> Vec<Value> {
    reading_under(ProgressScope::none(), edit, probe)
}

/// As [`reading`], the run reporting under `progress`.
fn reading_under(
    progress: ProgressScope,
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
    let run = run_demanding_under(&registry, &["wc.seen"], &["x"], progress);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    ext_items(&run, "wc.seen")
}

/// An answer as data: the value, or the error's data (`ExtError::data`)
/// without its free text `detail`.
fn answer<T: serde::Serialize>(result: Result<T, ExtError>) -> Value {
    match result {
        Ok(value) => serde_json::to_value(value).expect("data"),
        Err(error) => {
            let mut data = error.data();
            if let Some(map) = data.as_object_mut() {
                map.remove("detail");
            }
            data
        }
    }
}

/// As [`answer`], the error's `detail` kept: for a failure whose text the
/// test's own host service gives.
fn answer_with_detail<T: serde::Serialize>(result: Result<T, ExtError>) -> Value {
    match result {
        Ok(value) => serde_json::to_value(value).expect("data"),
        Err(error) => error.data(),
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
fn a_stage_reads_a_declared_files_service_at_a_qualified_path() {
    let host = Arc::new(the_document_s_folder());
    let seen = reading(
        |double| double.host("files").offering(Arc::clone(&host) as Arc<dyn HostService>).files(the_extension_s_files()),
        |cx| {
            let files = files_of(cx);
            let read = |path: &str| answer(files.read_text(path));
            // The document's folder is the test's own service: its texts are kept.
            let read_document = |path: &str| answer_with_detail(files.read_text(path));
            json!([
                read("self:data/entries.json"),
                read("self:./data/../data/entries.json"),
                read("self:absent.json"),
                read("self:../x.json"),
                read("self:data\\entries.json"),
                read_document("doc:bib.json"),
                read_document("doc:absent.json"),
                read_document("doc:../x.json"),
                read_document("doc:outside.json"),
            ])
        },
    );
    let refused = json!({ "kind": "host", "service": "files", "method": "read_text" });
    assert_eq!(
        seen,
        [json!([
            "[1, 2]",
            "[1, 2]",
            null,
            refused,
            null,
            "{}",
            null,
            {
                "kind": "host", "service": "files", "method": "read_text",
                "detail": "the path `../x.json` must stay inside the document's folder `doc`",
            },
            {
                "kind": "host", "service": "files", "method": "read_text",
                "detail": "the file `outside.json` leads outside the document's folder `doc` through a symbolic link",
            },
        ])]
    );
    // The host's object saw the paths under `doc` alone.
    let calls = host.calls.lock().expect("the calls");
    let paths: Vec<&Value> = calls.iter().map(|(_, _, args)| &args["path"]).collect();
    assert_eq!(paths, [&json!("doc:bib.json"), &json!("doc:absent.json"), &json!("doc:../x.json"), &json!("doc:outside.json")]);
    assert!(calls.iter().all(|(_, method, _)| method == "read_text"));
}

#[test]
fn a_bare_path_given_to_the_files_service_is_refused() {
    let host = Arc::new(the_document_s_folder());
    let seen = reading(
        |double| double.host("files").offering(Arc::clone(&host) as Arc<dyn HostService>).files(the_extension_s_files()),
        |cx| answer(files_of(cx).read_text("data/entries.json")),
    );
    assert_eq!(seen, [json!({ "kind": "host", "service": "files", "method": "read_text" })]);
    assert!(host.calls.lock().expect("the calls").is_empty());
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
        [json!([{ "kind": "argument", "method": "host" }, { "kind": "argument", "method": "host" }])]
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

    fn call(&self, caller: &HostCaller, method: &str, args: &Value, _: &ProgressScope) -> Result<HostAnswer, HostServiceError> {
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
                answer_with_detail(echo.call("shout", Value::Null)),
            ])
        },
    );
    assert_eq!(
        seen,
        [json!([
            "echo",
            { "a": [1] },
            { "kind": "argument", "method": "call" },
            { "kind": "host", "service": "echo", "method": "shout", "detail": "the service `echo` has no method `shout`" },
        ])]
    );
    let callers = echo.callers.lock().expect("the callers");
    assert_eq!(callers.len(), 3);
    let caller = &callers[0];
    assert_eq!((caller.extension.identity.as_str(), caller.extension.stem.as_str()), ("wc", "wc"));
    assert_eq!(caller.package, "_");
    assert_eq!(&*caller.digest_input, b"the code");
}

/// A service `mark` that declares and finishes a row `test.mark` under the
/// scope it receives.
struct Marking;

impl HostService for Marking {
    fn name(&self) -> &str {
        "mark"
    }

    fn call(&self, _: &HostCaller, _: &str, _: &Value, progress: &ProgressScope) -> Result<HostAnswer, HostServiceError> {
        progress.child(&Kind::new("test.mark"), None).start().finish();
        Ok(HostAnswer::Data(Value::Null))
    }
}

#[test]
fn a_service_receives_the_scope_of_the_handler_call() {
    let (sheet, build) = Sheet::start(&Kind::new("test.build"), None, Observer::default(), None);
    let seen = reading_under(
        build.scope(),
        |double| double.host("mark").offering(Arc::new(Marking) as Arc<dyn HostService>),
        |cx| {
            let HostHandle::Service(mark) = cx.host("mark").expect("declared") else { panic!("a generic handle") };
            answer(mark.call("now", Value::Null))
        },
    );
    assert_eq!(seen, [Value::Null]);
    build.finish();
    // The service's row is under the row of the stage whose handler called it.
    let rows = sheet.snapshot().rows;
    let mark = rows.iter().find(|row| row.kind == "test.mark").expect("the service's row");
    let parent = rows.iter().find(|row| Some(row.id) == mark.parent).expect("its parent");
    assert_eq!(parent.kind, "wc.read");
}

#[test]
fn the_dispatcher_lends_the_handles_and_refuses_a_path_that_is_not_a_text() {
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
                call(&mut table, handle.clone(), "read_text", json!(["self:data/entries.json"])),
                call(&mut table, handle, "read_text", json!([3])),
            ])
        },
    );
    assert_eq!(
        seen,
        [json!([
            "files",
            "[1, 2]",
            { "kind": "argument", "method": "read_text" },
        ])]
    );
}
