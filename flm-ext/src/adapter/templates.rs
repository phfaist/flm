//! The templates feature: the feature through which a package that ships
//! compose templates contributes them, for every engine.

use std::sync::Arc;

use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use flm_core::stdcore::features::compose::{Accepts, Template, TEMPLATES_SLOT};
use serde_json::{json, Value};

use crate::loader::canonical;

/// The feature of a package that ships compose templates: named after the
/// package's factory (`<identity>#<package>`, the one name that is unique and
/// unprefixed), titled by the extension's description, contributing each
/// template to the compose templates slot through `peer_slot`, since the
/// compose feature may be absent or switched off. It takes no settings.
///
/// Its [`register`](FlmFeature::register) feeds the registry digest the
/// extension's digest (the manifest and the engine's bytes, computed once by
/// the loader), then each template's package, name, description and text, so
/// that a changed template changes the digest.
pub struct TemplatesFeature {
    name: String,
    title: String,
    templates: Vec<Template>,
    digest: Arc<[u8]>,
}

impl TemplatesFeature {
    /// The feature `name` titled `title`, contributing `templates` and
    /// feeding `digest`, the extension's digest, into the registry digest.
    pub(crate) fn new(name: String, title: String, templates: Vec<Template>, digest: Arc<[u8]>) -> TemplatesFeature {
        TemplatesFeature { name, title, templates, digest }
    }
}

impl FlmFeature for TemplatesFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(self.name.as_str(), self.title.as_str())
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
/// canonical order) and its text.
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
        "config": canonical(&Value::Object(template.config.clone())),
        "delimiters": delimiters,
    });
    let value = json!([template.package, template.name, description, template.text]);
    serde_json::to_vec(&value).unwrap_or_default()
}
