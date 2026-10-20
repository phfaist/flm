//! The `xnotes` feature end to end: one document holding an xnote in every
//! place an xnote may stand — a paragraph, a heading's title, a list item,
//! `\text{…}` inside a displayed equation, a user definition's template —
//! and every shape an xnote may take — nested in another, labeled and
//! referred to, of two paragraphs, holding a list, empty, with a refused
//! label argument — rendered by the standard package's four workflows
//! (`text`, `html`, `latex-recomposed`, `check`) under three configurations:
//!
//! - the default one: the kind `footnote`, numbered and listed after the
//!   content;
//! - two kinds: `footnote` and a numbered `endnote`, each listed on its own;
//! - in place: `footnote` shown where it stands, together with an unnumbered
//!   `sidenote` shown in place with no mark.
//!
//! A second, shorter document holds the two cases where the run reports a
//! problem about an xnote's content, under the default and the in-place
//! configurations: a note of two paragraphs where no block can stand (a
//! heading's title, `\text{…}` in a displayed equation) — listed with its
//! paragraphs under the default configuration; reported by the language
//! layer (`flm.blocks.block-in-inline`, under the `check` workflow too) and
//! set inline with its paragraphs joined under the in-place one — and a
//! reference inside a note that resolves to no label. Its recomposed LaTeX
//! does not compile, and `tests/latex_compile.rs` skips it with the reason.
//!
//! Each run is a golden: the text output under `tests/goldens/text/`, the page
//! under `tests/goldens/html/` (parsed by `tests/html_validity.rs`, which also
//! holds every link to an identifier of the page), the recomposed LaTeX and
//! its style file under `tests/goldens/latex/` (compiled by
//! `tests/latex_compile.rs` when `FLM_TESTS_LATEX_COMPILE` is set), and every
//! run's diagnostics under `tests/goldens/stdcore/`. The last test runs each
//! workflow sequentially and in parallel and compares every table the runs
//! wrote, the lists of xnotes among them; the full battery runs this suite a
//! third time on a pool of one thread (`RAYON_NUM_THREADS=1`), against the
//! same goldens.

mod common;

use std::fmt::Write as _;
use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::golden::assert_text_golden;
use flm_core::dump::project::{ProjectOptions, Projector};
use flm_core::dump::to_json_pretty;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::numbering::PrefixWords;
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::features::xnotes::{XnoteKind, XnotePlacement, XnotesConfig};
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig, package};

use common::stdcore::run_workflow;

// ---------------------------------------------------------------------------------------------
// the document
// ---------------------------------------------------------------------------------------------

/// The document every configuration renders: an xnote in each place the
/// syntax allows, and in each shape. `\noted` is a user definition whose
/// template holds an xnote.
const DOCUMENT: &str = "\
\\section{A heading with a note\\footnote{The heading's note.}}\\label{sec:intro}

A paragraph holds a note.\\footnote{A note of one run.} The paragraph goes on.

A labeled note.\\footnote[\\label{fn:lab}]{The labeled note.} See \\ref{fn:lab}, \
\\ref[noprefix]{fn:lab} and \\ref{sec:intro}.

Two paragraphs.\\footnote{The first paragraph of the note.

The second paragraph of the note.} And on.

A note holding a list.\\footnote{Steps:
\\begin{itemize}\\item one \\item two\\end{itemize}}

