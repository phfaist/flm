//! Features, the units a registry is built from, and packages, the named
//! bundles in which features are shipped and registered together.
//!
//! A **feature** is a Rust value that contributes a related bundle of things to
//! a registry (elements, stages, tables, lowering rules, backends, services,
//! settings) through the feature builder. It implements [`FlmFeature`]: its
//! [`register`](FlmFeature::register) method calls methods of a
//! [`FeatureBuilder`], each of which records one declaration. A feature is
//! usually constructed from a plain Rust value of its own, its
//! [configuration](crate::guide::concepts_overview#settings-options-and-configuration)
//! (for example `Refs::new(RefsConfig { .. })`), so the compiler checks the
//! configuration instead of a schema. The finished registry does not keep the
//! feature value; it keeps only its [`FeatureInfo`]: its name, title,
//! documentation and optional version.
//!
//! A **package** is a named bundle of features together with the packages it
//! builds on. This module defines three forms of a package:
//!
//! - A [`PackageEntry`] is a package ready to register: the packages it uses
//!   and its own features, in order.
//!   [`RegistryBuilder::use_package`](super::RegistryBuilder::use_package)
//!   registers the features of an entry and of the packages it uses, each
//!   package once, however often it is listed.
//! - A [`PackageFactory`] is a package known before it is built: its name, the
//!   names of the packages it uses, its features each with the declaration of
//!   its settings ([`crate::core::settings`]), and the function that builds the
//!   package's entry from the final settings. A program that reads settings
//!   as data reads them with the declarations the factory lists, then builds
//!   the package.
//! - A [`PackageCatalog`] finds the factory of a package from the package's
//!   name: first among the factories compiled into the program, then through
//!   [`PackageResolver`] implementations, asked in order, only when no compiled
//!   factory has that name. A resolver may perform input and output (for
//!   example, find a package file on a search path). This crate performs no
//!   input or output itself, so it declares the trait and implements none.
//!
//! See also [features and
//! packages](crate::guide::concepts_overview#features-and-packages) for the
//! concepts, and [`super::build`] for how a set of features becomes a
//! registry.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

use crate::model::names::{FeatureName, Name};
use crate::core::registry::builder::{BuildError, FeatureBuilder};
use crate::core::registry::order::FeatureChain;
use crate::core::settings::cascade::Reach;
use crate::core::settings::{PackageSettings, SettingsDecl};

/// The description of a feature that a registry keeps: the feature's name,
/// title, documentation and optional version.
///
/// A feature returns this value from [`FlmFeature::info`]. The finished
/// registry returns it from
/// [`FlmRegistry::features`](super::FlmRegistry::features) and
/// [`FlmRegistry::feature`](super::FlmRegistry::feature) after the build,
/// when the registry no longer holds the feature value.
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::FeatureInfo;
///
/// let info = FeatureInfo::new("refs", "Cross-references")
///     .docs("Labels, references, and the table that resolves them.")
///     .version("1.0");
/// assert_eq!(info.name, "refs");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct FeatureInfo {
    /// The feature's registered name, unique within one registry. Every
    /// declaration the feature makes is attributed to this name. By
    /// convention, the names of the entities a feature declares contain it:
    /// the `refs` feature's resolve stage is `flm.refs.resolve`.
    pub name: FeatureName,
    /// A short human-readable title.
    pub title: String,
    /// Documentation prose about the feature as a whole.
    pub docs: String,
    /// A free-form version string, for attribution. It is an input of the
    /// [registry digest](crate::model::digest), so two registries whose
    /// features differ only in version have different digests, and data
    /// cached under one is not read back under the other.
    pub version: Option<String>,
}

impl FeatureInfo {
    /// The description of the feature `name`, titled `title`, with no
    /// documentation and no version.
    pub fn new(name: impl Into<FeatureName>, title: impl Into<String>) -> FeatureInfo {
        FeatureInfo { name: name.into(), title: title.into(), docs: String::new(), version: None }
    }

    /// Sets the documentation prose. Returns `self`, for chaining.
    pub fn docs(mut self, docs: impl Into<String>) -> FeatureInfo {
        self.docs = crate::kept_docs(docs);
        self
    }

    /// Sets the version string. Returns `self`, for chaining.
    pub fn version(mut self, version: impl Into<String>) -> FeatureInfo {
        self.version = Some(version.into());
        self
    }
}

