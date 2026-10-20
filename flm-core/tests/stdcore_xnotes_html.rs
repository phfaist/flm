//! The `xnotes` feature on the `html` backend: the mark with its link, the
//! list of a kind with its links back, the two in-place shapes without bracket
//! text, the stylesheet, and what changes where a feature is absent.
//!
//! Every golden this suite writes under `tests/goldens/html/` is parsed by
//! `tests/html_validity.rs` in the same test run, which also holds that every
//! link into the page names an identifier the page has; none is excused.
//!
//! Every registry is the standard package, its xnotes feature under the
//! configuration a test names, unless a test builds one of its own.

mod common;

use std::collections::BTreeMap;
use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{DefContent, FlmFeature, FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::model::names::Name;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::html::{
    self, DEFINITIONS_TABLE, DefinitionKind, HtmlRenderOptions,
};
use flm_core::stdcore::backends::{lowering, pieces};
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::numbering::{
    CounterStyleDecl, NumberTemplate, NumberingConfig, PrefixWords,
};
use flm_core::stdcore::features::xnotes::{
    self, XnoteKind, XnotePlacement, XnotesConfig, XnotesFeature,
};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::features::{index, numbering, refpoints};
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig, check_feature, package};

use common::html::{assert_golden, fragment, page};
use common::stdcore::{identifiers, identifiers_but_unknown_size, plan, registry_over, run_workflow};

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

/// The standard package, its xnotes feature under `xnotes`.
fn registry(xnotes: XnotesConfig) -> Arc<FlmRegistry> {
    registry_with(StdcoreConfig::new(), xnotes)
}

/// The standard package under its default configuration: the one kind
/// `footnote`, listed.
fn std() -> Arc<FlmRegistry> {
    registry(XnotesConfig::new())
}

/// The configuration whose `footnote` kind is shown in place.
fn in_place() -> XnotesConfig {
    XnotesConfig { kinds: vec![XnoteKind::footnote().placement(XnotePlacement::InPlace)] }
}

/// An unnumbered kind shown in place, `\sidenote{…}`, with the empty mark.
fn sidenote() -> XnoteKind {
    XnoteKind::unnumbered("sidenote", PrefixWords::new("Side note~", "Side notes~"))
        .commands(["sidenote"])
        .label_type("sn")
        .placement(XnotePlacement::InPlace)
}

/// The `html` run of `texts`, one tree each, under `registry` and
/// `overrides`, sequentially.
fn render_under(registry: &Arc<FlmRegistry>, texts: &[&str], overrides: &Overrides) -> Run {
    run_workflow(registry, html::WORKFLOW, texts, overrides)
}

/// The `html` run of `texts` under `registry`; the run must report nothing.
fn clean_with(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Run {
    let run = render_under(registry, texts, &Overrides::none());
    assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    run
}

/// The `html` run of `texts` under the default registry; the run must report
/// nothing.
fn clean(texts: &[&str]) -> Run {
    clean_with(&std(), texts)
}

/// The part of `page` inside `<main …>` and `</main>`: the trees, then the
/// pieces placed after them.
fn main_of(page: &str) -> String {
    let start = page.find("<main").expect("the page has a <main>");
    let start = start + page[start..].find('>').expect("the <main> tag closes") + 1;
    let end = page.find("</main>").expect("the page closes its <main>");
    String::from(&page[start..end])
}

/// The identifiers of the definitions the run's document requires, in the
/// definitions table's order.
fn definitions(run: &Run) -> Vec<(String, DefinitionKind, String)> {
    run.table(DEFINITIONS_TABLE)
        .expect("the definitions table is declared")
        .expect("the definitions table is written")
        .definitions
        .iter()
        .map(|row| (row.id.to_string(), row.kind, row.owner.to_string()))
        .collect()
}

/// Whether the run's document requires the stylesheet `flm.xnotes.style`.
fn requires_style(run: &Run) -> bool {
    definitions(run).iter().any(|(id, _, _)| id == xnotes::STYLE)
}

/// The value of the attribute `name` on the first element of `html` whose
/// opening tag holds `marker`.
fn attribute_of(html: &str, marker: &str, name: &str) -> Option<String> {
    let at = html.find(marker)?;
    let open = html[..at].rfind('<')?;
    let close = at + html[at..].find('>')?;
    let tag = &html[open..close];
    let key = format!(" {name}=\"");
    let start = tag.find(&key)? + key.len();
    let end = start + tag[start..].find('"')?;
    Some(String::from(&tag[start..end]))
}

/// The worked example of the design: a note of one run, and a note of two
/// paragraphs.
const WORKED: &str = "The claim holds in every case.\\footnote{Except the empty one, which we exclude \
    by convention.} The next sentence goes on.\n\n\
    Two paragraphs.\\footnote{First paragraph.\n\n\
    Second paragraph.} And on.";

/// The mark of the xnote whose identifier is `anchor`: its `<sup>`, as the
/// page writes it, from the opening tag to the closing one.
fn mark_of(html: &str, anchor: &str) -> String {
    let href = format!("href=\"#{anchor}\"");
    let link = html.find(&href).unwrap_or_else(|| panic!("no link to {anchor}: {html}"));
    let start = html[..link].rfind("<sup").expect("the mark's <sup>");
    let end = link + html[link..].find("</sup>").expect("the mark closes") + "</sup>".len();
    String::from(&html[start..end])
}

// ---------------------------------------------------------------------------------------------
// the worked example's page (test 51)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_worked_example_s_page_is_the_golden() {
    let run = clean(&[WORKED]);
    let page = page(&run);
    let main = main_of(&page);
    // The trees, then the list, inside <main>.
    let at = |text: &str| main.find(text).unwrap_or_else(|| panic!("{text} in {main}"));
    assert!(at("And on.") < at("role=\"doc-endnotes\""), "{main}");
    assert_golden("xnotes-worked.page.html", &page);
}

