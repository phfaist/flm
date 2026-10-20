//! The whole pipeline through the `html` workflow, over a registry built from
//! `flm_core::stdcore::package` alone: the page of the document exercising
//! every standard feature and of the session-06 document as goldens, the
//! former's lowered projection, its per-tree output table and its definitions
//! table as goldens, one document served by three backends — `text` and `html`
//! in one plan, `latex-recomposed` in a plan of its own — the same bytes under
//! both execution paths, and the source-span attribute the render option
//! `source_spans` adds. The text-workflow counterpart is `spine_text.rs` and
//! the LaTeX one `spine_latex.rs`; the backend's own suites, one property per
//! golden, are `html_emitter.rs`, `html_page.rs`, `html_pipeline.rs`,
//! `html_math.rs` and `html_features.rs`.
//!
//! **Why the page goldens carry no stylesheet.** A style definition reaches a
//! page either as its own text, written into the page, or as an address the
//! page loads; which of the two is the host's decision, taken through the
//! render option `definitions`. These goldens take the second for every
//! stylesheet this backend ships, so that each page shows *which* chunks the
//! document required without burying that under several hundred lines of
//! unchanging rules. The bytes of the shipped chunks are pinned once, by
//! `definitions.page.html` in `html_page.rs`. The formula renderer's script is
//! left as it is declared, so that a page here shows the delivery a document
//! gets with no option set: its configuration written into the page, its
//! program loaded from an address.
//!
//! Every golden here is parsed by the validity check (`html_validity.rs`),
//! which discovers the HTML goldens of this directory by their extension.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan, PlanRequest};
use flm_core::core::registry::DefContent;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::golden::{assert_json_golden, assert_text_golden};
use flm_core::dump::project::{ProjectOptions, project_tree};
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::model::addr::TreeId;
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::names::{BackendName, Name};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::html::{
    self, DEFINITIONS_TABLE, DefinitionOverride, HtmlRenderOptions, OUTPUT_TABLE,
};
use flm_core::stdcore::backends::latex_recomposed;
use flm_core::stdcore::backends::lowering::lowered_table;
use flm_core::stdcore::backends::text::{TEXT_DOCUMENT, TextRenderOptions};
use flm_core::stdcore::features::refs::{RefsRenderOptions, UnresolvedRefs};
use techy::serialize::SerialValue;

use common::html::{assert_golden_at, fragments, page as rendered_page};
use common::latex::golden_text;
use common::lowering::text as text_of;
use common::pipeline;
use common::spine::{
    FEATURES_DOCUMENT, SESSION06_DOCUMENT, golden_path, registry, tables_projection,
};
use common::stdcore::{identifiers, plan, projection, run_workflow};
use common::wiring::count_element;

// --- the render request these goldens are written under -----------------------------------

/// The render options the page goldens are written under: every stylesheet
/// this backend ships delivered by an address, the unresolved reference the
/// features document carries on purpose tolerated as a warning, and the text
/// width the text goldens of `spine_text.rs` use, so that one request serves
/// all three backends in the three-backend test below.
///
/// The `definitions` map is built from the stylesheets the backend registers
/// rather than from a list written here, so that a stylesheet added later is
/// delivered by address too and no golden quietly gains a few hundred lines.
fn spine_overrides() -> Overrides {
    Overrides { render_options: spine_render_options(), ..Overrides::none() }
}

/// The option values behind [`spine_overrides`], for the one test that builds
/// a [`PlanRequest`] of its own instead of asking for a workflow.
fn spine_render_options() -> OptionValues {
    let mut definitions = BTreeMap::new();
    for style in html::defs::stylesheets() {
        let id = style.id.to_string();
        let stem = id.rsplit('.').next().expect("an identifier has a last part");
        definitions.insert(
            Name::from(id.as_str()),
            DefinitionOverride::Parts(vec![DefContent::Url(format!("css/{stem}.css"))]),
        );
    }
    OptionValues::new()
        .with(HtmlRenderOptions { definitions, ..HtmlRenderOptions::default() })
        .with(RefsRenderOptions { unresolved: UnresolvedRefs::Allow })
        .with(TextRenderOptions { width: Some(80), ..TextRenderOptions::default() })
}

/// The features document, or the session-06 document, through the `html`
/// workflow over the spine's registry, sequentially, under
/// [`spine_overrides`].
fn rendered(document: &str) -> Run {
    run_workflow(&registry(), html::WORKFLOW, &[document], &spine_overrides())
}

