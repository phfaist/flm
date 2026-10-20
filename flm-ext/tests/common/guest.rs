//! The guest crates of `flm-ext/tests/wasm/` (`wordcount`, case 1 compiled from
//! Rust, and `failing`) and the stub of `flm-ext/tests/native/stub/`, built for the
//! target `wasm32-unknown-unknown` in release by the gated tests that load their
//! modules: `flm-ext/tests/wasm_case1.rs`, `flm-ext/tests/native.rs`,
//! `flm-ext/tests/native_wasm32.rs` (which also builds the program of
//! `flm-ext/tests/native/program/` itself, under the same gate) and
//! `flm-cli/tests/extensions_wasm.rs`, which includes this file by its path.
//! Those tests run only with `FLM_TESTS_WASM_GUEST=1` and the target installed
//! (building a guest compiles `flm-core` for the target in release); otherwise
//! each writes one line beginning `skipped:` and passes.
//!
//! A guest is built with `cargo build --manifest-path flm-ext/tests/wasm/NAME/Cargo.toml
//! --target wasm32-unknown-unknown --release --locked` into the target folder
//! `target/wasm-guest/` of the checkout, once per test process; its module is
//! `target/wasm-guest/wasm32-unknown-unknown/release/NAME.wasm`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

/// The target a guest is compiled to.
pub const TARGET: &str = "wasm32-unknown-unknown";

/// The line a gated test writes when it cannot run here, or `None` when it can:
/// `FLM_TESTS_WASM_GUEST` is not `1`, or the target is not installed.
pub fn skipped() -> Option<String> {
    if std::env::var("FLM_TESTS_WASM_GUEST").ok().as_deref() != Some("1") {
        return Some(
            "skipped: the WebAssembly guest case (set FLM_TESTS_WASM_GUEST=1 to build the guest crates of \
             flm-ext/tests/wasm/)"
                .to_string(),
        );
    }
    let installed = Command::new("rustup").args(["target", "list", "--installed"]).output();
    match installed {
        Ok(output) if String::from_utf8_lossy(&output.stdout).lines().any(|line| line.trim() == TARGET) => None,
        _ => Some(format!("skipped: the WebAssembly guest case (the target `{TARGET}` is not installed)")),
    }
}

/// The folder of the guest crate `name`, under `flm_ext`, the folder of the crate `flm-ext`.
pub fn crate_folder(flm_ext: &Path, name: &str) -> PathBuf {
    flm_ext.join("tests/wasm").join(name)
}

/// The module of the guest crate `name` of `flm-ext/tests/wasm/`, built once per test
/// process, `flm_ext` being the folder of the crate `flm-ext`; or `None`, after writing the
/// line of [`skipped`], when the gated tests cannot run here. A build that fails panics with
/// cargo's output.
pub fn module(flm_ext: &Path, name: &str) -> Option<Vec<u8>> {
    module_in(flm_ext, &crate_folder(flm_ext, name), name)
}

/// The module of the guest crate `name` in the folder `folder` (`flm-ext/tests/native/stub`,
/// say), built as [`module`] builds one.
pub fn module_in(flm_ext: &Path, folder: &Path, name: &str) -> Option<Vec<u8>> {
    if let Some(line) = skipped() {
        eprintln!("{line}");
        return None;
    }
    static BUILT: Mutex<BTreeMap<String, Vec<u8>>> = Mutex::new(BTreeMap::new());
    let mut built = BUILT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if let Some(bytes) = built.get(name) {
        return Some(bytes.clone());
    }
    let target = flm_ext.join("../target/wasm-guest");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .arg("build")
        .arg("--manifest-path")
        .arg(folder.join("Cargo.toml"))
        .args(["--target", TARGET, "--release", "--locked"])
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .expect("cargo runs");
    assert!(
        output.status.success(),
        "the guest crate `{name}` must build for {TARGET}:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = target.join(TARGET).join("release").join(format!("{name}.wasm"));
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    built.insert(name.to_string(), bytes.clone());
    Some(bytes)
}
