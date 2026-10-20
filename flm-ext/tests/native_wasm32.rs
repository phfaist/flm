//! The engine `native` in a program built for `wasm32-unknown-unknown`: `flm-ext`'s cargo
//! feature `native`, not the target, makes the guest client answer an extension's requests in
//! the same process and the macro `extension!` write no export. Gated on
//! `FLM_TESTS_WASM_GUEST=1` and the target installed (`common::guest`), since each test builds
//! crates for the target in release, into `target/wasm-guest/`:
//!
//! - the program of `tests/native/program/`, which carries the two extension crates `one` and
//!   `two` through the engine `native`, built with them in one cargo run: it links, and its
//!   module imports neither `flm.call` nor `flm.take` and exports none of the guest's four
//!   functions; run by `wasmi`, it loads both extensions and builds a registry of their
//!   packages, each feature's `register` sending its request, answered in the same process;
//! - the module of `one`, written by that same run, where the program turns on `native`: it
//!   lacks the guest's exports, and the engine `wasm` refuses it when it loads, saying which
//!   export it lacks;
//! - the stub of `tests/native/stub/`, built alone (no crate of the run turns on `native`): a
//!   WebAssembly module of its own, with the four exports and the two imports.

#![cfg(feature = "wasm")]

mod common;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use flm_ext::wasm::{WasmHost, WasmSettings};
use flm_ext::{MemoryFiles, MemorySource};

use common::guest::TARGET;

/// The four functions a WebAssembly module of the guest client exports.
const GUEST_EXPORTS: [&str; 4] = ["flm_alloc", "flm_free", "flm_entry", "flm_last_panic"];

/// The folder of the crate `flm-ext`.
fn flm_ext() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// The modules of the program of `tests/native/program/` and of its extension `one`, built for
/// the target in release in one cargo run (`--workspace`), once per test process; or `None`,
/// after writing the line of `common::guest::skipped`, when the gated tests cannot run here.
/// A build that fails panics with cargo's output.
fn program_and_one() -> Option<&'static (Vec<u8>, Vec<u8>)> {
    if let Some(line) = common::guest::skipped() {
        eprintln!("{line}");
        return None;
    }
    static BUILT: OnceLock<(Vec<u8>, Vec<u8>)> = OnceLock::new();
    Some(BUILT.get_or_init(|| {
        let target = flm_ext().join("../target/wasm-guest");
        let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
        let output = Command::new(cargo)
            .arg("build")
            .arg("--manifest-path")
            .arg(flm_ext().join("tests/native/program/Cargo.toml"))
            .args(["--target", TARGET, "--release", "--locked", "--workspace"])
            .env("CARGO_TARGET_DIR", &target)
            .output()
            .expect("cargo runs");
        assert!(
            output.status.success(),
            "the program of tests/native/program/ must build for {TARGET}:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let read = |name: &str| {
            let path: PathBuf = target.join(TARGET).join("release").join(format!("{name}.wasm"));
            std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        };
        (read("program"), read("one"))
    }))
}

/// The module `bytes`, compiled by `wasmi` for inspection.
fn compiled(bytes: &[u8]) -> wasmi::Module {
    wasmi::Module::new(&wasmi::Engine::default(), bytes).expect("a valid WebAssembly module")
}

/// The imports of the module `bytes`, each as `MODULE.NAME`.
fn imports(bytes: &[u8]) -> Vec<String> {
    compiled(bytes).imports().map(|import| format!("{}.{}", import.module(), import.name())).collect()
}

/// Which of the guest's four functions the module `bytes` exports.
fn guest_exports(bytes: &[u8]) -> Vec<&'static str> {
    let module = compiled(bytes);
    GUEST_EXPORTS.into_iter().filter(|name| module.get_export(name).is_some()).collect()
}

/// Calls the function `name` (no argument, a 32-bit answer) of the module `bytes`,
/// instantiated by `wasmi` with no import.
fn call(bytes: &[u8], name: &str) -> u32 {
    let module = compiled(bytes);
    let mut store = wasmi::Store::new(module.engine(), ());
    let instance = wasmi::Linker::<()>::new(module.engine())
        .instantiate_and_start(&mut store, &module)
        .expect("the program instantiates with no import");
    let function = instance
        .get_typed_func::<(), u32>(&store, name)
        .unwrap_or_else(|error| panic!("the program exports `{name}`: {error}"));
    function.call(&mut store, ()).unwrap_or_else(|error| panic!("`{name}` runs: {error}"))
}

#[test]
fn a_program_built_for_wasm32_carries_two_extensions_and_imports_nothing_of_the_guest_s_interface() {
    let Some((program, _)) = program_and_one() else { return };
    let imports = imports(program);
    assert!(!imports.iter().any(|import| import.starts_with("flm.")), "the program imports {imports:?}");
    assert_eq!(guest_exports(program), Vec::<&str>::new());
}

#[test]
fn a_program_built_for_wasm32_runs_its_two_extensions_and_answers_their_requests() {
    let Some((program, _)) = program_and_one() else { return };
    assert_eq!(call(program, "load_two_extensions"), 2, "both extensions load");
    assert_eq!(call(program, "build_registry"), 2, "the registry holds the table of each extension");
}

#[test]
fn a_module_built_where_the_feature_native_is_on_lacks_its_exports_and_is_refused_at_load() {
    let Some((_, one)) = program_and_one() else { return };
    assert_eq!(guest_exports(one), Vec::<&str>::new());
    let files = MemoryFiles::new().with_bytes("one.wasm", one);
    let source = MemorySource::new().with("one", "one", common::wasm_manifest("one.wasm"), files);
    let loader = common::wasm_loader(&WasmHost::new(WasmSettings::default()), source);
    let lacks = |name: &str| {
        format!(
            "the module `one.wasm` of the extension `one` must export the functions `flm_entry`, `flm_alloc` and \
             `flm_free` and the memory `memory`; it lacks `{name}`"
        )
    };
    assert_eq!(common::load_problems(&loader, "one"), [lacks("flm_entry"), lacks("flm_alloc"), lacks("flm_free")]);
}

#[test]
fn the_stub_built_alone_is_a_module_of_its_own_with_the_guest_s_exports_and_imports() {
    let Some(stub) = common::guest::module_in(flm_ext(), &flm_ext().join("tests/native/stub"), "stub") else { return };
    assert_eq!(guest_exports(&stub), GUEST_EXPORTS);
    let imports = imports(&stub);
    assert!(imports.contains(&"flm.call".to_string()) && imports.contains(&"flm.take".to_string()), "{imports:?}");
}
