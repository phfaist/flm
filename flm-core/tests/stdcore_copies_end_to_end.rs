//! Copies of shown content end to end: one document holding every kind of
//! content a reference shows again away from where it is written, each
//! holding something that must behave in the copy —
//!
//! - an unnumbered heading whose title holds a footnote, referred to twice;
//! - one whose title holds a labeled footnote, with a reference to the
//!   heading and one to the note;
//! - an unnumbered figure whose caption holds a footnote, referred to;
//! - a defined term holding a footnote, referred to through its label;
//! - a list item whose tag holds a footnote, referred to;
//! - a heading whose title holds a note of a configured kind shown where it
//!   is written (`sidenote`, numbered, in place), referred to;
//! - a heading whose title holds a reference to nothing, referred to twice;
//! - a heading whose title holds an image with `width=`, referred to twice,
//!   the host's inspector answering a picture with no resolution;
//! - a user definition whose template holds a reference to nothing, used
//!   twice, kept as a LaTeX definition on the recomposed LaTeX.
//!
//! What a reference shows of a note is the note's mark, linked to the note
//! and with no identifier of its own; the note is listed once. Every report
//! is made wherever it is raised, and one raised inside a copy carries a
//! frame naming the place the copy is shown. The document is rendered by the
//! standard package's four workflows (`check`, `text`, `html`,
//! `latex-recomposed`).
//!
//! Each run is a golden: the text under `tests/goldens/text/`, the page under
//! `tests/goldens/html/` (parsed by `tests/html_validity.rs`, which holds
//! every identifier of the page unique and every link to one resolved), the
//! recomposed LaTeX and its style file under `tests/goldens/latex/`, and every
//! run's diagnostics, frames included, under `tests/goldens/stdcore/`. The
//! document's references to nothing are deliberate, so the recomposed LaTeX
//! is compiled by a test of its own in `tests/latex_compile.rs`, which
//! expects exactly LaTeX's warning of undefined references. The last test
//! runs each workflow sequentially and in parallel and compares every table
//! the runs wrote; the full battery runs this suite a third time on a pool of
//! one thread (`RAYON_NUM_THREADS=1`), against the same goldens.

mod common;

use std::fmt::Write as _;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::golden::assert_text_golden;
use flm_core::dump::project::{ProjectOptions, Projector};
use flm_core::dump::to_json_pretty;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::pres::I48F16;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::graphics::{
    GraphicsConfig, GraphicsInspectError, GraphicsInspectRequest, GraphicsInspectedInformation,
    GraphicsInspector, INSPECTOR_SLOT, PixelSize,
};
use flm_core::stdcore::features::numbering::PrefixWords;
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::features::xnotes::{XnoteKind, XnotePlacement, XnotesConfig};
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig, package};

use common::stdcore::SHOWN_HERE;

// ---------------------------------------------------------------------------------------------
// the document
// ---------------------------------------------------------------------------------------------

/// The document. `\seenowhere` is a user definition whose template refers to
/// `sec:nowhere`, which nothing labels.
///
/// No reference names the label of a footnote inside a caption: LaTeX sets a
/// caption in a box, where a footnote loses its text and its label, so such a
/// reference would be undefined in the PDF.
const DOCUMENT: &str = "\
\\section*{Intro\\footnote{A note in the title.}}\\label{sec:a}

See \\ref{sec:a}, and again \\ref{sec:a}.

\\section*{Labeled\\footnote[\\label{fn:a}]{A labeled note.}}\\label{sec:b}

See \\ref{sec:b}; its note is \\ref{fn:a}.

\\begin{figure}\\nonumber
\\includegraphics{figures/a.png}
\\caption{A photo\\footnote{A note in the caption.}}\\label{figure:a}
\\end{figure}

See \\ref{figure:a}.

\\begin{defterm}{Entropy\\footnote{A note in the term.}}\\label{topic:entropy}
A measure of disorder.
\\end{defterm}

See \\ref{topic:entropy}.

\\begin{enumerate}
\\item[Tag\\footnote{A note in the tag.}]\\label{item:a} One.
\\end{enumerate}

See \\ref{item:a}.

\\section*{Aside\\sidenote{A note shown where it stands.}}\\label{sec:d}

See \\ref{sec:d}.

\\section*{See \\ref{sec:nowhere}}\\label{sec:e}

See \\ref{sec:e}, and again \\ref{sec:e}.

\\section*{Plot \\includegraphics[width=5cm]{bare.png}}\\label{sec:c}

See \\ref{sec:c}, and again \\ref{sec:c}.

A definition refers nowhere: \\seenowhere{} and \\seenowhere{}.";

// ---------------------------------------------------------------------------------------------
// the configuration
// ---------------------------------------------------------------------------------------------

/// A host's inspector: the figure's picture states its resolution; the one
/// image of a title, `bare.png`, is a picture of 300 by 150 pixels stating
/// none. It counts the requests about `bare.png` it answers.
struct Inspector(Arc<AtomicUsize>);

