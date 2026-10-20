//! The content features on the `html` backend
//! (`flm_core::stdcore::backends::html`), from FLM source through the whole
//! pipeline to the HTML of a fragment: one golden per property, each named for
//! the property it pins.
//!
//! What is pinned here: the four text-style commands and their nesting; the
//! nine spellings that stand for a single character and the sentence-spacing
//! mark; paragraphs, and what a fragment's declared layout decides about them;
//! headings at every level, with labels, starred, run in, and with a rich
//! title; the two referencing callables, resolved and unresolved; the five
//! verbatim callables, alone, inside an argument and inside a heading; an
//! unattributed macro and environment; an element the caller accepted as
//! unrendered; an author's own definitions; a comment; a document of two
//! trees; text that tries to break out of every context a document can reach;
//! and the one byte of that document which only a changed text configuration
//! can deliver, the straight double quote in an attribute value.
//!
//! Three neighboring suites hold the rest and are not repeated here: the
//! emitter over hand-built presentation trees is `tests/html_emitter.rs`,
//! mathematics is `tests/html_math.rs`, and the page, its definitions and its
//! stylesheets are `tests/html_page.rs`. Every golden this suite writes is
//! parsed by `tests/html_validity.rs` in the same test run.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{BuildError, FeatureBuilder, FlmFeature, FlmRegistry};
use flm_core::core::run::Run;
use flm_core::dump::diag::expect;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::model::doc::FragmentLayout;
use flm_core::model::names::{BackendName, ElementName};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::html;
use flm_core::stdcore::features::blocks::BlocksConfig;
use flm_core::stdcore::features::headings::HeadingsConfig;
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use techy::error::Severity;

use common::html::{assert_golden, fragment, run_one_fragment_under};
use common::macro_def;
use common::stdcore::{registry, run_workflow};

// ============================================================================
// the fixtures every area uses
// ============================================================================

