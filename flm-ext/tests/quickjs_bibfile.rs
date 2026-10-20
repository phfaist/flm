//! The fixture `tests/js/bibfile/` (E6, plan section 7.1): a citation
//! provider written in JavaScript reading the entries of the cited keys from
//! `bib.json` beside the document through the host service `files`, a
//! service slot `bibfile.notes` the package `_` declares and the package
//! `extra` fills, and the workflow `noted`. Loaded through the js engine with
//! a `files` service over memory ([`MemoryHostFiles`]); the document of the
//! plan rendered through the three standard outputs with goldens; the
//! identity merging two spellings of one key; a missing `bib.json`; the note
//! of `bibfile#extra`. Under `--features flm-core/parallel`, the runs of the
//! goldens execute their stages on the pool, so the provider answers on a
//! pool thread.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{FlmRegistry, PackageEntry};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::meta::{DocMeta, MetaText};
use flm_ext::host::HostService;
use flm_ext::quickjs::{JsHost, JsSettings};
use flm_ext::{ExtensionLoader, MemoryFiles};
use serde_json::{json, Value};

use common::{ext_items, factory_of, registry_with, MemoryHostFiles};

// --- helpers -----------------------------------------------------------------------------------

/// The fixture's files, as `tests/js/bibfile/` holds them (its manifest is
/// [`manifest`], the JSON equal to `flmextension.yaml`).
const BIBFILE_FILES: [(&str, &str); 1] = [("main.js", include_str!("js/bibfile/main.js"))];

/// The fixture's manifest, `tests/js/bibfile/flmextension.yaml` as JSON.
fn manifest() -> Value {
    json!({ "extension": {
        "description": "Bibliography entries from a JSON file beside the document",
        "engine": "js",
        "js": { "main": "main.js" },
    } })
}

/// The document of plan section 7.1, its body (the title `Three works` is
/// set as the document's metadata, [`run_doc`]).
const DOCUMENT: &str = "See \\cite{knuth84} and \\cite{lamport94}; the code of \\cite{Shor95} (also \\cite{shor95}).\n";

/// The file `bib.json` beside the document, as plan section 7.1 writes it.
const BIB_JSON: &str = r#"{
  "knuth84": { "author": "D. E. Knuth", "title": "The TeXbook", "year": 1984 },
  "lamport94": { "author": "L. Lamport", "title": "LaTeX: A Document Preparation System", "year": 1994 },
  "shor95": { "author": "P. W. Shor", "title": "Scheme for reducing decoherence in quantum computer memory", "year": 1995 }
}
"#;

