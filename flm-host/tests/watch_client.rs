//! The watch client's built files, which this crate includes (`src/watch/client/`): the
//! page script and its stylesheet, and the tools page's files under `tools/`, checked
//! against a fresh build of their sources, the npm package `npm/flm-watchclient` of the
//! repository.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository's root folder.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// Every file under `folder`, by its path relative to `folder` (with `/`), and its bytes.
fn files_under(folder: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(folder: &Path, prefix: &str, files: &mut BTreeMap<String, Vec<u8>>) {
        for entry in std::fs::read_dir(folder).unwrap() {
            let entry = entry.unwrap();
            let name = format!("{prefix}{}", entry.file_name().to_str().unwrap());
            if entry.file_type().unwrap().is_dir() {
                walk(&entry.path(), &format!("{name}/"), files);
            } else {
                files.insert(name, std::fs::read(entry.path()).unwrap());
            }
        }
    }
    let mut files = BTreeMap::new();
    walk(folder, "", &mut files);
    files
}

#[test]
fn the_committed_bundle_matches_a_fresh_build() {
    let workspace = repository().join("npm");
    let member = workspace.join("flm-watchclient");
    if Command::new("node").arg("--version").output().is_err() {
        eprintln!("skipped: `node` is not on the PATH, so the files built by {} cannot be rebuilt", member.display());
        return;
    }
    if !["morphdom", "preact", "@preact/signals", "lucide-static", "pdfjs-dist", "highlight.js", "ansi_up", "esbuild", "typescript"].iter().all(|name| workspace.join("node_modules").join(name).is_dir()) {
        eprintln!("skipped: the npm workspace's dependencies are not installed (run `npm install` in {})", workspace.display());
        return;
    }
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flm-host-watch-client");
    let _ = std::fs::remove_dir_all(&out);
    let output = Command::new("node").arg("build.mjs").arg("--out").arg(&out).current_dir(&member).output().unwrap();
    assert!(output.status.success(), "{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    let committed_folder = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/watch/client");
    let fresh = files_under(&out);
    let committed = files_under(&committed_folder);
    let rebuild = format!("run `npm run build -w flm-watchclient` in {}", workspace.display());
    assert_eq!(
        fresh.keys().collect::<Vec<_>>(),
        committed.keys().collect::<Vec<_>>(),
        "the files of {} differ from those of a fresh build: {rebuild}",
        committed_folder.display()
    );
    for (name, bytes) in &fresh {
        assert!(bytes == &committed[name], "`{}` differs from a fresh build: {rebuild}", committed_folder.join(name).display());
    }
}
