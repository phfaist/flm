//! The numbers of headings (`flm_core::stdcore::features::headings::numbers`):
//! the counters of the five sectioning levels, what the configuration's depth
//! does to a document, and what a numbered heading and a reference to one show
//! on the plain-text and the HTML backend.
//!
//! No heading kind is numbered unless the registry's configuration says so, so
//! the first test here is that a registry which asks for nothing prints a
//! document exactly as it was before this feature had counters at all. The
//! rest set the depth,
//! [`HeadingsConfig::numbering`](flm_core::stdcore::features::headings::HeadingsConfig::numbering),
//! the Python implementation's "number the levels down to this depth"
//! setting.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::golden::assert_text_golden;
use flm_core::model::doc::Doc;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::{StdcoreConfig, package};
use flm_core::stdcore::backends::latex_recomposed;
use flm_core::stdcore::backends::{html, text};
use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};
use flm_core::stdcore::features::headings::numbers::NUMBER_ROLE;
use flm_core::stdcore::features::headings::title::DEFAULT_TITLE_NUMBER_FORMAT;
use flm_core::stdcore::features::numbering::NumberingOptions;
use techy::error::Severity;

use common::lowering::text as printed;
use common::stdcore::{golden_path, plan, registry};

// ---------------------------------------------------------------------------------------------
// what a document asks for, and how it is run
// ---------------------------------------------------------------------------------------------

/// Every one of the five heading kinds numbered.
fn every_level() -> HeadingNumbering {
    HeadingNumbering::AllNumbered
}

/// The heading kinds down to level `depth` numbered, and the deeper ones not:
/// what the Python implementation asks for with its `numbering_section_depth`.
fn to_depth(depth: u8) -> HeadingNumbering {
    HeadingNumbering::UpToDepth(depth)
}

/// The standard package with the heading kinds `depth` numbers.
fn numbered_registry(depth: HeadingNumbering) -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new().headings(HeadingsConfig::new().numbering(depth)))
}

/// The standard package with the kinds of levels 4 and 5 set run in, and the
/// heading kinds `depth` numbers.
fn run_in_registry(depth: HeadingNumbering) -> Arc<FlmRegistry> {
    registry(
        StdcoreConfig::new()
            .headings(HeadingsConfig::new().run_in_levels([4, 5]).numbering(depth)),
    )
}

/// Runs `text` under the workflow `workflow` of `registry`, with `options`
/// bound on the document.
fn run_with(
    registry: &Arc<FlmRegistry>,
    workflow: &str,
    text: &str,
    options: NumberingOptions,
) -> Run {
    let doc = document(registry, text, options);
    common::pipeline::run_with(
        &doc,
        &plan(registry, workflow, &Overrides::none()),
        Externals::none(),
        &RunOptions::sequential(),
    )
}

/// The document of one tree holding `text`, with `options` bound on it.
fn document(registry: &Arc<FlmRegistry>, text: &str, options: NumberingOptions) -> Arc<Doc> {
    common::pipeline::doc_with_options(registry, &[text], OptionValues::new().with(options))
}

/// The standard package under its default configuration.
fn std_registry() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// The plain text of `text` under the standard package numbering the heading
/// kinds `depth` numbers; a run that reported anything at all is a test
/// failure.
fn text_of(text: &str, depth: HeadingNumbering) -> String {
    let run = run_with(&numbered_registry(depth), text::WORKFLOW, text, NumberingOptions::new());
    assert_clean(&run);
    printed(&run)
}

/// The HTML of `text` under the standard package numbering the heading kinds
/// `depth` numbers; a run that reported anything at all is a test failure.
fn html_of(text: &str, depth: HeadingNumbering) -> String {
    let run = run_with(&numbered_registry(depth), html::WORKFLOW, text, NumberingOptions::new());
    assert_clean(&run);
    common::html::fragment(&run)
}

/// Fails, printing the whole report, when the run reported anything.
fn assert_clean(run: &Run) {
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
}

// ---------------------------------------------------------------------------------------------
// a document that asks for nothing
// ---------------------------------------------------------------------------------------------

