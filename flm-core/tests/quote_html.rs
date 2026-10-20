//! The `quote` feature on the HTML backend: every quote environment written
//! as a `<blockquote>` from the backend's tag table, its sections as `<div>`s
//! carrying their role's class, the lines of a lines section carrying their
//! indentation as custom properties in a `style` attribute, and the feature's
//! stylesheet, required by a page holding a quotation and by no other. The
//! goldens are parsed by `tests/html_validity.rs` with no excuse.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{DefContent, FlmRegistry};
use flm_core::core::run::Run;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::html::{DEFINITIONS_TABLE, WORKFLOW as HTML_WORKFLOW};
use flm_core::stdcore::backends::html::conventions::{BLOCK_TAGS, block_tag};
use flm_core::stdcore::features::quote::{self, QuoteConfig, QuoteEnvironment, QuoteSection};

use common::stdcore::{identifiers, run_workflow};

// ---------------------------------------------------------------------------------------------
// registries and runs
// ---------------------------------------------------------------------------------------------

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The run of the `html` workflow over `document`.
fn html_run(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, HTML_WORKFLOW, &[document], &Overrides::none())
}

/// The HTML fragment of `document` under the default configuration; the run
/// must report nothing.
fn html_output(document: &str) -> String {
    let run = html_run(&std(), document);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    common::html::fragment(&run)
}

/// Whether the page of `run` requires the feature's stylesheet.
fn requires_style(run: &Run) -> bool {
    run.table(DEFINITIONS_TABLE)
        .expect("the definitions table is declared")
        .expect("the definitions table is written")
        .definitions
        .iter()
        .any(|row| row.id.as_str() == quote::html::STYLE)
}

/// The opening tag of a quotation of the environment `name`.
fn quotation(name: &str) -> String {
    format!("<blockquote class=\"flm-pres-block flm-quote\" data-flm-kind=\"{name}\">")
}

/// The opening tag of a section of the role `flm.quote.<section>`.
fn section(section: &str) -> String {
    format!("<div class=\"flm-pres-block flm-quote-{section}\">")
}

// ---------------------------------------------------------------------------------------------
// the tag table (test 8)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_tag_table_maps_the_quotation_s_role() {
    // The quotation's row, beside the float's two (`tests/floats_html.rs`).
    assert_eq!(BLOCK_TAGS.iter().filter(|row| row.tag == "blockquote").count(), 1);
    assert_eq!(block_tag(Some("flm.quote")), "blockquote");
    // A section's role, a theorem's, no role at all: a `<div>`.
    for role in [Some("flm.quote.text"), Some("flm.theorems.theorem"), None] {
        assert_eq!(block_tag(role), "div", "{role:?}");
    }
}

#[test]
fn a_quotation_is_a_blockquote_and_its_sections_are_divs() {
    let html = html_output(
        "\\begin{quote}\\text{To be or not to be.}\\attributed{Shakespeare}\\end{quote}",
    );
    assert!(html.starts_with(&quotation("quote")), "{html}");
    let text = format!("{}\n    <p class=\"flm-pres-par\">To be or not to be.</p>", section("text"));
    assert!(html.contains(&text), "{html}");
    let attributed =
        format!("{}\n    <p class=\"flm-pres-par\">Shakespeare</p>", section("attributed"));
    assert!(html.contains(&attributed), "{html}");
    assert!(html.trim_end().ends_with("</blockquote>"), "{html}");
    assert_eq!(html.matches("<blockquote").count(), 1, "{html}");
    common::html::assert_golden("quote-sections.html", &html);
}

#[test]
fn every_environment_is_a_blockquote_whatever_its_name() {
    let html = html_output(
        "\\begin{blockquote}One.\n\nTwo.\\end{blockquote}\n\n\
         \\begin{quote}\\block{one line}\\lines{a\\\\b}\\end{quote}\n\n\
         \\begin{address}123 Main St\\\\City\\end{address}",
    );
    for name in ["blockquote", "quote", "address"] {
        assert!(html.contains(&quotation(name)), "{name}: {html}");
    }
    // The block section of a `blockquote` holds its two paragraphs.
    let block = format!(
        "{}\n    <p class=\"flm-pres-par\">One.</p>\n    <p class=\"flm-pres-par\">Two.</p>",
        section("block")
    );
    assert!(html.contains(&block), "{html}");
    for role in ["block", "lines"] {
        assert!(html.contains(&section(role)), "{role}: {html}");
    }
    assert_eq!(html.matches("<blockquote").count(), 3, "{html}");
}

#[test]
fn a_block_of_any_other_role_is_still_a_div() {
    let html = html_output("\\begin{remark}A claim.\\end{remark}");
    assert!(
        html.starts_with("<div class=\"flm-pres-block flm-theorems-theorem\""),
        "{html}"
    );
    assert!(!html.contains("<blockquote"), "{html}");
}

// ---------------------------------------------------------------------------------------------
// a line's indentation on the page (test 8, the lines)
// ---------------------------------------------------------------------------------------------

