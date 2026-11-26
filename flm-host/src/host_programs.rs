//! The host service `programs`: finding a program installed on the machine and running it
//! in a fresh temporary folder, for an extension's handlers.
//!
//! An extension declares the host services its handlers call (the key `host` of a package,
//! [`flm_ext::host`]); a handler calls this one as `cx.host("programs").call(method,
//! args)`. The `flm` program offers it to every document; a program embedding this library
//! offers it only when it asks, by setting
//! [`Request::offer_programs`](crate::session::Request::offer_programs), which makes
//! [`packages::loader`](crate::packages::loader) register a [`ProgramsService`] beside the
//! `files` service, over the handle of the registry's caches ([`ServiceCaches`]). Without it,
//! the loader refuses a package that declares `programs`.
//!
//! When a package that declares `programs` loads, the service checks each cache the package
//! declares (the key `caches`, [`flm_ext::host::CacheDeclaration`]) alone, and refuses the
//! package when cachette refuses one ([`ExtensionCache::check_package`]).
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
//! holds `/` is a path: it answers itself when it is a program. In a folder of `folders`, `*`
//! stands for any run of characters inside one folder name, never a `/` (`/opt/gs*/bin`
//! matches `/opt/gs10.02.1/bin`; several `*` in one name are allowed; no other pattern is).
//! Where several folders match, they are searched in descending natural order: runs of digits
//! are compared as numbers, so `gs10.02.1` comes before `gs9.56.1`, the newest version first.
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
//! | `cache` | the cache folders of the run: a map from a relative path to `{ "name": text, "cost_ms": number }` (`cost_ms` optional) | none |
//!
//! The answer is `{ "status", "timed_out", "stdout", "stderr", "files" }`: `status` the
//! program's exit status (`null` when it was ended by a signal), `timed_out` `false`,
//! `stdout` and `stderr` the text it wrote (bytes that are not UTF-8 replaced by `U+FFFD`),
//! and `files` a map from each path of `collect` at which a file (not a folder) exists after
//! the run to `{ "text" }` when the file is UTF-8 text and `{ "base64" }` otherwise. A
//! program that exits with a status other than 0 is an answer like any other; the script
//! reads `status`.
//!
//! A program that runs past its time is stopped, and that too is an answer:
//! `{ "status": null, "timed_out": true, "files" }`. Its output is not waited for (a program
//! it started may hold it open), so `stdout` and `stderr` are absent; `files` is read as
//! after any run, so a log the program wrote as it went is there.
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
//! folder, written as the host writes paths, `/` between names ([`crate::files`]), and
//! resolved as text before it is joined to the folder, each `..` taking off the name before
//! it even when that name is a link ([`BuildFiles::resolve_within`]); one that is absolute,
//! whose `..` climbs above the folder, or that names the folder itself is refused. A
//! program that cannot be found or started is an error the script receives, naming the
//! extension and the program.
//!
//! # Cache folders
//!
//! The working folder is fresh at every run; only a cache folder that the caller's package
//! declares persists from one run to the next. Each entry of `cache` names, under `name`, a
//! folder cache that the package declares (the key `caches`, with `folder`); a name that it
//! does not declare, or that declares a values cache, is refused. The service opens a
//! session of that cache through the handle of the registry's caches ([`ServiceCaches`]),
//! with the cache's name and instance of [`ExtensionCache::folder`], and makes, at the
//! entry's relative path in the fresh folder, a link to the absolute path of the cache's
//! folder (also under a manager whose root is a relative path): a symbolic link on Unix, a
//! junction on Windows. The program always finds a folder at that path. When the session
//! has no folder (the manager offers no folder cache, or another build holds the folder),
//! when the link cannot be made (the system refuses it, for a cache folder on another
//! volume, say, or the current folder that a relative root starts from cannot be read), or
//! when the link, followed, leads to no folder, the service closes the session and makes an
//! empty folder at that path (after removing the link in the last case), and the program
//! runs as on a first run. The service does not create a missing folder inside the cache:
//! that folder belongs to the manager. The call is refused when that empty folder cannot be
//! made, or when a link that leads to no folder cannot be removed. A path of `files` or
//! `collect` may lie under such a path:
//! a file written there is written into the cache's folder. Two paths of `cache` may not be
//! one path, nor lie one inside the other.
//!
//! After the run and the reading of `collect`, whatever happened, the service removes each
//! link, then the fresh folder, then closes the sessions. Removing the fresh folder never
//! removes a file of a cache folder: a link that cannot be removed leaves the fresh folder
//! in place. `cost_ms`, a whole number of milliseconds from 1, is the estimate of the time
//! needed to produce the folder's content again after this run, recorded when the session
//! closes; without it, the cost recorded before, or the one that the cache declares,
//! stands. The service gives the extension no path of a cache folder: the program reaches
//! the folder through the relative path alone.
//!
//! # Progress and cancellation
//!
//! `run` declares a row of kind [`RUN_KIND`] under the progress scope of the handler call it
//! is made from, labeled with the program as the extension named it, with no count. The row
//! runs from the program's start to its end and ends as finished when the program ended by
//! itself, whatever its status, and as failed otherwise. While it waits for the program, the
//! service asks whether the build is cancelled; when it is, the program is stopped as a
//! program past its time is, with the programs it started, and the call fails.
//!
//! # Paths
//!
//! Every path an extension gives the service, and every path the service answers, is written
//! as the host writes paths ([`crate::files`]): `/` between names on every system, `\` a
//! character of a name, and on Windows an absolute path beginning with its drive
//! (`/C:/Program Files/gs/bin/gswin64c.exe`). These are the names and the folders of `find`
//! and the path it answers, the program of `run`, and the paths of `files` and `collect`; the
//! service converts each to the system's form where it calls the system ([`to_system`]). The
//! folders of `PATH` are the system's: each is converted to the host's form where the service
//! reads it ([`from_system`]), and a folder that has none (one that is not valid Unicode
//! text) is left out. The arguments `args` are given to the program as they are written.
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
//! A run's answer is not cached: every call starts the program again, and only the files of
//! a cache folder persist between runs. A run is not watched: the program and the files it
//! reads outside its folder are not among the files a build read, so the live preview does
//! not build again when they change. The program is not confined: it runs with the rights
//! of the user running the host, reads and writes wherever that user may, and the temporary
//! folder only gives it a place to work.
//!
//! # Trust
//!
//! An extension runs code, and through this service it can run any program on the machine
//! with the user's rights. Extensions are therefore installed from trusted sources only. A
//! document's folder `_flmextensions` comes with the document: building a document received
//! from someone else runs the extensions in its folder, which, when the program offers this
//! service, may run programs.

