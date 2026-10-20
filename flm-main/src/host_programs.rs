//! The host service `programs`: finding a program installed on the machine and running it
//! in a fresh temporary folder, for an extension's handlers.
//!
//! An extension declares the host services its handlers call (the key `host` of a package,
//! [`flm_ext::host`]); a handler calls this one as `cx.host("programs").call(method,
//! args)`. The `flm` program offers it to every document; a program embedding this library
//! offers it only when it asks, by setting
//! [`Request::offer_programs`](crate::session::Request::offer_programs), which makes
//! [`packages::loader`](crate::packages::loader) register a [`ProgramsService`] beside the
//! `files` service. Without it, the loader refuses a package that declares `programs`.
//!
//! # The two methods
//!
//! `find` answers the path of a program, or `null`:
//!
//! | Arguments | Answer |
//! |---|---|
//! | `{ "names": [text, …], "folders": [text, …] }` (`folders` optional) | the path, a text, or `null` |
//!
//! The names are tried in order. For each name, the folders of the environment variable
//! `PATH` are searched first (on Windows, with each extension of `PATHEXT` appended to a name
//! that has none), then the folders of `folders`, in their order; the first file found that
//! is a program (on Unix, a file with an execute permission) answers its path. A name that
//! holds a folder separator is taken as a path: it answers itself when it is a program. In a
//! folder of `folders`, `*` stands for any run of characters inside one folder name, never a
//! separator (`/opt/gs*/bin` matches `/opt/gs10.02.1/bin`; several `*` in one name are
//! allowed; no other pattern is). Where several folders match, they are searched in
//! descending natural order: runs of digits are compared as numbers, so `gs10.02.1` comes
//! before `gs9.56.1`, the newest version first.
//!
//! `run` runs a program and answers what it wrote:
//!
//! | Argument | Value | Default |
//! |---|---|---|
//! | `program` | a path (as `find` answers) or a name, looked up in the folders of `PATH` | required |
//! | `args` | the command-line arguments, a list of texts | none |
//! | `files` | the files written into the folder first: a map from a relative path to `{ "text": text }` or `{ "base64": text }` | none |
//! | `stdin` | a text written to the program's standard input | nothing (the input is empty) |
//! | `collect` | the relative paths of the files read back from the folder after the run, a list of texts | none |
//! | `timeout_ms` | the time the program may take, in milliseconds, a positive whole number | 60000 |
//!
//! The answer is `{ "status", "stdout", "stderr", "files" }`: `status` the program's exit
//! status (`null` when it was ended by a signal), `stdout` and `stderr` the text it wrote
//! (bytes that are not UTF-8 replaced by `U+FFFD`), and `files` a map from each path of
//! `collect` at which a file (not a folder) exists after the run to `{ "text" }` when the file is UTF-8
//! text and `{ "base64" }` otherwise. A program that exits with a status other than 0 is
//! an answer like any other; the script reads `status`.
//!
//! Base64, in `files` and in the answer, is written in the standard alphabet with padding.
//! The two forms `{ "text" }` and `{ "base64" }` are those in which an extension writes and
//! reads the content of an output's file
//! ([the data form of an artifact](flm_ext::api::core::artifact#the-data-form-of-an-artifact)),
//! so an extension passes a file between an output and a program without converting it.
//!
//! The service makes a fresh folder under the system's temporary folder, writes `files`
//! into it, starts the program there (its working folder) with the host's environment,
//! waits for it and for the end of its output at most `timeout_ms`, reads `collect`, and
//! removes the folder, whatever happened. A path of `files` or `collect` is relative to the
//! folder, written with `/`; one that is absolute, leaves the folder or names the folder
//! itself once `.` and `name/..` are removed is refused. A program that cannot be found or started, or that
//! runs past its time (it is then stopped), is an error the script receives, naming the
//! extension and the program.
//!
//! # Stopping a program and the programs it started
//!
//! A program often runs other programs: `latexmk` runs LuaLaTeX, for instance. On Unix,
//! the service starts the program in a process group of its own, which the programs it
//! starts join, and a program past its time is stopped together with every program of
//! that group. The time also covers the end of the program's output, so a program that has
//! ended while a program it started still holds its output open is past its time too, and
//! that group is stopped as well. On other systems, only the program that the service
//! started is stopped, and the programs it started keep running.
//!
//! A signal sent to the host's process group does not reach a program in a group of its
//! own. Such signals ask the host to end: the interrupt that Ctrl+C sends to the terminal's
//! foreground process group (`SIGINT`), the quit signal that `Ctrl+\` sends to that group
//! (`SIGQUIT`), the hang-up sent when the terminal closes (`SIGHUP`), and `SIGTERM`, which a
//! program that supervises a command (an editor stopping a build, for instance) sends to the
//! group it started the command in. A host that handles these signals therefore calls
//! [`stop_running_programs`] before it ends, which stops every program that the service is
//! running, with the programs each one started; the `flm` program does. On other systems
//! the program is started as any other, and the function does nothing.
//!
//! # What the service does not do
//!
//! A run is not cached: every call starts the program again. A run is not watched: the
//! program and the files it reads outside its folder are not among the files a build read,
//! so the live preview does not build again when they change. The program is not confined:
//! it runs with the rights of the user running the host, reads and writes wherever that
//! user may, and the temporary folder only gives it a place to work.
//!
//! # Trust
//!
//! An extension runs code, and through this service it can run any program on the machine
//! with the user's rights. Extensions are therefore installed from trusted sources only. A
//! document's folder `_flmextensions` comes with the document: building a document received
//! from someone else runs the extensions in its folder, which, when the program offers this
//! service, may run programs.

