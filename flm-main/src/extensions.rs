//! The filesystem source of extensions: finding an extension by its locator in the folders
//! of the search path, and reading its files.
//!
//! An *extension* is what a document's `use` entry finds outside the program: a manifest,
//! and, for an extension written as a folder, the files beside it. The library's loader
//! ([`flm_ext::loader::ExtensionLoader`]) turns it into packages; this module gives the
//! loader the extension, as an [`ExtensionSource`] ([`ExtensionFinder`]), and its files, as
//! [`ExtensionFiles`].
//!
//! # Where an extension is found
//!
//! The search path is an ordered list of folders ([`ExtensionFolder`], given by
//! [`Request::extension_folders`](crate::session::Request::extension_folders)). An entry's
//! locator (`myext`, `@acme/style`) is looked for in each folder in order, in two layouts:
//! the file `LOCATOR.yaml`, or the folder `LOCATOR/` holding the manifest
//! `flmextension.yaml` ([`MANIFEST_FILE`]). The first folder that holds either gives the
//! extension; a folder holding both is a problem. A nested locator is joined to each folder
//! (`_flmextensions/@acme/style.yaml`).
//!
//! # What is installed
//!
//! [`ExtensionFinder::locators`] lists the extensions installed in the folders of the search
//! path, without loading any: the locators of the two layouts directly in a folder
//! (`style.yaml`, `style/flmextension.yaml`) and one level under a folder whose name begins
//! with `@` (`@acme/style`). These are the values that the JSON Schemas of a front matter
//! and of a manifest suggest for an entry of `use` ([`crate::packages::suggestions`]).
//!
//! An extension is identified by the path of its manifest, normalized ([`fold`]: `.` and
//! `name/..` removed, links not followed), so that the spelling of the folder leading to it
//! makes no difference; that path names the extension's packages (`defs/myext.yaml#_`).
//!
//! # The manifest
//!
//! The manifest is YAML; its root is a map of these two keys, each optional:
//!
//! | Key | What it holds |
//! |---|---|
//! | `extension` | what the extension is: `description` (a text, the title of the compose templates it ships), `author` (a text), `flm_version` (the versions of FLM it works with, as a requirement such as `>=0.4`), `engine` (`data`, the default, `js` or `wasm`), `js` (for the engine `js`: `main`, the path of its main module, and `limits`), `wasm` (for the engine `wasm`: `module`, the path of its module, and `limits`) |
//! | `flm` | for the engine `data`: the package's contributions, as in a front matter: `use` (the packages it uses), `features` (a map from a feature's name to its settings, or to `false`), `parsing`, `workflows` (a map from a workflow's name to its fields: `extends`, `outputs`, `demands`, `prefer`, `settings` and `docs`), and `feature_order` (the chains of the order of features) |
//!
//! An extension written as a folder may ship compose templates, laid out as those of
//! `stdmain` are: in `templates/OUTPUT/`, each template either the file `NAME.yaml`, a
//! description alone, or the folder `NAME/` holding the description `flmtemplate.yaml` and
//! the files that are the template's own, its text among them ([`flm_ext::templates`]).
//!
//! # The extension's files
//!
//! The files of an extension written as a folder are read by a path relative to its
//! folder, through [`BuildFiles`], so that each appears on the list of the files a build
//! read, found or not. A file or a folder that a symbolic link leads outside the
//! extension's folder is refused ([`Files::stays_within`](crate::files::Files::stays_within));
//! the extension's folder may itself be a link. An extension written as one file has no
//! files. A data file (a template's description) is read as the manifest is. The files of
//! an extension compiled into the program, and those of `stdmain`'s compose templates, are
//! read in the same way from a static table of paths and bytes ([`StaticFiles`]).
//!
//! The files a compose template includes are read when the template is applied, during a
//! build, not when the extension is loaded: the template's handle on the extension's files
//! ([`ExtensionFiles::shared`]) reads through the [`ServiceFiles`] of the registry, so
//! that each such read is recorded among the files *that build* read
//! ([`Built::files_read`](crate::session::Built::files_read)), as the reads of a registry's
//! services are.
//!
//! The finder keeps each manifest it parsed, with the place of every value
//! ([`ExtensionFinder::manifest`]), so that a problem with a value an extension writes is
//! placed at its line in the manifest. It keeps them in a store ([`ParsedManifests`]) that
//! the host's other sources share (the extensions compiled into the program, under the
//! cargo feature `bundled-extensions`), so that the manifests they parse are placed the
//! same way. Each source gives the loader the resolver of its manifest's places
//! ([`FoundExtension::place_of`]), through which a problem the loader reports by key path
//! is placed at its line, and a manifest that is not valid YAML carries the place of its
//! problem in the error ([`SourceError::place`]).

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use flm_ext::Locator;
use flm_ext::source::{ExtensionFiles, ExtensionSource, FilesError, FoundExtension, PlaceOf, SourceError};
use serde_json::Value;

