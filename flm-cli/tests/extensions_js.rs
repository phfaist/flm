//! Extensions written in JavaScript, found and run by the binary (cargo feature `js`): the
//! three cases of the extension layer's acceptance and the first case of step E4
//! (`exercises`, a numbered, referenceable environment) through `flm render` and `flm schema`,
//! and a backend written in JavaScript, `typst`, rendered through its output.
//!
//! The documents are under `tests/documents/extensions/`; the extensions written in
//! JavaScript are not copied there: each test copies `wordcount`, `twopack`, `typst`, `bibfile`,
//! `tinytpl` and `exercises` from `flm-ext/tests/js/` into `_flmextensions/` of a fresh folder
//! beside the documents, so one copy of each script serves the tests of `flm-ext` and these.
//! Beside the documents, `bib.json` holds the entries `bibfile.flm` cites, and `logo.png` is the
//! banner `tiny.flm` names. The folder
//! `_flmextensions/` of the documents adds `counting`, an extension of the engine `data`
//! that defines the workflow `counted`, `house` and `margins`, which `data.flm` uses, and
//! `runsh`, a script whose stage runs `sh` through the host service `programs`.
//! The stage of `wordcount` that counts the words provides a capability that joins the
//! collection `flm.validated`, so the plain workflows (`html`, `text`) run the count and
//! report its Notes; `counted` (the HTML page, demanding the table `wordcount.words`) stays
//! as the example of a workflow that demands a table.

#![cfg(feature = "js")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flm_core::dump::golden::assert_text_golden;

#[path = "../../flm-core/tests/support/json_schema.rs"]
mod validator;

/// The folder of the documents of these tests.
fn documents() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/extensions")
}

/// The folder of the extensions written in JavaScript of `flm-ext`'s tests.
fn scripts() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../flm-ext/tests/js")
}

fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/extensions").join(name)
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

/// Replaces `old` by `new` in the file `path`, which must hold `old`.
fn edit(path: &Path, old: &str, new: &str) {
    let text = std::fs::read_to_string(path).unwrap();
    assert!(text.contains(old), "{}: {old}", path.display());
    std::fs::write(path, text.replace(old, new)).unwrap();
}

/// A fresh folder `name` under the build folder, holding the documents of these tests and
/// their extensions, `wordcount`, `twopack`, `typst`, `bibfile`, `tinytpl` and `exercises`
/// copied from `flm-ext`'s tests.
fn folder(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-extensions-js-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    copy_tree(&documents(), &path);
    for extension in ["wordcount", "twopack", "typst", "bibfile", "tinytpl", "exercises"] {
        copy_tree(&scripts().join(extension), &path.join("_flmextensions").join(extension));
    }
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

/// The standard output of a run that succeeded and reported nothing.
fn quietly(cwd: &Path, args: &[&str]) -> String {
    let output = flm(cwd, args);
    let reported = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(0), "{args:?}: {reported}");
    assert!(reported.is_empty(), "{args:?}: {reported}");
    String::from_utf8(output.stdout).unwrap()
}

/// The standard output of a run that succeeded, and its standard error.
fn noted(cwd: &Path, args: &[&str]) -> (String, String) {
    let output = flm(cwd, args);
    let reported = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(0), "{args:?}: {reported}");
    (String::from_utf8(output.stdout).unwrap(), reported)
}

/// The standard error of a run that ended with status `code`.
fn reported(output: &Output, code: i32) -> String {
    let reported = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(code), "{reported}");
    reported
}

// --- case 1: the extension `wordcount` installed beside the document ----------------------------

/// The count stage joins `flm.validated`, so the document's `report_counts` makes its Note
/// appear under the plain workflows too.
#[test]
fn case_one_renders_from_flmextensions_with_its_settings() {
    let dir = folder("case-one");
    let (text, notes) = noted(&dir, &["render", "-p", "text", "wordcount.flm"]);
    // The configuration `shout_suffix` reaches `register`; `\todo` writes nothing.
    assert!(text.contains("LISTEN!") && text.contains("QUIETLY!") && !text.contains("check the numbers"), "{text}");
    assert_text_golden(golden("wordcount.txt"), &text);
    assert!(notes.contains("note: the tree `tree#0` holds 9 words of at least 4 characters"), "{notes}");
    let (html, notes) = noted(&dir, &["render", "-f", "html", "-t", "none", "wordcount.flm"]);
    assert!(html.contains("LISTEN!") && !html.contains("check the numbers"), "{html}");
    assert_text_golden(golden("wordcount.html"), &html);
    assert!(notes.contains("note: the tree `tree#0` holds 9 words of at least 4 characters"), "{notes}");
}

