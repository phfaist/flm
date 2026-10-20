//! The `theorems` feature on the recomposed LaTeX: a theorem written back as
//! the environment the author wrote, its labels in the labels hook,
//! `\noproofref` as written, a proof's back-reference in the two forms the
//! render option `resolved_proof_refs` chooses between, and the declarations
//! of the kinds in the style file (goldens, which `tests/latex_compile.rs`
//! compiles under both engines, with and without the author's own
//! `hyperref`); and a theorem inside a user definition's template kept as a
//! LaTeX definition.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::{Definition, ExpandPolicy};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::latex_recomposed::{self, LatexRecomposedRenderOptions};
use flm_core::stdcore::features::numbering::PrefixWords;
use flm_core::stdcore::features::theorems::{TheoremClass, TheoremKind, TheoremsConfig};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::StdcoreConfig;

use common::stdcore::{identifiers, run_workflow};

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The overrides asking for the resolved form of a back-reference, or none.
fn overrides(resolved: bool) -> Overrides {
    Overrides {
        render_options: OptionValues::new().with(LatexRecomposedRenderOptions {
            resolved_proof_refs: resolved,
            ..Default::default()
        }),
        ..Overrides::none()
    }
}

/// The recomposition of `document` under `registry` in the form `resolved`
/// chooses; the run must report nothing.
fn recompose(registry: &Arc<FlmRegistry>, document: &str, resolved: bool) -> Run {
    let run = run_workflow(registry, latex_recomposed::WORKFLOW, &[document], &overrides(resolved));
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    run
}

/// The standard package with the theorems feature under `config`.
fn with_config(config: TheoremsConfig) -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new().theorems(config))
}

/// The recomposed LaTeX of `document` in the form `resolved` chooses, and its
/// style file; the run must report nothing.
fn written(registry: &Arc<FlmRegistry>, document: &str, resolved: bool) -> (String, String) {
    let run = recompose(registry, document, resolved);
    (common::latex::output(&run), common::latex::style(&run).0)
}

/// The line of the style file declaring `name`, guarded on it.
fn declaration(name: &str, body: &str) -> String {
    format!("\\ifcsname {name}\\endcsname\\else{body}\\fi\n")
}

/// What the declaration of the numbered kind `env` writes after the
/// environment: the kind's reference type for `zref-clever`, with the words
/// given — the abbreviated pair, where there is one, under both the
/// capitalized and the lowercase abbreviated keys — and the counter
/// `counter` the environment steps mapped to it.
fn typed_words(env: &str, words: [&str; 4], abbreviated: Option<[&str; 2]>, counter: &str) -> String {
    let [capital_one, capital_other, lower_one, lower_other] = words;
    let abbreviated = match abbreviated {
        Some([one, other]) => format!(
            ",Name-sg-ab={{{one}}},name-sg-ab={{{one}}},Name-pl-ab={{{other}}},name-pl-ab={{{other}}}"
        ),
        None => String::new(),
    };
    format!(
        "\\zcRefTypeSetup{{{env}}}{{Name-sg={{{capital_one}}},name-sg={{{lower_one}}},Name-pl={{{capital_other}}},name-pl={{{lower_other}}}{abbreviated}}}\\AddToHook{{env/{env}/begin}}{{\\zcsetup{{countertype={{{counter}={env}}}}}}}"
    )
}

/// [`typed_words`] for a kind of the default configuration, whose words are
/// its name capitalized, with an `s` for the plural, and in lowercase, and
/// whose abbreviated words are the ones the configuration ships.
fn typed(env: &str, counter: &str) -> String {
    let capital = format!("{}{}", env[..1].to_uppercase(), &env[1..]);
    let abbreviated = match env {
        "theorem" => ["Thm.", "Thms."],
        "lemma" => ["Lem.", "Lems."],
        "conjecture" => ["Conj.", "Conjs."],
        "remark" => ["Rem.", "Rems."],
        "definition" => ["Def.", "Defs."],
        other => panic!("no abbreviated words known here for the kind `{other}`"),
    };
    typed_words(env, [&capital, &format!("{capital}s"), env, &format!("{env}s")], Some(abbreviated), counter)
}

/// The declaration of the numbered kind `env`, its body `body` followed by
/// its reference type ([`typed`]).
fn numbered(env: &str, body: &str, counter: &str) -> String {
    declaration(env, &format!("{body}{}", typed(env, counter)))
}

