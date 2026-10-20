//! The `citations` feature end to end: one document citing in every place
//! inline content stands — a paragraph, a footnote, a list item, a heading's
//! title that a reference shows, a theorem's title and its statement, a
//! quotation's passage and a line of its `\lines`, a float's caption, a cell
//! of a table and an entry of a `\celldata` — with notes, a chunk of several
//! calls, `\R` in its three forms (plain, `[S]`, `[I]`), and a key no provider
//! answers. The entries come from a test provider answering from a map, each
//! entry FLM source holding `\emph`, a link or a formula.
//!
//! It is rendered by the standard package's four workflows (`check`, `text`,
//! `html`, `latex-recomposed`) under four settings, each a run of its own: the
//! listed placement with bracketed marks (the default), the listed placement
//! with raised marks, the in-place placement, and the default settings under
//! the render option `root_heading_level` set to 2, where the document's
//! subsections are its first-level headings and its section is above the
//! root. The recomposed LaTeX is the same under every setting (the placement,
//! the form and the root change nothing there), so one golden pins it and
//! the other runs are compared with it.
//!
//! Each run is a golden: the text under `tests/goldens/text/`, the page under
//! `tests/goldens/html/` (parsed by `tests/html_validity.rs`), the recomposed
//! LaTeX and its style file under `tests/goldens/latex/` (compiled by
//! `tests/latex_compile.rs` under `pdflatex` and `lualatex`, plain and with
//! `hyperref`, with a `thebibliography` appended that defines the cited keys,
//! when `FLM_TESTS_LATEX_COMPILE` is set), and every run's diagnostics under
//! `tests/goldens/stdcore/`. The key no provider answers is an Error of the
//! check stage, which every workflow runs, `check` included; the heading above
//! the root is an Error of the two renders that choose a heading's tag, which
//! `check` does not see. The last test runs each workflow sequentially and in
//! parallel and compares every table the runs wrote.

mod common;

use std::fmt::Write as _;
use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::golden::assert_text_golden;
use flm_core::dump::project::{ProjectOptions, Projector};
use flm_core::dump::to_json_pretty;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::citations::{CitationPlacement, CitationsConfig, MarkForm};
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig};

// ---------------------------------------------------------------------------------------------
// the document and its entries
// ---------------------------------------------------------------------------------------------

/// The document citing in every place. It reports one Error under the
/// default root: the key `unknown`, which no provider answers.
const DOCUMENT: &str = "\
\\section{Results \\cite{shor95}}\\label{sec:results}

A paragraph citing \\cite{knuth84}, then a chunk of several calls \
\\cite{lamport94}\\cite[Thm 4]{knuth84}\\cite{shor95}\\cite[Thm 4]{gottesman97}, and a work \
no provider knows \\cite{unknown}. As shown in \\R\\cite{lamport94}, in \
\\R[S]\\cite{knuth84,shor95} and in the works (see \\R[I]\\cite{lamport94,shor95}), the claim \
holds. See \\ref{sec:results}.\\footnote{A note citing \\cite{aharonov97} and \
\\cite[p.~2]{knuth84}.}

\\subsection{Places}

\\begin{itemize}
\\item An item citing \\cite{knuth84}.
\\end{itemize}

\\begin{theorem}[Main result \\cite{lamport94}]\\label{thm:main}
A statement citing \\cite[Lemma 2]{shor95}.
\\end{theorem}

\\begin{proof}[*thm:main]
A proof citing \\cite{gottesman97}.
\\end{proof}

\\begin{quote}
\\text{A quotation citing \\cite{knuth84}.}
\\lines{A line citing \\cite{lamport94}\\\\
a second line}
\\end{quote}

\\begin{figure}
\\includegraphics{figures/codes.png}
\\caption{A caption citing \\cite{gottesman97}}\\label{figure:codes}
\\end{figure}

\\subsection{Tables}

\\begin{table}
\\begin{cells}
  \\celldata<H>{Work & Where}
  \\celldata{Knuth & \\cite{knuth84}}
  \\cell{Shor}\\cell{\\cite[Sec.~3]{shor95}}
\\end{cells}
\\caption{A table citing \\cite{aharonov97}}\\label{table:works}
\\end{table}";

/// The entries the test provider answers: every cited key but `unknown`.
const ENTRIES: [(&str, &str); 5] = [
    ("knuth84", "D. Knuth, \\emph{The Art of Computer Programming}, 1984"),
    ("lamport94", "L. Lamport, \\emph{A Document Preparation System}, \\href{https://example.org/latex}{online}, 1994"),
    ("shor95", "P. Shor, a code of length \\(9\\), 1995"),
    ("gottesman97", "D. Gottesman, \\emph{Stabilizer codes}, 1997"),
    ("aharonov97", "D. Aharonov and M. Ben-Or, 1997"),
];

// ---------------------------------------------------------------------------------------------
// the cases
// ---------------------------------------------------------------------------------------------

/// One setting of the document, with the stem of its goldens.
struct Case {
    /// The stem of the goldens: `citations-e2e-<name>`.
    name: &'static str,
    /// The citations feature's configuration.
    config: CitationsConfig,
    /// The render option `root_heading_level` of the text and HTML outputs.
    root: u8,
}

