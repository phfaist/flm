//! Reading files for the host, and recording the list of the files one build read.
//!
//! Every module of the host reads its files through this module (the citations provider's
//! cache file excepted): the primary file (the document's own file), the manifests and
//! files of extensions, the files an `\input` includes, image files and
//! bibliography files. The bytes come from a [`Files`],
//! the trait that reads a file at a path: [`DiskFiles`] reads the disk (cargo feature
//! `fs`), and [`MemoryFiles`] reads from a map held in memory, for tests and for a host
//! with no file system.
//!
//! # Paths
//!
//! A path is text in which `/` separates the names, on every system; every other character,
//! `\` included, belongs to a name, so that `my\folder\file.flm` names one file. A path is
//! absolute when it begins with `/`, or is `~`, or begins with `~/` (the user's home folder);
//! on Windows, an absolute path begins with its drive, as in `/C:/figures/a.png`, or is a
//! network path, as in `//server/share/a.png`. These paths are the names of the files: the
//! names that messages print, that the list of files read records and that the packages of
//! an extension carry, the same on every system. The module [`system`] converts a path to
//! the system's form where a [`Files`] implementation meets the system, and converts back a
//! path the system gives (a path on the command line, in the environment, a real path).
//!
//! A relative path written in a file is joined to the folder of that file
//! ([`BuildFiles::resolve`], [`BuildFiles::folder_of`]), as text: `figures/a.png` written in
//! `doc/paper.flm` is `doc/figures/a.png`. The join removes the `.` names (but a first `./`
//! whose removal would make the path absolute, as in `./~/a.flm`, the folder `~`) and
//! changes nothing else: a `..` is kept as written, and the system resolves it when the file
//! is read (on Linux and macOS after following symbolic links, on Windows as text; files
//! held in memory as text, [`MemoryFiles`]), so that `doc/../a.flm` and `a.flm` are two
//! names of one file. The temporary folder of a program
//! that an extension runs confines the paths joined to it ([`BuildFiles::resolve_within`]).
//!
//! # Builds
//!
//! A *build* is one processing of a document ([`crate::session::build`]; *prepare* reads
//! files the same way). The reads of one build go through one [`BuildFiles`], which joins a
//! relative path to the folder of the file in which the path is written, and records every
//! path it looked for, including the paths where nothing was found. The live preview
//! ([`crate::watch`]) watches exactly that list, so it also notices a file that appears
//! where one was missing. [`BuildFiles`] also keeps a digest of each result, so that it can
//! tell later which of its paths would give a different result if read again
//! ([`BuildFiles::changed`]).
//!
//! A registry service, such as the finder of image files, is made once and serves many
//! builds; it reads through a [`ServiceFiles`], to which each build attaches its own
//! [`BuildFiles`] while it runs.

pub mod system;

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Arc, Mutex};

use system::ToSystemError;

/// The reason a file or a folder could not be read, as one of four cases: the
/// [`kind`](FileError::kind) of a [`FileError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileErrorKind {
    /// Nothing exists at the path, or the path names no file on this system
    /// ([`FileError::invalid`]).
    NotFound,
    /// Something exists at the path and cannot be read.
    Unreadable,
    /// The file was read as text and is not valid UTF-8.
    NotText,
    /// A folder was to be listed, and the path holds a file.
    NotAFolder,
}

/// The error returned when a file cannot be read or a folder cannot be listed: the path,
/// the reason as a [`FileErrorKind`], and the explanation given by the [`Files`]
/// implementation.
///
/// Its `Display` text is the message a person reads, for example
/// "the file `doc/a.flm` must exist"; it shows [`detail`](FileError::detail) only for
/// [`FileErrorKind::Unreadable`]. When [`invalid`](FileError::invalid) is given, it names the
/// path and says why it names no file on this system: "the path ‘/figures/a.png’ names no
/// file on this system: an absolute path on Windows must begin with its drive, as in
/// ‘/C:/figures/a.png’".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileError {
    /// The path, as resolved.
    pub path: String,
    /// Why the file could not be read.
    pub kind: FileErrorKind,
    /// The explanation given by the [`Files`] implementation (for [`DiskFiles`], the
    /// operating system's error text).
    pub detail: String,
    /// Why the path names no file on this system, when that is why nothing was found: the
    /// path has no system's form ([`system::to_system`]), and the kind is
    /// [`FileErrorKind::NotFound`]. `None` for every other error.
    pub invalid: Option<ToSystemError>,
}

impl FileError {
    /// The error of kind `kind` at `path`, with the explanation `detail`.
    fn new(path: &str, kind: FileErrorKind, detail: impl Into<String>) -> FileError {
        FileError { path: path.to_owned(), kind, detail: detail.into(), invalid: None }
    }
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            FileErrorKind::NotFound => match &self.invalid {
                Some(reason) => write!(f, "the path ‘{}’ names no file on this system: {reason}", self.path),
                None => write!(f, "the file `{}` must exist", self.path),
            },
            FileErrorKind::Unreadable => write!(f, "the file `{}` cannot be read: {}", self.path, self.detail),
            FileErrorKind::NotText => write!(f, "the file `{}` must be text encoded as UTF-8", self.path),
            FileErrorKind::NotAFolder => write!(f, "`{}` must be a folder, not a file", self.path),
        }
    }
}

