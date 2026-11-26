//! What the tests of the crate share: a registry holding the standard package
//! and any list of further packages (scripted ones among them), and a
//! document rendered through each output.

#![allow(dead_code)]

pub mod double;
pub mod guest;
pub mod json_double;
pub mod wat;

use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan, PlanWarning};
use flm_core::core::registry::{FlmRegistry, PackageEntry, RegistryBuildError, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdlib::backends::{html, latex_recomposed, text};
use flm_core::stdlib::{self, StdlibConfig};

/// The registry holding the standard package and `packages`, or the build's
/// error.
pub fn try_registry_with(packages: impl IntoIterator<Item = PackageEntry>) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    for package in packages {
        builder.use_package(package);
    }
    builder.build(&RegistryInputs::new())
}

/// The errors of a failed registry build, each as (feature, detail) when it
/// is a feature's own error (`BuildError::Feature`), or panicking on another
/// kind of error. A failing extension feature's detail begins with its
/// identifier (`flm.ext.handler-failed: `, `flm.ext.name-outside-extension: `).
pub fn feature_errors(error: &RegistryBuildError) -> Vec<(String, String)> {
    use flm_core::core::registry::BuildError;
    error
        .errors
        .iter()
        .map(|error| match error {
            BuildError::Feature { feature, detail } => (feature.as_str().to_string(), detail.clone()),
            other => panic!("a feature's error, not {other:?}"),
        })
        .collect()
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
    let plan = Arc::new(workflow_plan(registry, workflow));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The diagnostics of a run, rendered, one per line.
pub fn diagnostics(run: &Run) -> Vec<String> {
    run.diagnostics().iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect()
}

/// The identifier of the Warning a run reports, before its own diagnostics,
/// for each output its plan keeps as unchecked
/// ([`PlanWarning::UncheckedOutput`]): `math-passthrough`, which every
/// registry of these tests holds, declares its formulas unchecked on `html`,
/// `latex-rendered` and `latex-recomposed`. A test whose subject is the
/// diagnostics of such a run lists it among them.
pub const UNCHECKED_OUTPUT: &str = "flm.run.unchecked-output";

/// The Warning [`UNCHECKED_OUTPUT`] of a run of the page, as [`diagnostics`]
/// renders it.
pub const UNCHECKED_ON_HTML: &str = "flm.run.unchecked-output: Unchecked output for the formula renderer in the \
                                     reader's browser: feature ‘math-passthrough’ writes each formula's LaTeX \
                                     source as is (backend ‘html’)";

/// The Warning [`UNCHECKED_OUTPUT`] of a run of the recomposed LaTeX, as
/// [`diagnostics`] renders it.
pub const UNCHECKED_ON_LATEX: &str = "flm.run.unchecked-output: Unchecked output for LaTeX: feature \
                                      ‘math-passthrough’ writes each formula's recomposed LaTeX as is \
                                      (backend ‘latex-recomposed’)";

/// The plan of `workflow` under `registry`, with no overrides: the plan
/// [`run`] and the renderings below run.
pub fn workflow_plan(registry: &Arc<FlmRegistry>, workflow: &str) -> Plan {
    Plan::for_workflow(registry, workflow, &Overrides::none()).expect("a plan")
}

/// Asserts that `diagnostics`, those of a run of `plan` as [`diagnostics`]
/// renders them, are exactly the Warnings [`UNCHECKED_OUTPUT`] of the
/// unchecked outputs the plan keeps, one per such plan warning and with its
/// message, and nothing else: what a test whose subject is not the plan
/// asserts of a run that reports nothing of its own.
#[track_caller]
pub fn assert_only_unchecked_output(plan: &Plan, diagnostics: &[String]) {
    let expected: Vec<String> = plan
        .warnings()
        .iter()
        .filter(|warning| matches!(warning, PlanWarning::UncheckedOutput { .. }))
        .map(|warning| format!("{UNCHECKED_OUTPUT}: {warning}"))
        .collect();
    assert_eq!(diagnostics, expected);
}

/// The unchecked outputs the plan of `workflow` under `registry` keeps for
/// the feature that owns the lowering rule `rule`, as (backend, written,
/// reader).
pub fn unchecked_outputs_of(registry: &Arc<FlmRegistry>, workflow: &str, rule: &str) -> Vec<(String, String, String)> {
    let owner = &registry.rules().iter().find(|entry| entry.rule.name.as_str() == rule).expect("the rule is registered").owner;
    workflow_plan(registry, workflow)
        .warnings()
        .iter()
        .filter_map(|warning| match warning {
            PlanWarning::UncheckedOutput { backend, feature, written, reader, .. } if feature == owner => {
                Some((backend.to_string(), written.clone(), reader.clone()))
            }
            _ => None,
        })
        .collect()
}

/// The identifiers of a run's diagnostics.
pub fn identifiers(run: &Run) -> Vec<String> {
    run.diagnostics().iter().map(|d| d.identifier().to_string()).collect()
}

/// The data of the run's diagnostics carrying `identifier`, in order, as
/// JSON (a handler-failed diagnostic's data is the error's
/// [`ExtError::data`](flm_ext::ExtError::data)).
pub fn data_json(run: &Run, identifier: &str) -> Vec<serde_json::Value> {
    run.diagnostics()
        .iter()
        .filter(|d| d.identifier() == identifier)
        .map(|d| flm_ext::adapter::values::diagnostic_value_to_json(&d.data().serializable_data()))
        .collect()
}

/// The identifiers of diagnostics rendered by [`diagnostics`] (each line
/// `identifier: message`).
pub fn ids(lines: &[String]) -> Vec<&str> {
    lines.iter().map(|line| line.split_once(": ").map_or(line.as_str(), |(identifier, _)| identifier)).collect()
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
    use flm_core::stdlib::features::toc;
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

/// The registry of the standard features with the composer, `package`, and
/// [`TinyNamer`].
pub fn compose_registry(package: PackageEntry) -> Result<Arc<FlmRegistry>, String> {
    use flm_core::core::artifact::ArtifactNamer;
    use flm_core::core::registry::FlmFeature;
    use flm_core::stdlib::backends::lowering::NAMER_SLOT;
    use flm_core::stdlib::features::compose::ComposeFeature;
    let mut standard = stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new());
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

/// A catalog with a compiled package `std` of the standard features, as a host's,
/// and `loader` as its resolver.
pub fn catalog_with(loader: &Arc<flm_ext::ExtensionLoader>) -> flm_core::core::registry::PackageCatalog {
    use flm_core::core::registry::PackageFactory;
    use flm_core::core::settings::PackageSettings;
    let mut catalog = flm_core::core::registry::PackageCatalog::new();
    let standard = PackageFactory::new("std", |settings: &PackageSettings| {
        Ok(stdlib::add_features_from_settings(PackageEntry::new("std"), settings))
    });
    catalog.add_compiled(standard.expect("the factory of the standard features"));
    catalog.add_resolver(Arc::clone(loader) as Arc<dyn flm_core::core::registry::PackageResolver>);
    catalog
}

// ---- extensions held in memory, and their loaders ----

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

// ---- service slots and host services ----

/// The run of a plan demanding `demands` over a document of one tree per text
/// of `texts`, in order.
pub fn run_demanding(registry: &Arc<FlmRegistry>, demands: &[&str], texts: &[&str]) -> Run {
    run_demanding_under(registry, demands, texts, flm_core::progress::Scope::none())
}

/// A progress sheet whose build is cancelled as soon as a row of kind `kind`
/// starts (inside the observer's `row_started`), and its root row: a handler
/// that starts such a row is then running in a cancelled build.
pub fn sheet_cancelled_at(kind: &'static str) -> (flm_core::progress::Sheet, flm_core::progress::Task) {
    use flm_core::progress::{Kind, Observer, Sheet};
    let cancelled: Arc<std::sync::OnceLock<Sheet>> = Arc::default();
    let observer = Observer {
        row_started: Some(Box::new({
            let cancelled = Arc::clone(&cancelled);
            move |row| {
                if row.kind.as_str() == kind {
                    cancelled.get().expect("the sheet").cancel();
                }
                None
            }
        })),
        ..Observer::default()
    };
    let (sheet, root) = Sheet::start(&Kind::new("test.build"), None, observer, None);
    assert!(cancelled.set(sheet.clone()).is_ok(), "the sheet is set once");
    (sheet, root)
}

/// A log sink that keeps every line it receives, as its level, its source
/// and its text, and wants every level.
#[derive(Default)]
pub struct KeptLines(std::sync::Mutex<Vec<(flm_core::progress::LogLevel, String, String)>>);

impl KeptLines {
    /// The lines received so far, in the order received.
    pub fn lines(&self) -> Vec<(flm_core::progress::LogLevel, String, String)> {
        self.0.lock().expect("the lines").clone()
    }
}

impl flm_core::progress::LogSink for KeptLines {
    fn enabled(&self, _level: flm_core::progress::LogLevel) -> bool {
        true
    }

    fn log(&self, record: &flm_core::progress::LogRecord<'_>) {
        self.0.lock().expect("the lines").push((record.level, record.source.to_string(), record.text.to_string()));
    }
}

/// As [`run_demanding`], the run reporting under `progress`.
pub fn run_demanding_under(
    registry: &Arc<FlmRegistry>,
    demands: &[&str],
    texts: &[&str],
    progress: flm_core::progress::Scope,
) -> Run {
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
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run").with_progress(progress);
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

/// A `files` host service over memory, serving the root `doc` alone from a
/// map of the document's folder: a path that is absolute or climbs out is
/// refused, a path listed as a symbolic link leading out is refused, a path
/// with nothing is `null`; another root is a failure of the test. It records
/// every call with its caller.
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
        MemoryHostFiles {
            files,
            links: Vec::new(),
            calls: std::sync::Mutex::new(Vec::new()),
        }
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
        _: &flm_core::progress::Scope,
    ) -> Result<flm_ext::host::HostAnswer, flm_ext::host::HostServiceError> {
        use flm_ext::host::{HostAnswer, HostServiceError};
        use flm_ext::ExtensionFiles;
        self.calls.lock().expect("the calls").push((caller.clone(), method.to_string(), args.clone()));
        let error = |detail: String| HostServiceError { detail };
        let written = args["path"].as_str().ok_or_else(|| error("the path must be a text".to_string()))?;
        let (root, path) = written.split_once(':').expect("the host receives a qualified path");
        assert_eq!(root, "doc", "the host serves the root `doc` alone, not `{written}`");
        let files = &self.files;
        if self.links.iter().any(|link| link == path) {
            return Err(error(format!("the file `{path}` leads outside the document's folder `doc` through a symbolic link")));
        }
        let failed = |e: flm_ext::source::FilesError| match e.kind {
            flm_ext::source::FilesErrorKind::Unreadable(detail) => error(detail),
            _ => error(format!("the path `{path}` must stay inside the document's folder `doc`")),
        };
        match method {
            "read_text" => match files.read_text("", path).map_err(failed)? {
                Some(text) => Ok(HostAnswer::Data(serde_json::Value::String(text))),
                None => Ok(HostAnswer::Data(serde_json::Value::Null)),
            },
            "read_bytes" => match files.read_bytes("", path).map_err(failed)? {
                Some(bytes) => Ok(HostAnswer::Bytes(bytes)),
                None => Ok(HostAnswer::Data(serde_json::Value::Null)),
            },
            other => Err(error(format!("the service `files` has no method `{other}`"))),
        }
    }
}