// ---------------------------------------------------------------------------------------------
// the mark (test 52)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_mark_is_a_superscript_carrying_its_identifier_around_a_link_to_the_xnote() {
    let run = clean(&[WORKED]);
    let html = fragment(&run);
    let first = mark_of(&html, "fn-1");
    // The <sup> carries the mark's identifier, the position's four letters
    // after `fnref--1`; the link carries the xnote's, its role and its
    // accessible name.
    let mark_id = attribute_of(&first, "flm-xnotes-xnote-mark", "id").expect("the mark's id");
    assert!(mark_id.starts_with("fnref--1") && mark_id.len() == "fnref--1".len() + 4, "{mark_id}");
    assert_eq!(
        first,
        format!(
            "<sup class=\"flm-pres-span flm-xnotes-xnote-mark\" id=\"{mark_id}\" \
             data-flm-style=\"superscript\"><a class=\"flm-pres-link\" href=\"#fn-1\" \
             role=\"doc-noteref\" aria-label=\"Footnote 1\" data-flm-kind=\"xnote\">1</a></sup>"
        )
    );
    // Attached to the word before it; the text after it keeps its space.
    assert!(html.contains(&format!("every case.{first} The next")), "{html}");
    let second = mark_of(&html, "fn-2");
    assert!(second.contains("aria-label=\"Footnote 2\""), "{second}");
    assert!(second.contains(">2</a></sup>"), "{second}");
    // No stylesheet rule and no bracket text for a listed mark.
    assert!(!html.contains('['), "{html}");
}

#[test]
fn a_labeled_xnote_is_the_target_of_a_reference_on_the_page() {
    let run = clean(&["A note.\\footnote[\\label{fn:e}]{See the appendix.} See \\ref{fn:e}."]);
    let main = main_of(&page(&run));
    // The reference links to the xnote's identifier, which the list entry
    // carries.
    assert!(main.contains("href=\"#fn-1\""), "{main}");
    assert!(main.contains("<li id=\"fn-1\">"), "{main}");
    assert!(main.contains(">Footnote&#xA0;1</a>"), "{main}");
}

