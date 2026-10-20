//! The equation lines of the `math` feature
//! (`flm_core::stdcore::features::math::lines`): the four commands an author
//! writes inside a display-math environment's body — `\\`, `\label`, `\tag`,
//! `\nonumber` — exist inside those bodies only, and `equation_lines` splits a
//! body into lines by this library's rules: a `\\` ends a line, a label, tag
//! or no-number mark belongs to the line it stands in, a trailing `\\`
//! followed only by whitespace and comments opens no further line and is not
//! recorded (that whitespace and those comments are the last line's content),
//! and a nested environment's `\\` splits nothing. The parse reports a label
//! or tag on a line without a number (`flm.math.label-without-number`), the
//! check stage of the `math-passthrough` feature an empty line
//! (`flm.math-passthrough.empty-line`), and the tree is kept as written. A
//! math-mode `\label` is a field node read by the `refs` helpers, and the text
//! output still copies the body byte for byte.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::dump::diag::expect;
use flm_core::dump::json::to_json;
use flm_core::techy::serialize::SerialValue;
use flm_core::model::doc::Fragment;
use flm_core::lang::structural;
use flm_core::model::element::element_of;
use flm_core::stdcore::backends::text;
use flm_core::lang::mint::field_name;
use flm_core::stdcore::features::math::lines::{
    EquationLine, PACKAGE, equation_lines, line_break, tag_of,
};
use flm_core::stdcore::features::math::{self, ENVIRONMENT};
use flm_core::math_passthrough::OPAQUE;
use flm_core::model::aspects::Label;
use flm_core::stdcore::features::refs::{
    self, ANCHORS, LABELS, REF_TARGETS, label_use,
};
use flm_core::stdcore::features::refpoints::REFPOINTS;
use flm_core::stdcore::{CHECK_WORKFLOW, StdcoreConfig};
use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};
use flm_core::techy::core::node::NodeRef;
use flm_core::lang::Flm;
use serde_json::json;

use common::lowering::text as text_of;
use common::stdcore::{identifiers, projection, registry, run_workflow};

/// The standard package, whose `math` and `refs` features are the ones under
/// test.
fn std() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::default())
}

/// The standard package with every heading kind numbered.
fn numbered_headings() -> Arc<FlmRegistry> {
    registry(
        StdcoreConfig::default()
            .headings(HeadingsConfig::new().numbering(HeadingNumbering::AllNumbered)),
    )
}

/// The diagnostics a run of the `check` workflow over `text` reports: the
/// parse's, and the checks' (the empty lines are reported by the check stage
/// of the `math-passthrough` feature, not by the parse).
fn checked(registry: &Arc<FlmRegistry>, text: &str) -> flm_core::core::run::Run {
    run_workflow(registry, CHECK_WORKFLOW, &[text], &Overrides::none())
}

/// The first display-math environment node of a parse of `text`.
fn environment<'f>(fragment: &'f Fragment) -> NodeRef<'f, Flm> {
    common::callables(fragment)
        .into_iter()
        .find(|node| element_of(node).name == ENVIRONMENT)
        .expect("a display-math environment")
}

/// The element names of every callable node of a parse, in document order, a
/// field node as `field:<name>` — the four line commands are fields, so their
/// nodes all carry one element and are told apart by the field's name.
fn elements(fragment: &Fragment) -> Vec<String> {
    common::callables(fragment)
        .iter()
        .map(|node| match field_name(node) {
            Some(name) => format!("field:{name}"),
            None => element_of(node).name.to_string(),
        })
        .collect()
}

/// `\\` as [`elements`] names it.
const LINEBREAK: &str = "field:\\";
/// `\tag` as [`elements`] names it.
const TAG: &str = "field:tag";
/// `\nonumber` as [`elements`] names it.
const NONUMBER: &str = "field:nonumber";
/// `\label` as [`elements`] names it.
const LABEL: &str = "field:label";

/// The source text a line's content nodes were written as, concatenated.
fn content_text(line: &EquationLine<'_>) -> String {
    line.content.iter().map(|node| node.span().content()).collect()
}

/// The label texts written on a line.
fn label_texts(line: &EquationLine<'_>) -> Vec<String> {
    line.labels.iter().map(|us| us.text.clone()).collect()
}

/// Whether every content node of a line is blank: a comment, or whitespace.
fn is_blank(line: &EquationLine<'_>) -> bool {
    line.content.iter().all(|node| node.is_comment() || node.chars().is_some_and(|c| c.trim().is_empty()))
}

/// The spacing of the line's `\\`: `None` when the line has no `\\`,
/// `Some(None)` for a bare `\\`, `Some(Some(text))` for `\\[text]`.
fn spacing(line: &EquationLine<'_>) -> Option<Option<String>> {
    line.linebreak.as_ref().map(|brk| brk.spacing.clone())
}

// --- the declarations -----------------------------------------------------------------

#[test]
fn the_four_commands_parse_inside_a_display_environment_body() {
    let registry = std();
    let text = "\\begin{align}\n  a &= b \\label{eq:a} \\tag{A} \\\\\n  c &= d \\nonumber \\\\\n  \
                e &= f \\tag*{E}\n\\end{align}";
    let fragment = common::parse(registry.lang(), text);
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    assert_eq!(elements(&fragment), [ENVIRONMENT, LABEL, TAG, LINEBREAK, NONUMBER, LINEBREAK, TAG]);
}