#[test]
fn a_registry_that_asks_for_no_number_has_none() {
    // No kind is numbered under the default depth, so the standard package
    // numbers no heading unless its configuration says otherwise: this is the
    // whole of what a document written before the counters existed sees.
    assert_eq!(text_of("\\section{Intro}\n", HeadingNumbering::NothingNumbered), "Intro\n=====\n");
    let html = html_of("\\section{Intro}\n", HeadingNumbering::default());
    assert!(!html.contains(NUMBER_ROLE), "{html}");
    assert!(!html.contains("flm-heading-number"), "{html}");
}

#[test]
fn a_depth_of_one_numbers_the_sections_alone() {
    // The Python implementation's depth boundary: with only `section`
    // numbered, a subsection is printed without a number of its own.
    assert_eq!(
        text_of("\\section{A}\\subsection{Sub}\n", to_depth(1)),
        "1. A\n====\n\nSub\n---\n"
    );
}

// ---------------------------------------------------------------------------------------------
// the sequence, the restart and the star
// ---------------------------------------------------------------------------------------------

#[test]
fn the_sections_of_a_document_are_numbered_in_the_order_they_are_written() {
    assert_eq!(text_of("\\section{Intro}\n", to_depth(1)), "1. Intro\n========\n");
    assert_eq!(
        text_of("\\section{Alpha}\\section{Beta}\n", to_depth(1)),
        "1. Alpha\n========\n\n2. Beta\n=======\n"
    );
    let five = text_of("\\section{A}\\section{B}\\section{C}\\section{D}\\section{E}\n", to_depth(1));
    assert_eq!(
        five,
        "1. A\n====\n\n2. B\n====\n\n3. C\n====\n\n4. D\n====\n\n5. E\n====\n"
    );
}

#[test]
fn headings_that_carry_the_same_title_are_numbered_apart() {
    assert_eq!(
        text_of("\\section{Intro}\\section{Intro}\\section{Intro}\n", to_depth(1)),
        "1. Intro\n========\n\n2. Intro\n========\n\n3. Intro\n========\n"
    );
}

#[test]
fn a_subsection_restarts_under_every_section() {
    // A counter's count is kept for each combination of values its ancestors
    // stand at, so the subsections of the second section start again at one.
    assert_eq!(
        text_of(
            "\\section{A}\\subsection{A1}\\subsection{A2}\\section{B}\\subsection{B1}\n",
            to_depth(2)
        ),
        "1. A\n====\n\n1.1. A1\n-------\n\n1.2. A2\n-------\n\n2. B\n====\n\n2.1. B1\n-------\n"
    );
}

#[test]
fn three_levels_are_each_numbered_within_the_one_above() {
    assert_eq!(
        text_of("\\section{A}\\subsection{A1}\\subsubsection{A1a}\n", to_depth(3)),
        "1. A\n====\n\n1.1. A1\n-------\n\n### 1.1.1. A1a\n"
    );
    // And every level restarts under the level above it, through a whole
    // document that returns to each combination once only.
    assert_eq!(
        text_of(
            "\\section{A}\\subsection{A1}\\subsubsection{A1a}\\subsubsection{A1b}\
             \\subsection{A2}\\subsubsection{A2a}\
             \\section{B}\\subsection{B1}\\subsubsection{B1a}\n",
            to_depth(3)
        ),
        "1. A\n====\n\n1.1. A1\n-------\n\n### 1.1.1. A1a\n\n### 1.1.2. A1b\n\n\
         1.2. A2\n-------\n\n### 1.2.1. A2a\n\n2. B\n====\n\n2.1. B1\n-------\n\n### 2.1.1. B1a\n"
    );
}

#[test]
fn a_starred_heading_takes_no_number_and_restarts_nothing() {
    // A starred heading states no number source at all, so it neither takes a
    // number nor advances the counter: the heading after it carries the next
    // number, and the subsections under it go on under the last section that
    // was numbered.
    assert_eq!(
        text_of("\\section{X}\\section*{Y}\\section{Z}\n", to_depth(2)),
        "1. X\n====\n\nY\n=\n\n2. Z\n====\n"
    );
    assert_eq!(
        text_of("\\section{X}\\subsection{A}\\section*{Y}\\subsection{B}\n", to_depth(2)),
        "1. X\n====\n\n1.1. A\n------\n\nY\n=\n\n1.2. B\n------\n"
    );
}

