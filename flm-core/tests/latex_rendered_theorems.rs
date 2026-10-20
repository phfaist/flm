//! Theorems on the `latex-rendered` backend, end to end over the standard
//! package: the documents `tests/stdcore_theorems_html.rs` renders (the
//! shipped kinds, the heading and the mark at the body's edges, a kind of a
//! class of the host's own, a run-in section heading inside a theorem), each
//! a golden under `tests/goldens/latex-rendered/` holding the document and
//! the style file, which `tests/latex_rendered_compile.rs` compiles with
//! `lualatex`; and the theorems feature's definition piece, which the
//! feature's style identifier brings into the style file.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::headings::HeadingsConfig;
use flm_core::stdcore::features::theorems::latex_rendered::PIECE;
use flm_core::stdcore::features::theorems::{TheoremClass, TheoremKind, TheoremsConfig};

use common::latex_rendered::{assert_golden, document, render, style};

/// The standard package under its default configuration.
fn std() -> Arc<FlmRegistry> {
    common::stdcore::registry(StdcoreConfig::new())
}

/// The shipped kinds: a theorem with a title and a label, a proof showing
/// its back-reference and one hiding it, a definition, a remark, a
/// proposition and a corollary stating they have no proof, and a conjecture.
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

/// A theorem with an empty body, a lemma whose body is a list, a proof
/// ending with a list, and an empty proof.
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
fn the_shipped_kinds_are_written_by_class_and_the_style_file_holds_the_piece() {
    let run = render(&std(), &[KINDS], &Overrides::none());
    let written = document(&run);
    assert!(
        written.contains(
            "\\begin{flmxBlock}[role=flm.theorems.theorem, kinds={theoremlike theorem}, anchor=theorem-1]"
        ),
        "{written}"
    );
    assert!(written.contains("kinds={prooflike proof}"), "{written}");
    assert!(written.contains("kinds={definitionlike definition}"), "{written}");
    let (text, _) = style(&run);
    assert!(text.contains(&format!("% {PIECE} --")), "{text}");
    assert!(text.contains("\\flmxDefineBlock { theoremlike }"), "{text}");
    assert_golden("theorems-kinds.tex", &run);
}

#[test]
fn the_heading_and_the_mark_at_the_body_s_edges() {
    let run = render(&std(), &[EDGES], &Overrides::none());
    assert!(document(&run).contains("\\flmxDecoFinal{□}"), "{}", document(&run));
    assert_golden("theorems-edges.tex", &run);
}

#[test]
fn a_kind_of_a_class_of_the_host_s_own_is_written_with_its_tokens() {
    let config = TheoremsConfig::new()
        .class("exerciselike", TheoremClass::numbered().shared_counter(false))
        .kind("exercise", TheoremKind::from_word("exerciselike", "exercise"));
    let registry = common::stdcore::registry(StdcoreConfig::new().theorems(config));
    let run = render(
        &registry,
        &["\\begin{theorem}\\noproofref\nA statement.\n\\end{theorem}\n\n\\begin{exercise}[Warm-up]\\label{x:warmup}\nProve it.\n\\end{exercise}\n"],
        &Overrides::none(),
    );
    assert!(document(&run).contains("kinds={exerciselike exercise}"), "{}", document(&run));
    assert_golden("theorems-host-kind.tex", &run);
}

#[test]
fn a_run_in_section_heading_inside_a_theorem_keeps_its_own_look() {
    let registry = common::stdcore::registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().run_in_levels([4, 5])),
    );
    let run = render(
        &registry,
        &["\\begin{theorem}\\noproofref\nFirst.\n\n\\paragraph{A case} Second.\n\\end{theorem}\n"],
        &Overrides::none(),
    );
    assert!(document(&run).contains("\\flmxRuninHeading["), "{}", document(&run));
    assert_golden("theorems-runin-inside.tex", &run);
}

#[test]
fn a_document_without_a_theorem_does_not_hold_the_piece() {
    let run = render(&std(), &["Plain text."], &Overrides::none());
    assert!(!style(&run).0.contains(PIECE), "{}", style(&run).0);
}

/// A proof of a claim inside a proof: the inner proof ends with its closing
/// mark, and the outer one after it. Each theorem of a shipped class saves
/// the defaults outside every theorem only once, in the outermost one, so
/// that the nested proof's mark does not call itself (the golden compiles).
#[test]
fn a_proof_nested_in_a_proof_ends_with_its_own_mark() {
    let run = render(
        &std(),
        &["\\begin{proof}\nWe first show a claim.\n\\begin{proof}\nThe claim holds.\n\\end{proof}\nThe rest follows.\n\\end{proof}\n"],
        &Overrides::none(),
    );
    let written = document(&run);
    assert_eq!(written.matches("\\flmxDecoFinal{□}").count(), 2, "{written}");
    assert_golden("theorems-nested-proof.tex", &run);
}

/// A remark inside a theorem, with a run-in section heading inside the
/// remark: the heading is a run-in decoration after the remark's own, written
/// with the decoration's look outside every theorem (the golden compiles).
#[test]
fn a_nested_theorem_with_a_later_run_in_decoration() {
    let registry = common::stdcore::registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().run_in_levels([4, 5])),
    );
    let run = render(
        &registry,
        &["\\begin{theorem}\\noproofref\nOuter.\n\\begin{remark}\nInner.\n\n\\paragraph{A case} Later.\n\\end{remark}\n\\end{theorem}\n"],
        &Overrides::none(),
    );
    let written = document(&run);
    assert!(written.contains("kinds={definitionlike remark}"), "{written}");
    assert!(written.contains("\\flmxRuninHeading["), "{written}");
    assert_golden("theorems-nested-runin.tex", &run);
}
