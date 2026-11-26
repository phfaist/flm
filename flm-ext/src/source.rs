//! Where extensions come from: the host's [`ExtensionSource`]s (a locator in,
//! an extension out) and each extension's [`ExtensionFiles`] (read a text,
//! read a file's bytes, read a data file, list a folder, by a path written in
//! a file of the extension), with the memory implementations [`MemorySource`]
//! and [`MemoryFiles`], and the row type of a table of files
//! ([`BundledExtension`]), in which a program or a module describes the
//! extensions it carries as their files.
//!
//! The extension's files are consulted at load, on the loading thread, so the
//! trait itself needs no thread-safety; a compose template the extension
//! ships keeps a handle to them that may be read from any thread
//! ([`ExtensionFiles::shared`]), through which it reads, when it is applied,
//! the files it includes. A source is shared by the
//! [loader](crate::loader::ExtensionLoader), which the catalog holds as a
//! `PackageResolver`, so it is `Send + Sync`.
//!
//! # Paths inside an extension
//!
//! A path inside an extension is text in which `/` separates the names, on
//! every system; every other character, `\` included, belongs to a name. A
//! path is relative to the file that names it: a path the manifest writes
//! (`extension.js.main`, `extension.wasm.module`) to the extension's folder,
//! where the manifest stands; a path a compose template's description writes
//! to the description's folder. Every read is asked as "the path `X`,
//! written in the file `Y`", `Y` itself a path relative to the extension's
//! folder, or the empty text for that folder itself. The implementation of
//! [`ExtensionFiles`], the host's, joins the two and checks that the result
//! stays inside the extension's folder: it refuses a path that is absolute,
//! one whose `..` climbs above the extension's folder, and, on a disk, a file
//! whose real location (symbolic links followed) is outside that folder
//! ([`FilesErrorKind`]). The check is against a broken or misconfigured
//! extension, not a protection. A path that names no file on the system
//! (on Windows, one holding a name with `\` or `:`) is an error that says
//! why ([`FilesErrorKind::NoSystemForm`]).
//!
//! The modules a JavaScript extension imports are named by JavaScript's own
//! rule for an import's specifier, which the engine `js` applies
//! ([`crate::quickjs`]); the engine then reads each module through these
//! files, by its path relative to the extension's folder. The main module,
//! a path of the manifest, is read as written, and named by the same rule.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use crate::entry::Locator;

/// A source of extensions: what a host gives the loader to find an extension
/// by its locator (the filesystem source looks in `_flmextensions` and the
/// other folders of the search path).
pub trait ExtensionSource: Send + Sync {
    /// The extension at `locator`, or `Ok(None)` when this source has none.
    ///
    /// # Errors
    ///
    /// A failure other than "not here": a file that cannot be read or is not
    /// UTF-8, a manifest that does not parse, two layouts at once.
    fn find(&self, locator: &Locator) -> Result<Option<FoundExtension>, SourceError>;
}

/// An extension a source found: its manifest as a parsed value, its identity
/// (the source's string for it; for the filesystem source, the path of the
/// manifest, joined as text to the folder of the search path that holds it),
/// its files, and where each key of the manifest stands in the manifest's
/// text.
pub struct FoundExtension {
    /// The parsed manifest.
    pub manifest: Value,
    /// The source's string for the extension: the prefix of its factory names.
    pub identity: String,
    /// The extension's files.
    pub files: Box<dyn ExtensionFiles>,
    /// The place of a key of the manifest in its text, by key path
    /// ([`PlaceOf`]), when the source keeps the places (a memory source does
    /// not). The loader places with it every problem of the manifest it
    /// reports by key path.
    pub place_of: Option<PlaceOf>,
}

/// A line and a column of a manifest's text, both counted from 1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Place {
    /// The line, from 1.
    pub line: u32,
    /// The column, from 1.
    pub column: u32,
}

/// The place in the manifest's text of the value at a key path, or `None`
/// when the source cannot place it. The crate writes the key paths, the
/// source reads them: `KEY(.KEY)*`, each `KEY` a map's key optionally
/// followed by `[N]`, the item at the index `N` (from 0) of the list it holds
/// (`extension.js.main`, `flm.use[2]`, `flm.features.text.width`).
pub type PlaceOf = Arc<dyn Fn(&str) -> Option<Place> + Send + Sync>;

