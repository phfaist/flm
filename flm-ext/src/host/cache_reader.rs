//! The reading of a package object's `caches` into [`CacheDeclarations`],
//! every broken rule found: the one reading of the table form of
//! [`CacheDeclaration`] for every engine.

use std::num::{NonZeroU32, NonZeroU64};

use serde_json::Value;

use super::{
    CacheDeclaration, CacheDeclarations, CacheEngine, CacheExpiry, CacheKind, CachePercent, CacheScope, CacheSort, CacheTime,
};
use crate::binding::packages::joined;

/// Reads the package object's `caches` (`null` when absent) of the package
/// `package` of the extension `stem`; each problem a text in `problems`.
pub(crate) fn read_caches(stem: &str, package: &str, caches: &Value, problems: &mut Vec<String>) -> CacheDeclarations {
    let mut declarations = CacheDeclarations::new();
    let map = match caches {
        Value::Null => return declarations,
        Value::Object(map) => map,
        _ => {
            problems.push(format!(
                "‘caches’ of the package ‘{package}’ of the extension ‘{stem}’ must be a map from a cache's name to \
                 its declaration"
            ));
            return declarations;
        }
    };
    for (name, value) in map {
        let of = format!("of the package ‘{package}’ of the extension ‘{stem}’");
        if !is_cache_name(name) {
            problems.push(format!(
                "invalid cache name ‘{name}’ {of}: cache names must be segments of lowercase ASCII letters, digits, \
                 ‘-’ and ‘_’ joined by ‘.’"
            ));
            continue;
        }
        let mut rules = Vec::new();
        let declaration = read_cache(value, &mut rules);
        problems.extend(rules.iter().map(|rule| format!("invalid cache ‘{name}’ {of}: {rule}")));
        if let Some(declaration) = declaration.filter(|_| rules.is_empty()) {
            declarations.insert(name.clone(), declaration);
        }
    }
    declarations
}

/// Says whether `name` is segments of lowercase ASCII letters, digits, `-`
/// and `_` joined by `.`.
fn is_cache_name(name: &str) -> bool {
    let segment = |segment: &str| {
        !segment.is_empty() && segment.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
    };
    name.split('.').all(segment)
}

/// The keys of a cache's declaration.
const CACHE_KEYS: [&str; 8] = ["version", "values", "folder", "unused_s", "lifetime_s", "expiry", "cost_ms", "scope"];

/// One cache's declaration, read; each broken rule a text in `rules`.
fn read_cache(value: &Value, rules: &mut Vec<String>) -> Option<CacheDeclaration> {
    let Value::Object(map) = value else {
        rules.push(format!("a cache's declaration must be a map of {}", listed(&CACHE_KEYS)));
        return None;
    };
    let get = |key: &str| map.get(key).filter(|value| !value.is_null());
    for key in map.keys().filter(|key| !CACHE_KEYS.contains(&key.as_str())) {
        rules.push(format!("‘{key}’ is not a key of a cache's declaration (the keys are {})", listed(&CACHE_KEYS)));
    }
    let version = match get("version") {
        None => {
            rules.push("missing ‘version’".to_string());
            None
        }
        Some(value) => {
            let version = value.as_u64().and_then(|version| u32::try_from(version).ok()).and_then(NonZeroU32::new);
            if version.is_none() {
                rules.push("‘version’ must be a whole number, at least 1".to_string());
            }
            version
        }
    };
    let cost_ms = match get("cost_ms") {
        None => {
            rules.push("missing ‘cost_ms’".to_string());
            None
        }
        Some(value) => {
            let cost = value.as_u64().and_then(NonZeroU64::new);
            if cost.is_none() {
                rules.push("‘cost_ms’ must be a whole number of milliseconds, at least 1".to_string());
            }
            cost
        }
    };
    let scope = match get("scope") {
        None => {
            rules.push("missing ‘scope’".to_string());
            None
        }
        Some(value) => match value.as_str() {
            Some("shared") => Some(CacheScope::Shared),
            Some("document") => Some(CacheScope::Document),
            _ => {
                rules.push("‘scope’ must be ‘shared’ or ‘document’".to_string());
                None
            }
        },
    };
    let unused = get("unused_s").map(|value| {
        let time = read_time(value);
        if time.is_none() {
            rules.push("‘unused_s’ must be a whole number of seconds or ‘unlimited’".to_string());
        }
        time
    });
    let lifetime = get("lifetime_s").map(|value| {
        let lifetime = value.as_u64();
        if lifetime.is_none() {
            rules.push("‘lifetime_s’ must be a whole number of seconds".to_string());
        }
        lifetime
    });
    let expiry = get("expiry").map(|value| read_expiry(value, rules));
    if unused.is_some() == lifetime.is_some() {
        rules.push("a cache must have exactly one of ‘unused_s’ and ‘lifetime_s’".to_string());
    }
    if expiry.is_some() && lifetime.is_none() {
        rules.push("‘expiry’ may be given only with ‘lifetime_s’".to_string());
    }
    let kind = match (get("values"), get("folder")) {
        (Some(values), None) => {
            let (engine, blobs) = read_values(values, rules);
            let sort = match (unused, lifetime) {
                (Some(Some(unused)), None) => Some(CacheSort::Stable { unused }),
                (None, Some(Some(lifetime_s))) => Some(CacheSort::Volatile {
                    lifetime_s,
                    expiry: expiry.flatten().unwrap_or_default(),
                }),
                _ => None,
            };
            Some(CacheKind::Values { engine: engine?, blobs: blobs?, sort: sort? })
        }
        (None, Some(folder)) => {
            if !folder.as_object().is_some_and(serde_json::Map::is_empty) {
                rules.push("‘folder’ must be an empty map".to_string());
            }
            if lifetime.is_some() {
                rules.push("‘lifetime_s’ may not be given for a folder cache; use ‘unused_s’".to_string());
            }
            Some(CacheKind::Folder { unused: unused.flatten()? })
        }
        _ => {
            rules.push("a cache must have exactly one of ‘values’ and ‘folder’".to_string());
            None
        }
    };
    Some(CacheDeclaration { version: version?, kind: kind?, cost_ms: cost_ms?, scope: scope? })
}

