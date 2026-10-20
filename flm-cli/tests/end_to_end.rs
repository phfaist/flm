//! The program from end to end: one document holding a front matter and every feature of
//! the standard library whose use needs no service of the host beyond reading the document,
//! rendered by `flm render` to each of the four outputs, as goldens, and checked by
//! `flm check`.
//!
//! The document is `tests/documents/every-feature.flm`. Its front matter holds the metadata
//! (a title with emphasis and a formula, two authors, a date) and, under `flm`, a key of
//! `parsing` and the settings of two features (user definitions, an annotator). Its content
//! uses the headings, labels and references, the formulas, the notes, the text styles and
//! literals, verbatim text, a link, an annotation, a term's definition, a theorem and its
//! proof, the three kinds of list, a quotation, a figure holding an image and a table of
//! cells. The citations are left out: citing a work needs a source of bibliography entries,
//! which the program does not set up yet.
//!
//! A second document, `tests/documents/several-files/paper.flm`, is made of several files and
//! takes settings from two extensions found by name in the folder `_flmextensions` beside it:
//! `house`, an extension written as one file (a user macro, the text's width), and `journal`,
//! an extension written as a folder that ships a template for each output and chooses it. The document
//! brings in `parts/aside.flm` in the middle of a paragraph (a content input) and
//! `parts/method.flm`, a file that opens with its own section (a heading input), which itself
//! brings in `parts/details.flm`, named relative to it. The document is rendered to the four
//! outputs as goldens, checked by `flm check`, and its extensions' manifests and front matters
//! are checked against the schemas `flm schema` prints. The primary file goes on after the
//! heading input, so its notes are counted in reading order across the files: the primary
//! file's, the heading input's, then the primary file's again.
//!
//! The LaTeX documents are compiled when the environment variable `FLM_TESTS_LATEX_COMPILE`
//! is set, as the library's own compilation checks are: the recomposed LaTeX under
//! `pdflatex` and `lualatex`, the LaTeX for LuaLaTeX under `lualatex`.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flm_core::dump::golden::assert_text_golden;

#[path = "../../flm-core/tests/support/json_schema.rs"]
mod validator;

/// The folder holding the document and the image it shows.
fn documents() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents")
}

/// The document, as the author wrote it.
fn document() -> String {
    std::fs::read_to_string(documents().join("every-feature.flm")).unwrap()
}

fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens").join(name)
}

/// A fresh folder `name` under the build folder, holding the document as `paper.flm` and
/// the image it shows, `figures/plot.png`.
fn folder(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-end-to-end-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(path.join("figures")).unwrap();
    std::fs::write(path.join("paper.flm"), document()).unwrap();
    std::fs::copy(documents().join("figures/plot.png"), path.join("figures/plot.png")).unwrap();
    path
}

fn flm(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flm")).args(args).current_dir(cwd).output().unwrap()
}

/// Runs `flm` with `args` in `cwd`, and asserts that it succeeded and reported nothing.
fn succeeds_silently(cwd: &Path, args: &[&str]) -> Output {
    let output = flm(cwd, args);
    let reported = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{args:?}: {reported}");
    assert!(reported.is_empty(), "{args:?}: {reported}");
    output
}

