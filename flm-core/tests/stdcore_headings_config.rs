//! Heading kinds and heading commands
//! (`flm_core::stdcore::features::headings::{HeadingDecl, HeadingCommand}`):
//! a kind of the host's own and a second command writing a shipped kind; the
//! optional key-value argument of every heading command — `numbered`, `toc`,
//! `alt` written without a value, the reserved `pdfbookmark` — and the star as `numbered=false`
//! alone; what each lowering rule reports about that argument; the LaTeX
//! command and the HTML tag a kind is written with; and the configuration's
//! place in the registry digest and its check.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::model::aspects::Referenceable;
use flm_core::model::element::element_of;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::headings::latex::HeadingsLatexOptions;
use flm_core::stdcore::features::headings::{
    HEADING, HeadingCommand, HeadingDecl, HeadingElement, HeadingNumbering, HeadingsConfig,
};
use flm_core::stdcore::features::numbering::{NumberTemplate, NumberingOptions, Within};
use flm_core::stdcore::{StdcoreConfig, package};
use techy::error::Severity;

use common::lowering::text as printed;
use common::stdcore::{plan, registry, reports};

// --- helpers -------------------------------------------------------------------------------

/// The shipped kinds, an `appendix` of level 1 numbered in capital letters,
/// and a second command `quietsection` writing a `section` left out of the
/// table of contents by default.
fn custom_config() -> HeadingsConfig {
    let mut kinds = HeadingsConfig::new().headings;
    kinds.push(HeadingDecl::new("appendix", 1).number_format(NumberTemplate::formats("#{Alph}")));
    HeadingsConfig::new()
        .headings(kinds)
        .command(HeadingCommand::new("quietsection", "section").default_value("toc", "false"))
}

/// The standard package under `headings`.
fn registry_with(headings: HeadingsConfig) -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new().headings(headings))
}

/// Runs `text` under the workflow `workflow` of `registry`, with `numbering`
/// bound on the document and `render` as the render options.
fn run(
    registry: &Arc<FlmRegistry>,
    workflow: &str,
    text: &str,
    numbering: NumberingOptions,
    render: OptionValues,
) -> Run {
    let doc =
        common::pipeline::doc_with_options(registry, &[text], OptionValues::new().with(numbering));
    let overrides = Overrides { render_options: render, ..Overrides::none() };
    common::pipeline::run_with(
        &doc,
        &plan(registry, workflow, &overrides),
        Externals::none(),
        &RunOptions::sequential(),
    )
}

/// Fails, printing the whole report, when the run reported anything.
fn assert_clean(run: &Run) {
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
}

/// The HTML of `text` under `registry`, with `numbering` bound on the
/// document; a run that reported anything is a test failure.
fn html_of(registry: &Arc<FlmRegistry>, text: &str, numbering: NumberingOptions) -> String {
    let run = run(registry, html::WORKFLOW, text, numbering, OptionValues::new());
    assert_clean(&run);
    common::html::fragment(&run)
}

/// The recomposed LaTeX of `text` under `registry` and `render`; a run that
/// reported anything is a test failure.
fn latex_of(registry: &Arc<FlmRegistry>, text: &str, render: OptionValues) -> String {
    let run = run(registry, latex_recomposed::WORKFLOW, text, NumberingOptions::new(), render);
    assert_clean(&run);
    common::latex::output(&run)
}

/// `headings` with the kinds of level 1 numbered: the sections, and the
/// appendices of [`custom_config`].
fn level_one_numbered(headings: HeadingsConfig) -> HeadingsConfig {
    headings.numbering(HeadingNumbering::UpToDepth(1))
}

// --- a kind of the host's own, and a second command ---------------------------------------

