//! The settings of the features ([`Environment::settings`]) and the parsing configuration
//! ([`Environment::parsing_settings`]).

use flm_core::core::settings::schema_of;
use flm_core::lang::FlmParsing;
use flm_core::model::options::Bind;
use serde::Serialize;
use serde_json::{Map, Value};

use super::{Environment, is_false, summary_of};

impl Environment {
    /// The settings of the feature `feature`, from the declaration that its package's
    /// factory lists ([`SettingsDecl`](flm_core::core::settings::SettingsDecl)): every key
    /// with its binding, its JSON Schema, its default and its description, in the order
    /// declared. `None` when no factory of the environment lists the feature or when the
    /// feature takes no settings.
    ///
    /// Under the cargo feature `flm-main`, for an environment made from a prepared
    /// document, each key also holds its value in effect with where it was written, and
    /// the view says where the feature was switched off.
    pub fn settings(&self, feature: &str) -> Option<SettingsView> {
        let decl = self.factories.iter().find_map(|factory| factory.settings(feature))?;
        let schema = decl.json_schema();
        let properties = schema.get("properties").and_then(Value::as_object);
        #[cfg(feature = "flm-main")]
        let in_effect = self.host.as_ref().and_then(|host| host.settings.get(feature));
        let keys = decl
            .keys()
            .map(|key| {
                let fragment = properties.and_then(|properties| properties.get(&key.name)).cloned().unwrap_or(Value::Bool(true));
                let description = fragment.get("description").and_then(Value::as_str).map(str::to_string);
                SettingKeyView {
                    summary: description.as_deref().map(summary_of).map(str::to_string),
                    description,
                    default: fragment.get("default").cloned(),
                    merge_entries: decl.mark_at(&key.name).is_some_and(|mark| mark.merge_entries),
                    bind: key.bind,
                    #[cfg(feature = "flm-main")]
                    in_effect: in_effect.and_then(|feature| feature.keys.get(&key.name)).cloned(),
                    name: key.name,
                    schema: fragment,
                }
            })
            .collect();
        Some(SettingsView {
            feature: feature.to_string(),
            description: schema.get("description").and_then(Value::as_str).map(str::to_string),
            keys,
            definitions: schema.get("$defs").and_then(Value::as_object).cloned().unwrap_or_default(),
            #[cfg(feature = "flm-main")]
            switched_off: in_effect.and_then(|feature| feature.switched_off.clone()),
        })
    }

    /// The parsing configuration the registry was built with
    /// ([`BuiltLang::parsing`](flm_core::lang::BuiltLang::parsing), [`FlmParsing`]): each
    /// key (`comments`, `comment_prefix`, `dollar_math`) with its value, its default and its
    /// description, from the configuration's JSON Schema.
    pub fn parsing_settings(&self) -> ParsingView {
        let schema = schema_of::<FlmParsing>();
        let properties = schema.get("properties").and_then(Value::as_object);
        let values = match serde_json::to_value(self.registry.lang().parsing()) {
            Ok(Value::Object(values)) => values,
            _ => Map::new(),
        };
        let keys = values
            .into_iter()
            .map(|(name, value)| {
                let fragment = properties.and_then(|properties| properties.get(&name)).cloned().unwrap_or(Value::Bool(true));
                let description = fragment.get("description").and_then(Value::as_str).map(str::to_string);
                ParsingKeyView {
                    summary: description.as_deref().map(summary_of).map(str::to_string),
                    description,
                    default: fragment.get("default").cloned(),
                    name,
                    value,
                    schema: fragment,
                }
            })
            .collect();
        ParsingView { keys }
    }
}

/// The settings of one feature, as [`Environment::settings`] answers them.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct SettingsView {
    /// The feature's name, under which its settings are written.
    pub feature: String,
    /// The description of the feature's settings, as plain text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Every key, in the order declared.
    pub keys: Vec<SettingKeyView>,
    /// The schemas of the types the keys' schemas refer to, by name: a key's schema refers
    /// to one as `#/$defs/NAME`.
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub definitions: Map<String, Value>,
    /// Where the feature was switched off, when a settings source wrote `false` for it;
    /// present only for an environment made from a prepared document (cargo feature
    /// `flm-main`).
    #[cfg(feature = "flm-main")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub switched_off: Option<crate::host::OriginView>,
}

/// One settings key of a feature.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct SettingKeyView {
    /// The key's name, written under the feature's name.
    pub name: String,
    /// Its binding: where a value for it may be set (`registry`, `document`, `render`,
    /// `either`).
    pub bind: Bind,
    /// Whether two sources of settings combine the key's value entry by entry, instead of
    /// the higher-ranked source replacing it whole.
    #[serde(skip_serializing_if = "is_false")]
    pub merge_entries: bool,
    /// Its default value, as data, when the schema states one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    /// Its description, as plain text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The first paragraph of [`description`](SettingKeyView::description).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Its JSON Schema, as the feature's settings schema gives it
    /// ([`SettingsDecl::json_schema`](flm_core::core::settings::SettingsDecl::json_schema)).
    pub schema: Value,
    /// Its value in effect, with where it was written, when a settings source wrote it;
    /// present only for an environment made from a prepared document (cargo feature
    /// `flm-main`). A key that no source wrote has the feature's default.
    #[cfg(feature = "flm-main")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub in_effect: Option<crate::host::InEffectView>,
}

/// The parsing configuration of a registry, as [`Environment::parsing_settings`] answers
/// it.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct ParsingView {
    /// Every key of the configuration, in its order.
    pub keys: Vec<ParsingKeyView>,
}

/// One key of the parsing configuration.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct ParsingKeyView {
    /// The key's name.
    pub name: String,
    /// The value the registry was built with.
    pub value: Value,
    /// The default value, when the schema states one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
    /// The description, as plain text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The first paragraph of [`description`](ParsingKeyView::description).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// The key's JSON Schema.
    pub schema: Value,
}
