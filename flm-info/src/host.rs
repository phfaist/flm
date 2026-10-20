//! The layer over the host library `flm-main` (cargo feature `flm-main`): the environment of
//! a document the host prepared, and what the host adds to it.
//!
//! The host library prepares a document ([`flm_main::session::prepare`]): it reads the
//! document's front matter, finds the packages the document uses, combines the settings
//! from every source (the program's defaults, the packages of extensions, the front matter,
//! the settings a package gives the workflow, the command line), builds the registry and
//! the plan of the workflow that runs. [`Environment::of_prepared`] makes the
//! [`Environment`] of such a prepared document: its registry, its plan and its package
//! factories, together with a [`HostInfo`] that holds what only the host knows. The views
//! then gain their host fields: where each package comes from, with the description, the
//! author and the engine of an extension's manifest
//! ([`PackageView::origin`](crate::env::PackageView::origin)), and each settings key's value
//! in effect with where it was written
//! ([`SettingKeyView::in_effect`](crate::env::SettingKeyView::in_effect)).
//! [`Environment::of_request`] does the same for a request without a document, and
//! [`Environment::front_matter`] describes the keys a front matter takes.
//!
//! A document shapes its environment through its front matter alone: nothing of its
//! content is read or described here.

// What flm info describes: the environment, which a document influences through its front
// matter alone. See decision rationale at [§flm-dd-dr:info-scope].

use std::collections::BTreeMap;
use std::sync::Arc;

use flm_core::core::registry::PackageFactory;
use flm_core::model::doc::FragmentLayout;
use flm_core::stdcore::features::math::CONTENT;
use flm_ext::Manifest;
use flm_main::files::Files;
use flm_main::seed::{MathChoice, Place, front_matter_schema};
use flm_main::session::{Input, PrepareError, Prepared, Request, prepare};
use flm_main::settings::{FeatureInEffect, KeyInEffect, Origin, Source};
use serde::Serialize;
use serde_json::{Map, Value};

use crate::env::{Environment, is_false, summary_of};

/// The prefix of the identity of an extension compiled into the program, as the host
/// library gives it (`bundled:ENGINE/STEM`, its module `bundled`).
const BUNDLED_PREFIX: &str = "bundled:";

impl Environment {
    /// Makes the environment of the document that the host prepared: its registry, its
    /// plan and its package factories ([`Prepared::registry`], [`Prepared::plan`],
    /// [`Prepared::factories`]), with the [`HostInfo`] of the preparation.
    pub fn of_prepared(prepared: &Prepared) -> Environment {
        let mut env = Environment::new(
            Arc::clone(prepared.registry()),
            Some(Arc::clone(prepared.plan())),
            prepared.factories().to_vec(),
        );
        env.host = Some(HostInfo::of(prepared, &env));
        env
    }

    /// Makes the environment of `request` without a document: the request's input is
    /// replaced by an empty text, which has no front matter, and the host prepares it
    /// ([`prepare`]) with `files` and `compiled`, the compiled packages the program carries
    /// besides those of the host library. A program sets on the request the defaults it
    /// gives every document (its initial list, its math processor, its search path, the
    /// profile), so that the environment is the one a document that specifies nothing
    /// gets.
    ///
    /// # Errors
    ///
    /// The [`PrepareError`] of the preparation, such as a package of the initial list that
    /// cannot be found.
    pub fn of_request(mut request: Request, files: Arc<dyn Files>, compiled: &[PackageFactory]) -> Result<Environment, PrepareError> {
        request.input = Input::Text(String::new());
        let prepared = prepare(request, files, compiled)?;
        Ok(Environment::of_prepared(&prepared))
    }

    /// What the host adds to the environment, for an environment made from a prepared
    /// document; `None` for one made with [`Environment::new`].
    pub fn host(&self) -> Option<&HostInfo> {
        self.host.as_ref()
    }

