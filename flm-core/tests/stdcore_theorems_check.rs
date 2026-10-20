//! The check of the `theorems` feature, `flm.theorems.check-proofs`: a
//! statement expecting a proof draws `flm.theorems.missing-proof` where no
//! proof refers back to it and it carries no `\noproofref`, and a malformed
//! back-reference is `flm.theorems.bad-relation-ref`; both reported once,
//! the same, under the `check` workflow and under every render, over what
//! the document tree holds; inside a user definition the recomposed LaTeX
//! keeps, the check sees nothing, as the last test pins.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::lang::rewrites::userdefs::{Definition, ExpandPolicy};
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::theorems::{
    CHECK_PROOFS, TheoremClass, TheoremKind, TheoremsConfig,
};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig};
use techy::error::Severity;

use common::stdcore::{package_features_but, plan, registry_over, reports, run_workflow};

const MISSING: &str = "flm.theorems.missing-proof";
const BAD: &str = "flm.theorems.bad-relation-ref";

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The workflows the check runs under: validation alone, and two renders.
const WORKFLOWS: [&str; 3] = [CHECK_WORKFLOW, text::WORKFLOW, html::WORKFLOW];

/// Runs `workflow` of `registry` over one tree.
fn run(registry: &Arc<FlmRegistry>, workflow: &str, document: &str) -> Run {
    run_workflow(registry, workflow, &[document], &Overrides::none())
}

/// What `registry` reports over `document` under every workflow of
/// [`WORKFLOWS`], asserted to be the same under each: one verdict per
/// document.
fn verdict_with(registry: &Arc<FlmRegistry>, document: &str) -> Vec<(String, Severity)> {
    let mut verdicts = WORKFLOWS.iter().map(|workflow| (workflow, reports(&run(registry, workflow, document))));
    let (_, first) = verdicts.next().expect("a workflow");
    for (workflow, verdict) in verdicts {
        assert_eq!(verdict, first, "`{workflow}` and `check` disagree over {document:?}");
    }
    first
}

/// [`verdict_with`] under the standard package.
fn verdict(document: &str) -> Vec<(String, Severity)> {
    verdict_with(&std(), document)
}

/// A run's reports: each diagnostic's identifier and severity.
type Reports = Vec<(String, Severity)>;

/// `n` missing-proof Warnings.
fn missing(n: usize) -> Vec<(String, Severity)> {
    vec![(MISSING.to_string(), Severity::Warning); n]
}

#[test]
fn the_check_is_a_member_of_the_validated_collection_in_every_workflow() {
    let registry = std();
    for workflow in WORKFLOWS {
        let plan = plan(&registry, workflow, &Overrides::none());
        assert!(
            plan.scheduled_names().iter().any(|name| name.as_str() == CHECK_PROOFS),
            "`{workflow}` does not schedule the check"
        );
    }
}

#[test]
fn a_theorem_with_no_proof_draws_one_warning() {
    assert_eq!(verdict("\\begin{theorem}\\label{thm:a}A.\\end{theorem}"), missing(1));
    // Unlabeled, it can have no proof, and draws the Warning the same.
    assert_eq!(verdict("\\begin{theorem}A.\\end{theorem}"), missing(1));
    // Each kind that expects a proof.
    for kind in ["proposition", "lemma", "corollary"] {
        assert_eq!(verdict(&format!("\\begin{{{kind}}}A.\\end{{{kind}}}")), missing(1), "{kind}");
    }
    // A proof that does not refer back to the theorem is no proof of it.
    assert_eq!(
        verdict("\\begin{theorem}\\label{thm:a}A.\\end{theorem}\\begin{proof}Q.\\end{proof}"),
        missing(1)
    );
    // Nor is a plain reference to it.
    assert_eq!(
        verdict("\\begin{theorem}\\label{thm:a}A.\\end{theorem}\n\nSee \\ref{thm:a}."),
        missing(1)
    );
}

#[test]
fn the_warning_is_reported_at_the_environment() {
    let document = "Text.\n\n\\begin{lemma}A.\\end{lemma}";
    let run = run(&std(), CHECK_WORKFLOW, document);
    assert_eq!(run.diagnostics().iter().count(), 1);
    // The environment opens the third line.
    assert!(run.diagnostics().render_all().contains("line 3, col 1"), "{}", run.diagnostics().render_all());
}

