//! The preview's server (`watch::server`): the loop driven over files held in memory, each
//! of its events handed to the server, and the server read through plain connections of
//! this machine. Every wait has an upper bound and ends as soon as what is expected
//! arrives; the system chooses every port.

#[cfg(unix)]
#[path = "support/editor.rs"]
mod editor;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use flm_main::files::{FileError, Files, MemoryFiles};
use flm_main::output::{Naming, OutputError};
use flm_main::session::{Input, Request};
use flm_main::watch::editor::Editors;
use flm_main::watch::server::{EVENTS, OPEN, Port, REBUILD, SETTINGS, ServeError, Server};
use flm_main::watch::settings::DocumentSettings;
use flm_main::watch::update::BuildOutput;
use flm_main::watch::{Event, Handle, Watch};
use serde_json::{Value, json};

/// The upper bound of every wait.
const BOUND: Duration = Duration::from_secs(30);

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
}

/// The settings of `doc/paper.flm` over `files`, a save written into them, the forms
/// made from `schema`.
fn settings(files: &Shared, schema: Value) -> DocumentSettings {
    let written = files.clone();
    DocumentSettings::new(schema, "doc/paper.flm", Arc::new(files.clone()), move |path, bytes| {
        written.0.lock().unwrap().insert(path, bytes);
        Ok(())
    })
}

impl Files for Shared {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FileError> {
        self.0.lock().unwrap().read(path)
    }

    fn entries(&self, path: &Path) -> Result<Vec<String>, FileError> {
        self.0.lock().unwrap().entries(path)
    }
}

/// The loop and the server, the loop on a thread of its own handing each event to the
/// server and then to the test.
struct Preview {
    server: Arc<Server>,
    handle: Handle,
    built: Receiver<(u64, Option<Arc<BuildOutput>>)>,
    ended: Receiver<()>,
}

impl Preview {
    /// The preview of `request` over `files`, at a port the system chooses.
    fn start(request: Request, files: &Shared) -> Preview {
        Preview::start_with(request, files, settings(files, schema()))
    }

    /// The preview of `request` over `files`, the settings read and saved by `settings`.
    fn start_with(request: Request, files: &Shared, settings: DocumentSettings) -> Preview {
        Preview::serve(request, files, settings, Editors::none())
    }

    /// The preview of `request` over `files`, a place opened in one of `editors`.
    #[cfg(unix)]
    fn start_with_editors(request: Request, files: &Shared, editors: Editors) -> Preview {
        Preview::serve(request, files, settings(files, schema()), editors)
    }

    /// The preview of `request` over `files`, the settings read and saved by `settings`, a
    /// place opened in one of `editors`.
    fn serve(request: Request, files: &Shared, settings: DocumentSettings, editors: Editors) -> Preview {
        let watch = Watch::new(request, Arc::new(files.clone()), Vec::new());
        let handle = watch.handle();
        let server = Arc::new(Server::start(Port::Exactly(0), "doc/paper.flm", handle.clone(), editors, |_| {}, settings)
                .expect("the server starts"));
        let (send, built) = channel();
        let (done, ended) = channel();
        let serving = Arc::clone(&server);
        thread::spawn(move || {
            watch
                .run(|event| {
                    serving.tell(&event);
                    if let Event::Built(built) = event {
                        let _ = send.send((built.update.build, built.output.clone()));
                    }
                })
                .expect("the loop ran");
            let _ = done.send(());
        });
        Preview { server, handle, built, ended }
    }

    /// The next build done: its number and the output now current.
    fn built(&self) -> (u64, Option<Arc<BuildOutput>>) {
        self.built.recv_timeout(BOUND).expect("a build within the bound")
    }

    fn address(&self) -> SocketAddr {
        self.server.address()
    }

    fn stop(self) {
        self.handle.stop();
        self.ended.recv_timeout(BOUND).expect("the loop ends within the bound");
    }
}

/// The schema the server is started with, standing for the schema of a front matter; the
/// first build that prepares the document replaces it ([`prepared_schema`]).
fn schema() -> Value {
    json!({ "description": "the schema of a front matter" })
}

