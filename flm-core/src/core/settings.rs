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
//! on some keys ([`KeyMark`]). Every field of those types is one settings key of
//! the feature: the keys of the `html` feature, for example, are the fields of
//! its render options, `title`, `lang`, `heading_tags` and so on. The declaration
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
//! # Collections of named things
//!
//! A collection of named things, such as the xnote kinds or the float types, is
//! written in data as a map from a name to an entry, in the order written;
//! [`named`] provides the function that reads such a field
//! ([`named::deserialize`]). Its feature marks
//! the key as merged entry by entry ([`SettingsDecl::merge_entries`]) and gives
//! its default entries as data ([`SettingsDecl::default_entries`]). A host that
//! combines several sources of settings combines such a key one entry at a time,
//! with the default entries as the lowest source, instead of letting one
//! source's value replace another's whole. This module reads one map per
//! feature, already combined.
//!
//! # Settings declared from data
//!
//! A feature whose settings have no Rust types, such as a feature written in a
//! script, declares them from data instead: its configuration with
//! [`SettingsDecl::config_data`] and each option type with
//! [`SettingsDecl::options_data`], each from a JSON Schema of an object, and a
//! key merged entry by entry with [`SettingsDecl::merge_entries_data`]. Such a
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
//! - **The schema.** [`SettingsDecl::json_schema`] takes the keys' schemas from
//!   the schema given, with the binding and the marks added as for a Rust type;
//!   the schema's own `$defs` join those of the result.
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
//! over the packages it uses and its peers must agree, a key marked merged entry
//! by entry is combined one entry at a time over its default entries, and the
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
use std::sync::Arc;

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::core::registry::{BuildError, FeatureBuilder};
use crate::model::names::{FeatureName, Name};
use crate::model::options::{Bind, DataValue, OptionKey, Options, OptionValues};

pub mod cascade;
pub mod named;

/// The message for a merge mark set on `path`, a path below a key of the
/// feature `feature`: one text for [`SettingsDecl::merge_entries`]'s panic and
/// [`SettingsDecl::merge_entries_data`]'s error.
fn merge_path_detail(feature: &FeatureName, path: &str) -> String {
    format!(
        "the feature `{feature}` marks `{path}` as merged entry by entry, and only a setting itself \
         can be marked so, not a path below one"
    )
}

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

/// What a settings key's value means, beyond its shape: plain data, text in FLM
/// markup, or a path to a file.
///
/// A feature sets it on a key with a [`KeyMark`]. The kind says how the value
/// is meant; the value itself is read as the field's Rust type says. A host
/// uses the kind, for example to resolve a relative path against the folder of
/// the file in which it was written before passing the value on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ValueKind {
    /// Plain data: a number, a switch, a name, a map.
    Data,
    /// Text written in FLM markup, which the feature parses.
    Markup,
    /// A path to a file.
    Path,
}

impl ValueKind {
    /// The kind's name as the JSON Schema writes it: `data`, `markup` or
    /// `path`.
    pub fn label(self) -> &'static str {
        match self {
            ValueKind::Data => "data",
            ValueKind::Markup => "markup",
            ValueKind::Path => "path",
        }
    }
}

/// A mark a feature sets on one of its settings keys, or on a value at any depth
/// below one: whether two sources of settings combine the key's value entry by
/// entry, and what the value means ([`ValueKind`]).
///
/// A feature sets marks with [`SettingsDecl::merge_entries`],
/// [`SettingsDecl::markup`] and [`SettingsDecl::path`]; a host reads them with
/// [`SettingsDecl::marks`] and [`SettingsDecl::mark_at`], and the JSON Schema
/// lists them under `x-flm-marks`.
///
/// The marked value is identified by its path below the feature's name, its
/// segments joined by `.`. The segment `*` means every entry of a map and every
/// item of a list (`kinds.*.words`, a key of the theorems feature, marks the
/// `words` of every theorem kind, `labels.*`
/// every item of the list `labels`); a variant of an enumeration written as a
/// map of one key is a segment of its own (`nesting.template`). A key merged
/// entry by entry is always a settings key itself, a path of one segment (see
/// [`merge_entries`](SettingsDecl::merge_entries)); deeper paths and the segment
/// `*` serve the marks that say what a value means.
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
    /// Whether two sources of settings combine the value, a map from a name to
    /// an entry, entry by entry, instead of a higher-ranked source replacing it
    /// whole. Only a settings key itself, a path of one segment, is marked so.
    pub merge_entries: bool,
    /// What the value means.
    pub kind: ValueKind,
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
type ReadFn = Arc<dyn Fn(Value) -> Result<ReadValue, PartError> + Send + Sync>;

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