#[test]
fn the_commands_are_scoped_to_the_environment_bodies_and_not_to_all_of_mathematics() {
    let registry = std();
    // Inline and bare display mathematics: the same four names are unknown
    // commands, accepted silently as opaque mathematics.
    let inline = common::parse(registry.lang(), "\\(a \\\\ b \\label{eq:x} \\tag{T} \\nonumber\\)");
    assert!(inline.diags().is_empty(), "{:?}", inline.diags());
    assert_eq!(elements(&inline), [OPAQUE, OPAQUE, OPAQUE, OPAQUE]);
    let display = common::parse(registry.lang(), "\\[ a \\\\ b \\]");
    assert!(display.diags().is_empty(), "{:?}", display.diags());
    assert_eq!(elements(&display), [OPAQUE]);
    // Text mode stays strict: `\\` and `\tag` are unknown macros and errors.
    let text = common::parse(registry.lang(), "a \\\\ b \\tag{T}");
    assert!(text.diags().has_errors());
    assert_eq!(elements(&text), [structural::UNKNOWN_MACRO, structural::UNKNOWN_MACRO]);
    // The scoped package is registered under its name.
    assert_eq!(PACKAGE, "flm.math.lines");
}

#[test]
fn notag_inside_a_body_is_opaque_content_and_not_a_no_number_mark() {
    // `\notag` is not declared: inside a body it is an unknown command in
    // mathematics, kept as opaque content of its line.
    let registry = std();
    let fragment = common::parse(registry.lang(), "\\begin{align} a \\notag \\\\ b \\end{align}");
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    assert_eq!(elements(&fragment), [ENVIRONMENT, OPAQUE, LINEBREAK]);
    assert!(!elements(&fragment).iter().any(|name| name == NONUMBER));
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 2);
    assert!(!lines[0].nonumber);
    assert!(content_text(&lines[0]).contains("\\notag"));
}

#[test]
fn the_six_environments_all_see_the_commands() {
    let registry = std();
    for (name, _) in flm_core::stdcore::features::math::ENVIRONMENTS {
        let text = format!("\\begin{{{name}}} a \\\\ b \\end{{{name}}}");
        let fragment = common::parse(registry.lang(), &text);
        assert!(fragment.diags().is_empty(), "{name}: {:?}", fragment.diags());
        assert_eq!(elements(&fragment), [ENVIRONMENT, LINEBREAK], "{name}");
        assert_eq!(equation_lines(&environment(&fragment)).len(), 2, "{name}");
    }
}

#[test]
fn a_math_mode_label_is_a_label_node_the_refs_helpers_read() {
    let registry = std();
    let fragment = common::parse(registry.lang(), "\\begin{equation} x \\label{eq:x} \\end{equation}");
    let label = common::callables(&fragment)
        .into_iter()
        .find(|node| field_name(node).is_some_and(|name| name == "label"))
        .expect("the label node");
    assert_eq!(refs::field_text(&label).as_deref(), Some("eq:x"));
    let us = label_use(&label).expect("a label use");
    assert_eq!(us.text, "eq:x");
    let parsed = us.label.expect("well-formed");
    assert_eq!(parsed.ref_type, "eq");
    assert_eq!(parsed.ref_label, "x");
    assert_eq!(us.span.content(), "\\label{eq:x}");
}

// --- splitting a body into lines ----------------------------------------------------------

#[test]
fn a_label_on_a_line_of_its_own_belongs_to_the_line_the_following_break_ends() {
    // The Python implementation's `test_simple_math_2`: each label, written on
    // a line of its own, belongs to the equation line before it.
    let registry = std();
    let text = "\\begin{align}\n   x &= 0;\n   \\label{eq:one}\\\\\n   z &= 1;\n   \\label{eq:two}\n\\end{align}";
    let fragment = common::parse(registry.lang(), text);
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 2);
    assert_eq!(content_text(&lines[0]), "\n   x &= 0;\n   ");
    assert_eq!(label_texts(&lines[0]), ["eq:one"]);
    assert_eq!(spacing(&lines[0]), Some(None));
    assert_eq!(content_text(&lines[1]), "\n   z &= 1;\n   \n");
    assert_eq!(label_texts(&lines[1]), ["eq:two"]);
    assert_eq!(spacing(&lines[1]), None);
    for line in &lines {
        assert!(line.tag.is_none());
        assert!(!line.nonumber);
    }
}

#[test]
fn a_leading_label_belongs_to_the_first_line() {
    // The Python implementation's `test_simple_math_3`.
    let registry = std();
    let text = "\\begin{align}\n   \\label{eq:MyEquation}\n   x &= 0.\n\\end{align}";
    let fragment = common::parse(registry.lang(), text);
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 1);
    assert_eq!(label_texts(&lines[0]), ["eq:MyEquation"]);
    assert_eq!(content_text(&lines[0]), "\n   \n   x &= 0.\n");
    assert!(lines[0].linebreak.is_none());
}

#[test]
fn the_optional_argument_of_a_line_break_is_kept_as_written() {
    // The Python implementation's `test_simple_math`.
    let registry = std();
    let text = "\\begin{align}\n  x &= 0; \\\\[1ex]\n  z &= 1.\n\\end{align}";
    let fragment = common::parse(registry.lang(), text);
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 2);
    assert_eq!(content_text(&lines[0]), "\n  x &= 0; ");
    assert_eq!(spacing(&lines[0]), Some(Some("1ex".to_string())));
    assert_eq!(lines[0].linebreak.as_ref().unwrap().node.span().content(), "\\\\[1ex]");
    assert_eq!(content_text(&lines[1]), "\n  z &= 1.\n");
    assert_eq!(spacing(&lines[1]), None);
}

