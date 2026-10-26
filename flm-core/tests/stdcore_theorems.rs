//! The `theorems` feature: the configuration and its three presets, the
//! element and its fields, the reference points and the counters, the
//! heading and the closing mark placed at lowering, the back-reference read
//! from a proof's title, the feature without the numbering and the references
//! features, and documents that must neither panic nor loop; over the `text`
//! workflow, with the `check` workflow where the tables are what a test
//! reads.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};
use flm_core::stdcore::features::numbering::{NumberingOptions, Within};
use flm_core::lang::{Flm, FlmOrigin, FlmSource, ParseOptions, parse_fragment};
use flm_core::model::aspects::{Label, Referencing};
use flm_core::model::doc::{Fragment, FragmentLayout};
use flm_core::model::element::element_value;
use flm_core::stdcore::features::numbering::{
    CounterStyleDecl, NumberTemplate, NumberingConfig, PluralWords, PrefixWords,
};
use flm_core::stdcore::features::refs::{ANCHORS, LABELS};
use flm_core::stdcore::features::theorems::{
    DEFAULT_CLOSING_MARK, DEFAULT_HEADING, DEFAULT_LABEL_TYPES, TheoremClass, TheoremElement,
    TheoremKind, TheoremsConfig, TitleReading, has_noproofref, title_reading,
};
use flm_core::stdcore::backends::text;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig, package};
use techy::core::node::NodeRef;

use common::lowering::text as printed;
use common::stdcore::{identifiers, package_features_but, plan, registry_over, run_workflow};

// ---------------------------------------------------------------------------------------------
// registries and runs
// ---------------------------------------------------------------------------------------------

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The standard package with the theorems feature under `config`.
fn with_config(config: TheoremsConfig) -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().theorems(config))
}

/// What the registry build under `config` reported, rendered; a build that
/// succeeds is a test failure.
fn build_error(config: TheoremsConfig) -> String {
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(StdcoreConfig::new().theorems(config)));
    builder.use_package(flm_core::math_passthrough::package());
    match builder.build(&RegistryInputs::new()) {
        Ok(_) => panic!("the registry build was expected to fail"),
        Err(error) => error.render(),
    }
}

/// Runs the `check` workflow of `registry` over one tree.
fn check_with(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, CHECK_WORKFLOW, &[document], &Overrides::none())
}

/// `document` parsed by `registry`'s language, as one fragment.
fn parsed(registry: &FlmRegistry, document: &str) -> Fragment {
    let source = Arc::new(FlmSource::new(document).with_origin(FlmOrigin::none()));
    parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("a parse")
}

/// The theorem nodes of `fragment`, in document order.
fn theorems(fragment: &Fragment) -> Vec<NodeRef<'_, Flm>> {
    fragment
        .tree()
        .root()
        .descendants()
        .filter(|node| element_value::<TheoremElement>(node).is_some())
        .collect()
}

/// The identifiers of every diagnostic of the run but the feature's check's
/// `flm.theorems.missing-proof`, which most documents here draw — a theorem
/// with no proof — and which `stdcore_theorems_check.rs` tests. A document
/// that proves every statement uses `identifiers`, unfiltered.
fn identifiers_but_missing_proof(run: &Run) -> Vec<String> {
    common::stdcore::identifiers(run)
        .into_iter()
        .filter(|identifier| identifier != "flm.theorems.missing-proof")
        .collect()
}

/// The anchor of the point keeping `label`, where the run gave it one.
fn anchor_of(run: &Run, label: &str) -> Option<String> {
    let labels = run.table(LABELS).expect("declared").expect("written");
    let anchors = run.table(ANCHORS).expect("declared").expect("written");
    let point = labels.by_label.get(&Label::parse(label).expect("a label"))?;
    anchors.id(*point).map(String::from)
}

// ---------------------------------------------------------------------------------------------
// the configuration (P2, P15; tests 15 and 19)
// ---------------------------------------------------------------------------------------------

/// The names of the kinds of `config`, in order.
fn kind_names(config: &TheoremsConfig) -> Vec<&str> {
    config.kinds.keys().map(|name| name.as_str()).collect()
}

/// The words of the kind `name` of `config`: the base pair, then each
/// variant's pair by name.
fn words(config: &TheoremsConfig, name: &str) -> PrefixWords {
    config.kind_and_class(name).expect("declared").0.words.clone()
}

#[test]
fn the_three_presets_hold_six_eight_and_thirteen_kinds() {
    assert_eq!(
        kind_names(&TheoremsConfig::simpleset()),
        ["corollary", "definition", "lemma", "proof", "proposition", "theorem"]
    );
    assert_eq!(
        kind_names(&TheoremsConfig::new()),
        [
            "conjecture", "corollary", "definition", "lemma", "proof", "proposition", "remark",
            "theorem"
        ]
    );
    assert_eq!(
        kind_names(&TheoremsConfig::richset()),
        [
            "claim", "conjecture", "corollary", "definition", "idea", "lemma", "observation",
            "problem", "proof", "proposition", "question", "remark", "theorem"
        ]
    );
    for config in [TheoremsConfig::simpleset(), TheoremsConfig::new(), TheoremsConfig::richset()] {
        assert_eq!(config.validate(), Ok(()));
    }
}

