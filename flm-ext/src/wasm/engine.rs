//! The engine `wasm` ([`WasmEngine`]) and [`ExtensionLoader::with_wasm`]:
//! loading an extension compiled to one WebAssembly module, which declares its
//! packages from its entry `define` and answers them from its entry
//! `packages`, read into [`PackageReport`](crate::engine::PackageReport)s by
//! the shared `binding::packages`.

use std::sync::Arc;

use wasmi::{ExternType, Module, ValType};

use super::collection::{Collection, CollectionExtension};
use super::host::WasmHost;
use super::instance::{self, ModuleOrigin, WasmExtension, WasmModule};
use super::ENGINE_VERSION;
use crate::binding::packages::{read_packages, unreadable};
use crate::binding::{self, BindingRuntime};
use crate::collection::SECTION;
use crate::engine::{Engine, ExtensionIdentity, LoadError, Loaded};
use crate::loader::ExtensionLoader;
use crate::manifest::Manifest;
use crate::source::{ExtensionFiles, FilesError, FilesErrorKind};

/// The name of the engine.
const WASM: &str = "wasm";

/// The engine `wasm`: an extension compiled to one WebAssembly module, run by
/// the interpreter wasmi. Created with the host every instance of the
/// extensions it loads uses.
///
/// [`load`](Engine::load) reads the module (`extension.wasm.module`) through
/// the extension's files, on the calling thread, once; compiles it; checks
/// its imports and exports; creates an instance and calls its entries
/// `define` and `packages`; and reports the packages they declare. Every
/// other instance of the extension, on any thread, is created from the
/// compiled module. An extension of a collection the engine was given
/// ([`WasmEngine::with_collection`]) runs on the collection's module instead,
/// which the engine neither reads nor compiles. The module page
/// ([`crate::wasm`]) gives the ABI and the rules.
#[derive(Debug, Clone)]
pub struct WasmEngine {
    /// The host.
    host: WasmHost,
    /// The wasmi engine every module of this engine is compiled for, with
    /// fuel metering on.
    engine: wasmi::Engine,
    /// The collections the program offers, in the order given.
    collections: Vec<Offered>,
}

/// A collection given to the engine, with the memory limit of its instances.
#[derive(Debug, Clone)]
struct Offered {
    /// The collection.
    collection: Collection,
    /// The memory limit of its instances: the sum of the limits of its
    /// extensions that have code; `None` is unlimited.
    memory_limit: Option<usize>,
}

impl WasmEngine {
    /// The engine `wasm` over `host`.
    pub fn new(host: WasmHost) -> WasmEngine {
        let mut config = wasmi::Config::default();
        config.consume_fuel(true);
        WasmEngine { host, engine: wasmi::Engine::new(&config), collections: Vec::new() }
    }

    /// The engine with `collection` among the collections whose extensions
    /// it runs: an extension whose identity is that of a row of the
    /// collection (`LABEL:FOLDER/STEM`, [`Collection`]) runs on the
    /// collection's module, and its manifest's `extension.wasm.module` is not
    /// read.
    ///
    /// `manifest` answers the manifest of each row, called once for each, in
    /// the collection's order, or `None` when it cannot be read. It must be
    /// the manifest that the program's source gives for that row. The memory
    /// limit of the collection's instances is the sum of the memory limits of
    /// the rows that have code (the engine `wasm`), each its manifest's or
    /// else the host's ([`WasmSettings::with_limits`](super::WasmSettings::with_limits)),
    /// unlimited when one of them is. A row whose manifest cannot be read
    /// counts for nothing; it fails its own load when a document uses it.
    pub fn with_collection(
        mut self,
        collection: Collection,
        manifest: impl Fn(&CollectionExtension) -> Option<Manifest>,
    ) -> WasmEngine {
        let memory_limit = collection
            .extensions()
            .iter()
            .filter_map(&manifest)
            .filter(|manifest| manifest.engine == WASM)
            .try_fold(0usize, |sum, manifest| {
                let limit = match &manifest.wasm {
                    Some(wasm) => self.host.settings().with_limits(&wasm.limits).memory_limit,
                    None => self.host.settings().memory_limit,
                };
                limit.map(|limit| sum.saturating_add(limit))
            });
        self.collections.push(Offered { collection, memory_limit });
        self
    }
}

