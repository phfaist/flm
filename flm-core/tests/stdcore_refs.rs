//! The `refs` feature (`flm_core::stdcore::features::refs`): labels are scanned
//! off every reference point with their display content copied into a value and
//! duplicates recorded; anchors are encoded from the first label the point
//! keeps or from its title, a repeated title numbered and any other second
//! point computing the same anchor reported; a reference resolves to its point
//! or to an external
//! target, and an unknown one is recorded unresolved — an error by default, a
//! warning under `Allow`; the rules lower a resolved reference to a link around
//! its spliced title and an unresolved one to an unresolved-styled span; a
//! reference inside a heading title resolves through the spliced value; and the
//! `check` workflow reports duplicate labels and unresolved references.

mod common;

use std::sync::Arc;

use flm_core::lang::text::plain_text;
use flm_core::lang::mint::FIELD;
use flm_core::model::addr::TreeId;
use flm_core::model::aspects::{Label, Referencing};
use flm_core::model::element::element_of;
use flm_core::model::options::OptionValues;
use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Externals;
use flm_core::dump::diag::expect;
use flm_core::dump::golden::assert_json_golden;
use flm_core::stdcore::backends::pres::{self, LinkPayload, SpanPayload, SpanStyle, payload_of};
use flm_core::stdcore::backends::{lowering, text};
use flm_core::stdcore::check_feature;
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::refs::{
    self, ANCHORS, BadLabel, CHECK_LABELS, DuplicateLabelCondition, EXTERNAL_INDEX,
    ExternalIndex, ExternalTarget, LABELS, REF_RESOLUTIONS, REF_TARGETS,
    RefsRenderOptions, Resolution, SiteId, Unresolved, UnresolvedRefs,
};
use flm_core::stdcore::features::refpoints::{self, REFPOINTS};
use flm_core::stdcore::features::{headings, index};
use flm_core::stdcore::{CHECK_WORKFLOW, VALIDATED};
use techy::error::Severity;

use common::lowering::{lowered, lowered_projection, text as text_of};
use common::stdcore::{
    SHOWN_HERE, golden_path, identifiers, projection, registry_over, run_workflow,
    run_workflow_with,
};

/// A registry over the `refs` feature, the `headings` feature (its labeled
/// element), the `index` feature, the lowering machinery, the text backend,
/// the `blocks` feature and the `check` feature (whose workflow the tests
/// exercise).
fn registry() -> Arc<FlmRegistry> {
    registry_over(vec![
        lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        Arc::new(Blocks::default()),
        check_feature(),
        index::feature(),
        refpoints::feature(),
        refs::feature(),
        headings::feature(),
    ])
}

/// The render-bound option making an unresolved reference a warning.
fn allow_unresolved() -> Overrides {
    Overrides {
        render_options: OptionValues::new()
            .with(RefsRenderOptions { unresolved: UnresolvedRefs::Allow }),
        ..Overrides::none()
    }
}

/// Two headings, the second repeating the first's label and adding one.
const DUPLICATE: &str =
    "\\section{Intro}\\label{sec:intro}\n\n\\section{More}\\label{sec:intro}\\label{sec:more}\n";

/// A labeled heading, a reference to it and a reference to nothing.
const ONE_OF_EACH: &str =
    "\\section{Intro}\\label{sec:intro}\n\nSee \\ref{sec:intro} and \\ref{sec:nowhere}.\n";

#[test]
fn labels_are_scanned_off_the_points_with_their_titles_and_duplicates_recorded() {
    let run = run_workflow(&registry(), text::WORKFLOW, &[DUPLICATE], &Overrides::none());
    assert_json_golden(golden_path("refs-labels.json"), &projection(&run, LABELS));
    assert_json_golden(golden_path("refs-points.json"), &projection(&run, REFPOINTS));
}

#[test]
fn anchors_are_encoded_from_the_first_kept_label_or_from_the_title() {
    let document = "\\section{The Sum, of Things}\n\n\\section{Éléments 2}\n\n\
                    \\section{Third}\\label{sec:third}\n\n\\section*{!!}\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_json_golden(golden_path("refs-anchors.json"), &projection(&run, ANCHORS));
}

