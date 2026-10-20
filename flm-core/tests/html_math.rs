//! Mathematics on the `html` backend: the `math` feature's rule for inline and
//! unnamed displayed mathematics, which is this backend's alone
//! (`flm_core::stdcore::features::math::html`), and its line-aware rule for the
//! display-math environments, which serves every backend — from FLM source
//! through the whole pipeline to the HTML of a fragment.
//!
//! What is pinned here: a formula handed to the page whole as LaTeX; the six
//! display-math environments, with their labels and no-number marks dropped and
//! their breaks and tags kept; the invisible point at the start of each equation
//! line, which a `\ref` to that line links to; the argument of a text command
//! written as real HTML inside the formula, wherever the command stands; and
//! what stays source — a verbatim callable, a `\ref` written directly in
//! mathematics, a `\tag`.
//!
//! The emitter's own suite is `tests/html_emitter.rs`, over hand-built
//! presentation trees; this suite is about what the rules stage.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::dump::diag::expect;
use flm_core::dump::golden::assert_json_golden;
use flm_core::dump::project::{ProjectOptions, project_tree};
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::model::addr::TreeId;
use flm_core::model::names::BackendName;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::html;
use flm_core::stdcore::backends::lowering::lowered_table;
use flm_core::stdcore::features::math;
use flm_core::stdcore::features::refs;
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use techy::error::Severity;
use techy::serialize::SerialValue;

use common::html::{assert_golden, fragment, golden_path};
use common::stdcore::{identifiers, registry, registry_over, run_workflow};

/// The registry of the standard package under its default configuration.
fn std_registry() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// Runs `text` under the `html` workflow of the standard package and answers
/// the run.
fn render(text: &str) -> Run {
    run_workflow(&std_registry(), html::WORKFLOW, &[text], &Overrides::none())
}

/// The HTML of a document with no diagnostic at all.
fn clean(text: &str) -> String {
    let run = render(text);
    assert!(
        run.diagnostics().iter().next().is_none(),
        "{}",
        run.diagnostics().render_all()
    );
    fragment(&run)
}

/// The projection of the first tree as the `html` backend lowered it.
///
/// Local to this suite because `common::lowering::lowered` is wired to the
/// `text` backend. The second suite that wants it should move this function
/// into `common::html` rather than copy it.
fn lowered_projection(run: &Run) -> SerialValue {
    let table = lowered_table(&BackendName::from(html::BACKEND));
    let lowered = run
        .table(table)
        .expect("the lowered table is declared")
        .and_then(|trees| trees.get(TreeId::tree(0)))
        .unwrap_or_else(|| {
            panic!("no lowered tree; diagnostics:\n{}", run.diagnostics().render_all())
        });
    project_tree(&lowered.tree, &ProjectOptions::new())
}

// --- inline and display mathematics --------------------------------------------------------

#[test]
fn inline_mathematics_reaches_the_page_as_the_latex_the_author_wrote() {
    // The formula whole, between the delimiters the emitter chose, with the
    // source between them untouched: what the formula renderer in the reader's
    // browser reads out of the page.
    let html = clean("Let \\(a_i^2 = \\frac{1}{2}\\) hold.\n\nAnd \\(b > a\\) too.\n");
    assert!(html.contains("\\(a_i^2 = \\frac{1}{2}\\)"), "{html}");
    assert_golden("math-inline.html", &html);
}

#[test]
fn displayed_mathematics_written_without_an_environment_carries_the_display_form() {
    // `\[…\]` is displayed mathematics that names no environment: the same rule,
    // with the display form read off the group and no environment on the
    // payload.
    let html = clean("\\[ \\sum_{i} a_i = 0 \\]\n");
    assert!(html.contains("data-flm-display=\"display\""), "{html}");
    assert!(!html.contains("data-flm-env"), "{html}");
    assert!(html.contains("\\[ \\sum_{i} a_i = 0 \\]"), "{html}");
    assert_golden("math-display.html", &html);
}

