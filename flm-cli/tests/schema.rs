//! `flm schema`, run as the binary: it prints a JSON Schema, the same bytes at every run;
//! `-o` writes it to a file; `--of` chooses the schema of an extension's manifest or of a
//! front matter; a document given covers the packages it uses, as its preparation finds
//! them; the schema suggests the packages and the extensions installed on the search path;
//! a file that cannot be written is reported.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A fresh folder for one test, under the build folder.
fn folder(name: &str) -> PathBuf {
    let path = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-cli-schema-{name}"));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// Runs `flm` with `args` in the folder `cwd`, its home folder there too and `FLMPATH`
/// unset.
fn flm(cwd: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flm"))
        .args(args)
        .current_dir(cwd)
        .env("HOME", cwd)
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE")
        .output()
        .unwrap()
}

/// Writes `text` at `path` below `dir`, making the folders.
fn write(dir: &Path, path: &str, text: &str) {
    let path = dir.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// The JSON Schema a run printed, checked to be one.
fn schema(output: &Output) -> serde_json::Value {
    assert!(output.status.success(), "{output:?}");
    let schema: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON");
    assert_eq!(schema["$schema"], "https://json-schema.org/draft/2020-12/schema");
    schema
}

#[test]
fn flm_schema_prints_the_same_json_schema_at_every_run() {
    let dir = folder("prints");
    let first = flm(&dir, &["schema"]);
    let second = flm(&dir, &["schema"]);
    let schema = schema(&first);
    assert_eq!(first.stdout, second.stdout);
    assert!(first.stdout.ends_with(b"}\n"));
    // An extension's manifest by default: `extension` and `flm` at the root, the
    // features' settings under `flm.features`.
    assert!(schema["properties"]["extension"].is_object());
    let flm_keys = &schema["properties"]["flm"]["properties"];
    assert!(flm_keys["workflows"].is_object());
    assert!(flm_keys["features"]["properties"]["floats"].is_object());
    assert_eq!(flm(&dir, &["schema", "--of", "extension"]).stdout, first.stdout);
}

#[test]
fn of_chooses_the_schema_of_an_extension_manifest_or_of_a_front_matter() {
    let dir = folder("of");
    let manifest = schema(&flm(&dir, &["schema", "--of", "extension"]));
    let front_matter = schema(&flm(&dir, &["schema", "--of", "front-matter"]));
    assert!(manifest["properties"]["extension"].is_object());
    assert!(manifest["properties"].get("title").is_none());
    assert!(front_matter["properties"].get("extension").is_none());
    assert!(front_matter["properties"]["flm"]["properties"]["features"].is_object());
    assert!(front_matter["properties"]["title"].is_object());
    assert!(front_matter["properties"].get("workflows").is_none());
    let unknown = flm(&dir, &["schema", "--of", "document"]);
    assert_eq!(unknown.status.code(), Some(2));
}

/// The schema of a front matter describes `default_profile`, under a map that admits only
/// the keys it describes.
#[test]
fn the_front_matter_s_schema_accepts_default_profile() {
    let dir = folder("default-profile");
    let front_matter = schema(&flm(&dir, &["schema", "--of", "front-matter"]));
    let flm_block = &front_matter["properties"]["flm"];
    assert_eq!(flm_block["additionalProperties"], serde_json::json!(false));
    assert_eq!(flm_block["properties"]["default_profile"]["type"], "string");
}

#[test]
fn o_writes_the_schema_to_a_file() {
    let dir = folder("file");
    let printed = flm(&dir, &["schema", "--of", "front-matter"]);
    let written = flm(&dir, &["schema", "--of", "front-matter", "-o", "flm.schema.json"]);
    assert!(written.status.success(), "{written:?}");
    assert!(written.stdout.is_empty());
    assert_eq!(std::fs::read(dir.join("flm.schema.json")).unwrap(), printed.stdout);
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1, "the schema's file alone is written");
}

#[test]
fn a_file_that_cannot_be_written_is_reported() {
    let dir = folder("unwritable");
    let output = flm(&dir, &["schema", "-o", "nosuch/flm.schema.json"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stderr).contains("nosuch/flm.schema.json"), "{output:?}");
}

/// The names of the features whose settings the front matter's schema `schema` describes.
fn features(schema: &serde_json::Value) -> Vec<String> {
    schema["properties"]["flm"]["properties"]["features"]["properties"]
        .as_object()
        .expect("the features")
        .keys()
        .cloned()
        .collect()
}

/// Without a document, the schema covers the features of `stdcore` and of `stdmain`
/// (`std` has none of its own), and describes the front matter's `initial_use`; a document
/// whose initial list leaves `std` out has a schema without `stdmain`'s features.
#[test]
fn the_schema_covers_stdcore_and_stdmain_and_a_document_s_initial_list() {
    let dir = folder("stdmain");
    let compiled = schema(&flm(&dir, &["schema", "--of", "front-matter"]));
    let names = features(&compiled);
    for feature in ["floats", "compose", "compose-standard", "write", "preview", "graphics-files", "citations-standard"] {
        assert!(names.iter().any(|name| name == feature), "{feature}: {names:?}");
    }
    assert!(compiled["properties"]["flm"]["properties"]["initial_use"].is_object());
    write(&dir, "paper.flm", "---\nflm:\n  initial_use: stdcore\n---\nHello.\n");
    let names = features(&schema(&flm(&dir, &["schema", "--of", "front-matter", "paper.flm"])));
    assert!(names.iter().any(|name| name == "floats"), "{names:?}");
    assert!(!names.iter().any(|name| name == "write"), "{names:?}");
}

/// Without a document, the schema covers the package of the program's math processor too:
/// `math-mathjax`, an extension bundled into the program (cargo features `js` and
/// `bundled-extensions`), whose feature no compiled package holds.
#[cfg(all(feature = "js", feature = "bundled-extensions"))]
#[test]
fn the_schema_without_a_document_covers_the_program_s_math_processor() {
    let dir = folder("math-processor");
    let names = features(&schema(&flm(&dir, &["schema", "--of", "front-matter"])));
    assert!(names.iter().any(|name| name == "math-mathjax"), "{names:?}");
    let manifest = schema(&flm(&dir, &["schema"]));
    assert!(manifest["properties"]["flm"]["properties"]["features"]["properties"]["math-mathjax"].is_object());
}

/// A document given to `flm schema` that chooses its math processor with `math` makes the
/// schema cover that processor's package and not the program's; the front matter's schema
/// describes the key `math`.
#[test]
fn a_document_s_math_processor_is_the_one_its_schema_covers() {
    let dir = folder("document-math");
    write(&dir, "paper.flm", "---\nflm:\n  math: passthrough\n---\nHello.\n");
    let printed = schema(&flm(&dir, &["schema", "--of", "front-matter", "paper.flm"]));
    let names = features(&printed);
    assert!(names.iter().any(|name| name == "math-passthrough"), "{names:?}");
    assert!(!names.iter().any(|name| name == "math-mathjax"), "{names:?}");
    assert!(printed["properties"]["flm"]["properties"]["math"]["anyOf"].is_array());
}

/// A document given to `flm schema` makes the schema cover the packages that its
/// preparation uses: the templates feature of the extension `marked`, found in
/// `_flmextensions` beside it, which the schema of the compiled packages lacks; the same
/// features as [`flm_main::session::prepare`] resolves for the document
/// ([`flm_main::session::Prepared::factories`], what the preview's settings tool reads).
#[test]
fn a_document_given_to_schema_adds_the_features_of_the_extensions_it_uses() {
    let dir = folder("document");
    write(&dir, "paper.flm", "---\nflm:\n  use: [marked]\n---\nHello.\n");
    write(&dir, "_flmextensions/marked/flmextension.yaml", "extension:\n  description: A template between markers.\n");
    write(
        &dir,
        "_flmextensions/marked/templates/text/marked/flmtemplate.yaml",
        "output: text\nengine: minijinja\nmain: marked.txt\naccepts: all\n",
    );
    write(&dir, "_flmextensions/marked/templates/text/marked/marked.txt", "BEGIN\n{{ regions.content }}\nEND\n");
    let document = dir.join("paper.flm");
    let document = document.to_str().unwrap();
    let compiled = schema(&flm(&dir, &["schema", "--of", "front-matter"]));
    let printed = schema(&flm(&dir, &["schema", "--of", "front-matter", document]));
    let templates = |schema: &serde_json::Value| features(schema).into_iter().filter(|name| name.ends_with("marked/flmextension.yaml#_")).count();
    assert_eq!(templates(&compiled), 0);
    assert_eq!(templates(&printed), 1, "{:?}", features(&printed));
    // The manifest's schema covers the same packages.
    let manifest = schema(&flm(&dir, &["schema", "--of", "extension", document]));
    assert_eq!(manifest["properties"]["flm"]["properties"]["features"]["properties"], printed["properties"]["flm"]["properties"]["features"]["properties"]);

    // The preparation of the same document, over the same search path, resolves the same
    // packages.
    let mut request = flm_main::session::Request::new(flm_main::session::Input::File(PathBuf::from(document)));
    request.default_math = flm_cli::default_math();
    request.extension_folders =
        vec![flm_main::packages::ExtensionFolder { path: dir.join("_flmextensions"), required: false }];
    let compiled = vec![
        #[cfg(feature = "phfpack")]
        flm_phfpack::factory().unwrap(),
    ];
    let prepared =
        flm_main::session::prepare(request, std::sync::Arc::new(flm_main::files::DiskFiles), &compiled).expect("prepared");
    let refs: Vec<_> = prepared.factories().iter().collect();
    assert_eq!(flm_main::seed::front_matter_schema(&refs, prepared.suggestions()), printed);
}

/// With an extension installed in the user's folder, the front matter's schema suggests it
/// for `use`, and suggests `NAME` for `math` for an extension `math-NAME`; nothing
/// restricts the values.
#[test]
fn the_schema_suggests_the_extensions_installed() {
    let dir = folder("suggestions");
    write(&dir, ".flm/extensions/style.yaml", "");
    write(&dir, ".flm/extensions/math-fake/flmextension.yaml", "");
    let printed = schema(&flm(&dir, &["schema", "--of", "front-matter"]));
    let keys = &printed["properties"]["flm"]["properties"];
    let examples = |key: &str| keys[key]["anyOf"][0]["examples"].as_array().cloned().unwrap_or_default();
    assert!(examples("use").contains(&serde_json::json!("style")), "{}", keys["use"]);
    assert!(examples("math").contains(&serde_json::json!("fake")), "{}", keys["math"]);
    for key in ["use", "initial_use", "math"] {
        assert!(!keys[key].to_string().contains("\"enum\""), "{}", keys[key]);
    }
}

#[test]
fn a_document_with_an_unknown_entry_gives_status_2() {
    let dir = folder("unknown-entry");
    write(&dir, "paper.flm", "---\nflm:\n  use: [nosuch]\n---\nHello.\n");
    let output = flm(&dir, &["schema", "--of", "front-matter", "paper.flm"]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert!(output.stdout.is_empty());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("paper.flm:3:") && stderr.contains("`nosuch` must name a package compiled into the program"), "{stderr}");
    // A folder named with `--extensions-dir` must exist, as for `flm render`.
    let missing = flm(&dir, &["schema", "--extensions-dir", "nosuch", "paper.flm"]);
    assert_eq!(missing.status.code(), Some(2), "{missing:?}");
    assert!(String::from_utf8_lossy(&missing.stderr).contains("the folder `nosuch` of the search path must exist"), "{missing:?}");
    // The option names where a document's extensions are found: it requires a document.
    let alone = flm(&dir, &["schema", "--extensions-dir", "."]);
    assert_eq!(alone.status.code(), Some(2), "{alone:?}");
}