/// The registry of the standard package under its default configuration.
fn std_registry() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// Runs the documents `texts`, one tree each, under the `html` workflow of
/// `registry`, and answers the run.
fn render_over(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Run {
    run_workflow(registry, html::WORKFLOW, texts, &Overrides::none())
}

/// The HTML of the documents `texts` under the standard package, with no
/// diagnostic reported at all.
///
/// A diagnostic here is a test failure printing the run's whole report, since
/// every property this suite pins is a property of a document the library
/// accepts without complaint.
fn clean(texts: &[&str]) -> String {
    let run = render_over(&std_registry(), texts);
    assert_clean(&run);
    fragment(&run)
}

/// Fails, printing the whole report, when the run reported anything.
fn assert_clean(run: &Run) {
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
}

// ============================================================================
// text styles
// ============================================================================

#[test]
fn the_four_text_style_commands_are_four_elements_carrying_their_style() {
    // One span element per command, each with the style on `data-flm-style`
    // and a tag chosen for it. `\emph` and `\textit` stay two styles — the
    // Python implementation makes the first a synonym of the second — because
    // semantic emphasis and an italic face are two different statements about
    // the text, which a stylesheet and a screen reader may treat differently.
    let html = clean(&[
        "\\textbf{bold}, \\textit{italic}, \\emph{emphasized} and \\texttt{code}.\n",
    ]);
    assert!(html.contains("data-flm-style=\"bold\""), "{html}");
    assert!(html.contains("data-flm-style=\"italic\""), "{html}");
    assert!(html.contains("data-flm-style=\"emph\""), "{html}");
    assert!(html.contains("data-flm-style=\"monospace\""), "{html}");
    assert_golden("styles-each.html", &html);
}

#[test]
fn text_styles_nest_as_elements_rather_than_stacking_in_one_class_list() {
    // A command inside another command is an element inside an element: one
    // span carries one style. The Python renderer instead collects the styles
    // of one node into a single class list, which cannot say which style is
    // inside which.
    let html = clean(&[
        "\\textbf{bold with \\textit{italic inside}} and \\textbf{\\emph{emphasis in bold}}.\n",
    ]);
    assert_golden("styles-nested.html", &html);
}

// ============================================================================
// the literal spellings and the sentence-spacing mark
// ============================================================================

#[test]
fn every_literal_spelling_reaches_the_page_as_the_character_it_stands_for() {
    // The nine spellings that stand for a single character, then the
    // sentence-spacing mark `\@`, which stands for no character at all and
    // whose only job is to tell the typesetter that the period before it ends
    // a word rather than a sentence. A character HTML gives a meaning to is
    // written as a character reference (`&amp;`), and so is a space a reader
    // could not otherwise tell from an ordinary one (`&#xA0;` for the
    // non-breaking space `~` stands for).
    let html = clean(&[
        "\\% \\# \\& \\$ a\\ b \\{ \\} \\textbackslash{} a~b.\n\nMr.\\@ Bond.\n",
    ]);
    assert!(html.contains("&amp;"), "{html}");
    assert!(html.contains("a&#xA0;b"), "{html}");
    assert!(html.contains("Mr. Bond."), "{html}");
    assert_golden("literals-all.html", &html);
}

// ============================================================================
// paragraphs, and the layout that decides them
// ============================================================================

#[test]
fn a_paragraph_break_makes_two_paragraph_elements() {
    let html = clean(&["First paragraph.\n\nSecond paragraph.\n"]);
    assert_eq!(html.matches("<p class=\"flm-pres-par\">").count(), 2, "{html}");
    assert_golden("paragraphs-two.html", &html);
}

#[test]
fn a_fragments_declared_layout_decides_whether_a_paragraph_is_written() {
    // The emitter writes what the tree holds and invents nothing, so whether a
    // run of text becomes a paragraph is settled before it: by the layout the
    // fragment was parsed under. A host that wants a paragraph element always
    // asks for the blocks layout; one rendering a title or a caption asks for
    // the inline layout. Under the default layout a single run stays inline,
    // which is what the Python renderer arrives at by guessing from what it
    // was handed.
    let one_run = "A \\emph{single} run of text";
    let registry = std_registry();
    let under = |layout| {
        let run = run_one_fragment_under(&registry, layout, one_run);
        assert_clean(&run);
        fragment(&run)
    };
    let inline = under(FragmentLayout::Inline);
    let blocks = under(FragmentLayout::Blocks);
    let default = under(FragmentLayout::default());
    assert!(!inline.contains("<p"), "{inline}");
    assert!(blocks.contains("<p class=\"flm-pres-par\">"), "{blocks}");
    assert_eq!(inline, default);
    assert_golden("layout-inline.html", &inline);
    assert_golden("layout-blocks.html", &blocks);
    assert_golden("layout-default.html", &default);
}

// ============================================================================
// headings
// ============================================================================

/// A document holding one heading of each of the five sectioning commands,
/// each followed on the same paragraph by the text it introduces.
///
/// The text stands on the heading's own line so that the two configurations
/// below can be told apart over one document: a heading that stays a block is
/// a block whatever follows it, and the text after it becomes a paragraph of
/// its own; a level the host lists as run in leads that paragraph instead,
/// which is the whole point of the setting.
const EVERY_LEVEL: &str = "\\section{Level one} Text.\n\n\
                           \\subsection{Level two} Text.\n\n\
                           \\subsubsection{Level three} Text.\n\n\
                           \\paragraph{Level four} Text.\n\n\
                           \\subparagraph{Level five} Text.\n";

#[test]
fn the_five_sectioning_commands_are_five_heading_levels() {
    // Every heading is a block of its own under the default configuration,
    // whatever its level, and carries its level as an attribute beside the
    // heading tag chosen for it. The Python renderer stops giving a heading
    // tag below level three and writes a span instead; here the level is data
    // and the tag follows it.
    let html = clean(&[EVERY_LEVEL]);
    for level in 1..=5 {
        assert!(html.contains(&format!("data-flm-level=\"{level}\"")), "{html}");
        assert!(html.contains(&format!("<h{level} class=\"flm-pres-block--heading\"")), "{html}");
    }
    assert_golden("headings-levels.html", &html);
}

#[test]
fn a_heading_with_a_label_carries_the_labels_anchor_as_its_identifier() {
    // The identifier is the anchor the references feature computed for the
    // label — one label, one anchor, no counter and no length limit — and the
    // emitter writes it without inventing anything of its own. A heading the
    // author gave no label is still a point a reference could name, so it is
    // still given an anchor, minted from its title; what changes is only that
    // no label names it.
    let html = clean(&[
        "\\section{With a label}\\label{sec:labeled}\n\n\\section{Without one}\n",
    ]);
    assert!(html.contains("id=\"sec-labeled\""), "{html}");
    assert_eq!(html.matches("id=").count(), 2, "{html}");
    assert_golden("headings-labels.html", &html);
}

#[test]
fn a_starred_heading_is_written_like_any_other_heading() {
    // The star asks for a heading that consumes no number. No feature numbers
    // headings yet, so the two are written alike today; the golden is what the
    // numbering step will change, and the star is already carried through the
    // document model rather than lost at the parse.
    let html = clean(&["\\section*{Starred}\n\nText.\n\n\\section{Plain}\n"]);
    assert_golden("headings-starred.html", &html);
}

#[test]
fn a_run_in_heading_is_a_decoration_at_the_start_of_its_paragraph() {
    // A level a host lists as run in is set at the start of the paragraph it
    // introduces instead of standing on a line of its own. What the tree
    // carries is a decoration holding the title; this backend writes the stop
    // after the title as ordinary text inside the decoration's span, so a
    // reader copies it with the rest of the page, and the stylesheet supplies
    // only the gap after it.
    //
    // The document is the one the test above renders, so that the golden pair
    // shows exactly what the setting changes and nothing else. A run-in
    // heading with nothing after it on its own side of a blank line makes a
    // paragraph holding the decoration alone; that is block structure, the
    // same on every backend, and `tests/stdcore_headings_runin.rs` pins it.
    let run_in = registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().run_in_levels([4, 5])),
    );
    let run = render_over(&run_in, &[EVERY_LEVEL]);
    assert_clean(&run);
    let html = fragment(&run);
    assert!(html.contains("<span class=\"flm-pres-deco-runin\">"), "{html}");
    // The first three levels are untouched; the last two are decorations
    // inside the paragraph that follows them.
    assert!(html.contains("<h3 class=\"flm-pres-block--heading\""), "{html}");
    assert!(!html.contains("<h4"), "{html}");
    assert!(!html.contains("<h5"), "{html}");
    assert_eq!(html.matches("flm-pres-deco-runin").count(), 2, "{html}");
    // Each decoration opens the paragraph holding the text it introduces: the
    // title, the stop inside the same span, and then the text.
    assert_eq!(html.matches("</span>.</span>Text.</p>").count(), 2, "{html}");
    assert_golden("headings-runin.html", &html);
}

