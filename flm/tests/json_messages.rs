//! `--json-messages`: what the program writes on standard error, as one JSON record per
//! line, run through the binary.
//!
//! Each test runs the program in a folder of its own under the cargo target's folder for
//! tests, holding a copy of the documents of `tests/documents/json-messages/`, with that
//! folder as the home folder and `FLMPATH` unset. Every line of standard error is parsed as
//! JSON; the first is the header.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

/// A fresh folder of the test's own, `NAME` under the cargo target's folder for tests,
/// holding a copy of the documents of `tests/documents/json-messages/`, with the
/// extensions of its folder `_flmextensions`.
fn folder(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("json-messages").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    copy_folder(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/json-messages"), &dir);
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

/// Runs the program with `args` in `dir`.
fn flm(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flm"))
        .args(args)
        .current_dir(dir)
        .env("HOME", dir)
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE")
        .output()
        .unwrap()
}

/// Every line of standard error of `output`, each parsed as JSON, but the `progress`
/// records, which a build that lasts past 500 ms writes; the first must be the header of the
/// records.
fn records(output: &Output) -> Vec<Value> {
    let stderr = String::from_utf8(output.stderr.clone()).unwrap();
    let records: Vec<Value> = stderr
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap_or_else(|error| panic!("{error}: {line:?}")))
        .filter(|record| record["kind"] != "progress")
        .collect();
    let header = records.first().unwrap_or_else(|| panic!("no record: {output:?}"));
    assert_eq!(header, &serde_json::json!({"kind": "messages", "flm": env!("CARGO_PKG_VERSION"), "format": 1}));
    assert_eq!(records.iter().filter(|record| record["kind"] == "messages").count(), 1, "one header");
    records
}

/// `flm check` on a document with two errors and one warning writes the header, one record
/// per diagnostic with the identifier of its condition and its place (the line, the column
/// in bytes and the byte offset counted from the top of the file, past its front matter),
/// then the counts; the exit status is unchanged.
#[test]
fn check_writes_a_record_per_diagnostic_then_the_counts() {
    let dir = folder("check");
    let text = std::fs::read_to_string(dir.join("errors.flm")).unwrap();
    let output = flm(&dir, &["check", "--json-messages", "--math", "passthrough", "errors.flm"]);
    assert_eq!(output.status.code(), Some(1));
    let records = records(&output);
    let kinds: Vec<&str> = records.iter().map(|record| record["kind"].as_str().unwrap()).collect();
    assert_eq!(kinds, ["messages", "diagnostic", "diagnostic", "diagnostic", "summary"]);
    let offset = |piece: &str| text.find(piece).unwrap();
    let expected = [
        ("error", "core.specs.unresolvable-command", 4, 3, offset("\\nosuch")),
        ("error", "latexlike.environments.unknown-environment", 6, 8, offset("nosuchenv")),
        ("warning", "flm.refs.unresolved", 4, 33, offset("\\ref")),
    ];
    for (record, (severity, id, line, column, offset)) in records[1..4].iter().zip(expected) {
        assert_eq!(record["severity"], severity, "{record}");
        assert_eq!(record["id"], id, "{record}");
        assert!(record["message"].is_string() && record["data"].is_object(), "{record}");
        let place = &record["place"];
        assert_eq!(place["file"], "errors.flm", "{record}");
        assert_eq!((place["line"].as_u64(), place["column"].as_u64()), (Some(line), Some(column)), "{record}");
        assert_eq!(place["offset"].as_u64(), Some(offset as u64), "{record}");
        assert!(record["frames"].is_array());
    }
    assert!(offset("\\nosuch") > 40, "the offset counts the front matter");
    assert_eq!(records[4], serde_json::json!({"kind": "summary", "errors": 2, "warnings": 1}));
    assert!(output.stdout.is_empty());
}

/// The option is accepted before the command's name too, and the records are the same.
#[test]
fn the_option_is_accepted_before_the_command_s_name() {
    let dir = folder("before");
    let after = flm(&dir, &["check", "--json-messages", "--math", "passthrough", "errors.flm"]);
    let before = flm(&dir, &["--json-messages", "check", "--math", "passthrough", "errors.flm"]);
    assert_eq!(before.status.code(), Some(1));
    assert_eq!(records(&before), records(&after));
}

/// `flm render` of a document whose front matter uses a package that does not exist writes
/// the header and one record of the problem that stopped the work; status 2.
#[test]
fn a_problem_that_stops_the_work_is_one_record() {
    let dir = folder("problem");
    let output = flm(&dir, &["render", "--json-messages", "--math", "passthrough", "unknown-package.flm"]);
    assert_eq!(output.status.code(), Some(2));
    let records = records(&output);
    assert_eq!(records.len(), 2, "{records:?}");
    let problem = &records[1];
    assert_eq!(problem["kind"], "problem");
    assert_eq!(problem["severity"], "error");
    assert!(problem["message"].is_string(), "{problem}");
    assert_eq!(problem["place"]["file"], "unknown-package.flm");
    assert_eq!(problem["place"]["line"], 3);
}

/// `-v` adds a record of the kind `log` for each line that tells what the program does; an
/// error of the arguments is a record of the kind `problem`.
#[test]
fn verbose_lines_are_log_records() {
    let dir = folder("verbose");
    let output = flm(&dir, &["--json-messages", "render", "-v", "-p", "text", "--math", "passthrough", "-c", "Hello."]);
    assert!(output.status.success(), "{output:?}");
    let logs = records(&output);
    assert!(logs[1..].iter().all(|record| record["kind"] == "log"), "{logs:?}");
    let texts: Vec<&str> = logs[1..].iter().map(|record| record["text"].as_str().unwrap()).collect();
    assert!(texts.contains(&"profile text"), "{texts:?}");
    assert!(texts.iter().any(|text| text.starts_with("packages ")), "{texts:?}");
    assert!(texts.iter().any(|text| text.starts_with("read ")), "{texts:?}");

    let output = flm(&dir, &["render", "--json-messages", "-V", "errors.flm"]);
    assert_eq!(output.status.code(), Some(2));
    let records = records(&output);
    assert_eq!(records.len(), 2);
    assert_eq!(records[1]["kind"], "problem");
    assert!(records[1].get("place").is_none());
}

/// A run with nothing to report writes nothing on standard error, not even the header.
#[test]
fn a_run_with_nothing_to_report_writes_nothing() {
    let dir = folder("quiet");
    let output = flm(&dir, &["--json-messages", "check", "--math", "passthrough", "-c", "Hello."]);
    assert!(output.status.success() && output.stderr.is_empty(), "{output:?}");
}

/// The usage text of a command line that the parser rejects stays text, and `flm dump` does
/// not accept the option.
#[test]
fn the_usage_text_and_flm_dump_stay_text() {
    let dir = folder("outside");
    let output = flm(&dir, &["--json-messages", "check", "--no-such-option"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with("error: unexpected argument"), "{stderr}");
    let output = flm(&dir, &["--json-messages", "dump", "errors.flm"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.starts_with("error: "), "{stderr}");
}

/// A log line at the level `Info` is written under `-v` and not without it, and a line at
/// the level `Warn` in both cases: the extension `talk` writes on its console one line of
/// each, the first when it is loaded and the second when its feature registers. Under
/// `--json-messages`, each is a `log` record holding the line's level, the extension's name
/// as its source, and its text, for every command that prepares a document (`flm info`
/// has no `-v`).
#[cfg(feature = "js")]
#[test]
fn an_info_log_line_is_written_under_v_only() {
    let dir = folder("console");
    for verbose in [false, true] {
        let args = [&["check", "--math", "passthrough", "talk.flm"][..], if verbose { &["-v"] } else { &[] }].concat();
        let output = flm(&dir, &args);
        assert_eq!(output.status.code(), Some(0), "{output:?}");
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert_eq!(stderr.contains("talk: hello from the extension\n"), verbose, "{args:?}: {stderr}");
        assert!(stderr.contains("talk: warning: registering\n"), "{args:?}: {stderr}");
    }
    for (verbose, args) in [
        (false, &["--json-messages", "check", "--math", "passthrough", "talk.flm"][..]),
        (true, &["--json-messages", "check", "-v", "--math", "passthrough", "talk.flm"][..]),
        (false, &["--json-messages", "info", "--json", "--math", "passthrough", "-d", "talk.flm", "features/talk"][..]),
    ] {
        let output = flm(&dir, args);
        assert_eq!(output.status.code(), Some(0), "{args:?}: {output:?}");
        let records = records(&output);
        assert!(records[1..].iter().all(|record| record["kind"] == "log"), "{args:?}: {records:?}");
        let logs: Vec<(&str, &str, &str)> = records
            .iter()
            .filter(|record| record["kind"] == "log")
            .map(|record| {
                (record["level"].as_str().unwrap(), record["source"].as_str().unwrap(), record["text"].as_str().unwrap())
            })
            .collect();
        assert_eq!(logs.contains(&("info", "talk", "hello from the extension")), verbose, "{args:?}: {records:?}");
        assert!(logs.contains(&("warn", "talk", "registering")), "{args:?}: {records:?}");
    }
}
