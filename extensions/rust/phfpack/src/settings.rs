//! The settings of the feature `phfqit`: the macros declared in addition to
//! the built-in ones, and the named values (texts held under a key and an entry
//! name, such as `spaceKets.Bar`, which the macros' templates insert with
//! `#[spaceKets.Bar]{config}`).
//!
//! Settings are a feature's configuration written as data, one map per feature
//! from key to value. A document writes the settings of `phfqit` in its front
//! matter under `flm`, `features`, `phfqit`; the crate documentation gives an
//! example. The feature's `register` receives the final configuration, combined
//! from every source of settings, as a JSON value.
//!
//! The module holds two forms of the settings:
//!
//! - [`PhfqitData`] is the settings as written in data, with the types under it
//!   ([`MacroData`], [`MacroFields`], [`MacroArgList`], [`MacroArgEntry`],
//!   [`SlotName`], [`Content`]). Their documentation is also the description
//!   of each key in the settings' JSON schema ([`schema`]). Each argument of a
//!   declared macro is the guest client's `ArgDef`, which reads, writes and
//!   describes an argument in the form of a user definition's `args`: an
//!   argument code, or an argument's map with a code or a structured syntax.
//!   The arguments of a delimited macro are the guest client's
//!   `ArgListData`, which reads, writes and describes an argument list in the
//!   form of a user definition's `args`. [`PhfqitData::from_settings`] reads
//!   them from the configuration value.
//! - [`PhfqitConfig`] is the feature's configuration: the declared macros as the
//!   feature registers them and the named values. [`PhfqitConfig::from_data`]
//!   converts the data into it.
//!
//! The feature's configuration reader, [`read`], reads the configuration in
//! both forms when the host reads a document's settings: a value it cannot
//! read is reported there, by its path below the feature's name
//! (`macro_definitions.abs.args`, `spaceKets.Bar`), where the document wrote
//! it. `register` reads the configuration the reader checked the same way.

use std::collections::BTreeMap;
use std::fmt;

use flm_ext::ExtError;
use flm_ext::guest::core::settings::ReadError;
use flm_ext::guest::lang::mint::{ArgDef, ArgListData};
use flm_ext::guest::lang::rewrites::userdefs::{Definition, DefinitionKind};
use serde::de::value::{MapAccessDeserializer, StrDeserializer};
use serde::de::{Error as _, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

use crate::feature::FEATURE;
use crate::macros::{Macro, MacroArg};
use crate::sizing::SlotKind;
use crate::tables::VALUES;

/// The settings of the feature `phfqit`, as written in data.
///
/// When a program combines several sources of settings, each of the four keys
/// is combined entry by entry, the defaults included: a document that sets one
/// entry keeps every other entry. A template reads a named value with the placeholder
/// `#[key.entry]{config}`: `#[spaceKets.Bar]{config}` reads the entry `Bar` of
/// the key `spaceKets`.
#[derive(Debug, Clone, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct PhfqitData {
    /// The macros declared in addition to the built-in ones: a map from a
    /// macro's name, without the backslash, to its declaration. A declaration
    /// is either a text, the template of a macro that takes no argument, or a
    /// map with the key `content` (the template) and either `delimited_args`
    /// (for a macro whose arguments are written between sized delimiters) or
    /// `args` (for any other list of arguments). A declared macro with the name
    /// of a built-in macro replaces it.
    pub macro_definitions: BTreeMap<String, MacroData>,
    /// The spacing around the bar of `\braket`, `\matrixel` and `\dmatrixel`
    /// (the entry `Bar`, by default `\mkern 1.5mu `), and between the angle
    /// brackets of `\ketbra` and `\proj` (the entry `RLAngle`, by default
    /// `\mkern -1.8mu `), each written as formula text.
    #[serde(rename = "spaceKets")]
    pub space_kets: BTreeMap<String, String>,
    /// The same spacing for the macros with double angle brackets: around the
    /// bar of `\obraket`, `\omatrixel` and `\odmatrixel` (`Bar`), and between
    /// the angle brackets of `\oketbra` and `\oproj` (`RLAngle`), with the same
    /// defaults.
    #[serde(rename = "spaceOKets")]
    pub space_okets: BTreeMap<String, String>,
    /// The spacing before the comma of `\innerprod` (the entry `BeforeComma`, by
    /// default nothing) and after it (the entry `AfterComma`, by default
    /// `\mkern 1.5mu `).
    pub space: BTreeMap<String, String>,
}

