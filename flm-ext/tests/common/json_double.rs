//! `JsonDouble`, an [`ExtRuntime`] whose handlers receive only handles, in
//! their JSON shape, and act exclusively through
//! [`dispatch::call`](flm_ext::adapter::dispatch::call) with JSON: what a
//! JavaScript handler does, minus the engine (plan A, section 7.1). Beside it,
//! [`case_one`], case 1 written against it as plan B's `wordcount.js` writes it.
//!
//! A handler is a closure given a [`Js`] (the running call's table, which is
//! what the binding's glue reaches) and the handles of the objects lent to the
//! call. Lending a script's own stage or hook into the table (`lend_stage`,
//! `lend_expand`) is the glue's work in plan B (`__flm.lend_stage`,
//! `__flm.lend_expand`); [`Js`] does it here.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use flm_core::core::registry::{PackageEntry, PackageFactory};
use flm_core::core::settings::PackageSettings;
use flm_ext::adapter::dispatch;
use flm_ext::adapter::handles::{module_handle, CallTable, Entry};
use flm_ext::adapter::values::{
    expansion_from_value, handle_from_value, handle_to_value, lowering_from_value, restage_from_value, JsonForm,
};
use flm_ext::api::core::lowering::{Lower, LowerCx, Lowerer, Lowering};
use flm_ext::api::core::data::store::StageOut;
use flm_ext::api::core::registry::FeatureBuilder;
use flm_ext::api::core::run::{RewriteCx, StageCx};
use flm_ext::api::core::stage::{Needs, RewriteDecl, RewriteStage, Stage, StageDecl};
use flm_ext::api::techy::transform::{Restage, Restager};
use flm_ext::api::lang::rewrites::userdefs::{Expand, ExpandCx, ExpandPolicy, Expander, Expansion};
use flm_ext::api::techy::node::Node;
use flm_ext::adapter::package_factory;
use flm_ext::engine::SettingsReport;
use flm_ext::{ExtError, ExtRuntime, ExtensionIdentity, FeatureReport, PackageReport};
use serde_json::{json, Value};

/// The number of the next call, for every table of every double: a handle
/// never names the call of another table.
static CALLS: AtomicU64 = AtomicU64::new(1);

/// A fresh call number.
fn next_call() -> u64 {
    CALLS.fetch_add(1, Ordering::Relaxed)
}

/// What a handler holds: the running call's table, reached by name with JSON.
pub struct Js<'t, 'c> {
    table: &'t mut CallTable<'c>,
}

impl<'c> Js<'_, 'c> {
    /// `dispatch::call` on the object `target` stands for (a handle in its
    /// JSON shape), with the JSON list `args`.
    pub fn call(&mut self, target: &Value, method: &str, args: Value) -> Result<Value, ExtError> {
        let handle = handle_from_value(target)?;
        dispatch::call(self.table, handle, method, &args)
    }

    /// The number of the running call.
    pub fn call_number(&self) -> u64 {
        self.table.call()
    }

    /// Lends `entry` into the running call's table, as a binding lends the
    /// objects it holds; answers the handle in its JSON shape. (The tests lend
    /// a definition registered from Rust this way, so that a hook can reach
    /// its compiled form.)
    pub fn lend(&mut self, entry: Entry<'c>) -> Value {
        handle_to_value(self.table.lend(entry))
    }

    /// Lends a stage of the handler's own into the table, as plan B's
    /// `__flm.lend_stage(decl, needs)` does: the declaration and the needs in
    /// their JSON shape, the work a closure given the handles of the context
    /// and the output.
    pub fn lend_stage(
        &mut self,
        decl: &Value,
        needs: &Value,
        run: impl for<'t, 'x> Fn(&mut Js<'t, 'x>, &Value, &Value) -> Result<(), ExtError> + Send + Sync + 'static,
    ) -> Result<Value, ExtError> {
        let stage = JsonStage { decl: StageDecl::from_value(decl)?, needs: Needs::from_value(needs)?, run: Box::new(run) };
        Ok(handle_to_value(self.table.lend(Entry::Stage(Arc::new(stage)))))
    }

    /// Lends a hook of the handler's own into the table, as plan B's
    /// `__flm.lend_expand(element, policy)` does.
    pub fn lend_expand(
        &mut self,
        policy: &str,
        expand: impl for<'t, 'x> Fn(&mut Js<'t, 'x>, &Value, &Value, &Value) -> Result<Value, ExtError> + Send + Sync + 'static,
    ) -> Result<Value, ExtError> {
        let policy = ExpandPolicy::from_value(&json!(policy))?;
        let hook = JsonExpand { policy, expand: Box::new(expand) };
        Ok(handle_to_value(self.table.lend(Entry::Expand(Arc::new(hook)))))
    }
}