/// A feature: a Rust value that contributes a related bundle of things to a
/// registry (elements, stages, tables, lowering rules, backends, services,
/// settings) through the feature builder.
///
/// Implement this trait on a value that holds the feature's configuration.
/// [`info`](FlmFeature::info) describes the feature, and
/// [`register`](FlmFeature::register) declares what it contributes by calling
/// methods of the [`FeatureBuilder`] it is handed. Then give the value to
/// [`RegistryBuilder::feature`](super::RegistryBuilder::feature), or add it to
/// a [`PackageEntry`]. The trait requires `Send + Sync` so that packages,
/// which hold their features, can be built and shared across threads.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::core::registry::{
///     BuildError, CapabilityDef, FeatureBuilder, FeatureInfo, FlmFeature, build_registry,
/// };
///
/// struct Counting;
///
/// impl FlmFeature for Counting {
///     fn info(&self) -> FeatureInfo {
///         FeatureInfo::new("counting", "Counting")
///     }
///
///     fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
///         b.capability(CapabilityDef::new("counting.done").docs("Counting has run."))
///     }
/// }
///
/// let registry = build_registry([Arc::new(Counting) as Arc<dyn FlmFeature>])?;
/// assert!(registry.has_feature("counting"));
/// assert!(registry.capability_named("counting.done").is_some());
/// # Ok::<(), flm_core::core::registry::RegistryBuildError>(())
/// ```
pub trait FlmFeature: Send + Sync {
    /// The feature's name, title, documentation and version.
    ///
    /// The build calls this method once, before the feature registers. Two
    /// features that return the same name make the build fail with
    /// [`BuildError::DuplicateFeature`].
    fn info(&self) -> FeatureInfo;

    /// Declares everything the feature contributes, by calling methods of `b`.
    ///
    /// This is the first of the build's two registration rounds. Every method
    /// of [`FeatureBuilder`] may be called here, in any order, regardless of
    /// what other features declare: nothing is checked or resolved until every
    /// feature has registered, so a feature never needs to know which other
    /// features are present or which registered first. A feature that must
    /// read what other features contributed to service slots registers a
    /// [`Finalizer`](super::Finalizer), which runs in the second round.
    ///
    /// # Errors
    ///
    /// Return a [`BuildError`] (usually the one a builder method returned) to
    /// refuse registration. The build records a [`BuildError::Feature`]
    /// unchanged, and wraps any other error in a [`BuildError::Feature`]
    /// attributed to this feature. The other features still register, and the
    /// build then fails at the end of the collect phase, the first of its five
    /// phases (see [`super::build`]).
    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError>;

    /// The declaration of the settings the feature takes as data (its
    /// configuration and its option types, each with its binding), or `None`
    /// for a feature that takes no settings as data. The default
    /// implementation returns `None`. See [`crate::core::settings`].
    ///
    /// The registry build declares the option types listed in this
    /// declaration, with their bindings and documentation, before it calls
    /// [`register`](FlmFeature::register); `register` must not declare them
    /// again, or the build reports a duplicate option type. A
    /// [`PackageFactory`] lists this declaration next to the feature's name.
    fn settings(&self) -> Option<SettingsDecl> {
        None
    }
}

/// The error a package's build function returns when it cannot build the
/// package from the given settings; see [`PackageFactory::build`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageError {
    /// What failed, as text for a person to read.
    pub detail: String,
}

impl PackageError {
    /// An error whose text is `detail`.
    pub fn new(detail: impl Into<String>) -> PackageError {
        PackageError { detail: detail.into() }
    }
}

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for PackageError {}

/// One feature of a [`PackageFactory`]: the feature's name, and the
/// declaration of its settings if it takes settings as data, as the feature
/// returns them from [`FlmFeature::info`] and [`FlmFeature::settings`].
#[derive(Debug, Clone)]
pub struct PackageFeature {
    /// The feature's registered name.
    pub name: FeatureName,
    /// The declaration of the feature's settings; `None` for a feature that
    /// takes none.
    pub settings: Option<SettingsDecl>,
}

/// The function a [`PackageFactory`] builds its package with.
type BuildFn = dyn Fn(&PackageSettings) -> Result<PackageEntry, PackageError> + Send + Sync;