impl ExtensionLoader {
    /// The loader with the engine `wasm` over `host`:
    /// `self.with_engine(Arc::new(WasmEngine::new(host)))`.
    pub fn with_wasm(self, host: WasmHost) -> ExtensionLoader {
        self.with_engine(Arc::new(WasmEngine::new(host)))
    }
}

/// The function types of the ABI, as the messages write them.
const CALL_TYPE: &str = "(func (param i32 i32) (result i32))";
const TAKE_TYPE: &str = "(func (param i32 i32))";
const ENTRY_TYPE: &str = "(func (param i32 i32) (result i64))";
const ALLOC_TYPE: &str = "(func (param i32) (result i32))";
const FREE_TYPE: &str = "(func (param i32 i32))";
const LAST_PANIC_TYPE: &str = "(func (result i64))";

/// Whether `ty` is a function of the parameters `params` and results
/// `results`.
fn is_func(ty: &ExternType, params: &[ValType], results: &[ValType]) -> bool {
    matches!(ty, ExternType::Func(func) if func.params() == params && func.results() == results)
}

/// A way in which a module does not follow the guest's ABI: what
/// [`Collection::read`] reports in
/// [`CollectionErrorKind::Abi`](super::CollectionErrorKind::Abi).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AbiProblem {
    /// An import other than `flm.call` and `flm.take`.
    Import {
        /// The module the import names (`env` of `env.f`).
        module: String,
        /// The import's name (`f` of `env.f`).
        name: String,
    },
    /// The import `flm.call` or `flm.take` is not of its type.
    ImportType {
        /// The import's name: `call` or `take`.
        name: String,
        /// The type it must have, in the text format.
        expected: &'static str,
    },
    /// A required export is missing: `flm_entry`, `flm_alloc`, `flm_free` or
    /// `memory`.
    MissingExport(&'static str),
    /// An export is not of its type.
    ExportType {
        /// The export's name.
        name: &'static str,
        /// The type it must have, in the text format.
        expected: &'static str,
    },
}

/// Every problem of the module `module` against the ABI: each import other
/// than `flm.call` and `flm.take` or not of its type; each required export
/// missing or not of its type, and `flm_last_panic` when given and not of its
/// type.
pub(super) fn check_abi(module: &Module) -> Vec<AbiProblem> {
    use ValType::{I32, I64};
    let mut problems = Vec::new();
    for import in module.imports() {
        let (expected, params, results): (&str, &[ValType], &[ValType]) = match (import.module(), import.name()) {
            ("flm", "call") => (CALL_TYPE, &[I32, I32], &[I32]),
            ("flm", "take") => (TAKE_TYPE, &[I32, I32], &[]),
            (from, name) => {
                problems.push(AbiProblem::Import { module: from.to_string(), name: name.to_string() });
                continue;
            }
        };
        if !is_func(import.ty(), params, results) {
            problems.push(AbiProblem::ImportType { name: import.name().to_string(), expected });
        }
    }
    let functions: [(&str, &str, &[ValType], &[ValType]); 3] = [
        ("flm_entry", ENTRY_TYPE, &[I32, I32], &[I64]),
        ("flm_alloc", ALLOC_TYPE, &[I32], &[I32]),
        ("flm_free", FREE_TYPE, &[I32, I32], &[]),
    ];
    for (name, expected, params, results) in functions {
        match module.get_export(name) {
            None => problems.push(AbiProblem::MissingExport(name)),
            Some(ty) if !is_func(&ty, params, results) => problems.push(AbiProblem::ExportType { name, expected }),
            Some(_) => {}
        }
    }
    match module.get_export("memory") {
        None => problems.push(AbiProblem::MissingExport("memory")),
        Some(ExternType::Memory(_)) => {}
        Some(_) => problems.push(AbiProblem::ExportType { name: "memory", expected: "(memory)" }),
    }
    match module.get_export("flm_last_panic") {
        Some(ty) if !is_func(&ty, &[], &[I64]) => {
            problems.push(AbiProblem::ExportType { name: "flm_last_panic", expected: LAST_PANIC_TYPE })
        }
        _ => {}
    }
    problems
}

