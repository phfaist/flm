//! Turning the artifacts of a command's outputs into named files, and handing them to a
//! *sink*, which writes them to the disk or keeps them in memory.
//!
//! The library makes each output as an [`ArtifactSet`]: an
//! [artifact](flm_core::core::artifact::Artifact) is one file's worth of output held as a
//! value (a folder artifact holds the files of a folder), identified by its producer and a
//! name and never by a path; an artifact set is the artifacts of one output, at most one of
//! which is the entry artifact, the one that represents the output as a whole (the HTML
//! page, the text, the LaTeX document). A command produces every output of its workflow, or
//! the one it names. This module gives each artifact a path and hands the resulting files
//! to their destination:
//!
//! - a **naming scheme** ([`Naming`]) gives each artifact its path, relative to the output
//!   folder. Every file of a command shares one *stem*, a file name without extension:
//!   [`Naming::Folder`] gives the stem `index`; [`Naming::File`] gives the name it holds,
//!   less the extension of one of the command's outputs when the name ends with one
//!   (`paper.html`, `paper.txt` and `paper` all give `paper` to a command writing the
//!   outputs `html` and `text`), and the name as written otherwise. The entry artifact of
//!   each output is `STEM.EXT`, `EXT` being the output's extension (`paper.html`,
//!   `paper.txt`); every other artifact, named `NAME` by its producer, is
//!   `STEM-NAME.EXT` (`paper-plot-3fa9c2d1.png`, `index-flmrecomposed.1a2b3c4d.sty`), with
//!   the extension its producer gives, else the one of its media type; a folder artifact
//!   (the files of one folder shipped as one artifact) is `STEM-NAME`, with no extension.
//!   An artifact that states a path of its own keeps it (the recomposed LaTeX's style file, which the
//!   document loads by that name). [`Naming`] implements the library's [`ArtifactNamer`],
//!   so a stage can ask where an artifact will be written before the artifact is made; the
//!   outputs of the command reach it as the [`NamingScope`] of each call.
//! - [`EntryNames`] wraps a naming scheme and gives the entry artifact of some outputs a
//!   path of its own, taken exactly as written: the setting `write.output_names` of the
//!   feature `write` ([`crate::stdpkg::write`]) sets those paths. Every other file of such
//!   an output keeps the stem of the wrapped scheme, because the producers of those files
//!   ask for their paths during the run, without knowing the output they belong to; only
//!   the write stage names an entry artifact.
//! - [`place`] turns one output's artifact set into [`NamedFile`]s, one for each file of a
//!   folder artifact, and refuses two artifacts of the output given the same path and a
//!   path that leaves the output folder.
//! - a **sink** ([`Sink`]) receives the named files of a command, every output's at once,
//!   in one delivery: [`DiskSink`] writes them under a folder of the disk (cargo feature
//!   `fs`), and [`MemorySink`] keeps them in memory. A program may implement its own sink;
//!   the `flm` program has one that prints a command's one file on standard output.
//!
//! The feature `write` of the host's package ([`crate::stdpkg::write`]) calls [`place`] for
//! every output the plan demands. Two outputs may hold a file at one path: when the two
//! files have the same bytes and media type, they are one file, written once and listed
//! under both outputs (a stylesheet that the compose templates of two outputs both ship);
//! when they differ, nothing is written. The feature hands the files of every output to the sink of
//! the request ([`Request::sink`](crate::session::Request::sink)) in one delivery, or hands
//! nothing when any output cannot be placed: the files of a command are written together
//! or not at all.
//!
//! [`DiskSink`] writes each file first into a temporary folder inside the output folder,
//! then moves it into place, and removes the temporary folder. It keeps one record per
//! output: beside the output's entry artifact, a list of the files it wrote for that
//! output. When it receives an output again, it removes the files of that output's
//! previous record that no file of the new delivery holds; a path the delivery writes is
//! never removed. A command that produces only one of the outputs reads and rewrites only
//! that output's record, so it leaves the other outputs' files in place, except a file
//! the two outputs shared that the one output no longer writes: that file is removed with
//! the rest of the output's stale files, and rendering the other output again writes it
//! back. [`MemorySink`] does the same with the files it holds.
//!
//! A file that is not the output of a document, such as the schema of the settings that
//! `flm schema` writes, is written by [`write_file`], alone and without such a list.
//! [`OutputFolder`] computes the address, seen from the output folder, of a file that the
//! output refers to instead of holding it (such as an image under the graphics strategy
//! `reference`).

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

use flm_core::core::artifact::{ArtifactData, ArtifactNamer, ArtifactRef, ArtifactSet, NamingScope, RelPath, media_type_of, path_of};
use flm_core::core::registry::RegisteredOutput;

use crate::files::fold;

