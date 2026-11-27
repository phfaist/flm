//! Editing a document's front matter in place: the changes that a page of the live preview
//! asks for in one request (`POST` of the server's front matter) are written into the
//! document's file together, and every other byte stays as the author wrote it.
//!
//! A document's [front matter](crate::seed) is the block of YAML between a first line `---`
//! and the next `---` line of its file ([`crate::seed::split`]). An [`Edit`] names a path, the
//! keys from the root of the front matter ([`KeyPath`]), and either sets the value there or
//! removes it ([`Change`]). [`edit_front_matter`] returns the file's text with a list of edits
//! made, in order, and changes nothing else: the comments, the blank lines, the quoting, the
//! style (block or flow) of each map and list, the two `---` lines and the content below them
//! stay as they were. It compares what the front matter holds now, as data, with what it
//! holds after the edits, and rewrites only the values that differ, in place, through a YAML
//! editor that keeps every byte it is not asked to change (the crate `yaml_edit`).
//! [`DocumentSettings::write`](super::settings::DocumentSettings::write) calls it.

use serde_json::{Map, Value};
use yaml_edit::{AsYaml, Mapping, ScalarValue, Sequence, YamlFile, YamlNode};

use crate::seed::{Node, RESERVED_KEY, SeedError, read_front_matter, split};

/// A path in a front matter: the keys from its root, at least one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPath(Vec<String>);

impl KeyPath {
    /// Returns the path of the keys `keys`, from the root; `None` when `keys` is empty.
    pub fn new(keys: Vec<String>) -> Option<KeyPath> {
        (!keys.is_empty()).then_some(KeyPath(keys))
    }

    /// The keys of the path, from the root, at least one.
    pub fn keys(&self) -> &[String] {
        &self.0
    }
}

/// One change to a front matter, at the path of its [`Edit`].
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    /// Sets the value at the path; the maps that the path lacks are created.
    Set(Value),
    /// Removes the value at the path, and each map that the removal leaves empty.
    Remove,
}

/// One edit of a front matter: a change at a path.
#[derive(Debug, Clone, PartialEq)]
pub struct Edit {
    /// The path of the value changed.
    pub path: KeyPath,
    /// The change made at the path.
    pub change: Change,
}

