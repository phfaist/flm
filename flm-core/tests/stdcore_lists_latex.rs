//! The `lists` feature in the recomposed LaTeX: the three shipped kinds,
//! nesting, tags, labels on items and references to them, a list's own label
//! template written as the `enumitem` package's `label=` key, a template
//! holding markup recomposed as content, a list beside the other features inside an item, a list
//! standing in a paragraph's run of text, and a list environment a host adds
//! whose LaTeX the author supplies.
//!
//! Every golden this suite writes under `tests/goldens/latex/` is compiled by
//! `tests/latex_compile.rs` under `pdflatex` and `lualatex` when
//! `FLM_TESTS_LATEX_COMPILE` is set.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::latex::{ExtraPreamble, LatexPackageDef};
use flm_core::stdcore::backends::latex_recomposed::{LatexRecomposedRenderOptions, WORKFLOW};
use flm_core::stdcore::backends::{html, text};
use flm_core::stdcore::features::lists::{ListKind, ListsConfig};

use common::latex::{assert_golden, body, style_if_any};
use common::lists::ACROSS_FEATURES;
use common::stdcore::{identifiers, registry, run_workflow};

// ---------------------------------------------------------------------------------------------
// running a document
// ---------------------------------------------------------------------------------------------

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// The recomposition of `texts`, one tree each, under the standard package.
fn recompose(texts: &[&str]) -> Run {
    run_workflow(&std(), WORKFLOW, texts, &Overrides::none())
}

/// The recomposition of `texts`; the run must report nothing.
fn clean(texts: &[&str]) -> Run {
    let run = recompose(texts);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    run
}

/// Whether the run's style file requests the `enumitem` package.
fn requests_enumitem(run: &Run) -> bool {
    style_if_any(run).is_some_and(|(style, _)| style.contains("\\RequirePackage{enumitem}\n"))
}

// ---------------------------------------------------------------------------------------------
// the kinds, nesting, tags and references
// ---------------------------------------------------------------------------------------------

/// The three kinds, nested, with tags, labels and references: the list is
/// written back as the document wrote it, the labels inside the labels hook.
const KINDS: &str = "\\section{Lists}\n\n\
    \\begin{enumerate}\n\
    \\item\\label{item:a} First, before \\ref{item:b}.\n\
    \\item\\label{item:b} Second, holding a list:\n\
    \\begin{enumerate}\n\
    \\item Inner.\n\
    \\item[Custom]\\label{item:t} Tagged.\n\
    \\end{enumerate}\n\
    \\item Third.\n\
    \\end{enumerate}\n\n\
    \\begin{itemize}\n\
    \\item A bullet.\n\
    \\item[tag={\\emph{Rich} tag}] A tag with markup.\n\
    \\item[{a]b}] A tag holding a bracket.\n\
    \\end{itemize}\n\n\
    \\begin{description}\n\
    \\item[Entropy]\\label{item:e} A measure.\n\
    \\item[Energy] Another.\n\
    \\end{description}\n\n\
    See \\ref{item:a}, \\ref[noprefix]{item:t} and \\ref{item:e}.";

#[test]
fn a_list_is_written_back_as_its_environment_and_its_items() {
    let run = clean(&[KINDS]);
    let body = body(&run);
    for written in [
        "\\begin{enumerate}\n\\item\\flmLabels{\\label{ref1}} First, before \\flmRef{ref2}.\n",
        "\\item[Custom]\\flmLabels{\\label{ref3}} Tagged.\n",
        // A tag holding a command is braced, so that nothing in it is read
        // as the end of the bracket.
        "\\item[{\\emph{Rich} tag}] A tag with markup.\n",
        // So is a tag holding a `]`.
        "\\item[{a]b}] A tag holding a bracket.\n",
        "\\begin{description}\n\\item[Entropy]\\flmLabels{\\label{ref4}} A measure.\n",
    ] {
        assert!(body.contains(written), "{written}: {body}");
    }
    // The labels beside the items are LaTeX's own: nothing asks for the
    // package that writes a list's own label.
    assert!(!requests_enumitem(&run));
    assert_golden("lists.tex", &run);
}

