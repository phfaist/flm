//! Reading and writing collections of named things as settings data: a map from
//! each entry's name to the rest of the entry, in the order written.
//!
//! A feature keeps some of its settings as a list of entries that each carry
//! their own name, such as the xnote kinds, the float types or the heading
//! kinds. In data, such a list is written as a map from each entry's name to the
//! rest of the entry, so that two sources of settings can be combined entry by
//! entry ([`SettingsDecl::merge_entries`](super::SettingsDecl::merge_entries));
//! the list keeps the order in which the map was written.
//!
//! - Reading: a field of this kind is read with [`deserialize`], through the
//!   attribute
//!   `#[serde(deserialize_with = "flm_core::core::settings::named::deserialize")]`,
//!   and its entry type implements [`NamedEntry`]. The functions
//!   [`read_unnamed`], [`read_with_name`], [`read_over`], [`read_fields_over`]
//!   and [`required`] help an implementation of [`NamedEntry::read_entry`], one
//!   for each way an entry relates to its name and its defaults.
//! - Writing: [`to_data`] writes such a list back as that map, for a program
//!   that states settings in Rust and passes them on as data (for example as
//!   the default entries of a key merged entry by entry).
//! - Schema: [`map_schema`] and [`require_only`] adjust the JSON Schema that
//!   `schemars` derives for such fields and entry types.

use std::fmt;
use std::marker::PhantomData;

use serde::de::{DeserializeOwned, DeserializeSeed, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;

/// An entry type of a collection of named things, read from data under its name
/// by [`deserialize`].
pub trait NamedEntry: Sized {
    /// Reads the entry named `name` from `data`, the rest of the entry as
    /// written under that name.
    ///
    /// # Errors
    ///
    /// Returns a message saying why `data` cannot be read as an entry;
    /// [`deserialize`] reports it at the entry's key.
    fn read_entry(name: &str, data: Value) -> Result<Self, String>;
}

/// Reads an entry of type `T` that does not hold its name, from the data written
/// under the name `name`, with `T`'s `Deserialize` implementation. Use it for an
/// entry type that is kept beside its name rather than holding it. Nothing
/// written (`null`) reads as an empty map.
///
/// # Errors
///
/// Returns a message saying why the data cannot be read as a `T`, or saying
/// that the entry is not a map.
pub fn read_unnamed<T: DeserializeOwned>(name: &str, data: Value) -> Result<T, String> {
    read(Value::Object(entry_object(name, data)?))
}

/// Reads an entry of type `T` that holds its name in the field `field`: the data
/// written under the name `name`, with the name added under `field`, read by
/// `T`'s `Deserialize` implementation. Use it for an entry type whose other
/// fields take defaults that do not depend on its name.
///
/// # Errors
///
/// Returns a message saying why the data cannot be read as a `T`. The data
/// must not contain `field` itself, because the key of the map already gives
/// the name; data that contains `field` is refused.
pub fn read_with_name<T: DeserializeOwned>(field: &str, name: &str, data: Value) -> Result<T, String> {
    let mut object = entry_object(name, data)?;
    if object.contains_key(field) {
        return Err(format!(
            "the entry `{name}` must not contain the field `{field}`: its key in the map already \
             gives its name"
        ));
    }
    object.insert(field.into(), Value::from(name));
    read(Value::Object(object))
}

/// Reads the field `field` of the data of the entry named `name`, a field that
/// the data must contain.
///
/// Use it for a value that the entry's Rust constructor takes as a parameter:
/// the data must give it too, and the caller passes it to the constructor to
/// make the base value that [`read_over`] then reads the entry over.
///
/// # Errors
///
/// Returns a message saying that the entry does not contain the field, that the
/// entry is not a map, or why the field's value cannot be read as a `T`.
pub fn required<T: DeserializeOwned>(name: &str, data: &Value, field: &str) -> Result<T, String> {
    let value = match data {
        Value::Object(object) => object.get(field),
        Value::Null => None,
        other => return Err(format!("the entry `{name}` must be a map, not `{other}`")),
    };
    match value {
        Some(value) => read(value.clone()).map_err(|error| format!("`{field}`: {error}")),
        None => Err(format!("the entry `{name}` must contain the field `{field}`")),
    }
}

/// Reads an entry of type `T` over `base`, the entry that the entry type's Rust
/// constructor creates for the name `name`: every field written in the data replaces the base's
/// value, and every field not written keeps it. `base` holds the name in the
/// field `field`.
///
/// Use it for an entry type whose defaults depend on its name, for example a
/// float type's caption words. The defaults then stay those of the Rust
/// constructor, stated once.
///
/// # Errors
///
/// Returns a message saying why the data cannot be read as a `T`. A field that
/// `T` does not have is refused if `T`'s `Deserialize` implementation refuses
/// unknown fields, and a `field` written in the data is refused, because the
/// key of the map already gives the name.
pub fn read_over<T: Serialize + DeserializeOwned>(
    base: &T,
    field: &str,
    name: &str,
    data: Value,
) -> Result<T, String> {
    // The defaults stay the Rust constructor's, stated once.
    // See decision rationale at [§flm-dd-dr:feat-settings-named]
    let written = entry_object(name, data)?;
    if written.contains_key(field) {
        return Err(format!(
            "the entry `{name}` must not contain the field `{field}`: its key in the map already \
             gives its name"
        ));
    }
    read_fields_over(base, written)
}

/// Reads a `T` over `base`: every field that `written` holds replaces the
/// base's value, and every field it does not hold keeps it.
///
/// Use it for a value whose defaults are those of another value of its type
/// rather than the type's own, for example the counter style that the citations
/// feature uses by default. [`read_over`] uses it to read an entry.
///
/// # Errors
///
/// Returns a message saying why the fields cannot be read as a `T`. A field
/// that `T` does not have is refused if `T`'s `Deserialize` implementation
/// refuses unknown fields. When `base` does not serialize as a map of fields,
/// which is a mistake in the code that gives the base and not in the data, the
/// message says so.
pub fn read_fields_over<T: Serialize + DeserializeOwned>(
    base: &T,
    written: serde_json::Map<String, Value>,
) -> Result<T, String> {
    let mut object = match serde_json::to_value(base) {
        Ok(Value::Object(object)) => object,
        // A fault of the feature's own code, not of the data.
        _ => {
            return Err(format!(
                "the base value of type `{}`, whose fields the written fields replace, must \
                 serialize as a map of fields",
                std::any::type_name::<T>()
            ))
        }
    };
    for (key, value) in written {
        object.insert(key, value);
    }
    read(Value::Object(object))
}

/// The entry's data as a map; nothing written is the empty map.
fn entry_object(name: &str, data: Value) -> Result<serde_json::Map<String, Value>, String> {
    match data {
        Value::Object(object) => Ok(object),
        Value::Null => Ok(serde_json::Map::new()),
        other => Err(format!("the entry `{name}` must be a map, not `{other}`")),
    }
}

/// Reads `T`; the error message gives the path of the problem inside the entry.
fn read<T: DeserializeOwned>(data: Value) -> Result<T, String> {
    serde_path_to_error::deserialize(data).map_err(|error| {
        let path = error.path().to_string();
        let inner = error.into_inner();
        if path == "." { inner.to_string() } else { format!("`{path}`: {inner}") }
    })
}

/// Reads a list of named entries from a map from each entry's name to the rest
/// of the entry, keeping the order in which the map was written. Each entry is
/// read with [`NamedEntry::read_entry`]. See the [module documentation](self).
///
/// # Errors
///
/// Returns the deserializer's error when the value is not a map, or when an
/// entry cannot be read.
pub fn deserialize<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: NamedEntry,
{
    struct Entries<T>(PhantomData<T>);

    impl<'de, T: NamedEntry> Visitor<'de> for Entries<T> {
        type Value = Vec<T>;

        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a map from a name to an entry")
        }

        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Vec<T>, A::Error> {
            let mut entries = Vec::new();
            while let Some(name) = map.next_key::<String>()? {
                entries.push(map.next_value_seed(Entry { name: &name, entry: PhantomData })?);
            }
            Ok(entries)
        }
    }

    deserializer.deserialize_map(Entries(PhantomData))
}

