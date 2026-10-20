//! Option types: the typed values a feature lets a caller set on a document or
//! on a render request, and how a stage reads them.
//!
//! An option type is a plain Rust struct a feature declares once. Its values
//! are set on a document or on a render request, and the type's binding says
//! which; a render request is the set of option values that a [plan
//! request](crate::core::plan::PlanRequest) carries
//! ([`PlanRequest::render_options`](crate::core::plan::PlanRequest::render_options())).
//! A stage reads the value for an option type it declared a read of: the
//! render value if one is set, otherwise the document value, otherwise the
//! type's default. An **option value** is a value set for an option type.
//! Option values differ from a feature's configuration, which is fixed when the
//! registry is built (see [settings, options and
//! configuration](crate::guide::concepts_overview#settings-options-and-configuration)).
//!
//! # Declaring an option type
//!
//! A feature declares an option type with
//! [`FeatureBuilder::options`](crate::core::registry::FeatureBuilder::options)
//! (or [`options_with_docs`](crate::core::registry::FeatureBuilder::options_with_docs)),
//! giving its binding ([`Bind`]). The Rust type identifies the option type:
//! options have no names of their own. A feature declares one struct per
//! binding, and a feature with more options gives that struct more fields. Any
//! type that implements `Default`, `Hash` and `Debug` and can be shared between
//! threads can be an option type ([`Options`]).
//!
//! # Option types declared from data
//!
//! A feature whose settings are given as data rather than as Rust types (a
//! feature written in a script, for example) declares its option types by name,
//! with [`SettingsDecl::options_data`](crate::core::settings::SettingsDecl::options_data)
//! (and its configuration with
//! [`SettingsDecl::config_data`](crate::core::settings::SettingsDecl::config_data),
//! named by the feature's name). Such an option type is identified by its name
//! ([`OptionKey::Data`]), and its values are [`DataValue`]s, the data as
//! written; its default is the empty map. Everything above holds for it alike:
//! its binding, the resolution of a read, the three mistakes. A stage declares a
//! read of one with
//! [`Needs::option_named`](crate::core::stage::Needs::option_named) and reads it
//! with [`StageCx::option_named`](crate::core::run::StageCx::option_named); a
//! caller sets a value with [`OptionValues::set_named`].
//!
//! # Setting values and reading them
//!
//! A caller sets option values in an [`OptionValues`] collection, in one of two
//! places: on the document, with
//! [`Doc::with_options`](crate::model::doc::Doc::with_options), or on the render
//! request, with
//! [`PlanRequest::render_options`](crate::core::plan::PlanRequest::render_options()).
//! The option type's [`Bind`] says which of the two places may set it.
//!
//! A stage, a rewrite stage or a lowering rule declares each option type it
//! reads, with [`Needs::option`](crate::core::stage::Needs::option) or
//! [`LoweringDecl::option`](crate::core::lowering::LoweringDecl::option).
//! Before the stage runs, each declared read is resolved once into
//! [`ResolvedOptions`]: the value set on the render request if there is one,
//! otherwise the value set on the document if there is one, otherwise the
//! type's [`Default`]. The stage then reads the value from its context, with
//! [`StageCx::option`](crate::core::run::StageCx::option). The same rule
//! applies everywhere, so no feature defines an order of its own.
//!
//! Three mistakes are reported as an [`OptionError`] rather than ignored:
//!
//! - reading an option type that the stage did not declare, reported when the
//!   stage reads it;
//! - setting a value for an option type in a place its binding does not admit,
//!   or for an option type that no feature declares, reported when the values
//!   are checked ([`validate_values`]): when a run is constructed, for the
//!   values set on the document, and when a plan is built, for the values set
//!   on the render request;
//! - declaring a read of an option type whose binding is [`Bind::Registry`],
//!   reported when a plan is built ([`OptionError::RegistryBoundRead`]).
//!
//! # Why reads and bindings are declared
//!
//! Because each stage declares the option types it reads, the library knows,
//! without running anything, which option values a stage's output can depend
//! on. The stamp of a run ([`Stamp`](crate::core::run::Stamp)), which records
//! what the values the run produces were produced under, uses this: it holds a
//! digest of the option values that the rewrite stages read
//! ([`ResolvedOptions::digest`]), which is why every option type must
//! implement [`Hash`]. Because each option type declares where its values are
//! set, a plan depends only on the render option values of its plan request,
//! whose digest is part of the plan's identity
//! ([`PlanKey`](crate::core::plan::PlanKey)), and never on the values set on
//! any one document.
//!
//! # Items
//!
//! - [`Bind`] and [`BindSite`] — the bindings, and the two places where values
//!   are set.
//! - [`Options`] — the requirements an option type meets; [`OptionKey`] — the
//!   identifier of an option type, its Rust type or the name it was declared
//!   under from data.
//! - [`DataValue`] — the value of an option type declared from data;
//!   [`hash_value`] — how such a value is hashed.
//! - [`OptionDecl`] — a feature's declaration of an option type;
//!   [`OptionRef`] — a stage's declaration that it reads one.
//! - [`OptionValues`] — the values set in one place; [`validate_values`] —
//!   checking them against the declarations.
//! - [`ResolvedOptions`] — one stage's declared reads, resolved to values.
//! - [`OptionError`] — every error about options.
//! - [`TypeKey`] and [`short_type_name`] — the identifier of a Rust type, and
//!   its short name as messages show it.
//!
//! # Position of this module
//!
//! This module is part of [`crate::model`], beside the document, rather than
//! of [`crate::core`], which reads the values: a [`Doc`](crate::model::doc::Doc)
//! holds the [`OptionValues`] set on it, so the document model would
//! otherwise depend on the layer above it. This module itself uses only the
//! names of [`crate::model::names`] and the hasher of [`crate::model::digest`].

