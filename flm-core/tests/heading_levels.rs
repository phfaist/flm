//! Heading levels counted from zero, and the render option
//! `root_heading_level`: every level accepted by the three features that set
//! one (a heading kind's level, an xnote kind's `title_level`, the citations
//! feature's `heading_level`); the page's tags and the text output's marks
//! counted from the root; a heading above the root reported as
//! `flm.pres.heading-above-root` and written as a first-level heading; the
//! attribute of the true level, the numbering depth, the run-in levels, the
//! identifiers, a reference's display and the recomposed LaTeX untouched by
//! the option; the titles "Footnotes" and "References" following the root like
//! any heading; the extreme values rendering without a panic.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::pres::DEFAULT_ROOT_HEADING_LEVEL;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::citations::CitationsConfig;
use flm_core::stdcore::features::headings::{HeadingDecl, HeadingNumbering, HeadingsConfig};
use flm_core::stdcore::features::xnotes::{XnoteKind, XnotesConfig};
use flm_core::stdcore::StdcoreConfig;
use techy::error::Severity;

use common::stdcore::{reports, run_workflow};

/// The identifier of the condition a heading above the root is reported as.
const ABOVE_ROOT: &str = "flm.pres.heading-above-root";

// --- registries and runs ---------------------------------------------------------------------

/// The shipped heading kinds with a `chapter` of level 0 in front of them.
fn with_chapter() -> HeadingsConfig {
    let mut kinds = HeadingsConfig::new().headings;
    kinds.insert(0, HeadingDecl::new("chapter", 0));
    HeadingsConfig::new().headings(kinds)
}

/// The standard package under `headings`, the test citation provider
/// answering every key.
fn registry_of(headings: HeadingsConfig) -> Arc<FlmRegistry> {
    common::citations::registry(StdcoreConfig::new().headings(headings))
}

/// The standard package with a `chapter` of level 0.
fn registry() -> Arc<FlmRegistry> {
    registry_of(with_chapter())
}

/// The render options of the page and of the text output, both with the root
/// `root`.
fn root(root: u8) -> Overrides {
    let render = OptionValues::new()
        .with(html::HtmlRenderOptions { root_heading_level: root, ..html::HtmlRenderOptions::default() })
        .with(text::TextRenderOptions { root_heading_level: root, ..text::TextRenderOptions::default() });
    Overrides { render_options: render, ..Overrides::none() }
}

/// The `html` run of `source` under `registry` and the root `level`.
fn html_run(registry: &Arc<FlmRegistry>, source: &str, level: u8) -> Run {
    run_workflow(registry, html::WORKFLOW, &[source], &root(level))
}

/// The `text` run of `source` under `registry` and the root `level`.
fn text_run(registry: &Arc<FlmRegistry>, source: &str, level: u8) -> Run {
    run_workflow(registry, text::WORKFLOW, &[source], &root(level))
}

/// The HTML of the trees of `source`, the run's reports beside it.
fn html_of(registry: &Arc<FlmRegistry>, source: &str, level: u8) -> (String, Vec<(String, Severity)>) {
    let run = html_run(registry, source, level);
    (common::html::fragment(&run), reports(&run))
}

/// The page of `source`, the run's reports beside it.
fn page_of(registry: &Arc<FlmRegistry>, source: &str, level: u8) -> (String, Vec<(String, Severity)>) {
    let run = html_run(registry, source, level);
    (common::html::page(&run), reports(&run))
}

/// The text output of `source`, the run's reports beside it.
fn text_of(registry: &Arc<FlmRegistry>, source: &str, level: u8) -> (String, Vec<(String, Severity)>) {
    let run = text_run(registry, source, level);
    (common::lowering::text(&run), reports(&run))
}

/// `count` reports of the heading above the root, as Errors.
fn above_root(count: usize) -> Vec<(String, Severity)> {
    vec![(String::from(ABOVE_ROOT), Severity::Error); count]
}

/// The opening tag of a block heading of `kind` at `level`, written `<tag>`,
/// up to its identifier.
fn heading_open(tag: &str, kind: &str, id: &str, level: u8) -> String {
    format!(
        "<{tag} class=\"flm-pres-block--heading\" id=\"{id}\" data-flm-heading=\"{kind}\" \
         data-flm-level=\"{level}\""
    )
}