use std::cmp::Ordering;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};
use std::sync::mpsc;
#[cfg(unix)]
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use flm_core::progress::{Kind, Scope};
use flm_ext::host::{HostAnswer, HostCaller, HostService, HostServiceError};
use serde_json::{Map, Value};

use crate::cache::cachette::{Cost, FolderCacheDeclaration, FolderSession};
use crate::cache::{ExtensionCache, Instance, ServiceCaches};
use crate::files::system::{ToSystemError, from_system, to_system};
use crate::files::{BuildFiles, OutsideFolder, resolved_as_text};
use crate::host_cache::{ContentShape, content_bytes, content_value};

/// The service's name, as a package declares it.
pub const NAME: &str = "programs";
/// The method answering the path of a program.
pub const FIND: &str = "find";
/// The method running a program.
pub const RUN: &str = "run";
/// The time a program may take when `run` gives no `timeout_ms`: 60 seconds.
pub const DEFAULT_TIMEOUT_MS: u64 = 60_000;
/// The kind of the row of the progress sheet that `run` declares for the time the program
/// runs, `flm.programs.run`, titled "Running program" and labeled with the program as the
/// extension named it.
pub const RUN_KIND: Kind = Kind::titled("flm.programs.run", "Running program");

/// The host service `programs`: finds a program on the machine and runs it in a fresh
/// temporary folder, with the cache folders it asks for. See the
/// [module documentation](self).
#[derive(Debug, Clone)]
pub struct ProgramsService {
    /// The handle through which the service opens the sessions of the cache folders of a
    /// run.
    pub caches: ServiceCaches,
}

