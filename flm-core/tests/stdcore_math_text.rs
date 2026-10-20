//! The text commands of mathematics (`flm_core::stdcore::features::math::text`):
//! `\text{…}` and its kin are declared for math mode only, their argument is
//! read as ordinary document content, the same names in text mode are what they
//! always were, an unknown macro beside them is still opaque source, and what
//! the parse builds survives freezing and the serialized round trip.

mod common;

use std::sync::Arc;

use flm_core::core::plan::Overrides;
use flm_core::core::registry::FlmRegistry;
use flm_core::dump::project::{ProjectOptions, Projector};
use flm_core::lang::structural;
use flm_core::model::element::{element_of, element_value};
use flm_core::stdcore::backends::text;
use flm_core::stdcore::features::math::text::{COMMANDS, MathTextCommand, RULE, TEXT};
use flm_core::stdcore::features::textstyles::{TEXT_STYLE, TextStyle, TextStyleElement};
use flm_core::math_passthrough;
use flm_core::stdcore::features::{math, refs};
use flm_core::stdcore::{StdcoreConfig, backends::pres};
use techy::core::node::{NodeRef, NodeTree};
use techy::latexlike::Mode;

use common::dump::round_trip;
use common::stdcore::{identifiers, registry, run_workflow};

/// The whole standard package: the `math` feature stands beside `textstyles`,
/// which declares three of the same names in text mode only, and beside `refs`,
/// whose `\ref` a text argument may hold.
fn std_registry() -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new())
}

/// Every element value the registry holds, for the reading side's resolver.
fn elements(registry: &Arc<FlmRegistry>) -> Vec<Arc<flm_core::model::element::ElementSpec>> {
    registry.elements().iter().map(|entry| Arc::clone(&entry.element)).collect()
}

/// The element names of a parse's callable nodes, in document order.
fn callable_elements(registry: &Arc<FlmRegistry>, source: &str) -> Vec<String> {
    let fragment = common::parse(registry.lang(), source);
    common::callables(&fragment).iter().map(|node| element_of(node).name.to_string()).collect()
}

/// The one `flm.math.text` node of a parse of `source`.
fn text_node<'t>(fragment: &'t flm_core::model::doc::Fragment) -> NodeRef<'t, flm_core::lang::Flm> {
    fragment
        .tree()
        .iter_storage_order()
        .find(|node| element_of(node).name == TEXT)
        .expect("a text command was parsed")
}

/// The projection of a tree, spans included.
fn project(tree: &NodeTree<flm_core::lang::Flm>) -> techy::serialize::SerialValue {
    Projector::new(ProjectOptions::new().with_spans()).tree(tree)
}

#[test]
fn every_text_command_parses_in_math_mode_as_one_value_of_one_element_type() {
    let registry = std_registry();
    for command in COMMANDS {
        let source = format!("\\(x\\{command}{{a}}\\)");
        let fragment = common::parse(registry.lang(), &source);
        assert!(fragment.diags().is_empty(), "{command}: {:?}", fragment.diags());
        let node = text_node(&fragment);
        let value = element_value::<MathTextCommand>(&node).expect("the command value");
        assert_eq!(value.command, command, "{source}");
    }
}

#[test]
fn the_argument_of_a_text_command_is_read_in_text_mode() {
    let registry = std_registry();
    let fragment = common::parse(registry.lang(), "\\(x\\text{a}y\\)");
    let node = text_node(&fragment);
    // The command itself stands in mathematics; its argument does not.
    assert_eq!(node.parsing_state().mode(), Mode::Math);
    let content = node.argument_content_nodes(0).expect("the provided argument");
    for child in content.iter() {
        assert_eq!(child.parsing_state().mode(), Mode::Text, "{}", child.summary());
    }
}

#[test]
fn a_text_argument_holds_ordinary_nodes() {
    let registry = std_registry();
    // A styled span, a reference, and mathematics again, all inside one
    // argument: each parses as what it is in text.
    let source = "\\(\\text{see \\emph{a} \\ref{sec:b} and \\(y^2\\)}\\)";
    let fragment = common::parse(registry.lang(), source);
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    let inside: Vec<String> = fragment
        .tree()
        .iter_storage_order()
        .filter(|node| node.is_callable())
        .map(|node| element_of(&node).name.to_string())
        .collect();
    assert_eq!(inside, [TEXT, TEXT_STYLE, refs::REF]);
    // Mathematics again: a math group of its own inside the argument.
    assert!(
        fragment
            .tree()
            .iter_storage_order()
            .any(|node| element_of(&node).name == structural::MATH),
        "the nested mathematics is a math region of its own"
    );
    // The styled span is the `textstyles` feature's, as in text anywhere else.
    let styled = fragment
        .tree()
        .iter_storage_order()
        .find(|node| element_of(node).name == TEXT_STYLE)
        .expect("the styled span");
    let value = element_value::<TextStyleElement>(&styled).expect("the style value");
    assert_eq!(value.style, TextStyle::Emphasis);
}

