//! The header of a JSON output: what kind of output it is, which version of
//! FLM wrote it, and which version of that kind's format it follows.
//!
//! A program that writes data about FLM as JSON (a description of an FLM
//! environment, a stream of diagnostics) opens the output with a [`Meta`]
//! value, so that a reader knows what it reads before it reads the rest. Every
//! JSON output of this kind shares this one type, whichever crate writes it.

use serde::{Deserialize, Serialize};

/// The header that opens a JSON output: its kind, the version of FLM that
/// wrote it, and the version of the kind's format.
///
/// It serializes as a map with three keys, in this order: `kind`, `flm` and
/// `format`. A program writes it first: as the value of a `meta` key that
/// comes first in a single JSON document, or as the first line of a stream of
/// JSON values written one per line.
///
/// # Examples
///
/// ```
/// use flm_core::dump::Meta;
///
/// let meta = Meta::new("diagnostics", 1);
/// let value = serde_json::to_value(&meta).unwrap();
/// assert_eq!(value["kind"], "diagnostics");
/// assert_eq!(value["format"], 1);
/// // The version of FLM that wrote the output, such as `0.5.0`.
/// assert_eq!(value["flm"], meta.flm.as_str());
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Meta {
    /// What the output is, in one lowercase word: for example `diagnostics`
    /// for a stream of diagnostics.
    pub kind: String,
    /// The version of FLM that wrote the output, such as `0.5.0`. Every crate
    /// of FLM has the same version.
    pub flm: String,
    /// The version of the format of outputs of this kind: an integer that the
    /// program writing the kind sets, counted from 1. A reader compares it with
    /// the version it knows how to read.
    pub format: u32,
}

impl Meta {
    /// Returns the header of an output of the kind `kind` (`diagnostics`)
    /// whose format is the version `format` of that kind's format, written by
    /// this version of FLM.
    pub fn new(kind: impl Into<String>, format: u32) -> Meta {
        Meta { kind: kind.into(), flm: String::from(env!("CARGO_PKG_VERSION")), format }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_header_serializes_its_kind_the_version_and_its_format_in_this_order() {
        let meta = Meta::new("info", 2);
        assert_eq!(meta.flm, env!("CARGO_PKG_VERSION"));
        let text = serde_json::to_string(&meta).unwrap();
        assert_eq!(text, format!("{{\"kind\":\"info\",\"flm\":\"{}\",\"format\":2}}", env!("CARGO_PKG_VERSION")));
        let read: Meta = serde_json::from_str(&text).unwrap();
        assert_eq!(read, meta);
    }

    #[test]
    fn the_schema_of_a_header_requires_its_three_keys() {
        let schema = serde_json::to_value(schemars::schema_for!(Meta)).unwrap();
        let required: Vec<&str> =
            schema["required"].as_array().unwrap().iter().filter_map(serde_json::Value::as_str).collect();
        assert_eq!(required, ["kind", "flm", "format"]);
    }
}
