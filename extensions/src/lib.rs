//! The extensions shipped with FLM, compiled into the program, in two tables:
//!
//! - every file of every extension under this crate's folders `data/` (the engine `data`)
//!   and `js/` (the engine `js`), as bytes in a static table ([`EXTENSIONS`]), found by an
//!   extension's stem ([`find`]);
//! - under the cargo feature `native`, the extensions written in Rust under the folder
//!   `rust/`, compiled in: each one's stem, the text of its manifest and its code, in the
//!   table `NATIVE_EXTENSIONS`, which the program gives to the engine `native` of `flm-ext`.
//!
//! An extension written as a folder (a folder holding `flmextension.yaml`) is in the first
//! table with every file under its folder, by its path relative to the folder with `/`
//! between the names, except entries whose name begins with `.`: an extension's folder holds
//! what ships and nothing else. The tools that build an extension's committed files (the
//! bundle of `lecturenotes`, MathJax's bundle under `math-mathjax/vendor/`) live in the
//! repository's npm workspace, `npm/`, outside this crate. An extension written as one file
//! (`NAME.yaml`) is in the table with that file alone, under its own name. The crate reads
//! nothing at run time; the program's host reads both tables as one more source of
//! extensions, last on its search path.
//!
//! An extension written in Rust is a crate of its own under `rust/` (a member of the
//! workspace), written against the guest client `flm_ext::guest`, and the same crate builds
//! as a WebAssembly module (`rust/build-wasm.sh`). Adding one takes a line in the
//! workspace's `members`, an optional dependency of this crate turned on by the feature
//! `native`, and a line of `NATIVE_EXTENSIONS`.

/// One file of a bundled extension.
#[derive(Debug)]
pub struct BundledFile {
    /// The path relative to the extension's folder, with `/` between the names
    /// (`templates/html/lecturenotes/flmtemplate.yaml`); for an extension written as one
    /// file, the file's name (`NAME.yaml`).
    pub path: &'static str,
    /// The file's bytes.
    pub bytes: &'static [u8],
}

/// One bundled extension: the folder of the engine it was found in, its stem and its files.
#[derive(Debug)]
pub struct BundledExtension {
    /// The engine folder the extension was found in: `data` or `js`.
    pub engine_folder: &'static str,
    /// The extension's stem: the name of its folder, or of its file without `.yaml`.
    pub stem: &'static str,
    /// The extension's files, in path order.
    pub files: &'static [BundledFile],
}

impl BundledExtension {
    /// The file at `path`, if the extension has one there.
    pub fn file(&self, path: &str) -> Option<&'static BundledFile> {
        self.files.iter().find(|file| file.path == path)
    }
}

include!(concat!(env!("OUT_DIR"), "/bundled.rs"));

/// The bundled extension whose stem is `stem`, the first in [`EXTENSIONS`] when two engine
/// folders hold one of that stem.
pub fn find(stem: &str) -> Option<&'static BundledExtension> {
    EXTENSIONS.iter().find(|extension| extension.stem == stem)
}

/// The extensions written in Rust and compiled into the program (cargo feature `native`),
/// in name order: for each, the stem a document's `use` entry gives, the text of its
/// manifest (`rust/STEM/flmextension.yaml`, the manifest of its WebAssembly form) and the
/// static `FLM_EXTENSION` that the macro `extension!` writes at the root of its crate.
#[cfg(feature = "native")]
pub static NATIVE_EXTENSIONS: &[flm_ext::native::NativeExtension] = &[flm_ext::native::NativeExtension {
    stem: "flmdoc",
    manifest: include_str!("../rust/flmdoc/flmextension.yaml"),
    compiled: &flmdoc::FLM_EXTENSION,
}, flm_ext::native::NativeExtension {
    stem: "profile-pdf-latex",
    manifest: include_str!("../rust/profile-pdf-latex/flmextension.yaml"),
    compiled: &profile_pdf_latex::FLM_EXTENSION,
}, flm_ext::native::NativeExtension {
    stem: "syntax-highlight",
    manifest: include_str!("../rust/syntax-highlight/flmextension.yaml"),
    compiled: &syntax_highlight::FLM_EXTENSION,
}];

