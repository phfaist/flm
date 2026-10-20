//! The whole pipeline through the `text` workflow, over a registry built from
//! `flm_core::stdcore::package` alone: the text output at width 80 and at width
//! 40 of a document exercising every standard feature, as goldens, and,
//! property by property, headings, paragraphs, the formatting commands,
//! typography, math, the user definitions, a reference resolved to a link
//! showing its target's title, an unresolved reference reported as an error or
//! as a warning, and the verbatim callables printed byte for byte (the
//! `verbatim` feature's own suite is `stdcore_verbatim.rs`).

mod common;

use flm_core::model::element::element_of;
use flm_core::dump::diag::expect;
use flm_core::dump::golden::{assert_json_golden, assert_text_golden};
use flm_core::lang::Flm;
use flm_core::stdcore::backends::pres::{self, LinkPayload, SpanPayload, SpanStyle, payload_of};
use flm_core::stdcore::features::refs::UnresolvedRefs;
use techy::core::node::{NodeRef, NodeTree};
use techy::error::Severity;

use common::layout::assert_well_formed;
use common::lowering::{lowered_projection, text as text_of};
use common::spine::{
    FEATURES_DOCUMENT, SESSION06_DOCUMENT, golden_path, overrides, registry, render,
    tables_projection, text_at,
};
use common::stdcore::identifiers;

/// The lowered form of the features document, tolerating its unresolved
/// reference.
fn features_lowered() -> std::sync::Arc<NodeTree<Flm>> {
    let run = render(&registry(), &[FEATURES_DOCUMENT], &overrides(None, UnresolvedRefs::Allow));
    let lowered = common::lowering::lowered(&run, flm_core::model::addr::TreeId::tree(0));
    std::sync::Arc::clone(&lowered.tree)
}

/// Every node of `element` in storage order.
fn nodes_of<'t>(tree: &'t NodeTree<Flm>, element: &str) -> Vec<NodeRef<'t, Flm>> {
    tree.iter_storage_order().filter(|node| element_of(node).name == element).collect()
}

/// The plain text under a presentation node's body.
fn body_text(node: &NodeRef<'_, Flm>) -> String {
    pres::plain_text(node.body().expect("a body").iter())
}

/// The features document printed without wrapping, for substring checks.
fn features_unwrapped() -> String {
    text_at(&registry(), FEATURES_DOCUMENT, None)
}

// --- the goldens: the step's deliverable ---------------------------------------------------

#[test]
fn the_features_document_renders_at_width_80_as_the_golden_shows() {
    let text = text_at(&registry(), FEATURES_DOCUMENT, Some(80));
    assert_well_formed(&text);
    assert!(text.lines().all(|line| line.chars().count() <= 80), "{text}");
    assert_text_golden(golden_path("features-80.txt"), &text);
}

#[test]
fn the_features_document_renders_at_width_40_as_the_golden_shows() {
    let text = text_at(&registry(), FEATURES_DOCUMENT, Some(40));
    assert_well_formed(&text);
    assert_text_golden(golden_path("features-40.txt"), &text);
}

#[test]
fn the_session_06_document_renders_at_width_80_as_the_golden_shows() {
    let text = text_at(&registry(), SESSION06_DOCUMENT, Some(80));
    assert_well_formed(&text);
    assert!(text.lines().all(|line| line.chars().count() <= 80), "{text}");
    assert_text_golden(golden_path("session06-80.txt"), &text);
}

#[test]
fn the_session_06_document_renders_at_width_40_as_the_golden_shows() {
    let text = text_at(&registry(), SESSION06_DOCUMENT, Some(40));
    assert_well_formed(&text);
    assert!(text.lines().all(|line| line.chars().count() <= 40), "{text}");
    assert_text_golden(golden_path("session06-40.txt"), &text);
}

#[test]
fn the_features_document_lowers_as_the_golden_shows() {
    let run = render(&registry(), &[FEATURES_DOCUMENT], &overrides(None, UnresolvedRefs::Allow));
    assert_json_golden(golden_path("features-lowered.json"), &lowered_projection(&run));
}

#[test]
fn the_reference_tables_of_the_features_document_project_as_the_golden_shows() {
    let run = render(&registry(), &[FEATURES_DOCUMENT], &overrides(None, UnresolvedRefs::Allow));
    let names = [
        "flm.refpoints",
        "flm.labels",
        "flm.anchors",
        "flm.refs.targets",
        "flm.refs.resolutions",
    ];
    let tables = tables_projection(&run, &names);
    assert_json_golden(golden_path("features-tables.json"), &tables);
}