impl Js<'_, '_> {
    /// Lends a rewrite stage of the handler's own into the table, as a
    /// binding's glue lends a script's rewrite stage: the declaration and the
    /// needs in their JSON shape, the handler a closure given the handles of
    /// the node, the context and the restager, answering the restage in its
    /// JSON shape (`"descend"` or `{"emit": [...]}`).
    pub fn lend_rewrite(
        &mut self,
        decl: &Value,
        needs: &Value,
        restage: impl for<'t, 'x> Fn(&mut Js<'t, 'x>, &Value, &Value, &Value) -> Result<Value, ExtError> + Send + Sync + 'static,
    ) -> Result<Value, ExtError> {
        let stage =
            JsonRewrite { decl: RewriteDecl::from_value(decl)?, needs: Needs::from_value(needs)?, restage: Box::new(restage) };
        Ok(handle_to_value(self.table.lend(Entry::RewriteStage(Arc::new(stage)))))
    }
}

impl Js<'_, '_> {
    /// Lends a rule's body of the handler's own into the table, as a
    /// binding's glue lends a script's body (the call is the body's `lower`,
    /// given the handles of the node, the context and the lowerer; it answers
    /// the lowering in its JSON shape).
    pub fn lend_lower(
        &mut self,
        lower: impl for<'t, 'x> Fn(&mut Js<'t, 'x>, &Value, &Value, &Value) -> Result<Value, ExtError> + Send + Sync + 'static,
    ) -> Value {
        handle_to_value(self.table.lend(Entry::Lower(Arc::new(JsonLower { lower: Box::new(lower) }))))
    }
}

/// The handle of the module `path`, in its JSON shape.
pub fn module(path: &str) -> Value {
    module_handle(path).map(handle_to_value).unwrap_or(Value::Null)
}

/// A feature's `register` handler: given the builder's handle.
type RegisterFn = Box<dyn for<'t, 'c> Fn(&mut Js<'t, 'c>, &Value) -> Result<(), ExtError> + Send + Sync>;

/// A stage's `run` handler: given the handles of the context and the output.
type StageHandlerFn = Box<dyn for<'t, 'c> Fn(&mut Js<'t, 'c>, &Value, &Value) -> Result<(), ExtError> + Send + Sync>;

/// A hook's `expand` handler: given the handles of the use site, the context
/// and the expander; answers the expansion in its JSON shape.
type ExpandHandlerFn =
    Box<dyn for<'t, 'c> Fn(&mut Js<'t, 'c>, &Value, &Value, &Value) -> Result<Value, ExtError> + Send + Sync>;

/// An extension whose handlers act through JSON alone.
pub struct JsonDouble {
    name: String,
    report: PackageReport,
    registers: BTreeMap<String, RegisterFn>,
}

impl JsonDouble {
    /// The package `name`, using nothing, with no feature yet.
    pub fn new(name: &str) -> JsonDouble {
        JsonDouble {
            name: name.to_string(),
            report: PackageReport {
                name: "_".to_string(),
                uses: Vec::new(),
                block: Default::default(),
                templates: Vec::new(),
                features: Vec::new(),
                host: Vec::new(),
            },
            registers: BTreeMap::new(),
        }
    }