// ---------------------------------------------------------------------------------------------
// a list's own template
// ---------------------------------------------------------------------------------------------

#[test]
fn a_list_s_own_template_is_written_as_the_enumitem_label_key() {
    let run = clean(&["\\begin{enumerate}[label=(#{alph})]\\item a\\item b\\end{enumerate}\n\n\
                       \\begin{enumerate}[(i)]\\item a\\item b\\end{enumerate}\n\n\
                       \\begin{enumerate}[1.]\\item a\\end{enumerate}\n\n\
                       \\begin{enumerate}[A)]\\item a\\end{enumerate}\n\n\
                       \\begin{enumerate}[label=#{Roman}.]\\item a\\end{enumerate}\n\n\
                       \\begin{enumerate}[label={*}]\\item a\\end{enumerate}\n\n\
                       \\begin{itemize}[label={--}]\\item a\\end{itemize}\n\n\
                       \\begin{itemize}[-]\\item a\\end{itemize}\n\n\
                       \\begin{itemize}[*]\\item a\\end{itemize}\n\n\
                       \\begin{itemize}[+]\\item a\\end{itemize}\n\n\
                       \\begin{itemize}[label={}]\\item a\\end{itemize}\n\n\
                       \\begin{itemize}[label={§ a, b]}]\\item a\\end{itemize}\n\n\
                       \\begin{enumerate}[label={Step~#{arabic}:}]\\item a\\end{enumerate}\n\n\
                       \\begin{enumerate}[label={50\\% #{arabic}}]\\item a\\end{enumerate}\n\n\
                       \\begin{enumerate}[label={{Q}#{arabic}}]\\item a\\end{enumerate}"]);
    let body = body(&run);
    for written in [
        "\\begin{enumerate}[label={(\\alph*)}]",
        "\\begin{enumerate}[label={(\\roman*)}]",
        // A short form's reference template drops its trailing full stop, so
        // it is written beside the label; a `label=` sets both, one key.
        "\\begin{enumerate}[label={\\arabic*.}, ref={\\arabic*}]",
        "\\begin{enumerate}[label={\\Alph*)}]",
        "\\begin{enumerate}[label={\\Roman*.}]",
        "\\begin{enumerate}[label={*}]",
        "\\begin{itemize}[label={--}]",
        "\\begin{itemize}[label={-}]",
        "\\begin{itemize}[label={*}]",
        "\\begin{itemize}[label={+}]",
        "\\begin{itemize}[label={}]",
        // An escaped character is written as it is everywhere in the output,
        // and a brace group holding text, a format or such a character with
        // its braces: `Step 1:`, `50% 1` and `Q1` in the compiled document.
        "\\begin{enumerate}[label={Step~\\arabic*:}]",
        "\\begin{enumerate}[label={50\\% \\arabic*}]",
        "\\begin{enumerate}[label={{Q}\\arabic*}]",
    ] {
        assert!(body.contains(written), "{written}: {body}");
    }
    assert!(requests_enumitem(&run));
    assert_golden("lists-templates.tex", &run);
}

/// Lists whose own templates hold markup: a command around the number, a
/// command beside it, a formula, a user definition taking the number as its
/// argument, and a `ref=` holding a word and a non-breaking space.
const MARKUP: &str = "\\begin{enumerate}[label={\\textbf{#{arabic}}.}]\\item a\\end{enumerate}\n\n\
    \\begin{enumerate}[label={\\emph{Q}#{alph}}, ref={Step~#{arabic}:}]\\item\\label{item:q} b\\end{enumerate}\n\n\
    \\begin{enumerate}[label={\\(x\\) #{roman}}]\\item c\\end{enumerate}\n\n\
    \\begin{enumerate}[label={\\hello{#{Alph}}}]\\item d\\end{enumerate}\n\n\
    \\begin{itemize}[label={\\textbf{--}}]\\item e\\end{itemize}\n\n\
    See \\ref{item:q}.";