/// A package known before it is built: its name, the names of the packages it
/// uses, its features each with the declaration of its settings, and the
/// function that builds the package's [`PackageEntry`] from the final
/// settings.
///
/// A factory serves a program that chooses packages by name and reads their
/// settings as data, for example from a document's front matter. The program
/// reads the settings written for the package's features with the
/// declarations the factory lists ([`settings`](PackageFactory::settings)),
/// combines them over all its sources, and then builds the package once with
/// [`build`](PackageFactory::build). The factory lists the packages it uses by
/// name only: the program builds each of them from its own factory, so the
/// entry that `build` returns should use no package. [`crate::stdcore::factory`]
/// returns the factory of the standard package.
///
/// The factory finds its features by building the package once, when it is
/// created, from empty settings: it lists each feature of that entry, in
/// order, with the name and the settings declaration the feature returns. A
/// feature that the package includes only under some settings is therefore
/// not listed.
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::core::registry::{
///     BuildError, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, PackageFactory,
/// };
/// use flm_core::core::settings::PackageSettings;
///
/// struct Empty;
/// impl FlmFeature for Empty {
///     fn info(&self) -> FeatureInfo { FeatureInfo::new("empty", "Empty") }
///     fn register(&self, _b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> { Ok(()) }
/// }
///
/// let factory = PackageFactory::new("mine", |settings: &PackageSettings| {
///     let mut entry = PackageEntry::new("mine");
///     if !settings.is_off("empty") {
///         entry = entry.feature(Arc::new(Empty));
///     }
///     Ok(entry)
/// })?;
///
/// assert_eq!(factory.features().len(), 1);
/// assert_eq!(factory.features()[0].name, "empty");
/// let entry = factory.build(&PackageSettings::new())?;
/// assert_eq!(entry.features.len(), 1);
/// # Ok::<(), flm_core::core::registry::PackageError>(())
/// ```
#[derive(Clone)]
pub struct PackageFactory {
    name: Name,
    uses: Vec<Name>,
    features: Vec<PackageFeature>,
    build: Arc<BuildFn>,
}

impl PackageFactory {
    /// The factory of the package `name`, whose entry the function `build`
    /// builds; the package uses no other package until
    /// [`uses`](PackageFactory::uses) adds one.
    ///
    /// `build` receives the final settings of the package's features and
    /// returns the package's entry; it should leave out every feature the
    /// settings switch off ([`PackageSettings::is_off`]). This constructor
    /// calls `build` once, with empty settings, and lists the features of the
    /// entry it returns (see [`features`](PackageFactory::features)).
    ///
    /// # Errors
    ///
    /// The [`PackageError`] that `build` returns for empty settings.
    pub fn new(
        name: impl Into<Name>,
        build: impl Fn(&PackageSettings) -> Result<PackageEntry, PackageError> + Send + Sync + 'static,
    ) -> Result<PackageFactory, PackageError> {
        let features = build(&PackageSettings::new())?
            .features
            .iter()
            .map(|feature| PackageFeature { name: feature.info().name, settings: feature.settings() })
            .collect();
        Ok(PackageFactory { name: name.into(), uses: Vec::new(), features, build: Arc::new(build) })
    }

    /// Adds the name of a package this package uses, after those already
    /// added. Returns `self`, for chaining.
    pub fn uses(mut self, package: impl Into<Name>) -> PackageFactory {
        self.uses.push(package.into());
        self
    }

    /// The package's name.
    pub fn name(&self) -> &Name {
        &self.name
    }

    /// The names of the packages this one uses, in the order listed.
    pub fn used(&self) -> &[Name] {
        &self.uses
    }

    /// The package's features, in the package's order: the features of the
    /// entry built from empty settings, each with its name and its settings
    /// declaration.
    pub fn features(&self) -> &[PackageFeature] {
        &self.features
    }

    /// The declaration of the settings of the feature `feature`, or `None`
    /// when the package does not hold that feature or the feature takes no
    /// settings as data.
    pub fn settings(&self, feature: &str) -> Option<&SettingsDecl> {
        self.features
            .iter()
            .find(|entry| entry.name == feature)
            .and_then(|entry| entry.settings.as_ref())
    }

    /// Builds the package's entry from the final settings of its features, by
    /// calling the function given to [`new`](PackageFactory::new).
    ///
    /// # Errors
    ///
    /// The [`PackageError`] that the package's build function returns.
    pub fn build(&self, settings: &PackageSettings) -> Result<PackageEntry, PackageError> {
        (self.build)(settings)
    }
}

impl fmt::Debug for PackageFactory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PackageFactory")
            .field("name", &self.name)
            .field("uses", &self.uses)
            .field("features", &self.features.iter().map(|entry| &entry.name).collect::<Vec<_>>())
            .finish()
    }
}

