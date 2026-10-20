//! `ExtTemplateEngine`, `flm-core`'s template engine over the extension
//! API's [`TemplateModule`], the [`Template`] value a module's description
//! gives, and the context a call of the module receives
//! ([`TemplateCxFacade`]).
//!
//! # One engine per template
//!
//! A template module is its own engine: the adapter contributes, for each
//! module, the compose template its description gives and an engine of its
//! own, named after the template's package and the template's name joined by
//! `:` (`ext.yaml#_:tiny`). The composer finds a template's engine by the
//! name the template gives, so the name, unique in the registry, leads it to
//! this module and no other, with no table of modules kept anywhere. Nobody
//! writes the name; it shows only in a listing of the engines.
//!
//! # What a failure leaves behind
//!
//! [`TemplateEngine::artifacts`] and [`TemplateEngine::render`] answer
//! `Err(text)`, which the composer reports as the template's failure
//! (`flm.compose.template-failed`), an Error at its stage; the output is
//! not made. A module that fails, or whose files cannot be shipped, answers
//! [`HANDLER_FAILED`]`: the template module ... failed: ...`.

use std::sync::Arc;

use flm_core::core::artifact::media_type_of;
use flm_core::model::names::Name;
use flm_core::stdcore::features::compose::{Template, TemplateArtifact, TemplateContext, TemplateEngine};
use serde_json::{json, Value};

use crate::adapter::diag::HANDLER_FAILED;
use crate::adapter::names::Owner;
use crate::adapter::services::DeclaredServices;
use crate::api::host::{FilesScope, HostHandle};
use crate::api::stdcore::features::compose::{Accepts, TemplateCx, TemplateDescription, TemplateFile, TemplateModule};
use crate::host::{files, HostAnswer};
use crate::loader::canonical;
use crate::runtime::ExtError;
use crate::sealed::Sealed;

/// `flm-core`'s template engine over one extension's [`TemplateModule`]:
/// what a contribution to `flm.compose.templates` registers beside the
/// template (the [module documentation](self)).
///
/// The composer calls it on the thread of its stage; a binding runs the
/// module on that thread's engine.
pub struct ExtTemplateEngine {
    /// The extension's module.
    pub(crate) module: Arc<dyn TemplateModule>,
    /// The engine's name: the template's package and name joined by `:`.
    pub(crate) engine_name: String,
    /// The template's name, the module's.
    pub(crate) template_name: String,
    /// The extension and the package that contributed it.
    pub(crate) owner: Owner,
    /// The feature that contributed it.
    pub(crate) feature: String,
    /// The host services the package declared, which the context lends and
    /// the shipped files are read through.
    pub(crate) services: Arc<DeclaredServices>,
}

impl ExtTemplateEngine {
    /// The text of a failure of the module.
    fn failed(&self, error: impl std::fmt::Display) -> String {
        let (name, feature, package, stem) = (&self.template_name, &self.feature, &self.owner.package, &self.owner.stem);
        format!(
            "{HANDLER_FAILED}: the template module `{name}` of the feature `{feature}` of the package `{package}` of \
             the extension `{stem}` failed: {error}"
        )
    }

    /// The file `file` the module ships, as `flm-core`'s artifact; a file of
    /// a root read through the `files` service.
    fn artifact(&self, file: TemplateFile) -> Result<TemplateArtifact, String> {
        let (name, path, scope, media_type) = match file {
            TemplateFile::Content { name, extension, media_type, text } => {
                return Ok(TemplateArtifact::File { name, extension, media_type, apply: false, bytes: text.into_bytes() });
            }
            TemplateFile::File { name, path, scope, media_type } => (name, path, scope, media_type),
        };
        let from = match scope {
            FilesScope::Extension => "from the extension's folder",
            FilesScope::Document => "from the document's folder",
        };
        let Some(extension) = extension_of_file(&path) else {
            return Err(format!("the file `{path}` it ships {from} must have an extension"));
        };
        let Some(media_type) = media_type.or_else(|| media_type_of(extension).map(str::to_string)) else {
            return Err(format!(
                "the file `{path}` it ships {from} must be given a `media_type`, since \
                 its extension `{extension}` has no known media type"
            ));
        };
        if !self.services.declares(files::NAME) {
            let (package, stem) = (&self.owner.package, &self.owner.stem);
            return Err(format!(
                "the package `{package}` of the extension `{stem}` must declare the host service `files` in its `host` \
                 list for the module to ship the file `{path}` {from}"
            ));
        }
        let extension = extension.to_string();
        let bytes = match self.services.read_file(scope, &path, true) {
            Ok(Some(HostAnswer::Bytes(bytes))) => bytes,
            Ok(Some(HostAnswer::Data(_))) | Ok(None) => {
                return Err(match scope {
                    FilesScope::Extension => {
                        format!("the file `{path}` it ships {from} is not a file of the extension")
                    }
                    FilesScope::Document => format!("the file `{path}` it ships {from} is not there"),
                });
            }
            Err(error) => return Err(format!("the file `{path}` it ships {from} cannot be read: {error}")),
        };
        Ok(TemplateArtifact::File { name, extension, media_type, apply: false, bytes })
    }
}

