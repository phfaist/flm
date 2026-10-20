//! Conversion between JSON text and techy's serialized values and wire
//! segments.
//!
//! A techy serialized value ([`SerialValue`]) has at most one JSON form (a value
//! holding a map key that begins with `$` has none), and two values have the same
//! JSON text exactly when they are equal. Comparing two
//! [projections](crate::dump::project) as values and comparing their JSON text
//! therefore always agree, so a [golden file](crate::dump::golden) can hold a
//! projection as readable JSON that a person may edit, without weakening the
//! comparison.
//!
//! A wire segment ([`Segment`]) is one unit of techy's serialized stream, as a
//! [serialization session](crate::dump::session) emits it. Its JSON form is
//! written on one line, so that a stream is one segment per line; `flm dump
//! --view wire` prints it that way.
//!
//! [`from_serde`] converts any value `serde` can serialize, such as a
//! `serde_json::json!` literal, into a serialized value, which is how a test
//! writes an expected value.

use serde::Serialize;
use techy::serialize::{Segment, SerialValue, SerialValueError};

/// The error type of the JSON functions of this module: `serde_json`'s error.
///
/// Writing JSON fails for a value holding a map key that begins with `$`, a
/// prefix techy reserves; reading JSON fails for text that is not the JSON form
/// of a serialized value.
pub type JsonError = serde_json::Error;

/// Writes a serialized value as indented JSON: two spaces per level, no
/// trailing newline.
///
/// This is the form a JSON golden file holds (followed by a newline) and the
/// form a failing comparison prints.
///
/// # Errors
///
/// [`JsonError`] if the value holds a map key beginning with `$`.
///
/// # Examples
///
/// ```
/// use flm_core::dump::to_json_pretty;
/// use flm_core::techy::serialize::SerialValue;
///
/// let value = SerialValue::List(vec![SerialValue::Int(1), SerialValue::Str("a".into())]);
/// assert_eq!(to_json_pretty(&value)?, "[\n  1,\n  \"a\"\n]");
/// # Ok::<(), flm_core::dump::JsonError>(())
/// ```
pub fn to_json_pretty(value: &SerialValue) -> Result<String, JsonError> {
    serde_json::to_string_pretty(value)
}

/// Writes a serialized value as JSON on one line, with no space between the
/// tokens.
///
/// # Errors
///
/// [`JsonError`] if the value holds a map key beginning with `$`.
pub fn to_json(value: &SerialValue) -> Result<String, JsonError> {
    serde_json::to_string(value)
}

/// Reads a serialized value from JSON text.
///
/// # Errors
///
/// [`JsonError`] if the text is not JSON; if it holds a floating-point number
/// or an integer outside the range of `i64` (serialized values have neither);
/// if it holds an object key beginning with `$` other than the two keys techy
/// reserves for bytes and table references (`$bytes`, `$index`); or if it nests more lists and
/// maps than [`SerialValue::MAX_NESTING_DEPTH`] allows.
pub fn from_json(text: &str) -> Result<SerialValue, JsonError> {
    serde_json::from_str(text)
}

/// Converts any value that `serde` can serialize into a techy serialized value.
///
/// This is how a test's `serde_json::json!({ ... })` literal becomes the
/// expected value of a comparison.
///
/// # Errors
///
/// [`SerialValueError`] if the value holds a floating-point number or an integer
/// outside the range of `i64`, which serialized values cannot represent; if it
/// holds a map key that is not a string or that begins with `$` (struct field
/// names and the names of enum variants with data count as map keys); or if its
/// own `Serialize` implementation reports an error.
pub fn from_serde<T: Serialize + ?Sized>(value: &T) -> Result<SerialValue, SerialValueError> {
    techy::serialize::to_value(value)
}

/// Writes a wire segment as one line of JSON, with no line break inside it.
///
/// A stream written this way holds one segment per line; `flm dump --view
/// wire` prints this form. [`segment_from_line`] reads it back.
///
/// # Errors
///
/// [`JsonError`] if the segment holds a map key beginning with `$`.
pub fn segment_line(segment: &Segment) -> Result<String, JsonError> {
    serde_json::to_string(segment)
}

/// Reads a wire segment from one line of JSON written by [`segment_line`].
///
/// # Errors
///
/// [`JsonError`] if the line is not a JSON rendering of a segment.
pub fn segment_from_line(line: &str) -> Result<Segment, JsonError> {
    serde_json::from_str(line)
}