/// Returns the text of the file `text` with the edits `edits` made in its front matter, every
/// other byte as it was.
///
/// The edits are made in the order given, each on the data as the edits before it left it;
/// an empty list, or edits that leave the data as it was, give `text` itself. Each key of an
/// edit's path but the last names a map. A value set creates the maps that the path lacks (a
/// key that holds `null` counts as lacking); a removal of a key that the front matter does
/// not hold changes nothing, and a removal also removes each map of the path that it leaves
/// empty.
///
/// The value that the front matter holds is compared with the value it holds after the
/// edits, as data, and the text is changed only where the two differ: a key removed goes
/// with its line; a key added is laid out as its siblings are, at the end of its map, except
/// in two maps, where the keys held keep their order and the key added takes its place among
/// them: in the root map, a key of the metadata goes after the last key other than `flm`,
/// else before `flm`, and `flm` at the end; in `flm`, the keys are placed in the order
/// `initial_use`, `math`, `use`, `default_profile`, `layout`, `parsing`, `features`,
/// `feature_order`, a key added after the nearest key held that this order puts before it,
/// else before the nearest one it puts after it; two maps are compared key by key and two
/// lists entry by entry; any other value that
/// differs is written in place of the old one. The YAML editor decides the layout of what
/// it writes: a new line is indented as its siblings and ends with a line feed (also in a
/// file whose lines end with a carriage return and a line feed), and a text that would
/// otherwise read as another type (`'true'`, `'1'`), or a text written into a map in flow
/// style (`{a: "x"}`), is quoted. A file with no front matter receives one, between two
/// `---` lines, above its content.
///
/// Two kinds of value are never written: a text of several lines, and a map placed as an
/// entry of a list in block style (`- …` lines). The YAML editor does not write them
/// exactly; each is refused with a reason of its own.
///
/// The front matter's text ends with a line end, so that the closing `---` is on a line of
/// its own: where the edited text lacks one (for example because the removal of its last
/// line also removed that line's end), a line feed is added. The whole new file is read
/// back before it is returned, as the program reads a file ([`split`], then
/// [`read_front_matter`]): its front matter's data must equal the data after the edits,
/// and its content must equal the old content. When the YAML editor cannot write the edits
/// exactly, they are refused.
///
/// # Errors
///
/// Returns a [`SeedError`] when the front matter cannot be read ([`split`]'s and
/// [`read_front_matter`]'s errors), when its root is not a map, when a key of an edit's path
/// but the last holds a value that is not a map, when a value set holds one of the two kinds
/// of value above, and when the edits are refused. The first edit whose path is refused ends
/// the call. The error's place is the one of the front matter's root, or of the value that is
/// not a map where the front matter as read holds it.
///
/// # Examples
///
/// ```
/// use flm_host::watch::front_matter::{Change, Edit, KeyPath, edit_front_matter};
/// use serde_json::json;
///
/// let text = "---\ntitle: Notes  # for now\nflm:\n  features:\n    theorems: {numbered: true}\n---\nHello.\n";
/// let path = |keys: &[&str]| KeyPath::new(keys.iter().map(|key| key.to_string()).collect()).unwrap();
/// let lists = Edit { path: path(&["flm", "features", "lists"]), change: Change::Set(json!(false)) };
/// assert_eq!(
///     edit_front_matter(text, &[lists]).unwrap(),
///     "---\ntitle: Notes  # for now\nflm:\n  features:\n    theorems: {numbered: true}\n    lists: false\n---\nHello.\n"
/// );
/// let numbered = Edit { path: path(&["flm", "features", "theorems", "numbered"]), change: Change::Remove };
/// assert_eq!(edit_front_matter(text, &[numbered]).unwrap(), "---\ntitle: Notes  # for now\n---\nHello.\n");
/// ```
pub fn edit_front_matter(text: &str, edits: &[Edit]) -> Result<String, SeedError> {
    let parts = split(text)?;
    let yaml = parts.front_matter.unwrap_or("");
    let root = read_front_matter(yaml, parts.front_matter_line)?;
    let before = match root.to_json() {
        Value::Null => Map::new(),
        Value::Object(before) => before,
        _ => return Err(SeedError::at(root.place, "The front matter must be a map from keys to values")),
    };
    let mut intended = before.clone();
    for Edit { path, change } in edits {
        let path = path.keys();
        let changed = match change {
            Change::Set(value) => set_at(&mut intended, path, value.clone()),
            Change::Remove => remove_at(&mut intended, path).map(drop),
        };
        if let Err(depth) = changed {
            return Err(not_a_map(&root, &intended, path, depth));
        }
    }
    if intended == before {
        return Ok(text.to_string());
    }

    let file: YamlFile = yaml.parse().map_err(|_| refused(edits))?;
    let Some(mapping) = file.ensure_document().as_mapping() else {
        return Err(refused(edits));
    };
    let mut editor = Editor { limit: None };
    editor.edit_map(&mapping, &before, &intended, Placement::Root);
    if let Some(limit) = editor.limit {
        return Err(limit.refusal(edits));
    }
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
    // The file read back before it is given: the YAML editor writes some shapes wrongly, and
    // this check is what makes writing the author's file safe. The whole text is checked,
    // since a check of the front matter alone would miss a closing fence glued to its last
    // line. An empty front matter reads as `null`, the empty map.
    let reads_as_intended = split(&written).is_ok_and(|new| {
        new.content == parts.content
            && read_front_matter(new.front_matter.unwrap_or(""), new.front_matter_line).is_ok_and(|read_back| {
                match read_back.to_json() {
                    Value::Null => intended.is_empty(),
                    Value::Object(read_back) => read_back == intended,
                    _ => false,
                }
            })
    });
    if !reads_as_intended {
        return Err(refused(edits));
    }
    Ok(written)
}

/// Sets `value` at `path`, at least one key, in `map`, creating the maps that the path
/// lacks; `Err` gives the index in `path` of a key that holds a value that is neither a map
/// nor `null`.
fn set_at(map: &mut Map<String, Value>, path: &[String], value: Value) -> Result<(), usize> {
    let [key, rest @ ..] = path else { return Ok(()) };
    if rest.is_empty() {
        map.insert(key.clone(), value);
        return Ok(());
    }
    let child = map.entry(key.clone()).or_insert(Value::Null);
    if child.is_null() {
        *child = Value::Object(Map::new());
    }
    match child {
        Value::Object(child) => set_at(child, rest, value).map_err(|depth| depth + 1),
        _ => Err(0),
    }
}