impl GraphicsInspector for Inspector {
    fn inspect(
        &self,
        request: &GraphicsInspectRequest,
    ) -> Result<GraphicsInspectedInformation, GraphicsInspectError> {
        let raster = |dpi: Option<i32>| GraphicsInspectedInformation::Raster {
            dpi: dpi.map(I48F16::from_num),
            size: PixelSize { width: 300, height: 150 },
            physical_dimensions: None,
        };
        match request.path.as_str() {
            "figures/a.png" => Ok(raster(Some(150))),
            "bare.png" => {
                self.0.fetch_add(1, Ordering::SeqCst);
                Ok(raster(None))
            }
            _ => Err(GraphicsInspectError::new("no such file")),
        }
    }
}

/// The standard package: `\includegraphics` a command of the whole document
/// (`global`), so that an image may stand in a heading's title; the
/// footnote listed after the content and a numbered `sidenote` shown where
/// it stands; the definition `\seenowhere`; the inspector, counting into
/// `asked`.
fn registry(asked: &Arc<AtomicUsize>) -> Arc<FlmRegistry> {
    let sidenote = XnoteKind::numbered("sidenote", PrefixWords::new("Side note~", "Side notes~"))
        .commands(["sidenote"])
        .label_type("sn")
        .placement(XnotePlacement::InPlace);
    let seenowhere = Definition::macro_("seenowhere", Vec::new()).text("(see \\ref{sec:nowhere})");
    let config = StdcoreConfig::new()
        .graphics(GraphicsConfig::new().global(true))
        .xnotes(XnotesConfig::new().kind(sidenote))
        .userdefs(UserdefsConfig::new().definitions(vec![seenowhere]));
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(config));
    builder.use_package(flm_core::math_passthrough::package());
    builder.contribute_to(INSPECTOR_SLOT, Arc::new(Inspector(Arc::clone(asked))) as Arc<dyn GraphicsInspector>);
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

// ---------------------------------------------------------------------------------------------
// runs and their goldens
// ---------------------------------------------------------------------------------------------

/// The four workflows, in the order the diagnostics golden lists them.
const WORKFLOWS: [&str; 4] = [CHECK_WORKFLOW, text::WORKFLOW, html::WORKFLOW, latex_recomposed::WORKFLOW];

/// The run of `workflow` over [`DOCUMENT`].
fn run(registry: &Arc<FlmRegistry>, workflow: &str, options: &RunOptions) -> Run {
    let doc = common::pipeline::doc_with_options(registry, &[DOCUMENT], OptionValues::new());
    let plan = common::stdcore::plan(registry, workflow, &Overrides::none());
    common::pipeline::run_with(&doc, &plan, Externals::none(), options)
}

#[test]
fn every_workflow_writes_the_document_as_its_golden() {
    let registry = registry(&Arc::new(AtomicUsize::new(0)));
    let mut diagnostics = String::new();
    for workflow in WORKFLOWS {
        let run = run(&registry, workflow, &RunOptions::sequential());
        writeln!(diagnostics, "===== {workflow} =====").expect("writing to a string");
        diagnostics.push_str(&common::stdcore::diagnostic_lines(&run));
        match workflow {
            text::WORKFLOW => assert_text_golden(
                common::lowering::golden_path("copies-e2e.txt"),
                &common::lowering::text(&run),
            ),
            html::WORKFLOW => common::html::assert_golden("copies-e2e.page.html", &common::html::page(&run)),
            latex_recomposed::WORKFLOW => common::latex::assert_golden("copies-e2e.tex", &run),
            _ => {}
        }
    }
    assert_text_golden(common::stdcore::golden_path("copies-e2e-diagnostics.txt"), &diagnostics);
}

/// The identifier of each diagnostic of `run` whose frames name the places
/// given, each frame's span starting at the offset of the text it names in
/// [`DOCUMENT`]: `(identifier, [start of each frame])`, sorted.
fn reports_with_frames(run: &Run) -> Vec<(String, Vec<usize>)> {
    let mut found: Vec<(String, Vec<usize>)> = run
        .diagnostics()
        .iter()
        .zip(common::stdcore::frames(run))
        .map(|(diagnostic, frames)| {
            for (title, _) in &frames {
                assert_eq!(title, SHOWN_HERE, "{}", run.diagnostics().render_all());
            }
            (diagnostic.identifier().to_string(), frames.into_iter().map(|(_, at)| at).collect())
        })
        .collect();
    found.sort();
    found
}

/// The offsets in [`DOCUMENT`] of every occurrence of `text`, in order.
fn offsets(text: &str) -> Vec<usize> {
    DOCUMENT.match_indices(text).map(|(at, _)| at).collect()
}