impl TemplateEngine for ExtTemplateEngine {
    fn name(&self) -> &str {
        &self.engine_name
    }

    /// The module's [`files`](TemplateModule::files), each read or taken as
    /// it is.
    fn artifacts(&self, _template: &Template, context: &TemplateContext) -> Result<Vec<TemplateArtifact>, String> {
        let cx = TemplateCxFacade { services: &self.services };
        let files = self.module.files(context, &cx).map_err(|error| self.failed(error))?;
        files.into_iter().map(|file| self.artifact(file).map_err(|error| self.failed(error))).collect()
    }

    /// The module's [`render`](TemplateModule::render).
    fn render(&self, _template: &Template, context: &TemplateContext) -> Result<String, String> {
        let cx = TemplateCxFacade { services: &self.services };
        self.module.render(context, &cx).map_err(|error| self.failed(error))
    }
}

/// What follows the last `.` of the last part of `path`, when that `.`
/// neither begins nor ends the part, as `Template::from_data` reads a
/// shipped file's extension.
fn extension_of_file(path: &str) -> Option<&str> {
    let last = path.rsplit('/').next().unwrap_or(path);
    match last.rfind('.') {
        Some(dot) if dot > 0 && dot + 1 < last.len() => Some(&last[dot + 1..]),
        _ => None,
    }
}

/// The extension API's [`TemplateCx`]: what [`ExtTemplateEngine`] lends a
/// module's `files` and `render`.
pub struct TemplateCxFacade<'r> {
    services: &'r Arc<DeclaredServices>,
}

impl Sealed for TemplateCxFacade<'_> {}

impl TemplateCx for TemplateCxFacade<'_> {
    fn host(&self, name: &str) -> Result<HostHandle, ExtError> {
        self.services.handle(name)
    }
}

/// The compose template a module's description gives: named `name`,
/// shipped by the package `package` (`<identity>#<package>`), applied by the
/// engine `engine`, with no text, no delimiters, no files of its own (the
/// module's `files` answers them when it is applied) and no include handle.
pub(crate) fn template_of(name: &str, package: &str, engine: &str, description: TemplateDescription) -> Template {
    let TemplateDescription { output, accepts, requires, config } = description;
    Template {
        name: name.to_string(),
        package: package.to_string(),
        output,
        engine: engine.to_string(),
        text: String::new(),
        accepts,
        requires: requires.into_iter().map(Name::from).collect(),
        config,
        delimiters: None,
        artifacts: Vec::new(),
        files: None,
    }
}

/// The bytes of a module's template the registry digest takes: one JSON list
/// of its package, its name and its description (maps in the canonical
/// order). The module's code is in the extension's digest already.
pub(crate) fn digest_of(template: &Template) -> Vec<u8> {
    let accepts = match &template.accepts {
        Accepts::All => json!("all"),
        Accepts::Only(names) => Value::from(names.iter().map(|name| name.to_string()).collect::<Vec<_>>()),
    };
    let requires: Vec<String> = template.requires.iter().map(|name| name.to_string()).collect();
    let description = json!({
        "output": template.output,
        "accepts": accepts,
        "requires": requires,
        "config": canonical(&Value::Object(template.config.clone())),
    });
    serde_json::to_vec(&json!([template.package, template.name, description])).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_shipped_file_s_extension_follows_the_last_dot_of_its_last_part() {
        assert_eq!(extension_of_file("img/logo.png"), Some("png"));
        assert_eq!(extension_of_file("a.b/c.tar.gz"), Some("gz"));
        for path in ["a.b/c", ".hidden", "dir/name.", ""] {
            assert_eq!(extension_of_file(path), None, "{path}");
        }
    }
}