/// The messages of the missing-proof Warnings `registry` reports over
/// `document` under the `check` workflow.
fn missing_messages(registry: &Arc<FlmRegistry>, document: &str) -> Vec<String> {
    run(registry, CHECK_WORKFLOW, document)
        .diagnostics()
        .iter()
        .filter(|diagnostic| diagnostic.identifier() == MISSING)
        .map(|diagnostic| diagnostic.message().to_string())
        .collect()
}

#[test]
fn the_warning_names_the_kind_and_the_first_label_and_the_remedies() {
    assert_eq!(
        missing_messages(&std(), "\\begin{theorem}\\label{thm:main}\\label{thm:other}A.\\end{theorem}"),
        [
            "theorem thm:main has no proof that refers back to it. Write its proof as \
             `\\begin{proof}[*thm:main]`; or mark the theorem with `\\noproofref`; or set \
             `expects_proof` to false for the kind `theorem` in the configuration of the theorems \
             feature."
        ]
    );
}

#[test]
fn the_warning_on_a_statement_with_no_label_asks_for_one() {
    assert_eq!(
        missing_messages(&std(), "\\begin{lemma}A.\\end{lemma}"),
        [
            "lemma has no label and no proof that refers back to it. Give it a `\\label{thm:...}` \
             and write its proof as `\\begin{proof}[*thm:...]`; or mark the lemma with \
             `\\noproofref`; or set `expects_proof` to false for the kind `lemma` in the \
             configuration of the theorems feature."
        ]
    );
}

#[test]
fn the_warning_names_the_first_label_the_kind_accepts() {
    // `eq:a` is refused on a theorem and defines nothing: `[*eq:a]` would
    // resolve to nothing, so the message names `thm:b`.
    assert_eq!(
        missing_messages(&std(), "\\begin{theorem}\\label{eq:a}\\label{thm:b}A.\\end{theorem}"),
        [
            "theorem thm:b has no proof that refers back to it. Write its proof as \
             `\\begin{proof}[*thm:b]`; or mark the theorem with `\\noproofref`; or set \
             `expects_proof` to false for the kind `theorem` in the configuration of the theorems \
             feature."
        ]
    );
    // With only a refused label, the theorem is as unlabeled.
    assert_eq!(
        missing_messages(&std(), "\\begin{lemma}\\label{eq:a}A.\\end{lemma}"),
        [
            "lemma has no label and no proof that refers back to it. Give it a `\\label{thm:...}` \
             and write its proof as `\\begin{proof}[*thm:...]`; or mark the lemma with \
             `\\noproofref`; or set `expects_proof` to false for the kind `lemma` in the \
             configuration of the theorems feature."
        ]
    );
}

#[test]
fn the_warning_names_a_host_kind_and_its_label_types() {
    let config = TheoremsConfig::new()
        .class("exerciselike", TheoremClass::numbered())
        .kind("exercise", TheoremKind::from_word("exerciselike", "exercise").expects_proof(true))
        .label_types(["ex", "thm"]);
    let registry = common::stdcore::registry(StdcoreConfig::new().theorems(config));
    assert_eq!(
        missing_messages(&registry, "\\begin{exercise}\\label{ex:one}A.\\end{exercise}"),
        [
            "exercise ex:one has no proof that refers back to it. Write its proof as \
             `\\begin{proof}[*ex:one]`; or mark the exercise with `\\noproofref`; or set \
             `expects_proof` to false for the kind `exercise` in the configuration of the \
             theorems feature."
        ]
    );
    assert_eq!(
        missing_messages(&registry, "\\begin{exercise}A.\\end{exercise}"),
        [
            "exercise has no label and no proof that refers back to it. Give it a \
             `\\label{ex:...}` and write its proof as `\\begin{proof}[*ex:...]`; or mark the \
             exercise with `\\noproofref`; or set `expects_proof` to false for the kind \
             `exercise` in the configuration of the theorems feature."
        ]
    );
}

#[test]
fn the_warning_names_the_host_kind_that_refers_back() {
    // The host's proof-like kind is `solution`; there is no `proof`.
    let config = TheoremsConfig::new()
        .without_kind("proof")
        .kind("solution", TheoremKind::from_word("prooflike", "solution"));
    let registry = common::stdcore::registry(StdcoreConfig::new().theorems(config));
    assert_eq!(
        missing_messages(&registry, "\\begin{theorem}\\label{thm:a}A.\\end{theorem}"),
        [
            "theorem thm:a has no proof that refers back to it. Write its proof as \
             `\\begin{solution}[*thm:a]`; or mark the theorem with `\\noproofref`; or set \
             `expects_proof` to false for the kind `theorem` in the configuration of the theorems \
             feature."
        ]
    );
    assert_eq!(
        missing_messages(&registry, "\\begin{theorem}A.\\end{theorem}"),
        [
            "theorem has no label and no proof that refers back to it. Give it a \
             `\\label{thm:...}` and write its proof as `\\begin{solution}[*thm:...]`; or mark the \
             theorem with `\\noproofref`; or set `expects_proof` to false for the kind `theorem` \
             in the configuration of the theorems feature."
        ]
    );
}

