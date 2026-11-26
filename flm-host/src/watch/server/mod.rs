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
//!   [`preview`](crate::stdpkg::preview)), which replaces it by the document's page once a
//!   build gives one.
//! - **The tools page**, a page of the server's own at [`TOOLS`], which shows the document's
//!   page (at `/`) in a frame beside the preview's tools, and the files it uses, each at
//!   [`ASSETS`] followed by its path. The same page at [`HELP`] shows the help browser
//!   alone. These files are compiled into the program,
//!   compressed with gzip (the crate's build script compresses the files of the folder
//!   `watch/client/tools/` of its sources); each is sent as it is compiled in, with
//!   `Content-Encoding: gzip`, to a request whose header `Accept-Encoding` accepts gzip, as
//!   every browser's does, and decompressed to any other. An address under [`ASSETS`] that
//!   is not exactly the path of one of these files is answered with `404`. [`Server::url`]
//!   gives the address of the document's page, and [`Server::tools_url`] that of the tools
//!   page.
//! - **Its own addresses**, under the prefix [`PREFIX`]: [`EVENTS`], the stream of
//!   messages; [`REBUILD`], an HTTP request to build again now, which the server passes to the
//!   watch loop's [`Handle::build_again`](super::Handle::build_again); [`OPEN`], a request
//!   to open a place of the document's files in the editor (below); [`VIEWER`], a request
//!   to change the viewer settings (below); [`PROFILE`], a request
//!   to change the build type (below); [`SETTINGS`], the settings state of the document;
//!   [`SETTINGS_YAML`], a value of the front matter as YAML text; [`SETTINGS_SOURCES`], the
//!   sources of the value at a path; [`FRONT_MATTER`], a change to the front matter; [`AT`], the constructs at a place of a source;
//!   [`HELP_INDEX`] and [`HELP_PAGE`], the help pages; and [`EXPORT`] and [`EXPORT_OPEN`],
//!   the export of the document (below).
//!
//! Every answer tells the browser not to keep it (`Cache-Control: no-store`), so that a page
//! loaded anew shows the last build, and closes its connection, so that a page left open
//! while the server was stopped and started again in the same program connects anew.
//!
//! **Only the preview's own page can act.** An HTTP request that changes something (every
//! `POST`: build again, open a place, change the viewer settings, change the build type,
//! change the front matter, export the document, open the exported file) is
//! carried out only when it comes
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
//! **The protocol.** The page script and the tools page (the *watch client*) and the server
//! exchange messages and requests that the watch client's protocol specifies: the file
//! `npm/flm-watchclient/PROTOCOL.md` of FLM's repository, beside the client's sources. This
//! module describes how this server carries out its side of the protocol.
//!
//! **The stream of messages** is a stream of server-sent events, a web standard that a
//! browser reads with `EventSource` and reconnects to by itself when the connection is
//! lost. The server sends `hello` first on every connection, `building` when it is told
//! that a build starts ([`Server::tell`]), `progress` while that build runs, and `update`
//! when it is told that a build is done, with the build's update as
//! [`Update::to_json`](super::update::Update::to_json) gives it. A `progress` message holds
//! the text of the progress line of the build under way
//! ([`progress_line`]), estimated from its progress sheet
//! with the table of expected durations that the watch loop's handle gives
//! ([`Handle::progress`](super::Handle::progress)), the fraction never decreasing during a
//! build ([`ShownFraction`]): the server reads the sheet four times a second, and sends the
//! text when it differs from the last one sent for that build, and a stream opened during
//! that build gets the last text sent right after `hello`. Both `hello` and `update` carry what the build's [document
//! snapshot](super::snapshot::Snapshot) gives
//! ([`Snapshot::fields`](super::snapshot::Snapshot::fields)): the problems of the last
//! build, the labels of the page's elements, and the outline and the labels of the
//! document that the output served shows (`null` while no output is served). The fields of
//! `hello` come from what the server holds and from what its caller gave to
//! [`Server::start`]: `server` is an identifier the server makes when it starts, from its
//! process, its port and the time; `document` is the document's name;
//! `editors` lists every editor the preview knows
//! ([`Editors::listed`](super::editor::Editors::listed)), in their order, then the editor
//! command, each with its name, the name a page shows, and whether a place can be opened
//! in it (`available`: the editor was found on this machine; always, for the editor
//! command, which choosing writes when there is none); `viewer` gives the viewer settings (below); and `help` says whether the
//! program running the preview gives the help pages (below). Both `hello` and `update` also
//! say what output is served and how long the last build took: `media_type`, the media type
//! of the output's entry file (`null` while no output is served, and for an output without
//! an entry file, such as the output without files of a profile that declares no output);
//! `earlier_output`, `true` when the output served is that of a build before the last one
//! done (the last build did not hand over its output); and `duration_ms`, how long the last
//! build took, in whole milliseconds (`null` before the first build is done). They also say
//! which *build types* the page can offer: `profile`, the profile the last build done ran
//! ([`Built::profile`](super::Built::profile): the profile asked for, else that of the last
//! preparation that succeeded; `null` before the first build is done, and when neither
//! exists); `default_profile`, the profile the builds run when no page has chosen one
//! ([`Built::default_profile`](super::Built::default_profile): the profile asked for when
//! the preview started, else the front matter's `default_profile`, else `html`, as the last
//! preparation that succeeded found them; `null` before the first build is done, and while
//! no profile was asked for at start and no preparation has succeeded); and `profiles`, the profiles the document can run, in name order, as
//! the last preparation that succeeded lists them
//! ([`Prepared::profiles`](crate::session::Prepared::profiles); empty before it). Last,
//! they say when to read the settings state again: `environment`, the digest of the
//! registry of the last preparation that succeeded, as 16 hexadecimal digits (`null` before
//! it), which changes when the packages, the features or their settings change; and
//! `settings`, a counter that the server increases at each build done (0 before the first),
//! whenever the settings state may have changed: the document's file changed, or the
//! document was prepared again. Every answer that carries a file of an output identifies
//! the server and the build in its header `Server-Timing` (the metrics `flm-server` and
//! `flm-build`).
//!
//! **The constructs at a place.** A `GET` of [`AT`] whose query gives the name of a
//! source and a byte offset into it, as the page's elements carry them
//! (`?file=paper.flm&start=230`), answers one JSON object: the file and the offset, the
//! line and the column of the offset (both counted from 1, the column in characters), and
//! the constructs that contain it, from the innermost outward, each as
//! [`Construct::to_json`](super::snapshot::Construct::to_json) gives it
//! ([`Processed::at`](super::snapshot::Processed::at)), with the field `help`: the path of
//! its help page, as the program's services give it for the last preparation that succeeded
//! ([`Help::construct_page`](super::services::Help::construct_page)), or `null`. They come
//! from the document snapshot of the output served. The answer is `400`, with one line of
//! plain text, for a query whose `file` or `start` is missing, given twice or not valid,
//! and `404` when no output is served yet, when the output served has no source of that
//! name, or when the offset is past the end of the source.
//!
//! **The help pages.** The program running the preview gives the help pages of the
//! document's environment through its services ([`Services::help`]), for the last
//! preparation that succeeded: the server keeps the preparation that the watch loop hands
//! it ([`Preparation::Prepared`]), and passes it to the service on the request's own
//! thread. A `GET` of [`HELP_INDEX`] answers one JSON object: `environment`,
//! the digest of the preparation's registry (as `environment` in `hello`), and `pages`, every
//! page as [`HelpEntry::to_json`](super::services::HelpEntry::to_json) gives it, in the
//! order of the tree of the pages. A `GET` of [`HELP_PAGE`] whose query gives a page's path
//! (`?path=features/headings`) answers one JSON object: `path`, the path asked for; `page`,
//! the page as [`HelpPage::to_json`](super::services::HelpPage::to_json) gives it, or `null`
//! when no page has that path; and `nearest`, for such a path, the nearest pages, each with
//! its `path` and `title` (empty otherwise). The answer is `400` for a query whose `path`
//! is missing, given twice or not valid; `501` when the program gives no help pages; `404`
//! when no preparation has succeeded yet; and `500` when the page or the index cannot be
//! made; each with one line of plain text.
//!
//! **The settings of the document.** A `GET` of [`SETTINGS`] answers the *settings state*,
//! one JSON object ([`DocumentSettings::state`]): the front matter's revision and what it
//! writes, read from the document's file at that moment, or the reason it cannot be read;
//! and, from the latest successful preparation ([`Server::tell`] gives it to
//! [`DocumentSettings::set_prepared`]), the schema of a front matter, the values in effect
//! with their origins, and the choices that depend on the document's environment; after a
//! preparation that failed, why it failed, with the schema, the packages found and the math
//! processors of the document's packages as the watch loop found them when it could
//! ([`DocumentSettings::set_failed`]), and whether the schema is current. A `GET`
//! of [`SETTINGS_YAML`] whose query gives a path of the front matter as a JSON Pointer
//! (`?path=/flm/features/floats`) answers the value there as YAML text
//! ([`DocumentSettings::yaml_at`]). A `GET` of [`SETTINGS_SOURCES`] with such a query
//! answers one JSON object: every source's value at the path, which one is in effect, the
//! value in effect or the default that applies, and the values that a write there copies
//! ([`DocumentSettings::sources_at`]). A `POST` of [`FRONT_MATTER`] whose body is one JSON
//! object, the revision of the front matter that the page saw and a list of changes, each a
//! path and a value as JSON, a value as YAML text, or a removal, writes the document's file
//! with those changes, made in the order given and edited in place
//! ([`super::front_matter::edit_front_matter`], [`DocumentSettings::write`]), with the
//! values that a value set inside a value decided whole copies, and answers `200` once the
//! file is written, with one JSON object whose field `copied` lists these copies; the file
//! is written once, with all the changes or none,
//! and the one build that follows is the watch loop's, which sees the file change. When a
//! request cannot be carried out, the answer is a status and one line of plain text that the
//! page shows: `400` for a body that does not give a revision and a list of changes, or that
//! is longer than the server accepts (4 MiB), for YAML text that cannot be read, and for a
//! missing or invalid path of a value as YAML or of the sources of a value; `412` for a
//! revision that is not the front matter's; `409` for changes that cannot be written into the front matter (a path
//! through a value that is not a map, a value of a kind that is never written, or an edit
//! that does not read back exactly),
//! and for a front matter that cannot be read (for example because it is not valid YAML
//! while the author is editing it), except in the settings state, which gives the reason in
//! its field `unreadable` (nothing is written in any of these cases); and `500` for a file
//! that cannot be read or written. The server reads and writes this one
//! file, the document's, and serves nothing else from the disk.
//!
//! **Exporting the document.** An export is a build of the document as `flm render -o
//! STEM.EXT` builds it, which the watch loop carries out between two builds of the preview
//! ([`super::export`], [`Handle::export`](super::Handle::export)); each request about an export
//! waits for the watch loop's answer, after the build under way. A `GET` of [`EXPORT`] whose
//! query gives a build type (`?profile=html`) answers one JSON object: `profile`, the build
//! type; `name`, the export's name `STEM.EXT`, for which the watch loop prepares the build
//! type ([`Handle::export_name`](super::Handle::export_name)); and `command` and
//! `command_continuing`, the command lines of `flm render` that write the same files, without
//! and with `--continue-with-errors`, as the program's services compose them
//! ([`Services::export_command`]; `null` when the program composes none). A `POST` of
//! [`EXPORT`] whose body is one JSON object, `{"profile": NAME, "action": "save" |
//! "download", "continue_with_errors": BOOL}`, carries out the export: `save` writes the files
//! into the document's folder, and the answer is one JSON object, `name` and `files` (the
//! paths written, relative to the document's folder); `download` writes nothing, and the
//! answer is the export's one file, or a zip archive `STEM.zip` of its files, each at its
//! path, with a header `Content-Disposition` that names it. The server takes no path from the
//! request: the export is always named and placed as above. When nothing is written, the
//! answer's first line is the reason and, for a refusal for errors, the errors of the
//! export's build in full follow after a blank line (the log of a failed external program
//! is quoted there). The answer is `428` when the document has errors under the build type,
//! found before any file is produced or any external program runs, and the export does not
//! continue with errors: the one refusal that `"continue_with_errors": true` lifts. It is
//! `409` when the build type declares no output, a file of a save has the path of a file
//! that the document's build reads, or the build fails, which includes an error reported by
//! a stage that writes files or runs an external program when the export does not continue
//! with errors. A `GET` whose build type cannot be prepared or declares no output is
//! answered with `409` in the same way. The answer is `400` for a query or a body that does
//! not give a build type as above, or a body longer than the server accepts (4 KiB); `500`
//! when files of a save could not be written or the archive of a download could not be
//! made; and `503` once the preview has ended. A `POST` of [`EXPORT_OPEN`], with no body, opens
//! the main file of the last export saved (the entry file of its leading output) with the
//! system's default application, through the program's services ([`Services::open_file`])
//! on the request's thread, and answers `204`; `404` when no export was saved since the
//! server started, `501` when the program opens no file, and `500` when it could not open
//! it, each with one line of plain text.
//!
//! **Changing the build type.** A page sets the profile of the builds that follow with a
//! `POST` to [`PROFILE`] whose body is one JSON object, `{"profile": NAME}`. The server
//! passes the name to the watch loop's
//! [`Handle::set_profile`](super::Handle::set_profile) and answers `204`; the build that
//! follows is reported on the stream as any other. A body that is not such an object, whose
//! name is empty, or that is longer than the server accepts (4 KiB) is answered with `400`
//! and one line of plain text. The name is not checked against `profiles`: a profile that
//! cannot be prepared gives a build whose problems say why, and the output served stays the
//! last one (`earlier_output`).
//!
//! **Opening a place in the editor.** A page asks for a place of the document's files to be
//! opened in an editor (the place of an element the author clicked, or of a problem) with
//! a `POST` to [`OPEN`] whose body is one JSON object, `{"file": NAME, "start": OFFSET}` or
//! `{"file": NAME, "line": LINE, "column": COLUMN}`: the name of a source and either a byte
//! offset into it, as the page's elements and the problems in a file's content carry them,
//! or a line and a column (the column in bytes), as a problem in a file's front matter
//! carries them ([`Spot`](super::editor::Spot)). The request names no editor. The server
//! opens only a file that the last build read, and, for an offset, that is a source of the
//! build whose output it serves ([`Places`](super::snapshot::Places)); it turns the place
//! into a line and a column itself ([`Editors::locate`]), and starts the editor of the
//! viewer settings (below) on the file at that line and column. It answers `204` once the
//! editor has started. When the place is located but no editor can be opened (there is none,
//! or the one chosen cannot be started), it answers `200` with one JSON object, `{"place":
//! "PATH:LINE:COLUMN", "reason": TEXT}`: the place, the file's path absolute and in the
//! system's form, for the page to copy, and one line that says why no editor was opened.
//! Otherwise the answer is a status and one line of plain text that says why nothing was
//! opened, which the page shows: `400` for a body that is not such an object or that is
//! longer than the server accepts (64 KiB); `403` for a file that the last build did not
//! read, or, for an offset, that is not such a source; `404` for an offset past the end of
//! the file; and `500` for a file that names no file on this system.
//!
//! The server reports the outcome of each request to open a place, as an [`Opening`], to
//! the function given to [`Server::start`]: the file, the line, the column and the editor
//! opened; the place and the reason when no editor was opened; or the text the page received
//! when nothing was opened; and, when an editor that was started later ends with failure,
//! how it ended and what it wrote on its standard error. The server answers the page as
//! soon as the editor has started, and waits for the editor's end afterwards, on the
//! request's own thread. The server prints nothing; its
//! caller decides how to show these outcomes (the `flm` program prints them in its
//! terminal).
//!
//! **The viewer settings** ([`super::viewer`]) are the author's own, in the file
//! `.flm/watch.json` of the home folder that the program's services give
//! ([`Services::home_folder`]); without one, they live in memory. The server reads the file
//! when it starts, and sends the settings in `hello`, as `viewer`: one JSON object,
//! `editor`, the name of the editor a place opens in (`null` when there is none), every other
//! viewer setting but the editor command
//! ([`ViewerSettings::to_json`](super::viewer::ViewerSettings::to_json)), and `unreadable`,
//! the reason the file cannot be read, on one line, or `null` ([`Server::viewer_unreadable`]
//! gives it to the caller, to report it once). The editor a place opens in is chosen
//! ([`Editors::choose`]) from the name that the program's command line gives
//! ([`Editors::preferred`]), until a page chooses one, else from the viewer settings; the
//! editor command of the command line counts over the file's. A `POST` to [`VIEWER`] whose
//! body is one JSON object of the settings to change and their values (`null` for a
//! default), such as `{"theme": "dark"}`, changes them ([`ViewerFile::change`]): it writes
//! those keys into the file as it is at that moment, keeping every other key, known or not,
//! answers `204`, and sends every page the message `viewer`, whose fields `editors` and
//! `viewer` are those of `hello` (the file's editor command, read again, can change the
//! editors). A change never sets the editor command, and names no key that is not a viewer
//! setting: such a body, a value of the wrong kind, an editor that is not one of `editors`,
//! a body that is not a JSON object or that is longer than the server accepts (4 KiB) is
//! answered with `400`. While the file cannot be read, a change is answered with `409` and
//! the reason, and nothing is written; a file that cannot be written is answered with
//! `500`. Each with one line of plain text.
//!
//! **Threads.** The server accepts connections on a thread of its own and answers each
//! request on a new thread. It keeps what it needs of each event it is told and sends the
//! messages; each open stream is written on its own thread, so a slow or closed page holds
//! up neither the other pages nor the watch loop. A stream whose page went away is dropped
//! at the next message.

