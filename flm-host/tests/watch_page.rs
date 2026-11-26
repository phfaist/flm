//! The preview's page script (the watch client, `npm/flm-watchclient/`), in Chrome
//! without a window: the update of a region in place, the contract for a page updated in
//! place with the formula renderer's part in it, the scroll to what an update changed,
//! the marks of the problems, the gesture that opens a place in the editor, the messages
//! with the tools page, the waiting page, the connection; the tools page's output view,
//! panel and status bar, and a write of the front matter from its *Metadata* tab; and the
//! pages of the `math-mathjax` extension in its two modes (feature `js`).
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
#[cfg(feature = "js")]
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

use browser::{BOUND, Page, wait_until};
use flm_core::core::registry::DefContent;
#[cfg(feature = "js")]
use flm_core::progress::Scope;
use flm_core::stdlib::backends::html::mathjax_script;
use flm_host::files::{FileError, Files, MemoryFiles};
use flm_host::output::Naming;
use flm_host::session::{Input, Request};
use flm_host::stdpkg::preview::{SCRIPT_TEXT, STYLE_TEXT};
use flm_host::watch::editor::Editors;
use flm_host::watch::server::{Opening, Port, Server};
use flm_host::watch::settings::DocumentSettings;
use flm_host::watch::{Event, Handle, Watch};
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
    fn read(&self, path: &str) -> Result<Vec<u8>, FileError> {
        self.0.lock().unwrap().read(path)
    }

    fn entries(&self, path: &str) -> Result<Vec<String>, FileError> {
        self.0.lock().unwrap().entries(path)
    }
}

/// The schema of a front matter for the features of the packages the preview uses
/// ([`Preview::start`]), in their order: `std`, then `math-passthrough`; as
/// `flm schema --of front-matter` prints it.
fn front_matter_schema() -> Value {
    let std = flm_host::stdpkg::factory(flm_host::stdpkg::StdParts::new(Arc::new(Naming::Folder))).unwrap();
    let math_passthrough = flm_core::math_passthrough::factory().unwrap();
    flm_host::seed::front_matter_schema(&[&std, &math_passthrough], &[])
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
        let mut request = Request::new(Input::File(String::from("doc/paper.flm")));
        request.default_math = flm_host::seed::MathChoice::processor("passthrough");
        request.naming = Naming::Folder;
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
                flm_host::watch::services::NoServices,
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
    /// The entry file at `/` in place of the page, when given: its media type and bytes.
    entry: Option<(String, Vec<u8>)>,
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
    /// The body of each request to open a place, in order.
    opened: Vec<Value>,
    /// The text of the answer `503` to a request to open a place; answered `204` when `None`.
    open_refusal: Option<String>,
}

/// A server of the test's own at a port of this machine: a page, the files beside it, the
/// stream of the preview's messages, which the test writes, and the tools page, served from
/// the files that the program embeds ([`FILES`]).
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
            "/.flm-preview/open" => {
                let mut body = String::new();
                let _ = request.as_reader().read_to_string(&mut body);
                let refusal = {
                    let mut staged = staged.lock().unwrap();
                    staged.opened.push(serde_json::from_str(&body).unwrap_or(Value::Null));
                    staged.open_refusal.clone()
                };
                let _ = match refusal {
                    Some(text) => request.respond(tiny_http::Response::from_string(text).with_status_code(503).with_header(no_store)),
                    None => request.respond(tiny_http::Response::empty(204).with_header(no_store)),
                };
            }
            "/" => {
                let (page, served_by) = {
                    let staged = staged.lock().unwrap();
                    let page = staged.entry.clone().unwrap_or_else(|| (String::from("text/html; charset=utf-8"), staged.page.clone().into_bytes()));
                    (page, staged.served_by.clone())
                };
                let (media, bytes) = page;
                let mut response = tiny_http::Response::from_data(bytes).with_header(no_store).with_header(typed(&media));
                if let Some(value) = served_by {
                    response.add_header(tiny_http::Header::from_bytes("Server-Timing", value).unwrap());
                }
                let _ = request.respond(response);
            }
            "/.flm-preview/" => Stage::serve_tools_file(request, "index.html"),
            other if other.starts_with("/.flm-preview/assets/") => Stage::serve_tools_file(request, &other["/.flm-preview/assets/".len()..]),
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

    /// Answers with the file of the tools page at `path` among [`FILES`], compressed with
    /// gzip as it is embedded, or `404`.
    fn serve_tools_file(request: tiny_http::Request, path: &str) {
        let _ = match FILES.iter().find(|(name, _, _)| *name == path) {
            Some((_, media, bytes)) => request.respond(
                tiny_http::Response::from_data(*bytes)
                    .with_header(tiny_http::Header::from_bytes("Cache-Control", "no-store").unwrap())
                    .with_header(tiny_http::Header::from_bytes("Content-Type", *media).unwrap())
                    .with_header(tiny_http::Header::from_bytes("Content-Encoding", "gzip").unwrap()),
            ),
            None => request.respond(tiny_http::Response::empty(404)),
        };
    }

    fn url(&self) -> String {
        format!("http://{}/", self.address)
    }

    /// The address of the tools page.
    fn tools_url(&self) -> String {
        format!("http://{}/.flm-preview/", self.address)
    }

    /// Serves at `/` the file `bytes` of the media type `media` in place of the page.
    fn serve_entry(&self, media: &str, bytes: Vec<u8>) {
        self.state.lock().unwrap().entry = Some((String::from(media), bytes));
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
}