/// The text of `problem` of the module at `path` of the extension `stem`.
fn extension_problem(stem: &str, path: &str, problem: &AbiProblem) -> String {
    let what = format!("the module `{path}` of the extension `{stem}`");
    match problem {
        AbiProblem::Import { module, name } => format!(
            "{what} imports `{module}.{name}`, which this program does not offer; the imports offered are `flm.call` \
             and `flm.take`"
        ),
        AbiProblem::ImportType { name, expected } => {
            format!("the import `flm.{name}` of {what} must have the type `{expected}`")
        }
        AbiProblem::MissingExport(name) => format!(
            "{what} must export the functions `flm_entry`, `flm_alloc` and `flm_free` and the memory `memory`; it lacks \
             `{name}`"
        ),
        AbiProblem::ExportType { name, expected } => {
            format!("the export `{name}` of {what} must have the type `{expected}`")
        }
    }
}

/// `text` on one line: each line break, with the indentation after it, becomes
/// one space (`wasmi` writes the byte arrays of a bad header one element per
/// line).
pub(super) fn one_line(text: &str) -> String {
    let mut lines = text.lines();
    let mut joined = lines.next().unwrap_or_default().to_string();
    for line in lines {
        joined.push(' ');
        joined.push_str(line.trim_start());
    }
    joined
}

/// The engine's part of the digest: the bytes `flm-ext wasm `, the engine's
/// version, a NUL byte, the module's bytes (the bytes of the collection's
/// file, for an extension of a collection), a NUL byte. The module's path does
/// not enter (the manifest names it), nor do the host's settings.
fn digest(bytes: &[u8]) -> Vec<u8> {
    let mut digest = b"flm-ext wasm ".to_vec();
    digest.extend_from_slice(ENGINE_VERSION.as_bytes());
    digest.push(0);
    digest.extend_from_slice(bytes);
    digest.push(0);
    digest
}

impl Engine for WasmEngine {
    /// `wasm`.
    fn name(&self) -> &str {
        WASM
    }

    /// Loads the extension: the module's bytes read, by the path the manifest
    /// writes (relative to the extension's folder), and compiled, its imports
    /// and exports checked, or, for an extension of a collection the engine
    /// was given, the collection's module taken; then the extension defined in
    /// this thread's instance of the module (created first if needed), the
    /// packages read from its entry `packages`, the templates read. The
    /// instance stays on this thread for the registry build.
    fn load(
        &self,
        extension: &ExtensionIdentity,
        manifest: &Manifest,
        files: Box<dyn ExtensionFiles>,
    ) -> Result<Loaded, Vec<LoadError>> {
        let mut settings = match &manifest.wasm {
            Some(wasm) => self.host.settings().with_limits(&wasm.limits),
            None => self.host.settings().clone(),
        };
        let offered = self.collections.iter().find(|offered| offered.collection.holds(&extension.identity));
        let (origin, module, digest) = match offered {
            Some(offered) => {
                settings.memory_limit = offered.memory_limit;
                let origin = ModuleOrigin::Collection(offered.collection.label().to_string());
                (origin, Arc::clone(offered.collection.module()), digest(offered.collection.bytes()))
            }
            None => {
                let path = manifest.wasm.as_ref().map(|wasm| wasm.module.clone()).unwrap_or_default();
                let bytes = self.read_module(extension, &path, &*files)?;
                let module = Arc::new(self.compile(extension, &path, &bytes)?);
                (ModuleOrigin::Folder(path), module, digest(&bytes))
            }
        };
        let loaded = Arc::new(WasmExtension {
            id: binding::next_extension_id(),
            host: self.host.clone(),
            settings,
            extension: extension.clone(),
            origin,
            module,
        });
        let defined =
            instance::call_load_entry(&loaded, "packages").map_err(|error| vec![LoadError::Load { detail: error.to_string() }])?;
        let items: Vec<serde_json::Value> = serde_json::from_value(defined).map_err(|error| unreadable(extension, error))?;
        let packages = read_packages(extension, &items, &*files)?;
        drop(files);
        Ok(Loaded { packages, runtime: Arc::new(BindingRuntime { binding: loaded, digest }) })
    }
}

