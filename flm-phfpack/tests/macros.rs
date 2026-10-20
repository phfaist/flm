//! Each built-in macro, in one formula: what the three outputs hold, and that
//! the recomposed LaTeX compiles (acceptance test 7). The expected replacements
//! are what the Python package writes for the same formulas.
//!
//! **The compilation test is skipped unless the environment variable
//! `FLM_TESTS_LATEX_COMPILE` is set** (to anything but the empty string or
//! `0`), as the library's own LaTeX compilation suite is; where it is set,
//! `pdflatex` and `lualatex` must both run from the `PATH`:
//!
//! ```text
//! FLM_TESTS_LATEX_COMPILE=1 cargo test -p flm-phfpack --test macros
//! ```
//!
//! It writes every macro's recomposed LaTeX into one minimal `article` that
//! loads `amsmath`, and leaves out the macros of [`NEEDS_MORE_THAN_AMSMATH`],
//! whose replacements name a command or a character that document lacks: the
//! package does not yet say what its replacements need of an output.

mod common;

use common::{compile_under_both_engines, compiling_enabled, html_of, latex_of, registry, text_of, ENABLE_VARIABLE};

/// Each macro's use and the replacement it gives.
const MACROS: &[(&str, &str)] = &[
    (r"\abs{x}", r"\lvert{x}\rvert"),
    (r"\norm{x}", r"\lVert{x}\rVert"),
    (r"\avg{x}", r"\langle{x}\rangle"),
    (r"\ket{x}", r"\lvert{x}\rangle"),
    (r"\bra{x}", r"\langle{x}\rvert"),
    (r"\braket{x}{y}", r"\langle{x}\mkern 1.5mu \vert\mkern 1.5mu {y}\rangle"),
    (r"\ketbra{x}{y}", r"\lvert{x}\rangle \mkern -1.8mu \langle{y}\rvert"),
    (r"\proj{x}", r"\lvert{x}\rangle \mkern -1.8mu \langle{x}\rvert"),
    (r"\matrixel{x}{A}{y}", r"\langle{x}\mkern 1.5mu \vert \mkern 1.5mu {A}\mkern 1.5mu \vert \mkern 1.5mu {y}\rangle"),
    (r"\dmatrixel{x}{y}", r"\langle{x}\mkern 1.5mu \vert \mkern 1.5mu {y}\mkern 1.5mu \vert \mkern 1.5mu {x}\rangle"),
    (r"\innerprod{x}{y}", r"\langle{x},\mkern 1.5mu {y}\rangle"),
    (r"\oket{x}", r"\lvert{x}\rrangle"),
    (r"\obra{x}", r"\llangle{x}\rvert"),
    (r"\obraket{x}{y}", r"\llangle{x}\mkern 1.5mu \vert \mkern 1.5mu {y}\rrangle"),
    (r"\oketbra{x}{y}", r"\lvert{x}\rrangle \mkern -1.8mu \llangle{y}\rvert"),
    (r"\oproj{x}", r"\lvert{x}\rrangle \mkern -1.8mu \llangle{x}\rvert"),
    (r"\omatrixel{x}{A}{y}", r"\llangle{x}\mkern 1.5mu \vert \mkern 1.5mu {A}\mkern 1.5mu \vert \mkern 1.5mu {y}\rrangle"),
    (r"\odmatrixel{x}{y}", r"\llangle{x}\mkern 1.5mu \vert \mkern 1.5mu {y}\mkern 1.5mu \vert \mkern 1.5mu {x}\rrangle"),
    (r"\intervalc{x}{y}", r"[{x\mathclose{},\mathopen{}y}]"),
    (r"\intervalo{x}{y}", r"]{x\mathclose{},\mathopen{}y}["),
    (r"\intervalco{x}{y}", r"[{x\mathclose{},\mathopen{}y}["),
    (r"\intervaloc{x}{y}", r"]{x\mathclose{},\mathopen{}y}]"),
    (r"\Hs", r"\mathscr{H}"),
    (r"\Ident", r"𝟙"),
    (r"\bit{0}", r"\texttt{0}"),
    (r"\bitstring{0110}", r"\ensuremath{\underline{\overline{\texttt{0110}}}}"),
    (r"\gate{C-not}", r"\textsc{C-not}"),
    (r"\AND", r"\textsc{and}"),
    (r"\XOR", r"\textsc{xor}"),
    (r"\CNOT", r"\textsc{c-not}"),
    (r"\NOT", r"\textsc{not}"),
    (r"\NOOP", r"\textsc{no-op}"),
    (r"\uu(n)", r"\mathrm{u}({n})"),
    (r"\UU(n)", r"\mathrm{U}({n})"),
    (r"\su(n)", r"\mathrm{su}({n})"),
    (r"\SU(n)", r"\mathrm{SU}({n})"),
    (r"\so(n)", r"\mathrm{so}({n})"),
    (r"\SO(n)", r"\mathrm{SO}({n})"),
    (r"\slalg(n)", r"\mathrm{sl}({n})"),
    (r"\SL(n)", r"\mathrm{SL}({n})"),
    (r"\GL(n)", r"\mathrm{GL}({n})"),
    (r"\SN(n)", r"\mathrm{S}_{n}"),
    (r"\tr", r"\operatorname{tr}"),
    (r"\supp", r"\operatorname{supp}"),
    (r"\rank", r"\operatorname{rank}"),
    (r"\linspan", r"\operatorname{span}"),
    (r"\spec", r"\operatorname{spec}"),
    (r"\diag", r"\operatorname{diag}"),
    (r"\Re", r"\operatorname{Re}"),
    (r"\Im", r"\operatorname{Im}"),
    (r"\poly", r"\operatorname{poly}"),
];

