//! The syntax of a path that a compose template or an extension's code reads:
//! a path with a root ([`QualifiedPath`]), a bare path joined to the place of
//! the text it is written in ([`WrittenPath::at`]), the conditions of the
//! forms that have no meaning ([`PathError`]), and the files of the
//! extensions of a preparation by their use names ([`ExtensionsByName`]).
//!
//! # The syntax
//!
//! A path is read by three rules, in this order ([`WrittenPath::parse`]).
//!
//! 1. A path that begins with `/`, is `~`, or begins with `~/` is an
//!    *absolute path*, as in a document. It is read as `doc:` followed by
//!    itself: `/C:/refs.bib` is `doc:/C:/refs.bib`.
//! 2. Any other path that holds `:` is `ROOT:PATH`: the text before the
//!    first `:` is the root (`ext+@acme/style:img/logo.svg` has the root
//!    `ext+@acme/style`).
//! 3. Any other path is a *bare path*. It holds no `:`.
//!
//! # The roots
//!
//! | Root | The folder | The path after the root |
//! |---|---|---|
//! | `self` | the base folder of the files that the text belongs to: the folder of its extension, or the program's own files for a text of `std` | relative, inside the folder |
//! | `ext+NAME` | the folder of the extension that the use name `NAME` finds | relative, inside the folder |
//! | `std` | the program's own files (`std:templates/html/simple/simple.html`) | relative, inside the folder |
//! | `doc` | the document's folder: the folder of the input file, or the current folder for a text given as the input | as a path written in the document: relative, absolute, `~`, a `..` that climbs |
//! | `cwd` | the folder in which the program runs | as a path given on the command line |
//! | `https` | an address on the network, `https://…` | the address |
//!
//! The *use name* of an extension is the text written in a `use` entry that
//! finds it, without its `#PACKAGE` part (`journals/aps`, `@acme/style`).
//!
//! A path after a root of a base folder (`self`, `ext+NAME`, `std`) is kept
//! with its `.` and empty names left out and each `..` taking off the name
//! before it; a `..` that climbs above the base folder is refused, and so is
//! an absolute path after the root. A path after `doc` or `cwd` is kept as
//! written: the host reads it as it reads a path of the document.
//!
//! # The place of a text
//!
//! The *place* of a text is the file that the text was read from, given as a
//! qualified path. A bare path is relative to the folder of its text's place:
//! `partials/head.html` written in `ext+x:templates/html/a/page.html` is
//! `ext+x:templates/html/a/partials/head.html`. Its `..` may climb inside the
//! same base folder, and under `doc` and `cwd` as a path of the document may.
//! `self:` takes the root of the place, which must be `std` or an extension's
//! (a file of the document's folder belongs to no extension). A text that no
//! file holds has no place: its caller refuses a bare path and `self:`. No
//! text that the program applies is read from an address: a bare path at a
//! place under `https` is refused ([`PathErrorKind::BesideAddress`]).
//!
//! # A drive letter
//!
//! A path whose root is one letter (`C:\Users\me\logo.png`), or that begins
//! with `\`, one letter and `:`, is refused with the form of an absolute path
//! on Windows: `/C:/Users/me/logo.png`.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex};

use crate::source::{ExtensionFiles, Outside, held_at, is_absolute};

/// The prefix of the root of an extension's folder, `ext+NAME`.
const EXT_PREFIX: &str = "ext+";

/// A root other than `self`: the folder a [`QualifiedPath`] is read in. See the
/// [module documentation](self#the-roots).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Root {
    /// `ext+NAME`: the folder of the extension that the use name `NAME` finds.
    Ext(String),
    /// `std`: the program's own files.
    Std,
    /// `doc`: the document's folder.
    Doc,
    /// `cwd`: the folder in which the program runs.
    Cwd,
    /// `https`: an address on the network.
    Https,
}

impl Root {
    /// Whether the root names a base folder (`ext+NAME`, `std`), in which a
    /// path is relative and stays inside the folder.
    pub fn is_base_folder(&self) -> bool {
        matches!(self, Root::Ext(_) | Root::Std)
    }
}

