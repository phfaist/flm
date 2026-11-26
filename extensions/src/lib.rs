//! This crate holds the extensions shipped with FLM, so that a program can compile them in
//! and offer them to its documents with nothing installed. It also builds as a WebAssembly
//! module that holds some of them.
//!
//! # The extensions and their cargo features
//!
//! Each extension has a cargo feature of its own, named after its folder in the crate:
//!
//! - `lecturenotes` (the folder `data/`, the engine `data`);
//! - `letter` and `math-mathjax` (the folder `js/`, the engine `js`);
//! - `flmdoc`, `math-techxt`, `phfpack`, `profile-pdf-latex`, `profile-tty` and
//!   `syntax-highlight` (the folder `rust/`, extensions written in Rust).
//!
//! The feature `all` turns on every extension, and no feature is on by default. The
//! features are additive: turning on one more feature adds its extension and changes
//! nothing else.
//!
//! # The two tables
//!
//! The crate describes the extensions whose features are on in two tables, whose row types
//! are defined in `flm-ext`:
//!
//! - [`EXTENSIONS`], the *table of files*, has a row for every extension
//!   ([`flm_ext::source::BundledExtension`]): its folder, its stem and its files. An
//!   extension written as a folder (a folder that holds `flmextension.yaml`) has every file
//!   under its folder, by its path relative to the folder, except the entries whose name
//!   begins with `.` and, for an extension written in Rust, its folders `src/` (its code)
//!   and `tests/`. An extension written as one file (`NAME.yaml`) has that file alone.
//! - `COMPILED_EXTENSIONS`, the *table of code*, has a row for every extension written in
//!   Rust (`flm_ext::guest::NamedExtension`): its stem and its compiled code. The static
//!   exists when the feature of at least one extension written in Rust, or the feature
//!   `collection`, is on.
//!
//! A program built on the host library `flm-host` gives both tables in its request, with
//! `flm_host::bundled::Bundled::new(EXTENSIONS).with_code(COMPILED_EXTENSIONS)`. The table
//! of code needs the cargo feature `native` of `flm-host`, which turns on the feature
//! `native` of `flm-ext` and the engine that runs the extensions written in Rust in the
//! program's own process. This crate never turns on `flm-ext`'s feature `native` itself
//! (see below). The crate reads no file at run time: the build script embeds every file in
//! the program.
//!
//! An extension's folder holds only the files that the extension ships. The tools that
//! build some of its committed files (the bundle of `lecturenotes`, MathJax's bundle under
//! `math-mathjax/vendor/`) are in the repository's npm workspace, `npm/`.
//!
//! # Extensions written in Rust
//!
//! An extension written in Rust is a module of this crate, named after the extension with
//! each `-` written `_` (`flm_extensions::profile_pdf_latex`). Its code is the folder
//! `rust/NAME/src/`, written against the guest client `flm_ext::guest`. The module declares
//! the static `FLM_EXTENSION`, the extension's code, which its row of the table of code
//! refers to.
//!
//! # WebAssembly modules and collections
//!
//! Built for `wasm32-unknown-unknown`, the crate is a WebAssembly module that holds the code
//! of the extensions written in Rust whose features are on: the macro
//! `flm_ext::guest::extensions!`, invoked over the table of code, writes the module's
//! exports. The build must run in a cargo run where no crate turns on `flm-ext`'s feature
//! `native`, with the package `flm-extensions` selected alone; otherwise the module has no
//! exports. The script `rust/build-wasm.sh` of the crate builds the two outputs:
//!
//! - the module of one extension written in Rust, built with that extension's feature
//!   alone, in the extension's folder beside its other files;
//! - a *collection*, built with the cargo feature `collection` and the features of the
//!   extensions it holds. The module then also holds every file of these extensions, those
//!   of the folders `data/` and `js/` included, in its custom section `flmextensions`
//!   (the format of [`flm_ext::collection`]). A collection has the exports of a module
//!   even when it holds no extension written in Rust. The feature `all` does not turn on
//!   `collection`.
//!
//! The records of the section and the exports are compiled for `wasm32` only, in a cargo
//! run where `flm-ext`'s feature `native` is off. In a program that links the crate, the
//! feature `collection` therefore changes nothing.

