//! Finding the packages a document uses, compiled packages and the packages of extensions,
//! in the order in which they are registered.
//!
//! A [package](flm_core::core::registry::PackageEntry) is a named bundle of features
//! together with the packages it builds on. A *compiled* package is written in Rust and
//! carried by the program. An *extension* is found outside the program, on the host's
//! search path, by the locator of a `use` entry (`myext`, `myext#svg`): a manifest and its
//! files, which an engine turns into one or more packages ([`crate::extensions`] finds an
//! extension and reads its files; the library [`flm_ext`] loads it). The host looks
//! packages up through the library's [`PackageCatalog`], made by [`catalog`]: the library's
//! standard package `stdcore`, the host's own packages `stdmain` and `std`
//! ([`crate::stdpkg`]), the library's package `math-passthrough`
//! ([`flm_core::math_passthrough`], one of the packages that process mathematics), the
//! compiled packages the program carries, and, for every other
//! name, the [`ExtensionLoader`] of the preparation ([`loader`]), its one resolver. A name is
//! looked up among the compiled packages first, so an extension never replaces a compiled
//! package.
//!
//! [`resolve`] asks the catalog for the packages a document uses: first those of the
//! *initial list*, then the package of the *math processor*, then those its front matter
//! lists under `use`, then those that the request adds, then the package of the *profile*
//! when it exists.
//!
//! - The initial list is the one given by the request ([`Request::initial_use`], the
//!   command line's `--initial-use`), else the front matter's `initial_use`, else the
//!   host's default ([`Request::default_initial_use`], `std` unless the host gives
//!   another); the list given is used exactly as given.
//! - The math processor is the package that processes the document's mathematics
//!   ([`MathChoice`](crate::seed::MathChoice)). It is chosen apart from the initial list: by the request
//!   ([`Request::math`], the command line's `--math`), else by the front matter's `math`,
//!   else by the host's default ([`Request::default_math`], no processor unless the host
//!   sets one). The processor `NAME` adds the package `math-NAME`, found as an entry of
//!   `use` is; the choice of no processor adds nothing.
//! - The packages that the request adds ([`Request::uses`], the command line's `--use`)
//!   are used as if the front matter listed them last under `use`, each found as an entry
//!   of `use` is.
//! - The profile is the name that chooses the workflow that runs
//!   ([`Request::profile`], else the front matter's `default_profile`, else `html`;
//!   [`crate::session`], "Which profile runs, and its workflow"). The profile `NAME` adds the
//!   package `profile-NAME` when the catalog knows that name, found as an entry of `use`
//!   is; when no compiled package, no folder of the search path and no bundled extension
//!   holds it, the profile adds nothing.
//!
//! The catalog follows the packages each of
//! them uses and orders them, each after every package it uses and each only once
//! ([`PackageCatalog::resolve_all`]). The host places each problem where it belongs:
//!
//! - a problem in an extension's manifest (a manifest that is not valid YAML, a key it does
//!   not admit, a value of the wrong shape) is placed in the manifest's file, at its line
//!   when the problem is a YAML one, else in the file alone (the message names the key);
//! - a name that no package has is placed at the entry that names it: in the document's
//!   front matter (under `use` or `initial_use`, or at the value of `math` for the math
//!   processor's package), or in the manifest of the extension whose package uses it; an
//!   entry that no file holds (an entry of an initial list or of the packages added, or a
//!   math processor, that the request or the host gives, and the profile's package) is
//!   placed nowhere, its message opening with where the entry comes from
//!   ([`GivenList::source`](crate::session::GivenList::source): "in the option
//!   `--initial-use`, …", "in the option `--use`, …";
//!   [`GivenMath::source`](crate::session::GivenMath::source);
//!   [`DEFAULT_MATH_PROCESSOR`](crate::session::DEFAULT_MATH_PROCESSOR); "in the profile
//!   `NAME`, …");
//! - any other problem (an extension that fails to load, a package an extension does not
//!   provide, a folder holding an extension in both layouts) is placed where the document
//!   lists the package that led to it (nowhere for an entry the request or the host gives).
//!
//! [`suggestions`] lists what the JSON Schemas of a front matter and of a manifest suggest
//! for an entry of `use` and of `initial_use` ([`crate::seed::use_schema`]): the compiled
//! packages, the extensions installed on the search path
//! ([`ExtensionFinder::locators`]) and the bundled extensions, by their locators, nothing
//! loaded. [`session::prepare`](crate::session::prepare) keeps those of the document's search
//! path ([`Prepared::suggestions`](crate::session::Prepared::suggestions)).
//!
//! The settings a package of an extension writes (its block: `features`, `parsing`,
//! `workflows`) are a settings source of the packages' rank ([`crate::settings`]): a
//! package wins over the packages it uses, directly or through others, and two packages of
//! which neither uses the other are peers that must agree. The catalog computes which
//! package reaches which ([`PackageCatalog::reach`]); [`crate::settings`] applies the
//! rule. A workflow that a package writes is combined with the workflow of the same name
//! that the compiled packages define, if they define one: the fields that result from the
//! packages' values replace that workflow's fields. The host gives the library the
//! resulting fields, once per workflow
//! ([`workflow_def`](flm_core::core::settings::cascade::workflow_def)), and the library
//! combines them
//! ([`RegistryBuilder::workflow_fields`](flm_core::core::registry::RegistryBuilder::workflow_fields)).

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use flm_core::core::registry::{PackageCatalog, PackageError, PackageFactory, UseErrorKind};
use flm_ext::ExtensionLoader;
use flm_ext::source::ExtensionSource;
use serde_json::Value;