/// The schema of a front matter for the packages that the preparation of `request()` over
/// `files` uses: the one the server answers once a build prepared the document.
fn prepared_schema(files: &Shared) -> Value {
    let prepared = flm_main::session::prepare(request(), Arc::new(files.clone()), &[]).expect("prepared");
    flm_main::seed::front_matter_schema(&prepared.factories().iter().collect::<Vec<_>>(), prepared.suggestions())
}

/// The request for `doc/paper.flm`, its output named in a folder (`index.html`, …).
fn request() -> Request {
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.naming = Naming::Folder;
    request
}

const DOC: &str = "---\ntitle: Notes\n---\nHello \\emph{world}.\n\nA second paragraph.\n";

/// An answer read off a connection: its status, its headers (names in lower case) and its
/// body.
struct Answer {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Answer {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
    }
}

/// Sends `method target` to `address` over a new connection, and reads the answer.
fn ask(address: SocketAddr, method: &str, target: &str) -> Answer {
    send(address, method, target, &[], b"")
}

/// Sends `method target` with the headers `headers` and the body `body` to `address` over
/// a new connection, and reads the answer.
fn send(address: SocketAddr, method: &str, target: &str, headers: &[(&str, &str)], body: &[u8]) -> Answer {
    let mut connection = TcpStream::connect(address).expect("the server is reached");
    connection.set_read_timeout(Some(BOUND)).unwrap();
    let length = if method == "POST" { format!("Content-Length: {}\r\n", body.len()) } else { String::new() };
    let headers: String = headers.iter().map(|(name, value)| format!("{name}: {value}\r\n")).collect();
    write!(connection, "{method} {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n{length}{headers}\r\n").unwrap();
    connection.write_all(body).unwrap();
    let mut bytes = Vec::new();
    connection.read_to_end(&mut bytes).expect("the answer within the bound");
    let split = bytes.windows(4).position(|window| window == b"\r\n\r\n").expect("a head");
    let head = String::from_utf8(bytes[..split].to_vec()).unwrap();
    let mut lines = head.lines();
    let status = lines.next().unwrap().split(' ').nth(1).unwrap().parse().unwrap();
    let headers = lines
        .filter_map(|line| line.split_once(':'))
        .map(|(name, value)| (name.trim().to_ascii_lowercase(), value.trim().to_owned()))
        .collect();
    Answer { status, headers, body: bytes[split + 4..].to_vec() }
}

fn get(address: SocketAddr, target: &str) -> Answer {
    ask(address, "GET", target)
}

/// A client of the stream of messages.
struct Client {
    lines: BufReader<TcpStream>,
    head: Vec<String>,
}

impl Client {
    fn connect(address: SocketAddr) -> Client {
        let mut connection = TcpStream::connect(address).expect("the server is reached");
        connection.set_read_timeout(Some(BOUND)).unwrap();
        write!(connection, "GET {EVENTS} HTTP/1.1\r\nHost: localhost\r\nAccept: text/event-stream\r\n\r\n").unwrap();
        let mut lines = BufReader::new(connection);
        let mut head = Vec::new();
        loop {
            let mut line = String::new();
            lines.read_line(&mut line).expect("the head within the bound");
            if line.trim_end().is_empty() {
                break;
            }
            head.push(line.trim_end().to_ascii_lowercase());
        }
        Client { lines, head }
    }

    /// The next message: its event's name and its data read as JSON.
    fn next(&mut self) -> (String, Value) {
        let (mut event, mut data) = (String::new(), String::new());
        loop {
            let mut line = String::new();
            let read = self.lines.read_line(&mut line).expect("a message within the bound");
            assert!(read > 0, "the stream ended");
            let line = line.trim_end();
            if line.is_empty() {
                if !event.is_empty() {
                    return (event, serde_json::from_str(&data).expect("JSON"));
                }
            } else if let Some(name) = line.strip_prefix("event: ") {
                event = name.to_owned();
            } else if let Some(text) = line.strip_prefix("data: ") {
                data = text.to_owned();
            }
        }
    }

    /// The next message named `name`, passing over the others.
    fn next_named(&mut self, name: &str) -> Value {
        loop {
            let (event, data) = self.next();
            if event == name {
                return data;
            }
        }
    }
}

