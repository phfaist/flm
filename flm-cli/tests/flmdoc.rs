//! The extension `flmdoc`, compiled into the binary (cargo feature `bundled-extensions`):
//! the markup of documentation pages. Each document of `tests/documents/flmdoc/` lists
//! `flmdoc` in its `use` list; the tests render it with the profiles `text` and `html` (the
//! HTML without a compose template, `-t none`, so that the output is the content alone) and
//! compare the output to the goldens of `tests/goldens/flmdoc/` (`UPDATE_GOLDEN=1` rewrites
//! them).
#![cfg(feature = "bundled-extensions")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use flm_core::dump::golden::assert_text_golden;

/// A fresh folder of the test's own, `NAME` under the cargo target's folder for tests,
/// holding a copy of the document `document` of `tests/documents/flmdoc/`.
fn folder(name: &str, document: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("flmdoc").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).unwrap();
    }
    std::fs::create_dir_all(&dir).unwrap();
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents/flmdoc").join(document);
    std::fs::copy(source, dir.join(document)).unwrap();
    dir
}

/// The path of the golden `name`.
fn golden(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/goldens/flmdoc").join(name)
}

/// Runs `flm render` with `args` in `dir`, with no extension installed on the search path
/// (`HOME` is the folder, `FLMPATH` is unset).
fn flm(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flm"))
        .arg("render")
        .args(args)
        .current_dir(dir)
        .env("HOME", dir)
        .env_remove("FLMPATH")
        .env_remove("USERPROFILE")
        .output()
        .unwrap()
}

/// The standard output of a render that succeeded with nothing reported.
fn rendered(dir: &Path, args: &[&str]) -> String {
    let output = flm(dir, args);
    assert!(output.status.success() && output.stderr.is_empty(), "{args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap()
}

/// `document` rendered with the profile `text`, and with the profile `html` without a
/// compose template, each after the options `options`.
fn text_and_html(dir: &Path, document: &str, options: &[&str]) -> (String, String) {
    let text = rendered(dir, &[&["-p", "text"], options, &[document]].concat());
    let html = rendered(dir, &[&["-p", "html", "-t", "none"], options, &[document]].concat());
    (text, html)
}

/// The code spans: one, two or three backticks, a run of backticks shorter than the
/// delimiters inside, the space removed at each end, the spaces kept, markup characters read
/// as raw text, and spans wrapped across lines, whose line endings are spaces. Each is
/// inline code: `<code>` in HTML, its characters in text.
#[test]
fn code_spans_render_as_inline_code() {
    let dir = folder("code-spans", "backticks.flm");
    let (text, html) = text_and_html(&dir, "backticks.flm", &[]);
    for expected in [
        "backtick: x.",
        "single one: a`b.",
        "a``b.",
        "removed: `a` holds",
        "and x holds",
        "only is kept:  x.",
        "\\emph{x} % y",
        "over two lines,",
        "and x has a line ending",
    ] {
        assert!(text.contains(expected), "{expected:?} in {text}");
    }
    let code = |content: &str| {
        format!("<code class=\"flm-pres-verbatim\" data-flm-kind=\"code\" data-flm-display=\"inline\">{content}</code>")
    };
    for content in ["x", "a`b", "a``b", "`a`", "x", " x", "  ", "\\emph{x} % y", "{", "}", "over two lines"] {
        assert!(html.contains(&code(content)), "{content:?} in {html}");
    }
    assert_text_golden(golden("backticks.txt"), &text);
    assert_text_golden(golden("backticks.html"), &html);
}

/// Inside mathematics a backtick is an ordinary character: the formula keeps its backticks,
/// and only the code span in text becomes inline code.
#[test]
fn a_backtick_inside_mathematics_is_an_ordinary_character() {
    let dir = folder("math", "math.flm");
    let (text, html) = text_and_html(&dir, "math.flm", &[]);
    assert!(text.contains("character, \\(a`b`c\\), while"), "{text}");
    let math = "<span class=\"flm-pres-math--passthrough\" data-flm-display=\"inline\">\\(a`b`c\\)</span>";
    assert!(html.contains(math), "{html}");
    assert_eq!(html.matches("<code").count(), 1, "{html}");
    let code = "<code class=\"flm-pres-verbatim\" data-flm-kind=\"code\" data-flm-display=\"inline\">x</code>";
    assert!(html.contains(code), "{html}");
}

/// One documentation item with its signature, its summary, a paragraph, two lists of
/// parameters (the default title and a title given; a parameter with a name, with a role,
/// with neither, with no text and with paragraphs) and three labeled facts.
#[test]
fn a_documentation_item_renders_its_signature_summary_parameters_and_facts() {
    let dir = folder("item", "item.flm");
    let (text, html) = text_and_html(&dir, "item.flm", &[]);
    for expected in [
        "macro \\section\n\n\\section*[short]{title}\n\nStarts a section",
        "Arguments\n\n* star: The section is not numbered.",
        "{title} title (flm.headings.title): The title.",
        "{body}\n\nThe first paragraph",
        "Fields\n\n\\label{…} label (flm.refs.defined-label): The label",
        "Declared by: the feature headings <flm info features/headings>",
    ] {
        assert!(text.contains(expected), "{expected:?} in {text}");
    }
    for expected in [
        "<div class=\"flm-pres-block flmdoc-item\" id=\"macro-section\" data-flm-kind=\"macro\">",
        "<span class=\"flm-pres-span flmdoc-item-kind\">macro</span>",
        "<div class=\"flm-pres-block flmdoc-signature\">",
        "<span class=\"flm-pres-span flmdoc-summary\">",
        "<ul class=\"flm-pres-list\" data-flm-kind=\"flmdoc.params\">",
        "<span class=\"flm-pres-span flmdoc-param-role\">flm.headings.title</span>",
        "<strong class=\"flm-pres-span flmdoc-field-key\" data-flm-style=\"bold\">Declared by</strong>: ",
    ] {
        assert!(html.contains(expected), "{expected:?} in {html}");
    }
    assert_text_golden(golden("item.txt"), &text);
    assert_text_golden(golden("item.html"), &html);
}

/// The arguments read as raw text take any pair of delimiters, so that one holding a brace
/// alone is written between others: the name and the signature of `\{` between `|…|`, a
/// parameter's form between `+…+`, a page's path between `(…)`; braces still pair up.
#[test]
fn raw_text_between_other_delimiters_may_hold_a_brace_alone() {
    let dir = folder("delimiters", "delimiters.flm");
    let (text, html) = text_and_html(&dir, "delimiters.flm", &[]);
    for expected in [
        "macro \\{\n\n\\{\n\nWrites an opening brace.",
        "Forms\n\n{: A brace alone",
        "{text} text: A form",
        "See also: the closing brace <flm info macros/close-brace>",
    ] {
        assert!(text.contains(expected), "{expected:?} in {text}");
    }
    for expected in [
        "<div class=\"flm-pres-block flmdoc-item\" id=\"macro-open-brace\" data-flm-kind=\"macro\">",
        "data-flm-display=\"inline\">\\{</code>",
        "data-flm-display=\"block\"><code>\\{</code></pre>",
        "href=\"flm info macros/close-brace\" data-flm-kind=\"flmdoc.page\"",
    ] {
        assert!(html.contains(expected), "{expected:?} in {html}");
    }
    assert_text_golden(golden("delimiters.txt"), &text);
    assert_text_golden(golden("delimiters.html"), &html);
}

/// `\docref` writes the command that shows the page by default, and the page's path followed
/// by `.html` under `-O flmdoc.link_style=path`; `command` given explicitly is the default.
#[test]
fn a_page_link_writes_its_address_as_the_link_style_says() {
    let dir = folder("links", "links.flm");
    let (text, html) = text_and_html(&dir, "links.flm", &[]);
    assert!(text.contains("headings <flm info features/headings>"), "{text}");
    assert!(html.contains("href=\"flm info features/headings\" data-flm-kind=\"flmdoc.page\""), "{html}");
    assert_text_golden(golden("links.txt"), &text);
    assert_text_golden(golden("links.html"), &html);
    assert_eq!(text_and_html(&dir, "links.flm", &["-O", "flmdoc.link_style=command"]), (text, html));
    let (text, html) = text_and_html(&dir, "links.flm", &["-O", "flmdoc.link_style=path"]);
    assert!(text.contains("headings <features/headings.html>"), "{text}");
    assert!(html.contains("href=\"features/headings.html\" data-flm-kind=\"flmdoc.page\""), "{html}");
    assert_text_golden(golden("links-path.txt"), &text);
    assert_text_golden(golden("links-path.html"), &html);
}

/// A value of `link_style` that is neither `command` nor `path` is an Error at each
/// `\docref`.
#[test]
fn a_link_style_of_another_value_is_an_error_at_each_link() {
    let dir = folder("links-unknown-style", "links.flm");
    let output = flm(&dir, &["-p", "text", "-O", "flmdoc.link_style=url", "links.flm"]);
    let reported = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(1), "{reported}");
    let message = "error: the setting `flmdoc.link_style` must be `command` or `path`, not `url`";
    assert_eq!(reported.matches(message).count(), 2, "{reported}");
    assert!(reported.contains("at links.flm:6:17") && reported.contains("at links.flm:6:75"), "{reported}");
}

