//! The `flm` block step: a parsed block (a document's front matter, or a
//! data extension's `flm:`) read into a package's contributions — the `use`
//! list, the settings per feature, the parsing settings, the workflows, the
//! chains of the feature order — so that a document and a data extension are
//! read by one rule.
//!
//! Turning text into a value is the host's; this module reads the value. The
//! errors carry the key's path (`use[2]`, `features.text`,
//! `workflows.draft.settings.html`), no place: the host, which holds its tree
//! with places, finds the place of a path itself. A chain of the feature
//! order keeps its key path likewise ([`ChainBlock`]), and the caller gives
//! it the origin that a message about the feature order quotes.

use std::fmt;

use flm_core::core::registry::FeatureChain;
use flm_core::lang::FlmParsing;
use serde_json::{Map, Value};

use crate::entry::UseEntry;

/// The keys of a block the block step reads ([`read`]), in order: every
/// other key is the caller's to accept or refuse ([`Block::other`]).
pub(crate) const BLOCK_KEYS: [&str; 5] = ["use", "features", "parsing", "workflows", "feature_order"];

/// The keys of `parsing`.
const PARSING_KEYS: [&str; 3] = ["comments", "comment_prefix", "dollar_math"];

/// A block read: its contributions, and every key it does not read, in order.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Block {
    /// `use`: one entry or a list, each parsed by [`UseEntry::parse`].
    pub uses: Vec<UseEntry>,
    /// `features`: each feature's name with its settings map, or `false`.
    pub features: Vec<(String, FeatureBlock)>,
    /// `parsing`: the keys written, checked as
    /// [`FlmParsing`] reads them; `None` when not written.
    pub parsing: Option<Map<String, Value>>,
    /// `workflows`: each workflow's fields and settings.
    pub workflows: Vec<WorkflowBlock>,
    /// `feature_order`: one chain or a list of chains, each read by
    /// [`FeatureChain::parse`], in the order written.
    pub feature_order: Vec<ChainBlock>,
    /// Every other key, in order, for the caller to accept or refuse (a
    /// document's `layout` and `default_profile`).
    pub other: Vec<(String, Value)>,
}

/// What a block writes for one feature.
#[derive(Debug, Clone, PartialEq)]
pub enum FeatureBlock {
    /// `false`: the feature is switched off.
    Off,
    /// The feature's settings (an empty map for an entry with no value): the
    /// feature is switched on.
    Settings(Map<String, Value>),
}

/// One workflow a block declares.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkflowBlock {
    /// The workflow's name.
    pub name: String,
    /// Its fields other than `settings`, in order: `extends`, `docs` (texts),
    /// `outputs`, `demands`, `prefer` (lists of names).
    pub fields: Vec<(String, Value)>,
    /// Its `settings`: each feature's name with its settings map.
    pub settings: Vec<(String, Map<String, Value>)>,
}

/// One chain a block writes under `feature_order`: features named in the
/// order they must take in the registry (`math-mathjax < phfqit`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainBlock {
    /// The key's path: `feature_order` for a chain written alone,
    /// `feature_order[1]` for an item of a list.
    pub path: String,
    /// The chain, read by [`FeatureChain::parse`], whose origin is the key's
    /// path.
    pub chain: FeatureChain,
}

impl ChainBlock {
    /// Returns the chain with the origin `origin` in place of the key's path:
    /// the caller's text naming where the chain is written (`package
    /// math-mathjax`, `front matter of doc.flm, line 4`), which a message about
    /// the feature order quotes.
    pub fn chain_with_origin(&self, origin: impl Into<String>) -> FeatureChain {
        self.chain.clone().with_origin(origin)
    }
}

/// A problem in a block: the key's path and the condition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockError {
    /// The key's path: `use[2]`, `features.text`, `parsing.dollar_math`,
    /// `workflows.draft.settings.html`; empty for the block itself.
    pub path: String,
    /// The condition, as text for a person.
    pub detail: String,
}

