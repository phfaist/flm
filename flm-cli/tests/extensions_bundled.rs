//! The extensions shipped with FLM, compiled into the binary (cargo feature
//! `bundled-extensions`): a document that uses `lecturenotes` or `math-mathjax` renders
//! with nothing installed, and its outputs are byte for byte those of the same document
//! rendered with `--extensions-dir` naming the repository's folder of the extension; a copy
//! installed beside the document wins over the bundled one.

#![cfg(all(feature = "bundled-extensions", feature = "js"))]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

use flm_core::dump::golden::assert_text_golden;

/// The repository's folder of extensions of the engine `engine`.
fn shipped(engine: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../extensions").join(engine)
}

/// A fresh folder for one test, under the build folder, holding the files `files`.
fn folder(name: &str, files: &[(&str, &[u8])]) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-extensions-bundled-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    for (file, bytes) in files {
        let file = path.join(file);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, bytes).unwrap();
    }
    path
}

/// Runs `flm render ARGS -o out/NAME.html NAME.flm` in the folder `cwd`, its home folder
/// there too and `FLMPATH` unset, and returns every file written under `out/`, by name.
fn render(cwd: &Path, args: &[&str], name: &str) -> BTreeMap<String, Vec<u8>> {
    let output = Command::new(env!("CARGO_BIN_EXE_flm"))
        .arg("render")
        .args(args)
        .args(["-o", &format!("out/{name}.html"), &format!("{name}.flm")])
        .current_dir(cwd)
        .env("HOME", cwd)
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE")
        .output()
        .unwrap();
    let reported = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{args:?}: {reported}");
    assert!(reported.is_empty(), "{args:?}: {reported}");
    std::fs::read_dir(cwd.join("out"))
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .map(|entry| (entry.file_name().into_string().unwrap(), std::fs::read(entry.path()).unwrap()))
        .collect()
}

/// The outputs of the document `name.flm`, holding `document`, with the files `beside`
/// beside it: rendered with `--extensions-dir` naming the repository's folder `engine`, and
/// rendered with nothing installed. Asserts that the two are the same, and returns them.
fn both_ways(test: &str, engine: &str, name: &str, document: &[u8], beside: &[(&str, &[u8])]) -> BTreeMap<String, Vec<u8>> {
    let mut files = vec![(format!("{name}.flm"), document)];
    files.extend(beside.iter().map(|(file, bytes)| (file.to_string(), *bytes)));
    let files: Vec<(&str, &[u8])> = files.iter().map(|(file, bytes)| (file.as_str(), *bytes)).collect();
    let installed = folder(&format!("{test}-installed"), &files);
    let dir = shipped(engine);
    let through_folder = render(&installed, &["--extensions-dir", dir.to_str().unwrap()], name);
    let bundled = folder(&format!("{test}-bundled"), &files);
    let through_bundle = render(&bundled, &[], name);
    assert_eq!(
        through_folder.keys().collect::<Vec<_>>(),
        through_bundle.keys().collect::<Vec<_>>(),
        "the same files are written"
    );
    for (file, bytes) in &through_folder {
        assert!(through_bundle[file] == *bytes, "the file `{file}` is the same both ways");
    }
    through_bundle
}

/// The lecture-notes build's sample document and its image.
fn sample() -> (Vec<u8>, Vec<u8>) {
    let documents = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/lecturenotes");
    (std::fs::read(documents.join("notes.flm")).unwrap(), std::fs::read(documents.join("cycle.svg")).unwrap())
}

#[test]
fn the_lecture_notes_sample_renders_with_nothing_installed_as_through_the_folder() {
    let (notes, image) = sample();
    let outputs = both_ways("lecturenotes", "data", "notes", &notes, &[("cycle.svg", &image)]);
    let names: Vec<&str> = outputs.keys().map(String::as_str).collect();
    assert_eq!(names, ["notes-fragment.html", "notes-script.js", "notes-style.css", "notes.html"]);
    let page = String::from_utf8(outputs["notes.html"].clone()).unwrap();
    assert!(page.contains("<header class=\"ln-titleblock\">"), "{page}");
}

/// A document of one formula using `math-mathjax`, in its `mathml` mode.
const MATH: &str = "---\nflm:\n  use: [math-mathjax]\n  features:\n    math-mathjax:\n      mode: mathml\n---\nThe square \\(x^2\\).\n";

#[test]
fn a_formula_renders_as_mathml_with_nothing_installed_as_through_the_folder() {
    let outputs = both_ways("math-mathjax", "js", "paper", MATH.as_bytes(), &[]);
    let page = String::from_utf8(outputs["paper.html"].clone()).unwrap();
    assert!(
        page.contains(r#"<math xmlns="http://www.w3.org/1998/Math/MathML"><msup><mi>x</mi><mn>2</mn></msup></math>"#),
        "{page}"
    );
    let golden = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/extensions/bundled-math-mathjax.html");
    assert_text_golden(golden, &page);
}

#[test]
fn a_copy_installed_beside_the_document_wins_over_the_bundled_extension() {
    let (notes, image) = sample();
    let copy: &[u8] = b"extension:\n  description: A copy of the lecture notes that sets nothing\n";
    let dir = folder(
        "precedence",
        &[("notes.flm", &notes), ("cycle.svg", &image), ("_flmextensions/lecturenotes.yaml", copy)],
    );
    let outputs = render(&dir, &[], "notes");
    let names: Vec<&str> = outputs.keys().map(String::as_str).collect();
    assert!(!names.contains(&"notes-script.js"), "{names:?}");
    let page = String::from_utf8(outputs["notes.html"].clone()).unwrap();
    assert!(!page.contains("ln-titleblock"), "{page}");
}
