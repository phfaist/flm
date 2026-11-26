//! The extensions that a request offers, compiled into the program, as the bundled source
//! answers them from the tables of the extensions shipped with FLM (the crate
//! `flm-extensions`): two shipped extensions held as files, their manifests and their files;
//! the extensions written in Rust `math-techxt`, `profile-pdf-latex` and `profile-tty`
//! (cargo feature `native`), each its manifest given to the engine `native` and the files of
//! its folder; nothing for any other name; a compiled-in extension that has code and a
//! compose template among the files of its row, from tables that the test makes; and a
//! request with empty tables, which offers no extension.

use std::sync::Arc;

use flm_core::progress::Scope;
use flm_ext::Locator;
use flm_ext::source::{ExtensionSource, FoundExtension};
use flm_host::bundled::{Bundled, BundledSource};
use flm_host::extensions::ParsedManifests;
use flm_host::files::MemoryFiles;
use flm_host::seed::{MathChoice, read_front_matter};
use flm_host::session::{Input, PrepareErrorKind, Request, prepare};

/// The workspace's folder of extensions.
const EXTENSIONS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../extensions");

/// The tables of the extensions shipped with FLM.
fn shipped() -> Bundled {
    #[cfg(feature = "native")]
    return Bundled::new(flm_extensions::EXTENSIONS).with_code(flm_extensions::COMPILED_EXTENSIONS);
    #[cfg(not(feature = "native"))]
    Bundled::new(flm_extensions::EXTENSIONS)
}

/// What the source over the shipped extensions answers for the locator `name`, with the
/// store of its manifests.
fn find(name: &str) -> (Option<FoundExtension>, Arc<ParsedManifests>) {
    let manifests = Arc::new(ParsedManifests::default());
    let source = BundledSource::new(Arc::clone(&manifests), shipped());
    (source.find(&Locator::Named(name.to_string())).expect("no failure"), manifests)
}

#[test]
fn math_mathjax_is_answered_with_its_manifest_its_identity_and_its_files() {
    let (found, manifests) = find("math-mathjax");
    let found = found.expect("bundled");
    assert_eq!(found.manifest["extension"]["engine"], "js");
    assert_eq!(found.identity, "bundled:js/math-mathjax");
    // The parsed manifest is kept for the host's placing of problems.
    assert!(manifests.manifest("bundled:js/math-mathjax").and_then(|node| node.at(&["extension", "engine"]).cloned()).is_some());
    let files = found.files;
    let root = files.list("", "").unwrap().expect("the root");
    assert!(root.iter().any(|name| name == "vendor") && root.iter().any(|name| name == "main.js"), "{root:?}");
    assert_eq!(root.iter().filter(|name| *name == "vendor").count(), 1, "{root:?}");
    let vendor = files.list("", "vendor").unwrap().expect("the folder vendor");
    assert!(vendor.iter().any(|name| name == "mathjax.js"), "{vendor:?}");
    let main = files.read_text("", "main.js").unwrap().expect("main.js");
    assert_eq!(main, std::fs::read_to_string(format!("{EXTENSIONS}/js/math-mathjax/main.js")).unwrap());
    assert_eq!(files.read_bytes("", "main.js").unwrap().map(String::from_utf8), Some(Ok(main)));
    // A file is no folder.
    assert_eq!(files.list("", "main.js").unwrap(), None);
    // The handle kept by a compose template reads the same files.
    assert!(files.shared().read_text("", "vendor/mathjax.js").unwrap().is_some());
}

#[test]
fn lecturenotes_answers_its_template_s_description_as_a_data_file() {
    let (found, _) = find("lecturenotes");
    let found = found.expect("bundled");
    assert_eq!(found.identity, "bundled:data/lecturenotes");
    let path = "templates/html/lecturenotes/flmtemplate.yaml";
    let text = std::fs::read_to_string(format!("{EXTENSIONS}/data/lecturenotes/{path}")).unwrap();
    let expected = read_front_matter(&text, 1).expect("valid YAML").to_json();
    let value = found.files.read_value("", path).unwrap().expect("the description");
    assert_eq!(value, expected);
    assert_eq!(value["output"], "html");
}

/// Checks that the source answers the extension written in Rust `stem` for the engine
/// `native`: its identity `bundled:rust/STEM`, the manifest of its WebAssembly form with
/// its engine replaced and its module `module` removed, the parsed manifest kept, and the
/// files of its folder, without the module.
#[cfg(feature = "native")]
fn assert_answered_for_the_engine_native(stem: &str, module: &str) {
    let (found, manifests) = find(stem);
    let found = found.expect("bundled");
    let identity = format!("bundled:rust/{stem}");
    assert_eq!(found.identity, identity);
    // The manifest of the WebAssembly form, its engine replaced and its module removed.
    let written = std::fs::read_to_string(format!("{EXTENSIONS}/rust/{stem}/flmextension.yaml")).unwrap();
    let mut expected = read_front_matter(&written, 1).expect("valid YAML").to_json();
    assert_eq!(expected["extension"]["engine"], "wasm");
    assert_eq!(expected["extension"]["wasm"]["module"], module);
    expected["extension"]["engine"] = "native".into();
    expected["extension"].as_object_mut().unwrap().remove("wasm");
    assert_eq!(found.manifest, expected);
    // The parsed manifest is kept for the host's placing of problems.
    assert!(manifests.manifest(&identity).and_then(|node| node.at(&["extension", "description"]).cloned()).is_some());
    let files = found.files;
    let root = files.list("", "").unwrap().expect("the root");
    assert!(root.iter().any(|name| name == "flmextension.yaml"), "{root:?}");
    assert!(!root.iter().any(|name| name == "src" || name == "tests"), "{root:?}");
    assert_eq!(files.read_text("", "flmextension.yaml").unwrap(), Some(written));
    assert_eq!(files.read_bytes("", module).unwrap(), None);
}

