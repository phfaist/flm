//! Golden files: compare a test's output with the expected output kept in a
//! file, and rewrite that file on request.
//!
//! A *golden file* holds the output a test expects, and is kept in version
//! control beside the test. The test compares its actual output with the file.
//! When the output changes because the code was meant to change it, run the
//! test with the environment variable [`UPDATE_GOLDEN`] set to exactly `1`: the
//! file is then rewritten with the actual output, and the change is reviewed as
//! an ordinary diff. A golden file that does not exist yet fails the
//! comparison, and the failure message shows the actual output and how to
//! create the file. The caller passes the file's path; by convention a test
//! suite keeps its golden files under `tests/goldens/<suite>/`.
//!
//! There are two kinds of golden file:
//!
//! - **Text** ([`check_text_golden`], [`assert_text_golden`]): compared byte
//!   for byte.
//! - **JSON** ([`check_json_golden`], [`assert_json_golden`]): the expected
//!   value is a techy serialized value, such as a
//!   [projection](crate::dump::project). The file is parsed back into a
//!   serialized value and compared with the actual value, so a file a person
//!   reindented by hand still matches. Two serialized values are equal exactly
//!   when [`to_json`](crate::dump::json::to_json) writes the same text for
//!   them, so the comparison ignores only spacing and indentation.
//!
//! Each kind comes in two forms. The `check_` functions take the
//! [`GoldenMode`] as an argument and return a [`GoldenError`] on failure, so
//! their behavior can be tested without touching the process environment. The
//! `assert_` functions read the mode from [`UPDATE_GOLDEN`] and panic with the
//! error's message; use them in tests.
//!
//! This module is the only part of `flm-core` that reads or writes files;
//! nothing in the processing of a document touches the file system.

use std::fmt;
use std::path::{Path, PathBuf};

use techy::serialize::SerialValue;

use crate::dump::json::{from_json, to_json_pretty};

/// The name of the environment variable that switches the `assert_` functions
/// to update mode: `UPDATE_GOLDEN`.
///
/// Set to exactly `1`, it selects [`GoldenMode::Update`]; any other value, and
/// the variable being unset, select [`GoldenMode::Compare`]
/// ([`GoldenMode::from_env`]).
pub const UPDATE_GOLDEN: &str = "UPDATE_GOLDEN";

/// What a golden check does: compare the actual output with the golden file,
/// or rewrite the file with the actual output.
///
/// The `check_` functions take the mode as an argument; the `assert_` functions
/// read it from the environment ([`GoldenMode::from_env`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GoldenMode {
    /// Compare the actual output with the golden file (the default).
    #[default]
    Compare,
    /// Write the actual output to the golden file, creating the directories
    /// above it; the check then succeeds unless the write fails or, for a JSON
    /// golden file, the actual value has no JSON form.
    Update,
}

impl GoldenMode {
    /// Returns the mode the [`UPDATE_GOLDEN`] environment variable of the
    /// process selects: [`Update`](GoldenMode::Update) when it is set to exactly
    /// `1`, [`Compare`](GoldenMode::Compare) for anything else, including unset,
    /// empty and `true`.
    pub fn from_env() -> GoldenMode {
        GoldenMode::from_setting(std::env::var(UPDATE_GOLDEN).ok().as_deref())
    }

    /// Returns the mode a given value of [`UPDATE_GOLDEN`] selects; `None`
    /// means the variable is unset. [`from_env`](GoldenMode::from_env) is this
    /// function applied to the process environment.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::dump::GoldenMode;
    ///
    /// assert_eq!(GoldenMode::from_setting(Some("1")), GoldenMode::Update);
    /// assert_eq!(GoldenMode::from_setting(Some("true")), GoldenMode::Compare);
    /// assert_eq!(GoldenMode::from_setting(None), GoldenMode::Compare);
    /// ```
    pub fn from_setting(value: Option<&str>) -> GoldenMode {
        match value {
            Some("1") => GoldenMode::Update,
            _ => GoldenMode::Compare,
        }
    }
}