use std::cmp::Ordering;
use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::mpsc;
#[cfg(unix)]
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use flm_ext::host::{HostAnswer, HostCaller, HostService, HostServiceError};
use serde_json::{Map, Value};

use crate::files::fold;

/// The service's name, as a package declares it.
pub const NAME: &str = "programs";
/// The method answering the path of a program.
pub const FIND: &str = "find";
/// The method running a program.
pub const RUN: &str = "run";
/// The time a program may take when `run` gives no `timeout_ms`: 60 seconds.
pub const DEFAULT_TIMEOUT_MS: u64 = 60_000;

/// The host service `programs`: finds a program on the machine and runs it in a fresh
/// temporary folder. See the [module documentation](self).
#[derive(Debug, Clone, Copy, Default)]
pub struct ProgramsService;

impl ProgramsService {
    /// Returns the service.
    pub fn new() -> ProgramsService {
        ProgramsService
    }
}

impl HostService for ProgramsService {
    /// `programs`.
    fn name(&self) -> &str {
        NAME
    }

    /// `find` or `run`; the caller's extension is named in the messages of `run`.
    fn call(&self, caller: &HostCaller, method: &str, args: &Value) -> Result<HostAnswer, HostServiceError> {
        match method {
            FIND => find(args),
            RUN => run(caller, args),
            other => Err(refused(format!("the method of the host service `programs` must be `find` or `run`, not `{other}`"))),
        }
    }
}

/// A refusal with the text `detail`.
fn refused(detail: String) -> HostServiceError {
    HostServiceError { detail }
}

/// The texts of the list under `key` of `args`, an empty list when absent; `None` when the
/// value is not a list of texts.
fn texts(args: &Value, key: &str) -> Option<Vec<String>> {
    match args.get(key) {
        None | Some(Value::Null) => Some(Vec::new()),
        Some(Value::Array(items)) => items.iter().map(|item| item.as_str().map(str::to_string)).collect(),
        Some(_) => None,
    }
}

/// The refusal of the argument `key`, which must be `what`.
fn wrong(key: &str, what: &str) -> HostServiceError {
    refused(format!("`{key}` must be {what}"))
}