mod at;
mod export;
mod help;
mod http;
mod open;
mod output;
mod profile;
mod settings;
mod stream;
mod tools;
mod viewer;

use std::fmt;
use std::net::{Ipv4Addr, SocketAddr, TcpListener};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use serde_json::{Map, Value, json};
use tiny_http::{Method, Request};

use self::http::reply;
use self::stream::frame;
use self::viewer::Viewer;
use super::editor::{EditorCommand, EditorFailure, Editors};
use super::services::Services;
use super::settings::DocumentSettings;
use super::snapshot::Snapshot;
use super::update::{BuildOutput, PageChange};
use super::viewer::{ViewerError, ViewerFile, path_in};
use super::{Event, Handle, Preparation};
use crate::report::{ShownFraction, progress_line};
use crate::session::Prepared;

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

/// The address of the tools page, `/.flm-preview/`: the page that shows the document in a
/// frame beside the preview's tools.
pub const TOOLS: &str = "/.flm-preview/";

/// The address of the help browser alone, `/.flm-preview/help`: the tools page, which at
/// this address shows the help pages alone, on the page whose path the address's fragment
/// gives.
pub const HELP: &str = "/.flm-preview/help";

/// The prefix of the addresses of the files of the tools page, `/.flm-preview/assets/`: the
/// file at the path `PATH` among them is at `/.flm-preview/assets/PATH`.
pub const ASSETS: &str = "/.flm-preview/assets/";

