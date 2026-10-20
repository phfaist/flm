//! Expansion (plan A, WP-4): scripted definitions expanded by their own
//! templates and by scripted hooks — computed characters, recoveries, the
//! definition's own template instantiated through the façades, diagnostics
//! kept beside the node, the pass's depth and limit, hooks nested inside an
//! instantiated template — rendered through the text, HTML and recomposed
//! LaTeX outputs.

mod common;

use std::sync::{Arc, Mutex};

use flm_core::dump::golden::assert_text_golden;
use flm_core::core::registry::{
    BuildError, FeatureBuilder as CoreFeatureBuilder, FeatureInfo, FlmFeature, PackageEntry,
};
use flm_core::lang::rewrites::userdefs::{
    Expand as CoreExpand, ExpandCx as CoreExpandCx, ExpandError, Expander as CoreExpander,
    Expansion as CoreExpansion, UserDefinition as CoreUserDefinition, DEFAULT_RECURSION_LIMIT,
};
use flm_core::lang::Flm;
use flm_core::stdcore::features::userdefs::{element_name_under, expand_in_tree_rule_for as core_expand_in_tree_rule_for};
use flm_core::techy::core::node::NodeRef;
use flm_core::model::aspects::BlockRole;
use flm_core::stdcore::backends::text;
use flm_core::techy::error::DiagnosticValue;
use flm_ext::adapter::node::NodeFacade;
use flm_ext::api::core::lowering::{chars_at_lowering, spelling_at_lowering};
use flm_ext::api::lang::mint::{ArgDef, ElementDef, Mode, SyntaxDef};
use flm_ext::api::lang::rewrites::userdefs::{
    Definition, DefinitionBindings, Expand, ExpandCx, ExpandPolicy, Expander, Expansion, Staged, UserDefinition,
};
use flm_ext::api::lang::text::chars_text;
use flm_ext::api::model::element::{Aspect, ElementSpec};
use flm_ext::api::stdcore::features::userdefs::expand_in_tree_rule_for;
use flm_ext::api::techy::error::Diagnostic;
use flm_ext::api::techy::node::Node;
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::double::{defining, expand, expanded_trees, Double};
use common::{html_of, latex_of, registry_with, text_of};

