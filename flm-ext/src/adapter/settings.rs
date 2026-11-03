//! A scripted feature's settings declaration, built from the data its report
//! carries ([`SettingsReport`](crate::engine::SettingsReport)) with the data
//! constructors of [`SettingsDecl`]: the configuration
//! ([`config_data`](SettingsDecl::config_data), or
//! [`config_data_read`](SettingsDecl::config_data_read) for a feature that
//! reads it, its reading calling the feature's configuration reader,
//! [`ExtRuntime::read_settings`], whenever the settings of a document that
//! uses the feature's package are read), each option type
//! ([`options_data`](SettingsDecl::options_data), its name under the stem
//! rule), each key merged entry by entry
//! ([`merge_entries_data`](SettingsDecl::merge_entries_data)), and the
//! markup and path marks.
//!
//! The declaration is built when the package's factory is made
//! ([`package_factory`](crate::adapter::package_factory)), so that a
//! declaration that contradicts itself fails the extension's load.

use std::sync::Arc;

use flm_core::core::settings::{ReadError, SettingsDecl, SettingsError};
use flm_core::model::options::Bind;
use serde_json::Value;

use crate::adapter::names;
use crate::engine::{DataBind, ExtensionIdentity, FeatureReport, LoadError, OptionReport};
use crate::runtime::ExtRuntime;

/// The settings declaration of the feature `feature` of the package
/// `package` (its name within the extension) of `extension`, run by
/// `runtime`, or `None` when the feature declares no settings.
///
/// In the order the report gives them: the configuration (`settings.config`,
/// read by the feature's configuration reader when `settings.read` is set),
/// each option type (`settings.options.NAME`, in the order written), each key
/// merged entry by entry (`settings.merge_entries.KEY`), each markup mark,
/// each path mark.
///
/// # Errors
///
/// A [`LoadError::Load`] for every problem found, each naming the extension,
/// the feature and the key path under `settings`: a declaration that is not
/// one or contradicts itself (a schema without `properties`, a key declared
/// twice, a merge mark on a path below a key, a reader without a
/// configuration), an option whose part cannot be read or whose default
/// entries are not a map, and an option name that does not begin with one of
/// the extension's namespaces and a dot ([`names::namespaces`]).
pub(crate) fn settings_decl(
    extension: &ExtensionIdentity,
    package: &str,
    feature: &FeatureReport,
    runtime: &Arc<dyn ExtRuntime>,
) -> Result<Option<SettingsDecl>, Vec<LoadError>> {
    let Some(report) = &feature.settings else {
        return Ok(None);
    };
    let stem = &extension.stem;
    let name = &feature.name;
    let mut errors = Vec::new();
    let mut decl = SettingsDecl::new(name.as_str());
    if let Some(config) = &report.config {
        let result = if report.read {
            let read = reader(runtime, package, name);
            decl.clone().config_data_read(config.schema.clone(), config.docs.as_str(), read)
        } else {
            decl.clone().config_data(config.schema.clone(), config.docs.as_str())
        };
        apply(&mut decl, result, stem, "settings.config", &mut errors);
    } else if report.read {
        errors.push(LoadError::Load {
            detail: format!(
                "in the extension `{stem}`, the feature `{name}` of the package `{package}` reads its configuration \
                 and must declare one (`settings.config`)"
            ),
        });
    }
    for (option, part) in &report.options {
        let path = format!("settings.options.{option}");
        if let Err(error) = names::check("option", option, stem, package) {
            errors.push(LoadError::Load { detail: error.to_string() });
        }
        // The shapes serde would word with a type's name are refused first.
        let Value::Object(fields) = part else {
            errors.push(LoadError::Load {
                detail: format!(
                    "in the extension `{stem}`, the option `{option}` of the feature `{name}` of the package \
                     `{package}` must be a map with `bind`, `schema` and, optionally, `docs` (`{path}`)"
                ),
            });
            continue;
        };
        if let Some(written) = fields.get("bind").filter(|bind| !bind.is_string()) {
            errors.push(LoadError::Load {
                detail: format!(
                    "in the extension `{stem}`, `bind` of the option `{option}` of the feature `{name}` of the \
                     package `{package}` must be `document`, `render` or `either`, and it is `{written}` (`{path}.bind`)"
                ),
            });
            continue;
        }
        let part: OptionReport = match serde_json::from_value(part.clone()) {
            Ok(part) => part,
            Err(error) => {
                errors.push(LoadError::Load {
                    detail: format!(
                        "in the extension `{stem}`, the settings of the feature `{name}` of the package `{package}` \
                         cannot be read (`{path}`): {error}"
                    ),
                });
                continue;
            }
        };
        let result = decl.clone().options_data(option.as_str(), bind(part.bind), part.schema, part.docs);
        apply(&mut decl, result, stem, &path, &mut errors);
    }
    for (key, entries) in &report.merge_entries {
        let path = format!("settings.merge_entries.{key}");
        let Value::Object(entries) = entries else {
            errors.push(LoadError::Load {
                detail: format!("in the extension `{stem}`, the default entries of `{name}.{key}` (`{path}`) must be a map"),
            });
            continue;
        };
        let result = decl.clone().merge_entries_data(key.as_str(), entries.clone());
        apply(&mut decl, result, stem, &path, &mut errors);
    }
    for path in &report.markup {
        decl = decl.markup(path.as_str());
    }
    for path in &report.path {
        decl = decl.path(path.as_str());
    }
    if errors.is_empty() { Ok(Some(decl)) } else { Err(errors) }
}

/// The reading of the configuration of the feature `feature` of the package
/// `package`: a call of its configuration reader through `runtime`, which
/// runs whenever the host reads the settings of a document that uses the
/// package and does not switch the feature off, with the configuration
/// written under the feature (`{}` when nothing is written under it). A
/// failure of the call is the fault of the configuration as a whole, with the
/// failure's text.
fn reader(
    runtime: &Arc<dyn ExtRuntime>,
    package: &str,
    feature: &str,
) -> Arc<dyn Fn(Value) -> Result<Value, ReadError> + Send + Sync> {
    let (runtime, package, feature) = (Arc::clone(runtime), package.to_string(), feature.to_string());
    Arc::new(move |settings: Value| {
        runtime
            .read_settings(&package, &feature, &settings)
            .unwrap_or_else(|error| Err(ReadError { path: String::new(), detail: error.to_string() }))
    })
}

/// The binding of a data option.
fn bind(bind: DataBind) -> Bind {
    match bind {
        DataBind::Document => Bind::Document,
        DataBind::Render => Bind::Render,
        DataBind::Either => Bind::Either,
    }
}

/// Keeps the declaration `result` answers, or records its refusal at `path`.
fn apply(
    decl: &mut SettingsDecl,
    result: Result<SettingsDecl, SettingsError>,
    stem: &str,
    path: &str,
    errors: &mut Vec<LoadError>,
) {
    match result {
        Ok(next) => *decl = next,
        Err(error) => errors.push(invalid(stem, &error, path)),
    }
}

/// A declaration the data constructors refuse, as a load failure: the
/// constructor's text, after the extension, with the key path under
/// `settings` of the part that was declared.
fn invalid(stem: &str, error: &SettingsError, path: &str) -> LoadError {
    LoadError::Load { detail: format!("in the extension `{stem}`, {} (`{path}`)", error.detail) }
}
