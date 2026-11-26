//! Settings are a feature's configuration and option values written as data, one
//! map per feature from key to value, which this module reads into the feature's
//! own Rust types; a settings key is one field of those types.
//!
//! A feature's configuration is the plain Rust value the feature is constructed
//! from; it is fixed when the registry is built and cannot vary per document or
//! per render ([settings, options and
//! configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//! An option type is a plain Rust struct a feature declares once, whose values
//! are set on a document or on a render request ([`crate::model::options`]).
//!
//! A program that does not construct these Rust values itself, such as a
//! command line or a program that reads a document's front matter, gives them as
//! data instead. This module reads that data. It adds no second definition of
//! any setting: the keys, their defaults and their documentation are those of
//! the Rust types, and the data is read into those types.
//!
//! # The declaration
//!
//! A feature that takes settings gives one [`SettingsDecl`]: the feature's name,
//! the Rust types its settings are read into, each with its
//! [binding](crate::model::options::Bind) (where a value may be set: fixed with
//! the registry, on the document, on the render request, or on either), and marks
//! on some keys or on values below them ([`Mark`], [`KeyMark`]). Every field of
//! those types is one settings key of the feature: the keys of the `html`
//! feature, for example, are the fields of its render options, `title`, `lang`,
//! `heading_tags` and so on. The declaration
//! lists the keys ([`SettingsDecl::keys`]), reads a map of them
//! ([`SettingsDecl::read`]) and describes them as a JSON Schema
//! ([`SettingsDecl::json_schema`]), with the descriptions taken from the Rust
//! documentation of the fields.
//!
//! A feature gives its declaration through
//! [`FlmFeature::settings`](crate::core::registry::FlmFeature::settings), and
//! nothing else states it again: the registry build declares the feature's
//! option types from it, each with its binding and its documentation, and a
//! package factory lists it beside the feature's name
//! ([`PackageFactory`](crate::core::registry::PackageFactory)).
//!
//! # Reading
//!
//! Data is a [`serde_json::Value`], the only data model this module reads; the
//! host is responsible for reading another notation (for example YAML) into it.
//! The types the data is read into implement [`serde::Deserialize`] and are
//! public, so that another program reads the same keys. Reading gives a
//! [`FeatureSettings`]: the feature's configuration (its default when no key of
//! it is written), and the option values of every option type at least one of
//! whose keys is written.
//!
//! A key the feature does not declare, and a value that does not have the shape
//! its key requires, are each reported as a [`SettingsError`] that identifies the
//! key by its path from the feature's name (`html.heading_tags.section`).
//! [`SettingsDecl::read`] receives only the data, not the file or the line in
//! which a value was written: a host, which records where each value was
//! written, adds that place when it reports the error.
//!
//! The key [`ENABLED_KEY`] (`enabled`) is reserved: a source of settings
//! writes it beside a feature's settings to switch the feature on or off, and
//! a declaration refuses a key of that name, as it refuses a key declared
//! twice.
//!
//! # Collections of named things
//!
//! A collection of named things, such as the xnote kinds or the float types, is
//! written in data as a map from a name to an entry, in the order written;
//! [`named`] provides the function that reads such a field
//! ([`named::deserialize`]). Its feature marks
//! the key as combined key by key ([`SettingsDecl::merge_entries`]), giving
//! its default entries as data ([`SettingsDecl::default_entries`]), and as one
//! in which an entry written `false` is removed
//! ([`SettingsDecl::remove_false`]). A host that combines several sources of
//! settings combines such a key one entry at a time, with the default entries
//! as the lowest source, instead of letting one source's value replace
//! another's whole. This module reads one map per feature, already combined.
//!
//! # Marks
//!
//! A mark ([`Mark`]) says how sources of settings combine a value, or what the
//! value means. It is written on the node of the JSON Schema that it concerns,
//! as the list of the mark names under [`MARKS_KEYWORD`] (`x-flm-marks`):
//! `{"type": "object", "x-flm-marks": ["merge-object-key-values",
//! "remove-key-when-value-is-false"]}`. The declaration keeps each mark with
//! the path of its node ([`KeyMark`]). A declaration written in Rust sets marks
//! by path, with one method per mark; a part declared from data writes them on
//! the nodes of its schema. Either way, the schema that
//! [`SettingsDecl::json_schema`] prints carries every mark on its node.
//!
//! # Settings declared from data
//!
//! A feature whose settings have no Rust types, such as a feature written in a
//! script, declares them from data instead: its configuration with
//! [`SettingsDecl::config_data`] and each option type with
//! [`SettingsDecl::options_data`], each from a JSON Schema of an object, with
//! the marks written on the nodes of that schema. Such a
//! part of the declaration is a *data part*; it lives in one declaration beside
//! the parts declared with Rust types, and everything above holds for it alike:
//! its keys are the `properties` of its schema, it is read, declared to the
//! registry and described as the others are.
//!
//! - **Its key.** The configuration is the option type named by the feature's
//!   name, and an option type the one named by the name the feature gives
//!   ([`OptionKey::Data`]). A stage reads
//!   it by that name
//!   ([`StageCx::option_named`](crate::core::run::StageCx::option_named), after
//!   [`Needs::option_named`](crate::core::stage::Needs::option_named)), and a
//!   package reads a configuration declared from data as a
//!   [`DataValue`] ([`PackageSettings::config`]).
//! - **Reading.** A key outside the schema's `properties` is an unknown key, as
//!   for a Rust type that refuses unknown fields. The values are kept as written,
//!   as a [`DataValue`]: they are not checked against the types the schema
//!   states, which is the feature's own task. The configuration read when none
//!   of its keys is written is the empty map.
//! - **A reader.** A feature that reads its configuration when the settings are
//!   read declares it with [`SettingsDecl::config_data_read`], giving the
//!   function that reads it: the configuration written goes in, and the value
//!   kept as the [`DataValue`] comes out, or a [`ReadError`] naming the value at
//!   fault, which is reported as the conversion errors of
//!   [`SettingsDecl::config_from`] are, at that value's path. This is the twin of
//!   reading a Rust type `D` and converting it with `C::try_from`.
//! - **The marks.** The declaration reads the marks from the nodes of the
//!   schema, each at its node's path ([`KeyMark`]): a property `K` of an
//!   object adds the segment `K`, `additionalProperties` and `items` add the
//!   segment `*`, and a `$ref` into the schema's own `$defs` is followed. The
//!   default entries of an object combined key by key are the `default` of its
//!   node, the empty map when it has none.
//! - **The schema.** [`SettingsDecl::json_schema`] takes the keys' schemas from
//!   the schema given, with the binding added as for a Rust type and the marks
//!   written on their nodes; the schema's own `$defs` join those of the result.
//! - **Mistakes.** The data constructors return a [`SettingsError`] of the kind
//!   [`SettingsErrorKind::InvalidDeclaration`] where the constructors taking a
//!   Rust type panic: their input comes from outside the program, and nothing it
//!   holds may stop the program.
//!
//! # Combining settings from several sources
//!
//! A host that reads settings from several sources, ranked (a program's
//! defaults, the packages a document uses, its front matter, a command line),
//! combines them with [`cascade`]: every value is recorded with where it was
//! written, the source of the highest rank decides each key, a package wins
//! over the packages it uses and its peers must agree, a key marked as combined
//! key by key is combined one entry at a time over its default entries, and the
//! result is read through the declarations above, each problem placed where the
//! value at fault was written. The host keeps its sources and the texts that
//! name them; the cascade reads no file.
//!
//! # The settings of a package
//!
//! A package (a bundle of features ready to register) is built from the settings
//! of all its features at once, gathered in a [`PackageSettings`]; see
//! [`PackageFactory`](crate::core::registry::PackageFactory).

use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, OnceLock};

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::model::names::{FeatureName, Name};
use crate::model::options::{Bind, DataValue, OptionKey, Options, OptionValues};

pub mod cascade;
mod marks;
pub mod named;

/// The key that switches a feature on or off in a source of settings
/// (`citations: {enabled: false}`), reserved in every feature's settings: a
/// [`SettingsDecl`] refuses a key of this name (the constructors that take a
/// Rust type panic, those that take data return a [`SettingsError`] of the
/// kind [`SettingsErrorKind::InvalidDeclaration`]). The host that reads the
/// sources of settings reads the key itself; the feature never receives it.
pub const ENABLED_KEY: &str = "enabled";

/// The bounds a Rust type must meet for settings to be read into it from data.
///
/// The trait is implemented for every type that implements both
/// [`serde::de::DeserializeOwned`], which reads the data, and
/// [`schemars::JsonSchema`], from which the schema of the settings is derived
/// (and is `Send + Sync + 'static`). A crate outside this library that declares
/// settings of its own lists `serde` and `schemars` as dependencies in its
/// manifest and derives both traits on its settings types.
pub trait SettingsData: DeserializeOwned + schemars::JsonSchema + Any + Send + Sync {}
impl<T: DeserializeOwned + schemars::JsonSchema + Any + Send + Sync> SettingsData for T {}

/// The keyword under which a node of a JSON Schema lists its marks
/// ([`Mark`]), as the names of the marks: `{"type": "object", "x-flm-marks":
/// ["merge-object-key-values", "remove-key-when-value-is-false"]}`.
pub const MARKS_KEYWORD: &str = "x-flm-marks";