/// A failure of a [`PackageResolver`] that recognized a package name but could
/// not produce the package's factory (for example, an unreadable file or a
/// malformed definition): what failed and, for a failure in a file, where.
///
/// A resolver that has no package of that name returns `Ok(None)` instead; that
/// is not a failure.
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::ResolveError;
///
/// let error = ResolveError::new("the file is not valid YAML").in_file("mystyle.yaml").at(3, 5);
/// assert_eq!((error.line, error.column), (Some(3), Some(5)));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolveError {
    /// What failed, as text for a person to read.
    pub detail: String,
    /// The file the failure is in, written the way the resolver shows files
    /// to a person, when the failure is in a file.
    pub file: Option<String>,
    /// The line in that file, counted from 1, when known.
    pub line: Option<u32>,
    /// The column in that line, counted from 1, when known.
    pub column: Option<u32>,
}

impl ResolveError {
    /// The failure `detail`, not located in any file.
    pub fn new(detail: impl Into<String>) -> ResolveError {
        ResolveError { detail: detail.into(), file: None, line: None, column: None }
    }

    /// The same failure, located in the file `file`. Returns `self`, for
    /// chaining.
    pub fn in_file(mut self, file: impl Into<String>) -> ResolveError {
        self.file = Some(file.into());
        self
    }

    /// The same failure, located at `line` and `column` (both counted from 1)
    /// of its file. Returns `self`, for chaining.
    pub fn at(mut self, line: u32, column: u32) -> ResolveError {
        self.line = Some(line);
        self.column = Some(column);
        self
    }
}

impl fmt::Display for ResolveError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for ResolveError {}

/// A source of packages that are not compiled into the program: given a
/// package name, it returns the package's [`PackageFactory`].
///
/// A [`PackageCatalog`] asks its resolvers, in the order they were added, only
/// when none of its compiled factories has the name asked for. A resolver may
/// read files or otherwise reach outside the program. The library performs no
/// input or output of its own, so this crate declares the trait and implements
/// none: a program that finds packages on a search path implements it and adds
/// the implementation with [`PackageCatalog::add_resolver`].
pub trait PackageResolver: Send + Sync {
    /// The factory of the package `name`, or `Ok(None)` when this resolver has
    /// no package of that name.
    ///
    /// # Errors
    ///
    /// Every [`ResolveError`] found when the resolver recognized the name but
    /// could not produce a factory; a file may hold several problems. An error
    /// stops the catalog's search: later resolvers are not asked.
    fn resolve(&self, name: &str) -> Result<Option<PackageFactory>, Vec<ResolveError>>;

    /// The package names this resolver can resolve, or `None` when it cannot
    /// list them. A program calls this method to show which packages are
    /// available; nothing in this crate calls it. The default implementation
    /// returns `None`.
    fn list(&self) -> Option<Vec<Name>> {
        None
    }
}

/// A lookup from a package's name to the package's [`PackageFactory`]: first
/// among the factories compiled into the program, then through
/// [`PackageResolver`] implementations, asked in order, only when no compiled
/// factory has the name.
///
/// A program builds a catalog to accept package names chosen at run time, for
/// example from a document's front matter. The registry build never consults
/// a catalog: a catalog only finds factories. The program then builds each
/// package from its factory and passes the entry to
/// [`RegistryBuilder::use_package`](super::RegistryBuilder::use_package), like
/// any other package. Looking up a compiled package never asks a resolver, so
/// a document that lists only compiled packages causes no input or output.
///
/// # Examples
///
/// ```
/// use flm_core::core::registry::{PackageCatalog, PackageEntry, PackageFactory};
///
/// let mut catalog = PackageCatalog::new();
/// catalog.add_compiled(PackageFactory::new("mine", |_| Ok(PackageEntry::new("mine")))?);
///
/// let found = catalog.resolve("mine").expect("no resolver to fail").expect("added above");
/// assert_eq!(found.name(), "mine");
/// assert!(catalog.resolve("absent").expect("no resolver to fail").is_none());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Default, Clone)]
pub struct PackageCatalog {
    compiled: BTreeMap<Name, PackageFactory>,
    resolvers: Vec<Arc<dyn PackageResolver>>,
}