// --- acceptance 1 and 2: what is served, and that nothing is kept ------------------------------

#[test]
fn the_entry_is_served_at_the_root_and_at_its_path_as_built_and_nothing_else() {
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start(request(), &files);
    let (build, output) = preview.built();
    let output = output.expect("an output");
    let entry = output.entry().expect("an entry");
    assert_eq!(entry.path, "index.html");
    // The answer names the server and the build it comes from.
    let served_by = get(preview.address(), "/").header("server-timing").map(str::to_owned).expect("the server and the build");
    assert!(served_by.starts_with("flm-server;desc=\""), "{served_by}");
    assert!(served_by.ends_with(&format!(", flm-build;desc=\"{build}\"")), "{served_by}");
    for target in ["/", "/index.html", "/index.html?v=2", "/%69ndex.html"] {
        let answer = get(preview.address(), target);
        assert_eq!(answer.status, 200, "{target}");
        assert_eq!(answer.body, entry.bytes, "{target}");
        assert_eq!(answer.header("content-type"), Some("text/html; charset=utf-8"), "{target}");
        assert_eq!(answer.header("cache-control"), Some("no-store"), "{target}");
    }
    // Nothing of the disk, whatever the spelling; an address nobody serves.
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let absolute = format!("/{}", manifest.display());
    for target in [
        "/Cargo.toml",
        "/../Cargo.toml",
        "/%2e%2e/Cargo.toml",
        "/..%2fCargo.toml",
        "//etc/hosts",
        absolute.as_str(),
        "/doc/paper.flm",
        "/paper.flm",
        "/nosuch.html",
        "/.flm-preview/nosuch",
        "/%ff",
    ] {
        let answer = get(preview.address(), target);
        assert_eq!(answer.status, 404, "{target}");
        assert_eq!(answer.header("cache-control"), Some("no-store"), "{target}");
    }
    assert_eq!(ask(preview.address(), "POST", "/index.html").status, 405);
    assert_eq!(get(preview.address(), REBUILD).status, 405);
    preview.stop();
}

#[cfg(feature = "graphics")]
#[test]
fn every_file_beside_the_entry_is_served_at_its_path_with_its_media_type() {
    let doc = "---\nflm:\n  features:\n    graphics:\n      global: true\n---\nA \\includegraphics{plot.png}.\n";
    let files = Shared::with("doc/paper.flm", doc);
    files.0.lock().unwrap().insert("doc/plot.png", png());
    let mut request = request();
    request.options.push(flm_main::settings::CommandLineSetting::new("graphics.strategy.html=copy", "test"));
    let preview = Preview::start(request, &files);
    let (_, output) = preview.built();
    let output = output.expect("an output");
    let beside: Vec<_> = output.files.values().filter(|file| !file.entry).collect();
    assert!(!beside.is_empty(), "{:?}", output.files.keys());
    for file in beside {
        let answer = get(preview.address(), &format!("/{}", file.path));
        assert_eq!(answer.status, 200, "{}", file.path);
        assert_eq!(answer.body, file.bytes, "{}", file.path);
        assert_eq!(answer.header("content-type"), Some(file.media_type.as_str()), "{}", file.path);
        assert_eq!(answer.header("cache-control"), Some("no-store"), "{}", file.path);
    }
    preview.stop();
}

#[cfg(feature = "graphics")]
fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
    encoder.set_color(png::ColorType::Grayscale);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().unwrap().write_image_data(&[0, 255]).unwrap();
    bytes
}

#[test]
fn before_a_build_gives_an_output_the_root_is_the_waiting_page_with_the_page_script() {
    // The front matter does not read: no output.
    let files = Shared::with("doc/paper.flm", "---\ntitle: [unclosed\n---\nText.\n");
    let preview = Preview::start(request(), &files);
    let (_, output) = preview.built();
    assert!(output.is_none());
    let answer = get(preview.address(), "/");
    assert_eq!(answer.status, 200);
    assert_eq!(answer.header("cache-control"), Some("no-store"));
    let page = String::from_utf8(answer.body).unwrap();
    assert!(page.contains("data-flm-preview-waiting"), "{page}");
    assert!(page.contains(flm_main::stdpkg::preview::SCRIPT_TEXT), "{page}");
    assert_eq!(get(preview.address(), "/index.html").status, 404);
    // Once mended, the document's page.
    files.write("doc/paper.flm", DOC);
    preview.handle.build_again();
    let (_, output) = preview.built();
    assert_eq!(get(preview.address(), "/").body, output.unwrap().entry().unwrap().bytes);
    preview.stop();
}