// ---------------------------------------------------------------------------------------------
// the environments as written (test 14)
// ---------------------------------------------------------------------------------------------

/// The cases of the reference implementation's recomposer: a title, a label,
/// a back-reference shown and one recorded only, `\noproofref`.
const CASES: &str = "\\begin{theorem}[My Theorem]\\label{thm:a}
The square root of two is irrational.
\\end{theorem}

\\begin{proof}[*thm:a]
By contradiction.
\\end{proof}

\\begin{lemma}\\label{lem:b}
A lemma.
\\end{lemma}

\\begin{proof}[**lem:b]
Recorded only.
\\end{proof}

\\begin{conjecture}\\noproofref
Open.
\\end{conjecture}
";

#[test]
fn the_environments_are_written_as_the_author_wrote_them_in_the_authored_form() {
    let (output, style) = written(&std(), CASES, false);
    for expected in [
        "\\begin{theorem}[My Theorem]\\flmLabels{\\label{ref1}}\nThe square root of two is irrational.\n\\end{theorem}",
        "\\begin{proof}[*ref1]\nBy contradiction.\n\\end{proof}",
        "\\begin{lemma}\\flmLabels{\\label{ref2}}\nA lemma.\n\\end{lemma}",
        "\\begin{proof}[**ref2]\nRecorded only.\n\\end{proof}",
        "\\begin{conjecture}\\noproofref Open.\n\\end{conjecture}",
    ] {
        assert!(output.contains(expected), "{expected}\n{output}");
    }
    // The package that reads the back-reference is requested, asked for no
    // kind of its own, and nothing of `amsthm`'s own is.
    assert!(style.contains("\\RequirePackage[thmset=empty]{phfthm}\n"), "{style}");
    assert!(!style.contains("\\RequirePackage{amsthm}"), "{style}");
    // `\noproofref` is provided as nothing where no package defines it.
    assert!(style.contains("\\ProvideDocumentCommand\\noproofref{}{}\n"), "{style}");
    // The proof title hook is provided with the authored form's default,
    // the title alone, and with that one only.
    assert!(style.contains("\\ProvideDocumentCommand\\flmProofTitle{m}{#1}\n"), "{style}");
    assert_eq!(style.matches("\\ProvideDocumentCommand\\flmProofTitle").count(), 1, "{style}");
}

#[test]
fn a_back_reference_is_resolved_into_the_proof_title_hook_in_the_resolved_form() {
    let (output, style) = written(&std(), CASES, true);
    for expected in [
        "\\begin{theorem}[My Theorem]\\flmLabels{\\label{ref1}}\n",
        "\\begin{proof}[{\\flmProofTitle{\\flmRef{ref1}}}]\nBy contradiction.\n\\end{proof}",
        "\\begin{proof}\nRecorded only.\n\\end{proof}",
        "\\begin{conjecture}\\noproofref Open.\n\\end{conjecture}",
    ] {
        assert!(output.contains(expected), "{expected}\n{output}");
    }
    assert!(style.contains("\\RequirePackage{amsthm}\n"), "{style}");
    assert!(!style.contains("\\RequirePackage{phfthm}"), "{style}");
    assert!(style.contains("\\ProvideDocumentCommand\\noproofref{}{}\n"), "{style}");
    // The proof title hook is provided with the resolved form's default,
    // and with that one only.
    assert!(style.contains("\\ProvideDocumentCommand\\flmProofTitle{m}{\\proofname\\space (#1)}\n"), "{style}");
    assert_eq!(style.matches("\\ProvideDocumentCommand\\flmProofTitle").count(), 1, "{style}");
}

#[test]
fn a_title_the_author_wrote_on_a_proof_is_written_in_the_proof_title_hook_in_both_forms() {
    let document = "\\begin{proof}[Sketch of \\emph{proof}]\nShort.\n\\end{proof}\n\n\\begin{proof}[*\\emph{x}]\nMalformed.\n\\end{proof}\n";
    for resolved in [false, true] {
        let run = run_workflow(&std(), latex_recomposed::WORKFLOW, &[document], &overrides(resolved));
        let output = common::latex::output(&run);
        assert!(output.contains("\\begin{proof}[{\\flmProofTitle{Sketch of \\emph{proof}}}]"), "{output}");
        // A title beginning with a star that is no back-reference is a title.
        assert!(output.contains("\\begin{proof}[{\\flmProofTitle{*\\emph{x}}}]"), "{output}");
        let style = common::latex::style(&run).0;
        let default = if resolved { "\\proofname\\space (#1)" } else { "#1" };
        assert!(style.contains(&format!("\\ProvideDocumentCommand\\flmProofTitle{{m}}{{{default}}}\n")), "{style}");
    }
}