// The table `FILES` of the files of the tools page, which the crate's build script writes
// and the program embeds: the stage serves each file with its media type and its bytes as
// the preview's server does, the page at `/.flm-preview/` and the others under
// `/.flm-preview/assets/`.
include!(concat!(env!("OUT_DIR"), "/watch_tools.rs"));

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
    format!("<span class=\"flm-pres-math--passthrough flm-math-formula\" data-flm-display=\"inline\">\\({source}\\)</span>")
}

// --- a region brought up to date in place, with nothing of the preview sent ---------------

/// What the page script changed of the window's global names and of the built-in objects,
/// against a fresh frame's window: the names added to the window, and, for each built-in
/// constructor or object and its prototype, each property added and each function that
/// is not the browser's own.
const CHANGED_BUILT_INS: &str = "(() => {
  const frame = document.createElement('iframe'); document.body.appendChild(frame);
  const fresh = frame.contentWindow;
  const beyond = (own, base) => { const known = new Set(Object.getOwnPropertyNames(base));
    return Object.getOwnPropertyNames(own).filter(n => !known.has(n) && !/^\\d+$/.test(n)).sort(); };
  const native = (f) => fresh.Function.prototype.toString.call(f).includes('[native code]');
  const changed = [];
  for (const name of ['Object', 'Function', 'Array', 'String', 'Number', 'Boolean', 'Symbol', 'Promise',
      'Map', 'Set', 'WeakMap', 'RegExp', 'Date', 'Error', 'JSON', 'Math', 'Reflect', 'URL', 'EventSource',
      'EventTarget', 'Node', 'Element', 'HTMLElement', 'Document', 'Window', 'MouseEvent', 'CustomEvent']) {
    const pairs = [[window[name], fresh[name], name]];
    if (typeof window[name] === 'function') pairs.push([window[name].prototype, fresh[name].prototype, name + '.prototype']);
    for (const [own, base, where] of pairs) {
      for (const added of beyond(own, base)) changed.push(where + '.' + added);
      for (const key of Object.getOwnPropertyNames(own)) {
        const d = Object.getOwnPropertyDescriptor(own, key);
        for (const f of [d.value, d.get, d.set]) {
          if (typeof f === 'function' && !native(f)) changed.push(where + '.' + key);
        }
      }
    }
  }
  const result = { window: beyond(window, fresh), changed };
  frame.remove(); return result; })()";

#[test]
fn a_changed_region_is_brought_up_to_date_in_place_and_the_page_changes_no_global_name_or_built_in() {
    let files = Shared::with("doc/paper.flm", &long_document("The last paragraph."));
    let preview = Preview::start(&files, Port::Exactly(0));
    assert_eq!(preview.built(), 1);
    let Some(mut page) = Page::open(&preview.url()) else { return preview.stop() };
    page.wait("document.readyState === 'complete' && document.getElementById('flm-region-content') !== null");
    // The page script defines no global name and changes no built-in object.
    assert_eq!(page.eval(CHANGED_BUILT_INS), json!({ "window": [], "changed": [] }));
    // The HTML the server sent holds no mark.
    let sent = fetch(&preview.url(), "/");
    assert!(sent.contains("Paragraph number 0"));
    assert!(!sent.contains(" data-flm-preview-problem="));
    // The last paragraph in view: an update that changes it keeps the reader's place, and
    // the paragraph flashes.
    page.eval(
        "(() => { window.scrollTo(0, document.documentElement.scrollHeight);
           document.querySelector('#flm-region-content p').flmKept = true; return true; })()",
    );
    page.wait("window.scrollY > 0");
    let bottom = page.eval("window.scrollY");
    let last = |text: &str| format!("Array.from(document.querySelectorAll('#flm-region-content p')).find(p => p.textContent.includes('{text}'))");
    files.write("doc/paper.flm", &long_document("The last paragraph, written again."));
    preview.handle.build_again();
    assert_eq!(preview.built(), 2);
    page.wait(&format!("{} !== undefined", last("written again")));
    assert_eq!(page.eval("document.querySelector('#flm-region-content p').flmKept === true"), json!(true));
    assert_eq!(page.eval("window.scrollY"), bottom);
    // Out of view, it is scrolled to, and it alone flashes.
    page.eval("window.scrollTo(0, 0)");
    page.wait("window.scrollY === 0");
    files.write("doc/paper.flm", &long_document("The last paragraph, written once more."));
    preview.handle.build_again();
    assert_eq!(preview.built(), 3);
    let changed = last("once more");
    page.wait(&format!("{changed} !== undefined && {changed}.hasAttribute('data-flm-preview-flash')"));
    assert_eq!(
        page.eval(&format!("Array.from(document.querySelectorAll('[data-flm-preview-flash]')).every(e => e === {changed})")),
        json!(true),
    );
    page.wait(&format!(
        "(() => {{ const r = {changed}.getBoundingClientRect(); return window.scrollY > 0 && r.top >= 0 && r.bottom <= window.innerHeight; }})()"
    ));
    page.wait(&format!("!{changed}.hasAttribute('data-flm-preview-flash')"));
    // An added paragraph, out of view: it is scrolled to and flashes, with the paragraph
    // before it (its source span changed), and the region's element does not flash.
    page.eval("window.scrollTo(0, 0)");
    page.wait("window.scrollY === 0");
    files.write("doc/paper.flm", &long_document("The last paragraph, written once more.\n\nAn added paragraph."));
    preview.handle.build_again();
    assert_eq!(preview.built(), 4);
    let added = last("An added paragraph");
    page.wait(&format!("{added} !== undefined && {added}.hasAttribute('data-flm-preview-flash')"));
    assert_eq!(
        page.eval(&format!("Array.from(document.querySelectorAll('[data-flm-preview-flash]')).every(e => e === {added} || e === {changed})")),
        json!(true),
    );
    page.wait(&format!(
        "(() => {{ const r = {added}.getBoundingClientRect(); return window.scrollY > 0 && r.top >= 0 && r.bottom <= window.innerHeight; }})()"
    ));
    drop(page);
    preview.stop();
}