#[test]
fn the_optional_argument_must_follow_the_break_immediately() {
    let registry = std();
    let text = "\\begin{align} a \\\\ [b] \\end{align}";
    let fragment = common::parse(registry.lang(), text);
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 2);
    assert_eq!(spacing(&lines[0]), Some(None));
    assert_eq!(content_text(&lines[1]), " [b] ");
}

#[test]
fn two_consecutive_breaks_make_an_empty_line() {
    // The Python implementation's `test_gather_environment`: `\\\\` is two
    // line breaks, the second ending an empty line.
    let registry = std();
    let text = "\\begin{gather}\n  a = b \\\\\\\\\n  c = d\n\\end{gather}";
    let fragment = common::parse(registry.lang(), text);
    // The empty middle line is an error of the check; the lines are still read.
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    let run = checked(&registry, text);
    expect(run.diagnostics().iter(), "flm.math-passthrough.empty-line")
        .data(json!({ "environment": "gather", "line": 1 }))
        .once();
    assert_eq!(run.diagnostics().iter().count(), 1);
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 3);
    assert_eq!(content_text(&lines[0]), "\n  a = b ");
    assert_eq!(content_text(&lines[1]), "");
    assert!(lines[1].linebreak.is_some());
    assert_eq!(content_text(&lines[2]), "\n  c = d\n");
    assert!(lines[2].linebreak.is_none());
}

#[test]
fn a_trailing_break_followed_only_by_blank_content_opens_no_further_line_and_is_not_recorded() {
    // The whitespace and comments after the final `\\` stay in the last
    // line's content as written, so the content is the whole body with the
    // `\\` (and its spacing) cut out.
    let registry = std();
    for (text, content) in [
        ("\\begin{align} a \\\\ \\end{align}", " a  "),
        ("\\begin{align} a \\\\\n  %% a comment\n\\end{align}", " a \n  %% a comment\n"),
        ("\\begin{align} a \\\\\n\n\\end{align}", " a \n\n"),
        ("\\begin{align} a \\\\\n\\end{align}", " a \n"),
        ("\\begin{align} a \\\\[1ex]\n\\end{align}", " a \n"),
    ] {
        let fragment = common::parse(registry.lang(), text);
        assert!(fragment.diags().is_empty(), "{text}: {:?}", fragment.diags());
        let names: Vec<String> =
            elements(&fragment).into_iter().filter(|name| name != structural::PARBREAK).collect();
        assert_eq!(names, [ENVIRONMENT, LINEBREAK], "{text}");
        let lines = equation_lines(&environment(&fragment));
        assert_eq!(lines.len(), 1, "{text}");
        assert!(lines[0].linebreak.is_none(), "{text}");
        assert_eq!(content_text(&lines[0]), content, "{text}");
    }
    // The comment after the final `\\` is the last content node of the last line.
    let fragment = common::parse(registry.lang(), "\\begin{align} a \\\\\n  %% a comment\n\\end{align}");
    let lines = equation_lines(&environment(&fragment));
    let last = lines[0].content.last().expect("content");
    assert!(last.is_comment());
    assert_eq!(last.span().content(), "%% a comment\n");
}

#[test]
fn a_line_command_after_the_last_break_opens_an_empty_line_which_is_an_error() {
    // A `\label`, `\tag` or `\nonumber` written after the last `\\` opens a
    // further line, which carries it and holds no mathematics: the line is
    // read, and the parse reports it as empty.
    let registry = std();
    for (text, at) in [
        ("\\begin{align} a \\\\ \\label{eq:z} \\end{align}", "\\label{eq:z}"),
        ("\\begin{align} a \\\\\n  \\tag{Z}\n\\end{align}", "\\tag{Z}"),
        ("\\begin{align} a \\\\ %% c\n \\nonumber\n\\end{align}", "\\nonumber\n"),
        ("\\begin{align} a \\\\ \\label{eq:z} \\nonumber \\tag{Z} \\end{align}", "\\label{eq:z} \\nonumber \\tag{Z}"),
    ] {
        let fragment = common::parse(registry.lang(), text);
        expect(checked(&registry, text).diagnostics().iter(), "flm.math-passthrough.empty-line")
            .at(at)
            .data(json!({ "environment": "align", "line": 1 }))
            .once();
        let lines = equation_lines(&environment(&fragment));
        assert_eq!(lines.len(), 2, "{text}");
        assert!(lines[0].linebreak.is_some(), "{text}");
        assert_eq!(content_text(&lines[0]), " a ", "{text}");
        assert!(lines[1].linebreak.is_none(), "{text}");
        assert!(is_blank(&lines[1]), "{text}");
    }
    let fragment = common::parse(
        registry.lang(),
        "\\begin{align} a \\\\ \\label{eq:z} \\nonumber \\tag{Z} \\end{align}",
    );
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(label_texts(&lines[1]), ["eq:z"]);
    assert!(lines[1].nonumber);
    assert_eq!(lines[1].tag.as_ref().map(|tag| tag.written()), Some(String::from("Z")));
}