/// The four settings: listed and bracketed, listed and raised, in place, and
/// the default settings under a root of 2.
fn cases() -> Vec<Case> {
    vec![
        Case { name: "listed", config: CitationsConfig::new(), root: 1 },
        Case {
            name: "superscript",
            config: CitationsConfig::new().mark(MarkForm::Superscript),
            root: 1,
        },
        Case {
            name: "in-place",
            config: CitationsConfig::new().placement(CitationPlacement::InPlace),
            root: 1,
        },
        Case { name: "root-2", config: CitationsConfig::new(), root: 2 },
    ]
}

/// The standard package under the case's configuration, the entries of
/// [`ENTRIES`] answered by the test provider.
fn registry(case: &Case) -> Arc<FlmRegistry> {
    common::citations::registry_with(
        StdcoreConfig::new().citations(case.config.clone()),
        vec![Arc::new(common::citations::FromMap::of(&ENTRIES))],
    )
}

// ---------------------------------------------------------------------------------------------
// runs and their goldens
// ---------------------------------------------------------------------------------------------

/// The four workflows, in the order the diagnostics golden lists them.
const WORKFLOWS: [&str; 4] = [CHECK_WORKFLOW, text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW];

/// The overrides of `case`: the root of the text and HTML outputs.
fn overrides(case: &Case) -> Overrides {
    Overrides {
        render_options: OptionValues::new()
            .with(html::HtmlRenderOptions { root_heading_level: case.root, ..html::HtmlRenderOptions::default() })
            .with(text::TextRenderOptions { root_heading_level: case.root, ..text::TextRenderOptions::default() }),
        ..Overrides::none()
    }
}

/// The run of `workflow` over [`DOCUMENT`] under `case`.
fn run(registry: &Arc<FlmRegistry>, case: &Case, workflow: &str, options: &RunOptions) -> Run {
    let doc = common::pipeline::doc_with_options(registry, &[DOCUMENT], OptionValues::new());
    let plan = common::stdcore::plan(registry, workflow, &overrides(case));
    common::pipeline::run_with(&doc, &plan, Externals::none(), options)
}

#[test]
fn every_workflow_writes_each_case_as_its_golden() {
    for case in cases() {
        let registry = registry(&case);
        let mut diagnostics = String::new();
        for workflow in WORKFLOWS {
            let run = run(&registry, &case, workflow, &RunOptions::sequential());
            writeln!(diagnostics, "===== {workflow} =====").expect("writing to a string");
            diagnostics.push_str(&common::stdcore::diagnostic_lines(&run));
            match workflow {
                text::WORKFLOW => assert_text_golden(
                    common::lowering::golden_path(&format!("citations-e2e-{}.txt", case.name)),
                    &common::lowering::text(&run),
                ),
                html::WORKFLOW => common::html::assert_golden(
                    &format!("citations-e2e-{}.page.html", case.name),
                    &common::html::page(&run),
                ),
                // One golden: the other settings are compared with it below.
                latex_recomposed::WORKFLOW if case.name == "listed" => {
                    common::latex::assert_golden("citations-e2e.tex", &run)
                }
                _ => {}
            }
        }
        assert_text_golden(
            common::stdcore::golden_path(&format!("citations-e2e-{}-diagnostics.txt", case.name)),
            &diagnostics,
        );
    }
}

#[test]
fn the_recomposed_latex_is_the_same_under_every_setting() {
    let cases = cases();
    let written = |case: &Case| {
        let run = run(&registry(case), case, latex_recomposed::WORKFLOW, &RunOptions::sequential());
        common::latex::output(&run)
    };
    let first = written(&cases[0]);
    for case in &cases[1..] {
        assert_eq!(written(case), first, "{}", case.name);
    }
}

#[test]
fn the_missing_key_is_reported_by_every_workflow_and_the_heading_above_the_root_by_the_renders_only() {
    for case in cases() {
        let registry = registry(&case);
        for workflow in WORKFLOWS {
            let run = run(&registry, &case, workflow, &RunOptions::sequential());
            let count = |identifier: &str| {
                run.diagnostics().iter().filter(|d| d.identifier() == identifier).count()
            };
            assert_eq!(count("flm.citations.no-entry"), 1, "{} under {workflow}", case.name);
            // The section is above a root of 2; the text output and the page
            // choose a heading's tag, so they report it, and `check` renders
            // nothing (the inventory of what `check` does not see).
            let above = usize::from(case.root == 2 && (workflow == text::WORKFLOW || workflow == html::WORKFLOW));
            assert_eq!(count("flm.pres.heading-above-root"), above, "{} under {workflow}", case.name);
            // The figure's image has no inspector to measure it: each output
            // that writes it reports its unknown size.
            let unknown_size = usize::from(workflow != CHECK_WORKFLOW);
            assert_eq!(count("flm.graphics.unknown-size"), unknown_size, "{} under {workflow}", case.name);
            assert_eq!(
                run.diagnostics().iter().count(),
                1 + above + unknown_size,
                "{} under {workflow}: {}",
                case.name,
                run.diagnostics().render_all()
            );
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
    for case in cases() {
        let registry = registry(&case);
        for workflow in WORKFLOWS {
            let under = |options: &RunOptions| snapshot(&run(&registry, &case, workflow, options));
            let sequential = under(&RunOptions::sequential());
            let parallel = under(&RunOptions::new());
            assert_eq!(parallel, sequential, "{} under {workflow}", case.name);
        }
    }
}
