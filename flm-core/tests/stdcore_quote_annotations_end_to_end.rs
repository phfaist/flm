//! The `quote` and `annotations` features end to end: one document holding
//! every quote environment — `quote`, `blockquote`, `address` and a
//! configured one, `verse` (an attribution section beside bare lines, a name
//! LaTeX's standard classes define) — every section, the lines sub-grammar
//! with its line breaks and indents, and every annotation form (`\phf{…}`,
//! `\phf … \endphf`, `\phf[…]`) of two annotators, one with initials and one
//! without, in every place inline and block content may stand: a heading's
//! title, a list item, a footnote, a `\text{…}` in a formula, a `\block`
//! section, a line of a lines section, an attribution, and a user
//! definition's template; an annotation cut across a paragraph break (a
//! comment and a highlight), one around a list, and cuts inside a list item,
//! inside a `\block` section and inside a footnote; a quotation inside a list
//! item and a list inside a quotation.
//!
//! It is rendered by the standard package's four workflows (`check`, `text`,
//! `html`, `latex-recomposed`), shown and, on the three renders, hidden (the
//! render option of the annotations feature). A second, short document holds
//! the cases a run reports: bare content in a `quote`, a section inside a
//! section, an indent after the start of its line, an annotation standing
//! between a quotation's body and a section command (`\phf{\text{x}}`: the
//! annotation is bare content, the `\text` a section inside it), a passage
//! the input ends before closing, one an enclosing environment ends, and a
//! cut inside a heading's title. The quotation's three conditions are
//! reported at lowering, so `check` reports none of them, which its golden
//! shows.
//!
//! Each run is a golden: the text under `tests/goldens/text/`, the page under
//! `tests/goldens/html/` (parsed by `tests/html_validity.rs`), the recomposed
//! LaTeX and its style file under `tests/goldens/latex/` (compiled by
//! `tests/latex_compile.rs` under `pdflatex` and `lualatex`, with and without
//! `hyperref`, when `FLM_TESTS_LATEX_COMPILE` is set; the reported document's
//! is not compiled, its passages being left open), and every run's
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
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::annotations::{AnnotationsConfig, AnnotationsRenderOptions, Annotator};
use flm_core::stdcore::features::quote::{QuoteConfig, QuoteEnvironment, QuoteSection};
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig};

// ---------------------------------------------------------------------------------------------
// the documents
// ---------------------------------------------------------------------------------------------

/// The document of every environment, every section, every form and every
/// place. It reports nothing. The macro `\todo` is [`definitions`]'s.
const DOCUMENT: &str = "\
\\section{Quotations \\phf{marked} \\phf[a remark in a title]}

\\begin{quote}
\\text{To be or not to be.}
\\lines{Line one\\\\
\\indent Line two, \\phf{marked}\\\\
\\indent\\indent Line three}
\\attributed{Shakespeare \\abc[check the source]}
\\block{A block section holding \\phf{a highlight} and \\abc[a remark].

A second paragraph with a comment cut \\phf[across a

paragraph break] and a list:
\\begin{itemize}
\\item one
\\item two, \\phf{marked}
\\end{itemize}}
\\end{quote}

\\begin{blockquote}
A blockquote's first paragraph, \\phf{marked}.

Its second paragraph, before a list:
\\begin{enumerate}
\\item an item
\\end{enumerate}
\\end{blockquote}

\\begin{address}
123 Main St\\\\
\\indent City, ST 12345
\\end{address}

\\begin{verse}
Roses are red,\\\\
\\indent violets are blue.
\\attributed{Anonymous}
\\end{verse}

Some text \\phf[A comment.

With multiple paragraphs.] More text.

Some text \\phf A highlight

across a paragraph break,\\endphf{} then \\abc{a highlight of abc's} and \\phf{x}[y].

\\phf see \\begin{itemize}\\item a list in a passage\\end{itemize} here \\endphf

\\begin{itemize}
\\item An entry \\phf{marked} and \\phf[remarked].
\\item An entry holding a quotation:
\\begin{quote}\\text{Quoted in a list.}\\end{quote}
\\item An entry with a comment \\phf[cut

across a break] in it.
\\end{itemize}

A footnote.\\footnote{Holding \\phf{a passage}, \\abc[a remark], and \\phf[a comment

cut in the note].}

A formula \\(x = y \\text{ where \\phf{y} is marked}\\) in the text.

A user definition's template: \\todo{check this}.";

/// The document of the cases a run reports.
const REPORTED: &str = "\
\\begin{quote}
Bare content in a quote.
\\text{A passage.}
\\end{quote}

\\begin{quote}
\\lines{a \\text{section inside a section}\\\\
b \\indent after the start of its line}
\\end{quote}

\\begin{quote}\\phf{\\text{x}}\\end{quote}

\\section{A title \\phf{cut

here}}

\\begin{itemize}
\\item \\phf a passage its environment ends
\\end{itemize}

\\phf a passage the input ends";

// ---------------------------------------------------------------------------------------------
// the configuration and the cases
// ---------------------------------------------------------------------------------------------

/// The user definition [`DOCUMENT`] calls: `\todo{…}`, a comment of `phf`,
/// kept on the recomposed LaTeX as a LaTeX definition and expanded on every
/// other output.
fn definitions() -> Vec<Definition> {
    vec![Definition::macro_("todo", vec![ArgDef::mandatory("text")]).text("\\phf[#1]")]
}

/// The standard package: the three shipped quote environments and `verse`
/// (an attribution section, bare content read as lines); two annotators,
/// `phf` with the initials `PhF` (annotator 0) and `abc` with none
/// (annotator 1); [`definitions`].
fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(
        StdcoreConfig::new()
            .quote(QuoteConfig::new().environment(
                "verse",
                QuoteEnvironment::new([QuoteSection::Attributed]).bare(QuoteSection::Lines),
            ))
            .annotations(
                AnnotationsConfig::new()
                    .annotator(Annotator::new("phf").initials("PhF"))
                    .annotator(Annotator::new("abc")),
            )
            .userdefs(UserdefsConfig::new().definitions(definitions())),
    )
}

