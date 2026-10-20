//! `Double`, an [`ExtRuntime`] whose handlers are Rust closures, so that every
//! path of the adapter is driven without an engine; and closures made into the
//! two traits a scripted feature registers ([`stage`], [`expand`]).

use std::collections::BTreeMap;
use std::sync::Arc;

use flm_core::core::registry::{PackageEntry, PackageFactory};
use flm_core::core::settings::PackageSettings;
use flm_ext::api::core::data::store::StageOut;
use flm_ext::api::core::registry::FeatureBuilder;
use flm_ext::api::core::run::StageCx;
use flm_ext::api::core::stage::{Needs, Stage, StageDecl};
use flm_ext::api::lang::rewrites::userdefs::{Expand, ExpandCx, ExpandPolicy, Expander, Expansion};
use flm_ext::api::techy::node::Node;
use flm_ext::adapter::package_factory;
use flm_ext::adapter::services::DeclaredServices;
use flm_ext::host::{HostCaller, HostService};
use flm_ext::source::ExtensionFiles;
use flm_ext::engine::SettingsReport;
use flm_ext::{ExtError, ExtRuntime, ExtensionIdentity, FeatureReport, MemoryFiles, PackageReport, UseEntry};
use serde_json::Value;

/// A feature's `register` handler, given the builder and the feature's final
/// configuration.
type Register = Box<dyn Fn(&mut dyn FeatureBuilder, &Value) -> Result<(), ExtError> + Send + Sync>;

/// An extension whose identity and stem are its name, with one package `_`
/// whose report is built in Rust and whose `register` handlers are closures,
/// one per feature.
pub struct Double {
    name: String,
    report: PackageReport,
    registers: BTreeMap<String, Register>,
    digest: Vec<u8>,
    offered: BTreeMap<String, Arc<dyn HostService>>,
    files: MemoryFiles,
}

impl Double {
    /// The package `name`, using nothing (the test host registers the
    /// standard package beside it), with no feature yet.
    pub fn new(name: &str) -> Double {
        Double {
            name: name.to_string(),
            report: PackageReport {
                name: "_".to_string(),
                uses: Vec::new(),
                block: Default::default(),
                templates: Vec::new(),
                features: Vec::new(),
                host: Vec::new(),
            },
            registers: BTreeMap::new(),
            digest: Vec::new(),
            offered: BTreeMap::new(),
            files: MemoryFiles::new(),
        }
    }

    /// Declares the host service `name` in the package's `host` list.
    pub fn host(mut self, name: &str) -> Double {
        self.report.host.push(name.to_string());
        self
    }

    /// Has the host offer `service`.
    pub fn offering(mut self, service: Arc<dyn HostService>) -> Double {
        self.offered.insert(service.name().to_string(), service);
        self
    }

    /// Sets the extension's own files (empty by default).
    pub fn files(mut self, files: MemoryFiles) -> Double {
        self.files = files;
        self
    }

    /// Adds the feature `name`, whose `register` handler is `register`.
    pub fn feature(
        self,
        name: &str,
        register: impl Fn(&mut dyn FeatureBuilder) -> Result<(), ExtError> + Send + Sync + 'static,
    ) -> Double {
        self.configured(name, move |b, _| register(b))
    }

    /// Adds the feature `name`, whose `register` handler is `register`, given
    /// the feature's final configuration as well.
    pub fn configured(
        mut self,
        name: &str,
        register: impl Fn(&mut dyn FeatureBuilder, &Value) -> Result<(), ExtError> + Send + Sync + 'static,
    ) -> Double {
        self.report.features.push(FeatureReport {
            name: name.to_string(),
            title: format!("The feature {name}"),
            docs: String::new(),
            version: None,
            settings: None,
        });
        self.registers.insert(name.to_string(), Box::new(register));
        self
    }

    /// Declares `report` as the settings of the feature `feature`.
    pub fn settings(mut self, feature: &str, report: SettingsReport) -> Double {
        if let Some(found) = self.report.features.iter_mut().find(|f| f.name == feature) {
            found.settings = Some(report);
        }
        self
    }

    /// Sets the bytes that stand for the extension's code.
    pub fn digest(mut self, bytes: &[u8]) -> Double {
        self.digest = bytes.to_vec();
        self
    }

    /// Adds an entry of the package's `uses`.
    pub fn uses(mut self, entry: &str) -> Double {
        self.report.uses.push(UseEntry::parse(entry).expect("an entry"));
        self
    }

    /// Changes the package's report with `edit`.
    pub fn describe_with(mut self, edit: impl FnOnce(&mut PackageReport)) -> Double {
        edit(&mut self.report);
        self
    }

    /// The extension's identity: its name, as identity and stem.
    pub fn identity(&self) -> ExtensionIdentity {
        ExtensionIdentity { identity: self.name.clone(), stem: self.name.clone() }
    }

    /// The package's factory, `NAME#_`, or the problems of its report.
    pub fn try_factory(self) -> Result<PackageFactory, Vec<flm_ext::LoadError>> {
        let (extension, report) = (self.identity(), self.report.clone());
        let digest: Arc<[u8]> = Arc::from(self.digest.clone());
        let caller = HostCaller { extension: extension.clone(), package: report.name.clone(), digest_input: Arc::clone(&digest) };
        let services = Arc::new(DeclaredServices::new(caller, &self.offered, &report.host, self.files.shared()));
        package_factory(&extension, &report, Arc::new(self), digest, "", services)
    }