/// A chapter, a section, a subsection and a subsubsection.
const LADDER: &str = "\\chapter{C}\n\n\\section{S}\n\n\\subsection{U}\n\n\\subsubsection{V}\n";

// --- every level accepted ----------------------------------------------------------------------

#[test]
fn the_three_settings_that_take_a_level_accept_zero() {
    let xnotes = XnotesConfig::empty().kind(XnoteKind::footnote().title_level(0));
    let citations = CitationsConfig::new().heading_level(0);
    let registry = common::citations::registry(
        StdcoreConfig::new().headings(with_chapter()).xnotes(xnotes).citations(citations),
    );
    let (page, reports) = page_of(&registry, "\\chapter{C}\n\nA\\footnote{N} \\cite{k}.\n", 0);
    assert_eq!(reports, [], "{page}");
    assert!(page.contains(&heading_open("h1", "chapter", "sec--C", 0)), "{page}");
    assert!(page.contains("data-flm-heading=\"xnotes\" data-flm-level=\"0\""), "{page}");
    assert!(page.contains("data-flm-heading=\"citations\" data-flm-level=\"0\""), "{page}");
}

// --- the default root ------------------------------------------------------------------------

#[test]
fn the_default_root_is_one() {
    assert_eq!(DEFAULT_ROOT_HEADING_LEVEL, 1);
    assert_eq!(html::HtmlRenderOptions::default().root_heading_level, 1);
    assert_eq!(text::TextRenderOptions::default().root_heading_level, 1);
}

#[test]
fn under_the_default_root_a_level_zero_heading_is_an_error_written_as_a_first_level_heading() {
    let registry = registry();
    let (html, reports) = html_of(&registry, LADDER, DEFAULT_ROOT_HEADING_LEVEL);
    assert_eq!(reports, above_root(1));
    assert!(html.contains(&heading_open("h1", "chapter", "sec--C", 0)), "{html}");
    assert!(html.contains(&heading_open("h1", "section", "sec--S", 1)), "{html}");
    assert!(html.contains(&heading_open("h2", "subsection", "sec--U", 2)), "{html}");
    assert!(html.contains(&heading_open("h3", "subsubsection", "sec--V", 3)), "{html}");

    let run = text_run(&registry, LADDER, DEFAULT_ROOT_HEADING_LEVEL);
    assert_eq!(common::stdcore::reports(&run), above_root(1));
    let diagnostics = run.diagnostics().render_all();
    assert!(
        diagnostics.contains(
            "a heading's level must be at least the root heading level 1 (the render option \
             `root_heading_level`); this heading's level is 0"
        ),
        "{diagnostics}"
    );
    assert_eq!(
        common::lowering::text(&run),
        "C\n=\n\nS\n=\n\nU\n-\n\n### V\n",
    );
}

// --- other roots -----------------------------------------------------------------------------

#[test]
fn under_a_root_of_zero_a_chapter_is_h1_and_a_section_h2() {
    let registry = registry();
    let (html, reports) = html_of(&registry, LADDER, 0);
    assert_eq!(reports, []);
    assert!(html.contains(&heading_open("h1", "chapter", "sec--C", 0)), "{html}");
    assert!(html.contains(&heading_open("h2", "section", "sec--S", 1)), "{html}");
    assert!(html.contains(&heading_open("h3", "subsection", "sec--U", 2)), "{html}");
    assert!(html.contains(&heading_open("h4", "subsubsection", "sec--V", 3)), "{html}");
    let (text, reports) = text_of(&registry, LADDER, 0);
    assert_eq!(reports, []);
    assert_eq!(text, "C\n=\n\nS\n-\n\n### U\n\n#### V\n");
}

#[test]
fn under_a_root_of_two_a_subsection_is_h1_and_a_section_an_error_written_h1() {
    let registry = registry();
    let (html, reports) = html_of(&registry, LADDER, 2);
    assert_eq!(reports, above_root(2));
    assert!(html.contains(&heading_open("h1", "chapter", "sec--C", 0)), "{html}");
    assert!(html.contains(&heading_open("h1", "section", "sec--S", 1)), "{html}");
    assert!(html.contains(&heading_open("h1", "subsection", "sec--U", 2)), "{html}");
    assert!(html.contains(&heading_open("h2", "subsubsection", "sec--V", 3)), "{html}");
    let (text, reports) = text_of(&registry, LADDER, 2);
    assert_eq!(reports, above_root(2));
    assert_eq!(text, "C\n=\n\nS\n=\n\nU\n=\n\nV\n-\n");
}

