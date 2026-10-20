//! The preview's page script (`watch/page/`), in Chrome without a window: the update of a
//! region in place, the contract for a page updated in place with the formula renderer's
//! part in it, the toolbar and its tools, the gestures and *Open in editor* with the menu of
//! editors, the waiting
//! page, the connection; and the pages of the `math-mathjax` extension in its two modes
//! (feature `js`).
//!
//! These tests run only when `FLM_TESTS_BROWSER` is `1` and Chrome is found (see
//! `support/browser.rs`); the full battery runs them. Some drive the real loop and the real
//! server over files held in memory; the others serve a page of their own making and send
//! the messages of the server's stream themselves (the *stage*), so that they can send what
//! no build gives — a formula of their choosing, a file changed under an unchanged path, a
//! message that is not understood. Every wait has an upper bound and ends as soon as what
//! is expected holds; the system chooses every port.

#[path = "support/browser.rs"]
mod browser;
#[cfg(unix)]
#[path = "support/editor.rs"]
mod editor;

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

use browser::{BOUND, Page, wait_until};
use flm_core::core::registry::DefContent;
use flm_core::stdcore::backends::html::mathjax_script;
use flm_main::files::{FileError, Files, MemoryFiles};
use flm_main::output::Naming;
use flm_main::session::{Input, Request};
use flm_main::stdpkg::preview::{SCRIPT_TEXT, STYLE_TEXT};
use flm_main::watch::editor::Editors;
use flm_main::watch::server::{Opening, Port, Server};
use flm_main::watch::settings::DocumentSettings;
use flm_main::watch::{Event, Handle, Watch};
use serde_json::{Value, json};

// --- the real loop and server ---------------------------------------------------------------

/// Files held in memory that a test changes while the loop runs.
#[derive(Clone, Default)]
struct Shared(Arc<Mutex<MemoryFiles>>);

impl Shared {
    fn with(path: &str, text: &str) -> Shared {
        let shared = Shared::default();
        shared.write(path, text);
        shared
    }

    fn write(&self, path: &str, text: &str) {
        self.0.lock().unwrap().insert(path, text.as_bytes());
    }

    /// The text of `path`.
    fn text(&self, path: &str) -> String {
        String::from_utf8(self.read(Path::new(path)).unwrap()).unwrap()
    }

    /// The settings of `doc/paper.flm` over these files, a save written into them, the
    /// forms made from the schema of a front matter for the packages the preview uses
    /// ([`front_matter_schema`]).
    fn settings(&self) -> DocumentSettings {
        let written = self.clone();
        DocumentSettings::new(front_matter_schema(), "doc/paper.flm", Arc::new(self.clone()), move |path, bytes| {
            written.0.lock().unwrap().insert(path, bytes);
            Ok(())
        })
    }
}

impl Files for Shared {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FileError> {
        self.0.lock().unwrap().read(path)
    }

    fn entries(&self, path: &Path) -> Result<Vec<String>, FileError> {
        self.0.lock().unwrap().entries(path)
    }
}

/// The schema of a front matter for the features of the packages the preview uses
/// ([`Preview::start_with_folders`]), in their order: `stdcore` and `stdmain`, the
/// packages of `std`, then `math-passthrough`; as `flm schema --of front-matter`
/// prints it.
fn front_matter_schema() -> Value {
    let stdmain = flm_main::stdpkg::stdmain_factory(flm_main::stdpkg::StdParts::new(Arc::new(Naming::Folder))).unwrap();
    let stdcore = flm_core::stdcore::factory().unwrap();
    let math_passthrough = flm_core::math_passthrough::factory().unwrap();
    flm_main::seed::front_matter_schema(&[&stdcore, &stdmain, &math_passthrough], &[])
}

/// The loop and the server, the loop on a thread of its own handing each event to the
/// server; the test is told the number of each build done.
struct Preview {
    server: Arc<Server>,
    handle: Handle,
    built: Receiver<u64>,
    ended: Receiver<()>,
    /// What the server told of each request to open a place.
    openings: Receiver<Opening>,
}

impl Preview {
    /// The preview of `doc/paper.flm` over `files`, at `port`: when the port is named
    /// exactly, tried until it is free (a server just stopped may hold it for a moment).
    fn start(files: &Shared, port: Port) -> Preview {
        Preview::start_with(files, port, Editors::none())
    }

    /// [`Preview::start`], a place to open opened in one of `editors`.
    fn start_with(files: &Shared, port: Port, editors: Editors) -> Preview {
        Preview::start_with_folders(files, port, editors, Vec::new())
    }

    /// [`Preview::start_with`], extensions looked for in `folders`.
    fn start_with_folders(files: &Shared, port: Port, editors: Editors, folders: Vec<flm_main::packages::ExtensionFolder>) -> Preview {
        let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
        request.default_math = flm_main::seed::MathChoice::processor("passthrough");
        request.naming = Naming::Folder;
        request.extension_folders = folders;
        let watch = Watch::new(request, Arc::new(files.clone()), Vec::new());
        let handle = watch.handle();
        let (tell, openings) = channel();
        let server = wait_until("the port is free", || {
            let tell = tell.clone();
            Server::start(
                port,
                "doc/paper.flm",
                handle.clone(),
                editors.clone(),
                move |opening| {
                    let _ = tell.send(opening);
                },
                files.settings(),
            )
            .ok()
        });
        let server = Arc::new(server);
        let (send, built) = channel();
        let (done, ended) = channel();
        let serving = Arc::clone(&server);
        thread::spawn(move || {
            watch
                .run(|event| {
                    serving.tell(&event);
                    if let Event::Built(built) = event {
                        let _ = send.send(built.update.build);
                    }
                })
                .expect("the loop ran");
            let _ = done.send(());
        });
        Preview { server, handle, built, ended, openings }
    }

    /// The number of the next build done.
    fn built(&self) -> u64 {
        self.built.recv_timeout(BOUND).expect("a build within the bound")
    }

    /// The next thing the server tells of a request to open a place.
    fn opening(&self) -> Opening {
        self.openings.recv_timeout(BOUND).expect("the server tells what became of a request within the bound")
    }

    fn url(&self) -> String {
        self.server.url()
    }

    /// Ends the loop, and the server with it.
    fn stop(self) {
        self.handle.stop();
        self.ended.recv_timeout(BOUND).expect("the loop ends within the bound");
    }
}

/// A document long enough that the window scrolls, its last paragraph `last`.
fn long_document(last: &str) -> String {
    let mut text = String::from("---\ntitle: Notes\n---\n");
    for index in 0..60 {
        text.push_str(&format!("Paragraph number {index}, which says nothing in particular.\n\n"));
    }
    text.push_str(last);
    text.push('\n');
    text
}

/// The answer to `GET target` at `address`: its body.
fn fetch(address: &str, target: &str) -> String {
    let host = address.trim_start_matches("http://").trim_end_matches('/');
    let mut connection = TcpStream::connect(host).expect("the server is reached");
    connection.set_read_timeout(Some(BOUND)).unwrap();
    write!(connection, "GET {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n").unwrap();
    let mut bytes = Vec::new();
    connection.read_to_end(&mut bytes).expect("the answer within the bound");
    let text = String::from_utf8(bytes).unwrap();
    text.split_once("\r\n\r\n").map(|(_, body)| body.to_owned()).unwrap_or_default()
}

// --- the stage: a page and a stream of the test's own -------------------------------------

/// What the stage serves and has been asked.
#[derive(Default)]
struct Staged {
    /// The page at `/`.
    page: String,
    /// The header `Server-Timing` of the answer at `/`, naming a server and a build as the
    /// preview's server does; none when `None`.
    served_by: Option<String>,
    /// The files beside it, by path: media type and bytes.
    files: BTreeMap<String, (String, Vec<u8>)>,
    /// How many times each path was asked for.
    asked: BTreeMap<String, usize>,
    /// The first message of every connection to the stream (its data).
    hello: String,
    /// The open streams.
    clients: Vec<Sender<String>>,
    /// How many requests to build again arrived.
    rebuilds: usize,
    /// The body of each request to open a place, in order.
    opened: Vec<Value>,
}

/// A server of the test's own at a port of this machine: a page, the files beside it, and
/// the stream of the preview's messages, which the test writes.
struct Stage {
    address: SocketAddr,
    state: Arc<Mutex<Staged>>,
    http: Arc<tiny_http::Server>,
}

impl Stage {
    fn start(page: String, hello: Value) -> Stage {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let address = listener.local_addr().unwrap();
        let http = Arc::new(tiny_http::Server::from_listener(listener, None).unwrap());
        let state = Arc::new(Mutex::new(Staged { page, hello: hello.to_string(), ..Staged::default() }));
        let (serving, staged) = (Arc::clone(&http), Arc::clone(&state));
        thread::spawn(move || {
            for request in serving.incoming_requests() {
                let staged = Arc::clone(&staged);
                thread::spawn(move || Stage::answer(&staged, request));
            }
        });
        Stage { address, state, http }
    }

