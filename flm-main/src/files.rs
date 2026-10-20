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
//! [`BuildFiles`] while it runs. [`fold`] normalizes the paths the host builds, so that one
//! file reached through two spellings of its path has one name.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

/// The reason a file or a folder could not be read, as one of four cases: the
/// [`kind`](FileError::kind) of a [`FileError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileErrorKind {
    /// Nothing exists at the path.
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
/// [`FileErrorKind::Unreadable`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileError {
    /// The path, as resolved.
    pub path: PathBuf,
    /// Why the file could not be read.
    pub kind: FileErrorKind,
    /// The explanation given by the [`Files`] implementation (for [`DiskFiles`], the
    /// operating system's error text).
    pub detail: String,
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            FileErrorKind::NotFound => write!(f, "the file `{}` must exist", self.path.display()),
            FileErrorKind::Unreadable => {
                write!(f, "the file `{}` cannot be read: {}", self.path.display(), self.detail)
            }
            FileErrorKind::NotText => {
                write!(f, "the file `{}` must be text encoded as UTF-8", self.path.display())
            }
            FileErrorKind::NotAFolder => write!(f, "`{}` must be a folder, not a file", self.path.display()),
        }
    }
}

impl std::error::Error for FileError {}

/// Reads the bytes of a file, or lists the entries of a folder, at a path that is already
/// resolved (joined to its base folder).
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
    fn read(&self, path: &Path) -> Result<Vec<u8>, FileError>;

    /// The names of the entries — files and folders — of the folder at `path`, in name
    /// order.
    ///
    /// # Errors
    ///
    /// A [`FileError`] of kind [`FileErrorKind::NotFound`] when nothing exists at `path`,
    /// of kind [`FileErrorKind::NotAFolder`] when a file does, and of kind
    /// [`FileErrorKind::Unreadable`] when a folder does and cannot be listed.
    fn entries(&self, path: &Path) -> Result<Vec<String>, FileError>;

    /// Whether `path`, which exists, lies inside `folder` once every symbolic link of both
    /// is followed. The default answers `true`: files held in memory have no links.
    /// [`DiskFiles`] follows the links on the disk; an extension's files are read only
    /// where this answers `true` ([`crate::extensions`]).
    ///
    /// # Errors
    ///
    /// A [`FileError`] when a path cannot be followed to its target.
    fn stays_within(&self, folder: &Path, path: &Path) -> Result<bool, FileError> {
        let _ = (folder, path);
        Ok(true)
    }
}

/// The [`Files`] implementation that reads the disk (cargo feature `fs`): a relative path is
/// read relative to the process's current folder, as the operating system reads it. The
/// empty path stands for the current folder when a folder is listed.
#[cfg(feature = "fs")]
#[derive(Debug, Clone, Copy, Default)]
pub struct DiskFiles;

#[cfg(feature = "fs")]
impl Files for DiskFiles {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FileError> {
        std::fs::read(path).map_err(|error| FileError {
            path: path.to_path_buf(),
            kind: if error.kind() == std::io::ErrorKind::NotFound {
                FileErrorKind::NotFound
            } else {
                FileErrorKind::Unreadable
            },
            detail: error.to_string(),
        })
    }

    fn entries(&self, path: &Path) -> Result<Vec<String>, FileError> {
        let error = |error: std::io::Error| FileError {
            path: path.to_path_buf(),
            kind: match error.kind() {
                std::io::ErrorKind::NotFound => FileErrorKind::NotFound,
                std::io::ErrorKind::NotADirectory => FileErrorKind::NotAFolder,
                _ => FileErrorKind::Unreadable,
            },
            detail: error.to_string(),
        };
        // The current folder is written as the empty path.
        let folder = if path.as_os_str().is_empty() { Path::new(".") } else { path };
        let mut names = Vec::new();
        for entry in std::fs::read_dir(folder).map_err(error)? {
            names.push(entry.map_err(error)?.file_name().to_string_lossy().into_owned());
        }
        names.sort();
        Ok(names)
    }

