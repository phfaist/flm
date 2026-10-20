//! The heading key `alt=` (`flm_core::stdcore::features::headings`): the
//! alternative title a heading gives a table of contents. What the sectioning
//! aspect answers for an entry (`Sectioning::toc_title`), with and without
//! `alt=`; what the reader `heading_alt` returns; a later `alt=` replacing an
//! earlier one; the heading itself and a reference to it keeping the full
//! title; `alt` written without a value the Error `flm.heading.missing-value`;
//! and `pdfbookmark` still the Warning `flm.heading.key-not-implemented`.
//!
//! The table of contents itself, which shows the alternative title in its
//! entries, is tested in `tests/toc.rs`; the relay of `alt` to the recomposed
//! LaTeX in `tests/latex_recomposed.rs`.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::lang::Flm;
use flm_core::lang::copy::WrittenPiece;
use flm_core::lang::text::plain_text;
use flm_core::model::aspects::{ContentRef, Sectioning};
use flm_core::model::element::element_of;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::headings::{HEADING, TITLE_ARG, heading_alt};
use techy::core::node::{NodeRef, NodeTree};
use techy::error::Severity;

use common::lowering::text as printed;
use common::stdcore::{registry, reports, run_workflow};

// --- helpers -------------------------------------------------------------------------------

