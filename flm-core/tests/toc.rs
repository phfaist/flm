//! The table of contents (`flm_core::stdcore::features::toc`): the table
//! `flm.toc.entries` (which headings, in which order, at which level, with
//! which point; the setting `depth`), the piece `flm.toc` on the text and HTML
//! outputs (the nesting, the numbers, the links, the stylesheet), the entries
//! of headings written with `alt=`, and the recomposed LaTeX unchanged.
//!
//! The tests build their registries from the `stdcore` package, as
//! `tests/meta.rs` and `tests/html_pieces.rs` do.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Run, RunOptions};
use flm_core::dump::golden::{assert_json_golden, assert_text_golden};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::html::{self, DEFINITIONS_TABLE, PIECES_TABLE};
use flm_core::stdcore::backends::pieces::PiecesRenderOptions;
use flm_core::stdcore::backends::{latex_recomposed, text};
use flm_core::stdcore::features::headings::{HeadingDecl, HeadingNumbering, HeadingsConfig};
use flm_core::stdcore::features::toc::{self, ENTRIES, TocEntries, TocOptions};
use techy::error::Severity;

use common::stdcore::{diagnostic_lines, identifiers, registry, reports, run_workflow};

// --- helpers -------------------------------------------------------------------------------

/// The standard package with every heading kind numbered.
fn numbered() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new().headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered)))
}

/// The standard package as configured by default: no heading is numbered.
fn unnumbered() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// Overrides setting `options` on the render request.
fn render_options(options: OptionValues) -> Overrides {
    Overrides { render_options: options, ..Overrides::none() }
}

/// Overrides that place the table of contents.
fn with_toc() -> Overrides {
    render_options(OptionValues::new().with(PiecesRenderOptions::default().include(toc::PIECE)))
}

/// A run demanding the table of entries alone, over one tree per text, under
/// the document options `options`.
fn entries_run(registry: &Arc<FlmRegistry>, texts: &[&str], options: OptionValues) -> Run {
    let doc = common::pipeline::doc_with_options(registry, texts, options);
    let plan = common::pipeline::plan(registry, &[ENTRIES.name()]);
    common::pipeline::run(&doc, &plan, &RunOptions::sequential())
}

/// The table of entries of `run`.
fn table(run: &Run) -> &TocEntries {
    run.table(ENTRIES).expect("declared").unwrap_or_else(|| panic!("not written:\n{}", diagnostic_lines(run)))
}

/// Each entry of the table of the document `text`, under the depth `depth`
/// (the default where `None`): its level, and whether it has a point.
fn levels(registry: &Arc<FlmRegistry>, text: &str, depth: Option<u32>) -> Vec<(u32, bool)> {
    let options = depth.map_or_else(OptionValues::new, |depth| OptionValues::new().with(TocOptions { depth }));
    let run = entries_run(registry, &[text], options);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", diagnostic_lines(&run));
    table(&run).entries.iter().map(|entry| (entry.level, entry.point.is_some())).collect()
}

/// The printed text of the `text` workflow over `texts` under `overrides`;
/// the run must report nothing.
fn printed(registry: &Arc<FlmRegistry>, texts: &[&str], overrides: &Overrides) -> String {
    let run = run_workflow(registry, text::WORKFLOW, texts, overrides);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", diagnostic_lines(&run));
    common::lowering::text(&run)
}

/// The run of the `html` workflow over `texts` under `overrides`.
fn html_run(registry: &Arc<FlmRegistry>, texts: &[&str], overrides: &Overrides) -> Run {
    run_workflow(registry, html::WORKFLOW, texts, overrides)
}

/// The page of the `html` workflow over `texts` with the table of contents
/// placed; the run must report nothing.
fn page(registry: &Arc<FlmRegistry>, texts: &[&str]) -> String {
    let run = html_run(registry, texts, &with_toc());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", diagnostic_lines(&run));
    common::html::page(&run)
}