/// The Notes of the count under the plain `html` workflow, which demands no table of the
/// extension, are those of the workflow `counted`, which demands one.
#[test]
fn case_one_reports_its_count_under_the_plain_html_workflow() {
    let dir = folder("case-one-plain-notes");
    let output = flm(&dir, &["render", "-p", "html", "wordcount.flm"]);
    let notes = reported(&output, 0);
    assert_text_golden(golden("wordcount-notes.txt"), &notes);
}

/// `flm render` prints the Notes of a run on standard error, as it prints the Errors and
/// the Warnings: the count is read there.
#[test]
fn case_one_reports_its_count_with_the_front_matter_s_min_length_and_o_changes_it() {
    let dir = folder("case-one-notes");
    let output = flm(&dir, &["render", "-p", "counted", "wordcount.flm"]);
    let notes = reported(&output, 0);
    assert!(notes.contains("note: the tree `tree#0` holds 9 words of at least 4 characters"), "{notes}");
    assert_text_golden(golden("wordcount-notes.txt"), &notes);
    let output = flm(&dir, &["render", "-p", "counted", "-O", "wordcount.min_length=2", "wordcount.flm"]);
    let notes = reported(&output, 0);
    assert!(notes.contains("note: the tree `tree#0` holds 12 words of at least 2 characters"), "{notes}");
    assert_text_golden(golden("wordcount-notes-o.txt"), &notes);
    // Without `report_counts`, a document-bound setting, the stage reports nothing.
    edit(&dir.join("wordcount.flm"), "report_counts: true", "report_counts: false");
    quietly(&dir, &["render", "-p", "counted", "wordcount.flm"]);
}

#[test]
fn case_one_s_settings_are_refused_where_they_are_written() {
    let dir = folder("case-one-refused");
    edit(&dir.join("wordcount.flm"), "min_length: 4", "min_lenght: 4");
    let output = flm(&dir, &["render", "-p", "text", "wordcount.flm"]);
    let reported = reported(&output, 2);
    assert!(
        reported.contains("error: the feature `wordcount` has no setting `min_lenght`\n  at wordcount.flm:8:19"),
        "{reported}"
    );
}

// --- E4's first case: the extension `exercises` ---------------------------------------------------

