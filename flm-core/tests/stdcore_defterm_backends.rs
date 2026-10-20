//! The `defterm` feature on the HTML backend and on the recomposed LaTeX: the
//! page of a definition and of a term link (goldens, which
//! `tests/html_validity.rs` parses with no excuse), the stylesheet of the term
//! being defined, and the recomposed LaTeX of a definition and of a term link
//! (goldens, which `tests/latex_compile.rs` compiles under both engines, with
//! and without the author's own `hyperref`), the lead-in written into the
//! environment's definition from the configured template, a term link in a
//! user definition's template kept as a LaTeX definition, and the definitions
//! whose term link holds a placeholder, expanded at each use site.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{DefContent, FlmRegistry};
use flm_core::core::run::Run;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::{
    Definition, ExpandPolicy, NotRecomposable, UserDefinition,
};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::html::{DEFINITIONS_TABLE, WORKFLOW as HTML_WORKFLOW};
use flm_core::stdcore::backends::latex_recomposed::{self, LatexRecomposedRenderOptions};
use flm_core::stdcore::features::defterm::{self, DeftermConfig};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::StdcoreConfig;
use techy::error::Severity;

use common::stdcore::{identifiers, reports, run_workflow};

// ---------------------------------------------------------------------------------------------
// registries and runs
// ---------------------------------------------------------------------------------------------

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The standard package with the defterm feature under `config`.
fn with_config(config: DeftermConfig) -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().defterm(config))
}

/// The standard package with `definitions` as its user definitions.
fn with_definitions(definitions: Vec<Definition>) -> Arc<FlmRegistry> {
    common::stdcore::registry(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)),
    )
}

/// The run of the `html` workflow over `document`.
fn html_run(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, HTML_WORKFLOW, &[document], &Overrides::none())
}

/// The HTML fragment of `document`; the run must report nothing.
fn html_output(registry: &Arc<FlmRegistry>, document: &str) -> String {
    let run = html_run(registry, document);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    common::html::fragment(&run)
}

/// The recomposition of `document` under `overrides`.
fn recompose_under(registry: &Arc<FlmRegistry>, document: &str, overrides: &Overrides) -> Run {
    run_workflow(registry, latex_recomposed::WORKFLOW, &[document], overrides)
}

