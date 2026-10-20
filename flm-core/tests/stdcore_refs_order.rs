//! Where a reference may stand relative to the point it names
//! (`flm_core::stdcore::features::refs`): resolution is document-wide, so a
//! reference written before the heading it names resolves just as one written
//! after it does.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::stdcore::backends::{lowering, text};
use flm_core::stdcore::check_feature;
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::{headings, index, refpoints, refs};

use common::lowering::text as text_of;
use common::stdcore::{identifiers, registry_over, run_workflow};

/// A registry over the `refs` feature, the `headings` feature (its labeled
/// element), the `index` feature, the lowering machinery, the text backend and
/// the `blocks` feature.
fn registry() -> Arc<FlmRegistry> {
    registry_over(vec![
        lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        check_feature(),
        Arc::new(Blocks::default()),
        index::feature(),
        refpoints::feature(),
        refs::feature(),
        headings::feature(),
    ])
}

/// The text `document` prints as, with nothing reported.
fn text(document: &str) -> String {
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    text_of(&run)
}

// Ported from Python FLM's test/test_feature_refs.py::TestRefWithHeadings::test_ref_forward_reference.
#[test]
fn a_reference_resolves_against_a_point_declared_later_in_the_document() {
    assert_eq!(
        text("See \\ref{sec:later}.\n\n\\section{Later Section}\\label{sec:later}\n"),
        "See Later Section.\n\nLater Section\n=============\n"
    );
}

#[test]
fn a_reference_resolves_against_a_point_of_an_earlier_tree_of_the_document() {
    let registry = registry();
    let run = run_workflow(
        &registry,
        text::WORKFLOW,
        &["See \\ref{sec:later}.\n", "\\section{Later Section}\\label{sec:later}\n"],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(text_of(&run), "See Later Section.\n\nLater Section\n=============\n");
}
