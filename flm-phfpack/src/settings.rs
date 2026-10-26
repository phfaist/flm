//! The settings of the feature `phfqit`: the macros declared in addition to
//! the built-in ones, and the named values (texts held under a key and an entry
//! name, such as `spaceKets.Bar`, which the macros' templates insert with
//! `#[spaceKets.Bar]{config}`).
//!
//! [Settings](flm_core::core::settings) are a feature's configuration and option
//! values written as data, one map per feature from key to value, which the
//! library reads into the feature's own Rust types. A program gives the
//! settings of `phfqit` under the feature's name ([`FEATURE`](crate::FEATURE));
//! with the `flm` program, a document writes them in its front matter under
//! `flm`, `features`, `phfqit`:
//!
//! ```yaml
//! ---
//! flm:
//!   use: [phfpack]
//!   features:
//!     phfqit:
//!       macro_definitions:
//!         opnorm:
//!           delimited_args: ['{']
//!           content: '#[\lVert]{DelimLeft}{#1}#[\rVert]{DelimRight}_\infty'
//!       spaceKets:
//!         Bar: '\mkern 2mu '
//! ---
//! ```
//!
//! This example declares the macro `\opnorm`, a delimited macro with one
//! argument (`\opnorm{A}`, ``\opnorm`\Big{A}``, `\opnorm*{A}`), and changes the
//! spacing around the bar of `\braket`, `\matrixel` and `\dmatrixel`.
//!
//! The module holds two forms of the settings:
//!
//! - [`PhfqitData`] is the settings as written in data, with the types under it
//!   ([`MacroData`], [`MacroFields`], [`MacroArgList`], [`MacroArgEntry`],
//!   [`SlotName`]). Their documentation is also the description of each key in
//!   the settings' JSON schema.
//! - [`PhfqitConfig`] is the feature's configuration: the plain Rust value the
//!   feature is constructed from ([`Phfqit::new`](crate::Phfqit::new)), fixed
//!   when the registry is built. `PhfqitConfig::try_from` converts the data into
//!   it, and fails with a [`DeclarationError`] when a declaration cannot be
//!   read.

use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};

use flm_core::lang::mint::{ArgData, ArgDef, ArgListData, ArgListError};
use flm_core::lang::rewrites::userdefs::DefinitionKind;
use flm_core::stdcore::features::userdefs::{DefinitionContent, UserdefsConfig};
use serde::de::value::{MapAccessDeserializer, StrDeserializer};
use serde::de::{MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer};

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
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
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

/// Returns the default entries of the named-value key `key` (`spaceKets`,
/// `spaceOKets` or `space`) as data: the default entries that the settings
/// declaration gives for the key ([`SettingsDecl::merge_entries`](flm_core::core::settings::SettingsDecl::merge_entries)).
pub(crate) fn default_values(key: &str) -> serde_json::Value {
    let data = PhfqitData::default();
    let entries = data
        .value_groups()
        .into_iter()
        .filter(|(group, _)| *group == key)
        .flat_map(|(_, values)| values.clone())
        .map(|(name, value)| (name, serde_json::Value::String(value)))
        .collect();
    serde_json::Value::Object(entries)
}

/// One macro declaration under `macro_definitions`: a text, or a map.
#[derive(Debug, Clone, schemars::JsonSchema)]
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
#[derive(Debug, Clone, Default, Deserialize, schemars::JsonSchema)]
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
    pub content: Option<DefinitionContent>,
    /// The default values of optional arguments, each keyed by the argument's
    /// name or by the number a template writes it as (`1` for `#1`).
    pub defaults: BTreeMap<String, String>,
    /// The macro's documentation, for a person reading the registry's
    /// description of the macro.
    pub docs: String,
}

/// The arguments of a macro declaration's `args`: one text of argument codes,
/// or a list.
#[derive(Debug, Clone, schemars::JsonSchema)]
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

            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<MacroArgList, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = seq.next_element()? {
                    entries.push(entry);
                }
                Ok(MacroArgList::List(entries))
            }
        }

        deserializer.deserialize_any(List)
    }
}

/// One entry of a list of arguments under `args`: a way of giving a size, or an
/// ordinary argument.
#[derive(Debug, Clone, schemars::JsonSchema)]
#[schemars(untagged)]
pub enum MacroArgEntry {
    /// One of the three ways of giving a size.
    Slot(SlotName),
    /// An argument code, or an argument's map: the argument's syntax, as a
    /// structured syntax or a code, with its name and its other keys.
    Arg(ArgData),
}

// An entry is a text, the name of a way of giving a size or else an argument
// code, or an argument's map, and is read as the one or the other, so that a
// problem inside the map is reported with its own message.
impl<'de> Deserialize<'de> for MacroArgEntry {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<MacroArgEntry, D::Error> {
        struct Entry;

