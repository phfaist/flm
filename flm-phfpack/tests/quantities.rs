//! The quantity macros — entropies, information measures, the identity
//! process, the exponential, the trivial system — in one formula each
//! (acceptance tests 20 to 22). The expected replacements are what the LaTeX
//! package `phfqit` renders for the same uses — the parenthesis of a quantity
//! left out when every part it holds is blank — a sub- or superscript that
//! would hold nothing left out.
//!
//! The compilation test is skipped unless the environment variable
//! `FLM_TESTS_LATEX_COMPILE` is set, as in `tests/macros.rs`.

mod common;

use std::sync::Arc;

use flm_core::core::registry::FlmRegistry;
use flm_phfpack::settings::PhfqitConfig;

use common::{
    compile_under_both_engines, compiling_enabled, html_of, latex_of, registry, registry_with, text_of,
    ENABLE_VARIABLE,
};

/// Each quantity macro: a use with every optional part written (the size
/// among them, where the macro takes one) and the replacement it gives, then a
/// use with none and its replacement.
const QUANTITIES: &[(&str, &str, &str, &str)] = &[
    (
        r"\HH`\big[\rho][\epsilon]{X}[Y]",
        r"H^{\epsilon}\bigl (X\,\mathclose{}\big \vert\mathopen{}\,Y\bigr )_{\rho}",
        r"\HH{X}",
        r"H(X)",
    ),
    (
        r"\Hmin`\big[\rho][\epsilon]{X}[Y]",
        r"H_{\mathrm{min}}^{\epsilon}\bigl (X\,\mathclose{}\big \vert\mathopen{}\,Y\bigr )_{\rho}",
        r"\Hmin{X}",
        r"H_{\mathrm{min}}(X)",
    ),
    (
        r"\Hzero`\big[\rho][\epsilon]{X}[Y]",
        r"H_{\mathrm{max},0}^{\epsilon}\bigl (X\,\mathclose{}\big \vert\mathopen{}\,Y\bigr )_{\rho}",
        r"\Hzero{X}",
        r"H_{\mathrm{max},0}(X)",
    ),
    (
        r"\Hmaxf`\big[\rho][\epsilon]{X}[Y]",
        r"H_{\mathrm{max}}^{\epsilon}\bigl (X\,\mathclose{}\big \vert\mathopen{}\,Y\bigr )_{\rho}",
        r"\Hmaxf{X}",
        r"H_{\mathrm{max}}(X)",
    ),
    (r"\Hfn`\big(x)", r"H\bigl (x\bigr )", r"\Hfn(x)", r"H(x)"),
    (
        r"\II`\big[\rho][\epsilon]{A}{C}[B]",
        r"I^{\epsilon}\bigl (A\,:\,C\,\mathclose{}\big \vert\mathopen{}\,B\bigr )_{\rho}",
        r"\II{A}{C}",
        r"I(A\,:\,C)",
    ),
    (
        r"\DD_{a}^{b}`\big{\rho}{\sigma}",
        r"D_{a}^{b}\bigl (\rho\,\mathclose{}\big \Vert\mathopen{}\,\sigma\bigr )",
        r"\DD{\rho}{\sigma}",
        r"D(\rho\,\Vert\,\sigma)",
    ),
    (
        r"\Dmax[\epsilon]`\big{\rho}{\sigma}",
        r"D_{\mathrm{max}}^{\epsilon}\bigl (\rho\,\mathclose{}\big \Vert\mathopen{}\,\sigma\bigr )",
        r"\Dmax{\rho}{\sigma}",
        r"D_{\mathrm{max}}(\rho\,\Vert\,\sigma)",
    ),
    (
        r"\Dminz[\epsilon]`\big{\rho}{\sigma}",
        r"D_{\mathrm{min,0}}^{\epsilon}\bigl (\rho\,\mathclose{}\big \Vert\mathopen{}\,\sigma\bigr )",
        r"\Dminz{\rho}{\sigma}",
        r"D_{\mathrm{min,0}}(\rho\,\Vert\,\sigma)",
    ),
    (
        r"\Dminf[\epsilon]`\big{\rho}{\sigma}",
        r"D_{\mathrm{min}}^{\epsilon}\bigl (\rho\,\mathclose{}\big \Vert\mathopen{}\,\sigma\bigr )",
        r"\Dminf{\rho}{\sigma}",
        r"D_{\mathrm{min}}(\rho\,\Vert\,\sigma)",
    ),
    (
        r"\Dr[\epsilon]`\big{\rho}{\sigma}",
        r"D_{\mathrm{r}}^{\epsilon}\bigl (\rho\,\mathclose{}\big \Vert\mathopen{}\,\sigma\bigr )",
        r"\Dr{\rho}{\sigma}",
        r"D_{\mathrm{r}}(\rho\,\Vert\,\sigma)",
    ),
    (
        r"\DHyp[\epsilon]`\big{\rho}{\sigma}",
        r"D_{\mathrm{H}}^{\epsilon}\bigl (\rho\,\mathclose{}\big \Vert\mathopen{}\,\sigma\bigr )",
        r"\DHyp{\rho}{\sigma}",
        r"D_{\mathrm{H}}^{\eta}(\rho\,\Vert\,\sigma)",
    ),
    (
        r"\Dhyp[\epsilon]`\big{\rho}{\sigma}",
        r"D_{\mathrm{h}}^{\epsilon}\bigl (\rho\,\mathclose{}\big \Vert\mathopen{}\,\sigma\bigr )",
        r"\Dhyp{\rho}{\sigma}",
        r"D_{\mathrm{h}}^{\eta}(\rho\,\Vert\,\sigma)",
    ),
    (
        r"\IdentProc`\big[A][A']{\rho}",
        r"\operatorname{id}_{A\to A'}\bigl (\rho\bigr )",
        r"\IdentProc{\rho}",
        r"\operatorname{id}(\rho)",
    ),
    (r"\ee^{x}", r"e^{x}", r"\ee", r"e"),
    (r"\emptysystem", r"\emptyset", r"\emptysystem", r"\emptyset"),
];