#[test]
fn with_no_kind_that_refers_back_the_warning_omits_that_remedy() {
    let config = TheoremsConfig::new().without_kind("proof");
    let registry = common::stdcore::registry(StdcoreConfig::new().theorems(config));
    assert_eq!(
        missing_messages(&registry, "\\begin{theorem}\\label{thm:a}A.\\end{theorem}"),
        [
            "theorem thm:a has no proof that refers back to it. Mark the theorem with \
             `\\noproofref`; or set `expects_proof` to false for the kind `theorem` in the \
             configuration of the theorems feature."
        ]
    );
    assert_eq!(
        missing_messages(&registry, "\\begin{lemma}A.\\end{lemma}"),
        [
            "lemma has no proof that refers back to it. Mark the lemma with `\\noproofref`; or \
             set `expects_proof` to false for the kind `lemma` in the configuration of the \
             theorems feature."
        ]
    );
}

#[test]
fn a_back_reference_shown_or_not_or_noproofref_satisfies_the_check() {
    assert_eq!(
        verdict("\\begin{theorem}\\label{thm:a}A.\\end{theorem}\\begin{proof}[*thm:a]Q.\\end{proof}"),
        []
    );
    assert_eq!(
        verdict("\\begin{theorem}\\label{thm:a}A.\\end{theorem}\\begin{proof}[**thm:a]Q.\\end{proof}"),
        []
    );
    assert_eq!(verdict("\\begin{theorem}\\label{thm:a}\\noproofref A.\\end{theorem}"), []);
    assert_eq!(verdict("\\begin{theorem}[T]\\noproofref A.\\end{theorem}"), []);
    // The proof may come first.
    assert_eq!(
        verdict("\\begin{proof}[*lem:a]Q.\\end{proof}\\begin{lemma}\\label{lem:a}A.\\end{lemma}"),
        []
    );
    // Any of the statement's labels.
    assert_eq!(
        verdict(
            "\\begin{lemma}\\label{lem:a}\\label{lem:b}A.\\end{lemma}\\begin{proof}[*lem:b]Q.\\end{proof}"
        ),
        []
    );
}

#[test]
fn a_kind_that_expects_no_proof_is_never_reported() {
    for kind in ["conjecture", "definition", "remark", "proof"] {
        assert_eq!(verdict(&format!("\\begin{{{kind}}}A.\\end{{{kind}}}")), [], "{kind}");
    }
    // A proof back-referring to a definition: nothing is expected of it,
    // nothing is reported.
    assert_eq!(
        verdict("\\begin{definition}\\label{def:a}A.\\end{definition}\\begin{proof}[*def:a]Q.\\end{proof}"),
        []
    );
}

#[test]
fn two_proofs_of_one_theorem_are_not_reported() {
    assert_eq!(
        verdict(
            "\\begin{theorem}\\label{thm:a}A.\\end{theorem}\\begin{proof}[*thm:a]Q.\\end{proof}\
             \\begin{proof}[**thm:a]R.\\end{proof}"
        ),
        []
    );
}

#[test]
fn a_statement_nothing_resolves_to_draws_the_warning_beside_the_references_report() {
    // A malformed label: the references feature's Error, and no proof can
    // name the theorem.
    let found = verdict("\\begin{theorem}\\label{nolabel}A.\\end{theorem}");
    assert_eq!(
        found,
        [("flm.refs.bad-label".to_string(), Severity::Error), (MISSING.to_string(), Severity::Warning)]
    );
    // A proof naming a label nothing declares resolves to nothing.
    let document = "\\begin{theorem}\\label{thm:a}A.\\end{theorem}\\begin{proof}[*thm:b]Q.\\end{proof}";
    let check = reports(&run(&std(), CHECK_WORKFLOW, document));
    assert_eq!(
        check,
        [("flm.refs.unresolved".to_string(), Severity::Warning), (MISSING.to_string(), Severity::Warning)]
    );
    // Under a render the unresolved reference is an Error, as it is for
    // every reference; the missing proof is the same Warning.
    let render = reports(&run(&std(), text::WORKFLOW, document));
    assert_eq!(
        render,
        [("flm.refs.unresolved".to_string(), Severity::Error), (MISSING.to_string(), Severity::Warning)]
    );
}

