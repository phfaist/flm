//! Template compilation for user definitions
//! (`flm_core::lang::rewrites::userdefs`): the placeholder forms, the escape,
//! the argument number offset, placeholders inside verbatim content, the
//! build-time diagnostics that refuse a definition, and what a compiled
//! template sees.

mod common;

use std::sync::Arc;

use flm_core::model::element::element_of;
use flm_core::lang::mint::{ArgDef, ArgStdKind, ElementDef, SyntaxDef};
use flm_core::lang::rewrites::userdefs::{
    compile, BadDefinition, BadPlaceholder, Definition, DefinitionProblem, PlaceholderProblem,
    PlaceholderTarget,
};
use flm_core::lang::rewrites::blocks::check_parse_contract;
use flm_core::lang::{Flm, FlmCallableType, FlmGroupType};
use techy::core::constructs::UnresolvableCommand;
use techy::core::token::EndOfStreamAfterEscape;
use techy::core::node::NodeTree;
use techy::error::{Diagnostic, DiagnosticInfo};
use techy::latexlike::Mode;

use common::userdefs::{expanded, recomposed, setup, setup_definitions, Setup};
use common::{element, macro_def};

/// The written references of a compiled template's placeholders, in storage order.
fn placeholder_names(setup: &Setup, name: &str, mode: Mode) -> Vec<String> {
    let compiled = setup.definition(name).compiled().expect("compiled");
    let variant = compiled.variant(mode).expect("a template for the mode");
    variant.template().placeholders().map(|(_, reference)| reference.written.clone()).collect()
}

fn bad_placeholders(setup: &Setup) -> Vec<&BadPlaceholder> {
    setup
        .build_diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.data().downcast_ref::<BadPlaceholder>())
        .collect()
}

fn bad_definitions(setup: &Setup) -> Vec<&BadDefinition> {
    setup
        .build_diagnostics
        .iter()
        .filter_map(|diagnostic| diagnostic.data().downcast_ref::<BadDefinition>())
        .collect()
}

/// A verbatim-argument macro `\verbcode{…}` of another feature.
fn verb_macro() -> ElementDef {
    ElementDef::new(
        "t",
        ["verbcode"],
        SyntaxDef::Macro { args: vec![ArgDef::new(ArgStdKind::Verbatim { delimiters: None })] },
        element("t.verbcode"),
    )
}

/// A specials `~` of another feature.
fn tilde_specials() -> ElementDef {
    ElementDef::new(
        "t",
        Vec::<&str>::new(),
        SyntaxDef::Specials { trigger: "~".into(), args: Vec::new() },
        element("t.nbsp"),
    )
}

/// A specials `#!` of another feature.
fn hash_specials() -> ElementDef {
    ElementDef::new(
        "t",
        Vec::<&str>::new(),
        SyntaxDef::Specials { trigger: "#!".into(), args: Vec::new() },
        element("t.hash"),
    )
}

// --- what a compiled template holds --------------------------------------------------------

#[test]
fn placeholders_are_compiled_once_into_typed_nodes() {
    let setup = setup_definitions(vec![
        Definition::macro_("foo", vec![ArgDef::optional("opt"), ArgDef::mandatory("mand")])
            .text("FOO(#1|#2)"),
    ]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    assert_eq!(placeholder_names(&setup, "foo", Mode::Text), ["1", "2"]);
    let compiled = setup.definition("foo").compiled().unwrap();
    let tree = compiled.variant(Mode::Text).unwrap().template().tree();
    let placeholders = tree
        .root()
        .children()
        .iter()
        .filter(|node| node.callable_type() == Some(FlmCallableType::Placeholder))
        .count();
    assert_eq!(placeholders, 2);
    let pieces: Vec<&str> = tree.root().children().iter().filter_map(|node| node.chars()).collect();
    assert_eq!(pieces, ["FOO(", "|", ")"]);
}

#[test]
fn a_placeholder_node_carries_the_placeholder_element() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", vec![ArgDef::mandatory("a")]).text("#1"),
    ]);
    let compiled = setup.definition("f").compiled().unwrap();
    let tree = compiled.variant(Mode::Text).unwrap().template().tree();
    let node = tree.root().child(0).unwrap();
    assert!(Arc::ptr_eq(element_of(&node), &setup.lang.structural().placeholder));
}