impl fmt::Display for Root {
    /// The root as written: `ext+NAME`, `std`, `doc`, `cwd`, `https`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Root::Ext(name) => write!(f, "{EXT_PREFIX}{name}"),
            Root::Std => f.write_str("std"),
            Root::Doc => f.write_str("doc"),
            Root::Cwd => f.write_str("cwd"),
            Root::Https => f.write_str("https"),
        }
    }
}

/// A path with a root other than `self`: what a reader of the roots reads
/// (written `ROOT:PATH`). Under a base folder, the path is relative to the
/// folder and stays inside it, its `.`, its empty names and its `..` resolved
/// ([module documentation](self#the-roots)).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct QualifiedPath {
    root: Root,
    path: String,
}

impl QualifiedPath {
    /// The path `path` under `root`.
    ///
    /// # Errors
    ///
    /// Under a base folder, [`PathErrorKind::NotRelative`] for an absolute
    /// `path` and [`PathErrorKind::Above`] for a `..` that climbs above the
    /// folder, each with the path written `ROOT:PATH`.
    pub fn new(root: Root, path: &str) -> Result<QualifiedPath, PathError> {
        if !root.is_base_folder() {
            return Ok(QualifiedPath { root, path: path.to_string() });
        }
        let path = held_at("", path)
            .map_err(|outside| PathError { path: format!("{root}:{path}"), kind: outside_at(outside, &root.to_string()) })?;
        Ok(QualifiedPath { root, path })
    }

    /// The root.
    pub fn root(&self) -> &Root {
        &self.root
    }

    /// The path after the root: relative to the base folder under a base
    /// folder, as written under `doc` and `cwd`, the address after `https:`
    /// (`//example.org/a.css`).
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The name of the file that the path names: its last part (`logo.png` for
    /// `ext+x:img/logo.png`); under `https`, the last part of the address's path,
    /// without the query and the fragment (`font.woff2` for
    /// `https://example.org/a/font.woff2?v=2`), empty for an address whose path
    /// ends with `/` or that has none.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_ext::paths::{QualifiedPath, Root};
    ///
    /// let address = QualifiedPath::new(Root::Https, "//example.org/a/font.woff2?v=2#x").unwrap();
    /// assert_eq!(address.file_name(), "font.woff2");
    /// let address = QualifiedPath::new(Root::Https, "//example.org?v=2").unwrap();
    /// assert_eq!(address.file_name(), "");
    /// ```
    pub fn file_name(&self) -> &str {
        let path = match self.root {
            Root::Https => {
                let address = self.path.strip_prefix("//").unwrap_or(&self.path);
                let address = address.split(['?', '#']).next().unwrap_or("");
                address.split_once('/').map_or("", |(_, path)| path)
            }
            _ => &self.path,
        };
        path.rsplit('/').next().unwrap_or("")
    }

    /// The path `bare`, relative to the folder of this place's file.
    fn join(&self, bare: &str) -> Result<QualifiedPath, PathError> {
        // `bare` holds no `:` and does not begin with `/` or `~`: joined to a relative
        // folder, it can only climb.
        let above = |_| PathError { path: bare.to_string(), kind: PathErrorKind::Above(self.root.to_string()) };
        let path = match &self.root {
            Root::Ext(_) | Root::Std => held_at(&self.path, bare).map_err(above)?,
            Root::Doc | Root::Cwd => beside(&self.path, bare),
            // No text that the program applies is read from the network.
            Root::Https => return Err(PathError { path: bare.to_string(), kind: PathErrorKind::BesideAddress }),
        };
        Ok(QualifiedPath { root: self.root.clone(), path })
    }
}

impl fmt::Display for QualifiedPath {
    /// `ROOT:PATH`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.root, self.path)
    }
}

/// A path as written, read by the three rules of the
/// [module documentation](self#the-syntax).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WrittenPath {
    /// A path with a root other than `self`, an absolute path among them
    /// (`doc:/C:/refs.bib`).
    Qualified(QualifiedPath),
    /// `self:PATH`: the path, relative to the base folder of the text's place,
    /// its `.`, its empty names and its `..` resolved.
    Own(String),
    /// A bare path, relative to the folder of the text's place, as written.
    Bare(String),
}

