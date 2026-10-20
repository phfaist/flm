//! The spellings the `literals` feature declares, written inside another
//! feature's markup: an escaped ampersand in a heading's title, a non-breaking
//! space in front of a reference and in a heading's title, and the escaped
//! space that keeps a word apart from a user macro whose invocation swallowed
//! the space after it.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::stdcore::backends::pres::{self, HeadingPayload, payload_of};
use flm_core::stdcore::backends::{lowering, text};
use flm_core::stdcore::check_feature;
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::userdefs::{Userdefs, UserdefsConfig};
use flm_core::stdcore::features::{headings, index, refpoints, literals, refs, textstyles};

use common::lowering::{lowered, text as text_of};
use common::stdcore::{identifiers, registry_over, run_workflow};

/// A registry over the `literals` feature and the features whose markup the
/// tests write it inside: `headings` with the `refs` and `index` features it
/// needs, `textstyles`, `userdefs` carrying `definitions`, the lowering
/// machinery, the text backend and the `blocks` feature.
fn registry(definitions: Vec<Definition>) -> Arc<FlmRegistry> {
    registry_over(vec![
        lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        check_feature(),
        Arc::new(Blocks::default()),
        textstyles::feature(),
        literals::feature(),
        index::feature(),
        refpoints::feature(),
        refs::feature(),
        headings::feature(),
        Arc::new(Userdefs::new(UserdefsConfig::new().definitions(definitions))),
    ])
}

/// The anchors the headings of a run carry, in document order.
fn anchors(run: &flm_core::core::run::Run) -> Vec<Option<String>> {
    lowered(run, TreeId::tree(0))
        .tree
        .root()
        .descendants()
        .filter(|node| element_of(node).name == pres::BLOCK_HEADING)
        .map(|node| {
            payload_of::<HeadingPayload>(&node).expect("a heading payload").anchor.clone()
        })
        .collect()
}

// Ported from Python FLM's test/test_feature_headings.py::TestFeatureHeadings::test_special_characters_in_heading.
#[test]
fn an_escaped_ampersand_in_a_title_is_the_character_in_the_output_and_in_the_anchor() {
    let document = "\\section{Hello \\& World}\n\nText.\n";
    let run = run_workflow(&registry(Vec::new()), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(anchors(&run)[0].as_deref(), Some("sec--Hello_World~9a6b33"));
    assert_eq!(text_of(&run), "Hello & World\n=============\n\nText.\n");
}

// Ported from Python FLM's test/test_feature_refs.py::TestRefEdgeCases::test_ref_with_tilde_nonbreaking_space.
#[test]
fn a_non_breaking_space_before_a_reference_reaches_the_output_as_the_character() {
    let document = "\\section{Methods}\\label{sec:methods}\n\nSee~\\ref{sec:methods}.\n";
    let run = run_workflow(&registry(Vec::new()), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(text_of(&run), "Methods\n=======\n\nSee\u{a0}Methods.\n");
}

// Ported from Python FLM's test/test_feature_substmacros.py::TestFeatureSubstMacros::test_simple_wflm.
#[test]
fn the_escaped_space_separates_a_user_macros_expansion_from_the_word_after_it() {
    let mymacro = Definition::macro_("mymacro", Vec::<ArgDef>::new())
        .text("[my \\textit{macro} content]");
    let document = "Test \\textbf{macro content: \\emph{a \\mymacro\\ b}}.\n";
    let run =
        run_workflow(&registry(vec![mymacro]), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(text_of(&run), "Test macro content: a [my macro content] b.\n");
}

// Ported from Python FLM's test/test_feature_baseformatting.py::TestBaseFormattingText::test_tilde.
#[test]
fn a_non_breaking_space_in_a_title_is_one_column_of_the_rule_under_it() {
    let document = "\\section{a~~b}\n\nText.\n";
    let run = run_workflow(&registry(Vec::new()), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    // The title is four columns wide — two letters and two non-breaking spaces,
    // which are characters of the title and not the gaps between its words — so
    // the rule under it is four characters long.
    assert_eq!(text_of(&run), "a\u{a0}\u{a0}b\n====\n\nText.\n");
}