// --- find ----------------------------------------------------------------------------------

/// `find { names, folders }`.
fn find(args: &Value) -> Result<HostAnswer, HostServiceError> {
    let names = match args.get("names") {
        Some(Value::Array(_)) => texts(args, "names"),
        _ => None,
    }
    .ok_or_else(|| wrong("names", "a list of texts"))?;
    let folders = texts(args, "folders").ok_or_else(|| wrong("folders", "a list of texts"))?;
    let found = find_program(&names, &folders).and_then(|path| path.into_os_string().into_string().ok());
    Ok(HostAnswer::Data(found.map_or(Value::Null, Value::String)))
}

/// The first of `names` found in the folders of `PATH`, then of `folders` (with their
/// wildcards), as the [module documentation](self) says.
fn find_program(names: &[String], folders: &[String]) -> Option<PathBuf> {
    let path_folders: Vec<PathBuf> = std::env::var_os("PATH").map(|path| std::env::split_paths(&path).collect()).unwrap_or_default();
    let extra: Vec<PathBuf> = folders.iter().flat_map(|folder| expand(folder)).collect();
    names.iter().find_map(|name| {
        if holds_separator(name) {
            let path = PathBuf::from(name);
            return is_program(&path).then_some(path);
        }
        // A program whose path is not UTF-8 cannot be answered as a text: the search goes on.
        path_folders
            .iter()
            .chain(&extra)
            .find_map(|folder| candidates(folder, name).into_iter().find(|path| path.to_str().is_some() && is_program(path)))
    })
}

/// Whether `name` holds a folder separator, and so is a path.
fn holds_separator(name: &str) -> bool {
    name.contains('/') || (cfg!(windows) && name.contains('\\'))
}

/// The files that would be the program `name` in `folder`: the name itself, and on Windows
/// the name with each extension of `PATHEXT` appended.
fn candidates(folder: &Path, name: &str) -> Vec<PathBuf> {
    let mut paths = vec![folder.join(name)];
    if cfg!(windows) {
        let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| String::from(".COM;.EXE;.BAT;.CMD"));
        let has_extension = Path::new(name).extension().is_some();
        if !has_extension {
            paths.clear();
        }
        for extension in extensions.split(';').filter(|extension| !extension.is_empty()) {
            let mut file = OsString::from(name);
            file.push(extension);
            paths.push(folder.join(file));
        }
    }
    paths
}

