//! The requests of the document's settings: `GET` of [`SETTINGS`](super::SETTINGS) (the
//! settings state), of [`SETTINGS_YAML`](super::SETTINGS_YAML) (a value as YAML text) and
//! of [`SETTINGS_SOURCES`](super::SETTINGS_SOURCES) (the sources of the value at a path),
//! and `POST` of [`FRONT_MATTER`](super::FRONT_MATTER) (the changes of one request to the
//! front matter, written together).

use std::fmt;
use std::io::Read;

use serde_json::{Map, Value, json};
use tiny_http::Request;

use super::Shared;
use super::http::{QueryError, query_value, reply};
use crate::seed::read_front_matter;
use crate::watch::front_matter::{Change, Edit, KeyPath};
use crate::watch::settings::{SettingsError, copied_record};

/// The largest body, in bytes, of a request to change the front matter; a longer body is
/// answered with `400`.
const FRONT_MATTER_BODY_LIMIT: u64 = 4 * 1024 * 1024;

/// Answers a request for the settings state ([`DocumentSettings::state`](crate::watch::settings::DocumentSettings::state)):
/// see the [module documentation](super).
pub(super) fn read(shared: &Shared, request: Request) {
    match shared.settings.state() {
        Ok(state) => reply(request, 200, Some("application/json"), None, state.to_string().as_bytes()),
        Err(error) => refuse(request, &error),
    }
}

/// Answers a request for the value at a path of the front matter as YAML text
/// ([`DocumentSettings::yaml_at`](crate::watch::settings::DocumentSettings::yaml_at)), the
/// path given as a JSON Pointer in the query's parameter `path`: see the [module
/// documentation](super).
pub(super) fn yaml(shared: &Shared, request: Request) {
    let path = match query_path(request.url()) {
        Ok(path) => path,
        Err(error) => {
            reply(request, 400, Some("text/plain"), None, error.to_string().as_bytes());
            return;
        }
    };
    match shared.settings.yaml_at(&path) {
        Ok(text) => reply(request, 200, Some("text/plain"), None, text.as_bytes()),
        Err(error) => refuse(request, &error),
    }
}

/// Answers a request for the sources of the value at a path of the front matter
/// ([`DocumentSettings::sources_at`](crate::watch::settings::DocumentSettings::sources_at)),
/// the path given as a JSON Pointer in the query's parameter `path`: see the [module
/// documentation](super).
pub(super) fn sources(shared: &Shared, request: Request) {
    let path = match query_path(request.url()) {
        Ok(path) => path,
        Err(error) => {
            reply(request, 400, Some("text/plain"), None, error.to_string().as_bytes());
            return;
        }
    };
    match shared.settings.sources_at(&path) {
        Ok(answer) => reply(request, 200, Some("application/json"), None, answer.to_string().as_bytes()),
        Err(error) => refuse(request, &error),
    }
}

/// Answers a request to change the front matter
/// ([`DocumentSettings::write`](crate::watch::settings::DocumentSettings::write)), its body
/// a JSON object: see the [module documentation](super).
pub(super) fn write(shared: &Shared, mut request: Request) {
    let mut body = Vec::new();
    let read = request.as_reader().take(FRONT_MATTER_BODY_LIMIT + 1).read_to_end(&mut body);
    let object = read
        .ok()
        .filter(|_| body.len() as u64 <= FRONT_MATTER_BODY_LIMIT)
        .and_then(|_| serde_json::from_slice::<Value>(&body).ok());
    let written = match object {
        Some(Value::Object(object)) => written_edits(&object),
        _ => Err(String::from("Invalid body of the request: the body must be a JSON object (at most 4 MiB)")),
    };
    let (revision, edits) = match written {
        Ok(written) => written,
        Err(reason) => {
            reply(request, 400, Some("text/plain"), None, reason.as_bytes());
            return;
        }
    };
    match shared.settings.write(&revision, &edits) {
        Ok(copied) => {
            let answer = json!({ "copied": copied.iter().map(copied_record).collect::<Vec<_>>() });
            reply(request, 200, Some("application/json"), None, answer.to_string().as_bytes());
        }
        Err(error) => refuse(request, &error),
    }
}

/// The revision and the edits of the body `object` of a request to change the front matter;
/// `Err` gives why the body does not give them, as the response's text.
fn written_edits(object: &Map<String, Value>) -> Result<(String, Vec<Edit>), String> {
    let revision = match object.get("revision") {
        Some(Value::String(revision)) => revision,
        Some(_) => return Err(String::from("Invalid field ‘revision’ in the body of the request: the revision must be a text")),
        None => {
            return Err(String::from(
                "Missing field ‘revision’ in the body of the request: it gives the revision of the front matter that the changes are made against",
            ));
        }
    };
    let changes = match object.get("changes") {
        Some(Value::Array(changes)) => changes,
        Some(_) => return Err(String::from("Invalid field ‘changes’ in the body of the request: the changes must be a list")),
        None => {
            return Err(String::from(
                "Missing field ‘changes’ in the body of the request: it gives the list of the changes to make, each a path and a value or a removal",
            ));
        }
    };
    let edits = changes.iter().enumerate().map(|(index, change)| edit_of(index + 1, change)).collect::<Result<_, _>>()?;
    Ok((revision.clone(), edits))
}