/// A mark that a declaration sets on one of its settings, or on a value at any
/// depth below one: how sources of settings combine the value, or what the
/// value means.
///
/// A mark is written on the node of the JSON Schema that it concerns, under
/// [`MARKS_KEYWORD`] (`x-flm-marks`), as its name ([`Mark::name`]); see
/// [`KeyMark`] for the path of the node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Mark {
    /// `merge-object-key-values`, on an object: sources of settings are
    /// combined key by key. The `default` of the object's node is its default
    /// entries ([`SettingsDecl::default_entries`]).
    MergeObjectKeyValues,
    /// `remove-key-when-value-is-false`, on an object: a key whose value is
    /// `false` is removed.
    RemoveKeyWhenValueIsFalse,
    /// `set-to-default-with-null`, on one setting: a value of `null` gives the
    /// setting its default.
    SetToDefaultWithNull,
    /// `path`, on a text or on a value that holds texts: each text is the path
    /// of a file. The origin of the value gives each such text the form its
    /// reader receives ([`CascadeOrigin::resolve_path`](cascade::CascadeOrigin::resolve_path)).
    Path,
    /// `markup`, on a text or on a value that holds texts: each text is
    /// written in FLM markup, which the feature parses.
    Markup,
}

impl Mark {
    /// Every mark, in the order in which a node lists them.
    pub const ALL: [Mark; 5] =
        [Mark::MergeObjectKeyValues, Mark::RemoveKeyWhenValueIsFalse, Mark::SetToDefaultWithNull, Mark::Path, Mark::Markup];

    /// The mark's name, as a schema writes it under [`MARKS_KEYWORD`].
    pub fn name(self) -> &'static str {
        match self {
            Mark::MergeObjectKeyValues => "merge-object-key-values",
            Mark::RemoveKeyWhenValueIsFalse => "remove-key-when-value-is-false",
            Mark::SetToDefaultWithNull => "set-to-default-with-null",
            Mark::Path => "path",
            Mark::Markup => "markup",
        }
    }

    /// The mark named `name`, or `None` when no mark has that name.
    pub fn from_name(name: &str) -> Option<Mark> {
        Mark::ALL.into_iter().find(|mark| mark.name() == name)
    }
}

impl fmt::Display for Mark {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The marks ([`Mark`]) that a declaration sets on one of its settings, or on
/// a value at any depth below one.
///
/// A declaration written in Rust sets marks with
/// [`SettingsDecl::merge_entries`], [`SettingsDecl::remove_false`],
/// [`SettingsDecl::default_on_null`], [`SettingsDecl::markup`] and
/// [`SettingsDecl::path`]; a part declared from data writes them on the nodes
/// of its schema. A host reads them with [`SettingsDecl::marks`] and
/// [`SettingsDecl::mark_at`], and the JSON Schema writes each on its node
/// ([`SettingsDecl::json_schema`]).
///
/// The marked value is identified by its path below the feature's name, its
/// segments joined by `.`. The segment `*` means every entry of a map and every
/// item of a list (`kinds.*.words`, a key of the theorems feature, marks the
/// `words` of every theorem kind, `labels.*`
/// every item of the list `labels`); a variant of an enumeration written as a
/// map of one key is a segment of its own (`nesting.template`). In the schema,
/// a property `K` of an object is the segment `K`, `additionalProperties` and
/// `items` are the segment `*`, and the alternatives of a node (`anyOf`,
/// `oneOf`, `allOf`) and the definition its `$ref` names in the schema's own
/// `$defs` are at the node's path. A path that reaches no node of the
/// declaration's schema is a mistake of the declaration.
///
/// A mark on a value that holds several texts (a list, a map, a structure) says
/// what every text inside it is: `environments.*.labels` (the lists feature)
/// marks every label template of every list environment, and `styles.*.join`
/// (the numbering feature) every text of each counter style's join. A
/// value that holds texts of different kinds is not marked as a whole; the
/// values below it are marked one by one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyMark {
    /// The path of the marked value below the feature's name, its segments
    /// separated by `.`, with `*` meaning every entry of a map and every item of
    /// a list.
    pub path: String,
    /// The marks of the value, each once, in the order of [`Mark::ALL`].
    pub marks: Vec<Mark>,
}

impl KeyMark {
    /// Whether the value carries `mark`.
    pub fn has(&self, mark: Mark) -> bool {
        self.marks.contains(&mark)
    }
}

/// Rewrites, in `data` (settings that `schema`, the object schema of a part
/// declared from data, describes), every text at a node that the schema marks
/// with `mark`, by `rewrite`; where the value at the node is a list or a map,
/// every text inside it. The paths of the marked nodes are read as
/// [`KeyMark`] says. A schema whose marks cannot be read marks nothing here:
/// its declaration reports the fault ([`SettingsDecl::entry_data`]).
///
/// # Errors
///
/// The text of the first `Err` of `rewrite`, which leaves the text unchanged.
pub(crate) fn rewrite_marked(
    schema: &Value,
    mark: Mark,
    data: &mut Map<String, Value>,
    rewrite: &mut dyn FnMut(&str) -> Result<String, String>,
) -> Result<(), String> {
    let Ok(read) = marks::read(schema) else { return Ok(()) };
    let mut failed = None;
    for node in read.iter().filter(|node| node.marks.contains(&mark)) {
        let mut segments = node.path.split('.');
        let Some(value) = segments.next().and_then(|key| data.get_mut(key)) else { continue };
        let below: Vec<&str> = segments.collect();
        texts_at(value, &below, &mut |text| {
            if failed.is_some() {
                return;
            }
            match rewrite(text) {
                Ok(rewritten) => *text = rewritten,
                Err(error) => failed = Some(error),
            }
        });
    }
    failed.map_or(Ok(()), Err)
}

/// Calls `visit` on every text in `value` at the place that `segments`
/// designates below it (`*` matching every entry of a map and every item of a
/// list); where the value at that place is a list or a map, on every text
/// inside it.
pub(crate) fn texts_at(value: &mut Value, segments: &[&str], visit: &mut dyn FnMut(&mut String)) {
    match (segments.split_first(), value) {
        (None, Value::String(text)) => visit(text),
        (None, Value::Array(items)) => items.iter_mut().for_each(|item| texts_at(item, &[], visit)),
        (None, Value::Object(map)) => map.values_mut().for_each(|item| texts_at(item, &[], visit)),
        (Some((&"*", rest)), Value::Array(items)) => items.iter_mut().for_each(|item| texts_at(item, rest, visit)),
        (Some((&"*", rest)), Value::Object(map)) => map.values_mut().for_each(|item| texts_at(item, rest, visit)),
        (Some((name, rest)), Value::Object(map)) => {
            if let Some(item) = map.get_mut(*name) {
                texts_at(item, rest, visit);
            }
        }
        _ => {}
    }
}

/// One settings key of a feature, as [`SettingsDecl::keys`] and
/// [`SettingsDecl::key`] return it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyDecl {
    /// The key's name, which is the name of the Rust field its value is read
    /// into, or a property of the schema of a part declared from data.
    pub name: String,
    /// The binding of the Rust type the field belongs to: where a value for the
    /// key may be set.
    pub bind: Bind,
}

/// The function of a [`Part`] that reads the part's keys, given as a map.
type ReadFn = Arc<dyn Fn(Value) -> Result<ReadValue, ReadError> + Send + Sync>;

/// The function of a [`Part`] that declares it as an option type.
type DeclareFn = Arc<dyn Fn(&mut FeatureBuilder<'_>, Bind, String) -> Result<(), BuildError> + Send + Sync>;

/// The function of a [`Part`] that returns its JSON Schema.
type SchemaFn = Arc<dyn Fn(&mut schemars::SchemaGenerator) -> schemars::Schema + Send + Sync>;

/// One Rust type of a feature's settings, or one part declared from data, with
/// the functions that read it from data, declare it as an option type and give
/// its JSON Schema.
#[derive(Clone)]
struct Part {
    bind: Bind,
    option: OptionKey,
    docs: String,
    keys: Vec<String>,
    read: ReadFn,
    declare: DeclareFn,
    schema: SchemaFn,
}

/// The value read for one [`Part`]: a configuration, or the function that sets
/// the option value read in an [`OptionValues`].
enum ReadValue {
    Config(Arc<dyn Any + Send + Sync>),
    Option(Box<dyn FnOnce(&mut OptionValues)>),
}

/// A failure to read a feature's settings, or one part of them: the value at
/// fault, by its path below the feature's name, and the condition it violates.
///
/// The reader of a configuration declared from data answers one
/// ([`SettingsDecl::config_data_read`]), and so does each part of a
/// declaration whose keys cannot be read; [`SettingsDecl::read`] reports it
/// as a [`SettingsError`] of the kind [`SettingsErrorKind::InvalidValue`],
/// whose path is the feature's name followed by `path`. As data, it is the
/// map `{ path, detail }`, `path` being optional.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadError {
    /// The path of the value at fault below the feature's name, its segments
    /// separated by `.` (`macros.abs.args`); empty when the fault concerns
    /// the settings as a whole.
    #[serde(default)]
    pub path: String,
    /// The condition the value violates, as a message for a person (``must be
    /// a text, not `3` ``).
    pub detail: String,
}

