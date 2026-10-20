//! The `theorems` feature end to end: one document holding every kind of the
//! richest shipped set and a host's kind with a variant of its own naming,
//! rendered by the standard package's four workflows (`check`, `text`,
//! `html`, `latex-recomposed`), the recomposed LaTeX under both values of the
//! render option `resolved_proof_refs`. The document holds a statement with a
//! title and a label, a proof back-referencing a lemma with one star and one
//! with two, a statement marked `\noproofref`, a theorem with an empty body
//! and one whose body opens with a list, a theorem inside a list item and
//! inside a footnote, a theorem inside a user definition's template that the
//! recomposed LaTeX keeps as a LaTeX definition, and references to theorems,
//! one of them to two statements.
//!
//! It is rendered under two numberings: the default one, where every
//! numbered kind counts in one sequence through the document, and the one a
//! host asks for with the numbering feature's document option, numbering
//! theorems within sections (`2.3`). The recomposed LaTeX leaves numbers to
//! LaTeX and does not write that hierarchy, so it is the same under both,
//! which a test holds.
//!
//! A second, short document holds references asking for a wording of a
//! kind's words — `\ref[abbreviated]`, `\ref[lowercase]`, `\ref[capital]`,
//! `\ref[S]` and the variant of the host's kind. Its recomposed LaTeX is
//! compiled, and what each reference prints read, by the compile suite: the
//! reference hook's default translates the shipped wordings and drops the
//! host's, which prints the default wording.
//!
//! A third, short document holds the cases a run reports: a theorem no
//! proof refers back to (a Warning), a proof whose back-reference is
//! malformed (an Error) and one whose back-reference names nothing. Each is
//! reported once per workflow, by `check` and by every render alike. Its
//! recomposed LaTeX is not compiled: the reference to nothing would stop the
//! compile suite's check for undefined references (`NOT_COMPILED` in
//! `tests/latex_compile.rs`).
//!
//! Each run is a golden: the text under `tests/goldens/text/`, the page under
//! `tests/goldens/html/` (parsed by `tests/html_validity.rs`), the recomposed
//! LaTeX and its style file under `tests/goldens/latex/` (compiled by
//! `tests/latex_compile.rs` under `pdflatex` and `lualatex`, with and without
//! `hyperref`, when `FLM_TESTS_LATEX_COMPILE` is set), and every run's
//! diagnostics under `tests/goldens/stdcore/`. The last test runs each
//! workflow sequentially and in parallel and compares every table the runs
//! wrote; the full battery runs this suite a third time on a pool of one
//! thread (`RAYON_NUM_THREADS=1`), against the same goldens.

mod common;

use std::fmt::Write as _;
use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::golden::assert_text_golden;
use flm_core::dump::project::{ProjectOptions, Projector};
use flm_core::dump::to_json_pretty;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::{Definition, ExpandPolicy};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::latex_recomposed::{self, LatexRecomposedRenderOptions};
use flm_core::stdcore::backends::{html, text};
use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};
use flm_core::stdcore::features::numbering::{NumberingOptions, PluralWords, PrefixWords, Within};
use flm_core::stdcore::features::theorems::{TheoremKind, TheoremsConfig};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig};

// ---------------------------------------------------------------------------------------------
// the documents
// ---------------------------------------------------------------------------------------------

/// The document of every kind and every place. Every statement expecting a
/// proof has one, or is marked `\noproofref`, so that the runs report
/// nothing. The environment `keptthm` is [`definitions`]'s.
const DOCUMENT: &str = "\
\\section{Statements}

\\begin{theorem}[Main result]\\label{thm:main}
Every shipped kind is written in this document.
\\end{theorem}

\\begin{proof}[*thm:main]
By \\ref{lem:key} and \\ref{lem:aux}; together, \\ref{thm:main,lem:key}.
\\end{proof}

\\begin{lemma}[Key lemma]\\label{lem:key}
A lemma proved below, its proof naming it.
\\end{lemma}