use crate::files::{BuildFiles, FileErrorKind, ServiceFiles, fold};
use crate::packages::{ExtensionFolder, PackageProblem};
use crate::seed::{Node, NodeValue, SeedError, read_front_matter};

/// The name of the manifest of an extension written as a folder: `flmextension.yaml`.
pub const MANIFEST_FILE: &str = "flmextension.yaml";

/// The filesystem source: finds an extension by its locator in the folders of the search
/// path, in order, in either layout, and reads every file through [`BuildFiles`]. See the
/// [module documentation](self).
pub struct ExtensionFinder {
    files: BuildFiles,
    /// The file reader of the build under way, for the files a compose template includes.
    service: ServiceFiles,
    folders: Vec<ExtensionFolder>,
    /// The manifest of every extension found, parsed, by identity, shared with the host's
    /// other sources.
    manifests: Arc<ParsedManifests>,
}

impl fmt::Debug for ExtensionFinder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtensionFinder").field("folders", &self.folders).finish_non_exhaustive()
    }
}

impl ExtensionFinder {
    /// Returns a finder that looks in the folders `folders`, in order, and reads through
    /// `files`; the files a compose template of an extension includes are read through
    /// `service`, the file reader of the build under way when the template is applied.
    pub fn new(files: BuildFiles, service: ServiceFiles, folders: Vec<ExtensionFolder>) -> ExtensionFinder {
        ExtensionFinder {
            files,
            service,
            folders,
            manifests: Arc::new(ParsedManifests::default()),
        }
    }

    /// Returns a problem for each folder of the search path that must exist and does not
    /// (or cannot be listed).
    pub fn missing_folders(&self) -> Vec<PackageProblem> {
        self.folders
            .iter()
            .filter(|folder| folder.required)
            .filter_map(|folder| match self.files.entries(Path::new(""), &folder.path) {
                Err(error) => Some(PackageProblem {
                    name: String::new(),
                    file: None,
                    place: None,
                    detail: match error.kind {
                        FileErrorKind::NotFound => {
                            format!("the folder `{}` of the search path must exist", folder.path.display())
                        }
                        _ => error.to_string(),
                    },
                }),
                Ok(_) => None,
            })
            .collect()
    }

    /// Returns the folders of the search path, in order.
    pub fn folders(&self) -> &[ExtensionFolder] {
        &self.folders
    }

    /// Returns the locators of the extensions installed in the folders of the search path,
    /// without reading any manifest: for each folder, in order, and each of its entries, in
    /// name order, a file `NAME.yaml` gives `NAME`, a folder `NAME` that holds the manifest
    /// [`MANIFEST_FILE`] gives `NAME`, and a folder `@SCOPE` gives `@SCOPE/NAME` for each of
    /// its own entries that one of the first two rules admits; `NAME` and `SCOPE` are
    /// segments of a locator (letters, digits, `-` and `_`). Each locator comes once, where
    /// it is first found.
    ///
    /// Every other entry is skipped: a file of another kind, a folder without the manifest,
    /// a name that is not a segment, an entry deeper than `@SCOPE/NAME`. A folder that cannot
    /// be listed contributes nothing, and the listing reports no problem. Whether an entry is
    /// a file or a folder is told by listing it, a file being what cannot be listed for that
    /// reason ([`FileErrorKind::NotAFolder`]).
    ///
    /// The folders are listed through a reader of their own ([`BuildFiles::unrecorded`]), so
    /// the listing is not among the paths read: the packages a document uses do not depend on
    /// it, and a change to a folder of the search path that touches no extension in use does
    /// not make a preview prepare the document again.
    pub fn locators(&self) -> Vec<String> {
        // See decision rationale at [§flm-dd-dr:schema-suggestions]
        let files = self.files.unrecorded();
        let listed = |path: &Path| files.entries(Path::new(""), path).unwrap_or_default();
        let mut locators: Vec<String> = Vec::new();
        let mut add = |locator: String| {
            if !locators.contains(&locator) {
                locators.push(locator);
            }
        };
        // The stem of the extension that the entry `name` of the folder `folder` holds in one
        // of the two layouts: `NAME` for a file `NAME.yaml` (what cannot be listed for being a
        // file) and for a folder `NAME` holding the manifest; `None` for any other entry.
        let installed = |folder: &Path, name: &str| {
            let path = fold(&folder.join(name));
            if let Some(stem) = name.strip_suffix(".yaml") {
                let file = matches!(files.entries(Path::new(""), &path), Err(error) if error.kind == FileErrorKind::NotAFolder);
                return (is_segment(stem) && file).then(|| stem.to_string());
            }
            let manifest = is_segment(name) && listed(&path).iter().any(|entry| entry == MANIFEST_FILE);
            manifest.then(|| name.to_string())
        };
        for folder in &self.folders {
            for name in listed(&folder.path) {
                match name.strip_prefix('@') {
                    Some(scope) if is_segment(scope) => {
                        let scoped = fold(&folder.path.join(&name));
                        for inner in listed(&scoped) {
                            if let Some(stem) = installed(&scoped, &inner) {
                                add(format!("{name}/{stem}"));
                            }
                        }
                    }
                    Some(_) => {}
                    None => {
                        if let Some(stem) = installed(&folder.path, &name) {
                            add(stem);
                        }
                    }
                }
            }
        }
        locators
    }