impl fmt::Display for BlockError {
    /// The detail.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for BlockError {}

fn error(path: impl Into<String>, detail: impl Into<String>) -> BlockError {
    BlockError { path: path.into(), detail: detail.into() }
}

/// Reads the block `value`. A null value is an empty block.
///
/// # Errors
///
/// Every problem found: a block that is not a map; a `use` that is neither
/// an entry nor a list of entries, an item that is not a text or breaks the
/// entry grammar; a `features` that is not a map, a feature's value that is
/// neither a map, nor empty, nor `false`; a `parsing` that is not a map, holds
/// another key than `comments`, `comment_prefix` and `dollar_math`, or a value
/// of the wrong type; a `workflows` that is not a map, a workflow that is not
/// a map, an unknown field or a field of the wrong shape, a workflow's
/// settings switching a feature off; a `feature_order` that is neither a
/// chain nor a list of chains, an item that is not a text or is not a chain
/// ([`FeatureChain::parse`]: the form `NAME < NAME < … < NAME`, each name once).
pub fn read(value: &Value) -> Result<Block, Vec<BlockError>> {
    let mut block = Block::default();
    let map = match value {
        Value::Null => return Ok(block),
        Value::Object(map) => map,
        _ => return Err(vec![error("", "the block must be a map")]),
    };
    let mut errors = Vec::new();
    for (key, value) in map {
        match BLOCK_KEYS.iter().position(|known| known == key) {
            Some(0) => block.uses = read_use(value, &mut errors),
            Some(1) => block.features = read_features("features", value, &mut errors),
            Some(2) => block.parsing = read_parsing(value, &mut errors),
            Some(3) => block.workflows = read_workflows(value, &mut errors),
            Some(4) => block.feature_order = read_feature_order(value, &mut errors),
            _ => block.other.push((key.clone(), value.clone())),
        }
    }
    if errors.is_empty() { Ok(block) } else { Err(errors) }
}

/// `use`: an entry, a list of entries, or null.
fn read_use(value: &Value, errors: &mut Vec<BlockError>) -> Vec<UseEntry> {
    let items: Vec<(String, &Value)> = match value {
        Value::String(_) => vec![("use".to_string(), value)],
        Value::Array(items) => items.iter().enumerate().map(|(index, item)| (format!("use[{index}]"), item)).collect(),
        Value::Null => Vec::new(),
        _ => {
            errors.push(error("use", "`use` must be an entry or a list of entries"));
            return Vec::new();
        }
    };
    let mut uses = Vec::new();
    for (path, item) in items {
        match item {
            Value::String(text) => match UseEntry::parse(text) {
                Ok(entry) => uses.push(entry),
                Err(entry_error) => errors.push(error(path, entry_error.detail)),
            },
            _ => errors.push(error(&path, format!("`{path}` must be an entry, written as a text"))),
        }
    }
    uses
}

/// `feature_order`: a chain, a list of chains, or null; each chain a text
/// read by [`FeatureChain::parse`] with its key path as its origin.
fn read_feature_order(value: &Value, errors: &mut Vec<BlockError>) -> Vec<ChainBlock> {
    let items: Vec<(String, &Value)> = match value {
        Value::String(_) => vec![("feature_order".to_string(), value)],
        Value::Array(items) => {
            items.iter().enumerate().map(|(index, item)| (format!("feature_order[{index}]"), item)).collect()
        }
        Value::Null => Vec::new(),
        _ => {
            errors.push(error("feature_order", "`feature_order` must be a chain or a list of chains"));
            return Vec::new();
        }
    };
    let mut chains = Vec::new();
    for (path, item) in items {
        match item {
            Value::String(text) => match FeatureChain::parse(text, path.as_str()) {
                Ok(chain) => chains.push(ChainBlock { path, chain }),
                Err(chain_error) => errors.push(error(path, chain_error.to_string())),
            },
            _ => errors.push(error(&path, format!("`{path}` must be a chain, written as a text"))),
        }
    }
    chains
}

/// A map from a feature's name to its settings, at `path` (`features`, or a
/// workflow's `settings`): null reads as nothing, an entry with no value as
/// an empty map.
fn read_features(path: &str, value: &Value, errors: &mut Vec<BlockError>) -> Vec<(String, FeatureBlock)> {
    let entries = match value {
        Value::Null => return Vec::new(),
        Value::Object(entries) => entries,
        _ => {
            errors.push(error(path, format!("`{path}` must be a map from a feature's name to its settings")));
            return Vec::new();
        }
    };
    let mut features = Vec::new();
    for (name, value) in entries {
        match value {
            Value::Object(map) => features.push((name.clone(), FeatureBlock::Settings(map.clone()))),
            Value::Null => features.push((name.clone(), FeatureBlock::Settings(Map::new()))),
            Value::Bool(false) => features.push((name.clone(), FeatureBlock::Off)),
            _ => errors.push(error(
                format!("{path}.{name}"),
                format!("the settings of the feature `{name}` (`{path}.{name}`) must be a map, or `false` to switch it off"),
            )),
        }
    }
    features
}

/// `parsing`: a map of the three keys, each of its type.
fn read_parsing(value: &Value, errors: &mut Vec<BlockError>) -> Option<Map<String, Value>> {
    let Value::Object(map) = value else {
        errors.push(error("parsing", "`parsing` must be a map"));
        return None;
    };
    let mut known = true;
    for key in map.keys().filter(|key| !PARSING_KEYS.contains(&key.as_str())) {
        known = false;
        errors.push(error(
            format!("parsing.{key}"),
            format!("`parsing.{key}` must be one of `comments`, `comment_prefix` and `dollar_math`"),
        ));
    }
    if !known {
        return None;
    }
    match serde_json::from_value::<FlmParsing>(value.clone()) {
        Ok(_) => Some(map.clone()),
        Err(detail) => {
            errors.push(error("parsing", format!("each setting under `parsing` must hold a value of its type: {detail}")));
            None
        }
    }
}

/// `workflows`: a map from a workflow's name to a map of its fields.
fn read_workflows(value: &Value, errors: &mut Vec<BlockError>) -> Vec<WorkflowBlock> {
    let Value::Object(entries) = value else {
        errors.push(error("workflows", "`workflows` must be a map from a workflow's name to its fields"));
        return Vec::new();
    };
    let mut workflows = Vec::new();
    for (name, fields) in entries {
        let Value::Object(fields) = fields else {
            errors.push(error(
                format!("workflows.{name}"),
                format!("the workflow `{name}` (`workflows.{name}`) must be a map of its fields"),
            ));
            continue;
        };
        let mut workflow = WorkflowBlock { name: name.clone(), fields: Vec::new(), settings: Vec::new() };
        for (field, value) in fields {
            let path = format!("workflows.{name}.{field}");
            let names = || matches!(value, Value::Array(items) if items.iter().all(Value::is_string));
            let shape_ok = match field.as_str() {
                "extends" | "docs" => value.is_string(),
                "outputs" | "demands" | "prefer" => names(),
                "settings" => {
                    for (feature, entry) in read_features(&path, value, errors) {
                        match entry {
                            FeatureBlock::Settings(map) => workflow.settings.push((feature, map)),
                            FeatureBlock::Off => errors.push(error(
                                format!("{path}.{feature}"),
                                format!("the settings of the workflow `{name}` must not switch the feature `{feature}` off"),
                            )),
                        }
                    }
                    continue;
                }
                _ => {
                    errors.push(error(
                        path,
                        format!(
                            "`{field}`, in the workflow `{name}`, must be one of the fields of a workflow: `extends`, \
                             `outputs`, `demands`, `prefer`, `settings` and `docs`"
                        ),
                    ));
                    continue;
                }
            };
            if shape_ok {
                workflow.fields.push((field.clone(), value.clone()));
            } else {
                let expected = if matches!(field.as_str(), "extends" | "docs") { "a text" } else { "a list of names" };
                errors.push(error(path, format!("the field `{field}` of the workflow `{name}` must be {expected}")));
            }
        }
        workflows.push(workflow);
    }
    workflows
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn problems(value: Value) -> Vec<(String, String)> {
        read(&value).expect_err("refused").into_iter().map(|e| (e.path, e.detail)).collect()
    }

    fn pair(path: &str, detail: &str) -> (String, String) {
        (path.to_string(), detail.to_string())
    }

    #[test]
    fn use_is_one_entry_a_list_or_null() {
        assert_eq!(read(&json!({ "use": "myext" })).unwrap().uses, [UseEntry::parse("myext").unwrap()]);
        assert_eq!(
            read(&json!({ "use": ["stdcore", "myext#core"] })).unwrap().uses,
            [UseEntry::parse("stdcore").unwrap(), UseEntry::parse("myext#core").unwrap()]
        );
        assert_eq!(read(&json!({ "use": null })).unwrap().uses, []);
        assert_eq!(read(&Value::Null).unwrap(), Block::default());
        assert_eq!(problems(json!({ "use": 3 })), [pair("use", "`use` must be an entry or a list of entries")]);
        assert_eq!(
            problems(json!({ "use": ["a", 2, "./b"] })),
            [
                pair("use[1]", "`use[1]` must be an entry, written as a text"),
                pair("use[2]", &UseEntry::parse("./b").unwrap_err().detail),
            ]
        );
    }

    #[test]
    fn features_are_maps_false_or_empty() {
        let block = read(&json!({ "features": { "text": { "width": 60 }, "toc": false, "compose": null } })).unwrap();
        assert_eq!(
            block.features,
            [
                ("text".to_string(), FeatureBlock::Settings(json!({ "width": 60 }).as_object().unwrap().clone())),
                ("toc".to_string(), FeatureBlock::Off),
                ("compose".to_string(), FeatureBlock::Settings(Map::new())),
            ]
        );
        assert_eq!(read(&json!({ "features": null })).unwrap().features, []);
        assert_eq!(
            problems(json!({ "features": [1] })),
            [pair("features", "`features` must be a map from a feature's name to its settings")]
        );
        assert_eq!(
            problems(json!({ "features": { "toc": true } })),
            [pair("features.toc", "the settings of the feature `toc` (`features.toc`) must be a map, or `false` to switch it off")]
        );
    }

    #[test]
    fn parsing_checks_its_three_keys_and_their_types() {
        let block = read(&json!({ "parsing": { "dollar_math": true } })).unwrap();
        assert_eq!(block.parsing, Some(json!({ "dollar_math": true }).as_object().unwrap().clone()));
        assert_eq!(problems(json!({ "parsing": 1 })), [pair("parsing", "`parsing` must be a map")]);
        assert_eq!(
            problems(json!({ "parsing": { "dollars": true } })),
            [pair("parsing.dollars", "`parsing.dollars` must be one of `comments`, `comment_prefix` and `dollar_math`")]
        );
        let found = problems(json!({ "parsing": { "dollar_math": "yes" } }));
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0, "parsing");
        assert!(found[0].1.starts_with("each setting under `parsing` must hold a value of its type: "), "{}", found[0].1);
    }