    fn answer(staged: &Mutex<Staged>, mut request: tiny_http::Request) {
        let path = request.url().split('?').next().unwrap_or_default().to_owned();
        let no_store = tiny_http::Header::from_bytes("Cache-Control", "no-store").unwrap();
        let typed = |media: &str| tiny_http::Header::from_bytes("Content-Type", media).unwrap();
        *staged.lock().unwrap().asked.entry(path.clone()).or_default() += 1;
        match path.as_str() {
            "/.flm-preview/events" => {
                let (send, messages) = channel::<String>();
                {
                    let mut staged = staged.lock().unwrap();
                    let _ = send.send(format!("retry: 300\nevent: hello\ndata: {}\n\n", staged.hello));
                    staged.clients.push(send);
                }
                let mut writer = request.into_writer();
                let head = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\n\r\n";
                if writer.write_all(head).and_then(|()| writer.flush()).is_err() {
                    return;
                }
                for message in messages {
                    if writer.write_all(message.as_bytes()).and_then(|()| writer.flush()).is_err() {
                        return;
                    }
                }
            }
            "/.flm-preview/rebuild" => {
                staged.lock().unwrap().rebuilds += 1;
                let _ = request.respond(tiny_http::Response::empty(204).with_header(no_store));
            }
            "/.flm-preview/open" => {
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                staged.lock().unwrap().opened.push(serde_json::from_str(&body).unwrap_or(Value::Null));
                let _ = request.respond(tiny_http::Response::empty(204).with_header(no_store));
            }
            "/" => {
                let (page, served_by) = {
                    let staged = staged.lock().unwrap();
                    (staged.page.clone(), staged.served_by.clone())
                };
                let mut response = tiny_http::Response::from_string(page).with_header(no_store).with_header(typed("text/html; charset=utf-8"));
                if let Some(value) = served_by {
                    response.add_header(tiny_http::Header::from_bytes("Server-Timing", value).unwrap());
                }
                let _ = request.respond(response);
            }
            other => {
                let file = staged.lock().unwrap().files.get(other.trim_start_matches('/')).cloned();
                let _ = match file {
                    Some((media, bytes)) => {
                        request.respond(tiny_http::Response::from_data(bytes).with_header(no_store).with_header(typed(&media)))
                    }
                    None => request.respond(tiny_http::Response::empty(404)),
                };
            }
        }
    }

    fn url(&self) -> String {
        format!("http://{}/", self.address)
    }

    /// Writes `text` to every open stream, as it stands.
    fn send_raw(&self, text: &str) {
        self.state.lock().unwrap().clients.retain(|client| client.send(text.to_owned()).is_ok());
    }

    /// Sends the message `event` with the data `data` to every open stream.
    fn send(&self, event: &str, data: &Value) {
        self.send_raw(&format!("event: {event}\ndata: {data}\n\n"));
    }

    /// Makes the answer at `/` name the server `stage` and the build `build`, as the
    /// preview's server does.
    fn serve_build(&self, build: u64) {
        self.state.lock().unwrap().served_by = Some(format!("flm-server;desc=\"stage\", flm-build;desc=\"{build}\""));
    }

    /// Makes the first message of every later connection name the build `build`.
    fn hello_build(&self, build: u64) {
        let mut staged = self.state.lock().unwrap();
        let mut hello: Value = serde_json::from_str(&staged.hello).unwrap();
        hello["build"] = json!(build);
        staged.hello = hello.to_string();
    }

    /// Waits until `count` streams have been opened in all.
    fn wait_connections(&self, count: usize) {
        wait_until("the page connects", || {
            (self.state.lock().unwrap().asked.get("/.flm-preview/events").copied().unwrap_or(0) >= count).then_some(())
        });
    }

    fn asked(&self, path: &str) -> usize {
        self.state.lock().unwrap().asked.get(path).copied().unwrap_or(0)
    }

    /// Waits until `count` requests to open a place have arrived in all, and gives the body
    /// of the last one.
    fn opened(&self, count: usize) -> Value {
        wait_until("the request to open a place", || {
            let staged = self.state.lock().unwrap();
            (staged.opened.len() >= count).then(|| staged.opened[count - 1].clone())
        })
    }

    /// Makes the first message of every later connection list `editors` (each `[name,
    /// label]`), `editor` the default.
    fn hello_editors(&self, editors: &[[&str; 2]], editor: Option<&str>) {
        let mut staged = self.state.lock().unwrap();
        let mut hello: Value = serde_json::from_str(&staged.hello).unwrap();
        hello["editors"] = json!(editors.iter().map(|[name, label]| json!({ "name": name, "label": label })).collect::<Vec<_>>());
        hello["editor"] = json!(editor);
        staged.hello = hello.to_string();
    }
}

impl Drop for Stage {
    fn drop(&mut self) {
        self.http.unblock();
        self.state.lock().unwrap().clients.clear();
    }
}

/// The first message of a stage whose page shows build 1 of `doc/paper.flm`.
fn hello(problems: Value) -> Value {
    json!({ "server": "stage", "document": "doc/paper.flm", "build": 1, "building": false, "problems": problems })
}

/// A page as the composer writes one, reduced to what the script needs: the preview's
/// stylesheet, the region `flm-region-content` holding `content`, `scripts` (written before
/// the page script, as the library's definitions are), and the page script.
fn staged_page(content: &str, scripts: &[&str]) -> String {
    let scripts: String = scripts.iter().map(|script| format!("<script>\n{script}</script>\n")).collect();
    format!(
        "<!DOCTYPE html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n<title>Notes</title>\n\
         <style>\n{STYLE_TEXT}</style>\n</head>\n<body>\n<main>\n<div id=\"flm-region-content\">{content}</div>\n\
         </main>\n{scripts}<script>\n{SCRIPT_TEXT}</script>\n</body>\n</html>\n"
    )
}

/// An update naming the region `flm-region-content` with the HTML `content`.
fn content_update(build: u64, content: &str, problems: Value) -> Value {
    json!({
        "build": build,
        "page": "regions",
        "regions": [{ "id": "flm-region-content", "name": "content", "kind": "html", "text": content }],
        "files": [],
        "problems": problems,
    })
}

/// The library's configuration of the formula renderer: the text part of its definition.
fn formula_configuration() -> String {
    match &mathjax_script().content[0] {
        DefContent::Inline(text) => text.clone(),
        other => panic!("the configuration is text: {other:?}"),
    }
}

/// A stand-in for the formula renderer, in the place of the program the library's
/// definition loads from the network: what the configuration relies on, and nothing more.
/// It starts as the renderer does (the configuration's `startup.pageReady`, after the page
/// is read), and typesets an element by replacing its text with an element
/// `<mjx-container>` holding it, counting each typesetting in the element's property
/// `flmTypeset`.
const RENDERER: &str = r#"(function () {
  var config = window.MathJax;
  var selector = config.startup.elements.join(', ');
  function typeset(elements) {
    elements.forEach(function (element) {
      var container = document.createElement('mjx-container');
      container.textContent = element.textContent;
      element.textContent = '';
      element.appendChild(container);
      element.flmTypeset = (element.flmTypeset || 0) + 1;
    });
    return Promise.resolve();
  }
  var renderer = {
    typesetPromise: function (elements) { return typeset(elements); },
    startup: {
      elements: config.startup.elements,
      defaultPageReady: function () {
        return typeset(Array.prototype.slice.call(document.querySelectorAll(selector)));
      }
    }
  };
  window.MathJax = renderer;
  var read = document.readyState === 'loading'
    ? new Promise(function (done) { document.addEventListener('DOMContentLoaded', done); })
    : Promise.resolve();
  renderer.startup.promise = read.then(function () {
    return config.startup.pageReady ? config.startup.pageReady() : renderer.startup.defaultPageReady();
  });
})();
"#;

/// A formula as the library writes one: its source inside the formula's element.
fn formula(source: &str) -> String {
    format!("<span class=\"flm-pres-math--passthrough\" data-flm-display=\"inline\">\\({source}\\)</span>")
}

// --- acceptance 1: a region brought up to date in place, with nothing of the preview sent -