impl ProgramsService {
    /// Returns the service opening the sessions of its cache folders through `caches`.
    pub fn new(caches: ServiceCaches) -> ProgramsService {
        ProgramsService { caches }
    }
}

impl Default for ProgramsService {
    /// The service over the default [`ServiceCaches`]: a manager that keeps nothing, so that
    /// every cache folder of a run is an empty folder.
    fn default() -> ProgramsService {
        ProgramsService::new(ServiceCaches::default())
    }
}

impl HostService for ProgramsService {
    /// `programs`.
    fn name(&self) -> &str {
        NAME
    }

    /// `find` or `run`; the caller's extension is named in the messages of `run`.
    fn call(&self, caller: &HostCaller, method: &str, args: &Value, progress: &Scope) -> Result<HostAnswer, HostServiceError> {
        match method {
            FIND => find(args),
            RUN => run(&self.caches, caller, args, progress),
            other => Err(refused(format!("the method of the host service `programs` must be `find` or `run`, not `{other}`"))),
        }
    }

    /// Checks each cache the package declares alone ([`ExtensionCache::check_package`]).
    fn check_package(&self, caller: &HostCaller) -> Result<(), HostServiceError> {
        ExtensionCache::check_package(caller)
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
    let found = find_program(&names, &folders).map(|(path, _)| path);
    Ok(HostAnswer::Data(found.map_or(Value::Null, Value::String)))
}

/// The first of `names` found in the folders of `PATH`, then of `folders` (with their
/// wildcards), as the [module documentation](self) says: its path, as the host writes paths,
/// and the system's form of that path.
fn find_program(names: &[String], folders: &[String]) -> Option<(String, PathBuf)> {
    // A folder of `PATH` that has no form as the host writes paths (one that is not valid
    // Unicode text) cannot hold a program the service answers: it is left out.
    let path_folders: Vec<String> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).filter_map(|folder| from_system(&folder).ok()).collect())
        .unwrap_or_default();
    let extra: Vec<String> = folders.iter().flat_map(|folder| expand(folder)).collect();
    names.iter().find_map(|name| {
        if is_path(name) {
            return to_system(name).ok().filter(|system| is_program(system)).map(|system| (name.clone(), system));
        }
        let files = program_files(name);
        path_folders.iter().chain(&extra).find_map(|folder| {
            files.iter().find_map(|file| {
                let path = BuildFiles::resolve(folder, file);
                to_system(&path).ok().filter(|system| is_program(system)).map(|system| (path, system))
            })
        })
    })
}

/// Whether the program `name` is a path: it holds `/`.
fn is_path(name: &str) -> bool {
    name.contains('/')
}