impl std::error::Error for FileError {}

/// Reads the bytes of a file, or lists the entries of a folder, at a path that is already
/// resolved (joined to its base folder): a path of the [module documentation](self#paths),
/// with `/` between its names.
///
/// [`DiskFiles`] and [`MemoryFiles`] implement it; a program that keeps its files elsewhere
/// (in a browser's storage, in an archive) implements it too and passes it to
/// [`prepare`](crate::session::prepare). The modules of the host never call it directly:
/// they read through a [`BuildFiles`], which resolves paths and records what was read.
/// An implementation is shared across threads, so it must be `Send` and `Sync`.
pub trait Files: Send + Sync {
    /// The bytes of the file at `path`.
    ///
    /// # Errors
    ///
    /// A [`FileError`] of kind [`FileErrorKind::NotFound`] when nothing exists at `path`,
    /// and of kind [`FileErrorKind::Unreadable`] when something does and cannot be read.
    fn read(&self, path: &str) -> Result<Vec<u8>, FileError>;

    /// The names of the entries — files and folders — of the folder at `path`, in name
    /// order. The empty path is the current folder.
    ///
    /// # Errors
    ///
    /// A [`FileError`] of kind [`FileErrorKind::NotFound`] when nothing exists at `path`,
    /// of kind [`FileErrorKind::NotAFolder`] when a file does, and of kind
    /// [`FileErrorKind::Unreadable`] when a folder does and cannot be listed.
    fn entries(&self, path: &str) -> Result<Vec<String>, FileError>;

    /// Whether `path`, which exists, lies inside `folder` once every symbolic link of both
    /// is followed. The default answers `true`: files held in memory have no links.
    /// [`DiskFiles`] follows the links on the disk; an extension's files are read only
    /// where this answers `true` ([`crate::extensions`]).
    ///
    /// # Errors
    ///
    /// A [`FileError`] when a path cannot be followed to its target.
    fn stays_within(&self, folder: &str, path: &str) -> Result<bool, FileError> {
        let _ = (folder, path);
        Ok(true)
    }
}

/// The [`Files`] implementation that reads the disk (cargo feature `fs`): each path is
/// converted to the system's form ([`system::to_system`], which replaces `~` by the home
/// folder), and a relative path is read relative to the process's current folder, as the
/// operating system reads it. A path that has no system's form names no file: reading it
/// answers [`FileErrorKind::NotFound`], with the reason ([`FileError::invalid`]). A name
/// of a folder's entry that is not valid Unicode text is not listed.
#[cfg(feature = "fs")]
#[derive(Debug, Clone, Copy, Default)]
pub struct DiskFiles;

#[cfg(feature = "fs")]
impl DiskFiles {
    /// The system's path for `path`; or the error of a path that names no file on this
    /// system, of kind [`FileErrorKind::NotFound`] with the reason
    /// ([`FileError::invalid`]).
    pub(crate) fn system(path: &str) -> Result<std::path::PathBuf, FileError> {
        system::to_system(path).map_err(|reason| {
            let error = FileError::new(path, FileErrorKind::NotFound, reason.to_string());
            FileError { invalid: Some(reason), ..error }
        })
    }

    /// The system's path for `path`, as [`DiskFiles::system`], with `.` for the empty path
    /// (the current folder), which the system's calls do not take.
    fn system_or_current(path: &str) -> Result<std::path::PathBuf, FileError> {
        let system = DiskFiles::system(path)?;
        Ok(if system.as_os_str().is_empty() { std::path::PathBuf::from(".") } else { system })
    }

    /// The error at `path` of the system's error `error`, met reading a file or, when
    /// `listing`, listing a folder.
    fn error(path: &str, error: &std::io::Error, listing: bool) -> FileError {
        let kind = match error.kind() {
            std::io::ErrorKind::NotFound => FileErrorKind::NotFound,
            std::io::ErrorKind::NotADirectory if listing => FileErrorKind::NotAFolder,
            _ => FileErrorKind::Unreadable,
        };
        FileError::new(path, kind, error.to_string())
    }
}

#[cfg(feature = "fs")]
impl Files for DiskFiles {
    fn read(&self, path: &str) -> Result<Vec<u8>, FileError> {
        std::fs::read(DiskFiles::system(path)?).map_err(|error| DiskFiles::error(path, &error, false))
    }

