//! Label types checked against the labeled element
//! (`flm_core::model::aspects::AcceptedRefTypes`): a reference point declares
//! which label types may be written for it, a heading accepts `sec` and
//! `topic`, a point declaring `Any` accepts every type, and a label of a type
//! its point does not accept defines nothing — it is recorded in `flm.labels`,
//! reported by the label check as `flm.refs.label-type-not-accepted` under every
//! workflow, a rendering one included, and no reference resolves through it.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry,
};
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, BlockRole, BlockRoleAspect, ContentRef, FixedBlockRole,
    Label, RefPoint, Referenceable,
};
use flm_core::model::element::ElementSpec;
use flm_core::dump::diag::expect;
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::refs::{
    ANCHORS, AnchorCollision, LABELS, LabelTypeNotAccepted, REF_TARGETS, Unresolved,
};
use flm_core::stdcore::features::refpoints::{self, REFPOINTS};
use flm_core::stdcore::features::{headings, index, refs};
use flm_core::stdcore::backends::{lowering, text};
use flm_core::stdcore::{CHECK_WORKFLOW, check_feature};
use techy::core::node::NodeRef;
use techy::error::Severity;

use common::stdcore::{identifiers, registry_over, run_workflow};

/// A registry over the features a labeled document needs: the headings (the
/// standard labeled element), the references, the index, the lowering
/// machinery and the text backend, the block structure and the check feature
/// (whose workflow runs the label check).
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

// --- a point accepting every label type ----------------------------------------------

/// The element type name of the test-only labeled element, `x.point`.
const POINT: &str = "x.point";

/// A test-only reference point that accepts every label type, which is what a
/// feature declares for a construct whose label types are not fixed in
/// advance.
#[derive(Debug)]
struct AnyPoint;

impl Referenceable for AnyPoint {
    fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        let (labels, malformed) = refs::point_labels(refs::label_uses(node).iter());
        vec![RefPoint {
            kind: "point".into(),
            labels,
            malformed,
            computed: 0,
            anchor: AnchorPolicy::FromFirstLabel,
            number: None,
            content: Some(ContentRef::Argument(0)),
            accepts: AcceptedRefTypes::any(),
        }]
    }
}

/// A test-only feature registering `\point{text}\label{…}`, whose one
/// reference point accepts every label type.
#[derive(Debug)]
struct PointFeature;

impl FlmFeature for PointFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("x-point", "A labeled element accepting any label type")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        let value = Arc::new(AnyPoint);
        b.element(ElementDef::new(
            owner,
            ["point"],
            SyntaxDef::Macro { args: vec![ArgDef::mandatory("text"), refs::label_fields_arg()] },
            Arc::new(
                ElementSpec::new(POINT)
                    .docs("a labeled element accepting every label type")
                    .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
                    .with::<dyn Referenceable>(value as Arc<dyn Referenceable>),
            ),
        ))?;
        Ok(())
    }
}

/// The registry of [`registry`] with the test-only labeled element added.
fn registry_with_any_point() -> Arc<FlmRegistry> {
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
        Arc::new(PointFeature),
    ])
}

// --- the accepted types pass ---------------------------------------------------------