#[test]
fn the_shipped_classes_are_the_python_products() {
    let config = TheoremsConfig::new();
    let names: Vec<&str> = config.classes.keys().map(|name| name.as_str()).collect();
    assert_eq!(names, ["definitionlike", "prooflike", "theoremlike"]);
    for name in ["theoremlike", "definitionlike"] {
        assert_eq!(config.classes[name], TheoremClass::numbered(), "{name}");
    }
    let prooflike = &config.classes["prooflike"];
    assert!(!prooflike.numbered);
    assert!(prooflike.relation_ref);
    assert_eq!(prooflike.closing_mark.as_deref(), Some(DEFAULT_CLOSING_MARK));
    assert_eq!(DEFAULT_CLOSING_MARK, "\u{25a1}");
    assert_eq!(prooflike.heading, DEFAULT_HEADING);
    assert_eq!(DEFAULT_HEADING, r"#{name}\IfValueT{#{title}}{ (#{title})}");
    assert_eq!(config.label_types, DEFAULT_LABEL_TYPES);
    assert_eq!(DEFAULT_LABEL_TYPES, ["thm", "prop", "cor", "lem", "rem", "def", "dfn", "x", "topic"]);
}

#[test]
fn every_shipped_kind_writes_its_words_out_in_every_variant() {
    let pair = |one: &str, other: &str| PluralWords::new(one, other);
    for (name, lower, capital, abbreviated) in [
        ("theorem", ("theorem", "theorems"), ("Theorem", "Theorems"), ("Thm.", "Thms.")),
        (
            "proposition",
            ("proposition", "propositions"),
            ("Proposition", "Propositions"),
            ("Prop.", "Props."),
        ),
        ("lemma", ("lemma", "lemmas"), ("Lemma", "Lemmas"), ("Lem.", "Lems.")),
        ("corollary", ("corollary", "corollaries"), ("Corollary", "Corollaries"), ("Cor.", "Cors.")),
        (
            "conjecture",
            ("conjecture", "conjectures"),
            ("Conjecture", "Conjectures"),
            ("Conj.", "Conjs."),
        ),
        (
            "definition",
            ("definition", "definitions"),
            ("Definition", "Definitions"),
            ("Def.", "Defs."),
        ),
        ("remark", ("remark", "remarks"), ("Remark", "Remarks"), ("Rem.", "Rems.")),
        ("proof", ("proof", "proofs"), ("Proof", "Proofs"), ("Proof", "Proofs")),
        ("idea", ("idea", "ideas"), ("Idea", "Ideas"), ("Idea", "Ideas")),
        ("question", ("question", "questions"), ("Question", "Questions"), ("Qtn.", "Qtns.")),
        ("claim", ("claim", "claims"), ("Claim", "Claims"), ("Clm.", "Clms.")),
        (
            "observation",
            ("observation", "observations"),
            ("Observation", "Observations"),
            ("Obs.", "Obs."),
        ),
        ("problem", ("problem", "problems"), ("Problem", "Problems"), ("Prob.", "Probs.")),
    ] {
        let words = words(&TheoremsConfig::richset(), name);
        assert_eq!(words.base, pair(capital.0, capital.1), "{name}");
        assert_eq!(words.variants["capital"], pair(capital.0, capital.1), "{name}");
        assert_eq!(words.variants["lowercase"], pair(lower.0, lower.1), "{name}");
        assert_eq!(words.variants["abbreviated"], pair(abbreviated.0, abbreviated.1), "{name}");
    }
}

#[test]
fn the_kinds_that_expect_a_proof_are_the_four_statements() {
    let config = TheoremsConfig::richset();
    let expecting: Vec<&str> = config
        .kinds
        .iter()
        .filter(|(_, kind)| kind.expects_proof)
        .map(|(name, _)| name.as_str())
        .collect();
    assert_eq!(expecting, ["corollary", "lemma", "proposition", "theorem"]);
}

#[test]
fn a_kind_from_one_word_derives_the_regular_forms_and_no_abbreviation() {
    let kind = TheoremKind::from_word("theoremlike", "lemma");
    assert_eq!(kind.words.base, PluralWords::new("Lemma", "Lemmas"));
    assert_eq!(kind.words.variants["capital"], PluralWords::new("Lemma", "Lemmas"));
    assert_eq!(kind.words.variants["lowercase"], PluralWords::new("lemma", "lemmas"));
    assert_eq!(kind.words.variants.len(), 2);
    assert!(!kind.expects_proof);
    assert_eq!(kind.class.as_str(), "theoremlike");
}

#[test]
fn the_builders_add_replace_and_remove() {
    let config = TheoremsConfig::new()
        .kind("exercise", TheoremKind::from_word("definitionlike", "exercise"))
        .without_kind("conjecture")
        .class("exerciselike", TheoremClass::numbered().shared_counter(false))
        .label_types(["thm", "ex"]);
    assert!(config.kinds.contains_key("exercise"));
    assert!(!config.kinds.contains_key("conjecture"));
    assert!(config.classes.contains_key("exerciselike"));
    assert_eq!(config.label_types, ["thm", "ex"]);
    assert!(TheoremsConfig::empty().kinds.is_empty() && TheoremsConfig::empty().classes.is_empty());
}