/// The golden file `name` under `tests/goldens/`.
fn golden(name: &str) -> String {
    format!("{}/tests/goldens/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// A definition, as the helpers take it.
fn definition(definition: Definition) -> UserDefinition {
    UserDefinition::new(definition)
}

/// `\shout{text}`, whose own template is `<#1>`.
fn shout_definition() -> UserDefinition {
    definition(Definition::macro_("shout", vec![ArgDef::mandatory("text")]).content("<#1>"))
}

/// The registry holding the standard package and the package `wc` declaring
/// `definitions`.
fn registry(definitions: Vec<(UserDefinition, Option<Arc<dyn Expand>>)>) -> Arc<flm_core::core::registry::FlmRegistry> {
    registry_with([defining("wc", definitions).package()])
}

/// Case 1's hook of `\shout`: the argument's characters in upper case, or the
/// use site recovered with the package's own diagnostic.
fn shout_hook() -> Arc<dyn Expand> {
    expand(ExpandPolicy::PerPlan, |node, cx, xp| {
        let nodes = node.argument_content_nodes_named("text")?.unwrap_or_default();
        match chars_text(&nodes) {
            Some(text) => Ok(Expansion::Emit(vec![cx.stage_chars(&text.to_uppercase())?])),
            None => {
                let diagnostic = Diagnostic::error("wc.shout.not-characters", "`\\shout` must be given plain text")
                    .with_data(json!({ "nodes": nodes.len() }));
                Ok(Expansion::Emit(xp.recover(cx, node, diagnostic)?))
            }
        }
    })
}

/// A hook that instantiates `definition`'s own template at the use site, as
/// the definition's own expansion does.
fn instantiating_hook(definition: UserDefinition) -> Arc<dyn Expand> {
    expand(ExpandPolicy::PerPlan, move |node, cx, xp| instantiate_own(&definition, node, cx, xp))
}

/// `definition`'s template instantiated at `node`.
fn instantiate_own(
    definition: &UserDefinition,
    node: &dyn Node<'_>,
    cx: &mut dyn ExpandCx,
    xp: &mut dyn Expander,
) -> Result<Expansion, ExtError> {
    let variant = definition.variant_for_use(node)?;
    let template = variant.template();
    let bindings = DefinitionBindings::new(node, &*variant);
    Ok(Expansion::Emit(xp.instantiate(cx, node, &*template, &*bindings)?))
}

/// The data of the first diagnostic of `run` whose identifier is `id`.
fn data_of(run: &flm_core::core::run::Run, id: &str) -> Option<DiagnosticValue> {
    run.diagnostics().iter().find(|d| d.identifier() == id).map(|d| d.data().serializable_data())
}

// --- definitions without a hook ------------------------------------------------------------

#[test]
fn fixed_replacement_macros_render_in_text_html_and_latex() {
    let registry = registry(vec![
        (definition(Definition::macro_("hello", Vec::new()).content("Hello, world")), None),
        (definition(Definition::macro_("pair", vec![ArgDef::mandatory("a"), ArgDef::mandatory("b")]).content("(#1, #2)")), None),
        (definition(Definition::macro_("strong", vec![ArgDef::mandatory("text")]).content("\\textbf{#1}")), None),
    ]);
    let document = "Say \\hello{} to \\pair{x}{y}, then \\strong{z}.";
    let (text, diagnostics) = text_of(&registry, document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_text_golden(golden("fixed.txt"), &text);
    let (html, diagnostics) = html_of(&registry, document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_text_golden(golden("fixed.html"), &html);
    let (latex, diagnostics) = latex_of(&registry, document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_text_golden(golden("fixed.tex"), &latex);
}

/// `\ket{label}`, in force in math alone (its content is math).
fn ket_definition() -> UserDefinition {
    definition(Definition::macro_("ket", vec![ArgDef::mandatory("label")]).content("\\lvert #1\\rangle"))
}

#[test]
fn a_definition_in_force_in_math_alone_is_an_error_in_running_text() {
    let ket = ket_definition;
    let document = "A \\ket{x} and \\(\\ket{y}\\).";
    // The definition's own expansion.
    let (text, diagnostics) = text_of(&registry(vec![(ket(), None)]), document);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].starts_with("flm.userdefs.bad-definition"), "{diagnostics:?}");
    assert_eq!(text, "A \\ket{x} and \\(\\lvert y\\rangle\\).\n");
    // A hook asking for the variant gets the condition as an error, and
    // recovers with a diagnostic of its own carrying it.
    let ket = ket();
    let held = ket.clone();
    let seen: Arc<Mutex<Vec<Value>>> = Arc::new(Mutex::new(Vec::new()));
    let seen_by_hook = Arc::clone(&seen);
    let hook = expand(ExpandPolicy::PerPlan, move |node, cx, xp| match held.variant_for_use(node) {
        Ok(_) => instantiate_own(&held, node, cx, xp),
        Err(ExtError::Core { kind, detail, data }) => {
            if let Ok(mut seen) = seen_by_hook.lock() {
                seen.push(data);
            }
            let diagnostic = Diagnostic::error("wc.ket.not-here", detail).with_data(json!({ "condition": kind }));
            Ok(Expansion::Emit(xp.recover(cx, node, diagnostic)?))
        }
        Err(error) => Err(error),
    });
    let registry = registry(vec![(ket, Some(hook))]);
    let run = common::run(&registry, text::WORKFLOW, document);
    let diagnostics = common::diagnostics(&run);
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].starts_with("wc.ket.not-here: the definition of ‘\\ket’"), "{diagnostics:?}");
    assert_eq!(
        data_of(&run, "wc.ket.not-here"),
        Some(DiagnosticValue::Map(vec![(
            "condition".to_string(),
            DiagnosticValue::Str("flm.userdefs.bad-definition".to_string())
        )]))
    );
    // The error carries the condition's own data (plan A, A34): the same
    // value the definition's own expansion reports with the condition.
    let own_run = common::run(&registry_with([defining("wc", vec![(ket_definition(), None)]).package()]), text::WORKFLOW, document);
    let expected = data_of(&own_run, "flm.userdefs.bad-definition").expect("the condition is reported");
    let seen = seen.lock().map(|seen| seen.clone()).unwrap_or_default();
    assert_eq!(seen.len(), 1, "{seen:?}");
    assert!(!seen[0].is_null(), "{seen:?}");
    assert_eq!(seen[0], flm_ext::adapter::values::diagnostic_value_to_json(&expected));
}

#[test]
fn a_scripted_definition_marked_overriding_replaces_a_standard_macro_in_the_output() {
    let emph = definition(Definition::macro_("emph", vec![ArgDef::mandatory("text")]).content("[#1]").overriding());
    let registry = registry(vec![(emph, None)]);
    let (text, diagnostics) = text_of(&registry, "An \\emph{x} here.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "An [x] here.\n");
    let (html, _) = html_of(&registry, "An \\emph{x} here.");
    assert!(!html.contains("<em"), "{html}");
}

#[test]
fn a_block_definition_renders_its_body_s_paragraphs() {
    // The role sets the layout of the environment's body: laid out as blocks,
    // it holds paragraphs of its own. An expanded use site is spliced into
    // its sibling run with no wrapper carrying the role (`flm-core`'s
    // `an_expanded_block_role_definition_still_splices_flat`), so the HTML is
    // that of the paragraphs the body and the template give.
    let note = definition(
        Definition::environment("note", Vec::new()).content("\\textbf{Note.} #{body}").block_role(BlockRole::Block),
    );
    let document = "Before.\n\n\\begin{note}Mind the gap.\n\nAnd the step.\\end{note}\n\nAfter.";
    let (html, diagnostics) = html_of(&registry(vec![(note, None)]), document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_text_golden(golden("block.html"), &html);
}

// --- hooks -----------------------------------------------------------------------------------

#[test]
fn a_hook_stages_computed_characters() {
    let registry = registry(vec![(shout_definition(), Some(shout_hook()))]);
    let (text, diagnostics) = text_of(&registry, "Say \\shout{hey there} and \\shout{you}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_text_golden(golden("shout.txt"), &text);
}

#[test]
fn a_hook_recovers_the_use_site_with_its_own_diagnostic() {
    let registry = registry(vec![(shout_definition(), Some(shout_hook()))]);
    let run = common::run(&registry, text::WORKFLOW, "Say \\shout{\\emph{hey}}.");
    let diagnostics = common::diagnostics(&run);
    assert_eq!(diagnostics, ["wc.shout.not-characters: `\\shout` must be given plain text"]);
    assert_eq!(
        data_of(&run, "wc.shout.not-characters"),
        Some(DiagnosticValue::Map(vec![("nodes".to_string(), DiagnosticValue::Int(1))]))
    );
    let text = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    assert!(text.contains("\\shout{\\emph{hey}}"), "{text}");
}

#[test]
fn a_hook_instantiates_its_definition_s_own_template() {
    let document = "Say \\shout{hey} and \\shout{\\emph{you}}.";
    let (without, _) = text_of(&registry(vec![(shout_definition(), None)]), document);
    let shout = shout_definition();
    let hook = instantiating_hook(shout.clone());
    let (with, diagnostics) = text_of(&registry(vec![(shout, Some(hook))]), document);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(with, without);
    assert_eq!(with, "Say <hey> and <you>.\n");
}

/// Plan A, A37: a hook that holds no definition of its own instantiates the
/// definition its element was registered with, which its context answers;
/// an element declared in the data form has none.
#[test]
fn a_hook_instantiates_the_registered_definition_through_its_context() {
    let hook = expand(ExpandPolicy::PerPlan, |node, cx, xp| {
        let definition = cx.definition().ok_or_else(|| ExtError::Handler {
            detail: "the element must have been declared from a definition".to_string(),
            data: Value::Null,
        })?;
        assert_eq!(definition.definition().name.as_str(), "loud");
        instantiate_own(&definition, node, cx, xp)
    });
    let loud = definition(Definition::macro_("loud", vec![ArgDef::mandatory("text")]).content("[#1]"));
    let (text, diagnostics) = text_of(&registry(vec![(loud, Some(hook))]), "Say \\loud{x}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Say [x].\n");

    let double = Double::new("wc").feature("badges", |b| {
        let hook = expand(ExpandPolicy::Always, |_, cx, _| {
            let answer = if cx.definition().is_none() { "none" } else { "some" };
            Ok(Expansion::Emit(vec![cx.stage_chars(answer)?]))
        });
        let element = ElementSpec::new("wc.badge").with(Aspect::Expand(hook))?;
        let expanded = Definition::macro_("badge", Vec::new()).expand(ExpandPolicy::Always);
        b.lowering_rule(expand_in_tree_rule_for("wc.badge", &expanded))?;
        b.element(ElementDef::new(b.feature(), ["badge"], SyntaxDef::Macro { args: Vec::new() }, element))
    });
    let (text, diagnostics) = text_of(&registry_with([double.package()]), "A \\badge{} here.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A none here.\n");
}

#[test]
fn a_hook_that_keeps_its_node_reports_through_diagnose() {
    let hook = expand(ExpandPolicy::PerPlan, |_, cx, xp| {
        xp.diagnose(Diagnostic::note("wc.shout.seen", "this use was seen"))?;
        Ok(Expansion::Emit(vec![cx.stage_chars("seen")?]))
    });
    let registry = registry(vec![(shout_definition(), Some(hook))]);
    let document = "Say \\shout{hey}.";
    let run = common::run(&registry, text::WORKFLOW, document);
    let notes: Vec<_> = run.diagnostics().iter().filter(|d| d.identifier() == "wc.shout.seen").collect();
    assert_eq!(notes.len(), 1, "{:?}", common::diagnostics(&run));
    let start = document.find("\\shout").expect("the use");
    assert_eq!(notes[0].span().range(), start..document.len() - 1);
    assert_eq!(notes[0].severity(), flm_core::techy::error::Severity::Note);
    let text = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    assert_eq!(text, "Say seen.\n");
}

#[test]
fn a_diagnostic_outside_the_package_is_the_hook_s_failure() {
    let hook = expand(ExpandPolicy::PerPlan, |_, _, xp| {
        xp.diagnose(Diagnostic::note("other.seen", "seen"))?;
        Ok(Expansion::Default)
    });
    let registry = registry(vec![(shout_definition(), Some(hook))]);
    let (text, diagnostics) = text_of(&registry, "Say \\shout{hey}.");
    assert_eq!(
        diagnostics,
        ["flm.ext.handler-failed: the expansion hook of `wc.macro.shout` failed: the diagnostic `other.seen` declared by the package `_` of the extension `wc` must begin with `wc.`"]
    );
    assert!(text.contains("\\shout{hey}"), "{text}");
}

#[test]
fn a_failing_expand_handler_recovers_the_use_site_with_handler_failed() {
    let hook = expand(ExpandPolicy::PerPlan, |_, _, _| {
        Err(ExtError::Handler { detail: "TypeError: x is undefined".to_string(), data: json!({ "line": 3 }) })
    });
    let registry = registry(vec![(shout_definition(), Some(hook))]);
    let run = common::run(&registry, text::WORKFLOW, "Say \\shout{hey}.");
    assert_eq!(
        common::diagnostics(&run),
        ["flm.ext.handler-failed: the expansion hook of `wc.macro.shout` failed: TypeError: x is undefined"]
    );
    let data = data_of(&run, "flm.ext.handler-failed");
    assert!(format!("{data:?}").contains("\"line\""), "{data:?}");
    let text = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    assert_eq!(text, "Say \\shout{hey}.\n");
}

#[test]
fn instantiate_refuses_a_site_the_call_was_not_given() {
    let shout = shout_definition();
    let held = shout.clone();
    let hook = expand(ExpandPolicy::PerPlan, move |node, cx, xp| {
        let variant = held.variant_for_use(node)?;
        let template = variant.template();
        let bindings = DefinitionBindings::new(node, &*variant);
        let parent = node.parent().ok_or_else(|| ExtError::Handler { detail: "no parent".into(), data: Value::Null })?;
        Ok(Expansion::Emit(xp.instantiate(cx, &*parent, &*template, &*bindings)?))
    });
    let (text, diagnostics) = text_of(&registry(vec![(shout, Some(hook))]), "Say \\shout{hey}.");
    assert_eq!(
        diagnostics,
        ["flm.ext.handler-failed: the expansion hook of `wc.macro.shout` failed: `instantiate`: `site` must be the node this hook call was given"]
    );
    assert_eq!(text, "Say \\shout{hey}.\n");
}

#[test]
fn an_expansion_naming_a_staged_node_twice_is_the_hook_s_failure() {
    let hook = expand(ExpandPolicy::PerPlan, |_, cx, _| {
        let staged = cx.stage_chars("x")?;
        Ok(Expansion::Emit(vec![staged, staged]))
    });
    let (text, diagnostics) = text_of(&registry(vec![(shout_definition(), Some(hook))]), "Say \\shout{hey}.");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].contains("a node may appear only once in what the hook returns"), "{diagnostics:?}");
    assert_eq!(text, "Say \\shout{hey}.\n");
}

#[test]
fn an_expansion_naming_a_node_another_call_staged_is_the_hook_s_failure() {
    // The hook keeps the node it staged at the first use site and answers it
    // again at the second.
    let kept: Arc<Mutex<Option<Staged>>> = Arc::new(Mutex::new(None));
    let hook = expand(ExpandPolicy::PerPlan, move |_, cx, _| {
        let earlier = kept.lock().ok().and_then(|kept| *kept);
        match earlier {
            Some(staged) => Ok(Expansion::Emit(vec![staged])),
            None => {
                let staged = cx.stage_chars("x")?;
                if let Ok(mut kept) = kept.lock() {
                    *kept = Some(staged);
                }
                Ok(Expansion::Emit(vec![staged]))
            }
        }
    });
    let registry = registry(vec![(shout_definition(), Some(hook))]);
    let (text, diagnostics) = text_of(&registry, "Say \\shout{hey} and \\shout{you}.");
    assert_eq!(
        diagnostics,
        ["flm.ext.handler-failed: the expansion hook of `wc.macro.shout` failed: `expand`: each node the hook returns must have been created during this same call"]
    );
    assert_eq!(text, "Say x and \\shout{you}.\n");
}

/// A Rust feature `rs` declaring `\\boom`, whose `flm-core` hook always fails.
struct Boom;

/// `\\boom`'s hook.
struct BoomHook;

impl CoreExpand for BoomHook {
    fn expand(
        &self,
        _: NodeRef<'_, Flm>,
        _: &mut CoreExpandCx<'_>,
        _: &mut dyn CoreExpander,
    ) -> Result<CoreExpansion, ExpandError> {
        Err(ExpandError::Hook { detail: "boom".to_string() })
    }
}

impl FlmFeature for Boom {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("rs", "A feature whose macro's hook always fails")
    }

    fn register(&self, b: &mut CoreFeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        let boom = CoreUserDefinition::new(Definition::macro_("boom", Vec::new()).expand(ExpandPolicy::Always));
        let element = element_name_under("rs", boom.definition());
        b.element(boom.element_def_with(owner, element.clone(), Arc::new(BoomHook)))?;
        b.lowering_rule(core_expand_in_tree_rule_for(element, boom.definition()))?;
        Ok(())
    }
}

#[test]
fn a_failure_of_the_pass_inside_instantiate_ends_the_pass_though_the_hook_recovers() {
    // `\\outer`'s template uses `\\boom`, whose hook fails while `\\outer`'s
    // hook instantiates; the hook catches the error and recovers the use site.
    let outer = definition(Definition::macro_("outer", Vec::new()).content("[\\boom]"));
    let held = outer.clone();
    let hook = expand(ExpandPolicy::PerPlan, move |node, cx, xp| match instantiate_own(&held, node, cx, xp) {
        Err(ExtError::Core { kind, .. }) if kind == "expand" => {
            Ok(Expansion::Emit(xp.recover(cx, node, Diagnostic::error("wc.outer.failed", "the template failed"))?))
        }
        other => other,
    });
    let registry = registry_with([
        defining("wc", vec![(outer, Some(hook))]).package(),
        PackageEntry::new("rs").feature(Arc::new(Boom)),
    ]);
    let (_, diagnostics) = text_of(&registry, "Go \\outer.");
    // The hook's recovery reports its diagnostic; the pass then ends with the
    // failure met inside `instantiate`.
    assert_eq!(diagnostics.len(), 2, "{diagnostics:?}");
    assert_eq!(diagnostics[0], "wc.outer.failed: the template failed");
    assert!(diagnostics[1].starts_with("flm.userdefs.expansion-failed"), "{diagnostics:?}");
    assert!(diagnostics[1].ends_with("the expansion code of a definition failed: boom"), "{diagnostics:?}");
}

#[test]
fn an_expansion_left_as_it_is_keeps_the_node_and_the_pass_continues_into_it() {
    let keep_hook = expand(ExpandPolicy::PerPlan, |_, _, _| Ok(Expansion::Default));
    let keep = definition(Definition::macro_("keep", vec![ArgDef::mandatory("text")]).content("#1"));
    let registry = registry(vec![(keep, Some(keep_hook)), (shout_definition(), Some(shout_hook()))]);
    let (trees, diagnostics) = expanded_trees(&registry, "\\keep{\\shout{a}}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let tree = trees.first().expect("one tree");
    let root = NodeFacade::shared(Arc::clone(tree), tree.root().id());
    let nodes = root.descendants();
    let names: Vec<String> = nodes.iter().map(|node| node.element_name()).collect();
    assert!(names.iter().any(|name| name == "wc.macro.keep"), "{names:?}");
    assert!(!names.iter().any(|name| name == "wc.macro.shout"), "{names:?}");
    let chars: Vec<String> = nodes.iter().filter_map(|node| node.chars()).collect();
    assert_eq!(chars, ["A"]);
}

// --- depth, limit, nesting ------------------------------------------------------------------

/// What the hooks saw: their name, the depth and the limit.
type Seen = Arc<Mutex<Vec<(String, usize, usize)>>>;

#[test]
fn the_depth_and_the_limit_are_those_of_the_pass() {
    let seen: Seen = Arc::new(Mutex::new(Vec::new()));
    // `\inner{…}` stages its argument in upper case; `\outer{…}` instantiates
    // its own template, `[\inner{#1}]`, so that the pass calls `\inner`'s hook
    // while `\outer`'s `instantiate` runs.
    let inner = definition(Definition::macro_("inner", vec![ArgDef::mandatory("text")]).content("#1"));
    let outer = definition(Definition::macro_("outer", vec![ArgDef::mandatory("text")]).content("[\\inner{#1}]"));
    let record = |name: &'static str, seen: &Seen, xp: &dyn Expander| {
        if let Ok(mut seen) = seen.lock() {
            seen.push((name.to_string(), xp.depth(), xp.recursion_limit()));
        }
    };
    let inner_seen = Arc::clone(&seen);
    let inner_hook = expand(ExpandPolicy::PerPlan, move |node, cx, xp| {
        record("inner", &inner_seen, xp);
        let nodes = node.argument_content_nodes_named("text")?.unwrap_or_default();
        let text = chars_text(&nodes).unwrap_or_default();
        Ok(Expansion::Emit(vec![cx.stage_chars(&text.to_uppercase())?]))
    });
    let outer_seen = Arc::clone(&seen);
    let held = outer.clone();
    let outer_hook = expand(ExpandPolicy::PerPlan, move |node, cx, xp| {
        record("outer", &outer_seen, xp);
        instantiate_own(&held, node, cx, xp)
    });
    let registry = registry(vec![(inner, Some(inner_hook)), (outer, Some(outer_hook))]);
    let (text, diagnostics) = text_of(&registry, "A \\inner{b} and \\outer{c}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A B and [C].\n");
    let seen = seen.lock().map(|seen| seen.clone()).unwrap_or_default();
    let limit = DEFAULT_RECURSION_LIMIT;
    assert_eq!(
        seen,
        [("inner".to_string(), 0, limit), ("outer".to_string(), 0, limit), ("inner".to_string(), 1, limit)]
    );
}

#[test]
fn a_hook_instantiating_itself_without_end_stops_at_the_limit() {
    let again = definition(Definition::macro_("again", Vec::new()).content("\\again"));
    let hook = instantiating_hook(again.clone());
    let (_, diagnostics) = text_of(&registry(vec![(again, Some(hook))]), "Go \\again.");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].starts_with("flm.userdefs.recursion-limit"), "{diagnostics:?}");
}