#[test]
fn the_page_scrolls_to_changes_only_while_the_viewer_settings_of_the_stream_ask_for_it() {
    let content = |last: &str| {
        let mut text: String = (0..80).map(|index| format!("<p>Paragraph number {index}.</p>")).collect();
        text.push_str(&format!("<p id=\"last\">{last}</p>"));
        text
    };
    let mut first = hello(json!([]));
    first["viewer"] = json!({ "theme": "system", "scroll_to_changes": false });
    let stage = Stage::start(staged_page(&content("The last paragraph."), &[]), first);
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    let last = "document.getElementById('last')";
    page.wait("window.scrollY === 0");
    // Off, as `hello` says: the change out of view is neither scrolled to nor flashed.
    stage.send("update", &content_update(2, &content("Written again."), json!([])));
    page.wait(&format!("{last}.textContent === 'Written again.'"));
    assert_eq!(page.eval(&format!("window.scrollY === 0 && !{last}.hasAttribute('data-flm-preview-flash')")), json!(true));
    // On, as the message `viewer` says: the next change is scrolled to and flashed.
    stage.send("viewer", &json!({ "editors": [], "viewer": { "theme": "system", "scroll_to_changes": true } }));
    stage.send("update", &content_update(3, &content("Written once more."), json!([])));
    page.wait(&format!("{last}.textContent === 'Written once more.' && {last}.hasAttribute('data-flm-preview-flash')"));
    page.wait("window.scrollY > 0");
}

// --- the contract, and the formula renderer's part in it ------------------------------------

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

// --- the page loaded anew, the title set ------------------------------------------------------

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
                 "regions": [{ "id": "flm-region-flm.metadata.title", "name": "flm.metadata.title", "kind": "words", "text": "New" }] }),
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

// --- the marks of the problems -------------------------------------------------------------

#[test]
fn the_errors_and_the_warnings_mark_the_innermost_elements_they_lie_in() {
    let content = "<p data-flm-source-span='[\"doc/paper.flm\",0,15]'>Paragraph 0.</p>\
                   <p id=\"second\" data-flm-source-span='[\"doc/paper.flm\",20,35]'>Paragraph 1.</p>\
                   <p id=\"outer\" data-flm-source-span='[\"doc/paper.flm\",2000,2040]'>Last <em id=\"inner\" \
                   data-flm-source-span='[\"doc/paper.flm\",2010,2020]'>word</em> here.</p>\
                   <p id=\"noted\" data-flm-source-span='[\"doc/paper.flm\",3000,3010]'>Noted.</p>";
    let problem = |severity: &str, start: Option<u64>| match start {
        Some(start) => json!({ "kind": "diagnostic", "severity": severity, "id": "t", "message": "T",
                               "place": { "file": "doc/paper.flm", "offset": start, "line": 1, "column": 1 },
                               "frames": [], "bytes": { "start": start, "end": start + 3 } }),
        None => json!({ "kind": "problem", "severity": severity, "message": "T" }),
    };
    let problems = json!([
        problem("warning", Some(25)),
        problem("error", Some(2012)),
        problem("note", Some(3002)),
        problem("error", None),
    ]);
    let stage = Stage::start(staged_page(content, &[]), hello(problems));
    let Some(mut page) = Page::open(&stage.url()) else { return };
    stage.wait_connections(1);
    let marks = "Array.from(document.querySelectorAll('[data-flm-preview-problem]')).map(e => [e.id, e.getAttribute('data-flm-preview-problem')])";
    page.wait(&format!("{marks}.length === 2"));
    assert_eq!(page.eval(marks), json!([["second", "warning"], ["inner", "error"]]));
    // The stylesheet outlines each mark, dashed, in the color of its severity.
    assert_eq!(
        page.eval(
            "['second', 'inner'].map(id => { const style = getComputedStyle(document.getElementById(id));
               return [style.outlineStyle, style.outlineColor]; })"
        ),
        json!([["dashed", "rgb(138, 90, 0)"], ["dashed", "rgb(179, 38, 30)"]]),
    );
    // An update brings the marks of its build.
    stage.send("update", &content_update(2, content, json!([problem("warning", Some(2002))])));
    page.wait(&format!("JSON.stringify({marks}) === JSON.stringify([['outer', 'warning']])"));
    stage.send("update", &content_update(3, content, json!([])));
    page.wait(&format!("{marks}.length === 0"));
}

// --- the images and the files beside the page -------------------------------------------------

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

// --- a page that connects again ------------------------------------------------------------