#[test]
fn a_command_that_mathematics_accepts_and_text_refuses_is_refused_inside_the_argument() {
    // The one thing math mode and text mode disagree about is a callable no
    // feature defines: mathematics accepts it silently as opaque source, text
    // refuses it. Inside a text argument text's answer is the one that holds.
    let registry = std_registry();
    let plain = common::parse(registry.lang(), "\\(\\alpha\\)");
    assert!(plain.diags().is_empty(), "{:?}", plain.diags());
    assert_eq!(callable_elements(&registry, "\\(\\alpha\\)"), [math_passthrough::OPAQUE]);
    let inside = common::parse(registry.lang(), "\\(\\text{\\alpha}\\)");
    assert!(inside.diags().has_errors(), "an unknown command in a text argument is an error");
    assert_eq!(
        callable_elements(&registry, "\\(\\text{\\alpha}\\)"),
        [TEXT, structural::UNKNOWN_MACRO]
    );
}

#[test]
fn the_same_names_in_text_mode_are_untouched() {
    let registry = std_registry();
    // `\textbf` in text mode is the `textstyles` feature's, as before.
    assert_eq!(callable_elements(&registry, "\\textbf{a}"), [TEXT_STYLE]);
    // `\text` and `\mbox` are undefined in text mode, as before.
    for command in ["text", "mbox", "textrm"] {
        let source = format!("\\{command}{{a}}");
        let fragment = common::parse(registry.lang(), &source);
        assert!(fragment.diags().has_errors(), "{source}");
        assert_eq!(callable_elements(&registry, &source), [structural::UNKNOWN_MACRO], "{source}");
    }
}

#[test]
fn an_unknown_macro_beside_a_text_command_is_still_opaque_source() {
    let registry = std_registry();
    let source = "\\(\\alpha \\text{and} \\beta\\)";
    let fragment = common::parse(registry.lang(), source);
    assert!(fragment.diags().is_empty(), "{:?}", fragment.diags());
    assert_eq!(callable_elements(&registry, source), [math_passthrough::OPAQUE, TEXT, math_passthrough::OPAQUE]);
}

#[test]
fn the_element_owns_one_rule_on_every_backend_and_one_on_the_recomposed_latex() {
    let registry = std_registry();
    let mut rules: Vec<&str> =
        registry.rules_for(TEXT).iter().map(|idx| registry.rule(*idx).rule.name.as_str()).collect();
    rules.sort_unstable();
    assert_eq!(rules, [RULE, math::latex::TEXT_LATEX_RULE]);
    let general = &registry.rule(registry.rules_for(TEXT)[0]).rule;
    assert_eq!(general.name, RULE);
    assert_eq!(
        general.decl.produces,
        [flm_core::model::names::ElementName::from(pres::MATH_TEXT)]
    );
}

#[test]
fn the_parse_survives_the_serialized_round_trip_unchanged() {
    let registry = std_registry();
    let live = Arc::new(common::parse(
        registry.lang(),
        "\\(x + \\textbf{a --- b} = \\(y\\)\\) and \\(\\mbox{}\\)",
    ));
    let read = round_trip(registry.lang(), &live, elements(&registry));
    assert_eq!(project(read.tree()), project(live.tree()));
}

#[test]
fn what_the_parse_builds_reaches_the_frozen_tree() {
    let registry = std_registry();
    let run = run_workflow(
        &registry,
        text::WORKFLOW,
        &["A formula \\(x = \\text{one}\\) here."],
        &Overrides::none(),
    );
    assert_eq!(identifiers(&run), Vec::<String>::new());
    let frozen = common::wiring::frozen(&run);
    let node = frozen
        .iter_storage_order()
        .find(|node| element_of(node).name == TEXT)
        .expect("the text command survives freezing");
    let value = element_value::<MathTextCommand>(&node).expect("the command value");
    assert_eq!(value.command, "text");
}
