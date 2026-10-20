//! The handler trait of `flm:stdcore/features/compose`: [`TemplateModule`].

use serde_json::Value;

use crate::api::stdcore::features::compose::{TemplateContext, TemplateDescription, TemplateFile};
use crate::guest::stdcore::features::compose::TemplateCx;
use crate::runtime::ExtError;

/// Mirrors [`TemplateModule`](crate::api::stdcore::features::compose::TemplateModule):
/// a compose template module. A guest's module is always asked for its
/// files, the default running in the guest.
pub trait TemplateModule: Send + Sync {
    /// Mirrors [`TemplateModule::name`](crate::api::stdcore::features::compose::TemplateModule::name).
    fn name(&self) -> String;

    /// Mirrors [`TemplateModule::describe`](crate::api::stdcore::features::compose::TemplateModule::describe).
    ///
    /// # Errors
    ///
    /// What the module answers.
    fn describe(&self, settings: &Value) -> Result<TemplateDescription, ExtError>;

    /// Mirrors [`TemplateModule::files`](crate::api::stdcore::features::compose::TemplateModule::files):
    /// by default, none.
    ///
    /// # Errors
    ///
    /// What the module answers.
    fn files(&self, data: &TemplateContext, cx: &TemplateCx) -> Result<Vec<TemplateFile>, ExtError> {
        let _ = (data, cx);
        Ok(Vec::new())
    }

    /// Mirrors [`TemplateModule::render`](crate::api::stdcore::features::compose::TemplateModule::render).
    ///
    /// # Errors
    ///
    /// What the module answers.
    fn render(&self, data: &TemplateContext, cx: &TemplateCx) -> Result<String, ExtError>;
}
