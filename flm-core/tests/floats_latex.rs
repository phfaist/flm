//! The `floats` feature on the recomposed LaTeX
//! (`flm_core::stdcore::features::floats::latex`): the hook environment
//! `\begin{flmFloat}{<type>}{<case>}` with its four cases, the caption and
//! the labels, an unnumbered float's labels showing its caption, each used
//! type's reference type for `zref-clever`, the definition piece and the
//! `caption` package it requests (test 18); a footnote and a reference
//! inside a caption (test 19, the LaTeX half). The goldens are compiled under
//! both engines by `tests/latex_compile.rs`; `floats-configured.tex`, whose
//! environment the author defines, by its own test there.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::latex::{packages, preamble};
use flm_core::stdcore::backends::latex_recomposed;
use flm_core::stdcore::features::floats::latex::{FloatCase, PIECE, definition_piece};
use flm_core::stdcore::features::floats::{FloatNumbering, FloatType, FloatsConfig};
use flm_core::stdcore::features::numbering::{PluralWords, PrefixWords};

use common::stdcore::{identifiers, identifiers_but_unknown_size};

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The run of the recomposed LaTeX of `registry` over `document`.
fn run(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    common::stdcore::run_workflow(registry, latex_recomposed::WORKFLOW, &[document], &Overrides::none())
}

/// The recomposed LaTeX and the style file of `document` under `registry`;
/// the run must report nothing, and so must the text run, so that the
/// document is one FLM accepts on every output.
fn written(registry: &Arc<FlmRegistry>, document: &str) -> (String, String, Run) {
    let text = common::stdcore::run_workflow(
        registry,
        flm_core::stdcore::backends::text::WORKFLOW,
        &[document],
        &Overrides::none(),
    );
    assert_eq!(identifiers_but_unknown_size(&text), Vec::<String>::new(), "{}", text.diagnostics().render_all());
    let run = run(registry, document);
    assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let output = common::latex::output(&run);
    let style = common::latex::style(&run).0;
    (output, style, run)
}

/// The four cases, each holding an image and a label, and references to the
/// numbered ones and to the captioned unnumbered one.
const CASES: &str = "\\begin{figure}\\includegraphics{figures/a.png}\\caption{A photo}\\label{figure:a}\\end{figure}\n\n\
                     \\begin{figure}\\label{figure:b}\\includegraphics{figures/b.png}\\end{figure}\n\n\
                     \\begin{figure}\\nonumber\\includegraphics{figures/c.png}\\caption[alt=Photo C]{A photo of \\emph{C}}\\label{figure:c}\\end{figure}\n\n\
                     \\begin{figure}\\includegraphics{figures/d.png}\\end{figure}\n\n\
                     See \\ref{figure:a}, \\ref{figure:b} and \\ref{figure:c}. \\ref[S]{figure:a} is a photo.\n";