#[test]
fn a_list_s_own_template_holding_markup_is_recomposed_as_content() {
    use flm_core::lang::mint::ArgDef;
    use flm_core::lang::rewrites::userdefs::Definition;
    use flm_core::stdcore::features::userdefs::UserdefsConfig;
    let config = StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(vec![
        Definition::macro_("hello", vec![ArgDef::mandatory("name")]).text("Hello, #{name}!"),
    ]));
    let run = run_workflow(&registry(config.clone()), WORKFLOW, &[MARKUP], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let body = body(&run);
    for written in [
        // The template is content: each command written as the rules write
        // it, the number as the package's command inside it.
        "\\begin{enumerate}[label={\\textbf{\\arabic*}.}]",
        "\\begin{enumerate}[label={\\emph{Q}\\alph*}, ref={Step~\\arabic*:}]",
        // A formula as the recomposed LaTeX writes every formula.
        "\\begin{enumerate}[label={\\(x\\) \\roman*}]",
        // A user definition as the recomposed LaTeX writes every use of one
        // — a call of its definition in the style file — the number standing
        // as its argument.
        "\\begin{enumerate}[label={\\flmUDhello{\\Alph*}}]",
        "\\begin{itemize}[label={\\textbf{--}}]",
    ] {
        assert!(body.contains(written), "{written}: {body}");
    }
    assert!(requests_enumitem(&run));
    // The other backends write the same labels, the number in place of the
    // package's command, and report nothing: the compiled recomposed LaTeX
    // prints a bold `1.`, `Qa`, `x i`, `Hello, A!`, a bold `--` and `Step 1:`.
    let on_text = run_workflow(&registry(config.clone()), text::WORKFLOW, &[MARKUP], &Overrides::none());
    assert_eq!(identifiers(&on_text), Vec::<String>::new());
    let printed = common::lowering::text(&on_text);
    for written in ["1. a", "Qa b", "x i c", "Hello, A! d", "\u{2013} e", "See Item\u{a0}Step\u{a0}1:."] {
        assert!(printed.contains(written), "{written}: {printed}");
    }
    let on_html = run_workflow(&registry(config), html::WORKFLOW, &[MARKUP], &Overrides::none());
    assert_eq!(identifiers(&on_html), Vec::<String>::new());
    let fragment = common::html::fragment(&on_html);
    for label in [
        "<strong class=\"flm-pres-span\" data-flm-style=\"bold\">1</strong>.",
        "<em class=\"flm-pres-span\" data-flm-style=\"emph\">Q</em>a",
        "<span class=\"flm-pres-math--passthrough\" data-flm-display=\"inline\">\\(x\\)</span> i",
        "Hello, A!",
        "<strong class=\"flm-pres-span\" data-flm-style=\"bold\">\u{2013}</strong>",
    ] {
        let span = format!("<span class=\"flm-pres-list-label\">{label}</span>");
        assert!(fragment.contains(&span), "{span}: {fragment}");
    }
    assert_golden("lists-label-markup.tex", &run);
}

/// Lists whose own templates write a number format inside a formula and
/// inside an inline verbatim command.
const POINTS: [&str; 2] = [
    "\\begin{enumerate}[label={\\(#{arabic}\\)}]\\item a\\end{enumerate}",
    "\\begin{enumerate}[label={\\verbtext|x#{arabic}|}]\\item b\\end{enumerate}",
];

