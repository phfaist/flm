//! The wired rewrite features through the runner
//! (`flm_core::stdcore::features::blocks` and
//! `flm_core::stdcore::features::userdefs`): a plan demanding block structure
//! expands and then structures; a kept plan structures around the use sites;
//! the raw and kept alternatives freeze the parsed tree itself; the two
//! expansion sets; the published trees tables; the registry-bound configuration
//! reaching the passes; the recovered conditions reaching the run's
//! diagnostics; and parallel and sequential runs agreeing.

mod common;

use std::sync::Arc;

use flm_core::model::addr::TreeId;
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::core::registry::FlmFeature;
use flm_core::core::run::{Run, RunOptions};
use flm_core::core::stage::TreesTable;
use flm_core::dump::expect;
use flm_core::lang::GuardPolicy;
use flm_core::lang::rewrites::blocks::BlocksConfig;
use flm_core::lang::rewrites::userdefs::{Definition, ExpandPolicy, RecursionLimit};
use flm_core::stdcore::features::blocks::{Blocks, STRUCTURED};
use flm_core::stdcore::features::userdefs::{
    ALWAYS_EXPANDED, EXPANDED, Userdefs, UserdefsConfig,
};

use common::pipeline::{plan, run};
use common::wiring::{
    child_elements, count_element, doc, frozen, paragraphs, registry, registry_with,
    registry_with_guard, run_blocks, text_under,
};

/// One `per-plan` macro `\foo` with the given text-mode template.
fn foo(template: &str) -> Vec<Definition> {
    vec![Definition::macro_("foo", Vec::new()).text(template)]
}

/// `\a`, expanded always, and `\p`, expanded per plan.
fn policies() -> Vec<Definition> {
    vec![
        Definition::macro_("a", Vec::new()).text("A").expand(ExpandPolicy::Always),
        Definition::macro_("p", Vec::new()).text("P"),
    ]
}

// --- the alternatives, run -----------------------------------------------------------------------

#[test]
fn a_structured_plan_expands_definitions_before_structuring() {
    let run = run_blocks(&registry(foo("hello")), r"say \foo.", &["flm.blocks.structured"]);
    assert_eq!(paragraphs(frozen(&run)), ["say hello."]);
    assert_eq!(count_element(frozen(&run), "flm.userdefs.macro.foo"), 0);
}

#[test]
fn a_template_paragraph_break_splits_the_enclosing_paragraph() {
    let run = run_blocks(&registry(foo("a\n\nb")), r"x \foo.", &["flm.blocks.structured"]);
    assert_eq!(paragraphs(frozen(&run)), ["x a", "b."]);
}

#[test]
fn a_kept_plan_structures_around_the_use_site() {
    let demands = ["flm.userdefs.kept", "flm.blocks.structured"];
    let run = run_blocks(&registry(foo("hello")), r"say \foo.", &demands);
    let tree = frozen(&run);
    assert_eq!(child_elements(&tree.root()), ["flm.paragraph"]);
    let paragraph = tree.root().child(0).expect("a paragraph");
    assert_eq!(
        child_elements(&paragraph),
        ["flm.text", "flm.userdefs.macro.foo", "flm.text"]
    );
}

#[test]
fn raw_and_kept_freeze_the_parsed_tree_unchanged() {
    let registry = registry(foo("hello"));
    let doc = doc(&registry, FragmentLayout::Blocks, &[r"say \foo."]);
    let plan = plan(&registry, &["flm.blocks.raw", "flm.userdefs.kept"]);
    let run = run(&doc, &plan, &RunOptions::new());
    let parsed = doc.content().tree(TreeId::tree(0)).expect("placed");
    assert!(Arc::ptr_eq(frozen(&run), parsed));
}

// --- the two expansion sets ----------------------------------------------------------------------

#[test]
fn the_always_stage_expands_only_always_definitions() {
    let run = run_blocks(&registry(policies()), r"\a\p", &["flm.userdefs.always-expanded"]);
    let tree = frozen(&run);
    assert_eq!(child_elements(&tree.root()), ["flm.text", "flm.userdefs.macro.p"]);
    assert_eq!(text_under(&tree.root()), "A");
}

#[test]
fn the_per_plan_stage_leaves_always_definitions_to_their_own_stage() {
    let run = run_blocks(&registry(policies()), r"\a\p", &["flm.userdefs.expanded"]);
    let tree = frozen(&run);
    assert_eq!(child_elements(&tree.root()), ["flm.userdefs.macro.a", "flm.text"]);
    assert_eq!(text_under(&tree.root()), "P");
}

#[test]
fn a_kept_plan_with_always_expansion_runs_the_always_stage_before_structure() {
    let demands = ["flm.userdefs.kept", "flm.userdefs.always-expanded", "flm.blocks.structured"];
    let run = run_blocks(&registry(policies()), r"\a\p", &demands);
    let paragraph = frozen(&run).root().child(0).expect("a paragraph");
    assert_eq!(child_elements(&paragraph), ["flm.text", "flm.userdefs.macro.p"]);
    assert_eq!(text_under(&paragraph), "A");
}

#[test]
fn both_stages_active_expand_every_definition_in_order() {
    let demands = ["flm.userdefs.always-expanded", "flm.blocks.structured"];
    let run = run_blocks(&registry(policies()), r"\a\p", &demands);
    assert_eq!(paragraphs(frozen(&run)), ["AP"]);
}

// --- the published tables ------------------------------------------------------------------------