/// Uses of the quantities whose parenthesis holds blank parts, and the
/// replacement each gives: the parenthesis is left out when every part it
/// holds is blank, as the LaTeX package leaves it out, and written when any
/// one of them is given.
const BLANK_PARTS: &[(&str, &str)] = &[
    (r"\Hmaxf{}", r"H_{\mathrm{max}}"),
    (r"\HH{}", r"H"),
    (r"\HH[][\epsilon]{}", r"H^{\epsilon}"),
    (r"\HH[\rho]{}", r"H()_{\rho}"),
    (r"\Hmin{}[Y]", r"H_{\mathrm{min}}(\,\vert\,Y)"),
    (r"\II{}{}", r"I"),
    (r"\II{}{C}", r"I(\,:\,C)"),
    (r"\DD{}{}", r"D"),
    (r"\DD_{a}{}{}", r"D_{a}"),
    (r"\Dmax{}{}", r"D_{\mathrm{max}}"),
    (r"\DHyp{}{}", r"D_{\mathrm{H}}^{\eta}"),
    (r"\Dr{}{\sigma}", r"D_{\mathrm{r}}(\,\Vert\,\sigma)"),
];

/// The replacement of the one formula `source` in the text output, which must
/// report nothing.
fn formula(registry: &Arc<FlmRegistry>, source: &str) -> String {
    let (text, diagnostics) = text_of(registry, &format!(r"\({source}\)"));
    assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
    text.strip_prefix(r"\(").and_then(|t| t.strip_suffix("\\)\n")).unwrap_or(&text).to_string()
}

/// Every use of [`QUANTITIES`] and of [`BLANK_PARTS`] with its replacement.
fn uses() -> impl Iterator<Item = (&'static str, &'static str)> {
    QUANTITIES
        .iter()
        .flat_map(|(full, full_expected, none, none_expected)| {
            [(*full, *full_expected), (*none, *none_expected)]
        })
        .chain(BLANK_PARTS.iter().copied())
}

#[test]
fn the_test_names_sixteen_quantity_macros() {
    assert_eq!(QUANTITIES.len(), 16);
}

#[test]
fn each_quantity_macro_is_replaced_in_the_text_output() {
    let registry = registry();
    for (use_, expected) in uses() {
        assert_eq!(formula(&registry, use_), expected, "{use_}");
    }
}

#[test]
fn each_quantity_macro_is_replaced_in_the_formula_the_web_page_hands_the_renderer() {
    let registry = registry();
    for (use_, expected) in uses() {
        let (html, diagnostics) = html_of(&registry, &format!(r"\({use_}\)"));
        assert!(diagnostics.is_empty(), "{use_}: {diagnostics:?}");
        assert!(html.contains(&format!(r"\({expected}\)")), "{use_}: {html}");
    }
}

#[test]
fn each_quantity_macro_is_replaced_in_the_recomposed_latex() {
    let registry = registry();
    for (use_, expected) in uses() {
        let (latex, diagnostics) = latex_of(&registry, &format!(r"\({use_}\)"));
        assert!(diagnostics.is_empty(), "{use_}: {diagnostics:?}");
        assert!(latex.contains(&format!(r"\({expected}\)")), "{use_}: {latex}");
    }
}

