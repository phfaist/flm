//! The `href` feature (`flm_core::stdcore::features::href`): `\href{address}{text}`,
//! `\url{address}` and `\email{address}`, links to an address outside the
//! document.
//!
//! What is pinned here: the link each command stages — kind `url`, the address
//! as written (`mailto:` before an `\email`'s), the display, and whether the
//! content is the address's own display; an address read as raw text, `%`,
//! `#`, `~`, `_` and nested braces included; the text output under the four
//! combinations of the text backend's two link options; the HTML output
//! (goldens, which `tests/html_validity.rs` parses with no excuse), with a
//! refused scheme reported by the HTML writer and the text kept; the recomposed
//! LaTeX (goldens, which `tests/latex_compile.rs` compiles under both engines,
//! with and without the author's own `hyperref`); a user definition's
//! placeholder in an address, expanded and kept; an address that is not
//! characters; the three commands on every backend without the references
//! and numbering features; hard inputs; and a heading identified from its
//! title's words with the address left out.

mod common;

use std::sync::Arc;

use flm_core::core::plan::{LoweringSelection, Overrides};
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::lang::mint::{ArgDef, ArgStdKind};
use flm_core::lang::rewrites::userdefs::{Definition, ExpandPolicy, NotRecomposable, UserDefinition};
use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::pres::{self, LinkPayload, payload_of};
use flm_core::stdcore::backends::latex_recomposed::LatexRecomposedRenderOptions;
use flm_core::stdcore::backends::text::TextRenderOptions;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::href::{self, HrefCommand, display};
use flm_core::stdcore::features::refs::encode_anchor;
use flm_core::stdcore::features::userdefs::{UserdefsConfig, instantiate_rule_name};
use flm_core::stdcore::StdcoreConfig;
use techy::error::Severity;

use common::lowering::lowered;
use common::stdcore::{identifiers, package_features_but, registry_over, reports, run_workflow};

/// The identifier of the feature's one condition.
const NOT_CHARACTERS: &str = "flm.href.address-not-characters";

// ---------------------------------------------------------------------------------------------
// registries and runs
// ---------------------------------------------------------------------------------------------

/// The standard package, with `definitions` as its user definitions.
fn registry_with(definitions: Vec<Definition>) -> Arc<FlmRegistry> {
    common::stdcore::registry(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)),
    )
}

/// The standard package.
fn std() -> Arc<FlmRegistry> {
    registry_with(Vec::new())
}

/// The run of `workflow` over `document` under `overrides`.
fn render(registry: &Arc<FlmRegistry>, workflow: &str, document: &str, overrides: &Overrides) -> Run {
    run_workflow(registry, workflow, &[document], overrides)
}

/// The text output of `document` under the text render options `options`;
/// the run must report nothing.
fn text_under(registry: &Arc<FlmRegistry>, document: &str, options: TextRenderOptions) -> String {
    let overrides =
        Overrides { render_options: OptionValues::new().with(options), ..Overrides::none() };
    let run = render(registry, text::WORKFLOW, document, &overrides);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    common::lowering::text(&run)
}

/// The text output of `document` under the default render options; the run
/// must report nothing.
fn text_output(registry: &Arc<FlmRegistry>, document: &str) -> String {
    text_under(registry, document, TextRenderOptions::default())
}