#[test]
fn a_template_sees_the_registry_global_names() {
    let setup = setup_definitions(vec![
        Definition::macro_("rec", Vec::new()).text("r\\rec"),
    ]);
    let compiled = setup.definition("rec").compiled().unwrap();
    let tree = compiled.variant(Mode::Text).unwrap().template().tree();
    let names: Vec<&str> =
        tree.root().children().iter().filter_map(|node| node.macro_name()).collect();
    assert_eq!(names, ["rec"]);
}

#[test]
fn a_definition_without_a_math_template_has_no_math_variant() {
    let setup = setup_definitions(vec![
        Definition::macro_("t", Vec::new()).text("T"),
    ]);
    let compiled = setup.definition("t").compiled().unwrap();
    assert!(compiled.variant(Mode::Math).is_none());
    assert!(compiled.variant(Mode::Text).is_some());
}

#[test]
fn a_math_template_is_parsed_in_math_mode() {
    let setup = setup_definitions(vec![Definition::macro_("m", Vec::new()).math("x")]);
    let compiled = setup.definition("m").compiled().unwrap();
    let tree = compiled.variant(Mode::Math).unwrap().template().tree();
    assert_eq!(tree.root().child(0).unwrap().parsing_state().mode(), Mode::Math);
}

// --- the placeholder forms -----------------------------------------------------------------

#[test]
fn a_named_placeholder_resolves_to_the_declared_argument() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", vec![ArgDef::mandatory("first"), ArgDef::mandatory("second")])
            .text("#{second}-#{first}"),
    ]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let compiled = setup.definition("f").compiled().unwrap();
    let targets: Vec<PlaceholderTarget> = compiled
        .variant(Mode::Text)
        .unwrap()
        .template()
        .placeholders()
        .map(|(_, reference)| reference.target.clone())
        .collect();
    assert_eq!(targets, [PlaceholderTarget::Argument(1), PlaceholderTarget::Argument(0)]);
    let (tree, _, _) = expanded(&setup, r"\f{a}{b}", 8);
    assert_eq!(recomposed(&tree), "b-a");
}

#[test]
fn a_double_escape_is_a_literal_hash() {
    let setup = setup_definitions(vec![
        Definition::macro_("h", Vec::new()).text("a##b"),
    ]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let (tree, _, _) = expanded(&setup, r"\h", 8);
    assert_eq!(recomposed(&tree), "a#b");
}

#[test]
fn a_lone_hash_at_the_end_of_a_template_is_refused_at_build() {
    // techy's rule for an escape character with nothing after it; a literal
    // trailing `#` is written `##`.
    let setup = setup_definitions(vec![Definition::macro_("h", Vec::new()).text("a#")]);
    assert_eq!(setup.build_diagnostics.len(), 1);
    assert!(setup.build_diagnostics[0].data().is::<EndOfStreamAfterEscape>());
    assert!(setup.definition("h").compiled().is_none());
}

#[test]
fn a_digit_after_a_numbered_placeholder_is_content() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", vec![ArgDef::mandatory("a")]).text("#12"),
    ]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let (tree, _, _) = expanded(&setup, r"\f{x}", 8);
    assert_eq!(recomposed(&tree), "x2");
}

#[test]
fn the_space_after_a_numbered_placeholder_is_kept() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", vec![ArgDef::mandatory("a")]).text("#1 and"),
    ]);
    let (tree, _, _) = expanded(&setup, r"\f{x}", 8);
    assert_eq!(recomposed(&tree), "x and");
}

#[test]
fn the_declared_argument_number_offset_shifts_every_numbered_placeholder() {
    // The offset is one value for the whole template, declared on the definition
    // as the Python implementation's `substmacros` declares it.
    let setup = setup_definitions(vec![
        Definition::macro_("f", vec![ArgDef::mandatory("a"), ArgDef::mandatory("b")])
            .text("<#1>")
            .argument_number_offset(1),
    ]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let (tree, _, _) = expanded(&setup, r"\f{x}{y}", 8);
    assert_eq!(recomposed(&tree), "<y>");
}

#[test]
fn a_named_reference_is_not_shifted_by_the_argument_number_offset() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", vec![ArgDef::mandatory("a"), ArgDef::mandatory("b")])
            .text("<#{a}|#1>")
            .argument_number_offset(1),
    ]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let (tree, _, _) = expanded(&setup, r"\f{x}{y}", 8);
    assert_eq!(recomposed(&tree), "<x|y>");
}

