//! The adapter: `flm-core`'s own traits implemented by calling an extension,
//! and the per-call handle table and by-name dispatcher a binding whose values
//! cross as JSON reaches the extension API through.
//!
//! # The lifecycle of a scripted package
//!
//! 1. An engine loads the extension and reports its packages
//!    ([`Engine::load`](crate::engine::Engine::load)); the loader turns each
//!    report into a factory with [`package_factory`].
//! 2. The factory's build makes one `flm-core` feature per feature of the
//!    report not switched off, plus the templates feature
//!    ([`TemplatesFeature`](templates::TemplatesFeature)) when the package
//!    ships templates, names the packages the report uses, and carries the
//!    chains of the feature order its block writes.
//! 3. The registry build asks each feature for its information and settings,
//!    then calls its `register`, which is where the extension's `register`
//!    handler runs ([`ExtRuntime::register`]) against a façade over the real
//!    builder.
//! 4. Later, `flm-core` calls the adapter's stages, rewrite stages,
//!    expansion hooks and rule bodies wherever the registry holds them, and
//!    they call the extension's
//!    [`Stage`](crate::api::core::stage::Stage),
//!    [`RewriteStage`](crate::api::core::stage::RewriteStage),
//!    [`Expand`](crate::api::lang::rewrites::userdefs::Expand) and
//!    [`Lower`](crate::api::core::lowering::Lower) objects; a
//!    late-init hook compiles the templates of the extension's user
//!    definitions once the language is built. A lowering rule the extension
//!    built is registered by a finalizer, in the build's second round
//!    ([`lower::RuleFinalizer`]), and so are the extension's stages and
//!    backends (a backend's own stages typed by the tables they write, once
//!    every feature has declared its tables). A rewrite stage is registered
//!    at once ([`rewrite::ExtRewriteStage`]): the table it publishes is a
//!    table of trees, whose type is known.

use std::collections::BTreeSet;
use std::sync::Arc;

use flm_core::core::registry::{PackageEntry, PackageError, PackageFactory};
use flm_core::core::settings::{PackageSettings, SettingsDecl};
use flm_core::model::options::DataValue;
use serde_json::Value;

use crate::engine::{ExtensionIdentity, LoadError, PackageReport};
use crate::runtime::ExtRuntime;
use services::DeclaredServices;

pub mod aspects;
pub mod builder;
pub mod citations;
pub mod compose;
pub mod diag;
pub mod dispatch;
pub mod durable;
pub mod expand;
pub mod feature;
#[rustfmt::skip]
pub(crate) mod generated;
pub mod handles;
pub mod lower;
pub mod names;
pub mod node;
pub mod payloads;
pub mod rewrite;
pub mod services;
pub(crate) mod settings;
pub mod slots;
pub mod stage;
pub mod table;
pub mod tables;
pub mod templates;
pub mod values;

/// The table of every constant of the extension API, as `(module, name,
/// value)`, which the generator of the dispatcher, the prelude and the
/// declarations reads (`tests/api_surface.rs`); not part of the API.
#[doc(hidden)]
pub use generated::constants;