#[test]
fn a_changed_region_is_brought_up_to_date_in_place_and_the_page_defines_no_global_name() {
    let files = Shared::with("doc/paper.flm", &long_document("The last paragraph."));
    let preview = Preview::start(&files, Port::Exactly(0));
    assert_eq!(preview.built(), 1);
    let Some(mut page) = Page::open(&preview.url()) else { return preview.stop() };
    page.wait("document.querySelector('.flm-preview-toolbar') !== null");
    // Acceptance 9: the page script's own parts and `morphdom` define no name. The copy of
    // the polyfill library `core-js` that `@json-editor/json-editor` bundles makes four
    // changes when the script loads (`vendor/json-editor/README.md`): it adds
    // `__core-js_shared__` to the window, adds `useSetter` and `useSimple` to `Symbol`, and
    // replaces `Function.prototype.toString` by a wrapper. The first three are compared with
    // a fresh frame's window, so that a refresh of the library that adds more fails here.
    let added = page.eval(
        "(() => { const frame = document.createElement('iframe'); document.body.appendChild(frame);
           const fresh = frame.contentWindow;
           const beyond = (own, base) => { const known = new Set(Object.getOwnPropertyNames(base));
             return Object.getOwnPropertyNames(own).filter(n => !known.has(n) && !/^\\d+$/.test(n)).sort(); };
           const added = { window: beyond(window, fresh), symbol: beyond(Symbol, fresh.Symbol) };
           frame.remove(); return added; })()",
    );
    assert_eq!(added, json!({ "window": ["__core-js_shared__"], "symbol": ["useSetter", "useSimple"] }));
    // The HTML the server sent holds no toolbar, no mark and no form of the tool Settings.
    let sent = fetch(&preview.url(), "/");
    assert!(sent.contains("Paragraph number 0"));
    assert!(!sent.contains("class=\"flm-preview-toolbar") && !sent.contains(" data-flm-preview-problem="));
    for marker in ["class=\"flm-preview-settings", "class=\"flm-preview-setting\"", "<form"] {
        assert!(!sent.contains(marker), "{marker}");
    }
    page.eval(
        "(() => { window.scrollTo(0, 600);
           document.querySelector('#flm-region-content p').flmKept = true; return true; })()",
    );
    page.wait("window.scrollY === 600");
    files.write("doc/paper.flm", &long_document("The last paragraph, written again."));
    preview.handle.build_again();
    assert_eq!(preview.built(), 2);
    page.wait("document.getElementById('flm-region-content').textContent.includes('written again')");
    assert_eq!(page.eval("document.querySelector('#flm-region-content p').flmKept === true"), json!(true));
    assert_eq!(page.eval("window.scrollY"), json!(600));
    drop(page);
    preview.stop();
}

// --- acceptance 2 and 3: the contract, and the formula renderer's part in it ----------------

#[test]
fn a_typeset_formula_whose_source_did_not_change_is_kept_and_the_others_are_typeset() {
    let configuration = formula_configuration();
    let content = |second: &str, third: &str| format!("<p>Text {} and {}{third}.</p>", formula("a"), formula(second));
    let stage = Stage::start(staged_page(&content("b", ""), &[&configuration, RENDERER]), hello(json!([])));
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    page.wait("document.querySelectorAll('mjx-container').length === 2");
    page.eval(
        "(() => { const [a, b] = document.querySelectorAll('.flm-pres-math--passthrough');
           a.flmKept = true; a.firstElementChild.flmKept = true; b.flmKept = true; return true; })()",
    );
    let formulas = "Array.from(document.querySelectorAll('.flm-pres-math--passthrough'))";
    // The text changes, the first formula stays, the second changes, a third comes.
    stage.send("update", &content_update(2, &content("c", &format!(" then {}", formula("d"))).replace("Text", "Words"), json!([])));
    page.wait(&format!("{formulas}.length === 3 && {formulas}.every(f => f.querySelector('mjx-container'))"));
    let state = page.eval(&format!(
        "{formulas}.map(f => ({{ kept: f.flmKept === true, inner: f.firstElementChild.flmKept === true,
           text: f.textContent, typeset: f.flmTypeset }}))"
    ));
    assert_eq!(
        state,
        json!([
            { "kept": true, "inner": true, "text": "\\(a\\)", "typeset": 1 },
            { "kept": true, "inner": false, "text": "\\(c\\)", "typeset": 2 },
            { "kept": false, "inner": false, "text": "\\(d\\)", "typeset": 1 },
        ]),
    );
    assert_eq!(page.eval("document.querySelector('p').textContent.startsWith('Words')"), json!(true));
}

#[test]
fn the_event_reaches_the_region_and_the_document_with_the_region_and_the_changed_files() {
    let listen = "window.heard = [];
      document.getElementById('flm-region-content').addEventListener('flm:region-updated',
        e => window.heard.push(['region', e.detail.region, e.detail.artifacts]));
      document.addEventListener('flm:region-updated',
        e => window.heard.push(['document', e.detail.region, e.target.id]));";
    let stage = Stage::start(staged_page("<p>One.</p>", &[listen]), hello(json!([])));
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    let mut update = content_update(2, "<p>Two.</p>", json!([]));
    update["files"] = json!(["figure.png"]);
    stage.send("update", &update);
    page.wait("window.heard.length === 2");
    assert_eq!(
        page.eval("window.heard"),
        json!([
            ["region", "flm-region-content", ["figure.png"]],
            ["document", "flm-region-content", "flm-region-content"],
        ]),
    );
}

// --- acceptance 4: the page loaded anew, the title set ----------------------------------------

#[test]
fn an_update_that_asks_for_it_loads_the_page_anew_and_the_title_is_set_without_that() {
    let stage = Stage::start(staged_page("<p>One.</p>", &[]), hello(json!([])));
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    page.eval("window.flmKept = true");
    stage.send(
        "update",
        &json!({ "build": 2, "page": "regions", "files": [], "problems": [],
                 "regions": [{ "id": "flm-region-title", "name": "title", "kind": "words", "text": "New title" }] }),
    );
    page.wait("document.title === 'New title'");
    assert_eq!(page.eval("window.flmKept === true"), json!(true));
    stage.send("update", &json!({ "build": 3, "page": "reload", "regions": [], "files": [], "problems": [] }));
    page.wait("window.flmKept === undefined && document.title === 'Notes'");
    assert_eq!(stage.asked("/"), 2);
    // Words the page has no place for: the page is loaded anew.
    stage.wait_connections(2);
    page.eval("window.flmKept = true");
    stage.send(
        "update",
        &json!({ "build": 4, "page": "regions", "files": [], "problems": [],
                 "regions": [{ "id": "flm-region-flm.meta.title", "name": "flm.meta.title", "kind": "words", "text": "New" }] }),
    );
    page.wait("window.flmKept === undefined");
    assert_eq!(stage.asked("/"), 3);
}

// --- the build a page shows, named by the answer that carried it ------------------------------

#[test]
fn a_page_knows_its_build_from_the_answer_that_carried_it() {
    // The answer and the first message name the same build: the page stays, and the next
    // update is applied in place.
    let stage = Stage::start(staged_page("<p>One.</p>", &[]), hello(json!([])));
    stage.serve_build(1);
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    page.eval("window.flmKept = true");
    stage.send("update", &content_update(2, "<p>Two.</p>", json!([])));
    page.wait("document.getElementById('flm-region-content').textContent === 'Two.'");
    assert_eq!(page.eval("window.flmKept === true"), json!(true));
    assert_eq!(stage.asked("/"), 1);
    drop(page);
    drop(stage);

    // A build ended between the answer and the first message: the page loads itself anew,
    // and stays once the answer names the build the stream names.
    let stage = Stage::start(staged_page("<p>One.</p>", &[]), hello(json!([])));
    stage.serve_build(1);
    stage.hello_build(2);
    let Some(mut page) = Page::open(&stage.url()) else { return };
    wait_until("the page loads itself anew", || (stage.asked("/") >= 2).then_some(()));
    stage.serve_build(2);
    // Each page loaded connects once before it loads itself anew; the one the answer names
    // build 2 for stays.
    page.wait(
        "performance.getEntriesByType('navigation')[0].serverTiming\
         .some(function (metric) { return metric.name === 'flm-build' && metric.description === '2'; })",
    );
    let asked = stage.asked("/");
    stage.wait_connections(asked);
    page.eval("window.flmKept = true");
    stage.send("update", &content_update(3, "<p>Three.</p>", json!([])));
    page.wait("document.getElementById('flm-region-content').textContent === 'Three.'");
    assert_eq!(page.eval("window.flmKept === true"), json!(true));
}

// --- acceptance 5 and 6: the toolbar's state, the tool Problems -------------------------------

#[test]
fn the_state_shows_a_build_under_way_and_the_count_of_problems_otherwise() {
    let stage = Stage::start(staged_page("<p>One.</p>", &[]), hello(json!([])));
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    let state = "document.querySelector('.flm-preview-state')";
    page.wait(&format!("{state}.textContent === 'No problems'"));
    stage.send("building", &json!({ "changed": ["doc/paper.flm"] }));
    page.wait(&format!("{state}.textContent === 'Building\\u2026' && {state}.hasAttribute('data-flm-preview-building')"));
    let warning = json!({ "severity": "warning", "text": "W", "file": null, "line": null, "column": null, "start": null, "end": null });
    stage.send("update", &content_update(2, "<p>Two.</p>", json!([warning, warning])));
    page.wait(&format!("{state}.textContent === '2 problems' && !{state}.hasAttribute('data-flm-preview-building')"));
    assert_eq!(page.eval(&format!("{state}.getAttribute('data-flm-preview-severity')")), json!("warning"));
}