#[test]
fn the_resolved_back_reference_goes_through_the_proof_title_hook_whatever_the_class_heading_template() {
    // The class's heading template is FLM's heading; on LaTeX the heading is
    // LaTeX's, and the proof title hook is the one place a title joins it.
    let config = TheoremsConfig::new().class(
        "prooflike",
        TheoremClass::unnumbered()
            .closing_mark(Some(String::from("\u{25a1}")))
            .relation_ref(true)
            .heading("#{name}\\IfValueT{#{title}}{ of #{title}}"),
    );
    let (output, _) = written(&with_config(config), CASES, true);
    assert!(output.contains("\\begin{proof}[{\\flmProofTitle{\\flmRef{ref1}}}]"), "{output}");
    assert!(output.contains("\\begin{proof}\nRecorded only."), "{output}");
}

#[test]
fn with_the_label_hooks_off_the_back_reference_is_written_as_the_command_the_hook_runs() {
    let overrides = Overrides {
        render_options: OptionValues::new().with(LatexRecomposedRenderOptions {
            resolved_proof_refs: true,
            label_hooks: false,
            ..Default::default()
        }),
        ..Overrides::none()
    };
    let run = run_workflow(&std(), latex_recomposed::WORKFLOW, &[CASES], &overrides);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    let output = common::latex::output(&run);
    assert!(output.contains("\\begin{theorem}[My Theorem]\\label{ref1}\n"), "{output}");
    assert!(output.contains("\\begin{proof}[{\\flmProofTitle{\\zcref{ref1}}}]"), "{output}");
}

// ---------------------------------------------------------------------------------------------
// the declarations
// ---------------------------------------------------------------------------------------------

#[test]
fn the_kinds_the_document_uses_are_declared_guarded_in_the_authored_form() {
    let (_, style) = written(&std(), CASES, false);
    assert!(
        style.contains(
            "% --- definitions written from the configuration of FLM features ---\n% th@theoremlike (the theorems feature's, written from its configuration): declared unless \\th@theoremlike is already defined"
        ),
        "{style}"
    );
    for line in [
        numbered("theorem", "\\phfMakeTheorem[thmstyle=theoremlike,counter=phfthmcounter]{theorem}{Theorem}", "theorem"),
        declaration("proof", "\\phfMakeProofEnv[defaultproofname={Proof}]{proof}"),
        numbered("lemma", "\\phfMakeTheorem[thmstyle=theoremlike,counter=phfthmcounter]{lemma}{Lemma}", "lemma"),
        numbered(
            "conjecture",
            "\\phfMakeTheorem[thmstyle=theoremlike,counter=phfthmcounter,proofref=false]{conjecture}{Conjecture}",
            "conjecture",
        ),
    ] {
        assert_eq!(style.matches(&line).count(), 1, "{line}\n{style}");
    }
    // A kind the document does not use is not declared.
    assert!(!style.contains("{definition}"), "{style}");
}

#[test]
fn the_kinds_the_document_uses_are_declared_guarded_in_the_resolved_form() {
    let (_, style) = written(&std(), CASES, true);
    for line in [
        numbered("theorem", "\\begingroup\\theoremstyle{theoremlike}\\newtheorem{theorem}[flmtheorem]{Theorem}\\endgroup", "flmtheorem"),
        declaration("proof", "\\newenvironment{proof}[1][{Proof}]{\\begin{proof}[{#1}]}{\\end{proof}}"),
        numbered("lemma", "\\begingroup\\theoremstyle{theoremlike}\\newtheorem{lemma}[flmtheorem]{Lemma}\\endgroup", "flmtheorem"),
        numbered("conjecture", "\\begingroup\\theoremstyle{theoremlike}\\newtheorem{conjecture}[flmtheorem]{Conjecture}\\endgroup", "flmtheorem"),
        declaration("c@flmtheorem", "\\newcounter{flmtheorem}"),
    ] {
        assert_eq!(style.matches(&line).count(), 1, "{line}\n{style}");
    }
}