#[test]
fn a_run_in_title_that_already_ends_in_punctuation_takes_no_second_stop() {
    // The stop is written by this backend, from the decoration's content tree,
    // so a title that already ends in sentence punctuation keeps what it has:
    // `Why?` reads as `Why?`, not `Why?.`. The test is the one every backend
    // shares, so the plain-text output of the same document agrees.
    //
    // Each heading is followed by the text it introduces on the same
    // paragraph, since that is where a stop and the gap after it are read.
    let run_in = registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().run_in_levels([4])),
    );
    let run = render_over(
        &run_in,
        &[
            "\\paragraph{Why?}\\label{sec:why} Because.\n\n\
             \\paragraph{Note}\\label{sec:note} Something.\n\n\
             \\paragraph{Done.}\\label{sec:done} Finished.\n",
        ],
    );
    assert_clean(&run);
    let html = fragment(&run);
    assert_eq!(html.matches("flm-pres-deco-runin").count(), 3, "{html}");
    // `Why?` and `Done.` end in punctuation of their own and take nothing;
    // `Note` takes the period.
    assert!(html.contains("Why?</span></span>Because."), "{html}");
    assert!(html.contains("Note</span>.</span>Something."), "{html}");
    assert!(html.contains("Done.</span></span>Finished."), "{html}");
    assert_golden("headings-runin-punctuation.html", &html);
}