#[test]
fn the_problems_are_listed_and_marked_and_a_click_leads_to_the_first_marked_element() {
    // A long page, the element a problem lies in at its end.
    let mut content = String::new();
    for index in 0..60 {
        content.push_str(&format!("<p data-flm-source-span='[\"doc/paper.flm\",{},{}]'>Paragraph {index}.</p>\n", index * 20, index * 20 + 15));
    }
    content.push_str(
        "<p id=\"outer\" data-flm-source-span='[\"doc/paper.flm\",2000,2040]'>Last <em id=\"inner\" \
         data-flm-source-span='[\"doc/paper.flm\",2010,2020]'>word</em> here.</p>",
    );
    let problem = |severity: &str, text: &str, start: Value, line: Value| {
        json!({ "severity": severity, "text": text, "file": if start.is_null() { Value::Null } else { json!("doc/paper.flm") },
                "line": line, "column": if line.is_null() { Value::Null } else { json!(7) }, "start": start, "end": start.as_u64().map(|s| s + 3) })
    };
    let problems = json!([
        problem("warning", "A warning.", json!(25), json!(3)),
        problem("error", "An error.\n  inside a group", json!(2012), json!(64)),
        problem("note", "A note with no place.", Value::Null, Value::Null),
    ]);
    let stage = Stage::start(staged_page(&content, &[]), hello(problems));
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    let state = "document.querySelector('.flm-preview-state')";
    page.wait(&format!("{state}.textContent === '3 problems'"));
    assert_eq!(page.eval(&format!("{state}.getAttribute('data-flm-preview-severity')")), json!("error"));
    // The marks: the innermost elements the problems lie in.
    assert_eq!(
        page.eval("Array.from(document.querySelectorAll('[data-flm-preview-problem]')).map(e => [e.id || e.textContent, e.getAttribute('data-flm-preview-problem')])"),
        json!([["Paragraph 1.", "warning"], ["inner", "error"]]),
    );
    // Unfolded, the list.
    page.eval("document.querySelector('[data-flm-preview-tool=problems]').click()");
    page.wait("document.querySelectorAll('.flm-preview-problem').length === 3");
    assert_eq!(
        page.eval(
            "Array.from(document.querySelectorAll('.flm-preview-problem')).map(p => [p.getAttribute('data-flm-preview-severity'),
               p.querySelector('.flm-preview-text').textContent, (p.querySelector('.flm-preview-place') || {}).textContent || null])"
        ),
        json!([
            ["warning", "A warning.", "doc/paper.flm:3:7"],
            ["error", "An error.\n  inside a group", "doc/paper.flm:64:7"],
            ["note", "A note with no place.", null],
        ]),
    );
    // A click on the error scrolls to the element it lies in, which flashes.
    assert_eq!(page.eval("window.scrollY"), json!(0));
    page.eval("document.querySelectorAll('.flm-preview-problem-show')[1].click()");
    page.wait("document.getElementById('inner').hasAttribute('data-flm-preview-flash')");
    // The flash ends with its animation.
    page.wait("!document.getElementById('inner').hasAttribute('data-flm-preview-flash')");
    page.wait(
        "(() => { const r = document.getElementById('inner').getBoundingClientRect();
           return window.scrollY > 0 && r.top >= 0 && r.bottom <= window.innerHeight; })()",
    );
    // Folded to its state, then unfolded.
    page.eval("document.querySelector('.flm-preview-fold').click()");
    page.wait(
        "document.querySelector('.flm-preview-toolbar').hasAttribute('data-flm-preview-folded')
           && document.querySelector('.flm-preview-tools').offsetParent === null
           && document.querySelector('.flm-preview-panel').hidden",
    );
    page.eval(&format!("{state}.click()"));
    page.wait("document.querySelector('.flm-preview-tools').offsetParent !== null && !document.querySelector('.flm-preview-panel').hidden");
    // A build without problem: the count is zero and the marks are gone.
    stage.send("update", &content_update(2, &content, json!([])));
    page.wait(&format!("{state}.textContent === 'No problems'"));
    assert_eq!(page.eval("document.querySelectorAll('[data-flm-preview-problem]').length"), json!(0));
    assert_eq!(page.eval("document.querySelectorAll('.flm-preview-problem').length"), json!(0));
}

// --- acceptance 7: the images and the files beside the page -----------------------------------

#[test]
fn a_changed_image_is_shown_and_a_file_changed_under_its_path_is_loaded_anew() {
    let svg = |width: u32| {
        format!("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='{width}' height='10'%3E%3C/svg%3E")
    };
    let content = |width: u32| format!("<p><img id=\"inline\" src=\"{}\"> <img id=\"beside\" src=\"figure.svg\"></p>", svg(width));
    let stage = Stage::start(staged_page(&content(10), &[]), hello(json!([])));
    let file = |width: u32| format!("<svg xmlns='http://www.w3.org/2000/svg' width='{width}' height='10'></svg>").into_bytes();
    stage.state.lock().unwrap().files.insert(String::from("figure.svg"), (String::from("image/svg+xml"), file(30)));
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    page.wait("document.getElementById('inline').naturalWidth === 10 && document.getElementById('beside').naturalWidth === 30");
    // The image written into the region changes with the region.
    stage.send("update", &content_update(2, &content(20), json!([])));
    page.wait("document.getElementById('inline').naturalWidth === 20");
    // A file beside the page changed under the same path: the page asks for it again.
    stage.state.lock().unwrap().files.insert(String::from("figure.svg"), (String::from("image/svg+xml"), file(40)));
    stage.send("update", &json!({ "build": 3, "page": "same", "regions": [], "files": ["figure.svg"], "problems": [] }));
    page.wait("document.getElementById('beside').naturalWidth === 40");
    assert_eq!(stage.asked("/figure.svg"), 2);
}

// --- acceptance 8: a page that connects again ----------------------------------------------

#[test]
fn a_page_connects_again_by_itself_and_loads_itself_anew_when_the_document_changed() {
    let files = Shared::with("doc/paper.flm", "---\ntitle: Notes\n---\nFirst text.\n");
    let preview = Preview::start(&files, Port::Exactly(0));
    preview.built();
    let port = preview.server.address().port();
    let Some(mut page) = Page::open(&preview.url()) else { return preview.stop() };
    page.wait("document.querySelector('.flm-preview-toolbar') !== null");
    page.eval("window.flmKept = true");
    // The server is stopped; the document changes meanwhile; the server starts again at the
    // same port.
    preview.stop();
    files.write("doc/paper.flm", "---\ntitle: Notes\n---\nSecond text.\n");
    let preview = Preview::start(&files, Port::Exactly(port));
    preview.built();
    page.wait("window.flmKept === undefined && document.body.textContent.includes('Second text.')");
    page.wait("document.querySelector('.flm-preview-toolbar') !== null");
    // The page receives the next update in place.
    page.eval("window.flmKept = true");
    files.write("doc/paper.flm", "---\ntitle: Notes\n---\nThird text.\n");
    preview.handle.build_again();
    preview.built();
    page.wait("document.body.textContent.includes('Third text.')");
    assert_eq!(page.eval("window.flmKept === true"), json!(true));
    drop(page);
    preview.stop();
}

// --- acceptance 10: the tools Rebuild and Outputs -------------------------------------------

#[test]
fn rebuild_sends_the_request_and_the_state_shows_the_build_that_follows() {
    let files = Shared::with("doc/paper.flm", "---\ntitle: Notes\n---\nText.\n");
    let preview = Preview::start(&files, Port::Exactly(0));
    assert_eq!(preview.built(), 1);
    let Some(mut page) = Page::open(&preview.url()) else { return preview.stop() };
    page.wait("document.querySelector('[data-flm-preview-tool=rebuild]') !== null");
    // The state as each message leaves it, recorded as it changes.
    page.eval(
        "(() => { const state = document.querySelector('.flm-preview-state'); window.flmStates = [];
           new MutationObserver(() => window.flmStates.push(state.textContent))
             .observe(state, { childList: true, characterData: true, subtree: true }); return true; })()",
    );
    page.eval("document.querySelector('[data-flm-preview-tool=rebuild]').click()");
    assert_eq!(preview.built(), 2);
    page.wait("window.flmStates.includes('Building\\u2026') && window.flmStates[window.flmStates.length - 1] === 'No problems'");
    drop(page);
    preview.stop();
}

