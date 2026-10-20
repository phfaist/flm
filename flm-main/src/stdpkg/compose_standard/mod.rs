//! The feature `compose-standard` of the package `stdmain`: the template engine `minijinja`
//! and `stdmain`'s own compose templates, contributed to the service slots of the library's
//! feature `compose` ([`flm_core::stdcore::features::compose`]).
//!
//! The feature `compose` assembles each output's entry artifact (the HTML page, the
//! printed text, the recomposed LaTeX document) from the output's regions through a
//! **compose template**, a text with named places for those regions, applied by a
//! **template engine**. It declares two service slots, `flm.compose.engines` and
//! `flm.compose.templates` ([`ENGINES_SLOT`], [`TEMPLATES_SLOT`]), and contributes the
//! template engine `content-only` and no compose template. The package `stdmain` registers
//! `compose` ([`crate::stdpkg::stdmain_factory`]), and the host prefers its composer in every plan
//! of an output it serves ([`crate::session`]).
//!
//! The feature `compose-standard` contributes to those two service slots:
//!
//! - the template engine `minijinja` ([`MinijinjaEngine`]), which applies a compose
//!   template written in the template language of the `minijinja` crate;
//! - `stdmain`'s compose templates, kept under `templates/OUTPUT/` in the forms of an
//!   extension's, compiled into the program by this crate's build script and read by the
//!   reader of an extension's compose templates ([`templates`]); `flm render --help` lists
//!   them under the option `-t`.
//!
//! The service slots are `compose`'s, so these are peer contributions
//! ([`FeatureBuilder::peer_slot`]): where `compose` is switched off in the settings, the
//! registry build drops them. Where `compose-standard` alone is switched off, the composer
//! finds no compose template of `stdmain`, and the default choice `simple` is then a problem
//! of the render, unless another package ships a compose template of that name.
//!
//! # Map of the items
//!
//! - [`ComposeStandardFeature`], [`FEATURE`]: the feature.
//! - [`minijinja`](mod@minijinja) ([`MinijinjaEngine`], [`MINIJINJA`]): the template
//!   engine.
//! - [`templates`]: `stdmain`'s compose templates.

pub mod minijinja;
pub mod templates;

use std::sync::Arc;

use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature};
use flm_core::stdcore::features::compose::{ENGINES_SLOT, TEMPLATES_SLOT, Template, TemplateEngine};

pub use minijinja::{MINIJINJA, MinijinjaEngine};

/// The name under which the feature is registered, `compose-standard`.
pub const FEATURE: &str = "compose-standard";

/// The feature `compose-standard` of `stdmain`: the template engine `minijinja` and `stdmain`'s
/// compose templates, contributed through `peer_slot` (the feature `compose` may be
/// switched off). See the [module documentation](self).
pub struct ComposeStandardFeature {
    templates: Vec<Template>,
}

impl ComposeStandardFeature {
    /// Returns the feature that contributes `templates` to [`TEMPLATES_SLOT`], beside the
    /// template engine `minijinja`. The package `stdmain` gives it its own compose templates
    /// ([`templates::std_templates`]).
    pub fn new(templates: Vec<Template>) -> ComposeStandardFeature {
        ComposeStandardFeature { templates }
    }
}

impl FlmFeature for ComposeStandardFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new(FEATURE, "Standard templates").docs(
            "the template engine `minijinja` and the program's own compose templates",
        )
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        // The slots are `compose`'s, which may be switched off.
        b.peer_slot::<dyn TemplateEngine>(ENGINES_SLOT, Arc::new(MinijinjaEngine))?;
        for template in &self.templates {
            b.peer_slot::<Template>(TEMPLATES_SLOT, Arc::new(template.clone()))?;
        }
        Ok(())
    }
}