#[test]
fn the_reports_follow_the_document_order() {
    let run = run(
        &std(),
        CHECK_WORKFLOW,
        "\\begin{lemma}A.\\end{lemma}\\begin{proof}[*\\emph{x}]Q.\\end{proof}\
         \\begin{theorem}B.\\end{theorem}\\begin{corollary}C.\\end{corollary}",
    );
    let found: Vec<String> = run
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            let message = diagnostic.message();
            let kind = ["lemma", "theorem", "corollary"]
                .into_iter()
                .find(|kind| message.starts_with(&format!("{kind} ")))
                .unwrap_or("");
            format!("{} {kind}", diagnostic.identifier())
        })
        .collect();
    assert_eq!(
        found,
        [
            format!("{MISSING} lemma"),
            format!("{BAD} "),
            format!("{MISSING} theorem"),
            format!("{MISSING} corollary"),
        ]
    );
}

#[test]
fn across_trees_the_reports_follow_the_document_order_under_either_runner() {
    let registry = std();
    let texts = [
        "\\begin{lemma}\\label{lem:a}A.\\end{lemma}\\begin{proof}[*nolabel]Q.\\end{proof}\
         \\begin{corollary}C.\\end{corollary}",
        "\\begin{theorem}B.\\end{theorem}\\begin{proof}[*lem:a]Q.\\end{proof}\
         \\begin{proposition}P.\\end{proposition}",
    ];
    let doc = common::pipeline::doc(&registry, &texts);
    for workflow in WORKFLOWS {
        let plan = plan(&registry, workflow, &Overrides::none());
        let sequential = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential());
        let parallel = common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::new());
        // The first tree's lemma is proved in the second tree.
        let expected = [
            (BAD.to_string(), Severity::Error),
            (MISSING.to_string(), Severity::Warning),
            (MISSING.to_string(), Severity::Warning),
            (MISSING.to_string(), Severity::Warning),
        ];
        assert_eq!(reports(&sequential), expected, "{workflow}");
        assert_eq!(reports(&parallel), expected, "{workflow}");
        let messages_of = |run: &Run| -> Vec<String> {
            run.diagnostics().iter().map(|diagnostic| diagnostic.message().to_string()).collect()
        };
        assert_eq!(messages_of(&sequential), messages_of(&parallel), "{workflow}");
        let messages = messages_of(&sequential);
        assert!(messages[1].starts_with("corollary "), "{}", messages[1]);
        assert!(messages[2].starts_with("theorem "), "{}", messages[2]);
        assert!(messages[3].starts_with("proposition "), "{}", messages[3]);
    }
}

#[test]
fn a_malformed_back_reference_is_reported_once_under_every_workflow() {
    for title in ["*\\emph{x}", "*nolabel", "*", "**"] {
        let document = format!("\\begin{{proof}}[{title}]Q.\\end{{proof}}");
        assert_eq!(verdict(&document), [(BAD.to_string(), Severity::Error)], "{title}");
    }
    // On a kind that reads no back-reference a star is a title.
    assert_eq!(verdict("\\begin{theorem}[*thm:a]\\noproofref A.\\end{theorem}"), []);
}

#[test]
fn a_host_kind_expects_a_proof_where_its_configuration_says_so() {
    let config = TheoremsConfig::new()
        .class("exerciselike", TheoremClass::numbered())
        .kind("exercise", TheoremKind::from_word("exerciselike", "exercise").expects_proof(true))
        .kind("solution", TheoremKind::from_word("prooflike", "solution"));
    let registry = common::stdcore::registry(StdcoreConfig::new().theorems(config));
    assert_eq!(verdict_with(&registry, "\\begin{exercise}A.\\end{exercise}"), missing(1));
    assert_eq!(
        verdict_with(
            &registry,
            "\\begin{exercise}\\label{thm:e}A.\\end{exercise}\\begin{solution}[*thm:e]S.\\end{solution}"
        ),
        []
    );
    // The shipped `remark` expects none unless the host says so.
    let config = TheoremsConfig::new()
        .kind("remark", TheoremKind::from_word("definitionlike", "remark").expects_proof(true));
    let registry = common::stdcore::registry(StdcoreConfig::new().theorems(config));
    assert_eq!(verdict_with(&registry, "\\begin{remark}A.\\end{remark}"), missing(1));
}