/// The HTML fragment of `document`; the run must report nothing.
fn html_output(registry: &Arc<FlmRegistry>, document: &str) -> String {
    let run = render(registry, html::WORKFLOW, document, &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    common::html::fragment(&run)
}

/// The recomposition of `document`.
fn recompose(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    render(registry, latex_recomposed::WORKFLOW, document, &Overrides::none())
}

/// The payloads of the links the text backend's lowering staged for
/// `document`, in document order.
fn links(registry: &Arc<FlmRegistry>, document: &str) -> Vec<(LinkPayload, String)> {
    let run = render(registry, text::WORKFLOW, document, &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let lowered = lowered(&run, TreeId::tree(0));
    lowered
        .tree
        .root()
        .descendants()
        .filter(|node| element_of(node).name == pres::LINK)
        .map(|node| {
            let payload = payload_of::<LinkPayload>(&node).expect("a link payload").clone();
            let content: String =
                node.descendants().filter_map(|inner| inner.chars().map(String::from)).collect();
            (payload, content)
        })
        .collect()
}

/// The payload of a link of this feature.
fn url_link(href: &str, shows_target: bool) -> LinkPayload {
    LinkPayload {
        href: href.into(),
        kind: href::LINK_KIND.into(),
        doc_role: None,
        label: None,
        shows_target,
    }
}

// ---------------------------------------------------------------------------------------------
// the element and the display (tests 1, 2, 3)
// ---------------------------------------------------------------------------------------------

#[test]
fn an_href_stages_one_link_to_its_address_around_its_text() {
    let found = links(&std(), "See \\href{https://example.com}{the \\emph{site}}.");
    assert_eq!(found, [(url_link("https://example.com", false), String::from("the site"))]);
}

#[test]
fn a_url_and_an_email_show_their_display_and_say_so() {
    let found = links(&std(), "\\url{https://example.com/} and \\email{a@b.c}.");
    assert_eq!(
        found,
        [
            (url_link("https://example.com/", true), String::from("example.com")),
            (url_link("mailto:a@b.c", true), String::from("a@b.c")),
        ]
    );
}

#[test]
fn the_display_is_the_pythons_pretty_url() {
    // The ruled example: every trailing `/`, `#` and `?` is left out.
    assert_eq!(display("https://example.com/?#"), "example.com");
    let found = links(&std(), "\\url{https://example.com/?#}");
    assert_eq!(found[0].1, "example.com");
    assert_eq!(found[0].0.href, "https://example.com/?#");
}

#[test]
fn an_address_is_raw_text_staged_unchanged() {
    // `%` is no comment, `#` no placeholder, `~` no space, `_` nothing
    // special, and nested braces are counted and kept.
    let address = "https://x.org/a%20b_c~d/{e{f}}#g";
    let found = links(&std(), &format!("\\url{{{address}}} \\href{{{address}}}{{t}}"));
    assert_eq!(found[0].0, url_link(address, true));
    assert_eq!(found[0].1, "x.org/a%20b_c~d/{e{f}}#g");
    assert_eq!(found[1].0, url_link(address, false));
}

#[test]
fn a_command_is_a_value_of_the_one_element_carrying_its_meaning() {
    let registry = std();
    let run = render(&registry, text::WORKFLOW, "\\href{a}{b}\\url{c}\\email{d}", &Overrides::none());
    let tree = common::wiring::frozen(&run);
    let commands: Vec<HrefCommand> = tree
        .root()
        .descendants()
        .filter(|node| element_of(node).name == href::LINK_ELEMENT)
        .map(|node| *element_of(&node).value_as::<HrefCommand>().expect("the command"))
        .collect();
    assert_eq!(commands, HrefCommand::ALL);
}

// ---------------------------------------------------------------------------------------------
// the text output (test 4)
// ---------------------------------------------------------------------------------------------

/// The text output of `document` under the two link options.
fn text_with(document: &str, link_addresses: bool, when_shown: bool) -> String {
    let options = TextRenderOptions {
        link_addresses,
        link_addresses_when_shown: when_shown,
        ..TextRenderOptions::default()
    };
    text_under(&std(), document, options)
}

#[test]
fn the_text_output_of_each_command_under_the_four_combinations_of_the_options() {
    let href = "\\href{https://example.com}{Click here}";
    let url = "\\url{https://example.com}";
    let email = "\\email{a@b.c}";
    // (link_addresses, link_addresses_when_shown) -> the three outputs.
    let expected = [
        ((true, false), ["Click here <https://example.com>", "example.com", "a@b.c"]),
        (
            (true, true),
            ["Click here <https://example.com>", "example.com <https://example.com>", "a@b.c <mailto:a@b.c>"],
        ),
        ((false, false), ["Click here", "example.com", "a@b.c"]),
        // The second option has no effect when the first is off.
        ((false, true), ["Click here", "example.com", "a@b.c"]),
    ];
    for ((addresses, when_shown), outputs) in expected {
        for (document, output) in [href, url, email].into_iter().zip(outputs) {
            assert_eq!(
                text_with(document, addresses, when_shown),
                format!("{output}\n"),
                "{document} under ({addresses}, {when_shown})"
            );
        }
    }
    // The defaults are the first combination.
    assert_eq!(text_output(&std(), href), "Click here <https://example.com>\n");
    assert_eq!(text_output(&std(), url), "example.com\n");
    assert_eq!(text_output(&std(), email), "a@b.c\n");
}

#[test]
fn a_heading_s_rule_is_as_wide_as_its_title_with_the_addresses_printed() {
    // The rule under a heading covers the address a link prints after its
    // text, and only where the options print one.
    let document = "\\section{See \\href{https://x.org}{the site}}";
    assert_eq!(text_with(document, true, false), "See the site <https://x.org>\n============================\n");
    assert_eq!(text_with(document, false, false), "See the site\n============\n");
}

#[test]
fn an_href_whose_text_matches_its_address_still_prints_the_address() {
    // The author wrote the text; nothing compares it with the address.
    assert_eq!(
        text_output(&std(), "\\href{https://example.com}{example.com}"),
        "example.com <https://example.com>\n"
    );
}

#[test]
fn a_reference_prints_no_address_under_either_option() {
    let document = "\\section{Intro}\\label{sec:intro}\n\nSee \\ref{sec:intro}.";
    for (addresses, when_shown) in [(true, false), (true, true), (false, false)] {
        assert!(text_with(document, addresses, when_shown).ends_with("See Intro.\n"));
    }
}

// ---------------------------------------------------------------------------------------------
// the HTML output (test 5)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_html_of_each_command() {
    let html = html_output(
        &std(),
        "\\href{https://example.com}{Click here}, \\url{https://example.com/a%20b?q=1&r=2#top} \
         and \\email{a@b.c}.",
    );
    common::html::assert_golden("href-links.html", &html);
    assert!(html.contains("href=\"mailto:a@b.c\""), "{html}");
    assert!(html.contains("data-flm-kind=\"url\""), "{html}");
}

#[test]
fn a_refused_scheme_is_the_html_writers_report_and_the_text_stays() {
    // The feature stages the address as written; the HTML writer's policy
    // refuses the scheme where it writes the page, and the link keeps its
    // text. This covers the writer through the feature; the policy is the
    // writer's.
    let run = render(&std(), html::WORKFLOW, "\\href{javascript:alert(1)}{click}", &Overrides::none());
    assert_eq!(
        reports(&run),
        [(String::from("flm.html.refused-address"), Severity::Error)],
        "{}",
        run.diagnostics().render_all()
    );
    let html = common::html::fragment(&run);
    common::html::assert_golden("href-refused-scheme.html", &html);
    assert!(html.contains(">click</a>"), "{html}");
    assert!(!html.contains("javascript"), "{html}");
}

// ---------------------------------------------------------------------------------------------
// the recomposed LaTeX (test 6)
// ---------------------------------------------------------------------------------------------

/// The document of the recomposed-LaTeX golden: each command, addresses
/// holding `%`, `#`, `~`, `_`, braces and a backslash, and a hook inside a
/// moving argument.
const LATEX_DOCUMENT: &str = "\\section{See \\url{https://example.com/docs/}}\n\n\
     Visit \\href{https://example.com}{the \\emph{example} site}, \\url{https://example.com/} \
     or write to \\email{someone@example.com}.\n\n\
     An address holding specials: \\url{https://x.org/a%20b_c~d/{e}?q=1&r=$2#top} and \
     \\href{https://x.org/~user/a%2Fb#sec_2}{a link}, \\href{https://x.org/back\\slash}{a backslash}.\n";

#[test]
fn the_recomposed_latex_writes_each_command_as_its_hook() {
    let run = recompose(&std(), LATEX_DOCUMENT);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let body = common::latex::output(&run);
    for expected in [
        "\\section{See \\flmUrl{https://example.com/docs/}}",
        "\\flmHref{https://example.com}{the \\emph{example} site}",
        "\\flmUrl{https://example.com/} or",
        "\\flmEmail{someone@example.com}",
        "\\flmUrl{https://x.org/a\\%20b_c~d/{e}?q=1&r=$2\\#top} and",
        "\\flmHref{https://x.org/~user/a\\%2Fb\\#sec_2}{a link}",
        "\\flmHref{https://x.org/back\\\\slash}{a backslash}",
    ] {
        assert!(body.contains(expected), "no {expected} in:\n{body}");
    }
    let (style, _) = common::latex::style(&run);
    assert!(style.contains("\\RequirePackage{hyperref}"), "{style}");
    assert!(style.contains("\\ProvideDocumentCommand\\flmUrl{m}"), "{style}");
    common::latex::assert_golden("href.tex", &run);
}

#[test]
fn a_document_that_loads_hyperref_itself_skips_the_request() {
    // `hyperref` named in `skip_packages`: the style file requests no
    // package for the hooks, and the document loads `hyperref` itself, with
    // options of its own, after the style file — which `tests/latex_compile.rs`
    // compiles under both engines.
    let options = LatexRecomposedRenderOptions {
        skip_packages: [String::from("hyperref")].into_iter().collect(),
        ..LatexRecomposedRenderOptions::default()
    };
    let overrides =
        Overrides { render_options: OptionValues::new().with(options), ..Overrides::none() };
    let run = render(&std(), latex_recomposed::WORKFLOW, LATEX_DOCUMENT, &overrides);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let (style, _) = common::latex::style(&run);
    assert!(!style.contains("\\RequirePackage{hyperref}"), "{style}");
    assert!(style.contains("\\ProvideDocumentCommand\\flmUrl{m}"), "{style}");
    common::latex::assert_golden("href-skip-hyperref.tex", &run);
}

/// The addresses whose display the recomposed LaTeX leaves to its hooks, each
/// with the hook it is written as: the scheme prefixes, the trailing run of
/// `/`, `#` and `?`, the characters `%`, `#`, `~` and `_`, a backslash, braces
/// and no scheme. `tests/latex_compile.rs` compiles the hooks' helper over the
/// same addresses and compares its result with the feature's display.
const DISPLAYED: [(&str, &str, &str); 8] = [
    ("\\url{https://example.com/}", "\\flmUrl{https://example.com/}", "example.com"),
    ("\\url{https://example.com/?#}", "\\flmUrl{https://example.com/?\\#}", "example.com"),
    ("\\email{a@b.c}", "\\flmEmail{a@b.c}", "a@b.c"),
    ("\\email{a@b.c/}", "\\flmEmail{a@b.c/}", "a@b.c"),
    (
        "\\url{http://x.org/100%25/a_b~c#top}",
        "\\flmUrl{http://x.org/100\\%25/a_b~c\\#top}",
        "x.org/100%25/a_b~c#top",
    ),
    ("\\url{example.org/a/}", "\\flmUrl{example.org/a/}", "example.org/a"),
    ("\\url{mailto:x@y.z?#/}", "\\flmUrl{mailto:x@y.z?\\#/}", "x@y.z"),
    ("\\url{https://x.org/back\\slash/{e}/}", "\\flmUrl{https://x.org/back\\\\slash/{e}/}", "x.org/back\\slash/{e}"),
];

#[test]
fn the_recomposed_latex_writes_the_address_alone_and_leaves_the_display_to_the_hook() {
    // What a `\url` or an `\email` shows is presentation: the rule writes the
    // address, escaped, and the hook derives the display in LaTeX.
    for (written, hook, shown) in DISPLAYED {
        let run = recompose(&std(), written);
        assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
        // The paragraph is written alone, the `%%` after it ending the file.
        assert_eq!(common::latex::output(&run).trim_end().trim_end_matches('%'), hook, "{written}");
        // The display the other outputs show, which the hook imitates.
        let target = links(&std(), written)[0].0.href.clone();
        assert_eq!(display(&target), shown, "{written}");
    }
}

#[test]
fn a_kept_url_whose_address_is_a_placeholder_expands_its_use_sites_and_shows_the_display() {
    // `\url{#1}` and `\url{https://example.com/#1}` read their argument's
    // characters, so each use site is expanded and the hook receives the
    // whole address, from which it derives the display: `x.org` and
    // `example.com/a`, as on text and HTML. `tests/latex_compile.rs` compiles
    // the golden and reads what each hook shows.
    let registry = registry_with(vec![
        Definition::macro_("u", vec![ArgDef::mandatory("to")]).text("\\url{#1}"),
        Definition::macro_("ex", vec![ArgDef::mandatory("path")]).text("\\url{https://example.com/#1}"),
    ]);
    let document = "\\u{https://x.org/} \\ex{a/}";
    assert_eq!(text_output(&registry, document), "x.org example.com/a\n");
    let run = recompose(&registry, document);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let body = common::latex::output(&run);
    assert!(body.starts_with("\\flmUrl{https://x.org/} \\flmUrl{https://example.com/a/}"), "{body}");
    assert!(!body.contains("\\flmUD"), "{body}");
    common::latex::assert_golden("href-kept-display.tex", &run);
}

// ---------------------------------------------------------------------------------------------
// a user definition's placeholder in an address (test 7)
// ---------------------------------------------------------------------------------------------

/// `\doi{…}`, `\href{https://doi.org/#1}{#1}`, its argument declared `shape`.
fn doi(shape: ArgStdKind, expand: ExpandPolicy) -> Definition {
    Definition::macro_("doi", vec![ArgDef::named("id", shape)])
        .text("\\href{https://doi.org/#1}{#1}")
        .expand(expand)
}

#[test]
fn a_placeholder_in_an_address_takes_the_use_sites_characters_on_text_and_html() {
    let registry = registry_with(vec![doi(ArgStdKind::Mandatory, ExpandPolicy::PerPlan)]);
    let document = "See \\doi{10.1000/xyz-1}.";
    let written_out = "See \\href{https://doi.org/10.1000/xyz-1}{10.1000/xyz-1}.";
    assert_eq!(text_output(&registry, document), "See 10.1000/xyz-1 <https://doi.org/10.1000/xyz-1>.\n");
    assert_eq!(text_output(&registry, document), text_output(&std(), written_out));
    assert_eq!(html_output(&registry, document), html_output(&std(), written_out));
}

#[test]
fn a_use_site_passes_an_address_holding_a_percent_through_a_verbatim_argument() {
    // A verbatim argument is how an author passes `%` without its reading as a
    // comment at the use site.
    let shape = ArgStdKind::Verbatim { delimiters: Some(('{', '}')) };
    let registry = registry_with(vec![doi(shape, ExpandPolicy::PerPlan)]);
    let found = links(&registry, "\\doi{10.1000/a%20b#c}");
    assert_eq!(found[0].0.href, "https://doi.org/10.1000/a%20b#c");
    assert_eq!(
        text_output(&registry, "\\doi{10.1000/a%20b#c}"),
        "10.1000/a%20b#c <https://doi.org/10.1000/a%20b#c>\n"
    );
}

#[test]
fn a_command_passed_into_an_address_is_the_features_error_and_no_link() {
    let registry = registry_with(vec![doi(ArgStdKind::Mandatory, ExpandPolicy::PerPlan)]);
    for workflow in [text::WORKFLOW, html::WORKFLOW] {
        let run = render(&registry, workflow, "\\doi{\\emph{x}}", &Overrides::none());
        assert_eq!(
            reports(&run),
            [(String::from(NOT_CHARACTERS), Severity::Error)],
            "{}",
            run.diagnostics().render_all()
        );
    }
    let run = render(&registry, text::WORKFLOW, "\\doi{\\emph{x}}", &Overrides::none());
    // No link: the text alone, with no address after it.
    assert_eq!(common::lowering::text(&run), "x\n");
    let lowered = lowered(&run, TreeId::tree(0));
    assert!(!lowered.tree.root().descendants().any(|node| element_of(&node).name == pres::LINK));
}

#[test]
fn a_formula_passed_into_an_address_is_the_features_error_and_no_link() {
    // A formula has no characters, as a command has none: the address is not
    // read as `a`.
    let registry = registry_with(vec![doi(ArgStdKind::Mandatory, ExpandPolicy::PerPlan)]);
    for workflow in [text::WORKFLOW, html::WORKFLOW] {
        let run = render(&registry, workflow, "\\doi{\\(a\\)}", &Overrides::none());
        assert_eq!(
            reports(&run),
            [(String::from(NOT_CHARACTERS), Severity::Error)],
            "{}",
            run.diagnostics().render_all()
        );
    }
    let run = render(&registry, text::WORKFLOW, "\\doi{\\(a\\)}", &Overrides::none());
    let lowered = lowered(&run, TreeId::tree(0));
    assert!(!lowered.tree.root().descendants().any(|node| element_of(&node).name == pres::LINK));
}

#[test]
fn an_unreadable_address_of_a_url_shows_the_display_of_its_words() {
    let registry = registry_with(vec![
        Definition::macro_("site", vec![ArgDef::mandatory("path")])
            .text("\\url{https://x.org/#1/}"),
        Definition::macro_("mail", vec![ArgDef::mandatory("who")]).text("\\email{#1}"),
    ]);
    let run = render(&registry, text::WORKFLOW, "\\site{\\emph{a}} \\mail{\\textbf{b}@c.d}", &Overrides::none());
    assert_eq!(identifiers(&run), [NOT_CHARACTERS, NOT_CHARACTERS], "{}", run.diagnostics().render_all());
    assert_eq!(common::lowering::text(&run), "x.org/a b@c.d\n");
}

#[test]
fn a_placeholder_in_a_kept_address_expands_every_use_site() {
    // An address is read as characters, and a placeholder's characters are
    // known at a call and not before: so the definition is not written as a
    // LaTeX definition, every use site of it is expanded, and the address holds
    // the use site's characters, escaped as an address. Nothing is reported,
    // and the definition stays recomposable: it is its template's lowering that
    // expands the use sites.
    let registry = registry_with(vec![
        doi(ArgStdKind::Mandatory, ExpandPolicy::PerPlan),
        Definition::macro_("anchor", vec![ArgDef::mandatory("frag")])
            .text("\\url{https://x.org/100%/##sec-#1}"),
    ]);
    assert_eq!(not_recomposable(&registry), Vec::<(String, NotRecomposable)>::new());
    let run = recompose(&registry, "\\doi{10.1000/xyz} \\anchor{intro}");
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let (style, _) = common::latex::style(&run);
    assert!(!style.contains("\\flmUD"), "{style}");
    let body = common::latex::output(&run);
    assert!(
        body.contains(
            "\\flmHref{https://doi.org/10.1000/xyz}{10.1000/xyz} \
             \\flmUrl{https://x.org/100\\%/\\#sec-intro}"
        ),
        "{body}"
    );
    common::latex::assert_golden("href-userdefs-kept.tex", &run);
}

#[test]
fn a_command_passed_into_a_kept_address_is_the_features_error_as_on_the_expanded_paths() {
    // Every use site is expanded, so the feature's own rule reads each value:
    // a command and a special are reported and the text shown alone, as on
    // the expanded paths, and characters are linked.
    let registry = registry_with(vec![doi(ArgStdKind::Mandatory, ExpandPolicy::PerPlan)]);
    let run = recompose(&registry, "\\doi{\\emph{x}} \\doi{a~b} \\doi{10.1/y}");
    assert_eq!(
        reports(&run),
        [(String::from(NOT_CHARACTERS), Severity::Error), (String::from(NOT_CHARACTERS), Severity::Error)],
        "{}",
        run.diagnostics().render_all()
    );
    let body = common::latex::output(&run);
    assert!(body.starts_with("\\emph{x} a~b \\flmHref{https://doi.org/10.1/y}{10.1/y}"), "{body}");
    assert!(!body.contains("\\flmUDdoi"), "{body}");
}

#[test]
fn a_kept_definition_instantiated_at_lowering_writes_the_use_sites_characters() {
    let definition = doi(ArgStdKind::Mandatory, ExpandPolicy::PerPlan);
    let registry = registry_with(vec![definition.clone()]);
    let selection = LoweringSelection::new().prefer(instantiate_rule_name(&definition).as_str());
    let overrides = Overrides {
        lowering: [(latex_recomposed::BACKEND.into(), selection)].into_iter().collect(),
        ..Overrides::none()
    };
    let run = render(&registry, latex_recomposed::WORKFLOW, "\\doi{10.1000/a#b}", &overrides);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let body = common::latex::output(&run);
    assert!(body.contains("\\flmHref{https://doi.org/10.1000/a\\#b}{10.1000/a\\#b}"), "{body}");
}

// ---------------------------------------------------------------------------------------------
// without the references and numbering features
// ---------------------------------------------------------------------------------------------

#[test]
fn the_three_commands_render_on_every_backend_without_the_references_and_numbering_features() {
    // The feature reads nothing of either: its links are addresses, not
    // labels, and it numbers nothing.
    let features: Vec<_> = package_features_but(StdcoreConfig::new(), "refs")
        .into_iter()
        .filter(|feature| feature.info().name.as_str() != "numbering")
        .collect();
    let registry = registry_over(features);
    let document = "Visit \\href{https://x.org}{the site}, \\url{https://x.org/a/} or \\email{a@b.c}.";
    let run = |workflow: &str| {
        let run = render(&registry, workflow, document, &Overrides::none());
        assert_eq!(identifiers(&run), Vec::<String>::new(), "{workflow}: {}", run.diagnostics().render_all());
        run
    };
    assert_eq!(
        common::lowering::text(&run(text::WORKFLOW)),
        "Visit the site <https://x.org>, x.org/a or a@b.c.\n"
    );
    let html = common::html::fragment(&run(html::WORKFLOW));
    for expected in ["href=\"https://x.org\"", "href=\"https://x.org/a/\"", "href=\"mailto:a@b.c\""] {
        assert!(html.contains(expected), "no {expected} in {html}");
    }
    let latex = common::latex::output(&run(latex_recomposed::WORKFLOW));
    assert!(
        latex.contains(
            "Visit \\flmHref{https://x.org}{the site}, \\flmUrl{https://x.org/a/} or \\flmEmail{a@b.c}."
        ),
        "{latex}"
    );
}

// ---------------------------------------------------------------------------------------------
// hard inputs (test 16, the hyperlink half)
// ---------------------------------------------------------------------------------------------

#[test]
fn hard_addresses_neither_panic_nor_loop_on_any_output() {
    let hashes = "#".repeat(100);
    let documents = [
        String::from("\\url{}"),
        String::from("\\href{}{}"),
        String::from("\\email{}"),
        String::from("\\url{   }"),
        String::from("\\href{ \n }{x}"),
        format!("\\url{{{hashes}}}"),
        format!("\\href{{{hashes}}}{{t}}"),
        String::from("\\url{https://}"),
        String::from("\\url{a{b"),
        String::from("\\href{a}"),
        String::from("\\url"),
    ];
    let registry = std();
    for document in &documents {
        for workflow in [text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW] {
            // Whatever is reported, a run completes.
            let _ = render(&registry, workflow, document, &Overrides::none());
        }
    }
    // An empty display shows nothing, and a `\url`'s address is not printed
    // after its own display; the address of an `\href` with no text is.
    assert_eq!(text_output(&registry, "A \\url{} B"), "A B\n");
    assert_eq!(text_output(&registry, &format!("A \\url{{{hashes}}} B")), "A B\n");
    assert_eq!(text_output(&registry, "\\href{}{}"), "<>\n");
    // An address that begins with `#` is a place in the page, whose address
    // the text output never prints.
    assert_eq!(text_output(&registry, &format!("\\href{{{hashes}}}{{t}}")), "t\n");
}

// ---------------------------------------------------------------------------------------------
// a heading identified from its words (test 18, the hyperlink half)
// ---------------------------------------------------------------------------------------------

/// The identifiers of the page's first-level headings, in order.
fn heading_ids(html: &str) -> Vec<String> {
    html.match_indices("<h1 ")
        .filter_map(|(at, _)| {
            let tag = &html[at..at + html[at..].find('>').expect("a closed tag")];
            tag.split_once(" id=\"").map(|(_, rest)| rest[..rest.find('"').expect("a quote")].to_string())
        })
        .collect()
}

#[test]
fn a_heading_is_identified_from_its_words_without_an_address() {
    // The address is no word of the title; `\href`'s text is. A `\url`'s
    // display is derived at lowering and is no node of the tree, so the second
    // title's words are "See " alone. The first title's words, "Visit the
    // site", hold single spaces and take the encoding's plain path; the second
    // and third hold a trailing or a doubled space, which sends them down the
    // tagged path — a slug, `~` and a tag of the text. Each identifier is the
    // encoding of exactly those words.
    let html = html_output(
        &std(),
        "\\section{Visit \\href{https://x.org}{the site}}\n\n\
         \\section{See \\url{https://x.org}}\n\n\
         \\section{Mail \\email{a@b.c} now}\n\nText.",
    );
    let title = |words: &str| format!("sec--{}", encode_anchor(words));
    assert_eq!(
        heading_ids(&html),
        [title("Visit the site"), title("See "), title("Mail  now")],
        "{html}"
    );
    assert_eq!(title("Visit the site"), "sec--Visit_the_site");
    assert!(title("See ").starts_with("sec--See~"));
}

/// The reason each per-plan definition of `registry` is not recomposable.
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
fn a_verbatim_argument_into_a_kept_address_expands_each_use_site() {
    // The address reads its characters, so every use site of `\doi` and of
    // `\link` is expanded and the address holds the use site's characters —
    // a backslash, `~`, `^` among them — escaped as an address. Nothing is
    // reported. `\code`, whose verbatim argument goes into text, stays a LaTeX
    // definition, its call holding the characters as text.
    let shape = ArgStdKind::Verbatim { delimiters: Some(('{', '}')) };
    let registry = registry_with(vec![
        doi(shape.clone(), ExpandPolicy::PerPlan),
        Definition::macro_("link", vec![ArgDef::named("to", shape.clone())]).text("\\url{#1}"),
        Definition::macro_("code", vec![ArgDef::named("c", shape)]).text("\\textbf{#1}"),
    ]);
    assert_eq!(not_recomposable(&registry), Vec::<(String, NotRecomposable)>::new());
    let run = recompose(
        &registry,
        "\\doi{10.1000/a%20b#c} \\doi{a^b} \\doi{10.1000/x~y\\z^w} \\link{https://x.org/a%20b#c} \\code{x%y}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let body = common::latex::output(&run);
    assert!(body.contains("\\flmHref{https://doi.org/10.1000/a\\%20b\\#c}{"), "{body}");
    assert!(body.contains("\\flmHref{https://doi.org/a^b}{"), "{body}");
    assert!(body.contains("\\flmHref{https://doi.org/10.1000/x~y\\\\z^w}{"), "{body}");
    assert!(body.contains("\\flmUrl{https://x.org/a\\%20b\\#c} "), "{body}");
    assert!(!body.contains("\\flmUDdoi") && !body.contains("\\flmUDlink"), "{body}");
    assert!(body.contains("\\flmUDcode{"), "{body}");
    common::latex::assert_golden("href-userdefs-verbatim.tex", &run);
}