/// Removes the value at `path`, at least one key, from `map`, and each map of the path that
/// the removal leaves empty; returns whether it removed a value. `Err` gives the index in
/// `path` of a key that holds a value that is neither a map nor `null`.
fn remove_at(map: &mut Map<String, Value>, path: &[String]) -> Result<bool, usize> {
    let [key, rest @ ..] = path else { return Ok(false) };
    if rest.is_empty() {
        return Ok(map.remove(key).is_some());
    }
    let Some(child) = map.get_mut(key) else { return Ok(false) };
    let removed = match child {
        Value::Null => return Ok(false),
        Value::Object(child) => remove_at(child, rest).map_err(|depth| depth + 1)?,
        _ => return Err(0),
    };
    if removed && map.get(key).and_then(Value::as_object).is_some_and(Map::is_empty) {
        map.remove(key);
    }
    Ok(removed)
}

/// The refusal of a change at `path` whose key at the index `depth` holds a value that is
/// not a map in `data`, the front matter's data as the edits before it left it. Its place is
/// the one of that value in `root`, the front matter as read, when `root` holds it there.
fn not_a_map(root: &Node, data: &Map<String, Value>, path: &[String], depth: usize) -> SeedError {
    let keys: Vec<&str> = path[..=depth].iter().map(String::as_str).collect();
    let held = keys[1..].iter().fold(data.get(keys[0]), |value, key| value.and_then(|value| value.get(*key)));
    let place = root.at(&keys).filter(|node| held == Some(&node.to_json())).map(|node| node.place);
    let detail = format!("Invalid path ‘{}’: ‘{}’ must be a map", dotted(path), keys.join("."));
    SeedError { place, detail }
}

/// The refusal of the edits `edits` that the YAML editor cannot write exactly.
fn refused(edits: &[Edit]) -> SeedError {
    SeedError {
        place: None,
        detail: format!("Unable to write {} exactly as the front matter is laid out; change it in the file itself", named(edits)),
    }
}

/// `path` written as its keys joined by dots, as a message shows it.
fn dotted(path: &[String]) -> String {
    path.join(".")
}

/// The paths of `edits`, each dotted and quoted, as a message lists them: ‘a.b’, ‘a.b’ and
/// ‘c’, or ‘a.b’, ‘c’ and ‘d’.
fn named(edits: &[Edit]) -> String {
    let paths: Vec<String> = edits.iter().map(|edit| format!("‘{}’", dotted(edit.path.keys()))).collect();
    match paths.split_last() {
        Some((last, [])) => last.clone(),
        Some((last, others)) => format!("{} and {last}", others.join(", ")),
        None => String::new(),
    }
}

/// A kind of value that [`edit_front_matter`] never writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Limit {
    /// A text of several lines.
    SeveralLines,
    /// A map placed as an entry of a list in block style.
    MapInBlockList,
}

impl Limit {
    /// The refusal of the edits `edits`, one of whose values holds this kind of value.
    fn refusal(self, edits: &[Edit]) -> SeedError {
        let paths = named(edits);
        let detail = match self {
            Limit::SeveralLines => {
                format!("A text of several lines may not be written from the page (in {paths}); write it in the file itself")
            }
            Limit::MapInBlockList => format!(
                "A map may not be written from the page as an entry of a list of ‘- …’ lines (in {paths}); write it in the file itself"
            ),
        };
        SeedError { place: None, detail }
    }
}

/// The keys of `flm`, in the order in which a key that `flm` lacks is written among those
/// it holds: the keys that decide the packages, in the order their packages are used (the
/// initial list, the math processor's package, those of `use`, the build type's package);
/// then the keys that decide how the content is read; then the settings of the features
/// and their order.
const FLM_ORDER: [&str; 8] =
    ["initial_use", "math", "use", "default_profile", "layout", "parsing", "features", "feature_order"];

/// Where a map stands in the front matter, which decides where a key that the map lacks is
/// written.
///
/// The text keeps the order of the keys that it holds; a key added is placed among them:
///
/// - in the root map, a key other than `flm` (one of the document's metadata) after the last
///   key other than `flm`, else before `flm`; `flm` at the end;
/// - in `flm`, after the nearest key that it holds among those that [`FLM_ORDER`] puts
///   before it, else before the nearest among those it puts after it;
/// - in any other map, at the end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placement {
    Root,
    Flm,
    Other,
}