/// A feature's declaration of its settings: the Rust types they are read into,
/// each with its binding, and marks on some of their keys.
///
/// A feature returns its declaration from
/// [`FlmFeature::settings`](crate::core::registry::FlmFeature::settings). The
/// registry build declares the feature's option types from it, and a package
/// factory lists it beside the feature's name
/// ([`PackageFactory`](crate::core::registry::PackageFactory)). A host uses it to
/// read the feature's settings from data ([`SettingsDecl::read`]), to know the
/// keys and their marks, and to describe them ([`SettingsDecl::json_schema`]).
///
/// Create one with [`SettingsDecl::new`], then add at most one configuration
/// ([`config`](SettingsDecl::config) or [`config_from`](SettingsDecl::config_from)),
/// any number of option types ([`options`](SettingsDecl::options)) and marks on
/// the values they declare ([`merge_entries`](SettingsDecl::merge_entries),
/// [`remove_false`](SettingsDecl::remove_false),
/// [`default_on_null`](SettingsDecl::default_on_null),
/// [`markup`](SettingsDecl::markup), [`path`](SettingsDecl::path)), each method
/// taking and returning the declaration. Two types that declare the same key,
/// and a mark whose path reaches no node of the schema, make these methods
/// panic: a declaration that contradicts itself is a mistake in the feature's
/// code. The parts declared from data
/// ([`config_data`](SettingsDecl::config_data),
/// [`config_data_read`](SettingsDecl::config_data_read),
/// [`options_data`](SettingsDecl::options_data)) read their marks from the
/// nodes of their schema, and return a fault of the declaration as an error.
/// See the [module documentation](self).
///
/// # Examples
///
/// ```
/// use flm_core::core::settings::SettingsDecl;
/// use flm_core::model::options::Bind;
///
/// #[derive(Debug, Clone, Default, Hash, serde::Deserialize, schemars::JsonSchema)]
/// #[serde(default, deny_unknown_fields)]
/// struct PageOptions {
///     /// The page's width, in characters.
///     width: u32,
/// }
///
/// let decl = SettingsDecl::new("page").options::<PageOptions>(Bind::Render, "The page.");
/// assert_eq!(decl.keys().map(|key| key.name).collect::<Vec<_>>(), ["width"]);
///
/// let data = serde_json::json!({ "width": 72 });
/// let settings = decl.read(data.as_object().unwrap()).unwrap();
/// assert_eq!(settings.render_options().get::<PageOptions>().unwrap().width, 72);
///
/// let wrong = serde_json::json!({ "height": 72 });
/// let errors = decl.read(wrong.as_object().unwrap()).unwrap_err();
/// assert_eq!(errors[0].path, "page.height");
/// ```
#[derive(Clone)]
pub struct SettingsDecl {
    feature: FeatureName,
    parts: Vec<Part>,
    marks: Vec<KeyMark>,
    defaults: Vec<(String, Map<String, Value>)>,
    /// The places where other features declare entries
    /// ([`open_entries`](SettingsDecl::open_entries)).
    open: Vec<String>,
    /// The entries of other features' keys that this feature declares
    /// ([`entry_data`](SettingsDecl::entry_data)).
    entries: Vec<EntryDecl>,
    /// The entries in force of this feature's own keys, which other features
    /// declare ([`with_entries`](SettingsDecl::with_entries)).
    in_force: Vec<EntryDecl>,
    /// The schema of the parts, against which the paths of the marks set by
    /// method are checked: made on the first mark set after a part is added.
    /// It holds no mark set by method; a data part keeps the `x-flm-marks` of
    /// its own schema there.
    bare: OnceLock<Arc<Value>>,
}

/// The declaration of one entry of a map that a key of another feature holds:
/// the schema of that entry, which a feature gives in its own settings
/// declaration ([`SettingsDecl::entry_data`]).
///
/// The feature that holds the key, the *owner*, states in its declaration the
/// places where other features declare entries
/// ([`SettingsDecl::open_entries`]); an entry is declared at one place that
/// such a path names, by its concrete path below the owner's name (each `*`
/// replaced by the entry's name). A declaration of an entry is in force when
/// its feature and the owner are among the features of the packages in use and
/// neither is switched off; the [`cascade`] then combines the owner's
/// declaration with it ([`SettingsDecl::with_entries`]):
///
/// - the marks of the entry's schema apply to the values written under the
///   entry, as the owner's own marks do;
/// - a key written under the entry that the schema's `properties` lack is left
///   out of the combined settings, with a warning at its origin;
/// - the owner's JSON Schema ([`SettingsDecl::json_schema`]) holds the entry's
///   schema at its place.
///
/// The values written under the entry are not checked against the types of
/// the schema, as for any part declared from data.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryDecl {
    owner: FeatureName,
    path: String,
    /// What the entry is, in the words of the person who writes its schema
    /// ([`SettingsDecl::entry_data`]).
    label: String,
    schema: Value,
    keys: Vec<String>,
    /// The marks of the schema, each path relative to the entry.
    marks: Vec<KeyMark>,
    /// The default entries of the objects of the schema combined key by key,
    /// each path relative to the entry.
    defaults: Vec<(String, Map<String, Value>)>,
}

impl EntryDecl {
    /// The name of the feature that holds the key the entry belongs to.
    pub fn owner(&self) -> &FeatureName {
        &self.owner
    }

    /// The entry's path below the owner's name, its segments joined by `.`
    /// (`template_config.html.base`).
    pub fn path(&self) -> &str {
        &self.path
    }

    /// What the entry is, in the words of the person who writes its schema,
    /// without an article (`compose template ‘base’ for the output ‘html’`).
    pub fn label(&self) -> &str {
        &self.label
    }

    /// The keys the entry declares: the names of the `properties` of its
    /// schema, in their order.
    pub fn keys(&self) -> &[String] {
        &self.keys
    }
}

/// A function that returns the default entries of an object combined key by
/// key: the entries the object holds when no source of settings writes it.
///
/// The function returns the entries as data, a map from each entry's name to the
/// rest of the entry, or a text saying why they cannot be written as data.
/// [`SettingsDecl::merge_entries`] calls it once, when the mark is set.
pub type DefaultEntries = fn() -> Result<Value, String>;

impl SettingsDecl {
    /// Creates the declaration of the settings of the feature named `feature`,
    /// with no key yet.
    pub fn new(feature: impl Into<FeatureName>) -> SettingsDecl {
        SettingsDecl {
            feature: feature.into(),
            parts: Vec::new(),
            marks: Vec::new(),
            defaults: Vec::new(),
            open: Vec::new(),
            entries: Vec::new(),
            in_force: Vec::new(),
            bare: OnceLock::new(),
        }
    }

    /// Adds the feature's configuration, the type `C` its constructor takes,
    /// with the description `docs`, and returns the declaration. Every field of
    /// `C` is a settings key, bound when the registry is built
    /// ([`Bind::Registry`]).
    ///
    /// `C` is read with its `Deserialize` implementation. An empty map must read
    /// as `C`'s default, which `#[serde(default)]` on the type provides, because
    /// [`read`](SettingsDecl::read) reads the configuration even when none of
    /// its keys is written.
    ///
    /// # Panics
    ///
    /// When a field of `C` has the name of a key already declared or is named
    /// [`ENABLED_KEY`] (`enabled`), when the declaration already has a
    /// configuration, or when `C` is not read from a map of named fields.
    pub fn config<C: SettingsData + Options>(self, docs: impl Into<String>) -> SettingsDecl {
        self.config_from::<C, C>(docs)
    }

    /// Adds the feature's configuration, the type `C` its constructor takes,
    /// read from data as the type `D` and converted with `C::try_from`, and
    /// returns the declaration. Use it for a configuration that holds values
    /// data cannot state, or whose form as data differs from its Rust type.
    /// Every field of `D` is a settings key; a conversion error is reported as
    /// a [`SettingsError`] for the feature as a whole. Otherwise as
    /// [`config`](SettingsDecl::config).
    ///
    /// # Panics
    ///
    /// When a field of `D` has the name of a key already declared or is named
    /// [`ENABLED_KEY`] (`enabled`), when the declaration already has a
    /// configuration, or when `D` is not read from a map of named fields.
    pub fn config_from<D, C>(self, docs: impl Into<String>) -> SettingsDecl
    where
        D: SettingsData,
        C: Options + TryFrom<D>,
        <C as TryFrom<D>>::Error: fmt::Display,
    {
        fn read<D, C>(data: Value) -> Result<ReadValue, ReadError>
        where
            D: SettingsData,
            C: Options + TryFrom<D>,
            <C as TryFrom<D>>::Error: fmt::Display,
        {
            let value = deserialize::<D>(data)?;
            let config = C::try_from(value)
                .map_err(|error| ReadError { path: String::new(), detail: error.to_string() })?;
            Ok(ReadValue::Config(Arc::new(config)))
        }
        fn declare<C: Options>(
            b: &mut FeatureBuilder<'_>,
            bind: Bind,
            docs: String,
        ) -> Result<(), BuildError> {
            b.options_with_docs::<C>(bind, docs)
        }
        self.part(Part {
            bind: Bind::Registry,
            option: OptionKey::of::<C>(),
            docs: crate::kept_docs(docs),
            keys: field_names::<D>(),
            read: Arc::new(read::<D, C>),
            declare: Arc::new(declare::<C>),
            schema: Arc::new(|generator: &mut schemars::SchemaGenerator| generator.subschema_for::<D>()),
        })
    }

    /// Adds the feature's configuration declared from data, and returns the
    /// declaration: `schema` is a JSON Schema of an object, whose `properties`
    /// are the configuration's settings keys, bound when the registry is built
    /// ([`Bind::Registry`]); `docs` describes it.
    ///
    /// The configuration is the option type named by the feature's name
    /// ([`OptionKey::Data`]). [`read`](SettingsDecl::read) keeps the values
    /// written as a [`DataValue`], the empty map when none of the keys is
    /// written, and checks nothing of them but their keys (see the [module
    /// documentation](self#settings-declared-from-data)); a package reads it with
    /// [`PackageSettings::config`]`::<DataValue>`. A feature that reads its
    /// configuration when the settings are read declares it with
    /// [`config_data_read`](SettingsDecl::config_data_read).
    ///
    /// The schema's own `$defs` join those of the declaration's schema
    /// ([`json_schema`](SettingsDecl::json_schema)): a `$defs` name shared with
    /// another part of the declaration refers to one definition.
    ///
    /// # Errors
    ///
    /// A [`SettingsError`] of the kind [`SettingsErrorKind::InvalidDeclaration`]
    /// when `schema` has no `properties` map, when one of its properties is a
    /// key declared already or is named [`ENABLED_KEY`] (`enabled`), when
    /// the declaration has a configuration already, or when the marks of its
    /// nodes are not a list of mark names, lie outside every setting (on the
    /// schema's root, or under a keyword that no path follows), mark an
    /// object as combined key by key with a `default` that is not a map, or
    /// are read while a definition of `$defs` that refers to itself is read
    /// (a mark is kept with its path, and a path names one depth of the
    /// definition).
    pub fn config_data(self, schema: Value, docs: impl Into<String>) -> Result<SettingsDecl, SettingsError> {
        self.data_config(schema, docs.into(), Arc::new(|data: Value| Ok(ReadValue::Config(Arc::new(DataValue(data))))))
    }

