//! The host service `programs` (`flm_main::host_programs`): `find` over the folders of `PATH`
//! and the extra folders with their wildcards, `run` in a fresh temporary folder, and the
//! service offered only when the embedding program asks. The tests that start a program
//! use `sh` and run on Unix only.

#![cfg(feature = "fs")]

use std::path::{Path, PathBuf};
use std::sync::Arc;

use flm_ext::engine::ExtensionIdentity;
use flm_ext::host::{HostAnswer, HostCaller, HostService, HostServiceError};
use flm_main::host_programs::ProgramsService;
use serde_json::{Value, json};

fn caller() -> HostCaller {
    HostCaller {
        extension: ExtensionIdentity { identity: String::from("/x/runner"), stem: String::from("runner") },
        package: String::from("_"),
        digest_input: Arc::from(Vec::new()),
    }
}

fn call(method: &str, args: Value) -> Result<HostAnswer, HostServiceError> {
    ProgramsService::new().call(&caller(), method, &args)
}

/// The data a call answered.
fn data(answer: Result<HostAnswer, HostServiceError>) -> Value {
    match answer {
        Ok(HostAnswer::Data(value)) => value,
        other => panic!("{other:?}"),
    }
}

/// A fresh folder `name` under the build folder.
fn scratch(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-main-host-programs-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// Writes a program `name` into `folder` (a shell script with an execute permission).
#[cfg(unix)]
fn program(folder: &Path, name: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(folder).unwrap();
    let path = folder.join(name);
    std::fs::write(&path, "#!/bin/sh\necho hi\n").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

fn text(path: &Path) -> Value {
    Value::String(path.to_str().unwrap().to_string())
}

// --- find -----------------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn find_answers_a_path_for_sh_and_null_for_a_name_that_does_not_exist() {
    let found = data(call("find", json!({ "names": ["sh"] })));
    let path = PathBuf::from(found.as_str().unwrap());
    assert!(path.is_absolute() && path.ends_with("sh"), "{found}");
    assert_eq!(data(call("find", json!({ "names": ["flm-no-such-program"] }))), Value::Null);
    assert_eq!(data(call("find", json!({ "names": [] }))), Value::Null);
    // A name holding a separator is a path: it answers itself when it is a program.
    assert_eq!(data(call("find", json!({ "names": [found.clone()] }))), found);
    assert_eq!(data(call("find", json!({ "names": ["/flm/no/such/program"] }))), Value::Null);
}

#[cfg(unix)]
#[test]
fn find_tries_the_names_in_order_and_finds_one_in_an_extra_folder() {
    let root = scratch("order");
    let extra = root.join("bin");
    let first = program(&extra, "flm-test-first");
    program(&extra, "flm-test-second");
    // A file without an execute permission is not a program.
    std::fs::write(extra.join("flm-test-plain"), "x").unwrap();
    let folders = json!([extra.to_str().unwrap()]);
    let found = |names: Value| data(call("find", json!({ "names": names, "folders": folders })));
    assert_eq!(found(json!(["flm-test-first", "flm-test-second"])), text(&first));
    assert_eq!(found(json!(["flm-test-missing", "flm-test-second"])), text(&extra.join("flm-test-second")));
    // The first name found wins, although `sh` is in a folder of `PATH`, searched first.
    assert_eq!(found(json!(["flm-test-first", "sh"])), text(&first));
    assert_eq!(found(json!(["flm-test-plain"])), Value::Null);
    // Without the extra folder, the program is not found.
    assert_eq!(data(call("find", json!({ "names": ["flm-test-first"] }))), Value::Null);
}

#[cfg(unix)]
#[test]
fn a_wildcard_inside_a_folder_name_chooses_the_highest_version() {
    let root = scratch("wildcard");
    for version in ["9.56.1", "10.02.1", "2", "10.1.5"] {
        program(&root.join(format!("tool {version}")).join("bin"), "flm-test-tool");
    }
    // A matching name that is a file, not a folder, is passed over.
    std::fs::write(root.join("tool 99.0"), "x").unwrap();
    let find = |pattern: String| data(call("find", json!({ "names": ["flm-test-tool"], "folders": [pattern] })));
    // `10.02.1` before `10.1.5` and `9.56.1`; `2` does not match `*.*`.
    assert_eq!(find(format!("{}/tool *.*/bin", root.display())), text(&root.join("tool 10.02.1/bin/flm-test-tool")));
    // Several `*` in one name.
    assert_eq!(find(format!("{}/t*l 9*/bin", root.display())), text(&root.join("tool 9.56.1/bin/flm-test-tool")));
    // Where the newest version lacks the program, the next one answers.
    std::fs::remove_file(root.join("tool 10.02.1/bin/flm-test-tool")).unwrap();
    assert_eq!(find(format!("{}/tool *.*/bin", root.display())), text(&root.join("tool 10.1.5/bin/flm-test-tool")));
}

#[test]
fn a_wildcard_matching_nothing_finds_nothing() {
    let root = scratch("wildcard-nothing");
    let pattern = format!("{}/none*/bin", root.display());
    assert_eq!(data(call("find", json!({ "names": ["flm-test-tool"], "folders": [pattern] }))), Value::Null);
    let pattern = format!("{}/no-such-folder/x*", root.display());
    assert_eq!(data(call("find", json!({ "names": ["flm-test-tool"], "folders": [pattern] }))), Value::Null);
}

#[test]
fn find_s_arguments_of_the_wrong_shape_are_refused() {
    assert_eq!(call("find", json!({ "names": "sh" })).unwrap_err().detail, "`names` must be a list of texts");
    assert_eq!(call("find", json!({})).unwrap_err().detail, "`names` must be a list of texts");
    assert_eq!(call("find", json!({ "names": ["sh"], "folders": [1] })).unwrap_err().detail, "`folders` must be a list of texts");
    assert_eq!(
        call("start", json!({})).unwrap_err().detail,
        "the method of the host service `programs` must be `find` or `run`, not `start`"
    );
}

// --- run ------------------------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn run_writes_the_files_runs_the_program_and_collects_its_files() {
    let script = r#"pwd > where.txt
cat sub/in.txt > out.txt
printf '%s|%s\n' "$0" "$1" >> out.txt
cat >> out.txt
od -An -tx1 data.bin | tr -d ' \n' >> out.txt
printf '\377\000\001' > out.bin
echo "to standard output"
echo "to standard error" >&2
exit 3"#;
    let answer = data(call(
        "run",
        json!({
            "program": "sh",
            "args": ["-c", script, "zero", "one"],
            "files": { "sub/in.txt": { "text": "input text\n" }, "data.bin": { "base64": "AP8=" } },
            "stdin": "from stdin\n",
            "collect": ["out.txt", "out.bin", "where.txt", "never-written.txt"],
        }),
    ));
    assert_eq!(answer["status"], json!(3));
    assert_eq!(answer["stdout"], json!("to standard output\n"));
    assert_eq!(answer["stderr"], json!("to standard error\n"));
    let files = answer["files"].as_object().unwrap();
    assert_eq!(files.len(), 3, "{answer}");
    assert_eq!(files["out.txt"], json!({ "text": "input text\nzero|one\nfrom stdin\n00ff" }));
    assert_eq!(files["out.bin"], json!({ "base64": "/wAB" }));
    // The program ran in a fresh folder of its own, which is removed after.
    let folder = PathBuf::from(files["where.txt"]["text"].as_str().unwrap().trim_end());
    assert!(folder.file_name().unwrap().to_str().unwrap().starts_with("flm-programs-"), "{folder:?}");
    assert!(!folder.exists(), "{folder:?}");
}