#[test]
fn content_or_a_further_break_after_the_last_break_opens_the_line_with_its_commands() {
    // Mathematics after the last `\\` opens the next line, and a command
    // written between the two belongs to it; so does a further `\\`, whose
    // own line is then the empty last line.
    let registry = std();
    let fragment = common::parse(registry.lang(), "\\begin{align} a \\\\ \\label{eq:z} b \\end{align}");
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 2);
    assert_eq!(label_texts(&lines[1]), ["eq:z"]);
    assert_eq!(content_text(&lines[1]), "  b ");
    assert!(lines[1].linebreak.is_none());
    let text = "\\begin{align} a \\\\ \\label{eq:z} \\\\ \\end{align}";
    let fragment = common::parse(registry.lang(), text);
    expect(checked(&registry, text).diagnostics().iter(), "flm.math-passthrough.empty-line").at("\\label{eq:z}").once();
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 2);
    assert_eq!(label_texts(&lines[1]), ["eq:z"]);
    assert!(is_blank(&lines[1]));
    assert!(lines[1].linebreak.is_none(), "the final break is not recorded");
}

#[test]
fn a_body_without_a_break_is_one_line_and_an_empty_body_is_one_empty_line() {
    let registry = std();
    let fragment = common::parse(registry.lang(), "\\begin{equation} E = mc^2 \\end{equation}");
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 1);
    assert_eq!(content_text(&lines[0]), " E = mc^2 ");
    assert!(lines[0].linebreak.is_none());
    let fragment = common::parse(registry.lang(), "\\begin{equation}\\end{equation}");
    expect(checked(&registry, "\\begin{equation}\\end{equation}").diagnostics().iter(), "flm.math-passthrough.empty-line")
        .data(json!({ "environment": "equation", "line": 0 }))
        .once();
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 1);
    assert!(lines[0].content.is_empty());
    assert!(lines[0].linebreak.is_none());
}

#[test]
fn tags_and_no_number_marks_are_recorded_on_their_line() {
    let registry = std();
    let text = "\\begin{align}\n  a &= b \\tag{A} \\\\\n  c &= d \\tag*{ C }\\\\\n  \
                e &= f \\nonumber \\\\\n  g &= h \\nonumber\n\\end{align}";
    let fragment = common::parse(registry.lang(), text);
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 4);
    let tag = lines[0].tag.as_ref().expect("a tag");
    assert_eq!(tag.written(), "A");
    assert!(!tag.star);
    // An unstarred tag is shown in parentheses; the backend writes them.
    assert!(tag.parenthesized());
    let tag = lines[1].tag.as_ref().expect("a starred tag");
    assert_eq!(tag.written(), " C ");
    assert!(tag.star);
    assert!(!tag.parenthesized());
    // The tag's text is inline content, not characters: it is a run of nodes.
    assert_eq!(lines[0].tag.as_ref().unwrap().content.len(), 1);
    assert!(!lines[0].nonumber && !lines[1].nonumber);
    assert!(lines[2].nonumber && lines[3].nonumber);
    assert!(lines[2].tag.is_none() && lines[3].tag.is_none());
    // The commands are taken out of the content. The newline after `\nonumber`
    // goes with it: the whitespace immediately after a command's name is part
    // of the invocation, not content.
    assert_eq!(content_text(&lines[0]), "\n  a &= b  ");
    assert_eq!(content_text(&lines[3]), "\n  g &= h ");
}

#[test]
fn the_last_tag_on_a_line_wins_and_labels_accumulate() {
    let registry = std();
    let text = "\\begin{equation} x \\tag{A} \\tag{B} \\label{eq:a} \\label{eq:b} \\end{equation}";
    let fragment = common::parse(registry.lang(), text);
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].tag.as_ref().unwrap().written(), "B");
    assert_eq!(label_texts(&lines[0]), ["eq:a", "eq:b"]);
}

#[test]
fn a_break_inside_a_nested_environment_splits_no_outer_line() {
    let registry = std();
    let text = "\\begin{align}\n  f &= \\begin{cases} 1 \\\\ 2 \\end{cases} \\\\\n  g &= 0\n\\end{align}";
    let fragment = common::parse(registry.lang(), text);
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 2);
    assert_eq!(content_text(&lines[0]), "\n  f &= \\begin{cases} 1 \\\\ 2 \\end{cases} ");
    let nested = lines[0].content.iter().find(|node| node.is_callable()).expect("the cases node");
    assert_eq!(element_of(nested).name, OPAQUE);
}

#[test]
fn the_node_readers_answer_none_for_other_nodes() {
    let registry = std();
    let fragment = common::parse(registry.lang(), "\\begin{align} a \\\\ b \\tag{T} \\end{align}");
    let callables = common::callables(&fragment);
    let env = &callables[0];
    assert!(line_break(env).is_none());
    assert!(tag_of(env).is_none());
    assert!(label_use(env).is_none());
    assert!(equation_lines(&callables[1]).is_empty(), "a `\\\\` node has no body");
    assert!(line_break(&callables[1]).is_some());
    assert!(tag_of(&callables[2]).is_some());
}

// --- the two errors: the parse's and the check's -------------------------------------------------------------------------