#[test]
fn a_displayed_formula_inside_a_paragraph_is_a_span_like_every_other_formula() {
    // A `\[…\]` written between two runs of text stands inside the paragraph's
    // own element, because FLM models displayed mathematics as an inline node
    // of the text around it. HTML allows no block element inside a `<p>`, so
    // the element around a formula is a `<span>` whatever its display is, told
    // apart by `data-flm-display`; the default stylesheet is what makes the
    // displayed one look like a block. A `<div>` here would close the
    // paragraph in the tree a browser builds and move the text after the
    // formula out of it.
    let html = clean(
        "Before the formula \\[ x = 1 \\] and after it, same paragraph.\n\nA second \
         paragraph, so that the first one is written as an element of its own.\n",
    );
    assert!(!html.contains("<div"), "{html}");
    assert!(html.contains("<span class=\"flm-pres-math--passthrough\""), "{html}");
    assert!(html.contains("data-flm-display=\"display\""), "{html}");
    // The text after the formula is still inside the paragraph's element: a
    // `<div>` here would have closed the `<p>` in the tree a browser builds.
    assert!(html.contains("and after it, same paragraph.\n</p>"), "{html}");
    assert_golden("math-display-in-paragraph.html", &html);
}

#[test]
fn the_characters_html_gives_a_meaning_to_are_escaped_in_a_formula() {
    // `<`, `&` and `>` are notation in mathematics and markup in HTML: the sink
    // escapes them, and what the formula renderer reads is the text a browser
    // built from the page, in which they are the author's characters again.
    let html = clean("Compare \\(a < b\\) with \\(a & b\\) and \\(c > d\\).\n");
    assert!(html.contains("\\(a &lt; b\\)"), "{html}");
    assert!(html.contains("\\(a &amp; b\\)"), "{html}");
    assert!(html.contains("\\(c &gt; d\\)"), "{html}");
    assert_golden("math-escapes.html", &html);
}

#[test]
fn the_formula_renderers_own_tag_typed_by_an_author_stays_characters() {
    // The mechanism by which HTML is put inside a formula cannot be reached by
    // typing its tag: the emitter writes an empty TeX group after the `<`, which
    // typesets nothing. The rules here copy what the author wrote and do nothing
    // special; the emitter's suite pins the substitution itself.
    let html = clean("A formula \\(x = <tex-html><b>P</b></tex-html>\\).\n");
    assert!(!html.contains("<tex-html>"), "{html}");
    assert!(html.contains("&lt;{}tex-html"), "{html}");
    assert!(html.contains("&lt;{}/tex-html"), "{html}");
    assert_golden("math-literal-tag.html", &html);
}

// --- the display-math environments ---------------------------------------------------------

/// One document holding all six display-math environments, with a label, a
/// custom tag, a `\nonumber`, a `\\[1ex]` and a starred form among them.
const ENVIRONMENTS: &str = "\
\\begin{equation}\n  a = b \\label{eq:first}\n\\end{equation}\n\n\
\\begin{equation*}\n  c = d\n\\end{equation*}\n\n\
\\begin{align}\n  x &= y \\label{eq:x} \\\\[1ex]\n  z &= w \\nonumber\n\\end{align}\n\n\
\\begin{align*}\n  p &= q \\\\\n  r &= s\n\\end{align*}\n\n\
\\begin{gather}\n  u = v \\tag{A}\n\\end{gather}\n\n\
\\begin{gather*}\n  m = n\n\\end{gather*}\n";