#[test]
fn the_deeper_depths_share_h6_and_take_one_hash_per_depth() {
    let run_in = registry_of(HeadingsConfig::new().run_in_levels([4, 5]));
    let source = "\\section{A}\n\n\\subsection{B}\n\n\\subsubsection{C}\n\n\\paragraph{D} x\n\n\\subparagraph{E} y\n";
    let deep = registry_of(HeadingsConfig::new());
    let (html, reports) = html_of(&deep, source, 0);
    assert_eq!(reports, []);
    assert!(html.contains(&heading_open("h6", "subparagraph", "sec--E", 5)), "{html}");
    assert!(html.contains(&heading_open("h5", "paragraph", "sec--D", 4)), "{html}");
    let (text, _) = text_of(&deep, source, 0);
    assert!(text.contains("\n#### C\n") && text.contains("\n##### D\n") && text.contains("\n###### E\n"), "{text}");
    // The run-in levels are written run in under every root: the option does
    // not decide which headings run in.
    for level in [0, 1, 2, 4] {
        let (html, _) = html_of(&run_in, source, level);
        assert!(html.contains("class=\"flm-pres-runin--heading\" id=\"sec--D\""), "{level}: {html}");
        assert!(html.contains("class=\"flm-pres-runin--heading\" id=\"sec--E\""), "{level}: {html}");
    }
}

// --- what the option does not touch ----------------------------------------------------------

#[test]
fn the_numbering_the_identifiers_and_a_reference_s_display_do_not_follow_the_root() {
    let registry = registry_of(HeadingsConfig::new().numbering(HeadingNumbering::UpToDepth(1)));
    let source = "\\section{A}\\label{sec:a}\n\n\\subsection{B}\n\nSee \\ref{sec:a}.\n";
    let body = |level: u8| {
        let (text, _) = text_of(&registry, source, level);
        // The words after the heading marks: the numbered titles and the
        // reference, which no root changes.
        text.lines().filter(|line| !line.chars().all(|c| c == '=' || c == '-')).map(|line| line.trim_start_matches('#').trim().to_string()).collect::<Vec<_>>()
    };
    let default = body(DEFAULT_ROOT_HEADING_LEVEL);
    for level in [0, 2, 3] {
        assert_eq!(body(level), default, "root {level}");
    }
    let ids = |level: u8| {
        let (html, _) = html_of(&registry, source, level);
        html.split(" id=\"").skip(1).map(|rest| rest.split('"').next().unwrap_or("").to_string()).collect::<Vec<_>>()
    };
    let default_ids = ids(DEFAULT_ROOT_HEADING_LEVEL);
    assert!(!default_ids.is_empty());
    for level in [0, 2, 3] {
        assert_eq!(ids(level), default_ids, "root {level}");
    }
}

#[test]
fn the_recomposed_latex_does_not_follow_the_root() {
    let registry = registry();
    let latex = |level: u8| {
        let run = run_workflow(&registry, latex_recomposed::WORKFLOW, &[LADDER], &root(level));
        assert_eq!(reports(&run), []);
        common::latex::output(&run)
    };
    let default = latex(DEFAULT_ROOT_HEADING_LEVEL);
    assert!(default.contains("\\chapter{C}"), "{default}");
    for level in [0, 2, 255] {
        assert_eq!(latex(level), default, "root {level}");
    }
}

// --- the titles above the lists --------------------------------------------------------------

