//! `flm info`: the pages that describe the environment, run through the binary.
//!
//! Each test runs the program in a folder of its own under the cargo target's folder for
//! tests, with that folder as the home folder and `FLMPATH` and `COLUMNS` unset, so that no
//! extension installed on the machine takes part. Standard output is not a terminal, so the
//! text has the text backend's default width, which no option of the command changes. The
//! text pages are compared to the goldens of
//! `tests/goldens/info/` (`UPDATE_GOLDEN=1` rewrites them); they need the extension `flmdoc`,
//! which the program carries among its bundled extensions. The JSON pages are read back.
//! Where the page does not depend on the math processor, `--math passthrough` keeps the
//! environment from loading the bundled MathJax extension, which takes a few seconds.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

/// A fresh folder of the test's own, `NAME` under the cargo target's folder for tests,
/// holding in `doc/` a copy of the files of `tests/documents/info/` when `with_documents`.
fn folder(name: &str, with_documents: bool) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("info").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    if with_documents {
        copy_folder(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/info"), &dir.join("doc"));
    }
    dir
}

/// Copies the folder `from`, with the folders it holds, to `to`.
fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        if entry.file_type().unwrap().is_dir() {
            copy_folder(&entry.path(), &to.join(entry.file_name()));
        } else {
            std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
        }
    }
}

/// Runs `flm info` with `args` in `dir`, with `dir` as the home folder and `FLMPATH` and
/// `COLUMNS` unset.
fn flm(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flm"))
        .arg("info")
        .args(args)
        .current_dir(dir)
        .env("HOME", dir)
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE")
        .env_remove("COLUMNS")
        .output()
        .unwrap()
}