/// The anchor identifiers of `run`'s reference points, in document order.
fn anchor_ids(run: &flm_core::core::run::Run) -> Vec<Option<String>> {
    let anchors = run.table(&ANCHORS).expect("declared").expect("written");
    anchors.ids.values().cloned().collect()
}

/// Two unlabeled headings with one title: the first keeps the anchor, the
/// second takes the same one numbered, and nothing is reported.
#[test]
fn a_repeated_heading_title_is_numbered_and_reported_nowhere() {
    let document = "\\section{Same}\n\n\\section{Same}\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        anchor_ids(&run),
        [Some("sec--Same".to_string()), Some("sec--Same-2".to_string())]
    );
}

/// The numbering counts on: a third heading with the same title takes `-3`.
#[test]
fn a_third_heading_with_one_title_takes_the_next_number() {
    let document = "\\section{Same}\n\n\\section{Same}\n\n\\section{Same}\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        anchor_ids(&run),
        [
            Some("sec--Same".to_string()),
            Some("sec--Same-2".to_string()),
            Some("sec--Same-3".to_string()),
        ]
    );
}

/// A number a heading has already taken under a title of its own is skipped:
/// `Same-2` encodes to the identifier the numbering would hand the second
/// `Same`, so that one takes `-3` instead.
#[test]
fn a_numbered_identifier_another_heading_already_holds_is_skipped() {
    let document = "\\section{Same}\n\n\\section{Same-2}\n\n\\section{Same}\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        anchor_ids(&run),
        [
            Some("sec--Same".to_string()),
            Some("sec--Same-2".to_string()),
            Some("sec--Same-3".to_string()),
        ]
    );
}

/// The anchors stage is document scope, so the numbering runs across the
/// document's trees, not within one of them.
#[test]
fn a_title_repeated_in_a_second_tree_of_the_document_is_numbered_too() {
    let run = run_workflow(
        &registry(),
        text::WORKFLOW,
        &["\\section{Same}\n", "\\section{Same}\n"],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        anchor_ids(&run),
        [Some("sec--Same".to_string()), Some("sec--Same-2".to_string())]
    );
}

/// A heading's identifier begins with `sec` whether the heading was labeled or
/// not: `sec-intro` from `\label{sec:intro}`, `sec--Examples` from the title.
#[test]
fn a_labeled_and_an_unlabeled_heading_share_the_same_identifier_prefix() {
    let document = "\\section{Intro}\\label{sec:intro}\n\n\\section{Examples}\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        anchor_ids(&run),
        [Some("sec-intro".to_string()), Some("sec--Examples".to_string())]
    );
}

/// A label whose own text begins with a hyphen used to be the one way an
/// identifier made from a label could meet one made from a title:
/// `\label{sec:-Foo}` joins `sec` and `-Foo` with one hyphen, which reads as
/// the double hyphen a title-derived identifier uses. The encoder now sends a
/// text beginning with a hyphen down its tagged path, so the two are apart
/// whichever comes first and nothing is reported either way.
#[test]
fn a_label_beginning_with_a_hyphen_cannot_meet_a_title_derived_identifier() {
    let label_first = "\\section{A}\\label{sec:-Foo}\n\n\\section{Foo}\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[label_first], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        anchor_ids(&run),
        [
            Some(refs::label_anchor(&Label::new("sec", "-Foo"))),
            Some("sec--Foo".to_string()),
        ]
    );
    let title_first = "\\section{Foo}\n\n\\section{A}\\label{sec:-Foo}\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[title_first], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        anchor_ids(&run),
        [
            Some("sec--Foo".to_string()),
            Some(refs::label_anchor(&Label::new("sec", "-Foo"))),
        ]
    );
}

/// The same hole for the other prefix: an equation line labeled `eq:-7`
/// cannot take the shape of an unlabeled line's identifier, `eq--<position>`
/// followed by four letters.
#[test]
fn a_label_beginning_with_a_hyphen_cannot_meet_a_position_derived_identifier() {
    let labeled = refs::label_anchor(&Label::new("eq", "-7"));
    assert!(!labeled.starts_with("eq--"), "{labeled}");
    assert_ne!(labeled, refs::auto_anchor("eq", 7));
}

