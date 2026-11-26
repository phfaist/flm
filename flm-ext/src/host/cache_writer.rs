//! The writing of [`CacheDeclarations`] in the table form of
//! [`CacheDeclaration`], every key written: the guest client's package
//! report, which the engine reads as it reads a JavaScript package object.

use serde_json::{json, Map, Value};

use super::{CacheDeclaration, CacheDeclarations, CacheEngine, CacheKind, CacheScope, CacheSort, CacheTime};

/// The package object's `caches` that declares `caches`: a map from each
/// name to its declaration's table form, no default left out.
pub(crate) fn write_caches(caches: &CacheDeclarations) -> Value {
    Value::Object(caches.iter().map(|(name, declaration)| (name.clone(), write_cache(declaration))).collect())
}

/// One cache's declaration in its table form.
fn write_cache(declaration: &CacheDeclaration) -> Value {
    let mut map = Map::new();
    map.insert("version".to_string(), json!(declaration.version.get()));
    match &declaration.kind {
        CacheKind::Values { engine, blobs, sort } => {
            let engine = match engine {
                CacheEngine::Jsonl => "jsonl",
                CacheEngine::Files => "files",
                CacheEngine::Sqlite => "sqlite",
            };
            map.insert("values".to_string(), json!({ "engine": engine, "blobs": blobs }));
            match sort {
                CacheSort::Stable { unused } => {
                    map.insert("unused_s".to_string(), write_time(*unused));
                }
                CacheSort::Volatile { lifetime_s, expiry } => {
                    map.insert("lifetime_s".to_string(), json!(lifetime_s));
                    map.insert(
                        "expiry".to_string(),
                        json!({
                            "stale_percent": expiry.stale_percent.get(),
                            "jitter_percent": expiry.jitter_percent.get(),
                            "grace_s": write_time(expiry.grace),
                        }),
                    );
                }
            }
        }
        CacheKind::Folder { unused } => {
            map.insert("folder".to_string(), json!({}));
            map.insert("unused_s".to_string(), write_time(*unused));
        }
    }
    map.insert("cost_ms".to_string(), json!(declaration.cost_ms.get()));
    let scope = match declaration.scope {
        CacheScope::Shared => "shared",
        CacheScope::Document => "document",
    };
    map.insert("scope".to_string(), json!(scope));
    Value::Object(map)
}

/// A time: the number of seconds, or `unlimited`.
fn write_time(time: CacheTime) -> Value {
    match time {
        CacheTime::Seconds(seconds) => json!(seconds),
        CacheTime::Unlimited => json!("unlimited"),
    }
}
