//! What the program does for the preview's gestures: finding the editors and opening a
//! place of the document's files in one of them (`watch::editor` and the server's address
//! `/.flm-preview/open`), and
//! the labels of the page's elements each update carries, for copying a reference. The
//! loop is driven over files held in memory, each of its events handed to the server, and
//! the requests are sent over plain connections of this machine. Each editor is a script of
//! the test's own, which writes the arguments it is given into a file, found on a search
//! path or in an application folder of the test's own: no editor is opened. Every wait has an upper bound and ends as soon as
//! what is expected arrives; the system chooses every port.

#![cfg(unix)]

#[path = "support/editor.rs"]
mod editor;

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::thread;
use std::time::Duration;

use editor::{failing_editor, fake_editor, folder, recorded};

use flm_main::files::MemoryFiles;
use flm_main::output::Naming;
use flm_main::session::{Input, Request};
use flm_main::watch::editor::{EDITORS, Editors, UnknownEditor};
use flm_main::watch::server::{OPEN, Opening, Port, Server};
use flm_main::watch::settings::DocumentSettings;
use flm_main::watch::{Event, Handle, Watch};

/// The upper bound of every wait.
const BOUND: Duration = Duration::from_secs(30);

/// The primary file: a front matter of three lines, then a line holding characters of
/// several bytes before `\emph`, then a file brought in.
const PAPER: &str = "---\ntitle: Notes\n---\nÇa été « dit » \\emph{ici}.\n\n\\input{part.flm}\n";

/// The file brought in, with a front matter of its own.
const PART: &str = "---\ntitle: A part\n---\n\nFirst line.\n\nThe \\textbf{bold} word.\n";

/// The content of `text` below its front matter of three lines: what the library parses,
/// and what the page's byte offsets count in.
fn content(text: &str) -> &str {
    let mut rest = text;
    for _ in 0..3 {
        rest = &rest[rest.find('\n').unwrap() + 1..];
    }
    rest
}

/// The loop and the server, the loop on a thread of its own handing each event to the
/// server.
struct Preview {
    server: Arc<Server>,
    handle: Handle,
    ended: Receiver<()>,
    /// What the server told of each request to open a place.
    openings: Receiver<Opening>,
}

impl Preview {
    /// The preview of `doc/paper.flm` and `doc/part.flm`, a place opened in one of
    /// `editors`; returns once the first build is done.
    fn start(editors: Editors) -> Preview {
        Preview::of(PAPER, editors)
    }

    /// The same, with `paper` as the text of `doc/paper.flm`.
    fn of(paper: &str, editors: Editors) -> Preview {
        let mut files = MemoryFiles::new();
        files.insert("doc/paper.flm", paper.as_bytes());
        files.insert("doc/part.flm", PART.as_bytes());
        let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
        request.default_math = flm_main::seed::MathChoice::processor("passthrough");
        request.naming = Naming::Folder;
        let files = Arc::new(files);
        let watch = Watch::new(request, files.clone(), Vec::new());
        let handle = watch.handle();
        let (tell, openings) = channel();
        // The settings are read and never saved here.
        let settings = DocumentSettings::new(serde_json::Value::Null, "doc/paper.flm", files, |_, _| Ok(()));
        let server = Arc::new(
            Server::start(
                Port::Exactly(0),
                "doc/paper.flm",
                handle.clone(),
                editors,
                move |opening| {
                    let _ = tell.send(opening);
                },
                settings,
            )
            .expect("the server starts"),
        );
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
        built.recv_timeout(BOUND).expect("the first build within the bound");
        Preview { server, handle, ended, openings }
    }

    fn address(&self) -> SocketAddr {
        self.server.address()
    }

    /// The next thing the server tells of a request to open a place.
    fn opening(&self) -> Opening {
        self.openings.recv_timeout(BOUND).expect("the server tells what became of the request within the bound")
    }

    fn stop(self) {
        self.handle.stop();
        self.ended.recv_timeout(BOUND).expect("the loop ends within the bound");
    }
}