impl PhfqitData {
    /// Returns the three keys that hold named values, each with its map of
    /// entries.
    fn value_groups(&self) -> [(&'static str, &BTreeMap<String, String>); 3] {
        [("spaceKets", &self.space_kets), ("spaceOKets", &self.space_okets), ("space", &self.space)]
    }

    /// Returns the bytes that the feature gives as an input of the registry
    /// digest (`FeatureBuilder::digest_input`): the settings as data, written
    /// in a binary form in which every list and every text is preceded by its
    /// count, a little-endian 64-bit number. Two settings that can render
    /// differently give different bytes; the keys of every map are written in
    /// their sorted order, so the bytes do not depend on the order in which a
    /// document writes them.
    pub fn digest_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        // The data types serialize to a JSON value whatever they hold: every
        // map key is a text.
        write_value(&mut out, &serde_json::to_value(self).unwrap_or_default());
        out
    }
}

impl Default for PhfqitData {
    fn default() -> PhfqitData {
        let group = |name: &str| -> BTreeMap<String, String> {
            VALUES
                .iter()
                .filter(|(group, _)| *group == name)
                .flat_map(|(_, values)| values.iter())
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect()
        };
        PhfqitData {
            macro_definitions: BTreeMap::new(),
            space_kets: group("spaceKets"),
            space_okets: group("spaceOKets"),
            space: group("space"),
        }
    }
}

/// Writes `value` to `out` in the binary form of [`PhfqitData::digest_bytes`]:
/// a tag byte for the kind of value, then the value, each list, map and text
/// preceded by its count.
fn write_value(out: &mut Vec<u8>, value: &Value) {
    let count = |out: &mut Vec<u8>, count: usize| out.extend_from_slice(&(count as u64).to_le_bytes());
    let text = |out: &mut Vec<u8>, text: &str| {
        count(out, text.len());
        out.extend_from_slice(text.as_bytes());
    };
    match value {
        Value::Null => out.push(0),
        Value::Bool(flag) => out.extend_from_slice(&[1, u8::from(*flag)]),
        Value::Number(number) => {
            out.push(2);
            text(out, &number.to_string());
        }
        Value::String(string) => {
            out.push(3);
            text(out, string);
        }
        Value::Array(items) => {
            out.push(4);
            count(out, items.len());
            for item in items {
                write_value(out, item);
            }
        }
        Value::Object(map) => {
            out.push(5);
            count(out, map.len());
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by_key(|(key, _)| *key);
            for (key, item) in entries {
                text(out, key);
                write_value(out, item);
            }
        }
    }
}

/// Returns the default entries of the named-value key `key` (`spaceKets`,
/// `spaceOKets` or `space`) as data: the default entries that the settings
/// declaration gives for the key, combined entry by entry.
pub fn default_values(key: &str) -> Value {
    let data = PhfqitData::default();
    let entries = data
        .value_groups()
        .into_iter()
        .filter(|(group, _)| *group == key)
        .flat_map(|(_, values)| values.clone())
        .map(|(name, value)| (name, Value::String(value)))
        .collect();
    Value::Object(entries)
}

/// Returns the JSON schema of the settings ([`PhfqitData`]), which the
/// feature's settings declaration gives as the schema of its configuration.
pub fn schema() -> Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(PhfqitData)).unwrap_or_default();
    if let Value::Object(map) = &mut schema {
        // The schema of one feature's configuration, placed in the schema of a
        // whole front matter: no dialect of its own.
        map.remove("$schema");
    }
    schema
}