/// The projection of the first tree's lowered form for the HTML backend.
fn lowered_projection(run: &Run) -> SerialValue {
    let lowered = run
        .table(lowered_table(&BackendName::from(html::BACKEND)))
        .expect("the lowered table is declared")
        .and_then(|trees| trees.get(TreeId::tree(0)))
        .unwrap_or_else(|| {
            panic!("the tree was not lowered; diagnostics:\n{}", run.diagnostics().render_all())
        });
    project_tree(&lowered.tree, &ProjectOptions::new())
}

// --- the plan ---------------------------------------------------------------------------------

#[test]
fn the_workflow_plans_the_structured_tree_the_anchors_and_the_backends_four_stages() {
    // The spine's registry holds two per-plan user definitions and one
    // `always` definition, so both expansion rewrites run, and the block
    // structure is built because this backend renders paragraphs.
    let plan = plan(&registry(), html::WORKFLOW, &spine_overrides());
    let names = |names: &[flm_core::model::names::StageName]| {
        names.iter().map(|name| name.to_string()).collect::<Vec<_>>()
    };
    assert_eq!(
        names(&plan.rewrite_names()),
        ["flm.input.splice", "flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]
    );
    // The workflow demands `flm.validated` beside its output, so the three
    // checks and the stages they read are planned with the backend's own. The
    // backend claims no element the `refs` feature owns, so that feature's
    // rule is selected, its read of the resolutions schedules the resolve
    // stage and, behind it, the stage that computes the anchors — which is
    // where a heading's identifier comes from.
    assert_eq!(
        names(&plan.stage_names()),
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
            "flm.graphics.publish.html",
            "flm.floats.check-handlers",
            "flm.citations.collect",
            "flm.citations.retrieve",
            "flm.citations.check",
            "flm.citations.build-bibliography",
            "flm.meta.build",
            "flm.html.pieces",
            "flm.html.emit-pieces",
            "flm.html.lower",
            "flm.html.emit",
            "flm.html.resolve-definitions",
            "flm.html.regions",
            "flm.html.final.singlefile",
            "flm.math-passthrough.check",
        ]
    );
    assert_eq!(
        plan.active_backends().iter().map(|b| b.to_string()).collect::<Vec<_>>(),
        [html::BACKEND]
    );
}

// --- the goldens: the step's deliverable ------------------------------------------------------

#[test]
fn the_features_document_is_written_as_the_golden_shows() {
    let run = rendered(FEATURES_DOCUMENT);
    let page = rendered_page(&run);
    // Every heading at its level, the two labeled ones carrying the anchor the
    // `refs` feature computed as their identifier.
    assert!(page.contains("<h1 class=\"flm-pres-block--heading\" id=\"sec-intro\""), "{page}");
    assert!(page.contains("<h2 class=\"flm-pres-block--heading\" id=\"sec-fmt\""), "{page}");
    // The two resolved references are links to those identifiers, showing the
    // target's title; the third is the unresolved-reference span.
    assert!(page.contains("href=\"#sec-intro\""), "{page}");
    assert!(page.contains("href=\"#sec-fmt\""), "{page}");
    assert!(page.contains("data-flm-style=\"unresolved-reference\">sec:missing</span>"), "{page}");
    // The formatting commands, each with its own tag and style.
    for style in ["emph", "bold", "italic", "monospace"] {
        assert!(page.contains(&format!("data-flm-style=\"{style}\"")), "{style}: {page}");
    }
    // Typography ran: the dash, the ellipsis and the quotation marks are the
    // characters the substitution produced, and the comment left no trace.
    assert!(page.contains("\u{2014} with a dash, an ellipsis\u{2026}"), "{page}");
    assert!(page.contains("\u{201c}quotes\u{201d}"), "{page}");
    assert!(!page.contains("A comment vanishes"), "{page}");
    // Mathematics reaches the page as the LaTeX the author wrote, with the
    // emitter's own delimiters around it and an anchor at the start of each
    // line of an environment.
    assert!(page.contains("\\(x_1 + \\alpha\\)"), "{page}");
    assert!(page.contains("data-flm-env=\"equation\""), "{page}");
    assert!(page.contains("data-flm-env=\"align*\""), "{page}");
    // Verbatim content, byte for byte, inside a `<code>` and a `<pre><code>`.
    assert!(page.contains("a{b}\\c"), "{page}");
    assert!(page.contains("<pre class=\"flm-pres-verbatim\""), "{page}");
    assert!(page.contains("def f(x):\n    return {x} + 1"), "{page}");
    // The user definitions are expanded before lowering, so no use site reaches
    // the page.
    assert!(page.contains("Hello, World! from a user macro"), "{page}");
    assert!(page.contains("[Note: a user environment]"), "{page}");
    assert!(page.contains("It is version 1.0."), "{page}");
    // The document's one deliberately dangling reference is reported, and
    // changes no byte of the page.
    assert_eq!(identifiers(&run), ["flm.refs.unresolved"]);
    assert_golden_at(golden_path("features.page.html"), &page);
}

