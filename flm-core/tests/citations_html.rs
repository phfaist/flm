//! The `citations` feature on the `html` backend: the marks with their links
//! and document roles, the bibliography block, the feature's stylesheet, the
//! space before an attached mark, both forms and the in-place placement.
//!
//! Every golden this suite writes under `tests/goldens/html/` is parsed by
//! `tests/html_validity.rs` in the same test run, which also holds that every
//! link into the page names an identifier the page has; none is excused.
//!
//! Every registry is the standard package, its citations feature under the
//! configuration a test names, with the test provider answering every key
//! with the key itself as text, unless a test names its providers.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::stdcore::backends::html::{self, DEFINITIONS_TABLE};
use flm_core::stdcore::features::citations::{
    self, CitationPlacement, CitationProvider, CitationsConfig, MarkForm,
};
use flm_core::stdcore::StdcoreConfig;

use common::html::{assert_golden, page};
use common::citations::FromMap;
use common::stdcore::{identifiers, run_workflow};

/// The no-break space as the page writes it, a character reference.
const NBSP: &str = "&#xA0;";

/// The space before an attached mark, as the page writes it: the no-break
/// space in a span of its own.
const SPACE: &str = "<span class=\"flm-pres-span flm-citations-space\">&#xA0;</span>";

// --- registries and runs ---------------------------------------------------------------------

/// The standard package under `config`, the test provider answering every
/// key.
fn registry_of(config: CitationsConfig) -> Arc<FlmRegistry> {
    common::citations::registry(StdcoreConfig::new().citations(config))
}

/// The standard package, the default configuration.
fn registry() -> Arc<FlmRegistry> {
    registry_of(CitationsConfig::new())
}

