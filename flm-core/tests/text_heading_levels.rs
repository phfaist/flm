//! How the text backend marks each of the five heading levels: level 1 is
//! underlined with `=`, level 2 with `-`, and levels 3, 4 and 5 carry a prefix
//! of as many `#` marks as the level.

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

/// A registry over the `headings` feature, the `refs` and `index` features it
/// needs, the lowering machinery, the text backend and the `blocks` feature.
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

// Ported from Python FLM's test/test_feature_headings.py::TestFeatureHeadingsTextRenderer::test_subsubsection_text.
#[test]
fn the_five_levels_print_two_underlines_and_three_hash_prefixes() {
    let document = "\\section{One}\n\n\\subsection{Two}\n\n\\subsubsection{Deep}\n\n\
                    \\paragraph{Four}\n\n\\subparagraph{Five}\n\nText.\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        text_of(&run),
        "One\n===\n\nTwo\n---\n\n### Deep\n\n#### Four\n\n##### Five\n\nText.\n"
    );
}