    /// Adds the feature's configuration declared from data, read by `read`,
    /// and returns the declaration: as [`config_data`](SettingsDecl::config_data),
    /// except that [`read`](SettingsDecl::read) gives the configuration
    /// written (the empty map when none of its keys is written) to `read`, and
    /// keeps the value `read` answers as the [`DataValue`] that
    /// [`PackageSettings::config`]`::<DataValue>` gives.
    ///
    /// `read` is the twin of [`config_from`](SettingsDecl::config_from)'s
    /// reading as `D` and conversion with `C::try_from`: it answers the value
    /// the feature receives, or the value at fault as a [`ReadError`], which
    /// is reported as `config_from`'s conversion errors are, a
    /// [`SettingsError`] of the kind [`SettingsErrorKind::InvalidValue`] whose
    /// path is the feature's name followed by the error's path. A reader that
    /// only checks the configuration answers it unchanged.
    ///
    /// # Errors
    ///
    /// As [`config_data`](SettingsDecl::config_data).
    pub fn config_data_read(
        self,
        schema: Value,
        docs: impl Into<String>,
        read: Arc<dyn Fn(Value) -> Result<Value, ReadError> + Send + Sync>,
    ) -> Result<SettingsDecl, SettingsError> {
        let read: ReadFn = Arc::new(move |data: Value| Ok(ReadValue::Config(Arc::new(DataValue(read(data)?)))));
        self.data_config(schema, docs.into(), read)
    }

    /// The configuration declared from data with the schema `schema`, the
    /// description `docs` and the reading `read`: what
    /// [`config_data`](SettingsDecl::config_data) and
    /// [`config_data_read`](SettingsDecl::config_data_read) add.
    fn data_config(self, schema: Value, docs: String, read: ReadFn) -> Result<SettingsDecl, SettingsError> {
        let feature = self.feature.clone();
        let keys = schema_keys(&schema).ok_or_else(|| {
            self.invalid(
                String::new(),
                format!(
                    "the schema of the configuration of the feature `{feature}` must be an object \
                     schema with `properties`, a map from each setting to its schema"
                ),
            )
        })?;
        let name = Name::clone(&feature);
        self.try_part(Part {
            bind: Bind::Registry,
            option: OptionKey::Data(name.clone()),
            docs: crate::kept_docs(docs),
            keys,
            read,
            declare: declare_named(name),
            schema: data_schema(schema.clone()),
        })?
        .with_read_marks(&schema)
    }

    /// Adds the option type named `name`, declared from data, with the binding
    /// `bind`, and returns the declaration: `schema` is a JSON Schema of an
    /// object, whose `properties` are the option type's settings keys; `docs`
    /// describes it.
    ///
    /// The option type is identified by `name` ([`OptionKey::Data`]); a stage
    /// reads it by that name
    /// ([`StageCx::option_named`](crate::core::run::StageCx::option_named)).
    /// [`read`](SettingsDecl::read) keeps the values written as a [`DataValue`]
    /// and checks nothing of them but their keys; when none of its keys is
    /// written, the option type gets no value, and a read resolves to the empty
    /// map.
    ///
    /// The schema's own `$defs` join those of the declaration's schema
    /// ([`json_schema`](SettingsDecl::json_schema)): a `$defs` name shared with
    /// another part of the declaration refers to one definition.
    ///
    /// # Errors
    ///
    /// As [`config_data`](SettingsDecl::config_data), and when `bind` is
    /// [`Bind::Registry`] (a value fixed with the registry is the feature's
    /// configuration).
    pub fn options_data(
        self,
        name: impl Into<Name>,
        bind: Bind,
        schema: Value,
        docs: impl Into<String>,
    ) -> Result<SettingsDecl, SettingsError> {
        let name = name.into();
        let feature = self.feature.clone();
        if bind == Bind::Registry {
            return Err(self.invalid(
                String::new(),
                format!(
                    "the feature `{feature}` declares the option `{name}` bound to the registry, \
                     and a value fixed when the registry is built belongs in the feature's \
                     configuration"
                ),
            ));
        }
        let keys = schema_keys(&schema).ok_or_else(|| {
            self.invalid(
                String::new(),
                format!(
                    "the schema of the option `{name}` of the feature `{feature}` must be an object \
                     schema with `properties`, a map from each setting to its schema"
                ),
            )
        })?;
        let read_name = name.clone();
        self.try_part(Part {
            bind,
            option: OptionKey::Data(name.clone()),
            docs: crate::kept_docs(docs),
            keys,
            read: Arc::new(move |data: Value| {
                let name = read_name.clone();
                Ok(ReadValue::Option(Box::new(move |values: &mut OptionValues| values.set_named(name, data))))
            }),
            declare: declare_named(name),
            schema: data_schema(schema.clone()),
        })?
        .with_read_marks(&schema)
    }

    /// Adds the option type `O` of the feature, with the binding `bind` and the
    /// description `docs`, and returns the declaration. Every field of `O` is a
    /// settings key.
    ///
    /// # Panics
    ///
    /// When `bind` is [`Bind::Registry`] (a value fixed with the registry is
    /// the feature's configuration: see [`config`](SettingsDecl::config)), when
    /// a field of `O` has the name of a key already declared or is named
    /// [`ENABLED_KEY`] (`enabled`), or when `O` is not read from a map of named
    /// fields.
    pub fn options<O: SettingsData + Options>(
        self,
        bind: Bind,
        docs: impl Into<String>,
    ) -> SettingsDecl {
        assert!(
            bind != Bind::Registry,
            "the feature `{}` declares the option type `{}` with `Bind::Registry`, but the values \
             of an option type must be set on the document or on the render request; a value \
             fixed when the registry is built belongs in the feature's configuration, declared \
             with `SettingsDecl::config`",
            self.feature,
            crate::model::options::short_type_name(std::any::type_name::<O>())
        );
        fn read<O: SettingsData + Options>(data: Value) -> Result<ReadValue, ReadError> {
            let value = deserialize::<O>(data)?;
            Ok(ReadValue::Option(Box::new(move |values: &mut OptionValues| values.set::<O>(value))))
        }
        fn declare<O: Options>(
            b: &mut FeatureBuilder<'_>,
            bind: Bind,
            docs: String,
        ) -> Result<(), BuildError> {
            b.options_with_docs::<O>(bind, docs)
        }
        self.part(Part {
            bind,
            option: OptionKey::of::<O>(),
            docs: crate::kept_docs(docs),
            keys: field_names::<O>(),
            read: Arc::new(read::<O>),
            declare: Arc::new(declare::<O>),
            schema: Arc::new(|generator: &mut schemars::SchemaGenerator| generator.subschema_for::<O>()),
        })
    }

    /// Adds `part`; panics on a contradiction (a declaration in Rust code).
    fn part(self, part: Part) -> SettingsDecl {
        match self.try_part(part) {
            Ok(decl) => decl,
            Err(error) => panic!("{}", error.detail),
        }
    }

    /// Adds `part`, or returns the fault it makes: a key named
    /// [`ENABLED_KEY`], or a contradiction with the parts already declared (a
    /// key declared twice, or a second configuration).
    fn try_part(mut self, part: Part) -> Result<SettingsDecl, SettingsError> {
        if part.keys.iter().any(|key| key == ENABLED_KEY) {
            let detail = format!(
                "the feature ‘{}’ declares the setting ‘{ENABLED_KEY}’, a name reserved for \
                 switching a feature on or off",
                self.feature
            );
            return Err(self.invalid(ENABLED_KEY.to_string(), detail));
        }
        for key in &part.keys {
            if self.part_of(key).is_some() {
                let detail = format!("the feature `{}` declares the setting `{key}` twice", self.feature);
                return Err(self.invalid(key.clone(), detail));
            }
        }
        if part.bind == Bind::Registry && self.parts.iter().any(|p| p.bind == Bind::Registry) {
            let detail = format!("the feature `{}` declares two configurations", self.feature);
            return Err(self.invalid(String::new(), detail));
        }
        self.parts.push(part);
        self.bare = OnceLock::new();
        Ok(self)
    }

    /// An [`InvalidDeclaration`](SettingsErrorKind::InvalidDeclaration) error
    /// at `path` below the feature's name (the feature's name alone when `path`
    /// is empty).
    fn invalid(&self, path: String, detail: String) -> SettingsError {
        let path = if path.is_empty() { self.feature.to_string() } else { format!("{}.{path}", self.feature) };
        SettingsError { path, kind: SettingsErrorKind::InvalidDeclaration, detail }
    }

