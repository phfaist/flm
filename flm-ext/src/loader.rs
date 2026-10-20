//! The loader of extension packages, [`ExtensionLoader`]: a
//! [`PackageResolver`] over the host's sources and the engines.
//!
//! `resolve(name)`, in order:
//!
//! 1. **Exact lookup.** `name` is a factory name (`<identity>#<package>`) of
//!    a loaded package: that factory, before any parsing.
//! 2. **Parse.** `name` read as a `use` entry
//!    ([`UseEntry::parse`]); a name that is not one is not this loader's
//!    (`Ok(None)`); the locator `.` is refused.
//! 3. **Find.** The sources are asked in order; the first that answers gives
//!    the extension. A locator asked twice is looked up once, so that every
//!    answer of one loader is the same.
//! 4. **Load once per identity**: the manifest read, checked and kept
//!    ([`ExtensionLoader::manifest`]), the engine picked, [`Engine::load`]
//!    called, each reported package's `host` list checked against the host
//!    services the loader offers, each reported package turned into a factory
//!    ([`package_factory`]). A failed extension answers its problems to every
//!    entry that reaches it. A problem of the manifest is placed in the
//!    manifest's file, at the line and column the source's place resolver
//!    gives for its key path
//!    ([`FoundExtension::place_of`](crate::source::FoundExtension::place_of)).
//!
//! The extension's digest, fed by every feature into the registry digest, is
//! the manifest's canonical JSON, then for each package in the order
//! reported its name and its `host` list (each item followed by a NUL byte),
//! then the engine's bytes ([`ExtRuntime::digest_input`](crate::ExtRuntime::digest_input)).
//! 5. **Answer** the factory of the package the entry names.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use flm_core::core::registry::{PackageFactory, PackageResolver, ResolveError};
use flm_core::model::names::Name;
use serde_json::{Map, Value};

use crate::adapter::package_factory;
use crate::adapter::services::DeclaredServices;
use crate::block::Block;
use crate::data::DataEngine;
use crate::engine::{Engine, ExtensionIdentity, LoadError};
use crate::entry::{Locator, UseEntry};
use crate::manifest::{Manifest, FLM_VERSION};
use crate::host::{HostCaller, HostService};
use crate::source::{ExtensionFiles, ExtensionSource, PlaceOf};

/// A loaded package's block, for the host's cascade: the factory's name (the
/// package source's identity in the cascade), the extension, the block.
#[derive(Debug, Clone, PartialEq)]
pub struct LoadedBlock {
    /// The factory's name, `<identity>#<package>`.
    pub package: Name,
    /// The extension that provides the package.
    pub extension: ExtensionIdentity,
    /// The package's block.
    pub block: Block,
}

/// A loaded extension: its manifest, its factories by package name and its
/// blocks in the order the engine reported the packages.
struct LoadedExtension {
    /// The manifest as read from the value the source gave.
    manifest: Manifest,
    factories: BTreeMap<String, PackageFactory>,
    /// The package names in the order reported.
    packages: Vec<String>,
    blocks: Vec<LoadedBlock>,
}

/// What the loader has found and loaded so far.
#[derive(Default)]
struct LoaderState {
    /// Each locator asked, with the identity it gave (none: no source has it).
    found: BTreeMap<String, Option<String>>,
    /// Each extension reached, by identity: loaded, or its problems.
    loaded: BTreeMap<String, Result<Arc<LoadedExtension>, Vec<ResolveError>>>,
    /// Every factory of a loaded extension, by factory name.
    factories: BTreeMap<String, PackageFactory>,
    /// The identities in the order they were first reached.
    order: Vec<String>,
}