/// The address of the stream of messages, `/.flm-preview/events`.
pub const EVENTS: &str = "/.flm-preview/events";

/// The address of a request to build the document again now, `/.flm-preview/rebuild`,
/// with the method `POST`.
pub const REBUILD: &str = "/.flm-preview/rebuild";

/// The address of a request to open a place in the editor, `/.flm-preview/open`, with the
/// method `POST` (see the [module documentation](self), "Opening a place in the editor").
pub const OPEN: &str = "/.flm-preview/open";

/// The address of a request to change the build type, `/.flm-preview/profile`, with the
/// method `POST` (see the [module documentation](self), "Changing the build type").
pub const PROFILE: &str = "/.flm-preview/profile";

/// The address of the constructs at a place of a source, `/.flm-preview/at`, with the method
/// `GET` and the place in the query's parameters `file` and `start` (see the [module
/// documentation](self), "The constructs at a place").
pub const AT: &str = "/.flm-preview/at";

/// The address of the index of the help pages, `/.flm-preview/help/index`, with the method
/// `GET` (see the [module documentation](self), "The help pages").
pub const HELP_INDEX: &str = "/.flm-preview/help/index";

/// The address of one help page, `/.flm-preview/help/page`, with the method `GET` and the
/// page's path in the query's parameter `path` (see the [module documentation](self), "The
/// help pages").
pub const HELP_PAGE: &str = "/.flm-preview/help/page";