impl PackageCatalog {
    /// A catalog with no factories and no resolvers.
    pub fn new() -> PackageCatalog {
        PackageCatalog::default()
    }

    /// Adds a factory compiled into the program, replacing any compiled
    /// factory already added under the same name. Returns `&mut self`, for
    /// chaining.
    pub fn add_compiled(&mut self, factory: PackageFactory) -> &mut PackageCatalog {
        self.compiled.insert(factory.name.clone(), factory);
        self
    }

    /// Adds a resolver, to be asked after every resolver already added.
    /// Returns `&mut self`, for chaining.
    pub fn add_resolver(&mut self, resolver: Arc<dyn PackageResolver>) -> &mut PackageCatalog {
        self.resolvers.push(resolver);
        self
    }

    /// The names of the compiled factories, in name order. The resolvers are
    /// not asked; [`PackageResolver::list`] lists the package names a resolver
    /// can resolve, when it can list them.
    pub fn compiled_names(&self) -> impl Iterator<Item = &Name> + '_ {
        self.compiled.keys()
    }

    /// The factory of the package `name`: the compiled factory of that name if
    /// there is one, otherwise the factory returned by the first resolver that
    /// knows the name. `Ok(None)` when no compiled factory and no resolver
    /// knows the name.
    ///
    /// # Errors
    ///
    /// The [`ResolveError`]s of the first resolver that fails; later resolvers
    /// are not asked.
    pub fn resolve(&self, name: &str) -> Result<Option<PackageFactory>, Vec<ResolveError>> {
        if let Some(factory) = self.compiled.get(name) {
            return Ok(Some(factory.clone()));
        }
        for resolver in &self.resolvers {
            if let Some(factory) = resolver.resolve(name)? {
                return Ok(Some(factory));
            }
        }
        Ok(None)
    }
}

impl PackageCatalog {
    /// The factories of the packages listed in `names` and of every package
    /// they use, directly or not, in the order in which the packages should
    /// register: each package after the packages it uses, in the order it
    /// lists them, and each package once, however often it is listed or used.
    /// The names are taken in the order given.
    ///
    /// A package is identified by its factory's [name](PackageFactory::name),
    /// so two names that lead to one factory (for example, a resolver that
    /// finds one file under two spellings) are one package. A package that uses
    /// itself, directly or through others, is accepted: the walk skips a
    /// package it has already found or is currently walking the uses of.
    /// Packages that use each other therefore register in the order the walk
    /// reaches them, and every other package still registers after the
    /// packages it uses.
    ///
    /// # Errors
    ///
    /// Every problem found, each a [`UseError`] that identifies the package at
    /// fault and the position in `names` of the name whose walk reached it:
    /// a name that neither the compiled factories nor the resolvers know, or a
    /// resolver's failure.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::core::registry::{PackageCatalog, PackageEntry, PackageFactory};
    ///
    /// let mut catalog = PackageCatalog::new();
    /// catalog.add_compiled(PackageFactory::new("base", |_| Ok(PackageEntry::new("base")))?);
    /// catalog.add_compiled(PackageFactory::new("mine", |_| Ok(PackageEntry::new("mine")))?.uses("base"));
    ///
    /// let found = catalog.resolve_all(&["mine", "base"]).expect("both known");
    /// let names: Vec<&str> = found.iter().map(|factory| factory.name().as_str()).collect();
    /// assert_eq!(names, ["base", "mine"]);
    /// assert_eq!(catalog.resolve_all(&["absent"]).unwrap_err()[0].index, 0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn resolve_all(&self, names: &[&str]) -> Result<Vec<PackageFactory>, Vec<UseError>> {
        let mut walk = UseWalk { catalog: self, path: Vec::new(), found: Vec::new(), errors: Vec::new() };
        for (index, name) in names.iter().enumerate() {
            walk.add(index, name);
        }
        if walk.errors.is_empty() { Ok(walk.found) } else { Err(walk.errors) }
    }
}