    /// Adds the feature `name`, whose `register` handler is `register`.
    pub fn feature(
        mut self,
        name: &str,
        register: impl for<'t, 'c> Fn(&mut Js<'t, 'c>, &Value) -> Result<(), ExtError> + Send + Sync + 'static,
    ) -> JsonDouble {
        self.report.features.push(FeatureReport {
            name: name.to_string(),
            title: format!("The feature {name}"),
            docs: String::new(),
            version: None,
            settings: None,
        });
        self.registers.insert(name.to_string(), Box::new(register));
        self
    }

    /// Declares `report` as the settings of the feature `feature`.
    pub fn settings(mut self, feature: &str, report: SettingsReport) -> JsonDouble {
        if let Some(found) = self.report.features.iter_mut().find(|f| f.name == feature) {
            found.settings = Some(report);
        }
        self
    }

    /// The package's factory, `NAME#_`.
    pub fn factory(self) -> PackageFactory {
        let extension = ExtensionIdentity { identity: self.name.clone(), stem: self.name.clone() };
        let report = self.report.clone();
        let digest: Arc<[u8]> = Arc::from(Vec::new());
        let caller = flm_ext::host::HostCaller { extension: extension.clone(), package: report.name.clone(), digest_input: Arc::clone(&digest) };
        let files: Arc<dyn flm_ext::ExtensionFiles + Send + Sync> = Arc::new(flm_ext::MemoryFiles::new());
        let services = Arc::new(flm_ext::adapter::services::DeclaredServices::new(caller, &BTreeMap::new(), &[], files));
        package_factory(&extension, &report, Arc::new(self), digest, "", services).expect("the report is a package")
    }

    /// The package built from empty settings.
    pub fn package(self) -> PackageEntry {
        self.factory().build(&PackageSettings::new()).expect("the package builds")
    }
}

impl ExtRuntime for JsonDouble {
    fn register(&self, package: &str, feature: &str, _: &Value, b: &mut dyn FeatureBuilder) -> Result<(), ExtError> {
        assert_eq!(package, "_", "the double has one package");
        let Some(register) = self.registers.get(feature) else {
            return Err(ExtError::Handler { detail: format!("no feature `{feature}`"), data: Value::Null });
        };
        let mut table = CallTable::new(next_call());
        let b = handle_to_value(table.lend(Entry::Builder(b)));
        register(&mut Js { table: &mut table }, &b)
    }

    fn digest_input(&self) -> Vec<u8> {
        Vec::new()
    }
}

/// A stage lent by a handler.
struct JsonStage {
    decl: StageDecl,
    needs: Needs,
    run: StageHandlerFn,
}

impl Stage for JsonStage {
    fn decl(&self) -> StageDecl {
        self.decl.clone()
    }

    fn needs(&self) -> Needs {
        self.needs.clone()
    }

    fn run<'c>(&self, cx: &'c dyn StageCx, out: &'c mut dyn StageOut) -> Result<(), ExtError> {
        let mut table = CallTable::new(next_call());
        let cx = handle_to_value(table.lend(Entry::StageCx(cx)));
        let out = handle_to_value(table.lend(Entry::StageOut(out)));
        (self.run)(&mut Js { table: &mut table }, &cx, &out)
    }
}

/// A hook lent by a handler.
struct JsonExpand {
    policy: ExpandPolicy,
    expand: ExpandHandlerFn,
}

impl Expand for JsonExpand {
    fn policy(&self) -> ExpandPolicy {
        self.policy
    }

    fn expand<'c>(
        &self,
        node: &'c dyn Node<'c>,
        cx: &'c mut dyn ExpandCx,
        xp: &'c mut dyn Expander,
    ) -> Result<Expansion, ExtError> {
        let mut table = CallTable::new(next_call());
        let node = handle_to_value(table.lend(Entry::Node(Box::new(node))));
        let cx = handle_to_value(table.lend(Entry::ExpandCx(cx)));
        let xp = handle_to_value(table.lend(Entry::Expander(xp)));
        let answer = (self.expand)(&mut Js { table: &mut table }, &node, &cx, &xp)?;
        expansion_from_value(&mut table, &answer)
    }
}

