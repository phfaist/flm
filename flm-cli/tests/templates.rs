//! The templates through the binary: `flm render` with nothing named, for each of the four
//! outputs, as goldens; the table of contents that HTML's `simple` places between the header
//! and the main content, and its depth set on the command line; the recomposed LaTeX's
//! `simple` compiled under both engines, with and without metadata, written in a folder and
//! under a file's name; the LaTeX for LuaLaTeX's `none`, the settings of its `simple`, and
//! its `simple` compiled under `lualatex`; and the files that the compose templates of an extension ship (the
//! extensions `pair`, `fancy` and `iconic` of `tests/documents/outputs/_flmextensions/`):
//! one file two outputs' templates share, a page's stylesheet, applied as a template, and
//! logo, and a folder of icons shipped whole.
//!
//! The HTML goldens are under `tests/goldens/`, where the library's check of HTML validity
//! reads them too. The compilation is skipped unless the environment variable
//! `FLM_TESTS_LATEX_COMPILE` is set, as the library's own compilation checks are.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flm_core::dump::golden::assert_text_golden;

/// A fresh folder for one test, under the build folder.
fn folder(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-templates-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// Runs `flm render` with `args` in the folder `cwd`.
fn flm(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flm")).arg("render").args(args).current_dir(cwd).output().unwrap()
}

fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens").join(name)
}

/// A document holding metadata, a formula, a footnote and a heading.
const PAPER: &str = "---\ntitle: A \\emph{fine} paper on \\(x^2\\)\nauthor: [Ann, Bob]\ndate: 30 September 2026\n---\n\
\\section{Introduction}\n\nHello world\\footnote{A note.}, with \\(a+b\\).\n";

/// Renders `PAPER` with `args` into the folder `name`, and gives the folder.
fn rendered(name: &str, text: &str, args: &[&str]) -> PathBuf {
    let dir = folder(name);
    std::fs::write(dir.join("paper.flm"), text).unwrap();
    let output = flm(&dir, args);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    dir
}

// --- test 28 ---------------------------------------------------------------------------------