// --- plan A, A32: the follow-ups of WP-1 -----------------------------------------------------

#[test]
fn element_value_answers_a_scripted_element_s_value() {
    let value = json!({ "color": "red", "level": 2 });
    let stored = value.clone();
    let double = Double::new("wc").feature("badges", move |b| {
        let hook = expand(ExpandPolicy::Always, |node, cx, _| {
            let color = node.element_value().and_then(|value| value["color"].as_str().map(str::to_string));
            Ok(Expansion::Emit(vec![cx.stage_chars(&format!("[{}]", color.unwrap_or_default()))?]))
        });
        let element = ElementSpec::new("wc.badge").with_value(stored.clone()).with(Aspect::Expand(hook))?;
        // The element is always expanded, which the rule tells the plan.
        let expanded = Definition::macro_("badge", Vec::new()).expand(ExpandPolicy::Always);
        b.lowering_rule(expand_in_tree_rule_for("wc.badge", &expanded))?;
        b.element(ElementDef::new(b.feature(), ["badge"], SyntaxDef::Macro { args: Vec::new() }, element))
    });
    let registry = registry_with([double.package()]);
    // Read on the parsed node.
    let source = Arc::new(flm_core::lang::FlmSource::new("A (\\badge) here.").with_origin(flm_core::lang::FlmOrigin::none()));
    let fragment = flm_core::lang::parse_fragment(
        registry.lang(),
        source,
        flm_core::model::doc::FragmentLayout::default(),
        &flm_core::lang::ParseOptions::default(),
    )
    .expect("a parse");
    let root = NodeFacade::shared(Arc::clone(fragment.tree()), fragment.tree().root().id());
    let badges: Vec<Value> = root.descendants().iter().filter_map(|node| node.element_value()).collect();
    assert_eq!(badges, [value]);
    // Read by the element's hook.
    let (text, diagnostics) = text_of(&registry, "A (\\badge) here.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A ([red]) here.\n");
}