#[test]
fn a_configuration_that_cannot_be_built_is_refused_naming_the_culprit() {
    let refused = |config: TheoremsConfig| config.validate().expect_err("refused");
    let error = refused(TheoremsConfig::new().kind("exercise", TheoremKind::from_word("nosuch", "exercise")));
    assert!(error.contains("`exercise`") && error.contains("`nosuch`"), "{error}");
    for name in ["", "my lemma", "le{mma", "le}mma", "lem\\ma", "lem%ma"] {
        let error = refused(TheoremsConfig::new().kind(name, TheoremKind::from_word("theoremlike", "x")));
        assert!(error.contains("the theorem kind name"), "{error}");
        let error = refused(TheoremsConfig::new().class(name, TheoremClass::numbered()));
        assert!(error.contains("the theorem class name"), "{error}");
    }
    // A kind that expects a proof but accepts no label could never be
    // proved: of an unnumbered class, or with no label type declared.
    let error = refused(
        TheoremsConfig::new().kind("claim", TheoremKind::from_word("prooflike", "claim").expects_proof(true)),
    );
    assert!(error.contains("`claim`") && error.contains("expects a proof"), "{error}");
    assert!(TheoremsConfig::new().kind("claim", TheoremKind::from_word("prooflike", "claim")).validate().is_ok());
    let error = refused(TheoremsConfig::new().label_types(Vec::<String>::new()));
    assert!(error.contains("expects a proof") && error.contains("no label types"), "{error}");
    // With no kind expecting a proof, no label type is needed.
    let unlabeled = TheoremsConfig::empty()
        .class("theoremlike", TheoremClass::numbered())
        .kind("note", TheoremKind::from_word("theoremlike", "note"))
        .label_types(Vec::<String>::new());
    assert_eq!(unlabeled.validate(), Ok(()));
    // The registry build fails naming the kind.
    let error = build_error(
        TheoremsConfig::new().kind("claim", TheoremKind::from_word("prooflike", "claim").expects_proof(true)),
    );
    assert!(error.contains("`claim`") && error.contains("expects a proof"), "{error}");
}

#[test]
fn a_host_kind_of_an_undeclared_class_fails_the_build_naming_it() {
    let error = build_error(
        TheoremsConfig::new().kind("exercise", TheoremKind::from_word("exerciselike", "exercise")),
    );
    assert!(error.contains("`exercise`") && error.contains("`exerciselike`"), "{error}");
    let error = build_error(TheoremsConfig::new().kind("my lemma", TheoremKind::from_word("theoremlike", "x")));
    assert!(error.contains("`my lemma`"), "{error}");
}

// ---------------------------------------------------------------------------------------------
// the element, its fields and its point (P3, P4, P5; tests 1, 3, 5, 7)
// ---------------------------------------------------------------------------------------------

#[test]
fn a_theorem_parses_to_one_node_of_its_kind_with_its_title_and_fields() {
    let registry = std();
    let fragment = parsed(&registry, "\\begin{theorem}x\\end{theorem}");
    let nodes = theorems(&fragment);
    assert_eq!(nodes.len(), 1);
    let value = element_value::<TheoremElement>(&nodes[0]).expect("a theorem");
    assert_eq!(value.name.as_str(), "theorem");
    assert_eq!(value.class_name.as_str(), "theoremlike");
    assert_eq!(nodes[0].ext().element.name.as_str(), "flm.theorems.theorem");
    assert!(nodes[0].argument_content_nodes_named("title").expect("declared").is_none());
    assert_eq!(title_reading(&nodes[0]), Some(TitleReading::Absent));

    // A star is no syntax: on a theorem it is a title.
    let fragment = parsed(&registry, "\\begin{theorem}[*thm:a]\\label{thm:b}\\noproofref x\\end{theorem}");
    let node = theorems(&fragment)[0];
    let title = node.argument_content_nodes_named("title").expect("declared").expect("written");
    assert_eq!(title.iter().filter_map(|node| node.chars()).collect::<String>(), "*thm:a");
    assert_eq!(title_reading(&node), Some(TitleReading::Title));
    assert!(has_noproofref(&node));
    let value = element_value::<TheoremElement>(&node).expect("a theorem");
    assert!(value.sites(&node).is_empty());
}

#[test]
fn a_proof_reads_a_back_reference_from_its_title() {
    let registry = std();
    let reading = |title: &str| {
        let fragment = parsed(&registry, &format!("\\begin{{proof}}[{title}]x\\end{{proof}}"));
        let node = theorems(&fragment)[0];
        let value = element_value::<TheoremElement>(&node).expect("a theorem").clone();
        (title_reading(&node).expect("a theorem"), value.sites(&node))
    };
    let label = Label::new("thm", "a");
    let (read, sites) = reading("*thm:a");
    assert_eq!(read, TitleReading::Relation { text: "thm:a".into(), label: label.clone(), shown: true });
    assert_eq!(sites.len(), 1);
    assert!(sites[0].shown && sites[0].display.with_prefix && sites[0].display.variant.is_none());
    assert_eq!(sites[0].target, label);
    let (read, sites) = reading(" ** thm:a ");
    assert_eq!(read, TitleReading::Relation { text: "thm:a".into(), label: label.clone(), shown: false });
    assert!(!sites[0].shown);
    for malformed in ["*nolabel", "*", "**", "*\\emph{x}", "* \\emph{thm:a}"] {
        let (read, sites) = reading(malformed);
        assert_eq!(read, TitleReading::Malformed, "{malformed}");
        assert!(sites.is_empty(), "{malformed}");
    }
    // Only a title whose first characters, before any command, are a star
    // is a back-reference; `[*\emph{x}]` above is malformed, while a title
    // opening with a command or a formula is a title, stars later or not.
    for title in ["Sketch of proof", "\\emph{*x}", "a *thm:a", "\\emph{Main} *claim", "\\(n=1\\) *"] {
        let (read, sites) = reading(title);
        assert_eq!(read, TitleReading::Title, "{title}");
        assert!(sites.is_empty(), "{title}");
    }
}