// ---------------------------------------------------------------------------------------------
// the list (test 53)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_list_is_an_endnotes_block_with_its_title_and_a_labeled_list_with_links_back() {
    let run = clean(&[WORKED]);
    let html = fragment(&run);
    let main = main_of(&page(&run));
    let marks: Vec<String> = ["fn-1", "fn-2"]
        .iter()
        .map(|anchor| {
            let mark = mark_of(&html, anchor);
            attribute_of(&mark, "flm-xnotes-xnote-mark", "id").expect("the mark's id")
        })
        .collect();
    let list = &main[main.find("<div class=\"flm-pres-block flm-xnotes-footnote-list\"").expect("the list")..];
    assert!(list.starts_with(
        "<div class=\"flm-pres-block flm-xnotes-footnote-list\" role=\"doc-endnotes\" \
         data-flm-kind=\"xnotes footnote\">"
    ), "{list}");
    // The title, a level-2 heading out of every table of contents.
    assert!(list.contains(
        "<h2 class=\"flm-pres-block--heading\" data-flm-heading=\"xnotes\" data-flm-level=\"2\" \
         data-flm-toc=\"false\">Footnotes</h2>"
    ), "{list}");
    assert!(list.contains(
        "<ol class=\"flm-pres-list\" data-flm-kind=\"numbered xnotes footnote labeled\">"
    ), "{list}");
    // Each entry carries the xnote's identifier and its mark as its label.
    assert!(list.contains(
        "<li id=\"fn-1\"><span class=\"flm-pres-list-label\"><sup class=\"flm-pres-span \
         flm-xnotes-entry-mark\" data-flm-style=\"superscript\">1</sup></span>"
    ), "{list}");
    assert!(list.contains("<li id=\"fn-2\">"), "{list}");
    // The link back: the last content of the entry's last paragraph, after a
    // space, to the mark's identifier, with its role and accessible name.
    let back = |n: usize| {
        format!(
            " <a class=\"flm-pres-link\" href=\"#{}\" role=\"doc-backlink\" \
             aria-label=\"Back to Footnote {n}\" data-flm-kind=\"xnote-back\">\u{21a9}</a></p>",
            marks[n - 1]
        )
    };
    assert!(list.contains(&format!("by convention.{}", back(1))), "{list}");
    // The entry of two paragraphs: two <p>, the link back in the last.
    let second = &list[list.find("<li id=\"fn-2\">").unwrap()..];
    let second = &second[..second.find("</li>").unwrap()];
    assert_eq!(second.matches("<p class=\"flm-pres-par\">").count(), 2, "{second}");
    assert!(second.contains("<p class=\"flm-pres-par\">First paragraph.</p>"), "{second}");
    assert!(second.contains(&format!("Second paragraph.{}", back(2))), "{second}");
    assert_eq!(second.matches("doc-backlink").count(), 1, "{second}");
}

#[test]
fn a_contributed_kind_s_list_has_its_links_back_too() {
    let registry = common::xnotes::registry();
    let run = clean_with(&registry, &["A\\taddendum{a} B\\footnote{f} C\\taddendum{b}"]);
    let main = main_of(&page(&run));
    assert!(main.contains("data-flm-kind=\"numbered xnotes addendum labeled\""), "{main}");
    assert!(main.contains("aria-label=\"Back to Add. 2\""), "{main}");
    assert!(main.contains("aria-label=\"Back to Footnote 1\""), "{main}");
}

#[test]
fn an_empty_back_link_text_writes_no_link_back() {
    let registry = registry(XnotesConfig { kinds: vec![XnoteKind::footnote().backlink("")] });
    let run = clean_with(&registry, &["A\\footnote{a}"]);
    let main = main_of(&page(&run));
    assert!(main.contains("<li id=\"fn-1\">"), "{main}");
    assert!(!main.contains("doc-backlink"), "{main}");
    assert!(main.contains("<p class=\"flm-pres-par\">a</p>"), "{main}");
}

// ---------------------------------------------------------------------------------------------
// the stylesheet, and no script (tests 54, 57)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_stylesheet_is_required_and_delivered_and_no_script_is() {
    let run = clean(&[WORKED]);
    let rows = definitions(&run);
    let row = rows.iter().find(|(id, _, _)| id == xnotes::STYLE).expect("the stylesheet is required");
    assert_eq!((row.1, row.2.as_str()), (DefinitionKind::Style, xnotes::FEATURE));
    // No script at all: the document holds no mathematics, and the feature
    // ships none.
    assert!(rows.iter().all(|(_, kind, _)| *kind == DefinitionKind::Style), "{rows:?}");
    let page = page(&run);
    let DefContent::Inline(css) = &xnotes::html::stylesheet().content[0] else { panic!("inline") };
    assert!(page.contains(css.trim_end()), "{page}");
    assert!(!page.contains("<script"), "{page}");
    // An in-place kind requires it too, for its brackets.
    let run = clean_with(&registry(in_place()), &["A\\footnote{a}"]);
    assert!(requires_style(&run));
}

#[test]
fn a_document_without_xnotes_requires_no_stylesheet_and_places_nothing() {
    let run = clean(&["No note here.\n\nNor here."]);
    assert!(!requires_style(&run));
    let page = page(&run);
    assert!(!page.contains("xnotes"), "{page}");
    // The feature registers no script definition.
    assert!(std().scripts().iter().all(|(owner, _)| owner.as_str() != xnotes::FEATURE));
}

