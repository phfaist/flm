//! The guest client's hand-written items (refresh section 6.4), one file per
//! module of the extension API that needs any (`core_registry.rs` holds those
//! of `flm:core/registry`), the twin of the prelude's fragments: the guest's
//! own types, and the ten handler traits.
//!
//! A type is the guest's own when its crossing form can hold a handle or a
//! handler object, or when the guest cannot build it back as the api's type
//! from its crossing form (a `flm-core` identifier only a tree makes, a
//! constructor the guest cannot call, a field the form leaves out): the list
//! `GUEST_OWN` of the generator's mapping table. Each has the api type's
//! name, its public constructors and methods with the guest's types, and a
//! handle where the api holds an object; its conversion is written here, the
//! one statement of the guest type's form, and tested against the host's
//! converter of the same shape (`tests/guest_forms.rs`). A type that no item
//! of the api names is added where a constructor of the guest's own type
//! takes it (the list `GUEST_ADDED`): `ArgSyntax`, the code or the standard
//! kind that `ArgDef::new` takes. The types that hold handler objects
//! (`ElementSpec`, `ElementDef`, `FallbackDef`, `ScopedPackageDef`,
//! `BackendDef`, `Aspect`, `BackendStage`, `Contributed`)
//! keep each object beside the crossing form its lending answered; the
//! methods that record and lend the objects are the runtime's.
//!
//! The handler traits are the api's with the guest's types: the same
//! methods, receivers, defaults and bounds (`Send + Sync`). The generated
//! surface (`generated.rs`) re-exports every item from its module.

use serde_json::{Map, Value};

use crate::adapter::values::JsonForm;
use crate::guest::abi::ToArg;
use crate::runtime::ExtError;

pub mod core_data_table;
pub mod core_lowering;
pub mod core_registry;
pub mod core_run;
pub mod core_stage;
pub mod host;
pub mod lang_mint;
pub mod lang_rewrites_userdefs;
pub mod model_aspects;
pub mod model_element;
pub mod stdcore_features_citations;
pub mod stdcore_features_compose;
pub mod stdcore_features_math;
pub mod stdcore_features_numbering;
pub mod techy_error;
pub mod techy_transform;

/// `value` converted for a request of `method` ([`ToArg::arg`]), the
/// method's name filled in on an [`ExtError::Argument`] that names none, as
/// the host's dispatcher fills it.
///
/// # Errors
///
/// What [`ToArg::arg`] answers.
pub(crate) fn arg<T: ToArg + ?Sized>(value: &T, method: &str) -> Result<Value, ExtError> {
    value.arg().map_err(|error| match error {
        ExtError::Argument { method: named, detail } if named.is_empty() => ExtError::Argument { method: method.to_string(), detail },
        other => other,
    })
}

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

/// `"WORD"` or `{"emit": [staged, …]}`, the shape of an expansion, a
/// lowering and a rewrite handler's answer: `None` for the word.
///
/// # Errors
///
/// `value` is neither ([`ExtError::Value`]), or an item is not a staged
/// node's handle.
pub(crate) fn emit_from_value(
    what: &str,
    word: &str,
    value: &Value,
) -> Result<Option<Vec<crate::guest::lang::rewrites::userdefs::Staged>>, ExtError> {
    if value.as_str() == Some(word) {
        return Ok(None);
    }
    match single(value, &["emit"]) {
        Some((_, items @ Value::Array(_))) => JsonForm::from_value(items).map(Some),
        _ => Err(not_shape(what, &format!("`\"{word}\"` or `{{\"emit\": [...]}}`"), value)),
    }
}

/// `{"emit": [staged, …]}`.
pub(crate) fn emit_to_value(staged: &[crate::guest::lang::rewrites::userdefs::Staged]) -> Value {
    serde_json::json!({ "emit": staged.iter().map(JsonForm::to_value).collect::<Vec<_>>() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_argument_that_cannot_cross_is_refused_with_its_methods_name() {
        let error = arg(&[0xff_u8][..], "digest_input").expect_err("bytes that are not a text cannot cross");
        assert!(matches!(&error, ExtError::Argument { method, .. } if method == "digest_input"), "{error:?}");
        assert_eq!(arg("text", "option").ok(), Some(Value::String("text".to_string())));
    }
}