        impl<'de> Visitor<'de> for Entry {
            type Value = MacroArgEntry;

            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a way of giving a size, an argument code or a map")
            }

            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<MacroArgEntry, E> {
                Ok(match SlotName::deserialize(StrDeserializer::<E>::new(text)) {
                    Ok(slot) => MacroArgEntry::Slot(slot),
                    Err(_) => MacroArgEntry::Arg(ArgData::Code(text.to_string())),
                })
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<MacroArgEntry, A::Error> {
                ArgData::deserialize(MapAccessDeserializer::new(map)).map(MacroArgEntry::Arg)
            }
        }

        deserializer.deserialize_any(Entry)
    }
}

/// The three ways of giving a size, as an entry of a list of arguments under
/// `args`. Each is an argument of the macro through which a use asks for the
/// size of the macro's delimiters. In the examples, `\m` is the declared
/// macro.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, schemars::JsonSchema)]
pub enum SlotName {
    /// A size command after a backtick (``\m`\Big{x}``), or a star after a
    /// backtick (``\m`*{x}``).
    SizeArgBacktick,
    /// A star (`\m*{x}`).
    SizeArgStar,
    /// A size command or a star in brackets (`\m[\Big]{x}`, `\m[*]{x}`).
    SizeArgOptArg,
}

impl From<SlotName> for SlotKind {
    fn from(name: SlotName) -> SlotKind {
        match name {
            SlotName::SizeArgBacktick => SlotKind::Backtick,
            SlotName::SizeArgStar => SlotKind::Star,
            SlotName::SizeArgOptArg => SlotKind::Bracket,
        }
    }
}

/// The configuration of the feature `phfqit`: the declared macros and the
/// named values. The feature is constructed from it
/// ([`Phfqit::new`](crate::Phfqit::new)) and it is fixed when the registry is
/// built.
///
/// The default value declares no macro and holds the default named values.
/// Its implementation of `TryFrom<PhfqitData>` builds it from the settings as
/// data ([`PhfqitData`]), and fails with a [`DeclarationError`].
#[derive(Debug, Clone)]
pub struct PhfqitConfig {
    /// The declared macros, in the order of their names; a macro declared with
    /// a template per parsing mode gives one macro per mode. Each holds its user
    /// definition ([`Definition`](flm_core::lang::rewrites::userdefs::Definition))
    /// and the positions of its size arguments.
    pub declared: Vec<Macro>,
    /// The named values, each keyed as a template refers to it
    /// (`spaceKets.Bar`).
    pub values: BTreeMap<String, String>,
}

impl Default for PhfqitConfig {
    fn default() -> PhfqitConfig {
        let values = VALUES
            .iter()
            .flat_map(|(group, values)| {
                values.iter().map(move |(key, value)| (format!("{group}.{key}"), value.to_string()))
            })
            .collect();
        PhfqitConfig { declared: Vec::new(), values }
    }
}

impl PhfqitConfig {
    /// Returns the bytes that the feature gives as an input of the [registry
    /// digest](flm_core::model::digest)
    /// ([`FeatureBuilder::digest_input`](flm_core::core::registry::FeatureBuilder::digest_input)):
    /// the named values, the positions and kinds of the size arguments of each
    /// declared macro, and the
    /// declared user definitions as the `userdefs` feature digests its own
    /// ([`UserdefsConfig::digest_bytes`]). Two configurations that can render
    /// differently give different bytes.
    pub fn digest_bytes(&self) -> Vec<u8> {
        // Every list is written with its count first, so that the bytes of
        // one part alone tell any two configurations apart.
        let mut out = Vec::new();
        let count =
            |out: &mut Vec<u8>, count: usize| out.extend_from_slice(&(count as u64).to_le_bytes());
        let put = |out: &mut Vec<u8>, text: &str| {
            count(out, text.len());
            out.extend_from_slice(text.as_bytes());
        };
        count(&mut out, self.values.len());
        for (name, value) in &self.values {
            put(&mut out, name);
            put(&mut out, value);
        }
        count(&mut out, self.declared.len());
        for declared in &self.declared {
            count(&mut out, declared.slots.len());
            for (index, kind) in &declared.slots {
                put(&mut out, &format!("{index}:{kind:?}"));
            }
        }
        let definitions = self.declared.iter().map(|declared| declared.definition.clone());
        out.extend(UserdefsConfig::new().definitions(definitions).digest_bytes());
        out
    }
}

/// Hashes the configuration's [digest bytes](PhfqitConfig::digest_bytes): a
/// configuration type must implement `Hash`
/// ([`Options`](flm_core::model::options::Options)).
impl Hash for PhfqitConfig {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.digest_bytes());
    }
}