// --- acceptance 5: a page built under `watch` ----------------------------------------------------

#[test]
fn a_page_built_by_the_preview_carries_its_definitions_and_the_places_in_the_source() {
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start(request(), &files);
    preview.built();
    let page = String::from_utf8(get(preview.address(), "/").body).unwrap();
    assert_eq!(page.matches("data-flm-def=\"flm.preview.script\"").count(), 1, "{page}");
    assert_eq!(page.matches("data-flm-def=\"flm.preview.style\"").count(), 1, "{page}");
    assert!(page.contains(flm_main::stdpkg::preview::SCRIPT_TEXT), "{page}");
    assert!(page.contains("data-flm-source-span='[\"doc/paper.flm\","), "{page}");
    preview.stop();
}

// --- acceptance 3 and 12: the stream ------------------------------------------------------------

#[test]
fn a_client_is_told_the_build_first_then_every_update_in_order_and_two_clients_the_same() {
    let files = Shared::with("doc/paper.flm", "Hello \\ref{nosuch}.\n");
    let preview = Preview::start(request(), &files);
    preview.built();
    let mut first = Client::connect(preview.address());
    assert!(first.head.contains(&String::from("content-type: text/event-stream")), "{:?}", first.head);
    assert!(first.head.contains(&String::from("cache-control: no-store")), "{:?}", first.head);
    let (event, hello) = first.next();
    assert_eq!(event, "hello");
    assert_eq!(hello["document"], "doc/paper.flm");
    assert_eq!(hello["build"], 1);
    assert_eq!(hello["building"], false);
    assert!(hello["server"].is_string());
    // No editor was given to the server.
    assert_eq!(hello["editors"], json!([]));
    assert_eq!(hello["editor"], Value::Null);
    let problems = hello["problems"].as_array().expect("the problems");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0]["severity"], "error");
    let mut second = Client::connect(preview.address());
    assert_eq!(second.next_named("hello"), hello);

    // A client that went away holds up neither the others nor the loop.
    let gone = Client::connect(preview.address());
    drop(gone);

    for (index, text) in ["Hello.\n", "Hello again.\n", "Hello once more.\n"].iter().enumerate() {
        files.write("doc/paper.flm", text);
        preview.handle.build_again();
        let (build, _) = preview.built();
        assert_eq!(build, index as u64 + 2);
    }
    for client in [&mut first, &mut second] {
        let mut builds = Vec::new();
        for _ in 0..3 {
            let building = client.next();
            assert_eq!(building.0, "building");
            let (event, update) = client.next();
            assert_eq!(event, "update");
            builds.push(update["build"].as_u64().unwrap());
            assert!(update["problems"].as_array().unwrap().is_empty());
        }
        assert_eq!(builds, [2, 3, 4]);
    }
    // A client connecting now is told the last build.
    let mut third = Client::connect(preview.address());
    assert_eq!(third.next_named("hello")["build"], 4);
    preview.stop();
}

#[cfg(unix)]
#[test]
fn the_first_message_lists_the_editors_found_and_the_default() {
    let folder = editor::folder("server_hello");
    let record = folder.join("record");
    for name in ["code", "emacsclient", "gvim"] {
        editor::fake_editor(&folder, name, &record);
    }
    let editors = Editors::find(Some("emacsclient"), Some(folder.as_os_str()), &[]).unwrap();
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start_with_editors(request(), &files, editors);
    preview.built();
    let hello = Client::connect(preview.address()).next_named("hello");
    assert_eq!(
        hello["editors"],
        json!([
            { "name": "code", "label": "VS Code" },
            { "name": "emacsclient", "label": "Emacs" },
            { "name": "gvim", "label": "gVim" },
        ])
    );
    assert_eq!(hello["editor"], "emacsclient");
    preview.stop();
}

