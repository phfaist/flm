//! Writes the table of the bundled extensions, `$OUT_DIR/bundled.rs`: for each engine
//! folder (`data`, `js`), in name order, each extension it holds (a folder holding
//! `flmextension.yaml`, or a file `NAME.yaml`), with every file of a folder extension,
//! recursively, in path order, except entries whose name begins with `.`: an extension's
//! folder holds what ships and nothing else (the tools that build its committed files live
//! in the repository's npm workspace, `npm/`). Each file's bytes are included with
//! `include_bytes!` by absolute path.
//! The walk of a folder is repeated in the build script of the crate `flm-main`
//! (`flm-main/build.rs`), for the compose templates of the package `stdmain`: a build script
//! shares no code without a crate of its own.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// The engine folders, in the order of the table.
const ENGINE_FOLDERS: [&str; 2] = ["data", "js"];

/// The manifest of an extension written as a folder.
const MANIFEST_FILE: &str = "flmextension.yaml";

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"));
    let mut table = String::from("/// Every bundled extension, by engine folder then stem, in name order.\n");
    table.push_str("pub static EXTENSIONS: &[BundledExtension] = &[\n");
    for engine in ENGINE_FOLDERS {
        println!("cargo:rerun-if-changed={engine}");
        let folder = root.join(engine);
        for (name, path) in entries(&folder) {
            if path.is_dir() {
                if !path.join(MANIFEST_FILE).is_file() {
                    continue;
                }
                let mut files = Vec::new();
                collect(&path, "", &mut files);
                push_extension(&mut table, engine, &name, &files);
            } else if let Some(stem) = name.strip_suffix(".yaml") {
                push_extension(&mut table, engine, stem, &[(name.clone(), path)]);
            }
        }
    }
    table.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("Cargo sets OUT_DIR")).join("bundled.rs");
    fs::write(&out, table).expect("the table of bundled extensions is written");
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
/// the extension's folder (`prefix` is the path of `folder`, empty for the extension's own).
fn collect(folder: &Path, prefix: &str, files: &mut Vec<(String, PathBuf)>) {
    for (name, path) in entries(folder) {
        let relative = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
        if path.is_dir() {
            collect(&path, &relative, files);
        } else {
            files.push((relative, path));
        }
    }
}

/// Writes the entry of one extension.
fn push_extension(table: &mut String, engine: &str, stem: &str, files: &[(String, PathBuf)]) {
    let _ = writeln!(table, "    BundledExtension {{\n        engine_folder: {engine:?},\n        stem: {stem:?},\n        files: &[");
    for (relative, path) in files {
        let absolute = path.to_str().expect("the path of a shipped file is UTF-8");
        let _ = writeln!(
            table,
            "            BundledFile {{ path: {relative:?}, bytes: include_bytes!({absolute:?}) }},"
        );
    }
    table.push_str("        ],\n    },\n");
}