/// The files of one extension, each read by a path `path` written in the
/// file `from` ([module documentation](self#paths-inside-an-extension)):
/// `from` is a path relative to the extension's folder (the empty text for
/// that folder itself), and `path` is relative to the folder of `from` unless
/// it is absolute. The implementation joins the two and refuses a path that
/// does not stay inside the extension's folder. A single-file extension has
/// no folder: its files answer "not there" to everything.
pub trait ExtensionFiles {
    /// The text of the file at `path`, written in `from`; `Ok(None)` when
    /// nothing is there.
    ///
    /// # Errors
    ///
    /// A path refused ([`FilesErrorKind::Absolute`],
    /// [`FilesErrorKind::Above`], [`FilesErrorKind::Link`]), a path that
    /// names no file on the system ([`FilesErrorKind::NoSystemForm`]), or a
    /// file that cannot be read or is not UTF-8 text
    /// ([`FilesErrorKind::Unreadable`]).
    fn read_text(&self, from: &str, path: &str) -> Result<Option<String>, FilesError>;

    /// The bytes of the file at `path`, written in `from`, whatever they hold
    /// (a text, an image); `Ok(None)` when nothing is there. A compose
    /// template reads through it the files its description lists.
    ///
    /// # Errors
    ///
    /// A path refused or that names no file on the system, or a file that
    /// cannot be read, as for [`read_text`](ExtensionFiles::read_text).
    fn read_bytes(&self, from: &str, path: &str) -> Result<Option<Vec<u8>>, FilesError>;

    /// The value of the data file at `path`, written in `from` (YAML on disk,
    /// parsed by the source as the manifest was); `Ok(None)` when nothing is
    /// there.
    ///
    /// # Errors
    ///
    /// As [`read_text`](ExtensionFiles::read_text), and a file that does not
    /// parse ([`FilesErrorKind::Unreadable`]).
    fn read_value(&self, from: &str, path: &str) -> Result<Option<Value>, FilesError>;

    /// The names of the entries of the folder at `path`, written in `from`
    /// (`from` and `path` both empty for the extension's folder);
    /// `Ok(None)` when no folder is there.
    ///
    /// # Errors
    ///
    /// As [`read_text`](ExtensionFiles::read_text).
    fn list(&self, from: &str, path: &str) -> Result<Option<Vec<String>>, FilesError>;

    /// These files, as a handle that may be kept after the load and read from
    /// any thread. The loader registers it under each use name that reached
    /// the extension ([`ExtensionsByName`](crate::paths::ExtensionsByName)),
    /// through which the root `ext+NAME` is read: by the loader's `files`
    /// service, for the code of an extension, and by a template engine, for
    /// the files a compose template includes when it is applied. A read
    /// through the handle is a read of these files.
    fn shared(&self) -> Arc<dyn ExtensionFiles + Send + Sync>;
}

/// A source's failure other than "not here": unreadable, not UTF-8, a
/// manifest that does not parse, two layouts at once. Text for a person.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceError {
    /// The locator asked, as written.
    pub locator: String,
    /// The file at fault, as the host shows it, when there is one.
    pub path: Option<String>,
    /// Where in that file, when the source knows (a manifest that does not
    /// parse, at the parser's place).
    pub place: Option<Place>,
    /// What failed, as text for a person.
    pub detail: String,
}

impl fmt::Display for SourceError {
    /// The detail.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for SourceError {}

/// A failure to read the extension's files other than absence: the path as
/// the caller gave it, and why. The caller words its own message, naming
/// what the path was for; the `Display` text is a plain one, for a caller
/// that adds nothing ("the path `../x.js` leaves the extension's folder").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesError {
    /// The path asked, as the caller gave it (relative to the file it was
    /// written in).
    pub path: String,
    /// Why it was not read.
    pub kind: FilesErrorKind,
}