#[test]
fn the_stylesheet_is_the_list_s_rule_and_the_brackets_every_value_a_custom_property() {
    let style = xnotes::html::stylesheet();
    assert_eq!(style.id.as_str(), xnotes::STYLE);
    let registry = std();
    let registered: Vec<&str> = registry
        .styles()
        .iter()
        .filter(|(owner, _)| owner.as_str() == xnotes::FEATURE)
        .map(|(_, style)| style.id.as_str())
        .collect();
    assert_eq!(registered, [xnotes::STYLE]);
    let DefContent::Inline(css) = &style.content[0] else { panic!("inline") };
    for property in [
        "--flm-xnotes-list-rule",
        "--flm-xnotes-list-size",
        "--flm-xnotes-list-space",
        "--flm-xnotes-inplace-open",
        "--flm-xnotes-inplace-close",
    ] {
        assert!(css.contains(&format!("{property}:")), "{property} is not declared");
        assert!(css.contains(&format!("var({property})")), "{property} is not used");
    }
    // Every declaration outside `:root` takes its value from a property.
    let body = &css[css.find(":root").unwrap()..];
    let rules = &body[body.find('}').unwrap() + 1..];
    for declaration in rules.split(['{', '}', ';']).filter(|part| part.contains(':')) {
        let declaration = declaration.trim();
        if declaration.starts_with('.') || declaration.starts_with("/*") {
            continue;
        }
        assert!(declaration.contains("var(--flm-xnotes-"), "`{declaration}` is no custom property");
    }
    // The three chunk rules — one layer, contract classes, declared
    // properties — are held over every stylesheet the package registers, this
    // one included, by `tests/html_page.rs`.
}

// ---------------------------------------------------------------------------------------------
// in place (test 55)
// ---------------------------------------------------------------------------------------------

#[test]
fn in_place_inline_content_is_one_span_with_the_mark_inside_no_link_no_bracket() {
    let run = clean_with(&registry(in_place()), &[
        "The claim holds in every case. \\footnote{Except the empty one, which we exclude by \
         convention.} The next sentence goes on.",
    ]);
    let html = fragment(&run);
    let span = &html[html.find("<span class=\"flm-pres-span flm-xnotes-xnote-inplace\"").expect("the span")..];
    assert!(span.starts_with(
        "<span class=\"flm-pres-span flm-xnotes-xnote-inplace\" id=\"fn-1\" role=\"doc-footnote\" \
         data-flm-kind=\"xnotes footnote\"><sup class=\"flm-pres-span flm-xnotes-xnote-mark\" id=\"fnref--1"
    ), "{span}");
    assert!(span.contains(
        "data-flm-style=\"superscript\">1</sup>Except the empty one, which we exclude by \
         convention.</span> The next"
    ), "{span}");
    assert!(html.contains("every case. <span"), "{html}");
    assert!(!html.contains("<a "), "{html}");
    assert!(!html.contains('[') && !html.contains(']'), "{html}");
    // Nothing is placed after the content.
    assert!(!main_of(&page(&run)).contains("doc-endnotes"));
    assert_golden("xnotes-inplace-inline.html", &html);
}

#[test]
fn in_place_block_content_is_a_block_between_the_two_halves_of_its_paragraph() {
    let run = clean_with(&registry(in_place()), &[
        "Two paragraphs.\\footnote{First paragraph.\n\nSecond paragraph.} And on.",
    ]);
    let html = fragment(&run);
    let lines: Vec<&str> = html.lines().map(str::trim).collect();
    assert_eq!(lines[0], "<p class=\"flm-pres-par\">Two paragraphs.</p>", "{html}");
    assert_eq!(
        lines[1],
        "<div class=\"flm-pres-block flm-xnotes-xnote-inplace\" id=\"fn-1\" role=\"doc-footnote\" \
         data-flm-kind=\"xnotes footnote\">",
        "{html}"
    );
    assert!(lines[2].starts_with(
        "<p class=\"flm-pres-par\"><sup class=\"flm-pres-span flm-xnotes-xnote-mark\" id=\"fnref--1"
    ), "{html}");
    assert!(lines[2].ends_with("data-flm-style=\"superscript\">1</sup>First paragraph.</p>"), "{html}");
    assert_eq!(lines[3], "<p class=\"flm-pres-par\">Second paragraph.</p>", "{html}");
    assert_eq!(lines[4], "</div>", "{html}");
    assert_eq!(lines[5], "<p class=\"flm-pres-par\">And on.</p>", "{html}");
    assert!(!html.contains('[') && !html.contains(']'), "{html}");
    assert_golden("xnotes-inplace-blocks.html", &html);
}

