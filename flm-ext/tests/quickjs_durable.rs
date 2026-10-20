//! Durable tokens through JavaScript (plan E4, sections 6.2 and 3.6): a
//! template compiled in a late-initialization hook and found again by a rule
//! on another engine instance without a second compilation; a token kept in
//! module scope across calls, and stale under a second registry built from the
//! same package; `lw.lang()` and `xp.lang()` given to `new TemplateLanguage`;
//! `instantiate` spliced by a rule and `xp.instantiate` in a hook, with
//! `ValueBindings`; and the refusals a script meets (a node that is not a
//! tree's root, a specification that does not compile).
//!
//! Run also under `--features flm-core/parallel`, where the run's stages go
//! to the runner's thread pool.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{FlmRegistry, PackageFactory};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::backends::text;
use flm_ext::quickjs::{JsHost, JsSettings};
use serde_json::Value;

use common::{js_factory, registry_with, text_of};

/// The main module of the extension `wc`: the package `_` with the feature
/// `durable`, whose late-initialization hook compiles the title template
/// `Exercise #{number}` (reporting its token as the warning `wc.late`), and
/// whose elements each test one item, their rules and hook reporting what they
/// saw as the diagnostic `wc.note` (a warning: an object as JSON text, or
/// `kind|message` for an error a script caught):
///
/// - `\wctitle{}`: the title template compiled again in a rule, instantiated
///   with the number `3` and spliced; the note holds the token and whether
///   the late-initialization hook ran on this instance;
/// - `\wckept{}`: a template kept in module scope at the first call and used
///   at every later one, the note `compiled` or `kept`;
/// - `\wchook{}`: a hook compiling `Hook #{number}` against `xp.lang()` and
///   instantiating it through `xp.instantiate` with the number `5`, its note
///   the token of `new TemplateLanguage(xp.lang())`;
/// - `\wclang{}`: a rule noting the token of `new TemplateLanguage(lw.lang())`;
/// - `\wcroot{}`: a rule giving `ValueBindings.with` the node it lowers;
/// - `\wcbad{}`: a rule compiling a template that names an unknown placeholder.
const MAIN: &str = r#"
import { LoweringRule, LoweringDecl, DefaultMark, Lowering } from "flm:core/lowering";
import { ArgDef, ElementDef, SyntaxDef } from "flm:lang/mint";
import { ElementSpec } from "flm:model/element";
import { Definition, UserDefinition, Expansion, TemplateLanguage, TemplateSpec, ValueBindings, CompiledTemplate,
  compile_template_with, instantiate } from "flm:lang/rewrites/userdefs";
import { element_name_under, expand_in_tree_rule_for } from "flm:stdcore/features/userdefs";
import { SPAN } from "flm:stdcore/backends/pres";

// Whether the late-initialization hook ran on this instance.
let LATE = false;
// The template `\wckept` keeps in module scope.
let KEPT = null;

function spec(text) {
  return new TemplateSpec({
    text, mode: "text",
    vocabulary: { arity: 0, extras: [{ name: "number", substitution: false }] },
    definition: "the title of wc", label: "title of wc", origin_label: "the title format of wc",
  });
}

function compile(lang, text) { return compile_template_with(spec(text), lang, new TemplateLanguage(lang)); }

// The tree of `title` with the number `number`, itself a template
// instantiated with no value, whose root node the bindings take.
function title_tree(node, lang, title, number) {
  const root = instantiate(node, compile(lang, number), new ValueBindings());
  return instantiate(node, title, new ValueBindings().with("number", root));
}

function caught(error) { return `${error.kind}|${error.message}`; }

function rule(b, name, lower) {
  const element = `wc.${name}`;
  b.element(new ElementDef(b.feature(), [`wc${name}`], SyntaxDef.Macro([ArgDef.mandatory("content")]), new ElementSpec(element)));
  b.lowering_rule(new LoweringRule(`${element}.lower.default`, element, new LoweringDecl().produces(SPAN), {
    lower(node, cx, lw) {
      const [note, staged] = lower(node, cx, lw);
      lw.diagnose({ id: "wc.note", severity: "warning", message: note });
      return Lowering.Emit(staged);
    },
  }, DefaultMark.Everywhere));
}

