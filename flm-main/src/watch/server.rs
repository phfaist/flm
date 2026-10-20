//! The preview's server: serves the output of the last build from memory to a browser on
//! this machine, and sends a stream of messages to every open page.
//!
//! The live preview shows the document in a browser. A [`Server`] is the web server it
//! uses: the caller starts it ([`Server::start`]), hands it each event of the watch loop
//! ([`Server::tell`]), and the server keeps the output of the last build, serves it, and
//! tells every open page what changed. The server listens on this machine's loopback
//! address only (`127.0.0.1`), at a port its caller chooses or at the first free one from
//! [`DEFAULT_PORT`] on ([`Port`]). It stops when it is dropped.
//!
//! **What it serves.**
//!
//! - **The output** that the watch loop holds ([`Built::output`](super::Built::output)):
//!   each of its named files at its path, the entry file (the file a reader opens first,
//!   [`BuildOutput::entry`]) also at `/`, with its media type and its bytes exactly as
//!   built. Nothing is read from the disk: an address whose path, once percent-decoded, is
//!   not exactly the path of a file of the output is answered with `404`.
//!   Before a build has given an output, `/` is a page of the server's own, which says
//!   that the preview is waiting for the document; it carries the page script (the
//!   preview's script that keeps a page up to date, declared by the feature
//!   [`preview`](crate::stdpkg::preview)), which lays the toolbar over it with the last
//!   build's problems and replaces it by the document's page once a build gives one.
//! - **Its own addresses**, under the prefix [`PREFIX`]: [`EVENTS`], the stream of
//!   messages; [`REBUILD`], an HTTP request to build again now, which the server passes to the
//!   watch loop's [`Handle::build_again`](super::Handle::build_again); [`OPEN`], a request
//!   to open a place of the document's files in the editor (below); and [`SETTINGS`], the
//!   settings of the document's features, read and saved for the toolbar's *Settings* tool
//!   (below).
//!
//! Every answer tells the browser not to keep it (`Cache-Control: no-store`), so that a page
//! loaded anew shows the last build, and closes its connection, so that a page left open
//! while the server was stopped and started again in the same program connects anew.
//!
//! **Only the preview's own page can act.** An HTTP request that changes something (every
//! `POST`: build again, open a place, save the settings) is carried out only when it comes
//! from a page that the server itself served, or from no page at all. For each `POST`, a
//! browser sends the origin of the page that sends it (its scheme, its host and its port)
//! in the header `Origin`, and no page can change that header. The server compares it with
//! its own origin, `http://127.0.0.1:PORT`, and, when they differ, answers `403` with one
//! line of text and does nothing. A page of another site open in the same browser can send
//! a request to any address of this machine, but it therefore cannot make the preview
//! build, start an editor or write the document's file. A request without `Origin` comes
//! from no browser page but from a program of this machine, which could do as much
//! without the server; it is carried out. A request that only reads (`GET`) is not
//! checked: the browser does not let a page of another origin read the answer.
//!
//! **The stream of messages** is a stream of server-sent events, a web standard that a
//! browser reads with `EventSource` and reconnects to by itself when the connection is
//! lost. Each message is one event with a name and one line of JSON:
//!
//! | Event | When | Its JSON |
//! |---|---|---|
//! | `hello` | first, on every connection | `{"server", "document", "build", "building", "problems", "references", "editors", "editor"}` |
//! | `building` | a build starts | `{"changed": [paths]}` |
//! | `update` | a build is done | the update, as [`Update::to_json`](super::update::Update::to_json) gives it |
//!
//! In `hello`, `server` identifies this server, so that a page can tell that the server was
//! started anew; `document` is the document's name that the caller gave to [`Server::start`]; `build`
//! is the number of the build whose output the server holds (`null` before the first
//! output); `building` says whether a build is under way; `problems` are the problems of
//! the last build, in the update's form; `references` are the labels of the elements of
//! the output the server holds, in the update's form (`null` before the first output);
//! `editors` are the editors found on this machine that a place can be opened in, which
//! the caller gave to [`Server::start`] ([`Editors`]), in their order, each
//! `{"name": NAME, "label": LABEL}` (the name of its program, and its name for a menu:
//! [`Editor::label`]); and `editor` is the name of the default, the editor a place is
//! opened in when the page asks for none (`null` when no editor was found). A
//! page that shows another build than the one the server holds loads itself anew. A page
//! learns which server and which build it shows from the answer that carried it: every
//! answer that carries a file of an output identifies them in its header `Server-Timing`
//! (the metrics `flm-server` and `flm-build`), which a page reads in the browser; the
//! bytes of the answer stay the build's.
//!
//! **The settings of the document.** A `GET` of [`SETTINGS`] answers one JSON object,
//! `{"schema": SCHEMA, "features": FEATURES}`: the schema of a front matter for the
//! packages of the latest successful preparation (the schema the server was started with,
//! that of the compiled packages, until the first; [`Server::tell`] replaces it), and what the document's front matter holds under
//! `flm.features` now (`null` when it holds nothing there), read from the document's file
//! at that moment ([`DocumentSettings`]). A `POST` of [`SETTINGS`] whose body is one JSON
//! object, the new map of features, writes the document's file with its front matter's
//! `flm.features` holding that map, edited in place
//! ([`super::front_matter::edit_features`]), and answers `204`; the build that follows is
//! the watch loop's, which sees the file change. When either request cannot be carried
//! out, the answer is a status and one line of plain text that the page shows: `400` for
//! a body that is not a JSON object, or that is longer than the server accepts (4 MiB);
//! `409` for a front matter that cannot be read (for example because it is not valid YAML
//! while the author is editing it) or that the settings cannot be written into exactly as
//! it is laid out (nothing is written in either case); and `500` for a file that cannot
//! be read or written. The server reads and writes this one file, the document's, and
//! serves nothing else from the disk.
//!
//! **Opening a place in the editor.** A page asks for a place of the document's files to be
//! opened in an editor (the place of an element the author clicked, or of a problem in
//! the toolbar's list) with a `POST` to [`OPEN`] whose body is one JSON object,
//! `{"file": NAME, "start": OFFSET, "editor": EDITOR}`: the name of a source and a byte
//! offset into it, as the page's elements and the update's problems carry them, and, when
//! the page asks for one, the name of the editor to open (one of the `editors` of `hello`;
//! the field may be left out). The server opens only a file
//! that is a source of the build whose output it serves ([`Places`]) and that the last
//! build read, turns the offset into a line and a column itself, from those sources, and
//! starts the editor asked for, or the default when the page asks for none, on the file at
//! that line and column; the editors are those its caller gave ([`Editors`]). It answers
//! `204` once the editor has started; otherwise a status and one line of plain text that
//! says why nothing was opened, which the page shows: `400` for a body that is not such an
//! object or that is longer than the server accepts (64 KiB), or for an editor that is not
//! one of the editors found; `403` for a file that is not such a source, `404` for an
//! offset past the end of the file, `503` when no editor was found at all, and `500` when
//! the editor could not be started.
//!
//! The server reports the outcome of each request to open a place, as an [`Opening`], to
//! the function given to [`Server::start`]: the file, the line, the column and the editor
//! opened, or the text the page received when nothing was opened; and, when an editor that
//! was started later ends with failure, how it ended and what it wrote on its standard
//! error. The server answers the page as soon as the editor has started, and waits for the
//! editor's end afterwards, on the request's own thread. The server prints nothing; its
//! caller decides how to show these outcomes (the `flm` program prints them in its
//! terminal).
//!
//! **Threads.** The server accepts connections on a thread of its own and answers each
//! request on a new thread. It keeps what it needs of each event it is told and sends the
//! messages; each open stream is written on its own thread, so a slow or closed page holds
//! up neither the other pages nor the watch loop. A stream whose page went away is dropped
//! at the next message.

