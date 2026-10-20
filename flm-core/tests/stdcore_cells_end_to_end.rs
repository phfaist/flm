//! The `cells` feature end to end, over two documents.
//!
//! The first, `global`, is rendered by a registry whose `cells` environment is
//! an ordinary environment of the whole document (`CellsConfig { global: true
//! }`). It holds the examples of the Python product's documentation, a table in
//! a `table` float with a caption and a label and the two references to it
//! (`\ref` and `\ref[S]`), a table in a list item and in a footnote holding
//! blocks, a cell holding a formula, a reference, a footnote, two paragraphs
//! and a nested table, every shipped style token, names of rows and columns
//! used in a placement and in a `\merge`, a span to the end of the table, and
//! every mistake a table can make once — the ten conditions, each with its
//! recovery and, where something is left out, its mark — and a blank line
//! inside a row's data, the language's report of a paragraph break inside
//! inline content.
//!
//! The second, `floats`, is rendered under the default configuration, where
//! the environment exists inside a `table` float alone: its tables stand in
//! floats, one cell holds an image (inline content of the cell), and a table is
//! nested in a cell of a table in a float. No float stands inside a cell.
//!
//! Each document is rendered by the standard package's four workflows
//! (`check`, `text`, `html`, `latex-recomposed`), and each run is a golden: the
//! text under `tests/goldens/text/`, the page under `tests/goldens/html/`
//! (parsed by `tests/html_validity.rs`), the recomposed LaTeX and its style
//! file under `tests/goldens/latex/` (both compiled by `tests/latex_compile.rs`
//! under `pdflatex` and `lualatex` when `FLM_TESTS_LATEX_COMPILE` is set, a
//! one-pixel picture written at the image's path), and every run's
//! diagnostics under `tests/goldens/stdcore/`. A table's mistakes are reported
//! by the one reader of its body while the document is lowered, so `check`
//! reports none of them, which its golden shows. The last test runs each
//! workflow sequentially and in parallel and compares every table the runs
//! wrote.

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
use flm_core::stdcore::features::cells::CellsConfig;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig};

// ---------------------------------------------------------------------------------------------
// the documents
// ---------------------------------------------------------------------------------------------

/// The document under `global: true`: every place, every token, every mistake.
const GLOBAL: &str = "\
\\section{Tables of cells}\\label{sec:cells}

A table between paragraphs, the first example of the Python product's documentation:

\\begin{cells}
  \\celldata<H>{
    Header 1 & Header 2
  }
  \\celldata{
    Row 1, Col 1 & Row 1, Col 2 \\\\
    Row 2, Col 1 & Row 2, Col 2
  }
\\end{cells}

A header spanning two columns, then data placed from the second column on:

\\begin{cells}
  \\cell<H l>[1;\\merge{3-4}]{Hi!}
  \\\\
  \\celldata[2-5]{A & B & C & D \\\\ E & F & G & H}
\\end{cells}

Merged cells from a mapping:

\\begin{cells}
  \\celldata[\\merge{1,2},\\merge{+2};4,6]{A & AA \\\\ B & BB}
\\end{cells}

Styles per column, a mapping permuting the columns, and a cell overriding its entry:

\\begin{cells}
  \\celldata<H l, H c>{One & Two & Three}
  \\celldata[3,2,1]{A & B & C}
  \\celldata<H>{A & B & \\cell<green>[\\merge{+2}]{C}}
\\end{cells}

Every token, with names for the rows and the columns:

\\begin{cells}
  \\colnames{name, city, height}
  \\rownames{head, john, anna, both}
  \\celldata<H>{Name & City & Height}
  \\cell<rH>[john;name]{John}\\cell<l red>{Berlin}\\cell<r>{1.80}
  \\\\
  \\cell<head{row}>[anna;name]{Anna}\\cell<c green lvert rvert>{Zurich}\\cell<align{right} blue>{1.65}
  \\\\
  \\cell<yellow rule{above} rule{below}>[both;\\merge{name-city}]{Two columns by name}
  \\cell<head{column} align{center} rule{left} rule{right}>[;height]{Last}
  \\\\
  \\cell[\\merge{2-}]{To the end of the table}
\\end{cells}

\\begin{table}
  \\begin{cells}
    \\celldata{
      & Name & Value \\\\
      & Alpha & 1 \\\\
      & Beta & 2
    }
  \\end{cells}
  \\caption{A simple table.}
  \\label{table:x}
\\end{table}

See \\ref{table:x} and \\ref[S]{table:x}.

