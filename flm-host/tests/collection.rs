//! A collection loaded by the program and given in the request: the extensions `phfpack`,
//! `math-techxt` and `lecturenotes` built into one collection by
//! `extensions/rust/build-wasm.sh --collection`, read with `Collection::read` under the
//! label `shipped`, and offered by a request whose tables are empty. A document that uses
//! the three renders, the compose template of `lecturenotes` read from the collection, and
//! the output is byte for byte that of the same request offering the program's own tables
//! (the crate `flm-extensions`), with the same diagnostics (the definitions of `phfpack` name
//! commands that `math-techxt` does not know). Gated on `FLM_TESTS_WASM_GUEST=1` and the target
//! `wasm32-unknown-unknown` being installed; otherwise it writes one line beginning
//! `skipped:`.

#![cfg(all(feature = "wasm", feature = "native"))]

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use flm_core::progress::Scope;
use flm_ext::wasm::Collection;
use flm_host::bundled::Bundled;
use flm_host::files::MemoryFiles;
use flm_host::output::{MemorySink, Sink};
use flm_host::session::{Input, Prepared, Request, build, prepare};

#[path = "../../flm-ext/tests/common/guest.rs"]
#[allow(dead_code)]
mod guest;

/// The document: a section with a quantity macro of `phfpack` (its feature `phfqit`, whose
/// macros that `math-techxt` also defines are those of `math-techxt`) in formulas that
/// `math-techxt` renders, on the page of `lecturenotes`.
const DOCUMENT: &str = "\
---
title: Notes
flm:
  use: [lecturenotes, phfpack]
  math: techxt
  feature_order: ['phfqit < math-techxt']
---
\\section{States}
A state \\(\\ket{\\psi}\\) of the system \\(X\\) has the entropy \\(\\Hmin{X}\\).
";

/// What the request for `DOCUMENT` to `html` that offers `bundled` and `collections` gives:
/// the files of the output, each its bytes; the diagnostics of the build, each its
/// identifier and its message; and what the preparation reports. The output is written
/// although the document has errors: the definitions of `phfpack` name commands that
/// `math-techxt` does not know, which the build reports whichever place offers the
/// extensions.
fn rendered(bundled: Bundled, collections: Vec<Collection>) -> (BTreeMap<String, Vec<u8>>, Vec<String>, Prepared) {
    let mut request = Request::new(Input::File(String::from("doc/paper.flm")));
    request.format = Some(String::from("html"));
    request.continue_with_errors = true;
    request.bundled = bundled;
    request.collections = collections;
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    let files = MemoryFiles::new().with("doc/paper.flm", DOCUMENT);
    let prepared = prepare(request, Arc::new(files), &[], &Scope::none()).expect("prepared");
    let built = build(&prepared, &Scope::none()).expect("built");
    let diagnostics = built.diagnostics.iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect();
    let output = sink.files().into_iter().map(|(path, file)| (path, file.bytes)).collect();
    (output, diagnostics, prepared)
}

#[test]
fn a_document_renders_with_the_extensions_of_a_collection_as_with_the_program_s_own_tables() {
    if let Some(line) = guest::skipped() {
        println!("{line}");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let file = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-host-collection/shipped.flmextensions.wasm");
    let _ = std::fs::remove_file(&file);
    // The script's cargo run builds in a target folder of its own, apart from the folder
    // of the cargo run that runs this test.
    let script = Command::new(root.join("extensions/rust/build-wasm.sh"))
        .arg("--collection")
        .arg(&file)
        .args(["phfpack", "math-techxt", "lecturenotes"])
        .env("CARGO_TARGET_DIR", root.join("target/wasm-guest"))
        .output()
        .unwrap();
    assert!(script.status.success(), "the script must build the collection:\n{}", String::from_utf8_lossy(&script.stderr));
    let collection = Collection::read("shipped", std::fs::read(&file).unwrap()).expect("the collection reads");

    let (from_collection, diagnostics, prepared) = rendered(Bundled::default(), vec![collection]);
    let engines: Vec<(&str, Option<&str>)> = ["shipped:data/lecturenotes", "shipped:rust/math-techxt", "shipped:rust/phfpack"]
        .into_iter()
        .map(|identity| (identity, prepared.manifest(identity).map(|manifest| manifest.engine.as_str())))
        .collect();
    assert_eq!(
        engines,
        [
            ("shipped:data/lecturenotes", Some("data")),
            ("shipped:rust/math-techxt", Some("wasm")),
            ("shipped:rust/phfpack", Some("wasm"))
        ]
    );
    // The page of the compose template of `lecturenotes` (its file `lecturenotes.html`),
    // which only the collection holds.
    let page = String::from_utf8_lossy(&from_collection["index.html"]);
    assert!(page.contains("<div class=\"ln-app\">") && page.contains("States"), "{page}");

    let shipped = Bundled::new(flm_extensions::EXTENSIONS).with_code(flm_extensions::COMPILED_EXTENSIONS);
    let (from_tables, diagnostics_of_tables, _) = rendered(shipped, Vec::new());
    assert_eq!(diagnostics, diagnostics_of_tables);
    assert_eq!(from_collection.keys().collect::<Vec<_>>(), from_tables.keys().collect::<Vec<_>>());
    for (path, bytes) in &from_collection {
        assert!(*bytes == from_tables[path], "the file `{path}` differs from that of the program's own tables");
    }
}
