//! A placeholder stands for the argument's content nodes and nothing else,
//! whatever kind the argument was declared — and content that is not
//! characters inside verbatim content is an Error of every rule staging
//! verbatim content: the `verbatim` feature's, and the default rule of a bare
//! verbatim group that an extension's element lowers through the rules.
//!
//! Each case runs on the three outputs (text, HTML, the recomposed LaTeX),
//! with the definition kept (`per-plan`, a LaTeX definition and its calls on
//! the recomposed LaTeX) and expanded (`always`). The recomposed LaTeX goldens
//! written here are compiled by `tests/latex_compile.rs` under `pdflatex` and
//! `lualatex`, with and without `hyperref`.

mod common;

use std::sync::Arc;

use flm_core::core::lowering::{DefaultMark, Lowering, LoweringDecl};
use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::Run;
use flm_core::lang::mint::{ArgDef, ArgStdKind, ElementDef, SyntaxDef};
use flm_core::stdcore::package;
use flm_core::lang::rewrites::userdefs::{Definition, ExpandPolicy};
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig};
use techy::error::Severity;

use common::latex::{assert_golden, body, style, style_if_any};
use common::stdcore::{identifiers, reports, run_workflow};

/// The condition the verbatim feature reports for content that is not
/// characters.
const NOT_CHARACTERS: &str = "flm.verbatim.content-not-characters";

/// The two expansion policies every case runs under.
const POLICIES: [ExpandPolicy; 2] = [ExpandPolicy::PerPlan, ExpandPolicy::Always];

/// A verbatim argument, delimited by whatever character follows the command.
fn verbatim() -> ArgDef {
    ArgDef::new(ArgStdKind::Verbatim { delimiters: None })
}

/// `\code`: one verbatim argument, into inline verbatim content.
fn code(policy: ExpandPolicy) -> Definition {
    Definition::macro_("code", vec![verbatim()]).text("\\verbcode{#1}").expand(policy)
}

/// `\c`: one verbatim argument, into text.
fn c(policy: ExpandPolicy) -> Definition {
    Definition::macro_("c", vec![verbatim()]).text("see #1").expand(policy)
}

/// `\ocode`: one ordinary argument, into inline verbatim content.
fn ocode(policy: ExpandPolicy) -> Definition {
    Definition::macro_("ocode", vec![ArgDef::mandatory("x")]).text("\\verbcode{#1}").expand(policy)
}

/// `\oblock`: one ordinary argument, into a verbatim block.
fn oblock(policy: ExpandPolicy) -> Definition {
    Definition::macro_("oblock", vec![ArgDef::mandatory("x")])
        .text("\\begin{verbatimcode}#1\\end{verbatimcode}")
        .expand(policy)
}

/// The standard package with the four definitions under `policy`.
fn registry(policy: ExpandPolicy) -> Arc<FlmRegistry> {
    let definitions = vec![code(policy), c(policy), ocode(policy), oblock(policy)];
    common::stdcore::registry(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)),
    )
}

fn run(registry: &Arc<FlmRegistry>, workflow: &str, document: &str) -> Run {
    run_workflow(registry, workflow, &[document], &Overrides::none())
}

/// The text output of `document`.
fn text_of(registry: &Arc<FlmRegistry>, document: &str) -> (String, Vec<(String, Severity)>) {
    let run = run(registry, text::WORKFLOW, document);
    (common::lowering::text(&run), reports(&run))
}

/// The HTML fragment of `document`.
fn html_of(registry: &Arc<FlmRegistry>, document: &str) -> (String, Vec<(String, Severity)>) {
    let run = run(registry, html::WORKFLOW, document);
    (common::html::fragment(&run), reports(&run))
}

/// The recomposed LaTeX of `document`.
fn latex_of(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run(registry, latex_recomposed::WORKFLOW, document)
}

/// An inline verbatim `<code>` element of kind `code`, holding `content`.
fn html_code(content: &str) -> String {
    format!(
        "<code class=\"flm-pres-verbatim\" data-flm-kind=\"code\" data-flm-display=\"inline\">\
         {content}</code>"
    )
}

