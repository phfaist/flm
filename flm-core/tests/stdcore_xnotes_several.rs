//! Several xnotes on one node: the element `\ttwonotes{first}{second}` of the
//! test module `common::xnotes`, one node carrying two xnotes of one kind,
//! written through the same helpers as `\footnote` — numbered in order,
//! marked side by side, listed each with its content, written as two
//! commands on the recomposed LaTeX, standing for both in content shown again,
//! shown in place under an in-place kind; a node whose element answers
//! its xnotes and their contents in different numbers; and one whose element
//! names one region for two xnotes.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Arc;

use flm_core::core::lowering::{
    DefaultMark, Lower, LowerCx, LowerError, Lowerer, Lowering, LoweringDecl, LoweringRule, NodeRegion,
};
use flm_core::core::plan::Overrides;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry, RegistryBuilder, RegistryInputs,
};
use flm_core::core::run::Run;
use flm_core::lang::Flm;
use flm_core::lang::mint::{ArgDef, ElementDef, SyntaxDef};
use flm_core::model::aspects::{RefPoint, Referenceable};
use flm_core::model::element::ElementSpec;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::xnotes::{
    self, XNOTES, XnoteKind, XnoteOf, XnotePlacement, XnoteSource, XnotesConfig,
};
use flm_core::stdcore::{StdcoreConfig, package};
use techy::core::node::NodeRef;
use techy::error::Severity;

use common::lowering::text as printed;
use common::stdcore::{identifiers, reports, run_workflow};
use common::xnotes::two_notes_registry;

/// The standard package, `footnote` listed, and `\ttwonotes` of footnotes.
fn listed() -> Arc<FlmRegistry> {
    two_notes_registry(XnotesConfig::new(), "footnote")
}

/// The standard package, `footnote` shown in place, and `\ttwonotes` of
/// footnotes.
fn in_place() -> Arc<FlmRegistry> {
    two_notes_registry(
        XnotesConfig { kinds: vec![XnoteKind::footnote().placement(XnotePlacement::InPlace)] },
        "footnote",
    )
}

/// The run of `texts` through `workflow` under `registry`.
fn run(registry: &Arc<FlmRegistry>, workflow: &str, texts: &[&str]) -> Run {
    run_workflow(registry, workflow, texts, &Overrides::none())
}

/// The run of `texts` through `workflow` under `registry`, which must report
/// nothing.
fn clean(registry: &Arc<FlmRegistry>, workflow: &str, texts: &[&str]) -> Run {
    let run = run(registry, workflow, texts);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    run
}

/// The part of the page inside `<main …>` and `</main>`.
fn main_of(run: &Run) -> String {
    let page = common::html::page(run);
    let start = page.find("<main").expect("a <main>");
    let start = start + page[start..].find('>').expect("the tag closes") + 1;
    let end = page.find("</main>").expect("a </main>");
    page[start..end].to_string()
}

const TWO: &str = "A\\ttwonotes{one}{two} B\\footnote{three} C";

// ---------------------------------------------------------------------------------------------
// the listed kind: numbers, marks, list (test 21)
// ---------------------------------------------------------------------------------------------

#[test]
fn two_notes_on_one_node_are_numbered_in_order_marked_side_by_side_and_listed() {
    let run = clean(&listed(), text::WORKFLOW, &[TWO]);
    assert_eq!(
        printed(&run),
        "A\u{b9}\u{b2} B\u{b3} C\n\nFootnotes\n---------\n\n\u{b9} one\n\n\u{b2} two\n\n\u{b3} three\n"
    );
    let table = run.table(XNOTES).expect("declared").expect("written");
    let records = table.of_kind("footnote");
    assert_eq!(records.len(), 3);
    // The first two are the one node's, each paired with its own argument.
    assert_eq!(records[0].point.addr, records[1].point.addr);
    assert_eq!(table.records_at("footnote", records[0].point.addr).count(), 2);
    assert_eq!(records[0].content, Some(NodeRegion::Argument(0)));
    assert_eq!(records[1].content, Some(NodeRegion::Argument(1)));
}