use crate::extensions::ExtensionFinder;
use crate::files::ServiceFiles;
use crate::host_files::FilesService;
use crate::seed::{NodeValue, Place};
use crate::session::Request;
use crate::stdpkg;

/// Where an entry of the list that [`resolve`] takes is written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntrySource {
    /// In the file that lists the packages (the document's), at this place.
    File(Place),
    /// In no file: in an initial list, a list of packages added, or a choice of math
    /// processor, that the request or the host gives, or the package of the profile that
    /// runs, which these words name
    /// ([`GivenList::source`](crate::session::GivenList::source),
    /// [`GivenMath::source`](crate::session::GivenMath::source),
    /// [`DEFAULT_MATH_PROCESSOR`](crate::session::DEFAULT_MATH_PROCESSOR), "the profile
    /// `NAME`"); a problem that stays at the entry opens with them.
    Given(String),
}

/// A problem finding or reading a package: the package's name, the file and place of the
/// problem, and the message. Its `Display` text is [`detail`](PackageProblem::detail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageProblem {
    /// The package's name.
    pub name: String,
    /// The file the problem is in: an extension's manifest for a problem inside it, else
    /// the file that lists the package; `None` for the document itself when it was given as
    /// text, or for a problem that is in no file.
    pub file: Option<String>,
    /// Where in that file, when known.
    pub place: Option<Place>,
    /// The problem, as a text for a reader.
    pub detail: String,
}

impl fmt::Display for PackageProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for PackageProblem {}

/// One folder of the search path, where extensions are looked for by their locator
/// ([`crate::extensions`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ExtensionFolder {
    /// The folder, relative to the current folder or absolute.
    pub path: PathBuf,
    /// Whether the folder must exist: `true` for a folder the user gave, `false` for one
    /// the program looks in by default. A required folder that does not exist stops
    /// [`prepare`](crate::session::prepare).
    pub required: bool,
}

/// Returns the compiled packages of one command, in this order: the library's `stdcore`,
/// the host's `stdmain` (`stdmain`, made by [`stdpkg::stdmain_factory`] from the
/// command's parts), the host's `std` ([`stdpkg::factory`]), the library's
/// `math-passthrough` ([`flm_core::math_passthrough::factory`]), and `compiled`, the
/// compiled packages the program carries.
///
/// # Errors
///
/// The [`PackageError`] of the factory of `stdcore`, `std` or `math-passthrough`, which do
/// not fail in practice.
pub fn carried(stdmain: PackageFactory, compiled: &[PackageFactory]) -> Result<Vec<PackageFactory>, PackageError> {
    let mut factories =
        vec![flm_core::stdcore::factory()?, stdmain, stdpkg::factory()?, flm_core::math_passthrough::factory()?];
    factories.extend(compiled.iter().cloned());
    Ok(factories)
}