#[test]
fn the_session_06_document_is_written_as_the_golden_shows() {
    let run = rendered(SESSION06_DOCUMENT);
    let page = rendered_page(&run);
    assert!(page.contains("<h1 class=\"flm-pres-block--heading\" id=\"sec-intro\""), "{page}");
    assert!(page.contains("href=\"#sec-intro\""), "{page}");
    assert!(page.contains("this emphasized bit"), "{page}");
    // The one non-ASCII character is written through as UTF-8.
    assert!(page.contains('\u{25a1}'), "{page}");
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_golden_at(golden_path("session06.page.html"), &page);
}

#[test]
fn the_features_document_lowers_for_the_html_backend_as_the_golden_shows() {
    let run = rendered(FEATURES_DOCUMENT);
    assert_json_golden(golden_path("features-html-lowered.json"), &lowered_projection(&run));
}

#[test]
fn the_per_tree_output_table_of_the_features_document_projects_as_the_golden_shows() {
    // What `flm dump --table flm.output.html` prints: the tree's HTML as the
    // segments the emit stage wrote, and the identifiers of the definitions
    // the tree needs — the union of what its lowering rules required and what
    // the emitter required as it wrote. This is what an embedder that
    // assembles its own page around the content reads.
    let run = rendered(FEATURES_DOCUMENT);
    assert_json_golden(
        golden_path("features-html-tables.json"),
        &tables_projection(&run, &[OUTPUT_TABLE.name()]),
    );
    // The page carries the tree's HTML unchanged: the final stage indents the
    // first line into the content container and rewrites nothing else. This is
    // why the page golden above and this one hold the same content twice.
    let tree = fragments(&run).trees.get(&TreeId::tree(0)).expect("the tree's record").html();
    assert!(rendered_page(&run).contains(&tree), "the page holds the tree's HTML as written");
}

#[test]
fn the_definitions_table_of_the_features_document_projects_as_the_golden_shows() {
    // What `flm dump --table flm.html.definitions` prints: the manifest an
    // external bundler or a site generator works from — every definition the
    // document requires, once each, in registration order, with its kind, its
    // owner and how it reaches the page.
    let run = rendered(FEATURES_DOCUMENT);
    assert_json_golden(
        golden_path("features-html-definitions.json"),
        &projection(&run, DEFINITIONS_TABLE),
    );
    // The document holds mathematics, so the formula renderer is required; it
    // holds no unattributed callable and no unsupported element, so the
    // stylesheet of the error looks is not.
    let table = run.table(DEFINITIONS_TABLE).expect("declared").expect("written");
    assert!(table.get(html::conventions::SCRIPT_MATHJAX).is_some());
    assert!(table.get(html::conventions::STYLE_ERRORS).is_none());
}

// --- the source-span attribute ------------------------------------------------------------------

#[test]
fn the_render_option_writes_where_each_element_stands_in_its_source() {
    // With `source_spans` on, every element the emitter writes for a node
    // that has a position in a source — inline or block — carries one
    // attribute, whose
    // value is a JSON array of the source's name and the two byte offsets,
    // written between single quotation marks so that the page shows it as
    // JSON. A document parsed from a source with no name carries nothing,
    // which is why this test names its source. That the attribute is the
    // *only* difference the option makes is pinned by `html_emitter.rs`.
    let run = rendered_with_spans(SESSION06_DOCUMENT, "session06.flm");
    let page = rendered_page(&run);
    assert!(page.contains("data-flm-source-span='[\"session06.flm\",0,"), "{page}");
    assert_golden_at(golden_path("session06-spans.page.html"), &page);
}

/// The document through the `html` workflow, parsed from a source named
/// `origin` and rendered with `source_spans` on, everything else as
/// [`spine_overrides`].
fn rendered_with_spans(document: &str, origin: &str) -> Run {
    let registry = registry();
    let source = Arc::new(FlmSource::new(document).with_origin(FlmOrigin::new(origin)));
    let parsed = parse_fragment(
        registry.lang(),
        source,
        FragmentLayout::default(),
        &ParseOptions::default(),
    )
    .expect("the parse yields a tree");
    let mut content = DocContent::new();
    content.place(Arc::new(parsed));
    let doc = Arc::new(Doc::new(content));

    let mut options = spine_render_options();
    let html_options = options.get::<HtmlRenderOptions>().cloned().unwrap_or_default();
    options = options.with(HtmlRenderOptions { source_spans: true, ..html_options });
    let overrides = Overrides { render_options: options, ..Overrides::none() };
    let plan = plan(&registry, html::WORKFLOW, &overrides);
    pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential())
}

