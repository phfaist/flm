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
use flm_core::stdlib::backends::{html, latex_recomposed, text};
use flm_core::stdlib::features::theorems::{
    CHECK_PROOFS, MissingProof, TheoremClass, TheoremKind, TheoremsConfig,
};
use flm_core::stdlib::features::userdefs::UserdefsConfig;
use flm_core::stdlib::{CHECK_WORKFLOW, StdlibConfig};
use techy::error::Severity;

use common::stdlib::{UNCHECKED_OUTPUT, package_features_but, plan, registry_over, reports, run_workflow};

const MISSING: &str = "flm.theorems.missing-proof";
const BAD: &str = "flm.theorems.bad-relation-ref";

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdlib::registry(StdlibConfig::new())
}

/// The workflows the check runs under: validation alone, and two renders.
const WORKFLOWS: [&str; 3] = [CHECK_WORKFLOW, text::WORKFLOW, html::WORKFLOW];

/// What a run of `workflow` reports before the document's own diagnostics:
/// the Warning of the unchecked formulas of `math-passthrough` on the page
/// and in the recomposed LaTeX, nothing elsewhere.
fn unchecked_on(workflow: &str) -> Vec<(String, Severity)> {
    if workflow == html::WORKFLOW || workflow == latex_recomposed::WORKFLOW {
        vec![(UNCHECKED_OUTPUT.to_string(), Severity::Warning)]
    } else {
        Vec::new()
    }
}

/// Runs `workflow` of `registry` over one tree.
fn run(registry: &Arc<FlmRegistry>, workflow: &str, document: &str) -> Run {
    run_workflow(registry, workflow, &[document], &Overrides::none())
}

/// What `registry` reports over `document` under every workflow of
/// [`WORKFLOWS`], asserted to be the same under each, after what the
/// workflow reports of its own ([`unchecked_on`]): one verdict per document.
fn verdict_with(registry: &Arc<FlmRegistry>, document: &str) -> Vec<(String, Severity)> {
    let mut verdicts = WORKFLOWS.iter().map(|workflow| (workflow, reports(&run(registry, workflow, document))));
    let (_, first) = verdicts.next().expect("a workflow");
    for (workflow, verdict) in verdicts {
        let expected = [unchecked_on(workflow), first.clone()].concat();
        assert_eq!(verdict, expected, "`{workflow}` and `check` disagree over {document:?}");
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
    flm_core::dump::diag::expect(run.diagnostics().iter(), MISSING).at_line_col(3, 1).once();
}

/// The conditions of the missing-proof Warnings `registry` reports over
/// `document` under the `check` workflow.
fn missing_reports(registry: &Arc<FlmRegistry>, document: &str) -> Vec<MissingProof> {
    run(registry, CHECK_WORKFLOW, document)
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| diagnostic.data().downcast_ref::<MissingProof>())
        .cloned()
        .collect()
}

/// The missing-proof condition of a theorem of `kind` whose first accepted
/// label is `label`, suggesting a label of type `label_type` and a proof
/// written as the environment `proof_kind`.
fn missing_proof(kind: &str, label: Option<&str>, label_type: &str, proof_kind: Option<&str>) -> MissingProof {
    MissingProof {
        kind: kind.into(),
        label: label.map(String::from),
        label_type: label_type.into(),
        proof_kind: proof_kind.map(String::from),
    }
}

#[test]
fn the_warning_names_the_kind_and_the_first_label_and_the_remedies() {
    assert_eq!(
        missing_reports(&std(), "\\begin{theorem}\\label{thm:main}\\label{thm:other}A.\\end{theorem}"),
        [missing_proof("theorem", Some("thm:main"), "thm", Some("proof"))]
    );
}

#[test]
fn the_warning_on_a_statement_with_no_label_asks_for_one() {
    assert_eq!(
        missing_reports(&std(), "\\begin{lemma}A.\\end{lemma}"),
        [missing_proof("lemma", None, "thm", Some("proof"))]
    );
}

#[test]
fn the_warning_names_the_first_label_the_kind_accepts() {
    // `eq:a` is refused on a theorem and defines nothing: `[*eq:a]` would
    // resolve to nothing, so the report names `thm:b`.
    assert_eq!(
        missing_reports(&std(), "\\begin{theorem}\\label{eq:a}\\label{thm:b}A.\\end{theorem}"),
        [missing_proof("theorem", Some("thm:b"), "thm", Some("proof"))]
    );
    // With only a refused label, the theorem is as unlabeled.
    assert_eq!(
        missing_reports(&std(), "\\begin{lemma}\\label{eq:a}A.\\end{lemma}"),
        [missing_proof("lemma", None, "thm", Some("proof"))]
    );
}

