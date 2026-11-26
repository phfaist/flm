//! `flm watch` and the bare form `flm FILE`, run as the binary over files under the build
//! folder: the link the program gives, the page served, an update after a change, Ctrl+C; the
//! browser opened through a program the test supplies (the environment variable
//! `BROWSER`); the editor a place opens in, among programs the test puts on the search path
//! (the environment variable `PATH`); what the preview refuses. (What `-V` under `render`
//! opens is tested in process, in the crate itself, since it opens a window otherwise.)
//! Every wait has an upper bound and ends as soon as what is expected arrives; the system
//! chooses the port (`--port 0`), and no browser window or editor opens. Each test runs
//! the program under `--json-messages`, and reads the link and the lines that tell what the
//! program does from its records, as an editor or a script reads them.

use std::io::{BufRead, BufReader, Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::{Duration, Instant};

/// The upper bound of every wait.
const BOUND: Duration = Duration::from_secs(60);

/// A fresh folder for one test, under the build folder.
fn folder(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-watch-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

const PAPER: &str = "---\ntitle: A paper\n---\nHello world.\n";

/// A program standing for the browser: it writes the address it is given to
/// `opened.txt` beside it. The test reads the file; no window opens.
#[cfg(unix)]
fn recorder(dir: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    let program = dir.join("browser.sh");
    let record = dir.join("opened.txt");
    std::fs::write(&program, format!("#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\n", record.display())).unwrap();
    std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
    program
}

/// What the recorder was given so far, a line each.
fn opened(dir: &Path) -> Vec<String> {
    std::fs::read_to_string(dir.join("opened.txt")).unwrap_or_default().lines().map(String::from).collect()
}

/// A program named `name` in the folder `bin`, standing for an editor: it writes the words
/// it is given, a line each, to the file it returns, `name.txt` beside it. No editor opens.
#[cfg(unix)]
fn editor(bin: &Path, name: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(bin).unwrap();
    let record = bin.join(format!("{name}.txt"));
    let script = format!("#!/bin/sh\nfor word in \"$@\"; do printf '%s\\n' \"$word\"; done >> '{}'\n", record.display());
    std::fs::write(bin.join(name), script).unwrap();
    std::fs::set_permissions(bin.join(name), std::fs::Permissions::from_mode(0o755)).unwrap();
    record
}

/// Asks the preview at `address` to open a place of `paper.flm`, as a click on the page does.
fn open_place(address: &str) {
    assert_eq!(post(address, "/.flm-preview/open", r#"{"file": "paper.flm", "line": 4, "column": 1}"#), 204);
}

/// Waits until `done` holds, within the bound.
fn eventually(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < BOUND, "{what}: not within the bound");
        thread::sleep(Duration::from_millis(20));
    }
}

/// The program running under `--json-messages`, its standard error read one record at a
/// time.
struct Running {
    child: Child,
    lines: Receiver<String>,
    /// The records read so far, the header first.
    seen: Vec<serde_json::Value>,
}

impl Running {
    fn start(dir: &Path, args: &[&str], browser: &Path) -> Running {
        Running::start_with_path(dir, args, browser, None)
    }

    /// [`Running::start`], the variable `PATH` set to `path` when it is given.
    fn start_with_path(dir: &Path, args: &[&str], browser: &Path, path: Option<&Path>) -> Running {
        let mut command = Command::new(env!("CARGO_BIN_EXE_flm"));
        command.arg("--json-messages").args(args).current_dir(dir).env("BROWSER", browser).env("HOME", dir).env_remove("FLMPATH");
        if let Some(path) = path {
            command.env("PATH", path);
        }
        let mut child = command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn().unwrap();
        let stderr = child.stderr.take().unwrap();
        let (send, lines) = channel();
        thread::spawn(move || {
            for line in BufReader::new(stderr).lines().map_while(Result::ok) {
                let _ = send.send(line);
            }
        });
        Running { child, lines, seen: Vec::new() }
    }

    /// Reads records until one satisfies `wanted`, and gives that record; `what` names it
    /// for the message of a wait that ends unanswered.
    fn record_where(&mut self, what: &str, wanted: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
        loop {
            let line = self.lines.recv_timeout(BOUND).unwrap_or_else(|_| panic!("{what} within the bound: {:?}", self.seen));
            let record: serde_json::Value = serde_json::from_str(&line).unwrap_or_else(|error| panic!("{error}: {line}"));
            self.seen.push(record.clone());
            if wanted(&record) {
                return record;
            }
        }
    }

    /// Reads records until a record of the kind `log` holds `text` in its text.
    fn logged(&mut self, text: &str) {
        self.record_where(&format!("`{text}`"), |record| {
            record["kind"] == "log" && record["text"].as_str().is_some_and(|logged| logged.contains(text))
        });
    }

    /// The address of the preview, `http://HOST:PORT/`, from the link the program gave (the
    /// record `preview-link`).
    fn address(&mut self) -> String {
        let record = self.record_where("the link", |record| record["kind"] == "preview-link");
        let rest = record["url"].as_str().expect("a link");
        assert!(rest.starts_with("http://"), "{rest}");
        // `http://HOST:PORT/`: up to the first slash after the scheme's two.
        let end = rest["http://".len()..].find('/').expect("a path") + "http://".len() + 1;
        rest[..end].to_string()
    }

    /// Sends the interrupt signal, as Ctrl+C does, and gives the exit status.
    #[cfg(unix)]
    fn interrupt(mut self) -> Option<i32> {
        let pid = self.child.id().to_string();
        let sent = Command::new("kill").args(["-INT", &pid]).status().unwrap();
        assert!(sent.success());
        let start = Instant::now();
        loop {
            if let Some(status) = self.child.try_wait().unwrap() {
                return status.code();
            }
            assert!(start.elapsed() < BOUND, "the program ends within the bound");
            thread::sleep(Duration::from_millis(20));
        }
    }
}

/// The host and port of an address `http://HOST:PORT/`.
fn host(address: &str) -> String {
    address.trim_start_matches("http://").trim_end_matches('/').to_string()
}

/// The body of `GET target` at `address`.
fn get(address: &str, target: &str) -> String {
    let mut connection = TcpStream::connect(host(address)).unwrap();
    connection.set_read_timeout(Some(BOUND)).unwrap();
    write!(connection, "GET {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    let mut bytes = Vec::new();
    connection.read_to_end(&mut bytes).unwrap();
    let text = String::from_utf8(bytes).unwrap();
    let split = text.find("\r\n\r\n").expect("a head");
    assert!(text.starts_with("HTTP/1.1 200"), "{}", &text[..split]);
    text[split + 4..].to_string()
}

/// A client of the stream of messages, connected once its first message, `hello`, came: the
/// data line of each later message named `update`.
fn updates(address: &str) -> Receiver<String> {
    let mut connection = TcpStream::connect(host(address)).unwrap();
    connection.set_read_timeout(Some(BOUND)).unwrap();
    write!(connection, "GET /.flm-preview/events HTTP/1.1\r\nHost: localhost\r\n\r\n").unwrap();
    let mut lines = BufReader::new(connection).lines().map_while(Result::ok);
    assert!(lines.any(|line| line == "event: hello"), "the first message");
    let (send, received) = channel();
    thread::spawn(move || {
        let mut event = String::new();
        for line in lines {
            if let Some(name) = line.strip_prefix("event: ") {
                event = name.to_string();
            } else if let Some(data) = line.strip_prefix("data: ")
                && event == "update"
                && send.send(data.to_string()).is_err()
            {
                return;
            }
        }
    });
    received
}

// --- the program from end to end, and the browser ------------------------------------------------

#[cfg(unix)]
#[test]
fn the_preview_serves_the_page_updates_it_and_ends_on_the_interrupt_both_spellings() {
    for (name, args) in [("bare", &["paper.flm", "--port", "0"][..]), ("watch", &["watch", "paper.flm", "--port", "0"][..])] {
        let dir = folder(name);
        std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
        let browser = recorder(&dir);
        let mut running = Running::start(&dir, args, &browser);
        let address = running.address();
        running.logged("build 1 done");
        let page = get(&address, "/");
        assert!(page.contains("Hello world."), "{page}");
        assert!(page.contains("flm.preview.script"), "{page}");
        // The browser was opened on the link given, the tools page's.
        let tools = format!("{address}.flm-preview/");
        assert!(running.seen.iter().any(|record| record["kind"] == "preview-link" && record["url"] == *tools), "{:?}", running.seen);
        eventually("the browser opened", || opened(&dir) == [tools.clone()]);
        assert!(get(&address, "/.flm-preview/").contains("/.flm-preview/assets/tools.js"));

        let updates = updates(&address);
        std::fs::write(dir.join("paper.flm"), "---\ntitle: A paper\n---\nHello again.\n").unwrap();
        running.logged("changed paper.flm");
        // The update that carries the change. The system may report the file's first
        // writing late, which causes a build of the same text before it.
        loop {
            let update = updates.recv_timeout(BOUND).expect("an update within the bound");
            if update.contains("Hello again.") {
                assert!(update.contains("\"page\":\"regions\""), "{update}");
                break;
            }
        }
        assert!(get(&address, "/").contains("Hello again."));
        assert_eq!(running.interrupt(), Some(0), "{name}");
    }
}

#[cfg(unix)]
#[test]
fn no_browser_opens_nothing() {
    let dir = folder("no-browser");
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    let browser = recorder(&dir);
    let mut running = Running::start(&dir, &["paper.flm", "--no-browser", "--port", "0"], &browser);
    let address = running.address();
    running.logged("build 1 done");
    assert!(get(&address, "/").contains("Hello world."));
    assert!(opened(&dir).is_empty());
    assert_eq!(running.interrupt(), Some(0));
    assert!(opened(&dir).is_empty());
}

#[cfg(unix)]
#[test]
fn a_place_opens_in_the_editor_the_command_line_asks_for() {
    let dir = folder("editors");
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    let browser = recorder(&dir);
    // Two editors on the search path, Code first; the command line asks for Zed.
    let bin = dir.join("bin");
    let (code, zed) = (editor(&bin, "code"), editor(&bin, "zed"));
    let args = ["paper.flm", "--no-browser", "--port", "0", "--editor", "zed"];
    let mut running = Running::start_with_path(&dir, &args, &browser, Some(&bin));
    let address = running.address();
    running.logged("build 1 done");
    open_place(&address);
    eventually("Zed ran", || zed.exists());
    assert!(!code.exists(), "Code did not run");
    assert_eq!(running.interrupt(), Some(0));
}

#[cfg(unix)]
#[test]
fn the_editor_command_of_the_command_line_counts_over_the_viewer_settings() {
    let dir = folder("viewer-settings");
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    let browser = recorder(&dir);
    // Two editor commands of the author's own, their programs on the search path: one in the
    // viewer settings of the home folder, which the test sets to its own folder; the other
    // on the command line.
    let bin = dir.join("bin");
    let (given, set) = (editor(&bin, "myeditor"), editor(&bin, "fileeditor"));
    let settings = dir.join(".flm/watch.json");
    std::fs::create_dir_all(settings.parent().unwrap()).unwrap();
    std::fs::write(&settings, r#"{"editor_command": "fileeditor {file}"}"#).unwrap();
    let args = ["paper.flm", "--no-browser", "--port", "0", "--editor-command", "myeditor +{line} {file}"];
    let mut running = Running::start_with_path(&dir, &args, &browser, Some(&bin));
    let address = running.address();
    running.logged("build 1 done");
    open_place(&address);
    eventually("the command line's command ran", || given.exists());
    assert!(!set.exists(), "the viewer settings' command did not run");
    assert_eq!(running.interrupt(), Some(0));
}

// --- what the preview refuses ----------------------------------------------------------------------

#[cfg(unix)]
#[test]
fn what_the_preview_refuses_is_reported_and_nothing_is_served_or_opened() {
    let dir = folder("refused");
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    let browser = recorder(&dir);
    let cases: [&[&str]; 12] = [
        &["--editor", "nano", "paper.flm"],
        &["watch", "--editor", "emacs", "paper.flm"],
        &["watch", "--editor-command", "myeditor '{file}", "paper.flm"],
        &["-o", "paper.html", "paper.flm"],
        &["watch", "-o", "paper.html", "paper.flm"],
        &["-c", "Some text."],
        &["watch", "-c", "Some text."],
        &["-"],
        &["watch", "-"],
        &["-V", "-o", "out.html", "paper.flm"],
        &["watch", "-V", "paper.flm"],
        &["render", "-V", "paper.flm"],
    ];
    for args in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_flm"))
            .args(args)
            .current_dir(&dir)
            .env("BROWSER", &browser)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let reported = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {reported}");
        assert!(!reported.contains("http://"), "{args:?}: {reported}");
        assert!(output.stdout.is_empty(), "{args:?}");
    }
    assert!(opened(&dir).is_empty());
    assert!(!dir.join("paper.html").exists());
    assert!(!dir.join("out.html").exists());
}

// --- what `-v` prints ------------------------------------------------------------------------------

/// Under `--json-messages`, the preview writes JSON records on standard error: the
/// header first, then the link at which the preview is viewed as a record of the kind
/// `preview-link`, the lines that tell what it does as records of the kind `log`, and the
/// start of each build as a record of the kind `build` with its number. The records of the
/// kind `progress`, written while a build runs for longer than a moment, are set aside.
#[cfg(unix)]
#[test]
fn json_messages_writes_the_preview_s_lines_and_builds_as_records() {
    let dir = folder("json-messages");
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    let browser = recorder(&dir);
    let mut running = Running::start(&dir, &["paper.flm", "--no-browser", "--port", "0"], &browser);
    let address = running.address();
    running.logged("build 1 done");
    assert!(get(&address, "/").contains("Hello world."));
    let records = running.seen.clone();
    assert_eq!(records[0]["kind"], "messages", "{records:?}");
    assert_eq!(records[0]["format"], 1);
    assert_eq!(records[1]["kind"], "preview-link", "{records:?}");
    let build = records.iter().position(|record| record["kind"] == "build").expect("a build record");
    assert_eq!(records[build]["number"], 1);
    assert!(records[build]["time"].as_u64().unwrap() > 0);
    let done = records.iter().position(|record| record["text"] == "build 1 done").unwrap();
    assert!(build < done);
    assert!(records[1..].iter().all(|record| matches!(record["kind"].as_str(), Some("preview-link" | "log" | "build" | "progress"))), "{records:?}");
    assert_eq!(running.interrupt(), Some(0));
}

#[cfg(unix)]
#[test]
fn verbose_prints_the_packages_and_the_workflow_as_render_does() {
    let dir = folder("verbose");
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    let browser = recorder(&dir);
    let mut running = Running::start(&dir, &["paper.flm", "-v", "--no-browser", "--port", "0"], &browser);
    running.logged("build 1 done");
    let seen = running.seen.clone();
    let before = |text: &str| seen.iter().any(|record| record["text"].as_str().is_some_and(|logged| logged.starts_with(text)));
    assert!(before("packages "), "{seen:?}");
    assert!(before("workflow "), "{seen:?}");
    assert_eq!(running.interrupt(), Some(0));
}

// --- the tool Settings: the schema, a change, the build that follows -----------------------------

/// Sends `POST target` with the JSON body `body` to `address`, as the preview's page does
/// (its `Origin` the server's own), and gives the answer's status.
fn post(address: &str, target: &str, body: &str) -> u16 {
    let mut connection = TcpStream::connect(host(address)).unwrap();
    connection.set_read_timeout(Some(BOUND)).unwrap();
    let origin = address.trim_end_matches('/');
    write!(
        connection,
        "POST {target} HTTP/1.1\r\nHost: {}\r\nOrigin: {origin}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        host(address),
        body.len()
    )
    .unwrap();
    let mut bytes = Vec::new();
    connection.read_to_end(&mut bytes).unwrap();
    String::from_utf8_lossy(&bytes).split(' ').nth(1).unwrap().parse().unwrap()
}

/// Sends a change of the front matter to `address`: the value `value` at `path`, against
/// the revision of the front matter that the settings state gives now; gives the answer's
/// status.
fn change(address: &str, path: &[&str], value: serde_json::Value) -> u16 {
    let settings: serde_json::Value = serde_json::from_str(&get(address, "/.flm-preview/settings")).unwrap();
    let body = serde_json::json!({ "revision": settings["revision"], "changes": [{ "path": path, "value": value }] });
    post(address, "/.flm-preview/front-matter", &body.to_string())
}

/// The next update whose data satisfies `wanted`, passing over the others: the system may
/// report a file's writing late, which causes a build of the same text first.
fn update_where(updates: &Receiver<String>, wanted: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
    loop {
        let update: serde_json::Value = serde_json::from_str(&updates.recv_timeout(BOUND).expect("an update within the bound")).unwrap();
        if wanted(&update) {
            return update;
        }
    }
}

#[cfg(unix)]
#[test]
fn the_settings_tool_reads_the_schema_and_a_change_is_built_as_any_save_of_the_file() {
    let dir = folder("settings");
    let paper = "---\ntitle: A paper\n---\n\\begin{theorem}\nA claim.\n\\end{theorem}\n";
    std::fs::write(dir.join("paper.flm"), paper).unwrap();
    let browser = recorder(&dir);
    let mut running = Running::start(&dir, &["paper.flm", "--no-browser", "--port", "0"], &browser);
    let address = running.address();
    running.logged("build 1 done");
    assert!(get(&address, "/").contains("flm-theorems-theorem"));

    // The schema `flm schema --of front-matter paper.flm` prints for the document, whose
    // preparation the tool's schema comes from once a build is done, and what the front
    // matter writes.
    let printed = Command::new(env!("CARGO_BIN_EXE_flm"))
        .args(["schema", "--of", "front-matter", "paper.flm"])
        .current_dir(&dir)
        .env("HOME", &dir)
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE")
        .output()
        .unwrap();
    assert!(printed.status.success(), "{printed:?}");
    let printed: serde_json::Value = serde_json::from_slice(&printed.stdout).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&get(&address, "/.flm-preview/settings")).unwrap();
    assert_eq!(settings["schema"], printed);
    assert_eq!(settings["front_matter"], serde_json::json!({"title": "A paper"}));

    // A change writes the file; the build that follows is the watcher's, caused by that
    // file, and the page it gives shows the change: the theorem is not in it.
    let updates = updates(&address);
    assert_eq!(change(&address, &["flm", "features", "theorems"], serde_json::json!(false)), 200);
    assert_eq!(
        std::fs::read_to_string(dir.join("paper.flm")).unwrap(),
        "---\ntitle: A paper\nflm:\n  features:\n    theorems: false\n---\n\\begin{theorem}\nA claim.\n\\end{theorem}\n"
    );
    running.logged("changed paper.flm");
    // The page is loaded anew or brought up to date in place, as the change asks: the page
    // served after the update is read.
    update_where(&updates, |_| {
        let page = get(&address, "/");
        page.contains("A claim") && !page.contains("flm-theorems-theorem")
    });

    // A value a feature refuses is a problem of the build that follows, at its line in the
    // front matter.
    assert_eq!(change(&address, &["flm", "features", "math"], serde_json::json!({"numbered_equations": "yes"})), 200);
    let refused = |problem: &serde_json::Value| problem["severity"] == "error" && problem["place"]["file"] == "paper.flm" && problem["place"]["line"] == 7;
    update_where(&updates, |update| update["problems"].as_array().is_some_and(|problems| problems.iter().any(refused)));

    // The preview wrote no other file.
    let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, ["browser.sh", "paper.flm"]);
    assert_eq!(running.interrupt(), Some(0));
}

// --- the help pages -------------------------------------------------------------------------------

/// A data extension beside the document, which defines the macro `\project` with a
/// documentation text.
const NOTES: &str = "flm:\n  features:\n    userdefs:\n      macros:\n        project:\n          \
                     content: 'the project'\n          docs: \"The project of the course.\\n\\nIt is written as text.\"\n";

/// The help pages of the document's environment, as `flm info -d` gives them: the page of a
/// command that the document's own package defines is in the index under its names, and
/// renders as HTML with links of the kind `flmdoc.page`; a path without a page answers the
/// nearest pages; a construct written in the document gives its page.
#[cfg(all(unix, feature = "bundled-extensions"))]
#[test]
fn the_help_pages_hold_the_document_s_own_command_and_a_path_without_a_page_answers_the_nearest() {
    let dir = folder("help");
    std::fs::create_dir_all(dir.join("_flmextensions")).unwrap();
    std::fs::write(dir.join("_flmextensions/notes.yaml"), NOTES).unwrap();
    let content = "The \\project is late.\n";
    std::fs::write(dir.join("paper.flm"), format!("---\nflm:\n  use: [notes]\n---\n{content}")).unwrap();
    let browser = recorder(&dir);
    let mut running = Running::start(&dir, &["paper.flm", "--no-browser", "--port", "0", "--math", "passthrough"], &browser);
    let address = running.address();
    running.logged("build 1 done");

    let index: serde_json::Value = serde_json::from_str(&get(&address, "/.flm-preview/help/index")).unwrap();
    let pages = index["pages"].as_array().unwrap();
    let path = "features/userdefs/macros/project";
    let project = pages.iter().find(|page| page["path"] == path).unwrap_or_else(|| panic!("{index}"));
    assert_eq!(project["kind"], "command", "{project}");
    assert_eq!(project["title"], "Macro \\project", "{project}");
    assert_eq!(project["summary"], "The project of the course.", "{project}");
    assert_eq!(project["names"], serde_json::json!(["\\project", "project"]), "{project}");
    assert!(pages.iter().any(|page| page["path"] == "program/"), "the program's pages");

    let page: serde_json::Value =
        serde_json::from_str(&get(&address, "/.flm-preview/help/page?path=features%2Fuserdefs%2Fmacros%2Fproject")).unwrap();
    assert_eq!(page["page"]["title"], "Macro \\project", "{page}");
    let html = page["page"]["content"].as_str().unwrap();
    assert!(html.contains("The project of the course."), "{html}");
    assert!(html.contains("data-flm-kind=\"flmdoc.page\""), "{html}");
    assert!(html.contains("href=\"features/userdefs.html\""), "{html}");
    assert!(!html.contains("<html"), "the content alone: {html}");

    let missing: serde_json::Value =
        serde_json::from_str(&get(&address, "/.flm-preview/help/page?path=features/userdefs/macros/projet")).unwrap();
    assert_eq!(missing["page"], serde_json::Value::Null, "{missing}");
    let nearest = missing["nearest"].as_array().unwrap();
    assert!(nearest.iter().any(|near| near["path"] == path && near["title"] == "Macro \\project"), "{missing}");

    let start = content.find("\\project").unwrap();
    let at: serde_json::Value = serde_json::from_str(&get(&address, &format!("/.flm-preview/at?file=paper.flm&start={start}"))).unwrap();
    assert_eq!(at["constructs"][0]["help"], path, "{at}");
    assert_eq!(running.interrupt(), Some(0));
}

// --- an extension written in JavaScript: a save of a script, the tool Settings ---------------------

/// Copies every file below `from` into `to`, making the folders.
#[cfg(all(unix, feature = "js"))]
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

/// A fresh folder holding the documents of `tests/documents/extensions/` and the extension
/// `wordcount` of `flm-ext`'s tests under `_flmextensions/`, as `tests/extensions_js.rs`
/// sets them up.
#[cfg(all(unix, feature = "js"))]
fn with_wordcount(name: &str) -> PathBuf {
    let dir = folder(name);
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    copy_tree(&manifest.join("tests/documents/extensions"), &dir);
    copy_tree(&manifest.join("../flm-ext/tests/js/wordcount"), &dir.join("_flmextensions/wordcount"));
    dir
}

#[cfg(all(unix, feature = "js"))]
#[test]
fn the_preview_rebuilds_when_a_script_of_an_extension_is_saved() {
    let dir = with_wordcount("script");
    let browser = recorder(&dir);
    let mut running = Running::start(&dir, &["wordcount.flm", "--no-browser", "--port", "0"], &browser);
    let address = running.address();
    running.logged("build 1 done");
    assert!(get(&address, "/").contains("LISTEN!"));
    let main = dir.join("_flmextensions/wordcount/main.js");
    let text = std::fs::read_to_string(&main).unwrap();
    std::fs::write(&main, text.replace("text.toUpperCase() + suffix", "text.toUpperCase() + suffix + suffix")).unwrap();
    running.logged("changed _flmextensions/wordcount/main.js");
    running.logged("build 2 done");
    eventually("the page shows the new suffix", || get(&address, "/").contains("LISTEN!!"));
    assert_eq!(running.interrupt(), Some(0));
}

#[cfg(all(unix, feature = "js"))]
#[test]
fn the_settings_tool_s_schema_holds_the_settings_of_a_scripted_feature() {
    let dir = with_wordcount("settings-js");
    let browser = recorder(&dir);
    let mut running = Running::start(&dir, &["wordcount.flm", "-p", "counted", "--no-browser", "--port", "0"], &browser);
    let address = running.address();
    running.logged("build 1 done");
    let printed = Command::new(env!("CARGO_BIN_EXE_flm"))
        .args(["schema", "--of", "front-matter", "wordcount.flm"])
        .current_dir(&dir)
        .env("HOME", &dir)
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE")
        .output()
        .unwrap();
    assert!(printed.status.success(), "{printed:?}");
    let printed: serde_json::Value = serde_json::from_slice(&printed.stdout).unwrap();
    let settings: serde_json::Value = serde_json::from_str(&get(&address, "/.flm-preview/settings")).unwrap();
    assert_eq!(settings["schema"], printed);
    let keys: Vec<&String> = settings["schema"]["$defs"]["wordcount"]["properties"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["enabled", "shout_suffix", "min_length", "report_counts"]);
    assert_eq!(settings["front_matter"]["flm"]["features"]["wordcount"], serde_json::json!({"shout_suffix": "!", "min_length": 4, "report_counts": true}));

    // A change writes the front matter; the build that follows counts with the setting
    // changed.
    let updates = updates(&address);
    assert_eq!(change(&address, &["flm", "features", "wordcount", "min_length"], serde_json::json!(2)), 200);
    assert!(std::fs::read_to_string(dir.join("wordcount.flm")).unwrap().contains("min_length: 2"));
    running.logged("changed wordcount.flm");
    let counted = |update: &serde_json::Value| {
        update["problems"].as_array().is_some_and(|problems| {
            problems.iter().any(|problem| problem["severity"] == "note" && problem["message"].to_string().contains("of at least 2 characters"))
        })
    };
    update_where(&updates, counted);
    assert_eq!(running.interrupt(), Some(0));
}

// --- the export: the files saved and the command line ---------------------------------------------

/// Sends `POST target` with the JSON body `body` to `address`, as the preview's page does,
/// and gives the answer's status and body.
fn post_answer(address: &str, target: &str, body: &str) -> (u16, String) {
    let mut connection = TcpStream::connect(host(address)).unwrap();
    connection.set_read_timeout(Some(BOUND)).unwrap();
    let origin = address.trim_end_matches('/');
    write!(
        connection,
        "POST {target} HTTP/1.1\r\nHost: {}\r\nOrigin: {origin}\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
        host(address),
        body.len()
    )
    .unwrap();
    let mut bytes = Vec::new();
    connection.read_to_end(&mut bytes).unwrap();
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let split = text.find("\r\n\r\n").expect("a head");
    (text.split(' ').nth(1).unwrap().parse().unwrap(), text[split + 4..].to_string())
}

/// The names of the files in `dir`, sorted, hidden files included.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        std::fs::read_dir(dir).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

/// An export saved from the preview of a document in a folder writes beside the document what
/// the command line it shows writes, run from the folder the preview was started in: the
/// options of `flm watch` that change the build are in it, the document's folder is in its
/// `-o`.
#[cfg(unix)]
#[test]
fn an_export_saved_from_the_preview_writes_what_its_command_line_writes() {
    let dir = folder("export");
    std::fs::create_dir_all(dir.join("doc")).unwrap();
    let paper = "---\ntitle: A paper\n---\nHello \\emph{world}, at length.\n";
    std::fs::write(dir.join("doc/paper.flm"), paper).unwrap();
    let browser = recorder(&dir);
    let args = ["watch", "doc/paper.flm", "--no-browser", "--port", "0", "--math", "passthrough", "-O", "text.width=20"];
    let mut running = Running::start(&dir, &args, &browser);
    let address = running.address();
    running.logged("build 1 done");

    let described: serde_json::Value = serde_json::from_str(&get(&address, "/.flm-preview/export?profile=text")).unwrap();
    assert_eq!(described["name"], "paper.txt", "{described}");
    let command = "flm render -p text --math passthrough -O text.width=20 -o doc/paper.txt doc/paper.flm --view";
    assert_eq!(described["command"], command, "{described}");
    assert_eq!(
        described["command_continuing"],
        command.replace("-p text", "-p text --continue-with-errors"),
        "{described}"
    );

    let (status, body) =
        post_answer(&address, "/.flm-preview/export", r#"{"profile": "text", "action": "save", "continue_with_errors": false}"#);
    assert_eq!(status, 200, "{body}");
    assert_eq!(serde_json::from_str::<serde_json::Value>(&body).unwrap(), serde_json::json!({"name": "paper.txt", "files": ["paper.txt"]}));
    assert_eq!(running.interrupt(), Some(0));

    // The command line, without `--view`, run from another folder holding the same document.
    let other = folder("export-render");
    std::fs::create_dir_all(other.join("doc")).unwrap();
    std::fs::write(other.join("doc/paper.flm"), paper).unwrap();
    let words: Vec<&str> = command.split(' ').skip(1).filter(|word| *word != "--view").collect();
    let rendered = Command::new(env!("CARGO_BIN_EXE_flm")).args(&words).current_dir(&other).env("HOME", &other).env_remove("FLMPATH").output().unwrap();
    assert!(rendered.status.success(), "{rendered:?}");
    assert_eq!(names(&dir.join("doc")), names(&other.join("doc")));
    let written = std::fs::read_to_string(dir.join("doc/paper.txt")).unwrap();
    assert_eq!(written, std::fs::read_to_string(other.join("doc/paper.txt")).unwrap());
    // `text.width=20` breaks the line.
    assert!(written.lines().count() > 2, "{written}");
}