    /// Returns the parsed manifest of the extension `identity` (for this finder, the path of
    /// its manifest), if this finder or a source sharing its [`ParsedManifests`] found it:
    /// every value with its place.
    pub fn manifest(&self, identity: &str) -> Option<Node> {
        self.manifests.manifest(identity)
    }

    /// Returns the store of the manifests this finder parsed, which the host's other
    /// sources share ([`ParsedManifests::parse`]), so that the problems with the values of
    /// any extension's manifest are placed through the finder.
    pub fn parsed_manifests(&self) -> &Arc<ParsedManifests> {
        &self.manifests
    }

    /// The extension of the manifest at `path`, whose text is `text`, found for `locator`
    /// whose stem is `stem`; `in_folder` for the folder layout.
    fn found(
        &self,
        locator: &str,
        stem: &str,
        path: &Path,
        text: &str,
        in_folder: bool,
    ) -> Result<FoundExtension, SourceError> {
        let identity = path.display().to_string();
        let (manifest, place_of) = self.manifests.parse(locator, stem, &identity, text)?;
        let files: Box<dyn ExtensionFiles> = if in_folder {
            Box::new(FolderFiles {
                files: Reading::Load(self.files.clone()),
                service: self.service.clone(),
                root: path.parent().map(Path::to_path_buf).unwrap_or_default(),
                stem: stem.to_string(),
            })
        } else {
            Box::new(NoFiles)
        };
        Ok(FoundExtension { manifest, identity, files, place_of: Some(place_of) })
    }
}

impl ExtensionSource for ExtensionFinder {
    /// The extension `locator` names in the first folder of the search path that holds it,
    /// in either layout; `Ok(None)` when no folder does, and for `.`.
    ///
    /// # Errors
    ///
    /// A manifest that cannot be read or is not valid YAML, and a folder that holds the
    /// extension in both layouts.
    fn find(&self, locator: &Locator) -> Result<Option<FoundExtension>, SourceError> {
        let Locator::Named(written) = locator else { return Ok(None) };
        let stem = locator.stem().unwrap_or(written);
        for folder in &self.folders {
            let layouts = [
                (fold(&folder.path.join(format!("{written}.yaml"))), false),
                (fold(&folder.path.join(written).join(MANIFEST_FILE)), true),
            ];
            let mut found = Vec::new();
            for (path, in_folder) in layouts {
                match self.files.read_text(Path::new(""), &path) {
                    Ok(text) => found.push((path, text, in_folder)),
                    Err(error) if error.kind == FileErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(SourceError {
                            locator: written.clone(),
                            path: Some(path.display().to_string()),
                            place: None,
                            detail: error.to_string(),
                        });
                    }
                }
            }
            match found.as_slice() {
                [] => continue,
                [(path, text, in_folder)] => return self.found(written, stem, path, text, *in_folder).map(Some),
                _ => {
                    return Err(SourceError {
                        locator: written.clone(),
                        path: None,
                        place: None,
                        detail: format!(
                            "the folder `{}` must hold the extension `{stem}` as either `{written}.yaml` or \
                             `{written}/{MANIFEST_FILE}`, not both",
                            folder.path.display()
                        ),
                    });
                }
            }
        }
        Ok(None)
    }
}

/// The manifests the host's sources parsed, each kept with the place of every value, by
/// the extension's identity: the YAML step that the filesystem source ([`ExtensionFinder`])
/// and the bundled source share, so that a problem with a value an extension writes is
/// placed at its line in the manifest whichever source found it.
#[derive(Default)]
pub struct ParsedManifests {
    /// The manifest of every extension found, parsed, by identity: for the places of the
    /// values of a data extension's block and of its `use` entries.
    manifests: Mutex<BTreeMap<String, Node>>,
}

impl fmt::Debug for ParsedManifests {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ParsedManifests").finish_non_exhaustive()
    }
}

