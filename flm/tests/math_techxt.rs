//! The extension `math-techxt`, compiled into the binary (cargo feature
//! `bundled-extensions`): the math processor chosen with `math: techxt` in a front matter
//! renders each formula as Unicode text.
//!
//! The document holds an inline formula with a text command, a displayed formula, a
//! display-math environment of two lines (a matrix drawn over two lines and numbered, then a
//! line with a text command and an author's tag), a reference to the numbered line, and a
//! reference inside a text command of a formula. Rendered with `-p text -o -` and with `-p
//! html -o -`, it equals the goldens `tests/goldens/math-techxt/paper.txt` and `paper.html`
//! (`UPDATE_GOLDEN=1` rewrites them), with nothing reported. The same document without the
//! key `math`, rendered with `--math techxt`, gives the same text. Rendered to
//! `latex-recomposed`, it gives the files that `--math passthrough` gives, the formulas being
//! written as LaTeX from their nodes under either processor, and the run reports the plan's
//! warning that nothing checks that LaTeX, naming the feature `math-techxt`, and nothing
//! else.
//!
//! One test is gated: with `FLM_TESTS_WASM_GUEST=1` (and the target `wasm32-unknown-unknown`
//! installed), the script `extensions/rust/build-wasm.sh` builds the extension's WebAssembly
//! form, and the document rendered to `text` and to `html` with `--extensions-dir` pointing
//! at it gives the same bytes as the extension compiled in. Otherwise it writes one line
//! beginning `skipped:`.
#![cfg(feature = "bundled-extensions")]

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flm_core::dump::golden::assert_text_golden;
use serde_json::Value;

/// The document's body, after its front matter.
const BODY: &str = "\
Let \\(a + \\mbox{b} = c\\) hold, with
\\[ \\sum_{i=1}^{n} x_i^2 \\]
and the lines
\\begin{align}
  a &= \\begin{pmatrix} 1 & 2 \\\\ 30 & 4 \\end{pmatrix} \\label{eq:a} \\\\
  b &\\le \\text{c} \\tag{T}
\\end{align}
See \\eqref{eq:a}, and \\(x \\text{ by \\eqref{eq:a}}\\).
";

/// The document of the tests, choosing the processor in its front matter.
fn paper() -> String {
    format!("---\nflm:\n  math: techxt\n---\n{BODY}")
}

/// A fresh folder for one test, holding `paper.flm` with the text `document`.
fn folder(name: &str, document: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-math-techxt").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("paper.flm"), document).unwrap();
    dir
}

/// The path of the golden `name`.
fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/math-techxt").join(name)
}

/// Runs `flm render paper.flm` with `args` in `dir`, with no extension installed on the
/// search path (`HOME` is the folder, `FLMPATH` is unset).
fn render(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flm"))
        .arg("render")
        .arg("paper.flm")
        .args(args)
        .current_dir(dir)
        .env("HOME", dir)
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE")
        .output()
        .unwrap()
}

/// The standard output of a render that succeeded with nothing reported.
fn rendered(dir: &Path, args: &[&str]) -> String {
    let output = render(dir, args);
    assert!(output.status.success() && output.stderr.is_empty(), "{args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

/// The files of the folder `dir`, by name, with their bytes.
fn files(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| {
            let entry = entry.unwrap();
            (entry.file_name().into_string().unwrap(), std::fs::read(entry.path()).unwrap())
        })
        .collect()
}

/// The diagnostics of `output`, a run with `--json-messages`, each one's record.
fn diagnostics(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap_or_else(|error| panic!("{error}: {line:?}")))
        .filter(|record| record["kind"] == "diagnostic")
        .collect()
}

#[test]
fn the_text_output_writes_the_formulas_as_unicode_text() {
    let dir = folder("text", &paper());
    let text = rendered(&dir, &["-p", "text", "-o", "-"]);
    // FLM's number reaches the reference inside the formula's text command.
    assert!(text.contains("𝑥 by (1)"), "{text}");
    assert_text_golden(golden("paper.txt"), &text);
}

#[test]
fn the_html_output_writes_the_formulas_as_unicode_text_with_the_extension_s_style() {
    let dir = folder("html", &paper());
    let page = rendered(&dir, &["-p", "html", "-o", "-"]);
    assert!(page.contains("math-techxt.style"), "{page}");
    assert_text_golden(golden("paper.html"), &page);
}