    fn entries(&self, path: &str) -> Result<Vec<String>, FileError> {
        let error = |error: std::io::Error| DiskFiles::error(path, &error, true);
        let mut names = Vec::new();
        for entry in std::fs::read_dir(DiskFiles::system_or_current(path)?).map_err(error)? {
            if let Ok(name) = entry.map_err(error)?.file_name().into_string() {
                names.push(name);
            }
        }
        names.sort();
        Ok(names)
    }

    /// Both paths made canonical (every link followed, `.` and `..` resolved), then
    /// compared.
    fn stays_within(&self, folder: &str, path: &str) -> Result<bool, FileError> {
        let canonical = |path: &str| {
            std::fs::canonicalize(DiskFiles::system_or_current(path)?)
                .map_err(|error| DiskFiles::error(path, &error, false))
        };
        Ok(canonical(path)?.starts_with(canonical(folder)?))
    }
}

/// The [`Files`] implementation that reads files held in memory, each stored under its
/// path.
///
/// A path is looked up after [`BuildFiles`] has resolved it (`notes/a.flm` written in a
/// file of the folder `doc/` is looked up as `doc/notes/a.flm`), the way a system finds a
/// file where no symbolic link is involved: its `.` names and its empty names are left
/// out, and each `..` takes off the name before it, as text (`doc/parts/../a.flm` is the
/// file stored at `doc/a.flm`). A `..` with no name before it stays a name: `../a.flm`
/// finds only a file stored under that text. A file is stored under its path taken the
/// same way. Nothing else is converted: `~/a.flm` names the file stored under that text,
/// not a file of the home folder. The list of files read and the messages name a file by
/// the path as it is given (`doc/parts/../a.flm`). A folder exists when a file is stored
/// under it, directly or deeper.
///
/// # Examples
///
/// ```
/// use flm_host::files::{Files, MemoryFiles};
///
/// let files = MemoryFiles::new().with("doc/paper.flm", "Hello.");
/// assert_eq!(files.read("doc/paper.flm").unwrap(), b"Hello.");
/// assert!(files.read("doc/other.flm").is_err());
/// // A `..` takes off the name before it.
/// assert_eq!(files.read("doc/notes/../paper.flm").unwrap(), b"Hello.");
/// ```
#[derive(Debug, Clone, Default)]
pub struct MemoryFiles {
    files: BTreeMap<String, Vec<u8>>,
}

impl MemoryFiles {
    /// Returns an empty set of files.
    pub fn new() -> MemoryFiles {
        MemoryFiles::default()
    }

    /// Stores `bytes` as the file at `path`, replacing any file stored there. Returns
    /// `self`, for chaining.
    pub fn insert(&mut self, path: impl Into<String>, bytes: impl Into<Vec<u8>>) -> &mut MemoryFiles {
        let path = path.into();
        self.files.insert(resolved_as_text(&path).into_owned(), bytes.into());
        self
    }

    /// Returns these files with `text` stored as the file at `path` (a builder form of
    /// [`insert`](MemoryFiles::insert)).
    pub fn with(mut self, path: impl Into<String>, text: &str) -> MemoryFiles {
        self.insert(path, text.as_bytes());
        self
    }
}

impl Files for MemoryFiles {
    fn read(&self, path: &str) -> Result<Vec<u8>, FileError> {
        self.files
            .get(resolved_as_text(path).as_ref())
            .cloned()
            .ok_or_else(|| FileError::new(path, FileErrorKind::NotFound, "no such file in memory"))
    }

    /// A folder exists in memory when a file is stored under it, directly or deeper.
    fn entries(&self, path: &str) -> Result<Vec<String>, FileError> {
        let key = resolved_as_text(path);
        if self.files.contains_key(key.as_ref()) {
            return Err(FileError::new(path, FileErrorKind::NotAFolder, "a file in memory"));
        }
        let prefix = if key.is_empty() || key.ends_with('/') { key } else { Cow::Owned(format!("{key}/")) };
        let mut names: Vec<String> = self
            .files
            .keys()
            .filter_map(|file| file.strip_prefix(prefix.as_ref()))
            .filter_map(|rest| rest.split('/').next())
            .filter(|name| !name.is_empty())
            .map(str::to_string)
            .collect();
        names.sort();
        names.dedup();
        if names.is_empty() {
            return Err(FileError::new(path, FileErrorKind::NotFound, "no such folder in memory"));
        }
        Ok(names)
    }
}

/// One path that a build looked for, by reading a file or listing a folder, and whether
/// something was found there; [`BuildFiles::list`] returns these.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileRead {
    /// The path, as resolved.
    pub path: String,
    /// Whether a file (or, for a listing, a folder) was found at the path. A path where
    /// nothing was found is recorded too, so that a file appearing there later can be
    /// noticed. A file that exists and cannot be read counts as found.
    pub found: bool,
}

/// Why [`BuildFiles::resolve_within`] refused a path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutsideFolder {
    /// The path is absolute.
    Absolute,
    /// A `..` of the path climbs above the folder.
    Above,
}

