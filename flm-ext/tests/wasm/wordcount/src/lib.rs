//! Case 1 compiled to WebAssembly: the extension `wordcount` written against
//! `flm_ext::guest`, the mirrored API over the host's one import, the twin of
//! `tests/js/wordcount/main.js`. Fixed replacements, a marker a stage collects, a
//! hook computing its replacement, and a per-tree word count; three settings, one
//! per binding: `shout_suffix` (the configuration, read by `register`),
//! `min_length` (the option `wordcount.counting`, set by a render) and
//! `report_counts` (the option `wordcount.reporting`, set by a document).

use std::sync::Arc;

use flm_ext::ExtError;
use flm_ext::guest::core::data::store::StageOut;
use flm_ext::guest::core::data::table::TableValue;
use flm_ext::guest::core::registry::FeatureBuilder;
use flm_ext::guest::core::run::StageCx;
use flm_ext::guest::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_ext::guest::lang::mint::ArgDef;
use flm_ext::guest::lang::rewrites::userdefs::{
    Definition, Expand, ExpandCx, Expander, Expansion, UserDefinition,
};
use flm_ext::guest::lang::text::chars_text;
use flm_ext::guest::stdcore::features::userdefs::{element_name_under, expand_in_tree_rule_for};
use flm_ext::guest::techy::error::Diagnostic;
use flm_ext::guest::techy::node::Node;
use flm_ext::guest::{Extension, Feature, Package};
use serde_json::{Value, json};

mod helpers;

const WORDS: &str = "wordcount.words";
const TODOS: &str = "wordcount.todos";
const COUNTED: &str = "wordcount.counted";

// The module's exports (`flm_entry`, `flm_alloc`, `flm_free`, `flm_last_panic`) and
// the routing of `define` to `extension`.
flm_ext::guest::extension!(extension);

/// The settings the feature declares, as the JavaScript case writes them.
fn settings() -> Value {
    let object = |key: &str, kind: &str, description: &str| {
        json!({ "type": "object", "properties": { key: { "type": kind, "description": description } } })
    };
    json!({
        "config": {
            "docs": "What the macros write.",
            "schema": object("shout_suffix", "string", "Appended to the capitals of \\shout."),
        },
        "options": {
            "wordcount.counting": {
                "bind": "render",
                "docs": "How words are counted.",
                "schema": object("min_length", "integer", "The characters a word must have to count."),
            },
            "wordcount.reporting": {
                "bind": "document",
                "docs": "What the count reports.",
                "schema": object("report_counts", "boolean", "Whether each tree's count is reported as a note."),
            },
        },
    })
}

fn extension(ext: &mut Extension) {
    ext.package(
        "_",
        Package::new().uses(["stdcore"]).feature(
            "wordcount",
            Feature::new()
                .title("Word count")
                .docs("Counts words, collects to-do markers, shouts.")
                .settings(settings())
                .register(register),
        ),
    );
}

/// Declares `definition`: its element under the helper's name (with `hook` when one is
/// given) and its lowering rule. Answers the element's name.
fn declare(
    b: &mut FeatureBuilder,
    owner: &str,
    definition: Definition,
    hook: Option<Arc<dyn Expand>>,
) -> Result<String, ExtError> {
    let ud = UserDefinition::new(definition.clone())?;
    let element = element_name_under("wordcount", &definition)?;
    b.element(match hook {
        Some(hook) => ud.element_def_with(owner, &element, hook)?,
        None => ud.element_def(owner, &element)?,
    })?;
    b.lowering_rule(expand_in_tree_rule_for(&element, &definition)?)?;
    Ok(element)
}

/// The hook: the argument's characters in capitals, staged as one chars node.
struct Shout {
    suffix: String,
}

impl Expand for Shout {
    fn expand(&self, node: &Node, cx: &mut ExpandCx, xp: &mut Expander) -> Result<Expansion, ExtError> {
        let nodes = node.argument_content_nodes_named("text")?.unwrap_or_default();
        match chars_text(&nodes)? {
            Some(text) => Ok(Expansion::Emit(vec![cx.stage_chars(&format!("{}{}", text.to_uppercase(), self.suffix))?])),
            None => Ok(Expansion::Emit(xp.recover(
                cx,
                node,
                Diagnostic::error("wordcount.shout.not-characters", "the argument of \\shout must be plain characters"),
            )?)),
        }
    }
}