#[test]
fn the_warning_names_a_host_kind_and_its_label_types() {
    let config = TheoremsConfig::new()
        .class("exerciselike", TheoremClass::numbered())
        .kind("exercise", TheoremKind::from_word("exerciselike", "exercise").expects_proof(true))
        .label_types(["ex", "thm"]);
    let registry = common::stdlib::registry(StdlibConfig::new().theorems(config));
    assert_eq!(
        missing_reports(&registry, "\\begin{exercise}\\label{ex:one}A.\\end{exercise}"),
        [missing_proof("exercise", Some("ex:one"), "ex", Some("proof"))]
    );
    assert_eq!(
        missing_reports(&registry, "\\begin{exercise}A.\\end{exercise}"),
        [missing_proof("exercise", None, "ex", Some("proof"))]
    );
}

#[test]
fn the_warning_names_the_host_kind_that_refers_back() {
    // The host's proof-like kind is `solution`; there is no `proof`.
    let config = TheoremsConfig::new()
        .without_kind("proof")
        .kind("solution", TheoremKind::from_word("prooflike", "solution"));
    let registry = common::stdlib::registry(StdlibConfig::new().theorems(config));
    assert_eq!(
        missing_reports(&registry, "\\begin{theorem}\\label{thm:a}A.\\end{theorem}"),
        [missing_proof("theorem", Some("thm:a"), "thm", Some("solution"))]
    );
    assert_eq!(
        missing_reports(&registry, "\\begin{theorem}A.\\end{theorem}"),
        [missing_proof("theorem", None, "thm", Some("solution"))]
    );
}

#[test]
fn with_no_kind_that_refers_back_the_warning_omits_that_remedy() {
    let config = TheoremsConfig::new().without_kind("proof");
    let registry = common::stdlib::registry(StdlibConfig::new().theorems(config));
    assert_eq!(
        missing_reports(&registry, "\\begin{theorem}\\label{thm:a}A.\\end{theorem}"),
        [missing_proof("theorem", Some("thm:a"), "thm", None)]
    );
    assert_eq!(
        missing_reports(&registry, "\\begin{lemma}A.\\end{lemma}"),
        [missing_proof("lemma", None, "thm", None)]
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
            let kind = diagnostic.data().downcast_ref::<MissingProof>().map_or("", |missing| missing.kind.as_str());
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
            unchecked_on(workflow),
            vec![
                (BAD.to_string(), Severity::Error),
                (MISSING.to_string(), Severity::Warning),
                (MISSING.to_string(), Severity::Warning),
                (MISSING.to_string(), Severity::Warning),
            ],
        ]
        .concat();
        assert_eq!(reports(&sequential), expected, "{workflow}");
        assert_eq!(reports(&parallel), expected, "{workflow}");
        let reported = |run: &Run| -> Vec<String> {
            run.diagnostics().iter().map(common::stdlib::identifier_and_data).collect()
        };
        assert_eq!(reported(&sequential), reported(&parallel), "{workflow}");
        let kinds: Vec<&str> = sequential
            .diagnostics()
            .iter()
            .filter_map(|diagnostic| diagnostic.data().downcast_ref::<MissingProof>())
            .map(|missing| missing.kind.as_str())
            .collect();
        assert_eq!(kinds, ["corollary", "theorem", "proposition"], "{workflow}");
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
    let registry = common::stdlib::registry(StdlibConfig::new().theorems(config));
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
    let registry = common::stdlib::registry(StdlibConfig::new().theorems(config));
    assert_eq!(verdict_with(&registry, "\\begin{remark}A.\\end{remark}"), missing(1));
}

#[test]
fn without_the_references_feature_no_missing_proof_is_reported() {
    let registry = registry_over(package_features_but(StdlibConfig::new(), "refs"));
    assert_eq!(verdict_with(&registry, "\\begin{theorem}[T]A.\\end{theorem}"), []);
    // A malformed back-reference is a fact of the title alone.
    assert_eq!(
        verdict_with(&registry, "\\begin{proof}[*nolabel]Q.\\end{proof}"),
        [(BAD.to_string(), Severity::Error)]
    );
}

#[test]
fn without_the_numbering_feature_the_check_reports_the_same() {
    let registry = registry_over(package_features_but(StdlibConfig::new(), "numbering"));
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
            [unchecked_on(workflow), vec![("flm.refs.nothing-to-show".to_string(), Severity::Error)]].concat(),
            "{workflow}"
        );
    }
}

// A plan that keeps a user definition — the recomposed LaTeX's default for a
// per-plan definition (`emit-call`) — holds the call in the document tree,
// not the definition's content, so the check sees nothing inside the
// definition: what a kept definition holds is LaTeX's to check. The workflows
// that expand the definition report what it holds.
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
    let registry = common::stdlib::registry(
        StdlibConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)),
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
            let expected = [unchecked_on(workflow), expanded.clone()].concat();
            assert_eq!(reports(&run(&registry, workflow, document)), expected, "{workflow} over {document:?}");
        }
        assert_eq!(
            reports(&run(&registry, latex_recomposed::WORKFLOW, document)),
            [unchecked_on(latex_recomposed::WORKFLOW), kept].concat(),
            "latex-recomposed over {document:?}"
        );
    }
}