#[cfg(feature = "native")]
#[test]
fn math_techxt_is_answered_for_the_engine_native_with_no_module_and_the_files_of_its_folder() {
    assert_answered_for_the_engine_native("math-techxt", "math_techxt.wasm");
}

#[cfg(feature = "native")]
#[test]
fn profile_pdf_latex_is_answered_for_the_engine_native_with_no_module_and_the_files_of_its_folder() {
    assert_answered_for_the_engine_native("profile-pdf-latex", "profile_pdf_latex.wasm");
}

#[cfg(feature = "native")]
#[test]
fn profile_tty_is_answered_for_the_engine_native_with_no_module_and_the_files_of_its_folder() {
    assert_answered_for_the_engine_native("profile-tty", "profile_tty.wasm");
}

#[test]
fn any_other_name_and_any_path_outside_the_table_answer_nothing() {
    assert!(find("nothing").0.is_none());
    assert!(find("data/lecturenotes").0.is_none());
    assert!(find("rust/profile-pdf-latex").0.is_none());
    assert!(find(".").0.is_none());
    let files = find("lecturenotes").0.expect("bundled").files;
    for path in ["nothing.yaml", "templates/nothing.yaml", "nothing", "templates/html/lecturenotes/flmtemplate"] {
        assert_eq!(files.read_text("", path).unwrap(), None, "{path}");
        assert_eq!(files.read_bytes("", path).unwrap(), None, "{path}");
        assert_eq!(files.read_value("", path).unwrap(), None, "{path}");
        assert_eq!(files.list("", path).unwrap(), None, "{path}");
    }
}

/// A request for the file `doc/paper.flm` of `files`, holding `text`, offering the
/// extensions of `bundled`.
fn request_over(text: &str, bundled: Bundled, format: &str) -> (Request, MemoryFiles) {
    let mut request = Request::new(Input::File(String::from("doc/paper.flm")));
    request.default_math = MathChoice::processor("passthrough");
    request.format = Some(String::from(format));
    request.bundled = bundled;
    (request, MemoryFiles::new().with("doc/paper.flm", text))
}

/// The extension `plain`, written in Rust and compiled into the test program: one package
/// whose compose template `plain` is a file of its row of the table of files.
#[cfg(feature = "native")]
mod plain {
    use flm_ext::guest::{CompiledExtension, Extension, NamedExtension, Package};
    use flm_ext::source::{BundledExtension, BundledFile};

    fn define(extension: &mut Extension) {
        extension.package("_", Package::new().templates(["templates/html/plain/flmtemplate.yaml"]));
    }

    static PLAIN: CompiledExtension = CompiledExtension::new(define, "plain", "0.1.0");

    /// The table of code: the extension `plain`.
    pub static CODE: [NamedExtension; 1] = [NamedExtension { stem: "plain", compiled: &PLAIN }];

    /// The table of files: the manifest of the extension's WebAssembly form, and its
    /// compose template.
    pub static FILES: [BundledExtension; 1] = [BundledExtension {
        folder: "rust",
        stem: "plain",
        files: &[
            BundledFile {
                path: "flmextension.yaml",
                bytes: b"extension:\n  description: A plain page\n  engine: wasm\n  wasm:\n    module: plain.wasm\n",
            },
            BundledFile {
                path: "templates/html/plain/flmtemplate.yaml",
                bytes: b"name: plain\noutput: html\nengine: minijinja\nmain: page.html\naccepts: all\n",
            },
            BundledFile {
                path: "templates/html/plain/page.html",
                bytes: b"<main class=\"plain\">{{ regions.content }}</main>\n",
            },
        ],
    }];
}

#[cfg(feature = "native")]
#[test]
fn a_compiled_in_extension_with_code_finds_a_compose_template_among_the_files_of_its_row() {
    use flm_host::output::{MemorySink, Sink};
    let text = "---\nflm:\n  use: [plain]\n  features:\n    compose: {template: {html: plain}}\n---\nHello.\n";
    let (mut request, files) = request_over(text, Bundled::new(&plain::FILES).with_code(&plain::CODE), "html");
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    let prepared = prepare(request, Arc::new(files), &[], &Scope::none()).expect("prepared");
    assert_eq!(prepared.manifest("bundled:rust/plain").map(|manifest| manifest.engine.as_str()), Some("native"));
    let built = flm_host::session::build(&prepared, &Scope::none()).expect("built");
    assert!(!built.has_errors());
    let received = sink.files();
    let entry = received.values().find(|file| file.entry).expect("an output");
    let page = String::from_utf8(entry.bytes.clone()).unwrap();
    assert!(page.starts_with("<main class=\"plain\">") && page.contains("Hello."), "{page}");
}

#[test]
fn a_request_with_empty_tables_finds_no_bundled_extension_and_suggests_none() {
    let shipped = shipped().stems();
    assert!(shipped.contains(&"lecturenotes"), "{shipped:?}");
    let (request, files) = request_over("---\nflm:\n  use: [lecturenotes]\n---\nHello.\n", Bundled::default(), "html");
    let refused = prepare(request, Arc::new(files), &[], &Scope::none()).expect_err("refused");
    assert!(matches!(refused.kind, PrepareErrorKind::Packages), "{refused:?}");
    let (request, files) = request_over("Hello.\n", Bundled::default(), "html");
    let prepared = prepare(request, Arc::new(files), &[], &Scope::none()).expect("prepared");
    let suggested = prepared.suggestions();
    assert!(!suggested.is_empty());
    for stem in shipped {
        assert!(!suggested.iter().any(|name| name == stem), "{stem} in {suggested:?}");
    }
}