/// The two label types a heading accepts both define a label, anchor the
/// heading and resolve.
#[test]
fn a_heading_accepts_a_sec_and_a_topic_label() {
    let document = "\\section{Intro}\\label{sec:intro}\n\n\
                    \\section{Codes}\\label{topic:codes}\n\n\
                    See \\ref{sec:intro} and \\ref{topic:codes}.\n";
    let run = run_workflow(&registry(), CHECK_WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let targets = run.table(&REF_TARGETS).expect("declared").expect("written");
    assert_eq!(targets.by_label[&Label::new("sec", "intro")].href, "#sec-intro");
    assert_eq!(targets.by_label[&Label::new("topic", "codes")].href, "#topic-codes");
    let labels = run.table(&LABELS).expect("declared").expect("written");
    assert!(labels.refused.is_empty());
}

/// `headings::LABEL_TYPES` is what the heading's reference point declares, so
/// the documented list and the enforced one cannot drift apart.
#[test]
fn the_heading_declares_the_documented_label_types() {
    assert_eq!(headings::LABEL_TYPES, ["sec", "topic"]);
    let accepted = AcceptedRefTypes::these(headings::LABEL_TYPES);
    assert!(accepted.accepts("sec"));
    assert!(accepted.accepts("topic"));
    assert!(!accepted.accepts("eq"));
}

// --- a type the element does not accept ----------------------------------------------

/// A heading labeled `eq:x`: the label check reports
/// `flm.refs.label-type-not-accepted` naming the label, the kind of thing it
/// was written on, and the types that thing accepts.
#[test]
fn a_label_of_a_type_a_heading_does_not_accept_is_reported_with_the_accepted_types() {
    let document = "\\section{Sum}\\label{eq:sum}\n";
    let run = run_workflow(&registry(), CHECK_WORKFLOW, &[document], &Overrides::none());
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.label-type-not-accepted").once();
    assert_eq!(diagnostic.severity(), Severity::Error);
    let condition = diagnostic.data().downcast_ref::<LabelTypeNotAccepted>().expect("typed");
    assert_eq!(condition.label, "eq:sum");
    assert_eq!(condition.kind, "section");
    assert_eq!(condition.accepted, ["sec".to_string(), "topic".to_string()]);
}

/// The scan records the refusal in `flm.labels`, which is what the check
/// reads: the label is not in the label index, and the point that wrote it is
/// named beside the types it accepts.
#[test]
fn a_refused_label_is_recorded_in_the_labels_table_and_joins_no_index() {
    let document = "\\section{Sum}\\label{eq:sum}\\label{sec:sum}\n";
    let run = run_workflow(&registry(), CHECK_WORKFLOW, &[document], &Overrides::none());
    let labels = run.table(&LABELS).expect("declared").expect("written");
    assert_eq!(labels.refused.len(), 1);
    let refusal = &labels.refused[0];
    assert_eq!(refusal.label, Label::new("eq", "sum"));
    assert_eq!(refusal.kind.as_ref(), "section");
    assert_eq!(refusal.accepted, ["sec".to_string(), "topic".to_string()]);
    assert!(!labels.by_label.contains_key(&Label::new("eq", "sum")));
    assert!(labels.by_label.contains_key(&Label::new("sec", "sum")));
    // The point wrote an accepted label too, so it keeps that one's anchor.
    let points = run.table(&REFPOINTS).expect("declared").expect("written");
    assert_eq!(points.records()[0].id, refusal.point);
}

/// A refused label defines nothing: no target carries it, so a reference to it
/// is unresolved, and the heading whose only label was refused gets no anchor
/// of its own.
#[test]
fn a_refused_label_defines_nothing_so_a_reference_to_it_is_unresolved() {
    let document = "\\section{Sum}\\label{eq:sum}\n\nSee \\ref{eq:sum}.\n";
    let run = run_workflow(&registry(), CHECK_WORKFLOW, &[document], &Overrides::none());
    let targets = run.table(&REF_TARGETS).expect("declared").expect("written");
    assert!(targets.by_label.is_empty());
    let unresolved = expect(run.diagnostics().iter(), "flm.refs.unresolved").once();
    let condition = unresolved.data().downcast_ref::<Unresolved>().expect("typed");
    assert_eq!(condition.label, "eq:sum");
    let anchors = run.table(&ANCHORS).expect("declared").expect("written");
    let ids: Vec<Option<&str>> = anchors.ids.values().map(|id| id.as_deref()).collect();
    assert_eq!(ids, [None]);
}

/// A rendering workflow demands the validation collection beside its output,
/// so it runs the label check too: the refusal is reported there exactly as it
/// is under `check`, beside the reference that the refused label leaves
/// undefined.
#[test]
fn a_rendering_reports_the_refused_label_and_the_reference_to_it() {
    let document = "\\section{Sum}\\label{eq:sum}\n\nSee \\ref{eq:sum}.\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), ["flm.refs.unresolved", "flm.refs.label-type-not-accepted"]);
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.label-type-not-accepted").once();
    assert_eq!(diagnostic.severity(), Severity::Error);
}

// --- a point accepting every type ----------------------------------------------------

/// A point declaring `AcceptedRefTypes::Any` takes a label of any type at all,
/// including one no standard element accepts.
#[test]
fn a_point_that_accepts_any_type_takes_every_label() {
    let document = "\\point{A}\\label{eq:a} and \\point{B}\\label{whatever:b}.\n\n\
                    See \\ref{eq:a} and \\ref{whatever:b}.\n";
    let run =
        run_workflow(&registry_with_any_point(), CHECK_WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let labels = run.table(&LABELS).expect("declared").expect("written");
    assert!(labels.refused.is_empty());
    let targets = run.table(&REF_TARGETS).expect("declared").expect("written");
    assert_eq!(targets.by_label[&Label::new("eq", "a")].href, "#eq-a");
    assert_eq!(targets.by_label[&Label::new("whatever", "b")].href, "#whatever-b");
}

/// Two labels that encode to one anchor (`a-b:c` and `a:b-c` both give
/// `a-b-c`) are a collision: the first point keeps the anchor and the target,
/// the second gets neither. The two label types are of no standard element, so
/// the points are the test-only ones, which accept every type.
#[test]
fn two_labels_encoding_the_same_anchor_collide_on_a_point_accepting_any_type() {
    let document = "\\point{One}\\label{a-b:c} and \\point{Two}\\label{a:b-c}.\n";
    let run =
        run_workflow(&registry_with_any_point(), CHECK_WORKFLOW, &[document], &Overrides::none());
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.anchor-collision").once();
    assert_eq!(diagnostic.severity(), Severity::Error);
    let condition = diagnostic.data().downcast_ref::<AnchorCollision>().expect("typed");
    assert_eq!(condition.anchor, "a-b-c");
    assert_eq!((condition.first.as_str(), condition.second.as_str()), ("a-b:c", "a:b-c"));
    let targets = run.table(&REF_TARGETS).expect("declared").expect("written");
    assert!(targets.by_label.contains_key(&Label::new("a-b", "c")));
    assert!(!targets.by_label.contains_key(&Label::new("a", "b-c")));
}