\\begin{proof}[*lem:key]
One star shows the statement proved.
\\end{proof}

\\begin{lemma}\\label{lem:aux}
A lemma whose proof records the relation without showing it.
\\end{lemma}

\\begin{proof}[**lem:aux]
Two stars record the relation only.
\\end{proof}

\\begin{proposition}\\label{prop:open}\\noproofref
A proposition whose proof is not coming.
\\end{proposition}

\\begin{corollary}\\label{cor:empty}
\\end{corollary}

\\begin{proof}[*cor:empty]
The corollary above has an empty body.
\\end{proof}

\\begin{conjecture}\\label{thm:conj}
A conjecture expects no proof.
\\end{conjecture}

\\section{Definitions and the other kinds}

\\begin{definition}[Opening with a list]\\label{def:list}
\\begin{itemize}
\\item a body opening with a list gives the heading a paragraph of its own;
\\item and the list follows it.
\\end{itemize}
\\end{definition}

\\begin{remark}\\label{rem:r}
A remark.
\\end{remark}

\\begin{idea}
An idea.
\\end{idea}

\\begin{question}
A question?
\\end{question}

\\begin{claim}
A claim.
\\end{claim}

\\begin{observation}
An observation.
\\end{observation}

\\begin{problem}
A problem.
\\end{problem}

\\begin{exercise}[Host's kind]\\label{x:ex}
An exercise, a kind the host configures.
\\end{exercise}

See \\ref{x:ex}, \\ref{def:list} and \\ref{rem:r}.

\\begin{itemize}
\\item \\begin{lemma}\\label{lem:item}
A lemma in a list item.
\\end{lemma}
\\item \\begin{proof}[*lem:item]
Its proof in the next item.
\\end{proof}
\\end{itemize}

A note holds a remark.\\footnote{\\begin{remark}\\label{rem:note}
A remark in a footnote.
\\end{remark}} See \\ref{rem:note} and \\ref{lem:item}.

\\begin{keptthm}{Kept}
A theorem inside a user definition's template.
\\end{keptthm}";

/// The document of the references asking for a wording of the kind's words:
/// `\ref[abbreviated]`, `\ref[lowercase]`, `\ref[capital]`, and the variant
/// `short` the host's kind declares. The recomposed LaTeX writes each option
/// back as written, and the reference hook's default translates the shipped
/// names and drops `short`; `tests/latex_compile.rs` reads what each prints.
const VARIANTS: &str = "\
\\begin{lemma}\\label{lem:v}\\noproofref
A lemma.
\\end{lemma}

\\begin{exercise}\\label{x:v}
An exercise.
\\end{exercise}

\\ref[abbreviated]{lem:v}, \\ref[lowercase]{lem:v}, \\ref[capital]{lem:v}, \\ref[S]{lem:v} and \\ref[short]{x:v}.";

/// The document of the cases a run reports.
const REPORTED: &str = "\
\\begin{theorem}\\label{thm:alone}
A theorem no proof refers back to.
\\end{theorem}

\\begin{proof}[*nolabel]
A back-reference whose text is no label: its title is shown as written.
\\end{proof}

\\begin{proof}[*thm:missing]
A back-reference naming nothing.
\\end{proof}";

// ---------------------------------------------------------------------------------------------
// the configuration and the cases
// ---------------------------------------------------------------------------------------------

/// The user definition [`DOCUMENT`] calls: `keptthm`, an environment whose
/// template holds a theorem marked `\noproofref`, kept on the recomposed
/// LaTeX as a LaTeX definition and expanded on every other output.
fn definitions() -> Vec<Definition> {
    vec![
        Definition::environment("keptthm", vec![ArgDef::mandatory("title")])
            .content("\\begin{theorem}[#1]\\noproofref\n#{body}\n\\end{theorem}")
            .expand(ExpandPolicy::PerPlan),
    ]
}

/// The standard package: the richest shipped set of kinds and the host's
/// kind `exercise`, in the class `definitionlike`, with a variant of its own
/// naming (`short`); numbered headings; [`definitions`].
fn registry() -> Arc<FlmRegistry> {
    let exercise = TheoremKind::new(
        "definitionlike",
        PrefixWords::new("Exercise", "Exercises").with_variant("short", PluralWords::new("Ex.", "Exs.")),
    );
    common::stdcore::registry(
        StdcoreConfig::new()
            .theorems(TheoremsConfig::richset().kind("exercise", exercise))
            .headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered))
            .userdefs(UserdefsConfig::new().definitions(definitions())),
    )
}