/// Returns the loader of one preparation: `finder` its first source and, under the cargo
/// feature `bundled-extensions`, the extensions compiled into the program
/// ([`BundledSource`](crate::bundled::BundledSource)) its last, sharing the finder's
/// [`ParsedManifests`](crate::extensions::ParsedManifests), together with the engine
/// `native` ([`NativeEngine`](flm_ext::native::NativeEngine)) holding the extensions written
/// in Rust among them (`flm_extensions::NATIVE_EXTENSIONS`); with the data engine,
/// under the cargo feature `js`, the engine `js` over the request's
/// [`js_host`](crate::session::Request::js_host)
/// ([`ExtensionLoader::with_js`](flm_ext::ExtensionLoader::with_js)); and, under the cargo
/// feature `wasm`, the engine `wasm` over the request's `wasm_host`
/// (`ExtensionLoader::with_wasm`). Without a feature, an extension whose manifest names
/// its engine fails its load with the loader's message for an engine the program does not
/// have.
///
/// The loader offers the host service `files` ([`FilesService`]) over the folder
/// `document`, reading through `files`, the file reader of the registry's services, so
/// that each read is recorded among the files of the build under way; and, when the request
/// asks for it ([`Request::offer_programs`](crate::session::Request::offer_programs), cargo
/// feature `fs`), the host service `programs`
/// ([`ProgramsService`](crate::host_programs::ProgramsService)).
pub fn loader(
    finder: &Arc<ExtensionFinder>,
    request: &Request,
    files: &ServiceFiles,
    document: &Path,
) -> Arc<ExtensionLoader> {
    let source: Arc<dyn ExtensionSource> = Arc::clone(finder) as Arc<dyn ExtensionSource>;
    #[allow(unused_mut)]
    let mut sources = vec![source];
    #[cfg(feature = "bundled-extensions")]
    sources.push(Arc::new(crate::bundled::BundledSource::new(Arc::clone(finder.parsed_manifests()))));
    let loader = ExtensionLoader::new(sources)
        .with_host_service(Arc::new(FilesService::new(files.clone(), document.to_path_buf())));
    #[cfg(feature = "bundled-extensions")]
    let loader = loader.with_engine(Arc::new(flm_ext::native::NativeEngine::new(
        flm_extensions::NATIVE_EXTENSIONS.iter().copied(),
    )));
    #[cfg(feature = "fs")]
    let loader = if request.offer_programs {
        loader.with_host_service(Arc::new(crate::host_programs::ProgramsService::new()))
    } else {
        loader
    };
    #[cfg(feature = "js")]
    let loader = loader.with_js(request.js_host.clone());
    #[cfg(feature = "wasm")]
    let loader = loader.with_wasm(request.wasm_host.clone());
    #[cfg(not(any(feature = "js", feature = "wasm")))]
    let _ = request;
    Arc::new(loader)
}

/// Returns the package catalog of one command: the compiled packages of [`carried`]
/// (`stdmain` being the host's package made from the command's parts), and `loader`, which
/// the catalog asks for every other name.
///
/// # Errors
///
/// The [`PackageError`] of the factory of `stdcore`, `std` or `math-passthrough`, which do
/// not fail in practice.
pub fn catalog(
    stdmain: PackageFactory,
    compiled: &[PackageFactory],
    loader: Arc<ExtensionLoader>,
) -> Result<PackageCatalog, PackageError> {
    let mut catalog = PackageCatalog::new();
    for factory in carried(stdmain, compiled)? {
        catalog.add_compiled(factory);
    }
    catalog.add_resolver(loader);
    Ok(catalog)
}

/// Returns the values that the JSON Schemas suggest for an entry of `use` and of
/// `initial_use`, in the order in which a name is looked up, each once: `compiled`, the
/// names of the packages compiled into the program, in name order; the locators of the
/// extensions installed in the folders of `finder`'s search path
/// ([`ExtensionFinder::locators`]); and, under the cargo feature `bundled-extensions`, the
/// stems of the extensions compiled into the program
/// ([`bundled::stems`](crate::bundled::stems)).
///
/// Nothing is loaded: an extension is suggested by its locator, and the packages it provides
/// besides its default one (`LOCATOR#PACKAGE`), which only its loading tells, are not. The
/// listing reports no problem ([`ExtensionFinder::locators`]).
pub fn suggestions<'a>(compiled: impl IntoIterator<Item = &'a str>, finder: &ExtensionFinder) -> Vec<String> {
    let mut names: Vec<String> = compiled.into_iter().map(str::to_string).collect();
    names.sort();
    let mut suggestions: Vec<String> = Vec::new();
    #[allow(unused_mut)]
    let mut found = finder.locators();
    #[cfg(feature = "bundled-extensions")]
    found.extend(crate::bundled::stems().into_iter().map(str::to_string));
    for name in names.into_iter().chain(found) {
        if !suggestions.contains(&name) {
            suggestions.push(name);
        }
    }
    suggestions
}