/// The feature's configuration reader
/// ([`Feature::read`](flm_ext::guest::Feature::read)): reads `settings`, the
/// configuration written, as `register` reads it
/// ([`PhfqitData::from_settings`], then [`PhfqitConfig::from_data`]), and
/// answers it unchanged.
///
/// # Errors
///
/// The value at fault, by its path below the feature's name, and the
/// condition it violates: as [`PhfqitData::from_settings`] and
/// [`PhfqitConfig::from_data`] state them.
pub fn read(settings: Value) -> Result<Value, ReadError> {
    PhfqitConfig::from_data(PhfqitData::from_settings(&settings)?)?;
    Ok(settings)
}

/// The error with which `register` fails for the value at fault `error`,
/// worded as the host reports the refusal of the configuration reader
/// ([`read`]).
pub fn refused(error: ReadError) -> ExtError {
    let path = if error.path.is_empty() { FEATURE.to_string() } else { format!("{FEATURE}.{}", error.path) };
    ExtError::Handler { detail: format!("the value of `{path}` is invalid: {}", error.detail), data: Value::Null }
}

impl PhfqitData {
    /// Reads the settings as data from `settings`, the configuration value the
    /// feature's reader and its `register` receive. Each key, and each entry
    /// under it, is read on its own, so that a value that cannot be read is
    /// reported with its path (`spaceKets.Bar`), and a key of a declaration
    /// under `macro_definitions` with the macro's name and the key
    /// (`macro_definitions.abs.args`). A key not written keeps its default
    /// entries.
    ///
    /// # Errors
    ///
    /// The value at fault when the value is not of the shape of
    /// [`PhfqitData`]: a key that is not one of the four, a key's value that
    /// is not a map, a named value that is not a text, a declaration that is
    /// neither a text nor a map, a key of a declaration's map that cannot be
    /// read (an argument given by a code that is not one, its position from
    /// 1 written first: `argument 1: …`).
    pub fn from_settings(settings: &Value) -> Result<PhfqitData, ReadError> {
        let Value::Object(keys) = settings else {
            return Err(fault("", format!("must be a map, not `{settings}`")));
        };
        let mut data = PhfqitData::default();
        for (key, value) in keys {
            let entries = match key.as_str() {
                "macro_definitions" => {
                    data.macro_definitions = read_declarations(value)?;
                    continue;
                }
                "spaceKets" => &mut data.space_kets,
                "spaceOKets" => &mut data.space_okets,
                "space" => &mut data.space,
                _ => return Err(fault("", format!("the feature `{FEATURE}` has no setting `{key}`"))),
            };
            *entries = read_values(key, value)?;
        }
        Ok(data)
    }
}

/// Reads the macro declarations of `value`, the value of `macro_definitions`,
/// each declaration on its own; `null` declares none.
///
/// # Errors
///
/// The value at fault when `value` is not a map, or when a declaration cannot
/// be read ([`declaration_fault`]).
fn read_declarations(value: &Value) -> Result<BTreeMap<String, MacroData>, ReadError> {
    let declarations = match value {
        Value::Null => return Ok(BTreeMap::new()),
        Value::Object(declarations) => declarations,
        other => return Err(fault("macro_definitions", format!("must be a map, not `{other}`"))),
    };
    let mut declared = BTreeMap::new();
    for (name, declaration) in declarations {
        let data = MacroData::deserialize(declaration)
            .map_err(|error| declaration_fault(name, declaration, error.to_string()))?;
        declared.insert(name.clone(), data);
    }
    Ok(declared)
}

/// The fault of `declaration`, the declaration of the macro `name`, which
/// cannot be read with the error `detail`: the first key of the
/// declaration's map that cannot be read on its own, with its own error
/// (`macro_definitions.abs.args`); the declaration itself otherwise
/// (`macro_definitions.abs`), with `detail`.
fn declaration_fault(name: &str, declaration: &Value, detail: String) -> ReadError {
    let path = format!("macro_definitions.{name}");
    if let Value::Object(fields) = declaration {
        for (key, value) in fields {
            let alone = Value::Object([(key.clone(), value.clone())].into_iter().collect());
            if let Err(error) = MacroFields::deserialize(&alone) {
                return fault(format!("{path}.{key}"), error.to_string());
            }
        }
    }
    fault(path, detail)
}