/// Sends `method target` with the body `body` to `address` over a new connection: the
/// answer's status and body.
fn ask(address: SocketAddr, method: &str, target: &str, body: &[u8]) -> (u16, String) {
    let mut connection = TcpStream::connect(address).expect("the server is reached");
    connection.set_read_timeout(Some(BOUND)).unwrap();
    write!(
        connection,
        "{method} {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\nContent-Type: application/json\r\n\
         Content-Length: {}\r\n\r\n",
        body.len()
    )
    .unwrap();
    // The server may answer before it has read a body it does not take.
    let _ = connection.write_all(body);
    let mut bytes = Vec::new();
    let _ = connection.read_to_end(&mut bytes);
    let text = String::from_utf8_lossy(&bytes).into_owned();
    let status = text.split(' ').nth(1).and_then(|status| status.parse().ok()).expect("an answer within the bound");
    let body = text.split_once("\r\n\r\n").map(|(_, body)| body.to_owned()).unwrap_or_default();
    (status, body)
}

/// Asks the server at `address` to open the byte `start` of `file`, in the default editor.
fn open(address: SocketAddr, file: &str, start: usize) -> (u16, String) {
    ask(address, "POST", OPEN, serde_json::json!({ "file": file, "start": start }).to_string().as_bytes())
}

/// Asks the server at `address` to open the byte `start` of `file` in the editor `editor`.
fn open_in(address: SocketAddr, file: &str, start: usize, editor: &str) -> (u16, String) {
    ask(address, "POST", OPEN, serde_json::json!({ "file": file, "start": start, "editor": editor }).to_string().as_bytes())
}

/// The editors found on the search path `folder` alone.
fn on_path(folder: &std::path::Path) -> Editors {
    Editors::find(None, Some(folder.as_os_str()), &[]).unwrap()
}

/// The path the editor is given for the file named `name`.
fn given(name: &str) -> String {
    std::path::absolute(name).unwrap().display().to_string()
}

#[test]
fn a_place_is_opened_at_the_files_line_and_column_counted_in_characters() {
    let folder = folder("a_place_is_opened");
    let record = folder.join("record");
    fake_editor(&folder, "code", &record);
    let editors = on_path(&folder);
    assert_eq!(editors.default().expect("the script is found").name(), "code");
    let preview = Preview::start(editors);
    // The place the page names for `\emph`: the offset in the content below the front matter.
    let start = content(PAPER).find("\\emph").unwrap();
    let mut page = String::new();
    TcpStream::connect(preview.address())
        .and_then(|mut connection| {
            connection.set_read_timeout(Some(BOUND))?;
            connection.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")?;
            connection.read_to_string(&mut page)
        })
        .expect("the page");
    assert!(page.contains(&format!("[\"doc/paper.flm\",{start},")), "the page names the place of `\\emph`");
    assert_eq!(open(preview.address(), "doc/paper.flm", start), (204, String::new()));
    // Line 4 of the file, below a front matter of three lines; column 16, after fifteen
    // characters of which four take two bytes.
    assert_eq!(recorded(&record), ["--goto", &format!("{}:4:16", given("doc/paper.flm"))]);
    preview.stop();
}

#[test]
fn the_program_is_told_what_became_of_each_request_to_open_a_place() {
    let folder = folder("the_program_is_told");
    failing_editor(&folder, "emacsclient", "no Emacs server is running");
    let preview = Preview::start(on_path(&folder));
    let start = content(PAPER).find("\\emph").unwrap();
    // One opening, and the editor's failure after it.
    assert_eq!(open(preview.address(), "doc/paper.flm", start), (204, String::new()));
    assert_eq!(
        preview.opening(),
        Opening::Opened { editor: "emacsclient", file: PathBuf::from(given("doc/paper.flm")), line: 4, column: 16 }
    );
    let Opening::Failed(failure) = preview.opening() else { panic!("the editor's failure is told") };
    assert_eq!(failure.name, "emacsclient");
    assert_eq!(failure.said, "no Emacs server is running");
    assert!(failure.end.contains('3'), "{}", failure.end);
    // One refusal, told as the page is answered.
    let (status, message) = open(preview.address(), "doc/other.flm", 0);
    assert_eq!(status, 403);
    assert_eq!(preview.opening(), Opening::Refused { reason: message });
    preview.stop();
}

