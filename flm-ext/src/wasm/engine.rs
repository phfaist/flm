//! The engine `wasm` ([`WasmEngine`]) and [`ExtensionLoader::with_wasm`]:
//! loading an extension compiled to one WebAssembly module, which declares its
//! packages from its entry `define` and answers them from its entry
//! `packages`, read into [`PackageReport`](crate::engine::PackageReport)s by
//! the shared `binding::packages`.

use std::sync::Arc;

use wasmi::{ExternType, Module, ValType};

use super::host::WasmHost;
use super::instance::{self, WasmExtension, WasmInstance};
use super::ENGINE_VERSION;
use crate::binding::packages::read_packages;
use crate::binding::{self, BindingRuntime};
use crate::engine::{Engine, ExtensionIdentity, LoadError, Loaded};
use crate::loader::ExtensionLoader;
use crate::manifest::Manifest;
use crate::paths::{self, PathErrorKind};
use crate::source::ExtensionFiles;

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
/// compiled module. The module page ([`crate::wasm`]) gives the ABI and the
/// rules.
#[derive(Debug, Clone)]
pub struct WasmEngine {
    /// The host.
    host: WasmHost,
    /// The wasmi engine every module of this engine is compiled for, with
    /// fuel metering on.
    engine: wasmi::Engine,
}