// --- a verbatim argument stands for its characters --------------------------------------------

#[test]
fn a_verbatim_argument_into_verbatim_content_shows_its_characters_on_every_output() {
    // The characters between the delimiters are the argument's content; the
    // template puts them inside `\verbcode`, which shows them as code. Before,
    // the argument substituted as its verbatim group, which showed its braces
    // on text and HTML and failed the recomposed LaTeX.
    let document = "See \\code{x%y}.";
    for policy in POLICIES {
        let registry = registry(policy);
        assert_eq!(text_of(&registry, document), (String::from("See x%y.\n"), Vec::new()));
        let (html, found) = html_of(&registry, document);
        assert_eq!(found, Vec::new());
        assert!(html.contains(&format!("See {}.", html_code("x%y"))), "{html}");
        let latex = latex_of(&registry, document);
        assert_eq!(identifiers(&latex), Vec::<String>::new(), "{}", latex.diagnostics().render_all());
        // Kept or not, the use site is expanded: verbatim content is read as
        // characters, which a call's parameter does not have before the call.
        let written = body(&latex);
        assert!(
            written.contains("See \\EscVerb[formatcom=\\flmFmtVRB{verbcode}]{x\\%y}."),
            "{written}"
        );
        assert!(style_if_any(&latex).is_none_or(|(style, _)| !style.contains("\\flmUD")));
        match policy {
            ExpandPolicy::PerPlan => assert_golden("userdefs-verbatim-into-verbatim.tex", &latex),
            ExpandPolicy::Always => {
                assert_golden("userdefs-verbatim-into-verbatim-expanded.tex", &latex)
            }
        }
    }
}

#[test]
fn a_verbatim_argument_into_text_is_text() {
    // The same characters, into text: they are the words of the sentence, and
    // the recomposed LaTeX escapes them as text.
    let document = "See \\c{x%y}.";
    for policy in POLICIES {
        let registry = registry(policy);
        assert_eq!(text_of(&registry, document), (String::from("See see x%y.\n"), Vec::new()));
        let (html, found) = html_of(&registry, document);
        assert_eq!(found, Vec::new());
        assert!(html.contains("See see x%y."), "{html}");
        assert!(!html.contains("<code"), "{html}");
        let latex = latex_of(&registry, document);
        assert_eq!(identifiers(&latex), Vec::<String>::new(), "{}", latex.diagnostics().render_all());
        let written = body(&latex);
        match policy {
            ExpandPolicy::PerPlan => {
                assert!(written.contains("See \\flmUDc{x\\%y}."), "{written}");
                let (style, _) = style(&latex);
                assert!(style.contains("\\providecommand\\flmUDc[1]{see #1}"), "{style}");
            }
            ExpandPolicy::Always => assert!(written.contains("See see x\\%y."), "{written}"),
        }
        // The characters were read with the verbatim rules off and now stand
        // in text; the tree contract has nothing to say about that.
        let check = run(&registry, CHECK_WORKFLOW, document);
        assert!(!check.diagnostics().any_recorded(), "{}", check.diagnostics().render_all());
    }
}

// --- brace groups --------------------------------------------------------------------------------

#[test]
fn braces_are_characters_of_a_verbatim_argument_and_syntax_of_an_ordinary_one() {
    // Inside a verbatim argument, `{` and `}` are characters like any other:
    // `a{b}c` is five characters, and the code shows all five. In an ordinary
    // argument they open and close a group — syntax, not content — so the
    // verbatim element reads the group's characters with the braces left out,
    // as every reader of content as characters does, and shows `abc`. Neither
    // is reported.
    for policy in POLICIES {
        let registry = registry(policy);
        let (text, found) = text_of(&registry, "\\code{a{b}c} \\ocode{a{b}c}");
        assert_eq!((text.as_str(), found), ("a{b}c abc\n", Vec::new()));
        let (html, found) = html_of(&registry, "\\code{a{b}c} \\ocode{a{b}c}");
        assert_eq!(found, Vec::new());
        assert!(html.contains(&format!("{} {}", html_code("a{b}c"), html_code("abc"))), "{html}");
        let latex = latex_of(&registry, "\\code{a{b}c} \\ocode{a{b}c}");
        assert_eq!(identifiers(&latex), Vec::<String>::new(), "{}", latex.diagnostics().render_all());
        // Kept or not, both use sites are expanded, the verbatim content
        // being read as characters.
        let written = body(&latex);
        assert!(
            written.contains(
                "\\EscVerb[formatcom=\\flmFmtVRB{verbcode}]{a\\{b\\}c} \
                 \\EscVerb[formatcom=\\flmFmtVRB{verbcode}]{abc}"
            ),
            "{written}"
        );
    }
}

