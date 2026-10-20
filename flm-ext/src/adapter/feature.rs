//! `ExtFeature`, a scripted feature as the registry sees it, and the
//! late-init hook that compiles the templates of its user definitions.

use std::sync::Arc;

use flm_core::core::registry::{
    BuildError, FeatureBuilder as CoreFeatureBuilder, FeatureInfo, FlmFeature, LateInit, LateInitError,
};
use flm_core::core::settings::SettingsDecl;
use flm_core::lang::rewrites::userdefs::UserDefinition as CoreUserDefinition;
use flm_core::lang::{BuiltLang, FlmDiagnostic, FlmDiagnostics};
use serde_json::Value;

use crate::adapter::builder::BuilderFacade;
use crate::adapter::diag::{ExtCondition, HANDLER_FAILED};
use crate::adapter::stage::nowhere;
use crate::adapter::names::{Owner, NAME_OUTSIDE_EXTENSION};
use crate::adapter::services::DeclaredServices;
use crate::engine::{ExtensionIdentity, FeatureReport};
use crate::runtime::ExtRuntime;

/// One scripted feature as the registry sees it: the runtime that runs the
/// extension, the extension and the package the feature belongs to, the
/// feature's report, its settings declaration (built from the report's
/// settings when the factory was made), its final configuration, the
/// extension's digest, and the host services the package declared.
///
/// Its [`settings`](FlmFeature::settings) answers the declaration. Its
/// [`register`](FlmFeature::register) runs the extension's `register`
/// handler ([`ExtRuntime::register`]) with the configuration, against a
/// façade over the real builder,
/// feeds the extension's digest (the manifest and the engine's bytes, computed
/// once by the loader) into the registry digest, and registers the late-init
/// hook that compiles the templates of the user definitions whose elements
/// the handler registered, except one an expansion hook expands without a
/// template.
pub struct ExtFeature {
    runtime: Arc<dyn ExtRuntime>,
    /// The extension's stem.
    stem: String,
    /// The package's name within the extension.
    package: String,
    feature: FeatureReport,
    decl: Option<SettingsDecl>,
    configuration: Value,
    digest: Arc<[u8]>,
    services: Arc<DeclaredServices>,
}

impl ExtFeature {
    /// The feature `feature` of the package `package` of `extension`, run by
    /// `runtime`, declaring the settings `decl`, registered with the final
    /// configuration `configuration` (`{}` when it declares none or none is
    /// written), feeding `digest` into the registry digest, its stages
    /// lending the host services `services`.
    #[allow(clippy::too_many_arguments, reason = "one argument per part of the feature, each held as given")]
    pub(crate) fn new(
        runtime: Arc<dyn ExtRuntime>,
        extension: &ExtensionIdentity,
        package: &str,
        feature: FeatureReport,
        decl: Option<SettingsDecl>,
        configuration: Value,
        digest: Arc<[u8]>,
        services: Arc<DeclaredServices>,
    ) -> ExtFeature {
        ExtFeature {
            runtime,
            stem: extension.stem.clone(),
            package: package.to_string(),
            feature,
            decl,
            configuration,
            digest,
            services,
        }
    }
}

impl FlmFeature for ExtFeature {
    fn info(&self) -> FeatureInfo {
        let feature = &self.feature;
        let info = FeatureInfo::new(feature.name.as_str(), feature.title.as_str()).docs(feature.docs.as_str());
        match &feature.version {
            Some(version) => info.version(version.as_str()),
            None => info,
        }
    }

    /// The declaration built from the report's settings
    /// ([`FeatureReport::settings`]); `None` when the feature declares none.
    fn settings(&self) -> Option<SettingsDecl> {
        self.decl.clone()
    }

    /// # Errors
    ///
    /// [`BuildError::Feature`] when the handler fails: its detail is
    /// `flm.ext.name-outside-extension: <the error's text>` for a name declared
    /// outside the extension's stem, and `flm.ext.handler-failed: <the
    /// error's text>` for any other failure.
    ///
    /// Each call is one registration: it makes the feature's durable table
    /// for the registry being built ([`DurableTable`](crate::adapter::durable::DurableTable)),
    /// which every handler object the registration makes carries, so that
    /// the table dies with that registry.
    fn register(&self, b: &mut CoreFeatureBuilder<'_>) -> Result<(), BuildError> {
        let feature = b.feature().clone();
        let owner = Owner::new(&self.stem, &self.package, &self.feature.name);
        let mut facade = BuilderFacade::new(b, &owner, &self.services, &self.configuration);
        let result = self.runtime.register(&owner.package, &self.feature.name, &self.configuration, &mut facade);
        let definitions = facade.into_definitions();
        if let Err(error) = result {
            // By the kind, which a failure followed by the texts of the queued
            // jobs (`ExtError::Followed`) answers from its first failure.
            let id = match error.kind() {
                "name_outside_extension" => NAME_OUTSIDE_EXTENSION,
                _ => HANDLER_FAILED,
            };
            return Err(BuildError::Feature { feature, detail: format!("{id}: {error}") });
        }
        b.digest_input(&self.digest)?;
        if !definitions.is_empty() {
            b.late_init(Arc::new(CompileDefinitions::new(definitions, owner.clone(), feature.to_string())))?;
        }
        Ok(())
    }
}

/// The identifier of the adapter's report of a definition with neither a
/// template nor an expansion hook.
pub const DEFINITION_NEEDS_TEMPLATE: &str = "flm.ext.definition-needs-template";

/// The late-init hook: compiles every scripted definition except one an
/// expansion hook expands without a template
/// ([`BuilderFacade::into_definitions`]) against the built
/// language, as a compiled package's own hook does, and reports
/// what the compilation reports. A definition with neither a template nor
/// an expansion hook is not compiled: the hook reports it as the Error
/// [`DEFINITION_NEEDS_TEMPLATE`], naming the feature, the package and the
/// extension, at no place in the document.
pub(crate) struct CompileDefinitions {
    definitions: Vec<Arc<CoreUserDefinition>>,
    owner: Owner,
    feature: String,
}

impl CompileDefinitions {
    /// The hook compiling `definitions`, declared by the feature `feature`
    /// of the package and extension `owner`.
    pub(crate) fn new(definitions: Vec<Arc<CoreUserDefinition>>, owner: Owner, feature: String) -> CompileDefinitions {
        CompileDefinitions { definitions, owner, feature }
    }
}

impl LateInit for CompileDefinitions {
    fn late_init(&self, lang: &BuiltLang, diags: &mut FlmDiagnostics) -> Result<(), LateInitError> {
        for definition in &self.definitions {
            if definition.definition().content.is_none() {
                let message = format!(
                    "the definition `{}` declared by the feature `{}` of the package `{}` of the extension `{}` must \
                     have a template or an expansion hook",
                    definition.definition().display_name(),
                    self.feature,
                    self.owner.package,
                    self.owner.stem
                );
                let condition = ExtCondition::new(DEFINITION_NEEDS_TEMPLATE, message, Value::Null)
                    .map_err(|error| LateInitError::new(error.to_string()))?;
                diags.push(FlmDiagnostic::error(condition, nowhere()));
                continue;
            }
            let (Ok(reported) | Err(reported)) = definition.compile(lang);
            for diagnostic in reported {
                diags.push(diagnostic);
            }
        }
        Ok(())
    }
}