#[test]
fn the_shared_counter_is_declared_before_the_kinds_counting_in_it() {
    // The shared counter is a LaTeX counter of the feature's own,
    // `flmtheorem`, declared once, guarded on its register, before the first
    // kind counting in it; every such kind is declared over it alike, and no
    // other kind is declared for it.
    let counter = declaration("c@flmtheorem", "\\newcounter{flmtheorem}");
    let (_, style) = written(&std(), "\\begin{definition}\nD.\n\\end{definition}\n", true);
    let definition = style
        .find(&numbered(
            "definition",
            "\\begingroup\\theoremstyle{definitionlike}\\newtheorem{definition}[flmtheorem]{Definition}\\endgroup",
            "flmtheorem",
        ))
        .expect(&style);
    assert!(style.find(&counter).expect(&style) < definition, "{style}");
    assert_eq!(style.matches("\\newtheorem{").count(), 1, "{style}");
    // A host's kind counting in a counter of its own makes its own counter,
    // of its own name, and the shared counter is declared once for the
    // kinds counting in it, whatever they are.
    let document = "\\begin{lemma}\\noproofref\nL.\n\\end{lemma}\n\n\\begin{exercise}\nT.\n\\end{exercise}\n\n\\begin{remark}\nR.\n\\end{remark}\n";
    let (_, style) = written(&with_config(host_config()), document, true);
    assert_eq!(style.matches(&counter).count(), 1, "{style}");
    assert!(
        style.contains(&numbered("lemma", "\\begingroup\\theoremstyle{theoremlike}\\newtheorem{lemma}[flmtheorem]{Lemma}\\endgroup", "flmtheorem")),
        "{style}"
    );
    assert!(
        style.contains(&numbered("remark", "\\begingroup\\theoremstyle{definitionlike}\\newtheorem{remark}[flmtheorem]{Remark}\\endgroup", "flmtheorem")),
        "{style}"
    );
    assert!(
        style.contains(&declaration(
            "exercise",
            &format!(
                "\\begingroup\\theoremstyle{{exerciselike}}\\newtheorem{{exercise}}{{Exercise}}\\endgroup{}",
                typed_words("exercise", EXERCISE_WORDS, None, "exercise")
            )
        )),
        "{style}"
    );
    // In the authored form the package's own counter is the shared one.
    let (_, style) = written(&std(), "\\begin{definition}\nD.\n\\end{definition}\n", false);
    assert!(
        style.contains(&numbered(
            "definition",
            "\\phfMakeTheorem[thmstyle=definitionlike,counter=phfthmcounter,proofref=false]{definition}{Definition}",
            "definition",
        )),
        "{style}"
    );
    assert!(!style.contains("{theorem}"), "{style}");
}

/// The words of the host's kind `exercise`, which declares no lowercase
/// variant: a reference names it with its base words throughout.
const EXERCISE_WORDS: [&str; 4] = ["Exercise", "Exercises", "Exercise", "Exercises"];

#[test]
fn a_hosts_kinds_are_declared_from_their_configuration() {
    // A numbered class counting in counters of its own, and an unnumbered
    // kind of the proof-like class, its word lowered through the rules.
    let config = TheoremsConfig::new()
        .class("exerciselike", TheoremClass::numbered().shared_counter(false))
        .kind("exercise", TheoremKind::new("exerciselike", PrefixWords::new("Exercise", "Exercises")))
        .kind(
            "sketch",
            TheoremKind::new("prooflike", PrefixWords::new("Sketch \\& idea", "Sketches"))
                .expects_proof(false),
        );
    let document = "\\begin{exercise}\nE.\n\\end{exercise}\n\n\\begin{sketch}\nS.\n\\end{sketch}\n";
    let registry = with_config(config);
    let (_, style) = written(&registry, document, false);
    assert!(
        style.contains(&declaration(
            "exercise",
            &format!(
                "\\phfMakeTheorem[thmstyle=exerciselike,proofref=false]{{exercise}}{{Exercise}}{}",
                typed_words("exercise", EXERCISE_WORDS, None, "phfthm@exercise")
            )
        )),
        "{style}"
    );
    assert!(
        style.contains(&declaration("sketch", "\\phfMakeProofEnv[defaultproofname={Sketch \\& idea}]{sketch}")),
        "{style}"
    );
    let (_, style) = written(&registry, document, true);
    assert!(
        style.contains(&declaration(
            "exercise",
            &format!(
                "\\begingroup\\theoremstyle{{exerciselike}}\\newtheorem{{exercise}}{{Exercise}}\\endgroup{}",
                typed_words("exercise", EXERCISE_WORDS, None, "exercise")
            )
        )),
        "{style}"
    );
    assert!(
        style.contains(&declaration(
            "sketch",
            "\\newenvironment{sketch}[1][{Sketch \\& idea}]{\\begin{proof}[{#1}]}{\\end{proof}}"
        )),
        "{style}"
    );
    assert!(!style.contains("c@theorem"), "{style}");
}