#[test]
fn without_the_references_feature_no_missing_proof_is_reported() {
    let registry = registry_over(package_features_but(StdcoreConfig::new(), "refs"));
    assert_eq!(verdict_with(&registry, "\\begin{theorem}[T]A.\\end{theorem}"), []);
    // A malformed back-reference is a fact of the title alone.
    assert_eq!(
        verdict_with(&registry, "\\begin{proof}[*nolabel]Q.\\end{proof}"),
        [(BAD.to_string(), Severity::Error)]
    );
}

#[test]
fn without_the_numbering_feature_the_check_reports_the_same() {
    let registry = registry_over(package_features_but(StdcoreConfig::new(), "numbering"));
    assert_eq!(verdict_with(&registry, "\\begin{theorem}\\label{thm:a}A.\\end{theorem}"), missing(1));
    // A back-reference recorded and not shown: every workflow agrees.
    assert_eq!(
        verdict_with(
            &registry,
            "\\begin{theorem}\\label{thm:a}A.\\end{theorem}\\begin{proof}[**thm:a]Q.\\end{proof}"
        ),
        []
    );
    // A shown back-reference: its target has nothing to show without the
    // numbering feature, which is reported where the reference is lowered.
    // `check` lowers nothing, so it does not see it; the renders do.
    let shown = "\\begin{theorem}\\label{thm:a}A.\\end{theorem}\\begin{proof}[*thm:a]Q.\\end{proof}";
    assert_eq!(reports(&run(&registry, CHECK_WORKFLOW, shown)), []);
    for workflow in [text::WORKFLOW, html::WORKFLOW] {
        assert_eq!(
            reports(&run(&registry, workflow, shown)),
            [("flm.refs.nothing-to-show".to_string(), Severity::Error)],
            "{workflow}"
        );
    }
}

// A plan that keeps a user definition — the recomposed LaTeX's default for a
// per-plan definition (`emit-call`) — holds the call in the document tree,
// not the definition's content, so the check sees nothing inside the
// definition: what a kept definition holds is LaTeX's to check. The workflows
// that expand the definition report what it holds.
// See decision rationale at [§flm-dd-dr:std-theorem-proof-check]
#[test]
fn the_reports_under_each_workflow_for_content_inside_a_definition_the_recomposed_latex_keeps() {
    let definitions = vec![
        Definition::environment("keptthm", vec![])
            .content("\\begin{theorem}#{body}\\end{theorem}")
            .expand(ExpandPolicy::PerPlan),
        Definition::environment("keptproof", vec![])
            .content("\\begin{proof}[*lem:a]#{body}\\end{proof}")
            .expand(ExpandPolicy::PerPlan),
        Definition::environment("keptbad", vec![])
            .content("\\begin{proof}[*nolabel]#{body}\\end{proof}")
            .expand(ExpandPolicy::PerPlan),
    ];
    let registry = common::stdcore::registry(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)),
    );
    // Each document, what the expanding workflows report, what the
    // recomposed LaTeX reports.
    let cases: [(&str, Reports, Reports); 3] = [
        // A theorem inside the definition: expanded, it draws the Warning;
        // kept, the check does not see it.
        ("\\begin{keptthm}A.\\end{keptthm}", missing(1), vec![]),
        // A proof inside the definition naming a lemma outside it: expanded,
        // it proves the lemma; kept, it does not count, and the lemma draws
        // the Warning.
        (
            "\\begin{lemma}\\label{lem:a}L.\\end{lemma}\n\n\\begin{keptproof}Q.\\end{keptproof}",
            vec![],
            missing(1),
        ),
        // A malformed back-reference inside the definition: expanded, the
        // Error; kept, nothing.
        ("\\begin{keptbad}Q.\\end{keptbad}", vec![(BAD.to_string(), Severity::Error)], vec![]),
    ];
    for (document, expanded, kept) in cases {
        for workflow in WORKFLOWS {
            assert_eq!(reports(&run(&registry, workflow, document)), expanded, "{workflow} over {document:?}");
        }
        assert_eq!(
            reports(&run(&registry, latex_recomposed::WORKFLOW, document)),
            kept,
            "latex-recomposed over {document:?}"
        );
    }
}