// --- the features, one property each --------------------------------------------------------

#[test]
fn every_heading_level_prints_with_its_rule_or_its_prefix() {
    let text = features_unwrapped();
    assert!(text.contains("Introduction\n============\n"), "{text}");
    assert!(text.contains("Formatting\n----------\n"), "{text}");
    assert!(text.contains("\n### Math\n"), "{text}");
    assert!(text.contains("\n#### Definitions\n"), "{text}");
    assert!(text.contains("\n##### References\n"), "{text}");
    assert!(text.contains("Unnumbered\n==========\n"), "{text}");
}

#[test]
fn a_comment_leaves_no_trace_in_the_text() {
    let text = features_unwrapped();
    assert!(!text.contains("comment vanishes"), "{text}");
    assert!(!text.contains('%'), "{text}");
}

#[test]
fn the_verbatim_callables_print_their_content_byte_for_byte() {
    // The three macros and the inline environment sit in their paragraph; the
    // `verbatimcode` environment is a block of its own, indentation kept.
    let text = features_unwrapped();
    assert!(text.contains("Inline a{b}\\c text, the code f(x) = 1"), "{text}");
    assert!(text.contains("the identifier struct, then raw {text} in the same"), "{text}");
    assert!(text.contains("\n\ndef f(x):\n    return {x} + 1\n\n"), "{text}");
}

#[test]
fn paragraphs_are_blocks_one_blank_line_apart() {
    let text = text_at(&registry(), SESSION06_DOCUMENT, None);
    assert!(text.contains("forty columns.\n\nTheorem."), "{text}");
    assert!(text.contains("nicely.\n\n2. Second"), "{text}");
}

#[test]
fn the_formatting_commands_lower_to_spans_of_their_styles() {
    let tree = features_lowered();
    let styles: Vec<Option<SpanStyle>> = nodes_of(&tree, pres::SPAN)
        .iter()
        .map(|span| payload_of::<SpanPayload>(span).expect("a span payload").style)
        .collect();
    for style in [SpanStyle::Emphasis, SpanStyle::Bold, SpanStyle::Italic, SpanStyle::Monospace] {
        assert!(styles.contains(&Some(style)), "{style:?} is missing from {styles:?}");
    }
}

#[test]
fn typography_substitutes_the_dash_the_ellipsis_and_the_quotes() {
    let text = features_unwrapped();
    assert!(text.contains("text \u{2014} with a dash, an ellipsis\u{2026} and"), "{text}");
    assert!(text.contains("\u{201c}quotes\u{201d}"), "{text}");
}

#[test]
fn math_is_carried_as_its_source() {
    let text = features_unwrapped();
    assert!(text.contains("\\(x_1 + \\alpha\\)"), "{text}");
    assert!(text.contains("\\int_0^1 f(x)\\,dx = 1"), "{text}");
    // A numbered line carries its number where an author's `\tag` would
    // stand; the lines of `align*` carry none.
    assert!(
        text.contains("\\begin{equation}\n    E = mc^2\\tag*{(1)}\n  \\end{equation}"),
        "{text}"
    );
    assert!(text.contains("\\begin{align*}"), "{text}");
}

#[test]
fn an_unknown_macro_inside_math_is_the_opaque_fallback_carried_verbatim() {
    let run = render(&registry(), &[FEATURES_DOCUMENT], &overrides(None, UnresolvedRefs::Allow));
    assert_eq!(
        identifiers(&run),
        ["flm.refs.unresolved"],
        "no parse error for \\alpha"
    );
    let tree = features_lowered();
    let mut opaque: Vec<String> = tree
        .iter_storage_order()
        .filter(|node| element_of(node).name == "flm.math-passthrough.opaque")
        .map(|node| node.span().content().to_string())
        .collect();
    opaque.sort();
    // `\alpha`, `\int`, `\,` in the math groups; the `\\` in the environment is
    // the declared line break, not an opaque node.
    assert_eq!(opaque, ["\\,", "\\alpha", "\\int"]);
}

#[test]
fn user_definitions_are_expanded_before_lowering() {
    let text = features_unwrapped();
    assert!(
        text.contains("Hello, World! from a user macro, and [Note: a user environment] in"),
        "{text}"
    );
    assert!(text.contains("It is version 1.0."), "{text}");
    let tree = features_lowered();
    let survivors = tree
        .iter_storage_order()
        .filter(|node| element_of(node).name.as_str().starts_with("flm.userdefs."))
        .count();
    assert_eq!(survivors, 0);
}