    /// The keys a document's front matter takes under `flm` (`use`, `initial_use`, `math`,
    /// `features`, `parsing`, `feature_order`, `input_as`, `layout`, `default_profile`),
    /// each with its description and its JSON Schema, as the host's schema of a front
    /// matter describes them ([`front_matter_schema`]) for the features of the
    /// environment's packages. The values the schema suggests for `use` are those of the
    /// host's [`suggestions`](HostInfo::suggestions), none for an environment made with
    /// [`Environment::new`].
    pub fn front_matter(&self) -> FrontMatterView {
        let factories: Vec<&PackageFactory> = self.factories().iter().collect();
        let suggestions = self.host.as_ref().map(|host| host.suggestions.as_slice()).unwrap_or_default();
        let schema = front_matter_schema(&factories, suggestions);
        let keys = schema
            .pointer("/properties/flm/properties")
            .and_then(Value::as_object)
            .map(|properties| {
                properties
                    .iter()
                    .map(|(name, fragment)| {
                        let description = fragment.get("description").and_then(Value::as_str).map(str::to_string);
                        FrontMatterKeyView {
                            name: name.clone(),
                            summary: description.as_deref().map(summary_of).map(str::to_string),
                            description,
                            schema: fragment.clone(),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        FrontMatterView {
            keys,
            definitions: schema.get("$defs").and_then(Value::as_object).cloned().unwrap_or_default(),
        }
    }
}

/// What the host adds to an environment made from a prepared document
/// ([`Environment::of_prepared`]).
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct HostInfo {
    /// The profile in view: the profile that runs, its workflow, its package, its outputs.
    pub profile: ProfileView,
    /// The math processor in use: the package whose feature processes the document's
    /// mathematics in the plan of the profile in view; absent when no package does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub math: Option<MathProcessorView>,
    /// Every package in use, in the order of [`Prepared::packages`], with where it comes
    /// from.
    pub packages: Vec<HostPackageView>,
    /// The settings in effect, by the feature's name: for each feature of the packages in
    /// use that a settings source wrote ([`Prepared::settings_in_effect`]).
    pub settings: BTreeMap<String, FeatureInEffectView>,
    /// The workflows that a settings source writes, by the workflow's name: where each field
    /// it writes was written, by the field's name (`docs`, `extends`, `outputs`, …), as the
    /// settings cascade gives the fields it hands the registry
    /// ([`Cascade::workflows`](flm_core::core::settings::cascade::Cascade::workflows)). A
    /// workflow that no feature declares is declared by these fields, on behalf of the host
    /// ([`HOST_FEATURE`](flm_core::core::registry::HOST_FEATURE)).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub workflows: BTreeMap<String, BTreeMap<String, OriginView>>,
    /// The document's front matter as read.
    pub front_matter: FrontMatterRead,
    /// The search path: the folders in which an extension is looked for, in the order
    /// searched ([`Prepared::search_path`]).
    pub search_path: Vec<FolderView>,
    /// The values the host suggests for an entry of `use` ([`Prepared::suggestions`]): the
    /// compiled packages in name order, then the extensions found on the search path, by
    /// their locators, then the extensions compiled into the program, by their stems.
    pub suggestions: Vec<String>,
}

impl HostInfo {
    fn of(prepared: &Prepared, env: &Environment) -> HostInfo {
        let seed = prepared.seed();
        HostInfo {
            profile: ProfileView {
                name: prepared.profile().to_string(),
                workflow: prepared.workflow().to_string(),
                package: prepared.profile_package().map(str::to_string),
                outputs: prepared.outputs().to_vec(),
            },
            math: math_processor(env),
            packages: prepared
                .packages()
                .iter()
                .map(|name| HostPackageView { name: name.clone(), origin: package_origin(name, Some(prepared)) })
                .collect(),
            settings: prepared
                .settings_in_effect()
                .into_iter()
                .map(|(feature, in_effect)| (feature, FeatureInEffectView::of(&in_effect)))
                .collect(),
            workflows: prepared
                .settings()
                .workflows()
                .into_iter()
                .map(|(workflow, fields)| {
                    let fields = fields.into_iter().map(|(field, value)| (field, OriginView::of(&value.origin))).collect();
                    (workflow, fields)
                })
                .collect(),
            front_matter: FrontMatterRead {
                text: prepared.front_matter_text().map(str::to_string),
                uses: seed.uses.iter().map(|(value, place)| WrittenView::new(value, *place)).collect(),
                initial_use: seed
                    .initial_use
                    .as_ref()
                    .map(|entries| entries.iter().map(|(value, place)| WrittenView::new(value, *place)).collect()),
                math: seed.math.as_ref().map(|(choice, place)| {
                    let value = match choice {
                        MathChoice::Processor(name) => name.clone(),
                        MathChoice::NoProcessor => String::from("false"),
                    };
                    WrittenView::new(&value, *place)
                }),
                default_profile: seed.default_profile.as_ref().map(|profile| WrittenView::new(&profile.name, profile.place)),
                layout: seed.layout.as_ref().map(|(layout, place)| WrittenView::new(layout_spelling(*layout), *place)),
                feature_order: seed
                    .feature_order
                    .iter()
                    .map(|(chain, place)| WrittenView::new(&chain.chain.to_string(), *place))
                    .collect(),
            },
            search_path: prepared
                .search_path()
                .iter()
                .map(|folder| FolderView { path: folder.path.display().to_string(), required: folder.required })
                .collect(),
            suggestions: prepared.suggestions().to_vec(),
        }
    }

    /// Where the package called `name` comes from: listed among [`packages`](HostInfo::packages),
    /// or read from its name.
    pub(crate) fn origin_of(&self, name: &str) -> PackageOrigin {
        self.packages
            .iter()
            .find(|package| package.name == name)
            .map(|package| package.origin.clone())
            .unwrap_or_else(|| package_origin(name, None))
    }
}

/// Where the package called `name` comes from, read from its name as the host library
/// names packages ([`Prepared::packages`]): `<identity>#<package>` for a package of an
/// extension, the identity beginning with `bundled:` for an extension compiled into the
/// program; a compiled package's own name otherwise. The manifest of an extension is the
/// one `prepared` keeps ([`Prepared::manifest`]).
fn package_origin(name: &str, prepared: Option<&Prepared>) -> PackageOrigin {
    let Some((identity, package)) = name.rsplit_once('#') else {
        return PackageOrigin::Compiled;
    };
    let manifest = prepared.and_then(|prepared| prepared.manifest(identity));
    let extension = ExtensionOrigin::new(identity, package, manifest);
    if identity.starts_with(BUNDLED_PREFIX) {
        PackageOrigin::Bundled(extension)
    } else {
        PackageOrigin::Extension(extension)
    }
}

/// The value of `flm.layout` that stands for `layout`, as a front matter writes it. The host
/// library reads these three spellings and keeps them private, so they are repeated here.
fn layout_spelling(layout: FragmentLayout) -> &'static str {
    match layout {
        FragmentLayout::Blocks => "blocks",
        FragmentLayout::Inline => "inline",
        FragmentLayout::InlineOrBlocks => "auto",
    }
}

/// The math processor in use: the package that holds the feature whose stage provides the
/// capability `flm.math.content` among the stages that the plan in view schedules. A plan
/// schedules one provider of each capability it needs, and every standard workflow needs
/// this one; a registry may hold several providers, of which the plan rules out all but
/// one.
fn math_processor(env: &Environment) -> Option<MathProcessorView> {
    let registry = env.registry();
    let plan = env.plan()?;
    let id = registry.cap_id(CONTENT)?;
    let stage = plan
        .rewrites()
        .iter()
        .chain(plan.order())
        .map(|idx| registry.stage(*idx))
        .find(|stage| stage.provides.contains(&id))?;
    let feature = stage.owner.to_string();
    let package = env
        .factories()
        .iter()
        .find(|factory| factory.features().iter().any(|listed| listed.name == *feature))
        .map(|factory| factory.name().to_string())?;
    Some(MathProcessorView { package, feature })
}

/// The profile in view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ProfileView {
    /// The profile's name ([`Prepared::profile`]).
    pub name: String,
    /// The workflow that runs, the profile's name ([`Prepared::workflow`]).
    pub workflow: String,
    /// The profile's package, `profile-NAME`, when it exists and is in use
    /// ([`Prepared::profile_package`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    /// The outputs the request produces, in the workflow's order ([`Prepared::outputs`]).
    pub outputs: Vec<String>,
}

/// The math processor in use.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct MathProcessorView {
    /// The package's name, as [`Prepared::packages`] gives it.
    pub package: String,
    /// The feature that processes the mathematics.
    pub feature: String,
}

/// One package in use, with where it comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct HostPackageView {
    /// The package's name, as [`Prepared::packages`] gives it.
    pub name: String,
    /// Where it comes from.
    pub origin: PackageOrigin,
}

/// Where a package comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PackageOrigin {
    /// A package written in Rust and compiled into the program: a package of FLM's
    /// libraries (such as `stdcore` and `math-passthrough`) or one the program adds.
    Compiled,
    /// A package of an extension found on the search path; the extension's identity is the
    /// path of its manifest.
    Extension(ExtensionOrigin),
    /// A package of an extension compiled into the program; the extension's identity is
    /// `bundled:ENGINE/STEM`.
    Bundled(ExtensionOrigin),
}

