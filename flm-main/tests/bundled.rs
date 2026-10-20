//! The extensions compiled into the program (cargo feature `bundled-extensions`), as the
//! bundled source answers them: the two shipped extensions held as files, their manifests
//! and their files; the extension written in Rust, `profile-pdf-latex`, its manifest given
//! to the engine `native`; and nothing for any other name.

#![cfg(feature = "bundled-extensions")]

use std::sync::Arc;

use flm_ext::Locator;
use flm_ext::source::{ExtensionSource, FoundExtension};
use flm_main::bundled::BundledSource;
use flm_main::extensions::ParsedManifests;
use flm_main::seed::read_front_matter;

/// The workspace's folder of extensions.
const EXTENSIONS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../extensions");

/// What the source answers for the locator `name`, with the store of its manifests.
fn find(name: &str) -> (Option<FoundExtension>, Arc<ParsedManifests>) {
    let manifests = Arc::new(ParsedManifests::default());
    let source = BundledSource::new(Arc::clone(&manifests));
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
    let root = files.list("").unwrap().expect("the root");
    assert!(root.iter().any(|name| name == "vendor") && root.iter().any(|name| name == "main.js"), "{root:?}");
    assert_eq!(root.iter().filter(|name| *name == "vendor").count(), 1, "{root:?}");
    let vendor = files.list("vendor").unwrap().expect("the folder vendor");
    assert!(vendor.iter().any(|name| name == "mathjax.js"), "{vendor:?}");
    let main = files.read_text("main.js").unwrap().expect("main.js");
    assert_eq!(main, std::fs::read_to_string(format!("{EXTENSIONS}/js/math-mathjax/main.js")).unwrap());
    assert_eq!(files.read_bytes("main.js").unwrap().map(String::from_utf8), Some(Ok(main)));
    // A file is no folder.
    assert_eq!(files.list("main.js").unwrap(), None);
    // The handle kept by a compose template reads the same files.
    assert!(files.shared().read_text("vendor/mathjax.js").unwrap().is_some());
}

#[test]
fn lecturenotes_answers_its_template_s_description_as_a_data_file() {
    let (found, _) = find("lecturenotes");
    let found = found.expect("bundled");
    assert_eq!(found.identity, "bundled:data/lecturenotes");
    let path = "templates/html/lecturenotes/flmtemplate.yaml";
    let text = std::fs::read_to_string(format!("{EXTENSIONS}/data/lecturenotes/{path}")).unwrap();
    let expected = read_front_matter(&text, 1).expect("valid YAML").to_json();
    let value = found.files.read_value(path).unwrap().expect("the description");
    assert_eq!(value, expected);
    assert_eq!(value["output"], "html");
}

#[test]
fn profile_pdf_latex_is_answered_for_the_engine_native_with_no_module_and_no_files() {
    let (found, manifests) = find("profile-pdf-latex");
    let found = found.expect("bundled");
    assert_eq!(found.identity, "bundled:rust/profile-pdf-latex");
    // The manifest of the WebAssembly form, its engine replaced and its module removed.
    let written = std::fs::read_to_string(format!("{EXTENSIONS}/rust/profile-pdf-latex/flmextension.yaml")).unwrap();
    let mut expected = read_front_matter(&written, 1).expect("valid YAML").to_json();
    assert_eq!(expected["extension"]["engine"], "wasm");
    expected["extension"]["engine"] = "native".into();
    expected["extension"].as_object_mut().unwrap().remove("wasm");
    assert_eq!(found.manifest, expected);
    // The parsed manifest is kept for the host's placing of problems.
    assert!(manifests.manifest("bundled:rust/profile-pdf-latex").and_then(|node| node.at(&["extension", "description"]).cloned()).is_some());
    let files = found.files;
    assert_eq!(files.list("").unwrap(), None);
    assert_eq!(files.read_text("flmextension.yaml").unwrap(), None);
    assert_eq!(files.read_bytes("profile_pdf_latex.wasm").unwrap(), None);
}

#[test]
fn any_other_name_and_any_path_outside_the_table_answer_nothing() {
    assert!(find("nothing").0.is_none());
    assert!(find("data/lecturenotes").0.is_none());
    assert!(find("rust/profile-pdf-latex").0.is_none());
    assert!(find(".").0.is_none());
    let files = find("lecturenotes").0.expect("bundled").files;
    for path in ["nothing.yaml", "templates/nothing.yaml", "nothing", "templates/html/lecturenotes/flmtemplate"] {
        assert_eq!(files.read_text(path).unwrap(), None, "{path}");
        assert_eq!(files.read_bytes(path).unwrap(), None, "{path}");
        assert_eq!(files.read_value(path).unwrap(), None, "{path}");
        assert_eq!(files.list(path).unwrap(), None, "{path}");
    }
}