impl WasmEngine {
    /// The engine `wasm` over `host`.
    pub fn new(host: WasmHost) -> WasmEngine {
        let mut config = wasmi::Config::default();
        config.consume_fuel(true);
        WasmEngine { host, engine: wasmi::Engine::new(&config) }
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

/// Every problem of the module `module` at `path` of the extension `stem`
/// against the ABI: each import other than `flm.call` and `flm.take` or not of
/// its type; each required export missing or not of its type, and
/// `flm_last_panic` when given and not of its type.
fn check_abi(stem: &str, path: &str, module: &Module) -> Vec<LoadError> {
    use ValType::{I32, I64};
    let mut problems = Vec::new();
    for import in module.imports() {
        let (expected, params, results): (&str, &[ValType], &[ValType]) = match (import.module(), import.name()) {
            ("flm", "call") => (CALL_TYPE, &[I32, I32], &[I32]),
            ("flm", "take") => (TAKE_TYPE, &[I32, I32], &[]),
            (from, name) => {
                problems.push(format!(
                    "the module `{path}` of the extension `{stem}` imports `{from}.{name}`, which this program does not \
                     offer; the imports offered are `flm.call` and `flm.take`"
                ));
                continue;
            }
        };
        if !is_func(import.ty(), params, results) {
            problems.push(format!(
                "the import `flm.{}` of the module `{path}` of the extension `{stem}` must have the type `{expected}`",
                import.name()
            ));
        }
    }
    let functions: [(&str, &str, &[ValType], &[ValType]); 3] = [
        ("flm_entry", ENTRY_TYPE, &[I32, I32], &[I64]),
        ("flm_alloc", ALLOC_TYPE, &[I32], &[I32]),
        ("flm_free", FREE_TYPE, &[I32, I32], &[]),
    ];
    for (name, expected, params, results) in functions {
        match module.get_export(name) {
            None => problems.push(lacks(stem, path, name)),
            Some(ty) if !is_func(&ty, params, results) => problems.push(of_type(stem, path, name, expected)),
            Some(_) => {}
        }
    }
    match module.get_export("memory") {
        None => problems.push(lacks(stem, path, "memory")),
        Some(ExternType::Memory(_)) => {}
        Some(_) => problems.push(of_type(stem, path, "memory", "(memory)")),
    }
    match module.get_export("flm_last_panic") {
        Some(ty) if !is_func(&ty, &[], &[I64]) => problems.push(of_type(stem, path, "flm_last_panic", LAST_PANIC_TYPE)),
        _ => {}
    }
    problems.into_iter().map(|detail| LoadError::Load { detail }).collect()
}

/// The text of a required export the module lacks.
fn lacks(stem: &str, path: &str, name: &str) -> String {
    format!(
        "the module `{path}` of the extension `{stem}` must export the functions `flm_entry`, `flm_alloc` and \
         `flm_free` and the memory `memory`; it lacks `{name}`"
    )
}

/// The text of an export not of its type.
fn of_type(stem: &str, path: &str, name: &str, expected: &str) -> String {
    format!("the export `{name}` of the module `{path}` of the extension `{stem}` must have the type `{expected}`")
}

/// `text` on one line: each line break, with the indentation after it, becomes
/// one space (`wasmi` writes the byte arrays of a bad header one element per
/// line).
fn one_line(text: &str) -> String {
    let mut lines = text.lines();
    let mut joined = lines.next().unwrap_or_default().to_string();
    for line in lines {
        joined.push(' ');
        joined.push_str(line.trim_start());
    }
    joined
}

/// The engine's part of the digest: the bytes `flm-ext wasm `, the engine's
/// version, a NUL byte, the module's bytes, a NUL byte. The module's path does
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

    /// Loads the extension: the module's path normalized, its bytes read and
    /// compiled, its imports and exports checked, an instance created (its
    /// entry `define` called), the packages read from its entry `packages`,
    /// the templates read; the instance is then kept on this thread for the
    /// registry build.
    fn load(
        &self,
        extension: &ExtensionIdentity,
        manifest: &Manifest,
        files: Box<dyn ExtensionFiles>,
    ) -> Result<Loaded, Vec<LoadError>> {
        let stem = &extension.stem;
        let at_module = |detail: String| vec![LoadError::Manifest { path: "extension.wasm.module".to_string(), detail }];
        let written = manifest.wasm.as_ref().map(|wasm| wasm.module.clone()).unwrap_or_default();
        let path = paths::normalize(&written).map_err(|error| {
            at_module(match error.kind {
                PathErrorKind::LeavesRoot => format!(
                    "`extension.wasm.module` of the extension `{stem}` is `{written}`, which leaves the extension's folder"
                ),
                PathErrorKind::Absolute => format!(
                    "`extension.wasm.module` of the extension `{stem}` is `{written}`, which is an absolute path; it \
                     must be a path inside the extension's folder"
                ),
                PathErrorKind::Backslash => format!(
                    "`extension.wasm.module` of the extension `{stem}` is `{written}`, which holds `\\`; a path inside \
                     the extension uses `/`"
                ),
            })
        })?;
        let bytes = match files.read_bytes(&path) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => {
                return Err(at_module(format!(
                    "the extension `{stem}` names its module `{path}` (`extension.wasm.module`), and the extension holds \
                     no file `{path}`"
                )));
            }
            Err(error) => {
                return Err(vec![LoadError::Load {
                    detail: format!("the module `{path}` of the extension `{stem}` cannot be read: {error}"),
                }]);
            }
        };
        let module = Module::new(&self.engine, &bytes).map_err(|error| {
            vec![LoadError::Load {
                detail: format!(
                    "the module `{path}` of the extension `{stem}` must be a valid WebAssembly module: {}",
                    one_line(&error.to_string())
                ),
            }]
        })?;
        let problems = check_abi(stem, &path, &module);
        if !problems.is_empty() {
            return Err(problems);
        }
        let initial_memory = match module.get_export("memory") {
            Some(ExternType::Memory(memory)) => memory.minimum().saturating_mul(1 << 16),
            _ => 0,
        };
        let loading = WasmExtension {
            id: binding::next_extension_id(),
            host: self.host.clone(),
            settings: match &manifest.wasm {
                Some(wasm) => self.host.settings().with_limits(&wasm.limits),
                None => self.host.settings().clone(),
            },
            extension: extension.clone(),
            path,
            engine: self.engine.clone(),
            module,
            initial_memory,
        };
        let created = WasmInstance::create(&loading).map_err(|error| vec![LoadError::Load { detail: error.to_string() }])?;
        let defined =
            created.call_load_entry(&loading, "packages", &[]).map_err(|error| vec![LoadError::Load { detail: error.to_string() }])?;
        let packages = read_packages(extension, &defined.to_string(), &*files)?;
        drop(files);
        // Shared before it is kept: the thread's map holds it weakly.
        let loaded = Arc::new(loading);
        instance::keep(&loaded, created);
        Ok(Loaded { packages, runtime: Arc::new(BindingRuntime { binding: loaded, digest: digest(&bytes) }) })
    }
}