\\begin{itemize}
\\item An item holding a table:
\\begin{cells}\\celldata<H>{Key & Value}\\celldata{a & 1}\\end{cells}
\\end{itemize}

A footnote holding blocks.\\footnote{Its first paragraph.

\\begin{cells}\\celldata{c & d}\\end{cells}}

\\begin{cells}
  \\cell<H>{Content}\\cell<H>{Nested}
  \\\\
  \\cell{A formula \\(x^2\\), a reference to \\ref{sec:cells} and a note\\footnote{In a cell.}.

  A second paragraph.}
  \\cell{\\begin{cells}\\celldata<H>{n & m}\\celldata{1 & 2}\\end{cells}}
\\end{cells}

\\section{Mistakes}

Content outside a cell: \\begin{cells}\\cell{A}stray text\\cell{B}\\end{cells}

A placement that cannot be read: \\begin{cells}\\cell{A}\\cell[a;b;c]{P}\\end{cells}

A command among the styles: \\begin{cells}\\cell<\\emph{x}>{S}\\end{cells}

An unknown style: \\begin{cells}\\cell<foo r>{U}\\end{cells}

A value a style does not take: \\begin{cells}\\cell<align{middle} green{x}>{V}\\end{cells}

Names that cannot be: \\begin{cells}\\colnames{a,a,1x}\\cell{N}\\end{cells}

Two cells on one position: \\begin{cells}\\cell{O}\\cell[1]{Q}\\end{cells}

A table too large: \\begin{cells}\\cell{T}\\cell[\\merge{1-1000};\\merge{1-1001}]{Big}\\end{cells}

An empty table: \\begin{cells}\\end{cells}

A blank line inside a row's data: \\begin{cells}\\celldata{A

B & C}\\end{cells}

Commands outside their table:
\\begin{cells}\\celldata{x & \\celldata{y & z}}\\celldata{\\emph{v \\\\ w}}\\end{cells}";

/// The document under the default configuration: tables inside `table` floats
/// alone.
const FLOATS: &str = "\
\\section{Tables in floats}

\\begin{table}
\\begin{cells}
  \\celldata<H>{Picture & What it shows}
  \\celldata{\\includegraphics{figures/cell.png} & A picture standing in a cell}
\\end{cells}
\\caption{A table holding a picture}\\label{table:picture}
\\end{table}

\\begin{table}
\\begin{cells}
  \\cell<H>{Outer}\\cell<H>{Inner}
  \\\\
  \\cell{A cell}\\cell{\\begin{cells}\\celldata<H>{n & m}\\celldata{1 & 2}\\end{cells}}
\\end{cells}
\\caption{A table nested in a cell}\\label{table:nested}
\\end{table}

\\begin{table}
\\begin{cells}\\celldata{x & y}\\end{cells}
\\end{table}

See \\ref{table:picture} and \\ref[S]{table:nested}.";

// ---------------------------------------------------------------------------------------------
// the configurations and the cases
// ---------------------------------------------------------------------------------------------

/// One document, its configuration, with the stem of its goldens.
struct Case {
    /// The stem of the goldens: `cells-e2e-<name>`.
    name: &'static str,
    /// The document.
    document: &'static str,
    /// The standard package's configuration it is rendered under.
    config: fn() -> StdcoreConfig,
}

/// [`GLOBAL`] under `global: true`, then [`FLOATS`] under the default.
fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "global",
            document: GLOBAL,
            config: || StdcoreConfig::new().cells(CellsConfig::new().global(true)),
        },
        Case { name: "floats", document: FLOATS, config: StdcoreConfig::new },
    ]
}

fn case(name: &str) -> Case {
    cases().into_iter().find(|case| case.name == name).expect("the case")
}

// ---------------------------------------------------------------------------------------------
// runs and their goldens
// ---------------------------------------------------------------------------------------------

/// The four workflows, in the order the diagnostics golden lists them.
const WORKFLOWS: [&str; 4] = [CHECK_WORKFLOW, text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW];

/// The run of `workflow` over the case's document.
fn run(registry: &Arc<FlmRegistry>, case: &Case, workflow: &str, options: &RunOptions) -> Run {
    let doc = common::pipeline::doc_with_options(registry, &[case.document], OptionValues::new());
    let plan = common::stdcore::plan(registry, workflow, &Overrides::none());
    common::pipeline::run_with(&doc, &plan, Externals::none(), options)
}