/// The names of the files that would be the program `name` in a folder: the name itself, and
/// on Windows the name with each extension of `PATHEXT` appended (the name alone only when it
/// has an extension).
fn program_files(name: &str) -> Vec<String> {
    if !cfg!(windows) {
        return vec![name.to_string()];
    }
    let extensions = std::env::var("PATHEXT").unwrap_or_else(|_| String::from(".COM;.EXE;.BAT;.CMD"));
    let has_extension = name.rfind('.').is_some_and(|dot| dot > 0);
    let mut files = if has_extension { vec![name.to_string()] } else { Vec::new() };
    files.extend(extensions.split(';').filter(|extension| !extension.is_empty()).map(|extension| format!("{name}{extension}")));
    files
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

/// The folders `folder` (a path as the host writes paths) names: itself when it holds no
/// `*`, else every existing folder its wildcards match, the matches of each folder name in
/// descending natural order.
fn expand(folder: &str) -> Vec<String> {
    let Some(star) = folder.find('*') else { return vec![folder.to_string()] };
    // The text up to the name that holds the first `*`, that name, and the text after it.
    let start = folder[..star].rfind('/').map_or(0, |slash| slash + 1);
    let end = folder[star..].find('/').map_or(folder.len(), |slash| star + slash);
    let (base, pattern, rest) = (&folder[..start], &folder[start..end], &folder[end..]);
    let Ok(listed) = to_system(base) else { return Vec::new() };
    let listed = if listed.as_os_str().is_empty() { PathBuf::from(".") } else { listed };
    let Ok(entries) = std::fs::read_dir(listed) else { return Vec::new() };
    let mut matched: Vec<String> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| matches(pattern, name) && to_system(&format!("{base}{name}")).is_ok_and(|path| path.is_dir()))
        .collect();
    matched.sort_by(|a, b| natural(b, a));
    matched.into_iter().flat_map(|name| expand(&format!("{base}{name}{rest}"))).collect()
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
    caches: Vec<CacheArg>,
}

/// An entry of the argument `cache` of `run`.
struct CacheArg {
    /// The relative path of the link in the fresh folder.
    path: String,
    /// The name of the folder cache, as the package declares it.
    name: String,
    /// The cost of the folder's content after the run, when given.
    cost: Option<Cost>,
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
    let caches = match args.get("cache") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Object(map)) => map.iter().map(|(path, entry)| cache_arg(path, entry)).collect::<Result<_, _>>()?,
        Some(_) => {
            return Err(refused(String::from(
                "‘cache’ must be a map from a relative path to ‘{ name, cost_ms }’",
            )));
        }
    };
    Ok(RunArgs { program, args: arguments, files, stdin, collect, timeout: Duration::from_millis(timeout_ms), caches })
}

/// The entry `entry` of the argument `cache` at the relative path `path`.
fn cache_arg(path: &str, entry: &Value) -> Result<CacheArg, HostServiceError> {
    let Some(name) = entry.get("name").and_then(Value::as_str) else {
        return Err(refused(format!("the cache folder ‘{path}’ must be ‘{{ name, cost_ms }}’, with ‘name’ a text")));
    };
    let cost = match entry.get("cost_ms") {
        None | Some(Value::Null) => None,
        Some(value) => Some(value.as_u64().and_then(Cost::new).ok_or_else(|| {
            refused(format!("‘cost_ms’ of the cache folder ‘{path}’ must be a whole number from 1 to {}", i64::MAX))
        })?),
    };
    Ok(CacheArg { path: path.to_string(), name: name.to_string(), cost })
}

/// Refuses two paths of `caches` that are one path or lie one inside the other.
fn separate(caches: &[CacheArg]) -> Result<(), HostServiceError> {
    for (index, outer) in caches.iter().enumerate() {
        let outer_resolved = resolved_as_text(&outer.path);
        let outer_names: Vec<&str> = outer_resolved.split('/').collect();
        for inner in &caches[index + 1..] {
            let inner_resolved = resolved_as_text(&inner.path);
            let inner_names: Vec<&str> = inner_resolved.split('/').collect();
            if inner_names.starts_with(&outer_names) || outer_names.starts_with(&inner_names) {
                return Err(refused(format!(
                    "the cache folders ‘{}’ and ‘{}’ may not be one folder, nor lie one inside the other",
                    outer.path, inner.path
                )));
            }
        }
    }
    Ok(())
}

