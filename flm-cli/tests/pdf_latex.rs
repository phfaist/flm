//! The extension `profile-pdf-latex`, compiled into the binary (cargo feature
//! `bundled-extensions`): its workflow `pdf-latex` compiles the files of the output
//! `latex-rendered` with `latexmk`, found on `PATH` and run through the host service
//! `programs`.
//!
//! Each test runs the `flm` binary with `PATH` holding only a folder of its own, which holds
//! a stand-in for `latexmk` (a `#!/bin/sh` script) or nothing. The stand-in writes
//! `STEM.pdf` holding the line `%PDF-fake`, its arguments, the sorted list of the files in
//! its folder, the `.tex` file it received and the byte 0xFF (not UTF-8, so the PDF crosses
//! the extension's boundary as base64); it writes `STEM.log`, and appends its arguments to
//! the file whose path the environment variable `FAKE_LATEXMK_RECORD` gives, which reaches it
//! through `flm` and the service. The stand-in that runs past its time records instead the
//! process ID of the program it starts, so that a test checks that this program is stopped
//! with it. The variable `TMPDIR` points at a folder of the test's own, so that the service
//! makes its temporary folders there, and a temporary folder that a signal leaves behind
//! stays among the test's files.
//!
//! Two tests are gated. With `FLM_TESTS_LATEX_COMPILE` set, the real `latexmk` compiles a
//! small document. With `FLM_TESTS_WASM_GUEST=1` (and the target `wasm32-unknown-unknown`
//! installed), the script `extensions/rust/build-wasm.sh` builds the extension's WebAssembly
//! form, and the document of the first test, rendered with `--extensions-dir` pointing at
//! it, gives the same PDF as the extension compiled in. Otherwise each writes one line
//! beginning `skipped:`.

#![cfg(all(unix, feature = "bundled-extensions"))]

use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

/// The arguments the extension gives `latexmk` for the entry file `paper.tex`.
const ARGS: &str = "-norc -lualatex -f -interaction=nonstopmode -file-line-error paper.tex";

/// The stand-in for `latexmk` that succeeds.
const FAKE: &str = r#"#!/bin/sh
# A stand-in for latexmk: writes STEM.pdf and STEM.log, and records its arguments.
PATH=/usr/bin:/bin
for last; do :; done
stem=$(basename "$last" .tex)
files=$(find . -type f | sort)
{
  printf '%%PDF-fake\n'
  printf 'args: %s\n' "$*"
  printf 'files:\n%s\n' "$files"
  printf 'tex:\n'
  cat "$last"
  printf '\377'
} > "$stem.pdf"
printf 'This is a stand-in log.\n' > "$stem.log"
printf '%s\n' "$*" >> "$FAKE_LATEXMK_RECORD"
"#;

/// The stand-in for `latexmk` that fails: a log recording an error, a PDF all the same, and
/// the exit status 1.
const FAILING: &str = r#"#!/bin/sh
PATH=/usr/bin:/bin
for last; do :; done
stem=$(basename "$last" .tex)
printf '%%PDF-partial\377' > "$stem.pdf"
printf 'This is LuaHBTeX\n(./paper.tex\n./paper.tex:3: Undefined control sequence.\nl.3 \\foo\n\n)\n' > "$stem.log"
printf 'Latexmk: Errors, so I did not complete making targets\n' >&2
printf '%s\n' "$*" >> "$FAKE_LATEXMK_RECORD"
exit 1
"#;

/// The stand-in for `latexmk` that runs past any time it is given: it starts a program of
/// its own, as `latexmk` starts LuaLaTeX, records that program's process ID instead of its
/// arguments, and waits for it.
const SLOW: &str = "#!/bin/sh\n/bin/sleep 30 &\nprintf '%s\\n' \"$!\" >> \"$FAKE_LATEXMK_RECORD\"\nwait\n";

/// The stand-in for `latexmk` that fails with no error in its log: it writes no `.log` file,
/// prints two lines to its standard error, and exits with the status 12.
const SILENT_LOG: &str = "#!/bin/sh\nprintf 'Latexmk: one\\nLatexmk: two\\n' >&2\nexit 12\n";

/// The stand-in for `latexmk` that exits with the status 0 and writes no file.
const NO_PDF: &str = "#!/bin/sh\nexit 0\n";

/// The stand-in for `latexmk` that a signal stops.
const KILLED: &str = "#!/bin/sh\nkill -KILL $$\n";

