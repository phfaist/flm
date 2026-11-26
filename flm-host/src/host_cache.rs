//! The host service `cache`: the values caches that an extension's package declares, read
//! and written by its handlers.
//!
//! A package declares the caches its handlers use (the key `caches`,
//! [`flm_ext::host::CacheDeclaration`]) beside the host services they call (the key `host`);
//! a handler calls this service as `cx.host("cache").call(method, args)`. The loader of
//! every preparation offers it ([`packages::loader`](crate::packages::loader)). When a
//! package that declares `cache` loads, the service checks each cache the package declares
//! alone, and refuses the package when cachette refuses one
//! ([`ExtensionCache::check_package`]).
//!
//! # The three methods
//!
//! Every method takes `cache`, the name of a values cache that the caller's package
//! declares; a name that it does not declare, or that declares a folder, is refused.
//!
//! | Method | Arguments beside `cache` | Answer |
//! |---|---|---|
//! | `get` | `key`, and optionally `blobs` (a list of blob names) | `null`, or `{ "record", "blobs", "freshness", "refresh" }` |
//! | `put` | `key`, `record`, and optionally `blobs`, `cost_ms`, `lifetime_s` | `null` |
//! | `remove` | `key` | `null` |
//!
//! - **A key is any JSON value.** The value is stored under the SHA-256, in lowercase
//!   hexadecimal digits, of the key's JSON text with the members of each object sorted by
//!   name: two keys that differ only in the order of an object's members are one key. A
//!   handler therefore gives everything its value depends on as the key, whatever its size.
//! - `record` is any JSON value. `blobs` of `put` is a map from a blob's name to
//!   `{ "text": text }` or `{ "base64": text }`; a blob's name has 1 to 64 characters among
//!   the lowercase ASCII letters, the digits, `-`, `_` and `.`, and does not begin with `.`.
//!   `get` answers, in its own `blobs`, the blobs that its argument names and that the
//!   value has, read in one step with the record, each as `{ "text" }` when its bytes are
//!   UTF-8 text and `{ "base64" }` otherwise; without the argument it answers none (`{}`).
//!   Base64 is written in the standard alphabet with padding. These are the forms of the
//!   files of the host service `programs`. Both forms stand for the blob's bytes, so a
//!   handler reads a blob in either form, whichever form it was stored in.
//! - **A volatile value.** `freshness` is `"fresh"`, `"stale"` or `"expired"`, as cachette
//!   computes it with the manager's clock, and `null` for a stable value; `refresh` says
//!   whether the handler should produce the value again and store it (`false` for a stable
//!   value). A read is never refused because of a value's age: a handler that cannot
//!   produce the value again uses the kept one. `lifetime_s` of `put`, a whole number of
//!   seconds, gives one volatile value a lifetime other than the cache's.
//! - `cost_ms` of `put`, a whole number of milliseconds from 1, is the estimate of the time
//!   needed to produce the value again; without it, the cost that the cache declares.
//!
//! The cache's name, the instance of the extension (and of the document, for the scope
//! `document`) and the engine are those of [`ExtensionCache`]. The service opens the
//! session through the handle of the registry's services ([`ServiceCaches`]), so one session
//! serves every call of one build.
//!
//! # Failures
//!
//! An argument of the wrong shape is refused. Nothing else is: a failure of the cache, and
//! what cachette reports about a value (a blob given to a cache declared without blobs, a
//! lifetime given to a stable value), go to the manager's reporter, and the call answers as
//! without a cache: `get` answers `null`, and `put` stores nothing.

use std::sync::Arc;
use std::time::Duration;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64;
use flm_core::progress::Scope;
use flm_ext::host::{HostAnswer, HostCaller, HostService, HostServiceError};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::cache::cachette::{BlobName, BlobSelection, Cost, FoundValue, Freshness, PutOptions, Record, ValuesSession};
use crate::cache::{ExtensionCache, ServiceCaches};

/// The service's name, as a package declares it.
pub const NAME: &str = "cache";
/// The method that reads a value.
pub const GET: &str = "get";
/// The method that stores a value.
pub const PUT: &str = "put";
/// The method that removes a value.
pub const REMOVE: &str = "remove";

/// The host service `cache` over the caches of the registry's services. See the
/// [module documentation](self).
#[derive(Clone, Debug)]
pub struct CacheService {
    /// The handle through which the service opens its sessions: the sessions of the build
    /// under way.
    pub caches: ServiceCaches,
}

impl CacheService {
    /// Returns the service opening its sessions through `caches`.
    pub fn new(caches: ServiceCaches) -> CacheService {
        CacheService { caches }
    }

