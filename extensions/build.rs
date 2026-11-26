//! Writes three files for the extensions whose cargo features are on (cargo sets
//! `CARGO_FEATURE_NAME` for each, the name in upper case with each `-` replaced by `_`):
//! the two tables, and the records of a collection.
//!
//! - `$OUT_DIR/files.rs`, the table of files `EXTENSIONS`: for each folder (`data`, `js`,
//!   `rust`), in name order, each extension it holds (a folder holding `flmextension.yaml`,
//!   or a file `NAME.yaml`), with every file of a folder extension, recursively, in path
//!   order, except entries whose name begins with `.` and, for an extension written in Rust,
//!   its folders `src/` and `tests/` (its code and its tests): an extension's folder holds
//!   what ships and nothing else (the tools that build its committed files live in the
//!   repository's npm workspace, `npm/`). Each file's bytes are included with
//!   `include_bytes!` by absolute path.
//! - `$OUT_DIR/compiled.rs`, the table of code `COMPILED_EXTENSIONS`: for each extension of
//!   the folder `rust`, in name order, its stem and the static `FLM_EXTENSION` of its
//!   module (the stem with each `-` replaced by `_`).
//! - `$OUT_DIR/records.rs`, with the cargo feature `collection`: for every file of the table
//!   of files, in the same order, one invocation of `flm_ext::collection::record!`, which
//!   writes the file's record in the custom section `flmextensions` of the crate built as a
//!   WebAssembly module, the file's bytes included by absolute path. Without the feature,
//!   the file is empty.
//!
//! When the table of code has a row, or the feature `collection` is on, the script also sets
//! the cfg `compiled_extensions`, under which the crate holds the table of code and the
//! exports of a WebAssembly module: a collection has those exports even when no extension
//! written in Rust is among its features, and its table of code then has no row.
//!
//! The walk of a folder is repeated in the build script of the crate `flm-host`
//! (`flm-host/build.rs`), for the compose templates of the package `std`: a build script
//! shares no code without a crate of its own.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// The folders of extensions, in the order of the table of files.
const FOLDERS: [&str; 3] = ["data", "js", "rust"];

/// The folder of the extensions written in Rust, which are also in the table of code.
const RUST_FOLDER: &str = "rust";

/// The folders of an extension written in Rust that are not among its files: its code and
/// its tests.
const RUST_CODE_FOLDERS: [&str; 2] = ["src", "tests"];

/// The manifest of an extension written as a folder.
const MANIFEST_FILE: &str = "flmextension.yaml";

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"));
    let mut files = String::from(
        "/// The files of every extension whose feature is on, by folder then stem, in name order.\n",
    );
    files.push_str("pub static EXTENSIONS: &[flm_ext::source::BundledExtension] = &[\n");
    let mut compiled = String::from(
        "/// The code of every extension written in Rust whose feature is on, in name order.\n",
    );
    compiled.push_str("pub static COMPILED_EXTENSIONS: &[flm_ext::guest::NamedExtension] = &[\n");
    let mut records = String::new();
    let collection = std::env::var_os("CARGO_FEATURE_COLLECTION").is_some();
    let mut has_compiled = false;
    for folder_name in FOLDERS {
        println!("cargo::rerun-if-changed={folder_name}");
        let folder = root.join(folder_name);
        for (name, path) in entries(&folder) {
            if path.is_dir() {
                if !path.join(MANIFEST_FILE).is_file() || !feature_on(&name) {
                    continue;
                }
                let mut held = Vec::new();
                collect(&path, "", folder_name == RUST_FOLDER, &mut held);
                push_extension(&mut files, folder_name, &name, &held);
                if collection {
                    push_records(&mut records, folder_name, &name, &held);
                }
                if folder_name == RUST_FOLDER {
                    has_compiled = true;
                    let module = name.replace('-', "_");
                    let _ = writeln!(
                        compiled,
                        "    flm_ext::guest::NamedExtension {{ stem: {name:?}, compiled: &crate::{module}::FLM_EXTENSION }},"
                    );
                }
            } else if let Some(stem) = name.strip_suffix(".yaml").filter(|stem| feature_on(stem)) {
                let held = [(name.clone(), path)];
                push_extension(&mut files, folder_name, stem, &held);
                if collection {
                    push_records(&mut records, folder_name, stem, &held);
                }
            }
        }
    }
    files.push_str("];\n");
    compiled.push_str("];\n");
    println!("cargo::rustc-check-cfg=cfg(compiled_extensions)");
    if has_compiled || collection {
        println!("cargo::rustc-cfg=compiled_extensions");
    }
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("Cargo sets OUT_DIR"));
    fs::write(out.join("files.rs"), files).expect("the table of files is written");
    fs::write(out.join("compiled.rs"), compiled).expect("the table of code is written");
    fs::write(out.join("records.rs"), records).expect("the records of a collection are written");
}

/// Whether the cargo feature of the extension `stem` is on.
fn feature_on(stem: &str) -> bool {
    let variable = format!("CARGO_FEATURE_{}", stem.to_uppercase().replace('-', "_"));
    std::env::var_os(variable).is_some()
}

/// The entries of `folder` kept by the rule (nothing beginning with `.`), with their
/// names, in name order; none when the folder does not exist.
fn entries(folder: &Path) -> Vec<(String, PathBuf)> {
    let Ok(read) = fs::read_dir(folder) else { return Vec::new() };
    let mut entries: Vec<(String, PathBuf)> = read
        .map(|entry| entry.expect("a folder of extensions can be listed"))
        .filter_map(|entry| {
            let name = entry.file_name().into_string().expect("the name of a shipped file is UTF-8");
            (!name.starts_with('.')).then(|| (name, entry.path()))
        })
        .collect();
    entries.sort();
    entries
}

/// Adds to `files` every file under `folder`, recursively, each with its path relative to
/// the extension's folder (`prefix` is the path of `folder`, empty for the extension's own),
/// leaving out the folders `src/` and `tests/` of the extension's own folder when
/// `without_code` (an extension written in Rust).
fn collect(folder: &Path, prefix: &str, without_code: bool, files: &mut Vec<(String, PathBuf)>) {
    for (name, path) in entries(folder) {
        let is_dir = path.is_dir();
        if without_code && prefix.is_empty() && is_dir && RUST_CODE_FOLDERS.contains(&name.as_str()) {
            continue;
        }
        let relative = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
        if is_dir {
            collect(&path, &relative, without_code, files);
        } else {
            files.push((relative, path));
        }
    }
}

/// Writes the entry of one extension.
fn push_extension(table: &mut String, folder: &str, stem: &str, files: &[(String, PathBuf)]) {
    let _ = writeln!(table, "    flm_ext::source::BundledExtension {{\n        folder: {folder:?},\n        stem: {stem:?},\n        files: &[");
    for (relative, path) in files {
        let absolute = path.to_str().expect("the path of a shipped file is UTF-8");
        let _ = writeln!(
            table,
            "            flm_ext::source::BundledFile {{ path: {relative:?}, bytes: include_bytes!({absolute:?}) }},"
        );
    }
    table.push_str("        ],\n    },\n");
}

/// Writes the record of each file of one extension in the section of a collection.
fn push_records(records: &mut String, folder: &str, stem: &str, files: &[(String, PathBuf)]) {
    for (relative, path) in files {
        let absolute = path.to_str().expect("the path of a shipped file is UTF-8");
        let _ = writeln!(records, "flm_ext::collection::record!({folder:?}, {stem:?}, {relative:?}, {absolute:?});");
    }
}