#[test]
fn every_report_is_made_and_one_inside_a_copy_names_where_the_copy_is_shown() {
    let registry = registry(&Arc::new(AtomicUsize::new(0)));
    let shown_e = offsets("\\ref{sec:e}");
    let shown_c = offsets("\\ref{sec:c}");
    let uses = offsets("\\seenowhere{}");
    let unresolved = String::from("flm.refs.unresolved");
    let at = |identifier: &str, frames: Vec<usize>| (format!("flm.graphics.{identifier}"), frames);
    for workflow in [text::WORKFLOW, html::WORKFLOW] {
        let run = run(&registry, workflow, &RunOptions::sequential());
        // The heading's own reference, then one inside each copy of its title
        // a reference shows; the image's two Warnings where it stands and in
        // each copy; the definition's reference at each use site, where the
        // use site is expanded.
        let mut expected = vec![
            (unresolved.clone(), vec![]),
            (unresolved.clone(), vec![shown_e[0]]),
            (unresolved.clone(), vec![shown_e[1]]),
            (unresolved.clone(), vec![]),
            (unresolved.clone(), vec![]),
            at("assumed-resolution", vec![]),
            at("assumed-resolution", vec![shown_c[0]]),
            at("assumed-resolution", vec![shown_c[1]]),
            at("resized", vec![]),
            at("resized", vec![shown_c[0]]),
            at("resized", vec![shown_c[1]]),
        ];
        expected.sort();
        assert_eq!(reports_with_frames(&run), expected, "{workflow}: {}", run.diagnostics().render_all());
    }
    // The recomposed LaTeX writes every reference for LaTeX to resolve, and
    // lowers no copy shown again; the kept definition's template is lowered
    // once per use site, its report naming that use site.
    let recomposed = run(&registry, latex_recomposed::WORKFLOW, &RunOptions::sequential());
    let mut expected = vec![
        (unresolved.clone(), vec![]),
        (unresolved.clone(), vec![uses[0]]),
        (unresolved.clone(), vec![uses[1]]),
        at("assumed-resolution", vec![]),
        at("resized", vec![]),
    ];
    expected.sort();
    assert_eq!(reports_with_frames(&recomposed), expected, "{}", recomposed.diagnostics().render_all());
    // `check` lowers nothing: the heading's reference and the definition's,
    // where the document holds them.
    let checked = run(&registry, CHECK_WORKFLOW, &RunOptions::sequential());
    let expected = vec![(unresolved.clone(), vec![]), (unresolved.clone(), vec![]), (unresolved, vec![])];
    assert_eq!(reports_with_frames(&checked), expected, "{}", checked.diagnostics().render_all());
}

#[test]
fn the_inspector_is_asked_about_the_title_s_image_once_per_tree_holding_it() {
    // Where the image is written, then in each copy of the title a reference
    // shows, on text and HTML; once on the recomposed LaTeX, which lowers no
    // copy; never by `check`, whose plan schedules no inspection.
    for (workflow, times) in
        [(CHECK_WORKFLOW, 0), (text::WORKFLOW, 3), (html::WORKFLOW, 3), (latex_recomposed::WORKFLOW, 1)]
    {
        let asked = Arc::new(AtomicUsize::new(0));
        let _run = run(&registry(&asked), workflow, &RunOptions::sequential());
        assert_eq!(asked.load(Ordering::SeqCst), times, "{workflow}");
    }
}

#[test]
fn a_reference_shows_each_note_as_its_mark_and_every_note_is_listed_once() {
    let registry = registry(&Arc::new(AtomicUsize::new(0)));
    let run = run(&registry, text::WORKFLOW, &RunOptions::sequential());
    let out = common::lowering::text(&run);
    for note in [
        "A note in the title.",
        "A labeled note.",
        "A note in the caption.",
        "A note in the term.",
        "A note in the tag.",
        "A note shown where it stands.",
    ] {
        assert_eq!(out.matches(note).count(), 1, "{note}: {out}");
    }
    let page = common::html::page(&run_page(&registry));
    // Every mark a reference shows is written inside the reference's link,
    // its own link flattened to a span, with no identifier.
    let shown = "<span class=\"flm-pres-link\" data-flm-kind=\"xnote\">";
    assert_eq!(page.matches(shown).count(), 7, "{page}");
    // The labeled note resolves to the note itself.
    assert!(out.contains("its note is Footnote\u{a0}2."), "{out}");
}

/// The run of the `html` workflow over [`DOCUMENT`].
fn run_page(registry: &Arc<FlmRegistry>) -> Run {
    run(registry, html::WORKFLOW, &RunOptions::sequential())
}

/// Everything `run` wrote: every table of the store through its projection,
/// node references resolved through the run's own frozen trees (so that two
/// runs of one document compare equal), then every diagnostic by severity,
/// identifier, span and frames.
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
    out.push_str(&common::stdcore::diagnostic_lines(run));
    out
}

#[test]
fn every_workflow_writes_the_same_tables_sequentially_and_in_parallel() {
    let registry = registry(&Arc::new(AtomicUsize::new(0)));
    for workflow in WORKFLOWS {
        let under = |options: &RunOptions| snapshot(&run(&registry, workflow, options));
        let sequential = under(&RunOptions::sequential());
        let parallel = under(&RunOptions::new());
        assert_eq!(parallel, sequential, "under {workflow}");
    }
}
