//! The `theorems` feature on the HTML backend: the page of every shipped kind
//! and of a host's kind (goldens, which `tests/html_validity.rs` parses with
//! no excuse), the heading's edges — a body opening with a list, an empty
//! body — and the feature's stylesheet, required once by a page holding any
//! theorem and by no other.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::{DefContent, FlmRegistry};
use flm_core::core::run::Run;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::html::{DEFINITIONS_TABLE, WORKFLOW as HTML_WORKFLOW};
use flm_core::stdcore::features::headings::HeadingsConfig;
use flm_core::stdcore::features::theorems::{self, TheoremClass, TheoremKind, TheoremsConfig};

use common::stdcore::{identifiers, run_workflow};

// ---------------------------------------------------------------------------------------------
// registries and runs
// ---------------------------------------------------------------------------------------------

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The run of the `html` workflow over `document`.
fn html_run(registry: &Arc<FlmRegistry>, document: &str) -> Run {
    run_workflow(registry, HTML_WORKFLOW, &[document], &Overrides::none())
}

/// The HTML fragment of `document`; the run must report nothing.
fn html_output(registry: &Arc<FlmRegistry>, document: &str) -> String {
    let run = html_run(registry, document);
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    common::html::fragment(&run)
}

/// Whether the page of `run` requires the feature's stylesheet.
fn requires_style(run: &Run) -> bool {
    run.table(DEFINITIONS_TABLE)
        .expect("the definitions table is declared")
        .expect("the definitions table is written")
        .definitions
        .iter()
        .any(|row| row.id.as_str() == theorems::html::STYLE)
}

/// The opening tag of a theorem's block: its role's class, its anchor where it
/// has one, and its kind tokens.
fn block(anchor: Option<&str>, kinds: &str) -> String {
    let id = anchor.map(|anchor| format!(" id=\"{anchor}\"")).unwrap_or_default();
    format!("<div class=\"flm-pres-block flm-theorems-theorem\"{id} data-flm-kind=\"{kinds}\">")
}

/// A heading's name span.
fn name(text: &str) -> String {
    format!("<span class=\"flm-pres-span flm-theorems-name\">{text}</span>")
}

/// A heading's title span around `inner`.
fn title(inner: &str) -> String {
    format!("<span class=\"flm-pres-span flm-theorems-title\">{inner}</span>")
}

/// The opening of a paragraph and of its run-in decoration, followed by
/// `start`, the heading's first part.
fn runin(start: &str) -> String {
    format!("<p class=\"flm-pres-par\"><span class=\"flm-pres-deco-runin\">{start}")
}

/// The closing mark, as the final decoration.
const MARK: &str = "<span class=\"flm-pres-deco-final\">□</span>";

// ---------------------------------------------------------------------------------------------
// the page of every shipped kind (test 13)
// ---------------------------------------------------------------------------------------------

/// A numbered theorem with a title and a label, a proof back-referencing it
/// and shown, a proof recording the relation without showing it, a
/// definition, a remark, and the default set's other kinds: a proposition
/// and a corollary, each stating it has no proof, and a conjecture.
const KINDS: &str = "\\begin{theorem}[My theorem]\\label{thm:a}
The square root of two is irrational.
\\end{theorem}

\\begin{proof}[*thm:a]
We argue by contradiction.
\\end{proof}

\\begin{proof}[**thm:a]
Once more, the same way.
\\end{proof}

\\begin{definition}
A number is rational when it is a ratio of integers.
\\end{definition}

\\begin{remark}
Every integer is rational.
\\end{remark}

\\begin{proposition}\\noproofref
The square root of three is irrational.
\\end{proposition}

\\begin{corollary}\\noproofref
The square root of twelve is irrational.
\\end{corollary}

\\begin{conjecture}
The number pi plus e is irrational.
\\end{conjecture}
";

