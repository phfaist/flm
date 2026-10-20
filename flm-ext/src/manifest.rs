//! An extension's manifest: the `extension:` block (description, author, the
//! FLM version required, the engine, the key of the js engine and of the wasm
//! engine) and, for the data engine, the `flm:` block as written.
//!
//! The host parses the manifest's file (`NAME.yaml` or
//! `NAME/flmextension.yaml`) and hands the value to the loader; [`Manifest::read`]
//! checks its shape. A problem names the extension's stem and the key's path;
//! the host places it in the manifest's file.

use std::collections::BTreeMap;
use std::fmt;

use serde_json::{Map, Value};

/// This program's FLM version, which a manifest's `flm_version` is checked
/// against: `flm-ext`'s own crate version (every crate of the workspace
/// shares it).
pub const FLM_VERSION: &str = env!("CARGO_PKG_VERSION");

/// The keys of the `extension:` block, in the order the messages list them.
const EXTENSION_KEYS: &str = "`description`, `author`, `flm_version`, `engine`, `js`, `wasm`";

/// The engines that have a key of their own in the `extension:` block, one row
/// each: the engine, its key, the key that must be in it, and what that key
/// gives (for the message). Each key is refused for another engine; each
/// engine refuses the absence of the key that must be in its block (the block
/// absent, or a map without it), whatever else the block holds.
const ENGINE_KEYS: [(&str, &str, &str, &str); 2] =
    [("js", "js", "main", "the path of its main module"), ("wasm", "wasm", "module", "the path of its module")];

/// The keys of `extension.js.limits`, in the order the messages list them.
const JS_LIMIT_KEYS: [&str; 3] = ["memory_mib", "stack_mib", "time_s"];

/// The keys of `extension.wasm.limits`, in the order the messages list them.
const WASM_LIMIT_KEYS: [&str; 2] = ["memory_mib", "fuel"];

/// An extension's manifest, read from its parsed value: the `extension:`
/// block and, for the data engine, the `flm:` block as written.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest {
    /// `extension.description`: the extension's description.
    pub description: Option<String>,
    /// `extension.author`: kept; nothing reads it yet.
    pub author: Option<String>,
    /// `extension.flm_version` as written; [`read`](Manifest::read) checked
    /// that it is a version requirement in the `semver` crate's syntax.
    pub flm_version: Option<String>,
    /// `extension.engine`; `data` when absent. Checked against the loader's
    /// engines at load, not here.
    pub engine: String,
    /// `extension.js`: the js engine's settings.
    pub js: Option<JsManifest>,
    /// `extension.wasm`: the wasm engine's settings.
    pub wasm: Option<WasmManifest>,
    /// The root key `flm`, as written; read by the data engine
    /// ([`block::read`](crate::block::read)).
    pub flm: Option<Value>,
}

/// `extension.js`: the js engine's settings.
///
/// ```yaml
/// extension:
///   engine: js
///   js:
///     main: main.js
///     limits: { memory_mib: 256, stack_mib: 16, time_s: 60 }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsManifest {
    /// `extension.js.main`: the main module's path relative to the
    /// extension's root, as written.
    pub main: String,
    /// `extension.js.limits`: the limits every engine instance of the
    /// extension is created with, each in place of the host's; all absent
    /// by default.
    pub limits: JsLimits,
}

/// `extension.js.limits`: the limits an extension declares for every engine
/// instance of its own, each optional; an absent one keeps the host's value
/// (`JsSettings` of the `quickjs` binding, which says how each applies).
/// Each is a whole number from 1 up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JsLimits {
    /// `memory_mib`: the memory an engine instance may allocate, in MiB.
    pub memory_mib: Option<u64>,
    /// `stack_mib`: the JavaScript stack an engine instance may use, in MiB;
    /// below [`MAX_STACK_MIB`], the real stack of the threads that run
    /// handlers.
    pub stack_mib: Option<u64>,
    /// `time_s`: the time one handler call may run, in seconds.
    pub time_s: Option<u64>,
}

