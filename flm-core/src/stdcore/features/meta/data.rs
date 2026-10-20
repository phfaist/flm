//! The metadata written as data: how a program that holds the author's metadata as a
//! map of data values (for example the root keys of a front matter) makes the
//! document's [`DocMeta`] from it, with [`DocMeta::from_data`].
//!
//! The form is the same for every program:
//!
//! | Key | Its value |
//! |---|---|
//! | [`TITLE_KEY`] (`title`) | one text |
//! | [`AUTHOR_KEY`] (`author`) | one text, or a list of texts, one per author |
//! | [`DATE_KEY`] (`date`) | one text |
//! | any other key | any data, kept in [`DocMeta::other`] |
//!
//! Where a text is expected, a number or a truth value is read as the text it is
//! written as (`date: 2026` is the text `2026`); any other value is an error
//! ([`MetaDataError`]). The texts are FLM, parsed when the document is processed.
//! [`metadata_schema`] describes the same form as a JSON Schema.

use std::fmt;

use serde_json::{Map, Value};

use super::{DocMeta, MetaPlace, MetaText};

/// The key of the title, `title`: one text.
pub const TITLE_KEY: &str = "title";

/// The key of the authors, `author`: one text, or a list of texts, one per
/// author.
pub const AUTHOR_KEY: &str = "author";

/// The key of the date, `date`: one text, as the author writes the date.
pub const DATE_KEY: &str = "date";

/// Returns the JSON Schema of the metadata written as data, in the form the
/// [module documentation](self) states: a map in which [`TITLE_KEY`],
/// [`AUTHOR_KEY`] and [`DATE_KEY`] hold texts, and any other key any value. A
/// program uses it to describe the files in which authors write metadata, for
/// example to an editor that checks them.
pub fn metadata_schema() -> Value {
    let text = serde_json::json!({ "type": ["string", "number", "boolean"] });
    serde_json::json!({
        "type": "object",
        "description": "The metadata of the document: its title, its authors, its date, and any other key, \
            whose value is kept as written.",
        "properties": {
            TITLE_KEY: {
                "description": "The title of the document, a text in FLM.",
                "type": ["string", "number", "boolean"],
            },
            AUTHOR_KEY: {
                "description": "The authors of the document: one text in FLM, or a list of texts, one per author.",
                "anyOf": [text, { "type": "array", "items": text }],
            },
            DATE_KEY: {
                "description": "The date of the document, a text in FLM, as the author writes it.",
                "type": ["string", "number", "boolean"],
            },
        },
    })
}

/// Where a value is in the metadata's data: its key and, for an item of a list,
/// its position in the list the key holds (counted from 0). It is displayed as
/// `author.1` for the second author, and as the key alone otherwise.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MetaPath {
    /// The key.
    pub key: String,
    /// The position in the key's list, for an item of a list.
    pub index: Option<usize>,
}

impl fmt::Display for MetaPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.index {
            Some(index) => write!(f, "{}.{index}", self.key),
            None => f.write_str(&self.key),
        }
    }
}

/// A value of the metadata's data that is not of the form the [module
/// documentation](self) states: where it is, and what is wrong. It is displayed
/// as its [`detail`](MetaDataError::detail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaDataError {
    /// The value at fault.
    pub path: MetaPath,
    /// The problem, as a sentence for a person to read.
    pub detail: String,
}

impl fmt::Display for MetaDataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for MetaDataError {}