/// A failure to read one [`Part`]: the path of the offending key below the
/// feature's name, and a message.
struct PartError {
    path: String,
    detail: String,
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
/// any number of option types ([`options`](SettingsDecl::options)) and marks
/// ([`merge_entries`](SettingsDecl::merge_entries),
/// [`markup`](SettingsDecl::markup), [`path`](SettingsDecl::path)), each method
/// taking and returning the declaration. Two types that declare the same key
/// make these methods panic: a declaration that contradicts itself is a mistake
/// in the feature's code. The parts declared from data
/// ([`config_data`](SettingsDecl::config_data),
/// [`options_data`](SettingsDecl::options_data),
/// [`merge_entries_data`](SettingsDecl::merge_entries_data)) return the same
/// conditions as errors instead. See the [module documentation](self).
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
}

/// A function that returns the default entries of a key merged entry by entry:
/// the entries the key holds when no source of settings writes it.
///
/// The function returns the entries as data, a map from each entry's name to the
/// rest of the entry, or a text saying why they cannot be written as data.
/// [`SettingsDecl::merge_entries`] calls it once, when the mark is set.
pub type DefaultEntries = fn() -> Result<Value, String>;

impl SettingsDecl {
    /// Creates the declaration of the settings of the feature named `feature`,
    /// with no key yet.
    pub fn new(feature: impl Into<FeatureName>) -> SettingsDecl {
        SettingsDecl { feature: feature.into(), parts: Vec::new(), marks: Vec::new(), defaults: Vec::new() }
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
    /// When a field of `C` has the name of a key already declared, when the
    /// declaration already has a configuration, or when `C` is not read from a
    /// map of named fields.
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
    /// When a field of `D` has the name of a key already declared, when the
    /// declaration already has a configuration, or when `D` is not read from a
    /// map of named fields.
    pub fn config_from<D, C>(self, docs: impl Into<String>) -> SettingsDecl
    where
        D: SettingsData,
        C: Options + TryFrom<D>,
        <C as TryFrom<D>>::Error: fmt::Display,
    {
        fn read<D, C>(data: Value) -> Result<ReadValue, PartError>
        where
            D: SettingsData,
            C: Options + TryFrom<D>,
            <C as TryFrom<D>>::Error: fmt::Display,
        {
            let value = deserialize::<D>(data)?;
            let config = C::try_from(value)
                .map_err(|error| PartError { path: String::new(), detail: error.to_string() })?;
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
    /// [`PackageSettings::config`]`::<DataValue>`.
    ///
    /// The schema's own `$defs` join those of the declaration's schema
    /// ([`json_schema`](SettingsDecl::json_schema)): a `$defs` name shared with
    /// another part of the declaration refers to one definition.
    ///
    /// # Errors
    ///
    /// A [`SettingsError`] of the kind [`SettingsErrorKind::InvalidDeclaration`]
    /// when `schema` has no `properties` map, when one of its properties is a
    /// key declared already, or when the declaration has a configuration
    /// already.
    pub fn config_data(self, schema: Value, docs: impl Into<String>) -> Result<SettingsDecl, SettingsError> {
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
            read: Arc::new(|data: Value| Ok(ReadValue::Config(Arc::new(DataValue(data))))),
            declare: declare_named(name),
            schema: data_schema(schema),
        })
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
            schema: data_schema(schema),
        })
    }

    /// Adds the option type `O` of the feature, with the binding `bind` and the
    /// description `docs`, and returns the declaration. Every field of `O` is a
    /// settings key.
    ///
    /// # Panics
    ///
    /// When `bind` is [`Bind::Registry`] (a value fixed with the registry is
    /// the feature's configuration: see [`config`](SettingsDecl::config)), when
    /// a field of `O` has the name of a key already declared, or when `O` is
    /// not read from a map of named fields.
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
        fn read<O: SettingsData + Options>(data: Value) -> Result<ReadValue, PartError> {
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

    /// Adds `part`, or returns the contradiction it makes with the parts
    /// already declared: a key declared twice, or a second configuration.
    fn try_part(mut self, part: Part) -> Result<SettingsDecl, SettingsError> {
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
        Ok(self)
    }

    /// An [`InvalidDeclaration`](SettingsErrorKind::InvalidDeclaration) error
    /// at `path` below the feature's name (the feature's name alone when `path`
    /// is empty).
    fn invalid(&self, path: String, detail: String) -> SettingsError {
        let path = if path.is_empty() { self.feature.to_string() } else { format!("{}.{path}", self.feature) };
        SettingsError { path, kind: SettingsErrorKind::InvalidDeclaration, detail }
    }


    /// Marks the settings key `key` as merged entry by entry, and returns the
    /// declaration: its value is a map from a name to an entry, and a host that
    /// combines two sources of settings combines that map one entry at a time.
    ///
    /// The key is one of the feature's own settings keys, never a path below
    /// one: a host combines the feature's keys, and combines nothing inside a
    /// key's value except the entries of a key marked so.
    ///
    /// `default_entries` returns, as data, the entries the key holds when no
    /// source writes it: the feature's own default. A host treats them as the
    /// lowest-ranked source, so an entry a source writes replaces the default
    /// entry of that name, and the default entries no source writes stay. The
    /// function usually writes the default of the key's Rust field as data
    /// ([`named::to_data`] for a list of named entries). Read them back with
    /// [`default_entries`](SettingsDecl::default_entries).
    ///
    /// # Panics
    ///
    /// When `key` is a path below a key (it contains a `.` or a `*`), and when
    /// `default_entries` fails or returns something other than a map: both are
    /// mistakes in the feature's code, not in any data.
    pub fn merge_entries(self, key: impl Into<String>, default_entries: DefaultEntries) -> SettingsDecl {
        let path = key.into();
        if path.contains('.') || path.contains('*') {
            panic!("{}", merge_path_detail(&self.feature, &path));
        }
        let entries = match default_entries() {
            Ok(Value::Object(entries)) => entries,
            Ok(other) => panic!(
                "the default entries of `{}.{path}` must be written as a map, not `{other}`",
                self.feature
            ),
            Err(detail) => {
                panic!("the default entries of `{}.{path}` cannot be written as data: {detail}", self.feature)
            }
        };
        self.merge_mark(path, entries)
    }

    /// Marks the settings key `key` as merged entry by entry, with the default
    /// entries `default_entries` given as data, and returns the declaration:
    /// [`merge_entries`](SettingsDecl::merge_entries) for a feature whose
    /// settings are declared from data.
    ///
    /// # Errors
    ///
    /// A [`SettingsError`] of the kind [`SettingsErrorKind::InvalidDeclaration`]
    /// when `key` is a path below a key (it contains a `.` or a `*`).
    pub fn merge_entries_data(
        self,
        key: impl Into<String>,
        default_entries: Map<String, Value>,
    ) -> Result<SettingsDecl, SettingsError> {
        let path = key.into();
        if path.contains('.') || path.contains('*') {
            let detail = merge_path_detail(&self.feature, &path);
            return Err(self.invalid(path, detail));
        }
        Ok(self.merge_mark(path, default_entries))
    }

    /// Sets the merge mark on `path` and records its default entries.
    fn merge_mark(self, path: String, entries: Map<String, Value>) -> SettingsDecl {
        let mut decl = self.mark(path.clone(), true, None);
        decl.defaults.retain(|(marked, _)| *marked != path);
        decl.defaults.push((path, entries));
        decl
    }

    /// The default entries of the settings key `path`, marked as merged entry by
    /// entry: the entries the key holds when no source of settings writes it, as
    /// a map from each entry's name to the rest of the entry, in the feature's
    /// order. Returns `None` when the key is not marked as merged entry by
    /// entry.
    pub fn default_entries(&self, path: &str) -> Option<&Map<String, Value>> {
        self.defaults.iter().find(|(marked, _)| marked == path).map(|(_, entries)| entries)
    }

    /// Marks the value at `path` as text in FLM markup ([`ValueKind::Markup`])
    /// and returns the declaration. See [`KeyMark`] for the form of the path.
    pub fn markup(self, path: impl Into<String>) -> SettingsDecl {
        self.mark(path.into(), false, Some(ValueKind::Markup))
    }

    /// Marks the value at `path` as a path to a file ([`ValueKind::Path`]) and
    /// returns the declaration. See [`KeyMark`] for the form of the path.
    pub fn path(self, path: impl Into<String>) -> SettingsDecl {
        self.mark(path.into(), false, Some(ValueKind::Path))
    }

    fn mark(mut self, path: String, merge: bool, kind: Option<ValueKind>) -> SettingsDecl {
        if let Some(mark) = self.marks.iter_mut().find(|mark| mark.path == path) {
            mark.merge_entries |= merge;
            if let Some(kind) = kind {
                mark.kind = kind;
            }
        } else {
            self.marks.push(KeyMark {
                path,
                merge_entries: merge,
                kind: kind.unwrap_or(ValueKind::Data),
            });
        }
        self
    }

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

    /// The marks on the feature's settings keys, in the order they were first
    /// set.
    pub fn marks(&self) -> &[KeyMark] {
        &self.marks
    }

    /// The mark on the value at `path` (see [`KeyMark`]), or `None` when no mark
    /// is set there.
    pub fn mark_at(&self, path: &str) -> Option<&KeyMark> {
        self.marks.iter().find(|mark| mark.path == path)
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
    /// allowed. The default of a key merged entry by entry is its default
    /// entries ([`default_entries`](SettingsDecl::default_entries)). The marks
    /// are listed under `x-flm-marks`, one object per mark with its `path`,
    /// `merge_entries` and `kind`. The schemas of the types the keys are read
    /// into are under `$defs`.
    ///
    /// For a part declared from data, each key's schema is the one its
    /// `properties` give, with its binding added, and the part's own `$defs`
    /// join those of the result; its description is the `docs` given.
    ///
    /// Every description is the Rust documentation turned into plain text, for
    /// any reader of the schema: a link is written as its words alone, without
    /// its target; a code block is left out, and so is everything from the
    /// first heading on (`# Examples`, `# Panics`). The rest is kept as written,
    /// including code spans in backticks.
    pub fn json_schema(&self) -> Value {
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
                // A key merged entry by entry states its default entries as
                // data, which is what the schema's default is.
                if let Some(entries) = self.default_entries(key) {
                    if let Value::Object(object) = &mut property {
                        object.insert("default".into(), Value::Object(entries.clone()));
                    }
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
        if !self.marks.is_empty() {
            let marks = self
                .marks
                .iter()
                .map(|mark| {
                    serde_json::json!({
                        "path": mark.path,
                        "merge_entries": mark.merge_entries,
                        "kind": mark.kind.label(),
                    })
                })
                .collect();
            root.insert("x-flm-marks".into(), Value::Array(marks));
        }
        let definitions = generator.take_definitions(true);
        if !definitions.is_empty() {
            root.insert("$defs".into(), Value::Object(definitions));
        }
        let mut root = Value::Object(root);
        plain_descriptions(&mut root);
        root
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
fn deserialize<T: DeserializeOwned>(data: Value) -> Result<T, PartError> {
    serde_path_to_error::deserialize::<_, T>(data).map_err(|error| {
        let path = error.path().to_string();
        let path = if path == "." { String::new() } else { path };
        PartError { path, detail: error.into_inner().to_string() }
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
    // cannot disagree. See decision rationale at [§flm-dd-dr:feat-settings-read]
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
    /// conversion of its configuration refuses it.
    InvalidValue,
    /// A declaration of settings from data contradicts itself or is malformed
    /// ([`SettingsDecl::config_data`], [`SettingsDecl::options_data`],
    /// [`SettingsDecl::merge_entries_data`]): no data was read.
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
/// the standard package builds a feature with no settings from its defaults.
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
    /// added.
    pub fn feature(&self, feature: &str) -> Option<&FeatureSettings> {
        self.features.get(feature)
    }

    /// The configuration of type `C` of the feature named `feature`, or `None`
    /// when no settings were added for that feature or they hold no
    /// configuration of that type.
    pub fn config<C: Any>(&self, feature: &str) -> Option<&C> {
        self.feature(feature).and_then(FeatureSettings::config::<C>)
    }
}