// --- content that is not characters -----------------------------------------------------------

/// The four uses of `\ocode` whose argument is not characters, and what the
/// code shows for each: the content's source spelling.
const NOT_CHARACTERS_CASES: [(&str, &str); 4] = [
    ("\\ocode{\\emph{x}}", "\\emph{x}"),
    ("\\ocode{a\\%b}", "a\\%b"),
    ("\\ocode{\\textit{y}}", "\\textit{y}"),
    ("\\ocode{\\(a^2\\)}", "\\(a^2\\)"),
];

#[test]
fn content_that_is_not_characters_is_the_verbatim_features_error_on_every_output() {
    // A command, a literal written as a command, mathematics: none has
    // characters to give. The verbatim element reports it once and shows its
    // content written as its source spelling — the same recovery on every
    // output — and every output still reaches its native set.
    let error = vec![(String::from(NOT_CHARACTERS), Severity::Error)];
    for policy in POLICIES {
        let registry = registry(policy);
        for (document, shown) in NOT_CHARACTERS_CASES {
            let (text, found) = text_of(&registry, document);
            assert_eq!((text, &found), (format!("{shown}\n"), &error), "{document} ({policy:?})");
            let (html, found) = html_of(&registry, document);
            assert_eq!(found, error, "{document} ({policy:?})");
            assert!(html.contains(&html_code(shown)), "{document} ({policy:?}): {html}");
            let latex = latex_of(&registry, document);
            assert_eq!(reports(&latex), error, "{document} ({policy:?}): {}", latex.diagnostics().render_all());
        }
    }
}

#[test]
fn the_recomposed_latex_shows_the_source_spelling_as_code() {
    // Kept or expanded, the recomposed LaTeX writes the content's source
    // spelling as the verbatim content, in the encoding `\EscVerb` reads: a
    // kept definition whose template names its argument inside verbatim
    // content has its use sites expanded, so that the verbatim element reads
    // the value and reports it exactly as the expanded document does.
    let document = "\\ocode{\\emph{x}} \\ocode{a\\%b} \\ocode{\\textit{y}} \\ocode{\\(a^2\\)}\n\n\
                    \\oblock{\\emph{x}}\n";
    for policy in POLICIES {
        let latex = latex_of(&registry(policy), document);
        assert_eq!(
            identifiers(&latex),
            vec![String::from(NOT_CHARACTERS); 5],
            "{}",
            latex.diagnostics().render_all()
        );
        let written = body(&latex);
        assert!(
            written.contains(
                "\\EscVerb[formatcom=\\flmFmtVRB{verbcode}]{\\\\emph\\{x\\}} \
                 \\EscVerb[formatcom=\\flmFmtVRB{verbcode}]{a\\\\\\%b} \
                 \\EscVerb[formatcom=\\flmFmtVRB{verbcode}]{\\\\textit\\{y\\}} \
                 \\EscVerb[formatcom=\\flmFmtVRB{verbcode}]{\\\\\\(a\\^2\\\\\\)}"
            ),
            "{written}"
        );
        assert!(
            written.contains("\\begin{flmVerbatimCode}[]\n\\emph{x}\n\\end{flmVerbatimCode}"),
            "{written}"
        );
        assert!(!written.contains("\\flmUDocode"), "{written}");
        if policy == ExpandPolicy::PerPlan {
            assert_golden("userdefs-verbatim-not-characters.tex", &latex);
        }
    }
}