#[test]
fn a_place_in_an_included_file_is_opened_at_that_files_line() {
    let folder = folder("an_included_file");
    let record = folder.join("record");
    fake_editor(&folder, "subl", &record);
    let preview = Preview::start(on_path(&folder));
    let start = content(PART).find("\\textbf").unwrap();
    assert_eq!(open(preview.address(), "doc/part.flm", start), (204, String::new()));
    assert_eq!(recorded(&record), [format!("{}:7:5", given("doc/part.flm"))]);
    preview.stop();
}

#[test]
fn a_place_far_into_a_long_file_is_opened_at_its_line() {
    let folder = folder("a_long_file");
    let record = folder.join("record");
    fake_editor(&folder, "code", &record);
    // A front matter of three lines, then 12 000 paragraphs of 52 bytes each (624 000
    // bytes), then `\emph`.
    let filler = "Some words of a long document, one more paragraph.\n\n";
    assert_eq!(filler.len(), 52);
    let paper = format!("---\ntitle: Long\n---\n{}Here \\emph{{far}}.\n", filler.repeat(12_000));
    let start = content(&paper).find("\\emph").unwrap();
    assert!(start > 500_000);
    let preview = Preview::of(&paper, on_path(&folder));
    assert_eq!(open(preview.address(), "doc/paper.flm", start), (204, String::new()));
    // Three lines of front matter, two lines per paragraph, then the line of `\emph`.
    assert_eq!(recorded(&record), ["--goto", &format!("{}:{}:6", given("doc/paper.flm"), 3 + 2 * 12_000 + 1)]);
    preview.stop();
}

#[test]
fn a_file_the_build_did_not_read_is_never_opened() {
    let folder = folder("a_file_not_read");
    let record = folder.join("record");
    fake_editor(&folder, "code", &record);
    let preview = Preview::start(on_path(&folder));
    for file in ["doc/other.flm", "/etc/hosts", "doc/../doc/paper.flm", ""] {
        let (status, message) = open(preview.address(), file, 0);
        assert_eq!(status, 403, "{file}");
        assert!(message.contains("last build read"), "{message}");
    }
    let (status, _) = open(preview.address(), "doc/paper.flm", PAPER.len() * 2);
    assert_eq!(status, 404);
    assert!(!record.exists(), "the editor was not run");
    preview.stop();
}

/// Writes, in `folder`, a file named `name` (a path relative to `folder`) that cannot be
/// executed.
fn not_executable(folder: &std::path::Path, name: &str) {
    let path = folder.join(name);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "#!/bin/sh\n").unwrap();
}