#[test]
fn an_unnumbered_kind_s_mark_is_an_empty_span_so_that_its_identifier_exists() {
    let run = clean_with(&registry(XnotesConfig::new().kind(sidenote())), &[
        "An aside stands here. \\sidenote{An aside.} And on.",
    ]);
    let html = fragment(&run);
    let span = &html[html.find("<span class=\"flm-pres-span flm-xnotes-xnote-inplace\"").expect("the span")..];
    let id = attribute_of(span, "flm-xnotes-xnote-inplace", "id").expect("the span's id");
    assert!(id.starts_with("sn--1"), "{id}");
    let mark = attribute_of(span, "flm-xnotes-xnote-mark", "id").expect("the mark's id");
    assert!(mark.starts_with("snref--1"), "{mark}");
    assert!(span.starts_with(&format!(
        "<span class=\"flm-pres-span flm-xnotes-xnote-inplace\" id=\"{id}\" role=\"doc-footnote\" \
         data-flm-kind=\"xnotes sidenote\"><sup class=\"flm-pres-span flm-xnotes-xnote-mark\" \
         id=\"{mark}\" data-flm-style=\"superscript\"></sup>An aside.</span> And on."
    )), "{span}");
}

#[test]
fn an_in_place_block_where_no_block_can_stand_is_the_inline_span() {
    let registry = registry(in_place());
    let run = render_under(&registry, &["\\section{A title\\footnote{p\n\nq}}"], &Overrides::none());
    assert_eq!(identifiers(&run), ["flm.blocks.block-in-inline"], "{}", run.diagnostics().render_all());
    let html = fragment(&run);
    assert!(html.contains("<span class=\"flm-pres-span flm-xnotes-xnote-inplace\""), "{html}");
    assert!(html.contains("</sup>p q</span>"), "{html}");
}

// ---------------------------------------------------------------------------------------------
// the title's tag (test 56)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_title_heading_takes_the_tag_heading_tags_maps_xnotes_to() {
    let options = HtmlRenderOptions {
        heading_tags: BTreeMap::from([(Name::from(xnotes::TITLE_HEADING), String::from("h3"))]),
        ..HtmlRenderOptions::default()
    };
    let overrides =
        Overrides { render_options: OptionValues::new().with(options), ..Overrides::none() };
    let run = render_under(&std(), &["A\\footnote{a}"], &overrides);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let main = main_of(&page(&run));
    assert!(main.contains(
        "<h3 class=\"flm-pres-block--heading\" data-flm-heading=\"xnotes\" data-flm-level=\"2\" \
         data-flm-toc=\"false\">Footnotes</h3>"
    ), "{main}");
    // A kind with no title writes no heading.
    let mut kind = XnoteKind::footnote();
    kind.title = None;
    let untitled = registry(XnotesConfig { kinds: vec![kind] });
    let main = main_of(&page(&clean_with(&untitled, &["A\\footnote{a}"])));
    assert!(!main.contains("flm-pres-block--heading"), "{main}");
    assert!(main.contains("role=\"doc-endnotes\""), "{main}");
}

// ---------------------------------------------------------------------------------------------
// a replaced mark style, absent features, the check workflow
// ---------------------------------------------------------------------------------------------

#[test]
fn a_heading_s_identifier_is_encoded_from_its_title_without_the_xnotes_in_it() {
    // The words of an xnote are set elsewhere and are not the title's, labels
    // and all; a title that is only a mark has no words at all, and its
    // identifier is the encoding of the empty title.
    let html = page(&clean(&[
        "\\section{A title with a note\\footnote{A note in a heading's title.}}\n\n\
         \\section{Labeled\\footnote[\\label{fn:l}]{x}}\n\n\
         \\section{\\footnote{Only a mark.}}\n\nText.",
    ]));
    let ids: Vec<Option<String>> = html
        .match_indices("<h1 ")
        .map(|(at, _)| {
            let tag = &html[at..at + html[at..].find('>').expect("a closed tag")];
            tag.split_once(" id=\"").map(|(_, rest)| rest[..rest.find('"').expect("a quote")].to_string())
        })
        .collect();
    assert_eq!(
        ids,
        [Some("sec--A_title_with_a_note"), Some("sec--Labeled"), Some("sec--~cbf29c")].map(|id| id.map(String::from)),
        "{html}"
    );
}