/// The numbers of the signals the tests send (the same on every Unix system): the hang-up,
/// the interrupt that Ctrl+C sends, the quit signal that `Ctrl+\` sends, and `SIGTERM`.
const SIGHUP: i32 = 1;
const SIGINT: i32 = 2;
const SIGQUIT: i32 = 3;
const SIGTERM: i32 = 15;

/// A document with an image, whose front matter needs nothing for the profile `pdf-latex`.
const PAPER: &str = "---\ntitle: A paper\n---\nHello \\emph{world}.\n\n\
                     \\begin{figure}\n\\includegraphics{plot.png}\n\\caption{A plot.}\n\\end{figure}\n";

/// A fresh folder for the test `name`, under the build folder, holding `paper.flm` (written
/// `document`), the image `plot.png`, the empty folder `tmp/`, and the folder `bin/`, holding
/// the stand-in `fake` for `latexmk` when one is given.
fn folder(name: &str, document: &str, fake: Option<&str>) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-cli-pdf-latex").join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("bin")).unwrap();
    std::fs::create_dir_all(dir.join("tmp")).unwrap();
    std::fs::write(dir.join("paper.flm"), document).unwrap();
    let image = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/figures/plot.png");
    std::fs::copy(image, dir.join("plot.png")).unwrap();
    if let Some(script) = fake {
        let program = dir.join("bin/latexmk");
        std::fs::write(&program, script).unwrap();
        std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    dir
}

/// The command `flm render ARGS paper.flm` in `dir`, in the environment of [`environment`].
fn command(dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_flm"));
    command.arg("render").args(args).arg("paper.flm");
    environment(&mut command, dir);
    command
}

/// The command of [`command`], started through `/bin/sh`, which first runs the shell command
/// `setup` and then gives its process to `flm` (`exec`): `flm` keeps what `setup` set (a
/// signal ignored, a limit) and has the process ID of the command started.
fn command_after(setup: &str, dir: &Path, args: &[&str]) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", &format!("{setup}; exec \"$0\" \"$@\""), env!("CARGO_BIN_EXE_flm"), "render"])
        .args(args)
        .arg("paper.flm");
    environment(&mut command, dir);
    command
}

/// Runs `command` in `dir`, with `PATH` holding `dir/bin` alone, the stand-in's record at
/// `dir/record`, the system's temporary folder at `dir/tmp`, the home folder `dir` and
/// `FLMPATH` unset.
fn environment(command: &mut Command, dir: &Path) {
    command
        .current_dir(dir)
        .env("PATH", dir.join("bin"))
        .env("FAKE_LATEXMK_RECORD", dir.join("record"))
        .env("TMPDIR", dir.join("tmp"))
        .env("HOME", dir)
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE");
}

/// Runs the command `flm render ARGS paper.flm` of [`command`] and waits for its end.
fn render(dir: &Path, args: &[&str]) -> Output {
    command(dir, args).output().unwrap()
}

/// The standard error of `output`.
fn reported(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The arguments the stand-in recorded, one line per run; empty when it never ran.
fn recorded(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("record")).map(|text| text.lines().map(str::to_string).collect()).unwrap_or_default()
}

/// The names of the files written in `dir/out`, those beginning with `.` aside, in name
/// order; empty when the folder does not exist.
fn written(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir.join("out")) else { return Vec::new() };
    let mut names: Vec<String> = entries
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .filter(|name| !name.starts_with('.'))
        .collect();
    names.sort();
    names
}

/// What the stand-in wrote into the PDF: its arguments, the files it found, the `.tex` file.
struct Received {
    args: String,
    files: Vec<String>,
    tex: String,
}

/// Reads the stand-in's PDF `bytes`, checking its first line and its last byte.
fn received(bytes: &[u8]) -> Received {
    assert!(bytes.starts_with(b"%PDF-fake\n"), "{}", String::from_utf8_lossy(bytes));
    assert_eq!(bytes.last(), Some(&0xFF), "the byte 0xFF ends the file");
    let text = std::str::from_utf8(&bytes[..bytes.len() - 1]).expect("the rest is text");
    let (head, tex) = text.split_once("\ntex:\n").expect("the .tex file");
    let (args, files) = head.split_once("\nfiles:\n").expect("the files");
    Received {
        args: args.strip_prefix("%PDF-fake\nargs: ").expect("the arguments").to_string(),
        files: files.lines().map(|file| file.trim_start_matches("./").to_string()).collect(),
        tex: tex.to_string(),
    }
}

