//! The feature `compose-standard` of the package `std`: the template engine `minijinja`
//! and the host's own compose templates, contributed to the service slots of the library's
//! feature `compose` ([`flm_core::stdlib::features::compose`]).
//!
//! The feature `compose` assembles each output's entry artifact (the HTML page, the
//! printed text, the recomposed LaTeX document) from the output's regions through a
//! **compose template**, a text with named places for those regions, applied by a
//! **template engine**. It declares two service slots, `flm.compose.engines` and
//! `flm.compose.templates` ([`ENGINES_SLOT`], [`TEMPLATES_SLOT`]), and contributes the
//! template engine `content-only` and no compose template. The package `std` registers
//! `compose` ([`crate::stdpkg::factory`]), and the host prefers its composer in every plan
//! of an output it serves ([`crate::session`]).
//!
//! The feature `compose-standard` contributes to those two service slots:
//!
//! - the template engine `minijinja` ([`MinijinjaEngine`]), which applies a compose
//!   template written in the template language of the `minijinja` crate;
//! - the host's compose templates, kept under `templates/OUTPUT/` in the layouts of an
//!   extension's, compiled into the program by this crate's build script and read by the
//!   reader of an extension's compose templates ([`templates`]); `flm render --help` lists
//!   them under the option `-t`. The feature's settings declaration declares the settings
//!   of each, as the entry `template_config.OUTPUT.NAME` of `compose`
//!   ([`compose::declare_settings`]).
//!
//! The service slots are `compose`'s, so these are peer contributions
//! ([`FeatureBuilder::peer_slot`]): where `compose` is switched off in the settings, the
//! registry build drops them. Where `compose-standard` alone is switched off, the composer
//! finds no compose template of the host, and the default choice `simple` is then a problem
//! of the render, unless another package ships a compose template of that name.
//!
//! # Map of the items
//!
//! - [`ComposeStandardFeature`], [`FEATURE`]: the feature.
//! - [`minijinja`](mod@minijinja) ([`MinijinjaEngine`], [`MINIJINJA`]): the template
//!   engine.
//! - [`templates`]: the host's compose templates.

pub mod minijinja;
pub mod templates;

use std::sync::Arc;

use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use flm_core::core::settings::{SettingsDecl, SettingsError};
use flm_core::stdlib::features::compose;
use flm_core::stdlib::features::compose::{ENGINES_SLOT, TEMPLATES_SLOT, Template, TemplateEngine};

pub use minijinja::{MINIJINJA, MinijinjaEngine};

/// The name under which the feature is registered, `compose-standard`.
pub const FEATURE: &str = "compose-standard";

/// The feature `compose-standard` of `std`: the template engine `minijinja` and the host's
/// compose templates, contributed through `peer_slot` (the feature `compose` may be
/// switched off). See the [module documentation](self).
pub struct ComposeStandardFeature {
    templates: Vec<Template>,
    settings: SettingsDecl,
    engine: MinijinjaEngine,
}

impl ComposeStandardFeature {
    /// Returns the feature that contributes `templates` to [`TEMPLATES_SLOT`], beside
    /// `engine`, the template engine `minijinja`, with the settings declaration that
    /// declares the settings of `templates` ([`compose::declare_settings`]). The package
    /// `std` gives it the host's compose templates ([`templates::std_templates`]) and the
    /// engine made from the reader of the qualified paths of the preparation
    /// ([`StdParts`](crate::stdpkg::StdParts)).
    ///
    /// # Errors
    ///
    /// The [`SettingsError`] of [`compose::declare_settings`] when the settings of
    /// `templates` cannot be declared.
    pub fn new(templates: Vec<Template>, engine: MinijinjaEngine) -> Result<ComposeStandardFeature, SettingsError> {
        let settings = compose::declare_settings(SettingsDecl::new(FEATURE), &templates)?;
        Ok(ComposeStandardFeature { templates, settings, engine })
    }
}

impl FlmFeature for ComposeStandardFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Standard templates").docs(
            "the template engine `minijinja` and the program's own compose templates",
        )
    }

    fn settings(&self) -> Option<SettingsDecl> {
        Some(self.settings.clone())
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        // The slots are `compose`'s, which may be switched off.
        b.peer_slot::<dyn TemplateEngine>(ENGINES_SLOT, Arc::new(self.engine.clone()))?;
        for template in &self.templates {
            b.peer_slot::<Template>(TEMPLATES_SLOT, Arc::new(template.clone()))?;
        }
        Ok(())
    }
}
