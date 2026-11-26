//! The host service `files`: the files at a qualified path, read by an extension's handlers
//! at build time.
//!
//! An extension declares the host services its handlers call (the key `host` of a package,
//! [`flm_ext::host`]); the loader of every preparation offers this one under the name
//! `files` ([`packages::loader`](crate::packages::loader)). A handler reads a file with
//! `read_text` or `read_bytes` and the argument `{ path }`, a path with a root
//! ([`flm_ext::paths`]). The roots `self`, the extension's own folder, and `ext+NAME`, the
//! folder of the extension that the use name `NAME` found, are served in `flm_ext`, from the
//! files of the extensions that the loader loaded, and never reach this object, nor do a
//! path with no root and an `https` address; it refuses these as `flm_ext` does, an `https`
//! address with the words that name the host service `download`
//! ([`crate::host_download`]). The roots that reach it are `std`, `doc` and `cwd`, which [`FilesService`] reads through the reader of qualified paths of the
//! preparation ([`Roots`]):
//!
//! - `std`: the program's own files;
//! - `doc`: the document's folder (the folder of the input file; the current folder for a
//!   text given as the input), the path joined to it as a path written in the document is
//!   ([`BuildFiles::resolve`]). Any path is read: an absolute path, `~`, a `..` that climbs
//!   above the folder, a symbolic link leading anywhere. The root reads what the document's
//!   own `\input` and images may read, the files the user running the program may read;
//! - `cwd`: the folder in which the program runs, read the same way;
//! - nothing at the path answers `null`; any other failure answers its text;
//! - `read_text` answers the file's text, and refuses a file that is not UTF-8;
//!   `read_bytes` answers its bytes (to a Rust caller: the files a compose template ships).
//!
//! # Why the reads go through `ServiceFiles`
//!
//! The service is made once per preparation, and its handlers read during each build made
//! from it (a citation provider answering the cited keys, a template module rendering the
//! page). Each read of the document's folder goes through [`ServiceFiles::reader`], the
//! file reader of the build under way, as the standard citation provider reads a style file
//! and the graphics resolver an image: the file is recorded among that build's files read
//! ([`Built::files_read`](crate::session::Built::files_read)), so the live preview watches
//! it, and a change to it builds again over the same preparation. [`Roots`] reads the files
//! of an extension found in a folder the same way.

use std::fmt;

use flm_core::progress::Scope;
use flm_ext::host::{HostAnswer, HostCaller, HostService, HostServiceError, files};
use flm_ext::paths::{Root, WrittenPath};
use serde_json::Value;

use crate::files::{BuildFiles, FileError, FileErrorKind, ServiceFiles};
use crate::roots::Roots;

/// The host service `files` over the roots of a preparation. See the
/// [module documentation](self).
#[derive(Clone)]
pub struct FilesService {
    /// The reader of qualified paths of the preparation.
    roots: Roots,
}

impl fmt::Debug for FilesService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilesService").field("roots", &self.roots).finish()
    }
}

impl FilesService {
    /// Returns the service reading the files at a qualified path through `roots`.
    pub fn new(roots: Roots) -> FilesService {
        FilesService { roots }
    }
}