#[test]
fn outputs_shows_the_four_commands_with_the_document_s_name() {
    let mut greeting = hello(json!([]));
    greeting["document"] = json!("my notes/paper.flm");
    let stage = Stage::start(staged_page("<p>One.</p>", &[]), greeting);
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    page.eval("document.querySelector('[data-flm-preview-tool=outputs]').click()");
    page.wait("document.querySelectorAll('.flm-preview-outputs code').length === 4");
    assert_eq!(
        page.eval("Array.from(document.querySelectorAll('.flm-preview-outputs code')).map(c => c.textContent)"),
        json!([
            "flm render -p html -o 'my notes/paper.html' 'my notes/paper.flm'",
            "flm render -p latex-rendered -o 'my notes/paper.tex' 'my notes/paper.flm'",
            "flm render -p latex-recomposed -o 'my notes/paper.tex' 'my notes/paper.flm'",
            "flm check 'my notes/paper.flm'",
        ]),
    );
    // The tool Rebuild sends its request.
    page.eval("document.querySelector('[data-flm-preview-tool=rebuild]').click()");
    wait_until("the request to build again", || (stage.state.lock().unwrap().rebuilds == 1).then_some(()));
}

// --- acceptance 11: the waiting page -----------------------------------------------------------

#[test]
fn the_waiting_page_shows_the_problems_and_is_replaced_by_the_document_once_built() {
    let files = Shared::with("doc/paper.flm", "---\ntitle: [unclosed\n---\nText.\n");
    let preview = Preview::start(&files, Port::Exactly(0));
    preview.built();
    let Some(mut page) = Page::open(&preview.url()) else { return preview.stop() };
    page.wait("document.documentElement.hasAttribute('data-flm-preview-waiting') && document.querySelectorAll('.flm-preview-problem').length > 0");
    assert_eq!(page.eval("document.querySelector('.flm-preview-state').getAttribute('data-flm-preview-severity')"), json!("error"));
    assert_eq!(page.eval("document.body.textContent.includes('doc/paper.flm')"), json!(true));
    files.write("doc/paper.flm", "---\ntitle: Notes\n---\nThe mended text.\n");
    preview.handle.build_again();
    preview.built();
    page.wait("!document.documentElement.hasAttribute('data-flm-preview-waiting') && document.body.textContent.includes('The mended text.')");
    page.wait("document.querySelector('.flm-preview-state').textContent === 'No problems'");
    drop(page);
    preview.stop();
}

// --- acceptance 12: messages that are not understood ---------------------------------------

#[test]
fn no_message_the_script_does_not_understand_breaks_the_page() {
    let stage = Stage::start(staged_page("<p>One.</p>", &[]), hello(json!([])));
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    page.eval("window.flmKept = true");
    for junk in [
        "event: update\ndata: not JSON\n\n",
        "event: update\ndata: [1, 2]\n\n",
        "event: update\ndata: {\"page\": \"regions\", \"regions\": [null, {\"id\": 5}, 7], \"problems\": \"none\"}\n\n",
        "event: update\ndata: {\"page\": 12, \"files\": {\"a\": 1}, \"problems\": [null, 3, {\"severity\": 4}]}\n\n",
        "event: hello\ndata: {\"document\": 5, \"problems\": {}}\n\n",
        "event: building\ndata: 17\n\n",
        "event: something-new\ndata: {}\n\n",
        "data: an unnamed message\n\n",
    ] {
        stage.send_raw(junk);
    }
    stage.send("update", &content_update(2, "<p>Two.</p>", json!([])));
    page.wait("document.getElementById('flm-region-content').textContent === 'Two.'");
    assert_eq!(page.eval("window.flmKept === true"), json!(true));
    assert_eq!(page.eval("document.querySelector('.flm-preview-state').textContent"), json!("No problems"));
}

// --- the gestures and *Open in editor* -------------------------------------------------------

/// Makes a mouse button go down, then the click, on the element `selector` finds, with the
/// modifier keys `keys` (`ctrlKey`, `metaKey`, `shiftKey` as a JavaScript object's fields).
fn gesture(page: &mut Page, selector: &str, keys: &str) {
    page.eval(&format!(
        "(() => {{ const target = document.querySelector('{selector}'); const keys = {keys};
           for (const type of ['mousedown', 'mouseup', 'click']) {{
             target.dispatchEvent(new MouseEvent(type, Object.assign({{ bubbles: true, cancelable: true, button: 0 }}, keys)));
           }} return true; }})()"
    ));
}

/// Chooses the editor `name` in the toolbar's menu of editors, as the author does.
fn choose_editor(page: &mut Page, name: &str) {
    page.eval(&format!(
        "(() => {{ const menu = document.querySelector('.flm-preview-editor'); menu.value = '{name}';
           menu.dispatchEvent(new Event('change', {{ bubbles: true }})); return true; }})()"
    ));
}

#[cfg(unix)]
#[test]
fn a_gesture_and_open_in_editor_send_the_place_and_the_editor_opens_it() {
    use editor::{fake_editor, folder, recorded};
    let folder = folder("page_gestures");
    let record = folder.join("record");
    let zed_record = folder.join("zed-record");
    fake_editor(&folder, "code", &record);
    fake_editor(&folder, "zed", &zed_record);
    let editors = Editors::find(None, Some(folder.as_os_str()), &[]).unwrap();
    let files = Shared::with("doc/paper.flm", "---\ntitle: Notes\n---\nÇa été \\emph{ici}.\n\nThe \\nosuchmacro here.\n");
    let preview = Preview::start_with(&files, Port::Exactly(0), editors);
    assert_eq!(preview.built(), 1);
    let Some(mut page) = Page::open(&preview.url()) else { return preview.stop() };
    page.wait("document.querySelector('[data-flm-preview-tool=problems]') !== null && document.querySelector('em') !== null");
    // The menu of editors lists the two editors found, and starts on the default, the first.
    page.wait("document.querySelector('.flm-preview-editor').options.length === 2");
    assert_eq!(page.eval("document.querySelector('.flm-preview-editor').value"), json!("code"));
    let file = std::path::absolute("doc/paper.flm").unwrap().display().to_string();
    let opened = |line: usize, column: usize| Opening::Opened {
        editor: "code",
        file: PathBuf::from(&file),
        line,
        column,
    };
    // Cmd and Shift on the emphasized word: its place, line 4 below the front matter of
    // three lines, column 8 after seven characters of which two take two bytes; it flashes.
    gesture(&mut page, "em", "{ metaKey: true, shiftKey: true }");
    page.wait("document.querySelector('em').hasAttribute('data-flm-preview-flash')");
    assert_eq!(recorded(&record), ["--goto", &format!("{file}:4:8")]);
    std::fs::remove_file(&record).unwrap();
    // Ctrl and Shift on a word with no place of its own: the place of the element around it.
    page.eval("(() => { const b = document.createElement('b'); b.id = 'placeless'; b.textContent = 'x'; document.querySelector('em').appendChild(b); return true; })()");
    gesture(&mut page, "#placeless", "{ ctrlKey: true, shiftKey: true }");
    assert_eq!(recorded(&record), ["--goto", &format!("{file}:4:8")]);
    std::fs::remove_file(&record).unwrap();
    // *Open in editor* on the problem: the problem's place.
    page.eval("document.querySelector('[data-flm-preview-tool=problems]').click()");
    page.wait("document.querySelector('.flm-preview-problem-open') !== null");
    page.eval("document.querySelector('.flm-preview-problem-open').click()");
    assert_eq!(recorded(&record), ["--goto", &format!("{file}:6:5")]);
    for (line, column) in [(4, 8), (4, 8), (6, 5)] {
        assert_eq!(preview.opening(), opened(line, column));
    }
    // A gesture on a button of the toolbar is the button's: the *Outputs* panel opens, and
    // no place is asked for, although an element with a place (the start of the content)
    // stands just before the toolbar, where a gesture the toolbar did not keep would find
    // one. The next place the server is asked to open is the one of the gesture after it,
    // on the document.
    std::fs::remove_file(&record).unwrap();
    page.eval(
        "(() => { const p = document.createElement('p'); p.textContent = 'placed';
           p.setAttribute('data-flm-source-span', '[\"doc/paper.flm\",0,1]');
           document.body.insertBefore(p, document.querySelector('.flm-preview-toolbar')); return true; })()",
    );
    gesture(&mut page, "[data-flm-preview-tool=outputs]", "{ ctrlKey: true, shiftKey: true }");
    page.wait("!document.querySelector('.flm-preview-panel').hidden && document.querySelector('.flm-preview-panel').getAttribute('data-flm-preview-tool') === 'outputs'");
    gesture(&mut page, "em", "{ ctrlKey: true, shiftKey: true }");
    assert_eq!(recorded(&record), ["--goto", &format!("{file}:4:8")]);
    assert_eq!(preview.opening(), opened(4, 8));
    // Zed chosen in the menu: the place opens in Zed.
    choose_editor(&mut page, "zed");
    gesture(&mut page, "em", "{ ctrlKey: true, shiftKey: true }");
    assert_eq!(recorded(&zed_record), [format!("{file}:4:8")]);
    assert_eq!(preview.opening(), Opening::Opened { editor: "zed", file: PathBuf::from(&file), line: 4, column: 8 });
    drop(page);
    preview.stop();
}

