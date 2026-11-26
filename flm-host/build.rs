//! Writes the tables of the files compiled into the crate.
//!
//! **The compose templates.** The table of the files of the compose templates of the
//! package `std`, `$OUT_DIR/std_templates.rs`: every file under `src/stdpkg/compose_standard/templates/`,
//! recursively, in path order, except entries whose name begins with `.`, each by its path
//! `templates/` followed by its path relative to that folder, with `/` between the names
//! (`templates/html/none.yaml`). Each file's bytes are included with `include_bytes!` by
//! absolute path. The walk of the folder is the one of the build script of the crate
//! `flm-extensions` (`extensions/build.rs`): a build script shares no code without a crate
//! of its own.
//!
//! **The tools page of the live preview** (cargo feature `watch`). The table of the files
//! of the preview's tools page, `$OUT_DIR/watch_tools.rs`: every file under
//! `src/watch/client/tools/`, by the same walk, each by its path relative to that folder
//! (`index.html`), with its media type, found from its extension, and its bytes compressed
//! with gzip into `$OUT_DIR/watch_tools/` and included from there with `include_bytes!`.
//! A file whose extension has no media type in [`MEDIA_TYPES`] stops the build: the browser
//! needs the right type to run a script or apply a stylesheet.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

/// The folder of the compose templates of `std`, relative to the crate's folder.
const TEMPLATES: &str = "src/stdpkg/compose_standard/templates";

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("Cargo sets CARGO_MANIFEST_DIR"));
    templates(&root);
    #[cfg(feature = "watch")]
    watch_tools(&root);
}

/// Writes the table of the files of the compose templates of `std`.
fn templates(root: &Path) {
    println!("cargo:rerun-if-changed={TEMPLATES}");
    let mut files = Vec::new();
    collect(&root.join(TEMPLATES), "templates", &mut files);
    let mut table = String::from("/// Every file of the compose templates of `std`, by its path, in path order.\n");
    table.push_str("static FILES: &[(&str, &[u8])] = &[\n");
    for (relative, path) in &files {
        let absolute = path.to_str().expect("the path of a template's file is UTF-8");
        let _ = writeln!(table, "    ({relative:?}, include_bytes!({absolute:?})),");
    }
    table.push_str("];\n");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("Cargo sets OUT_DIR")).join("std_templates.rs");
    fs::write(&out, table).expect("the table of the compose templates of std is written");
}

/// The folder of the files of the preview's tools page, relative to the crate's folder.
#[cfg(feature = "watch")]
const TOOLS: &str = "src/watch/client/tools";

/// The media type of each extension of a file of the tools page.
#[cfg(feature = "watch")]
const MEDIA_TYPES: &[(&str, &str)] = &[
    ("html", "text/html"),
    ("js", "text/javascript"),
    ("mjs", "text/javascript"),
    ("css", "text/css"),
    ("json", "application/json"),
    ("map", "application/json"),
    ("svg", "image/svg+xml"),
    ("png", "image/png"),
    ("ico", "image/x-icon"),
    ("txt", "text/plain"),
    ("wasm", "application/wasm"),
    ("ttf", "font/ttf"),
    ("woff2", "font/woff2"),
    ("bcmap", "application/octet-stream"),
    ("pfb", "application/octet-stream"),
];

/// Writes the table of the files of the preview's tools page, each compressed with gzip.
#[cfg(feature = "watch")]
fn watch_tools(root: &Path) {
    use std::io::Write as _;

    use flate2::Compression;
    use flate2::write::GzEncoder;

    println!("cargo:rerun-if-changed={TOOLS}");
    let mut files = Vec::new();
    collect(&root.join(TOOLS), "", &mut files);
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("Cargo sets OUT_DIR"));
    let compressed = out.join("watch_tools");
    let mut table = String::from("/// Every file of the tools page, by its path, in path order: its media type and its bytes, compressed with gzip.\n");
    table.push_str("static FILES: &[(&str, &str, &[u8])] = &[\n");
    for (index, (relative, path)) in files.iter().enumerate() {
        // `collect` gives each path as `/` followed by the path within the folder.
        let relative = relative.trim_start_matches('/');
        let extension = relative.rsplit_once('.').map(|(_, extension)| extension).unwrap_or_default();
        let Some((_, media_type)) = MEDIA_TYPES.iter().find(|(known, _)| *known == extension) else {
            panic!("Unknown media type of the file ‘{relative}’ of the tools page (extension ‘{extension}’): add the extension to MEDIA_TYPES in build.rs");
        };
        let bytes = fs::read(path).expect("a file of the tools page can be read");
        let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
        encoder.write_all(&bytes).expect("a file of the tools page is compressed");
        let gzip = encoder.finish().expect("a file of the tools page is compressed");
        let target = compressed.join(format!("{index}.gz"));
        fs::create_dir_all(&compressed).expect("the folder of the compressed files is made");
        fs::write(&target, gzip).expect("a compressed file of the tools page is written");
        let absolute = target.to_str().expect("the path of the build's output folder is UTF-8");
        let _ = writeln!(table, "    ({relative:?}, {media_type:?}, include_bytes!({absolute:?})),");
    }
    table.push_str("];\n");
    fs::write(out.join("watch_tools.rs"), table).expect("the table of the tools page's files is written");
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
