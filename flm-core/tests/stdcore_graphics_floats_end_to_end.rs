//! The `graphics` and `floats` features end to end: one document holding
//! images and floats in every place block content may stand — between
//! paragraphs (the graphics feature configured `global`, so that
//! `\includegraphics` is a command of the whole document), in a list item, in
//! a footnote holding blocks, in a quotation's `\block` section and in a
//! `blockquote` — and every case of a float: numbered (labeled) with and
//! without a caption, `\nonumber` with a caption and a label, unnumbered for
//! want of a label and of a caption, a type configured
//! `AutoNumbered` (`diagram`, admitting any content) with and without a label,
//! a type configured `Unnumbered` (`sketch`), labeled and unlabeled floats, a
//! `table`; references to each (`\ref`, `\ref[S]`, a list of two); and a
//! caption holding a footnote, a reference and a formula.
//!
//! It is rendered by the standard package's four workflows (`check`, `text`,
//! `html`, `latex-recomposed`). A second, short document holds the cases a
//! run reports: an unknown option, an empty path, a float holding what its
//! type does not admit, two captions, a `\nonumber` that changes nothing, a
//! caption's unknown key, and a reference to a float with neither a number
//! nor a caption. The float's body conditions and the options are reported
//! while the document is lowered, so `check` reports none of them, which its
//! golden shows.
//!
//! Each run is a golden: the text under `tests/goldens/text/`, the page under
//! `tests/goldens/html/` (parsed by `tests/html_validity.rs`), the recomposed
//! LaTeX and its style file under `tests/goldens/latex/` (the main
//! document's compiled by `tests/latex_compile.rs` under `pdflatex` and
//! `lualatex`, behind the author's declaration of the two configured float
//! environments, when `FLM_TESTS_LATEX_COMPILE` is set; the reported
//! document's is not compiled), and every run's diagnostics under
//! `tests/goldens/stdcore/`. The last test runs each workflow sequentially
//! and in parallel and compares every table the runs wrote; the full battery
//! runs this suite a third time on a pool of one thread
//! (`RAYON_NUM_THREADS=1`), against the same goldens.

mod common;

use std::fmt::Write as _;
use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::golden::assert_text_golden;
use flm_core::dump::project::{ProjectOptions, Projector};
use flm_core::dump::to_json_pretty;
use flm_core::model::addr::TreeId;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::pres::{ImageDisplay, ImagePayload, payload_of};
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::floats::{FloatNumbering, FloatType, FloatsConfig};
use flm_core::stdcore::features::graphics::GraphicsConfig;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig};

// ---------------------------------------------------------------------------------------------
// the documents
// ---------------------------------------------------------------------------------------------

/// The document of every place and every case. It reports the Warning of its
/// one `width=` on the three renders, and on every workflow the Note that no
/// feature provides tables of cells, which a `table` may hold.
///
/// A numbered float stands everywhere but in the footnote: LaTeX sets a
/// footnote's text in a box, where a float environment cannot stand, so the
/// float there is unnumbered, which the recomposed LaTeX sets in place.
const DOCUMENT: &str = "\
\\section{Graphics and floats}\\label{sec:intro}

An image standing between paragraphs:

\\includegraphics[alt=A plot, width=5cm]{figures/plot.png}

\\begin{figure}
\\includegraphics{figures/a.png}
\\caption{A photo with a note\\footnote{Taken at noon.}, a reference to \
\\ref{sec:intro} and a formula \\(x^2\\)}
\\label{figure:a}
\\end{figure}

\\begin{figure}\\label{figure:b}\\includegraphics{figures/b.png}\\end{figure}

\\begin{figure}\\nonumber
\\includegraphics{figures/c.png}
\\caption[alt=Photo C]{A photo of \\emph{C}}\\label{figure:c}
\\end{figure}

\\begin{figure}\\includegraphics{figures/d.png}\\end{figure}

\\begin{table}
\\caption{A table shown as an image}\\label{table:t}
\\includegraphics{figures/t.png}
\\end{table}

\\begin{diagram}Boxes and arrows.\\caption{Arrows}\\label{diagram:x}\\end{diagram}

\\begin{diagram}More boxes.\\caption{An unlabeled diagram}\\end{diagram}