#[test]
fn a_run_in_title_ending_in_emphasis_is_read_through_to_its_last_character() {
    // The test looks at the plain text of the whole content tree, not at the
    // decoration's own last child, so a title whose last element is an
    // emphasized word is judged by the characters inside that element.
    let run_in = registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().run_in_levels([4])),
    );
    let run = render_over(
        &run_in,
        &[
            "\\paragraph{See \\emph{this}} Text.\n\n\
             \\paragraph{See \\emph{that!}} More.\n",
        ],
    );
    assert_clean(&run);
    let html = fragment(&run);
    assert!(html.contains("this</em></span>.</span>Text."), "{html}");
    assert!(html.contains("that!</em></span></span>More."), "{html}");
}

#[test]
fn a_run_in_heading_with_no_title_takes_no_stop() {
    // `\paragraph{}` leaves the decoration with nothing in it. A stop with no
    // title in front of it says nothing, so neither backend writes one: the
    // text backend prints the decoration not at all, and this one writes the
    // span and leaves it empty. A title of nothing but spaces is the same
    // case; a title that is a non-breaking space is not, since that space is
    // text the author asked for.
    let run_in = registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().run_in_levels([4])),
    );
    let run = render_over(
        &run_in,
        &["\\paragraph{} Some content.\n\n\\paragraph{ } More content.\n"],
    );
    assert_clean(&run);
    let html = fragment(&run);
    assert_eq!(html.matches("flm-pres-deco-runin").count(), 2, "{html}");
    assert!(!html.contains("</span>.</span>"), "{html}");
}

#[test]
fn the_stop_after_a_run_in_heading_is_a_render_option() {
    // The stop is `HtmlRenderOptions::run_in_stop`: a host asking for a colon
    // gets one, and the empty text writes no stop at all. The option has the
    // same name and the same meaning on the text backend.
    let run_in = registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().run_in_levels([4])),
    );
    let document = "\\paragraph{Note} Some content.\n";
    let under = |stop: &str| {
        let overrides = Overrides {
            render_options: OptionValues::new().with(html::HtmlRenderOptions {
                run_in_stop: String::from(stop),
                ..html::HtmlRenderOptions::default()
            }),
            ..Overrides::none()
        };
        let run = run_workflow(&run_in, html::WORKFLOW, &[document], &overrides);
        assert_clean(&run);
        fragment(&run)
    };
    let colon = under(":");
    assert!(colon.contains("Note</span>:</span>"), "{colon}");
    let none = under("");
    assert!(none.contains("Note</span></span>"), "{none}");
    // The stop is document text like any other: it reaches the page escaped.
    let ampersand = under(" &");
    assert!(ampersand.contains("Note</span> &amp;</span>"), "{ampersand}");
}

#[test]
fn a_heading_title_holding_markup_and_mathematics_keeps_both_in_the_heading() {
    // A title is content like any other: a style command inside it is an
    // element inside the heading element, and a formula inside it is handed to
    // the formula renderer where it stands.
    let html = clean(&["\\section{The \\emph{shape} of \\(x^2\\)}\\label{sec:shape}\n"]);
    assert!(html.contains("<h1 class=\"flm-pres-block--heading\" id=\"sec-shape\""), "{html}");
    assert!(html.contains("data-flm-style=\"emph\""), "{html}");
    assert!(html.contains("\\(x^2\\)"), "{html}");
    assert_golden("headings-rich-title.html", &html);
}

// ============================================================================
// references
// ============================================================================

/// A document whose first tree defines a heading the second refers to; kept in
/// one tree here so that each golden is self-contained.
const REFERENCED: &str = "\\section{Sums of squares}\\label{sec:sums}\n\n";

