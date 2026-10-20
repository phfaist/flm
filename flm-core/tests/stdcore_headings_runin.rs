//! Run-in headings (`flm_core::stdcore::features::headings::HeadingsConfig`): a
//! heading level a host lists as *run in* is set at the start of the paragraph
//! it introduces rather than on a line of its own.
//!
//! The setting changes the heading's block role — it becomes a decorator
//! leading its paragraph — and what the lowering rule stages for it: a
//! `flm.pres.deco.runin` decoration over a `flm.pres.runin-heading` carrying
//! the same payload a block heading carries, instead of a
//! `flm.pres.block-heading`. Everything else about
//! the heading is untouched, which is what most of these tests check: the level,
//! the table-of-contents membership and the reference point stay as they are,
//! and the default configuration — no level listed — leaves every heading a
//! block.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::model::addr::TreeId;
use flm_core::model::aspects::{
    BlockRole, Placement, Referenceable, Sectioning, block_role_of,
};
use flm_core::model::element::element_of;
use flm_core::model::names::Name;
use flm_core::stdcore::backends::pres::{self, HeadingPayload, attached_slots, payload_of};
use flm_core::stdcore::backends::{html, text};
use flm_core::stdcore::features::headings::title::NUMBER_ELEMENT;
use flm_core::stdcore::features::headings::{HEADING, HeadingsConfig};
use flm_core::stdcore::{StdcoreConfig, package};
use flm_core::lang::Flm;
use techy::core::node::NodeRef;

use common::lowering::{lowered, text as text_of};
use common::stdcore::{registry as stdcore_registry, run_workflow};
use common::wiring::frozen;

/// The standard package with `\paragraph` (level 4) set as a run-in heading.
fn run_in_registry() -> Arc<FlmRegistry> {
    stdcore_registry(StdcoreConfig::new().headings(HeadingsConfig::new().run_in_levels([4])))
}

/// The standard package under its default configuration: no run-in level.
fn default_registry() -> Arc<FlmRegistry> {
    stdcore_registry(StdcoreConfig::new())
}

/// A run of the text workflow over one document holding `text`, with no
/// diagnostic reported.
fn render(registry: &Arc<FlmRegistry>, text: &str) -> flm_core::core::run::Run {
    let run = run_workflow(registry, text::WORKFLOW, &[text], &Overrides::none());
    assert!(
        run.diagnostics().iter().next().is_none(),
        "{}",
        run.diagnostics().render_all()
    );
    run
}

/// The elements of the direct children of the frozen tree's root, one name per
/// child, so that a test can say what the document was cut into.
fn top_level(run: &flm_core::core::run::Run) -> Vec<String> {
    frozen(run)
        .root()
        .children()
        .iter()
        .map(|node| element_of(&node).name.to_string())
        .collect()
}

/// The one heading node of the frozen tree.
fn heading_of(run: &flm_core::core::run::Run) -> NodeRef<'_, Flm> {
    frozen(run)
        .root()
        .descendants()
        .find(|node| element_of(node).name == HEADING)
        .expect("a heading in the frozen tree")
}

// --- block structure ------------------------------------------------------------------

/// The document of the walkthrough: a paragraph, the run-in heading with the
/// content it introduces, and a paragraph after it.
const DOCUMENT: &str = "Intro text.\n\n\\paragraph{Note} Some content.\n\nAfter.\n";

#[test]
fn a_run_in_heading_leads_the_paragraph_it_opens() {
    let run = render(&run_in_registry(), DOCUMENT);
    assert_eq!(top_level(&run), ["flm.paragraph", "flm.paragraph", "flm.paragraph"]);
    let paragraph = frozen(&run).root().children().iter().nth(1).expect("the second block");
    let children: Vec<String> =
        paragraph.children().iter().map(|node| element_of(&node).name.to_string()).collect();
    assert_eq!(children[0], HEADING);
    assert_eq!(children.len(), 2, "the heading and the content it introduces");
    assert_eq!(
        block_role_of(&heading_of(&run)),
        BlockRole::BlockDecorator(Placement::Leads)
    );
}

#[test]
fn the_same_document_under_the_default_configuration_keeps_the_heading_a_block() {
    let run = render(&default_registry(), DOCUMENT);
    assert_eq!(
        top_level(&run),
        ["flm.paragraph", HEADING, "flm.paragraph", "flm.paragraph"]
    );
    assert_eq!(block_role_of(&heading_of(&run)), BlockRole::Block);
}

/// A run-in heading with nothing after it on its own side of the blank line
/// makes a paragraph of its own, holding the heading and nothing else. Nothing
/// is dropped, and no empty paragraph is produced.
#[test]
fn a_run_in_heading_followed_by_a_blank_line_stands_in_a_paragraph_of_its_own() {
    let run = render(&run_in_registry(), "\\paragraph{Note}\n\nSome content.\n");
    assert_eq!(top_level(&run), ["flm.paragraph", "flm.paragraph"]);
    let first = frozen(&run).root().children().iter().next().expect("the first block");
    let children: Vec<String> =
        first.children().iter().map(|node| element_of(&node).name.to_string()).collect();
    assert_eq!(children, [HEADING]);
    assert_eq!(text_of(&run), "Note.\n\nSome content.\n");
}