/// Reads the named values of `value`, the value of the key `key` (`spaceKets`,
/// `spaceOKets` or `space`): a map from an entry's name to its text.
///
/// # Errors
///
/// The value at fault: the key's value when it is not a map, an entry
/// (`spaceKets.Bar`) when it is not a text.
fn read_values(key: &str, value: &Value) -> Result<BTreeMap<String, String>, ReadError> {
    let Value::Object(entries) = value else {
        return Err(fault(key, format!("must be a map, not `{value}`")));
    };
    entries
        .iter()
        .map(|(name, text)| match text {
            Value::String(text) => Ok((name.clone(), text.clone())),
            other => Err(fault(format!("{key}.{name}"), format!("must be a text, not `{other}`"))),
        })
        .collect()
}

/// The value at fault at `path` below the feature's name, violating the
/// condition `detail`.
fn fault(path: impl Into<String>, detail: impl Into<String>) -> ReadError {
    ReadError { path: path.into(), detail: detail.into() }
}

/// One macro declaration under `macro_definitions`: a text, or a map.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(untagged)]
pub enum MacroData {
    /// A text: the template of a macro that takes no argument. The macro is
    /// declared for running text and for formulas, and is available in each of
    /// the two where the template parses.
    Template(String),
    /// A map that declares the macro's template and arguments.
    Declared(MacroFields),
}

// A declaration is a text or a map, and is read as the one or the other, so
// that a problem inside the map is reported with its own message.
impl<'de> Deserialize<'de> for MacroData {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<MacroData, D::Error> {
        struct Declaration;

        impl<'de> Visitor<'de> for Declaration {
            type Value = MacroData;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a template or a map")
            }

            fn visit_str<E: serde::de::Error>(self, template: &str) -> Result<MacroData, E> {
                Ok(MacroData::Template(template.to_string()))
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<MacroData, A::Error> {
                MacroFields::deserialize(MapAccessDeserializer::new(map)).map(MacroData::Declared)
            }
        }

        deserializer.deserialize_any(Declaration)
    }
}

/// A macro declaration under `macro_definitions`, written as a map.
#[derive(Debug, Clone, Default, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct MacroFields {
    /// The arguments of a delimited macro, written as the `args` of a user
    /// definition of the `userdefs` feature: one text whose characters are
    /// argument codes, or a list of argument codes and maps. The macro takes
    /// the three ways of giving a size first (a size after a backtick, a star,
    /// a size in brackets), then these arguments; `#1` is the first of these
    /// arguments.
    pub delimited_args: Option<ArgListData>,
    /// The arguments, in the order a use writes them: one text whose characters
    /// are argument codes (the argument codes of the `userdefs` feature's
    /// `args`, such as `{` for an argument in braces and `[` for an optional
    /// argument in brackets), or a list whose entries are each an argument code,
    /// an argument's map as in the `userdefs` feature's `args` (the argument's
    /// syntax, as a structured syntax or a code, with its name and its other
    /// keys), or one of the three ways of giving a size, `SizeArgBacktick` (a
    /// size after a backtick), `SizeArgStar` (a star) and `SizeArgOptArg` (a
    /// size in brackets).
    pub args: Option<MacroArgList>,
    /// How many leading arguments the numbered placeholders skip: with an
    /// offset of 1, `#1` is the second argument. For a delimited macro the
    /// count starts at the first argument of `delimited_args`.
    pub argument_number_offset: usize,
    /// The template, written as a user definition's template is: either one
    /// text, declared for running text and for formulas and available in each
    /// of the two where it parses, or a map with the keys `text` and `math`, a template for running text and one
    /// for formulas, one or both given. A macro declared without a template is
    /// reported when the registry is built, and each use of it is an Error.
    pub content: Option<Content>,
    /// The default values of optional arguments, each keyed by the argument's
    /// name or by the number a template writes it as (`1` for `#1`).
    pub defaults: BTreeMap<String, String>,
    /// The macro's documentation, for a person reading the registry's
    /// description of the macro.
    pub docs: String,
}