impl ParsedManifests {
    /// Parses `text`, the manifest of the extension `identity` found for the locator
    /// `locator` whose stem is `stem`, as YAML, keeps the parsed node by `identity`, and
    /// returns its value with the resolver of the places of its values ([`PlaceOf`]),
    /// which the source gives in its [`FoundExtension`].
    ///
    /// # Errors
    ///
    /// A manifest that is not valid YAML, the place of its problem in the error
    /// ([`SourceError::place`]).
    pub fn parse(&self, locator: &str, stem: &str, identity: &str, text: &str) -> Result<(Value, PlaceOf), SourceError> {
        let node = read_front_matter(text, 1).map_err(|error| SourceError {
            locator: locator.to_string(),
            path: Some(identity.to_string()),
            place: error.place.map(|place| flm_ext::source::Place { line: place.line, column: place.column }),
            detail: format!(
                "the manifest `{identity}` of the extension `{stem}` must be valid YAML: {}",
                yaml_detail(&error)
            ),
        })?;
        let manifest = node.to_json();
        let kept = Arc::new(node.clone());
        let place_of: PlaceOf = Arc::new(move |path: &str| place_at(&kept, path));
        self.manifests.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).insert(identity.to_string(), node);
        Ok((manifest, place_of))
    }

    /// Returns the parsed manifest of the extension `identity`, if it was parsed: every
    /// value with its place.
    pub fn manifest(&self, identity: &str) -> Option<Node> {
        self.manifests.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).get(identity).cloned()
    }
}

/// The value of the data file `path` of the extension `stem`, whose text is `text`, parsed
/// as the manifest is; `shown` is the file as the messages show it.
pub(crate) fn data_value(text: &str, path: &str, shown: &str, stem: &str) -> Result<Value, FilesError> {
    read_front_matter(text, 1).map(|node| node.to_json()).map_err(|error| {
        let at = error.place.map(|place| format!(" (line {}, column {})", place.line, place.column)).unwrap_or_default();
        FilesError {
            path: path.to_string(),
            detail: format!("the file `{shown}` of the extension `{stem}` must be valid YAML: {}{at}", yaml_detail(&error)),
        }
    })
}