#[cfg(feature = "flmdoc")]
#[path = "../rust/flmdoc/src/lib.rs"]
pub mod flmdoc;
#[cfg(feature = "math-techxt")]
#[path = "../rust/math-techxt/src/lib.rs"]
pub mod math_techxt;
#[cfg(feature = "phfpack")]
#[path = "../rust/phfpack/src/lib.rs"]
pub mod phfpack;
#[cfg(feature = "profile-pdf-latex")]
#[path = "../rust/profile-pdf-latex/src/lib.rs"]
pub mod profile_pdf_latex;
#[cfg(feature = "profile-tty")]
#[path = "../rust/profile-tty/src/lib.rs"]
pub mod profile_tty;
#[cfg(feature = "syntax-highlight")]
#[path = "../rust/syntax-highlight/src/lib.rs"]
pub mod syntax_highlight;

include!(concat!(env!("OUT_DIR"), "/files.rs"));

// The table of code, when it has a row or the feature `collection` is on: the build script
// sets `compiled_extensions` then.
#[cfg(compiled_extensions)]
include!(concat!(env!("OUT_DIR"), "/compiled.rs"));

// The exports of the crate built as a WebAssembly module, over the table of code; in any
// other build, nothing.
#[cfg(compiled_extensions)]
flm_ext::guest::extensions!(COMPILED_EXTENSIONS);

// The records of the files of every extension of the table of files, in the custom section
// `flmextensions` of the crate built as a WebAssembly module (a collection); in any other
// build, nothing.
#[cfg(feature = "collection")]
include!(concat!(env!("OUT_DIR"), "/records.rs"));

#[cfg(all(test, feature = "all"))]
mod tests {
    use super::*;

