//! The writer of a formula's TeX of the `math` feature
//! (`flm_core::stdlib::features::math::tex`), over the formulas of a
//! stand-in processor that declares its commands of math mode by name: a
//! math group's content and an equation line's content are written as their
//! source, a group as its delimiters around its content, `\ensuremath{…}` as
//! its argument's content, a text command as a hole at any depth, a comment
//! as one space, a control word kept apart from a letter that follows it from
//! elsewhere; a line's content under the newline rule, with its trailing run;
//! a line's `\\`, `\tag`, `\nonumber` and labels never written.

mod common;

use std::sync::Arc;

use flm_core::core::registry::{FlmFeature, FlmRegistry, PackageEntry};
use flm_core::lang::Flm;
use flm_core::lang::mint::BodyDef;
use flm_core::lang::structural::MATH;
use flm_core::model::doc::Fragment;
use flm_core::model::element::element_of;
use flm_core::stdlib::features::math::ENVIRONMENT;
use flm_core::stdlib::features::math::lines::equation_lines;
use flm_core::stdlib::features::math::tex::{LineTex, TexPart, group_tex, line_tex};
use flm_core::stdlib::{StdlibConfig, add_features};
use flm_core::techy::core::node::NodeRef;
use flm_core::techy::latexlike::Mode;

use common::pipeline;

/// The stand-in processor's name.
const PROCESSOR: &str = "standin";

/// The standard package and a stand-in processor declaring, in math mode
/// only, `\alpha`, `\rangle`, `\frac` with two arguments and an environment
/// `cases`.
fn registry() -> Arc<FlmRegistry> {
    let mut features: Vec<Arc<dyn FlmFeature>> =
        add_features(PackageEntry::new("std"), StdlibConfig::default()).features;
    features.push(pipeline::feature(PROCESSOR, |b| {
        for (name, arguments) in [("alpha", 0), ("rangle", 0), ("frac", 2)] {
            b.element(common::macro_def(PROCESSOR, name, arguments).modes([Mode::Math]))?;
        }
        b.element(common::env_def(PROCESSOR, "cases", BodyDef::new()).modes([Mode::Math]))?;
        Ok(())
    }));
    pipeline::registry(features)
}

/// The parse of `text` under the registry's language.
fn parse(text: &str) -> Fragment {
    common::parse(registry().lang(), text)
}