/// `exercises` installed beside the document, rendered to text, HTML and the recomposed
/// LaTeX: the numbers, the heading lines, the hint inside an exercise, the references.
#[test]
fn exercises_render_from_flmextensions_to_text_html_and_latex() {
    let dir = folder("exercises");
    let text = quietly(&dir, &["render", "-p", "text", "exercises.flm"]);
    assert!(text.contains("Exercise 1 (A title)") && text.contains("Hint: Think of the first case."), "{text}");
    assert_text_golden(golden("exercises.txt"), &text);
    let html = quietly(&dir, &["render", "-f", "html", "-t", "none", "exercises.flm"]);
    assert!(html.contains(r#"id="ex-first""#) && html.contains(r##"href="#ex-first""##), "{html}");
    assert_text_golden(golden("exercises.html"), &html);
    quietly(&dir, &["render", "-p", "latex-recomposed", "exercises.flm", "-o", "out/"]);
    let latex = std::fs::read_to_string(dir.join("out/index.tex")).unwrap();
    assert!(latex.contains("\\begin{exercise}[A title]\\flmLabels{\\label{ref1}}"), "{latex}");
    assert_text_golden(golden("exercises.tex"), &latex);
    // The style file beside it holds the piece the LaTeX rule requires.
    let style = std::fs::read_dir(dir.join("out"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|extension| extension == "sty"))
        .expect("a style file");
    let style = std::fs::read_to_string(style).unwrap();
    assert!(style.contains("\\newtheorem{exercise}{Exercise}"), "{style}");
}

// --- case 3: the extension `twopack`, two packages ----------------------------------------------

#[test]
fn case_three_gives_both_packages_of_one_extension() {
    let dir = folder("case-three");
    let text = quietly(&dir, &["render", "-p", "text", "twopack.flm"]);
    // `\tpqed` is the package `proofs`'s; `\tpmark` the package `_`'s, which `proofs`
    // brings through `.#_`.
    assert!(text.contains("[mark]") && text.contains("[qed]"), "{text}");
    assert_text_golden(golden("twopack.txt"), &text);
}

#[test]
fn case_three_with_a_name_outside_the_stem_fails_the_build() {
    let dir = folder("case-three-stem");
    edit(&dir.join("_flmextensions/twopack/main.js"), r#"b.table("twopack.proofs""#, r#"b.table("proofs.count""#);
    let output = flm(&dir, &["render", "-p", "text", "twopack.flm"]);
    let reported = reported(&output, 2);
    assert!(
        reported.contains(
            "the table `proofs.count` declared by the package `proofs` of the extension `twopack` must begin with `twopack.`"
        ),
        "{reported}"
    );
    assert!(reported.contains("flm.ext.name-outside-extension"), "{reported}");
    assert!(!reported.contains("flm.ext.handler-failed"), "{reported}");
    assert!(output.stdout.is_empty());
}

/// The document uses `wordcount` directly and through `counting`: the failure is reported
/// at each of the two entries.
#[test]
fn a_script_s_syntax_error_is_reported_at_the_use_entry() {
    let dir = folder("syntax-error");
    let main = dir.join("_flmextensions/wordcount/main.js");
    let mut text = std::fs::read_to_string(&main).unwrap();
    let line = text.lines().count() + 1;
    text.push_str("function (\n");
    std::fs::write(&main, text).unwrap();
    let output = flm(&dir, &["render", "-p", "text", "wordcount.flm"]);
    let reported = reported(&output, 2);
    let message = format!("error: the modules of the extension `wordcount` must be valid JavaScript: function name expected (at main.js:{line}:10)");
    assert!(reported.contains(&format!("{message}\n  at wordcount.flm:4:9")), "{reported}");
    assert!(reported.contains(&format!("{message}\n  at wordcount.flm:4:20")), "{reported}");
}

// --- a backend written in JavaScript: the extension `typst` and its output -----------------------

/// The workflow `typst` of the extension, chosen by `-p`: the one artifact of its output
/// `typst`, the document as Typst markup, printed on standard output.
#[test]
fn a_scripted_backend_renders_its_output_with_w() {
    let dir = folder("typst-f");
    let typst = quietly(&dir, &["render", "-p", "typst", "typst.flm"]);
    // The fixture's own element, which reaches this backend alone, from its payload.
    assert!(typst.contains("#rect[*warning:* A callout with _content_.]"), "{typst}");
    assert_text_golden(golden("typst.typ"), &typst);
}

/// The output `typst` selected by `-f` within the workflow `typst` that the extension's `flm`
/// block declares: the same bytes as the workflow without `-f`, since it delivers that output
/// alone.
#[test]
fn the_extension_s_output_selected_with_f_gives_the_same_bytes() {
    let dir = folder("typst-w");
    let by_output = quietly(&dir, &["render", "-p", "typst", "-f", "typst", "typst.flm"]);
    let by_workflow = quietly(&dir, &["render", "-p", "typst", "typst.flm"]);
    assert_eq!(by_workflow, by_output);
}

/// With `-o`, the entry artifact is written to the file named, `out/paper.typ`.
#[test]
fn a_scripted_backend_s_artifact_is_written_with_o() {
    let dir = folder("typst-o");
    let printed = quietly(&dir, &["render", "-p", "typst", "-o", "out/paper.typ", "typst.flm"]);
    assert!(printed.is_empty(), "{printed}");
    let written = std::fs::read_to_string(dir.join("out/paper.typ")).expect("`out/paper.typ` is written");
    assert_text_golden(golden("typst.typ"), &written);
}

/// A format that the workflow `typst` does not provide fails with status 2, and the message
/// lists the formats it provides, `typst`.
#[test]
fn an_output_the_workflow_does_not_deliver_lists_the_ones_it_does() {
    let dir = folder("typst-unknown");
    let output = flm(&dir, &["render", "-p", "typst", "-f", "typs", "typst.flm"]);
    let reported = reported(&output, 2);
    let line = reported.lines().find(|line| line.contains("`typs`")).unwrap_or_default();
    assert_eq!(
        line,
        "error: the workflow `typst` does not provide the format `typs`; it provides `typst`",
        "{reported}"
    );
    assert!(output.stdout.is_empty());
}

// --- bibfile: a citation provider reading a file beside the document --------------------------

#[test]
fn bibfile_lists_the_three_works_of_the_file_beside_the_document() {
    let dir = folder("bibfile");
    let text = quietly(&dir, &["render", "-p", "text", "bibfile.flm"]);
    // `Shor95` and `shor95` are one work, by the identity the provider answers.
    assert!(text.contains("The TeXbook") && text.contains("LaTeX: A Document Preparation System"), "{text}");
    assert_eq!(text.matches("P. W. Shor").count(), 1, "{text}");
    assert_text_golden(golden("bibfile.txt"), &text);
    let html = quietly(&dir, &["render", "-f", "html", "-t", "none", "bibfile.flm"]);
    assert_text_golden(golden("bibfile.html"), &html);
}

/// The workflow `noted`, which the extension's own `flm` block declares, runs the stage that
/// reports the notes the package `extra` contributes to the slot the package `_` declares.
#[test]
fn bibfile_s_workflow_noted_reports_the_note_of_the_package_extra() {
    let dir = folder("bibfile-noted");
    edit(&dir.join("bibfile.flm"), "use: [bibfile]", "use: [bibfile, bibfile#extra]");
    let output = flm(&dir, &["render", "-p", "noted", "-f", "text", "bibfile.flm"]);
    let notes = reported(&output, 0);
    assert!(notes.contains("note: a package notes: the extra package was used"), "{notes}");
    assert_text_golden(golden("bibfile-notes.txt"), &notes);
}

/// Without `bib.json`, the provider answers its own text for every key, reported at each
/// citation; the run reports errors in the document (status 1).
#[test]
fn bibfile_without_its_file_reports_each_citation() {
    let dir = folder("bibfile-missing");
    std::fs::remove_file(dir.join("bib.json")).unwrap();
    let output = flm(&dir, &["render", "-p", "text", "bibfile.flm"]);
    let reported = reported(&output, 1);
    let why = "the file `bib.json` beside the document must hold the bibliography entries";
    for (key, column) in [("knuth84", 5), ("lamport94", 24), ("Shor95", 54), ("shor95", 74)] {
        for message in [
            format!("error: the bibliography entry of the key `{key}` could not be retrieved: {why}\n  at bibfile.flm:6:{column}\n"),
            format!("error: the cited key `{key}` must have a bibliography entry; retrieving it failed: {why}\n  at bibfile.flm:6:{column}\n"),
        ] {
            assert!(reported.contains(&message), "{message}\n---\n{reported}");
        }
    }
}

// --- tinytpl: a compose template written as a module ------------------------------------------

#[test]
fn tiny_writes_the_page_its_stylesheet_and_the_banner_beside_it() {
    let dir = folder("tiny");
    let output = flm(&dir, &["render", "-f", "html", "-t", "tiny", "-o", "out/tiny.html", "tiny.flm"]);
    let reported = reported(&output, 0);
    assert!(reported.is_empty(), "{reported}");
    let out = dir.join("out");
    let mut names: Vec<String> = std::fs::read_dir(&out)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort();
    assert_eq!(names, ["tiny-banner.png", "tiny-style.css", "tiny.html"]);
    let page = std::fs::read_to_string(out.join("tiny.html")).unwrap();
    for part in ["<meta charset=\"utf-8\">", "<h1>Notes</h1>", "href=\"tiny-style.css\"", "<img src=\"tiny-banner.png\" alt=\"\">"] {
        assert!(page.contains(part), "{part}\n---\n{page}");
    }
    assert_text_golden(golden("tiny.html"), &page);
    assert_eq!(std::fs::read_to_string(out.join("tiny-style.css")).unwrap(), "h1 { color: teal; }\n");
    assert_eq!(std::fs::read(out.join("tiny-banner.png")).unwrap(), std::fs::read(dir.join("logo.png")).unwrap());
}

// --- runsh: a stage running a program through the host service `programs` -------------------

/// The `flm` program offers the service: the stage finds `sh`, runs it in a temporary
/// folder and reports what it collected.
#[cfg(unix)]
#[test]
fn runsh_runs_sh_and_reports_the_files_it_collected() {
    let dir = folder("runsh");
    let output = flm(&dir, &["render", "-p", "ran", "-f", "text", "runsh.flm"]);
    let notes = reported(&output, 0);
    assert!(notes.contains(r#"out.txt {"text":"WRITTEN BY THE EXTENSION"}"#), "{notes}");
    assert!(notes.contains(r#"out.bin {"base64":"AQL/"}"#), "{notes}");
    assert_text_golden(golden("runsh-notes.txt"), &notes);
}

/// A program embedding the library offers the service only when it asks: the same document,
/// prepared by a request that does not, is refused by the loader.
#[test]
fn runsh_is_refused_by_a_library_request_that_does_not_offer_programs() {
    use flm_main::packages::ExtensionFolder;
    use flm_main::session::{Input, Request, prepare};

    let dir = folder("runsh-library");
    let mut request = Request::new(Input::File(dir.join("runsh.flm")));
    request.extension_folders = vec![ExtensionFolder { path: dir.join("_flmextensions"), required: false }];
    let error = prepare(request, std::sync::Arc::new(flm_main::files::DiskFiles), &[]).err().expect("refused");
    let details: Vec<&str> = error.problems.iter().map(|problem| problem.detail.as_str()).collect();
    assert_eq!(
        details,
        ["the package `_` of the extension `runsh` declares the host service `programs` (`host`), which this program \
          does not offer; it offers `files`"],
        "{error:?}"
    );
}

// --- a manifest's problem placed at its line ---------------------------------------------------

/// A problem the loader reports by key path is placed at the key's value in the manifest.
#[test]
fn a_manifest_problem_is_placed_at_its_line_and_column() {
    let dir = folder("manifest-place");
    edit(&dir.join("_flmextensions/bibfile/flmextension.yaml"), "    main: main.js\n", "    main: main.js\n    mian: other.js\n");
    let output = flm(&dir, &["render", "-p", "text", "bibfile.flm"]);
    let reported = reported(&output, 2);
    let message = "error: `extension.js.mian` is not a key of `extension.js` of the extension `bibfile`; the keys are \
                   `main` and `limits`\n  at _flmextensions/bibfile/flmextension.yaml:6:11";
    assert!(reported.contains(message), "{reported}");
}

// --- flm schema with a scripted feature --------------------------------------------------------

/// The schema `flm schema --of WHAT ARGS…` prints in `dir`.
fn printed_schema(dir: &Path, what: &str, args: &[&str]) -> serde_json::Value {
    let output = flm(dir, &[&["schema", "--of", what][..], args].concat());
    assert!(output.status.success(), "{output:?}");
    serde_json::from_slice(&output.stdout).expect("JSON")
}

#[test]
fn schema_of_a_front_matter_with_the_document_lists_the_scripted_feature() {
    let dir = folder("schema-front-matter");
    let schema = printed_schema(&dir, "front-matter", &["wordcount.flm"]);
    let features = &schema["properties"]["flm"]["properties"]["features"]["properties"];
    assert!(features["wordcount"].is_object(), "{features}");
    let settings = &schema["$defs"]["wordcount"]["properties"];
    let bound: Vec<(&str, &str)> = ["shout_suffix", "min_length", "report_counts"]
        .iter()
        .map(|key| (*key, settings[key]["x-flm-bind"].as_str().unwrap_or("absent")))
        .collect();
    assert_eq!(bound, [("shout_suffix", "registry"), ("min_length", "render"), ("report_counts", "document")]);
    // The document's front matter is valid under it, and a setting of the wrong type is not.
    let text = std::fs::read_to_string(dir.join("wordcount.flm")).unwrap();
    let split = flm_main::seed::split(&text).unwrap();
    let front_matter = |text: &str| flm_main::seed::read_front_matter(text, split.front_matter_line).unwrap().to_json();
    let written = split.front_matter.unwrap();
    assert!(validator::valid(&schema, &schema, &front_matter(written)));
    assert!(!validator::valid(&schema, &schema, &front_matter(&written.replace("min_length: 4", "min_length: four"))));
}

#[test]
fn schema_of_an_extension_validates_the_test_manifests() {
    let dir = folder("schema-extension");
    let schema = printed_schema(&dir, "extension", &[]);
    let manifest = |path: &Path| flm_main::seed::read_front_matter(&std::fs::read_to_string(path).unwrap(), 1).unwrap().to_json();
    let mut files = vec![
        scripts().join("wordcount/flmextension.yaml"),
        scripts().join("twopack/flmextension.yaml"),
        scripts().join("typst/flmextension.yaml"),
        scripts().join("exercises/flmextension.yaml"),
    ];
    for name in ["counting.yaml", "margins.yaml", "house/flmextension.yaml"] {
        files.push(documents().join("_flmextensions").join(name));
    }
    for file in &files {
        assert!(validator::valid(&schema, &schema, &manifest(file)), "the schema of a manifest refuses `{}`", file.display());
    }
    let wrong = flm_main::seed::read_front_matter("extension:\n  engines: js\n", 1).unwrap().to_json();
    assert!(!validator::valid(&schema, &schema, &wrong));
}