#[test]
fn the_numbered_kinds_share_one_counter_and_anchor_on_their_number() {
    let run = check_with(
        &std(),
        "\\begin{theorem}\\label{thm:a}A.\\end{theorem}\n\n\
         \\begin{lemma}\\label{lem:b}B.\\end{lemma}\n\n\
         \\begin{definition}\\label{def:c}C.\\end{definition}\n\n\
         \\begin{remark}\\label{rem:d}D.\\end{remark}\n\n\
         \\begin{proof}Q.\\end{proof}\n",
    );
    assert_eq!(
        identifiers_but_missing_proof(&run),
        Vec::<String>::new(),
        "{}",
        run.diagnostics().render_all()
    );
    assert_eq!(anchor_of(&run, "thm:a").as_deref(), Some("theorem-1"));
    assert_eq!(anchor_of(&run, "lem:b").as_deref(), Some("lemma-2"));
    assert_eq!(anchor_of(&run, "def:c").as_deref(), Some("definition-3"));
    assert_eq!(anchor_of(&run, "rem:d").as_deref(), Some("remark-4"));
}

#[test]
fn a_class_without_a_shared_counter_gives_each_kind_its_own_sequence() {
    let config = TheoremsConfig::new()
        .class("exerciselike", TheoremClass::numbered().shared_counter(false))
        .kind("exercise", TheoremKind::from_word("exerciselike", "exercise"))
        .kind("problem", TheoremKind::from_word("exerciselike", "problem"));
    let run = check_with(
        &with_config(config),
        "\\begin{exercise}\\label{x:a}A.\\end{exercise}\n\n\
         \\begin{theorem}\\label{thm:b}B.\\end{theorem}\n\n\
         \\begin{problem}\\label{x:c}C.\\end{problem}\n\n\
         \\begin{exercise}\\label{x:d}D.\\end{exercise}\n",
    );
    assert_eq!(
        identifiers_but_missing_proof(&run),
        Vec::<String>::new(),
        "{}",
        run.diagnostics().render_all()
    );
    assert_eq!(anchor_of(&run, "x:a").as_deref(), Some("exercise-1"));
    assert_eq!(anchor_of(&run, "thm:b").as_deref(), Some("theorem-1"));
    assert_eq!(anchor_of(&run, "x:c").as_deref(), Some("problem-1"));
    assert_eq!(anchor_of(&run, "x:d").as_deref(), Some("exercise-2"));
}

/// The kind `theorem` in a class of its own, counting on its own, beside the
/// lemma and the proposition, which share the counter.
fn theorems_on_their_own() -> TheoremsConfig {
    TheoremsConfig::new()
        .class("own", TheoremClass::numbered().shared_counter(false))
        .kind("theorem", TheoremKind::from_word("own", "theorem"))
}

/// Two theorems, a lemma and a proposition, interleaved, each referred to.
const ON_THEIR_OWN: &str = "\\begin{theorem}\\label{thm:a}A.\\end{theorem}\n\n\
     \\begin{lemma}\\label{lem:b}\\noproofref B.\\end{lemma}\n\n\
     \\begin{theorem}\\label{thm:c}C.\\end{theorem}\n\n\
     \\begin{proposition}\\label{prop:d}\\noproofref D.\\end{proposition}\n\n\
     \\ref{thm:a}; \\ref{lem:b}; \\ref{thm:c}; \\ref{prop:d}.\n";

#[test]
fn the_kind_theorem_counts_on_its_own_beside_kinds_sharing_the_counter() {
    let registry = with_config(theorems_on_their_own());
    let run = check_with(&registry, ON_THEIR_OWN);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(anchor_of(&run, "thm:a").as_deref(), Some("theorem-1"));
    assert_eq!(anchor_of(&run, "lem:b").as_deref(), Some("lemma-1"));
    assert_eq!(anchor_of(&run, "thm:c").as_deref(), Some("theorem-2"));
    assert_eq!(anchor_of(&run, "prop:d").as_deref(), Some("proposition-2"));
    let run = render_with(&registry, ON_THEIR_OWN);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(
        printed(&run),
        "Theorem\u{a0}1. A.\n\nLemma\u{a0}1. B.\n\nTheorem\u{a0}2. C.\n\nProposition\u{a0}2. D.\n\n\
         Theorem\u{a0}1; Lemma\u{a0}1; Theorem\u{a0}2; Proposition\u{a0}2.\n"
    );
}

#[test]
fn a_label_of_a_type_not_accepted_and_a_label_on_a_proof_are_refused() {
    let run = check_with(&std(), "\\begin{theorem}\\label{eq:x}A.\\end{theorem}\n");
    assert_eq!(identifiers_but_missing_proof(&run), ["flm.refs.label-type-not-accepted"]);
    let run = check_with(&std(), "\\begin{proof}\\label{x:pf}Q.\\end{proof}\n");
    assert_eq!(identifiers_but_missing_proof(&run), ["flm.refs.label-type-not-accepted"]);
    let message = run.diagnostics().render_all();
    assert!(message.contains("accepts no label"), "{message}");
    assert_eq!(anchor_of(&run, "x:pf"), None);
}