/// A label the anchors stage must not fold: no counter anywhere, so the same
/// label always encodes to the same anchor whatever else the document holds.
#[test]
fn a_label_that_is_not_plain_ascii_anchors_at_its_tagged_encoding() {
    let document = "\\section{T}\\label{sec:\\(x\\)}\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    let anchors = run.table(&ANCHORS).expect("declared").expect("written");
    let anchor = anchors.ids.values().next().expect("one point").clone().expect("an anchor");
    assert_eq!(anchor, refs::label_anchor(&Label::new("sec", "\\(x\\)")));
    assert!(anchor.starts_with("sec-x~"));
}

#[test]
fn the_targets_table_joins_each_label_with_its_anchor_and_display() {
    let run = run_workflow(&registry(), text::WORKFLOW, &[DUPLICATE], &Overrides::none());
    let targets = run.table(&REF_TARGETS).expect("declared").expect("written");
    let target = &targets.by_label[&Label::new("sec", "more")];
    assert_eq!(target.href, "#sec-more");
    let content = target.display.content.as_ref().expect("the point's content");
    assert_eq!(plain_text(content), "More");
    // No heading kind is numbered under the shipped configuration, so the
    // heading states no number source.
    assert!(target.display.number.is_none());
}

#[test]
fn a_reference_resolves_to_its_point_and_an_unknown_one_is_recorded_unresolved() {
    let run = run_workflow(&registry(), text::WORKFLOW, &[ONE_OF_EACH], &allow_unresolved());
    assert_json_golden(golden_path("refs-resolutions.json"), &projection(&run, REF_RESOLUTIONS));
}

#[test]
fn an_unresolved_reference_is_an_error_by_default() {
    let run = run_workflow(&registry(), text::WORKFLOW, &[ONE_OF_EACH], &Overrides::none());
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.unresolved").once();
    assert_eq!(diagnostic.severity(), Severity::Error);
    let condition = diagnostic.data().downcast_ref::<Unresolved>().expect("typed");
    assert_eq!(condition.label, "sec:nowhere");
}

#[test]
fn an_unresolved_reference_is_a_warning_under_allow_and_the_text_is_written() {
    let run = run_workflow(&registry(), text::WORKFLOW, &[ONE_OF_EACH], &allow_unresolved());
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.unresolved").once();
    assert_eq!(diagnostic.severity(), Severity::Warning);
    assert_eq!(text_of(&run), "Intro\n=====\n\nSee Intro and sec:nowhere.\n");
}

#[test]
fn a_resolved_reference_lowers_to_a_link_around_its_title() {
    let run = run_workflow(&registry(), text::WORKFLOW, &[ONE_OF_EACH], &allow_unresolved());
    assert_json_golden(golden_path("refs-lowered.json"), &lowered_projection(&run));
    let lowered = lowered(&run, TreeId::tree(0));
    let link = lowered
        .tree
        .root()
        .descendants()
        .find(|node| element_of(node).name == pres::LINK)
        .expect("a link");
    let payload = payload_of::<LinkPayload>(&link).expect("a link payload");
    assert_eq!((payload.href.as_str(), payload.kind.as_str()), ("#sec-intro", refs::LINK_KIND));
}

#[test]
fn an_unresolved_reference_lowers_to_an_unresolved_styled_span_around_its_label() {
    let run = run_workflow(&registry(), text::WORKFLOW, &[ONE_OF_EACH], &allow_unresolved());
    let lowered = lowered(&run, TreeId::tree(0));
    let styles: Vec<Option<SpanStyle>> = lowered
        .tree
        .root()
        .descendants()
        .filter(|node| element_of(node).name == pres::SPAN)
        .map(|node| payload_of::<SpanPayload>(&node).and_then(|payload| payload.style))
        .collect();
    assert_eq!(styles, [Some(SpanStyle::UnresolvedReference)]);
}

#[test]
fn no_label_field_reaches_the_lowered_tree() {
    // A `\label` is data the heading reads: the heading's rule writes what it
    // read, and the field node itself is never lowered. A field that did reach
    // a backend would be the error `flm.lowering.field-not-read`.
    let run = run_workflow(&registry(), text::WORKFLOW, &[DUPLICATE], &Overrides::none());
    let lowered = lowered(&run, TreeId::tree(0));
    assert!(lowered.tree.root().descendants().all(|node| element_of(&node).name != FIELD));
    assert!(!identifiers(&run).iter().any(|id| id == "flm.lowering.field-not-read"));
}