#[test]
fn on_the_page_each_mark_links_to_its_own_entry_and_back() {
    let run = clean(&listed(), html::WORKFLOW, &[TWO]);
    let main = main_of(&run);
    for n in 1..=3 {
        assert_eq!(main.matches(&format!("href=\"#fn-{n}\"")).count(), 1, "{main}");
    }
    let entry = |n: usize| {
        let entry = &main[main.find(&format!("<li id=\"fn-{n}\">")).expect("the entry")..];
        entry[..entry.find("</li>").expect("closed")].to_string()
    };
    assert!(entry(1).contains("one"), "{}", entry(1));
    assert!(entry(2).contains("two"), "{}", entry(2));
    // Each entry links back to its own mark: two distinct identifiers on
    // the one node.
    let back = |entry: &str| {
        let at = entry.find("href=\"#fnref--").expect("a link back") + "href=\"#".len();
        entry[at..at + entry[at..].find('"').expect("closed")].to_string()
    };
    assert_ne!(back(&entry(1)), back(&entry(2)));
    for n in [1, 2] {
        assert_eq!(main.matches(&format!("id=\"{}\"", back(&entry(n)))).count(), 1, "{main}");
    }
}

#[test]
fn on_the_recomposed_latex_each_note_is_a_command_of_its_own() {
    let run = clean(&listed(), latex_recomposed::WORKFLOW, &[TWO]);
    let body = common::latex::output(&run);
    assert!(
        body.contains("A\\flmXNfootnote{one}\\flmXNfootnote{two} B\\flmXNfootnote{three} C"),
        "{body}"
    );
}

// ---------------------------------------------------------------------------------------------
// content shown again: the stand-in stands for both notes
// ---------------------------------------------------------------------------------------------

const SHOWN: &str = "\\section*{Intro\\ttwonotes{a}{b}}\\label{sec:a}\n\nSee \\ref{sec:a}.";

#[test]
fn a_reference_showing_a_title_holding_the_node_shows_both_marks() {
    let run = clean(&listed(), text::WORKFLOW, &[SHOWN]);
    let out = printed(&run);
    assert!(out.contains("Intro\u{b9}\u{b2}"), "{out}");
    assert!(out.contains("See Intro\u{b9}\u{b2}."), "{out}");
    assert!(out.ends_with("\u{b9} a\n\n\u{b2} b\n"), "{out}");
    // On the page: the two marks again inside the reference, linked to
    // their notes, with no identifier of their own.
    let run = clean(&listed(), html::WORKFLOW, &[SHOWN]);
    let main = main_of(&run);
    for n in [1, 2] {
        assert!(main.matches(&format!("href=\"#fn-{n}\"")).count() >= 1, "{main}");
    }
    assert_eq!(main.matches("flm-xnotes-xnote-mark").count(), 4, "{main}");
    // On the recomposed LaTeX the reference is LaTeX's, the title the
    // heading's: the notes are written once, where they stand.
    let run = clean(&listed(), latex_recomposed::WORKFLOW, &[SHOWN]);
    let body = common::latex::output(&run);
    assert_eq!(body.matches("\\flmXNfootnote{").count(), 2, "{body}");
}

// ---------------------------------------------------------------------------------------------
// an in-place kind
// ---------------------------------------------------------------------------------------------

#[test]
fn under_an_in_place_kind_each_note_is_shown_in_place_with_its_own_mark() {
    let run = clean(&in_place(), text::WORKFLOW, &["A\\ttwonotes{one}{two} B"]);
    let out = printed(&run);
    assert_eq!(out, "A[\u{b9}one][\u{b2}two] B\n", "{out}");
    let run = clean(&in_place(), html::WORKFLOW, &["A\\ttwonotes{one}{two} B"]);
    let main = main_of(&run);
    assert_eq!(main.matches("flm-xnotes-xnote-inplace").count(), 2, "{main}");
    assert!(main.contains("id=\"fn-1\""), "{main}");
    assert!(main.contains("id=\"fn-2\""), "{main}");
    assert!(!main.contains('['), "the page's stylesheet draws the brackets: {main}");
    let run = clean(&in_place(), latex_recomposed::WORKFLOW, &["A\\ttwonotes{one}{two} B"]);
    let body = common::latex::output(&run);
    assert!(body.contains("A\\flmXNfootnote{one}\\flmXNfootnote{two} B"), "{body}");
}