/// Whether `path` is a file the system can start: a file, with an execute permission on
/// Unix.
fn is_program(path: &Path) -> bool {
    let Ok(metadata) = std::fs::metadata(path) else { return false };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// The folders `folder` names: itself when it holds no `*`, else every existing folder its
/// wildcards match, the matches of each folder name in descending natural order.
fn expand(folder: &str) -> Vec<PathBuf> {
    if !folder.contains('*') {
        return vec![PathBuf::from(folder)];
    }
    let mut current = vec![PathBuf::new()];
    for component in Path::new(folder).components() {
        let pattern = match component {
            Component::Normal(name) => name.to_str().filter(|name| name.contains('*')),
            _ => None,
        };
        let Some(pattern) = pattern else {
            for base in &mut current {
                base.push(component.as_os_str());
            }
            continue;
        };
        let mut next = Vec::new();
        for base in current {
            let listed = if base.as_os_str().is_empty() { Path::new(".") } else { base.as_path() };
            let Ok(entries) = std::fs::read_dir(listed) else { continue };
            let mut matched: Vec<String> = entries
                .filter_map(Result::ok)
                .filter_map(|entry| entry.file_name().into_string().ok())
                .filter(|name| matches(pattern, name) && base.join(name).is_dir())
                .collect();
            matched.sort_by(|a, b| natural(b, a));
            next.extend(matched.into_iter().map(|name| base.join(name)));
        }
        current = next;
    }
    current
}

/// Whether `name` matches `pattern`, in which each `*` stands for any run of characters.
fn matches(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if parts.len() == 1 {
        return name == pattern;
    }
    if !name.starts_with(first) || name.len() < first.len() + last.len() {
        return false;
    }
    let mut position = first.len();
    for part in &parts[1..parts.len() - 1] {
        match name[position..].find(part) {
            Some(found) => position += found + part.len(),
            None => return false,
        }
    }
    name.len() >= position + last.len() && name.ends_with(last)
}

/// The natural order of `a` and `b`: runs of ASCII digits compared as numbers, other
/// characters byte by byte.
fn natural(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.as_bytes(), b.as_bytes());
    loop {
        match (a.first(), b.first()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let run = |text: &[u8]| text.iter().take_while(|byte| byte.is_ascii_digit()).count();
                let (length_a, length_b) = (run(a), run(b));
                let trim = |digits: &[u8]| {
                    let zeros = digits.iter().take_while(|byte| **byte == b'0').count();
                    digits[zeros..].to_vec()
                };
                let (number_a, number_b) = (trim(&a[..length_a]), trim(&b[..length_b]));
                let order = number_a.len().cmp(&number_b.len()).then_with(|| number_a.cmp(&number_b));
                if order != Ordering::Equal {
                    return order;
                }
                (a, b) = (&a[length_a..], &b[length_b..]);
            }
            (Some(x), Some(y)) => {
                if x != y {
                    return x.cmp(y);
                }
                (a, b) = (&a[1..], &b[1..]);
            }
        }
    }
}

// --- run -----------------------------------------------------------------------------------

/// The arguments of `run`, read.
struct RunArgs {
    program: String,
    args: Vec<String>,
    files: Vec<(String, Vec<u8>)>,
    stdin: Option<String>,
    collect: Vec<String>,
    timeout: Duration,
}

/// Reads the arguments of `run`.
fn run_args(args: &Value) -> Result<RunArgs, HostServiceError> {
    let program = args.get("program").and_then(Value::as_str).ok_or_else(|| wrong("program", "a text"))?.to_string();
    let arguments = texts(args, "args").ok_or_else(|| wrong("args", "a list of texts"))?;
    let collect = texts(args, "collect").ok_or_else(|| wrong("collect", "a list of texts"))?;
    let stdin = match args.get("stdin") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) => Some(text.clone()),
        Some(_) => return Err(wrong("stdin", "a text")),
    };
    let timeout_ms = match args.get("timeout_ms") {
        None | Some(Value::Null) => DEFAULT_TIMEOUT_MS,
        Some(value) => value.as_u64().filter(|ms| *ms > 0).ok_or_else(|| wrong("timeout_ms", "a positive whole number"))?,
    };
    let files = match args.get("files") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(path, content)| Ok((path.clone(), file_content(&program, path, content)?)))
            .collect::<Result<_, HostServiceError>>()?,
        Some(_) => return Err(wrong("files", "a map from a relative path to `{ text }` or `{ base64 }`")),
    };
    Ok(RunArgs { program, args: arguments, files, stdin, collect, timeout: Duration::from_millis(timeout_ms) })
}

/// The bytes of the file `path` given to `program`, written `content`.
fn file_content(program: &str, path: &str, content: &Value) -> Result<Vec<u8>, HostServiceError> {
    let shape = || refused(format!("the file `{path}` given to the program `{program}` must be `{{ text }}` or `{{ base64 }}`"));
    let Some(map) = content.as_object().filter(|map| map.len() == 1) else { return Err(shape()) };
    match (map.get("text"), map.get("base64")) {
        (Some(Value::String(text)), None) => Ok(text.clone().into_bytes()),
        (None, Some(Value::String(encoded))) => BASE64.decode(encoded).map_err(|_| {
            refused(format!("`base64` of the file `{path}` given to the program `{program}` must be base64 in the standard alphabet, with padding"))
        }),
        _ => Err(shape()),
    }
}

