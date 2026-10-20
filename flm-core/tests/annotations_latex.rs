//! The `annotations` feature on the recomposed LaTeX: every annotation
//! written back as the author wrote it — the three forms, a comment and a
//! highlight crossing a paragraph break as one invocation, byte for byte —
//! over the `phfcc` package, whose commands the style file makes, one per
//! configured annotator in configuration order, each guarded on its name;
//! hidden, a highlight's passage and nothing of a comment but its paragraph
//! breaks; and annotations in the places content may stand. The goldens this
//! file writes are compiled by `tests/latex_compile.rs` under both engines.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::latex_recomposed;
use flm_core::stdcore::features::annotations::{AnnotationsConfig, AnnotationsRenderOptions, Annotator};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::StdcoreConfig;

use common::stdcore::{identifiers, run_workflow};

/// The two annotators these tests use: `phf` with the initials `PhF`,
/// annotator 0, and `abc` with none, annotator 1.
fn config() -> StdcoreConfig {
    StdcoreConfig::new().annotations(
        AnnotationsConfig::new()
            .annotator(Annotator::new("phf").initials("PhF"))
            .annotator(Annotator::new("abc")),
    )
}

/// The standard package under [`config`].
fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(config())
}

/// The overrides hiding every annotation, or none.
fn overrides(hidden: bool) -> Overrides {
    Overrides {
        render_options: OptionValues::new().with(AnnotationsRenderOptions { hidden }),
        ..Overrides::none()
    }
}

/// The recomposition of `document` under `registry`; the run must report
/// nothing.
fn recompose_under(registry: &Arc<FlmRegistry>, document: &str, hidden: bool) -> Run {
    let run = run_workflow(registry, latex_recomposed::WORKFLOW, &[document], &overrides(hidden));
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    run
}

/// The recomposed LaTeX of `document`, without the line naming the style
/// file and without the `%%` line ending the backend writes last.
fn written(document: &str, hidden: bool) -> String {
    let run = recompose_under(&registry(), document, hidden);
    let output = common::latex::output(&run);
    output.strip_suffix("%%\n").unwrap_or(&output).to_string()
}

/// The two declarations the style file holds, in configuration order.
const DECLARATIONS: [&str; 2] = [
    "\\ifcsname phf\\endcsname\\else\\phfMakeCommentingCommand[initials={PhF}]{phf}\\fi\n",
    "\\ifcsname abc\\endcsname\\else\\phfMakeCommentingCommand[formatinitials=hide]{abc}\\fi\n",
];

// ---------------------------------------------------------------------------------------------
// the invocation as written (P14; test 15)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_three_forms_come_back_as_written() {
    for document in [
        "word \\phf{passage} word",
        "word \\phf passage\\endphf word",
        "word \\phf[note] word",
        "word \\abc{x} word",
        "word \\phf{x}[y] word",
        "word \\phf  {spaced} word",
    ] {
        assert_eq!(written(document, false), document);
    }
}

#[test]
fn a_comment_across_a_paragraph_break_is_one_invocation_byte_for_byte() {
    for document in [
        "Some text \\phf[A comment.\n\nWith multiple paragraphs.] More text",
        "Some text \\phf{A highlight.\n\nWith multiple paragraphs.} More text",
        "Some text \\phf A highlight.\n\nWith multiple paragraphs.\\endphf More text",
        // The case with spaces around the break.
        "x \\phf{a \n\n b} y",
        "\\phf see \\begin{itemize}\\item a\\end{itemize} here \\endphf",
    ] {
        assert_eq!(written(document, false), document);
        assert_eq!(written(document, false).matches("\\phf").count(), 1, "{document}");
    }
}

#[test]
fn a_passage_left_open_is_written_with_no_closing_command() {
    let run = run_workflow(&registry(), latex_recomposed::WORKFLOW, &["a \\phf b"], &overrides(false));
    assert_eq!(identifiers(&run), ["flm.annotations.unterminated"]);
    assert_eq!(common::latex::output(&run), "a \\phf b%%\n");
    // A passage ended by an environment's `\end`: no closing command is
    // written, and the whitespace before `\end` is kept.
    let document = "\\begin{itemize}\\item \\phf a b  \\end{itemize}";
    let run = run_workflow(&registry(), latex_recomposed::WORKFLOW, &[document], &overrides(false));
    assert_eq!(identifiers(&run), ["flm.annotations.unterminated"]);
    let output = common::latex::output(&run);
    assert!(output.contains("\\phf a b  \\end{itemize}"), "{output}");
    assert!(!output.contains("\\endphf"), "{output}");
    // Hidden, the passage alone, the whitespace before `\end` kept.
    let run = run_workflow(&registry(), latex_recomposed::WORKFLOW, &[document], &overrides(true));
    let output = common::latex::output(&run);
    assert!(output.contains("a b  \\end{itemize}"), "{output}");
    assert!(!output.contains("\\phf"), "{output}");
}

