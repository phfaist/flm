//! Editing a document's front matter in place: the settings that the live preview's
//! *Settings* tool saves are written into the document's file, and every other byte stays
//! as the author wrote it.
//!
//! A document's [front matter](crate::seed) is the block of YAML between a first line `---`
//! and the next `---` line of its file ([`crate::seed::split`]); its key `flm.features`
//! holds, feature by feature, the settings the document gives. [`edit_features`] returns
//! the file's text with that key holding a new map, and changes nothing else: the
//! comments, the blank lines, the quoting, the style (block or flow) of each map and list,
//! the two `---` lines and the content below them stay as they were. It compares what the
//! front matter holds under `flm.features` now, as data, with the new map, and rewrites
//! only the values that differ, in place, through a YAML editor that keeps every byte it
//! is not asked to change (the crate `yaml_edit`).
//! [`DocumentSettings::save`](super::settings::DocumentSettings::save) calls it.

use serde_json::{Map, Value};
use yaml_edit::{AsYaml, Mapping, ScalarValue, Sequence, YamlFile, YamlNode};

use crate::seed::{RESERVED_KEY, SeedError, read_front_matter, split};

/// Returns the text of the file `text` with its front matter's `flm.features` holding
/// `features`, every other byte as it was.
///
/// The value that the front matter holds under `flm.features` is compared with
/// `features`, as data, and the text is changed only where the two differ: a key that
/// `features` does not hold is removed, with its line; a key that `features` adds is
/// written at the end of its map, laid out as its siblings are; two maps are compared key
/// by key and two lists entry by entry; any other value that differs is written in place
/// of the old one. The YAML editor decides the layout of what it writes: a new line is
/// indented as its siblings and ends with a line feed (also in a file whose lines end
/// with a carriage return and a line feed), and a text that would otherwise read as
/// another type (`'true'`, `'1'`), or a text written into a map in flow style
/// (`{a: "x"}`), is quoted. The keys `flm` and `features` are added when the front matter
/// lacks them; a file with no front matter receives one, between two `---` lines, above
/// its content.
///
/// The front matter's text ends with a line end, so that the closing `---` is on a line of
/// its own: where the edited text lacks one (for example because the removal of its last
/// line also removed that line's end), a line feed is added. The whole new file is read
/// back before it is returned, as the program reads a file ([`split`], then
/// [`read_front_matter`]): its front matter's data must equal the old front matter's data
/// with `flm.features` replaced by `features`, and its content must equal the old content.
/// When the YAML editor cannot write the change exactly, the edit is refused.
///
/// # Errors
///
/// Returns a [`SeedError`] when the front matter cannot be read ([`split`]'s and
/// [`read_front_matter`]'s errors), when its root is not a map, when `flm` holds a value
/// that is not a map, and when the edit is refused.
///
/// # Examples
///
/// ```
/// use flm_main::watch::front_matter::edit_features;
/// use serde_json::json;
///
/// let text = "---\ntitle: Notes  # for now\nflm:\n  features:\n    theorems: false\n---\nHello.\n";
/// let features = json!({ "theorems": false, "lists": false }).as_object().cloned().unwrap();
/// assert_eq!(
///     edit_features(text, &features).unwrap(),
///     "---\ntitle: Notes  # for now\nflm:\n  features:\n    theorems: false\n    lists: false\n---\nHello.\n"
/// );
/// ```
pub fn edit_features(text: &str, features: &Map<String, Value>) -> Result<String, SeedError> {
    let parts = split(text)?;
    let yaml = parts.front_matter.unwrap_or("");
    let root = read_front_matter(yaml, parts.front_matter_line)?;
    let before = match root.to_json() {
        Value::Null => Map::new(),
        Value::Object(before) => before,
        _ => return Err(SeedError::at(root.place, "the front matter must be a map from keys to values")),
    };
    let mut intended = before.clone();
    let flm = intended.entry(RESERVED_KEY).or_insert(Value::Null);
    if flm.is_null() {
        *flm = Value::Object(Map::new());
    }
    let Value::Object(settings) = flm else {
        let place = root.get(RESERVED_KEY).map(|flm| flm.place);
        return Err(SeedError { place, detail: format!("the value of `{RESERVED_KEY}` must be a map") });
    };
    settings.insert("features".into(), Value::Object(features.clone()));

    let file: YamlFile = yaml.parse().map_err(|_| refused())?;
    let Some(mapping) = file.ensure_document().as_mapping() else {
        return Err(refused());
    };
    edit_map(&mapping, &before, &intended);
    let mut edited = file.to_string();
    // The front matter's text stands on lines of its own between the two fences.
    if !edited.is_empty() && !edited.ends_with('\n') {
        edited.push('\n');
    }
    let (opening, closing) = match parts.front_matter {
        Some(_) => (parts.opening, parts.closing),
        None => ("---\n", "---\n"),
    };
    let written = [opening, &edited, closing, parts.content].concat();
    // The file read back before it is given. See decision rationale at [§flm-dd-dr:host-watch-settings].
    let intended = Value::Object(intended);
    let reads_as_intended = split(&written).is_ok_and(|new| {
        new.content == parts.content
            && new
                .front_matter
                .and_then(|yaml| read_front_matter(yaml, new.front_matter_line).ok())
                .is_some_and(|read_back| read_back.to_json() == intended)
    });
    if !reads_as_intended {
        return Err(refused());
    }
    Ok(written)
}

