//! The `xnotes` feature in the recomposed LaTeX: an xnote written back as
//! `\flmXN<kind>[<labels>]{<content>}`, the definition piece of each kind in
//! the style file, the labels written as a heading's are, a second kind, an
//! in-place kind, a nested xnote, an xnote in a heading's title, an xnote in
//! the display text a reference to an unnumbered place prints, the rules of
//! the list elements, and registries without the LaTeX features.
//!
//! Every golden this suite writes under `tests/goldens/latex/` is compiled by
//! `tests/latex_compile.rs` under `pdflatex` and `lualatex` when
//! `FLM_TESTS_LATEX_COMPILE` is set; that suite also checks that a reference
//! to a labeled footnote prints the footnote's number.
//!
//! Every registry is the standard package, its xnotes feature under the
//! configuration a test names, unless a test builds one of its own.

mod common;

use std::sync::Arc;

use flm_core::core::plan::{LoweringSelection, Overrides};
use flm_core::core::registry::{FlmFeature, FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::core::run::Run;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::latex::{ExtraPreamble, LatexPackageDef};
use flm_core::stdcore::backends::latex_recomposed::{LatexRecomposedRenderOptions, WORKFLOW};
use flm_core::stdcore::backends::{lowering, pieces, text};
use flm_core::stdcore::features::blocks::Blocks;
use flm_core::stdcore::features::numbering::PrefixWords;
use flm_core::stdcore::features::userdefs::{UserdefsConfig, instantiate_rule_name};
use flm_core::stdcore::features::xnotes::{
    self, XnoteKind, XnotePlacement, XnotesConfig, XnotesFeature,
};
use flm_core::stdcore::features::{index, numbering, refpoints, refs};
use flm_core::stdcore::{StdcoreConfig, check_feature, package};
use techy::error::Severity;

use common::latex::{assert_golden, output, style, style_if_any};
use common::stdcore::{
    identifiers, identifiers_but_unknown_size, registry_over, reports, reports_but_unknown_size,
    run_workflow, try_plan,
};

// ---------------------------------------------------------------------------------------------
// registries and runs
// ---------------------------------------------------------------------------------------------

/// The standard package, its xnotes feature under `xnotes`.
fn registry(xnotes: XnotesConfig) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().xnotes(xnotes)));
    builder.use_package(flm_core::math_passthrough::package());
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

/// The standard package with the xnotes feature under its default
/// configuration.
fn std() -> Arc<FlmRegistry> {
    registry(XnotesConfig::new())
}

/// The recomposition of `texts`, one tree each, under `registry` and the
/// render options `render`.
fn recompose_with(
    registry: &Arc<FlmRegistry>,
    texts: &[&str],
    render: LatexRecomposedRenderOptions,
) -> Run {
    let overrides =
        Overrides { render_options: OptionValues::new().with(render), ..Overrides::none() };
    run_workflow(registry, WORKFLOW, texts, &overrides)
}

