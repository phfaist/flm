//! A backend written in JavaScript: the fixture `typst` of `tests/js/typst/`
//! (the library's lowering stage, a scripted emit stage over the lowered
//! trees, a scripted final stage writing the one artifact, the output `typst`
//! with the extension `typ`, and a presentation element of its own with a
//! data payload). Its document is rendered through the output's artifact set
//! and, the same document, through the text backend, where the fixture's
//! element never arrives (`\tcallout` lowers to a block there); then a
//! failing emit stage and the output's declaration read from the registry.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;

use flm_core::core::artifact::{ArtifactId, ArtifactSet};
use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageEntry};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_ext::quickjs::{JsHost, JsSettings};

use common::{js_extension, js_loader, js_manifest, registry_with, text_of};

/// The fixture's main module, as `tests/js/typst/` holds it (its manifest is
/// [`js_manifest`]`("main.js")`, the JSON equal to `flmextension.yaml` but for
/// the description).
const TYPST_MAIN: &str = include_str!("js/typst/main.js");

/// The fixture's emitter.
const TYPST_EMIT: &str = include_str!("js/typst/emit.js");

/// The table of the output's artifact set.
const DOCUMENT_TABLE: &str = "typst.document";

/// The document of the goldens (plan section 10.1), without its front
/// matter, which the program reads (`flm-cli/tests/documents/extensions/typst.flm`),
/// and with a definition of the term `defined`, which the last paragraph
/// refers to (without it the reference is unresolved, an error).
const DOCUMENT: &str = "\\section{Dashes and emphasis}

A paragraph with \\emph{emphasis}, \\textbf{bold text}, \\verbcode{code} and a link to
\\href{https://example.org}{the site}; inline mathematics \\(x^2 + y^2\\).

\\begin{itemize}
\\item one
\\item two, \\emph{emphasized}
\\end{itemize}

\\begin{enumerate}
\\item first
\\item second
\\end{enumerate}

\\[ \\int_0^1 f(x) \\, dx \\]

\\tcallout[warning]{A callout with \\emph{content}.}

\\subsection{Verbatim and the rest}

\\begin{verbatimcode}
raw   text
\\end{verbatimcode}

\\begin{defterm}{defined}
A term and its definition.
\\end{defterm}

A footnote\\footnote{the note} and a term \\term{defined}.
";

/// The golden file `name` under `tests/goldens/quickjs/`.
fn golden(name: &str) -> String {
    format!("{}/tests/goldens/quickjs/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The fixture's package `_`, its main module's text changed by `edit`.
fn typst(edit: impl Fn(&str) -> String) -> PackageEntry {
    let main = edit(TYPST_MAIN);
    let loader = js_loader(
        &JsHost::new(JsSettings::default()),
        js_extension("typst", &[("main.js", main.as_str()), ("emit.js", TYPST_EMIT)], js_manifest("main.js")),
    );
    common::factory_of(&loader, "typst").build(&PackageSettings::new()).expect("the package builds")
}

/// The registry of the standard package and the fixture, its main module
/// changed by `edit`.
fn registry_edited(edit: impl Fn(&str) -> String) -> Arc<FlmRegistry> {
    registry_with([typst(edit)])
}

/// The registry of the standard package and the fixture as it is.
fn registry() -> Arc<FlmRegistry> {
    registry_edited(str::to_string)
}

/// The run of `request` over the document `source`.
fn run(registry: &Arc<FlmRegistry>, request: &PlanRequest, source: &str) -> Run {
    let source = Arc::new(FlmSource::new(source).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Arc::new(Plan::build(registry, request).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The output's artifact set of `source` (empty when the table holds none),
/// and the diagnostics.
fn artifacts_of(registry: &Arc<FlmRegistry>, source: &str) -> (ArtifactSet, Vec<String>) {
    let request = PlanRequest::new().demand(DOCUMENT_TABLE).demand("flm.validated");
    let ran = run(registry, &request, source);
    let set = ran
        .table(TableHandle::<ArtifactSet>::new(DOCUMENT_TABLE))
        .expect("the table is declared")
        .cloned()
        .unwrap_or_default();
    (set, common::diagnostics(&ran))
}

#[test]
fn the_fixture_s_registry_builds_without_a_diagnostic() {
    let registry = registry();
    let notes: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(notes.is_empty(), "{notes:?}");
    assert!(registry.stage_idx("flm.typst.lower").is_some());
    assert!(registry.stage_idx("typst.emit").is_some() && registry.stage_idx("typst.final").is_some());
}

#[test]
fn the_output_s_declaration_is_read_from_the_registry() {
    let registry = registry();
    let output = registry.output("typst").expect("the output `typst`");
    assert_eq!(output.def.entry.extension, "typ");
    assert_eq!(output.def.entry.media_type, "application/x-typst");
}

#[test]
fn the_fixture_writes_the_document_as_one_typst_artifact() {
    let (set, diagnostics) = artifacts_of(&registry(), DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(set.len(), 1);
    let entry = set.entry().expect("an entry artifact");
    assert_eq!(entry.id, ArtifactId::new("typst", "document"));
    assert_eq!(entry.media_type, "application/x-typst");
    assert!(entry.entry);
    let text = String::from_utf8(entry.as_bytes().expect("bytes").to_vec()).expect("UTF-8");
    // The callout from its payload, which reaches this backend alone.
    assert!(text.contains("#rect[*warning:* A callout with _content_.]"), "{text}");
    assert_text_golden(golden("typst.typ"), &text);
}

#[test]
fn the_same_document_through_the_text_backend_writes_the_callout_as_a_block() {
    let (text, diagnostics) = text_of(&registry(), DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(text.contains("A callout with content."), "{text}");
    assert!(!text.contains("unsupported"), "{text}");
    assert_text_golden(golden("typst.txt"), &text);
}

#[test]
fn a_failing_emit_stage_is_the_handler_failure_and_no_artifact_is_written() {
    let registry = registry_edited(|main| {
        main.replace("run(cx, out) { out.table(OUTPUT).push(", "run(cx, out) { throw new Error(\"no typst today\"); out.table(OUTPUT).push(")
    });
    let (set, diagnostics) = artifacts_of(&registry, DOCUMENT);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].starts_with("flm.ext.handler-failed: the stage `typst.emit`"), "{diagnostics:?}");
    assert!(diagnostics[0].ends_with("no typst today"), "{diagnostics:?}");
    assert!(set.is_empty(), "{set:?}");
}
