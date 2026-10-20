//! `\hyperref[type:label]{text}` of the `refs` feature
//! (`flm_core::stdcore::features::refs`): a reference whose display is the
//! text its author wrote. It is scanned, resolved and checked exactly as a
//! `\ref` is — the same stages, the same tables, the same reports — and
//! differs only in what stands inside the link: the author's text, lowered as
//! the ordinary inline content it is, markup and all. A target that resolves
//! against nothing, and one that is not spelled `type:label` at all, keep the
//! text and lose the link.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::dump::diag::expect;
use flm_core::dump::golden::assert_json_golden;
use flm_core::model::addr::TreeId;
use flm_core::model::aspects::{Label, Referencing};
use flm_core::model::element::element_of;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::pres::{self, LinkPayload, SpanPayload, SpanStyle, payload_of};
use flm_core::stdcore::backends::{lowering, text};
use flm_core::stdcore::check_feature;
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::refs::{
    self, HYPERREF, REF_RESOLUTIONS, RefsRenderOptions, Resolution, Unresolved, UnresolvedRefs,
};
use flm_core::stdcore::features::{headings, index, refpoints, textstyles};
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig};
use techy::error::Severity;

use common::lowering::{lowered, lowered_projection, text as text_of};
use common::stdcore::{
    golden_path, identifiers, registry_over, run_workflow,
};

/// The registry of `stdcore_refs.rs` with the inline formatting commands
/// added: the `refs` feature with the labeled construct (`headings`), the
/// element index, the lowering machinery, the text backend, block structure,
/// the `check` feature, and `textstyles`, which is what makes the `\emph`
/// written inside a `\hyperref`'s text ordinary content rather than an
/// unattributed macro.
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
        textstyles::feature(),
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

/// A labeled heading, a `\hyperref` to it whose text holds markup, and a
/// `\hyperref` to nothing.
const ONE_OF_EACH: &str = "\\section{Intro}\\label{sec:intro}\n\n\
                           See \\hyperref[sec:intro]{the \\emph{first} part} and \
                           \\hyperref[sec:nowhere]{nothing}.\n";

#[test]
fn a_resolved_hyperref_lowers_to_a_link_around_the_authors_text() {
    let run = run_workflow(&registry(), text::WORKFLOW, &[ONE_OF_EACH], &allow_unresolved());
    let lowered = lowered(&run, TreeId::tree(0));
    let link = lowered
        .tree
        .root()
        .descendants()
        .find(|node| element_of(node).name == pres::LINK)
        .expect("a link");
    let payload = payload_of::<LinkPayload>(&link).expect("a link payload");
    // The same link target and the same kind a `\ref` produces: what changed
    // is only what stands inside.
    assert_eq!((payload.href.as_str(), payload.kind.as_str()), ("#sec-intro", refs::LINK_KIND));
}

/// The lowered tree of the two references, as a golden: the author's text
/// inside the link with its emphasis kept as a span of its own, and the text
/// of the unresolved one inside the unresolved-styled span.
#[test]
fn the_lowered_tree_holds_the_authors_text_inside_the_link() {
    let run = run_workflow(&registry(), text::WORKFLOW, &[ONE_OF_EACH], &allow_unresolved());
    assert_json_golden(golden_path("refs-hyperref-lowered.json"), &lowered_projection(&run));
}

#[test]
fn the_text_backend_prints_the_authors_text_for_a_hyperref() {
    let run = run_workflow(&registry(), text::WORKFLOW, &[ONE_OF_EACH], &allow_unresolved());
    // The text backend writes a link's target only where it leaves the
    // document; an anchor of this document is not printed.
    assert_eq!(text_of(&run), "Intro\n=====\n\nSee the first part and nothing.\n");
}

#[test]
fn an_unresolved_hyperref_keeps_its_text_in_an_unresolved_styled_span() {
    let run = run_workflow(&registry(), text::WORKFLOW, &[ONE_OF_EACH], &allow_unresolved());
    let lowered = lowered(&run, TreeId::tree(0));
    let styles: Vec<Option<SpanStyle>> = lowered
        .tree
        .root()
        .descendants()
        .filter(|node| element_of(node).name == pres::SPAN)
        .map(|node| payload_of::<SpanPayload>(&node).and_then(|payload| payload.style))
        .collect();
    // Two spans: the emphasis inside the resolved link, lowered as the
    // ordinary content it is, and the unresolved reference's own.
    assert_eq!(styles, [Some(SpanStyle::Emphasis), Some(SpanStyle::UnresolvedReference)]);
}

#[test]
fn an_unresolved_hyperref_is_an_error_by_default_and_names_its_label() {
    let run = run_workflow(&registry(), text::WORKFLOW, &[ONE_OF_EACH], &Overrides::none());
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.unresolved").once();
    assert_eq!(diagnostic.severity(), Severity::Error);
    let condition = diagnostic.data().downcast_ref::<Unresolved>().expect("typed");
    assert_eq!(condition.label, "sec:nowhere");
}

