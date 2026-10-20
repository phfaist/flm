//! Headings on the `latex-rendered` backend, end to end over the standard
//! package: `\flmxHeading` and `\flmxRuninHeading` with their keys (`kind`,
//! `level`, `toc`, `anchor`, `number`, `plain`, `alt`, and the run-in heading's
//! `stop`), headings of every standard kind numbered and not, out of the table
//! of contents, with an alternative title, with a formula in the title, of a
//! level above the standard ones, and the HTML and text output of a heading
//! with `alt=`, which the alternative title does not change.
//!
//! Goldens live under `tests/goldens/latex-rendered/`, in the shape of
//! `tests/latex_rendered.rs` (the document, then the style file, each under a
//! marker line); `tests/latex_rendered_compile.rs` compiles every one with
//! `lualatex`.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::dump::golden::assert_text_golden;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::latex_rendered::{
    DOCUMENT, LatexRenderedRenderOptions, WORKFLOW, style_artifact,
};
use flm_core::stdcore::backends::{html, text};
use flm_core::stdcore::features::headings::{HeadingDecl, HeadingNumbering, HeadingsConfig};

// --- helpers -------------------------------------------------------------------------------

/// The standard kinds with a `chapter` of level 0 in front of them, every
/// kind numbered, and the kinds of the levels `run_in` set run in.
fn headings(run_in: &[u8]) -> HeadingsConfig {
    let mut kinds = HeadingsConfig::new().headings;
    kinds.insert(0, HeadingDecl::new("chapter", 0));
    HeadingsConfig::new()
        .headings(kinds)
        .numbering(HeadingNumbering::AllNumbered)
        .run_in_levels(run_in.iter().copied())
}

/// The standard package under `headings`.
fn registry_of(headings: HeadingsConfig) -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().headings(headings))
}

/// The render request of every `latex-rendered` run of this file, with
/// `options` set.
fn overrides(options: OptionValues) -> Overrides {
    Overrides {
        render_options: options,
        ..Overrides::none()
    }
}

/// Runs the workflow over `text` with `registry` and `options`, asserting
/// that nothing was reported.
fn render_with(registry: &Arc<FlmRegistry>, text: &str, options: OptionValues) -> Run {
    let run = common::stdcore::run_workflow(registry, WORKFLOW, &[text], &overrides(options));
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
    run
}

/// The entry artifact's text.
fn document(run: &Run) -> String {
    let set = run.table(DOCUMENT).expect("declared").unwrap_or_else(|| {
        panic!("no latex-rendered output; diagnostics:\n{}", run.diagnostics().render_all())
    });
    let entry = set.entry().expect("the entry artifact");
    String::from_utf8(entry.as_bytes().expect("bytes").to_vec()).expect("UTF-8")
}

/// The style file's text and its path.
fn style(run: &Run) -> (String, String) {
    let set = run.table(DOCUMENT).expect("declared").expect("written");
    let style = style_artifact(set).expect("the style file");
    let text = String::from_utf8(style.as_bytes().expect("bytes").to_vec()).expect("UTF-8");
    let path = style.fixed_path.as_ref().map(|path| path.as_str().to_string()).unwrap_or_default();
    (text, path)
}

/// Asserts the run's document and style file against
/// `tests/goldens/latex-rendered/<name>`, each under a marker line.
fn assert_golden(name: &str, run: &Run) {
    let (style, path) = style(run);
    let text = format!(
        "% ===== latex-rendered:document =====\n{}% ===== latex-rendered:style {path} =====\n{style}",
        document(run)
    );
    let mut golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    golden.push("tests/goldens/latex-rendered");
    golden.push(name);
    assert_text_golden(golden, &text);
}

/// The lines of the document that begin with `prefix`.
fn lines_starting(run: &Run, prefix: &str) -> Vec<String> {
    document(run).lines().filter(|line| line.starts_with(prefix)).map(String::from).collect()
}

// --- block headings ------------------------------------------------------------------------