#[test]
fn the_page_of_the_shipped_kinds() {
    let html = html_output(&std(), KINDS);
    // The block carries its role's class, its anchor and its kind tokens, the
    // class's name then the kind's; the heading's two spans carry their roles.
    let theorem = format!(
        "{}\n  {} ({}).</span>The square root",
        block(Some("theorem-1"), "theoremlike theorem"),
        runin(&name("Theorem&#xA0;1")),
        title("My theorem"),
    );
    assert!(html.contains(&theorem), "{html}");
    // A shown back-reference: a link of kind `ref` to the statement's anchor
    // inside the title span; a proof has no anchor, and closes with its mark.
    let link = "<a class=\"flm-pres-link\" href=\"#theorem-1\" data-flm-kind=\"ref\">Theorem&#xA0;1</a>";
    let proof = format!(
        "{}\n  {} ({}).</span>We argue by contradiction.{MARK}</p>",
        block(None, "prooflike proof"),
        runin(&name("Proof")),
        title(link),
    );
    assert!(html.contains(&proof), "{html}");
    // A relation recorded and not shown: the name alone.
    let hidden = format!("{}.</span>Once more", runin(&name("Proof")));
    assert!(html.contains(&hidden), "{html}");
    for (anchor, kinds, word) in [
        ("definition-2", "definitionlike definition", "Definition&#xA0;2"),
        ("remark-3", "definitionlike remark", "Remark&#xA0;3"),
        ("proposition-4", "theoremlike proposition", "Proposition&#xA0;4"),
        ("corollary-5", "theoremlike corollary", "Corollary&#xA0;5"),
        ("conjecture-6", "theoremlike conjecture", "Conjecture&#xA0;6"),
    ] {
        let opening = format!("{}\n  {}.</span>", block(Some(anchor), kinds), runin(&name(word)));
        assert!(html.contains(&opening), "{word}: {html}");
    }
    // No tag of the feature's own.
    assert_eq!(html.matches("<div").count(), 8, "{html}");
    common::html::assert_golden("theorems-kinds.html", &html);
}

/// A theorem with an empty body, a lemma whose body opens with a list, both
/// stating they have no proof, a proof ending with a list, and an empty
/// proof.
const EDGES: &str = "\\begin{theorem}\\noproofref\\end{theorem}

\\begin{lemma}\\noproofref
\\begin{enumerate}
\\item One case.
\\item The other.
\\end{enumerate}
\\end{lemma}

\\begin{proof}
Both cases:
\\begin{itemize}
\\item the first;
\\item the second.
\\end{itemize}
\\end{proof}

\\begin{proof}\\end{proof}
";

#[test]
fn the_heading_and_the_mark_at_the_body_s_edges() {
    let html = html_output(&std(), EDGES);
    // An empty body: one paragraph holding the heading alone.
    let empty = format!(
        "{}\n  {}.</span></p>\n</div>",
        block(Some("theorem-1"), "theoremlike theorem"),
        runin(&name("Theorem&#xA0;1"))
    );
    assert!(html.contains(&empty), "{html}");
    // A body opening with a list: the heading in a paragraph of its own
    // before it.
    let list = format!(
        "{}\n  {}.</span></p>\n  <ol ",
        block(Some("lemma-2"), "theoremlike lemma"),
        runin(&name("Lemma&#xA0;2"))
    );
    assert!(html.contains(&list), "{html}");
    // A body ending with a list: the mark in a paragraph of its own after it.
    assert!(html.contains(&format!("</ul>\n  <p class=\"flm-pres-par\">{MARK}</p>\n</div>")), "{html}");
    // An empty proof: one paragraph holding both.
    let both = format!(
        "{}\n  {}.</span>{MARK}</p>\n</div>",
        block(None, "prooflike proof"),
        runin(&name("Proof"))
    );
    assert!(html.contains(&both), "{html}");
    common::html::assert_golden("theorems-edges.html", &html);
}