// ---------------------------------------------------------------------------------------------
// the heading and the closing mark (P6, P7, P9, P11; tests 2, 8, 9, 10, 12)
// ---------------------------------------------------------------------------------------------

/// Runs the `text` workflow of `registry` over one tree.
fn render_with(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, text::WORKFLOW, &[document], &Overrides::none())
}

/// The text of `document` under `registry`, with nothing reported.
fn clean_with(registry: &Arc<FlmRegistry>, document: &str) -> String {
    let run = render_with(registry, document);
    assert_eq!(
        identifiers_but_missing_proof(&run),
        Vec::<String>::new(),
        "{}",
        run.diagnostics().render_all()
    );
    printed(&run)
}

/// The text of `document` under the standard package, with nothing
/// reported.
fn clean(document: &str) -> String {
    clean_with(&std(), document)
}

#[test]
fn the_text_output_is_the_python_products() {
    assert_eq!(
        clean(
            "\\begin{theorem}[My theorem]\\label{thm:a}\n  The square root of two is irrational.\n\
             \\end{theorem}\n\n\\begin{proof}\n  We argue by contradiction.\n\\end{proof}\n"
        ),
        "Theorem\u{a0}1 (My theorem). The square root of two is irrational.\n\n\
         Proof. We argue by contradiction.\u{a0}\u{25a1}\n"
    );
}


/// The text of `document` under the theorems feature configured with the
/// prooflike class writing `heading`.
fn with_proof_heading(heading: &str) -> Arc<FlmRegistry> {
    with_config(TheoremsConfig::new().class(
        "prooflike",
        TheoremClass::unnumbered()
            .closing_mark(Some(DEFAULT_CLOSING_MARK.to_string()))
            .relation_ref(true)
            .heading(heading),
    ))
}

#[test]
fn the_heading_opens_the_first_paragraph_and_the_mark_closes_the_last() {
    assert_eq!(clean("\\begin{proof}A.\n\nB.\\end{proof}"), "Proof. A.\n\nB.\u{a0}\u{25a1}\n");
    // A body opening and closing with a list: the heading and the mark in
    // paragraphs of their own.
    assert_eq!(
        clean("\\begin{proof}\\begin{itemize}\\item a\\end{itemize}\\end{proof}"),
        "Proof.\n\n\u{2022} a\n\n\u{25a1}\n"
    );
    // An empty body: one paragraph holding both.
    assert_eq!(clean("\\begin{proof}\\end{proof}"), "Proof.\u{a0}\u{25a1}\n");
    assert_eq!(clean("\\begin{theorem}\\end{theorem}"), "Theorem\u{a0}1.\n");
    // In the lowered tree: one block of the theorem's role and kind tokens,
    // anchored, the heading and the mark decorations of its paragraphs.
    let run = render_with(&std(), "\\begin{lemma}[T]\\label{lem:a}A.\\end{lemma}\\begin{proof}Q.\\end{proof}");
    let lowered = format!("{:?}", common::lowering::lowered_projection(&run));
    for expected in [
        "\"flm.theorems.theorem\"",
        "\"theoremlike\"",
        "\"lemma\"",
        "\"prooflike\"",
        "\"lemma-1\"",
        "\"flm.theorems.name\"",
        "\"flm.theorems.title\"",
        "flm.pres.deco.runin",
        "flm.pres.deco.final",
    ] {
        assert!(lowered.contains(expected), "{expected} in {lowered}");
    }
}

#[test]
fn the_heading_is_written_from_the_class_template() {
    assert_eq!(clean("\\begin{theorem}A.\\end{theorem}"), "Theorem\u{a0}1. A.\n");
    assert_eq!(
        clean("\\begin{theorem}[My theorem]A.\\end{theorem}"),
        "Theorem\u{a0}1 (My theorem). A.\n"
    );
    assert_eq!(clean("\\begin{proof}Q.\\end{proof}"), "Proof. Q.\u{a0}\u{25a1}\n");
    assert_eq!(
        clean("\\begin{proof}[Sketch of proof]Q.\\end{proof}"),
        "Proof (Sketch of proof). Q.\u{a0}\u{25a1}\n"
    );
    // A title with markup, and a template joining with a word.
    assert_eq!(
        clean("\\begin{theorem}[\\emph{Key} lemma]A.\\end{theorem}"),
        "Theorem\u{a0}1 (Key lemma). A.\n"
    );
    let registry = with_proof_heading(r"#{name}\IfValueT{#{title}}{ of #{title}}");
    assert_eq!(
        clean_with(&registry, "\\begin{theorem}\\label{thm:a}A.\\end{theorem}\n\n\\begin{proof}[*thm:a]Q.\\end{proof}"),
        "Theorem\u{a0}1. A.\n\nProof of Theorem\u{a0}1. Q.\u{a0}\u{25a1}\n"
    );
    // The conditional tests whether a title is bound: nothing is bound for a
    // proof with no title, or whose back-reference is not shown.
    assert_eq!(
        clean_with(&registry, "\\begin{theorem}\\label{thm:a}A.\\end{theorem}\n\n\\begin{proof}[**thm:a]Q.\\end{proof}\\begin{proof}Q.\\end{proof}"),
        "Theorem\u{a0}1. A.\n\nProof. Q.\u{a0}\u{25a1}\n\nProof. Q.\u{a0}\u{25a1}\n"
    );
}