/// Headings of every standard kind and of a level above them, numbered and
/// not, out of the table of contents, with an alternative title holding
/// markup, with `&` in the title, with a formula in the title, and references
/// to them.
const HEADINGS: &str = "\
\\chapter{Opening}
\\section{Introduction}\\label{sec:intro}
See \\ref{sec:alt}, \\ref{sec:formula} and \\ref{sec:intro}.

\\section[alt={Short \\emph{title}}]{A long and \\emph{fancy}   title}\\label{sec:alt}
Text.

\\section[toc=false]{Not listed}
Text.

\\section*{Unnumbered}
Text.

\\subsection{Salt \\& pepper}
Text.

\\subsubsection{Third level}
Text.

\\paragraph{Fourth level}
Text.

\\subparagraph{Fifth level}
Text.

\\section{The formula \\(x^2 + y_1\\)}\\label{sec:formula}
Text.";

#[test]
fn headings_of_every_kind_write_their_keys_and_the_golden_holds_them() {
    let run = render_with(&registry_of(headings(&[])), HEADINGS, OptionValues::new());
    let headings = lines_starting(&run, "\\flmxHeading");
    assert_eq!(headings.len(), 10, "{headings:#?}");
    // A numbered heading: its number as a key and in the title, its plain text
    // holding the number.
    assert!(
        headings[1].starts_with(
            "\\flmxHeading[kind=section, level=1, toc=true, anchor=sec-intro, number={1}, \
             plain={1. Introduction}]{\\flmxSpan[role=flm.heading.number]{1}."
        ),
        "{}",
        headings[1]
    );
    // The level above the standard ones is written as it is.
    assert!(headings[0].starts_with("\\flmxHeading[kind=chapter, level=0, toc=true, "), "{}", headings[0]);
    // The alternative title as lowered content; the plain text with its runs
    // of white space collapsed.
    assert!(
        headings[2].contains("plain={2. A long and fancy title}, alt={Short \\flmxSpan[style=emph]{title}}]"),
        "{}",
        headings[2]
    );
    // Out of the table of contents; and unnumbered, with no `number` key.
    assert!(headings[3].contains("toc=false"), "{}", headings[3]);
    assert!(!headings[4].contains("number="), "{}", headings[4]);
    assert!(headings[4].contains("plain={Unnumbered}"), "{}", headings[4]);
    // `&` escaped in the plain text, which the bookmark reads.
    assert!(headings[5].contains("plain={3.1. Salt \\& pepper}"), "{}", headings[5]);
    // A formula in the title: the formula's source in the plain text, as text.
    assert!(headings[9].contains("plain={4. The formula x\\textasciicircum{}2 + y\\_1}"), "{}", headings[9]);
    assert_golden("headings.tex", &run);
}

#[test]
fn the_headings_chunk_is_in_the_style_file_once_a_heading_is_written() {
    let run = render_with(&registry_of(headings(&[])), "\\section{One}\nText.", OptionValues::new());
    let (file, _) = style(&run);
    assert!(file.contains("% flm.latex.def.flmx.headings"), "{file}");
    assert!(file.contains("\\NewDocumentCommand \\flmxHeading"), "{file}");
    let run = render_with(&registry_of(headings(&[])), "Text.", OptionValues::new());
    let (file, _) = style(&run);
    assert!(!file.contains("% flm.latex.def.flmx.headings"), "{file}");
}

// --- run-in headings -----------------------------------------------------------------------

/// Run-in headings with and without the stop, and a theorem's run-in
/// decoration.
const RUN_IN: &str = "\
\\section{Run in}
\\paragraph{A run-in title}\\label{sec:a}
Text after it, see \\ref{sec:a}.

\\paragraph{Ends in a question?}
Text after it.

\\subparagraph{}
Text after an empty title.

\\begin{theorem}[Pythagoras]\\noproofref
A statement.
\\end{theorem}";

