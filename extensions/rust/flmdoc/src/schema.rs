//! The words of `\docjsontype`: the type a JSON Schema describes, in one line.

use serde_json::Value;

/// The type the JSON Schema `schema` describes, in words: `string`, `integer or null`,
/// `one of "a", "b"`, `list of string`, `map to string`, the name of a definition it refers
/// to, or `any value`. It reads `enum`, `const`, `$ref`, `anyOf`, `oneOf`, `type`, `items`
/// and `additionalProperties`, in that order of precedence; `type` may be a list of types.
pub(crate) fn type_text(schema: &Value) -> String {
    if let Some(values) = schema.get("enum").and_then(Value::as_array) {
        let values: Vec<String> = values.iter().map(json_text).collect();
        return format!("one of {}", values.join(", "));
    }
    if let Some(value) = schema.get("const") {
        return json_text(value);
    }
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        return reference.rsplit('/').next().unwrap_or(reference).to_string();
    }
    for key in ["anyOf", "oneOf"] {
        if let Some(options) = schema.get(key).and_then(Value::as_array) {
            return options.iter().map(type_text).collect::<Vec<_>>().join(" or ");
        }
    }
    let one = |name: &str| match name {
        "array" => match schema.get("items") {
            Some(items) => format!("list of {}", type_text(items)),
            None => String::from("list"),
        },
        "object" => match schema.get("additionalProperties").filter(|value| value.is_object()) {
            Some(values) => format!("map to {}", type_text(values)),
            None => String::from("map"),
        },
        other => other.to_string(),
    };
    match schema.get("type") {
        Some(Value::String(name)) => one(name),
        Some(Value::Array(names)) => names.iter().filter_map(Value::as_str).map(one).collect::<Vec<_>>().join(" or "),
        _ => String::from("any value"),
    }
}

/// A JSON value as compact JSON text.
fn json_text(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_default()
}