#[test]
fn a_heading_template_or_a_mark_that_does_not_compile_fails_the_build_naming_the_class() {
    let with_class = |class: TheoremClass| build_error(TheoremsConfig::new().class("theoremlike", class));
    for (template, cause) in [
        ("\\textbf{#{name}", "unclosed group"),
        ("#{word}", "#word"),
        ("#[bold]{name}", "#[bold]{name}"),
        ("#{name} #{title} #{title}", "more than once"),
    ] {
        let error = with_class(TheoremClass::numbered().heading(template));
        assert!(error.contains("`theoremlike`") && error.contains(cause), "{template}: {error}");
    }
    let error = with_class(TheoremClass::numbered().closing_mark(Some("\\textbf{x".to_string())));
    assert!(error.contains("closing mark") && error.contains("`theoremlike`"), "{error}");
    let error = with_class(TheoremClass::numbered().closing_mark(Some("#1".to_string())));
    assert!(error.contains("closing mark") && error.contains("`theoremlike`"), "{error}");
    let error = build_error(TheoremsConfig::new().kind(
        "exercise",
        TheoremKind::new("definitionlike", PrefixWords::new("\\textbf{Ex", "Exs")),
    ));
    assert!(error.contains("`exercise`"), "{error}");
    // `#{title}` as a conditional's test shows nothing, and is not counted.
    let config = TheoremsConfig::new()
        .class("theoremlike", TheoremClass::numbered().heading(r"#{name}\IfValueTF{#{title}}{: #{title}}{}"));
    assert_eq!(
        clean_with(&with_config(config), "\\begin{theorem}[T]A.\\end{theorem}\\begin{lemma}B.\\end{lemma}"),
        "Theorem\u{a0}1: T. A.\n\nLemma\u{a0}2. B.\n"
    );
}

#[test]
fn a_back_reference_shows_the_statement_it_proves() {
    let run = render_with(
        &std(),
        "\\begin{theorem}\\label{thm:a}A.\\end{theorem}\n\n\\begin{proof}[*thm:a]Q.\\end{proof}\n\n\
         \\begin{proof}[**thm:a]R.\\end{proof}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(
        printed(&run),
        "Theorem\u{a0}1. A.\n\nProof (Theorem\u{a0}1). Q.\u{a0}\u{25a1}\n\nProof. R.\u{a0}\u{25a1}\n"
    );
    // The reference is a link to the theorem's anchor.
    let lowered = format!("{:?}", common::lowering::lowered_projection(&run));
    assert!(lowered.contains("\"#theorem-1\""), "{lowered}");
    assert!(lowered.contains("flm.pres.link"), "{lowered}");
    // Nothing declares the label: the references feature's Error, the label's
    // text shown.
    let run = render_with(&std(), "\\begin{proof}[*thm:missing]Q.\\end{proof}");
    assert_eq!(identifiers(&run), ["flm.refs.unresolved"]);
    assert_eq!(printed(&run), "Proof (thm:missing). Q.\u{a0}\u{25a1}\n");
    // A star title that is no back-reference: the feature's Error, the title
    // as written.
    for (title, shown) in [("*\\emph{x}", "*x"), ("*nolabel", "*nolabel"), ("*", "*"), ("**", "**")] {
        let run = render_with(&std(), &format!("\\begin{{proof}}[{title}]Q.\\end{{proof}}"));
        assert_eq!(identifiers(&run), ["flm.theorems.bad-relation-ref"], "{title}");
        assert_eq!(printed(&run), format!("Proof ({shown}). Q.\u{a0}\u{25a1}\n"), "{title}");
    }
    // On a theorem-like kind a star is a title.
    assert_eq!(clean("\\begin{theorem}[*thm:a]A.\\end{theorem}"), "Theorem\u{a0}1 (*thm:a). A.\n");
}

