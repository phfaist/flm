//! Quotations on the `latex-rendered` backend, end to end over the standard
//! package: the documents `tests/quote_html.rs` renders (the quotation
//! sections of tests one to three, every environment, an address's indented
//! lines, environments a host configures), each a golden under
//! `tests/goldens/latex-rendered/` holding the document and the style file,
//! which `tests/latex_rendered_compile.rs` compiles with `lualatex`; and the
//! quote feature's definition piece, which the feature's style identifier
//! brings into the style file.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::quote::latex_rendered::PIECE;
use flm_core::stdcore::features::quote::{QuoteConfig, QuoteEnvironment, QuoteSection};

use common::latex_rendered::{assert_golden, document, render, style};

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The documents of the Python product's quote tests one to three: every
/// quotation section, a `blockquote`, a block section of one line, an
/// address with an indented line.
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
fn the_sections_of_tests_one_to_three_and_the_style_file_holds_the_piece() {
    let run = render(&std(), &[TESTS_ONE_TO_THREE], &Overrides::none());
    let written = document(&run);
    for role in ["text", "lines", "block", "attributed"] {
        assert!(written.contains(&format!("[role=flm.quote.{role}]")), "{written}");
    }
    assert!(written.contains("\\begin{flmxBlock}[role=flm.quote, kinds={address}]"), "{written}");
    let (text, _) = style(&run);
    assert!(text.contains(&format!("% {PIECE} --")), "{text}");
    assert_golden("quote-tests-1-3.tex", &run);
}

#[test]
fn every_environment_is_a_quotation_whatever_its_name() {
    let run = render(
        &std(),
        &["\\begin{blockquote}One.\n\nTwo.\\end{blockquote}\n\n\
           \\begin{quote}\\block{one line}\\lines{a\\\\b}\\end{quote}\n\n\
           \\begin{address}123 Main St\\\\City\\end{address}"],
        &Overrides::none(),
    );
    for name in ["blockquote", "quote", "address"] {
        assert!(document(&run).contains(&format!("[role=flm.quote, kinds={{{name}}}]")), "{}", document(&run));
    }
    assert_golden("quote-environments.tex", &run);
}

#[test]
fn environments_a_host_configures_are_quotations() {
    let config = QuoteConfig::new()
        .environment("myquote", QuoteEnvironment::new([QuoteSection::Text]))
        .environment("aside", QuoteEnvironment::new([]).bare(QuoteSection::Block));
    let registry = common::stdcore::registry(StdcoreConfig::new().quote(config));
    let run = render(
        &registry,
        &["\\begin{myquote}\\text{Mine.}\\end{myquote}\n\n\\begin{aside}Aside.\\end{aside}\n"],
        &Overrides::none(),
    );
    assert!(document(&run).contains("[role=flm.quote, kinds={myquote}]"), "{}", document(&run));
    assert_golden("quote-host-environments.tex", &run);
}

#[test]
fn a_document_without_a_quotation_does_not_hold_the_piece() {
    let run = render(&std(), &["A paragraph.\n"], &Overrides::none());
    assert!(!style(&run).0.contains(PIECE), "{}", style(&run).0);
}
