//! What the tests of the extension share: a registry holding the standard
//! package and the extension's package, loaded through the loader of the
//! extension interface with the engine `native`, the settings of the feature
//! `math-techxt` given as data; and a document rendered through an output.

#![allow(dead_code)]

use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{
    FlmRegistry, PackageEntry, PackageFactory, PackageResolver, RegistryBuildError, RegistryBuilder, RegistryInputs,
};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::model::addr::TreeId;
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::names::BackendName;
use flm_core::stdlib::backends::lowering::lowered_table;
use flm_core::stdlib::backends::{html, text};
use flm_core::stdlib::{self, StdlibConfig};
use flm_ext::guest::NamedExtension;
use flm_ext::native::NativeEngine;
use flm_ext::{ExtensionLoader, MemoryFiles, MemorySource};
use serde_json::{Value, json};

/// The extension's stem, which a document's `use` entry names, and the
/// identity the tests' source answers for it: `math-techxt`.
pub const STEM: &str = "math-techxt";

/// The name the loader gives the extension's one package, the default package
/// `_`: the identity, then `#_`.
pub const PACKAGE: &str = "math-techxt#_";

/// The extension's feature, under whose name a document gives its settings:
/// `math-techxt`.
pub const FEATURE: &str = "math-techxt";

/// The documents of the formulas' tests on every output.
pub mod documents {
    /// An inline formula with a text command.
    pub const INLINE: &str = "Let \\(a + \\mbox{b} = c\\) hold.\n";

    /// A displayed formula with a text command.
    pub const DISPLAYED: &str = "We have\n\\[ x^2 \\textbf{for all} y \\]\nhence.\n";

    /// A matrix in a displayed math group, its rows ended by `\\`.
    pub const DISPLAYED_MATRIX: &str = "We have\n\\[ \\begin{pmatrix} 1 & 2 \\\\ 30 & 4 \\end{pmatrix} \\]\nhence.\n";

    /// A matrix in an inline math group, its rows ended by `\\`.
    pub const INLINE_MATRIX: &str = "Let \\(M = \\begin{pmatrix} 1 & 2 \\\\ 3 & 4 \\end{pmatrix}\\) hold.\n";

    /// A display-math environment of two lines: a matrix drawn over two lines
    /// and numbered, then a line with a text command and an author's tag.
    pub const ENVIRONMENT: &str = "Lines:\n\\begin{align}\n  a &= \\begin{pmatrix} 1 & 2 \\\\ 30 & 4 \\end{pmatrix} \
                                   \\label{eq:a} \\\\\n  b &\\le \\text{c} \\tag{T}\n\\end{align}\nDone.\n";

    /// An environment whose first line shows nothing at its tag.
    pub const UNTAGGED_LINE: &str = "\\begin{align}\n  a \\nonumber \\\\\n  b\n\\end{align}\n";

    /// [`ENVIRONMENT`], then a reference to its first line.
    pub fn reference() -> String {
        format!("{ENVIRONMENT}See \\eqref{{eq:a}}.\n")
    }

    /// [`ENVIRONMENT`], then a reference to its first line inside a text
    /// command of a formula.
    pub fn reference_in_text() -> String {
        format!("{ENVIRONMENT}Then \\(x \\text{{see \\eqref{{eq:a}}}}\\).\n")
    }

    /// A formula inside a text command of a formula.
    pub const NESTED: &str = "\\(a \\text{for all \\(b^2\\)}\\)\n";

    /// An inline formula that holds a private-use character, so that it gets
    /// no row.
    pub const FORMULA_WITHOUT_ROW: &str = "A \\(a + \u{E001} \\text{b}\\) c.\n";

    /// An environment that holds a private-use character, so that it gets no
    /// row, with a number, a spacing and an author's tag.
    pub const ENVIRONMENT_WITHOUT_ROW: &str =
        "\\begin{align}\n  a + \u{E001} \\label{eq:z} \\\\[2pt]\n  b \\tag{U}\n\\end{align}\n";