#[test]
fn the_style_file_requests_phfcc_and_makes_every_annotators_command_in_order() {
    // `abc` is not used: its command is made all the same, after `phf`'s, so
    // that the package's colors follow the annotators' numbers.
    let run = recompose_under(&registry(), "word \\phf{x} word", false);
    let (style, _) = common::latex::style(&run);
    assert!(style.contains("\\RequirePackage{phfcc}\n"), "{style}");
    let first = style.find(DECLARATIONS[0]).expect("phf's command is made");
    let second = style.find(DECLARATIONS[1]).expect("abc's command is made");
    assert!(first < second, "{style}");
    assert_eq!(style.matches("\\phfMakeCommentingCommand").count(), 2, "{style}");
}

#[test]
fn initials_are_written_as_text() {
    let registry = common::stdcore::registry(
        StdcoreConfig::new().annotations(AnnotationsConfig::new().annotator(Annotator::new("rev").initials("A&B"))),
    );
    let run = recompose_under(&registry, "\\rev{x}", false);
    let (style, _) = common::latex::style(&run);
    assert!(
        style.contains("\\ifcsname rev\\endcsname\\else\\phfMakeCommentingCommand[initials={A\\&B}]{rev}\\fi\n"),
        "{style}"
    );
}

#[test]
fn a_document_holding_no_annotation_needs_no_phfcc() {
    let run = recompose_under(&registry(), "Plain text.", false);
    assert!(common::latex::style_if_any(&run).is_none_or(|(style, _)| !style.contains("phfcc")));
}

// ---------------------------------------------------------------------------------------------
// hidden (test 12 on the recomposed LaTeX)
// ---------------------------------------------------------------------------------------------

#[test]
fn hidden_a_highlight_is_its_passage_and_a_comment_nothing() {
    assert_eq!(written("word \\phf{passage} word", true), "word passage word");
    assert_eq!(written("word \\phf[note] word", true), "word  word");
    assert_eq!(written("a \\phf see \\begin{itemize}\\item b\\end{itemize} here\\endphf c", true),
        "a see \\begin{itemize}\\item b\\end{itemize} herec");
}

#[test]
fn hidden_a_comment_across_a_paragraph_break_leaves_the_break() {
    assert_eq!(
        written("Some text \\phf[A comment.\n\nWith more.] More text", true),
        "Some text \n\n More text"
    );
    // A highlight keeps its paragraphs with its passage.
    assert_eq!(written("x \\phf{a\n\nb} y", true), "x a\n\nb y");
}

#[test]
fn hidden_nothing_of_the_package_reaches_the_style_file() {
    let run = recompose_under(&registry(), "word \\phf{passage} \\phf[note] word", true);
    assert!(common::latex::style_if_any(&run).is_none_or(|(style, _)| !style.contains("phf")));
}

// ---------------------------------------------------------------------------------------------
// the places content may stand (test 19, the annotation half)
// ---------------------------------------------------------------------------------------------

#[test]
fn annotations_in_a_list_item_a_footnote_and_a_formula_come_back_as_written() {
    for document in [
        "\\begin{itemize}\n\\item a \\phf{b} \\abc[c]\n\\end{itemize}",
        "\\begin{itemize}\n\\item a \\phf[b\n\nc] d\n\\end{itemize}",
        "a \\(x \\text{ \\phf{y} }\\) b",
    ] {
        assert_eq!(written(document, false), document);
    }
    let footnote = written("x\\footnote{a \\phf{b} \\phf[c\n\nd]}", false);
    assert!(footnote.contains("a \\phf{b} \\phf[c\n\nd]"), "{footnote}");
}

#[test]
fn an_annotation_in_a_user_definitions_template_is_kept_in_the_definition() {
    let definition = Definition::macro_("note", vec![ArgDef::mandatory("text")]).text("\\phf[#1]");
    let registry = common::stdcore::registry(config().userdefs(UserdefsConfig::new().definition(definition)));
    let run = recompose_under(&registry, "word \\note{check} word", false);
    let (style, _) = common::latex::style(&run);
    assert!(style.contains("\\RequirePackage{phfcc}\n"), "{style}");
    assert!(style.contains(DECLARATIONS[0]), "{style}");
    assert!(style.contains("\\phf[#1]"), "{style}");
}

// ---------------------------------------------------------------------------------------------
// the goldens the compile suite compiles
// ---------------------------------------------------------------------------------------------

/// Every form, a cut comment and a cut highlight, a list inside a passage,
/// annotations in a heading's title, a list item and a footnote.
const DOCUMENT: &str = "\\section{A title \\phf{marked}}

Some text \\phf{a passage} and \\phf[a remark], then \\abc{another} and \\abc[a remark of abc's].

\\phf A passage closed by its command,\\endphf{} then \\phf{x}[y].

Some text \\phf[A comment.

With multiple paragraphs.] More text.

Some text \\phf{A highlight.

With multiple paragraphs.} More text.

\\phf see \\begin{itemize}\\item a list in a passage\\end{itemize} here \\endphf

\\begin{itemize}
\\item An entry \\phf{marked} and \\phf[remarked].
\\end{itemize}

A footnote.\\footnote{Holding \\phf{a passage} and \\abc[a remark].}
";

#[test]
fn the_golden_of_every_form() {
    let run = recompose_under(&registry(), DOCUMENT, false);
    common::latex::assert_golden("annotations.tex", &run);
}

#[test]
fn the_golden_of_every_form_hidden() {
    let run = recompose_under(&registry(), DOCUMENT, true);
    common::latex::assert_golden("annotations-hidden.tex", &run);
}