/// A naming scheme: how the artifacts of a command's outputs are named, relative to the
/// output folder. See the [module documentation](self).
///
/// It implements the library's [`ArtifactNamer`]; the host contributes it to the library's
/// [service slot](flm_core::core::services) `flm.artifacts.namer` through the feature `write`
/// ([`crate::stdpkg::write`]). [`disk_destination`] chooses one from a path given by the
/// user.
///
/// # Examples
///
/// ```
/// use flm_core::core::artifact::{ArtifactId, ArtifactNamer, ArtifactRef, NamingScope, ScopedOutput};
/// use flm_main::output::Naming;
///
/// let scope = NamingScope {
///     outputs: vec![
///         ScopedOutput { name: "html".into(), extension: "html".to_string() },
///         ScopedOutput { name: "text".into(), extension: "txt".to_string() },
///     ],
/// };
/// let id = ArtifactId::new("graphics", "plot-3fa9c2d1");
/// let copy = ArtifactRef { id: &id, entry: false, media_type: "image/png", extension: Some("png"), output: None, folder: false };
/// let naming = Naming::File { name: "paper.txt".to_string() };
/// assert_eq!(naming.url_of(&copy, &scope).as_str(), "paper-plot-3fa9c2d1.png");
/// assert_eq!(Naming::Folder.url_of(&copy, &scope).as_str(), "index-plot-3fa9c2d1.png");
/// // A folder artifact is named with no extension.
/// let id = ArtifactId::new("compose", "icons");
/// let icons = ArtifactRef { id: &id, entry: false, media_type: "", extension: None, output: None, folder: true };
/// assert_eq!(naming.url_of(&icons, &scope).as_str(), "paper-icons");
/// assert_eq!(Naming::Folder.url_of(&icons, &scope).as_str(), "index-icons");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Naming {
    /// Every file is written into the output folder under the stem `index`: the entry
    /// artifact of each output as `index.EXT`, every other artifact as `index-NAME.EXT`.
    Folder,
    /// The files are named after `name`, a file name given by the user: the stem is `name`
    /// less the extension of one of the command's outputs, when `name` ends with one (the
    /// first output of the scope whose extension, with its dot, ends `name`), else `name`
    /// as written; it is `name` as written, too, when removing the extension would leave
    /// nothing (`.html`). The entry artifact of each output is `STEM.EXT`, every other
    /// artifact `STEM-NAME.EXT`: a file named this way always carries its output's
    /// extension (`notes.txt` gives `notes.txt.html` to the output `html`).
    File {
        /// The file name, as the user gave it.
        name: String,
    },
}

impl Naming {
    /// The stem every file of the command whose outputs are `scope` shares.
    fn stem(&self, scope: &NamingScope) -> String {
        match self {
            Naming::Folder => String::from("index"),
            Naming::File { name } => {
                let matched = scope.outputs.iter().find_map(|output| name.strip_suffix(&format!(".{}", output.extension)));
                match matched {
                    Some(stem) if !stem.is_empty() => stem.to_string(),
                    _ => name.clone(),
                }
            }
        }
    }
}

/// Where an output is written: the output folder and the program's current folder, from
/// which [`address_of`](OutputFolder::address_of) computes the address of a file the output
/// refers to.
///
/// A page that shows an image by reference refers to the image's file by an address
/// relative to the page, that is, from the output folder to the file. The host reads a file
/// at a path relative to the current folder, or absolute ([`crate::files`]); the output
/// folder is relative to the current folder too, or absolute. Only the program knows its
/// current folder, and it gives it here, so that the address is right wherever the output
/// is written, including above the current folder (the `flm` program's
/// `-o ../site/page.html`). The request carries it as
/// [`Request::output_folder`](crate::session::Request::output_folder).
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use flm_main::output::OutputFolder;
///
/// let beside = OutputFolder::default();
/// assert_eq!(beside.address_of(Path::new("doc/figures/plot.png")), "doc/figures/plot.png");
/// let out = OutputFolder::new("out");
/// assert_eq!(out.address_of(Path::new("doc/figures/plot.png")), "../doc/figures/plot.png");
/// let above = OutputFolder::new("../site").with_current("/home/me/paper");
/// assert_eq!(above.address_of(Path::new("plot.png")), "../paper/plot.png");
/// ```
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct OutputFolder {
    /// The folder the output is written in: relative to [`current`](OutputFolder::current),
    /// or absolute. The empty path, the default, is the current folder, which is also the
    /// folder an output printed on standard output, or kept in memory, is taken to be read
    /// from.
    pub folder: PathBuf,
    /// The program's current folder, as an absolute path; the empty path when the program
    /// does not give it.
    pub current: PathBuf,
}

impl OutputFolder {
    /// Returns the output folder `folder`, relative to the current folder or absolute, with
    /// no current folder given.
    pub fn new(folder: impl Into<PathBuf>) -> OutputFolder {
        OutputFolder { folder: folder.into(), current: PathBuf::new() }
    }

    /// Returns this value with [`current`](OutputFolder::current) set to `current`.
    pub fn with_current(mut self, current: impl Into<PathBuf>) -> OutputFolder {
        self.current = current.into();
        self
    }

    /// Returns the address of the file at `path` (relative to the current folder, or
    /// absolute) as seen from the output folder, its parts separated by `/`:
    /// `../figures/plot.png`. When the two paths alone cannot give the address, because the
    /// current folder is not given and the output folder is above it, or because only one
    /// of the two paths is absolute, the result is `path` itself, normalized ([`fold`]).
    pub fn address_of(&self, path: &Path) -> String {
        // Both paths are taken from the current folder, whose names an output's folder
        // lying above it needs (`../site` seen from `/a/doc` leads back through `doc`).
        // See decision rationale at [§flm-dd-dr:host-output-folder].
        let folder = fold(&self.current.join(&self.folder));
        let file = fold(&self.current.join(path));
        let as_written = || file.to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/");
        if folder.is_absolute() != file.is_absolute() {
            return as_written();
        }
        let folder: Vec<Component<'_>> = folder.components().collect();
        let names: Vec<Component<'_>> = file.components().collect();
        let common = folder.iter().zip(&names).take_while(|(a, b)| a == b).count();
        // Leaving a folder whose name is not known (`..`, or another drive) cannot be
        // written as an address.
        if !folder[common..].iter().all(|part| matches!(part, Component::Normal(_))) {
            return as_written();
        }
        let mut parts: Vec<String> = vec![String::from(".."); folder.len() - common];
        parts.extend(names[common..].iter().map(|part| part.as_os_str().to_string_lossy().into_owned()));
        parts.join("/")
    }
}

/// The file extension of an artifact of media type `media_type`. An unknown type gives
/// no extension.
fn extension_of(media_type: &str) -> Option<&'static str> {
    let essence = media_type.split(';').next().unwrap_or_default().trim();
    Some(match essence {
        "text/html" => "html",
        "text/css" => "css",
        "text/plain" => "txt",
        "text/javascript" | "application/javascript" => "js",
        "text/x-tex" | "application/x-tex" | "application/x-latex" => "tex",
        "application/json" | "application/vnd.citationstyles.csl+json" => "json",
        "application/x-bibtex" => "bib",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/svg+xml" => "svg",
        "image/gif" => "gif",
        "image/webp" => "webp",
        "application/pdf" => "pdf",
        _ => return None,
    })
}