use std::fmt;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};

use flm_core::stdcore::backends::html::escape::escape_text_into;
use serde_json::{Value, json};
use tiny_http::{Method, Request};

use super::editor::{Editor, EditorFailure, Editors, Places, Started};
use super::settings::{DocumentSettings, SettingsError};
use super::update::{BuildOutput, PageChange};
use super::{Event, Handle};
use crate::stdpkg::preview::{SCRIPT_TEXT, STYLE_TEXT};

/// The first port that the default [`Port`] (`Port::From(DEFAULT_PORT)`) tries: 18910, the
/// port of the preview of FLM's Python implementation.
pub const DEFAULT_PORT: u16 = 18910;

/// How many ports after the first one the server tries, one after the other, under
/// [`Port::From`] when the first one is taken: 64.
pub const FURTHER_PORTS: u16 = 64;

/// The prefix of the server's own addresses, `/.flm-preview/`. An address under this
/// prefix is always answered as one of the server's own addresses, never with a file of
/// the output.
pub const PREFIX: &str = "/.flm-preview/";

/// The address of the stream of messages, `/.flm-preview/events`.
pub const EVENTS: &str = "/.flm-preview/events";

/// The address of a request to build the document again now, `/.flm-preview/rebuild`,
/// with the method `POST`.
pub const REBUILD: &str = "/.flm-preview/rebuild";