/// The standard package.
fn std() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// The first heading node of a parse of `text`, with what the test does with it.
fn with_heading<R>(text: &str, f: impl FnOnce(NodeRef<'_, Flm>) -> R) -> R {
    let registry = std();
    let fragment = common::parse(registry.lang(), text);
    let heading = common::callables(&fragment)
        .into_iter()
        .find(|node| element_of(node).name == HEADING)
        .expect("a heading");
    f(heading)
}

/// What the sectioning aspect answers for an entry of the first heading of `text`.
fn toc_title(text: &str) -> Option<ContentRef> {
    with_heading(text, |node| {
        element_of(&node).aspect::<dyn Sectioning>().expect("sectioning").toc_title(&node)
    })
}

/// The synthesized tree the sectioning aspect answers for the first heading of
/// `text`; any other answer is a test failure.
fn synthesized(text: &str) -> Arc<NodeTree<Flm>> {
    match toc_title(text) {
        Some(ContentRef::Synthesized(tree)) => tree,
        other => panic!("{text}: the aspect answered {other:?}"),
    }
}

/// The names of the callable nodes of `tree`, in document order.
fn callable_names(tree: &NodeTree<Flm>) -> Vec<String> {
    tree.root().descendants().filter(|node| node.is_callable()).filter_map(|node| node.name()).map(String::from).collect()
}

/// The diagnostics of a run of `text` under the `text` workflow, as
/// `(identifier, severity, message)`, with the printed text.
fn run_text(text: &str) -> (Vec<(String, Severity, String)>, String) {
    let run = run_workflow(&std(), text::WORKFLOW, &[text], &Overrides::none());
    let reported = run
        .diagnostics()
        .iter()
        .map(|d| (d.identifier().to_string(), d.severity(), d.message().to_string()))
        .collect();
    (reported, printed(&run))
}

// --- what the aspect answers ---------------------------------------------------------------

#[test]
fn without_alt_an_entry_shows_the_title_argument() {
    let title = Some(ContentRef::ArgumentNamed(TITLE_ARG.to_string()));
    assert_eq!(toc_title("\\section{A title}"), title);
    assert_eq!(toc_title("\\subsubsection*{A title}"), title);
    // Whether the heading is listed is the aspect's other question.
    assert_eq!(toc_title("\\section[toc=false]{A title}"), title);
    assert_eq!(toc_title("\\section[alt]{A title}"), title);
}

#[test]
fn with_alt_an_entry_shows_a_copy_of_the_alternative_title() {
    for text in [
        "\\section[alt={Short}]{A long title}",
        "\\section[alt=Short]{A long title}",
        "\\section[toc=false, alt= Short ]{A long title}",
        "\\section*[alt={Short}]{A long title}",
    ] {
        let tree = synthesized(text);
        assert_eq!(plain_text(&tree), "Short", "{text}");
    }
    // Whitespace around a braced value: the group is still read as its content.
    for text in [
        "\\section[alt= {Short \\emph{x}} ]{A long title}",
        "\\section[toc=false, alt= {Short \\emph{x}}]{A long title}",
    ] {
        let tree = synthesized(text);
        assert_eq!(plain_text(&tree), "Short x", "{text}");
        assert_eq!(callable_names(&tree), ["emph"], "{text}");
    }
}

#[test]
fn an_alternative_title_may_hold_markup_and_may_be_empty() {
    let tree = synthesized("\\section[alt={Short \\emph{title}}]{A long title}");
    assert_eq!(plain_text(&tree), "Short title");
    assert_eq!(callable_names(&tree), ["emph"]);

    let empty = synthesized("\\section[alt={}]{A long title}");
    assert_eq!(plain_text(&empty), "");
    assert!(callable_names(&empty).is_empty());
}

#[test]
fn a_later_alt_replaces_an_earlier_one_and_one_without_a_value_changes_nothing() {
    assert_eq!(plain_text(&synthesized("\\section[alt=A, alt=B]{T}")), "B");
    assert_eq!(plain_text(&synthesized("\\section[alt=A, alt]{T}")), "A");
}

// --- the reader --------------------------------------------------------------------------

#[test]
fn the_reader_returns_the_pieces_of_the_last_alt_as_written() {
    for text in [
        "\\section[alt={Short \\emph{title}}]{T}",
        "\\section[alt= {Short \\emph{title}} ]{T}",
        "\\section[toc=false, alt= {Short \\emph{title}}]{T}",
    ] {
        with_heading(text, |node| {
            let pieces = heading_alt(&node).expect("an alternative title");
            let names: Vec<Option<&str>> = pieces
                .iter()
                .map(|piece| match piece {
                    WrittenPiece::Node(node) => node.name(),
                    WrittenPiece::Text(_) => None,
                })
                .collect();
            assert_eq!(names, [None, Some("emph")], "{text}: {pieces:?}");
        });
    }
    with_heading("\\section[alt={}]{T}", |node| {
        assert!(heading_alt(&node).expect("an alternative title").is_empty());
    });
    for text in ["\\section{T}", "\\section[toc=false]{T}", "\\section[alt]{T}"] {
        with_heading(text, |node| assert!(heading_alt(&node).is_none(), "{text}"));
    }
}

// --- the heading and a reference keep the full title ---------------------------------------

#[test]
fn the_heading_and_a_reference_to_it_show_the_full_title() {
    let (reported, out) =
        run_text("\\section[alt={Short}]{A long title}\\label{sec:a}\n\nSee \\ref{sec:a}.\n");
    assert!(reported.is_empty(), "{reported:?}");
    assert!(out.starts_with("A long title\n"), "{out}");
    assert!(out.trim_end().ends_with("See A long title."), "{out}");
    assert!(!out.contains("Short"), "{out}");
}

// --- the diagnostics -------------------------------------------------------------------------

#[test]
fn alt_without_a_value_is_an_error_and_the_heading_is_written() {
    let (reported, out) = run_text("\\section[alt]{A title}\n");
    assert_eq!(
        reported,
        [(
            String::from("flm.heading.missing-value"),
            Severity::Error,
            String::from("the heading option `alt` must be given a value")
        )]
    );
    assert!(out.starts_with("A title\n"), "{out}");
}

#[test]
fn pdfbookmark_is_still_reserved_and_alt_is_not() {
    let (reported, _) = run_text("\\section[alt={Short}, pdfbookmark=B]{A title}\n");
    assert_eq!(
        reported,
        [(
            String::from("flm.heading.key-not-implemented"),
            Severity::Warning,
            String::from("the heading option `pdfbookmark` is not implemented yet")
        )]
    );
    let (reported, _) = run_text("\\section[color=red]{A title}\n");
    assert_eq!(
        reported,
        [(
            String::from("flm.heading.unknown-key"),
            Severity::Error,
            String::from("`color` is not a heading option; the heading options are `numbered`, `toc` and `alt`")
        )]
    );
    // `reports` is the shape the other heading tests compare.
    let run = run_workflow(&std(), text::WORKFLOW, &["\\section[alt={}]{A title}\n"], &Overrides::none());
    assert!(reports(&run).is_empty(), "{}", run.diagnostics().render_all());
}

// --- `numbered=` and `toc=` written as a braced value with whitespace around it -------------

/// What the heading of `text` says about itself, and what a render reports.
fn occurrence_and_reports(text: &str) -> (flm_core::stdcore::features::headings::HeadingOccurrence, Vec<(String, Severity, String)>) {
    let occurrence = with_heading(text, |node| flm_core::stdcore::features::headings::heading_options(&node, None));
    (occurrence, run_text(text).0)
}

#[test]
fn numbered_written_as_a_braced_value_with_whitespace_around_it_is_read() {
    for text in ["\\section[numbered= {false} ]{T}", "\\section[numbered={false}]{T}", "\\section[numbered= false ]{T}"] {
        let (occurrence, reports) = occurrence_and_reports(text);
        assert!(!occurrence.numbered, "{text}");
        assert!(occurrence.toc, "{text}");
        assert_eq!(reports, [], "{text}");
    }
}

#[test]
fn toc_written_as_a_braced_value_with_whitespace_around_it_is_read() {
    for text in ["\\section[toc= {false} ]{T}", "\\section[toc={false}]{T}", "\\section[toc= false ]{T}"] {
        let (occurrence, reports) = occurrence_and_reports(text);
        assert!(!occurrence.toc, "{text}");
        assert!(occurrence.numbered, "{text}");
        assert_eq!(reports, [], "{text}");
    }
}
