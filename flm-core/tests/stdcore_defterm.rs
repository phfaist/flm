//! The `defterm` feature: the configuration and its lead-in template, the
//! definition's reference point with its computed and written labels and
//! its anchor, the term link resolved by the references feature, the span a
//! term link inside its own definition is written as, the lead-in, the
//! feature without the references feature, the identifier of a heading
//! whose title holds a term link, and documents that must neither panic nor
//! loop; over the `text` workflow, with the `check` workflow where the
//! tables are what a test reads.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::{Definition, ExpandPolicy};
use flm_core::model::aspects::Label;
use flm_core::stdcore::backends::{latex_recomposed, text};
use flm_core::stdcore::features::defterm::{DEFAULT_LEAD_IN, DeftermConfig};
use flm_core::stdcore::features::refpoints::REFPOINTS;
use flm_core::stdcore::features::refs::{ANCHORS, LABELS};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig, package};
use techy::error::Severity;
use techy::serialize::SerialValue;

use common::lowering::text as printed;
use common::stdcore::{
    identifiers, package_features_but, projection, registry_over, reports, run_workflow,
};

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

/// What the registry build under `config` reported, rendered; a build that
/// succeeds is a test failure.
fn build_error(config: DeftermConfig) -> String {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().defterm(config)));
    builder.use_package(flm_core::math_passthrough::package());
    match builder.build(&RegistryInputs::new()) {
        Ok(_) => panic!("the registry build was expected to fail"),
        Err(error) => error.render(),
    }
}

/// Runs the `text` workflow of `registry` over one tree.
fn render_with(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, text::WORKFLOW, &[document], &Overrides::none())
}

/// The text of `document` under the standard package, with nothing
/// reported.
fn clean(document: &str) -> String {
    let run = render_with(&std(), document);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    printed(&run)
}

/// Runs the `check` workflow of the standard package over one tree.
fn check(document: &str) -> Run {
    run_workflow(&std(), CHECK_WORKFLOW, &[document], &Overrides::none())
}

/// The labels of every reference point of the run, in document order, as the
/// table of reference points projects them.
fn point_labels(run: &Run) -> Vec<Vec<String>> {
    let SerialValue::Map(tables) = projection(run, REFPOINTS) else { panic!("a map") };
    let SerialValue::Map(points) = tables.into_iter().next().expect("the table").1 else {
        panic!("a map of points")
    };
    points
        .into_iter()
        .map(|(_, point)| {
            let SerialValue::Map(fields) = point else { panic!("a point") };
            match fields.into_iter().find(|(key, _)| key == "labels") {
                Some((_, SerialValue::List(labels))) => labels
                    .into_iter()
                    .map(|label| match label {
                        SerialValue::Str(text) => text,
                        other => panic!("a label: {other:?}"),
                    })
                    .collect(),
                _ => Vec::new(),
            }
        })
        .collect()
}

/// The anchor of the point keeping `label`, where the run gave it one.
fn anchor_of(run: &Run, label: &Label) -> Option<String> {
    let labels = run.table(LABELS).expect("declared").expect("written");
    let anchors = run.table(ANCHORS).expect("declared").expect("written");
    let point = labels.by_label.get(label)?;
    anchors.id(*point).map(String::from)
}

/// The label a definition computes from the term written `term`.
fn term_label(term: &str) -> Label {
    Label::new("defterm", term)
}

/// A definition of `term` holding one paragraph, `body`.
fn definition(term: &str, body: &str) -> String {
    format!("\\begin{{defterm}}{{{term}}}\n{body}\n\\end{{defterm}}\n")
}

// ---------------------------------------------------------------------------------------------
// the configuration (P10, P12)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_default_lead_in_is_the_term_and_a_colon() {
    assert_eq!(DeftermConfig::new().lead_in.as_deref(), Some(DEFAULT_LEAD_IN));
    assert_eq!(DEFAULT_LEAD_IN, "#{term}: ");
    assert_eq!(StdcoreConfig::new().defterm, DeftermConfig::new());
}

#[test]
fn a_lead_in_that_does_not_compile_fails_the_build_naming_it() {
    for (template, cause) in [
        ("\\textbf{#{term}: ", "unclosed group"),
        ("#{word}: ", "placeholder ‘#word’"),
        ("#[bold]{term}: ", "#[bold]{term}"),
        ("#{term} (#{term}): ", "more than once"),
        ("  ", "blank"),
    ] {
        let error = build_error(DeftermConfig::new().lead_in(Some(template.to_string())));
        assert!(error.contains("the option `lead_in` of the feature `defterm`"), "{template}: {error}");
        assert!(error.contains(cause), "{template}: {error}");
    }
}