#[test]
fn a_request_to_build_again_causes_one_build_and_one_update_to_every_client() {
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start(request(), &files);
    preview.built();
    let mut first = Client::connect(preview.address());
    let mut second = Client::connect(preview.address());
    first.next_named("hello");
    second.next_named("hello");
    let answer = ask(preview.address(), "POST", REBUILD);
    assert_eq!(answer.status, 204);
    assert_eq!(answer.header("cache-control"), Some("no-store"));
    let (build, _) = preview.built();
    assert_eq!(build, 2);
    for client in [&mut first, &mut second] {
        let update = client.next_named("update");
        assert_eq!(update["build"], 2);
        assert_eq!(update["page"], "same");
    }
    // Exactly one: the next build is the one asked for now, numbered 3.
    preview.handle.build_again();
    assert_eq!(preview.built().0, 3);
    assert_eq!(first.next_named("update")["build"], 3);
    preview.stop();
}

#[test]
fn a_message_that_is_not_understood_and_a_page_gone_away_stall_nothing() {
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start(request(), &files);
    preview.built();
    // Bytes that are no request, and a connection closed in the middle of one.
    for bytes in [&b"\x00\x01garbage\r\n\r\n"[..], &b"GET /index.ht"[..]] {
        let mut connection = TcpStream::connect(preview.address()).unwrap();
        connection.write_all(bytes).unwrap();
        drop(connection);
    }
    assert_eq!(get(preview.address(), "/").status, 200);
    preview.handle.build_again();
    assert_eq!(preview.built().0, 2);
    preview.stop();
}

// --- acceptance 4: this machine, and the port --------------------------------------------------

#[test]
fn the_server_listens_on_this_machine_alone_and_takes_the_next_port_when_one_is_taken() {
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port();
    let files = Shared::with("doc/paper.flm", DOC);
    let watch = Watch::new(request(), Arc::new(files.clone()), Vec::new());
    let server = Server::start(Port::From(port), "doc/paper.flm", watch.handle(), Editors::none(), |_| {}, settings(&files, Value::Null))
        .expect("a free port after it");
    assert!(server.address().ip().is_loopback());
    assert!(server.address().port() > port);
    assert_eq!(server.url(), format!("http://127.0.0.1:{}/", server.address().port()));
    // A port named exactly is that one or none.
    match Server::start(Port::Exactly(port), "doc/paper.flm", watch.handle(), Editors::none(), |_| {}, settings(&files, Value::Null)) {
        Err(ServeError::Port { port: refused, .. }) => assert_eq!(refused, port),
        other => panic!("{other:?}"),
    }
    drop(taken);
}

// --- acceptance 8: an output that is no page ----------------------------------------------------

#[test]
fn an_output_that_is_no_page_is_served_and_brought_up_to_date() {
    let files = Shared::with("doc/paper.flm", "Hello.\n");
    let mut request = request();
    request.profile = Some(String::from("text"));
    let preview = Preview::start(request, &files);
    preview.built();
    let answer = get(preview.address(), "/");
    assert_eq!(answer.header("content-type"), Some("text/plain; charset=utf-8"));
    assert_eq!(String::from_utf8(answer.body).unwrap().trim(), "Hello.");
    files.write("doc/paper.flm", "Hello again.\n");
    preview.handle.build_again();
    preview.built();
    let answer = get(preview.address(), "/index.txt");
    assert_eq!(String::from_utf8(answer.body).unwrap().trim(), "Hello again.");
    preview.stop();
}