/// The standard output of `flm info` with `args`, which must succeed with nothing on
/// standard error.
fn shown(dir: &Path, args: &[&str]) -> String {
    let output = flm(dir, args);
    assert!(output.status.success() && output.stderr.is_empty(), "{args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

/// The JSON output of `flm info --json` with `args`, read back.
fn json(dir: &Path, args: &[&str]) -> Value {
    let text = shown(dir, &[&["--json"], args].concat());
    assert!(text.ends_with("}\n"), "one final newline");
    serde_json::from_str(&text).unwrap()
}

/// Compares `text` to the golden `name` of `tests/goldens/info/`.
#[allow(dead_code)]
fn golden(name: &str, text: &str) {
    flm_core::dump::golden::assert_text_golden(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/info").join(name),
        text,
    );
}

/// The paths of the pages of an output of `--json`.
fn paths(output: &Value) -> Vec<String> {
    output["pages"].as_array().unwrap().iter().map(|page| page["path"].as_str().unwrap().to_string()).collect()
}

/// The page `/` of the program's default environment, in which the bundled MathJax
/// extension processes mathematics.
#[cfg(all(feature = "bundled-extensions", feature = "js"))]
#[test]
fn the_root_page_is_a_golden() {
    let dir = folder("root", false);
    let text = shown(&dir, &[]);
    assert!(text.starts_with("The FLM environment\n"), "{text}");
    golden("root.txt", &text);
}

/// Three pages as text: a feature, a macro, the index of the namespaces.
#[cfg(feature = "bundled-extensions")]
#[test]
fn text_pages_are_goldens() {
    let dir = folder("text", false);
    for (path, name) in [
        ("features/headings", "features-headings.txt"),
        ("features/headings/macros/section", "features-headings-macros-section.txt"),
        ("namespaces/", "namespaces.txt"),
    ] {
        golden(name, &shown(&dir, &[path]));
    }
}

/// `-O text.width=40` shapes the environment described, not the pages: the page
/// `settings/text` shows 40 as the value in effect, written by the option, and the text of a
/// page is the same with the option as without it.
#[cfg(feature = "bundled-extensions")]
#[test]
fn an_option_shapes_the_environment_not_the_pages() {
    let dir = folder("width", false);
    let output = json(&dir, &["--math", "passthrough", "-O", "text.width=40", "settings/text"]);
    let keys = output["pages"][0]["body"]["keys"].as_array().unwrap();
    let width = keys.iter().find(|key| key["name"] == "width").unwrap();
    assert_eq!(width["in_effect"]["value"], 40, "{width}");
    assert_eq!(width["in_effect"]["origin"]["source"], "command-line");
    assert_eq!(width["in_effect"]["origin"]["text"], "the option `-O text.width=40`");
    let text = shown(&dir, &["--math", "passthrough", "-O", "text.width=40", "settings/text"]);
    assert!(text.contains("\nWritten in: the option `-O text.width=40`\n"), "{text}");
    let page = ["--math", "passthrough", "features/headings/macros/section"];
    assert_eq!(shown(&dir, &[&["-O", "text.width=40"], &page[..]].concat()), shown(&dir, &page));
}

/// Under `-r`, each page is followed by every page below it; two pages are separated by one
/// blank line.
#[cfg(feature = "bundled-extensions")]
#[test]
fn recursive_text_shows_the_pages_below() {
    let dir = folder("recursive-text", false);
    let text = shown(&dir, &["--math", "passthrough", "-r", "features/headings/macros"]);
    assert!(text.starts_with("Macros of feature headings\n"), "{text}");
    for title in ["Macro \\section\n", "Macro \\subsection\n", "Macro \\paragraph\n"] {
        assert!(text.contains(&format!("\n\n{title}")), "{title:?} in {text}");
    }
    assert!(!text.contains("\n\n\n"), "one blank line between pages:\n{text}");
}

/// `--json` prints the header and the page; the children of a page are entries, and full
/// pages under `-r`.
#[test]
fn json_holds_the_header_and_the_pages() {
    let dir = folder("json", false);
    let output = json(&dir, &["--math", "passthrough", "features/headings"]);
    assert_eq!(output["meta"]["kind"], "info");
    assert_eq!(output["meta"]["format"], 1);
    assert_eq!(output["meta"]["flm"], env!("CARGO_PKG_VERSION"));
    let pages = output["pages"].as_array().unwrap();
    assert_eq!(pages.len(), 1);
    let page = &pages[0];
    assert_eq!(page["path"], "features/headings");
    assert_eq!(page["title"], "Feature headings");
    let children = page["children"].as_array().unwrap();
    assert!(!children.is_empty());
    assert!(children.iter().all(|child| child.get("body").is_none() && child.get("children").is_none()));
    assert!(children.iter().any(|child| child["path"] == "features/headings/macros"));

    let output = json(&dir, &["--math", "passthrough", "-r", "features/headings"]);
    let child = &output["pages"][0]["children"][0];
    assert!(child["kind"].is_string() && child["children"].is_array(), "a full page: {child}");
    let macros = output["pages"][0]["children"].as_array().unwrap().iter().find(|child| child["path"] == "features/headings/macros").unwrap();
    assert!(macros["children"].as_array().unwrap().iter().any(|page| page["path"] == "features/headings/macros/section" && page["body"].is_object()));
}

/// Two paths give two pages, in order; a name that is no path is looked up: a macro with its
/// backslash, a stage by its dotted name.
#[test]
fn paths_and_names_give_their_pages() {
    let dir = folder("lookup", false);
    let output = json(&dir, &["--math", "passthrough", "settings/text", "features/headings"]);
    assert_eq!(paths(&output), ["settings/text", "features/headings"]);
    let output = json(&dir, &["--math", "passthrough", "\\section", "flm.refs.resolve"]);
    assert_eq!(paths(&output), ["features/headings/macros/section", "namespaces/stage/flm.refs.resolve"]);
}

/// A path that is no page and no name exits with status 2, prints nothing on standard output
/// and gives the nearest pages, when some are near: none for a path unlike any page's.
#[test]
fn a_path_with_no_page_exits_with_status_2() {
    let dir = folder("missing", false);
    let output = flm(&dir, &["--math", "passthrough", "nothing/here"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(stderr, "error: no page or name `nothing/here`\n");
    let output = flm(&dir, &["--math", "passthrough", "features/heading/macros/section"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("`features/headings/macros/section`"), "{stderr}");
}

/// With `-d`, the environment is the document's: the package of the data extension it uses
/// is described, and the program's pages show the extension found and its folder.
#[test]
fn a_document_brings_its_extension_into_the_environment() {
    let dir = folder("document", true);
    let output = json(&dir, &["--math", "passthrough", "-d", "doc/paper.flm", "packages/", "program/extensions", "program/paths"]);
    let pages = output["pages"].as_array().unwrap();
    let packages: Vec<&str> = pages[0]["children"].as_array().unwrap().iter().map(|child| child["path"].as_str().unwrap()).collect();
    let package = packages.iter().find(|path| path.contains("margins")).unwrap_or_else(|| panic!("{packages:?}"));
    let page = json(&dir, &["--math", "passthrough", "-d", "doc/paper.flm", package]);
    let body = &page["pages"][0]["body"];
    assert_eq!(body["origin"]["kind"], "extension", "{body}");
    assert_eq!(body["origin"]["description"], "The margins of the text.");
    assert_eq!(pages[1]["body"]["found"], serde_json::json!(["margins"]));
    let search_path = pages[2]["body"]["search_path"].as_array().unwrap();
    assert_eq!(search_path[0]["path"], Path::new("doc").join("_flmextensions").display().to_string());
    assert_eq!(search_path[0]["origin"], "beside the document");
    assert_eq!(search_path[0]["exists"], true);
    // Without `-d`, the extension is not in use.
    let output = json(&dir, &["--math", "passthrough", "packages/"]);
    assert!(!output["pages"][0]["children"].as_array().unwrap().iter().any(|child| child["path"].as_str().unwrap().contains("margins")));
}

/// The page `program/paths` names the four kinds of folder: the folders of the search path
/// of extensions (each with where it comes from: beside the document, `FLMPATH`, the user's
/// folder, or `--extensions-dir`), the folder of the citation cache, and the image folders
/// with their rule, from the setting `graphics-files.search_path` in effect.
#[test]
fn the_program_s_folders_are_named() {
    let dir = folder("paths", true);
    std::fs::create_dir_all(dir.join("listed")).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_flm"))
        .args(["info", "--json", "--math", "passthrough", "-d", "doc/paper.flm", "program/paths"])
        .current_dir(&dir)
        .env("HOME", &dir)
        .env("FLMPATH", dir.join("listed"))
        .env_remove("USERPROFILE")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let output: Value = serde_json::from_slice(&output.stdout).unwrap();
    let body = &output["pages"][0]["body"];
    let origins: Vec<&str> = body["search_path"].as_array().unwrap().iter().map(|folder| folder["origin"].as_str().unwrap()).collect();
    assert_eq!(origins, ["beside the document", "FLMPATH", "the user's folder"]);
    assert_eq!(body["search_path"][1]["exists"], true);
    assert!(body["search_path"][2].get("exists").is_none(), "the user's folder does not exist");
    assert_eq!(body["cache_folder"]["path"], "doc");
    assert_eq!(body["cache_folder"]["origin"], "the document's folder");
    assert_eq!(body["image_folders"]["folders"], serde_json::json!(["figures", "."]));
    assert!(body["image_folders"]["rule"].as_str().unwrap().contains("relative to the folder of the file"));

    std::fs::create_dir_all(dir.join("mine")).unwrap();
    let output = json(&dir, &["--math", "passthrough", "--extensions-dir", "mine", "program/paths"]);
    let body = &output["pages"][0]["body"];
    assert_eq!(body["search_path"][0]["path"], "mine");
    assert_eq!(body["search_path"][0]["origin"], "--extensions-dir");
    assert_eq!(body["cache_folder"]["origin"], "the current folder");
    assert_eq!(body["image_folders"]["folders"], serde_json::json!(["."]));
}

/// The profile in view decides the plan that the workflow pages show: `-p text` shows the
/// plan on `workflows/text`, which the default profile `html` does not.
#[test]
fn the_profile_in_view_decides_the_plan_shown() {
    let dir = folder("profile", false);
    let output = json(&dir, &["--math", "passthrough", "-p", "text", "workflows/text"]);
    assert!(output["pages"][0]["body"]["plan"].is_object(), "{output}");
    let output = json(&dir, &["--math", "passthrough", "workflows/text"]);
    assert!(output["pages"][0]["body"].get("plan").is_none());
}

/// A package that cannot be found stops the preparation: status 2, the problem on standard
/// error.
#[test]
fn an_environment_that_cannot_be_prepared_exits_with_status_2() {
    let dir = folder("unprepared", false);
    let output = flm(&dir, &["--math", "passthrough", "--use", "nosuchpackage"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8(output.stderr).unwrap().contains("nosuchpackage"));
}

/// The diagnostics that the preparation of the environment reports without stopping are
/// printed on standard error, as `flm check` prints them, before the pages; the status
/// stays 0. A document that switches off a feature that no package has gets a warning.
#[test]
fn a_warning_of_the_preparation_is_printed() {
    let dir = folder("warning", true);
    let output = flm(&dir, &["--json", "--math", "passthrough", "-d", "doc/switched-off.flm", "settings/"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(
        stderr.starts_with(
            "warning: `nosuchfeature` is switched off, but no package in use has a feature `nosuchfeature`\n  at "
        ),
        "{stderr}"
    );
    assert!(stderr.ends_with("0 errors, 1 warning\n"), "{stderr}");
    let pages: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(paths(&pages), ["settings/"]);
}

/// A documentation text of an extension that is not valid FLM is shown as plain text, after
/// a warning that names where it was written and the page, shows the text as written and
/// gives the problems found; the status stays 0. Here two texts of two paragraphs written as
/// data by the package of a data extension, each with a `%` and a brace that is not closed
/// in its first paragraph: a workflow's and a user definition's, whose warnings name the
/// package and the place in its file, both on the page that shows the text whole and on the
/// pages that show its summary alone.
#[cfg(feature = "bundled-extensions")]
#[test]
fn a_documentation_text_that_does_not_parse_is_shown_as_plain_text() {
    let dir = folder("unparsed", true);
    let file = Path::new("doc").join("draft").join("_flmextensions").join("drafts.yaml").display().to_string();
    let output = flm(&dir, &["--math", "passthrough", "-d", "doc/draft/draft.flm", "workflows/draft", "workflows/", "/"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("Workflow draft\n"), "{text}");
    assert!(text.contains("\nThe text output, 50% narrower, with {an unbalanced brace.\n"), "{text}");
    assert!(text.contains("\nIt extends `text`.\n"), "{text}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    let workflow = format!("warning: the documentation text of the workflow `draft` (package `{file}#_`, {file}:12:14) on the page");
    let summary = "must be valid FLM:\n  | The text output, 50% narrower, with {an unbalanced brace.\n";
    assert!(
        stderr.starts_with(&format!("{workflow} `workflows/draft` {summary}  |\n  | It extends `text`.\n  error: ")),
        "{stderr}"
    );
    assert!(stderr.contains("\n    at text:1:20\n"), "{stderr}");
    for page in ["workflows/", "/"] {
        assert!(stderr.contains(&format!("{workflow} `{page}` {summary}  error: ")), "{page}: {stderr}");
    }
    assert_eq!(stderr.matches("warning: ").count(), 3, "{stderr}");

    let pages = ["features/userdefs/macros/project", "features/userdefs/macros"];
    let output = flm(&dir, &["--math", "passthrough", "-d", "doc/draft/draft.flm", pages[0], pages[1]]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.starts_with("Macro \\project\n"), "{text}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    let entry = format!(
        "warning: the documentation text of the entry `project` of the setting `userdefs.macros` (package `{file}#_`, \
         {file}:16:9) on the page"
    );
    let summary = "must be valid FLM:\n  | The project, 10% {done.\n";
    assert!(stderr.starts_with(&format!("{entry} `{}` {summary}  |\n  | It is written as text.\n  error: ", pages[0])), "{stderr}");
    assert!(stderr.contains(&format!("{entry} `{}` {summary}  error: ", pages[1])), "{stderr}");
    assert_eq!(stderr.matches("warning: ").count(), 2, "{stderr}");
}

/// A documentation text that a JavaScript extension's feature declares and that is not valid
/// FLM: the warning names the feature and its package, and shows the text exactly as
/// written, its backslash included. The macro's text fails in its first paragraph too, which
/// the page also shows on its own; one warning covers both. On the list of features and on
/// the list of the names of the namespace `feature`, the feature's summary is named as the
/// feature's.
#[cfg(all(feature = "bundled-extensions", feature = "js"))]
#[test]
fn the_warning_names_the_feature_and_the_package() {
    let dir = folder("unparsed-feature", true);
    let manifest = Path::new("doc").join("loud").join("_flmextensions").join("loud").join("flmextension.yaml");
    let package = format!("{}#_", manifest.display());
    let output = flm(&dir, &["--math", "passthrough", "-d", "doc/loud/loud.flm", "features/loud/macros/shout"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\nLike `\\textbf`, but 100% {louder.\n\nIts argument is the text.\n"), "{text}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert_eq!(
        stderr.lines().take(4).collect::<Vec<_>>(),
        [
            format!(
                "warning: the documentation text declared by the feature `loud` (package `{package}`) on the page \
                 `features/loud/macros/shout` must be valid FLM:"
            )
            .as_str(),
            "  | Like `\\textbf`, but 100% {louder.",
            "  |",
            "  | Its argument is the text.",
        ],
        "{stderr}"
    );
    assert!(stderr.contains("\n    at text:1:24\n"), "{stderr}");
    assert_eq!(stderr.matches("warning: ").count(), 1, "{stderr}");

    let output = flm(&dir, &["--math", "passthrough", "-d", "doc/loud/loud.flm", "features/", "namespaces/feature"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    for page in ["features/", "namespaces/feature"] {
        assert!(
            stderr.contains(&format!(
                "warning: the documentation text declared by the feature `loud` (package `{package}`) on the page \
                 `{page}` must be valid FLM:\n  | Loud text, 50% {{louder.\n"
            )),
            "{page}: {stderr}"
        );
    }
    assert_eq!(stderr.matches("warning: ").count(), 2, "{stderr}");
}

/// Runs `flm render` with `args` in `dir`, as [`flm`] runs `flm info`.
#[cfg(feature = "bundled-extensions")]
fn flm_render(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flm"))
        .arg("render")
        .args(args)
        .current_dir(dir)
        .env("HOME", dir)
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE")
        .env_remove("COLUMNS")
        .output()
        .unwrap()
}

/// `--write-flm FILE` writes the page's source in FLM to FILE, ending with one newline, and
/// prints nothing; `flm render FILE -p text` prints the text that `flm info` prints for the
/// page. `--write-flm -` prints the same source on standard output.
#[cfg(feature = "bundled-extensions")]
#[test]
fn a_page_written_as_flm_renders_as_its_text() {
    let dir = folder("write-flm", false);
    let page = ["--math", "passthrough", "features/headings/macros/section"];
    let output = flm(&dir, &[&["--write-flm", "section.flm"], &page[..]].concat());
    assert!(output.status.success() && output.stdout.is_empty() && output.stderr.is_empty(), "{output:?}");
    let source = std::fs::read_to_string(dir.join("section.flm")).unwrap();
    assert!(
        source.starts_with("---\ntitle: \"Macro \\\\textbackslash{}section\"\nflm:\n  use: [flmdoc]\n  math: false\n---\n"),
        "{source}"
    );
    assert!(source.ends_with("\n") && !source.ends_with("\n\n"), "one final newline");
    assert_eq!(shown(&dir, &[&["--write-flm", "-"], &page[..]].concat()), source);
    let rendered = flm_render(&dir, &["section.flm", "-p", "text"]);
    assert!(rendered.status.success() && rendered.stderr.is_empty(), "{rendered:?}");
    assert_eq!(String::from_utf8(rendered.stdout).unwrap(), shown(&dir, &page));
}

/// `--write-flm` writes one page: two paths that give two pages exit with status 2 and
/// write nothing; `-r` and `--json` are refused with it.
#[test]
fn write_flm_writes_exactly_one_page() {
    let dir = folder("write-flm-one", false);
    let output = flm(&dir, &["--math", "passthrough", "--write-flm", "out.flm", "settings/text", "features/headings"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "error: the paths given must choose exactly one page with `--write-flm`; they choose 2: `settings/text`, \
         `features/headings`\n"
    );
    assert!(!dir.join("out.flm").exists());
    for option in ["-r", "--json"] {
        let output = flm(&dir, &["--math", "passthrough", "--write-flm", "out.flm", option, "features/headings"]);
        assert_eq!(output.status.code(), Some(2), "{option}");
        assert!(String::from_utf8(output.stderr).unwrap().contains("cannot be used with"), "{option}");
        assert!(!dir.join("out.flm").exists());
    }
}

/// A file that `--write-flm` cannot write (its folder does not exist) exits with status 1.
#[cfg(feature = "bundled-extensions")]
#[test]
fn a_file_that_cannot_be_written_exits_with_status_1() {
    let dir = folder("write-flm-unwritable", false);
    let output = flm(&dir, &["--math", "passthrough", "--write-flm", "nofolder/out.flm", "features/headings"]);
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with("error: the file `nofolder/out.flm` cannot be written: "), "{stderr}");
}

/// A documentation text that does not parse is written into the FLM document as plain text,
/// after the warning that the text output gives, so that `flm render` renders the document.
#[cfg(feature = "bundled-extensions")]
#[test]
fn a_documentation_text_that_does_not_parse_is_written_as_plain_text() {
    let dir = folder("write-flm-unparsed", true);
    let output = flm(&dir, &["--math", "passthrough", "-d", "doc/draft/draft.flm", "--write-flm", "draft.flm", "workflows/draft"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with("warning: the documentation text of the workflow `draft` "), "{stderr}");
    assert_eq!(stderr.matches("warning: ").count(), 1, "{stderr}");
    let source = std::fs::read_to_string(dir.join("draft.flm")).unwrap();
    assert!(source.contains("\\docsummary{The text output, 50\\% narrower, with \\{an unbalanced brace.}"), "{source}");
    let rendered = flm_render(&dir, &["draft.flm", "-p", "text"]);
    assert!(rendered.status.success() && rendered.stderr.is_empty(), "{rendered:?}");
    let text = String::from_utf8(rendered.stdout).unwrap();
    assert!(text.contains("\nThe text output, 50% narrower, with {an unbalanced brace.\n"), "{text}");
}