#[test]
fn the_recomposed_latex_of_each_quantity_macro_compiles_under_both_engines() {
    if !compiling_enabled() {
        println!("skipped: set {ENABLE_VARIABLE} to run the LaTeX compilation check");
        return;
    }
    let registry = registry();
    let source: String = uses().map(|(use_, _)| format!("\\({use_}\\)\n\n")).collect();
    let (body, diagnostics) = latex_of(&registry, &source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    for (use_, expected) in uses() {
        assert!(body.contains(expected), "{use_}: {body}");
    }
    compile_under_both_engines("quantities", &body);
}

#[test]
fn an_entropy_takes_a_size_command_or_a_star_after_a_backtick() {
    let registry = registry();
    assert_eq!(formula(&registry, r"\Hmin`\Big{X}"), r"H_{\mathrm{min}}\Bigl (X\Bigr )");
    assert_eq!(
        formula(&registry, r"\Hmin`*[\rho]{X}[Y]"),
        r"H_{\mathrm{min}}\mathopen{}\left (X\,\mathclose{}\middle \vert\mathopen{}\,Y\right )\mathclose{}_{\rho}"
    );
}

#[test]
fn a_relative_entropy_takes_its_subscript_and_superscript_in_either_order_or_not_at_all() {
    let registry = registry();
    for (use_, expected) in [
        (r"\DD_{a}{\rho}{\sigma}", r"D_{a}(\rho\,\Vert\,\sigma)"),
        (r"\DD^{b}{\rho}{\sigma}", r"D^{b}(\rho\,\Vert\,\sigma)"),
        (r"\DD_{a}^{b}{\rho}{\sigma}", r"D_{a}^{b}(\rho\,\Vert\,\sigma)"),
        (r"\DD^{b}_{a}{\rho}{\sigma}", r"D^{b}_{a}(\rho\,\Vert\,\sigma)"),
        (r"\DD{\rho}{\sigma}", r"D(\rho\,\Vert\,\sigma)"),
    ] {
        assert_eq!(formula(&registry, use_), expected, "{use_}");
    }
}

#[test]
fn the_hypothesis_testing_relative_entropy_defaults_to_eta() {
    let registry = registry();
    assert_eq!(formula(&registry, r"\DHyp{\rho}{\sigma}"), r"D_{\mathrm{H}}^{\eta}(\rho\,\Vert\,\sigma)");
    assert_eq!(formula(&registry, r"\Dhyp[\eta']{\rho}{\sigma}"), r"D_{\mathrm{h}}^{\eta'}(\rho\,\Vert\,\sigma)");
}

#[test]
fn the_identity_process_writes_its_parenthesis_only_around_an_argument() {
    let registry = registry();
    assert_eq!(formula(&registry, r"\IdentProc{}"), r"\operatorname{id}");
    assert_eq!(formula(&registry, r"\IdentProc[A][A']{}"), r"\operatorname{id}_{A\to A'}");
    assert_eq!(formula(&registry, r"\IdentProc[A]{\rho}"), r"\operatorname{id}_{A}(\rho)");
}

#[test]
fn the_exponential_writes_its_exponent_when_one_follows() {
    let registry = registry();
    assert_eq!(formula(&registry, r"\ee^{x}"), r"e^{x}");
    assert_eq!(formula(&registry, r"\ee"), r"e");
}

#[test]
fn a_declared_quantity_macro_with_the_arguments_of_an_entropy_renders_like_the_built_in_one() {
    // An entropy of the document's own, declared with the arguments of
    // `\Hmin` and its template: it gives what `\Hmin` gives.
    let hmin_template = r"H_{\mathrm{min}}\notblank{#2}{^{#2}}{}#[(]{DelimLeft}#3\notblank{#4}{\,#[\vert]{DelimMiddle}\,#4}{}#[)]{DelimRight}\notblank{#1}{_{#1}}{}";
    let renyi_template = hmin_template.replace(r"\mathrm{min}", r"\alpha");
    let factory = flm_phfpack::factory().expect("the factory");
    let decl = factory.settings(flm_phfpack::FEATURE).expect("declared");
    let serde_json::Value::Object(data) = serde_json::json!({
        "macro_definitions": {
            "Hminx": {
                "args": ["SizeArgBacktick", "[", "[", "{", "["],
                "argument_number_offset": 1,
                "content": hmin_template,
            },
            "Halpha": {
                "args": ["SizeArgBacktick", "[", "[", "{", "["],
                "argument_number_offset": 1,
                "content": renyi_template,
            },
        }
    }) else {
        panic!("a map")
    };
    let settings = decl.read(&data).expect("the settings read");
    let registry = registry_with(settings.config::<PhfqitConfig>().expect("the configuration").clone());
    for args in [r"{X}", r"[\rho][\epsilon]{X}[Y]", r"`\Big[\rho]{X}[Y]", r"`*{X}[Y]", r"[][\epsilon]{X}"] {
        let built_in = formula(&registry, &format!(r"\Hmin{args}"));
        assert_eq!(formula(&registry, &format!(r"\Hminx{args}")), built_in, "{args}");
        assert_eq!(
            formula(&registry, &format!(r"\Halpha{args}")),
            built_in.replace(r"\mathrm{min}", r"\alpha"),
            "{args}"
        );
    }
}