#[test]
fn every_workflow_writes_each_case_as_its_golden() {
    for case in cases() {
        let registry = common::stdcore::registry((case.config)());
        let mut diagnostics = String::new();
        for workflow in WORKFLOWS {
            let run = run(&registry, &case, workflow, &RunOptions::sequential());
            writeln!(diagnostics, "===== {workflow} =====").expect("writing to a string");
            diagnostics.push_str(&common::stdcore::diagnostic_lines(&run));
            match workflow {
                text::WORKFLOW => assert_text_golden(
                    common::lowering::golden_path(&format!("cells-e2e-{}.txt", case.name)),
                    &common::lowering::text(&run),
                ),
                html::WORKFLOW => common::html::assert_golden(
                    &format!("cells-e2e-{}.page.html", case.name),
                    &common::html::page(&run),
                ),
                latex_recomposed::WORKFLOW => {
                    common::latex::assert_golden(&format!("cells-e2e-{}.tex", case.name), &run)
                }
                _ => {}
            }
        }
        assert_text_golden(
            common::stdcore::golden_path(&format!("cells-e2e-{}-diagnostics.txt", case.name)),
            &diagnostics,
        );
    }
}

/// The ten conditions of a table, each reported by every render and none by
/// `check`, and nothing else but the language's two reports of block
/// structure: a block inside inline content, a paragraph break inside it.
#[test]
fn every_mistake_of_a_table_is_reported_by_every_render_and_none_by_check() {
    let case = case("global");
    let registry = common::stdcore::registry((case.config)());
    for workflow in WORKFLOWS {
        let run = run(&registry, &case, workflow, &RunOptions::sequential());
        let count = |identifier: &str| run.diagnostics().iter().filter(|d| d.identifier() == identifier).count();
        let lowered = if workflow == CHECK_WORKFLOW { 0 } else { 1 };
        for condition in [
            "flm.cells.content-outside-cell",
            "flm.cells.bad-styles",
            "flm.cells.unknown-style",
            "flm.cells.overlap",
            "flm.cells.too-large",
            "flm.cells.empty",
        ] {
            assert_eq!(count(condition), lowered, "{condition} under {workflow}");
        }
        // `a` declared twice and `1x`, which is not a name.
        assert_eq!(count("flm.cells.bad-names"), 2 * lowered, "under {workflow}");
        // `[a;b;c]`.
        assert_eq!(count("flm.cells.bad-placement"), lowered, "under {workflow}");
        // `align{middle}` and `green{x}`.
        assert_eq!(count("flm.cells.bad-style-value"), 2 * lowered, "under {workflow}");
        // The `\celldata` inside a `\celldata`'s data, the `&` of its own data
        // shown there, and the `\\` inside `\emph` in a `\celldata` entry.
        assert_eq!(count("flm.cells.outside-table"), 3 * lowered, "under {workflow}");
        // The `\celldata` inside a `\celldata`'s data is also a block inside
        // inline content, the language's report of block structure, which
        // `check` gives and the recomposed LaTeX, lowering the tree before
        // block structure, does not.
        let structure = if workflow == latex_recomposed::WORKFLOW { 0 } else { 1 };
        assert_eq!(count("flm.blocks.block-in-inline"), structure, "under {workflow}");
        // The blank line inside `\celldata{…}`, a paragraph break inside
        // inline content, the same.
        assert_eq!(count("flm.blocks.parbreak-in-inline"), structure, "under {workflow}");
        assert_eq!(run.diagnostics().iter().count(), 14 * lowered + 2 * structure, "under {workflow}");
    }
}

/// The document under the default configuration is reported nothing: its
/// tables stand in `table` floats, which admit them. (Its image, which no
/// inspector measures, is reported its unknown size alone.)
#[test]
fn the_floats_document_is_reported_nothing() {
    let case = case("floats");
    let registry = common::stdcore::registry((case.config)());
    for workflow in WORKFLOWS {
        let run = run(&registry, &case, workflow, &RunOptions::sequential());
        assert_eq!(
            common::stdcore::identifiers_but_unknown_size(&run),
            Vec::<String>::new(),
            "under {workflow}: {}",
            run.diagnostics().render_all()
        );
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
        let registry = common::stdcore::registry((case.config)());
        for workflow in WORKFLOWS {
            let under = |options: &RunOptions| snapshot(&run(&registry, &case, workflow, options));
            let sequential = under(&RunOptions::sequential());
            let parallel = under(&RunOptions::new());
            assert_eq!(parallel, sequential, "{} under {workflow}", case.name);
        }
    }
}