#[test]
fn the_deeper_levels_are_written_in_letters_and_the_fourth_shows_no_parent() {
    // `\paragraph` and `\subparagraph` are written in letters. A paragraph
    // restarts at every subsubsection and shows nothing of it, and a
    // subparagraph shows the paragraph it stands in.
    assert_eq!(
        text_of(
            "\\section{A}\\subsection{B}\\subsubsection{C}\\paragraph{D}\\subparagraph{E}\n",
            every_level()
        ),
        "1. A\n====\n\n1.1. B\n------\n\n### 1.1.1. C\n\n#### a. D\n\n##### a.a. E\n"
    );
    // A second paragraph in the same subsubsection is `b`, and the first
    // paragraph of the next subsubsection is `a` again.
    assert_eq!(
        text_of(
            "\\section{A}\\subsection{B}\\subsubsection{C}\\paragraph{D}\\paragraph{E}\
             \\subsubsection{F}\\paragraph{G}\n",
            every_level()
        ),
        "1. A\n====\n\n1.1. B\n------\n\n### 1.1.1. C\n\n#### a. D\n\n#### b. E\n\n\
         ### 1.1.2. F\n\n#### a. G\n"
    );
}

#[test]
fn a_paragraph_restarts_at_whichever_heading_above_it_came_last() {
    let registry = numbered_registry(every_level());
    // Directly under a subsection with no subsubsection before it, a
    // paragraph reads `a`, with no options and nothing reported.
    let run = run_with(
        &registry,
        text::WORKFLOW,
        "\\section{A}\\subsection{B}\\paragraph{P}\n",
        NumberingOptions::new(),
    );
    assert_clean(&run);
    assert_eq!(printed(&run), "1. A\n====\n\n1.1. B\n------\n\n#### a. P\n");
    // Two paragraphs under a subsection, then a subsubsection: its first
    // paragraph restarts at `a`; then a new subsection, and its first
    // paragraph is `a` again.
    let run = run_with(
        &registry,
        text::WORKFLOW,
        "\\section{A}\\subsection{B}\\paragraph{P}\\paragraph{Q}\\subsubsection{C}\\paragraph{R}\
         \\subsection{D}\\paragraph{S}\\paragraph{T}\\subsection{E}\\paragraph{U}\n",
        NumberingOptions::new(),
    );
    assert_clean(&run);
    assert_eq!(
        printed(&run),
        "1. A\n====\n\n1.1. B\n------\n\n#### a. P\n\n#### b. Q\n\n### 1.1.1. C\n\n#### a. R\n\n\
         1.2. D\n------\n\n#### a. S\n\n#### b. T\n\n1.3. E\n------\n\n#### a. U\n"
    );
}

#[test]
fn an_initial_value_holds_under_every_value_of_the_ancestors() {
    // A subsubsection written in a section with no subsection before it is
    // reported, and reads `1.0.1` once the subsections stand at 0.
    let registry = numbered_registry(every_level());
    let source = "\\section{A}\\subsubsection{B}\n";
    let refused = run_with(&registry, text::WORKFLOW, source, NumberingOptions::new());
    assert_eq!(
        common::stdcore::reports(&refused),
        [("flm.numbering.parent-uninitialized".to_string(), Severity::Error)]
    );
    let initial =
        run_with(&registry, text::WORKFLOW, source, NumberingOptions::new().initial("flm.headings.subsection", 0));
    assert_clean(&initial);
    assert_eq!(printed(&initial), "1. A\n====\n\n### 1.0.1. B\n");
}

#[test]
fn an_initial_value_makes_the_first_section_follow_it() {
    let run = run_with(
        &numbered_registry(to_depth(2)),
        text::WORKFLOW,
        "\\section{A}\\subsection{B}\\section{C}\n",
        NumberingOptions::new().initial("flm.headings.section", 3),
    );
    assert_clean(&run);
    assert_eq!(printed(&run), "4. A\n====\n\n4.1. B\n------\n\n5. C\n====\n");
}