#[test]
fn a_resolved_reference_is_a_link_showing_the_targets_title() {
    let tree = features_lowered();
    let links = nodes_of(&tree, pres::LINK);
    assert_eq!(links.len(), 2);
    let hrefs: Vec<String> = links
        .iter()
        .map(|link| payload_of::<LinkPayload>(link).expect("a link").href.clone())
        .collect();
    // The anchors are the labels made safe (`:` → `-`), as the refs feature
    // derives them from a `\\label`.
    assert_eq!(hrefs, ["#sec-intro", "#sec-fmt"]);
    let titles: Vec<String> = links.iter().map(body_text).collect();
    assert_eq!(titles, ["Introduction", "Formatting"]);
    let text = features_unwrapped();
    assert!(text.contains("See Introduction, Formatting and sec:missing."), "{text}");
}

#[test]
fn an_unresolved_reference_is_an_unresolved_span_in_the_lowered_tree() {
    let tree = features_lowered();
    let unresolved: Vec<NodeRef<'_, Flm>> = nodes_of(&tree, pres::SPAN)
        .into_iter()
        .filter(|span| {
            payload_of::<SpanPayload>(span).expect("a span").style
                == Some(SpanStyle::UnresolvedReference)
        })
        .collect();
    assert_eq!(unresolved.len(), 1);
    assert_eq!(body_text(&unresolved[0]), "sec:missing");
}

#[test]
fn an_unresolved_reference_is_an_error_by_default() {
    let run = render(&registry(), &[FEATURES_DOCUMENT], &overrides(None, UnresolvedRefs::Error));
    let diagnostic =
        expect(run.diagnostics().iter(), "flm.refs.unresolved").at("\\ref{sec:missing}").once();
    assert_eq!(diagnostic.severity(), Severity::Error);
    assert!(run.diagnostics().has_errors());
}

#[test]
fn an_unresolved_reference_is_a_warning_when_the_request_allows_it() {
    let run = render(&registry(), &[FEATURES_DOCUMENT], &overrides(None, UnresolvedRefs::Allow));
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.unresolved").once();
    assert_eq!(diagnostic.severity(), Severity::Warning);
    assert!(!run.diagnostics().has_errors());
    assert!(!text_of(&run).is_empty());
}

#[test]
fn the_features_document_reports_the_unresolved_reference_and_nothing_else() {
    let run = render(&registry(), &[FEATURES_DOCUMENT], &overrides(None, UnresolvedRefs::Allow));
    assert_eq!(identifiers(&run), ["flm.refs.unresolved"]);
}

#[test]
fn the_session_06_document_renders_without_a_diagnostic() {
    let run = render(&registry(), &[SESSION06_DOCUMENT], &overrides(None, UnresolvedRefs::Error));
    assert!(!run.diagnostics().any_recorded(), "{}", run.diagnostics().render_all());
}

#[test]
fn the_text_plan_over_the_package_schedules_every_stage_of_the_spine() {
    let plan = common::spine::text_plan(&registry(), &overrides(None, UnresolvedRefs::Allow));
    let names = |names: Vec<flm_core::model::names::StageName>| -> Vec<String> {
        names.iter().map(|name| name.to_string()).collect()
    };
    // The `always` definition's rule requires `flm.userdefs.always-expanded`,
    // so its stage is scheduled ahead of the per-plan expansion.
    assert_eq!(
        names(plan.rewrite_names()),
        ["flm.input.splice", "flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]
    );
    // The workflow demands `flm.validated` beside its output, so the three
    // checks are scheduled with the backend's own stages.
    assert_eq!(
        names(plan.stage_names()),
        [
            "flm.blocks.check-contract",
            "flm.index.build",
            "flm.refpoints.scan",
            "flm.numbering.number",
            "flm.refs.scan",
            "flm.refs.anchors",
            "flm.refs.resolve",
            "flm.refs.check-labels",
            "flm.xnotes.collect",
            "flm.xnotes.build-list.footnote",
            "flm.theorems.check-proofs",
            "flm.graphics.inspect",
            "flm.graphics.publish.text",
            "flm.floats.check-handlers",
            "flm.citations.collect",
            "flm.citations.retrieve",
            "flm.citations.check",
            "flm.citations.build-bibliography",
            "flm.meta.build",
            "flm.text.pieces",
            "flm.text.lower",
            "flm.text.emit",
            "flm.text.regions",
            "flm.text.final",
            "flm.math-passthrough.check",
        ]
    );
}