impl WasmEngine {
    /// The bytes of the module at `path` (as `extension.wasm.module` writes
    /// it) among the files of `extension`.
    ///
    /// # Errors
    ///
    /// The extension holds no such file, the path leaves its folder, is
    /// absolute or names no file on this system, a symbolic link leads
    /// outside the folder, or the file cannot be read.
    fn read_module(&self, extension: &ExtensionIdentity, path: &str, files: &dyn ExtensionFiles) -> Result<Vec<u8>, Vec<LoadError>> {
        let stem = &extension.stem;
        let at_module = |detail: String| vec![LoadError::Manifest { path: "extension.wasm.module".to_string(), detail }];
        match files.read_bytes("", path) {
            Ok(Some(bytes)) => Ok(bytes),
            Ok(None) => Err(at_module(format!(
                "the extension `{stem}` names its module `{path}` (`extension.wasm.module`), and the extension holds no \
                 file `{path}`"
            ))),
            Err(FilesError { kind: FilesErrorKind::Above, .. }) => Err(at_module(format!(
                "`extension.wasm.module` of the extension `{stem}` is `{path}`, which leaves the extension's folder"
            ))),
            Err(FilesError { kind: FilesErrorKind::Absolute, .. }) => Err(at_module(format!(
                "`extension.wasm.module` of the extension `{stem}` is `{path}`, which is an absolute path; it must be a \
                 path inside the extension's folder"
            ))),
            Err(FilesError { kind: FilesErrorKind::NoSystemForm(reason), .. }) => Err(at_module(format!(
                "the module ‘{path}’ of the extension ‘{stem}’ names no file on this system: {reason}"
            ))),
            Err(FilesError { kind: FilesErrorKind::Link, .. }) => Err(vec![LoadError::Load {
                detail: format!(
                    "the module ‘{path}’ of the extension ‘{stem}’ leads outside the extension's folder through a symbolic \
                     link"
                ),
            }]),
            Err(FilesError { kind: FilesErrorKind::Unreadable(detail), .. }) => Err(vec![LoadError::Load {
                detail: format!("the module `{path}` of the extension `{stem}` cannot be read: {detail}"),
            }]),
        }
    }

    /// The module of `extension` at `path`, whose bytes are `bytes`, compiled
    /// for this engine, checked against the ABI, and checked to hold no
    /// section `flmextensions`: a collection is loaded by the program
    /// ([`Collection::read`]), never named by a manifest.
    ///
    /// # Errors
    ///
    /// The bytes are not a valid module, the module holds the section
    /// `flmextensions`, or each problem against the ABI.
    fn compile(&self, extension: &ExtensionIdentity, path: &str, bytes: &[u8]) -> Result<WasmModule, Vec<LoadError>> {
        let stem = &extension.stem;
        let module = Module::new(&self.engine, bytes).map_err(|error| {
            vec![LoadError::Load {
                detail: format!(
                    "the module `{path}` of the extension `{stem}` must be a valid WebAssembly module: {}",
                    one_line(&error.to_string())
                ),
            }]
        })?;
        if module.custom_sections().any(|custom| custom.name() == SECTION) {
            return Err(vec![LoadError::Load {
                detail: format!(
                    "unexpected section ‘{SECTION}’ in the module ‘{path}’ of the extension ‘{stem}’: the module that \
                     ‘extension.wasm.module’ names may not be a collection"
                ),
            }]);
        }
        let problems = check_abi(&module);
        if !problems.is_empty() {
            return Err(problems
                .iter()
                .map(|problem| LoadError::Load { detail: extension_problem(stem, path, problem) })
                .collect());
        }
        Ok(WasmModule::new(self.engine.clone(), module))
    }
}