impl DocMeta {
    /// Returns the metadata written as `data`, a map from a key to its value, in
    /// the form the [module documentation](self) states.
    ///
    /// `place_of` returns where each value is written, when the caller knows it;
    /// each text then carries its place, so that a problem in it is reported
    /// there.
    ///
    /// # Errors
    ///
    /// Every value of `title`, `author` (or of one of its items) and `date`
    /// that is not a text, a number or a truth value, each with its path
    /// ([`MetaDataError`]).
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdcore::features::meta::{DocMeta, MetaPlace};
    ///
    /// let data = serde_json::json!({
    ///     "title": "On \\emph{flexible} markup",
    ///     "author": ["Ada", "Grace"],
    ///     "date": 2026,
    ///     "keywords": ["markup"],
    /// });
    /// let meta = DocMeta::from_data(data.as_object().unwrap(), |path| {
    ///     Some(MetaPlace::new("paper.flm", if path.key == "title" { 2 } else { 3 }, 8))
    /// })
    /// .unwrap();
    /// assert_eq!(meta.title.unwrap().place.unwrap().line, 2);
    /// assert_eq!(meta.authors.len(), 2);
    /// assert_eq!(meta.date.unwrap().text, "2026");
    /// assert!(meta.other.contains_key("keywords"));
    /// ```
    pub fn from_data(
        data: &Map<String, Value>,
        place_of: impl Fn(&MetaPath) -> Option<MetaPlace>,
    ) -> Result<DocMeta, Vec<MetaDataError>> {
        let text = |path: MetaPath, value: &Value| -> Result<MetaText, MetaDataError> {
            let text = match value {
                Value::String(text) => text.clone(),
                Value::Number(number) => number.to_string(),
                Value::Bool(value) => value.to_string(),
                _ => {
                    let detail = format!(
                        "the value of `{path}` must be a text, a number or a truth value (an item of \
                         a list is written `key.N`, counting from 0)"
                    );
                    return Err(MetaDataError { path, detail });
                }
            };
            let place = place_of(&path);
            let text = MetaText::new(text);
            Ok(match place {
                Some(place) => text.at(place),
                None => text,
            })
        };
        let whole = |key: &str| MetaPath { key: key.to_string(), index: None };
        let mut meta = DocMeta::default();
        let mut errors = Vec::new();
        for (key, value) in data {
            let read = match key.as_str() {
                TITLE_KEY => text(whole(key), value).map(|title| meta.title = Some(title)),
                DATE_KEY => text(whole(key), value).map(|date| meta.date = Some(date)),
                AUTHOR_KEY => match value {
                    Value::Array(items) => {
                        for (index, item) in items.iter().enumerate() {
                            let path = MetaPath { key: key.clone(), index: Some(index) };
                            match text(path, item) {
                                Ok(author) => meta.authors.push(author),
                                Err(error) => errors.push(error),
                            }
                        }
                        Ok(())
                    }
                    _ => text(whole(key), value).map(|author| meta.authors = vec![author]),
                },
                _ => {
                    meta.other.insert(key.clone(), value.clone());
                    Ok(())
                }
            };
            if let Err(error) = read {
                errors.push(error);
            }
        }
        if errors.is_empty() { Ok(meta) } else { Err(errors) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(data: Value) -> Result<DocMeta, Vec<MetaDataError>> {
        DocMeta::from_data(data.as_object().unwrap(), |_| None)
    }

    #[test]
    fn one_author_or_a_list_and_a_number_standing_for_its_text() {
        let meta = read(serde_json::json!({"author": "Ada", "date": 2026, "draft": true})).unwrap();
        assert_eq!(meta.authors, [MetaText::new("Ada")]);
        assert_eq!(meta.date, Some(MetaText::new("2026")));
        assert_eq!(meta.other["draft"], Value::Bool(true));
        let meta = read(serde_json::json!({"author": ["Ada", "Grace"]})).unwrap();
        assert_eq!(meta.authors.len(), 2);
    }

    #[test]
    fn a_value_that_is_not_a_text_is_refused_at_its_path() {
        let errors = read(serde_json::json!({"title": ["a"], "author": ["Ada", {"x": 1}]})).unwrap_err();
        let paths: Vec<String> = errors.iter().map(|error| error.path.to_string()).collect();
        assert_eq!(paths.len(), 2);
        assert!(paths.contains(&String::from("title")));
        assert!(paths.contains(&String::from("author.1")));
    }
}
