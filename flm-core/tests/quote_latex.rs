//! The `quote` feature on the `latex-recomposed` backend: every quotation
//! written back as the environment the author wrote, each section through
//! the hook of its kind, a line's indentation through `\flmIndent`, the
//! hooks and the environments `blockquote` and `address` provided by the
//! feature's definition piece, `\flmTabWidth` by the family's, and the three
//! errors reported as on every backend. The
//! goldens are compiled by `tests/latex_compile.rs` under both engines.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::latex_recomposed;
use flm_core::stdcore::features::quote::{QuoteConfig, QuoteEnvironment, QuoteSection};

use common::stdcore::{identifiers, run_workflow};

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The run of the `latex-recomposed` workflow over `document`.
fn run(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, latex_recomposed::WORKFLOW, &[document], &Overrides::none())
}

/// The recomposed LaTeX and the style file of `document`, whose run must
/// report nothing.
fn written(registry: &Arc<FlmRegistry>, document: &str) -> (String, String) {
    let run = run(registry, document);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    (common::latex::output(&run), common::latex::style(&run).0)
}

/// Tests 1 to 3 in one document, as the HTML suite writes them.
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

// ---------------------------------------------------------------------------------------------
// the environment and its sections (test 9)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_quotation_is_its_environment_with_one_hook_per_section() {
    let (output, _) = written(
        &std(),
        "\\begin{quote}\\text{To be or not to be.}\\attributed{Shakespeare}\\end{quote}",
    );
    assert!(
        output.contains(
            "\\begin{quote}\\flmQuoteText{To be or not to be.}\\flmQuoteAttributed{Shakespeare}\
             \\end{quote}"
        ),
        "{output}"
    );
}

#[test]
fn the_lines_are_joined_by_line_breaks_each_indentation_through_the_hook() {
    let (output, style) =
        written(&std(), "\\begin{quote}\\lines{  a  \\\\ \\indent b\\\\\\indent\\indent c}\\end{quote}");
    assert!(
        output.contains(
            "\\flmQuoteLines{a\\\\\\relax\n\\flmIndent{1\\flmTabWidth}b\\\\\\relax\n\\flmIndent{2\\flmTabWidth}c}"
        ),
        "{output}"
    );
    // The length is the family's, declared guarded on its name.
    assert!(
        style.contains(
            "\\ifcsname flmTabWidth\\endcsname\\else\\newlength{\\flmTabWidth}\
             \\setlength{\\flmTabWidth}{2em}\\fi"
        ),
        "{style}"
    );
}

#[test]
fn lines_with_no_indentation_need_no_tab_width() {
    let (output, style) = written(&std(), "\\begin{address}a\\\\b\\end{address}");
    assert!(output.contains("\\begin{address}\\flmQuoteLines{a\\\\\\relax\nb}\\end{address}"), "{output}");
    assert!(!style.contains("flmTabWidth}{2em}"), "{style}");
}

#[test]
fn bare_content_is_written_through_the_hook_of_its_kind_its_edges_as_they_stand() {
    let (output, _) = written(&std(), "\\begin{blockquote}\nOne.\n\nTwo.\n\\end{blockquote}");
    assert!(
        output.contains("\\begin{blockquote}\\flmQuoteBlock{One.\n\nTwo.}\\end{blockquote}"),
        "{output}"
    );
}

#[test]
fn the_piece_provides_the_hooks_and_the_two_environments() {
    let (output, style) = written(&std(), TESTS_ONE_TO_THREE);
    for hook in ["flmQuoteText", "flmQuoteLines", "flmQuoteAttributed", "flmQuoteBlock", "flmIndent"] {
        assert!(style.contains(&format!("\\ProvideDocumentCommand\\{hook}{{")), "{hook}: {style}");
    }
    for name in ["blockquote", "address"] {
        let definition = format!("\\ProvideDocumentEnvironment{{{name}}}{{}}");
        assert_eq!(style.matches(&definition).count(), 1, "{name}: {style}");
    }
    // `quote` is LaTeX's own, and no definition of the feature's is written
    // from its configuration.
    assert!(!style.contains("Environment{quote}"), "{style}");
    assert!(!style.contains("written from its configuration"), "{style}");
    assert!(output.contains("\\flmQuoteBlock{one line}"), "{output}");
    assert!(output.contains("\\flmQuoteBlock{One paragraph.\n\nAnother paragraph.}"), "{output}");
    let run = run(&std(), TESTS_ONE_TO_THREE);
    common::latex::assert_golden("quote-tests-1-3.tex", &run);
}

