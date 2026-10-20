//! The `refpoints` feature (`flm_core::stdcore::features::refpoints`): the one
//! stage that asks every node for its reference points and records them whole
//! into `flm.refpoints`, which the `refs` and `numbering` features require; and
//! the content a point carries as a tree of its own, which the `lists` feature
//! fills so that a reference to an item shows what stands beside the item — the
//! bullet, the tag or the list's own label, however it is written.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use flm_core::lang::text::plain_text;
use flm_core::core::plan::{Overrides, PlanError};
use flm_core::core::run::Run;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry, RegistryBuilder,
    RegistryInputs,
};
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::model::aspects::{
    AcceptedRefTypes, AnchorPolicy, BlockRole, BlockRoleAspect, ContentRef, FixedBlockRole, Label,
    NumberSource, RefPoint, Referenceable,
};
use flm_core::model::element::ElementSpec;
use flm_core::stdcore::backends::{html, lowering, text};
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::math;
use flm_core::stdcore::features::refpoints::{self, REFPOINTS};
use flm_core::stdcore::features::refs::{self, LABELS, REF_TARGETS};
use flm_core::stdcore::features::{headings, index, lists, numbering};
use flm_core::stdcore::features::numbering::COUNTERS;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig, check_feature, package};
use techy::core::node::NodeRef;