/// The `html` run of `texts`, one tree each, under `registry`.
fn render_on(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Run {
    run_workflow(registry, html::WORKFLOW, texts, &Overrides::none())
}

/// The standard package under `config`, the provider answering the keys of
/// [`WORKS`] and no other.
fn registry_of_works(config: CitationsConfig) -> Arc<FlmRegistry> {
    let provider: Arc<dyn CitationProvider> = Arc::new(FromMap::of(WORKS));
    common::citations::registry_with(StdcoreConfig::new().citations(config), vec![provider])
}

/// The entries the provider of [`registry_of_works`] knows, as FLM source.
const WORKS: &[(&str, &str)] = &[
    ("knuth84", "D. Knuth, \\emph{The Art of Computer Programming}, 1984"),
    ("lamport94", "L. Lamport, \\emph{LaTeX: A Document Preparation System}, 1994"),
    ("shor95", "P. Shor, Scheme for reducing decoherence, 1995"),
    ("gottesman97", "D. Gottesman, Stabilizer codes, 1997"),
];

/// Whether the run's document requires the stylesheet `flm.citations.style`.
fn requires_style(run: &Run) -> bool {
    run.table(DEFINITIONS_TABLE)
        .expect("the definitions table is declared")
        .expect("the definitions table is written")
        .definitions
        .iter()
        .any(|row| row.id.as_str() == citations::html::STYLE)
}

/// The page of `texts` under `registry`; the run must report nothing.
fn clean_page(registry: &Arc<FlmRegistry>, texts: &[&str]) -> String {
    let run = render_on(registry, texts);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    page(&run)
}

/// The part of `page` inside `<main …>` and `</main>`: the trees, then the
/// pieces placed after them.
fn main_of(page: &str) -> String {
    let start = page.find("<main").expect("the page has a <main>");
    let start = start + page[start..].find('>').expect("the <main> tag closes") + 1;
    let end = page.find("</main>").expect("the page closes its <main>");
    String::from(&page[start..end])
}

// --- the document roles ----------------------------------------------------------------------

#[test]
fn each_number_links_to_its_entry_as_a_bibliography_reference_and_the_list_is_a_bibliography() {
    let main = main_of(&clean_page(&registry(), &["Codes \\cite{a,b}."]));
    assert!(
        main.contains(
            "<a class=\"flm-pres-link\" href=\"#citation-1\" role=\"doc-biblioref\" \
             data-flm-kind=\"cite\">1</a>"
        ),
        "{main}"
    );
    assert!(
        main.contains(
            "<div class=\"flm-pres-block flm-citations-bibliography\" role=\"doc-bibliography\""
        ),
        "{main}"
    );
    // The entries carry no document role.
    assert!(!main.contains("doc-biblioentry"), "{main}");
    assert_eq!(main.matches("role=\"doc-").count(), 3, "{main}");
}

// --- the stylesheet --------------------------------------------------------------------------

#[test]
fn a_page_carries_the_stylesheet_only_where_it_holds_a_citation() {
    for document in ["Text \\cite{a}.", "See \\R\\cite{a}.", "See \\R[I]\\cite{a}."] {
        let run = render_on(&registry(), &[document]);
        assert!(requires_style(&run), "{document}");
        assert!(page(&run).contains("/* flm.citations.style */"), "{document}");
    }
    let run = render_on(&registry(), &["No citation here.\\footnote{Nor here.}"]);
    assert!(!requires_style(&run));
    assert!(!page(&run).contains("flm.citations.style"));
    // Under the in-place placement too.
    let in_place = CitationsConfig { placement: CitationPlacement::InPlace, ..CitationsConfig::new() };
    assert!(requires_style(&render_on(&registry_of(in_place), &["Text \\cite{a}."])));
}

// --- the worked page (tests 5, 7, 9, 13, 16) -------------------------------------------------

/// The worked example: attached and standalone marks, notes, `\R` in its
/// three forms, a citation in a footnote and in a heading a reference
/// shows, and a key no provider knows.
const WORKED: &str = "\\section{Results \\cite{shor95}}\\label{sec:results}\n\n\
Codes of this family are well studied \\cite{knuth84,lamport94}. The bound is tight \
\\cite{shor95}\\cite[Thm 4]{knuth84}\\cite{lamport94}\\cite[Thm 4]{gottesman97}. \
A work first cited in a footnote.\\footnote{See \\cite{gottesman97}.}\n\n\
As shown in \\R\\cite{lamport94} and \\R[S]\\cite{knuth84,shor95}, the claim holds (see, for \
instance, \\R[I]\\cite{lamport94,shor95}); a mark standing after a parenthesis shows its \
space (\\cite{knuth84}). See \\ref{sec:results}. A work no provider knows \\cite{unknown}.";

#[test]
fn the_worked_example_s_page_is_the_golden() {
    let run = render_on(&registry_of_works(CitationsConfig::new()), &[WORKED]);
    assert_eq!(identifiers(&run), ["flm.citations.no-entry"], "{}", run.diagnostics().render_all());
    let page = page(&run);
    let main = main_of(&page);
    // Test 16: the mark, its delimiters outside the links.
    assert!(
        main.contains(&format!(
            "studied{SPACE}<span class=\"flm-pres-span flm-citations-mark\" data-flm-kind=\"attached\">\
             [<a class=\"flm-pres-link\" href=\"#citation-2\" role=\"doc-biblioref\" \
             data-flm-kind=\"cite\">2</a>,<a class=\"flm-pres-link\" href=\"#citation-3\" \
             role=\"doc-biblioref\" data-flm-kind=\"cite\">3</a>]</span>."
        )),
        "{main}"
    );
    // Test 5: the space before an attached mark is the character, even after
    // a parenthesis; a standalone mark has none, and the author's space
    // before the word of `\R` stands.
    assert!(main.contains(&format!("space ({SPACE}<span")), "{main}");
    assert!(main.contains("instance, <span class=\"flm-pres-span flm-citations-mark\" data-flm-kind=\"standalone\">"), "{main}");
    assert!(main.contains("As shown in <span class=\"flm-pres-span flm-citations-word\">ref."), "{main}");
    assert!(main.contains("<span class=\"flm-pres-span flm-citations-word\">Refs."), "{main}");
    // Test 13: inside the reference's link, the mark's links are written as
    // their text.
    let reference = &main[main.find("See <a").expect("the reference") + "See <a".len()..];
    let reference = &reference[..reference.find("</a>").expect("the reference's link closes")];
    assert!(!reference.contains("<a "), "{reference}");
    assert!(
        reference.contains(
            "<span class=\"flm-pres-span flm-citations-mark\" data-flm-kind=\"attached\">\
             [<span class=\"flm-pres-link\" data-flm-kind=\"cite\">1</span>]</span>"
        ),
        "{reference}"
    );
    // The bibliography after the footnotes, its entries identified.
    let at = |text: &str| main.find(text).unwrap_or_else(|| panic!("{text} in {main}"));
    assert!(at("role=\"doc-endnotes\"") < at("role=\"doc-bibliography\""), "{main}");
    for n in 1..=5 {
        assert!(main.contains(&format!("<li id=\"citation-{n}\">")), "{main}");
    }
    assert!(main.contains("data-flm-condition=\"flm.citations.no-entry\""), "{main}");
    assert_golden("citations-worked.page.html", &page);
}

// --- the superscript form (test 6) -----------------------------------------------------------

#[test]
fn under_the_superscript_form_an_attached_mark_is_raised_with_no_space_and_a_standalone_one_is_not()
{
    let config = CitationsConfig { mark: MarkForm::Superscript, ..CitationsConfig::new() };
    let registry = registry_of_works(config);
    let page = clean_page(
        &registry,
        &["Codes of this family are well studied \\cite{knuth84,lamport94,shor95}. The bound is \
           tight \\cite{shor95}\\cite[Thm 4]{knuth84}. As shown in \\R\\cite{knuth84,lamport94} \
           (see, for instance, \\R[I]\\cite{lamport94,shor95})."],
    );
    let main = main_of(&page);
    assert!(
        main.contains(
            "studied<sup class=\"flm-pres-span flm-citations-mark\" \
             data-flm-style=\"superscript\" data-flm-kind=\"attached\"><a class=\"flm-pres-link\" href=\"#citation-1\" \
             role=\"doc-biblioref\" data-flm-kind=\"cite\">1</a>–"
        ),
        "{main}"
    );
    assert!(
        main.contains(&format!(
            "refs.{NBSP}</span><span class=\"flm-pres-span flm-citations-mark\" \
             data-flm-kind=\"standalone\"><a"
        )),
        "{main}"
    );
    assert!(!main.contains('['), "{main}");
    assert_golden("citations-superscript.page.html", &page);
}

// --- the in-place placement (test 12) --------------------------------------------------------

#[test]
fn in_place_a_mark_holds_the_entries_with_no_link_and_no_list_is_placed() {
    let mut pages = Vec::new();
    for mark in [MarkForm::Bracketed, MarkForm::Superscript] {
        let config = CitationsConfig { placement: CitationPlacement::InPlace, mark, ..CitationsConfig::new() };
        let run = render_on(
            &registry_of_works(config),
            &["The bound is tight \\cite{knuth84}\\cite[Thm 4]{lamport94}. As shown in \
               \\R\\cite{shor95,gottesman97}. A work no provider knows \\cite{unknown}."],
        );
        assert_eq!(identifiers(&run), ["flm.citations.no-entry"], "{}", run.diagnostics().render_all());
        let page = page(&run);
        let main = main_of(&page);
        assert!(
            main.contains(&format!(
                "tight{SPACE}<span class=\"flm-pres-span flm-citations-mark\" \
                 data-flm-kind=\"attached in-place\">[D. Knuth, <em"
            )),
            "{main}"
        );
        assert!(!main.contains("<a "), "{main}");
        assert!(!main.contains("<sup"), "{main}");
        assert!(!main.contains("doc-bibliography"), "{main}");
        assert!(main.contains("[<span class=\"flm-pres-dropped\""), "{main}");
        if mark == MarkForm::Bracketed {
            assert_golden("citations-in-place.page.html", &page);
        }
        pages.push(page);
    }
    // The form beside the in-place placement changes nothing.
    assert_eq!(pages[0], pages[1]);
}