impl Placement {
    /// The placement of the map at the key `key` of a map placed as `self`.
    fn of(self, key: &str) -> Placement {
        if self == Placement::Root && key == RESERVED_KEY { Placement::Flm } else { Placement::Other }
    }

    /// Writes `value` at `key` in `mapping`, a map placed as `self`: in place when the map
    /// holds the key, else where `self` places a key added.
    fn set(self, mapping: &Mapping, key: &str, value: &dyn AsYaml) {
        match self {
            Placement::Root if key != RESERVED_KEY => {
                // The order: the keys of the metadata as the text holds them, the key, `flm`.
                let held: Vec<YamlNode> = mapping.keys().filter(|held| *held != RESERVED_KEY).collect();
                let mut order: Vec<&dyn AsYaml> = held.iter().map(|held| held as &dyn AsYaml).collect();
                order.extend([&key as &dyn AsYaml, &RESERVED_KEY]);
                mapping.set_with_field_order(key, value, order);
            }
            Placement::Flm => mapping.set_with_field_order(key, value, FLM_ORDER),
            _ => mapping.set(key, value),
        }
    }
}

/// The edit of a front matter's YAML: the functions that write the edits into it, and the
/// first kind of value they met that [`edit_front_matter`] never writes.
struct Editor {
    limit: Option<Limit>,
}

impl Editor {
    /// Changes `mapping`, which holds `old` as data and is placed as `placement`, so that it
    /// holds `new`.
    ///
    /// The keys removed go last, so that a map keeps its style while it is edited: a block
    /// map whose last key is removed becomes `{}`, in flow style, and what is written into it
    /// after that would be written in flow style too.
    fn edit_map(&mut self, mapping: &Mapping, old: &Map<String, Value>, new: &Map<String, Value>, placement: Placement) {
        for (key, value) in new {
            let key = key.as_str();
            match old.get(key) {
                Some(was) if was == value => {}
                Some(was) if self.walk_into(mapping.get(key).as_ref(), was, value, placement.of(key)) => {}
                _ => {
                    let flow = mapping.is_flow_style();
                    placement.set(mapping, key, &*self.first_written(value, Container::Map, flow));
                    self.fill(mapping.get(key).as_ref(), value, flow, placement.of(key));
                }
            }
        }
        for key in old.keys().filter(|key| !new.contains_key(*key)) {
            remove_key(mapping, key);
        }
    }

    /// Changes `sequence`, which holds `old` as data, so that it holds `new`, entry by entry.
    fn edit_sequence(&mut self, sequence: &Sequence, old: &[Value], new: &[Value]) {
        for index in (new.len()..old.len()).rev() {
            sequence.remove(index);
        }
        for (index, (was, value)) in old.iter().zip(new).enumerate() {
            if was != value && !self.walk_into(sequence.get(index).as_ref(), was, value, Placement::Other) {
                let flow = sequence.is_flow_style();
                sequence.set(index, &*self.first_written(value, Container::Sequence, flow));
                self.fill(sequence.get(index).as_ref(), value, flow, Placement::Other);
            }
        }
        let flow = sequence.is_flow_style();
        for value in new.iter().skip(old.len()) {
            sequence.push(&*self.first_written(value, Container::Sequence, flow));
            self.fill(sequence.last().as_ref(), value, flow, Placement::Other);
        }
    }

    /// Changes `node`, which holds `old` as data, so that it holds `new`, when both are maps
    /// (`node` placed as `placement`) or both are lists; says whether it did.
    fn walk_into(&mut self, node: Option<&YamlNode>, old: &Value, new: &Value, placement: Placement) -> bool {
        match (old, new, node) {
            (Value::Object(old), Value::Object(new), Some(YamlNode::Mapping(mapping))) => {
                self.edit_map(mapping, old, new, placement);
            }
            (Value::Array(old), Value::Array(new), Some(YamlNode::Sequence(sequence))) => {
                self.edit_sequence(sequence, old, new);
            }
            _ => return false,
        }
        true
    }

