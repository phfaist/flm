//! The style marks of the text output (`flm_core::stdlib::backends::text`,
//! `TextOutput::styles`) over the standard registry: a section heading,
//! `\emph`, `\textbf`, `\texttt`, inline verbatim, a link outside the document
//! and a reference inside it, the document's title and a footnote (both
//! printed as pieces, where position marks are not recorded and style marks
//! are), a run-in heading and a list label (printed as flattened text), a
//! deep heading (its `#` prefix and its number inside its mark), the depth of
//! a heading and the marks of the rule under it and of the `#` prefix, a
//! verbatim block, and the projection's `styles` key.

mod common;

use std::sync::Arc;

use techy::serialize::SerialValue;

use flm_core::core::data::table::Table;
use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Run, RunOptions};
use flm_core::dump::project::{ProjectOptions, Projector};
use flm_core::model::options::OptionValues;
use flm_core::stdlib::StdlibConfig;
use flm_core::stdlib::backends::text::{self, TEXT_OUTPUT, TextOutput, TextStyle};
use flm_core::stdlib::features::headings::{HeadingNumbering, HeadingsConfig};
use flm_core::stdlib::features::metadata::{DocMeta, MetaText};

use common::pipeline;

const DOCUMENT: &str = "\
\\section{Introduction}\\label{sec:intro}