#[test]
fn the_lowering_readers_refuse_content_holding_a_placeholder() {
    let registry = registry_with([]);
    let definition =
        CoreUserDefinition::new(Definition::macro_("x", vec![ArgDef::mandatory("a")]).content("ab #1 cd"));
    assert!(definition.compile(registry.lang()).is_ok());
    let tree = definition
        .compiled()
        .and_then(|compiled| compiled.variant(Mode::Text))
        .map(|variant| Arc::clone(variant.template().tree()))
        .expect("the template in running text");
    let root = NodeFacade::shared(Arc::clone(&tree), tree.root().id());
    let content = root.children();
    let error = chars_at_lowering(&content).expect_err("a placeholder");
    assert!(matches!(&error, ExtError::Core { kind, .. } if kind == "lower"), "{error:?}");
    let error = spelling_at_lowering(&content).expect_err("a placeholder");
    assert!(matches!(&error, ExtError::Core { kind, .. } if kind == "lower"), "{error:?}");
    // The characters around the placeholder alone are characters.
    let first: Vec<_> = content.into_iter().take(1).collect();
    assert_eq!(chars_at_lowering(&first), Ok(Ok("ab ".to_string())));
}

// --- E4 (section 7.5): a definition with neither a template nor a hook -------------------------

#[test]
fn a_definition_with_neither_a_template_nor_a_hook_is_the_adapter_s_error_naming_the_extension() {
    use flm_ext::api::lang::rewrites::userdefs::{Definition, UserDefinition};
    let package = common::double::defining("ex", vec![(UserDefinition::new(Definition::macro_("bare", Vec::new())), None)]).package();
    let registry = common::registry_with([package]);
    let found: Vec<String> = registry.build_diagnostics().iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect();
    assert_eq!(
        found,
        ["flm.ext.definition-needs-template: the definition `\\bare` declared by the feature `defs` of the package `_` of the \
          extension `ex` must have a template or an expansion hook"]
    );
}

