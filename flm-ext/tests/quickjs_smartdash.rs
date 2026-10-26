//! A rewrite stage through JavaScript: the fixture `smartdash` of
//! `tests/js/smartdash/` rendered through the text and the HTML page with
//! goldens, its rewrite stage's table demanded beside the output (as the
//! fixture's workflow `smartdash` demands it in a program); a nested
//! `\sdquote` (a nested handler call); mathematics and verbatim untouched; a
//! failing handler; the strict mode. Then the prelude's names of the
//! modules E5 added against the library's, its classes' data forms, and a
//! backend and an output declared from JavaScript (`BackendDef` lending its
//! stage objects).
//!
//! The registry switches the `blocks` feature's own dash ligatures off, so
//! that every dash of the output is the rewrite's. Run under the library's
//! `parallel` feature, the same goldens hold (the rehearsal on the pool's
//! engine instances).

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;

use flm_core::core::artifact::ArtifactSet;
use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{workflow_request, Overrides, Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::backends::{html, text};
use flm_core::stdcore::features::blocks::BlocksConfig;
use flm_core::stdcore::{self, StdcoreConfig};
use flm_ext::quickjs::{eval_for_test, JsHost, JsSettings, TestPackage};
use serde_json::{json, Value};

use common::{js_extension, js_loader, js_manifest};

/// The fixture's main module, as `tests/js/smartdash/` holds it (its manifest
/// is [`js_manifest`]`("main.js")`, the JSON equal to `flmextension.yaml`
/// but for the description).
const SMARTDASH_MAIN: &str = include_str!("js/smartdash/main.js");

/// The capability the fixture's rewrite stage publishes.
const DASHED: &str = "smartdash.dashed";

/// The document of the goldens (plan section 10.2).
const DOCUMENT: &str = "A dash -- an em dash --- and \\sdquote{a quote -- with a dash, and \\sdquote{one inside}}.\n\n\
                        Inside mathematics \\(a--b\\) and verbatim \\verbcode{c--d} nothing changes.";

/// The first line of the text output of [`DOCUMENT`].
const FIRST_LINE: &str =
    "A dash \u{2013} an em dash \u{2014} and \u{201c}a quote \u{2013} with a dash, and \u{201c}one inside\u{201d}\u{201d}.";

/// The second line of the text output of [`DOCUMENT`], unchanged.
const SECOND_LINE: &str = "Inside mathematics \\(a--b\\) and verbatim c--d nothing changes.";

/// The golden file `name` under `tests/goldens/quickjs/`.
fn golden(name: &str) -> String {
    format!("{}/tests/goldens/quickjs/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The fixture's package `_`, its main module's text changed by `edit`,
/// loaded over a host with `settings`.
fn smartdash(settings: JsSettings, edit: impl Fn(&str) -> String) -> PackageEntry {
    let main = edit(SMARTDASH_MAIN);
    let loader =
        js_loader(&JsHost::new(settings), js_extension("smartdash", &[("main.js", main.as_str())], js_manifest("main.js")));
    common::factory_of(&loader, "smartdash").build(&PackageSettings::new()).expect("the package builds")
}

/// The registry of the standard package, with the `blocks` feature's dash
/// ligatures off, and of `packages`.
fn registry_of(packages: impl IntoIterator<Item = PackageEntry>) -> Arc<FlmRegistry> {
    let mut config = StdcoreConfig::new();
    config.blocks = BlocksConfig { ligature_unicode_dashes: false, ..BlocksConfig::default() };
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(config));
    builder.use_package(flm_core::math_passthrough::package());
    for package in packages {
        builder.use_package(package);
    }
    builder.build(&RegistryInputs::new()).expect("the registry builds")
}

/// The registry of the fixture over a host with `settings`, its main module
/// changed by `edit`.
fn registry_with(settings: JsSettings, edit: impl Fn(&str) -> String) -> Arc<FlmRegistry> {
    registry_of([smartdash(settings, edit)])
}

/// The registry of the fixture as it is.
fn registry() -> Arc<FlmRegistry> {
    registry_with(JsSettings::default(), str::to_string)
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

/// The request of the workflow `workflow` with the rewrite's table demanded
/// beside it, as the fixture's workflow `smartdash` (`extends: text`,
/// `demands: [smartdash.dashed]`) asks a program to build it.
fn request(registry: &Arc<FlmRegistry>, workflow: &str) -> PlanRequest {
    workflow_request(registry, workflow, &Overrides::none()).expect("the workflow").demand(DASHED)
}

/// The text output of `source`, and the diagnostics.
fn text_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = run(registry, &request(registry, text::WORKFLOW), source);
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    (out, common::diagnostics(&run))
}

/// The body of the HTML page of `source`, and the diagnostics.
fn page_body_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = run(registry, &request(registry, html::WORKFLOW), source);
    let page = run
        .table(html::DOCUMENT)
        .expect("declared")
        .and_then(|set| set.entry().and_then(|entry| entry.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned())))
        .unwrap_or_default();
    let body = page.find("<body").map(|at| page[at..].to_string()).unwrap_or_default();
    (body, common::diagnostics(&run))
}

