//! Where a display-mathematics environment stands in a document's block
//! structure: it is a displayed item, which stays in the running text written
//! around it on the neighbouring lines rather than dividing that text; and how
//! the backends that write paragraphs (HTML, the rendered LaTeX, the text
//! backend) write a displayed item in its paragraph, beside a list, a block,
//! which ends the paragraph it is written in.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::lang::structural::{MATH, TEXT};
use flm_core::model::addr::TreeId;
use flm_core::model::aspects::{BlockRole, block_role_of};
use flm_core::model::element::element_of;
use flm_core::stdlib::StdlibConfig;
use flm_core::stdlib::backends::pres;
use flm_core::stdlib::backends::{html, latex_rendered, lowering, text};
use flm_core::stdlib::check_feature;
use flm_core::stdlib::features::blocks::Blocks;
use flm_core::stdlib::features::{math, textstyles};

use common::html::fragment;
use common::lowering::{lowered, text as text_of};
use common::stdlib::{assert_only_unchecked_output, identifiers, registry_over, run_workflow};

/// A registry over the `math` and `textstyles` features, the lowering
/// machinery, the text backend and the `blocks` feature.
fn registry() -> Arc<FlmRegistry> {
    registry_over(vec![
        lowering::feature(),
        flm_core::stdlib::backends::pieces::feature(),
        text::feature(),
        check_feature(),
        Arc::new(Blocks::default()),
        textstyles::feature(),
        math::feature(),
        flm_core::math_passthrough::feature(),
    ])
}

// Ported from Python FLM's test/test_flmenvironment.py::TestBlocksBuilder::test_blocks_paragraphs_correct_number.
// A document holding no blank line is laid out as inline content, the
// environment among it, as a `\[…\]` group is.
#[test]
fn a_display_environment_inside_running_text_stays_in_it() {
    let document = "Test\nsome\ninput \\textbf{string} here. Does this\n\
                    \\begin{equation}\n a = b\n\\end{equation}\n\
                    work?\n";
    let run = run_workflow(&registry(), text::WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());

    let children: Vec<String> = lowered(&run, TreeId::tree(0))
        .tree
        .root()
        .children()
        .iter()
        .map(|node| element_of(&node).name.to_string())
        .collect();
    assert_eq!(children, [TEXT, pres::SPAN, TEXT, pres::MATH_PASSTHROUGH, TEXT]);

    assert_eq!(
        text_of(&run),
        "Test some input string here. Does this\n\
         \x20 \\begin{equation}\n   a = b\n  \\end{equation}\n\
         work?\n"
    );
}