impl WrittenPath {
    /// The path `text`, read by the three rules of the
    /// [module documentation](self#the-syntax).
    ///
    /// # Errors
    ///
    /// [`PathErrorKind::DriveLetter`] for a root of one letter, alone or after
    /// `\`; [`PathErrorKind::UnknownRoot`] for a root that is none of the six;
    /// and, under a base folder, the errors of [`QualifiedPath::new`].
    pub fn parse(text: &str) -> Result<WrittenPath, PathError> {
        if is_absolute(text) {
            return Ok(WrittenPath::Qualified(QualifiedPath { root: Root::Doc, path: text.to_string() }));
        }
        let Some((root, path)) = text.split_once(':') else { return Ok(WrittenPath::Bare(text.to_string())) };
        let error = |kind| PathError { path: text.to_string(), kind };
        let root = match root {
            "self" => {
                return held_at("", path).map(WrittenPath::Own).map_err(|outside| error(outside_at(outside, "self")))
            }
            "std" => Root::Std,
            "doc" => Root::Doc,
            "cwd" => Root::Cwd,
            "https" => Root::Https,
            root => match root.strip_prefix(EXT_PREFIX) {
                Some(name) if !name.is_empty() => Root::Ext(name.to_string()),
                _ if is_drive(root) => {
                    let drive = text.strip_prefix('\\').unwrap_or(text).replace('\\', "/");
                    return Err(error(PathErrorKind::DriveLetter(format!("/{drive}"))));
                }
                _ => return Err(error(PathErrorKind::UnknownRoot(root.to_string()))),
            },
        };
        QualifiedPath::new(root, path).map(WrittenPath::Qualified)
    }

    /// The path read at `place`, the place of the text it is written in: a
    /// qualified path as it is, `self:` under the root of the place, a bare
    /// path joined to the folder of the place's file
    /// ([module documentation](self#the-place-of-a-text)).
    ///
    /// # Errors
    ///
    /// [`PathErrorKind::OwnOutsidePackage`] for `self:` at a place under
    /// `doc`, `cwd` or `https`; [`PathErrorKind::Above`] for a bare path whose
    /// `..` climbs above the base folder of the place;
    /// [`PathErrorKind::BesideAddress`] for a bare path at a place under
    /// `https`.
    pub fn at(self, place: &QualifiedPath) -> Result<QualifiedPath, PathError> {
        match self {
            WrittenPath::Qualified(path) => Ok(path),
            WrittenPath::Own(path) if place.root.is_base_folder() => {
                Ok(QualifiedPath { root: place.root.clone(), path })
            }
            WrittenPath::Own(path) => {
                Err(PathError { path: format!("self:{path}"), kind: PathErrorKind::OwnOutsidePackage })
            }
            WrittenPath::Bare(bare) => place.join(&bare),
        }
    }
}

/// The path that a text marked as a path (a setting's value, its default)
/// gives its reader, written at `place`, the place of the text: an address
/// that begins with `http://` or `https://` as written, any other path read at
/// `place` ([`WrittenPath::parse`], [`WrittenPath::at`]) and written
/// `ROOT:PATH`.
///
/// # Errors
///
/// The errors of [`WrittenPath::parse`] and [`WrittenPath::at`].
///
/// # Examples
///
/// ```
/// use flm_ext::paths::{QualifiedPath, Root, setting_path};
///
/// let manifest = QualifiedPath::new(Root::Ext(String::from("mine")), "flmextension.yaml").unwrap();
/// assert_eq!(setting_path("img/logo.png", &manifest).unwrap(), "ext+mine:img/logo.png");
/// assert_eq!(setting_path("doc:logo.png", &manifest).unwrap(), "doc:logo.png");
/// assert_eq!(setting_path("/a/logo.png", &manifest).unwrap(), "doc:/a/logo.png");
/// assert_eq!(setting_path("http://example.org/a.css", &manifest).unwrap(), "http://example.org/a.css");
/// let front_matter = QualifiedPath::new(Root::Doc, "paper.flm").unwrap();
/// assert!(setting_path("self:logo.png", &front_matter).is_err());
/// ```
pub fn setting_path(written: &str, place: &QualifiedPath) -> Result<String, PathError> {
    if written.starts_with("http://") || written.starts_with("https://") {
        return Ok(written.to_string());
    }
    Ok(WrittenPath::parse(written)?.at(place)?.to_string())
}