/// A cache folder of a run: its session, the link to its folder in the fresh folder, and
/// the cost to record.
struct RunCache {
    /// The session; `None` when the handle could not open it (a configuration that
    /// cachette refuses) or the run has no usable link to its folder: the session has no
    /// folder, the link cannot be made, or the link leads to no folder.
    session: Option<FolderSession>,
    /// The system's form of the link; `None` when an empty folder stands in for the cache's.
    link: Option<PathBuf>,
    /// The cost of the folder's content after the run, when given.
    cost: Option<Cost>,
}

/// Opens the session of each cache folder of `wanted` through `caches`, and makes its link,
/// or an empty folder, at its path in `folder`, adding each to `made` as soon as its session
/// is open, so that the caller removes what was made also after a failure.
fn make_caches(
    caches: &ServiceCaches,
    folder: &str,
    wanted: Vec<(&CacheArg, FolderCacheDeclaration, Instance)>,
    program: &str,
    made: &mut Vec<RunCache>,
) -> Result<(), HostServiceError> {
    for (entry, declaration, instance) in wanted {
        let session = caches.open_folder(&declaration, &instance).ok();
        let target = session.as_ref().and_then(FolderSession::path).map(Path::to_path_buf);
        made.push(RunCache { session, link: None, cost: entry.cost });
        let cache = made.last_mut().expect("pushed above");
        let path = &entry.path;
        let not_made = |error: String| {
            refused(format!("the cache folder ‘{path}’ of the program ‘{program}’ could not be made: {error}"))
        };
        let system = to_system(&inside(folder, path)?)
            .map_err(|reason| reason.to_string())
            .and_then(|system| {
                system.parent().map_or(Ok(()), std::fs::create_dir_all).map_err(|error| error.to_string())?;
                Ok(system)
            })
            .map_err(not_made)?;
        match target.map_or(Link::None, |target| link_cache(&target, &system)) {
            Link::Folder => {
                cache.link = Some(system);
                continue;
            }
            Link::None => {}
            Link::Stuck(error) => {
                // The link stays in the fresh folder: the caller tries again to remove it,
                // and keeps the fresh folder if it cannot.
                cache.session = None;
                cache.link = Some(system);
                return Err(not_made(error));
            }
        }
        // No usable link (the session has no folder, the link cannot be made, or it leads to
        // no folder) is a failure of the cache, not of the run: the session closes, and the
        // program finds an empty folder at the path, as on a first run.
        cache.session = None;
        std::fs::create_dir(&system).map_err(|error| not_made(error.to_string()))?;
    }
    Ok(())
}

/// What [`link_cache`] leaves at the path of a cache folder.
enum Link {
    /// A link that leads to the cache's folder.
    Folder,
    /// Nothing: the link could not be made, or it led to no folder and is removed.
    None,
    /// A link that leads to no folder and cannot be removed, with the reason.
    Stuck(String),
}

/// Makes at `link` a link to the cache's folder `target`, and keeps it only when it leads
/// to a folder.
fn link_cache(target: &Path, link: &Path) -> Link {
    // The link leads to the folder's absolute path: a relative one would be read from the
    // link's own folder, whereas the session's path, under a relative root, is relative to
    // the current folder.
    if std::path::absolute(target).and_then(|target| link_folder(&target, link)).is_err() {
        return Link::None;
    }
    // The system makes a link whatever its target is; following it shows whether the
    // program finds a folder there.
    if std::fs::metadata(link).is_ok_and(|metadata| metadata.is_dir()) {
        Link::Folder
    } else if unlink(link) {
        Link::None
    } else {
        Link::Stuck("a link that leads to no folder cannot be removed".to_string())
    }
}

/// Makes at `link` a link to the folder `target`: a symbolic link on Unix.
#[cfg(unix)]
fn link_folder(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

/// Makes at `link` a link to the folder `target`: a junction on Windows.
#[cfg(windows)]
fn link_folder(target: &Path, link: &Path) -> std::io::Result<()> {
    junction::create(target, link)
}

/// A system with neither symbolic links nor junctions makes no link to a folder.
#[cfg(not(any(unix, windows)))]
fn link_folder(_target: &Path, _link: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "this system makes no link to a folder"))
}