/// The text between `before` and the next `}` after it in `tex`.
fn braced_after<'a>(tex: &'a str, before: &str) -> &'a str {
    let start = tex.find(before).unwrap_or_else(|| panic!("`{before}` in {tex}")) + before.len();
    let rest = &tex[start..];
    let rest = &rest[rest.find('{').expect("a brace") + 1..];
    &rest[..rest.find('}').expect("a closing brace")]
}

/// The document with an image, rendered with `-p pdf-latex -o out/paper.pdf`: only the PDF
/// is written, holding the stand-in's bytes, the byte 0xFF included; LaTeX received the
/// `.tex` file, the style file under the name the `.tex` file loads, and the copied image
/// under the path the `.tex` file shows it at.
#[test]
fn the_workflow_writes_the_pdf_alone_compiled_from_the_latex_files() {
    let dir = folder("pdf", PAPER, Some(FAKE));
    let output = render(&dir, &["-p", "pdf-latex", "-o", "out/paper.pdf"]);
    assert_eq!(output.status.code(), Some(0), "{}", reported(&output));
    assert_eq!(reported(&output), "");
    assert_eq!(written(&dir), ["paper.pdf"]);
    let pdf = std::fs::read(dir.join("out/paper.pdf")).unwrap();
    let received = received(&pdf);
    assert_eq!(received.args, ARGS);
    assert_eq!(recorded(&dir), [ARGS]);
    let style = format!("{}.sty", braced_after(&received.tex, "\\usepackage"));
    let image = braced_after(&received.tex, "\\flmxImage").to_string();
    // The compose template may supply class options; this test checks the compiled files.
    assert!(received.tex.starts_with("\\documentclass"), "{}", received.tex);
    assert_eq!(braced_after(&received.tex, "\\documentclass"), "article");
    assert!(image.starts_with("paper-plot-") && image.ends_with(".png"), "{image}");
    let mut expected = vec!["paper.tex".to_string(), style, image];
    expected.sort();
    assert_eq!(received.files, expected);
}

/// `-t marker` chooses the compose template `marker` of `latex-rendered`, which a data
/// extension beside the document ships: the `.tex` file LaTeX received is the template's.
#[test]
fn the_compose_template_given_with_t_makes_the_latex_compiled() {
    let document = PAPER.replace("title: A paper\n", "title: A paper\nflm:\n  use: [marking]\n");
    let dir = folder("template", &document, Some(FAKE));
    let template = dir.join("_flmextensions/marking/templates/latex-rendered/marker");
    std::fs::create_dir_all(&template).unwrap();
    std::fs::write(dir.join("_flmextensions/marking/flmextension.yaml"), "extension:\n  description: A marker\n").unwrap();
    std::fs::write(
        template.join("flmtemplate.yaml"),
        "output: latex-rendered\nengine: minijinja\nmain: marker.tex\naccepts: all\n",
    )
    .unwrap();
    std::fs::write(template.join("marker.tex"), "%% the marker of pdf-latex's test\n{{ regions[\"content\"] }}\n").unwrap();
    let output = render(&dir, &["-p", "pdf-latex", "-t", "marker", "-o", "out/paper.pdf"]);
    assert_eq!(output.status.code(), Some(0), "{}", reported(&output));
    let received = received(&std::fs::read(dir.join("out/paper.pdf")).unwrap());
    assert!(received.tex.starts_with("%% the marker of pdf-latex's test\n"), "{}", received.tex);
    assert!(received.tex.contains("Hello"), "{}", received.tex);
}

/// `latexmk` exits with the status 1: the error `pdf-latex.failed` quotes the log's first
/// error, the status is 1, and the PDF that LaTeX produced is written.
#[test]
fn a_failing_latexmk_is_an_error_quoting_the_log_and_its_pdf_is_written() {
    let dir = folder("failing", PAPER, Some(FAILING));
    let output = render(&dir, &["-p", "pdf-latex", "-o", "out/paper.pdf"]);
    let reported = reported(&output);
    assert_eq!(output.status.code(), Some(1), "{reported}");
    assert!(
        reported.contains(
            "error: LaTeX must compile `paper.tex` without an error; `latexmk` exited with the status 1; the log \
             records:\n    ./paper.tex:3: Undefined control sequence.\n    l.3 \\foo\n"
        ),
        "{reported}"
    );
    assert!(reported.ends_with("1 error, 0 warnings\n"), "{reported}");
    assert_eq!(std::fs::read(dir.join("out/paper.pdf")).unwrap(), b"%PDF-partial\xff");
}

