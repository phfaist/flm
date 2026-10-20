//! The standard argument kind `ArgStdKind::Terminated`: an argument that runs
//! up to a terminator, its content parsed (`\phf … \endphf`) or read as raw
//! text (a code span `` `…` ``), declared in Rust or by an argument code.

mod common;

use std::sync::Arc;

use flm_core::dump::diag::ExpectDiag;
use flm_core::dump::json::to_json;
use flm_core::dump::project::{project_node, project_tree, ProjectOptions};
use flm_core::lang::mint::{arg_def_from_code, ArgDef, ArgStdKind, ElementDef, SyntaxDef};
use flm_core::lang::{
    parse_fragment_with_state, FlmGroupType, FlmStateExt, ParseOptions, RestrictedInterior,
    TemplateCommandRule, TemplateInteriorRules,
};
use flm_core::model::doc::{Fragment, FragmentLayout};
use techy::core::ParsingStateDelta;

use common::{callable_names, callables, element, lang, macro_def, macro_with, parse, source};

/// `\phf … \endphf`, its content parsed.
fn phf() -> ElementDef {
    let kind = ArgStdKind::Terminated {
        terminator: String::from("\\endphf"),
        repeat_char: None,
        opener: String::new(),
        head: String::new(),
        raw: false,
    };
    macro_with("t", "phf", vec![ArgDef::named("content", kind)])
}

/// The code span: the special character sequence `` ` `` whose argument is
/// raw text up to as many backticks as opened the span.
fn code_span() -> ElementDef {
    let kind = ArgStdKind::Terminated {
        terminator: String::from("`"),
        repeat_char: Some('`'),
        opener: String::new(),
        head: String::new(),
        raw: true,
    };
    ElementDef::new(
        "t",
        Vec::<&str>::new(),
        SyntaxDef::Specials { trigger: "`".into(), args: vec![ArgDef::named("code", kind)] },
        element("t.code"),
    )
}

fn json(value: &flm_core::techy::serialize::SerialValue) -> serde_json::Value {
    serde_json::from_str(&to_json(value).unwrap()).unwrap()
}

#[test]
fn a_terminated_macro_reads_parsed_content_up_to_its_terminator() {
    let lang = lang(vec![phf()]);
    let fragment = parse(&lang, r"\phf a {b}\endphf!");
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    let node = callables(&fragment).into_iter().next().unwrap();
    let content = node.argument_content_nodes(0).unwrap();
    assert_eq!(content.len(), 2);
    assert_eq!(content.source_text(), Some("a {b}"));
    let group = node.argument_nodes(0).unwrap().get(0).unwrap();
    assert_eq!(group.group_type(), Some(FlmGroupType::Content));
    assert_eq!(group.group_delimiters(), Some(("", r"\endphf")));
    assert_eq!(
        json(&project_tree(fragment.tree(), &ProjectOptions::new())),
        serde_json::json!([
            { "elem": "t.phf", "name": "phf", "args": { "content": [
                { "chars": "a " },
                { "elem": "flm.group", "children": [{ "chars": "b" }] },
            ] } },
            { "chars": "!" },
        ])
    );
}

#[test]
fn the_terminator_wins_over_a_command_and_ends_only_outside_a_nested_group() {
    let lang = lang(vec![phf(), macro_def("t", "emph", 1)]);
    // Inside the nested group, `\endphf` is an ordinary command, which no
    // feature defines.
    let fragment = parse(&lang, r"\phf \emph{x}{y\endphf}z\endphf");
    fragment.diags().expect("core.specs.unresolvable-command").at(r"\endphf").once();
    assert_eq!(callable_names(&fragment), ["phf", "emph", "endphf"]);
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(node.argument_content_nodes(0).unwrap().source_text(), Some(r"\emph{x}{y\endphf}z"));
}

#[test]
fn a_code_span_reads_raw_text_up_to_as_many_backticks_as_opened_it() {
    let lang = lang(vec![code_span(), macro_def("t", "emph", 1)]);
    for (text, code, delimiters) in [
        ("`x`", "x", ("", "`")),
        ("``a`b``", "a`b", ("`", "``")),
        ("` x`", " x", ("", "`")),
        (r"`\emph{%x}`", r"\emph{%x}", ("", "`")),
    ] {
        let fragment = parse(&lang, text);
        assert!(fragment.diags().is_empty(), "{text}: {:?}", fragment.diags());
        let nodes = callables(&fragment);
        assert_eq!(nodes.len(), 1, "{text}");
        let content = nodes[0].argument_content_nodes(0).unwrap();
        assert_eq!(content.source_text(), Some(code), "{text}");
        let group = nodes[0].argument_nodes(0).unwrap().get(0).unwrap();
        assert_eq!(group.group_type(), Some(FlmGroupType::Verbatim), "{text}");
        assert_eq!(group.group_delimiters(), Some(delimiters), "{text}");
        let chars = group.child(0).unwrap();
        let rules = chars.parsing_state().rules();
        assert!(!rules.commands.enabled && !rules.comments.enabled && !rules.specials.enabled);
    }
}

