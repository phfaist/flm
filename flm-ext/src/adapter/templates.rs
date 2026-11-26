//! The templates feature: the feature through which a package that ships
//! compose templates contributes them, for every engine.

use std::sync::Arc;

use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use flm_core::core::settings::{SettingsDecl, SettingsError};
use flm_core::stdlib::features::compose::{self, Accepts, Template, TEMPLATES_SLOT};
use serde_json::{json, Value};

use crate::loader::canonical;

/// The feature of a package that ships compose templates: named after the
/// package's factory (`<identity>#<package>`, the one name that is unique and
/// unprefixed), titled by the extension's description, contributing each
/// template to the compose templates slot through `peer_slot`, since the
/// compose feature may be absent or switched off. It takes no settings of its
/// own; its settings declaration declares the settings of each template, as
/// the entry `template_config.OUTPUT.NAME` of the feature `compose`
/// ([`compose::declare_settings`]).
///
/// Its [`register`](FlmFeature::register) feeds the registry digest the
/// extension's digest (the manifest and the engine's bytes, computed once by
/// the loader), then each template's package, name, description and text, so
/// that a changed template changes the digest.
pub struct TemplatesFeature {
    name: String,
    title: String,
    templates: Vec<Template>,
    settings: SettingsDecl,
    digest: Arc<[u8]>,
}

impl TemplatesFeature {
    /// The feature `name` titled `title`, contributing `templates`, with
    /// `settings`, the declaration of their settings
    /// ([`settings_decl`]), and feeding `digest`, the extension's digest, into
    /// the registry digest.
    pub(crate) fn new(
        name: String,
        title: String,
        templates: Vec<Template>,
        settings: SettingsDecl,
        digest: Arc<[u8]>,
    ) -> TemplatesFeature {
        TemplatesFeature { name, title, templates, settings, digest }
    }
}

/// The settings declaration of the templates feature `name` that contributes
/// `templates`: the settings of each template, declared as an entry of the
/// feature `compose` ([`compose::declare_settings`]).
///
/// # Errors
///
/// The problem of two templates with settings of one output and one name, and
/// that of a template's schema whose marks are faulty
/// ([`compose::declare_settings`]).
pub(crate) fn settings_decl(name: &str, templates: &[Template]) -> Result<SettingsDecl, SettingsError> {
    compose::declare_settings(SettingsDecl::new(name), templates)
}

impl FlmFeature for TemplatesFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(self.name.as_str(), self.title.as_str())
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(self.settings.clone())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.digest_input(&self.digest)?;
        for template in &self.templates {
            b.digest_input(&digest_of(template))?;
            // The slot is `compose`'s, which may be switched off.
            b.peer_slot::<Template>(TEMPLATES_SLOT, Arc::new(template.clone()))?;
        }
        Ok(())
    }
}

/// The bytes of `template` the registry digest takes: one JSON list of its
/// package, its name, its description (every field read from it, maps in the
/// canonical order; the settings by their schema, which holds their defaults)
/// and its text.
fn digest_of(template: &Template) -> Vec<u8> {
    let accepts = match &template.accepts {
        Accepts::All => json!("all"),
        Accepts::Only(names) => Value::from(names.iter().map(|name| name.to_string()).collect::<Vec<_>>()),
    };
    let requires: Vec<String> = template.requires.iter().map(|name| name.to_string()).collect();
    let delimiters = template.delimiters.as_ref().map(|d| json!([d.block, d.variable, d.comment]));
    let description = json!({
        "output": template.output,
        "engine": template.engine,
        "accepts": accepts,
        "requires": requires,
        "settings": template.settings.as_ref().map(canonical),
        "delimiters": delimiters,
    });
    let value = json!([template.package, template.name, description, template.text]);
    serde_json::to_vec(&value).unwrap_or_default()
}