    /// The session of the cache that `args` names, among the values caches of the package
    /// of `caller`; `None` when the handle cannot open it (a configuration that cachette
    /// refuses), for a call answered as without a cache.
    fn session(&self, caller: &HostCaller, args: &Value) -> Result<Option<Arc<ValuesSession>>, HostServiceError> {
        let name = match args.get("cache") {
            None => return Err(missing("cache")),
            Some(value) => value.as_str().ok_or_else(|| wrong("cache", "a text"))?,
        };
        let (declaration, instance) = ExtensionCache::values(caller, name)?;
        Ok(self.caches.open_values(&declaration, &instance).ok())
    }
}

impl HostService for CacheService {
    /// `cache`.
    fn name(&self) -> &str {
        NAME
    }

    /// `get`, `put` or `remove` on a values cache of the caller's package.
    fn call(&self, caller: &HostCaller, method: &str, args: &Value, _progress: &Scope) -> Result<HostAnswer, HostServiceError> {
        if ![GET, PUT, REMOVE].contains(&method) {
            return Err(refused(format!(
                "the method of the host service ‘cache’ must be ‘get’, ‘put’ or ‘remove’, not ‘{method}’"
            )));
        }
        let session = self.session(caller, args)?;
        let key = key_digest(args.get("key").ok_or_else(|| missing("key"))?);
        let answer = match method {
            GET => get(session.as_deref(), &key, args)?,
            PUT => {
                put(session.as_deref(), &key, args)?;
                Value::Null
            }
            _ => {
                if let Some(session) = &session {
                    session.remove(&key);
                }
                Value::Null
            }
        };
        Ok(HostAnswer::Data(answer))
    }

    /// Checks each cache the package declares alone ([`ExtensionCache::check_package`]).
    fn check_package(&self, caller: &HostCaller) -> Result<(), HostServiceError> {
        ExtensionCache::check_package(caller)
    }
}

/// `get { cache, key, blobs }`: the value under `key`, or `null`.
fn get(session: Option<&ValuesSession>, key: &str, args: &Value) -> Result<Value, HostServiceError> {
    let names: Vec<BlobName> = match args.get("blobs") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| item.as_str().ok_or_else(|| wrong("blobs", "a list of blob names")).and_then(blob_name))
            .collect::<Result<_, _>>()?,
        Some(_) => return Err(wrong("blobs", "a list of blob names")),
    };
    let Some(session) = session else { return Ok(Value::Null) };
    let (value, bytes) = if names.is_empty() {
        match session.get(key) {
            Some(value) => (value, Vec::new()),
            None => return Ok(Value::Null),
        }
    } else {
        match session.get_with_blobs(key, BlobSelection::Named(&names)) {
            Some(found) => (found.value, found.bytes),
            None => return Ok(Value::Null),
        }
    };
    let FoundValue { record, freshness, refresh, .. } = value;
    let Ok(record) = record.deserialize::<Value>() else { return Ok(Value::Null) };
    let blobs: Map<String, Value> =
        bytes.into_iter().map(|(name, bytes)| (name.as_str().to_string(), content_value(bytes))).collect();
    let freshness = match freshness {
        None => Value::Null,
        Some(Freshness::Fresh) => Value::from("fresh"),
        Some(Freshness::Stale) => Value::from("stale"),
        Some(Freshness::Expired) => Value::from("expired"),
    };
    Ok(serde_json::json!({ "record": record, "blobs": blobs, "freshness": freshness, "refresh": refresh }))
}

/// `put { cache, key, record, blobs, cost_ms, lifetime_s }`: stores the value under `key`.
fn put(session: Option<&ValuesSession>, key: &str, args: &Value) -> Result<(), HostServiceError> {
    let record = args.get("record").ok_or_else(|| missing("record"))?;
    let record = Record::from_serialize(record).map_err(|error| refused(error.message().to_string()))?;
    let blobs = match args.get("blobs") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Object(map)) => map
            .iter()
            .map(|(name, content)| Ok((blob_name(name)?, blob_content(name, content)?)))
            .collect::<Result<_, HostServiceError>>()?,
        Some(_) => return Err(wrong("blobs", "a map from a blob name to ‘{ text }’ or ‘{ base64 }’")),
    };
    let cost = match args.get("cost_ms") {
        None | Some(Value::Null) => None,
        Some(value) => Some(
            value
                .as_u64()
                .and_then(Cost::new)
                .ok_or_else(|| wrong("cost_ms", &format!("a whole number from 1 to {}", i64::MAX)))?,
        ),
    };
    let lifetime = match args.get("lifetime_s") {
        None | Some(Value::Null) => None,
        Some(value) => Some(Duration::from_secs(value.as_u64().ok_or_else(|| wrong("lifetime_s", "a whole number"))?)),
    };
    if let Some(session) = session {
        session.put(key, record, blobs, PutOptions { cost, lifetime });
    }
    Ok(())
}