/// The extension that provides a package, with the description, the author and the engine
/// that the extension's manifest gives, as the host keeps the manifest of each extension in
/// use ([`Prepared::manifest`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct ExtensionOrigin {
    /// The extension's identity.
    pub identity: String,
    /// The package's name within the extension (`_` for its default package).
    pub package: String,
    /// The extension's description, when its manifest writes one (`extension.description`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The extension's author, when its manifest writes one (`extension.author`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// The engine that runs the extension, as the manifest the host keeps specifies it
    /// (`extension.engine`; `data` when the manifest specifies none): `native` for an
    /// extension written in Rust and compiled into the program. Absent when the host gives
    /// no manifest for the extension.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub engine: Option<String>,
}

impl ExtensionOrigin {
    fn new(identity: &str, package: &str, manifest: Option<&Manifest>) -> ExtensionOrigin {
        ExtensionOrigin {
            identity: identity.to_string(),
            package: package.to_string(),
            description: manifest.and_then(|manifest| manifest.description.clone()),
            author: manifest.and_then(|manifest| manifest.author.clone()),
            engine: manifest.map(|manifest| manifest.engine.clone()),
        }
    }
}

/// The settings in effect for one feature.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct FeatureInEffectView {
    /// Where the feature was switched off, when the source that decides it wrote `false`;
    /// none of its keys is then in effect.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub switched_off: Option<OriginView>,
    /// The value in effect of each key that a source wrote, by the key's name.
    pub keys: BTreeMap<String, InEffectView>,
}

