//! The relative-path arithmetic over an extension's files: joining a
//! specifier to the importing file's folder, normalizing `.` and `..`, and
//! refusing a path that climbs above the extension's root, is absolute, or
//! holds `\`.
//!
//! The [`ExtensionFiles`](crate::source::ExtensionFiles) of a source receive
//! normalized relative paths only. The callers word the messages, since only
//! they know what a path was for.

use std::fmt;

/// Why a relative path is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathErrorKind {
    /// It begins with `/` or `\` (or a drive letter and `:`).
    Absolute,
    /// It climbs above the extension's root.
    LeavesRoot,
    /// It holds `\` further along than its start: a path inside an extension
    /// uses `/` as its separator, on every system, so that every source reads
    /// it alike.
    Backslash,
}

/// A path refused by [`normalize`] or [`join`]: the path as given and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathError {
    /// The path as given (for [`join`], the specifier).
    pub path: String,
    /// Why it is refused.
    pub kind: PathErrorKind,
}

impl fmt::Display for PathError {
    /// "the path `P` must be relative", "the path `P` leaves the
    /// extension's folder" or "the path `P` must use `/` as its separator,
    /// not `\`".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            PathErrorKind::Absolute => write!(f, "the path `{}` must be relative", self.path),
            PathErrorKind::LeavesRoot => write!(f, "the path `{}` leaves the extension's folder", self.path),
            PathErrorKind::Backslash => write!(f, "the path `{}` must use `/` as its separator, not `\\`", self.path),
        }
    }
}

impl std::error::Error for PathError {}

/// `path` relative to the extension's root, with `.` and `..` segments
/// resolved and empty segments dropped (`./src/../main.js` → `main.js`).
///
/// # Errors
///
/// [`PathErrorKind::Absolute`] for a path beginning with `/`, `\` or a drive
/// letter and `:`; [`PathErrorKind::Backslash`] for a path holding `\`
/// anywhere else; [`PathErrorKind::LeavesRoot`] for a `..` that climbs above
/// the root.
pub fn normalize(path: &str) -> Result<String, PathError> {
    let error = |kind| PathError { path: path.to_string(), kind };
    let bytes = path.as_bytes();
    let drive = bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':';
    if path.starts_with('/') || path.starts_with('\\') || drive {
        return Err(error(PathErrorKind::Absolute));
    }
    if path.contains('\\') {
        return Err(error(PathErrorKind::Backslash));
    }
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if segments.pop().is_none() {
                    return Err(error(PathErrorKind::LeavesRoot));
                }
            }
            segment => segments.push(segment),
        }
    }
    Ok(segments.join("/"))
}

/// `specifier` (`./helpers.js`, `../x.js`) joined to the folder of `importer`
/// (a normalized path) and normalized.
///
/// # Errors
///
/// As [`normalize`], the error naming `specifier`.
pub fn join(importer: &str, specifier: &str) -> Result<String, PathError> {
    if normalize(specifier).is_err_and(|error| error.kind == PathErrorKind::Absolute) {
        return Err(PathError { path: specifier.to_string(), kind: PathErrorKind::Absolute });
    }
    let folder = parent(importer);
    let joined = if folder.is_empty() { specifier.to_string() } else { format!("{folder}/{specifier}") };
    normalize(&joined).map_err(|error| PathError { path: specifier.to_string(), kind: error.kind })
}

/// The folder of a normalized path (`src` for `src/main.js`, the empty text
/// for `main.js`).
pub fn parent(path: &str) -> &str {
    path.rsplit_once('/').map_or("", |(folder, _)| folder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_specifier_joins_the_importer_s_folder_and_normalizes() {
        assert_eq!(normalize("./src/../main.js"), Ok("main.js".to_string()));
        assert_eq!(normalize("templates//html/./x.yaml"), Ok("templates/html/x.yaml".to_string()));
        assert_eq!(normalize(""), Ok(String::new()));
        assert_eq!(join("main.js", "./helpers.js"), Ok("helpers.js".to_string()));
        assert_eq!(join("src/main.js", "./helpers.js"), Ok("src/helpers.js".to_string()));
        assert_eq!(join("src/main.js", "../lib/x.js"), Ok("lib/x.js".to_string()));
        assert_eq!(join("src/a/main.js", "../../x.js"), Ok("x.js".to_string()));
        assert_eq!(parent("src/main.js"), "src");
        assert_eq!(parent("main.js"), "");
    }

    #[test]
    fn an_absolute_path_and_a_climb_are_refused() {
        for path in ["/etc/passwd", "\\x", "C:/x", "c:x"] {
            assert_eq!(normalize(path), Err(PathError { path: path.to_string(), kind: PathErrorKind::Absolute }), "{path}");
        }
        assert_eq!(normalize("../x"), Err(PathError { path: "../x".to_string(), kind: PathErrorKind::LeavesRoot }));
        assert_eq!(normalize("a/../../x").map_err(|e| e.kind), Err(PathErrorKind::LeavesRoot));
        assert_eq!(
            join("src/main.js", "../../x.js"),
            Err(PathError { path: "../../x.js".to_string(), kind: PathErrorKind::LeavesRoot })
        );
        assert_eq!(join("src/main.js", "/x.js").map_err(|e| e.kind), Err(PathErrorKind::Absolute));
        assert_eq!(
            PathError { path: "../x".to_string(), kind: PathErrorKind::LeavesRoot }.to_string(),
            "the path `../x` leaves the extension's folder"
        );
    }

    #[test]
    fn a_backslash_anywhere_but_the_start_is_refused() {
        for path in ["a\\b.js", "src\\", "./x\\..\\y", "a/b\\c"] {
            assert_eq!(normalize(path), Err(PathError { path: path.to_string(), kind: PathErrorKind::Backslash }), "{path}");
        }
        assert_eq!(
            join("src/main.js", "./a\\b.js"),
            Err(PathError { path: "./a\\b.js".to_string(), kind: PathErrorKind::Backslash })
        );
        assert_eq!(
            normalize("a\\b").unwrap_err().to_string(),
            "the path `a\\b` must use `/` as its separator, not `\\`"
        );
    }
}