impl PackageCatalog {
    /// Which package each of `factories` reaches: for each factory, by its
    /// name, the names of the factories of the packages it uses, directly or
    /// through others. Each used name is resolved through this catalog (a name
    /// that does not resolve is left out: [`resolve_all`](PackageCatalog::resolve_all)
    /// reported it). A package in a loop reaches itself.
    ///
    /// A package is identified by its factory's name, so two spellings of one
    /// package (two names a resolver finds one factory for) reach and are
    /// reached as one; of two factories met under one name (one given here,
    /// one the catalog finds), the first met is kept. A host gives the result to the settings cascade
    /// ([`Cascade::with_reach`](crate::core::settings::cascade::Cascade::with_reach)),
    /// in which a package wins over the packages it reaches.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::core::registry::{PackageCatalog, PackageEntry, PackageFactory};
    ///
    /// let factory = |name: &str| PackageFactory::new(name, |_| Ok(PackageEntry::new("p")));
    /// let mut catalog = PackageCatalog::new();
    /// catalog.add_compiled(factory("base")?);
    /// catalog.add_compiled(factory("mid")?.uses("base"));
    /// let mine = factory("mine")?.uses("mid");
    ///
    /// let reach = catalog.reach(&[mine]);
    /// let reached: Vec<&str> = reach["mine"].iter().map(|name| name.as_str()).collect();
    /// assert_eq!(reached, ["base", "mid"]);
    /// # Ok::<(), flm_core::core::registry::PackageError>(())
    /// ```
    pub fn reach(&self, factories: &[PackageFactory]) -> Reach {
        // The names of the factories each package met uses directly, resolved.
        let mut direct: BTreeMap<Name, Vec<Name>> = BTreeMap::new();
        let mut pending: Vec<PackageFactory> = factories.to_vec();
        while let Some(factory) = pending.pop() {
            if direct.contains_key(factory.name()) {
                continue;
            }
            let mut used = Vec::new();
            for name in factory.used() {
                if let Ok(Some(found)) = self.resolve(name.as_str()) {
                    used.push(found.name().clone());
                    if !direct.contains_key(found.name()) {
                        pending.push(found);
                    }
                }
            }
            direct.insert(factory.name().clone(), used);
        }
        factories
            .iter()
            .map(|factory| {
                let mut reached = BTreeSet::new();
                let mut next: Vec<&Name> = direct[factory.name()].iter().collect();
                while let Some(name) = next.pop() {
                    if reached.insert(name.clone()) {
                        next.extend(direct.get(name).into_iter().flatten());
                    }
                }
                (factory.name().clone(), reached)
            })
            .collect()
    }
}

/// The state of the walk that [`PackageCatalog::resolve_all`] makes over the
/// packages and the packages they use.
// A loop of packages is skipped, not refused: its one harmful content, two
// packages disagreeing on a value, is a problem of the settings, reported there.
// See decision rationale at [§flm-dd-dr:feat-import-once].
struct UseWalk<'c> {
    catalog: &'c PackageCatalog,
    /// The names of the factories of the packages being added, each package
    /// using the next.
    path: Vec<Name>,
    found: Vec<PackageFactory>,
    errors: Vec<UseError>,
}

impl UseWalk<'_> {
    /// Whether the package whose factory has the name `name` was added
    /// already or is being added.
    fn met(&self, name: &str) -> bool {
        self.found.iter().any(|factory| factory.name() == name) || self.path.iter().any(|entry| entry == name)
    }

    /// Adds the package `name` after the packages it uses, unless it was added
    /// already or is being added; `index` is the position, in the names given
    /// to `resolve_all`, of the name the walk started from.
    fn add(&mut self, index: usize, name: &str) {
        if self.met(name) {
            return;
        }
        let used_by = self.path.last().cloned();
        let error = |kind: UseErrorKind| UseError { index, package: Name::from(name), used_by: used_by.clone(), kind };
        let factory = match self.catalog.resolve(name) {
            Ok(Some(factory)) => factory,
            Ok(None) => {
                let known = self.catalog.compiled_names().cloned().collect();
                self.errors.push(error(UseErrorKind::Unknown { known }));
                return;
            }
            Err(failures) => {
                self.errors.extend(failures.into_iter().map(|failure| error(UseErrorKind::Resolver(failure))));
                return;
            }
        };
        // The name asked for may be another spelling of a package met already.
        if self.met(factory.name()) {
            return;
        }
        self.path.push(factory.name().clone());
        for used in factory.used() {
            self.add(index, used.as_str());
        }
        self.path.pop();
        self.found.push(factory);
    }
}