#[test]
fn a_depth_of_two_numbers_sections_and_subsections_and_nothing_deeper() {
    assert_eq!(
        text_of("\\section{A}\\subsection{B}\\subsubsection{C}\n", to_depth(2)),
        "1. A\n====\n\n1.1. B\n------\n\n### C\n"
    );
}

#[test]
fn a_subsection_before_any_section_is_an_error_and_an_initial_value_numbers_it() {
    // A number is never shown with a part that has no value: a subsection
    // before the first section takes no number, and one Error is reported for
    // the counter, at the first such subsection. The subsections after the
    // first section are numbered as usual.
    let registry = numbered_registry(to_depth(2));
    let source = "\\subsection{Early}\\subsection{Later}\\section{A}\\subsection{Sub}\n";
    let refused = run_with(&registry, text::WORKFLOW, source, NumberingOptions::new());
    assert_eq!(
        common::stdcore::reports(&refused),
        [("flm.numbering.parent-uninitialized".to_string(), Severity::Error)]
    );
    assert_eq!(
        printed(&refused),
        "Early\n-----\n\nLater\n-----\n\n1. A\n====\n\n1.1. Sub\n--------\n"
    );
    // The document gives the sections the value 0 before the first one: the
    // early subsections are `0.1` and `0.2`, and nothing is reported.
    let initial = run_with(
        &registry,
        text::WORKFLOW,
        source,
        NumberingOptions::new().initial("flm.headings.section", 0),
    );
    assert_clean(&initial);
    assert_eq!(
        printed(&initial),
        "0.1. Early\n----------\n\n0.2. Later\n----------\n\n1. A\n====\n\n1.1. Sub\n--------\n"
    );
}

// ---------------------------------------------------------------------------------------------
// what a reference to a numbered heading shows
// ---------------------------------------------------------------------------------------------

#[test]
fn a_reference_to_a_numbered_heading_shows_the_section_sign_and_the_number() {
    let one = text_of(
        "\\section{Intro}\\label{sec:intro} See Section~\\ref{sec:intro}.\n",
        to_depth(1),
    );
    // The section sign and the number are joined by a narrow no-break space,
    // which the plain-text backend keeps as written.
    assert!(one.ends_with("See Section\u{a0}\u{a7}\u{202F}1.\n"), "{one:?}");
    let deep = text_of(
        "\\section{A}\\subsection{Sub}\\label{sec:sub} Ref to~\\ref{sec:sub}.\n",
        to_depth(2),
    );
    assert!(deep.ends_with("Ref to\u{a0}\u{a7}\u{202F}1.1.\n"), "{deep:?}");
    // `S` asks for the capitalized wording where the style declares one. A
    // heading's style declares no variant of its word — the section sign has
    // no capital form — so the usual wording is written, and nothing is
    // reported.
    let capital = text_of(
        "\\section{Intro}\\label{sec:intro} See~\\ref[S]{sec:intro}.\n",
        to_depth(1),
    );
    assert!(capital.ends_with("See\u{a0}\u{a7}\u{202F}1.\n"), "{capital:?}");
}

#[test]
fn a_reference_to_an_unnumbered_heading_still_shows_its_title() {
    // Nothing about a reference changes for a heading this document did not
    // number: what a reference shows is the number if there is one, else the
    // content, else the label as written.
    let text = text_of(
        "\\section{Methods}\\label{sec:m} See~\\ref{sec:m}.\n",
        HeadingNumbering::NothingNumbered,
    );
    assert!(text.ends_with("See\u{a0}Methods.\n"), "{text:?}");
}

#[test]
fn a_reference_to_a_starred_heading_shows_its_title_in_a_numbered_document() {
    // The star is what decides: the heading takes no number even where its
    // kind is numbered, so a reference to it shows the title.
    let text = text_of(
        "\\section{A}\\section*{Aside}\\label{sec:aside} See~\\ref{sec:aside}.\n",
        to_depth(1),
    );
    assert!(text.starts_with("1. A\n"), "{text:?}");
    assert!(text.ends_with("See\u{a0}Aside.\n"), "{text:?}");
}