#[test]
fn a_page_connects_again_by_itself_and_loads_itself_anew_when_the_document_changed() {
    let files = Shared::with("doc/paper.flm", "---\ntitle: Notes\n---\nFirst text.\n");
    let preview = Preview::start(&files, Port::Exactly(0));
    preview.built();
    let port = preview.server.address().port();
    let Some(mut page) = Page::open(&preview.url()) else { return preview.stop() };
    page.wait("document.readyState === 'complete' && document.body.textContent.includes('First text.')");
    page.eval("window.flmKept = true");
    // The server is stopped; the document changes meanwhile; the server starts again at the
    // same port.
    preview.stop();
    files.write("doc/paper.flm", "---\ntitle: Notes\n---\nSecond text.\n");
    let preview = Preview::start(&files, Port::Exactly(port));
    preview.built();
    page.wait("window.flmKept === undefined && document.body.textContent.includes('Second text.')");
    page.wait("document.readyState === 'complete'");
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

// --- the waiting page --------------------------------------------------------------------------

#[test]
fn the_waiting_page_is_replaced_by_the_document_once_built_and_the_tools_page_names_the_document_until_then() {
    let files = Shared::with("doc/paper.flm", "---\ntitle: [unclosed\n---\nText.\n");
    let preview = Preview::start(&files, Port::Exactly(0));
    preview.built();
    let Some(mut page) = Page::open(&preview.url()) else { return preview.stop() };
    page.wait("document.documentElement.hasAttribute('data-flm-preview-waiting')");
    assert_eq!(page.eval("document.body.textContent.includes('doc/paper.flm')"), json!(true));
    // Before any output, the tools page shows the document's name, and no frame.
    let Some(mut tools) = Page::open(&preview.server.tools_url()) else { return preview.stop() };
    tools.wait("document.querySelector('.flm-output code')?.textContent === 'doc/paper.flm'");
    assert_eq!(tools.eval("document.querySelector('iframe') === null"), json!(true));
    files.write("doc/paper.flm", "---\ntitle: Notes\n---\nThe mended text.\n");
    preview.handle.build_again();
    preview.built();
    page.wait("!document.documentElement.hasAttribute('data-flm-preview-waiting') && document.body.textContent.includes('The mended text.')");
    tools.wait("document.querySelector('iframe')?.contentDocument?.body?.textContent.includes('The mended text.') === true");
    drop(page);
    drop(tools);
    preview.stop();
}

// --- the tools page ----------------------------------------------------------------------------

/// The text of the tools page's build state.
const BUILD_STATE: &str = "document.querySelector('.flm-status .flm-build-state').textContent";

/// The counts of the status bar's problems, each with its word, joined by commas.
const COUNTS: &str = "Array.from(document.querySelectorAll('.flm-problems-button .flm-count'), (count) => count.textContent).join(', ')";

#[test]
fn the_tools_page_shows_the_document_and_follows_a_build() {
    let files = Shared::with("doc/paper.flm", "---\ntitle: Notes\n---\nFirst text.\n");
    let preview = Preview::start(&files, Port::Exactly(0));
    preview.built();
    let Some(mut tools) = Page::open(&preview.server.tools_url()) else { return preview.stop() };
    let frame_text = "(document.querySelector('iframe')?.contentDocument?.body?.textContent ?? '')";
    tools.wait(&format!("{frame_text}.includes('First text.') && {BUILD_STATE}.startsWith('Built in ')"));
    // The preview's formula renderer, `math-passthrough`, warns that it checks no formula.
    assert_eq!(tools.eval(COUNTS), json!("1 warning"));

    // A build is followed in the frame, in the status bar and in the counts.
    files.write("doc/paper.flm", "---\ntitle: Notes\n---\nSecond text, \\nosuchcommand.\n");
    preview.handle.build_again();
    preview.built();
    tools.wait(&format!("{frame_text}.includes('Second text') && {BUILD_STATE}.startsWith('Built in ')"));
    assert_eq!(tools.eval(COUNTS), json!("1 error, 1 warning"));

    // A click on the problems opens the panel on Problems, from a closed panel and from another
    // tab, and moves the focus into the tab.
    let on_problems = "document.querySelector('.flm-tab-open')?.title === 'Problems' \
                       && document.querySelector('.flm-panel-content').contains(document.activeElement)";
    tools.eval("document.querySelector('.flm-problems-button').click()");
    tools.wait(on_problems);
    tools.eval("document.querySelector('.flm-tab[title=\"Outline\"]').click()");
    tools.wait("document.querySelector('.flm-tab-open')?.title === 'Outline'");
    tools.eval("document.querySelector('.flm-problems-button').click()");
    tools.wait(on_problems);

    // A build without output keeps the output of the one before, and says so.
    files.write("doc/paper.flm", "---\ntitle: [unclosed\n---\nThird text.\n");
    preview.handle.build_again();
    preview.built();
    tools.wait(&format!("{BUILD_STATE} === 'Last successful build'"));
    assert_eq!(tools.eval(&format!("{frame_text}.includes('Second text')")), json!(true));

    // The server stops: the tools page says it is disconnected, and Build again is disabled.
    preview.stop();
    tools.wait(&format!("{BUILD_STATE} === 'Disconnected'"));
    assert_eq!(tools.eval("document.querySelector('[aria-label=\"Build again\"]').disabled"), json!(true));
    drop(tools);
}

#[test]
fn inspect_takes_the_click_and_its_card_shows_the_constructs_of_the_element_until_escape() {
    let files = Shared::with("doc/paper.flm", "---\ntitle: Notes\n---\nSome \\textbf{bold \\emph{word}} here.\n");
    let preview = Preview::start(&files, Port::Exactly(0));
    preview.built();
    let Some(mut tools) = Page::open(&preview.server.tools_url()) else { return preview.stop() };
    let doc = "document.querySelector('iframe').contentDocument";
    tools.wait(&format!("({doc}?.body?.textContent ?? '').includes('word') && {BUILD_STATE}.startsWith('Built in ')"));
    tools.eval(&format!("(() => {{ window.clicks = 0; {doc}.addEventListener('click', () => {{ window.clicks += 1; }}); return true; }})()"));
    tools.eval("document.querySelector('.flm-inspect').click()");
    tools.wait("document.querySelector('.flm-inspect').getAttribute('aria-pressed') === 'true'");

    // The click on the emphasized word reaches the page script alone; the card names the
    // innermost construct, then the one around it.
    let card = "document.querySelector('.flm-inspect-card')";
    let name = format!("{card}?.querySelector('.flm-inspect-name')?.textContent");
    // The page script may receive the message that turns Inspect on after a first click: the
    // click is repeated until the page script outlines an element. It is not repeated after
    // that, since each click is a new selection, which drops the answer awaited for the last.
    wait_until("the word selected", || {
        gesture_in(&mut tools, doc, "em", "{}");
        (tools.eval(&format!("{doc}.querySelector('[data-flm-preview-inspected]') !== null")) == json!(true))
            .then_some(())
    });
    tools.wait(&format!("{name} === '\\\\emph'"));
    tools.eval("window.clicks = 0");
    gesture_in(&mut tools, doc, "em", "{}");
    tools.wait(&format!("{name} === '\\\\emph'"));
    assert_eq!(tools.eval("window.clicks"), json!(0));
    assert_eq!(
        tools.eval(&format!("Array.from({card}.querySelectorAll('.flm-inspect-outer'), (each) => each.textContent)")),
        json!(["\\textbf"]),
    );
    assert_eq!(tools.eval(&format!("{doc}.querySelectorAll('[data-flm-preview-inspected]').length")), json!(1));

    // Escape closes the card, which holds the focus, and the element is no longer selected.
    assert_eq!(tools.eval(&format!("document.activeElement === {card}")), json!(true));
    tools.eval("document.activeElement.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))");
    tools.wait(&format!("{card} === null && {doc}.querySelectorAll('[data-flm-preview-inspected]').length === 0"));
    drop(tools);
    preview.stop();
}

#[test]
fn a_change_of_a_metadata_field_writes_the_file_and_the_row_shows_the_green_check() {
    let files = Shared::with("doc/paper.flm", "---\ntitle: Notes\nauthor: A. Author\n---\nFirst text.\n");
    let preview = Preview::start(&files, Port::Exactly(0));
    preview.built();
    let Some(mut tools) = Page::open(&preview.server.tools_url()) else { return preview.stop() };
    tools.wait("document.querySelector('.flm-panel-button') !== null");
    tools.eval("document.querySelector('.flm-panel-button').click()");
    tools.wait("document.querySelector('.flm-tab[title=\"Metadata\"]') !== null");
    tools.eval("document.querySelector('.flm-tab[title=\"Metadata\"]').click()");
    let title = "document.querySelector('.flm-metadata .flm-text-field')";
    tools.wait(&format!("{title}?.value === 'Notes' && !{title}.disabled"));

    // The field writes its text when the author presses Enter, and the row shows the check.
    tools.eval(&format!(
        "(() => {{ const field = {title}; field.focus(); field.value = 'Better notes'; \
         field.dispatchEvent(new Event('input', {{ bubbles: true }})); \
         field.dispatchEvent(new KeyboardEvent('keydown', {{ key: 'Enter', bubbles: true }})); }})()"
    ));
    tools.wait("document.querySelector('.flm-metadata .flm-check-slot [aria-label=\"Saved\"]') !== null");
    let text = String::from_utf8(files.read("doc/paper.flm").unwrap()).unwrap();
    assert_eq!(text, "---\ntitle: Better notes\nauthor: A. Author\n---\nFirst text.\n");
    drop(tools);
    preview.stop();
}

#[test]
fn the_build_type_chosen_in_the_tools_page_shows_its_text_output_which_follows_a_build_and_keeps_its_scroll() {
    let files = Shared::with("doc/paper.flm", &long_document("First end."));
    let preview = Preview::start(&files, Port::Exactly(0));
    preview.built();
    let Some(mut tools) = Page::open(&preview.server.tools_url()) else { return preview.stop() };
    let select = "document.querySelector('select[aria-label=\"Build type shown\"]')";
    tools.wait(&format!("{select}?.value === 'html' && document.querySelector('iframe') !== null"));

    // Choosing `text` builds the text output, which the text view shows in place of the frame.
    tools.eval(&format!(
        "(() => {{ const select = {select}; select.value = 'text'; select.dispatchEvent(new Event('change')); }})()"
    ));
    preview.built();
    let text = "(document.querySelector('.flm-text')?.textContent ?? '')";
    tools.wait(&format!("{select}.value === 'text' && {text}.includes('First end.')"));
    assert_eq!(tools.eval("document.querySelector('iframe') === null"), json!(true));

    // A build reads the text again, and the view keeps its scroll position.
    let scroll = "document.querySelector('.flm-text').scrollTop";
    tools.eval(&format!("{scroll} = 200"));
    tools.wait(&format!("{scroll} === 200"));
    files.write("doc/paper.flm", &long_document("Second end."));
    preview.handle.build_again();
    preview.built();
    tools.wait(&format!("{text}.includes('Second end.')"));
    assert_eq!(tools.eval(scroll), json!(200));
    drop(tools);
    preview.stop();
}

/// A PDF file with one US Letter page for each of `texts`, each page showing its text in
/// Helvetica, which the file names without holding. A text holds no parenthesis and no
/// backslash.
fn pdf(texts: &[&str]) -> Vec<u8> {
    // Objects 1 to 3: the catalog, the page tree, the font; then a page and its content for
    // each text.
    let kids: Vec<String> = (0..texts.len()).map(|index| format!("{} 0 R", 4 + 2 * index)).collect();
    let mut objects = vec![
        String::from("<< /Type /Catalog /Pages 2 0 R >>"),
        format!("<< /Type /Pages /Kids [{}] /Count {} >>", kids.join(" "), texts.len()),
        String::from("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
    ];
    for (index, text) in texts.iter().enumerate() {
        objects.push(format!(
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Resources << /Font << /F1 3 0 R >> >> /Contents {} 0 R >>",
            5 + 2 * index
        ));
        let content = format!("BT /F1 24 Tf 72 700 Td ({text}) Tj ET");
        objects.push(format!("<< /Length {} >>\nstream\n{content}\nendstream", content.len()));
    }
    let mut file = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        offsets.push(file.len());
        file.extend(format!("{} 0 obj\n{object}\nendobj\n", index + 1).bytes());
    }
    let table = file.len();
    file.extend(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).bytes());
    for offset in offsets {
        file.extend(format!("{offset:010} 00000 n \n").bytes());
    }
    file.extend(format!("trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{table}\n%%EOF\n", objects.len() + 1).bytes());
    file
}