#[test]
fn a_number_format_inside_a_formula_is_the_package_s_command_and_inside_verbatim_content_an_error() {
    // Inside mathematics, which is written from its own nodes rather than
    // through the rules, the number format is the package's command.
    let formula = clean(&[POINTS[0]]);
    let written = "\\begin{enumerate}[label={\\(\\arabic*\\)}]\\item a";
    assert!(body(&formula).contains(written), "{}", body(&formula));
    assert_golden("lists-label-points.tex", &formula);
    // Inside verbatim content the characters are read when the recomposed
    // LaTeX is written, before any number exists, so no command can stand
    // for the number there: reported, and the key is not written.
    let verbatim = recompose(&[POINTS[1]]);
    assert_eq!(
        identifiers(&verbatim),
        ["flm.lists.number-read-as-characters"],
        "{}",
        verbatim.diagnostics().render_all()
    );
    assert!(body(&verbatim).contains("\\begin{enumerate}\\item b"), "{}", body(&verbatim));
    assert!(!requests_enumitem(&verbatim));
    // The other backends write the number there and report nothing.
    let document = POINTS.join("\n\n");
    let on_text = run_workflow(&std(), text::WORKFLOW, &[&document], &Overrides::none());
    assert_eq!(identifiers(&on_text), Vec::<String>::new());
    let printed = common::lowering::text(&on_text);
    assert!(printed.contains("1 a") && printed.contains("x1 b"), "{printed}");
    let on_html = run_workflow(&std(), html::WORKFLOW, &[&document], &Overrides::none());
    assert_eq!(identifiers(&on_html), Vec::<String>::new());
    let fragment = common::html::fragment(&on_html);
    for label in [
        "<span class=\"flm-pres-math--passthrough\" data-flm-display=\"inline\">\\(1\\)</span>",
        "<code class=\"flm-pres-verbatim\" data-flm-kind=\"text\" data-flm-display=\"inline\">x1</code>",
    ] {
        let span = format!("<span class=\"flm-pres-list-label\">{label}</span>");
        assert!(fragment.contains(&span), "{span}: {fragment}");
    }
}

#[test]
fn a_number_format_enumitem_does_not_write_is_reported_and_its_key_left_out() {
    let run = recompose(&["\\begin{enumerate}[label=#{fnsymbol}]\\item a\\end{enumerate}\n\n\
                           \\begin{enumerate}[label={\\textbf{#{unicodesuperscript}}}]\\item b\\end{enumerate}\n\n\
                           \\begin{enumerate}[label=#{arabic}., ref=#{fnsymbol}]\\item c\\end{enumerate}\n\n\
                           \\begin{enumerate}[label=#{unicodesubscript}]\\item d\\end{enumerate}"]);
    assert_eq!(
        identifiers(&run),
        [
            "flm.lists.no-enumitem-format",
            "flm.lists.no-enumitem-format",
            "flm.lists.no-enumitem-format",
            "flm.lists.no-enumitem-format"
        ],
        "{}",
        run.diagnostics().render_all()
    );
    let body = body(&run);
    // The key is not written: LaTeX's own label stands beside the items, and
    // the label template serves references.
    assert!(body.contains("\\begin{enumerate}\\item a\\end{enumerate}"), "{body}");
    assert!(body.contains("\\begin{enumerate}\\item b\\end{enumerate}"), "{body}");
    assert!(body.contains("\\begin{enumerate}[label={\\arabic*.}]\\item c"), "{body}");
    assert!(body.contains("\\begin{enumerate}\\item d\\end{enumerate}"), "{body}");
    // The other backends write the number itself and report nothing.
    for workflow in [text::WORKFLOW, html::WORKFLOW] {
        let other = run_workflow(
            &std(),
            workflow,
            &["\\begin{enumerate}[label=#{fnsymbol}]\\item a\\end{enumerate}"],
            &Overrides::none(),
        );
        assert_eq!(identifiers(&other), Vec::<String>::new(), "{workflow}");
    }
}