    /// Writes the entries of `value` into `node`, the empty map or list
    /// [`Editor::first_written`] gave for it, in flow style when `flow`, a map placed as
    /// `placement`; a value that is neither is written whole by [`Editor::first_written`].
    ///
    /// A map or a list is written empty first and filled in place, so that the editor lays
    /// out its entries as it lays out any entry of the text. Its style is the one it was
    /// written in, passed down: the YAML editor shows a map or a list in block style that
    /// has no entry yet as `{}` or `[]`, in flow style.
    fn fill(&mut self, node: Option<&YamlNode>, value: &Value, flow: bool, placement: Placement) {
        match (value, node) {
            (Value::Object(entries), Some(YamlNode::Mapping(mapping))) => {
                for (key, value) in entries {
                    placement.set(mapping, key, &*self.first_written(value, Container::Map, flow));
                    self.fill(mapping.get(key.as_str()).as_ref(), value, flow, placement.of(key));
                }
            }
            (Value::Array(items), Some(YamlNode::Sequence(sequence))) => {
                for value in items {
                    sequence.push(&*self.first_written(value, Container::Sequence, flow));
                    self.fill(sequence.last().as_ref(), value, flow, Placement::Other);
                }
            }
            _ => {}
        }
    }

    /// What is first written for `value` into a map or a list (`into`), in flow style when
    /// `flow`: the value itself when it is neither a map nor a list, else an empty map or
    /// list in the same style, which [`Editor::fill`] then fills. Records a value of a kind
    /// that is never written.
    fn first_written(&mut self, value: &Value, into: Container, flow: bool) -> Box<dyn AsYaml> {
        let limit = match value {
            Value::String(text) if text.contains(['\n', '\r']) => Some(Limit::SeveralLines),
            Value::Object(_) if into == Container::Sequence && !flow => Some(Limit::MapInBlockList),
            _ => None,
        };
        self.limit = self.limit.or(limit);
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
}

/// Removes `key` from `mapping`, with its line in block style.
///
/// In flow style, the YAML editor keeps the separator `, ` with the entry it follows: the
/// removal of the last entry leaves it after the entry before, as in `{a: 1, }`. That entry
/// is then taken out and written back as it was, at the end, without the separator.
fn remove_key(mapping: &Mapping, key: &str) {
    let keys: Vec<YamlNode> = mapping.keys().collect();
    mapping.remove(key);
    if let [.., before, last] = keys.as_slice()
        && mapping.is_flow_style()
        && last == key
        && let Some(value) = mapping.remove(before).and_then(|entry| entry.value_node())
    {
        mapping.set(before, &value);
    }
}

/// What [`Editor::first_written`] writes a value into: a map or a list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Container {
    Map,
    Sequence,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A front matter with comments, blank lines and a block scalar.
    const COMMENTED: &str = "---\n# The paper.\ntitle: Notes   # a working title\n\nabstract: |\n  Two lines\n  of text.\nflm:\n  use: [std]   # the default\n  features:\n    # Headings as in the journal.\n    headings:\n      depth: 3   # no deeper\n      style: plain\n\n    floats: {types: {figure: {name: Fig.}}}\n    theorems: false\n  layout: blocks\n---\nThe content.\n";

    /// The path of the keys written in `dotted`, joined by dots.
    fn path(dotted: &str) -> KeyPath {
        KeyPath::new(dotted.split('.').map(String::from).collect()).unwrap()
    }

    /// The one edit `change` at the path written in `at`.
    fn one(at: &str, change: Change) -> [Edit; 1] {
        [Edit { path: path(at), change }]
    }

    /// `text` with `change` made at `at`, checked to read back as `text`'s front matter with
    /// that change.
    fn edited(text: &str, at: &str, change: Change) -> String {
        let written = edit_front_matter(text, &one(at, change.clone())).unwrap();
        let read = |text: &str| {
            let parts = split(text).unwrap();
            read_front_matter(parts.front_matter.unwrap_or(""), 1).unwrap().to_json()
        };
        let mut intended = match read(text) {
            Value::Object(map) => map,
            _ => Map::new(),
        };
        match change {
            Change::Set(value) => set_at(&mut intended, path(at).keys(), value).unwrap(),
            Change::Remove => drop(remove_at(&mut intended, path(at).keys()).unwrap()),
        }
        let read_back = match read(&written) {
            Value::Null => Map::new(),
            Value::Object(map) => map,
            other => panic!("{other}"),
        };
        assert_eq!(read_back, intended, "{written}");
        written
    }