/// Reads every entry of a list of arguments, each with its own reading; the
/// message of an entry that cannot be read names its position (`argument 2:
/// …`), as the message of an argument code in one text of codes does
/// ([`ArgListData::arg_defs`]).
fn entries<'de, A: SeqAccess<'de>, T: Deserialize<'de>>(mut seq: A) -> Result<Vec<T>, A::Error> {
    let mut entries = Vec::new();
    while let Some(entry) =
        seq.next_element().map_err(|error| A::Error::custom(format!("argument {}: {error}", entries.len() + 1)))?
    {
        entries.push(entry);
    }
    Ok(entries)
}

/// The arguments of a macro declaration's `args`: one text of argument codes,
/// or a list.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(untagged)]
pub enum MacroArgList {
    /// One text in which each character is one argument code.
    Codes(String),
    /// A list, each entry one argument.
    List(Vec<MacroArgEntry>),
}

// A list is a text or a list, and is read as the one or the other, so that a
// problem in one of its entries is reported with its own message, at the
// entry's position.
impl<'de> Deserialize<'de> for MacroArgList {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<MacroArgList, D::Error> {
        struct List;

        impl<'de> Visitor<'de> for List {
            type Value = MacroArgList;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a text of argument codes or a list of arguments")
            }

            fn visit_str<E: serde::de::Error>(self, codes: &str) -> Result<MacroArgList, E> {
                Ok(MacroArgList::Codes(codes.to_string()))
            }

            fn visit_seq<A: SeqAccess<'de>>(self, seq: A) -> Result<MacroArgList, A::Error> {
                entries(seq).map(MacroArgList::List)
            }
        }

        deserializer.deserialize_any(List)
    }
}

/// One entry of a list of arguments under `args`: a way of giving a size, or an
/// ordinary argument.
//
// The slot comes first: a text that is the name of a way of giving a size is
// that way, and any other text an argument code. An entry is read once and its
// argument moved into the macro's arguments, so the size of the argument's
// variant costs nothing; boxing it would only complicate the reading.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(untagged)]
pub enum MacroArgEntry {
    /// One of the three ways of giving a size.
    Slot(SlotName),
    /// An argument code, or an argument's map: the argument's syntax, as a
    /// structured syntax or a code, with its name and its other keys.
    Arg(ArgDef),
}

// An entry is a text, the name of a way of giving a size or else an argument
// code, or an argument's map, and is read as the one or the other, so that a
// problem in the argument is reported with the argument's own message.
impl<'de> Deserialize<'de> for MacroArgEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<MacroArgEntry, D::Error> {
        struct Entry;

        impl<'de> Visitor<'de> for Entry {
            type Value = MacroArgEntry;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a way of giving a size, an argument code or a map")
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<MacroArgEntry, E> {
                match SlotName::deserialize(StrDeserializer::<E>::new(text)) {
                    Ok(slot) => Ok(MacroArgEntry::Slot(slot)),
                    Err(_) => ArgDef::deserialize(StrDeserializer::<E>::new(text)).map(MacroArgEntry::Arg),
                }
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<MacroArgEntry, A::Error> {
                ArgDef::deserialize(MapAccessDeserializer::new(map)).map(MacroArgEntry::Arg)
            }
        }

        deserializer.deserialize_any(Entry)
    }
}

/// The three ways of giving a size, as an entry of a list of arguments under
/// `args`. Each is an argument of the macro through which a use asks for the
/// size of the macro's delimiters. In the examples, `\m` is the declared
/// macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, schemars::JsonSchema)]
pub enum SlotName {
    /// A size command after a backtick (``\m`\Big{x}``), or a star after a
    /// backtick (``\m`*{x}``).
    #[serde(rename = "SizeArgBacktick")]
    Backtick,
    /// A star (`\m*{x}`).
    #[serde(rename = "SizeArgStar")]
    Star,
    /// A size command or a star in brackets (`\m[\Big]{x}`, `\m[*]{x}`).
    #[serde(rename = "SizeArgOptArg")]
    Bracket,
}