#[test]
fn every_editor_is_looked_for_on_the_search_path_then_among_the_applications() {
    let folder = folder("every_editor");
    let (path, apps, more_apps) = (folder.join("path"), folder.join("Applications"), folder.join("home/Applications"));
    let record = folder.join("record");
    for name in ["subl", "gvim"] {
        fake_editor(&path, name, &record);
    }
    // In the application folders: the preview release of Zed in the first and its release
    // in the second; Emacs in the second; Sublime Text, also found on the search path;
    // Visual Studio Code's program as a file that cannot be executed.
    fake_editor(&apps, "Zed Preview.app/Contents/MacOS/cli", &record);
    fake_editor(&more_apps, "Zed.app/Contents/MacOS/cli", &record);
    fake_editor(&more_apps, "Emacs.app/Contents/MacOS/bin/emacsclient", &record);
    fake_editor(&apps, "Sublime Text.app/Contents/SharedSupport/bin/subl", &record);
    not_executable(&apps, "Visual Studio Code.app/Contents/Resources/app/bin/code");
    assert_eq!(EDITORS, ["code", "zed", "emacsclient", "subl", "gvim"]);
    let editors = Editors::find(None, Some(path.as_os_str()), &[apps.clone(), more_apps.clone()]).unwrap();
    let found: Vec<(&str, &str, PathBuf)> =
        editors.iter().map(|editor| (editor.name(), editor.label(), editor.program().to_path_buf())).collect();
    assert_eq!(
        found,
        [
            ("zed", "Zed", more_apps.join("Zed.app/Contents/MacOS/cli")),
            ("emacsclient", "Emacs", more_apps.join("Emacs.app/Contents/MacOS/bin/emacsclient")),
            ("subl", "Sublime Text", path.join("subl")),
            ("gvim", "gVim", path.join("gvim")),
        ]
    );
    assert_eq!(editors.get("subl").unwrap().program(), path.join("subl"));
    assert_eq!(editors.get("code"), None);
    // Without application folders, the search path alone.
    let names = |editors: &Editors| editors.iter().map(|editor| editor.name()).collect::<Vec<_>>();
    assert_eq!(names(&on_path(&path)), ["subl", "gvim"]);
    // The preview release of Zed, when Zed's release is in no folder.
    let editors = Editors::find(None, Some(path.as_os_str()), std::slice::from_ref(&apps)).unwrap();
    assert_eq!(editors.get("zed").unwrap().program(), apps.join("Zed Preview.app/Contents/MacOS/cli"));
}

#[test]
fn the_default_is_the_editor_preferred_when_it_is_found_else_the_first_found() {
    let folder = folder("the_default");
    let record = folder.join("record");
    for name in ["subl", "gvim"] {
        fake_editor(&folder, name, &record);
    }
    let path = folder.as_os_str();
    let default = |preferred: Option<&str>| {
        Editors::find(preferred, Some(path), &[]).unwrap().default().map(|editor| editor.name())
    };
    assert_eq!(default(None), Some("subl"));
    assert_eq!(default(Some("gvim")), Some("gvim"));
    // An editor preferred and not found gives way to the first found.
    assert_eq!(default(Some("code")), Some("subl"));
    // A name that is no editor's, `vim` among them, is refused.
    for name in ["nano", "vim"] {
        assert_eq!(Editors::find(Some(name), Some(path), &[]), Err(UnknownEditor { name: String::from(name) }));
    }
    // No editor found: no default.
    let none = Editors::find(Some("subl"), Some(folder.join("nothing").as_os_str()), &[]).unwrap();
    assert!(none.is_empty());
    assert_eq!(none.default(), None);
}

#[test]
fn a_place_is_opened_in_the_editor_the_page_asks_for_or_else_in_the_default() {
    let folder = folder("the_editor_asked_for");
    let (code, zed) = (folder.join("code-record"), folder.join("zed-record"));
    fake_editor(&folder, "code", &code);
    fake_editor(&folder, "zed", &zed);
    let preview = Preview::start(Editors::find(Some("zed"), Some(folder.as_os_str()), &[]).unwrap());
    let start = content(PAPER).find("\\emph").unwrap();
    let file = given("doc/paper.flm");
    // The editor asked for.
    assert_eq!(open_in(preview.address(), "doc/paper.flm", start, "code"), (204, String::new()));
    assert_eq!(recorded(&code), ["--goto", &format!("{file}:4:16")]);
    assert_eq!(preview.opening(), Opening::Opened { editor: "code", file: PathBuf::from(&file), line: 4, column: 16 });
    // None asked for: the default.
    assert_eq!(open(preview.address(), "doc/paper.flm", start), (204, String::new()));
    assert_eq!(recorded(&zed), [format!("{file}:4:16")]);
    assert_eq!(preview.opening(), Opening::Opened { editor: "zed", file: PathBuf::from(&file), line: 4, column: 16 });
    std::fs::remove_file(&code).unwrap();
    std::fs::remove_file(&zed).unwrap();
    // An editor that was not found, or that is no editor: nothing is opened.
    for name in ["subl", "vim", ""] {
        let (status, message) = open_in(preview.address(), "doc/paper.flm", start, name);
        assert_eq!(status, 400, "{name}");
        assert_eq!(message, format!("the editor `{name}` must be one of the editors found on this machine: `code`, `zed`"));
        assert_eq!(preview.opening(), Opening::Refused { reason: message });
    }
    // An editor's name that is not a text: a request that is not understood.
    let body = serde_json::json!({ "file": "doc/paper.flm", "start": start, "editor": 3 }).to_string();
    let (status, message) = ask(preview.address(), "POST", OPEN, body.as_bytes());
    assert_eq!(status, 400);
    assert!(message.contains("the name of an editor (`editor`)"), "{message}");
    assert!(!code.exists() && !zed.exists(), "no editor was run");
    preview.stop();
}

