//! A browser for the tests of the preview's page: Google Chrome without a window, driven
//! through its debugging protocol (JSON messages over a WebSocket).
//!
//! The tests that use it run only when the variable `FLM_TESTS_BROWSER` is `1`, and only
//! where Chrome is found: at the path the variable `FLM_TESTS_CHROME` names, else at its
//! place on macOS, else as `google-chrome` or `chromium` on the search path. Otherwise
//! [`Page::open`] gives `None` and the test returns without checking anything.
//!
//! Every wait has an upper bound and ends as soon as what is expected holds.

#![allow(dead_code)]

use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use tungstenite::{Message, WebSocket};

/// The upper bound of every wait.
pub const BOUND: Duration = Duration::from_secs(30);

/// Whether the browser tests are to run.
fn wanted() -> bool {
    std::env::var("FLM_TESTS_BROWSER").as_deref() == Ok("1")
}

/// The program of Chrome, if found.
fn chrome() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("FLM_TESTS_CHROME") {
        return Some(PathBuf::from(path));
    }
    let mac = Path::new("/Applications/Google Chrome.app/Contents/MacOS/Google Chrome");
    if mac.exists() {
        return Some(mac.to_path_buf());
    }
    let search = std::env::var_os("PATH")?;
    std::env::split_paths(&search)
        .flat_map(|folder| ["google-chrome", "chromium"].map(|name| folder.join(name)))
        .find(|path| path.exists())
}

/// One page of a browser without a window, which the browser ends with.
pub struct Page {
    child: Child,
    folder: PathBuf,
    socket: WebSocket<TcpStream>,
    session: String,
    next: u64,
}

impl Page {
    /// A browser without a window, showing `url`; `None` when the browser tests are not to
    /// run or no browser is found (said on the error stream).
    pub fn open(url: &str) -> Option<Page> {
        if !wanted() {
            eprintln!("skipped: set FLM_TESTS_BROWSER=1 to run the tests in a browser");
            return None;
        }
        let Some(program) = chrome() else {
            eprintln!("skipped: no Chrome found (FLM_TESTS_CHROME names one)");
            return None;
        };
        let folder = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("browser-{}-{}", std::process::id(), unique()));
        let _ = std::fs::remove_dir_all(&folder);
        std::fs::create_dir_all(&folder).expect("the browser's folder is made");
        let child = Command::new(program)
            .args([
                "--headless",
                "--remote-debugging-port=0",
                "--no-first-run",
                "--no-default-browser-check",
                "--disable-extensions",
                "--disable-background-networking",
                "--disable-component-update",
                "--disable-sync",
                "--window-size=1000,700",
            ])
            .arg(format!("--user-data-dir={}", folder.display()))
            .arg("about:blank")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("Chrome starts");
        // Chrome writes the port it listens at, and the path of its own endpoint, into
        // this file of its folder once it listens.
        let active = folder.join("DevToolsActivePort");
        let endpoint = wait_until("Chrome listens", || {
            let text = std::fs::read_to_string(&active).ok()?;
            let mut lines = text.lines();
            let port = lines.next()?.trim().to_owned();
            let path = lines.next()?.trim().to_owned();
            Some(format!("ws://127.0.0.1:{port}{path}"))
        });
        let address = endpoint.trim_start_matches("ws://").split('/').next().unwrap().to_owned();
        let stream = TcpStream::connect(&address).expect("Chrome is reached");
        stream.set_read_timeout(Some(BOUND)).unwrap();
        let (socket, _) = tungstenite::client::client(endpoint.as_str(), stream).expect("the WebSocket opens");
        let mut page = Page { child, folder, socket, session: String::new(), next: 0 };
        let target = page.call(None, "Target.createTarget", json!({ "url": url })).expect("a target is made");
        let target = target["targetId"].as_str().expect("a target").to_owned();
        let attached = page
            .call(None, "Target.attachToTarget", json!({ "targetId": target, "flatten": true }))
            .expect("the target is attached");
        page.session = attached["sessionId"].as_str().expect("a session").to_owned();
        Some(page)
    }

    /// Sends the command `method` with `params`, to the page when `session` names it, and
    /// gives its result, or the error the browser answered.
    fn call(&mut self, session: Option<&str>, method: &str, params: Value) -> Result<Value, String> {
        self.next += 1;
        let id = self.next;
        let mut message = json!({ "id": id, "method": method, "params": params });
        if let Some(session) = session {
            message["sessionId"] = Value::from(session);
        }
        self.socket.send(Message::text(message.to_string())).expect("a command is sent");
        loop {
            let message = self.socket.read().expect("an answer within the bound");
            let Message::Text(text) = message else { continue };
            let answer: Value = serde_json::from_str(text.as_str()).expect("an answer in JSON");
            if answer["id"].as_u64() != Some(id) {
                continue;
            }
            if let Some(error) = answer.get("error") {
                return Err(format!("{method} failed: {error}"));
            }
            return Ok(answer["result"].clone());
        }
    }

    /// The value of the JavaScript expression `expression` in the page (a promise awaited),
    /// as JSON.
    pub fn eval(&mut self, expression: &str) -> Value {
        self.try_eval(expression).unwrap_or_else(|error| panic!("`{expression}`: {error}"))
    }

    /// The value of `expression`, or why there is none: it threw, or the page was being
    /// loaded anew.
    fn try_eval(&mut self, expression: &str) -> Result<Value, String> {
        let session = self.session.clone();
        let result = self.call(
            Some(&session),
            "Runtime.evaluate",
            json!({ "expression": expression, "awaitPromise": true, "returnByValue": true }),
        )?;
        if let Some(exception) = result.get("exceptionDetails") {
            return Err(format!("it threw: {exception}"));
        }
        Ok(result["result"]["value"].clone())
    }

    /// Waits until the JavaScript expression `condition` is true in the page; while the page
    /// is loaded anew, or when the expression throws, it is not.
    pub fn wait(&mut self, condition: &str) {
        let start = Instant::now();
        while self.try_eval(&format!("!!({condition})")) != Ok(Value::Bool(true)) {
            assert!(start.elapsed() < BOUND, "`{condition}` within the bound");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.folder);
    }
}

/// Waits until `probe` gives a value, and gives it.
pub fn wait_until<T>(what: &str, mut probe: impl FnMut() -> Option<T>) -> T {
    let start = Instant::now();
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(start.elapsed() < BOUND, "{what} within the bound");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// A number no other page of this process has.
fn unique() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    NEXT.fetch_add(1, Ordering::SeqCst)
}