impl From<SlotName> for SlotKind {
    fn from(name: SlotName) -> SlotKind {
        match name {
            SlotName::Backtick => SlotKind::Backtick,
            SlotName::Star => SlotKind::Star,
            SlotName::Bracket => SlotKind::Bracket,
        }
    }
}

/// The `content` key of a macro declaration: one template for both parsing
/// modes, or one template per mode.
#[derive(Debug, Clone, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
#[schemars(untagged)]
pub enum Content {
    /// One template for running text and for formulas; the macro is available
    /// in each mode where it parses.
    Both(String),
    /// One template per mode, at least one of the two given.
    ByMode(ModeTemplates),
}

impl<'de> Deserialize<'de> for Content {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Content, D::Error> {
        use serde::de::Error;
        match Value::deserialize(deserializer)? {
            Value::String(template) => Ok(Content::Both(template)),
            value @ Value::Object(_) => {
                let templates: ModeTemplates = serde_json::from_value(value).map_err(D::Error::custom)?;
                if templates.text.is_none() && templates.math.is_none() {
                    return Err(D::Error::custom("a `content` map must give `text`, `math` or both"));
                }
                Ok(Content::ByMode(templates))
            }
            other => Err(D::Error::custom(format!(
                "`content` must be a text or a map with the keys `text` and `math`, not `{other}`"
            ))),
        }
    }
}

/// The templates of a content given per mode ([`Content::ByMode`]); at least
/// one is given.
#[derive(Debug, Clone, Default, Deserialize, Serialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
#[schemars(extend("minProperties" = 1))]
pub struct ModeTemplates {
    /// The template for running text.
    #[schemars(with = "String")]
    pub text: Option<String>,
    /// The template for formulas.
    #[schemars(with = "String")]
    pub math: Option<String>,
}

impl Content {
    /// Returns the definitions that this content gives the definition `base`:
    /// one declared for both modes, or one per mode whose template is given
    /// (text mode first).
    fn definitions(self, base: Definition) -> Vec<Definition> {
        match self {
            Content::Both(template) => vec![base.content(template)],
            Content::ByMode(templates) => {
                let mut definitions = Vec::new();
                if let Some(template) = templates.text {
                    definitions.push(base.clone().text(template));
                }
                if let Some(template) = templates.math {
                    definitions.push(base.math(template));
                }
                definitions
            }
        }
    }
}

/// The configuration of the feature `phfqit`: the declared macros and the
/// named values, built from the settings as data ([`PhfqitConfig::from_data`]).
#[derive(Debug, Clone)]
pub struct PhfqitConfig {
    /// The declared macros, in the order of their names; a macro declared with
    /// a template per parsing mode gives one macro per mode. Each holds its user
    /// definition and the positions of its size arguments.
    pub declared: Vec<Macro>,
    /// The named values, each keyed as a template refers to it
    /// (`spaceKets.Bar`).
    pub values: BTreeMap<String, String>,
}

impl PhfqitConfig {
    /// Returns the configuration that the settings `data` declare: each
    /// declaration under `macro_definitions` converted to its macros, and the
    /// named values keyed as a template refers to them.
    ///
    /// # Errors
    ///
    /// The value at fault when a declaration cannot be converted: the
    /// declaration (`macro_definitions.abs`) when it gives both
    /// `delimited_args` and `args`, its argument list
    /// (`macro_definitions.abs.args`, `macro_definitions.abs.delimited_args`)
    /// when a character of the list written as one text is not an argument
    /// code.
    pub fn from_data(data: PhfqitData) -> Result<PhfqitConfig, ReadError> {
        let mut values = BTreeMap::new();
        for (group, entries) in data.value_groups() {
            for (key, value) in entries {
                values.insert(format!("{group}.{key}"), value.clone());
            }
        }
        let mut declared = Vec::new();
        for (name, declaration) in data.macro_definitions {
            declared.extend(declaration_macros(&name, declaration)?);
        }
        Ok(PhfqitConfig { declared, values })
    }
}