#[test]
fn a_template_that_cannot_be_used_is_reported_once_and_written_as_no_label() {
    // A template that does not compile on an unnumbered list and one naming a
    // number there: the list's rule reports each, as on every backend, and
    // the list is written with no `label=`. On a numbered list the numbering
    // stage reports the template and the rule reports nothing more.
    let run = recompose(&["\\begin{itemize}[label=#{nope}]\\item a\\end{itemize}\n\n\
                           \\begin{itemize}[(i)]\\item a\\end{itemize}\n\n\
                           \\begin{enumerate}[label=#{nope}]\\item a\\end{enumerate}"]);
    assert_eq!(
        identifiers(&run),
        ["flm.numbering.bad-number-format", "flm.lists.bad-label", "flm.lists.label-names-number"]
    );
    let body = body(&run);
    assert!(body.contains("\\begin{itemize}\\item a\\end{itemize}"), "{body}");
    assert!(body.contains("\\begin{enumerate}\\item a\\end{enumerate}"), "{body}");
    assert!(!requests_enumitem(&run));
}

// ---------------------------------------------------------------------------------------------
// beside the other features, and a host's environment
// ---------------------------------------------------------------------------------------------

#[test]
fn an_item_holds_a_formula_a_reference_a_verbatim_block_and_a_nested_list() {
    let run = clean(&[ACROSS_FEATURES]);
    let body = body(&run);
    // The list in running text stays where the document wrote it: LaTeX
    // ends the paragraph at a list and goes on after it.
    assert!(
        body.contains("a list in running text \\begin{itemize}\\item one\\item two\\end{itemize} and"),
        "{body}"
    );
    assert!(body.contains("\\begin{flmVerbatimCode}"), "{body}");
    assert!(body.contains("\\flmRef{ref2}"), "{body}");
    assert_golden("lists-across-features.tex", &run);
}

#[test]
fn a_host_s_list_environment_is_written_under_its_own_name_for_the_author_s_latex() {
    // The environment a host configures is written under its own name, as
    // every list is; the author's LaTeX defines it — here through the
    // package that writes a list's own label, and a definition after it.
    let config = StdcoreConfig::new().lists(
        ListsConfig::new().environment("steps", ListKind::numbered(["Step #{arabic}:"])),
    );
    let render = LatexRecomposedRenderOptions {
        extra_packages: vec![LatexPackageDef::new("enumitem", "the author's list environment")],
        extra_preamble: ExtraPreamble {
            after_packages: String::from(
                "% the list environment the host configured\n\
                 \\newlist{steps}{enumerate}{3}\n\
                 \\setlist[steps]{label={Step \\arabic*:}}\n",
            ),
            ..ExtraPreamble::default()
        },
        ..LatexRecomposedRenderOptions::default()
    };
    let overrides =
        Overrides { render_options: OptionValues::new().with(render), ..Overrides::none() };
    let run = run_workflow(
        &registry(config),
        WORKFLOW,
        &["\\begin{steps}\\item\\label{item:s} Mix.\\item Bake, as \\ref{item:s} says.\\end{steps}"],
        &overrides,
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(body(&run).contains("\\begin{steps}\\item\\flmLabels{\\label{ref1}} Mix."));
    assert_golden("lists-configured.tex", &run);
}

// ---------------------------------------------------------------------------------------------
// registries without the numbering or the references feature, and deep nesting
// ---------------------------------------------------------------------------------------------

#[test]
fn without_the_numbering_feature_a_numbered_list_is_reported_and_still_recomposed() {
    use flm_core::core::registry::FlmFeature;
    use flm_core::stdcore::backends::lowering::feature as lowering_feature;
    use flm_core::stdcore::backends::{latex, latex_recomposed};
    use flm_core::stdcore::check_feature;
    use flm_core::stdcore::features::blocks::Blocks;
    use flm_core::stdcore::features::{index, refpoints, lists};
    let minimal = common::stdcore::registry_over(vec![
        lowering_feature(),
        latex::feature(),
        latex_recomposed::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        Arc::new(Blocks::default()) as Arc<dyn FlmFeature>,
        check_feature(),
        index::feature(),
        refpoints::feature(),
        lists::feature(),
    ]);
    let document = "\\begin{enumerate}[label=(#{alph})]\\item\\label{item:a} a\\item b\\end{enumerate}\
                    \\begin{enumerate}[label=#{nope}]\\item c\\end{enumerate}\
                    \\begin{enumerate}[label={*}]\\item d\\end{enumerate}\
                    \\begin{enumerate}\\item[T] e\\end{enumerate}";
    let run = run_workflow(&minimal, WORKFLOW, &[document], &Overrides::none());
    // With nothing numbering the items, the rule compiles the template it
    // writes and reports the one that does not compile, and reports each
    // numbered list whose items would show a number, once, as the other
    // backends do; the list is written all the same, and LaTeX numbers it. A
    // fixed label and a list whose every item is tagged show no number.
    assert_eq!(
        identifiers(&run),
        ["flm.lists.no-numbering", "flm.lists.bad-label", "flm.lists.no-numbering"],
        "{}",
        run.diagnostics().render_all()
    );
    let on_text = run_workflow(&minimal, text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&on_text), identifiers(&run));
    let body = body(&run);
    assert!(body.contains("\\begin{enumerate}[label={(\\alph*)}]\\item"), "{body}");
    assert!(body.contains("\\begin{enumerate}\\item c\\end{enumerate}"), "{body}");
    assert!(body.contains("\\begin{enumerate}[label={*}]\\item d\\end{enumerate}"), "{body}");
    assert!(requests_enumitem(&run));
}