#[test]
fn a_replaced_mark_style_changes_the_mark_on_the_page() {
    let mut style = CounterStyleDecl::new(xnotes::mark_style_name("footnote"));
    style.number_format = NumberTemplate::formats("[#{arabic}]");
    let registry = registry_with(
        StdcoreConfig::new().numbering(NumberingConfig::new().style(style)),
        XnotesConfig::new(),
    );
    let run = clean_with(&registry, &["A\\footnote{a}"]);
    let main = main_of(&page(&run));
    assert!(main.contains("aria-label=\"Footnote 1\" data-flm-kind=\"xnote\">[1]</a></sup>"), "{main}");
    assert!(main.contains("data-flm-style=\"superscript\">[1]</sup></span>"), "{main}");
}

#[test]
fn without_the_references_feature_there_is_no_link_no_link_back_and_no_identifier() {
    let registry = registry_over(vec![
        Arc::new(Blocks::default()) as Arc<dyn FlmFeature>,
        index::feature(),
        refpoints::feature(),
        numbering::feature(),
        Arc::new(XnotesFeature::new(XnotesConfig::new())),
        lowering::feature(),
        pieces::feature(),
        html::feature(),
        check_feature(),
    ]);
    let run = clean_with(&registry, &["A\\footnote{a} B."]);
    let main = main_of(&page(&run));
    assert!(main.contains(
        "A<sup class=\"flm-pres-span flm-xnotes-xnote-mark\" data-flm-style=\"superscript\">1</sup> B."
    ), "{main}");
    assert!(!main.contains("<a "), "{main}");
    assert!(!main.contains(" id=\""), "{main}");
    assert!(main.contains("<li><span class=\"flm-pres-list-label\">"), "{main}");
}

#[test]
fn the_check_workflow_writes_no_page_and_reports_nothing() {
    let registry = std();
    let run = run_workflow(&registry, CHECK_WORKFLOW, &[WORKED], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let plan = plan(&registry, CHECK_WORKFLOW, &Overrides::none());
    assert!(plan.stage_names().iter().all(|stage| !stage.as_str().starts_with("flm.html.")));
}

// ---------------------------------------------------------------------------------------------
// execution settings, and documents that must neither panic nor loop
// ---------------------------------------------------------------------------------------------

#[test]
fn the_page_is_the_same_bytes_sequentially_and_in_parallel() {
    let registry = std();
    let texts = [WORKED, "C\\footnote[\\label{fn:c}]{c} \\ref{fn:c}", "D\\footnote{d}"];
    let doc = common::pipeline::doc(&registry, &texts);
    let shared = plan(&registry, html::WORKFLOW, &Overrides::none());
    let under = |options: &RunOptions| {
        page(&common::pipeline::run_with(&doc, &shared, Externals::none(), options))
    };
    let sequential = under(&RunOptions::sequential());
    assert_eq!(under(&RunOptions::new()), sequential);
    assert_eq!(under(&RunOptions::new()), sequential);
}

#[test]
fn hard_documents_neither_panic_nor_loop_on_the_page() {
    let deep = format!("{}x{}", "\\footnote{".repeat(10), "}".repeat(10));
    for registry in [std(), registry(in_place().kind(sidenote()))] {
        for document in [
            deep.as_str(),
            "\\footnote[]{}",
            "\\footnote[]{x}",
            "\\footnote{}",
            "\\footnote",
            "\\footnote[\\footnote{x}]{y}",
            "\\section{\\footnote{a\n\nb}}",
            "\\(\\text{\\footnote{a\n\nb}}\\)",
            "\\begin{itemize}\\item \\footnote{x\\begin{enumerate}\\item y\\end{enumerate}}\\end{itemize}",
            "\\footnote{\\begin{itemize}\\item y\\end{itemize}}",
            "\\textbf{\\footnote{p\n\nq}}",
            "\\ref{fn:a}\\footnote[\\label{fn:a}\\label{fn:a}]{x}",
        ] {
            let run = render_under(&registry, &[document], &Overrides::none());
            let _ = page(&run);
        }
    }
}

#[test]
fn notes_nested_three_deep_have_unique_identifiers_and_each_links_back_to_its_own_mark() {
    let run = clean(&["A\\footnote{one\\footnote{two\\footnote{three}} end} B"]);
    let main = main_of(&page(&run));
    // Every identifier on the page is unique.
    let ids: Vec<&str> = main
        .match_indices(" id=\"")
        .map(|(at, key)| {
            let start = at + key.len();
            &main[start..start + main[start..].find('"').expect("the id closes")]
        })
        .collect();
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), ids.len(), "{ids:?}");
    // The three notes, numbered in document order, each an entry of its own
    // whose one link back goes to its own mark.
    let entry = |n: usize| {
        let open = format!("<li id=\"fn-{n}\">");
        assert_eq!(main.matches(&open).count(), 1, "{main}");
        let start = main.find(&open).unwrap();
        // An entry ends where the next one starts, or where the list ends.
        let rest = &main[start + open.len()..];
        let end = rest.find("<li id=\"fn-").or_else(|| rest.find("</ol>")).expect("the entry ends");
        String::from(&rest[..end])
    };
    for n in 1..=3 {
        let mark = mark_of(&main, &format!("fn-{n}"));
        let mark_id = attribute_of(&mark, "flm-xnotes-xnote-mark", "id").expect("the mark's id");
        let entry = entry(n);
        assert_eq!(entry.matches("doc-backlink").count(), 1, "{entry}");
        assert!(entry.contains(&format!("href=\"#{mark_id}\" role=\"doc-backlink\"")), "{entry}");
    }
    // Each inner note's mark stands in the entry of the note holding it.
    assert!(entry(1).starts_with("<span class=\"flm-pres-list-label\">"));
    assert!(entry(1).contains("href=\"#fn-2\""), "{}", entry(1));
    assert!(entry(1).contains(" end"), "{}", entry(1));
    assert!(entry(2).contains("href=\"#fn-3\""), "{}", entry(2));
    assert!(!entry(3).contains("href=\"#fn-"), "{}", entry(3));
}

