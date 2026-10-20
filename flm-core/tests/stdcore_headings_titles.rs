//! What a heading's *title* contributes (`flm_core::stdcore::features::headings`
//! with `refs`): the anchor and the reference display are built from the
//! title's plain text and not from the markup the author wrote, and two
//! headings sharing a title both print in full while only the first keeps an
//! anchor.

mod common;

use std::sync::Arc;

use flm_core::lang::text::plain_text;
use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::stdcore::backends::pres::{self, HeadingPayload, payload_of};
use flm_core::stdcore::backends::{lowering, text};
use flm_core::stdcore::check_feature;
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::model::aspects::Label;
use flm_core::stdcore::features::refs::REF_TARGETS;
use flm_core::stdcore::features::{headings, index, refpoints, refs, textstyles};

use common::lowering::{lowered, text as text_of};
use common::stdcore::{identifiers, registry_over, run_workflow};

/// A registry over the `headings` feature, the `refs` and `index` features it
/// needs, the `textstyles` feature (a title's markup), the lowering machinery, the
/// text backend and the `blocks` feature.
fn registry() -> Arc<FlmRegistry> {
    registry_over(vec![
        lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        check_feature(),
        Arc::new(Blocks::default()),
        textstyles::feature(),
        index::feature(),
        refpoints::feature(),
        refs::feature(),
        headings::feature(),
    ])
}

/// The anchors the headings of `document` carry, in document order.
fn anchors(run: &flm_core::core::run::Run) -> Vec<Option<String>> {
    lowered(run, TreeId::tree(0))
        .tree
        .root()
        .descendants()
        .filter(|node| element_of(node).name == pres::BLOCK_HEADING)
        .map(|node| {
            payload_of::<HeadingPayload>(&node)
                .expect("a heading payload")
                .anchor
                .clone()
        })
        .collect()
}

// Ported from Python FLM's test/test_feature_headings.py::TestFeatureHeadings::test_section_rich_text.
#[test]
fn a_titles_plain_text_and_not_its_source_is_what_the_anchor_is_built_from() {
    let document = "\\section{Introduction to \\emph{FLM}}\n\nText.\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    // The title's source would slug as `Introduction_to_emph_FLM`.
    assert_eq!(anchors(&run)[0].as_deref(), Some("sec--Introduction_to_FLM"));
    assert_eq!(text_of(&run), "Introduction to FLM\n===================\n\nText.\n");
}

// Ported from Python FLM's test/test_feature_headings.py::TestFeatureHeadings::test_section_rich_text.
#[test]
fn a_reference_displays_the_titles_plain_text_and_not_its_source() {
    let document = "\\section{Introduction to \\emph{FLM}}\\label{sec:x}\n\n\
                    See \\ref{sec:x}.\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let targets = run.table(&REF_TARGETS).expect("declared").expect("written");
    let target = &targets.by_label[&Label::new("sec", "x")];
    assert_eq!(target.href, "#sec-x");
    let content = target.display.content.as_ref().expect("the point's content");
    assert_eq!(plain_text(content), "Introduction to FLM");
    assert_eq!(
        text_of(&run),
        "Introduction to FLM\n===================\n\nSee Introduction to FLM.\n"
    );
}

// Ported from Python FLM's test/test_feature_headings.py::TestFeatureHeadingsTextRenderer::test_duplicate_heading_text_unnumbered_text.
#[test]
fn two_headings_with_one_title_both_print_and_the_second_anchor_is_numbered() {
    let document = "\\section{Same}\n\nA.\n\n\\section{Same}\n\nB.\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(anchors(&run), [Some("sec--Same".to_string()), Some("sec--Same-2".to_string())]);
    assert_eq!(text_of(&run), "Same\n====\n\nA.\n\nSame\n====\n\nB.\n");
}