/// Whether `text` is a segment of a locator, as the grammar of a `use` entry has it
/// ([`flm_ext::entry`]): one or more letters, digits, `-` and `_`.
fn is_segment(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// The place of the value at the key path `path` of the manifest `root`: `KEY(.KEY)*`, each
/// `KEY` a map's key followed by any number of `[N]`, the item at the index `N` of the list
/// it holds ([`PlaceOf`]); `None` when no value is at the path or the path is not of that
/// form.
fn place_at(root: &Node, path: &str) -> Option<flm_ext::source::Place> {
    let mut node = root;
    for part in path.split('.') {
        let (key, mut indices) = part.find('[').map_or((part, ""), |at| part.split_at(at));
        node = node.get(key)?;
        while let Some(rest) = indices.strip_prefix('[') {
            let (index, after) = rest.split_once(']')?;
            let index: usize = index.parse().ok()?;
            node = match &node.value {
                NodeValue::List(items) => items.get(index)?,
                _ => return None,
            };
            indices = after;
        }
        if !indices.is_empty() {
            return None;
        }
    }
    Some(flm_ext::source::Place { line: node.place.line, column: node.place.column })
}

/// The detail of a YAML problem, without the reader's own opening ("the text must be valid
/// YAML: "), which the messages of this module word themselves.
fn yaml_detail(error: &SeedError) -> &str {
    error.detail.strip_prefix("the text must be valid YAML: ").unwrap_or(&error.detail)
}

/// Where the reads of a [`FolderFiles`] are recorded.
#[derive(Clone)]
enum Reading {
    /// In the record of the work that loads the extension (*prepare*'s).
    Load(BuildFiles),
    /// In the record of the build under way at each read (the files a compose template
    /// includes, read when the template is applied).
    Build(ServiceFiles),
}

impl Reading {
    /// The file reader of the next read.
    fn reader(&self) -> BuildFiles {
        match self {
            Reading::Load(files) => files.clone(),
            Reading::Build(service) => service.reader(),
        }
    }
}

/// The files of an extension written as a folder: paths relative to its folder, every read
/// and listing through [`BuildFiles`] (so that each is listed among the files read, found
/// or not), and refused when a symbolic link leads outside the folder. The handle given by
/// [`shared`](ExtensionFiles::shared), through which a compose template's includes are read
/// when the template is applied, reads through the build under way ([`ServiceFiles`]), so
/// that those reads are listed among that build's files read.
#[derive(Clone)]
struct FolderFiles {
    files: Reading,
    /// The file reader of the build under way, for [`shared`](ExtensionFiles::shared).
    service: ServiceFiles,
    /// The extension's folder.
    root: PathBuf,
    /// The extension's stem, for the messages.
    stem: String,
}

impl FolderFiles {
    /// The path `path`, relative to the extension's folder, joined to it.
    fn joined(&self, path: &str) -> PathBuf {
        if path.is_empty() { self.root.clone() } else { self.root.join(path) }
    }

    /// The problem `detail` with the file `path`.
    fn error(path: &str, detail: String) -> FilesError {
        FilesError { path: path.to_string(), detail }
    }

    /// Checks that `full`, the path `path` joined to the extension's folder, which exists,
    /// lies inside the folder once every link is followed; `what` names it in the message
    /// (`file` or `folder`).
    fn check(&self, files: &BuildFiles, what: &str, path: &str, full: &Path) -> Result<(), FilesError> {
        match files.stays_within(Path::new(""), &self.root, full) {
            Ok(true) => Ok(()),
            Ok(false) => Err(FolderFiles::error(
                path,
                format!(
                    "the {what} `{}` of the extension `{}` leads outside the extension's folder `{}` through a \
                     symbolic link",
                    full.display(),
                    self.stem,
                    self.root.display()
                ),
            )),
            Err(error) => Err(FolderFiles::error(path, error.to_string())),
        }
    }
}

impl ExtensionFiles for FolderFiles {
    fn read_text(&self, path: &str) -> Result<Option<String>, FilesError> {
        let full = self.joined(path);
        let files = self.files.reader();
        match files.read_text(Path::new(""), &full) {
            Ok(text) => self.check(&files, "file", path, &full).map(|()| Some(text)),
            Err(error) if error.kind == FileErrorKind::NotFound => Ok(None),
            Err(error) => Err(FolderFiles::error(path, error.to_string())),
        }
    }

    /// The file's bytes, through [`BuildFiles::read_bytes`], with the same check of
    /// symbolic links as [`read_text`](ExtensionFiles::read_text).
    fn read_bytes(&self, path: &str) -> Result<Option<Vec<u8>>, FilesError> {
        let full = self.joined(path);
        let files = self.files.reader();
        match files.read_bytes(Path::new(""), &full) {
            Ok(bytes) => self.check(&files, "file", path, &full).map(|()| Some(bytes)),
            Err(error) if error.kind == FileErrorKind::NotFound => Ok(None),
            Err(error) => Err(FolderFiles::error(path, error.to_string())),
        }
    }

    /// The file read as text, then as YAML, as the manifest is.
    fn read_value(&self, path: &str) -> Result<Option<Value>, FilesError> {
        let Some(text) = self.read_text(path)? else { return Ok(None) };
        data_value(&text, path, &self.joined(path).display().to_string(), &self.stem).map(Some)
    }

    fn list(&self, path: &str) -> Result<Option<Vec<String>>, FilesError> {
        let full = self.joined(path);
        let files = self.files.reader();
        match files.entries(Path::new(""), &full) {
            Ok(names) => self.check(&files, "folder", path, &full).map(|()| Some(names)),
            Err(error) if matches!(error.kind, FileErrorKind::NotFound | FileErrorKind::NotAFolder) => Ok(None),
            Err(error) => Err(FolderFiles::error(path, error.to_string())),
        }
    }

    /// The same files, read through the file reader of the build under way at each read
    /// ([`ServiceFiles::reader`]), so that a read made while a compose template is applied
    /// is recorded among that build's files read.
    fn shared(&self) -> Arc<dyn ExtensionFiles + Send + Sync> {
        Arc::new(FolderFiles { files: Reading::Build(self.service.clone()), ..self.clone() })
    }
}

/// The files of an extension that has none: one written as one file, or one written in Rust
/// and compiled into the program ([`crate::bundled`]). Nothing is there.
pub(crate) struct NoFiles;

impl ExtensionFiles for NoFiles {
    fn read_text(&self, _: &str) -> Result<Option<String>, FilesError> {
        Ok(None)
    }

    fn read_bytes(&self, _: &str) -> Result<Option<Vec<u8>>, FilesError> {
        Ok(None)
    }

    fn read_value(&self, _: &str) -> Result<Option<Value>, FilesError> {
        Ok(None)
    }

    fn list(&self, _: &str) -> Result<Option<Vec<String>>, FilesError> {
        Ok(None)
    }

    fn shared(&self) -> Arc<dyn ExtensionFiles + Send + Sync> {
        Arc::new(NoFiles)
    }
}

/// The files of an extension compiled into the program, read from a static table of paths
/// and bytes, each path relative to the extension's folder with `/` between the names: the
/// files of a bundled extension (the module `bundled`, under the cargo feature
/// `bundled-extensions`) and those of the compose templates of the package `stdmain`
/// ([`crate::stdpkg::compose_standard::templates`]). A file of the table never changes: no
/// read of one is recorded among the files a build read.
#[derive(Debug, Clone)]
pub struct StaticFiles {
    /// The files, each a path and its bytes; the files under one folder stand together.
    files: Vec<(&'static str, &'static [u8])>,
    /// What the messages show before a file's path (`bundled:data/lecturenotes`, `stdmain`).
    identity: String,
    /// The extension's stem, for the messages.
    stem: String,
}

impl StaticFiles {
    /// Returns the files `files`, each a path and its bytes, of the extension `stem`, the
    /// messages showing each file's path after `identity`. The files under one folder must
    /// stand together in `files`, as they do in path order.
    pub(crate) fn new(
        files: impl IntoIterator<Item = (&'static str, &'static [u8])>,
        identity: &str,
        stem: &str,
    ) -> StaticFiles {
        StaticFiles { files: files.into_iter().collect(), identity: identity.to_string(), stem: stem.to_string() }
    }

    /// The bytes of the file at `path`.
    fn file(&self, path: &str) -> Option<&'static [u8]> {
        self.files.iter().find(|(at, _)| *at == path).map(|(_, bytes)| *bytes)
    }

    /// The file `path` as the messages show it.
    fn shown(&self, path: &str) -> String {
        format!("{}/{path}", self.identity)
    }
}