#[test]
fn a_reference_to_a_heading_is_a_link_carrying_its_kind() {
    // `\ref` shows the target's own display text and links to the anchor the
    // references feature computed. The kind of reference is an attribute, not
    // part of the class, so a stylesheet selecting the link class keeps
    // working when a new kind appears. The display options a reference site may
    // carry — here `noprefix`, which asks for the target's name without the
    // word naming its kind — are read by the feature that resolves the
    // reference and reach this backend only as the text it is given: the markup
    // is the same either way, which is what the two links below show.
    let html = clean(&[&format!(
        "{REFERENCED}See \\ref{{sec:sums}} and \\ref[noprefix]{{sec:sums}}.\n"
    )]);
    assert!(html.contains("data-flm-kind=\"ref\""), "{html}");
    assert_eq!(html.matches("href=\"#sec-sums\"").count(), 2, "{html}");
    assert_golden("refs-ref.html", &html);
}

#[test]
fn a_hyperref_shows_the_authors_own_text() {
    // The second referencing callable over the same targets: what it shows is
    // the text the author wrote, markup included, and where it points is the
    // same anchor.
    let html = clean(&[&format!(
        "{REFERENCED}See \\hyperref[sec:sums]{{the \\emph{{first}} part}}.\n"
    )]);
    assert!(html.contains("href=\"#sec-sums\""), "{html}");
    assert!(html.contains("data-flm-style=\"emph\""), "{html}");
    assert_golden("refs-hyperref.html", &html);
}

#[test]
fn a_reference_whose_target_is_unknown_is_marked_in_place_and_links_to_nothing() {
    // The Python renderer aborts the whole render on an unresolved reference.
    // Here the page is produced, each unresolved site is written as a span the
    // stylesheet shows as an error, and one Error diagnostic is reported for
    // each — so an author sees every broken reference at once instead of the
    // first one. No link is written, so the page holds no address pointing at
    // nothing.
    let run = render_over(
        &std_registry(),
        &["See \\ref{sec:nowhere} and \\hyperref[sec:nowhere]{this}.\n"],
    );
    expect(run.diagnostics().iter(), "flm.refs.unresolved").count(2).severity(Severity::Error);
    let html = fragment(&run);
    assert!(html.contains("data-flm-style=\"unresolved-reference\""), "{html}");
    assert!(!html.contains("href="), "{html}");
    assert_golden("refs-unresolved.html", &html);
}

// ============================================================================
// verbatim content
// ============================================================================

#[test]
fn the_five_verbatim_callables_carry_their_kind_their_display_and_their_language() {
    // Three macros and two environments. What tells them apart is data on the
    // element — the kind of verbatim run, whether it is inline or a block, and
    // the language tag where the author gave one — rather than a class per
    // combination, as the Python renderer builds. The content is copied byte
    // for byte, so a backslash stays a backslash: the formula renderer is
    // restricted to mathematics elements and never looks at code, which is why
    // the Python renderer's wrapping of every backslash is unnecessary here.
    let html = clean(&[
        "\\verbtext+hello world+ \\verbcode[python]+def f(): pass+ \\verba[rust]+struct+ \
         \\verbcode+a{b}c \\alpha+\n\n\
         \\begin{verbatimtext}some text here\\end{verbatimtext}\n\n\
         \\begin{verbatimcode}[python]\ndef g():\n    return 1\n\\end{verbatimcode}\n",
    ]);
    assert!(html.contains("data-flm-kind=\"text\""), "{html}");
    assert!(html.contains("data-flm-kind=\"code\""), "{html}");
    assert!(html.contains("data-flm-kind=\"a\""), "{html}");
    assert!(html.contains("data-flm-lang=\"python\""), "{html}");
    assert!(html.contains("data-flm-lang=\"rust\""), "{html}");
    assert!(html.contains("<pre class=\"flm-pres-verbatim\""), "{html}");
    assert!(html.contains("\\alpha"), "{html}");
    assert_golden("verbatim-callables.html", &html);
}

