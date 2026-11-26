//! The marks of a declaration on the nodes of a JSON Schema: reading them from
//! a schema given as data, finding the nodes at a path, and writing them on the
//! nodes of a printed schema ([`SettingsDecl::json_schema`](super::SettingsDecl::json_schema)).
//!
//! A path is a list of segments below the feature's name. From a node, a
//! segment `K` leads to the schema of the property `K`, and the segment `*` to
//! the schema of `additionalProperties` and of `items`. The alternatives of a
//! node (`anyOf`, `oneOf`, `allOf`) and the definition that its `$ref` names in
//! the schema's own `$defs` are nodes at the same path.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use super::{KeyMark, MARKS_KEYWORD, Mark};

/// The keywords whose values are data, not schemas.
const DATA_KEYWORDS: [&str; 4] = ["default", "const", "enum", "examples"];

/// The keywords whose values are maps from a name to a schema.
const SCHEMA_MAPS: [&str; 5] = ["properties", "patternProperties", "$defs", "definitions", "dependentSchemas"];

/// The keywords that hold alternatives: each is a node at the same path.
const ALTERNATIVES: [&str; 3] = ["anyOf", "oneOf", "allOf"];

/// The name of the definition that `node` refers to with a `$ref` into the
/// schema's own `$defs`, if it does.
fn local_ref(node: &Value) -> Option<String> {
    ref_name(node.get("$ref")?.as_str()?)
}

/// The name of the definition that `reference`, the value of a `$ref`, names
/// in the schema's own `$defs`, if it does.
fn ref_name(reference: &str) -> Option<String> {
    let name = reference.strip_prefix("#/$defs/")?;
    Some(name.replace("~1", "/").replace("~0", "~"))
}

/// `segment` escaped for a JSON pointer.
fn escape(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}

/// The JSON pointer of the definition `name` in a schema's `$defs`.
fn def_pointer(name: &str) -> String {
    format!("/$defs/{}", escape(name))
}

/// A mistake in the marks of a schema given as data.
pub(super) enum Fault {
    /// `x-flm-marks` at the path is not a list of texts.
    NotAList(String),
    /// The text at the path names no mark.
    Unknown(String, String),
    /// The `default` of an object at the path marked
    /// [`Mark::MergeObjectKeyValues`] is not a map.
    DefaultNotAMap(String),
    /// `x-flm-marks` at the JSON pointer lies at no settings path: the root of
    /// the schema, or a node that no property, `additionalProperties`, `items`,
    /// alternative or reference reaches.
    Outside(String),
    /// A mark at the path was read while the definition, named second, was
    /// being read, and that definition refers to itself: the mark stands at
    /// every depth of the definition, which no path names.
    Recursive(String, String),
}

/// The marks written on one node of a schema given as data.
pub(super) struct ReadMark {
    /// The node's path, its segments joined by `.`.
    pub path: String,
    /// The marks.
    pub marks: Vec<Mark>,
    /// The `default` of the node, for a node marked
    /// [`Mark::MergeObjectKeyValues`]: its default entries.
    pub default: Option<Map<String, Value>>,
}

/// The marks written on the nodes of `schema`, a part's schema given as data,
/// each with its path, in the order of the schema.
pub(super) fn read(schema: &Value) -> Result<Vec<ReadMark>, Fault> {
    let mut reader = Reader { defs: schema.get("$defs").and_then(Value::as_object), found: Vec::new(), visited: BTreeSet::new() };
    if let Some(Value::Object(properties)) = schema.get("properties") {
        for (key, property) in properties {
            let pointer = format!("/properties/{}", escape(key));
            reader.walk(property, pointer, &mut vec![key.clone()], &mut Vec::new())?;
        }
    }
    let mut written = Vec::new();
    marked_pointers(schema, String::new(), &mut written);
    if let Some(outside) = written.into_iter().find(|pointer| !reader.visited.contains(pointer)) {
        return Err(Fault::Outside(outside));
    }
    Ok(reader.found)
}

/// The walk of [`read`].
struct Reader<'s> {
    defs: Option<&'s Map<String, Value>>,
    found: Vec<ReadMark>,
    /// The JSON pointers of the nodes whose marks were read.
    visited: BTreeSet<String>,
}

