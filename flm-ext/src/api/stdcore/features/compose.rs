//! Mirrors [`flm_core::stdcore::features::compose`]: a compose template
//! written as a module ([`TemplateModule`]), which describes itself, chooses
//! the files it ships when it is applied, and renders the page.
//!
//! An extension contributes a template module to [`TEMPLATES_SLOT`] with
//! [`FeatureBuilder::peer_slot`](crate::api::core::registry::FeatureBuilder::peer_slot)
//! (or `contribute_to`) and the variant
//! [`Contributed::TemplateModule`](crate::api::core::registry::Contributed::TemplateModule).
//! The adapter then calls [`TemplateModule::describe`] with the feature's
//! configuration and contributes two values: the compose template its
//! description gives, to [`TEMPLATES_SLOT`], and a template engine of this
//! template's own, to [`ENGINES_SLOT`], through which the composer calls
//! [`files`](TemplateModule::files) and [`render`](TemplateModule::render). A
//! document chooses the template by its name, as any other.
//!
//! Across a binding, a module is an object with `name`, `describe(settings)`,
//! `render(data, cx)` and, optionally, `files(data, cx)`. `describe` answers
//! `{ output, accepts, requires, config }`; `files` answers a list of `{
//! name, extension, media_type, text }` (a file whose text the module
//! computed) or `{ name, file, scope, media_type }` (a file of the
//! extension's folder or of the document's folder, `scope` `"extension"` or
//! `"document"`, `media_type` optional); `data` is the [`TemplateContext`] as
//! `{ html, regions: [[name, text], …], pieces: [{ name, place, text, words
//! }, …], meta, config, artifacts: [[name, path], …] }`; `cx` is a
//! [`TemplateCx`].

use serde_json::{Map, Value};

use crate::api::host::{FilesScope, HostHandle};
use crate::runtime::ExtError;
use crate::sealed::Sealed;

pub use flm_core::stdcore::features::compose::{Accepts, PieceValue, TemplateContext};

/// Mirrors [`TEMPLATES_SLOT`](flm_core::stdcore::features::compose::TEMPLATES_SLOT):
/// the service slot of the compose templates, `flm.compose.templates`.
/// Nothing differs.
pub const TEMPLATES_SLOT: &str = flm_core::stdcore::features::compose::TEMPLATES_SLOT;

/// Mirrors [`ENGINES_SLOT`](flm_core::stdcore::features::compose::ENGINES_SLOT):
/// the service slot of the template engines, `flm.compose.engines`. Nothing
/// differs; a template module's engine is contributed there by the adapter.
pub const ENGINES_SLOT: &str = flm_core::stdcore::features::compose::ENGINES_SLOT;

/// No `flm-core` counterpart: a compose template written as a module. It
/// stands for a [`Template`](flm_core::stdcore::features::compose::Template)
/// (what [`describe`](TemplateModule::describe) answers) together with the
/// [`TemplateEngine`](flm_core::stdcore::features::compose::TemplateEngine)
/// that applies it (what [`files`](TemplateModule::files) and
/// [`render`](TemplateModule::render) do).
///
/// The composer runs [`files`](TemplateModule::files) and
/// [`render`](TemplateModule::render) on the thread of its stage. A failure
/// of either (an [`ExtError`]) is reported by the composer as the template's
/// failure, `flm.ext.handler-failed: the template module ... failed: ...`,
/// and the output is not made; a failure of
/// [`describe`](TemplateModule::describe) fails the feature's registration.
pub trait TemplateModule: Send + Sync {
    /// No `flm-core` counterpart: the template's name, which a document
    /// chooses it by (`-t NAME`, or the setting `compose.template`).
    fn name(&self) -> String;

    /// No `flm-core` counterpart: the template's description, given the
    /// feature's final configuration `settings`, called once when the
    /// feature registers.
    ///
    /// # Errors
    ///
    /// The handler's own failure: the feature's registration then fails.
    fn describe(&self, settings: &Value) -> Result<TemplateDescription, ExtError>;