impl FeatureInEffectView {
    fn of(in_effect: &FeatureInEffect) -> FeatureInEffectView {
        FeatureInEffectView {
            switched_off: in_effect.switched_off.as_ref().map(OriginView::of),
            keys: in_effect.keys.iter().map(|(key, value)| (key.clone(), InEffectView::of(value))).collect(),
        }
    }
}

/// The value in effect of one settings key, with where it was written.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct InEffectView {
    /// The value, as data, combined from every source that wrote the key; relative paths
    /// to files resolved.
    pub value: Value,
    /// Where the value was written; for a key merged entry by entry, the source of the
    /// highest rank among those that wrote it.
    pub origin: OriginView,
    /// For a key merged entry by entry, where each entry that a source wrote was written,
    /// by the entry's name.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub entry_origins: BTreeMap<String, OriginView>,
}

impl InEffectView {
    fn of(in_effect: &KeyInEffect) -> InEffectView {
        InEffectView {
            value: in_effect.value.clone(),
            origin: OriginView::of(&in_effect.origin),
            entry_origins: in_effect.entry_origins.iter().map(|(entry, origin)| (entry.clone(), OriginView::of(origin))).collect(),
        }
    }
}

/// Where a setting was written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct OriginView {
    /// The kind of source: `defaults` (the program's defaults), `package` (a package of an
    /// extension), `front-matter`, `workflow` (the settings a package gives the workflow),
    /// `command-line`.
    pub source: String,
    /// How a message refers to the place, as the host writes it ("the program's
    /// defaults", "the option `-O text.width=60`", `FILE:LINE:COLUMN`, …).
    pub text: String,
    /// The file the setting is written in, for a front matter or an extension's manifest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    /// The line in that file, counted from 1, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<u32>,
    /// The column in that line, counted from 1 in bytes, when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<u32>,
    /// For a package's settings and those it gives a workflow, the package's name
    /// (`<identity>#<package>`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    /// For the settings a package gives a workflow, the workflow's name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workflow: Option<String>,
}

impl OriginView {
    fn of(origin: &Origin) -> OriginView {
        let (source, package, workflow) = match &origin.source {
            Source::Defaults => ("defaults", None, None),
            Source::Package { factory, .. } => ("package", Some(factory.clone()), None),
            Source::FrontMatter(_) => ("front-matter", None, None),
            Source::Workflow { workflow, factory, .. } => ("workflow", Some(factory.clone()), Some(workflow.clone())),
            Source::CommandLine(..) => ("command-line", None, None),
        };
        OriginView {
            source: source.to_string(),
            text: origin.to_string(),
            file: origin.source.file().map(str::to_string),
            line: origin.place.map(|place| place.line),
            column: origin.place.map(|place| place.column),
            package,
            workflow,
        }
    }
}

/// The keys of a document's front matter that decide its environment, as the host read
/// them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct FrontMatterRead {
    /// The front matter's text, without its two `---` lines; absent when the document has
    /// no front matter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The entries of `flm.use`, as the host looks each one up.
    pub uses: Vec<WrittenView>,
    /// The entries of `flm.initial_use`, when the front matter writes the key.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_use: Option<Vec<WrittenView>>,
    /// `flm.math`: the math processor's name, or `false` for none.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub math: Option<WrittenView>,
    /// `flm.default_profile`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_profile: Option<WrittenView>,
    /// `flm.layout`: `blocks`, `inline` or `auto`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layout: Option<WrittenView>,
    /// The chains of `flm.feature_order`, each written as feature names separated by ` < `.
    pub feature_order: Vec<WrittenView>,
}