Some \\emph{emphasized} and \\textbf{bold} text, \\texttt{code} and \\verbcode+a{b+, \
\\href{https://example.com}{the site} and \\ref{sec:intro}.\\footnote{A \\textbf{strong} note.}
";

fn registry() -> Arc<FlmRegistry> {
    common::stdlib::registry(StdlibConfig::new())
}

/// The standard registry with the paragraph headings run in.
fn run_in_registry() -> Arc<FlmRegistry> {
    common::stdlib::registry(StdlibConfig::new().headings(HeadingsConfig::new().run_in_levels([4, 5])))
}

/// A run of the `text` workflow over `text`, the document's title set to
/// `title` when given.
fn run(registry: &Arc<FlmRegistry>, text: &str, title: Option<&str>) -> Run {
    let options = match title {
        Some(title) => OptionValues::new().with(DocMeta::default().title(MetaText::new(title))),
        None => OptionValues::new(),
    };
    let doc = pipeline::doc_with_options(registry, &[text], options);
    let plan = common::stdlib::plan(registry, text::WORKFLOW, &Overrides::none());
    pipeline::run(&doc, &plan, &RunOptions::sequential())
}

fn output(run: &Run) -> &TextOutput {
    run.table(TEXT_OUTPUT).expect("declared").unwrap_or_else(|| {
        panic!("no text output; diagnostics:\n{}", run.diagnostics().render_all())
    })
}

/// Each style mark as its style and the text it covers.
fn styles(output: &TextOutput) -> Vec<(TextStyle, &str)> {
    output.styles.iter().map(|mark| (mark.style.clone(), output.style_slice(mark))).collect()
}

fn heading(depth: u8) -> TextStyle {
    TextStyle::Heading { depth }
}

fn rule(depth: u8) -> TextStyle {
    TextStyle::HeadingRule { depth }
}

fn link(href: &str) -> TextStyle {
    TextStyle::Link { href: String::from(href) }
}

#[test]
fn each_styled_node_gets_a_style_mark_over_its_printed_content() {
    let registry = registry();
    let run = run(&registry, DOCUMENT, Some(r"The \emph{Title}"));
    assert!(!run.diagnostics().has_errors(), "{}", run.diagnostics().render_all());
    let output = output(&run);
    let styles = styles(output);
    let text = output.text.as_str();
    let found = |style: &TextStyle, slice: &str| {
        styles.iter().any(|(s, covered)| s == style && *covered == slice)
    };
    for (style, slice) in [
        (heading(0), "Introduction"),
        (TextStyle::Emphasis, "emphasized"),
        (TextStyle::Bold, "bold"),
        (TextStyle::Monospace, "code"),
        (TextStyle::Verbatim, "a{b"),
        (link("https://example.com"), "the site"),
        // Inside the pieces: the title before the content, the note after it.
        (TextStyle::Emphasis, "Title"),
        (TextStyle::Bold, "strong"),
    ] {
        assert!(found(&style, slice), "{style:?} over `{slice}` in {styles:?}\n{text}");
    }
    // The link's address, printed after its content, is outside its mark.
    assert!(text.contains("the site <https://example.com>"), "{text}");
    // The reference is a link inside the document.
    assert!(
        styles.iter().any(|(style, _)| matches!(style, TextStyle::Link { href } if href.starts_with('#'))),
        "{styles:?}"
    );
    // The rule under the heading carries a mark of its own, not the
    // heading's.
    assert!(found(&rule(0), "============"), "{styles:?}\n{text}");
    assert!(
        styles.iter().all(|(style, covered)| *style == rule(0) || !covered.contains("====")),
        "{styles:?}"
    );
    // The marks come in the order the printer reached their nodes.
    let starts: Vec<usize> = output.styles.iter().map(|mark| mark.bytes.start).collect();
    assert!(starts.windows(2).all(|pair| pair[0] <= pair[1]), "{starts:?}");
    // The title's mark is in the piece printed before the content, where no
    // position mark is recorded.
    let content = text.find("Introduction").expect("the heading");
    let title = output
        .styles
        .iter()
        .find(|mark| output.style_slice(mark) == "Title")
        .expect("the title's mark");
    assert!(title.bytes.end < content, "{text}");
    assert!(output.marks.iter().all(|mark| mark.bytes.start >= content), "no position mark in the title");
    let note = output.styles.iter().find(|mark| output.style_slice(mark) == "strong").expect("the note's");
    assert!(note.bytes.start > content, "{text}");
}

#[test]
fn a_style_marks_lines_and_columns_match_its_bytes() {
    let registry = registry();
    let run = run(&registry, DOCUMENT, Some(r"The \emph{Title}"));
    let output = output(&run);
    for mark in &output.styles {
        let before = &output.text[..mark.bytes.start];
        let line = before.matches('\n').count() as u32;
        let col = before.rsplit('\n').next().unwrap_or("").chars().count() as u32;
        assert_eq!((mark.start.line, mark.start.col), (line, col), "{mark:?}");
    }
}

#[test]
fn a_run_in_heading_and_a_list_label_get_marks_inside_their_flattened_text() {
    let registry = run_in_registry();
    let document = "\\paragraph{Run in} The paragraph \\emph{goes} on.\n\n\
                    \\begin{description}\n\\item[\\textbf{Term}] The item.\n\\end{description}\n";
    let run = run(&registry, document, None);
    assert!(!run.diagnostics().has_errors(), "{}", run.diagnostics().render_all());
    let output = output(&run);
    let styles = styles(output);
    // The stop after the title is outside the heading's mark.
    assert!(output.text.contains("Run in. The paragraph"), "{}", output.text);
    // `\paragraph` is at level 4, three below the root level of a section.
    assert!(styles.contains(&(heading(3), "Run in")), "{styles:?}");
    assert!(styles.contains(&(TextStyle::Emphasis, "goes")), "{styles:?}");
    assert!(styles.contains(&(TextStyle::Bold, "Term")), "{styles:?}");
}

#[test]
fn a_deep_headings_prefix_and_number_and_a_verbatim_block_are_inside_their_marks() {
    let registry =
        common::stdlib::registry(StdlibConfig::new().headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered)));
    let document = "\\section{One}\n\n\\subsection{Two}\n\n\\subsubsection{Deep}\n\nText.\n\n\
                    \\begin{verbatimcode}\nlet x = 1;\nlet y = 2;\n\\end{verbatimcode}\n";
    let run = run(&registry, document, None);
    assert!(!run.diagnostics().has_errors(), "{}", run.diagnostics().render_all());
    let output = output(&run);
    let styles = styles(output);
    let text = output.text.as_str();
    // The deep heading's whole line, `#` prefix and number included, is its
    // title's mark.
    let line = text.lines().find(|line| line.ends_with("Deep")).unwrap_or_else(|| panic!("{text}"));
    assert!(line.starts_with("### 1.1.1"), "{text}");
    assert!(styles.contains(&(heading(2), line)), "{styles:?}\n{text}");
    assert!(styles.contains(&(TextStyle::Verbatim, "let x = 1;\nlet y = 2;")), "{styles:?}\n{text}");
}