/// The `<nav>` of `page`, from its opening tag to its closing one.
fn nav(page: &str) -> &str {
    let start = page.find("<nav").unwrap_or_else(|| panic!("a <nav> in {page}"));
    let end = page.find("</nav>").expect("the <nav> closes") + "</nav>".len();
    &page[start..end]
}

/// The path of the golden `tests/goldens/<folder>/<name>`.
fn golden(folder: &str, name: &str) -> std::path::PathBuf {
    let mut path = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");
    path.push("goldens");
    path.push(folder);
    path.push(name);
    path
}

const OUTLINE: &str = "\\section{One}\nA.\n\n\\subsection[alt={Second}]{Two}\nB.\n\n\\subsubsection{Three}\nC.\n\n\\paragraph{Four}\nD.\n\n\\section{Five}\nE.";

// --- the table of entries ------------------------------------------------------------------

#[test]
fn the_table_lists_the_headings_in_order_with_their_levels_and_points_to_the_depth() {
    let registry = numbered();
    let run = entries_run(&registry, &[OUTLINE], OptionValues::new());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", diagnostic_lines(&run));
    let entries = &table(&run).entries;
    // The default depth, 3, leaves the paragraph out.
    assert_eq!(entries.iter().map(|entry| entry.level).collect::<Vec<_>>(), [1, 2, 3, 1]);
    // Each entry's point is its heading's first point.
    for entry in entries {
        let point = entry.point.expect("a point");
        assert_eq!((point.addr, point.index), (entry.addr, 0));
    }
    // In document order.
    let nodes: Vec<_> = entries.iter().map(|entry| entry.addr.node).collect();
    assert!(nodes.windows(2).all(|pair| pair[0] < pair[1]), "{nodes:?}");
    assert_json_golden(golden("dump", "toc-entries.json"), &common::stdcore::projection(&run, ENTRIES));
}

#[test]
fn the_depth_chooses_the_deepest_level_listed() {
    let registry = numbered();
    let listed = |depth| levels(&registry, OUTLINE, depth).into_iter().map(|(level, _)| level).collect::<Vec<_>>();
    assert_eq!(listed(Some(0)), Vec::<u32>::new());
    assert_eq!(listed(Some(1)), [1, 1]);
    assert_eq!(listed(None), [1, 2, 3, 1]);
    assert_eq!(listed(Some(3)), [1, 2, 3, 1]);
    assert_eq!(listed(Some(5)), [1, 2, 3, 4, 1]);
}

#[test]
fn a_starred_heading_is_listed_and_toc_false_and_a_kind_that_skips_the_contents_are_not() {
    let mut kinds = HeadingsConfig::new().headings;
    kinds.push(HeadingDecl::new("aside", 2).no_counter().skip_toc(true));
    let registry = registry(StdcoreConfig::new().headings(HeadingsConfig::new().headings(kinds)));
    let text = "\\section*{Starred}\n\n\\section[toc=false]{Hidden}\n\n\\aside{Aside}\n\n\\aside[toc=true]{Aside}\n\n\\section{Kept}";
    let run = entries_run(&registry, &[text], OptionValues::new());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", diagnostic_lines(&run));
    let projected = format!("{:?}", common::stdcore::projection(&run, ENTRIES));
    assert_eq!(table(&run).entries.len(), 2, "{projected}");
    assert!(!projected.contains("Hidden") && !projected.contains("Aside"), "{projected}");
}

#[test]
fn a_document_with_no_heading_has_an_empty_table_and_places_nothing() {
    let registry = numbered();
    assert_eq!(levels(&registry, "Only text.", None), []);
    let out = printed(&registry, &["Only text."], &with_toc());
    assert_eq!(out, "Only text.\n");
    let run = html_run(&registry, &["Only text."], &with_toc());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", diagnostic_lines(&run));
    assert!(!common::html::page(&run).contains("<nav"));
}

