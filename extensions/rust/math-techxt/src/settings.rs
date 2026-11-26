//! The settings of the feature `math-techxt`: the address of the font file
//! that the HTML stylesheet loads.
//!
//! Settings are a feature's configuration written as data, one map per feature
//! from key to value. A document writes the settings of `math-techxt` in its
//! front matter under `flm`, `features`, `math-techxt`. The feature's
//! configuration reader, [`read`], reads them into [`TechxtSettings`] when the
//! host reads a document's settings, so that a value it cannot read is
//! reported there, by its path below the feature's name; `register` reads the
//! configuration the reader checked the same way
//! ([`TechxtSettings::from_settings`]).

use flm_ext::ExtError;
use flm_ext::guest::core::settings::ReadError;
use serde::Serialize;
use serde_json::{Value, json};

use crate::math_techxt::FEATURE;

/// The address of JuliaMono 0.63.2, the default of
/// [`TechxtSettings::font_url`]: a font that holds every character techxt
/// writes, each glyph of one width, so that a formula drawn over several lines
/// aligns.
pub const DEFAULT_FONT_URL: &str = "https://cdnjs.cloudflare.com/ajax/libs/juliamono/0.63.2/JuliaMono-Regular.woff2";

/// The settings of the feature `math-techxt`, as written in data and as the
/// feature uses them. They are read by [`TechxtSettings::from_settings`]; the
/// derived schema ([`schema`]) states the same conditions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(default, deny_unknown_fields)]
pub struct TechxtSettings {
    /// The address of the font file that the HTML stylesheet loads for the
    /// formulas, as written in a CSS `url(…)`: a font in which every glyph has
    /// one width, so that a formula drawn over several lines aligns. Not empty.
    #[schemars(length(min = 1))]
    pub font_url: String,
}

impl Default for TechxtSettings {
    fn default() -> TechxtSettings {
        TechxtSettings { font_url: DEFAULT_FONT_URL.to_string() }
    }
}

impl TechxtSettings {
    /// Reads the settings from `settings`, the configuration value the
    /// feature's reader and its `register` receive. A key not written keeps
    /// its default.
    ///
    /// # Errors
    ///
    /// The value at fault, by its path below the feature's name: the value is
    /// not a map, a key is not `font_url`, or `font_url` is not a non-empty
    /// text.
    pub fn from_settings(settings: &Value) -> Result<TechxtSettings, ReadError> {
        let Value::Object(keys) = settings else {
            return Err(fault("", format!("must be a map, not `{settings}`")));
        };
        let mut read = TechxtSettings::default();
        for (key, value) in keys {
            match (key.as_str(), value) {
                ("font_url", Value::String(url)) if !url.is_empty() => read.font_url = url.clone(),
                ("font_url", other) => return Err(fault("font_url", format!("must be a non-empty text, not `{other}`"))),
                _ => return Err(fault("", format!("the feature `{FEATURE}` has no setting `{key}`"))),
            }
        }
        Ok(read)
    }
}

/// Returns the declaration of the feature's settings, as data: the
/// configuration with its JSON schema ([`schema`]).
pub fn declaration() -> Value {
    json!({
        "config": {
            "schema": schema(),
            "docs": "The address of the font file that the HTML stylesheet loads for the formulas (`font_url`). \
                     Fixed when the registry is built: it cannot vary per document or per render.",
        },
    })
}

/// Returns the JSON schema of the settings ([`TechxtSettings`]), which the
/// feature's settings declaration gives as the schema of its configuration.
pub fn schema() -> Value {
    let mut schema = serde_json::to_value(schemars::schema_for!(TechxtSettings)).unwrap_or_default();
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
/// ([`TechxtSettings::from_settings`]), and answers it unchanged.
///
/// # Errors
///
/// The value at fault, as [`TechxtSettings::from_settings`] states it.
pub fn read(settings: Value) -> Result<Value, ReadError> {
    TechxtSettings::from_settings(&settings)?;
    Ok(settings)
}

/// The error with which `register` fails for the value at fault `error`,
/// worded as the host reports the refusal of the configuration reader
/// ([`read`]).
pub fn refused(error: ReadError) -> ExtError {
    let path = if error.path.is_empty() { FEATURE.to_string() } else { format!("{FEATURE}.{}", error.path) };
    ExtError::Handler { detail: format!("the value of `{path}` is invalid: {}", error.detail), data: Value::Null }
}

/// The value at fault at `path`, below the feature's name, with the condition
/// it violates.
fn fault(path: impl Into<String>, detail: impl Into<String>) -> ReadError {
    ReadError { path: path.into(), detail: detail.into() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_settings_give_the_default_font() {
        assert_eq!(TechxtSettings::from_settings(&json!({})).expect("read").font_url, DEFAULT_FONT_URL);
    }

    #[test]
    fn a_font_url_is_read_and_must_be_a_non_empty_text() {
        let read = TechxtSettings::from_settings(&json!({ "font_url": "fonts/mono.woff2" })).expect("read");
        assert_eq!(read.font_url, "fonts/mono.woff2");
        for value in [json!(""), json!(3), json!(null)] {
            let error = TechxtSettings::from_settings(&json!({ "font_url": value })).expect_err("refused");
            assert_eq!(error.path, "font_url", "{value}");
        }
    }

    #[test]
    fn a_key_other_than_font_url_is_refused() {
        let error = TechxtSettings::from_settings(&json!({ "mode": "x" })).expect_err("refused");
        assert_eq!(error.path, "");
    }

    #[test]
    fn the_schema_describes_the_one_setting_with_its_default() {
        let schema = schema();
        assert_eq!(schema["properties"]["font_url"]["type"], "string");
        assert_eq!(schema["properties"]["font_url"]["default"], DEFAULT_FONT_URL);
        assert_eq!(schema["properties"]["font_url"]["minLength"], 1);
        assert_eq!(schema["additionalProperties"], false);
        assert!(schema.get("$schema").is_none());
    }
}