#[test]
fn a_reference_inside_a_heading_title_resolves_through_the_spliced_value() {
    let document = "\\section{First}\\label{sec:a}\n\n\
                    \\section{After \\ref{sec:a}}\\label{sec:b}\n\n\
                    Then \\ref{sec:b}.\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        text_of(&run),
        "First\n=====\n\nAfter First\n===========\n\nThen After First.\n"
    );
}

/// A heading whose title holds a reference nobody resolves, referenced
/// twice: the title is spliced twice, and the mistake is reported where the
/// title stands and at each splice.
#[test]
fn an_unresolved_reference_inside_a_spliced_title_is_reported_at_each_splice() {
    let document = "\\section{T \\ref{sec:missing}}\\label{sec:t}\n\n\
                    A \\ref{sec:t} and B \\ref{sec:t}.\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &allow_unresolved());
    assert_eq!(identifiers(&run), ["flm.refs.unresolved"; 3]);
    assert_eq!(
        text_of(&run),
        "T sec:missing\n=============\n\nA T sec:missing and B T sec:missing.\n"
    );
}

#[test]
fn a_malformed_label_inside_a_spliced_title_is_reported_at_each_splice() {
    let document = "\\section{T \\ref{nolabel}}\\label{sec:t}\n\n\
                    A \\ref{sec:t} and B \\ref{sec:t}.\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), ["flm.refs.bad-label"; 3]);
}

/// An unnumbered heading holding a reference nobody resolves, shown by two
/// references: on text and HTML the Error is reported at the heading and
/// again at each reference showing the title; `check` lowers nothing and
/// reports the one at the heading.
#[test]
fn an_unresolved_reference_in_an_unnumbered_title_is_reported_at_each_showing() {
    let registry = common::stdcore::registry(flm_core::stdcore::StdcoreConfig::new());
    let document = "\\section*{See \\ref{sec:nowhere}}\\label{sec:b}\n\n\\ref{sec:b} and \\ref{sec:b}.\n";
    for workflow in [text::WORKFLOW, flm_core::stdcore::backends::html::WORKFLOW] {
        let run = run_workflow(&registry, workflow, &[document], &Overrides::none());
        assert_eq!(
            common::stdcore::reports(&run),
            vec![(String::from("flm.refs.unresolved"), Severity::Error); 3],
            "{workflow}: {}",
            run.diagnostics().render_all()
        );
        // Each report points at the reference where it was written.
        assert!(
            run.diagnostics().iter().all(|d| d.span().content() == "\\ref{sec:nowhere}"),
            "{workflow}: {}",
            run.diagnostics().render_all()
        );
        // The heading's own report has no frame; each of the other two is
        // raised inside the copy one `\ref{sec:b}` shows, and names it.
        let mut found = common::stdcore::frames(&run);
        found.sort();
        let first = document.find("\\ref{sec:b}").expect("a first reference");
        let second = document.rfind("\\ref{sec:b}").expect("a second reference");
        assert_eq!(
            found,
            [vec![], vec![(String::from(SHOWN_HERE), first)], vec![(String::from(SHOWN_HERE), second)]],
            "{workflow}: {}",
            run.diagnostics().render_all()
        );
    }
    let run = run_workflow(&registry, CHECK_WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), ["flm.refs.unresolved"], "{}", run.diagnostics().render_all());
}

#[test]
fn a_report_inside_a_copy_shown_inside_a_copy_names_both_places_innermost_first() {
    // `\ref{sec:b}` shows the title of `sec:b`, which holds `\ref{sec:a}`,
    // which shows the title of `sec:a`, which holds a reference to nothing.
    let registry = common::stdcore::registry(flm_core::stdcore::StdcoreConfig::new());
    let document = "\\section*{A \\ref{sec:nowhere}}\\label{sec:a}\n\n\
                    \\section*{B \\ref{sec:a}}\\label{sec:b}\n\n\\ref{sec:b}\n";
    let inner = document.find("\\ref{sec:a}").expect("the inner reference");
    let outer = document.find("\\ref{sec:b}").expect("the outer reference");
    let at = |offset| (String::from(SHOWN_HERE), offset);
    for workflow in [text::WORKFLOW, flm_core::stdcore::backends::html::WORKFLOW] {
        let run = run_workflow(&registry, workflow, &[document], &Overrides::none());
        // HTML also reports the inner reference's link standing inside the
        // outer one's, where the page is written.
        let unresolved: Vec<_> = identifiers(&run)
            .into_iter()
            .zip(common::stdcore::frames(&run))
            .filter(|(id, _)| id == "flm.refs.unresolved")
            .map(|(_, frames)| frames)
            .collect();
        let mut found = unresolved;
        found.sort();
        assert_eq!(
            found,
            [vec![], vec![at(inner)], vec![at(inner), at(outer)]],
            "{workflow}: {}",
            run.diagnostics().render_all()
        );
    }
}