/// `extension.wasm`: the wasm engine's settings.
///
/// ```yaml
/// extension:
///   engine: wasm
///   wasm:
///     module: wordcount.wasm
///     limits: { memory_mib: 256, fuel: 5000000000 }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WasmManifest {
    /// `extension.wasm.module`: the module's path relative to the extension's
    /// root, as written.
    pub module: String,
    /// `extension.wasm.limits`: the limits every instance of the module is
    /// created with, each in place of the host's; all absent by default.
    pub limits: WasmLimits,
}

/// `extension.wasm.limits`: the limits an extension declares for every
/// instance of its module, each optional; an absent one keeps the host's value
/// (`WasmSettings` of the `wasm` binding, which says how each applies). Each is
/// a whole number from 1 up.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct WasmLimits {
    /// `memory_mib`: the memory an instance may have, in MiB.
    pub memory_mib: Option<u64>,
    /// `fuel`: the fuel one handler call may consume, in wasmi's units (about
    /// one per instruction).
    pub fuel: Option<u64>,
}

/// The bound of `extension.js.limits.stack_mib`, which must be below it: the
/// real stack, in MiB, of the worker threads that run handlers
/// ([`WORKER_STACK_SIZE`](flm_core::lang::guard::WORKER_STACK_SIZE)).
pub const MAX_STACK_MIB: u64 = (flm_core::lang::guard::WORKER_STACK_SIZE >> 20) as u64;

/// A problem in a manifest: the key's path (`extension.engine`) and the
/// condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestError {
    /// The key's path; empty for the root.
    pub path: String,
    /// The condition, as text for a person.
    pub detail: String,
}