#[test]
fn the_four_cases_are_written_through_the_hook_environment() {
    let (output, style, run) = written(&std(), CASES);
    for expected in [
        "\\begin{flmFloat}{figure}{NumCap}\\flmIncludeGraphics{figures/a.png}\\caption{A photo}\\flmLabels{\\label{ref1}}\\end{flmFloat}",
        "\\begin{flmFloat}{figure}{NumOnly}\\flmIncludeGraphics{figures/b.png}\\caption{}\\flmLabels{\\label{ref2}}\\end{flmFloat}",
        "\\begin{flmFloat}{figure}{CapOnly}\\flmIncludeGraphics{figures/c.png}\\caption{A photo of \\emph{C}}\\phantomsection\\flmLabelsWithDisplayText{Photo C}{\\flmLabels{\\label{ref3}}}\\end{flmFloat}",
        "\\begin{flmFloat}{figure}{Bare}\\flmIncludeGraphics{figures/d.png}\\end{flmFloat}",
    ] {
        assert!(output.contains(expected), "{expected}\n{output}");
    }
    // `\nonumber` itself is never written.
    assert!(!output.contains("nonumber"), "{output}");
    // The piece, the `caption` package, the display-text hook and its type.
    for id in [PIECE, preamble::LABEL_DISPLAY_TEXT, preamble::ZREF_CLEVER_TYPE] {
        assert!(style.contains(&format!("% {id}\n")), "{id}: {style}");
    }
    for package in ["caption", "zref-clever", "hyperref"] {
        assert!(style.contains(&format!("{{{package}}}")), "{package}: {style}");
    }
    // The type's words for `zref-clever`, declared once, without the tie: the
    // capital variant for the capitalized keys, which `S` and `capital`
    // print, the base words for the lowercase ones, which a reference prints
    // by default.
    let setup = "\\ifcsname flm@float@reftype@figure\\endcsname\\else\
                 \\zcRefTypeSetup{figure}{Name-sg={Figure},name-sg={Fig.},Name-pl={Figures},name-pl={Figs.}}\
                 \\def\\flm@float@reftype@figure{}\\fi";
    assert_eq!(style.matches("\\zcRefTypeSetup{figure}").count(), 1, "{style}");
    assert!(style.contains(setup), "{style}");
    common::latex::assert_golden("floats-cases.tex", &run);
}

#[test]
fn an_unnumbered_float_with_no_alternative_text_shows_its_caption_and_a_bare_one_its_labels_plainly() {
    let (output, _, _) = written(
        &std(),
        "\\begin{figure}\\nonumber\\includegraphics{c.png}\\caption{A photo}\\label{figure:c}\\end{figure}\n\n\
         \\begin{figure}\\nonumber\\includegraphics{d.png}\\label{figure:d}\\end{figure}\n\n\\ref{figure:c}",
    );
    assert!(output.contains("\\caption{A photo}\\phantomsection\\flmLabelsWithDisplayText{A photo}{\\flmLabels{\\label{ref1}}}"), "{output}");
    assert!(output.contains("\\flmIncludeGraphics{d.png}\\flmLabels{\\label{ref2}}\\end{flmFloat}"), "{output}");
}

#[test]
fn the_piece_defines_the_environment_and_its_customization_points() {
    let piece = definition_piece();
    assert_eq!(piece.requires.iter().map(|id| id.as_str()).collect::<Vec<_>>(), [packages::CAPTION]);
    for case in FloatCase::ALL {
        let name = case.as_str();
        assert!(piece.content.contains(&format!("\\ProvideDocumentCommand\\flmFloatSetUseConfig{name}{{m m}}")), "{name}");
        let placement = if case.is_numbered() { "[tbph]" } else { "[h]" };
        assert!(piece.content.contains(&format!("\\providecommand\\flmFloatPlacementArgs{name}{{{placement}}}")), "{name}");
    }
    assert!(piece.content.contains("\\ProvideDocumentEnvironment{flmFloat}{m m}"));
}

