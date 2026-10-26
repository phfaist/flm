//! The JSON boundary (plan A, WP-5): every handle kind driven by name through
//! `dispatch::call` with JSON arguments, the stale handles, the wrong kinds and
//! unknown methods, the values that do not cross, and case 1 run through JSON
//! alone with the `JsonDouble`.

mod common;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::{parse_fragment, Flm, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::techy::core::node::NodeTree;
use flm_ext::adapter::dispatch;
use flm_ext::adapter::handles::{module_handle, CallTable, Entry, Handle, HandleKind, MODULE_PATHS};
use flm_ext::adapter::node::NodeFacade;
use flm_ext::adapter::table::ExtTable;
use flm_ext::adapter::values::{handle_from_value, handle_to_value};
use flm_ext::api::lang::mint::ArgDef;
use flm_ext::api::lang::rewrites::userdefs::{Definition, Expansion, ExpandPolicy, UserDefinition};
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::json_double::{case_one, module, Js, JsonDouble, TODOS, WORDS};
use common::{registry_with, text_of, try_registry_with};

// --- helpers -----------------------------------------------------------------------------------

/// The golden file `name` under `tests/goldens/`.
fn golden(name: &str) -> String {
    format!("{}/tests/goldens/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The run of a plan demanding `demands` over a document of one tree per text
/// of `texts`, in order.
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
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The items of the extension's table `name` after `run`.
fn items(run: &Run, name: &str) -> Vec<Value> {
    run.table(TableHandle::<ExtTable>::new(name))
        .expect("the table is declared")
        .map(|table| table.items().to_vec())
        .unwrap_or_default()
}

/// What the handlers of a test observed, by label.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<BTreeMap<String, Value>>>);

impl Log {
    /// Records `value` under `label`.
    fn note(&self, label: impl Into<String>, value: Value) {
        if let Ok(mut log) = self.0.lock() {
            log.insert(label.into(), value);
        }
    }

    /// What was recorded under `label`, `null` when nothing was.
    fn get(&self, label: &str) -> Value {
        self.0.lock().ok().and_then(|log| log.get(label).cloned()).unwrap_or(Value::Null)
    }

    /// Every label recorded.
    fn labels(&self) -> Vec<String> {
        self.0.lock().map(|log| log.keys().cloned().collect()).unwrap_or_default()
    }
}

/// A call's result as the glue answers it: the value, or `{"err":
/// <ExtError::data()>}`.
fn outcome(result: Result<Value, ExtError>) -> Value {
    match result {
        Ok(value) => value,
        Err(error) => json!({ "err": error.data() }),
    }
}

/// `dispatch::call` on the object `target` stands for in `table`.
fn call(table: &mut CallTable<'_>, target: &Value, method: &str, args: Value) -> Result<Value, ExtError> {
    dispatch::call(table, handle_from_value(target)?, method, &args)
}

/// The tree of the fragment `text`, parsed with the standard package.
fn tree_of(text: &str) -> Arc<NodeTree<Flm>> {
    tree_of_with(&registry_with([]), text)
}

/// The tree of the fragment `text`, parsed with `registry`'s language.
fn tree_of_with(registry: &FlmRegistry, text: &str) -> Arc<NodeTree<Flm>> {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment =
        parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default()).expect("a parse");
    Arc::clone(fragment.tree())
}

/// A table of the call `call` holding the root of the fragment `text`, and
/// the root's handle.
fn table_with_root(call: u64, text: &str) -> (CallTable<'static>, Value) {
    let tree = tree_of(text);
    let id = tree.root().id();
    let mut table = CallTable::new(call);
    let root = handle_to_value(table.lend(Entry::Node(Box::new(NodeFacade::shared(tree, id)))));
    (table, root)
}

/// The first node under `root` whose name is `name`.
fn named(table: &mut CallTable<'_>, root: &Value, name: &str) -> Value {
    let descendants = call(table, root, "descendants", json!([])).expect("descendants");
    descendants
        .as_array()
        .into_iter()
        .flatten()
        .find(|node| call(table, node, "name", json!([])).ok() == Some(json!(name)))
        .cloned()
        .expect("the node is there")
}

/// The kind a value written as a handle names.
fn kind_of(value: &Value) -> Option<HandleKind> {
    handle_from_value(value).ok().map(|handle| handle.kind)
}

// --- case 1 ------------------------------------------------------------------------------------

#[test]
fn case_one_runs_through_json_alone() {
    let registry = registry_with([case_one().package()]);
    // The hook, against WP-4's golden.
    let (text, diagnostics) = text_of(&registry, "Say \\shout{hey there} and \\shout{you}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_text_golden(golden("shout.txt"), &text);
    // The recovery of a use site whose argument is not characters.
    let (text, diagnostics) = text_of(&registry, "Say \\shout{\\emph{hey}}.");
    assert_eq!(diagnostics, ["wordcount.shout.not-characters: the argument of \\shout must be plain characters"]);
    assert!(text.contains("\\shout{\\emph{hey}}"), "{text}");
    // The three fixed definitions, in plan B's document.
    let document = "\\flmname{} counts words. \\sayhi{World}\n\n\\shout{listen} to this \\todo{check the numbers}.\n\n\\section{Two}\nSecond paragraph, \\shout{quietly}.";
    let (text, diagnostics) = text_of(&registry, document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    for expected in ["FLM counts words. Hello, World!", "LISTEN to this", "Two", "Second paragraph, QUIETLY."] {
        assert!(text.contains(expected), "{expected:?} in {text:?}");
    }
    assert!(!text.contains("\\todo") && !text.contains("check the numbers") && !text.contains("\\shout"), "{text}");
    // The word count, against WP-3's table.
    let run = run_demanding(&registry, &[WORDS], &["One two \\emph{three}", "Four five"]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let mut found = items(&run, WORDS);
    found.sort_by_key(|item| item["tree"].as_str().map(str::to_string));
    assert_eq!(found, [json!({ "tree": "tree#0", "words": 3 }), json!({ "tree": "tree#1", "words": 2 })]);
    // The markers, against WP-3's table, under the default plan and with the
    // per-plan definitions expanded explicitly.
    let texts = ["A \\todo{check this} B.\n\nC \\todo{and that}.", "D \\todo{last}."];
    let expected = [
        json!({ "tree": "tree#0", "text": "check this" }),
        json!({ "tree": "tree#0", "text": "and that" }),
        json!({ "tree": "tree#1", "text": "last" }),
    ];
    for demands in [&[TODOS][..], &[TODOS, "flm.userdefs.expanded"]] {
        let run = run_demanding(&registry, demands, &texts);
        assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
        assert_eq!(items(&run, TODOS), expected, "{demands:?}");
    }
}

// --- every handle kind, by name: registration ---------------------------------------------------

/// A JSON hook staging `[<the element's value's color>]`.
fn badge_hook(js: &mut Js<'_, '_>, log: &Log) -> Result<Value, ExtError> {
    let log = log.clone();
    js.lend_expand("always", move |js, node, cx, _| {
        // An element declared in the data form has no registered definition.
        log.note("badge definition", json!([js.call(cx, "definition", json!([]))?]));
        let value = js.call(node, "element_value", json!([]))?;
        let color = value["color"].as_str().unwrap_or("").to_string();
        Ok(json!({ "emit": [js.call(cx, "stage_chars", json!([format!("[{color}]")]))?] }))
    })
}

#[test]
fn a_feature_builder_answers_its_methods_by_name() {
    let log = Log::default();
    let seen = log.clone();
    let double = JsonDouble::new("wc").feature("probe", move |js, b| {
        seen.note("feature", js.call(b, "feature", json!([]))?);
        // An element in its data form, with a value and a hook.
        let hook = badge_hook(js, &seen)?;
        let element = json!({
            "owner": "ignored", "names": ["badge"], "syntax": { "macro": { "args": [] } },
            "element": { "name": "wc.badge", "aspects": [{ "BlockRoleAspect": { "inline": { "absorb_before": false, "absorb_after": false } } }, { "Expand": hook }], "value": { "color": "red" } },
        });
        seen.note("element", js.call(b, "element", json!([element]))?);
        let badge = json!({ "kind": "macro", "name": "badge", "args": [], "expand": "always" });
        let rule = js.call(&module("stdcore/features/userdefs"), "expand_in_tree_rule_for", json!(["wc.badge", badge]))?;
        seen.note("lowering_rule", js.call(b, "lowering_rule", json!([rule]))?);
        // An override, with the definition replacing the standard macro.
        seen.note("override_callable", js.call(b, "override_callable", json!(["macro", "emph"]))?);
        let emph = json!({ "kind": "macro", "name": "emph", "args": [{ "code": "{", "name": "text" }], "content": "[#1]", "overrides": true });
        let ud = js.call(&module("lang/rewrites/userdefs"), "UserDefinition::new", json!([emph]))?;
        let owner = js.call(b, "feature", json!([]))?;
        let def = js.call(&ud, "element_def", json!([owner, "wc.macro.emph"]))?;
        js.call(b, "element", json!([def]))?;
        let rule = js.call(&module("stdcore/features/userdefs"), "expand_in_tree_rule_for", json!(["wc.macro.emph", emph]))?;
        js.call(b, "lowering_rule", json!([rule]))?;
        // A table, a capability and a stage providing it.
        seen.note("table", js.call(b, "table", json!(["wc.out", "what the stage wrote"]))?);
        seen.note("capability", js.call(b, "capability", json!(["wc.done", "the stage ran", null]))?);
        let stage = js.lend_stage(
            &json!({ "name": "wc.stage", "effect": "compute", "scope": "document", "provides": ["wc.done"], "writes": ["wc.out"] }),
            &json!({}),
            |js, _, out| {
                let piece = js.call(out, "table", json!(["wc.out"]))?;
                js.call(&piece, "push", json!(["ran"]))?;
                Ok(())
            },
        )?;
        seen.note("stage", js.call(b, "stage", json!([stage]))?);
        // A3: bytes cross as a string.
        seen.note("digest_input", js.call(b, "digest_input", json!(["the code, v1"]))?);
        // Refusals.
        seen.note("no method", outcome(js.call(b, "push", json!([1]))));
        seen.note("bad count", outcome(js.call(b, "table", json!(["wc.x"]))));
        seen.note("bad argument", outcome(js.call(b, "table", json!([1, ""]))));
        Ok(())
    });
    let registry = registry_with([double.package()]);
    assert_eq!(log.get("feature"), json!("probe"));
    for label in ["element", "lowering_rule", "override_callable", "table", "capability", "stage", "digest_input"] {
        assert_eq!(log.get(label), Value::Null, "{label}");
    }
    assert_eq!(log.get("no method")["err"], json!({ "kind": "argument", "method": "push", "detail": "an object of the kind `feature_builder` has no method `push`" }));
    assert_eq!(log.get("bad count")["err"]["detail"], json!("the method takes 2 arguments, not 1"));
    assert_eq!(log.get("bad argument")["err"]["detail"], json!("argument 1 must be a string, not `1`"));
    // What was declared works.
    let (text, diagnostics) = text_of(&registry, "A (\\badge) and \\emph{x}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A ([red]) and [x].\n");
    assert_eq!(log.get("badge definition"), json!([null]));
    // A fallback (whose element no output lowers, so in a registry of its own).
    let double = JsonDouble::new("wc").feature("loose", |js, b| {
        let fallback = json!({ "modes": ["text"], "kind": "macro", "element": { "name": "wc.unknown" } });
        js.call(b, "fallback", json!([fallback]))?;
        Ok(())
    });
    let names: Vec<String> = tree_of_with(&registry_with([double.package()]), "A \\nosuchmacro here.")
        .root()
        .descendants()
        .map(|node| node.ext().element.name.to_string())
        .collect();
    assert!(names.iter().any(|name| name == "wc.unknown"), "{names:?}");
    let run = run_demanding(&registry, &["wc.out"], &["Text."]);
    assert_eq!(items(&run, "wc.out"), [json!("ran")]);
    // The digest input changes the digest.
    let digest = |input: &'static str| {
        let double = JsonDouble::new("wc").feature("probe", move |js, b| {
            js.call(b, "digest_input", json!([input]))?;
            Ok(())
        });
        registry_with([double.package()]).digest()
    };
    assert_eq!(digest("a"), digest("a"));
    assert_ne!(digest("a"), digest("b"));
    // A required service slot nothing fills fails the build.
    let double = JsonDouble::new("wc").feature("probe", |js, b| {
        js.call(b, "require_service", json!(["wc.spell-checker"]))?;
        Ok(())
    });
    let error = try_registry_with([double.package()]).expect_err("an unfilled slot").to_string();
    assert!(error.contains("wc.spell-checker"), "{error}");
}

// --- every handle kind, by name: stages ---------------------------------------------------------

/// The registry of the package `wc` whose feature `probe` declares a per-tree
/// stage writing `wc.first`, a per-tree stage `wc.tree-probe` and a document
/// stage `wc.doc-probe` (reading `wc.first`, the trees of
/// `flm.userdefs.always-expanded` and the labels), each probe noting what it
/// reads into `log`; and the run demanding both probes' tables.
fn stage_scenario(log: &Log) -> Run {
    let tree_log = log.clone();
    let doc_log = log.clone();
    let double = JsonDouble::new("wc").feature("probe", move |js, b| {
        for table in ["wc.first", "wc.tree-out", "wc.doc-out"] {
            js.call(b, "table", json!([table, ""]))?;
        }
        let first = js.lend_stage(
            &json!({ "name": "wc.first", "effect": "compute", "scope": "per-tree", "writes": ["wc.first"] }),
            &json!({}),
            |js, cx, out| {
                let tree = js.call(cx, "tree_id", json!([]))?;
                let piece = js.call(out, "table", json!(["wc.first"]))?;
                js.call(&piece, "push", json!([{ "tree": tree }]))?;
                Ok(())
            },
        )?;
        js.call(b, "stage", json!([first]))?;
        let log = tree_log.clone();
        let tree_probe = js.lend_stage(
            &json!({ "name": "wc.tree-probe", "effect": "compute", "scope": "per-tree", "writes": ["wc.tree-out"] }),
            &json!({}),
            move |js, cx, out| {
                let tree = js.call(cx, "tree_id", json!([]))?;
                let id = tree.as_str().unwrap_or("").to_string();
                let note = |label: &str, value: Value| log.note(format!("{id} {label}"), value);
                note("scope", js.call(cx, "scope", json!([]))?);
                let root = js.call(cx, "my_tree", json!([]))?;
                note("my_tree", json!(kind_of(&root).map(HandleKind::name)));
                note("my_tree span", js.call(&root, "span_content", json!([]))?);
                let by_id = js.call(cx, "tree", json!([id]))?;
                note("tree span", js.call(&by_id, "span_content", json!([]))?);
                let frozen = js.call(cx, "frozen_tree", json!([id]))?;
                note("frozen_tree span", js.call(&frozen, "span_content", json!([]))?);
                note("tree unknown", js.call(cx, "tree", json!(["tree#99"]))?);
                // A4: a tuple crosses as a list.
                let trees = js.call(cx, "trees", json!([]))?;
                note("trees ids", json!(trees.as_array().map(|pairs| pairs.iter().map(|pair| pair[0].clone()).collect::<Vec<_>>())));
                note("trees kinds", json!(trees.as_array().map(|pairs| pairs.iter().map(|pair| kind_of(&pair[1]).map(HandleKind::name)).collect::<Vec<_>>())));
                let walk = js.call(cx, "walk", json!([]))?;
                let mut chars = Vec::new();
                for entry in walk.as_array().into_iter().flatten() {
                    let text = js.call(&entry["node"], "chars", json!([]))?;
                    if !text.is_null() {
                        chars.push(json!([entry["addr"]["tree"], entry["addr"]["node"].is_u64(), text]));
                    }
                }
                note("walk", json!(chars));
                note("get undeclared", outcome(js.call(cx, "get", json!(["flm.labels"]))));
                let piece = js.call(out, "table", json!(["wc.tree-out"]))?;
                note("table", json!(kind_of(&piece).map(HandleKind::name)));
                note("push", js.call(&piece, "push", json!([{ "tree": tree }]))?);
                note("push float", outcome(js.call(&piece, "push", json!([{ "ratio": 0.5 }]))));
                note("push reserved", outcome(js.call(&piece, "push", json!([{ "$x": 1 }]))));
                note("push handle key", outcome(js.call(&piece, "push", json!([{ "@handle": 1, "other": 2 }]))));
                note("push a handle", outcome(js.call(&piece, "push", json!([root]))));
                note("table undeclared", outcome(js.call(out, "table", json!(["wc.first"]))));
                let emph = js.call(&root, "descendants", json!([]))?;
                let at = emph.as_array().and_then(|nodes| nodes.first()).cloned().unwrap_or(Value::Null);
                let diagnostic = json!({ "id": "wc.seen", "severity": "note", "message": "seen", "data": { "n": 1 }, "at": at });
                note("diags", js.call(out, "diags", json!([diagnostic]))?);
                note("diags outside", outcome(js.call(out, "diags", json!([{ "id": "other.x", "severity": "note", "message": "m" }]))));
                note("no method", outcome(js.call(cx, "push", json!([]))));
                Ok(())
            },
        )?;
        js.call(b, "stage", json!([tree_probe]))?;
        let log = doc_log.clone();
        let doc_probe = js.lend_stage(
            &json!({ "name": "wc.doc-probe", "effect": "compute", "scope": "document", "writes": ["wc.doc-out"] }),
            &json!({ "reads": ["wc.first", "flm.userdefs.always-expanded", "flm.labels"] }),
            move |js, cx, _| {
                log.note("doc scope", js.call(cx, "scope", json!([]))?);
                log.note("doc tree_id", js.call(cx, "tree_id", json!([]))?);
                log.note("doc my_tree", js.call(cx, "my_tree", json!([]))?);
                let ext = js.call(cx, "get", json!(["wc.first"]))?;
                log.note("get ext", json!(ext.as_object().map(|map| map.keys().cloned().collect::<Vec<_>>())));
                log.note("ext items", js.call(&ext["ext"], "items", json!([]))?);
                let trees = js.call(cx, "get", json!(["flm.userdefs.always-expanded"]))?;
                log.note("get trees", json!(kind_of(&trees["trees"]).map(HandleKind::name)));
                let pairs = js.call(&trees["trees"], "trees", json!([]))?;
                log.note("trees ids", json!(pairs.as_array().map(|pairs| pairs.iter().map(|pair| pair[0].clone()).collect::<Vec<_>>())));
                let one = js.call(&trees["trees"], "tree", json!(["tree#1"]))?;
                log.note("tree", js.call(&one, "span_content", json!([]))?);
                log.note("tree unknown", js.call(&trees["trees"], "tree", json!(["tree#9"]))?);
                let labels = js.call(cx, "get", json!(["flm.labels"]))?;
                log.note("get projection", json!(labels.get("projection").is_some()));
                log.note("projection", labels["projection"].clone());
                Ok(())
            },
        )?;
        js.call(b, "stage", json!([doc_probe]))?;
        Ok(())
    });
    let registry = registry_with([double.package()]);
    run_demanding(&registry, &["wc.tree-out", "wc.doc-out"], &["One \\emph{two}.", "Three."])
}

#[test]
fn a_stage_context_answers_its_methods_by_name() {
    let log = Log::default();
    let run = stage_scenario(&log);
    assert_eq!(common::identifiers(&run), ["wc.seen", "wc.seen"], "{:?}", common::diagnostics(&run));
    assert_eq!(log.get("tree#0 scope"), json!("per-tree"));
    assert_eq!(log.get("tree#0 my_tree"), json!("node"));
    assert_eq!(log.get("tree#0 my_tree span"), json!("One \\emph{two}."));
    assert_eq!(log.get("tree#0 tree span"), log.get("tree#0 my_tree span"));
    assert_eq!(log.get("tree#1 frozen_tree span"), json!("Three."));
    assert_eq!(log.get("tree#0 tree unknown"), Value::Null);
    assert_eq!(log.get("tree#0 trees ids"), json!(["tree#0", "tree#1"]));
    assert_eq!(log.get("tree#0 trees kinds"), json!(["node", "node"]));
    // The walk covers every tree, each entry an address and a node.
    assert_eq!(
        log.get("tree#1 walk"),
        json!([["tree#0", true, "One "], ["tree#0", true, "two"], ["tree#0", true, "."], ["tree#1", true, "Three."]])
    );
    let undeclared = log.get("tree#0 get undeclared");
    assert_eq!(undeclared["err"]["kind"], json!("core"), "{undeclared}");
    assert_eq!(undeclared["err"]["core_kind"], json!("access"), "{undeclared}");
    assert_eq!(log.get("tree#0 no method")["err"]["detail"], json!("an object of the kind `stage_cx` has no method `push`"));
    assert_eq!(log.get("doc scope"), json!("document"));
    assert_eq!(log.get("doc tree_id"), Value::Null);
    assert_eq!(log.get("doc my_tree"), Value::Null);
    assert_eq!(log.get("get ext"), json!(["ext"]));
    assert_eq!(log.get("get trees"), json!("trees_table"));
    assert_eq!(log.get("get projection"), json!(true));
}

#[test]
fn a_stage_output_and_a_table_piece_answer_their_methods_by_name() {
    let log = Log::default();
    let run = stage_scenario(&log);
    assert_eq!(log.get("tree#0 table"), json!("table_piece"));
    assert_eq!(log.get("tree#0 push"), Value::Null);
    assert_eq!(items(&run, "wc.tree-out"), [json!({ "tree": "tree#0" }), json!({ "tree": "tree#1" })]);
    let undeclared = log.get("tree#0 table undeclared");
    assert_eq!(undeclared["err"]["kind"], json!("core"), "{undeclared}");
    assert_eq!(log.get("tree#0 diags"), Value::Null);
    // The diagnostic stands at the node its `at` names, the tree's first
    // descendant (`One ` and `Three.`), not at the whole tree.
    let spans: Vec<std::ops::Range<usize>> = run.diagnostics().iter().map(|d| d.span().range()).collect();
    assert_eq!(spans, [0..4, 0..6], "{:?}", common::diagnostics(&run));
    let outside = log.get("tree#0 diags outside");
    assert_eq!(outside["err"]["kind"], json!("name_outside_extension"), "{outside}");
}

#[test]
fn a_table_read_and_a_trees_table_answer_their_methods_by_name() {
    let log = Log::default();
    stage_scenario(&log);
    assert_eq!(log.get("ext items"), json!([{ "tree": "tree#0" }, { "tree": "tree#1" }]));
    assert_eq!(log.get("trees ids"), json!(["tree#0", "tree#1"]));
    assert_eq!(log.get("tree"), json!("Three."));
    assert_eq!(log.get("tree unknown"), Value::Null);
}

#[test]
fn a_float_and_a_reserved_key_do_not_cross() {
    let log = Log::default();
    stage_scenario(&log);
    let refused = |label: &str| log.get(&format!("tree#0 {label}"))["err"].clone();
    assert_eq!(refused("push float"), json!({ "kind": "value", "detail": "`push`: a number must be an integer, not `0.5`" }));
    assert_eq!(refused("push reserved"), json!({ "kind": "value", "detail": "`push`: a map key must not begin with `$`, as `$x` does" }));
    assert_eq!(refused("push handle key"), json!({ "kind": "value", "detail": "`push`: the map key `@handle` is reserved" }));
    // A handle is not data: it cannot be pushed into a table.
    assert_eq!(refused("push a handle"), json!({ "kind": "value", "detail": "`push`: the map key `@handle` is reserved" }));
    // A float as an argument is refused before the method runs.
    let (mut table, root) = table_with_root(1, "ab");
    let error = call(&mut table, &root, "child", json!([0.5])).expect_err("a float");
    assert_eq!(error, ExtError::Value { detail: "`child`: a number must be an integer, not `0.5`".to_string() });
    let error = call(&mut table, &root, "argument_is_provided", json!([{ "$x": 1 }])).expect_err("a reserved key");
    assert_eq!(error.kind(), "value");
}

// --- every handle kind, by name: nodes and key-value lists -----------------------------------------

#[test]
fn a_node_answers_its_methods_by_name() {
    let (mut table, root) = table_with_root(2, "\\emph{ab} cd");
    let mut ask = |target: &Value, method: &str, args: Value| outcome(call(&mut table, target, method, args));
    assert_eq!(ask(&root, "kind", json!([])), json!("list"));
    assert_eq!(ask(&root, "is_list", json!([])), json!(true));
    assert_eq!(ask(&root, "child_count", json!([])), json!(2));
    assert_eq!(ask(&root, "parent", json!([])), Value::Null);
    let emph = ask(&root, "child", json!([0]));
    assert_eq!(kind_of(&emph), Some(HandleKind::Node));
    assert_eq!(ask(&root, "child", json!([5])), Value::Null);
    for (method, answer) in [
        ("kind", json!("callable")),
        ("is_chars", json!(false)),
        ("is_group", json!(false)),
        ("is_callable", json!(true)),
        ("is_comment", json!(false)),
        ("is_list", json!(false)),
        ("span", json!({ "start": 0, "end": 9 })),
        ("span_content", json!("\\emph{ab}")),
        ("chars", Value::Null),
        ("name", json!("emph")),
        ("group_delimiters", Value::Null),
        ("element_name", json!("flm.textstyle")),
        ("element_value", Value::Null),
        ("mode", json!("text")),
        ("body", Value::Null),
    ] {
        assert_eq!(ask(&emph, method, json!([])), answer, "{method}");
    }
    let children = ask(&root, "children", json!([]));
    assert_eq!(children.as_array().map(Vec::len), Some(2));
    let descendants = ask(&root, "descendants", json!([]));
    let chars: Vec<Value> = descendants.as_array().into_iter().flatten().map(|node| ask(node, "chars", json!([]))).collect();
    assert_eq!(chars, [Value::Null, Value::Null, json!("ab"), json!(" cd")]);
    // A4: a pair crosses as a list.
    let group = ask(&emph, "child", json!([0]));
    assert_eq!(ask(&group, "group_delimiters", json!([])), json!(["{", "}"]));
    assert_eq!(ask(&group, "is_group", json!([])), json!(true));
    let parent = ask(&group, "parent", json!([]));
    assert_eq!(ask(&parent, "name", json!([])), json!("emph"));
    // The arguments.
    let nodes = ask(&emph, "argument_nodes", json!([0]));
    assert_eq!(nodes.as_array().map(|nodes| nodes.iter().map(|node| ask(node, "kind", json!([]))).collect::<Vec<_>>()), Some(vec![json!("group")]));
    let content = ask(&emph, "argument_content_nodes", json!([0]));
    assert_eq!(ask(&module("lang/text"), "chars_text", json!([content])), json!("ab"));
    assert_eq!(ask(&emph, "argument_nodes", json!([5])), Value::Null);
    let named_nodes = ask(&emph, "argument_nodes_named", json!(["content"]));
    assert_eq!(named_nodes.as_array().map(Vec::len), Some(1));
    let named_content = ask(&emph, "argument_content_nodes_named", json!(["content"]));
    assert_eq!(ask(&module("lang/text"), "chars_text", json!([named_content])), json!("ab"));
    let unknown = ask(&emph, "argument_nodes_named", json!(["nothing"]));
    assert_eq!(unknown["err"]["core_kind"], json!("access"), "{unknown}");
    let unknown = ask(&emph, "argument_is_provided", json!(["nothing"]));
    assert_eq!(unknown["err"]["kind"], json!("core"), "{unknown}");
    // A star argument, a body and a slot, the mode of a formula.
    let (mut table, root) = table_with_root(3, "\\section*{One} \\begin{itemize}\\item x\\end{itemize} \\(y\\)");
    let section = named(&mut table, &root, "section");
    let mut ask = |target: &Value, method: &str, args: Value| outcome(call(&mut table, target, method, args));
    assert_eq!(ask(&section, "argument_is_provided", json!(["star"])), json!(true));
    assert_eq!(ask(&section, "argument_is_provided", json!(["options"])), json!(false));
    let descendants = ask(&root, "descendants", json!([]));
    let mut list_node = Value::Null;
    let mut math = Value::Null;
    for node in descendants.as_array().into_iter().flatten() {
        if ask(node, "name", json!([])) == json!("itemize") {
            list_node = node.clone();
        }
        if ask(node, "chars", json!([])) == json!("y") {
            math = node.clone();
        }
    }
    let body = ask(&list_node, "body", json!([]));
    let first = body.as_array().and_then(|nodes| nodes.first()).cloned().unwrap_or(Value::Null);
    assert_eq!(ask(&first, "name", json!([])), json!("item"));
    let slot = ask(&list_node, "slot_content_nodes_named", json!(["nothing"]));
    assert_eq!(slot["err"]["kind"], json!("core"), "{slot}");
    assert_eq!(ask(&math, "mode", json!([])), json!("math"));
    assert_eq!(ask(&math, "is_chars", json!([])), json!(true));
}

#[test]
fn a_key_value_list_and_its_entries_answer_their_methods_by_name() {
    let (mut table, root) = table_with_root(4, "\\section[numbered=false, toc={a,b}, toc=c, bare]{Title}");
    let section = named(&mut table, &root, "section");
    let mut ask = |target: &Value, method: &str, args: Value| outcome(call(&mut table, target, method, args));
    let options = ask(&section, "argument_content_nodes_named", json!(["options"]));
    let list = ask(&module("techy/extract"), "parse_keyval_drop_annotations", json!([options]));
    assert_eq!(kind_of(&list), Some(HandleKind::KeyVals));
    assert_eq!(ask(&list, "len", json!([])), json!(4));
    assert_eq!(ask(&list, "is_empty", json!([])), json!(false));
    let entries = ask(&list, "iter", json!([]));
    let keys: Vec<Value> = entries.as_array().into_iter().flatten().map(|entry| ask(entry, "key", json!([]))).collect();
    assert_eq!(keys, [json!("numbered"), json!("toc"), json!("toc"), json!("bare")]);
    let toc = ask(&list, "get", json!(["toc"]));
    let value = ask(&toc, "value_content", json!([]));
    assert_eq!(ask(&module("lang/text"), "chars_text", json!([value])), json!("c"));
    let first_toc = ask(&list, "keyval", json!([1]));
    let value = ask(&first_toc, "value", json!([]));
    let kinds: Vec<Value> = value.as_array().into_iter().flatten().map(|node| ask(node, "kind", json!([]))).collect();
    assert_eq!(kinds, [json!("group")]);
    let bare = ask(&list, "get", json!(["bare"]));
    assert_eq!(ask(&bare, "value", json!([])), Value::Null);
    assert_eq!(ask(&list, "get", json!(["nothing"])), Value::Null);
    assert_eq!(ask(&list, "keyval", json!([4])), Value::Null);
    let wrong = ask(&module("techy/extract"), "parse_keyval_drop_annotations", json!([[root, section]]));
    assert_eq!(wrong["err"]["kind"], json!("argument"), "{wrong}");
}

// --- every handle kind, by name: the free functions and the definitions ----------------------------

#[test]
fn the_free_functions_answer_by_name_on_their_module() {
    let (mut table, root) = table_with_root(5, "\\emph{a{b}c} x\\emph{y} \\footnote{a\n\nb}");
    let footnote = named(&mut table, &root, "footnote");
    let emph = named(&mut table, &root, "emph");
    let mut ask = |target: &Value, method: &str, args: Value| outcome(call(&mut table, target, method, args));
    let content = ask(&emph, "argument_content_nodes", json!([0]));
    let text = module("lang/text");
    assert_eq!(ask(&text, "chars_text", json!([content])), json!("abc"));
    assert_eq!(ask(&text, "read_chars", json!([content])), json!({ "text": "abc", "has_placeholder": false }));
    let children = ask(&root, "children", json!([]));
    assert_eq!(ask(&text, "chars_text", json!([children])), Value::Null);
    let not = ask(&text, "read_chars", json!([children]));
    assert_eq!(not["err"]["kind"], json!("core"), "{not}");
    assert_eq!(not["err"]["core_kind"], json!("not-characters"), "{not}");
    assert!(not["err"]["data"]["node"].is_u64(), "{not}");
    let lowering = module("core/lowering");
    assert_eq!(ask(&lowering, "chars_at_lowering", json!([content])), json!("abc"));
    assert_eq!(ask(&lowering, "chars_at_lowering", json!([children]))["err"]["core_kind"], json!("not-characters"));
    assert_eq!(ask(&lowering, "spelling_at_lowering", json!([content])), Value::Null);
    let blocks = module("lang/rewrites/blocks");
    assert_eq!(ask(&blocks, "argument_holds_blocks", json!([footnote, 1])), json!(true));
    assert_eq!(ask(&blocks, "argument_holds_blocks", json!([root, 0])), json!(false));
    let mint = module("lang/mint");
    let arg = ask(&mint, "arg_def_from_code", json!(["["]));
    assert_eq!((arg["code"].clone(), arg["layout"].clone()), (json!("["), json!("inline")), "{arg}");
    assert_eq!(ask(&mint, "arg_def_from_code", json!(["q"]))["err"]["core_kind"], json!("arg-code"));
    let helpers = module("stdcore/features/userdefs");
    let shout = json!({ "kind": "macro", "name": "shout", "args": [{ "code": "{", "name": "text" }] });
    assert_eq!(ask(&helpers, "element_name_under", json!(["wc", shout])), json!("wc.macro.shout"));
    let rule = ask(&helpers, "expand_in_tree_rule_for", json!(["wc.macro.shout", shout]));
    assert_eq!(kind_of(&rule), Some(HandleKind::LoweringRule));
    // A lowering rule, like every object passed back as an argument, has no method.
    assert_eq!(ask(&rule, "name", json!([]))["err"]["detail"], json!("an object of the kind `lowering_rule` has no method `name`"));
    // Unknown functions and modules.
    assert_eq!(
        ask(&text, "nothing", json!([]))["err"],
        json!({ "kind": "argument", "method": "nothing", "detail": "the module `lang/text` has no function `nothing`" })
    );
    let beyond = handle_to_value(Handle { kind: HandleKind::Module, call: 0, index: 99 });
    assert_eq!(ask(&beyond, "chars_text", json!([[]]))["err"]["kind"], json!("argument"));
    // Every module path has a handle.
    for (index, path) in MODULE_PATHS.iter().enumerate() {
        assert_eq!(module_handle(path).map(|handle| handle.index as usize), Some(index));
    }
}

#[test]
fn a_user_definition_answers_its_methods_by_name() {
    let mut table = CallTable::new(6);
    let userdefs = module("lang/rewrites/userdefs");
    let definition = json!({
        "kind": "macro", "name": "shout", "args": [{ "code": "{", "name": "text" }], "content": "<#1>",
        "modes": "text", "defaults": { "1": "x" }, "block_role": "block", "expand": "always", "docs": "Shouts.",
    });
    let ud = call(&mut table, &userdefs, "UserDefinition::new", json!([definition])).expect("a definition");
    assert_eq!(kind_of(&ud), Some(HandleKind::UserDefinition));
    let written = call(&mut table, &ud, "definition", json!([])).expect("its declaration");
    assert_eq!(
        written,
        json!({
            "kind": "macro", "name": "shout",
            "args": [{ "code": "{", "name": "text", "layout": "inline", "opaque_wrapper": false, "docs": "", "mode": null, "scoped": [], "role": null }],
            "content": "<#1>", "modes": "text", "defaults": { "1": "x" }, "argument_number_offset": 0,
            "block_role": "block", "expand": "always", "overrides": false, "docs": "Shouts.", "extra_placeholders": [],
        })
    );
    let def = call(&mut table, &ud, "element_def", json!(["defs", "wc.macro.shout"])).expect("a declaration");
    assert_eq!(kind_of(&def), Some(HandleKind::ElementDef));
    let hook = handle_to_value(table.lend(Entry::Expand(common::double::expand(ExpandPolicy::PerPlan, |_, _, _| Ok(Expansion::Default)))));
    let with = call(&mut table, &ud, "element_def_with", json!(["defs", "wc.macro.shout", hook])).expect("with a hook");
    assert_eq!(kind_of(&with), Some(HandleKind::ElementDef));
    let wrong = call(&mut table, &ud, "element_def_with", json!(["defs", "wc.macro.shout", def])).expect_err("not a hook");
    assert_eq!(
        wrong,
        ExtError::Argument { method: "element_def_with".to_string(), detail: "an object of the kind `expand` is expected here, not one of the kind `element_def`".to_string() }
    );
    let refused = call(&mut table, &userdefs, "UserDefinition::new", json!([{ "kind": "macro" }])).expect_err("no name");
    assert_eq!(refused.kind(), "value");
}

/// The registry of the package `wc` declaring `\inner` and `\outer` from
/// Rust-made definitions through JSON register calls: `\inner` staged by a
/// JSON hook as `IN`, noting what its call sees; `\outer`, whose template
/// uses `\inner`, by a JSON hook that instantiates the definition's own
/// template, noting what its call sees and keeping its use site's handle,
/// which the inner call then uses.
fn nested(log: &Log) -> Arc<FlmRegistry> {
    let inner = UserDefinition::new(Definition::macro_("inner", Vec::new()).content("in"));
    let outer = UserDefinition::new(Definition::macro_("outer", vec![ArgDef::mandatory("text")]).content("[\\inner{} #1]"));
    let kept: Arc<Mutex<Value>> = Arc::new(Mutex::new(Value::Null));
    let (inner_log, outer_log) = (log.clone(), log.clone());
    let double = JsonDouble::new("wc").feature("defs", move |js, b| {
        let owner = js.call(b, "feature", json!([]))?;
        let helpers = module("stdcore/features/userdefs");
        // `\inner`.
        let log = inner_log.clone();
        let kept_inner = Arc::clone(&kept);
        let hook = js.lend_expand("per-plan", move |js, _, cx, xp| {
            let outer_site = kept_inner.lock().map(|site| site.clone()).unwrap_or(Value::Null);
            log.note("inner call", json!(js.call_number()));
            log.note("outer site in the inner call", outcome(js.call(&outer_site, "name", json!([]))));
            log.note("inner depth", js.call(xp, "depth", json!([]))?);
            log.note("diagnose", js.call(xp, "diagnose", json!([{ "id": "wc.inner.seen", "severity": "note", "message": "inner seen" }]))?);
            Ok(json!({ "emit": [js.call(cx, "stage_chars", json!(["IN"]))?] }))
        })?;
        let ud = js.lend(Entry::UserDefinition(inner.clone()));
        let register_log = inner_log.clone();
        register_log.note("no method expand", outcome(js.call(&hook, "expand", json!([]))));
        let def = js.call(&ud, "element_def_with", json!([owner, "wc.macro.inner", hook]))?;
        register_log.note("no method element_def", outcome(js.call(&def, "owner", json!([]))));
        let stage = js.lend_stage(
            &json!({ "name": "wc.unused", "effect": "compute", "scope": "per-tree", "provides": [], "writes": [], "docs": "" }),
            &json!({ "requires": [], "reads": [], "reads_opt": [], "after": [], "before": [] }),
            |_, _, _| Ok(()),
        )?;
        register_log.note("no method stage", outcome(js.call(&stage, "run", json!([]))));
        js.call(b, "element", json!([def]))?;
        let rule = js.call(&helpers, "expand_in_tree_rule_for", json!(["wc.macro.inner", { "kind": "macro", "name": "inner" }]))?;
        js.call(b, "lowering_rule", json!([rule]))?;
        // `\outer`.
        let log = outer_log.clone();
        let kept_outer = Arc::clone(&kept);
        let definition = outer.clone();
        let hook = js.lend_expand("per-plan", move |js, node, cx, xp| {
            if let Ok(mut site) = kept_outer.lock() {
                *site = node.clone();
            }
            log.note("outer call", json!(js.call_number()));
            log.note("outer depth", js.call(xp, "depth", json!([]))?);
            log.note("recursion_limit", js.call(xp, "recursion_limit", json!([]))?);
            // The definition the element was registered with, through the
            // context (plan A, A37).
            let registered = js.call(cx, "definition", json!([]))?;
            log.note("registered", json!(kind_of(&registered).map(HandleKind::name)));
            if kind_of(&registered).is_some() {
                log.note("registered declaration", js.call(&registered, "definition", json!([]))?);
            }
            let ud = js.lend(Entry::UserDefinition(definition.clone()));
            let variant = js.call(&ud, "variant_for_use", json!([node]))?;
            log.note("variant", json!(kind_of(&variant).map(HandleKind::name)));
            let template = js.call(&variant, "template", json!([]))?;
            log.note("template", json!(kind_of(&template).map(HandleKind::name)));
            let bindings = js.call(&module("lang/rewrites/userdefs"), "DefinitionBindings::new", json!([node, variant]))?;
            log.note("bindings", json!(kind_of(&bindings).map(HandleKind::name)));
            log.note("no method", outcome(js.call(&template, "instantiate", json!([]))));
            log.note("wrong order", outcome(js.call(xp, "instantiate", json!([node, cx, template, bindings]))));
            log.note("no method placeholder_bindings", outcome(js.call(&bindings, "bind", json!([]))));
            let staged = js.call(xp, "instantiate", json!([cx, node, template, bindings]))?;
            if let Some(first) = staged.as_array().and_then(|nodes| nodes.first()) {
                log.note("no method staged", outcome(js.call(first, "chars", json!([]))));
            }
            log.note("staged", json!(staged.as_array().map(|nodes| nodes.iter().map(|node| kind_of(node).map(HandleKind::name)).collect::<Vec<_>>())));
            Ok(json!({ "emit": staged }))
        })?;
        let ud = js.lend(Entry::UserDefinition(outer.clone()));
        let def = js.call(&ud, "element_def_with", json!([owner, "wc.macro.outer", hook]))?;
        js.call(b, "element", json!([def]))?;
        let rule = js.call(&helpers, "expand_in_tree_rule_for", json!(["wc.macro.outer", { "kind": "macro", "name": "outer", "args": "{" }]))?;
        js.call(b, "lowering_rule", json!([rule]))?;
        Ok(())
    });
    registry_with([double.package()])
}

#[test]
fn an_expander_a_context_and_the_definition_s_objects_answer_their_methods_by_name() {
    let log = Log::default();
    let registry = nested(&log);
    let run = common::run(&registry, flm_core::stdcore::backends::text::WORKFLOW, "Say \\outer{x}.");
    assert_eq!(common::diagnostics(&run), ["wc.inner.seen: inner seen"]);
    let text = run.table(flm_core::stdcore::backends::text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone());
    assert_eq!(text.as_deref(), Some("Say [IN x].\n"));
    assert_eq!(log.get("outer depth"), json!(0));
    assert_eq!(log.get("inner depth"), json!(1));
    assert!(log.get("recursion_limit").as_u64().is_some_and(|limit| limit > 1), "{}", log.get("recursion_limit"));
    assert_eq!(log.get("registered"), json!("user_definition"));
    assert_eq!(log.get("registered declaration")["name"], json!("outer"));
    assert_eq!(log.get("variant"), json!("mode_variant"));
    assert_eq!(log.get("template"), json!("compiled_template"));
    assert_eq!(log.get("bindings"), json!("placeholder_bindings"));
    assert_eq!(log.get("staged").as_array().map(|kinds| kinds.iter().all(|kind| kind == "staged")), Some(true));
    assert_eq!(log.get("diagnose"), Value::Null);
    assert_eq!(log.get("no method")["err"]["detail"], json!("an object of the kind `compiled_template` has no method `instantiate`"));
    // Every other kind without methods answers the same way (the WP-5
    // review's m3).
    for (kind, method) in
        [("stage", "run"), ("expand", "expand"), ("element_def", "owner"), ("staged", "chars"), ("placeholder_bindings", "bind")]
    {
        assert_eq!(
            log.get(&format!("no method {kind}"))["err"],
            json!({ "kind": "argument", "method": method, "detail": format!("an object of the kind `{kind}` has no method `{method}`") }),
            "{kind}"
        );
    }
    assert_eq!(
        log.get("wrong order")["err"],
        json!({ "kind": "argument", "method": "instantiate", "detail": "an object of the kind `expand_cx` is expected here, not one of the kind `node`" })
    );
}

// --- stale handles, wrong kinds, unknown methods ---------------------------------------------------

#[test]
fn a_handle_of_an_outer_call_is_stale_in_an_inner_call() {
    let log = Log::default();
    let registry = nested(&log);
    let (text, _) = text_of(&registry, "Say \\outer{x}.");
    assert_eq!(text, "Say [IN x].\n");
    let outer = log.get("outer call");
    let stale = log.get("outer site in the inner call");
    assert_eq!(stale["err"]["kind"], json!("stale_handle"), "{stale}");
    // The handle names the outer call, which is running but not innermost.
    assert_eq!(stale["err"]["handle"]["call"], outer);
    assert_ne!(log.get("inner call"), outer);
}

#[test]
fn a_handle_of_a_finished_call_is_stale() {
    let log = Log::default();
    let seen = log.clone();
    let double = JsonDouble::new("wc").feature("probe", move |js, b| {
        js.call(b, "table", json!(["wc.out", ""]))?;
        let builder = b.clone();
        let log = seen.clone();
        let stage = js.lend_stage(
            &json!({ "name": "wc.stage", "effect": "compute", "scope": "document", "writes": ["wc.out"] }),
            &json!({}),
            move |js, _, _| {
                log.note("builder in the stage", outcome(js.call(&builder, "feature", json!([]))));
                Ok(())
            },
        )?;
        js.call(b, "stage", json!([stage]))?;
        Ok(())
    });
    let registry = registry_with([double.package()]);
    run_demanding(&registry, &["wc.out"], &["Text."]);
    let stale = log.get("builder in the stage");
    assert_eq!(stale["err"]["kind"], json!("stale_handle"), "{stale}");
    assert_eq!(stale["err"]["handle"]["kind"], json!("feature_builder"), "{stale}");
    assert!(log.labels().len() == 1);
}

#[test]
fn a_method_on_a_handle_of_another_kind_is_an_argument_error() {
    let (mut table, root) = table_with_root(7, "\\section[a=b]{T}");
    let section = named(&mut table, &root, "section");
    let options = call(&mut table, &section, "argument_content_nodes_named", json!(["options"])).expect("written");
    let list = call(&mut table, &module("techy/extract"), "parse_keyval_drop_annotations", json!([options])).expect("a list");
    // A node's method on a key-value list.
    let error = call(&mut table, &list, "chars", json!([])).expect_err("not a node");
    assert_eq!(error, ExtError::Argument { method: "chars".to_string(), detail: "an object of the kind `key_vals` has no method `chars`".to_string() });
    // A handle naming another kind than the object at its index.
    let Ok(mut forged) = handle_from_value(&list) else { panic!("a handle") };
    forged.kind = HandleKind::Node;
    let error = dispatch::call(&mut table, forged, "chars", &json!([])).expect_err("a forged kind");
    assert_eq!(
        error.to_string(),
        format!("`chars`: the object at index {} of the running call is of the kind `key_vals`, not `node`", forged.index)
    );
    // An argument of another kind.
    let error = call(&mut table, &module("lang/text"), "chars_text", json!([[list]])).expect_err("not a node");
    assert_eq!(error.to_string(), "`chars_text`: an object of the kind `node` is expected here, not one of the kind `key_vals`");
    // An index the call never lent.
    let beyond = handle_to_value(Handle { kind: HandleKind::Node, call: 7, index: 999 });
    let error = call(&mut table, &beyond, "chars", json!([])).expect_err("never lent");
    assert_eq!(error.to_string(), "`chars`: the running call lent no `node` object at index 999");
}

#[test]
fn an_unknown_method_is_an_argument_error() {
    let (mut table, root) = table_with_root(8, "ab");
    let error = call(&mut table, &root, "nonesuch", json!([])).expect_err("no such method");
    assert_eq!(error, ExtError::Argument { method: "nonesuch".to_string(), detail: "an object of the kind `node` has no method `nonesuch`".to_string() });
    let error = call(&mut table, &root, "kind", json!({})).expect_err("arguments not a list");
    assert_eq!(error.to_string(), "`kind`: the arguments must be a list, not `{}`");
    let error = call(&mut table, &json!({ "kind": "node" }), "kind", json!([])).expect_err("not a handle");
    assert_eq!(error.kind(), "argument");
}

// --- E5's WP-1a: the data payload, the lowered trees, a node's payload and slots -------------------

#[test]
fn a_data_payload_element_and_the_lowered_trees_answer_by_name() {
    use flm_core::stdcore::backends::text;
    let log = Log::default();
    let seen = log.clone();
    let double = JsonDouble::new("bd").feature("notes", move |js, b| {
        js.call(
            b,
            "element",
            json!([{
                "owner": "notes",
                "names": ["bdnote"],
                "syntax": { "macro": { "args": [{ "name": "content", "code": "{" }] } },
                "element": { "name": "bd.note" },
            }]),
        )?;
        js.call(b, "presentation_element", json!(["bd.pres.note", "A note."]))?;
        let lowering = module("core/lowering");
        let stage_note = js.lend_lower(|js, node, cx, lw| {
            let content = js.call(lw, "lower_arg", json!([cx, node, 0]))?;
            let staged = js.call(lw, "pres", json!([cx, "bd.pres.note", { "kind": "warn" }, content]))?;
            Ok(json!({ "emit": [staged] }))
        });
        let rule = js.call(
            &lowering,
            "LoweringRule::new",
            json!(["bd.note.lower", "bd.note", { "produces": ["bd.pres.note"] }, stage_note, "everywhere"]),
        )?;
        js.call(b, "lowering_rule", json!([rule]))?;
        let read_note = js.lend_lower(|js, node, cx, lw| {
            let payload = js.call(node, "payload", json!([]))?;
            let kind = payload["kind"].as_str().unwrap_or("?").to_string();
            let mut content = vec![js.call(lw, "text", json!([cx, format!("({kind}) ")]))?];
            if let Value::Array(children) = js.call(lw, "lower_children", json!([cx, node]))? {
                content.extend(children);
            }
            let span = js.call(lw, "pres", json!([cx, "flm.pres.span", {}, content]))?;
            Ok(json!({ "emit": [span] }))
        });
        let rule = js.call(
            &lowering,
            "LoweringRule::new",
            json!(["bd.pres.note.lower", "bd.pres.note", { "produces": ["flm.pres.span"] }, read_note, "everywhere"]),
        )?;
        js.call(b, "lowering_rule", json!([rule]))?;
        js.call(b, "table", json!(["bd.seen", "What the reader saw."]))?;
        let seen = seen.clone();
        let stage = js.lend_stage(
            &json!({ "name": "bd.read", "effect": "compute", "scope": "document", "writes": ["bd.seen"] }),
            &json!({ "reads": ["flm.lowered.text"] }),
            move |js, cx, _| {
                let read = js.call(cx, "get", json!(["flm.lowered.text"]))?;
                let lowered = read["lowered"].clone();
                seen.note("kind", json!(kind_of(&lowered).map(HandleKind::name)));
                let trees = js.call(&lowered, "trees", json!([]))?;
                let id = trees[0][0].clone();
                seen.note("ids", json!(trees.as_array().map(|trees| trees.iter().map(|pair| pair[0].clone()).collect::<Vec<_>>())));
                seen.note("rounds", js.call(&lowered, "rounds", json!([id]))?);
                seen.note("requires", js.call(&lowered, "requires", json!([id]))?);
                seen.note("absent", js.call(&lowered, "tree", json!(["tree#9"]))?);
                let root = js.call(&lowered, "tree", json!([id]))?;
                seen.note("root", js.call(&root, "element_name", json!([]))?);
                seen.note("root payload", js.call(&root, "payload", json!([]))?);
                let descendants = js.call(&root, "descendants", json!([]))?;
                for node in descendants.as_array().into_iter().flatten() {
                    let element = js.call(node, "element_name", json!([]))?;
                    if element == json!("flm.pres.block-heading") {
                        seen.note("heading", js.call(node, "payload", json!([]))?);
                    }
                    if element == json!("flm.pres.list") {
                        seen.note("slot_count", js.call(node, "slot_count", json!([]))?);
                        seen.note("slot_name", js.call(node, "slot_name", json!([0]))?);
                        seen.note("slot past", js.call(node, "slot_content_nodes", json!([9]))?);
                        let item = js.call(node, "slot_content_nodes", json!([1]))?;
                        seen.note("item kinds", json!(item.as_array().map(Vec::len)));
                    }
                }
                Ok(())
            },
        )?;
        js.call(b, "stage", json!([stage]))?;
        Ok(())
    });
    let registry = registry_with([double.package()]);
    let source = "\\section{T}\n\n\\bdnote{x}\n\n\\begin{itemize}\\item a\\end{itemize}";
    let run = run_demanding(&registry, &[text::TEXT_OUTPUT.name(), "bd.seen"], &[source]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|out| out.text.clone()).unwrap_or_default();
    assert!(out.contains("(warn) x"), "{out}");
    assert_eq!(log.get("kind"), json!("lowered_trees"));
    assert_eq!(log.get("ids"), json!(["tree#0"]));
    assert_eq!(log.get("rounds"), json!(2));
    assert_eq!(log.get("requires"), json!(["flm.headings.style"]));
    assert_eq!(log.get("absent"), Value::Null);
    assert_eq!(log.get("root"), json!("flm.container"));
    assert_eq!(log.get("root payload"), Value::Null);
    assert_eq!(log.get("heading")["level"], json!(1), "{}", log.get("heading"));
    assert_eq!(log.get("slot_count"), json!(2));
    assert_eq!(log.get("slot_name"), json!("label"));
    assert_eq!(log.get("slot past"), Value::Null);
    assert_eq!(log.get("item kinds"), json!(1));
}

#[test]
fn a_rewrite_stage_written_against_handles_restages_through_the_dispatcher() {
    use flm_core::stdcore::backends::text;
    let log = Log::default();
    let seen = log.clone();
    let double = JsonDouble::new("bd").feature("quotes", move |js, b| {
        js.call(
            b,
            "element",
            json!([{
                "owner": "quotes",
                "names": ["bdquote"],
                "syntax": { "macro": { "args": [{ "name": "text", "code": "{" }, { "name": "kept", "code": "{" }] } },
                "element": { "name": "bd.quote" },
            }]),
        )?;
        let lower_quote = js.lend_lower(|js, node, cx, lw| {
            let mut out = js.call(lw, "lower_arg", json!([cx, node, 0]))?.as_array().cloned().unwrap_or_default();
            out.push(js.call(lw, "text", json!([cx, "/"]))?);
            out.extend(js.call(lw, "lower_arg", json!([cx, node, 1]))?.as_array().cloned().unwrap_or_default());
            Ok(json!({ "emit": out }))
        });
        let rule = js.call(&module("core/lowering"), "LoweringRule::new", json!(["bd.quote.lower", "bd.quote", {}, lower_quote, "everywhere"]))?;
        js.call(b, "lowering_rule", json!([rule]))?;
        let seen = seen.clone();
        let stage = js.lend_rewrite(
            &json!({ "name": "bd.at", "publishes": "bd.atted", "docs": "Writes `@` as `(at)`." }),
            &json!({ "after": ["flm.userdefs"], "before": ["flm.blocks.structured"] }),
            move |js, node, cx, rs| {
                if js.call(node, "parent", json!([]))?.is_null() {
                    let tree = js.call(cx, "tree_id", json!([]))?;
                    seen.note(format!("layout of {}", tree.as_str().unwrap_or("?")), js.call(cx, "layout", json!([]))?);
                    seen.note("option", json!(js.call(cx, "option", json!(["bd.options"])).err().map(|error| error.to_string())));
                }
                if let Value::String(text) = js.call(node, "chars", json!([]))? {
                    if !text.contains('@') {
                        return Ok(json!("descend"));
                    }
                    let staged = js.call(rs, "stage_chars", json!([text.replace('@', "(at)"), node]))?;
                    return Ok(json!({ "emit": [staged] }));
                }
                match js.call(node, "name", json!([]))?.as_str() {
                    Some("bdquote") => {
                        let mut content = Vec::new();
                        let children = js.call(node, "argument_content_nodes", json!([0]))?;
                        for child in children.as_array().into_iter().flatten() {
                            content.extend(js.call(rs, "restage_subtree", json!([child]))?.as_array().cloned().unwrap_or_default());
                        }
                        let argument = js.call(rs, "restage_argument_with_content", json!([node, 0, content]))?;
                        let kept = js.call(rs, "restage_argument_named", json!([node, "kept"]))?;
                        seen.note("provided", js.call(&argument, "is_provided", json!([]))?);
                        let refused = js.call(rs, "restage_argument", json!([node, 5])).err().map(|error| error.to_string());
                        seen.note("out of range", json!(refused));
                        let call = js.call(rs, "restage_invocation", json!([node, [argument, kept], []]))?;
                        let again = js.call(rs, "restage_invocation", json!([node, [argument], []])).err().map(|error| error.to_string());
                        seen.note("again", json!(again));
                        js.call(rs, "diagnose", json!([{ "id": "bd.quote.seen", "severity": "warning", "message": "a quote" }]))?;
                        let open = js.call(rs, "stage_chars", json!(["<", node]))?;
                        let close = js.call(rs, "stage_chars", json!([">", node]))?;
                        Ok(json!({ "emit": [open, call, close] }))
                    }
                    Some("emph") => Ok(json!({ "emit": js.call(rs, "restage_children", json!([node]))? })),
                    Some("itemize") => {
                        let slot = js.call(rs, "restage_slot", json!([node, 0]))?;
                        seen.note("slot", js.call(&slot, "name", json!([]))?);
                        let emptied = js.call(rs, "restage_slot_with_content", json!([node, 0, []]))?;
                        seen.note("emptied", js.call(&emptied, "name", json!([]))?);
                        Ok(json!("descend"))
                    }
                    _ => Ok(json!("descend")),
                }
            },
        )?;
        js.call(b, "rewrite_stage", json!([stage]))?;
        Ok(())
    });
    let registry = registry_with([double.package()]);
    let source = "x@y \\bdquote{a@b \\emph{c@d}}{e@f}\n\n\\begin{itemize}\\item g@h\\end{itemize}";
    let run = run_demanding(&registry, &[text::TEXT_OUTPUT.name(), "bd.atted"], &[source]);
    let found: Vec<String> = run.diagnostics().iter().map(|d| format!("{} {}", d.identifier(), d.message())).collect();
    assert_eq!(found, vec!["bd.quote.seen a quote"]);
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|out| out.text.clone()).unwrap_or_default();
    assert!(out.starts_with("x(at)y <a(at)b c(at)d/e(at)f>"), "{out}");
    assert!(out.contains("g(at)h"), "{out}");
    // The stage runs on the document's tree and on the synthesized values.
    assert_eq!(log.get("layout of tree#0"), json!("inline-or-blocks"));
    assert!(log.get("option").as_str().is_some_and(|text| text.contains("bd.options")), "{}", log.get("option"));
    assert_eq!(log.get("provided"), json!(true));
    assert!(log.get("out of range").as_str().is_some_and(|text| text == "`\\bdquote` on line 1 has no argument at index 5: it has 2 arguments"), "{}", log.get("out of range"));
    assert_eq!(
        log.get("again"),
        json!("`restage_invocation`: each restaged argument or slot may be given to `restage_invocation` once")
    );
    assert_eq!(log.get("slot"), json!("body"));
    assert_eq!(log.get("emptied"), json!("body"));
}