    /// Both paths made canonical (every link followed, `.` and `..` resolved), then
    /// compared.
    fn stays_within(&self, folder: &Path, path: &Path) -> Result<bool, FileError> {
        let canonical = |path: &Path| {
            std::fs::canonicalize(if path.as_os_str().is_empty() { Path::new(".") } else { path }).map_err(|error| {
                FileError {
                    path: path.to_path_buf(),
                    kind: if error.kind() == std::io::ErrorKind::NotFound {
                        FileErrorKind::NotFound
                    } else {
                        FileErrorKind::Unreadable
                    },
                    detail: error.to_string(),
                }
            })
        };
        Ok(canonical(path)?.starts_with(canonical(folder)?))
    }
}

/// The [`Files`] implementation that reads files held in memory, each stored under the
/// path at which it is read.
///
/// A path is looked up exactly as it is given, after [`BuildFiles`] has resolved it:
/// `notes/a.flm` written in a file of the folder `doc/` is looked up as `doc/notes/a.flm`.
/// A folder exists when a file is stored under it, directly or deeper.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use flm_main::files::{Files, MemoryFiles};
///
/// let files = MemoryFiles::new().with("doc/paper.flm", "Hello.");
/// assert_eq!(files.read(Path::new("doc/paper.flm")).unwrap(), b"Hello.");
/// assert!(files.read(Path::new("doc/other.flm")).is_err());
/// ```
#[derive(Debug, Clone, Default)]
pub struct MemoryFiles {
    files: BTreeMap<PathBuf, Vec<u8>>,
}

impl MemoryFiles {
    /// Returns an empty set of files.
    pub fn new() -> MemoryFiles {
        MemoryFiles::default()
    }

    /// Stores `bytes` as the file at `path`, replacing any file stored there. Returns
    /// `self`, for chaining.
    pub fn insert(&mut self, path: impl Into<PathBuf>, bytes: impl Into<Vec<u8>>) -> &mut MemoryFiles {
        self.files.insert(path.into(), bytes.into());
        self
    }

    /// Returns these files with `text` stored as the file at `path` (a builder form of
    /// [`insert`](MemoryFiles::insert)).
    pub fn with(mut self, path: impl Into<PathBuf>, text: &str) -> MemoryFiles {
        self.insert(path, text.as_bytes());
        self
    }
}

impl Files for MemoryFiles {
    fn read(&self, path: &Path) -> Result<Vec<u8>, FileError> {
        self.files.get(path).cloned().ok_or_else(|| FileError {
            path: path.to_path_buf(),
            kind: FileErrorKind::NotFound,
            detail: String::from("no such file in memory"),
        })
    }

    /// A folder exists in memory when a file is stored under it, directly or deeper.
    fn entries(&self, path: &Path) -> Result<Vec<String>, FileError> {
        if self.files.contains_key(path) {
            return Err(FileError {
                path: path.to_path_buf(),
                kind: FileErrorKind::NotAFolder,
                detail: String::from("a file in memory"),
            });
        }
        let mut names: Vec<String> = self
            .files
            .keys()
            .filter_map(|file| file.strip_prefix(path).ok())
            .filter_map(|rest| rest.components().next())
            .map(|first| first.as_os_str().to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .collect();
        names.dedup();
        if names.is_empty() {
            return Err(FileError {
                path: path.to_path_buf(),
                kind: FileErrorKind::NotFound,
                detail: String::from("no such folder in memory"),
            });
        }
        Ok(names)
    }
}

/// One path that a build looked for, by reading a file or listing a folder, and whether
/// something was found there; [`BuildFiles::list`] returns these.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct FileRead {
    /// The path, as resolved.
    pub path: PathBuf,
    /// Whether a file (or, for a listing, a folder) was found at the path. A path where
    /// nothing was found is recorded too, so that a file appearing there later can be
    /// noticed. A file that exists and cannot be read counts as found.
    pub found: bool,
}