impl ArtifactNamer for Naming {
    /// The path of `artifact`: `STEM.EXT` for an entry artifact, `STEM-NAME.EXT` for any
    /// other, `NAME` being the artifact's name ([`ArtifactId::name`](flm_core::core::artifact::ArtifactId::name))
    /// and `STEM` the stem of `scope` (see [`Naming`]). `EXT` is the extension the
    /// artifact gives, else the one of its media type; without either, the path has no
    /// extension. A folder artifact ([`ArtifactRef::folder`]) is named with no extension:
    /// `STEM-NAME`. The scheme does not read [`ArtifactRef::output`].
    fn url_of(&self, artifact: &ArtifactRef<'_>, scope: &NamingScope) -> RelPath {
        let stem = self.stem(scope);
        let base = if artifact.entry { stem } else { format!("{stem}-{}", artifact.id.name) };
        if artifact.folder {
            return RelPath::new(base);
        }
        let extension = artifact.extension.or_else(|| extension_of(artifact.media_type)).filter(|extension| !extension.is_empty());
        RelPath::new(match extension {
            Some(extension) => format!("{base}.{extension}"),
            None => base,
        })
    }
}

/// A naming scheme that gives the entry artifacts of some outputs a path of their own, and
/// every other artifact the path that another naming scheme gives: the paths that the
/// setting `write.output_names` of the feature `write` sets ([`crate::stdpkg::write`]).
///
/// `names` maps an output's name to the path, relative to the output folder, of that
/// output's entry artifact, taken exactly as written: no stem, and no extension added or
/// removed. The path applies when the artifact is an entry artifact
/// ([`ArtifactRef::entry`]) and names its output ([`ArtifactRef::output`]) among the keys
/// of `names`; any other artifact, including every other file of a renamed output, is
/// named by `namer`.
///
/// Only the write stage names an entry artifact, and only it knows the output of the
/// artifact it names: a producer that asks for a path during a run (a copied image, the
/// recomposed LaTeX's style file, a compose template's shipped files) asks for a file
/// other than an entry and gives no output. So the other files of a renamed output keep
/// the stem of `namer`.
///
/// # Examples
///
/// ```
/// use std::collections::BTreeMap;
/// use flm_core::core::artifact::{ArtifactId, ArtifactNamer, ArtifactRef, NamingScope, ScopedOutput};
/// use flm_main::output::{EntryNames, Naming};
///
/// let scope = NamingScope { outputs: vec![ScopedOutput { name: "bibtex".into(), extension: "bib".to_string() }] };
/// let names = BTreeMap::from([(String::from("bibtex"), String::from("refs.bib"))]);
/// let naming = EntryNames::new(&names, &Naming::Folder);
/// let id = ArtifactId::new("bibtex", "document");
/// let entry = ArtifactRef { id: &id, entry: true, media_type: "application/x-bibtex", extension: Some("bib"), output: Some("bibtex"), folder: false };
/// assert_eq!(naming.url_of(&entry, &scope).as_str(), "refs.bib");
/// let id = ArtifactId::new("graphics", "plot-3fa9c2d1");
/// let copy = ArtifactRef { id: &id, entry: false, media_type: "image/png", extension: Some("png"), output: Some("bibtex"), folder: false };
/// assert_eq!(naming.url_of(&copy, &scope).as_str(), "index-plot-3fa9c2d1.png");
/// ```
#[derive(Clone, Copy)]
pub struct EntryNames<'n> {
    names: &'n BTreeMap<String, String>,
    namer: &'n dyn ArtifactNamer,
}

impl<'n> EntryNames<'n> {
    /// Returns the naming scheme that gives the entry artifact of each output that `names`
    /// lists the path `names` gives it, and every other artifact the path that `namer`
    /// gives.
    pub fn new(names: &'n BTreeMap<String, String>, namer: &'n dyn ArtifactNamer) -> EntryNames<'n> {
        EntryNames { names, namer }
    }
}

impl fmt::Debug for EntryNames<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryNames").field("names", self.names).finish_non_exhaustive()
    }
}

impl ArtifactNamer for EntryNames<'_> {
    /// The path `names` gives the output of `artifact` when `artifact` is an entry
    /// artifact of an output that `names` lists; else the path that the wrapped naming
    /// scheme gives.
    fn url_of(&self, artifact: &ArtifactRef<'_>, scope: &NamingScope) -> RelPath {
        // See decision rationale at [§flm-dd-dr:host-output-names].
        let own = artifact.output.filter(|_| artifact.entry).and_then(|output| self.names.get(output));
        match own {
            Some(path) => RelPath::new(path.clone()),
            None => self.namer.url_of(artifact, scope),
        }
    }
}

/// One file of an output, ready to be written: its path relative to the output folder, the
/// output it belongs to, its media type and its bytes. [`place`] makes them; a [`Sink`]
/// receives them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedFile {
    /// The path, relative to the output folder, its parts separated by `/`.
    pub path: String,
    /// The name of the output the file belongs to, such as `html`.
    pub output: String,
    /// Whether the file is its output's entry artifact.
    pub entry: bool,
    /// The media type of the content.
    pub media_type: String,
    /// The content.
    pub bytes: Vec<u8>,
}

/// A problem placing an output's files ([`place`]) or writing them ([`Sink::receive`],
/// [`write_file`]); its `Display` text is [`detail`](OutputError::detail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputError {
    /// The path concerned, relative to the output folder (for [`write_file`], the path as
    /// given), when there is one.
    pub path: Option<String>,
    /// The problem, as a text for a reader.
    pub detail: String,
}

impl OutputError {
    fn at(path: &str, detail: impl Into<String>) -> OutputError {
        OutputError { path: Some(path.to_string()), detail: detail.into() }
    }
}