/// A rule's body lent by a handler.
struct JsonLower {
    lower: ExpandHandlerFn,
}

impl Lower for JsonLower {
    fn lower<'c>(
        &self,
        node: &'c dyn Node<'c>,
        cx: &'c mut dyn LowerCx,
        lw: &'c mut dyn Lowerer,
    ) -> Result<Lowering, ExtError> {
        let mut table = CallTable::new(next_call());
        let node = handle_to_value(table.lend(Entry::Node(Box::new(node))));
        let cx = handle_to_value(table.lend(Entry::LowerCx(cx)));
        let lw = handle_to_value(table.lend(Entry::Lowerer(lw)));
        let answer = (self.lower)(&mut Js { table: &mut table }, &node, &cx, &lw)?;
        lowering_from_value(&mut table, &answer)
    }
}

/// A rewrite stage lent by a handler.
struct JsonRewrite {
    decl: RewriteDecl,
    needs: Needs,
    restage: ExpandHandlerFn,
}

impl RewriteStage for JsonRewrite {
    fn decl(&self) -> RewriteDecl {
        self.decl.clone()
    }

    fn needs(&self) -> Needs {
        self.needs.clone()
    }

    fn restage<'c>(&self, node: &'c dyn Node<'c>, cx: &'c dyn RewriteCx, rs: &'c mut dyn Restager) -> Result<Restage, ExtError> {
        let mut table = CallTable::new(next_call());
        let node = handle_to_value(table.lend(Entry::Node(Box::new(node))));
        let cx = handle_to_value(table.lend(Entry::RewriteCx(cx)));
        let rs = handle_to_value(table.lend(Entry::Restager(rs)));
        let answer = (self.restage)(&mut Js { table: &mut table }, &node, &cx, &rs)?;
        restage_from_value(&mut table, &answer)
    }
}

// ---- case 1, as plan B's `wordcount.js` writes it ----

/// The table of word counts.
pub const WORDS: &str = "wordcount.words";
/// The table of markers.
pub const TODOS: &str = "wordcount.todos";

/// Declares `definition` (its JSON shape): its element under the helper's
/// name, made by the helper with `hook` when one is given, and its lowering
/// rule. Answers the element's name.
fn declare(js: &mut Js<'_, '_>, b: &Value, owner: &Value, definition: Value, hook: Option<Value>) -> Result<Value, ExtError> {
    let userdefs = module("lang/rewrites/userdefs");
    let helpers = module("stdcore/features/userdefs");
    let ud = js.call(&userdefs, "UserDefinition::new", json!([definition]))?;
    let element = js.call(&helpers, "element_name_under", json!(["wordcount", definition]))?;
    let def = match hook {
        Some(hook) => js.call(&ud, "element_def_with", json!([owner, element, hook]))?,
        None => js.call(&ud, "element_def", json!([owner, element]))?,
    };
    js.call(b, "element", json!([def]))?;
    let rule = js.call(&helpers, "expand_in_tree_rule_for", json!([element, definition]))?;
    js.call(b, "lowering_rule", json!([rule]))?;
    Ok(element)
}

/// The number of words of `text`, as plan B's `helpers.js` counts them.
fn count_words(text: &str) -> i64 {
    i64::try_from(text.split_whitespace().count()).unwrap_or(i64::MAX)
}

/// The elements of a list answered as JSON.
fn list(value: Value) -> Vec<Value> {
    match value {
        Value::Array(items) => items,
        _ => Vec::new(),
    }
}

