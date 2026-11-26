//! The formulas on the backends `text` and `html`: the rules of the formulas,
//! which stage the element `math-techxt.rendered-as-text`, and the rules of
//! the element, which write it in each backend's own elements.

mod common;

use flm_core::core::registry::DefContent;
use flm_core::stdlib::backends::{html, text};
use serde_json::json;

use common::documents::{self, ENVIRONMENT};
use common::{html_of, registry, registry_with, selected_rule, text_of};

/// The diagnostics of the extension's in `diagnostics`.
fn own(diagnostics: &[String]) -> Vec<&str> {
    diagnostics.iter().map(String::as_str).filter(|d| d.starts_with("math-techxt.")).collect()
}

#[test]
fn the_plans_select_the_rules_of_the_formulas_and_of_the_element() {
    let registry = registry();
    for (workflow, backend) in [(text::WORKFLOW, "text"), (html::WORKFLOW, "html")] {
        let rule = |element| selected_rule(&registry, workflow, backend, element);
        assert_eq!(rule("flm.math"), Some(format!("math-techxt.math.lower.{backend}")));
        assert_eq!(rule("flm.math.environment"), Some(format!("math-techxt.math-environment.lower.{backend}")));
        assert_eq!(rule("math-techxt.rendered-as-text"), Some(format!("math-techxt.rendered-as-text.lower.{backend}")));
    }
}