#[test]
fn a_host_kind_and_a_second_command_are_numbered_and_carry_their_attributes() {
    let registry = registry_with(level_one_numbered(custom_config()));
    let html = html_of(
        &registry,
        "\\section{Intro}\n\n\\quietsection{Quiet}\n\n\\appendix{Extra}\\label{sec:extra}\n\n\
         \\appendix{More}\\label{sec:more} See~\\ref{sec:more}.\n",
        NumberingOptions::new(),
    );
    // The second command writes a section: it counts in the section counter
    // and carries the kind's name, and its default leaves it out of the
    // contents.
    assert!(
        html.contains(
            "<h1 class=\"flm-pres-block--heading\" id=\"sec--Intro\" data-flm-heading=\"section\" \
             data-flm-level=\"1\" data-flm-toc=\"true\"><span class=\"flm-pres-span \
             flm-heading-number\">1</span>. Intro</h1>"
        ),
        "{html}"
    );
    assert!(
        html.contains(
            "id=\"sec--Quiet\" data-flm-heading=\"section\" data-flm-level=\"1\" \
             data-flm-toc=\"false\"><span class=\"flm-pres-span flm-heading-number\">2</span>. \
             Quiet</h1>"
        ),
        "{html}"
    );
    // The appendix counts in its own counter, in capital letters.
    assert!(
        html.contains(
            "id=\"sec-extra\" data-flm-heading=\"appendix\" data-flm-level=\"1\" \
             data-flm-toc=\"true\"><span class=\"flm-pres-span flm-heading-number\">A</span>. \
             Extra</h1>"
        ),
        "{html}"
    );
    assert!(html.contains("flm-heading-number\">B</span>. More</h1>"), "{html}");
    // A reference to the appendix shows the section sign and its letter.
    assert!(html.contains(">§&#x202F;B</a>"), "{html}");
}

#[test]
fn a_kind_with_no_counter_is_never_numbered() {
    let mut kinds = HeadingsConfig::new().headings;
    kinds.push(HeadingDecl::new("aside", 2).no_counter());
    let registry = registry_with(
        HeadingsConfig::new().headings(kinds).numbering(HeadingNumbering::AllNumbered),
    );
    let run = run(
        &registry,
        text::WORKFLOW,
        "\\section{A}\\aside{B}\n",
        NumberingOptions::new(),
        OptionValues::new(),
    );
    assert_clean(&run);
    assert_eq!(printed(&run), "1. A\n====\n\nB\n-\n");
}

// --- the star and the key-values ----------------------------------------------------------

/// The reference points of the one heading of `text` under `registry`.
fn points(registry: &Arc<FlmRegistry>, text: &str) -> Vec<flm_core::model::aspects::RefPoint> {
    let fragment = common::parse(registry.lang(), text);
    let heading = common::callables(&fragment)
        .into_iter()
        .find(|node| element_of(node).name == HEADING)
        .expect("a heading");
    element_of(&heading).aspect::<dyn Referenceable>().expect("referenceable").points(&heading)
}

#[test]
fn numbered_false_is_the_star_and_neither_leaves_the_contents() {
    let registry = registry_with(level_one_numbered(HeadingsConfig::new()));
    let starred = points(&registry, "\\section*{T}");
    let unnumbered = points(&registry, "\\section[numbered=false]{T}");
    assert_eq!(starred, unnumbered);
    assert!(starred[0].number.is_none());
    assert!(points(&registry, "\\section{T}")[0].number.is_some());
    assert!(points(&registry, "\\section*[numbered]{T}")[0].number.is_some());
    let numbering = NumberingOptions::new();
    let starred = html_of(&registry, "\\section{A}\\section*{T}\n", numbering.clone());
    let unnumbered = html_of(&registry, "\\section{A}\\section[numbered=false]{T}\n", numbering);
    assert_eq!(starred, unnumbered);
    assert!(starred.contains("data-flm-level=\"1\" data-flm-toc=\"true\">T</h1>"), "{starred}");
}

#[test]
fn toc_false_leaves_the_contents_and_keeps_the_number() {
    let registry = registry_with(level_one_numbered(HeadingsConfig::new()));
    let numbering = NumberingOptions::new;
    let html = html_of(&registry, "\\section[toc=false]{T}\n", numbering());
    assert!(html.contains("data-flm-toc=\"false\"><span"), "{html}");
    assert!(html.contains("flm-heading-number\">1</span>. T"), "{html}");
    let html = html_of(&registry, "\\section*[toc=false]{T}\n", numbering());
    assert!(html.contains("data-flm-toc=\"false\">T</h1>"), "{html}");
    // A later key replaces an earlier one, and the bracket replaces the
    // command's defaults.
    let html = html_of(&registry, "\\section[toc=false, toc=true]{T}\n", numbering());
    assert!(html.contains("data-flm-toc=\"true\""), "{html}");
    let quiet = registry_with(level_one_numbered(custom_config()));
    let html = html_of(&quiet, "\\quietsection[toc=true]{T}\n", numbering());
    assert!(html.contains("data-flm-toc=\"true\""), "{html}");
}

#[test]
fn a_kind_that_skips_the_contents_ignores_toc_true_silently() {
    let mut kinds = HeadingsConfig::new().headings;
    kinds.push(HeadingDecl::new("aside", 2).no_counter().skip_toc(true));
    let registry = registry_with(HeadingsConfig::new().headings(kinds));
    // `html_of` fails on any report, so nothing is reported for the key.
    let html = html_of(&registry, "\\aside[toc=true]{T}\n", NumberingOptions::new());
    assert!(html.contains("data-flm-heading=\"aside\" data-flm-level=\"2\" data-flm-toc=\"false\""));
}