use std::any::{Any, TypeId};
use std::collections::BTreeMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use serde_json::{Map, Value};

use crate::model::names::{FeatureName, Name, StageName, WorkflowName};
use crate::model::digest::{Fnv1a64, digest_of};

/// The binding of an option type: where a value for it may be set, on the
/// document, on the render request, on both, or nowhere.
///
/// A feature chooses the binding when it declares the option type, and the
/// binding holds for every caller: a caller of the library never chooses one,
/// and an option that must be settable in different places for different
/// purposes is declared as two option types.
/// [`Bind::admits`] says whether a binding lets a value be set in a given
/// place ([`BindSite`]).
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Bind {
    /// Fixed when the registry is built: the value is part of the
    /// configuration the feature was constructed with, and the feature holds
    /// it. No value may be set on the document or on the render request, and
    /// building a plan fails ([`OptionError::RegistryBoundRead`]) when a stage
    /// declares a read of such a type, since the read could only return the
    /// type's default. A feature declares its configuration type with this
    /// binding
    /// ([`SettingsDecl::config`](crate::core::settings::SettingsDecl::config)),
    /// so that the configuration appears in the registry's list of option
    /// declarations
    /// ([`FlmRegistry::options`](crate::core::registry::FlmRegistry::options))
    /// and in the settings schema.
    Registry,
    /// Set on the document ([`Doc::with_options`](crate::model::doc::Doc::with_options)).
    Document,
    /// Set on the render request
    /// ([`PlanRequest::render_options`](crate::core::plan::PlanRequest::render_options())).
    Render,
    /// Set on the document, on the render request, or on both; a value set on
    /// the render request replaces the document's value for the runs of that
    /// plan.
    Either,
}

/// One of the two places where a caller sets option values: the document or
/// the render request.
///
/// A [`Bind`] admits some of these places and refuses the others
/// ([`Bind::admits`]). The errors of [`validate_values`] hold the place, so
/// that a report says where the value in question was set. Its `Display` form
/// is `document` or `render request`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindSite {
    /// The document, through
    /// [`Doc::with_options`](crate::model::doc::Doc::with_options).
    Document,
    /// The render request, through
    /// [`PlanRequest::render_options`](crate::core::plan::PlanRequest::render_options()).
    Render,
}

impl Bind {
    /// Returns `true` when this binding lets a value be set in the place
    /// `site`.
    ///
    /// [`Bind::Registry`] admits neither place; [`Bind::Either`] admits both.
    ///
    /// # Examples
    ///
    /// ```
    /// use flm_core::model::options::{Bind, BindSite};
    ///
    /// assert!(Bind::Either.admits(BindSite::Document));
    /// assert!(Bind::Either.admits(BindSite::Render));
    /// assert!(!Bind::Render.admits(BindSite::Document));
    /// assert!(!Bind::Registry.admits(BindSite::Render));
    /// ```
    pub fn admits(self, site: BindSite) -> bool {
        matches!(
            (self, site),
            (Bind::Either, _)
                | (Bind::Document, BindSite::Document)
                | (Bind::Render, BindSite::Render)
        )
    }

    /// The binding's name: `registry`, `document`, `render` or `either`. It is
    /// the `Display` form, the value written in a settings schema, and part of
    /// the registry digest.
    pub fn label(self) -> &'static str {
        match self {
            Bind::Registry => "registry",
            Bind::Document => "document",
            Bind::Render => "render",
            Bind::Either => "either",
        }
    }
}

impl fmt::Display for Bind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

impl fmt::Display for BindSite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            BindSite::Document => "document",
            BindSite::Render => "render request",
        })
    }
}

/// The requirements an option type meets: a value that can be shared between
/// threads (`Any + Send + Sync`), has a default (`Default`), can be digested
/// (`Hash`) and can be printed in messages (`Debug`).
///
/// This trait has no methods and is implemented for every type that meets the
/// requirements, so a feature writes a plain struct and derives `Default`,
/// `Hash` and `Debug` on it.
pub trait Options: Any + Send + Sync + Default + Hash + fmt::Debug {}

impl<T: Any + Send + Sync + Default + Hash + fmt::Debug> Options for T {}

/// A Rust type name without its module path, as messages show it.
///
/// `"flm_core::stdcore::features::refs::tables::Labels"` becomes `"Labels"`.
/// The generic arguments are kept as written, with their paths, since the
/// outer name is enough to identify the type in a message.
///
/// # Examples
///
/// ```
/// use flm_core::model::options::short_type_name;
///
/// assert_eq!(short_type_name("std::vec::Vec<my::crate::Thing>"), "Vec<my::crate::Thing>");
/// assert_eq!(short_type_name("Labels"), "Labels");
/// ```
pub fn short_type_name(name: &'static str) -> &'static str {
    // The module path of the outer type is dropped; a generic argument keeps
    // its own spelling (the outer name is enough to name the type in a message).
    let head_end = name.find('<').unwrap_or(name.len());
    match name[..head_end].rfind("::") {
        Some(pos) => &name[pos + 2..],
        None => name,
    }
}

/// The identifier of a Rust type, usable as a map key: the type's name and its
/// `TypeId`.
///
/// Keys compare and order by the type's name first, so a collection keyed by
/// them is visited in the same order in every build and every process, and
/// the messages and digests computed from it are the same each time. The
/// `TypeId` is what a downcast is checked against.
///
/// [`OptionKey::Type`] holds one for an option type that is a Rust type.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TypeKey {
    type_name: &'static str,
    type_id: TypeId,
}

