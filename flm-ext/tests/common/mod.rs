//! What the tests of the crate share: a registry holding the standard package
//! and any list of further packages (scripted ones among them), and a
//! document rendered through each output.

#![allow(dead_code)]

pub mod double;
pub mod guest;
pub mod json_double;
pub mod wat;

use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{FlmRegistry, PackageEntry, RegistryBuildError, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::{self, StdcoreConfig};

/// The registry holding the standard package and `packages`, or the build's
/// error.
pub fn try_registry_with(packages: impl IntoIterator<Item = PackageEntry>) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    for package in packages {
        builder.use_package(package);
    }
    builder.build(&RegistryInputs::new())
}

/// The registry holding the standard package and `packages`.
pub fn registry_with(packages: impl IntoIterator<Item = PackageEntry>) -> Arc<FlmRegistry> {
    try_registry_with(packages).expect("the registry builds")
}

/// The run of `workflow` over the document `text`.
pub fn run(registry: &Arc<FlmRegistry>, workflow: &str, text: &str) -> Run {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Arc::new(Plan::for_workflow(registry, workflow, &Overrides::none()).expect("a plan"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The diagnostics of a run, rendered, one per line.
pub fn diagnostics(run: &Run) -> Vec<String> {
    run.diagnostics().iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect()
}

/// The identifiers of a run's diagnostics.
pub fn identifiers(run: &Run) -> Vec<String> {
    run.diagnostics().iter().map(|d| d.identifier().to_string()).collect()
}

/// The plain text of `source`.
pub fn text_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = run(registry, text::WORKFLOW, source);
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    (out, diagnostics(&run))
}

/// The HTML of `source`, without the page around it.
pub fn html_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = run(registry, html::WORKFLOW, source);
    let out = run
        .table(html::OUTPUT_TABLE)
        .expect("declared")
        .map(|fragments| fragments.trees.values().map(|f| f.html()).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();
    (out, diagnostics(&run))
}

/// The recomposed LaTeX of `source`: the entry artifact.
pub fn latex_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = run(registry, latex_recomposed::WORKFLOW, source);
    let out = run
        .table(latex_recomposed::DOCUMENT)
        .expect("declared")
        .and_then(|set| set.entry().and_then(|entry| entry.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned())))
        .unwrap_or_default();
    (out, diagnostics(&run))
}