/// The content of the pages of the tests of the menu of editors: an element whose place in
/// the source starts at byte 3.
const MENU_CONTENT: &str = "<p data-flm-source-span='[\"doc/paper.flm\",3,9]'>Some <em>text</em>.</p>";

/// Waits until the toolbar's menu of editors lists `count` editors.
fn menu_lists(page: &mut Page, count: usize) {
    page.wait(&format!(
        "document.querySelector('.flm-preview-editor') !== null && document.querySelector('.flm-preview-editor').options.length === {count}"
    ));
}

/// Makes the stage's page load itself anew, and waits until it connected again, its
/// `connections`-th connection.
fn reload_staged(stage: &Stage, page: &mut Page, connections: usize) {
    stage.send("update", &json!({ "build": 1, "page": "reload", "regions": [], "files": [], "problems": [] }));
    stage.wait_connections(connections);
    page.wait("document.querySelector('.flm-preview-toolbar') !== null");
}

#[test]
fn the_menu_of_editors_chooses_the_editor_of_each_request_and_the_browser_remembers_it() {
    let problem = json!({ "severity": "error", "text": "A problem.", "file": "doc/paper.flm", "line": 4, "column": 1, "start": 20, "end": 25 });
    let stage = Stage::start(staged_page(MENU_CONTENT, &[]), hello(json!([problem])));
    stage.hello_editors(&[["code", "VS Code"], ["zed", "Zed"], ["emacsclient", "Emacs"]], Some("zed"));
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    // The editors of the first message, by their labels, the default shown; the menu sits
    // in the bar, between the tools and the button that folds the toolbar.
    menu_lists(&mut page, 3);
    assert_eq!(
        page.eval("Array.from(document.querySelector('.flm-preview-editor').options).map(o => [o.value, o.textContent])"),
        json!([["code", "VS Code"], ["zed", "Zed"], ["emacsclient", "Emacs"]]),
    );
    assert_eq!(page.eval("document.querySelector('.flm-preview-editor').value"), json!("zed"));
    assert_eq!(page.eval("document.querySelector('.flm-preview-editor').getAttribute('aria-label')"), json!("Editor"));
    assert_eq!(
        page.eval("(() => { const menu = document.querySelector('.flm-preview-editor'); return !menu.hidden && menu.parentElement.classList.contains('flm-preview-bar') && menu.previousElementSibling.classList.contains('flm-preview-tools') && menu.nextElementSibling.classList.contains('flm-preview-fold'); })()"),
        json!(true),
    );
    // A gesture asks for the editor shown.
    gesture(&mut page, "em", "{ ctrlKey: true, shiftKey: true }");
    assert_eq!(stage.opened(1), json!({ "file": "doc/paper.flm", "start": 3, "editor": "zed" }));
    // Another editor chosen: the gesture and *Open in editor* ask for it.
    choose_editor(&mut page, "emacsclient");
    gesture(&mut page, "em", "{ metaKey: true, shiftKey: true }");
    assert_eq!(stage.opened(2), json!({ "file": "doc/paper.flm", "start": 3, "editor": "emacsclient" }));
    page.eval("document.querySelector('[data-flm-preview-tool=problems]').click()");
    page.wait("document.querySelector('.flm-preview-problem-open') !== null");
    page.eval("document.querySelector('.flm-preview-problem-open').click()");
    assert_eq!(stage.opened(3), json!({ "file": "doc/paper.flm", "start": 20, "editor": "emacsclient" }));
    // A gesture on the menu is the menu's: the next request is the gesture's after it,
    // although an element with a place stands just before the toolbar, where a gesture the
    // toolbar did not keep would find one.
    page.eval(
        "(() => { const p = document.createElement('p'); p.textContent = 'placed';
           p.setAttribute('data-flm-source-span', '[\"doc/paper.flm\",0,1]');
           document.body.insertBefore(p, document.querySelector('.flm-preview-toolbar')); return true; })()",
    );
    gesture(&mut page, ".flm-preview-editor", "{ ctrlKey: true, shiftKey: true }");
    gesture(&mut page, "em", "{ ctrlKey: true, shiftKey: true }");
    assert_eq!(stage.opened(4), json!({ "file": "doc/paper.flm", "start": 3, "editor": "emacsclient" }));
    assert_eq!(stage.state.lock().unwrap().opened.len(), 4);
    // The folded toolbar hides the menu.
    page.eval("document.querySelector('.flm-preview-fold').click()");
    assert_eq!(page.eval("getComputedStyle(document.querySelector('.flm-preview-editor')).display"), json!("none"));
    page.eval("document.querySelector('.flm-preview-state').click()");
    // The page loaded anew shows the editor chosen.
    reload_staged(&stage, &mut page, 2);
    menu_lists(&mut page, 3);
    assert_eq!(page.eval("document.querySelector('.flm-preview-editor').value"), json!("emacsclient"));
    // A server that did not find that editor: its default is shown, and the choice is kept
    // for a server that finds it.
    stage.hello_editors(&[["code", "VS Code"], ["zed", "Zed"]], Some("code"));
    reload_staged(&stage, &mut page, 3);
    menu_lists(&mut page, 2);
    assert_eq!(page.eval("document.querySelector('.flm-preview-editor').value"), json!("code"));
    gesture(&mut page, "em", "{ ctrlKey: true, shiftKey: true }");
    assert_eq!(stage.opened(5)["editor"], "code");
    stage.hello_editors(&[["zed", "Zed"], ["emacsclient", "Emacs"]], Some("zed"));
    reload_staged(&stage, &mut page, 4);
    menu_lists(&mut page, 2);
    assert_eq!(page.eval("document.querySelector('.flm-preview-editor').value"), json!("emacsclient"));
}

#[test]
fn with_no_editor_found_the_menu_is_hidden_and_a_request_asks_for_no_editor() {
    let stage = Stage::start(staged_page(MENU_CONTENT, &[]), hello(json!([])));
    stage.hello_editors(&[], None);
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    // The tool *Outputs* shows the commands once the first message has come.
    page.eval("document.querySelector('[data-flm-preview-tool=outputs]').click()");
    page.wait("document.querySelectorAll('.flm-preview-outputs code').length === 4");
    assert_eq!(page.eval("document.querySelector('.flm-preview-editor').hidden"), json!(true));
    assert_eq!(page.eval("getComputedStyle(document.querySelector('.flm-preview-editor')).display"), json!("none"));
    gesture(&mut page, "em", "{ ctrlKey: true, shiftKey: true }");
    assert_eq!(stage.opened(1), json!({ "file": "doc/paper.flm", "start": 3 }));
}

#[test]
fn what_the_server_opened_nothing_for_is_shown_in_the_toolbar() {
    let files = Shared::with("doc/paper.flm", "---\ntitle: Notes\n---\nSome \\emph{text}.\n");
    let preview = Preview::start(&files, Port::Exactly(0));
    assert_eq!(preview.built(), 1);
    let Some(mut page) = Page::open(&preview.url()) else { return preview.stop() };
    page.wait("document.querySelector('em') !== null && document.querySelector('.flm-preview-notice') !== null");
    assert_eq!(page.eval("document.querySelector('.flm-preview-notice').hidden"), json!(true));
    gesture(&mut page, "em", "{ ctrlKey: true, shiftKey: true }");
    page.wait("!document.querySelector('.flm-preview-notice').hidden");
    let notice = page.eval("document.querySelector('.flm-preview-notice').textContent");
    assert!(notice.as_str().unwrap().starts_with("no editor was found"), "{notice}");
    drop(page);
    preview.stop();
}

#[test]
fn the_reference_copied_is_that_of_the_nearest_element_a_label_names() {
    let content = "<section id=\"sec-intro\"><h2 id=\"heading\">Intro</h2><p>Some <em id=\"inner\">text</em>.</p></section>\
                   <p id=\"other\">Outside <span id=\"plain\">here</span>.</p>";
    let mut first = hello(json!([]));
    first["references"] = json!({ "sec-intro": "sec:intro", "unused": "eq:unused" });
    let stage = Stage::start(staged_page(content, &[]), first);
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    page.wait("document.querySelector('.flm-preview-toolbar') !== null");
    // The copies, recorded instead of written to the system's clipboard.
    page.eval(
        "(() => { window.flmCopied = []; Object.defineProperty(navigator, 'clipboard', { configurable: true,
           value: { writeText: (text) => { window.flmCopied.push(text); return Promise.resolve(); } } }); return true; })()",
    );
    // Nothing around `#plain` has a label: nothing is copied. Then the element inside the
    // section, the section's heading, the section itself.
    gesture(&mut page, "#plain", "{ ctrlKey: true }");
    gesture(&mut page, "#inner", "{ ctrlKey: true }");
    page.wait("document.getElementById('sec-intro').hasAttribute('data-flm-preview-flash')");
    gesture(&mut page, "#heading", "{ metaKey: true }");
    gesture(&mut page, "#sec-intro", "{ ctrlKey: true }");
    page.wait("window.flmCopied.length === 3");
    assert_eq!(page.eval("window.flmCopied"), json!(["\\ref{sec:intro}", "\\ref{sec:intro}", "\\ref{sec:intro}"]));
    // An update brings the labels of its build.
    let mut update = content_update(2, content, json!([]));
    update["references"] = json!({ "inner": "eq:inner" });
    stage.send("update", &update);
    page.wait("document.querySelector('.flm-preview-state').textContent === 'No problems'");
    gesture(&mut page, "#inner", "{ ctrlKey: true }");
    page.wait("window.flmCopied.length === 4");
    assert_eq!(page.eval("window.flmCopied[3]"), json!("\\ref{eq:inner}"));
}