#[test]
fn a_superscript_span_gets_no_mark() {
    // A note's mark is a link inside a superscript span: the link gets a
    // mark, the span none.
    let registry = registry();
    let run = run(&registry, "Some text.\\footnote{A note.}", None);
    let output = output(&run);
    assert!(output.text.contains("text.¹"), "{}", output.text);
    // The list of notes, a piece, has a heading of its own; the note's label
    // there is a superscript span too.
    assert_eq!(
        styles(output),
        [(link("#fn-1"), "¹"), (heading(1), "Footnotes"), (rule(1), "---------")]
    );
}

#[test]
fn the_projection_lists_the_style_marks_after_the_position_marks() {
    let registry = registry();
    let run = run(&registry, "A \\textbf{bold} \\href{https://example.com}{link}.", None);
    let output = output(&run);
    let projected = output.project(&Projector::new(ProjectOptions::new()).table_projection(&run));
    let SerialValue::Map(entries) = projected else { panic!("a map") };
    let keys: Vec<&str> = entries.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(keys, ["text", "marks", "styles"]);
    let SerialValue::List(styles) = &entries[2].1 else { panic!("a list") };
    assert_eq!(styles.len(), 2);
    let SerialValue::Map(bold) = &styles[0] else { panic!("a map") };
    let bold_keys: Vec<&str> = bold.iter().map(|(key, _)| key.as_str()).collect();
    assert_eq!(bold_keys, ["style", "bytes", "start", "end"]);
    assert_eq!(bold[0].1, SerialValue::Str("bold".into()));
    let SerialValue::Map(link) = &styles[1] else { panic!("a map") };
    assert_eq!(link[0].1, SerialValue::Str("link".into()));
    assert_eq!(link[1], (String::from("href"), SerialValue::Str("https://example.com".into())));
}

#[test]
fn a_headings_mark_holds_its_depth_and_its_rule_or_prefix_has_a_mark_of_its_own() {
    let registry = run_in_registry();
    let document = "\\section{One}\n\n\\subsection{Two}\n\n\\subsubsection{Deep}\n\n\
                    \\paragraph{Run in} The paragraph.\n";
    let run = run(&registry, document, None);
    assert!(!run.diagnostics().has_errors(), "{}", run.diagnostics().render_all());
    let output = output(&run);
    let text = output.text.as_str();
    assert!(text.starts_with("One\n===\n\nTwo\n---\n\n### Deep\n\nRun in. The paragraph."), "{text}");
    assert_eq!(
        styles(output),
        [
            (heading(0), "One"),
            (rule(0), "==="),
            (heading(1), "Two"),
            (rule(1), "---"),
            // The prefix, its space included, is inside the title's mark.
            (heading(2), "### Deep"),
            (rule(2), "### "),
            (heading(3), "Run in"),
        ],
        "{text}"
    );
}

#[test]
fn the_projection_writes_a_headings_depth_and_the_heading_rule_style() {
    let registry = registry();
    let run = run(&registry, "\\section{One}\n\n\\subsubsection{Deep}\n\nText.\n", None);
    let output = output(&run);
    let projected = output.project(&Projector::new(ProjectOptions::new()).table_projection(&run));
    let SerialValue::Map(entries) = projected else { panic!("a map") };
    let SerialValue::List(styles) = &entries[2].1 else { panic!("a list") };
    let style_and_depth: Vec<(SerialValue, Vec<&str>, SerialValue)> = styles
        .iter()
        .map(|mark| {
            let SerialValue::Map(mark) = mark else { panic!("a map") };
            let keys = mark.iter().map(|(key, _)| key.as_str()).collect();
            (mark[0].1.clone(), keys, mark[1].1.clone())
        })
        .collect();
    let keys = vec!["style", "depth", "bytes", "start", "end"];
    assert_eq!(
        style_and_depth,
        [
            (SerialValue::Str("heading".into()), keys.clone(), SerialValue::Int(0)),
            (SerialValue::Str("heading-rule".into()), keys.clone(), SerialValue::Int(0)),
            (SerialValue::Str("heading".into()), keys.clone(), SerialValue::Int(2)),
            (SerialValue::Str("heading-rule".into()), keys, SerialValue::Int(2)),
        ]
    );
}