#[test]
fn the_digest_changes_with_the_lead_in() {
    let default = std();
    let richer = with_config(
        DeftermConfig::new().lead_in(Some("\\textbf{Definition} (\\emph{#{term}}): ".to_string())),
    );
    let none = with_config(DeftermConfig::new().lead_in(None));
    assert_ne!(default.digest(), richer.digest());
    assert_ne!(default.digest(), none.digest());
    assert_eq!(default.digest(), std().digest());
}

// ---------------------------------------------------------------------------------------------
// the definition (P8): its point, its labels, its anchor
// ---------------------------------------------------------------------------------------------

#[test]
fn a_definition_declares_one_point_with_its_computed_and_written_labels() {
    // The term's own label first, then one per alias, then the written ones,
    // of the types `term` and `topic`.
    let run = check(
        "\\begin{defterm}{Markov  chain}\\label{topic:mc}\\aliasterm{Markov chains}\\label{term:mc}\n\
         A process.\n\\end{defterm}\n",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(
        point_labels(&run),
        [["defterm:Markov chain", "defterm:Markov chains", "topic:mc", "term:mc"]]
    );
    // The anchor is the first label's: plain ASCII takes the exact path.
    assert_eq!(anchor_of(&run, &term_label("Markov chain")).as_deref(), Some("defterm-Markov_chain"));
}

#[test]
fn a_term_with_markup_or_accents_takes_the_tagged_anchor() {
    let run = check(&[definition("\\(\\alpha\\)-decay", "A decay."), definition("Élément", "An element.")].concat());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let markup = anchor_of(&run, &term_label("\\(\\alpha\\)-decay")).expect("an anchor");
    assert!(markup.starts_with("defterm-alpha--decay~"), "{markup}");
    let accented = anchor_of(&run, &term_label("Élément")).expect("an anchor");
    assert!(accented.starts_with("defterm-Element~"), "{accented}");
}

#[test]
fn a_label_of_another_type_on_a_definition_is_refused() {
    let run = check(&definition("Entropy", "A measure.").replace("{Entropy}", "{Entropy}\\label{sec:e}"));
    assert_eq!(
        reports(&run),
        [(String::from("flm.refs.label-type-not-accepted"), Severity::Error)],
        "{}",
        run.diagnostics().render_all()
    );
    // The message names the types an author may write, and no other.
    let message = run.diagnostics().render_all();
    assert!(message.contains("its type must be one of term, topic\n"), "{message}");
}

#[test]
fn a_written_label_of_the_computed_type_on_a_definition_is_refused() {
    // The acceptance applies to the labels the author wrote; the labels the
    // definition computes are exempt from it.
    let run = check(&definition("Entropy", "A measure.").replace("{Entropy}", "{Entropy}\\label{defterm:S}"));
    assert_eq!(
        reports(&run),
        [(String::from("flm.refs.label-type-not-accepted"), Severity::Error)],
        "{}",
        run.diagnostics().render_all()
    );
    let message = run.diagnostics().render_all();
    assert!(message.contains("`defterm:S` cannot be written on this defterm: its type must be one of term, topic\n"), "{message}");
    // The computed label still defines the term, and anchors the point.
    assert_eq!(anchor_of(&run, &term_label("Entropy")).as_deref(), Some("defterm-Entropy"));
    assert_eq!(anchor_of(&run, &term_label("S")), None);
}

#[test]
fn a_written_label_of_the_computed_type_declares_no_alias() {
    // `\aliasterm` is the one way to declare another spelling: a term link
    // does not find a definition through a label written of the computed
    // type.
    let document = "\\begin{defterm}{Entropy}\\label{defterm:Other}\\aliasterm{Entropies}\n\
                    A measure.\n\\end{defterm}\n\n\\term{Entropy}, \\term{Entropies}, \\term{Other}.\n";
    let run = render_with(&std(), document);
    assert_eq!(
        reports(&run),
        [
            (String::from("flm.refs.unresolved"), Severity::Error),
            (String::from("flm.refs.label-type-not-accepted"), Severity::Error),
        ],
        "{}",
        run.diagnostics().render_all()
    );
    assert!(run.diagnostics().render_all().contains("`defterm:Other`"));
}

#[test]
fn a_definition_written_through_a_user_definition_has_the_same_identity() {
    // The term's nodes come from two sources, the template's copy and the use
    // site's: each reads as it was written.
    let dt = Definition::macro_("dt", vec![ArgDef::mandatory("first")])
        .text("\\begin{defterm}{#1  chain}\nA process.\n\\end{defterm}");
    let registry = common::stdcore::registry(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(vec![dt])),
    );
    let run = render_with(&registry, "\\dt{Markov}\n\nSee \\term{Markov chain}.\n");
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(printed(&run), "Markov chain: A process.\n\nSee Markov chain.\n");
    let run = run_workflow(&registry, CHECK_WORKFLOW, &["\\dt{Markov}\n"], &Overrides::none());
    assert_eq!(point_labels(&run), [["defterm:Markov chain"]]);
}

