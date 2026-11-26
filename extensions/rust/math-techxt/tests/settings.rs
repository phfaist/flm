//! The settings of `math-techxt`: read when a document's settings are read and
//! when the registry is built, and an input of the registry digest.

mod common;

use serde_json::json;

use common::{FEATURE, factory, registry_with};

/// The problems of reading the settings `data` with the feature's declaration,
/// each its path and its message.
fn read_problems(data: serde_json::Value) -> Vec<(String, String)> {
    let factory = factory();
    let decl = factory.settings(FEATURE).expect("the feature declares settings");
    match decl.read(data.as_object().expect("the settings are a map")) {
        Ok(_) => panic!("the settings read"),
        Err(errors) => errors.into_iter().map(|error| (error.path, error.detail)).collect(),
    }
}

#[test]
fn an_empty_font_address_is_refused_where_it_is_written() {
    let problems = read_problems(json!({ "font_url": "" }));
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].0.ends_with("font_url"), "{problems:?}");
}

#[test]
fn an_unknown_setting_is_refused() {
    let problems = read_problems(json!({ "mode": "mathml" }));
    assert!(!problems.is_empty());
}

#[test]
fn the_registry_digest_differs_when_the_font_address_differs() {
    let default = registry_with(json!({}));
    let other = registry_with(json!({ "font_url": "fonts/mono.woff2" }));
    assert_ne!(default.digest(), other.digest());
    assert_eq!(default.digest(), registry_with(json!({})).digest());
}