/// The file reader of one build: reads through a [`Files`], resolves each relative path
/// against a base folder, and records every path it looked for.
///
/// A path is *resolved* by joining it to a base folder ([`resolve`](BuildFiles::resolve)):
/// the folder of the file in which the path is written, or, for a document read from
/// standard input or given as text on the command line, the current folder, written as
/// the empty path. An absolute path is kept as it is. A clone shares the same record, so
/// every module of one build that holds a clone adds to one list
/// ([`list`](BuildFiles::list)).
#[derive(Clone)]
pub struct BuildFiles {
    files: Arc<dyn Files>,
    read: Arc<Mutex<Vec<Looked>>>,
}

/// One path a build looked for, how, and a digest of the result the [`Files`]
/// implementation returned.
#[derive(Debug, Clone)]
struct Looked {
    read: FileRead,
    folder: bool,
    answer: u64,
}

/// A digest of a result of the [`Files`] implementation: the bytes or the entries, or the
/// kind of error.
fn digest<T: Hash>(answer: &Result<T, FileError>) -> u64 {
    let mut hasher = DefaultHasher::new();
    match answer {
        Ok(value) => (0u8, value).hash(&mut hasher),
        Err(error) => (1u8, error.kind).hash(&mut hasher),
    }
    hasher.finish()
}

impl BuildFiles {
    /// Returns a file reader that reads through `files`, with nothing recorded yet.
    pub fn new(files: Arc<dyn Files>) -> BuildFiles {
        BuildFiles { files, read: Arc::new(Mutex::new(Vec::new())) }
    }

    /// Returns a file reader over the same [`Files`] with a record of its own, which nobody
    /// reads: what is read through it is on no list of this reader or its clones. It serves a
    /// reading that the work does not depend on, such as the listing of the extensions
    /// installed ([`ExtensionFinder::locators`](crate::extensions::ExtensionFinder::locators)).
    pub fn unrecorded(&self) -> BuildFiles {
        BuildFiles::new(Arc::clone(&self.files))
    }

    /// Returns `path` resolved against the folder `base`: `path` itself when it is absolute
    /// (it begins with `/`, or is `~`, or begins with `~/`) or when `base` is the empty path
    /// (the current folder); `base` when `path` is empty; else `base`, then `/` (unless
    /// `base` ends with one), then `path`. Every `.` name of the result is then removed
    /// (`a/./b` becomes `a/b`, `./a` becomes `a`, and a path of `.` names alone becomes
    /// `.`), except a first `.` whose removal would make the path absolute: `./~/a` stays
    /// `./~/a`, which names the folder `~` and not the home folder. Nothing else is changed:
    /// a `..` is kept as written, and so is an empty name (`a//b`, or the `//` that begins a
    /// network path). See the [module documentation](self#paths).
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_host::files::BuildFiles;
    ///
    /// assert_eq!(BuildFiles::resolve("doc", "figures/a.png"), "doc/figures/a.png");
    /// assert_eq!(BuildFiles::resolve("doc", "./a/../b.flm"), "doc/a/../b.flm");
    /// assert_eq!(BuildFiles::resolve("doc", "/C:/figures/a.png"), "/C:/figures/a.png");
    /// assert_eq!(BuildFiles::resolve("", "./~/a.flm"), "./~/a.flm");
    /// ```
    pub fn resolve(base: &str, path: &str) -> String {
        let joined = if BuildFiles::is_absolute(path) || base.is_empty() {
            Cow::Borrowed(path)
        } else if path.is_empty() {
            Cow::Borrowed(base)
        } else if base.ends_with('/') {
            Cow::Owned(format!("{base}{path}"))
        } else {
            Cow::Owned(format!("{base}/{path}"))
        };
        without_dots(&joined).into_owned()
    }

    /// Returns the folder of the file named `name`: the text before its last `/`. The
    /// folder of `/x` is `/`, that of `~/x` is `~`, and that of a name with no `/`, such as
    /// `x`, is the empty path, the current folder.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_host::files::BuildFiles;
    ///
    /// assert_eq!(BuildFiles::folder_of("doc/paper.flm"), "doc");
    /// assert_eq!(BuildFiles::folder_of("/paper.flm"), "/");
    /// assert_eq!(BuildFiles::folder_of("paper.flm"), "");
    /// ```
    pub fn folder_of(name: &str) -> &str {
        match name.rfind('/') {
            Some(0) => "/",
            Some(slash) => &name[..slash],
            None => "",
        }
    }

