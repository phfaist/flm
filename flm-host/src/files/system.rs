//! Converting a path between its FLM form and the system's.
//!
//! In FLM, a path is text in which `/` separates the names, on every system; every other
//! character, `\` included, belongs to a name, so that `my\folder\file.flm` names one file.
//! A path is absolute when it begins with `/`, or is `~`, or begins with `~/` (the user's
//! home folder, [`home_folder`]). On Windows, an absolute path begins with its drive, as in
//! `/C:/figures/a.png`, or is a network path, as in `//server/share/a.png`. On Linux and
//! macOS, an FLM path is the system's path as is.
//!
//! The host converts a path where it meets the system: [`to_system`] gives the path that a
//! call to the file system takes, and [`from_system`] gives the FLM form of a path that the
//! system gives (a path on the command line or in the environment, a real path).

use std::borrow::Cow;
use std::fmt;
use std::path::{Path, PathBuf};

#[cfg(not(windows))]
use unix as rule;
#[cfg(windows)]
use windows as rule;

/// Returns the system's path for the FLM path `path`.
///
/// `~` stands for the home folder ([`home_folder`]), and `~/x` for the name `x` in the
/// home folder; the home folder is looked up only for such a path. The rest of the
/// conversion depends on the system:
///
/// - on Linux, macOS and every system other than Windows, the text as is;
/// - on Windows, the names are separated with `\`, and an absolute path is written in the
///   system's form: `/C:/x/y` becomes `C:\x\y`, `/C:` alone becomes `C:\` (the root of the
///   drive), and `//server/share/x` becomes `\\server\share\x`. A relative path stays
///   relative: `x/y` becomes `x\y`.
///
/// A `\` of an FLM path is never made a separator. `.` and `..` are kept as written: the
/// system resolves them.
///
/// # Errors
///
/// A [`ToSystemError`] when the path names no file on this system:
///
/// - [`ToSystemError::Character`] on Windows, for a name holding `\` or `:` (a `:` would
///   select a drive, or a stream of a file); the drive itself, and the server and the
///   share of a network path, may hold `:`;
/// - [`ToSystemError::MisplacedDrive`] on Windows, for a name that is a drive (`C:`) or
///   begins with a drive and `\` (`C:\figures`) anywhere but at the start of an absolute
///   path, as in the relative path `C:/figures/a.png`;
/// - [`ToSystemError::NoDrive`] on Windows, for an absolute path that does not begin with a
///   drive and is not a network path, as `/figures/a.png`;
/// - [`ToSystemError::NoHomeFolder`] and [`ToSystemError::HomeFolder`] for `~` or a path
///   that begins with `~/`, when the home folder is not set or has no FLM form
///   ([`home_folder`]).
pub fn to_system(path: &str) -> Result<PathBuf, ToSystemError> {
    let path = expand_home(path, home_folder)?;
    rule::to_system(&path).map(PathBuf::from)
}

/// Returns the FLM form of the system's path `path`.
///
/// - On Linux, macOS and every system other than Windows, the path as is.
/// - On Windows, `\` becomes `/`, and an absolute path is written in FLM's form: `C:\x`
///   becomes `/C:/x`, with the drive letter in upper case; `\\server\share\x` becomes
///   `//server/share/x`; the forms of a real path, `\\?\C:\x` and
///   `\\?\UNC\server\share\x`, become `/C:/x` and `//server/share/x`. A path relative to a
///   drive (`C:x`, the name `x` in the current folder of the drive `C:`, and `\x`, the
///   name `x` at the root of the current drive) is first made absolute by the system's
///   rule ([`std::path::absolute`]), which also resolves its `.` and `..`. A relative path
///   stays relative: `x\y` becomes `x/y`.
///
/// On every system, a path whose first name is `~` is written with `./` before it (`~/x`
/// becomes `./~/x`), so that it names the folder `~` and not the home folder. In every
/// path that is not relative to a drive, nothing else is changed: `.` and `..` are kept as
/// written.
///
/// # Errors
///
/// [`FromSystemError::NotText`] when the path is not valid Unicode text (on Linux and
/// macOS, bytes that are not valid UTF-8), and, on Windows,
/// [`FromSystemError::AbsoluteFailed`] when a path relative to a drive cannot be made
/// absolute.
pub fn from_system(path: &Path) -> Result<String, FromSystemError> {
    let text = text_of(path)?;
    let converted = if rule::is_drive_relative(text) {
        let absolute = std::path::absolute(path)
            .map_err(|error| FromSystemError::AbsoluteFailed { path: path.to_path_buf(), detail: error.to_string() })?;
        rule::from_system(text_of(&absolute)?)
    } else {
        rule::from_system(text)
    };
    Ok(with_literal_tilde(converted))
}