    /// Marks the object at `path` as combined key by key
    /// ([`Mark::MergeObjectKeyValues`]), and returns the declaration: a host that
    /// combines two sources of settings combines that object one key at a time.
    /// See [`KeyMark`] for the form of the path, which reaches a value at any
    /// depth.
    ///
    /// `default_entries` returns, as data, the entries the object holds when no
    /// source writes it: the feature's own default. A host treats them as the
    /// lowest-ranked source, so an entry a source writes replaces the default
    /// entry of that name, and the default entries no source writes stay. The
    /// function usually writes the default of the key's Rust field as data
    /// ([`named::to_data`] for a list of named entries). Read them back with
    /// [`default_entries`](SettingsDecl::default_entries); the JSON Schema
    /// gives them as the `default` of the object's node.
    ///
    /// A collection of named things is also marked with
    /// [`remove_false`](SettingsDecl::remove_false).
    ///
    /// # Panics
    ///
    /// When `path` reaches no node of the declaration's schema, and when
    /// `default_entries` fails or returns something other than a map: both are
    /// mistakes in the feature's code, not in any data.
    pub fn merge_entries(self, path: impl Into<String>, default_entries: DefaultEntries) -> SettingsDecl {
        let path = path.into();
        let decl = self.mark(path.clone(), Mark::MergeObjectKeyValues);
        let entries = match default_entries() {
            Ok(Value::Object(entries)) => entries,
            Ok(other) => panic!(
                "the default entries of ‘{}.{path}’ must be written as a map, not ‘{other}’",
                decl.feature
            ),
            Err(detail) => panic!(
                "the default entries of ‘{}.{path}’ cannot be written as data: {detail}",
                decl.feature
            ),
        };
        decl.with_default(path, entries)
    }

    /// Records `entries` as the default entries of the object at `path`.
    fn with_default(mut self, path: String, entries: Map<String, Value>) -> SettingsDecl {
        self.defaults.retain(|(marked, _)| *marked != path);
        self.defaults.push((path, entries));
        self
    }

    /// The default entries of the object at `path`, marked as combined key by
    /// key ([`Mark::MergeObjectKeyValues`]): the entries it holds when no source
    /// of settings writes it, as a map from each entry's name to the rest of the
    /// entry, in the feature's order. Returns `None` when the object is not
    /// marked so.
    pub fn default_entries(&self, path: &str) -> Option<&Map<String, Value>> {
        self.defaults.iter().find(|(marked, _)| marked == path).map(|(_, entries)| entries)
    }

    /// Marks the object at `path` as one in which a key whose value is `false`
    /// is removed ([`Mark::RemoveKeyWhenValueIsFalse`]) and returns the
    /// declaration. See [`KeyMark`] for the form of the path.
    ///
    /// # Panics
    ///
    /// When `path` reaches no node of the declaration's schema.
    pub fn remove_false(self, path: impl Into<String>) -> SettingsDecl {
        self.mark(path.into(), Mark::RemoveKeyWhenValueIsFalse)
    }

    /// Marks the setting at `path` as one that a value of `null` gives its
    /// default ([`Mark::SetToDefaultWithNull`]) and returns the declaration. See
    /// [`KeyMark`] for the form of the path.
    ///
    /// # Panics
    ///
    /// When `path` reaches no node of the declaration's schema.
    pub fn default_on_null(self, path: impl Into<String>) -> SettingsDecl {
        self.mark(path.into(), Mark::SetToDefaultWithNull)
    }

    /// Marks the value at `path` as text in FLM markup ([`Mark::Markup`]) and
    /// returns the declaration. See [`KeyMark`] for the form of the path.
    ///
    /// # Panics
    ///
    /// When `path` reaches no node of the declaration's schema.
    pub fn markup(self, path: impl Into<String>) -> SettingsDecl {
        self.mark(path.into(), Mark::Markup)
    }

    /// Marks the value at `path` as a path to a file ([`Mark::Path`]) and
    /// returns the declaration. See [`KeyMark`] for the form of the path.
    ///
    /// # Panics
    ///
    /// When `path` reaches no node of the declaration's schema.
    pub fn path(self, path: impl Into<String>) -> SettingsDecl {
        self.mark(path.into(), Mark::Path)
    }

    /// Sets `mark` on the value at `path` and returns the declaration: the
    /// methods above. A mark set on a value that carries it already changes
    /// nothing.
    ///
    /// # Panics
    ///
    /// When `path` reaches no node of the declaration's schema.
    fn mark(self, path: String, mark: Mark) -> SettingsDecl {
        let bare = Arc::clone(self.bare.get_or_init(|| Arc::new(self.bare_schema())));
        if !marks::reaches(&bare, &path) {
            panic!("missing setting ‘{path}’ for the mark ‘{mark}’ of the feature ‘{}’", self.feature);
        }
        self.with_mark(path, mark)
    }

    /// Adds `mark` to the marks of the value at `path`.
    fn with_mark(mut self, path: String, mark: Mark) -> SettingsDecl {
        match self.marks.iter_mut().find(|marked| marked.path == path) {
            Some(marked) => {
                if !marked.has(mark) {
                    marked.marks.push(mark);
                    marked.marks.sort();
                }
            }
            None => self.marks.push(KeyMark { path, marks: vec![mark] }),
        }
        self
    }

    /// Adds the marks that the schema `schema` of a part declared from data
    /// writes on its nodes, each with its default entries.
    fn with_read_marks(mut self, schema: &Value) -> Result<SettingsDecl, SettingsError> {
        let read = read_marks(schema, &self.feature, "", &format!("the feature ‘{}’", self.feature))?;
        for written in read {
            for mark in written.marks {
                self = self.with_mark(written.path.clone(), mark);
            }
            if let Some(entries) = written.default {
                self = self.with_default(written.path, entries);
            }
        }
        Ok(self)
    }

    /// Marks the places `path` (see [`KeyMark`] for its form) as those where
    /// other features declare entries ([`entry_data`](SettingsDecl::entry_data),
    /// [`EntryDecl`]), and returns the declaration: `template_config.*.*`
    /// lets another feature declare the schema of `template_config.html.base`.
    ///
    /// # Panics
    ///
    /// When `path` reaches no node of the declaration's schema.
    pub fn open_entries(self, path: impl Into<String>) -> SettingsDecl {
        let path = path.into();
        let bare = Arc::clone(self.bare.get_or_init(|| Arc::new(self.bare_schema())));
        if !marks::reaches(&bare, &path) {
            panic!("missing setting ‘{path}’ for the entries that the feature ‘{}’ opens", self.feature);
        }
        let mut decl = self;
        if !decl.open.contains(&path) {
            decl.open.push(path);
        }
        decl
    }

    /// Whether another feature may declare the entry at `path` (its segments
    /// joined by `.`, with no `*`): one of the places
    /// [`open_entries`](SettingsDecl::open_entries) gives names it.
    pub fn opens(&self, path: &str) -> bool {
        let segments: Vec<String> = path.split('.').map(str::to_string).collect();
        self.open.iter().any(|open| cascade::names(open, &segments))
    }

    /// Declares the schema of the entry at `path` of a key of the feature
    /// `owner`, and returns the declaration (see [`EntryDecl`]). `path` is the
    /// entry's path below the owner's name, its segments joined by `.`, each
    /// segment the name of an entry (`template_config.html.base`); `label` is
    /// what the entry is, in the words of the person who writes its schema and
    /// without an article (`compose template ‘base’ for the output ‘html’`),
    /// which the messages about the entry name it by; `schema` is
    /// a JSON Schema of an object, whose `properties` are the keys of the
    /// entry, with the marks written on its nodes, read as those of a part
    /// declared from data are (see the [module
    /// documentation](self#settings-declared-from-data)). An entry is not a
    /// key of the declaring feature: a declaration with entries and no key of
    /// its own describes a feature that takes no settings
    /// ([`takes_settings`](SettingsDecl::takes_settings)).
    ///
    /// # Errors
    ///
    /// A [`SettingsError`] of the kind [`SettingsErrorKind::InvalidDeclaration`]
    /// when `path` holds `*` or an empty segment, when the declaration already
    /// declares that entry, when `schema` has no `properties` map, or when the
    /// marks of its nodes are faulty as [`config_data`](SettingsDecl::config_data)
    /// states.
    pub fn entry_data(
        mut self,
        owner: impl Into<FeatureName>,
        path: impl Into<String>,
        label: impl Into<String>,
        schema: Value,
    ) -> Result<SettingsDecl, SettingsError> {
        let (owner, path, label) = (owner.into(), path.into(), label.into());
        let at = |detail: String| SettingsError {
            path: format!("{owner}.{path}"),
            kind: SettingsErrorKind::InvalidDeclaration,
            detail,
        };
        if path.split('.').any(|segment| segment.is_empty() || segment == "*") {
            return Err(at(format!(
                "Invalid entry ‘{owner}.{path}’ declared by the feature ‘{}’: each part of the path must name one entry",
                self.feature
            )));
        }
        if self.entries.iter().any(|entry| entry.owner == owner && entry.path == path) {
            return Err(at(format!("Duplicate {label} (feature ‘{}’)", self.feature)));
        }
        let Some(keys) = schema_keys(&schema) else {
            return Err(at(format!(
                "Invalid settings schema of the {label}: it must be an object schema with ‘properties’, a map \
                 from each setting to its schema"
            )));
        };
        let mut marks: Vec<KeyMark> = Vec::new();
        let mut defaults = Vec::new();
        for written in read_marks(&schema, &owner, &path, &format!("the {label}"))? {
            match marks.iter_mut().find(|marked| marked.path == written.path) {
                Some(marked) => {
                    marked.marks.extend(written.marks);
                    marked.marks.sort();
                    marked.marks.dedup();
                }
                None => marks.push(KeyMark { path: written.path.clone(), marks: written.marks }),
            }
            if let Some(entries) = written.default {
                defaults.push((written.path, entries));
            }
        }
        self.entries.push(EntryDecl { owner, path, label, schema, keys, marks, defaults });
        Ok(self)
    }