#[test]
fn the_argument_number_offset_shortens_the_numbered_range() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", vec![ArgDef::mandatory("a"), ArgDef::mandatory("b")])
            .text("#2")
            .argument_number_offset(1),
    ]);
    let bad = bad_placeholders(&setup);
    assert_eq!(bad.len(), 1);
    // The reported arity is what a number may still choose from, not the
    // declared count.
    assert_eq!(bad[0].reason, PlaceholderProblem::OutOfRange { arity: 1 });
    assert!(setup.definition("f").compiled().is_none());
}

#[test]
fn the_offset_macro_is_no_longer_a_template_macro() {
    // The Python implementation has no such macro either: written in a template
    // body it is simply an unknown macro.
    let setup = setup_definitions(vec![
        Definition::macro_("f", vec![ArgDef::mandatory("a")])
            .text("\\SetArgumentNumberOffset{1}#1"),
    ]);
    assert!(setup.definition("f").compiled().is_none());
    assert!(setup
        .build_diagnostics
        .iter()
        .any(|diagnostic| diagnostic.data().is::<UnresolvableCommand>()));
}

// --- placeholders inside verbatim content --------------------------------------------------

#[test]
fn a_placeholder_inside_verbatim_content_interpolates() {
    let setup = setup(
        vec![Definition::macro_("code", vec![ArgDef::mandatory("a")]).text("\\verbcode{[#1]}")],
        vec![verb_macro()],
    );
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let (tree, diagnostics, _) = expanded(&setup, r"\code{xy}", 8);
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    let verbatim = tree
        .root()
        .descendants()
        .find(|node| node.group_type() == Some(FlmGroupType::Verbatim))
        .expect("the verbatim group survives the expansion");
    let texts: Vec<&str> = verbatim.children().iter().filter_map(|node| node.chars()).collect();
    assert_eq!(texts, ["[", "xy", "]"]);
}

/// A placeholder stands for the argument's content nodes and nothing else: a
/// verbatim argument substitutes the characters between its delimiters, not
/// its group, and the parse contract has nothing to say about where they
/// stand.
#[test]
fn a_verbatim_argument_substitutes_its_characters() {
    let code = Definition::macro_("code", vec![ArgDef::new(ArgStdKind::Verbatim { delimiters: None })])
        .text("<#1>");
    let setup = setup_definitions(vec![code]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let (tree, diagnostics, _) = expanded(&setup, "\\code|a{b|", 8);
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    assert!(!tree.root().descendants().any(|node| node.group_type() == Some(FlmGroupType::Verbatim)));
    let texts: Vec<&str> = tree.root().descendants().filter_map(|node| node.chars()).collect();
    assert_eq!(texts.concat(), "<a{b>");
    assert_eq!(recomposed(&tree), "<a{b>");
    assert!(check_parse_contract(&tree).is_empty(), "{:?}", check_parse_contract(&tree));
}

#[test]
fn a_named_placeholder_inside_verbatim_content_interpolates() {
    let setup = setup(
        vec![Definition::macro_("code", vec![ArgDef::mandatory("src")]).text("\\verbcode|#{src}|")],
        vec![verb_macro()],
    );
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let (tree, _, _) = expanded(&setup, r"\code{q}", 8);
    assert_eq!(recomposed(&tree), "\\verbcode|q|");
}

#[test]
fn a_specials_of_the_document_language_stays_invisible_inside_template_verbatim() {
    let setup = setup(
        vec![Definition::macro_("code", Vec::new()).text("\\verbcode{a~b}")],
        vec![verb_macro(), tilde_specials()],
    );
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let compiled = setup.definition("code").compiled().unwrap();
    let tree = compiled.variant(Mode::Text).unwrap().template().tree();
    let specials = tree
        .root()
        .descendants()
        .filter(|node| node.callable_type() == Some(FlmCallableType::Specials))
        .count();
    assert_eq!(specials, 0);
}

#[test]
fn a_specials_of_the_document_language_is_recognized_outside_verbatim() {
    let setup =
        setup(vec![Definition::macro_("t", Vec::new()).text("a~b")], vec![tilde_specials()]);
    let compiled = setup.definition("t").compiled().unwrap();
    let tree = compiled.variant(Mode::Text).unwrap().template().tree();
    let specials = tree
        .root()
        .descendants()
        .filter(|node| node.callable_type() == Some(FlmCallableType::Specials))
        .count();
    assert_eq!(specials, 1);
}

#[test]
fn a_document_specials_triggered_by_the_escape_is_shadowed_inside_a_template() {
    // techy reads commands before specials, so `#` opens a placeholder wherever
    // it stands in a template; a document specials whose trigger begins with `#`
    // is therefore not writable there.
    let setup = setup(
        vec![Definition::macro_("t", Vec::new()).text("a#!b")],
        vec![hash_specials()],
    );
    let bad = bad_placeholders(&setup);
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].reference, "!");
    assert_eq!(bad[0].reason, PlaceholderProblem::UnknownName);
}

