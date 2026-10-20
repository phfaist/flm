//! `flm watch` and the bare form `flm FILE`, run as the binary over files under the build
//! folder: the address printed, the page served, an update after a change, Ctrl+C; the
//! browser opened through a program the test supplies (the environment variable
//! `BROWSER`); what the preview refuses. (What `-V` under `render` opens is tested in
//! process, in the crate itself, since it opens a window otherwise.) Every wait
//! has an upper bound and ends as soon as what is expected arrives; the system chooses the
//! port (`--port 0`), and no browser window opens.

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
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-watch-{name}"));
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

/// Waits until `done` holds, within the bound.
fn eventually(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < BOUND, "{what}: not within the bound");
        thread::sleep(Duration::from_millis(20));
    }
}

/// The program running, its standard error read line by line.
struct Running {
    child: Child,
    lines: Receiver<String>,
    seen: Vec<String>,
}

impl Running {
    fn start(dir: &Path, args: &[&str], browser: &Path) -> Running {
        Running::start_with_path(dir, args, browser, None)
    }

    /// [`Running::start`], the variable `PATH` set to `path` when it is given.
    fn start_with_path(dir: &Path, args: &[&str], browser: &Path, path: Option<&Path>) -> Running {
        let mut command = Command::new(env!("CARGO_BIN_EXE_flm"));
        command.args(args).current_dir(dir).env("BROWSER", browser).env("HOME", dir).env_remove("FLMPATH");
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

    /// Reads standard error until a line holds `text`, and gives that line.
    fn line_with(&mut self, text: &str) -> String {
        loop {
            let line = self.lines.recv_timeout(BOUND).unwrap_or_else(|_| panic!("`{text}` within the bound: {:?}", self.seen));
            self.seen.push(line.clone());
            if line.contains(text) {
                return line;
            }
        }
    }

    /// The address of the preview, as the program printed it.
    fn address(&mut self) -> String {
        let line = self.line_with("the preview is at ");
        let rest = &line[line.find("http://").expect("an address")..];
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

// --- acceptance 7 and 9: the program from end to end, and the browser ----------------------------

#[cfg(unix)]
#[test]
fn the_preview_serves_the_page_updates_it_and_ends_on_the_interrupt_both_spellings() {
    for (name, args) in [("bare", &["paper.flm", "--port", "0"][..]), ("watch", &["watch", "paper.flm", "--port", "0"][..])] {
        let dir = folder(name);
        std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
        let browser = recorder(&dir);
        let mut running = Running::start(&dir, args, &browser);
        let address = running.address();
        running.line_with("flm: build 1 done");
        let page = get(&address, "/");
        assert!(page.contains("Hello world."), "{page}");
        assert!(page.contains("flm.preview.script"), "{page}");
        // The browser was opened on the address printed.
        eventually("the browser opened", || opened(&dir) == [address.clone()]);

        let updates = updates(&address);
        std::fs::write(dir.join("paper.flm"), "---\ntitle: A paper\n---\nHello again.\n").unwrap();
        running.line_with("flm: changed paper.flm");
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
    // Which editor was found depends on this machine; the terminal says it either way.
    let editor = running.line_with("editor");
    assert!(editor.starts_with("flm: editors found: ") || editor.starts_with("flm: no editor was found"), "{editor}");
    running.line_with("flm: build 1 done");
    assert!(get(&address, "/").contains("Hello world."));
    assert!(opened(&dir).is_empty());
    assert_eq!(running.interrupt(), Some(0));
    assert!(opened(&dir).is_empty());
}

#[cfg(unix)]
#[test]
fn the_terminal_lists_the_editors_found_and_the_default() {
    use std::os::unix::fs::PermissionsExt;
    let dir = folder("editors");
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    let browser = recorder(&dir);
    // Two editors on the search path: scripts that open nothing.
    let bin = dir.join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    for name in ["code", "zed"] {
        std::fs::write(bin.join(name), "#!/bin/sh\n").unwrap();
        std::fs::set_permissions(bin.join(name), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let mut running =
        Running::start_with_path(&dir, &["paper.flm", "--no-browser", "--port", "0", "--editor", "zed"], &browser, Some(&bin));
    running.address();
    let editors = running.line_with("editor");
    // Code and Zed are the first two editors; on macOS an editor installed as an
    // application on this machine may follow them.
    let found = format!("flm: editors found: `code` ({}), `zed` ({})", bin.join("code").display(), bin.join("zed").display());
    assert!(editors.starts_with(&found), "{editors}");
    assert!(editors.ends_with("; a place clicked on the page opens in the one chosen in the page's menu, `zed` by default"), "{editors}");
    assert_eq!(running.interrupt(), Some(0));
}

// --- acceptance 10: what the preview refuses -------------------------------------------------------

#[cfg(unix)]
#[test]
fn what_the_preview_refuses_is_reported_and_nothing_is_served_or_opened() {
    let dir = folder("refused");
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    let browser = recorder(&dir);
    let cases: [(&[&str], &str); 11] = [
        (&["--editor", "nano", "paper.flm"], "the editor `nano` must be one of the editors `code`, `zed`, `emacsclient`, `subl`, `gvim`"),
        (&["watch", "--editor", "vim", "paper.flm"], "the editor `vim` must be one of the editors `code`, `zed`, `emacsclient`, `subl`, `gvim`"),
        (&["-o", "paper.html", "paper.flm"], "flm render -o"),
        (&["watch", "-o", "paper.html", "paper.flm"], "flm render -o"),
        (&["-c", "Some text."], "flm render -c"),
        (&["watch", "-c", "Some text."], "flm render -c"),
        (&["-"], "flm render -"),
        (&["watch", "-"], "flm render -"),
        (&["-V", "-o", "out.html", "paper.flm"], "flm render -V -o"),
        (&["watch", "-V", "paper.flm"], "flm render -V -o"),
        (&["render", "-V", "paper.flm"], "`-o`"),
    ];
    for (args, named) in cases {
        let output = Command::new(env!("CARGO_BIN_EXE_flm"))
            .args(args)
            .current_dir(&dir)
            .env("BROWSER", &browser)
            .stdin(Stdio::null())
            .output()
            .unwrap();
        let reported = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(2), "{args:?}: {reported}");
        assert!(reported.contains(named), "{args:?}: {reported}");
        assert!(!reported.contains("the preview is at"), "{args:?}: {reported}");
        assert!(output.stdout.is_empty(), "{args:?}");
    }
    assert!(opened(&dir).is_empty());
    assert!(!dir.join("paper.html").exists());
    assert!(!dir.join("out.html").exists());
}

// --- what `-v` prints ------------------------------------------------------------------------------

/// Under `--json-diagnostics`, the preview writes JSON records on standard error: the
/// header first, then the lines that tell what it does as records of the kind `log`, and the
/// start of each build as a record of the kind `build` with its number.
#[cfg(unix)]
#[test]
fn json_diagnostics_writes_the_preview_s_lines_and_builds_as_records() {
    let dir = folder("json-diagnostics");
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    let browser = recorder(&dir);
    let mut running = Running::start(&dir, &["--json-diagnostics", "paper.flm", "--no-browser", "--port", "0"], &browser);
    let address = running.address();
    running.line_with("build 1 done");
    assert!(get(&address, "/").contains("Hello world."));
    let records: Vec<serde_json::Value> =
        running.seen.iter().map(|line| serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line}"))).collect();
    assert_eq!(records[0]["kind"], "diagnostics", "{records:?}");
    assert_eq!(records[0]["format"], 1);
    let build = records.iter().position(|record| record["kind"] == "build").expect("a build record");
    assert_eq!(records[build]["number"], 1);
    assert!(records[build]["time"].as_u64().unwrap() > 0);
    let done = records.iter().position(|record| record["text"] == "build 1 done").unwrap();
    assert!(build < done);
    assert!(records[1..].iter().all(|record| matches!(record["kind"].as_str(), Some("log" | "build"))), "{records:?}");
    assert_eq!(running.interrupt(), Some(0));
}

#[cfg(unix)]
#[test]
fn verbose_prints_the_packages_and_the_workflow_as_render_does() {
    let dir = folder("verbose");
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    let browser = recorder(&dir);
    let mut running = Running::start(&dir, &["paper.flm", "-v", "--no-browser", "--port", "0"], &browser);
    running.line_with("flm: build 1 done");
    let seen = running.seen.clone();
    let before = |text: &str| seen.iter().any(|line| line.starts_with(text));
    assert!(before("flm: packages "), "{seen:?}");
    assert!(before("flm: workflow "), "{seen:?}");
    assert_eq!(running.interrupt(), Some(0));
}

// --- the tool Settings: the schema, a save, the build that follows -------------------------------

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
fn the_settings_tool_reads_the_schema_and_a_save_is_built_as_any_save_of_the_file() {
    let dir = folder("settings");
    let paper = "---\ntitle: A paper\n---\n\\begin{theorem}\nA claim.\n\\end{theorem}\n";
    std::fs::write(dir.join("paper.flm"), paper).unwrap();
    let browser = recorder(&dir);
    let mut running = Running::start(&dir, &["paper.flm", "--no-browser", "--port", "0"], &browser);
    let address = running.address();
    running.line_with("flm: build 1 done");
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
    assert_eq!(settings["features"], serde_json::Value::Null);

    // A save writes the file; the build that follows is the watcher's, caused by that file,
    // and the page it gives shows the change: the theorem is no longer in it.
    let updates = updates(&address);
    assert_eq!(post(&address, "/.flm-preview/settings", r#"{"theorems": false}"#), 204);
    assert_eq!(
        std::fs::read_to_string(dir.join("paper.flm")).unwrap(),
        "---\ntitle: A paper\nflm:\n  features:\n    theorems: false\n---\n\\begin{theorem}\nA claim.\n\\end{theorem}\n"
    );
    running.line_with("flm: changed paper.flm");
    // The page is loaded anew or brought up to date in place, as the change asks: the page
    // served after the update is read.
    update_where(&updates, |_| {
        let page = get(&address, "/");
        page.contains("A claim") && !page.contains("flm-theorems-theorem")
    });

    // A value a feature refuses is a problem of the build that follows, at its line in the
    // front matter.
    assert_eq!(post(&address, "/.flm-preview/settings", r#"{"math": {"numbered_equations": "yes"}}"#), 204);
    let update = update_where(&updates, |update| {
        update["problems"].as_array().is_some_and(|problems| problems.iter().any(|problem| problem["text"].to_string().contains("numbered_equations")))
    });
    let problem = update["problems"].as_array().unwrap().iter().find(|problem| problem["text"].to_string().contains("numbered_equations")).unwrap().clone();
    assert_eq!((problem["severity"].as_str(), problem["file"].as_str(), problem["line"].as_u64()), (Some("error"), Some("paper.flm"), Some(6)), "{problem}");

    // The preview wrote no other file.
    let mut names: Vec<String> = std::fs::read_dir(&dir).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    assert_eq!(names, ["browser.sh", "paper.flm"]);
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
    running.line_with("flm: build 1 done");
    assert!(get(&address, "/").contains("LISTEN!"));
    let main = dir.join("_flmextensions/wordcount/main.js");
    let text = std::fs::read_to_string(&main).unwrap();
    std::fs::write(&main, text.replace("text.toUpperCase() + suffix", "text.toUpperCase() + suffix + suffix")).unwrap();
    running.line_with("flm: changed _flmextensions/wordcount/main.js");
    running.line_with("flm: build 2 done");
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
    running.line_with("flm: build 1 done");
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
    assert_eq!(keys, ["shout_suffix", "min_length", "report_counts"]);
    assert_eq!(settings["features"]["wordcount"], serde_json::json!({"shout_suffix": "!", "min_length": 4, "report_counts": true}));

    // A save writes the front matter; the build that follows counts with the new setting.
    let updates = updates(&address);
    let body = r#"{"wordcount": {"shout_suffix": "!", "min_length": 2, "report_counts": true}}"#;
    assert_eq!(post(&address, "/.flm-preview/settings", body), 204);
    assert!(std::fs::read_to_string(dir.join("wordcount.flm")).unwrap().contains("min_length: 2"));
    running.line_with("flm: changed wordcount.flm");
    let counted = |update: &serde_json::Value| {
        update["problems"].as_array().is_some_and(|problems| {
            problems.iter().any(|problem| problem["severity"] == "note" && problem["text"].to_string().contains("of at least 2 characters"))
        })
    };
    update_where(&updates, counted);
    assert_eq!(running.interrupt(), Some(0));
}