    /// The entries of other features' keys that this feature declares
    /// ([`entry_data`](SettingsDecl::entry_data)), in the order declared.
    pub fn entries(&self) -> &[EntryDecl] {
        &self.entries
    }

    /// Returns this declaration combined with the declarations of its entries
    /// `entries`, those in force (see [`EntryDecl`]): each entry of `entries`
    /// whose owner is this feature and whose path this declaration opens
    /// ([`opens`](SettingsDecl::opens)); the others are ignored.
    ///
    /// The result carries the marks of each entry's schema at their paths
    /// below the entry, with their default entries; the marks that this
    /// declaration sets on the places of the entry and of the maps above it,
    /// by a path with `*`, also at the entry's own path, so that its JSON
    /// Schema writes them on the entry's node; and the entry's schema at its
    /// place in its JSON Schema ([`json_schema`](SettingsDecl::json_schema)),
    /// its `$defs` joining those of the result under names of the entry's
    /// own (its path, a `.` and the definition's name). [`entry_in_force`](SettingsDecl::entry_in_force)
    /// gives each entry back. The cascade makes it from the entries declared by
    /// the features in use.
    pub fn with_entries<'e>(&self, entries: impl IntoIterator<Item = &'e EntryDecl>) -> SettingsDecl {
        let mut decl = self.clone();
        for entry in entries {
            if entry.owner != self.feature || !self.opens(&entry.path) || decl.entry_in_force(&entry.path).is_some() {
                continue;
            }
            let segments: Vec<String> = entry.path.split('.').map(str::to_string).collect();
            for depth in 2..=segments.len() {
                let place = &segments[..depth];
                let concrete = place.join(".");
                let owned: Vec<KeyMark> =
                    self.marks.iter().filter(|marked| cascade::names(&marked.path, place)).cloned().collect();
                for marked in owned {
                    for mark in &marked.marks {
                        decl = decl.with_mark(concrete.clone(), *mark);
                    }
                    if let Some(entries) = self.default_entries(&marked.path) {
                        decl = decl.with_default(concrete.clone(), entries.clone());
                    }
                }
            }
            for marked in &entry.marks {
                for mark in &marked.marks {
                    decl = decl.with_mark(format!("{}.{}", entry.path, marked.path), *mark);
                }
            }
            for (path, entries) in &entry.defaults {
                decl = decl.with_default(format!("{}.{path}", entry.path), entries.clone());
            }
            decl.in_force.push(entry.clone());
        }
        decl
    }

    /// The entry in force at `path` (its segments joined by `.`), declared by
    /// another feature and combined with this declaration by
    /// [`with_entries`](SettingsDecl::with_entries), if there is one.
    pub fn entry_in_force(&self, path: &str) -> Option<&EntryDecl> {
        self.in_force.iter().find(|entry| entry.path == path)
    }
}

/// The marks that `schema`, a schema given as data, writes on its nodes, each
/// at its path; `prefix` is the path of the schema below the feature `feature`
/// (empty for a part of the feature's own settings, the entry's path for a
/// declaration of an entry, [`SettingsDecl::entry_data`]), and the paths read
/// are relative to it. The messages name the schema's holder by `holder`
/// (`the feature ‘X’`, or the label of the entry with its article) and each
/// node by its path relative to `prefix`.
///
/// # Errors
///
/// A fault of the marks, as a [`SettingsError`] of the kind
/// [`SettingsErrorKind::InvalidDeclaration`] at the place it concerns.
fn read_marks(
    schema: &Value,
    feature: &FeatureName,
    prefix: &str,
    holder: &str,
) -> Result<Vec<marks::ReadMark>, SettingsError> {
    let full = |path: &str| match (prefix.is_empty(), path.is_empty()) {
        (true, _) => path.to_string(),
        (false, true) => prefix.to_string(),
        (false, false) => format!("{prefix}.{path}"),
    };
    marks::read(schema).map_err(|fault| {
        let (path, detail) = match fault {
            marks::Fault::NotAList(path) => {
                let detail = format!("‘{MARKS_KEYWORD}’ on ‘{path}’ of {holder} must be a list of mark names");
                (full(&path), detail)
            }
            marks::Fault::Unknown(path, name) => {
                let names: Vec<String> = Mark::ALL.iter().map(|mark| format!("‘{mark}’")).collect();
                let detail = format!("unknown mark ‘{name}’ on ‘{path}’ of {holder} (the marks are {})", names.join(", "));
                (full(&path), detail)
            }
            marks::Fault::DefaultNotAMap(path) => {
                let detail = format!(
                    "the default of ‘{path}’ of {holder} must be a map (mark ‘{}’)",
                    Mark::MergeObjectKeyValues
                );
                (full(&path), detail)
            }
            marks::Fault::Outside(pointer) => {
                let place = if pointer.is_empty() { "on the root".to_string() } else { format!("at ‘{pointer}’") };
                let detail = format!(
                    "misplaced ‘{MARKS_KEYWORD}’ {place} of the settings schema of {holder}: marks must be written on \
                     a setting or on a value below one"
                );
                (prefix.to_string(), detail)
            }
            marks::Fault::Recursive(path, definition) => {
                let detail = format!(
                    "invalid mark on ‘{path}’ of {holder}: \
                     marks may not be placed inside the recursive definition ‘{definition}’"
                );
                (full(&path), detail)
            }
        };
        let path = if path.is_empty() { feature.to_string() } else { format!("{feature}.{path}") };
        SettingsError { path, kind: SettingsErrorKind::InvalidDeclaration, detail }
    })
}

impl SettingsDecl {
    /// The name of the feature whose settings this declaration describes.
    pub fn feature(&self) -> &FeatureName {
        &self.feature
    }