/// Writes `entries` as data, the inverse of [`deserialize`]: a map from each
/// entry's name to the rest of the entry, in the order of the list. Each entry
/// holds its name in the field `field`, which is removed from the entry and
/// becomes its key.
///
/// # Errors
///
/// Returns a message when an entry does not serialize as a map of fields with a
/// text under `field`, which is a mistake in the entry type's code, not in any
/// data.
///
/// # Examples
///
/// ```
/// use flm_core::core::settings::named::to_data;
/// use flm_core::stdcore::features::floats::FloatType;
///
/// let data = to_data(&[FloatType::figure().caption_name("Fig.")], "name").unwrap();
/// assert_eq!(data["figure"]["caption_name"], "Fig.");
/// assert!(data["figure"].get("name").is_none());
/// ```
pub fn to_data<T: Serialize>(entries: &[T], field: &str) -> Result<Value, String> {
    let mut map = serde_json::Map::new();
    for entry in entries {
        let written = serde_json::to_value(entry).map_err(|error| error.to_string())?;
        let Value::Object(mut object) = written else {
            return Err(format!(
                "an entry of type `{}` must serialize as a map of fields",
                std::any::type_name::<T>()
            ));
        };
        let Some(Value::String(name)) = object.remove(field) else {
            return Err(format!(
                "an entry of type `{}` must hold its name as text in the field `{field}`",
                std::any::type_name::<T>()
            ));
        };
        map.insert(name, Value::Object(object));
    }
    Ok(Value::Object(map))
}

/// Reads one entry as the value of its name's key in the map, so that a problem
/// in the entry is reported at that key.
struct Entry<'n, T> {
    name: &'n str,
    entry: PhantomData<T>,
}

impl<'de, T: NamedEntry> DeserializeSeed<'de> for Entry<'_, T> {
    type Value = T;

    fn deserialize<D: Deserializer<'de>>(self, deserializer: D) -> Result<T, D::Error> {
        let data = Value::deserialize(deserializer)?;
        T::read_entry(self.name, data).map_err(serde::de::Error::custom)
    }
}

/// Removes the default from the JSON Schema of a field read by
/// [`deserialize`].
///
/// `schemars` derives the schema of such a field as a map from a name to an
/// entry, but the default it would record is the field's Rust default, a list
/// rather than a map, so the default is left out. Use it through the attribute
/// `#[schemars(transform = flm_core::core::settings::named::map_schema)]`.
pub fn map_schema(schema: &mut schemars::Schema) {
    schema.remove("default");
}

/// Sets the required fields of a type's JSON Schema to exactly `fields`
/// (removing the list when `fields` is empty).
///
/// Use it for a type read over a base value ([`read_over`],
/// [`read_fields_over`]): every field already has a value before the data is
/// read, so the data needs to give only what the Rust constructor of the base
/// takes. Call it from a function that the type lists in its attribute
/// `#[schemars(transform = …)]`.
pub fn require_only(schema: &mut schemars::Schema, fields: &[&str]) {
    if fields.is_empty() {
        schema.remove("required");
    } else {
        let fields = fields.iter().map(|field| Value::from(*field)).collect();
        schema.insert("required".into(), Value::Array(fields));
    }
}