/// What `flm render` prints on standard output for `text` with `args`, run in the folder `name`.
fn printed(name: &str, text: &str, args: &[&str]) -> String {
    let output = run(name, text, args);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn render_with_nothing_named_gives_a_page() {
    assert_text_golden(golden("cli-simple.page.html"), &printed("html", PAPER, &["paper.flm"]));
}

#[test]
fn render_with_nothing_named_gives_the_text() {
    assert_text_golden(golden("cli-simple.txt"), &printed("text", PAPER, &["-p", "text", "paper.flm"]));
}

#[test]
fn render_with_nothing_named_gives_a_latex_document() {
    // Standard output holds an output of one file, and this document's recomposed LaTeX
    // comes with its style file: the two are written into a folder, the golden is the
    // document.
    let dir = rendered("latex", PAPER, &["-p", "latex-recomposed", "paper.flm", "-o", "out/"]);
    let document = std::fs::read_to_string(dir.join("out/index.tex")).unwrap();
    assert_text_golden(golden("cli-simple.tex"), &document);
}

#[test]
fn the_none_template_gives_the_content_and_the_pieces() {
    let dir = rendered("none", PAPER, &["paper.flm", "-t", "none", "-o", "paper.html"]);
    let fragment = std::fs::read_to_string(dir.join("paper.html")).unwrap();
    assert_text_golden(golden("cli-none.html"), &fragment);
}

// --- test 25: `-t` --------------------------------------------------------------------------

/// Runs `flm render` with `args` in a fresh folder `name` holding `paper.flm` written `text`.
fn run(name: &str, text: &str, args: &[&str]) -> Output {
    let dir = folder(name);
    std::fs::write(dir.join("paper.flm"), text).unwrap();
    flm(&dir, args)
}

/// Whether `output` succeeded and printed a complete page, the template `simple`'s.
fn printed_a_page(output: &Output) -> bool {
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8_lossy(&output.stdout).starts_with("<!DOCTYPE html>")
}

#[test]
fn t_chooses_the_template_over_the_front_matter() {
    let text = "---\nflm: {features: {compose: {template: simple}}}\n---\nHello.\n";
    assert!(printed_a_page(&run("t-front-matter", text, &["paper.flm"])));
    assert!(!printed_a_page(&run("t-front-matter", text, &["paper.flm", "-t", "none"])));
}

#[test]
fn an_option_choosing_the_template_wins_over_t() {
    // `-t` stands before every `-O`, wherever it is written: the `-O` is the later value.
    for args in [["paper.flm", "-t", "simple", "-O", "compose.template=none"], ["paper.flm", "-O", "compose.template=none", "-t", "simple"]] {
        assert!(!printed_a_page(&run("t-then-o", "Hello.\n", &args)), "{args:?}");
    }
}

#[test]
fn a_template_no_name_matches_is_reported_at_t() {
    let output = run("t-nosuch", "Hello.\n", &["paper.flm", "-t", "nosuch"]);
    assert_eq!(output.status.code(), Some(2));
    let printed = String::from_utf8_lossy(&output.stderr);
    assert!(printed.contains("-t nosuch") && !printed.contains("compose.template"), "{printed}");
}

// --- the table of contents under `simple` ---------------------------------------------------

/// A document whose front matter places the table of contents `flm.toc` with the depth 2:
/// a section holding a subsection holding a subsubsection, and a second section.
const TOC_PAPER: &str = "---\ntitle: A paper\nflm: {features: {pieces: {include: [flm.toc]}, toc: {depth: 2}}}\n---\n\
\\section{One}\n\nHello.\n\n\\subsection{Two}\n\nMore.\n\n\\subsubsection{Three}\n\nDeeper.\n\n\\section{Four}\n\nEnd.\n";

/// The table of contents of `page`, from `<nav` to `</nav>`.
fn nav(page: &str) -> &str {
    let start = page.find("<nav").unwrap_or_else(|| panic!("a table of contents in {page}"));
    &page[start..start + page[start..].find("</nav>").unwrap()]
}

#[test]
fn the_simple_page_writes_the_table_of_contents_between_the_header_and_the_main_content() {
    let page = printed("toc", TOC_PAPER, &["paper.flm"]);
    let at = |needle: &str| page.find(needle).unwrap_or_else(|| panic!("{needle} in {page}"));
    let header_end = at("</header>");
    let nav_start = at("<nav class=\"flm-pres-block flm-toc\" role=\"doc-toc\" data-flm-kind=\"toc\">");
    assert!(header_end < at("<div id=\"flm-region-flm.toc\">") && nav_start < at("<main"), "{page}");
    let nav = nav(&page);
    for anchor in ["sec--One", "sec--Two", "sec--Four"] {
        assert!(nav.contains(&format!("href=\"#{anchor}\"")), "{anchor} in {nav}");
    }
    assert!(!nav.contains("Three"), "the depth 2 leaves the subsubsection out: {nav}");
}

#[test]
fn without_the_include_the_simple_page_writes_no_table_of_contents() {
    let text = TOC_PAPER.replace("pieces: {include: [flm.toc]}, ", "");
    let page = printed("toc-none", &text, &["paper.flm"]);
    assert!(!page.contains("<nav") && !page.contains("flm-region-flm.toc"), "{page}");
    assert!(page.contains("    </header>\n    <main class=\"flm-content\" id=\"flm-content\">"), "{page}");
}

#[test]
fn an_option_s_depth_replaces_the_front_matter_s() {
    let page = printed("toc-depth", TOC_PAPER, &["paper.flm", "-O", "toc.depth=1"]);
    let nav = nav(&page);
    assert!(nav.contains("href=\"#sec--One\"") && nav.contains("href=\"#sec--Four\""), "{nav}");
    assert!(!nav.contains("Two") && !nav.contains("Three"), "the subsections are left out: {nav}");
}

// --- test 21 ---------------------------------------------------------------------------------

const ENABLE_VARIABLE: &str = "FLM_TESTS_LATEX_COMPILE";

fn enabled() -> bool {
    let value = std::env::var(ENABLE_VARIABLE).unwrap_or_default();
    !matches!(value.trim(), "" | "0")
}

#[test]
fn the_simple_latex_document_compiles_under_both_engines() {
    if !enabled() {
        println!("skipped: set {ENABLE_VARIABLE} to run the LaTeX compilation checks");
        return;
    }
    let untitled = "\\section{Introduction}\n\nHello world\\footnote{A note.}.\n";
    let bare = "Hello world.\n";
    for (name, text) in [("titled", PAPER), ("untitled", untitled), ("bare", bare)] {
        for engine in ["pdflatex", "lualatex"] {
            let dir = rendered(&format!("compile-{name}-{engine}"), text, &["-p", "latex-recomposed", "paper.flm", "-o", "out/"]);
            let out = dir.join("out");
            let document = std::fs::read_to_string(out.join("index.tex")).unwrap();
            if name == "bare" {
                assert!(!document.contains("\\usepackage"), "{document}");
            } else {
                assert!(document.contains("\\usepackage{index-flmrecomposed."), "{document}");
            }
            compiles(engine, &out, "index.tex", name);
        }
    }
}

/// Runs `engine` over `document` in the folder `out`, and asserts that it succeeded.
fn compiles(engine: &str, out: &Path, document: &str, name: &str) {
    let status = Command::new(engine)
        .current_dir(out)
        .args(["-interaction=nonstopmode", "-halt-on-error", document])
        .output()
        .unwrap_or_else(|error| panic!("{engine} runs: {error}"));
    assert!(status.status.success(), "{name} under {engine}: {}", String::from_utf8_lossy(&status.stdout));
}

#[test]
fn a_latex_document_named_with_o_compiles_with_its_style_file_named_after_it() {
    if !enabled() {
        println!("skipped: set {ENABLE_VARIABLE} to run the LaTeX compilation checks");
        return;
    }
    for engine in ["pdflatex", "lualatex"] {
        let dir = rendered(&format!("compile-named-{engine}"), PAPER, &["-p", "latex-recomposed", "paper.flm", "-o", "out/paper.tex"]);
        let out = dir.join("out");
        let styles: Vec<String> = std::fs::read_dir(&out)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|name| name.ends_with(".sty"))
            .collect();
        let [style] = styles.as_slice() else { panic!("{styles:?}") };
        assert!(style.starts_with("paper-flmrecomposed."), "{style}");
        let document = std::fs::read_to_string(out.join("paper.tex")).unwrap();
        let package = format!("\\usepackage{{{}}}", style.strip_suffix(".sty").unwrap());
        assert!(document.contains(&package), "{document}");
        compiles(engine, &out, "paper.tex", "paper.tex");
    }
}