/// Returns the JSON Schema of an extension's manifest, for the features of the package
/// factories `factories`: a complete schema, with the types it refers to under `$defs`.
///
/// It admits at the root the two keys of the manifest ([`crate::extensions`]) and no other:
/// `extension`, with its six keys (the keys of the engines `js` and `wasm` with their
/// limits among them, whatever cargo features the program has), and `flm`, the block the engine `data` reads: `use`,
/// `features`, `parsing` and `feature_order` as in a front matter
/// ([`crate::seed::use_schema`], [`crate::seed::features_schema`],
/// [`crate::seed::parsing_schema`], [`crate::seed::feature_order_schema`]), and `workflows`,
/// each workflow's fields (`feature_order` is not one of them), its `settings` being a map
/// of features' settings that cannot switch a feature off. `use` suggests the values
/// `suggestions` ([`suggestions`]), as a front matter's does. The `flm` program writes this
/// schema with `flm schema`.
pub fn extension_schema(factories: &[&PackageFactory], suggestions: &[String]) -> Value {
    let mut defs = serde_json::Map::new();
    let features = crate::seed::features_schema(factories, true, &mut defs);
    let settings = crate::seed::features_schema(factories, false, &mut defs);
    let names = |description: &str| {
        serde_json::json!({ "description": description, "type": "array", "items": { "type": "string" } })
    };
    let text = |description: &str| serde_json::json!({ "description": description, "type": "string" });
    let whole = |description: &str| serde_json::json!({ "description": description, "type": "integer", "minimum": 1 });
    let limits = |description: &str, properties: Value| {
        serde_json::json!({
            "description": description,
            "type": "object",
            "properties": properties,
            "additionalProperties": false,
        })
    };
    let workflow = serde_json::json!({
        "type": "object",
        "properties": {
            "extends": text("The workflow this one starts from."),
            "outputs": names("The outputs the workflow delivers."),
            "demands": names("Capabilities the workflow asks for, in addition to those of its outputs."),
            "prefer": names("The stages the workflow prefers where several stages could provide the same capability, most preferred first."),
            "settings": settings,
            "docs": text("A text describing the workflow."),
        },
        "additionalProperties": false,
    });
    let extension = serde_json::json!({
        "description": "What the extension is: its description, its author, the versions of FLM it works \
            with, and the engine that loads it.",
        "type": ["object", "null"],
        "properties": {
            "description": text("A text describing the extension; the title of the compose templates it ships."),
            "author": text("The extension's author."),
            "flm_version": text("The versions of FLM the extension works with, as a version requirement \
                (`>=0.4`, `^1.2`); absent, every version."),
            "engine": {
                "description": "The engine that loads the extension: `data` (the default), an extension \
                    written as data whose package is the block `flm`; `js`, an extension written in \
                    JavaScript whose main module is `js.main`; or `wasm`, an extension compiled to \
                    WebAssembly whose module is `wasm.module`.",
                "enum": ["data", "js", "wasm"],
            },
            "js": {
                "description": "For the engine `js`: the extension's main module, and the limits of its \
                    engine instances.",
                "type": "object",
                "properties": {
                    "main": text("The path of the main module, relative to the extension's folder."),
                    "limits": limits(
                        "The limits every engine instance of the extension is created with, each in place of \
                            the program's; an absent one keeps the program's.",
                        serde_json::json!({
                            "memory_mib": whole("The memory an engine instance may allocate, in MiB."),
                            "stack_mib": {
                                "description": "The JavaScript stack an engine instance may use, in MiB; below \
                                    the stack of the threads that run the extension's handlers.",
                                "type": "integer",
                                "minimum": 1,
                                "maximum": flm_ext::manifest::MAX_STACK_MIB - 1,
                            },
                            "time_s": whole("The time one handler call may run, in seconds."),
                        }),
                    ),
                },
                "additionalProperties": false,
            },
            "wasm": {
                "description": "For the engine `wasm`: the extension's module, and the limits of its \
                    instances.",
                "type": "object",
                "properties": {
                    "module": text("The path of the WebAssembly module, relative to the extension's folder."),
                    "limits": limits(
                        "The limits every instance of the module is created with, each in place of the \
                            program's; an absent one keeps the program's.",
                        serde_json::json!({
                            "memory_mib": whole("The memory an instance may have, in MiB."),
                            "fuel": whole("The fuel one handler call may consume, in units of about one \
                                instruction."),
                        }),
                    ),
                },
                "additionalProperties": false,
            },
        },
        "additionalProperties": false,
    });
    let flm = serde_json::json!({
        "description": "The package of an extension written as data: the packages it uses, the settings \
            of features, the parsing settings, the workflows it defines or changes, and the order of \
            features it needs. The engine `data` reads it; every other engine refuses it.",
        "type": ["object", "null"],
        "properties": {
            "use": crate::seed::use_schema(suggestions),
            "features": features,
            "parsing": crate::seed::parsing_schema(),
            "feature_order": crate::seed::feature_order_schema(),
            "workflows": {
                "description": "The workflows the package defines or changes: a map from a workflow's name \
                    to its fields. The fields a package writes for a workflow the compiled packages define \
                    replace theirs; a workflow's settings hold only keys that are set per render request.",
                "type": "object",
                "additionalProperties": workflow,
            },
        },
        "additionalProperties": false,
    });
    let schema = serde_json::json!({
        "description": "The manifest of an extension: what the extension is, and, for an extension written \
            as data, its package.",
        "type": ["object", "null"],
        "properties": { "extension": extension, "flm": flm },
        "additionalProperties": false,
    });
    crate::seed::document(schema, defs)
}

