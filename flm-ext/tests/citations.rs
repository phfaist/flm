//! A citation provider contributed by a scripted feature (E6, plan sections
//! 2.4, 3.1 and 3.2), through the Rust test double: the adapter wraps the
//! mirror's provider in `flm-core`'s, the retrieve stage asks it in the order
//! of the slot, its context parses and builds entries and lends the host
//! services; a failing provider, a wrong count and an answer of the wrong
//! shape are reported per key. One provider speaks JSON, as a binding does,
//! through the dispatcher and the conversions of `values.rs`.

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use flm_ext::adapter::dispatch;
use flm_ext::adapter::handles::{CallTable, Entry};
use flm_ext::adapter::values::{citation_requests_to_value, citation_results_from_value};
use flm_ext::api::core::registry::Contributed;
use flm_ext::api::host::{FilesScope, HostHandle};
use flm_ext::host::HostService;
use flm_ext::api::stdcore::features::citations::{
    CitationAnswer, CitationProvider, CitationRequest, CitationResult, EntryPiece, ProviderCx, PROVIDERS_SLOT,
};
use flm_ext::{ExtError, MemoryFiles};
use flm_core::stdcore::backends::text;
use serde_json::{json, Value};

use common::double::Double;
use common::{diagnostics, registry_with, run, try_registry_with, MemoryHostFiles};

/// A provider over the entries of `refs.json` in the document's folder, read
/// through the `files` service on every call: `{ key: [source, identity] }`
/// (the identity `null` for none). It answers "not mine" for any other key,
/// and records the keys of each call.
struct FromFile {
    name: String,
    calls: Arc<Mutex<Vec<Vec<String>>>>,
}

impl CitationProvider for FromFile {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn answer(&self, _: &CitationRequest, _: &dyn ProviderCx) -> Result<CitationResult, ExtError> {
        unreachable!("the retrieve stage calls `answer_all`")
    }

    fn answer_all(&self, requests: &[CitationRequest], cx: &dyn ProviderCx) -> Result<Vec<CitationResult>, ExtError> {
        self.calls.lock().expect("calls").push(requests.iter().map(|request| request.key.clone()).collect());
        let HostHandle::Files(files) = cx.host("files")? else { panic!("`files` is the files handle") };
        let text = files.read_text(FilesScope::Document, "refs.json")?.unwrap_or_else(|| "{}".to_string());
        let entries: BTreeMap<String, (String, Option<String>)> = serde_json::from_str(&text).expect("the entries");
        requests
            .iter()
            .map(|request| {
                let Some((source, identity)) = entries.get(&request.key) else { return Ok(Ok(None)) };
                let entry = cx.parse_entry(source, "refs.json")?;
                let answer = CitationAnswer::new(entry);
                Ok(Ok(Some(match identity {
                    Some(identity) => answer.with_identity(identity.as_str()),
                    None => answer,
                })))
            })
            .collect()
    }
}

/// A provider that answers every key with an entry built from parts.
struct Built;

impl CitationProvider for Built {
    fn name(&self) -> String {
        "more.built".to_string()
    }

    fn answer(&self, request: &CitationRequest, cx: &dyn ProviderCx) -> Result<CitationResult, ExtError> {
        let pieces = [
            EntryPiece::Text(format!("{} ", request.key)),
            EntryPiece::Emphasized(vec![EntryPiece::Source("\\textbf{built}".to_string())]),
        ];
        Ok(Ok(Some(CitationAnswer::new(cx.build_entry(&pieces)?).with_warning("built from parts"))))
    }
}

/// The document's folder of the tests: three works, two keys of one.
fn the_document_s_folder() -> Arc<MemoryHostFiles> {
    let refs = json!({
        "knuth84": ["Knuth, \\emph{The TeXbook}", "knuth84"],
        "Knuth84": ["Knuth, \\emph{The TeXbook}", "knuth84"],
        "lamport94": ["Lamport, LaTeX", null],
        "shor95": ["Shor, 1995", null],
    });
    Arc::new(MemoryHostFiles::new(MemoryFiles::new().with_text("refs.json", &refs.to_string())))
}