#[test]
fn a_backslash_command_named_like_a_placeholder_form_is_not_a_placeholder() {
    // A feature defining the control symbol `\{`: in a template it must resolve to
    // that definition, never to the placeholder parser (which answers the `#`
    // escape only).
    let brace = macro_def("t", "{", 0);
    let brace_element = Arc::clone(&brace.element);
    let setup = setup(vec![Definition::macro_("t", Vec::new()).text("x\\{y")], vec![brace]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let compiled = setup.definition("t").compiled().unwrap();
    let tree = compiled.variant(Mode::Text).unwrap().template().tree();
    let callables: Vec<_> = tree.root().descendants().filter(|node| node.is_callable()).collect();
    assert_eq!(callables.len(), 1);
    assert!(Arc::ptr_eq(element_of(&callables[0]), &brace_element));
}

// --- the diagnostics that refuse a definition ---------------------------------------------

#[test]
fn an_out_of_range_placeholder_is_refused_at_build() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", vec![ArgDef::mandatory("a")]).text("#2"),
    ]);
    let bad = bad_placeholders(&setup);
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].name, "\\f");
    assert_eq!(bad[0].reference, "2");
    assert_eq!(bad[0].reason, PlaceholderProblem::OutOfRange { arity: 1 });
    assert!(setup.definition("f").compiled().is_none());
}

#[test]
fn an_unknown_placeholder_name_is_refused_at_build() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", Vec::new()).text("#{nope}"),
    ]);
    let bad = bad_placeholders(&setup);
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].reference, "nope");
    assert_eq!(bad[0].reason, PlaceholderProblem::UnknownName);
}

#[test]
fn a_body_reference_in_a_macro_is_refused_at_build() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", Vec::new()).text("#{body}"),
    ]);
    let bad = bad_placeholders(&setup);
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].reason, PlaceholderProblem::NoBody);
}

#[test]
fn a_substitution_argument_on_a_base_reference_is_refused_at_build() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", vec![ArgDef::mandatory("a")]).text("#[x]{1}"),
    ]);
    let bad = bad_placeholders(&setup);
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].reference, "1");
    assert_eq!(bad[0].reason, PlaceholderProblem::SubstitutionNotAccepted);
}

#[test]
fn a_bad_placeholder_is_reported_in_the_templates_own_source() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", Vec::new()).text("ab #{nope} cd"),
    ]);
    let diagnostic: &Diagnostic<_> = setup
        .build_diagnostics
        .iter()
        .find(|diagnostic| diagnostic.data().is::<BadPlaceholder>())
        .unwrap();
    assert_eq!(diagnostic.identifier(), BadPlaceholder::IDENTIFIER);
    assert_eq!(diagnostic.span().content(), "#{nope}");
    assert_eq!(diagnostic.span().source().content(), "ab #{nope} cd");
}

#[test]
fn an_unresolvable_command_in_a_template_is_a_build_time_diagnostic() {
    let setup = setup_definitions(vec![
        Definition::macro_("bad", Vec::new()).text("X \\item Y"),
    ]);
    let unresolvable: Vec<&Diagnostic<_>> = setup
        .build_diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.data().is::<UnresolvableCommand>())
        .collect();
    assert_eq!(unresolvable.len(), 1);
    assert_eq!(unresolvable[0].span().content(), "\\item ");
    assert!(setup.definition("bad").compiled().is_none());
}

