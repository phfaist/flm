//! The metadata written as data: how a program that holds the author's metadata as a
//! map of data values (for example the root keys of a front matter) makes the
//! document's [`DocMeta`] from it, with [`DocMeta::from_data`].
//!
//! The form is the same for every program:
//!
//! | Key | Its value |
//! |---|---|
//! | [`TITLE_KEY`] (`title`) | one text |
//! | [`AUTHOR_KEY`] (`author`) | one author, or a list of authors |
//! | [`DATE_KEY`] (`date`) | one text |
//! | any other key | any data, kept in [`DocMeta::other`] |
//!
//! An author ([`MetaAuthor`]) is a text, the author's name, or a map:
//!
//! | Key | Its value |
//! |---|---|
//! | [`NAME_KEY`] (`name`) | one text; required |
//! | [`AFFILIATION_KEY`] (`affiliation`) | one text, or a list of texts |
//! | [`EMAIL_KEY`] (`email`) | one text, a plain address |
//! | [`NOTE_KEY`] (`note`) | one text, or a list of texts |
//! | any other key | any data, kept in [`MetaAuthor::other`] |
//!
//! ```yaml
//! author:
//!   - name: 'Ada Lovelace'
//!     affiliation: ['University of X', 'Institute of Y']
//!     email: 'ada@example.com'
//!     note: 'Corresponding author'
//!   - 'Grace Hopper'
//! ```
//!
//! Where a text is expected, a number or a truth value is read as the text it is
//! written as (`date: 2026` is the text `2026`). A value of another shape, and an
//! author's map without `name`, are errors ([`MetaDataError`]). The texts are FLM,
//! parsed when the document is processed, except the email address, which is not
//! parsed. [`metadata_schema`] describes the same form as a JSON Schema.

use std::fmt;

use serde_json::{Map, Value};

use super::{DocMeta, MetaAuthor, MetaPlace, MetaText};

/// The key of the title, `title`: one text.
pub const TITLE_KEY: &str = "title";

/// The key of the authors, `author`: one author, or a list of authors, each a text
/// (the author's name) or a map (see the [module documentation](self)).
pub const AUTHOR_KEY: &str = "author";

/// The key of the date, `date`: one text, as the author writes the date.
pub const DATE_KEY: &str = "date";

/// The key of an author's name in an author's map, `name`: one text, required.
pub const NAME_KEY: &str = "name";

/// The key of an author's affiliations in an author's map, `affiliation`: one text,
/// or a list of texts.
pub const AFFILIATION_KEY: &str = "affiliation";

/// The key of an author's email address in an author's map, `email`: one text, a
/// plain address.
pub const EMAIL_KEY: &str = "email";

/// The key of the notes about an author in an author's map, `note`: one text, or a
/// list of texts.
pub const NOTE_KEY: &str = "note";

/// Returns the JSON Schema of the metadata written as data, in the form the
/// [module documentation](self) states: a map in which [`TITLE_KEY`] and
/// [`DATE_KEY`] hold texts, [`AUTHOR_KEY`] one author or a list of authors, and any
/// other key any value. The schema refers to no definition of its own. A program
/// uses it to describe the files in which authors write metadata, for example to
/// an editor that checks them.
pub fn metadata_schema() -> Value {
    let text = || serde_json::json!({ "type": ["string", "number", "boolean"] });
    let texts = |description: &str| {
        serde_json::json!({
            "description": description,
            "anyOf": [text(), { "type": "array", "items": text() }],
        })
    };
    let entry = serde_json::json!({
        "anyOf": [
            text(),
            {
                "type": "object",
                "properties": {
                    NAME_KEY: {
                        "description": "The author's name, a text in FLM.",
                        "type": ["string", "number", "boolean"],
                    },
                    AFFILIATION_KEY: texts("The author's affiliations: one text in FLM, or a list of texts."),
                    EMAIL_KEY: {
                        "description": "The author's email address, a plain text, not FLM.",
                        "type": ["string", "number", "boolean"],
                    },
                    NOTE_KEY: texts("The notes about the author: one text in FLM, or a list of texts."),
                },
                "required": [NAME_KEY],
            },
        ],
    });
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
                "description": "The authors of the document: one author, or a list of authors. An author is \
                    a text in FLM, the author's name, or a map with the author's `name` and, as the author \
                    wishes, `affiliation`, `email` and `note`.",
                "anyOf": [entry.clone(), { "type": "array", "items": entry }],
            },
            DATE_KEY: {
                "description": "The date of the document, a text in FLM, as the author writes it.",
                "type": ["string", "number", "boolean"],
            },
        },
    })
}