#[test]
fn a_registry_without_the_userdefs_feature_recomposes_a_list_s_own_template() {
    // The rule that writes a number format as the package's command is the
    // LaTeX family's, so a registry holding the lists feature without the
    // user definitions recomposes a list's own template in full.
    use flm_core::core::registry::FlmFeature;
    use flm_core::stdcore::backends::lowering::feature as lowering_feature;
    use flm_core::stdcore::backends::{latex, latex_recomposed};
    use flm_core::stdcore::check_feature;
    use flm_core::stdcore::features::blocks::Blocks;
    use flm_core::stdcore::features::{index, lists, math, numbering, refpoints, textstyles};
    let registry = common::stdcore::registry_over(vec![
        lowering_feature(),
        latex::feature(),
        latex_recomposed::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
        Arc::new(Blocks::default()) as Arc<dyn FlmFeature>,
        check_feature(),
        index::feature(),
        refpoints::feature(),
        numbering::feature(),
        textstyles::feature(),
        math::feature(),
        flm_core::math_passthrough::feature(),
        lists::feature(),
    ]);
    assert!(!registry.has_feature("userdefs"));
    let owner = registry.rule(registry.rule_idx(latex::PLACEHOLDER_LATEX_RULE).expect("the rule")).owner.to_string();
    assert_eq!(owner, latex::FEATURE_NAME);
    let document = "\\begin{enumerate}[label={\\textbf{#{arabic}}.}]\\item a\\end{enumerate}\
                    \\begin{enumerate}[label={\\(#{roman}\\)}]\\item b\\end{enumerate}";
    let run = run_workflow(&registry, WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let body = body(&run);
    assert!(body.contains("\\begin{enumerate}[label={\\textbf{\\arabic*}.}]\\item a"), "{body}");
    assert!(body.contains("\\begin{enumerate}[label={\\(\\roman*\\)}]\\item b"), "{body}");
}

#[test]
fn a_list_nested_twenty_deep_is_written_back_whole() {
    // LaTeX itself refuses lists nested this deep; the recomposed LaTeX is
    // written all the same, and nothing is reported.
    for kind in ["enumerate", "itemize"] {
        let open = format!("\\begin{{{kind}}}[label={{*}}]\\item ");
        let close = format!("\\end{{{kind}}}");
        let run = clean(&[&format!("{}deep{}", open.repeat(20), close.repeat(20))]);
        assert_eq!(body(&run).matches("[label={*}]").count(), 20, "{kind}");
    }
}

// ---------------------------------------------------------------------------------------------
// a list's own reference template
// ---------------------------------------------------------------------------------------------

#[test]
fn a_list_s_own_reference_template_is_written_as_the_enumitem_ref_key() {
    let run = recompose(&["\\begin{enumerate}[label={#{arabic}.}, ref={#{arabic}}]\
                           \\item\\label{item:a} a\\item\\label{item:b} b\\end{enumerate}\n\n\
                           \\begin{enumerate}[1.:]\\item\\label{item:c} c\\end{enumerate}\n\n\
                           \\begin{enumerate}[ref=(#{alph})]\\item\\label{item:d} d\\end{enumerate}\n\n\
                           \\begin{enumerate}[label=(#{roman}), ref=(#{roman})]\\item e\\end{enumerate}\n\n\
                           \\begin{itemize}[ref=#{arabic}]\\item f\\end{itemize}\n\n\
                           See \\ref{item:b}, \\ref{item:c} and \\ref{item:d}."]);
    // The one report: a `ref=` on an unnumbered list, which is set aside.
    assert_eq!(identifiers(&run), ["flm.lists.ref-ignored"]);
    let body = body(&run);
    for written in [
        // Both keys where the two templates differ, in the order label, ref.
        "\\begin{enumerate}[label={\\arabic*.}, ref={\\arabic*}]",
        // A short form's reference template drops its trailing punctuation.
        "\\begin{enumerate}[label={\\arabic*.:}, ref={\\arabic*}]",
        // `ref=` alone: the label is LaTeX's own.
        "\\begin{enumerate}[ref={(\\alph*)}]",
        // One key where the author wrote the same template for both.
        "\\begin{enumerate}[label={(\\roman*)}]",
        // Nothing for an unnumbered list's `ref=`.
        "\\begin{itemize}\\item f",
    ] {
        assert!(body.contains(written), "{written}: {body}");
    }
    assert!(requests_enumitem(&run));
    assert_golden("lists-ref-templates.tex", &run);
}

#[test]
fn a_reference_template_that_does_not_compile_is_reported_once_and_not_written() {
    // Under the numbering feature the stage reports it; the label is written.
    let run = recompose(&["\\begin{enumerate}[label=#{alph}., ref=#{nope}]\\item a\\end{enumerate}"]);
    assert_eq!(identifiers(&run), ["flm.numbering.bad-number-format"]);
    assert!(body(&run).contains("\\begin{enumerate}[label={\\alph*.}]\\item"), "{}", body(&run));
    // The other backends report it alike.
    for workflow in [text::WORKFLOW, html::WORKFLOW] {
        let other = run_workflow(
            &std(),
            workflow,
            &["\\begin{enumerate}[label=#{alph}., ref=#{nope}]\\item a\\end{enumerate}"],
            &Overrides::none(),
        );
        assert_eq!(identifiers(&other), ["flm.numbering.bad-number-format"], "{workflow}");
    }
}

#[test]
fn a_list_s_own_label_that_does_not_compile_is_reported_once_on_every_backend_whatever_its_items_are() {
    // The numbering stage compiles the reference template only for a numbered
    // item; every rule lowering the list compiles its own label all the same.
    for items in ["\\item[T] x", ""] {
        let document = format!("\\begin{{enumerate}}[label=#{{nosuch}}, ref=#{{arabic}}]{items}\\end{{enumerate}}");
        for workflow in [WORKFLOW, text::WORKFLOW, html::WORKFLOW] {
            let run = run_workflow(&std(), workflow, &[&document], &Overrides::none());
            assert_eq!(identifiers(&run), ["flm.lists.bad-label"], "{workflow}: {items}");
        }
    }
}