#[cfg(unix)]
#[test]
fn run_takes_a_path_as_find_answers_it_and_without_stdin_the_input_is_empty() {
    let sh = data(call("find", json!({ "names": ["sh"] })));
    let answer = data(call("run", json!({ "program": sh, "args": ["-c", "cat; echo done"] })));
    assert_eq!(answer, json!({ "status": 0, "stdout": "done\n", "stderr": "", "files": {} }));
}

#[test]
fn a_path_absolute_or_leaving_the_folder_is_refused() {
    let refused = |args: Value| call("run", args).unwrap_err().detail;
    assert_eq!(
        refused(json!({ "program": "sh", "files": { "../x.txt": { "text": "" } } })),
        "the path `../x.txt` must stay inside the program's temporary folder"
    );
    assert_eq!(
        refused(json!({ "program": "sh", "collect": ["a/../../x.txt"] })),
        "the path `a/../../x.txt` must stay inside the program's temporary folder"
    );
    assert_eq!(
        refused(json!({ "program": "sh", "collect": ["/etc/passwd"] })),
        "the path `/etc/passwd` must be relative to the program's temporary folder"
    );
    assert_eq!(
        refused(json!({ "program": "sh", "files": { "": { "text": "x" } } })),
        "the path `` must name a file inside the program's temporary folder"
    );
    assert_eq!(
        refused(json!({ "program": "sh", "collect": ["a/.."] })),
        "the path `a/..` must name a file inside the program's temporary folder"
    );
}

#[cfg(unix)]
#[test]
fn a_collected_path_naming_a_folder_is_absent_from_the_files() {
    let answer = data(call(
        "run",
        json!({ "program": "sh", "args": ["-c", "mkdir d; echo x > d/f.txt"], "collect": ["d", "d/f.txt"] }),
    ));
    assert_eq!(answer, json!({ "status": 0, "stdout": "", "stderr": "", "files": { "d/f.txt": { "text": "x\n" } } }));
}