    /// Every document above, in their order.
    pub fn all() -> Vec<String> {
        [INLINE, DISPLAYED, DISPLAYED_MATRIX, INLINE_MATRIX, ENVIRONMENT, UNTAGGED_LINE]
            .into_iter()
            .map(String::from)
            .chain([reference(), reference_in_text()])
            .chain([NESTED, FORMULA_WITHOUT_ROW, ENVIRONMENT_WITHOUT_ROW].map(String::from))
            .collect()
    }
}

/// The loader of the extension: the engine `native` holding this module, and a
/// source answering the stem `math-techxt` with the manifest `engine: native`.
pub fn loader() -> Arc<ExtensionLoader> {
    let engine = NativeEngine::new([NamedExtension { stem: STEM, compiled: &flm_extensions::math_techxt::FLM_EXTENSION }]);
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
/// `math-techxt` as a front matter writes them under `features: math-techxt:`,
/// read with the feature's declaration.
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

/// The registry holding the standard package, with its `math` feature, and
/// `package` after it, or the build's error.
pub fn try_registry_of(package: PackageEntry) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdlib::add_features(PackageEntry::new("std"), StdlibConfig::new()));
    builder.use_package(package);
    builder.build(&RegistryInputs::new())
}

/// The registry holding the standard package and the extension's package with
/// `settings` ([`package_with`]).
pub fn registry_with(settings: Value) -> Arc<FlmRegistry> {
    try_registry_of(package_with(settings)).unwrap_or_else(|error| panic!("the registry builds: {}", error.render()))
}

/// The registry holding the standard package and the extension's package, with
/// no settings.
pub fn registry() -> Arc<FlmRegistry> {
    registry_with(json!({}))
}

/// The plan of `workflow` under `registry`, with no overrides.
pub fn workflow_plan(registry: &Arc<FlmRegistry>, workflow: &str) -> Plan {
    Plan::for_workflow(registry, workflow, &Overrides::none()).expect("a plan")
}

/// The name of the rule the plan of `workflow` selects for `element` on
/// `backend`.
pub fn selected_rule(registry: &Arc<FlmRegistry>, workflow: &str, backend: &str, element: &str) -> Option<String> {
    let plan = workflow_plan(registry, workflow);
    let selection = plan.selection(backend)?;
    selection.rule_for(element).map(|idx| registry.rule(idx).rule.name.to_string())
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

/// The text output of a run of the workflow `text`.
pub fn text_output(run: &Run) -> String {
    run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default()
}

/// The plain text of `source`, and the diagnostics of its run.
pub fn text_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = run(registry, text::WORKFLOW, source);
    (text_output(&run), diagnostics(&run))
}

/// The HTML of `source`, without the page around it, the identifiers of the
/// definitions it requires, and the diagnostics of its run.
pub fn html_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>, Vec<String>) {
    let run = run(registry, html::WORKFLOW, source);
    let fragments = run.table(html::OUTPUT_TABLE).expect("declared");
    let fragments: Vec<_> = fragments.map(|fragments| fragments.trees.values().cloned().collect()).unwrap_or_default();
    let out = fragments.iter().map(|f| f.html()).collect::<Vec<_>>().join("\n");
    let requires = fragments.iter().flat_map(|f| f.requires.iter().map(ToString::to_string)).collect();
    (out, requires, diagnostics(&run))
}

/// The number of rounds the backend `backend` lowered the first tree of `run`
/// in.
pub fn rounds(run: &Run, backend: &str) -> u32 {
    run.table(lowered_table(&BackendName::from(backend)))
        .expect("the lowered table is declared")
        .and_then(|trees| trees.get(TreeId::tree(0)))
        .map(|lowered| lowered.rounds)
        .unwrap_or_else(|| panic!("no lowered tree: {:?}", diagnostics(run)))
}
