//! Equation numbers: the counter the `math` feature declares, the number a
//! numbered line shows on the text and the HTML backends, and `\eqref`.
//!
//! Everything here runs over the standard package — no test feature — because
//! the mathematics feature declares its own counter (`equation`) and its own
//! counter style (`flm.math.style.equation`), and numbers equations unless its
//! configuration says otherwise.
//!
//! What is pinned: which lines take a number and which take none; that a line
//! shows its number exactly where a line whose author wrote a `\tag` shows the
//! tag, and by the same mechanism; what `\ref` and `\eqref` show for a
//! numbered line; and what the recomposed LaTeX writes for an `\eqref`.

mod common;

use std::sync::Arc;

use techy::error::Severity;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmFeature;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::dump::diag::expect;
use flm_core::model::aspects::{Label, PointId};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::{html, latex_recomposed, text};
use flm_core::stdcore::features::headings::{
    HeadingDecl, HeadingNumbering, HeadingsConfig, LEVELS_PARENT_SET,
};
use flm_core::stdcore::features::math::{self, MathConfig};
use flm_core::stdcore::features::numbering::{COUNTERS, JoinDecl, NumberingOptions, Within};
use flm_core::stdcore::features::refs::{REF_TARGETS, RefsConfig};

use common::html::fragment;
use common::lowering::text as text_of;
use common::stdcore::{identifiers, plan, registry, reports, run_workflow};

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// Renders `document` under the `text` workflow of the standard package.
fn render_text(document: &str) -> Run {
    run_workflow(&std(), text::WORKFLOW, &[document], &Overrides::none())
}

/// The standard package with its sections numbered, and equations numbered
/// or not as `config` says.
fn with_sections(config: MathConfig) -> Arc<FlmRegistry> {
    registry(
        StdcoreConfig::new()
            .headings(HeadingsConfig::new().numbering(HeadingNumbering::UpToDepth(1)))
            .math(config),
    )
}

/// Renders `document` under the `text` workflow of `registry` with `options`
/// set on the document, which is how a document says what its counters are
/// numbered within.
fn render_text_with(registry: &Arc<FlmRegistry>, document: &str, options: NumberingOptions) -> Run {
    let doc = common::pipeline::doc_with_options(
        registry,
        &[document],
        OptionValues::new().with(options),
    );
    let plan = plan(registry, text::WORKFLOW, &Overrides::none());
    common::pipeline::run_with(&doc, &plan, Externals::none(), &RunOptions::sequential())
}

/// Renders `document` under the `html` workflow of the standard package.
fn render_html(document: &str) -> Run {
    run_workflow(&std(), html::WORKFLOW, &[document], &Overrides::none())
}

/// The value the numbering stage recorded for each equation line of the first
/// environment of `run`, in line order: the numbers as the record holds them,
/// `None` for a line the stage saw and did not number.
fn line_values(run: &Run, points: usize) -> Vec<Option<Vec<i64>>> {
    let counters = run.table(&COUNTERS).expect("declared").expect("written");
    let addr = run
        .table(&REF_TARGETS)
        .expect("declared")
        .expect("written")
        .by_label
        .values()
        .map(|target| target.point.addr)
        .next()
        .unwrap_or_else(|| environment_addr(run));
    (0..points)
        .map(|index| {
            counters
                .record(PointId { addr, index: index as u32 })
                .map(|record| record.value.components().to_vec())
        })
        .collect()
}

/// The address of the first display-math environment of the run's first tree,
/// for a document whose lines carry no label at all.
fn environment_addr(run: &Run) -> flm_core::model::addr::NodeAddr {
    let counters = run.table(&COUNTERS).expect("declared").expect("written");
    counters
        .points()
        .next()
        .map(|(point, _)| point.addr)
        .expect("the stage saw at least one reference point")
}

// ---------------------------------------------------------------------------------------------
// what the feature declares
// ---------------------------------------------------------------------------------------------

