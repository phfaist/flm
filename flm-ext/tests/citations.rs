//! A citation provider contributed by a scripted feature, through the Rust
//! test double: the adapter wraps the
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
use flm_ext::api::host::HostHandle;
use flm_ext::host::HostService;
use flm_ext::api::stdlib::features::citations::{
    CitationAnswer, CitationProvider, CitationRequest, CitationResult, EntryPiece, ProviderCx, PROVIDERS_SLOT,
};
use flm_ext::{ExtError, MemoryFiles};
use flm_core::stdlib::backends::text;
use flm_core::stdlib::features::citations::{CitationProviderError, MissingEntry, NoEntry, ProviderFailed, ProviderWarning};
use serde_json::{json, Value};

use common::double::Double;
use common::{diagnostics, feature_errors, registry_with, run, try_registry_with, MemoryHostFiles};

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
        let text = files.read_text("doc:refs.json")?.unwrap_or_else(|| "{}".to_string());
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

/// The provider-failed conditions of a run of `provider` over two keys, as
/// (key, detail), with `Built` asked after it (`bib` is used last, so its
/// feature is placed last and its provider asked first).
fn failures(provider: Arc<dyn CitationProvider>) -> Vec<(String, String)> {
    let host = the_document_s_folder();
    let registry = registry_with([more(Arc::new(Built)).package(), bib(provider, &host).package()]);
    let run = run(&registry, text::WORKFLOW, "A \\cite{knuth84}. B \\cite{shor95}.");
    run.diagnostics()
        .iter()
        .filter_map(|d| d.data().downcast_ref::<ProviderFailed>())
        .map(|failed| (failed.key.clone(), failed.detail.clone()))
        .collect()
}

#[test]
fn a_provider_that_fails_fails_every_key_of_its_call() {
    let error = ExtError::Handler { detail: "TypeError: entries is undefined".to_string(), data: Value::Null };
    let failing = Arc::new(Failing { error: Some(error), answers: 0 });
    let found = failures(failing);
    assert_eq!(found.iter().map(|(key, _)| key.as_str()).collect::<Vec<_>>(), ["knuth84", "shor95"]);
    for (_, detail) in &found {
        assert!(detail.starts_with("flm.ext.handler-failed: "), "{detail}");
        assert!(detail.ends_with("TypeError: entries is undefined"), "{detail}");
    }
}

#[test]
fn a_provider_answering_a_wrong_count_fails_every_key_of_its_call() {
    let found = failures(Arc::new(Failing { error: None, answers: 1 }));
    assert_eq!(found.iter().map(|(key, _)| key.as_str()).collect::<Vec<_>>(), ["knuth84", "shor95"]);
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
    let run = run(&registry, text::WORKFLOW, "A \\cite{knuth84,shor95,lamport94,x}.");
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    // The key whose answer is an error keeps its number, its entry dropped.
    assert_eq!(
        out,
        "A\u{a0}[1–4].\n\nReferences\n----------\n\n[1] knuth84\n\n[2] Shor, 1995\n\n[3] lamport94 built\n\n\
         [4] [dropped: bibliography entry]\n"
    );
    assert_eq!(
        common::identifiers(&run),
        [
            "flm.citations.provider-warning",
            "flm.citations.provider-warning",
            "flm.citations.provider-failed",
            "flm.citations.no-entry",
        ]
    );
    let warnings: Vec<&str> =
        run.diagnostics().iter().filter_map(|d| d.data().downcast_ref::<ProviderWarning>()).map(|w| w.detail.as_str()).collect();
    assert_eq!(warnings, ["a warning", "built from parts"]);
    let failed: Vec<&ProviderFailed> = run.diagnostics().iter().filter_map(|d| d.data().downcast_ref::<ProviderFailed>()).collect();
    assert_eq!(failed.iter().map(|f| (f.key.as_str(), f.detail.as_str())).collect::<Vec<_>>(), [("x", "the server did not answer")]);
    let missing: Vec<&NoEntry> = run.diagnostics().iter().filter_map(|d| d.data().downcast_ref::<NoEntry>()).collect();
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].key, "x");
    assert_eq!(missing[0].reason, MissingEntry::Failed(CitationProviderError::new("the server did not answer")).reason());
}

#[test]
fn an_answer_of_the_wrong_shape_fails_every_key_of_the_call() {
    let speaking = Speaking { answer: |entries| json!([{ "entry": entries[0], "identity": 3 }]) };
    let host = the_document_s_folder();
    let registry = registry_with([bib(Arc::new(speaking), &host).package()]);
    let run = run(&registry, text::WORKFLOW, "A \\cite{knuth84}.");
    let first = run.diagnostics().iter().next().and_then(|d| d.data().downcast_ref::<ProviderFailed>()).cloned();
    let first = first.expect("the first diagnostic is a provider failure");
    assert_eq!(first.key, "knuth84");
    assert!(first.detail.starts_with("flm.ext.handler-failed: "), "{}", first.detail);
}

/// The registry build's errors, as (feature, detail), when the package `bib`
/// contributes `value` to `slot`.
fn refusal(slot: &'static str, value: impl Fn() -> Contributed + Send + Sync + 'static) -> Vec<(String, String)> {
    let double = Double::new("bib").feature("bib", move |b| b.contribute_to(slot, value()));
    feature_errors(&try_registry_with([double.package()]).expect_err("the build fails"))
}

/// Whether `found` is one error of the feature `bib` whose detail begins with
/// the identifier `identifier`.
fn is_refused_with(found: &[(String, String)], identifier: &str) -> bool {
    matches!(found, [(feature, detail)] if feature == "bib" && detail.starts_with(&format!("{identifier}: ")))
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
    let found = refusal(PROVIDERS_SLOT, || Contributed::CitationProvider(Arc::new(Named("file"))));
    assert!(is_refused_with(&found, "flm.ext.name-outside-extension"), "{found:?}");
    let found = refusal(PROVIDERS_SLOT, || Contributed::CitationProvider(Arc::new(Named(""))));
    assert!(is_refused_with(&found, "flm.ext.handler-failed"), "{found:?}");
    let found = refusal("flm.compose.templates", || Contributed::CitationProvider(Arc::new(Named("bib.file"))));
    assert!(is_refused_with(&found, "flm.ext.handler-failed"), "{found:?}");
}

#[test]
fn a_provider_s_context_lends_the_declared_services_alone() {
    struct Asking(Arc<Mutex<Option<ExtError>>>);
    impl CitationProvider for Asking {
        fn name(&self) -> String {
            "bib.asking".to_string()
        }
        fn answer(&self, _: &CitationRequest, cx: &dyn ProviderCx) -> Result<CitationResult, ExtError> {
            let error = cx.host("files").err();
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
    let seen = seen.lock().expect("seen");
    assert!(matches!(seen.as_ref(), Some(ExtError::Argument { method, .. }) if method == "host"), "{seen:?}");
}