// --- what the lowering rules report --------------------------------------------------------

#[test]
fn each_key_the_heading_cannot_take_is_reported_once_per_render() {
    let registry = registry_with(HeadingsConfig::new());
    let text = "\\section[alt, pdfbookmark=B, color=red, numbered=maybe]{T}\n";
    let expected = [
        (String::from("flm.heading.missing-value"), Severity::Error),
        (String::from("flm.heading.key-not-implemented"), Severity::Warning),
        (String::from("flm.heading.unknown-key"), Severity::Error),
        (String::from("flm.heading.bad-value"), Severity::Error),
    ];
    for workflow in [html::WORKFLOW, text::WORKFLOW, latex_recomposed::WORKFLOW] {
        let run = run(&registry, workflow, text, NumberingOptions::new(), OptionValues::new());
        assert_eq!(reports(&run), expected, "{workflow}: {}", run.diagnostics().render_all());
    }
    // The heading is still written, numbered as the keys it could take say.
    let run = run(
        &registry_with(level_one_numbered(HeadingsConfig::new())),
        text::WORKFLOW,
        text,
        NumberingOptions::new(),
        OptionValues::new(),
    );
    assert_eq!(printed(&run), "1. T\n====\n");
}

#[test]
fn a_bracket_whose_keys_are_not_text_is_reported_and_ignored() {
    let registry = registry_with(HeadingsConfig::new());
    let run = run(
        &registry,
        text::WORKFLOW,
        "\\section[\\emph{x}=1]{T}\n",
        NumberingOptions::new(),
        OptionValues::new(),
    );
    assert_eq!(reports(&run), [(String::from("flm.heading.unreadable-options"), Severity::Error)]);
    assert_eq!(printed(&run), "T\n=\n");
}

// --- the backends' mappings ----------------------------------------------------------------

#[test]
fn the_recomposed_latex_writes_the_command_the_kind_maps_to() {
    let registry = registry_with(custom_config());
    // By default a kind is written as the command of its own name, and a
    // second command of a kind is written as the kind's command.
    let latex = latex_of(&registry, "\\quietsection{Q}\n", OptionValues::new());
    assert!(latex.contains("\\section{Q}"), "{latex}");
    assert!(!latex.contains("quietsection"), "{latex}");
    // The star is written for a heading that is not numbered, however it
    // said so; `toc` is the hook's key, and the bracket is not written back.
    let latex =
        latex_of(&registry, "\\section[numbered=false,toc=false]{U}\n", OptionValues::new());
    assert!(latex.contains("\\flmHeading[toc=false]\n\\section*{U}"), "{latex}");
    assert!(!latex.contains("numbered"), "{latex}");
    let latex = latex_of(&registry, "\\section*{S}\n", OptionValues::new());
    assert!(latex.contains("\\flmHeading[toc=true]\n\\section*{S}"), "{latex}");
    // With the hook off, nothing carries the key.
    let plain = OptionValues::new().with(latex_recomposed::LatexRecomposedRenderOptions {
        heading_hook: false,
        ..latex_recomposed::LatexRecomposedRenderOptions::default()
    });
    let latex = latex_of(&registry, "\\section[toc=false]{P}\n", plain);
    assert!(!latex.contains("toc"), "{latex}");
    assert!(latex.contains("\\section{P}"), "{latex}");
    // A mapping writes `section` as `\chapter`, whichever command wrote it.
    let mapped = OptionValues::new().with(HeadingsLatexOptions::new().command("section", "chapter"));
    let latex = latex_of(&registry, "\\section{A}\n\n\\quietsection*{B}\n", mapped);
    assert!(latex.contains("\\chapter{A}"), "{latex}");
    assert!(latex.contains("\\chapter*{B}"), "{latex}");
    assert!(!latex.contains("\\section"), "{latex}");
}

