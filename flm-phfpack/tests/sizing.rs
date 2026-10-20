//! The sizing forms of the delimited macros and the backtick form (acceptance
//! tests 8 and 9), and the conditions a size can meet. The expected
//! replacements are what the Python package writes for the same formulas, but
//! for a star after a backtick, which the Python package writes as `*l`, and
//! which draws the delimiters as tall as the content here.

mod common;

use common::{identifiers, registry, run, text_of};
use flm_core::stdcore::backends::text;

/// The text output of one formula, which must report nothing.
fn formula(source: &str) -> String {
    let (text, diagnostics) = text_of(&registry(), &format!(r"\({source}\)"));
    assert!(diagnostics.is_empty(), "{source}: {diagnostics:?}");
    let inner = text.strip_prefix(r"\(").and_then(|t| t.strip_suffix("\\)\n"));
    inner.unwrap_or_else(|| panic!("{source}: {text}")).to_string()
}

#[test]
fn a_delimited_macro_without_a_size_writes_its_delimiters_as_they_are() {
    assert_eq!(formula(r"\abs{x}"), r"\lvert{x}\rvert");
}

#[test]
fn each_size_command_after_a_backtick_and_in_brackets_sizes_the_delimiters() {
    for size in [r"\big", r"\Big", r"\bigg", r"\Bigg"] {
        let expected = format!(r"{size}l \lvert{{x}}{size}r \rvert");
        assert_eq!(formula(&format!(r"\abs`{size}{{x}}")), expected, "after a backtick");
        assert_eq!(formula(&format!(r"\abs[{size}]{{x}}")), expected, "in brackets");
    }
}

#[test]
fn a_star_alone_in_brackets_and_after_a_backtick_fits_the_content() {
    let expected = r"\mathopen{}\left \lvert{x}\right \rvert\mathclose{}";
    assert_eq!(formula(r"\abs*{x}"), expected);
    assert_eq!(formula(r"\abs[*]{x}"), expected);
    assert_eq!(formula(r"\abs`*{x}"), expected);
}

#[test]
fn a_middle_delimiter_is_sized_with_the_others() {
    assert_eq!(
        formula(r"\braket`\Big{x}{y}"),
        r"\Bigl \langle{x}\mkern 1.5mu \mathclose{}\Big \vert\mathopen{}\mkern 1.5mu {y}\Bigr \rangle"
    );
    assert_eq!(
        formula(r"\braket*{x}{y}"),
        r"\mathopen{}\left \langle{x}\mkern 1.5mu \mathclose{}\middle \vert\mathopen{}\mkern 1.5mu {y}\right \rangle\mathclose{}"
    );
}

/// The identifiers a formula reports, and its text output.
fn reported(source: &str) -> (Vec<String>, String) {
    let run = run(&registry(), text::WORKFLOW, &format!(r"\({source}\)"));
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone());
    (identifiers(&run), out.unwrap_or_default())
}

#[test]
fn two_sizes_at_once_are_reported_at_the_use() {
    for source in [r"\abs`\Big[\big]{x}", r"\abs*[\Big]{x}", r"\abs`\Big*{x}", r"\abs*[*]{x}"] {
        let (identifiers, out) = reported(source);
        assert_eq!(identifiers, ["phfpack.phfqit.bad-size"], "{source}");
        assert!(out.contains(source), "{source}: the use stays as written: {out}");
    }
}

#[test]
fn a_command_that_is_not_a_size_is_reported_at_the_use() {
    let (identifiers, _) = reported(r"\abs`\huge{x}");
    assert_eq!(identifiers, ["phfpack.phfqit.bad-size"]);
    let (identifiers, _) = reported(r"\abs[\huge]{x}");
    assert_eq!(identifiers, ["phfpack.phfqit.bad-size"]);
    let (identifiers, _) = reported(r"`\huge(x)");
    assert_eq!(identifiers, ["phfpack.phfqit.bad-size"]);
}

#[test]
fn the_backtick_form_writes_each_pair_of_delimiters_at_each_size() {
    let pairs = [("(", ")", "(", ")"), ("[", "]", "[", "]"), ("{", "}", r"\{", r"\}"), ("<", ">", r"\langle", r"\rangle")];
    for (open, close, left, right) in pairs {
        assert_eq!(formula(&format!("`{open} a{close}")), format!("{left} a{right}"));
        assert_eq!(
            formula(&format!(r"`\Big{open} a{close}")),
            format!(r"\Bigl {left} a\Bigr {right}")
        );
        assert_eq!(
            formula(&format!("`*{open} a{close}")),
            format!(r"\mathopen{{}}\left {left} a\right {right}\mathclose{{}}")
        );
    }
}

#[test]
fn a_delimiter_written_as_a_control_word_is_kept_apart_from_the_letter_after_it() {
    assert_eq!(formula(r"`\Bigg<x>"), r"\Biggl \langle x\Biggr \rangle");
    assert_eq!(formula("`<x>"), r"\langle x\rangle");
}

#[test]
fn the_backtick_form_stands_inside_a_macros_argument_and_inside_itself() {
    assert_eq!(formula(r"\ket{`\big(a)}"), r"\lvert{\bigl (a\bigr )}\rangle");
    assert_eq!(formula(r"`\Big(`(a))"), r"\Bigl ((a)\Bigr )");
}

#[test]
fn a_backtick_followed_by_no_group_is_reported() {
    let (identifiers, _) = reported(r"` x");
    assert!(identifiers.contains(&String::from("core.arguments.missing-mandatory-argument")), "{identifiers:?}");
}
