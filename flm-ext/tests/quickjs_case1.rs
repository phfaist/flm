//! Case 1 through JavaScript: the extension `wordcount` of
//! `tests/js/wordcount/` rendered through the text and HTML workflows with
//! goldens, its tables, its three settings, and the expansion modules driven
//! from scripts — a
//! hook's recovery and failure, a hook instantiating its definition, nested
//! hooks, the depth and the limit, a block role, a fallback, an override,
//! key-values, a time budget inside a render, and the rehearsal on a worker
//! thread.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;
use std::time::Duration;

use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageEntry};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::registry::PackageFactory;
use flm_core::core::settings::{FeatureSettings, PackageSettings};
use flm_core::dump::golden::{assert_json_golden, assert_text_golden};
use flm_core::dump::json::from_json;
use flm_core::lang::rewrites::userdefs::DEFAULT_RECURSION_LIMIT;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_ext::adapter::table::ExtTable;
use flm_ext::adapter::values::diagnostic_value_to_json;
use flm_core::model::options::OptionValues;
use flm_ext::quickjs::{JsHost, JsSettings};
use serde_json::{json, Value};

use common::{html_of, registry_with, text_of};

// --- helpers -----------------------------------------------------------------------------------

/// Plan B 8.3's document, with `\flmname{}` (plan A, A40).
const DOCUMENT: &str = "\\flmname{} counts words. \\sayhi{World}\n\n\\shout{listen} to this \\todo{check the numbers}.\n\n\\section{Two}\nSecond paragraph, \\shout{quietly}.";

/// The table of word counts.
const WORDS: &str = "wordcount.words";

/// The table of markers.
const TODOS: &str = "wordcount.todos";