#[test]
fn a_definition_in_a_kept_template_is_refused_with_the_user_definitions_error() {
    // On the recomposed LaTeX a template kept as a definition is lowered as a
    // value, which declares no document fact: the user-definitions feature
    // refuses it, naming the environment, with no code of this feature's.
    let kdt = Definition::macro_("kdt", vec![ArgDef::mandatory("term")])
        .text("\\begin{defterm}{#1}\nA process.\n\\end{defterm}")
        .expand(ExpandPolicy::PerPlan);
    let registry = common::stdcore::registry(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(vec![kdt])),
    );
    let run = run_workflow(&registry, latex_recomposed::WORKFLOW, &["\\kdt{Entropy}\n"], &Overrides::none());
    assert_eq!(
        identifiers(&run),
        ["flm.userdefs.kept-template-declares-point"],
        "{}",
        run.diagnostics().render_all()
    );
    let message = run.diagnostics().render_all();
    assert!(message.contains("(`\\begin{defterm}`)") && message.contains("expansion policy `always`"), "{message}");
}

// ---------------------------------------------------------------------------------------------
// the term link (P9, P13)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_term_link_resolves_to_the_definition_of_its_term_or_of_an_alias() {
    let run = check(
        "\\begin{defterm}{entropy}\\aliasterm{entropies}\\label{term:s}\\label{topic:s}\nA measure.\n\\end{defterm}\n\n\
         \\term{entropy}, \\term[entropy]{the entropy}, \\term{entropies}.\n",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let text = clean(
        "\\begin{defterm}{entropy}\\aliasterm{entropies}\\label{term:s}\\label{topic:s}\nA measure.\n\\end{defterm}\n\n\
         \\term{entropy}, \\term[entropy]{the entropy}, \\term{entropies}; \\ref{term:s}, \\ref{topic:s}.\n",
    );
    // A term link shows the text written for it; a reference to a written
    // label shows the term.
    assert!(text.ends_with("entropy, the entropy, entropies; entropy, entropy.\n"), "{text}");
}