// --- the tool Settings ------------------------------------------------------------------------

/// Each feature the schema of a front matter `schema` lists, in its order, with its
/// description: that of its settings' schema, or of the type it names under `$defs`.
fn listed_features(schema: &Value) -> Vec<(String, String)> {
    let listed = schema["properties"]["flm"]["properties"]["features"]["properties"].as_object().unwrap();
    listed
        .iter()
        .map(|(name, entry)| {
            let settings = entry["anyOf"].as_array().unwrap().iter().find(|branch| branch.get("const").is_none() && branch["type"] != "null").unwrap();
            let described = match settings["$ref"].as_str() {
                Some(reference) => &schema["$defs"][reference.trim_start_matches("#/$defs/")],
                None => settings,
            };
            (name.clone(), described["description"].as_str().unwrap_or_default().to_owned())
        })
        .collect()
}

/// Opens the tool *Settings* and waits until it shows the features.
fn open_settings(page: &mut Page) {
    page.eval("document.querySelector('button[data-flm-preview-tool=settings]').click()");
    page.wait("document.querySelectorAll('.flm-preview-setting').length > 0");
}

/// Unfolds the section of the feature `name` and waits until its form is made.
fn unfold(page: &mut Page, name: &str) {
    page.eval(&format!("document.querySelector('[data-flm-preview-feature=\"{name}\"]').open = true"));
    page.wait(&format!("document.querySelector('[data-flm-preview-feature=\"{name}\"] .je-ready') !== null"));
}

/// The front matter of the document of the tests of the tool *Settings*.
const SETTINGS_DOC: &str =
    "---\ntitle: Notes\nflm:\n  features:\n    math: {numbered_equations: false}\n    theorems: false\n---\nText.\n";

#[test]
fn settings_lists_the_features_and_shows_and_saves_what_the_front_matter_writes() {
    let files = Shared::with("doc/paper.flm", SETTINGS_DOC);
    let preview = Preview::start(&files, Port::Exactly(0));
    assert_eq!(preview.built(), 1);
    let Some(mut page) = Page::open(&preview.url()) else { return preview.stop() };
    page.wait("document.querySelector('[data-flm-preview-tool=settings]') !== null");
    open_settings(&mut page);
    // Every feature of the schema, in its order, with its description and its switch: off
    // for the one the front matter switches off.
    let shown = page.eval(
        "Array.from(document.querySelectorAll('.flm-preview-setting')).map(s => [s.getAttribute('data-flm-preview-feature'),
           s.querySelector('.flm-preview-setting-description').textContent, s.querySelector('.flm-preview-setting-switch').checked])",
    );
    let expected: Vec<Value> = listed_features(&front_matter_schema())
        .into_iter()
        .map(|(name, description)| json!([name, description, name != "theorems"]))
        .collect();
    assert!(expected.len() > 30);
    assert_eq!(shown, Value::Array(expected));
    // A feature's keys, with their descriptions, the value the front matter writes, and no
    // field for a key it does not write.
    unfold(&mut page, "math");
    assert_eq!(page.eval("document.querySelector('[name=\"root[numbered_equations]\"]').value"), json!(""));
    assert_eq!(page.eval("document.querySelector('[name=\"root[surround_display_math]\"]')"), Value::Null);
    let form = page.eval("document.querySelector('[data-flm-preview-feature=math] .flm-preview-setting-form').textContent");
    assert!(form.as_str().unwrap().contains("Whether the lines of `equation`, `align` and `gather` are numbered"), "{form}");
    // A change saved: the value changed, a key added, the feature switched off on again.
    page.eval(
        "(() => { const field = document.querySelector('[name=\"root[numbered_equations]\"]'); field.value = '1';
           field.dispatchEvent(new Event('change', { bubbles: true }));
           document.getElementById('root-emit_flm_math_environment_macro').click();
           document.querySelector('[data-flm-preview-feature=theorems] .flm-preview-setting-switch').click();
           document.querySelector('.flm-preview-settings-save').click(); return true; })()",
    );
    page.wait("document.querySelector('.flm-preview-settings-status').textContent.startsWith('Saved')");
    // The front matter edited in place: the map of `math` keeps its style (on one line), the
    // line of `theorems` is gone, every other byte is as it was.
    assert_eq!(
        files.text("doc/paper.flm"),
        "---\ntitle: Notes\nflm:\n  features:\n    math: {numbered_equations: true, \
         emit_flm_math_environment_macro: false}\n---\nText.\n"
    );
    // After the build, the tool opened again shows the saved values.
    preview.handle.build_again();
    assert_eq!(preview.built(), 2);
    page.eval("document.querySelector('button[data-flm-preview-tool=settings]').click()");
    page.wait("document.querySelector('.flm-preview-settings') === null");
    open_settings(&mut page);
    assert_eq!(page.eval("document.querySelector('[data-flm-preview-feature=theorems] .flm-preview-setting-switch').checked"), json!(true));
    unfold(&mut page, "math");
    assert_eq!(page.eval("document.querySelector('[name=\"root[numbered_equations]\"]').value"), json!("1"));
    assert_eq!(page.eval("document.querySelector('[name=\"root[emit_flm_math_environment_macro]\"]').value"), json!(""));
    drop(page);
    preview.stop();
}

#[test]
fn a_refused_save_shows_its_reason_in_the_panel_and_writes_nothing() {
    let files = Shared::with("doc/paper.flm", SETTINGS_DOC);
    let preview = Preview::start(&files, Port::Exactly(0));
    assert_eq!(preview.built(), 1);
    let Some(mut page) = Page::open(&preview.url()) else { return preview.stop() };
    page.wait("document.querySelector('[data-flm-preview-tool=settings]') !== null");
    open_settings(&mut page);
    // The author is in the middle of an edit of the front matter.
    let broken = "---\ntitle: [unclosed\n---\nText.\n";
    files.write("doc/paper.flm", broken);
    page.eval("document.querySelector('.flm-preview-settings-save').click()");
    page.wait("document.querySelector('.flm-preview-settings-status').textContent.includes('must be valid YAML')");
    assert!(page.eval("document.querySelector('.flm-preview-settings-status').textContent").as_str().unwrap().starts_with("doc/paper.flm:3:1: "));
    assert_eq!(files.text("doc/paper.flm"), broken);
    drop(page);
    preview.stop();
}

/// The tool *Settings* of a document using `wordcount`, the extension written in JavaScript
/// of `flm-ext`'s tests, held in memory beside it: the form of the scripted feature has a
/// field for each setting the front matter writes, from the schema of the preparation.
#[cfg(feature = "js")]
#[test]
fn the_settings_tool_shows_the_form_of_a_scripted_feature() {
    let doc = "---\nflm:\n  use: [wordcount]\n  features:\n    wordcount:\n      min_length: 4\n      report_counts: true\n---\nSay \\sayhi{you}.\n";
    let files = Shared::with("doc/paper.flm", doc);
    for (name, text) in [
        ("flmextension.yaml", include_str!("../../flm-ext/tests/js/wordcount/flmextension.yaml")),
        ("main.js", include_str!("../../flm-ext/tests/js/wordcount/main.js")),
        ("helpers.js", include_str!("../../flm-ext/tests/js/wordcount/helpers.js")),
    ] {
        files.write(&format!("doc/_flmextensions/wordcount/{name}"), text);
    }
    let folders = vec![flm_main::packages::ExtensionFolder { path: PathBuf::from("doc/_flmextensions"), required: false }];
    let preview = Preview::start_with_folders(&files, Port::Exactly(0), Editors::none(), folders);
    assert_eq!(preview.built(), 1);
    let Some(mut page) = Page::open(&preview.url()) else { return preview.stop() };
    page.wait("document.querySelector('[data-flm-preview-tool=settings]') !== null");
    open_settings(&mut page);
    unfold(&mut page, "wordcount");
    for key in ["min_length", "report_counts"] {
        assert_ne!(page.eval(&format!("document.querySelector('[name=\"root[{key}]\"]')")), Value::Null, "{key}");
    }
    let form = page.eval("document.querySelector('[data-flm-preview-feature=wordcount] .flm-preview-setting-form').textContent");
    assert!(form.as_str().unwrap().contains("The characters a word must have to count."), "{form}");
    drop(page);
    preview.stop();
}