impl fmt::Display for OutputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for OutputError {}

/// Whether `path`, relative and separated by `/`, stays inside the folder it is relative
/// to.
fn stays_inside(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !path.split('/').any(|part| part.is_empty() || part == "." || part == "..")
        && !path.contains(':')
}

/// Returns the named files of the artifact set `set` of the output `output`, in the set's
/// order: each artifact at the path that [`path_of`] gives it, the path it states if it
/// states one, else the path `namer` gives under `scope`, the outputs of the command
/// ([`NamingScope::of`] over the registry and the plan's demands).
///
/// The namer receives, for the entry artifact, the output's declared extension, and for
/// every other artifact the extension the artifact gives; for both, the output's name.
///
/// A folder artifact ([`ArtifactData::Files`]) gives one named file per file it holds, in
/// its order, at `FOLDER/PATH`: `FOLDER` the path of the folder (the path it states, else
/// the one `namer` gives it as a folder, [`ArtifactRef::folder`]), `PATH` the file's path
/// within it. Each file takes the media type of its own extension ([`media_type_of`]),
/// `application/octet-stream` for an extension the table does not list or a file with
/// none; none of them is an entry file.
///
/// # Errors
///
/// Every problem found, one [`OutputError`] each: an artifact or a file of a folder
/// artifact whose path leaves the output folder (an absolute path, an empty part, `.` or
/// `..`, a backslash or a colon), two artifacts or files given the same path, an artifact
/// that holds no bytes (one that refers to a file instead of holding its content), a
/// folder artifact that holds no file.
pub fn place(
    output: &RegisteredOutput,
    set: &ArtifactSet,
    namer: &dyn ArtifactNamer,
    scope: &NamingScope,
) -> Result<Vec<NamedFile>, Vec<OutputError>> {
    let name = output.def.name.to_string();
    let mut files: Vec<NamedFile> = Vec::new();
    let mut errors = Vec::new();
    for artifact in set.iter() {
        let path = path_of(artifact, Some(&output.def), namer, scope).as_str().to_string();
        // Each file of the artifact: its path, media type and bytes.
        let placed: Vec<(String, String, &Vec<u8>)> = match &artifact.data {
            ArtifactData::Bytes(bytes) => vec![(path, artifact.media_type.clone(), bytes)],
            ArtifactData::FileRef(_) => {
                errors.push(OutputError::at(&path, format!("the file `{path}` must hold its bytes in memory, not refer to another file")));
                continue;
            }
            ArtifactData::Files(within) if within.is_empty() => {
                errors.push(OutputError::at(&path, format!("the folder artifact `{}` holds no file", artifact.id)));
                continue;
            }
            ArtifactData::Files(within) => within
                .iter()
                .map(|(file, bytes)| (format!("{path}/{file}"), media_type_by_extension(file.as_str()).to_string(), bytes))
                .collect(),
        };
        for (path, media_type, bytes) in placed {
            if !stays_inside(&path) {
                errors.push(OutputError::at(&path, format!("the path `{path}` must stay inside the output folder")));
                continue;
            }
            if files.iter().any(|file| file.path == path) {
                errors.push(OutputError::at(&path, format!("two files of the output must not have the same path `{path}`")));
                continue;
            }
            files.push(NamedFile { path, output: name.clone(), entry: artifact.entry, media_type, bytes: bytes.clone() });
        }
    }
    if errors.is_empty() { Ok(files) } else { Err(errors) }
}

/// The media type of a file of a folder artifact, by the extension of the last part of its
/// path `path` ([`media_type_of`]): what follows the last `.` of that part, when the `.`
/// neither begins nor ends it. A file with no extension, or with one the table does not
/// list, is `application/octet-stream`.
fn media_type_by_extension(path: &str) -> &'static str {
    let last = path.rsplit('/').next().unwrap_or(path);
    let extension = match last.rfind('.') {
        Some(dot) if dot > 0 && dot + 1 < last.len() => &last[dot + 1..],
        _ => "",
    };
    media_type_of(extension).unwrap_or("application/octet-stream")
}

/// The destination of the named files of a command: the disk ([`DiskSink`]), memory
/// ([`MemorySink`]), or any other place a program implements, such as standard output.
///
/// A sink receives the files of every output of one command at once, in one delivery. A
/// sink that keeps files keeps one *record* per output present in the delivery, named by
/// the path of that output's entry artifact (of its first file when it has none), and lists
/// in it the output's paths; when it receives an output again, it removes the files that
/// the previous record of that name held and that no file of the delivery holds
/// ([`stale`]). A sink is shared across threads, so it must be `Send` and `Sync`.
pub trait Sink: Send + Sync {
    /// Receives `files`, the files of one command's outputs, all at once. A file two
    /// outputs share appears once per output, with the same path and bytes.
    ///
    /// # Errors
    ///
    /// One [`OutputError`] for each file that could not be written or removed.
    fn receive(&self, files: &[NamedFile]) -> Result<(), Vec<OutputError>>;
}

/// Returns the paths of `previous` that `files` does not hold: the files of an earlier
/// delivery that the new one no longer contains.
pub fn stale<'p>(previous: &'p [String], files: &[NamedFile]) -> Vec<&'p String> {
    previous.iter().filter(|path| !files.iter().any(|file| &file.path == *path)).collect()
}

/// The records of the delivery `files`: for each output present, in the order the outputs
/// first appear, its record name (the path of its entry file, else of its first file) and
/// the paths of its files.
fn records_of(files: &[NamedFile]) -> Vec<(String, Vec<String>)> {
    let mut outputs: Vec<&str> = Vec::new();
    for file in files {
        if !outputs.contains(&file.output.as_str()) {
            outputs.push(&file.output);
        }
    }
    outputs
        .into_iter()
        .map(|output| {
            let own: Vec<&NamedFile> = files.iter().filter(|file| file.output == output).collect();
            let record = own.iter().find(|file| file.entry).unwrap_or(&own[0]).path.clone();
            (record, own.iter().map(|file| file.path.clone()).collect())
        })
        .collect()
}