/// The word count, per tree: the chars nodes of the stage's own tree. Its capability
/// joins the collection `flm.validated`, so that the count runs with every standard
/// workflow.
struct Count;

impl Stage for Count {
    fn decl(&self) -> StageDecl {
        StageDecl::new("wordcount.count", Effect::Compute, Scope::PerTree)
            .writes(WORDS)
            .provides(COUNTED)
            .docs("counts the words of the tree")
    }

    fn needs(&self) -> Needs {
        Needs::new().option("wordcount.counting").option("wordcount.reporting")
    }

    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        let min = cx.option("wordcount.counting")?["min_length"].as_u64().unwrap_or(1) as usize;
        let tree = cx.tree_id()?.unwrap_or_default();
        let mut words = 0;
        if let Some(root) = cx.my_tree()? {
            for node in root.descendants()? {
                if node.is_chars()? {
                    words += helpers::count_words(&node.chars()?.unwrap_or_default(), min);
                }
            }
        }
        out.table(WORDS)?.push(json!({ "tree": tree, "words": words }))?;
        if cx.option("wordcount.reporting")?["report_counts"] == json!(true) {
            out.diags(Diagnostic::note(
                "wordcount.count",
                format!("the tree `{tree}` holds {words} words of at least {min} characters"),
            ))?;
        }
        Ok(())
    }
}

/// The markers, for the document, from the trees in which only the definitions of the
/// policy `always` are expanded.
struct Todos {
    element: String,
}

impl Stage for Todos {
    fn decl(&self) -> StageDecl {
        StageDecl::new("wordcount.todos", Effect::Compute, Scope::Document).writes(TODOS)
    }

    fn needs(&self) -> Needs {
        Needs::new().reads("flm.userdefs.always-expanded")
    }

    fn run(&self, cx: &StageCx, out: &mut StageOut) -> Result<(), ExtError> {
        let Some(TableValue::Trees(trees)) = cx.get("flm.userdefs.always-expanded")? else { return Ok(()) };
        for (tree, root) in trees.trees()? {
            for node in root.descendants()? {
                if node.is_callable()? && node.element_name()? == self.element {
                    let text = chars_text(&node.argument_content_nodes_named("text")?.unwrap_or_default())?;
                    out.table(TODOS)?.push(json!({ "tree": tree, "text": text }))?;
                }
            }
        }
        Ok(())
    }
}

fn register(b: &mut FeatureBuilder, settings: &Value) -> Result<(), ExtError> {
    let suffix = settings["shout_suffix"].as_str().unwrap_or("").to_string();
    let owner = b.feature()?;
    // Fixed replacements.
    declare(b, &owner, Definition::macro_("flmname", vec![]).content("FLM").docs("the name of the language"), None)?;
    declare(b, &owner, Definition::macro_("sayhi", vec![ArgDef::mandatory("name")]).content("Hello, #1!"), None)?;
    // The marker: expanded to nothing; a stage collects it.
    let todo = Definition::macro_("todo", vec![ArgDef::mandatory("text")]).content("").docs("a to-do note, kept out of the output");
    let todo_element = declare(b, &owner, todo, None)?;
    let shout = Definition::macro_("shout", vec![ArgDef::mandatory("text")]).docs("the text in capitals");
    declare(b, &owner, shout, Some(Arc::new(Shout { suffix })))?;
    b.table(WORDS, "the number of words of each tree")?;
    b.capability(COUNTED, "the words of each tree counted", None)?;
    b.collection_member("flm.validated", COUNTED)?;
    b.stage(Arc::new(Count))?;
    b.table(TODOS, "every \\todo of the document, with its tree")?;
    b.stage(Arc::new(Todos { element: todo_element }))?;
    Ok(())
}