#[test]
fn no_request_to_open_a_place_stalls_or_stops_the_server() {
    let preview = Preview::start(Editors::none());
    let address = preview.address();
    let start = content(PAPER).find("\\emph").unwrap();
    // No editor was found, whether the request asks for one or not.
    let (status, message) = open(address, "doc/paper.flm", start);
    assert_eq!(status, 503);
    assert_eq!(message, Editors::none().none_found());
    assert_eq!(open_in(address, "doc/paper.flm", start, "code"), (503, message));
    // Requests that are not understood.
    let bodies: [&[u8]; 7] = [
        b"",
        b"not json",
        b"[]",
        b"{\"file\": \"doc/paper.flm\"}",
        b"{\"file\": \"doc/paper.flm\", \"start\": -1}",
        b"{\"file\": 3, \"start\": 0}",
        b"{\"file\": \"doc/paper.flm\", \"start\": 1.5}",
    ];
    for body in bodies {
        assert_eq!(ask(address, "POST", OPEN, body).0, 400, "{}", String::from_utf8_lossy(body));
    }
    let long = vec![b' '; 200 * 1024];
    assert_eq!(ask(address, "POST", OPEN, &long).0, 400);
    assert_eq!(ask(address, "GET", OPEN, b"").0, 405);
    // A connection that sends half a request and goes away.
    {
        let mut connection = TcpStream::connect(address).unwrap();
        let _ = connection.write_all(b"POST /.flm-preview/open HTTP/1.1\r\nContent-Length: 100\r\n\r\n{\"fi");
    }
    // The server still answers.
    assert_eq!(ask(address, "GET", "/", b"").0, 200);
    assert_eq!(open(address, "doc/paper.flm", start).0, 503);
    preview.stop();
}

/// The first build of `text` as `doc/paper.flm`: its update as the page reads it, and its
/// page.
fn first_build(text: &str) -> (serde_json::Value, String) {
    let mut files = MemoryFiles::new();
    files.insert("doc/paper.flm", text.as_bytes());
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.naming = Naming::Folder;
    let watch = Watch::new(request, Arc::new(files), Vec::new());
    let handle = watch.handle();
    let mut first = None;
    watch
        .run(|event| {
            if let Event::Built(built) = event {
                let page = built.output.as_ref().and_then(|output| output.entry()).expect("a page").bytes.clone();
                first = Some((built.update.to_json(), String::from_utf8(page).unwrap()));
                handle.stop();
            }
        })
        .expect("the loop ran");
    first.expect("a build")
}

#[test]
fn each_update_carries_the_label_of_each_element_a_label_names() {
    let (update, page) = first_build(
        "\\section{Intro}\\label{sec:intro}\\label{sec:a-first}\n\nText.\n\n\\section{Next}\\label{sec:next}\n\nMore, \
         see \\ref{sec:intro}.\n\n\\section{Plain}\n\nNo label.\n",
    );
    let references = update["references"].as_object().expect("the labels");
    // One entry per element a label names: of several labels, the first in alphabetical order.
    let mut labels: Vec<&str> = references.values().map(|label| label.as_str().unwrap()).collect();
    labels.sort();
    assert_eq!(labels, ["sec:a-first", "sec:next"]);
    for (id, _) in references {
        assert!(page.contains(&format!("id=\"{id}\"")), "the page has an element `{id}`");
    }
}