/// The package `bib` contributing `provider` with `contribute_to`, declaring
/// and offered the `files` service.
fn bib(provider: Arc<dyn CitationProvider>, host: &Arc<MemoryHostFiles>) -> Double {
    Double::new("bib")
        .host("files")
        .offering(Arc::clone(host) as Arc<dyn HostService>)
        .feature("bib", move |b| b.contribute_to(PROVIDERS_SLOT, Contributed::CitationProvider(Arc::clone(&provider))))
}

/// The package `more`, contributing `provider` with `peer_slot`.
fn more(provider: Arc<dyn CitationProvider>) -> Double {
    Double::new("more").feature("more", move |b| b.peer_slot(PROVIDERS_SLOT, Contributed::CitationProvider(Arc::clone(&provider))))
}

/// The text output of `document` and the run's diagnostics.
fn text_and_diagnostics(registry: &Arc<flm_core::core::registry::FlmRegistry>, document: &str) -> (String, Vec<String>) {
    let run = run(registry, text::WORKFLOW, document);
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    (out, diagnostics(&run))
}

#[test]
fn a_provider_answers_from_a_file_and_leaves_the_rest_to_the_next() {
    let calls = Arc::new(Mutex::new(Vec::new()));
    let host = the_document_s_folder();
    let provider = Arc::new(FromFile { name: "bib.file".to_string(), calls: Arc::clone(&calls) });
    // `bib` is used last, so its feature is placed last and its provider asked first.
    let registry = registry_with([more(Arc::new(Built)).package(), bib(provider, &host).package()]);
    let (out, diagnostics) = text_and_diagnostics(&registry, "A \\cite{knuth84}. B \\cite{lamport94,shor95}. C \\cite{unknown}.");
    assert_eq!(
        out,
        "A\u{a0}[1]. B\u{a0}[2,3]. C\u{a0}[4].\n\nReferences\n----------\n\n[1] Knuth, The TeXbook\n\n[2] Lamport, LaTeX\n\n\
         [3] Shor, 1995\n\n[4] unknown built\n"
    );
    // The first provider was asked about every key in one call; the second
    // about the one the first left, which it answers with a warning.
    assert_eq!(*calls.lock().expect("calls"), [vec!["knuth84", "lamport94", "shor95", "unknown"]]);
    assert_eq!(diagnostics, ["flm.citations.provider-warning: built from parts"]);
    // The provider read through the host's `files` service, as its package.
    let seen = host.calls.lock().expect("calls");
    assert_eq!(seen.len(), 1);
    assert_eq!((seen[0].0.package.as_str(), seen[0].0.extension.stem.as_str(), seen[0].1.as_str()), ("_", "bib", "read_text"));
}