function register(b) {
  b.late_init({
    late_init(lang, diags) {
      LATE = true;
      const title = compile(lang, "Exercise #{number}");
      diags.push({ id: "wc.late", severity: "warning", message: JSON.stringify(title) });
    },
  });
  rule(b, "title", (node, cx, lw) => {
    const title = compile(lw.lang(), "Exercise #{number}");
    const note = JSON.stringify({ title, late_here: LATE, is_class: title instanceof CompiledTemplate });
    return [note, lw.splice(cx, title_tree(node, lw.lang(), title, "3"))];
  });
  rule(b, "kept", (node, cx, lw) => {
    let note = "kept";
    if (KEPT === null) { KEPT = compile(lw.lang(), "Kept #{number}"); note = "compiled"; }
    try { return [note, lw.splice(cx, title_tree(node, lw.lang(), KEPT, "4"))]; }
    catch (error) { return [caught(error), []]; }
  });
  rule(b, "lang", (node, cx, lw) => [JSON.stringify(new TemplateLanguage(lw.lang())), []]);
  rule(b, "root", (node, cx, lw) => {
    try { new ValueBindings().with("number", node); return ["accepted", []]; }
    catch (error) { return [caught(error), []]; }
  });
  rule(b, "bad", (node, cx, lw) => {
    try { compile(lw.lang(), "A #{nope}"); return ["compiled", []]; }
    catch (error) { return [JSON.stringify({ name: error.name, kind: error.kind, core_kind: error.data.core_kind, texts: error.data.data }), []]; }
  });
  const hook = new UserDefinition(Definition.macro_("wchook", [ArgDef.mandatory("content")]));
  const element = element_name_under("wc", hook.definition());
  b.element(hook.element_def_with(b.feature(), element, {
    expand(node, cx, xp) {
      const language = new TemplateLanguage(xp.lang());
      xp.diagnose({ id: "wc.note", severity: "warning", message: JSON.stringify(language) });
      const title = compile(xp.lang(), "Hook #{number}");
      const root = instantiate(node, compile(xp.lang(), "5"), new ValueBindings());
      return Expansion.Emit(xp.instantiate(cx, node, title, new ValueBindings().with("number", root)));
    },
  }));
  b.lowering_rule(expand_in_tree_rule_for(element, hook.definition()));
}

export default function (ext) {
  ext.package("_", { uses: ["stdcore"], features: { durable: { register } } });
}
"#;

/// The factory of the extension `wc`, loaded with a default host.
fn factory() -> PackageFactory {
    js_factory(&JsHost::new(JsSettings::default()), MAIN)
}

/// A registry of the standard package and the package `_` built by `factory`.
fn registry_of(factory: &PackageFactory) -> Arc<FlmRegistry> {
    registry_with([factory.build(&PackageSettings::new()).expect("the package builds")])
}

/// The messages of the notes `wc.note` among `diagnostics` (rendered as
/// `id: message`), and the other diagnostics.
fn notes(diagnostics: &[String]) -> (Vec<String>, Vec<String>) {
    let (notes, others): (Vec<&String>, Vec<&String>) = diagnostics.iter().partition(|d| d.starts_with("wc.note: "));
    (notes.iter().map(|d| d["wc.note: ".len()..].to_string()).collect(), others.into_iter().cloned().collect())
}

/// `text` parsed as JSON.
fn json(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|error| panic!("`{text}` is not JSON: {error}"))
}

