//! The guest client's hand-written items, one file per module of the
//! extension API that needs any (`core_registry.rs` holds those of
//! `flm:core/registry`), the twin of the prelude's fragments: the guest's
//! own types, and the ten handler traits.
//!
//! A type is the guest's own when its crossing form can hold a handle or a
//! handler object, or when the guest cannot build it back as the api's type
//! from its crossing form (a `flm-core` identifier only a tree makes, a
//! constructor the guest cannot call, a field the form leaves out, an
//! argument's parser, which the guest declares by its standard kind or reads
//! from its code), or when it holds
//! one of those (`ArgStdKind`, `SyntaxDef`, `Definition` hold arguments): the
//! list `GUEST_OWN` of the generator's mapping table. Each has the api type's
//! name, its public constructors and methods with the guest's types, and a
//! handle where the api holds an object; its conversion is written here, the
//! one statement of the guest type's form, and tested against the host's
//! converter of the same shape (`tests/guest_forms.rs`). Its conversions of
//! the typed route (the cargo feature `native`) are written beside it; a
//! typed conversion that no test of an extension reaches is checked against
//! the dispatcher's conversion of the same JSON form (`typed_forms.rs`). A
//! type that no item of the api names is added where a constructor of the
//! guest's own type takes it, or where an extension's settings hold it as
//! the mirror of a `flm-core` data type (`ArgListData`; the list
//! `GUEST_ADDED`), and a free function
//! whose answer a constructor of the guest's own type makes with no request
//! is not offered (the list `GUEST_OMITTED`): `ArgDef::from_code` reads an
//! argument code where `arg_def_from_code` would ask the host. Of the types that hold
//! handler objects, `ElementSpec` (and the declarations that hold one:
//! `ElementDef`, `FallbackDef`, `ScopedPackageDef`) keeps each object beside the crossing
//! form its lending answered; `BackendDef`, `Aspect`, `BackendStage` and
//! `Contributed` hold the objects alone, recorded and lent by the method
//! that takes them (`ElementSpec::with`, `FeatureBuilder::backend`,
//! `FeatureBuilder::contribute_to`, …). The functions that record and lend
//! the objects are the runtime's.
//!
//! The handler traits are the api's with the guest's types: the same
//! methods, receivers, defaults and bounds (`Send + Sync`). The generated
//! surface (`generated.rs`) re-exports every item from its module.

use serde_json::{Map, Value};

use crate::adapter::values::JsonForm;
use crate::runtime::ExtError;

pub mod core_data_table;
pub mod core_lowering;
pub mod core_registry;
pub mod core_run;
pub mod core_stage;
pub mod host;
pub(crate) mod joined;
pub mod lang_copy;
pub mod lang_mint;
pub mod lang_rewrites_userdefs;
pub mod model_aspects;
pub mod model_element;
pub mod stdlib_features_citations;
pub mod stdlib_features_compose;
pub mod stdlib_features_math;
pub mod stdlib_features_numbering;
pub mod techy_error;
pub mod techy_node;
pub mod techy_transform;
#[cfg(all(test, feature = "native"))]
mod typed_forms;

/// The error of a value that is not of the shape `shape`.
pub(crate) fn not_shape(what: &str, shape: &str, value: &Value) -> ExtError {
    ExtError::Value { detail: format!("{what} must be {shape}, not `{value}`") }
}

/// The map `value` is.
///
/// # Errors
///
/// `value` is not a map ([`ExtError::Value`]).
pub(crate) fn object<'v>(what: &str, value: &'v Value) -> Result<&'v Map<String, Value>, ExtError> {
    value.as_object().ok_or_else(|| not_shape(what, "a map", value))
}

/// The field `key` of `map` (an absent field being `null`), read through its
/// [`JsonForm`]; an error of its form names the field and the map.
///
/// # Errors
///
/// The field is not of its form ([`ExtError::Value`], prefixed with the
/// field's name), or what its form answers.
pub(crate) fn get<T: JsonForm>(what: &str, map: &Map<String, Value>, key: &str) -> Result<T, ExtError> {
    T::from_value(map.get(key).unwrap_or(&Value::Null)).map_err(|error| match error {
        ExtError::Value { detail } => ExtError::Value { detail: format!("in the field `{key}` of {what}, {detail}") },
        other => other,
    })
}

/// The one key of the map `value` and the value under it, when `value` is a
/// map of exactly one key and that key is one of `keys`.
pub(crate) fn single<'v>(value: &'v Value, keys: &[&str]) -> Option<(&'v str, &'v Value)> {
    let map = value.as_object()?;
    let (key, inner) = map.iter().next()?;
    (map.len() == 1 && keys.contains(&key.as_str())).then_some((key.as_str(), inner))
}

/// `"WORD"` or `{"emit": staged}`, the shape of an expansion, a lowering
/// and a rewrite handler's answer, `staged` a sequence of staged nodes or a
/// joined list: `None` for the word.
///
/// # Errors
///
/// `value` is neither ([`ExtError::Value`]), or a part is not the handle of
/// a staged node or of a sequence of staged nodes.
pub(crate) fn emit_from_value(
    what: &str,
    word: &str,
    value: &Value,
) -> Result<Option<crate::guest::lang::rewrites::userdefs::StagedNodes>, ExtError> {
    if value.as_str() == Some(word) {
        return Ok(None);
    }
    match single(value, &["emit"]) {
        Some((_, staged)) => JsonForm::from_value(staged).map(Some),
        _ => Err(not_shape(what, &format!("`\"{word}\"` or `{{\"emit\": [...]}}`"), value)),
    }
}

/// `{"emit": staged}`.
pub(crate) fn emit_to_value(staged: &crate::guest::lang::rewrites::userdefs::StagedNodes) -> Value {
    serde_json::json!({ "emit": staged.to_value() })
}
