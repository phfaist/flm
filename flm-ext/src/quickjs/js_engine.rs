//! The engine `js` ([`JsEngine`]) and [`ExtensionLoader::with_js`]: loading
//! an extension whose main module, written in JavaScript, declares its
//! packages from its default export, and reading the package objects into
//! [`PackageReport`](crate::engine::PackageReport)s (`binding::packages`).

use std::collections::BTreeMap;
use std::sync::Arc;

use super::engine::{self, Engine as Instance};
use super::host::JsHost;
use super::package::{self, JsExtension};
use crate::binding::packages::read_packages;
use crate::binding::{self, BindingRuntime};
use crate::engine::{Engine, ExtensionIdentity, LoadError, Loaded};
use crate::loader::ExtensionLoader;
use crate::manifest::Manifest;
use crate::paths::{self, PathErrorKind};
use crate::source::ExtensionFiles;

/// The name of the engine.
const JS: &str = "js";

/// The engine `js`: an extension whose main module, written in JavaScript,
/// declares its packages from its default export. Created with the host every
/// engine instance of the extensions it loads uses.
///
/// [`load`](Engine::load) reads the main module (`extension.js.main`) and
/// every module it imports through the extension's files, on the calling
/// thread, once; calls the default export with the extension builder; and
/// reports one package per `ext.package(name, object)` call. Every other
/// engine instance of the extension, on any thread, is built from the
/// modules recorded then. The module page ([`crate::quickjs`]) gives the
/// shapes and the rules.
#[derive(Debug, Clone)]
pub struct JsEngine {
    /// The host.
    host: JsHost,
}

impl JsEngine {
    /// The engine `js` over `host`.
    pub fn new(host: JsHost) -> JsEngine {
        JsEngine { host }
    }
}

impl ExtensionLoader {
    /// The loader with the engine `js` over `host`:
    /// `self.with_engine(Arc::new(JsEngine::new(host)))`.
    pub fn with_js(self, host: JsHost) -> ExtensionLoader {
        self.with_engine(Arc::new(JsEngine::new(host)))
    }
}

impl Engine for JsEngine {
    /// `js`.
    fn name(&self) -> &str {
        JS
    }

    /// Loads the extension: the main module's path normalized, its text read,
    /// an engine instance created in the files mode (every module of the
    /// static import graph read and recorded, the default export called), the
    /// package objects read, the templates read; the instance is then kept on
    /// this thread for the registry build.
    fn load(
        &self,
        extension: &ExtensionIdentity,
        manifest: &Manifest,
        files: Box<dyn ExtensionFiles>,
    ) -> Result<Loaded, Vec<LoadError>> {
        let stem = &extension.stem;
        let at_main = |detail: String| vec![LoadError::Manifest { path: "extension.js.main".to_string(), detail }];
        let written = manifest.js.as_ref().map(|js| js.main.clone()).unwrap_or_default();
        let main = paths::normalize(&written).map_err(|error| {
            at_main(match error.kind {
                PathErrorKind::LeavesRoot => format!(
                    "`extension.js.main` of the extension `{stem}` is `{written}`, which leaves the extension's folder"
                ),
                PathErrorKind::Absolute => format!(
                    "`extension.js.main` of the extension `{stem}` is `{written}`, which is an absolute path; it must \
                     be a path inside the extension's folder"
                ),
                PathErrorKind::Backslash => format!(
                    "`extension.js.main` of the extension `{stem}` is `{written}`, which holds `\\`; a path inside \
                     the extension uses `/`"
                ),
            })
        })?;
        let text = match files.read_text(&main) {
            Ok(Some(text)) => text,
            Ok(None) => {
                return Err(at_main(format!(
                    "the extension `{stem}` names its main module `{main}` (`extension.js.main`), and the extension \
                     holds no file `{main}`"
                )));
            }
            Err(error) => {
                return Err(vec![LoadError::Load {
                    detail: format!("the main module `{main}` of the extension `{stem}` cannot be read: {error}"),
                }]);
            }
        };
        let mut loading = JsExtension {
            id: binding::next_extension_id(),
            host: self.host.clone(),
            settings: match &manifest.js {
                Some(js) => self.host.settings().with_limits(&js.limits),
                None => self.host.settings().clone(),
            },
            extension: extension.clone(),
            main: main.clone(),
            modules: BTreeMap::from([(main, Arc::from(text))]),
            define: true,
        };
        let created = Instance::create(&self.host, &loading, Some(files)).map_err(|not| {
            not.details.into_iter().map(|detail| LoadError::Load { detail }).collect::<Vec<_>>()
        })?;
        let files = match created.files {
            Some(files) => files,
            None => return Err(vec![LoadError::Load { detail: format!("the files of the extension `{stem}` were lost") }]),
        };
        let defined = created.defined.unwrap_or_default();
        let packages = read_packages(extension, &defined, &*files)?;
        drop(files);
        let digest = package::digest(&self.host, &created.recorded);
        loading.modules = created.recorded;
        // Shared before it is kept: the thread's map holds it weakly.
        let loaded = Arc::new(loading);
        engine::keep(&self.host, &loaded, created.engine);
        Ok(Loaded { packages, runtime: Arc::new(BindingRuntime { binding: loaded, digest }) })
    }
}
