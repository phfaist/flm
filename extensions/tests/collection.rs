//! A collection built from the crate: the module that `flm-extensions` compiles for
//! `wasm32-unknown-unknown` with the feature `collection` holds, in its custom section
//! `flmextensions`, the files of each extension whose feature is on, as they are in the
//! crate's folders.
//!
//! Gated on `FLM_TESTS_WASM_GUEST=1` and the target `wasm32-unknown-unknown` being
//! installed: the test builds the module with `cargo rustc -p flm-extensions
//! --no-default-features --features collection,flmdoc,profile-pdf-latex,lecturenotes
//! --target wasm32-unknown-unknown --profile wasm-extension --crate-type cdylib --locked`, in
//! a cargo run of its own, into the target folder `target/wasm-collection/` of the checkout.
//! Otherwise it writes one line beginning `skipped:` and passes.

use std::path::Path;
use std::process::Command;

use flm_ext::collection::{SECTION, read_section};

/// The target a collection is compiled to.
const TARGET: &str = "wasm32-unknown-unknown";

/// The extensions of the collection: two written in Rust and one written as data.
const STEMS: [&str; 3] = ["flmdoc", "profile-pdf-latex", "lecturenotes"];

/// The line the test writes when it cannot run here, or `None` when it can.
fn skipped() -> Option<String> {
    if std::env::var("FLM_TESTS_WASM_GUEST").ok().as_deref() != Some("1") {
        return Some("skipped: the collection built for WebAssembly (set FLM_TESTS_WASM_GUEST=1 to build it)".into());
    }
    let installed = Command::new("rustup").args(["target", "list", "--installed"]).output();
    match installed {
        Ok(output) if String::from_utf8_lossy(&output.stdout).lines().any(|line| line.trim() == TARGET) => None,
        _ => Some(format!("skipped: the collection built for WebAssembly (the target `{TARGET}` is not installed)")),
    }
}

/// The module of the crate built as a collection of [`STEMS`].
fn build() -> Vec<u8> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let target = root.join("target/wasm-collection");
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let output = Command::new(cargo)
        .current_dir(&root)
        .args(["rustc", "-p", "flm-extensions", "--no-default-features", "--features"])
        .arg(format!("collection,{}", STEMS.join(",")))
        .args(["--target", TARGET, "--profile", "wasm-extension", "--crate-type", "cdylib", "--locked"])
        .env("CARGO_TARGET_DIR", &target)
        .output()
        .expect("cargo runs");
    assert!(
        output.status.success(),
        "the collection must build for {TARGET}:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let path = target.join(TARGET).join("wasm-extension/flm_extensions.wasm");
    std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A number of the module's binary form (unsigned LEB128) at `at`, and the place after it.
fn leb128(bytes: &[u8], mut at: usize) -> (usize, usize) {
    let (mut value, mut shift) = (0, 0);
    loop {
        let byte = bytes[at];
        at += 1;
        value |= usize::from(byte & 0x7f) << shift;
        shift += 7;
        if byte < 0x80 {
            return (value, at);
        }
    }
}

/// The contents of the custom section `name` of `module`, which holds one.
fn custom_section<'a>(module: &'a [u8], name: &str) -> &'a [u8] {
    assert_eq!(&module[..4], b"\0asm", "a WebAssembly module");
    let mut at = 8;
    while at < module.len() {
        let id = module[at];
        let (size, start) = leb128(module, at + 1);
        let end = start + size;
        if id == 0 {
            let (length, name_start) = leb128(module, start);
            if &module[name_start..name_start + length] == name.as_bytes() {
                return &module[name_start + length..end];
            }
        }
        at = end;
    }
    panic!("the module has no custom section ‘{name}’");
}

#[test]
fn the_reader_gives_the_files_of_each_extension_of_a_collection() {
    if let Some(line) = skipped() {
        eprintln!("{line}");
        return;
    }
    let module = build();
    let read = read_section(custom_section(&module, SECTION)).expect("the section reads");
    let mut expected: Vec<_> = flm_extensions::EXTENSIONS.iter().filter(|row| STEMS.contains(&row.stem)).collect();
    expected.sort_by_key(|row| (row.folder, row.stem));
    // The reader gives the files in the order of their paths as texts.
    let files = |row: &flm_ext::source::BundledExtension| {
        let mut files: Vec<_> = row.files.iter().collect();
        files.sort_by_key(|file| file.path);
        files
    };
    let shape = |folder: &str, stem: &str, paths: Vec<&str>| (folder.to_string(), stem.to_string(), paths.join(" "));
    assert_eq!(
        read.iter()
            .map(|extension| shape(extension.folder, extension.stem, extension.files.iter().map(|file| file.path).collect()))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|row| shape(row.folder, row.stem, files(row).iter().map(|file| file.path).collect()))
            .collect::<Vec<_>>(),
    );
    for (extension, row) in read.iter().zip(&expected) {
        for (file, bundled) in extension.files.iter().zip(files(row)) {
            assert!(file.bytes == bundled.bytes, "the bytes of ‘{}’ of ‘{}’", file.path, extension.stem);
        }
    }
}