/// The names of the files in `folder`, sorted.
fn listing(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        std::fs::read_dir(folder).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

#[test]
fn the_document_renders_to_a_page() {
    let dir = folder("html");
    succeeds_silently(&dir, &["render", "paper.flm", "-o", "paper.html"]);
    let page = std::fs::read_to_string(dir.join("paper.html")).unwrap();
    assert_text_golden(golden("e2e-every-feature.page.html"), &page);
}

#[test]
fn the_document_renders_to_text() {
    let dir = folder("text");
    let output = succeeds_silently(&dir, &["render", "-p", "text", "paper.flm"]);
    assert_text_golden(golden("e2e-every-feature.txt"), &String::from_utf8(output.stdout).unwrap());
}

#[test]
fn the_document_renders_to_latex_with_its_style_file() {
    let dir = folder("latex");
    succeeds_silently(&dir, &["render", "-p", "latex-recomposed", "paper.flm", "-o", "out/"]);
    let out = dir.join("out");
    // Beside the document and its style file stands the hidden record of the files written.
    let files: Vec<String> = listing(&out).into_iter().filter(|name| !name.starts_with('.')).collect();
    assert_eq!(files.len(), 2, "{files:?}");
    let style = files.iter().find(|name| name.ends_with(".sty")).expect("a style file");
    let document = std::fs::read_to_string(out.join("index.tex")).unwrap();
    assert!(document.contains(&format!("\\usepackage{{{}}}", style.trim_end_matches(".sty"))), "{document}");
    assert_text_golden(golden("e2e-every-feature.tex"), &document);
    // The document names its image where it stands, from the output's folder.
    assert!(document.contains("{../figures/plot.png}"), "{document}");
    compile(&out);
}

#[test]
fn the_document_renders_to_latex_rendered_with_its_style_file() {
    let dir = folder("latex-rendered");
    succeeds_silently(&dir, &["render", "-p", "latex-rendered", "paper.flm", "-o", "out/"]);
    let out = dir.join("out");
    // Beside the document and its style file stands the hidden record of the files written.
    let files: Vec<String> = listing(&out).into_iter().filter(|name| !name.starts_with('.')).collect();
    assert_eq!(files.len(), 2, "{files:?}");
    let style = files.iter().find(|name| name.ends_with(".sty")).expect("a style file");
    assert!(style.starts_with("index-flmx."), "{style}");
    let document = std::fs::read_to_string(out.join("index.tex")).unwrap();
    assert!(document.contains(&format!("\\usepackage{{{}}}", style.trim_end_matches(".sty"))), "{document}");
    assert_text_golden(golden("e2e-every-feature-rendered.tex"), &document);
    // The document names its image where it stands, from the output's folder.
    assert!(document.contains("{../figures/plot.png}"), "{document}");
    compile_rendered(&out);
}

#[test]
fn the_document_checks_clean_and_nothing_is_written() {
    let dir = folder("check");
    succeeds_silently(&dir, &["check", "paper.flm"]);
    for format in ["html", "text", "latex-recomposed", "latex-rendered"] {
        succeeds_silently(&dir, &["check", "-p", format, "paper.flm"]);
    }
    assert_eq!(listing(&dir), ["figures", "paper.flm"]);
}

const ENABLE_VARIABLE: &str = "FLM_TESTS_LATEX_COMPILE";

/// Compiles `index.tex` in `folder` under both engines, when the checks are enabled.
fn compile(folder: &Path) {
    let value = std::env::var(ENABLE_VARIABLE).unwrap_or_default();
    if matches!(value.trim(), "" | "0") {
        println!("skipped: set {ENABLE_VARIABLE} to run the LaTeX compilation checks");
        return;
    }
    for engine in ["pdflatex", "lualatex"] {
        let status = Command::new(engine)
            .current_dir(folder)
            .args(["-interaction=nonstopmode", "-halt-on-error", "index.tex"])
            .output()
            .unwrap_or_else(|error| panic!("{engine} runs: {error}"));
        assert!(status.status.success(), "under {engine}: {}", String::from_utf8_lossy(&status.stdout));
    }
}

/// Compiles `index.tex` in `folder` under `lualatex`, the one engine of the LaTeX for
/// LuaLaTeX, when the checks are enabled.
fn compile_rendered(folder: &Path) {
    let value = std::env::var(ENABLE_VARIABLE).unwrap_or_default();
    if matches!(value.trim(), "" | "0") {
        println!("skipped: set {ENABLE_VARIABLE} to run the LaTeX compilation checks");
        return;
    }
    let status = Command::new("lualatex")
        .current_dir(folder)
        .args(["-interaction=nonstopmode", "-halt-on-error", "index.tex"])
        .output()
        .unwrap_or_else(|error| panic!("lualatex runs: {error}"));
    assert!(status.status.success(), "under lualatex: {}", String::from_utf8_lossy(&status.stdout));
}

#[test]
fn check_reports_the_problems_of_included_files() {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-cli-end-to-end-check-input");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("parts")).unwrap();
    std::fs::write(dir.join("paper.flm"), "Intro.\n\n\\input{parts/one.flm}\n").unwrap();
    std::fs::write(
        dir.join("parts/one.flm"),
        "---\ntitle: One\nflm:\n  input_as: {heading: section}\n---\nText \\input{two.flm}.\n",
    )
    .unwrap();
    std::fs::write(dir.join("parts/two.flm"), "and \\nosuchmacro\n").unwrap();
    let output = flm(&dir, &["check", "paper.flm"]);
    let reported = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{reported}");
    assert!(reported.contains("parts/two.flm:1:5"), "{reported}");
    assert!(reported.contains("parts/one.flm:6:6"), "{reported}");
    assert_eq!(listing(&dir), ["paper.flm", "parts"]);
}

// --- a document of several files, with two extensions found by name -------------------------

/// The folder of the document of several files, its extensions and its included files.
fn several_files() -> PathBuf {
    documents().join("several-files")
}

/// Copies the folder `from`, its subfolders included, to `to`.
fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).unwrap();
        }
    }
}

/// A fresh folder `name` under the build folder, holding a copy of the document of several
/// files with its extensions and its included files.
fn several_files_folder(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-end-to-end-several-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    copy_folder(&several_files(), &path);
    path
}

#[test]
fn the_document_of_several_files_renders_to_text_with_its_packages() {
    let dir = several_files_folder("text");
    // The primary file named with a leading `./`, as a user may type it.
    let output = succeeds_silently(&dir, &["render", "-p", "text", "./paper.flm"]);
    let text = String::from_utf8(output.stdout).unwrap();
    // The folder extension's template, the file extension's macro and width, the three included
    // files in their places.
    assert!(text.starts_with("== The Journal ==\n"), "{text}");
    assert!(text.contains("the Example project"), "{text}");
    assert!(text.lines().all(|line| line.chars().count() <= 60), "{text}");
    // The note written after the heading input is counted after the heading input's.
    assert!(text.contains("after that file’s.ᶜ"), "{text}");
    assert_text_golden(golden("e2e-several-files.txt"), &text);
}