#[test]
fn every_empty_segment_shape_is_reported_once_with_its_index() {
    // A segment before the first `\\`, between two, or past the final `\\`
    // that holds nothing, or only line commands, is an empty line.
    let registry = std();
    for (text, line, at) in [
        // a leading `\\`: reported at the break that ends the empty line
        ("\\begin{align}\\\\ a \\end{align}", 0, "\\\\"),
        // `\\\\` in the middle: at the second break
        ("\\begin{align} a \\\\\\\\ b \\end{align}", 1, "\\\\"),
        // a command past the final `\\`: at the command
        ("\\begin{align} a \\\\ \\label{eq:z} \\end{align}", 1, "\\label{eq:z}"),
        // a body of only a label: at the label
        ("\\begin{equation} \\label{eq:x} \\end{equation}", 0, "\\label{eq:x}"),
        // only line commands between two breaks: at the commands
        ("\\begin{align} a \\\\ \\label{eq:x} \\tag{X} \\\\ b \\end{align}", 1, "\\label{eq:x} \\tag{X}"),
        // whitespace and a comment are not content either: at the break
        ("\\begin{align} a \\\\\n  %% c\n  \\\\ b \\end{align}", 1, "\\\\"),
        // an entirely empty body: at the body
        ("\\begin{gather}\\end{gather}", 0, ""),
    ] {
        let fragment = common::parse(registry.lang(), text);
        let environment_name = if text.contains("{align}") {
            "align"
        } else if text.contains("{gather}") {
            "gather"
        } else {
            "equation"
        };
        assert!(fragment.diags().is_empty(), "{text}: {:?}", fragment.diags());
        let run = checked(&registry, text);
        assert_eq!(run.diagnostics().iter().count(), 1, "{text}: {:?}", run.diagnostics());
        expect(run.diagnostics().iter(), "flm.math-passthrough.empty-line")
            .at(at)
            .data(json!({ "environment": environment_name, "line": line }))
            .once();
    }
}

#[test]
fn a_label_or_tag_on_a_nonumber_line_is_reported_per_command() {
    let registry = std();
    let text = "\\begin{align} a \\label{eq:a} \\tag{A} \\nonumber \\\\ b \\label{eq:b} \\\\ c \\nonumber \\end{align}";
    let fragment = common::parse(registry.lang(), text);
    assert_eq!(fragment.diags().iter().count(), 2, "{:?}", fragment.diags());
    expect(fragment.diags().iter(), "flm.math.label-without-number")
        .at("\\label{eq:a}")
        .data(json!({ "environment": "align", "line": 0, "command": "label" }))
        .once();
    expect(fragment.diags().iter(), "flm.math.label-without-number")
        .at("\\tag{A}")
        .data(json!({ "environment": "align", "line": 0, "command": "tag" }))
        .once();
    // The tree and the lines keep what was written.
    let lines = equation_lines(&environment(&fragment));
    assert_eq!(lines.len(), 3);
    assert_eq!(label_texts(&lines[0]), ["eq:a"]);
    assert!(lines[0].nonumber && lines[0].tag.is_some());
    assert_eq!(label_texts(&lines[1]), ["eq:b"]);
    assert!(lines[2].nonumber);
}

#[test]
fn a_label_or_tag_anywhere_in_a_starred_environment_is_reported() {
    let registry = std();
    for name in ["align*", "gather*", "equation*"] {
        let text = format!("\\begin{{{name}}} a \\label{{eq:a}} \\\\ b \\tag*{{B}} \\end{{{name}}}");
        let fragment = common::parse(registry.lang(), &text);
        assert_eq!(fragment.diags().iter().count(), 2, "{name}: {:?}", fragment.diags());
        expect(fragment.diags().iter(), "flm.math.label-without-number")
            .data(json!({ "environment": name, "line": 0, "command": "label" }))
            .once();
        expect(fragment.diags().iter(), "flm.math.label-without-number")
            .data(json!({ "environment": name, "line": 1, "command": "tag" }))
            .once();
        assert_eq!(equation_lines(&environment(&fragment)).len(), 2, "{name}");
    }
    // The unstarred forms accept the same body.
    for name in ["align", "gather", "equation"] {
        let text = format!("\\begin{{{name}}} a \\label{{eq:a}} \\\\ b \\tag*{{B}} \\end{{{name}}}");
        let fragment = common::parse(registry.lang(), &text);
        assert!(fragment.diags().is_empty(), "{name}: {:?}", fragment.diags());
    }
}

#[test]
fn an_empty_line_in_a_starred_environment_holding_only_a_label_reports_both_errors() {
    let registry = std();
    let text = "\\begin{align*} a \\\\ \\label{eq:z} \\end{align*}";
    let run = checked(&registry, text);
    assert_eq!(run.diagnostics().iter().count(), 2, "{:?}", run.diagnostics());
    expect(run.diagnostics().iter(), "flm.math-passthrough.empty-line").at("\\label{eq:z}").once();
    expect(run.diagnostics().iter(), "flm.math.label-without-number").at("\\label{eq:z}").once();
}

#[test]
fn a_well_formed_body_reports_nothing() {
    let registry = std();
    for text in [
        LABELED,
        "\\begin{align}\n  a &= b \\label{eq:a} \\\\\n  c &= d \\nonumber \\\\[2pt]\n  %% a comment line\n  \
         e &= f \\tag*{E}\n\\end{align}",
        "\\begin{align*}\n  a &= b \\nonumber \\\\\n  c &= d \\\\\n\\end{align*}",
        "\\begin{equation} {} \\end{equation}",
        "\\begin{gather} \\begin{cases} 1 \\\\ \\\\ 2 \\end{cases} \\end{gather}",
    ] {
        let fragment = common::parse(registry.lang(), text);
        assert!(fragment.diags().is_empty(), "{text}: {:?}", fragment.diags());
    }
}