#[test]
fn an_address_s_lines_carry_their_indentation_as_custom_properties() {
    let html = html_output(
        "\\begin{address}\n123 Main St\\\\\n\\indent City, ST 12345\\\\\nCountry\n\\end{address}",
    );
    assert!(html.starts_with(&quotation("address")), "{html}");
    assert!(html.contains(&section("lines")), "{html}");
    let lines = [
        "<div class=\"flm-pres-line\" data-flm-align=\"left\">123 Main St</div>",
        "<div class=\"flm-pres-line\" style=\"--flm-element-indent-left: calc(1 * var(--flm-tab))\" \
         data-flm-align=\"left\">City, ST 12345</div>",
        "<div class=\"flm-pres-line\" data-flm-align=\"left\">Country</div>",
    ];
    for line in lines {
        assert!(html.contains(line), "{line}: {html}");
    }
    assert!(!html.contains("data-flm-indent"), "{html}");
    common::html::assert_golden("quote-address.html", &html);
}

#[test]
fn a_line_indented_two_tabs_carries_one_property_and_a_plain_line_no_style() {
    let html = html_output("\\begin{quote}\\lines{a\\\\\\indent\\indent b}\\end{quote}");
    let indented = "<div class=\"flm-pres-line\" style=\"--flm-element-indent-left: calc(2 * \
                    var(--flm-tab))\" data-flm-align=\"left\">b</div>";
    assert!(html.contains(indented), "{html}");
    assert!(html.contains("<div class=\"flm-pres-line\" data-flm-align=\"left\">a</div>"), "{html}");
    assert_eq!(html.matches("style=").count(), 1, "{html}");
    assert!(!html.contains("data-flm-indent-left"), "{html}");
}

// ---------------------------------------------------------------------------------------------
// the stylesheet and the whole of test 8
// ---------------------------------------------------------------------------------------------

/// Tests 1 to 3 in one document: the four sections of a `quote` in the order
/// written, a `blockquote` of two paragraphs, a `\block` of one line, and an
/// `address` of three lines, the second indented.
const TESTS_ONE_TO_THREE: &str = "\\begin{quote}\\text{To be or not to be.}\\attributed{Shakespeare}\\end{quote}

\\begin{quote}
\\text{A passage.}
\\lines{First line\\\\ \\indent Second line}
\\block{One paragraph.

Another paragraph.}
\\attributed{Someone}
\\end{quote}

\\begin{blockquote}
First paragraph.

Second paragraph.
\\end{blockquote}

\\begin{quote}\\block{one line}\\end{quote}

\\begin{address}
123 Main St\\\\
\\indent City, ST 12345\\\\
Country
\\end{address}
";

#[test]
fn the_html_of_tests_one_to_three_is_valid_and_requires_the_stylesheet_once() {
    let run = html_run(&std(), TESTS_ONE_TO_THREE);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(requires_style(&run));
    let html = common::html::fragment(&run);
    assert_eq!(html.matches("<blockquote").count(), 5, "{html}");
    let order: Vec<usize> = ["text", "lines", "block", "attributed"]
        .iter()
        .map(|role| {
            let second = html.find(&quotation("quote")).and_then(|first| {
                html[first + 1..].find(&quotation("quote")).map(|at| at + first + 1)
            });
            let from = second.expect("the second quotation");
            html[from..].find(&section(role)).expect("the section") + from
        })
        .collect();
    assert!(order.windows(2).all(|pair| pair[0] < pair[1]), "{order:?}: {html}");
    common::html::assert_golden("quote-tests-1-3.html", &html);
    let page = common::html::page(&run);
    let marker = format!("data-flm-def=\"{}\"", quote::html::STYLE);
    assert_eq!(page.matches(&marker).count(), 1, "{page}");
    let DefContent::Inline(css) = &quote::html::stylesheet().content[0] else { panic!("inline") };
    assert_eq!(page.matches(css.trim_end()).count(), 1, "{page}");
    common::html::assert_golden("quote-tests-1-3.page.html", &page);
}

#[test]
fn a_page_holding_no_quotation_carries_no_stylesheet_of_the_feature() {
    let run = html_run(&std(), "A paragraph.\n");
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(!requires_style(&run));
}

#[test]
fn a_configured_environment_is_a_blockquote_and_requires_the_stylesheet() {
    let config = QuoteConfig::new()
        .environment("myquote", QuoteEnvironment::new([QuoteSection::Text]))
        .environment("aside", QuoteEnvironment::new([]).bare(QuoteSection::Block));
    let registry = common::stdcore::registry(StdcoreConfig::new().quote(config));
    let run = html_run(
        &registry,
        "\\begin{myquote}\\text{Mine.}\\end{myquote}\n\n\\begin{aside}Aside.\\end{aside}\n",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(requires_style(&run));
    let html = common::html::fragment(&run);
    assert!(html.contains(&quotation("myquote")), "{html}");
    assert!(html.contains(&quotation("aside")), "{html}");
}

#[test]
fn the_stylesheet_sets_a_quotation_apart_through_custom_properties() {
    let DefContent::Inline(css) = &quote::html::stylesheet().content[0] else { panic!("inline") };
    assert!(css.contains(".flm-quote > .flm-quote-attributed {"), "{css}");
    // An environment is selected by its kind token, never by the tag.
    assert!(css.contains(".flm-quote[data-flm-kind~=\"address\"] > .flm-quote-lines {"), "{css}");
    assert!(!css.contains("blockquote.") && !css.contains("blockquote {"), "{css}");
    for line in css.lines().map(str::trim) {
        let is_declaration = line.ends_with(';') && !line.starts_with("--");
        if is_declaration {
            assert!(line.contains("var(--flm-quote-"), "`{line}` is not a custom property");
        }
    }
    // No script, and no selector reaching past the contract's classes.
    assert!(!css.contains("<script"), "{css}");
}