#[test]
fn a_back_reference_to_a_label_holding_a_dash_reads_the_label_as_typed() {
    let run = render_with(
        &std(),
        "\\begin{lemma}\\label{lem:a--b}A.\\end{lemma}\n\n\\begin{proof}[*lem:a--b]Q.\\end{proof}",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert_eq!(printed(&run), "Lemma\u{a0}1. A.\n\nProof (Lemma\u{a0}1). Q.\u{a0}\u{25a1}\n");
}

#[test]
fn the_closing_mark_is_content_of_the_class() {
    let with_mark = |mark: Option<&str>| {
        with_config(TheoremsConfig::new().class(
            "prooflike",
            TheoremClass::unnumbered().closing_mark(mark.map(String::from)).relation_ref(true),
        ))
    };
    assert_eq!(clean_with(&with_mark(None), "\\begin{proof}Q.\\end{proof}"), "Proof. Q.\n");
    let run = render_with(&with_mark(Some("\\(\\blacksquare\\)")), "\\begin{proof}Q.\\end{proof}");
    let lowered = format!("{:?}", common::lowering::lowered_projection(&run));
    assert!(lowered.contains("blacksquare") && lowered.contains("flm.pres.deco.final"), "{lowered}");
    assert_eq!(
        identifiers_but_missing_proof(&run),
        Vec::<String>::new(),
        "{}",
        run.diagnostics().render_all()
    );
}

// ---------------------------------------------------------------------------------------------
// numbering and references (P5; tests 4, 6, 15)
// ---------------------------------------------------------------------------------------------

#[test]
fn numbered_within_sections_the_heading_and_the_anchor_follow() {
    let registry = common::stdcore::registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered)),
    );
    let options = NumberingOptions::new()
        .within("flm.theorems.theorem", Within::parent("flm.headings.section"));
    let doc = common::pipeline::doc_with_options(
        &registry,
        &["\\section{A}\n\n\\section{B}\n\n\\begin{theorem}A.\\end{theorem}\n\n\\begin{remark}B.\\end{remark}\n\n\
           \\begin{lemma}\\label{lem:c}C.\\end{lemma}\n\nSee \\ref{lem:c}.\n"],
        OptionValues::new().with(options),
    );
    let run = common::pipeline::run_with(
        &doc,
        &plan(&registry, text::WORKFLOW, &Overrides::none()),
        Externals::none(),
        &RunOptions::sequential(),
    );
    assert_eq!(
        identifiers_but_missing_proof(&run),
        Vec::<String>::new(),
        "{}",
        run.diagnostics().render_all()
    );
    let printed = printed(&run);
    assert!(printed.contains("Lemma\u{a0}2.3. C."), "{printed}");
    assert!(printed.contains("See Lemma\u{a0}2.3."), "{printed}");
    assert_eq!(anchor_of(&run, "lem:c").as_deref(), Some("lemma-2-3"));
}

#[test]
fn a_kinds_own_counter_numbered_within_sections_is_named_by_its_kind() {
    let registry = common::stdcore::registry(
        StdcoreConfig::new()
            .headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered))
            .theorems(theorems_on_their_own()),
    );
    let options = NumberingOptions::new()
        .within("flm.theorems.counter.theorem", Within::parent("flm.headings.section"));
    let doc = common::pipeline::doc_with_options(
        &registry,
        &["\\section{A}\n\n\\section{B}\n\n\\begin{theorem}\\label{thm:a}A.\\end{theorem}\n\n\
           \\begin{lemma}\\label{lem:b}\\noproofref B.\\end{lemma}\n\nSee \\ref{thm:a}; \\ref{lem:b}.\n"],
        OptionValues::new().with(options),
    );
    let run = common::pipeline::run_with(
        &doc,
        &plan(&registry, text::WORKFLOW, &Overrides::none()),
        Externals::none(),
        &RunOptions::sequential(),
    );
    assert_eq!(
        identifiers_but_missing_proof(&run),
        Vec::<String>::new(),
        "{}",
        run.diagnostics().render_all()
    );
    let printed = printed(&run);
    assert!(printed.contains("Theorem\u{a0}2.1. A."), "{printed}");
    assert!(printed.contains("Lemma\u{a0}1. B."), "{printed}");
    assert!(printed.contains("See Theorem\u{a0}2.1; Lemma\u{a0}1."), "{printed}");
    assert_eq!(anchor_of(&run, "thm:a").as_deref(), Some("theorem-2-1"));
}

#[test]
fn a_reference_to_a_theorem_reads_the_kinds_words() {
    let run = render_with(
        &std(),
        "\\begin{theorem}\\label{thm:a}A.\\end{theorem}\n\n\\begin{lemma}\\label{lem:b}B.\\end{lemma}\n\n\
         \\ref{thm:a}; \\ref{thm:a,lem:b}; \\ref{lem:b,thm:a}; \\ref[abbreviated]{lem:b}; \
         \\ref[lowercase]{lem:b}; \\ref[S]{lem:b}; \\ref[capital]{lem:b}.",
    );
    assert_eq!(
        identifiers_but_missing_proof(&run),
        Vec::<String>::new(),
        "{}",
        run.diagnostics().render_all()
    );
    // The printed text is wrapped: its line breaks are read as spaces.
    let printed = printed(&run).replace('\n', " ");
    assert!(
        printed.ends_with(
            "Theorem\u{a0}1; Theorem\u{a0}1 and Lemma\u{a0}2; Lemma\u{a0}2 and Theorem\u{a0}1; \
             Lem.\u{a0}2; lemma\u{a0}2; Lemma\u{a0}2; Lemma\u{a0}2. "
        ),
        "{printed}"
    );
    // Two statements of one kind: the style's plural word.
    let two = clean("\\begin{lemma}\\label{lem:a}A.\\end{lemma}\\begin{lemma}\\label{lem:b}B.\\end{lemma}\\ref{lem:a,lem:b}");
    assert!(two.ends_with("Lemmas\u{a0}1 and 2\n"), "{two:?}");
}