/// The declaration of the theorem style named after the class `class`, with
/// the parameters of `amsthm`'s `plain` style.
fn class_style(class: &str) -> String {
    declaration(
        &format!("th@{class}"),
        &format!("\\newtheoremstyle{{{class}}}{{}}{{}}{{\\itshape}}{{}}{{\\bfseries}}{{.}}{{5pt plus 1pt minus 1pt}}{{}}"),
    )
}

#[test]
fn a_kind_is_set_in_the_theorem_style_of_its_class_name() {
    // Each class the document uses is declared a theorem style of its name,
    // once, before its first kind; the shipped `definitionlike` is declared
    // first in `amsthm`'s `definition` style by the feature's piece, which
    // every theorem requires.
    let config = TheoremsConfig::new()
        .class("exerciselike", TheoremClass::numbered().shared_counter(false))
        .kind("exercise", TheoremKind::new("exerciselike", PrefixWords::new("Exercise", "Exercises")));
    let document = "\\begin{lemma}\\noproofref\nL.\n\\end{lemma}\n\n\\begin{remark}\nR.\n\\end{remark}\n\n\\begin{exercise}\nE.\n\\end{exercise}\n\n\\begin{theorem}\\noproofref\nT.\n\\end{theorem}\n";
    for resolved in [false, true] {
        let (_, style) = written(&with_config(config.clone()), document, resolved);
        for class in ["theoremlike", "definitionlike", "exerciselike"] {
            let line = class_style(class);
            assert_eq!(style.matches(&line).count(), 1, "{line}\n{style}");
        }
        let piece = style
            .find("\\ifcsname th@definitionlike\\endcsname\\else\\newtheoremstyle{definitionlike}{}{}{\\normalfont}{}{\\bfseries}{.}{5pt plus 1pt minus 1pt}{}\\fi\n")
            .expect(&style);
        assert!(piece < style.find(&class_style("definitionlike")).unwrap(), "{style}");
        assert!(style.find(&class_style("exerciselike")).unwrap() < style.find("{exercise}{Exercise}").unwrap(), "{style}");
        // Nothing names a style of amsthm's own.
        for amsthm in ["{plain}", "=plain", "{definition}", "=definition,"] {
            assert!(!style.contains(amsthm), "{amsthm}\n{style}");
        }
    }
    // An unnumbered kind sets no style, and still requires the piece.
    let (_, style) = written(&std(), "\\begin{proof}\nP.\n\\end{proof}\n", true);
    assert!(!style.contains("newtheoremstyle{prooflike}"), "{style}");
    assert!(style.contains("% flm.latex.def.theorems\n"), "{style}");
    assert!(style.contains("% flm.latex.def.theorems.resolved\n"), "{style}");
}

// ---------------------------------------------------------------------------------------------
// the goldens the compile suite compiles (test 14)
// ---------------------------------------------------------------------------------------------

/// A document holding every kind of the default configuration, a labeled
/// theorem, a proof showing its back-reference and one recording it only,
/// `\noproofref`, a written title on a proof, and references to two
/// statements. Each statement is proved once: the `phfthm` package anchors a
/// proof at `proof:<label>`, so two proofs of one statement define that
/// label twice.
const EVERY_KIND: &str = "\\section{Theorems}

\\begin{theorem}[My Theorem]\\label{thm:a}
The square root of two is irrational.
\\end{theorem}

\\begin{proof}[*thm:a]
By contradiction.
\\end{proof}