/// `path`, relative to `folder`, joined to it and folded, or the refusal of a path that is
/// absolute, leaves the folder or names the folder itself.
fn inside(folder: &Path, path: &str) -> Result<PathBuf, HostServiceError> {
    let bytes = path.as_bytes();
    let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if Path::new(path).is_absolute() || path.starts_with('/') || path.starts_with('\\') || drive {
        return Err(refused(format!("the path `{path}` must be relative to the program's temporary folder")));
    }
    let full = fold(&folder.join(path));
    let within = full
        .strip_prefix(folder)
        .is_ok_and(|rest| !matches!(rest.components().next(), Some(Component::ParentDir)));
    if !within {
        return Err(refused(format!("the path `{path}` must stay inside the program's temporary folder")));
    }
    if full == folder {
        return Err(refused(format!("the path `{path}` must name a file inside the program's temporary folder")));
    }
    Ok(full)
}

/// A counter making each temporary folder's name unique within the process.
static FOLDERS: AtomicU64 = AtomicU64::new(0);

/// A fresh, empty folder under the system's temporary folder.
fn fresh_folder() -> std::io::Result<PathBuf> {
    let base = fold(&std::env::temp_dir());
    loop {
        let count = FOLDERS.fetch_add(1, AtomicOrdering::Relaxed);
        let folder = base.join(format!("flm-programs-{}-{count}", std::process::id()));
        match std::fs::create_dir(&folder) {
            Ok(()) => return Ok(folder),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
}

/// `run { program, args, files, stdin, collect, timeout_ms }`.
fn run(caller: &HostCaller, args: &Value) -> Result<HostAnswer, HostServiceError> {
    let args = run_args(args)?;
    let stem = &caller.extension.stem;
    let program = &args.program;
    // The paths are checked before anything is made, against a stand-in folder.
    let stand_in = Path::new("folder");
    for path in args.files.iter().map(|(path, _)| path).chain(&args.collect) {
        inside(stand_in, path)?;
    }
    let path = if holds_separator(program) {
        let path = PathBuf::from(program);
        match std::env::current_dir() {
            Ok(current) if path.is_relative() => current.join(path),
            _ => path,
        }
    } else {
        find_program(std::slice::from_ref(program), &[]).ok_or_else(|| {
            refused(format!("the program `{program}` that the extension `{stem}` runs must be in one of the folders of `PATH`"))
        })?
    };
    let folder = fresh_folder().map_err(|error| {
        refused(format!(
            "the temporary folder of the program `{program}` that the extension `{stem}` runs could not be made: {error}"
        ))
    })?;
    let answer = run_in(&folder, &path, &args, stem);
    let _ = std::fs::remove_dir_all(&folder);
    answer
}

/// Runs the program at `path` in `folder` with `args`, for the extension `stem`.
fn run_in(folder: &Path, path: &Path, args: &RunArgs, stem: &str) -> Result<HostAnswer, HostServiceError> {
    let program = &args.program;
    for (file, bytes) in &args.files {
        let full = inside(folder, file)?;
        let written = full
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|()| std::fs::write(&full, bytes));
        written.map_err(|error| {
            refused(format!("the file `{file}` given to the program `{program}` could not be written: {error}"))
        })?;
    }
    let mut command = Command::new(path);
    command
        .args(&args.args)
        .current_dir(folder)
        .stdin(if args.stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // On Unix the program leads a process group of its own, which the programs it starts
    // join, so that `stop` and `stop_running_programs` stop them with it.
    // See decision rationale at [§flm-dd-dr:pipe-tool-stop]
    #[cfg(unix)]
    std::os::unix::process::CommandExt::process_group(&mut command, 0);
    let could_not_start = |error: std::io::Error| {
        refused(format!("the program `{program}` that the extension `{stem}` runs could not be started: {error}"))
    };
    #[cfg(unix)]
    let (mut child, listed) = match Listed::spawn(&mut command) {
        Some(started) => started.map_err(could_not_start)?,
        None => {
            return Err(refused(format!(
                "the program `{program}` that the extension `{stem}` runs could not be started: the host is ending \
                 and has stopped the programs it runs"
            )));
        }
    };
    #[cfg(not(unix))]
    let mut child = command.spawn().map_err(could_not_start)?;
    let deadline = Instant::now() + args.timeout;
    if let (Some(text), Some(mut pipe)) = (args.stdin.clone(), child.stdin.take()) {
        // Written from a thread of its own, so that neither side waits for the other with a
        // full pipe; a program that does not read its input ends the write.
        std::thread::spawn(move || {
            let _ = pipe.write_all(text.as_bytes());
        });
    }
    // The output is read from threads of their own, each sending what it read once the pipe
    // closes; a thread is left behind when the time runs out (it ends with the pipe).
    let (sender, outputs) = mpsc::channel::<(bool, Vec<u8>)>();
    let pipes: [(bool, Option<Box<dyn Read + Send>>); 2] = [
        (true, child.stdout.take().map(|pipe| Box::new(pipe) as Box<dyn Read + Send>)),
        (false, child.stderr.take().map(|pipe| Box::new(pipe) as Box<dyn Read + Send>)),
    ];
    for (is_stdout, pipe) in pipes {
        let sender = sender.clone();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            if let Some(mut pipe) = pipe {
                let _ = pipe.read_to_end(&mut bytes);
            }
            let _ = sender.send((is_stdout, bytes));
        });
    }
    drop(sender);
    let timed_out = || {
        refused(format!(
            "the program `{program}` that the extension `{stem}` runs must finish within {} milliseconds",
            args.timeout.as_millis()
        ))
    };
    let status: ExitStatus = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() >= deadline => {
                stop(&mut child);
                return Err(timed_out());
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(5)),
            Err(error) => {
                stop(&mut child);
                return Err(refused(format!(
                    "the program `{program}` that the extension `{stem}` runs could not be waited for: {error}"
                )));
            }
        }
    };
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    for _ in 0..2 {
        let left = deadline.saturating_duration_since(Instant::now());
        let Ok((is_stdout, bytes)) = outputs.recv_timeout(left) else {
            // The program has ended, and a program it started holds its output open.
            stop(&mut child);
            return Err(timed_out());
        };
        if is_stdout { stdout = bytes } else { stderr = bytes }
    }
    // The program has ended and its output has closed, so its group has, as a rule, no member
    // left, and the system may give its ID to another process: it is unlisted at once.
    #[cfg(unix)]
    drop(listed);
    let mut collected = Map::new();
    for file in &args.collect {
        let full = inside(folder, file)?;
        // Only a file is answered: a path at which there is none, a folder included, is absent.
        if !std::fs::metadata(&full).is_ok_and(|metadata| metadata.is_file()) {
            continue;
        }
        match std::fs::read(&full) {
            Ok(bytes) => {
                collected.insert(file.clone(), file_value(bytes));
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(refused(format!(
                    "the file `{file}` collected from the program `{program}` could not be read: {error}"
                )));
            }
        }
    }
    let mut answer = Map::new();
    answer.insert(String::from("status"), status.code().map_or(Value::Null, Value::from));
    answer.insert(String::from("stdout"), Value::String(String::from_utf8_lossy(&stdout).into_owned()));
    answer.insert(String::from("stderr"), Value::String(String::from_utf8_lossy(&stderr).into_owned()));
    answer.insert(String::from("files"), Value::Object(collected));
    Ok(HostAnswer::Data(Value::Object(answer)))
}