#[test]
fn a_kind_from_one_word_has_no_abbreviation_and_a_host_names_its_own_variant() {
    let config = TheoremsConfig::new()
        .kind("exercise", TheoremKind::from_word("definitionlike", "exercise"))
        .kind(
            "problem",
            TheoremKind::new(
                "definitionlike",
                PrefixWords::new("Problem", "Problems")
                    .with_variant("short", PluralWords::new("Pb.", "Pbs.")),
            ),
        );
    let registry = with_config(config);
    let run = render_with(
        &registry,
        "\\begin{exercise}\\label{x:a}A.\\end{exercise}\n\n\\begin{problem}\\label{x:b}B.\\end{problem}\n\n\
         \\ref[abbreviated]{x:a}; \\ref[lowercase]{x:a}; \\ref[short]{x:b}.",
    );
    assert_eq!(identifiers_but_missing_proof(&run), ["flm.refs.undeclared-variant"]);
    assert!(
        printed(&run).ends_with("Exercise\u{a0}1; exercise\u{a0}1; Pb.\u{a0}2.\n"),
        "{}",
        printed(&run)
    );
}

#[test]
fn a_host_s_label_types_and_a_kind_s_restyled_number_apply() {
    // The feature's one list of label types replaced: a type outside it is
    // refused, a type in it accepted.
    let registry = with_config(TheoremsConfig::new().label_types(["ex"]));
    let run = check_with(&registry, "\\begin{lemma}\\label{lem:a}\\label{ex:b}A.\\end{lemma}\n");
    assert_eq!(identifiers_but_missing_proof(&run), ["flm.refs.label-type-not-accepted"]);
    // One kind's number written otherwise: its style replaced by name through
    // the numbering feature's configuration.
    let mut lemmas = CounterStyleDecl::new("flm.theorems.style.lemma");
    lemmas.number_format = NumberTemplate::formats("#{Roman}");
    lemmas.prefix_words = PrefixWords::new("Lemma~", "Lemmas~");
    let registry = common::stdcore::registry(
        StdcoreConfig::new().numbering(NumberingConfig::new().style(lemmas)),
    );
    let printed = clean_with(
        &registry,
        "\\begin{theorem}\\noproofref A.\\end{theorem}\n\n\\begin{lemma}\\label{lem:b}\\noproofref B.\\end{lemma}\n\nSee \\ref{lem:b}.",
    );
    assert_eq!(printed, "Theorem\u{a0}1. A.\n\nLemma\u{a0}II. B.\n\nSee Lemma\u{a0}II.\n");
}

// ---------------------------------------------------------------------------------------------
// absence (P14; test 17)
// ---------------------------------------------------------------------------------------------

#[test]
fn without_the_numbering_feature_the_heading_shows_the_word_alone() {
    let registry = registry_over(package_features_but(StdcoreConfig::new(), "numbering"));
    let run = render_with(
        &registry,
        "\\begin{theorem}\\label{thm:a}A.\\end{theorem}\n\n\\begin{proof}[*thm:a]Q.\\end{proof}",
    );
    // The statement has neither a number nor content to show: the proof's
    // heading shows its label, and the reference is reported.
    assert_eq!(identifiers(&run), ["flm.refs.nothing-to-show"], "{}", run.diagnostics().render_all());
    assert_eq!(printed(&run), "Theorem. A.\n\nProof (thm:a). Q.\u{a0}\u{25a1}\n");
    assert_eq!(anchor_of(&run, "thm:a").as_deref(), Some("thm-a"));
}

#[test]
fn without_the_references_feature_a_back_reference_shows_its_label() {
    let registry = registry_over(package_features_but(StdcoreConfig::new(), "refs"));
    let run = render_with(
        &registry,
        "\\begin{theorem}[T]A.\\end{theorem}\n\n\\begin{proof}[*thm:a]Q.\\end{proof}",
    );
    assert_eq!(
        identifiers_but_missing_proof(&run),
        Vec::<String>::new(),
        "{}",
        run.diagnostics().render_all()
    );
    // The theorem is still numbered: the reference points are collected by
    // their own feature, and the numbering feature counts them.
    assert_eq!(printed(&run), "Theorem\u{a0}1 (T). A.\n\nProof (thm:a). Q.\u{a0}\u{25a1}\n");
}

// ---------------------------------------------------------------------------------------------
// no panic, no loop (test 18)
// ---------------------------------------------------------------------------------------------

#[test]
fn hard_documents_neither_panic_nor_loop() {
    let mut deep = String::new();
    for _ in 0..10 {
        deep.push_str("\\begin{proof}\\begin{theorem}");
    }
    deep.push('x');
    for _ in 0..10 {
        deep.push_str("\\end{theorem}\\end{proof}");
    }
    for document in [
        "\\begin{theorem}\\end{theorem}",
        "\\begin{theorem}[]\\end{theorem}",
        deep.as_str(),
        "Text.\\footnote{\\begin{theorem}[T]\\label{thm:a}A.\\end{theorem}}",
        "\\begin{theorem}[\\begin{lemma}x\\end{lemma}]A.\\end{theorem}",
        "\\begin{proof}[*]Q.\\end{proof}",
        "\\begin{proof}[***]Q.\\end{proof}",
        "\\begin{proof}[***thm:a]Q.\\end{proof}",
        "\\begin{proof}[*thm:a]\\label{x:a}\\noproofref\\noproofref Q.\\end{proof}",
        "\\begin{theorem}\\label{thm:a}\\begin{proof}[*thm:a]Q.\\end{proof}\\end{theorem}",
    ] {
        for workflow in [text::WORKFLOW, CHECK_WORKFLOW, "html"] {
            let _ = run_workflow(&std(), workflow, &[document], &Overrides::none());
        }
    }
}