/// `--math techxt` chooses the processor for a document whose front matter does not.
#[test]
fn the_option_math_chooses_techxt_as_the_front_matter_does() {
    let dir = folder("option", BODY);
    let text = rendered(&dir, &["--math", "techxt", "-p", "text", "-o", "-"]);
    assert_eq!(text, rendered(&folder("option-front-matter", &paper()), &["-p", "text", "-o", "-"]));
}

/// On `latex-recomposed`, the files are those that `--math passthrough` writes, and the run
/// reports the plan's warning naming `math-techxt`, and nothing else.
#[test]
fn the_latex_recomposed_output_is_the_library_s_latex_with_the_plan_s_warning() {
    let dir = folder("latex-recomposed", &paper());
    let output = render(&dir, &["-p", "latex-recomposed", "--json-messages", "-o", "out/"]);
    assert!(output.status.success(), "{output:?}");
    let reported = diagnostics(&output);
    assert_eq!(reported.len(), 1, "{reported:?}");
    let warning = &reported[0];
    assert_eq!(warning["severity"], "warning", "{warning}");
    assert_eq!(warning["id"], "flm.run.unchecked-output", "{warning}");
    assert_eq!(warning["data"]["feature"], "math-techxt", "{warning}");
    assert_eq!(warning["data"]["backend"], "latex-recomposed", "{warning}");

    let passthrough = folder("latex-recomposed-passthrough", BODY);
    let output = render(&passthrough, &["--math", "passthrough", "-p", "latex-recomposed", "-o", "out/"]);
    assert!(output.status.success(), "{output:?}");
    let written = files(&dir.join("out"));
    assert!(written.keys().any(|name| name.ends_with(".tex")), "{:?}", written.keys());
    let tex = written.values().map(|bytes| String::from_utf8_lossy(bytes).into_owned()).collect::<String>();
    assert!(tex.contains("\\begin{pmatrix}") && tex.contains("\\text{c}"), "{tex}");
    assert_eq!(written, files(&passthrough.join("out")));
}

#[cfg(feature = "wasm")]
#[path = "../../flm-ext/tests/common/guest.rs"]
#[allow(dead_code)]
mod guest;

/// The extension's WebAssembly form, built by `extensions/rust/build-wasm.sh` into a folder
/// that `--extensions-dir` points at, is found before the extension compiled in and gives
/// the same bytes for the document on `text` and on `html`.
#[cfg(feature = "wasm")]
#[test]
fn the_webassembly_form_built_by_the_script_gives_the_same_bytes() {
    if let Some(line) = guest::skipped() {
        println!("{line}");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let modules = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-math-techxt-wasm");
    let _ = std::fs::remove_dir_all(&modules);
    // The script's cargo run builds in a target folder of its own, apart from the folder
    // of the cargo run that runs this test.
    let script = Command::new(root.join("extensions/rust/build-wasm.sh"))
        .arg("--out")
        .arg(&modules)
        .arg("math-techxt")
        .env("CARGO_TARGET_DIR", root.join("target/wasm-guest"))
        .output()
        .unwrap();
    assert!(script.status.success(), "the script must build the module:\n{}", String::from_utf8_lossy(&script.stderr));
    let mut shipped: Vec<String> =
        std::fs::read_dir(modules.join("math-techxt")).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    shipped.sort();
    assert_eq!(shipped, ["README.md", "flmextension.yaml", "math_techxt.wasm"]);

    let dir = folder("wasm", &paper());
    for backend in ["text", "html"] {
        let native = rendered(&dir, &["-p", backend, "-o", "-"]);
        let output = render(&dir, &["--extensions-dir", modules.to_str().unwrap(), "-v", "-p", backend, "-o", "-"]);
        let said = String::from_utf8_lossy(&output.stderr).into_owned();
        assert_eq!(output.status.code(), Some(0), "{said}");
        // The module was loaded, not the extension compiled in (`-v` lists the packages in use).
        assert!(said.contains(modules.join("math-techxt").to_str().unwrap()) && !said.contains("bundled:rust/math-techxt"), "{said}");
        assert_eq!(String::from_utf8(output.stdout).unwrap(), native, "{backend}");
    }
}