/// Returns the package factories of a document that uses the packages `named`, in order:
/// the entries of the initial list, then the math processor's package, then those of the
/// front matter's `use`, then the profile's package when it exists (each entry as
/// the catalog is asked for it, with its place in the file `file` when it is written
/// there; `file` is `None` for a document given as text). Each package comes after the
/// packages it uses, and each package comes once. `loader` is the catalog's resolver and
/// `finder` its source; they place a problem in the manifest of the extension that lists
/// the package at fault.
///
/// # Errors
///
/// Every problem the catalog finds, each placed as the [module documentation](self) says.
pub fn resolve(
    catalog: &PackageCatalog,
    loader: &ExtensionLoader,
    finder: &ExtensionFinder,
    named: &[(String, EntrySource)],
    file: Option<&str>,
) -> Result<Vec<PackageFactory>, Vec<PackageProblem>> {
    let names: Vec<&str> = named.iter().map(|(name, _)| name.as_str()).collect();
    catalog.resolve_all(&names).map_err(|errors| {
        errors
            .into_iter()
            .map(|error| {
                let name = error.package.to_string();
                // An entry written in the file is placed there; one that the request or the
                // host gives is in no file.
                let source = named.get(error.index).map(|(_, source)| source);
                let place = match source {
                    Some(EntrySource::File(place)) => Some(*place),
                    _ => None,
                };
                let (mut in_file, mut place) = (place.and(file).map(str::to_string), place);
                let detail = match &error.kind {
                    UseErrorKind::Resolver(failure) => {
                        if let Some(failed) = &failure.file {
                            in_file = Some(failed.clone());
                            place = failure
                                .line
                                .zip(failure.column)
                                .map(|(line, column)| Place { line, column });
                        }
                        // The resolver's text states the condition itself.
                        failure.to_string()
                    }
                    UseErrorKind::Unknown { known } => {
                        // Met through a package of an extension: at the entry in that
                        // extension's manifest.
                        let user = error.used_by.as_ref().and_then(|user| {
                            loader.blocks().into_iter().find(|block| block.package == *user)
                        });
                        if let Some(user) = user {
                            let identity = &user.extension.identity;
                            let index =
                                user.block.uses.iter().position(|entry| entry.catalog_name(identity) == name);
                            let manifest = finder.manifest(identity);
                            let entry = manifest.as_ref().and_then(|root| root.at(&["flm", "use"]));
                            place = match (entry.map(|node| &node.value), index) {
                                (Some(NodeValue::List(items)), Some(index)) => items.get(index).map(|item| item.place),
                                (Some(NodeValue::Text(_)), Some(0)) => entry.map(|node| node.place),
                                _ => None,
                            };
                            in_file = Some(identity.clone());
                        }
                        let known: Vec<String> = known.iter().map(|name| format!("`{name}`")).collect();
                        let folders: Vec<String> =
                            finder.folders().iter().map(|folder| format!("`{}`", folder.path.display())).collect();
                        let folders = if folders.is_empty() {
                            String::from("a folder of the search path, which is empty")
                        } else {
                            format!("a folder of the search path ({})", folders.join(", "))
                        };
                        format!(
                            "`{name}` must name a package compiled into the program ({}) or an extension found in \
                             {folders}",
                            known.join(", ")
                        )
                    }
                };
                // A problem that stays at an entry no file holds opens with where its list
                // comes from.
                let detail = match (source, &in_file, place) {
                    (Some(EntrySource::Given(source)), None, None) => format!("in {source}, {detail}"),
                    _ => detail,
                };
                PackageProblem { name, file: in_file, place, detail }
            })
            .collect()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::files::{BuildFiles, MemoryFiles};
    use crate::output::{MemorySink, Naming};
    use crate::session::Input;
    use crate::stdpkg::StdParts;

    fn stdmain_factory() -> PackageFactory {
        stdpkg::stdmain_factory(StdParts {
            sink: Some(Arc::new(MemorySink::new())),
            ..StdParts::new(Arc::new(Naming::Folder))
        })
        .unwrap()
    }

    fn parts(files: MemoryFiles) -> (Arc<ExtensionFinder>, Arc<ExtensionLoader>, PackageCatalog) {
        let folder = ExtensionFolder { path: PathBuf::from("defs"), required: false };
        let files: Arc<dyn crate::files::Files> = Arc::new(files);
        let service = crate::files::ServiceFiles::new(Arc::clone(&files));
        let finder =
            Arc::new(ExtensionFinder::new(BuildFiles::new(Arc::clone(&files)), service.clone(), vec![folder]));
        let loader = loader(&finder, &Request::new(Input::Text(String::new())), &service, Path::new(""));
        let catalog = catalog(stdmain_factory(), &[], Arc::clone(&loader)).unwrap();
        (finder, loader, catalog)
    }

    fn names(factories: &[PackageFactory]) -> Vec<String> {
        factories.iter().map(|factory| factory.name().to_string()).collect()
    }

    #[test]
    fn the_catalog_carries_stdcore_stdmain_std_math_passthrough_then_the_host_s_packages() {
        let host = PackageFactory::new("hostpkg", |_| Ok(flm_core::core::registry::PackageEntry::new("hostpkg"))).unwrap();
        assert_eq!(
            names(&carried(stdmain_factory(), &[host]).unwrap()),
            ["stdcore", "stdmain", "std", "math-passthrough", "hostpkg"]
        );
    }

    #[test]
    fn each_package_comes_after_those_it_uses_and_once() {
        let (finder, loader, catalog) = parts(MemoryFiles::new());
        let given = || EntrySource::Given(String::from("the host's list"));
        let found = resolve(&catalog, &loader, &finder, &[(String::from("std"), given())], None).unwrap();
        assert_eq!(names(&found), ["stdcore", "stdmain", "std"]);
        // The initial list `stdcore`, then the document's `std`: `stdcore` once.
        let place = Place { line: 3, column: 9 };
        let named = [(String::from("stdcore"), given()), (String::from("std"), EntrySource::File(place))];
        assert_eq!(names(&resolve(&catalog, &loader, &finder, &named, None).unwrap()), ["stdcore", "stdmain", "std"]);
        assert!(resolve(&catalog, &loader, &finder, &[], None).unwrap().is_empty());
    }

    #[test]
    fn a_name_that_matches_no_package_is_placed() {
        let (finder, loader, catalog) = parts(MemoryFiles::new());
        let place = Place { line: 3, column: 9 };
        let named = [(String::from("nosuch"), EntrySource::File(place))];
        let problems = resolve(&catalog, &loader, &finder, &named, Some("d.flm")).unwrap_err();
        assert_eq!((problems[0].file.as_deref(), problems[0].place), (Some("d.flm"), Some(place)));
        assert_eq!(
            problems[0].detail,
            "`nosuch` must name a package compiled into the program (`math-passthrough`, `std`, `stdcore`, `stdmain`) or an extension \
             found in a folder of the search path (`defs`)"
        );
        // An entry the request gives is in no file.
        let named = [(String::from("nosuch"), EntrySource::Given(String::from("the option `--initial-use`")))];
        let problems = resolve(&catalog, &loader, &finder, &named, Some("d.flm")).unwrap_err();
        assert_eq!((problems[0].file.as_deref(), problems[0].place), (None, None));
        // Its message opens with where its list comes from.
        assert_eq!(
            problems[0].detail,
            "in the option `--initial-use`, `nosuch` must name a package compiled into the program \
             (`math-passthrough`, `std`, `stdcore`, `stdmain`) or an extension found in a folder of the search \
             path (`defs`)"
        );
    }
}