/// The factory `<identity>#<name>` of the package `report` of `extension`,
/// run by `runtime`: what a host's package catalog holds for a package of an
/// extension, as a compiled package's factory is for that package.
///
/// Its uses are `report.uses`, each through
/// [`UseEntry::catalog_name`](crate::entry::UseEntry::catalog_name) (`.` to the
/// identity). Its build holds, in the report's order, an
/// [`ExtFeature`](feature::ExtFeature) for each feature the settings do not
/// switch off, each with its settings declaration (built here, from the
/// report's settings) and its final configuration
/// ([`PackageSettings::config`]`::<`[`DataValue`]`>`, `{}` when absent), each
/// feeding `digest` into the registry digest, and, when the
/// package ships templates and the settings do not switch it off, the
/// [`TemplatesFeature`](templates::TemplatesFeature), titled `description`
/// (the extension's description; empty when it has none). Every feature holds
/// `services`, the host services the package declared, which its stages lend
/// ([`StageCx::host`](crate::api::core::run::StageCx::host)).
/// The entry carries the chains of the report's `feature_order`
/// ([`Block::feature_order`](crate::block::Block::feature_order)), whatever
/// the settings switch off (the registry build drops the features that are
/// not loaded), each with the origin `package NAME`, `NAME` being the
/// factory's name.
///
/// # Errors
///
/// A [`LoadError::Load`] for each feature with an empty name, each name used
/// by two features, and each problem of a feature's settings declaration: a
/// declaration that is not one or contradicts itself, an option name that
/// does not begin with one of the extension's namespaces and a dot
/// ([`names::namespaces`]).
pub fn package_factory(
    extension: &ExtensionIdentity,
    report: &PackageReport,
    runtime: Arc<dyn ExtRuntime>,
    digest: Arc<[u8]>,
    description: &str,
    services: Arc<DeclaredServices>,
) -> Result<PackageFactory, Vec<LoadError>> {
    let decls = match (check_report(extension, report), declarations(extension, report)) {
        (Ok(()), Ok(decls)) => decls,
        (checked, declared) => {
            let mut errors = checked.err().unwrap_or_default();
            errors.extend(declared.err().unwrap_or_default());
            return Err(errors);
        }
    };
    let name = extension.factory_name(&report.name);
    let uses: Vec<String> = report.uses.iter().map(|entry| entry.catalog_name(&extension.identity)).collect();
    let extension = extension.clone();
    let report = report.clone();
    let description = description.to_string();
    let entry_name = name.clone();
    let build = move |settings: &PackageSettings| -> Result<PackageEntry, PackageError> {
        let mut entry = PackageEntry::new(entry_name.as_str());
        for written in &report.block.feature_order {
            entry = entry.feature_order(written.chain_with_origin(format!("package {entry_name}")));
        }
        for feature in &report.features {
            if !settings.is_off(&feature.name) {
                let configuration = settings
                    .config::<DataValue>(&feature.name)
                    .map_or_else(|| Value::Object(serde_json::Map::new()), |config| config.0.clone());
                let decl = decls.iter().find(|(name, _)| *name == feature.name).and_then(|(_, decl)| decl.clone());
                let feature = feature::ExtFeature::new(
                    Arc::clone(&runtime),
                    &extension,
                    &report.name,
                    feature.clone(),
                    decl,
                    configuration,
                    Arc::clone(&digest),
                    Arc::clone(&services),
                );
                entry = entry.feature(Arc::new(feature));
            }
        }
        if !report.templates.is_empty() && !settings.is_off(&entry_name) {
            let templates = templates::TemplatesFeature::new(
                entry_name.clone(),
                description.clone(),
                report.templates.clone(),
                Arc::clone(&digest),
            );
            entry = entry.feature(Arc::new(templates));
        }
        Ok(entry)
    };
    let mut factory = PackageFactory::new(name, build).map_err(|error| vec![LoadError::Load { detail: error.to_string() }])?;
    for package in uses {
        factory = factory.uses(package);
    }
    Ok(factory)
}

/// The settings declaration of each feature of `report`, by the feature's
/// name ([`settings::settings_decl`]).
///
/// # Errors
///
/// Every problem of every feature's declaration.
fn declarations(
    extension: &ExtensionIdentity,
    report: &PackageReport,
) -> Result<Vec<(String, Option<SettingsDecl>)>, Vec<LoadError>> {
    let mut decls = Vec::new();
    let mut errors = Vec::new();
    for feature in &report.features {
        match settings::settings_decl(extension, &report.name, feature) {
            Ok(decl) => decls.push((feature.name.clone(), decl)),
            Err(found) => errors.extend(found),
        }
    }
    if errors.is_empty() { Ok(decls) } else { Err(errors) }
}

/// Checks that every feature of `report` has a name of its own, as
/// [`package_factory`] states.
fn check_report(extension: &ExtensionIdentity, report: &PackageReport) -> Result<(), Vec<LoadError>> {
    let (package, stem) = (&report.name, &extension.stem);
    let mut seen = BTreeSet::new();
    let mut errors = Vec::new();
    for feature in &report.features {
        if feature.name.is_empty() {
            errors.push(LoadError::Load {
                detail: format!("every feature of the package `{package}` of the extension `{stem}` must have a name"),
            });
        } else if !seen.insert(feature.name.as_str()) {
            errors.push(LoadError::Load {
                detail: format!(
                    "each feature of the package `{package}` of the extension `{stem}` must have a name of its own; \
                     `{}` is used twice",
                    feature.name
                ),
            });
        }
    }
    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