#[test]
fn the_six_environments_are_written_back_with_their_breaks_and_tags_and_without_their_labels() {
    let run = render(ENVIRONMENTS);
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let html = fragment(&run);
    for env in ["equation", "equation*", "align", "align*", "gather", "gather*"] {
        assert!(html.contains(&format!("data-flm-env=\"{env}\"")), "{env}: {html}");
        assert!(html.contains(&format!("\\begin{{{env}}}")), "{env}: {html}");
        assert!(html.contains(&format!("\\end{{{env}}}")), "{env}: {html}");
    }
    // A formula renderer has no use for a label, and the line that asks for no
    // number simply has none: the rule writes the number where it has one, so
    // the command that would suppress it has nothing to suppress.
    assert!(!html.contains("\\label"), "{html}");
    assert!(!html.contains("\\nonumber"), "{html}");
    // The break with its spacing is the author's own. So is the tag of the
    // `gather` line, whose content reaches the page as content: `\tag{A}`
    // shows `(A)`, so the parentheses are written around it.
    assert!(html.contains("\\\\[1ex]"), "{html}");
    assert!(
        html.contains(
            "\\tag*{$<tex-html><span class=\"flm-pres-math--text\">(A)</span></tex-html>$}"
        ),
        "{html}"
    );
    // The numbered lines take their numbers in the same shape: the two
    // `equation` and `align` lines that can be numbered, and no other.
    assert_eq!(html.matches("\\tag*{$").count(), 3, "{html}");
    assert_golden("math-environments.html", &html);
}

#[test]
fn the_lowered_environment_is_the_anchor_node_and_the_lines_source() {
    // The shape a later package reads: one `flm.pres.math-passthrough` naming the
    // environment, holding an empty `flm.pres.math-text` carrying each line's
    // anchor and the line's source after it.
    let run = render(
        "\\begin{align}\n  x &= y \\label{eq:x} \\\\\n  z &= w\n\\end{align}\n",
    );
    assert_json_golden(golden_path("math-environment-lowered.json"), &lowered_projection(&run));
}

// --- the anchor at the start of a line -------------------------------------------------------

#[test]
fn a_reference_to_a_labeled_line_links_to_a_point_that_stands_at_that_line() {
    // The whole formula carries no identifier, so the line's anchor is an empty
    // element at the start of the line's own content: the link lands where the
    // line begins, inside the formula.
    let html = clean(
        "\\begin{align}\n  x &= y \\label{eq:x} \\\\\n  z &= w \\label{eq:z}\n\\end{align}\n\n\
         See \\ref{eq:x} and \\ref{eq:z}.\n",
    );
    assert!(html.contains("<tex-html><span class=\"flm-pres-math--text\" id=\"eq-x\">"), "{html}");
    assert!(html.contains("href=\"#eq-x\""), "{html}");
    assert!(html.contains("href=\"#eq-z\""), "{html}");
    // Every line is a reference point, so every line carries one such element;
    // each line's number is a second one, holding the number's text.
    assert_eq!(html.matches("class=\"flm-pres-math--text\"").count(), 4, "{html}");
    assert_golden("math-anchors.html", &html);
}

#[test]
fn an_unlabeled_line_carries_the_anchor_its_position_gives_it() {
    // A line with no label of its own still declares a reference point, and the
    // anchors stage gives it an anchor built from the prefix its element
    // states, its position among the document's equation lines, and the four
    // letters that keep the identifier from reading as a number.
    let html = clean("\\begin{align}\n  x &= y \\\\\n  z &= w\n\\end{align}\n");
    // Two anchors and the two numbers, each of them one such element.
    assert_eq!(html.matches("class=\"flm-pres-math--text\"").count(), 4, "{html}");
    assert!(html.contains(&format!("id=\"{}\"", refs::auto_anchor("eq", 1))), "{html}");
    assert!(html.contains(&format!("id=\"{}\"", refs::auto_anchor("eq", 2))), "{html}");
}

#[test]
fn a_line_whose_anchor_another_line_had_claimed_carries_no_point() {
    // Two lines labeled the same: the second claims no anchor, so nothing is
    // staged for it and no identifier occurs twice in the page.
    let run = render(
        "\\begin{align}\n  x &= y \\label{eq:x} \\\\\n  z &= w \\label{eq:x}\n\\end{align}\n",
    );
    expect(run.diagnostics().iter(), "flm.refs.duplicate-label")
        .severity(Severity::Error)
        .once();
    let html = fragment(&run);
    // One anchor — the second line claims none — and the two numbers.
    assert_eq!(html.matches("class=\"flm-pres-math--text\"").count(), 3, "{html}");
    assert_eq!(html.matches("id=\"eq-x\"").count(), 1, "{html}");
    assert_golden("math-anchor-collision.html", &html);
}