#[test]
fn the_page_of_a_host_s_kind() {
    // A kind in a class of the host's own, with a counter of its own: its
    // tokens are the host's names, with no code written for it.
    let config = TheoremsConfig::new()
        .class("exerciselike", TheoremClass::numbered().shared_counter(false))
        .kind("exercise", TheoremKind::from_word("exerciselike", "exercise"));
    let registry = common::stdcore::registry(StdcoreConfig::new().theorems(config));
    let html = html_output(
        &registry,
        "\\begin{theorem}\\noproofref\nA statement.\n\\end{theorem}\n\n\\begin{exercise}[Warm-up]\\label{x:warmup}\nProve it.\n\\end{exercise}\n",
    );
    let exercise = format!(
        "{}\n  {} ({}).</span>Prove it.</p>",
        block(Some("exercise-1"), "exerciselike exercise"),
        runin(&name("Exercise&#xA0;1")),
        title("Warm-up")
    );
    assert!(html.contains(&exercise), "{html}");
    common::html::assert_golden("theorems-host-kind.html", &html);
}

// ---------------------------------------------------------------------------------------------
// the stylesheet
// ---------------------------------------------------------------------------------------------

#[test]
fn a_page_holding_theorems_carries_the_stylesheet_once() {
    let run = html_run(&std(), KINDS);
    assert!(requires_style(&run));
    let page = common::html::page(&run);
    let marker = format!("data-flm-def=\"{}\"", theorems::html::STYLE);
    assert_eq!(page.matches(&marker).count(), 1, "{page}");
    let DefContent::Inline(css) = &theorems::html::stylesheet().content[0] else { panic!("inline") };
    assert_eq!(page.matches(css.trim_end()).count(), 1, "{page}");
    // A page holding no theorem carries none of it.
    let run = html_run(&std(), "A paragraph.\n");
    assert_eq!(identifiers(&run), Vec::<String>::new(), "{}", run.diagnostics().render_all());
    assert!(!requires_style(&run));
}

#[test]
fn the_stylesheet_sets_the_name_by_its_role_and_the_class_by_its_token() {
    let DefContent::Inline(css) = &theorems::html::stylesheet().content[0] else { panic!("inline") };
    // The name, reached from the block through the heading; a proof's, through
    // the class's kind token; every value a custom property.
    let heading = ".flm-theorems-theorem > .flm-pres-par > .flm-pres-deco-runin";
    assert!(css.contains(&format!("{heading} > .flm-theorems-name {{")), "{css}");
    assert!(
        css.contains(
            ".flm-theorems-theorem[data-flm-kind~=\"prooflike\"] > .flm-pres-par > .flm-pres-deco-runin > .flm-theorems-name {"
        ),
        "{css}"
    );
    // The weight of the rest of the heading is reset on the heading alone:
    // the first child of the block's first paragraph.
    let first = ".flm-theorems-theorem > .flm-pres-par:first-child > .flm-pres-deco-runin:first-child {";
    assert!(css.contains(first), "{css}");
    for line in css.lines().map(str::trim) {
        let is_declaration = line.ends_with(';') && !line.starts_with("--");
        if is_declaration {
            assert!(line.contains("var(--flm-theorems-"), "`{line}` is not a custom property");
        }
    }
}

#[test]
fn a_run_in_section_heading_inside_a_theorem_is_not_the_theorem_s_heading() {
    // A run-in `\paragraph` heading opening the second paragraph of a theorem
    // is a run-in decoration directly inside the block too; the stylesheet's
    // reset of the heading's weight must not reach it. It selects the first
    // child of the block's first paragraph, which is the theorem's heading
    // alone: the section heading opens a later paragraph.
    let registry = common::stdcore::registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().run_in_levels([4, 5])),
    );
    let html = html_output(
        &registry,
        "\\begin{theorem}\\noproofref\nFirst.\n\n\\paragraph{A case} Second.\n\\end{theorem}\n",
    );
    let opening = format!("{}\n  {}.</span>First.</p>", block(Some("theorem-1"), "theoremlike theorem"), runin(&name("Theorem&#xA0;1")));
    assert!(html.contains(&opening), "{html}");
    let second = "</p>\n  <p class=\"flm-pres-par\"><span class=\"flm-pres-deco-runin\"><span class=\"flm-pres-runin--heading\"";
    assert!(html.contains(second), "{html}");
}