/// A fragment holding nothing that asks for blocks — no paragraph break and no
/// block-role element — is laid out as one inline run, so it has no paragraph
/// for the heading to lead. The heading stays where it stands, and the text
/// backend sets it with the same idiom.
#[test]
fn a_fragment_with_no_blocks_keeps_the_run_in_heading_in_its_inline_run() {
    let run = render(&run_in_registry(), "\\paragraph{Note} Some content.");
    let top = top_level(&run);
    assert_eq!(top[0], HEADING, "no paragraph was built: {top:?}");
    assert_eq!(text_of(&run), "Note. Some content.\n");
}

// --- lowering -------------------------------------------------------------------------

#[test]
fn a_run_in_heading_lowers_to_a_run_in_decoration_over_a_run_in_title() {
    let run = render(&run_in_registry(), "Intro.\n\n\\paragraph{Note}\\label{sec:note} Text.\n");
    let lowered = lowered(&run, TreeId::tree(0));
    let root = lowered.tree.root();
    assert!(
        root.descendants().all(|node| element_of(&node).name != pres::BLOCK_HEADING),
        "a run-in heading stages no block heading"
    );
    // The driver hoists the decoration into a slot attached to the paragraph and
    // leaves a mark where it stood.
    let paragraph = root
        .descendants()
        .find(|node| {
            element_of(node).name == pres::PAR
                && node.children().iter().any(|child| element_of(&child).name == pres::DECO_MARK)
        })
        .expect("a paragraph carrying a decoration mark");
    let slots = attached_slots(&paragraph);
    assert_eq!(slots.len(), 1);
    let deco = slots[0].content.iter().next().expect("the decoration");
    assert_eq!(element_of(&deco).name, pres::DECO_RUNIN);
    let title = deco.body().expect("the decoration's body").iter().next().expect("the title");
    assert_eq!(element_of(&title).name, pres::RUNIN_HEADING);
    let payload = payload_of::<HeadingPayload>(&title).expect("a heading payload");
    assert_eq!(
        payload,
        &HeadingPayload {
            name: Name::from("paragraph"),
            level: 4,
            in_toc: true,
            anchor: Some(String::from("sec-note")),
            number: None,
        }
    );
}

#[test]
fn a_level_that_is_not_listed_still_lowers_to_a_block_heading() {
    let run = render(&run_in_registry(), "\\subsection{Details}\n\nText.\n");
    let lowered = lowered(&run, TreeId::tree(0));
    assert!(
        lowered
            .tree
            .root()
            .descendants()
            .any(|node| element_of(&node).name == pres::BLOCK_HEADING),
        "level 2 is not listed as run in"
    );
}

// --- the text backend -----------------------------------------------------------------

#[test]
fn the_text_backend_prints_a_run_in_heading_before_the_first_word() {
    assert_eq!(
        text_of(&render(&run_in_registry(), DOCUMENT)),
        "Intro text.\n\nNote. Some content.\n\nAfter.\n"
    );
    assert_eq!(
        text_of(&render(&default_registry(), DOCUMENT)),
        "Intro text.\n\n#### Note\n\nSome content.\n\nAfter.\n"
    );
}

/// The text backend prints a run-in heading's title as its content, with the
/// number where the heading has one, exactly as it did when the title stood
/// in a generic span.
#[test]
fn the_text_backend_prints_the_run_in_title_as_its_content() {
    let registry = stdcore_registry(
        StdcoreConfig::new().headings(
            HeadingsConfig::new()
                .run_in_levels([4])
                .numbering(flm_core::stdcore::features::headings::HeadingNumbering::AllNumbered),
        ),
    );
    let run = render(
        &registry,
        "\\section{A}\\subsection{B}\\paragraph{Note}\\label{sec:note} Some content.\n\n\
         \\paragraph[toc=false]{Why?} More.\n",
    );
    assert_eq!(
        text_of(&run),
        "1. A\n====\n\n1.1. B\n------\n\na. Note. Some content.\n\nb. Why? More.\n"
    );
}

/// The stop after a run-in heading is the text backend's own convention, and it
/// is not added where the title already ends in sentence punctuation.
#[test]
fn a_title_that_already_ends_in_punctuation_takes_no_second_stop() {
    let run = render(&run_in_registry(), "Intro.\n\n\\paragraph{Note:} Some content.\n");
    assert_eq!(text_of(&run), "Intro.\n\nNote: Some content.\n");
}

// --- the HTML backend -----------------------------------------------------------------