#[test]
fn a_definition_needing_a_template_is_spelled_by_its_kind() {
    // S11: the definition is spelled as `flm-core` displays it.
    use flm_ext::api::lang::rewrites::userdefs::{Definition, UserDefinition};
    let package =
        common::double::defining("ex", vec![(UserDefinition::new(Definition::environment("barenv", Vec::new())), None)]).package();
    let registry = common::registry_with([package]);
    let found: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert_eq!(
        found,
        ["the definition `\\begin{barenv}` declared by the feature `defs` of the package `_` of the extension `ex` must have a \
          template or an expansion hook"]
    );
}

// --- a template compiled at the use site (plan E4, section 7.2) ------------------------------------

/// A text-mode template of `text` for `\shout`, with the extra placeholder
/// `word`.
fn use_site_spec(text: String) -> flm_ext::api::lang::rewrites::userdefs::TemplateSpec {
    flm_ext::api::lang::rewrites::userdefs::TemplateSpec {
        text,
        mode: Mode::Text,
        vocabulary: flm_ext::api::lang::rewrites::userdefs::PlaceholderVocabulary::new(0).extra("word", false),
        definition: "\\shout".to_string(),
        label: "expansion of \\shout".to_string(),
        origin_label: "the computed template of \\shout".to_string(),
        user_definition: false,
        offset: 0,
    }
}