/// Why a path of an extension was not read ([`FilesError`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilesErrorKind {
    /// The path is absolute: it begins with `/`, or is `~`, or begins with
    /// `~/`.
    Absolute,
    /// A `..` of the path, joined to the folder of the file it is written in,
    /// climbs above the extension's folder.
    Above,
    /// The file or the folder at the path lies outside the extension's folder
    /// once every symbolic link is followed.
    Link,
    /// The path names no file on the system the program runs on (on
    /// Windows, for instance, a name of the path holds `\` or `:`). The text
    /// says why, for a person, and names what is at fault ("a file name on
    /// Windows may not contain ‘\’ (‘lib\x.js’)").
    NoSystemForm(String),
    /// Something is at the path and cannot be read as asked: the system's
    /// error, a file that is not UTF-8 text, a data file that does not parse.
    /// The text says which, for a person, and names the file.
    Unreadable(String),
}

impl FilesError {
    /// The failure `kind` of the path `path`.
    pub fn new(path: &str, kind: FilesErrorKind) -> FilesError {
        FilesError { path: path.to_string(), kind }
    }
}

impl fmt::Display for FilesError {
    /// "the path `P` must be relative", "the path `P` leaves the extension's
    /// folder", "the path ‘P’ leads outside the extension's folder through a
    /// symbolic link", "the path ‘P’ names no file on this system: " and the
    /// reason, or the text of [`FilesErrorKind::Unreadable`].
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = &self.path;
        match &self.kind {
            FilesErrorKind::Absolute => write!(f, "the path `{path}` must be relative"),
            FilesErrorKind::Above => write!(f, "the path `{path}` leaves the extension's folder"),
            FilesErrorKind::Link => {
                write!(f, "the path ‘{path}’ leads outside the extension's folder through a symbolic link")
            }
            FilesErrorKind::NoSystemForm(reason) => {
                write!(f, "the path ‘{path}’ names no file on this system: {reason}")
            }
            FilesErrorKind::Unreadable(detail) => f.write_str(detail),
        }
    }
}

impl std::error::Error for FilesError {}

/// One file held in memory: a text, a data file's value parsed already, or
/// bytes.
#[derive(Debug, Clone, PartialEq)]
pub enum MemoryFile {
    /// A text file.
    Text(String),
    /// A data file, parsed.
    Value(Value),
    /// A file held as bytes (an image, or a text not yet decoded).
    Bytes(Vec<u8>),
}

/// An extension's files held in memory, each under its path relative to the
/// extension's folder. A read joins the path to the folder of the file it is
/// written in as text, and looks the result up with its `.` and empty names
/// left out and each `..` taking off the name before it
/// (`templates/html/./../logo.png` is the file held at `templates/logo.png`);
/// a path that is absolute, or whose `..` climbs above the extension's
/// folder, is refused. Nothing is a symbolic link. A
/// [`list`](ExtensionFiles::list) answers the names one level below the path
/// (a folder exists when a file is held below it); a
/// [`read_text`](ExtensionFiles::read_text) or a
/// [`read_bytes`](ExtensionFiles::read_bytes) of a value, a
/// [`read_value`](ExtensionFiles::read_value) of a text or of bytes, and a
/// `read_text` of bytes that are not UTF-8 are a [`FilesError`] of kind
/// [`FilesErrorKind::Unreadable`]. A `read_bytes` of a text gives its UTF-8
/// bytes.
#[derive(Debug, Clone, Default)]
pub struct MemoryFiles {
    files: BTreeMap<String, MemoryFile>,
}

impl MemoryFiles {
    /// No file.
    pub fn new() -> MemoryFiles {
        MemoryFiles::default()
    }

    /// The same files and the text `text` at `path`, relative to the
    /// extension's folder (`./main.js` is held as `main.js`).
    pub fn with_text(self, path: &str, text: &str) -> MemoryFiles {
        self.with(path, MemoryFile::Text(text.to_string()))
    }

    /// The same files and the data file `value` at `path`, relative to the
    /// extension's folder.
    pub fn with_value(self, path: &str, value: Value) -> MemoryFiles {
        self.with(path, MemoryFile::Value(value))
    }

    /// The same files and the bytes `bytes` at `path`, relative to the
    /// extension's folder.
    pub fn with_bytes(self, path: &str, bytes: &[u8]) -> MemoryFiles {
        self.with(path, MemoryFile::Bytes(bytes.to_vec()))
    }