/// A registry holding the mathematics rules and the HTML backend but **not**
/// the references feature: the anchors table is not there, so the environment
/// rule's optional read answers nothing.
fn registry_without_refs() -> Arc<FlmRegistry> {
    use flm_core::stdcore::backends::{html as html_backend, lowering as lowering_backend};
    use flm_core::stdcore::features::blocks::Blocks;
    use flm_core::stdcore::features::{index, refpoints};
    use flm_core::stdcore::features::userdefs::{Userdefs, UserdefsConfig};

    registry_over(vec![
        lowering_backend::feature(),
        flm_core::stdcore::backends::pieces::feature(),
        html_backend::feature(),
        flm_core::stdcore::check_feature(),
        Arc::new(Blocks::default()),
        Arc::new(Userdefs::new(UserdefsConfig::new())),
        index::feature(),
        refpoints::feature(),
        math::feature(),
        flm_core::math_passthrough::feature(),
    ])
}

#[test]
fn a_registry_without_the_references_feature_stages_no_anchor_and_reports_nothing() {
    let registry = registry_without_refs();
    let run = run_workflow(
        &registry,
        html::WORKFLOW,
        &["\\begin{align}\n  x &= y \\\\\n  z &= w\n\\end{align}\n"],
        &Overrides::none(),
    );
    assert!(
        run.diagnostics().iter().next().is_none(),
        "{}",
        run.diagnostics().render_all()
    );
    let html = fragment(&run);
    assert!(!html.contains("flm-pres-math--text"), "{html}");
    assert!(html.contains("\\begin{align}"), "{html}");
    assert_golden("math-no-refs.html", &html);
}

// --- text inside mathematics ------------------------------------------------------------------

#[test]
fn a_text_commands_argument_is_written_as_html_inside_the_formula() {
    // The argument is document content: it is lowered like content anywhere
    // else and written as HTML inside the formula's LaTeX, through the one
    // mechanism the formula renderer offers.
    let html = clean("Let \\(a = \\text{one half}\\) hold.\n");
    assert!(
        html.contains(
            "<tex-html><span class=\"flm-pres-math--text\" data-flm-command=\"text\">one \
             half</span></tex-html>"
        ),
        "{html}"
    );
    assert!(!html.contains("\\text{"), "{html}");
    assert_golden("math-text.html", &html);
}

#[test]
fn styled_text_a_reference_and_a_formula_inside_a_text_command_all_reach_the_page() {
    let html = clean(
        "\\section{Sums}\\label{sec:sums}\n\n\
         Let \\(x = \\text{\\emph{roughly} \\(y\\), see \\ref{sec:sums}}\\) hold.\n",
    );
    // The emphasis is a real element, the reference a real link, and the inner
    // formula's source goes into the formula around it with no delimiters of
    // its own — the characters already stand in mathematics.
    assert!(html.contains("data-flm-style=\"emph\""), "{html}");
    assert!(html.contains("href=\"#sec-sums\""), "{html}");
    assert_eq!(html.matches("flm-pres-math--passthrough").count(), 1, "{html}");
    assert_golden("math-text-content.html", &html);
}

#[test]
fn a_text_command_in_an_alignment_cell_leaves_the_alignment_alone() {
    // Nothing wraps a line, so the `&` marks stay where the author put them and
    // the content stands between them.
    let html = clean(
        "\\begin{align}\n  x &= y \\text{ for every } y \\\\\n  z &= w\n\\end{align}\n",
    );
    assert!(html.contains("&amp;= y <tex-html>"), "{html}");
    assert!(html.contains("</tex-html> y \\tag*{$"), "{html}");
    assert_golden("math-text-align.html", &html);
}