/// A `\hyperref` names a label the labeled construct refused for its type, so
/// the label defines nothing: the reference resolves against nothing and the
/// label check reports the refusal, exactly as it does for a `\ref`.
#[test]
fn a_hyperref_to_a_label_of_a_refused_type_is_unresolved_and_the_label_is_reported() {
    let document = "\\section{Intro}\\label{eq:intro}\n\nSee \\hyperref[eq:intro]{it}.\n";
    let run = run_workflow(&registry(), CHECK_WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), ["flm.refs.unresolved", "flm.refs.label-type-not-accepted"]);
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &allow_unresolved());
    assert_eq!(text_of(&run), "Intro\n=====\n\nSee it.\n");
}

/// A `\hyperref` written inside a heading title resolves like one written in
/// the body: the title is copied into the target's display value, and the
/// resolver is re-entered over that value by value catch-up.
#[test]
fn a_hyperref_inside_a_heading_title_resolves_through_the_spliced_value() {
    let document = "\\section{First}\\label{sec:a}\n\n\
                    \\section{After \\hyperref[sec:a]{that}}\\label{sec:b}\n\n\
                    Then \\ref{sec:b}.\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        text_of(&run),
        "First\n=====\n\nAfter that\n==========\n\nThen After that.\n"
    );
}

#[test]
fn a_hyperref_site_is_recorded_with_the_prefix_off() {
    let registry = registry();
    let fragment = common::parse(registry.lang(), "\\hyperref[sec:intro]{here}");
    let node = common::callables(&fragment)[0];
    let sites = element_of(&node).aspect::<dyn Referencing>().expect("referencing").sites(&node);
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].target, Label::new("sec", "intro"));
    assert!(sites[0].shown);
    // What a `\hyperref` shows is the author's text, so nothing is put before
    // it: no prefix, and no variant to spell one with.
    assert!(!sites[0].display.with_prefix);
    assert_eq!(sites[0].display.variant, None);
}

/// A target that is not spelled `type:label` — and an absent one, which reads
/// as the empty text — names nothing: the scan reports it once, the text is
/// still shown, and no site is recorded.
#[test]
fn a_target_that_is_no_label_is_reported_and_the_text_is_still_shown() {
    let document = "See \\hyperref[nolabel]{a} and \\hyperref{b}.\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), ["flm.refs.bad-label", "flm.refs.bad-label"]);
    let reported = expect(run.diagnostics().iter(), "flm.refs.bad-label").count(2);
    let texts: Vec<String> = reported
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            diagnostic.data().downcast_ref::<refs::BadLabel>().expect("typed").text.clone()
        })
        .collect();
    assert_eq!(texts, ["nolabel", ""]);
    assert_eq!(text_of(&run), "See a and b.\n");
    let resolutions = run.table(&REF_RESOLUTIONS).expect("declared").expect("written");
    assert!(resolutions.sites.is_empty());
}

/// The element the `\hyperref` callable carries, so that later packages name
/// it rather than guess it.
#[test]
fn the_hyperref_callable_carries_the_hyperref_element() {
    let registry = registry();
    let fragment = common::parse(registry.lang(), "\\hyperref[sec:a]{x}");
    let node = common::callables(&fragment)[0];
    assert_eq!(element_of(&node).name.as_str(), HYPERREF);
    assert_eq!(HYPERREF, "flm.hyperref");
}

/// A `\hyperref` to a line of a display-math environment, over the whole
/// standard package: a line is a reference point like a heading, and what a
/// `\hyperref` shows for it is the author's text — where a `\ref` to the
/// same line shows the number the document gave that line.
#[test]
fn a_hyperref_to_an_equation_line_shows_the_authors_text_where_a_ref_shows_the_number() {
    let registry = common::stdcore::registry(StdcoreConfig::new());
    let document = "\\begin{equation}\n  x = 1 \\label{eq:b}\n\\end{equation}\n\nSee \\ref{eq:b} and \\hyperref[eq:b]{the equation}.\n";
    let run = run_workflow(&registry, text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert!(
        text_of(&run).contains("See Eq.\u{a0}(1) and the equation."),
        "{}",
        text_of(&run)
    );
}

/// Both references resolve to one point, and each records a site of its own.
#[test]
fn a_ref_and_a_hyperref_to_one_label_each_record_a_site() {
    let document = "\\section{Intro}\\label{sec:intro}\n\n\
                    See \\ref{sec:intro} and \\hyperref[sec:intro]{here}.\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    let resolutions = run.table(&REF_RESOLUTIONS).expect("declared").expect("written");
    assert_eq!(resolutions.sites.len(), 2);
    assert!(resolutions.sites.values().all(|resolution| matches!(
        resolution,
        Resolution::Resolved { href, .. } if href == "#sec-intro"
    )));
    assert_eq!(text_of(&run), "Intro\n=====\n\nSee Intro and here.\n");
}
