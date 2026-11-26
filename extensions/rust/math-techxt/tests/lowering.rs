//! The smallest case of the lowering rounds through the extension: on `text`,
//! a math group lowers to the element `math-techxt.rendered-as-text`, with a
//! text command's content in a named slot, and another round lowers the
//! element with its own rule.

mod common;

use flm_core::stdlib::backends::text;

use common::{diagnostics, registry, rounds, run, selected_rule, text_output};

#[test]
fn the_text_plan_selects_the_rule_of_the_formula_and_the_rule_of_the_element() {
    let registry = registry();
    let rule = |element| selected_rule(&registry, text::WORKFLOW, "text", element);
    assert_eq!(rule("flm.math").as_deref(), Some("math-techxt.math.lower.text"));
    assert_eq!(rule("math-techxt.rendered-as-text").as_deref(), Some("math-techxt.rendered-as-text.lower.text"));
}

#[test]
fn a_formula_renders_as_unicode_text_with_its_text_command_lowered_in_place() {
    let registry = registry();
    let run = run(&registry, text::WORKFLOW, "Let \\(\\alpha + \\frac{1}{2} \\le x \\text{ for \\textbf{all} } x\\) hold.\n");
    assert_eq!(diagnostics(&run), Vec::<String>::new());
    assert_eq!(text_output(&run), "Let α + 1/2 ≤ 𝑥 for all 𝑥 hold.\n");
    // The rule of the formula stages the element; a second round lowers it.
    assert_eq!(rounds(&run, "text"), 2);
}

#[test]
fn a_formula_without_a_text_command_renders_in_one_piece() {
    let registry = registry();
    let run = run(&registry, text::WORKFLOW, "\\(x^2\\)\n");
    assert_eq!(diagnostics(&run), Vec::<String>::new());
    assert_eq!(text_output(&run), "𝑥²\n");
}
