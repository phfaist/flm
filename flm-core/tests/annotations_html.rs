//! The `annotations` feature on the HTML page: a highlight and a comment as a
//! span of their role in the flow of the text, a block piece as a `<div>` of
//! the role, and the initials' margin decoration written where its mark
//! stands — inside a paragraph, inside a list entry, in a heading's title, at
//! the start of a block piece; the feature's stylesheet, required where an
//! annotation is written; and the hidden render option. The goldens this file
//! writes are parsed by `tests/html_validity.rs` like every HTML golden.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{DefContent, FlmRegistry};
use flm_core::core::run::Run;
use flm_core::dump::golden::assert_text_golden;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::html::DEFINITIONS_TABLE;
use flm_core::stdcore::features::annotations::{html, AnnotationsConfig, AnnotationsRenderOptions, Annotator};
use flm_core::stdcore::StdcoreConfig;

use common::stdcore::{identifiers, run_workflow};

/// The two annotators these tests use: `phf` with the initials `PhF`,
/// annotator 0, and `abc` with none, annotator 1.
fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().annotations(
        AnnotationsConfig::new()
            .annotator(Annotator::new("phf").initials("PhF"))
            .annotator(Annotator::new("abc")),
    ))
}

/// `document` rendered through the `html` workflow under `overrides`; any
/// diagnostic is a test failure.
fn render_under(document: &str, overrides: &Overrides) -> Run {
    let run = run_workflow(&registry(), "html", &[document], overrides);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    run
}

/// The HTML of `document`'s tree, without the page around it.
fn html(document: &str) -> String {
    common::html::fragment(&render_under(document, &Overrides::none()))
}

/// The opening tag of a highlight of `phf`, annotator 0.
const PHF_HIGHLIGHT: &str =
    "<span class=\"flm-pres-span flm-annotations-highlight\" data-flm-kind=\"phf annotator-0\">";

/// The initials of `phf` in their margin decoration, as written at the mark.
const PHF_INITIALS: &str = "<span class=\"flm-pres-deco-margin\"><span class=\"flm-pres-span \
                            flm-annotations-initials\" data-flm-kind=\"phf annotator-0\">PhF</span></span>";

// ---------------------------------------------------------------------------------------------
// the margin decoration at its mark (P13; test 14)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_initials_are_written_at_the_mark_inside_the_paragraph() {
    // A second paragraph, so that the fragment is laid out as blocks.
    let out = html("word \\phf{passage} word\n\nNext.");
    assert!(out.contains(&format!("<p class=\"flm-pres-par\">word {PHF_HIGHLIGHT}{PHF_INITIALS}passage</span> word</p>")), "{out}");
    assert!(!out.contains("<aside"), "{out}");
}

#[test]
fn a_comment_carries_its_initials_at_the_mark_as_a_highlight_does() {
    let out = html("word \\phf[note] word");
    assert!(
        out.contains(&format!(
            "word <span class=\"flm-pres-span flm-annotations-comment\" data-flm-kind=\"phf \
             annotator-0\">{PHF_INITIALS}note</span> word"
        )),
        "{out}"
    );
}

#[test]
fn a_cut_mark_s_pieces_say_whether_a_piece_follows_and_precedes() {
    let out = html("x \\phf[a\n\nb\n\nc] y \\phf{d\n\ne} and \\phf[f] z");
    let comment = "<span class=\"flm-pres-span flm-annotations-comment\" data-flm-kind=\"phf annotator-0";
    let highlight = "<span class=\"flm-pres-span flm-annotations-highlight\" data-flm-kind=\"phf annotator-0";
    assert!(out.contains(&format!("{comment} mark-continues\">{PHF_INITIALS}a</span>")), "{out}");
    assert!(out.contains(&format!("{comment} mark-continues mark-continued\">b</span>")), "{out}");
    assert!(out.contains(&format!("{comment} mark-continued\">c</span>")), "{out}");
    assert!(out.contains(&format!("{highlight} mark-continues\">{PHF_INITIALS}d</span>")), "{out}");
    assert!(out.contains(&format!("{highlight} mark-continued\">e</span>")), "{out}");
    // A mark that was not cut carries neither token.
    assert!(out.contains(&format!("{comment}\">{PHF_INITIALS}f</span>")), "{out}");
}