/// The address of a request to open a place in the editor, `/.flm-preview/open`, with the
/// method `POST` (see the [module documentation](self), "Opening a place in the editor").
pub const OPEN: &str = "/.flm-preview/open";

/// The address of the settings of the document's features, `/.flm-preview/settings`: `GET`
/// reads them and `POST` saves them (see the [module documentation](self), "The settings
/// of the document").
pub const SETTINGS: &str = "/.flm-preview/settings";

/// The largest body, in bytes, of a request to [`OPEN`]; a longer body is answered with
/// `400`.
const OPEN_BODY_LIMIT: u64 = 64 * 1024;

/// The largest body, in bytes, of a request to save the settings; a longer body is
/// answered with `400`.
const SETTINGS_BODY_LIMIT: u64 = 4 * 1024 * 1024;

/// The port the server listens at, as its caller chooses it for [`Server::start`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Port {
    /// The first free port from this one on, trying up to [`FURTHER_PORTS`] more after it.
    From(u16),
    /// This port, and no other.
    Exactly(u16),
}

impl Default for Port {
    /// The first free port from [`DEFAULT_PORT`] on.
    fn default() -> Port {
        Port::From(DEFAULT_PORT)
    }
}

/// The error of [`Server::start`]: why the server could not start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServeError {
    /// The port chosen with [`Port::Exactly`] cannot be listened at: it is taken, or the
    /// system refuses it.
    Port {
        /// The port.
        port: u16,
        /// What the system said.
        detail: String,
    },
    /// None of the ports tried under [`Port::From`] is free.
    NoFreePort {
        /// The first port tried.
        first: u16,
        /// The last port tried.
        last: u16,
    },
}

impl fmt::Display for ServeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServeError::Port { port, detail } => {
                write!(f, "the preview's server cannot listen at port {port} of this machine: {detail}")
            }
            ServeError::NoFreePort { first, last } => {
                write!(f, "no port from {first} to {last} of this machine is free")
            }
        }
    }
}

impl std::error::Error for ServeError {}

/// The outcome of a request to open a place in the editor, which the server reports to the
/// function given to [`Server::start`] (see the [module documentation](self), "Opening a
/// place in the editor").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Opening {
    /// The editor was started on the file at the line and the column, both counted from 1,
    /// the column in characters.
    Opened {
        /// The editor's name, one of [`EDITORS`](super::editor::EDITORS).
        editor: &'static str,
        /// The file, as the editor was given it.
        file: PathBuf,
        /// The line.
        line: usize,
        /// The column.
        column: usize,
    },
    /// Nothing was opened: `reason` is the text the server answered to the page, which the
    /// page shows.
    Refused {
        /// Why nothing was opened.
        reason: String,
    },
    /// An editor that was started ([`Opening::Opened`]) ended with failure.
    Failed(EditorFailure),
}

/// The function to which the server reports each [`Opening`]; it is called on the server's
/// own threads.
type Openings = Box<dyn Fn(Opening) + Send + Sync>;

/// The web server of the live preview: see the [module documentation](self).
///
/// Start it with [`Server::start`], give it each event of the watch loop with
/// [`Server::tell`], and open [`Server::url`] in a browser. Dropping it stops it.
pub struct Server {
    http: Arc<tiny_http::Server>,
    address: SocketAddr,
    shared: Arc<Shared>,
    stopping: Arc<AtomicBool>,
    accepting: Option<JoinHandle<()>>,
}

