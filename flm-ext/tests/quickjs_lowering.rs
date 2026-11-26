//! The lowering façade through JavaScript: the fixture `lowerjs` of
//! `tests/js/lowerjs/` rendered through the HTML page, the text and the
//! recomposed LaTeX with goldens; a second feature declaring the same rule's
//! name; a body's failure; the depth bound of the data a script passes; and
//! the prelude's names against the library's.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;

use flm_core::core::registry::{BuildError, FlmRegistry, PackageEntry};
use flm_core::core::settings::PackageSettings;
use flm_core::dump::golden::assert_text_golden;
use flm_core::stdlib::backends::{html, latex_recomposed};
use flm_core::stdlib::backends::pres::names as pres;
use flm_ext::api::stdlib::features::math;
use flm_ext::quickjs::{eval_for_test, JsHost, JsSettings, TestPackage};
use serde_json::{json, Value};

use common::{assert_only_unchecked_output, js_extension, js_loader, js_manifest, latex_of, registry_with, text_of, try_registry_with};

/// The fixture's files, as `tests/js/lowerjs/` holds them (its manifest is
/// [`js_manifest`]`("main.js")`, the JSON equal to `flmextension.yaml`).
const LOWERJS_MAIN: &str = include_str!("js/lowerjs/main.js");

/// The document: two boxes, one holding an emphasis.
const DOCUMENT: &str = "Before the boxes.\n\n\\ljbox{A \\emph{first} box.}\n\n\\ljbox{A second one.}\n\nAfter them.";

/// The golden file `name` under `tests/goldens/quickjs/`.
fn golden(name: &str) -> String {
    format!("{}/tests/goldens/quickjs/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The packages `entries` of the fixture, its main module's text changed by
/// `edit`, built from empty settings.
fn lowerjs_edited(edit: impl Fn(&str) -> String, entries: &[&str]) -> Vec<PackageEntry> {
    let main = edit(LOWERJS_MAIN);
    let loader = js_loader(
        &JsHost::new(JsSettings::default()),
        js_extension("lowerjs", &[("main.js", main.as_str())], js_manifest("main.js")),
    );
    entries
        .iter()
        .map(|entry| common::factory_of(&loader, entry).build(&PackageSettings::new()).expect("the package builds"))
        .collect()
}

/// The registry of the standard package and the fixture's package `_`, its
/// main module changed by `edit`.
fn registry_edited(edit: impl Fn(&str) -> String) -> Arc<FlmRegistry> {
    registry_with(lowerjs_edited(edit, &["lowerjs"]))
}

/// The registry of the standard package and the fixture's package `_`.
fn registry() -> Arc<FlmRegistry> {
    registry_edited(str::to_string)
}

/// The HTML page of `source`, and the diagnostics.
fn page_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = common::run(registry, html::WORKFLOW, source);
    let page = run
        .table(html::DOCUMENT)
        .expect("declared")
        .and_then(|set| set.entry().and_then(|entry| entry.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned())))
        .unwrap_or_default();
    (page, common::diagnostics(&run))
}

// --- the fixture through the three outputs -----------------------------------------------------

#[test]
fn the_fixture_s_registry_builds_without_a_diagnostic() {
    let registry = registry();
    let notes: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(notes.is_empty(), "{notes:?}");
    let rule = registry
        .rules()
        .iter()
        .find(|entry| entry.rule.name.as_str() == "lowerjs.box.lower.default")
        .expect("the rule is registered");
    assert_eq!(rule.rule.docs, "Lowers a box to a block.");
}

#[test]
fn the_fixture_renders_the_html_page_with_its_stylesheet() {
    let (page, diagnostics) = page_of(&registry(), DOCUMENT);
    assert_only_unchecked_output(&common::workflow_plan(&registry(), html::WORKFLOW), &diagnostics);
    // The stylesheet's one part, the text the stage wrote into the table.
    assert!(page.contains("<style data-flm-def=\"lowerjs.style\">.lowerjs-box{border:1px solid}</style>"), "{page}");
    // The page's body, without the standard stylesheets of its head.
    let body = &page[page.find("<body").expect("a body")..];
    assert_text_golden(golden("lowerjs.page-body.html"), body);
}