#[test]
fn the_html_tag_follows_the_render_option_and_the_attributes_do_not_change() {
    let registry = registry_with(HeadingsConfig::new());
    let mut tags = std::collections::BTreeMap::new();
    tags.insert(flm_core::model::names::Name::from("section"), String::from("h2"));
    let render = OptionValues::new()
        .with(html::HtmlRenderOptions { heading_tags: tags, ..html::HtmlRenderOptions::default() });
    let run = run(
        &registry,
        html::WORKFLOW,
        "\\section{A}\n\n\\subsection{B}\n",
        NumberingOptions::new(),
        render,
    );
    assert_clean(&run);
    let html = common::html::fragment(&run);
    assert!(
        html.contains(
            "<h2 class=\"flm-pres-block--heading\" id=\"sec--A\" data-flm-heading=\"section\" \
             data-flm-level=\"1\" data-flm-toc=\"true\">A</h2>"
        ),
        "{html}"
    );
    // A kind the option does not name keeps the tag of its level.
    assert!(html.contains("<h2 class=\"flm-pres-block--heading\" id=\"sec--B\" data-flm-heading=\"subsection\" data-flm-level=\"2\""), "{html}");
}

// --- the configuration in the registry -----------------------------------------------------

#[test]
fn two_configurations_that_differ_in_a_declaration_or_a_default_digest_differently() {
    let digest = |headings: HeadingsConfig| registry_with(headings).digest();
    let shipped = digest(HeadingsConfig::new());
    assert_eq!(shipped, digest(HeadingsConfig::new()));
    let mut kinds = HeadingsConfig::new().headings;
    kinds[1] = kinds[1].clone().number_format(NumberTemplate::formats("#{roman}"));
    assert_ne!(shipped, digest(HeadingsConfig::new().headings(kinds)));
    let quiet = |value: &str| {
        HeadingsConfig::new()
            .command(HeadingCommand::new("quietsection", "section").default_value("toc", value))
    };
    assert_ne!(digest(quiet("false")), digest(quiet("true")));
}

#[test]
fn the_parents_of_the_paragraph_kind_enter_the_digest() {
    let digest = |headings: HeadingsConfig| registry_with(headings).digest();
    let mut kinds = HeadingsConfig::new().headings;
    let paragraph = kinds.iter_mut().find(|kind| kind.name.as_str() == "paragraph").unwrap();
    *paragraph =
        paragraph.clone().within(Within::parent("flm.headings.subsubsection").displayed_bare());
    assert_ne!(digest(HeadingsConfig::new()), digest(HeadingsConfig::new().headings(kinds)));
}

#[test]
fn a_command_naming_an_undeclared_kind_fails_the_build() {
    let config = StdcoreConfig::new()
        .headings(HeadingsConfig::new().command(HeadingCommand::new("chapter", "chapter")));
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(config));
    builder.use_package(flm_core::math_passthrough::package());
    let error = builder.build(&RegistryInputs::new()).expect_err("the build fails");
    let rendered = error.render();
    assert!(rendered.contains("`chapter`"), "{rendered}");
    assert!(rendered.contains("not declared"), "{rendered}");
}

#[test]
fn a_kind_no_command_writes_is_accepted_and_has_a_synthesized_value() {
    let mut kinds = HeadingsConfig::new().headings;
    kinds.push(HeadingDecl::new("part", 0));
    let commands = HeadingsConfig::new().commands;
    let registry = registry_with(HeadingsConfig::new().headings(kinds).commands(commands));
    // The kind has its element value, which no command names.
    let part = registry
        .elements_named(HEADING)
        .iter()
        .filter_map(|idx| registry.element(*idx))
        .find(|entry| {
            entry.element.value_as::<HeadingElement>().is_some_and(|value| value.decl.name.as_str() == "part")
        })
        .expect("the kind `part` has a value");
    assert!(part.element.value_as::<HeadingElement>().expect("valued").commands.is_empty());
    // The other kinds are written and rendered as usual.
    let run = run(&registry, text::WORKFLOW, "\\section{A}\n", NumberingOptions::new(), OptionValues::new());
    assert_clean(&run);
    assert_eq!(printed(&run), "A\n=\n");
}

#[test]
fn a_mapped_tag_that_is_not_writable_is_reported_and_the_level_tag_is_used() {
    let registry = registry_with(HeadingsConfig::new());
    let mut tags = std::collections::BTreeMap::new();
    tags.insert(flm_core::model::names::Name::from("section"), String::from("h2 onclick=x"));
    let render = OptionValues::new()
        .with(html::HtmlRenderOptions { heading_tags: tags, ..html::HtmlRenderOptions::default() });
    let run = run(&registry, html::WORKFLOW, "\\section{A}\n", NumberingOptions::new(), render);
    assert_eq!(reports(&run), [(String::from("flm.html.invalid-name"), Severity::Error)]);
    let html = common::html::fragment(&run);
    assert!(!html.contains("onclick"), "{html}");
    assert!(
        html.contains(
            "<h1 class=\"flm-pres-block--heading\" id=\"sec--A\" data-flm-heading=\"section\""
        ),
        "{html}"
    );
}