#[test]
fn a_definition_without_any_template_is_refused_at_build() {
    let setup = setup_definitions(vec![Definition::macro_("empty", Vec::new())]);
    let bad = bad_definitions(&setup);
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].name, "\\empty");
    assert_eq!(bad[0].problem, DefinitionProblem::NoTemplate);
}

#[test]
fn a_default_for_an_unknown_argument_is_refused_at_build() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", vec![ArgDef::mandatory("a")]).text("#1").default("zz", "x"),
    ]);
    let bad = bad_definitions(&setup);
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].problem, DefinitionProblem::UnknownDefault { key: String::from("zz") });
}

#[test]
fn defaults_referring_to_each_other_are_refused_at_build() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", vec![ArgDef::optional("a"), ArgDef::optional("b")])
            .text("#1#2")
            .default("a", "#2")
            .default("b", "#1"),
    ]);
    let bad = bad_placeholders(&setup);
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].reason, PlaceholderProblem::DefaultCycle);
    assert!(setup.definition("f").compiled().is_none());
}

#[test]
fn a_refused_definition_leaves_the_language_usable() {
    let setup = setup_definitions(vec![
        Definition::macro_("bad", Vec::new()).text("#{nope}"),
        Definition::macro_("good", Vec::new()).text("G"),
    ]);
    let (tree, _, _) = expanded(&setup, r"\good", 8);
    assert_eq!(recomposed(&tree), "G");
}

// --- the sources the diagnostics are raised in --------------------------------------------

#[test]
fn a_template_parse_diagnostic_is_raised_in_a_source_named_after_the_definition() {
    // The compiled template's source carries a label naming the definition and
    // the mode, so a diagnostic is named per definition, and no canonical path,
    // so it keys no path-keyed table.
    let setup = setup_definitions(vec![
        Definition::macro_("bad", Vec::new()).text("X \\item Y"),
    ]);
    let diagnostic = setup
        .build_diagnostics
        .iter()
        .find(|diagnostic| diagnostic.data().is::<UnresolvableCommand>())
        .unwrap();
    let origin = diagnostic.span().source().origin();
    assert_eq!(origin.synthetic_label(), Some("template of \\bad (text mode)"));
    assert_eq!(origin.canonical_path(), None);
}

#[test]
fn a_default_values_diagnostic_is_raised_in_a_source_named_after_the_default() {
    let setup = setup_definitions(vec![
        Definition::macro_("f", vec![ArgDef::optional("a")]).text("#1").default("a", "#{nope}"),
    ]);
    let diagnostic = setup
        .build_diagnostics
        .iter()
        .find(|diagnostic| diagnostic.data().is::<BadPlaceholder>())
        .unwrap();
    let label = diagnostic.span().source().origin().synthetic_label();
    assert_eq!(label, Some("default value of #1 of \\f (text mode)"));
}

#[test]
fn a_definition_level_diagnostic_is_raised_in_a_source_named_after_the_declaration() {
    let setup = setup_definitions(vec![Definition::macro_("empty", Vec::new())]);
    let label = setup.build_diagnostics[0].span().source().origin().synthetic_label();
    assert_eq!(label, Some("declaration of \\empty"));
}

// --- the shared template language -----------------------------------------------------------

#[test]
fn a_definition_compiles_over_a_template_language_built_for_the_call() {
    // `compile` is the convenience over `compile_with` (the helpers use the
    // latter, sharing one template language across the roster).
    let setup = setup_definitions(vec![Definition::macro_("t", Vec::new()).text("T")]);
    let compiled = compile(setup.definition("t").definition(), &setup.lang).unwrap();
    assert!(compiled.variant(Mode::Text).is_some());
}

/// Sanity: a compiled tree is a plain, unannotated `NodeTree<Flm>`.
#[test]
fn a_compiled_template_is_an_unannotated_tree() {
    let setup = setup_definitions(vec![
        Definition::macro_("t", Vec::new()).text("T"),
    ]);
    let compiled = setup.definition("t").compiled().unwrap();
    let tree: &Arc<NodeTree<Flm>> = compiled.variant(Mode::Text).unwrap().template().tree();
    assert_eq!(tree.root().child_count(), 1);
}