/// Stops the program `child` and waits for its end. On Unix, every program of its process
/// group is stopped with it: the programs it started, unless one of them left the group.
/// Elsewhere the program alone is stopped, and a program it started keeps running.
fn stop(child: &mut Child) {
    #[cfg(unix)]
    stop_group(child.id());
    let _ = child.kill();
    let _ = child.wait();
}

/// Stops every program that the service `programs` is running in this process, together
/// with the programs each one started, as a program past its time is stopped (see [the module
/// documentation](self#stopping-a-program-and-the-programs-it-started)); each run then answers
/// its extension as a program ended by a signal. From then on, the service starts no program
/// in this process: a later `run` is an error.
///
/// A program that the service runs on Unix is in a process group of its own, which the
/// signals sent to the host's process group (Ctrl+C's interrupt, the quit signal of
/// `Ctrl+\`, the hang-up of a closed terminal, `SIGTERM`) do not reach. A host that ends its
/// process on such a signal calls this function first, so that no program the service
/// started keeps running after the host. The run's temporary folder may then be left in the
/// system's temporary folder, since the host can end before the run removes it. On other
/// systems the function does nothing: the program is started as any other.
pub fn stop_running_programs() {
    #[cfg(unix)]
    {
        let mut running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
        for group in running.take().unwrap_or_default() {
            stop_group(group);
        }
    }
}