#[test]
fn a_table_and_a_configured_type_declare_their_own_words() {
    let config = FloatsConfig::new()
        .float_type(FloatType::table().handlers(["any"]))
        .float_type(
            FloatType::new("diagram", ["any"])
                .numbering(FloatNumbering::AutoNumbered)
                .words(
                    PrefixWords::new("Diagram~", "Diagrams~")
                        .with_variant("lowercase", PluralWords::new("diagram~", "diagrams~"))
                        .with_variant("abbreviated", PluralWords::new("Diag.~", "Diags.~")),
                ),
        );
    let registry = common::stdcore::registry(StdcoreConfig::new().floats(config));
    let (output, style, run) = written(
        &registry,
        "\\begin{table}T.\\caption{A table}\\label{table:t}\\end{table}\n\n\
         \\begin{diagram}Boxes.\\caption{Arrows}\\label{diagram:x}\\end{diagram}\n\n\
         \\begin{diagram}More boxes.\\end{diagram}\n\n\
         See \\ref{table:t} and \\ref[abbreviated]{diagram:x}.\n",
    );
    assert!(output.contains("\\begin{flmFloat}{table}{NumCap}T.\\caption{A table}"), "{output}");
    // Auto-numbered: the labeled diagram is numbered, the other one not.
    assert!(output.contains("\\begin{flmFloat}{diagram}{NumCap}Boxes."), "{output}");
    assert!(output.contains("\\begin{flmFloat}{diagram}{Bare}More boxes.\\end{flmFloat}"), "{output}");
    assert!(style.contains("\\zcRefTypeSetup{table}{Name-sg={Table},name-sg={Tab.},Name-pl={Tables},name-pl={Tabs.}}"), "{style}");
    assert!(style.contains(
        "\\zcRefTypeSetup{diagram}{Name-sg={Diagram},name-sg={diagram},Name-pl={Diagrams},name-pl={diagrams},\
         Name-sg-ab={Diag.},name-sg-ab={Diag.},Name-pl-ab={Diags.},name-pl-ab={Diags.}}"
    ), "{style}");
    assert_eq!(style.matches("\\zcRefTypeSetup{diagram}").count(), 1, "{style}");
    // No environment is declared for the configured type: the author's.
    assert!(!style.contains("environment{diagram}"), "{style}");
    assert!(!style.contains("DeclareFloatingEnvironment"), "{style}");
    common::latex::assert_golden("floats-configured.tex", &run);
}

#[test]
fn a_footnote_and_a_reference_inside_a_caption_are_written_in_the_caption() {
    let (output, _, run) = written(
        &std(),
        "\\section{Results}\\label{sec:results}\n\n\
         \\begin{figure}\\includegraphics{figures/plot.png}\\caption{A plot\\footnote{Measured twice.} of \
         \\ref{sec:results}.}\\label{figure:plot}\\end{figure}\n\n\
         See \\ref{figure:plot}.\n",
    );
    assert!(
        output.contains("\\caption{A plot\\flmXNfootnote{Measured twice.} of \\flmRef{ref1}.}\\flmLabels{\\label{ref2}}"),
        "{output}"
    );
    common::latex::assert_golden("floats-caption-notes.tex", &run);
}

#[test]
fn the_recomposed_latex_reports_as_the_other_outputs_do() {
    let registry = std();
    for document in [
        "\\begin{figure}\\includegraphics{a.png}\\caption{B}\\caption{C}\\caption[x=1]{D}\\nonumber\\nonumber\\end{figure}",
        "\\begin{figure}Text.\\end{figure}",
        "\\begin{table}\\label{fig:x}\\includegraphics{t.png}\\end{table}",
    ] {
        let latex = run(&registry, document);
        let text = common::stdcore::run_workflow(
            &registry,
            flm_core::stdcore::backends::text::WORKFLOW,
            &[document],
            &Overrides::none(),
        );
        assert_eq!(identifiers(&latex), identifiers(&text), "{document}");
    }
}

#[test]
fn hard_documents_neither_panic_nor_loop() {
    let registry = std();
    let mut nested = String::from("\\includegraphics{x.png}");
    for _ in 0..10 {
        nested = format!("\\begin{{figure}}{nested}\\caption{{c}}\\end{{figure}}");
    }
    for document in [
        nested.as_str(),
        "\\begin{figure}\\caption{}\\end{figure}",
        "\\begin{figure}\\end{figure}",
        "\\begin{itemize}\\item \\begin{figure}\\includegraphics{x.png}\\caption{C}\\end{figure}\\end{itemize}",
        "Text.\\footnote{\\begin{figure}\\includegraphics{x.png}\\caption{C}\\end{figure}}",
        "\\begin{figure}",
        "\\begin{figure}\\caption[alt=",
    ] {
        let _ = common::latex::body(&run(&registry, document));
    }
}