/// The file reader of one build: reads through a [`Files`], resolves each relative path
/// against a base folder, and records every path it looked for.
///
/// A path is *resolved* by joining it to a base folder: the folder of the file in which
/// the path is written, or, for a document read from standard input or given as text on
/// the command line, the current folder, written as the empty path. An absolute path is
/// kept as it is. A clone shares the same record, so every module of one build that holds
/// a clone adds to one list ([`list`](BuildFiles::list)).
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

    /// Returns `path` resolved against the folder `base`: `base` joined with `path`, or
    /// `path` itself when it is absolute. The result is not normalized (see [`fold`]).
    pub fn resolve(base: &Path, path: &Path) -> PathBuf {
        base.join(path)
    }

    /// Returns the bytes of the file at `path`, resolved against `base`
    /// ([`BuildFiles::resolve`]), and records the path, whether a file was found or not.
    ///
    /// # Errors
    ///
    /// The [`FileError`] returned by the [`Files`] implementation, when the file cannot be
    /// read.
    pub fn read_bytes(&self, base: &Path, path: &Path) -> Result<Vec<u8>, FileError> {
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
    pub fn read_text(&self, base: &Path, path: &Path) -> Result<String, FileError> {
        let bytes = self.read_bytes(base, path)?;
        String::from_utf8(bytes).map_err(|error| FileError {
            path: BuildFiles::resolve(base, path),
            kind: FileErrorKind::NotText,
            detail: error.to_string(),
        })
    }

    /// Returns the names of the entries (files and folders) of the folder at `path`,
    /// resolved against `base`, in name order, and records the folder, whether it was found
    /// or not, so that a file appearing in it later can be noticed.
    ///
    /// # Errors
    ///
    /// The [`FileError`] returned by the [`Files`] implementation, when the folder cannot be
    /// listed.
    pub fn entries(&self, base: &Path, path: &Path) -> Result<Vec<String>, FileError> {
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
    pub fn stays_within(&self, base: &Path, folder: &Path, path: &Path) -> Result<bool, FileError> {
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
    pub fn changed(&self) -> Vec<PathBuf> {
        let mut changed: Vec<PathBuf> = Vec::new();
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
    fn record<T: Hash>(&self, path: PathBuf, folder: bool, result: &Result<T, FileError>) {
        let found = !matches!(result, Err(error) if error.kind == FileErrorKind::NotFound);
        let looked = Looked { read: FileRead { path, found }, folder, answer: digest(result) };
        self.read.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(looked);
    }

    fn looked(&self) -> Vec<Looked> {
        self.read.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }
}

/// Returns `path` normalized without looking at the disk: every `.` component removed, and
/// every `..` removed together with the name before it, where there is one:
/// `docs/./a/../b.flm` becomes `docs/b.flm`, and `../b.flm` is unchanged. Links are not
/// followed.
///
/// The host normalizes with this function every path it builds (an included file's path,
/// an extension's manifest or file, a path written in a setting), so that one file reached through
/// two spellings of its path gets one name.
///
/// # Examples
///
/// ```
/// use std::path::{Path, PathBuf};
/// use flm_main::files::fold;
///
/// assert_eq!(fold(Path::new("docs/./a/../b.flm")), PathBuf::from("docs/b.flm"));
/// assert_eq!(fold(Path::new("../b.flm")), PathBuf::from("../b.flm"));
/// ```
pub fn fold(path: &Path) -> PathBuf {
    let mut folded = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if matches!(folded.components().next_back(), Some(Component::Normal(_))) => {
                folded.pop();
            }
            other => folded.push(other.as_os_str()),
        }
    }
    folded
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
        let base = Path::new("");
        assert!(files.stays_within(base, &folder, &folder.join("inside.txt")).unwrap());
        assert!(!files.stays_within(base, &folder, &folder.join("link.txt")).unwrap());
        // A folder that is itself a link: its files lie inside its target.
        assert!(files.stays_within(base, &linked, &linked.join("inside.txt")).unwrap());
        assert!(files.list().is_empty());
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_relative_path_is_resolved_against_the_folder_that_names_it() {
        let files = BuildFiles::new(Arc::new(MemoryFiles::new().with("doc/notes/a.flm", "A")));
        assert_eq!(files.read_text(Path::new("doc"), Path::new("notes/a.flm")).unwrap(), "A");
    }

    #[test]
    fn every_path_looked_for_is_listed_found_or_not() {
        let files = BuildFiles::new(Arc::new(MemoryFiles::new().with("a.flm", "A")));
        files.read_text(Path::new(""), Path::new("a.flm")).unwrap();
        let missing = files.read_text(Path::new(""), Path::new("b.flm")).unwrap_err();
        assert_eq!(missing.kind, FileErrorKind::NotFound);
        assert_eq!(
            files.list(),
            vec![
                FileRead { path: PathBuf::from("a.flm"), found: true },
                FileRead { path: PathBuf::from("b.flm"), found: false },
            ]
        );
    }

    #[test]
    fn a_folder_lists_its_files_and_folders_once_each() {
        let memory = MemoryFiles::new().with("p/a.yaml", "").with("p/b/x.yaml", "").with("p/b/y.yaml", "");
        let files = BuildFiles::new(Arc::new(memory));
        assert_eq!(files.entries(Path::new(""), Path::new("p")).unwrap(), ["a.yaml", "b"]);
        assert_eq!(files.entries(Path::new("p"), Path::new("c")).unwrap_err().kind, FileErrorKind::NotFound);
        assert_eq!(files.list()[1], FileRead { path: PathBuf::from("p/c"), found: false });
    }

    #[test]
    fn a_service_reads_through_the_build_attached_and_lists_nothing_otherwise() {
        let service = ServiceFiles::new(Arc::new(MemoryFiles::new().with("a.png", "A")));
        service.reader().read_bytes(Path::new(""), Path::new("a.png")).unwrap();
        let build = BuildFiles::new(Arc::new(MemoryFiles::new().with("a.png", "A")));
        {
            let _attached = service.attach(build.clone());
            service.reader().read_bytes(Path::new(""), Path::new("a.png")).unwrap();
            service.reader().read_bytes(Path::new(""), Path::new("b.png")).unwrap_err();
        }
        service.reader().read_bytes(Path::new(""), Path::new("a.png")).unwrap();
        assert_eq!(
            build.list(),
            vec![
                FileRead { path: PathBuf::from("a.png"), found: true },
                FileRead { path: PathBuf::from("b.png"), found: false },
            ]
        );
    }

    #[test]
    fn a_path_whose_answer_changed_is_named_once() {
        let shared = Arc::new(Mutex::new(MemoryFiles::new().with("a.flm", "A").with("p/b.flm", "B")));
        struct Shared(Arc<Mutex<MemoryFiles>>);
        impl Files for Shared {
            fn read(&self, path: &Path) -> Result<Vec<u8>, FileError> {
                self.0.lock().unwrap().read(path)
            }
            fn entries(&self, path: &Path) -> Result<Vec<String>, FileError> {
                self.0.lock().unwrap().entries(path)
            }
        }
        let files = BuildFiles::new(Arc::new(Shared(Arc::clone(&shared))));
        files.read_bytes(Path::new(""), Path::new("a.flm")).unwrap();
        files.read_bytes(Path::new(""), Path::new("a.flm")).unwrap();
        files.read_bytes(Path::new(""), Path::new("c.flm")).unwrap_err();
        files.entries(Path::new(""), Path::new("p")).unwrap();
        assert!(files.changed().is_empty());
        shared.lock().unwrap().insert("a.flm", "A2");
        shared.lock().unwrap().insert("c.flm", "C");
        shared.lock().unwrap().insert("p/d.flm", "D");
        assert_eq!(files.changed(), vec![PathBuf::from("a.flm"), PathBuf::from("c.flm"), PathBuf::from("p")]);
        assert_eq!(files.list().len(), 4);
    }

    #[test]
    fn bytes_that_are_not_utf8_are_not_text() {
        let mut memory = MemoryFiles::new();
        memory.insert("x.bin", vec![0xff, 0xfe]);
        let files = BuildFiles::new(Arc::new(memory));
        let error = files.read_text(Path::new(""), Path::new("x.bin")).unwrap_err();
        assert_eq!(error.kind, FileErrorKind::NotText);
    }
}