    /// Returns `path`, relative to the folder `folder`, resolved against it
    /// ([`resolve`](BuildFiles::resolve)), when it stays inside the folder as text.
    ///
    /// # Errors
    ///
    /// [`OutsideFolder::Absolute`] when `path` is absolute, and [`OutsideFolder::Above`]
    /// when a `..` of `path` climbs above `folder`, counting the names of `path` alone:
    /// `a/../b` stays inside, `../b` and `a/../../b` do not. A symbolic link is not
    /// followed here: [`stays_within`](BuildFiles::stays_within) checks where a file read
    /// lies.
    pub fn resolve_within(folder: &str, path: &str) -> Result<String, OutsideFolder> {
        if BuildFiles::is_absolute(path) {
            return Err(OutsideFolder::Absolute);
        }
        BuildFiles::depth_within(path).ok_or(OutsideFolder::Above)?;
        Ok(BuildFiles::resolve(folder, path))
    }

    /// Whether `path` is absolute: it begins with `/`, or is `~`, or begins with `~/`.
    pub(crate) fn is_absolute(path: &str) -> bool {
        path.starts_with('/') || path == "~" || path.starts_with("~/")
    }

    /// The number of folders below its folder at which the relative path `path` ends,
    /// counting its names (a `..` climbs one, `.` and an empty name none); `None` when a
    /// `..` climbs above the folder. `0` for a path that names the folder itself.
    pub(crate) fn depth_within(path: &str) -> Option<usize> {
        let mut depth = 0usize;
        for name in path.split('/') {
            match name {
                "" | "." => {}
                ".." => depth = depth.checked_sub(1)?,
                _ => depth += 1,
            }
        }
        Some(depth)
    }

    /// Returns the bytes of the file at `path`, resolved against `base`
    /// ([`BuildFiles::resolve`]), and records the path, whether a file was found or not.
    ///
    /// # Errors
    ///
    /// The [`FileError`] returned by the [`Files`] implementation, when the file cannot be
    /// read.
    pub fn read_bytes(&self, base: &str, path: &str) -> Result<Vec<u8>, FileError> {
        let path = BuildFiles::resolve(base, path);
        let result = self.files.read(&path);
        self.record(path, false, &result);
        result
    }

    /// Returns the text of the file at `path`, resolved against `base`, read and recorded
    /// as [`read_bytes`](BuildFiles::read_bytes) does.
    ///
    /// # Errors
    ///
    /// The [`FileError`] returned by the [`Files`] implementation, and one of kind
    /// [`FileErrorKind::NotText`] when the bytes are not valid UTF-8.
    pub fn read_text(&self, base: &str, path: &str) -> Result<String, FileError> {
        let bytes = self.read_bytes(base, path)?;
        String::from_utf8(bytes)
            .map_err(|error| FileError::new(&BuildFiles::resolve(base, path), FileErrorKind::NotText, error.to_string()))
    }

    /// Returns the names of the entries (files and folders) of the folder at `path`,
    /// resolved against `base`, in name order, and records the folder, whether it was found
    /// or not, so that a file appearing in it later can be noticed.
    ///
    /// # Errors
    ///
    /// The [`FileError`] returned by the [`Files`] implementation, when the folder cannot be
    /// listed.
    pub fn entries(&self, base: &str, path: &str) -> Result<Vec<String>, FileError> {
        let path = BuildFiles::resolve(base, path);
        let result = self.files.entries(&path);
        self.record(path, true, &result);
        result
    }

    /// Returns whether `path`, which exists, lies inside `folder` once every symbolic link
    /// of both is followed ([`Files::stays_within`]), both resolved against `base`. Records
    /// nothing: the paths were read already.
    ///
    /// # Errors
    ///
    /// The [`FileError`] returned by the [`Files`] implementation.
    pub fn stays_within(&self, base: &str, folder: &str, path: &str) -> Result<bool, FileError> {
        self.files.stays_within(&BuildFiles::resolve(base, folder), &BuildFiles::resolve(base, path))
    }

    /// Returns every path looked for so far, in the order of the reads; a path read twice
    /// is listed twice.
    pub fn list(&self) -> Vec<FileRead> {
        self.looked().into_iter().map(|looked| looked.read).collect()
    }

    /// Returns the paths looked for so far that would give a different result if looked
    /// for again now: a file whose bytes changed, a folder whose entries changed, a path
    /// where something appeared or disappeared, or a path that now fails with another
    /// [`FileErrorKind`]. Each path is listed once, in the order of its first read. The
    /// paths are looked for again through the [`Files`] implementation, and these reads are
    /// not recorded.
    pub fn changed(&self) -> Vec<String> {
        let mut changed: Vec<String> = Vec::new();
        for looked in self.looked() {
            let path = &looked.read.path;
            let answer = if looked.folder { digest(&self.files.entries(path)) } else { digest(&self.files.read(path)) };
            if answer != looked.answer && !changed.contains(path) {
                changed.push(path.clone());
            }
        }
        changed
    }

    /// Adds `path` to the list, with a digest of `result`, the result the [`Files`]
    /// implementation returned.
    fn record<T: Hash>(&self, path: String, folder: bool, result: &Result<T, FileError>) {
        let found = !matches!(result, Err(error) if error.kind == FileErrorKind::NotFound);
        let looked = Looked { read: FileRead { path, found }, folder, answer: digest(result) };
        self.read.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(looked);
    }