impl TypeKey {
    /// The key of the type `T`.
    pub fn of<T: ?Sized + Any>() -> TypeKey {
        TypeKey { type_name: std::any::type_name::<T>(), type_id: TypeId::of::<T>() }
    }

    /// The type's full Rust name, path included.
    pub fn type_name(&self) -> &'static str {
        self.type_name
    }

    /// The type's name without its module path, as messages show it
    /// ([`short_type_name`]).
    pub fn short_name(&self) -> &'static str {
        short_type_name(self.type_name)
    }

    /// The type's `TypeId`, against which a downcast is checked.
    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    /// Returns `true` when this is the key of the type `T`.
    pub fn is<T: ?Sized + Any>(&self) -> bool {
        self.type_id == TypeId::of::<T>()
    }
}

impl fmt::Debug for TypeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "TypeKey({})", self.short_name())
    }
}

impl fmt::Display for TypeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.short_name())
    }
}

/// The identifier of an option type: a Rust type, or a name given as data.
///
/// An option type declared with a Rust type has no name of its own: a feature
/// declares it once, and every other piece of code refers to it by its Rust
/// type ([`OptionKey::of`]). An option type declared from data
/// ([`SettingsDecl::options_data`](crate::core::settings::SettingsDecl::options_data),
/// [`SettingsDecl::config_data`](crate::core::settings::SettingsDecl::config_data))
/// is referred to by its name ([`OptionKey::named`]), and its values are
/// [`DataValue`]s. A type key and a data key never compare equal.
///
/// Keys order the type keys first, by the type's name, then the data keys, by
/// their name, so a collection keyed by them is visited in the same order in
/// every build and every process. The `Debug` form is `OptionKey(NAME)` and the
/// `Display` form is [`short_name`](OptionKey::short_name).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum OptionKey {
    /// An option type that is a Rust type.
    Type(TypeKey),
    /// An option type declared from data, named by the feature's name (a
    /// configuration) or by a name the feature gives (an option type).
    Data(Name),
}

impl OptionKey {
    /// The key of the option type that is the Rust type `T`.
    pub fn of<T: ?Sized + Any>() -> OptionKey {
        OptionKey::Type(TypeKey::of::<T>())
    }

    /// The key of the option type declared from data under the name `name`.
    pub fn named(name: impl Into<Name>) -> OptionKey {
        OptionKey::Data(name.into())
    }

    /// The name messages show: the Rust type's name without its module path
    /// ([`short_type_name`]), or the name of an option type declared from data.
    pub fn short_name(&self) -> &str {
        match self {
            OptionKey::Type(key) => key.short_name(),
            OptionKey::Data(name) => name.as_str(),
        }
    }

    /// The Rust type's key, or `None` for an option type declared from data.
    pub fn type_key(&self) -> Option<&TypeKey> {
        match self {
            OptionKey::Type(key) => Some(key),
            OptionKey::Data(_) => None,
        }
    }

    /// Returns `true` when this is the key of the Rust type `T`; always `false`
    /// for an option type declared from data.
    pub fn is<T: ?Sized + Any>(&self) -> bool {
        self.type_key().is_some_and(TypeKey::is::<T>)
    }

    /// Writes what identifies the key into a digest: the Rust type's full name,
    /// or `data:` followed by the name. The digests of option values and the
    /// registry digest use it.
    pub(crate) fn write_digest(&self, hasher: &mut Fnv1a64) {
        match self {
            OptionKey::Type(key) => hasher.write_str(key.type_name()),
            OptionKey::Data(name) => {
                hasher.write_str("data:");
                hasher.write_str(name.as_str());
            }
        }
    }
}

impl fmt::Debug for OptionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OptionKey({})", self.short_name())
    }
}

impl fmt::Display for OptionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.short_name())
    }
}

/// The value of a configuration or of an option type declared from data: the
/// data as written. Its default is the empty map.
///
/// It implements [`Options`]: every option type declared from data stores its
/// values as a `DataValue`, under its own [`OptionKey::Data`]. Two values equal
/// as data hash equal, whatever the order their maps' entries were written in
/// ([`hash_value`]).
#[derive(Debug, Clone, PartialEq)]
pub struct DataValue(pub Value);

impl Default for DataValue {
    fn default() -> DataValue {
        DataValue(Value::Object(Map::new()))
    }
}

impl Hash for DataValue {
    fn hash<H: Hasher>(&self, state: &mut H) {
        hash_value(&self.0, state);
    }
}

/// Hashes `value` with every map's entries in the order of their keys, so that
/// two values equal as data hash equal in any order written.
///
/// A number is hashed through its text, so `1` and `1.0` hash apart, as they
/// compare apart; `-0.0` is hashed as `0.0`, to which it compares equal.
pub fn hash_value<H: Hasher>(value: &Value, state: &mut H) {
    match value {
        Value::Null => 0u8.hash(state),
        Value::Bool(flag) => (1u8, flag).hash(state),
        Value::Number(number) if number.is_f64() && number.as_f64() == Some(0.0) => (2u8, "0.0").hash(state),
        Value::Number(number) => (2u8, number.to_string()).hash(state),
        Value::String(text) => (3u8, text).hash(state),
        Value::Array(items) => {
            (4u8, items.len()).hash(state);
            for item in items {
                hash_value(item, state);
            }
        }
        Value::Object(map) => {
            5u8.hash(state);
            let mut entries: Vec<(&String, &Value)> = map.iter().collect();
            entries.sort_by(|a, b| a.0.cmp(b.0));
            entries.len().hash(state);
            for (key, value) in entries {
                key.hash(state);
                hash_value(value, state);
            }
        }
    }
}