/// The bare path `bare` written in the file `file` of an extension (the
/// file's path inside the extension's folder), as the `self:` path of the
/// file it names: `./style.css` written in `templates/html/letter.js` is
/// `self:templates/html/style.css`.
///
/// # Errors
///
/// [`PathErrorKind::Above`] for a `..` that climbs above the extension's
/// folder.
pub(crate) fn own_beside(file: &str, bare: &str) -> Result<String, PathError> {
    held_at(file, bare)
        .map(|path| format!("self:{path}"))
        .map_err(|outside| PathError { path: bare.to_string(), kind: outside_at(outside, "self") })
}

/// Whether `root` is a drive letter: one ASCII letter, alone or after `\`.
fn is_drive(root: &str) -> bool {
    let letter = root.strip_prefix('\\').unwrap_or(root);
    letter.len() == 1 && letter.bytes().all(|byte| byte.is_ascii_alphabetic())
}

/// `bare` joined as text to the folder of the file `file` (the text before its
/// last `/`; the folder of `/x` is `/`, that of a name with no `/` the empty
/// path), its `..` kept: the join of a path under `doc` and `cwd`.
fn beside(file: &str, bare: &str) -> String {
    match file.rfind('/') {
        Some(0) => format!("/{bare}"),
        Some(slash) => format!("{}/{bare}", &file[..slash]),
        None => bare.to_string(),
    }
}

/// The condition of a path that does not stay inside the base folder of the
/// root `root`, as written.
fn outside_at(outside: Outside, root: &str) -> PathErrorKind {
    match outside {
        Outside::Absolute => PathErrorKind::NotRelative(root.to_string()),
        Outside::Above => PathErrorKind::Above(root.to_string()),
    }
}

/// A path that has no meaning: the path as written, and why
/// ([`PathErrorKind`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError {
    /// The path as written (a bare path that climbs: the bare path).
    pub path: String,
    /// Why the path has no meaning.
    pub kind: PathErrorKind,
}

/// Why a path has no meaning ([`PathError`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathErrorKind {
    /// The root, written, is none of the six.
    UnknownRoot(String),
    /// The root is a drive letter; the form of the path on Windows
    /// (`/C:/Users/me/logo.png`).
    DriveLetter(String),
    /// The path after the root of a base folder, written (`self`, `ext+NAME`,
    /// `std`), is absolute.
    NotRelative(String),
    /// A `..` climbs above the base folder of the root written (`self`,
    /// `ext+NAME`, `std`).
    Above(String),
    /// A bare path is written in a text whose place is an address on the
    /// network (`https`): no such text is applied, and no path is read beside
    /// an address.
    BesideAddress,
    /// `self:` is written in a text whose place belongs to no extension.
    OwnOutsidePackage,
    /// No `use` entry in effect writes the use name of the root `ext+NAME`;
    /// the use name ([`ExtensionsByName::files`]).
    UnknownExtension(String),
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let path = &self.path;
        match &self.kind {
            PathErrorKind::UnknownRoot(root) => write!(
                f,
                "Unknown root ‘{root}’ of the path ‘{path}’ (the roots are ‘self’, ‘ext+NAME’, ‘std’, ‘doc’, ‘cwd’ \
                 and ‘https’)"
            ),
            PathErrorKind::DriveLetter(form) => {
                write!(f, "Invalid path ‘{path}’: an absolute path on Windows must be written ‘{form}’")
            }
            PathErrorKind::NotRelative(root) => {
                write!(f, "Invalid path ‘{path}’: the path after ‘{root}:’ must be relative")
            }
            PathErrorKind::Above(root) => {
                write!(f, "Invalid path ‘{path}’: its ‘..’ may not climb above the folder of ‘{root}:’")
            }
            PathErrorKind::BesideAddress => {
                write!(f, "Missing root in the path ‘{path}’: a path written in a downloaded file must have a root")
            }
            PathErrorKind::OwnOutsidePackage => {
                write!(f, "Invalid path ‘{path}’: ‘self:’ may be written only in the files of an extension or of ‘std’")
            }
            PathErrorKind::UnknownExtension(name) => write!(
                f,
                "Unknown extension ‘{name}’ in the path ‘{path}’: a ‘use’ entry of the document or of a package it \
                 uses must name it"
            ),
        }
    }
}