#[test]
fn a_reference_naming_several_numbered_headings_shows_them_as_one_list() {
    // The word stands once, before the first number, and each number is a
    // link of its own; the compact join texts put a bare comma between two
    // numbers.
    let text = text_of(
        "\\section{Alpha}\\label{sec:a}\\section{Beta}\\label{sec:b} Back to~\\ref{sec:a,sec:b}.\n",
        to_depth(1),
    );
    // The word is written into a link of its own, so the space it ends in
    // stands at that link's edge. It is a no-break space, which this backend
    // keeps there, as it keeps it everywhere: the plain text reads
    // `\u{a7}\u{202F}1,2`, as the page does.
    assert!(text.ends_with("Back to\u{a0}\u{a7}\u{202F}1,2.\n"), "{text:?}");
    let html = html_of(
        "\\section{Alpha}\\label{sec:a}\\section{Beta}\\label{sec:b} Back to \\ref{sec:a,sec:b}.\n",
        to_depth(1),
    );
    // Three links: the word, the first number and the second, the word and the
    // first number both pointing at the first section.
    assert_eq!(html.matches("href=\"#sec-a\"").count(), 2, "{html}");
    assert_eq!(html.matches("href=\"#sec-b\"").count(), 1, "{html}");
}

#[test]
fn a_reference_written_with_noprefix_drops_the_word_and_keeps_the_number() {
    let text = text_of(
        "\\section{Intro}\\label{sec:intro} See~\\ref[noprefix]{sec:intro}.\n",
        to_depth(1),
    );
    assert!(text.ends_with("See\u{a0}1.\n"), "{text:?}");
}

// ---------------------------------------------------------------------------------------------
// the two backends
// ---------------------------------------------------------------------------------------------

#[test]
fn a_numbered_block_heading_carries_its_number_in_a_span_of_its_own() {
    let html = html_of("\\section{Intro}\\label{sec:intro}\n", to_depth(1));
    assert!(
        html.contains("<span class=\"flm-pres-span flm-heading-number\">1</span>. Intro"),
        "{html}"
    );
    // The number stands inside the heading, so the identifier the heading is
    // linked by is still the heading's own.
    assert!(html.contains("id=\"sec-intro\""), "{html}");
}

#[test]
fn the_rule_under_a_numbered_heading_measures_the_number_with_the_title() {
    // The plain-text backend draws the rule of a level-one heading the width
    // of what it printed, and the number is part of that: `1. Intro` is eight
    // characters and takes eight `=`.
    let text = text_of("\\section{Intro}\n", to_depth(1));
    let mut lines = text.lines();
    let title = lines.next().expect("the title");
    let rule = lines.next().expect("the rule");
    assert_eq!(title, "1. Intro");
    assert_eq!(rule.chars().count(), title.chars().count());
}

#[test]
fn a_numbered_run_in_heading_prints_its_number_before_its_title() {
    let run_in = run_in_registry(every_level());
    let run = run_with(
        &run_in,
        text::WORKFLOW,
        "\\section{A}\\subsection{B}\\subsubsection{C}\\paragraph{P} Content.\n",
        NumberingOptions::new(),
    );
    assert_clean(&run);
    // The number, the joiner, the title, and then the stop the backend writes
    // after a run-in title that does not end in punctuation of its own: one
    // stop, not two.
    assert_eq!(
        printed(&run),
        "1. A\n====\n\n1.1. B\n------\n\n### 1.1.1. C\n\na. P. Content.\n"
    );
    let html_run = run_with(
        &run_in,
        html::WORKFLOW,
        "\\section{A}\\subsection{B}\\subsubsection{C}\\paragraph{P} Content.\n",
        NumberingOptions::new(),
    );
    assert_clean(&html_run);
    let html = common::html::fragment(&html_run);
    assert!(html.contains("flm-heading-number\">a</span>. P</span>.</span>"), "{html}");
}

#[test]
fn the_recomposed_latex_writes_no_number_of_its_own() {
    // That backend writes the sectioning command back as the author wrote it
    // and leaves the numbering to LaTeX, so a document numbered here is
    // recomposed unchanged.
    let run = run_with(
        &numbered_registry(to_depth(1)),
        latex_recomposed::WORKFLOW,
        "\\section{Intro}\\label{sec:intro}\n",
        NumberingOptions::new(),
    );
    assert_clean(&run);
    let written = common::latex::output(&run);
    assert!(written.contains("\\section{Intro}"), "{written}");
    assert!(!written.contains("1."), "{written}");
}