    /// The row of [`EXTENSIONS`] whose stem is `stem`.
    fn find(stem: &str) -> Option<&'static flm_ext::source::BundledExtension> {
        EXTENSIONS.iter().find(|extension| extension.stem == stem)
    }

    /// The paths of the files of the extension `stem`.
    fn paths(stem: &str) -> Vec<&'static str> {
        find(stem).expect("bundled").files.iter().map(|file| file.path).collect()
    }

    /// Adds to `paths` every file under `folder`, recursively, by its path relative to the
    /// folder that `prefix` starts from, leaving out the names beginning with `.` and, in the
    /// extension's own folder, the names of `skipped`.
    fn walk(folder: &std::path::Path, prefix: &str, skipped: &[&str], paths: &mut Vec<String>) {
        for entry in std::fs::read_dir(folder).expect("the folder can be listed") {
            let entry = entry.expect("the folder can be listed");
            let name = entry.file_name().into_string().expect("the name is UTF-8");
            if name.starts_with('.') || (prefix.is_empty() && skipped.contains(&name.as_str())) {
                continue;
            }
            let relative = if prefix.is_empty() { name } else { format!("{prefix}/{name}") };
            if entry.path().is_dir() {
                walk(&entry.path(), &relative, skipped, paths);
            } else {
                paths.push(relative);
            }
        }
    }

    /// The extensions that the folder `folder` of the crate holds, in name order: each folder
    /// holding `flmextension.yaml`, and each file `NAME.yaml`.
    fn on_disk(folder: &str) -> Vec<String> {
        let mut stems: Vec<String> = std::fs::read_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(folder))
            .expect("the folder can be listed")
            .map(|entry| entry.expect("an entry"))
            .filter_map(|entry| {
                let name = entry.file_name().into_string().expect("UTF-8");
                if entry.path().join("flmextension.yaml").is_file() {
                    Some(name)
                } else {
                    name.strip_suffix(".yaml").filter(|_| !name.starts_with('.')).map(String::from)
                }
            })
            .collect();
        stems.sort_unstable();
        stems
    }

    /// The stems of the rows of the table of files from the folder `folder`, in the table's
    /// order.
    fn rows_of(folder: &str) -> Vec<&'static str> {
        EXTENSIONS.iter().filter(|extension| extension.folder == folder).map(|extension| extension.stem).collect()
    }

    #[test]
    fn the_table_of_files_holds_every_shipped_extension_and_its_files() {
        // Every extension of the three folders, by folder then stem, in name order.
        let folders: Vec<&str> = EXTENSIONS.iter().map(|extension| extension.folder).collect();
        let mut by_folder = folders.clone();
        by_folder.sort_by_key(|folder| ["data", "js", "rust"].iter().position(|f| f == folder));
        assert_eq!(folders, by_folder);
        for folder in ["data", "js", "rust"] {
            assert_eq!(rows_of(folder), on_disk(folder), "{folder}");
        }
        let lecturenotes = find("lecturenotes").expect("bundled");
        assert_eq!(lecturenotes.folder, "data");
        let mathjax = find("math-mathjax").expect("bundled");
        assert_eq!(mathjax.folder, "js");
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
            // An extension written as a folder: every file of the folder, and nothing else;
            // for one written in Rust, nothing of its code or its tests.
            let folder = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(extension.folder).join(extension.stem);
            if folder.is_dir() {
                let skipped: &[&str] = if extension.folder == "rust" { &["src", "tests"] } else { &[] };
                let mut on_disk = Vec::new();
                walk(&folder, "", skipped, &mut on_disk);
                on_disk.sort_unstable();
                assert_eq!(sorted, on_disk, "{}", extension.stem);
            }
            if extension.folder == "rust" {
                assert!(sorted.iter().all(|path| !path.starts_with("src/") && !path.starts_with("tests/")), "{sorted:?}");
            }
            // The crate has no YAML reader: the manifest holds one of the two keys of a
            // manifest at its root (the host's tests parse it).
            let manifest = extension.file("flmextension.yaml").or_else(|| extension.files.first()).expect("a manifest");
            let text = std::str::from_utf8(manifest.bytes).expect("the manifest is text");
            assert!(text.lines().any(|line| line.starts_with("extension:") || line.starts_with("flm:")), "{}", extension.stem);
        }
        assert_eq!(mathjax.file("main.js").map(|file| file.bytes), Some(&include_bytes!("../js/math-mathjax/main.js")[..]));
        let profile_pdf_latex = find("profile-pdf-latex").expect("bundled");
        assert_eq!(profile_pdf_latex.folder, "rust");
        assert_eq!(
            profile_pdf_latex.file("flmextension.yaml").map(|file| file.bytes),
            Some(&include_bytes!("../rust/profile-pdf-latex/flmextension.yaml")[..])
        );
    }

    #[test]
    fn the_table_of_code_holds_each_extension_of_the_folder_rust_by_its_stem() {
        let stems: Vec<&str> = COMPILED_EXTENSIONS.iter().map(|extension| extension.stem).collect();
        assert_eq!(stems, on_disk("rust"));
        assert_eq!(stems, rows_of("rust"));
        let profile_pdf_latex =
            COMPILED_EXTENSIONS.iter().find(|extension| extension.stem == "profile-pdf-latex").expect("compiled in");
        assert_eq!(profile_pdf_latex.compiled.name(), "profile-pdf-latex");
        assert!(!COMPILED_EXTENSIONS.iter().any(|extension| extension.stem == "lecturenotes"));
    }
}

#[cfg(all(
    test,
    feature = "phfpack",
    not(any(
        feature = "lecturenotes",
        feature = "letter",
        feature = "math-mathjax",
        feature = "flmdoc",
        feature = "math-techxt",
        feature = "profile-pdf-latex",
        feature = "profile-tty",
        feature = "syntax-highlight",
    ))
))]
mod one_feature_tests {
    use super::*;

    /// With one extension's feature alone, the tables hold that extension alone (run with
    /// `--no-default-features --features phfpack`).
    #[test]
    fn with_one_feature_alone_the_tables_hold_that_extension_alone() {
        let files: Vec<(&str, &str)> = EXTENSIONS.iter().map(|extension| (extension.folder, extension.stem)).collect();
        assert_eq!(files, [("rust", "phfpack")]);
        let code: Vec<&str> = COMPILED_EXTENSIONS.iter().map(|extension| extension.stem).collect();
        assert_eq!(code, ["phfpack"]);
    }
}