#[test]
fn a_raw_argument_projects_as_a_verbatim_group_holding_the_characters() {
    let lang = lang(vec![code_span()]);
    let fragment = parse(&lang, "a ``x`y`` b");
    assert_eq!(
        json(&project_tree(fragment.tree(), &ProjectOptions::new())),
        serde_json::json!([
            { "chars": "a " },
            { "elem": "t.code", "args": { "code": [{ "chars": "x`y" }] } },
            { "chars": " b" },
        ])
    );
    let node = callables(&fragment).into_iter().next().unwrap();
    let group = node.argument_nodes(0).unwrap().get(0).unwrap();
    assert_eq!(
        json(&project_node(&group, &ProjectOptions::new())),
        serde_json::json!({ "elem": "flm.verbatim", "children": [{ "chars": "x`y" }] })
    );
}

#[test]
fn a_missing_terminator_is_reported_at_the_argument_s_beginning() {
    let lang = lang(vec![phf(), code_span()]);
    let fragment = parse(&lang, "\\phf a\nb");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(node.argument_content_nodes(0).unwrap().source_text(), Some("a\nb"));
    fragment.diags().expect("core.arguments.missing-terminator").at_line_col(1, 6).once();

    let fragment = parse(&lang, "x ``ab` c");
    let diagnostic = fragment.diags().expect("core.arguments.missing-terminator").at("`").at_line_col(1, 4).once();
    assert!(diagnostic.message().contains("‘``’"), "{}", diagnostic.message());
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(node.argument_content_nodes(0).unwrap().source_text(), Some("ab` c"));
}

#[test]
fn an_incomplete_prefix_is_reported_where_the_run_was_expected_and_the_argument_is_absent() {
    // With `F+`, the run of `+` is the whole terminator: a use site that
    // writes none of it has no argument.
    let vb = lang(vec![macro_with("t", "vb", vec![arg_def_from_code("F+").unwrap()])]);
    let fragment = parse(&vb, r"\vb a+");
    let diagnostic = fragment.diags().expect("core.arguments.expected-opener").at_line_col(1, 5).once();
    assert!(diagnostic.message().contains("‘+’"), "{}", diagnostic.message());
    let node = callables(&fragment).into_iter().next().unwrap();
    assert!(node.argument_nodes(0).is_none());
}

#[test]
fn the_argument_codes_declare_the_same_arguments() {
    let declare = |code: &str| {
        let arg = arg_def_from_code(code).unwrap();
        ElementDef::new(
            "t",
            Vec::<&str>::new(),
            SyntaxDef::Specials { trigger: "`".into(), args: vec![arg] },
            element("t.code"),
        )
    };
    let span = lang(vec![declare("F``")]);
    let fragment = parse(&span, "``a`b``");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(node.argument_content_nodes(0).unwrap().source_text(), Some("a`b"));

    let phf = lang(vec![macro_with("t", "phf", vec![arg_def_from_code(r"u\endphf").unwrap()])]);
    let fragment = parse(&phf, r"\phf a {b}\endphf");
    let node = callables(&fragment).into_iter().next().unwrap();
    let group = node.argument_nodes(0).unwrap().get(0).unwrap();
    assert_eq!(group.group_type(), Some(FlmGroupType::Content));
    assert_eq!(node.argument_content_nodes(0).unwrap().len(), 2);

    let vb = lang(vec![macro_with("t", "vb", vec![arg_def_from_code("F+").unwrap()])]);
    let fragment = parse(&vb, r"\vb+++a+b %c+++");
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(node.argument_content_nodes(0).unwrap().source_text(), Some("a+b %c"));
}

#[test]
fn the_template_policy_keeps_its_rules_inside_a_raw_terminated_argument() {
    let lang = lang(vec![code_span(), macro_def("t", "1", 0)]);
    let policy = RestrictedInterior::Template(TemplateInteriorRules {
        commands: vec![TemplateCommandRule { escape_char: '#', name_chars: "0123456789".into() }],
        specials: false,
    });
    let ext = FlmStateExt::new(Arc::clone(&lang.initial_state().ext().structural))
        .with_restricted_interior(policy);
    let state = lang.derive_state(&ParsingStateDelta::new().ext(ext)).unwrap();
    let fragment: Fragment = parse_fragment_with_state(
        &lang,
        source("`a#1b\\x`"),
        FragmentLayout::default(),
        &ParseOptions::default(),
        state,
    )
    .unwrap();
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    let node = callables(&fragment).into_iter().next().unwrap();
    let group = node.argument_nodes(0).unwrap().get(0).unwrap();
    assert_eq!(group.group_type(), Some(FlmGroupType::Verbatim));
    let shapes: Vec<&str> = group
        .children()
        .iter()
        .map(|child| if child.is_callable() { "callable" } else { "chars" })
        .collect();
    assert_eq!(shapes, ["chars", "callable", "chars"]);
}