impl ExtensionFiles for StaticFiles {
    /// The file's bytes as text.
    fn read_text(&self, path: &str) -> Result<Option<String>, FilesError> {
        let Some(bytes) = self.file(path) else { return Ok(None) };
        match std::str::from_utf8(bytes) {
            Ok(text) => Ok(Some(text.to_string())),
            Err(_) => Err(FilesError {
                path: path.to_string(),
                detail: format!("the file `{}` must be text encoded as UTF-8", self.shown(path)),
            }),
        }
    }

    fn read_bytes(&self, path: &str) -> Result<Option<Vec<u8>>, FilesError> {
        Ok(self.file(path).map(<[u8]>::to_vec))
    }

    /// The file read as text, then as YAML, as the manifest is.
    fn read_value(&self, path: &str) -> Result<Option<Value>, FilesError> {
        let Some(text) = self.read_text(path)? else { return Ok(None) };
        data_value(&text, path, &self.shown(path), &self.stem).map(Some)
    }

    /// The names of the files and folders directly under `path`, each once, in the table's
    /// order; `None` when no file lies under `path` (a file, or nothing).
    fn list(&self, path: &str) -> Result<Option<Vec<String>>, FilesError> {
        let prefix = if path.is_empty() { String::new() } else { format!("{path}/") };
        let mut names: Vec<String> = Vec::new();
        for (at, _) in &self.files {
            let Some(rest) = at.strip_prefix(&prefix) else { continue };
            let name = rest.split('/').next().unwrap_or(rest);
            if names.last().map(String::as_str) != Some(name) {
                names.push(name.to_string());
            }
        }
        Ok(if names.is_empty() { None } else { Some(names) })
    }