// ---------------------------------------------------------------------------------------------
// the same document, whatever runs it
// ---------------------------------------------------------------------------------------------

#[test]
fn the_same_document_is_numbered_alike_under_every_way_of_running_it() {
    let source = "\\section{A}\\subsection{A1}\\label{sec:a1}\\section{B} See~\\ref{sec:a1}.\n";
    let registry = numbered_registry(to_depth(2));
    let expected = text_of(source, to_depth(2));
    for options in [RunOptions::new(), RunOptions::sequential()] {
        let doc = document(&registry, source, NumberingOptions::new());
        let run = common::pipeline::run_with(
            &doc,
            &plan(&registry, text::WORKFLOW, &Overrides::none()),
            Externals::none(),
            &options,
        );
        assert_clean(&run);
        assert_eq!(printed(&run), expected);
    }
    // And the numbers of the HTML output are the numbers of the text output.
    let html = html_of(source, to_depth(2));
    assert!(html.contains(">1</span>. A"), "{html}");
    assert!(html.contains(">1.1</span>. A1"), "{html}");
    assert!(html.contains(">2</span>. B"), "{html}");
}

#[test]
fn a_registry_without_the_numbering_feature_prints_the_headings_without_numbers() {
    // The counter table is declared as a peer table and read optionally, and
    // the counters and styles are peer contributions, so the headings feature
    // registers and renders where nothing numbers anything. The registry
    // build records one Note per dropped contribution.
    let registry = common::stdcore::registry_over(vec![
        flm_core::stdcore::backends::lowering::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        flm_core::stdcore::check_feature(),
        Arc::new(flm_core::stdcore::features::blocks::Blocks::default()),
        flm_core::stdcore::features::index::feature(),
        flm_core::stdcore::features::refpoints::feature(),
        flm_core::stdcore::features::refs::feature(),
        flm_core::stdcore::features::headings::feature(),
    ]);
    let doc = common::pipeline::doc(&registry, &["\\section{Intro}\n"]);
    let run = common::pipeline::run_with(
        &doc,
        &plan(&registry, text::WORKFLOW, &Overrides::none()),
        Externals::none(),
        &RunOptions::sequential(),
    );
    assert_clean(&run);
    assert_eq!(printed(&run), "Intro\n=====\n");
}

// ---------------------------------------------------------------------------------------------
// the goldens
// ---------------------------------------------------------------------------------------------

/// A document holding one heading of every level, each numbered, with a
/// reference to one of them.
const EVERY_LEVEL: &str = "\\section{First}\\label{sec:first}\n\n\
                           \\subsection{Second}\n\n\
                           \\subsubsection{Third}\n\n\
                           \\paragraph{Fourth}\n\n\
                           \\subparagraph{Fifth}\n\n\
                           Back to \\ref{sec:first}, and to \\ref[noprefix]{sec:first}.\n";

#[test]
fn the_text_golden_of_numbered_block_headings() {
    assert_text_golden(
        golden_path("headings-numbered.txt"),
        &text_of(EVERY_LEVEL, every_level()),
    );
}

#[test]
fn the_text_golden_of_numbered_run_in_headings() {
    let run_in = run_in_registry(every_level());
    let run = run_with(&run_in, text::WORKFLOW, EVERY_LEVEL, NumberingOptions::new());
    assert_clean(&run);
    assert_text_golden(golden_path("headings-numbered-runin.txt"), &printed(&run));
}

#[test]
fn the_html_golden_of_numbered_block_headings() {
    common::html::assert_golden("headings-numbered.html", &html_of(EVERY_LEVEL, every_level()));
}

#[test]
fn the_html_golden_of_numbered_run_in_headings() {
    let run_in = run_in_registry(every_level());
    let run = run_with(&run_in, html::WORKFLOW, EVERY_LEVEL, NumberingOptions::new());
    assert_clean(&run);
    common::html::assert_golden("headings-numbered-runin.html", &common::html::fragment(&run));
}