#[test]
fn a_heading_input_s_heading_comes_where_the_input_is_placed() {
    use flm_core::lang::{FlmOrigin, FlmSource, IncludedFile, InputHeading, ParseOptions};
    use flm_core::model::assemble::assemble;
    use flm_core::model::doc::{Doc, FragmentLayout};

    /// One heading input, `two.flm`, held in memory.
    struct One;
    impl flm_core::lang::InputResolver for One {
        fn resolve(
            &self,
            reference: &str,
            _including: Option<&str>,
            _parsing: &flm_core::lang::FlmParsing,
        ) -> Result<IncludedFile, flm_core::lang::InputError> {
            assert_eq!(reference, "two.flm");
            Ok(IncludedFile {
                name: reference.to_string(),
                text: "\\subsection{Inside}\nText.".to_string(),
                first_line: 4,
                heading: Some(InputHeading {
                    command: "section".to_string(),
                    title: "Two".to_string(),
                    title_at: Some((2, 8)),
                    label: None,
                    numbered: true,
                }),
                ..IncludedFile::default()
            })
        }
    }
    let registry = numbered();
    let source = Arc::new(FlmSource::new("\\section{One}\n\n\\input{two.flm}\n\n\\section{Three}").with_origin(FlmOrigin::new("main.flm")));
    let options = ParseOptions { input_resolver: Some(Arc::new(One)), ..ParseOptions::default() };
    let content = assemble(registry.lang(), source, FragmentLayout::default(), &options).expect("a parse");
    let doc = Arc::new(Doc::with_options(content, OptionValues::new()));
    let plan = common::pipeline::plan(&registry, &[ENTRIES.name()]);
    let run = common::pipeline::run(&doc, &plan, &RunOptions::sequential());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", diagnostic_lines(&run));
    let projected = format!("{:?}", common::stdcore::projection(&run, ENTRIES));
    let at = |needle: &str| projected.find(needle).unwrap_or_else(|| panic!("{needle} in {projected}"));
    assert_eq!(table(&run).entries.iter().map(|entry| entry.level).collect::<Vec<_>>(), [1, 1, 2, 1]);
    assert!(at("\"argument title\"") < projected.len());
    // The input's heading and its subsection stand between the two sections
    // of the primary file: the table holds the trees' nodes in document
    // order, which the trees' own node order does not give.
    let trees: Vec<_> = table(&run).entries.iter().map(|entry| entry.addr.tree).collect();
    assert_eq!(trees[0], trees[3], "{projected}");
    assert_ne!(trees[0], trees[2], "{projected}");

    // The text output lists the input's heading at its place.
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &with_toc());
    let run = common::pipeline::run(&doc, &plan, &RunOptions::sequential());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", diagnostic_lines(&run));
    let out = common::lowering::text(&run);
    let toc = &out[..out.find("1. One").map_or(out.len(), |at| at)];
    let at = |needle: &str| toc.find(needle).unwrap_or_else(|| panic!("{needle} in {out}"));
    assert!(at("One") < at("Two") && at("Two") < at("Inside") && at("Inside") < at("Three"), "{out}");
}

// --- the piece on the text output ----------------------------------------------------------

#[test]
fn the_text_output_prints_the_nested_lists_before_the_content() {
    let registry = numbered();
    let out = printed(&registry, &[OUTLINE], &with_toc());
    assert_text_golden(golden("stdcore", "toc.txt"), &out);
    // Without the include, nothing of the table of contents is printed.
    let without = printed(&registry, &[OUTLINE], &Overrides::none());
    assert!(out.ends_with(&without), "{out}\n---\n{without}");
    assert!(out.len() > without.len());
}

#[test]
fn a_level_gap_nests_one_list_deeper_not_two() {
    let registry = unnumbered();
    let run = html_run(&registry, &["\\section{One}\n\n\\subsubsection{Deep}\n\n\\subsection{Mid}"], &with_toc());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", diagnostic_lines(&run));
    let nav = nav(&common::html::page(&run)).to_string();
    // Two lists: the top one, and one inside the entry `One` holding both
    // `Deep` and `Mid`.
    assert_eq!(nav.matches("<ul").count(), 2, "{nav}");
    let deep = nav.find("Deep").expect("Deep");
    let mid = nav.find("Mid").expect("Mid");
    assert!(!nav[deep..mid].contains("<ul") && !nav[deep..mid].contains("</ul>"), "{nav}");
}