#[test]
fn an_external_target_resolves_a_label_no_point_carries() {
    let registry = registry();
    let display = common::parse(registry.lang(), "Elsewhere");
    let root = display.tree().root();
    let nodes: Vec<_> = root.children().iter().collect();
    let value = refs::value_of(&nodes, &root, registry.lang().guard()).expect("copied");
    let index = ExternalIndex::new().with(
        Label::new("sec", "ext"),
        ExternalTarget { href: "other.html#sec-ext".to_string(), display: value },
    );
    let run = run_workflow_with(
        &registry,
        text::WORKFLOW,
        &["See \\ref{sec:ext}.\n"],
        &Overrides::none(),
        Externals::none().install_unstamped(EXTERNAL_INDEX, index),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let resolutions = run.table(&REF_RESOLUTIONS).expect("declared").expect("written");
    let (site, resolution) = resolutions.sites.iter().next().expect("one site");
    assert_eq!(site.index, 0);
    assert!(matches!(
        resolution,
        Resolution::Resolved { point: None, href, .. } if href == "other.html#sec-ext"
    ));
    assert_eq!(text_of(&run), "See Elsewhere <other.html#sec-ext>.\n");
}

/// A `\ref` may name several targets at once, separated by commas. Nothing
/// here is numbered — no heading kind is numbered by default — so each target
/// shows its own title, and the whole reads as one list: two items joined by
/// ` and `.
#[test]
fn several_targets_of_one_reference_are_shown_as_one_list() {
    let document = "\\section{Intro}\\label{sec:a}

\\section{More}\\label{sec:b}

                    See \\ref{sec:a,sec:b}.
";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert!(text_of(&run).contains("See Intro and More."), "{}", text_of(&run));
}

/// Each target of a reference declares a reference site of its own, and each
/// site resolves on its own: one link per target, and a target nothing
/// resolves is reported and shown unresolved beside the others.
#[test]
fn each_target_of_a_reference_is_a_site_that_resolves_on_its_own() {
    let document = "\\section{Intro}\\label{sec:a}

                    See \\ref{sec:a,sec:nowhere}.
";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &allow_unresolved());
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.unresolved").once();
    assert_eq!(diagnostic.data().downcast_ref::<Unresolved>().expect("typed").label, "sec:nowhere");
    let resolutions = run.table(&REF_RESOLUTIONS).expect("declared").expect("written");
    let sites: Vec<SiteId> = resolutions.sites.keys().copied().collect();
    assert_eq!(sites.iter().map(|site| site.index).collect::<Vec<_>>(), [0, 1]);
    assert!(text_of(&run).contains("See Intro and sec:nowhere."), "{}", text_of(&run));
}

#[test]
fn a_reference_site_reads_its_display_options_from_the_keyval_argument() {
    let registry = registry();
    let fragment = common::parse(registry.lang(), "\\ref[noprefix,Capital]{sec:intro}");
    let node = common::callables(&fragment)[0];
    let sites = element_of(&node).aspect::<dyn Referencing>().expect("referencing").sites(&node);
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].target, Label::new("sec", "intro"));
    assert!(!sites[0].display.with_prefix);
    assert_eq!(sites[0].display.variant.as_deref(), Some("Capital"));
}