    /// Every settings key the feature declares, with its binding: the fields of its
    /// configuration first, then those of each option type, in the order
    /// declared.
    pub fn keys(&self) -> impl Iterator<Item = KeyDecl> + '_ {
        self.parts
            .iter()
            .flat_map(|part| part.keys.iter().map(|name| KeyDecl { name: name.clone(), bind: part.bind }))
    }

    /// Whether the feature takes settings: whether the declaration has a key
    /// of its own ([`keys`](SettingsDecl::keys)). A declaration that only
    /// declares entries of another feature's keys
    /// ([`entry_data`](SettingsDecl::entry_data)) describes a feature that
    /// takes no settings.
    pub fn takes_settings(&self) -> bool {
        self.keys().next().is_some()
    }

    /// The Rust types the settings are read into, each identified by its option
    /// key and given with its binding, in the order they were added; the
    /// configuration's binding is [`Bind::Registry`]. The registry build
    /// declares each of them as an option type of the feature.
    pub fn option_types(&self) -> impl Iterator<Item = (OptionKey, Bind)> + '_ {
        self.parts.iter().map(|part| (part.option.clone(), part.bind))
    }

    /// The settings key `name`, or `None` when the feature does not declare it.
    pub fn key(&self, name: &str) -> Option<KeyDecl> {
        self.keys().find(|key| key.name == name)
    }

    /// The marks on the feature's settings and the values below them, one
    /// [`KeyMark`] per marked path, in the order each path was first marked.
    pub fn marks(&self) -> &[KeyMark] {
        &self.marks
    }

    /// Whether a mark of the declaration sets `mark` on the place `place`, the
    /// key first, then the names below it: a mark's path names the place when
    /// each of its segments is the place's name there, or `*` (see [`KeyMark`]).
    pub fn is_marked(&self, place: &[String], mark: Mark) -> bool {
        self.marks.iter().any(|marks| marks.has(mark) && cascade::names(&marks.path, place))
    }

    /// The marks on the value at `path` (see [`KeyMark`]), or `None` when no
    /// mark is set there.
    pub fn mark_at(&self, path: &str) -> Option<&KeyMark> {
        self.marks.iter().find(|mark| mark.path == path)
    }

    /// Whether the value at `path` carries `mark`.
    pub fn has_mark(&self, path: &str, mark: Mark) -> bool {
        self.mark_at(path).is_some_and(|marked| marked.has(mark))
    }

    fn part_of(&self, key: &str) -> Option<&Part> {
        self.parts.iter().find(|part| part.keys.iter().any(|name| name == key))
    }

    /// Reads the feature's settings from `data`, a map from a settings key to its
    /// value.
    ///
    /// Returns the configuration (its default when none of its keys is
    /// written) and the option values of each option type at least one of whose
    /// keys is written; an option type none of whose keys is written gets no
    /// value. Values of an option type bound to the render request go to
    /// [`FeatureSettings::render_options`]; values of the other option types go
    /// to [`FeatureSettings::document_options`].
    ///
    /// # Errors
    ///
    /// Returns every problem found, each as a [`SettingsError`] that identifies
    /// the key by its path from the feature's name: every key in `data` that the
    /// feature does not declare, and every value that cannot be read as its key
    /// requires.
    pub fn read(&self, data: &Map<String, Value>) -> Result<FeatureSettings, Vec<SettingsError>> {
        let mut errors = Vec::new();
        for key in data.keys() {
            if self.part_of(key).is_none() {
                errors.push(SettingsError {
                    path: format!("{}.{key}", self.feature),
                    kind: SettingsErrorKind::UnknownKey,
                    detail: format!("the feature `{}` has no setting `{key}`", self.feature),
                });
            }
        }
        let mut settings = FeatureSettings {
            feature: self.feature.clone(),
            config: None,
            document: OptionValues::new(),
            render: OptionValues::new(),
        };
        for part in &self.parts {
            let written: Map<String, Value> = data
                .iter()
                .filter(|(key, _)| part.keys.iter().any(|name| name == *key))
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect();
            if written.is_empty() && part.bind != Bind::Registry {
                continue;
            }
            match (part.read)(Value::Object(written)) {
                Ok(ReadValue::Config(config)) => settings.config = Some(config),
                Ok(ReadValue::Option(set)) => {
                    let values = match part.bind {
                        Bind::Render => &mut settings.render,
                        _ => &mut settings.document,
                    };
                    set(values);
                }
                Err(error) => {
                    let path = if error.path.is_empty() {
                        self.feature.to_string()
                    } else {
                        format!("{}.{}", self.feature, error.path)
                    };
                    errors.push(SettingsError {
                        detail: format!("the value of `{path}` is invalid: {}", error.detail),
                        path,
                        kind: SettingsErrorKind::InvalidValue,
                    });
                }
            }
        }
        if errors.is_empty() { Ok(settings) } else { Err(errors) }
    }

    /// Declares the feature's option types on `b`, each with its binding and
    /// its documentation, the configuration among them as an option type bound
    /// when the registry is built. The registry build calls this for every
    /// feature that returns a declaration from
    /// [`FlmFeature::settings`](crate::core::registry::FlmFeature::settings),
    /// before it calls the feature's `register`.
    pub(crate) fn declare(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        for part in &self.parts {
            (part.declare)(b, part.bind, part.docs.clone())?;
        }
        Ok(())
    }

    /// Returns the JSON Schema of the feature's settings.
    ///
    /// The schema is an object whose description joins the descriptions given
    /// with each Rust type of the declaration. Its properties are the feature's
    /// settings keys, each with the description from its field's Rust
    /// documentation and its binding under `x-flm-bind`; no other property is
    /// allowed. The schemas of the types the keys are read into are under
    /// `$defs`.
    ///
    /// Every mark ([`marks`](SettingsDecl::marks)) is written on the node at
    /// its path, under [`MARKS_KEYWORD`] (`x-flm-marks`), as the list of the
    /// mark names; the default of an object combined key by key is its default
    /// entries ([`default_entries`](SettingsDecl::default_entries)). A mark is
    /// written at no other place: where the path of a mark passes through a
    /// definition of `$defs` that another place refers to as well, and the
    /// marks below the two places differ, the definition is written out in
    /// place along that path; otherwise the marks are written in the
    /// definition.
    ///
    /// For a part declared from data, each key's schema is the one its
    /// `properties` give, with its binding added, and the part's own `$defs`
    /// join those of the result; its description is the `docs` given.
    ///
    /// The schema of each entry in force ([`with_entries`](SettingsDecl::with_entries))
    /// stands at its place: each map on the way to it gets the property of the
    /// entry's name beside its `additionalProperties`, and the entry's
    /// property is the entry's schema, with its marks and those that this
    /// declaration sets on the entry's place; the entry's own `$defs` join
    /// those of the result, each named by the entry's path, a `.` and its own
    /// name.
    ///
    /// Every description is the Rust documentation turned into plain text, for
    /// any reader of the schema: a link is written as its words alone, without
    /// its target; a code block is left out, and so is everything from the
    /// first heading on (`# Examples`, `# Panics`). The rest is kept as written,
    /// including code spans in backticks.
    pub fn json_schema(&self) -> Value {
        let mut root = self.bare_schema();
        for entry in &self.in_force {
            marks::graft(&mut root, &entry.path, &entry.schema);
        }
        marks::print(&mut root, &self.marks, |path| self.default_entries(path));
        plain_descriptions(&mut root);
        root
    }

    /// The JSON Schema of the feature's settings, as
    /// [`json_schema`](SettingsDecl::json_schema) gives it before the marks
    /// are written and the descriptions made plain text.
    fn bare_schema(&self) -> Value {
        let mut generator = schemars::SchemaGenerator::default();
        let mut properties = Map::new();
        for part in &self.parts {
            let schema = (part.schema)(&mut generator);
            let resolved = resolve_ref(&generator, schema.as_value());
            let fields = resolved.get("properties").and_then(Value::as_object);
            for key in &part.keys {
                let mut property =
                    fields.and_then(|fields| fields.get(key)).cloned().unwrap_or(Value::Bool(true));
                if let Value::Object(object) = &mut property {
                    object.insert("x-flm-bind".into(), Value::from(part.bind.label()));
                } else {
                    let mut object = Map::new();
                    object.insert("x-flm-bind".into(), Value::from(part.bind.label()));
                    property = Value::Object(object);
                }
                properties.insert(key.clone(), property);
            }
        }
        let mut root = Map::new();
        let docs: Vec<&str> = self.parts.iter().map(|part| part.docs.as_str()).filter(|docs| !docs.is_empty()).collect();
        if !docs.is_empty() {
            root.insert("description".into(), Value::from(docs.join("\n\n")));
        }
        root.insert("type".into(), Value::from("object"));
        root.insert("properties".into(), Value::Object(properties));
        root.insert("additionalProperties".into(), Value::Bool(false));
        let definitions = generator.take_definitions(true);
        if !definitions.is_empty() {
            root.insert("$defs".into(), Value::Object(definitions));
        }
        Value::Object(root)
    }
}

impl fmt::Debug for SettingsDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SettingsDecl")
            .field("feature", &self.feature)
            .field("keys", &self.keys().map(|key| key.name).collect::<Vec<_>>())
            .field("marks", &self.marks)
            .finish()
    }
}

/// Returns the JSON Schema of the type `T`, written as
/// [`SettingsDecl::json_schema`] writes the schemas of a feature's keys: every
/// description is the Rust documentation of the type and its fields as plain
/// text, and the schemas of the types `T` refers to are under `$defs`.
///
/// Use it to describe, beside the features' settings, another part of a file
/// that a program reads into a type of this library (for example the parsing
/// configuration, [`FlmParsing`](crate::lang::FlmParsing)).
pub fn schema_of<T: schemars::JsonSchema>() -> Value {
    let mut schema = schemars::SchemaGenerator::default().into_root_schema_for::<T>().to_value();
    if let Value::Object(object) = &mut schema {
        object.remove("$schema");
        object.remove("title");
    }
    plain_descriptions(&mut schema);
    schema
}

/// Writes every description of `schema` as plain text ([`plain_text`]),
/// leaving alone the values that are data rather than schemas.
fn plain_descriptions(schema: &mut Value) {
    match schema {
        Value::Object(object) => {
            for (keyword, value) in object.iter_mut() {
                match (keyword.as_str(), value) {
                    ("description", Value::String(text)) => *text = plain_text(text),
                    ("default" | "const" | "enum" | "examples", _) => {}
                    (keyword, _) if keyword.starts_with("x-flm-") => {}
                    (_, value) => plain_descriptions(value),
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(plain_descriptions),
        _ => {}
    }
}

/// Returns a Rust documentation text, written in Markdown, as plain text: see
/// [`SettingsDecl::json_schema`].
fn plain_text(doc: &str) -> String {
    let mut lines = Vec::new();
    let mut in_code_block = false;
    for line in doc.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            continue;
        }
        if in_code_block {
            continue;
        }
        if trimmed.starts_with('#') && trimmed.trim_start_matches('#').starts_with(' ') {
            break;
        }
        lines.push(line);
    }
    let mut text = String::new();
    for line in unlinked(&lines.join("\n")).lines() {
        // One blank line between paragraphs, none at either end.
        if line.trim().is_empty() && (text.is_empty() || text.ends_with("\n\n")) {
            continue;
        }
        text.push_str(line);
        text.push('\n');
    }
    text.trim_end().to_string()
}

/// Returns `text` with every Markdown link written as its words alone:
/// `[words](target)`, `[words][reference]`, and `` [`code`] `` (a link to the
/// item the code refers to). Code spans are copied unchanged; a bracket that
/// opens no link is kept.
fn unlinked(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '`' {
            let end = code_span_end(&chars, i);
            out.extend(&chars[i..end]);
            i = end;
        } else if chars[i] == '[' {
            let Some(close) = closing(&chars, i, '[', ']') else {
                out.push('[');
                i += 1;
                continue;
            };
            let words: String = chars[i + 1..close].iter().collect();
            let target = match chars.get(close + 1) {
                Some('(') => closing(&chars, close + 1, '(', ')'),
                Some('[') => closing(&chars, close + 1, '[', ']'),
                _ if words.starts_with('`') && words.ends_with('`') => Some(close),
                _ => None,
            };
            match target {
                Some(end) => {
                    out.push_str(&unlinked(&words));
                    i = end + 1;
                }
                None => {
                    out.push('[');
                    i += 1;
                }
            }
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// The position just after the code span that opens at `start` (a run of
/// backticks, closed by the next run of the same length), or just after the
/// run when nothing closes it.
fn code_span_end(chars: &[char], start: usize) -> usize {
    let run = |at: usize| chars[at..].iter().take_while(|c| **c == '`').count();
    let length = run(start);
    let mut at = start + length;
    while at < chars.len() {
        if chars[at] == '`' {
            let found = run(at);
            if found == length {
                return at + found;
            }
            at += found;
        } else {
            at += 1;
        }
    }
    start + length
}

/// The position of the `close` matching the `open` at `start`, code spans
/// and nested pairs skipped, if there is one.
fn closing(chars: &[char], start: usize, open: char, close: char) -> Option<usize> {
    let mut depth = 0;
    let mut at = start;
    while at < chars.len() {
        let c = chars[at];
        if c == '`' {
            at = code_span_end(chars, at);
            continue;
        }
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                return Some(at);
            }
        }
        at += 1;
    }
    None
}

/// The schema that a `$ref` refers to, or the schema itself when it is not a
/// reference.
fn resolve_ref(generator: &schemars::SchemaGenerator, schema: &Value) -> Value {
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let name = reference.rsplit('/').next().unwrap_or(reference);
        if let Some(found) = generator.definitions().get(name) {
            return found.clone();
        }
    }
    schema.clone()
}