impl fmt::Debug for Server {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Server").field("address", &self.address).finish_non_exhaustive()
    }
}

/// What the threads of the server share.
struct Shared {
    /// The identifier of this server, sent to every page.
    server: String,
    /// The origin of the pages this server serves, `http://127.0.0.1:PORT`: the only origin
    /// whose `POST` requests are carried out.
    origin: String,
    /// The document's name, as the caller gave it.
    document: String,
    /// The loop's handle, for a request to build again now.
    handle: Handle,
    /// The editors a place can be opened in, and the default.
    editors: Editors,
    /// The function to which the outcome of each request to open a place is reported.
    openings: Openings,
    /// The schema of a front matter, and the document's file, for the *Settings* tool.
    settings: DocumentSettings,
    /// What the server holds of the last builds, and the open streams.
    state: Mutex<State>,
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// What the server holds of the last builds.
#[derive(Default)]
struct State {
    /// The output now current, and the number of the build that gave it.
    output: Option<(u64, Arc<BuildOutput>)>,
    /// Whether a build is under way.
    building: bool,
    /// The problems of the last build, in the update's form.
    problems: Vec<Value>,
    /// The places of the output served ([`super::Built::places`]).
    places: Option<Arc<Places>>,
    /// The files the last build attempt read and found: the only files the editor is
    /// opened on.
    read: Vec<PathBuf>,
    /// The open streams: each takes the messages, framed, for its own thread to write.
    clients: Vec<Sender<String>>,
}

impl State {
    /// Sends `message`, framed, to every open stream, and drops the streams that closed.
    fn send(&mut self, message: &str) {
        self.clients.retain(|client| client.send(message.to_owned()).is_ok());
    }
}

impl Server {
    /// Starts the server, listening at `port`, for the document `document` (the name of the
    /// document's file as the caller wants pages to show it; the server sends it to the
    /// pages in `hello` and writes it into the waiting page). An HTTP request to build again
    /// now is passed to `handle`. A place to open is opened in one of `editors`, the editors
    /// found on this machine ([`Editors::find`]): the one the page asks for, or the default;
    /// the server lists them to the pages in `hello`. The outcome of each such request is
    /// reported to `openings` ([`Opening`]). The *Settings* tool's requests are answered
    /// through `settings`. The server serves no output until it is told that a build gave
    /// one ([`Server::tell`]).
    ///
    /// # Errors
    ///
    /// Returns a [`ServeError`] when no port can be listened at.
    pub fn start(
        port: Port,
        document: impl Into<String>,
        handle: Handle,
        editors: Editors,
        openings: impl Fn(Opening) + Send + Sync + 'static,
        settings: DocumentSettings,
    ) -> Result<Server, ServeError> {
        let (http, address) = listen(port)?;
        let http = Arc::new(http);
        let shared = Arc::new(Shared {
            server: server_name(address),
            origin: format!("http://{address}"),
            document: document.into(),
            handle,
            editors,
            openings: Box::new(openings),
            settings,
            state: Mutex::new(State::default()),
        });
        let stopping = Arc::new(AtomicBool::new(false));
        let accepting = {
            let (http, shared, stopping) = (Arc::clone(&http), Arc::clone(&shared), Arc::clone(&stopping));
            thread::spawn(move || {
                loop {
                    match http.recv() {
                        Ok(request) => {
                            let shared = Arc::clone(&shared);
                            thread::spawn(move || answer(&shared, request));
                        }
                        Err(_) if stopping.load(Ordering::SeqCst) => return,
                        // A connection that failed before it made a request concerns that
                        // connection alone.
                        Err(_) => {}
                    }
                }
            })
        };
        Ok(Server { http, address, shared, stopping, accepting: Some(accepting) })
    }

    /// Returns the address and the port the server listens at, on this machine's loopback
    /// address.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Returns the address of the preview's page, `http://127.0.0.1:PORT/`, which a browser
    /// opens.
    pub fn url(&self) -> String {
        format!("http://{}/", self.address)
    }