/// The golden file `name` under `tests/goldens/quickjs/`.
fn golden(name: &str) -> String {
    format!("{}/tests/goldens/quickjs/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The golden file `name` of WP-4 under `tests/goldens/`.
fn wp4_golden(name: &str) -> String {
    format!("{}/tests/goldens/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The package `_` of the extension `wc` of the main module `main.js` (its
/// text `main`) and the modules `modules`, loaded with `host` and built from
/// empty settings.
fn package_with(host: &JsHost, main: &str, modules: &[(&str, &str)]) -> PackageEntry {
    let mut files = vec![("main.js", main)];
    files.extend_from_slice(modules);
    let loader = common::js_loader(host, common::js_extension("wc", &files, common::js_manifest("main.js")));
    let factory = common::factory_of(&loader, "wc");
    factory.build(&PackageSettings::new()).expect("the package builds")
}

/// The package of `main` alone, loaded with a default host.
fn package(main: &str) -> PackageEntry {
    package_with(&JsHost::new(JsSettings::default()), main, &[])
}

/// Case 1's factory, loaded with `host` from `tests/js/wordcount/`.
fn wordcount_factory(host: &JsHost) -> PackageFactory {
    let source = common::js_extension("wordcount", &common::WORDCOUNT_FILES, common::js_manifest("main.js"));
    common::factory_of(&common::js_loader(host, source), "wordcount")
}

/// Case 1's package, loaded with `host`, no setting written.
fn wordcount_with(host: &JsHost) -> PackageEntry {
    wordcount_factory(host).build(&PackageSettings::new()).expect("the package builds")
}

/// The registry of the standard package and case 1's package.
fn wordcount_registry() -> Arc<FlmRegistry> {
    registry_with([wordcount_with(&JsHost::new(JsSettings::default()))])
}

/// The settings of case 1's feature read from `data` with its declaration.
fn wordcount_settings(factory: &PackageFactory, data: Value) -> FeatureSettings {
    let decl = factory.settings("wordcount").expect("the feature declares settings");
    let Value::Object(map) = data else { panic!("not a map") };
    decl.read(&map).expect("the settings read")
}

/// The run of a plan demanding `demands` over a document of one tree per text
/// of `texts`, sequential (see `tests/quickjs_binding.rs`'s helper of the same
/// name: the stages run on the calling thread when it is a marked worker).
fn run_demanding(registry: &Arc<FlmRegistry>, demands: &[&str], texts: &[&str]) -> Run {
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
    run.execute(&RunOptions::sequential()).expect("the run executes");
    run
}

/// The items of the extension's table `name` after `run`.
fn items(run: &Run, name: &str) -> Vec<Value> {
    run.table(TableHandle::<ExtTable>::new(name))
        .expect("the table is declared")
        .map(|table| table.items().to_vec())
        .unwrap_or_default()
}

/// The data of the first diagnostic of `run` whose identifier is `id`, as
/// JSON.
fn data_of(run: &Run, id: &str) -> Value {
    run.diagnostics()
        .iter()
        .find(|d| d.identifier() == id)
        .map(|d| diagnostic_value_to_json(&d.data().serializable_data()))
        .unwrap_or(Value::Null)
}

/// `value` against the JSON golden `name`.
fn assert_json(name: &str, value: &Value) {
    let serial = from_json(&value.to_string()).expect("the value is JSON");
    assert_json_golden(golden(name), &serial);
}

/// `f` run on a thread marked as a worker of `flm-core`, on which a run
/// executes its stages itself.
fn on_worker<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    flm_core::lang::guard::run_with_worker_stack(f).expect("the worker thread starts")
}

/// A main module of the package `wc` with one feature `defs` whose
/// `register(b)` body is `body`, after the imports every test script uses.
fn wc_main(body: &str) -> String {
    format!(
        r##"
import {{ StageDecl, Needs, Effect, Scope }} from "flm:core/stage";
import {{ CallableKind }} from "flm:core/registry";
import {{ ArgDef, ElementDef, FallbackDef, FallbackKind, Mode, SyntaxDef }} from "flm:lang/mint";
import {{ chars_text }} from "flm:lang/text";
import {{ Definition, UserDefinition, DefinitionBindings, Expansion, ExpandPolicy }} from "flm:lang/rewrites/userdefs";
import {{ ElementSpec }} from "flm:model/element";
import {{ BlockRole }} from "flm:model/aspects";
import {{ parse_keyval_drop_annotations }} from "flm:techy/extract";
import {{ element_name_under, expand_in_tree_rule_for }} from "flm:stdcore/features/userdefs";

export default function (ext) {{ ext.package("_", {{ features: {{ defs: {{ register }} }} }}); }}

// Declares `definition` with `hook` (or none) and its lowering rule.
function declare(b, definition, hook) {{
  const ud = new UserDefinition(definition);
  const element = element_name_under("wc", definition);
  b.element(hook ? ud.element_def_with(b.feature(), element, hook) : ud.element_def(b.feature(), element));
  b.lowering_rule(expand_in_tree_rule_for(element, definition));
  return element;
}}

// A hook that instantiates the definition its element was registered with.
const INSTANTIATE = {{
  expand(node, cx, xp) {{
    const variant = cx.definition().variant_for_use(node);
    return Expansion.Emit(xp.instantiate(cx, node, variant.template(), new DefinitionBindings(node, variant)));
  }},
}};

function register(b) {{
{body}
}}
"##
    )
}

// --- case 1 ------------------------------------------------------------------------------------

#[test]
fn case_one_renders_through_the_text_workflow() {
    let (text, diagnostics) = text_of(&wordcount_registry(), DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    for expected in ["FLM counts words. Hello, World!", "LISTEN to this", "Two", "Second paragraph, QUIETLY."] {
        assert!(text.contains(expected), "{expected:?} in {text:?}");
    }
    assert!(!text.contains("\\todo") && !text.contains("check the numbers") && !text.contains("\\shout"), "{text}");
    assert_text_golden(golden("case1.txt"), &text);
    // The hook agrees with WP-4's golden for the same text.
    let (text, diagnostics) = text_of(&wordcount_registry(), "Say \\shout{hey there} and \\shout{you}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_text_golden(wp4_golden("shout.txt"), &text);
}

/// Plan A, A54 and A63: the adapter does not compile a definition that an
/// expansion hook expands and that declares no template, so `\shout` is not
/// refused when the registry is built.
#[test]
fn case_one_s_registry_builds_without_a_diagnostic() {
    let registry = wordcount_registry();
    let diagnostics: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

/// Plan A, A63, and E4's section 7.5: a scripted definition with neither an
/// expansion hook nor a template is reported by the adapter when the
/// registry is built, naming the feature, the package and the extension
/// (`flm.ext.definition-needs-template`); the definition with a hook and no
/// template beside it is not.
#[test]
fn a_definition_with_neither_a_hook_nor_a_template_is_refused_when_the_registry_is_built() {
    let main = wc_main(
        r##"
  declare(b, Definition.macro_("bare", []), null);
  declare(b, Definition.macro_("hooked", []), INSTANTIATE);
"##,
    );
    let registry = registry_with([package(&main)]);
    let diagnostics: Vec<String> =
        registry.build_diagnostics().iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect();
    assert_eq!(
        diagnostics,
        ["flm.ext.definition-needs-template: the definition `\\bare` declared by the feature `defs` of the package `_` of \
          the extension `wc` must have a template or an expansion hook"]
    );
}

#[test]
fn case_one_s_tables_project_as_expected() {
    let registry = wordcount_registry();
    let run = run_demanding(&registry, &[WORDS, TODOS], &[DOCUMENT, "A \\todo{last} one."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let mut words = items(&run, WORDS);
    words.sort_by_key(|item| item["tree"].as_str().map(str::to_string));
    assert_json("case1-words.json", &Value::Array(words));
    assert_json("case1-todos.json", &Value::Array(items(&run, TODOS)));
}

#[test]
fn case_one_renders_through_the_html_workflow() {
    let (html, diagnostics) = html_of(&wordcount_registry(), DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(html.contains("LISTEN") && !html.contains("check the numbers"), "{html}");
    assert_text_golden(golden("case1.html"), &html);
}

// --- expansion from JavaScript ------------------------------------------------------------------

#[test]
fn a_shout_of_a_command_recovers_the_use_site_with_the_script_s_diagnostic() {
    let registry = wordcount_registry();
    let run = common::run(&registry, flm_core::stdcore::backends::text::WORKFLOW, "Say \\shout{\\emph{x}}.");
    assert_eq!(common::identifiers(&run), ["wordcount.shout.not-characters"]);
    assert_eq!(common::diagnostics(&run), ["wordcount.shout.not-characters: the argument of \\shout must be plain characters"]);
    let (text, _) = text_of(&registry, "Say \\shout{\\emph{x}}.");
    assert!(text.contains("\\shout{\\emph{x}}"), "{text}");
}

#[test]
fn a_hook_that_throws_recovers_the_use_site_with_handler_failed() {
    let main = wc_main(
        r##"
  declare(b, Definition.macro_("shout", [ArgDef.mandatory("text")]), {
    expand(node, cx, xp) { throw new TypeError("the hook is not ready"); },
  });
"##,
    );
    let registry = registry_with([package(&main)]);
    let run = common::run(&registry, flm_core::stdcore::backends::text::WORKFLOW, "Say \\shout{hey}.");
    assert_eq!(
        common::diagnostics(&run),
        ["flm.ext.handler-failed: the expansion hook of `wc.macro.shout` failed: the hook is not ready"]
    );
    assert_eq!(data_of(&run, "flm.ext.handler-failed")["data"]["name"], "TypeError");
    let (text, _) = text_of(&registry, "Say \\shout{hey}.");
    assert_eq!(text, "Say \\shout{hey}.\n");
}

#[test]
fn a_hook_instantiates_its_definition_s_own_template_from_javascript() {
    let main = wc_main(
        r##"
  declare(b, Definition.macro_("loud", [ArgDef.mandatory("text")]).content("[#1]"), INSTANTIATE);
"##,
    );
    let (text, diagnostics) = text_of(&registry_with([package(&main)]), "Say \\loud{x}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Say [x].\n");
}

#[test]
fn a_nested_hook_runs_and_an_outer_handle_is_stale_inside_it() {
    // `\loud` instantiates its template, so the pass calls `\shout`'s hook
    // while `\loud`'s `instantiate` runs, inside the outer JavaScript call.
    // `\loud`'s hook keeps its expander in a module variable; `\shout`'s hook
    // uses it and reports what it met.
    let main = wc_main(
        r##"
  declare(b, Definition.macro_("loud", [ArgDef.mandatory("text")]).content("[#1]"), {
    expand(node, cx, xp) { OUTER = xp; return INSTANTIATE.expand(node, cx, xp); },
  });
  declare(b, Definition.macro_("shout", [ArgDef.mandatory("text")]), {
    expand(node, cx, xp) {
      if (OUTER !== null) {
        try { OUTER.depth(); } catch (error) {
          xp.diagnose({ id: "wc.outer", severity: "note", message: `${error.name} ${error.kind}` });
        }
      }
      return Expansion.Emit([cx.stage_chars(chars_text(node.argument_content_nodes_named("text")).toUpperCase())]);
    },
  });
"##,
    );
    let main = format!("let OUTER = null;\n{main}");
    let registry = registry_with([package(&main)]);
    let run = common::run(&registry, flm_core::stdcore::backends::text::WORKFLOW, "Say \\loud{\\shout{x}}.");
    assert_eq!(common::diagnostics(&run), ["wc.outer: FlmError stale_handle"]);
    let text = run.table(flm_core::stdcore::backends::text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone());
    assert_eq!(text.as_deref(), Some("Say [X].\n"));
}

#[test]
fn the_depth_and_the_limit_are_those_of_the_pass_from_javascript() {
    // As `tests/expand.rs`'s test of the same name: `\outer{…}` instantiates
    // `[\inner{#1}]`, so `\inner`'s hook runs one level deeper.
    let main = wc_main(
        r##"
  const seen = (name, xp) => xp.diagnose({ id: "wc.seen", severity: "note", message: `${name} ${xp.depth()} ${xp.recursion_limit()}` });
  declare(b, Definition.macro_("inner", [ArgDef.mandatory("text")]).content("#1"), {
    expand(node, cx, xp) {
      seen("inner", xp);
      return Expansion.Emit([cx.stage_chars((chars_text(node.argument_content_nodes_named("text")) ?? "").toUpperCase())]);
    },
  });
  declare(b, Definition.macro_("outer", [ArgDef.mandatory("text")]).content("[\\inner{#1}]"), {
    expand(node, cx, xp) { seen("outer", xp); return INSTANTIATE.expand(node, cx, xp); },
  });
"##,
    );
    let registry = registry_with([package(&main)]);
    let run = common::run(&registry, flm_core::stdcore::backends::text::WORKFLOW, "A \\inner{b} and \\outer{c}.");
    let text = run.table(flm_core::stdcore::backends::text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone());
    assert_eq!(text.as_deref(), Some("A B and [C].\n"));
    let limit = DEFAULT_RECURSION_LIMIT;
    let mut seen: Vec<String> = common::diagnostics(&run);
    seen.sort();
    let mut expected =
        vec![format!("wc.seen: inner 0 {limit}"), format!("wc.seen: outer 0 {limit}"), format!("wc.seen: inner 1 {limit}")];
    expected.sort();
    assert_eq!(seen, expected);
}

#[test]
fn a_helper_made_declaration_takes_docs_and_modes_from_javascript() {
    // A declaration a helper made holds an object of Rust: its accessors
    // answer `null`, and `docs` and `modes` answer the same instance. The
    // modes reach the declaration: `\hi` is known in mathematics alone.
    let main = wc_main(
        r##"
  const definition = Definition.macro_("hi", []).content("Hello");
  const def = new UserDefinition(definition).element_def(b.feature(), element_name_under("wc", definition));
  if (!(def instanceof ElementDef) || def.owner() !== null || def.element() !== null) throw new Error("not a helper's declaration");
  if (def.docs("a greeting").modes([Mode.Math]) !== def) throw new Error("not the same instance");
  b.element(def);
  b.lowering_rule(expand_in_tree_rule_for(element_name_under("wc", definition), definition));
  const data = new ElementDef(b.feature(), ["x"], SyntaxDef.Macro([]), new ElementSpec("wc.x"));
  if (data.owner() !== "defs" || data.element().name !== "wc.x") throw new Error("not a data declaration");
"##,
    );
    let (text, diagnostics) = text_of(&registry_with([package(&main)]), "Say \\hi{} and \\(\\hi{}\\).");
    assert_eq!(diagnostics, ["core.specs.unresolvable-command: cannot resolve command ‘\\hi’"]);
    assert_eq!(text, "Say \\hi and \\(Hello{}\\).\n");
}

#[test]
fn a_definition_declared_from_javascript_with_a_block_role_shapes_the_html() {
    // WP-4's scenario (`a_block_definition_renders_its_body_s_paragraphs`),
    // declared from a script: the same golden.
    let main = wc_main(
        r##"
  declare(b, Definition.environment("note", []).content("\\textbf{Note.} #{body}").block_role(BlockRole.Block));
"##,
    );
    let document = "Before.\n\n\\begin{note}Mind the gap.\n\nAnd the step.\\end{note}\n\nAfter.";
    let (html, diagnostics) = html_of(&registry_with([package(&main)]), document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_text_golden(wp4_golden("block.html"), &html);
}

#[test]
fn a_fallback_and_an_override_declared_from_javascript_take_effect() {
    // The fallback: an unknown macro in text mode becomes the element
    // `wc.unknown`, which a stage of the same script finds (no output lowers
    // the element, so the plan demands the stage's table alone).
    let main = wc_main(
        r##"
  b.fallback(new FallbackDef([Mode.Text], FallbackKind.Macro, new ElementSpec("wc.unknown").docs("an unknown macro")));
  b.table("wc.names", "the element names of each tree");
  b.stage({
    decl: () => new StageDecl("wc.names", Effect.Compute, Scope.PerTree).writes("wc.names"),
    run(cx, out) {
      for (const node of cx.my_tree().descendants()) if (node.is_callable()) out.table("wc.names").push(node.element_name());
    },
  });
"##,
    );
    let registry = registry_with([package(&main)]);
    let run = run_demanding(&registry, &["wc.names"], &["A \\nosuchmacro here."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(items(&run, "wc.names"), [json!("wc.unknown")]);
    // The override: `\emph` replaced by a definition of the script.
    let main = wc_main(
        r##"
  b.override_callable(CallableKind.Macro, "emph");
  declare(b, Definition.macro_("emph", [ArgDef.mandatory("text")]).content("[#1]").overriding());
"##,
    );
    let registry = registry_with([package(&main)]);
    let (text, diagnostics) = text_of(&registry, "An \\emph{x} here.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "An [x] here.\n");
}

#[test]
fn key_values_are_read_from_javascript() {
    let main = wc_main(
        r##"
  b.table("wc.options", "the options of each section");
  b.stage({
    decl: () => new StageDecl("wc.options", Effect.Compute, Scope.PerTree).writes("wc.options"),
    run(cx, out) {
      for (const node of cx.my_tree().descendants()) {
        if (!node.is_callable() || node.name() !== "section") continue;
        const list = parse_keyval_drop_annotations(node.argument_content_nodes_named("options"));
        const toc = list.get("toc");
        out.table("wc.options").push({
          len: list.len(),
          empty: list.is_empty(),
          keys: list.iter().map(entry => entry.key()),
          numbered: chars_text(list.keyval(0).value_content()),
          toc: chars_text(toc.value_content()),
          bare: list.get("bare").value(),
          missing: list.get("nothing"),
        });
      }
    },
  });
"##,
    );
    let registry = registry_with([package(&main)]);
    let run = run_demanding(&registry, &["wc.options"], &["\\section[numbered=false, toc={a,b}, toc=c, bare]{Title}"]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(
        items(&run, "wc.options"),
        [json!({
            "len": 4, "empty": false, "keys": ["numbered", "toc", "toc", "bare"],
            "numbered": "false", "toc": "c", "bare": null, "missing": null,
        })]
    );
}

// --- limits and threads --------------------------------------------------------------------------

#[test]
fn a_handler_over_its_time_budget_inside_a_render_is_an_error_at_the_use_site_and_the_render_completes() {
    let main = wc_main(
        r##"
  declare(b, Definition.macro_("spin", []), { expand(node, cx, xp) { for (;;) {} } });
  declare(b, Definition.macro_("shout", [ArgDef.mandatory("text")]), {
    expand(node, cx, xp) {
      return Expansion.Emit([cx.stage_chars(chars_text(node.argument_content_nodes_named("text")).toUpperCase())]);
    },
  });
"##,
    );
    let host = JsHost::new(JsSettings { time_budget: Some(Duration::from_millis(200)), ..JsSettings::default() });
    let registry = registry_with([package_with(&host, &main, &[])]);
    let run = common::run(&registry, flm_core::stdcore::backends::text::WORKFLOW, "A \\spin{} then \\shout{x}.");
    assert_eq!(
        common::diagnostics(&run),
        [
            "flm.ext.handler-failed: the expansion hook of `wc.macro.spin` failed: the expansion hook of the element \
             `wc.macro.spin` of the feature `defs` of the package `_` of the extension `wc` must finish within its time \
             budget of 200 ms"
        ]
    );
    assert_eq!(data_of(&run, "flm.ext.handler-failed")["data"]["limit"], "time");
    let text = run.table(flm_core::stdcore::backends::text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone());
    assert_eq!(text.as_deref(), Some("A \\spin then X.\n"));
}

#[test]
fn the_rehearsal_declares_the_same_objects() {
    let texts = [DOCUMENT, "A \\todo{last} one."];
    let tables = |run: &Run| {
        let mut words = items(run, WORDS);
        words.sort_by_key(|item| item["tree"].as_str().map(str::to_string));
        (words, items(run, TODOS))
    };
    // Built and run on one marked worker thread: the stages run on the engine
    // `register` ran on.
    let host = JsHost::new(JsSettings::default());
    let same = on_worker(|| {
        let registry = registry_with([wordcount_with(&host)]);
        tables(&run_demanding(&registry, &[WORDS, TODOS], &texts))
    });
    // Built on this thread, run on another marked worker thread: the
    // feature is rehearsed on that thread's engine first.
    let registry = wordcount_registry();
    let shared = Arc::clone(&registry);
    let other = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || on_worker(|| tables(&run_demanding(&shared, &[WORDS, TODOS], &texts))))
        .expect("the thread starts")
        .join()
        .expect("the thread runs");
    assert_eq!(other, same);
    assert_eq!(same.0.len(), 2, "{same:?}");
    assert_eq!(same.1, [json!({ "tree": "tree#0", "text": "check the numbers" }), json!({ "tree": "tree#1", "text": "last" })]);
}

// --- the settings ------------------------------------------------------------------------------

#[test]
fn register_receives_the_configuration_and_a_hook_on_a_worker_sees_the_same() {
    let host = JsHost::new(JsSettings::default());
    let factory = wordcount_factory(&host);
    let mut settings = PackageSettings::new();
    settings.insert(wordcount_settings(&factory, json!({ "shout_suffix": "!" })));
    let registry = registry_with([factory.build(&settings).expect("the package builds")]);
    // Built on this thread; the text workflow runs the hook on a run's own
    // worker thread, whose engine instance rehearses `register` with the
    // same configuration.
    let (text, diagnostics) = text_of(&registry, "Say \\shout{hey}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Say HEY!.\n");
    // Another build of the same factory with another configuration, run on
    // a marked worker thread that already ran the first build's hook.
    let mut other = PackageSettings::new();
    other.insert(wordcount_settings(&factory, json!({ "shout_suffix": "?" })));
    let second = registry_with([factory.build(&other).expect("the package builds")]);
    let texts = on_worker(|| {
        let first = common::text_of(&registry, "\\shout{a}").0;
        let then = common::text_of(&second, "\\shout{a}").0;
        let again = common::text_of(&registry, "\\shout{a}").0;
        (first, then, again)
    });
    assert_eq!(texts, ("A!\n".to_string(), "A?\n".to_string(), "A!\n".to_string()));
}

#[test]
fn the_count_stage_reads_both_options() {
    let host = JsHost::new(JsSettings::default());
    let factory = wordcount_factory(&host);
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
    let render = wordcount_settings(&factory, json!({ "min_length": 5 })).render_options().clone();
    let document = wordcount_settings(&factory, json!({ "report_counts": true })).document_options().clone();
    let run_with = |document: OptionValues, render: OptionValues| {
        let source = Arc::new(FlmSource::new("One two three seven eleven.").with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .expect("the tree parses");
        let mut content = DocContent::new();
        content.place(Arc::new(fragment));
        let request = PlanRequest::new().demand(WORDS).render_options(render);
        let plan = Arc::new(Plan::build(&registry, &request).expect("the plan builds"));
        let mut run = Run::new(Arc::new(Doc::with_options(content, document)), plan, Externals::none()).expect("a run");
        run.execute(&RunOptions::sequential()).expect("the run executes");
        run
    };
    // Nothing written: every word counts, and no note.
    let run = run_with(OptionValues::new(), OptionValues::new());
    assert_eq!(items(&run, WORDS), [json!({ "tree": "tree#0", "words": 5 })]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    // Both written: the words of five characters or more, and the note.
    let run = run_with(document, render);
    assert_eq!(items(&run, WORDS), [json!({ "tree": "tree#0", "words": 3 })]);
    assert_eq!(
        common::diagnostics(&run),
        ["wordcount.count: the tree `tree#0` holds 3 words of at least 5 characters"]
    );
}

/// E4's section 7.1: the count stage's capability joins `flm.validated`, so
/// that the count runs with every standard workflow, and its Note appears
/// under the plain text and HTML workflows, which demand no table of the
/// extension.
#[test]
fn the_count_s_note_appears_under_the_plain_workflows() {
    let host = JsHost::new(JsSettings::default());
    let factory = wordcount_factory(&host);
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
    let document = wordcount_settings(&factory, json!({ "report_counts": true })).document_options().clone();
    for workflow in [flm_core::stdcore::backends::text::WORKFLOW, flm_core::stdcore::backends::html::WORKFLOW] {
        let source = Arc::new(FlmSource::new("One two three seven eleven.").with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .expect("the tree parses");
        let mut content = DocContent::new();
        content.place(Arc::new(fragment));
        let plan = Arc::new(
            Plan::for_workflow(&registry, workflow, &flm_core::core::plan::Overrides::none()).expect("the plan builds"),
        );
        let mut run =
            Run::new(Arc::new(Doc::with_options(content, document.clone())), plan, Externals::none()).expect("a run");
        run.execute(&RunOptions::sequential()).expect("the run executes");
        assert_eq!(
            common::diagnostics(&run),
            ["wordcount.count: the tree `tree#0` holds 5 words of at least 1 characters"],
            "{workflow}"
        );
    }
}

// --- the module page ----------------------------------------------------------------------------

#[test]
fn the_module_page_states_the_mapping_rules() {
    let page = include_str!("../src/quickjs/mod.rs");
    let headings = [
        "//! 1. **Module paths.**",
        "//! 2. **A struct or enum of plain data**",
        "//! 3. **An enum of unit variants**",
        "//! 4. **A reference to an object with methods**",
        "//! 5. **A method generic over a type**",
        "//! 6. **A free function**",
        "//! 7. **A trait an extension implements**",
        "//! 8. **`Result<T, E>`**",
        "//! 9. **`Option<T>`**",
        "//! 10. **Values are JSON.**",
        "//! 11. **A diagnostic**",
        "//! 12. **A `const` name**",
    ];
    for heading in headings {
        assert!(page.contains(heading), "the module page lacks {heading:?}");
    }
    let sections = [
        "# What the binding is",
        "# An extension in JavaScript",
        "# The package object and the feature object",
        "# Imports",
        "# Loading and the engine instances",
        "# The mapping rules",
        "# The API modules",
        "# The sandbox",
        "# The limits",
        "# Strict mode",
        "# The rehearsal",
        "# Failures",
        "# The digest",
    ];
    let mut last = 0;
    for section in sections {
        let at = page.find(&format!("//! {section}\n")).unwrap_or_else(|| panic!("the module page lacks the section {section:?}"));
        assert!(at > last, "the section {section:?} is out of order");
        last = at;
    }
    for section in ["# The API modules", "# The sandbox", "# The limits", "# Strict mode", "# The rehearsal", "# Failures"] {
        assert!(page.contains(&format!("//! {section}")), "the module page lacks the section {section:?}");
    }
}

#[test]
fn the_module_page_s_example_loads_and_runs() {
    let page = include_str!("../src/quickjs/mod.rs");
    // The page's two JavaScript blocks: the main module, then `helpers.js`.
    let mut blocks: Vec<String> = Vec::new();
    let mut current: Option<String> = None;
    for line in page.lines().filter_map(|line| line.strip_prefix("//!")) {
        let line = line.strip_prefix(' ').unwrap_or(line);
        match (&mut current, line) {
            (None, "```js") => current = Some(String::new()),
            (Some(_), "```") => blocks.extend(current.take()),
            (Some(block), line) => {
                block.push_str(line);
                block.push('\n');
            }
            (None, _) => {}
        }
    }
    assert_eq!(blocks.len(), 2, "{blocks:?}");
    assert!(blocks[0].contains("export default function (ext)"), "{}", blocks[0]);
    let host = JsHost::new(JsSettings::default());
    let files = [("main.js", blocks[0].as_str()), ("helpers.js", blocks[1].as_str())];
    let loader = common::js_loader(&host, common::js_extension("wc", &files, common::js_manifest("main.js")));
    let factory = common::factory_of(&loader, "wc");
    let decl = factory.settings("wc").expect("the feature declares settings");
    let read = |data: Value| {
        let Value::Object(map) = data else { panic!("not a map") };
        decl.read(&map).expect("the settings read")
    };
    let mut settings = PackageSettings::new();
    settings.insert(read(json!({ "mark": "!" })));
    let registry = registry_with([factory.build(&settings).expect("the package builds")]);
    let (text, diagnostics) = text_of(&registry, "Say \\shout{hey}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Say HEY!.\n");
    let run = run_demanding(&registry, &["wc.words"], &["One two three."]);
    assert_eq!(items(&run, "wc.words"), [json!({ "tree": "tree#0", "words": 3 })]);
    // The option, set on the render request.
    let render = read(json!({ "min": 4 })).render_options().clone();
    let source = Arc::new(FlmSource::new("One two three.").with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the tree parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let request = PlanRequest::new().demand("wc.words").render_options(render);
    let plan = Arc::new(Plan::build(&registry, &request).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::sequential()).expect("the run executes");
    assert_eq!(items(&run, "wc.words"), [json!({ "tree": "tree#0", "words": 1 })]);
}