#[test]
fn a_hook_compiles_a_template_at_the_use_site_and_instantiates_it_with_value_bindings() {
    use flm_ext::api::lang::rewrites::userdefs::{compile_template_with, instantiate, TemplateLanguage, ValueBindings};
    // The template `<#{word}>`, and the word, upper-cased, as a template of
    // its own instantiated with no value: its root node is the value.
    let hook = expand(ExpandPolicy::PerPlan, |node, cx, xp| {
        let nodes = node.argument_content_nodes_named("text")?.unwrap_or_default();
        let word = chars_text(&nodes).unwrap_or_default().to_uppercase();
        let (template, word) = {
            let lang = xp.lang();
            let language = TemplateLanguage::new(&*lang)?;
            (
                compile_template_with(&use_site_spec("<#{word}>".to_string()), &*lang, &language)?,
                compile_template_with(&use_site_spec(word), &*lang, &language)?,
            )
        };
        let word = instantiate(node, &word, &ValueBindings::new())?;
        let bindings = ValueBindings::new().with("word", &*word)?;
        Ok(Expansion::Emit(xp.instantiate(cx, node, &template, &bindings)?))
    });
    let registry = registry(vec![(shout_definition(), Some(hook))]);
    let (text, diagnostics) = text_of(&registry, "A \\shout{hey} and \\shout{you}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A <HEY> and <YOU>.\n");
}