    #[test]
    fn one_change_at_each_kind_of_path_changes_its_line_alone() {
        let cases = [
            ("title", json!("Lecture notes"), "title: Notes   # a working title", "title: Lecture notes   # a working title"),
            ("flm.layout", json!("paragraphs"), "  layout: blocks\n", "  layout: paragraphs\n"),
            ("flm.features.headings.depth", json!(4), "depth: 3   # no deeper", "depth: 4   # no deeper"),
            ("flm.features.floats.types.figure.name", json!("Figure"), "{name: Fig.}", "{name: Figure}"),
        ];
        for (at, value, line, changed) in cases {
            assert_eq!(edited(COMMENTED, at, Change::Set(value)), COMMENTED.replace(line, changed), "{at}");
        }
        assert_eq!(edited(COMMENTED, "flm.use", Change::Set(json!(["std", "notes"]))), COMMENTED.replace("[std]", "[std, notes]"));
        // The same value gives the same text, byte for byte.
        for text in [COMMENTED, &COMMENTED.replace('\n', "\r\n")] {
            assert_eq!(edited(text, "flm.layout", Change::Set(json!("blocks"))), text);
        }
    }

    #[test]
    fn a_key_added_is_one_line_indented_as_its_siblings_at_its_map_s_end() {
        let written = edited(COMMENTED, "flm.features.headings.numbered", Change::Set(json!(true)));
        // The line comes right after the map's last entry, before the blank line that follows
        // the map.
        assert_eq!(written, COMMENTED.replace("      style: plain\n\n", "      style: plain\n      numbered: true\n\n"));
    }

    #[test]
    fn a_key_added_to_the_root_or_to_flm_takes_its_place_in_their_order_among_the_keys_held() {
        // A key of the metadata after the metadata, before `flm`; `flm` at the end.
        let written = edited(COMMENTED, "author", Change::Set(json!("A. Writer")));
        assert_eq!(written, COMMENTED.replace("  of text.\nflm:\n", "  of text.\nauthor: A. Writer\nflm:\n"));
        let text = "---\ntitle: Notes\n---\nHello.\n";
        assert_eq!(edited(text, "flm.math", Change::Set(json!(false))), "---\ntitle: Notes\nflm:\n  math: false\n---\nHello.\n");
        let text = "---\nflm:\n  use: [notes]\ntitle: Notes\n---\n";
        assert_eq!(edited(text, "date", Change::Set(json!("Today"))), "---\nflm:\n  use: [notes]\ntitle: Notes\ndate: Today\n---\n");
        // In `flm`, after the nearest key held that the order puts before it, else before the
        // nearest it puts after it: the order of the keys held is kept.
        let written = edited(COMMENTED, "flm.math", Change::Set(json!("mathjax")));
        assert_eq!(written, COMMENTED.replace("flm:\n  use:", "flm:\n  math: mathjax\n  use:"));
        let written = edited(COMMENTED, "flm.parsing.dollar_math", Change::Set(json!(true)));
        assert_eq!(written, COMMENTED.replace("  layout: blocks\n", "  layout: blocks\n  parsing:\n    dollar_math: true\n"));
        let text = "---\nflm:\n  use: [notes]\n  features:\n    toc: false\n---\n";
        let written = edited(text, "flm.parsing.dollar_math", Change::Set(json!(true)));
        assert_eq!(written, "---\nflm:\n  use: [notes]\n  parsing:\n    dollar_math: true\n  features:\n    toc: false\n---\n");
    }

    #[test]
    fn a_removal_removes_its_line_and_the_maps_it_leaves_empty() {
        let written = edited(COMMENTED, "flm.features.headings.style", Change::Remove);
        assert_eq!(written, COMMENTED.replace("      style: plain\n", ""));
        // The map `figure`, then `types`, then `floats`, left empty.
        let written = edited(COMMENTED, "flm.features.floats.types.figure.name", Change::Remove);
        assert_eq!(written, COMMENTED.replace("    floats: {types: {figure: {name: Fig.}}}\n", ""));
        // `features`, then `flm`, left empty.
        let text = "---\ntitle: Notes\nflm:\n  features:\n    theorems: false\n---\nHello.\n";
        assert_eq!(edited(text, "flm.features.theorems", Change::Remove), "---\ntitle: Notes\n---\nHello.\n");
        // A map that the removal does not empty, or that was empty, stays; a key that is not
        // written changes nothing.
        let text = "---\nflm:\n  features: {}\n  layout: blocks\n---\n";
        assert_eq!(edited(text, "flm.layout", Change::Remove), "---\nflm:\n  features: {}\n---\n");
        assert_eq!(edited(text, "flm.features.theorems.enabled", Change::Remove), text);
    }