    fn with(mut self, path: &str, file: MemoryFile) -> MemoryFiles {
        let key = held_at("", path).unwrap_or_else(|_| path.to_string());
        self.files.insert(key, file);
        self
    }

    /// The key of the file at `path`, written in `from`, or the refusal.
    fn key(from: &str, path: &str) -> Result<String, FilesError> {
        held_at(from, path).map_err(|outside| {
            let kind = match outside {
                Outside::Absolute => FilesErrorKind::Absolute,
                Outside::Above => FilesErrorKind::Above,
            };
            FilesError::new(path, kind)
        })
    }
}

/// Whether `path` is absolute: it begins with `/`, or is `~`, or begins with
/// `~/`.
pub(crate) fn is_absolute(path: &str) -> bool {
    path.starts_with('/') || path == "~" || path.starts_with("~/")
}

/// Why a path does not stay inside a base folder ([`held_at`]).
pub(crate) enum Outside {
    /// The path is absolute.
    Absolute,
    /// A `..` climbs above the folder.
    Above,
}

/// The rule of a path inside a base folder (an extension's folder, the
/// program's own files): `path` joined to the folder of the file `from` (the
/// text before its last `/`), unless absolute, then its names with the `.`
/// and empty ones left out and each `..` taking off the name before it. It is
/// the key under which [`MemoryFiles`] holds the file at `path`, written in
/// `from`.
///
/// # Errors
///
/// [`Outside::Absolute`] when the joined path is absolute ([`is_absolute`]);
/// [`Outside::Above`] when a `..` has no name before it to take off.
pub(crate) fn held_at(from: &str, path: &str) -> Result<String, Outside> {
    let joined = match from.rfind('/') {
        Some(slash) if !is_absolute(path) => format!("{}/{path}", &from[..slash]),
        _ => path.to_string(),
    };
    if is_absolute(&joined) {
        return Err(Outside::Absolute);
    }
    let mut names: Vec<&str> = Vec::new();
    for name in joined.split('/') {
        match name {
            "" | "." => {}
            ".." => {
                names.pop().ok_or(Outside::Above)?;
            }
            name => names.push(name),
        }
    }
    Ok(names.join("/"))
}

impl ExtensionFiles for MemoryFiles {
    fn read_text(&self, from: &str, path: &str) -> Result<Option<String>, FilesError> {
        let key = MemoryFiles::key(from, path)?;
        let unreadable = |detail: String| FilesError::new(path, FilesErrorKind::Unreadable(detail));
        match self.files.get(&key) {
            None => Ok(None),
            Some(MemoryFile::Text(text)) => Ok(Some(text.clone())),
            Some(MemoryFile::Value(_)) => {
                Err(unreadable(format!("the file `{key}` must be a text, and it is held as a data value")))
            }
            Some(MemoryFile::Bytes(bytes)) => String::from_utf8(bytes.clone())
                .map(Some)
                .map_err(|_| unreadable(format!("the file `{key}` must be UTF-8 text"))),
        }
    }

    fn read_bytes(&self, from: &str, path: &str) -> Result<Option<Vec<u8>>, FilesError> {
        let key = MemoryFiles::key(from, path)?;
        match self.files.get(&key) {
            None => Ok(None),
            Some(MemoryFile::Text(text)) => Ok(Some(text.as_bytes().to_vec())),
            Some(MemoryFile::Bytes(bytes)) => Ok(Some(bytes.clone())),
            Some(MemoryFile::Value(_)) => Err(FilesError::new(
                path,
                FilesErrorKind::Unreadable(format!(
                    "the file `{key}` must be a file of bytes, and it is held as a data value"
                )),
            )),
        }
    }

    fn read_value(&self, from: &str, path: &str) -> Result<Option<Value>, FilesError> {
        let key = MemoryFiles::key(from, path)?;
        let held_as = |held: &str| {
            FilesError::new(
                path,
                FilesErrorKind::Unreadable(format!("the file `{key}` must be a data file, and it is held as {held}")),
            )
        };
        match self.files.get(&key) {
            None => Ok(None),
            Some(MemoryFile::Value(value)) => Ok(Some(value.clone())),
            Some(MemoryFile::Text(_)) => Err(held_as("a text")),
            Some(MemoryFile::Bytes(_)) => Err(held_as("bytes")),
        }
    }

