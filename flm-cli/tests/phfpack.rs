//! The package `phfpack` in the program (acceptance tests 11, 14, 15 and 16 of
//! the step that added it): a document that uses it, its settings from a front
//! matter and from an extension written in YAML, the schema, the program without
//! it, and a program of its own over the library entry point.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

fn folder(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-phfpack-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn write(dir: &Path, path: &str, text: &str) {
    let path = dir.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Whether `text` holds the replacements, the text output's line breaks
/// read as spaces.
fn holds_replacements(text: &str) -> bool {
    text.split_whitespace().collect::<Vec<_>>().join(" ").contains(REPLACED)
}

fn flm(cwd: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_flm"));
    command.args(args).current_dir(cwd).env("HOME", cwd).env_remove("FLMPATH").env_remove("USERPROFILE");
    command.output().unwrap()
}

/// A document that uses the package, declares one macro in its front matter
/// and uses a package written in YAML that declares another, under the
/// program's default list of packages.
const PAPER: &str = "---\nflm:\n  use: [phfpack, norms]\n  features:\n    phfqit:\n      macro_definitions:\n        opnorm:\n          delimited_args: ['{']\n          content: '#[\\lVert]{DelimLeft}{#1}#[\\rVert]{DelimRight}_\\infty'\n---\nLet \\(\\ket{\\psi}\\), \\(\\opnorm`\\Big{A}\\) and \\(\\onenorm*{B}\\).\n";

const NORMS: &str = "flm:\n  features:\n    phfqit:\n      macro_definitions:\n        onenorm:\n          delimited_args: ['{']\n          content: '#[\\lVert]{DelimLeft}{#1}#[\\rVert]{DelimRight}_1'\n";

const REPLACED: &str = r"Let \(\lvert{\psi}\rangle\), \(\Bigl \lVert{A}\Bigr \rVert_\infty\) and \(\mathopen{}\left \lVert{B}\right \rVert\mathclose{}_1\).";

#[cfg(feature = "phfpack")]
#[test]
fn render_writes_the_replacements_with_macros_declared_in_a_front_matter_and_a_package() {
    let dir = folder("render");
    write(&dir, "paper.flm", PAPER);
    write(&dir, "norms.yaml", NORMS);
    let output = flm(&dir, &["render", "-p", "text", "--extensions-dir", ".", "paper.flm"]);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{output:?}");
    assert!(holds_replacements(&text), "{text}");
}

/// A document that uses the package under the program's default list, whose
/// math processor is `math-mathjax`: the double angle brackets alone, at a
/// size, and after `\left`, `\middle` and `\right`, and `\bitstring` (a template
/// that writes `\ensuremath`) in text and inside a formula.
#[cfg(all(feature = "phfpack", feature = "js", feature = "bundled-extensions"))]
const BRACKETS: &str = "---\nflm:\n  use: [phfpack]\n---\nLet \\(\\oket{\\psi}\\), \\(\\obra`\\Big{\\phi}\\), \\(\\obraket*{a}{b}\\) and \\(\\oketbra{x}{y}\\); the string \\bitstring{0110} and \\(s = \\bitstring{01}\\).\n";

#[cfg(all(feature = "phfpack", feature = "js", feature = "bundled-extensions"))]
#[test]
fn the_package_loads_beside_math_mathjax_in_both_of_its_modes() {
    let dir = folder("brackets");
    write(&dir, "paper.flm", BRACKETS);
    // `mathml`: MathJax typesets every formula as MathML.
    let output = flm(&dir, &["render", "-f", "html", "-O", "math-mathjax.mode=mathml", "paper.flm"]);
    let page = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(page.matches("<math ").count(), 6, "{page}");
    // `\oket`, `\obra`, `\obraket*` and `\oketbra` write three brackets of each kind.
    assert_eq!(page.matches('\u{27EA}').count(), 3, "{page}");
    assert_eq!(page.matches('\u{27EB}').count(), 3, "{page}");
    assert!(page.contains("<mo minsize=\"1.623em\" maxsize=\"1.623em\" lspace=\"0\" rspace=\"0\">\u{27EA}</mo>"), "{page}");
    // `\bitstring`: an inline formula in text, its argument inside a formula.
    assert_eq!(page.matches("<munder><mover>").count(), 2, "{page}");
    assert!(!page.contains("ensuremath"), "{page}");
    // `load-in-browser`, the default mode: the page's MathJax defines the two brackets.
    let output = flm(&dir, &["render", "-f", "html", "paper.flm"]);
    let page = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{output:?}");
    assert!(output.stderr.is_empty(), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(page.contains(r"\llangle"), "{page}");
    assert!(page.contains(r"\Bigl \llangle"), "{page}");
    assert!(page.contains(r"\left \llangle"), "{page}");
    assert!(page.contains("'math-mathjax']"), "{page}");
    assert!(page.contains("tex.TokenMap.DelimiterMap('math-mathjax-delimiters'"), "{page}");
    assert!(!page.contains("ensuremath"), "{page}");
}

#[cfg(feature = "phfpack")]
#[test]
fn schema_prints_the_settings_of_phfqit() {
    let dir = folder("schema");
    let output = flm(&dir, &["schema"]);
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.success(), "{output:?}");
    for key in ["phfqit", "macro_definitions", "delimited_args", "spaceKets", "AfterComma"] {
        assert!(text.contains(key), "`{key}` in the schema");
    }
}

/// Runs the library entry point in this process, as a program over it would,
/// writing its output to `out`.
fn run_in_process(dir: &Path, packages: Vec<flm_core::core::registry::PackageFactory>, out: &Path) -> ExitCode {
    let args: Vec<OsString> = ["render", "-p", "text", "--extensions-dir"]
        .iter()
        .map(OsString::from)
        .chain([dir.as_os_str().to_owned(), OsString::from("-o"), out.as_os_str().to_owned()])
        .chain([dir.join("paper.flm").into_os_string()])
        .collect();
    flm_cli::run(args, packages)
}

#[test]
fn a_program_of_its_own_carrying_the_package_renders_a_document_that_uses_it() {
    let dir = folder("own");
    write(&dir, "paper.flm", PAPER);
    write(&dir, "norms.yaml", NORMS);
    let out = dir.join("paper.txt");
    let code = run_in_process(&dir, vec![flm_phfpack::factory().unwrap()], &out);
    assert_eq!(code, ExitCode::SUCCESS);
    let text = std::fs::read_to_string(&out).unwrap();
    assert!(holds_replacements(&text), "{text}");
}

/// Set in the environment of the child process that
/// [`a_program_without_the_package_refuses_a_document_that_names_it`] runs: the
/// child is this test binary running that one test, which then runs the
/// program over no package and exits with its status, so that the parent reads
/// what the program wrote on its standard error.
const WITHOUT_THE_PACKAGE: &str = "FLM_CLI_TEST_RUN_WITHOUT_PHFPACK";

#[test]
fn a_program_without_the_package_refuses_a_document_that_names_it() {
    if std::env::var_os(WITHOUT_THE_PACKAGE).is_some() {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-cli-phfpack-without");
        let code = run_in_process(&dir, Vec::new(), &dir.join("paper.txt"));
        std::process::exit(if code == ExitCode::SUCCESS { 0 } else { 1 });
    }
    let dir = folder("without");
    let out = dir.join("paper.txt");
    write(&dir, "paper.flm", PAPER);
    write(&dir, "norms.yaml", NORMS);
    let child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "a_program_without_the_package_refuses_a_document_that_names_it"])
        .args(["--nocapture", "--test-threads=1"])
        .env(WITHOUT_THE_PACKAGE, "1")
        .output()
        .unwrap();
    assert!(!child.status.success(), "{child:?}");
    assert!(!out.exists());
    // What the program reports is an error naming the package it does not
    // know (the folder's own path, which holds the word too, left out).
    let reported = String::from_utf8_lossy(&child.stderr).replace(dir.to_str().unwrap(), "");
    assert!(
        reported.lines().any(|line| line.starts_with("error") && line.contains("phfpack")),
        "{reported}"
    );
}