#[test]
fn a_listed_subsection_under_a_section_left_out_is_nested_among_the_kept_headings() {
    let registry = numbered();
    let text = "\\section{One}\n\n\\section[toc=false]{Hidden}\n\n\\subsection{Sub}\n\n\\section{Three}";
    let run = html_run(&registry, &[text], &with_toc());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", diagnostic_lines(&run));
    let nav = nav(&common::html::page(&run)).to_string();
    assert!(!nav.contains("Hidden"), "{nav}");
    // `Sub` is nested in `One`, the section kept before it.
    let one = nav.find("One").expect("One");
    let sub = nav.find("Sub").expect("Sub");
    let three = nav.find("Three").expect("Three");
    assert!(nav[one..sub].contains("<ul") && nav[sub..three].contains("</ul>"), "{nav}");
}

// --- the piece on the HTML output ----------------------------------------------------------

#[test]
fn the_html_page_writes_a_nav_of_nested_lists_with_numbers_and_links() {
    let registry = numbered();
    let page = page(&registry, &["\\section{One}\n\n\\subsection{Two}"]);
    assert_eq!(
        nav(&page),
        "<nav class=\"flm-pres-block flm-toc\" role=\"doc-toc\" data-flm-kind=\"toc\">\n  <ul class=\"flm-pres-list\" data-flm-kind=\"unnumbered toc\">\n    <li><a class=\"flm-pres-link\" href=\"#sec--One\" data-flm-kind=\"toc\"><span class=\"flm-pres-span flm-toc-entry-number\">1</span> One</a>\n      <ul class=\"flm-pres-list\" data-flm-kind=\"unnumbered toc\">\n        <li><a class=\"flm-pres-link\" href=\"#sec--Two\" data-flm-kind=\"toc\"><span class=\"flm-pres-span flm-toc-entry-number\">1.1</span> Two</a></li>\n      </ul>\n    </li>\n  </ul>\n</nav>",
        "{page}"
    );
    // The stylesheet is in the page's definitions.
    assert!(page.contains("data-flm-def=\"flm.toc.style\""), "{page}");
    common::html::assert_golden("toc.page.html", &page);
}

#[test]
fn without_numbering_an_entry_has_no_number_span() {
    let page = page(&unnumbered(), &["\\section{One}\n\n\\subsection{Two}"]);
    let nav = nav(&page);
    assert!(!nav.contains("flm-toc-entry-number"), "{nav}");
    assert!(nav.contains("<a class=\"flm-pres-link\" href=\"#sec--One\" data-flm-kind=\"toc\">One</a>"), "{nav}");
}

#[test]
fn an_unnumbered_heading_has_no_number_and_a_heading_without_an_anchor_has_no_link() {
    let registry = numbered();
    let written = page(&registry, &["\\section{One}\n\n\\section*{Two}"]);
    let toc = nav(&written);
    assert!(toc.contains("<li><a class=\"flm-pres-link\" href=\"#sec--One\" data-flm-kind=\"toc\"><span class=\"flm-pres-span flm-toc-entry-number\">1</span> One</a></li>"), "{toc}");
    assert!(toc.contains("<li><a class=\"flm-pres-link\" href=\"#sec--Two\" data-flm-kind=\"toc\">Two</a></li>"), "{toc}");
    // Without the references feature, no heading has an anchor.
    let config = StdcoreConfig::new().headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered));
    let registry = common::stdcore::registry_over(common::stdcore::package_features_but(config, "refs"));
    let page = page(&registry, &["\\section{One}"]);
    let nav = nav(&page);
    assert!(nav.contains("<li><span class=\"flm-pres-span flm-toc-entry-number\">1</span> One</li>"), "{nav}");
}