// --- the LaTeX for LuaLaTeX ------------------------------------------------------------------

#[test]
fn render_with_nothing_named_gives_a_latex_rendered_document() {
    // The document comes with its style file: the two are written into a folder, the golden
    // is the document.
    let dir = rendered("rendered", PAPER, &["-p", "latex-rendered", "paper.flm", "-o", "out/"]);
    let document = std::fs::read_to_string(dir.join("out/index.tex")).unwrap();
    assert_text_golden(golden("cli-simple-rendered.tex"), &document);
}

#[test]
fn the_none_template_gives_the_latex_rendered_body() {
    let dir = rendered("rendered-none", PAPER, &["-p", "latex-rendered", "paper.flm", "-t", "none", "-o", "out/"]);
    let body = std::fs::read_to_string(dir.join("out/index.tex")).unwrap();
    assert!(!body.contains("\\documentclass") && !body.contains("\\begin{document}"), "{body}");
    assert!(body.contains("\\flmxHeading[") && body.contains("Hello world"), "{body}");
}

#[test]
fn the_simple_template_s_settings_class_options() {
    let class = |config: &str| {
        let option = format!("compose.template_config={config}");
        let args = ["-p", "latex-rendered", "paper.flm", "-t", "simple", "-O", option.as_str(), "-o", "out/"];
        let dir = rendered("rendered-config", PAPER, &args);
        let document = std::fs::read_to_string(dir.join("out/index.tex")).unwrap();
        document.lines().next().unwrap_or_default().to_string()
    };
    assert_eq!(class("{latex-rendered: {simple: {classoptions: '12pt,letterpaper'}}}"), "\\documentclass[12pt,letterpaper]{article}");
}

