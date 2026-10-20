//! The schema of the settings files: the schema of an extension's manifest and that of a
//! front matter, assembled from their owners' parts — each feature's from the declaration
//! of its settings — cover every key of every feature of `stdcore` and `std`, and refuse a
//! key no feature declares. That they accept the files the other tests write is checked
//! where each file is written, by the forms of `support/files.rs`. Every
//! option type a feature of those packages registers is read from settings, so no feature
//! takes a value that a settings file could not give it.

use std::path::PathBuf;
use std::sync::Arc;

use flm_core::model::options::OptionKey;
use flm_core::stdcore::features::meta::DocMeta;
use flm_main::files::MemoryFiles;
use flm_main::session::{Input, Request, prepare};
use serde_json::json;

#[path = "support/files.rs"]
mod support;
use support::{Written, document_accepted, extension, extension_accepted, factories, front_matter};

// --- test 4: every key, with its description, its moment and its mark --------------------

#[test]
fn every_key_of_every_feature_stands_in_both_schemas_with_its_description_moment_and_mark() {
    for schema in [extension(), front_matter()] {
        let defs = &schema["$defs"];
        let mut features = 0;
        for factory in factories() {
            for feature in factory.features() {
                let Some(decl) = &feature.settings else { continue };
                features += 1;
                let part = &defs[feature.name.as_str()];
                assert!(part["description"].as_str().is_some_and(|text| !text.is_empty()), "{}", feature.name);
                for key in decl.keys() {
                    let property = &part["properties"][key.name.as_str()];
                    let at = format!("{}.{}", feature.name, key.name);
                    assert!(property["description"].as_str().is_some_and(|text| !text.is_empty()), "{at}");
                    assert_eq!(property["x-flm-bind"], json!(key.bind.label()), "{at}");
                    let merged = part["x-flm-marks"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .any(|mark| mark["path"] == json!(key.name.as_str()) && mark["merge_entries"] == json!(true));
                    assert_eq!(merged, decl.mark_at(&key.name).is_some_and(|mark| mark.merge_entries), "{at}");
                }
            }
        }
        // The seventeen features of `stdcore` that take settings, its five backends,
        // `std`'s `compose` and `write`, `std`'s `graphics-files` under the cargo feature
        // `graphics`, and `std`'s `citations-standard` under the cargo feature `citations`.
        assert_eq!(
            features,
            24 + usize::from(cfg!(feature = "graphics")) + usize::from(cfg!(feature = "citations"))
        );
        assert!(defs["floats"]["properties"]["types"]["default"]["figure"].is_object());
    }
}

#[test]
fn every_option_type_a_feature_registers_is_read_from_its_settings() {
    // The registry of a document holds every option type the features of `stdcore` and
    // `std` declare. Each one but the document's metadata, which the front matter's root
    // gives, is one its feature's declaration reads: none is left that a settings file
    // could not set.
    let files = MemoryFiles::new().document("paper.flm", "Text.\n");
    let mut request = Request::new(Input::File(PathBuf::from("paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    let prepared = prepare(request, Arc::new(files), &[]).unwrap();
    let factories = factories();
    let declared: Vec<(String, OptionKey)> = factories
        .iter()
        .flat_map(|factory| factory.features())
        .filter_map(|feature| Some((feature.name.to_string(), feature.settings.as_ref()?)))
        .flat_map(|(name, decl)| decl.option_types().map(move |(key, _)| (name.clone(), key)))
        .collect();
    let mut checked = 0;
    for (key, decl) in prepared.registry().options() {
        if *key == OptionKey::of::<DocMeta>() {
            continue;
        }
        checked += 1;
        assert!(
            declared.iter().any(|(owner, declared)| owner == decl.owner.as_str() && declared == key),
            "the option type {key:?} of `{}` is not read from its settings",
            decl.owner
        );
    }
    assert!(checked >= 20, "{checked}");
}

// --- test 5: a key no feature declares refused (the files the tests write are accepted
// where each is written: `support/files.rs`) ---------------------------------------------

#[test]
fn the_schemas_refuse_a_key_no_feature_declares() {
    for block in [
        "features:\n  text:\n    widht: 20\n",
        "features:\n  nosuch: {}\n",
        "layout: blocks\n",
        "workflows:\n  text:\n    settings:\n      text: false\n",
        "workflows:\n  text:\n    colour: red\n",
        "parsing: {comment: false}\n",
        "features:\n  floats:\n    types:\n      algorithm: {caption_name: Algorithm}\n",
    ] {
        let text = format!("flm:\n{}", block.lines().map(|line| format!("  {line}\n")).collect::<String>());
        assert!(!extension_accepted(&text), "the manifest schema accepts:\n{text}");
    }
    for text in ["docs: A package.\n", "extension: {engine: python}\n", "extension: {docs: A package.}\n"] {
        assert!(!extension_accepted(text), "the manifest schema accepts:\n{text}");
    }
    for text in [
        "---\nflm:\n  features:\n    text:\n      widht: 20\n---\n",
        "---\nflm:\n  workflows: {}\n---\n",
        "---\nflm:\n  layout: sideways\n---\n",
        "---\ntitle: [A, B]\n---\n",
    ] {
        assert!(!document_accepted(text), "the front matter schema accepts:\n{text}");
    }
}

// --- the keys of the engines `js` and `wasm`, with their limits -----------------------------

#[test]
fn the_manifest_schema_states_the_engines_and_their_limits() {
    for text in [
        "extension:\n  engine: wasm\n  wasm:\n    module: ./wordcount.wasm\n",
        "extension:\n  engine: wasm\n  wasm:\n    module: wordcount.wasm\n    limits: {memory_mib: 128, fuel: 5000000000}\n",
        "extension:\n  engine: js\n  js:\n    main: main.js\n    limits: {memory_mib: 256, stack_mib: 16, time_s: 60}\n",
    ] {
        assert!(extension_accepted(text), "the manifest schema refuses:\n{text}");
    }
    let too_deep = flm_ext::manifest::MAX_STACK_MIB;
    for text in [
        "extension: {engine: wasm, wasm: {main: a.wasm}}\n".to_string(),
        "extension: {engine: wasm, wasm: {module: a.wasm, limits: {fuel: 0}}}\n".to_string(),
        "extension: {engine: wasm, wasm: {module: a.wasm, limits: {stack_mib: 1}}}\n".to_string(),
        "extension: {engine: js, js: {main: main.js, limits: {fuel: 1}}}\n".to_string(),
        "extension: {engine: js, js: {main: main.js, limits: {time_s: 0}}}\n".to_string(),
        format!("extension: {{engine: js, js: {{main: main.js, limits: {{stack_mib: {too_deep}}}}}}}\n"),
    ] {
        assert!(!extension_accepted(&text), "the manifest schema accepts:\n{text}");
    }
}

#[test]
fn a_schema_is_a_complete_document_its_references_resolved() {
    for schema in [extension(), front_matter()] {
        assert_eq!(schema["$schema"], json!("https://json-schema.org/draft/2020-12/schema"));
        let text = schema.to_string();
        let mut rest = text.as_str();
        while let Some(at) = rest.find("\"$ref\":\"#/$defs/") {
            let start = at + "\"$ref\":\"#/$defs/".len();
            let end = start + rest[start..].find('"').unwrap();
            let name = rest[start..end].replace("~1", "/").replace("~0", "~");
            assert!(schema["$defs"].get(&name).is_some(), "no definition `{name}`");
            rest = &rest[end..];
        }
    }
}
