//! The `headings` feature (`flm_core::stdcore::features::headings`): the five
//! shipped heading kinds are five values of one element carrying their
//! declarations; a starred heading stays in the contents and only `toc=false`
//! leaves them; every heading carries one reference
//! point of kind `section` displaying its title, anchored at its first label or
//! at an encoding of its title; and a heading lowers to
//! `flm.pres.block-heading` carrying its level and anchor, and prints as a
//! titled block.

mod common;

use std::sync::Arc;

use flm_core::model::addr::TreeId;
use flm_core::model::aspects::{AnchorPolicy, ContentRef, Referenceable, Sectioning};
use flm_core::model::element::element_of;
use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::dump::golden::assert_json_golden;
use flm_core::stdcore::backends::pres::{self, HeadingPayload, payload_of};
use flm_core::stdcore::backends::{lowering, text};
use flm_core::stdcore::check_feature;
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::headings::{self, HEADING, HeadingElement, HeadingsConfig, KIND};
use flm_core::stdcore::features::{index, refpoints, refs};
use techy::core::node::NodeRef;
use flm_core::lang::Flm;

use common::lowering::{lowered, lowered_projection, text as text_of};
use common::stdcore::{golden_path, identifiers, registry_over, run_workflow};

/// A registry over the `headings` feature, the `refs` and `index` features
/// it needs, the lowering machinery, the text backend and the `blocks`
/// feature.
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