/// The keys of a part declared from data: the names of the `properties` of
/// `schema`, in their order, or `None` when `schema` has no `properties` map.
fn schema_keys(schema: &Value) -> Option<Vec<String>> {
    schema.get("properties").and_then(Value::as_object).map(|properties| properties.keys().cloned().collect())
}

/// The function that declares a part declared from data as the option type
/// `name`.
fn declare_named(name: Name) -> DeclareFn {
    Arc::new(move |b: &mut FeatureBuilder<'_>, bind: Bind, docs: String| b.options_named(name.clone(), bind, docs))
}

/// The schema function of a part declared from data: the schema as given, its
/// `$defs` moved into the generator's definitions, where those of every part
/// join.
fn data_schema(schema: Value) -> SchemaFn {
    Arc::new(move |generator: &mut schemars::SchemaGenerator| {
        let mut schema = schema.clone();
        if let Some(Value::Object(definitions)) = schema.as_object_mut().and_then(|object| object.remove("$defs")) {
            generator.definitions_mut().extend(definitions);
        }
        schemars::Schema::try_from(schema).unwrap_or_default()
    })
}

/// Reads `T` from `data`; the error carries the path of the offending key.
fn deserialize<T: DeserializeOwned>(data: Value) -> Result<T, ReadError> {
    serde_path_to_error::deserialize::<_, T>(data).map_err(|error| {
        let path = error.path().to_string();
        let path = if path == "." { String::new() } else { path };
        ReadError { path, detail: error.into_inner().to_string() }
    })
}

/// The names of the fields `T` is read from, as its `Deserialize`
/// implementation declares them.
///
/// # Panics
///
/// When `T` is not read from a map of named fields.
fn field_names<T: DeserializeOwned>() -> Vec<String> {
    // The key list comes from the derivation that reads the keys, so the two
    // cannot disagree.
    let mut probe = FieldProbe { fields: None };
    let _ = T::deserialize(&mut probe);
    probe
        .fields
        .unwrap_or_else(|| panic!("`{}` is not read from named fields", std::any::type_name::<T>()))
        .iter()
        .map(|name| (*name).to_string())
        .collect()
}

/// A deserializer that reads nothing and records the field names that a struct's
/// `Deserialize` implementation passes to it.
struct FieldProbe {
    fields: Option<&'static [&'static str]>,
}

/// The only error of [`FieldProbe`].
#[derive(Debug)]
struct ProbeError;

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("field probe")
    }
}

impl std::error::Error for ProbeError {}

impl serde::de::Error for ProbeError {
    fn custom<T: fmt::Display>(_msg: T) -> ProbeError {
        ProbeError
    }
}

impl<'de> serde::Deserializer<'de> for &mut FieldProbe {
    type Error = ProbeError;

    fn deserialize_any<V: serde::de::Visitor<'de>>(self, _visitor: V) -> Result<V::Value, ProbeError> {
        Err(ProbeError)
    }

    fn deserialize_struct<V: serde::de::Visitor<'de>>(
        self,
        _name: &'static str,
        fields: &'static [&'static str],
        _visitor: V,
    ) -> Result<V::Value, ProbeError> {
        self.fields = Some(fields);
        Err(ProbeError)
    }

    serde::forward_to_deserialize_any! {
        bool i8 i16 i32 i64 i128 u8 u16 u32 u64 u128 f32 f64 char str string
        bytes byte_buf option unit unit_struct newtype_struct seq tuple
        tuple_struct map enum identifier ignored_any
    }
}

/// A feature's settings as read from data by [`SettingsDecl::read`]: its
/// configuration, and the option values to set on a document and on a render
/// request.
///
/// A host passes the configuration to the feature's package through
/// [`PackageSettings`], sets the document option values on the document and the
/// render option values on the plan request.
#[derive(Clone)]
pub struct FeatureSettings {
    feature: FeatureName,
    config: Option<Arc<dyn Any + Send + Sync>>,
    document: OptionValues,
    render: OptionValues,
}

impl FeatureSettings {
    /// The name of the feature whose settings these are.
    pub fn feature(&self) -> &FeatureName {
        &self.feature
    }

    /// The feature's configuration, or `None` when the feature declares no
    /// configuration of type `C`. The configuration is the type's default when
    /// none of its keys was written.
    pub fn config<C: Any>(&self) -> Option<&C> {
        self.config.as_deref().and_then(|config| config.downcast_ref::<C>())
    }

    /// The option values a host sets on the document
    /// ([`Doc::with_options`](crate::model::doc::Doc::with_options)): those of
    /// every option type bound to the document ([`Bind::Document`]) or to either
    /// site ([`Bind::Either`]) at least one of whose keys was written.
    pub fn document_options(&self) -> &OptionValues {
        &self.document
    }

    /// The option values a host sets on the render request
    /// ([`PlanRequest::render_options`](crate::core::plan::PlanRequest#structfield.render_options)):
    /// those of every option type bound to the render request
    /// ([`Bind::Render`]) at least one of whose keys was written.
    pub fn render_options(&self) -> &OptionValues {
        &self.render
    }
}

impl fmt::Debug for FeatureSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FeatureSettings")
            .field("feature", &self.feature)
            .field("config", &self.config.is_some())
            .field("document", &self.document)
            .field("render", &self.render)
            .finish()
    }
}

/// The kind of problem a [`SettingsError`] reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SettingsErrorKind {
    /// The feature declares no settings key of that name.
    UnknownKey,
    /// The value does not have the shape its key requires, or the feature's
    /// conversion or reader of its configuration refuses it
    /// ([`SettingsDecl::config_from`], [`SettingsDecl::config_data_read`]).
    InvalidValue,
    /// A declaration of settings from data contradicts itself or is malformed
    /// ([`SettingsDecl::config_data`], [`SettingsDecl::config_data_read`], [`SettingsDecl::options_data`]):
    /// no data was read.
    InvalidDeclaration,
}

/// A problem found while reading a feature's settings from data: the key, by its
/// path from the feature's name, and what is wrong with its value; or a problem
/// of a declaration of settings from data.
///
/// [`SettingsDecl::read`] returns these, and so do the constructors of the parts
/// declared from data ([`SettingsDecl::config_data`] and the others). The `Display` text is
/// [`detail`](SettingsError::detail); it does not say where the value was
/// written, which the host adds when it reports the error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettingsError {
    /// The key's path from the feature's name, its segments separated by `.`
    /// (`html.heading_tags.section`); the feature's name alone when the
    /// problem concerns the settings as a whole.
    pub path: String,
    /// The kind of problem.
    pub kind: SettingsErrorKind,
    /// The problem, as a message for a person.
    pub detail: String,
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for SettingsError {}

/// The settings a package is built from: the settings of each of its features
/// that takes some, and the features switched off.
///
/// A package is a named bundle of features together with the packages it builds
/// on, ready to register ([`PackageEntry`](crate::core::registry::PackageEntry)); a
/// [`PackageFactory`](crate::core::registry::PackageFactory) builds it from these
/// settings with
/// [`PackageFactory::build`](crate::core::registry::PackageFactory::build). A
/// host reads each feature's data with the feature's [`SettingsDecl`] and
/// inserts the result here. The package's build function decides what a
/// feature with no settings here becomes, and leaves every switched-off feature
/// out ([`PackageFactory::new`](crate::core::registry::PackageFactory::new));
/// the standard features are built from their defaults where the settings give none.
#[derive(Debug, Clone, Default)]
pub struct PackageSettings {
    features: BTreeMap<FeatureName, FeatureSettings>,
    off: BTreeSet<FeatureName>,
}

impl PackageSettings {
    /// Creates package settings with no feature settings and no feature switched
    /// off.
    pub fn new() -> PackageSettings {
        PackageSettings::default()
    }

    /// Adds one feature's settings, replacing any settings of the same feature
    /// already added, and returns `&mut self` for chaining.
    pub fn insert(&mut self, settings: FeatureSettings) -> &mut PackageSettings {
        self.features.insert(settings.feature.clone(), settings);
        self
    }

    /// Switches the feature named `feature` off, and returns `&mut self` for
    /// chaining. The package's build function leaves a switched-off feature out
    /// of the package
    /// ([`PackageFactory::new`](crate::core::registry::PackageFactory::new)).
    pub fn switch_off(&mut self, feature: impl Into<FeatureName>) -> &mut PackageSettings {
        self.off.insert(feature.into());
        self
    }

    /// Whether the feature named `feature` is switched off.
    pub fn is_off(&self, feature: &str) -> bool {
        self.off.contains(feature)
    }

    /// The settings of the feature named `feature`, or `None` when none were
    /// added. A reading through the cascade
    /// ([`Cascade::read`](cascade::Cascade::read)) adds the settings of every
    /// feature in use that declares some and is not switched off, whether or
    /// not anything is written under it.
    pub fn feature(&self, feature: &str) -> Option<&FeatureSettings> {
        self.features.get(feature)
    }

    /// The configuration of type `C` of the feature named `feature`, or `None`
    /// when no settings were added for that feature or they hold no
    /// configuration of that type. After a reading through the cascade
    /// ([`Cascade::read`](cascade::Cascade::read)), it is `Some` for every
    /// feature in use, not switched off, that declares a configuration of type
    /// `C`: the configuration read from the settings written under the
    /// feature, or from the empty map when nothing is written under it.
    pub fn config<C: Any>(&self, feature: &str) -> Option<&C> {
        self.feature(feature).and_then(FeatureSettings::config::<C>)
    }
}
