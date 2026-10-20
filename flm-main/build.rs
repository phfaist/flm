//! Writes the table of the files of the compose templates of the package `stdmain`,
//! `$OUT_DIR/std_templates.rs`: every file under `src/stdpkg/compose_standard/templates/`,
//! recursively, in path order, except entries whose name begins with `.`, each by its path
//! `templates/` followed by its path relative to that folder, with `/` between the names
//! (`templates/html/none.yaml`). Each file's bytes are included with `include_bytes!` by
//! absolute path. The walk of the folder is the one of the build script of the crate
//! `flm-extensions` (`extensions/build.rs`): a build script shares no code without a crate
//! of its own.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// The folder of `stdmain`'s compose templates, relative to the crate's folder.
const TEMPLATES: &str = "src/stdpkg/compose_standard/templates";

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"));
    println!("cargo:rerun-if-changed={TEMPLATES}");
    let mut files = Vec::new();
    collect(&root.join(TEMPLATES), "templates", &mut files);
    let mut table = String::from("/// Every file of `stdmain`'s compose templates, by its path, in path order.\n");
    table.push_str("static FILES: &[(&str, &[u8])] = &[\n");
    for (relative, path) in &files {
        let absolute = path.to_str().expect("the path of a template's file is UTF-8");
        let _ = writeln!(table, "    ({relative:?}, include_bytes!({absolute:?})),");
    }
    table.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("Cargo sets OUT_DIR")).join("std_templates.rs");
    fs::write(&out, table).expect("the table of stdmain's compose templates is written");
}

/// The entries of `folder` kept by the rule (nothing beginning with `.`), with their names,
/// in name order; none when the folder does not exist.
fn entries(folder: &Path) -> Vec<(String, PathBuf)> {
    let Ok(read) = fs::read_dir(folder) else { return Vec::new() };
    let mut entries: Vec<(String, PathBuf)> = read
        .map(|entry| entry.expect("the folder of the templates can be listed"))
        .filter_map(|entry| {
            let name = entry.file_name().into_string().expect("the name of a template's file is UTF-8");
            (!name.starts_with('.')).then(|| (name, entry.path()))
        })
        .collect();
    entries.sort();
    entries
}

/// Adds to `files` every file under `folder`, recursively, each with its path `prefix`
/// followed by its path relative to `folder`.
fn collect(folder: &Path, prefix: &str, files: &mut Vec<(String, PathBuf)>) {
    for (name, path) in entries(folder) {
        let relative = format!("{prefix}/{name}");
        if path.is_dir() {
            collect(&path, &relative, files);
        } else {
            files.push((relative, path));
        }
    }
}