/// One value written in a front matter, with its place.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct WrittenView {
    /// The value, as the host read it.
    pub value: String,
    /// Its line, counted from 1 at the top of the file.
    pub line: u32,
    /// Its column, counted from 1 in bytes.
    pub column: u32,
}

impl WrittenView {
    fn new(value: &str, place: Place) -> WrittenView {
        WrittenView { value: value.to_string(), line: place.line, column: place.column }
    }
}

/// One folder of the search path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub struct FolderView {
    /// The folder, relative to the current folder or absolute.
    pub path: String,
    /// Whether the folder must exist: given by the user, rather than looked in by default.
    #[serde(skip_serializing_if = "is_false")]
    pub required: bool,
}

/// The keys of a front matter, as [`Environment::front_matter`] describes them.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct FrontMatterView {
    /// The keys under `flm`, in the order of the schema.
    pub keys: Vec<FrontMatterKeyView>,
    /// The schemas of the types the keys' schemas refer to, by name (`#/$defs/NAME`).
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub definitions: Map<String, Value>,
}

/// One key of a front matter.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct FrontMatterKeyView {
    /// The key's name, under `flm`.
    pub name: String,
    /// Its description, as written in the schema.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The first paragraph of [`description`](FrontMatterKeyView::description).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Its JSON Schema.
    pub schema: Value,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use flm_core::core::data::store::StageOut;
    use flm_core::core::plan::{Overrides, Plan, PlanCx};
    use flm_core::core::registry::{
        BuildError, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, PackageFactory, RegistryBuilder,
        RegistryInputs,
    };
    use flm_core::core::run::StageCx;
    use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
    use flm_core::model::names::StageName;
    use flm_core::stdcore::features::math::CONTENT;
    use flm_core::stdcore::{self, StdcoreConfig};

    use super::math_processor;
    use crate::Environment;

    /// The check stage of a second math processor: it provides `flm.math.content`, as the
    /// check of `math-passthrough` does.
    struct SecondCheck;

    impl Stage for SecondCheck {
        fn decl(&self) -> StageDecl {
            StageDecl::new("second-math.check", Effect::Check, Scope::PerTree).provides(CONTENT)
        }
        fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
            Needs::new()
        }
        fn run(&self, _cx: &StageCx<'_>, _out: &mut StageOut<'_>) {}
    }

    struct SecondProcessor;

    impl FlmFeature for SecondProcessor {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new("second-math", "A second math processor")
        }
        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            b.stage(Arc::new(SecondCheck))?;
            Ok(())
        }
    }

    fn second_package() -> PackageEntry {
        PackageEntry::new("second-math").feature(Arc::new(SecondProcessor))
    }

    /// Of two features that provide `flm.math.content`, the math processor is the one whose
    /// stage the plan schedules, whatever the order of the registry.
    #[test]
    fn the_math_processor_is_the_provider_that_the_plan_schedules() {
        let mut builder = RegistryBuilder::new();
        builder.use_package(stdcore::package(StdcoreConfig::new()));
        builder.use_package(flm_core::math_passthrough::package());
        builder.use_package(second_package());
        let registry = builder.build(&RegistryInputs::new()).expect("the registry builds");
        let factories = vec![
            stdcore::factory().expect("the factory of stdcore"),
            flm_core::math_passthrough::factory().expect("the factory of math-passthrough"),
            PackageFactory::new("second-math", |_| Ok(second_package())).expect("the factory of second-math"),
        ];
        for (preferred, package) in [("second-math.check", "second-math"), (flm_core::math_passthrough::CHECK, "math-passthrough")] {
            let overrides = Overrides { prefer: vec![StageName::from(preferred)], ..Overrides::none() };
            let plan = Plan::for_workflow(&registry, "text", &overrides).expect("the plan of `text`");
            let env = Environment::new(Arc::clone(&registry), Some(Arc::new(plan)), factories.clone());
            let math = math_processor(&env).expect("a math processor");
            assert_eq!((math.package.as_str(), math.feature.as_str()), (package, package));
        }
        // Without a plan, no stage is scheduled and no processor is in use.
        assert!(math_processor(&Environment::new(registry, None, factories)).is_none());
    }
}