type ErasedValue = Arc<dyn Any + Send + Sync>;

fn default_erased<T: Options>() -> ErasedValue {
    Arc::new(T::default())
}

fn digest_erased<T: Options>(value: &(dyn Any + Send + Sync)) -> u64 {
    digest_of(value.downcast_ref::<T>().expect("an option value is digested through its own type"))
}

/// A stage's declaration that it reads one option type: the option type,
/// with the two operations on it that resolving the read needs and cannot
/// perform on a value whose type is erased (creating the type's default, and
/// digesting a value).
///
/// [`Needs::option`](crate::core::stage::Needs::option) and
/// [`LoweringDecl::option`](crate::core::lowering::LoweringDecl::option)
/// create one per option type a stage or a lowering rule reads. Reads compare
/// and order by their [key](OptionRef::key), so declaring the same read twice
/// declares it once. [`OptionRef::named`] declares a read of an option type
/// declared from data.
#[derive(Clone)]
pub struct OptionRef {
    key: OptionKey,
    make_default: fn() -> ErasedValue,
    digest: fn(&(dyn Any + Send + Sync)) -> u64,
}

impl OptionRef {
    /// The declaration of a read of the option type `T`.
    pub fn of<T: Options>() -> OptionRef {
        OptionRef {
            key: OptionKey::of::<T>(),
            make_default: default_erased::<T>,
            digest: digest_erased::<T>,
        }
    }

    /// The declaration of a read of the option type declared from data under
    /// the name `name`; the read resolves to a [`DataValue`], the empty map
    /// when no value is set.
    pub fn named(name: impl Into<Name>) -> OptionRef {
        OptionRef {
            key: OptionKey::named(name),
            make_default: default_erased::<DataValue>,
            digest: digest_erased::<DataValue>,
        }
    }

    /// The option type read.
    pub fn key(&self) -> &OptionKey {
        &self.key
    }

    /// A new default value of the option type, with its type erased: what a
    /// read resolves to when no value is set on the render request or on the
    /// document.
    pub fn make_default(&self) -> ErasedValue {
        (self.make_default)()
    }

    /// The digest of `value`: [`digest_of`] applied through the option type's
    /// [`Hash`] implementation. The same value gives the same number in every
    /// process.
    ///
    /// # Panics
    ///
    /// Panics if `value` is not a value of this read's option type. The
    /// library always passes a value of the matching type, so no document can
    /// cause this panic.
    pub fn digest(&self, value: &(dyn Any + Send + Sync)) -> u64 {
        (self.digest)(value)
    }
}

impl PartialEq for OptionRef {
    fn eq(&self, other: &OptionRef) -> bool {
        self.key == other.key
    }
}

impl Eq for OptionRef {}

impl PartialOrd for OptionRef {
    fn partial_cmp(&self, other: &OptionRef) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OptionRef {
    fn cmp(&self, other: &OptionRef) -> std::cmp::Ordering {
        self.key.cmp(&other.key)
    }
}

/// Writes an option reference to data as the name of the option type it
/// refers to ([`OptionKey::short_name`]); see [serialized
/// forms](crate::core::registry#serialized-forms).
impl serde::Serialize for OptionRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.key.short_name())
    }
}

/// Describes an option reference in a JSON schema as a string, the form
/// `Serialize` writes.
impl schemars::JsonSchema for OptionRef {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "String".into()
    }

    fn inline_schema() -> bool {
        true
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        String::json_schema(generator)
    }
}

/// Writes an option key to data as its name ([`OptionKey::short_name`]), for
/// [`OptionDecl`]'s serialized form.
fn serialize_key_name<S: serde::Serializer>(key: &OptionKey, serializer: S) -> Result<S::Ok, S::Error> {
    serializer.serialize_str(key.short_name())
}

impl fmt::Debug for OptionRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "OptionRef({})", self.key.short_name())
    }
}

/// A feature's declaration of one option type: the type, its binding, the
/// feature that declares it, and its description.
///
/// [`FeatureBuilder::options`](crate::core::registry::FeatureBuilder::options)
/// creates one; the registry collects them from every feature
/// ([`FlmRegistry::options`](crate::core::registry::FlmRegistry::options)), and
/// the option values a caller sets are checked against them
/// ([`validate_values`]).
///
/// It is written to data with `Serialize`; see [serialized
/// forms](crate::core::registry#serialized-forms).
#[derive(Debug, Clone)]
#[derive(serde::Serialize, schemars::JsonSchema)]
pub struct OptionDecl {
    /// The option type declared.
    #[serde(serialize_with = "serialize_key_name")]
    #[schemars(with = "String")]
    pub key: OptionKey,
    /// The binding: where values for the option type may be set.
    pub bind: Bind,
    /// The feature that declares the option type and owns it.
    pub owner: FeatureName,
    /// A description of what the option type sets, for a person who reads the
    /// registry's declarations
    /// ([`FlmRegistry::options`](crate::core::registry::FlmRegistry::options));
    /// empty unless the feature gave one
    /// ([`FeatureBuilder::options_with_docs`](crate::core::registry::FeatureBuilder::options_with_docs)).
    pub docs: String,
}

