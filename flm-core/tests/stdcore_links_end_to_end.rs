//! The `href` and `defterm` features end to end: one document holding
//! hyperlinks and defined terms in every place inline content may stand — a
//! paragraph, a heading's title, a list item, a footnote, `\text{…}` inside a
//! displayed equation, a user definition's template (expanded, and kept on the
//! recomposed LaTeX) — and every shape the two features take: a `\term` inside
//! its own definition, a `\term` before the definition it links to, two
//! definitions linking to each other, a body opening with markup, a `\term`
//! to an alias, `\ref` to a definition's `term:`
//! and `topic:` labels, an `\href` whose text holds a `\term`, a `\url` whose
//! address holds `%` and `#`, a term holding markup, a definition in a list
//! item and in a footnote, a definition written through an expanded user
//! definition and through a kept one, a user definition's placeholder in an
//! address, typed as an ordinary argument and as a verbatim one, and a kept
//! user definition whose term link holds a placeholder. It is rendered by the
//! standard package's four workflows (`text`, `html`, `latex-recomposed`,
//! `check`) under three configurations of the defined-term feature's lead-in:
//!
//! - the default one, `#{term}: `;
//! - a richer template, `\textbf{Definition} (\emph{#{term}}): `;
//! - none (`lead_in: None`): a definition is its body alone.
//!
//! A second, shorter document holds the cases where the run reports a problem
//! about a link or a term, under the default configuration: a term link that
//! resolves to no definition, plain and holding markup, and a definition
//! standing in a heading's title, where no block can stand. Its recomposed
//! LaTeX does not compile cleanly, and `tests/latex_compile.rs` skips it with
//! the reason.
//!
//! Each run is a golden: the text output under `tests/goldens/text/`, the page
//! under `tests/goldens/html/` (parsed by `tests/html_validity.rs`, which also
//! holds every link to an identifier of the page), the recomposed LaTeX and
//! its style file under `tests/goldens/latex/` (compiled by
//! `tests/latex_compile.rs` under both engines when `FLM_TESTS_LATEX_COMPILE`
//! is set), and every run's diagnostics under `tests/goldens/stdcore/`. The
//! last test runs each workflow sequentially and in parallel and compares
//! every table the runs wrote; the full battery runs this suite a third time on
//! a pool of one thread (`RAYON_NUM_THREADS=1`), against the same goldens.

mod common;

use std::fmt::Write as _;
use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::golden::assert_text_golden;
use flm_core::dump::project::{ProjectOptions, Projector};
use flm_core::dump::to_json_pretty;
use flm_core::lang::mint::{ArgDef, ArgStdKind};
use flm_core::lang::rewrites::userdefs::{Definition, ExpandPolicy};
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::defterm::DeftermConfig;
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig, package};

use common::stdcore::run_workflow;

// ---------------------------------------------------------------------------------------------
// the documents
// ---------------------------------------------------------------------------------------------

/// The document every configuration renders: hyperlinks and defined terms in
/// each place inline content may stand, and in each shape. The user
/// definitions it calls are [`definitions`]'s.
const DOCUMENT: &str = "\
\\section{Links in a title: \\href{https://example.com/a}{the site} and \\url{https://example.com/b}}