impl fmt::Display for ManifestError {
    /// The detail.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for ManifestError {}

impl Manifest {
    /// Reads the manifest of the extension whose stem is `stem` (for the
    /// messages). A null root is an empty data extension; an absent (or null)
    /// `extension:` block is empty.
    ///
    /// # Errors
    ///
    /// Every problem found: a root that is not a map; a key other than
    /// `extension` and `flm`; an `extension` that is not a map, or holds
    /// another key than its six, or a text field that is not a text; a
    /// `flm_version` that is not a version requirement; the engine `js`
    /// without `extension.js.main`, or `wasm` without `extension.wasm.module`;
    /// `extension.js` or `extension.wasm` with another engine, or not a map of
    /// its keys; limits that are not whole numbers from 1 up; `flm` with an
    /// engine other than `data`.
    pub fn read(stem: &str, value: &Value) -> Result<Manifest, Vec<ManifestError>> {
        let mut manifest = Manifest {
            description: None,
            author: None,
            flm_version: None,
            engine: crate::engine::DATA.to_string(),
            js: None,
            wasm: None,
            flm: None,
        };
        let error = |path: &str, detail: String| ManifestError { path: path.to_string(), detail };
        let root = match value {
            Value::Null => return Ok(manifest),
            Value::Object(root) => root,
            _ => {
                return Err(vec![error(
                    "",
                    format!(
                        "the manifest of the extension `{stem}` must be a map holding `extension` and, for the \
                         `data` engine, `flm`"
                    ),
                )]);
            }
        };
        let mut errors = Vec::new();
        let mut extension = &Map::new();
        for (key, value) in root {
            match (key.as_str(), value) {
                ("extension", Value::Object(map)) => extension = map,
                ("extension", Value::Null) => {}
                ("extension", _) => errors.push(error(
                    "extension",
                    format!("`extension` in the manifest of the extension `{stem}` must be a map"),
                )),
                ("flm", value) => manifest.flm = Some(value.clone()),
                (key, _) => errors.push(error(
                    key,
                    format!("`{key}` is not a key of the manifest of the extension `{stem}`; the keys are `extension` and `flm`"),
                )),
            }
        }
        let text = |key: &str, value: &Value, errors: &mut Vec<ManifestError>| match value {
            Value::String(text) => Some(text.clone()),
            _ => {
                errors.push(error(&format!("extension.{key}"), format!("`extension.{key}` of the extension `{stem}` must be a text")));
                None
            }
        };
        let mut engine_keys_written = Vec::new();
        for (key, value) in extension {
            match key.as_str() {
                "description" => manifest.description = text(key, value, &mut errors),
                "author" => manifest.author = text(key, value, &mut errors),
                "engine" => {
                    if let Some(engine) = text(key, value, &mut errors) {
                        manifest.engine = engine;
                    }
                }
                "flm_version" => {
                    if let Some(requirement) = text(key, value, &mut errors) {
                        match semver::VersionReq::parse(&requirement) {
                            Ok(_) => manifest.flm_version = Some(requirement),
                            Err(detail) => errors.push(error(
                                "extension.flm_version",
                                format!(
                                    "`extension.flm_version` of the extension `{stem}` must be a version requirement \
                                     such as `>=0.5` or `^0.5.0`, and `{requirement}` is not one: {detail}"
                                ),
                            )),
                        }
                    }
                }
                "js" => {
                    engine_keys_written.push("js");
                    if let Some((main, limits)) = read_engine_key(stem, ENGINE_KEYS[0], &JS_LIMIT_KEYS, value, &mut errors) {
                        let limits = JsLimits {
                            memory_mib: limits.get("memory_mib").copied(),
                            stack_mib: limits.get("stack_mib").copied(),
                            time_s: limits.get("time_s").copied(),
                        };
                        manifest.js = Some(JsManifest { main, limits });
                    }
                }
                "wasm" => {
                    engine_keys_written.push("wasm");
                    if let Some((module, limits)) = read_engine_key(stem, ENGINE_KEYS[1], &WASM_LIMIT_KEYS, value, &mut errors)
                    {
                        let limits =
                            WasmLimits { memory_mib: limits.get("memory_mib").copied(), fuel: limits.get("fuel").copied() };
                        manifest.wasm = Some(WasmManifest { module, limits });
                    }
                }
                key => errors.push(error(
                    &format!("extension.{key}"),
                    format!(
                        "`extension.{key}` is not a key of the `extension` block of the extension `{stem}`; the keys \
                         are {EXTENSION_KEYS}"
                    ),
                )),
            }
        }
        let engine = manifest.engine.as_str();
        for (owner, key, required, gives) in ENGINE_KEYS {
            // The required key is missing when the block is absent or is a map
            // without it; a block that is not a map, and the key written with
            // another type, have their own messages.
            let missing = match extension.get(key) {
                None => true,
                Some(Value::Object(block)) => !block.contains_key(required),
                Some(_) => false,
            };
            let block = format!("extension.{key}");
            if engine == owner && missing {
                errors.push(error(
                    &format!("{block}.{required}"),
                    format!(
                        "the extension `{stem}` has the engine `{owner}`, so its manifest must give \
                         `{block}.{required}`, {gives}"
                    ),
                ));
            }
            if engine != owner && engine_keys_written.contains(&key) {
                errors.push(error(
                    &block,
                    format!(
                        "the extension `{stem}` writes `{block}`, which is the `{owner}` engine's key, and its engine is \
                         `{engine}`"
                    ),
                ));
            }
        }
        if engine != crate::engine::DATA && manifest.flm.is_some() {
            errors.push(error(
                "flm",
                format!("the extension `{stem}` writes `flm`, which only the `data` engine reads, and its engine is `{engine}`"),
            ));
        }
        if errors.is_empty() { Ok(manifest) } else { Err(errors) }
    }

