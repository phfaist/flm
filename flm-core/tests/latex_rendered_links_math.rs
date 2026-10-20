//! Links and mathematics on the `latex-rendered` backend, end to end over the
//! standard package: references, hyperlinks (`\\href`, `\\url`, `\\email`), a
//! link inside a link, the `target` key, an address holding `~ % # & _`;
//! inline and displayed mathematics, every environment with numbered and
//! unnumbered lines, tags, labels and references to them, `\\text` inside a
//! formula and a formula inside `\\text`.
//!
//! Goldens live under `tests/goldens/latex-rendered/`, in the shape of
//! `tests/latex_rendered.rs`; `tests/latex_rendered_compile.rs` compiles every
//! one with `lualatex`.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::Run;
use flm_core::dump::golden::assert_text_golden;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::backends::latex_rendered::{DOCUMENT, WORKFLOW, style_artifact};
use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};

// --- helpers -------------------------------------------------------------------------------

/// The standard package with every heading kind numbered.
fn registry() -> Arc<FlmRegistry> {
    common::stdcore::registry(
        StdcoreConfig::new().headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered)),
    )
}

/// The render request of every `latex-rendered` run of this file, with
/// `options` set.
fn overrides(options: OptionValues) -> Overrides {
    Overrides {
        render_options: options,
        ..Overrides::none()
    }
}

/// Runs the workflow over `text`, asserting that nothing was reported.
fn render(text: &str) -> Run {
    let run = common::stdcore::run_workflow(&registry(), WORKFLOW, &[text], &overrides(OptionValues::new()));
    assert!(run.diagnostics().iter().next().is_none(), "{}", run.diagnostics().render_all());
    run
}

/// The entry artifact's text.
fn document(run: &Run) -> String {
    let set = run.table(DOCUMENT).expect("declared").unwrap_or_else(|| {
        panic!("no latex-rendered output; diagnostics:\n{}", run.diagnostics().render_all())
    });
    let entry = set.entry().expect("the entry artifact");
    String::from_utf8(entry.as_bytes().expect("bytes").to_vec()).expect("UTF-8")
}

/// The style file's text and its path.
fn style(run: &Run) -> (String, String) {
    let set = run.table(DOCUMENT).expect("declared").expect("written");
    let style = style_artifact(set).expect("the style file");
    let text = String::from_utf8(style.as_bytes().expect("bytes").to_vec()).expect("UTF-8");
    let path = style.fixed_path.as_ref().map(|path| path.as_str().to_string()).unwrap_or_default();
    (text, path)
}

/// Asserts the run's document and style file against
/// `tests/goldens/latex-rendered/<name>`, each under a marker line.
fn assert_golden(name: &str, run: &Run) {
    let (style, path) = style(run);
    let text = format!(
        "% ===== latex-rendered:document =====\n{}% ===== latex-rendered:style {path} =====\n{style}",
        document(run)
    );
    let mut golden = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    golden.push("tests/goldens/latex-rendered");
    golden.push(name);
    assert_text_golden(golden, &text);
}

/// The content of the document: what stands between `\begin{document}` and
/// `\end{document}`.
fn content(run: &Run) -> String {
    let text = document(run);
    let start = text.find("\\begin{document}\n").expect("the skeleton") + "\\begin{document}\n".len();
    let end = text.rfind("\\end{document}").expect("the skeleton");
    String::from(&text[start..end])
}

// --- links ---------------------------------------------------------------------------------

/// References, hyperlinks of the three commands, an address holding
/// `~ % # & _`, and a link inside a link.
const LINKS: &str = "\
\\section{Links}\\label{sec:links}
See \\ref{sec:links} and \\ref{eq:one}.
Visit \\href{https://example.org/~user/a%20b_c?x=1&y=2#frag}{the site},
\\url{https://example.org/path_x/~y}, and \\email{someone@example.org}.
A link around a reference: \\href{https://example.org/outer}{outer, see \\ref{sec:links}}.
\\begin{equation}\\label{eq:one}
a = b
\\end{equation}";

#[test]
fn links_write_their_kind_address_and_target() {
    let run = render(LINKS);
    let latex = content(&run);
    assert!(latex.contains("\\flmxLink[kind=ref, href={\\#sec-links}, target=sec-links]{"), "{latex}");
    assert!(
        latex.contains("\\flmxLink[kind=url, href={https://example.org/\\string~user/a\\%20b_c?x=1&y=2\\#frag}]{the site}"),
        "{latex}"
    );
    assert!(latex.contains("shows-target=true]{"), "{latex}");
    // The reference inside the hyperlink is its content alone.
    assert!(latex.contains("{outer, see §"), "{latex}");
    assert_golden("links.tex", &run);
}

// --- mathematics ---------------------------------------------------------------------------

/// Inline and displayed mathematics, every environment numbered and not,
/// tags, labels and references, `\text` in a formula and a formula in it.
const MATH: &str = "\
\\section{Mathematics}
Inline \\(x^2 + \\text{for all } y\\) and displayed:
\\[ \\int_0^1 f(x)\\,dx \\]
\\begin{equation}\\label{eq:a}
E = mc^2 \\quad \\text{where \\(c\\) is the speed of light}
\\end{equation}
\\begin{equation*}
a = b
\\end{equation*}
\\begin{align}
a &= b \\label{eq:b} \\\\
c &= d \\nonumber \\\\
e &= f \\tag{\\(\\star\\)}\\label{eq:star}
\\end{align}
\\begin{align*}
a &= b \\\\
c &= d
\\end{align*}
\\begin{gather}
x = y \\label{eq:c}
\\end{gather}
\\begin{gather*}
x = y
\\end{gather*}
See \\eqref{eq:a}, \\eqref{eq:b}, \\eqref{eq:star} and \\eqref{eq:c}.";

#[test]
fn mathematics_writes_its_source_between_delimiters_with_starred_environments() {
    let run = render(MATH);
    let latex = content(&run);
    assert!(latex.contains("\\(x^2 + \\text{for all } y\\)"), "{latex}");
    assert!(latex.contains("\\begin{equation*}"), "{latex}");
    assert!(latex.contains("\\begin{align*}"), "{latex}");
    assert!(latex.contains("\\begin{gather*}"), "{latex}");
    assert!(!latex.contains("\\begin{equation}"), "{latex}");
    // A line's anchor is a target in the formula, and a line's tag the
    // content alone inside `\tag*{…}`.
    assert!(latex.contains("\\flmxTarget{"), "{latex}");
    assert!(latex.contains("\\tag*{"), "{latex}");
    assert_golden("math.tex", &run);
}

#[test]
fn a_command_on_a_line_of_its_own_leaves_no_blank_line_in_an_environment() {
    // The line's source is written under the `math` library's newline rule,
    // as in the recomposed LaTeX: a `\label` or a final `\\` taken out of a
    // line of its own leaves one newline, and the run that ended the line is
    // written after its tag and its `\\`.
    let run = render(common::latex::COMMANDS_ON_LINES_OF_THEIR_OWN);
    let latex = content(&run);
    assert_eq!(common::latex::blank_lines_in_math(&latex), Vec::<usize>::new(), "{latex}");
    assert!(latex.contains("\\end{aligned}\\tag*{(1)}\n\\end{gather*}"), "{latex}");
    assert!(latex.contains("  a = b\n  + c\\tag*{"), "{latex}");
    assert!(latex.contains(" %% note\n  c &= d\\tag*{"), "{latex}");
    assert!(latex.contains("  x \\to \\infty\\tag*{"), "{latex}");
}
