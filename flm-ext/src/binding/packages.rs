//! The reading of package objects: the JSON values an extension's
//! `packages()` entry answers (one object per `ext.package(name, object)`
//! call), read into [`PackageReport`]s, every problem found. Shared by the
//! bindings: the shape is the prelude's and the guest client's alike.

#[cfg(any(feature = "quickjs", feature = "wasm"))]
use std::fmt::Display;

use serde_json::Value;

use crate::block::{self, BLOCK_KEYS};
use crate::engine::{ExtensionIdentity, FeatureReport, LoadError, PackageReport};
use crate::entry::{is_segment, UseEntry};
use crate::paths::Root;
use crate::source::ExtensionFiles;
use crate::templates;

/// The package objects `items` of the answer of `packages`, read into
/// reports; every problem found.
pub(crate) fn read_packages(
    extension: &ExtensionIdentity,
    items: &[Value],
    files: &dyn ExtensionFiles,
) -> Result<Vec<PackageReport>, Vec<LoadError>> {
    let mut errors = Vec::new();
    let mut packages = Vec::new();
    for item in items {
        let mut problems = Vec::new();
        let report = read_package(extension, item, files, &mut problems);
        if problems.is_empty() {
            packages.push(report);
        }
        errors.extend(problems.into_iter().map(|detail| LoadError::Load { detail }));
    }
    if errors.is_empty() { Ok(packages) } else { Err(errors) }
}

/// The load's failure when the answer of `packages` that a text binding
/// holds cannot be read as a list of package objects; `error` is the
/// reading's.
#[cfg(any(feature = "quickjs", feature = "wasm"))]
pub(crate) fn unreadable(extension: &ExtensionIdentity, error: impl Display) -> Vec<LoadError> {
    let stem = &extension.stem;
    vec![LoadError::Load { detail: format!("the package objects of the extension `{stem}` cannot be read: {error}") }]
}

/// One package object, read into a report; each problem a text in `problems`.
fn read_package(
    extension: &ExtensionIdentity,
    item: &Value,
    files: &dyn ExtensionFiles,
    problems: &mut Vec<String>,
) -> PackageReport {
    let stem = &extension.stem;
    let name = item.get("name").and_then(Value::as_str).unwrap_or_default().to_string();
    if !is_segment(&name) {
        problems.push(format!("the package name `{name}` of the extension `{stem}` must be letters, digits, `-` and `_`"));
    }
    let texts = |key: &str| -> Vec<String> {
        item.get(key)
            .and_then(Value::as_array)
            .map(|list| list.iter().filter_map(Value::as_str).map(str::to_string).collect())
            .unwrap_or_default()
    };
    let mut uses = Vec::new();
    for entry in texts("uses") {
        match UseEntry::parse(&entry) {
            Ok(entry) => uses.push(entry),
            Err(error) => {
                problems.push(format!("in `uses` of the package `{name}` of the extension `{stem}`, {}", error.detail));
            }
        }
    }
    let flm = item.get("flm").cloned().unwrap_or(Value::Null);
    let block = read_flm(stem, &name, &flm, problems);
    let factory = extension.factory_name(&name);
    let mut found_templates = Vec::new();
    let root = Root::Ext(extension.name.clone());
    for path in texts("templates") {
        match templates::read(files, &root, &path, &factory, extension) {
            Ok(template) => found_templates.push(template),
            Err(error) => problems.push(error.to_string()),
        }
    }
    let mut features = Vec::new();
    for feature in item.get("features").and_then(Value::as_array).into_iter().flatten() {
        if let Some(feature) = read_feature(stem, &name, feature, problems) {
            features.push(feature);
        }
    }
    // The host services the package's handlers call (`ext.package` refuses
    // a `host` that is not a list of texts; read again here, as data).
    let host = match item.get("host") {
        None | Some(Value::Null) => Vec::new(),
        Some(Value::Array(list)) if list.iter().all(Value::is_string) => texts("host"),
        Some(_) => {
            problems.push(format!("`host` of the package `{name}` of the extension `{stem}` must be a list of texts"));
            Vec::new()
        }
    };
    let caches = crate::host::read_caches(stem, &name, item.get("caches").unwrap_or(&Value::Null), problems);
    PackageReport { name, uses, block, templates: found_templates, features, host, caches }
}