    #[test]
    fn a_feature_switched_off_and_on_again_gives_the_text_it_had() {
        let block = "---\nflm:\n  features:\n    citations:\n      mark: superscript  # as the journal\n\n    toc: {depth: 2}\n---\nHello.\n";
        let one_line = "---\nflm:\n  features: {citations: {mark: superscript}, toc: {depth: 2}}\n---\nHello.\n";
        for (text, off) in [
            (block, block.replace("superscript  # as the journal\n\n", "superscript  # as the journal\n      enabled: false\n\n")),
            (one_line, one_line.replace("{mark: superscript}", "{mark: superscript, enabled: false}")),
        ] {
            let switched_off = edited(text, "flm.features.citations.enabled", Change::Set(json!(false)));
            assert_eq!(switched_off, off);
            assert_eq!(edited(&switched_off, "flm.features.citations.enabled", Change::Remove), text);
        }
    }

    #[test]
    fn a_value_set_creates_the_maps_its_path_lacks() {
        let value = json!({ "depth": 2, "tags": { "section": "h2" }, "names": ["a", "b"] });
        let root = edited("---\ntitle: Notes  # kept\n---\nHello.\n", "flm.features.headings", Change::Set(value.clone()));
        assert!(root.starts_with("---\ntitle: Notes  # kept\nflm:\n  features:\n    headings:\n"), "{root}");
        let features = edited("---\ntitle: Notes\nflm:\n  layout: blocks  # kept\n---\nHello.\n", "flm.features.headings", Change::Set(value.clone()));
        assert!(features.starts_with("---\ntitle: Notes\nflm:\n  layout: blocks  # kept\n  features:\n    headings:\n"), "{features}");
        let none = edited("Hello.\n---\n", "flm.features.headings", Change::Set(value.clone()));
        assert!(none.starts_with("---\nflm:\n  features:\n    headings:\n") && none.ends_with("\n---\nHello.\n---\n"), "{none}");
        let empty = edited("---\n---\nHello.\n", "flm.features.headings", Change::Set(value.clone()));
        assert!(empty.starts_with("---\nflm:\n  features:\n") && empty.ends_with("---\nHello.\n"), "{empty}");
        // A key that holds `null` counts as lacking.
        let null = edited("---\nflm: ~\n---\n", "flm.features.headings", Change::Set(value));
        assert!(null.starts_with("---\nflm:\n  features:\n    headings:\n"), "{null}");
    }

    #[test]
    fn a_map_written_into_a_flow_map_stays_in_flow_style() {
        let text = "---\nflm:\n  features: {theorems: false}\n---\n";
        let written = edited(text, "flm.features.headings", Change::Set(json!({ "depth": 2 })));
        assert_eq!(written, "---\nflm:\n  features: {theorems: false, headings: {depth: 2}}\n---\n");
    }

    #[test]
    fn a_list_entry_changed_changes_that_entry_alone() {
        let flow = "---\nflm:\n  features:\n    headings: {names: [a, b, c]}  # three\n---\n";
        let written = edited(flow, "flm.features.headings.names", Change::Set(json!(["a", "x", "c"])));
        assert_eq!(written, flow.replace("[a, b, c]", "[a, x, c]"));
        let block = "---\nflm:\n  features:\n    headings:\n      names:\n        - a   # first\n        - b\n---\n";
        let written = edited(block, "flm.features.headings.names", Change::Set(json!(["a", "y"])));
        assert_eq!(written, block.replace("- b\n", "- y\n"));
    }

    #[test]
    fn two_edits_of_one_call_are_written_together_and_every_other_byte_is_kept() {
        let edits = [
            Edit { path: path("flm.features.headings.depth"), change: Change::Set(json!(2)) },
            Edit { path: path("flm.features.headings.style"), change: Change::Remove },
        ];
        let written = edit_front_matter(COMMENTED, &edits).unwrap();
        let intended = COMMENTED.replace("depth: 3   # no deeper\n      style: plain\n", "depth: 2   # no deeper\n");
        assert_eq!(written, intended);
    }

