//! The preview's server (`watch::server`): the loop driven over files held in memory, each
//! of its events handed to the server, and the server read through plain connections of
//! this machine. Every wait has an upper bound and ends as soon as what is expected
//! arrives; the system chooses every port.

#[cfg(unix)]
#[path = "support/editor.rs"]
mod editor;
#[path = "support/waiting.rs"]
mod waiting;

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::Path;
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use flm_core::progress::Scope;
use flm_host::files::{FileError, Files, MemoryFiles};
use flm_host::output::{Naming, OutputError};
use flm_host::session::{Input, Prepared, Request};
use flm_host::watch::editor::{CUSTOM, EDITORS, Editors};
use flate2::read::GzDecoder;
use flm_host::watch::server::{
    ASSETS, AT, EVENTS, FRONT_MATTER, HELP, HELP_INDEX, HELP_PAGE, OPEN, Port, REBUILD, SETTINGS, SETTINGS_SOURCES, SETTINGS_YAML, ServeError,
    Server,
    TOOLS, VIEWER,
};
use flm_host::watch::services::{Help, HelpEntry, HelpError, HelpPage, HelpSegment, NoServices, Services};
use flm_host::watch::snapshot::Construct;
use flm_host::watch::settings::DocumentSettings;
use flm_host::watch::update::BuildOutput;
use flm_host::watch::{Event, Handle, Watch};
use flm_core::core::registry::PackageFactory;
use waiting::{Waiting, waiting_package};
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

/// The settings of `doc/paper.flm` over `files`, a change written into them, the schema
/// `schema` until the first preparation.
fn settings(files: &Shared, schema: Value) -> DocumentSettings {
    let written = files.clone();
    DocumentSettings::new(schema, "doc/paper.flm", Arc::new(files.clone()), move |path, bytes| {
        written.0.lock().unwrap().insert(path, bytes);
        Ok(())
    })
}

impl Files for Shared {
    fn read(&self, path: &str) -> Result<Vec<u8>, FileError> {
        self.0.lock().unwrap().read(path)
    }

    fn entries(&self, path: &str) -> Result<Vec<String>, FileError> {
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
        Preview::serve(request, files, settings, Editors::none(), NoServices, Vec::new())
    }

    /// The preview of `request` over `files`, with the compiled packages `compiled`.
    fn start_compiled(request: Request, files: &Shared, compiled: Vec<PackageFactory>) -> Preview {
        Preview::serve(request, files, settings(files, schema()), Editors::none(), NoServices, compiled)
    }

    /// The preview of `request` over `files`, the program's services given by `services`.
    fn start_with_services(request: Request, files: &Shared, services: impl Services + 'static) -> Preview {
        Preview::serve(request, files, settings(files, schema()), Editors::none(), services, Vec::new())
    }

    /// The preview of `request` over `files`, a place opened in one of `editors`.
    #[cfg(unix)]
    fn start_with_editors(request: Request, files: &Shared, editors: Editors) -> Preview {
        Preview::serve(request, files, settings(files, schema()), editors, NoServices, Vec::new())
    }

    /// The preview of `request` over `files`, the settings read and saved by `settings`, a
    /// place opened in one of `editors`, the program's services given by `services`, with the
    /// compiled packages `compiled`.
    fn serve(
        request: Request,
        files: &Shared,
        settings: DocumentSettings,
        editors: Editors,
        services: impl Services + 'static,
        compiled: Vec<PackageFactory>,
    ) -> Preview {
        let watch = Watch::new(request, Arc::new(files.clone()), compiled);
        let handle = watch.handle();
        let server = Arc::new(Server::start(Port::Exactly(0), "doc/paper.flm", handle.clone(), editors, |_| {}, settings, services)
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
    let prepared = flm_host::session::prepare(request(), Arc::new(files.clone()), &[], &Scope::none())
        .expect("prepared");
    flm_host::seed::front_matter_schema(&prepared.factories().iter().collect::<Vec<_>>(), prepared.suggestions())
}

/// The request for `doc/paper.flm`, its output named in a folder (`index.html`, …).
fn request() -> Request {
    let mut request = Request::new(Input::File(String::from("doc/paper.flm")));
    request.default_math = flm_host::seed::MathChoice::processor("passthrough");
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

    /// The next message other than `progress`, which the server sends while a build runs
    /// as often as its progress changes: its event's name and its data read as JSON.
    fn next(&mut self) -> (String, Value) {
        loop {
            let (event, data) = self.message();
            if event != "progress" {
                return (event, data);
            }
        }
    }

    /// The next message, whatever its name: its event's name and its data read as JSON.
    fn message(&mut self) -> (String, Value) {
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
            let (event, data) = self.message();
            if event == name {
                return data;
            }
        }
    }
}

// --- what is served, and that nothing is kept --------------------------------------------------

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
    request.options.push(flm_host::settings::CommandLineSetting::new("graphics.strategy.html=copy", "test"));
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
    assert!(page.contains(flm_host::stdpkg::preview::SCRIPT_TEXT), "{page}");
    assert_eq!(get(preview.address(), "/index.html").status, 404);
    // Once mended, the document's page.
    files.write("doc/paper.flm", DOC);
    preview.handle.build_again();
    let (_, output) = preview.built();
    assert_eq!(get(preview.address(), "/").body, output.unwrap().entry().unwrap().bytes);
    preview.stop();
}

// --- a page built under `watch` ------------------------------------------------------------------

#[test]
fn a_page_built_by_the_preview_carries_its_definitions_and_the_places_in_the_source() {
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start(request(), &files);
    preview.built();
    let page = String::from_utf8(get(preview.address(), "/").body).unwrap();
    assert_eq!(page.matches("data-flm-def=\"flm.preview.script\"").count(), 1, "{page}");
    assert_eq!(page.matches("data-flm-def=\"flm.preview.style\"").count(), 1, "{page}");
    assert!(page.contains(flm_host::stdpkg::preview::SCRIPT_TEXT), "{page}");
    assert!(page.contains("data-flm-source-span='[\"doc/paper.flm\","), "{page}");
    preview.stop();
}