impl OptionDecl {
    /// The declaration of the option type `T` by the feature `owner`, with the
    /// binding `bind` and an empty description.
    pub fn of<T: Options>(owner: impl Into<FeatureName>, bind: Bind) -> OptionDecl {
        OptionDecl { key: OptionKey::of::<T>(), bind, owner: owner.into(), docs: String::new() }
    }

    /// The declaration of the option type declared from data under the name
    /// `name`, by the feature `owner`, with the binding `bind` and an empty
    /// description.
    pub fn named(name: impl Into<Name>, owner: impl Into<FeatureName>, bind: Bind) -> OptionDecl {
        OptionDecl { key: OptionKey::named(name), bind, owner: owner.into(), docs: String::new() }
    }

    /// Builder: sets the description of the option type.
    pub fn docs(mut self, docs: impl Into<String>) -> OptionDecl {
        self.docs = crate::kept_docs(docs);
        self
    }
}

/// One option value set in one place, with its digest.
#[derive(Clone)]
struct OptionValue {
    value: ErasedValue,
    digest: u64,
}

/// The option values set in one place: at most one value per option type.
///
/// A caller fills one in to set option values on a document
/// ([`Doc::with_options`](crate::model::doc::Doc::with_options)) or on a
/// render request
/// ([`PlanRequest::render_options`](crate::core::plan::PlanRequest::render_options())).
/// Values are stored under the Rust type of the option struct, so setting a
/// value needs no name and cannot be misspelled. Whether an option type may be
/// set in that place is checked later, by [`validate_values`], when a run is
/// constructed or a plan is built.
///
/// # Examples
///
/// ```
/// use flm_core::model::options::OptionValues;
///
/// #[derive(Debug, Default, Hash, PartialEq, Eq)]
/// struct Width(u32);
///
/// let values = OptionValues::new().with(Width(72));
/// assert_eq!(values.get::<Width>(), Some(&Width(72)));
/// assert_eq!(values.len(), 1);
/// ```
#[derive(Clone, Default)]
pub struct OptionValues {
    entries: BTreeMap<OptionKey, OptionValue>,
}

impl OptionValues {
    /// A collection with no value set.
    pub fn new() -> OptionValues {
        OptionValues::default()
    }

    /// Sets `value` as the value of the option type `T`, replacing any value
    /// set for `T` before.
    pub fn set<T: Options>(&mut self, value: T) {
        let digest = digest_of(&value);
        self.entries.insert(OptionKey::of::<T>(), OptionValue { value: Arc::new(value), digest });
    }

    /// Builder: sets `value` as the value of the option type `T`, as
    /// [`OptionValues::set`] does.
    pub fn with<T: Options>(mut self, value: T) -> OptionValues {
        self.set(value);
        self
    }

    /// The value set for the option type `T` in this collection, or `None`
    /// when none is set.
    ///
    /// This reads what was set in this one place. A stage may read another
    /// value: it reads through [`ResolvedOptions`], which may give the value
    /// set in the other place or the type's default.
    pub fn get<T: Options>(&self) -> Option<&T> {
        self.entries.get(&OptionKey::of::<T>()).and_then(|entry| entry.value.downcast_ref::<T>())
    }

    /// Sets `value` as the value of the option type declared from data under
    /// the name `name`, replacing any value set for it before. The value is
    /// stored as a [`DataValue`].
    pub fn set_named(&mut self, name: impl Into<Name>, value: Value) {
        let value = DataValue(value);
        let digest = digest_of(&value);
        self.entries.insert(OptionKey::named(name), OptionValue { value: Arc::new(value), digest });
    }

    /// The value set for the option type declared from data under the name
    /// `name` in this collection, or `None` when none is set. As for
    /// [`get`](OptionValues::get), a stage reads through [`ResolvedOptions`]
    /// instead.
    pub fn get_named(&self, name: &str) -> Option<&Value> {
        self.entries
            .get(&OptionKey::named(name))
            .and_then(|entry| entry.value.downcast_ref::<DataValue>())
            .map(|value| &value.0)
    }

    /// The value set for the option type `key`, with its type erased, or
    /// `None` when none is set. For a caller that holds a key rather than the
    /// Rust type.
    pub fn get_erased(&self, key: &OptionKey) -> Option<&ErasedValue> {
        self.entries.get(key).map(|entry| &entry.value)
    }

    /// Copies every value of `other` into this collection, replacing the value
    /// of each option type that `other` sets and leaving the others unchanged.
    ///
    /// Each value is replaced whole: it is never combined field by field with
    /// the value it replaces. Use this to let a caller's own values replace,
    /// option type by option type, a set of values supplied together.
    pub fn merge_from(&mut self, other: &OptionValues) {
        for (key, entry) in &other.entries {
            self.entries.insert(key.clone(), entry.clone());
        }
    }

    /// The option types that have a value set, in key order.
    pub fn keys(&self) -> impl Iterator<Item = OptionKey> + '_ {
        self.entries.keys().cloned()
    }

    /// The number of option types that have a value set.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` when no value is set.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The digest of the whole collection: the option types and their values,
    /// in key order.
    ///
    /// Two collections that set the same values give the same number, in every
    /// process and every build. The digest of a plan request's render option
    /// values is part of the plan's identity
    /// ([`PlanKey`](crate::core::plan::PlanKey)), so plans built with different
    /// render option values are different plans.
    pub fn digest(&self) -> u64 {
        let mut hasher = Fnv1a64::new();
        for (key, entry) in &self.entries {
            key.write_digest(&mut hasher);
            hasher.write_u64(entry.digest);
        }
        hasher.value()
    }
}