#[test]
fn two_spellings_with_one_identity_are_one_work() {
    let host = the_document_s_folder();
    let provider = Arc::new(FromFile { name: "bib.file".to_string(), calls: Arc::default() });
    let registry = registry_with([bib(provider, &host).package()]);
    let (out, diagnostics) = text_and_diagnostics(&registry, "A \\cite{Knuth84}. B \\cite{knuth84}. C \\cite{shor95}.");
    assert_eq!(out, "A\u{a0}[1]. B\u{a0}[1]. C\u{a0}[2].\n\nReferences\n----------\n\n[1] Knuth, The TeXbook\n\n[2] Shor, 1995\n");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

/// A provider whose call fails with `error`, or answers `answers` results of
/// no entry.
struct Failing {
    error: Option<ExtError>,
    answers: usize,
}

impl CitationProvider for Failing {
    fn name(&self) -> String {
        "bib.failing".to_string()
    }

    fn answer(&self, _: &CitationRequest, _: &dyn ProviderCx) -> Result<CitationResult, ExtError> {
        unreachable!("the retrieve stage calls `answer_all`")
    }

    fn answer_all(&self, _: &[CitationRequest], _: &dyn ProviderCx) -> Result<Vec<CitationResult>, ExtError> {
        match &self.error {
            Some(error) => Err(error.clone()),
            None => Ok((0..self.answers).map(|_| Ok(None)).collect()),
        }
    }
}

/// The diagnostics of the provider-failed condition of a run of `provider`
/// over two keys, with `Built` asked after it (`bib` is used last, so its
/// feature is placed last and its provider asked first).
fn failures(provider: Arc<dyn CitationProvider>) -> Vec<String> {
    let host = the_document_s_folder();
    let registry = registry_with([more(Arc::new(Built)).package(), bib(provider, &host).package()]);
    let (_, diagnostics) = text_and_diagnostics(&registry, "A \\cite{knuth84}. B \\cite{shor95}.");
    diagnostics.into_iter().filter(|line| line.starts_with("flm.citations.provider-failed")).collect()
}

#[test]
fn a_provider_that_fails_fails_every_key_of_its_call() {
    let error = ExtError::Handler { detail: "TypeError: entries is undefined".to_string(), data: Value::Null };
    let failing = Arc::new(Failing { error: Some(error), answers: 0 });
    let detail = "flm.ext.handler-failed: the citation provider `bib.failing` of the feature `bib` of the package `_` of the \
                  extension `bib` failed: TypeError: entries is undefined";
    assert_eq!(
        failures(failing),
        [
            format!("flm.citations.provider-failed: the bibliography entry of the key `knuth84` could not be retrieved: {detail}"),
            format!("flm.citations.provider-failed: the bibliography entry of the key `shor95` could not be retrieved: {detail}"),
        ]
    );
}

#[test]
fn a_provider_answering_a_wrong_count_gets_flm_core_s_text() {
    let detail = "a source of bibliography entries must return one result for each key it is given, and it returned 1 \
                  result for 2 keys";
    assert_eq!(
        failures(Arc::new(Failing { error: None, answers: 1 })),
        [
            format!("flm.citations.provider-failed: the bibliography entry of the key `knuth84` could not be retrieved: {detail}"),
            format!("flm.citations.provider-failed: the bibliography entry of the key `shor95` could not be retrieved: {detail}"),
        ]
    );
}

/// A provider that speaks JSON, as a binding does: its context and the
/// requests are lent into a call table, the entries are made through the
/// dispatcher, and the answer `answer(entries)` is read back with the
/// conversions of `values.rs`.
struct Speaking {
    answer: fn(Vec<Value>) -> Value,
}

impl CitationProvider for Speaking {
    fn name(&self) -> String {
        "bib.speaking".to_string()
    }

    fn answer(&self, _: &CitationRequest, _: &dyn ProviderCx) -> Result<CitationResult, ExtError> {
        unreachable!("the retrieve stage calls `answer_all`")
    }

    fn answer_all(&self, requests: &[CitationRequest], cx: &dyn ProviderCx) -> Result<Vec<CitationResult>, ExtError> {
        let mut table = CallTable::new(1);
        let cx = flm_ext::adapter::values::handle_to_value(table.lend(Entry::ProviderCx(cx)));
        let cx = flm_ext::adapter::values::handle_from_value(&cx)?;
        let keys = citation_requests_to_value(requests);
        let mut entries = Vec::new();
        for request in keys.as_array().expect("a list") {
            let key = request["key"].as_str().expect("a key");
            let entry = if key == "shor95" {
                dispatch::call(&mut table, cx, "build_entry", &json!([[{ "strong": [{ "text": "Shor" }] }, { "text": ", 1995" }]]))?
            } else {
                dispatch::call(&mut table, cx, "parse_entry", &json!([format!("\\emph{{{key}}}"), "refs.json"]))?
            };
            entries.push(entry);
        }
        citation_results_from_value(&mut table, &(self.answer)(entries))
    }
}

#[test]
fn a_provider_speaking_json_answers_through_the_conversions() {
    let speaking = Speaking {
        answer: |entries| {
            json!([
                { "entry": entries[0], "identity": "k", "warnings": ["a warning"] },
                { "entry": entries[1] },
                null,
                { "error": "the server did not answer" },
            ])
        },
    };
    let host = the_document_s_folder();
    let registry = registry_with([more(Arc::new(Built)).package(), bib(Arc::new(speaking), &host).package()]);
    let (out, diagnostics) = text_and_diagnostics(&registry, "A \\cite{knuth84,shor95,lamport94,x}.");
    // The key whose answer is an error keeps its number, its entry dropped.
    assert_eq!(
        out,
        "A\u{a0}[1–4].\n\nReferences\n----------\n\n[1] knuth84\n\n[2] Shor, 1995\n\n[3] lamport94 built\n\n\
         [4] [dropped: bibliography entry]\n"
    );
    assert_eq!(
        diagnostics,
        [
            "flm.citations.provider-warning: a warning",
            "flm.citations.provider-warning: built from parts",
            "flm.citations.provider-failed: the bibliography entry of the key `x` could not be retrieved: the server did \
             not answer",
            "flm.citations.no-entry: the cited key `x` must have a bibliography entry; retrieving it failed: the server \
             did not answer",
        ]
    );
}

#[test]
fn an_answer_of_the_wrong_shape_fails_every_key_of_the_call() {
    let speaking = Speaking { answer: |entries| json!([{ "entry": entries[0], "identity": 3 }]) };
    let host = the_document_s_folder();
    let registry = registry_with([bib(Arc::new(speaking), &host).package()]);
    let (_, diagnostics) = text_and_diagnostics(&registry, "A \\cite{knuth84}.");
    assert_eq!(
        diagnostics.first().map(String::as_str),
        Some(
            "flm.citations.provider-failed: the bibliography entry of the key `knuth84` could not be retrieved: \
             flm.ext.handler-failed: the citation provider `bib.speaking` of the feature `bib` of the package `_` of the \
             extension `bib` failed: the answer must be a list with one item per request, each `null`, `{ error }` or \
             `{ entry, identity, warnings }`"
        )
    );
}

/// The registry build's error when the package `bib` contributes `value`
/// to `slot`.
fn refusal(slot: &'static str, value: impl Fn() -> Contributed + Send + Sync + 'static) -> String {
    let double = Double::new("bib").feature("bib", move |b| b.contribute_to(slot, value()));
    try_registry_with([double.package()]).expect_err("the build fails").render()
}

#[test]
fn a_provider_is_named_under_the_stem_and_contributed_to_its_slot_alone() {
    struct Named(&'static str);
    impl CitationProvider for Named {
        fn name(&self) -> String {
            self.0.to_string()
        }
        fn answer(&self, _: &CitationRequest, _: &dyn ProviderCx) -> Result<CitationResult, ExtError> {
            Ok(Ok(None))
        }
    }
    assert_eq!(
        refusal(PROVIDERS_SLOT, || Contributed::CitationProvider(Arc::new(Named("file")))),
        "feature `bib`: flm.ext.name-outside-extension: the citation provider `file` declared by the package `_` of the \
         extension `bib` must begin with `bib.`"
    );
    assert_eq!(
        refusal(PROVIDERS_SLOT, || Contributed::CitationProvider(Arc::new(Named("")))),
        "feature `bib`: flm.ext.handler-failed: `contribute_to`: a citation provider contributed to \
         `flm.citations.providers` by the package `_` of the extension `bib` must be an object with `name` and \
         `answer` or `answer_all`"
    );
    assert_eq!(
        refusal("flm.compose.templates", || Contributed::CitationProvider(Arc::new(Named("bib.file")))),
        "feature `bib`: flm.ext.handler-failed: `contribute_to`: the service slot `flm.compose.templates` takes a \
         compose template module (an object with `name`, `describe` and `render`), and the package `_` of the \
         extension `bib` contributed a citation provider"
    );
}

#[test]
fn a_provider_s_context_lends_the_declared_services_alone() {
    struct Asking(Arc<Mutex<Option<String>>>);
    impl CitationProvider for Asking {
        fn name(&self) -> String {
            "bib.asking".to_string()
        }
        fn answer(&self, _: &CitationRequest, cx: &dyn ProviderCx) -> Result<CitationResult, ExtError> {
            let error = cx.host("files").err().map(|error| error.to_string());
            *self.0.lock().expect("seen") = error;
            Ok(Ok(None))
        }
    }
    let seen = Arc::new(Mutex::new(None));
    let asking = Arc::new(Asking(Arc::clone(&seen)));
    let double = Double::new("bib")
        .offering(the_document_s_folder() as Arc<dyn HostService>)
        .feature("bib", move |b| b.contribute_to(PROVIDERS_SLOT, Contributed::CitationProvider(asking.clone())));
    let registry = registry_with([double.package()]);
    let _ = text_and_diagnostics(&registry, "A \\cite{a}.");
    assert_eq!(
        seen.lock().expect("seen").as_deref(),
        Some(
            "`host`: the host service `files` must be declared in the `host` list of the package `_` of the extension \
             `bib` before a handler calls it"
        )
    );
}
