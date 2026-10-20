//! The `textstyles` feature (`flm_core::stdcore::features::textstyles`): the four inline
//! formatting commands are four values of one element type, each lowering to a
//! `flm.pres.span` that carries its style and keeps the whitespace around it;
//! and the lowered tree of a small document as a projection golden.

mod common;

use std::sync::Arc;

use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::dump::golden::assert_json_golden;
use flm_core::stdcore::backends::pres::{self, SpanPayload, SpanStyle, payload_of};
use flm_core::stdcore::backends::{lowering, text};
use flm_core::stdcore::check_feature;
use flm_core::stdcore::features::textstyles::{self, TEXT_STYLE, TextStyle, TextStyleElement};
use flm_core::stdcore::features::blocks::Blocks;

use common::lowering::{lowered, lowered_projection, text as text_of};
use common::stdcore::{golden_path, identifiers, registry_over, run_workflow};

/// A registry over the `textstyles` feature, the lowering machinery, the text
/// backend and the `blocks` feature.
fn registry() -> Arc<FlmRegistry> {
    registry_over(vec![
        lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        check_feature(),
        Arc::new(Blocks::default()),
        textstyles::feature(),
    ])
}

#[test]
fn the_four_commands_are_four_values_of_one_element_type() {
    let registry = registry();
    let styles: Vec<TextStyle> = registry
        .elements_named(TEXT_STYLE)
        .iter()
        .map(|idx| {
            let entry = registry.element(*idx).expect("interned");
            entry.element.value_as::<TextStyleElement>().expect("valued").style
        })
        .collect();
    assert_eq!(styles, TextStyle::ALL);
}

#[test]
fn each_command_lowers_to_a_span_carrying_its_style() {
    let run = run_workflow(
        &registry(),
        text::WORKFLOW,
        &["\\emph{a} \\textbf{b} \\textit{c} \\texttt{d}"],
        &Overrides::none(),
    );
    let lowered = lowered(&run, TreeId::tree(0));
    let styles: Vec<Option<SpanStyle>> = lowered
        .tree
        .root()
        .descendants()
        .filter(|node| element_of(node).name == pres::SPAN)
        .map(|node| payload_of::<SpanPayload>(&node).expect("a span payload").style)
        .collect();
    assert_eq!(
        styles,
        [
            Some(SpanStyle::Emphasis),
            Some(SpanStyle::Bold),
            Some(SpanStyle::Italic),
            Some(SpanStyle::Monospace),
        ]
    );
    assert_eq!(text_of(&run), "a b c d\n");
}

/// A recovered parse (`\emph` with no argument) lowers to an empty span,
/// and the parse's own diagnostic is the only report.
#[test]
fn a_missing_argument_lowers_to_an_empty_span_with_the_parse_s_diagnostic_alone() {
    let run = run_workflow(&registry(), text::WORKFLOW, &["a \\emph"], &Overrides::none());
    assert_eq!(identifiers(&run), ["core.arguments.missing-mandatory-argument"]);
    let lowered = lowered(&run, TreeId::tree(0));
    let spans: Vec<usize> = lowered
        .tree
        .root()
        .descendants()
        .filter(|node| element_of(node).name == pres::SPAN)
        .map(|node| node.child_count())
        .collect();
    assert_eq!(spans, [0]);
    assert!(!text_of(&run).contains("unsupported"));
}

#[test]
fn a_command_keeps_the_whitespace_around_it() {
    let run = run_workflow(&registry(), text::WORKFLOW, &["x \\emph{a} y"], &Overrides::none());
    assert_eq!(text_of(&run), "x a y\n");
}

#[test]
fn the_lowered_tree_of_a_formatted_paragraph_matches_its_golden() {
    let run = run_workflow(
        &registry(),
        text::WORKFLOW,
        &["Some \\emph{emphasized \\textbf{and bold}} text."],
        &Overrides::none(),
    );
    assert_json_golden(golden_path("textstyles-spans.json"), &lowered_projection(&run));
}