#[test]
fn a_label_and_a_reference_holding_a_quote_meet_as_typed() {
    // The second label's doubled space meets the reference's one space: a run
    // of whitespace in a label counts as one space.
    let document = "\\section{Intro}\\label{sec:a \"b}\n\n\\section{Next}\\label{sec:c  d}\n\n\
        See \\ref{sec:a \"b} and \\ref{sec:c d}.\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let labels = run.table(&LABELS).expect("declared").expect("written");
    assert_eq!(
        labels.by_label.keys().cloned().collect::<Vec<_>>(),
        [Label::new("sec", "a \"b"), Label::new("sec", "c d")]
    );
    let resolutions = run.table(&REF_RESOLUTIONS).expect("declared").expect("written");
    let resolved: Vec<bool> =
        resolutions.sites.values().map(|resolution| matches!(resolution, Resolution::Resolved { .. })).collect();
    assert_eq!(resolved, [true, true]);
}

#[test]
fn a_malformed_label_is_reported_at_its_field_and_ignored() {
    // The heading's own reader parses the spelling as it reads the field, so
    // the report stands at the field, not at the heading.
    let run = run_workflow(
        &registry(),
        text::WORKFLOW,
        &["\\section{X}\\label{sec:ok}\\label{nolabel}\n"],
        &Overrides::none(),
    );
    let diagnostic =
        expect(run.diagnostics().iter(), "flm.refs.bad-label").at("\\label{nolabel}").once();
    assert_eq!(diagnostic.data().downcast_ref::<BadLabel>().expect("typed").text, "nolabel");
    // The well-formed label of the same heading is unaffected; the malformed
    // one names nothing.
    let labels = run.table(&LABELS).expect("declared").expect("written");
    assert_eq!(labels.by_label.keys().map(ToString::to_string).collect::<Vec<_>>(), ["sec:ok"]);
    let points = run.table(&REFPOINTS).expect("declared").expect("written");
    let point = &points.records().first().expect("the heading's point").point;
    assert_eq!(point.malformed.iter().map(|m| m.text.clone()).collect::<Vec<_>>(), ["nolabel"]);
}

#[test]
fn a_malformed_reference_target_is_reported_and_shown_unresolved() {
    let run =
        run_workflow(&registry(), text::WORKFLOW, &["See \\ref{nolabel}.\n"], &Overrides::none());
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.bad-label").once();
    assert_eq!(diagnostic.data().downcast_ref::<BadLabel>().expect("typed").text, "nolabel");
    assert_eq!(text_of(&run), "See nolabel.\n");
}

#[test]
fn the_check_workflow_reports_a_duplicate_label() {
    let run = run_workflow(&registry(), CHECK_WORKFLOW, &[DUPLICATE], &Overrides::none());
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.duplicate-label").once();
    let condition = diagnostic.data().downcast_ref::<DuplicateLabelCondition>().expect("typed");
    assert_eq!(condition.label, "sec:intro");
}

/// The `check` workflow warns about an unresolved reference: a check plan
/// renders nothing, so all it can say is that the reference *might* fail to
/// resolve. The render option plays no part.
#[test]
fn the_check_workflow_reports_an_unresolved_reference_as_a_warning() {
    let run = run_workflow(&registry(), CHECK_WORKFLOW, &[ONE_OF_EACH], &Overrides::none());
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.unresolved").once();
    assert_eq!(diagnostic.severity(), Severity::Warning);
    assert!(!run.diagnostics().has_errors());
}

/// The severity of an unresolved reference under `check` does not follow the
/// render options: a request that asks for Errors still gets a Warning there,
/// while the `text` workflow keeps reporting an Error.
#[test]
fn the_check_workflow_warns_where_the_text_workflow_errors() {
    let registry = registry();
    let errors = Overrides {
        render_options: OptionValues::new()
            .with(RefsRenderOptions { unresolved: UnresolvedRefs::Error }),
        ..Overrides::none()
    };
    let checked = run_workflow(&registry, CHECK_WORKFLOW, &[ONE_OF_EACH], &errors);
    assert_eq!(
        expect(checked.diagnostics().iter(), "flm.refs.unresolved").once().severity(),
        Severity::Warning
    );
    let rendered = run_workflow(&registry, text::WORKFLOW, &[ONE_OF_EACH], &errors);
    assert_eq!(
        expect(rendered.diagnostics().iter(), "flm.refs.unresolved").once().severity(),
        Severity::Error
    );
}

#[test]
fn the_label_check_is_a_member_of_the_validated_collection() {
    let registry = registry();
    let collection = registry.collection(VALIDATED).expect("declared by the check feature");
    let member = registry.cap_id(CHECK_LABELS).expect("declared by refs");
    assert!(collection.members.contains(&member));
}