\\begin{lemma}\\label{lem:b}
A lemma.
\\end{lemma}

\\begin{proof}[**lem:b]
Recorded only.
\\end{proof}

\\begin{proposition}\\noproofref
A proposition.
\\end{proposition}

\\begin{corollary}\\label{cor:c}\\noproofref
A corollary.
\\end{corollary}

\\begin{conjecture}
A conjecture.
\\end{conjecture}

\\begin{definition}\\label{def:d}
A definition.
\\end{definition}

\\begin{remark}
A remark.
\\end{remark}

\\begin{proof}[Sketch]
A sketch.
\\end{proof}

See \\ref{thm:a}, \\ref{lem:b} and \\ref{def:d}.
";

#[test]
fn every_kind_is_written_in_the_authored_form() {
    let run = recompose(&std(), EVERY_KIND, false);
    common::latex::assert_golden("theorems-authored.tex", &run);
}

#[test]
fn every_kind_is_written_in_the_resolved_form() {
    let run = recompose(&std(), EVERY_KIND, true);
    common::latex::assert_golden("theorems-resolved.tex", &run);
}

/// A host's configuration: a class counting in counters of its own, its
/// kind `exercise` declaring no lowercase words.
fn host_config() -> TheoremsConfig {
    TheoremsConfig::new()
        .class("exerciselike", TheoremClass::numbered().shared_counter(false))
        .kind("exercise", TheoremKind::new("exerciselike", PrefixWords::new("Exercise", "Exercises")))
}

/// A lemma, counting in the shared counter, and two exercises, counting in
/// a counter of their own, each referred to.
const HOST_KINDS: &str = "\\begin{lemma}\\label{lem:a}\\noproofref
A lemma.
\\end{lemma}

\\begin{exercise}\\label{x:a}
An exercise.
\\end{exercise}

\\begin{exercise}\\label{x:b}
Another.
\\end{exercise}

See \\ref{lem:a} and \\ref{x:b}.
";

#[test]
fn a_hosts_kinds_are_written_in_the_authored_form() {
    let run = recompose(&with_config(host_config()), HOST_KINDS, false);
    common::latex::assert_golden("theorems-host-authored.tex", &run);
}

#[test]
fn a_hosts_kinds_are_written_in_the_resolved_form() {
    let run = recompose(&with_config(host_config()), HOST_KINDS, true);
    common::latex::assert_golden("theorems-host-resolved.tex", &run);
}

/// The kind `theorem` in a class of its own, counting on its own, beside the
/// lemma and the proposition, which share the counter.
fn theorems_on_their_own() -> TheoremsConfig {
    TheoremsConfig::new()
        .class("own", TheoremClass::numbered().shared_counter(false))
        .kind("theorem", TheoremKind::from_word("own", "theorem"))
}

/// Two theorems, a lemma and a proposition, interleaved, each referred to:
/// the theorems read 1 and 2, the lemma 1 and the proposition 2.
const ON_THEIR_OWN: &str = "\\begin{theorem}\\label{thm:a}
A theorem.
\\end{theorem}

\\begin{lemma}\\label{lem:b}\\noproofref
A lemma.
\\end{lemma}

\\begin{theorem}\\label{thm:c}
Another theorem.
\\end{theorem}

\\begin{proposition}\\label{prop:d}\\noproofref
A proposition.
\\end{proposition}

See \\ref{thm:a}, \\ref{lem:b}, \\ref{thm:c} and \\ref{prop:d}.
";

#[test]
fn the_kind_theorem_on_its_own_is_written_in_the_authored_form() {
    let run = recompose(&with_config(theorems_on_their_own()), ON_THEIR_OWN, false);
    common::latex::assert_golden("theorems-own-authored.tex", &run);
}

#[test]
fn the_kind_theorem_on_its_own_is_written_in_the_resolved_form() {
    let run = recompose(&with_config(theorems_on_their_own()), ON_THEIR_OWN, true);
    common::latex::assert_golden("theorems-own-resolved.tex", &run);
}

// ---------------------------------------------------------------------------------------------
// a theorem in a kept user definition (test 20)
// ---------------------------------------------------------------------------------------------

/// The standard package with `definitions` as its user definitions.
fn with_definitions(definitions: Vec<Definition>) -> Arc<FlmRegistry> {
    common::stdcore::registry(
        StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)),
    )
}