#[test]
fn a_verbatim_block_shows_the_source_spelling_too() {
    let error = vec![(String::from(NOT_CHARACTERS), Severity::Error)];
    for policy in POLICIES {
        let registry = registry(policy);
        let (text, found) = text_of(&registry, "\\oblock{\\emph{x}}");
        assert_eq!((text.as_str(), found), ("\\emph{x}\n", error.clone()));
        let (html, found) = html_of(&registry, "\\oblock{\\emph{x}}");
        assert_eq!(found, error);
        assert!(html.contains("<code>\\emph{x}</code>"), "{html}");
    }
}

// --- an extension's verbatim argument, lowered through the rules --------------------------------

/// A toy feature declaring `\\toy`, whose one verbatim argument its rule lowers
/// through the rules — the argument's verbatim group, so that the default rule
/// of a bare verbatim group (`flm.verbatim.lower.default`) stages it — rather
/// than reading it itself as the `verbatim` feature's elements do.
fn toy_feature() -> Arc<dyn flm_core::core::registry::FlmFeature> {
    common::pipeline::feature("x", |b| {
        let owner = b.feature().clone();
        b.element(ElementDef::new(
            owner,
            ["toy"],
            SyntaxDef::Macro { args: vec![verbatim()] },
            common::element("x.toy"),
        ))?;
        b.lowering_rule(common::lowering::rule(
            "x.toy.lower.default",
            "x.toy",
            LoweringDecl::new(),
            DefaultMark::Everywhere,
            |node, cx, lw| {
                let group = node.argument_nodes(0).expect("the verbatim argument");
                Ok(Lowering::Emit(lw.lower_nodes(cx, group)?))
            },
        ))?;
        Ok(())
    })
}

/// The standard package, the toy feature, and `\\otoy`: one ordinary argument
/// into `\\toy`'s verbatim argument, under `policy`.
fn toy_registry(policy: ExpandPolicy) -> Arc<FlmRegistry> {
    let otoy = Definition::macro_("otoy", vec![ArgDef::mandatory("x")]).text("\\toy{#1}").expand(policy);
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(vec![otoy])),
    ));
    builder.use_package(flm_core::math_passthrough::package());
    builder.feature(toy_feature());
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

#[test]
fn a_verbatim_group_lowered_through_the_rules_reads_its_content_as_the_verbatim_feature_does() {
    // Characters pass as they are; a command substituted into the group is
    // the same Error, reported at the owning command, with the same recovery
    // — the content's source spelling — on every output, and every output
    // still reaches its native set.
    let error = vec![(String::from(NOT_CHARACTERS), Severity::Error)];
    let verbatim = |content: &str| {
        format!(
            "<code class=\"flm-pres-verbatim\" data-flm-kind=\"text\" \
             data-flm-display=\"inline\">{content}</code>"
        )
    };
    for policy in POLICIES {
        let registry = toy_registry(policy);
        assert_eq!(text_of(&registry, "\\toy{a%b}"), (String::from("a%b\n"), Vec::new()));
        let (text, found) = text_of(&registry, "\\otoy{\\emph{x}}");
        assert_eq!((text.as_str(), &found), ("\\emph{x}\n", &error), "{policy:?}");
        let (html, found) = html_of(&registry, "\\otoy{\\emph{x}}");
        assert_eq!(found, error, "{policy:?}");
        assert!(html.contains(&verbatim("\\emph{x}")), "{policy:?}: {html}");
        let latex = latex_of(&registry, "\\otoy{\\emph{x}}");
        assert_eq!(reports(&latex), error, "{policy:?}: {}", latex.diagnostics().render_all());
        let written = body(&latex);
        assert!(written.contains("\\\\emph\\{x\\}"), "{policy:?}: {written}");
        let message = latex.diagnostics().render_all();
        assert!(message.contains("the verbatim content of \\toy must be plain characters"), "{message}");
    }
}