    /// Hands the server an event of the watch loop. A build starting is sent to every page
    /// (`building`). A build done makes its output the one served (or, when it gave no
    /// output, keeps the last output a build gave), and its update is sent to every page
    /// (`update`); a build that prepared the document again first makes the schema of that
    /// preparation ([`Preparation::settings_schema`](super::Preparation::settings_schema))
    /// the one the *Settings* tool reads. A problem of the watcher is not passed to the
    /// pages; showing it is left to the caller.
    pub fn tell(&self, event: &Event) {
        if let Event::Built(built) = event
            && let Some(prepared) = &built.prepared
        {
            self.shared.settings.set_schema(Arc::clone(&prepared.settings_schema));
        }
        let mut state = self.shared.state();
        match event {
            Event::Building { changed } => {
                state.building = true;
                let changed: Vec<String> = changed.iter().map(|path| path.display().to_string()).collect();
                state.send(&frame("building", &json!({ "changed": changed })));
            }
            Event::Built(built) => {
                state.building = false;
                let update = built.update.to_json();
                state.problems = update["problems"].as_array().cloned().unwrap_or_default();
                state.read = built.files_read.iter().filter(|read| read.found).map(|read| read.path.clone()).collect();
                if built.update.page != PageChange::Absent {
                    state.output = built.output.clone().map(|output| (built.update.build, output));
                    state.places = built.places.clone();
                }
                state.send(&frame("update", &update));
            }
            Event::Watcher(_) => {}
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::SeqCst);
        self.http.unblock();
        if let Some(accepting) = self.accepting.take() {
            let _ = accepting.join();
        }
        // Each stream's thread ends once its sender is gone, closing its connection.
        self.shared.state().clients.clear();
    }
}

/// Listens on the loopback address at `port`, and gives the address listened at.
fn listen(port: Port) -> Result<(tiny_http::Server, SocketAddr), ServeError> {
    match port {
        Port::Exactly(port) => listen_at(port).map_err(|detail| ServeError::Port { port, detail }),
        Port::From(first) => {
            let last = first.saturating_add(FURTHER_PORTS);
            (first..=last).find_map(|port| listen_at(port).ok()).ok_or(ServeError::NoFreePort { first, last })
        }
    }
}

/// Listens at `port` of the loopback address (0: a port the system chooses), and gives the
/// address listened at; the system's words when it refuses.
fn listen_at(port: u16) -> Result<(tiny_http::Server, SocketAddr), String> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, port)).map_err(|error| error.to_string())?;
    let address = listener.local_addr().map_err(|error| error.to_string())?;
    let http = tiny_http::Server::from_listener(listener, None).map_err(|error| error.to_string())?;
    Ok((http, address))
}

/// The identifier of a server listening at `address`, started now: it tells a page that the
/// server it connects to is not the one that served it.
fn server_name(address: SocketAddr) -> String {
    let started = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos())
        .unwrap_or_default();
    format!("{}-{}-{started:x}", std::process::id(), address.port())
}

/// One server-sent event: its name, and its data on one line.
fn frame(event: &str, data: &Value) -> String {
    format!("event: {event}\ndata: {data}\n\n")
}

/// The value of the header `Content-Type` for the media type `media_type`, with the
/// character set UTF-8 added to a text type that has none: every text a build writes is
/// UTF-8. `None` for a media type that cannot be written in a header.
fn content_type(media_type: &str) -> Option<String> {
    if !media_type.chars().all(|character| character.is_ascii_graphic() || character == ' ') {
        return None;
    }
    if media_type.starts_with("text/") && !media_type.contains("charset") {
        Some(format!("{media_type}; charset=utf-8"))
    } else {
        Some(media_type.to_owned())
    }
}

/// The words that follow the status `status` in an answer's first line.
fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        _ => "",
    }
}