/// One document under one numbering, with the stem of its goldens.
struct Case {
    /// The stem of the goldens: `theorems-e2e-<name>`.
    name: &'static str,
    /// The document.
    document: &'static str,
    /// The document's option values: the numbering feature's, or none.
    options: fn() -> OptionValues,
}

/// No document option.
fn no_options() -> OptionValues {
    OptionValues::new()
}

/// The numbering feature's document option numbering theorems within
/// sections.
fn within_sections() -> OptionValues {
    OptionValues::new().with(
        NumberingOptions::new().within("flm.theorems.theorem", Within::parent("flm.headings.section")),
    )
}

/// [`DOCUMENT`] under the default numbering and within sections, then
/// [`VARIANTS`] and [`REPORTED`].
fn cases() -> Vec<Case> {
    vec![
        Case { name: "default", document: DOCUMENT, options: no_options },
        Case { name: "within", document: DOCUMENT, options: within_sections },
        Case { name: "variants", document: VARIANTS, options: no_options },
        Case { name: "reported", document: REPORTED, options: no_options },
    ]
}

// ---------------------------------------------------------------------------------------------
// runs and their goldens
// ---------------------------------------------------------------------------------------------

/// One run of a case: a workflow, and for the recomposed LaTeX the value of
/// `resolved_proof_refs`.
#[derive(Clone, Copy)]
struct Output {
    /// The workflow.
    workflow: &'static str,
    /// The value of the render option `resolved_proof_refs` (read by the
    /// recomposed LaTeX alone).
    resolved: bool,
    /// The name of the run in the goldens.
    label: &'static str,
}

/// The five runs of a case, in the order the diagnostics golden lists them.
const OUTPUTS: [Output; 5] = [
    Output { workflow: CHECK_WORKFLOW, resolved: false, label: "check" },
    Output { workflow: text::WORKFLOW, resolved: false, label: "text" },
    Output { workflow: html::WORKFLOW, resolved: false, label: "html" },
    Output { workflow: latex_recomposed::WORKFLOW, resolved: false, label: "latex-recomposed, authored" },
    Output { workflow: latex_recomposed::WORKFLOW, resolved: true, label: "latex-recomposed, resolved" },
];

/// The overrides of `output`: the recomposed LaTeX's render option.
fn overrides(output: Output) -> Overrides {
    Overrides {
        render_options: OptionValues::new().with(LatexRecomposedRenderOptions {
            resolved_proof_refs: output.resolved,
            ..Default::default()
        }),
        ..Overrides::none()
    }
}

/// The run of `output` over the case's document under `options`.
fn run(registry: &Arc<FlmRegistry>, case: &Case, output: Output, options: &RunOptions) -> Run {
    let doc = common::pipeline::doc_with_options(registry, &[case.document], (case.options)());
    let plan = common::stdcore::plan(registry, output.workflow, &overrides(output));
    common::pipeline::run_with(&doc, &plan, Externals::none(), options)
}