impl std::error::Error for PathError {}

/// The shared files of an extension, readable from any thread
/// ([`ExtensionFiles::shared`]).
pub type SharedFiles = Arc<dyn ExtensionFiles + Send + Sync>;

/// The extensions of one preparation by their use names: the root `ext+NAME`.
/// The [loader](crate::loader::ExtensionLoader) registers the shared files of
/// each extension it loads under each use name that reached it
/// ([`ExtensionLoader::with_extensions_by_name`](crate::loader::ExtensionLoader::with_extensions_by_name)).
/// A clone is a handle to the same table, so that a reader made when the
/// preparation starts sees every extension loaded after.
#[derive(Clone, Default)]
pub struct ExtensionsByName {
    table: Arc<Mutex<Table>>,
}

/// The table behind [`ExtensionsByName`].
#[derive(Default)]
struct Table {
    /// The files of each extension, by each use name that reached it.
    files: BTreeMap<String, SharedFiles>,
}

impl ExtensionsByName {
    /// No extension.
    pub fn new() -> ExtensionsByName {
        ExtensionsByName::default()
    }

    /// Registers `files`, the files of an extension, under the use name `name`
    /// that reached it.
    pub(crate) fn register(&self, name: &str, files: SharedFiles) {
        let mut table = self.table.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        table.files.insert(name.to_string(), files);
    }

    /// The files of the extension that the use name `name` found, `None` when no
    /// `use` entry in effect writes `name` (the condition
    /// [`PathErrorKind::UnknownExtension`]).
    pub fn files(&self, name: &str) -> Option<SharedFiles> {
        let table = self.table.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        table.files.get(name).cloned()
    }
}

impl fmt::Debug for ExtensionsByName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let table = self.table.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        f.debug_struct("ExtensionsByName").field("names", &table.files.keys().collect::<Vec<_>>()).finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qualified(text: &str) -> QualifiedPath {
        match WrittenPath::parse(text) {
            Ok(WrittenPath::Qualified(path)) => path,
            other => panic!("{text}: {other:?}"),
        }
    }

    #[test]
    fn the_first_colon_ends_the_root() {
        let path = qualified("ext+@acme/style:a/b.css");
        assert_eq!((path.root(), path.path()), (&Root::Ext(String::from("@acme/style")), "a/b.css"));
        let path = qualified("doc:/C:/a.png");
        assert_eq!((path.root(), path.path()), (&Root::Doc, "/C:/a.png"));
        let path = qualified("https://example.org/a:b");
        assert_eq!((path.root(), path.path()), (&Root::Https, "//example.org/a:b"));
    }

    #[test]
    fn a_bare_path_is_joined_beside_its_place_and_may_not_climb_above_the_base_folder() {
        let place = qualified("ext+x:templates/html/a/page.html");
        let joined = WrittenPath::parse("partials/head.html").unwrap().at(&place).unwrap();
        assert_eq!(joined, qualified("ext+x:templates/html/a/partials/head.html"));
        let climbed = WrittenPath::parse("../../../../logo.png").unwrap().at(&place).unwrap_err();
        assert_eq!(climbed.kind, PathErrorKind::Above(String::from("ext+x")));
    }

    #[test]
    fn an_absolute_path_is_a_path_of_the_document() {
        for text in ["/C:/a.png", "/home/me/a.png", "~/a.png"] {
            let path = qualified(text);
            assert_eq!((path.root(), path.path()), (&Root::Doc, text));
        }
    }

    #[test]
    fn a_one_letter_root_is_a_drive_letter_with_the_absolute_form() {
        let error = WrittenPath::parse("C:\\a.png").unwrap_err();
        assert_eq!(error.kind, PathErrorKind::DriveLetter(String::from("/C:/a.png")));
    }
}