/// Answers `request` with the status `status` and the body `body` of the media type
/// `media_type` (no body for `HEAD`), not to be kept by the browser, and closes the
/// connection after it; `served_by`, when given, is the value of the header
/// `Server-Timing` ([`served_by`]).
///
/// The answer is written by hand, as the stream's head is, to say `Connection: close`,
/// which `tiny_http` does not let an answer say, so that a browser never sends a request on
/// a connection a dropped server keeps without answering.
// See decision rationale at [§flm-dd-dr:host-watch-answers].
fn reply(request: Request, status: u16, media_type: Option<&str>, served_by: Option<&str>, body: &[u8]) {
    let head_only = *request.method() == Method::Head;
    let mut head = format!("HTTP/1.1 {status} {}\r\nCache-Control: no-store\r\nConnection: close\r\n", reason(status));
    if status != 204 {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    if let Some(value) = media_type.and_then(content_type) {
        head.push_str(&format!("Content-Type: {value}\r\n"));
    }
    if let Some(value) = served_by {
        head.push_str(&format!("Server-Timing: {value}\r\n"));
    }
    head.push_str("\r\n");
    let mut writer = request.into_writer();
    let _ = writer
        .write_all(head.as_bytes())
        .and_then(|()| if head_only { Ok(()) } else { writer.write_all(body) })
        .and_then(|()| writer.flush());
}

/// Answers `request`: an address of the server's own, a file of the output the server
/// holds (with the header `Server-Timing` identifying the server and the build:
/// [`served_by`]), the waiting page at `/` before any output, or "not found".
fn answer(shared: &Arc<Shared>, request: Request) {
    // The path alone: a query or a fragment names nothing the server serves.
    let path = request.url().split(['?', '#']).next().unwrap_or_default().to_owned();
    let method = request.method().clone();
    if path.starts_with(PREFIX) {
        if method == Method::Post && !from_own_page(shared, &request) {
            let reason = format!("this request must come from the preview's page at {}/", shared.origin);
            reply(request, 403, Some("text/plain"), None, reason.as_bytes());
            return;
        }
        match (path.as_str(), method) {
            (EVENTS, Method::Get) => stream(shared, request),
            (REBUILD, Method::Post) => {
                shared.handle.build_again();
                reply(request, 204, None, None, b"");
            }
            (OPEN, Method::Post) => open(shared, request),
            (SETTINGS, Method::Get) => read_settings(shared, request),
            (SETTINGS, Method::Post) => save_settings(shared, request),
            (EVENTS | REBUILD | OPEN | SETTINGS, _) => reply(request, 405, None, None, b""),
            _ => reply(request, 404, None, None, b""),
        }
        return;
    }
    if !matches!(method, Method::Get | Method::Head) {
        reply(request, 405, None, None, b"");
        return;
    }
    let output = shared.state().output.as_ref().map(|(build, output)| (*build, Arc::clone(output)));
    let Some(name) = file_name(&path) else {
        reply(request, 404, None, None, b"");
        return;
    };
    match (&output, name.as_str()) {
        (Some((build, output)), name) => {
            let file = if name.is_empty() { output.entry() } else { output.files.get(name) };
            match file {
                Some(file) => {
                    let served_by = served_by(&shared.server, *build);
                    reply(request, 200, Some(&file.media_type), Some(&served_by), &file.bytes)
                }
                None => reply(request, 404, None, None, b""),
            }
        }
        (None, "") => reply(request, 200, Some("text/html"), None, waiting_page(&shared.document).as_bytes()),
        (None, _) => reply(request, 404, None, None, b""),
    }
}

/// Whether `request` may change something: it carries no `Origin` header, or the origin of
/// the pages this server serves (see the [module documentation](self), "Only the preview's
/// own page can act").
// See decision rationale at [§flm-dd-dr:host-watch-address].
fn from_own_page(shared: &Shared, request: &Request) -> bool {
    request
        .headers()
        .iter()
        .filter(|header| header.field.equiv("Origin"))
        .all(|header| header.value.as_str() == shared.origin)
}

/// Answers a request to read the document's settings: see the [module documentation](self).
fn read_settings(shared: &Shared, request: Request) {
    match shared.settings.features() {
        Ok(features) => {
            let body = json!({ "schema": shared.settings.schema().as_ref(), "features": features }).to_string();
            reply(request, 200, Some("application/json"), None, body.as_bytes());
        }
        Err(error) => reply(request, settings_status(&error), Some("text/plain"), None, error.to_string().as_bytes()),
    }
}

/// Answers a request to save the document's settings: see the [module documentation](self).
fn save_settings(shared: &Shared, mut request: Request) {
    let mut body = Vec::new();
    let read = request.as_reader().take(SETTINGS_BODY_LIMIT + 1).read_to_end(&mut body);
    let features = read
        .ok()
        .filter(|_| body.len() as u64 <= SETTINGS_BODY_LIMIT)
        .and_then(|_| serde_json::from_slice::<Value>(&body).ok());
    let Some(Value::Object(features)) = features else {
        let reason = "the settings to save must be a JSON object of at most 4 MiB that maps each feature's name to its settings";
        reply(request, 400, Some("text/plain"), None, reason.as_bytes());
        return;
    };
    match shared.settings.save(&features) {
        Ok(()) => reply(request, 204, None, None, b""),
        Err(error) => reply(request, settings_status(&error), Some("text/plain"), None, error.to_string().as_bytes()),
    }
}

/// The status of an answer that says why the settings could not be read or saved.
fn settings_status(error: &SettingsError) -> u16 {
    match error {
        SettingsError::FrontMatter(_) => 409,
        SettingsError::File(_) => 500,
    }
}

/// Answers a request to open a place in the editor: see the [module documentation](self).
fn open(shared: &Shared, mut request: Request) {
    let mut body = Vec::new();
    let read = request.as_reader().take(OPEN_BODY_LIMIT + 1).read_to_end(&mut body);
    let place = read.ok().filter(|_| body.len() as u64 <= OPEN_BODY_LIMIT).and_then(|_| place_of(&body));
    let answer = match place {
        Some((file, start, editor)) => open_place(shared, &file, start, editor.as_deref()),
        None => Err((
            400,
            String::from(
                "the place to open must be a JSON object of at most 64 KiB with a file name (`file`), a byte offset \
                 (`start`) and, optionally, the name of an editor (`editor`)",
            ),
        )),
    };
    match answer {
        Ok((started, opened)) => {
            reply(request, 204, None, None, b"");
            (shared.openings)(opened);
            if let Err(failure) = started.wait() {
                (shared.openings)(Opening::Failed(failure));
            }
        }
        Err((status, reason)) => {
            reply(request, status, Some("text/plain"), None, reason.as_bytes());
            (shared.openings)(Opening::Refused { reason });
        }
    }
}

/// The place a request's body gives, and the editor it asks for: `{"file": NAME, "start":
/// OFFSET, "editor": EDITOR}`, the offset a whole number, the editor's name a text that may
/// be left out; `None` when the body is not that.
fn place_of(body: &[u8]) -> Option<(String, usize, Option<String>)> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let file = value.get("file")?.as_str()?.to_owned();
    let start = usize::try_from(value.get("start")?.as_u64()?).ok()?;
    let editor = match value.get("editor") {
        Some(editor) => Some(editor.as_str()?.to_owned()),
        None => None,
    };
    Some((file, start, editor))
}

