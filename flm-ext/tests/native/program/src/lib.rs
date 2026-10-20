//! A program that carries the extensions `one` and `two` compiled in, through the engine
//! `native`, and that is built for `wasm32-unknown-unknown` by the gated test
//! `flm-ext/tests/native_wasm32.rs`. Its two exports, which the test calls:
//!
//! - `load_two_extensions` loads both extensions: each one's `define` and `packages` run
//!   through the guest client;
//! - `build_registry` builds a registry of the package `stdcore` and both extensions'
//!   packages: each feature's `register` runs, and its request (the declaration of a table)
//!   is answered in the same process.
//!
//! The test also checks that the module links (the extension crates write no export when
//! `flm-ext`'s feature `native` is on) and that it imports neither `flm.call` nor `flm.take`
//! (the requests are answered in the same process).

use std::sync::Arc;

use flm_core::core::registry::{PackageResolver, RegistryBuilder, RegistryInputs};
use flm_core::core::settings::PackageSettings;
use flm_core::stdcore::{self, StdcoreConfig};
use flm_ext::native::{NativeEngine, NativeExtension};
use flm_ext::{ExtensionLoader, MemoryFiles, MemorySource};
use serde_json::json;

/// The manifest of an extension compiled into the program.
const MANIFEST: &str = "extension:\n  engine: native\n";

/// A loader that finds the extensions `one` and `two` and runs them through the engine
/// `native`.
fn loader() -> ExtensionLoader {
    let engine = NativeEngine::new([
        NativeExtension { stem: "one", manifest: MANIFEST, compiled: &one::FLM_EXTENSION },
        NativeExtension { stem: "two", manifest: MANIFEST, compiled: &two::FLM_EXTENSION },
    ]);
    let manifest = json!({ "extension": { "engine": "native" } });
    let source = MemorySource::new()
        .with("one", "compiled-in:one", manifest.clone(), MemoryFiles::new())
        .with("two", "compiled-in:two", manifest, MemoryFiles::new());
    ExtensionLoader::new(vec![Arc::new(source)]).with_engine(Arc::new(engine))
}

/// Loads the extensions `one` and `two` through the engine `native`; answers how many
/// loaded (2).
#[unsafe(no_mangle)]
pub extern "C" fn load_two_extensions() -> u32 {
    let loader = loader();
    ["one", "two"].into_iter().filter(|stem| matches!(loader.resolve(stem), Ok(Some(_)))).count() as u32
}

/// Builds a registry of the package `stdcore` and the packages of the extensions `one` and
/// `two`; answers how many of their tables `one.notes` and `two.notes` the registry holds
/// (2), or 0 when an extension does not load or the registry does not build.
///
/// The thread is marked as a worker thread: the module is linked with a stack of flm-core's
/// `WORKER_STACK_SIZE` (`build.rs`), and this target cannot start a thread.
#[unsafe(no_mangle)]
pub extern "C" fn build_registry() -> u32 {
    flm_core::lang::guard::mark_worker_thread();
    let loader = loader();
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(StdcoreConfig::new()));
    for stem in ["one", "two"] {
        let Ok(Some(factory)) = loader.resolve(stem) else { return 0 };
        let Ok(package) = factory.build(&PackageSettings::new()) else { return 0 };
        builder.use_package(package);
    }
    let Ok(registry) = builder.build(&RegistryInputs::new()) else { return 0 };
    ["one.notes", "two.notes"].into_iter().filter(|table| registry.table_named(table).is_some()).count() as u32
}