#[test]
fn inside_a_list_entry_the_initials_leave_the_entry_as_it_is() {
    // The entry's element holds what it holds without the decoration — its
    // paragraph — and the decoration stands inside that paragraph, so the
    // entry's grid of a label column and a content column is unchanged.
    let with = html("\\begin{enumerate}\\item \\phf{x} y\\end{enumerate}");
    let without = html("\\begin{enumerate}\\item \\abc{x} y\\end{enumerate}");
    let shape = |out: &str| {
        out.replace(PHF_INITIALS, "")
            .replace("data-flm-kind=\"phf annotator-0\"", "KINDS")
            .replace("data-flm-kind=\"abc annotator-1\"", "KINDS")
    };
    assert_eq!(shape(&with), shape(&without), "{with}");
    assert!(with.contains(&format!("{PHF_HIGHLIGHT}{PHF_INITIALS}x</span> y")), "{with}");
}

#[test]
fn in_a_heading_title_the_initials_stand_inside_the_heading() {
    let out = html("\\section{Title \\phf{draft}}\n\nText.");
    let heading = out.find("<h1").expect("the heading is written");
    let end = out.find("</h1>").expect("the heading is closed");
    let initials = out.find(PHF_INITIALS).expect("the initials are written");
    assert!(heading < initials && initials < end, "{out}");
}

#[test]
fn a_block_piece_is_a_div_of_the_role_its_initials_first_when_it_is_the_first_piece() {
    let out = html("\\phf \\begin{itemize}\\item a\\end{itemize} here\\endphf");
    assert!(
        out.contains(&format!(
            "<div class=\"flm-pres-block flm-annotations-highlight\" data-flm-kind=\"phf \
             annotator-0 mark-continues\">{PHF_INITIALS}"
        )),
        "{out}"
    );
    // The piece after the list continues the mark, with no initials of its own.
    assert_eq!(out.matches(PHF_INITIALS).count(), 1, "{out}");
    assert!(
        out.contains(
            "<span class=\"flm-pres-span flm-annotations-highlight\" data-flm-kind=\"phf annotator-0 mark-continued\"> \
             here</span>"
        ),
        "{out}"
    );
}

#[test]
fn the_placement_golden() {
    let out = html(PLACEMENT);
    assert!(!out.contains("<aside"), "{out}");
    assert_text_golden(common::html::golden_path("annotations-placement.html"), &out);
}

/// The places the initials are written at their mark: a paragraph, a list
/// entry, a heading's title, and a cut annotation whose middle piece is a
/// block.
const PLACEMENT: &str = "\\section{A title \\phf{marked} \\phf[with a remark]}

Some text \\phf{a passage} and \\phf[a remark], then \\abc{another}.

\\begin{itemize}
\\item An entry \\phf{marked} here.
\\item \\phf[A remark opening the entry.]
\\end{itemize}

\\phf A passage \\begin{enumerate}\\item holding a list\\end{enumerate} and more.\\endphf
";

// ---------------------------------------------------------------------------------------------
// the stylesheet and the page (P15; test 14)
// ---------------------------------------------------------------------------------------------

/// The render request hiding every annotation.
fn hidden() -> Overrides {
    Overrides {
        render_options: OptionValues::new().with(AnnotationsRenderOptions { hidden: true }),
        ..Overrides::none()
    }
}

/// Whether the page of `run` requires the feature's stylesheet.
fn requires_style(run: &Run) -> bool {
    run.table(DEFINITIONS_TABLE)
        .expect("the definitions table is declared")
        .expect("the definitions table is written")
        .definitions
        .iter()
        .any(|row| row.id.as_str() == html::STYLE)
}

#[test]
fn the_stylesheet_is_required_where_an_annotation_is_written_and_reaches_the_page() {
    let run = render_under("word \\abc[note] word", &Overrides::none());
    assert!(requires_style(&run));
    let page = common::html::page(&run);
    let DefContent::Inline(css) = &html::stylesheet().content[0] else { panic!("inline") };
    assert!(page.contains(css.trim_end()), "{page}");
    // A document holding no annotation needs none of it.
    assert!(!requires_style(&render_under("Plain text.", &Overrides::none())));
}