    #[test]
    fn workflows_check_their_fields_and_refuse_switching_a_feature_off() {
        let block = read(&json!({ "workflows": { "draft": {
            "extends": "html", "outputs": ["html"], "docs": "A draft.",
            "settings": { "html": { "x": 1 }, "toc": null },
        } } }))
        .unwrap();
        assert_eq!(
            block.workflows,
            [WorkflowBlock {
                name: "draft".to_string(),
                fields: vec![
                    ("extends".to_string(), json!("html")),
                    ("outputs".to_string(), json!(["html"])),
                    ("docs".to_string(), json!("A draft.")),
                ],
                settings: vec![
                    ("html".to_string(), json!({ "x": 1 }).as_object().unwrap().clone()),
                    ("toc".to_string(), Map::new()),
                ],
            }]
        );
        assert_eq!(
            problems(json!({ "workflows": { "draft": {
                "colour": 1, "extends": 2, "prefer": "x", "settings": { "toc": false },
            }, "bad": 3 } })),
            [
                pair(
                    "workflows.draft.colour",
                    "`colour`, in the workflow `draft`, must be one of the fields of a workflow: `extends`, `outputs`, \
                     `demands`, `prefer`, `settings` and `docs`"
                ),
                pair("workflows.draft.extends", "the field `extends` of the workflow `draft` must be a text"),
                pair("workflows.draft.prefer", "the field `prefer` of the workflow `draft` must be a list of names"),
                pair("workflows.draft.settings.toc", "the settings of the workflow `draft` must not switch the feature `toc` off"),
                pair("workflows.bad", "the workflow `bad` (`workflows.bad`) must be a map of its fields"),
            ]
        );
        assert_eq!(
            problems(json!({ "workflows": [] })),
            [pair("workflows", "`workflows` must be a map from a workflow's name to its fields")]
        );
    }

