//! The host service `files`: the files of the document's folder, read by an extension's
//! handlers at build time.
//!
//! An extension declares the host services its handlers call (the key `host` of a package,
//! [`flm_ext::host`]); the loader of every preparation offers this one under the name
//! `files` ([`packages::loader`](crate::packages::loader)). A handler reads a file with
//! `read_text` or `read_bytes` and the arguments `{ scope, path }`. Of the two scopes, the
//! extension's own folder (`extension`) is served by the extension's files themselves, in
//! `flm_ext`, and never reaches this object; [`FilesService`] serves the document's folder
//! (`document`):
//!
//! - the path is relative to the document's folder (the folder of the input file; the
//!   current folder for a text given as the input), written with `/`; an absolute path, or
//!   one that leaves the folder once `.` and `name/..` are removed ([`fold`]), is refused;
//! - a file that a symbolic link leads outside the folder is refused;
//! - nothing at the path answers `null`; any other failure answers its text;
//! - `read_text` answers the file's text, and refuses a file that is not UTF-8;
//!   `read_bytes` answers its bytes (to a Rust caller: the files a compose template ships).
//!
//! # Why the reads go through `ServiceFiles`
//!
//! The service is made once per preparation, and its handlers read during each build made
//! from it (a citation provider answering the cited keys, a template module rendering the
//! page). Each read goes through [`ServiceFiles::reader`], the file reader of the build under
//! way, as the standard citation provider reads a style file and the graphics resolver an
//! image: the file is recorded among that build's files read
//! ([`Built::files_read`](crate::session::Built::files_read)), so the live preview watches
//! it, and a change to it builds again over the same preparation.

use std::fmt;
use std::path::{Component, Path, PathBuf};

use flm_ext::host::{HostAnswer, HostCaller, HostService, HostServiceError, files};
use serde_json::Value;

use crate::files::{BuildFiles, FileErrorKind, ServiceFiles, fold};

/// The host service `files` over the document's folder, reading through the file reader of
/// the build under way. See the [module documentation](self).
#[derive(Clone)]
pub struct FilesService {
    /// The file reader of the registry's services: each read is recorded among the files
    /// the build under way read.
    pub files: ServiceFiles,
    /// The document's folder: the folder of the input file, the empty path (the current
    /// folder) for a text given as the input.
    pub document: PathBuf,
}

impl fmt::Debug for FilesService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilesService").field("document", &self.document).finish_non_exhaustive()
    }
}

impl FilesService {
    /// Returns the service reading the files of the folder `document` through `files`.
    pub fn new(files: ServiceFiles, document: PathBuf) -> FilesService {
        FilesService { files, document }
    }

    /// The document's folder as messages show it (`.` for the current folder).
    fn shown_folder(&self) -> String {
        let folder = fold(&self.document);
        if folder.as_os_str().is_empty() { String::from(".") } else { folder.display().to_string() }
    }

    /// The path `path`, relative to the document's folder, joined to it and folded, or the
    /// refusal of a path that is absolute or leaves the folder (H9).
    fn joined(&self, path: &str) -> Result<PathBuf, HostServiceError> {
        let bytes = path.as_bytes();
        let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
        if Path::new(path).is_absolute() || path.starts_with('/') || path.starts_with('\\') || drive {
            return Err(refused(format!("the path `{path}` must be relative to the document's folder")));
        }
        let folder = fold(&self.document);
        let full = fold(&folder.join(path));
        let inside = full
            .strip_prefix(&folder)
            .is_ok_and(|rest| !matches!(rest.components().next(), Some(Component::ParentDir)));
        if !inside {
            return Err(refused(format!(
                "the path `{path}` must stay inside the document's folder `{}`",
                self.shown_folder()
            )));
        }
        Ok(full)
    }