/// The one heading node of a parse of `text`, with what the test does with it.
fn with_heading<R>(text: &str, f: impl FnOnce(NodeRef<'_, Flm>) -> R) -> R {
    let registry = registry();
    let fragment = common::parse(registry.lang(), text);
    let heading = common::callables(&fragment)
        .into_iter()
        .find(|node| element_of(node).name == HEADING)
        .expect("a heading");
    f(heading)
}

#[test]
fn the_five_shipped_kinds_are_five_values_of_one_element_type_with_their_levels() {
    let registry = registry();
    let levels: Vec<u8> = registry
        .elements_named(HEADING)
        .iter()
        .map(|idx| {
            let entry = registry.element(*idx).expect("interned");
            entry.element.value_as::<HeadingElement>().expect("valued").decl.level
        })
        .collect();
    let expected: Vec<u8> = HeadingsConfig::new().headings.iter().map(|kind| kind.level).collect();
    assert_eq!(levels, expected);
}

#[test]
fn a_starred_heading_stays_in_the_contents_and_only_toc_false_leaves_them() {
    let in_toc = |text: &str| {
        with_heading(text, |node| {
            element_of(&node).aspect::<dyn Sectioning>().expect("sectioning").in_toc(&node)
        })
    };
    assert!(in_toc("\\section*{A}"));
    assert!(in_toc("\\section{A}"));
    assert!(in_toc("\\section[numbered=false]{A}"));
    assert!(!in_toc("\\section[toc=false]{A}"));
    assert!(!in_toc("\\section*[toc=false]{A}"));
    assert!(!in_toc("\\section[numbered=false,toc=false]{A}"));
}

#[test]
fn a_heading_answers_its_level_through_the_sectioning_aspect() {
    let level = with_heading("\\subsubsection{A}", |node| {
        element_of(&node).aspect::<dyn Sectioning>().expect("sectioning").level(&node)
    });
    assert_eq!(level, 3);
}

#[test]
fn a_labeled_heading_carries_one_point_anchored_at_its_first_label() {
    let points = with_heading("\\section{Intro}\\label{sec:intro}\\label{sec:start}", |node| {
        element_of(&node).aspect::<dyn Referenceable>().expect("referenceable").points(&node)
    });
    assert_eq!(points.len(), 1);
    let point = &points[0];
    assert_eq!(point.kind.as_str(), KIND);
    assert_eq!(point.labels.len(), 2);
    assert_eq!(point.anchor, AnchorPolicy::FromFirstLabel);
    // Under the shipped configuration no kind is numbered, so the heading
    // states no number source.
    assert!(point.number.is_none());
    assert_eq!(point.content, Some(ContentRef::ArgumentNamed("title".to_string())));
}

#[test]
fn a_heading_of_a_numbered_kind_states_its_counter_and_style() {
    // A heading that is not starred, of a kind the configuration's depth
    // numbers, counts in the counter of its kind and is shown in that kind's
    // style.
    let registry = common::stdcore::registry(
        flm_core::stdcore::StdcoreConfig::new().headings(
            flm_core::stdcore::features::headings::HeadingsConfig::new()
                .numbering(flm_core::stdcore::features::headings::HeadingNumbering::UpToDepth(1)),
        ),
    );
    let points = |text: &str| {
        let fragment = common::parse(registry.lang(), text);
        let heading = common::callables(&fragment)
            .into_iter()
            .find(|node| element_of(node).name == HEADING)
            .expect("a heading");
        element_of(&heading).aspect::<dyn Referenceable>().expect("referenceable").points(&heading)
    };
    let section = points("\\section{Intro}");
    let number = section[0].number.as_ref().expect("a number source");
    assert_eq!(number.counter.as_str(), "flm.headings.section");
    assert_eq!(number.style.as_str(), "flm.headings.style.section");
    assert_eq!(number.advance, [0], "one level: the number itself");
    // A kind the depth leaves unnumbered states none.
    assert!(points("\\subsection{Sub}")[0].number.is_none());
}

#[test]
fn an_unlabeled_heading_is_anchored_at_an_encoding_of_its_title() {
    let points = with_heading("\\section{Intro}", |node| {
        element_of(&node).aspect::<dyn Referenceable>().expect("referenceable").points(&node)
    });
    let title = ContentRef::ArgumentNamed("title".to_string());
    assert_eq!(
        points[0].anchor,
        AnchorPolicy::Slug { prefix: "sec".to_string(), content: title }
    );
    assert!(points[0].labels.is_empty());
}

#[test]
fn a_heading_lowers_to_a_block_heading_carrying_its_level_and_anchor() {
    let run = run_workflow(
        &registry(),
        text::WORKFLOW,
        &["\\subsection{Intro}\\label{sec:intro}\n\nText.\n"],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_json_golden(golden_path("headings-lowered.json"), &lowered_projection(&run));
    let lowered = lowered(&run, TreeId::tree(0));
    let heading = lowered
        .tree
        .root()
        .descendants()
        .find(|node| element_of(node).name == pres::BLOCK_HEADING)
        .expect("a block heading");
    let payload = payload_of::<HeadingPayload>(&heading).expect("a heading payload");
    assert_eq!((payload.level, payload.anchor.as_deref()), (2, Some("sec-intro")));
}

/// A recovered parse (`\section` with no title) lowers to a heading with an
/// empty title, and the parse's own diagnostic is the only report.
#[test]
fn a_missing_title_lowers_to_an_empty_heading_with_the_parse_s_diagnostic_alone() {
    let run = run_workflow(&registry(), text::WORKFLOW, &["\\section"], &Overrides::none());
    assert_eq!(identifiers(&run), ["core.arguments.missing-mandatory-argument"]);
    let lowered = lowered(&run, TreeId::tree(0));
    let headings: Vec<usize> = lowered
        .tree
        .root()
        .descendants()
        .filter(|node| element_of(node).name == pres::BLOCK_HEADING)
        .map(|node| node.child_count())
        .collect();
    assert_eq!(headings, [0]);
    assert!(!text_of(&run).contains("unsupported"));
}

#[test]
fn a_heading_prints_as_a_titled_block() {
    let run = run_workflow(
        &registry(),
        text::WORKFLOW,
        &["\\section{Intro}\n\nText.\n\n\\subsubsection{Deep}\n\nMore.\n"],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(text_of(&run), "Intro\n=====\n\nText.\n\n### Deep\n\nMore.\n");
}