/// The resolver of extension packages: the host's sources asked in order, the
/// engines by name, the host services by name, a cache of loaded extensions
/// by identity. Implements [`PackageResolver`]; a host adds it to its catalog
/// and keeps an `Arc` for [`blocks`](ExtensionLoader::blocks).
pub struct ExtensionLoader {
    sources: Vec<Arc<dyn ExtensionSource>>,
    engines: BTreeMap<String, Arc<dyn Engine>>,
    host_services: BTreeMap<String, Arc<dyn HostService>>,
    state: Mutex<LoaderState>,
}

impl ExtensionLoader {
    /// A loader over `sources`, asked in order, with the data engine.
    pub fn new(sources: Vec<Arc<dyn ExtensionSource>>) -> ExtensionLoader {
        let loader = ExtensionLoader {
            sources,
            engines: BTreeMap::new(),
            host_services: BTreeMap::new(),
            state: Mutex::new(LoaderState::default()),
        };
        loader.with_engine(Arc::new(DataEngine))
    }

    /// Adds `engine` under its name (replacing one of that name).
    pub fn with_engine(mut self, engine: Arc<dyn Engine>) -> ExtensionLoader {
        self.engines.insert(engine.name().to_string(), engine);
        self
    }

    /// The names of the engines, sorted (for the unknown-engine message).
    pub fn engines(&self) -> Vec<String> {
        self.engines.keys().cloned().collect()
    }

    /// Offers `service` to the extensions under its name (replacing one of
    /// that name): a package that declares it in its `host` list reaches it
    /// through a handle ([`crate::host`]).
    pub fn with_host_service(mut self, service: Arc<dyn HostService>) -> ExtensionLoader {
        self.host_services.insert(service.name().to_string(), service);
        self
    }

    /// The names of the host services offered, sorted (for the refusal of a
    /// package declaring another).
    pub fn host_services(&self) -> Vec<String> {
        self.host_services.keys().cloned().collect()
    }

    /// The refusal of the host services of `report` this loader does not
    /// offer, one per service.
    fn unoffered(&self, extension: &ExtensionIdentity, report: &crate::engine::PackageReport) -> Vec<ResolveError> {
        let offered: Vec<String> = self.host_services.keys().map(|name| format!("`{name}`")).collect();
        let offers = if offered.is_empty() {
            "it offers no host service".to_string()
        } else {
            format!("it offers {}", offered.join(", "))
        };
        report
            .host
            .iter()
            .filter(|name| !self.host_services.contains_key(name.as_str()))
            .map(|name| {
                ResolveError::new(format!(
                    "the package `{}` of the extension `{}` declares the host service `{name}` (`host`), which this \
                     program does not offer; {offers}",
                    report.name, extension.stem
                ))
            })
            .collect()
    }

    /// Every loaded package's block: the extensions in the order they were
    /// first reached, and each extension's packages in the order its engine
    /// reported them.
    pub fn blocks(&self) -> Vec<LoadedBlock> {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        state
            .order
            .iter()
            .filter_map(|identity| state.loaded.get(identity)?.as_ref().ok())
            .flat_map(|loaded| loaded.blocks.iter().cloned())
            .collect()
    }

    /// Returns the manifest of the extension `identity` as this loader read it
    /// ([`Manifest::read`]), when the extension is loaded. The identity is the
    /// one a [`LoadedBlock`]'s extension carries
    /// ([`ExtensionIdentity::identity`]). `None` for an identity this loader
    /// has not reached, or whose extension failed to load.
    ///
    /// The loader reads the manifest from the value the extension's source
    /// gave, which may differ from the manifest's file: a host's source gives
    /// the manifest of an extension written in Rust and compiled into the
    /// program with the engine `native`. So the manifest returned specifies
    /// the engine that runs the extension.
    pub fn manifest(&self, identity: &str) -> Option<Manifest> {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        Some(state.loaded.get(identity)?.as_ref().ok()?.manifest.clone())
    }