/// Returns `path`, the FLM form of a system's path, with `./` before it when its first
/// name is `~`, which would otherwise stand for the home folder.
fn with_literal_tilde(path: String) -> String {
    if path == "~" || path.starts_with("~/") { format!("./{path}") } else { path }
}

/// Returns the user's home folder, as an FLM path ([`from_system`]): the value of the
/// environment variable `HOME`, or, where `HOME` is not set or empty, the value of
/// `USERPROFILE`; `None` when neither is set (a variable set to the empty text counts as
/// not set). The variables are read anew at each call.
///
/// # Errors
///
/// A [`HomeFolderError`], when the value has no FLM form.
pub fn home_folder() -> Result<Option<String>, HomeFolderError> {
    let set = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty());
    set("HOME").or_else(|| set("USERPROFILE")).map(|home| from_system(Path::new(&home)).map_err(HomeFolderError)).transpose()
}

/// The reason an FLM path names no file on this system: the error of [`to_system`].
///
/// Its `Display` text says what is wrong, for a message that names the path, for example
/// "an absolute path on Windows must begin with its drive, as in ‘/C:/figures/a.png’".
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToSystemError {
    /// On Windows: a name of the path holds `\` or `:`, which no file name may contain.
    Character {
        /// The name, as written in the path.
        name: String,
        /// The first character of the name that a file name may not contain, `\` or `:`.
        character: char,
    },
    /// On Windows: a name of the path is a drive (a letter and `:`, as `C:`) or begins with a
    /// drive and `\` (as `C:\figures`), and is not the first name of an absolute path: the
    /// path names a drive without the `/` that begins an absolute path, as the relative path
    /// `C:/figures/a.png` does.
    MisplacedDrive {
        /// The path from that name on, as written: `C:/figures/a.png` in
        /// `doc/C:/figures/a.png`.
        path: String,
    },
    /// On Windows: the path is absolute (it begins with `/`), its first name is not a drive
    /// (a letter and `:`, as `C:`), and it is not a network path (beginning with `//`).
    NoDrive {
        /// The path, after the home folder replaced a leading `~`.
        path: String,
    },
    /// The path is `~` or begins with `~/`, and the home folder is not set
    /// ([`home_folder`]).
    NoHomeFolder,
    /// The path is `~` or begins with `~/`, and the home folder has no FLM form.
    HomeFolder(HomeFolderError),
}

/// The example of an absolute path on Windows that a message gives, `/C:/figures/a.png`.
const EXAMPLE: &str = "/C:/figures/a.png";

impl fmt::Display for ToSystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ToSystemError::Character { name, character } => {
                write!(f, "a file name on Windows may not contain ‘{character}’ (‘{name}’)")
            }
            ToSystemError::MisplacedDrive { path } => {
                // The path as written, with `/` before it and between its names, when that
                // makes an absolute path that names a file.
                let absolute = format!("/{}", path.replace('\\', "/"));
                let valid = !absolute[3..].contains(':');
                let example = if valid { absolute.as_str() } else { EXAMPLE };
                write!(f, "an absolute path on Windows must begin with ‘/’ and its drive, as in ‘{example}’")
            }
            ToSystemError::NoDrive { path } => {
                // The path with the drive `C:` before it, when that makes a path that names
                // a file.
                let valid = !path.contains([':', '\\']);
                let example = if valid { format!("/C:{path}") } else { String::from(EXAMPLE) };
                write!(f, "an absolute path on Windows must begin with its drive, as in ‘{example}’")
            }
            ToSystemError::NoHomeFolder => {
                write!(f, "the home folder is not set (environment variable ‘HOME’ or ‘USERPROFILE’)")
            }
            ToSystemError::HomeFolder(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ToSystemError {}

/// The reason a system's path has no FLM form: the error of [`from_system`].
///
/// Its `Display` text says what is wrong and names the path, for example "the path
/// ‘/tmp/�.flm’ must be valid Unicode text" (a byte that is not text is shown as `�`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FromSystemError {
    /// The path is not valid Unicode text: on Linux and macOS, its bytes are not valid
    /// UTF-8; on Windows, it holds a lone half of a UTF-16 surrogate pair.
    NotText {
        /// The path.
        path: PathBuf,
    },
    /// On Windows: the path is relative to a drive (`C:x` or `\x`), and the system could not
    /// make it absolute.
    AbsoluteFailed {
        /// The path.
        path: PathBuf,
        /// The operating system's error text.
        detail: String,
    },
}