A paragraph holds \\href{https://example.com}{a hyperlink}, an address \
\\url{https://example.com/100%25/a_b~c?q=1&r=2#top} and \\email{someone@example.com}; \\term{entropy} is \
defined below.

\\begin{defterm}{Markov chain}\\label{topic:markov}\\aliasterm{Markov chains}
A random process; a \\term{Markov chain} forgets its past, as all \\term{Markov chains} do, and may \
produce \\term{entropy}.
\\end{defterm}

\\begin{defterm}{entropy}\\label{term:entropy}
\\emph{A measure} of disorder, which a \\term[Markov chain]{process} may produce.
\\end{defterm}

\\begin{defterm}{\\(\\alpha\\)-decay}
A decay that emits an \\(\\alpha\\) particle.
\\end{defterm}

See \\term{entropy}, \\term[entropy]{the entropy}, \\term{Markov chains}, \
\\term{\\(\\alpha\\)-decay}, \\ref{topic:markov} and \\ref{term:entropy}; \
\\href{https://example.com/g}{a page on \\term{entropy}}.

\\section{A heading about \\term[entropy]{disorder}}

\\begin{itemize}
\\item An item holds \\href{https://example.org}{a link} and \\term{entropy}.
\\item \\begin{defterm}{list term}
A term defined in a list item, where \\term{list term} is itself.
\\end{defterm}
\\end{itemize}

A note holds links.\\footnote{The note holds \\url{https://example.net/} and \\term{entropy}.} \
Another defines a term.\\footnote{\\begin{defterm}{note term}
A term defined in a footnote.
\\end{defterm}} See \\term{note term} and \\term{list term}.

\\begin{equation}
S = k \\log W \\text{ where \\href{https://example.com/f}{a link} and \\term{entropy} stand}
\\end{equation}

Through definitions: \\seeentropy{}; \\doi{10.1000/xyz-1}; \\anchor{intro}; \
\\vdoi{10.1000/a%20b#c}; \\linkto{Markov chain}.

\\dt{expanded term}

\\kdt{kept term}

See \\term{expanded term}.";

/// The second document, of the cases where the run reports a problem about a
/// link or a term, rendered under the default configuration: a term link that
/// resolves to no definition, plain and holding markup, a definition in a
/// heading's title, where no block can stand, and a reference to nothing in an
/// unnumbered heading's title, which a reference to that heading shows again
/// (reported there too, naming the reference). It is a document of its own,
/// and not part of [`DOCUMENT`], because its recomposed LaTeX does not compile
/// cleanly (see `NOT_COMPILED` in `tests/latex_compile.rs`), while every
/// golden of [`DOCUMENT`] does.
const REPORTED: &str = "\
\\section{A title defines \\begin{defterm}{title term}Defined in a title.\\end{defterm}}

A term nobody defines: \\term{nothing}, and one holding markup: \\term{\\emph{no} such term}.

\\section*{See \\ref{sec:nowhere}}\\label{sec:shown}

Shown again: \\ref{sec:shown}.";

// ---------------------------------------------------------------------------------------------
// the user definitions and the configurations
// ---------------------------------------------------------------------------------------------

/// The user definitions [`DOCUMENT`] calls, each written back as a LaTeX
/// definition on the recomposed LaTeX unless said otherwise:
///
/// - `\seeentropy`: a term link with a fixed target;
/// - `\doi` and `\anchor`: a placeholder in an address, an ordinary argument;
/// - `\vdoi`: a placeholder in an address, a verbatim-typed argument, so that
///   the use site may write `%` and `#` (not recomposable: expanded at each
///   use site);
/// - `\linkto`: a term link whose target is a placeholder (not recomposable:
///   expanded at each use site);
/// - `\dt`: a definition, expanded early on every output (`always`);
/// - `\kdt`: the same definition kept, which the recomposed LaTeX refuses,
///   since a template lowered as a value declares no reference point.
fn definitions() -> Vec<Definition> {
    let verbatim = ArgStdKind::Verbatim { delimiters: Some(('{', '}')) };
    let definition_template = "\\begin{defterm}{#1}\nA term defined through a user definition.\n\\end{defterm}";
    vec![
        Definition::macro_("seeentropy", Vec::new()).text("see \\term{entropy}"),
        Definition::macro_("doi", vec![ArgDef::mandatory("id")]).text("\\href{https://doi.org/#1}{#1}"),
        Definition::macro_("anchor", vec![ArgDef::mandatory("frag")]).text("\\url{https://example.com/c/##sec-#1}"),
        Definition::macro_("vdoi", vec![ArgDef::named("id", verbatim)]).text("\\href{https://doi.org/#1}{DOI}"),
        Definition::macro_("linkto", vec![ArgDef::mandatory("term")]).text("\\term[#1]{this term}"),
        Definition::macro_("dt", vec![ArgDef::mandatory("term")])
            .text(definition_template)
            .expand(ExpandPolicy::Always),
        Definition::macro_("kdt", vec![ArgDef::mandatory("term")])
            .text(definition_template)
            .expand(ExpandPolicy::PerPlan),
    ]
}

/// One configuration of the defined-term feature, with the document it
/// renders and the stem of its goldens.
struct Case {
    /// The stem of the goldens: `links-e2e-<name>`.
    name: &'static str,
    /// The standard package, the defined-term feature under this configuration.
    registry: Arc<FlmRegistry>,
    /// The document.
    document: &'static str,
}

/// The standard package, with [`definitions`] and its defined-term feature
/// under `defterm`.
fn registry(defterm: DeftermConfig) -> Arc<FlmRegistry> {
    let config = StdcoreConfig::new()
        .userdefs(UserdefsConfig::new().definitions(definitions()))
        .defterm(defterm);
    let mut builder = RegistryBuilder::new();
    builder.use_package(package(config));
    builder.use_package(flm_core::math_passthrough::package());
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

/// The three configurations over [`DOCUMENT`], then the default one over
/// [`REPORTED`].
fn cases() -> Vec<Case> {
    let richer = DeftermConfig::new()
        .lead_in(Some(String::from("\\textbf{Definition} (\\emph{#{term}}): ")));
    vec![
        Case { name: "default", registry: registry(DeftermConfig::new()), document: DOCUMENT },
        Case { name: "richer", registry: registry(richer), document: DOCUMENT },
        Case { name: "no-lead-in", registry: registry(DeftermConfig::new().lead_in(None)), document: DOCUMENT },
        Case { name: "reported", registry: registry(DeftermConfig::new()), document: REPORTED },
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
    run_workflow(&case.registry, workflow, &[case.document], &Overrides::none())
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
                    common::lowering::golden_path(&format!("links-e2e-{}.txt", case.name)),
                    &common::lowering::text(&run),
                ),
                html::WORKFLOW => common::html::assert_golden(
                    &format!("links-e2e-{}.page.html", case.name),
                    &common::html::page(&run),
                ),
                latex_recomposed::WORKFLOW => {
                    common::latex::assert_golden(&format!("links-e2e-{}.tex", case.name), &run)
                }
                _ => {}
            }
        }
        assert_text_golden(
            common::stdcore::golden_path(&format!("links-e2e-{}-diagnostics.txt", case.name)),
            &diagnostics,
        );
    }
}

/// The term link inside the `\href`'s text is a link inside a link: the page
/// writes it as its text, a span with the link's class and kind and no
/// address, inside the `\href`'s link, and reports nothing.
#[test]
fn the_term_link_inside_an_href_is_written_as_its_text_on_the_page() {
    let case = cases().into_iter().find(|case| case.name == "default").expect("the default case");
    let run = run(&case, html::WORKFLOW);
    assert!(
        run.diagnostics().iter().all(|d| !d.identifier().starts_with("flm.html.")),
        "{}",
        run.diagnostics().render_all()
    );
    let page = common::html::page(&run);
    assert!(
        page.contains(
            "<a class=\"flm-pres-link\" href=\"https://example.com/g\" data-flm-kind=\"url\">a page on \
             <span class=\"flm-pres-link\" data-flm-kind=\"term\">entropy</span></a>"
        ),
        "{page}"
    );
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
        let doc = common::pipeline::doc(&case.registry, &[case.document]);
        for workflow in WORKFLOWS {
            let plan = common::stdcore::plan(&case.registry, workflow, &Overrides::none());
            let under = |options: &RunOptions| {
                snapshot(&common::pipeline::run_with(&doc, &plan, Externals::none(), options))
            };
            let sequential = under(&RunOptions::sequential());
            assert_eq!(under(&RunOptions::new()), sequential, "{} under `{workflow}`", case.name);
            assert!(
                sequential.contains("flm.refs.resolutions = "),
                "the snapshot holds the resolutions of the term links ({} under `{workflow}`):\n{sequential}",
                case.name
            );
        }
    }
}