#[test]
fn the_html_golden_where_a_reference_option_changes_what_a_reference_shows() {
    // `\ref` and `\ref[noprefix]` name one numbered heading: the first writes
    // the word before the number, the second writes the number alone. Until a
    // reference had a number to show there was no option that changed the
    // markup, and the two produced the same page. The last `\ref` names a
    // numbered section and an unnumbered subsection: two items of one list,
    // two links with the joining text between them, outside both links.
    let html = html_of(
        "\\section{Sums}\\label{sec:sums}\n\n\
         See \\ref{sec:sums} and \\ref[noprefix]{sec:sums}.\n\n\
         \\subsection{Terms}\\label{sec:terms}\n\n\
         See \\ref{sec:sums,sec:terms}.\n",
        to_depth(1),
    );
    // The narrow no-break space of the word reaches the page as a numeric
    // character reference, which is how this backend writes the space
    // characters a reader cannot tell from an ordinary space.
    assert!(html.contains("\u{a7}&#x202F;1</a>"), "{html}");
    common::html::assert_golden("refs-numbered-heading.html", &html);
}

// ---------------------------------------------------------------------------------------------
// the declarations themselves
// ---------------------------------------------------------------------------------------------

#[test]
fn the_five_counters_and_the_levels_are_declared_to_the_numbering_feature() {
    let registry = std_registry();
    let declared: Vec<String> = registry
        .peer_slots()
        .iter()
        .filter(|entry| entry.owner.as_str() == "headings")
        .map(|entry| entry.slot.to_string())
        .collect();
    // Five counters, five styles and the parent set of the levels, every one
    // of them resolved by the numbering feature, and the definition piece of
    // the `latex-rendered` backend in the LaTeX preamble slot.
    assert_eq!(declared.len(), 12, "{declared:?}");
    assert_eq!(declared.iter().filter(|slot| *slot == "flm.latex.preamble").count(), 1, "{declared:?}");
    assert!(registry.peer_slots().iter().all(|entry| entry.resolved));
    // The title number format is one text for every level.
    assert_eq!(DEFAULT_TITLE_NUMBER_FORMAT, "#{number}. ");
    assert!(
        HeadingsConfig::new()
            .headings
            .iter()
            .all(|kind| kind.title_number_format == DEFAULT_TITLE_NUMBER_FORMAT)
    );
}

// ---------------------------------------------------------------------------------------------
// what stands before the title: the title number format
// ---------------------------------------------------------------------------------------------

/// The shipped configuration with every kind numbered, the kinds of levels in
/// `run_in` set run in, and the kind `kind`'s title number format replaced by
/// `template`.
fn with_title(kind: &str, template: &str, run_in: &[u8]) -> HeadingsConfig {
    let mut config =
        HeadingsConfig::new().run_in_levels(run_in.iter().copied()).numbering(every_level());
    for declared in &mut config.headings {
        if declared.name.as_str() == kind {
            *declared = declared.clone().title_number_format(template);
        }
    }
    config
}

/// The run of `text` under the workflow `workflow` of the standard package
/// configured by `config`; a run that reported anything at all is a test
/// failure.
fn run_titled(config: HeadingsConfig, workflow: &str, text: &str) -> Run {
    let registry = registry(StdcoreConfig::new().headings(config));
    let run = run_with(&registry, workflow, text, NumberingOptions::new());
    assert_clean(&run);
    run
}

/// What the registry build under `config` reported, rendered; a build that
/// succeeds is a test failure.
fn title_build_error(config: HeadingsConfig) -> String {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().headings(config)));
    builder.use_package(flm_core::math_passthrough::package());
    match builder.build(&RegistryInputs::new()) {
        Ok(_) => panic!("the registry build was expected to fail"),
        Err(error) => error.render(),
    }
}

#[test]
fn a_title_number_format_can_hold_a_word_and_its_own_joining_text() {
    let run = run_titled(
        with_title("section", "Chapter~#{number}: ", &[]),
        text::WORKFLOW,
        "\\section{Intro}\\subsection{Sub}\n",
    );
    // The tie is a no-break space; the other kinds keep the default.
    assert_eq!(printed(&run), "Chapter\u{a0}1: Intro\n================\n\n1.1. Sub\n--------\n");
}