/// The table of contents' entries of `text`, as `(level, whether the title
/// is the first argument)`.
pub fn toc_entries(registry: &Arc<FlmRegistry>, text: &str) -> Vec<(u32, bool)> {
    use flm_core::core::plan::PlanRequest;
    use flm_core::model::aspects::ContentRef as CoreContent;
    use flm_core::stdcore::features::toc;
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Plan::build(registry, &PlanRequest::new().demand(toc::ENTRIES.name())).expect("a plan");
    let mut run = Run::new(Arc::new(Doc::new(content)), Arc::new(plan), Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run.table(toc::ENTRIES)
        .expect("declared")
        .map(|table| {
            table.entries.iter().map(|entry| (entry.level, matches!(entry.title, Some(CoreContent::Argument(0))))).collect()
        })
        .unwrap_or_default()
}

/// The page `tiny.EXT`, every other file `tiny-NAME.EXT`.
pub struct TinyNamer;

impl flm_core::core::artifact::ArtifactNamer for TinyNamer {
    fn url_of(
        &self,
        artifact: &flm_core::core::artifact::ArtifactRef<'_>,
        _scope: &flm_core::core::artifact::NamingScope,
    ) -> flm_core::core::artifact::RelPath {
        let extension = artifact.extension.unwrap_or("bin");
        if artifact.entry {
            flm_core::core::artifact::RelPath::new(format!("tiny.{extension}"))
        } else {
            flm_core::core::artifact::RelPath::new(format!("tiny-{}.{extension}", artifact.id.name))
        }
    }
}

/// The registry of the standard package with the composer, `package`, and
/// [`TinyNamer`].
pub fn compose_registry(package: PackageEntry) -> Result<Arc<FlmRegistry>, String> {
    use flm_core::core::artifact::ArtifactNamer;
    use flm_core::core::registry::FlmFeature;
    use flm_core::stdcore::backends::lowering::NAMER_SLOT;
    use flm_core::stdcore::features::compose::ComposeFeature;
    let mut standard = stdcore::package(StdcoreConfig::new());
    standard.features.push(Arc::new(ComposeFeature::new()) as Arc<dyn FlmFeature>);
    let mut builder = RegistryBuilder::new();
    builder.use_package(standard);
    builder.use_package(flm_core::math_passthrough::package());
    builder.use_package(package);
    builder.contribute_to(NAMER_SLOT, Arc::new(TinyNamer) as Arc<dyn ArtifactNamer>);
    builder.build(&RegistryInputs::new()).map_err(|error| error.to_string())
}

/// A loader over `source` alone, with the data engine.
pub fn memory_loader(source: flm_ext::MemorySource) -> Arc<flm_ext::ExtensionLoader> {
    Arc::new(flm_ext::ExtensionLoader::new(vec![Arc::new(source)]))
}

/// A catalog with the compiled `stdcore` and `loader` as its resolver.
pub fn catalog_with(loader: &Arc<flm_ext::ExtensionLoader>) -> flm_core::core::registry::PackageCatalog {
    let mut catalog = flm_core::core::registry::PackageCatalog::new();
    catalog.add_compiled(stdcore::factory().expect("the stdcore factory"));
    catalog.add_resolver(Arc::clone(loader) as Arc<dyn flm_core::core::registry::PackageResolver>);
    catalog
}

// ---- WP-5 ----

/// The manifest of a js extension whose main module is `main`.
pub fn js_manifest(main: &str) -> serde_json::Value {
    serde_json::json!({ "extension": { "engine": "js", "js": { "main": main } } })
}

/// A memory source holding one extension, found as `locator` (also its
/// identity), with the manifest `manifest` and the files `files` (path, text).
pub fn js_extension(locator: &str, files: &[(&str, &str)], manifest: serde_json::Value) -> flm_ext::MemorySource {
    let files = files.iter().fold(flm_ext::MemoryFiles::new(), |held, (path, text)| held.with_text(path, text));
    flm_ext::MemorySource::new().with(locator, locator, manifest, files)
}

/// Case 1's files, as `tests/js/wordcount/` holds them (its manifest is
/// [`js_manifest`]`("main.js")`, the JSON equal to `flmextension.yaml`).
pub const WORDCOUNT_FILES: [(&str, &str); 2] =
    [("main.js", include_str!("../js/wordcount/main.js")), ("helpers.js", include_str!("../js/wordcount/helpers.js"))];

/// Case 3's files, as `tests/js/twopack/` holds them.
pub const TWOPACK_FILES: [(&str, &str); 2] =
    [("main.js", include_str!("../js/twopack/main.js")), ("shared.js", include_str!("../js/twopack/shared.js"))];

/// A loader over `source` with the data engine and the js engine over `host`.
#[cfg(feature = "quickjs")]
pub fn js_loader(host: &flm_ext::quickjs::JsHost, source: flm_ext::MemorySource) -> Arc<flm_ext::ExtensionLoader> {
    Arc::new(flm_ext::ExtensionLoader::new(vec![Arc::new(source)]).with_js(host.clone()))
}

/// The manifest of a wasm extension whose module is `module`.
pub fn wasm_manifest(module: &str) -> serde_json::Value {
    serde_json::json!({ "extension": { "engine": "wasm", "wasm": { "module": module } } })
}

/// A loader over `source` with the data engine and the wasm engine over `host`.
#[cfg(feature = "wasm")]
pub fn wasm_loader(host: &flm_ext::wasm::WasmHost, source: flm_ext::MemorySource) -> Arc<flm_ext::ExtensionLoader> {
    Arc::new(flm_ext::ExtensionLoader::new(vec![Arc::new(source)]).with_wasm(host.clone()))
}

/// The factory `entry` resolves to through `loader`.
pub fn factory_of(loader: &Arc<flm_ext::ExtensionLoader>, entry: &str) -> flm_core::core::registry::PackageFactory {
    use flm_core::core::registry::PackageResolver;
    match loader.resolve(entry) {
        Ok(Some(factory)) => factory,
        Ok(None) => panic!("no extension answers `{entry}`"),
        Err(errors) => panic!("`{entry}` does not load: {:?}", errors.iter().map(|e| e.detail.clone()).collect::<Vec<_>>()),
    }
}

/// The problems of the load `entry` reaches through `loader`, as texts.
pub fn load_problems(loader: &Arc<flm_ext::ExtensionLoader>, entry: &str) -> Vec<String> {
    use flm_core::core::registry::PackageResolver;
    match loader.resolve(entry) {
        Ok(found) => panic!("`{entry}` loads: {:?}", found.map(|f| f.name().clone())),
        Err(errors) => errors.into_iter().map(|error| error.detail).collect(),
    }
}

/// The extension `wc` of the one module `main.js` (its text `main`),
/// loaded with `host`: its package `_`'s factory.
#[cfg(feature = "quickjs")]
pub fn js_factory(host: &flm_ext::quickjs::JsHost, main: &str) -> flm_core::core::registry::PackageFactory {
    let loader = js_loader(host, js_extension("wc", &[("main.js", main)], js_manifest("main.js")));
    factory_of(&loader, "wc")
}

// ---- E6 WP-2a: service slots and host services ----

/// The run of a plan demanding `demands` over a document of one tree per text
/// of `texts`, in order.
pub fn run_demanding(registry: &Arc<FlmRegistry>, demands: &[&str], texts: &[&str]) -> Run {
    use flm_core::core::plan::PlanRequest;
    let mut content = DocContent::new();
    for text in texts {
        let source = Arc::new(FlmSource::new(*text).with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .expect("the tree parses");
        content.place(Arc::new(fragment));
    }
    let request = demands.iter().fold(PlanRequest::new(), |request, demand| request.demand(*demand));
    let plan = Arc::new(Plan::build(registry, &request).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The items of the extension's table `name` after `run`.
pub fn ext_items(run: &Run, name: &str) -> Vec<serde_json::Value> {
    use flm_core::core::data::table::TableHandle;
    run.table(TableHandle::<flm_ext::adapter::table::ExtTable>::new(name))
        .expect("the table is declared")
        .map(|table| table.items().to_vec())
        .unwrap_or_default()
}

/// A `files` host service over memory, serving the `document` scope from a
/// map of the document's folder: a path that is absolute or climbs out is
/// refused, a path listed as a symbolic link leading out is refused, a path
/// with nothing is `null`. It records every call with its caller.
pub struct MemoryHostFiles {
    /// The document's folder.
    pub files: flm_ext::MemoryFiles,
    /// The paths that are symbolic links leading outside the folder.
    pub links: Vec<String>,
    /// Every call: the caller, the method, the arguments.
    pub calls: std::sync::Mutex<Vec<(flm_ext::host::HostCaller, String, serde_json::Value)>>,
}

impl MemoryHostFiles {
    /// The service over the document's folder `files`.
    pub fn new(files: flm_ext::MemoryFiles) -> MemoryHostFiles {
        MemoryHostFiles { files, links: Vec::new(), calls: std::sync::Mutex::new(Vec::new()) }
    }

    /// The same service, `path` a symbolic link leading outside the folder.
    pub fn with_link(mut self, path: &str) -> MemoryHostFiles {
        self.links.push(path.to_string());
        self
    }
}

impl flm_ext::host::HostService for MemoryHostFiles {
    fn name(&self) -> &str {
        flm_ext::host::files::NAME
    }

    fn call(
        &self,
        caller: &flm_ext::host::HostCaller,
        method: &str,
        args: &serde_json::Value,
    ) -> Result<flm_ext::host::HostAnswer, flm_ext::host::HostServiceError> {
        use flm_ext::host::{HostAnswer, HostServiceError};
        use flm_ext::ExtensionFiles;
        self.calls.lock().expect("the calls").push((caller.clone(), method.to_string(), args.clone()));
        let error = |detail: String| HostServiceError { detail };
        assert_eq!(args["scope"], "document", "the host serves the document scope alone");
        let path = args["path"].as_str().ok_or_else(|| error("the path must be a text".to_string()))?;
        let normalized = flm_ext::paths::normalize(path)
            .map_err(|_| error(format!("the path `{path}` must stay inside the document's folder `doc`")))?;
        if self.links.contains(&normalized) {
            return Err(error(format!("the file `{path}` leads outside the document's folder `doc` through a symbolic link")));
        }
        match method {
            "read_text" => match self.files.read_text(&normalized).map_err(|e| error(e.detail))? {
                Some(text) => Ok(HostAnswer::Data(serde_json::Value::String(text))),
                None => Ok(HostAnswer::Data(serde_json::Value::Null)),
            },
            "read_bytes" => match self.files.read_bytes(&normalized).map_err(|e| error(e.detail))? {
                Some(bytes) => Ok(HostAnswer::Bytes(bytes)),
                None => Ok(HostAnswer::Data(serde_json::Value::Null)),
            },
            other => Err(error(format!("the service `files` has no method `{other}`"))),
        }
    }
}