/// The address of the export of the document, `/.flm-preview/export`: with the method `GET`
/// and the build type in the query's parameter `profile`, the name and the command lines of
/// an export; with the method `POST`, an export (see the [module documentation](self),
/// "Exporting the document").
pub const EXPORT: &str = "/.flm-preview/export";

/// The address of a request to open the main file of the last export saved,
/// `/.flm-preview/export/open`, with the method `POST` (see the [module documentation](self),
/// "Exporting the document").
pub const EXPORT_OPEN: &str = "/.flm-preview/export/open";

/// The address of a request to change the viewer settings, `/.flm-preview/viewer`, with the
/// method `POST` (see the [module documentation](self), "The viewer settings").
pub const VIEWER: &str = "/.flm-preview/viewer";

/// The address of the settings state of the document, `/.flm-preview/settings`, with the
/// method `GET` (see the [module documentation](self), "The settings of the document").
pub const SETTINGS: &str = "/.flm-preview/settings";

/// The address of a value of the front matter as YAML text, `/.flm-preview/settings/yaml`,
/// with the method `GET` and the path in the query's parameter `path` (see the [module
/// documentation](self), "The settings of the document").
pub const SETTINGS_YAML: &str = "/.flm-preview/settings/yaml";

/// The address of the sources of the value at a path of the front matter,
/// `/.flm-preview/settings/sources`, with the method `GET` and the path in the query's
/// parameter `path` (see the [module documentation](self), "The settings of the document").
pub const SETTINGS_SOURCES: &str = "/.flm-preview/settings/sources";