/// The refusal of an edit the YAML editor cannot write exactly.
fn refused() -> SeedError {
    SeedError {
        place: None,
        detail: String::from(
            "the settings cannot be written into the front matter exactly as it is laid out; change them in the file itself",
        ),
    }
}

/// Changes `mapping`, which holds `old` as data, so that it holds `new`.
///
/// The keys removed go last, so that a map keeps its style while it is edited: a block map
/// whose last key is removed becomes `{}`, in flow style, and what is written into it after
/// that would be written in flow style too.
fn edit_map(mapping: &Mapping, old: &Map<String, Value>, new: &Map<String, Value>) {
    for (key, value) in new {
        let key = key.as_str();
        match old.get(key) {
            Some(was) if was == value => {}
            Some(was) if walk_into(mapping.get(key).as_ref(), was, value) => {}
            _ => {
                mapping.set(key, &*first_written(value, mapping.is_flow_style()));
                fill(mapping.get(key).as_ref(), value);
            }
        }
    }
    for key in old.keys().filter(|key| !new.contains_key(*key)) {
        mapping.remove(key.as_str());
    }
}

/// Changes `sequence`, which holds `old` as data, so that it holds `new`, entry by entry.
fn edit_sequence(sequence: &Sequence, old: &[Value], new: &[Value]) {
    for index in (new.len()..old.len()).rev() {
        sequence.remove(index);
    }
    for (index, (was, value)) in old.iter().zip(new).enumerate() {
        if was != value && !walk_into(sequence.get(index).as_ref(), was, value) {
            sequence.set(index, &*first_written(value, sequence.is_flow_style()));
            fill(sequence.get(index).as_ref(), value);
        }
    }
    for value in new.iter().skip(old.len()) {
        sequence.push(&*first_written(value, sequence.is_flow_style()));
        fill(sequence.last().as_ref(), value);
    }
}

/// Changes `node`, which holds `old` as data, so that it holds `new`, when both are maps or
/// both are lists; says whether it did.
fn walk_into(node: Option<&YamlNode>, old: &Value, new: &Value) -> bool {
    match (old, new, node) {
        (Value::Object(old), Value::Object(new), Some(YamlNode::Mapping(mapping))) => edit_map(mapping, old, new),
        (Value::Array(old), Value::Array(new), Some(YamlNode::Sequence(sequence))) => edit_sequence(sequence, old, new),
        _ => return false,
    }
    true
}