/// Where a value is in the metadata's data: its key, then the steps from the key's
/// value down to the value ([`MetaStep`]). It is displayed as the key and the steps
/// joined by `.`: `author.1` for the second author of a list, `author.0.affiliation`
/// for the affiliations of the first, and the key alone for the key's own value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MetaPath {
    /// The key.
    pub key: String,
    /// The steps from the key's value down to the value, outermost first; none for
    /// the key's own value.
    pub steps: Vec<MetaStep>,
}

impl MetaPath {
    /// Returns the path of the value of `key`.
    pub fn of(key: impl Into<String>) -> MetaPath {
        MetaPath { key: key.into(), steps: Vec::new() }
    }

    /// Returns the path one step further down, at `step`.
    pub fn then(&self, step: MetaStep) -> MetaPath {
        let mut path = self.clone();
        path.steps.push(step);
        path
    }
}

/// One step of a [`MetaPath`] into a value: an item of a list, or the value of a
/// key of a map.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MetaStep {
    /// The item at this position in a list, counted from 0.
    Index(usize),
    /// The value of this key in a map.
    Key(String),
}

impl fmt::Display for MetaPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.key)?;
        for step in &self.steps {
            match step {
                MetaStep::Index(index) => write!(f, ".{index}")?,
                MetaStep::Key(key) => write!(f, ".{key}")?,
            }
        }
        Ok(())
    }
}

/// A value of the metadata's data that is not of the form the [module
/// documentation](self) states: where it is, and what is wrong. It is displayed
/// as its [`detail`](MetaDataError::detail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetaDataError {
    /// The value at fault: for a missing key, the map that lacks it.
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
    /// A [`MetaDataError`], with its path, for each value of `title`, `author` and
    /// `date`, or inside `author`, that is not of the form stated. An author's map
    /// whose `name` is missing or is not a text gives one error, and its other keys
    /// are not read: an error in its `affiliation`, `email` or `note` is reported
    /// only once its `name` is a text.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::stdlib::features::metadata::{DocMeta, MetaPlace};
    ///
    /// let data = serde_json::json!({
    ///     "title": "On \\emph{flexible} markup",
    ///     "author": [{"name": "Ada", "affiliation": "University of X"}, "Grace"],
    ///     "date": 2026,
    ///     "keywords": ["markup"],
    /// });
    /// let meta = DocMeta::from_data(data.as_object().unwrap(), |path| {
    ///     Some(MetaPlace::new("paper.flm", if path.key == "title" { 2 } else { 3 }, 8))
    /// })
    /// .unwrap();
    /// assert_eq!(meta.title.unwrap().place.unwrap().line, 2);
    /// assert_eq!(meta.authors.len(), 2);
    /// assert_eq!(meta.authors[0].affiliations[0].text, "University of X");
    /// assert_eq!(meta.date.unwrap().text, "2026");
    /// assert!(meta.other.contains_key("keywords"));
    /// ```
    pub fn from_data(
        data: &Map<String, Value>,
        place_of: impl Fn(&MetaPath) -> Option<MetaPlace>,
    ) -> Result<DocMeta, Vec<MetaDataError>> {
        let reader = Reader { place_of: &place_of };
        let mut meta = DocMeta::default();
        let mut errors = Vec::new();
        for (key, value) in data {
            let path = MetaPath::of(key.as_str());
            match key.as_str() {
                TITLE_KEY => meta.title = reader.text(&path, value, &mut errors),
                DATE_KEY => meta.date = reader.text(&path, value, &mut errors),
                AUTHOR_KEY => meta.authors = reader.authors(&path, value, &mut errors),
                _ => {
                    meta.other.insert(key.clone(), value.clone());
                }
            }
        }
        if errors.is_empty() { Ok(meta) } else { Err(errors) }
    }
}

/// Reads the values of the metadata's data, each text with its place, and collects
/// the errors.
struct Reader<'p> {
    place_of: &'p dyn Fn(&MetaPath) -> Option<MetaPlace>,
}

