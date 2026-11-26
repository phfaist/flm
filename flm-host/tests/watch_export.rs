//! The export of the live preview (the module `watch::export` and the server's requests of
//! `/.flm-preview/export`), over documents written under the build folder: a save writes what
//! a build named as `flm render -o` names it writes, through the same sink; each case in
//! which nothing is written, among them an error of a stage that writes files; a download of several files is one archive; the name, the
//! command lines and the opening of the exported file through the program's services.

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use flm_core::progress::Scope;
use flm_host::files::DiskFiles;
use flm_host::files::system::from_system;
use flm_host::output::{DiskSink, Naming, OutputFolder, Sink};
use flm_host::packages::ExtensionFolder;
use flm_host::session::{Input, Request, build, prepare};
use flm_host::watch::editor::Editors;
use flm_host::watch::server::{Port, Server};
use flm_host::watch::services::{OpenFileError, Services};
use flm_host::watch::settings::DocumentSettings;
use flm_host::watch::{Event, Handle, Watch};
use serde_json::{Value, json};

/// The upper bound of every wait.
const BOUND: Duration = Duration::from_secs(60);

/// A fresh folder for one test, under the build folder, as the host writes paths.
fn folder(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-host-export-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// The path `path` as the host writes paths: its FLM form.
fn host(path: &Path) -> String {
    from_system(path).expect("the path is text")
}

/// The request of the document `paper.flm` in `dir`, as a program gives it to the watch loop.
fn request(dir: &Path) -> Request {
    let mut request = Request::new(Input::File(host(&dir.join("paper.flm"))));
    request.default_math = flm_host::seed::MathChoice::processor("passthrough");
    request.naming = Naming::Folder;
    request.extension_folders = vec![ExtensionFolder { path: host(&dir.join("_flmextensions")), required: false }];
    request
}

/// The services of a program that composes a command line of its own and notes the files it
/// is asked to open.
#[derive(Clone, Default)]
struct Noting {
    opened: Arc<Mutex<Vec<PathBuf>>>,
}

impl Services for Noting {
    fn export_command(&self, profile: &str, name: &str, continue_with_errors: bool) -> Option<String> {
        let continuing = if continue_with_errors { " --continue-with-errors" } else { "" };
        Some(format!("render -p {profile}{continuing} -o {name}"))
    }

    fn open_file(&self, path: &Path) -> Result<(), OpenFileError> {
        self.opened.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }
}

/// The preview of the document `paper.flm` in `dir`, at a port the system chooses.
struct Preview {
    server: Arc<Server>,
    handle: Handle,
    built: Receiver<u64>,
}

impl Preview {
    fn start(dir: &Path, services: impl Services + 'static) -> Preview {
        Preview::start_with(request(dir), dir, services)
    }

    fn start_with(request: Request, dir: &Path, services: impl Services + 'static) -> Preview {
        let document = host(&dir.join("paper.flm"));
        let watch = Watch::new(request, Arc::new(DiskFiles), Vec::new());
        let handle = watch.handle();
        let settings = DocumentSettings::on_disk(json!({}), document.clone());
        let server = Arc::new(
            Server::start(Port::Exactly(0), document, handle.clone(), Editors::none(), |_| {}, settings, services)
                .expect("the server starts"),
        );
        let (send, built) = channel();
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
        });
        let preview = Preview { server, handle, built };
        preview.built.recv_timeout(BOUND).expect("the first build within the bound");
        preview
    }

    fn address(&self) -> SocketAddr {
        self.server.address()
    }
}

impl Drop for Preview {
    fn drop(&mut self) {
        self.handle.stop();
    }
}

/// An answer: its status, its headers (names in lower case) and its body.
struct Answer {
    status: u16,
    headers: Vec<(String, String)>,
    body: Vec<u8>,
}

impl Answer {
    fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(key, _)| key == name).map(|(_, value)| value.as_str())
    }

    fn text(&self) -> String {
        String::from_utf8(self.body.clone()).unwrap()
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&self.body).unwrap_or_else(|_| panic!("JSON: {}", self.text()))
    }
}

/// Sends `method target` with the body `body` to `address`, and reads the answer.
fn send(address: SocketAddr, method: &str, target: &str, body: &str) -> Answer {
    let mut connection = TcpStream::connect(address).expect("the server is reached");
    connection.set_read_timeout(Some(BOUND)).unwrap();
    let length = if method == "POST" { format!("Content-Length: {}\r\n", body.len()) } else { String::new() };
    write!(connection, "{method} {target} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n{length}\r\n{body}").unwrap();
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

/// `POST /.flm-preview/export` of the build type `profile`, the action `action`.
fn export(preview: &Preview, profile: &str, action: &str, continue_with_errors: bool) -> Answer {
    let body = json!({ "profile": profile, "action": action, "continue_with_errors": continue_with_errors });
    send(preview.address(), "POST", "/.flm-preview/export", &body.to_string())
}

/// The names of the files in `dir`, sorted, hidden files included.
fn names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> =
        std::fs::read_dir(dir).unwrap().map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned()).collect();
    names.sort();
    names
}