#[test]
fn a_hook_compiles_a_template_at_the_use_site_through_the_dispatcher() {
    use common::json_double::{module, JsonDouble};
    use flm_ext::adapter::handles::Entry;
    let definition = shout_definition();
    let double = JsonDouble::new("wc").feature("defs", move |js, b| {
        let owner = js.call(b, "feature", json!([]))?;
        let hook = js.lend_expand("per-plan", |js, node, cx, xp| {
            let userdefs = module("lang/rewrites/userdefs");
            let spec = |text: &str| {
                json!({
                    "text": text, "mode": "text",
                    "vocabulary": { "arity": 0, "extras": [{ "name": "word", "substitution": false }] },
                    "definition": "\\shout", "label": "expansion of \\shout",
                    "origin_label": "the computed template of \\shout",
                })
            };
            let lang = js.call(xp, "lang", json!([]))?;
            let language = js.call(&userdefs, "TemplateLanguage::new", json!([lang]))?;
            let template = js.call(&userdefs, "compile_template_with", json!([spec("<#{word}>"), lang, language]))?;
            let word = js.call(&userdefs, "compile_template_with", json!([spec("WORD"), lang, language]))?;
            let empty = js.call(&userdefs, "ValueBindings::new", json!([]))?;
            let word = js.call(&userdefs, "instantiate", json!([node, word, empty]))?;
            let bindings = js.call(&empty, "with", json!(["word", word]))?;
            // The template passed as its durable token.
            let staged = js.call(xp, "instantiate", json!([cx, node, template, bindings]))?;
            Ok(json!({ "emit": staged }))
        })?;
        let ud = js.lend(Entry::UserDefinition(definition.clone()));
        let def = js.call(&ud, "element_def_with", json!([owner, "wc.macro.shout", hook]))?;
        js.call(b, "element", json!([def]))?;
        let helpers = module("stdcore/features/userdefs");
        let rule = js.call(&helpers, "expand_in_tree_rule_for", json!(["wc.macro.shout", { "kind": "macro", "name": "shout", "args": "{" }]))?;
        js.call(b, "lowering_rule", json!([rule]))?;
        Ok(())
    });
    let registry = registry_with([double.package()]);
    let (text, diagnostics) = text_of(&registry, "A \\shout{hey}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A <WORD>.\n");
}