impl FromSystemError {
    /// Writes what is wrong with the path, named as `what` ("the path", "the home folder").
    fn describe(&self, f: &mut fmt::Formatter<'_>, what: &str) -> fmt::Result {
        match self {
            FromSystemError::NotText { path } => {
                write!(f, "{what} ‘{}’ must be valid Unicode text", path.display())
            }
            FromSystemError::AbsoluteFailed { path, detail } => {
                write!(f, "failed to make {what} ‘{}’ absolute: {detail}", path.display())
            }
        }
    }
}

impl fmt::Display for FromSystemError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.describe(f, "the path")
    }
}

impl std::error::Error for FromSystemError {}

/// The reason the home folder has no FLM form: the error of [`home_folder`].
///
/// Its `Display` text names the home folder, for example "the home folder ‘/home/�’ must be
/// valid Unicode text".
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HomeFolderError(pub FromSystemError);

impl fmt::Display for HomeFolderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.describe(f, "the home folder")
    }
}

impl std::error::Error for HomeFolderError {}

/// Returns the text of the system's path `path`.
fn text_of(path: &Path) -> Result<&str, FromSystemError> {
    path.to_str().ok_or_else(|| FromSystemError::NotText { path: path.to_path_buf() })
}

/// Returns `path` with a leading `~` replaced by the home folder that `home` returns
/// ([`home_folder`], called only for such a path): `~` becomes the home folder, and `~/x`
/// becomes the home folder, then `/` (where the home folder does not end with one), then
/// `x` (`~/` the home folder followed by `/`). Any other path is returned as it is.
fn expand_home(
    path: &str,
    home: impl FnOnce() -> Result<Option<String>, HomeFolderError>,
) -> Result<Cow<'_, str>, ToSystemError> {
    let rest = if path == "~" {
        None
    } else if let Some(rest) = path.strip_prefix("~/") {
        Some(rest)
    } else {
        return Ok(Cow::Borrowed(path));
    };
    let mut expanded = home().map_err(ToSystemError::HomeFolder)?.ok_or(ToSystemError::NoHomeFolder)?;
    if let Some(rest) = rest {
        if !expanded.ends_with('/') {
            expanded.push('/');
        }
        expanded.push_str(rest);
    }
    Ok(Cow::Owned(expanded))
}

/// The rule of Linux, macOS and every system other than Windows: an FLM path is the
/// system's path as is.
#[cfg(any(not(windows), test))]
mod unix {
    use super::ToSystemError;

    /// Returns the system's path for `path`, in which `~` was already replaced: the text as
    /// is.
    pub(super) fn to_system(path: &str) -> Result<String, ToSystemError> {
        Ok(path.to_owned())
    }

    /// Whether `path` is relative to a drive: never.
    pub(super) fn is_drive_relative(path: &str) -> bool {
        let _ = path;
        false
    }

    /// Returns the FLM form of `path`: the text as is.
    pub(super) fn from_system(path: &str) -> String {
        path.to_owned()
    }
}

/// The rule of Windows, written on text so that it can be tested on every system.
#[cfg(any(windows, test))]
mod windows {
    use super::ToSystemError;