/// The process groups of the programs that `run` has started and whose output has not
/// closed yet: those that [`stop_running_programs`] stops. `None` once that function has
/// run, when the service starts no more programs.
#[cfg(unix)]
static RUNNING: Mutex<Option<Vec<u32>>> = Mutex::new(Some(Vec::new()));

/// The process group of a program that `run` started, listed in [`RUNNING`] from the
/// program's start until it has ended and its output has closed.
#[cfg(unix)]
struct Listed(u32);

#[cfg(unix)]
impl Listed {
    /// Starts `command`, whose program leads a process group of its own, and lists that
    /// group in [`RUNNING`] under the same lock, so that [`stop_running_programs`] sees every
    /// program started before it and no program starts after it. `None`, and nothing
    /// started, once that function has run.
    fn spawn(command: &mut Command) -> Option<std::io::Result<(Child, Listed)>> {
        let mut running = RUNNING.lock().unwrap_or_else(PoisonError::into_inner);
        let groups = running.as_mut()?;
        Some(command.spawn().map(|child| {
            groups.push(child.id());
            let listed = Listed(child.id());
            (child, listed)
        }))
    }
}

#[cfg(unix)]
impl Drop for Listed {
    fn drop(&mut self) {
        if let Some(groups) = RUNNING.lock().unwrap_or_else(PoisonError::into_inner).as_mut() {
            groups.retain(|group| *group != self.0);
        }
    }
}

/// Sends `SIGKILL` to the process group `group`, which a program that `run` started leads.
///
/// The group is signaled while its program runs or, once the program has been waited for,
/// until the program's output has closed (the group is then unlisted from [`RUNNING`] at
/// once). Meanwhile the group, as a rule, has a member (the program itself, or a program it
/// started that holds the output open), and the system does not give the group's ID to
/// another process while the group has a member, so the signal reaches that group alone.
/// The exception is the short time between the later of the two events (the wait for the
/// program, and the closing of its output) and the group's unlisting: the group may then
/// have no member, and the signal could reach another group only if the system gave the
/// freed ID to a new process group within that time. A group with no member left answers `ESRCH`, ignored.
#[cfg(unix)]
fn stop_group(group: u32) {
    use nix::sys::signal::{Signal, killpg};
    use nix::unistd::Pid;
    if let Ok(group) = i32::try_from(group) {
        let _ = killpg(Pid::from_raw(group), Signal::SIGKILL);
    }
}