/// The golden file `name` under `tests/goldens/`.
fn golden(name: &str) -> String {
    format!("{}/tests/goldens/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The document's folder: `bib.json` when `with_bib`, else nothing.
fn folder(with_bib: bool) -> Arc<MemoryHostFiles> {
    let files = if with_bib { MemoryFiles::new().with_text("bib.json", BIB_JSON) } else { MemoryFiles::new() };
    Arc::new(MemoryHostFiles::new(files))
}

/// The loader of the fixture, found as `bibfile`, offering `service`.
fn loader(service: &Arc<MemoryHostFiles>) -> Arc<ExtensionLoader> {
    let files = BIBFILE_FILES.iter().fold(MemoryFiles::new(), |held, (path, text)| held.with_text(path, text));
    let source = flm_ext::MemorySource::new().with("bibfile", "bibfile", manifest(), files);
    let loader = ExtensionLoader::new(vec![Arc::new(source)])
        .with_js(JsHost::new(JsSettings::default()))
        .with_host_service(Arc::clone(service) as Arc<dyn HostService>);
    Arc::new(loader)
}

/// The package `entry` of `loader`, built from empty settings.
fn built(loader: &Arc<ExtensionLoader>, entry: &str) -> PackageEntry {
    factory_of(loader, entry).build(&PackageSettings::new()).expect("the package builds")
}

/// The registry of the standard package and the packages `entries` of the
/// fixture, its document's folder `service`.
fn registry(service: &Arc<MemoryHostFiles>, entries: &[&str]) -> Arc<FlmRegistry> {
    let loader = loader(service);
    registry_with(entries.iter().map(|entry| built(&loader, entry)).collect::<Vec<_>>())
}

/// `f` run on a thread marked as a worker of `flm-core`.
fn on_worker<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    flm_core::lang::guard::run_with_worker_stack(f).expect("the worker thread starts")
}

/// The run of the workflow `workflow` over [`DOCUMENT`] titled `Three works`,
/// with the default run options (on the pool under `flm-core/parallel`).
fn run_doc(registry: &Arc<FlmRegistry>, workflow: &str) -> Run {
    run_doc_with(registry, workflow, &Overrides::none())
}

/// [`run_doc`] with `overrides`.
fn run_doc_with(registry: &Arc<FlmRegistry>, workflow: &str, overrides: &Overrides) -> Run {
    let source = Arc::new(FlmSource::new(DOCUMENT).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let meta = DocMeta::default().title(MetaText::new("Three works"));
    let plan = Arc::new(Plan::for_workflow(registry, workflow, overrides).expect("a plan"));
    let doc = Doc::with_options(content, OptionValues::new().with(meta));
    let mut run = Run::new(Arc::new(doc), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The text output of the document, with the run's diagnostics.
fn text_of(registry: &Arc<FlmRegistry>) -> (String, Vec<String>) {
    let run = run_doc(registry, text::WORKFLOW);
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    (out, common::diagnostics(&run))
}

/// The HTML content of the document: the inside of the page's `main`
/// element (the title, the content and the bibliography, without the head
/// and its stylesheet), with the run's diagnostics.
fn html_of(registry: &Arc<FlmRegistry>) -> (String, Vec<String>) {
    let run = run_doc(registry, html::WORKFLOW);
    let page = run
        .table(html::DOCUMENT)
        .expect("declared")
        .and_then(|set| set.entry().and_then(|entry| entry.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned())))
        .unwrap_or_default();
    let start = page.find("<main").and_then(|at| page[at..].find('>').map(|end| at + end + 1)).expect("a main element");
    let end = page.find("</main>").expect("the main element ends");
    (page[start..end].trim().to_string() + "\n", common::diagnostics(&run))
}

/// The recomposed LaTeX of the document, with the run's diagnostics.
fn latex_of(registry: &Arc<FlmRegistry>) -> (String, Vec<String>) {
    let run = run_doc(registry, latex_recomposed::WORKFLOW);
    let out = run
        .table(latex_recomposed::DOCUMENT)
        .expect("declared")
        .and_then(|set| set.entry().and_then(|entry| entry.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned())))
        .unwrap_or_default();
    (out, common::diagnostics(&run))
}

// --- the document of section 7.1 ---------------------------------------------------------------

#[test]
fn the_bibliography_of_three_works_renders_as_text() {
    let service = folder(true);
    let registry = registry(&service, &["bibfile"]);
    let (out, diagnostics) = on_worker(|| text_of(&registry));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_text_golden(golden("bibfile.txt"), &out);
    // Three works: `Shor95` and `shor95` are one, by the identity.
    assert_eq!(out.matches("Knuth").count(), 1, "{out}");
    assert_eq!(out.matches("Shor,").count(), 1, "{out}");
    // The provider read `bib.json` through the host, as the package `_`.
    let calls = service.calls.lock().expect("the calls");
    assert!(!calls.is_empty());
    for (caller, method, args) in calls.iter() {
        assert_eq!((caller.extension.stem.as_str(), caller.package.as_str(), method.as_str()), ("bibfile", "_", "read_text"));
        assert_eq!(args, &json!({ "scope": "document", "path": "bib.json" }));
    }
}

#[test]
fn the_bibliography_of_three_works_renders_as_html() {
    let registry = registry(&folder(true), &["bibfile"]);
    let (out, diagnostics) = on_worker(|| html_of(&registry));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_text_golden(golden("bibfile.html"), &out);
}

#[test]
fn the_bibliography_of_three_works_renders_as_recomposed_latex() {
    let registry = registry(&folder(true), &["bibfile"]);
    let (out, diagnostics) = on_worker(|| latex_of(&registry));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_text_golden(golden("bibfile.tex"), &out);
}

// --- a missing `bib.json` ----------------------------------------------------------------------

#[test]
fn a_missing_entries_file_fails_each_cited_key_and_the_check_reports_no_entry() {
    let registry = registry(&folder(false), &["bibfile"]);
    let (_, diagnostics) = on_worker(|| text_of(&registry));
    // The provider answers `{ error }` for each key: the retrieve stage
    // reports `ProviderFailed` with the provider's text at each key.
    let failed = |key: &str| {
        format!(
            "flm.citations.provider-failed: the bibliography entry of the key `{key}` could not be retrieved: \
             the file `bib.json` beside the document must hold the bibliography entries"
        )
    };
    for key in ["knuth84", "lamport94", "Shor95", "shor95"] {
        assert_eq!(diagnostics.iter().filter(|d| **d == failed(key)).count(), 1, "{key}: {diagnostics:#?}");
        assert!(
            diagnostics.iter().any(|d| d.starts_with("flm.citations.no-entry:") && d.contains(&format!("`{key}`"))),
            "{key}: {diagnostics:#?}"
        );
    }
}

#[test]
fn entries_that_are_not_json_fail_the_provider_at_each_cited_key() {
    // `JSON.parse` throws: the handler fails, and each key is reported with
    // the handler's failure (D13).
    let service = Arc::new(MemoryHostFiles::new(MemoryFiles::new().with_text("bib.json", "{ not json")));
    let registry = registry(&service, &["bibfile"]);
    let (_, diagnostics) = on_worker(|| text_of(&registry));
    let failed = |key: &str| {
        format!(
            "flm.citations.provider-failed: the bibliography entry of the key `{key}` could not be retrieved: \
             flm.ext.handler-failed: the citation provider `bibfile.json` of the feature `bibfile` of the package `_` \
             of the extension `bibfile` failed: Expected property name or '}}' in JSON at position 2 (line 1 column 3)"
        )
    };
    for key in ["knuth84", "lamport94", "Shor95", "shor95"] {
        assert_eq!(diagnostics.iter().filter(|d| **d == failed(key)).count(), 1, "{key}: {diagnostics:#?}");
        assert!(diagnostics.iter().any(|d| d.starts_with("flm.citations.no-entry:") && d.contains(&format!("`{key}`"))));
    }
}

// --- the package `extra` and the slot `bibfile.notes` ------------------------------------------

/// The note `bibfile#extra` contributes, as the stage reports it.
const NOTE: &str = "bibfile.note: a package notes: the extra package was used";

#[test]
fn the_note_of_the_package_extra_is_reported_in_a_run_demanding_the_notes() {
    let registry = registry(&folder(true), &["bibfile", "bibfile#extra"]);
    let run = on_worker(|| common::run_demanding(&registry, &["bibfile.notes"], &[DOCUMENT]));
    assert_eq!(common::diagnostics(&run), [NOTE]);
    assert_eq!(ext_items(&run, "bibfile.notes"), [json!({ "note": "the extra package was used" })]);
}

#[test]
fn the_workflow_noted_is_the_package_s_and_reports_the_note_beside_the_text() {
    // The workflow `noted` is in the package's block, which the host reads
    // (a registry has no workflow of a package's block): its fields as the
    // fixture writes them.
    let service = folder(true);
    let loader = loader(&service);
    let packages = [built(&loader, "bibfile"), built(&loader, "bibfile#extra")];
    let blocks = loader.blocks();
    let block = blocks.iter().find(|block| block.package.as_str() == "bibfile#_").expect("the block of `_`");
    let noted = block.block.workflows.iter().find(|workflow| workflow.name == "noted").expect("the workflow `noted`");
    assert_eq!(
        noted.fields,
        [
            ("extends".to_string(), json!("text")),
            ("demands".to_string(), json!(["flm.validated", "bibfile.notes"])),
            ("docs".to_string(), json!("The text output, with the notes reported.")),
        ]
    );
    // What a host makes of it: the text workflow demanding the notes too.
    let registry = registry_with(packages);
    let overrides = Overrides { demands: vec!["flm.validated".into(), "bibfile.notes".into()], ..Overrides::none() };
    let run = on_worker(|| run_doc_with(&registry, text::WORKFLOW, &overrides));
    assert_eq!(common::diagnostics(&run), [NOTE]);
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    assert_text_golden(golden("bibfile.txt"), &out);
}

#[test]
fn without_the_package_extra_the_notes_are_none() {
    let registry = registry(&folder(true), &["bibfile"]);
    let run = on_worker(|| common::run_demanding(&registry, &["bibfile.notes"], &[DOCUMENT]));
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert!(ext_items(&run, "bibfile.notes").is_empty());
}

