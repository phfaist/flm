//! Where extensions come from: the host's [`ExtensionSource`]s (a locator in,
//! an extension out) and each extension's [`ExtensionFiles`] (read a text,
//! read a file's bytes, read a data file, list a folder, by a normalized
//! relative path), with the
//! memory implementations [`MemorySource`] and [`MemoryFiles`].
//!
//! The extension's files are consulted at load, on the loading thread, so the
//! trait itself needs no thread-safety; a compose template the extension
//! ships keeps a handle to them that may be read from any thread
//! ([`ExtensionFiles::shared`]), through which it reads, when it is applied,
//! the files it includes. A source is shared by the
//! [loader](crate::loader::ExtensionLoader), which the catalog holds as a
//! `PackageResolver`, so it is `Send + Sync`.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use serde_json::Value;

use crate::entry::Locator;
use crate::paths;

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
/// (the source's string for it; for the filesystem source, the folded path of
/// the manifest), its files, and where each key of the manifest stands in the
/// manifest's text.
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

/// The files of one extension, by a normalized path relative to the
/// extension's root ([`paths::normalize`]). A single-file extension has no
/// folder: its files answer "not there" to everything.
pub trait ExtensionFiles {
    /// The text of the file at `path`, `Ok(None)` when nothing is there.
    ///
    /// # Errors
    ///
    /// A failure other than absence (unreadable, not UTF-8, a link outside
    /// the extension's folder).
    fn read_text(&self, path: &str) -> Result<Option<String>, FilesError>;

    /// The bytes of the file at `path`, whatever they hold (a text, an image),
    /// `Ok(None)` when nothing is there. A compose template reads through it
    /// the files its description lists.
    ///
    /// # Errors
    ///
    /// A failure other than absence (unreadable, a link outside the
    /// extension's folder).
    fn read_bytes(&self, path: &str) -> Result<Option<Vec<u8>>, FilesError>;

    /// The value of the data file at `path` (YAML on disk, parsed by the
    /// source as the manifest was), `Ok(None)` when nothing is there.
    ///
    /// # Errors
    ///
    /// As [`read_text`](ExtensionFiles::read_text), and a file that does not
    /// parse.
    fn read_value(&self, path: &str) -> Result<Option<Value>, FilesError>;

    /// The names of the entries of the folder at `path` (the empty text for
    /// the root), `Ok(None)` when no folder is there.
    ///
    /// # Errors
    ///
    /// As [`read_text`](ExtensionFiles::read_text).
    fn list(&self, path: &str) -> Result<Option<Vec<String>>, FilesError>;

    /// These files, as a handle that may be kept after the load and read from
    /// any thread. The reader of compose templates attaches one to each
    /// template it reads ([`crate::templates::read`]), through which the
    /// template reads the files it includes when it is applied; a read through
    /// the handle is a read of these files.
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

/// A failure to read the extension's files other than absence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilesError {
    /// The path asked, relative to the extension's root.
    pub path: String,
    /// What failed, as text for a person.
    pub detail: String,
}

impl fmt::Display for FilesError {
    /// The detail.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
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

/// An extension's files held in memory, by normalized relative path. A
/// [`list`](ExtensionFiles::list) answers the names one level below the path
/// (a folder exists when a file is held below it); a
/// [`read_text`](ExtensionFiles::read_text) or a
/// [`read_bytes`](ExtensionFiles::read_bytes) of a value, a
/// [`read_value`](ExtensionFiles::read_value) of a text or of bytes, and a
/// `read_text` of bytes that are not UTF-8 are a [`FilesError`]. A
/// `read_bytes` of a text gives its UTF-8 bytes.
#[derive(Debug, Clone, Default)]
pub struct MemoryFiles {
    files: BTreeMap<String, MemoryFile>,
}

impl MemoryFiles {
    /// No file.
    pub fn new() -> MemoryFiles {
        MemoryFiles::default()
    }

    /// The same files and the text `text` at `path` (normalized).
    pub fn with_text(self, path: &str, text: &str) -> MemoryFiles {
        self.with(path, MemoryFile::Text(text.to_string()))
    }

    /// The same files and the data file `value` at `path` (normalized).
    pub fn with_value(self, path: &str, value: Value) -> MemoryFiles {
        self.with(path, MemoryFile::Value(value))
    }

    /// The same files and the bytes `bytes` at `path` (normalized).
    pub fn with_bytes(self, path: &str, bytes: &[u8]) -> MemoryFiles {
        self.with(path, MemoryFile::Bytes(bytes.to_vec()))
    }