/// The extension written in Rust whose stem is `stem`, compiled into the program (cargo
/// feature `native`).
#[cfg(feature = "native")]
pub fn find_native(stem: &str) -> Option<&'static flm_ext::native::NativeExtension> {
    NATIVE_EXTENSIONS.iter().find(|extension| extension.stem == stem)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The paths of the files of the extension `stem`.
    fn paths(stem: &str) -> Vec<&'static str> {
        find(stem).expect("bundled").files.iter().map(|file| file.path).collect()
    }

    /// Adds to `paths` every file under `folder`, recursively, by its path relative to the
    /// folder that `prefix` starts from, leaving out the names beginning with `.`.
    fn walk(folder: &std::path::Path, prefix: &str, paths: &mut Vec<String>) {
        for entry in std::fs::read_dir(folder).expect("the folder can be listed") {
            let entry = entry.expect("the folder can be listed");
            let name = entry.file_name().into_string().expect("the name is UTF-8");
            if name.starts_with('.') {
                continue;
            }
            let relative = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            if entry.path().is_dir() {
                walk(&entry.path(), &relative, paths);
            } else {
                paths.push(relative);
            }
        }
    }

    #[test]
    fn the_table_holds_the_two_shipped_extensions_and_their_files() {
        let lecturenotes = find("lecturenotes").expect("bundled");
        assert_eq!(lecturenotes.engine_folder, "data");
        let mathjax = find("math-mathjax").expect("bundled");
        assert_eq!(mathjax.engine_folder, "js");
        for path in [
            "flmextension.yaml",
            "templates/html/lecturenotes/flmtemplate.yaml",
            "templates/html/lecturenotes/dist/lecturenotes.css",
        ] {
            assert!(paths("lecturenotes").contains(&path), "{path}");
        }
        for path in ["flmextension.yaml", "main.js", "vendor/mathjax.js"] {
            assert!(paths("math-mathjax").contains(&path), "{path}");
        }
        assert!(find("nothing").is_none());
        for extension in EXTENSIONS {
            let mut sorted: Vec<&str> = extension.files.iter().map(|file| file.path).collect();
            sorted.sort_unstable();
            sorted.dedup();
            assert_eq!(sorted.len(), extension.files.len(), "{}", extension.stem);
            // An extension written as a folder: every file of the folder, and nothing else.
            let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(extension.engine_folder).join(extension.stem);
            if folder.is_dir() {
                let mut on_disk = Vec::new();
                walk(&folder, "", &mut on_disk);
                on_disk.sort_unstable();
                assert_eq!(sorted, on_disk, "{}", extension.stem);
            }
            // The crate has no YAML reader: the manifest holds one of the two keys of a
            // manifest at its root (the host's tests parse it).
            let manifest = extension.file("flmextension.yaml").or_else(|| extension.files.first()).expect("a manifest");
            let text = std::str::from_utf8(manifest.bytes).expect("the manifest is text");
            assert!(text.lines().any(|line| line.starts_with("extension:") || line.starts_with("flm:")), "{}", extension.stem);
        }
        assert_eq!(mathjax.file("main.js").map(|file| file.bytes), Some(&include_bytes!("../js/math-mathjax/main.js")[..]));
        // The folder `rust/` is not an engine folder: its extensions are compiled in.
        assert!(find("profile-pdf-latex").is_none());
    }

    #[cfg(feature = "native")]
    #[test]
    fn the_native_table_holds_each_extension_of_the_folder_rust_by_its_stem() {
        let stems: Vec<&str> = NATIVE_EXTENSIONS.iter().map(|extension| extension.stem).collect();
        let mut sorted = stems.clone();
        sorted.sort_unstable();
        assert_eq!(stems, sorted);
        let mut folders: Vec<String> = std::fs::read_dir(concat!(env!("CARGO_MANIFEST_DIR"), "/rust"))
            .expect("the folder rust")
            .map(|entry| entry.expect("an entry"))
            .filter(|entry| entry.path().join("Cargo.toml").is_file())
            .map(|entry| entry.file_name().into_string().expect("UTF-8"))
            .collect();
        folders.sort_unstable();
        assert_eq!(stems, folders);
        let profile_pdf_latex = find_native("profile-pdf-latex").expect("compiled in");
        assert_eq!(profile_pdf_latex.manifest, include_str!("../rust/profile-pdf-latex/flmextension.yaml"));
        assert_eq!(profile_pdf_latex.compiled.name(), "profile-pdf-latex");
        assert!(find_native("lecturenotes").is_none());
    }
}