#[test]
fn the_fixture_renders_the_text() {
    let (text, diagnostics) = text_of(&registry(), DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(text.contains("A second one. (box 2"), "{text}");
    assert_text_golden(golden("lowerjs.txt"), &text);
}

#[test]
fn the_fixture_renders_the_recomposed_latex() {
    let (latex, diagnostics) = latex_of(&registry(), DOCUMENT);
    assert_only_unchecked_output(&common::workflow_plan(&registry(), latex_recomposed::WORKFLOW), &diagnostics);
    assert_text_golden(golden("lowerjs.tex"), &latex);
}

// --- the failures ------------------------------------------------------------------------------

#[test]
fn a_second_feature_declaring_the_same_rule_name_fails_the_build() {
    let error = match try_registry_with(lowerjs_edited(str::to_string, &["lowerjs", "lowerjs#dup"])) {
        Ok(_) => panic!("the registry builds"),
        Err(error) => error,
    };
    assert!(
        error.errors.iter().any(|error| matches!(
            error,
            BuildError::Duplicate { kind: "lowering rule", name, .. } if name.as_str() == "lowerjs.box.lower.default"
        )),
        "{error:?}"
    );
}

#[test]
fn an_unchecked_output_the_script_declares_is_kept_by_the_plan() {
    let registry = registry_edited(|main| {
        main.replace(".selection_requires(STYLE),", ".selection_requires(STYLE).unchecked_output(\"html\", \"each box's text\", \"the reader's browser\"),")
    });
    assert_eq!(
        common::unchecked_outputs_of(&registry, html::WORKFLOW, "lowerjs.box.lower.default"),
        [("html".to_string(), "each box's text".to_string(), "the reader's browser".to_string())],
    );
}

#[test]
fn a_failing_body_is_the_handler_failure_at_the_node_and_the_rest_lowers() {
    let registry = registry_edited(|main| {
        main.replace("const content = lw.lower_arg_named(cx, node, \"text\");", "throw new Error(\"no box today\");")
    });
    let (text, diagnostics) = text_of(&registry, DOCUMENT);
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    for diagnostic in &diagnostics {
        assert!(diagnostic.starts_with("flm.ext.handler-failed: "), "{diagnostic}");
        assert!(diagnostic.ends_with("no box today"), "{diagnostic}");
    }
    assert!(text.contains("Before the boxes.") && text.contains("After them."), "{text}");
    assert!(text.contains("[unsupported: lowerjs.box]"), "{text}");
}

#[test]
fn a_tree_deeper_than_the_bound_is_the_body_s_failure() {
    // 70 levels of spans around the text leaf: 141 levels of lists and maps.
    let registry = registry_edited(|main| {
        main.replace(
            "return lw.pres_tree(cx, {",
            "let deep = { text: \"x\" };\n  for (let i = 0; i < 70; i++) deep = { element: SPAN, children: [deep] };\n  lw.pres_tree(cx, deep);\n  return lw.pres_tree(cx, {",
        )
    });
    let (_, diagnostics) = text_of(&registry, "\\ljbox{x}");
    assert_eq!(common::ids(&diagnostics), ["flm.ext.handler-failed"], "{diagnostics:?}");
}

#[test]
fn an_answer_deeper_than_the_bound_is_the_body_s_failure() {
    // The body answers `{ emit: <a list nested 200 deep> }`.
    let registry = registry_edited(|main| {
        main.replace(
            "{ lower(node, cx, lw) { return Lowering.Emit([lw.pres(cx, BLOCK, { role: BOX }, [inner(node, cx, lw)])]); } },",
            "{ lower(node, cx, lw) { let deep = []; for (let i = 1; i < 200; i++) deep = [deep]; return { emit: deep }; } },",
        )
    });
    let (_, diagnostics) = text_of(&registry, "\\ljbox{x}");
    assert_eq!(common::ids(&diagnostics), ["flm.ext.handler-failed"], "{diagnostics:?}");
}

// --- the prelude's names -----------------------------------------------------------------------

#[test]
fn the_prelude_s_names_are_the_library_s() {
    let host = JsHost::new(JsSettings::default());
    let main = r#"
import * as MATH from "flm:stdlib/features/math";
import * as PRES from "flm:stdlib/backends/pres";
import { TAG } from "flm:stdlib/backends/html";
export function names() {
  const math = {};
  for (const key of ["MATH", "ENVIRONMENT", "ENVIRONMENTS", "CONTENT", "RENDERED", "KIND", "COUNTER", "ANCHORS", "COUNTERS",
                     "LINES_PACKAGE", "LINEBREAK_COMMAND", "TAG_COMMAND", "NONUMBER_COMMAND", "TEXT", "TEXT_COMMANDS"])
    math[key] = MATH[key];
  return JSON.stringify({ math, pres: PRES.ALL, tag: TAG, slots: [PRES.BODY_SLOT, PRES.LABEL_SLOT, PRES.ITEM_SLOT, PRES.CELL_SLOT] });
}
"#;
    let package = TestPackage::new(&host, &[("main.js", main)]).expect("the test package");
    let answer = eval_for_test(&host, &package, "return main.names();").expect("the script runs");
    let names: Value = serde_json::from_str(&answer).expect("JSON");
    let names: Value = match names {
        Value::String(text) => serde_json::from_str(&text).expect("JSON"),
        other => other,
    };
    assert_eq!(
        names["math"],
        json!({
            "MATH": math::MATH,
            "ENVIRONMENT": math::ENVIRONMENT,
            "ENVIRONMENTS": math::ENVIRONMENTS.iter().map(|(name, numbered)| json!([name, numbered])).collect::<Vec<_>>(),
            "CONTENT": math::CONTENT,
            "RENDERED": math::RENDERED,
            "KIND": math::KIND,
            "COUNTER": math::COUNTER,
            "ANCHORS": math::ANCHORS,
            "COUNTERS": math::COUNTERS,
            "LINES_PACKAGE": math::LINES_PACKAGE,
            "LINEBREAK_COMMAND": math::LINEBREAK_COMMAND,
            "TAG_COMMAND": math::TAG_COMMAND,
            "NONUMBER_COMMAND": math::NONUMBER_COMMAND,
            "TEXT": math::TEXT,
            "TEXT_COMMANDS": math::TEXT_COMMANDS,
        })
    );
    assert_eq!(names["pres"], json!(pres::ALL));
    assert_eq!(names["tag"], json!(html::construct::TAG));
    assert_eq!(names["slots"], json!([pres::BODY_SLOT, pres::LABEL_SLOT, pres::ITEM_SLOT, pres::CELL_SLOT]));
}

/// A box inside a box's argument: the outer body's `lower_arg_named` calls
/// the inner box's body on the same engine instance, a nested call. The box
/// is made inline content here, so that it may stand in an argument, and its
/// rule stages the span alone.
#[test]
fn a_scripted_rule_s_node_inside_another_s_is_lowered_by_a_nested_call() {
    let registry = registry_edited(|main| {
        main.replace(".with(Aspect.BlockRoleAspect(BlockRole.Block))", "")
            .replace("Lowering.Emit([lw.pres(cx, BLOCK, { role: BOX }, [inner(node, cx, lw)])])", "Lowering.Emit([inner(node, cx, lw)])")
    });
    let (text, diagnostics) = text_of(&registry, "\\ljbox{Outer \\ljbox{inner} box.}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Outer inner (box 2, anchors read) box. (box 1, anchors read)\n");
}

#[test]
fn javascript_workflow_preferences_override_native_verbatim_handling() {
    let main = r#"
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import { VERBATIM, SPAN } from "flm:stdlib/backends/pres";
export default ext => ext.package("_", {
  features: { preferences: { register(b) {
    b.peer_element(VERBATIM, "Input");
    b.peer_element(SPAN, "Output");
    b.lowering_rule(new LoweringRule("wc.verbatim", VERBATIM, new LoweringDecl().produces(SPAN), {
      lower(node, cx, lw) {
        return Lowering.Emit([lw.pres(cx, SPAN, {}, [lw.text(cx, "replacement")])]);
      }
    }, DefaultMark.Never));
    b.workflow_lowering("html", {html: {prefer: ["wc.verbatim"]}});
  }}}
});
"#;
    let package = common::js_factory(&JsHost::new(JsSettings::default()), main)
        .build(&PackageSettings::new()).unwrap();
    let registry = registry_with([package]);
    let (html, diagnostics) = common::html_of(&registry, "\\verbcode[python]|original|");
    common::assert_only_unchecked_output(&common::workflow_plan(&registry, html::WORKFLOW), &diagnostics);
    assert!(html.contains("replacement") && !html.contains("original"), "{html}");
}