    fn with(mut self, path: &str, file: MemoryFile) -> MemoryFiles {
        let path = paths::normalize(path).unwrap_or_else(|_| path.to_string());
        self.files.insert(path, file);
        self
    }
}

impl ExtensionFiles for MemoryFiles {
    fn read_text(&self, path: &str) -> Result<Option<String>, FilesError> {
        match self.files.get(path) {
            None => Ok(None),
            Some(MemoryFile::Text(text)) => Ok(Some(text.clone())),
            Some(MemoryFile::Value(_)) => Err(FilesError {
                path: path.to_string(),
                detail: format!("the file `{path}` must be a text, and it is held as a data value"),
            }),
            Some(MemoryFile::Bytes(bytes)) => String::from_utf8(bytes.clone()).map(Some).map_err(|_| FilesError {
                path: path.to_string(),
                detail: format!("the file `{path}` must be UTF-8 text"),
            }),
        }
    }

    fn read_bytes(&self, path: &str) -> Result<Option<Vec<u8>>, FilesError> {
        match self.files.get(path) {
            None => Ok(None),
            Some(MemoryFile::Text(text)) => Ok(Some(text.as_bytes().to_vec())),
            Some(MemoryFile::Bytes(bytes)) => Ok(Some(bytes.clone())),
            Some(MemoryFile::Value(_)) => Err(FilesError {
                path: path.to_string(),
                detail: format!("the file `{path}` must be a file of bytes, and it is held as a data value"),
            }),
        }
    }

    fn read_value(&self, path: &str) -> Result<Option<Value>, FilesError> {
        match self.files.get(path) {
            None => Ok(None),
            Some(MemoryFile::Value(value)) => Ok(Some(value.clone())),
            Some(MemoryFile::Text(_)) => Err(FilesError {
                path: path.to_string(),
                detail: format!("the file `{path}` must be a data file, and it is held as a text"),
            }),
            Some(MemoryFile::Bytes(_)) => Err(FilesError {
                path: path.to_string(),
                detail: format!("the file `{path}` must be a data file, and it is held as bytes"),
            }),
        }
    }

    fn list(&self, path: &str) -> Result<Option<Vec<String>>, FilesError> {
        let mut names: Vec<String> = Vec::new();
        for key in self.files.keys() {
            let below = if path.is_empty() { Some(key.as_str()) } else { key.strip_prefix(path).and_then(|rest| rest.strip_prefix('/')) };
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
        assert_eq!(files.read_text("main.js"), Ok(Some("export default 1;".to_string())));
        assert_eq!(files.read_text("absent.js"), Ok(None));
        assert_eq!(files.read_value("templates/html/fancy.yaml"), Ok(Some(json!({ "output": "html" }))));
        assert!(files.read_text("templates/html/fancy.yaml").is_err());
        assert!(files.read_value("main.js").is_err());
        assert_eq!(files.list(""), Ok(Some(vec!["main.js".to_string(), "templates".to_string()])));
        assert_eq!(files.list("templates"), Ok(Some(vec!["html".to_string(), "latex".to_string()])));
        assert_eq!(files.list("templates/html"), Ok(Some(vec!["fancy.html".to_string(), "fancy.yaml".to_string()])));
        assert_eq!(files.list("main.js"), Ok(None));
        assert_eq!(files.list("templ"), Ok(None));
    }

    #[test]
    fn memory_files_read_bytes() {
        let files = MemoryFiles::new()
            .with_bytes("img/logo.png", &[0x89, b'P', b'N', b'G', 0xff])
            .with_text("fancy.css", "p {}")
            .with_value("fancy.yaml", json!({ "output": "html" }));
        assert_eq!(files.read_bytes("img/logo.png"), Ok(Some(vec![0x89, b'P', b'N', b'G', 0xff])));
        assert_eq!(files.read_bytes("fancy.css"), Ok(Some(b"p {}".to_vec())));
        assert_eq!(files.read_bytes("absent.png"), Ok(None));
        assert!(files.read_bytes("fancy.yaml").is_err());
        // Bytes that are not UTF-8 are not a text.
        assert!(files.read_text("img/logo.png").is_err());
        assert_eq!(MemoryFiles::new().with_bytes("a.txt", b"hi").read_text("a.txt"), Ok(Some("hi".to_string())));
        assert_eq!(files.list("img"), Ok(Some(vec!["logo.png".to_string()])));
    }

    #[test]
    fn memory_files_name_how_a_file_that_is_no_data_file_is_held() {
        let files = MemoryFiles::new().with_bytes("img/logo.png", &[0x89]).with_text("fancy.css", "p {}");
        let detail = |path: &str| files.read_value(path).unwrap_err().detail;
        assert_eq!(detail("img/logo.png"), "the file `img/logo.png` must be a data file, and it is held as bytes");
        assert_eq!(detail("fancy.css"), "the file `fancy.css` must be a data file, and it is held as a text");
    }
}