// --- the math-mathjax extension: a page in each of its two modes ---------------------------------

/// The page of the document `text`, rendered as HTML with the workspace's folder of
/// extensions in JavaScript `extensions/js/` on the search path, read from the disk (the document is written
/// to the file `name` in a folder of the tests' own).
#[cfg(feature = "js")]
fn math_mathjax_page(name: &str, text: &str) -> String {
    use flm_main::output::{MemorySink, Sink};
    let folder = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("math-mathjax-browser");
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join(name);
    std::fs::write(&path, text).unwrap();
    let mut request = Request::new(Input::File(path));
    request.format = Some(String::from("html"));
    let extensions = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../extensions/js"));
    request.extension_folders = vec![flm_main::packages::ExtensionFolder { path: extensions, required: true }];
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    let prepared = flm_main::session::prepare(request, Arc::new(flm_main::files::DiskFiles), &[]).unwrap();
    let built = flm_main::session::build(&prepared).unwrap();
    assert!(built.diagnostics.is_empty(), "{:?}", built.diagnostics);
    let entry = sink.files().into_values().find(|file| file.entry).unwrap();
    String::from_utf8(entry.bytes).unwrap()
}

/// The page of the extension's `mathml` mode, as the browser lays it out: the `math`
/// element is MathML, the link of a reference written inside `\text{…}` is inside its
/// `mtext` and laid out there, and the labeled line's row carries the label's identifier.
#[cfg(feature = "js")]
#[test]
fn the_mathml_page_of_math_mathjax_shows_the_formulas_with_their_links_and_rows() {
    let document = "---\nflm:\n  use: [math-mathjax]\n  features:\n    math-mathjax:\n      mode: mathml\n---\n\
                    \\begin{align}\n  a &= b \\label{eq:a}\n\\end{align}\n\n\
                    See \\(x^2 + \\text{see \\ref{eq:a}}\\).\n";
    let html = math_mathjax_page("mathml.flm", document);
    let stage = Stage::start(html, hello(json!([])));
    let Some(mut page) = Page::open(&stage.url()) else { return };
    page.wait("document.querySelector('math') !== null");
    assert_eq!(page.eval("document.querySelector('math') instanceof MathMLElement"), json!(true));
    let link = "document.querySelector('mtext a.flm-pres-link')";
    assert_eq!(page.eval(&format!("{link}.getAttribute('href')")), json!("#eq-a"));
    assert_eq!(page.eval(&format!("{link}.getBoundingClientRect().width > 0")), json!(true));
    assert_eq!(page.eval("document.getElementById('eq-a').localName"), json!("mtr"));
    assert_eq!(page.eval("document.getElementById('eq-a') instanceof MathMLElement"), json!(true));
    // The tag's cell, at the end of the row, stands to the right of the line's cells.
    assert_eq!(
        page.eval(
            "(() => { const cells = document.getElementById('eq-a').children;
               return cells[cells.length - 1].getBoundingClientRect().left > cells[0].getBoundingClientRect().right; })()"
        ),
        json!(true)
    );
    // The line's own cells (between the empty first cell and the tag's) stand in the
    // middle of the formula's block, within a few pixels.
    assert_eq!(
        page.eval(
            "(() => { const row = document.getElementById('eq-a'); const cells = row.children;
               const left = cells[1].getBoundingClientRect().left;
               const right = cells[cells.length - 2].getBoundingClientRect().right;
               const block = row.closest('math').getBoundingClientRect();
               return right - left > 0 && Math.abs((left + right) / 2 - (block.left + block.right) / 2) < 4; })()"
        ),
        json!(true)
    );
}

/// The page of the extension's `mathml` mode, for an `align` whose lines hold different
/// numbers of cells: the cells of the first line, two pairs, stand next to each other in
/// the middle of the formula's block, not spread across it.
#[cfg(feature = "js")]
#[test]
fn the_mathml_page_of_math_mathjax_keeps_the_cells_of_an_uneven_align_together() {
    let document = "---\nflm:\n  use: [math-mathjax]\n  features:\n    math-mathjax:\n      mode: mathml\n---\n\
                    \\begin{align}\n  a &= b & c &= d \\label{eq:a} \\\\\n  e &= f \\nonumber\n\\end{align}\n";
    let html = math_mathjax_page("mathml-uneven.flm", document);
    let stage = Stage::start(html, hello(json!([])));
    let Some(mut page) = Page::open(&stage.url()) else { return };
    page.wait("document.getElementById('eq-a') !== null");
    // The line's four cells (between the empty first cell and the tag's): no gap
    // between two of them wider than a few pixels, and together far narrower than the
    // formula's block, their middle at the block's middle.
    assert_eq!(
        page.eval(
            "(() => { const row = document.getElementById('eq-a'); const cells = [...row.children];
               if (cells.length !== 6) return 'cells: ' + cells.length;
               const boxes = cells.slice(1, 5).map((cell) => cell.getBoundingClientRect());
               const block = row.closest('math').getBoundingClientRect();
               for (let i = 1; i < boxes.length; i++) {
                 if (boxes[i].left - boxes[i - 1].right > 4) return 'gap after cell ' + i;
               }
               const left = boxes[0].left, right = boxes[boxes.length - 1].right;
               if (right - left > (block.right - block.left) / 2) return 'spread: ' + (right - left);
               if (Math.abs((left + right) / 2 - (block.left + block.right) / 2) > 4) return 'off center';
               return true; })()"
        ),
        json!(true)
    );
}

/// MathJax's own program and TeX components from the repository's npm workspace
/// (`npm/node_modules/@mathjax/src`, present after `npm install` in `npm/`), as files the
/// stage serves under `mathjax/`; `None` when the folder is absent.
#[cfg(feature = "js")]
fn local_mathjax() -> Option<Vec<(String, Vec<u8>)>> {
    let bundle = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../npm/node_modules/@mathjax/src/bundle"));
    let mut files = vec![(String::from("mathjax/tex-chtml.js"), std::fs::read(bundle.join("tex-chtml.js")).ok()?)];
    for entry in std::fs::read_dir(bundle.join("input/tex/extensions")).ok()? {
        let entry = entry.ok()?;
        let name = entry.file_name().to_string_lossy().into_owned();
        files.push((format!("mathjax/input/tex/extensions/{name}"), std::fs::read(entry.path()).ok()?));
    }
    Some(files)
}

/// The page of the extension's `load-in-browser` mode, typeset by MathJax 4 (served from
/// the repository's npm workspace, so that nothing is fetched from the network): a formula
/// written inside `\text{…}` of another is typeset with it, as one formula; the page's
/// configuration lists `base` among the TeX packages and loads no component for it, since
/// MathJax 4 has none.
#[cfg(feature = "js")]
#[test]
fn the_load_in_browser_page_of_math_mathjax_typesets_a_formula_inside_a_text_command() {
    let Some(mathjax) = local_mathjax() else {
        eprintln!("skipped: run `npm install` in npm/ for MathJax's own files");
        return;
    };
    let document = "---\nflm:\n  use: [math-mathjax]\n  features:\n    math-mathjax:\n      mode: load-in-browser\n      \
                    loader_url: mathjax/tex-chtml.js\n---\nSee \\(\\text{a \\(y\\) b}\\).\n";
    let html = math_mathjax_page("browser.flm", document);
    assert!(html.contains("packages: ['base', 'texhtml']"), "{html}");
    let stage = Stage::start(html, hello(json!([])));
    for (path, bytes) in mathjax {
        stage.state.lock().unwrap().files.insert(path, (String::from("text/javascript"), bytes));
    }
    let Some(mut page) = Page::open(&stage.url()) else { return };
    page.wait("document.querySelector('.math-mathjax-formula mjx-container') !== null");
    // One formula, typeset whole: `y` is a typeset identifier, the two holes are the text
    // around it, and none of the TeX's characters remains.
    let formula = "document.querySelector('.math-mathjax-formula')";
    assert_eq!(page.eval(&format!("{formula}.querySelectorAll('mjx-container').length")), json!(1));
    assert_eq!(page.eval(&format!("{formula}.querySelectorAll('mjx-container mjx-mi').length")), json!(1));
    let text = page.eval(&format!("{formula}.querySelector('mjx-container').textContent"));
    let text = text.as_str().unwrap();
    assert!(text.contains("a ") && text.contains(" b") && !text.contains('\\'), "{text}");
    assert_eq!(stage.asked("/mathjax/input/tex/extensions/texhtml.js"), 1);
    assert_eq!(stage.asked("/mathjax/input/tex/extensions/base.js"), 0);
}