/// An update of the stage that names a changed PDF file at `/`.
fn pdf_update(build: u64) -> Value {
    json!({
        "build": build, "page": "reload", "regions": [], "files": [], "problems": [], "references": null,
        "media_type": "application/pdf", "earlier_output": false, "duration_ms": 1200,
        "profile": "pdf-latex", "profiles": ["html", "pdf-latex"],
    })
}

#[test]
fn the_pdf_view_loads_pdf_js_for_a_pdf_alone_and_keeps_the_page_read_and_the_zoom_across_files() {
    let mut first = hello(json!([]));
    first["media_type"] = json!("text/html; charset=utf-8");
    let stage = Stage::start(staged_page("<p>Text of the page.</p>", &[]), first);
    stage.serve_build(1);
    let Some(mut tools) = Page::open(&stage.tools_url()) else { return };
    let frame_text = "(document.querySelector('iframe')?.contentDocument?.body?.textContent ?? '')";
    tools.wait(&format!("{frame_text}.includes('Text of the page.')"));
    // An HTML page loads nothing of pdf.js.
    let library = "/.flm-preview/assets/pdfjs/pdf.min.mjs";
    assert_eq!(stage.asked(library), 0);

    // A PDF file: its pages one under the other, the page read, the number of pages, the zoom.
    stage.serve_entry("application/pdf", pdf(&["First one.", "First two.", "First three."]));
    stage.send("update", &pdf_update(2));
    let head = "(document.querySelector('.flm-pdf-head')?.textContent ?? '')";
    let page_text = |page: u32| format!("(document.querySelector('.flm-pdf-page[data-page=\"{page}\"] .textLayer')?.textContent ?? '')");
    tools.wait(&format!("document.querySelectorAll('.flm-pdf-page').length === 3 && {}.includes('First one.')", page_text(1)));
    tools.wait(&format!("{head}.includes('Page 1 of 3') && {head}.includes('100%')"));
    assert_eq!(tools.eval("document.querySelector('iframe') === null"), json!(true));
    assert!(stage.asked(library) >= 1);
    assert!(stage.asked("/.flm-preview/assets/pdfjs/pdf.worker.min.mjs") >= 1);

    // The text of a page can be selected.
    let selected = "(() => { const layer = document.querySelector('.flm-pdf-page[data-page=\"1\"] .textLayer'); \
        getSelection().selectAllChildren(layer); return getSelection().toString(); })()";
    assert!(tools.eval(selected).as_str().unwrap_or_default().contains("First one."));

    // The zoom, then the second page read.
    tools.eval("document.querySelector('[aria-label=\"Zoom in\"]').click()");
    tools.wait(&format!("{head}.includes('110%')"));
    let view = "document.querySelector('.flm-pdf-view')";
    let second_top = "document.querySelector('.flm-pdf-page[data-page=\"2\"]').offsetTop";
    tools.eval(&format!("{view}.scrollTop = {second_top} + 40"));
    tools.wait(&format!("{head}.includes('Page 2 of 3') && {}.includes('First two.')", page_text(2)));
    let scrolled = tools.eval(&format!("{view}.scrollTop"));

    // A changed file: the page read and the zoom stay.
    stage.serve_entry("application/pdf", pdf(&["Second one.", "Second two.", "Second three."]));
    stage.send("update", &pdf_update(3));
    tools.wait(&format!("{}.includes('Second two.')", page_text(2)));
    tools.wait(&format!("{head}.includes('Page 2 of 3') && {head}.includes('110%')"));
    assert_eq!(tools.eval(&format!("Math.abs({view}.scrollTop - {scrolled}) < 1")), json!(true));
    assert_eq!(tools.eval(&format!("Math.round({view}.scrollTop - {second_top})")), json!(40));
    drop(tools);
}