/// Removes the link at `link`, without touching the folder it points to; `true` when no
/// link is left there. A path at which the program left no link (it removed or replaced
/// it) holds nothing of the cache's folder.
fn unlink(link: &Path) -> bool {
    match std::fs::symlink_metadata(link) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            // On Windows a junction is removed as a folder, which leaves its target alone.
            #[cfg(windows)]
            let removed = std::fs::remove_dir(link);
            #[cfg(not(windows))]
            let removed = std::fs::remove_file(link);
            removed.is_ok()
        }
        _ => true,
    }
}

/// The bytes of the file `path` given to `program`, written `content`.
fn file_content(program: &str, path: &str, content: &Value) -> Result<Vec<u8>, HostServiceError> {
    content_bytes(content).map_err(|shape| match shape {
        ContentShape::NotAForm => {
            refused(format!("the file `{path}` given to the program `{program}` must be `{{ text }}` or `{{ base64 }}`"))
        }
        ContentShape::InvalidBase64 => refused(format!(
            "`base64` of the file `{path}` given to the program `{program}` must be base64 in the standard alphabet, with padding"
        )),
    })
}

/// `path`, relative to `folder`, resolved as text and joined to it, or the refusal of a path
/// that is absolute, climbs above the folder or names the folder itself
/// ([`BuildFiles::resolve_within`]). The joined path holds no `..`, so the system never
/// follows a `..` out of a link to a cache folder: `a/../b`, with a link at `a`, is `b` in
/// `folder`.
fn inside(folder: &str, path: &str) -> Result<String, HostServiceError> {
    match BuildFiles::resolve_within(folder, path) {
        Err(OutsideFolder::Absolute) => {
            Err(refused(format!("the path `{path}` must be relative to the program's temporary folder")))
        }
        Err(OutsideFolder::Above) => {
            Err(refused(format!("the path `{path}` must stay inside the program's temporary folder")))
        }
        Ok(_) if BuildFiles::depth_within(path) == Some(0) => {
            Err(refused(format!("the path `{path}` must name a file inside the program's temporary folder")))
        }
        Ok(_) => Ok(BuildFiles::resolve(folder, &resolved_as_text(path))),
    }
}

/// The pause between two tests of whether the program has ended, after each of which the
/// wait asks whether the build is cancelled.
const PAUSE: Duration = Duration::from_millis(5);

/// A counter making each temporary folder's name unique within the process.
static FOLDERS: AtomicU64 = AtomicU64::new(0);

/// A fresh, empty folder under the system's temporary folder: its path as the host writes
/// paths, and its system's form.
fn fresh_folder() -> Result<(String, PathBuf), String> {
    let base = from_system(&std::env::temp_dir()).map_err(|error| error.to_string())?;
    loop {
        let count = FOLDERS.fetch_add(1, AtomicOrdering::Relaxed);
        let folder = BuildFiles::resolve(&base, &format!("flm-programs-{}-{count}", std::process::id()));
        let system = to_system(&folder).map_err(|reason| reason.to_string())?;
        match std::fs::create_dir(&system) {
            Ok(()) => return Ok((folder, system)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.to_string()),
        }
    }
}