#[test]
fn the_simple_latex_rendered_document_compiles_under_lualatex() {
    if !enabled() {
        println!("skipped: set {ENABLE_VARIABLE} to run the LaTeX compilation checks");
        return;
    }
    let untitled = "\\section{Introduction}\n\nHello world\\footnote{A note.}.\n";
    for (name, text) in [("titled", PAPER), ("untitled", untitled), ("bare", "Hello world.\n")] {
        let dir = rendered(&format!("compile-rendered-{name}"), text, &["-p", "latex-rendered", "paper.flm", "-o", "out/"]);
        let out = dir.join("out");
        compiles("lualatex", &out, "index.tex", name);
    }
}

// --- the files a compose template ships ----------------------------------------------------

/// Copies the folder `from` into `to`, recursively.
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

/// The extensions of `tests/documents/outputs/_flmextensions/`.
fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/outputs/_flmextensions")
}

/// A fresh folder `name` holding the extensions of [`fixtures`] and `paper.flm`, which uses
/// the extension `extension`.
fn using(name: &str, extension: &str) -> PathBuf {
    let dir = folder(name);
    copy_tree(&fixtures(), &dir.join("_flmextensions"));
    std::fs::write(dir.join("paper.flm"), format!("---\nflm: {{use: [{extension}]}}\n---\nHello.\n")).unwrap();
    dir
}

/// The names in `folder` other than the document and the extensions, sorted.
fn written(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name != "paper.flm" && name != "_flmextensions")
        .collect();
    names.sort();
    names
}

#[test]
fn a_file_two_outputs_templates_ship_is_written_once_and_listed_by_both() {
    let dir = using("shared-equal", "pair");
    let output = flm(&dir, &["-v", "-p", "pair", "-t", "pair", "-o", "paper.html", "paper.flm"]);
    let told = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{told}");
    assert_eq!(
        written(&dir),
        [".paper.html.flm-written", ".paper.txt.flm-written", "paper-shared.css", "paper.html", "paper.txt"]
    );
    let shipped = std::fs::read(fixtures().join("pair/shared.css")).unwrap();
    assert_eq!(std::fs::read(dir.join("paper-shared.css")).unwrap(), shipped);
    for (record, entry) in [(".paper.html.flm-written", "paper.html"), (".paper.txt.flm-written", "paper.txt")] {
        let listed = std::fs::read_to_string(dir.join(record)).unwrap();
        assert_eq!(listed.lines().collect::<Vec<_>>(), ["paper-shared.css", entry], "{record}");
    }
    assert!(std::fs::read_to_string(dir.join("paper.html")).unwrap().contains("href=\"paper-shared.css\""));
    // `-v` names the file once.
    assert_eq!(told.matches("flm: wrote paper-shared.css\n").count(), 1, "{told}");
}

#[test]
fn two_outputs_templates_shipping_different_files_at_one_path_write_nothing() {
    let dir = using("shared-different", "pair");
    let output = flm(&dir, &["-p", "pair", "-t", "clash", "-o", "paper.html", "paper.flm"]);
    let told = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{told}");
    assert!(
        told.contains(
            "error: the output `text` cannot be written: the output `html` and the output `text` must not write \
             different content at one path, `paper-shared.css`"
        ),
        "{told}"
    );
    assert!(written(&dir).is_empty(), "{:?}", written(&dir));
}