#[test]
fn every_text_command_carries_the_name_it_was_written_with() {
    // One element with one value per command: the command as written travels to
    // the page on the node's own attribute, and the styling is the
    // stylesheet's.
    let source: String = math::text::COMMANDS
        .iter()
        .map(|command| format!("\\({command}: \\{command}{{a}}\\)\n\n"))
        .collect();
    let html = clean(&source);
    for command in math::text::COMMANDS {
        assert!(html.contains(&format!("data-flm-command=\"{command}\"")), "{command}: {html}");
    }
    assert_golden("math-text-commands.html", &html);
}

#[test]
fn a_text_command_becomes_content_wherever_it_stands_in_the_formula() {
    // The rule walks the whole formula, so a text command inside a group is
    // content just as one standing beside the formula's other nodes is. The
    // braces the walk writes back are character nodes the rule made, standing
    // beside copies of the author's own nodes — `\frac` here — and the two
    // kinds must be written together correctly.
    let html = clean(
        "Let \\(x_{\\text{eff}} = 1\\) and \\(\\frac{\\text{top}}{2}\\) hold.\n",
    );
    assert!(html.contains("\\(x_{<tex-html>"), "{html}");
    assert!(html.contains("</tex-html>} = 1\\)"), "{html}");
    assert!(html.contains("\\(\\frac{<tex-html>"), "{html}");
    assert!(html.contains("</tex-html>}{2}\\)"), "{html}");
    assert!(!html.contains("\\text{"), "{html}");
    assert_golden("math-text-nested.html", &html);
}

#[test]
fn a_text_command_inside_a_nested_environment_becomes_content_too() {
    // A `cases` standing in an `align`: the walk writes `\begin{cases}` and
    // `\end{cases}` back and opens the body, so the two text commands inside
    // become content. The `\\` and the `&` of the nested environment are
    // written as typed — only the outer environment's lines are read as lines.
    let html = clean(
        "\\begin{align}\n  f(x) &= \\begin{cases} 1 & \\text{if } x > 0 \\\\ \
         0 & \\text{otherwise} \\end{cases}\n\\end{align}\n",
    );
    assert!(html.contains("\\begin{cases}"), "{html}");
    assert!(html.contains("\\end{cases}"), "{html}");
    assert!(html.contains("data-flm-command=\"text\">if </span>"), "{html}");
    assert!(html.contains("data-flm-command=\"text\">otherwise</span>"), "{html}");
    assert_golden("math-text-in-cases.html", &html);
}

#[test]
fn a_reference_deep_inside_a_text_argument_is_a_link() {
    // The argument of a text command is document content at any depth, so a
    // `\ref` written there is a real link in the page; one written directly in
    // mathematics, where nothing is content, is passed on as typed.
    let html = clean(
        "\\section{Sums}\\label{sec:sums}\n\nLet \\(\\frac{\\text{see \\ref{sec:sums}}}\
         {\\ref{sec:sums}}\\) hold.\n",
    );
    assert!(html.contains("href=\"#sec-sums\""), "{html}");
    assert!(html.contains("}{\\ref{sec:sums}}\\)"), "{html}");
    assert_golden("math-ref-deep.html", &html);
}

#[test]
fn a_user_definition_used_deep_inside_a_formula_arrives_expanded() {
    // The expansion rewrite runs before lowering, so the use site is gone by
    // the time the walk meets the group that held it.
    let run = run_workflow(
        &registry_with_definition(),
        html::WORKFLOW,
        &["Let \\(\\frac{\\half}{2}\\) hold.\n"],
        &Overrides::none(),
    );
    assert!(
        run.diagnostics().iter().next().is_none(),
        "{}",
        run.diagnostics().render_all()
    );
    let html = fragment(&run);
    assert!(!html.contains("\\half"), "{html}");
    assert!(html.contains("\\frac{\\frac{1}{2}}{2}"), "{html}");
}