#[test]
fn the_equation_counter_stands_on_its_own() {
    let decl = math::counter_decl();
    assert_eq!(decl.name, math::COUNTER);
    assert_eq!(decl.default_style, math::COUNTER_STYLE);
    assert!(decl.within.is_none(), "no hierarchy by default: a document asks for one");
    assert!(MathConfig::new().numbered_equations, "equations are numbered by default");
}

#[test]
fn the_equation_style_is_the_number_in_parentheses_with_its_word() {
    let style = math::counter_style();
    assert_eq!(style.name, math::COUNTER_STYLE);
    assert_eq!((style.delimiters.open.as_str(), style.delimiters.close.as_str()), ("(", ")"));
    assert_eq!(style.prefix_words.base.one, "Eq.~");
    assert_eq!(style.prefix_words.base.other, "Eqs.~");
    let capital = &style.prefix_words.variants["capital"];
    assert_eq!((capital.one.as_str(), capital.other.as_str()), ("Equation~", "Equations~"));
    assert!(style.sub_levels.is_empty(), "sub-numbered equations are not built");
}

/// The Warning every document with a numbered equation line carried until the
/// feature declared its counter: a document renders without a diagnostic now.
#[test]
fn a_document_with_a_numbered_equation_reports_nothing() {
    let run = render_text("\\begin{equation}\n  E = mc^2\n\\end{equation}\n");
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

// ---------------------------------------------------------------------------------------------
// which lines take a number
// ---------------------------------------------------------------------------------------------

#[test]
fn every_line_of_a_numbering_environment_takes_the_next_number() {
    let run = render_text(
        "\\begin{align}\n  a &= b \\\\\n  c &= d\n\\end{align}\n\n\
         \\begin{equation}\n  e = f\n\\end{equation}\n",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let text = text_of(&run);
    assert!(text.contains("\\tag*{(1)}"), "{text}");
    assert!(text.contains("\\tag*{(2)}"), "{text}");
    assert!(text.contains("\\tag*{(3)}"), "{text}");
}

/// The lines of `equation*`, `align*` and `gather*` take no number, and
/// neither does a line marked `\nonumber`. The Python implementation numbers
/// them, which is a defect this design does not reproduce.
#[test]
fn the_lines_that_carry_no_number_take_none_and_are_recorded_as_unnumbered() {
    for document in [
        "\\begin{equation*}\n  a = b\n\\end{equation*}\n",
        "\\begin{align*}\n  a &= b \\\\\n  c &= d\n\\end{align*}\n",
        "\\begin{gather*}\n  a = b\n\\end{gather*}\n",
        "\\begin{equation}\n  a = b \\nonumber\n\\end{equation}\n",
    ] {
        let run = render_text(document);
        assert_eq!(identifiers(&run), Vec::<String>::new(), "{document}");
        let text = text_of(&run);
        assert!(!text.contains("\\tag"), "{document}: {text}");
        let counters = run.table(&COUNTERS).expect("declared").expect("written");
        assert!(
            counters.points().all(|(_, record)| record.is_none()),
            "{document}: a line that carries no number gets no record"
        );
    }
}

/// A line whose author wrote a `\tag` counts in nothing: the next numbered
/// line takes the number the tagged line did not.
#[test]
fn a_tagged_line_takes_no_number_and_advances_the_counter_by_nothing() {
    let run = render_text(
        "\\begin{align}\n  a &= b \\tag{A} \\\\\n  c &= d\n\\end{align}\n",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let text = text_of(&run);
    assert!(text.contains("\\tag*{(A)}"), "{text}");
    assert!(text.contains("\\tag*{(1)}"), "{text}");
    assert_eq!(line_values(&run, 2), [None, Some(vec![1])]);
}

/// Every line has an identifier, numbered or not: a link from outside can
/// reach any line of any document.
#[test]
fn every_line_has_an_identifier_whether_or_not_it_is_numbered() {
    let run = render_html(
        "\\begin{align}\n  a &= b \\label{eq:a} \\\\\n  c &= d \\nonumber\n\\end{align}\n\n\
         \\begin{align*}\n  e &= f\n\\end{align*}\n",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let html = fragment(&run);
    assert!(html.contains("id=\"eq-a\""), "{html}");
    for position in [2, 3] {
        let id = flm_core::stdcore::features::refs::auto_anchor(math::ANCHOR_PREFIX, position);
        assert!(html.contains(&format!("id=\"{id}\"")), "line {position}: {html}");
    }
}

/// A registry configured to number no equation numbers none at all, its
/// headings still numbered, and every line is still a reference point with an
/// identifier of its own.
#[test]
fn a_registry_may_number_no_equation() {
    let run = render_text_with(
        &with_sections(MathConfig::new().numbered_equations(false)),
        "\\section{A}\n\n\\begin{equation}\n  a = b \\label{eq:a}\n\\end{equation}\n\n\
         See \\ref{eq:a}.\n",
        NumberingOptions::new(),
    );
    // Nothing numbered the line and it states no content, so the reference
    // has nothing to show.
    assert_eq!(identifiers(&run), ["flm.refs.nothing-to-show"]);
    let text = text_of(&run);
    assert!(text.starts_with("1. A\n"), "{text}");
    assert!(!text.contains("\\tag"), "{text}");
    // Nothing numbered it and the line states no content, so the reference
    // shows the label as the document wrote it.
    assert!(text.contains("See eq:a."), "{text}");
}

// ---------------------------------------------------------------------------------------------
// the number where the tag stands
// ---------------------------------------------------------------------------------------------

/// On HTML a line's number is page content inside the formula, exactly as an
/// author's tag is: `\tag*{$…$}` around the element the emitter writes for
/// content standing in a formula.
#[test]
fn on_html_a_number_and_a_tag_are_written_by_the_one_mechanism() {
    let numbered = fragment(&render_html("\\begin{equation}\n  a = b\n\\end{equation}\n"));
    let tagged =
        fragment(&render_html("\\begin{equation}\n  a = b \\tag*{A}\n\\end{equation}\n"));
    assert!(
        numbered.contains(
            "\\tag*{$<tex-html><span class=\"flm-pres-math--text\">(1)</span></tex-html>$}"
        ),
        "{numbered}"
    );
    assert!(
        tagged.contains(
            "\\tag*{$<tex-html><span class=\"flm-pres-math--text\">A</span></tex-html>$}"
        ),
        "{tagged}"
    );
}

/// A tag holding markup reaches the page as markup, which is what writing it
/// as content inside the formula is for.
#[test]
fn a_tag_holding_markup_reaches_an_html_page_as_markup() {
    let html = fragment(&render_html(
        "\\begin{equation}\n  a = b \\tag{\\emph{a}}\n\\end{equation}\n",
    ));
    assert!(html.contains("\\tag*{$<tex-html>"), "{html}");
    assert!(html.contains("<em class=\"flm-pres-span\" data-flm-style=\"emph\">a</em>"), "{html}");
    assert!(html.contains("</tex-html>$}"), "{html}");
}

// ---------------------------------------------------------------------------------------------
// what a reference shows
// ---------------------------------------------------------------------------------------------

/// `\ref` to a numbered line shows the word and the number; `\ref[S]` the
/// capital wording; `\eqref` the number alone.
#[test]
fn a_reference_to_a_numbered_line_shows_the_number_under_its_options() {
    let run = render_text(
        "\\begin{align}\n  x = 1 \\label{eq:a}\n\\end{align}\n\n\
         See \\ref{eq:a}, \\ref[S]{eq:a} and \\eqref{eq:a}.\n",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let text = text_of(&run);
    assert!(
        text.contains("See Eq.\u{a0}(1), Equation\u{a0}(1) and (1)."),
        "{text}"
    );
}

/// Two environments, each one line: the numbers follow the document's order,
/// and each reference shows its own target's.
#[test]
fn two_labeled_equations_are_numbered_in_document_order() {
    let run = render_text(
        "\\begin{align}\n  x = 1 \\label{eq:first}\n\\end{align}\n\n\
         \\begin{align}\n  y = 2 \\label{eq:second}\n\\end{align}\n\n\
         First: \\ref{eq:first}. Second: \\ref{eq:second}.\n",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let text = text_of(&run);
    assert!(
        text.contains("First: Eq.\u{a0}(1). Second: Eq.\u{a0}(2)."),
        "{text}"
    );
}

/// Several equations in one `\ref`: the numbers are written as one list,
/// which is the formatter's business and is pinned here over the equation
/// style the mathematics feature ships.
#[test]
fn one_reference_naming_two_equations_writes_them_as_one_list() {
    let run = render_text(
        "\\begin{align}\n  x = 1 \\label{eq:a}\n\\end{align}\n\n\
         \\begin{align}\n  y = 2 \\label{eq:b}\n\\end{align}\n\n\
         See \\ref{eq:a,eq:b}.\n",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let text = text_of(&run);
    assert!(text.contains("See Eqs.\u{a0}(1,2)."), "{text}");
}

/// A reference to a line whose author wrote a `\tag` shows the tag's content:
/// the author said what that line is called.
#[test]
fn a_reference_to_a_tagged_line_shows_the_tag() {
    let run = render_text(
        "\\begin{equation}\n  a = b \\tag{\\emph{star}} \\label{eq:s}\n\\end{equation}\n\n\
         See \\ref{eq:s} and \\eqref{eq:s}.\n",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let text = text_of(&run);
    assert!(text.contains("See star and star."), "{text}");
}

// ---------------------------------------------------------------------------------------------
// `\eqref`
// ---------------------------------------------------------------------------------------------

#[test]
fn an_eqref_links_to_the_line_it_names() {
    let html = fragment(&render_html(
        "\\begin{align}\n  x = 1 \\label{eq:a}\n\\end{align}\n\nSee \\eqref{eq:a}.\n",
    ));
    let link = "<a class=\"flm-pres-link\" href=\"#eq-a\" data-flm-kind=\"ref\">(1)</a>";
    assert!(html.contains(link), "{html}");
}

/// A one-target `\ref` is a list of one item and takes the list texts
/// `one_pre` and `one_post` of [`RefsConfig`]; `\eqref` of the same target
/// writes its one item bare, the number and nothing else.
#[test]
fn an_eqref_does_not_take_the_texts_around_a_one_item_list() {
    let join = JoinDecl {
        one_pre: Some("[".to_string()),
        one_post: Some("]".to_string()),
        ..JoinDecl::default()
    };
    let bracketed = registry(StdcoreConfig::new().refs(RefsConfig::new().join(join)));
    let run = run_workflow(
        &bracketed,
        text::WORKFLOW,
        &["\\begin{align}\n  x = 1 \\label{eq:a}\n\\end{align}\n\n\
           See \\ref{eq:a} and \\eqref{eq:a}.\n"],
        &Overrides::none(),
    );
    let text = text_of(&run);
    assert!(text.contains("See [Eq.\u{a0}(1)] and (1)."), "{text}");
}

/// `\eqref` refers to equations: a target of any other reference type is an
/// Error at the command, and the command then shows the label as written.
#[test]
fn an_eqref_naming_something_that_is_not_an_equation_is_an_error() {
    let run = render_text(
        "\\section{Intro}\\label{sec:intro}\n\nSee \\eqref{sec:intro}.\n",
    );
    expect(run.diagnostics().iter(), "flm.math.eqref-not-an-equation")
        .severity(Severity::Error)
        .once();
    let text = text_of(&run);
    assert!(text.contains("See sec:intro."), "{text}");
}

/// A target that is not spelled `type:label` at all is the same Error: it
/// names no equation either.
#[test]
fn an_eqref_naming_no_label_at_all_is_the_same_error() {
    let run = render_text("See \\eqref{nothing}.\n");
    assert_eq!(
        reports(&run),
        [(String::from("flm.math.eqref-not-an-equation"), Severity::Error)]
    );
    assert!(text_of(&run).contains("See nothing."), "{}", text_of(&run));
}

/// The same Error on the backend that recomposes LaTeX, where the reference
/// is written as the document wrote it rather than as the name allocated for
/// the heading that does carry the label: a recomposed `\eqref` can never
/// come to print a section's number in an equation's parentheses.
#[test]
fn an_eqref_naming_something_that_is_not_an_equation_is_an_error_on_every_backend() {
    let run = run_workflow(
        &std(),
        latex_recomposed::WORKFLOW,
        &["\\section{Intro}\\label{sec:intro}\n\nSee \\eqref{sec:intro}.\n"],
        &Overrides::none(),
    );
    expect(run.diagnostics().iter(), "flm.math.eqref-not-an-equation")
        .severity(Severity::Error)
        .once();
    let latex = common::latex::output(&run);
    assert!(latex.contains("\\eqref{sec:intro}"), "{latex}");
    // The heading allocated a safe key for that label; the reference is not
    // written with it.
    assert!(latex.contains("\\label{ref1}"), "{latex}");
    assert!(!latex.contains("\\eqref{ref1}"), "{latex}");
}

/// A target no equation line carries is an unresolved reference, reported by
/// the references feature exactly as an unresolved `\ref` is.
#[test]
fn an_eqref_to_a_label_nothing_carries_is_unresolved() {
    let run = render_text("See \\eqref{eq:missing}.\n");
    expect(run.diagnostics().iter(), "flm.refs.unresolved").severity(Severity::Error).once();
    assert!(text_of(&run).contains("See eq:missing."), "{}", text_of(&run));
}

/// On the `latex-recomposed` backend an `\eqref` is written as `\eqref`
/// around the safe key of its target, which is the same key the line's own
/// `\label` is written with, and the `amsmath` package is requested.
#[test]
fn an_eqref_recomposes_to_eqref_around_the_safe_key() {
    let run = run_workflow(
        &std(),
        latex_recomposed::WORKFLOW,
        &[
            "\\begin{equation}\n  a = b \\label{eq:test}\n\\end{equation}\n\n\
             See \\eqref{eq:test}.\n",
        ],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let latex = common::latex::output(&run);
    assert!(latex.contains("\\eqref{ref1}"), "{latex}");
    assert!(latex.contains("\\label{ref1}"), "{latex}");
    // The command is the `amsmath` package's, so the style file requests it.
    let (style, _) = common::latex::style(&run);
    assert!(style.contains("amsmath"), "{style}");
    // The golden is compiled by `tests/latex_compile.rs`, which compiles
    // every golden of this directory with both engines: that is what proves
    // the reference resolves in a compiled document.
    common::latex::assert_golden("math-eqref.tex", &run);
}

// ---------------------------------------------------------------------------------------------
// registries that hold neither feature, one of them, or both
// ---------------------------------------------------------------------------------------------

/// The document the three tests below render: one numbered line carrying a
/// label, and an `\eqref` naming it.
const A_LINE_AND_AN_EQREF: &str =
    "\\begin{equation}\n  E = mc^2 \\label{eq:e}\n\\end{equation}\n\nSee \\eqref{eq:e}.\n";

/// A lean registry: the mathematics feature, the block structure every
/// document has, the lowering machinery, the `check` feature the workflows
/// demand and the text backend — plus the features `extra` names.
fn lean(extra: Vec<Arc<dyn FlmFeature>>) -> Arc<FlmRegistry> {
    let mut features: Vec<Arc<dyn FlmFeature>> = vec![
        math::feature(),
        flm_core::math_passthrough::feature(),
        Arc::new(flm_core::stdcore::features::blocks::Blocks::default()),
        flm_core::stdcore::backends::lowering::feature(),
        flm_core::stdcore::check_feature(),
        flm_core::stdcore::backends::pieces::feature(),
        text::feature(),
    ];
    features.extend(extra);
    common::pipeline::registry(features)
}

/// Renders `document` under the `text` workflow of `registry`.
fn render_text_over(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, text::WORKFLOW, &[document], &Overrides::none())
}

/// Without the references feature nothing resolves a reference, so an
/// `\eqref` shows the label the document wrote, unlinked, and reports
/// nothing: the restriction on the target's type is about what the author
/// wrote, not about what a registry can resolve. The counter still counts,
/// because the feature that numbers reads the element index and not the
/// references feature.
#[test]
fn an_eqref_shows_its_label_where_nothing_resolves_references() {
    let registry = lean(vec![
        flm_core::stdcore::features::index::feature(),
        flm_core::stdcore::features::refpoints::feature(),
        flm_core::stdcore::features::numbering::feature(),
    ]);
    assert!(!registry.has_feature("refs"));
    let run = render_text_over(&registry, A_LINE_AND_AN_EQREF);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let text = text_of(&run);
    assert!(text.contains("See eq:e."), "{text}");
    assert!(text.contains("\\tag*{(1)}"), "{text}");
}

/// Without the feature that numbers, no line takes a number, so an `\eqref`
/// shows what a reference shows for a target with no number and no content:
/// the label the document wrote, reported as a target with nothing to show.
#[test]
fn an_eqref_shows_its_label_where_nothing_numbers() {
    let registry = lean(vec![
        flm_core::stdcore::features::index::feature(),
        flm_core::stdcore::features::refpoints::feature(),
        flm_core::stdcore::features::refs::feature(),
    ]);
    assert!(!registry.has_feature("numbering"));
    let run = render_text_over(&registry, A_LINE_AND_AN_EQREF);
    assert_eq!(identifiers(&run), ["flm.refs.nothing-to-show"]);
    let text = text_of(&run);
    assert!(text.contains("See eq:e."), "{text}");
    assert!(!text.contains("\\tag*"), "{text}");
}

/// Without either feature the command still renders, and the line carries
/// neither a number nor a tag.
#[test]
fn an_eqref_renders_where_neither_feature_is_registered() {
    let registry = lean(Vec::new());
    assert!(!registry.has_feature("refs") && !registry.has_feature("numbering"));
    let run = render_text_over(&registry, A_LINE_AND_AN_EQREF);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let text = text_of(&run);
    assert!(text.contains("See eq:e."), "{text}");
    assert!(!text.contains("\\tag*"), "{text}");
}

// ---------------------------------------------------------------------------------------------
// the integration cases of the Python suite
// ---------------------------------------------------------------------------------------------

/// Equations numbered on their own: `(1)`, `(2)`.
#[test]
fn equations_without_a_hierarchy_are_numbered_one_two() {
    let run = render_text(
        "\\section{First}\n\n\\begin{align}\n  a = 1 \\label{eq:a}\n\\end{align}\n\n\
         \\begin{align}\n  b = 2 \\label{eq:b}\n\\end{align}\n\n\
         See \\eqref{eq:a} and \\eqref{eq:b}.\n",
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let text = text_of(&run);
    assert!(text.contains("\\tag*{(1)}"), "{text}");
    assert!(text.contains("\\tag*{(2)}"), "{text}");
    assert!(text.contains("See (1) and (2)."), "{text}");
}

/// Equations numbered within sections: `(1.1)`, `(1.2)`, `(2.1)`.
///
/// A document that wants a section's number before an equation's asks, in its
/// numbering options, for the `flm.math.equation` counter numbered within
/// `flm.headings.section`, which writes the section's number and a full stop
/// before the equation's own; the registry numbers its sections. The equation counter then restarts
/// at each section, because a counter's count is kept per list of its
/// ancestors and their values.
#[test]
fn equations_within_sections_are_numbered_by_section() {
    let run = render_text_with(
        &with_sections(MathConfig::new()),
        "\\section{First}\n\n\\begin{equation}\n  E = mc^2 \\label{eq:a}\n\\end{equation}\n\n\
         \\begin{equation}\n  F = ma \\label{eq:b}\n\\end{equation}\n\n\
         \\section{Second}\n\n\
         \\begin{equation}\n  x^2 + y^2 = z^2 \\label{eq:c}\n\\end{equation}\n\n\
         See \\eqref{eq:b} and \\eqref{eq:c}.\n",
        NumberingOptions::new().within("flm.math.equation", Within::parent("flm.headings.section")),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let text = text_of(&run);
    assert!(text.contains("\\tag*{(1.1)}"), "{text}");
    assert!(text.contains("\\tag*{(1.2)}"), "{text}");
    assert!(text.contains("\\tag*{(2.1)}"), "{text}");
    // A reference shows the same number the line shows, the section's number
    // and all.
    assert!(text.contains("See (1.2) and (2.1)."), "{text}");
}

// ---------------------------------------------------------------------------------------------
// the same numbers wherever the document is rendered
// ---------------------------------------------------------------------------------------------

/// One document, three backends: the numbers are the document's, not the
/// backend's. The recomposed LaTeX is the exception the design states — it
/// leaves numbering to LaTeX — so what is pinned there is the label, not the
/// number.
#[test]
fn the_text_and_html_backends_show_the_same_numbers() {
    let document =
        "\\begin{align}\n  a &= b \\\\\n  c &= d \\label{eq:c}\n\\end{align}\n\nSee \\ref{eq:c}.\n";
    let text = text_of(&render_text(document));
    let html = fragment(&render_html(document));
    assert!(text.contains("\\tag*{(1)}") && text.contains("\\tag*{(2)}"), "{text}");
    assert!(html.contains(">(1)<") && html.contains(">(2)<"), "{html}");
    assert!(text.contains("See Eq.\u{a0}(2)."), "{text}");
    assert!(html.contains(">Eq.&#xA0;(2)</a>"), "{html}");
}

/// A label on a line the document numbers resolves to that line, and the
/// target's record is the one the stage wrote for it.
#[test]
fn the_record_of_a_labeled_line_is_the_number_it_shows() {
    let run = render_text(
        "\\begin{align}\n  a &= b \\\\\n  c &= d \\label{eq:c}\n\\end{align}\n",
    );
    let targets = run.table(&REF_TARGETS).expect("declared").expect("written");
    let target = targets.by_label.get(&Label::new("eq", "c")).expect("a target");
    assert_eq!(target.point.index, 1, "the second line");
    let counters = run.table(&COUNTERS).expect("declared").expect("written");
    let record = counters.record(target.point).expect("a record");
    assert_eq!(record.counter.as_str(), "flm.math.equation");
    assert_eq!(record.value.components(), [2]);
    assert_eq!(record.style.name().as_str(), math::COUNTER_STYLE);
}

/// Every tree of a document counts in one sequence, whatever workflow ran:
/// the check workflow reads the same table and reports nothing.
#[test]
fn the_check_workflow_numbers_the_lines_and_reports_nothing() {
    let run = run_workflow(
        &std(),
        flm_core::stdcore::CHECK_WORKFLOW,
        &["\\begin{align}\n  a &= b \\\\\n  c &= d \\label{eq:c}\n\\end{align}\n"],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(line_values(&run, 2), [Some(vec![1]), Some(vec![2])]);
}

// ---------------------------------------------------------------------------------------------
// equations within the heading levels
// ---------------------------------------------------------------------------------------------

/// One `equation` holding `x`, as a line of source.
const EQ: &str = "\\begin{equation}\n  x\n\\end{equation}\n\n";

/// The numbers the lines of `text` were written with, in order.
fn tags(text: &str) -> Vec<String> {
    text.match_indices("\\tag*{")
        .map(|(start, _)| {
            let rest = &text[start + "\\tag*{".len()..];
            rest[..rest.find('}').expect("a closing brace")].to_string()
        })
        .collect()
}

#[test]
fn equations_within_the_heading_levels_follow_the_heading_that_applies() {
    // A book-like registry: chapters, sections and subsections, all numbered.
    // Equations are numbered within whichever of them came last, through the
    // parent set the headings feature registers.
    let kinds = vec![
        HeadingDecl::new("chapter", 1),
        HeadingDecl::new("section", 2).within(Within::parent("flm.headings.chapter")),
        HeadingDecl::new("subsection", 3).within(Within::parent("flm.headings.section")),
    ];
    let registry = registry(
        StdcoreConfig::new()
            .headings(
                HeadingsConfig::new().headings(kinds).numbering(HeadingNumbering::AllNumbered),
            ),
    );
    let document = format!(
        "\\chapter{{C}}\n\n{EQ}\\section{{S}}\n\n{EQ}{EQ}\\section{{T}}\n\n\\section{{U}}\n\n\
         \\subsection{{A}}\n\n\\subsection{{B}}\n\n{EQ}{EQ}{EQ}"
    );
    let options = NumberingOptions::new()
        .initial("flm.headings.chapter", 3)
        .within("flm.math.equation", Within::parent_set(LEVELS_PARENT_SET));
    let run = render_text_with(&registry, &document, options);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(
        tags(&text_of(&run)),
        ["(4.1)", "(4.1.1)", "(4.1.2)", "(4.3.2.1)", "(4.3.2.2)", "(4.3.2.3)"]
    );
}

#[test]
fn a_heading_kind_the_depth_leaves_unnumbered_is_never_the_parent_that_applies() {
    // Under a depth of two a subsubsection states no number source, so it
    // advances nothing, and an equation after it is numbered within the
    // subsection.
    let registry = registry(
        StdcoreConfig::new()
            .headings(HeadingsConfig::new().numbering(HeadingNumbering::UpToDepth(2))),
    );
    let document = format!("\\section{{A}}\n\n\\subsection{{B}}\n\n\\subsubsection{{C}}\n\n{EQ}");
    let options =
        NumberingOptions::new().within("flm.math.equation", Within::parent_set(LEVELS_PARENT_SET));
    let run = render_text_with(&registry, &document, options);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let text = text_of(&run);
    assert!(text.contains("### C\n"), "{text}");
    assert_eq!(tags(&text), ["(1.1.1)"]);
}

#[test]
fn initial_values_count_as_advanced_outermost_first() {
    // `flm.headings.chapter` is inside `flm.headings.part` but sorts before it
    // by name. Both stand at an
    // initial value and nothing has advanced: the innermost, `chapter`, counts
    // as the most recent, so an equation before any heading is `(1.0.1)`.
    let kinds = vec![
        HeadingDecl::new("part", 1),
        HeadingDecl::new("chapter", 2).within(Within::parent("flm.headings.part")),
    ];
    let registry = registry(
        StdcoreConfig::new()
            .headings(
                HeadingsConfig::new().headings(kinds).numbering(HeadingNumbering::AllNumbered),
            ),
    );
    let options = NumberingOptions::new()
        .initial("flm.headings.part", 1)
        .initial("flm.headings.chapter", 0)
        .within("flm.math.equation", Within::parent_set(LEVELS_PARENT_SET));
    let run = render_text_with(&registry, EQ, options);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    assert_eq!(tags(&text_of(&run)), ["(1.0.1)"]);
}

#[test]
fn equations_are_numbered_where_no_heading_is() {
    // The shipped configuration numbers no heading and every equation.
    let run = render_text(&format!("\\section{{A}}\n\n{EQ}"));
    let text = text_of(&run);
    assert!(text.starts_with("A\n=\n"), "{text}");
    assert_eq!(tags(&text), ["(1)"]);
}
