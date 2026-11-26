//! What the tests of the extension share: a registry holding the standard
//! package, `math-passthrough` and the extension's package, loaded through the
//! loader of the extension interface with the engine `native`, the settings of
//! the feature `phfqit` given as data; and a document rendered through each
//! output.

#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan, PlanWarning};
use flm_core::core::registry::{
    FlmRegistry, PackageEntry, PackageFactory, PackageResolver, RegistryBuildError, RegistryBuilder, RegistryInputs,
};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdlib::backends::{html, latex_recomposed, text};
use flm_core::stdlib::{self, StdlibConfig};
use flm_ext::guest::NamedExtension;
use flm_ext::native::NativeEngine;
use flm_ext::{ExtensionLoader, MemoryFiles, MemorySource};
use serde_json::{json, Value};

/// The extension's stem, which a document's `use` entry names, and the
/// identity the tests' source answers for it: `phfpack`.
pub const STEM: &str = "phfpack";

/// The name the loader gives the extension's one package, the default package
/// `_`: the identity, then `#_`.
pub const PACKAGE: &str = "phfpack#_";

/// The extension's feature, under whose name a document gives its settings:
/// `phfqit`.
pub const FEATURE: &str = "phfqit";

/// The loader of the extension: the engine `native` holding this module, and a
/// source answering the stem `phfpack` with the manifest `engine: native`.
pub fn loader() -> Arc<ExtensionLoader> {
    let engine = NativeEngine::new([NamedExtension { stem: STEM, compiled: &flm_extensions::phfpack::FLM_EXTENSION }]);
    let manifest = json!({ "extension": { "engine": "native" } });
    let source = MemorySource::new().with(STEM, STEM, manifest, MemoryFiles::new());
    Arc::new(ExtensionLoader::new(vec![Arc::new(source)]).with_engine(Arc::new(engine)))
}

/// The factory of the extension's package, as the loader resolves the stem.
pub fn factory() -> PackageFactory {
    match loader().resolve(STEM) {
        Ok(Some(factory)) => factory,
        Ok(None) => panic!("no extension answers `{STEM}`"),
        Err(errors) => panic!("`{STEM}` does not load: {:?}", errors.iter().map(|e| e.detail.clone()).collect::<Vec<_>>()),
    }
}

/// The extension's package built with `settings`, the settings of the feature
/// `phfqit` as a front matter writes them under `features: phfqit:`, read with
/// the feature's declaration.
pub fn package_with(settings: Value) -> PackageEntry {
    let factory = factory();
    let decl = factory.settings(FEATURE).expect("the feature declares settings");
    let Value::Object(map) = settings else { panic!("the settings are a map: {settings}") };
    let read = decl.read(&map).unwrap_or_else(|errors| {
        panic!("the settings read: {:?}", errors.iter().map(ToString::to_string).collect::<Vec<_>>())
    });
    let mut package_settings = PackageSettings::new();
    package_settings.insert(read);
    factory.build(&package_settings).expect("the package builds")
}

/// The extension's package with no settings.
pub fn package() -> PackageEntry {
    package_with(json!({}))
}

/// The registry holding the standard package, `math-passthrough` and the
/// extension's package with `settings` ([`package_with`]), or the build's
/// error.
pub fn try_registry_with(settings: Value) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.use_package(package_with(settings));
    builder.build(&RegistryInputs::new())
}

/// The registry holding the standard package, `math-passthrough` and the
/// extension's package with `settings`.
pub fn registry_with(settings: Value) -> Arc<FlmRegistry> {
    try_registry_with(settings).unwrap_or_else(|error| panic!("the registry builds: {}", error.render()))
}

/// The registry holding the standard package, `math-passthrough` and the
/// extension's package, with no settings.
pub fn registry() -> Arc<FlmRegistry> {
    registry_with(json!({}))
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
/// ([`PlanWarning::UncheckedOutput`]): `math-passthrough`, which the registry
/// holds, declares its formulas unchecked on `html`, `latex-rendered` and
/// `latex-recomposed`.
pub const UNCHECKED_OUTPUT: &str = "flm.run.unchecked-output";

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

/// The identifiers of a run's diagnostics.
pub fn identifiers(run: &Run) -> Vec<String> {
    run.diagnostics().iter().map(|d| d.identifier().to_string()).collect()
}

/// The plain text of `text`.
pub fn text_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = run(registry, text::WORKFLOW, source);
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    (out, diagnostics(&run))
}

/// The HTML of `text`, without the page around it.
pub fn html_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = run(registry, html::WORKFLOW, source);
    let out = run
        .table(html::OUTPUT_TABLE)
        .expect("declared")
        .map(|fragments| fragments.trees.values().map(|f| f.html()).collect::<Vec<_>>().join("\n"))
        .unwrap_or_default();
    (out, diagnostics(&run))
}

/// The recomposed LaTeX of `text`: the entry artifact.
pub fn latex_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = run(registry, latex_recomposed::WORKFLOW, source);
    let out = run
        .table(latex_recomposed::DOCUMENT)
        .expect("declared")
        .and_then(|set| set.entry().and_then(|entry| entry.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned())))
        .unwrap_or_default();
    (out, diagnostics(&run))
}

/// The environment variable that turns the compilation tests on.
pub const ENABLE_VARIABLE: &str = "FLM_TESTS_LATEX_COMPILE";

/// The engines the compilation tests run, in turn.
pub const ENGINES: &[&str] = &["pdflatex", "lualatex"];

/// Whether [`ENABLE_VARIABLE`] turns the compilation tests on: set, and neither
/// empty nor `0`. Where it does, an engine that cannot run is a failure.
pub fn compiling_enabled() -> bool {
    let value = std::env::var(ENABLE_VARIABLE).unwrap_or_default();
    if matches!(value.trim(), "" | "0") {
        return false;
    }
    for engine in ENGINES {
        let runs = Command::new(engine).arg("--version").output().is_ok_and(|out| out.status.success());
        assert!(runs, "{ENABLE_VARIABLE} is set, but `{engine}` cannot be run from the PATH");
    }
    true
}

/// Compiles `body` in a minimal `article` loading `amsmath` under each of
/// [`ENGINES`], in a folder named after `name` under the tests' temporary
/// directory; fails on an engine's error or a missing character.
pub fn compile_under_both_engines(name: &str, body: &str) {
    let document = format!(
        "\\documentclass{{article}}\n\\usepackage{{amsmath}}\n\\begin{{document}}\n{body}\n\\end{{document}}\n"
    );
    for engine in ENGINES {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("phfpack-compile-{name}-{engine}"));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("a temporary directory");
        fs::write(dir.join("doc.tex"), &document).expect("the document");
        let out = Command::new(engine)
            .current_dir(&dir)
            .args(["-interaction=nonstopmode", "-halt-on-error", "doc.tex"])
            .output()
            .unwrap_or_else(|error| panic!("{engine} runs: {error}"));
        let log = fs::read_to_string(dir.join("doc.log")).unwrap_or_default();
        let _ = fs::remove_dir_all(&dir);
        let first_error = log.lines().skip_while(|line| !line.starts_with('!')).take(4).collect::<Vec<_>>().join("\n");
        assert!(out.status.success(), "{engine} fails:\n{first_error}\n\n{document}");
        assert!(!log.contains("Missing character"), "{engine}: a character is missing:\n{log}");
    }
}