/// The plain text of `source` and its diagnostics, the run sequential: its
/// stages run on the calling thread when it is a marked worker.
fn text_here(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let source = Arc::new(FlmSource::new(source).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Arc::new(Plan::for_workflow(registry, text::WORKFLOW, &Overrides::none()).expect("a plan"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::sequential()).expect("the run executes");
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    (out, common::diagnostics(&run))
}

#[test]
fn a_template_compiled_in_late_init_is_found_by_a_rule_on_another_instance_without_compiling_again() {
    let registry = registry_of(&factory());
    let late: Vec<String> = registry
        .build_diagnostics()
        .iter()
        .filter(|d| d.identifier() == "wc.late")
        .map(|d| d.message().to_string())
        .collect();
    assert_eq!(late.len(), 1, "{late:?}");
    let late = json(&late[0]);
    // The late-initialization hook built the language (entry 0), then the
    // title template (entry 1).
    assert_eq!(late["@durable"]["kind"], "compiled_template", "{late}");
    assert_eq!(late["@durable"]["index"], 1, "{late}");
    let (text, diagnostics) = text_of(&registry, "A \\wctitle{} and \\wctitle{}.");
    let (notes, others) = notes(&diagnostics);
    assert!(others.is_empty(), "{others:?}");
    assert_eq!(text, "A Exercise 3 and Exercise 3.\n");
    assert_eq!(notes.len(), 2, "{notes:?}");
    for note in &notes {
        let note = json(note);
        // The rule runs on an instance the late-initialization hook did not
        // run on, and its compilation answers the hook's entry: the table
        // compiled the title once.
        assert_eq!(note["late_here"], false, "{note}");
        assert_eq!(note["title"], late, "{note}");
        assert_eq!(note["is_class"], true, "{note}");
    }
}

#[test]
fn a_token_kept_in_module_scope_serves_two_calls_and_is_stale_under_a_second_registry() {
    let factory = factory();
    let first = registry_of(&factory);
    let second = registry_of(&factory);
    // Both runs on this worker thread, so that one engine instance serves
    // both registries.
    let ((text, diagnostics), (stale_text, stale_diagnostics)) = flm_core::lang::guard::run_with_worker_stack(|| {
        (text_here(&first, "A \\wckept{} and \\wckept{}."), text_here(&second, "B \\wckept{}."))
    })
    .expect("the worker thread starts");
    let (notes_first, others) = notes(&diagnostics);
    assert!(others.is_empty(), "{others:?}");
    assert_eq!(text, "A Kept 4 and Kept 4.\n");
    assert_eq!(notes_first, ["compiled", "kept"]);
    let (notes_second, others) = notes(&stale_diagnostics);
    assert!(others.is_empty(), "{others:?}");
    assert_eq!(stale_text, "B .\n");
    assert_eq!(
        notes_second,
        ["stale_durable|the `compiled_template` object 2 must be one the feature `durable` of the package `_` of the extension `wc` made under the registry in use"]
    );
}

#[test]
fn the_language_of_a_rule_and_of_a_hook_answer_one_template_language() {
    let registry = registry_of(&factory());
    let (text, diagnostics) = text_of(&registry, "A \\wclang{}, \\wchook{}.");
    let (notes, others) = notes(&diagnostics);
    assert!(others.is_empty(), "{others:?}");
    assert_eq!(text, "A , Hook 5.\n");
    assert_eq!(notes.len(), 2, "{notes:?}");
    let (rule, hook) = (json(&notes[0]), json(&notes[1]));
    assert_eq!(rule["@durable"]["kind"], "template_language", "{rule}");
    assert_eq!(rule["@durable"]["index"], 0, "{rule}");
    assert_eq!(rule, hook);
}

#[test]
fn a_hook_instantiates_through_the_expander_with_value_bindings() {
    let registry = registry_of(&factory());
    let (text, diagnostics) = text_of(&registry, "A \\wchook{} b.");
    let (_, others) = notes(&diagnostics);
    assert!(others.is_empty(), "{others:?}");
    assert_eq!(text, "A Hook 5 b.\n");
}

#[test]
fn value_bindings_refuse_a_node_that_is_not_a_tree_s_root() {
    let registry = registry_of(&factory());
    let (_, diagnostics) = text_of(&registry, "A \\wcroot{}.");
    let (notes, others) = notes(&diagnostics);
    assert!(others.is_empty(), "{others:?}");
    assert_eq!(notes, ["argument|`with`: the value of the placeholder `number` must be the root node of a tree"]);
}

#[test]
fn a_specification_that_does_not_compile_is_thrown_as_a_core_error_with_the_diagnostics() {
    let registry = registry_of(&factory());
    let (_, diagnostics) = text_of(&registry, "A \\wcbad{}.");
    let (notes, others) = notes(&diagnostics);
    assert!(others.is_empty(), "{others:?}");
    assert_eq!(notes.len(), 1, "{notes:?}");
    let note = json(&notes[0]);
    assert_eq!(note["name"], "FlmError", "{note}");
    assert_eq!(note["kind"], "core", "{note}");
    assert_eq!(note["core_kind"], "compile_template", "{note}");
    let texts = note["texts"].as_array().expect("a list of texts");
    assert!(!texts.is_empty(), "{note}");
    assert!(texts.iter().any(|text| text.as_str().is_some_and(|text| text.contains("nope"))), "{note}");
}
