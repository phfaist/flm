//! The `xnotes` feature: the kinds and their configuration, the element with
//! its two arguments and three aspects, the two reference points of every
//! xnote, the label argument, the collect stage and the table `flm.xnotes`;
//! and the general lowering rules over the `text` workflow — the mark, the two
//! in-place shapes, the conditions the rules report; and each kind's list:
//! its builder, its rule, its override, a kind another feature contributes.
//!
//! Most tests of the tables run the `check` workflow with the table
//! `flm.xnotes` demanded beside it, and read the tables — `flm.xnotes`,
//! `flm.refpoints`, `flm.counters`, `flm.anchors`, `flm.labels` — and the
//! diagnostics. The tests of what is printed where an xnote stands run the
//! `text` workflow with every kind's list left out of the render; the tests
//! of the lists render them.
//!
//! Every registry is the standard package, its xnotes feature under the
//! configuration a test names, unless a test builds one of its own.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::model::options::OptionValues;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry, RegistryBuilder,
    RegistryInputs,
};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::copy::{ValueContent, ValuePiece, copy_pieces_into_value};
use flm_core::lang::guard;
use flm_core::lang::{Flm, GuardPolicy};
use flm_core::lang::mint::{ArgLayout, ElementDef, FlmSynthesizedSpec, SyntaxDef};
use flm_core::lang::rewrites::blocks::argument_holds_blocks;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::lang::structural;
use flm_core::core::lowering::NodeRegion;
use flm_core::model::addr::TreeId;
use flm_core::model::doc::FragmentLayout;
use flm_core::model::aspects::{
    BlockRole, InlineSpacing, Label, RefPoint, Referenceable, block_role_of,
};
use flm_core::model::element::ElementSpec;
use flm_core::stdcore::backends::pieces::PiecesRenderOptions;
use flm_core::stdcore::backends::{lowering, pieces, text};
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};
use flm_core::stdcore::features::numbering::{
    COUNTERS, NumberTemplate, PrefixWords, Within,
};
use flm_core::stdcore::features::refpoints::REFPOINTS;
use flm_core::stdcore::features::refs::{ANCHORS, LABELS};
use flm_core::stdcore::features::xnotes::{
    self, XNOTE, XNOTES, XnoteKind, XnoteOf, XnotePlacement, XnoteSource,
    XnotesConfig, XnotesFeature, labels_of,
};
use flm_core::stdcore::features::{index, numbering, refpoints, refs};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig, check_feature, package};
use techy::core::Lang;
use techy::core::node::{
    BodySlotExt, BuildId, ChildRegion, ContentNodes, NodeKind, NodeRef, NodeTree, ParsedSlot, ParsedSlots,
    SlotRole,
};
use techy::latexlike::BodyMarker;
use techy::transform::{Restage, RestageContext, RestageVisitor};
use techy::error::Severity;
use techy::serialize::SerialValue;

use common::lowering::text as printed;
use common::stdcore::{
    identifiers, identifiers_but_unknown_size, package_features_but, projection, registry_over,
    reports, run_workflow,
};

// ---------------------------------------------------------------------------------------------
// registries and runs
// ---------------------------------------------------------------------------------------------

/// The standard package under `config`, its xnotes feature under `xnotes`.
fn registry_with(config: StdcoreConfig, xnotes: XnotesConfig) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(config.xnotes(xnotes)));
    builder.use_package(flm_core::math_passthrough::package());
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

/// The standard package with the xnotes feature under `xnotes`.
fn registry(xnotes: XnotesConfig) -> Arc<FlmRegistry> {
    registry_with(StdcoreConfig::new(), xnotes)
}

/// The standard package with the xnotes feature under its default
/// configuration.
fn std() -> Arc<FlmRegistry> {
    registry(XnotesConfig::new())
}

/// The overrides of a `check` run demanding the table `flm.xnotes` beside the
/// workflow's own demands: nothing reads the table yet, so the collect stage
/// is scheduled only where it is demanded.
fn with_xnotes() -> Overrides {
    Overrides { demands: vec!["flm.xnotes".into()], ..Overrides::none() }
}

