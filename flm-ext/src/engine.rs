//! The engine contract: what turns an extension's manifest and files into
//! packages ([`Engine`]), what it reports ([`Loaded`], [`PackageReport`],
//! [`FeatureReport`] and the settings it declares), and its failures
//! ([`LoadError`]).
//!
//! The [loader](crate::loader::ExtensionLoader) picks the engine the
//! manifest names (`extension.engine`, `data` when absent), calls
//! [`Engine::load`] once per extension, and turns each reported package into
//! a factory ([`package_factory`](crate::adapter::package_factory)).

use std::fmt;
use std::sync::Arc;

use flm_core::stdcore::features::compose::Template;
use serde_json::{Map, Value};

use crate::block::Block;
use crate::entry::UseEntry;
use crate::manifest::Manifest;
use crate::runtime::ExtRuntime;
use crate::source::ExtensionFiles;

/// The name of the engine every loader has.
pub const DATA: &str = "data";

/// An extension as the loader and the engines name it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtensionIdentity {
    /// The source's string for the extension; the factory names' prefix.
    pub identity: String,
    /// The last segment of the locator that first reached it; the declared
    /// names' prefix.
    pub stem: String,
}

impl ExtensionIdentity {
    /// The factory name of the extension's package `package`:
    /// `<identity>#<package>`, always with the package.
    pub fn factory_name(&self, package: &str) -> String {
        format!("{}#{package}", self.identity)
    }
}

impl fmt::Display for ExtensionIdentity {
    /// "the extension `myext` (`_flmextensions/myext.yaml`)".
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the extension `{}` (`{}`)", self.stem, self.identity)
    }
}

/// An engine: what turns an extension's manifest and files into packages.
pub trait Engine: Send + Sync {
    /// The name `extension.engine` gives (`data`, `js`).
    fn name(&self) -> &str;

    /// Loads the extension: reads what it needs through `files`, which it
    /// owns from now on (an engine may keep them for a module loader that
    /// must be `'static`), and reports its packages.
    ///
    /// # Errors
    ///
    /// Every problem found; the loader reports a [`LoadError::Manifest`] in
    /// the manifest's file and the others where the `use` entry stands.
    fn load(
        &self,
        extension: &ExtensionIdentity,
        manifest: &Manifest,
        files: Box<dyn ExtensionFiles>,
    ) -> Result<Loaded, Vec<LoadError>>;
}

/// What an engine reports about a loaded extension.
pub struct Loaded {
    /// Its packages, in the order reported; names unique.
    pub packages: Vec<PackageReport>,
    /// The runtime that runs every feature of every package of the
    /// extension; its [`digest_input`](ExtRuntime::digest_input) is the
    /// engine's part of the digest (the modules, the engine's version).
    pub runtime: Arc<dyn ExtRuntime>,
}

/// One package of a loaded extension.
#[derive(Debug, Clone, PartialEq)]
pub struct PackageReport {
    /// Its name within the extension (`_` is the default package).
    pub name: String,
    /// The packages it uses, `.` included (rewritten by the loader to the
    /// extension's identity).
    pub uses: Vec<UseEntry>,
    /// Its block (the data engine: the manifest's `flm`; the js engine: the
    /// package object's `flm`, with `uses` outside it).
    pub block: Block,
    /// The compose templates it ships, each read from its description,
    /// `templates/OUTPUT/NAME.yaml` or `templates/OUTPUT/NAME/flmtemplate.yaml`
    /// ([`templates::read`](crate::templates::read)), `package` set to the
    /// factory name.
    pub templates: Vec<Template>,
    /// Its scripted features, in order.
    pub features: Vec<FeatureReport>,
    /// The host services its handlers call (`files`), by name, as the
    /// package declares them (the js engine: the package object's key
    /// `host`; the data engine: none). The
    /// [loader](crate::loader::ExtensionLoader) refuses a package declaring a
    /// service the host does not offer, and feeds the list into the
    /// extension's digest; a handler reaches a declared service alone
    /// ([`StageCx::host`](crate::api::core::run::StageCx::host)).
    pub host: Vec<String>,
}

/// One scripted feature: the fields of
/// [`FeatureInfo`](flm_core::core::registry::FeatureInfo) and the settings it
/// declares, as the engine reads them from the feature object.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct FeatureReport {
    /// The feature's name, unique in the registry; not prefixed by the
    /// extension's stem.
    pub name: String,
    /// A one-line title.
    #[serde(default)]
    pub title: String,
    /// Documentation prose about the feature.
    #[serde(default)]
    pub docs: String,
    /// The feature's version, if it has one.
    #[serde(default)]
    pub version: Option<String>,
    /// The settings it declares, if any.
    #[serde(default)]
    pub settings: Option<SettingsReport>,
}

/// A feature's settings declared as data, turned into a
/// [`SettingsDecl`](flm_core::core::settings::SettingsDecl) by the adapter.
#[derive(Debug, Clone, PartialEq, Default, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct SettingsReport {
    /// `config`: `{ docs, schema }`.
    #[serde(default)]
    pub config: Option<ConfigReport>,
    /// `options`: name → `{ bind, docs, schema }`, in the order written (each
    /// read by the adapter into an [`OptionReport`], so that a problem names
    /// its key path).
    #[serde(default)]
    pub options: Map<String, Value>,
    /// `merge_entries`: key → its default entries (a map).
    #[serde(default)]
    pub merge_entries: Map<String, Value>,
    /// The keys whose values are markup.
    #[serde(default)]
    pub markup: Vec<String>,
    /// The keys whose values are paths.
    #[serde(default)]
    pub path: Vec<String>,
}

/// A feature's configuration declared as data.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConfigReport {
    /// Documentation prose about the configuration.
    #[serde(default)]
    pub docs: String,
    /// Its JSON Schema: an object schema with `properties`.
    pub schema: Value,
}

/// One option type declared as data.
#[derive(Debug, Clone, PartialEq, serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
pub struct OptionReport {
    /// Where its values may be set.
    pub bind: DataBind,
    /// Documentation prose about the option.
    #[serde(default)]
    pub docs: String,
    /// Its JSON Schema: an object schema with `properties`.
    pub schema: Value,
}

/// Where a data option's values may be set: `document`, `render` or `either`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DataBind {
    /// In the document.
    Document,
    /// On the render request.
    Render,
    /// In either.
    Either,
}

/// A failure to load an extension.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadError {
    /// A problem in the manifest or a block it holds, at a key path; placed
    /// in the manifest's file.
    Manifest {
        /// The key's path (`flm.use[2]`, `extension.engine`).
        path: String,
        /// The condition, as text for a person; it names the extension.
        detail: String,
    },
    /// Any other failure: a module, a template, a declaration, the engine
    /// itself.
    Load {
        /// The condition, as text for a person; it names the extension.
        detail: String,
    },
}

impl LoadError {
    /// The detail.
    pub fn detail(&self) -> &str {
        match self {
            LoadError::Manifest { detail, .. } | LoadError::Load { detail } => detail,
        }
    }
}

impl fmt::Display for LoadError {
    /// The detail.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.detail())
    }
}

impl std::error::Error for LoadError {}