// --- messages that are not understood ------------------------------------------------------

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
}

// --- the gesture that opens a place in the editor -------------------------------------------

/// Makes a mouse button go down, then the click, on the element `selector` finds in the
/// document `document` (a JavaScript expression), with the modifier keys `keys`
/// (`ctrlKey`, `metaKey`, `shiftKey` as a JavaScript object's fields).
fn gesture_in(page: &mut Page, document: &str, selector: &str, keys: &str) {
    page.eval(&format!(
        "(() => {{ const doc = {document}; const target = doc.querySelector('{selector}'); const keys = {keys};
           for (const type of ['mousedown', 'mouseup', 'click']) {{
             target.dispatchEvent(new doc.defaultView.MouseEvent(type, Object.assign({{ bubbles: true, cancelable: true, button: 0 }}, keys)));
           }} return true; }})()"
    ));
}

/// [`gesture_in`] the page's own document.
#[cfg(unix)]
fn gesture(page: &mut Page, selector: &str, keys: &str) {
    gesture_in(page, "document", selector, keys);
}

#[cfg(unix)]
#[test]
fn a_gesture_sends_the_place_and_the_editor_opens_it() {
    use editor::{fake_editor, folder, recorded};
    let folder = folder("page_gestures");
    let record = folder.join("record");
    fake_editor(&folder, "code", &record);
    let editors = Editors::find(None, Some(folder.as_os_str()), &[]).unwrap();
    let files = Shared::with("doc/paper.flm", "---\ntitle: Notes\n---\nÇa été \\emph{ici}.\n\nThe end.\n");
    let preview = Preview::start_with(&files, Port::Exactly(0), editors);
    assert_eq!(preview.built(), 1);
    let Some(mut page) = Page::open(&preview.url()) else { return preview.stop() };
    page.wait("document.readyState === 'complete' && document.querySelector('em') !== null");
    let file = std::path::absolute("doc/paper.flm").unwrap().display().to_string();
    let opened = Opening::Opened { editor: String::from("code"), file: String::from("doc/paper.flm"), line: 4, column: 8 };
    // Cmd and Shift on the emphasized word: its place, line 4 below the front matter of
    // three lines, column 8 after seven characters of which two take two bytes; it flashes.
    gesture(&mut page, "em", "{ metaKey: true, shiftKey: true }");
    page.wait("document.querySelector('em').hasAttribute('data-flm-preview-flash')");
    assert_eq!(recorded(&record), ["--goto", &format!("{file}:4:8")]);
    assert_eq!(preview.opening(), opened);
    std::fs::remove_file(&record).unwrap();
    // Ctrl and Shift on a word with no place of its own: the place of the element around it.
    page.eval("(() => { const b = document.createElement('b'); b.id = 'placeless'; b.textContent = 'x'; document.querySelector('em').appendChild(b); return true; })()");
    gesture(&mut page, "#placeless", "{ ctrlKey: true, shiftKey: true }");
    assert_eq!(recorded(&record), ["--goto", &format!("{file}:4:8")]);
    assert_eq!(preview.opening(), opened);
    drop(page);
    preview.stop();
}