/// The edit that `change`, the entry at the position `position` (from 1) of the field
/// `changes` of a request's body, gives; `Err` gives why it gives none, as the response's
/// text.
fn edit_of(position: usize, change: &Value) -> Result<Edit, String> {
    let invalid = || {
        format!(
            "Invalid change {position} in the field ‘changes’ of the body of the request (counting from 1): a change must hold ‘path’ (a list of keys, texts, at least one) and one of ‘value’ (JSON), ‘yaml’ (a text) and ‘remove: true’"
        )
    };
    let Value::Object(change) = change else { return Err(invalid()) };
    let path = match change.get("path") {
        Some(Value::Array(keys)) => keys.iter().map(|key| key.as_str().map(String::from)).collect::<Option<Vec<_>>>().and_then(KeyPath::new),
        _ => None,
    };
    let Some(path) = path else { return Err(invalid()) };
    let change = match (change.get("value"), change.get("yaml"), change.get("remove")) {
        (Some(value), None, None) => Change::Set(value.clone()),
        (None, Some(Value::String(yaml)), None) => match read_front_matter(yaml, 1) {
            Ok(node) => Change::Set(node.to_json()),
            Err(error) => {
                let place = error.place.map(|place| format!(" at line {}, column {}", place.line, place.column)).unwrap_or_default();
                return Err(format!("Invalid YAML text in the field ‘yaml’ of change {position}{place}: {}", error.yaml_detail()));
            }
        },
        (None, None, Some(Value::Bool(true))) => Change::Remove,
        _ => return Err(invalid()),
    };
    Ok(Edit { path, change })
}

/// Answers `request` with the status and the text of `error`.
fn refuse(request: Request, error: &SettingsError) {
    let status = match error {
        SettingsError::FrontMatter(_) | SettingsError::Unprepared(_) => 409,
        SettingsError::Stale(_) => 412,
        SettingsError::File(_) => 500,
    };
    reply(request, status, Some("text/plain"), None, error.to_string().as_bytes());
}

/// Why the query of a request for a value as YAML, or for the sources of a value, gives no
/// path.
#[derive(Debug, PartialEq)]
enum PathError {
    /// The query has no parameter `path`.
    Missing,
    /// The query gives the parameter `path` more than once.
    Duplicate,
    /// The parameter `path`, as written (percent-decoded where it decodes to text), is not a
    /// JSON Pointer.
    Invalid(String),
}

impl fmt::Display for PathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PathError::Missing => write!(f, "Missing parameter ‘path’ in the query: it gives the path of the value, such as ‘/flm/features’"),
            PathError::Duplicate => write!(f, "Duplicate parameter ‘path’ in the query: a request reads the value at one path"),
            PathError::Invalid(path) => {
                write!(f, "Invalid path ‘{path}’: the path must be a JSON Pointer (RFC 6901), such as ‘/flm/features’")
            }
        }
    }
}

/// The path that the query of `url` gives in its parameter `path`, a JSON Pointer
/// (RFC 6901), percent-encoded: its keys, unescaped (`~1` is `/`, `~0` is `~`).
fn query_path(url: &str) -> Result<Vec<String>, PathError> {
    let pointer = query_value(url, "path").map_err(|error| match error {
        QueryError::Missing => PathError::Missing,
        QueryError::Duplicate => PathError::Duplicate,
        QueryError::NotText(value) => PathError::Invalid(value),
    })?;
    pointer_keys(&pointer).ok_or(PathError::Invalid(pointer))
}

/// The keys of the JSON Pointer `pointer`: none for the empty pointer, else the segments
/// after each `/`, unescaped. `None` for a text that is not a JSON Pointer.
fn pointer_keys(pointer: &str) -> Option<Vec<String>> {
    if pointer.is_empty() {
        return Some(Vec::new());
    }
    pointer.strip_prefix('/')?.split('/').map(unescape).collect()
}

/// The segment `segment` of a JSON Pointer, unescaped; `None` for a `~` that is followed
/// by neither `0` nor `1`.
fn unescape(segment: &str) -> Option<String> {
    let mut key = String::new();
    let mut characters = segment.chars();
    while let Some(character) = characters.next() {
        match character {
            '~' => match characters.next()? {
                '0' => key.push('~'),
                '1' => key.push('/'),
                _ => return None,
            },
            other => key.push(other),
        }
    }
    Some(key)
}