#[test]
fn a_theorem_in_a_kept_template_is_written_as_the_environment() {
    let claim = Definition::environment("claim", vec![ArgDef::mandatory("title")])
        .content("\\begin{theorem}[#1]\n#{body}\n\\end{theorem}")
        .expand(ExpandPolicy::PerPlan);
    let registry = with_definitions(vec![claim]);
    let document = "\\begin{claim}{Main}\nEvery claim holds.\n\\end{claim}\n";
    for resolved in [false, true] {
        let (output, style) = written(&registry, document, resolved);
        assert!(output.contains("\\begin{flmUEclaim}{Main}\nEvery claim holds.\n\\end{flmUEclaim}"), "{output}");
        assert!(style.contains("\\begin{theorem}[#1]\n#2\n\\end{theorem}"), "{style}");
        let body = if resolved {
            "\\begingroup\\theoremstyle{theoremlike}\\newtheorem{theorem}[flmtheorem]{Theorem}\\endgroup"
        } else {
            "\\phfMakeTheorem[thmstyle=theoremlike,counter=phfthmcounter]{theorem}{Theorem}"
        };
        let counter = if resolved { "flmtheorem" } else { "theorem" };
        assert!(style.contains(&numbered("theorem", body, counter)), "{style}");
    }
}

#[test]
fn a_back_reference_a_call_completes_expands_the_use_site() {
    // The statement a proof names is completed by the call: the definition
    // cannot write it, so each use site is expanded, and the proof it holds
    // is written under the key of the statement that use site names.
    let proves = Definition::environment("proves", vec![ArgDef::mandatory("target")])
        .content("\\begin{proof}[*#1]\n#{body}\n\\end{proof}")
        .expand(ExpandPolicy::PerPlan);
    let registry = with_definitions(vec![proves]);
    // The lemma is marked `\noproofref`: its proof stands in the use site of
    // a user definition the plan keeps, where the check, which runs before
    // the use site is expanded at lowering, does not see it, and would report
    // the lemma as having no proof. The mark keeps this test about the
    // recomposition alone; the check's limit is its own tests' business.
    let document = "\\begin{lemma}\\label{lem:x}\\noproofref\nL.\n\\end{lemma}\n\n\\begin{proves}{lem:x}\nBy hand.\n\\end{proves}\n";
    for (resolved, expected) in [(false, "\\begin{proof}[*ref1]"), (true, "\\begin{proof}[{\\flmProofTitle{\\flmRef{ref1}}}]")] {
        let (output, style) = written(&registry, document, resolved);
        assert!(output.contains(expected), "{output}");
        assert!(!output.contains("flmUEproves"), "{output}");
        assert!(!style.contains("flmUEproves"), "{style}");
    }
}

#[test]
fn a_literal_back_reference_in_a_kept_template_is_written_in_the_template() {
    // The statement a proof names is written in the template itself, so the
    // definition is kept and the back-reference is written in its body, under
    // the key the references feature gave the label (through the names of
    // the value spliced at lowering), in the form the render option chooses.
    // The statement is of a kind expecting no proof, so that the check, which
    // does not see a proof inside a kept definition, has nothing to report.
    let proves = Definition::environment("byhand", vec![])
        .content("\\begin{proof}[*def:a]\n#{body}\n\\end{proof}")
        .expand(ExpandPolicy::PerPlan);
    let registry = with_definitions(vec![proves]);
    let document = "\\begin{definition}\\label{def:a}\nD.\n\\end{definition}\n\n\\begin{byhand}\nBy hand.\n\\end{byhand}\n";
    for (resolved, written_form) in
        [(false, "\\begin{proof}[*ref1]\n"), (true, "\\begin{proof}[{\\flmProofTitle{\\flmRef{ref1}}}]\n")]
    {
        let (output, style) = written(&registry, document, resolved);
        assert!(output.contains("\\begin{definition}\\flmLabels{\\label{ref1}}\n"), "{output}");
        assert!(output.contains("\\begin{flmUEbyhand}\n"), "{output}");
        assert!(
            style.contains(&format!("\\ProvideDocumentEnvironment{{flmUEbyhand}}{{+b}}{{{written_form}#1\n\\end{{proof}}}}{{}}")),
            "{style}"
        );
    }
}