/// The address of a request to change the front matter, `/.flm-preview/front-matter`, with the
/// method `POST` (see the [module documentation](self), "The settings of the document").
pub const FRONT_MATTER: &str = "/.flm-preview/front-matter";

/// The time between two readings of the progress of the build under way, for the
/// `progress` messages: at most four a second.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

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
        /// The editor's name: one of [`EDITORS`](super::editor::EDITORS), or the program of
        /// the editor command.
        editor: String,
        /// The file, as the build named it; the editor is given its absolute path in the
        /// system's form ([`Editor::open`](super::editor::Editor::open)).
        file: String,
        /// The line.
        line: usize,
        /// The column.
        column: usize,
    },
    /// The place was located, but no editor could be opened: there is none, or the one
    /// chosen could not be started. The page was given the place, to copy.
    Unopened {
        /// The place, `PATH:LINE:COLUMN`, the path absolute and in the system's form.
        place: String,
        /// Why no editor was opened.
        reason: String,
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
/// [`Server::tell`], and open [`Server::tools_url`] in a browser (or [`Server::url`] for the
/// document alone). Dropping it stops it.
pub struct Server {
    http: Arc<tiny_http::Server>,
    address: SocketAddr,
    shared: Arc<Shared>,
    stopping: Arc<AtomicBool>,
    accepting: Option<JoinHandle<()>>,
    /// The thread that sends the `progress` messages ([`tell_progress`]), and the sender
    /// that stops it, by being dropped.
    progress: Option<(Sender<()>, JoinHandle<()>)>,
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
    /// The editors found, and the editor and the editor command the command line gives.
    editors: Editors,
    /// The viewer settings, which choose the editor a place opens in.
    viewer: Mutex<Viewer>,
    /// The function to which the outcome of each request to open a place is reported.
    openings: Openings,
    /// What the last successful preparation gives of the settings, and the document's file,
    /// for the requests of [`SETTINGS`] and [`SETTINGS_YAML`].
    settings: DocumentSettings,
    /// What the program running the preview gives: the help pages.
    services: Box<dyn Services>,
    /// What the server holds of the last builds, and the open streams.
    state: Mutex<State>,
}

impl Shared {
    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The viewer settings, locked. When both locks are held, this one is taken first,
    /// before [`Shared::state`].
    fn viewer(&self) -> MutexGuard<'_, Viewer> {
        self.viewer.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

/// What the server holds of the last builds.
#[derive(Default)]
struct State {
    /// The output now current, and the number of the build that gave it.
    output: Option<(u64, Arc<BuildOutput>)>,
    /// Whether a build is under way.
    building: bool,
    /// What the pages were told of the progress of the build under way.
    told: Told,
    /// The document snapshot of the last build ([`super::Built::snapshot`]): its problems,
    /// and what the run of the build that gave the output served knows of the document.
    snapshot: Arc<Snapshot>,
    /// The files the last build attempt read and found: the only files the editor is
    /// opened on.
    read: Vec<String>,
    /// The number of the last build done and how long it took; `None` before the first.
    last: Option<(u64, Duration)>,
    /// The profile the last build done ran ([`super::Built::profile`]); `None` before the
    /// first build is done, and while no profile is asked for and no preparation succeeded.
    profile: Option<String>,
    /// The default profile of the last build done ([`super::Built::default_profile`]);
    /// `None` before the first build is done, and while no profile is asked for at start and
    /// no preparation succeeded.
    default_profile: Option<String>,
    /// The profiles the document can run, as the last preparation that succeeded lists them
    /// ([`Prepared::profiles`](crate::session::Prepared::profiles)); empty before it.
    profiles: Vec<String>,
    /// The last preparation that succeeded ([`super::Preparation::Prepared`]), for which the
    /// program's services answer, and whose registry's digest is the `environment`; `None`
    /// before it.
    prepared: Option<Arc<Prepared>>,
    /// The number of builds done: the counter that tells the pages to read the settings
    /// state again.
    settings: u64,
    /// The main file of the last export saved, as the host writes paths; `None` before it.
    exported: Option<String>,
    /// The open streams: each takes the messages, framed, for its own thread to write.
    clients: Vec<Sender<String>>,
}

/// What the pages were told of the progress of the build under way, from its start.
#[derive(Default)]
struct Told {
    /// The fraction of the build done, as the `progress` messages show it.
    fraction: ShownFraction,
    /// The text of the last `progress` message; `None` before the first.
    text: Option<String>,
}

impl State {
    /// Sends `message`, framed, to every open stream, and drops the streams that closed.
    fn send(&mut self, message: &str) {
        self.clients.retain(|client| client.send(message.to_owned()).is_ok());
    }

    /// The fields of `hello` and `update` that describe the output served, the last build
    /// and the build types: `media_type`, the media type of the output's entry file (`null`
    /// when no output is served or it has no entry file); `earlier_output`, whether the
    /// output served is from a build before the last one done; `duration_ms`, how long the
    /// last build took, in whole milliseconds (`null` before the first); `profile`, the
    /// profile the last build done ran (`null` before the first, and while no profile is
    /// asked for and no preparation succeeded); `default_profile`, the profile the builds run
    /// when the pages have chosen none (`null` before the first build is done, and while no
    /// profile is asked for at start and no preparation succeeded); `profiles`, the profiles
    /// the document can run; `environment`, the digest of the registry of the last preparation that
    /// succeeded (`null` before it); and `settings`, the counter of the settings state.
    fn served(&self) -> Map<String, Value> {
        let media_type = self.output.as_ref().and_then(|(_, output)| output.entry()).map(|entry| entry.media_type.clone());
        let earlier = match (&self.output, self.last) {
            (Some((build, _)), Some((last, _))) => *build != last,
            _ => false,
        };
        let duration = self.last.map(|(_, duration)| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX));
        let mut fields = Map::new();
        fields.insert(String::from("media_type"), json!(media_type));
        fields.insert(String::from("earlier_output"), json!(earlier));
        fields.insert(String::from("duration_ms"), json!(duration));
        fields.insert(String::from("profile"), json!(self.profile));
        fields.insert(String::from("default_profile"), json!(self.default_profile));
        fields.insert(String::from("profiles"), json!(self.profiles));
        fields.insert(String::from("environment"), json!(self.prepared.as_deref().map(environment_of)));
        fields.insert(String::from("settings"), json!(self.settings));
        fields
    }
}

impl Server {
    /// Starts the server, listening at `port`, for the document `document` (the name of the
    /// document's file as the caller wants pages to show it; the server sends it to the
    /// pages in `hello` and writes it into the waiting page). An HTTP request to build again
    /// now is passed to `handle`. A place to open is opened in one of `editors`, the editors
    /// found on this machine ([`Editors::find`]) and the editor command, the one that the
    /// viewer settings choose; the server lists them to the pages in `hello`. The server
    /// reads the viewer settings in the home folder that `services` gives
    /// ([`Services::home_folder`]). The outcome of each such request is
    /// reported to `openings` ([`Opening`]). The requests of [`SETTINGS`],
    /// [`SETTINGS_YAML`] and [`FRONT_MATTER`] are answered through `settings`; those of
    /// [`HELP_INDEX`] and [`HELP_PAGE`], and the help page of each construct that a request
    /// of [`AT`] answers, and the command lines of an export ([`EXPORT`]) and the opening of
    /// an exported file ([`EXPORT_OPEN`]), through `services`, what the program running the
    /// preview gives ([`Services`]; [`NoServices`](super::services::NoServices) for none);
    /// an export is carried out by the watch loop of `handle`. The server
    /// serves no output until it is told that a build gave one ([`Server::tell`]).
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
        services: impl Services + 'static,
    ) -> Result<Server, ServeError> {
        let (http, address) = listen(port)?;
        let http = Arc::new(http);
        let viewer = ViewerFile::read(services.home_folder().map(|home| path_in(&home)));
        let shared = Arc::new(Shared {
            server: server_name(address),
            origin: format!("http://{address}"),
            document: document.into(),
            handle,
            editors,
            viewer: Mutex::new(Viewer::new(viewer)),
            openings: Box::new(openings),
            settings,
            services: Box::new(services),
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
        let progress = {
            let shared = Arc::clone(&shared);
            let (stop, stopped) = mpsc::channel::<()>();
            (stop, thread::spawn(move || tell_progress(&shared, &stopped)))
        };
        Ok(Server { http, address, shared, stopping, accepting: Some(accepting), progress: Some(progress) })
    }

    /// Returns the address and the port the server listens at, on this machine's loopback
    /// address.
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Returns the address of the document's page, `http://127.0.0.1:PORT/`: the document
    /// alone, kept up to date by its page script, without the tools.
    pub fn url(&self) -> String {
        format!("http://{}/", self.address)
    }

    /// Returns the address of the tools page, `http://127.0.0.1:PORT/.flm-preview/`, which a
    /// browser opens: the document in a frame, beside the preview's tools.
    pub fn tools_url(&self) -> String {
        format!("http://{}{TOOLS}", self.address)
    }

    /// The name of the editor a place opens in now: one of
    /// [`EDITORS`](super::editor::EDITORS), or [`CUSTOM`](super::editor::CUSTOM); `None`
    /// when there is no editor (see the [module documentation](self), "The viewer
    /// settings").
    pub fn editor(&self) -> Option<&'static str> {
        self.shared.viewer().chosen(&self.shared.editors).map(|chosen| chosen.name())
    }

    /// The editors found, and the editor and the editor command the command line gives, as
    /// the caller gave them to [`Server::start`].
    pub fn editors(&self) -> &Editors {
        &self.shared.editors
    }

    /// The editor command in effect: the command line's ([`Editors::command`]), else the
    /// viewer settings'; `None` when there is neither.
    pub fn editor_command(&self) -> Option<EditorCommand> {
        let viewer = self.shared.viewer();
        self.shared.editors.command().or(viewer.file.settings().editor_command.as_ref()).cloned()
    }

    /// Why the file of the viewer settings cannot be read, as the server read it at start;
    /// `None` when it can, or does not exist, or there is no file (see the [module
    /// documentation](self), "The viewer settings").
    pub fn viewer_unreadable(&self) -> Option<ViewerError> {
        self.shared.viewer().file.unreadable().cloned()
    }

    /// Hands the server an event of the watch loop. A build starting is sent to every page
    /// (`building`), and so is the progress of that build while it runs (`progress`, see the
    /// [module documentation](self), "The stream of messages"). A build done makes its output the one served (or, when it did not
    /// hand over its output, keeps the last output a build handed over), and its update is sent to every page
    /// (`update`), with the profile in use and the counter of the settings state increased;
    /// a build that prepared the document again first makes that preparation the one that
    /// `GET` of [`SETTINGS`] reads ([`DocumentSettings::set_prepared`]), its profiles the
    /// ones the pages are offered
    /// ([`Prepared::profiles`](crate::session::Prepared::profiles)), and the digest of its
    /// registry the `environment`, and the preparation itself the one for which the
    /// program's services answer; a build whose preparation failed is recorded in the
    /// settings state with the packages the watch loop found
    /// ([`Preparation::Failed`],
    /// [`DocumentSettings::set_failed`]). What the settings state takes from the
    /// preparation and the profiles are computed here, on the caller's thread, which is the
    /// watch loop's. A
    /// problem of the watcher is not passed to the pages; showing it is left to the caller.
    pub fn tell(&self, event: &Event) {
        if let Event::Built(built) = event {
            match &built.preparation {
                Preparation::Prepared(prepared) => self.shared.settings.set_prepared(prepared),
                Preparation::Failed(failed) => {
                    if let Err(error) = &built.outcome {
                        self.shared.settings.set_failed(error, failed.packages.as_ref());
                    }
                }
                Preparation::Reused => {}
            }
        }
        let mut state = self.shared.state();
        match event {
            Event::Building { changed } => {
                state.building = true;
                state.told = Told::default();
                state.send(&frame("building", &json!({ "changed": changed })));
            }
            Event::Built(built) => {
                state.building = false;
                state.told = Told::default();
                let mut update = built.update.to_json();
                state.snapshot = Arc::clone(&built.snapshot);
                state.read = built.files_read.iter().filter(|read| read.found).map(|read| read.path.clone()).collect();
                if built.update.page != PageChange::Absent {
                    state.output = built.output.clone().map(|output| (built.update.build, output));
                }
                state.last = Some((built.update.build, built.duration));
                state.profile.clone_from(&built.profile);
                state.default_profile.clone_from(&built.default_profile);
                if let Some(prepared) = built.preparation.prepared() {
                    state.profiles = prepared.profiles();
                    state.prepared = Some(Arc::clone(prepared));
                }
                state.settings += 1;
                if let Value::Object(fields) = &mut update {
                    fields.extend(state.snapshot.fields());
                    fields.extend(state.served());
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
        if let Some((stop, progress)) = self.progress.take() {
            drop(stop);
            let _ = progress.join();
        }
        // Each stream's thread ends once its sender is gone, closing its connection.
        self.shared.state().clients.clear();
    }
}

/// Sends every page the progress of the build under way (`progress`), every
/// [`PROGRESS_EVERY`] until `stop` closes: the text of its progress line, when it differs
/// from the text last sent for that build.
fn tell_progress(shared: &Shared, stop: &Receiver<()>) {
    // Nothing is ever sent: the loop ends when the server drops the sender.
    while let Err(RecvTimeoutError::Timeout) = stop.recv_timeout(PROGRESS_EVERY) {
        // The build under way is read while the state is locked, as [`Server::tell`] holds
        // it: the watch loop forgets a build's progress before it hands the build's end, so
        // the message of a build always comes before that build's `update` and before the
        // `building` of the next one.
        let mut state = shared.state();
        let Some(progress) = shared.handle.progress() else { continue };
        if !state.building {
            continue;
        }
        let snapshot = progress.sheet.snapshot();
        let fraction = state.told.fraction.estimate(&snapshot, &progress.expected, progress.origin.elapsed());
        let text = progress_line(&snapshot, fraction);
        if state.told.text.as_ref() != Some(&text) {
            state.send(&frame("progress", &json!({ "text": text })));
            state.told.text = Some(text);
        }
    }
}

/// The digest of the registry of `prepared` as 16 hexadecimal digits: the `environment`
/// of `hello`, `update` and the index of the help pages.
fn environment_of(prepared: &Prepared) -> String {
    format!("{:016x}", prepared.registry().digest())
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

/// Answers `request`: an address of the server's own, routed to the module of its group of
/// requests, or a file of the output ([`output::serve`]).
fn answer(shared: &Arc<Shared>, request: Request) {
    // The path alone: a query or a fragment names nothing the server serves.
    let path = request.url().split(['?', '#']).next().unwrap_or_default().to_owned();
    let method = request.method().clone();
    if !path.starts_with(PREFIX) {
        output::serve(shared, request, &path);
        return;
    }
    if method == Method::Post && !from_own_page(shared, &request) {
        let reason = format!("this request must come from the preview's page at {}/", shared.origin);
        reply(request, 403, Some("text/plain"), None, reason.as_bytes());
        return;
    }
    match (path.as_str(), method) {
        (EVENTS, Method::Get) => stream::stream(shared, request),
        (REBUILD, Method::Post) => {
            shared.handle.build_again();
            reply(request, 204, None, None, b"");
        }
        (OPEN, Method::Post) => open::open(shared, request),
        (VIEWER, Method::Post) => viewer::change(shared, request),
        (PROFILE, Method::Post) => profile::set(shared, request),
        (AT, Method::Get) => at::at(shared, request),
        (SETTINGS, Method::Get) => settings::read(shared, request),
        (SETTINGS_YAML, Method::Get) => settings::yaml(shared, request),
        (SETTINGS_SOURCES, Method::Get) => settings::sources(shared, request),
        (FRONT_MATTER, Method::Post) => settings::write(shared, request),
        (HELP_INDEX, Method::Get) => help::index(shared, request),
        (HELP_PAGE, Method::Get) => help::page(shared, request),
        (EXPORT, Method::Get) => export::describe(shared, request),
        (EXPORT, Method::Post) => export::export(shared, request),
        (EXPORT_OPEN, Method::Post) => export::open(shared, request),
        (
            EVENTS | REBUILD | OPEN | VIEWER | PROFILE | AT | SETTINGS | SETTINGS_YAML | SETTINGS_SOURCES | FRONT_MATTER | HELP_INDEX
            | HELP_PAGE | EXPORT | EXPORT_OPEN,
            _,
        ) => reply(request, 405, None, None, b""),
        (TOOLS | HELP, _) => tools::serve_index(request),
        (path, _) => match path.strip_prefix(ASSETS) {
            Some(asset) => tools::serve_asset(request, asset),
            None => reply(request, 404, None, None, b""),
        },
    }
}

/// Whether `request` may change something: it carries no `Origin` header, or the origin of
/// the pages this server serves (see the [module documentation](self), "Only the preview's
/// own page can act").
// The header is compared with the server's own origin, never with the request's `Host`: a
// page of a site whose name is made to resolve to this machine would pass that comparison.
fn from_own_page(shared: &Shared, request: &Request) -> bool {
    request
        .headers()
        .iter()
        .filter(|header| header.field.equiv("Origin"))
        .all(|header| header.value.as_str() == shared.origin)
}