    #[test]
    fn feature_order_is_one_chain_a_list_or_null() {
        let chain = |text: &str, path: &str| ChainBlock {
            path: path.to_string(),
            chain: FeatureChain::parse(text, path).unwrap(),
        };
        assert_eq!(
            read(&json!({ "feature_order": "math-mathjax < phfqit" })).unwrap().feature_order,
            [chain("math-mathjax < phfqit", "feature_order")]
        );
        let block = read(&json!({ "feature_order": ["a<b", "headings  <  mysections < toc"] })).unwrap();
        assert_eq!(block.feature_order, [chain("a < b", "feature_order[0]"), chain("headings < mysections < toc", "feature_order[1]")]);
        assert_eq!(block.feature_order[1].chain.origin(), "feature_order[1]");
        assert_eq!(read(&json!({ "feature_order": null })).unwrap().feature_order, []);
        assert!(block.other.is_empty());
        // The caller gives each chain its origin.
        let given = block.feature_order[1].chain_with_origin("package phfpack");
        assert_eq!((given.to_string().as_str(), given.origin()), ("headings < mysections < toc", "package phfpack"));
        assert_eq!(given.names(), block.feature_order[1].chain.names());
    }

    #[test]
    fn feature_order_refuses_what_is_not_a_chain_each_at_its_key() {
        assert_eq!(
            problems(json!({ "feature_order": { "a": "b" } })),
            [pair("feature_order", "`feature_order` must be a chain or a list of chains")]
        );
        assert_eq!(
            problems(json!({ "feature_order": "a > b" })),
            [pair("feature_order", "the chain `a > b` must have the form `NAME < NAME < … < NAME`, with two or more feature names")]
        );
        assert_eq!(
            problems(json!({ "feature_order": ["a < b", 3, "a", "a < b < a"] })),
            [
                pair("feature_order[1]", "`feature_order[1]` must be a chain, written as a text"),
                pair("feature_order[2]", "the chain `a` must have the form `NAME < NAME < … < NAME`, with two or more feature names"),
                pair("feature_order[3]", "each feature must appear only once in the chain `a < b < a`, but `a` appears more than once"),
            ]
        );
    }