/// Writes the entries of `value` into `node`, the empty map or list [`first_written`] gave for it;
/// a value that is neither is written whole by [`first_written`].
///
/// A map or a list is written empty first and filled in place, so that the editor lays out
/// its entries as it lays out any entry of the text.
fn fill(node: Option<&YamlNode>, value: &Value) {
    match (value, node) {
        (Value::Object(entries), Some(YamlNode::Mapping(mapping))) => {
            for (key, value) in entries {
                mapping.set(key.as_str(), &*first_written(value, mapping.is_flow_style()));
                fill(mapping.get(key.as_str()).as_ref(), value);
            }
        }
        (Value::Array(items), Some(YamlNode::Sequence(sequence))) => {
            for value in items {
                sequence.push(&*first_written(value, sequence.is_flow_style()));
                fill(sequence.last().as_ref(), value);
            }
        }
        _ => {}
    }
}

/// What is first written for `value` into a map or a list, in flow style when `flow`: the
/// value itself when it is neither a map nor a list, else an empty map or list in the
/// style of the map or list it is written into, which [`fill`] then fills.
fn first_written(value: &Value, flow: bool) -> Box<dyn AsYaml> {
    match value {
        Value::Object(_) if flow => Box::new(Mapping::new_flow()),
        Value::Object(_) => Box::new(Mapping::new_pending_block()),
        Value::Array(_) if flow => Box::new(Sequence::new_flow()),
        Value::Array(_) => Box::new(Sequence::new_pending_block()),
        Value::Null => Box::new(ScalarValue::null()),
        Value::Bool(value) => Box::new(ScalarValue::from(*value)),
        // A number that is an integer is written as one, any other with a fraction.
        Value::Number(number) => Box::new(match number.as_i64() {
            Some(integer) => ScalarValue::from(integer),
            None => ScalarValue::from(number.as_f64().unwrap_or_default()),
        }),
        Value::String(text) => Box::new(ScalarValue::string(text.as_str())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seed::written_features;
    use serde_json::json;

    /// The front matter of the tests of the module `seed`, between its fences.
    const FILE: &str = "---\ntitle: A \\emph{title}\nauthor: [A, B]\nflm:\n  use: [std]\n  \
                        features:\n    html: {heading_tags: {section: h2}}\n    theorems: false\n  \
                        parsing: {dollar_math: true}\n  layout: blocks\n  \
                        default_profile: text\n---\nHello.\n";

    /// A front matter with comments, blank lines and a block scalar.
    const COMMENTED: &str = "---\n# The paper.\ntitle: Notes   # a working title\n\nabstract: |\n  Two lines\n  of text.\nflm:\n  features:\n    # Headings as in the journal.\n    headings:\n      depth: 3   # no deeper\n      style: plain\n\n    theorems: false\n  layout: blocks\n---\nThe content.\n";

    fn map(value: Value) -> Map<String, Value> {
        value.as_object().cloned().unwrap()
    }

    /// `text` edited to hold `features`, checked to read back as them.
    fn edited(text: &str, features: Value) -> String {
        let features = map(features);
        let written = edit_features(text, &features).unwrap();
        assert_eq!(written_features(&written).unwrap(), Value::Object(features));
        written
    }

    /// The features `text` writes, as a map.
    fn features_of(text: &str) -> Value {
        written_features(text).unwrap()
    }

    #[test]
    fn the_same_features_give_the_same_text_byte_for_byte() {
        for text in [FILE, COMMENTED, &COMMENTED.replace('\n', "\r\n")] {
            assert_eq!(edited(text, features_of(text)), text);
        }
    }

    #[test]
    fn a_value_changed_changes_its_line_alone() {
        let mut features = features_of(COMMENTED);
        features["headings"]["depth"] = json!(4);
        let written = edited(COMMENTED, features.clone());
        assert_eq!(written, COMMENTED.replace("depth: 3   # no deeper", "depth: 4   # no deeper"));
        features["headings"]["style"] = json!("numbered");
        assert_eq!(edited(&written, features), written.replace("style: plain", "style: numbered"));
    }

    #[test]
    fn a_key_added_is_one_line_at_its_map_s_end_indented_as_its_siblings() {
        let mut features = features_of(COMMENTED);
        features["headings"]["numbered"] = json!(true);
        let written = edited(COMMENTED, features);
        // The blank line after the map is the map's own, in the editor's view: the line comes
        // after it.
        assert_eq!(written, COMMENTED.replace("      style: plain\n\n", "      style: plain\n\n      numbered: true\n"));
    }

    #[test]
    fn a_key_removed_takes_its_line_alone() {
        let mut features = features_of(COMMENTED);
        features["headings"].as_object_mut().unwrap().remove("style");
        assert_eq!(edited(COMMENTED, features), COMMENTED.replace("      style: plain\n", ""));
    }

    #[test]
    fn a_feature_switched_off_and_on_and_a_feature_off_given_settings() {
        let mut features = features_of(COMMENTED);
        features["headings"] = json!(false);
        let off = edited(COMMENTED, features.clone());
        // The block of settings replaced, the blank line after it, which is its own in the
        // editor's view, included.
        assert_eq!(off, COMMENTED.replace("    headings:\n      depth: 3   # no deeper\n      style: plain\n\n", "    headings: false\n"));
        features.as_object_mut().unwrap().remove("headings");
        let on = edited(&off, features.clone());
        assert_eq!(on, off.replace("    headings: false\n", ""));
        features["theorems"] = json!({ "numbering": "per-section" });
        let given = edited(&on, features);
        assert!(given.contains("    theorems:\n      numbering: per-section\n  layout: blocks\n"), "{given}");
        assert!(given.starts_with("---\n# The paper.\ntitle: Notes   # a working title\n\nabstract: |\n"), "{given}");
    }

    #[test]
    fn a_new_feature_with_nested_settings_where_flm_or_features_or_the_front_matter_is_missing() {
        let new = json!({ "headings": { "depth": 2, "tags": { "section": "h2" }, "names": ["a", "b"] } });
        let root = edited("---\ntitle: Notes  # kept\n---\nHello.\n", new.clone());
        assert!(root.starts_with("---\ntitle: Notes  # kept\nflm:\n  features:\n    headings:\n"), "{root}");
        let features = edited("---\ntitle: Notes\nflm:\n  layout: blocks  # kept\n---\nHello.\n", new.clone());
        assert!(features.starts_with("---\ntitle: Notes\nflm:\n  layout: blocks  # kept\n  features:\n    headings:\n"), "{features}");
        let none = edited("Hello.\n---\n", new.clone());
        assert!(none.starts_with("---\nflm:\n  features:\n    headings:\n") && none.ends_with("\n---\nHello.\n---\n"), "{none}");
        let empty = edited("---\n---\nHello.\n", new);
        assert!(empty.starts_with("---\nflm:\n  features:\n") && empty.ends_with("---\nHello.\n"), "{empty}");
    }

    #[test]
    fn a_block_map_whose_every_key_is_replaced_stays_in_block_style() {
        let text = "---\ntitle: A paper\nflm:\n  features:\n    theorems: false\n---\n";
        let written = edited(text, json!({ "math": { "numbered_equations": "yes" } }));
        assert_eq!(written, "---\ntitle: A paper\nflm:\n  features:\n    math:\n      numbered_equations: 'yes'\n---\n");
        assert_eq!(edited(&written, json!({})), "---\ntitle: A paper\nflm:\n  features: {}\n---\n");
    }

    #[test]
    fn a_map_written_into_a_flow_map_stays_in_flow_style() {
        let text = "---\nflm:\n  features: {theorems: false}\n---\n";
        let written = edited(text, json!({ "theorems": false, "headings": { "depth": 2 } }));
        assert_eq!(written, "---\nflm:\n  features: {theorems: false, headings: {depth: 2}}\n---\n");
    }

    #[test]
    fn a_list_entry_changed_changes_that_entry_alone() {
        let flow = "---\nflm:\n  features:\n    headings: {names: [a, b, c]}  # three\n---\n";
        let written = edited(flow, json!({ "headings": { "names": ["a", "x", "c"] } }));
        assert_eq!(written, flow.replace("[a, b, c]", "[a, x, c]"));
        let block = "---\nflm:\n  features:\n    headings:\n      names:\n        - a   # first\n        - b\n---\n";
        let written = edited(block, json!({ "headings": { "names": ["a", "y"] } }));
        assert_eq!(written, block.replace("- b\n", "- y\n"));
    }

    #[test]
    fn the_fences_and_the_content_are_kept_byte_for_byte() {
        let text = "---  \r\ntitle: x\r\n---\t\r\n\\section{A}\r\n---\r\ntrailing";
        let written = edited(text, json!({ "theorems": false }));
        assert!(written.starts_with("---  \r\ntitle: x\r\n"), "{written:?}");
        assert!(written.ends_with("---\t\r\n\\section{A}\r\n---\r\ntrailing"), "{written:?}");
    }

    #[test]
    fn a_front_matter_that_cannot_be_read_or_whose_flm_is_not_a_map_is_refused() {
        let text = "---\ntitle: [unclosed\n---\nHello.\n";
        let error = edit_features(text, &Map::new()).unwrap_err();
        assert_eq!(written_features(text).unwrap_err(), error);
        assert!(edit_features("---\n- a\n---\n", &Map::new()).unwrap_err().detail.contains("must be a map"));
        let error = edit_features("---\nflm: 3\n---\n", &Map::new()).unwrap_err();
        assert_eq!((error.detail.as_str(), error.place), ("the value of `flm` must be a map", Some(crate::seed::Place { line: 2, column: 6 })));
        assert!(edit_features("---\ntitle: x\n", &Map::new()).is_err());
    }

    #[test]
    fn a_block_list_shortened_at_the_front_matter_s_end_keeps_the_closing_fence_on_its_own_line() {
        let text = "---\nflm:\n  features:\n    h:\n      n:\n        - a\n        - b\n---\nText.\n---\nmore\n";
        assert_eq!(
            edited(text, json!({ "h": { "n": ["a"] } })),
            "---\nflm:\n  features:\n    h:\n      n:\n        - a\n---\nText.\n---\nmore\n"
        );
    }

    /// Rests on the YAML editor's behavior of today (yaml-edit 0.3.2): the comment at the end
    /// of the front matter's last line comes out on a line of its own below the block of
    /// settings that replaces the line's value.
    #[test]
    fn a_value_with_a_comment_on_the_front_matter_s_last_line_replaced_by_a_map() {
        let text = "---\ntitle: Notes\nflm:\n  features:\n    theorems: false   # off\n---\nHello.\n";
        assert_eq!(
            edited(text, json!({ "theorems": { "x": 1 } })),
            "---\ntitle: Notes\nflm:\n  features:\n    theorems:\n      x: 1\n   # off\n---\nHello.\n"
        );
    }

    /// Rests on the YAML editor's behavior of today (yaml-edit 0.3.2): a key whose value is
    /// empty, followed by a sibling at the same indentation, given a map, puts the sibling one
    /// level up. The check before writing refuses the edit.
    #[test]
    fn an_edit_that_does_not_read_back_as_intended_is_refused() {
        let text = "---\nflm:\n  features:\n  layout: blocks\n---\n";
        let error = edit_features(text, &map(json!({ "theorems": false }))).unwrap_err();
        assert_eq!(error.place, None);
        assert!(error.detail.contains("exactly as it is laid out"), "{error}");
    }
}