#[test]
fn an_inline_formula_is_text_within_its_line() {
    let registry = registry();
    let source = documents::INLINE;
    let (text, diagnostics) = text_of(&registry, source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert_eq!(text, "Let 𝑎 + b = 𝑐 hold.\n");
    let (html, requires, diagnostics) = html_of(&registry, source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert_eq!(
        html,
        "Let <span class=\"math-techxt flm-math-formula\" data-flm-display=\"inline\">𝑎 + <span class=\"flm-pres-math--text\" data-flm-command=\"mbox\">b</span> \
         = 𝑐</span> hold. "
    );
    assert_eq!(requires, ["flm.html.style.math-passthrough", "math-techxt.style"]);
}

#[test]
fn a_displayed_formula_is_set_on_lines_of_its_own() {
    let registry = registry();
    let source = documents::DISPLAYED;
    let (text, diagnostics) = text_of(&registry, source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert_eq!(text, "We have\n\n  𝑥² for all 𝑦\n\nhence.\n");
    let (html, _, diagnostics) = html_of(&registry, source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert_eq!(
        html,
        "We have<span class=\"math-techxt math-techxt-display flm-math-formula\" data-flm-display=\"display\">𝑥² <span class=\"flm-pres-math--text\" \
         data-flm-command=\"textbf\">for all</span> 𝑦</span>hence. "
    );
}

#[test]
fn a_matrix_in_a_displayed_math_group_is_drawn_over_two_lines() {
    let registry = registry();
    let source = documents::DISPLAYED_MATRIX;
    let (text, diagnostics) = text_of(&registry, source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert_eq!(text, "We have\n\n  ⎛  1  2 ⎞\n  ⎝ 30  4 ⎠\n\nhence.\n");
    let (html, _, diagnostics) = html_of(&registry, source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert_eq!(html, "We have<span class=\"math-techxt math-techxt-display flm-math-formula\" data-flm-display=\"display\">⎛  1  2 ⎞\n⎝ 30  4 ⎠</span>hence. ");
}

#[test]
fn a_matrix_in_an_inline_math_group_is_rendered_without_a_diagnostic() {
    let registry = registry();
    let source = documents::INLINE_MATRIX;
    let (text, diagnostics) = text_of(&registry, source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert_eq!(text, "Let 𝑀 = ( 1 2; 3 4 ) hold.\n");
    let (html, _, diagnostics) = html_of(&registry, source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert_eq!(html, "Let <span class=\"math-techxt flm-math-formula\" data-flm-display=\"inline\">𝑀 = ( 1  2; 3  4 )</span> hold. ");
}

#[test]
fn an_environment_shows_each_line_with_its_number_or_tag_and_keeps_its_drawing() {
    let registry = registry();
    let (text, diagnostics) = text_of(&registry, ENVIRONMENT);
    assert_eq!(diagnostics, Vec::<String>::new());
    // The second line of the matrix keeps its leading spaces and its runs of
    // spaces, so that it stays aligned under the first; the number follows
    // the line's last printed line.
    assert_eq!(text, "Lines:\n\n  𝑎 = ⎛  1  2 ⎞\n      ⎝ 30  4 ⎠ (1)\n  𝑏 ≤ c (T)\n\nDone.\n");
    let (html, _, diagnostics) = html_of(&registry, ENVIRONMENT);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert_eq!(
        html,
        "Lines:<span class=\"math-techxt math-techxt-display flm-math-formula\" data-flm-display=\"display\" data-flm-env=\"align\">\
         <span class=\"math-techxt-line\" id=\"eq-a\"><span class=\"math-techxt-text\">𝑎 = ⎛  1  2 ⎞\n    ⎝ 30  4 ⎠\
         </span><span class=\"math-techxt-tag\">(1)</span></span>\
         <span class=\"math-techxt-line\" id=\"eq--2oqua\"><span class=\"math-techxt-text\">𝑏 ≤ \
         <span class=\"flm-pres-math--text\" data-flm-command=\"text\">c</span></span>\
         <span class=\"math-techxt-tag\">(T)</span></span></span>Done. "
    );
}

#[test]
fn a_line_that_shows_nothing_at_its_tag_is_written_without_one() {
    let registry = registry();
    let source = documents::UNTAGGED_LINE;
    let (text, diagnostics) = text_of(&registry, source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert_eq!(text, "  𝑎\n  𝑏 (1)\n");
    let (html, _, diagnostics) = html_of(&registry, source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert_eq!(
        html,
        "<span class=\"math-techxt math-techxt-display flm-math-formula\" data-flm-display=\"display\" data-flm-env=\"align\">\
         <span class=\"math-techxt-line\" id=\"eq--1opqm\"><span class=\"math-techxt-text\">𝑎</span></span>\
         <span class=\"math-techxt-line\" id=\"eq--2oqua\"><span class=\"math-techxt-text\">𝑏</span>\
         <span class=\"math-techxt-tag\">(1)</span></span></span>"
    );
}

#[test]
fn a_reference_to_an_equation_line_prints_its_number() {
    let registry = registry();
    let source = documents::reference();
    let (text, diagnostics) = text_of(&registry, &source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert!(text.ends_with("Done. See (1).\n"), "{text}");
}

#[test]
fn a_reference_inside_a_text_command_shows_the_number_and_links_on_html() {
    let registry = registry();
    let source = documents::reference_in_text();
    let (text, diagnostics) = text_of(&registry, &source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert!(text.ends_with("Done. Then 𝑥 see (1).\n"), "{text}");
    let (html, _, diagnostics) = html_of(&registry, &source);
    assert_eq!(diagnostics, Vec::<String>::new());
    let expected = "Then <span class=\"math-techxt flm-math-formula\" data-flm-display=\"inline\">𝑥 <span class=\"flm-pres-math--text\" data-flm-command=\"text\">\
                    see <a class=\"flm-pres-link\" href=\"#eq-a\" data-flm-kind=\"ref\">(1)</a></span></span>. ";
    assert!(html.ends_with(expected), "{html}");
}

#[test]
fn a_formula_inside_a_text_command_of_a_formula_is_rendered_too() {
    let registry = registry();
    let source = documents::NESTED;
    let (text, diagnostics) = text_of(&registry, source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert_eq!(text, "𝑎 for all 𝑏²\n");
    let (html, _, diagnostics) = html_of(&registry, source);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert_eq!(
        html,
        "<span class=\"math-techxt flm-math-formula\" data-flm-display=\"inline\">𝑎 <span class=\"flm-pres-math--text\" data-flm-command=\"text\">for all \
         <span class=\"math-techxt flm-math-formula\" data-flm-display=\"inline\">𝑏²</span></span></span> "
    );
}

#[test]
fn a_formula_without_a_row_is_written_as_its_tex() {
    let registry = registry();
    let source = documents::FORMULA_WITHOUT_ROW;
    let (text, diagnostics) = text_of(&registry, source);
    assert_eq!(own(&diagnostics).len(), 1, "{diagnostics:?}");
    assert_eq!(text, "A \\(a + \u{E001} \\text{b}\\) c.\n");
    let (html, _, diagnostics) = html_of(&registry, source);
    assert_eq!(own(&diagnostics).len(), 1, "{diagnostics:?}");
    assert!(html.contains("data-flm-display=\"inline\""), "{html}");
}

#[test]
fn an_environment_without_a_row_is_written_as_its_tex_with_its_numbers_and_tags() {
    let registry = registry();
    let source = documents::ENVIRONMENT_WITHOUT_ROW;
    let (text, diagnostics) = text_of(&registry, source);
    assert_eq!(own(&diagnostics).len(), 1, "{diagnostics:?}");
    assert_eq!(text, "  \\begin{align}\n    a + \u{E001}  \\tag*{(1)}\\\\[2pt]\n    b\\tag*{(U)}\n  \\end{align}\n");
    let (html, _, _) = html_of(&registry, source);
    assert!(html.contains("<span class=\"flm-pres-math--text\" id=\"eq-z\"></span>"), "{html}");
}

#[test]
fn the_stylesheet_loads_the_font_of_the_setting() {
    let registry = registry_with(json!({ "font_url": r#"fonts/Julia"Mono\x.woff2"# }));
    let (_, style) = registry
        .styles()
        .iter()
        .find(|(_, style)| style.id.as_str() == "math-techxt.style")
        .expect("the stylesheet is registered");
    let [DefContent::Inline(text)] = style.content.as_slice() else { panic!("one inline part: {style:?}") };
    assert!(text.contains(r#"src: url("fonts/Julia\"Mono\\x.woff2") format("woff2");"#), "{text}");
    assert!(text.contains("@layer flm {"), "{text}");
}