\\begin{sketch}\\includegraphics{figures/s.png}\\caption{A sketch}\\label{sketch:s}\\end{sketch}

\\begin{itemize}
\\item An item holding an image:
\\includegraphics{figures/item.png}
\\item An item holding a figure:
\\begin{figure}\\includegraphics{figures/item-figure.png}\\caption{In an item}\\label{figure:item}\\end{figure}
\\end{itemize}

A footnote holding blocks.\\footnote{Its first paragraph.

\\includegraphics{figures/note.png}

\\begin{figure}\\nonumber\\includegraphics{figures/note-figure.png}\\caption{In a note}\\label{figure:note}\\end{figure}}

\\begin{quote}
\\block{A block section.

\\includegraphics{figures/quote.png}

\\begin{figure}\\includegraphics{figures/quote-figure.png}\\caption{In a quotation}\\label{figure:quote}\\end{figure}}
\\end{quote}

\\begin{blockquote}
A blockquote.

\\includegraphics{figures/blockquote.png}

\\begin{figure}\\includegraphics{figures/blockquote-figure.png}\\caption{In a blockquote}\\label{figure:blockquote}\\end{figure}
\\end{blockquote}

See \\ref{figure:a}, \\ref[S]{figure:a}, \\ref{figure:a,figure:b}, \\ref{figure:c}, \
\\ref{table:t}, \\ref{diagram:x}, \\ref{sketch:s}, \\ref{figure:item}, \\ref{figure:note} \
and \\ref{figure:quote}.";

/// The document of the cases a run reports.
const REPORTED: &str = "\
\\includegraphics[alt=x, scale=2]{figures/x.png}

\\includegraphics{}

\\begin{figure}Only text, no image.\\end{figure}

\\begin{figure}\\includegraphics{figures/y.png}\\caption{One}\\caption{Two}\\end{figure}

\\begin{diagram}Unlabeled.\\nonumber\\end{diagram}

\\begin{figure}\\includegraphics{figures/z.png}\\caption[short=Z]{Zed}\\end{figure}

\\begin{figure}\\nonumber\\includegraphics{figures/w.png}\\label{figure:w}\\end{figure}

See \\ref{figure:w}.";

// ---------------------------------------------------------------------------------------------
// the configuration and the cases
// ---------------------------------------------------------------------------------------------

/// The standard package: `\includegraphics` a command of the whole document
/// (`global`); besides `figure` and `table`, a type `diagram`, admitting any
/// content, numbered when labeled, and a type `sketch`, admitting one image,
/// never numbered.
fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(
        StdcoreConfig::new().graphics(GraphicsConfig::new().global(true)).floats(
            FloatsConfig::new()
                .float_type(FloatType::new("diagram", ["any"]).numbering(FloatNumbering::AutoNumbered))
                .float_type(
                    FloatType::new("sketch", ["includegraphics"]).numbering(FloatNumbering::Unnumbered),
                ),
        ),
    )
}

/// One document, with the stem of its goldens.
struct Case {
    /// The stem of the goldens: `graphics-floats-e2e-<name>`.
    name: &'static str,
    /// The document.
    document: &'static str,
}

/// [`DOCUMENT`], then [`REPORTED`].
fn cases() -> Vec<Case> {
    vec![Case { name: "shown", document: DOCUMENT }, Case { name: "reported", document: REPORTED }]
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
    let registry = registry();
    for case in cases() {
        let mut diagnostics = String::new();
        for workflow in WORKFLOWS {
            let run = run(&registry, &case, workflow, &RunOptions::sequential());
            writeln!(diagnostics, "===== {workflow} =====").expect("writing to a string");
            diagnostics.push_str(&common::stdcore::diagnostic_lines(&run));
            match workflow {
                text::WORKFLOW => assert_text_golden(
                    common::lowering::golden_path(&format!("graphics-floats-e2e-{}.txt", case.name)),
                    &common::lowering::text(&run),
                ),
                html::WORKFLOW => common::html::assert_golden(
                    &format!("graphics-floats-e2e-{}.page.html", case.name),
                    &common::html::page(&run),
                ),
                latex_recomposed::WORKFLOW => common::latex::assert_golden(
                    &format!("graphics-floats-e2e-{}.tex", case.name),
                    &run,
                ),
                _ => {}
            }
        }
        assert_text_golden(
            common::stdcore::golden_path(&format!("graphics-floats-e2e-{}-diagnostics.txt", case.name)),
            &diagnostics,
        );
    }
}