#[test]
fn a_term_link_is_a_link_of_kind_term_to_the_definition_s_anchor() {
    let run = run_workflow(
        &std(),
        "html",
        &[&[definition("entropy", "A measure."), String::from("\\term{entropy}\n")].concat()],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let html = common::html::fragment(&run);
    assert!(html.contains("id=\"defterm-entropy\""), "{html}");
    assert!(
        html.contains("<a class=\"flm-pres-link\" href=\"#defterm-entropy\" data-flm-kind=\"term\">entropy</a>"),
        "{html}"
    );
}

#[test]
fn matching_is_exact_case_included() {
    let run = render_with(&std(), &[definition("entropy", "A measure."), String::from("\\term{Entropy}\n")].concat());
    assert_eq!(
        reports(&run),
        [(String::from("flm.refs.unresolved"), Severity::Error)],
        "{}",
        run.diagnostics().render_all()
    );
}

#[test]
fn a_term_link_inside_its_own_definition_is_a_span_and_no_link() {
    let document = "\\begin{defterm}{entropy}\\aliasterm{entropies}\n\
                    The \\term{entropy} of a state; \\term{entropies} add. See \\term{order}.\n\
                    \\end{defterm}\n\n\\begin{defterm}{order}\nThe opposite.\n\\end{defterm}\n";
    let run = run_workflow(&std(), "html", &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let html = common::html::fragment(&run);
    // The term and its alias, inside their own definition: a span of the
    // role, no link. Another term, inside it: a link.
    assert_eq!(html.matches("<span class=\"flm-pres-span flm-defterm-term\">").count(), 2, "{html}");
    assert!(html.contains("href=\"#defterm-order\" data-flm-kind=\"term\">order</a>"), "{html}");
    assert!(!html.contains("href=\"#defterm-entropy\""), "{html}");
}

#[test]
fn an_unresolved_term_link_is_the_references_feature_s_unresolved_reference() {
    let run = render_with(&std(), "See \\term{nothing}.\n");
    assert_eq!(
        reports(&run),
        [(String::from("flm.refs.unresolved"), Severity::Error)],
        "{}",
        run.diagnostics().render_all()
    );
    assert_eq!(printed(&run), "See nothing.\n");
    // Under `check` it is a Warning, as for any reference.
    let run = check("See \\term{nothing}.\n");
    assert_eq!(reports(&run), [(String::from("flm.refs.unresolved"), Severity::Warning)]);
}

#[test]
fn a_heading_whose_title_holds_a_term_link_is_identified_from_its_words() {
    // The bracket is no word of the title; the text shown is.
    let run = check(
        &[definition("Markov chain", "A process."), String::from("\\section{About \\term[Markov chain]{chains}}\n")]
            .concat(),
    );
    let SerialValue::Map(tables) = projection(&run, ANCHORS) else { panic!("a map") };
    let projected = format!("{tables:?}");
    assert!(projected.contains("\"sec--About_chains\")"), "{projected}");
}

// ---------------------------------------------------------------------------------------------
// the lead-in (P10)
// ---------------------------------------------------------------------------------------------

#[test]
fn the_default_lead_in_runs_into_the_first_paragraph() {
    // A colon ends the lead-in, so the text backend adds no period.
    let text = clean(&definition("Entropy", "A measure of disorder.\n\nIt grows."));
    assert_eq!(text, "Entropy: A measure of disorder.\n\nIt grows.\n");
}

#[test]
fn a_richer_lead_in_is_written_from_its_template() {
    let registry = with_config(
        DeftermConfig::new().lead_in(Some("\\textbf{Definition} (\\emph{#{term}}): ".to_string())),
    );
    let run = render_with(&registry, &definition("Entropy", "A measure of disorder."));
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(printed(&run), "Definition (Entropy): A measure of disorder.\n");
}

#[test]
fn no_lead_in_leaves_the_body_alone() {
    let registry = with_config(DeftermConfig::new().lead_in(None));
    let run = render_with(&registry, &definition("Entropy", "A measure of disorder."));
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(printed(&run), "A measure of disorder.\n");
}

#[test]
fn the_lead_in_shows_the_term_where_it_is_written() {
    // The term is lowered once, in the lead-in, as the document's own
    // content, and a term link in the body inside its own definition is the
    // span.
    let run = render_with(
        &std(),
        &definition("\\emph{Markov} chain", "A \\term{\\emph{Markov} chain} forgets."),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(printed(&run), "Markov chain: A Markov chain forgets.\n");
}

#[test]
fn a_body_opening_with_a_block_gets_the_lead_in_in_a_paragraph_of_its_own() {
    let text = clean("\\begin{defterm}{List}\n\\begin{itemize}\\item one\\end{itemize}\n\\end{defterm}\n");
    assert!(text.starts_with("List:\n"), "{text}");
    let text = clean("\\begin{defterm}{Empty}\\end{defterm}\n");
    assert_eq!(text, "Empty:\n");
}

// ---------------------------------------------------------------------------------------------
// without the references feature (P1)
// ---------------------------------------------------------------------------------------------

#[test]
fn without_the_references_feature_a_term_link_is_its_text() {
    let registry = registry_over(package_features_but(StdcoreConfig::new(), "refs"));
    let run = render_with(
        &registry,
        &[definition("entropy", "A measure, \\term{entropy}."), String::from("See \\term{entropy}, \\term{nothing}.\n")]
            .concat(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(printed(&run), "entropy: A measure, entropy.\n\nSee entropy, nothing.\n");
}

// ---------------------------------------------------------------------------------------------
// execution settings, and documents that must neither panic nor loop
// ---------------------------------------------------------------------------------------------

#[test]
fn the_text_is_the_same_sequentially_and_in_parallel() {
    let registry = std();
    let texts = [
        "\\begin{defterm}{a}\\aliasterm{as}\nAn \\term{a}.\n\\end{defterm}\n",
        "See \\term{as} and \\term[a]{it}.\n",
        "\\begin{defterm}{b}\nA \\term{b}, not \\term{a}.\n\\end{defterm}\n",
    ];
    let doc = common::pipeline::doc(&registry, &texts);
    let plan = common::stdcore::plan(&registry, text::WORKFLOW, &Overrides::none());
    let sequential = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
    let parallel = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::new());
    assert_eq!(printed(&sequential), printed(&parallel));
    assert_eq!(identifiers(&sequential), Vec::<String>::new());
}

#[test]
fn an_empty_term_names_nothing_and_is_reported() {
    for document in ["\\begin{defterm}{}\nA.\n\\end{defterm}\n", "\\begin{defterm}{  }\nA.\n\\end{defterm}\n"] {
        let run = render_with(&std(), document);
        assert_eq!(
            reports(&run),
            [(String::from("flm.defterm.empty-term"), Severity::Error)],
            "{document}: {}",
            run.diagnostics().render_all()
        );
    }
    let run = render_with(&std(), "See \\term{} and \\term[ ]{ }.\n");
    assert_eq!(
        identifiers(&run),
        ["flm.defterm.empty-term", "flm.defterm.empty-term"],
        "{}",
        run.diagnostics().render_all()
    );
}

#[test]
fn two_definitions_of_one_term_are_a_duplicate_label() {
    let run = check(&[definition("entropy", "One."), definition("entropy", "Two.")].concat());
    assert_eq!(
        reports(&run),
        [(String::from("flm.refs.duplicate-label"), Severity::Error)],
        "{}",
        run.diagnostics().render_all()
    );
}

#[test]
fn definitions_nested_ten_deep_render() {
    let open: String = (0..10).map(|depth| format!("\\begin{{defterm}}{{t{depth}}}\nWith \\term{{t0}}.\n")).collect();
    let document = format!("{open}{}", "\\end{defterm}\n".repeat(10));
    let text = clean(&document);
    assert!(text.starts_with("t0: With t0."), "{text}");
    assert_eq!(text.matches("With t0.").count(), 10, "{text}");
}

#[test]
fn hard_documents_neither_panic_nor_loop() {
    let registry = std();
    for document in [
        "\\term",
        "\\term[",
        "\\term[x",
        "\\term{",
        "\\term[]{}",
        "\\term[\\term{x}]{\\term{y}}",
        "\\begin{defterm}",
        "\\begin{defterm}{x}",
        "\\begin{defterm}{x}\\aliasterm{}\\end{defterm}",
        "\\begin{defterm}{x}\\aliasterm{}\\aliasterm{x}\\label{}\\end{defterm}",
        "\\begin{defterm}{\\term{x}}\\end{defterm}",
        "\\begin{defterm}{\\begin{defterm}{y}\\end{defterm}}\\end{defterm}",
        "\\aliasterm{x}",
        "\\section{\\term{x}}\\begin{defterm}{x}\\end{defterm}",
        "\\footnote{\\begin{defterm}{x}A \\term{x}.\\end{defterm}}",
        "$\\term{x}$ \\(\\text{\\term{x}}\\)",
    ] {
        for workflow in [text::WORKFLOW, CHECK_WORKFLOW, "html", latex_recomposed::WORKFLOW] {
            let _ = run_workflow(&registry, workflow, &[document], &Overrides::none());
        }
    }
}

#[test]
fn the_language_guide_s_example_prints_as_the_guide_says() {
    let text = clean(
        "\\begin{defterm}{Markov chain}\\label{topic:markov}\\aliasterm{Markov chains}\n  \
         A random process whose next state depends on its present state alone.\n\\end{defterm}\n\n\
         Every \\term{Markov chain} forgets its past, as all \\term{Markov chains} do;\n\
         \\term[Markov chain]{such a process} does too. See \\ref{topic:markov}.\n",
    );
    assert_eq!(
        text,
        "Markov chain: A random process whose next state depends on its present state\n\
         alone.\n\nEvery Markov chain forgets its past, as all Markov chains do; such a process\n\
         does too. See Markov chain.\n"
    );
}