/// Checks the text output and the page's body of [`DOCUMENT`] through
/// `registry` against the goldens.
fn check_goldens(registry: &Arc<FlmRegistry>) {
    let (text, diagnostics) = text_of(registry, DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text.lines().collect::<Vec<_>>(), [FIRST_LINE, "", SECOND_LINE], "{text}");
    assert_text_golden(golden("smartdash.txt"), &text);
    let (body, diagnostics) = page_body_of(registry, DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(body.contains("smartdash-quote"), "{body}");
    assert_text_golden(golden("smartdash.page-body.html"), &body);
}

// --- the fixture -------------------------------------------------------------------------------

#[test]
fn the_fixture_s_registry_builds_without_a_diagnostic() {
    let registry = registry();
    let notes: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(notes.is_empty(), "{notes:?}");
    assert!(registry.stage_idx("smartdash.dash").is_some(), "the rewrite stage is registered");
    let rule = registry
        .rules()
        .iter()
        .find(|entry| entry.rule.name.as_str() == "smartdash.quote.lower.default")
        .expect("the rule is registered");
    assert_eq!(rule.rule.docs, "A quotation as a span of kind `smartdash-quote`.");
}

#[test]
fn the_fixture_renders_the_text_and_the_page_against_the_goldens() {
    check_goldens(&registry());
}

/// The inner `\sdquote` stands in the outer one's argument: the outer
/// handler's `restage_subtree` calls the handler again for it, a nested call
/// on the same engine instance, and both are rebuilt.
#[test]
fn a_nested_quote_is_rewritten_by_a_nested_handler_call() {
    let (text, diagnostics) = text_of(&registry(), "\\sdquote{out -- \\sdquote{in -- side} -- out}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text.trim_end(), "\u{201c}out \u{2013} \u{201c}in \u{2013} side\u{201d} \u{2013} out\u{201d}");
}

#[test]
fn mathematics_and_verbatim_are_left_as_they_are() {
    let (text, diagnostics) =
        text_of(&registry(), "x -- y \\(a--b\\) \\verbcode{c--d} \\begin{equation}e---f\\end{equation} z --- w");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(text.starts_with("x \u{2013} y \\(a--b\\) c--d"), "{text}");
    assert!(text.contains("e---f"), "{text}");
    assert!(text.trim_end().ends_with("z \u{2014} w"), "{text}");
}

/// Without the rewrite's table demanded, the stage does not run and the
/// dashes stay (the ligatures are off).
#[test]
fn without_the_table_demanded_the_dashes_stay() {
    let registry = registry();
    let run = run(&registry, &workflow_request(&registry, text::WORKFLOW, &Overrides::none()).expect("the workflow"), "a -- b");
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    assert_eq!(out.trim_end(), "a -- b");
}

/// The handler throws at one characters node: the Error
/// `flm.ext.handler-failed` at that node, the node kept with its dashes,
/// the rest rewritten.
#[test]
fn a_failing_handler_keeps_its_node_and_reports_at_it() {
    let registry = registry_with(JsSettings::default(), |main| {
        main.replace(
            "if (!text.includes(\"--\") || opaque(node)) return Restage.Descend;",
            "if (!text.includes(\"--\") || opaque(node)) return Restage.Descend;\n        if (text.includes(\"boom\")) throw new Error(\"no dashes here\");",
        )
    });
    let source = "a -- b \\sdquote{boom -- x} c -- d";
    let run = run(&registry, &request(&registry, text::WORKFLOW), source);
    let found: Vec<String> = run
        .diagnostics()
        .iter()
        .map(|d| format!("{} {} {}", d.identifier(), d.span().range().start, d.message()))
        .collect();
    let at = source.find("boom").expect("the node");
    assert_eq!(found, [format!("flm.ext.handler-failed {at} the rewrite stage `smartdash.dash` failed: no dashes here")]);
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    assert_eq!(out.trim_end(), "a \u{2013} b \u{201c}boom -- x\u{201d} c \u{2013} d");
}

/// In the strict mode every outermost handler call runs on a fresh engine
/// instance (rehearsing the feature each time): the output is the same.
#[test]
fn the_strict_mode_gives_the_same_output() {
    check_goldens(&registry_with(JsSettings { strict: true, ..JsSettings::default() }, str::to_string));
}

// --- the prelude's names and classes -----------------------------------------------------------

/// The JSON a test script's `main.names()` answers.
fn names_of(main: &str) -> Value {
    let host = JsHost::new(JsSettings::default());
    let package = TestPackage::new(&host, &[("main.js", main)]).expect("the test package");
    let answer = eval_for_test(&host, &package, "return main.names();").expect("the script runs");
    match serde_json::from_str(&answer).expect("JSON") {
        Value::String(text) => serde_json::from_str(&text).expect("JSON"),
        other => other,
    }
}

#[test]
fn the_prelude_s_names_of_the_new_modules_are_the_library_s() {
    let names = names_of(
        r#"
import * as STRUCTURAL from "flm:lang/structural";
import { lowered_table_name } from "flm:stdcore/backends/lowering";
import { media_type_of } from "flm:core/artifact";
import { StandardTable } from "flm:core/registry";
import { Restage } from "flm:techy/transform";
export function names() {
  const structural = {};
  for (const key of ["TEXT", "COMMENT", "CONTAINER", "GROUP", "MATH", "VERBATIM", "OPAQUE_WRAPPER", "PARAGRAPH", "PARBREAK",
                     "PLACEHOLDER", "UNKNOWN_MACRO", "UNKNOWN_ENVIRONMENT", "NAMES"])
    structural[key] = STRUCTURAL[key];
  return JSON.stringify({
    structural,
    lowered: lowered_table_name("typst"),
    media: [media_type_of("css"), media_type_of("typ")],
    tables: StandardTable,
    restage: [Restage.Descend, Restage.Emit([])],
  });
}
"#,
    );
    use flm_core::lang::structural as s;
    assert_eq!(
        names["structural"],
        json!({
            "TEXT": s::TEXT, "COMMENT": s::COMMENT, "CONTAINER": s::CONTAINER, "GROUP": s::GROUP, "MATH": s::MATH,
            "VERBATIM": s::VERBATIM, "OPAQUE_WRAPPER": s::OPAQUE_WRAPPER, "PARAGRAPH": s::PARAGRAPH, "PARBREAK": s::PARBREAK, "PLACEHOLDER": s::PLACEHOLDER,
            "UNKNOWN_MACRO": s::UNKNOWN_MACRO, "UNKNOWN_ENVIRONMENT": s::UNKNOWN_ENVIRONMENT, "NAMES": s::NAMES,
        })
    );
    let backend = flm_core::model::names::BackendName::from("typst");
    assert_eq!(names["lowered"], json!(flm_core::stdcore::backends::lowering::lowered_table_name(&backend).to_string()));
    assert_eq!(names["media"], json!([flm_core::core::artifact::media_type_of("css"), Value::Null]));
    assert_eq!(
        names["tables"],
        json!({ "LoweredTrees": "lowered-trees", "LoweredPieces": "lowered-pieces", "Regions": "regions", "ArtifactSet": "artifact-set" })
    );
    assert_eq!(names["restage"], json!(["descend", { "emit": [] }]));
}

/// The data forms the classes of section 2 write (`toJSON`), in the
/// argument orders the prelude's module headers pin.
#[test]
fn the_prelude_s_classes_write_the_data_forms() {
    let names = names_of(
        r#"
import { Artifact, ArtifactId } from "flm:core/artifact";
import { OutputDef } from "flm:core/registry";
import { RewriteDecl } from "flm:core/stage";
export function names() {
  const id = new ArtifactId("typst", "document");
  return JSON.stringify({
    bytes: Artifact.bytes(id, "application/x-typst", "= Title").with_extension("typ").as_entry(),
    folder: Artifact.folder(new ArtifactId("typst", "assets"), [["check.svg", "<svg/>"]]),
    output: new OutputDef("typst", "typst.document", "typst.document", "application/x-typst", "typ").docs("Typst."),
    rewrite: new RewriteDecl("smartdash.dash", "smartdash.dashed").docs("Dashes."),
  });
}
"#,
    );
    assert_eq!(
        names["bytes"],
        json!({ "id": { "owner": "typst", "name": "document" }, "media_type": "application/x-typst",
                "data": { "bytes": "= Title" }, "entry": true, "extension": "typ" })
    );
    assert_eq!(
        names["folder"],
        json!({ "id": { "owner": "typst", "name": "assets" }, "media_type": "",
                "data": { "files": [["check.svg", "<svg/>"]] }, "entry": false, "extension": null })
    );
    assert_eq!(
        names["output"],
        json!({ "name": "typst", "capability": "typst.document", "table": "typst.document",
                "media_type": "application/x-typst", "extension": "typ", "made_from": [], "docs": "Typst." })
    );
    assert_eq!(
        names["rewrite"],
        json!({ "name": "smartdash.dash", "publishes": "smartdash.dashed", "refines": null, "default": false, "docs": "Dashes." })
    );
}

/// A backend declared from JavaScript: `new BackendDef(name, native,
/// lowering, emit, final_stage)` with the library's lowering stage
/// (`new LoweringStage("bk")`) and two stage objects, which the constructor
/// records and lends as `b.stage` does; an output over the artifact set its
/// final stage writes with `Artifact.bytes`. The plan demanding the output's
/// table runs the three stages (on fresh engine instances, after the
/// rehearsal) and the entry artifact holds the emitted text.
#[test]
fn a_backend_and_an_output_declared_from_javascript_write_the_artifact() {
    const MAIN: &str = r#"
import { Effect, Needs, Scope, StageDecl } from "flm:core/stage";
import { BackendDef, OutputDef, StandardTable } from "flm:core/registry";
import { LoweringStage, lowered_table_name } from "flm:stdcore/backends/lowering";
import { Artifact, ArtifactId } from "flm:core/artifact";
import { ALL } from "flm:stdcore/backends/pres";
import { COMMENT, CONTAINER, GROUP, OPAQUE_WRAPPER, PARBREAK, TEXT } from "flm:lang/structural";

// The lowered trees' table, as `lowered_table_name("bk")` answers it in
// `register` (a function of the extension API is called from a handler,
// never at a module's top level).
const LOWERED = "flm.lowered.bk";

const emit = {
  decl() { return new StageDecl("bk.emit", Effect.Compute, Scope.Document).writes("bk.output"); },
  needs() { return new Needs().reads(LOWERED); },
  run(cx, out) {
    const lowered = cx.get(LOWERED).lowered;
    let text = "";
    for (const [, root] of lowered.trees())
      for (const node of root.descendants()) if (node.is_chars()) text += node.chars();
    out.table("bk.output").push({ text });
  },
};

const final_stage = {
  decl() { return new StageDecl("bk.final", Effect.Compute, Scope.Document).writes("bk.document"); },
  needs() { return new Needs().reads("bk.output"); },
  run(cx, out) {
    const text = cx.get("bk.output").ext.items().map(item => item.text).join("");
    out.table("bk.document").push(Artifact.bytes(new ArtifactId("bk", "document"), "text/plain", text).with_extension("txt").as_entry());
  },
};

// The native set: the presentation vocabulary, the six structural elements
// plain text writes, and the one element of the standard package with no
// rule for a backend of an extension (ruling S12), which this emitter skips.
export default function (ext) {
  ext.package("_", {
    uses: ["stdcore"],
    features: {
      bk: {
        register(b) {
          b.table_named(lowered_table_name("bk"), StandardTable.LoweredTrees, "The lowered trees.");
          b.table("bk.output", "The emitted text.");
          b.table_named("bk.document", StandardTable.ArtifactSet, "The document.");
          b.backend(new BackendDef("bk", [...ALL, TEXT, GROUP, OPAQUE_WRAPPER, COMMENT, CONTAINER, PARBREAK, "flm.graphics.image"], new LoweringStage("bk"), emit, final_stage)
            .docs("A test backend."));
          b.output(new OutputDef("bk", "bk.document", "bk.document", "text/plain", "txt").docs("A test output."));
        },
      },
    },
  });
}
"#;
    let loader =
        js_loader(&JsHost::new(JsSettings::default()), js_extension("bk", &[("main.js", MAIN)], js_manifest("main.js")));
    let package = common::factory_of(&loader, "bk").build(&PackageSettings::new()).expect("the package builds");
    let registry = registry_of([package]);
    let notes: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(notes.is_empty(), "{notes:?}");
    assert!(registry.stage_idx("flm.bk.lower").is_some());
    assert!(registry.stage_idx("bk.emit").is_some() && registry.stage_idx("bk.final").is_some());
    let output = registry.output("bk").expect("the output `bk`");
    assert_eq!(output.def.entry.extension, "txt");
    let ran = run(&registry, &PlanRequest::new().demand("bk.document"), "Hello \\emph{dear} world.");
    assert!(common::diagnostics(&ran).is_empty(), "{:?}", common::diagnostics(&ran));
    let set =
        ran.table(TableHandle::<ArtifactSet>::new("bk.document")).expect("the table is declared").cloned().unwrap_or_default();
    let entry = set.entry().expect("an entry artifact");
    assert_eq!(entry.media_type, "text/plain");
    assert_eq!(entry.as_bytes(), Some(&b"Hello dear world."[..]));
}

// --- the binding's own refusals ----------------------------------------------------------------

/// A `register` that declares the rewrite stage only when its builder is a
/// real one (not a rehearsal's, whose token is `null`): the engine instance
/// of the worker thread that rewrites rehearses the feature, finds no stage
/// of that name, and the handler fails at every node (message 12).
#[test]
fn a_rewrite_stage_not_declared_by_the_rehearsal_is_the_handler_s_failure() {
    let registry = registry_with(JsSettings::default(), |main| {
        main.replace("  b.rewrite_stage({", "  if (b._token !== null) b.rewrite_stage({")
    });
    let (text, diagnostics) = text_of(&registry, "a -- b");
    assert!(!diagnostics.is_empty());
    for diagnostic in &diagnostics {
        assert_eq!(
            diagnostic,
            "flm.ext.handler-failed: the rewrite stage `smartdash.dash` failed: the rewrite stage `smartdash.dash` must be \
             declared by `register` of the feature `dashes` of the package `_` of the extension `smartdash`"
        );
    }
    assert_eq!(text.trim_end(), "a -- b");
}

/// `__flm.lend_rewrite` called while no handler runs (at a module's top
/// level) answers message 13, which the default export throws here.
#[test]
fn lending_a_rewrite_stage_outside_a_handler_is_refused() {
    let main = "const reply = JSON.parse(globalThis.__flm.lend_rewrite(\"{}\", \"{}\"));\n\
                export default function (ext) { throw new Error(reply.message); }\n";
    let loader =
        js_loader(&JsHost::new(JsSettings::default()), js_extension("lone", &[("main.js", main)], js_manifest("main.js")));
    let problems = common::load_problems(&loader, "lone");
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].ends_with(
            "the extension `lone` must declare a rewrite stage from a feature's `register` or from a handler that \
             `register` declared"
        ),
        "{problems:?}"
    );
}