/// A workflow of two outputs, `html` and `text`, in an extension beside the document.
fn with_both(dir: &Path) {
    std::fs::create_dir_all(dir.join("_flmextensions")).unwrap();
    std::fs::write(
        dir.join("_flmextensions/both.yaml"),
        "flm:\n  workflows:\n    both:\n      extends: html\n      outputs: [html, text]\n",
    )
    .unwrap();
}

const PAPER: &str = "---\ntitle: A paper\nflm: {use: [both]}\n---\nHello \\emph{world}.\n";

#[test]
fn a_saved_export_writes_what_a_build_named_as_render_o_names_it_writes() {
    let dir = folder("saved");
    with_both(&dir);
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    let preview = Preview::start(&dir, Noting::default());

    let saved = export(&preview, "both", "save", false);
    assert_eq!(saved.status, 200, "{}", saved.text());
    assert_eq!(saved.json(), json!({ "name": "paper.html", "files": ["paper.html", "paper.txt"] }));

    // What `flm render -p both -o paper.html paper.flm` writes, built here as it builds it:
    // the request as given, the files named after `paper.html`, written by the disk's sink.
    let other = folder("saved-render");
    with_both(&other);
    std::fs::write(other.join("paper.flm"), PAPER).unwrap();
    let mut rendered = request(&other);
    rendered.profile = Some(String::from("both"));
    rendered.naming = Naming::File { name: String::from("paper.html") };
    rendered.output_folder = OutputFolder::new(host(&other));
    rendered.sink = Some(Arc::new(DiskSink::new(host(&other))) as Arc<dyn Sink>);
    build(&prepare(rendered, Arc::new(DiskFiles), &[], &Scope::none()).unwrap(), &Scope::none()).unwrap();
    assert_eq!(names(&dir), names(&other));
    for name in ["paper.html", "paper.txt"] {
        assert_eq!(std::fs::read(dir.join(name)).unwrap(), std::fs::read(other.join(name)).unwrap(), "{name}");
    }
    // None of the preview's own settings: no place in the source, no page script.
    let page = std::fs::read_to_string(dir.join("paper.html")).unwrap();
    assert!(page.contains("world") && !page.contains("data-flm-source-span"), "{page}");

    // The main file of the last export saved opens through the program's services.
    let services = Noting::default();
    let opened = Arc::clone(&services.opened);
    drop(preview);
    let preview = Preview::start(&dir, services);
    assert_eq!(send(preview.address(), "POST", "/.flm-preview/export/open", "").status, 404);
    assert_eq!(export(&preview, "both", "save", false).status, 200);
    assert_eq!(send(preview.address(), "POST", "/.flm-preview/export/open", "").status, 204);
    assert_eq!(*opened.lock().unwrap(), [dir.join("paper.html")]);
}

#[test]
fn the_name_and_the_command_lines_come_from_the_build_type_and_the_program() {
    let dir = folder("named");
    with_both(&dir);
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    let preview = Preview::start(&dir, Noting::default());
    let named = send(preview.address(), "GET", "/.flm-preview/export?profile=text", "");
    assert_eq!(named.status, 200, "{}", named.text());
    assert_eq!(
        named.json(),
        json!({
            "profile": "text",
            "name": "paper.txt",
            "command": "render -p text -o paper.txt",
            "command_continuing": "render -p text --continue-with-errors -o paper.txt",
        })
    );
    // A program that composes none gives none.
    drop(preview);
    let preview = Preview::start(&dir, flm_host::watch::services::NoServices);
    let named = send(preview.address(), "GET", "/.flm-preview/export?profile=both", "").json();
    assert_eq!((named["name"].clone(), named["command"].clone()), (json!("paper.html"), Value::Null));
    assert_eq!(send(preview.address(), "POST", "/.flm-preview/export/open", "").status, 404);
    assert_eq!(export(&preview, "both", "save", false).status, 200);
    assert_eq!(send(preview.address(), "POST", "/.flm-preview/export/open", "").status, 501);
    assert_eq!(send(preview.address(), "GET", "/.flm-preview/export", "").status, 400);
}

