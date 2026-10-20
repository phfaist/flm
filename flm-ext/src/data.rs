//! The data engine: an extension written as data, whose manifest's `flm:`
//! block is its one package and whose `templates/` folder holds its compose
//! templates, each the file `templates/OUTPUT/NAME.yaml` or the folder
//! `templates/OUTPUT/NAME/` holding `flmtemplate.yaml`
//! ([`templates::read_folder`]).

use std::sync::Arc;

use serde_json::Value;

use crate::api::core::registry::FeatureBuilder;
use crate::block;
use crate::engine::{Engine, ExtensionIdentity, LoadError, Loaded, PackageReport, DATA};
use crate::entry::DEFAULT_PACKAGE;
use crate::manifest::{Manifest, FLM_VERSION};
use crate::runtime::{ExtError, ExtRuntime};
use crate::source::ExtensionFiles;
use crate::block::BLOCK_KEYS;
use crate::templates;

/// The data engine: exactly one package, `_`, whose block is the manifest's
/// `flm`, whose uses are the block's `use`, whose templates are the
/// `templates/` folder. It has no scripted feature. The chains of the block's
/// `feature_order` reach the package's entry, as every package's do
/// ([`package_factory`](crate::adapter::package_factory)).
#[derive(Debug, Clone, Copy, Default)]
pub struct DataEngine;

impl Engine for DataEngine {
    fn name(&self) -> &str {
        DATA
    }

    /// Reads the block (each problem a [`LoadError::Manifest`] at
    /// `flm.<path>`), refuses every key the block does not read, reads the
    /// templates; every problem is reported, and any one fails the load.
    fn load(
        &self,
        extension: &ExtensionIdentity,
        manifest: &Manifest,
        files: Box<dyn ExtensionFiles>,
    ) -> Result<Loaded, Vec<LoadError>> {
        let stem = &extension.stem;
        let mut errors = Vec::new();
        let block = match block::read(manifest.flm.as_ref().unwrap_or(&Value::Null)) {
            Ok(block) => block,
            Err(found) => {
                errors.extend(found.into_iter().map(|error| LoadError::Manifest {
                    path: if error.path.is_empty() { "flm".to_string() } else { format!("flm.{}", error.path) },
                    detail: format!("in the manifest of the extension `{stem}`, {}", error.detail),
                }));
                block::Block::default()
            }
        };
        // The keys the block does not read, taken from the `flm` map itself, so
        // that they are refused even when the block has problems.
        let others = manifest.flm.as_ref().and_then(Value::as_object).into_iter().flat_map(|map| map.keys());
        let keys: Vec<String> = BLOCK_KEYS.iter().map(|key| format!("`{key}`")).collect();
        for key in others.filter(|key| !BLOCK_KEYS.contains(&key.as_str())) {
            let detail =
                format!("`flm.{key}` is not a key of the `flm` block of the extension `{stem}`; the keys are {}", keys.join(", "));
            errors.push(LoadError::Manifest { path: format!("flm.{key}"), detail });
        }
        let package = extension.factory_name(DEFAULT_PACKAGE);
        let templates = templates::read_folder(files.as_ref(), &package, extension).unwrap_or_else(|found| {
            errors.extend(found);
            Vec::new()
        });
        if !errors.is_empty() {
            return Err(errors);
        }
        let report = PackageReport {
            name: DEFAULT_PACKAGE.to_string(),
            uses: block.uses.clone(),
            block,
            templates,
            features: Vec::new(),
            host: Vec::new(),
        };
        Ok(Loaded { packages: vec![report], runtime: Arc::new(DataRuntime) })
    }
}

/// The data engine's runtime: it runs no feature.
struct DataRuntime;

impl ExtRuntime for DataRuntime {
    fn register(&self, package: &str, feature: &str, _: &Value, _: &mut dyn FeatureBuilder) -> Result<(), ExtError> {
        Err(ExtError::Argument {
            method: "register".to_string(),
            detail: format!("the package `{package}` of a data extension has no feature `{feature}`"),
        })
    }

    /// `data ` followed by this program's FLM version.
    fn digest_input(&self) -> Vec<u8> {
        format!("data {FLM_VERSION}").into_bytes()
    }
}