    fn list(&self, from: &str, path: &str) -> Result<Option<Vec<String>>, FilesError> {
        let key = MemoryFiles::key(from, path)?;
        let mut names: Vec<String> = Vec::new();
        for held in self.files.keys() {
            let below =
                if key.is_empty() { Some(held.as_str()) } else { held.strip_prefix(&key).and_then(|rest| rest.strip_prefix('/')) };
            if let Some(below) = below {
                let name = below.split('/').next().unwrap_or(below);
                if !names.iter().any(|known| known == name) {
                    names.push(name.to_string());
                }
            }
        }
        Ok(if names.is_empty() { None } else { Some(names) })
    }

    /// A copy of these files.
    fn shared(&self) -> Arc<dyn ExtensionFiles + Send + Sync> {
        Arc::new(self.clone())
    }
}

/// One extension a [`MemorySource`] holds.
#[derive(Debug, Clone)]
struct MemoryExtension {
    identity: String,
    manifest: Value,
    files: MemoryFiles,
}

/// A source over memory: each locator, as written, mapped to an identity, a
/// manifest and the files. For tests and for a host that holds its
/// extensions in memory.
#[derive(Debug, Clone, Default)]
pub struct MemorySource {
    extensions: BTreeMap<String, MemoryExtension>,
}

impl MemorySource {
    /// No extension.
    pub fn new() -> MemorySource {
        MemorySource::default()
    }

    /// The same source, and the locator `locator` answering the extension
    /// `identity` of the manifest `manifest` and the files `files` (several
    /// locators may answer one identity).
    pub fn with(mut self, locator: &str, identity: &str, manifest: Value, files: MemoryFiles) -> MemorySource {
        self.extensions
            .insert(locator.to_string(), MemoryExtension { identity: identity.to_string(), manifest, files });
        self
    }
}

impl ExtensionSource for MemorySource {
    fn find(&self, locator: &Locator) -> Result<Option<FoundExtension>, SourceError> {
        let Locator::Named(text) = locator else { return Ok(None) };
        Ok(self.extensions.get(text).map(|extension| FoundExtension {
            manifest: extension.manifest.clone(),
            identity: extension.identity.clone(),
            files: Box::new(extension.files.clone()),
            place_of: None,
        }))
    }
}

/// A `BundledExtension` is one row of a *table of files*: an extension that a
/// program carries, described by its folder, its stem and its files, the
/// manifest among them. A table of files is a static slice of rows,
/// `&'static [BundledExtension]`; the crate `flm-extensions` provides one for
/// the extensions shipped with FLM, and the host library `flm-host` finds an
/// extension by its stem in the table that a program gives it. The type needs
/// no cargo feature. The code of an extension written in Rust is not in this
/// table: it is a row of the *table of code* (`flm_ext::guest::NamedExtension`,
/// cargo feature `guest`).
#[derive(Debug)]
pub struct BundledExtension {
    /// The folder that holds the extension among the folders of the
    /// extensions shipped with FLM: `data`, `js` or `rust`.
    pub folder: &'static str,
    /// The extension's stem: the name of its folder, or of its file without
    /// `.yaml`.
    pub stem: &'static str,
    /// The extension's files, in path order.
    pub files: &'static [BundledFile],
}

impl BundledExtension {
    /// The file at `path`, if the extension has one there.
    pub fn file(&self, path: &str) -> Option<&'static BundledFile> {
        self.files.iter().find(|file| file.path == path)
    }
}