#[test]
fn the_number_alone_is_in_the_span_and_the_joining_text_outside_it() {
    let run = run_titled(
        with_title("section", "Chapter~#{number}: ", &[]),
        html::WORKFLOW,
        "\\section{Intro}\n",
    );
    let html = common::html::fragment(&run);
    assert!(
        html.contains(
            "Chapter&#xA0;<span class=\"flm-pres-span flm-heading-number\">1</span>: Intro</h1>"
        ),
        "{html}"
    );
    assert_eq!(html.matches("flm-heading-number").count(), 1, "{html}");
}

#[test]
fn markup_may_span_the_number() {
    let run = run_titled(
        with_title("section", "\\emph{#{number}}. ", &[]),
        html::WORKFLOW,
        "\\section{Intro}\n",
    );
    let html = common::html::fragment(&run);
    // The emphasis holds the span, and the span holds the number alone.
    let span = "<span class=\"flm-pres-span flm-heading-number\">1</span>";
    let at = html.find(span).unwrap_or_else(|| panic!("{html}"));
    assert!(html[..at].contains("<em"), "{html}");
    assert!(html[at + span.len()..].starts_with("</em>. Intro"), "{html}");
}

#[test]
fn a_format_key_writes_the_kind_s_own_number_in_that_format() {
    let run = run_titled(
        with_title("subsection", "#[Roman]{number}. ", &[]),
        text::WORKFLOW,
        "\\section{A}\\subsection{B}\\subsection{C}\n",
    );
    // The parents are written as they are, the own number in capital roman
    // numerals.
    assert_eq!(printed(&run), "1. A\n====\n\n1.I. B\n------\n\n1.II. C\n-------\n");
}

#[test]
fn the_bare_key_writes_the_kind_s_own_number_without_its_parents() {
    let run = run_titled(
        with_title("subsection", "#[bare]{number} ", &[]),
        text::WORKFLOW,
        "\\section{A}\\subsection{B}\\subsection{C}\n",
    );
    // `2` where the default writes `1.2`.
    assert_eq!(printed(&run), "1. A\n====\n\n1 B\n---\n\n2 C\n---\n");
}

#[test]
fn a_template_without_the_placeholder_shows_no_number_and_still_counts() {
    let run = run_titled(
        with_title("section", "* ", &[]),
        text::WORKFLOW,
        "\\section{A}\\section{B}\\subsection{C}\n",
    );
    // The sections count: the subsection of the second one is `2.1`.
    assert_eq!(printed(&run), "* A\n===\n\n* B\n===\n\n2.1. C\n------\n");
}

#[test]
fn a_run_in_kind_writes_its_template_before_its_title() {
    let run = run_titled(
        with_title("paragraph", "(#{number})~", &[4, 5]),
        text::WORKFLOW,
        "\\section{A}\\paragraph{P} Content.\n",
    );
    assert_eq!(printed(&run), "1. A\n====\n\n(a)\u{a0}P. Content.\n");
}

#[test]
fn an_unnumbered_heading_shows_nothing_of_the_template() {
    let run = run_titled(
        with_title("section", "Chapter~#{number}: ", &[]),
        text::WORKFLOW,
        "\\section*{U}\\section[numbered=false]{V}\n",
    );
    assert_eq!(printed(&run), "U\n=\n\nV\n=\n");
}

#[test]
fn a_title_number_format_that_does_not_compile_fails_the_build_naming_the_kind() {
    // Each case fails for its own reason, which the message names after the kind.
    for (template, what, cause) in [
        ("\\textbf{#{number}. ", "not valid FLM", "unclosed group"),
        ("#{num}. ", "an unknown placeholder", "placeholder ‘#num’"),
        ("#[bold]{number}. ", "an unknown key", "`bold`"),
    ] {
        let error = title_build_error(with_title("subsection", template, &[]));
        assert!(error.contains("heading kind `subsection`"), "{what}: {error}");
        assert!(error.contains(cause), "{what}: {error}");
    }
}