/// The parts written as one string, a hole as `<command:argument>`, the
/// argument as written.
fn written(parts: &[TexPart<'_>]) -> String {
    parts
        .iter()
        .map(|part| match part {
            TexPart::Tex(tex) => tex.clone(),
            TexPart::Hole(hole) => {
                let argument: String = hole
                    .node
                    .argument_content_nodes(0)
                    .map(|nodes| nodes.iter().map(|node| node.span_content()).collect())
                    .unwrap_or_default();
                format!("<{}:{argument}>", hole.command)
            }
        })
        .collect()
}

/// How many TeX parts and holes `parts` holds.
fn counts(parts: &[TexPart<'_>]) -> (usize, usize) {
    let tex = parts.iter().filter(|part| matches!(part, TexPart::Tex(_))).count();
    (tex, parts.len() - tex)
}

/// The math groups of a parse, in document order.
fn groups(fragment: &Fragment) -> Vec<NodeRef<'_, Flm>> {
    fragment.tree().root().descendants().filter(|node| element_of(node).name == MATH).collect()
}

/// The TeX of the one math group of a parse of `text`, as [`written`] writes
/// it, and its counts.
fn group(text: &str) -> (String, (usize, usize)) {
    let fragment = parse(text);
    let groups = groups(&fragment);
    assert_eq!(groups.len(), 1, "one math group in {text:?}");
    let parts = group_tex(&groups[0]);
    (written(&parts), counts(&parts))
}

/// The TeX of each line of the first display-math environment of a parse of
/// `text`: its parts as [`written`] writes them and its trailing run.
fn lines(text: &str) -> Vec<(String, String)> {
    let fragment = parse(text);
    let environment = common::callables(&fragment)
        .into_iter()
        .find(|node| element_of(node).name == ENVIRONMENT)
        .expect("a display-math environment");
    let lines = equation_lines(&environment);
    let mut after_line_end = false;
    lines
        .iter()
        .map(|line| {
            let tex: LineTex<'_> = line_tex(line, after_line_end);
            after_line_end = tex.ends_line();
            (written(&tex.parts), tex.trailing)
        })
        .collect()
}

#[test]
fn a_formula_without_text_commands_is_one_tex_part_written_as_its_source() {
    assert_eq!(group("\\(\\frac{a}{\\alpha} + x\\)"), (String::from("\\frac{a}{\\alpha} + x"), (1, 0)));
    assert_eq!(group("\\[ {a}_{b} \\]"), (String::from(" {a}_{b} "), (1, 0)));
    assert_eq!(
        group("\\(\\begin{cases} a & b \\end{cases}\\)"),
        (String::from("\\begin{cases} a & b \\end{cases}"), (1, 0))
    );
    assert_eq!(group("\\(\\)"), (String::new(), (0, 0)), "an empty formula has no part");
}

#[test]
fn a_text_command_is_a_hole_at_any_depth() {
    assert_eq!(group("\\(x \\text{if} y\\)"), (String::from("x <text:if> y"), (2, 1)));
    // Inside a group and inside a command's argument, the delimiters and the
    // call written around the hole.
    assert_eq!(group("\\({x \\textbf{n}}\\)"), (String::from("{x <textbf:n>}"), (2, 1)));
    assert_eq!(
        group("\\(\\frac{\\mbox{n}}{2}\\)"),
        (String::from("\\frac{<mbox:n>}{2}"), (2, 1))
    );
    assert_eq!(
        group("\\(\\begin{cases} a & \\text{if } b \\end{cases}\\)"),
        (String::from("\\begin{cases} a & <text:if > b \\end{cases}"), (2, 1))
    );
    // Two holes side by side, no TeX between them.
    assert_eq!(group("\\(\\text{a}\\text{b}\\)"), (String::from("<text:a><text:b>"), (0, 2)));
    let fragment = parse("\\(\\textit{x}\\)");
    let parts = group_tex(&groups(&fragment)[0]);
    let TexPart::Hole(hole) = &parts[0] else { panic!("a hole") };
    assert_eq!(hole.command, "textit");
    assert_eq!(hole.node.name(), Some("textit"), "the hole carries the text command's node");
}

#[test]
fn ensuremath_is_written_as_its_arguments_content() {
    assert_eq!(group("\\(x + \\ensuremath{y}\\)"), (String::from("x + y"), (1, 0)));
    assert_eq!(group("\\(\\ensuremath{\\text{a}}\\)"), (String::from("<text:a>"), (0, 1)));
}

#[test]
fn a_control_word_is_kept_apart_from_a_letter_that_follows_it_from_elsewhere() {
    assert_eq!(group("\\(\\alpha\\ensuremath{b}\\)"), (String::from("\\alpha b"), (1, 0)));
    assert_eq!(group("\\(\\rangle\\ensuremath{A}\\)"), (String::from("\\rangle A"), (1, 0)));
    assert_eq!(group("\\(\\alpha\\ensuremath{{b}}\\)"), (String::from("\\alpha{b}"), (1, 0)));
}

#[test]
fn a_comment_is_one_space() {
    // A comment is written `%%` and takes in its line end.
    assert_eq!(group("\\[\\alpha%%c\nb\\]"), (String::from("\\alpha b"), (1, 0)));
    // Inside a group, too.
    assert_eq!(group("\\[{a%%c\nb}\\]"), (String::from("{a b}"), (1, 0)));
    // In a line, a comment does not end the line.
    assert_eq!(
        lines("\\begin{equation*}\n  a %%c\n  b\n\\end{equation*}"),
        [(String::from("\n  a  b"), String::from("\n"))]
    );
}

#[test]
fn a_line_spanning_newlines_keeps_them_and_its_trailing_run_is_taken_off() {
    let text = "\\begin{align}\n  a\n  &= b \\\\\n  c\n\\end{align}";
    assert_eq!(
        lines(text),
        [
            (String::from("\n  a\n  &= b "), String::new()),
            (String::from("\n  c"), String::from("\n")),
        ]
    );
}

#[test]
fn a_line_ending_on_its_own_line_moves_its_run_and_the_next_line_drops_its_own() {
    // `\label` and `\nonumber` on lines of their own: the run that ended the
    // first line's content is its trailing run, and the next line's content
    // begins without its run. The whitespace after `\nonumber` belongs to
    // the call, so the last line's run is the one before it.
    let text = "\\begin{align}\n  a \\label{eq:a}\n  \\\\\n  b\n  \\nonumber\n\\end{align}";
    assert_eq!(
        lines(text),
        [(String::from("\n  a"), String::from("\n  ")), (String::from("b"), String::from("\n  "))]
    );
}

#[test]
fn a_lines_tag_and_line_break_are_not_written_and_a_hole_is_not_a_line_end() {
    let text = "\\begin{gather}\n  x \\text{if}\n  y \\tag{T} \\\\[1ex]\n  z\n\\end{gather}";
    assert_eq!(
        lines(text),
        [
            // The spaces before and after `\\tag{T}` are both the line's.
            (String::from("\n  x <text:if>\n  y  "), String::new()),
            (String::from("\n  z"), String::from("\n")),
        ]
    );
}

#[test]
fn a_line_written_after_a_line_end_drops_the_run_that_begins_it() {
    let fragment = parse("\\begin{equation}\n  \\alpha\n\\end{equation}");
    let environment = common::callables(&fragment)
        .into_iter()
        .find(|node| element_of(node).name == ENVIRONMENT)
        .expect("a display-math environment");
    let lines = equation_lines(&environment);
    let first = line_tex(&lines[0], false);
    assert_eq!((written(&first.parts), first.trailing.as_str()), (String::from("\n  \\alpha"), "\n"));
    assert!(first.ends_line());
    let after = line_tex(&lines[0], true);
    assert_eq!(written(&after.parts), "\\alpha");
}