/// A problem found by [`PackageCatalog::resolve_all`]: the package at fault,
/// the position of the name whose walk reached it, and what is wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UseError {
    /// The position, in the names given to `resolve_all`, of the name whose
    /// walk reached the problem: the problem is in that name's package, or in
    /// a package it uses, directly or not.
    pub index: usize,
    /// The package at fault.
    pub package: Name,
    /// The package that builds on the package at fault, or `None` when the
    /// package at fault is one of the names given to `resolve_all`.
    pub used_by: Option<Name>,
    /// What is wrong.
    pub kind: UseErrorKind,
}

/// What is wrong in a [`UseError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UseErrorKind {
    /// No compiled factory and no resolver knows the name; `known` lists the
    /// names of the compiled factories.
    Unknown {
        /// The names of the compiled factories, in name order.
        known: Vec<Name>,
    },
    /// A resolver knew the name and failed.
    Resolver(ResolveError),
}

impl fmt::Display for UseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let package = &self.package;
        let joined = |names: &[Name]| names.iter().map(Name::as_str).collect::<Vec<_>>().join("`, `");
        match &self.kind {
            UseErrorKind::Unknown { known } => {
                match &self.used_by {
                    Some(used_by) => write!(
                        f,
                        "package `{used_by}` builds on the package `{package}`, but no package of \
                         that name is compiled into the program or can be loaded; packages \
                         compiled into the program: `{}`",
                        joined(known)
                    ),
                    None => write!(
                        f,
                        "the package `{package}` is requested, but no package of that name is \
                         compiled into the program or can be loaded; packages compiled into the \
                         program: `{}`",
                        joined(known)
                    ),
                }
            }
            UseErrorKind::Resolver(failure) => match &self.used_by {
                Some(used_by) => write!(
                    f,
                    "the package `{package}`, which package `{used_by}` builds on, could not be \
                     loaded: {failure}"
                ),
                None => write!(f, "the package `{package}` could not be loaded: {failure}"),
            },
        }
    }
}

impl std::error::Error for UseError {}

impl fmt::Debug for PackageCatalog {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PackageCatalog")
            .field("compiled", &self.compiled.keys().collect::<Vec<_>>())
            .field("resolvers", &self.resolvers.len())
            .finish()
    }
}

/// A package ready to register: a named bundle of features together with the
/// packages it builds on.
///
/// A package is the unit in which features are shipped and registered
/// together; [`crate::stdcore::package`] returns the standard package. Pass a
/// package to
/// [`RegistryBuilder::use_package`](super::RegistryBuilder::use_package),
/// which adds the features of the packages it uses first, then its own
/// features, in order, together with the chains of the feature order each
/// package states. The registry builder adds a package at most once,
/// however many times it is passed or used, so two packages may each use a
/// third and its features are still registered once. The order in which the
/// features are added is their implicit order; the chains may change it
/// ([`super::order`]).
///
/// # Examples
///
/// ```
/// use std::sync::Arc;
/// use flm_core::core::registry::{
///     BuildError, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry,
/// };
///
/// struct Base;
/// impl FlmFeature for Base {
///     fn info(&self) -> FeatureInfo { FeatureInfo::new("base", "Base") }
///     fn register(&self, _b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> { Ok(()) }
/// }
///
/// let base = PackageEntry::new("base").feature(Arc::new(Base));
/// let mine = PackageEntry::new("mine").uses(base);
/// assert_eq!(mine.uses.len(), 1);
/// ```
#[derive(Clone)]
pub struct PackageEntry {
    /// The package's name. The registry builder recognizes a package it has
    /// already registered by this name, so it treats two entries of the same
    /// name as the same package.
    pub name: Name,
    /// The packages this package uses; their features are registered before
    /// this package's own features.
    pub uses: Vec<PackageEntry>,
    /// The package's own features, in the order they are added to the
    /// registry builder.
    pub features: Vec<Arc<dyn FlmFeature>>,
    /// The chains of the feature order that the package states, such as
    /// `math-mathjax < phfqit` for a package whose definitions are to apply
    /// over those of `math-mathjax`. They may name the features of any
    /// package; the registry build combines them with every other chain it is
    /// given ([`super::order`]).
    pub feature_order: Vec<FeatureChain>,
}