/// `run { program, args, files, stdin, collect, timeout_ms, cache }`, opening the sessions
/// of the cache folders through `caches`.
fn run(caches: &ServiceCaches, caller: &HostCaller, args: &Value, progress: &Scope) -> Result<HostAnswer, HostServiceError> {
    let args = run_args(args)?;
    let stem = &caller.extension.stem;
    let program = &args.program;
    // The paths and the caches are checked before anything is made, the paths against a
    // stand-in folder.
    let stand_in = "folder";
    let cache_paths = args.caches.iter().map(|entry| &entry.path);
    for path in args.files.iter().map(|(path, _)| path).chain(&args.collect).chain(cache_paths) {
        inside(stand_in, path)?;
    }
    separate(&args.caches)?;
    let wanted = args
        .caches
        .iter()
        .map(|entry| ExtensionCache::folder(caller, &entry.name).map(|(declaration, instance)| (entry, declaration, instance)))
        .collect::<Result<Vec<_>, _>>()?;
    let no_file = |reason: ToSystemError| {
        refused(format!(
            "the program ‘{program}’ that the extension ‘{stem}’ runs names no file on this system: {reason}"
        ))
    };
    let path = if is_path(program) {
        let path = to_system(program).map_err(no_file)?;
        match std::env::current_dir() {
            Ok(current) if path.is_relative() => current.join(path),
            _ => path,
        }
    } else {
        match find_program(std::slice::from_ref(program), &[]) {
            Some((_, system)) => system,
            // A name that has no system's form (on Windows, one holding `\` or `:`) is no
            // file in any folder.
            None => {
                return Err(match to_system(program) {
                    Err(reason) => no_file(reason),
                    Ok(_) => refused(format!(
                        "the program `{program}` that the extension `{stem}` runs must be in one of the folders of `PATH`"
                    )),
                });
            }
        }
    };
    let (folder, system) = fresh_folder().map_err(|error| {
        refused(format!(
            "the temporary folder of the program `{program}` that the extension `{stem}` runs could not be made: {error}"
        ))
    })?;
    let mut made = Vec::new();
    let answer = make_caches(caches, &folder, wanted, program, &mut made)
        .and_then(|()| run_in(&folder, &system, &path, &args, stem, progress));
    // Each link is removed before the fresh folder, so that removing the folder never
    // reaches a file of a cache folder; a link left in place keeps the folder.
    let left = made.iter().filter_map(|cache| cache.link.as_deref()).filter(|link| !unlink(link)).count();
    if left == 0 {
        let _ = std::fs::remove_dir_all(&system);
    }
    for cache in made {
        if let (Some(session), Some(cost)) = (&cache.session, cache.cost) {
            session.set_cost(cost);
        }
    }
    answer
}