    /// Loads the extension `identity` reached as `stem`, from its manifest
    /// value and its files, placing a problem of the manifest with
    /// `place_of`.
    fn load(
        &self,
        identity: &str,
        stem: &str,
        value: &Value,
        files: Box<dyn ExtensionFiles>,
        place_of: Option<&PlaceOf>,
    ) -> Result<Arc<LoadedExtension>, Vec<ResolveError>> {
        let extension = ExtensionIdentity { identity: identity.to_string(), stem: stem.to_string() };
        let in_manifest = |detail: String, path: &str| {
            let error = ResolveError::new(detail).in_file(identity);
            match place_of.and_then(|place_of| place_of(path)) {
                Some(place) => error.at(place.line, place.column),
                None => error,
            }
        };
        let manifest = Manifest::read(stem, value).map_err(|errors| {
            errors.into_iter().map(|error| in_manifest(error.detail, &error.path)).collect::<Vec<_>>()
        })?;
        if !manifest.admits_this_program() {
            let requirement = manifest.flm_version.as_deref().unwrap_or_default();
            return Err(vec![in_manifest(
                format!(
                    "the extension `{stem}` requires FLM `{requirement}` (`extension.flm_version`), and this program \
                     is FLM {FLM_VERSION}"
                ),
                "extension.flm_version",
            )]);
        }
        let Some(engine) = self.engines.get(&manifest.engine) else {
            let engines: Vec<String> = self.engines().iter().map(|name| format!("`{name}`")).collect();
            return Err(vec![in_manifest(
                format!(
                    "the extension `{stem}` specifies the engine `{}` (`extension.engine`), and this program has the \
                     engines {}",
                    manifest.engine,
                    engines.join(", ")
                ),
                "extension.engine",
            )]);
        };
        // The handle the `extension` scope of the `files` host service reads
        // through, taken before the engine owns the files.
        let shared = files.shared();
        let loaded = engine.load(&extension, &manifest, files).map_err(|errors| {
            errors
                .into_iter()
                .map(|error| match error {
                    LoadError::Manifest { path, detail } => in_manifest(detail, &path),
                    LoadError::Load { detail } => ResolveError::new(detail),
                })
                .collect::<Vec<_>>()
        })?;
        let mut errors = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        for report in &loaded.packages {
            if !seen.insert(report.name.as_str()) {
                errors.push(ResolveError::new(format!("{extension} declares the package `{}` twice", report.name)));
            }
        }
        for report in &loaded.packages {
            errors.extend(self.unoffered(&extension, report));
        }
        if !errors.is_empty() {
            return Err(errors);
        }
        let mut digest = serde_json::to_vec(&canonical(value)).unwrap_or_default();
        for report in &loaded.packages {
            for item in std::iter::once(&report.name).chain(&report.host) {
                digest.extend(item.as_bytes());
                digest.push(0);
            }
        }
        digest.extend(loaded.runtime.digest_input());
        let digest: Arc<[u8]> = Arc::from(digest);
        let description = manifest.description.as_deref().unwrap_or_default();
        let mut result = LoadedExtension {
            manifest: manifest.clone(),
            factories: BTreeMap::new(),
            packages: Vec::new(),
            blocks: Vec::new(),
        };
        for report in &loaded.packages {
            let caller =
                HostCaller { extension: extension.clone(), package: report.name.clone(), digest_input: Arc::clone(&digest) };
            let services =
                Arc::new(DeclaredServices::new(caller, &self.host_services, &report.host, Arc::clone(&shared)));
            let runtime = Arc::clone(&loaded.runtime);
            match package_factory(&extension, report, runtime, Arc::clone(&digest), description, services) {
                Ok(factory) => {
                    result.blocks.push(LoadedBlock {
                        package: factory.name().clone(),
                        extension: extension.clone(),
                        block: report.block.clone(),
                    });
                    result.packages.push(report.name.clone());
                    result.factories.insert(report.name.clone(), factory);
                }
                Err(found) => errors.extend(found.into_iter().map(|error| ResolveError::new(error.to_string()))),
            }
        }
        if errors.is_empty() { Ok(Arc::new(result)) } else { Err(errors) }
    }
}