    fn shared(&self) -> Arc<dyn ExtensionFiles + Send + Sync> {
        Arc::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::MemoryFiles;
    use std::sync::Arc;

    fn finder(files: MemoryFiles, folders: &[&str]) -> ExtensionFinder {
        let folders = folders.iter().map(|path| ExtensionFolder { path: PathBuf::from(path), required: false }).collect();
        let files: Arc<dyn crate::files::Files> = Arc::new(files);
        ExtensionFinder::new(BuildFiles::new(Arc::clone(&files)), ServiceFiles::new(files), folders)
    }

    fn named(text: &str) -> Locator {
        Locator::Named(text.to_string())
    }

    #[test]
    fn an_extension_in_both_layouts_in_one_folder_is_a_problem() {
        let files = MemoryFiles::new().with("defs/p.yaml", "").with("defs/p/flmextension.yaml", "");
        let error = finder(files, &["defs"]).find(&named("p")).err().expect("a problem");
        assert_eq!(error.path, None);
        assert_eq!(
            error.detail,
            "the folder `defs` must hold the extension `p` as either `p.yaml` or `p/flmextension.yaml`, not both"
        );
    }

    #[test]
    fn a_locator_joins_each_folder_in_both_layouts() {
        let files = MemoryFiles::new()
            .with("second/@acme/style/flmextension.yaml", "extension: {description: Style.}\n")
            .with("third/@acme/style.yaml", "");
        let finder = finder(files, &["first", "second", "third"]);
        let found = finder.find(&named("@acme/style")).unwrap().expect("found");
        assert_eq!(found.identity, "second/@acme/style/flmextension.yaml");
        assert_eq!(found.manifest, serde_json::json!({ "extension": { "description": "Style." } }));
        assert!(finder.find(&named("nosuch")).unwrap().is_none());
        assert!(finder.find(&Locator::Current).unwrap().is_none());
    }

    #[test]
    fn the_identity_is_the_folded_path_of_the_manifest() {
        let files = MemoryFiles::new().with("doc/defs/x.yaml", "flm:\n  use: [y]\n");
        let finder = finder(files, &["doc/./sub/../defs"]);
        let found = finder.find(&named("x")).unwrap().expect("found");
        assert_eq!(found.identity, "doc/defs/x.yaml");
        let manifest = finder.manifest("doc/defs/x.yaml").expect("kept");
        assert_eq!(manifest.at(&["flm", "use"]).map(|node| node.place), Some(crate::seed::Place { line: 2, column: 8 }));
    }

    #[test]
    fn the_files_of_a_folder_extension_are_read_relative_to_it_and_a_single_file_has_none() {
        let files = MemoryFiles::new()
            .with("defs/f/flmextension.yaml", "")
            .with("defs/f/templates/html/a.yaml", "output: html\n")
            .with("defs/f/main.js", "export default 1;\n")
            .with("defs/s.yaml", "");
        let finder = finder(files, &["defs"]);
        let folder = finder.find(&named("f")).unwrap().expect("found").files;
        assert_eq!(folder.read_text("main.js").unwrap().as_deref(), Some("export default 1;\n"));
        assert_eq!(folder.read_text("absent.js").unwrap(), None);
        assert_eq!(folder.list("").unwrap(), Some(vec!["flmextension.yaml".into(), "main.js".into(), "templates".into()]));
        assert_eq!(folder.list("templates").unwrap(), Some(vec!["html".to_string()]));
        assert_eq!(folder.list("main.js").unwrap(), None);
        assert_eq!(folder.list("nosuch").unwrap(), None);
        let single = finder.find(&named("s")).unwrap().expect("found").files;
        assert_eq!(single.read_text("main.js").unwrap(), None);
        assert_eq!(single.read_value("a.yaml").unwrap(), None);
        assert_eq!(single.list("").unwrap(), None);
    }

    #[test]
    fn the_locators_listed_are_those_of_both_layouts_and_of_a_scope_in_search_path_then_name_order() {
        let files = MemoryFiles::new()
            .with("first/style.yaml", "")
            .with("first/@acme/kit/flmextension.yaml", "")
            .with("first/notes.txt", "")
            .with("first/plain/notes.yaml", "")
            .with("second/fancy/flmextension.yaml", "")
            .with("second/style/flmextension.yaml", "");
        let finder = finder(files, &["first", "missing", "second"]);
        assert_eq!(finder.locators(), ["@acme/kit", "style", "fancy"]);
        // The listing is not among the paths the finder read.
        assert!(finder.files.list().is_empty(), "{:?}", finder.files.list());
    }

    #[test]
    fn a_data_file_is_read_as_the_manifest_is() {
        let files = MemoryFiles::new()
            .with("defs/f/flmextension.yaml", "")
            .with("defs/f/a.yaml", "output: html\nengine: minijinja\n")
            .with("defs/f/bad.yaml", "output: [html\n");
        let found = finder(files, &["defs"]).find(&named("f")).unwrap().expect("found").files;
        assert_eq!(found.read_value("a.yaml").unwrap(), Some(serde_json::json!({ "output": "html", "engine": "minijinja" })));
        let error = found.read_value("bad.yaml").unwrap_err();
        assert!(error.detail.starts_with("the file `defs/f/bad.yaml` of the extension `f` must be valid YAML: "), "{error:?}");
    }

    #[test]
    fn a_manifest_that_is_not_yaml_is_placed_at_its_problem() {
        let finder = finder(MemoryFiles::new().with("defs/p.yaml", "a: [b\n"), &["defs"]);
        let error = finder.find(&named("p")).err().expect("a problem");
        assert_eq!(error.path.as_deref(), Some("defs/p.yaml"));
        assert!(error.detail.starts_with("the manifest `defs/p.yaml` of the extension `p` must be valid YAML: "), "{error:?}");
        assert_eq!(error.place, Some(flm_ext::source::Place { line: 2, column: 1 }));
    }

    #[test]
    fn the_place_of_a_key_path_is_read_from_the_kept_manifest() {
        let text = "extension:\n  js:\n    main: m.js\nflm:\n  use: [a, b]\n  features:\n    text: {width: 3}\n";
        let finder = finder(MemoryFiles::new().with("defs/x.yaml", text), &["defs"]);
        let found = finder.find(&named("x")).unwrap().expect("found");
        let place_of = found.place_of.expect("a resolver");
        let at = |line, column| Some(flm_ext::source::Place { line, column });
        assert_eq!(place_of("extension.js.main"), at(3, 11));
        assert_eq!(place_of("flm.use[1]"), at(5, 12));
        assert_eq!(place_of("flm.features.text.width"), at(7, 19));
        assert_eq!(place_of("flm.use"), at(5, 8));
        assert_eq!(place_of("flm.use[2]"), None);
        assert_eq!(place_of("flm.nosuch"), None);
        assert_eq!(place_of("flm.use[x]"), None);
    }

    #[test]
    fn static_files_are_read_and_listed_from_their_table() {
        let table: [(&'static str, &'static [u8]); 4] = [
            ("templates/html/none.yaml", b"output: html\n"),
            ("templates/html/simple/flmtemplate.yaml", b"main: simple.html\n"),
            ("templates/html/simple/simple.html", b"<p>"),
            ("templates/text/bad.yaml", b"a: [b\n"),
        ];
        let files = StaticFiles::new(table, "stdmain", "stdmain");
        assert_eq!(files.list("").unwrap(), Some(vec![String::from("templates")]));
        assert_eq!(files.list("templates").unwrap(), Some(vec![String::from("html"), String::from("text")]));
        assert_eq!(
            files.list("templates/html").unwrap(),
            Some(vec![String::from("none.yaml"), String::from("simple")])
        );
        assert_eq!(files.list("templates/html/none.yaml").unwrap(), None);
        assert_eq!(files.list("nothing").unwrap(), None);
        assert_eq!(files.read_text("templates/html/simple/simple.html").unwrap().as_deref(), Some("<p>"));
        assert_eq!(files.read_text("templates/html/simple").unwrap(), None);
        assert_eq!(files.read_bytes("templates/html/simple/simple.html").unwrap(), Some(b"<p>".to_vec()));
        assert_eq!(files.read_bytes("nothing").unwrap(), None);
        assert_eq!(
            files.read_value("templates/html/simple/flmtemplate.yaml").unwrap(),
            Some(serde_json::json!({"main": "simple.html"}))
        );
        assert_eq!(files.read_value("templates/text/bad.yaml").unwrap_err().path, "templates/text/bad.yaml");
        assert_eq!(files.shared().read_text("templates/html/simple/simple.html").unwrap().as_deref(), Some("<p>"));
    }

    #[cfg(all(feature = "fs", unix))]
    #[test]
    fn a_file_or_a_folder_linked_outside_the_extension_s_folder_is_refused_on_the_disk() {
        let root = std::env::temp_dir().join(format!("flm-folder-files-{}", std::process::id()));
        let folder = root.join("defs").join("x");
        std::fs::create_dir_all(folder.join("inside")).unwrap();
        std::fs::create_dir_all(root.join("elsewhere")).unwrap();
        std::fs::write(folder.join(MANIFEST_FILE), "").unwrap();
        std::fs::write(folder.join("inside").join("a.txt"), "A").unwrap();
        std::fs::write(root.join("outside.txt"), "B").unwrap();
        std::os::unix::fs::symlink(root.join("outside.txt"), folder.join("link.txt")).unwrap();
        std::os::unix::fs::symlink(root.join("elsewhere"), folder.join("templates")).unwrap();
        let folders = vec![ExtensionFolder { path: root.join("defs"), required: false }];
        let finder = ExtensionFinder::new(
            BuildFiles::new(Arc::new(crate::files::DiskFiles)),
            ServiceFiles::new(Arc::new(crate::files::DiskFiles)),
            folders,
        );
        let files = finder.find(&named("x")).unwrap().expect("found").files;
        assert_eq!(files.read_text("inside/a.txt").unwrap().as_deref(), Some("A"));
        assert_eq!(files.list("inside").unwrap(), Some(vec![String::from("a.txt")]));
        let file = files.read_text("link.txt").unwrap_err();
        assert_eq!(file.path, "link.txt");
        assert_eq!(
            file.detail,
            format!(
                "the file `{}` of the extension `x` leads outside the extension's folder `{}` through a symbolic link",
                folder.join("link.txt").display(),
                folder.display()
            )
        );
        let listed = files.list("templates").unwrap_err();
        assert_eq!(
            listed.detail,
            format!(
                "the folder `{}` of the extension `x` leads outside the extension's folder `{}` through a symbolic \
                 link",
                folder.join("templates").display(),
                folder.display()
            )
        );
        std::fs::remove_dir_all(&root).unwrap();
    }
}