    fn looked(&self) -> Vec<Looked> {
        self.read.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }
}

/// Returns `path` with its `.` names removed, as [`BuildFiles::resolve`] describes.
fn without_dots(path: &str) -> Cow<'_, str> {
    if !path.split('/').any(|name| name == ".") {
        return Cow::Borrowed(path);
    }
    let kept = path.split('/').filter(|name| *name != ".").collect::<Vec<_>>().join("/");
    Cow::Owned(if kept.is_empty() {
        String::from(if path.starts_with('/') { "/" } else { "." })
    } else if (path == "." || path.starts_with("./")) && BuildFiles::is_absolute(&kept) {
        format!("./{kept}")
    } else {
        kept
    })
}

/// Returns `path` resolved as text, as a system resolves a path where no symbolic link is
/// involved: its `.` names and its empty names left out and each `..` taking off the name
/// before it (`a/./b/../c` gives `a/c`, `a//b` gives `a/b`, `a/..` the empty path). The
/// `/` or `//` that begins an absolute path is kept, and so is a `..` with no name before it
/// to take off (`../a`, `/../a`).
///
/// It gives the key under which files held in memory ([`MemoryFiles`], and the static table
/// of [`StaticFiles`](crate::extensions::StaticFiles)) store and look up a file, and the
/// path, relative to the output folder, at which the output's files are placed
/// ([`place`](crate::output::place)).
pub(crate) fn resolved_as_text(path: &str) -> Cow<'_, str> {
    let names = path.trim_start_matches('/');
    if names.is_empty() || names.split('/').all(|name| !matches!(name, "" | "." | "..")) {
        return Cow::Borrowed(path);
    }
    let mut kept: Vec<&str> = Vec::new();
    for name in names.split('/') {
        match name {
            "" | "." => {}
            ".." if kept.last().is_some_and(|last| *last != "..") => {
                kept.pop();
            }
            name => kept.push(name),
        }
    }
    let root = &path[..path.len() - names.len()];
    Cow::Owned(format!("{root}{}", kept.join("/")))
}

/// The file reader of a registry's services: a service reads through the file reader of the
/// build under way, so that what it reads is recorded in that build's list of files read.
///
/// A [service](flm_core::core::services) contributed to the registry, such as the finder of
/// image files, is made once, when [`prepare`](crate::session::prepare) builds the
/// registry, and serves every build made from it; a build's file reader ([`BuildFiles`]) is
/// made anew by each [`build`](crate::session::build). The service therefore holds a
/// `ServiceFiles`, shared with the [`Prepared`](crate::session::Prepared), and
/// [`build`](crate::session::build) attaches its own [`BuildFiles`] to it for the duration
/// of its run ([`attach`](ServiceFiles::attach)). Every read a service makes meanwhile is recorded
/// in that build's [`Built::files_read`](crate::session::Built::files_read).
///
/// The builds made from one [`Prepared`](crate::session::Prepared) take turns: a second
/// build waits until the first has detached its [`BuildFiles`]. A service that reads while no build is attached (in a program that
/// runs the registry without [`build`](crate::session::build)) reads through the same
/// [`Files`], and the read is recorded nowhere.
#[derive(Clone)]
pub struct ServiceFiles {
    files: Arc<dyn Files>,
    attached: Arc<Mutex<Option<BuildFiles>>>,
    turn: Arc<Mutex<()>>,
}

impl ServiceFiles {
    /// Returns the file reader of a registry's services, which reads through `files`, with no
    /// build attached.
    pub fn new(files: Arc<dyn Files>) -> ServiceFiles {
        ServiceFiles { files, attached: Arc::new(Mutex::new(None)), turn: Arc::new(Mutex::new(())) }
    }

    /// Returns the file reader a service reads through now: the attached build's, or, when
    /// no build is attached, a new [`BuildFiles`] whose record nobody reads.
    pub fn reader(&self) -> BuildFiles {
        let attached = self.attached.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        attached.clone().unwrap_or_else(|| BuildFiles::new(Arc::clone(&self.files)))
    }

    /// Attaches `reads`, the file reader of one build, until the returned guard is dropped.
    /// Blocks while another build's file reader is attached.
    pub fn attach(&self, reads: BuildFiles) -> AttachedBuild<'_> {
        let turn = self.turn.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        *self.attached.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(reads);
        AttachedBuild { files: self, _turn: turn }
    }
}

impl fmt::Debug for ServiceFiles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ServiceFiles").finish_non_exhaustive()
    }
}

/// The guard returned by [`ServiceFiles::attach`]: until it is dropped, the services read
/// through the attached build's file reader; dropping it detaches that file reader and lets
/// the next build attach its own.
#[must_use = "the reading is detached as soon as the guard is dropped"]
pub struct AttachedBuild<'a> {
    files: &'a ServiceFiles,
    _turn: std::sync::MutexGuard<'a, ()>,
}