/// The blob name `name`, or the refusal of a name that cachette does not accept.
fn blob_name(name: &str) -> Result<BlobName, HostServiceError> {
    BlobName::new(name).map_err(|_| {
        refused(format!(
            "invalid blob name ‘{name}’: blob names must have 1 to {} characters among the lowercase ASCII letters, \
             the digits, ‘-’, ‘_’ and ‘.’, and may not begin with ‘.’",
            BlobName::MAX_LEN
        ))
    })
}

/// The bytes of the blob `name`, written `content`.
fn blob_content(name: &str, content: &Value) -> Result<Vec<u8>, HostServiceError> {
    content_bytes(content).map_err(|shape| match shape {
        ContentShape::NotAForm => refused(format!("the blob ‘{name}’ must be ‘{{ text }}’ or ‘{{ base64 }}’")),
        ContentShape::InvalidBase64 => {
            refused(format!("‘base64’ of the blob ‘{name}’ must be base64 in the standard alphabet, with padding"))
        }
    })
}

/// Why a content written as data is refused ([`content_bytes`]).
pub(crate) enum ContentShape {
    /// The value is neither `{ text }` nor `{ base64 }`.
    NotAForm,
    /// The text of `{ base64 }` is not base64 in the standard alphabet, with padding.
    InvalidBase64,
}

/// The bytes of the content `content`, written `{ text }` or `{ base64 }` in the standard
/// alphabet with padding. The host services `cache` and `programs` take the content of a
/// blob and of a file in this form.
pub(crate) fn content_bytes(content: &Value) -> Result<Vec<u8>, ContentShape> {
    let Some(map) = content.as_object().filter(|map| map.len() == 1) else { return Err(ContentShape::NotAForm) };
    match (map.get("text"), map.get("base64")) {
        (Some(Value::String(text)), None) => Ok(text.clone().into_bytes()),
        (None, Some(Value::String(encoded))) => BASE64.decode(encoded).map_err(|_| ContentShape::InvalidBase64),
        _ => Err(ContentShape::NotAForm),
    }
}

/// The content `bytes` as data: `{ text }` when it is UTF-8, `{ base64 }` otherwise, in the
/// standard alphabet with padding. The host services `cache`, `programs` and `download`
/// answer the content of a blob, of a file and of a downloaded file in this form.
pub(crate) fn content_value(bytes: Vec<u8>) -> Value {
    let mut map = Map::new();
    match String::from_utf8(bytes) {
        Ok(text) => map.insert(String::from("text"), Value::String(text)),
        Err(error) => map.insert(String::from("base64"), Value::String(BASE64.encode(error.as_bytes()))),
    };
    Value::Object(map)
}

/// The text under which the service stores the value of `key`: the SHA-256, in lowercase
/// hexadecimal digits, of the key's compact JSON text with the members of each object
/// sorted by name (by the code points of the names).
fn key_digest(key: &Value) -> String {
    let mut text = String::new();
    canonical(key, &mut text);
    Sha256::digest(text.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Writes the compact JSON text of `value` into `text`, the members of each object sorted
/// by name.
fn canonical(value: &Value, text: &mut String) {
    match value {
        Value::Array(items) => {
            text.push('[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    text.push(',');
                }
                canonical(item, text);
            }
            text.push(']');
        }
        Value::Object(map) => {
            let mut members: Vec<(&String, &Value)> = map.iter().collect();
            members.sort_by(|a, b| a.0.cmp(b.0));
            text.push('{');
            for (index, (name, member)) in members.into_iter().enumerate() {
                if index > 0 {
                    text.push(',');
                }
                text.push_str(&Value::String(name.clone()).to_string());
                text.push(':');
                canonical(member, text);
            }
            text.push('}');
        }
        scalar => text.push_str(&scalar.to_string()),
    }
}

/// A refusal with the text `detail`.
fn refused(detail: String) -> HostServiceError {
    HostServiceError { detail }
}

/// The refusal of a call without the argument `key`.
fn missing(key: &str) -> HostServiceError {
    refused(format!("missing argument ‘{key}’"))
}

/// The refusal of the argument `key`, which must be `what`.
fn wrong(key: &str, what: &str) -> HostServiceError {
    refused(format!("‘{key}’ must be {what}"))
}