    #[test]
    fn the_fences_and_the_content_are_kept_byte_for_byte() {
        let text = "---  \r\ntitle: x\r\n---\t\r\n\\section{A}\r\n---\r\ntrailing";
        let written = edited(text, "flm.features.theorems", Change::Set(json!(false)));
        assert!(written.starts_with("---  \r\ntitle: x\r\n"), "{written:?}");
        assert!(written.ends_with("---\t\r\n\\section{A}\r\n---\r\ntrailing"), "{written:?}");
    }

    #[test]
    fn a_front_matter_that_cannot_be_read_or_a_path_through_a_value_that_is_not_a_map_is_refused() {
        let set = Change::Set(json!(1));
        let error = edit_front_matter("---\ntitle: [unclosed\n---\nHello.\n", &one("title", set.clone())).unwrap_err();
        assert_eq!(error.place, Some(crate::seed::Place { line: 3, column: 1 }));
        assert!(edit_front_matter("---\n- a\n---\n", &one("title", set.clone())).is_err());
        assert!(edit_front_matter("---\ntitle: x\n", &one("title", set.clone())).is_err());
        let text = "---\nflm:\n  features:\n    theorems: false\n---\n";
        for change in [set, Change::Remove] {
            let error = edit_front_matter(text, &one("flm.features.theorems.enabled", change)).unwrap_err();
            assert_eq!(error.place, Some(crate::seed::Place { line: 4, column: 15 }), "{error}");
        }
    }

    #[test]
    fn a_text_of_several_lines_and_a_map_in_a_block_list_are_refused() {
        let text = "---\nabstract: One line.\nflm:\n  features:\n    floats:\n      list:\n        - a\n    toc: {depth: 2}\n---\n";
        for (at, value, limit) in [
            ("abstract", json!("Two\nlines."), Limit::SeveralLines),
            ("flm.features.floats", json!({ "caption": { "text": "a\r\nb" } }), Limit::SeveralLines),
            ("flm.features.floats.list", json!(["a", { "b": 1 }]), Limit::MapInBlockList),
            ("flm.features.headings.names", json!([{ "a": 1 }]), Limit::MapInBlockList),
        ] {
            let edits = one(at, Change::Set(value));
            let error = edit_front_matter(text, &edits).unwrap_err();
            assert_eq!(error, limit.refusal(&edits), "{at}");
        }
        // A map as an entry of a list in flow style is written.
        let written = edited(text, "flm.features.toc.list", Change::Set(json!([{ "a": 1 }])));
        assert_eq!(written, text.replace("{depth: 2}", "{depth: 2, list: [{a: 1}]}"));
    }

    #[test]
    fn a_block_list_shortened_at_the_front_matter_s_end_keeps_the_closing_fence_on_its_own_line() {
        let text = "---\nflm:\n  features:\n    h:\n      n:\n        - a\n        - b\n---\nText.\n---\nmore\n";
        assert_eq!(
            edited(text, "flm.features.h.n", Change::Set(json!(["a"]))),
            "---\nflm:\n  features:\n    h:\n      n:\n        - a\n---\nText.\n---\nmore\n"
        );
    }

    /// Rests on the YAML editor's behavior of today (yaml-edit 0.3.6): the comment at the end
    /// of the front matter's last line stays on the key's line, above the block of settings
    /// that replaces the line's value.
    #[test]
    fn a_value_with_a_comment_on_the_front_matter_s_last_line_replaced_by_a_map() {
        let text = "---\ntitle: Notes\nflm:\n  features:\n    theorems: false   # off\n---\nHello.\n";
        assert_eq!(
            edited(text, "flm.features.theorems", Change::Set(json!({ "x": 1 }))),
            "---\ntitle: Notes\nflm:\n  features:\n    theorems: # off\n      x: 1\n---\nHello.\n"
        );
    }

    /// A whole number beyond the 64-bit integers of the YAML reader is written as it stands,
    /// but does not read back as that number. The check before writing refuses the edit.
    #[test]
    fn an_edit_that_does_not_read_back_as_intended_is_refused() {
        let text = "---\nflm:\n  layout: blocks\n---\n";
        let edits = one("flm.layout", Change::Set(json!(12345678901234567890u64)));
        let error = edit_front_matter(text, &edits).unwrap_err();
        assert_eq!(error, refused(&edits));
    }
}