/// Lines whose first character a bare `\\` would read as its own — an
/// optional argument's `[`, a star, a `[` on the line after a comment — and
/// an empty first line, in a lines section and in an `address`. The goldens
/// are compiled by `tests/latex_compile.rs`, which reads every line back.
const LINE_EDGES: &str = "\\begin{quote}\\lines{First line\\\\ [chorus] sung\\\\ *starred line\\\\ %% a comment
[after a comment]}\\end{quote}

\\begin{quote}\\lines{\\\\ After an empty line}\\end{quote}

\\begin{address}
\\\\
Opening after an empty line
\\end{address}
";

#[test]
fn every_line_break_is_ended_so_the_next_line_is_never_its_argument() {
    let (output, _) = written(&std(), LINE_EDGES);
    assert!(
        output.contains("First line\\\\\\relax\n[chorus] sung\\\\\\relax\n*starred line\\\\\\relax\n"),
        "{output}"
    );
    assert!(output.contains("\\flmQuoteLines{\\\\\\relax\nAfter an empty line}"), "{output}");
    common::latex::assert_golden("quote-lines-edges.tex", &run(&std(), LINE_EDGES));
}

#[test]
fn a_configured_environment_is_written_under_its_name_and_declared_by_nobody() {
    let config = QuoteConfig::new()
        .environment("myquote", QuoteEnvironment::new([QuoteSection::Text, QuoteSection::Attributed]));
    let registry = common::stdcore::registry(StdcoreConfig::new().quote(config));
    let document = "\\begin{myquote}\\text{Mine.}\\attributed{Me}\\end{myquote}\n";
    let (output, style) = written(&registry, document);
    assert!(
        output.contains("\\begin{myquote}\\flmQuoteText{Mine.}\\flmQuoteAttributed{Me}\\end{myquote}"),
        "{output}"
    );
    assert!(!style.contains("myquote"), "{style}");
    common::latex::assert_golden("quote-myquote.tex", &run(&registry, document));
}

#[test]
fn every_argument_is_escaped_as_content() {
    let (output, _) = written(&std(), "\\begin{quote}\\text{50\\% of \\#1}\\attributed{A \\& B}\\end{quote}");
    assert!(output.contains("\\flmQuoteText{50\\% of \\#1}"), "{output}");
    assert!(output.contains("\\flmQuoteAttributed{A \\& B}"), "{output}");
}

// ---------------------------------------------------------------------------------------------
// the three errors, reported as on every backend
// ---------------------------------------------------------------------------------------------

#[test]
fn the_three_errors_are_reported_here_too_and_the_content_kept() {
    let registry = std();
    let bare = run(&registry, "\\begin{quote}Bare text.\\text{a}\\end{quote}");
    assert_eq!(identifiers(&bare), ["flm.quote.bare-content"]);
    let output = common::latex::output(&bare);
    assert!(output.contains("\\begin{quote}Bare text.\\flmQuoteText{a}\\end{quote}"), "{output}");

    let nested = run(&registry, "\\begin{quote}\\lines{\\text{x}}\\end{quote}");
    assert_eq!(identifiers(&nested), ["flm.quote.nested-section"]);
    assert!(common::latex::output(&nested).contains("\\flmQuoteLines{x}"), "{}", common::latex::output(&nested));

    let mid = run(&registry, "\\begin{quote}\\lines{a \\indent b}\\end{quote}");
    assert_eq!(identifiers(&mid), ["flm.quote.indent-mid-line"]);
    assert!(common::latex::output(&mid).contains("\\flmQuoteLines{a b}"), "{}", common::latex::output(&mid));
}
