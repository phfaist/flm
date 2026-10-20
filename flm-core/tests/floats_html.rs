//! The `floats` feature on the HTML backend: a float written as a `<figure>`
//! and the line under its content as a `<figcaption>`, both from the
//! backend's tag table, the content a `<div>`; the feature's stylesheet,
//! required by a page holding a float and by no other; a footnote and a
//! reference inside a caption. The goldens are parsed by
//! `tests/html_validity.rs` with no excuse.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{DefContent, FlmRegistry};
use flm_core::core::run::Run;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::html::conventions::{BLOCK_TAGS, BlockTag, block_tag};
use flm_core::stdcore::backends::html::{DEFINITIONS_TABLE, WORKFLOW as HTML_WORKFLOW};
use flm_core::stdcore::features::floats::{self, FloatNumbering, FloatType, FloatsConfig};

use common::stdcore::{identifiers_but_unknown_size, run_workflow};

// ---------------------------------------------------------------------------------------------
// registries and runs
// ---------------------------------------------------------------------------------------------

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The run of the `html` workflow of `registry` over `document`.
fn html_run(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, HTML_WORKFLOW, &[document], &Overrides::none())
}

/// The HTML fragment of `document` under `registry`; the run must report
/// nothing.
fn html_output(registry: &Arc<FlmRegistry>, document: &str) -> String {
    let run = html_run(registry, document);
    assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    common::html::fragment(&run)
}

/// Whether the page of `run` requires the feature's stylesheet.
fn requires_style(run: &Run) -> bool {
    run.table(DEFINITIONS_TABLE)
        .expect("the definitions table is declared")
        .expect("the definitions table is written")
        .definitions
        .iter()
        .any(|row| row.id.as_str() == floats::html::STYLE)
}

/// The feature's stylesheet, as the page carries it.
fn css() -> String {
    let DefContent::Inline(css) = &floats::html::stylesheet().content[0] else { panic!("inline") };
    css.clone()
}

// ---------------------------------------------------------------------------------------------
// the tag table (test 24)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_tag_table_maps_the_float_and_the_line_under_its_content_and_nothing_else_new() {
    assert_eq!(
        BLOCK_TAGS,
        [
            BlockTag { role: "flm.quote", tag: "blockquote" },
            BlockTag { role: floats::FLOAT_ROLE, tag: "figure" },
            BlockTag { role: floats::CAPTION_ROLE, tag: "figcaption" },
            // The table of contents' row, which is not the floats feature's.
            BlockTag { role: "flm.toc", tag: "nav" },
        ]
    );
    assert_eq!(block_tag(Some(floats::FLOAT_ROLE)), "figure");
    assert_eq!(block_tag(Some(floats::CAPTION_ROLE)), "figcaption");
    // The content is a `<div>`, as every block of a role the table does not
    // name.
    assert_eq!(block_tag(Some(floats::CONTENT_ROLE)), "div");
}

// ---------------------------------------------------------------------------------------------
// the four floats of test 10 (test 16)
// ---------------------------------------------------------------------------------------------

/// Test 10's four floats, each holding an image: bare, captioned, labeled,
/// and labeled and captioned with the fields before the image.
const FOUR: &str = "\\begin{figure}\\includegraphics{a.png}\\end{figure}\n\n\
                    \\begin{figure}\\includegraphics{b.png}\\caption{A photo}\\end{figure}\n\n\
                    \\begin{figure}\\label{figure:c}\\includegraphics{c.png}\\end{figure}\n\n\
                    \\begin{figure}\\caption{Last}\\label{figure:d}\\includegraphics[alt=A plot]{d.png}\\end{figure}\n";

#[test]
fn the_four_floats_are_figures_holding_their_content_then_the_line_under_it() {
    let run = html_run(&std(), FOUR);
    assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(requires_style(&run));
    let html = common::html::fragment(&run);
    // A `figure` is numbered when it carries a label: the first two are not.
    let unnumbered = "<figure class=\"flm-pres-block flm-floats-float\" data-flm-kind=\"figure unnumbered\">";
    assert_eq!(html.matches(unnumbered).count(), 2, "{html}");
    for number in 1..=2 {
        let figure = format!(
            "<figure class=\"flm-pres-block flm-floats-float\" id=\"figure-{number}\" data-flm-kind=\"figure numbered\">"
        );
        assert!(html.contains(&figure), "{number}: {html}");
    }
    assert_eq!(html.matches("<figure").count(), 4, "{html}");
    assert_eq!(html.matches("</figure>").count(), 4, "{html}");
    assert_eq!(html.matches("<div class=\"flm-pres-block flm-floats-float-content\">").count(), 4, "{html}");
    let caption = "<figcaption class=\"flm-pres-block flm-floats-float-caption\" data-flm-kind=\"figure numbered\">";
    assert_eq!(html.matches(caption).count(), 2, "{html}");
    let alone = "<figcaption class=\"flm-pres-block flm-floats-float-caption\" data-flm-kind=\"figure unnumbered\">";
    assert_eq!(html.matches(alone).count(), 1, "{html}");
    assert!(html.contains(
        "<span class=\"flm-pres-span flm-floats-float-number\">Figure&#xA0;2</span>: Last"
    ), "{html}");
    // The content stands before the line under it whatever the source's
    // order: the fourth figure's caption was written first.
    let fourth = html.find("id=\"figure-2\"").expect("the fourth figure");
    let image = html[fourth..].find("src=\"d.png\"").expect("its image");
    let line = html[fourth..].find("<figcaption").expect("its caption");
    assert!(image < line, "{html}");
    // The caption is the last child of its figure.
    for figure in html.split("</figure>").filter(|part| part.contains("<figcaption")) {
        assert!(figure.trim_end().ends_with("</figcaption>"), "{figure}");
    }
    common::html::assert_golden("floats-four.html", &html);
    let page = common::html::page(&run);
    let marker = format!("data-flm-def=\"{}\"", floats::html::STYLE);
    assert_eq!(page.matches(&marker).count(), 1, "{page}");
    assert_eq!(page.matches(css().trim_end()).count(), 1, "{page}");
    common::html::assert_golden("floats-four.page.html", &page);
}

