//! The layer over the host library (cargo feature `flm-main`): the environment of a
//! document prepared in memory, which uses a data extension found in the folder
//! `_flmextensions` beside it.

#![cfg(feature = "flm-main")]

use std::path::PathBuf;
use std::sync::Arc;

use flm_info::Environment;
use flm_info::host::{ExtensionOrigin, PackageOrigin};
use flm_main::files::MemoryFiles;
use flm_main::packages::ExtensionFolder;
use flm_main::seed::MathChoice;
use flm_main::session::{Input, Request, prepare};

/// A data extension that sets the text backend's width; its manifest writes a description
/// and an author.
const NARROW: &str =
    "extension:\n  description: Narrow text.\n  author: Ada Lovelace\nflm:\n  features:\n    text:\n      width: 20\n";

/// The document: it uses the extension and writes the width again, at line 6.
const DOCUMENT: &str = "---\nflm:\n  use: [narrow]\n  features:\n    text:\n      width: 30\n  layout: blocks\n---\nHello.\n";

fn files() -> MemoryFiles {
    let mut files = MemoryFiles::new();
    files.insert("doc/paper.flm", DOCUMENT);
    files.insert("doc/_flmextensions/narrow.yaml", NARROW);
    files
}

fn request() -> Request {
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = MathChoice::processor("passthrough");
    request.profile = Some(String::from("text"));
    request.extension_folders = vec![ExtensionFolder { path: PathBuf::from("doc/_flmextensions"), required: false }];
    request
}

fn environment() -> Environment {
    let prepared = prepare(request(), Arc::new(files()), &[]).expect("the document is prepared");
    Environment::of_prepared(&prepared)
}

#[test]
fn the_packages_say_where_they_come_from() {
    let env = environment();
    let packages = env.packages();
    let origin = |name: &str| packages.iter().find(|package| package.name == name).and_then(|package| package.origin.clone());
    assert_eq!(origin("std"), Some(PackageOrigin::Compiled));
    assert_eq!(origin("math-passthrough"), Some(PackageOrigin::Compiled));
    // The package of the extension, with its manifest's description, author and engine.
    let narrow = ExtensionOrigin {
        identity: String::from("doc/_flmextensions/narrow.yaml"),
        package: String::from("_"),
        description: Some(String::from("Narrow text.")),
        author: Some(String::from("Ada Lovelace")),
        engine: Some(String::from("data")),
    };
    assert_eq!(origin("doc/_flmextensions/narrow.yaml#_"), Some(PackageOrigin::Extension(narrow)), "{packages:?}");
    let json = serde_json::to_value(origin("doc/_flmextensions/narrow.yaml#_")).expect("JSON");
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "extension",
            "identity": "doc/_flmextensions/narrow.yaml",
            "package": "_",
            "description": "Narrow text.",
            "author": "Ada Lovelace",
            "engine": "data"
        })
    );
    let host = env.host().expect("the host's part");
    assert_eq!(host.packages.len(), packages.len());
    assert_eq!(host.math.as_ref().map(|math| math.package.as_str()), Some("math-passthrough"));
    assert_eq!((host.profile.name.as_str(), host.profile.workflow.as_str()), ("text", "text"));
    assert_eq!(host.profile.package, None);
    // An environment made without the host has no origin.
    let bare = Environment::new(Arc::clone(env.registry()), None, env.factories().to_vec());
    assert!(bare.packages().iter().all(|package| package.origin.is_none()));
    assert!(bare.host().is_none());
}

#[test]
fn a_setting_written_in_the_front_matter_is_in_effect_with_its_place() {
    let env = environment();
    let settings = env.settings("text").expect("the text feature takes settings");
    let width = settings.keys.iter().find(|key| key.name == "width").expect("the key `width`");
    let in_effect = width.in_effect.as_ref().expect("the width is written");
    assert_eq!(in_effect.value, serde_json::json!(30));
    assert_eq!(in_effect.origin.source, "front-matter");
    assert_eq!(in_effect.origin.file.as_deref(), Some("doc/paper.flm"));
    assert_eq!(in_effect.origin.line, Some(6));
    // A key no source wrote has no value in effect.
    assert!(settings.keys.iter().filter(|key| key.name != "width").all(|key| key.in_effect.is_none()));
    let read = &env.host().expect("the host's part").front_matter;
    assert_eq!(read.uses.iter().map(|used| used.value.as_str()).collect::<Vec<_>>(), ["narrow"]);
    assert_eq!(read.uses[0].line, 3);
    assert_eq!(read.layout.as_ref().map(|layout| layout.value.as_str()), Some("blocks"));
    assert!(read.text.as_deref().is_some_and(|text| text.contains("width: 30")));
}

#[test]
fn the_extensions_of_the_folder_are_listed() {
    let env = environment();
    let host = env.host().expect("the host's part");
    assert!(host.suggestions.iter().any(|suggestion| suggestion == "narrow"), "{:?}", host.suggestions);
    assert_eq!(host.search_path.len(), 1);
    assert_eq!(host.search_path[0].path, "doc/_flmextensions");
}

#[test]
fn a_request_without_a_document_has_the_environment_of_its_defaults() {
    let env = Environment::of_request(request(), Arc::new(files()), &[]).expect("prepared");
    let host = env.host().expect("the host's part");
    assert!(host.front_matter.text.is_none());
    assert!(host.packages.iter().all(|package| package.origin == PackageOrigin::Compiled), "{:?}", host.packages);
    assert!(host.suggestions.iter().any(|suggestion| suggestion == "narrow"));
}

#[test]
fn the_front_matter_keys_are_described() {
    let env = environment();
    let front_matter = env.front_matter();
    let names: Vec<&str> = front_matter.keys.iter().map(|key| key.name.as_str()).collect();
    for name in ["use", "initial_use", "math", "features", "parsing", "feature_order", "layout", "default_profile"] {
        assert!(names.contains(&name), "{name}: {names:?}");
    }
    assert!(front_matter.keys.iter().all(|key| key.description.is_some()));
    let schema = flm_info::describe::schema();
    assert!(schema["$defs"]["HostInfo"].is_object());
    serde_json::to_value(env.host()).expect("the host's part is JSON");
}
