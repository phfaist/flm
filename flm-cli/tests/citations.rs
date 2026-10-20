//! Citations through the program `flm`: where the cache file of the fetched records stands,
//! the entries of a bibliography's works, a style file named in an extension, and the two
//! exports, BibTeX and CSL-JSON.
//! The documents cite works of bibliography files only, so that no test opens a connection.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fresh folder for one test, under the build folder.
fn folder(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-citations-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// Writes `text` at `path` below `dir`, making the folders.
fn write(dir: &Path, path: &str, text: &str) {
    let path = dir.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Runs `flm` with `args` in the folder `cwd`, its home folder there too.
fn flm(cwd: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_flm"));
    command.args(args).current_dir(cwd).env("HOME", cwd).env_remove("FLMPATH").env_remove("USERPROFILE");
    command.output().unwrap()
}

const PAPER: &str =
    "---\nflm:\n  features:\n    citations-standard:\n      bibliography: [refs.json]\n---\nSee \\cite{bib:knuth84}.\n";

const REFS: &str = r#"[{"id": "knuth84", "type": "book", "title": "The TeXbook", "author": [{"family": "Knuth", "given": "Donald E."}], "issued": {"date-parts": [[1984]]}}]"#;

#[test]
fn the_cache_file_stands_beside_the_document_whatever_the_current_folder() {
    let root = folder("cache-beside");
    write(&root, "doc/paper.flm", PAPER);
    write(&root, "doc/refs.json", REFS);
    std::fs::create_dir_all(root.join("elsewhere")).unwrap();
    let output = flm(&root.join("elsewhere"), &["render", "-p", "text", "../doc/paper.flm"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("D. E. Knuth"), "{stdout}");
    assert!(root.join("doc/.flm-citations.jsonl").is_file());
    assert!(!root.join("elsewhere/.flm-citations.jsonl").exists());
}

#[test]
fn a_document_named_without_a_folder_keeps_its_cache_file_in_the_current_folder() {
    let root = folder("cache-bare-name");
    write(&root, "paper.flm", PAPER);
    write(&root, "refs.json", REFS);
    let output = flm(&root, &["render", "-p", "text", "paper.flm"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("D. E. Knuth"), "{stdout}");
    assert!(root.join(".flm-citations.jsonl").is_file());
}

// --- the entries and the exports through the program -----------------------------------------

/// A bibliography of three works: an article with the two fields the BibTeX export patches
/// (a date `published` and no `issued`; a list of ISSNs), a book, and notes by an
/// institution, whose DOI and address hold characters special to LaTeX.
const LIBRARY: &str = r#"[
  {"id": "calderbank96", "type": "article-journal",
   "author": [{"family": "Calderbank", "given": "A. R."}, {"family": "Shor", "given": "Peter W."}],
   "title": "Good quantum error-correcting codes exist", "container-title": "Physical Review A",
   "volume": "54", "issue": "2", "page": "1098-1105", "published": {"date-parts": [[1996, 8, 1]]},
   "ISSN": ["1050-2947", "1094-1622"], "DOI": "10.1103/physreva.54.1098", "arxivid": "quant-ph/9512032"},
  {"id": "knuth84", "type": "book", "author": [{"family": "Knuth", "given": "Donald E."}],
   "title": "The TeXbook: 100% {braces} & $x^2$", "publisher": "Addison-Wesley", "annote": "A manual & more", "issued": {"date-parts": [[1984]]}},
  {"id": "lhc", "author": [{"name": "The LHC Collaboration"}], "title": "Notes", "issued": {"date-parts": [[2020]]},
   "DOI": "10.1142/9789814449243_0008", "URL": "https://example.org/~lhc/notes_2020%20v2"}
]"#;

/// The document citing, in this order, `bib:knuth84`, `bib:calderbank96`, the book again
/// under a second spelling of its key, and `bib:lhc`.
const CITING: &str = "---\nflm:\n  features:\n    citations-standard:\n      bibliography: [library.json]\n---\n\
                      See \\cite{bib:knuth84}, \\cite{bib:calderbank96}, \\cite{BIB:knuth84} and \\cite{bib:lhc}.\n";

/// The keys of the document, as written, in order of first appearance.
const KEYS: [&str; 4] = ["bib:knuth84", "bib:calderbank96", "BIB:knuth84", "bib:lhc"];

/// A folder holding the document `doc/paper.flm` citing the works of `doc/library.json`.
fn citing(name: &str) -> PathBuf {
    let root = folder(name);
    write(&root, "doc/paper.flm", CITING);
    write(&root, "doc/library.json", LIBRARY);
    root
}

#[test]
fn a_document_citing_works_of_a_bibliography_lists_their_entries() {
    let root = citing("entries");
    let output = flm(&root, &["render", "-p", "text", "doc/paper.flm"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8_lossy(&output.stdout);
    // The article states its date as `published` alone, which the style does not read.
    assert!(text.contains("A. R. Calderbank and P. W. Shor, Physical Review A 54, 1098."), "{text}");
    assert!(text.contains("D. E. Knuth,"), "{text}");
    assert!(text.contains("The LHC Collaboration, (2020)."), "{text}");
}

#[test]
fn the_csl_json_export_holds_one_record_per_key_as_written_in_order() {
    let root = citing("csl-json");
    let output = flm(&root, &["render", "-p", "csl-json", "doc/paper.flm"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let records: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    let records = records.as_array().expect("a list of records");
    let ids: Vec<&str> = records.iter().map(|record| record["id"].as_str().unwrap()).collect();
    assert_eq!(ids, KEYS);
    // The two spellings of one work: two records holding the same details.
    assert_eq!(records[0]["title"], records[2]["title"]);
    // `type` written, `null` where the record has none; a name alone made a family name.
    assert_eq!(records[3]["type"], serde_json::Value::Null);
    assert_eq!(records[3]["author"][0]["family"], "The LHC Collaboration");
    assert!(records.iter().all(|record| record.get("key").is_none()));
}

#[test]
fn the_bibtex_export_is_read_back_and_carries_the_two_patches() {
    let root = citing("bibtex");
    let output = flm(&root, &["render", "-p", "bibtex", "doc/paper.flm"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8(output.stdout).unwrap();
    let bibliography = biblatex::Bibliography::parse(&text).unwrap_or_else(|error| panic!("{error:?}\n{text}"));
    let keys: Vec<&str> = bibliography.iter().map(|entry| entry.key.as_str()).collect();
    assert_eq!(keys, KEYS, "{text}");
    let article = bibliography.get("bib:calderbank96").expect("the article");
    // The year from `published`, the ISSNs joined by a space.
    assert!(text.contains("year = {1996}"), "{text}");
    // The month as BibTeX's macro for it.
    assert!(text.contains("month = aug,"), "{text}");
    assert!(text.contains("issn = {1050-2947 1094-1622}"), "{text}");
    assert_eq!(article.entry_type.to_string(), "article");
    assert!(text.contains("author = {Calderbank, A. R. and Shor, Peter W.}"), "{text}");
    // The characters special to LaTeX written by their commands, the formula kept a formula.
    assert!(text.contains("100\\% \\textbraceleft{}braces\\textbraceright{} \\& $x^2$"), "{text}");
    // A field of the earlier export that BibTeX's own styles do not read.
    assert!(text.contains("annote = {A manual \\& more}"), "{text}");
    // An institution's name is one name.
    assert!(text.contains("author = {{The LHC Collaboration}}"), "{text}");
    // An identifier and an address written as they are.
    assert!(text.contains("doi = {10.1142/9789814449243_0008}"), "{text}");
    assert!(text.contains("url = {https://example.org/~lhc/notes_2020%20v2}"), "{text}");
    let notes = bibliography.get("bib:lhc").expect("the notes");
    assert!(notes.fields.contains_key("doi") && notes.fields.contains_key("url"), "{text}");
}

#[test]
fn an_export_is_written_to_standard_output_or_as_index_bib_in_the_output_folder() {
    let root = citing("written");
    let printed = flm(&root, &["render", "-p", "bibtex", "doc/paper.flm"]);
    assert!(printed.status.success(), "{}", String::from_utf8_lossy(&printed.stderr));
    let written = flm(&root, &["render", "-p", "bibtex", "-o", "out/", "doc/paper.flm"]);
    assert!(written.status.success(), "{}", String::from_utf8_lossy(&written.stderr));
    assert_eq!(std::fs::read(root.join("out/index.bib")).unwrap(), printed.stdout);
    let json = flm(&root, &["render", "-p", "csl-json", "-o", "out/", "doc/paper.flm"]);
    assert!(json.status.success(), "{}", String::from_utf8_lossy(&json.stderr));
    assert!(root.join("out/index.json").is_file());
}

#[test]
fn a_cited_work_with_no_record_is_an_error_of_the_export() {
    let root = folder("no-record");
    write(&root, "paper.flm", "---\nflm:\n  features:\n    citations-standard:\n      bibliography: [library.json]\n---\nSee \\cite{bib:absent}.\n");
    write(&root, "library.json", LIBRARY);
    let output = flm(&root, &["render", "-p", "csl-json", "paper.flm"]);
    assert!(!output.status.success());
    let errors = String::from_utf8_lossy(&output.stderr);
    assert!(errors.contains("the citation key `bib:absent` must have a bibliography record to be exported"), "{errors}");
}

/// A style writing a work's title alone.
const TITLES: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<style xmlns="http://purl.org/net/xbiblio/csl" class="in-text" version="1.0">
  <info><title>Titles</title><id>titles</id><updated>2026-10-01T00:00:00+00:00</updated></info>
  <citation><layout><text variable="citation-number"/></layout></citation>
  <bibliography><layout><text variable="title" prefix="Title: "/></layout></bibliography>
</style>
"#;

#[test]
fn a_style_file_named_in_a_package_is_found_beside_the_package() {
    let root = folder("style-in-package");
    write(&root, "doc/_flmextensions/titles-pkg.yaml", "flm:\n  features:\n    citations-standard:\n      style: {file: styles/titles.csl}\n");
    write(&root, "doc/_flmextensions/styles/titles.csl", TITLES);
    write(&root, "doc/library.json", LIBRARY);
    write(
        &root,
        "doc/paper.flm",
        "---\nflm:\n  use: [titles-pkg]\n  features:\n    citations-standard:\n      bibliography: [library.json]\n---\nSee \\cite{bib:lhc}.\n",
    );
    let output = flm(&root, &["render", "-p", "text", "doc/paper.flm"]);
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Title: Notes"), "{text}");
}