/// A package object's `flm`, read by the block step: `use` is refused (a
/// package's uses are its `uses` list), and so is every key the block step
/// does not read.
fn read_flm(stem: &str, package: &str, flm: &Value, problems: &mut Vec<String>) -> block::Block {
    if flm.get("use").is_some() {
        problems.push(format!(
            "`flm.use` of the package `{package}` of the extension `{stem}` must be written as the package's `uses` \
             list"
        ));
    }
    let block = match block::read(flm) {
        Ok(block) => block,
        Err(found) => {
            for error in found {
                problems.push(format!("in `flm` of the package `{package}` of the extension `{stem}`, {}", error.detail));
            }
            block::Block::default()
        }
    };
    let keys: Vec<String> =
        BLOCK_KEYS.iter().filter(|key| **key != "use").map(|key| format!("`{key}`")).collect::<Vec<_>>();
    let listed = joined(&keys);
    let others = flm.as_object().into_iter().flat_map(|map| map.keys());
    for key in others.filter(|key| !BLOCK_KEYS.contains(&key.as_str())) {
        problems.push(format!(
            "`flm.{key}` is not a key of `flm` of the package `{package}` of the extension `{stem}`; the keys are {listed}"
        ));
    }
    block
}

/// The texts `items` joined by commas and a final `and` (`a, b and c`).
pub(crate) fn joined(items: &[String]) -> String {
    match items.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => items.join(", "),
    }
}

/// A feature object as a script writes it (its `register` left out): the
/// keys of [`FeatureReport`] but its name, which is the feature's key in
/// `features`. A key the object holds besides these is refused as unknown,
/// `name` included.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FeatureObject {
    #[serde(default)]
    title: String,
    #[serde(default)]
    docs: String,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    settings: Option<crate::engine::SettingsReport>,
    #[serde(default)]
    templates: std::collections::BTreeMap<String, crate::engine::ModuleReport>,
}

/// The parts of a feature object's `settings` that must be maps, and the
/// shape each text names.
const SETTINGS_MAPS: [(&str, &str); 2] = [
    ("config", "a map with `schema` and, optionally, `docs`"),
    ("options", "a map from an option's name to its declaration"),
];

/// One feature `{ name, object }` (its object without its `register`), read
/// into a report: the shapes serde would word with a type's name are refused
/// first, each naming its key path. The keys `settings` and `templates`
/// written `null`, and a part of `settings` written `null`, are read as
/// absent.
fn read_feature(stem: &str, package: &str, feature: &Value, problems: &mut Vec<String>) -> Option<FeatureReport> {
    let name = feature.get("name").and_then(Value::as_str).unwrap_or_default();
    let owner = format!("of the feature `{name}` of the package `{package}` of the extension `{stem}`");
    let mut object = feature.get("object").cloned().unwrap_or(Value::Null);
    let mut shaped = true;
    if let Value::Object(fields) = &mut object {
        if fields.get("settings").is_some_and(Value::is_null) {
            fields.remove("settings");
        }
        match fields.get_mut("settings") {
            None => {}
            Some(Value::Object(settings)) => {
                settings.retain(|_, value| !value.is_null());
                for (key, shape) in SETTINGS_MAPS {
                    if settings.get(key).is_some_and(|value| !value.is_object()) {
                        problems.push(format!("`settings.{key}` {owner} must be {shape}"));
                        shaped = false;
                    }
                }
            }
            Some(_) => {
                problems.push(format!(
                    "Invalid ‘settings’ of the feature ‘{name}’ of the package ‘{package}’ of the \
                     extension ‘{stem}’: it must be a map with the optional keys ‘config’, \
                     ‘options’ and ‘read’"
                ));
                shaped = false;
            }
        }
        if fields.get("templates").is_some_and(Value::is_null) {
            fields.remove("templates");
        }
        let modules_shaped = match fields.get("templates") {
            None => true,
            Some(Value::Object(modules)) => modules.values().all(|module| {
                module.as_object().is_some_and(|module| module.get("output").is_some_and(Value::is_string))
            }),
            Some(_) => false,
        };
        if !modules_shaped {
            problems.push(format!(
                "Invalid ‘templates’ of the feature ‘{name}’ of the package ‘{package}’ of the extension \
                 ‘{stem}’: it must be a map from a template module's name to ‘{{ output, settings }}’ \
                 (‘settings’ optional)"
            ));
            shaped = false;
        }
    }
    if !shaped {
        return None;
    }
    match serde_json::from_value::<FeatureObject>(object) {
        Ok(read) => Some(FeatureReport {
            name: name.to_string(),
            title: read.title,
            docs: read.docs,
            version: read.version,
            settings: read.settings,
            templates: read.templates,
        }),
        Err(error) => {
            problems.push(format!(
                "the feature `{name}` of the package `{package}` of the extension `{stem}` cannot be read: {error}"
            ));
            None
        }
    }
}