#[test]
fn the_trees_tables_are_published_under_their_capabilities() {
    let demands = ["flm.userdefs.always-expanded", "flm.blocks.structured"];
    let run = run_blocks(&registry(foo("hello")), r"say \foo.", &demands);
    let first = TreeId::tree(0);
    let always: &TreesTable = run.table(ALWAYS_EXPANDED).unwrap().expect("published");
    let expanded: &TreesTable = run.table(EXPANDED).unwrap().expect("published");
    let structured: &TreesTable = run.table(STRUCTURED).unwrap().expect("published");
    assert!(always.trees.contains_key(&first));
    assert!(expanded.trees.contains_key(&first));
    assert!(Arc::ptr_eq(&structured.trees[&first], frozen(&run)));
}

// --- the configuration reaches the passes ------------------------------------------------------

#[test]
fn the_registry_bound_text_options_reach_the_structure_pass() {
    let plain = registry_with(Blocks::new(BlocksConfig::no_typography()), UserdefsConfig::new());
    let run = run_blocks(&plain, "a--b", &["flm.blocks.structured"]);
    assert_eq!(paragraphs(frozen(&run)), ["a--b"]);
    let typeset = registry_with(Blocks::default(), UserdefsConfig::new());
    let run = run_blocks(&typeset, "a--b", &["flm.blocks.structured"]);
    assert_eq!(paragraphs(frozen(&run)), ["a–b"]);
}

#[test]
fn the_configured_depth_limit_reaches_the_expand_pass() {
    let recursive = Definition::macro_("foo", Vec::new()).text(r"\foo");
    let config = UserdefsConfig::new().definition(recursive).recursion_limit(3);
    let registry = registry_with(Blocks::default(), config);
    let run = run_blocks(&registry, r"\foo", &["flm.userdefs.expanded"]);
    let diagnostic = expect(run.diagnostics().iter(), "flm.userdefs.recursion-limit").once();
    let condition = diagnostic.data().downcast_ref::<RecursionLimit>().expect("typed");
    assert_eq!(condition.name, r"\foo");
    assert_eq!(condition.limit, 3);
}

// --- recovered conditions reach the run --------------------------------------------------------

#[test]
fn a_use_site_outside_the_definitions_modes_is_an_unknown_macro_through_the_run() {
    // A definition is visible in the parsing modes it applies in and nowhere
    // else, so a math-only definition used in text mode is a name the registry
    // does not define: the parse reports it, and the text stands as written.
    let math_only = vec![Definition::macro_("foo", Vec::new()).math("x")];
    let run = run_blocks(&registry(math_only), r"say \foo.", &["flm.blocks.structured"]);
    expect(run.diagnostics().iter(), "core.specs.unresolvable-command").at(r"\foo").once();
    // The use site stands in the tree as the unknown-macro element, which
    // carries no text of its own; a backend decides how to print it.
    assert_eq!(paragraphs(frozen(&run)), ["say ."]);
}

// --- the failure mapping ------------------------------------------------------------------------

/// A one-tree document nesting deeper than a depth limit of three, parsed
/// under the stack budget so that only the rewrites meet the limit.
fn nested_doc() -> Arc<Doc> {
    let lang = common::lang(Vec::new());
    let mut content = DocContent::new();
    content.place(Arc::new(common::parse(&lang, "{{{{{x}}}}}")));
    Arc::new(Doc::new(content))
}

#[test]
fn a_guard_refusal_in_the_structure_stage_leaves_the_tree_absent_with_its_condition() {
    let blocks: Arc<dyn FlmFeature> = Arc::new(Blocks::default());
    let registry = registry_with_guard(vec![blocks], GuardPolicy::DepthLimit(3));
    let plan = plan(&registry, &["flm.blocks.structured"]);
    let run = run(&nested_doc(), &plan, &RunOptions::new());
    assert!(run.frozen_tree(TreeId::tree(0)).is_none());
    expect(run.diagnostics().iter(), "flm.blocks.nesting-too-deep").once();
    let structured: &TreesTable = run.table(STRUCTURED).unwrap().expect("provided");
    assert!(structured.trees.is_empty());
}

#[test]
fn a_guard_refusal_in_the_expand_stage_leaves_the_tree_absent_with_its_condition() {
    let userdefs: Arc<dyn FlmFeature> = Arc::new(Userdefs::default());
    let registry = registry_with_guard(vec![userdefs], GuardPolicy::DepthLimit(3));
    let plan = plan(&registry, &["flm.userdefs.expanded"]);
    let run = run(&nested_doc(), &plan, &RunOptions::new());
    assert!(run.frozen_tree(TreeId::tree(0)).is_none());
    expect(run.diagnostics().iter(), "flm.blocks.nesting-too-deep").once();
    let expanded: &TreesTable = run.table(EXPANDED).unwrap().expect("provided");
    assert!(expanded.trees.is_empty());
}

// --- determinism ---------------------------------------------------------------------------------

#[test]
fn parallel_and_sequential_runs_freeze_identical_trees() {
    let registry = registry(foo("a\n\nb"));
    let texts = [r"x \foo.", r"\foo y", "plain", r"\foo\foo"];
    let doc = doc(&registry, FragmentLayout::Blocks, &texts);
    let plan = plan(&registry, &["flm.blocks.structured"]);
    let parallel = run(&doc, &plan, &RunOptions::new());
    let sequential = run(&doc, &plan, &RunOptions::sequential());
    let render = |run: &Run| -> Vec<(TreeId, Vec<String>)> {
        run.frozen_trees().map(|(id, tree)| (id, paragraphs(tree))).collect()
    };
    assert_eq!(render(&parallel), render(&sequential));
    assert_eq!(render(&parallel).len(), texts.len());
}