#[test]
fn run_s_arguments_of_the_wrong_shape_are_refused() {
    let refused = |args: Value| call("run", args).unwrap_err().detail;
    assert_eq!(refused(json!({ "args": [] })), "`program` must be a text");
    assert_eq!(refused(json!({ "program": "sh", "args": "-c" })), "`args` must be a list of texts");
    assert_eq!(refused(json!({ "program": "sh", "collect": [1] })), "`collect` must be a list of texts");
    assert_eq!(refused(json!({ "program": "sh", "stdin": 1 })), "`stdin` must be a text");
    assert_eq!(refused(json!({ "program": "sh", "timeout_ms": 0 })), "`timeout_ms` must be a positive whole number");
    assert_eq!(
        refused(json!({ "program": "sh", "files": ["a"] })),
        "`files` must be a map from a relative path to `{ text }` or `{ base64 }`"
    );
    assert_eq!(
        refused(json!({ "program": "sh", "files": { "a": { "text": "x", "base64": "" } } })),
        "the file `a` given to the program `sh` must be `{ text }` or `{ base64 }`"
    );
    assert_eq!(
        refused(json!({ "program": "sh", "files": { "a": { "base64": "*" } } })),
        "`base64` of the file `a` given to the program `sh` must be base64 in the standard alphabet, with padding"
    );
}

#[test]
fn a_missing_program_is_an_error_naming_the_extension_and_the_program() {
    let detail = call("run", json!({ "program": "flm-no-such-program" })).unwrap_err().detail;
    assert_eq!(detail, "the program `flm-no-such-program` that the extension `runner` runs must be in one of the folders of `PATH`");
    let detail = call("run", json!({ "program": "/flm/no/such/program" })).unwrap_err().detail;
    assert!(
        detail.starts_with("the program `/flm/no/such/program` that the extension `runner` runs could not be started: "),
        "{detail}"
    );
}

#[cfg(unix)]
#[test]
fn a_program_running_past_its_time_is_stopped_with_an_error_naming_the_extension_and_the_program() {
    let started = std::time::Instant::now();
    let detail = call("run", json!({ "program": "sh", "args": ["-c", "exec sleep 5"], "timeout_ms": 200 }))
        .unwrap_err()
        .detail;
    assert_eq!(detail, "the program `sh` that the extension `runner` runs must finish within 200 milliseconds");
    assert!(started.elapsed() < std::time::Duration::from_secs(4), "{:?}", started.elapsed());
}

/// Whether the process `pid` has ended within five seconds (`kill -0` fails for it; an ended
/// process may last a moment until the system collects it).
#[cfg(unix)]
fn ended(pid: &str) -> bool {
    let until = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let alive = std::process::Command::new("kill")
            .args(["-0", pid])
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success());
        if !alive {
            return true;
        }
        if std::time::Instant::now() >= until {
            return false;
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(unix)]
#[test]
fn a_program_past_its_time_is_stopped_with_the_programs_it_started() {
    let folder = scratch("group");
    // The program starts `sleep`, writes its process ID, and waits for it. Five seconds leave
    // the program the time to write the ID on a loaded machine before it is stopped.
    let waiting = folder.join("waiting.pid");
    let script = format!("sleep 30 & echo $! > '{}'; wait", waiting.display());
    let detail = call("run", json!({ "program": "sh", "args": ["-c", script], "timeout_ms": 5000 })).unwrap_err().detail;
    assert_eq!(detail, "the program `sh` that the extension `runner` runs must finish within 5000 milliseconds");
    let pid = std::fs::read_to_string(&waiting).unwrap();
    assert!(ended(pid.trim()), "the program's `sleep` ({}) still runs", pid.trim());
    // The program ends at once, and the `sleep` it started holds its output open.
    let holding = folder.join("holding.pid");
    let script = format!("sleep 30 & echo $! > '{}'", holding.display());
    let detail = call("run", json!({ "program": "sh", "args": ["-c", script], "timeout_ms": 5000 })).unwrap_err().detail;
    assert_eq!(detail, "the program `sh` that the extension `runner` runs must finish within 5000 milliseconds");
    let pid = std::fs::read_to_string(&holding).unwrap();
    assert!(ended(pid.trim()), "the program's `sleep` ({}) still runs", pid.trim());
}

// --- the service offered only on request ----------------------------------------------------

#[test]
fn the_service_is_offered_only_when_the_embedding_program_asks() {
    use flm_main::extensions::ExtensionFinder;
    use flm_main::files::{BuildFiles, MemoryFiles, ServiceFiles};
    use flm_main::packages::loader;
    use flm_main::session::{Input, Request};

    let files = Arc::new(MemoryFiles::new());
    let service = ServiceFiles::new(files.clone());
    let finder = Arc::new(ExtensionFinder::new(BuildFiles::new(files), service.clone(), Vec::new()));
    let mut request = Request::new(Input::Text(String::new()));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    assert!(!request.offer_programs);
    assert_eq!(loader(&finder, &request, &service, Path::new("")).host_services(), ["files"]);
    request.offer_programs = true;
    assert_eq!(loader(&finder, &request, &service, Path::new("")).host_services(), ["files", "programs"]);
}