/// With no `latexmk` on `PATH`: the error `pdf-latex.latexmk-missing`, status 1, nothing
/// written.
#[test]
fn latexmk_missing_from_path_is_an_error() {
    let dir = folder("missing", PAPER, None);
    let output = render(&dir, &["-p", "pdf-latex", "-o", "out/paper.pdf"]);
    let reported = reported(&output);
    assert_eq!(output.status.code(), Some(1), "{reported}");
    assert!(written(&dir).is_empty());
}

/// A document with an error stops before LaTeX runs (status 1, nothing written); with
/// `--continue-with-errors`, LaTeX runs and the PDF is written, the status still 1.
#[test]
fn a_document_with_an_error_runs_latex_only_with_continue_with_errors() {
    let dir = folder("errors", &PAPER.replace("Hello", "Hello \\nosuchmacro{x}"), Some(FAKE));
    let output = render(&dir, &["-p", "pdf-latex", "-o", "out/paper.pdf"]);
    let said = reported(&output);
    assert_eq!(output.status.code(), Some(1), "{said}");
    assert!(said.contains("cannot resolve command ‘\\nosuchmacro’"), "{said}");
    assert!(recorded(&dir).is_empty());
    assert!(written(&dir).is_empty());
    let output = render(&dir, &["-p", "pdf-latex", "--continue-with-errors", "-o", "out/paper.pdf"]);
    assert_eq!(output.status.code(), Some(1), "{}", reported(&output));
    assert_eq!(recorded(&dir), [ARGS]);
    assert_eq!(written(&dir), ["paper.pdf"]);
}

/// `latexmk` running past `timeout_s` is stopped, with the program it started: the error
/// `pdf-latex.failed` gives the time and the setting, and nothing is written. Five seconds
/// leave the stand-in the time to record its program on a loaded machine.
#[test]
fn latexmk_running_past_timeout_s_is_an_error_giving_the_setting() {
    let document = PAPER.replace("title: A paper\n", "title: A paper\nflm:\n  features:\n    pdf-latex: {timeout_s: 5}\n");
    let dir = folder("timeout", &document, Some(SLOW));
    let output = render(&dir, &["-p", "pdf-latex", "-o", "out/paper.pdf"]);
    let reported = reported(&output);
    assert_eq!(output.status.code(), Some(1), "{reported}");
    assert!(
        reported.contains(
            "error: `latexmk` must finish compiling `paper.tex` within 5 seconds, the time that the setting \
             `timeout_s` of the feature `pdf-latex` allows"
        ),
        "{reported}"
    );
    assert!(written(&dir).is_empty());
    let started = recorded(&dir);
    assert_eq!(started.len(), 1, "{started:?}");
    assert!(ended(&started[0]), "the program `latexmk` started ({}) still runs", started[0]);
}

/// Ctrl+C while `latexmk` runs stops it, with the program it started, and then ends `flm` by
/// the interrupt signal, as the signal ends a program that does not handle it; nothing is
/// written.
#[test]
fn ctrl_c_stops_latexmk_with_the_program_it_started() {
    a_signal_stops_latexmk_with_the_program_it_started("interrupt", SIGINT);
}

/// `SIGTERM` while `latexmk` runs stops it, with the program it started, and then ends `flm`
/// by `SIGTERM`; nothing is written.
#[test]
fn sigterm_stops_latexmk_with_the_program_it_started() {
    a_signal_stops_latexmk_with_the_program_it_started("terminate", SIGTERM);
}

/// The quit signal (`Ctrl+\`) while `latexmk` runs stops it, with the program it started,
/// and then ends `flm` by the quit signal; nothing is written.
#[test]
fn the_quit_signal_stops_latexmk_with_the_program_it_started() {
    a_signal_stops_latexmk_with_the_program_it_started("quit", SIGQUIT);
}