/// Runs the program at `path` in `folder`, whose system's form is `system`, with `args`,
/// for the extension `stem`, under the row of kind [`RUN_KIND`] that it declares under
/// `progress`.
fn run_in(
    folder: &str,
    system: &Path,
    path: &Path,
    args: &RunArgs,
    stem: &str,
    progress: &Scope,
) -> Result<HostAnswer, HostServiceError> {
    let program = &args.program;
    let row = progress.child(&RUN_KIND, Some(program));
    for (file, bytes) in &args.files {
        let full = inside(folder, file)?;
        let written = to_system(&full).map_err(|reason| reason.to_string()).and_then(|full| {
            full.parent()
                .map_or(Ok(()), std::fs::create_dir_all)
                .and_then(|()| std::fs::write(&full, bytes))
                .map_err(|error| error.to_string())
        });
        written.map_err(|error| {
            refused(format!("the file `{file}` given to the program `{program}` could not be written: {error}"))
        })?;
    }
    let mut command = Command::new(path);
    command
        .args(&args.args)
        .current_dir(system)
        .stdin(if args.stdin.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // On Unix the program leads a process group of its own, which the programs it starts
    // join, so that `stop` and `stop_running_programs` stop them with it: `latexmk`, for one,
    // starts LuaLaTeX as a program of its own, which stopping `latexmk` alone leaves running.
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
    // The row runs while the program does; dropped on an early return, it ends as failed.
    let row = row.start();
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
    // A cancelled build stops the program as its time running out does, and the call fails.
    let cancelled = || {
        refused(format!(
            "the program `{program}` that the extension `{stem}` runs was stopped: the build was cancelled"
        ))
    };
    // The exit status and the two outputs, or `None` when the program runs past its time: it
    // is then stopped, and its output is not waited for. Whether the build is cancelled is
    // asked at each pause of the wait.
    let finished: Option<(ExitStatus, Vec<u8>, Vec<u8>)> = 'wait: {
        let status: ExitStatus = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) if Instant::now() >= deadline => {
                    stop(&mut child);
                    break 'wait None;
                }
                Ok(None) if progress.cancelled() => {
                    stop(&mut child);
                    return Err(cancelled());
                }
                Ok(None) => std::thread::sleep(PAUSE),
                Err(error) => {
                    stop(&mut child);
                    return Err(refused(format!(
                        "the program `{program}` that the extension `{stem}` runs could not be waited for: {error}"
                    )));
                }
            }
        };
        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        let mut received = 0;
        while received < 2 {
            let left = deadline.saturating_duration_since(Instant::now());
            match outputs.recv_timeout(left.min(PAUSE)) {
                Ok((is_stdout, bytes)) => {
                    received += 1;
                    if is_stdout { stdout = bytes } else { stderr = bytes }
                }
                // The program has ended, and a program it started holds its output open.
                Err(mpsc::RecvTimeoutError::Timeout) if left > PAUSE => {
                    if progress.cancelled() {
                        stop(&mut child);
                        return Err(cancelled());
                    }
                }
                Err(_) => {
                    stop(&mut child);
                    break 'wait None;
                }
            }
        }
        Some((status, stdout, stderr))
    };
    // A program that ended by itself has completed its run; one stopped past its time has not.
    if finished.is_some() {
        row.finish();
    }
    // The program has ended and its output has closed, or it was stopped with its group, so
    // its group has, as a rule, no member left, and the system may give its ID to another
    // process: it is unlisted at once.
    #[cfg(unix)]
    drop(listed);
    let mut collected = Map::new();
    for file in &args.collect {
        // A path that has no system's form names no file: it is absent.
        let Ok(full) = to_system(&inside(folder, file)?) else { continue };
        // Only a file is answered: a path at which there is none, a folder included, is absent.
        if !std::fs::metadata(&full).is_ok_and(|metadata| metadata.is_file()) {
            continue;
        }
        match std::fs::read(&full) {
            Ok(bytes) => {
                collected.insert(file.clone(), content_value(bytes));
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
    match finished {
        Some((status, stdout, stderr)) => {
            answer.insert(String::from("status"), status.code().map_or(Value::Null, Value::from));
            answer.insert(String::from("timed_out"), Value::Bool(false));
            answer.insert(String::from("stdout"), Value::String(String::from_utf8_lossy(&stdout).into_owned()));
            answer.insert(String::from("stderr"), Value::String(String::from_utf8_lossy(&stderr).into_owned()));
        }
        None => {
            answer.insert(String::from("status"), Value::Null);
            answer.insert(String::from("timed_out"), Value::Bool(true));
        }
    }
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
                extension: ExtensionIdentity { identity: String::from("/x/runner"), stem: String::from("runner"), name: String::from("runner") },
                package: String::from("_"),
                caches: Default::default(),
                digest_input: std::sync::Arc::from(Vec::new()),
            };
            ProgramsService::default().call(&caller, RUN, &args, &Scope::none())
        };
        let (_, folder) = fresh_folder().unwrap();
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
        let folder = "t";
        assert_eq!(inside(folder, "a/../b.txt").unwrap(), "t/b.txt");
        assert_eq!(inside(folder, "./b.txt").unwrap(), "t/b.txt");
        assert_eq!(inside(folder, "a/..").unwrap_err().detail, "the path `a/..` must name a file inside the program's temporary folder");
        assert_eq!(inside(folder, "../b").unwrap_err().detail, "the path `../b` must stay inside the program's temporary folder");
        for path in ["/etc/x", "~/x"] {
            assert_eq!(inside(folder, path).unwrap_err().detail, format!("the path `{path}` must be relative to the program's temporary folder"));
        }
    }
}