/// A cache's `values`: its engine and whether its values have blobs, each
/// `None` when wrong; each broken rule a text in `rules`.
fn read_values(values: &Value, rules: &mut Vec<String>) -> (Option<CacheEngine>, Option<bool>) {
    const KEYS: [&str; 2] = ["engine", "blobs"];
    let Value::Object(map) = values else {
        rules.push(format!("‘values’ must be a map of {}", listed(&KEYS)));
        return (None, None);
    };
    for key in map.keys().filter(|key| !KEYS.contains(&key.as_str())) {
        rules.push(format!("‘values.{key}’ is not a key of ‘values’ (the keys are {})", listed(&KEYS)));
    }
    let engine = match map.get("engine").filter(|value| !value.is_null()).map(Value::as_str) {
        None => Some(CacheEngine::Sqlite),
        Some(Some("jsonl")) => Some(CacheEngine::Jsonl),
        Some(Some("files")) => Some(CacheEngine::Files),
        Some(Some("sqlite")) => Some(CacheEngine::Sqlite),
        Some(_) => {
            rules.push("‘values.engine’ must be ‘jsonl’, ‘files’ or ‘sqlite’".to_string());
            None
        }
    };
    let blobs = match map.get("blobs").filter(|value| !value.is_null()) {
        None => Some(false),
        Some(Value::Bool(blobs)) => Some(*blobs),
        Some(_) => {
            rules.push("‘values.blobs’ must be ‘true’ or ‘false’".to_string());
            None
        }
    };
    (engine, blobs)
}

/// A cache's `expiry`, `None` when wrong; each broken rule a text in `rules`.
fn read_expiry(expiry: &Value, rules: &mut Vec<String>) -> Option<CacheExpiry> {
    const KEYS: [&str; 3] = ["stale_percent", "jitter_percent", "grace_s"];
    let Value::Object(map) = expiry else {
        rules.push(format!("‘expiry’ must be a map of {}", listed(&KEYS)));
        return None;
    };
    for key in map.keys().filter(|key| !KEYS.contains(&key.as_str())) {
        rules.push(format!("‘expiry.{key}’ is not a key of ‘expiry’ (the keys are {})", listed(&KEYS)));
    }
    let default = CacheExpiry::default();
    let get = |key: &str| map.get(key).filter(|value| !value.is_null());
    let mut percent = |key: &str, default: CachePercent| match get(key) {
        None => Some(default),
        Some(value) => {
            let percent = value.as_u64().and_then(|percent| u8::try_from(percent).ok()).and_then(CachePercent::new);
            if percent.is_none() {
                rules.push(format!("‘expiry.{key}’ must be a whole number from 0 to 100"));
            }
            percent
        }
    };
    let stale_percent = percent("stale_percent", default.stale_percent);
    let jitter_percent = percent("jitter_percent", default.jitter_percent);
    let grace = match get("grace_s") {
        None => Some(default.grace),
        Some(value) => {
            let grace = read_time(value);
            if grace.is_none() {
                rules.push("‘expiry.grace_s’ must be a whole number of seconds or ‘unlimited’".to_string());
            }
            grace
        }
    };
    Some(CacheExpiry { stale_percent: stale_percent?, jitter_percent: jitter_percent?, grace: grace? })
}

/// A whole number of seconds or `unlimited`; `None` for any other value.
fn read_time(value: &Value) -> Option<CacheTime> {
    match value {
        Value::String(text) if text == "unlimited" => Some(CacheTime::Unlimited),
        _ => value.as_u64().map(CacheTime::Seconds),
    }
}

/// The keys `keys`, quoted, joined by commas and a final `and`.
fn listed(keys: &[&str]) -> String {
    joined(&keys.iter().map(|key| format!("‘{key}’")).collect::<Vec<_>>())
}