impl fmt::Debug for OptionValues {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.entries.keys()).finish()
    }
}

/// An error about option types and option values.
///
/// Each variant holds what a report needs, including the option types that
/// were available, so that a message can list them without looking anything
/// up again. A plan build returns these inside
/// [`PlanError`](crate::core::plan::PlanError), a run inside
/// [`RunError::DocumentOption`](crate::core::run::RunError::DocumentOption),
/// and a stage's read of an option directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OptionError {
    /// A stage read an option type that it did not declare. The read returns
    /// this error rather than the type's default, so that a missing
    /// declaration is reported instead of changing what the stage computes.
    UndeclaredRead {
        /// The stage, rewrite stage or lowering rule that read the option type.
        stage: StageName,
        /// The option type it read, by short name ([`OptionKey::short_name`]).
        option: String,
        /// The option types it declared, by short name.
        declared: Vec<String>,
    },
    /// A value was set for an option type that no feature of the registry
    /// declares.
    UnknownOption {
        /// Where the value was set.
        site: BindSite,
        /// The option type, by short name.
        option: String,
        /// Every option type the registry declares, by short name.
        roster: Vec<String>,
        /// The workflow whose render request sets the value, or `None` for a
        /// value set on the document or on a render request built by hand.
        workflow: Option<WorkflowName>,
    },
    /// A value was set in a place that the option type's binding does not
    /// admit, for example on the document for an option type bound to the
    /// render request.
    BindingRefused {
        /// Where the value was set.
        site: BindSite,
        /// The option type, by short name.
        option: String,
        /// The binding that the feature declared.
        declared: Bind,
        /// The feature that declares the option type.
        owner: FeatureName,
        /// The workflow whose render request sets the value, or `None` for a
        /// value set on the document or on a render request built by hand.
        workflow: Option<WorkflowName>,
    },
    /// A stage declared a read of an option type whose binding is
    /// [`Bind::Registry`]. Such a read could only return the type's default,
    /// since no value can be set for it; the feature that declares the option
    /// type holds the value in its configuration.
    RegistryBoundRead {
        /// The stage that declared the read.
        stage: StageName,
        /// The option type, by short name.
        option: String,
        /// The feature that declares the option type.
        owner: FeatureName,
    },
}

impl fmt::Display for OptionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OptionError::UndeclaredRead { stage, option, declared } => write!(
                f,
                "stage `{stage}` must declare each option type it reads, but it reads \
                 `{option}` without declaring it; it declares: [{}]",
                declared.join(", ")
            ),
            OptionError::UnknownOption { site, option, roster, workflow } => write!(
                f,
                "{} for the option type `{option}`, but no feature declares that option type; \
                 declared option types: [{}]",
                value_set(*site, workflow),
                roster.join(", ")
            ),
            OptionError::BindingRefused { site, option, declared, owner, workflow } => write!(
                f,
                "{} for the option type `{option}` of feature `{owner}`, but feature `{owner}` \
                 declares that values of this option type may be set only {}",
                value_set(*site, workflow),
                match declared {
                    Bind::Registry => {
                        format!("in the configuration of feature `{owner}`, when the registry is built")
                    }
                    Bind::Document => "on the document".to_string(),
                    Bind::Render => "on the render request".to_string(),
                    Bind::Either => "on the document or on the render request".to_string(),
                }
            ),
            OptionError::RegistryBoundRead { stage, option, owner } => write!(
                f,
                "stage `{stage}` declares a read of the option type `{option}` of feature \
                 `{owner}`, but feature `{owner}` fixes the value of that option type when the \
                 registry is built, so no stage can read it"
            ),
        }
    }
}

impl std::error::Error for OptionError {}

/// The opening of a message about a value set for an option type: "a value is
/// set on the render request of workflow `w`" when the value is set on the
/// render request of the workflow `w`, and "a value is set on the document"
/// (or "on the render request") otherwise. The render request of a workflow
/// holds the workflow's values and the caller's, so the sentence names the
/// request, not the workflow, as the one that sets the value.
fn value_set(site: BindSite, workflow: &Option<WorkflowName>) -> String {
    match workflow {
        Some(workflow) => format!("a value is set on the {site} of workflow `{workflow}`"),
        None => format!("a value is set on the {site}"),
    }
}

/// Checks the option values `values`, set in the place `site`, against the
/// option declarations: each value's option type must be declared, with a
/// binding that admits `site`.
///
/// `lookup` returns the declaration of an option type, or `None` when no
/// feature declares it; `roster` lists every declared option type, which the
/// error lists when a value is set for an undeclared type. `workflow` is the
/// workflow whose render request sets the values, which the errors name; it is
/// `None` for the document's values and for a render request built by hand
/// ([`PlanRequest::workflow`](crate::core::plan::PlanRequest::workflow)). Returns every
/// error found, in key order: an [`OptionError::UnknownOption`] or an
/// [`OptionError::BindingRefused`] per value in question, and an empty list
/// when every value is admitted.
///
/// The library calls this when a run is constructed
/// ([`Run::new`](crate::core::run::Run::new)), for the values set on the
/// document, and when a plan is built, for the values set on the render
/// request; both report the first error only. The registry build calls it for
/// the render option values of every workflow and reports every error.
pub fn validate_values<'d>(
    values: &OptionValues,
    site: BindSite,
    lookup: impl Fn(&OptionKey) -> Option<&'d OptionDecl>,
    roster: &[OptionKey],
    workflow: Option<&WorkflowName>,
) -> Vec<OptionError> {
    let mut errors = Vec::new();
    for key in values.keys() {
        match lookup(&key) {
            None => errors.push(OptionError::UnknownOption {
                site,
                option: key.short_name().to_string(),
                roster: roster.iter().map(|key| key.short_name().to_string()).collect(),
                workflow: workflow.cloned(),
            }),
            Some(decl) if !decl.bind.admits(site) => errors.push(OptionError::BindingRefused {
                site,
                option: key.short_name().to_string(),
                declared: decl.bind,
                owner: decl.owner.clone(),
                workflow: workflow.cloned(),
            }),
            Some(_) => {}
        }
    }
    errors
}