impl PackageEntry {
    /// A package called `name`, which uses no package and holds no feature
    /// yet.
    pub fn new(name: impl Into<Name>) -> PackageEntry {
        PackageEntry {
            name: name.into(),
            uses: Vec::new(),
            features: Vec::new(),
            feature_order: Vec::new(),
        }
    }

    /// Adds a package this package uses, after those already added. Returns
    /// `self`, for chaining.
    pub fn uses(mut self, package: PackageEntry) -> PackageEntry {
        self.uses.push(package);
        self
    }

    /// Adds one of this package's own features, after those already added.
    /// Returns `self`, for chaining.
    pub fn feature(mut self, feature: Arc<dyn FlmFeature>) -> PackageEntry {
        self.features.push(feature);
        self
    }

    /// Adds one chain of the feature order that this package states, after
    /// those already added. Returns `self`, for chaining.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::core::registry::{FeatureChain, PackageEntry};
    ///
    /// let chain = FeatureChain::parse("math-mathjax < mine", "package mine")?;
    /// let mine = PackageEntry::new("mine").feature_order(chain);
    /// assert_eq!(mine.feature_order.len(), 1);
    /// # Ok::<(), flm_core::core::registry::ChainError>(())
    /// ```
    pub fn feature_order(mut self, chain: FeatureChain) -> PackageEntry {
        self.feature_order.push(chain);
        self
    }
}

impl fmt::Debug for PackageEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PackageEntry")
            .field("name", &self.name)
            .field("uses", &self.uses.iter().map(|p| &p.name).collect::<Vec<_>>())
            .field("features", &self.features.iter().map(|x| x.info().name).collect::<Vec<_>>())
            .field("feature_order", &self.feature_order.iter().map(ToString::to_string).collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A resolver that knows the factories it holds under one or more names each.
    struct Spellings(Vec<(&'static str, PackageFactory)>);

    impl PackageResolver for Spellings {
        fn resolve(&self, name: &str) -> Result<Option<PackageFactory>, Vec<ResolveError>> {
            Ok(self.0.iter().find(|(spelling, _)| *spelling == name).map(|(_, factory)| factory.clone()))
        }
    }

    fn factory(name: &str, uses: &[&str]) -> PackageFactory {
        let mut factory = PackageFactory::new(name, |_| Ok(PackageEntry::new("p"))).unwrap();
        for used in uses {
            factory = factory.uses(*used);
        }
        factory
    }

    fn names(reach: &Reach, from: &str) -> Vec<String> {
        reach[from].iter().map(|name| name.to_string()).collect()
    }

    #[test]
    fn the_reach_follows_uses_through_others_and_a_loop_reaches_itself() {
        let mut catalog = PackageCatalog::new();
        // `a` uses `b`, which uses `c` and the unknown `nosuch`; `c` uses `b`, a loop.
        catalog.add_compiled(factory("b", &["c", "nosuch"]));
        catalog.add_compiled(factory("c", &["b"]));
        let (a, d) = (factory("a", &["b"]), factory("d", &[]));
        let reach = catalog.reach(&[a, d, factory("b", &["c", "nosuch"])]);
        assert_eq!(names(&reach, "a"), ["b", "c"]);
        assert_eq!(names(&reach, "b"), ["b", "c"]);
        assert!(reach["d"].is_empty());
        // Only the factories given are keys.
        assert_eq!(reach.keys().map(Name::as_str).collect::<Vec<_>>(), ["a", "b", "d"]);
    }

    #[test]
    fn the_reach_keys_two_spellings_of_one_package_by_its_factory_name() {
        let shared = factory("lib/shared.yaml", &[]);
        let mut catalog = PackageCatalog::new();
        catalog.add_resolver(Arc::new(Spellings(vec![("shared", shared.clone()), ("./lib/shared", shared)])));
        let one = factory("one.yaml", &["shared"]);
        let two = factory("two.yaml", &["./lib/shared", "shared"]);
        let reach = catalog.reach(&[one, two]);
        assert_eq!(names(&reach, "one.yaml"), ["lib/shared.yaml"]);
        assert_eq!(names(&reach, "two.yaml"), ["lib/shared.yaml"]);
    }
}