/// The reason a golden check failed, returned by [`check_text_golden`] and
/// [`check_json_golden`].
///
/// Its `Display` text is the message [`assert_text_golden`] and
/// [`assert_json_golden`] panic with. The message of a missing or mismatching
/// file includes the actual output (and the expected output), and says how to
/// rewrite the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GoldenError {
    /// The golden file does not exist. `actual` is the output that update mode
    /// would write to it.
    Missing {
        /// The path of the golden file.
        path: PathBuf,
        /// The actual output.
        actual: String,
    },
    /// The actual output differs from the golden file.
    Mismatch {
        /// The path of the golden file.
        path: PathBuf,
        /// The expected output: the file's text, or for a JSON golden file the
        /// file's value written again as indented JSON.
        expected: String,
        /// The actual output.
        actual: String,
    },
    /// The golden file exists but cannot be read, or a JSON golden file's text
    /// is not the JSON form of a serialized value. [`from_json`] lists every
    /// such case; one of them is valid JSON that nests deeper than
    /// [`SerialValue::MAX_NESTING_DEPTH`].
    Unreadable {
        /// The path of the golden file.
        path: PathBuf,
        /// The underlying error, as text.
        detail: String,
    },
    /// The golden file cannot be written in update mode.
    Unwritable {
        /// The path of the golden file.
        path: PathBuf,
        /// The underlying error, as text.
        detail: String,
    },
    /// The actual value of a JSON comparison has no JSON form: it holds a map
    /// key beginning with `$`, a prefix techy reserves.
    Unrenderable {
        /// The path of the golden file.
        path: PathBuf,
        /// The underlying error, as text.
        detail: String,
    },
}