/// The macros whose replacements need more of a LaTeX document than `amsmath`:
/// `\mathscr` (the package `mathrsfs`), the character `𝟙` (which `pdflatex`
/// cannot typeset without a declaration of its own), and the double angle
/// brackets `\llangle` and `\rrangle` (which no standard package defines under
/// these names).
const NEEDS_MORE_THAN_AMSMATH: &[&str] = &[
    r"\Hs",
    r"\Ident",
    r"\oket",
    r"\obra",
    r"\obraket",
    r"\oketbra",
    r"\oproj",
    r"\omatrixel",
    r"\odmatrixel",
];

/// The name of the macro a use calls: the control word it begins with.
fn macro_name(use_: &str) -> &str {
    let end = use_[1..].find(|c: char| !c.is_ascii_alphabetic()).map_or(use_.len(), |i| i + 1);
    &use_[..end]
}

#[test]
fn the_tables_hold_every_macro_the_test_names() {
    assert_eq!(MACROS.len(), 51);
}

/// A replacement as a formula's source is handed to the text output and to
/// the web page's renderer: inside a formula, `\ensuremath{X}` stands for `X`
/// and is written without the command (the recomposed LaTeX keeps it).
fn handed(expected: &str) -> &str {
    expected.strip_prefix(r"\ensuremath{").and_then(|rest| rest.strip_suffix('}')).unwrap_or(expected)
}

#[test]
fn each_macro_is_replaced_in_the_text_output() {
    let registry = registry();
    for (use_, expected) in MACROS {
        let (text, diagnostics) = text_of(&registry, &format!(r"\({use_}\)"));
        assert!(diagnostics.is_empty(), "{use_}: {diagnostics:?}");
        assert_eq!(text, format!("\\({}\\)\n", handed(expected)), "{use_}");
    }
}

#[test]
fn each_macro_is_replaced_in_the_formula_the_web_page_hands_the_renderer() {
    let registry = registry();
    for (use_, expected) in MACROS {
        let (html, diagnostics) = html_of(&registry, &format!(r"\({use_}\)"));
        assert!(diagnostics.is_empty(), "{use_}: {diagnostics:?}");
        assert!(html.contains(&format!(r"\({}\)", handed(expected))), "{use_}: {html}");
    }
}

#[test]
fn bitstring_written_in_text_is_an_inline_formula() {
    // The template writes `\ensuremath`: in text, the replacement is the
    // inline formula of the template's argument, as if written with `\(…\)`.
    let registry = registry();
    let used = r"The string \bitstring{0110}.";
    let written = r"The string \(\underline{\overline{\texttt{0110}}}\).";
    let (text, diagnostics) = text_of(&registry, used);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "The string \\(\\underline{\\overline{\\texttt{0110}}}\\).\n");
    let (html, diagnostics) = html_of(&registry, used);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(html, html_of(&registry, written).0);
    assert!(html.contains(r#"data-flm-display="inline">\(\underline{\overline{"#), "{html}");
}

#[test]
fn each_macro_is_replaced_in_the_recomposed_latex() {
    let registry = registry();
    for (use_, expected) in MACROS {
        let (latex, diagnostics) = latex_of(&registry, &format!(r"\({use_}\)"));
        assert!(diagnostics.is_empty(), "{use_}: {diagnostics:?}");
        assert!(latex.contains(&format!(r"\({expected}\)")), "{use_}: {latex}");
    }
}

#[test]
fn the_recomposed_latex_of_each_macro_compiles_under_both_engines() {
    if !compiling_enabled() {
        println!("skipped: set {ENABLE_VARIABLE} to run the LaTeX compilation check");
        return;
    }
    let registry = registry();
    let uses: Vec<&str> =
        MACROS.iter().map(|(use_, _)| *use_).filter(|use_| !NEEDS_MORE_THAN_AMSMATH.contains(&macro_name(use_))).collect();
    assert_eq!(uses.len(), MACROS.len() - NEEDS_MORE_THAN_AMSMATH.len(), "every left-out macro is in the table");
    let source: String = uses.iter().map(|use_| format!("\\({use_}\\)\n\n")).collect();
    let (body, diagnostics) = latex_of(&registry, &source);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    for use_ in &uses {
        let expected = MACROS.iter().find(|(u, _)| u == use_).map(|(_, e)| *e).unwrap_or_default();
        assert!(body.contains(expected), "{use_}: {body}");
    }
    compile_under_both_engines("macros", &body);
}