// ---------------------------------------------------------------------------------------------
// diagnostics per note
// ---------------------------------------------------------------------------------------------

#[test]
fn a_diagnostic_in_the_second_note_s_content_is_reported_there() {
    for workflow in [text::WORKFLOW, html::WORKFLOW] {
        let run = run(&listed(), workflow, &["A\\ttwonotes{one}{see \\ref{nowhere}}"]);
        let ids = identifiers(&run);
        assert_eq!(ids.len(), 1, "{workflow}: {}", run.diagnostics().render_all());
        flm_core::dump::diag::expect(run.diagnostics().iter(), &ids[0]).at("\\ref{nowhere}").once();
    }
}

#[test]
fn an_empty_second_note_is_reported_once() {
    let run = run(&listed(), text::WORKFLOW, &["A\\ttwonotes{one}{}"]);
    assert_eq!(reports(&run), [("flm.xnotes.empty".to_string(), Severity::Warning)]);
    assert!(printed(&run).starts_with("A\u{b9}\u{b2}\n"), "{}", printed(&run));
}

// ---------------------------------------------------------------------------------------------
// an element answering its xnotes and their contents in different numbers
// ---------------------------------------------------------------------------------------------

/// `\tunpaired{a}{b}`: two contents of the kind `footnote` answered, and one
/// pair of points only.
struct Unpaired(Arc<XnoteKind>);

impl XnoteSource for Unpaired {
    fn xnotes(&self, node: &NodeRef<'_, Flm>) -> Vec<XnoteOf> {
        ["a", "b"]
            .iter()
            .filter_map(|arg| NodeRegion::argument_named(node, arg))
            .map(|content| XnoteOf { kind: self.0.name.clone(), content })
            .collect()
    }
}

impl Referenceable for Unpaired {
    fn points(&self, _node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        xnotes::ref_points(&self.0, &[], None)
    }
}

/// The rule of `\tunpaired`: the marks of both contents it answers.
struct UnpairedRule(Arc<XnoteKind>);

impl Lower for UnpairedRule {
    fn lower(&self, node: NodeRef<'_, Flm>, cx: &mut LowerCx<'_>, lw: &mut dyn Lowerer) -> Result<Lowering, LowerError> {
        let mut marks = xnotes::stage_mark(cx, lw, &node, &self.0, 0)?;
        marks.extend(xnotes::stage_mark(cx, lw, &node, &self.0, 1)?);
        Ok(Lowering::Emit(marks))
    }
}

/// The feature of `\tunpaired`.
struct UnpairedFeature;

impl FlmFeature for UnpairedFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t-unpaired", "Unpaired")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let kind = Arc::new(XnoteKind::footnote());
        let value = Arc::new(Unpaired(Arc::clone(&kind)));
        b.element(ElementDef::new(
            b.feature().clone(),
            ["tunpaired"],
            SyntaxDef::Macro { args: vec![ArgDef::mandatory("a"), ArgDef::mandatory("b")] },
            Arc::new(
                ElementSpec::new("t.unpaired")
                    .with::<dyn XnoteSource>(Arc::clone(&value) as Arc<dyn XnoteSource>)
                    .with::<dyn Referenceable>(value as Arc<dyn Referenceable>),
            ),
        ))?;
        b.lowering_rule(LoweringRule::new(
            "t.unpaired.lower",
            "t.unpaired",
            LoweringDecl::new()
                .reads(XNOTES)
                .reads_opt(flm_core::stdcore::features::numbering::COUNTERS)
                .reads_opt(flm_core::stdcore::features::refs::ANCHORS)
                .produces(flm_core::stdcore::backends::pres::SPAN)
                .produces(flm_core::stdcore::backends::pres::LINK),
            Arc::new(UnpairedRule(kind)),
            DefaultMark::Everywhere,
        ))
    }
}