impl fmt::Display for GoldenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GoldenError::Missing { path, actual } => write!(
                f,
                "the golden file {} does not exist\n\
                 (run the test with {UPDATE_GOLDEN}=1 to create it)\n\
                 --- actual ---\n{actual}",
                path.display()
            ),
            GoldenError::Mismatch { path, expected, actual } => write!(
                f,
                "the actual output differs from the golden file {}\n\
                 (run the test with {UPDATE_GOLDEN}=1 to rewrite the file)\n\
                 --- expected ---\n{expected}\n--- actual ---\n{actual}",
                path.display()
            ),
            GoldenError::Unreadable { path, detail } => {
                write!(f, "the golden file {} cannot be read: {detail}", path.display())
            }
            GoldenError::Unwritable { path, detail } => {
                write!(f, "the golden file {} cannot be written: {detail}", path.display())
            }
            GoldenError::Unrenderable { path, detail } => {
                write!(
                    f,
                    "the actual value compared with the golden file {} cannot be written \
                     as JSON: {detail}",
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for GoldenError {}

/// Writes `content` to `path`, creating the directories above it.
fn write_golden(path: &Path, content: &str) -> Result<(), GoldenError> {
    let unwritable = |error: std::io::Error| GoldenError::Unwritable {
        path: path.to_path_buf(),
        detail: error.to_string(),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(unwritable)?;
    }
    std::fs::write(path, content).map_err(unwritable)
}

/// Reads the golden at `path`: `Ok(None)` when it does not exist.
fn read_golden(path: &Path) -> Result<Option<String>, GoldenError> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(Some(content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => {
            Err(GoldenError::Unreadable { path: path.to_path_buf(), detail: error.to_string() })
        }
    }
}

/// Compares the text `actual` with the golden file at `path`, byte for byte.
///
/// In [`GoldenMode::Update`] the file is rewritten with `actual` instead,
/// creating the directories above it, and the check succeeds.
///
/// # Errors
///
/// [`GoldenError::Missing`] when the file does not exist;
/// [`GoldenError::Mismatch`] when it differs from `actual`;
/// [`GoldenError::Unreadable`] when it cannot be read;
/// [`GoldenError::Unwritable`] when update mode cannot write it.
///
/// See also [`assert_text_golden`], which reads the mode from the environment.
pub fn check_text_golden(path: &Path, actual: &str, mode: GoldenMode) -> Result<(), GoldenError> {
    if mode == GoldenMode::Update {
        return write_golden(path, actual);
    }
    match read_golden(path)? {
        None => {
            Err(GoldenError::Missing { path: path.to_path_buf(), actual: String::from(actual) })
        }
        Some(expected) if expected == actual => Ok(()),
        Some(expected) => Err(GoldenError::Mismatch {
            path: path.to_path_buf(),
            expected,
            actual: String::from(actual),
        }),
    }
}

/// Compares the serialized value `value` with the JSON golden file at `path`.
///
/// The file is parsed back into a serialized value and the two values are
/// compared, so differences of layout in the file do not matter. In
/// [`GoldenMode::Update`] the file is rewritten instead with `value` as written
/// by [`to_json_pretty`] followed by a newline, creating the directories above
/// it, and the check succeeds.
///
/// # Errors
///
/// [`GoldenError::Missing`] when the file does not exist;
/// [`GoldenError::Mismatch`] when its value differs from `value`;
/// [`GoldenError::Unreadable`] when it cannot be read or its text is not the
/// JSON form of a serialized value (the cases [`from_json`] refuses, among them
/// nesting deeper than [`SerialValue::MAX_NESTING_DEPTH`]);
/// [`GoldenError::Unwritable`] when update mode cannot write it;
/// [`GoldenError::Unrenderable`] when `value` holds a map key beginning with
/// `$`, which has no JSON form.
///
/// See also [`assert_json_golden`], which reads the mode from the environment.
pub fn check_json_golden(
    path: &Path,
    value: &SerialValue,
    mode: GoldenMode,
) -> Result<(), GoldenError> {
    let render = |value: &SerialValue| {
        to_json_pretty(value).map(|text| text + "\n").map_err(|error| GoldenError::Unrenderable {
            path: path.to_path_buf(),
            detail: error.to_string(),
        })
    };
    let actual = render(value)?;
    if mode == GoldenMode::Update {
        return write_golden(path, &actual);
    }
    let Some(text) = read_golden(path)? else {
        return Err(GoldenError::Missing { path: path.to_path_buf(), actual });
    };
    let expected = from_json(&text).map_err(|error| GoldenError::Unreadable {
        path: path.to_path_buf(),
        detail: format!("its content is not the JSON form of a serialized value: {error}"),
    })?;
    if &expected == value {
        return Ok(());
    }
    Err(GoldenError::Mismatch { path: path.to_path_buf(), expected: render(&expected)?, actual })
}

/// Asserts that the text `actual` equals the golden file at `path`, or rewrites
/// the file when the environment variable [`UPDATE_GOLDEN`] is `1`.
///
/// This is [`check_text_golden`] with the mode read from the environment
/// ([`GoldenMode::from_env`]).
///
/// # Panics
///
/// Panics with the [`GoldenError`]'s message whenever
/// [`check_text_golden`] returns one: the golden file is missing, differs
/// from `actual`, or cannot be read or written.
pub fn assert_text_golden(path: impl AsRef<Path>, actual: &str) {
    if let Err(error) = check_text_golden(path.as_ref(), actual, GoldenMode::from_env()) {
        panic!("{error}");
    }
}

/// Asserts that the serialized value `value` equals the JSON golden file at
/// `path`, or rewrites the file when the environment variable [`UPDATE_GOLDEN`]
/// is `1`.
///
/// This is [`check_json_golden`] with the mode read from the environment
/// ([`GoldenMode::from_env`]). A projection is usually what is compared:
/// `assert_json_golden(path, &project_tree(tree, &ProjectOptions::new()))`
/// ([`project_tree`](crate::dump::project::project_tree)).
///
/// # Panics
///
/// Panics with the [`GoldenError`]'s message whenever
/// [`check_json_golden`] returns one: the golden file is missing, differs in
/// value from `value`, cannot be read or written, or holds text that is not the
/// JSON form of a serialized value (the cases [`from_json`] refuses); or `value`
/// has no JSON form.
pub fn assert_json_golden(path: impl AsRef<Path>, value: &SerialValue) {
    if let Err(error) = check_json_golden(path.as_ref(), value, GoldenMode::from_env()) {
        panic!("{error}");
    }
}