/// The error of reading the settings of `phfqit`: a macro declaration under
/// `macro_definitions` that cannot be read. Its message identifies the macro
/// and states the problem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeclarationError {
    /// The macro's name, without the backslash.
    pub name: String,
    /// The problem, as a message for a person.
    pub detail: String,
}

impl fmt::Display for DeclarationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "in the declaration of the macro `\\{}` under `macro_definitions`: {}",
            self.name, self.detail
        )
    }
}

impl std::error::Error for DeclarationError {}

impl TryFrom<PhfqitData> for PhfqitConfig {
    type Error = DeclarationError;

    fn try_from(data: PhfqitData) -> Result<PhfqitConfig, DeclarationError> {
        let mut values = BTreeMap::new();
        for (group, entries) in data.value_groups() {
            for (key, value) in entries {
                values.insert(format!("{group}.{key}"), value.clone());
            }
        }
        let mut declared = Vec::new();
        for (name, declaration) in data.macro_definitions {
            let error = |detail: String| DeclarationError { name: name.clone(), detail };
            declared.extend(declaration_macros(&name, declaration).map_err(error)?);
        }
        Ok(PhfqitConfig { declared, values })
    }
}

/// Returns the macros that the declaration of the macro `name` declares: one,
/// or one per parsing mode for a declaration with a template per mode.
///
/// # Errors
///
/// A message for a person when the declaration gives both `delimited_args`
/// and `args`, or when an argument cannot be read: an argument code is
/// invalid, or an argument's map gives both a code and a structured syntax,
/// or neither.
fn declaration_macros(name: &str, declaration: MacroData) -> Result<Vec<Macro>, String> {
    let fields = match declaration {
        MacroData::Template(template) => MacroFields {
            content: Some(DefinitionContent::Both(template)),
            ..MacroFields::default()
        },
        MacroData::Declared(fields) => fields,
    };
    let (args, offset) = match (fields.delimited_args, fields.args) {
        (Some(_), Some(_)) => {
            return Err(String::from("either `delimited_args` or `args` may be given, not both"));
        }
        (Some(list), None) => {
            let mut args: Vec<MacroArg> =
                [SlotKind::Backtick, SlotKind::Star, SlotKind::Bracket].map(MacroArg::Size).into();
            let listed = list.arg_defs().map_err(|error| error.to_string())?;
            let leading = args.len();
            args.extend(listed.into_iter().map(MacroArg::Arg));
            (args, leading + fields.argument_number_offset)
        }
        (None, Some(list)) => (read_args(list)?, fields.argument_number_offset),
        (None, None) => (Vec::new(), fields.argument_number_offset),
    };
    let base = Macro::new(DefinitionKind::Macro, name, args).map_err(|error| error.to_string())?;
    let mut base = base;
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

/// Returns the arguments of an `args` list: each of the three ways of giving a
/// size by its name, and every other entry read as an argument, from its code
/// or its argument's map (the `TryFrom<ArgData>` implementation of
/// [`ArgDef`], and [`ArgListData::arg_defs`]).
///
/// # Errors
///
/// A message for a person, naming the entry's position, when an entry is not
/// an argument code or is a map that gives both a code and a structured
/// syntax, or neither.
fn read_args(list: MacroArgList) -> Result<Vec<MacroArg>, String> {
    match list {
        MacroArgList::Codes(codes) => Ok(ArgListData::Codes(codes)
            .arg_defs()
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(MacroArg::Arg)
            .collect()),
        MacroArgList::List(entries) => entries
            .into_iter()
            .enumerate()
            .map(|(index, entry)| match entry {
                MacroArgEntry::Slot(name) => Ok(MacroArg::Size(name.into())),
                MacroArgEntry::Arg(data) => ArgDef::try_from(data)
                    .map(MacroArg::Arg)
                    .map_err(|error| ArgListError { index, error }.to_string()),
            })
            .collect(),
    }
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
        assert_eq!(PhfqitConfig::default().values.len(), 6);
    }

    #[test]
    fn two_configurations_whose_size_slots_stand_on_different_macros_digest_differently() {
        let one = Macro::new(DefinitionKind::Macro, "one", Vec::new()).expect("a macro");
        let two = Macro::new(DefinitionKind::Macro, "two", Vec::new()).expect("a macro");
        let slot = vec![(0, SlotKind::Backtick)];
        let with_slots = |first: &[(usize, SlotKind)], second: &[(usize, SlotKind)]| PhfqitConfig {
            declared: vec![
                Macro { slots: first.to_vec(), ..one.clone() },
                Macro { slots: second.to_vec(), ..two.clone() },
            ],
            ..PhfqitConfig::default()
        };
        let on_first = with_slots(&slot, &[]);
        let on_second = with_slots(&[], &slot);
        assert_ne!(on_first.digest_bytes(), on_second.digest_bytes());
    }
}