/// `register` of case 1's feature `wordcount`.
fn register(js: &mut Js<'_, '_>, b: &Value) -> Result<(), ExtError> {
    let owner = js.call(b, "feature", json!([]))?;
    // Fixed replacements.
    declare(js, b, &owner, json!({ "kind": "macro", "name": "flmname", "args": [], "content": "FLM", "docs": "the name of the language" }), None)?;
    declare(js, b, &owner, json!({ "kind": "macro", "name": "sayhi", "args": [{ "code": "{", "name": "name" }], "content": "Hello, #1!" }), None)?;
    // The marker: expanded to nothing; a stage collects it.
    let todo = json!({
        "kind": "macro", "name": "todo", "args": [{ "code": "{", "name": "text" }], "content": "",
        "docs": "a to-do note, kept out of the output",
    });
    let todo_element = declare(js, b, &owner, todo, None)?;
    // The hook: the argument's characters in capitals, staged as one chars node.
    let shout = js.lend_expand("per-plan", |js, node, cx, xp| {
        let nodes = js.call(node, "argument_content_nodes_named", json!(["text"]))?;
        let text = js.call(&module("lang/text"), "chars_text", json!([nodes]))?;
        match text.as_str() {
            Some(text) => Ok(json!({ "emit": [js.call(cx, "stage_chars", json!([text.to_uppercase()]))?] })),
            None => {
                let diagnostic = json!({
                    "id": "wordcount.shout.not-characters", "severity": "error",
                    "message": "the argument of \\shout must be plain characters",
                });
                Ok(json!({ "emit": js.call(xp, "recover", json!([cx, node, diagnostic]))? }))
            }
        }
    })?;
    let shout_definition = json!({ "kind": "macro", "name": "shout", "args": [{ "code": "{", "name": "text" }], "docs": "the text in capitals" });
    declare(js, b, &owner, shout_definition, Some(shout))?;
    // The word count, per tree.
    js.call(b, "table", json!([WORDS, "the number of words of each tree"]))?;
    let count = js.lend_stage(
        &json!({ "name": "wordcount.count", "effect": "compute", "scope": "per-tree", "writes": [WORDS], "docs": "counts the words of the tree" }),
        &json!({}),
        |js, cx, out| {
            let tree = js.call(cx, "tree_id", json!([]))?;
            let root = js.call(cx, "my_tree", json!([]))?;
            let mut words = 0;
            for node in list(js.call(&root, "descendants", json!([]))?) {
                if js.call(&node, "is_chars", json!([]))? == json!(true) {
                    words += count_words(js.call(&node, "chars", json!([]))?.as_str().unwrap_or(""));
                }
            }
            let piece = js.call(out, "table", json!([WORDS]))?;
            js.call(&piece, "push", json!([{ "tree": tree, "words": words }]))?;
            Ok(())
        },
    )?;
    js.call(b, "stage", json!([count]))?;
    // The markers, for the document, from the trees in which only the
    // definitions of the policy `always` are expanded.
    js.call(b, "table", json!([TODOS, "every \\todo of the document, with its tree"]))?;
    let collect = js.lend_stage(
        &json!({ "name": "wordcount.todos", "effect": "compute", "scope": "document", "writes": [TODOS] }),
        &json!({ "reads": ["flm.userdefs.always-expanded"] }),
        move |js, cx, out| {
            let read = js.call(cx, "get", json!(["flm.userdefs.always-expanded"]))?;
            let trees = read.get("trees").cloned().unwrap_or(Value::Null);
            for pair in list(js.call(&trees, "trees", json!([]))?) {
                let (tree, root) = (pair.get(0).cloned().unwrap_or(Value::Null), pair.get(1).cloned().unwrap_or(Value::Null));
                for node in list(js.call(&root, "descendants", json!([]))?) {
                    if js.call(&node, "is_callable", json!([]))? == json!(true)
                        && js.call(&node, "element_name", json!([]))? == todo_element
                    {
                        let nodes = js.call(&node, "argument_content_nodes_named", json!(["text"]))?;
                        let text = js.call(&module("lang/text"), "chars_text", json!([nodes]))?;
                        let piece = js.call(out, "table", json!([TODOS]))?;
                        js.call(&piece, "push", json!([{ "tree": tree, "text": text }]))?;
                    }
                }
            }
            Ok(())
        },
    )?;
    js.call(b, "stage", json!([collect]))?;
    Ok(())
}

/// Case 1: the package `wordcount` and its one feature `wordcount`.
pub fn case_one() -> JsonDouble {
    JsonDouble::new("wordcount").feature("wordcount", register)
}