/// Opens the byte `start` of the source named `file` in the editor named `asked`, or in
/// the default when `asked` is `None`, and returns the editor started and what was opened;
/// returns the status and the message of the answer when nothing is opened.
fn open_place(shared: &Shared, file: &str, start: usize, asked: Option<&str>) -> Result<(Started, Opening), (u16, String)> {
    let (places, read) = {
        let state = shared.state();
        (state.places.clone(), state.read.iter().any(|path| path == Path::new(file)))
    };
    let places = places.filter(|places| read && places.has(file));
    let Some(places) = places else {
        return Err((403, format!("the file to open must be a source of the document that its last build read, not `{file}`")));
    };
    let Some((line, column)) = places.line_column(file, start) else {
        return Err((404, format!("the place to open must lie within the file `{file}` as it was when the page's build read it; byte {start} is past its end")));
    };
    let editors = &shared.editors;
    if editors.is_empty() {
        return Err((503, editors.none_found()));
    }
    let chosen = match asked {
        Some(name) => editors.get(name),
        None => editors.default(),
    };
    let Some(editor) = chosen else {
        let found: Vec<&str> = editors.iter().map(Editor::name).collect();
        let name = asked.unwrap_or_default();
        return Err((400, format!("the editor `{name}` must be one of the editors found on this machine: `{}`", found.join("`, `"))));
    };
    let path = std::path::absolute(file).unwrap_or_else(|_| PathBuf::from(file));
    let started = editor.open(&path, line, column).map_err(|error| (500, error.to_string()))?;
    Ok((started, Opening::Opened { editor: editor.name(), file: path, line, column }))
}