#[test]
fn a_run_in_heading_writes_the_stop_as_a_key_unless_the_title_ends_in_punctuation() {
    let run = render_with(&registry_of(headings(&[4, 5])), RUN_IN, OptionValues::new());
    let latex = document(&run);
    assert!(
        latex.contains(
            "\\flmxDecoRunin[stop={.}]{\\flmxRuninHeading[kind=paragraph, level=4, toc=true, anchor=sec-a, \
             number={a}, plain={a. A run-in title}, stop={.}]{"
        ),
        "{latex}"
    );
    assert!(latex.contains("plain={b. Ends in a question?}]{"), "{latex}");
    assert!(!latex.contains("question?}, stop="), "{latex}");
    // A theorem's run-in decoration carries the stop too.
    assert!(latex.contains("\\flmxDecoRunin[stop={.}]{\\flmxSpan[role=flm.theorems.name]{Theorem"), "{latex}");
    assert_golden("headings-runin.tex", &run);
}

#[test]
fn an_empty_stop_option_writes_no_stop() {
    let options = OptionValues::new()
        .with(LatexRenderedRenderOptions { run_in_stop: String::new(), ..LatexRenderedRenderOptions::default() });
    let run = render_with(&registry_of(headings(&[4, 5])), RUN_IN, options);
    assert!(!document(&run).contains("stop="), "{}", document(&run));
}

// --- HTML and text -------------------------------------------------------------------------

#[test]
fn the_html_and_text_of_a_heading_with_alt_are_those_of_the_same_heading_without() {
    let registry = registry_of(headings(&[]));
    let with = "\\section[alt={Short \\emph{title}}]{A long title}\\label{sec:a}\nSee \\ref{sec:a}.";
    let without = "\\section{A long title}\\label{sec:a}\nSee \\ref{sec:a}.";
    let page = |source: &str| {
        let run = common::stdcore::run_workflow(&registry, html::WORKFLOW, &[source], &Overrides::none());
        assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
        common::html::page(&run)
    };
    assert_eq!(page(with), page(without));
    assert!(!page(with).contains("Short"), "{}", page(with));
    let printed = |source: &str| {
        let run = common::stdcore::run_workflow(&registry, text::WORKFLOW, &[source], &Overrides::none());
        assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
        common::lowering::text(&run)
    };
    assert_eq!(printed(with), printed(without));
}

// --- the headings feature's definition piece --------------------------------------------------

#[test]
fn the_heading_rule_brings_in_the_number_span_s_piece() {
    use flm_core::stdcore::features::headings::latex_rendered::PIECE;
    let run = render_with(&registry_of(headings(&[])), "\\section{One}\nText.", OptionValues::new());
    let (text, _) = style(&run);
    assert!(text.contains(&format!("% {PIECE} --")), "{text}");
    assert!(text.contains("\\flmxDefineSpan{flm.heading.number}"), "{text}");
    // A document without a heading holds neither the chunk nor the piece.
    let run = render_with(&registry_of(headings(&[])), "Text.", OptionValues::new());
    assert!(!style(&run).0.contains(PIECE), "{}", style(&run).0);
}

/// A heading another feature writes (the title of the list of footnotes) needs
/// nothing of the headings feature: in a registry without it, the document
/// renders on this backend without an Error, its style file holding the
/// headings chunk and not the headings feature's piece.
#[test]
fn a_heading_another_feature_writes_renders_without_the_headings_feature() {
    use flm_core::stdcore::features::headings::latex_rendered::PIECE;
    let registry =
        common::stdcore::registry_over(common::stdcore::package_features_but(StdcoreConfig::new(), "headings"));
    let run = render_with(&registry, "Text.\\footnote{A note.}", OptionValues::new());
    assert!(document(&run).contains("\\flmxHeading"), "{}", document(&run));
    let (text, _) = style(&run);
    assert!(text.contains("% flm.latex.def.flmx.headings --"), "{text}");
    assert!(!text.contains(PIECE), "{text}");
}