\\begin{itemize}
\\item An item with a note.\\footnote{The item's note.}
\\end{itemize}

\\begin{equation}
x = y \\text{ where a note stands\\footnote{The formula's note.}}
\\end{equation}

A definition writes one: \\noted{} Nested.\\footnote{Outer\\footnote{inner} note.}

An empty note.\\footnote{} A refused label argument.\\footnote[\\textbf{x}\\label{fn:bad}]{The \
refused label's note.} See \\ref{fn:bad}.";

/// The second document, of the two cases where the run reports a problem
/// about an xnote's content, rendered under the default and the in-place
/// configurations: a note of two paragraphs where no block can stand (a
/// heading's title, `\\text{…}` in a displayed equation), and a reference
/// inside a note that resolves to no label. It is a document of its own, and
/// not part of [`DOCUMENT`], because its recomposed LaTeX does not compile
/// (see `NOT_COMPILED` in `tests/latex_compile.rs`), while every golden of
/// [`DOCUMENT`] does.
const REPORTED: &str = "\
\\section{A heading holds blocks\\footnote{The heading's first paragraph.

The heading's second paragraph.}}

A formula holds blocks:
\\begin{equation}
x \\text{ where a note stands\\footnote{The formula's first paragraph.

The formula's second paragraph.}}
\\end{equation}

A note refers nowhere.\\footnote{See \\ref{fn:nowhere}.}";

/// What the configuration of two kinds adds to [`DOCUMENT`]: three xnotes of
/// the kind `endnote`, one labeled and referred to, one of two paragraphs.
const ENDNOTES: &str = "\n\nEndnotes too.\\endnote{The first endnote.} A labeled one.\\endnote[\\label{en:e}]{The labeled \
endnote.} See \\ref{en:e}. Two paragraphs.\\endnote{The first paragraph of the endnote.

The second paragraph of the endnote.} And on.";

/// What the in-place configuration adds to [`DOCUMENT`]: an xnote of the
/// unnumbered kind `sidenote`, labeled and referred to.
const SIDENOTES: &str = "\n\nAn aside stands here. \\sidenote[\\label{sn:a}]{An aside.} And on; see \\ref{sn:a}.";

// ---------------------------------------------------------------------------------------------
// the three configurations
// ---------------------------------------------------------------------------------------------

/// One configuration of the xnotes feature, with the document it renders and
/// the stem of its goldens.
struct Case {
    /// The stem of the goldens: `xnotes-e2e-<name>`.
    name: &'static str,
    /// The standard package, the xnotes feature under this configuration.
    registry: Arc<FlmRegistry>,
    /// The document.
    document: String,
}

/// The standard package, with `\noted` defined and its xnotes feature under
/// `xnotes`.
fn registry(xnotes: XnotesConfig) -> Arc<FlmRegistry> {
    let noted = Definition::macro_("noted", Vec::new())
        .text("a definition's text\\footnote{The definition's note.}");
    let config = StdcoreConfig::new()
        .userdefs(UserdefsConfig::new().definitions(vec![noted]))
        .xnotes(xnotes);
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(config));
    builder.use_package(flm_core::math_passthrough::package());
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

/// The configuration that shows every footnote where it stands, with an
/// unnumbered `sidenote` shown in place too.
fn in_place() -> XnotesConfig {
    let sidenote = XnoteKind::unnumbered("sidenote", PrefixWords::new("Side note~", "Side notes~"))
        .commands(["sidenote"])
        .label_type("sn")
        .placement(XnotePlacement::InPlace);
    XnotesConfig { kinds: vec![XnoteKind::footnote().placement(XnotePlacement::InPlace), sidenote] }
}

/// The three configurations over [`DOCUMENT`], then the default and the
/// in-place ones over [`REPORTED`].
fn cases() -> Vec<Case> {
    let endnote = XnoteKind::numbered("endnote", PrefixWords::new("Endnote~", "Endnotes~"))
        .commands(["endnote"])
        .label_type("en");
    vec![
        Case {
            name: "default",
            registry: registry(XnotesConfig::new()),
            document: DOCUMENT.to_string(),
        },
        Case {
            name: "two-kinds",
            registry: registry(XnotesConfig::new().kind(endnote)),
            document: format!("{DOCUMENT}{ENDNOTES}"),
        },
        Case {
            name: "in-place",
            registry: registry(in_place()),
            document: format!("{DOCUMENT}{SIDENOTES}"),
        },
        Case {
            name: "reported-default",
            registry: registry(XnotesConfig::new()),
            document: REPORTED.to_string(),
        },
        Case {
            name: "reported-in-place",
            registry: registry(in_place()),
            document: REPORTED.to_string(),
        },
    ]
}

// ---------------------------------------------------------------------------------------------
// runs and their goldens
// ---------------------------------------------------------------------------------------------

/// The four workflows, in the order the diagnostics golden lists them.
const WORKFLOWS: [&str; 4] =
    [CHECK_WORKFLOW, text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW];

/// The run of `workflow` over the case's document, sequentially.
fn run(case: &Case, workflow: &str) -> Run {
    run_workflow(&case.registry, workflow, &[&case.document], &Overrides::none())
}

#[test]
fn every_workflow_writes_the_document_of_every_place_as_its_golden_under_each_configuration() {
    for case in cases() {
        let mut diagnostics = String::new();
        for workflow in WORKFLOWS {
            let run = run(&case, workflow);
            writeln!(diagnostics, "===== {workflow} =====").expect("writing to a string");
            diagnostics.push_str(&common::stdcore::diagnostic_lines(&run));
            match workflow {
                text::WORKFLOW => assert_text_golden(
                    common::lowering::golden_path(&format!("xnotes-e2e-{}.txt", case.name)),
                    &common::lowering::text(&run),
                ),
                html::WORKFLOW => common::html::assert_golden(
                    &format!("xnotes-e2e-{}.page.html", case.name),
                    &common::html::page(&run),
                ),
                // The recomposed LaTeX keeps `\noted` as a call and writes its
                // template into the style file, the xnote's command with it.
                latex_recomposed::WORKFLOW => {
                    common::latex::assert_golden(&format!("xnotes-e2e-{}.tex", case.name), &run)
                }
                _ => {}
            }
        }
        assert_text_golden(
            common::stdcore::golden_path(&format!("xnotes-e2e-{}-diagnostics.txt", case.name)),
            &diagnostics,
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
        let doc = common::pipeline::doc(&case.registry, &[&case.document]);
        for workflow in WORKFLOWS {
            let plan = common::stdcore::plan(&case.registry, workflow, &Overrides::none());
            let under = |options: &RunOptions| {
                snapshot(&common::pipeline::run_with(
                    &doc,
                    &plan,
                    Externals::none(),
                    options,
                ))
            };
            let sequential = under(&RunOptions::sequential());
            assert_eq!(under(&RunOptions::new()), sequential, "{} under `{workflow}`", case.name);
            if workflow != CHECK_WORKFLOW {
                assert!(
                    sequential.contains(".pieces = ") || workflow == latex_recomposed::WORKFLOW,
                    "the snapshot holds the lowered pieces ({} under `{workflow}`):\n{sequential}",
                    case.name
                );
            }
        }
    }
}