/// A run-in heading's title is written as a `<span>` carrying the same
/// attributes as a block heading, from the same facts; a block heading is
/// written as before.
#[test]
fn the_html_backend_writes_a_run_in_heading_with_the_attributes_of_a_block_heading() {
    let run = run_workflow(
        &run_in_registry(),
        html::WORKFLOW,
        &["\\subsection{Details}\\label{sec:details}\n\n\\paragraph[toc=false]{Note}\\label{sec:note} Text.\n"],
        &Overrides::none(),
    );
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
    let html = common::html::fragment(&run);
    assert!(
        html.contains(
            "<span class=\"flm-pres-deco-runin\"><span class=\"flm-pres-runin--heading\" \
             id=\"sec-note\" data-flm-heading=\"paragraph\" data-flm-level=\"4\" \
             data-flm-toc=\"false\">Note</span>.</span>"
        ),
        "{html}"
    );
    assert!(
        html.contains(
            "<h2 class=\"flm-pres-block--heading\" id=\"sec-details\" \
             data-flm-heading=\"subsection\" data-flm-level=\"2\" data-flm-toc=\"true\">\
             Details</h2>"
        ),
        "{html}"
    );
}

// --- the aspects the setting leaves alone ---------------------------------------------

#[test]
fn a_run_in_heading_still_answers_its_level_and_its_contents_membership() {
    let registry = run_in_registry();
    let run = render(&registry, "Intro.\n\n\\paragraph{Note} Text.\n");
    let heading = heading_of(&run);
    let element = element_of(&heading);
    let sectioning = element.aspect::<dyn Sectioning>().expect("the sectioning aspect");
    assert_eq!(sectioning.level(&heading), 4);
    assert!(sectioning.in_toc(&heading), "a heading is in the contents");

    let starred = render(&registry, "Intro.\n\n\\paragraph*{Note} Text.\n");
    let heading = heading_of(&starred);
    assert!(
        element_of(&heading).aspect::<dyn Sectioning>().expect("aspect").in_toc(&heading),
        "a starred run-in heading stays in the contents, as a starred block heading does"
    );
    let left_out = render(&registry, "Intro.\n\n\\paragraph[toc=false]{Note} Text.\n");
    let heading = heading_of(&left_out);
    assert!(
        !element_of(&heading).aspect::<dyn Sectioning>().expect("aspect").in_toc(&heading),
        "`toc=false` leaves a run-in heading out of the contents"
    );
}

#[test]
fn a_run_in_heading_still_carries_its_reference_point() {
    let run = render(&run_in_registry(), "Intro.\n\n\\paragraph{Note}\\label{sec:note} Text.\n");
    let heading = heading_of(&run);
    let points = element_of(&heading)
        .aspect::<dyn Referenceable>()
        .expect("the referenceable aspect")
        .points(&heading);
    assert_eq!(points.len(), 1);
    assert_eq!(points[0].labels.len(), 1);
}

#[test]
fn a_reference_to_a_run_in_heading_resolves_and_displays_its_title() {
    let run = render(
        &run_in_registry(),
        "Intro.\n\n\\paragraph{Note}\\label{sec:note} Some content.\n\nSee \\ref{sec:note}.\n",
    );
    assert_eq!(text_of(&run), "Intro.\n\nNote. Some content.\n\nSee Note.\n");
}

// --- the setting and the registry -----------------------------------------------------

/// The rule's declaration lists every presentation element it may build, and a
/// backend claiming those productions must be able to render them. With no level
/// listed the rule can build no decoration and no run-in title, so a default
/// registry must not ask its backend for them. The span of a numbered heading's
/// number is built at any level by the rule of the number node, which the
/// heading rule's content holds.
#[test]
fn the_rule_declares_the_decoration_only_where_a_level_is_listed() {
    let produces = |registry: &Arc<FlmRegistry>| {
        let idx = *registry
            .rules_for(HEADING)
            .first()
            .expect("the heading rule");
        registry
            .rule(idx)
            .rule
            .decl
            .produces
            .iter()
            .map(|name| name.to_string())
            .collect::<Vec<String>>()
    };
    assert_eq!(produces(&default_registry()), [pres::BLOCK_HEADING]);
    assert_eq!(
        produces(&run_in_registry()),
        [pres::BLOCK_HEADING, pres::DECO_RUNIN, pres::RUNIN_HEADING]
    );
    let registry = default_registry();
    let idx = *registry.rules_for(NUMBER_ELEMENT).first().expect("the number rule");
    let produced: Vec<String> =
        registry.rule(idx).rule.decl.produces.iter().map(|name| name.to_string()).collect();
    assert_eq!(produced, [pres::SPAN]);
}

#[test]
fn two_registries_that_differ_in_the_run_in_levels_have_different_digests() {
    let digest = |levels: [u8; 1]| {
        stdcore_registry(
            StdcoreConfig::new().headings(HeadingsConfig::new().run_in_levels(levels)),
        )
        .digest()
    };
    assert_eq!(digest([4]), digest([4]));
    assert_ne!(digest([4]), digest([5]));
    assert_ne!(digest([4]), default_registry().digest());
}

#[test]
fn the_package_is_the_same_roster_whatever_the_setting() {
    let names = |config: StdcoreConfig| {
        package(config)
            .features
            .iter()
            .map(|feature| feature.info().name.to_string())
            .collect::<Vec<String>>()
    };
    assert_eq!(
        names(StdcoreConfig::new()),
        names(StdcoreConfig::new().headings(HeadingsConfig::new().run_in_levels([4, 5])))
    );
}