/// A collected file's value: `{ text }` when it is UTF-8, `{ base64 }` otherwise.
fn file_value(bytes: Vec<u8>) -> Value {
    let mut map = Map::new();
    match String::from_utf8(bytes) {
        Ok(text) => map.insert(String::from("text"), Value::String(text)),
        Err(error) => map.insert(String::from("base64"), Value::String(BASE64.encode(error.as_bytes()))),
    };
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order_compares_runs_of_digits_as_numbers() {
        assert_eq!(natural("gs10.02.1", "gs9.56.1"), Ordering::Greater);
        assert_eq!(natural("v1.10", "v1.9"), Ordering::Greater);
        assert_eq!(natural("a", "b"), Ordering::Less);
        assert_eq!(natural("x007", "x7"), Ordering::Equal);
        assert_eq!(natural("x7", "x7a"), Ordering::Less);
    }

    #[test]
    fn a_star_matches_any_run_inside_a_name() {
        assert!(matches("MyUtil*", "MyUtil 1.3.0-alpha4"));
        assert!(matches("gs*", "gs"));
        assert!(matches("a*b*c", "aXXbYYc"));
        assert!(!matches("a*b*c", "aXXc"));
        assert!(!matches("ab*ba", "aba"));
        assert!(!matches("gs*", "xgs1"));
    }

    /// `stop_running_programs` stops a program that the service is running, with the program
    /// it started, and the run answers a program ended by a signal; a later run is refused. No
    /// other test of the crate's library runs a program, so the stop reaches this test's program
    /// alone, and the service, which starts no program after the stop, is not used again.
    #[cfg(unix)]
    #[test]
    fn stop_running_programs_stops_a_running_program_with_the_program_it_started() {
        use flm_ext::engine::ExtensionIdentity;
        use nix::sys::signal::kill;
        use nix::unistd::Pid;
        let run_program = |args: Value| {
            let caller = HostCaller {
                extension: ExtensionIdentity { identity: String::from("/x/runner"), stem: String::from("runner") },
                package: String::from("_"),
                digest_input: std::sync::Arc::from(Vec::new()),
            };
            ProgramsService::new().call(&caller, RUN, &args)
        };
        let folder = fresh_folder().unwrap();
        let pid_file = folder.join("sleep.pid");
        let script = format!("sleep 30 & echo $! > '{}.part' && mv '{0}.part' '{0}'; wait", pid_file.display());
        let running =
            std::thread::spawn(move || run_program(serde_json::json!({ "program": "sh", "args": ["-c", script] })));
        let until = Instant::now() + Duration::from_secs(20);
        let pid = loop {
            if let Ok(pid) = std::fs::read_to_string(&pid_file) {
                break pid.trim().parse::<i32>().unwrap();
            }
            assert!(Instant::now() < until, "the program never started `sleep`");
            std::thread::sleep(Duration::from_millis(10));
        };
        stop_running_programs();
        let answer = running.join().unwrap();
        let _ = std::fs::remove_dir_all(&folder);
        match answer {
            Ok(HostAnswer::Data(answer)) => assert_eq!(answer["status"], Value::Null, "{answer}"),
            other => panic!("{other:?}"),
        }
        // `kill -0` fails once the `sleep` has ended and the system has collected it.
        let until = Instant::now() + Duration::from_secs(5);
        while kill(Pid::from_raw(pid), None).is_ok() {
            assert!(Instant::now() < until, "the program's `sleep` ({pid}) still runs");
            std::thread::sleep(Duration::from_millis(10));
        }
        let detail = run_program(serde_json::json!({ "program": "sh", "args": ["-c", "true"] })).unwrap_err().detail;
        assert_eq!(
            detail,
            "the program `sh` that the extension `runner` runs could not be started: the host is ending and has \
             stopped the programs it runs"
        );
    }

    #[test]
    fn a_path_leaving_the_folder_or_absolute_is_refused() {
        let folder = Path::new("t");
        assert_eq!(inside(folder, "a/../b.txt").unwrap(), PathBuf::from("t/b.txt"));
        assert_eq!(inside(folder, "../b").unwrap_err().detail, "the path `../b` must stay inside the program's temporary folder");
        assert_eq!(inside(folder, "/etc/x").unwrap_err().detail, "the path `/etc/x` must be relative to the program's temporary folder");
    }
}