/// The files of the delivery `files` at distinct paths: the first file of each path.
fn distinct(files: &[NamedFile]) -> Vec<&NamedFile> {
    let mut seen: Vec<&NamedFile> = Vec::new();
    for file in files {
        if !seen.iter().any(|kept| kept.path == file.path) {
            seen.push(file);
        }
    }
    seen
}

/// A [`Sink`] that keeps the files in memory, by path; [`files`](MemorySink::files) returns
/// them. It keeps one record per output, as [`DiskSink`] does.
#[derive(Debug, Default)]
pub struct MemorySink {
    files: Mutex<BTreeMap<String, NamedFile>>,
    records: Mutex<BTreeMap<String, Vec<String>>>,
}

impl MemorySink {
    /// Returns a sink holding no file.
    pub fn new() -> MemorySink {
        MemorySink::default()
    }

    /// Returns a copy of the files the sink holds, by path. A file two outputs share is
    /// held once, as the first output of the delivery gave it.
    pub fn files(&self) -> BTreeMap<String, NamedFile> {
        self.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone()
    }
}

impl Sink for MemorySink {
    fn receive(&self, files: &[NamedFile]) -> Result<(), Vec<OutputError>> {
        let mut held = self.files.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut records = self.records.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let delivered = records_of(files);
        for (record, _) in &delivered {
            if let Some(previous) = records.get(record) {
                for path in stale(previous, files) {
                    held.remove(path);
                }
            }
        }
        for file in distinct(files) {
            held.insert(file.path.clone(), file.clone());
        }
        records.extend(delivered);
        Ok(())
    }
}

/// A [`Sink`] that writes the files under a folder of the disk, the output folder (cargo
/// feature `fs`).
///
/// Each file is first written into a temporary folder inside the output folder, then moved
/// into place; the temporary folder is removed afterwards. A file two outputs share is
/// written once. The paths written for each output are listed in a hidden file beside the
/// output's entry artifact, `.ENTRY.flm-written`; a later delivery of the same output
/// reads that list and removes the files that no file of the delivery holds.
#[cfg(feature = "fs")]
#[derive(Debug, Clone)]
pub struct DiskSink {
    location: std::path::PathBuf,
}

#[cfg(feature = "fs")]
impl DiskSink {
    /// Returns a sink that writes under the folder `location`, which is created when it
    /// does not exist; the empty path is the current folder.
    pub fn new(location: impl Into<std::path::PathBuf>) -> DiskSink {
        DiskSink { location: location.into() }
    }

    /// Returns the folder the sink writes under, as it was given.
    pub fn location(&self) -> &Path {
        &self.location
    }

    /// Returns the path of the file that lists the files written for the record name
    /// `record`.
    fn record_file(&self, record: &str) -> std::path::PathBuf {
        let (folder, name) = match record.rfind('/') {
            Some(slash) => (&record[..=slash], &record[slash + 1..]),
            None => ("", record),
        };
        // See decision rationale at [§flm-dd-dr:host-written-record].
        self.location.join(format!("{folder}.{name}.flm-written"))
    }

    fn write_all(&self, scratch: &std::path::Path, files: &[&NamedFile]) -> Vec<OutputError> {
        let mut errors = Vec::new();
        let failed = |path: &str, error: std::io::Error| {
            OutputError::at(path, format!("the file `{path}` cannot be written: {error}"))
        };
        for (index, file) in files.iter().enumerate() {
            let temporary = scratch.join(index.to_string());
            let target = self.location.join(&file.path);
            let result = std::fs::write(&temporary, &file.bytes)
                .and_then(|()| match target.parent() {
                    Some(parent) => std::fs::create_dir_all(parent),
                    None => Ok(()),
                })
                .and_then(|()| std::fs::rename(&temporary, &target));
            if let Err(error) = result {
                errors.push(failed(&file.path, error));
            }
        }
        errors
    }
}

#[cfg(feature = "fs")]
impl Sink for DiskSink {
    fn receive(&self, files: &[NamedFile]) -> Result<(), Vec<OutputError>> {
        let location = if self.location.as_os_str().is_empty() {
            std::path::Path::new(".")
        } else {
            self.location.as_path()
        };
        let scratch = location.join(format!(".flm-scratch-{}", std::process::id()));
        let prepared = std::fs::create_dir_all(location)
            .and_then(|()| {
                let _ = std::fs::remove_dir_all(&scratch);
                std::fs::create_dir(&scratch)
            });
        if let Err(error) = prepared {
            let place = location.display();
            return Err(vec![OutputError { path: None, detail: format!("the folder `{place}` cannot be written: {error}") }]);
        }
        let mut errors = self.write_all(&scratch, &distinct(files));
        let _ = std::fs::remove_dir_all(&scratch);
        for (record, paths) in records_of(files) {
            let record_file = self.record_file(&record);
            let previous: Vec<String> = std::fs::read_to_string(&record_file)
                .map(|text| text.lines().map(str::to_string).collect())
                .unwrap_or_default();
            for path in stale(&previous, files) {
                if stays_inside(path) {
                    match std::fs::remove_file(location.join(path)) {
                        Err(error) if error.kind() != std::io::ErrorKind::NotFound => errors.push(
                            OutputError::at(path, format!("the file `{path}` cannot be removed: {error}")),
                        ),
                        _ => {}
                    }
                }
            }
            let listing: String = paths.iter().map(|path| format!("{path}\n")).collect();
            if let Err(error) = std::fs::write(&record_file, listing) {
                errors.push(OutputError {
                    path: None,
                    detail: format!("the list of written files `{}` cannot be written: {error}", record_file.display()),
                });
            }
        }
        if errors.is_empty() { Ok(()) } else { Err(errors) }
    }
}