impl Reader<'_> {
    /// Returns the text `value` at `path`: a text, or a number or a truth value
    /// read as the text it is written as; `None`, with the error, for a value of
    /// another shape.
    fn text(&self, path: &MetaPath, value: &Value, errors: &mut Vec<MetaDataError>) -> Option<MetaText> {
        let text = match value {
            Value::String(text) => text.clone(),
            Value::Number(number) => number.to_string(),
            Value::Bool(value) => value.to_string(),
            _ => {
                errors.push(MetaDataError {
                    path: path.clone(),
                    detail: format!("‘{path}’ must be a text, a number or a truth value"),
                });
                return None;
            }
        };
        let text = MetaText::new(text);
        Some(match (self.place_of)(path) {
            Some(place) => text.at(place),
            None => text,
        })
    }

    /// Returns the texts of `value` at `path`: one text, or a list of texts.
    fn texts(&self, path: &MetaPath, value: &Value, errors: &mut Vec<MetaDataError>) -> Vec<MetaText> {
        match value {
            Value::Array(items) => items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| self.text(&path.then(MetaStep::Index(index)), item, errors))
                .collect(),
            Value::Object(_) | Value::Null => {
                errors.push(MetaDataError {
                    path: path.clone(),
                    detail: format!("‘{path}’ must be a text or a list of texts"),
                });
                Vec::new()
            }
            _ => self.text(path, value, errors).into_iter().collect(),
        }
    }

    /// Returns the authors of `value`, the value of `author`: one author, or a list
    /// of authors.
    fn authors(&self, path: &MetaPath, value: &Value, errors: &mut Vec<MetaDataError>) -> Vec<MetaAuthor> {
        match value {
            Value::Array(items) => items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| self.author(&path.then(MetaStep::Index(index)), item, errors))
                .collect(),
            Value::Null => {
                errors.push(MetaDataError {
                    path: path.clone(),
                    detail: format!("‘{path}’ must be a name, a map with the key ‘{NAME_KEY}’, or a list of these"),
                });
                Vec::new()
            }
            _ => self.author(path, value, errors).into_iter().collect(),
        }
    }

    /// Returns the author `value` at `path`: a text, the author's name, or a map
    /// holding `name` and, as the author wishes, `affiliation`, `email`, `note` and
    /// other keys.
    fn author(&self, path: &MetaPath, value: &Value, errors: &mut Vec<MetaDataError>) -> Option<MetaAuthor> {
        let entries = match value {
            Value::Object(entries) => entries,
            Value::Array(_) | Value::Null => {
                errors.push(MetaDataError {
                    path: path.clone(),
                    detail: format!("‘{path}’ must be a name or a map with the key ‘{NAME_KEY}’"),
                });
                return None;
            }
            _ => return self.text(path, value, errors).map(MetaAuthor::new),
        };
        let at = |key: &str| path.then(MetaStep::Key(key.to_string()));
        let Some(name) = entries.get(NAME_KEY) else {
            errors.push(MetaDataError { path: path.clone(), detail: format!("Missing key ‘{NAME_KEY}’ in ‘{path}’") });
            return None;
        };
        let mut author = MetaAuthor::new(self.text(&at(NAME_KEY), name, errors)?);
        for (key, value) in entries {
            match key.as_str() {
                NAME_KEY => {}
                AFFILIATION_KEY => author.affiliations = self.texts(&at(key), value, errors),
                EMAIL_KEY => author.email = self.text(&at(key), value, errors),
                NOTE_KEY => author.notes = self.texts(&at(key), value, errors),
                _ => {
                    author.other.insert(key.clone(), value.clone());
                }
            }
        }
        Some(author)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read(data: Value) -> Result<DocMeta, Vec<MetaDataError>> {
        DocMeta::from_data(data.as_object().unwrap(), |_| None)
    }

    #[test]
    fn an_author_is_a_text_a_list_of_texts_or_a_list_with_maps() {
        let meta = read(serde_json::json!({"author": "Ada", "date": 2026, "draft": true})).unwrap();
        assert_eq!(meta.authors, [MetaAuthor::new(MetaText::new("Ada"))]);
        assert_eq!(meta.date, Some(MetaText::new("2026")));
        assert_eq!(meta.other["draft"], Value::Bool(true));
        let meta = read(serde_json::json!({"author": ["Ada", "Grace"]})).unwrap();
        assert_eq!(meta.authors.len(), 2);
        let meta = read(serde_json::json!({"author": [
            {"name": "Ada", "affiliation": ["U. of X", "I. of Y"], "email": "ada@example.com",
             "note": "Corresponding author", "orcid": "0000"},
            "Grace",
        ]}))
        .unwrap();
        let ada = MetaAuthor::new(MetaText::new("Ada"))
            .affiliation(MetaText::new("U. of X"))
            .affiliation(MetaText::new("I. of Y"))
            .email(MetaText::new("ada@example.com"))
            .note(MetaText::new("Corresponding author"))
            .other("orcid", serde_json::json!("0000"));
        assert_eq!(meta.authors, [ada, MetaAuthor::new(MetaText::new("Grace"))]);
    }

    #[test]
    fn a_value_of_another_shape_and_an_author_without_a_name_are_refused_at_their_paths() {
        let errors = read(serde_json::json!({
            "title": ["a"],
            "author": ["Ada", {"x": 1}, {"name": "Grace", "affiliation": {"y": 2}}],
        }))
        .unwrap_err();
        let paths: Vec<String> = errors.iter().map(|error| error.path.to_string()).collect();
        assert_eq!(paths.len(), 3, "{paths:?}");
        assert!(paths.contains(&String::from("title")));
        assert!(paths.contains(&String::from("author.1")));
        assert!(paths.contains(&String::from("author.2.affiliation")));
    }
}