    /// The bytes of the file `path` of the document's folder, `None` when nothing is there.
    fn read(&self, path: &str) -> Result<Option<Vec<u8>>, HostServiceError> {
        let full = self.joined(path)?;
        let reader: BuildFiles = self.files.reader();
        let bytes = match reader.read_bytes(Path::new(""), &full) {
            Ok(bytes) => bytes,
            Err(error) if error.kind == FileErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(refused(error.to_string())),
        };
        match reader.stays_within(Path::new(""), &fold(&self.document), &full) {
            Ok(true) => Ok(Some(bytes)),
            Ok(false) => Err(refused(format!(
                "the file `{path}` leads outside the document's folder `{}` through a symbolic link",
                self.shown_folder()
            ))),
            Err(error) => Err(refused(error.to_string())),
        }
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

    /// `read_text` or `read_bytes` with `{ scope: "document", path }`; the caller is not
    /// read.
    fn call(&self, _caller: &HostCaller, method: &str, args: &Value) -> Result<HostAnswer, HostServiceError> {
        if method != files::READ_TEXT && method != files::READ_BYTES {
            return Err(refused(format!(
                "the method of the host service `files` must be `read_text` or `read_bytes`, not `{method}`"
            )));
        }
        let scope = args.get("scope").and_then(Value::as_str);
        let path = args.get("path").and_then(Value::as_str);
        let (Some(scope), Some(path)) = (scope, path) else {
            return Err(refused(String::from("the arguments must be `{ scope, path }`, both texts")));
        };
        match scope {
            files::DOCUMENT => {}
            files::EXTENSION => {
                return Err(refused(String::from(
                    "the host's `files` service must be called with the scope `document`; the scope `extension` \
                     is read from the extension's own files",
                )));
            }
            other => {
                return Err(refused(format!(
                    "the scope of a file read must be `extension` or `document`, not `{other}`"
                )));
            }
        }
        let Some(bytes) = self.read(path)? else { return Ok(HostAnswer::Data(Value::Null)) };
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
    use serde_json::json;

    use super::*;
    use crate::files::{Files, MemoryFiles};

    fn caller() -> HostCaller {
        HostCaller {
            extension: ExtensionIdentity { identity: String::from("x.yaml"), stem: String::from("x") },
            package: String::from("_"),
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
        (FilesService::new(service_files.clone(), PathBuf::from(document)), service_files)
    }

    fn call(service: &FilesService, method: &str, path: &str) -> Result<HostAnswer, HostServiceError> {
        service.call(&caller(), method, &json!({ "scope": "document", "path": path }))
    }

    #[test]
    fn a_file_of_the_document_folder_is_read_and_recorded() {
        let (service, service_files) = made("doc");
        let reads = BuildFiles::new(Arc::new(MemoryFiles::new().with("doc/bib.json", "{}")));
        let _attached = service_files.attach(reads.clone());
        assert_eq!(call(&service, "read_text", "bib.json").unwrap(), HostAnswer::Data(json!("{}")));
        assert_eq!(call(&service, "read_text", "./sub/../bib.json").unwrap(), HostAnswer::Data(json!("{}")));
        assert_eq!(call(&service, "read_bytes", "bib.json").unwrap(), HostAnswer::Bytes(b"{}".to_vec()));
        let read: Vec<PathBuf> = reads.list().into_iter().map(|read| read.path).collect();
        assert_eq!(read, vec![PathBuf::from("doc/bib.json"); 3]);
    }

    #[test]
    fn nothing_at_the_path_answers_null() {
        let (service, _) = made("doc");
        assert_eq!(call(&service, "read_text", "missing.json").unwrap(), HostAnswer::Data(Value::Null));
        assert_eq!(call(&service, "read_bytes", "missing.json").unwrap(), HostAnswer::Data(Value::Null));
    }

    #[test]
    fn a_path_leaving_the_folder_or_absolute_is_refused() {
        let (service, _) = made("doc");
        assert_eq!(
            call(&service, "read_text", "../other.txt").unwrap_err().detail,
            "the path `../other.txt` must stay inside the document's folder `doc`"
        );
        for path in ["/etc/passwd", "\\x", "C:/x"] {
            assert_eq!(
                call(&service, "read_text", path).unwrap_err().detail,
                format!("the path `{path}` must be relative to the document's folder")
            );
        }
        let (service, _) = made("");
        assert_eq!(call(&service, "read_text", "top.txt").unwrap(), HostAnswer::Data(json!("t")));
        assert_eq!(
            call(&service, "read_text", "../top.txt").unwrap_err().detail,
            "the path `../top.txt` must stay inside the document's folder `.`"
        );
    }

    #[test]
    fn a_file_that_is_not_utf8_is_refused_as_text() {
        let mut files = MemoryFiles::new();
        files.insert("doc/logo.png", vec![0xff_u8, 0xfe, 0x00]);
        let service = FilesService::new(ServiceFiles::new(Arc::new(files)), PathBuf::from("doc"));
        assert_eq!(call(&service, "read_text", "logo.png").unwrap_err().detail, "the file `logo.png` must be UTF-8 text");
        assert_eq!(call(&service, "read_bytes", "logo.png").unwrap(), HostAnswer::Bytes(vec![0xff, 0xfe, 0x00]));
    }

    #[test]
    fn a_scope_or_a_method_other_than_the_document_reads_is_refused() {
        let (service, _) = made("doc");
        let answer = service.call(&caller(), "read_text", &json!({ "scope": "elsewhere", "path": "bib.json" }));
        assert_eq!(answer.unwrap_err().detail, "the scope of a file read must be `extension` or `document`, not `elsewhere`");
        let answer = service.call(&caller(), "write", &json!({ "scope": "document", "path": "bib.json" }));
        assert_eq!(
            answer.unwrap_err().detail,
            "the method of the host service `files` must be `read_text` or `read_bytes`, not `write`"
        );
    }

    #[cfg(all(unix, feature = "fs"))]
    #[test]
    fn a_symbolic_link_leading_outside_the_folder_is_refused() {
        use crate::files::DiskFiles;
        let root = std::env::temp_dir().join(format!("flm-host-files-{}", std::process::id()));
        let document = root.join("doc");
        std::fs::create_dir_all(&document).unwrap();
        std::fs::write(root.join("secret.txt"), "s").unwrap();
        let link = document.join("link.txt");
        let _ = std::fs::remove_file(&link);
        std::os::unix::fs::symlink(root.join("secret.txt"), &link).unwrap();
        let service = FilesService::new(ServiceFiles::new(Arc::new(DiskFiles)), document.clone());
        let detail = call(&service, "read_text", "link.txt").unwrap_err().detail;
        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(
            detail,
            format!("the file `link.txt` leads outside the document's folder `{}` through a symbolic link", document.display())
        );
    }
}