#[test]
fn a_site_is_keyed_by_the_node_address_and_its_index() {
    let run = run_workflow(&registry(), text::WORKFLOW, &[ONE_OF_EACH], &allow_unresolved());
    let resolutions = run.table(&REF_RESOLUTIONS).expect("declared").expect("written");
    let sites: Vec<SiteId> = resolutions.sites.keys().copied().collect();
    assert_eq!(sites.len(), 2);
    assert!(sites.iter().all(|site| site.index == 0 && site.addr.tree == TreeId::tree(0)));
}

// ---------------------------------------------------------------------------------------------
// `\label` written in an argument: the scoped package `flm.refs.label`
// ---------------------------------------------------------------------------------------------

/// The registry of these tests with a test feature declaring `\tnote[…]{…}`,
/// whose optional argument names the scoped package `flm.refs.label`.
fn registry_with_label_argument() -> Arc<FlmRegistry> {
    use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
    use flm_core::model::element::ElementSpec;
    let note = common::pipeline::feature("test-note", |b| {
        b.peer_scoped_package(refs::LABEL_PACKAGE, "the labels of a note")?;
        b.element(ElementDef::new(
            b.feature().clone(),
            ["tnote"],
            SyntaxDef::Macro {
                args: vec![
                    ArgDef::optional("labels").scoped(refs::LABEL_PACKAGE),
                    ArgDef::mandatory("text"),
                ],
            },
            Arc::new(ElementSpec::new("test.note")),
        ))
    });
    registry_over(vec![
        lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        Arc::new(Blocks::default()),
        check_feature(),
        index::feature(),
        refpoints::feature(),
        refs::feature(),
        headings::feature(),
        note,
    ])
}

#[test]
fn label_uses_in_reads_a_label_in_a_run_as_label_uses_reads_a_tack_on_one() {
    use flm_core::lang::mint::field_name;
    use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
    use flm_core::model::doc::FragmentLayout;
    let registry = registry_with_label_argument();
    let source = "\\section{T}\\label{x:a}\\label{nolabel}\n\n\\tnote[\\label{x:a} \\label{nolabel}]{n}";
    let fragment = parse_fragment(
        registry.lang(),
        Arc::new(FlmSource::new(source).with_origin(FlmOrigin::none())),
        FragmentLayout::default(),
        &ParseOptions::default(),
    )
    .expect("a parse");
    let root = fragment.tree().root();
    let heading = root.descendants().find(|n| n.name() == Some("section")).expect("the heading");
    let note = root.descendants().find(|n| n.name() == Some("tnote")).expect("the note");
    let run = note.argument_content_nodes_named("labels").expect("readable").expect("written");
    // Every `\label` of the run is a field node named `label`, as a tack-on
    // one is.
    let fields: Vec<_> = run.iter().filter(|node| element_of(node).name == FIELD).collect();
    assert_eq!(fields.len(), 2);
    assert!(fields.iter().all(|node| field_name(node).is_some_and(|name| name == "label")));
    let tack_on = refs::label_uses(&heading);
    let in_run = refs::label_uses_in(run.iter());
    let read = |uses: &[refs::LabelUse]| -> Vec<(String, Option<Label>)> {
        uses.iter().map(|one| (one.text.clone(), one.label.clone())).collect()
    };
    assert_eq!(read(&in_run), read(&tack_on));
    assert_eq!(read(&in_run), [("x:a".to_string(), Some(Label::new("x", "a"))), ("nolabel".to_string(), None)]);
    // The same split into a point's labels and its malformed spellings.
    let (labels, malformed) = refs::point_labels(&in_run);
    assert_eq!(labels, [Label::new("x", "a")]);
    assert_eq!(malformed.len(), 1);
    assert_eq!(malformed[0].position, 1);
}

#[test]
fn a_label_outside_an_argument_naming_the_package_is_an_unknown_command() {
    let registry = registry_with_label_argument();
    let run = run_workflow(&registry, CHECK_WORKFLOW, &["\\tnote{x\\label{x:a}} \\label{x:b}"], &Overrides::none());
    assert_eq!(identifiers(&run), ["core.specs.unresolvable-command"; 2]);
}