use common::lowering::text as text_of;
use common::stdcore::{identifiers, registry, registry_over, run_workflow, try_plan};

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// The printed text of `document` under the standard package's `text`
/// workflow; the run must report nothing.
fn printed(document: &str) -> String {
    let run = run_workflow(&std(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    text_of(&run)
}

/// The HTML fragment of `document` under the standard package's `html`
/// workflow; the run must report nothing.
fn html_of(document: &str) -> String {
    let run = run_workflow(&std(), html::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    common::html::fragment(&run)
}

/// The last paragraph of a printed text.
fn last_paragraph(out: &str) -> String {
    out.trim_end().rsplit("\n\n").next().expect("a paragraph").to_string()
}

// ---------------------------------------------------------------------------------------------
// a reference to an item shows what stands beside it
// ---------------------------------------------------------------------------------------------

/// The three cases the label beside the item is no node of the tree: the
/// bullet a kind's configuration writes, a tag written without braces after
/// its key, and a list's own label written the same way.
const THREE_CASES: &str = "\\begin{itemize}\\item\\label{item:x} first\\end{itemize}\n\n\
                           \\begin{enumerate}\\item[tag=Custom]\\label{item:y} second\\end{enumerate}\n\n\
                           \\begin{itemize}[label=*]\\item\\label{item:z} third\\end{itemize}\n\n\
                           See \\ref{item:x}, \\ref{item:y}, \\ref{item:z}.";

#[test]
fn a_reference_to_an_item_shows_the_bullet_the_unbraced_tag_and_the_unbraced_label() {
    let out = printed(THREE_CASES);
    assert!(out.starts_with("\u{2022} first\n\nCustom second\n\n* third\n\n"), "{out}");
    assert_eq!(last_paragraph(&out), "See \u{2022}, Custom, *.");
}

#[test]
fn on_html_the_three_references_link_to_the_items_and_show_their_labels() {
    let out = html_of(THREE_CASES);
    for (anchor, shown) in [("item-x", "\u{2022}"), ("item-y", "Custom"), ("item-z", "*")] {
        let link = format!("href=\"#{anchor}\" data-flm-kind=\"ref\">{shown}</a>");
        assert!(out.contains(&link), "{link}: {out}");
    }
}

#[test]
fn braces_around_a_value_change_nothing_a_reference_shows() {
    // Braces delimit a key's value; the tag and the list's own label read the
    // same with and without them, and so does a reference to the item.
    let out = printed(
        "\\begin{description}\\item[tag=Custom]\\label{item:a} a\
         \\item[tag={Custom}]\\label{item:b} b\\item[Custom]\\label{item:c} c\\end{description}\n\n\
         \\begin{itemize}[label=*]\\item\\label{item:d} d\\end{itemize}\n\n\
         \\begin{itemize}[label={*}]\\item\\label{item:e} e\\end{itemize}\n\n\
         \\begin{itemize}[*]\\item\\label{item:f} f\\end{itemize}\n\n\
         \\ref{item:a} \\ref{item:b} \\ref{item:c} \\ref{item:d} \\ref{item:e} \\ref{item:f}",
    );
    assert_eq!(last_paragraph(&out), "Custom Custom Custom * * *");
}

#[test]
fn a_reference_to_a_nested_item_shows_the_bullet_of_its_depth() {
    let out = printed(
        "\\begin{itemize}\\item a\\begin{itemize}\\item\\label{item:b} b\\end{itemize}\
         \\end{itemize}\n\n\\ref{item:b}",
    );
    assert_eq!(last_paragraph(&out), "-");
}

#[test]
fn a_tag_holding_markup_is_shown_by_a_reference_as_the_item_shows_it() {
    let out = html_of(
        "\\begin{description}\\item[tag=\\emph{New} term]\\label{item:n} n\\end{description}\n\n\
         \\ref{item:n}",
    );
    let link = "href=\"#item-n\" data-flm-kind=\"ref\">\
                <em class=\"flm-pres-span\" data-flm-style=\"emph\">New</em> term</a>";
    assert!(out.contains(link), "{out}");
}

#[test]
fn a_reference_inside_a_tag_resolves_when_a_reference_to_the_item_shows_the_tag() {
    // The tag is content copied into a tree of its own; showing it brings that
    // tree up to date, which resolves the reference it holds as one in the
    // body.
    for tag in ["tag={see \\ref{sec:intro}}", "tag=see \\ref{sec:intro}"] {
        let out = printed(&format!(
            "\\section{{Intro}}\\label{{sec:intro}}\n\n\
             \\begin{{description}}\\item[{tag}]\\label{{item:t}} t\\end{{description}}\n\n\
             \\ref{{item:t}}"
        ));
        assert_eq!(last_paragraph(&out), "see Intro", "{tag}: {out}");
    }
}

// ---------------------------------------------------------------------------------------------
// the table of reference points
// ---------------------------------------------------------------------------------------------

/// The synthesized content a point of the table states, by the label it
/// carries.
fn synthesized(run: &Run, label: &str) -> Arc<techy::core::node::NodeTree<Flm>> {
    let points = run.table(&REFPOINTS).expect("declared").expect("written");
    let label = Label::parse(label).expect("a label");
    let record = points
        .records()
        .iter()
        .find(|record| record.point.labels.contains(&label))
        .unwrap_or_else(|| panic!("no point labeled `{label}`"));
    match &record.point.content {
        Some(ContentRef::Synthesized(tree)) => Arc::clone(tree),
        other => panic!("`{label}` states {other:?}"),
    }
}

#[test]
fn every_item_showing_the_bullet_shares_one_tree_and_a_reference_shares_it_too() {
    let run = run_workflow(
        &std(),
        text::WORKFLOW,
        &["\\begin{itemize}\\item\\label{item:a} a\\item\\label{item:b} b\\end{itemize}\n\n\
           \\begin{itemize}\\item\\label{item:c} c\
           \\begin{itemize}\\item\\label{item:d} d\\end{itemize}\\end{itemize}\n\n\
           \\ref{item:a}"],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let (a, b, c, d) = (
        synthesized(&run, "item:a"),
        synthesized(&run, "item:b"),
        synthesized(&run, "item:c"),
        synthesized(&run, "item:d"),
    );
    // One tree per configured label, whatever list the item stands in.
    assert!(Arc::ptr_eq(&a, &b) && Arc::ptr_eq(&a, &c));
    assert!(!Arc::ptr_eq(&a, &d), "the second depth writes its own label");
    assert_eq!(plain_text(&a), "\u{2022}");
    assert_eq!(plain_text(&d), "-");
    // The target a reference resolves to shows that very tree, not a copy.
    let targets = run.table(&REF_TARGETS).expect("declared").expect("written");
    let target = &targets.by_label[&Label::new("item", "a")];
    assert!(Arc::ptr_eq(target.display.content.as_ref().expect("content"), &a));
}

#[test]
fn a_tag_is_carried_as_its_own_tree_and_compares_by_identity() {
    let run = run_workflow(
        &std(),
        text::WORKFLOW,
        &["\\begin{description}\\item[tag=Same]\\label{item:a} a\
           \\item[tag=Same]\\label{item:b} b\\end{description}"],
        &Overrides::none(),
    );
    let (a, b) = (synthesized(&run, "item:a"), synthesized(&run, "item:b"));
    assert_eq!(plain_text(&a), "Same");
    assert_eq!(plain_text(&b), "Same");
    // Two trees built apart are two contents, whatever they hold.
    assert_ne!(ContentRef::Synthesized(Arc::clone(&a)), ContentRef::Synthesized(b));
    assert_eq!(ContentRef::Synthesized(Arc::clone(&a)), ContentRef::Synthesized(a));
}

#[test]
fn the_projection_shows_a_synthesized_content_as_its_plain_text() {
    let run = run_workflow(
        &std(),
        text::WORKFLOW,
        &["\\begin{itemize}\\item\\label{item:a} a\\end{itemize}"],
        &Overrides::none(),
    );
    let projected = common::stdcore::projection(&run, REFPOINTS);
    let written = flm_core::dump::json::to_json(&projected).expect("JSON");
    assert!(written.contains("\"content\":\"\u{2022}\""), "{written}");
    // The label table holds what the labels make of the points, and no point.
    let labels = common::stdcore::projection(&run, LABELS);
    assert!(!format!("{labels:?}").contains("points"), "{labels:?}");
}

// ---------------------------------------------------------------------------------------------
// the aspect is asked once per node and run
// ---------------------------------------------------------------------------------------------

/// The element type name of the counting element, `x.counted`.
const COUNTED: &str = "x.counted";

/// A reference point that counts how often it is asked for: numbered in the
/// equation counter, labeled by its `\label` fields, showing its argument.
#[derive(Debug)]
struct Counted {
    calls: Arc<AtomicUsize>,
}

impl Referenceable for Counted {
    fn points(&self, node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let (labels, malformed) = refs::point_labels(refs::label_uses(node).iter());
        vec![RefPoint {
            kind: "counted".into(),
            labels,
            malformed,
            computed: 0,
            anchor: AnchorPolicy::FromFirstLabel,
            number: Some(NumberSource {
                counter: math::COUNTER.into(),
                instances: Vec::new(),
                style: math::COUNTER_STYLE.into(),
                advance: vec![0],
                number_format: None,
            }),
            content: Some(ContentRef::Argument(0)),
            accepts: AcceptedRefTypes::any(),
        }]
    }
}

/// A test-only feature registering `\counted{text}\label{…}`.
#[derive(Debug)]
struct CountedFeature {
    calls: Arc<AtomicUsize>,
}

impl FlmFeature for CountedFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("x-counted", "A reference point counting how often it is asked for")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        let value = Arc::new(Counted { calls: Arc::clone(&self.calls) });
        b.element(ElementDef::new(
            owner,
            ["counted"],
            SyntaxDef::Macro { args: vec![ArgDef::mandatory("text"), refs::label_fields_arg()] },
            Arc::new(
                ElementSpec::new(COUNTED)
                    .docs("a labeled, numbered element counting how often it is asked")
                    .with::<dyn BlockRoleAspect>(Arc::new(FixedBlockRole(BlockRole::DEFAULT)))
                    .with::<dyn Referenceable>(value as Arc<dyn Referenceable>),
            ),
        ))?;
        Ok(())
    }
}

#[test]
fn the_aspect_is_asked_once_per_node_in_a_run_that_numbers_anchors_and_resolves() {
    let calls = Arc::new(AtomicUsize::new(0));
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(Arc::new(CountedFeature { calls: Arc::clone(&calls) }));
    let registry = builder.build(&RegistryInputs::new()).expect("the registry builds");
    let document = "\\counted{A}\\label{eq:a} \\counted{B} \\counted{C}\\label{eq:c}\n\n\
                    See \\ref{eq:a} and \\ref{eq:c}.";
    // The `check` workflow runs the label scan, the anchors, the resolver and
    // the numbering stage, every consumer of the points.
    let run = run_workflow(&registry, CHECK_WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let counters = run.table(&COUNTERS).expect("declared").expect("numbered");
    let targets = run.table(&REF_TARGETS).expect("declared").expect("written");
    let numbers: Vec<i64> = ["a", "c"]
        .iter()
        .map(|label| {
            let point = targets.by_label[&Label::new("eq", *label)].point;
            counters.record(point).expect("numbered").value.number()
        })
        .collect();
    assert_eq!(numbers, [1, 3]);
    assert_eq!(calls.load(Ordering::SeqCst), 3, "one call per node");
}

// ---------------------------------------------------------------------------------------------
// registries holding one of the two consumers, or neither's dependency
// ---------------------------------------------------------------------------------------------

/// The features every registry below holds: the lowering machinery, the text
/// backend, the block structure and the check feature.
fn base() -> Vec<Arc<dyn FlmFeature>> {
    vec![lowering::feature(), flm_core::stdcore::backends::pieces::feature(), text::feature(), Arc::new(Blocks::default()), check_feature()]
}

#[test]
fn a_registry_with_numbering_and_without_refs_numbers_as_before() {
    let mut features = base();
    features.extend([
        index::feature(),
        refpoints::feature(),
        numbering::feature(),
        lists::feature(),
    ]);
    let registry = registry_over(features);
    let run = run_workflow(
        &registry,
        text::WORKFLOW,
        &["\\begin{enumerate}\\item a\\item b\\end{enumerate}"],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(text_of(&run), "1. a\n\n2. b\n");
}

#[test]
fn a_registry_with_refs_and_without_numbering_resolves_as_before() {
    let mut features = base();
    features.extend([
        index::feature(),
        refpoints::feature(),
        refs::feature(),
        headings::feature(),
    ]);
    let registry = registry_over(features);
    let run = run_workflow(
        &registry,
        text::WORKFLOW,
        &["\\section{Intro}\\label{sec:intro}\n\nSee \\ref{sec:intro}."],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert!(text_of(&run).ends_with("See Intro.\n"), "{}", text_of(&run));
}

/// The plan error of the `text` workflow over `features`.
fn plan_error(features: Vec<Arc<dyn FlmFeature>>) -> PlanError {
    let registry = registry_over(features);
    match try_plan(&registry, text::WORKFLOW, &Overrides::none()) {
        Ok(_) => panic!("the plan was made without the table of reference points"),
        Err(error) => error,
    }
}

/// Asserts that `error` is the stage `expected_stage` needing the table
/// `flm.refpoints`, which no feature of the registry declares.
fn assert_needs_refpoints(error: PlanError, expected_stage: &str) {
    match error {
        PlanError::NeedsUndeclared { stage, kind, name, .. } => {
            assert_eq!(stage, expected_stage);
            assert_eq!(kind, "table");
            assert_eq!(name, "flm.refpoints");
        }
        other => panic!("expected the stage to need an undeclared table, got: {other}"),
    }
}

#[test]
fn a_registry_holding_refs_without_refpoints_builds_and_fails_to_plan() {
    // The registry builds: what a stage reads is settled when a plan is made.
    // Every plan scheduling the label scan then fails, naming the stage and
    // the table of reference points.
    let mut features = base();
    features.extend([index::feature(), refs::feature(), headings::feature()]);
    assert_needs_refpoints(plan_error(features), "flm.refs.scan");
}

#[test]
fn a_registry_holding_numbering_without_refpoints_builds_and_fails_to_plan() {
    let mut features = base();
    features.extend([index::feature(), numbering::feature(), lists::feature()]);
    assert_needs_refpoints(plan_error(features), "flm.numbering.number");
}