    /// Whether this program's FLM version ([`FLM_VERSION`]) meets
    /// `flm_version` (true when absent).
    pub fn admits_this_program(&self) -> bool {
        admits(self.flm_version.as_deref(), FLM_VERSION)
    }
}

/// The block of an engine's key (`extension.js`, `extension.wasm`) of the
/// extension `stem`, read from `value` by its row of [`ENGINE_KEYS`] and the
/// keys of its limits `limit_keys`: the required key's text and the limits
/// given. Each problem is pushed on `errors`; `None` when the block is not a
/// map or lacks its required key.
fn read_engine_key(
    stem: &str,
    (_, key, required, _): (&str, &str, &str, &str),
    limit_keys: &[&'static str],
    value: &Value,
    errors: &mut Vec<ManifestError>,
) -> Option<(String, BTreeMap<&'static str, u64>)> {
    let error = |path: &str, detail: String| ManifestError { path: path.to_string(), detail };
    let block = format!("extension.{key}");
    let Value::Object(map) = value else {
        errors.push(error(&block, format!("`{block}` of the extension `{stem}` must be a map")));
        return None;
    };
    let mut found = None;
    let mut limits = BTreeMap::new();
    for (name, value) in map {
        let path = format!("{block}.{name}");
        if name == required {
            match value {
                Value::String(text) => found = Some(text.clone()),
                _ => errors.push(error(&path, format!("`{path}` of the extension `{stem}` must be a text"))),
            }
        } else if name == "limits" {
            limits = read_limits(stem, &path, limit_keys, value, errors);
        } else {
            errors.push(error(
                &path,
                format!(
                    "`{path}` is not a key of `{block}` of the extension `{stem}`; the keys are `{required}` and \
                     `limits`"
                ),
            ));
        }
    }
    found.map(|text| (text, limits))
}

/// The limits at `path` (`extension.js.limits`) of the extension `stem`,
/// read from `value`, whose keys are `keys`: each limit given, by key; each
/// problem pushed on `errors`, and the key it is at left absent. A
/// `stack_mib` must also be below [`MAX_STACK_MIB`].
fn read_limits(
    stem: &str,
    path: &str,
    keys: &[&'static str],
    value: &Value,
    errors: &mut Vec<ManifestError>,
) -> BTreeMap<&'static str, u64> {
    let error = |path: &str, detail: String| ManifestError { path: path.to_string(), detail };
    let listed = keys.iter().map(|key| format!("`{key}`")).collect::<Vec<_>>().join(", ");
    let mut limits = BTreeMap::new();
    let Value::Object(map) = value else {
        errors.push(error(path, format!("`{path}` of the extension `{stem}` must be a map of {listed}")));
        return limits;
    };
    for (key, value) in map {
        let at = format!("{path}.{key}");
        let Some(known) = keys.iter().find(|known| **known == key.as_str()) else {
            errors.push(error(
                &at,
                format!("`{at}` is not a key of `{path}` of the extension `{stem}`; the keys are {listed}"),
            ));
            continue;
        };
        match value.as_u64().filter(|n| *n >= 1) {
            None => errors.push(error(&at, format!("`{at}` of the extension `{stem}` must be a whole number from 1 up"))),
            Some(n) if *known == "stack_mib" && n >= MAX_STACK_MIB => errors.push(error(
                &at,
                format!(
                    "`{at}` of the extension `{stem}` must be below {MAX_STACK_MIB}, the stack in MiB of the \
                     threads that run the extension's handlers"
                ),
            )),
            Some(n) => {
                limits.insert(*known, n);
            }
        }
    }
    limits
}

/// Whether `version` meets the requirement `requirement` (true when absent,
/// or when either does not parse, which [`Manifest::read`] has excluded).
fn admits(requirement: Option<&str>, version: &str) -> bool {
    let Some(requirement) = requirement else { return true };
    match (semver::VersionReq::parse(requirement), semver::Version::parse(version)) {
        (Ok(requirement), Ok(version)) => requirement.matches(&version),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn details(value: Value) -> Vec<(String, String)> {
        Manifest::read("myext", &value).expect_err("refused").into_iter().map(|e| (e.path, e.detail)).collect()
    }

    #[test]
    fn an_empty_manifest_is_a_data_extension() {
        for value in [Value::Null, json!({}), json!({ "extension": null })] {
            let manifest = Manifest::read("myext", &value).expect("read");
            assert_eq!(manifest.engine, "data");
            assert_eq!((manifest.description, manifest.flm, manifest.js), (None, None, None));
        }
        let manifest =
            Manifest::read("myext", &json!({ "extension": { "description": "Mine", "author": "Me" }, "flm": { "use": "x" } }))
                .expect("read");
        assert_eq!(manifest.description.as_deref(), Some("Mine"));
        assert_eq!(manifest.author.as_deref(), Some("Me"));
        assert_eq!(manifest.flm, Some(json!({ "use": "x" })));
    }

    #[test]
    fn the_engine_is_data_when_absent() {
        assert_eq!(Manifest::read("myext", &json!({ "extension": { "description": "x" } })).unwrap().engine, "data");
        let js = Manifest::read("myext", &json!({ "extension": { "engine": "js", "js": { "main": "main.js" } } })).unwrap();
        assert_eq!(
            (js.engine.as_str(), js.js),
            ("js", Some(JsManifest { main: "main.js".to_string(), limits: JsLimits::default() }))
        );
        // An engine the program lacks is the loader's to refuse.
        assert_eq!(Manifest::read("myext", &json!({ "extension": { "engine": "python" } })).unwrap().engine, "python");
    }

    #[test]
    fn each_unknown_key_is_refused_with_its_path() {
        assert_eq!(
            details(json!({ "extension": { "engines": "js", "name": "x" }, "docs": "d" })),
            [
                (
                    "docs".to_string(),
                    "`docs` is not a key of the manifest of the extension `myext`; the keys are `extension` and `flm`".to_string()
                ),
                (
                    "extension.engines".to_string(),
                    "`extension.engines` is not a key of the `extension` block of the extension `myext`; the keys are \
                     `description`, `author`, `flm_version`, `engine`, `js`, `wasm`"
                        .to_string()
                ),
                (
                    "extension.name".to_string(),
                    "`extension.name` is not a key of the `extension` block of the extension `myext`; the keys are \
                     `description`, `author`, `flm_version`, `engine`, `js`, `wasm`"
                        .to_string()
                ),
            ]
        );
        assert_eq!(
            details(json!(["x"])),
            [(
                String::new(),
                "the manifest of the extension `myext` must be a map holding `extension` and, for the `data` engine, `flm`"
                    .to_string()
            )]
        );
        assert_eq!(
            details(json!({ "extension": "x" })),
            [("extension".to_string(), "`extension` in the manifest of the extension `myext` must be a map".to_string())]
        );
        assert_eq!(
            details(json!({ "extension": { "description": 3 } })),
            [("extension.description".to_string(), "`extension.description` of the extension `myext` must be a text".to_string())]
        );
    }

    #[test]
    fn js_needs_main_and_is_refused_for_another_engine() {
        assert_eq!(
            details(json!({ "extension": { "engine": "js" } })),
            [(
                "extension.js.main".to_string(),
                "the extension `myext` has the engine `js`, so its manifest must give `extension.js.main`, the path of \
                 its main module"
                    .to_string()
            )]
        );
        assert_eq!(
            details(json!({ "extension": { "js": { "main": "main.js" } } })),
            [(
                "extension.js".to_string(),
                "the extension `myext` writes `extension.js`, which is the `js` engine's key, and its engine is `data`"
                    .to_string()
            )]
        );
        assert_eq!(
            details(json!({ "extension": { "engine": "js", "js": { "main": "m.js", "workers": 2 } } })),
            [(
                "extension.js.workers".to_string(),
                "`extension.js.workers` is not a key of `extension.js` of the extension `myext`; the keys are `main` \
                 and `limits`"
                    .to_string()
            )]
        );
        assert_eq!(
            details(json!({ "extension": { "engine": "js", "js": "main.js" } })),
            [("extension.js".to_string(), "`extension.js` of the extension `myext` must be a map".to_string())]
        );
        // The missing key is reported whatever else the block holds.
        assert_eq!(
            details(json!({ "extension": { "engine": "js", "js": { "limits": { "time_s": 0 } } } })),
            [
                (
                    "extension.js.limits.time_s".to_string(),
                    "`extension.js.limits.time_s` of the extension `myext` must be a whole number from 1 up".to_string()
                ),
                (
                    "extension.js.main".to_string(),
                    "the extension `myext` has the engine `js`, so its manifest must give `extension.js.main`, the \
                     path of its main module"
                        .to_string()
                ),
            ]
        );
    }

    #[test]
    fn js_limits_are_read_each_optional_and_checked() {
        let read = |limits: Value| Manifest::read("myext", &json!({ "extension": { "engine": "js", "js": { "main": "m.js", "limits": limits } } }));
        let manifest = read(json!({ "stack_mib": 16, "time_s": 60 })).expect("read");
        assert_eq!(
            manifest.js.map(|js| js.limits),
            Some(JsLimits { memory_mib: None, stack_mib: Some(16), time_s: Some(60) })
        );
        let mut found: Vec<(String, String)> = read(json!({ "stack_mib": 64, "memory_mib": 0, "heap": 1 }))
            .expect_err("refused")
            .into_iter()
            .map(|e| (e.path, e.detail))
            .collect();
        found.sort();
        assert_eq!(
            found,
            [
                (
                    "extension.js.limits.heap".to_string(),
                    "`extension.js.limits.heap` is not a key of `extension.js.limits` of the extension `myext`; the \
                     keys are `memory_mib`, `stack_mib`, `time_s`"
                        .to_string()
                ),
                (
                    "extension.js.limits.memory_mib".to_string(),
                    "`extension.js.limits.memory_mib` of the extension `myext` must be a whole number from 1 up".to_string()
                ),
                (
                    "extension.js.limits.stack_mib".to_string(),
                    "`extension.js.limits.stack_mib` of the extension `myext` must be below 64, the stack in MiB of \
                     the threads that run the extension's handlers"
                        .to_string()
                ),
            ]
        );
        let found = read(json!(16)).expect_err("refused");
        assert_eq!(
            found[0].detail,
            "`extension.js.limits` of the extension `myext` must be a map of `memory_mib`, `stack_mib`, `time_s`"
        );
    }

    #[test]
    fn wasm_needs_module_and_is_refused_for_another_engine() {
        let read = Manifest::read("myext", &json!({ "extension": { "engine": "wasm", "wasm": { "module": "m.wasm" } } }))
            .expect("read");
        assert_eq!(read.wasm, Some(WasmManifest { module: "m.wasm".to_string(), limits: WasmLimits::default() }));
        assert_eq!(read.js, None);
        assert_eq!(
            details(json!({ "extension": { "engine": "wasm" } })),
            [(
                "extension.wasm.module".to_string(),
                "the extension `myext` has the engine `wasm`, so its manifest must give `extension.wasm.module`, the \
                 path of its module"
                    .to_string()
            )]
        );
        assert_eq!(
            details(json!({ "extension": { "engine": "js", "js": { "main": "m.js" }, "wasm": { "module": "m.wasm" } } })),
            [(
                "extension.wasm".to_string(),
                "the extension `myext` writes `extension.wasm`, which is the `wasm` engine's key, and its engine is `js`"
                    .to_string()
            )]
        );
        assert_eq!(
            details(json!({ "extension": { "engine": "wasm", "wasm": { "module": "m.wasm", "main": "x" } } })),
            [(
                "extension.wasm.main".to_string(),
                "`extension.wasm.main` is not a key of `extension.wasm` of the extension `myext`; the keys are \
                 `module` and `limits`"
                    .to_string()
            )]
        );
        assert_eq!(
            details(json!({ "extension": { "engine": "wasm", "wasm": "m.wasm" } })),
            [("extension.wasm".to_string(), "`extension.wasm` of the extension `myext` must be a map".to_string())]
        );
        assert_eq!(
            details(json!({ "extension": { "engine": "wasm", "wasm": { "module": 3 } } })),
            [("extension.wasm.module".to_string(), "`extension.wasm.module` of the extension `myext` must be a text".to_string())]
        );
        // The missing key is reported whatever else the block holds.
        assert_eq!(
            details(json!({ "extension": { "engine": "wasm", "wasm": { "limits": { "fuel": 0 } } } })),
            [
                (
                    "extension.wasm.limits.fuel".to_string(),
                    "`extension.wasm.limits.fuel` of the extension `myext` must be a whole number from 1 up".to_string()
                ),
                (
                    "extension.wasm.module".to_string(),
                    "the extension `myext` has the engine `wasm`, so its manifest must give `extension.wasm.module`, \
                     the path of its module"
                        .to_string()
                ),
            ]
        );
    }

    #[test]
    fn wasm_limits_are_read_each_optional_and_checked() {
        let read = |limits: Value| {
            Manifest::read("myext", &json!({ "extension": { "engine": "wasm", "wasm": { "module": "m.wasm", "limits": limits } } }))
        };
        let manifest = read(json!({ "memory_mib": 16, "fuel": 5000 })).expect("read");
        assert_eq!(manifest.wasm.map(|wasm| wasm.limits), Some(WasmLimits { memory_mib: Some(16), fuel: Some(5000) }));
        let mut found: Vec<(String, String)> =
            read(json!({ "fuel": 0, "stack_mib": 1 })).expect_err("refused").into_iter().map(|e| (e.path, e.detail)).collect();
        found.sort();
        assert_eq!(
            found,
            [
                (
                    "extension.wasm.limits.fuel".to_string(),
                    "`extension.wasm.limits.fuel` of the extension `myext` must be a whole number from 1 up".to_string()
                ),
                (
                    "extension.wasm.limits.stack_mib".to_string(),
                    "`extension.wasm.limits.stack_mib` is not a key of `extension.wasm.limits` of the extension \
                     `myext`; the keys are `memory_mib`, `fuel`"
                        .to_string()
                ),
            ]
        );
        let found = read(json!(16)).expect_err("refused");
        assert_eq!(found[0].detail, "`extension.wasm.limits` of the extension `myext` must be a map of `memory_mib`, `fuel`");
    }

    #[test]
    fn flm_is_refused_for_an_engine_other_than_data() {
        assert_eq!(
            details(json!({ "extension": { "engine": "js", "js": { "main": "main.js" } }, "flm": { "use": "x" } })),
            [("flm".to_string(), "the extension `myext` writes `flm`, which only the `data` engine reads, and its engine is `js`".to_string())]
        );
    }

    #[test]
    fn flm_version_must_be_a_requirement() {
        let found = details(json!({ "extension": { "flm_version": "soon" } }));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "extension.flm_version");
        assert!(
            found[0].1.starts_with(
                "`extension.flm_version` of the extension `myext` must be a version requirement such as `>=0.5` or \
                 `^0.5.0`, and `soon` is not one: "
            ),
            "{}",
            found[0].1
        );
        let manifest = Manifest::read("myext", &json!({ "extension": { "flm_version": ">=0.5" } })).unwrap();
        assert_eq!(manifest.flm_version.as_deref(), Some(">=0.5"));
    }

    #[test]
    fn flm_version_is_checked_against_this_program() {
        assert!(admits(None, "0.5.0"));
        assert!(admits(Some(">=0.5"), "0.5.0"));
        assert!(admits(Some("^0.5.0"), "0.5.3"));
        assert!(!admits(Some(">=0.9"), "0.5.0"));
        let this = Manifest::read("myext", &json!({ "extension": { "flm_version": format!("={FLM_VERSION}") } })).unwrap();
        assert!(this.admits_this_program());
        let later = Manifest::read("myext", &json!({ "extension": { "flm_version": ">=999" } })).unwrap();
        assert!(!later.admits_this_program());
    }
}