#[test]
fn an_unnumbered_float_carries_its_kind_tokens_and_the_caption_alone() {
    let html = html_output(
        &std(),
        "\\begin{figure}\\includegraphics{a.png}\\caption{A photo}\\end{figure}\n\n\
         \\begin{figure}\\nonumber\\label{figure:b}\\includegraphics{b.png}\\end{figure}",
    );
    assert!(html.contains(
        "<figure class=\"flm-pres-block flm-floats-float\" data-flm-kind=\"figure unnumbered\">"
    ), "{html}");
    assert!(html.contains(
        "<figcaption class=\"flm-pres-block flm-floats-float-caption\" data-flm-kind=\"figure unnumbered\">"
    ), "{html}");
    assert!(!html.contains("flm-floats-float-number"), "{html}");
    // An unnumbered float with a label is anchored by the label; with no
    // caption, it has no line under its content.
    assert!(html.contains(
        "<figure class=\"flm-pres-block flm-floats-float\" id=\"figure-b\" data-flm-kind=\"figure unnumbered\">"
    ), "{html}");
    assert_eq!(html.matches("<figcaption").count(), 1, "{html}");
    common::html::assert_golden("floats-unnumbered.html", &html);
}

#[test]
fn a_configured_type_is_a_figure_too_and_requires_the_stylesheet() {
    let config = FloatsConfig::new().float_type(
        FloatType::new("diagram", ["any"]).numbering(FloatNumbering::AutoNumbered),
    );
    let registry = common::stdcore::registry(StdcoreConfig::new().floats(config));
    let run = html_run(&registry, "\\begin{diagram}\\label{diagram:x}Boxes.\\caption{Arrows}\\end{diagram}");
    assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(requires_style(&run));
    let html = common::html::fragment(&run);
    assert!(html.contains(
        "<figure class=\"flm-pres-block flm-floats-float\" id=\"diagram-1\" data-flm-kind=\"diagram numbered\">"
    ), "{html}");
    assert!(html.contains("<p class=\"flm-pres-par\">Boxes.</p>"), "{html}");
    assert!(html.contains("Diagram&#xA0;1</span>: Arrows"), "{html}");
}

#[test]
fn a_page_holding_no_float_carries_no_stylesheet_of_the_feature() {
    let run = html_run(&std(), "A paragraph.\n");
    assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(!requires_style(&run));
}

#[test]
fn the_stylesheet_sets_a_float_apart_through_custom_properties() {
    let css = css();
    assert!(css.contains(".flm-pres-block.flm-floats-float {"), "{css}");
    assert!(css.contains(".flm-floats-float > .flm-pres-block.flm-floats-float-caption {"), "{css}");
    // Every selector begins with a class of the contract, never a tag.
    for line in css.lines().map(str::trim).filter(|line| line.ends_with('{') && !line.starts_with('@') && !line.starts_with(":root")) {
        assert!(line.starts_with(".flm-"), "`{line}` does not begin with a class of the contract");
        assert!(!line.contains("figure") && !line.contains("figcaption"), "`{line}` names a tag");
    }
    // Every value is a custom property the chunk declares, but for the
    // automatic side margins of a block image, exempted by name: `auto` is
    // how CSS centers a block, not a look a host tunes.
    assert!(css.contains(".flm-floats-float .flm-pres-block.flm-pres-image {\n    margin-inline: auto;\n  }"), "{css}");
    for line in css.lines().map(str::trim) {
        let is_declaration =
            line.ends_with(';') && !line.starts_with("--") && line != "margin-inline: auto;";
        if is_declaration {
            assert!(line.contains("var(--flm-floats-"), "`{line}` is not a custom property");
        }
    }
}

// ---------------------------------------------------------------------------------------------
// a footnote and a reference inside a caption (test 19, the HTML half)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_footnote_and_a_reference_inside_a_caption_are_written_in_the_line_under_the_content() {
    let run = html_run(
        &std(),
        "\\section{Results}\\label{sec:results}\n\n\
         \\begin{figure}\\includegraphics{plot.png}\\caption{A plot\\footnote{Measured twice.} of \
         \\ref{sec:results}.}\\label{figure:plot}\\end{figure}\n\n\
         See \\ref{figure:plot}.\n",
    );
    assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let html = common::html::fragment(&run);
    let caption_at = html.find("<figcaption").expect("the line under the content");
    let caption = &html[caption_at..html[caption_at..].find("</figcaption>").expect("closed") + caption_at];
    assert!(caption.contains("Figure&#xA0;1</span>: A plot"), "{caption}");
    // The footnote's mark stands in the caption, a link to its entry.
    assert!(caption.contains("flm-xnotes-xnote-mark"), "{caption}");
    assert!(caption.contains("href=\"#fn-1\""), "{caption}");
    // The reference inside the caption links to the section.
    assert!(caption.contains("href=\"#sec-results\""), "{caption}");
    // The reference to the figure links to it.
    assert!(html.contains("href=\"#figure-1\""), "{html}");
    // The note itself is listed on the page, once.
    let page = common::html::page(&run);
    assert_eq!(page.matches("Measured twice.").count(), 1, "{page}");
    assert!(page.contains("id=\"fn-1\""), "{page}");
    common::html::assert_golden("floats-caption-notes.page.html", &page);
}