/// Returns the bytes of the file at `path` of the folder `document`, or `None` when no
/// file is there.
///
/// The path is joined to the folder as a path written in the document is
/// ([`BuildFiles::resolve`]): it may be relative, absolute, `~`, or climb with `..`. The
/// file is read through `files`, the file reader of the build under way, so that the read
/// is recorded among that build's files read.
///
/// [`Roots`] reads the files of the roots `doc` and `cwd` with this function, `document`
/// being the document's folder for `doc`, and `""`, the current folder, for `cwd`. Read a qualified path with
/// [`Roots::read`], which applies the rules of each root.
///
/// # Errors
///
/// Returns the reader's [`FileError`] when the file is there and cannot be read.
pub fn read_document_file(files: &ServiceFiles, document: &str, path: &str) -> Result<Option<Vec<u8>>, FileError> {
    let reader: BuildFiles = files.reader();
    match reader.read_bytes(document, path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind == FileErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

/// A refusal with the text `detail`.
fn refused(detail: String) -> HostServiceError {
    HostServiceError { detail }
}

impl HostService for FilesService {
    /// `files`.
    fn name(&self) -> &str {
        files::NAME
    }

    /// `read_text` or `read_bytes` with `{ path }`, a qualified path other than `self`;
    /// neither the caller nor the scope is read: a read is not slow work.
    fn call(&self, _caller: &HostCaller, method: &str, args: &Value, _progress: &Scope) -> Result<HostAnswer, HostServiceError> {
        if method != files::READ_TEXT && method != files::READ_BYTES {
            return Err(refused(format!(
                "the method of the host service ‘files’ must be ‘read_text’ or ‘read_bytes’, not ‘{method}’"
            )));
        }
        let Some(path) = args.get("path").and_then(Value::as_str) else {
            return Err(refused(String::from("Invalid arguments of the host service ‘files’: the arguments must be ‘{ path }’, a text")));
        };
        let qualified = match WrittenPath::parse(path).map_err(|error| refused(error.to_string()))? {
            WrittenPath::Qualified(qualified) if *qualified.root() == Root::Https => {
                return Err(refused(format!(
                    "Unsupported path ‘{path}’ for the host service ‘files’: an ‘https’ address is read through the \
                     host service ‘download’"
                )));
            }
            WrittenPath::Qualified(qualified) => qualified,
            WrittenPath::Own(_) | WrittenPath::Bare(_) => {
                return Err(refused(format!(
                    "Invalid path ‘{path}’ for the host service ‘files’: the path must have a root other than ‘self’"
                )));
            }
        };
        let Some(bytes) = self.roots.read(&qualified).map_err(|error| refused(error.to_string()))?.map(|file| file.bytes)
        else {
            return Ok(HostAnswer::Data(Value::Null));
        };
        if method == files::READ_BYTES {
            return Ok(HostAnswer::Bytes(bytes));
        }
        String::from_utf8(bytes)
            .map(|text| HostAnswer::Data(Value::String(text)))
            .map_err(|_| refused(format!("the file `{path}` must be UTF-8 text")))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use flm_ext::engine::ExtensionIdentity;
    use flm_ext::paths::ExtensionsByName;
    use serde_json::json;

    use super::*;
    use crate::files::{Files, MemoryFiles};

    fn caller() -> HostCaller {
        HostCaller {
            extension: ExtensionIdentity { identity: String::from("x.yaml"), stem: String::from("x"), name: String::from("x") },
            package: String::from("_"),
            caches: Default::default(),
            digest_input: Arc::from(Vec::new()),
        }
    }

    fn made(document: &str) -> (FilesService, ServiceFiles) {
        let files: Arc<dyn Files> = Arc::new(
            MemoryFiles::new()
                .with("doc/bib.json", "{}")
                .with("doc/sub/a.txt", "a")
                .with("other.txt", "o")
                .with("top.txt", "t"),
        );
        let service_files = ServiceFiles::new(files);
        (service(service_files.clone(), document), service_files)
    }

    /// The service over the document's folder `document` read through `files`, with no
    /// extension and no file of `std`.
    fn service(files: ServiceFiles, document: &str) -> FilesService {
        let std = Arc::new(flm_ext::MemoryFiles::new());
        FilesService::new(Roots::new(ExtensionsByName::new(), std, files, String::from(document)))
    }

    fn call(service: &FilesService, method: &str, path: &str) -> Result<HostAnswer, HostServiceError> {
        service.call(&caller(), method, &json!({ "path": path }), &Scope::none())
    }

    #[test]
    fn a_file_of_the_document_folder_is_read_and_recorded() {
        let (service, service_files) = made("doc");
        let reads = BuildFiles::new(Arc::new(MemoryFiles::new().with("doc/bib.json", "{}")));
        let _attached = service_files.attach(reads.clone());
        assert_eq!(call(&service, "read_text", "doc:bib.json").unwrap(), HostAnswer::Data(json!("{}")));
        assert_eq!(call(&service, "read_text", "doc:./bib.json").unwrap(), HostAnswer::Data(json!("{}")));
        assert_eq!(call(&service, "read_bytes", "doc:bib.json").unwrap(), HostAnswer::Bytes(b"{}".to_vec()));
        let read: Vec<String> = reads.list().into_iter().map(|read| read.path).collect();
        assert_eq!(read, vec![String::from("doc/bib.json"); 3]);
    }

    #[test]
    fn nothing_at_the_path_answers_null() {
        let (service, _) = made("doc");
        assert_eq!(call(&service, "read_text", "doc:missing.json").unwrap(), HostAnswer::Data(Value::Null));
        assert_eq!(call(&service, "read_bytes", "doc:missing.json").unwrap(), HostAnswer::Data(Value::Null));
    }

    #[test]
    fn any_path_is_joined_to_the_document_folder_and_read() {
        let (service, service_files) = made("doc");
        let reads = BuildFiles::new(Arc::new(MemoryFiles::new().with("/top.txt", "t").with("~/home.txt", "h")));
        let _attached = service_files.attach(reads.clone());
        assert_eq!(call(&service, "read_text", "/top.txt").unwrap(), HostAnswer::Data(json!("t")));
        assert_eq!(call(&service, "read_text", "~/home.txt").unwrap(), HostAnswer::Data(json!("h")));
        assert_eq!(call(&service, "read_text", "doc:../other.txt").unwrap(), HostAnswer::Data(Value::Null));
        let read: Vec<String> = reads.list().into_iter().map(|read| read.path).collect();
        assert_eq!(read, ["/top.txt", "~/home.txt", "doc/../other.txt"]);
        let (service, _) = made("");
        assert_eq!(call(&service, "read_text", "doc:top.txt").unwrap(), HostAnswer::Data(json!("t")));
    }

    #[test]
    fn a_file_that_is_not_utf8_is_refused_as_text() {
        let mut files = MemoryFiles::new();
        files.insert("doc/logo.png", vec![0xff_u8, 0xfe, 0x00]);
        let service = service(ServiceFiles::new(Arc::new(files)), "doc");
        assert!(call(&service, "read_text", "doc:logo.png").is_err());
        assert_eq!(call(&service, "read_bytes", "doc:logo.png").unwrap(), HostAnswer::Bytes(vec![0xff, 0xfe, 0x00]));
    }

    #[test]
    fn a_path_with_no_root_or_a_method_other_than_the_reads_is_refused() {
        let (service, _) = made("doc");
        assert!(call(&service, "read_text", "bib.json").is_err());
        assert!(call(&service, "read_text", "self:bib.json").is_err());
        assert!(call(&service, "write", "doc:bib.json").is_err());
    }

    #[cfg(all(unix, feature = "fs"))]
    #[test]
    fn a_file_outside_the_folder_is_read_through_a_symbolic_link_or_a_climb() {
        use crate::files::DiskFiles;
        let root = std::env::temp_dir().join(format!("flm-host-files-{}", std::process::id()));
        let document = root.join("doc");
        std::fs::create_dir_all(&document).unwrap();
        std::fs::write(root.join("secret.txt"), "s").unwrap();
        let link = document.join("link.txt");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(root.join("secret.txt"), &link).unwrap();
        let folder = crate::files::system::from_system(&document).unwrap();
        let service = service(ServiceFiles::new(Arc::new(DiskFiles)), &folder);
        let linked = call(&service, "read_text", "doc:link.txt");
        let climbed = call(&service, "read_text", "doc:../secret.txt");
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(linked.unwrap(), HostAnswer::Data(json!("s")));
        assert_eq!(climbed.unwrap(), HostAnswer::Data(json!("s")));
    }
}