/// Writes `bytes` into the file at `path`, replacing the file if it exists (cargo feature
/// `fs`). It is meant for one file that is not the output of a document, such as a schema;
/// it keeps no list of written files.
///
/// # Errors
///
/// An [`OutputError`] when the file cannot be written.
#[cfg(feature = "fs")]
pub fn write_file(path: &std::path::Path, bytes: &[u8]) -> Result<(), OutputError> {
    std::fs::write(path, bytes).map_err(|error| OutputError {
        path: Some(path.display().to_string()),
        detail: format!("the file `{}` cannot be written: {error}", path.display()),
    })
}

/// Returns the sink and the naming scheme for the files of a command written at `path` on
/// the disk (cargo feature `fs`): when `path` ends with `/` or is an existing folder, a
/// [`DiskSink`] for that folder with [`Naming::Folder`]; otherwise a [`DiskSink`] for the
/// folder of `path` with [`Naming::File`], whose name is the file name of `path`.
#[cfg(feature = "fs")]
pub fn disk_destination(path: &std::path::Path) -> (DiskSink, Naming) {
    let text = path.to_string_lossy();
    if text.ends_with('/') || text.ends_with(std::path::MAIN_SEPARATOR) || path.is_dir() {
        return (DiskSink::new(path), Naming::Folder);
    }
    let folder = path.parent().map(std::path::Path::to_path_buf).unwrap_or_default();
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    (DiskSink::new(folder), Naming::File { name })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flm_core::core::artifact::{Artifact, ArtifactId, ScopedOutput};
    use flm_core::core::data::table::TableId;
    use flm_core::core::registry::{CapId, OutputDef};

    fn scope(extensions: &[(&str, &str)]) -> NamingScope {
        NamingScope {
            outputs: extensions
                .iter()
                .map(|(name, extension)| ScopedOutput { name: (*name).into(), extension: extension.to_string() })
                .collect(),
        }
    }

    fn both() -> NamingScope {
        scope(&[("html", "html"), ("latex-recomposed", "tex")])
    }

    fn output(name: &str, extension: &str) -> RegisteredOutput {
        RegisteredOutput {
            def: OutputDef::new(name, "cap", "table", "text/html", extension),
            owner: "test".into(),
            table: TableId(0),
            capability: CapId(0),
        }
    }

    fn set(fixed: Option<&str>) -> ArtifactSet {
        let mut set = ArtifactSet::new();
        set.insert(Artifact::bytes(ArtifactId::new("html", "document"), "text/html", b"<p>".to_vec()).as_entry())
            .unwrap();
        let mut style = Artifact::bytes(ArtifactId::new("html", "style"), "text/css", b"p{}".to_vec());
        style.fixed_path = fixed.map(RelPath::new);
        set.insert(style).unwrap();
        set
    }

    fn paths(files: &[NamedFile]) -> Vec<&str> {
        files.iter().map(|file| file.path.as_str()).collect()
    }

    fn file(path: &str, output: &str, entry: bool, bytes: &[u8]) -> NamedFile {
        NamedFile {
            path: path.to_string(),
            output: output.to_string(),
            entry,
            media_type: String::from("text/plain"),
            bytes: bytes.to_vec(),
        }
    }

    fn named(naming: &Naming, id: &ArtifactId, entry: bool, media_type: &str, extension: Option<&str>, scope: &NamingScope) -> String {
        let artifact = ArtifactRef { id, entry, media_type, extension, output: None, folder: false };
        naming.url_of(&artifact, scope).as_str().to_string()
    }

    #[test]
    fn an_address_climbs_out_of_the_output_s_folder_to_the_file() {
        let address = |folder: &str, current: &str, file: &str| {
            OutputFolder::new(folder).with_current(current).address_of(Path::new(file))
        };
        assert_eq!(address("", "", "a/b.png"), "a/b.png");
        assert_eq!(address("out/x", "", "./a/../b.png"), "../../b.png");
        assert_eq!(address("../site", "/home/me/doc", "b.png"), "../doc/b.png");
        assert_eq!(address("/srv/site", "/home/me", "b.png"), "../../home/me/b.png");
        // Where the current folder is not given, a folder above it has no known name.
        assert_eq!(address("../site", "", "b.png"), "b.png");
        assert_eq!(address("out", "", "/pics/b.png"), "/pics/b.png");
    }

    #[test]
    fn the_stem_drops_the_extension_of_any_of_the_command_s_outputs() {
        let stem = |name: &str| Naming::File { name: name.to_string() }.stem(&both());
        assert_eq!(stem("paper.html"), "paper");
        assert_eq!(stem("paper.tex"), "paper");
        assert_eq!(stem("paper"), "paper");
        assert_eq!(stem("index.htm"), "index.htm");
        assert_eq!(stem(".html"), ".html");
        assert_eq!(Naming::Folder.stem(&both()), "index");
        // An extension of an output the command does not produce is kept.
        assert_eq!(Naming::File { name: "paper.tex".into() }.stem(&scope(&[("html", "html")])), "paper.tex");
    }

    #[test]
    fn an_entry_is_stem_dot_extension_and_a_side_artifact_stem_dash_name() {
        let file = Naming::File { name: "paper.html".into() };
        let document = ArtifactId::new("html", "document");
        let plot = ArtifactId::new("graphics", "plot-3fa9c2d1");
        let blob = ArtifactId::new("x", "blob");
        assert_eq!(named(&file, &document, true, "text/html", Some("html"), &both()), "paper.html");
        assert_eq!(named(&file, &document, true, "application/x-latex", Some("tex"), &both()), "paper.tex");
        assert_eq!(named(&file, &plot, false, "image/png", Some("png"), &both()), "paper-plot-3fa9c2d1.png");
        // The extension of the media type, when the artifact gives none.
        assert_eq!(named(&file, &plot, false, "image/png", None, &both()), "paper-plot-3fa9c2d1.png");
        // An artifact without a known extension.
        assert_eq!(named(&file, &blob, false, "application/x-unknown", None, &both()), "paper-blob");
        assert_eq!(named(&Naming::Folder, &document, true, "text/html", Some("html"), &both()), "index.html");
        assert_eq!(named(&Naming::Folder, &plot, false, "image/png", Some("png"), &both()), "index-plot-3fa9c2d1.png");
        // A file named by the user always carries its output's extension.
        let notes = Naming::File { name: "notes.txt".into() };
        assert_eq!(named(&notes, &document, true, "text/html", Some("html"), &scope(&[("html", "html")])), "notes.txt.html");
        let htm = Naming::File { name: "index.htm".into() };
        assert_eq!(named(&htm, &document, true, "application/x-latex", Some("tex"), &both()), "index.htm.tex");
    }

    #[test]
    fn place_names_the_files_under_the_scope_and_keeps_a_fixed_path() {
        let html = output("html", "html");
        let file = Naming::File { name: "paper.tex".into() };
        let placed = place(&html, &set(None), &file, &both()).unwrap();
        assert_eq!(paths(&placed), ["paper.html", "paper-style.css"]);
        assert!(placed.iter().all(|file| file.output == "html"));
        let folder = place(&html, &set(None), &Naming::Folder, &both()).unwrap();
        assert_eq!(paths(&folder), ["index.html", "index-style.css"]);
        let fixed = place(&html, &set(Some("flmstyle.sty")), &file, &both()).unwrap();
        assert_eq!(paths(&fixed), ["paper.html", "flmstyle.sty"]);
    }

    #[test]
    fn entry_names_renames_the_entry_of_a_listed_output_alone() {
        let html = output("html", "html");
        let names = BTreeMap::from([(String::from("html"), String::from("site/page"))]);
        let naming = Naming::File { name: "paper.tex".into() };
        let placed = place(&html, &set(None), &EntryNames::new(&names, &naming), &both()).unwrap();
        assert_eq!(paths(&placed), ["site/page", "paper-style.css"]);
        // An output the map does not list keeps the scheme's names.
        let other = BTreeMap::from([(String::from("text"), String::from("notes.txt"))]);
        let placed = place(&html, &set(None), &EntryNames::new(&other, &Naming::Folder), &both()).unwrap();
        assert_eq!(paths(&placed), ["index.html", "index-style.css"]);
        // A path leaving the output folder is refused, as any other.
        let leaving = BTreeMap::from([(String::from("html"), String::from("../page.html"))]);
        assert!(place(&html, &set(None), &EntryNames::new(&leaving, &Naming::Folder), &both()).is_err());
    }

    #[test]
    fn a_path_leaving_the_location_and_two_files_of_one_path_are_refused() {
        let html = output("html", "html");
        let file = Naming::File { name: "p.html".into() };
        assert!(place(&html, &set(Some("../x.css")), &file, &both()).is_err());
        assert!(place(&html, &set(Some("p.html")), &file, &both()).is_err());
    }

    /// The set of a page and the folder artifact `compose:icons` holding `within`.
    fn with_folder(within: &[(&str, &[u8])]) -> ArtifactSet {
        let mut set = ArtifactSet::new();
        set.insert(Artifact::bytes(ArtifactId::new("html", "document"), "text/html", b"<p>".to_vec()).as_entry())
            .unwrap();
        let files = within.iter().map(|(path, bytes)| (RelPath::new(*path), bytes.to_vec())).collect();
        set.insert(Artifact::folder(ArtifactId::new("compose", "icons"), files)).unwrap();
        set
    }

    #[test]
    fn a_folder_is_named_with_no_extension() {
        let icons = ArtifactId::new("compose", "icons");
        let named = |naming: &Naming| {
            let artifact = ArtifactRef { id: &icons, entry: false, media_type: "", extension: None, output: None, folder: true };
            naming.url_of(&artifact, &both()).as_str().to_string()
        };
        assert_eq!(named(&Naming::File { name: "paper.html".into() }), "paper-icons");
        assert_eq!(named(&Naming::Folder), "index-icons");
        // An extension given with a folder is not used.
        let artifact = ArtifactRef { id: &icons, entry: false, media_type: "image/png", extension: Some("png"), output: None, folder: true };
        assert_eq!(Naming::Folder.url_of(&artifact, &both()).as_str(), "index-icons");
    }

    #[test]
    fn place_writes_one_file_per_file_of_a_folder_with_its_media_type() {
        let html = output("html", "html");
        let set = with_folder(&[("check.svg", b"<svg/>"), ("more/x.png", b"png"), ("data.weird", b"w"), ("LICENSE", b"l")]);
        // `-o out/paper.html`: the stem `paper`.
        let placed = place(&html, &set, &Naming::File { name: "paper.html".into() }, &both()).unwrap();
        let described: Vec<(&str, &str, bool, &[u8])> = placed
            .iter()
            .map(|file| (file.path.as_str(), file.media_type.as_str(), file.entry, file.bytes.as_slice()))
            .collect();
        assert_eq!(
            described,
            [
                ("paper-icons/check.svg", "image/svg+xml", false, &b"<svg/>"[..]),
                ("paper-icons/more/x.png", "image/png", false, &b"png"[..]),
                ("paper-icons/data.weird", "application/octet-stream", false, &b"w"[..]),
                ("paper-icons/LICENSE", "application/octet-stream", false, &b"l"[..]),
                ("paper.html", "text/html", true, &b"<p>"[..]),
            ]
        );
        // `-o out/`: the stem `index`.
        let placed = place(&html, &set, &Naming::Folder, &both()).unwrap();
        assert_eq!(
            paths(&placed),
            ["index-icons/check.svg", "index-icons/more/x.png", "index-icons/data.weird", "index-icons/LICENSE", "index.html"]
        );
        // The record of the output lists each file.
        let sink = MemorySink::new();
        sink.receive(&placed).unwrap();
        let records = sink.records.lock().unwrap().clone();
        assert_eq!(
            records["index.html"],
            ["index-icons/check.svg", "index-icons/more/x.png", "index-icons/data.weird", "index-icons/LICENSE", "index.html"]
        );
    }

    #[test]
    fn an_empty_folder_and_a_file_leaving_the_output_folder_are_refused() {
        let html = output("html", "html");
        let file = Naming::File { name: "p.html".into() };
        let refused = |set: &ArtifactSet| -> Vec<String> {
            place(&html, set, &file, &both()).unwrap_err().into_iter().map(|error| error.detail).collect()
        };
        assert_eq!(refused(&with_folder(&[])), ["the folder artifact `compose:icons` holds no file"]);
        assert_eq!(
            refused(&with_folder(&[("../x.svg", b"x")])),
            ["the path `p-icons/../x.svg` must stay inside the output folder"]
        );
    }

    #[cfg(feature = "fs")]
    #[test]
    fn a_disk_sink_writes_a_folder_s_files_and_removes_a_stale_one() {
        let folder = std::env::temp_dir().join(format!("flm-output-folder-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let html = output("html", "html");
        let naming = Naming::File { name: "paper.html".into() };
        let sink = DiskSink::new(&folder);
        let set = with_folder(&[("check.svg", b"<svg/>"), ("more/x.png", b"png")]);
        sink.receive(&place(&html, &set, &naming, &both()).unwrap()).unwrap();
        let read = |name: &str| std::fs::read_to_string(folder.join(name)).ok();
        assert_eq!(read("paper-icons/more/x.png").as_deref(), Some("png"));
        assert_eq!(read(".paper.html.flm-written").as_deref(), Some("paper-icons/check.svg\npaper-icons/more/x.png\npaper.html\n"));
        let set = with_folder(&[("check.svg", b"<svg/>")]);
        sink.receive(&place(&html, &set, &naming, &both()).unwrap()).unwrap();
        assert_eq!(read("paper-icons/more/x.png"), None);
        assert_eq!(read("paper-icons/check.svg").as_deref(), Some("<svg/>"));
        let _ = std::fs::remove_dir_all(&folder);
    }

    /// A delivery of the outputs `html` and `tex` sharing the file `shared.png`.
    fn delivery() -> Vec<NamedFile> {
        vec![
            file("p.html", "html", true, b"<p>"),
            file("shared.png", "html", false, b"png"),
            file("p.tex", "tex", true, b"\\x"),
            file("shared.png", "tex", false, b"png"),
            file("p-style.sty", "tex", false, b"sty"),
        ]
    }

    #[test]
    fn a_memory_sink_keeps_one_record_per_output() {
        let sink = MemorySink::new();
        sink.receive(&delivery()).unwrap();
        let held = sink.files();
        assert_eq!(held.keys().collect::<Vec<_>>(), ["p-style.sty", "p.html", "p.tex", "shared.png"]);
        // The shared file is held once, as the first output gave it.
        assert_eq!(held["shared.png"].output, "html");
        let records = sink.records.lock().unwrap().clone();
        assert_eq!(records["p.html"], ["p.html", "shared.png"]);
        assert_eq!(records["p.tex"], ["p.tex", "shared.png", "p-style.sty"]);
        // The output `tex` drops its style file and its image; the image is still the
        // output `html`'s, and stays.
        let next = vec![
            file("p.html", "html", true, b"<p>"),
            file("shared.png", "html", false, b"png"),
            file("p.tex", "tex", true, b"\\x"),
        ];
        sink.receive(&next).unwrap();
        assert_eq!(sink.files().keys().collect::<Vec<_>>(), ["p.html", "p.tex", "shared.png"]);
        // A delivery narrowed to the output `html` leaves the output `tex`'s files.
        sink.receive(&next[..2]).unwrap();
        assert_eq!(sink.files().keys().collect::<Vec<_>>(), ["p.html", "p.tex", "shared.png"]);
        // The output `html` drops the image; no file of the delivery holds it: it goes.
        sink.receive(&next[..1]).unwrap();
        assert_eq!(sink.files().keys().collect::<Vec<_>>(), ["p.html", "p.tex"]);
    }

    #[cfg(feature = "fs")]
    #[test]
    fn a_disk_sink_writes_a_shared_file_once_and_keeps_one_record_per_output() {
        let folder = std::env::temp_dir().join(format!("flm-output-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let sink = DiskSink::new(&folder);
        sink.receive(&delivery()).unwrap();
        let read = |name: &str| std::fs::read_to_string(folder.join(name)).ok();
        assert_eq!(read("shared.png").as_deref(), Some("png"));
        assert_eq!(read(".p.html.flm-written").as_deref(), Some("p.html\nshared.png\n"));
        assert_eq!(read(".p.tex.flm-written").as_deref(), Some("p.tex\nshared.png\np-style.sty\n"));
        // The output `html` no longer holds the image, which the output `tex` still writes:
        // it is kept. The output `tex` drops its style file: it is removed.
        let next = vec![
            file("p.html", "html", true, b"<p>"),
            file("p.tex", "tex", true, b"\\x"),
            file("shared.png", "tex", false, b"png"),
        ];
        sink.receive(&next).unwrap();
        assert_eq!(read("shared.png").as_deref(), Some("png"));
        assert_eq!(read("p-style.sty"), None);
        assert_eq!(read(".p.html.flm-written").as_deref(), Some("p.html\n"));
        // A delivery narrowed to the output `html` leaves the output `tex`'s files.
        sink.receive(&next[..1]).unwrap();
        assert_eq!(read("p.tex").as_deref(), Some("\\x"));
        assert_eq!(read("shared.png").as_deref(), Some("png"));
        // A stale path of the output's own record is removed.
        sink.receive(&[file("p.tex", "tex", true, b"\\x")]).unwrap();
        assert_eq!(read("shared.png"), None);
        let _ = std::fs::remove_dir_all(&folder);
    }
}