/// Returns the macros that the declaration of the macro `name` declares: one,
/// or one per parsing mode for a declaration with a template per mode.
///
/// # Errors
///
/// The value at fault, as [`PhfqitConfig::from_data`] states it.
fn declaration_macros(name: &str, declaration: MacroData) -> Result<Vec<Macro>, ReadError> {
    let path = format!("macro_definitions.{name}");
    let refused = |key: &str| {
        let path = format!("{path}.{key}");
        move |error: ExtError| fault(path, error.to_string())
    };
    let fields = match declaration {
        MacroData::Template(template) => {
            MacroFields { content: Some(Content::Both(template)), ..MacroFields::default() }
        }
        MacroData::Declared(fields) => fields,
    };
    let (args, offset) = match (fields.delimited_args, fields.args) {
        (Some(_), Some(_)) => {
            return Err(fault(path, "either `delimited_args` or `args` may be given, not both"));
        }
        (Some(list), None) => {
            let mut args: Vec<MacroArg> =
                [SlotKind::Backtick, SlotKind::Star, SlotKind::Bracket].map(MacroArg::Size).into();
            let leading = args.len();
            args.extend(list.arg_defs().map_err(refused("delimited_args"))?.into_iter().map(MacroArg::Arg));
            (args, leading + fields.argument_number_offset)
        }
        (None, Some(list)) => {
            let args = match list {
                MacroArgList::Codes(codes) => ArgListData::Codes(codes)
                    .arg_defs()
                    .map_err(refused("args"))?
                    .into_iter()
                    .map(MacroArg::Arg)
                    .collect(),
                MacroArgList::List(entries) => entries
                    .into_iter()
                    .map(|entry| match entry {
                        MacroArgEntry::Slot(slot) => MacroArg::Size(slot.into()),
                        MacroArgEntry::Arg(arg) => MacroArg::Arg(arg),
                    })
                    .collect(),
            };
            (args, fields.argument_number_offset)
        }
        (None, None) => (Vec::new(), fields.argument_number_offset),
    };
    let mut base = Macro::new(DefinitionKind::Macro, name, args);
    base.definition = base.definition.argument_number_offset(offset).docs(fields.docs);
    for (key, text) in fields.defaults {
        base.definition = base.definition.default(key, text);
    }
    Ok(match fields.content {
        None => vec![base],
        Some(content) => {
            let slots = base.slots;
            content
                .definitions(base.definition)
                .into_iter()
                .map(|definition| Macro { definition, slots: slots.clone() })
                .collect()
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_value_keys_are_the_groups_of_the_table_of_values() {
        let data = PhfqitData::default();
        let keys: Vec<&str> = data.value_groups().iter().map(|(group, _)| *group).collect();
        let groups: Vec<&str> = VALUES.iter().map(|(group, _)| *group).collect();
        assert_eq!(keys, groups);
        let config = PhfqitConfig::from_data(data).expect("the default settings declare no macro");
        assert_eq!(config.values.len(), 6);
    }

    #[test]
    fn two_configurations_whose_size_slots_stand_on_different_macros_digest_differently() {
        let declared = |args: &[MacroArgEntry]| {
            MacroData::Declared(MacroFields { args: Some(MacroArgList::List(args.to_vec())), ..MacroFields::default() })
        };
        let slot = [MacroArgEntry::Slot(SlotName::Backtick)];
        let with_slots = |first: &[MacroArgEntry], second: &[MacroArgEntry]| PhfqitData {
            macro_definitions: BTreeMap::from([
                (String::from("one"), declared(first)),
                (String::from("two"), declared(second)),
            ]),
            ..PhfqitData::default()
        };
        let on_first = with_slots(&slot, &[]);
        let on_second = with_slots(&[], &slot);
        assert_ne!(on_first.digest_bytes(), on_second.digest_bytes());
    }
}