#[test]
fn a_verbatim_macro_inside_another_commands_argument_is_written_where_it_stands() {
    // Verbatim content is read at the parse, under a parsing state of its own,
    // so it survives standing inside another command's argument: the element
    // is written inside that command's element, its content still untouched.
    let html = clean(&["\\textbf{Run \\verbcode+make -j8+ first.}\n"]);
    assert!(html.contains("data-flm-style=\"bold\""), "{html}");
    assert!(html.contains("make -j8"), "{html}");
    assert_golden("verbatim-in-argument.html", &html);
}

#[test]
fn a_verbatim_macro_inside_a_heading_is_written_inside_the_heading() {
    let html = clean(&["\\section{The \\verbcode+--jobs+ option}\\label{sec:jobs}\n"]);
    assert!(html.contains("<h1 class=\"flm-pres-block--heading\" id=\"sec-jobs\""), "{html}");
    assert!(html.contains("--jobs"), "{html}");
    assert_golden("verbatim-in-heading.html", &html);
}

// ============================================================================
// unattributed callables, and an element accepted as unrendered
// ============================================================================

#[test]
fn an_unattributed_macro_and_environment_are_shown_as_the_author_wrote_them() {
    // A macro or an environment no feature claims is not dropped and does not
    // stop the render: the author's own spelling is written into the page,
    // marked so that the stylesheet can show it as an error, and one
    // diagnostic is reported for each. An environment keeps its body, with its
    // `\begin` and `\end` lines around it as marker elements of their own.
    let run = render_over(
        &std_registry(),
        &["Before \\nosuchmacro after.\n\n\\begin{nosuchenv}\nA body.\n\\end{nosuchenv}\n"],
    );
    let html = fragment(&run);
    assert!(html.contains("data-flm-name=\"nosuchmacro\""), "{html}");
    assert!(html.contains("data-flm-name=\"nosuchenv\""), "{html}");
    assert!(html.contains("flm-pres-unknown-marker"), "{html}");
    assert_golden("unknown-callables.html", &html);
}

/// A feature declaring `\hole{…}` and no lowering rule for it, so that a
/// caller has something to accept as unrendered.
fn hole_feature() -> Arc<dyn FlmFeature> {
    common::pipeline::feature("t", |b: &mut FeatureBuilder<'_>| -> Result<(), BuildError> {
        b.element(macro_def("t", "hole", 1))
    })
}