#[test]
fn nothing_is_written_when_the_build_type_has_no_output_or_the_document_has_errors_or_the_build_fails() {
    let dir = folder("refused");
    std::fs::write(dir.join("paper.flm"), "Hello \\nosuchcommand{world}.\n").unwrap();
    let preview = Preview::start(&dir, Noting::default());

    // A build type without output.
    assert_eq!(send(preview.address(), "GET", "/.flm-preview/export?profile=check", "").status, 409);
    let answer = export(&preview, "check", "save", true);
    assert_eq!(answer.status, 409);
    assert_eq!(answer.text().lines().count(), 1, "{}", answer.text());

    // A document with errors, unless the export continues with them: the errors follow the
    // reason, after a blank line.
    let answer = export(&preview, "html", "save", false);
    assert_eq!(answer.status, 428, "{}", answer.text());
    assert!(answer.text().lines().nth(1) == Some("") && answer.text().lines().count() > 2, "{}", answer.text());
    assert_eq!(export(&preview, "html", "download", false).status, 428);
    assert_eq!(names(&dir), ["paper.flm"]);

    // A build type that cannot be prepared.
    let answer = export(&preview, "nosuchtype", "save", true);
    assert_eq!(answer.status, 409, "{}", answer.text());
    assert_eq!(names(&dir), ["paper.flm"]);

    // Through the errors, the export is written.
    let answer = export(&preview, "html", "save", true);
    assert_eq!(answer.status, 200, "{}", answer.text());
    assert_eq!(names(&dir), [".paper.html.flm-written", "paper.flm", "paper.html"]);

    // A body that orders no export.
    let refused = send(preview.address(), "POST", "/.flm-preview/export", "{\"profile\": \"html\", \"action\": \"print\"}");
    assert_eq!(refused.status, 400);
}

/// A build type whose workflow names an unknown output in `write.output_names`: the stage
/// that writes the files reports an error, after the error gate, on a document without
/// errors.
#[test]
fn nothing_is_written_when_a_stage_that_writes_files_reports_an_error() {
    let dir = folder("misnamed");
    std::fs::create_dir_all(dir.join("_flmextensions")).unwrap();
    std::fs::write(
        dir.join("_flmextensions/misnamed.yaml"),
        "flm:\n  workflows:\n    misnamed:\n      extends: html\n      settings:\n        write: {output_names: {nosuchoutput: other.html}}\n",
    )
    .unwrap();
    std::fs::write(dir.join("paper.flm"), "---\nflm: {use: [misnamed]}\n---\nHello \\emph{world}.\n").unwrap();
    let preview = Preview::start(&dir, Noting::default());
    for action in ["save", "download"] {
        let answer = export(&preview, "misnamed", action, false);
        assert_eq!(answer.status, 409, "{}", answer.text());
        assert!(answer.text().lines().nth(1) == Some("") && answer.text().lines().count() > 2, "{}", answer.text());
    }
    assert_eq!(names(&dir), ["_flmextensions", "paper.flm"]);
}

#[test]
fn nothing_is_written_over_a_file_that_the_document_s_build_reads() {
    let dir = folder("read");
    std::fs::write(dir.join("paper.flm"), "Main \\input{paper.html}\n").unwrap();
    std::fs::write(dir.join("paper.html"), "included\n").unwrap();
    let preview = Preview::start(&dir, Noting::default());
    let answer = export(&preview, "html", "save", false);
    assert_eq!(answer.status, 409, "{}", answer.text());
    assert_eq!(std::fs::read_to_string(dir.join("paper.html")).unwrap(), "included\n");
    // A download writes nothing, and is carried out.
    let answer = export(&preview, "html", "download", false);
    assert_eq!(answer.status, 200, "{}", answer.text());
    assert!(answer.text().contains("included"), "{}", answer.text());
}

#[test]
fn a_download_of_one_file_is_the_file_and_of_several_files_one_archive() {
    let dir = folder("download");
    with_both(&dir);
    std::fs::write(dir.join("paper.flm"), PAPER).unwrap();
    let preview = Preview::start(&dir, Noting::default());

    let one = export(&preview, "text", "download", false);
    assert_eq!(one.status, 200, "{}", one.text());
    assert!(one.header("content-type").is_some_and(|value| value.starts_with("text/plain")), "{:?}", one.headers);
    assert_eq!(one.header("content-disposition"), Some("attachment; filename*=UTF-8''paper.txt"));
    assert!(one.text().contains("world"), "{}", one.text());

    let several = export(&preview, "both", "download", false);
    assert_eq!(several.status, 200, "{}", several.text());
    assert_eq!(several.header("content-type"), Some("application/zip"));
    assert_eq!(several.header("content-disposition"), Some("attachment; filename*=UTF-8''paper.zip"));
    let mut archive = zip::ZipArchive::new(std::io::Cursor::new(several.body)).expect("a zip archive");
    let mut paths: Vec<String> = archive.file_names().map(|name| name.unwrap().into_owned()).collect();
    paths.sort();
    assert_eq!(paths, ["paper.html", "paper.txt"]);
    let mut text = String::new();
    archive.by_name("paper.txt").unwrap().read_to_string(&mut text).unwrap();
    assert!(text.contains("world"), "{text}");
    // Nothing was written.
    assert_eq!(names(&dir), ["_flmextensions", "paper.flm"]);
}