    /// The package's factory, `NAME#_`.
    pub fn factory(self) -> PackageFactory {
        self.try_factory().expect("the report is a package")
    }

    /// The package built from empty settings.
    pub fn package(self) -> PackageEntry {
        self.factory().build(&PackageSettings::new()).expect("the package builds")
    }
}

impl ExtRuntime for Double {
    fn register(&self, package: &str, feature: &str, settings: &Value, b: &mut dyn FeatureBuilder) -> Result<(), ExtError> {
        assert_eq!(package, "_", "the double has one package");
        match self.registers.get(feature) {
            Some(register) => register(b, settings),
            None => Err(ExtError::Handler { detail: format!("no feature `{feature}`"), data: Value::Null }),
        }
    }

    fn digest_input(&self) -> Vec<u8> {
        self.digest.clone()
    }
}

/// A stage's `run` handler.
type Run = Box<dyn Fn(&dyn StageCx, &mut dyn StageOut) -> Result<(), ExtError> + Send + Sync>;

/// A [`Stage`] from its declaration, its needs and a `run` closure.
pub struct StageFn {
    decl: StageDecl,
    needs: Needs,
    run: Run,
}

impl Stage for StageFn {
    fn decl(&self) -> StageDecl {
        self.decl.clone()
    }

    fn needs(&self) -> Needs {
        self.needs.clone()
    }

    fn run<'c>(&self, cx: &'c dyn StageCx, out: &'c mut dyn StageOut) -> Result<(), ExtError> {
        (self.run)(cx, out)
    }
}

/// The stage of `decl` and `needs` whose work is `run`.
pub fn stage(
    decl: StageDecl,
    needs: Needs,
    run: impl Fn(&dyn StageCx, &mut dyn StageOut) -> Result<(), ExtError> + Send + Sync + 'static,
) -> Arc<dyn Stage> {
    Arc::new(StageFn { decl, needs, run: Box::new(run) })
}

/// A hook's `expand` handler.
type ExpandHandler =
    Box<dyn Fn(&dyn Node<'_>, &mut dyn ExpandCx, &mut dyn Expander) -> Result<Expansion, ExtError> + Send + Sync>;

/// An [`Expand`] from a policy and an `expand` closure.
pub struct ExpandFn {
    policy: ExpandPolicy,
    expand: ExpandHandler,
}

impl Expand for ExpandFn {
    fn policy(&self) -> ExpandPolicy {
        self.policy
    }

    fn expand<'c>(
        &self,
        node: &'c dyn Node<'c>,
        cx: &'c mut dyn ExpandCx,
        xp: &'c mut dyn Expander,
    ) -> Result<Expansion, ExtError> {
        (self.expand)(node, cx, xp)
    }
}

/// The hook of policy `policy` whose work is `f`.
pub fn expand(
    policy: ExpandPolicy,
    f: impl Fn(&dyn Node<'_>, &mut dyn ExpandCx, &mut dyn Expander) -> Result<Expansion, ExtError> + Send + Sync + 'static,
) -> Arc<dyn Expand> {
    Arc::new(ExpandFn { policy, expand: Box::new(f) })
}

// ---- WP-4: expansion ----

/// The package `package` with one feature, `defs`, declaring each of
/// `definitions`: its element under the helper's name
/// ([`element_name_under`](flm_ext::api::stdcore::features::userdefs::element_name_under)),
/// expanded by its own template or by the hook given beside it, its lowering
/// rule, and the override of the standard callable when the definition is
/// marked overriding.
pub fn defining(
    package: &str,
    definitions: Vec<(flm_ext::api::lang::rewrites::userdefs::UserDefinition, Option<Arc<dyn Expand>>)>,
) -> Double {
    use flm_ext::api::stdcore::features::userdefs::{element_name_under, expand_in_tree_rule_for};
    let prefix = package.to_string();
    Double::new(package).feature("defs", move |b| {
        for (definition, hook) in &definitions {
            let declared = definition.definition();
            let name = element_name_under(&prefix, declared);
            if declared.overrides {
                b.override_callable(declared.callable_kind(), declared.name.as_str())?;
            }
            b.lowering_rule(expand_in_tree_rule_for(&name, declared))?;
            let element = match hook {
                Some(hook) => definition.element_def_with(&b.feature(), &name, Arc::clone(hook)),
                None => definition.element_def(&b.feature(), &name),
            };
            b.element(element)?;
        }
        Ok(())
    })
}

/// The trees `text` gives once its definitions are expanded: the table
/// `flm.userdefs.expanded` of a run of the plan demanding it alone (no
/// lowering, so a use site a hook kept stays in the tree).
pub fn expanded_trees(
    registry: &Arc<flm_core::core::registry::FlmRegistry>,
    text: &str,
) -> (Vec<Arc<flm_core::techy::core::node::NodeTree<flm_core::lang::Flm>>>, Vec<String>) {
    use flm_core::core::plan::{Plan, PlanRequest};
    use flm_core::core::run::{Externals, Run, RunOptions};
    use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
    use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
    use flm_core::stdcore::features::userdefs::EXPANDED;
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Plan::build(registry, &PlanRequest::new().demand(EXPANDED.name())).expect("a plan");
    let mut run = Run::new(Arc::new(Doc::new(content)), Arc::new(plan), Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    let trees = run.table(EXPANDED).expect("declared").map(|table| table.trees.values().cloned().collect()).unwrap_or_default();
    let diagnostics = run.diagnostics().iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect();
    (trees, diagnostics)
}