/// Sends the signal `signal` to `flm` while the stand-in `SLOW` runs, in the test folder
/// `name`, and checks that `flm` ended by that signal, that the program the stand-in started
/// has ended, and that nothing is written. `flm` starts with the size of a core file limited
/// to zero, so that the quit signal, whose default action writes one where the system
/// allows it, writes none.
fn a_signal_stops_latexmk_with_the_program_it_started(name: &str, signal: i32) {
    let dir = folder(name, PAPER, Some(SLOW));
    let flm = command_after("ulimit -c 0", &dir, &["-p", "pdf-latex", "-o", "out/paper.pdf"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for_the_stand_in(&dir);
    let sent = Command::new("kill").args([format!("-{signal}"), flm.id().to_string()]).status().unwrap();
    assert!(sent.success(), "`kill -{signal}` reaches `flm`");
    let output = flm.wait_with_output().unwrap();
    assert_eq!(output.status.signal(), Some(signal), "{:?}: {}", output.status, reported(&output));
    let started = recorded(&dir);
    assert!(ended(&started[0]), "the program `latexmk` started ({}) still runs", started[0]);
    assert!(written(&dir).is_empty());
}

/// A hang-up that the program's parent left ignored, as `nohup` leaves it, stays ignored:
/// `flm` goes on, and the stand-in, which pauses for two seconds, finishes and gives the PDF.
#[test]
fn a_hang_up_left_ignored_by_the_parent_stays_ignored() {
    let paused = FAKE.replacen(
        "PATH=/usr/bin:/bin\n",
        "PATH=/usr/bin:/bin\nprintf 'started\\n' >> \"$FAKE_LATEXMK_RECORD\"\nsleep 2\n",
        1,
    );
    let dir = folder("hang-up-ignored", PAPER, Some(&paused));
    // The shell sets the hang-up to be ignored, which `flm` keeps.
    let flm = command_after("trap '' HUP", &dir, &["-p", "pdf-latex", "-o", "out/paper.pdf"])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    wait_for_the_stand_in(&dir);
    let sent = Command::new("kill").args([format!("-{SIGHUP}"), flm.id().to_string()]).status().unwrap();
    assert!(sent.success(), "`kill -{SIGHUP}` reaches `flm`");
    let output = flm.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{:?}: {}", output.status, reported(&output));
    assert_eq!(recorded(&dir), ["started", ARGS]);
    assert_eq!(written(&dir), ["paper.pdf"]);
}

/// Waits until the stand-in for `latexmk` has written its first record (`flm` built for
/// debugging takes several seconds to start it).
fn wait_for_the_stand_in(dir: &Path) {
    let until = Instant::now() + Duration::from_secs(120);
    while recorded(dir).is_empty() {
        assert!(Instant::now() < until, "the stand-in for `latexmk` never ran");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Whether the process `pid` has ended within five seconds (`kill -0` fails for it; an ended
/// process may last a moment until the system collects it).
fn ended(pid: &str) -> bool {
    let until = Instant::now() + Duration::from_secs(5);
    loop {
        let alive =
            Command::new("kill").args(["-0", pid]).stderr(Stdio::null()).status().is_ok_and(|status| status.success());
        if !alive {
            return true;
        }
        if Instant::now() >= until {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// `latexmk` failing with no error in its log: the error `pdf-latex.failed` quotes the last
/// lines of its standard error.
#[test]
fn a_failure_with_no_error_in_the_log_quotes_the_program_s_output() {
    let dir = folder("silent-log", PAPER, Some(SILENT_LOG));
    let output = render(&dir, &["-p", "pdf-latex", "-o", "out/paper.pdf"]);
    let reported = reported(&output);
    assert_eq!(output.status.code(), Some(1), "{reported}");
    assert!(
        reported.contains(
            "error: LaTeX must compile `paper.tex` without an error; `latexmk` exited with the status 12; its output \
             ends with:\n    Latexmk: one\n    Latexmk: two\n"
        ),
        "{reported}"
    );
    assert!(written(&dir).is_empty());
}

/// `latexmk` exiting with the status 0 without a PDF, or stopped by a signal: the error
/// `pdf-latex.failed` says so, and nothing is written.
#[test]
fn a_run_without_a_pdf_or_stopped_by_a_signal_is_an_error() {
    let dir = folder("no-pdf", PAPER, Some(NO_PDF));
    let output = render(&dir, &["-p", "pdf-latex", "-o", "out/paper.pdf"]);
    let reported_no_pdf = reported(&output);
    assert_eq!(output.status.code(), Some(1), "{reported_no_pdf}");
    assert!(
        reported_no_pdf.contains(
            "error: LaTeX must compile `paper.tex` without an error; `latexmk` exited with the status 0 and wrote no \
             file `paper.pdf`\n"
        ),
        "{reported_no_pdf}"
    );
    assert!(written(&dir).is_empty());
    let dir = folder("killed", PAPER, Some(KILLED));
    let output = render(&dir, &["-p", "pdf-latex", "-o", "out/paper.pdf"]);
    let reported_killed = reported(&output);
    assert_eq!(output.status.code(), Some(1), "{reported_killed}");
    assert!(
        reported_killed
            .contains("error: LaTeX must compile `paper.tex` without an error; `latexmk` was stopped by a signal\n"),
        "{reported_killed}"
    );
    assert!(written(&dir).is_empty());
}

/// A `timeout_s` that is not a positive whole number stops the command (status 2).
#[test]
fn a_timeout_s_that_is_not_a_positive_whole_number_stops_the_command() {
    let document = PAPER.replace("title: A paper\n", "title: A paper\nflm:\n  features:\n    pdf-latex: {timeout_s: 0}\n");
    let dir = folder("timeout-zero", &document, Some(FAKE));
    let output = render(&dir, &["-p", "pdf-latex", "-o", "out/paper.pdf"]);
    let reported = reported(&output);
    assert_eq!(output.status.code(), Some(2), "{reported}");
    assert!(
        reported.contains(
            "the setting `timeout_s` of the feature `pdf-latex` must be a positive whole number of seconds, at most \
             9223372036854775, not `0`"
        ),
        "{reported}"
    );
    assert!(recorded(&dir).is_empty());
}

// --- gated ---------------------------------------------------------------------------------

/// The variable that enables the compilation by the real `latexmk`.
const LATEX_COMPILE: &str = "FLM_TESTS_LATEX_COMPILE";

/// The real `latexmk`, found on the test's own `PATH`, compiles a small document into a PDF.
#[test]
fn the_real_latexmk_compiles_a_small_document() {
    if matches!(std::env::var(LATEX_COMPILE).unwrap_or_default().trim(), "" | "0") {
        println!("skipped: set {LATEX_COMPILE} to compile a document with the real latexmk");
        return;
    }
    let dir = folder("real", PAPER, None);
    let output = Command::new(env!("CARGO_BIN_EXE_flm"))
        .args(["render", "-p", "pdf-latex", "-o", "out/paper.pdf", "paper.flm"])
        .current_dir(&dir)
        .env("HOME", &dir)
        .env_remove("FLMPATH")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", reported(&output));
    assert_eq!(written(&dir), ["paper.pdf"]);
    assert!(std::fs::read(dir.join("out/paper.pdf")).unwrap().starts_with(b"%PDF-"));
}

#[cfg(feature = "wasm")]
#[path = "../../flm-ext/tests/common/guest.rs"]
#[allow(dead_code)]
mod guest;

/// The extension's WebAssembly form, built by `extensions/rust/build-wasm.sh` into a folder
/// that `--extensions-dir` points at, is found before the extension compiled in and gives
/// the same PDF for the document of the first test.
#[cfg(feature = "wasm")]
#[test]
fn the_webassembly_form_built_by_the_script_gives_the_same_pdf() {
    if let Some(line) = guest::skipped() {
        println!("{line}");
        return;
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let modules = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-cli-pdf-latex-wasm");
    let _ = std::fs::remove_dir_all(&modules);
    // The script's cargo run builds in a target folder of its own, apart from the folder
    // of the cargo run that runs this test.
    let script = Command::new(root.join("extensions/rust/build-wasm.sh"))
        .arg("--out")
        .arg(&modules)
        .arg("profile-pdf-latex")
        .env("CARGO_TARGET_DIR", root.join("target/wasm-guest"))
        .output()
        .unwrap();
    assert!(script.status.success(), "the script must build the module:\n{}", String::from_utf8_lossy(&script.stderr));
    let mut shipped: Vec<String> =
        std::fs::read_dir(modules.join("profile-pdf-latex")).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    shipped.sort();
    assert_eq!(shipped, ["README.md", "flmextension.yaml", "profile_pdf_latex.wasm"]);

    let native = folder("wasm-native", PAPER, Some(FAKE));
    let output = render(&native, &["-p", "pdf-latex", "-o", "out/paper.pdf"]);
    assert_eq!(output.status.code(), Some(0), "{}", reported(&output));
    let wasm = folder("wasm-module", PAPER, Some(FAKE));
    let output = render(&wasm, &["--extensions-dir", modules.to_str().unwrap(), "-v", "-p", "pdf-latex", "-o", "out/paper.pdf"]);
    let said = reported(&output);
    assert_eq!(output.status.code(), Some(0), "{said}");
    // The module was loaded, not the extension compiled in (`-v` lists the packages in use).
    assert!(said.contains(modules.join("profile-pdf-latex").to_str().unwrap()) && !said.contains("bundled:rust/profile-pdf-latex"), "{said}");
    assert_eq!(std::fs::read(wasm.join("out/paper.pdf")).unwrap(), std::fs::read(native.join("out/paper.pdf")).unwrap());
}