/// The recomposition of `texts` under `registry` and the default render
/// options.
fn recompose(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Run {
    run_workflow(registry, WORKFLOW, texts, &Overrides::none())
}

/// The recomposition of `texts` under the default registry; the run must
/// report nothing.
fn clean(texts: &[&str]) -> Run {
    let run = recompose(&std(), texts);
    assert_eq!(identifiers_but_unknown_size(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    run
}

/// How many times `needle` stands in `haystack`.
fn count(haystack: &str, needle: &str) -> usize {
    haystack.matches(needle).count()
}

/// The configuration of two kinds: `footnote`, and a numbered `endnote`
/// written `\endnote`, handed to LaTeX's `\endnote`.
fn two_kinds() -> XnotesConfig {
    XnotesConfig::new().kind(
        XnoteKind::numbered("endnote", PrefixWords::new("Endnote~", "Endnotes~"))
            .commands(["endnote"])
            .label_type("en")
            .latex_command("endnote"),
    )
}

// ---------------------------------------------------------------------------------------------
// the command, its content and its labels (design tests 58, 59)
// ---------------------------------------------------------------------------------------------

/// The worked example: an xnote of one run, one of two paragraphs, a labeled
/// one and references to it.
const WORKED: &str = "The claim holds in every case.\\footnote{Except the empty one, which we exclude \
    by convention.} The next sentence goes on.\n\n\
    Two paragraphs.\\footnote{First paragraph.\n\n\
    Second paragraph.} And on.\n\n\
    A labeled note.\\footnote[\\label{fn:appendix}]{See the appendix.} \
    See \\ref{fn:appendix}, or \\ref[noprefix]{fn:appendix}.";

#[test]
fn an_xnote_is_written_as_its_kind_s_command_and_its_paragraphs_as_paragraphs() {
    let run = clean(&[WORKED]);
    let body = output(&run);
    for written in [
        "The claim holds in every case.\\flmXNfootnote{Except the empty one, which we exclude by \
         convention.} The next sentence goes on.",
        // The blank line inside the note is the note's: the command's
        // argument takes a paragraph break.
        "Two paragraphs.\\flmXNfootnote{First paragraph.\n\nSecond paragraph.} And on.",
    ] {
        assert!(body.contains(written), "{written}: {body}");
    }
    assert_golden("xnotes.tex", &run);
}

#[test]
fn a_labeled_xnote_writes_its_labels_in_the_labels_hook_as_the_optional_argument() {
    let run = clean(&[WORKED]);
    let body = output(&run);
    // The labels group as a heading writes it; the emitter braces an optional
    // argument holding a command, which LaTeX reads as the same argument.
    assert!(
        body.contains("A labeled note.\\flmXNfootnote[{\\flmLabels{\\label{ref1}}}]{See the appendix.}"),
        "{body}"
    );
    assert!(body.contains("See \\flmRef{ref1}, or \\flmRef[noprefix]{ref1}."), "{body}");
    // Only a labeled xnote has the optional argument.
    assert_eq!(count(&body, "\\flmXNfootnote["), 1, "{body}");
    let (style, _) = style(&run);
    assert!(style.contains("\\ProvideDocumentCommand\\flmLabels"), "{style}");
}

#[test]
fn with_the_label_hooks_off_the_optional_argument_holds_bare_labels() {
    let render = LatexRecomposedRenderOptions { label_hooks: false, ..LatexRecomposedRenderOptions::default() };
    let run = recompose_with(&std(), &["A note.\\footnote[\\label{fn:a}\\label{fn:b}]{Text.} See \\ref{fn:a}."], render);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let body = output(&run);
    assert!(body.contains("\\flmXNfootnote[{\\label{ref1}\\label{ref2}}]{Text.}"), "{body}");
    assert!(!body.contains("flmLabels"), "{body}");
    assert_golden("xnotes-no-label-hooks.tex", &run);
}

// ---------------------------------------------------------------------------------------------
// the definition piece (design test 60)
// ---------------------------------------------------------------------------------------------

/// The definition piece of the kind `footnote`, as the style file holds it.
const FOOTNOTE_PIECE: &str = "% \\flmXNfootnote[<labels>]{<note>}: a note of the kind `footnote' \
    in the FLM document;\n\
    % by default it passes the note, its labels first, to \\footnote.\n\
    % Redefine it to set these notes another way.\n\
    \\ProvideDocumentCommand\\flmXNfootnote{+O{} +m}{\\footnote{#1#2}}\n";

#[test]
fn the_style_file_holds_the_definition_piece_once_however_many_xnotes() {
    let run = clean(&["One.\\footnote{a} Two.\\footnote{b}", "Three.\\footnote{c}"]);
    let (style, _) = style(&run);
    assert_eq!(count(&style, FOOTNOTE_PIECE), 1, "{style}");
    // It requests no package: the command it hands the note to is LaTeX's.
    assert!(!style.contains("\\RequirePackage"), "{style}");
}

#[test]
fn a_document_without_xnotes_gets_no_definition_piece() {
    let run = clean(&["A heading-less text.", "\\section{Title}\\label{sec:a} See \\ref{sec:a}."]);
    let style = style_if_any(&run).map(|(style, _)| style).unwrap_or_default();
    assert!(!style.contains("flmXN"), "{style}");
}

// ---------------------------------------------------------------------------------------------
// kinds
// ---------------------------------------------------------------------------------------------

#[test]
fn a_second_kind_gets_its_own_command_and_definition_piece() {
    // The endnotes package is the author's: the piece requests nothing, and
    // the author asks for the package through the render options.
    let render = LatexRecomposedRenderOptions {
        extra_packages: vec![LatexPackageDef::new("endnotes", "the author's endnotes")],
        extra_preamble: ExtraPreamble {
            at_end: String::from("% the endnotes printed at the end of the document\n\\AtEndDocument{\\theendnotes}\n"),
            ..ExtraPreamble::default()
        },
        ..LatexRecomposedRenderOptions::default()
    };
    let run = recompose_with(
        &registry(two_kinds()),
        &["A foot.\\footnote{F.} An end.\\endnote[\\label{en:e}]{E.} See \\ref{en:e}."],
        render,
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let body = output(&run);
    assert!(body.contains("A foot.\\flmXNfootnote{F.} An end.\\flmXNendnote[{\\flmLabels{\\label{ref1}}}]{E.}"), "{body}");
    let (style, _) = style(&run);
    assert_eq!(count(&style, FOOTNOTE_PIECE), 1, "{style}");
    assert!(style.contains("\\ProvideDocumentCommand\\flmXNendnote{+O{} +m}{\\endnote{#1#2}}\n"), "{style}");
    assert_golden("xnotes-endnote.tex", &run);
}

#[test]
fn a_kind_without_xnotes_in_the_document_gets_no_piece() {
    let run = recompose(&registry(two_kinds()), &["A foot.\\footnote{F.}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let (style, _) = style(&run);
    assert!(style.contains("flmXNfootnote"), "{style}");
    assert!(!style.contains("flmXNendnote"), "{style}");
}

#[test]
fn an_in_place_kind_is_written_the_same_way() {
    // In place is the other outputs' placement; on LaTeX setting such notes
    // another way is the author's, who redefines the command.
    let config = XnotesConfig::new().kind(
        XnoteKind::unnumbered("sidenote", PrefixWords::new("Side note~", "Side notes~"))
            .commands(["sidenote"])
            .label_type("sn")
            .placement(XnotePlacement::InPlace),
    );
    let run = recompose(
        &registry(config),
        &["An aside.\\sidenote{Inline.} And one holding paragraphs.\\sidenote{First.\n\nSecond.} On."],
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let body = output(&run);
    assert!(
        body.contains("An aside.\\flmXNsidenote{Inline.} And one holding paragraphs.\\flmXNsidenote{First.\n\nSecond.} On."),
        "{body}"
    );
    let (style, _) = style(&run);
    assert!(style.contains("\\ProvideDocumentCommand\\flmXNsidenote{+O{} +m}{\\footnote{#1#2}}\n"), "{style}");
    assert_golden("xnotes-in-place.tex", &run);
}

// ---------------------------------------------------------------------------------------------
// where an xnote stands, and what is reported
// ---------------------------------------------------------------------------------------------

#[test]
fn a_nested_xnote_is_written_inside_the_outer_one_as_any_xnote_labels_included() {
    let run = recompose(&std(), &["Outer.\\footnote{a\\footnote[\\label{fn:in}]{b} c} After. See \\ref{fn:in}."]);
    assert_eq!(reports(&run), Vec::<(String, Severity)>::new(), "{}", run.diagnostics().render_all());
    let body = output(&run);
    // The inner xnote is an xnote like any: its label is written, and LaTeX
    // numbers it, after the outer one, as FLM does. (LaTeX's own `\footnote`
    // prints the inner note's mark and leaves its text unset, so the reference
    // to its label is undefined in the PDF: the compile suite's own test of
    // this golden expects that.)
    assert!(
        body.contains("Outer.\\flmXNfootnote{a\\flmXNfootnote[{\\flmLabels{\\label{ref1}}}]{b} c} After."),
        "{body}"
    );
    assert_golden("xnotes-nested.tex", &run);
}

#[test]
fn an_xnote_in_a_heading_title_is_the_robust_command_in_the_sectioning_command() {
    let run = clean(&["\\section{A title\\footnote{A note on the title.}}\\label{sec:t}\n\nSee \\ref{sec:t}."]);
    let body = output(&run);
    assert!(body.contains("\\section{A title\\flmXNfootnote{A note on the title.}}"), "{body}");
    assert_golden("xnotes-in-heading.tex", &run);
}

/// An xnote in what a reference to an unnumbered place prints: the caption of
/// an unnumbered float, the term of a definition, each referred to twice; a
/// second unnumbered float whose caption's alternative text leaves the note
/// out, referred to twice; then a later note.
const IN_DISPLAY_TEXT: &str = "\\begin{figure}\\nonumber\\includegraphics{figures/c.png}\\caption{C plot\\footnote{Note C.}}\
    \\label{figure:c}\\end{figure}\n\n\
    \\begin{defterm}{Entropy\\footnote{Note E.}}\\label{topic:entropy}\nA measure.\n\\end{defterm}\n\n\
    \\begin{figure}\\nonumber\\includegraphics{figures/d.png}\\caption[alt=A photo]{A photo\\footnote{Note D.}}\
    \\label{figure:d}\\end{figure}\n\n\
    See \\ref{figure:c} and \\ref{figure:c}; \\ref{topic:entropy} and \\ref{topic:entropy}; \
    \\ref{figure:d} and \\ref{figure:d}.\\footnote{Last note.}\n";

#[test]
fn an_xnote_in_a_display_text_is_written_as_in_the_element() {
    // The display text is the element's own words as written, lowered as
    // ordinary content, the note included — as a heading's title is reused
    // by its hook; nothing is reported. The alternative text is written
    // alone.
    let run = clean(&[IN_DISPLAY_TEXT]);
    let body = output(&run);
    for expected in [
        "\\caption{C plot\\flmXNfootnote{Note C.}}\\phantomsection\\flmLabelsWithDisplayText{C plot\\flmXNfootnote{Note C.}}{",
        "\\begin{defterm}{Entropy\\flmXNfootnote{Note E.}}\\phantomsection\\flmLabelsWithDisplayText{Entropy\\flmXNfootnote{Note E.}}{",
        "\\caption{A photo\\flmXNfootnote{Note D.}}\\phantomsection\\flmLabelsWithDisplayText{A photo}{",
    ] {
        assert!(body.contains(expected), "{expected}\n{body}");
    }
    assert_eq!(count(&body, "\\flmXNfootnote"), 6, "{body}");
    assert_golden("xnotes-in-display-text.tex", &run);
}

#[test]
fn a_labeled_xnote_in_a_display_text_keeps_the_element() {
    // A labeled note in an unnumbered float's caption and in a defined term:
    // the display text is written as ordinary content, the label included,
    // and the float and the definition are both written back.
    let run = recompose(
        &std(),
        &["\\begin{figure}\\nonumber\\includegraphics{figures/c.png}\\caption{C plot\\footnote[\\label{fn:c}]{Note C.}}\
            \\label{figure:c}\\end{figure}\n\n\
            \\begin{defterm}{Entropy\\footnote[\\label{fn:e}]{Note E.}}\\label{topic:entropy}\nA measure.\n\\end{defterm}\n\n\
            See \\ref{figure:c}, \\ref{topic:entropy}, \\ref{fn:c} and \\ref{fn:e}.\n"],
    );
    assert_eq!(reports_but_unknown_size(&run), Vec::<(String, Severity)>::new(), "{}", run.diagnostics().render_all());
    let body = output(&run);
    assert!(!body.contains("% flm: unsupported element"), "{body}");
    for expected in [
        "\\begin{flmFloat}",
        "\\flmLabelsWithDisplayText{C plot\\flmXNfootnote[",
        "\\begin{defterm}{Entropy\\flmXNfootnote[",
        "\\flmLabelsWithDisplayText{Entropy\\flmXNfootnote[",
    ] {
        assert!(body.contains(expected), "{expected}\n{body}");
    }
}

#[test]
fn the_optional_argument_s_refused_content_and_an_empty_xnote_are_reported() {
    let run = recompose(&std(), &["A.\\footnote[\\textbf{x}\\label{fn:e}]{y} B.\\footnote{} C.\\footnote[]{}"]);
    assert_eq!(
        reports(&run),
        [
            ("flm.xnotes.bad-label-argument".to_string(), Severity::Error),
            ("flm.xnotes.empty".to_string(), Severity::Warning),
            ("flm.xnotes.empty".to_string(), Severity::Warning),
        ],
        "{}",
        run.diagnostics().render_all()
    );
    let body = output(&run);
    // The label beside the refused content still labels the xnote.
    assert!(body.contains("A.\\flmXNfootnote[{\\flmLabels{\\label{ref1}}}]{y} B.\\flmXNfootnote{} C.\\flmXNfootnote{}"), "{body}");
}

#[test]
fn an_xnote_in_a_list_s_own_label_template_is_written_into_the_key() {
    // A list's own label template is lowered as a value: the recomposed LaTeX
    // writes an xnote wherever it is lowered, so LaTeX sets it beside every
    // item's label, and nothing is reported.
    let run = clean(&["\\begin{enumerate}[label={#{arabic}\\footnote{x}.}]\\item a\\end{enumerate}"]);
    let body = output(&run);
    assert!(body.contains("[label={\\arabic*\\flmXNfootnote{x}.}]"), "{body}");
}

/// The standard package with the user definitions `definitions`.
fn with_definitions(definitions: Vec<Definition>) -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions))));
    builder.use_package(flm_core::math_passthrough::package());
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

#[test]
fn an_xnote_in_a_kept_definition_s_template_is_written_in_the_style_file() {
    // The definition is kept as a call and its template lowered into the
    // style file, the xnote's command with it: LaTeX sets the note wherever
    // the call stands, once per call, and nothing is reported.
    let registry = with_definitions(vec![
        Definition::macro_("noted", Vec::new()).text("a definition's text\\footnote{The definition's note.}"),
    ]);
    let run = recompose(&registry, &["One \\noted{} and two \\noted{}."]);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(output(&run).contains("One \\flmUDnoted{} and two \\flmUDnoted{}."), "{}", output(&run));
    let (style, _) = style(&run);
    assert!(
        style.contains("\\providecommand\\flmUDnoted{a definition's text\\flmXNfootnote{The definition's note.}}"),
        "{style}"
    );
    assert!(style.contains("\\ProvideDocumentCommand\\flmXNfootnote"), "{style}");
}

#[test]
fn a_labeled_xnote_in_a_kept_definition_s_template_is_refused_as_any_labeled_point_there() {
    // A template lowered as a value may not declare a label, which is a fact
    // about the document: the definition is refused at each use site, as it
    // is for a labeled heading in a template, every use site lowering to the stand-in
    // of a failed rule, and the reference, whose label the document's own
    // scan never saw inside the kept call, is unresolved.
    let registry = with_definitions(vec![
        Definition::macro_("noted", Vec::new()).text("text\\footnote[\\label{fn:k}]{n}"),
        Definition::macro_("titled", Vec::new()).text("\\section{T}\\label{sec:k}"),
    ]);
    let run = recompose(&registry, &["\\noted{} and \\noted{}. See \\ref{fn:k}."]);
    assert_eq!(
        identifiers(&run),
        [
            "flm.refs.unresolved",
            "flm.userdefs.kept-template-declares-point",
            "flm.userdefs.kept-template-declares-point"
        ],
        "{}",
        run.diagnostics().render_all()
    );
    assert!(!output(&run).contains("flmXN"), "{}", output(&run));
    let heading = recompose(&registry, &["\\titled{}\n\nSee \\ref{sec:k}."]);
    assert_eq!(
        identifiers(&heading),
        ["flm.refs.unresolved", "flm.userdefs.kept-template-declares-point"]
    );
    // The author's remedy: the definition marked to be expanded in every plan,
    // so that its use site is expanded in the document before the labels are
    // scanned, and its label is the document's own.
    let expanded = with_definitions(vec![
        Definition::macro_("noted", Vec::new())
            .text("text\\footnote[\\label{fn:k}]{n}")
            .expand(flm_core::lang::rewrites::userdefs::ExpandPolicy::Always),
    ]);
    let run = recompose(&expanded, &["\\noted{}. See \\ref{fn:k}."]);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(
        output(&run).contains("text\\flmXNfootnote[{\\flmLabels{\\label{ref1}}}]{n}{}. See \\flmRef{ref1}."),
        "{}",
        output(&run)
    );
}

#[test]
fn a_use_site_expanded_where_it_stands_writes_its_argument_s_xnote_as_written() {
    // A use site copied to be expanded where it stands is the document's
    // content there: the copy is verbatim, and the note in its argument is
    // written as the note's command inside the expansion.
    let my_macro = Definition::macro_("myMacro", vec![ArgDef::mandatory("text")]).text("A #1 B");
    let registry = with_definitions(vec![my_macro.clone()]);
    let selection = LoweringSelection::new().prefer(instantiate_rule_name(&my_macro).as_str());
    let overrides =
        Overrides { lowering: [("latex-recomposed".into(), selection)].into_iter().collect(), ..Overrides::none() };
    let run = run_workflow(&registry, WORKFLOW, &["\\myMacro{x\\footnote{N.}}"], &overrides);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(output(&run).contains("A x\\flmXNfootnote{N.} B"), "{}", output(&run));
    assert_eq!(count(&output(&run), "\\flmXNfootnote{"), 1, "{}", output(&run));
    // The same where the recomposition itself expands every use site, a rule
    // of the template needing the characters of a placeholder.
    let registry = with_definitions(vec![
        Definition::macro_("myref", vec![ArgDef::mandatory("key"), ArgDef::mandatory("text")])
            .text("X (see \\ref{#1}: #2)."),
    ]);
    let run = recompose(&registry, &["\\section{A}\\label{sec:a}\n\n\\myref{sec:a}{x\\footnote{N.}}"]);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(output(&run).contains("X (see \\flmRef{ref1}: x\\flmXNfootnote{N.})."), "{}", output(&run));
}

#[test]
fn an_xnote_nested_ten_deep_is_written_without_a_panic() {
    let mut text = String::from("x");
    for _ in 0..10 {
        text = format!("a\\footnote{{{text}}}");
    }
    let run = recompose(&std(), &[&text]);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(count(&output(&run), "\\flmXNfootnote{"), 10);
}

// ---------------------------------------------------------------------------------------------
// plans and registries
// ---------------------------------------------------------------------------------------------

#[test]
fn the_recomposed_latex_plan_builds_with_the_feature_registered_and_no_hole() {
    // Every element the feature registers — the xnote, each kind's list, the
    // entry — has a rule on this backend, so the plan needs no accepted hole.
    let registry = registry(two_kinds());
    let plan = try_plan(&registry, WORKFLOW, &Overrides::none())
        .unwrap_or_else(|error| panic!("the plan failed: {error}"));
    // The backend places no list: no builder, no pieces stage, and nothing
    // reads the table of xnotes.
    let stages = plan.stage_names();
    assert!(!stages.iter().any(|stage| stage.as_str().starts_with("flm.xnotes.")), "{stages:?}");
    let rule = |element: &str| {
        registry
            .rules_for(element)
            .iter()
            .map(|idx| &registry.rule(*idx).rule)
            .find(|rule| rule.default.is_specific_to(&"latex-recomposed".into()))
            .map(|rule| rule.name.to_string())
    };
    assert_eq!(rule(xnotes::XNOTE).as_deref(), Some(xnotes::latex::LATEX_RULE));
    for kind in ["footnote", "endnote"] {
        assert_eq!(
            rule(xnotes::list_element_name(kind).as_str()),
            Some(xnotes::latex::list_rule_name(kind).to_string())
        );
    }
}

#[test]
fn without_the_latex_features_the_feature_builds_and_renders_text() {
    let registry = registry_over(vec![
        Arc::new(Blocks::default()) as Arc<dyn FlmFeature>,
        index::feature(),
        refpoints::feature(),
        numbering::feature(),
        refs::feature(),
        Arc::new(XnotesFeature::new(two_kinds())),
        lowering::feature(),
        pieces::feature(),
        text::feature(),
        check_feature(),
    ]);
    // The rules naming the backend are set aside, and the definition pieces
    // are dropped, each with one Note; nothing fails.
    assert!(!registry.has_feature("latex-recomposed"));
    let notes: Vec<&str> = registry.build_diagnostics().iter().map(|d| d.identifier()).collect();
    assert!(!notes.is_empty());
    assert!(notes.iter().all(|id| *id == "flm.registry.unresolved-peer"), "{notes:?}");
    let set_aside: Vec<&str> = registry
        .set_aside_rules()
        .map(|entry| entry.rule.name.as_str())
        .filter(|name| name.starts_with("flm.xnotes.") && name.ends_with(".latex-recomposed"))
        .collect();
    assert_eq!(
        set_aside,
        [
            "flm.xnotes.footnote.list.lower.latex-recomposed",
            "flm.xnotes.endnote.list.lower.latex-recomposed",
            "flm.xnotes.content-pointer.lower.latex-recomposed",
            "flm.xnotes.mark-of.lower.latex-recomposed",
            "flm.xnotes.xnote.lower.latex-recomposed",
        ]
    );
    // The feature renders as it does in a registry holding the backend.
    let run = run_workflow(
        &registry,
        text::WORKFLOW,
        &["A foot.\\footnote{F.} An end.\\endnote{E.}"],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(
        common::lowering::text(&run),
        "A foot.¹ An end.¹\n\nFootnotes\n---------\n\n¹ F.\n\nEndnotes\n--------\n\n¹ E.\n"
    );
}

#[test]
fn a_contributing_feature_builds_without_the_latex_features_and_declares_nothing_itself() {
    // `contribute_kind` peer-declares, per kind, every name the kind's LaTeX
    // rules use: a feature contributing two kinds, declaring no peer of its
    // own, builds in a registry without the LaTeX features, its two list
    // rules on that backend set aside.
    let registry = registry_over(vec![
        Arc::new(Blocks::default()) as Arc<dyn FlmFeature>,
        index::feature(),
        refpoints::feature(),
        numbering::feature(),
        refs::feature(),
        Arc::new(XnotesFeature::new(XnotesConfig::new())),
        common::pipeline::feature("t.contributor", |b| {
            xnotes::contribute_kind(
                b,
                Arc::new(
                    XnoteKind::numbered("addendum", PrefixWords::new("A~", "As~")).label_type("a"),
                ),
            )?;
            xnotes::contribute_kind(
                b,
                Arc::new(
                    XnoteKind::numbered("remark", PrefixWords::new("R~", "Rs~")).label_type("r"),
                ),
            )
        }),
        lowering::feature(),
        pieces::feature(),
        text::feature(),
        check_feature(),
    ]);
    assert!(!registry.has_feature("latex-recomposed"));
    let set_aside: Vec<&str> = registry
        .set_aside_rules()
        .map(|entry| entry.rule.name.as_str())
        .filter(|name| name.ends_with(".list.lower.latex-recomposed"))
        .collect();
    assert_eq!(
        set_aside,
        [
            "flm.xnotes.footnote.list.lower.latex-recomposed",
            "flm.xnotes.addendum.list.lower.latex-recomposed",
            "flm.xnotes.remark.list.lower.latex-recomposed",
        ]
    );
}

#[test]
fn a_kind_whose_latex_command_is_not_letters_fails_the_build() {
    // The command is written into the style file as it stands.
    for command in ["", "foot note", "footnote}\\evil{", "\\footnote"] {
        let kind = XnoteKind::footnote().latex_command(command);
        assert!(kind.validate().is_err(), "{command:?}");
        let mut builder = RegistryBuilder::new();
        builder.use_package(package(StdcoreConfig::new().xnotes(XnotesConfig { kinds: vec![kind] })));
        builder.use_package(flm_core::math_passthrough::package());
        assert!(builder.build(&RegistryInputs::new()).is_err(), "{command:?}");
    }
    assert!(XnoteKind::footnote().latex_command("marginpar").validate().is_ok());
}


// ---------------------------------------------------------------------------------------------
// an xnote in content shown again (tests 2 and P5 of the copies of shown content)
// ---------------------------------------------------------------------------------------------

/// A rule for the mark of a copied xnote that fails wherever it is asked to
/// lower one: preferred over the feature's own, it makes every lowering of
/// content shown again visible as `flm.lowering.rule-failed`.
struct FailsIfLowered;

impl flm_core::core::lowering::Lower for FailsIfLowered {
    fn lower(
        &self,
        _node: techy::core::node::NodeRef<'_, flm_core::lang::Flm>,
        _cx: &mut flm_core::core::lowering::LowerCx<'_>,
        _lw: &mut dyn flm_core::core::lowering::Lowerer,
    ) -> Result<flm_core::core::lowering::Lowering, flm_core::core::lowering::LowerError> {
        Err(flm_core::core::lowering::LowerError::structure("content shown again was lowered"))
    }
}

/// The name of the probe rule [`FailsIfLowered`].
const PROBE: &str = "t.probe.mark-of";

/// The standard package with the probe rule [`FailsIfLowered`] for the mark
/// of a copied xnote, a default nowhere.
fn with_probe() -> Arc<FlmRegistry> {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(common::pipeline::feature("t.probe", |b| {
        b.lowering_rule(flm_core::core::lowering::LoweringRule::new(
            PROBE,
            xnotes::MARK_OF,
            flm_core::core::lowering::LoweringDecl::new(),
            Arc::new(FailsIfLowered),
            flm_core::core::lowering::DefaultMark::Never,
        ))
    }));
    builder.build(&RegistryInputs::new()).expect("a registry")
}

/// The overrides preferring the probe rule on `backend`.
fn probing(backend: &str) -> Overrides {
    let selection = flm_core::core::plan::LoweringSelection::new().prefer(PROBE);
    Overrides { lowering: [(backend.into(), selection)].into_iter().collect(), ..Overrides::none() }
}

/// Every construct whose content a reference shows, each holding a note, each
/// referred to.
const SHOWN_AGAIN: &str = "\\section*{Intro\\footnote{N.}}\\label{sec:a}\n\n\
    \\begin{figure}\\nonumber\\includegraphics{figures/a.png}\\caption{A photo\\footnote{P.}}\\label{figure:a}\
    \\end{figure}\n\n\
    \\begin{figure}\\nonumber\\includegraphics{figures/b.png}\\caption[alt=B \\footnote{Q.}]{B}\\label{figure:b}\
    \\end{figure}\n\n\
    \\begin{defterm}{Entropy\\footnote{E.}}\\label{topic:entropy}\nA measure.\n\\end{defterm}\n\n\
    \\begin{enumerate}\\item[Tag\\footnote{T.}]\\label{item:a} One.\\end{enumerate}\n\n\
    See \\ref{sec:a}, \\ref{figure:a}, \\ref{figure:b}, \\ref{topic:entropy} and \\ref{item:a}.";

#[test]
fn no_rule_lowers_content_shown_again_on_the_recomposed_latex() {
    let registry = with_probe();
    // The probe is live: the text output shows the content again, and the
    // probe fails there.
    let text = run_workflow(&registry, text::WORKFLOW, &[SHOWN_AGAIN], &probing("text"));
    assert!(identifiers(&text).iter().any(|id| id == "flm.lowering.rule-failed"), "{:?}", identifiers(&text));
    // The recomposed LaTeX never lowers it.
    let latex = run_workflow(&registry, WORKFLOW, &[SHOWN_AGAIN], &probing("latex-recomposed"));
    assert_eq!(identifiers_but_unknown_size(&latex), Vec::<String>::new(), "{}", latex.diagnostics().render_all());
}

#[test]
fn a_labeled_xnote_in_a_title_a_reference_shows_is_written_once_with_its_labels() {
    // The recomposed LaTeX writes the heading and the note as the document
    // wrote them, once, whatever a reference to the heading shows elsewhere.
    let run = clean(&[
        "\\section*{Intro\\footnote[\\label{fn:a}]{N.}}\\label{sec:a}\n\nSee \\ref{sec:a} and \\ref{fn:a}.",
    ]);
    let body = output(&run);
    assert!(
        body.contains("\\section*{Intro\\flmXNfootnote[{\\flmLabels{\\label{ref2}}}]{N.}}{\\flmLabels{\\label{ref1}}}"),
        "{body}"
    );
    assert_eq!(count(&body, "\\flmXNfootnote"), 1, "{body}");
    assert_eq!(count(&body, "\\flmLabels{"), 2, "{body}");
    // The golden, which the compile suite compiles: an unnumbered heading
    // holding a note and a numbered one holding a labeled note; then each
    // construct whose words a reference prints through the display-text hook
    // holding a labeled note (an unnumbered heading, an unnumbered float's
    // caption, a defined term); each referred to, and every labeled note
    // referred to but the caption's: LaTeX's `\footnote`, which the default
    // `\flmXNfootnote` hands a note to, loses a note inside a caption, its
    // label with it, so a reference to that label would stay undefined.
    let golden = clean(&[
        "\\section*{Intro\\footnote{N.}}\\label{sec:a}\n\n\\section{Other\\footnote[\\label{fn:b}]{M.}}\\label{sec:b}\n\n\
         \\section*{Starred\\footnote[\\label{fn:c}]{S.}}\\label{sec:c}\n\n\
         \\begin{figure}\\nonumber\\includegraphics{figures/a.png}\
         \\caption{A photo\\footnote[\\label{fn:d}]{P.}}\\label{figure:a}\\end{figure}\n\n\
         \\begin{defterm}{Entropy\\footnote[\\label{fn:e}]{E.}}\\label{topic:entropy}\nA measure.\n\\end{defterm}\n\n\
         See \\ref{sec:a}, \\ref{sec:b}, \\ref{sec:c}, \\ref{figure:a}, \\ref{topic:entropy}, \\ref{fn:b}, \
         \\ref{fn:c} and \\ref{fn:e}.",
    ]);
    assert_golden("xnotes-shown-again.tex", &golden);
}