impl Drop for AttachedBuild<'_> {
    fn drop(&mut self) {
        *self.files.attached.lock().unwrap_or_else(|poisoned| poisoned.into_inner()) = None;
    }
}

impl fmt::Debug for BuildFiles {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildFiles").field("read", &self.list()).finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(all(feature = "fs", unix))]
    #[test]
    fn a_link_leaving_the_folder_does_not_stay_within_it() {
        let root = std::env::temp_dir().join(format!("flm-stays-within-{}", std::process::id()));
        let folder = root.join("ext");
        std::fs::create_dir_all(&folder).unwrap();
        std::fs::write(root.join("outside.txt"), "x").unwrap();
        std::fs::write(folder.join("inside.txt"), "x").unwrap();
        std::os::unix::fs::symlink(root.join("outside.txt"), folder.join("link.txt")).unwrap();
        let linked = root.join("linked");
        std::os::unix::fs::symlink(&folder, &linked).unwrap();
        let files = BuildFiles::new(Arc::new(DiskFiles));
        let flm = |path: &std::path::Path| system::from_system(path).unwrap();
        let (folder, linked) = (flm(&folder), flm(&linked));
        assert!(files.stays_within("", &folder, &format!("{folder}/inside.txt")).unwrap());
        assert!(!files.stays_within("", &folder, &format!("{folder}/link.txt")).unwrap());
        // A folder that is itself a link: its files lie inside its target.
        assert!(files.stays_within("", &linked, &format!("{linked}/inside.txt")).unwrap());
        assert!(files.list().is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_relative_path_is_joined_to_its_folder_as_text() {
        for (base, path, resolved) in [
            ("doc", "notes/a.flm", "doc/notes/a.flm"),
            ("doc/", "a.flm", "doc/a.flm"),
            ("/", "a.flm", "/a.flm"),
            ("~", "a.flm", "~/a.flm"),
            ("", "a.flm", "a.flm"),
            ("doc", "", "doc"),
            ("doc", "/C:/a.flm", "/C:/a.flm"),
            ("doc", "~/a.flm", "~/a.flm"),
            ("doc", "~", "~"),
            ("doc", r"my\folder\a.flm", r"doc/my\folder\a.flm"),
            ("doc", "~x/a.flm", "doc/~x/a.flm"),
        ] {
            assert_eq!(BuildFiles::resolve(base, path), resolved, "{base} {path}");
        }
    }

    #[test]
    fn the_join_removes_the_dot_names_and_keeps_the_rest_as_written() {
        for (base, path, resolved) in [
            ("doc", "./a/./b.flm", "doc/a/b.flm"),
            ("", "./a.flm", "a.flm"),
            ("", ".", "."),
            ("", "./.", "."),
            (".", "a.flm", "a.flm"),
            ("doc", ".", "doc"),
            ("/", ".", "/"),
            ("doc", "a/../b.flm", "doc/a/../b.flm"),
            ("doc", "../b.flm", "doc/../b.flm"),
            ("doc", "a//b.flm", "doc/a//b.flm"),
            ("", "//server/share/a.png", "//server/share/a.png"),
            // A first `.` stays where its removal would make the path absolute.
            ("", "./~/a.flm", "./~/a.flm"),
            ("", "./~", "./~"),
            ("", ".//a.flm", ".//a.flm"),
            ("doc", "./~/a.flm", "doc/~/a.flm"),
            ("", "~/./a.flm", "~/a.flm"),
        ] {
            assert_eq!(BuildFiles::resolve(base, path), resolved, "{base} {path}");
        }
    }

    #[test]
    fn the_folder_of_a_name_is_the_text_before_its_last_slash() {
        for (name, folder) in [("doc/a.flm", "doc"), ("a.flm", ""), ("/a.flm", "/"), ("~/a.flm", "~"), ("//s/x/a", "//s/x")] {
            assert_eq!(BuildFiles::folder_of(name), folder, "{name}");
        }
    }

    #[test]
    fn a_path_confined_to_its_folder_is_relative_and_climbs_no_higher() {
        assert_eq!(BuildFiles::resolve_within("ext", "a/../b.png"), Ok(String::from("ext/a/../b.png")));
        assert_eq!(BuildFiles::resolve_within("ext", "./b.png"), Ok(String::from("ext/b.png")));
        assert_eq!(BuildFiles::resolve_within("ext", "./~/b.png"), Ok(String::from("ext/~/b.png")));
        assert_eq!(BuildFiles::resolve_within("ext", ""), Ok(String::from("ext")));
        for path in ["/b.png", "~/b.png", "~", "//server/share/b.png"] {
            assert_eq!(BuildFiles::resolve_within("ext", path), Err(OutsideFolder::Absolute), "{path}");
        }
        for path in ["..", "../b.png", "a/../../b.png", "../ext/b.png"] {
            assert_eq!(BuildFiles::resolve_within("ext", path), Err(OutsideFolder::Above), "{path}");
        }
    }

    #[test]
    fn a_relative_path_is_resolved_against_the_folder_that_names_it() {
        let files = BuildFiles::new(Arc::new(MemoryFiles::new().with("doc/notes/a.flm", "A")));
        assert_eq!(files.read_text("doc", "notes/a.flm").unwrap(), "A");
    }

    #[test]
    fn every_path_looked_for_is_listed_found_or_not() {
        let files = BuildFiles::new(Arc::new(MemoryFiles::new().with("a.flm", "A")));
        files.read_text("", "a.flm").unwrap();
        let missing = files.read_text("", "b.flm").unwrap_err();
        assert_eq!(missing.kind, FileErrorKind::NotFound);
        assert_eq!(
            files.list(),
            vec![
                FileRead { path: String::from("a.flm"), found: true },
                FileRead { path: String::from("b.flm"), found: false },
            ]
        );
    }

    #[test]
    fn a_folder_lists_its_files_and_folders_once_each() {
        let memory = MemoryFiles::new().with("p/a.yaml", "").with("p/b/x.yaml", "").with("p/b/y.yaml", "").with("p/b.yaml", "");
        let files = BuildFiles::new(Arc::new(memory));
        assert_eq!(files.entries("", "p").unwrap(), ["a.yaml", "b", "b.yaml"]);
        assert_eq!(files.entries("", "").unwrap(), ["p"]);
        assert_eq!(files.entries("p", "c").unwrap_err().kind, FileErrorKind::NotFound);
        assert_eq!(files.list()[2], FileRead { path: String::from("p/c"), found: false });
    }

    #[test]
    fn a_path_in_memory_is_looked_up_with_its_dot_names_resolved_as_text() {
        let memory = MemoryFiles::new().with("doc/a.flm", "A").with("../up.flm", "U");
        assert_eq!(memory.read("doc/./x/../a.flm").unwrap(), b"A");
        assert_eq!(memory.entries("doc/x/..").unwrap(), ["a.flm"]);
        // A `..` with no name before it stays a name.
        assert_eq!(memory.read("doc/../../up.flm").unwrap(), b"U");
        assert_eq!(memory.read("../a.flm").unwrap_err().kind, FileErrorKind::NotFound);
        // The path is named as it is given.
        assert_eq!(memory.read("doc/x/../b.flm").unwrap_err().path, "doc/x/../b.flm");
    }

    #[test]
    fn a_service_reads_through_the_build_attached_and_lists_nothing_otherwise() {
        let service = ServiceFiles::new(Arc::new(MemoryFiles::new().with("a.png", "A")));
        service.reader().read_bytes("", "a.png").unwrap();
        let build = BuildFiles::new(Arc::new(MemoryFiles::new().with("a.png", "A")));
        {
            let _attached = service.attach(build.clone());
            service.reader().read_bytes("", "a.png").unwrap();
            service.reader().read_bytes("", "b.png").unwrap_err();
        }
        service.reader().read_bytes("", "a.png").unwrap();
        assert_eq!(
            build.list(),
            vec![
                FileRead { path: String::from("a.png"), found: true },
                FileRead { path: String::from("b.png"), found: false },
            ]
        );
    }

    #[test]
    fn a_path_whose_answer_changed_is_named_once() {
        let shared = Arc::new(Mutex::new(MemoryFiles::new().with("a.flm", "A").with("p/b.flm", "B")));
        struct Shared(Arc<Mutex<MemoryFiles>>);
        impl Files for Shared {
            fn read(&self, path: &str) -> Result<Vec<u8>, FileError> {
                self.0.lock().unwrap().read(path)
            }
            fn entries(&self, path: &str) -> Result<Vec<String>, FileError> {
                self.0.lock().unwrap().entries(path)
            }
        }
        let files = BuildFiles::new(Arc::new(Shared(Arc::clone(&shared))));
        files.read_bytes("", "a.flm").unwrap();
        files.read_bytes("", "a.flm").unwrap();
        files.read_bytes("", "c.flm").unwrap_err();
        files.entries("", "p").unwrap();
        assert!(files.changed().is_empty());
        shared.lock().unwrap().insert("a.flm", "A2");
        shared.lock().unwrap().insert("c.flm", "C");
        shared.lock().unwrap().insert("p/d.flm", "D");
        assert_eq!(files.changed(), ["a.flm", "c.flm", "p"]);
        assert_eq!(files.list().len(), 4);
    }

    #[test]
    fn bytes_that_are_not_utf8_are_not_text() {
        let mut memory = MemoryFiles::new();
        memory.insert("x.bin", vec![0xff, 0xfe]);
        let files = BuildFiles::new(Arc::new(memory));
        let error = files.read_text("", "x.bin").unwrap_err();
        assert_eq!(error.kind, FileErrorKind::NotText);
    }
}