/// One stage's declared option reads, resolved to values.
///
/// A run builds one per scheduled stage when it is constructed, so that every
/// read within the stage returns the same value, and the stage reads the
/// values through its context
/// ([`StageCx::option`](crate::core::run::StageCx::option)). A read of an
/// option type that the stage did not declare returns an error
/// ([`OptionError::UndeclaredRead`]), not the type's default.
#[derive(Clone)]
pub struct ResolvedOptions {
    stage: StageName,
    entries: BTreeMap<OptionKey, (ErasedValue, u64)>,
}

impl ResolvedOptions {
    /// Resolves each read in `reads`, the declared reads of the stage
    /// `stage`: to the value set in `render` if there is one, otherwise the
    /// value set in `document` if there is one, otherwise the option type's
    /// default.
    ///
    /// This function checks nothing. Check both collections with
    /// [`validate_values`] first, so that every value found here is admitted
    /// by its binding.
    pub fn resolve(
        stage: StageName,
        reads: &[OptionRef],
        render: &OptionValues,
        document: &OptionValues,
    ) -> ResolvedOptions {
        let mut entries = BTreeMap::new();
        for read in reads {
            let key = read.key().clone();
            let (value, digest) =
                match render.entries.get(&key).or_else(|| document.entries.get(&key)) {
                    Some(entry) => (Arc::clone(&entry.value), entry.digest),
                    None => {
                        let value = read.make_default();
                        let digest = read.digest(value.as_ref());
                        (value, digest)
                    }
                };
            entries.insert(key, (value, digest));
        }
        ResolvedOptions { stage, entries }
    }

    /// The resolved value of the option type `T`.
    ///
    /// # Errors
    ///
    /// Returns [`OptionError::UndeclaredRead`], with the stage and the option
    /// types it declared, when `T` is not among the stage's declared reads.
    pub fn get<T: Options>(&self) -> Result<&T, OptionError> {
        let key = OptionKey::of::<T>();
        match self.entries.get(&key) {
            Some((value, _)) => Ok(value
                .downcast_ref::<T>()
                .expect("a resolved option is stored under its own type")),
            None => Err(self.undeclared(key.short_name())),
        }
    }

    /// The resolved value of the option type declared from data under the name
    /// `name`: the data as written, or the empty map when no value is set.
    ///
    /// # Errors
    ///
    /// Returns [`OptionError::UndeclaredRead`], with the stage, the name and the
    /// option types it declared, when the stage did not declare a read of
    /// `name` ([`OptionRef::named`]).
    pub fn get_named(&self, name: &str) -> Result<&Value, OptionError> {
        match self.entries.get(&OptionKey::named(name)) {
            Some((value, _)) => Ok(&value
                .downcast_ref::<DataValue>()
                .expect("an option declared from data is stored as a `DataValue`")
                .0),
            None => Err(self.undeclared(name)),
        }
    }

    /// The error of a read of `option`, which the stage did not declare.
    fn undeclared(&self, option: &str) -> OptionError {
        OptionError::UndeclaredRead {
            stage: self.stage.clone(),
            option: option.to_string(),
            declared: self.entries.keys().map(|key| key.short_name().to_string()).collect(),
        }
    }

    /// The option types the stage declared, in key order.
    pub fn keys(&self) -> impl Iterator<Item = OptionKey> + '_ {
        self.entries.keys().cloned()
    }

    /// The resolved value of the option type `key`, with its type erased, or
    /// `None` when the stage did not declare that read.
    ///
    /// For a caller that holds a key rather than the Rust type: the lowering
    /// driver returns a lowering rule's option reads through it
    /// ([`Lowerer::option_erased`](crate::core::lowering::Lowerer::option_erased)),
    /// after checking the read against the rule's own declaration.
    pub fn get_erased(&self, key: &OptionKey) -> Option<&(dyn Any + Send + Sync)> {
        self.entries.get(key).map(|(value, _)| &**value)
    }

    /// The digest of the resolved values: the option types and their values,
    /// in key order.
    ///
    /// It is the same in every process for the same values. The stamp of an
    /// external table ([`Stamp`](crate::core::run::Stamp)) includes the digests
    /// of the values that the rewrite stages read, so that data computed with
    /// other option values is not mistaken for data computed with these.
    pub fn digest(&self) -> u64 {
        let mut hasher = Fnv1a64::new();
        for (key, (_, digest)) in &self.entries {
            key.write_digest(&mut hasher);
            hasher.write_u64(*digest);
        }
        hasher.value()
    }
}