    #[test]
    fn a_workflow_does_not_take_feature_order() {
        assert_eq!(
            problems(json!({ "workflows": { "draft": { "feature_order": ["a < b"] } } })),
            [pair(
                "workflows.draft.feature_order",
                "`feature_order`, in the workflow `draft`, must be one of the fields of a workflow: `extends`, \
                 `outputs`, `demands`, `prefer`, `settings` and `docs`"
            )]
        );
    }

    #[test]
    fn other_keys_come_back_in_order() {
        let block = read(&json!({ "layout": "inline", "use": "x", "default_profile": "html", "docs": "d" })).unwrap();
        assert_eq!(
            block.other,
            [
                ("layout".to_string(), json!("inline")),
                ("default_profile".to_string(), json!("html")),
                ("docs".to_string(), json!("d")),
            ]
        );
    }

    #[test]
    fn each_error_carries_its_key_path() {
        let paths: Vec<String> = problems(json!({
            "use": ["ok", 1],
            "features": { "text": 3 },
            "parsing": { "nope": 1 },
            "workflows": { "draft": { "settings": { "html": 2 } } },
        }))
        .into_iter()
        .map(|(path, _)| path)
        .collect();
        assert_eq!(paths, ["use[1]", "features.text", "parsing.nope", "workflows.draft.settings.html"]);
        assert_eq!(
            problems(json!({ "workflows": { "draft": { "settings": { "html": 2 } } } }))[0].1,
            "the settings of the feature `html` (`workflows.draft.settings.html`) must be a map, or `false` to switch it off"
        );
        assert_eq!(problems(json!("x")), [pair("", "the block must be a map")]);
    }
}