/// The recomposition of `document`, which must report nothing.
fn recompose_clean(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    let run = recompose_under(registry, document, &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    run
}

/// Whether the page of `run` requires the feature's stylesheet.
fn requires_style(run: &Run) -> bool {
    run.table(DEFINITIONS_TABLE)
        .expect("the definitions table is declared")
        .expect("the definitions table is written")
        .definitions
        .iter()
        .any(|row| row.id.as_str() == defterm::html::STYLE)
}

// ---------------------------------------------------------------------------------------------
// the page (test 13, the HTML half)
// ---------------------------------------------------------------------------------------------

/// A document holding a definition with an alias and a written `topic:`
/// label, whose body uses its own term; term links outside it — by the
/// term, by a bracket, by the alias — a reference to the label, and an
/// `\href` whose text holds a term link.
const DOCUMENT: &str = "\\section{Terms}

\\begin{defterm}{Entropy}\\label{topic:entropy}\\aliasterm{Entropies}
A measure of disorder: the \\term{Entropy} of a system never decreases.
\\end{defterm}

The \\term{Entropy} of a gas, \\term[Entropy]{its entropy}, all \\term{Entropies}; see
\\ref{topic:entropy} and \\href{https://example.com/}{a page on \\term{Entropy}}.
";

#[test]
fn the_page_of_a_definition_and_of_term_links() {
    // The term link inside the `\href`'s text is flattened, and nothing is
    // reported.
    let run = html_run(&std(), DOCUMENT);
    assert_eq!(reports(&run), Vec::<(String, Severity)>::new(), "{}", run.diagnostics().render_all());
    let html = common::html::fragment(&run);
    // The definition: a block of its role carrying its anchor, the lead-in a
    // run-in decoration at the head of its first paragraph.
    assert!(
        html.contains(
            "<div class=\"flm-pres-block flm-defterm\" id=\"defterm-Entropy\">\n  <p class=\"flm-pres-par\"><span class=\"flm-pres-deco-runin\"><span class=\"flm-pres-span flm-defterm-leadin\">Entropy: </span></span>A measure"
        ),
        "{html}"
    );
    // Inside its own definition, the term is a span of its role, no link.
    assert!(html.contains("the <span class=\"flm-pres-span flm-defterm-term\">Entropy</span> of a system"), "{html}");
    // Outside, a link of kind `term` to the anchor, by the term, a bracket or
    // the alias.
    for shown in ["Entropy", "its entropy", "Entropies"] {
        assert!(
            html.contains(&format!(
                "<a class=\"flm-pres-link\" href=\"#defterm-Entropy\" data-flm-kind=\"term\">{shown}</a>"
            )),
            "{shown}: {html}"
        );
    }
    // A reference to the written label shows the term.
    assert!(
        html.contains("<a class=\"flm-pres-link\" href=\"#defterm-Entropy\" data-flm-kind=\"ref\">Entropy</a>"),
        "{html}"
    );
    // Inside an `\href`'s text the term link is no second `<a>`: a span with
    // the link's class and kind, the address being the outer link's.
    assert!(
        html.contains(
            "<a class=\"flm-pres-link\" href=\"https://example.com/\" data-flm-kind=\"url\">a page on <span class=\"flm-pres-link\" data-flm-kind=\"term\">Entropy</span></a>"
        ),
        "{html}"
    );
    common::html::assert_golden("defterm.html", &html);
}

/// A definition of `term` holding one paragraph that uses its own term.
fn definition_of(term: &str) -> String {
    format!("\\begin{{defterm}}{{{term}}}\nWhat \\term{{{term}}} means.\n\\end{{defterm}}\n")
}

#[test]
fn the_page_of_a_richer_lead_in_and_of_none() {
    let richer = with_config(
        DeftermConfig::new().lead_in(Some(String::from("\\textbf{Definition} (\\emph{#{term}}): "))),
    );
    let html = html_output(&richer, &definition_of("Entropy"));
    assert!(
        html.contains(
            "<span class=\"flm-pres-deco-runin\"><span class=\"flm-pres-span flm-defterm-leadin\"><strong class=\"flm-pres-span\" data-flm-style=\"bold\">Definition</strong> (<em class=\"flm-pres-span\" data-flm-style=\"emph\">Entropy</em>): </span></span>What"
        ),
        "{html}"
    );
    common::html::assert_golden("defterm-lead-in-richer.html", &html);
    let html = html_output(&with_config(DeftermConfig::new().lead_in(None)), &definition_of("Entropy"));
    assert!(!html.contains("flm-pres-deco-runin"), "{html}");
    assert!(
        html.contains("<div class=\"flm-pres-block flm-defterm\" id=\"defterm-Entropy\">\n  <p class=\"flm-pres-par\">What <span"),
        "{html}"
    );
    common::html::assert_golden("defterm-lead-in-none.html", &html);
}

#[test]
fn the_stylesheet_is_required_by_every_definition() {
    let run = html_run(&std(), &definition_of("Entropy"));
    assert!(requires_style(&run));
    let page = common::html::page(&run);
    let DefContent::Inline(css) = &defterm::html::stylesheet().content[0] else { panic!("inline") };
    assert!(page.contains(css.trim_end()), "{page}");
    assert!(css.contains("--flm-defterm-term-style"), "{css}");
    // A definition not using its own term requires it too, since the
    // definition's rule requires it at every definition.
    let run = html_run(
        &std(),
        "\\begin{defterm}{Entropy}\nA measure.\n\\end{defterm}\n\nSee \\term{Entropy}.\n",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(requires_style(&run));
    // A term link alone, its definition absent, needs none of it.
    let run = html_run(&std(), "See \\term{Entropy}.\n");
    assert!(!requires_style(&run));
}

#[test]
fn a_link_inside_a_link_keeps_the_outer_address_on_every_nesting() {
    // The older case of a `\ref` inside a `\hyperref`, and a `\url` inside an
    // `\href`: one `<a>` each, the inner link a span with no address, and
    // nothing reported.
    let run = html_run(
        &std(),
        "\\section{A}\\label{sec:a}\n\n\\hyperref[sec:a]{see \\ref{sec:a}} and \\href{https://x.org}{\\url{https://y.org}}.\n",
    );
    assert_eq!(reports(&run), Vec::<(String, Severity)>::new(), "{}", run.diagnostics().render_all());
    let html = common::html::fragment(&run);
    assert_eq!(html.matches("<a ").count(), 2, "{html}");
    assert!(html.contains("<span class=\"flm-pres-link\" data-flm-kind=\"ref\">"), "{html}");
    assert!(html.contains("<span class=\"flm-pres-link\" data-flm-kind=\"url\">y.org</span>"), "{html}");
    assert!(!html.contains("y.org\""), "{html}");
}

#[test]
fn a_note_s_mark_inside_a_link_s_text_is_the_same_case() {
    // A footnote's mark is a link to the note: inside an `\href`'s text it is
    // written as its text, as any link inside a link, with no document role
    // and no accessible name, and nothing is reported.
    let run = html_run(&std(), "See \\href{https://x.org}{the page\\footnote{A note.}}.\n");
    assert_eq!(reports(&run), Vec::<(String, Severity)>::new(), "{}", run.diagnostics().render_all());
    let html = common::html::fragment(&run);
    assert_eq!(html.matches("<a ").count(), 1, "{html}");
    assert!(html.contains("<a class=\"flm-pres-link\" href=\"https://x.org\""), "{html}");
    assert!(html.contains("<span class=\"flm-pres-link\" data-flm-kind=\"xnote\">1</span>"), "{html}");
    assert!(!html.contains("doc-noteref"), "{html}");
}

// ---------------------------------------------------------------------------------------------
// the recomposed LaTeX of a definition (test 14, its first half)
// ---------------------------------------------------------------------------------------------

/// A definition with an alias and two written labels, of the types `topic`
/// and `term`, whose body is two paragraphs, and a reference to each label,
/// which prints the term.
const DEFINITION_DOCUMENT: &str = "\\section{Terms}

\\begin{defterm}{Entropy}\\label{topic:entropy}\\label{term:entropy}\\aliasterm{Entropies}
A measure of disorder.

It never decreases in an isolated system.
\\end{defterm}

See \\ref{topic:entropy} and \\ref{term:entropy}.
";

#[test]
fn a_definition_is_written_back_as_its_environment_with_its_labels_and_the_term() {
    let run = recompose_clean(&std(), DEFINITION_DOCUMENT);
    let output = common::latex::output(&run);
    // The environment as written, the term its argument; the body opens with
    // the anchor and the labels with the term as their display text: the
    // computed label, the alias's, then the written ones, each a safe key;
    // then `\\ignorespaces`, so that the line break adds no second space after
    // the lead-in's own.
    assert!(
        output.contains(
            "\\begin{defterm}{Entropy}\\phantomsection\\flmLabelsWithDisplayText{Entropy}{\\flmLabels{\\label{ref1}\\label{ref2}\\label{ref3}\\label{ref4}}}\\ignorespaces\nA measure"
        ),
        "{output}"
    );
    assert!(output.contains("\\end{defterm}"), "{output}");
    // The references print the term through the display text.
    assert!(output.contains("See \\flmRef{ref3} and \\flmRef{ref4}."), "{output}");
    let (style, _) = common::latex::style(&run);
    // The environment is defined from the default lead-in, guarded, apart from
    // the document's own definitions.
    assert!(
        style.contains(
            "% --- definitions written from the configuration of FLM features ---\n% defterm (the defterm feature's, written from its configuration): defined unless the name is already taken"
        ),
        "{style}"
    );
    assert!(style.contains("\\ProvideDocumentEnvironment{defterm}{+m}{#1: }{}\n"), "{style}");
    assert!(!style.contains("% --- user definitions ---"), "{style}");
    // The piece requests what the body uses.
    for request in [
        "% flm.latex.def.defterm",
        "\\RequirePackage{hyperref}",
        "\\ProvideDocumentCommand\\flmLabelsWithDisplayText",
        "\\zcRefTypeSetup{flm--internal--ref}",
    ] {
        assert!(style.contains(request), "{request}: {style}");
    }
    common::latex::assert_golden("defterm-definition.tex", &run);
}

/// The style file of the recomposition of `document` under `registry`, with
/// nothing reported.
fn style_of(registry: &Arc<FlmRegistry>, document: &str) -> String {
    common::latex::style(&recompose_clean(registry, document)).0
}

#[test]
fn the_environment_s_begin_code_follows_the_configured_lead_in() {
    let document = "\\begin{defterm}{Entropy}\\label{topic:entropy}\nA measure.\n\\end{defterm}\n\nSee \\ref{topic:entropy}.\n";
    let richer = with_config(
        DeftermConfig::new().lead_in(Some(String::from("\\textbf{Definition} (\\emph{#{term}}): "))),
    );
    let run = recompose_clean(&richer, document);
    let style = common::latex::style(&run).0;
    assert!(
        style.contains(
            "\\ProvideDocumentEnvironment{defterm}{+m}{\\textbf{Definition} (\\emph{#1}): }{}\n"
        ),
        "{style}"
    );
    common::latex::assert_golden("defterm-lead-in-richer.tex", &run);
    let run = recompose_clean(&with_config(DeftermConfig::new().lead_in(None)), document);
    let style = common::latex::style(&run).0;
    assert!(style.contains("\\ProvideDocumentEnvironment{defterm}{+m}{}{}\n"), "{style}");
    common::latex::assert_golden("defterm-lead-in-none.tex", &run);
    // A literal `#` of a template, written `##`, is a character there, not a
    // parameter.
    let hash = with_config(DeftermConfig::new().lead_in(Some(String::from("##. #{term}: "))));
    let style = style_of(&hash, document);
    assert!(style.contains("\\ProvideDocumentEnvironment{defterm}{+m}{\\#. #1: }{}\n"), "{style}");
}

#[test]
fn with_the_label_hooks_off_the_labels_are_written_bare_after_the_anchor() {
    let overrides = Overrides {
        render_options: OptionValues::new()
            .with(LatexRecomposedRenderOptions { label_hooks: false, ..Default::default() }),
        ..Overrides::none()
    };
    let run = recompose_under(
        &std(),
        "\\begin{defterm}{Entropy}\\label{topic:entropy}\nA measure.\n\\end{defterm}\n",
        &overrides,
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let output = common::latex::output(&run);
    assert!(
        output.contains(
            "\\begin{defterm}{Entropy}\\phantomsection\\label{ref1}\\label{ref2}\\ignorespaces\nA measure."
        ),
        "{output}"
    );
    assert!(!output.contains("flmLabelsWithDisplayText"), "{output}");
}

#[test]
fn an_empty_term_is_reported_on_the_recomposed_latex_too() {
    let run = recompose_under(
        &std(),
        "\\begin{defterm}{ }\nA measure.\n\\end{defterm}\n",
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), ["flm.defterm.empty-term"], "{}", run.diagnostics().render_all());
    assert!(common::latex::output(&run).contains("\\begin{defterm}{ }"), "{}", common::latex::output(&run));
}

// ---------------------------------------------------------------------------------------------
// the recomposed LaTeX of a term link (test 14, its second half)
// ---------------------------------------------------------------------------------------------

/// Two definitions, each using a term: its own, the other's, by a bracket;
/// term links in a heading's title — a moving argument — by the alias, and in
/// an `\\href`'s text; a reference to a written label.
const TERMS_DOCUMENT: &str = "\\section{About \\term{Entropy}}

\\begin{defterm}{Entropy}\\label{topic:entropy}\\aliasterm{Entropies}
A measure of disorder: the \\term{Entropy} of a system never decreases as its
\\term{Temperature} is kept.
\\end{defterm}

\\begin{defterm}{Temperature}
What a thermometer reads; not the \\term[Entropy]{entropy}.
\\end{defterm}

All \\term{Entropies} grow; see \\ref{topic:entropy} and
\\href{https://example.com/}{a page on \\term{Entropy}}.
";

#[test]
fn a_term_link_is_written_as_its_hook_and_inside_its_own_definition_as_the_other() {
    let run = recompose_clean(&std(), TERMS_DOCUMENT);
    let output = common::latex::output(&run);
    // In a heading's title, before the definition: the definition's key.
    assert!(output.contains("\\section{About \\flmTerm{ref1}{Entropy}}"), "{output}");
    // Inside its own definition, the term being defined; another term there is
    // a link.
    assert!(output.contains("the \\flmTermInDefterm{Entropy} of a system"), "{output}");
    assert!(output.contains("its\n\\flmTerm{ref4}{Temperature} is kept"), "{output}");
    // By a bracket, by the alias, in an `\\href`'s text.
    assert!(output.contains("not the \\flmTerm{ref1}{entropy}."), "{output}");
    assert!(output.contains("All \\flmTerm{ref2}{Entropies} grow; see \\flmRef{ref3}"), "{output}");
    assert!(output.contains("\\flmHref{https://example.com/}{a page on \\flmTerm{ref1}{Entropy}}"), "{output}");
    let (style, _) = common::latex::style(&run);
    assert!(style.contains("\\ProvideDocumentCommand\\flmTerm{m m}{\\flmHyperref{#1}{#2}}"), "{style}");
    assert!(style.contains("\\ProvideDocumentCommand\\flmTermInDefterm{m}{\\emph{#1}}"), "{style}");
    assert!(style.contains("% flm.latex.def.hyperref"), "{style}");
    common::latex::assert_golden("defterm-terms.tex", &run);
}

#[test]
fn an_unresolved_term_or_equation_reference_is_written_under_an_allocated_key() {
    // A term no definition carries, one whose term holds markup, and an
    // `\eqref` to no equation: each reference's target is allocated a safe
    // key as an unresolved `\hyperref`'s is, so no markup reaches a LaTeX
    // label name, and each is reported as unresolved. The golden is compiled
    // under both engines by `tests/latex_compile.rs`.
    let run = recompose_under(
        &std(),
        "See \\term{Nothing}, \\term{\\(\\alpha\\)-decay} and \\eqref{eq:none}.\n",
        &Overrides::none(),
    );
    assert_eq!(
        reports(&run),
        [
            (String::from("flm.refs.unresolved"), Severity::Error),
            (String::from("flm.refs.unresolved"), Severity::Error),
            (String::from("flm.refs.unresolved"), Severity::Error),
        ],
        "{}",
        run.diagnostics().render_all()
    );
    let output = common::latex::output(&run);
    assert!(
        output.contains(
            "See \\flmTerm{ref1}{Nothing}, \\flmTerm{ref2}{\\(\\alpha\\)-decay} and \\eqref{ref3}."
        ),
        "{output}"
    );
    common::latex::assert_golden("defterm-unresolved.tex", &run);
    // An empty term: reported, an empty label.
    let run = recompose_under(&std(), "See \\term{ }.\n", &Overrides::none());
    assert!(identifiers(&run).contains(&String::from("flm.defterm.empty-term")), "{}", run.diagnostics().render_all());
    assert!(common::latex::output(&run).contains("See \\flmTerm{}{ }."), "{}", common::latex::output(&run));
}

/// A macro `name` of one argument whose template is `template`, kept as a
/// LaTeX definition on the recomposed LaTeX.
fn kept(name: &str, template: &str) -> Definition {
    Definition::macro_(name, vec![ArgDef::mandatory("term")])
        .text(template)
        .expand(ExpandPolicy::PerPlan)
}

/// The name and the reason of every per-plan user definition of `registry`
/// that is not recomposable.
fn not_recomposable(registry: &FlmRegistry) -> Vec<(String, NotRecomposable)> {
    registry
        .elements()
        .iter()
        .filter_map(|entry| entry.element.value_as::<UserDefinition>())
        .filter(|value| value.definition().expand == ExpandPolicy::PerPlan)
        .filter_map(|value| Some((value.definition().display_name(), value.recomposable().err()?)))
        .collect()
}

#[test]
fn a_kept_template_whose_term_link_holds_a_placeholder_is_expanded_at_each_use_site() {
    let registry = with_definitions(vec![
        // A fixed term: kept, its safe key written in the style file.
        kept("seeentropy", "(see \\term{Entropy} and #1)"),
        // A placeholder in the bracket, in the braces with no bracket, and
        // inside markup: the label is computed from the term a call supplies,
        // so the rule cannot write it for the definition, and each use site is
        // expanded. The definitions are recomposable all the same: it is the
        // lowering of their template that expands the use sites.
        kept("linkto", "\\term[#1]{this term}"),
        kept("plain", "\\term{#1}"),
        kept("styled", "\\term[\\emph{#1} chain]{it}"),
    ]);
    assert_eq!(not_recomposable(&registry), Vec::<(String, NotRecomposable)>::new());
    let document = "\\begin{defterm}{Entropy}\nA measure.\n\\end{defterm}\n\n\\begin{defterm}{\\emph{Markov} chain}\nA process.\n\\end{defterm}\n\n\\seeentropy{x} \\linkto{Entropy} \\plain{Entropy} \\styled{Markov}\n";
    // Every link resolves, nothing is reported.
    let run = recompose_clean(&registry, document);
    let output = common::latex::output(&run);
    assert!(
        output.contains(
            "\\flmUDseeentropy{x} \\flmTerm{ref1}{this term} \\flmTerm{ref1}{Entropy} \\flmTerm{ref2}{it}"
        ),
        "{output}"
    );
    let (style, _) = common::latex::style(&run);
    assert!(
        style.contains("\\providecommand\\flmUDseeentropy[1]{(see \\flmTerm{ref1}{Entropy} and #1)}"),
        "{style}"
    );
    for name in ["linkto", "plain", "styled"] {
        assert!(!style.contains(&format!("\\flmUD{name}")), "{name}: {style}");
    }
    common::latex::assert_golden("defterm-userdefs-kept.tex", &run);
}

#[test]
fn a_placeholder_nested_deep_in_a_kept_terms_markup_expands_the_use_site() {
    // The term is read by its spelling, wherever in its markup the placeholder
    // stands: two commands deep here.
    let registry = with_definitions(vec![kept("deep", "\\term[\\textbf{\\emph{#1}} chain]{it}")]);
    let document = "\\begin{defterm}{\\textbf{\\emph{Markov}} chain}\nA process.\n\\end{defterm}\n\n\\deep{Markov}\n";
    let run = recompose_clean(&registry, document);
    let output = common::latex::output(&run);
    assert!(output.contains("\\flmTerm{ref1}{it}"), "{output}");
    let (style, _) = common::latex::style(&run);
    assert!(!style.contains("\\flmUDdeep"), "{style}");
}