#[test]
fn the_footnotes_and_references_titles_follow_the_root_like_any_heading() {
    let registry = registry();
    let source = "A\\footnote{N} \\cite{k}.\n";
    let footnotes = |tag: &str| format!("<{tag} class=\"flm-pres-block--heading\" data-flm-heading=\"xnotes\" data-flm-level=\"2\"");
    let references = |tag: &str| format!("<{tag} class=\"flm-pres-block--heading\" data-flm-heading=\"citations\" data-flm-level=\"2\"");
    for (level, tag, count) in [(DEFAULT_ROOT_HEADING_LEVEL, "h2", 0), (2, "h1", 0), (3, "h1", 2)] {
        let (page, reports) = page_of(&registry, source, level);
        assert_eq!(reports, above_root(count), "root {level}");
        assert!(page.contains(&footnotes(tag)), "root {level}: {page}");
        assert!(page.contains(&references(tag)), "root {level}: {page}");
        let (text, reports) = text_of(&registry, source, level);
        assert_eq!(reports, above_root(count), "root {level}");
        let rule = if tag == "h2" { "-" } else { "=" };
        assert!(text.contains(&format!("Footnotes\n{}\n", rule.repeat(9))), "root {level}: {text}");
        assert!(text.contains(&format!("References\n{}\n", rule.repeat(10))), "root {level}: {text}");
    }
}

// --- a host-mapped tag -----------------------------------------------------------------------

/// The `html` run of `source` under the root `level`, `section` mapped to
/// `<h3>` and `subsection` to `<h4>`.
fn mapped_html_run(registry: &Arc<FlmRegistry>, source: &str, level: u8) -> Run {
    let mut tags = std::collections::BTreeMap::new();
    tags.insert(flm_core::model::names::Name::from("section"), String::from("h3"));
    tags.insert(flm_core::model::names::Name::from("subsection"), String::from("h4"));
    let render = OptionValues::new().with(html::HtmlRenderOptions {
        heading_tags: tags,
        root_heading_level: level,
        ..html::HtmlRenderOptions::default()
    });
    run_workflow(registry, html::WORKFLOW, &[source], &Overrides { render_options: render, ..Overrides::none() })
}

#[test]
fn a_heading_above_the_root_is_reported_whatever_tag_a_host_maps_its_kind_to() {
    let registry = registry();
    let run = mapped_html_run(&registry, "\\section{S}\n\n\\subsection{U}\n\n\\subsubsection{V}\n", 2);
    assert_eq!(reports(&run), above_root(1));
    let html = common::html::fragment(&run);
    assert!(html.contains(&heading_open("h3", "section", "sec--S", 1)), "{html}");
    assert!(html.contains(&heading_open("h4", "subsection", "sec--U", 2)), "{html}");
    assert!(html.contains(&heading_open("h2", "subsubsection", "sec--V", 3)), "{html}");
}

#[test]
fn a_heading_with_a_host_mapped_tag_at_or_below_the_root_reports_nothing() {
    let registry = registry();
    let run = mapped_html_run(&registry, "\\section{S}\n\n\\subsection{U}\n", 1);
    assert_eq!(reports(&run), []);
    let html = common::html::fragment(&run);
    assert!(html.contains(&heading_open("h3", "section", "sec--S", 1)), "{html}");
    assert!(html.contains(&heading_open("h4", "subsection", "sec--U", 2)), "{html}");
}

// --- the extreme values ----------------------------------------------------------------------

#[test]
fn a_level_of_255_and_a_root_of_255_render_without_a_panic() {
    let mut kinds = HeadingsConfig::new().headings;
    kinds.push(HeadingDecl::new("deepest", 255));
    let registry = registry_of(HeadingsConfig::new().headings(kinds));
    let source = "\\section{S}\n\n\\deepest{D}\n";
    let (html, reports) = html_of(&registry, source, DEFAULT_ROOT_HEADING_LEVEL);
    assert_eq!(reports, []);
    assert!(html.contains(&heading_open("h6", "deepest", "sec--D", 255)), "{html}");
    let (text, _) = text_of(&registry, source, DEFAULT_ROOT_HEADING_LEVEL);
    assert!(text.contains(&format!("{} D", "#".repeat(255))), "{text}");
    let (html, reports) = html_of(&registry, source, 255);
    assert_eq!(reports, above_root(1));
    assert!(html.contains(&heading_open("h1", "deepest", "sec--D", 255)), "{html}");
    assert!(html.contains(&heading_open("h1", "section", "sec--S", 1)), "{html}");
    let (text, reports) = text_of(&registry, source, 255);
    assert_eq!(reports, above_root(1));
    assert_eq!(text, "S\n=\n\nD\n=\n");
}