/// Raw text that a user definition's template fills with markup is reported as the library
/// reports any such raw text, once per command, and by nothing else.
#[test]
fn raw_text_holding_markup_is_reported_at_each_command() {
    let dir = folder("raw-markup", "raw-markup.flm");
    let output = flm(&dir, &["-p", "text", "raw-markup.flm"]);
    let reported = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(1), "{reported}");
    for name in ["\\`", "\\docsignature", "\\docref"] {
        let message = format!("error: the verbatim content of {name} must be plain characters");
        assert_eq!(reported.matches(&message).count(), 1, "{message:?} in {reported}");
    }
    assert!(reported.ends_with("3 errors, 0 warnings\n"), "{reported}");
}

/// The rules stage presentation nodes, the default on every backend: the backend
/// `latex-rendered` writes them, with each role as an option, and the backend
/// `latex-recomposed`, which does not write them, refuses the plan.
#[test]
fn the_latex_rendered_backend_writes_the_constructs_and_latex_recomposed_refuses_them() {
    let dir = folder("latex", "item.flm");
    let output = flm(&dir, &["-p", "latex-rendered", "-o", "item.tex", "item.flm"]);
    assert!(output.status.success() && output.stderr.is_empty(), "{output:?}");
    let tex = std::fs::read_to_string(dir.join("item.tex")).unwrap();
    assert!(tex.contains("\\begin{flmxBlock}[role=flmdoc.item, kinds={macro}, anchor=macro-section]"), "{tex}");
    assert!(tex.contains("\\flmxLink[kind=flmdoc.page, href={flm info features/headings}]{headings}"), "{tex}");
    let output = flm(&dir, &["-p", "latex-recomposed", "-o", "recomposed.tex", "item.flm"]);
    let reported = String::from_utf8_lossy(&output.stderr).into_owned();
    assert_eq!(output.status.code(), Some(2), "{reported}");
    assert!(reported.contains("backend `latex-recomposed` cannot render the elements [flmdoc.field, flmdoc.item"), "{reported}");
}