// --- the messages with the tools page ------------------------------------------------------------

/// A page that shows the stage's page at `/` in a frame, as the tools page does, and
/// records each message from the frame's window (`window.received`); `window.toPage`
/// sends a message to the page script.
const FRAMING_PAGE: &str = "<!DOCTYPE html>\n<html><body style=\"margin: 0\">\
  <iframe src=\"/\" style=\"border: 0; width: 800px; height: 500px\"></iframe>\n<script>\n\
  window.received = [];\n\
  const frame = document.querySelector('iframe');\n\
  window.addEventListener('message', (e) => { if (e.source === frame.contentWindow) window.received.push(e.data); });\n\
  window.toPage = (message) => frame.contentWindow.postMessage(Object.assign({ channel: 'flm-preview' }, message), location.origin);\n\
  </script></body></html>\n";

#[test]
fn the_messages_with_the_tools_page_are_accepted_from_its_window_alone() {
    let mut content = String::from(
        "<p id=\"target\" data-flm-source-span='[\"doc/paper.flm\",10,30]'>The <em id=\"inside\">target</em>.</p>\
         <p id=\"refused\">Refused.</p><p id=\"plain\">Plain.</p>",
    );
    for index in 0..60 {
        content.push_str(&format!("<p data-flm-source-span='[\"doc/paper.flm\",{},{}]'>Paragraph {index}.</p>", 100 + index * 20, 115 + index * 20));
    }
    content.push_str("<p id=\"far\" data-flm-source-span='[\"doc/paper.flm\",5000,5010]'>Far.</p>");
    let clicks = "window.clicks = 0; document.addEventListener('click', () => { window.clicks += 1; });";
    let mut first = hello(json!([]));
    first["references"] = json!({ "target": "thm:main" });
    let stage = Stage::start(staged_page(&content, &[clicks]), first);
    {
        let mut staged = stage.state.lock().unwrap();
        staged.files.insert(String::from("frame.html"), (String::from("text/html"), FRAMING_PAGE.as_bytes().to_vec()));
        staged.open_refusal = Some(String::from("No editor found"));
    }
    let Some(mut page) = Page::open(&format!("{}frame.html", stage.url())) else { return };
    stage.wait_connections(1);
    let frame = "document.querySelector('iframe').contentWindow";
    let doc = format!("{frame}.document");
    page.wait(&format!("{doc}.readyState === 'complete' && {doc}.getElementById('far') !== null"));
    // Before `attach`, the page script sends nothing: neither a scroll nor a refused opening,
    // whose reason it writes to the console (recorded here). Ctrl alone asks nothing.
    page.eval(&format!("(() => {{ const w = {frame}; w.logged = []; w.console.error = (...parts) => w.logged.push(parts.join(' ')); return true; }})()"));
    page.eval(&format!("{frame}.scrollTo(0, 200)"));
    gesture_in(&mut page, &doc, "#inside", "{ ctrlKey: true }");
    gesture_in(&mut page, &doc, "#inside", "{ ctrlKey: true, shiftKey: true }");
    assert_eq!(stage.opened(1), json!({ "file": "doc/paper.flm", "start": 10 }));
    page.wait(&format!("{frame}.logged.length === 1"));
    assert_eq!(page.eval(&format!("{frame}.logged")), json!(["No editor found"]));
    page.eval("window.toPage({ type: 'attach' })");
    page.wait("window.received.length > 0");
    assert_eq!(page.eval("window.received"), json!([{ "channel": "flm-preview", "type": "loaded", "waiting": false }]));
    // A scroll, and a refused opening.
    page.eval(&format!("{frame}.scrollTo(0, 400)"));
    page.wait("window.received.some(m => m.type === 'scrolled' && m.y === 400)");
    gesture_in(&mut page, &doc, "#inside", "{ metaKey: true, shiftKey: true }");
    page.wait("window.received.some(m => m.type === 'open-failed')");
    assert_eq!(page.eval("window.received.find(m => m.type === 'open-failed').text"), json!("No editor found"));
    assert_eq!(stage.state.lock().unwrap().opened.len(), 2);
    // An element shown by its id, the elements of a place: scrolled to, and flashed.
    let flashing = |id: &str| format!("{doc}.getElementById('{id}').hasAttribute('data-flm-preview-flash')");
    page.eval("window.toPage({ type: 'show-element', id: 'far' })");
    page.wait(&format!(
        "{} && (() => {{ const r = {doc}.getElementById('far').getBoundingClientRect(); return r.top >= 0 && r.bottom <= {frame}.innerHeight; }})()",
        flashing("far")
    ));
    page.eval("window.toPage({ type: 'show-place', place: { file: 'doc/paper.flm', start: 12, end: 14 } })");
    page.wait(&flashing("target"));
    // A message from the document's own window, or from another window of the server's
    // origin, is refused; the one from the tools page's window after them is accepted.
    page.eval(&format!(
        "(() => {{ {frame}.eval(\"window.postMessage({{ channel: 'flm-preview', type: 'show-element', id: 'refused' }}, '*')\");
           const other = document.createElement('iframe'); document.body.appendChild(other);
           other.contentWindow.eval(\"parent.document.querySelector('iframe').contentWindow.postMessage({{ channel: 'flm-preview', type: 'show-element', id: 'refused' }}, '*')\");
           window.toPage({{ type: 'show-element', id: 'plain' }}); return true; }})()"
    ));
    page.wait(&flashing("plain"));
    assert_eq!(page.eval(&flashing("refused")), json!(false));
    // Inspect takes the clicks: the innermost element with a place is selected and
    // reported, with its rectangle and its label, and the document sees no click. (The
    // answer to `attach` tells that the messages before it were applied.)
    let applied = |page: &mut Page| {
        page.eval("window.received = []; window.toPage({ type: 'attach' })");
        page.wait("window.received.some(m => m.type === 'loaded')");
        page.eval("window.received = []");
    };
    page.eval("window.toPage({ type: 'inspect', on: true })");
    applied(&mut page);
    page.eval(&format!("{frame}.clicks = 0"));
    gesture_in(&mut page, &doc, "#inside", "{}");
    page.wait("window.received.some(m => m.type === 'inspected')");
    let inspected = page.eval("window.received.find(m => m.type === 'inspected')");
    assert_eq!(inspected["place"], json!({ "file": "doc/paper.flm", "start": 10, "end": 30 }));
    assert_eq!(inspected["label"], json!("thm:main"));
    assert!(inspected["rect"]["width"].as_f64().unwrap() > 0.0, "{inspected}");
    let selected = format!("{doc}.querySelectorAll('[data-flm-preview-inspected]')");
    assert_eq!(page.eval(&format!("Array.from({selected}).map(e => e.id)")), json!(["target"]));
    assert_eq!(page.eval(&format!("{frame}.clicks")), json!(0));
    // A scroll made just before a click is not reported after the selection, whose
    // rectangle it does not put out of date; a scroll after the selection is.
    page.eval("window.received = []");
    page.eval(&format!(
        "(() => {{ const w = {frame}; w.scrollTo(0, w.scrollY + 50);
           w.document.getElementById('inside').dispatchEvent(new w.MouseEvent('click', {{ bubbles: true, cancelable: true, button: 0 }}));
           w.requestAnimationFrame(() => w.requestAnimationFrame(() => {{ w.framesDrawn = true; }})); return true; }})()"
    ));
    page.wait(&format!("{frame}.framesDrawn === true && window.received.some(m => m.type === 'inspected')"));
    assert_eq!(page.eval("window.received.map(m => m.type)"), json!(["inspected"]));
    page.eval(&format!("{frame}.scrollBy(0, 50)"));
    page.wait("window.received.some(m => m.type === 'scrolled')");
    // A click where no element has a place selects nothing.
    page.eval("window.received = []");
    gesture_in(&mut page, &doc, "#plain", "{}");
    page.wait("window.received.some(m => m.type === 'inspected')");
    assert_eq!(
        page.eval("window.received.find(m => m.type === 'inspected')"),
        json!({ "channel": "flm-preview", "type": "inspected", "place": null, "rect": null, "label": null }),
    );
    assert_eq!(page.eval(&format!("{selected}.length")), json!(0));
    // Inspect off: nothing stays selected, and the document has its clicks.
    gesture_in(&mut page, &doc, "#inside", "{}");
    page.wait(&format!("{selected}.length === 1"));
    page.eval("window.toPage({ type: 'inspect', on: false })");
    applied(&mut page);
    assert_eq!(page.eval(&format!("{selected}.length")), json!(0));
    gesture_in(&mut page, &doc, "#inside", "{}");
    assert_eq!(page.eval(&format!("{frame}.clicks")), json!(1));
}

// --- the math-mathjax extension: a page in each of its two modes ---------------------------------

/// The page of the document `text`, rendered as HTML with the workspace's folder of
/// extensions in JavaScript `extensions/js/` on the search path, read from the disk (the document is written
/// to the file `name` in a folder of the tests' own).
#[cfg(feature = "js")]
fn math_mathjax_page(name: &str, text: &str) -> String {
    use flm_host::output::{MemorySink, Sink};
    let folder = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("math-mathjax-browser");
    std::fs::create_dir_all(&folder).unwrap();
    let path = folder.join(name);
    std::fs::write(&path, text).unwrap();
    let flm = |path: &Path| flm_host::files::system::from_system(path).unwrap();
    let mut request = Request::new(Input::File(flm(&path)));
    request.format = Some(String::from("html"));
    let extensions = flm(Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../extensions/js")));
    request.extension_folders = vec![flm_host::packages::ExtensionFolder { path: extensions, required: true }];
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    let prepared = flm_host::session::prepare(request, Arc::new(flm_host::files::DiskFiles), &[], &Scope::none())
        .unwrap();
    let built = flm_host::session::build(&prepared, &Scope::none()).unwrap();
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
