//! The HTML backend through a plan of the `html` workflow
//! (`flm_core::stdcore::backends::html`): the stages and rewrites the workflow
//! schedules, the page it writes as a one-artifact set, a heading and two
//! referencing callables giving the identifier and the two links, a fragment
//! parsed with the inline layout, the three execution settings agreeing byte
//! for byte, one plan serving both the `text` and the `html` output, the
//! refusal of `html` beside `latex-recomposed`, and the page assembly's two
//! conditions on a tree placed in another tree.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use flm_core::core::artifact::ArtifactId;
use flm_core::core::plan::{Overrides, Plan, PlanError, PlanRequest};
use flm_core::core::registry::{DefContent, FlmRegistry};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::diag::expect;
use flm_core::lang::{FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::model::addr::TreeId;
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::names::Name;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::html::{
    HtmlPieces,
    self, DOCUMENT, DefinitionOverride, HtmlFragment, HtmlFragments, HtmlRenderOptions,
    HtmlSegment, ResolvedDefinitions, page,
};
use flm_core::stdcore::backends::text::{self, TEXT_DOCUMENT};
use flm_core::stdcore::backends::latex_recomposed;
use flm_core::stdcore::StdcoreConfig;
use techy::error::Severity;

use common::html::{
    assert_golden, fragment, fragments, page as rendered_page, run_one_fragment_under,
};
use common::stdcore::{plan, registry, run_workflow};

/// The registry of the standard package under its default configuration.
fn std_registry() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// Runs `texts` under the `html` workflow.
fn render(texts: &[&str]) -> Run {
    run_workflow(&std_registry(), html::WORKFLOW, texts, &Overrides::none())
}

/// Runs `texts` under the `html` workflow with every stylesheet this backend
/// ships delivered by a local address, so that each one the document requires
/// reaches the page as one `<link rel="stylesheet">` line.
///
/// The page goldens of this suite pin the page's skeleton and the content in
/// it. The bytes of the shipped stylesheets are a property of their own, and
/// are pinned once, by `definitions.page.html` in `tests/html_page.rs`;
/// carrying them here as well would bury each golden's own property under
/// two hundred lines of unchanging rules. An address is used rather than
/// [`DefinitionOverride::ProvidedByHost`] because a `<link>` per definition
/// still shows *which* chunks the document required, which is what these
/// goldens pin about the definitions.
fn render_with_stylesheets_by_address(texts: &[&str]) -> Run {
    let mut definitions = BTreeMap::new();
    for style in html::defs::stylesheets() {
        let id = style.id.to_string();
        let stem = id.rsplit('.').next().expect("an identifier has a last part");
        definitions.insert(
            Name::from(id.as_str()),
            DefinitionOverride::Parts(vec![DefContent::Url(format!("css/{stem}.css"))]),
        );
    }
    let options = HtmlRenderOptions { definitions, ..HtmlRenderOptions::default() };
    let overrides = Overrides {
        render_options: OptionValues::new().with(options),
        ..Overrides::none()
    };
    run_workflow(&std_registry(), html::WORKFLOW, texts, &overrides)
}

/// Every stage the `html` workflow schedules, in the order they run.
const EXPECTED_STAGES: [&str; 27] = [
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
];

// --- the plan ------------------------------------------------------------------------------

#[test]
fn the_html_workflow_schedules_the_rewrites_and_the_stages_it_needs() {
    let plan = plan(&std_registry(), html::WORKFLOW, &Overrides::none());
    assert_eq!(
        plan.active_backends().iter().map(|b| b.to_string()).collect::<Vec<_>>(),
        ["html"]
    );
    let rewrites: Vec<String> = plan.rewrite_names().iter().map(|n| n.to_string()).collect();
    assert_eq!(
        rewrites,
        ["flm.input.splice", "flm.userdefs.expand-always", "flm.userdefs.expand", "flm.blocks.structure"]
    );
    // The whole stage list, in the order the stages run. The backend claims no
    // element the `refs` feature owns, so that feature's own rule is selected,
    // its read of the resolutions schedules the resolve stage and, behind it,
    // the stage that computes the anchors.
    let stages: Vec<String> = plan.stage_names().iter().map(|n| n.to_string()).collect();
    assert_eq!(stages, EXPECTED_STAGES, "{stages:?}");
    let at = |name: &str| {
        stages.iter().position(|n| n == name).unwrap_or_else(|| panic!("{name} in {stages:?}"))
    };
    assert!(at("flm.refs.anchors") < at("flm.html.lower"));
    assert!(at("flm.html.lower") < at("flm.html.emit"));
    assert!(at("flm.html.emit") < at("flm.html.final.singlefile"));
}

#[test]
fn one_plan_serves_the_text_and_the_html_output_together() {
    // Both backends want the same tree state — blocks structured, user
    // definitions expanded — so one plan carries both outputs.
    let registry = std_registry();
    let request =
        PlanRequest::new().demand(TEXT_DOCUMENT.name()).demand(DOCUMENT.name());
    let plan = Plan::build(&registry, &request).expect("one plan carries both outputs");
    let backends: Vec<String> =
        plan.active_backends().iter().map(|b| b.to_string()).collect();
    assert_eq!(backends, ["html", "text"]);
}

#[test]
fn html_and_latex_recomposed_in_one_request_are_refused() {
    // The recomposed backend's rules want the raw tree; this one wants the
    // structured tree, and the two states are mutually exclusive.
    let registry = std_registry();
    let request = PlanRequest::new()
        .demand(DOCUMENT.name())
        .demand(latex_recomposed::DOCUMENT.name());
    assert!(matches!(
        Plan::build(&registry, &request),
        Err(PlanError::Unsatisfiable { .. })
    ));
}

// --- the page ------------------------------------------------------------------------------

#[test]
fn the_workflow_writes_the_page_as_a_one_artifact_set() {
    let run = render(&["Hello world."]);
    let set = run.table(DOCUMENT).expect("declared").expect("written");
    assert_eq!(set.len(), 1);
    let entry = set.entry().expect("the entry artifact");
    assert_eq!(entry.id, ArtifactId::new(html::ARTIFACT_OWNER, html::ARTIFACT_NAME));
    assert_eq!(entry.media_type, html::MEDIA_TYPE);
    assert_eq!(html::EXTENSION, "html");
    let page = rendered_page(&run);
    assert!(page.starts_with("<!DOCTYPE html>\n<html>"), "{page}");
    assert!(page.contains("<title>FLM document</title>"), "{page}");
    assert!(page.trim_end().ends_with("</html>"), "{page}");
}

/// A document with a heading, a reference to it and a `\hyperref` to it: what
/// the anchors stage computes becomes the heading's `id`, and both referencing
/// callables become links to it.
const REFERENCED: &str = "\\section{Sums}\\label{sec:sums}\n\nSee \\ref{sec:sums} and \
                          \\hyperref[sec:sums]{the section on sums}.\n";

#[test]
fn a_heading_carries_the_anchor_as_its_identifier_and_both_callables_link_to_it() {
    let run = render(&[REFERENCED]);
    let html = fragment(&run);
    let id = html
        .split_once("id=\"")
        .map(|(_, rest)| rest.split('"').next().expect("the value ends").to_string())
        .expect("the heading carries an identifier");
    assert!(!id.is_empty());
    assert_eq!(html.matches(&format!("href=\"#{id}\"")).count(), 2, "{html}");
    assert_eq!(html.matches("data-flm-kind=\"ref\"").count(), 2, "{html}");
    assert!(html.contains("the section on sums"), "{html}");
    assert_golden("references.html", &html);
}

#[test]
fn a_small_document_is_a_page() {
    let run = render_with_stylesheets_by_address(&[
        "\\section{A title}\n\nA paragraph with \\emph{emphasis} and \\verbcode|code|.\n",
    ]);
    assert_golden("document.page.html", &rendered_page(&run));
}

/// A document whose mathematics covers the shapes a formula is written in:
/// inline mathematics, a mathematics environment, and a text command inside a
/// formula.
const MATHEMATICS: &str = "Let \\(a = \\text{one half}\\) hold.\n\n\
                           \\begin{align}\n  x &= y \\\\\n  z &= w\n\\end{align}\n\n\
                           And \\(b > a\\) too.\n";

#[test]
fn a_document_holding_mathematics_is_a_page_whose_formulas_are_latex() {
    let run = render_with_stylesheets_by_address(&[MATHEMATICS]);
    let page = rendered_page(&run);
    // A formula reaches the page as the LaTeX the author wrote, inside the
    // element the formula renderer is pointed at: inline mathematics in a
    // `<span>`, a mathematics environment in a `<div>` naming it.
    assert!(page.contains("class=\"flm-pres-math--passthrough\" data-flm-display=\"inline\""), "{page}");
    assert!(page.contains("data-flm-env=\"align\""), "{page}");
    assert!(page.contains("\\begin{align}"), "{page}");
    // The text inside a formula is content, not notation: the
    // `math-passthrough` feature's rule stages it as a `flm.pres.math-text` node, which the
    // emitter writes as HTML inside the formula's LaTeX. Each line of the
    // environment begins with an empty one carrying the line's anchor, so that
    // a reference to that line has a point in the formula to land on. What
    // those rules stage is covered in `tests/html_math.rs`.
    assert!(!page.contains("\\text{one half}"), "{page}");
    assert!(page.contains("data-flm-command=\"text\">one half</span></tex-html>"), "{page}");
    assert_golden("document-math.page.html", &page);
}

#[test]
fn a_tree_holding_mathematics_asks_the_page_for_the_formula_renderer() {
    // A requirement is a name the emit stage records per tree; the piece that
    // turns it into a script in the page is the next one.
    use flm_core::stdcore::backends::html::conventions::SCRIPT_MATHJAX;

    let run = render(&[MATHEMATICS]);
    let required: Vec<String> = fragments(&run)
        .trees
        .values()
        .flat_map(|fragment| fragment.requires.iter())
        .map(|name| name.as_str().to_string())
        .collect();
    assert!(required.contains(&String::from(SCRIPT_MATHJAX)), "{required:?}");
    assert!(required.contains(&String::from("flm.html.style.math-passthrough")), "{required:?}");
}

#[test]
fn the_page_of_a_document_holding_mathematics_is_the_same_under_every_setting() {
    let registry = std_registry();
    let run_with = |options: &RunOptions| {
        let source = Arc::new(
            FlmSource::new(MATHEMATICS).with_origin(FlmOrigin::new("math.flm")),
        );
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
        let plan = plan(&registry, html::WORKFLOW, &Overrides::none());
        let mut run = Run::new(doc, plan, Externals::none()).expect("the run starts");
        run.execute(options).expect("the run finishes");
        rendered_page(&run)
    };
    assert_eq!(run_with(&RunOptions::sequential()), run_with(&RunOptions::new()));
}

#[test]
fn a_fragment_parsed_with_the_inline_layout_is_written_without_a_paragraph() {
    let run = run_one_fragment_under(
        &std_registry(),
        FragmentLayout::Inline,
        "A \\emph{title} in one run",
    );
    let html = fragment(&run);
    assert!(!html.contains("<p"), "{html}");
    assert_eq!(
        html.trim_end(),
        "A <em class=\"flm-pres-span\" data-flm-style=\"emph\">title</em> in one run"
    );
}

#[test]
fn the_page_is_the_same_under_the_three_execution_settings() {
    let registry = std_registry();
    let doc = || {
        let source = Arc::new(
            FlmSource::new(REFERENCED).with_origin(FlmOrigin::new("doc.flm")),
        );
        let parsed = parse_fragment(
            registry.lang(),
            source,
            FragmentLayout::default(),
            &ParseOptions::default(),
        )
        .expect("the parse yields a tree");
        let mut content = DocContent::new();
        content.place(Arc::new(parsed));
        Arc::new(Doc::new(content))
    };
    let run_with = |options: &RunOptions| {
        let plan = plan(&registry, html::WORKFLOW, &Overrides::none());
        let mut run = Run::new(doc(), plan, Externals::none()).expect("the run starts");
        run.execute(options).expect("the run finishes");
        rendered_page(&run)
    };
    let sequential = run_with(&RunOptions::sequential());
    let default = run_with(&RunOptions::new());
    assert_eq!(sequential, default);
}

// --- the render options ----------------------------------------------------------------------

#[test]
fn the_title_and_the_language_tag_come_from_the_render_options() {
    use flm_core::model::options::OptionValues;

    let overrides = Overrides {
        render_options: OptionValues::new().with(HtmlRenderOptions {
            title: String::from("A <report> & more"),
            lang: Some(String::from("de-CH")),
            ..HtmlRenderOptions::default()
        }),
        ..Overrides::none()
    };
    let run = run_workflow(&std_registry(), html::WORKFLOW, &["Text."], &overrides);
    let page = rendered_page(&run);
    assert!(page.contains("<html lang=\"de-CH\">"), "{page}");
    // The title is document-derived text like any other: it is escaped where
    // it is written.
    assert!(page.contains("<title>A &lt;report&gt; &amp; more</title>"), "{page}");
}

// --- the trust guarantee, end to end ------------------------------------------------------------

#[test]
fn a_document_holding_every_character_that_could_break_out_reaches_the_page_escaped() {
    let run = render(&[
        "Text with < and > and & and \" and </script> and --> and <tex-html>.\n\n\
         \\verbcode|a < b & c </script>|\n",
    ]);
    let page = rendered_page(&run);
    assert!(!page.contains("</script>"), "{page}");
    assert!(!page.contains("<tex-html>"), "{page}");
    assert!(page.contains("&lt;/script&gt;"), "{page}");
    assert!(page.contains("&lt;tex-html&gt;"), "{page}");
    // Nothing this backend writes is an HTML comment, so nothing in a page
    // can close one: a document's own text never holds `<!--` or `-->` either,
    // the typographic pass having turned the two hyphens into a dash.
    assert!(!page.contains("<!--"), "{page}");
    assert!(!page.contains("-->"), "{page}");
}

#[test]
fn mathematics_holding_the_formula_renderers_own_tag_cannot_reach_the_page_as_markup() {
    // What a reviewer tries: a document that writes, inside a formula, the tag
    // by which HTML is put into a formula. The sink escapes it in the page,
    // and the renderer reads the text a browser built from the page, where the
    // escaping is undone — so the emitter writes an empty TeX group inside the
    // tag as well. Both are visible here.
    let run = render(&[
        "A formula \\(a < b\\) and \\(x = <tex-html><b id=\"p\">P</b></tex-html>\\) \
         and \\(y = </tex-html>\\).\n",
    ]);
    let page = rendered_page(&run);
    assert!(!page.contains("<tex-html>"), "{page}");
    assert!(!page.contains("<b id="), "{page}");
    // Escaped, and broken: `&lt;{}tex-html` is what a browser hands the
    // formula renderer as `<{}tex-html`, which is not its tag.
    assert_eq!(page.matches("&lt;{}tex-html").count(), 1, "{page}");
    assert_eq!(page.matches("&lt;{}/tex-html").count(), 2, "{page}");
    // The reader sees the characters the author typed, since the group
    // typesets nothing.
    assert!(page.contains("&lt;{}tex-html&gt;&lt;b id=\"p\"&gt;"), "{page}");
}

// --- the page assembly's conditions ---------------------------------------------------------

/// A fragments table from `(tree, segments)` pairs.
fn fragments_of(trees: Vec<(u32, Vec<HtmlSegment>)>) -> HtmlFragments {
    let mut table = HtmlFragments::default();
    for (index, segments) in trees {
        table.trees.insert(
            TreeId::tree(index),
            HtmlFragment { segments, requires: Default::default() },
        );
    }
    table
}

#[test]
fn a_placed_tree_is_written_where_it_was_placed_and_not_again_at_the_top_level() {
    let table = fragments_of(vec![
        (
            0,
            vec![
                HtmlSegment::Html(String::from("<p>before</p>")),
                HtmlSegment::Tree(TreeId::tree(1)),
                HtmlSegment::Html(String::from("<p>after</p>")),
            ],
        ),
        (1, vec![HtmlSegment::Html(String::from("<p>placed</p>"))]),
    ]);
    let made = page::regions(
        &table,
        &HtmlPieces::default(),
        &ResolvedDefinitions::default(),
        &[TreeId::tree(0), TreeId::tree(1)],
        &HtmlRenderOptions::default(),
    );
    assert!(made.missing.is_empty());
    assert!(made.cycles.is_empty());
    let assembled = page::assemble(&made.regions);
    assert_eq!(assembled.matches("<p>placed</p>").count(), 1);
    let before = assembled.find("<p>before</p>").expect("written");
    let placed = assembled.find("<p>placed</p>").expect("written");
    let after = assembled.find("<p>after</p>").expect("written");
    assert!(before < placed && placed < after);
    assert_golden("subtree.page.html", &assembled);
}

#[test]
fn a_placed_tree_with_no_html_is_reported_and_writes_nothing() {
    let table =
        fragments_of(vec![(0, vec![HtmlSegment::Tree(TreeId::tree(7))])]);
    let made = page::regions(
        &table,
        &HtmlPieces::default(),
        &ResolvedDefinitions::default(),
        &[TreeId::tree(0)],
        &HtmlRenderOptions::default(),
    );
    assert_eq!(made.missing, vec![TreeId::tree(7)]);
    assert!(made.cycles.is_empty());
}

#[test]
fn two_trees_placing_each_other_are_written_once_and_the_cycle_is_reported() {
    let table = fragments_of(vec![
        (
            0,
            vec![
                HtmlSegment::Html(String::from("<p>zero</p>")),
                HtmlSegment::Tree(TreeId::tree(1)),
            ],
        ),
        (
            1,
            vec![
                HtmlSegment::Html(String::from("<p>one</p>")),
                HtmlSegment::Tree(TreeId::tree(0)),
            ],
        ),
    ]);
    let made = page::regions(
        &table,
        &HtmlPieces::default(),
        &ResolvedDefinitions::default(),
        &[TreeId::tree(0), TreeId::tree(1)],
        &HtmlRenderOptions::default(),
    );
    assert_eq!(made.cycles, vec![TreeId::tree(0)]);
    let assembled = page::assemble(&made.regions);
    assert_eq!(assembled.matches("<p>zero</p>").count(), 1);
    assert_eq!(assembled.matches("<p>one</p>").count(), 1);
}

// --- every check still runs ----------------------------------------------------------------

#[test]
fn an_unresolved_reference_is_an_error_under_this_render_as_under_the_others() {
    let run = render(&["See \\ref{sec:nothing}.\n"]);
    expect(run.diagnostics().iter(), "flm.refs.unresolved").severity(Severity::Error).once();
    // The content stays and the page is still produced.
    let page = rendered_page(&run);
    assert!(page.contains("data-flm-style=\"unresolved-reference\""), "{page}");
}

#[test]
fn the_text_backend_and_this_one_agree_on_what_the_document_says() {
    let registry = std_registry();
    let html_run = run_workflow(&registry, html::WORKFLOW, &["A \\emph{word}."], &Overrides::none());
    let text_run = run_workflow(&registry, text::WORKFLOW, &["A \\emph{word}."], &Overrides::none());
    let html = fragment(&html_run);
    let text = common::lowering::text(&text_run);
    assert!(html.contains("word"), "{html}");
    assert_eq!(text, "A word.\n");
}