#[test]
fn an_element_the_caller_accepted_as_unrendered_is_an_empty_span_naming_it() {
    // An element no chain of rules can turn into something this backend
    // writes would refuse the plan. A caller who wants the rendering anyway
    // accepts the element as a *hole*, per backend; each occurrence then
    // becomes an empty element naming what could not be written, which the
    // stylesheet shows as an error, and one Error diagnostic. The content that
    // could not be rendered never reaches the page: it stays in the
    // diagnostic.
    let mut builder = flm_core::core::registry::RegistryBuilder::new();
    builder.use_package(flm_core::stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(hole_feature());
    let registry = builder
        .build(&flm_core::core::registry::RegistryInputs::new())
        .unwrap_or_else(|error| {
            panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
        });
    let mut overrides = Overrides::none();
    overrides
        .unsupported
        .entry(BackendName::from(html::BACKEND))
        .or_default()
        .push(ElementName::from("t.hole"));
    let run = run_workflow(&registry, html::WORKFLOW, &["Before \\hole{x} after.\n"], &overrides);
    let diagnostic = expect(run.diagnostics().iter(), "flm.lowering.unsupported-element").once();
    assert_eq!(diagnostic.severity(), Severity::Error);
    let html = fragment(&run);
    assert!(html.contains("data-flm-element=\"t.hole\""), "{html}");
    assert!(!html.contains(">x<"), "{html}");
    assert_golden("hole-accepted.html", &html);
}

// ============================================================================
// the author's own definitions
// ============================================================================

#[test]
fn a_user_definition_used_in_text_reaches_the_page_expanded() {
    // No rule of the author-definition feature names this backend, so the
    // general expansion rewrite runs and the emitter never sees the use site:
    // what reaches the page is the definition's content, styled and escaped
    // like any other content.
    let registry = registry(StdcoreConfig::new().userdefs(
        UserdefsConfig::new().definitions(vec![
            Definition::macro_("libname", Vec::new()).text("\\textbf{FLM}"),
        ]),
    ));
    let run = render_over(&registry, &["The \\libname{} language.\n"]);
    assert_clean(&run);
    let html = fragment(&run);
    assert!(!html.contains("libname"), "{html}");
    assert!(html.contains("data-flm-style=\"bold\""), "{html}");
    assert_golden("userdefs-text.html", &html);
}

#[test]
fn a_user_definition_taking_arguments_reaches_the_page_with_them_substituted() {
    // An argument written at the use site is substituted into the definition's
    // content, and an argument the author left out takes the declared default.
    // Both are settled before lowering, so the page shows only the result.
    let registry = registry(StdcoreConfig::new().userdefs(
        UserdefsConfig::new().definitions(vec![
            Definition::macro_(
                "greet",
                vec![ArgDef::optional("greeting"), ArgDef::mandatory("name")],
            )
            .text("\\emph{#{greeting}}, #{name}!")
            .default("greeting", "Hello"),
        ]),
    ));
    let run = render_over(&registry, &["\\greet[Bonjour]{world} And \\greet{world}\n"]);
    assert_clean(&run);
    let html = fragment(&run);
    assert!(html.contains("Bonjour"), "{html}");
    assert!(html.contains("Hello"), "{html}");
    assert!(!html.contains("greet"), "{html}");
    assert_golden("userdefs-arguments.html", &html);
}

// ============================================================================
// comments, and a document of several trees
// ============================================================================

#[test]
fn a_comment_is_dropped_and_leaves_no_trace_in_the_page() {
    // A comment is the author's note to himself and is not content, so nothing
    // of it is written. It is not written as an HTML comment either: an HTML
    // comment is a place where document text could close the comment and start
    // markup, which is exactly the door this backend keeps shut.
    let html = clean(&["Before the note. %% a note the reader never sees\nAfter it.\n"]);
    assert!(!html.contains("note the reader"), "{html}");
    assert!(!html.contains("<!--"), "{html}");
    assert_golden("comments-dropped.html", &html);
}

#[test]
fn a_document_of_two_trees_writes_one_piece_of_html_for_each() {
    // A document is a set of trees, and the emit stage runs once per tree —
    // possibly on several threads — so each tree's HTML is a piece of its own.
    // The page assembly is what puts them in order; a fragment's golden shows
    // the pieces themselves, one line apart.
    let html = clean(&[
        "\\section{First tree}\\label{sec:first}\n\nIts text.\n",
        "The second tree, with \\emph{its own} content.\n",
    ]);
    assert!(html.contains("sec-first"), "{html}");
    assert!(html.contains("its own"), "{html}");
    assert_golden("trees-two.html", &html);
}

// ============================================================================
// the trust guarantee, from a document
// ============================================================================

#[test]
fn text_that_tries_to_break_out_of_every_context_it_can_reach_is_escaped_or_refused() {
    // The document a reviewer writes, under the configuration a reader of this
    // library gets by default: every byte that means something to an HTML
    // parser, in every place a document's own text reaches the page — running
    // text; a heading's title; a label, which becomes an identifier and the
    // address of every link to it; a title with no label, from which an
    // identifier is minted instead; the name of an environment no feature
    // claims, which becomes an attribute value; a language tag, likewise an
    // attribute value; and verbatim content, where the author's bytes are
    // copied as they stand.
    //
    // Nothing of it can leave the context it was written in. Text escapes `&`,
    // `<` and `>`. An attribute value escapes those three and the double quote
    // as well. An identifier is not escaped but **encoded**: the anchor
    // encoder keeps the characters an identifier may hold and replaces the
    // rest, ending the anchor with a short hash of the label so that two
    // labels differing only in refused characters stay two anchors — which is
    // why the `href` written from it is safe by construction. And nothing this
    // backend writes is a comment or a script a document could close.
    //
    // A `javascript:` address can only ever appear here as text a reader sees:
    // the address policy decides every address this backend writes, and no
    // address in this document comes from the document itself.
    let run = render_over(
        &std_registry(),
        &[
            "\\section{A title with < and > and & and </script> in it}\
             \\label{sec:a<b&c}\n\n\
             Text with < and > and & and </script> and javascript:alert(1) \
             and <tex-html> in it. See \\ref{sec:a<b&c}.\n\n\
             \\section{A title with no label: < & </script> javascript:}\n\n\
             \\verbcode[la<ng>&]+a < b & c </script> --> \\end+\n\n\
             \\begin{no<such&env}\nA body with < in it.\n\\end{no<such&env}\n",
        ],
    );
    let html = fragment(&run);
    // Nothing a parser would read as markup, a comment or a script's end.
    assert!(!html.contains("</script>"), "{html}");
    assert!(!html.contains("<tex-html>"), "{html}");
    assert!(!html.contains("<!--"), "{html}");
    assert!(!html.contains("-->"), "{html}");
    // The pieces a reader does see, escaped.
    assert!(html.contains("&lt;/script&gt;"), "{html}");
    assert!(html.contains("&lt;tex-html&gt;"), "{html}");
    assert!(html.contains("javascript:alert(1)"), "{html}");
    // The two identifiers: one encoded from the label, one minted from the
    // title of the heading that carries no label. Neither holds a byte an
    // attribute value or an address would have to escape, and the link the
    // `\ref` writes is the first of them with a `#` before it.
    let anchor = attribute_value(&html, "id=\"");
    assert!(!anchor.contains(['<', '>', '&', '"', ' ']), "{anchor}");
    assert!(html.contains(&format!("href=\"#{anchor}\"")), "{html}");
    // It is the only address the page holds: nothing the document typed
    // becomes one.
    assert_eq!(html.matches("href=\"").count(), 1, "{html}");
    assert!(!html.contains("href=\"javascript:"), "{html}");
    // The environment's name and the language tag reach attribute values with
    // their angle brackets and their ampersands escaped, so neither attribute
    // can be closed early.
    assert!(html.contains("data-flm-name=\"no&lt;such&amp;env\""), "{html}");
    assert!(html.contains("data-flm-lang=\"la&lt;ng&gt;&amp;\""), "{html}");
    assert_golden("escaping-breakout.html", &html);
}

/// The value of the first attribute of `html` written with the spelling
/// `prefix` (`id="`, say): everything from it up to the closing quote.
fn attribute_value(html: &str, prefix: &str) -> String {
    let rest = html.split_once(prefix).unwrap_or_else(|| panic!("no `{prefix}` in {html}")).1;
    rest.split('"').next().expect("a closing quote").to_string()
}

#[test]
fn a_straight_double_quote_reaching_an_attribute_value_is_escaped() {
    // The one byte the document above cannot carry. Under the default text
    // configuration the `blocks` feature turns a straight double quote into a
    // typographic one before this backend ever sees it, everywhere — inside a
    // verbatim callable's language tag as well — so a document written as a
    // reader writes one cannot put a straight `"` into an attribute value.
    // The typographic substitutions are therefore turned off for this
    // document alone, and for this one byte: a document that closed an
    // attribute early could write markup of its own, which is exactly what an
    // attribute value's escaping prevents.
    let plain = registry(StdcoreConfig::new().blocks(BlocksConfig::no_typography()));
    let html = fragment(&render_over(&plain, &["\\verbcode[la\"ng]+a \" b+\n"]));
    assert!(html.contains("data-flm-lang=\"la&quot;ng\""), "{html}");
    // In element content a double quote closes nothing, so it stays the byte
    // the author typed.
    assert!(html.contains("a \" b"), "{html}");
    assert_golden("escaping-attribute-quote.html", &html);
}