/// The value of the header `Server-Timing` on an answer that carries a file of the output
/// of the build `build`: the metrics `flm-server` and `flm-build`, whose descriptions are
/// the server's identifier and the build's number. A page reads it from the answer that
/// carried it (the navigation entry's `serverTiming` in the browser), and so learns which
/// server and which build it shows without inferring them from the stream's first
/// `hello`; the bytes of the answer stay the build's.
fn served_by(server: &str, build: u64) -> String {
    format!("flm-server;desc=\"{server}\", flm-build;desc=\"{build}\"")
}

/// The path, relative to the output, of the file that the address path `path` designates:
/// `path` without its leading `/`, its percent-encoded bytes decoded; `None` when they do
/// not make UTF-8 text. The empty path designates the entry file.
fn file_name(path: &str) -> Option<String> {
    let path = path.strip_prefix('/').unwrap_or(path);
    percent_encoding::percent_decode_str(path).decode_utf8().ok().map(|name| name.into_owned())
}

/// Opens a stream of messages over `request`'s connection: the head of the answer, then
/// `hello`, then every message sent to the clients, on a thread of the stream's own; once
/// the server stops, the end of the stream.
///
/// The answer is written by hand ([`reply`] says why): an answer of `tiny_http` whose body
/// is a stream keeps the messages in its buffers until the body ends. Its body comes in
/// chunks, one per message, so that the browser is told where it ends, and a page whose
/// stream ended connects again.
fn stream(shared: &Arc<Shared>, request: Request) {
    let (send, messages) = channel::<String>();
    let editors: Vec<Value> =
        shared.editors.iter().map(|editor| json!({ "name": editor.name(), "label": editor.label() })).collect();
    {
        let mut state = shared.state();
        let output_build = state.output.as_ref().map(|(build, _)| *build);
        let references = state.output.as_ref().map(|(_, output)| &output.references);
        let hello = json!({
            "server": shared.server,
            "document": shared.document,
            "build": output_build,
            "building": state.building,
            "problems": state.problems,
            "references": references,
            "editors": editors,
            "editor": shared.editors.default().map(Editor::name),
        });
        // The browser waits a second, not its default three, before it connects again.
        let _ = send.send(format!("retry: 1000\n{}", frame("hello", &hello)));
        state.clients.push(send);
    }
    let mut writer = request.into_writer();
    let head = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\n\
                 Connection: close\r\nTransfer-Encoding: chunked\r\n\r\n";
    if writer.write_all(head).and_then(|()| writer.flush()).is_err() {
        return;
    }
    for message in messages {
        let chunk = format!("{:x}\r\n{message}\r\n", message.len());
        if writer.write_all(chunk.as_bytes()).and_then(|()| writer.flush()).is_err() {
            return;
        }
    }
    // The server stopped: the last chunk, which ends the body.
    let _ = writer.write_all(b"0\r\n\r\n").and_then(|()| writer.flush());
}

/// The page the server serves at `/` before a build has given an output, for the document
/// `document`: it says that the preview is waiting for the document, and carries the page
/// script and its stylesheet, written into it (the page is the server's own, not a
/// build's). The script lays the toolbar over it with the last build's problems, and loads
/// the document's page once a build gives one.
fn waiting_page(document: &str) -> String {
    let mut escaped = String::new();
    escape_text_into(&mut escaped, document);
    let document = escaped;
    format!(
        "<!DOCTYPE html>\n<html data-flm-preview-waiting>\n<head>\n<meta charset=\"utf-8\">\n\
         <title>FLM preview: {document}</title>\n<style>\n{STYLE_TEXT}</style>\n</head>\n<body>\n\
         <div class=\"flm-preview-waiting\">\n<h1>FLM preview</h1>\n\
         <p>The preview is waiting for the document <code>{document}</code>: it is being built, \
         or the last build could not produce it. The toolbar below lists the problems of the \
         last build. This page is replaced by the document as soon as a build produces it.</p>\n</div>\n\
         <script>\n{SCRIPT_TEXT}</script>\n</body>\n</html>\n"
    )
}
