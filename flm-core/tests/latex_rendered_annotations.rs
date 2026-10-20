//! Annotations on the `latex-rendered` backend, end to end over the standard
//! package with two annotators: the documents `tests/annotations_html.rs`
//! renders (highlights and comments in a heading, in a paragraph and in list
//! items, a highlight cut around a list, comments and highlights over several
//! paragraphs), each a golden under `tests/goldens/latex-rendered/` holding
//! the document and the style file, which `tests/latex_rendered_compile.rs`
//! compiles with `lualatex`; the annotations feature's definition piece,
//! which the feature's style identifier brings into the style file; and the
//! render option `hidden`, under which nothing of an annotation is written
//! and the piece is not required.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::annotations::latex_rendered::PIECE;
use flm_core::stdcore::features::annotations::{AnnotationsConfig, AnnotationsRenderOptions, Annotator};

use common::latex_rendered::{assert_golden, document, render, style};

/// The standard package with the annotators `phf` (initials `PhF`, number 0)
/// and `abc` (number 1).
fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().annotations(
        AnnotationsConfig::new()
            .annotator(Annotator::new("phf").initials("PhF"))
            .annotator(Annotator::new("abc")),
    ))
}

/// Annotations in a heading, in a paragraph, in list items, and a highlight
/// cut around a list.
const PLACEMENT: &str = "\\section{A title \\phf{marked} \\phf[with a remark]}

Some text \\phf{a passage} and \\phf[a remark], then \\abc{another}.

\\begin{itemize}
\\item An entry \\phf{marked} here.
\\item \\phf[A remark opening the entry.]
\\end{itemize}

\\phf A passage \\begin{enumerate}\\item holding a list\\end{enumerate} and more.\\endphf
";

/// The Python product's test 11: the forms of an annotation, and a comment
/// and a highlight over several paragraphs, cut at each.
const TEST_11: &str = "\\phf{passage} and \\phf passage\\endphf and \\abc{x} and \\phf[note] and \\phf{x}[y].

Some text \\phf[A comment.

With multiple paragraphs.] More text.

Some text \\phf{A highlight.

With multiple paragraphs.} More text.

\\phf see \\begin{itemize}\\item a\\end{itemize} here \\endphf
";

#[test]
fn annotations_in_every_place_and_the_style_file_holds_the_piece() {
    let run = render(&registry(), &[PLACEMENT], &Overrides::none());
    let written = document(&run);
    assert!(written.contains("[role=flm.annotations.highlight, kinds={phf annotator-0}]"), "{written}");
    assert!(written.contains("[role=flm.annotations.highlight, kinds={abc annotator-1}]"), "{written}");
    assert!(written.contains("[role=flm.annotations.comment, kinds={phf annotator-0}]"), "{written}");
    let (text, _) = style(&run);
    assert!(text.contains(&format!("% {PIECE} --")), "{text}");
    assert_golden("annotations-placement.tex", &run);
}

#[test]
fn the_forms_and_the_annotations_cut_over_several_paragraphs() {
    let run = render(&registry(), &[TEST_11], &Overrides::none());
    let written = document(&run);
    assert!(written.contains("mark-continues"), "{written}");
    assert!(written.contains("mark-continued"), "{written}");
    assert_golden("annotations-test-11.tex", &run);
}

#[test]
fn hidden_annotations_write_nothing_and_require_no_piece() {
    let overrides = Overrides {
        render_options: OptionValues::new().with(AnnotationsRenderOptions { hidden: true }),
        ..Overrides::none()
    };
    let run = render(&registry(), &[PLACEMENT], &overrides);
    assert!(!document(&run).contains("flm.annotations"), "{}", document(&run));
    assert!(!style(&run).0.contains(PIECE), "{}", style(&run).0);
    assert_golden("annotations-hidden.tex", &run);
}

#[test]
fn an_annotator_without_initials_writes_no_margin_decoration() {
    let run = render(&registry(), &["Some text \\abc{a passage} and \\abc[a remark of abc]."], &Overrides::none());
    let written = document(&run);
    assert!(written.contains("[role=flm.annotations.comment, kinds={abc annotator-1}]"), "{written}");
    assert!(!written.contains("\\flmxDecoMargin"), "{written}");
    assert_golden("annotations-no-initials.tex", &run);
}