#[test]
fn an_extension_template_ships_a_stylesheet_applied_as_a_template_and_a_logo() {
    let dir = using("fancy", "fancy");
    let output = flm(&dir, &["-f", "html", "-o", "paper.html", "paper.flm"]);
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(written(&dir), [".paper.html.flm-written", "paper-logo.png", "paper-style.css", "paper.html"]);
    let page = std::fs::read_to_string(dir.join("paper.html")).unwrap();
    assert!(page.starts_with("<link rel=\"stylesheet\" href=\"paper-style.css\">"), "{page}");
    // The stylesheet holds the template's setting `color` and the logo's path.
    assert_eq!(
        std::fs::read_to_string(dir.join("paper-style.css")).unwrap(),
        "p { color: teal; background: url(paper-logo.png) }\n"
    );
    assert_eq!(std::fs::read(dir.join("paper-logo.png")).unwrap(), std::fs::read(fixtures().join("fancy/img/logo.png")).unwrap());
}

#[test]
fn an_extension_template_ships_a_folder_written_under_the_stem_and_cleaned_on_the_next_render() {
    let dir = using("iconic", "iconic");
    let render = || {
        let output = flm(&dir, &["-f", "html", "-o", "out/paper.html", "paper.flm"]);
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    };
    render();
    let out = dir.join("out");
    let page = std::fs::read_to_string(out.join("paper.html")).unwrap();
    // The page sees the folder's path, with no trailing slash.
    assert!(page.starts_with("<img src=\"paper-icons/check.svg\">"), "{page}");
    let shipped = fixtures().join("iconic/templates/html/iconic/icons");
    for file in ["check.svg", "more/cross.svg"] {
        assert_eq!(std::fs::read(out.join("paper-icons").join(file)).unwrap(), std::fs::read(shipped.join(file)).unwrap(), "{file}");
    }
    assert_eq!(
        std::fs::read_to_string(out.join(".paper.html.flm-written")).unwrap(),
        "paper-icons/check.svg\npaper-icons/more/cross.svg\npaper.html\n"
    );
    // A file removed from the folder is removed from the output at the next render.
    std::fs::remove_file(dir.join("_flmextensions/iconic/templates/html/iconic/icons/more/cross.svg")).unwrap();
    render();
    assert!(!out.join("paper-icons/more/cross.svg").exists());
    assert!(out.join("paper-icons/check.svg").exists());
    assert_eq!(std::fs::read_to_string(out.join(".paper.html.flm-written")).unwrap(), "paper-icons/check.svg\npaper.html\n");
}

#[test]
fn an_extension_template_includes_a_template_file_and_the_text_of_files_as_it_is() {
    let dir = using("including", "including");
    let render = || flm(&dir, &["-f", "html", "-o", "out/paper.html", "paper.flm"]);
    let output = render();
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    let out = dir.join("out");
    let dist = fixtures().join("including/dist");
    let script = std::fs::read_to_string(dist.join("x.js")).unwrap();
    // The page: the included template applied with the page's context and escaping, the
    // raw file written as it is.
    let page = std::fs::read_to_string(out.join("paper.html")).unwrap();
    assert!(
        page.starts_with(&format!(
            "<link rel=\"stylesheet\" href=\"paper-style.css\"><script src=\"paper-script.js\"></script>\n\
             <header>A &lt;b&gt;bold&lt;&#x2f;b&gt; title</header>\n<script>{script}</script>\n"
        )),
        "{page}"
    );
    // The shipped files applied as templates: the raw files as they are, the settings as JSON.
    assert_eq!(
        std::fs::read_to_string(out.join("paper-style.css")).unwrap(),
        format!(":root {{ --c: teal; }}\n{}\n", std::fs::read_to_string(dist.join("x.css")).unwrap())
    );
    assert_eq!(
        std::fs::read_to_string(out.join("paper-script.js")).unwrap(),
        format!("{script}\nsetup({{\"color\":\"teal\",\"title\":\"A \\u003cb\\u003ebold\\u003c/b\\u003e title\"}});\n")
    );
    // An included file that is gone: the template cannot be applied.
    std::fs::remove_file(dir.join("_flmextensions/including/dist/x.js")).unwrap();
    let output = render();
    assert_ne!(output.status.code(), Some(0));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(
            "the compose template `including` includes `../../../dist/x.js`, and there is no such file in the \
             extension"
        ),
        "{stderr}"
    );
}