// --- one document, three backends ---------------------------------------------------------------

#[test]
fn one_document_serves_three_backends_from_one_registry() {
    // The `text` and `html` backends want the same tree state — paragraphs
    // built, user definitions expanded, references resolved — so one plan
    // carries both outputs and one run writes both. The recomposed LaTeX wants
    // the raw tree instead, which is the exclusive alternative, so it takes a
    // plan and a run of its own over the very same document.
    let registry = registry();
    let doc = pipeline::doc(&registry, &[FEATURES_DOCUMENT]);
    let first = TreeId::tree(0);

    let request = PlanRequest::new()
        .demand("flm.validated")
        .demand(TEXT_DOCUMENT.name())
        .demand(html::DOCUMENT.name())
        .render_options(spine_render_options());
    let together = Arc::new(
        Plan::build(&registry, &request).expect("one plan carries both outputs"),
    );
    assert_eq!(
        together.active_backends().iter().map(|b| b.to_string()).collect::<Vec<_>>(),
        ["html", "text"]
    );
    let run = pipeline::run_with(&doc, &together, Externals::none(), &RunOptions::sequential());
    // Both outputs are the goldens the two single-backend suites pin.
    assert_text_golden(golden_path("features-80.txt"), &text_of(&run));
    assert_golden_at(golden_path("features.page.html"), &rendered_page(&run));
    let structured = run.frozen_tree(first).expect("the run froze the tree");

    // The recomposed LaTeX over the same document, in its own plan.
    let latex_plan = plan(&registry, latex_recomposed::WORKFLOW, &Overrides::none());
    let latex_run =
        pipeline::run_with(&doc, &latex_plan, Externals::none(), &RunOptions::sequential());
    assert_text_golden(golden_path("features-recomposed.tex"), &golden_text(&latex_run));
    let raw = latex_run.frozen_tree(first).expect("the LaTeX run froze the tree");

    // Two plans, two tree states: paragraphs here, paragraph-break markers and
    // kept definitions there.
    assert!(count_element(structured, "flm.paragraph") > 0);
    assert_eq!(count_element(structured, "flm.parbreak"), 0);
    assert_eq!(count_element(raw, "flm.paragraph"), 0);
    assert!(count_element(raw, "flm.parbreak") > 0);
    assert!(!Arc::ptr_eq(structured, raw), "two plans, two frozen trees");

    // Each run holds its own outputs and not the other's.
    assert!(run.table(latex_recomposed::DOCUMENT).expect("declared").is_none());
    assert!(latex_run.table(html::DOCUMENT).expect("declared").is_none());
    assert!(latex_run.table(TEXT_DOCUMENT).expect("declared").is_none());
}

// --- determinism ----------------------------------------------------------------------------

#[test]
fn the_page_is_the_same_bytes_under_both_execution_paths() {
    // The goldens above are compared under every build setting of the battery
    // — the default one, the one without default features and the one that
    // holds rayon to a single thread; this test compares the two runners
    // inside one process over a document of both spine documents, where the
    // emit stage runs once per tree and may run them in parallel.
    let registry = registry();
    let doc = pipeline::doc(&registry, &[FEATURES_DOCUMENT, SESSION06_DOCUMENT]);
    let plan = plan(&registry, html::WORKFLOW, &spine_overrides());
    let sequential = pipeline::run(&doc, &plan, &RunOptions::sequential());
    let parallel = pipeline::run(&doc, &plan, &RunOptions::new());
    let again = pipeline::run(&doc, &plan, &RunOptions::new());
    assert_eq!(rendered_page(&sequential), rendered_page(&parallel));
    assert_eq!(rendered_page(&parallel), rendered_page(&again));
    assert_eq!(identifiers(&sequential), identifiers(&parallel));
    // One fragment record per tree, and both trees' HTML inside the one page.
    assert_eq!(fragments(&sequential).trees.len(), 2);
    let page = rendered_page(&sequential);
    assert_eq!(page.matches("<main class=\"flm-content\"").count(), 1, "{page}");
    assert_eq!(page.matches("Every finite thing is finite").count(), 1, "{page}");
}