impl<'s> Reader<'s> {
    /// Reads the marks of `node`, at the JSON pointer `pointer` and the path
    /// `path`, and of the nodes below it; `entered` holds the definitions
    /// entered on the way to `node`, each with whether it was met again while
    /// being read. A definition met again is not entered a second time, and
    /// one met again whose walk read a mark is a [`Fault::Recursive`].
    fn walk(
        &mut self,
        node: &'s Value,
        pointer: String,
        path: &mut Vec<String>,
        entered: &mut Vec<(String, bool)>,
    ) -> Result<(), Fault> {
        let Value::Object(object) = node else { return Ok(()) };
        if let Some(written) = object.get(MARKS_KEYWORD) {
            let at = path.join(".");
            let names = written.as_array().ok_or_else(|| Fault::NotAList(at.clone()))?;
            let mut marks = Vec::new();
            for name in names {
                let name = name.as_str().ok_or_else(|| Fault::NotAList(at.clone()))?;
                marks.push(Mark::from_name(name).ok_or_else(|| Fault::Unknown(at.clone(), name.to_string()))?);
            }
            let default = match object.get("default") {
                _ if !marks.contains(&Mark::MergeObjectKeyValues) => None,
                None => Some(Map::new()),
                Some(Value::Object(entries)) => Some(entries.clone()),
                Some(_) => return Err(Fault::DefaultNotAMap(at)),
            };
            self.visited.insert(pointer.clone());
            self.found.push(ReadMark { path: at, marks, default });
        }
        for keyword in ALTERNATIVES {
            if let Some(Value::Array(items)) = object.get(keyword) {
                for (index, item) in items.iter().enumerate() {
                    self.walk(item, format!("{pointer}/{keyword}/{index}"), path, entered)?;
                }
            }
        }
        if let Some(name) = local_ref(node) {
            if let Some(definition) = self.defs.and_then(|defs| defs.get(&name)) {
                if let Some(entry) = entered.iter_mut().find(|(entered_name, _)| *entered_name == name) {
                    entry.1 = true;
                } else {
                    let before = self.found.len();
                    entered.push((name.clone(), false));
                    self.walk(definition, def_pointer(&name), path, entered)?;
                    let recursive = entered.pop().is_some_and(|(_, met_again)| met_again);
                    if recursive && self.found.len() > before {
                        return Err(Fault::Recursive(self.found[before].path.clone(), name));
                    }
                }
            }
        }
        if let Some(Value::Object(properties)) = object.get("properties") {
            for (key, property) in properties {
                path.push(key.clone());
                self.walk(property, format!("{pointer}/properties/{}", escape(key)), path, entered)?;
                path.pop();
            }
        }
        for keyword in ["additionalProperties", "items"] {
            if let Some(inner) = object.get(keyword) {
                path.push("*".to_string());
                self.walk(inner, format!("{pointer}/{keyword}"), path, entered)?;
                path.pop();
            }
        }
        Ok(())
    }
}

/// Adds to `found` the JSON pointer of every node below `node` (at `pointer`)
/// that writes `x-flm-marks`.
fn marked_pointers(node: &Value, pointer: String, found: &mut Vec<String>) {
    match node {
        Value::Object(object) => {
            if object.contains_key(MARKS_KEYWORD) {
                found.push(pointer.clone());
            }
            for (keyword, value) in object {
                let at = format!("{pointer}/{}", escape(keyword));
                match (keyword.as_str(), value) {
                    (keyword, _) if DATA_KEYWORDS.contains(&keyword) || keyword == MARKS_KEYWORD => {}
                    (keyword, Value::Object(schemas)) if SCHEMA_MAPS.contains(&keyword) => {
                        for (name, schema) in schemas {
                            marked_pointers(schema, format!("{at}/{}", escape(name)), found);
                        }
                    }
                    (_, value) => marked_pointers(value, at, found),
                }
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                marked_pointers(item, format!("{pointer}/{index}"), found);
            }
        }
        _ => {}
    }
}

