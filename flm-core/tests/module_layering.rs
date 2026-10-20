//! The crate's module layering, checked against the source tree rather than
//! against review discipline.
//!
//! The rule stated in the crate documentation is
//!
//! ```text
//! {model, lang} < core < dump < stdcore < math_passthrough        layout standalone
//! ```
//!
//! and it is meant literally: a module names another module of this crate only
//! if that module is at its own level or below. This test reads the files under
//! `flm-core/src` at test time and fails on any `crate::…` path that points
//! upward. Reading files from a test is what test-facing code is allowed to do;
//! nothing in the library itself touches the file system.
//!
//! When a top-level module is added, renamed or removed, `ALLOWED` below has to
//! be updated in the same change — the second test fails otherwise, so the
//! table cannot silently fall behind the source tree.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// For each top-level module, the top-level modules it may name. A module
/// always names itself, so every row contains its own name.
///
/// `model` and `lang` share a row because they are one level, not two: a node
/// carries its element, so the language needs the model, and the model's
/// fragments and traits are typed by the language, so the model needs the
/// language.
const ALLOWED: &[(&str, &[&str])] = &[
    ("model", &["model", "lang"]),
    ("lang", &["model", "lang"]),
    ("core", &["model", "lang", "core"]),
    ("dump", &["model", "lang", "core", "dump"]),
    ("stdcore", &["model", "lang", "core", "dump", "stdcore", "layout"]),
    // The package `math-passthrough` uses the standard package `stdcore`.
    ("math_passthrough", &["model", "lang", "core", "dump", "stdcore", "math_passthrough"]),
    // The layout engine stands apart: it names no other module of the crate,
    // which is what would make extracting it into a crate of its own a move —
    // as was done with the Unicode-to-LaTeX encoder, now the published crate
    // `untechxt`.
    ("layout", &["layout"]),
];

/// The path of the crate's source directory.
fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Every `.rs` file under `dir`, in a deterministic order.
fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut entries: Vec<PathBuf> =
        fs::read_dir(dir).expect("the crate's source directory is readable").map(
            |entry| entry.expect("a directory entry of the source tree is readable").path(),
        ).collect();
    entries.sort();
    let mut files = Vec::new();
    for entry in entries {
        if entry.is_dir() {
            files.extend(rust_files(&entry));
        } else if entry.extension().is_some_and(|ext| ext == "rs") {
            files.push(entry);
        }
    }
    files
}

/// The line with any `//` comment cut off, so that the doc links and the code
/// samples of the documentation are not read as imports. Cutting at a `//`
/// inside a string literal can only hide a path, never invent one.
fn code_of(line: &str) -> &str {
    match line.find("//") {
        Some(pos) => &line[..pos],
        None => line,
    }
}

/// The top-level modules that `text` names through a `crate::…` path.
fn named_modules(text: &str) -> BTreeSet<String> {
    let mut named = BTreeSet::new();
    let mut rest = text;
    while let Some(pos) = rest.find("crate::") {
        rest = &rest[pos + "crate::".len()..];
        let end = rest
            .find(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        if end > 0 {
            named.insert(rest[..end].to_string());
        }
    }
    named
}

/// The top-level module a source file belongs to: the first path component
/// under `src`, for a file that is not the crate root.
fn top_module_of(file: &Path, src: &Path) -> Option<String> {
    let relative = file.strip_prefix(src).expect("every file walked lies under src");
    let first = relative.components().next()?.as_os_str().to_string_lossy().to_string();
    // `lib.rs` is the crate root: it declares the modules and documents the
    // layering, and belongs to no module of its own.
    if first.ends_with(".rs") { None } else { Some(first) }
}

#[test]
fn no_module_names_a_module_above_its_own_level() {
    let src = src_dir();
    let mut violations: Vec<String> = Vec::new();
    let tops: BTreeSet<&str> = ALLOWED.iter().map(|(top, _)| *top).collect();

    for file in rust_files(&src) {
        let Some(top) = top_module_of(&file, &src) else { continue };
        let allowed = ALLOWED
            .iter()
            .find(|(name, _)| *name == top)
            .map(|(_, allowed)| *allowed)
            .unwrap_or_else(|| panic!("the module `{top}` is missing from the ALLOWED table"));
        let text = fs::read_to_string(&file).expect("a source file of the crate is readable");
        for (number, line) in text.lines().enumerate() {
            for named in named_modules(code_of(line)) {
                // A `crate::` path may also name an item re-exported at the
                // crate root; only the top-level modules are checked here.
                if !tops.contains(named.as_str()) || allowed.contains(&named.as_str()) {
                    continue;
                }
                let shown = file.strip_prefix(&src).unwrap_or(&file).display();
                violations.push(format!(
                    "{shown}:{}: `{top}` names `crate::{named}`",
                    number + 1
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "the module layering `{{model, lang}} < core < dump < stdcore < math_passthrough` is broken by:\n{}",
        violations.join("\n")
    );
}

#[test]
fn the_allowed_table_lists_exactly_the_crate_s_top_level_modules() {
    let src = src_dir();
    let mut found: BTreeSet<String> = BTreeSet::new();
    for entry in fs::read_dir(&src).expect("the crate's source directory is readable") {
        let path = entry.expect("a directory entry of the source tree is readable").path();
        let name = path.file_name().expect("a directory entry has a name").to_string_lossy();
        if path.is_dir() {
            found.insert(name.to_string());
        } else if name != "lib.rs" && path.extension().is_some_and(|ext| ext == "rs") {
            found.insert(name.trim_end_matches(".rs").to_string());
        }
    }
    let listed: BTreeSet<String> = ALLOWED.iter().map(|(top, _)| (*top).to_string()).collect();
    assert_eq!(
        found, listed,
        "the ALLOWED table and the top-level modules under src/ have drifted apart"
    );
}