/// Runs the `check` workflow of `registry` over one tree per text, with the
/// table `flm.xnotes` demanded.
fn check_with(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Run {
    run_workflow(registry, CHECK_WORKFLOW, texts, &with_xnotes())
}

/// Runs the `check` workflow of the default registry over one tree per text.
fn check(texts: &[&str]) -> Run {
    check_with(&std(), texts)
}

/// Renders one tree under the `text` workflow of `registry`, every kind's
/// list of xnotes left out: the text where the xnotes stand, which is what
/// the tests of the marks read. [`render_listed`] renders the lists too.
fn render_with(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    render_texts(registry, &[document])
}

/// [`render_with`] over one tree per text.
fn render_texts(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Run {
    run_workflow(registry, text::WORKFLOW, texts, &without_lists(registry))
}

/// Renders one tree per text under the `text` workflow of `registry`, the
/// lists of xnotes placed after the content.
fn render_listed(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Run {
    run_workflow(registry, text::WORKFLOW, texts, &Overrides::none())
}

/// The overrides leaving the list of every kind of `registry` out of a
/// render.
fn without_lists(registry: &FlmRegistry) -> Overrides {
    let options = registry
        .services()
        .services_named::<XnoteKind>(xnotes::KINDS_SLOT)
        .fold(PiecesRenderOptions::default(), |options, kind| {
            options.exclude(xnotes::piece_name(kind.name.as_str()))
        });
    Overrides { render_options: OptionValues::new().with(options), ..Overrides::none() }
}

/// The identifiers of the Errors of a run, in order.
fn errors(run: &Run) -> Vec<String> {
    reports(run)
        .into_iter()
        .filter(|(_, severity)| *severity == Severity::Error)
        .map(|(id, _)| id)
        .collect()
}

/// The projection of `flm.xnotes`: for each kind, its records as
/// `(point, mark)` keys.
fn records(run: &Run) -> Vec<(String, Vec<(String, String)>)> {
    let SerialValue::Map(kinds) = projection(run, XNOTES) else { panic!("a map") };
    let SerialValue::Map(kinds) = kinds.into_iter().next().expect("the table").1 else {
        panic!("a map of kinds")
    };
    kinds
        .into_iter()
        .map(|(kind, records)| {
            let SerialValue::List(records) = records else { panic!("a list") };
            let records = records
                .into_iter()
                .map(|record| {
                    let SerialValue::Map(fields) = record else { panic!("a record") };
                    let text = |key: &str| match fields.iter().find(|(k, _)| k == key) {
                        Some((_, SerialValue::Str(text))) => text.clone(),
                        other => panic!("{key}: {other:?}"),
                    };
                    (text("point"), text("mark"))
                })
                .collect();
            (kind, records)
        })
        .collect()
}

/// How many xnotes of `kind` the run recorded.
fn count(run: &Run, kind: &str) -> usize {
    run.table(XNOTES).expect("declared").expect("written").of_kind(kind).len()
}

/// The anchor of every xnote of `kind`, then of its mark, in document order.
fn anchors(run: &Run, kind: &str) -> Vec<(Option<String>, Option<String>)> {
    let table = run.table(XNOTES).expect("declared").expect("written");
    let anchors = run.table(ANCHORS).expect("declared").expect("written");
    table
        .of_kind(kind)
        .iter()
        .map(|record| {
            (
                anchors.id(record.point).map(String::from),
                anchors.id(record.mark).map(String::from),
            )
        })
        .collect()
}

/// The number the counter table gave every xnote of `kind`, in document
/// order, as its components (ancestors first).
fn numbers(run: &Run, kind: &str) -> Vec<Option<Vec<i64>>> {
    let table = run.table(XNOTES).expect("declared").expect("written");
    let counters = run.table(COUNTERS).expect("declared").expect("written");
    table
        .of_kind(kind)
        .iter()
        .map(|record| {
            counters.record(record.point).map(|record| {
                let mut parts: Vec<i64> =
                    record.ancestors.iter().map(|ancestor| ancestor.value.number()).collect();
                parts.push(record.value.number());
                parts
            })
        })
        .collect()
}

/// The first tree of a run, frozen.
fn tree(run: &Run) -> &Arc<NodeTree<Flm>> {
    run.frozen_tree(TreeId::tree(0)).expect("the first tree")
}

/// Every node of `tree` carrying the element `name`, in document order.
fn nodes_of<'t>(tree: &'t NodeTree<Flm>, name: &str) -> Vec<NodeRef<'t, Flm>> {
    tree.root().descendants().filter(|node| node.ext().element.name.as_str() == name).collect()
}

/// The labels of the table `flm.labels`, as written.
fn labels(run: &Run) -> Vec<String> {
    let labels = run.table(LABELS).expect("declared").expect("written");
    labels.by_label.keys().map(ToString::to_string).collect()
}

/// A kind named `name`, numbered, written `\<name>`, with the label type
/// `type`.
fn numbered(name: &str, label_type: &str) -> XnoteKind {
    let capital = format!("{}{}", name[..1].to_uppercase(), &name[1..]);
    XnoteKind::numbered(name, PrefixWords::new(format!("{capital}~"), format!("{capital}s~")))
        .commands([name])
        .label_type(label_type)
}

/// The unnumbered in-place kind `sidenote`, written `\sidenote`, with the
/// label type `sn`.
fn sidenote() -> XnoteKind {
    XnoteKind::unnumbered("sidenote", PrefixWords::new("Side note~", "Side notes~"))
        .commands(["sidenote"])
        .label_type("sn")
        .placement(XnotePlacement::InPlace)
}

// ---------------------------------------------------------------------------------------------
// numbers and records (tests 27, 31, 33)
// ---------------------------------------------------------------------------------------------

#[test]
fn two_xnotes_number_one_and_two_in_document_order_across_two_trees() {
    let run = check(&["First.\\footnote{One.}", "Second.\\footnote{Two.}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(numbers(&run, "footnote"), [Some(vec![1]), Some(vec![2])]);
    let counters = run.table(COUNTERS).expect("declared").expect("written");
    let table = run.table(XNOTES).expect("declared").expect("written");
    for record in table.of_kind("footnote") {
        let counted = counters.record(record.point).expect("numbered");
        assert_eq!(counted.counter.as_str(), "flm.xnotes.footnote");
        assert_eq!(counted.style.name().as_str(), "flm.xnotes.style.footnote");
        // The mark point counts in nothing.
        assert!(counters.record(record.mark).is_none());
    }
    // The two records sit in two trees.
    let trees: Vec<TreeId> = table.of_kind("footnote").iter().map(|r| r.point.addr.tree).collect();
    assert_eq!(trees, [TreeId::tree(0), TreeId::tree(1)]);
}

#[test]
fn the_table_records_each_kind_s_points_in_document_order() {
    let registry = registry(XnotesConfig::new().kind(numbered("endnote", "en")).kind(sidenote()));
    let run = check_with(
        &registry,
        &["A\\footnote{1} B\\endnote{1} C\\sidenote{s} D\\footnote{2}", "E\\endnote{2}"],
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let records = records(&run);
    let kinds: Vec<&str> = records.iter().map(|(kind, _)| kind.as_str()).collect();
    assert_eq!(kinds, ["endnote", "footnote", "sidenote"]);
    let lengths: Vec<usize> = records.iter().map(|(_, records)| records.len()).collect();
    assert_eq!(lengths, [2, 2, 1]);
    // Each record is the xnote's point, index 0, and its mark, index 1, of
    // one node.
    for (_, records) in &records {
        for (point, mark) in records {
            assert!(point.ends_with("#0"), "{point}");
            assert_eq!(mark.strip_suffix("#1"), point.strip_suffix("#0"), "{mark}");
        }
    }
    // Document order within a kind.
    let table = run.table(XNOTES).expect("declared").expect("written");
    let footnotes = table.of_kind("footnote");
    assert!(footnotes[0].span.start() < footnotes[1].span.start());
    assert_eq!(numbers(&run, "endnote"), [Some(vec![1]), Some(vec![2])]);
    assert_eq!(numbers(&run, "footnote"), [Some(vec![1]), Some(vec![2])]);
    assert_eq!(numbers(&run, "sidenote"), [None]);
    // A lookup by the node's address finds the record and its kind.
    let (kind, found) = table.record_at(footnotes[1].point.addr).expect("recorded");
    assert_eq!((kind.as_str(), found), ("footnote", &footnotes[1]));
}

#[test]
fn every_kind_has_an_entry_even_with_no_xnote() {
    let registry = registry(XnotesConfig::new().kind(numbered("endnote", "en")));
    let run = check_with(&registry, &["No notes here."]);
    assert_eq!(
        records(&run),
        [("endnote".to_string(), Vec::new()), ("footnote".to_string(), Vec::new())]
    );
    assert!(run.table(XNOTES).expect("declared").expect("written").is_empty());
}

#[test]
fn a_second_kind_gets_its_own_counter_and_its_own_styles() {
    let registry = registry(XnotesConfig::new().kind(numbered("endnote", "en")));
    let run = check_with(&registry, &["A\\footnote{f} B\\endnote{e} C\\endnote{e}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let counters = run.table(COUNTERS).expect("declared").expect("written");
    let table = run.table(XNOTES).expect("declared").expect("written");
    let endnote = counters.record(table.of_kind("endnote")[1].point).expect("numbered");
    assert_eq!(endnote.counter.as_str(), "flm.xnotes.endnote");
    assert_eq!(endnote.value.number(), 2);
    for style in [
        "flm.xnotes.style.endnote",
        "flm.xnotes.style.endnote.mark",
        "flm.xnotes.style.footnote",
        "flm.xnotes.style.footnote.mark",
    ] {
        assert!(counters.style(&style.into()).is_some(), "{style}");
    }
    let mark = counters.style(&xnotes::mark_style_name("endnote")).expect("declared");
    assert_eq!(mark.name().as_str(), "flm.xnotes.style.endnote.mark");
    assert_eq!(anchors(&run, "endnote")[1].0.as_deref(), Some("en-2"));
}

#[test]
fn an_unnumbered_kind_declares_no_counter_and_no_style() {
    let registry = registry(XnotesConfig::new().kind(sidenote()));
    let run = check_with(&registry, &["A\\sidenote{s}"]);
    let counters = run.table(COUNTERS).expect("declared").expect("written");
    assert!(counters.style(&xnotes::style_name("sidenote")).is_none());
    assert!(counters.style(&xnotes::mark_style_name("sidenote")).is_none());
    assert_eq!(numbers(&run, "sidenote"), [None]);
}

// ---------------------------------------------------------------------------------------------
// references (tests 28, 29)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_reference_to_an_xnote_shows_the_kind_s_words_and_its_number() {
    let run = render_with(&std(), "A\\footnote[\\label{fn:a}]{x} B\\footnote[\\label{fn:b}]{y}\n\n\
         See \\ref{fn:a}, \\ref[noprefix]{fn:a}, \\ref{fn:a,fn:b}.",
    );
    assert_eq!(errors(&run), Vec::<String>::new());
    let out = printed(&run);
    assert!(out.starts_with("A\u{b9} B\u{b2}"), "{out}");
    // The reference style joins several numbers compactly, as every standard
    // feature's styles do.
    assert!(
        out.ends_with("See Footnote\u{a0}1, 1, Footnotes\u{a0}1,2.\n"),
        "{out}"
    );
}

#[test]
fn a_reference_to_an_unnumbered_xnote_shows_its_content_or_its_label() {
    let registry = registry(XnotesConfig::new().kind(sidenote()));
    let run = render_with(&registry, "A\\sidenote[\\label{sn:a}]{an aside} B\\sidenote[\\label{sn:b}]{one\n\ntwo}\n\n\
         See \\ref{sn:a}; \\ref{sn:b}.",
    );
    let out = printed(&run);
    // Inline content is shown; content holding paragraphs is not, and the
    // reference shows the label.
    assert!(out.ends_with("See an aside; sn:b.\n"), "{out}");
}

/// The standard package's features but `numbering`, with the xnotes feature
/// under `xnotes`.
fn without_numbering(xnotes: XnotesConfig) -> Arc<FlmRegistry> {
    registry_over(vec![
        Arc::new(Blocks::default()) as Arc<dyn FlmFeature>,
        index::feature(),
        refpoints::feature(),
        refs::feature(),
        Arc::new(XnotesFeature::new(xnotes)),
        lowering::feature(),
        pieces::feature(),
        text::feature(),
        check_feature(),
    ])
}

#[test]
fn without_numbering_a_numbered_kind_is_reported_once_and_a_reference_shows_the_label() {
    let registry = without_numbering(XnotesConfig::new().kind(sidenote()));
    // The counter and the styles of `footnote` are dropped with a Note per
    // slot; the unnumbered kind declared none.
    let notes: Vec<String> = registry
        .build_diagnostics()
        .iter()
        .filter(|d| d.severity() == Severity::Note)
        .map(|d| d.to_string())
        .collect();
    for slot in ["flm.numbering.styles", "flm.numbering.counters"] {
        assert!(notes.iter().any(|note| note.contains("`xnotes`") && note.contains(slot)), "{notes:#?}");
    }
    let run = check_with(&registry, &["A\\footnote[\\label{fn:a}]{x} B\\footnote{y} C\\sidenote{s}"]);
    assert_eq!(errors(&run), ["flm.xnotes.no-numbering"]);
    assert_eq!(count(&run, "footnote"), 2);
    let diagnostic = run.diagnostics().iter().next().expect("one").to_string();
    assert!(diagnostic.contains("footnote"), "{diagnostic}");
    // A numbered xnote with no number is anchored on its label, or not at all.
    assert_eq!(anchors(&run, "footnote")[0].0.as_deref(), Some("fn-a"));
    assert_eq!(anchors(&run, "footnote")[1].0, None);
    let run = render_with(&registry, "A\\footnote[\\label{fn:a}]{x}\n\nSee \\ref{fn:a}.");
    // The xnote's rule reads `flm.xnotes`, so a render schedules the collect
    // stage, which reports the kind once; the mark is empty, and the
    // reference has nothing to show.
    assert_eq!(errors(&run), ["flm.xnotes.no-numbering", "flm.refs.nothing-to-show"]);
    assert_eq!(printed(&run), "A\n\nSee fn:a.\n");
}

#[test]
fn without_numbering_an_unnumbered_kind_reports_nothing() {
    let registry = without_numbering(XnotesConfig::empty().kind(sidenote()));
    let run = check_with(&registry, &["A\\sidenote{s}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(count(&run, "sidenote"), 1);
}

#[test]
fn a_document_without_xnotes_reports_nothing_without_numbering() {
    let run = check_with(&without_numbering(XnotesConfig::new()), &["No notes."]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

// ---------------------------------------------------------------------------------------------
// anchors (tests 30, 32, 34)
// ---------------------------------------------------------------------------------------------

/// Whether `id` is `<prefix>--<position>` and four lowercase letters.
fn is_positional(id: &str, prefix: &str, position: usize) -> bool {
    id.strip_prefix(&format!("{prefix}--{position}"))
        .is_some_and(|letters| letters.len() == 4 && letters.chars().all(|c| c.is_ascii_lowercase()))
}

#[test]
fn the_anchors_of_a_numbered_and_of_an_unnumbered_xnote() {
    let registry = registry(XnotesConfig::new().kind(sidenote()));
    let run = check_with(&registry, &["A\\footnote{f} B\\sidenote{s} C\\sidenote[\\label{sn:x}]{t}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let footnote = &anchors(&run, "footnote")[0];
    assert_eq!(footnote.0.as_deref(), Some("fn-1"));
    assert!(is_positional(footnote.1.as_deref().expect("a mark anchor"), "fnref", 1), "{footnote:?}");
    let sidenotes = anchors(&run, "sidenote");
    assert!(is_positional(sidenotes[0].0.as_deref().expect("anchored"), "sn", 1), "{sidenotes:?}");
    assert!(is_positional(sidenotes[0].1.as_deref().expect("anchored"), "snref", 1), "{sidenotes:?}");
    // A labeled unnumbered xnote is anchored on its label; its mark keeps its
    // position.
    assert_eq!(sidenotes[1].0.as_deref(), Some("sn-x"));
    assert!(is_positional(sidenotes[1].1.as_deref().expect("anchored"), "snref", 2), "{sidenotes:?}");
}

/// The standard package with sections numbered and the footnotes numbered
/// within them.
fn within_sections() -> Arc<FlmRegistry> {
    registry_with(
        StdcoreConfig::new().headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered)),
        XnotesConfig { kinds: vec![XnoteKind::footnote().within(Within::parent("flm.headings.section"))] },
    )
}

#[test]
fn a_kind_numbered_within_sections_restarts_and_is_anchored_on_both_numbers() {
    let run = check_with(
        &within_sections(),
        &["\\section{One}\nA\\footnote{a} B\\footnote{b}\n\n\\section{Two}\nC\\footnote{c}"],
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(numbers(&run, "footnote"), [Some(vec![1, 1]), Some(vec![1, 2]), Some(vec![2, 1])]);
    let ids: Vec<Option<String>> = anchors(&run, "footnote").into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [Some("fn-1-1".into()), Some("fn-1-2".into()), Some("fn-2-1".into())]);
}

#[test]
fn a_kind_numbered_within_a_counter_with_no_value_falls_back_to_the_label() {
    let run = check_with(
        &within_sections(),
        &["A\\footnote[\\label{fn:e}]{a} B\\footnote{b}\n\n\\section{One}\nC\\footnote{c}"],
    );
    assert_eq!(errors(&run), ["flm.numbering.parent-uninitialized"]);
    assert_eq!(numbers(&run, "footnote"), [None, None, Some(vec![1, 1])]);
    let ids: Vec<Option<String>> = anchors(&run, "footnote").into_iter().map(|(id, _)| id).collect();
    assert_eq!(ids, [Some("fn-e".into()), None, Some("fn-1-1".into())]);
}

// ---------------------------------------------------------------------------------------------
// where an xnote stands (test 41; design §9)
// ---------------------------------------------------------------------------------------------

#[test]
fn an_xnote_in_a_formula_is_none_and_in_text_inside_a_formula_it_is_one() {
    let run = check(&["\\(x\\footnote{no}\\) and \\(y\\text{\\footnote{yes}}\\)"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(count(&run, "footnote"), 1);
    let tree = tree(&run);
    let found = nodes_of(tree, XNOTE);
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].span_content(), "\\footnote{yes}");
}

#[test]
fn an_xnote_in_a_heading_title_and_in_a_list_item_is_an_xnote() {
    let run = check(&[
        "\\section{Title\\footnote{t}}\n\n\\begin{itemize}\\item a\\footnote{i}\\end{itemize}",
    ]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(numbers(&run, "footnote"), [Some(vec![1]), Some(vec![2])]);
}

#[test]
fn an_xnote_inside_an_xnote_answers_its_two_points_and_is_counted_in_document_order() {
    let run = check(&["A\\footnote{a\\footnote[\\label{fn:in}]{b}} B\\footnote{c}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(numbers(&run, "footnote"), [Some(vec![1]), Some(vec![2]), Some(vec![3])]);
    // The inner xnote's label labels it.
    assert_eq!(labels(&run), ["fn:in"]);
    let points = run.table(REFPOINTS).expect("declared").expect("written");
    assert_eq!(points.len(), 6);
    let tree = tree(&run);
    let found = nodes_of(tree, XNOTE);
    assert_eq!(found.len(), 3);
    let value = found[1].ext().element.aspect::<dyn Referenceable>().expect("the aspect");
    assert_eq!(value.points(&found[1]).len(), 2);
    assert_eq!(count(&run, "footnote"), 3);
}

/// The visitor of a test's list entry: called once, at the root of the tree
/// it restages, it emits a list node holding one node of `entry` whose body
/// is a verbatim copy of `content` — what a kind's list builder makes of an
/// xnote.
struct EntryOver<'a, 't> {
    entry: Arc<FlmSynthesizedSpec>,
    content: &'a [NodeRef<'t, Flm>],
}

/// A visitor that carries every node over unchanged.
struct Verbatim;

impl RestageVisitor<Flm, (), ()> for Verbatim {
    type Error = String;

    fn restage(
        &mut self,
        _node: NodeRef<'_, Flm>,
        _cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, String> {
        Ok(Restage::Descend(()))
    }
}

impl RestageVisitor<Flm, (), ()> for EntryOver<'_, '_> {
    type Error = String;

    fn restage(
        &mut self,
        root: NodeRef<'_, Flm>,
        cx: &mut RestageContext<'_, Flm, (), ()>,
    ) -> Result<Restage<()>, String> {
        let mut children: Vec<BuildId> = Vec::new();
        for node in self.content {
            children.extend(cx.restage_subtree(*node, &mut Verbatim).map_err(|e| e.to_string())?);
        }
        let span = root.span().clone();
        let state = Arc::clone(root.parsing_state());
        let count = children.len() as u32;
        let mut data = self.entry.callable_data();
        data.slots = ParsedSlots::new(vec![ParsedSlot::new(
            ChildRegion::new(0..count, ContentNodes::InRegion(0..count)),
            "body",
            SlotRole::Content,
            BodyMarker::make_body(),
        )]);
        let kind = NodeKind::callable(data);
        let ext = Flm::make_node_ext(&kind, &span, &state, cx.builder().staged_children(&children))
            .map_err(|e| e.to_string())?;
        let entry = cx
            .builder()
            .add(kind, span.clone(), Arc::clone(&state), children, ext, ())
            .map_err(|e| e.to_string())?;
        let kind = NodeKind::list();
        let ext = Flm::make_node_ext(&kind, &span, &state, cx.builder().staged_children(&[entry]))
            .map_err(|e| e.to_string())?;
        let list = cx.builder().add(kind, span, state, vec![entry], ext, ()).map_err(|e| e.to_string())?;
        Ok(Restage::Emit(vec![list]))
    }
}

#[test]
fn an_xnote_in_an_author_definition_is_an_ordinary_xnote() {
    use flm_core::lang::rewrites::userdefs::Definition;
    use flm_core::stdcore::features::userdefs::UserdefsConfig;
    let definitions = UserdefsConfig::new().definitions(vec![
        Definition::macro_("noted", Vec::new()).text("text\\footnote{from a definition}"),
    ]);
    let registry = registry_with(StdcoreConfig::new().userdefs(definitions), XnotesConfig::new());
    let run = check_with(&registry, &["A\\noted{} B\\footnote{b}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(numbers(&run, "footnote"), [Some(vec![1]), Some(vec![2])]);
}

// ---------------------------------------------------------------------------------------------
// the label argument (test 42)
// ---------------------------------------------------------------------------------------------

/// The labels and the rejected texts of the first xnote of `document`, and
/// the run's labels and diagnostics.
fn label_case(document: &str) -> (Vec<String>, Vec<String>, Run) {
    let run = check(&[document]);
    let tree = tree(&run);
    let node = nodes_of(tree, XNOTE).into_iter().next().expect("an xnote");
    let (uses, rejected) = labels_of(&node);
    let uses = uses.into_iter().map(|one| one.text).collect();
    let rejected = rejected.into_iter().map(|one| one.found).collect();
    (uses, rejected, run)
}

#[test]
fn a_label_in_the_optional_argument_labels_the_xnote() {
    let (uses, rejected, run) = label_case("A\\footnote[\\label{fn:e}]{x}");
    assert_eq!((uses, rejected), (vec!["fn:e".to_string()], Vec::<String>::new()));
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(labels(&run), ["fn:e"]);
    let table = run.table(LABELS).expect("declared").expect("written");
    let point = table.by_label[&Label::new("fn", "e")];
    assert_eq!(point, run.table(XNOTES).expect("declared").expect("written").of_kind("footnote")[0].point);
    assert_eq!(anchors(&run, "footnote")[0].0.as_deref(), Some("fn-1"));
}

#[test]
fn two_labels_both_label_the_xnote() {
    let (uses, rejected, run) = label_case("A\\footnote[\\label{fn:a}\\label{fn:b}]{x}");
    assert_eq!(uses, ["fn:a", "fn:b"]);
    assert!(rejected.is_empty());
    assert_eq!(labels(&run), ["fn:a", "fn:b"]);
}

#[test]
fn anything_else_is_rejected_and_the_labels_beside_it_still_stand() {
    let (uses, rejected, run) = label_case("A\\footnote[\\textbf{x}\\label{fn:e} text {g}]{x}");
    assert_eq!(uses, ["fn:e"]);
    assert_eq!(rejected, ["\\textbf{x}", "text", "{g}"]);
    assert_eq!(labels(&run), ["fn:e"]);
    // The rejection is the lowering rule's to report; the check reports
    // nothing about it.
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

#[test]
fn whitespace_and_comments_are_skipped() {
    let (uses, rejected, run) = label_case("A\\footnote[ %% a comment\n  \\label{fn:e}  ]{x}");
    assert_eq!((uses, rejected), (vec!["fn:e".to_string()], Vec::<String>::new()));
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

#[test]
fn a_label_after_the_xnote_is_an_unknown_command_and_labels_nothing() {
    let (uses, rejected, run) = label_case("A\\footnote{x}\\label{fn:e} B");
    assert_eq!((uses, rejected), (Vec::<String>::new(), Vec::<String>::new()));
    assert_eq!(errors(&run), ["core.specs.unresolvable-command"]);
    assert!(labels(&run).is_empty());
}

#[test]
fn a_label_in_the_content_is_an_unknown_command() {
    let run = check(&["A\\footnote{x\\label{fn:e}}"]);
    assert_eq!(errors(&run), ["core.specs.unresolvable-command"]);
    assert!(labels(&run).is_empty());
}

#[test]
fn a_label_of_another_type_is_refused_and_the_anchor_is_unchanged() {
    let (uses, _, run) = label_case("A\\footnote[\\label{sec:e}]{x}");
    assert_eq!(uses, ["sec:e"]);
    assert_eq!(errors(&run), ["flm.refs.label-type-not-accepted"]);
    assert!(labels(&run).is_empty());
    assert_eq!(anchors(&run, "footnote")[0].0.as_deref(), Some("fn-1"));
    // An unnumbered xnote is anchored on its first label, or on its position
    // where it wrote none; one whose every label was refused gets no anchor,
    // as every point anchored so does.
    let registry = registry(XnotesConfig::new().kind(sidenote()));
    let run = check_with(&registry, &["A\\sidenote[\\label{sec:e}]{x}"]);
    assert_eq!(errors(&run), ["flm.refs.label-type-not-accepted"]);
    assert_eq!(anchors(&run, "sidenote")[0].0, None);
}

#[test]
fn a_malformed_label_is_reported_by_the_label_scan() {
    let (uses, _, run) = label_case("A\\footnote[\\label{nocolon}]{x}");
    assert_eq!(uses, ["nocolon"]);
    assert_eq!(errors(&run), ["flm.refs.bad-label"]);
}

#[test]
fn without_the_references_feature_a_label_is_an_unknown_command_there() {
    let registry = registry_over(vec![
        Arc::new(Blocks::default()) as Arc<dyn FlmFeature>,
        index::feature(),
        refpoints::feature(),
        numbering::feature(),
        Arc::new(XnotesFeature::new(XnotesConfig::new())),
        check_feature(),
    ]);
    let notes: Vec<String> = registry
        .build_diagnostics()
        .iter()
        .filter(|d| d.identifier() == "flm.registry.unresolved-peer")
        .map(|d| d.to_string())
        .collect();
    assert!(notes.iter().any(|note| note.contains("flm.refs.label")), "{notes:#?}");
    let run = check_with(&registry, &["A\\footnote[\\label{fn:e}]{x}"]);
    assert_eq!(errors(&run), ["core.specs.unresolvable-command"]);
    let tree = tree(&run);
    let node = nodes_of(tree, XNOTE).into_iter().next().expect("an xnote");
    let (uses, rejected) = labels_of(&node);
    assert!(uses.is_empty());
    // `\label` is a command of no argument there, so its braces are a group
    // of their own, and both are refused.
    let found: Vec<&str> = rejected.iter().map(|one| one.found.as_str()).collect();
    assert_eq!(found, ["\\label", "{fn:e}"]);
    assert_eq!(numbers(&run, "footnote"), [Some(vec![1])]);
}

// ---------------------------------------------------------------------------------------------
// the block role and the layout (test 47, design §4.2)
// ---------------------------------------------------------------------------------------------

/// The block role of every xnote of `document` under `kind`, and the run.
fn roles(kind: XnoteKind, document: &str) -> (Vec<BlockRole>, Run) {
    let run = check_with(&registry(XnotesConfig { kinds: vec![kind] }), &[document]);
    let roles = nodes_of(tree(&run), XNOTE).iter().map(block_role_of).collect();
    (roles, run)
}

#[test]
fn a_listed_xnote_is_inline_and_absorbs_the_space_before_it() {
    let (roles, _) = roles(XnoteKind::footnote(), "A \\footnote{x} B\\footnote{y\n\nz}");
    let listed = BlockRole::Inline(InlineSpacing { absorb_before: true, absorb_after: false });
    assert_eq!(roles, [listed, listed]);
    let run = render_with(&std(), "Word \\footnote{x} next.");
    assert_eq!(printed(&run), "Word\u{b9} next.\n");
}

#[test]
fn an_in_place_xnote_is_inline_or_a_block_by_its_content() {
    let kind = XnoteKind::footnote().placement(XnotePlacement::InPlace);
    let (roles, run) = roles(kind, "A \\footnote{x} B\\footnote{y\n\nz} C.");
    assert_eq!(roles, [BlockRole::Inline(InlineSpacing::CONTENT), BlockRole::Block]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    // The block stands between the two halves of the paragraph.
    let tree = tree(&run);
    let block = nodes_of(tree, XNOTE)[1];
    assert_ne!(
        block.parent().map(|p| p.ext().element.name.as_str().to_string()),
        Some(structural::PARAGRAPH.to_string())
    );
    assert_eq!(nodes_of(tree, structural::PARAGRAPH).len(), 4);
}

#[test]
fn an_empty_in_place_xnote_is_inline_unless_its_kind_lays_out_blocks() {
    let kind = XnoteKind::footnote().placement(XnotePlacement::InPlace);
    let (empty, _) = roles(kind.clone(), "A\\footnote{} B\\footnote{ } C\\footnote{\n\n}");
    assert_eq!(empty, [BlockRole::Inline(InlineSpacing::CONTENT); 3]);
    let (blocks, _) = roles(kind.layout(ArgLayout::Blocks), "A\\footnote{} B");
    assert_eq!(blocks, [BlockRole::Block]);
}

#[test]
fn a_kind_laid_out_inline_refuses_a_blank_line() {
    let kind = XnoteKind::footnote().layout(ArgLayout::Inline);
    let (_, run) = roles(kind, "A\\footnote{one\n\ntwo} B");
    assert_eq!(errors(&run), ["flm.blocks.parbreak-in-inline"]);
    let tree = tree(&run);
    let node = nodes_of(tree, XNOTE)[0];
    assert!(!argument_holds_blocks(&node, 1));
}

#[test]
fn a_kind_laid_out_as_blocks_makes_a_one_line_xnote_a_paragraph() {
    let kind = XnoteKind::footnote().layout(ArgLayout::Blocks);
    let (_, run) = roles(kind, "A\\footnote{one line} B");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let tree = tree(&run);
    let node = nodes_of(tree, XNOTE)[0];
    assert!(argument_holds_blocks(&node, 1));
    let content: Vec<String> = node
        .argument_content_nodes_named(xnotes::XNOTE_ARG)
        .expect("readable")
        .expect("written")
        .iter()
        .map(|child| child.ext().element.name.as_str().to_string())
        .collect();
    assert_eq!(content, [structural::PARAGRAPH]);
}

#[test]
fn an_inferred_xnote_is_inline_on_one_run_and_paragraphs_otherwise() {
    let run = check(&["A\\footnote{one} B\\footnote{one\n\ntwo} C\\footnote{\\begin{itemize}\\item i\\end{itemize}}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let tree = tree(&run);
    let answers: Vec<bool> = nodes_of(tree, XNOTE).iter().map(|node| argument_holds_blocks(node, 1)).collect();
    assert_eq!(answers, [false, true, true]);
}

// ---------------------------------------------------------------------------------------------
// the configuration (test 47)
// ---------------------------------------------------------------------------------------------

/// The rendered error of a registry build that must fail.
fn build_error(xnotes: XnotesConfig) -> String {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().xnotes(xnotes)));
    builder.use_package(flm_core::math_passthrough::package());
    match builder.build(&RegistryInputs::new()) {
        Ok(_) => panic!("the build must fail"),
        Err(error) => error.render(),
    }
}

#[test]
fn the_configuration_s_refusals_fail_the_registry_build() {
    let words = || PrefixWords::new("Note~", "Notes~");
    for (config, expected) in [
        (XnotesConfig::empty().kind(XnoteKind::numbered("side_note", words())), "`side_note`"),
        (XnotesConfig::new().kind(numbered("other", "ot").commands(["footnote"])), "\\footnote"),
        (XnotesConfig::new().kind(numbered("other", "fn")), "label type `fn`"),
        (XnotesConfig::new().kind(numbered("other", "o:t")), "label type `o:t`"),
        (
            XnotesConfig::empty()
                .kind(XnoteKind::unnumbered("aside", words()).mark(NumberTemplate::formats("#{arabic}"))),
            "must not write a number",
        ),
    ] {
        let error = build_error(config);
        assert!(error.contains(expected), "{expected}: {error}");
    }
}

#[test]
fn a_bad_template_of_a_numbered_kind_fails_the_build_through_the_numbering_feature() {
    let error = build_error(XnotesConfig {
        kinds: vec![XnoteKind::footnote().mark(NumberTemplate::formats("#{nosuchformat}"))],
    });
    assert!(error.contains("flm.xnotes.style.footnote.mark"), "{error}");
}

#[test]
fn a_kind_with_no_command_is_accepted_and_written_by_nobody() {
    let registry = registry(XnotesConfig::new().kind(XnoteKind::numbered("addendum", PrefixWords::new("Add.~", "Adds.~"))));
    let run = check_with(&registry, &["A\\addendum{x}"]);
    assert_eq!(errors(&run), ["core.specs.unresolvable-command"]);
    assert_eq!(records(&run)[0], ("addendum".to_string(), Vec::new()));
}

#[test]
fn a_kind_with_several_commands_is_written_by_each() {
    let registry = registry(XnotesConfig { kinds: vec![XnoteKind::footnote().commands(["footnote", "fn"])] });
    let run = check_with(&registry, &["A\\footnote{a} B\\fn{b}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(count(&run, "footnote"), 2);
}

#[test]
fn a_command_another_feature_defines_is_the_language_s_clash() {
    let error = build_error(XnotesConfig { kinds: vec![XnoteKind::footnote().commands(["section"])] });
    assert!(error.contains("section"), "{error}");
}

// ---------------------------------------------------------------------------------------------
// a kind another feature contributes (design §7.3)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_contributed_kind_s_nodes_are_collected_numbered_and_anchored() {
    let registry = common::xnotes::registry();
    let run = check_with(&registry, &["A\\taddendum{a} B\\footnote{f} C\\taddendum{b}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(numbers(&run, "addendum"), [Some(vec![1]), Some(vec![2])]);
    assert_eq!(numbers(&run, "footnote"), [Some(vec![1])]);
    assert_eq!(anchors(&run, "addendum")[1].0.as_deref(), Some("add-2"));
    // A `\taddendum` inside a footnote is an addendum like any.
    let run = check_with(&registry, &["A\\footnote{\\taddendum{a}}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(count(&run, "addendum"), 1);
    assert_eq!(numbers(&run, "addendum"), [Some(vec![1])]);
}

#[test]
fn a_contributed_kind_builds_and_renders_without_the_xnotes_feature() {
    let mut builder = RegistryBuilder::new();
    for feature in package_features_but(StdcoreConfig::new(), xnotes::FEATURE) {
        builder.feature(feature);
    }
    builder.feature(Arc::new(common::xnotes::Addenda));
    let registry = builder.build(&RegistryInputs::new()).expect("a registry");
    let notes: Vec<String> = registry
        .build_diagnostics()
        .iter()
        .filter(|d| d.identifier() == "flm.registry.unresolved-peer")
        .map(|d| d.to_string())
        .collect();
    assert!(notes.iter().any(|note| note.contains("flm.xnotes.kinds")), "{notes:#?}");
    // A feature contributing the kind and nothing else renders, reporting
    // nothing: the kind's builder is scheduled, reads no table of xnotes,
    // writes no content, and no list is placed. (The test feature's own
    // `\taddendum` rule reads the table of xnotes as a hard dependency, so it is
    // set aside here and a plan cannot lower `\taddendum`: that is the
    // contributor's rule, not the kind's.)
    let mut builder = RegistryBuilder::new();
    for feature in package_features_but(StdcoreConfig::new(), xnotes::FEATURE) {
        builder.feature(feature);
    }
    builder.feature(common::pipeline::feature("t.contributor", |b| {
        xnotes::contribute_kind(b, Arc::new(common::xnotes::kind()))
    }));
    let registry = builder.build(&RegistryInputs::new()).expect("a registry");
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
    assert!(scheduled(&plan, xnotes::builder_name("addendum").as_str()));
    let run = render_listed(&registry, &["A B"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "A B\n");
}

#[test]
fn a_contributor_may_declare_the_list_rule_s_peers_itself_before_or_after_contributing() {
    // `contribute_kind` declares every peer its rules need; a contributor
    // declaring some of them itself, in either order and for several kinds,
    // builds all the same.
    for before in [true, false] {
        let mut builder = RegistryBuilder::new();
        builder.use_package(package(StdcoreConfig::new()));
        builder.use_package(flm_core::math_passthrough::package());
        builder.feature(common::pipeline::feature("t.contributor", move |b| {
            let own = |b: &mut FeatureBuilder<'_>| {
                b.peer_element::<flm_core::stdcore::backends::pres::SpanPayload>(
                    flm_core::stdcore::backends::pres::SPAN,
                    "the contributor's own span",
                )
            };
            if before {
                own(b)?;
            }
            xnotes::contribute_kind(b, Arc::new(common::xnotes::kind()))?;
            xnotes::contribute_kind(
                b,
                Arc::new(
                    XnoteKind::numbered("otheraddendum", PrefixWords::new("O~", "Os~"))
                        .label_type("oa"),
                ),
            )?;
            if !before {
                own(b)?;
            }
            Ok(())
        }));
        let registry = builder.build(&RegistryInputs::new());
        assert!(registry.is_ok(), "before = {before}: {}", registry.err().unwrap().render());
    }
}

#[test]
fn two_features_contributing_kinds_of_one_name_fail_the_build() {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().xnotes(XnotesConfig::new().kind(
        XnoteKind::numbered("addendum", PrefixWords::new("A~", "As~")).label_type("a"),
    ))));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(Arc::new(common::xnotes::Addenda));
    let error = match builder.build(&RegistryInputs::new()) {
        Ok(_) => panic!("the build must fail"),
        Err(error) => error.render(),
    };
    // The numbering feature meets the two kinds' styles first.
    assert!(
        error.contains("two features declare the counter style `flm.xnotes.style.addendum`"),
        "{error}"
    );
}

// ---------------------------------------------------------------------------------------------
// execution settings, and documents that must neither panic nor loop (§4 item 9)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_records_are_the_same_sequentially_and_in_parallel() {
    let registry = std();
    let texts = ["A\\footnote{a} B\\footnote[\\label{fn:b}]{b}", "C\\footnote{c}", "D\\footnote{d}"];
    let doc = common::pipeline::doc(&registry, &texts);
    let plan = common::stdcore::plan(&registry, CHECK_WORKFLOW, &with_xnotes());
    let sequential = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    let parallel = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::new());
    assert_eq!(records(&sequential), records(&parallel));
    assert_eq!(anchors(&sequential, "footnote"), anchors(&parallel, "footnote"));
    assert_eq!(numbers(&sequential, "footnote"), numbers(&parallel, "footnote"));
}

#[test]
fn hard_documents_neither_panic_nor_loop() {
    let deep = format!("{}x{}", "\\footnote{".repeat(10), "}".repeat(10));
    let registry = registry(XnotesConfig::new().kind(sidenote()));
    for document in [
        deep.as_str(),
        "\\footnote[]{}",
        "\\footnote[]{x}",
        "\\footnote{}",
        "\\footnote",
        "\\footnote[",
        "\\footnote[\\label{fn:a}",
        "\\footnote[\\footnote{x}]{y}",
        "\\footnote[\\label{}]{y}",
        "\\footnote[\\label]{y}",
        "\\section{\\footnote{a\n\nb}}",
        "\\sidenote{a\n\nb} \\section{T\\sidenote{a\n\nb}}",
        "\\(\\text{\\sidenote{a\n\nb}}\\)",
        "\\begin{itemize}\\item \\footnote{x\\begin{enumerate}\\item y\\end{enumerate}}\\end{itemize}",
        "\\textbf{\\sidenote{p\n\nq}}",
        "\\ref{fn:a}\\footnote[\\label{fn:a}\\label{fn:a}]{x}",
    ] {
        let run = check_with(&registry, &[document]);
        let _ = run.table(XNOTES);
    }
    let run = check_with(&registry, &[deep.as_str()]);
    assert_eq!(count(&run, "footnote"), 10);
}


// ---------------------------------------------------------------------------------------------
// the digest, and the aspect of the element
// ---------------------------------------------------------------------------------------------

#[test]
fn the_configuration_enters_the_registry_digest() {
    let base = std().digest();
    assert_eq!(std().digest(), base);
    let placed = registry(XnotesConfig {
        kinds: vec![XnoteKind::footnote().placement(XnotePlacement::InPlace)],
    });
    assert_ne!(placed.digest(), base);
    assert_ne!(registry(XnotesConfig::new().kind(sidenote())).digest(), base);
}

#[test]
fn the_element_answers_one_xnote_of_its_kind_its_content_argument() {
    let run = check(&["A\\footnote{x}"]);
    let tree = tree(&run);
    let node = nodes_of(tree, XNOTE)[0];
    let source = node.ext().element.aspect::<dyn XnoteSource>().expect("the aspect");
    assert_eq!(
        source.xnotes(&node),
        [XnoteOf { kind: "footnote".into(), content: NodeRegion::Argument(1) }]
    );
    // The content apart from the text is derived from that answer, the
    // labels argument beside it.
    let apart = flm_core::model::aspects::apart_from_text_of(&node);
    assert_eq!(
        apart,
        [
            flm_core::model::aspects::ContentRef::ArgumentNamed("labels".into()),
            flm_core::model::aspects::ContentRef::Argument(1),
        ]
    );
    assert_eq!(xnotes::regions_apart_from_text(&node), [flm_core::model::aspects::ContentRef::Argument(1)]);
}

// ---------------------------------------------------------------------------------------------
// the general rules over the text workflow (tests 26, 35–38, 40, 43, 44, 46, 48)
// ---------------------------------------------------------------------------------------------

use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule,
};
use flm_core::dump::golden::{assert_json_golden, assert_text_golden};
use flm_core::stdcore::backends::pres::{
    self, BlockPayload, DocRole, LinkPayload, SpanPayload, SpanStyle, payload_of,
};
use flm_core::stdcore::features::numbering::{CounterStyleDecl, NumberingConfig};
use flm_core::model::names::Name;

/// The worked example of a note holding inline content.
const CLAIM: &str = "The claim holds in every case.\\footnote{Except the empty one, which we \
                     exclude by convention.} The next sentence goes on.";

/// The worked example of a note holding two paragraphs.
const TWO_PARAGRAPHS: &str = "Two paragraphs.\\footnote{First paragraph.\n\nSecond paragraph.} And on.";

/// The default kind shown in place, and the unnumbered in-place `sidenote`.
fn in_place() -> Arc<FlmRegistry> {
    registry(XnotesConfig {
        kinds: vec![XnoteKind::footnote().placement(XnotePlacement::InPlace), sidenote()],
    })
}

/// The lowered form of the first tree of a run.
fn lowered_tree(run: &Run) -> Arc<NodeTree<Flm>> {
    Arc::clone(&common::lowering::lowered(run, TreeId::tree(0)).tree)
}

/// The payload of every node of the element `element` of the lowered form of
/// the first tree of a run, in document order.
fn payloads<P: flm_core::lang::PresPayload + Clone>(
    run: &Run,
    element: &str,
) -> Vec<P> {
    let tree = lowered_tree(run);
    nodes_of(&tree, element).iter().filter_map(|node| payload_of::<P>(node).cloned()).collect()
}

/// The spans of a run's first lowered tree whose role is `role`.
fn spans_of_role(run: &Run, role: &str) -> Vec<SpanPayload> {
    payloads::<SpanPayload>(run, pres::SPAN)
        .into_iter()
        .filter(|span| span.role.as_ref().is_some_and(|name| name.as_str() == role))
        .collect()
}

#[test]
fn the_worked_example_prints_the_mark_where_the_xnote_stands_and_the_list_after_the_content() {
    let run = render_listed(&std(), &[CLAIM, TWO_PARAGRAPHS]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    // The marks where the xnotes stand; the list under its `Footnotes`
    // heading, the labels hanging the entries at one column, the second
    // paragraph of an entry indented to that column after a blank line.
    assert_text_golden(common::lowering::golden_path("xnotes-worked-example.txt"), &printed(&run));
    assert!(printed(&run).starts_with("The claim holds in every case.\u{b9} The next"), "{}", printed(&run));
    assert!(
        printed(&run).ends_with(
            "Footnotes\n---------\n\n\u{b9} Except the empty one, which we exclude by convention.\n\n\
             \u{b2} First paragraph.\n\n  Second paragraph.\n"
        ),
        "{}",
        printed(&run)
    );
}

#[test]
fn the_mark_is_a_superscript_span_identified_as_the_mark_around_a_link_to_the_xnote() {
    let run = render_with(&std(), CLAIM);
    assert_json_golden(
        common::lowering::golden_path("xnotes-mark-lowered.json"),
        &common::lowering::lowered_projection(&run),
    );
    let marks = spans_of_role(&run, xnotes::MARK_ROLE);
    assert_eq!(marks.len(), 1);
    assert_eq!(marks[0].style, Some(SpanStyle::Superscript));
    assert!(is_positional(marks[0].anchor.as_deref().expect("identified"), "fnref", 1), "{marks:?}");
    let links = payloads::<LinkPayload>(&run, pres::LINK);
    assert_eq!(
        links,
        [LinkPayload {
            href: "#fn-1".into(),
            kind: xnotes::LINK_KIND.into(),
            doc_role: Some(DocRole::NoteRef),
            label: Some("Footnote 1".into()),
            shows_target: false,
        }]
    );
}

#[test]
fn a_space_before_a_listed_xnote_is_absorbed() {
    let run = render_with(&std(), "case. \\footnote{x} The next.\\footnote{y}");
    assert_eq!(printed(&run), "case.\u{b9} The next.\u{b2}\n");
}

#[test]
fn an_in_place_xnote_holding_inline_content_is_one_bracketed_span_where_it_stands() {
    let run = render_with(
        &in_place(),
        "The claim holds in every case. \\footnote{Except the empty one.} The next.\n\n\
         case.\\footnote{x} The\n\nAn aside stands here. \\sidenote{An aside.} And on.",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    // The author's spacing is kept; the unnumbered kind has an empty mark.
    assert_eq!(
        printed(&run),
        "The claim holds in every case. [\u{b9}Except the empty one.] The next.\n\n\
         case.[\u{b2}x] The\n\nAn aside stands here. [An aside.] And on.\n"
    );
    let spans = spans_of_role(&run, xnotes::INPLACE_ROLE);
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[0].kinds, [Name::from("xnotes"), Name::from("footnote")]);
    assert_eq!(spans[0].anchor.as_deref(), Some("fn-1"));
    assert_eq!(spans[0].doc_role, Some(DocRole::Footnote));
    assert_eq!(spans[0].style, None);
    assert_eq!(spans[2].kinds, [Name::from("xnotes"), Name::from("sidenote")]);
    assert!(is_positional(spans[2].anchor.as_deref().expect("identified"), "sn", 1), "{spans:?}");
    // Every xnote has its mark span, the unnumbered one empty, and no mark is
    // a link: the xnote is right there.
    let marks = spans_of_role(&run, xnotes::MARK_ROLE);
    assert_eq!(marks.len(), 3);
    assert!(is_positional(marks[2].anchor.as_deref().expect("identified"), "snref", 1), "{marks:?}");
    assert!(payloads::<LinkPayload>(&run, pres::LINK).is_empty());
}

#[test]
fn an_in_place_xnote_holding_paragraphs_is_a_block_between_the_paragraph_s_halves() {
    let run = render_with(&in_place(), TWO_PARAGRAPHS);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_text_golden(common::lowering::golden_path("xnotes-in-place-blocks.txt"), &printed(&run));
    assert_eq!(printed(&run), "Two paragraphs.\n\n[\u{b9}First paragraph.\n\nSecond paragraph.]\n\nAnd on.\n");
    assert_json_golden(
        common::lowering::golden_path("xnotes-in-place-blocks-lowered.json"),
        &common::lowering::lowered_projection(&run),
    );
    let blocks = payloads::<BlockPayload>(&run, pres::BLOCK);
    assert_eq!(
        blocks,
        [BlockPayload {
            role: Some(xnotes::INPLACE_ROLE.into()),
            kinds: vec!["xnotes".into(), "footnote".into()],
            anchor: Some("fn-1".into()),
            doc_role: Some(DocRole::Footnote),
        }]
    );
}

#[test]
fn an_in_place_block_where_no_block_stands_is_set_inline_its_paragraphs_joined() {
    let run = render_with(
        &in_place(),
        "\\section{Title \\footnote{a\n\nb}}\n\nText \\textbf{bold \\footnote{c\n\nd}} and \
         \\(x\\text{\\footnote{e\n\nf}}\\).",
    );
    // The language layer reports each once; the rule reports nothing more.
    assert_eq!(errors(&run), ["flm.blocks.block-in-inline"; 3]);
    let out = printed(&run);
    // The heading's underline covers the joined xnote.
    assert!(out.starts_with("Title [\u{b9}a b]\n============\n"), "{out}");
    assert!(out.contains("Text bold [\u{b2}c d] and"), "{out}");
    assert!(payloads::<BlockPayload>(&run, pres::BLOCK).is_empty());
}

#[test]
fn an_in_place_block_alone_in_an_inline_fragment_is_set_inline() {
    // The fragment's root is laid out inline: the note alone at its top level
    // stands in an inline region, as it does with text around it.
    let registry = in_place();
    for text in ["\\footnote{a\n\nb}", "x \\footnote{a\n\nb} y"] {
        let doc = common::wiring::doc(&registry, FragmentLayout::Inline, &[text]);
        let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
        let run = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
        assert_eq!(errors(&run), ["flm.blocks.block-in-inline"], "{text:?}");
        assert!(payloads::<BlockPayload>(&run, pres::BLOCK).is_empty(), "{text:?}");
        assert!(printed(&run).contains("[\u{b9}a b]"), "{text:?}: {}", printed(&run));
    }
    // Laid out as blocks, the same note alone is a block.
    let doc = common::wiring::doc(&registry, FragmentLayout::Blocks, &["\\footnote{a\n\nb}"]);
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
    let run = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(payloads::<BlockPayload>(&run, pres::BLOCK).len(), 1);
}

#[test]
fn an_in_place_block_in_text_inside_a_display_equation_is_an_xnote_set_inline() {
    // The math feature hands `\text{…}` in a display environment to the
    // ordinary rules: the xnote is lowered there, in the inline shape.
    let run = render_with(&in_place(), "\\begin{equation}x\\text{ y\\footnote{e\n\nf}}\\end{equation}");
    assert_eq!(errors(&run), ["flm.blocks.block-in-inline"]);
    assert!(payloads::<BlockPayload>(&run, pres::BLOCK).is_empty());
    let spans = spans_of_role(&run, xnotes::INPLACE_ROLE);
    assert_eq!(spans.len(), 1, "{spans:?}");
    assert_eq!(spans[0].anchor.as_deref(), Some("fn-1"));
}

#[test]
fn an_in_place_block_in_a_list_item_splits_the_item_s_paragraph() {
    let run = render_with(&in_place(), "\\begin{itemize}\\item one \\footnote{x\n\ny} two\\end{itemize}");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "\u{2022} one\n\n  [\u{b9}x\n\n  y]\n\n  two\n");
}

#[test]
fn an_xnote_in_a_list_item_is_marked_in_the_item_s_paragraph() {
    let run = render_with(&std(), "\\begin{itemize}\\item one\\footnote{x} two\\end{itemize}");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "\u{2022} one\u{b9} two\n");
}

#[test]
fn a_content_beginning_or_ending_with_a_list_gets_a_paragraph_for_the_bracket() {
    let run = render_with(&in_place(), "Two.\\footnote{\\begin{itemize}\\item a\\end{itemize}} And.");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "Two.\n\n[\u{b9}\n\n\u{2022} a\n\n]\n\nAnd.\n");
}

#[test]
fn a_nested_xnote_in_place_is_an_xnote_with_its_mark_and_its_number() {
    let run = render_with(&in_place(), "A\\footnote{a \\footnote[\\label{fn:in}]{b} c} B\\footnote{d} See \\ref{fn:in}.");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "A[\u{b9}a [\u{b2}b] c] B[\u{b3}d] See Footnote\u{a0}2.\n");
    assert_eq!(count(&run, "footnote"), 3);
    let spans = spans_of_role(&run, xnotes::INPLACE_ROLE);
    assert_eq!(spans.len(), 3);
    assert_eq!(spans[1].anchor.as_deref(), Some("fn-2"));
    assert_eq!(spans_of_role(&run, xnotes::MARK_ROLE).len(), 3);
}

#[test]
fn a_nested_xnote_holding_paragraphs_inside_an_in_place_block_is_a_block_of_its_own() {
    let run = render_with(&in_place(), "A\\footnote{a \\footnote{b\n\nc} d} B");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "A\n\n[\u{b9}a\n\n[\u{b2}b\n\nc]\n\nd]\n\nB\n");
}

#[test]
fn xnotes_nested_ten_deep_neither_panic_nor_loop() {
    let mut document = String::from("x");
    for _ in 0..10 {
        document = format!("a\\footnote{{{document}}}");
    }
    let listed = render_listed(&std(), &[&document]);
    // Every xnote is listed, each entry holding the next one's mark.
    assert_eq!(identifiers(&listed), Vec::<String>::new());
    assert_eq!(count(&listed, "footnote"), 10);
    let out = printed(&listed);
    assert!(out.starts_with("a\u{b9}\n\nFootnotes\n---------\n\n\u{b9}  a\u{b2}\n\n"), "{out}");
    assert!(out.ends_with("\u{b9}\u{2070} x\n"), "{out}");
    let placed = render_with(&in_place(), &document);
    assert_eq!(identifiers(&placed), Vec::<String>::new());
    assert_eq!(count(&placed, "footnote"), 10);
    assert!(printed(&placed).starts_with("a[\u{b9}a[\u{b2}a[\u{b3}a["), "{}", printed(&placed));
    assert!(printed(&placed).trim_end().ends_with("x]]]]]]]]]]"), "{}", printed(&placed));
}

#[test]
fn an_xnote_in_a_heading_title_is_marked_there_and_in_a_reference_to_the_heading() {
    let run = render_with(&std(), "\\section{Title\\footnote{t}}\\label{sec:x}\n\nSee \\ref{sec:x}.");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    // The underline measures the mark; the reference shows the title with
    // the note's mark.
    assert_eq!(printed(&run), "Title\u{b9}\n======\n\nSee Title\u{b9}.\n");
}

#[test]
fn an_empty_xnote_is_reported_and_marked() {
    let run = render_with(&std(), "A\\footnote{} B\\footnote[]{ } C\\footnote{%%\n} D\\footnote{~}");
    assert_eq!(reports(&run), vec![("flm.xnotes.empty".to_string(), Severity::Warning); 4]);
    assert_eq!(printed(&run), "A\u{b9} B\u{b2} C\u{b3} D\u{2074}\n");
    let placed = render_with(&in_place(), "A \\footnote{} B");
    assert_eq!(identifiers(&placed), ["flm.xnotes.empty"]);
    assert_eq!(printed(&placed), "A [\u{b9}] B\n");
}

#[test]
fn a_refused_optional_argument_is_reported_once_and_its_label_still_stands() {
    let run = render_with(&std(), "A\\footnote[\\textbf{x}\\label{fn:e}]{x}\n\nSee \\ref{fn:e}.");
    assert_eq!(errors(&run), ["flm.xnotes.bad-label-argument"]);
    let report = run.diagnostics().iter().next().expect("one").to_string();
    assert!(report.contains("\\textbf{x}"), "{report}");
    assert_eq!(printed(&run), "A\u{b9}\n\nSee Footnote\u{a0}1.\n");
}

#[test]
fn a_replaced_mark_style_changes_the_mark_and_not_the_reference() {
    let mut mark = CounterStyleDecl::new(xnotes::mark_style_name("footnote"));
    mark.number_format = NumberTemplate::formats("[#{arabic}]");
    let registry = registry_with(
        StdcoreConfig::new().numbering(NumberingConfig::new().style(mark)),
        XnotesConfig::new(),
    );
    let run = render_with(&registry, "A\\footnote[\\label{fn:a}]{x}\n\nSee \\ref{fn:a}.");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    // A bracket has no superscript form and is written as it stands.
    assert_eq!(printed(&run), "A[\u{b9}]\n\nSee Footnote\u{a0}1.\n");
    // The link's name is the reference's number, not the mark's text.
    assert_eq!(payloads::<LinkPayload>(&run, pres::LINK)[0].label.as_deref(), Some("Footnote 1"));
}

#[test]
fn a_mark_style_writing_nothing_writes_an_empty_mark() {
    let mut mark = CounterStyleDecl::new(xnotes::mark_style_name("footnote"));
    mark.number_format = NumberTemplate::formats("");
    let registry = registry_with(
        StdcoreConfig::new().numbering(NumberingConfig::new().style(mark)),
        XnotesConfig::new(),
    );
    let run = render_with(&registry, "A\\footnote{x} B");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "A B\n");
    assert_eq!(spans_of_role(&run, xnotes::MARK_ROLE).len(), 1);
}

#[test]
fn the_mark_of_a_kind_numbered_within_sections_is_its_own_number() {
    let run = render_with(
        &within_sections(),
        "\\section{One}\nA\\footnote{a}\n\n\\section{Two}\nB\\footnote[\\label{fn:b}]{b} C\\footnote{c}\n\n\
         See \\ref{fn:b}.",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let out = printed(&run);
    assert!(out.contains("A\u{b9}\n"), "{out}");
    assert!(out.contains("B\u{b9} C\u{b2}\n"), "{out}");
    assert!(out.contains("See Footnote\u{a0}2.1."), "{out}");
    let links = payloads::<LinkPayload>(&run, pres::LINK);
    let marks: Vec<(&str, Option<&str>)> =
        links.iter().filter(|link| link.kind.as_str() == xnotes::LINK_KIND).map(|link| (link.href.as_str(), link.label.as_deref())).collect();
    assert_eq!(
        marks,
        [("#fn-1-1", Some("Footnote 1.1")), ("#fn-2-1", Some("Footnote 2.1")), ("#fn-2-2", Some("Footnote 2.2"))]
    );
}

#[test]
fn an_unnumbered_listed_kind_writes_an_empty_mark_span_linking_to_the_xnote() {
    let aside = XnoteKind::unnumbered("aside", PrefixWords::new("Aside~", "Asides~"))
        .commands(["aside"])
        .label_type("as");
    let run = render_with(&registry(XnotesConfig::new().kind(aside)), "A\\aside{x} B");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "A B\n");
    let marks = spans_of_role(&run, xnotes::MARK_ROLE);
    assert!(is_positional(marks[0].anchor.as_deref().expect("identified"), "asref", 1), "{marks:?}");
    let link = &payloads::<LinkPayload>(&run, pres::LINK)[0];
    assert!(link.href.starts_with("#as--1"), "{link:?}");
    assert_eq!(link.label.as_deref(), Some("Aside"));
}

/// The unnumbered listed kind `aside`, written `\aside`, whose fixed mark
/// is `mark`.
fn aside_marked(mark: &str) -> XnoteKind {
    XnoteKind::unnumbered("aside", PrefixWords::new("Aside~", "Asides~"))
        .commands(["aside"])
        .label_type("as")
        .mark(NumberTemplate::formats(mark))
}

#[test]
fn an_unnumbered_kind_s_fixed_mark_is_compiled_as_flm() {
    let run = render_with(&registry(XnotesConfig::new().kind(aside_marked("\u{2020}"))), "A\\aside{x} B");
    assert_eq!(printed(&run), "A\u{2020} B\n");
    // `~` is a non-breaking space, not the character `~`.
    let run = render_with(&registry(XnotesConfig::new().kind(aside_marked("~*"))), "A\\aside{x} B");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "A\u{a0}* B\n");
    // A macro is what it stands for in the registry's language.
    let dag = Definition::macro_("dag", Vec::new()).text("\u{2020}");
    let registry = registry_with(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(vec![dag])),
        XnotesConfig::new().kind(aside_marked("\\dag")),
    );
    let run = render_with(&registry, "A\\aside{x} B");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "A\u{2020} B\n");
    // In place, the mark is the same content, before the xnote's content.
    let inplace = aside_marked("\\dag").placement(XnotePlacement::InPlace);
    let dag = Definition::macro_("dag", Vec::new()).text("\u{2020}");
    let registry = registry_with(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(vec![dag])),
        XnotesConfig::new().kind(inplace),
    );
    let run = render_with(&registry, "A \\aside{x} B");
    assert_eq!(printed(&run), "A [\u{2020}x] B\n");
}

#[test]
fn a_fixed_mark_that_is_not_valid_flm_fails_the_registry_build() {
    let error = build_error(XnotesConfig::new().kind(aside_marked("\\textbf{x")));
    assert!(error.contains("the mark `\\textbf{x` of the xnote kind `aside`"), "{error}");
}

/// The standard package's features but `refs`, with the xnotes feature under
/// its default configuration.
fn without_refs() -> Arc<FlmRegistry> {
    registry_over(vec![
        Arc::new(Blocks::default()) as Arc<dyn FlmFeature>,
        index::feature(),
        refpoints::feature(),
        numbering::feature(),
        Arc::new(XnotesFeature::new(XnotesConfig::new())),
        lowering::feature(),
        pieces::feature(),
        text::feature(),
        check_feature(),
    ])
}

#[test]
fn without_the_references_feature_the_mark_is_the_bare_span() {
    let run = render_with(&without_refs(), "A\\footnote{x} B\\footnote[\\label{fn:e}]{y}");
    // The label package is absent: `\label` is unknown there, and refused.
    assert_eq!(errors(&run), ["core.specs.unresolvable-command", "flm.xnotes.bad-label-argument"]);
    assert_eq!(printed(&run), "A\u{b9} B\u{b2}\n");
    assert!(payloads::<LinkPayload>(&run, pres::LINK).is_empty());
    let marks = spans_of_role(&run, xnotes::MARK_ROLE);
    assert_eq!(marks.len(), 2);
    assert!(marks.iter().all(|mark| mark.anchor.is_none()), "{marks:?}");
}

#[test]
fn without_numbering_the_marks_are_empty_and_the_kind_is_reported_once() {
    let run = render_with(&without_numbering(XnotesConfig::new()), "A\\footnote{x} B\\footnote{y}");
    assert_eq!(errors(&run), ["flm.xnotes.no-numbering"]);
    assert_eq!(printed(&run), "A B\n");
    let links = payloads::<LinkPayload>(&run, pres::LINK);
    assert!(links.iter().all(|link| link.label.as_deref() == Some("Footnote")), "{links:?}");
}

#[test]
fn the_output_is_the_same_however_the_run_is_executed() {
    let registry = in_place();
    let document = format!("{TWO_PARAGRAPHS}\n\n{CLAIM} \\sidenote{{s}} \\footnote{{a\\footnote{{b}}}}");
    let doc = common::pipeline::doc(&registry, &[&document]);
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
    let sequential =
        common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    let parallel = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::new());
    assert_eq!(printed(&sequential), printed(&parallel));
    assert_eq!(identifiers(&sequential), identifiers(&parallel));
}

#[test]
fn an_xnote_in_every_place_is_written_without_panic_under_both_placements() {
    use flm_core::lang::rewrites::userdefs::Definition;
    use flm_core::stdcore::features::userdefs::UserdefsConfig;
    let document = "\\section{T\\footnote{h}}\\label{sec:t}\n\n\
                    P\\footnote[\\label{fn:p}]{p} \\(x\\footnote{m}\\) \\(y\\text{\\footnote{t}}\\) \\noted{}\n\n\
                    \\begin{itemize}\\item i\\footnote{i\n\nj}\\end{itemize}\n\n\
                    \\footnote[]{} \\footnote{a\\footnote{b\\footnote{c}}} See \\ref{sec:t}, \\ref{fn:p}.";
    let definitions = UserdefsConfig::new()
        .definitions(vec![Definition::macro_("noted", Vec::new()).text("d\\footnote{from a definition}")]);
    for kind in [XnoteKind::footnote(), XnoteKind::footnote().placement(XnotePlacement::InPlace)] {
        let registry = registry_with(
            StdcoreConfig::new().userdefs(definitions.clone()),
            XnotesConfig { kinds: vec![kind] },
        );
        let run = render_with(&registry, document);
        // The reference shows the title with the note's mark.
        assert!(printed(&run).contains("See T\u{b9}, Footnote\u{a0}2."), "{}", printed(&run));
    }
}

/// A test feature declaring `\tentry`, whose rule splices a value holding an
/// entry of a list of xnotes and nothing around it: the one way a lowering
/// meets an entry outside its list, since no document can write one.
struct EntryOutside;

/// The rule of `\tentry`.
struct SpliceEntry;

impl Lower for SpliceEntry {
    fn lower(&self, node: NodeRef<'_, Flm>, cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer) -> Result<Lowering, LowerError> {
        let entry = Arc::new(FlmSynthesizedSpec::new(Arc::new(ElementSpec::new(xnotes::ENTRY))));
        let mut visitor = EntryOver { entry, content: &[] };
        let value = guard::restager(&mut visitor, GuardPolicy::StackBudget)
            .restage(node.tree())
            .map_err(|error| LowerError::other(error.to_string()))?;
        Ok(Lowering::Emit(lw.splice(cx, &Arc::new(value))?))
    }
}

impl FlmFeature for EntryOutside {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t-entry", "Entry outside")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let owner = b.feature().clone();
        b.element(ElementDef::new(
            owner,
            ["tentry"],
            SyntaxDef::Macro { args: Vec::new() },
            Arc::new(ElementSpec::new("t.entry")),
        ))?;
        b.lowering_rule(LoweringRule::new(
            "t.entry.lower",
            "t.entry",
            LoweringDecl::new(),
            Arc::new(SpliceEntry),
            DefaultMark::Everywhere,
        ))
    }
}

#[test]
fn an_entry_outside_its_list_writes_nothing_and_is_reported() {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(Arc::new(EntryOutside));
    let registry = builder.build(&RegistryInputs::new()).expect("the registry");
    let run = render_with(&registry, "A \\tentry{} B");
    assert_eq!(errors(&run), ["flm.xnotes.entry-outside-list"]);
    assert_eq!(printed(&run), "A B\n");
}

#[test]
fn the_check_workflow_schedules_no_lowering_no_list_and_no_pieces() {
    let registry = std();
    let plan = common::stdcore::plan(&registry, CHECK_WORKFLOW, &Overrides::none());
    let names: Vec<String> = plan.stage_names().iter().map(ToString::to_string).collect();
    assert!(!names.iter().any(|name| name.contains("pieces") || name.contains("build-list")), "{names:?}");
    assert!(!names.iter().any(|name| name.contains("lower")), "{names:?}");
    // The collect stage runs where something reads its table: a render.
    assert!(!names.iter().any(|name| name == xnotes::COLLECT), "{names:?}");
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
    assert!(plan.stage_names().iter().any(|name| name.as_str() == xnotes::COLLECT));
}

use flm_core::core::data::store::StageOut;
use flm_core::core::plan::PlanCx;
use flm_core::core::run::StageCx;
use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::stdcore::backends::pres::{HeadingPayload, ListPayload};

// ---------------------------------------------------------------------------------------------
// the list of each kind (design §7): the builder, the rule, the override, a contributed kind
// ---------------------------------------------------------------------------------------------

/// The content the builder of the kind `kind` wrote, as its plain text, or
/// `None` where it wrote nothing to place.
fn piece_text(run: &Run, kind: &str) -> Option<String> {
    let piece = run.table(&xnotes::piece_table(kind)).expect("declared").expect("written");
    piece.content.as_ref().map(|content| flm_core::lang::text::plain_text(content))
}

/// Whether the plan of `run` scheduled the stage `stage`.
fn scheduled(plan: &flm_core::core::plan::Plan, stage: &str) -> bool {
    plan.stage_names().iter().any(|name| name.as_str() == stage)
}

#[test]
fn a_second_kind_gets_its_own_list_after_the_first() {
    let registry = registry(XnotesConfig::new().kind(numbered("endnote", "en")));
    let run = render_listed(&registry, &["A\\footnote{f} B\\endnote{e} C\\footnote{g}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    // Two pieces, in the order the kinds were declared; each list numbered
    // on its own.
    assert_eq!(
        printed(&run),
        "A\u{b9} B\u{b9} C\u{b2}\n\nFootnotes\n---------\n\n\u{b9} f\n\n\u{b2} g\n\n\
         Endnotes\n--------\n\n\u{b9} e\n"
    );
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
    assert!(scheduled(&plan, xnotes::builder_name("footnote").as_str()));
    assert!(scheduled(&plan, xnotes::builder_name("endnote").as_str()));
}

#[test]
fn the_list_is_a_block_holding_a_heading_and_a_labeled_list_identifying_each_entry() {
    let run = render_listed(&std(), &["A\\footnote{x} B\\footnote[\\label{fn:y}]{y}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let pieces = run.table(&pieces::lowered_pieces_table(&text::BACKEND.into())).expect("declared").expect("written");
    let piece = pieces.pieces.iter().find(|p| p.name.as_str() == "flm.xnotes.list.footnote").expect("placed");
    let tree = &piece.tree.tree;
    let block = nodes_of(tree, pres::BLOCK);
    assert_eq!(block.len(), 1);
    let block = payload_of::<BlockPayload>(&block[0]).expect("a block payload").clone();
    assert_eq!(block.role.as_ref().map(|r| r.as_str()), Some("flm.xnotes.footnote.list"));
    assert_eq!(block.kinds, [Name::from("xnotes"), Name::from("footnote")]);
    assert_eq!(block.doc_role, Some(DocRole::Endnotes));
    let heading = nodes_of(tree, pres::BLOCK_HEADING);
    let heading = payload_of::<HeadingPayload>(&heading[0]).expect("a heading payload").clone();
    assert_eq!((heading.name.as_str(), heading.level, heading.in_toc), ("xnotes", 2, false));
    let list = nodes_of(tree, pres::LIST);
    let list = payload_of::<ListPayload>(&list[0]).expect("a list payload").clone();
    let kinds: Vec<&str> = list.kinds.iter().map(|k| k.as_str()).collect();
    assert_eq!(kinds, ["numbered", "xnotes", "footnote", "labeled"]);
    assert_eq!(list.entry_anchors, [Some("fn-1".to_string()), Some("fn-2".to_string())]);
    let labels: Vec<SpanPayload> = nodes_of(tree, pres::SPAN)
        .iter()
        .filter_map(|node| payload_of::<SpanPayload>(node).cloned())
        .collect();
    assert_eq!(labels.len(), 2);
    assert!(labels.iter().all(|label| label.style == Some(SpanStyle::Superscript)
        && label.role.as_ref().is_some_and(|r| r.as_str() == xnotes::ENTRY_MARK_ROLE)));
}

#[test]
fn an_unnumbered_block_content_xnote_is_listed_and_a_reference_to_it_shows_the_label() {
    let aside = XnoteKind::unnumbered("aside", PrefixWords::new("Aside~", "Asides~"))
        .commands(["aside"])
        .label_type("as")
        .mark(NumberTemplate::formats("*"));
    let run = render_listed(
        &registry(XnotesConfig::new().kind(aside)),
        &["A\\aside[\\label{as:x}]{One.\n\nTwo.} B\n\nSee \\ref{as:x}."],
    );
    // The aside has neither a number nor content to show.
    assert_eq!(identifiers(&run), ["flm.refs.nothing-to-show"]);
    assert_eq!(printed(&run), "A* B\n\nSee as:x.\n\nAsides\n------\n\n* One.\n\n  Two.\n");
}

#[test]
fn a_kind_with_no_title_has_no_heading_and_a_configured_title_prints_at_its_level() {
    let run = render_listed(&registry(XnotesConfig { kinds: vec![XnoteKind::footnote().without_title()] }), &["A\\footnote{x}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "A\u{b9}\n\n\u{b9} x\n");
    let titled = XnoteKind::footnote().title("\\emph{Notes}").title_level(3);
    let run = render_listed(&registry(XnotesConfig { kinds: vec![titled] }), &["A\\footnote{x}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "A\u{b9}\n\n### Notes\n\n\u{b9} x\n");
}

#[test]
fn a_title_that_is_not_valid_flm_fails_the_registry_build() {
    let error = build_error(XnotesConfig { kinds: vec![XnoteKind::footnote().title("\\emph{Notes")] });
    assert!(error.contains("the title `\\emph{Notes` of the xnote kind `footnote`"), "{error}");
    // A placeholder: the title's language has none.
    let error = build_error(XnotesConfig { kinds: vec![XnoteKind::footnote().title("Notes #{n}")] });
    assert!(error.contains("the title `Notes #{n}` of the xnote kind `footnote`"), "{error}");
}

#[test]
fn a_literal_hash_in_a_title_is_written_escaped_or_doubled() {
    for title in ["Notes \\#1", "Notes ##1"] {
        let kind = XnoteKind::footnote().title(title);
        let run = render_listed(&registry(XnotesConfig { kinds: vec![kind] }), &["A\\footnote{x}"]);
        assert_eq!(identifiers(&run), Vec::<String>::new(), "{title}");
        assert!(printed(&run).contains("Notes #1\n"), "{title}: {}", printed(&run));
    }
}

#[test]
fn a_kind_with_no_xnote_and_an_in_place_kind_place_no_list() {
    let run = render_listed(&in_place(), &["A \\footnote{x} B \\sidenote{y}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(piece_text(&run, "footnote"), None);
    assert_eq!(piece_text(&run, "sidenote"), None);
    assert!(!printed(&run).contains("Footnotes"), "{}", printed(&run));
    let run = render_listed(&std(), &["No note here."]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(piece_text(&run, "footnote"), None);
    assert_eq!(printed(&run), "No note here.\n");
}

#[test]
fn an_excluded_list_is_left_out_and_its_builder_is_not_scheduled() {
    let registry = std();
    let overrides = without_lists(&registry);
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &overrides);
    assert!(!scheduled(&plan, xnotes::builder_name("footnote").as_str()));
    let run = run_workflow(&registry, text::WORKFLOW, &[CLAIM], &overrides);
    assert_eq!(printed(&run), "The claim holds in every case.\u{b9} The next sentence goes on.\n");
}

#[test]
fn an_xnote_inside_a_listed_xnote_has_its_mark_in_the_entry_and_an_entry_of_its_own() {
    let run = render_listed(&std(), &["A\\footnote{a\\footnote[\\label{fn:in}]{b}} C. See \\ref{fn:in}."]);
    // Where the outer xnote stands only its mark is written; its entry shows
    // its content, where the inner xnote's mark stands, and the inner xnote's
    // entry follows. Nothing is reported, and the content is lowered once.
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        printed(&run),
        "A\u{b9} C. See Footnote\u{a0}2.\n\nFootnotes\n---------\n\n\u{b9} a\u{b2}\n\n\u{b2} b\n"
    );
    assert_eq!(count(&run, "footnote"), 2);
    // With the list left out, nothing is reported either.
    let run = render_with(&std(), "A\\footnote{a\\footnote{b}} C");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "A\u{b9} C\n");
}

#[test]
fn an_xnote_of_another_kind_inside_a_listed_xnote_lands_in_its_own_kind_s_list() {
    // An addendum inside a footnote: in the list of addenda.
    let run = render_listed(&common::xnotes::registry(), &["A\\footnote{As in \\taddendum{Knuth}.} B\\taddendum{Lamport}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        printed(&run),
        "A\u{b9} B\u{b2}\n\nFootnotes\n---------\n\n\u{b9} As in\u{b9}.\n\n\
         Addenda\n-------\n\n\u{b9} Knuth\n\n\u{b2} Lamport\n"
    );
    // A footnote inside an endnote, and an endnote inside a footnote.
    let run = render_listed(
        &registry(XnotesConfig::new().kind(numbered("endnote", "en"))),
        &["A\\endnote{e\\footnote{f}} B\\footnote{g\\endnote{h}}"],
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        printed(&run),
        "A\u{b9} B\u{b2}\n\nFootnotes\n---------\n\n\u{b9} f\n\n\u{b2} g\u{b2}\n\n\
         Endnotes\n--------\n\n\u{b9} e\u{b9}\n\n\u{b2} h\n"
    );
}

#[test]
fn an_in_place_xnote_and_a_listed_xnote_nest_in_each_other() {
    let run = render_listed(
        &registry(XnotesConfig::new().kind(sidenote())),
        &["A\\footnote{f \\sidenote{s}} B\\sidenote{t\\footnote{u}}"],
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let out = printed(&run);
    // The side note inside the footnote is set in place inside the entry; the
    // footnote inside the side note has its mark in place, and its entry.
    assert!(out.starts_with("A\u{b9} B[t\u{b2}]\n\nFootnotes\n"), "{out:?}");
    assert!(out.contains("\u{b9} f [s]\n"), "{out:?}");
    assert!(out.ends_with("\u{b2} u\n"), "{out:?}");
}

#[test]
fn a_reference_inside_an_entry_resolves_to_the_document_s_label() {
    let run = render_listed(
        &std(),
        &["\\section{Intro}\\label{sec:i}\n\nA\\footnote{See \\ref{sec:i} and \\ref{fn:b}.} B\\footnote[\\label{fn:b}]{b}"],
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert!(
        printed(&run).ends_with("\u{b9} See Intro and Footnote\u{a0}2.\n\n\u{b2} b\n"),
        "{}",
        printed(&run)
    );
}

#[test]
fn lists_inside_a_footnote_are_numbered_and_identified_in_its_entry_as_the_document_numbered_them() {
    // The entry shows the footnote's own content, so its items are the
    // document's points: numbered, and anchored where the list shows them,
    // with no report.
    let run = render_listed(
        &std(),
        &["A\\footnote{Two lists:\n\\begin{itemize}\\item one\\item two\\end{itemize}\n\
           \\begin{enumerate}\\item three\\item\\label{item:four} four\\end{enumerate}}\n\nSee \\ref{item:four}."],
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let out = printed(&run);
    assert!(out.starts_with("A\u{b9}\n\nSee Item\u{a0}2.\n"), "{out}");
    assert!(
        out.ends_with("\u{b9} Two lists:\n\n  • one\n\n  • two\n\n  1. three\n\n  2. four\n"),
        "{out}"
    );
    let lists = run.table(&pieces::lowered_pieces_table(&text::BACKEND.into())).expect("declared").expect("written");
    let inner: Vec<ListPayload> = nodes_of(&lists.pieces[0].tree.tree, pres::LIST)
        .iter()
        .filter_map(|node| payload_of::<ListPayload>(node).cloned())
        .filter(|list| list.kinds[0].as_str() == "numbered" && list.kinds[1].as_str() != "xnotes")
        .collect();
    assert_eq!(inner.len(), 1, "{inner:?}");
    assert!(inner[0].entry_anchors.iter().all(Option::is_some), "{inner:?}");
    assert_eq!(inner[0].entry_anchors[1].as_deref(), Some("item-four"));
}

#[test]
fn a_labeled_equation_inside_a_footnote_is_an_ordinary_point() {
    let run = render_listed(
        &std(),
        &["A\\footnote{See:\n\\begin{equation}\n  x = 1 \\label{eq:x}\n\\end{equation}\n} B\\footnote{b}\n\nSee \\eqref{eq:x}."],
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let out = printed(&run);
    assert!(out.starts_with("A\u{b9} B\u{b2}\n\nSee (1).\n\nFootnotes\n"), "{out}");
    assert!(out.contains("x = 1"), "{out}");
    assert!(out.ends_with("\u{b2} b\n"), "{out}");
}

#[test]
fn a_heading_inside_a_footnote_shows_its_number_and_its_anchor_where_the_list_shows_it() {
    let run = render_listed(
        &registry_with(
            StdcoreConfig::new().headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered)),
            XnotesConfig::new(),
        ),
        &["\\section{One}\n\nA\\footnote{\\section{Inner}\\label{sec:in}\n\nx}\n\n\\section{Two}\n\nSee \\ref{sec:in}."],
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let out = printed(&run);
    assert!(out.contains("1. One\n"), "{out}");
    assert!(out.contains("3. Two\n"), "{out}");
    assert!(out.contains("See \u{a7}\u{202f}2.") || out.contains("See \u{a7} 2."), "{out:?}");
    assert!(out.contains("2. Inner\n"), "{out}");
    let lists = run.table(&pieces::lowered_pieces_table(&text::BACKEND.into())).expect("declared").expect("written");
    let heading = nodes_of(&lists.pieces[0].tree.tree, pres::BLOCK_HEADING)
        .iter()
        .filter_map(|node| payload_of::<HeadingPayload>(node).cloned())
        .find(|heading| heading.name.as_str() == "section")
        .expect("the footnote's section");
    assert_eq!(heading.anchor.as_deref(), Some("sec-in"));
}

#[test]
fn an_unnumbered_listed_kind_labels_its_entries_with_its_fixed_mark_or_hangs_them_at_the_margin() {
    let run = render_listed(&registry(XnotesConfig::new().kind(aside_marked("\u{2020}"))), &["A\\aside{x} B\\aside{y}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert!(printed(&run).ends_with("Asides\n------\n\n\u{2020} x\n\n\u{2020} y\n"), "{}", printed(&run));
    let run = render_listed(&registry(XnotesConfig::new().kind(aside_marked(""))), &["A\\aside{x} B\\aside{y}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert!(printed(&run).ends_with("Asides\n------\n\nx\n\ny\n"), "{}", printed(&run));
    let lists = run.table(&pieces::lowered_pieces_table(&text::BACKEND.into())).expect("declared").expect("written");
    let tree = &lists.pieces[0].tree.tree;
    let list = payload_of::<ListPayload>(&nodes_of(tree, pres::LIST)[0]).expect("a list").clone();
    assert_eq!(list.kinds[0].as_str(), "unnumbered");
}

#[test]
fn the_list_is_the_same_however_the_run_is_executed() {
    let registry = registry(XnotesConfig::new().kind(numbered("endnote", "en")));
    let document = format!("{CLAIM}\n\n{TWO_PARAGRAPHS}\\endnote{{e\\footnote{{n}}}}");
    let doc = common::pipeline::doc(&registry, &[&document, "B\\footnote{c}"]);
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
    let sequential =
        common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    let parallel = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::new());
    assert_eq!(printed(&sequential), printed(&parallel));
    assert_eq!(identifiers(&sequential), identifiers(&parallel));
    assert!(printed(&sequential).contains("Endnotes"), "{}", printed(&sequential));
    // Every table, nested xnotes included, is the same.
    let snapshot = common::spine::snapshot(&sequential);
    assert_eq!(common::spine::snapshot(&parallel), snapshot);
    assert!(snapshot.contains("flm.xnotes = "), "{snapshot}");
}

#[test]
fn a_contributed_kind_is_listed_with_the_footnotes_machinery_in_a_list_of_its_own() {
    // Test 39: the programmatic path — `contribute_kind`, the aspect, the
    // points, the mark helper — and nothing else.
    let registry = common::xnotes::registry();
    let run = render_listed(&registry, &["A\\taddendum{Knuth} B\\footnote{f} C\\taddendum{Lamport}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        printed(&run),
        "A\u{b9} B\u{b9} C\u{b2}\n\nFootnotes\n---------\n\n\u{b9} f\n\n\
         Addenda\n-------\n\n\u{b9} Knuth\n\n\u{b2} Lamport\n"
    );
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
    assert!(scheduled(&plan, xnotes::builder_name("addendum").as_str()));
    let lists = run.table(&pieces::lowered_pieces_table(&text::BACKEND.into())).expect("declared").expect("written");
    let addenda = lists.pieces.iter().find(|p| p.name.as_str() == "flm.xnotes.list.addendum").expect("placed");
    let list = payload_of::<ListPayload>(&nodes_of(&addenda.tree.tree, pres::LIST)[0]).expect("a list").clone();
    assert_eq!(list.entry_anchors, [Some("add-1".to_string()), Some("add-2".to_string())]);
}

/// What the override builder of a test writes into the footnotes' piece.
#[derive(Clone, Copy)]
enum OverrideContent {
    /// A value parsed from this source, followed by the number of footnotes
    /// the table of xnotes records.
    Counted(&'static str),
    /// A value holding nothing at all.
    Empty,
    /// The footnotes' list with one entry, whose content pointer names this
    /// region of the first node of this element in the first tree: a list
    /// built by hand, pointing where it wants.
    Pointing(&'static str, NodeRegion),
}

/// A test feature overriding the footnotes' list: a document-scope stage
/// writing the kind's piece table from the table of xnotes.
struct OverrideList(OverrideContent);

/// The name of the override's stage.
const OVERRIDE_STAGE: &str = "t.override.build-footnotes";

/// The override's stage.
struct OverrideBuilder(OverrideContent);

impl Stage for OverrideBuilder {
    fn decl(&self) -> StageDecl {
        StageDecl::new(OVERRIDE_STAGE, Effect::Compute, Scope::Document)
            .writes(xnotes::piece_table("footnote"))
            .docs("writes the footnotes' list its own way")
    }

    fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
        Needs::new().reads(XNOTES)
    }

    fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
        let count = cx.get(XNOTES).expect("declared").map_or(0, |table| table.of_kind("footnote").len());
        let text = match self.0 {
            OverrideContent::Counted(source) => format!("{source} {count}."),
            OverrideContent::Empty => String::new(),
            OverrideContent::Pointing(element, region) => {
                *out.table(&xnotes::piece_table("footnote")).expect("declared") =
                    flm_core::core::piece::Piece::new(pointing_list(cx, element, region));
                return;
            }
        };
        let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(cx.registry().lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .expect("the override's source parses");
        *out.table(&xnotes::piece_table("footnote")).expect("declared") =
            flm_core::core::piece::Piece::new(Arc::clone(fragment.tree()));
    }
}

/// The footnotes' list of [`OverrideContent::Pointing`].
fn pointing_list(cx: &StageCx<'_>, element: &str, region: NodeRegion) -> Arc<NodeTree<Flm>> {
    let (id, tree) = cx.trees().next().expect("a tree");
    let target = tree.iter_storage_order().find(|node| node.ext().element.name == element).expect("the node");
    let spec = |name: &str| {
        let element = cx.registry().element_value(name).expect("known").expect("registered");
        Arc::new(FlmSynthesizedSpec::new(Arc::clone(element)))
    };
    let pointer = flm_core::lang::copy::SynthesizedPiece {
        spec: spec(xnotes::CONTENT_POINTER),
        slot: "body",
        span: target.span().clone(),
        state: Arc::clone(target.parsing_state()),
        body: Vec::new(),
        payload: Some(Arc::new(lowering::ContentPointer {
            target: flm_core::model::addr::NodeAddr::new(id, target.id()),
            region,
        })),
    };
    let entry = flm_core::lang::copy::SynthesizedPiece {
        spec: spec(xnotes::ENTRY),
        body: vec![ValuePiece::Synthesized(&pointer)],
        payload: None,
        ..pointer.clone()
    };
    let list = flm_core::lang::copy::SynthesizedPiece {
        spec: spec(xnotes::list_element_name("footnote").as_str()),
        body: vec![ValuePiece::Synthesized(&entry)],
        payload: None,
        ..pointer.clone()
    };
    copy_pieces_into_value(&[ValuePiece::Synthesized(&list)], &target, ValueContent::Own, cx.registry().lang().guard()).expect("built")
}

#[test]
fn a_pointer_naming_what_is_no_xnote_s_content_is_refused_where_it_is_lowered() {
    // A \textbf's argument is no xnote's content: shown there, it would be
    // shown twice.
    let registry = overridden(OverrideContent::Pointing("flm.textstyle", NodeRegion::Argument(0)));
    let run = run_workflow(&registry, text::WORKFLOW, &["A \\textbf{bold}\\footnote{f}"], &preferring_override());
    assert_eq!(identifiers(&run), ["flm.xnotes.pointer-refused"]);
    flm_core::dump::diag::expect(run.diagnostics().iter(), "flm.xnotes.pointer-refused").at("\\textbf{bold}").once();
    assert_eq!(printed(&run), "A bold\u{b9}\n\nFootnotes\n---------\n");
    // A footnote's own content region, pointed at by hand, is accepted and
    // labeled from the record found at its address.
    let registry = overridden(OverrideContent::Pointing(XNOTE, NodeRegion::Argument(1)));
    let run = run_workflow(&registry, text::WORKFLOW, &["A \\textbf{bold}\\footnote{f}"], &preferring_override());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "A bold\u{b9}\n\nFootnotes\n---------\n\n\u{b9} f\n");
    // Its labels region is no content of the xnote.
    let registry = overridden(OverrideContent::Pointing(XNOTE, NodeRegion::Argument(0)));
    let run = run_workflow(&registry, text::WORKFLOW, &["A \\footnote[\\label{fn:a}]{f}"], &preferring_override());
    assert_eq!(identifiers(&run), ["flm.xnotes.pointer-refused"]);
}

impl FlmFeature for OverrideList {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t-override", "Override of the footnotes' list")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.stage(Arc::new(OverrideBuilder(self.0)))
    }
}

/// The standard package, the default kind and an endnote kind, and the
/// override writing `content`.
fn overridden(content: OverrideContent) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(
        StdcoreConfig::new().xnotes(XnotesConfig::new().kind(numbered("endnote", "en"))),
    ));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(Arc::new(OverrideList(content)));
    builder.build(&RegistryInputs::new()).expect("the registry")
}

/// The overrides preferring the override's stage.
fn preferring_override() -> Overrides {
    Overrides { prefer: vec![OVERRIDE_STAGE.into()], ..Overrides::none() }
}

#[test]
fn an_override_preferred_by_the_request_replaces_one_kind_s_list_and_no_other() {
    let registry = overridden(OverrideContent::Counted("Footnotes written:"));
    let run = run_workflow(&registry, text::WORKFLOW, &["A\\footnote{f} B\\endnote{e}"], &preferring_override());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "A\u{b9} B\u{b9}\n\nFootnotes written: 1.\n\nEndnotes\n--------\n\n\u{b9} e\n");
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &preferring_override());
    assert!(scheduled(&plan, OVERRIDE_STAGE));
    assert!(!scheduled(&plan, xnotes::builder_name("footnote").as_str()));
    assert!(scheduled(&plan, xnotes::builder_name("endnote").as_str()));
}

#[test]
fn an_override_without_a_preference_is_a_contest_naming_both_builders() {
    let registry = overridden(OverrideContent::Counted("Written:"));
    let error = match common::stdcore::try_plan(&registry, text::WORKFLOW, &Overrides::none()) {
        Ok(_) => panic!("the plan must be contested"),
        Err(error) => error,
    };
    match &error {
        flm_core::core::plan::PlanError::Contested { capability, candidates, .. } => {
            assert_eq!(capability.as_str(), "flm.xnotes.list.footnote");
            let names: Vec<&str> = candidates.iter().map(|name| name.as_str()).collect();
            assert_eq!(names, ["flm.xnotes.build-list.footnote", OVERRIDE_STAGE]);
        }
        other => panic!("{other}"),
    }
}

#[test]
fn an_override_writing_another_feature_s_elements_or_nothing_neither_panics_nor_loops() {
    let registry = overridden(OverrideContent::Counted("\\begin{enumerate}\\item Notes:\\end{enumerate}"));
    let run = run_workflow(&registry, text::WORKFLOW, &["A\\footnote{f}"], &preferring_override());
    // The value's own list is the value's: its item, a point no stage of the
    // document saw, has no number and no identifier, and each table's missing
    // entry is reported once.
    assert_eq!(identifiers(&run), ["flm.run.missing-entry"; 2]);
    assert!(printed(&run).ends_with("A\u{b9}\n\nNotes:\n\n1.\n"), "{}", printed(&run));
    let registry = overridden(OverrideContent::Empty);
    let run = run_workflow(&registry, text::WORKFLOW, &["A\\footnote{f}"], &preferring_override());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(printed(&run), "A\u{b9}\n");
}

#[test]
fn the_list_is_written_without_the_references_or_the_numbering_feature() {
    let run = render_listed(&without_refs(), &["A\\footnote{x}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert!(printed(&run).ends_with("Footnotes\n---------\n\n\u{b9} x\n"), "{}", printed(&run));
    let lists = run.table(&pieces::lowered_pieces_table(&text::BACKEND.into())).expect("declared").expect("written");
    let list = payload_of::<ListPayload>(&nodes_of(&lists.pieces[0].tree.tree, pres::LIST)[0]).expect("a list").clone();
    assert_eq!(list.entry_anchors, [None]);
    let run = render_listed(&without_numbering(XnotesConfig::new()), &["A\\footnote{x}"]);
    assert_eq!(errors(&run), ["flm.xnotes.no-numbering"]);
    assert!(printed(&run).ends_with("Footnotes\n---------\n\nx\n"), "{}", printed(&run));
}

/// A test element whose xnote's content is a region its node does not have:
/// a fault in the program that answered, which the list meets where it
/// lowers the entry's pointer.
struct ForeignContent {
    kind: Arc<XnoteKind>,
}

impl XnoteSource for ForeignContent {
    fn xnotes(&self, _node: &NodeRef<'_, Flm>) -> Vec<XnoteOf> {
        vec![XnoteOf { kind: self.kind.name.clone(), content: NodeRegion::Argument(4) }]
    }
}

impl Referenceable for ForeignContent {
    fn points(&self, _node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        xnotes::ref_points(&self.kind, &[], None)
    }
}

/// The feature of `\tbad`: a contributed kind `bad` whose one element
/// answers [`ForeignContent`], written as nothing where it stands.
struct Foreign;

impl FlmFeature for Foreign {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t-foreign", "Foreign content")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let kind = Arc::new(numbered("bad", "bad").commands(Vec::<String>::new()));
        xnotes::contribute_kind(b, Arc::clone(&kind))?;
        let value = Arc::new(ForeignContent { kind });
        b.element(ElementDef::new(
            b.feature().clone(),
            ["tbad"],
            SyntaxDef::Macro { args: Vec::new() },
            Arc::new(
                ElementSpec::new("t.bad")
                    .with::<dyn XnoteSource>(Arc::clone(&value) as Arc<dyn XnoteSource>)
                    .with::<dyn Referenceable>(value as Arc<dyn Referenceable>),
            ),
        ))?;
        b.lowering_rule(LoweringRule::new(
            "t.bad.lower",
            "t.bad",
            LoweringDecl::new(),
            Arc::new(Nothing),
            DefaultMark::Everywhere,
        ))
    }
}

/// A rule writing nothing.
struct Nothing;

impl Lower for Nothing {
    fn lower(&self, _node: NodeRef<'_, Flm>, _cx: &mut LowerCx<'_>, _lw: &mut dyn Lowerer) -> Result<Lowering, LowerError> {
        Ok(Lowering::Emit(Vec::new()))
    }
}

#[test]
fn a_pointer_naming_a_region_its_node_does_not_have_is_reported_and_its_entry_is_empty() {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(Arc::new(Foreign));
    let registry = Arc::new(builder.build(&RegistryInputs::new()).expect("a registry"));
    let run = render_listed(&registry, &["A\\tbad B\\tbad C"]);
    assert_eq!(errors(&run), ["flm.lowering.region-not-found"; 2]);
    flm_core::dump::diag::expect(run.diagnostics().iter(), "flm.lowering.region-not-found").at("\\tbad ").count(2);
    assert!(printed(&run).ends_with("Bads\n----\n\n\u{b9}\n\n\u{b2}\n"), "{}", printed(&run));
}

/// A test element carrying xnotes of two kinds on one node: `\tboth{a}{b}`,
/// an xnote of the kind `gloss` whose content is `a`, and one of the kind
/// `source` whose content is `b`.
struct BothKinds {
    gloss: Arc<XnoteKind>,
    source: Arc<XnoteKind>,
}

impl XnoteSource for BothKinds {
    fn xnotes(&self, _node: &NodeRef<'_, Flm>) -> Vec<XnoteOf> {
        vec![
            XnoteOf { kind: self.gloss.name.clone(), content: NodeRegion::Argument(0) },
            XnoteOf { kind: self.source.name.clone(), content: NodeRegion::Argument(1) },
        ]
    }
}

impl Referenceable for BothKinds {
    fn points(&self, _node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        [xnotes::ref_points(&self.gloss, &[], None), xnotes::ref_points(&self.source, &[], None)].concat()
    }
}

/// The rule of `\tboth`: the mark of each kind, in the order of the kinds.
struct BothMarks(Arc<BothKinds>);

impl Lower for BothMarks {
    fn lower(&self, node: NodeRef<'_, Flm>, cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer) -> Result<Lowering, LowerError> {
        let mut marks = xnotes::stage_mark(cx, lw, &node, &self.0.gloss, 0)?;
        marks.extend(xnotes::stage_mark(cx, lw, &node, &self.0.source, 0)?);
        Ok(Lowering::Emit(marks))
    }
}

/// The feature of `\tboth`: two contributed kinds, `gloss` then `source`,
/// and the element carrying one xnote of each.
struct TwoKindsOnOneNode;

impl FlmFeature for TwoKindsOnOneNode {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t-two-kinds", "Two kinds on one node")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let gloss = Arc::new(numbered("gloss", "gl").commands(Vec::<String>::new()));
        let source = Arc::new(numbered("source", "src").commands(Vec::<String>::new()));
        xnotes::contribute_kind(b, Arc::clone(&gloss))?;
        xnotes::contribute_kind(b, Arc::clone(&source))?;
        let value = Arc::new(BothKinds { gloss, source });
        b.element(ElementDef::new(
            b.feature().clone(),
            ["tboth"],
            SyntaxDef::Macro { args: vec![flm_core::lang::mint::ArgDef::mandatory("a"), flm_core::lang::mint::ArgDef::mandatory("b")] },
            Arc::new(
                ElementSpec::new("t.both")
                    .with::<dyn XnoteSource>(Arc::clone(&value) as Arc<dyn XnoteSource>)
                    .with::<dyn Referenceable>(Arc::clone(&value) as Arc<dyn Referenceable>),
            ),
        ))?;
        b.lowering_rule(LoweringRule::new(
            "t.both.lower",
            "t.both",
            LoweringDecl::new()
                .reads(XNOTES)
                .reads_opt(COUNTERS)
                .reads_opt(ANCHORS)
                .produces(pres::SPAN)
                .produces(pres::LINK)
                .produces(pres::BLOCK)
                .produces(pres::PAR),
            Arc::new(BothMarks(value)),
            DefaultMark::Everywhere,
        ))
    }
}

#[test]
fn a_node_carrying_xnotes_of_two_kinds_is_marked_and_listed_in_each_kind_s_list() {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(Arc::new(TwoKindsOnOneNode));
    let registry = Arc::new(builder.build(&RegistryInputs::new()).expect("a registry"));
    let run = render_listed(&registry, &["A\\tboth{one}{two} B\\tboth{three}{four}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        printed(&run),
        "A\u{b9}\u{b9} B\u{b2}\u{b2}\n\nGlosss\n------\n\n\u{b9} one\n\n\u{b2} three\n\n\
         Sources\n-------\n\n\u{b9} two\n\n\u{b2} four\n"
    );
    let lists = run.table(&pieces::lowered_pieces_table(&text::BACKEND.into())).expect("declared").expect("written");
    for (kind, anchors) in [("gloss", ["gl-1", "gl-2"]), ("source", ["src-1", "src-2"])] {
        let piece = lists.pieces.iter().find(|p| p.name.as_str() == format!("flm.xnotes.list.{kind}")).expect("placed");
        let list = payload_of::<ListPayload>(&nodes_of(&piece.tree.tree, pres::LIST)[0]).expect("a list").clone();
        assert_eq!(list.entry_anchors, anchors.map(|a| Some(a.to_string())), "{kind}");
    }
}

#[test]
fn the_projection_of_a_kind_s_list_prints_its_tree_and_each_pointer() {
    let run = render_listed(&std(), &["A\\footnote{x} B"]);
    let projected = flm_core::dump::json::to_json(&projection(&run, xnotes::piece_table("footnote")))
        .expect("json");
    assert!(projected.contains("flm.xnotes.content-pointer"), "{projected}");
    assert!(projected.contains("\"region\":\"argument 1\""), "{projected}");
    assert!(projected.contains("\"target\":\"tree#0/"), "{projected}");
}

// ---------------------------------------------------------------------------------------------
// an xnote in content shown again: its mark (tests 1–4, 10 and 13 of the copies of shown
// content)
// ---------------------------------------------------------------------------------------------

/// The marks of a run's first lowered tree, in document order, as whether each
/// carries an identifier.
fn marks_anchored(run: &Run) -> Vec<bool> {
    spans_of_role(run, xnotes::MARK_ROLE).iter().map(|span| span.anchor.is_some()).collect()
}

#[test]
fn a_reference_to_a_heading_holding_an_xnote_shows_the_title_with_the_note_s_mark() {
    let document = "\\section*{Intro\\footnote{N.}}\\label{sec:a}\n\nSee \\ref{sec:a}.";
    let run = render_listed(&std(), &[document]);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    // The mark where the note stands and the mark the reference shows, the
    // second with no identifier; the note listed once.
    assert_eq!(
        printed(&run),
        "Intro\u{b9}\n======\n\nSee Intro\u{b9}.\n\nFootnotes\n---------\n\n\u{b9} N.\n"
    );
    assert_eq!(marks_anchored(&run), [true, false]);
    assert_eq!(count(&run, "footnote"), 1);
}

#[test]
fn a_labeled_xnote_in_a_title_shown_again_renders_and_its_label_resolves_to_the_note() {
    let document = "\\section*{Intro\\footnote[\\label{fn:a}]{N.}}\\label{sec:a}\n\n\
        See \\ref{sec:a} and \\ref{fn:a}.";
    let run = render_with(&std(), document);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(printed(&run), "Intro\u{b9}\n======\n\nSee Intro\u{b9} and Footnote\u{a0}1.\n");
    assert_eq!(count(&run, "footnote"), 1);
}

#[test]
fn two_references_show_two_marks_of_one_note_for_each_place_that_shows_content() {
    for (document, shown) in [
        ("\\section*{Intro\\footnote{N.}}\\label{sec:a}\n\nSee \\ref{sec:a} and \\ref{sec:a}.", "Intro"),
        (
            "\\begin{figure}\\nonumber\\includegraphics{a.png}\\caption{A photo\\footnote{N.}}\\label{figure:a}\
             \\end{figure}\n\nSee \\ref{figure:a} and \\ref{figure:a}.",
            "A photo",
        ),
        (
            "\\begin{defterm}{Entropy\\footnote{N.}}\\label{topic:entropy}\nA measure.\n\\end{defterm}\n\n\
             See \\ref{topic:entropy} and \\ref{topic:entropy}.",
            "Entropy",
        ),
        (
            "\\begin{enumerate}\\item[Tag\\footnote{N.}]\\label{item:a} One.\\end{enumerate}\n\n\
             See \\ref{item:a} and \\ref{item:a}.",
            "Tag",
        ),
    ] {
        let run = render_listed(&std(), &[document]);
        assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new(), "{document}\n{}", run.diagnostics().render_all());
        let out = printed(&run);
        assert!(out.contains(&format!("See {shown}\u{b9} and {shown}\u{b9}.")), "{out}");
        assert_eq!(marks_anchored(&run), [true, false, false], "{out}");
        assert_eq!(count(&run, "footnote"), 1, "{out}");
        assert_eq!(out.matches("Footnotes").count(), 1, "{out}");
    }
}

/// A float's alternative text and a list item's tag, each read by the
/// key-value reader of its bracket and holding a note, each shown by a
/// reference.
const SHOWN_THROUGH_KEY_VALUES: &str = "\\begin{figure}\\nonumber\\includegraphics{a.png}\\caption[alt=B\\footnote{Q.}]{B}\
    \\label{figure:b}\\end{figure}\n\n\\begin{enumerate}\\item[tag=T\\footnote{x}]\\label{item:a} One.\\end{enumerate}\n\n\
    See \\ref{figure:b} and \\ref{item:a}.";

#[test]
fn content_read_by_the_key_value_reader_shown_again_shows_the_note_s_mark() {
    let run = render_listed(&std(), &[SHOWN_THROUGH_KEY_VALUES]);
    assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let out = printed(&run);
    assert!(out.contains("See B\u{b9} and T\u{b2}."), "{out}");
    // Each note listed once. The tag's mark where it stands is anchored, the
    // marks the references show are not; the alternative text is not written
    // where it stands, so its note's mark is the one the reference shows.
    assert_eq!(count(&run, "footnote"), 2, "{out}");
    assert_eq!(marks_anchored(&run), [true, false, false], "{out}");
}

#[test]
fn a_reference_to_a_heading_holding_an_in_place_xnote_shows_its_mark_and_not_the_note() {
    let run = render_with(&in_place(), "\\section*{Intro\\footnote{N.}}\\label{sec:a}\n\nSee \\ref{sec:a}.");
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    // The note stands in the title where it was written; the reference shows
    // the mark alone.
    assert_eq!(printed(&run), "Intro[\u{b9}N.]\n==========\n\nSee Intro\u{b9}.\n");
    assert_eq!(spans_of_role(&run, xnotes::INPLACE_ROLE).len(), 1);
    assert_eq!(marks_anchored(&run), [true, false]);
}

#[test]
fn the_mark_of_a_copied_xnote_is_registered_with_a_rule_on_every_backend_and_never_parsed() {
    let registry = std();
    assert_eq!(registry.elements_named(xnotes::MARK_OF).len(), 1);
    let rules: Vec<(&str, bool)> = registry
        .rules_for(xnotes::MARK_OF)
        .iter()
        .map(|idx| {
            let entry = registry.rule(*idx);
            (entry.rule.name.as_str(), entry.rule.default.is_general())
        })
        .collect();
    assert_eq!(
        rules,
        [(xnotes::MARK_OF_RULE, true), (xnotes::latex::MARK_OF_LATEX_RULE, false)]
    );
    let latex = registry.rule(registry.rules_for(xnotes::MARK_OF)[1]);
    assert!(latex.rule.default.is_specific_to(&"latex-recomposed".into()));
    // Without the feature, no such element and no rule.
    let without = registry_over(package_features_but(StdcoreConfig::new(), xnotes::FEATURE));
    assert!(without.elements_named(xnotes::MARK_OF).is_empty());
    assert!(without.rules_for(xnotes::MARK_OF).is_empty());
    // A document holds none of it: only a copy shown again does.
    let run = render_with(&registry, "A\\footnote{a}\\section*{T\\footnote{b}}\\label{sec:t} \\ref{sec:t}");
    assert!(nodes_of(tree(&run), xnotes::MARK_OF).is_empty());
}

#[test]
fn the_payload_of_the_mark_of_a_copied_xnote_names_its_kinds_and_where_it_was_written() {
    let run = render_with(&std(), "\\section*{Intro\\footnote{N.}}\\label{sec:a}\n\nSee \\ref{sec:a}.");
    let targets = run.table(refs::REF_TARGETS).expect("declared").expect("written");
    let SerialValue::Map(fields) = targets
        .by_label
        .values()
        .find_map(|target| target.display.content.as_ref())
        .and_then(|value| {
            let node = value.root().descendants().find(|node| node.ext().element.name.as_str() == xnotes::MARK_OF);
            node.and_then(|node| node.ext().payload.as_deref().map(|payload| payload.project()))
        })
        .expect("a mark in the copy")
    else {
        panic!("a map")
    };
    assert_eq!(fields[0], ("kinds".to_string(), SerialValue::List(vec![SerialValue::Str("footnote".to_string())])));
    assert_eq!(fields[1], ("original".to_string(), SerialValue::Str("@15-28".to_string())));
}

#[test]
fn content_shown_again_holding_xnotes_neither_panics_nor_loops() {
    // A heading whose title holds a note holding a reference to the heading.
    let own = render_with(&std(), "\\section*{Intro\\footnote{See \\ref{sec:a}.}}\\label{sec:a}\n\nSee \\ref{sec:a}.");
    assert_eq!(identifiers(&own), Vec::<String>::new(), "{}", own.diagnostics().render_all());
    assert_eq!(marks_anchored(&own), [true, false]);
    // A note inside a note inside a title shown again: one mark, the outer
    // note's.
    let nested = render_listed(&std(), &["\\section*{Intro\\footnote{a\\footnote{b}}}\\label{sec:a}\n\nSee \\ref{sec:a}."]);
    assert_eq!(identifiers(&nested), Vec::<String>::new(), "{}", nested.diagnostics().render_all());
    assert!(printed(&nested).contains("See Intro\u{b9}."), "{}", printed(&nested));
    assert_eq!(count(&nested, "footnote"), 2);
    // Titles shown ten references deep, each holding a note.
    let mut document = String::from("\\section*{T0\\footnote{n0}}\\label{sec:t0}\n\n");
    for level in 1..=10 {
        document.push_str(&format!(
            "\\section*{{T{level} \\ref{{sec:t{}}}\\footnote{{n{level}}}}}\\label{{sec:t{level}}}\n\n",
            level - 1
        ));
    }
    document.push_str("See \\ref{sec:t10}.");
    for registry in [std(), in_place()] {
        let run = render_with(&registry, &document);
        assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
        assert!(printed(&run).contains("See T10 T9 T8"), "{}", printed(&run));
    }
    // A title copied ten definitions deep: ten nested user definitions, each
    // adding a note, in a starred title shown by a reference. Eleven notes,
    // each shown again as its mark.
    let run = render_with(&with_nested_definitions(), NESTED_DEFINITIONS_TITLE);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(count(&run, "footnote"), 11);
    assert_eq!(marks_anchored(&run), [vec![true; 11], vec![false; 11]].concat());
}

/// Ten user definitions `\nesta` … `\nestj`, each adding a note to the
/// one before it.
fn nested_definitions() -> Vec<Definition> {
    let letters: Vec<char> = "abcdefghij".chars().collect();
    letters
        .iter()
        .enumerate()
        .map(|(level, letter)| {
            let template = match level {
                0 => String::from("x\\footnote{n0}"),
                _ => format!("\\nest{} x\\footnote{{n{level}}}", letters[level - 1]),
            };
            Definition::macro_(format!("nest{letter}"), Vec::new()).text(template)
        })
        .collect()
}

/// The standard package with the ten [`nested_definitions`].
fn with_nested_definitions() -> Arc<FlmRegistry> {
    registry_with(StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(nested_definitions())), XnotesConfig::new())
}

/// A starred title holding a note and the outermost of the ten
/// [`nested_definitions`], shown by a reference.
const NESTED_DEFINITIONS_TITLE: &str = "\\section*{T\\footnote{t} \\nestj}\\label{sec:a}\n\nSee \\ref{sec:a}.";

#[test]
fn an_xnote_in_a_list_s_own_label_template_is_a_missing_entry_at_each_item() {
    // A list's own label template is content the run holds as its own,
    // lowered at each item: its xnote is not the document's, the table of
    // xnotes records none for it, and each item reports the missing entry.
    let document = "\\begin{itemize}[label={*\\footnote{x}}]\\item a\\item b\\end{itemize}";
    for workflow in [text::WORKFLOW, flm_core::stdcore::backends::html::WORKFLOW] {
        let run = run_workflow(&std(), workflow, &[document], &Overrides::none());
        assert_eq!(
            reports(&run),
            vec![(String::from("flm.run.missing-entry"), Severity::Error); 2],
            "{workflow}: {}",
            run.diagnostics().render_all()
        );
    }
}