#[test]
fn a_tag_holding_text_reaches_the_page_as_content_inside_the_formula() {
    // `\tag`'s argument is read in text mode, as LaTeX reads it, and it reaches
    // the page as the page content it is: `\tag*{$…$}` around the element the
    // emitter writes for content standing inside a formula. FLM's reading of
    // the unstarred form — `\tag{A}` shows `(A)` — is the parentheses written
    // inside that content, so the renderer adds none of its own.
    let html = clean("\\begin{gather}\n  u = v \\tag{a note}\n\\end{gather}\n");
    assert!(
        html.contains(
            "\\tag*{$<tex-html><span class=\"flm-pres-math--text\">(a note)</span></tex-html>$}"
        ),
        "{html}"
    );
    assert_golden("math-tag-text.html", &html);
}

#[test]
fn a_tag_holding_markup_reaches_the_page_as_that_markup() {
    // The whole point of writing a tag as content: what the author marked up
    // arrives marked up, inside the formula, where a renderer sets it in its
    // tag column.
    let html = clean("\\begin{gather}\n  u = v \\tag{\\emph{a}}\n\\end{gather}\n");
    assert!(html.contains("\\tag*{$<tex-html>"), "{html}");
    assert!(html.contains("data-flm-style=\"emph\""), "{html}");
    assert!(html.contains("</tex-html>$}"), "{html}");
    assert_golden("math-tag-markup.html", &html);
}

// --- user definitions -------------------------------------------------------------------------

/// A registry whose roster holds one definition, used inside mathematics and
/// inside a text command's argument.
fn registry_with_definition() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(vec![
        Definition::macro_("half", Vec::new()).math("\\frac{1}{2}"),
        Definition::macro_("roughly", Vec::new()).text("about"),
    ])))
}

#[test]
fn a_user_definition_used_inside_mathematics_reaches_the_page_expanded() {
    // A browser's formula renderer knows none of the author's macros, so the
    // expansion rewrite runs and the use site never reaches the emitter.
    let run = run_workflow(
        &registry_with_definition(),
        html::WORKFLOW,
        &["Let \\(a = \\half\\) and \\(b = \\text{\\roughly, one half}\\).\n"],
        &Overrides::none(),
    );
    assert!(
        run.diagnostics().iter().next().is_none(),
        "{}",
        run.diagnostics().render_all()
    );
    let html = fragment(&run);
    assert!(!html.contains("\\half"), "{html}");
    assert!(!html.contains("\\roughly"), "{html}");
    assert!(html.contains("\\frac{1}{2}"), "{html}");
    assert!(html.contains("about, one half"), "{html}");
    assert_golden("math-userdef.html", &html);
}

// --- what stays source ---------------------------------------------------------------------

#[test]
fn a_verbatim_callable_and_a_reference_written_in_mathematics_stay_source() {
    // `\verbcode` and `\ref` are text-only, so inside a formula they are unknown
    // commands kept as written (opaque nodes; a `\ref` there declares no
    // reference site), and nothing lowers them here: the rule copies a
    // formula's nodes as the source they were
    // written as, so what reaches the reader is the characters, which the
    // formula renderer makes what it can of. Passing them on as typed is a
    // temporary measure until FLM parses mathematics: "\ref should not be
    // exposed as a command in math. At this stage, it is expected to be lowered
    // to a literal representation on all outputs, until we set up better
    // parsing math environments."
    // Inside a text command's argument, which is text, a `\ref` is a real link.
    let html = clean(
        "\\section{Sums}\\label{sec:sums}\n\nLet \\(a = \\verbcode|x| + \\ref{sec:sums}\\) hold.\n",
    );
    assert!(html.contains("\\verbcode|x|"), "{html}");
    assert!(html.contains("\\ref{sec:sums}"), "{html}");
    assert!(!html.contains("href=\"#sec-sums\""), "{html}");
    assert!(!html.contains("flm-pres-verbatim"), "{html}");
    assert_golden("math-source-callables.html", &html);
}