/// Removes `x-flm-marks` from every node of `node`.
fn strip(node: &mut Value) {
    match node {
        Value::Object(object) => {
            object.remove(MARKS_KEYWORD);
            for (keyword, value) in object.iter_mut() {
                match (keyword.as_str(), value) {
                    (keyword, _) if DATA_KEYWORDS.contains(&keyword) => {}
                    (keyword, Value::Object(schemas)) if SCHEMA_MAPS.contains(&keyword) => {
                        schemas.values_mut().for_each(strip);
                    }
                    (_, value) => strip(value),
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(strip),
        _ => {}
    }
}

/// Whether the path `path` reaches at least one node of `root`, an object
/// schema whose `properties` are the settings keys and whose `$defs` hold the
/// definitions its nodes refer to.
pub(super) fn reaches(root: &Value, path: &str) -> bool {
    let segments: Vec<&str> = path.split('.').collect();
    reaches_from(root, root, &segments, &mut Vec::new())
}

fn reaches_from(root: &Value, node: &Value, segments: &[&str], entered: &mut Vec<String>) -> bool {
    let Some((first, rest)) = segments.split_first() else { return !matches!(node, Value::Bool(false)) };
    let Value::Object(object) = node else { return false };
    let alternatives = ALTERNATIVES
        .iter()
        .filter_map(|keyword| object.get(*keyword).and_then(Value::as_array))
        .flatten()
        .any(|item| reaches_from(root, item, segments, entered));
    if alternatives {
        return true;
    }
    if let Some(name) = local_ref(node) {
        if !entered.contains(&name) {
            if let Some(definition) = root.pointer(&def_pointer(&name)) {
                entered.push(name);
                let found = reaches_from(root, definition, segments, entered);
                entered.pop();
                if found {
                    return true;
                }
            }
        }
    }
    let children: Vec<&Value> = if *first == "*" {
        ["additionalProperties", "items"].iter().filter_map(|keyword| object.get(*keyword)).collect()
    } else {
        object.get("properties").and_then(|properties| properties.get(*first)).into_iter().collect()
    };
    children.into_iter().any(|child| reaches_from(root, child, rest, &mut Vec::new()))
}

/// Writes `schema`, the schema of the entry at `path` (each segment the name
/// of an entry), at its place in `root`, a schema as [`reaches`] reads it:
/// down the path, each map above the entry gets the property of the entry's
/// name, made from its `additionalProperties` (a definition met on the way is
/// written out in place), and the entry's property is `schema`, whose `$defs`
/// join those of `root`, each definition named by the entry's path, a `.` and
/// its own name, and each `$ref` of `schema` to it renamed to match, so that
/// the definitions of two entries never meet. A path that leaves the schema
/// writes nothing.
pub(super) fn graft(root: &mut Value, path: &str, schema: &Value) {
    let segments: Vec<&str> = path.split('.').collect();
    let Some((key, below)) = segments.split_first() else { return };
    let mut schema = schema.clone();
    if let Some(Value::Object(definitions)) = schema.as_object_mut().and_then(|object| object.remove("$defs")) {
        let names: Vec<String> = definitions.keys().cloned().collect();
        refer_within(&mut schema, path, &names);
        if let Value::Object(object) = root {
            if let Value::Object(defs) = object.entry("$defs").or_insert_with(|| Value::Object(Map::new())) {
                for (name, mut definition) in definitions {
                    refer_within(&mut definition, path, &names);
                    defs.insert(format!("{path}.{name}"), definition);
                }
            }
        }
    }
    let mut pointer = format!("/properties/{}", escape(key));
    for (at, name) in below.iter().enumerate() {
        let mut entered = Vec::new();
        while let Some(name) = root.pointer(&pointer).and_then(local_ref).filter(|name| !entered.contains(name)) {
            let Some(Value::Object(mut inlined)) = root.pointer(&def_pointer(&name)).cloned() else { break };
            if let Some(Value::Object(own)) = root.pointer(&pointer) {
                for (keyword, value) in own {
                    if keyword != "$ref" {
                        inlined.insert(keyword.clone(), value.clone());
                    }
                }
            }
            if let Some(slot) = root.pointer_mut(&pointer) {
                *slot = Value::Object(inlined);
            }
            entered.push(name);
        }
        let Some(Value::Object(node)) = root.pointer_mut(&pointer) else { return };
        let child = if at + 1 == below.len() {
            schema.clone()
        } else {
            match node.get("properties").and_then(|properties| properties.get(*name)) {
                Some(existing) => existing.clone(),
                None => match node.get("additionalProperties") {
                    Some(Value::Object(inner)) => Value::Object(inner.clone()),
                    _ => Value::Object(Map::new()),
                },
            }
        };
        if let Value::Object(properties) = node.entry("properties").or_insert_with(|| Value::Object(Map::new())) {
            properties.insert(name.to_string(), child);
        }
        pointer = format!("{pointer}/properties/{}", escape(name));
    }
}

/// Renames each `$ref` of `node` to a definition of `names`, the definitions
/// of the entry at `path`, to the name [`graft`] gives that definition: the
/// entry's path, a `.` and the definition's name. The values of the data
/// keywords are left as written.
fn refer_within(node: &mut Value, path: &str, names: &[String]) {
    match node {
        Value::Object(object) => {
            let name = object.get("$ref").and_then(Value::as_str).and_then(ref_name);
            if let Some(name) = name.filter(|name| names.contains(name)) {
                object.insert("$ref".into(), Value::from(format!("#{}", def_pointer(&format!("{path}.{name}")))));
            }
            for (keyword, value) in object.iter_mut() {
                if keyword != "$ref" && !DATA_KEYWORDS.contains(&keyword.as_str()) {
                    refer_within(value, path, names);
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(|item| refer_within(item, path, names)),
        _ => {}
    }
}

/// Writes every mark of `marks` on its nodes in `root`, a schema as
/// [`reaches`] reads it, after removing every mark the schema holds. A node
/// marked [`Mark::MergeObjectKeyValues`] gets the default entries that
/// `defaults` gives for its path as its `default`.
///
/// Where the path of a mark passes through a definition of `$defs` that
/// another place of the schema refers to as well, and the marks below the two
/// places differ, the definition is written out in place along the path, so
/// that no mark reaches a place that its path does not name.
pub(super) fn print<'d>(root: &mut Value, marks: &[KeyMark], defaults: impl Fn(&str) -> Option<&'d Map<String, Value>>) {
    strip(root);
    let shared = shared(root, marks, &defaults);
    for mark in marks {
        let segments: Vec<&str> = mark.path.split('.').collect();
        let mut targets = Vec::new();
        place(root, String::new(), &segments, &shared, &mut Vec::new(), &mut targets);
        let names: Vec<Value> = mark.marks.iter().map(|mark| Value::from(mark.name())).collect();
        for target in targets {
            let Some(node) = root.pointer_mut(&target) else { continue };
            if *node == Value::Bool(true) {
                *node = Value::Object(Map::new());
            }
            let Value::Object(object) = node else { continue };
            if mark.has(Mark::MergeObjectKeyValues) {
                if let Some(entries) = defaults(&mark.path) {
                    object.insert("default".into(), Value::Object(entries.clone()));
                }
            }
            object.insert(MARKS_KEYWORD.into(), Value::Array(names.clone()));
        }
    }
}

/// Adds to `targets` the JSON pointer of every node of `root` at `segments`
/// below the node at `pointer`, writing out in place each definition that the
/// walk passes through and that `shared` does not hold; `entered` holds the
/// definitions entered at this path.
fn place(
    root: &mut Value,
    pointer: String,
    segments: &[&str],
    shared: &BTreeSet<String>,
    entered: &mut Vec<String>,
    targets: &mut Vec<String>,
) {
    let Some(node) = root.pointer(&pointer) else { return };
    let Some((first, rest)) = segments.split_first() else {
        if !matches!(node, Value::Bool(false)) {
            targets.push(pointer);
        }
        return;
    };
    if let Some(name) = local_ref(node).filter(|name| !entered.contains(name)) {
        if let Some(definition) = root.pointer(&def_pointer(&name)).cloned() {
            entered.push(name.clone());
            if !shared.contains(&name) {
                // The definition, written out in place: the place's own keywords
                // (its description) beside those of the definition.
                let mut inlined = match definition {
                    Value::Object(object) => object,
                    _ => Map::new(),
                };
                if let Some(Value::Object(own)) = root.pointer(&pointer) {
                    for (keyword, value) in own {
                        if keyword != "$ref" {
                            inlined.insert(keyword.clone(), value.clone());
                        }
                    }
                }
                if let Some(slot) = root.pointer_mut(&pointer) {
                    *slot = Value::Object(inlined);
                }
                place(root, pointer, segments, shared, entered, targets);
                entered.pop();
                return;
            }
            place(root, def_pointer(&name), segments, shared, entered, targets);
            entered.pop();
        }
    }
    let Some(Value::Object(object)) = root.pointer(&pointer) else { return };
    let mut alternatives = Vec::new();
    for keyword in ALTERNATIVES {
        if let Some(Value::Array(items)) = object.get(keyword) {
            alternatives.extend((0..items.len()).map(|index| format!("{pointer}/{keyword}/{index}")));
        }
    }
    let children: Vec<String> = if *first == "*" {
        ["additionalProperties", "items"]
            .iter()
            .filter(|keyword| object.contains_key(**keyword))
            .map(|keyword| format!("{pointer}/{keyword}"))
            .collect()
    } else if object.get("properties").and_then(|properties| properties.get(*first)).is_some() {
        vec![format!("{pointer}/properties/{}", escape(first))]
    } else {
        Vec::new()
    };
    for alternative in alternatives {
        place(root, alternative, segments, shared, entered, targets);
    }
    for child in children {
        place(root, child, rest, shared, &mut Vec::new(), targets);
    }
}

/// The marks below one place of a definition, relative to the place: each
/// mark's path below the place, its marks and its default entries.
type Below<'m> = Vec<(String, &'m [Mark], Option<&'m Map<String, Value>>)>;

/// The definitions of `root` whose places all have the same marks below them
/// (none, for a place below which no mark lies): the marks below a place can
/// then be written in the definition itself.
fn shared<'d>(
    root: &Value,
    marks: &[KeyMark],
    defaults: &impl Fn(&str) -> Option<&'d Map<String, Value>>,
) -> BTreeSet<String> {
    let paths: Vec<Vec<&str>> = marks.iter().map(|mark| mark.path.split('.').collect()).collect();
    let mut places: BTreeMap<String, Vec<Option<Vec<String>>>> = BTreeMap::new();
    if let Some(Value::Object(properties)) = root.get("properties") {
        for (key, node) in properties {
            enter(root, node, vec![key.clone()], &paths, &mut Vec::new(), &mut places);
        }
    }
    let below = |place: &Option<Vec<String>>| -> Below<'_> {
        let Some(place) = place else { return Vec::new() };
        marks
            .iter()
            .zip(&paths)
            .filter(|(_, path)| path.len() > place.len() && path.iter().zip(place).all(|(a, b)| a == b))
            .map(|(mark, path)| (path[place.len()..].join("."), mark.marks.as_slice(), defaults(&mark.path)))
            .collect()
    };
    places
        .into_iter()
        .filter(|(_, places)| places.windows(2).all(|pair| below(&pair[0]) == below(&pair[1])))
        .map(|(name, _)| name)
        .collect()
}

/// Records in `places` each place, at `path`, of every definition that `node`
/// refers to, or that the definitions it refers to refer to; `None` for a
/// place below which no path of `paths` lies. `entered` holds the
/// definitions entered on the way, each entered once.
fn enter(
    root: &Value,
    node: &Value,
    path: Vec<String>,
    paths: &[Vec<&str>],
    entered: &mut Vec<String>,
    places: &mut BTreeMap<String, Vec<Option<Vec<String>>>>,
) {
    let marked_below =
        paths.iter().any(|marked| marked.len() > path.len() && marked.iter().zip(&path).all(|(a, b)| a == b));
    if !marked_below {
        let mut found = Vec::new();
        refs(node, false, &mut found);
        let mut reached = BTreeSet::new();
        while let Some(name) = found.pop() {
            if reached.insert(name.clone()) {
                if let Some(definition) = root.pointer(&def_pointer(&name)) {
                    refs(definition, false, &mut found);
                }
                places.entry(name).or_default().push(None);
            }
        }
        return;
    }
    let Value::Object(object) = node else { return };
    if let Some(name) = local_ref(node) {
        places.entry(name.clone()).or_default().push(Some(path.clone()));
        if let Some(definition) = root.pointer(&def_pointer(&name)).filter(|_| !entered.contains(&name)) {
            entered.push(name);
            enter(root, definition, path.clone(), paths, entered, places);
            entered.pop();
        }
    }
    for keyword in ALTERNATIVES {
        for item in object.get(keyword).and_then(Value::as_array).into_iter().flatten() {
            enter(root, item, path.clone(), paths, entered, places);
        }
    }
    if let Some(Value::Object(properties)) = object.get("properties") {
        for (key, property) in properties {
            let mut below = path.clone();
            below.push(key.clone());
            enter(root, property, below, paths, entered, places);
        }
    }
    for keyword in ["additionalProperties", "items"] {
        if let Some(inner) = object.get(keyword) {
            let mut below = path.clone();
            below.push("*".to_string());
            enter(root, inner, below, paths, entered, places);
        }
    }
}

/// Adds to `found` the definition named by every `$ref` of `node`; `map` says
/// that `node` is a map from a name to a schema.
fn refs(node: &Value, map: bool, found: &mut Vec<String>) {
    match node {
        Value::Object(object) if map => object.values().for_each(|schema| refs(schema, false, found)),
        Value::Object(object) => {
            if let Some(name) = local_ref(node) {
                found.push(name);
            }
            for (keyword, value) in object {
                match keyword.as_str() {
                    keyword if DATA_KEYWORDS.contains(&keyword) || keyword == "$defs" => {}
                    keyword => refs(value, SCHEMA_MAPS.contains(&keyword), found),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| refs(item, false, found)),
        _ => {}
    }
}
