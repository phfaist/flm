//! Building a fragment over a tree obtained elsewhere
//! (`flm_core::model::doc::Fragment::new`): the fragment records the tree, the
//! source it is to report against, the diagnostics handed with it and the
//! layout chosen for its root region — none of which it recomputes.

mod common;

use std::sync::Arc;

use flm_core::lang::FlmDiagnostics;
use flm_core::model::doc::{Fragment, FragmentLayout};
use flm_core::model::element::element_of;

use common::{lang, macro_def, parse, source};

// Ported from Python FLM's test/test_flmfragment.py::TestFLMFragmentInitWithNodeList::test_init_with_preparsed_nodes.
#[test]
fn a_fragment_built_over_a_parsed_tree_carries_that_tree() {
    let lang = lang(vec![macro_def("t", "emph", 1)]);
    let parsed = parse(&lang, r"Hello \emph{world}!");
    let built = Fragment::new(
        Arc::clone(parsed.source()),
        Arc::clone(parsed.tree()),
        FlmDiagnostics::new(),
        FragmentLayout::default(),
    );
    assert!(Arc::ptr_eq(built.tree(), parsed.tree()));
    let names: Vec<String> = built
        .tree()
        .root()
        .descendants()
        .filter(|node| node.is_callable())
        .map(|node| element_of(&node).name.to_string())
        .collect();
    assert_eq!(names, ["t.emph"]);
}

// Ported from Python FLM's test/test_flmfragment.py::TestFLMFragmentInitWithNodeList::test_init_with_preparsed_nodes_custom_flm_text.
#[test]
fn a_fragment_built_over_a_parsed_tree_reports_against_the_source_it_is_given() {
    let lang = lang(Vec::new());
    let parsed = parse(&lang, "a b");
    let own = source("the text this fragment stands for");
    let built = Fragment::new(
        Arc::clone(&own),
        Arc::clone(parsed.tree()),
        FlmDiagnostics::new(),
        FragmentLayout::Blocks,
    );
    assert!(Arc::ptr_eq(built.source(), &own));
    assert_eq!(built.source().content(), "the text this fragment stands for");
    assert_eq!(built.layout(), FragmentLayout::Blocks);
    assert!(built.diags().is_empty());
}