    /// No `flm-core` counterpart (the composer asks it through
    /// [`TemplateEngine::artifacts`](flm_core::stdcore::features::compose::TemplateEngine::artifacts)):
    /// the files the template ships when it is applied to `data`, which the
    /// composer names and writes beside the page and whose paths it gives to
    /// [`render`](TemplateModule::render) in `data.artifacts`. `data.artifacts`
    /// is empty here. The default ships none.
    ///
    /// # Errors
    ///
    /// The handler's own failure: the template then fails.
    fn files(&self, data: &TemplateContext, cx: &dyn TemplateCx) -> Result<Vec<TemplateFile>, ExtError> {
        let _ = (data, cx);
        Ok(Vec::new())
    }

    /// No `flm-core` counterpart (the composer asks it through
    /// [`TemplateEngine::render`](flm_core::stdcore::features::compose::TemplateEngine::render)):
    /// the page, from `data`.
    ///
    /// # Errors
    ///
    /// The handler's own failure: the template then fails.
    fn render(&self, data: &TemplateContext, cx: &dyn TemplateCx) -> Result<String, ExtError>;
}

/// Mirrors the fields of [`Template`](flm_core::stdcore::features::compose::Template)
/// a template module gives: what [`TemplateModule::describe`] answers. What
/// differs: `name` is the module's; `package`, `engine`, `text`, `artifacts`
/// and `files` are the adapter's (the extension's package, the module's own
/// engine, no text, the files [`TemplateModule::files`] answers, none);
/// `delimiters` is left out, since a module has no text to delimit. Across a
/// binding, `{ output, accepts, requires, config }`, `accepts` `"all"` or a
/// list of piece names, `requires` and `config` optional.
#[derive(Debug, Clone, PartialEq)]
pub struct TemplateDescription {
    /// The name of the output the template serves (`html`).
    pub output: String,
    /// The pieces it accepts.
    pub accepts: Accepts,
    /// The pieces it requires.
    pub requires: Vec<String>,
    /// Its own settings, with their defaults, which the render option
    /// `template_config` can change; the module reads them in `data.config`.
    pub config: Map<String, Value>,
}

/// Mirrors [`TemplateArtifact::File`](flm_core::stdcore::features::compose::TemplateArtifact::File):
/// one file a template module ships, what [`TemplateModule::files`] answers.
/// What differs: a file is given by its text or read by the adapter from one
/// of the two roots of the `files` service; `apply` is left out (a module
/// computes what it ships); a folder
/// ([`TemplateArtifact::Folder`](flm_core::stdcore::features::compose::TemplateArtifact::Folder))
/// cannot be shipped yet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateFile {
    /// A file whose text the module computed. Across a binding, `{ name,
    /// extension, media_type, text }`.
    Content {
        /// The file's name, by which the module reads its path
        /// (`data.artifacts`).
        name: String,
        /// The file's extension, without the dot (`css`).
        extension: String,
        /// The file's media type (`text/css`).
        media_type: String,
        /// The file's text.
        text: String,
    },
    /// A file the adapter reads from the extension's folder or from the
    /// document's folder, through the `files` host service, which the
    /// package must declare. Across a binding, `{ name, file, scope,
    /// media_type }`.
    File {
        /// The file's name, by which the module reads its path.
        name: String,
        /// The file's path in its root, with `/`; its extension is what
        /// follows the last `.` of its last part.
        path: String,
        /// The root the path is read in.
        scope: FilesScope,
        /// The file's media type; `None` for the one its extension gives.
        media_type: Option<String>,
    },
}

/// No `flm-core` counterpart: the context a template module's
/// [`files`](TemplateModule::files) and [`render`](TemplateModule::render)
/// receive beside the data, valid for the call.
///
/// The trait is sealed: the adapter alone implements it.
pub trait TemplateCx: Sealed {
    /// No `flm-core` counterpart: the host service `name`, which the
    /// module's package declared, as a handle valid for the call; as
    /// [`StageCx::host`](crate::api::core::run::StageCx::host). A module
    /// reads a file of its own folder through the `files` service, scope
    /// `extension`.
    ///
    /// # Errors
    ///
    /// The package did not declare `name` ([`ExtError::Argument`]).
    fn host(&self, name: &str) -> Result<HostHandle, ExtError>;
}

fn _object_safe(_: &dyn TemplateModule, _: &dyn TemplateCx) {}
