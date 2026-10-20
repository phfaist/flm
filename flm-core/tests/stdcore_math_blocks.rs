//! Where a display-mathematics environment stands in a document's block
//! structure: it is a block of its own, so the text written around it on the
//! neighbouring lines becomes two paragraphs rather than one.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::stdcore::backends::pres;
use flm_core::stdcore::backends::{lowering, text};
use flm_core::stdcore::check_feature;
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::{math, textstyles};

use common::lowering::{lowered, text as text_of};
use common::stdcore::{identifiers, registry_over, run_workflow};

/// A registry over the `math` and `textstyles` features, the lowering
/// machinery, the text backend and the `blocks` feature.
fn registry() -> Arc<FlmRegistry> {
    registry_over(vec![
        lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        check_feature(),
        Arc::new(Blocks::default()),
        textstyles::feature(),
        math::feature(),
        flm_core::math_passthrough::feature(),
    ])
}

// Ported from Python FLM's test/test_flmenvironment.py::TestBlocksBuilder::test_blocks_paragraphs_correct_number.
#[test]
fn a_display_environment_inside_running_text_makes_three_blocks_of_it() {
    let document = "Test\nsome\ninput \\textbf{string} here. Does this\n\
                    \\begin{equation}\n a = b\n\\end{equation}\n\
                    work?\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());

    let blocks: Vec<String> = lowered(&run, TreeId::tree(0))
        .tree
        .root()
        .children()
        .iter()
        .map(|node| element_of(&node).name.to_string())
        .collect();
    assert_eq!(
        blocks,
        [pres::PAR.to_string(), pres::MATH_PASSTHROUGH.to_string(), pres::PAR.to_string()]
    );

    assert_eq!(
        text_of(&run),
        "Test some input string here. Does this\n\n\
         \x20 \\begin{equation}\n   a = b\n  \\end{equation}\n\n\
         work?\n"
    );
}