/// One document under one value of the render option `hidden`, with the stem
/// of its goldens.
struct Case {
    /// The stem of the goldens: `quote-annotations-e2e-<name>`.
    name: &'static str,
    /// The document.
    document: &'static str,
    /// The value of the annotations feature's render option `hidden`.
    hidden: bool,
}

/// [`DOCUMENT`] shown and hidden, then [`REPORTED`].
fn cases() -> Vec<Case> {
    vec![
        Case { name: "shown", document: DOCUMENT, hidden: false },
        Case { name: "hidden", document: DOCUMENT, hidden: true },
        Case { name: "reported", document: REPORTED, hidden: false },
    ]
}

// ---------------------------------------------------------------------------------------------
// runs and their goldens
// ---------------------------------------------------------------------------------------------

/// The four workflows, in the order the diagnostics golden lists them.
const WORKFLOWS: [&str; 4] = [CHECK_WORKFLOW, text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW];

/// The overrides of `case`: the annotations feature's render option.
fn overrides(case: &Case) -> Overrides {
    Overrides {
        render_options: OptionValues::new().with(AnnotationsRenderOptions { hidden: case.hidden }),
        ..Overrides::none()
    }
}

/// The run of `workflow` over the case's document.
fn run(registry: &Arc<FlmRegistry>, case: &Case, workflow: &str, options: &RunOptions) -> Run {
    let doc = common::pipeline::doc_with_options(registry, &[case.document], OptionValues::new());
    let plan = common::stdcore::plan(registry, workflow, &overrides(case));
    common::pipeline::run_with(&doc, &plan, Externals::none(), options)
}

#[test]
fn every_workflow_writes_each_case_as_its_golden() {
    let registry = registry();
    for case in cases() {
        let mut diagnostics = String::new();
        for workflow in WORKFLOWS {
            // Hiding is a render option: `check` renders nothing, so the
            // hidden case has no `check` run of its own.
            if case.hidden && workflow == CHECK_WORKFLOW {
                continue;
            }
            let run = run(&registry, &case, workflow, &RunOptions::sequential());
            writeln!(diagnostics, "===== {workflow} =====").expect("writing to a string");
            diagnostics.push_str(&common::stdcore::diagnostic_lines(&run));
            match workflow {
                text::WORKFLOW => assert_text_golden(
                    common::lowering::golden_path(&format!("quote-annotations-e2e-{}.txt", case.name)),
                    &common::lowering::text(&run),
                ),
                html::WORKFLOW => common::html::assert_golden(
                    &format!("quote-annotations-e2e-{}.page.html", case.name),
                    &common::html::page(&run),
                ),
                latex_recomposed::WORKFLOW => common::latex::assert_golden(
                    &format!("quote-annotations-e2e-{}.tex", case.name),
                    &run,
                ),
                _ => {}
            }
        }
        assert_text_golden(
            common::stdcore::golden_path(&format!("quote-annotations-e2e-{}-diagnostics.txt", case.name)),
            &diagnostics,
        );
    }
}

#[test]
fn the_shown_document_is_reported_nothing_by_any_workflow() {
    let registry = registry();
    for case in cases().iter().filter(|case| case.name != "reported") {
        for workflow in WORKFLOWS {
            let run = run(&registry, case, workflow, &RunOptions::sequential());
            assert_eq!(
                run.diagnostics().iter().count(),
                0,
                "{} under {workflow}: {}",
                case.name,
                run.diagnostics().render_all()
            );
        }
    }
}

#[test]
fn the_reported_cases_are_reported_by_every_render_and_the_parser_s_by_check_too() {
    let registry = registry();
    let case = cases().into_iter().find(|case| case.name == "reported").expect("the reported case");
    for workflow in WORKFLOWS {
        let run = run(&registry, &case, workflow, &RunOptions::sequential());
        let count = |identifier: &str| {
            run.diagnostics().iter().filter(|d| d.identifier() == identifier).count()
        };
        // The annotations feature's one Error is the parser's: every
        // workflow reports it, once per passage left open.
        assert_eq!(count("flm.annotations.unterminated"), 2, "under {workflow}");
        // The quotation's three conditions are the lowering rule's: every
        // render reports them, `check` none (the inventory of what `check`
        // does not see).
        let lowered = if workflow == CHECK_WORKFLOW { 0 } else { 1 };
        assert_eq!(count("flm.quote.nested-section"), 2 * lowered, "under {workflow}");
        assert_eq!(count("flm.quote.indent-mid-line"), lowered, "under {workflow}");
        assert_eq!(count("flm.quote.bare-content"), 2 * lowered, "under {workflow}");
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
        for workflow in WORKFLOWS {
            let under = |options: &RunOptions| snapshot(&run(&registry, &case, workflow, options));
            let sequential = under(&RunOptions::sequential());
            let parallel = under(&RunOptions::new());
            assert_eq!(parallel, sequential, "{} under {workflow}", case.name);
        }
    }
}