impl PackageResolver for ExtensionLoader {
    /// The factory `name` gives, as the [module documentation](self) states.
    ///
    /// # Errors
    ///
    /// The locator `.`; a source's failure; every problem of the extension
    /// the entry reaches (its manifest, its `flm_version`, its engine, the
    /// engine's load, a package reported twice), each time the extension is
    /// reached; a package the extension does not provide.
    fn resolve(&self, name: &str) -> Result<Option<PackageFactory>, Vec<ResolveError>> {
        let mut state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if let Some(factory) = state.factories.get(name) {
            return Ok(Some(factory.clone()));
        }
        let Ok(entry) = UseEntry::parse(name) else { return Ok(None) };
        let Locator::Named(locator) = &entry.locator else {
            return Err(vec![ResolveError::new(format!(
                "the locator `.` of the entry `{name}` may be written only in the `uses` list of an extension's package"
            ))]);
        };
        let identity = match state.found.get(locator) {
            Some(found) => found.clone(),
            None => {
                let mut found = None;
                for source in &self.sources {
                    let extension = match source.find(&entry.locator) {
                        Ok(Some(extension)) => extension,
                        Ok(None) => continue,
                        Err(error) => {
                            let mut resolve_error = ResolveError::new(error.detail);
                            if let Some(path) = error.path {
                                resolve_error = resolve_error.in_file(path);
                            }
                            if let Some(place) = error.place {
                                resolve_error = resolve_error.at(place.line, place.column);
                            }
                            return Err(vec![resolve_error]);
                        }
                    };
                    let identity = extension.identity.clone();
                    if !state.loaded.contains_key(&identity) {
                        let stem = entry.stem().unwrap_or_default();
                        let place_of = extension.place_of.as_ref();
                        let result = self.load(&identity, stem, &extension.manifest, extension.files, place_of);
                        if let Ok(loaded) = &result {
                            for factory in loaded.factories.values() {
                                state.factories.insert(factory.name().to_string(), factory.clone());
                            }
                        }
                        state.loaded.insert(identity.clone(), result);
                        state.order.push(identity.clone());
                    }
                    found = Some(identity);
                    break;
                }
                state.found.insert(locator.clone(), found.clone());
                found
            }
        };
        let Some(identity) = identity else { return Ok(None) };
        let loaded = match &state.loaded[&identity] {
            Ok(loaded) => Arc::clone(loaded),
            Err(errors) => return Err(errors.clone()),
        };
        match loaded.factories.get(&entry.package) {
            Some(factory) => Ok(Some(factory.clone())),
            None => {
                let extension = loaded.blocks.first().map(|block| block.extension.clone()).unwrap_or_else(|| {
                    ExtensionIdentity { identity: identity.clone(), stem: entry.stem().unwrap_or_default().to_string() }
                });
                let provided: Vec<String> = loaded.packages.iter().map(|package| format!("`{package}`")).collect();
                Err(vec![ResolveError::new(format!(
                    "{extension} provides no package `{}`; it provides {}",
                    entry.package,
                    provided.join(", ")
                ))])
            }
        }
    }

    /// The factory names of the packages loaded so far.
    fn list(&self) -> Option<Vec<Name>> {
        let state = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        Some(state.factories.keys().map(|name| Name::from(name.as_str())).collect())
    }
}

/// `value` with the keys of every map sorted, recursively: the canonical form
/// of a parsed manifest, the same whatever order the host parsed it in.
pub(crate) fn canonical(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let sorted: Map<String, Value> = keys.into_iter().map(|key| (key.clone(), canonical(&map[key]))).collect();
            Value::Object(sorted)
        }
        Value::Array(items) => Value::Array(items.iter().map(canonical).collect()),
        other => other.clone(),
    }
}
