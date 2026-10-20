//! The verbatim wrap of `flm_core::lang::verbatim`: every verbatim run is one
//! `Group(Verbatim)` node whose interior is read under the rules-off parsing
//! state, under both interior policies.

mod common;

use std::sync::Arc;

use flm_core::model::doc::FragmentLayout;
use flm_core::lang::mint::{ArgDef, ArgStdKind, BodyDef, ElementDef, SyntaxDef};
use flm_core::lang::{
    parse_fragment_with_state, FlmGroupType, FlmStateExt, ParseOptions, TemplateCommandRule,
    TemplateInteriorRules, RestrictedInterior,
};
use techy::core::ParsingStateDelta;

use common::{callable_names, callables, element, env_def, lang, macro_def, parse, source};

fn verb_macro() -> ElementDef {
    ElementDef::new(
        "t",
        ["verbcode"],
        SyntaxDef::Macro { args: vec![ArgDef::new(ArgStdKind::Verbatim { delimiters: None })] },
        element("t.verbcode"),
    )
}

fn verb_env() -> ElementDef {
    env_def("t", "verbatimcode", BodyDef::new().verbatim())
}

#[test]
fn a_verbatim_argument_is_wrapped_in_a_verbatim_group() {
    let lang = lang(vec![verb_macro()]);
    let fragment = parse(&lang, r"\verbcode{a\b}");
    let node = callables(&fragment).into_iter().next().unwrap();
    let argument = node.argument_nodes(0).unwrap();
    let group = argument.iter().find(|node| node.is_group()).unwrap();
    assert_eq!(group.group_type(), Some(FlmGroupType::Verbatim));
    assert_eq!(group.child(0).and_then(|node| node.chars()), Some(r"a\b"));
}

#[test]
fn a_verbatim_argument_content_is_the_group_interior() {
    let lang = lang(vec![verb_macro()]);
    let fragment = parse(&lang, r"\verbcode|a{b|");
    let node = callables(&fragment).into_iter().next().unwrap();
    let content = node.argument_content_nodes(0).unwrap();
    assert_eq!(content.source_text(), Some("a{b"));
}

#[test]
fn a_verbatim_environment_body_is_wrapped_in_a_verbatim_group() {
    let lang = lang(vec![verb_env()]);
    let fragment = parse(&lang, "\\begin{verbatimcode}\n\\x{y}\n\\end{verbatimcode}");
    let node = callables(&fragment).into_iter().next().unwrap();
    let body = node.body().unwrap();
    assert_eq!(body.len(), 1);
    let group = body.first().unwrap();
    assert_eq!(group.group_type(), Some(FlmGroupType::Verbatim));
    assert_eq!(group.child(0).and_then(|node| node.chars()), Some("\\x{y}\n"));
}

#[test]
fn the_document_policy_keeps_commands_off_inside_verbatim() {
    let lang = lang(vec![verb_env(), macro_def("t", "emph", 1)]);
    let fragment = parse(&lang, "\\begin{verbatimcode}\\emph{a}\\end{verbatimcode}");
    assert_eq!(callable_names(&fragment), ["verbatimcode"]);
    assert!(fragment.diags().is_empty());
}

#[test]
fn the_restricted_interior_state_has_the_token_rules_off() {
    let lang = lang(vec![verb_env()]);
    let fragment = parse(&lang, "\\begin{verbatimcode}a\\end{verbatimcode}");
    let node = callables(&fragment).into_iter().next().unwrap();
    let group = node.body().unwrap().first().unwrap();
    let chars = group.child(0).unwrap();
    let rules = chars.parsing_state().rules();
    assert!(!rules.commands.enabled);
    assert!(!rules.comments.enabled);
    assert!(!rules.specials.enabled);
    assert!(!rules.paragraphs.enabled);
}

#[test]
fn the_template_policy_keeps_the_listed_command_visible_inside_verbatim() {
    let lang = lang(vec![verb_env(), macro_def("t", "1", 0)]);
    let policy = RestrictedInterior::Template(TemplateInteriorRules {
        commands: vec![TemplateCommandRule { escape_char: '#', name_chars: "0123456789".into() }],
        specials: false,
    });
    let ext = FlmStateExt::new(Arc::clone(&lang.initial_state().ext().structural))
        .with_restricted_interior(policy);
    let state = lang.derive_state(&ParsingStateDelta::new().ext(ext)).unwrap();
    let fragment = parse_fragment_with_state(
        &lang,
        source("\\begin{verbatimcode}a#1b\\x\\end{verbatimcode}"),
        FragmentLayout::default(),
        &ParseOptions::default(),
        state,
    )
    .unwrap();
    assert_eq!(callable_names(&fragment), ["verbatimcode", "1"]);
    assert!(fragment.diags().is_empty());
}

#[test]
fn the_template_policy_keeps_the_verbatim_wrap() {
    let lang = lang(vec![verb_env(), macro_def("t", "1", 0)]);
    let policy = RestrictedInterior::Template(TemplateInteriorRules {
        commands: vec![TemplateCommandRule { escape_char: '#', name_chars: "0123456789".into() }],
        specials: false,
    });
    let ext = FlmStateExt::new(Arc::clone(&lang.initial_state().ext().structural))
        .with_restricted_interior(policy);
    let state = lang.derive_state(&ParsingStateDelta::new().ext(ext)).unwrap();
    let fragment = parse_fragment_with_state(
        &lang,
        source("\\begin{verbatimcode}a#1b\\end{verbatimcode}"),
        FragmentLayout::default(),
        &ParseOptions::default(),
        state,
    )
    .unwrap();
    let node = callables(&fragment).into_iter().next().unwrap();
    let group = node.body().unwrap().first().unwrap();
    assert_eq!(group.group_type(), Some(FlmGroupType::Verbatim));
    let shapes: Vec<&str> = group
        .children()
        .iter()
        .map(|child| if child.is_callable() { "callable" } else { "chars" })
        .collect();
    assert_eq!(shapes, ["chars", "callable", "chars"]);
}

#[test]
fn the_document_policy_is_the_built_language_default() {
    let lang = lang(Vec::new());
    assert_eq!(lang.initial_state().ext().restricted_interior, RestrictedInterior::Document);
}

#[test]
fn the_newline_gobbled_after_a_begin_is_staged_under_the_surrounding_state() {
    // That newline stands outside the verbatim group and is not verbatim
    // content, so it carries the parsing
    // state of the surroundings — commands recognized — and not the rules-off
    // interior state the body is read under.
    let lang = lang(vec![verb_env()]);
    let fragment = parse(&lang, "\\begin{verbatimcode}\nx\n\\end{verbatimcode}");
    let newline = fragment
        .tree()
        .iter_storage_order()
        .find(|node| node.chars() == Some("\n"))
        .expect("the gobbled newline is staged");
    assert!(newline.parsing_state().rules().commands.enabled);
    // The body's content is the verbatim group alone: the newline is outside it.
    let node = callables(&fragment).into_iter().next().unwrap();
    let body = node.body().unwrap();
    assert_eq!(body.len(), 1);
    assert_eq!(body.first().unwrap().group_type(), Some(FlmGroupType::Verbatim));
}