// --- the stream ---------------------------------------------------------------------------------

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
    // No editor was given to the server: every editor is listed, none available but the
    // editor command; and the viewer settings are the defaults.
    let editors = hello["editors"].as_array().unwrap();
    assert_eq!(editors.len(), EDITORS.len() + 1);
    assert!(editors.iter().all(|editor| editor["available"] == (editor["name"] == CUSTOM)), "{editors:?}");
    assert_eq!(
        hello["viewer"],
        json!({
            "editor": null,
            "theme": "system",
            "scroll_to_changes": true,
            "open_problems_on_errors": false,
            "wrap_lines": true,
            "panel_side": "right",
            "panel_width": null,
            "panel_height": null,
            "unreadable": null,
        })
    );
    // The Warning of the page's unchecked formulas, then the document's Error.
    let problems = hello["problems"].as_array().expect("the problems");
    assert_eq!(problems.len(), 2, "{problems:?}");
    assert_eq!(problems[0]["severity"], "warning");
    assert!(problems[0]["message"].as_str().is_some_and(|text| text.starts_with("Unchecked output")), "{problems:?}");
    assert_eq!(problems[1]["severity"], "error");
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
            // The Warning of the page's unchecked formulas alone.
            let problems = update["problems"].as_array().unwrap();
            assert_eq!(problems.len(), 1, "{problems:?}");
            assert_eq!(problems[0]["severity"], "warning");
            assert!(problems[0]["message"].as_str().is_some_and(|text| text.starts_with("Unchecked output")), "{problems:?}");
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
fn the_first_message_lists_every_editor_those_found_available_and_the_default() {
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
    // Every editor, then the editor command, always available.
    let listed: Vec<(&str, bool)> = hello["editors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|editor| (editor["name"].as_str().unwrap(), editor["available"].as_bool().unwrap()))
        .collect();
    let names: Vec<&str> = listed.iter().map(|(name, _)| *name).collect();
    assert_eq!(names, EDITORS.iter().copied().chain([CUSTOM]).collect::<Vec<_>>());
    let available: Vec<&str> = listed.iter().filter(|(_, available)| *available).map(|(name, _)| *name).collect();
    assert_eq!(available, ["code", "emacsclient", "gvim", CUSTOM]);
    assert_eq!(hello["editors"][0], json!({ "name": "code", "label": "VS Code", "available": true }));
    assert_eq!(hello["viewer"]["editor"], "emacsclient");
    preview.stop();
}

/// The program's services that give the home folder `home` alone.
struct Home(std::path::PathBuf);

impl Services for Home {
    fn home_folder(&self) -> Option<std::path::PathBuf> {
        Some(self.0.clone())
    }
}

/// A home folder of its own for the test `name`, empty, under the build's folder for tests.
fn home(name: &str) -> std::path::PathBuf {
    let folder = Path::new(env!("CARGO_TARGET_TMPDIR")).join("viewer").join(name);
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    folder
}

/// Asks the server at `address` to change the viewer settings by `changes`.
fn change_viewer(address: SocketAddr, changes: Value) -> Answer {
    send(address, "POST", VIEWER, &[("Content-Type", "application/json")], changes.to_string().as_bytes())
}

#[test]
fn the_viewer_settings_are_read_from_the_home_folder_and_a_change_writes_them_for_every_page() {
    let home = home("read_and_written");
    let file = home.join(".flm/watch.json");
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, r#"{"theme": "dark", "later": {"kept": true}}"#).unwrap();
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start_with_services(request(), &files, Home(home.clone()));
    preview.built();
    assert_eq!(preview.server.viewer_unreadable(), None);
    let mut client = Client::connect(preview.address());
    let hello = client.next_named("hello");
    assert_eq!(hello["viewer"]["theme"], "dark");
    assert_eq!(hello["viewer"]["wrap_lines"], true);
    // A change: written into the file, the other keys kept, and told to every page.
    let answer = change_viewer(preview.address(), json!({ "wrap_lines": false, "panel_width": 420 }));
    assert_eq!(answer.status, 204, "{}", String::from_utf8_lossy(&answer.body));
    let message = client.next_named("viewer");
    assert_eq!(message["editors"], hello["editors"]);
    let viewer = &message["viewer"];
    assert_eq!(viewer["wrap_lines"], false);
    assert_eq!(viewer["panel_width"], 420);
    assert_eq!(viewer["theme"], "dark");
    let written: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(written, json!({ "theme": "dark", "later": {"kept": true}, "wrap_lines": false, "panel_width": 420 }));
    assert_eq!(&Client::connect(preview.address()).next_named("hello")["viewer"], viewer);
    assert_eq!(send(preview.address(), "GET", VIEWER, &[], b"").status, 405);
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
fn the_stream_carries_the_progress_of_a_build_while_its_stage_runs() {
    let (running, stage_runs) = channel();
    let (release, released) = channel();
    let (cancelled, _) = channel();
    let waiting = Arc::new(Waiting { running, release: Mutex::new(released), cancelled });
    let files = Shared::with("doc/paper.flm", "---\nflm: {use: [waiting]}\n---\nHello.\n");
    let mut request = request();
    request.profile = Some(String::from("waiting"));
    let preview = Preview::start_compiled(request, &files, vec![waiting_package(waiting)]);
    stage_runs.recv_timeout(BOUND).expect("the build's stage runs");
    let mut client = Client::connect(preview.address());
    assert_eq!(client.next_named("hello")["building"], true);
    // Sent after `hello` when the text was sent before the client connected, and otherwise
    // at the server's next reading of the build.
    let progress = client.next_named("progress");
    assert!(progress["text"].as_str().is_some_and(|text| text.contains('%')), "{progress}");
    release.send(()).unwrap();
    assert_eq!(preview.built().0, 1);
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

// --- this machine, and the port ----------------------------------------------------------------

#[test]
fn the_server_listens_on_this_machine_alone_and_takes_the_next_port_when_one_is_taken() {
    let taken = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = taken.local_addr().unwrap().port();
    let files = Shared::with("doc/paper.flm", DOC);
    let watch = Watch::new(request(), Arc::new(files.clone()), Vec::new());
    let server = Server::start(Port::From(port), "doc/paper.flm", watch.handle(), Editors::none(), |_| {}, settings(&files, Value::Null), NoServices)
        .expect("a free port after it");
    assert!(server.address().ip().is_loopback());
    assert!(server.address().port() > port);
    assert_eq!(server.url(), format!("http://127.0.0.1:{}/", server.address().port()));
    // A port named exactly is that one or none.
    match Server::start(Port::Exactly(port), "doc/paper.flm", watch.handle(), Editors::none(), |_| {}, settings(&files, Value::Null), NoServices) {
        Err(ServeError::Port { port: refused, .. }) => assert_eq!(refused, port),
        other => panic!("{other:?}"),
    }
    drop(taken);
}

// --- an output that is no page ------------------------------------------------------------------

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
    let mut client = Client::connect(preview.address());
    assert_eq!(client.next_named("hello")["media_type"], "text/plain");
    files.write("doc/paper.flm", "Hello again.\n");
    preview.handle.build_again();
    preview.built();
    // A text has no output regions: its change is a reload.
    let update = client.next_named("update");
    assert_eq!(update["page"], "reload", "{update}");
    assert_eq!(update["media_type"], "text/plain");
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

// --- the tools page -----------------------------------------------------------------------------

/// The bytes of `answer`'s body, decompressed when it says it is compressed with gzip.
fn decoded(answer: &Answer) -> Vec<u8> {
    if answer.header("content-encoding") == Some("gzip") {
        let mut bytes = Vec::new();
        GzDecoder::new(answer.body.as_slice()).read_to_end(&mut bytes).expect("gzip");
        bytes
    } else {
        answer.body.clone()
    }
}

#[test]
fn the_tools_page_is_sent_compressed_to_a_request_that_accepts_gzip_and_plain_otherwise() {
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start(request(), &files);
    preview.built();
    let index = format!("{ASSETS}index.html");
    // The tools page is also the help browser alone, at its address of its own.
    for target in [TOOLS, HELP, index.as_str()] {
        let compressed = send(preview.address(), "GET", target, &[("Accept-Encoding", "deflate, gzip;q=0.8, br")], b"");
        assert_eq!(compressed.status, 200, "{target}");
        assert_eq!(compressed.header("content-encoding"), Some("gzip"), "{target}");
        assert_eq!(compressed.header("content-type"), Some("text/html; charset=utf-8"), "{target}");
        assert_eq!(compressed.header("cache-control"), Some("no-store"), "{target}");
        let page = String::from_utf8(decoded(&compressed)).unwrap();
        // The page loads its script and its stylesheet from the assets.
        for asset in ["tools.js", "tools.css"] {
            assert!(page.contains(&format!("\"{ASSETS}{asset}\"")), "{page}");
        }
        for headers in [&[][..], &[("Accept-Encoding", "identity")][..], &[("Accept-Encoding", "gzip;q=0, *")][..]] {
            let plain = send(preview.address(), "GET", target, headers, b"");
            assert_eq!(plain.status, 200, "{target} {headers:?}");
            assert_eq!(plain.header("content-encoding"), None, "{target} {headers:?}");
            assert_eq!(String::from_utf8(plain.body).unwrap(), page, "{target} {headers:?}");
        }
    }
    let assets = [
        ("tools.js", "text/javascript; charset=utf-8"),
        ("tools.css", "text/css; charset=utf-8"),
        ("pdfjs/pdf.min.mjs", "text/javascript; charset=utf-8"),
        ("pdfjs/pdf.worker.min.mjs", "text/javascript; charset=utf-8"),
        ("pdfjs/standard_fonts/FoxitSymbol.pfb", "application/octet-stream"),
        ("pdfjs/standard_fonts/FoxitDingbats.pfb", "application/octet-stream"),
        ("pdfjs/standard_fonts/LICENSES.txt", "text/plain; charset=utf-8"),
    ];
    for (asset, media_type) in assets {
        let answer = send(preview.address(), "GET", &format!("{ASSETS}{asset}"), &[], b"");
        assert_eq!((answer.status, answer.header("content-type")), (200, Some(media_type)), "{asset}");
    }
    let wildcard = send(preview.address(), "GET", TOOLS, &[("Accept-Encoding", "*")], b"");
    assert_eq!(wildcard.header("content-encoding"), Some("gzip"));
    preview.stop();
}

#[test]
fn nothing_but_the_embedded_files_is_served_under_assets() {
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start(request(), &files);
    preview.built();
    let targets = [
        String::from(ASSETS),
        format!("{ASSETS}INDEX.HTML"),
        format!("{ASSETS}index.htm"),
        format!("{ASSETS}missing.js"),
        format!("{ASSETS}../paper.flm"),
        format!("{ASSETS}%2e%2e/doc/paper.flm"),
        format!("{ASSETS}/index.html"),
        format!("{ASSETS}index.html/"),
        format!("{TOOLS}index.html"),
        format!("{ASSETS}%ff"),
    ];
    for target in &targets {
        assert_eq!(get(preview.address(), target).status, 404, "{target}");
    }
    assert_eq!(ask(preview.address(), "POST", TOOLS).status, 405);
    preview.stop();
}

#[test]
fn the_first_message_and_each_update_tell_the_media_type_and_an_output_of_an_earlier_build() {
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start(request(), &files);
    preview.built();
    let hello = Client::connect(preview.address()).next_named("hello");
    assert_eq!(hello["media_type"], "text/html", "{hello}");
    assert_eq!(hello["earlier_output"], false, "{hello}");
    assert!(hello["duration_ms"].is_u64(), "{hello}");
    let mut client = Client::connect(preview.address());
    client.next_named("hello");
    // A front matter that cannot be read: the build gives no output, and the output served
    // stays the first build's.
    files.write("doc/paper.flm", "---\ntitle: [\n---\nHello.\n");
    preview.handle.build_again();
    preview.built();
    let update = client.next_named("update");
    assert_eq!(update["page"], "absent", "{update}");
    assert_eq!(update["media_type"], "text/html", "{update}");
    assert_eq!(update["earlier_output"], true, "{update}");
    assert!(update["duration_ms"].is_u64(), "{update}");
    let hello = Client::connect(preview.address()).next_named("hello");
    assert_eq!(hello["build"], 1, "{hello}");
    assert_eq!(hello["earlier_output"], true, "{hello}");
    files.write("doc/paper.flm", DOC);
    preview.handle.build_again();
    preview.built();
    let update = client.next_named("update");
    assert_eq!(update["earlier_output"], false, "{update}");
    preview.stop();
}

#[test]
fn a_request_to_change_the_build_type_builds_the_other_profile_and_the_stream_tells_it() {
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start(request(), &files);
    preview.built();
    let mut client = Client::connect(preview.address());
    let hello = client.next_named("hello");
    assert_eq!(hello["profile"], "html", "{hello}");
    let profiles = hello["profiles"].as_array().expect("a list of profiles");
    assert!(profiles.iter().any(|profile| profile == "html") && profiles.iter().any(|profile| profile == "text"), "{hello}");
    let changed = send(preview.address(), "POST", "/.flm-preview/profile", &[], br#"{"profile": "text"}"#);
    assert_eq!(changed.status, 204);
    preview.built();
    let update = client.next_named("update");
    assert_eq!(update["profile"], "text", "{update}");
    assert_eq!(update["page"], "reload", "{update}");
    assert!(update["media_type"].as_str().is_some_and(|media_type| media_type.starts_with("text/plain")), "{update}");
    assert_eq!(update["earlier_output"], false, "{update}");
    // A profile that cannot be prepared: the text output stays, from an earlier build.
    let changed = send(preview.address(), "POST", "/.flm-preview/profile", &[], br#"{"profile": "nosuch"}"#);
    assert_eq!(changed.status, 204);
    preview.built();
    let update = client.next_named("update");
    assert_eq!(update["profile"], "nosuch", "{update}");
    assert_eq!(update["earlier_output"], true, "{update}");
    assert!(update["media_type"].as_str().is_some_and(|media_type| media_type.starts_with("text/plain")), "{update}");
    assert!(!update["problems"].as_array().unwrap().is_empty(), "{update}");
    assert_eq!(update["profiles"], hello["profiles"], "{update}");
    // A body that names no profile is refused, with its reason.
    for body in [&b"{}"[..], br#"{"profile": ""}"#, b"text"] {
        let refused = send(preview.address(), "POST", "/.flm-preview/profile", &[], body);
        assert_eq!(refused.status, 400);
        assert!(!refused.body.is_empty());
    }
    assert_eq!(ask(preview.address(), "GET", "/.flm-preview/profile").status, 405);
    preview.stop();
}

#[test]
fn the_default_build_type_is_the_one_asked_for_at_start_else_the_front_matter_s_and_follows_the_front_matter() {
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start(request(), &files);
    preview.built();
    let mut client = Client::connect(preview.address());
    let hello = client.next_named("hello");
    assert_eq!(hello["default_profile"], "html", "{hello}");
    // The build type a page chooses is not the default build type.
    let changed = send(preview.address(), "POST", "/.flm-preview/profile", &[], br#"{"profile": "text"}"#);
    assert_eq!(changed.status, 204);
    preview.built();
    let update = client.next_named("update");
    assert_eq!(update["profile"], "text", "{update}");
    assert_eq!(update["default_profile"], "html", "{update}");
    // The front matter's `default_profile` is followed when it changes.
    files.write("doc/paper.flm", "---\ntitle: Notes\nflm:\n  default_profile: text\n---\nHello.\n");
    preview.handle.build_again();
    preview.built();
    let update = client.next_named("update");
    assert_eq!(update["default_profile"], "text", "{update}");
    preview.stop();
    // A profile asked for at start is the default build type, over the front matter's.
    let mut asked = request();
    asked.profile = Some(String::from("html"));
    let preview = Preview::start(asked, &files);
    preview.built();
    let hello = Client::connect(preview.address()).next_named("hello");
    assert_eq!(hello["default_profile"], "html", "{hello}");
    preview.stop();
}

#[test]
fn before_any_build_the_first_message_tells_no_output_and_no_duration() {
    let files = Shared::with("doc/paper.flm", DOC);
    let watch = Watch::new(request(), Arc::new(files.clone()), Vec::new());
    let server = Server::start(Port::Exactly(0), "doc/paper.flm", watch.handle(), Editors::none(), |_| {}, settings(&files, Value::Null), NoServices)
        .expect("the server starts");
    let hello = Client::connect(server.address()).next_named("hello");
    assert_eq!(hello["media_type"], Value::Null, "{hello}");
    assert_eq!(hello["earlier_output"], false, "{hello}");
    assert_eq!(hello["duration_ms"], Value::Null, "{hello}");
    assert_eq!(hello["profile"], Value::Null, "{hello}");
    assert_eq!(hello["default_profile"], Value::Null, "{hello}");
    assert_eq!(hello["profiles"], json!([]), "{hello}");
    for field in ["references", "outline", "labels"] {
        assert_eq!(hello[field], Value::Null, "{field}: {hello}");
    }
    assert_eq!(get(server.address(), &format!("{AT}?file=doc/paper.flm&start=0")).status, 404);
}

// --- the document snapshot: the outline, the labels, the constructs at a place ------------------

const OUTLINED: &str = "---\ntitle: Notes\n---\n\\section{Intro}\\label{sec:intro}\n\nHello \\emph{a \\textbf{bold} word}.\n";

#[test]
fn the_messages_carry_the_outline_and_the_labels_and_a_place_answers_its_constructs() {
    let files = Shared::with("doc/paper.flm", OUTLINED);
    let preview = Preview::start(request(), &files);
    preview.built();
    let hello = Client::connect(preview.address()).next_named("hello");
    assert_eq!(hello["outline"], json!([{"level": 1, "number": null, "title": "Intro", "id": "sec-intro"}]), "{hello}");
    assert_eq!(hello["labels"], json!([{"label": "sec:intro", "designation": "Intro", "id": "sec-intro", "kind": "section", "heading": 0}]), "{hello}");
    assert_eq!(hello["references"], json!({"sec-intro": "sec:intro"}), "{hello}");
    let mut client = Client::connect(preview.address());
    client.next_named("hello");
    preview.handle.build_again();
    preview.built();
    let update = client.next_named("update");
    assert_eq!(update["outline"], hello["outline"], "{update}");
    assert_eq!(update["labels"], hello["labels"], "{update}");

    // The content below the front matter, as the page's elements count it.
    let content = OUTLINED.split("---\n").nth(2).unwrap();
    let start = content.find("bold").unwrap();
    let answer = get(preview.address(), &format!("{AT}?file=doc%2Fpaper.flm&start={start}"));
    assert_eq!(answer.status, 200);
    assert_eq!(answer.header("content-type"), Some("application/json"));
    let at: Value = serde_json::from_slice(&answer.body).unwrap();
    assert_eq!((&at["file"], &at["start"], &at["line"]), (&json!("doc/paper.flm"), &json!(start), &json!(6)), "{at}");
    let names: Vec<&str> = at["constructs"].as_array().unwrap().iter().map(|construct| construct["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["textbf", "emph"], "{at}");
    let textbf = &at["constructs"][0];
    assert_eq!(textbf["kind"], "macro", "{at}");
    let range = textbf["start"].as_u64().unwrap() as usize..textbf["end"].as_u64().unwrap() as usize;
    assert_eq!(&content[range], "\\textbf{bold}");
    // A program that gives no help pages gives no construct a help page.
    assert_eq!(textbf["help"], Value::Null, "{at}");

    for (query, status) in [
        (String::from("?file=doc/paper.flm"), 400),
        (String::from("?start=3"), 400),
        (String::from("?file=doc/paper.flm&start=x"), 400),
        (String::from("?file=a&file=b&start=0"), 400),
        (String::from("?file=doc/other.flm&start=0"), 404),
        (format!("?file=doc/paper.flm&start={}", content.len() + 1), 404),
    ] {
        let answer = get(preview.address(), &format!("{AT}{query}"));
        assert_eq!(answer.status, status, "{query}");
        assert!(!answer.body.is_empty(), "{query}");
    }
    assert_eq!(ask(preview.address(), "POST", AT).status, 405);
    preview.stop();
}

// --- the help pages ----------------------------------------------------------------------------

/// A help service standing for the program's: one page, `features/emph`, and the page of
/// each construct named after it; it records the digest of each preparation it is asked
/// about.
struct OnePage {
    asked: Arc<Mutex<Vec<u64>>>,
}

impl Services for OnePage {
    fn help(&self) -> Option<&dyn Help> {
        Some(self)
    }
}

impl Help for OnePage {
    fn index(&self, prepared: &Prepared) -> Result<Arc<[HelpEntry]>, HelpError> {
        self.asked.lock().unwrap().push(prepared.registry().digest());
        let entry = HelpEntry {
            path: String::from("features/emph"),
            kind: String::from("command"),
            title: String::from("Macro \\emph"),
            summary: String::from("Emphasis."),
            names: vec![String::from("\\emph"), String::from("emph")],
        };
        Ok(Arc::from(vec![entry]))
    }

    fn page(&self, prepared: &Prepared, path: &str) -> Result<HelpPage, HelpError> {
        self.asked.lock().unwrap().push(prepared.registry().digest());
        match path {
            "features/emph" => Ok(HelpPage {
                path: path.to_string(),
                kind: String::from("command"),
                title: String::from("Macro \\emph"),
                content: String::from("<p>Emphasis.</p>"),
                settings_feature: Some(String::from("emph")),
                path_line: vec![
                    HelpSegment { text: String::from("features"), path: String::from("features/") },
                    HelpSegment { text: String::from("emph"), path: String::from("features/emph") },
                ],
            }),
            "broken" => Err(HelpError::Failed(String::from("Failed to render the help page ‘broken’"))),
            _ => Err(HelpError::NoPage { path: path.to_string(), nearest: vec![(String::from("features/emph"), String::from("Macro \\emph"))] }),
        }
    }

    fn construct_page(&self, _prepared: &Prepared, construct: &Construct) -> Option<String> {
        Some(format!("features/{}", construct.name))
    }
}

#[test]
fn a_program_that_gives_no_help_pages_is_told_and_its_requests_refused() {
    let files = Shared::with("doc/paper.flm", DOC);
    let preview = Preview::start(request(), &files);
    preview.built();
    assert_eq!(Client::connect(preview.address()).next_named("hello")["help"], false);
    for target in [HELP_INDEX.to_string(), format!("{HELP_PAGE}?path=features/emph")] {
        let answer = get(preview.address(), &target);
        assert_eq!(answer.status, 501, "{target}");
        assert!(!answer.body.is_empty(), "{target}");
    }
    preview.stop();
}

#[test]
fn the_help_pages_are_the_program_s_for_the_last_preparation() {
    let files = Shared::with("doc/paper.flm", OUTLINED);
    let asked = Arc::new(Mutex::new(Vec::new()));
    let preview = Preview::start_with_services(request(), &files, OnePage { asked: Arc::clone(&asked) });
    preview.built();
    let hello = Client::connect(preview.address()).next_named("hello");
    assert_eq!(hello["help"], true, "{hello}");

    // The index, for the environment of the preparation the stream names.
    let answer = get(preview.address(), HELP_INDEX);
    assert_eq!(answer.status, 200);
    assert_eq!(answer.header("content-type"), Some("application/json"));
    let index: Value = serde_json::from_slice(&answer.body).unwrap();
    assert_eq!(index["environment"], hello["environment"], "{index}");
    assert_eq!(
        index["pages"],
        json!([{ "path": "features/emph", "kind": "command", "title": "Macro \\emph", "summary": "Emphasis.", "names": ["\\emph", "emph"] }])
    );
    let digest = u64::from_str_radix(hello["environment"].as_str().unwrap(), 16).unwrap();
    assert_eq!(*asked.lock().unwrap(), [digest]);

    // A page, a path without a page, a page that cannot be made.
    let page: Value = serde_json::from_slice(&get(preview.address(), &format!("{HELP_PAGE}?path=features%2Femph")).body).unwrap();
    assert_eq!(page["path"], "features/emph", "{page}");
    assert_eq!(page["page"]["content"], "<p>Emphasis.</p>", "{page}");
    assert_eq!(page["page"]["settings_feature"], "emph", "{page}");
    assert_eq!(
        page["page"]["path_line"],
        json!([{ "text": "features", "path": "features/" }, { "text": "emph", "path": "features/emph" }]),
        "{page}"
    );
    assert_eq!(page["nearest"], json!([]), "{page}");
    let missing: Value = serde_json::from_slice(&get(preview.address(), &format!("{HELP_PAGE}?path=features/em")).body).unwrap();
    assert_eq!(missing, json!({ "path": "features/em", "page": null, "nearest": [{ "path": "features/emph", "title": "Macro \\emph" }] }));
    let broken = get(preview.address(), &format!("{HELP_PAGE}?path=broken"));
    assert_eq!((broken.status, broken.body.as_slice()), (500, "Failed to render the help page ‘broken’".as_bytes()));
    for query in ["", "?path=a&path=b", "?other=a"] {
        assert_eq!(get(preview.address(), &format!("{HELP_PAGE}{query}")).status, 400, "{query}");
    }
    assert_eq!(ask(preview.address(), "POST", HELP_INDEX).status, 405);

    // Each construct at a place, with its help page.
    let content = OUTLINED.split("---\n").nth(2).unwrap();
    let start = content.find("bold").unwrap();
    let at: Value = serde_json::from_slice(&get(preview.address(), &format!("{AT}?file=doc/paper.flm&start={start}")).body).unwrap();
    let help: Vec<&Value> = at["constructs"].as_array().unwrap().iter().map(|construct| &construct["help"]).collect();
    assert_eq!(help, [&json!("features/textbf"), &json!("features/emph")], "{at}");
    preview.stop();
}

#[test]
fn before_a_preparation_succeeds_the_help_pages_are_not_found() {
    let files = Shared::with("doc/paper.flm", DOC);
    let watch = Watch::new(request(), Arc::new(files.clone()), Vec::new());
    let services = OnePage { asked: Arc::new(Mutex::new(Vec::new())) };
    let server = Server::start(Port::Exactly(0), "doc/paper.flm", watch.handle(), Editors::none(), |_| {}, settings(&files, Value::Null), services)
        .expect("the server starts");
    let answer = get(server.address(), HELP_INDEX);
    assert_eq!(answer.status, 404);
    assert!(!answer.body.is_empty());
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

/// The revision of the front matter that the settings state gives.
fn revision(address: SocketAddr) -> String {
    let state: Value = serde_json::from_slice(&get(address, SETTINGS).body).unwrap();
    state["revision"].as_str().unwrap().to_string()
}

/// A request to change the front matter, with the body `body`.
fn change(address: SocketAddr, body: Value) -> Answer {
    send(address, "POST", FRONT_MATTER, &[("Content-Type", "application/json")], body.to_string().as_bytes())
}

#[test]
fn the_settings_are_read_from_the_file_as_it_is_now_and_a_change_writes_it() {
    let files = Shared::with("doc/paper.flm", SET);
    let preview = Preview::start(request(), &files);
    preview.built();
    let answer = get(preview.address(), SETTINGS);
    assert_eq!((answer.status, answer.header("content-type")), (200, Some("application/json")));
    let read: Value = serde_json::from_slice(&answer.body).unwrap();
    // The schema of the build's preparation, which replaced the one the server started with.
    assert_ne!(prepared_schema(&files), schema());
    assert_eq!(read["schema"], prepared_schema(&files));
    let written = json!({ "title": "Notes", "flm": { "features": { "refs": { "unresolved": "allow" }, "theorems": false } } });
    assert_eq!(read["front_matter"], written);
    // The file as it is now, not as the last build read it.
    files.write("doc/paper.flm", "Hello.\n");
    let now: Value = serde_json::from_slice(&get(preview.address(), SETTINGS).body).unwrap();
    assert_eq!(now["front_matter"], Value::Null);
    assert_ne!(now["revision"], read["revision"]);

    // A value as JSON, a value as YAML text, a removal.
    let text = || String::from_utf8(files.read("doc/paper.flm").unwrap()).unwrap();
    let path = json!(["flm", "features", "math", "numbered_equations"]);
    let answer = change(preview.address(), json!({ "revision": now["revision"], "changes": [{ "path": path, "value": false }] }));
    assert_eq!((answer.status, answer.header("content-type")), (200, Some("application/json")));
    assert_eq!(serde_json::from_slice::<Value>(&answer.body).unwrap(), json!({ "copied": [] }));
    assert_eq!(text(), "---\nflm:\n  features:\n    math:\n      numbered_equations: false\n---\nHello.\n");
    let yaml = json!({ "revision": revision(preview.address()), "changes": [{ "path": ["flm", "features", "toc"], "yaml": "{enabled: false}\n" }] });
    assert_eq!(change(preview.address(), yaml).status, 200);
    assert!(text().contains("    toc:\n      enabled: false\n"), "{}", text());
    let removal = json!({ "revision": revision(preview.address()), "changes": [{ "path": path, "remove": true }] });
    assert_eq!(change(preview.address(), removal).status, 200);
    assert_eq!(text(), "---\nflm:\n  features:\n    toc:\n      enabled: false\n---\nHello.\n");
    preview.stop();
}

/// A document that uses the extension `notes`, which writes settings of its own, sets a
/// key and a parsing key, and switches a feature off.
const USES_NOTES: &str = "---\nflm:\n  use: [notes]\n  features:\n    math: {numbered_equations: false}\n    \
                          toc: false\n  parsing: {dollar_math: true}\n---\nHello.\n";

/// The extension `notes`: a description, a key of `refs`, and `theorems` switched off.
const NOTES: &str = "extension: {description: Notes for a course}\nflm:\n  features:\n    \
                     refs: {unresolved: allow}\n    theorems: false\n";

#[test]
fn the_settings_state_gives_each_value_in_effect_with_its_origin() {
    let files = Shared::with("doc/paper.flm", USES_NOTES);
    files.write("ext/notes.yaml", NOTES);
    let mut request = request();
    request.extension_folders = vec![flm_host::packages::ExtensionFolder { path: String::from("ext"), required: false }];
    request.options.push(flm_host::settings::CommandLineSetting::new("text.width=72", "-O text.width=72"));
    request.math = Some(flm_host::session::GivenMath::new(
        flm_host::seed::MathChoice::processor("passthrough"),
        "the option `--math`",
    ));
    let preview = Preview::start(request, &files);
    preview.built();
    let state: Value = serde_json::from_slice(&get(preview.address(), SETTINGS).body).unwrap();
    let features = &state["in_effect"]["features"];
    // A value from a package, placed in its manifest, with the package and the extension.
    let unresolved = &features["refs"]["keys"]["unresolved"];
    assert_eq!(unresolved["value"], "allow", "{state}");
    let origin = &unresolved["origin"];
    assert_eq!((&origin["source"], &origin["file"], &origin["line"]), (&json!("package"), &json!("ext/notes.yaml"), &json!(4)), "{origin}");
    assert_eq!(origin["extension"], "notes", "{origin}");
    assert!(origin["package"].as_str().is_some_and(|package| package.starts_with("ext/notes.yaml#")), "{origin}");
    assert_eq!(origin["text"], "ext/notes.yaml:4:24", "{origin}");
    // A value from the front matter, placed in the document's file.
    let numbered = &features["math"]["keys"]["numbered_equations"];
    assert_eq!(numbered["value"], false, "{state}");
    assert_eq!(numbered["origin"], json!({ "source": "front_matter", "text": "doc/paper.flm:5:32", "file": "doc/paper.flm", "line": 5, "column": 32 }));
    // A value from the command line, with the words typed.
    let width = &features["text"]["keys"]["width"];
    assert_eq!(width["value"], 72, "{state}");
    assert_eq!(width["origin"], json!({ "source": "command_line", "text": "the option `-O text.width=72`" }));
    // A feature switched off by the front matter, and one switched off by a package.
    assert_eq!(features["toc"]["switched_off"]["source"], "front_matter", "{state}");
    assert_eq!(features["toc"]["switched_off"]["line"], 6, "{state}");
    assert_eq!(features["theorems"]["switched_off"]["source"], "package", "{state}");
    assert_eq!(features["theorems"]["keys"], json!({}), "{state}");
    assert_eq!(features["refs"]["switched_off"], Value::Null, "{state}");
    // The keys of `flm` itself.
    let flm = &state["in_effect"]["flm"];
    assert_eq!(flm["use"], json!([{ "value": "notes", "origin": { "source": "front_matter", "text": "doc/paper.flm:3:9", "file": "doc/paper.flm", "line": 3, "column": 9 } }]));
    assert_eq!(flm["math"], json!({ "value": "passthrough", "origin": { "source": "command_line", "text": "the option `--math`" } }));
    assert_eq!(flm["initial_use"]["value"], json!(["std"]), "{flm}");
    assert_eq!(flm["initial_use"]["origin"]["source"], "defaults", "{flm}");
    assert_eq!(flm["default_profile"], json!({ "value": "html", "origin": { "source": "defaults", "text": "the program's defaults" } }));
    assert_eq!(flm["parsing"]["dollar_math"]["value"], true, "{flm}");
    assert_eq!(flm["parsing"]["dollar_math"]["origin"]["line"], 7, "{flm}");
    // The choices of the environment.
    let choices = &state["choices"];
    let packages = choices["packages"].as_array().unwrap();
    assert!(packages.contains(&json!({ "name": "notes", "description": "Notes for a course" })), "{choices}");
    assert!(packages.contains(&json!({ "name": "std" })), "{choices}");
    // The packages in use, by the names that `use` gives them, each after those it uses.
    let in_use = choices["packages_in_use"].as_array().unwrap();
    for name in ["std", "math-passthrough", "notes"] {
        assert!(in_use.contains(&json!(name)), "{name}: {choices}");
    }
    assert!(choices["math_processors"].as_array().unwrap().contains(&json!("passthrough")), "{choices}");
    assert_eq!(choices["outputs"][0]["name"], "html", "{choices}");
    assert_eq!(choices["outputs"][0]["composed_as"], "html", "{choices}");
    assert!(choices["profiles"].as_array().unwrap().contains(&json!("text")), "{choices}");
    preview.stop();
}

#[test]
fn after_a_failed_preparation_the_settings_state_keeps_what_follows_from_the_packages_current() {
    let files = Shared::with("doc/paper.flm", USES_NOTES);
    files.write("ext/notes.yaml", NOTES);
    let mut request = request();
    request.extension_folders = vec![flm_host::packages::ExtensionFolder { path: String::from("ext"), required: false }];
    let preview = Preview::start(request.clone(), &files);
    preview.built();
    let state_after = |text: &str| {
        files.write("doc/paper.flm", text);
        preview.handle.build_again();
        preview.built();
        serde_json::from_slice::<Value>(&get(preview.address(), SETTINGS).body).unwrap()
    };
    let succeeded: Value = serde_json::from_slice(&get(preview.address(), SETTINGS).body).unwrap();
    assert_eq!((&succeeded["preparation_failed"], &succeeded["schema_current"]), (&Value::Null, &json!(true)));

    // A value of the wrong type, without `notes`: the schema and the packages found are
    // those of the document's packages as they are; the values in effect and the choices
    // that need the registry stay.
    let failed = state_after("---\nflm:\n  features:\n    refs: {unresolved: 3}\n---\nHello.\n");
    let reason = failed["preparation_failed"].as_str().expect("a reason");
    assert!(reason.starts_with("doc/paper.flm:"), "{reason}");
    assert_eq!(failed["schema_current"], true);
    let found = flm_host::session::document_packages(request, Arc::new(files.clone()), &[]).expect("found");
    let schema = flm_host::seed::front_matter_schema(&found.factories.iter().collect::<Vec<_>>(), &found.suggestions);
    assert_eq!(failed["schema"], schema);
    let packages = failed["choices"]["packages"].as_array().unwrap();
    assert!(packages.contains(&json!({ "name": "notes" })), "{packages:?}");
    assert_eq!(failed["in_effect"], succeeded["in_effect"]);
    for kept in ["templates", "outputs", "profiles"] {
        assert_eq!(failed["choices"][kept], succeeded["choices"][kept], "{kept}");
    }

    // A package that does not exist: the schema and the choices stay those of the last
    // successful preparation, and the schema is not current.
    let missing = state_after("---\nflm:\n  use: [nosuch]\n---\nHello.\n");
    assert!(missing["preparation_failed"].as_str().is_some_and(|reason| reason.contains("nosuch")), "{missing}");
    assert_eq!(missing["schema_current"], false);
    assert_eq!(missing["schema"], succeeded["schema"]);
    assert_eq!(missing["choices"], succeeded["choices"]);

    // A preparation that succeeds again gives all of it.
    let mended = state_after(USES_NOTES);
    assert_eq!((&mended["preparation_failed"], &mended["schema_current"]), (&Value::Null, &json!(true)));
    assert_eq!(mended["schema"], succeeded["schema"]);
    preview.stop();
}

// --- where each value comes from, and a write that keeps every other value ----------------------

/// The preview of `doc/paper.flm`, whose text is `doc`, over `files`, with the extensions
/// `extensions` (each a name and the text of its `flmextension.yaml`) in the folder `defs`,
/// once built.
fn preview_with(files: &Shared, doc: &str, extensions: &[(&str, &str)]) -> Preview {
    files.write("doc/paper.flm", doc);
    for (name, manifest) in extensions {
        files.write(&format!("defs/{name}/flmextension.yaml"), manifest);
    }
    let mut request = request();
    request.extension_folders = vec![flm_host::packages::ExtensionFolder { path: String::from("defs"), required: false }];
    let preview = Preview::start(request, files);
    preview.built();
    preview
}

/// The files of the extension `basetemplate`, whose compose template `base` for HTML
/// declares the settings `accent`, `note` and `logo` (a path), and of `mytemplate`, which
/// uses it, chooses it, and writes `accent` and its own `logo`.
fn template_files() -> Shared {
    let files = Shared::default();
    files.write(
        "defs/basetemplate/templates/html/base/flmtemplate.yaml",
        "name: base\noutput: html\nengine: minijinja\nmain: base.html\naccepts: all\nsettings:\n  type: object\n  \
         properties:\n    accent: {type: string, default: teal}\n    note: {type: string, default: ''}\n    \
         logo: {type: [string, 'null'], default: null, x-flm-marks: [path]}\n",
    );
    files.write("defs/basetemplate/templates/html/base/base.html", "{{ config.accent }} {{ regions.content }}");
    files
}

/// The manifests of `basetemplate` and `mytemplate` (see [`template_files`]).
const TEMPLATES: [(&str, &str); 2] = [
    ("basetemplate", "flm: {}\n"),
    (
        "mytemplate",
        "flm:\n  use: [basetemplate]\n  features:\n    compose:\n      template: {html: base}\n      \
         template_config:\n        html:\n          base:\n            accent: navy\n            logo: logo.svg\n",
    ),
];

/// A document that uses `mytemplate` and writes the setting `note` of `base`.
const NOTED: &str = "---\nflm:\n  use: [mytemplate]\n  features:\n    compose:\n      template_config:\n        \
                     html:\n          base:\n            note: From the document\n---\nHello.\n";

/// The answer of the server for the sources of the value at `pointer`.
fn sources(address: SocketAddr, pointer: &str) -> Value {
    let answer = get(address, &format!("{SETTINGS_SOURCES}?path={pointer}"));
    assert_eq!(answer.status, 200, "{}", String::from_utf8_lossy(&answer.body));
    serde_json::from_slice(&answer.body).unwrap()
}

#[test]
fn the_state_gives_the_origin_of_each_setting_of_a_compose_template() {
    let files = template_files();
    let preview = preview_with(&files, NOTED, &TEMPLATES);
    let state: Value = serde_json::from_slice(&get(preview.address(), SETTINGS).body).unwrap();
    let config = &state["in_effect"]["features"]["compose"]["keys"]["template_config"];
    let base = &config["entries"]["html"]["entries"]["base"]["entries"];
    let accent = &base["accent"]["origin"];
    assert_eq!((&accent["source"], &accent["extension"]), (&json!("package"), &json!("mytemplate")), "{config}");
    let note = &base["note"]["origin"];
    assert_eq!((&note["source"], &note["file"]), (&json!("front_matter"), &json!("doc/paper.flm")), "{config}");
    assert_eq!(config["value"]["html"]["base"]["logo"], "ext+mytemplate:logo.svg", "{config}");
    preview.stop();
}

#[test]
fn the_sources_of_a_value_are_listed_lowest_rank_first_with_the_one_in_effect() {
    let doc = "---\nflm:\n  use: [upper]\n  features:\n    citations: {heading_level: 4}\n---\nHello.\n";
    let lower = "flm:\n  features:\n    citations: {heading_level: 2}\n";
    let upper = "flm:\n  use: [lower]\n  features:\n    citations: {heading_level: 3}\n";
    let preview = preview_with(&Shared::default(), doc, &[("lower", lower), ("upper", upper)]);
    let answer = sources(preview.address(), "/flm/features/citations/heading_level");
    let listed: Vec<(Value, Value, Value, Value)> = answer["sources"]
        .as_array()
        .unwrap_or_else(|| panic!("{answer}"))
        .iter()
        .map(|write| (write["value"].clone(), write["origin"]["source"].clone(), write["origin"]["extension"].clone(), write["in_effect"].clone()))
        .collect();
    let expected = vec![
        (json!(1), json!("defaults"), Value::Null, json!(false)),
        (json!(2), json!("package"), json!("lower"), json!(false)),
        (json!(3), json!("package"), json!("upper"), json!(false)),
        (json!(4), json!("front_matter"), Value::Null, json!(true)),
    ];
    assert_eq!(listed, expected, "{answer}");
    assert_eq!((&answer["value"], &answer["origin"]["source"]), (&json!(4), &json!("front_matter")), "{answer}");
    preview.stop();
}

#[test]
fn a_write_inside_a_value_decided_whole_copies_the_other_values_of_the_package() {
    let pair = "flm:\n  features:\n    compose:\n      template: {html: simple, text: none}\n";
    let files = Shared::default();
    let preview = preview_with(&files, "---\nflm:\n  use: [pair]\n---\nHello.\n", &[("pair", pair)]);
    let copied = json!([{ "path": ["flm", "features", "compose", "template", "text"], "value": "none" }]);
    assert_eq!(sources(preview.address(), "/flm/features/compose/template/html")["write_copies"], copied);
    let path = json!(["flm", "features", "compose", "template", "html"]);
    let answer = change(preview.address(), json!({ "revision": revision(preview.address()), "changes": [{ "path": path, "value": "none" }] }));
    assert_eq!(answer.status, 200, "{}", String::from_utf8_lossy(&answer.body));
    assert_eq!(serde_json::from_slice::<Value>(&answer.body).unwrap(), json!({ "copied": copied }));
    let state: Value = serde_json::from_slice(&get(preview.address(), SETTINGS).body).unwrap();
    let written = json!({ "use": ["pair"], "features": { "compose": { "template": { "html": "none", "text": "none" } } } });
    assert_eq!(state["front_matter"]["flm"], written, "{state}");
    preview.stop();
}

#[test]
fn a_write_of_one_setting_of_a_compose_template_copies_nothing() {
    let files = template_files();
    let preview = preview_with(&files, NOTED, &TEMPLATES);
    assert_eq!(sources(preview.address(), "/flm/features/compose/template_config/html/base/accent")["write_copies"], json!([]));
    let path = json!(["flm", "features", "compose", "template_config", "html", "base", "accent"]);
    let answer = change(preview.address(), json!({ "revision": revision(preview.address()), "changes": [{ "path": path, "value": "red" }] }));
    assert_eq!(serde_json::from_slice::<Value>(&answer.body).unwrap(), json!({ "copied": [] }));
    let state: Value = serde_json::from_slice(&get(preview.address(), SETTINGS).body).unwrap();
    let base = json!({ "note": "From the document", "accent": "red" });
    assert_eq!(state["front_matter"]["flm"]["features"]["compose"]["template_config"]["html"]["base"], base, "{state}");
    preview.stop();
}

#[test]
fn after_a_failed_preparation_a_write_that_would_copy_a_value_is_refused() {
    let pair = "flm:\n  features:\n    compose:\n      template: {html: simple, text: none}\n";
    let files = Shared::default();
    let preview = preview_with(&files, "---\nflm:\n  use: [pair]\n---\nHello.\n", &[("pair", pair)]);
    let html = json!(["flm", "features", "compose", "template", "html"]);
    // The status of the answer to `edit` once the front matter writes `template` too, with
    // a value of the wrong type so that the preparation fails, and whether the file changed.
    let answer_with = |template: &str, edit: Value| {
        let text = format!("---\nflm:\n  use: [pair]\n  features:\n    refs: {{unresolved: 3}}\n{template}---\nHello.\n");
        files.write("doc/paper.flm", &text);
        preview.handle.build_again();
        preview.built();
        let address = preview.address();
        let answer = change(address, json!({ "revision": revision(address), "changes": [edit] }));
        let written = files.read("doc/paper.flm").unwrap() != text.as_bytes();
        (answer.status, written)
    };
    let set = json!({ "path": html, "value": "none" });
    let remove = json!({ "path": html, "remove": true });
    // The package gives the names.
    assert_eq!(answer_with("", set.clone()), (409, false));
    // The front matter writes a single name.
    let single = "    compose: {template: simple}\n";
    assert_eq!(answer_with(single, set.clone()), (409, false));
    assert_eq!(answer_with(single, remove), (409, false));
    // The front matter writes a map: nothing is copied.
    assert_eq!(answer_with("    compose: {template: {html: simple, text: none}}\n", set), (200, true));
    preview.stop();
}

/// The extension `@acme/notes`: a description, parsing keys and keys of
/// features, and `theorems` switched off.
const SCOPED: &str = "extension: {description: Notes of Acme}\nflm:\n  parsing: {dollar_math: true}\n  features:\n    \
                      refs: {unresolved: allow}\n    theorems: false\n";

/// An extension `notes` that the document does not use, with a description of its own.
const UNUSED: &str = "extension: {description: Notes of the folder}\n";

#[test]
fn a_package_s_description_and_values_are_its_own() {
    let files = Shared::with("doc/paper.flm", "---\nflm:\n  use: ['@acme/notes']\n---\nHello.\n");
    files.write("ext/@acme/notes.yaml", SCOPED);
    files.write("ext/notes.yaml", UNUSED);
    let mut request = request();
    request.extension_folders = vec![flm_host::packages::ExtensionFolder { path: String::from("ext"), required: false }];
    let preview = Preview::start(request, &files);
    preview.built();
    // The description goes with the extension in use, never with another of the same stem.
    let state: Value = serde_json::from_slice(&get(preview.address(), SETTINGS).body).unwrap();
    let packages = state["choices"]["packages"].as_array().unwrap();
    assert!(packages.contains(&json!({ "name": "@acme/notes", "description": "Notes of Acme" })), "{packages:?}");
    assert!(packages.contains(&json!({ "name": "notes" })), "{packages:?}");
    // Where the front matter writes nothing, the value as YAML of a feature and of the
    // parsing keys is what the package writes.
    let yaml = |path: &str| {
        let answer = get(preview.address(), &format!("{SETTINGS_YAML}?path={path}"));
        assert_eq!(answer.status, 200, "{path}");
        String::from_utf8(answer.body).unwrap()
    };
    assert_eq!(yaml("/flm/features/refs"), "unresolved: allow\n");
    assert_eq!(yaml("/flm/features/theorems"), "enabled: false\n");
    assert_eq!(yaml("/flm/parsing"), "dollar_math: true\n");
    preview.stop();
}

#[test]
fn a_front_matter_that_cannot_be_read_gives_its_reason_in_the_state() {
    let files = Shared::with("doc/paper.flm", SET);
    let preview = Preview::start(request(), &files);
    preview.built();
    let before: Value = serde_json::from_slice(&get(preview.address(), SETTINGS).body).unwrap();
    files.write("doc/paper.flm", "---\ntitle: [unclosed\n---\nHello.\n");
    preview.handle.build_again();
    preview.built();
    let answer = get(preview.address(), SETTINGS);
    assert_eq!(answer.status, 200);
    let state: Value = serde_json::from_slice(&answer.body).unwrap();
    assert_eq!((&state["revision"], &state["front_matter"]), (&Value::Null, &Value::Null), "{state}");
    assert!(state["unreadable"].as_str().is_some_and(|reason| reason.starts_with("doc/paper.flm:3:1: ")), "{state}");
    // What the last successful preparation gave stays.
    assert_eq!(state["in_effect"], before["in_effect"]);
    assert_eq!(state["choices"], before["choices"]);
    preview.stop();
}

#[test]
fn a_change_the_server_cannot_make_is_refused_with_its_reason_and_writes_nothing() {
    let files = Shared::with("doc/paper.flm", SET);
    let preview = Preview::start(request(), &files);
    preview.built();
    let text = || String::from_utf8(files.read("doc/paper.flm").unwrap()).unwrap();
    let revision = revision(preview.address());
    let refused = |body: Value, status: u16| {
        let answer = change(preview.address(), body.clone());
        let reason = String::from_utf8(answer.body).unwrap();
        assert_eq!(answer.status, status, "{body}: {reason}");
        assert!(!reason.is_empty() && !reason.contains('\n'), "{body}: {reason}");
        assert_eq!(text(), SET, "{body}");
    };
    // A revision that is not the front matter's.
    refused(json!({ "revision": "0000000000000000", "changes": [{ "path": ["title"], "value": "Lecture notes" }] }), 412);
    // The two kinds of value that are never written.
    refused(json!({ "revision": revision, "changes": [{ "path": ["title"], "value": "Two\nlines" }] }), 409);
    refused(json!({ "revision": revision, "changes": [{ "path": ["flm", "features", "toc"], "yaml": "entries:\n  - {depth: 2}\n" }] }), 409);
    // Two changes whose second is refused: the first is not written either.
    let changes = json!([{ "path": ["flm", "layout"], "value": "blocks" }, { "path": ["title", "subtitle"], "value": "x" }]);
    refused(json!({ "revision": revision, "changes": changes }), 409);
    // Bodies that are not a change.
    refused(json!([1]), 400);
    assert_eq!(send(preview.address(), "POST", FRONT_MATTER, &[], b"not JSON").status, 400);
    // A front matter that is not YAML: the reader's error, placed in the file.
    let broken = "---\ntitle: [unclosed\n---\nHello.\n";
    files.write("doc/paper.flm", broken);
    let answer = change(preview.address(), json!({ "revision": revision, "changes": [{ "path": ["title"], "value": 1 }] }));
    let reason = String::from_utf8(answer.body).unwrap();
    assert_eq!(answer.status, 409, "{reason}");
    assert!(reason.starts_with("doc/paper.flm:3:1: "), "{reason}");
    assert_eq!(text(), broken);
    // The front matter is changed by a `POST` alone.
    assert_eq!(ask(preview.address(), "PUT", FRONT_MATTER).status, 405);
    preview.stop();
}

#[test]
fn a_change_the_file_cannot_take_is_refused_with_its_reason_and_the_file_is_untouched() {
    let files = Shared::with("doc/paper.flm", SET);
    let failing = DocumentSettings::new(schema(), "doc/paper.flm", Arc::new(files.clone()), |_, _| {
        Err(OutputError { path: Some("doc/paper.flm".into()), detail: "the disk is full".into() })
    });
    let preview = Preview::start_with(request(), &files, failing);
    preview.built();
    let body = json!({ "revision": revision(preview.address()), "changes": [{ "path": ["flm", "features", "toc"], "value": false }] });
    let answer = change(preview.address(), body);
    let reason = String::from_utf8(answer.body).unwrap();
    assert_eq!(answer.status, 500, "{reason}");
    assert!(reason.contains("the disk is full"), "{reason}");
    assert_eq!(String::from_utf8(files.read("doc/paper.flm").unwrap()).unwrap(), SET);
    preview.stop();
}

#[test]
fn a_request_that_acts_from_the_servers_port_under_another_name_is_refused_and_does_nothing() {
    let files = Shared::with("doc/paper.flm", SET);
    let preview = Preview::start(request(), &files);
    preview.built();
    let own = format!("http://{}", preview.address());
    let other_name = format!("http://localhost:{}", preview.address().port());
    for target in [REBUILD, OPEN, FRONT_MATTER] {
        let answer = send(preview.address(), "POST", target, &[("Origin", &other_name)], b"{}");
        let reason = String::from_utf8(answer.body).unwrap();
        assert_eq!(answer.status, 403, "{target}: {reason}");
    }
    assert_eq!(String::from_utf8(files.read("doc/paper.flm").unwrap()).unwrap(), SET);
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
        for target in [REBUILD, OPEN, FRONT_MATTER] {
            let answer = send(preview.address(), "POST", target, &[("Origin", origin)], b"{}");
            let reason = String::from_utf8(answer.body).unwrap();
            assert_eq!(answer.status, 403, "{origin} {target}: {reason}");
        }
    }
    assert_eq!(String::from_utf8(files.read("doc/paper.flm").unwrap()).unwrap(), SET);
    // A page of the server's own is honored, and the refused requests to build again built
    // nothing: the build that follows is the second.
    assert_eq!(send(preview.address(), "POST", REBUILD, &[("Origin", &own)], b"").status, 204);
    assert_eq!(preview.built().0, 2);
    let body = json!({ "revision": revision(preview.address()), "changes": [{ "path": ["flm", "layout"], "value": "blocks" }] }).to_string();
    assert_eq!(send(preview.address(), "POST", FRONT_MATTER, &[("Origin", &own)], body.as_bytes()).status, 200);
    assert!(String::from_utf8(files.read("doc/paper.flm").unwrap()).unwrap().contains("  layout: blocks\n"));
    // A page that reads is not checked: the browser keeps the answer from a page of another
    // origin.
    assert_eq!(send(preview.address(), "GET", SETTINGS, &[("Origin", "http://example.com")], b"").status, 200);
    preview.stop();
}