#[test]
fn a_node_answering_more_contents_than_xnotes_is_reported_and_its_extra_content_is_written_by_no_one() {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(Arc::new(UnpairedFeature));
    let registry = builder.build(&RegistryInputs::new()).expect("a registry");
    let run = run(&registry, text::WORKFLOW, &["A\\tunpaired{x}{y} B"]);
    assert_eq!(
        reports(&run),
        [
            ("flm.xnotes.unpaired".to_string(), Severity::Error),
            ("flm.run.missing-entry".to_string(), Severity::Error),
        ]
    );
    let unpaired = flm_core::dump::diag::expect(run.diagnostics().iter(), "flm.xnotes.unpaired");
    unpaired.at("\\tunpaired{x}{y}").once();
    let message = run.diagnostics().iter().find(|d| d.identifier() == "flm.xnotes.unpaired").unwrap().to_string();
    assert!(message.contains("must give the content of each note of the kind"), "{message}");
    // The one xnote is the first content, listed.
    assert!(printed(&run).ends_with("\u{b9} x\n"), "{}", printed(&run));
}

// ---------------------------------------------------------------------------------------------
// an element answering one region for two of its xnotes
// ---------------------------------------------------------------------------------------------

/// `\tsame{a}{b}`: two xnotes of the kind `footnote`, both answered with the
/// region `a` — against what `XnoteSource` states, each `XnoteOf` of a node
/// naming a region of its own.
struct SameRegion(Arc<XnoteKind>);

impl XnoteSource for SameRegion {
    fn xnotes(&self, node: &NodeRef<'_, Flm>) -> Vec<XnoteOf> {
        ["a", "a"]
            .iter()
            .filter_map(|arg| NodeRegion::argument_named(node, arg))
            .map(|content| XnoteOf { kind: self.0.name.clone(), content })
            .collect()
    }
}

impl Referenceable for SameRegion {
    fn points(&self, _node: &NodeRef<'_, Flm>) -> Vec<RefPoint> {
        let mut points = xnotes::ref_points(&self.0, &[], None);
        points.extend(xnotes::ref_points(&self.0, &[], None));
        points
    }
}

/// The feature of `\tsame`, its rule the one of `\tunpaired`: the marks of
/// both xnotes.
struct SameRegionFeature;

impl FlmFeature for SameRegionFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("t-same", "Same region")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let kind = Arc::new(XnoteKind::footnote());
        let value = Arc::new(SameRegion(Arc::clone(&kind)));
        b.element(ElementDef::new(
            b.feature().clone(),
            ["tsame"],
            SyntaxDef::Macro { args: vec![ArgDef::mandatory("a"), ArgDef::mandatory("b")] },
            Arc::new(
                ElementSpec::new("t.same")
                    .with::<dyn XnoteSource>(Arc::clone(&value) as Arc<dyn XnoteSource>)
                    .with::<dyn Referenceable>(value as Arc<dyn Referenceable>),
            ),
        ))?;
        b.lowering_rule(LoweringRule::new(
            "t.same.lower",
            "t.same",
            LoweringDecl::new()
                .reads(XNOTES)
                .reads_opt(flm_core::stdcore::features::numbering::COUNTERS)
                .reads_opt(flm_core::stdcore::features::refs::ANCHORS)
                .produces(flm_core::stdcore::backends::pres::SPAN)
                .produces(flm_core::stdcore::backends::pres::LINK),
            Arc::new(UnpairedRule(kind)),
            DefaultMark::Everywhere,
        ))
    }
}

#[test]
fn an_element_naming_one_region_for_two_xnotes_is_rendered_without_failing() {
    // Outside the contract `XnoteSource` states: the list finds an entry's
    // xnote by its node and region, so both entries find the first xnote.
    // What is pinned here is that the document is rendered all the same.
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(Arc::new(SameRegionFeature));
    let registry = builder.build(&RegistryInputs::new()).expect("a registry");
    let run = run(&registry, text::WORKFLOW, &["A\\tsame{x}{y} B"]);
    assert!(printed(&run).starts_with("A\u{b9}\u{b2} B\n"), "{}", printed(&run));
    assert!(printed(&run).contains("\u{b9} x"), "{}", printed(&run));
    let run = run_workflow(&registry, html::WORKFLOW, &["A\\tsame{x}{y} B"], &Overrides::none());
    assert!(main_of(&run).contains("id=\"fn-1\""), "{}", main_of(&run));
}