// ---------------------------------------------------------------------------------------------
// an xnote in content shown again (tests 1–3 of the copies of shown content)
// ---------------------------------------------------------------------------------------------

/// The mark a reference shows of a copied footnote numbered 1: a superscript
/// with no identifier, its link to the note standing inside the reference's
/// own link and so written by the page as its text — a span with the link's
/// class and kind, and no address, document role or accessible name.
const SHOWN_MARK: &str = "<sup class=\"flm-pres-span flm-xnotes-xnote-mark\" data-flm-style=\"superscript\">\
    <span class=\"flm-pres-link\" data-flm-kind=\"xnote\">1</span></sup>";

#[test]
fn a_reference_to_a_heading_holding_an_xnote_shows_the_mark_its_link_written_as_its_text() {
    // The mark's link inside the reference's link is written as its text by
    // the page, and nothing is reported (`clean`).
    let document = "\\section*{Intro\\footnote{N.}}\\label{sec:a}\n\nSee \\ref{sec:a}.";
    let run = clean(&[document]);
    let page = page(&run);
    let main = main_of(&page);
    // The reference is a link to the heading, holding the title and the mark.
    assert!(
        main.contains(&format!("See <a class=\"flm-pres-link\" href=\"#sec-a\" data-flm-kind=\"ref\">Intro{SHOWN_MARK}</a>.")),
        "{main}"
    );
    // The mark where the note stands keeps its identifier and its link; the
    // note is listed once.
    let original = mark_of(&main, "fn-1");
    assert!(original.contains(" id=\"fnref--1"), "{original}");
    assert_eq!(main.matches("href=\"#fn-1\"").count(), 1, "{main}");
    assert_eq!(main.matches("<li id=\"fn-").count(), 1, "{main}");
    assert_golden("xnotes-shown-again.page.html", &page);
}

#[test]
fn a_labeled_xnote_in_a_title_shown_again_renders_and_a_reference_to_it_names_the_note() {
    let document = "\\section*{Intro\\footnote[\\label{fn:a}]{N.}}\\label{sec:a}\n\n\
        See \\ref{sec:a} and \\ref{fn:a}.";
    let run = clean(&[document]);
    let main = main_of(&page(&run));
    assert!(main.contains(&format!("Intro{SHOWN_MARK}</a> and ")), "{main}");
    assert!(main.contains("href=\"#fn-1\" data-flm-kind=\"ref\">Footnote&#xA0;1</a>"), "{main}");
    assert_eq!(main.matches("<li id=\"fn-").count(), 1, "{main}");
}

#[test]
fn two_references_to_a_float_s_caption_show_two_marks_of_one_note() {
    let document = "\\begin{figure}\\nonumber\\includegraphics{a.png}\\caption{A photo\\footnote{N.}}\
        \\label{figure:a}\\end{figure}\n\nSee \\ref{figure:a} and \\ref{figure:a}.";
    let run = clean(&[document]);
    let main = main_of(&page(&run));
    assert_eq!(main.matches(&format!("A photo{SHOWN_MARK}</a>")).count(), 2, "{main}");
    assert_eq!(main.matches("id=\"fnref--").count(), 1, "{main}");
    assert_eq!(main.matches("<li id=\"fn-").count(), 1, "{main}");
}

