//! What the tests of the crate share: a registry holding the standard package
//! and `phfpack`, and a document rendered through each output.

#![allow(dead_code)]

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::{self, StdcoreConfig};
use flm_phfpack::settings::PhfqitConfig;

/// The registry holding the standard package and `phfpack` over `config`.
pub fn registry_with(config: PhfqitConfig) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.use_package(flm_phfpack::package(config));
    builder.build(&RegistryInputs::new()).expect("the registry builds")
}

/// The registry holding the standard package and `phfpack`, with no settings.
pub fn registry() -> Arc<FlmRegistry> {
    registry_with(PhfqitConfig::default())
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
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("flm-phfpack-compile-{name}-{engine}"));
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
