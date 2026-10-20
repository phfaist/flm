//! A validator for the subset of JSON Schema the schemas of settings use: those of the
//! features' declarations (`SettingsDecl::json_schema`) and the whole schemas the host
//! assembles around them. It knows the keywords those schemas use and panics on any other,
//! so that a keyword it does not check can never pass data silently; annotations
//! (`description`, `default`, `examples`, `format`, `title`, the `x-flm-*` extensions) are not checked,
//! as JSON Schema's own rules say. Shared by the tests of `flm-core` and `flm-main`, each
//! including this file as a module.

use serde_json::{Map, Value};

/// Whether `data` is valid under `schema`, `$ref`s resolved in `root`.
pub fn valid(root: &Value, schema: &Value, data: &Value) -> bool {
    let object = match schema {
        Value::Bool(accept) => return *accept,
        Value::Object(object) => object,
        other => panic!("not a schema: {other}"),
    };
    object.iter().all(|(keyword, argument)| check(root, object, keyword, argument, data))
}

fn check(root: &Value, schema: &Map<String, Value>, keyword: &str, argument: &Value, data: &Value) -> bool {
    match keyword {
        "description" | "default" | "examples" | "format" | "$defs" | "title" | "$schema" => true,
        _ if keyword.starts_with("x-flm-") => true,
        "$ref" => {
            let reference = argument.as_str().expect("a reference");
            let name = reference.strip_prefix("#/$defs/").expect("a reference into $defs");
            let name = name.replace("~1", "/").replace("~0", "~");
            let definition = root["$defs"].get(&name).unwrap_or_else(|| panic!("no definition `{name}`"));
            valid(root, definition, data)
        }
        "type" => match argument {
            Value::String(name) => has_type(name, data),
            Value::Array(names) => names.iter().any(|name| has_type(name.as_str().unwrap(), data)),
            other => panic!("a type: {other}"),
        },
        "const" => data == argument,
        "enum" => argument.as_array().unwrap().contains(data),
        "pattern" => data.as_str().is_none_or(|text| {
            let pattern = argument.as_str().expect("a pattern");
            regex::Regex::new(pattern).unwrap_or_else(|error| panic!("a pattern: {error}")).is_match(text)
        }),
        "anyOf" => argument.as_array().unwrap().iter().any(|branch| valid(root, branch, data)),
        "oneOf" => argument.as_array().unwrap().iter().filter(|branch| valid(root, branch, data)).count() == 1,
        "minimum" => data.as_f64().is_none_or(|number| number >= argument.as_f64().unwrap()),
        "maximum" => data.as_f64().is_none_or(|number| number <= argument.as_f64().unwrap()),
        "properties" => data.as_object().is_none_or(|object| {
            argument.as_object().unwrap().iter().all(|(key, property)| {
                object.get(key).is_none_or(|value| valid(root, property, value))
            })
        }),
        "additionalProperties" => data.as_object().is_none_or(|object| {
            let declared = schema.get("properties").and_then(Value::as_object);
            object
                .iter()
                .filter(|(key, _)| declared.is_none_or(|declared| !declared.contains_key(*key)))
                .all(|(_, value)| valid(root, argument, value))
        }),
        "required" => data.as_object().is_none_or(|object| {
            argument.as_array().unwrap().iter().all(|key| object.contains_key(key.as_str().unwrap()))
        }),
        "minProperties" => data.as_object().is_none_or(|object| object.len() as u64 >= argument.as_u64().unwrap()),
        "items" => data.as_array().is_none_or(|items| {
            let skip = schema.get("prefixItems").and_then(Value::as_array).map_or(0, Vec::len);
            items.iter().skip(skip).all(|item| valid(root, argument, item))
        }),
        "prefixItems" => data.as_array().is_none_or(|items| {
            argument.as_array().unwrap().iter().zip(items).all(|(schema, item)| valid(root, schema, item))
        }),
        "minItems" => data.as_array().is_none_or(|items| items.len() as u64 >= argument.as_u64().unwrap()),
        "maxItems" => data.as_array().is_none_or(|items| items.len() as u64 <= argument.as_u64().unwrap()),
        "uniqueItems" => data.as_array().is_none_or(|items| {
            !argument.as_bool().unwrap()
                || items.iter().enumerate().all(|(i, item)| !items[..i].contains(item))
        }),
        other => panic!("the validator does not know the keyword `{other}`"),
    }
}

fn has_type(name: &str, data: &Value) -> bool {
    match name {
        "object" => data.is_object(),
        "array" => data.is_array(),
        "string" => data.is_string(),
        "boolean" => data.is_boolean(),
        "null" => data.is_null(),
        "number" => data.is_number(),
        "integer" => data.is_i64() || data.is_u64() || data.as_f64().is_some_and(|n| n.fract() == 0.0),
        other => panic!("the validator does not know the type `{other}`"),
    }
}