#[test]
fn the_errors_are_parse_diagnostics_the_check_workflow_reports() {
    // The blank line inside the body is a paragraph-break node, which is
    // whitespace to the lines: the last line holds only the label.
    let document = "\\begin{align*}\n  a &= b \\\\\n\n  \\label{eq:a}\n\\end{align*}\n";
    let run = run_workflow(&std(), CHECK_WORKFLOW, &[document], &Overrides::none());
    assert_eq!(
        identifiers(&run),
        ["flm.math.label-without-number", "flm.math-passthrough.empty-line"],
        "{}",
        run.diagnostics().render_all()
    );
}

// --- what the rest of the pipeline sees -----------------------------------------------------

/// A document with labels inside `align`, on their own lines and inline.
const LABELED: &str = "\\section{Math}\\label{sec:math}\n\n\\begin{align}\n  a &= b\n  \\label{eq:a}\\\\\n  \
                       c &= d \\label{eq:c} \\tag{C}\n\\end{align}\n\nSee \\ref{sec:math}.\n";

#[test]
fn the_check_workflow_reports_nothing_for_labels_inside_an_environment() {
    let run = run_workflow(&std(), CHECK_WORKFLOW, &[LABELED], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
}

#[test]
fn every_equation_line_is_a_reference_point_carrying_its_own_labels() {
    // The environment declares one point per line, the point's index being the
    // line's index, and each point carries the labels written on that line —
    // which is what pins a label to its own line. The heading's point is the
    // fourth.
    let run = run_workflow(&std(), text::WORKFLOW, &[LABELED], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let labels = run.table(&LABELS).expect("declared").expect("written");
    let kinds: Vec<String> = labels.by_label.keys().map(|label| label.ref_type.clone()).collect();
    assert_eq!(kinds, ["eq", "eq", "sec"]);
    let points = run.table(&REFPOINTS).expect("declared").expect("written");
    assert_eq!(points.len(), 3, "the heading and the two equation lines");
    let lines: Vec<(u32, Vec<String>)> = points
        .records()
        .iter()
        .filter(|record| record.point.kind.as_str() == "equation")
        .map(|record| {
            (record.id.index, record.point.labels.iter().map(|label| label.to_string()).collect())
        })
        .collect();
    assert_eq!(
        lines,
        [(0, vec![String::from("eq:a")]), (1, vec![String::from("eq:c")])]
    );
}

/// The text backend prints a formula as the LaTeX it is, line by line: the
/// mathematics as written, each line's tag — the number this document gave the
/// first line, the author's own tag on the second — and no `\label`, which is
/// the line's data and not part of the formula.
#[test]
fn the_text_output_writes_each_line_with_its_tag_and_without_its_labels() {
    let run = run_workflow(&std(), text::WORKFLOW, &[LABELED], &Overrides::none());
    let text = text_of(&run);
    assert!(
        text.contains(
            "\\begin{align}\n    a &= b\\tag*{(1)}\\\\\n    c &= d\\tag*{(C)}\n  \
             \\end{align}"
        ),
        "{text}"
    );
}

#[test]
fn a_malformed_label_inside_an_environment_is_reported_like_any_other() {
    // The environment reads its lines' labels and parses each as it reads it,
    // so a text that is not spelled `type:label` is reported at the field,
    // exactly as a heading's own reader reports one.
    let document = "\\begin{equation} x \\label{nolabel} \\end{equation}\n";
    let run = run_workflow(&std(), CHECK_WORKFLOW, &[document], &Overrides::none());
    assert_eq!(identifiers(&run), ["flm.refs.bad-label"]);
    expect(run.diagnostics().iter(), "flm.refs.bad-label").at("\\label{nolabel}").once();
}

// --- the reference points of the lines -------------------------------------------------------

/// Three aligned lines, the third one labeled, and a reference to it.
const THIRD_LINE_LABELED: &str = "\\begin{align}\n  a &= b \\\\\n  c &= d \\\\\n  \
                                  e &= f \\label{eq:third}\n\\end{align}\n\nSee \\ref{eq:third}.\n";

#[test]
fn a_reference_to_an_equation_line_resolves_to_that_line() {
    // The label is pinned to the line it was written on: the point the
    // reference resolves to is the environment's third, index 2. What the
    // reference shows is the line's number, which the numbering stage
    // computed for it — the third numbered line of the document.
    let run = run_workflow(&std(), text::WORKFLOW, &[THIRD_LINE_LABELED], &Overrides::none());
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let targets = run.table(&REF_TARGETS).expect("declared").expect("written");
    let target = targets.by_label.get(&Label::new("eq", "third")).expect("a target");
    assert_eq!(target.point.index, 2, "the third line of the environment");
    assert_eq!(target.href, "#eq-third");
    let number = target.display.number.as_ref().expect("a number source");
    assert_eq!(number.counter.as_str(), "flm.math.equation");
    assert_eq!(number.advance, [0]);
    assert!(target.display.content.is_none(), "an equation line states no content");
    // The text backend writes a link inside the document as its content
    // alone, so the reference prints the number the style writes: the word,
    // a non-breaking space, then the value between parentheses.
    assert!(text_of(&run).contains("See Eq.\u{a0}(3)."), "{}", text_of(&run));
}

/// A document with all three shapes a reference point takes, under a registry
/// numbering its headings ([`numbered_headings`]): a heading, which states
/// both a number source and content; a line of `equation`, which states a
/// number source and no content; and a line of `equation*`, which states
/// neither.
const THREE_SHAPES: &str = "\\section{S}\n\n\\begin{equation}\n  x = y\n\\end{equation}\n\n                            \\begin{equation*}\n  z = w\n\\end{equation*}\n";

#[test]
fn a_point_states_a_number_source_content_or_neither() {
    // What a reference shows is the point's number where a document numbered
    // it, else its content, else the label as written; a point says which of
    // the two it has, and may have neither.
    let run =
        run_workflow(&numbered_headings(), text::WORKFLOW, &[THREE_SHAPES], &Overrides::none());
    let points = run.table(&REFPOINTS).expect("declared").expect("written");
    let shapes: Vec<(String, bool, bool)> = points
        .records()
        .iter()
        .map(|record| {
            (
                record.point.kind.to_string(),
                record.point.number.is_some(),
                record.point.content.is_some(),
            )
        })
        .collect();
    assert_eq!(
        shapes,
        [
            ("section".to_string(), true, true),
            ("equation".to_string(), true, false),
            ("equation".to_string(), false, false),
        ]
    );
    let numbered = points.records()[1].point.number.as_ref().expect("a number source");
    assert_eq!(numbered.counter.as_str(), math::COUNTER);
    assert_eq!(numbered.style.as_str(), math::COUNTER_STYLE);
    assert_eq!(numbered.advance, [0], "one level: the number itself");
}

#[test]
fn the_projection_shows_each_point_only_what_it_states() {
    // The dump projects a point's number source and its content under one key
    // each, and leaves out the key a point does not state — so a point that
    // shows nothing of its own projects as an empty map.
    let run =
        run_workflow(&numbered_headings(), text::WORKFLOW, &[THREE_SHAPES], &Overrides::none());
    let SerialValue::Map(table) = projection(&run, REFPOINTS) else { panic!("a map") };
    let (_, points) =
        table.into_iter().find(|(key, _)| key == "flm.refpoints").expect("the table");
    let SerialValue::Map(points) = points else { panic!("a map") };
    let displays: Vec<SerialValue> = points
        .into_iter()
        .map(|(_, point)| {
            let SerialValue::Map(entries) = point else { panic!("a map") };
            entries.into_iter().find(|(key, _)| key == "display").expect("the display").1
        })
        .collect();
    let written: Vec<String> =
        displays.iter().map(|value| to_json(value).expect("a JSON value")).collect();
    assert_eq!(
        written,
        [
            r#"{"number":{"counter":"flm.headings.section","style":"flm.headings.style.section","advance":[0]},"content":"argument title"}"#,
            r#"{"number":{"counter":"flm.math.equation","style":"flm.math.style.equation","advance":[0]}}"#,
            r#"{}"#,
        ]
    );
}

#[test]
fn an_unlabeled_lines_identifier_is_its_position_and_four_letters() {
    // Every line gets an identifier, so that a link can reach any line of any
    // document. A labeled line keeps the encoding of its label; an unlabeled
    // one takes the prefix its element states, its position among the
    // document's equation lines, and four letters computed from those two and
    // nothing else — so the identifier is the same in every document holding
    // a line at that position, and the same on every run.
    let document = "\\begin{equation}\n  a = b \\label{eq:one}\n\\end{equation}\n\n                    \\begin{align}\n  c &= d \\\\\n  e &= f\n\\end{align}\n";
    let identifiers_of = |source: &str| -> Vec<String> {
        let run = run_workflow(&std(), text::WORKFLOW, &[source], &Overrides::none());
        let points = run.table(&REFPOINTS).expect("declared").expect("written");
        let anchors = run.table(&ANCHORS).expect("declared").expect("written");
        points
            .records()
            .iter()
            .map(|record| anchors.id(record.id).expect("an anchor").to_string())
            .collect()
    };

    let found = identifiers_of(document);
    assert_eq!(found[0], "eq-one", "a labeled line keeps its label's encoding");
    assert_eq!(found[1], "eq--2oqua");
    assert_eq!(found[2], "eq--3oqke");
    // The position and the four letters, and nothing but ASCII lowercase
    // letters after the position.
    for (position, identifier) in found.iter().enumerate().skip(1) {
        let letters = identifier
            .strip_prefix(&format!("eq--{}", position + 1))
            .unwrap_or_else(|| panic!("{identifier} begins with its position"));
        assert_eq!(letters.len(), 4, "{identifier}");
        assert!(letters.bytes().all(|byte| byte.is_ascii_lowercase()), "{identifier}");
    }
    // The same document again, and another document whose lines stand at the
    // same positions: the same identifiers.
    assert_eq!(identifiers_of(document), found);
    let other = "\\begin{equation}\n  p = q \\label{eq:one}\n\\end{equation}\n\n                 \\begin{align*}\n  r &= s \\\\\n  t &= u\n\\end{align*}\n";
    assert_eq!(identifiers_of(other), found, "the position alone decides, not the content");
}

#[test]
fn a_label_written_on_a_heading_and_on_an_equation_line_is_a_duplicate() {
    // Two reference points of two features, one label: the first in walk order
    // keeps it and the check reports the second, exactly as two headings are
    // reported.
    let document = "\\section{S}\\label{eq:twice}\n\n\\begin{equation}\n  x \\label{eq:twice}\n\
                    \\end{equation}\n";
    let run = run_workflow(&std(), CHECK_WORKFLOW, &[document], &Overrides::none());
    // The heading refuses the type `eq`, so its own label defines nothing and
    // the line keeps it: the duplicate is reported against the line's point
    // only where the type is one both accept, so this document reports the
    // refusal instead.
    assert_eq!(
        identifiers(&run),
        ["flm.refs.label-type-not-accepted"]
    );
    let document = "\\section{S}\\label{sec:twice}\n\n\\begin{equation}\n  x \\label{sec:twice}\n\
                    \\end{equation}\n";
    let run = run_workflow(&std(), CHECK_WORKFLOW, &[document], &Overrides::none());
    // The line refuses `sec` in its turn. A label both accept is `eq` on the
    // line and nothing on the heading, so the duplicate across the two shows
    // with two equation lines of one environment:
    assert_eq!(
        identifiers(&run),
        ["flm.refs.label-type-not-accepted"]
    );
    let document =
        "\\begin{align}\n  a &= b \\label{eq:same}\\\\\n  c &= d \\label{eq:same}\n\\end{align}\n";
    let run = run_workflow(&std(), CHECK_WORKFLOW, &[document], &Overrides::none());
    let diagnostic = expect(run.diagnostics().iter(), "flm.refs.duplicate-label").once();
    assert!(diagnostic.message().contains("eq:same"), "{}", diagnostic.message());
}

/// A `\nonumber` line and a line of a starred environment, each with a label
/// written on it and a reference naming it; the starred line also carries a
/// text that is no label at all.
const NO_NUMBER_LABELED: &str = "\\begin{align}\n  a &= b \\label{eq:kept}\\\\\n  \
                                 c &= d \\nonumber\\label{eq:none}\n\\end{align}\n\n\
                                 \\begin{align*}\n  e &= f \\label{eq:starred}\\label{nolabel}\n\
                                 \\end{align*}\n\n\
                                 See \\ref{eq:none} and \\ref{eq:starred}.\n";

#[test]
fn a_line_that_will_carry_no_number_is_a_point_nothing_can_refer_to() {
    // A line with no number — one marked `\nonumber`, and every line of a
    // starred environment — is still a reference point, so that every line of
    // every environment is one and tooling can walk them; but it keeps no
    // label, because a reference to it has no number to print. The label
    // written there is the document's error, reported once at the field by the
    // parse; the reference naming it then resolves to nothing, exactly as a
    // reference to a label no construct kept always does.
    let run = run_workflow(&std(), text::WORKFLOW, &[NO_NUMBER_LABELED], &Overrides::none());
    expect(run.diagnostics().iter(), "flm.math.label-without-number")
        .at("\\label{eq:none}")
        .once();
    expect(run.diagnostics().iter(), "flm.math.label-without-number")
        .at("\\label{eq:starred}")
        .once();
    expect(run.diagnostics().iter(), "flm.math.label-without-number")
        .at("\\label{nolabel}")
        .once();
    expect(run.diagnostics().iter(), "flm.refs.unresolved").at("\\ref{eq:none}").once();
    expect(run.diagnostics().iter(), "flm.refs.unresolved").at("\\ref{eq:starred}").once();

    // `nolabel` is not spelled `type:label`, and on a numbered line the scan
    // would report it as `flm.refs.bad-label` at the field. Here the line keeps
    // no text written as a label at all, so the one report the field gets is
    // that it has no number to label — one report per fact about the document,
    // and nothing else is reported anywhere.
    assert_eq!(
        identifiers(&run),
        [
            "flm.math.label-without-number",
            "flm.math.label-without-number",
            "flm.math.label-without-number",
            "flm.refs.unresolved",
            "flm.refs.unresolved",
        ]
    );

    // Three lines, three points; only the numbered one carries its label.
    let labels = run.table(&LABELS).expect("declared").expect("written");
    let points = run.table(&REFPOINTS).expect("declared").expect("written");
    let lines: Vec<(u32, Vec<String>)> = points
        .records()
        .iter()
        .filter(|record| record.point.kind.as_str() == "equation")
        .map(|record| {
            (record.id.index, record.point.labels.iter().map(|label| label.to_string()).collect())
        })
        .collect();
    assert_eq!(
        lines,
        [(0, vec![String::from("eq:kept")]), (1, Vec::new()), (0, Vec::new())],
        "the `\\nonumber` line and the starred line keep nothing"
    );
    assert_eq!(labels.by_label.keys().map(|l| l.to_string()).collect::<Vec<_>>(), ["eq:kept"]);

    // Every point still has an anchor: the labeled line takes its label's, the
    // other two their position among the equation lines.
    let anchors = run.table(&ANCHORS).expect("declared").expect("written");
    let mut identifiers: Vec<&str> = points
        .records()
        .iter()
        .filter(|record| record.point.kind.as_str() == "equation")
        .map(|record| anchors.id(record.id).expect("an anchor"))
        .collect();
    identifiers.sort_unstable();
    assert_eq!(
        identifiers,
        [refs::auto_anchor("eq", 2), refs::auto_anchor("eq", 3), "eq-kept".to_string()]
    );
}