#[test]
fn content_read_by_the_key_value_reader_shown_again_shows_the_note_s_mark() {
    // A float's alternative text and a list item's tag, each read by the
    // key-value reader of its bracket: a reference shows the mark of the note
    // either holds, with no identifier; each note listed once.
    let document = "\\begin{figure}\\nonumber\\includegraphics{a.png}\\caption[alt=B\\footnote{Q.}]{B}\
        \\label{figure:b}\\end{figure}\n\n\\begin{enumerate}\\item[tag=T\\footnote{x}]\\label{item:a} One.\\end{enumerate}\n\n\
        See \\ref{figure:b} and \\ref{item:a}.";
    let run = clean(&[document]);
    let main = main_of(&page(&run));
    let shown_second = SHOWN_MARK.replace(">1<", ">2<");
    assert!(main.contains(&format!("B{SHOWN_MARK}</a>")), "{main}");
    assert!(main.contains(&format!("T{shown_second}</a>")), "{main}");
    // The tag's mark where it stands carries the one identifier; the
    // alternative text is not written where it stands, so its note's mark is
    // the one the reference shows.
    assert_eq!(main.matches("id=\"fnref--").count(), 1, "{main}");
    assert_eq!(main.matches("<li id=\"fn-").count(), 2, "{main}");
}

/// Ten user definitions `\nesta` … `\nestj`, each adding a note to the one
/// before it.
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

#[test]
fn content_shown_again_holding_xnotes_neither_panics_nor_loops() {
    // A heading whose title holds a note holding a reference to the heading.
    let own = clean(&["\\section*{Intro\\footnote{See \\ref{sec:a}.}}\\label{sec:a}\n\nSee \\ref{sec:a}."]);
    let main = main_of(&page(&own));
    assert!(main.contains(&format!("See <a class=\"flm-pres-link\" href=\"#sec-a\" data-flm-kind=\"ref\">Intro{SHOWN_MARK}</a>.")), "{main}");
    // A note inside a note inside a title shown again: one mark, the outer
    // note's.
    let nested = clean(&["\\section*{Intro\\footnote{a\\footnote{b}}}\\label{sec:a}\n\nSee \\ref{sec:a}."]);
    let main = main_of(&page(&nested));
    assert!(main.contains(&format!("Intro{SHOWN_MARK}</a>.")), "{main}");
    assert_eq!(main.matches("<li id=\"fn-").count(), 2, "{main}");
    // Titles shown ten references deep, each holding a note.
    let mut document = String::from("\\section*{T0\\footnote{n0}}\\label{sec:t0}\n\n");
    for level in 1..=10 {
        document.push_str(&format!(
            "\\section*{{T{level} \\ref{{sec:t{}}}\\footnote{{n{level}}}}}\\label{{sec:t{level}}}\n\n",
            level - 1
        ));
    }
    document.push_str("See \\ref{sec:t10}.");
    // Each title holds a reference, so every title shown again holds links
    // inside the reference's link, which the page writes as their text: one
    // `<a>` never inside another, and nothing reported.
    for registry in [std(), registry(in_place())] {
        let run = render_under(&registry, &[&document], &Overrides::none());
        assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
        let main = main_of(&page(&run));
        assert!(main.contains("See <a class=\"flm-pres-link\" href=\"#sec-t10\""));
        let mut open = 0;
        for (_, tag) in main.match_indices("<a ").chain(main.match_indices("</a>")).collect::<BTreeMap<_, _>>() {
            open += if tag == "</a>" { -1 } else { 1 };
            assert!((0..=1).contains(&open), "{main}");
        }
    }
    // A title copied ten definitions deep: ten nested user definitions, each
    // adding a note, in a starred title shown by a reference. Eleven notes,
    // each shown again as its mark with no identifier.
    let definitions = registry_with(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(nested_definitions())),
        XnotesConfig::new(),
    );
    let run = clean_with(&definitions, &["\\section*{T\\footnote{t} \\nestj}\\label{sec:a}\n\nSee \\ref{sec:a}."]);
    let main = main_of(&page(&run));
    assert_eq!(main.matches("<li id=\"fn-").count(), 11, "{main}");
    assert_eq!(main.matches("id=\"fnref--").count(), 11, "{main}");
    assert_eq!(main.matches("flm-xnotes-xnote-mark\" data-flm-style=\"superscript\">").count(), 11, "{main}");
}