#[test]
fn the_document_of_several_files_renders_to_a_page() {
    let dir = several_files_folder("html");
    succeeds_silently(&dir, &["render", "paper.flm", "-o", "paper.html"]);
    let page = std::fs::read_to_string(dir.join("paper.html")).unwrap();
    assert!(page.contains("<article class=\"journal\">"), "{page}");
    assert_text_golden(golden("e2e-several-files.page.html"), &page);
}

#[test]
fn the_document_of_several_files_renders_to_one_latex_file_that_compiles() {
    let dir = several_files_folder("latex");
    succeeds_silently(&dir, &["render", "-p", "latex-recomposed", "paper.flm", "-o", "out/"]);
    let out = dir.join("out");
    // One document for all the source files, and its style file.
    let files: Vec<String> = listing(&out).into_iter().filter(|name| !name.starts_with('.')).collect();
    assert_eq!(files.len(), 2, "{files:?}");
    let document = std::fs::read_to_string(out.join("index.tex")).unwrap();
    assert!(document.starts_with("\\documentclass[twocolumn]{article}"), "{document}");
    assert_text_golden(golden("e2e-several-files.tex"), &document);
    compile(&out);
}

#[test]
fn the_document_of_several_files_renders_to_latex_rendered_that_compiles() {
    let dir = several_files_folder("latex-rendered");
    // The extension `journal` chooses its own template, which it ships for the three other
    // outputs alone: `-t` chooses `std`'s.
    let args = ["render", "-p", "latex-rendered", "-t", "simple", "paper.flm", "-o", "out/"];
    succeeds_silently(&dir, &args);
    let out = dir.join("out");
    // One document for all the source files, and its style file.
    let files: Vec<String> = listing(&out).into_iter().filter(|name| !name.starts_with('.')).collect();
    assert_eq!(files.len(), 2, "{files:?}");
    let document = std::fs::read_to_string(out.join("index.tex")).unwrap();
    assert_text_golden(golden("e2e-several-files-rendered.tex"), &document);
    compile_rendered(&out);
}

#[test]
fn the_document_of_several_files_checks_clean_and_nothing_is_written() {
    let dir = several_files_folder("check");
    let before = listing(&dir);
    for format in ["html", "text", "latex-recomposed"] {
        succeeds_silently(&dir, &["check", "-p", format, "paper.flm"]);
    }
    // The extension `journal` ships no template for this output: `-t` chooses `std`'s.
    succeeds_silently(&dir, &["check", "-p", "latex-rendered", "-t", "simple", "paper.flm"]);
    assert_eq!(listing(&dir), before);
}

/// The schema `flm schema --of of` prints, as data.
fn printed_schema(of: &str) -> serde_json::Value {
    let output = succeeds_silently(&several_files(), &["schema", "--of", of]);
    serde_json::from_slice(&output.stdout).expect("JSON")
}

/// The YAML `text`, starting at the line `line` of its file, as data.
fn yaml(text: &str, line: u32) -> serde_json::Value {
    flm_main::seed::read_front_matter(text, line).expect("YAML").to_json()
}

/// The front matter of the source file `path`, as data.
fn front_matter(path: &Path) -> serde_json::Value {
    let text = std::fs::read_to_string(path).unwrap();
    let split = flm_main::seed::split(&text).unwrap();
    yaml(split.front_matter.expect("a front matter"), split.front_matter_line)
}

#[test]
fn the_manifests_and_front_matters_of_the_document_of_several_files_match_the_printed_schemas() {
    let manifest = printed_schema("extension");
    for file in ["_flmextensions/house.yaml", "_flmextensions/journal/flmextension.yaml"] {
        let data = yaml(&std::fs::read_to_string(several_files().join(file)).unwrap(), 1);
        assert!(validator::valid(&manifest, &manifest, &data), "the schema of a manifest refuses `{file}`");
    }
    // The schema sees a value its key does not accept.
    let wrong = yaml("flm:\n  features:\n    text:\n      width: wide\n", 1);
    assert!(!validator::valid(&manifest, &manifest, &wrong));
    let front = printed_schema("front-matter");
    for file in ["paper.flm", "parts/method.flm"] {
        let data = front_matter(&several_files().join(file));
        assert!(validator::valid(&front, &front, &data), "the schema of a front matter refuses `{file}`");
    }
    // The feature `write` lists its setting `output_names`.
    assert!(front["$defs"]["write"]["properties"].get("output_names").is_some(), "{}", front["$defs"]["write"]);
    let named = yaml("flm:\n  features:\n    write:\n      output_names: {bibtex: refs.bib}\n", 1);
    assert!(validator::valid(&front, &front, &named));
}
