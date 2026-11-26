//! The reader of qualified paths of one preparation, [`Roots`]: the bytes at a path with a
//! root ([`flm_ext::paths`]), read in the folder its root names.
//!
//! - `ext+NAME`: the files of the extension that the use name `NAME` found, as the loader of
//!   the preparation registered them ([`ExtensionsByName`]). A name that no `use` entry in
//!   effect writes, of the document or of a package it uses, is refused.
//! - `std`: the program's own files, which the host gives (the files of the package `std`,
//!   each path beginning with `templates/`).
//! - `doc`: the document's folder, read through the file reader of the build under way as
//!   a path written in the document is ([`read_document_file`]).
//! - `cwd`: the folder in which the program runs, read the same way.
//! - `https`: an address on the network, `https:` and the address after it
//!   (`https://example.org/a.css`), read through the program's download
//!   ([`Downloads::get`]) and kept in the program's cache. The read answers, beside the
//!   bytes, the media type that the server states and the warning of a kept copy used past
//!   its hard expiry ([`RootsFile`]). A program given no downloader refuses it
//!   ([`DownloadErrorKind::NoNetwork`](crate::download::DownloadErrorKind::NoNetwork)).
//!
//! The root `self` never reaches the reader: the caller gives the path at the place of its
//! text ([`WrittenPath::at`](flm_ext::paths::WrittenPath::at)), which replaces `self` by the
//! root of the place.
//!
//! A read is recorded as any read of the same file is: a file of the document's folder, and
//! a file of an extension found in a folder of the search path (whose shared files read
//! through the build under way), are among the files that the build read
//! ([`Built::files_read`](crate::session::Built::files_read)). The files of `std` and of an
//! extension that the program carries never change, and no read of one is recorded.

use std::fmt;

use flm_ext::paths::{ExtensionsByName, PathError, PathErrorKind, QualifiedPath, Root, SharedFiles};
use flm_ext::source::FilesError;

use crate::cache::ServiceCaches;
use crate::download::{DownloadError, Downloads};
use crate::files::{FileError, ServiceFiles};
use crate::host_files::read_document_file;

/// The reader of qualified paths of one preparation. See the [module documentation](self).
/// It is made when the preparation starts and shared, as the file reader of the registry's
/// services is: a clone reads the same roots.
#[derive(Clone)]
pub struct Roots {
    /// The files of the extensions loaded, by use name.
    extensions: ExtensionsByName,
    /// The program's own files.
    std: SharedFiles,
    /// The file reader of the registry's services.
    files: ServiceFiles,
    /// The document's folder: the folder of the input file, the empty path (the current
    /// folder) for a text given as the input.
    document: String,
    /// The program's downloads, for the root `https`.
    downloads: Downloads,
    /// The handle of the registry's caches, through which a download opens its cache.
    caches: ServiceCaches,
}

impl Roots {
    /// Returns the reader of the extensions of `extensions`, of the program's own files `std`,
    /// and of the document's folder `document` and the current folder through `files`. It
    /// has no downloader: it refuses the root `https` ([`with_downloads`](Self::with_downloads)).
    pub fn new(extensions: ExtensionsByName, std: SharedFiles, files: ServiceFiles, document: String) -> Roots {
        Roots { extensions, std, files, document, downloads: Downloads::default(), caches: ServiceCaches::default() }
    }

    /// Returns this reader, reading the root `https` through `downloads`, which keeps the
    /// files in the caches of `caches`.
    pub fn with_downloads(self, downloads: Downloads, caches: ServiceCaches) -> Roots {
        Roots { downloads, caches, ..self }
    }

    /// The table of the extensions loaded, by use name, which the loader of the preparation
    /// fills ([`ExtensionLoader::with_extensions_by_name`](flm_ext::ExtensionLoader::with_extensions_by_name)).
    pub fn extensions(&self) -> &ExtensionsByName {
        &self.extensions
    }