/// One file of a row of a table of files ([`BundledExtension`]).
#[derive(Debug)]
pub struct BundledFile {
    /// The path relative to the extension's folder, with `/` between the
    /// names (`templates/html/lecturenotes/flmtemplate.yaml`); for an
    /// extension written as one file, the file's name (`NAME.yaml`).
    pub path: &'static str,
    /// The file's bytes.
    pub bytes: &'static [u8],
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn memory_files_read_and_list_by_relative_path() {
        let files = MemoryFiles::new()
            .with_text("./main.js", "export default 1;")
            .with_value("templates/html/fancy.yaml", json!({ "output": "html" }))
            .with_text("templates/html/fancy.html", "<p/>")
            .with_text("templates/latex/x.tex", "");
        assert_eq!(files.read_text("", "main.js"), Ok(Some("export default 1;".to_string())));
        assert_eq!(files.read_text("", "absent.js"), Ok(None));
        assert_eq!(files.read_value("", "templates/html/fancy.yaml"), Ok(Some(json!({ "output": "html" }))));
        assert!(files.read_text("", "templates/html/fancy.yaml").is_err());
        assert!(files.read_value("", "main.js").is_err());
        assert_eq!(files.list("", ""), Ok(Some(vec!["main.js".to_string(), "templates".to_string()])));
        assert_eq!(files.list("", "templates"), Ok(Some(vec!["html".to_string(), "latex".to_string()])));
        assert_eq!(files.list("", "templates/html"), Ok(Some(vec!["fancy.html".to_string(), "fancy.yaml".to_string()])));
        assert_eq!(files.list("", "main.js"), Ok(None));
        assert_eq!(files.list("", "templ"), Ok(None));
    }

    #[test]
    fn a_path_is_relative_to_the_file_it_is_written_in_and_stays_inside_the_extension() {
        let files = MemoryFiles::new().with_text("templates/html/fancy.html", "<p/>").with_text("logo.svg", "<svg/>");
        let description = "templates/html/fancy.yaml";
        assert_eq!(files.read_text(description, "fancy.html"), Ok(Some("<p/>".to_string())));
        assert_eq!(files.read_text(description, "./x/../fancy.html"), Ok(Some("<p/>".to_string())));
        assert_eq!(files.read_text(description, "../../logo.svg"), Ok(Some("<svg/>".to_string())));
        assert_eq!(files.list(description, ""), Ok(Some(vec!["fancy.html".to_string()])));
        let kind = |path: &str| files.read_text(description, path).unwrap_err().kind;
        assert_eq!(kind("../../../logo.svg"), FilesErrorKind::Above);
        for path in ["/logo.svg", "~", "~/logo.svg"] {
            assert_eq!(kind(path), FilesErrorKind::Absolute, "{path}");
        }
        // `\` belongs to a name.
        let files = MemoryFiles::new().with_text("a\\b.js", "1");
        assert_eq!(files.read_text("", "a\\b.js"), Ok(Some("1".to_string())));
        assert_eq!(files.read_text("", "a/b.js"), Ok(None));
    }

    #[test]
    fn memory_files_read_bytes() {
        let files = MemoryFiles::new()
            .with_bytes("img/logo.png", &[0x89, b'P', b'N', b'G', 0xff])
            .with_text("fancy.css", "p {}")
            .with_value("fancy.yaml", json!({ "output": "html" }));
        assert_eq!(files.read_bytes("", "img/logo.png"), Ok(Some(vec![0x89, b'P', b'N', b'G', 0xff])));
        assert_eq!(files.read_bytes("", "fancy.css"), Ok(Some(b"p {}".to_vec())));
        assert_eq!(files.read_bytes("", "absent.png"), Ok(None));
        assert!(files.read_bytes("", "fancy.yaml").is_err());
        // Bytes that are not UTF-8 are not a text.
        assert!(files.read_text("", "img/logo.png").is_err());
        assert_eq!(MemoryFiles::new().with_bytes("a.txt", b"hi").read_text("", "a.txt"), Ok(Some("hi".to_string())));
        assert_eq!(files.list("", "img"), Ok(Some(vec!["logo.png".to_string()])));
    }

    #[test]
    fn memory_files_refuse_a_data_read_of_a_file_that_is_no_data_file() {
        let files = MemoryFiles::new().with_bytes("img/logo.png", &[0x89]).with_text("fancy.css", "p {}");
        let path = |path: &str| files.read_value("", path).unwrap_err().path;
        assert_eq!(path("img/logo.png"), "img/logo.png");
        assert_eq!(path("fancy.css"), "fancy.css");
    }
}