#[test]
fn the_page_of_test_11() {
    let run = render_under(TEST_11, &Overrides::none());
    let page = common::html::page(&run);
    assert!(page.contains(PHF_HIGHLIGHT), "{page}");
    assert!(page.contains("<span class=\"flm-pres-span flm-annotations-comment\" data-flm-kind=\"phf annotator-0\">"), "{page}");
    assert!(page.contains("<span class=\"flm-pres-span flm-annotations-highlight\" data-flm-kind=\"abc annotator-1\">x</span>"), "{page}");
    assert!(page.contains("<div class=\"flm-pres-block flm-annotations-highlight\" data-flm-kind=\"phf annotator-0 mark-continues mark-continued\">"), "{page}");
    assert_text_golden(common::html::golden_path("annotations.page.html"), &page);
}

// ---------------------------------------------------------------------------------------------
// hidden (test 12 on the page)
// ---------------------------------------------------------------------------------------------

#[test]
fn hidden_a_highlight_is_its_passage_and_a_comment_nothing() {
    let run = render_under("word \\phf{passage} and \\phf[note] word\n\nNext.", &hidden());
    let out = common::html::fragment(&run);
    assert!(out.contains("<p class=\"flm-pres-par\">word passage and  word</p>"), "{out}");
    assert!(!out.contains("flm-annotations") && !out.contains("flm-pres-deco-margin"), "{out}");
    assert!(!requires_style(&run));
}

#[test]
fn hidden_the_paragraphs_and_the_block_of_a_cut_annotation_stay() {
    let out = common::html::fragment(&render_under(
        "Some text \\phf{A.\n\nB.} and \\phf[C.\n\nD.] end\n\n\\phf see \\begin{itemize}\\item a\\end{itemize} here\\endphf",
        &hidden(),
    ));
    // The highlight's paragraphs are kept, and the comment's break stays
    // between the text around it.
    assert!(
        out.contains(
            "<p class=\"flm-pres-par\">Some text A.</p>\n<p class=\"flm-pres-par\">B. and </p>\n\
             <p class=\"flm-pres-par\"> end</p>\n<p class=\"flm-pres-par\">see </p>\n<ul"
        ),
        "{out}"
    );
    assert!(!out.contains("flm-annotations"), "{out}");
}

#[test]
fn hidden_a_comment_standing_as_a_paragraph_leaves_no_paragraph() {
    let out = common::html::fragment(&render_under("Text.\n\n\\phf[A remark.]\n\nMore.", &hidden()));
    assert_eq!(out.matches("<p").count(), 2, "{out}");
    assert!(out.contains("<p class=\"flm-pres-par\">Text.</p>\n<p class=\"flm-pres-par\">More.</p>"), "{out}");
}

#[test]
fn hidden_the_middle_piece_of_a_comment_cut_in_three_leaves_no_paragraph() {
    let out = common::html::fragment(&render_under("x \\phf[a\n\nb\n\nc] y", &hidden()));
    assert!(!out.contains("<p class=\"flm-pres-par\"></p>"), "{out}");
    assert!(out.contains("<p class=\"flm-pres-par\">x </p>\n<p class=\"flm-pres-par\"> y</p>"), "{out}");
}

#[test]
fn the_hidden_page_golden() {
    let page = common::html::page(&render_under(TEST_11, &hidden()));
    assert_text_golden(common::html::golden_path("annotations-hidden.page.html"), &page);
}

/// The cases of test 11: every form, both annotators, a cut comment and a cut
/// highlight, a list inside a passage.
const TEST_11: &str = "\\phf{passage} and \\phf passage\\endphf and \\abc{x} and \\phf[note] and \\phf{x}[y].

Some text \\phf[A comment.

With multiple paragraphs.] More text.

Some text \\phf{A highlight.

With multiple paragraphs.} More text.

\\phf see \\begin{itemize}\\item a\\end{itemize} here \\endphf
";
