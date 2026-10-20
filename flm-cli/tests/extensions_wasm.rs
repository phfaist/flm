//! An extension compiled from Rust to WebAssembly, found and run by the binary (cargo
//! feature `wasm`): case 1, the guest crate `flm-ext/tests/wasm/wordcount/` (the twin of the
//! JavaScript `wordcount`), its module copied with its manifest into `_flmextensions/` of a
//! fresh folder beside the documents of `tests/documents/extensions/`, rendered with `flm
//! render` to text and HTML; the output equals the JavaScript case's goldens
//! (`tests/goldens/extensions/wordcount.{txt,html}`, compared, never written). Gated on
//! `FLM_TESTS_WASM_GUEST=1` and the target `wasm32-unknown-unknown` being installed (the
//! module is built by `flm-ext/tests/common/guest.rs`); otherwise the test writes one
//! `skipped:` line. One test is not gated: the program's loader has the engine `wasm`,
//! shown with a module that builds no guest.

#![cfg(feature = "wasm")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flm_core::dump::golden::{check_text_golden, GoldenMode};

#[path = "../../flm-ext/tests/common/guest.rs"]
mod guest;

/// The folder of the crate `flm-ext`, whose `tests/wasm/` holds the guest crates.
fn flm_ext() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../flm-ext")
}

/// The folder of the documents of these tests.
fn documents() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/extensions")
}

/// `actual` equal to the golden `name` of the JavaScript case, compared and never written.
fn assert_javascript_golden(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/extensions").join(name);
    if let Err(error) = check_text_golden(&path, actual, GoldenMode::Compare) {
        panic!("{error}");
    }
}

/// Copies every file below `from` into `to`, making the folders.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A fresh folder `name` under the build folder, holding the documents of these tests and,
/// in `_flmextensions/wordcount/`, the guest's manifest and its module `module`.
fn folder(name: &str, module: &[u8]) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-extensions-wasm-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    copy_tree(&documents(), &path);
    let extension = path.join("_flmextensions/wordcount");
    std::fs::create_dir_all(&extension).unwrap();
    std::fs::copy(guest::crate_folder(&flm_ext(), "wordcount").join("flmextension.yaml"), extension.join("flmextension.yaml"))
        .unwrap();
    std::fs::write(extension.join("wordcount.wasm"), module).unwrap();
    path
}

/// Runs `flm` with `args` in the folder `cwd`, its home folder there too and `FLMPATH`
/// unset.
fn flm(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flm"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", cwd)
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE")
        .output()
        .unwrap()
}

/// The standard output of a run that succeeded, and its standard error.
fn noted(cwd: &Path, args: &[&str]) -> (String, String) {
    let output = flm(cwd, args);
    let reported = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(0), "{args:?}: {reported}");
    (String::from_utf8(output.stdout).unwrap(), reported)
}

/// The document's settings reach the guest (`shout_suffix` its `register`, `min_length`
/// and `report_counts` its count stage, whose Note appears under the plain workflows), and
/// the outputs are the JavaScript case's.
#[test]
fn case_one_compiled_to_webassembly_renders_from_flmextensions_with_its_settings() {
    let Some(module) = guest::module(&flm_ext(), "wordcount") else { return };
    let dir = folder("case-one", &module);
    let (text, notes) = noted(&dir, &["render", "-p", "text", "wordcount.flm"]);
    assert!(text.contains("LISTEN!") && text.contains("QUIETLY!") && !text.contains("check the numbers"), "{text}");
    assert_javascript_golden("wordcount.txt", &text);
    assert!(notes.contains("note: the tree `tree#0` holds 9 words of at least 4 characters"), "{notes}");
    let (html, notes) = noted(&dir, &["render", "-f", "html", "-t", "none", "wordcount.flm"]);
    assert!(html.contains("LISTEN!") && !html.contains("check the numbers"), "{html}");
    assert_javascript_golden("wordcount.html", &html);
    assert!(notes.contains("note: the tree `tree#0` holds 9 words of at least 4 characters"), "{notes}");
}

/// The program's loader has the engine `wasm`, with no guest built: the manifest
/// `engine: wasm` beside the empty module of 8 bytes reaches the engine, which reports the
/// module's missing exports in its own text (a program without the engine would report
/// instead that the manifest names an engine the program lacks).
#[test]
fn the_program_hands_a_manifest_of_the_engine_wasm_to_that_engine() {
    let dir = folder("engine", b"\0asm\x01\0\0\0");
    let output = flm(&dir, &["render", "-p", "text", "wordcount.flm"]);
    let reported = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(2), "{reported}");
    assert!(
        reported.contains(
            "the module `wordcount.wasm` of the extension `wordcount` must export the functions `flm_entry`, \
             `flm_alloc` and `flm_free` and the memory `memory`; it lacks `flm_entry`"
        ),
        "{reported}"
    );
    assert!(!reported.contains("(`extension.engine`), and this program has the engines"), "{reported}");
}