#[cfg(feature = "citations")]
#[test]
fn the_bibtex_export_is_served_and_brought_up_to_date() {
    let doc = "---\nflm:\n  features:\n    citations-standard:\n      bibliography: [refs.json]\n---\nSee \\cite{bib:knuth84}.\n";
    let refs = |title: &str| {
        format!(r#"[{{"id": "knuth84", "type": "book", "title": "{title}", "author": [{{"family": "Knuth", "given": "Donald E."}}], "issued": {{"date-parts": [[1984]]}}}}]"#)
    };
    let files = Shared::with("doc/paper.flm", doc);
    files.write("doc/refs.json", &refs("The TeXbook"));
    let mut request = request();
    request.profile = Some(String::from("bibtex"));
    request.citations.cache_folder = None;
    let preview = Preview::start(request, &files);
    let (_, output) = preview.built();
    let entry = output.expect("an output").entry().expect("an entry").path.clone();
    let answer = get(preview.address(), "/");
    assert_eq!(answer.header("content-type"), Some("application/x-bibtex"));
    assert!(String::from_utf8(answer.body).unwrap().contains("The TeXbook"));
    files.write("doc/refs.json", &refs("The METAFONTbook"));
    preview.handle.build_again();
    preview.built();
    let answer = get(preview.address(), &format!("/{entry}"));
    assert!(String::from_utf8(answer.body).unwrap().contains("The METAFONTbook"));
    preview.stop();
}

// --- the end of the stream ---------------------------------------------------------------------

#[test]
fn a_client_of_the_stream_is_told_its_end_when_the_server_stops() {
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start(request(), &files);
    preview.built();
    let mut client = Client::connect(preview.address());
    assert!(client.head.contains(&String::from("transfer-encoding: chunked")), "{:?}", client.head);
    assert_eq!(client.next().0, "hello");
    preview.stop();
    // The body's last chunk, of length zero.
    let mut rest = String::new();
    loop {
        let mut line = String::new();
        let read = client.lines.read_line(&mut line).expect("the end within the bound");
        assert!(read > 0, "the last chunk before the connection ends: {rest:?}");
        rest.push_str(&line);
        if line == "0\r\n" {
            break;
        }
    }
}

// --- the settings of the document (the *Settings* tool) -----------------------------------------

/// A document whose front matter sets one feature and switches another off.
const SET: &str = "---\ntitle: Notes\nflm:\n  features:\n    refs: {unresolved: allow}\n    theorems: false\n---\nHello.\n";

#[test]
fn the_settings_are_read_from_the_file_as_it_is_now_and_a_save_writes_it() {
    let files = Shared::with("doc/paper.flm", SET);
    let preview = Preview::start(request(), &files);
    preview.built();
    let answer = get(preview.address(), SETTINGS);
    assert_eq!((answer.status, answer.header("content-type")), (200, Some("application/json")));
    let read: Value = serde_json::from_slice(&answer.body).unwrap();
    // The schema of the build's preparation, which replaced the one the server started with.
    assert_ne!(prepared_schema(&files), schema());
    assert_eq!(read, json!({ "schema": prepared_schema(&files), "features": { "refs": { "unresolved": "allow" }, "theorems": false } }));
    // The file as it is now, not as the last build read it.
    files.write("doc/paper.flm", "Hello.\n");
    let read: Value = serde_json::from_slice(&get(preview.address(), SETTINGS).body).unwrap();
    assert_eq!(read["features"], Value::Null);

    let saved = json!({ "theorems": false, "math": { "numbered_equations": false } }).to_string();
    let answer = send(preview.address(), "POST", SETTINGS, &[("Content-Type", "application/json")], saved.as_bytes());
    assert_eq!(answer.status, 204);
    let text = String::from_utf8(files.read(Path::new("doc/paper.flm")).unwrap()).unwrap();
    assert_eq!(text, "---\nflm:\n  features:\n    theorems: false\n    math:\n      numbered_equations: false\n---\nHello.\n");
    preview.stop();
}

#[test]
fn a_save_the_server_cannot_do_is_refused_with_its_reason_and_writes_nothing() {
    let broken = "---\ntitle: [unclosed\n---\nHello.\n";
    let files = Shared::with("doc/paper.flm", broken);
    let preview = Preview::start(request(), &files);
    preview.built();
    let text = || String::from_utf8(files.read(Path::new("doc/paper.flm")).unwrap()).unwrap();
    // A front matter that is not YAML: the reader's error, placed in the file.
    for method in ["GET", "POST"] {
        let answer = send(preview.address(), method, SETTINGS, &[], b"{}");
        let reason = String::from_utf8(answer.body).unwrap();
        assert_eq!(answer.status, 409, "{method}: {reason}");
        assert!(reason.starts_with("doc/paper.flm:3:1: ") && reason.contains("must be valid YAML"), "{reason}");
    }
    assert_eq!(text(), broken);
    // A body that is not a map of features.
    for body in [&b"[1]"[..], b"not JSON"] {
        let answer = send(preview.address(), "POST", SETTINGS, &[], body);
        assert_eq!(answer.status, 400);
        assert!(String::from_utf8(answer.body).unwrap().contains("must be a JSON object"));
    }
    assert_eq!(text(), broken);
    // The settings are read and saved, never deleted or replaced.
    assert_eq!(ask(preview.address(), "PUT", SETTINGS).status, 405);
    preview.stop();
}

#[test]
fn a_save_the_file_cannot_take_is_refused_with_its_reason_and_the_file_is_untouched() {
    let files = Shared::with("doc/paper.flm", SET);
    let failing = DocumentSettings::new(schema(), "doc/paper.flm", Arc::new(files.clone()), |_, _| {
        Err(OutputError { path: Some("doc/paper.flm".into()), detail: "the disk is full".into() })
    });
    let preview = Preview::start_with(request(), &files, failing);
    preview.built();
    let saved = json!({ "theorems": false }).to_string();
    let answer = send(preview.address(), "POST", SETTINGS, &[("Content-Type", "application/json")], saved.as_bytes());
    let reason = String::from_utf8(answer.body).unwrap();
    assert_eq!(answer.status, 500, "{reason}");
    assert!(reason.contains("the disk is full"), "{reason}");
    assert_eq!(String::from_utf8(files.read(Path::new("doc/paper.flm")).unwrap()).unwrap(), SET);
    preview.stop();
}

#[test]
fn a_request_that_acts_from_the_servers_port_under_another_name_is_refused_and_does_nothing() {
    let files = Shared::with("doc/paper.flm", SET);
    let preview = Preview::start(request(), &files);
    preview.built();
    let own = format!("http://{}", preview.address());
    let other_name = format!("http://localhost:{}", preview.address().port());
    for target in [REBUILD, OPEN, SETTINGS] {
        let answer = send(preview.address(), "POST", target, &[("Origin", &other_name)], b"{}");
        let reason = String::from_utf8(answer.body).unwrap();
        assert_eq!(answer.status, 403, "{target}: {reason}");
        assert_eq!(reason, format!("this request must come from the preview's page at {own}/"));
    }
    assert_eq!(String::from_utf8(files.read(Path::new("doc/paper.flm")).unwrap()).unwrap(), SET);
    // The refused request to build again built nothing: the build that follows is the second.
    assert_eq!(send(preview.address(), "POST", REBUILD, &[("Origin", &own)], b"").status, 204);
    assert_eq!(preview.built().0, 2);
    preview.stop();
}

#[test]
fn a_request_that_acts_from_a_page_of_another_origin_is_refused_and_does_nothing() {
    let files = Shared::with("doc/paper.flm", SET);
    let preview = Preview::start(request(), &files);
    preview.built();
    let own = format!("http://{}", preview.address());
    for origin in ["http://example.com", "null", "http://localhost:1"] {
        for target in [REBUILD, OPEN, SETTINGS] {
            let answer = send(preview.address(), "POST", target, &[("Origin", origin)], b"{}");
            let reason = String::from_utf8(answer.body).unwrap();
            assert_eq!(answer.status, 403, "{origin} {target}: {reason}");
            assert_eq!(reason, format!("this request must come from the preview's page at {own}/"));
        }
    }
    assert_eq!(String::from_utf8(files.read(Path::new("doc/paper.flm")).unwrap()).unwrap(), SET);
    // A page of the server's own is honored, and the refused requests to build again built
    // nothing: the build that follows is the second.
    assert_eq!(send(preview.address(), "POST", REBUILD, &[("Origin", &own)], b"").status, 204);
    assert_eq!(preview.built().0, 2);
    assert_eq!(send(preview.address(), "POST", SETTINGS, &[("Origin", &own)], b"{}").status, 204);
    assert!(String::from_utf8(files.read(Path::new("doc/paper.flm")).unwrap()).unwrap().contains("features: {}"));
    // A page that reads is not checked: the browser keeps the answer from a page of another
    // origin.
    assert_eq!(send(preview.address(), "GET", SETTINGS, &[("Origin", "http://example.com")], b"").status, 200);
    preview.stop();
}