    /// Returns the system's path for `path`, in which `~` was already replaced
    /// ([`super::to_system`]).
    pub(super) fn to_system(path: &str) -> Result<String, ToSystemError> {
        if let Some(network) = path.strip_prefix("//") {
            // The server and the share are not names of files: a `:` there is the system's
            // to judge.
            check_names(path, 2, 2)?;
            return Ok(format!(r"\\{}", network.replace('/', r"\")));
        }
        if let Some(rest) = path.strip_prefix('/') {
            let (drive, names) = rest.split_once('/').unwrap_or((rest, ""));
            if !is_drive(drive) {
                return Err(ToSystemError::NoDrive { path: path.to_owned() });
            }
            check_names(path, path.len() - names.len(), 0)?;
            return Ok(format!(r"{drive}\{}", names.replace('/', r"\")));
        }
        check_names(path, 0, 0)?;
        Ok(path.replace('/', r"\"))
    }

    /// Whether `name`, the first name of an absolute FLM path, is a drive: a letter and
    /// `:`, as `C:`.
    fn is_drive(name: &str) -> bool {
        matches!(name.as_bytes(), [letter, b':'] if letter.is_ascii_alphabetic())
    }

    /// Whether `name`, a name that is not the first of an absolute path, names a drive: it is
    /// a drive (`C:`), or begins with a drive and `\` (`C:\figures`).
    fn is_misplaced_drive(name: &str) -> bool {
        matches!(name.as_bytes(), [letter, b':'] | [letter, b':', b'\\', ..] if letter.is_ascii_alphabetic())
    }

    /// Checks the names of `path` from the byte `start` on, names separated with `/`: the
    /// first `named` of them (the server and the share of a network path) with [`check`]
    /// and `:` allowed, every other one for a drive ([`is_misplaced_drive`]), then with
    /// [`check`].
    fn check_names(path: &str, start: usize, named: usize) -> Result<(), ToSystemError> {
        let mut offset = start;
        for (index, name) in path[start..].split('/').enumerate() {
            if index >= named && is_misplaced_drive(name) {
                return Err(ToSystemError::MisplacedDrive { path: path[offset..].to_owned() });
            }
            check(name, index < named)?;
            offset += name.len() + 1;
        }
        Ok(())
    }

    /// Checks that `name` holds no `\`, and, unless `colon_allowed`, no `:`.
    fn check(name: &str, colon_allowed: bool) -> Result<(), ToSystemError> {
        match name.chars().find(|&character| character == '\\' || (character == ':' && !colon_allowed)) {
            Some(character) => Err(ToSystemError::Character { name: name.to_owned(), character }),
            None => Ok(()),
        }
    }

    /// Whether `path` is relative to a drive, and so must be made absolute before
    /// [`from_system`] converts it: a drive letter and `:` followed by no separator (`C:`,
    /// `C:x`), or a single separator first (`\x`, the root of the current drive).
    pub(super) fn is_drive_relative(path: &str) -> bool {
        let separator = |byte: &u8| *byte == b'\\' || *byte == b'/';
        match path.as_bytes() {
            [first, rest @ ..] if separator(first) => !rest.first().is_some_and(separator),
            [letter, b':', rest @ ..] if letter.is_ascii_alphabetic() => !rest.first().is_some_and(separator),
            _ => false,
        }
    }

    /// Returns the FLM form of `path`, which is not relative to a drive
    /// ([`is_drive_relative`]).
    pub(super) fn from_system(path: &str) -> String {
        let path = path.replace('\\', "/");
        if let Some(verbatim) = path.strip_prefix("//?/") {
            if verbatim.get(..4).is_some_and(|unc| unc.eq_ignore_ascii_case("UNC/")) {
                return format!("//{}", &verbatim[4..]);
            }
            return with_drive(verbatim).unwrap_or(path);
        }
        with_drive(&path).unwrap_or(path)
    }

    /// Returns `path`, a path that begins with a drive (`C:` alone or followed by `/`), in
    /// FLM's form: `/`, the drive letter in upper case, then the rest; `None` for a path
    /// that does not begin with a drive.
    fn with_drive(path: &str) -> Option<String> {
        match path.as_bytes() {
            [letter, b':', rest @ ..] if letter.is_ascii_alphabetic() && rest.first().is_none_or(|byte| *byte == b'/') => {
                Some(format!("/{}:{}", letter.to_ascii_uppercase() as char, &path[2..]))
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A home folder as [`home_folder`] would return it.
    fn home(folder: &str) -> impl FnOnce() -> Result<Option<String>, HomeFolderError> + use<> {
        let folder = folder.to_owned();
        move || Ok(Some(folder))
    }

    #[test]
    fn on_windows_an_flm_path_takes_the_systems_form() {
        for (flm, system) in [
            ("/C:/figures/a.png", r"C:\figures\a.png"),
            ("/d:/x/", r"d:\x\"),
            ("/C:", r"C:\"),
            ("/C:/", r"C:\"),
            ("//server/share/a.png", r"\\server\share\a.png"),
            ("//?/C:/x", r"\\?\C:\x"),
            ("figures/a.png", r"figures\a.png"),
            ("../a/./b", r"..\a\.\b"),
            ("~x/a", r"~x\a"),
            ("", ""),
        ] {
            assert_eq!(windows::to_system(flm), Ok(String::from(system)), "{flm}");
        }
    }

    #[test]
    fn on_windows_a_name_holding_a_backslash_or_a_colon_names_no_file() {
        for (flm, name, character) in [
            (r"my\folder\file.flm", r"my\folder\file.flm", '\\'),
            ("doc/a:b.flm", "a:b.flm", ':'),
            ("C:x/a.png", "C:x", ':'),
            (r"/C:/x\y", r"x\y", '\\'),
            ("/C:/a/:", ":", ':'),
            (r"//server/sh\are/x", r"sh\are", '\\'),
            ("//server/share/a:b", "a:b", ':'),
        ] {
            let expected = ToSystemError::Character { name: String::from(name), character };
            assert_eq!(windows::to_system(flm), Err(expected), "{flm}");
        }
    }

    #[test]
    fn on_windows_an_absolute_path_begins_with_its_drive() {
        for flm in ["/figures/a.png", "/", "/C:x/a.png", "/CD:/a.png", "/1:/a.png"] {
            assert_eq!(windows::to_system(flm), Err(ToSystemError::NoDrive { path: String::from(flm) }), "{flm}");
        }
    }

    #[test]
    fn on_windows_a_drive_is_written_only_at_the_start_of_an_absolute_path() {
        for (flm, from) in [
            ("C:/figures/a.png", "C:/figures/a.png"),
            ("c:", "c:"),
            (r"C:\figures\a.png", r"C:\figures\a.png"),
            ("doc/C:/figures/a.png", "C:/figures/a.png"),
            (r"doc/C:\figures/a.png", r"C:\figures/a.png"),
            ("/C:/doc/D:/a.png", "D:/a.png"),
            ("//server/share/C:/a.png", "C:/a.png"),
        ] {
            assert_eq!(windows::to_system(flm), Err(ToSystemError::MisplacedDrive { path: String::from(from) }), "{flm}");
        }
    }

    #[test]
    fn the_example_a_message_gives_names_a_file_on_windows() {
        let errors = ["/figures/a.png", "/", "/C:x/a.png", "/x/a:b", r"C:\figures\a.png", "doc/C:/a:b"]
            .map(|flm| windows::to_system(flm).unwrap_err());
        for error in errors {
            let message = error.to_string();
            let example = message.rsplit('‘').next().and_then(|rest| rest.strip_suffix('’')).unwrap();
            assert!(windows::to_system(example).is_ok(), "{message}");
        }
    }

    #[test]
    fn on_windows_a_systems_path_takes_the_flm_form() {
        for (system, flm) in [
            (r"C:\figures\a.png", "/C:/figures/a.png"),
            (r"c:\x", "/C:/x"),
            (r"C:\", "/C:/"),
            ("C:/x/y", "/C:/x/y"),
            (r"\\server\share\a.png", "//server/share/a.png"),
            (r"\\?\C:\x", "/C:/x"),
            (r"\\?\c:\x", "/C:/x"),
            (r"\\?\UNC\server\share\x", "//server/share/x"),
            (r"\\?\Volume{1}\x", "//?/Volume{1}/x"),
            (r"figures\a.png", "figures/a.png"),
            (r"..\a\.\b", "../a/./b"),
            ("", ""),
        ] {
            assert_eq!(windows::from_system(system), flm, "{system}");
        }
    }

    #[test]
    fn on_windows_only_a_path_relative_to_a_drive_is_made_absolute_first() {
        for system in ["C:", "C:x", r"c:x\y", r"\x", r"\", "/x"] {
            assert!(windows::is_drive_relative(system), "{system}");
        }
        for system in [r"C:\x", "C:/", r"\\server\share", r"\\?\C:\x", "x", "", "CD:x"] {
            assert!(!windows::is_drive_relative(system), "{system}");
        }
    }

    #[test]
    fn on_windows_a_path_survives_the_round_trip() {
        for flm in ["/C:/x/y", "/C:/", "//server/share/x", "x/y", "../x", ""] {
            let system = windows::to_system(flm).unwrap();
            assert_eq!(windows::from_system(&system), flm, "{flm}");
        }
        for system in [r"C:\x\y", r"C:\", r"\\server\share\x", r"x\y"] {
            assert_eq!(windows::to_system(&windows::from_system(system)), Ok(String::from(system)), "{system}");
        }
    }

    #[test]
    fn elsewhere_a_path_is_the_systems_path_as_is() {
        for path in [r"my\folder/a:b.flm", "/C:/x", "//server/x", "C:x", r"\x", "x", ""] {
            assert_eq!(unix::to_system(path), Ok(String::from(path)), "{path}");
            assert!(!unix::is_drive_relative(path), "{path}");
            assert_eq!(unix::from_system(path), path, "{path}");
        }
    }

    #[test]
    fn a_leading_tilde_stands_for_the_home_folder() {
        assert_eq!(expand_home("~", home("/home/me")).unwrap(), "/home/me");
        assert_eq!(expand_home("~/", home("/home/me")).unwrap(), "/home/me/");
        assert_eq!(expand_home("~/a/b", home("/home/me")).unwrap(), "/home/me/a/b");
        assert_eq!(expand_home("~/a", home("/")).unwrap(), "/a");
        for path in ["~x/a", "a/~", "a/~/b", "/~"] {
            assert_eq!(expand_home(path, || Ok(None)).unwrap(), path, "{path}");
        }
        assert_eq!(expand_home("~/a", || Ok(None)), Err(ToSystemError::NoHomeFolder));
        let windows_home = windows::from_system(r"C:\Users\me");
        let expanded = expand_home("~/a/b", home(&windows_home)).unwrap();
        assert_eq!(windows::to_system(&expanded), Ok(String::from(r"C:\Users\me\a\b")));
        let expanded = expand_home("~/a", home(&windows::from_system(r"C:\"))).unwrap();
        assert_eq!(windows::to_system(&expanded), Ok(String::from(r"C:\a")));
    }

    #[test]
    fn a_systems_path_whose_first_name_is_a_tilde_names_that_folder() {
        for (system, flm) in [("~", "./~"), ("~/x", "./~/x"), ("~x/a", "~x/a"), ("a/~", "a/~")] {
            assert_eq!(from_system(Path::new(system)), Ok(String::from(flm)), "{system}");
        }
        // A system's absolute path, in each system's form: on Windows, the system's path
        // `/~/a` would name the folder `~` at the root of the current drive.
        assert_eq!(with_literal_tilde(unix::from_system("/~/a")), "/~/a");
        assert_eq!(with_literal_tilde(windows::from_system(r"C:\~\a")), "/C:/~/a");
        assert_eq!(with_literal_tilde(windows::from_system(r"~\x")), "./~/x");
    }

    #[cfg(unix)]
    #[test]
    fn a_systems_path_that_is_not_utf8_has_no_flm_form() {
        use std::os::unix::ffi::OsStrExt;
        let path = Path::new(std::ffi::OsStr::from_bytes(b"/tmp/\xff.flm"));
        assert_eq!(from_system(path), Err(FromSystemError::NotText { path: path.to_path_buf() }));
        assert_eq!(from_system(Path::new("/tmp/a.flm")), Ok(String::from("/tmp/a.flm")));
    }
}