impl fmt::Debug for ResolvedOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ResolvedOptions")
            .field("stage", &self.stage)
            .field("options", &self.entries.keys().collect::<Vec<_>>())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default, Hash, PartialEq, Eq)]
    struct Width(u32);

    #[derive(Debug, Default, Hash)]
    struct Depth(u32);

    #[test]
    fn a_binding_admits_its_own_site_and_either_admits_both() {
        assert!(Bind::Document.admits(BindSite::Document));
        assert!(!Bind::Document.admits(BindSite::Render));
        assert!(Bind::Render.admits(BindSite::Render));
        assert!(!Bind::Render.admits(BindSite::Document));
        assert!(Bind::Either.admits(BindSite::Document));
        assert!(Bind::Either.admits(BindSite::Render));
        assert!(!Bind::Registry.admits(BindSite::Document));
    }

    #[test]
    fn resolution_takes_render_over_document_over_default() {
        let reads = [OptionRef::of::<Width>(), OptionRef::of::<Depth>()];
        let render = OptionValues::new().with(Width(120));
        let document = OptionValues::new().with(Width(100)).with(Depth(5));
        let resolved = ResolvedOptions::resolve("s".into(), &reads, &render, &document);
        assert_eq!(resolved.get::<Width>().unwrap(), &Width(120));
        assert_eq!(resolved.get::<Depth>().unwrap().0, 5);
        let none = ResolvedOptions::resolve(
            "s".into(),
            &reads,
            &OptionValues::new(),
            &OptionValues::new(),
        );
        assert_eq!(none.get::<Width>().unwrap(), &Width(0));
    }

    #[test]
    fn an_undeclared_read_is_a_typed_error() {
        let resolved =
            ResolvedOptions::resolve("s".into(), &[], &OptionValues::new(), &OptionValues::new());
        let error = resolved.get::<Width>().unwrap_err();
        assert!(matches!(error, OptionError::UndeclaredRead { ref option, .. } if option == "Width"));
    }

    #[test]
    fn values_at_a_refused_site_are_reported() {
        let decl = OptionDecl::of::<Width>("f", Bind::Render);
        let values = OptionValues::new().with(Width(1)).with(Depth(2));
        let errors = validate_values(
            &values,
            BindSite::Document,
            |key| if key.is::<Width>() { Some(&decl) } else { None },
            &[OptionKey::of::<Width>()],
            None,
        );
        assert_eq!(errors.len(), 2);
        assert!(matches!(errors[0], OptionError::UnknownOption { ref option, .. } if option == "Depth"));
        assert!(matches!(errors[1], OptionError::BindingRefused { ref option, .. } if option == "Width"));
    }

    #[test]
    fn the_values_digest_depends_on_the_values() {
        let a = OptionValues::new().with(Width(1));
        let b = OptionValues::new().with(Width(2));
        assert_ne!(a.digest(), b.digest());
        assert_eq!(a.digest(), OptionValues::new().with(Width(1)).digest());
    }

    fn hash_of(value: &DataValue) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }

    #[test]
    fn equal_data_values_hash_equal_whatever_the_order_of_their_entries() {
        let one = DataValue(serde_json::json!({ "a": 1, "b": { "x": [1, 2], "y": "t" } }));
        let two = DataValue(serde_json::json!({ "b": { "y": "t", "x": [1, 2] }, "a": 1 }));
        assert_eq!(one, two);
        assert_eq!(hash_of(&one), hash_of(&two));
        assert_eq!(digest_of(&one), digest_of(&two));
        let other = DataValue(serde_json::json!({ "a": 2, "b": { "x": [1, 2], "y": "t" } }));
        assert_ne!(hash_of(&one), hash_of(&other));
        assert_eq!(DataValue::default(), DataValue(serde_json::json!({})));
        let (negative, zero) = (DataValue(serde_json::json!(-0.0)), DataValue(serde_json::json!(0.0)));
        assert_eq!(negative, zero);
        assert_eq!(hash_of(&negative), hash_of(&zero));
    }

    #[test]
    fn a_type_key_and_a_data_key_never_compare_equal() {
        // A data option named as a Rust type's full name is still another key.
        let full = std::any::type_name::<Width>();
        assert_ne!(OptionKey::of::<Width>(), OptionKey::named(full));
        assert_ne!(OptionKey::of::<Width>(), OptionKey::named("Width"));
        assert_eq!(OptionKey::named("w.display"), OptionKey::named("w.display"));
        assert!(OptionKey::of::<Width>() < OptionKey::named("a"));
        assert!(OptionKey::of::<Width>().is::<Width>());
        assert!(!OptionKey::named("Width").is::<Width>());
        assert_eq!(OptionKey::named("w.display").short_name(), "w.display");
        // Their values, set side by side, stay apart, and so do their digests.
        let values = OptionValues::new().with(Width(1));
        let mut named = values.clone();
        named.set_named("Width", serde_json::json!({ "w": 1 }));
        assert_eq!(named.len(), 2);
        assert_eq!(named.get::<Width>(), Some(&Width(1)));
        assert_eq!(named.get_named("Width"), Some(&serde_json::json!({ "w": 1 })));
        assert_ne!(values.digest(), named.digest());
    }

    #[test]
    fn a_data_read_resolves_to_the_value_set_or_the_empty_map() {
        let reads = [OptionRef::named("w.display"), OptionRef::named("w.other")];
        let mut document = OptionValues::new();
        document.set_named("w.display", serde_json::json!({ "size": 3 }));
        let resolved = ResolvedOptions::resolve("s".into(), &reads, &OptionValues::new(), &document);
        assert_eq!(resolved.get_named("w.display").unwrap(), &serde_json::json!({ "size": 3 }));
        assert_eq!(resolved.get_named("w.other").unwrap(), &serde_json::json!({}));
        let error = resolved.get_named("w.none").unwrap_err();
        assert_eq!(
            error.to_string(),
            "stage `s` must declare each option type it reads, but it reads `w.none` without \
             declaring it; it declares: [w.display, w.other]"
        );
    }
}