#[test]
fn without_the_include_no_piece_is_built() {
    let registry = numbered();
    let run = html_run(&registry, &["\\section{One}"], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", diagnostic_lines(&run));
    let page = common::html::page(&run);
    assert!(!page.contains("<nav"), "{page}");
    let pieces = run.table(&PIECES_TABLE).expect("declared").expect("written");
    assert!(pieces.pieces.iter().all(|piece| piece.name.as_str() != toc::PIECE));
    let definitions = run.table(&DEFINITIONS_TABLE).expect("declared").expect("written");
    assert!(definitions.definitions.iter().all(|definition| definition.id.as_str() != toc::STYLE));
    // The builder is not scheduled at all.
    assert!(run.table(&flm_core::core::data::table::TableHandle::<flm_core::core::piece::Piece>::new(toc::PIECE)).map_or(true, |piece| piece.is_none()));
}

#[test]
fn the_depth_is_read_from_the_render_request() {
    let registry = numbered();
    let options = OptionValues::new()
        .with(PiecesRenderOptions::default().include(toc::PIECE))
        .with(TocOptions { depth: 1 });
    let run = html_run(&registry, &[OUTLINE], &render_options(options));
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", diagnostic_lines(&run));
    let page = common::html::page(&run);
    let nav = nav(&page);
    assert!(nav.contains("One") && nav.contains("Five") && !nav.contains("Two"), "{nav}");
}

// --- alt= ----------------------------------------------------------------------------------

#[test]
fn an_entry_shows_the_alternative_title_and_the_heading_and_a_reference_the_full_one() {
    let registry = numbered();
    let text = "\\section[alt={Short \\emph{title}}]{A long title}\\label{sec:a}\nSee \\ref{sec:a}.";
    let page = page(&registry, &[text]);
    let nav = nav(&page);
    assert!(nav.contains("</span> Short <em class=\"flm-pres-span\" data-flm-style=\"emph\">title</em></a>"), "{nav}");
    assert!(!nav.contains("long"), "{nav}");
    let rest = &page[page.find("</nav>").expect("a nav")..];
    assert!(rest.contains("A long title</h1>"), "{rest}");
    assert!(!rest.contains("Short"), "{rest}");
}

#[test]
fn an_empty_alternative_title_gives_an_entry_with_its_number_alone() {
    let registry = numbered();
    let nav_of = |text: &str| nav(&page(&registry, &[text])).to_string();
    let nav = nav_of("\\section[alt={}]{Long}");
    assert!(nav.contains("<span class=\"flm-pres-span flm-toc-entry-number\">1</span> </a>"), "{nav}");
    assert!(!nav.contains(">Long"), "{nav}");
}

#[test]
fn a_footnote_in_the_alternative_title_shows_its_mark_in_the_entry_and_its_note_once() {
    let registry = numbered();
    let text = "\\section[alt={Short\\footnote{The note.}}]{Long}\nText.";
    let page = page(&registry, &[text]);
    let nav = nav(&page);
    assert!(nav.contains("flm-xnotes-xnote-mark"), "{nav}");
    assert!(!nav.contains("The note."), "{nav}");
    assert_eq!(page.matches("The note.").count(), 1, "{page}");
    // The text output too: the note is listed once.
    let out = printed(&registry, &[text], &with_toc());
    assert_eq!(out.matches("The note.").count(), 1, "{out}");
}

// --- the recomposed LaTeX ------------------------------------------------------------------

#[test]
fn the_recomposed_latex_is_unchanged_by_the_feature() {
    let config = || StdcoreConfig::new().headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered));
    let latex = |registry: &Arc<FlmRegistry>, overrides: &Overrides| {
        let run = run_workflow(registry, latex_recomposed::WORKFLOW, &[OUTLINE], overrides);
        assert_eq!(
            reports(&run).into_iter().filter(|(_, severity)| *severity >= Severity::Warning).collect::<Vec<_>>(),
            [],
            "{}",
            diagnostic_lines(&run)
        );
        common::latex::output(&run)
    };
    let without = common::stdcore::registry_over(common::stdcore::package_features_but(config(), toc::FEATURE));
    let with = registry(config());
    assert_eq!(latex(&with, &Overrides::none()), latex(&without, &Overrides::none()));
    // Placed on that output by the render request, it writes nothing.
    assert_eq!(latex(&with, &with_toc()), latex(&without, &Overrides::none()));
}