#[test]
fn every_workflow_writes_the_document_as_its_golden_under_each_numbering() {
    let registry = registry();
    for case in cases() {
        let mut diagnostics = String::new();
        for output in OUTPUTS {
            let run = run(&registry, &case, output, &RunOptions::sequential());
            writeln!(diagnostics, "===== {} =====", output.label).expect("writing to a string");
            diagnostics.push_str(&common::stdcore::diagnostic_lines(&run));
            let form = if output.resolved { "resolved" } else { "authored" };
            match output.workflow {
                text::WORKFLOW => assert_text_golden(
                    common::lowering::golden_path(&format!("theorems-e2e-{}.txt", case.name)),
                    &common::lowering::text(&run),
                ),
                html::WORKFLOW => common::html::assert_golden(
                    &format!("theorems-e2e-{}.page.html", case.name),
                    &common::html::page(&run),
                ),
                // The hierarchy of the numbering is not written on the
                // recomposed LaTeX: `within` writes what `default` writes,
                // which the next test holds, so it has no golden of its own.
                latex_recomposed::WORKFLOW if case.name != "within" => common::latex::assert_golden(
                    &format!("theorems-e2e-{}-{form}.tex", case.name),
                    &run,
                ),
                _ => {}
            }
        }
        assert_text_golden(
            common::stdcore::golden_path(&format!("theorems-e2e-{}-diagnostics.txt", case.name)),
            &diagnostics,
        );
    }
}

#[test]
fn the_recomposed_latex_does_not_write_the_numbering_within_sections() {
    let registry = registry();
    let [default, within, ..] = <[Case; 4]>::try_from(cases()).ok().expect("four cases");
    for output in &OUTPUTS[3..] {
        let one = run(&registry, &default, *output, &RunOptions::sequential());
        let other = run(&registry, &within, *output, &RunOptions::sequential());
        assert_eq!(common::latex::golden_text(&one), common::latex::golden_text(&other), "{}", output.label);
    }
}

#[test]
fn the_reported_cases_are_reported_once_by_every_workflow() {
    let registry = registry();
    let case = cases().into_iter().find(|case| case.name == "reported").expect("the reported case");
    for output in OUTPUTS {
        let run = run(&registry, &case, output, &RunOptions::sequential());
        let count = |identifier: &str| {
            run.diagnostics().iter().filter(|d| d.identifier() == identifier).count()
        };
        for identifier in
            ["flm.theorems.missing-proof", "flm.theorems.bad-relation-ref", "flm.refs.unresolved"]
        {
            assert_eq!(count(identifier), 1, "{identifier} under {}", output.label);
        }
    }
}

/// Everything `run` wrote: every table of the store through its projection,
/// node references resolved through the run's own frozen trees (so that two
/// runs of one document compare equal), then every diagnostic by severity,
/// identifier and span.
fn snapshot(run: &Run) -> String {
    let registry = run.plan().registry();
    let projector = Projector::new(ProjectOptions::new());
    let mut out = String::new();
    for id in run.computed().ids() {
        let entry = registry.table(id);
        let value = run.computed().get_erased(id).expect("a listed table is present");
        let projected = entry.def.ty.project(value.as_ref(), &projector.table_projection(run));
        let json = to_json_pretty(&projected).unwrap_or_else(|_| format!("{projected:?}"));
        writeln!(out, "{} = {json}", entry.def.name).expect("writing to a string");
    }
    for diagnostic in run.diagnostics().iter() {
        writeln!(
            out,
            "{:?} {} {:?}",
            diagnostic.severity(),
            diagnostic.identifier(),
            diagnostic.span().range()
        )
        .expect("writing to a string");
    }
    out
}

#[test]
fn every_workflow_writes_the_same_tables_sequentially_and_in_parallel() {
    let registry = registry();
    for case in cases() {
        let doc = common::pipeline::doc_with_options(&registry, &[case.document], (case.options)());
        for output in OUTPUTS {
            let plan = common::stdcore::plan(&registry, output.workflow, &overrides(output));
            let under = |options: &RunOptions| {
                snapshot(&common::pipeline::run_with(&doc, &plan, Externals::none(), options))
            };
            let sequential = under(&RunOptions::sequential());
            let parallel = under(&RunOptions::new());
            assert_eq!(parallel, sequential, "{} under {}", case.name, output.label);
            assert!(
                sequential.contains("flm.refs.resolutions = "),
                "the snapshot holds the resolutions of the references ({} under {})",
                case.name,
                output.label
            );
        }
    }
}
