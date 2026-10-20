//! `flm render` and `flm check`, run as the binary over files under the build folder:
//! the output on standard output, in a file and in a folder; the options of the input and
//! of the output; the error gate and the exit statuses; the playground's command line of
//! `flm dump`; a workflow of two outputs (the extension `both` of
//! `tests/documents/outputs/_flmextensions/`, the outputs `html` and `text`) under each form
//! of `-o`, on standard output, narrowed by `-f`, told by `-v` and used through `--use`; the
//! recomposed LaTeX's style file named after the stem; a workflow whose outputs cannot share
//! one plan; the setting `write.output_names`, which names one output's main file.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

/// A fresh folder for one test, under the build folder.
fn folder(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-render-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// Runs `flm` with `args` in the folder `cwd`, `stdin` given to it.
fn flm_in(cwd: &Path, args: &[&str], stdin: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_flm"))
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(stdin.as_bytes()).unwrap();
    child.wait_with_output().unwrap()
}

fn flm(cwd: &Path, args: &[&str]) -> Output {
    flm_in(cwd, args, "")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

fn listing(folder: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        std::fs::read_dir(folder).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

const PAPER: &str = "---\ntitle: A paper\n---\nHello \\emph{world}.\n";
const REFERENCED: &str = "\\section{Intro}\\label{sec:intro}\n\nSee \\ref{sec:intro}.\n";
const BROKEN: &str = "Hello \\nosuchmacro.\n";

fn with_file(name: &str, text: &str) -> PathBuf {
    let dir = folder(name);
    std::fs::write(dir.join("paper.flm"), text).unwrap();
    dir
}

// --- test 7: dump -----------------------------------------------------------------------

#[test]
fn the_playground_s_command_line_of_dump_still_works() {
    let dir = with_file("dump", REFERENCED);
    let output = flm(&dir, &["dump", "paper.flm", "--workflow", "html", "--output", "html"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let value: serde_json::Value = serde_json::from_str(&stdout(&output)).unwrap();
    assert!(value.get("flm.output.html.document").is_some());
}

// --- test 14: render --------------------------------------------------------------------

#[test]
fn render_prints_the_page_on_standard_output() {
    // What `flm FILE` printed until the bare form became the preview.
    let dir = with_file("bare", PAPER);
    let output = flm(&dir, &["render", "paper.flm"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(stdout(&output).contains("world"));
    let text = flm(&dir, &["render", "-p", "text", "paper.flm"]);
    // The title of the front matter comes first.
    assert_eq!(stdout(&text), "A paper\n\nHello world.\n");
    assert!(listing(&dir) == ["paper.flm"]);
}

#[test]
fn no_final_newline_with_n() {
    let dir = with_file("newline", PAPER);
    let output = flm(&dir, &["render", "-p", "text", "-n", "paper.flm"]);
    assert_eq!(stdout(&output), "A paper\n\nHello world.");
    let html = flm(&dir, &["render", "-f", "html", "-n", "paper.flm"]);
    assert!(!stdout(&html).ends_with('\n'), "{}", stdout(&html));
    let html = flm(&dir, &["render", "-f", "html", "paper.flm"]);
    assert!(stdout(&html).ends_with('\n') && !stdout(&html).ends_with("\n\n"), "{}", stdout(&html));
}

#[test]
fn content_from_the_command_line_and_from_standard_input() {
    let dir = folder("content");
    let given = flm(&dir, &["render", "-p", "text", "-c", "Some \\emph{text}."]);
    assert_eq!(stdout(&given), "Some text.\n");
    let piped = flm_in(&dir, &["render", "-p", "text", "-"], "Piped text.");
    assert_eq!(stdout(&piped), "Piped text.\n");
}

#[test]
fn the_layout_is_chosen_on_the_command_line() {
    let dir = folder("layout");
    for args in [&["--layout", "blocks"][..], &["-B"][..], &["--layout", "inline"][..], &["--layout", "auto"][..]] {
        let mut line = vec!["render", "-f", "html", "-c", "One."];
        line.extend_from_slice(args);
        let output = flm(&dir, &line);
        assert_eq!(output.status.code(), Some(0), "{args:?}: {}", stderr(&output));
    }
    let blocks = stdout(&flm(&dir, &["render", "-f", "html", "-c", "One.", "-B"]));
    let inline = stdout(&flm(&dir, &["render", "-f", "html", "-c", "One.", "--layout", "inline"]));
    assert_ne!(blocks, inline);
}

#[test]
fn initial_use_on_the_command_line_wins_over_the_front_matter_s() {
    let dir = with_file("initial-use", "---\nflm:\n  initial_use: [std, nosuch]\n---\nHello \\emph{world}.\n");
    // The front matter's list: its unknown entry is reported where it is written.
    let written = flm(&dir, &["render", "-p", "text", "paper.flm"]);
    assert_eq!(written.status.code(), Some(2), "{}", stderr(&written));
    assert!(stderr(&written).contains("paper.flm:3:") && stderr(&written).contains("`nosuch` must name a package"), "{}", stderr(&written));
    // The command line's list replaces it; the program's math processor follows it.
    let given = flm(&dir, &["render", "-p", "text", "--initial-use", "std", "paper.flm"]);
    assert_eq!(given.status.code(), Some(0), "{}", stderr(&given));
    assert_eq!(stdout(&given), "Hello world.\n");
    // An unknown package given on the command line is in no file.
    let unknown = flm(&dir, &["render", "-p", "text", "--initial-use", "nosuch", "-c", "Hello."]);
    assert_eq!(unknown.status.code(), Some(2), "{}", stderr(&unknown));
    assert!(
        stderr(&unknown).contains("in the option `--initial-use`, `nosuch` must name a package compiled into the program"),
        "{}",
        stderr(&unknown)
    );
    // A value that is not a list of names stops the program before it reads the document.
    let refused = flm(&dir, &["render", "-p", "text", "--initial-use", "{a: 1}", "-c", "Hello."]);
    assert_eq!(refused.status.code(), Some(2), "{}", stderr(&refused));
    assert!(stderr(&refused).contains("`--initial-use` must be a package's name or a list of packages' names"), "{}", stderr(&refused));
}

#[test]
fn math_on_the_command_line_wins_over_the_front_matter_s() {
    let dir = with_file("math", "---\nflm:\n  math: nosuch\n---\nHello \\emph{world}.\n");
    // The front matter's processor: its unknown package is reported at the value of `math`.
    let written = flm(&dir, &["render", "-p", "text", "paper.flm"]);
    assert_eq!(written.status.code(), Some(2), "{}", stderr(&written));
    assert!(
        stderr(&written).contains("paper.flm:3:9") && stderr(&written).contains("`math-nosuch` must name a package"),
        "{}",
        stderr(&written)
    );
    // `--math` takes precedence, on `render` and on `check`.
    let given = flm(&dir, &["render", "-p", "text", "--math", "passthrough", "paper.flm"]);
    assert_eq!(given.status.code(), Some(0), "{}", stderr(&given));
    assert_eq!(stdout(&given), "Hello world.\n");
    let checked = flm(&dir, &["check", "--math", "passthrough", "paper.flm"]);
    assert_eq!(checked.status.code(), Some(0), "{}", stderr(&checked));
    // `--math false` adds no processor: a document without formulas runs without one, and
    // one listed under `use` is used.
    std::fs::write(dir.join("listed.flm"), "---\nflm: {use: [math-passthrough]}\n---\nHello.\n").unwrap();
    for command in ["render", "check"] {
        let none = flm(&dir, &[command, "--math", "false", "paper.flm"]);
        assert_eq!(none.status.code(), Some(0), "{}", stderr(&none));
        let listed = flm(&dir, &[command, "--math", "false", "listed.flm"]);
        assert_eq!(listed.status.code(), Some(0), "{}", stderr(&listed));
    }
    // An unknown processor given on the command line is in no file.
    let unknown = flm(&dir, &["check", "--math", "nosuch", "-c", "Hello."]);
    assert_eq!(unknown.status.code(), Some(2), "{}", stderr(&unknown));
    assert!(
        stderr(&unknown).contains("in the option `--math`, `math-nosuch` must name a package compiled into the program"),
        "{}",
        stderr(&unknown)
    );
    // A value that is not a processor's name stops the program before it reads the document.
    for (value, message) in [
        ("a/b", "`--math` must be a math processor's name, made of letters, digits, `-` and `_` only, not `a/b`"),
        ("true", "`--math` must be a math processor's name, such as `mathjax` or `passthrough`, or `false`"),
        ("[a", "the value of `--math` must be valid YAML: "),
    ] {
        let refused = flm(&dir, &["render", "--math", value, "-c", "Hello."]);
        assert_eq!(refused.status.code(), Some(2), "{}", stderr(&refused));
        assert!(stderr(&refused).contains(message), "{}", stderr(&refused));
    }
}

#[test]
fn render_to_a_file_and_to_a_folder() {
    let dir = with_file("file", REFERENCED);
    let output = flm(&dir, &["render", "-p", "latex-recomposed", "-o", "out/paper.tex", "paper.flm"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let written = listing(&dir.join("out"));
    assert!(written.contains(&String::from("paper.tex")), "{written:?}");
    assert!(written.iter().any(|name| name.ends_with(".sty")), "{written:?}");
    let output = flm(&dir, &["render", "-f", "html", "-o", "site/", "paper.flm"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(listing(&dir.join("site")).contains(&String::from("index.html")));
    assert!(stdout(&output).is_empty());
}

#[test]
fn verbose_tells_what_the_program_does() {
    let dir = with_file("verbose", PAPER);
    let output = flm(&dir, &["render", "-v", "-p", "text", "-o", "paper.txt", "paper.flm"]);
    let told = stderr(&output);
    for word in ["paper.flm", "std", "text", "paper.txt"] {
        assert!(told.contains(word), "{word}: {told}");
    }
}

#[test]
fn verbose_names_no_file_when_the_gate_stops_the_run() {
    let dir = with_file("verbose-gate", BROKEN);
    let output = flm(&dir, &["render", "-v", "-p", "text", "-o", "paper.txt", "paper.flm"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!stderr(&output).contains("paper.txt"), "{}", stderr(&output));
}

// --- test 12: the output on the disk ----------------------------------------------------

#[test]
fn the_disk_keeps_no_scratch_folder_and_loses_the_file_no_longer_made() {
    let dir = with_file("stale", REFERENCED);
    flm(&dir, &["render", "-p", "latex-recomposed", "-o", "out/paper.tex", "paper.flm"]);
    let first = listing(&dir.join("out"));
    assert!(first.iter().any(|name| name.ends_with(".sty")), "{first:?}");
    assert!(!first.iter().any(|name| name.starts_with(".flm-scratch")), "{first:?}");
    std::fs::write(dir.join("paper.flm"), "Plain text.\n").unwrap();
    flm(&dir, &["render", "-p", "latex-recomposed", "-o", "out/paper.tex", "paper.flm"]);
    let second = listing(&dir.join("out"));
    assert!(!second.iter().any(|name| name.ends_with(".sty")), "{second:?}");
    assert!(second.contains(&String::from("paper.tex")));
}

// --- test 13: the gate ------------------------------------------------------------------

#[test]
fn a_document_with_errors_writes_nothing_unless_asked() {
    let dir = with_file("gate", BROKEN);
    let output = flm(&dir, &["render", "-p", "text", "-o", "paper.txt", "paper.flm"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(!dir.join("paper.txt").exists());
    let output = flm(&dir, &["render", "-p", "text", "-o", "paper.txt", "--continue-with-errors", "paper.flm"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(dir.join("paper.txt").exists());
}

#[test]
fn warnings_alone_succeed() {
    let dir = with_file("warnings", PAPER);
    let output = flm(&dir, &["render", "-p", "text", "-O", "pieces.include=[nosuch.piece]", "paper.flm"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(!stderr(&output).is_empty());
}

// --- test 15: check ---------------------------------------------------------------------

#[test]
fn check_prints_the_problems_and_writes_nothing() {
    let dir = with_file("check", BROKEN);
    let output = flm(&dir, &["check", "paper.flm"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty());
    assert!(stderr(&output).contains("paper.flm:1:"), "{}", stderr(&output));
    assert_eq!(listing(&dir), ["paper.flm"]);
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    assert_eq!(flm(&dir, &["check", "paper.flm"]).status.code(), Some(0));
    assert_eq!(flm(&dir, &["check", "-p", "html", "paper.flm"]).status.code(), Some(0));
}

/// The help of `-p` and `-f`, which `render`, `check` and `watch` share.
#[test]
fn the_help_of_each_command_that_builds_lists_p_and_f() {
    let dir = folder("help-p");
    for command in ["render", "check", "watch"] {
        let output = flm(&dir, &[command, "--help"]);
        assert_eq!(output.status.code(), Some(0));
        let help = stdout(&output);
        assert!(help.contains("-p, --profile <PROFILE>") && help.contains("-f, --format <FORMAT>"), "{help}");
    }
}

/// `-f` does not choose the workflow of `flm check`, `check`, which provides no format.
#[test]
fn check_runs_its_workflow_whatever_f_names() {
    let dir = with_file("check-f", PAPER);
    let output = flm(&dir, &["check", "-f", "html", "paper.flm"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("the workflow `check` does not provide the format `html`; it provides none"),
        "{}",
        stderr(&output)
    );
}

// --- test 16: the printer, a problem in the front matter --------------------------------

#[test]
fn a_problem_in_the_front_matter_is_printed_at_its_line() {
    let dir = with_file("front", "---\ntitle: x\nflm:\n  features:\n    text: {wdth: 3}\n---\nx\n");
    let output = flm(&dir, &["render", "-p", "text", "paper.flm"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("paper.flm:5:"), "{}", stderr(&output));
}

/// The warning `flm` prints for `false` written for the feature `nosuch`, which no package in
/// use has, at line 4 of `paper.flm`.
const NOSUCH_OFF: &str =
    "warning: `nosuch` is switched off, but no package in use has a feature `nosuch`\n  at paper.flm:4:13\n";

#[test]
fn a_warning_found_before_the_work_stopped_is_printed_before_the_problems() {
    // Beside a problem with the settings, then beside a problem of the registry (chains
    // that place features in a circle).
    for (more, problem) in [
        ("    text: {wdth: 3}\n", "paper.flm:5:"),
        ("  feature_order: [headings < toc, toc < headings]\n", "must not contain a cycle"),
    ] {
        let dir = with_file("warning-stopped", &format!("---\nflm:\n  features:\n    nosuch: false\n{more}---\nx\n"));
        let output = flm(&dir, &["render", "-p", "text", "paper.flm"]);
        assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
        let printed = stderr(&output);
        assert!(printed.starts_with(NOSUCH_OFF), "{printed}");
        assert!(printed[NOSUCH_OFF.len()..].starts_with("error: ") && printed.contains(problem), "{printed}");
    }
}

#[cfg(all(feature = "js", feature = "bundled-extensions"))]
#[test]
fn a_misspelled_math_processor_switched_off_is_named_beside_the_errors_it_causes() {
    // `mathjax` for `math-mathjax`: the program's math processor stays, beside the one the
    // document names, and the registry cannot be built.
    let dir = with_file(
        "warning-processor",
        "---\nflm:\n  features:\n    mathjax: false\n  use: [math-passthrough]\n---\nx\n",
    );
    let output = flm(&dir, &["render", "-p", "text", "paper.flm"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    let printed = stderr(&output);
    let warning = "warning: `mathjax` is switched off, but no package in use has a feature `mathjax`\n  at paper.flm:4:14\n";
    assert!(printed.starts_with(warning), "{printed}");
    assert!(printed.contains("may have only one default lowering rule"), "{printed}");
}

// --- test 18: status 2 for each condition before the work starts ------------------------

#[test]
fn arguments_that_make_no_command() {
    let dir = folder("usage");
    assert_eq!(flm(&dir, &[]).status.code(), Some(2));
    assert_eq!(flm(&dir, &["render"]).status.code(), Some(2));
    assert_eq!(flm(&dir, &["render", "-p", "text", "--nosuch", "x.flm"]).status.code(), Some(2));
    // `-V` belongs to `render`, and there is no `-W`: the bare form is the preview.
    assert_eq!(flm(&dir, &["-V", "x.flm"]).status.code(), Some(2));
    assert_eq!(flm(&dir, &["-W", "x.flm"]).status.code(), Some(2));
    assert_eq!(flm(&dir, &["render", "-W", "x.flm"]).status.code(), Some(2));
    // `flm` alone prints the usage, naming the preview.
    let usage = flm(&dir, &[]);
    assert!(stderr(&usage).contains("Usage: flm"), "{}", stderr(&usage));
    assert!(stderr(&usage).contains("watch"), "{}", stderr(&usage));
}

#[test]
fn a_text_and_a_file_together_make_no_command() {
    // The file exists and reads, so the status comes from the two inputs alone.
    let dir = with_file("text-and-file", "x\n");
    assert_eq!(flm(&dir, &["render", "paper.flm"]).status.code(), Some(0));
    assert_eq!(flm(&dir, &["render", "-c", "x", "paper.flm"]).status.code(), Some(2));
}

#[test]
fn an_input_that_cannot_be_read() {
    let dir = folder("unreadable");
    assert_eq!(flm(&dir, &["render", "nosuch.flm"]).status.code(), Some(2));
}

#[test]
fn a_front_matter_that_does_not_read() {
    for (index, text) in ["---\ntitle: x\n", "---\ntitle: [x\n---\n", "---\n- x\n---\n", "---\nflm: {nope: 1}\n---\n"]
        .into_iter()
        .enumerate()
    {
        let dir = with_file(&format!("front-{index}"), text);
        assert_eq!(flm(&dir, &["render", "paper.flm"]).status.code(), Some(2), "{text}");
    }
}

#[test]
fn a_name_that_matches_nothing() {
    let dir = with_file("names", "---\nflm: {use: [nosuch]}\n---\nx\n");
    assert_eq!(flm(&dir, &["render", "paper.flm"]).status.code(), Some(2));
    std::fs::write(dir.join("paper.flm"), "x\n").unwrap();
    assert_eq!(flm(&dir, &["render", "-f", "nosuch", "paper.flm"]).status.code(), Some(2));
    assert_eq!(flm(&dir, &["render", "-p", "nosuch", "paper.flm"]).status.code(), Some(2));
}

#[test]
fn a_setting_that_does_not_read() {
    let dir = with_file("settings", "x\n");
    assert_eq!(flm(&dir, &["render", "-O", "text.wdth=3", "paper.flm"]).status.code(), Some(2));
    assert_eq!(flm(&dir, &["render", "-O", "text.width=wide", "paper.flm"]).status.code(), Some(2));
    assert_eq!(flm(&dir, &["render", "-O", "nosuch", "paper.flm"]).status.code(), Some(2));
}

#[test]
fn a_registry_that_does_not_build() {
    let dir = with_file("registry", "---\nflm:\n  features: {lowering: false}\n---\nx\n");
    assert_eq!(flm(&dir, &["render", "-p", "text", "paper.flm"]).status.code(), Some(2));
}

const WRITE_OFF: &str = "---\nflm:\n  features: {write: false}\n---\nHello.\n";

#[test]
fn check_reads_a_document_that_switches_write_off() {
    let dir = with_file("write-off-check", WRITE_OFF);
    let output = flm(&dir, &["check", "paper.flm"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
}

#[test]
fn render_cannot_start_when_write_is_switched_off() {
    let dir = with_file("write-off-render", WRITE_OFF);
    let output = flm(&dir, &["render", "-p", "text", "paper.flm"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("paper.flm:3:"), "{}", stderr(&output));
    assert!(stdout(&output).is_empty());
}

#[test]
fn the_keys_of_a_feature_switched_off_are_not_read() {
    let dir = with_file("text-off", "---\nflm:\n  features: {text: false, xnotes: false, floats: false}\n---\nHello.\n");
    let output = flm(&dir, &["render", "-f", "html", "-O", "text.width=30", "paper.flm"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
}

#[test]
fn a_plan_that_does_not_build() {
    let dir = with_file("plan", "---\nflm:\n  features: {pieces: false}\n---\nx\n");
    assert_eq!(flm(&dir, &["render", "-p", "text", "paper.flm"]).status.code(), Some(2));
}

// --- every output of a workflow ----------------------------------------------------------

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

/// A fresh folder `name` holding the extensions of `tests/documents/outputs/_flmextensions/`
/// and `paper.flm`, which uses the extension `extension` and holds `text`.
fn using(name: &str, extension: &str, text: &str) -> PathBuf {
    let dir = folder(name);
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/outputs/_flmextensions");
    copy_tree(&fixtures, &dir.join("_flmextensions"));
    std::fs::write(dir.join("paper.flm"), format!("---\nflm: {{use: [{extension}]}}\n---\n{text}")).unwrap();
    dir
}

/// The names in `folder` other than the document and the extensions, sorted.
fn written(folder: &Path) -> Vec<String> {
    listing(folder).into_iter().filter(|name| name != "paper.flm" && name != "_flmextensions").collect()
}

/// Runs `flm` with `args` in `cwd`, and asserts that it succeeded and reported nothing.
fn succeeds(cwd: &Path, args: &[&str]) -> Output {
    let output = flm(cwd, args);
    assert_eq!(output.status.code(), Some(0), "{args:?}: {}", stderr(&output));
    assert!(output.stderr.is_empty(), "{args:?}: {}", stderr(&output));
    output
}

/// The lines of the record `name` in `folder`.
fn record(folder: &Path, name: &str) -> Vec<String> {
    std::fs::read_to_string(folder.join(name)).unwrap().lines().map(String::from).collect()
}

#[test]
fn every_form_of_o_names_the_files_of_both_outputs() {
    let text = "Hello \\emph{world}.\n";
    for (index, (name, stem)) in [("paper.html", "paper"), ("paper.txt", "paper"), ("paper", "paper"), ("index.htm", "index.htm")]
        .into_iter()
        .enumerate()
    {
        let dir = using(&format!("both-{index}"), "both", text);
        succeeds(&dir, &["render", "-p", "both", "-o", name, "paper.flm"]);
        let (html, txt) = (format!("{stem}.html"), format!("{stem}.txt"));
        let mut expected = vec![format!(".{html}.flm-written"), format!(".{txt}.flm-written"), html.clone(), txt.clone()];
        expected.sort();
        assert_eq!(written(&dir), expected, "-o {name}");
        assert_eq!(record(&dir, &format!(".{html}.flm-written")), std::slice::from_ref(&html), "-o {name}");
        assert_eq!(record(&dir, &format!(".{txt}.flm-written")), std::slice::from_ref(&txt), "-o {name}");
        let page = std::fs::read_to_string(dir.join(&html)).unwrap();
        assert!(page.starts_with("<!DOCTYPE html>") && page.contains("world"), "-o {name}: {page}");
        assert_eq!(std::fs::read_to_string(dir.join(&txt)).unwrap(), "Hello world.\n", "-o {name}");
    }
    let dir = using("both-folder", "both", text);
    succeeds(&dir, &["render", "-p", "both", "-o", "out/", "paper.flm"]);
    assert_eq!(listing(&dir.join("out")), [".index.html.flm-written", ".index.txt.flm-written", "index.html", "index.txt"]);
    assert_eq!(written(&dir), ["out"]);
}

#[test]
fn standard_output_refuses_the_files_of_two_outputs() {
    let dir = using("both-stdout", "both", "Hello.\n");
    let output = flm(&dir, &["render", "-p", "both", "paper.flm"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stdout(&output).is_empty(), "{}", stdout(&output));
    assert!(
        stderr(&output).contains(
            "error: the output `html` cannot be written: standard output can receive one file, and this command's \
             outputs (`html`, `text`) hold 2 files (`index.html`, `index.txt`): write them with `-o FILE` or `-o \
             FOLDER/`"
        ),
        "{}",
        stderr(&output)
    );
    assert!(written(&dir).is_empty());
    // One output of the workflow, named with `-f`, is one file: standard output takes it.
    let output = succeeds(&dir, &["render", "-p", "both", "-f", "text", "paper.flm"]);
    assert_eq!(stdout(&output), "Hello.\n");
}

#[test]
fn f_narrows_the_command_to_one_output_and_leaves_the_other_s_files() {
    let dir = using("both-narrowed", "both", "First.\n");
    succeeds(&dir, &["render", "-p", "both", "-o", "paper.html", "paper.flm"]);
    std::fs::write(dir.join("paper.flm"), "---\nflm: {use: [both]}\n---\nSecond.\n").unwrap();
    succeeds(&dir, &["render", "-p", "both", "-f", "html", "-o", "paper.html", "paper.flm"]);
    assert_eq!(written(&dir), [".paper.html.flm-written", ".paper.txt.flm-written", "paper.html", "paper.txt"]);
    assert!(std::fs::read_to_string(dir.join("paper.html")).unwrap().contains("Second."));
    // The text is the first command's, untouched, and so is its record.
    assert_eq!(std::fs::read_to_string(dir.join("paper.txt")).unwrap(), "First.\n");
    assert_eq!(record(&dir, ".paper.txt.flm-written"), ["paper.txt"]);
    // In a fresh folder, the narrowed command writes the page alone.
    let fresh = using("both-narrowed-fresh", "both", "First.\n");
    succeeds(&fresh, &["render", "-p", "both", "-f", "html", "-o", "paper.html", "paper.flm"]);
    assert_eq!(written(&fresh), [".paper.html.flm-written", "paper.html"]);
}

#[test]
fn use_adds_the_extension_that_declares_the_profile_s_workflow() {
    let dir = folder("both-use");
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/outputs/_flmextensions");
    copy_tree(&fixtures, &dir.join("_flmextensions"));
    std::fs::write(dir.join("paper.flm"), "Hello.\n").unwrap();
    let without = flm(&dir, &["render", "-p", "both", "-f", "text", "paper.flm"]);
    assert_eq!(without.status.code(), Some(2), "{}", stderr(&without));
    let output = succeeds(&dir, &["render", "--use", "both", "-p", "both", "-f", "text", "paper.flm"]);
    assert_eq!(stdout(&output), "Hello.\n");
}

#[test]
fn verbose_names_both_outputs_and_every_file_written() {
    let dir = using("both-verbose", "both", "Hello.\n");
    let output = flm(&dir, &["render", "-v", "-p", "both", "-o", "paper.html", "paper.flm"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let told = stderr(&output);
    for line in ["flm: workflow both, outputs html, text\n", "flm: wrote paper.html\n", "flm: wrote paper.txt\n"] {
        assert_eq!(told.matches(line).count(), 1, "{line}: {told}");
    }
    assert_eq!(told.matches("flm: wrote ").count(), 2, "{told}");
}

/// The name of the one style file among `names`, which starts with `stem-flmrecomposed.`
/// and ends with `.sty`, without its `.sty`.
fn style_package(names: &[String], stem: &str) -> String {
    let styles: Vec<&String> = names.iter().filter(|name| name.ends_with(".sty")).collect();
    assert_eq!(styles.len(), 1, "{names:?}");
    let package = styles[0].strip_suffix(".sty").unwrap();
    let hash = package.strip_prefix(&format!("{stem}-flmrecomposed.")).unwrap_or_else(|| panic!("{names:?}"));
    assert!(hash.len() == 8 && hash.chars().all(|c| c.is_ascii_hexdigit()), "{names:?}");
    package.to_string()
}

#[test]
fn the_style_file_of_the_recomposed_latex_takes_the_stem() {
    for (index, (name, place, stem)) in
        [("paper.tex", "", "paper"), ("paper", "", "paper"), ("out/", "out", "index")].into_iter().enumerate()
    {
        let dir = with_file(&format!("style-{index}"), REFERENCED);
        succeeds(&dir, &["render", "-p", "latex-recomposed", "-o", name, "paper.flm"]);
        let folder = dir.join(place);
        let names: Vec<String> = written(&folder);
        let package = style_package(&names, stem);
        let mut expected = vec![format!(".{stem}.tex.flm-written"), format!("{package}.sty"), format!("{stem}.tex")];
        expected.sort();
        assert_eq!(names, expected, "-o {name}");
        let document = std::fs::read_to_string(folder.join(format!("{stem}.tex"))).unwrap();
        assert!(document.contains(&format!("\\usepackage{{{package}}}")), "-o {name}: {document}");
        let style = std::fs::read_to_string(folder.join(format!("{package}.sty"))).unwrap();
        assert!(style.contains(&format!("\\ProvidesPackage{{{package}}}")), "-o {name}: {style}");
        let mut listed = record(&folder, &format!(".{stem}.tex.flm-written"));
        listed.sort();
        assert_eq!(listed, [format!("{package}.sty"), format!("{stem}.tex")], "-o {name}");
    }
}

#[test]
fn a_workflow_whose_outputs_cannot_share_one_plan_is_refused() {
    // The recomposed LaTeX needs the tree as parsed, the page needs it laid out in
    // paragraphs: one run cannot make both.
    let dir = using("htmltex", "htmltex", "Hello.\n");
    let output = flm(&dir, &["render", "-p", "htmltex", "-o", "paper.html", "paper.flm"]);
    assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
    assert!(
        stderr(&output).contains(
            "error: for workflow `htmltex`, the plan needs the capability `flm.blocks.raw`, but every stage that \
             provides it was ruled out"
        ),
        "{}",
        stderr(&output)
    );
    assert!(written(&dir).is_empty());
}

// --- the setting `write.output_names` ---------------------------------------------------

#[test]
fn output_names_renames_the_main_file_of_one_output() {
    let dir = using("names-folder", "both", "Hello.\n");
    succeeds(&dir, &["render", "-p", "both", "-o", "out/", "-O", "write.output_names.text=notes.txt", "paper.flm"]);
    let out = dir.join("out");
    assert_eq!(listing(&out), [".index.html.flm-written", ".notes.txt.flm-written", "index.html", "notes.txt"]);
    assert_eq!(record(&out, ".notes.txt.flm-written"), ["notes.txt"]);
    assert_eq!(std::fs::read_to_string(out.join("notes.txt")).unwrap(), "Hello.\n");
    // Under `-o FILE`, the other output keeps the stem `-o` gives.
    let dir = using("names-file", "both", "Hello.\n");
    succeeds(&dir, &["render", "-p", "both", "-o", "paper.html", "-O", "write.output_names.text=notes.txt", "paper.flm"]);
    assert_eq!(written(&dir), [".notes.txt.flm-written", ".paper.html.flm-written", "notes.txt", "paper.html"]);
}

#[test]
fn output_names_is_read_from_the_front_matter_and_from_a_workflow() {
    let dir = using("names-front-matter", "both", "");
    std::fs::write(
        dir.join("paper.flm"),
        "---\nflm: {use: [both], features: {write: {output_names: {text: notes.txt}}}}\n---\nHello.\n",
    )
    .unwrap();
    succeeds(&dir, &["render", "-p", "both", "-o", "out/", "paper.flm"]);
    assert_eq!(listing(&dir.join("out")), [".index.html.flm-written", ".notes.txt.flm-written", "index.html", "notes.txt"]);
    let dir = using("names-workflow", "both", "Hello.\n");
    succeeds(&dir, &["render", "-p", "named", "-o", "out/", "paper.flm"]);
    assert_eq!(listing(&dir.join("out")), [".index.html.flm-written", ".notes.txt.flm-written", "index.html", "notes.txt"]);
}

#[test]
fn output_names_leaves_the_files_a_template_ships_under_the_stem() {
    let dir = using("names-fancy", "fancy", "Hello.\n");
    succeeds(&dir, &["render", "-f", "html", "-o", "out/", "-O", "write.output_names.html=page.html", "paper.flm"]);
    let out = dir.join("out");
    assert_eq!(listing(&out), [".page.html.flm-written", "index-logo.png", "index-style.css", "page.html"]);
    let page = std::fs::read_to_string(out.join("page.html")).unwrap();
    assert!(page.contains("href=\"index-style.css\""), "{page}");
}

#[test]
fn output_names_takes_a_folder_part() {
    let dir = using("names-subfolder", "both", "Hello.\n");
    succeeds(&dir, &["render", "-p", "both", "-o", "out/", "-O", "write.output_names.text=exports/notes.txt", "paper.flm"]);
    let out = dir.join("out");
    assert_eq!(listing(&out), [".index.html.flm-written", "exports", "index.html"]);
    assert_eq!(listing(&out.join("exports")), [".notes.txt.flm-written", "notes.txt"]);
    assert_eq!(record(&out, "exports/.notes.txt.flm-written"), ["exports/notes.txt"]);
}

#[test]
fn output_names_for_an_unknown_output_writes_nothing() {
    let dir = using("names-unknown", "both", "Hello.\n");
    let output = flm(&dir, &["render", "-p", "both", "-o", "out/", "-O", "write.output_names.foo=x.txt", "paper.flm"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(
        stderr(&output).contains(
            "error: `foo` in the setting `write.output_names` must be the name of an output of the packages in use: \
             `bibtex`, `csl-json`, `html`, `latex-recomposed`, `latex-rendered`, `text`"
        ),
        "{}",
        stderr(&output)
    );
    assert!(!dir.join("out").exists() || listing(&dir.join("out")).is_empty());
}

#[test]
fn output_names_must_stay_inside_the_output_folder() {
    let dir = using("names-leaving", "both", "Hello.\n");
    let output = flm(&dir, &["render", "-p", "both", "-o", "out/", "-O", "write.output_names.text=../notes.txt", "paper.flm"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert!(stderr(&output).contains("the path `../notes.txt` must stay inside the output folder"), "{}", stderr(&output));
    assert!(written(&dir).iter().all(|name| name == "out"), "{:?}", written(&dir));
    assert!(!dir.join("out").exists() || listing(&dir.join("out")).is_empty());
}

#[test]
fn output_names_changes_nothing_on_standard_output() {
    let dir = using("names-stdout", "both", "Hello.\n");
    let output = succeeds(&dir, &["render", "-p", "both", "-f", "text", "-O", "write.output_names.text=notes.txt", "paper.flm"]);
    assert_eq!(stdout(&output), "Hello.\n");
    assert!(written(&dir).is_empty());
}

#[test]
fn a_second_render_under_another_name_leaves_the_earlier_file() {
    // The record of an output's files is named after its main file: under a new name, the
    // record is a new one, and the file of the earlier name is not removed.
    let dir = using("names-rename", "both", "Hello.\n");
    succeeds(&dir, &["render", "-p", "both", "-f", "text", "-o", "out/", "-O", "write.output_names.text=notes.txt", "paper.flm"]);
    succeeds(&dir, &["render", "-p", "both", "-f", "text", "-o", "out/", "-O", "write.output_names.text=memo.txt", "paper.flm"]);
    assert_eq!(
        listing(&dir.join("out")),
        [".memo.txt.flm-written", ".notes.txt.flm-written", "memo.txt", "notes.txt"]
    );
}