    /// Returns the file at `path` ([`RootsFile`]), `None` when nothing is there.
    ///
    /// # Errors
    ///
    /// [`RootsErrorKind::Path`] for a root `ext+NAME` whose name no `use` entry in effect
    /// writes ([`PathErrorKind::UnknownExtension`]); [`RootsErrorKind::Extension`] and
    /// [`RootsErrorKind::File`] for a file that is there and cannot be read;
    /// [`RootsErrorKind::Download`] for an address of the root `https` that gives no file.
    pub fn read(&self, path: &QualifiedPath) -> Result<Option<RootsFile>, RootsError> {
        let error = |kind| RootsError { path: path.to_string(), kind };
        let bytes = match path.root() {
            Root::Ext(name) => {
                let files = self.extensions.files(name).ok_or_else(|| {
                    error(RootsErrorKind::Path(PathError {
                        path: path.to_string(),
                        kind: PathErrorKind::UnknownExtension(name.clone()),
                    }))
                })?;
                files.read_bytes("", path.path()).map_err(|failed| error(RootsErrorKind::Extension(Box::new(failed))))
            }
            Root::Std => {
                self.std.read_bytes("", path.path()).map_err(|failed| error(RootsErrorKind::Extension(Box::new(failed))))
            }
            Root::Doc => read_document_file(&self.files, &self.document, path.path())
                .map_err(|failed| error(RootsErrorKind::File(Box::new(failed)))),
            Root::Cwd => {
                read_document_file(&self.files, "", path.path())
                    .map_err(|failed| error(RootsErrorKind::File(Box::new(failed))))
            }
            Root::Https => {
                let downloaded = self
                    .downloads
                    .get(&self.caches, &path.to_string())
                    .map_err(|failed| error(RootsErrorKind::Download(failed)))?;
                return Ok(Some(RootsFile {
                    bytes: downloaded.bytes,
                    media_type: downloaded.media_type,
                    warning: downloaded.warning,
                }));
            }
        }?;
        Ok(bytes.map(|bytes| RootsFile { bytes, media_type: None, warning: None }))
    }
}

/// A file that [`Roots::read`] read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootsFile {
    /// The file's bytes.
    pub bytes: Vec<u8>,
    /// The media type that the server stated, for the root `https`; `None` when it stated
    /// none, and for the roots `ext+NAME`, `std`, `doc` and `cwd`.
    pub media_type: Option<String>,
    /// For the root `https`, the warning of a kept copy used past its hard expiry, which
    /// could not be downloaded again ([`Downloaded::warning`](crate::download::Downloaded::warning));
    /// `None` otherwise. A caller reports it, or refuses the root `https` before it calls
    /// [`Roots::read`].
    pub warning: Option<String>,
}

impl fmt::Debug for Roots {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Roots")
            .field("extensions", &self.extensions)
            .field("document", &self.document)
            .finish_non_exhaustive()
    }
}

/// A qualified path that [`Roots::read`] did not read: the path, written `ROOT:PATH`, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RootsError {
    /// The path, written `ROOT:PATH`.
    pub path: String,
    /// Why it was not read.
    pub kind: RootsErrorKind,
}

/// Why a qualified path was not read ([`RootsError`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootsErrorKind {
    /// The path has no meaning in this preparation: no `use` entry in effect writes the use
    /// name of the root `ext+NAME` ([`PathErrorKind::UnknownExtension`]).
    Path(PathError),
    /// The file of an extension or of `std` is there and cannot be read.
    Extension(Box<FilesError>),
    /// The file of the document's folder or of the current folder is there and cannot be
    /// read.
    File(Box<FileError>),
    /// The path is an address on the network that gives no file: the program gave no
    /// downloader, the address does not begin with `https://`, or the request failed with
    /// no kept value.
    Download(DownloadError),
}

impl fmt::Display for RootsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = &self.path;
        match &self.kind {
            RootsErrorKind::Path(error) => write!(f, "{error}"),
            RootsErrorKind::Extension(error) => write!(f, "Failed to read ‘{path}’: {error}"),
            RootsErrorKind::File(error) => write!(f, "Failed to read ‘{path}’: {error}"),
            RootsErrorKind::Download(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for RootsError {}