/// The six display-math environments and a `\[…\]` group are displayed items
/// in a document, and a `\(…\)` group is ordinary inline content.
#[test]
fn the_display_environments_and_a_displayed_group_are_displayed_items() {
    let mut document = String::from("Text \\(a\\) and \\[b\\] then\n");
    for (name, _) in math::ENVIRONMENTS {
        document.push_str(&format!("\\begin{{{name}}}\n  c\n\\end{{{name}}}\n"));
    }
    let run = run_workflow(&registry(), text::WORKFLOW, &[document.as_str()], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let tree = run.frozen_tree(TreeId::tree(0)).expect("the first tree is frozen");
    let roles = |name: &str| -> Vec<BlockRole> {
        tree.root()
            .descendants()
            .filter(|node| element_of(node).name.as_str() == name)
            .map(|node| block_role_of(&node))
            .collect()
    };
    assert_eq!(roles(math::ENVIRONMENT), [BlockRole::Displayed; 6]);
    assert_eq!(roles(MATH), [BlockRole::DEFAULT, BlockRole::Displayed]);
}

// --- a displayed item in its paragraph, on each backend ----------------------------------

/// An environment in the running text, the text after it on the next line,
/// then a blank line and a second paragraph.
const CONTINUED: &str = "Text before\n\\begin{equation}\n  a = b\n\\end{equation}\n\
                         text after.\n\nMore text.\n";

/// [`CONTINUED`] with a blank line on each side of the environment.
const BROKEN: &str = "Text before\n\n\\begin{equation}\n  a = b\n\\end{equation}\n\n\
                      text after.\n\nMore text.\n";

/// [`CONTINUED`] with a displayed group, `\[…\]`, in place of the environment.
const GROUP: &str = "Text before\n\\[ a = b \\]\ntext after.\n\nMore text.\n";

/// [`CONTINUED`] with a list, a block, in place of the environment.
const LIST: &str = "Text before\n\\begin{itemize}\n\\item One.\n\\end{itemize}\n\
                    text after.\n\nMore text.\n";

/// The run of `workflow` over `document` in the standard package, asserting
/// that it reported nothing but the Warnings of its plan's unchecked output.
fn standard_run(workflow: &str, document: &str) -> Run {
    let registry = common::stdlib::registry(StdlibConfig::new());
    let run = run_workflow(&registry, workflow, &[document], &Overrides::none());
    assert_only_unchecked_output(&run);
    run
}

/// The rendered LaTeX of `document`: what stands between `\begin{document}`
/// and `\end{document}`.
fn rendered_latex(document: &str) -> String {
    let run = standard_run(latex_rendered::WORKFLOW, document);
    let set = run.table(latex_rendered::DOCUMENT).expect("declared").expect("written");
    let entry = set.entry().expect("the entry artifact");
    let text = String::from_utf8(entry.as_bytes().expect("bytes").to_vec()).expect("UTF-8");
    let start = text.find("\\begin{document}\n").expect("the skeleton")
        + "\\begin{document}\n".len();
    let end = text.rfind("\\end{document}").expect("the skeleton");
    String::from(&text[start..end])
}

#[test]
fn html_writes_a_displayed_item_inside_its_paragraph_with_no_space_beside_it() {
    // One `<p>` holding the text, the formula's `<span>` and the text after
    // it, the document's spaces beside the formula gone (the line break and
    // the indentation around the `<span>` are the writer's layout); the blank
    // lines of `broken` make four paragraphs, the second holding only the
    // formula; a list ends its paragraph, and the text after it is a new one.
    // No paragraph holds flow content, so every paragraph is a `<p>`.
    let paragraph = |text: &str| format!("<p class=\"flm-pres-par\">{text}</p>\n");
    let formula = "  <span class=\"flm-pres-math--passthrough flm-math-formula\" data-flm-display=\"display\"";
    let tail = format!("{}{}", paragraph("text after."), paragraph("More text."));
    let html = |document: &str| fragment(&standard_run(html::WORKFLOW, document));

    for document in [CONTINUED, GROUP] {
        let out = html(document);
        let head = format!("<p class=\"flm-pres-par\">Text before\n{formula}");
        assert!(out.starts_with(&head), "{out}");
        let end = format!("</span>text after.\n</p>\n{}", paragraph("More text."));
        assert!(out.ends_with(&end), "{out}");
        assert_eq!(out.matches("<p ").count(), 2, "{out}");
    }
    let out = html(BROKEN);
    let head = format!("{}<p class=\"flm-pres-par\">\n{formula}", paragraph("Text before"));
    assert!(out.starts_with(&head), "{out}");
    assert!(out.ends_with(&format!("</span>\n</p>\n{tail}")), "{out}");
    assert_eq!(out.matches("<p ").count(), 4, "{out}");
    let out = html(LIST);
    assert!(out.starts_with(&format!("{}<ul ", paragraph("Text before"))), "{out}");
    assert!(out.ends_with(&format!("</ul>\n{tail}")), "{out}");
    for document in [CONTINUED, BROKEN, GROUP, LIST] {
        let out = html(document);
        assert!(!out.contains("role=\"paragraph\""), "{out}");
    }
}

#[test]
fn the_rendered_latex_writes_a_displayed_item_on_lines_of_its_own_inside_its_paragraph() {
    // The environment on lines of its own inside the LaTeX paragraph, with no
    // blank line before or after it, so the text after it continues the
    // paragraph; the blank lines of `broken` stay, and a list is a block
    // between two paragraphs.
    let out = rendered_latex(CONTINUED);
    assert!(out.starts_with("Text before\n\\begin{equation*}"), "{out}");
    assert!(out.ends_with("\\end{equation*}\ntext after.\n\nMore text.\n"), "{out}");
    let out = rendered_latex(BROKEN);
    assert!(out.starts_with("Text before\n\n\\begin{equation*}"), "{out}");
    assert!(out.ends_with("\\end{equation*}\n\ntext after.\n\nMore text.\n"), "{out}");
    assert_eq!(rendered_latex(GROUP), "Text before\n\\[ a = b \\]\ntext after.\n\nMore text.\n");
    let out = rendered_latex(LIST);
    assert!(out.starts_with("Text before\n\n\\begin{flmxList}"), "{out}");
    assert!(out.ends_with("\\end{flmxList}\n\ntext after.\n\nMore text.\n"), "{out}");
}

#[test]
fn the_text_backend_writes_a_displayed_item_indented_on_lines_of_its_own() {
    // The environment indented on lines of its own, with no blank line around
    // it inside its paragraph; blank lines between the paragraphs of
    // `broken`, and around the list, a block.
    let text = |document: &str| text_of(&standard_run(text::WORKFLOW, document));
    let environment = "  \\begin{equation}\n    a = b\\tag*{(1)}\n  \\end{equation}\n";
    assert_eq!(
        text(CONTINUED),
        format!("Text before\n{environment}text after.\n\nMore text.\n")
    );
    assert_eq!(
        text(BROKEN),
        format!("Text before\n\n{environment}\ntext after.\n\nMore text.\n")
    );
    assert_eq!(text(GROUP), "Text before\n  \\[ a = b \\]\ntext after.\n\nMore text.\n");
    assert_eq!(text(LIST), "Text before\n\n• One.\n\ntext after.\n\nMore text.\n");
}

#[test]
fn the_text_backend_writes_an_inline_top_level_as_it_writes_a_paragraph() {
    // A fragment with no blank line and no block has an inline top level and
    // no paragraph; a displayed group or a display environment in it takes
    // lines of its own with no blank line beside it, as inside a paragraph.
    let text = |document: &str| text_of(&standard_run(text::WORKFLOW, document));
    assert_eq!(
        text("Text before\n\\[ a = b \\]\ntext after.\n"),
        "Text before\n  \\[ a = b \\]\ntext after.\n"
    );
    assert_eq!(
        text("Text before\n\\begin{equation}\n  a = b\n\\end{equation}\ntext after.\n"),
        "Text before\n  \\begin{equation}\n    a = b\\tag*{(1)}\n  \\end{equation}\ntext after.\n"
    );
}