#[test]
fn the_shown_document_is_reported_its_width_alone() {
    let registry = registry();
    let case = cases().into_iter().find(|case| case.name == "shown").expect("the shown case");
    for workflow in WORKFLOWS {
        let run = run(&registry, &case, workflow, &RunOptions::sequential());
        let identifiers: Vec<&str> = run.diagnostics().iter().map(|d| d.identifier()).collect();
        // The options are read where the image is lowered, which `check`
        // does not do. No inspector measures the files: every other image,
        // given no size, is reported its unknown size where it is lowered.
        let mut expected: Vec<&str> = Vec::new();
        if workflow != CHECK_WORKFLOW {
            expected.push("flm.graphics.resized");
        }
        let unknown = identifiers.iter().filter(|id| **id == "flm.graphics.unknown-size").count();
        assert_eq!(unknown > 0, workflow != CHECK_WORKFLOW, "under {workflow}");
        let mut found: Vec<&str> =
            identifiers.iter().copied().filter(|id| *id != "flm.graphics.unknown-size").collect();
        found.sort_unstable();
        expected.sort_unstable();
        assert_eq!(found, expected, "under {workflow}: {}", run.diagnostics().render_all());
    }
}

/// A `sketch`, whose type names the `includegraphics` handler and no scoped
/// package, holds its image as a block: a direct child of a float's body, as
/// in a `figure`; an image in a paragraph of its own is inline.
#[test]
fn the_image_of_a_sketch_is_a_block_as_a_figure_s_is() {
    let registry = registry();
    let case = cases().into_iter().find(|case| case.name == "shown").expect("the shown case");
    let run = run(&registry, &case, text::WORKFLOW, &RunOptions::sequential());
    let tree = &common::lowering::lowered(&run, TreeId::tree(0)).tree;
    let displays: Vec<(String, ImageDisplay)> = tree
        .root()
        .descendants()
        .filter_map(|node| payload_of::<ImagePayload>(&node).map(|image| (image.src.clone(), image.display)))
        .collect();
    for (src, display) in [
        ("figures/s.png", ImageDisplay::Block),
        ("figures/a.png", ImageDisplay::Block),
        ("figures/plot.png", ImageDisplay::Inline),
    ] {
        assert!(displays.contains(&(String::from(src), display)), "{src}: {displays:?}");
    }
}

#[test]
fn the_reported_cases_are_reported_by_every_render_and_none_by_check() {
    let registry = registry();
    let case = cases().into_iter().find(|case| case.name == "reported").expect("the reported case");
    for workflow in WORKFLOWS {
        let run = run(&registry, &case, workflow, &RunOptions::sequential());
        let count = |identifier: &str| {
            run.diagnostics().iter().filter(|d| d.identifier() == identifier).count()
        };
        // Every condition below is reported while the document is lowered:
        // every render reports it, `check` none (the inventory of what
        // `check` does not see).
        let lowered = if workflow == CHECK_WORKFLOW { 0 } else { 1 };
        assert_eq!(count("flm.graphics.unknown-key"), lowered, "under {workflow}");
        assert_eq!(count("flm.graphics.empty-path"), lowered, "under {workflow}");
        assert_eq!(count("flm.floats.content-refused"), lowered, "under {workflow}");
        assert_eq!(count("flm.floats.caption-repeated"), lowered, "under {workflow}");
        assert_eq!(count("flm.floats.nonumber-without-effect"), lowered, "under {workflow}");
        assert_eq!(count("flm.floats.caption-unknown-key"), lowered, "under {workflow}");
        // The recomposed LaTeX writes the reference for LaTeX to resolve.
        let shown = if workflow == CHECK_WORKFLOW || workflow == latex_recomposed::WORKFLOW { 0 } else { 1 };
        assert_eq!(count("flm.refs.nothing-to-show"), shown, "under {workflow}");
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
