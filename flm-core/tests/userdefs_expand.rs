//! The expand pass over user definitions
//! (`flm_core::lang::rewrites::userdefs`): the splice, per-use provenance,
//! argument hygiene, the parsing modes a definition applies in, the recursion
//! bound, nested expansion, a template's paragraph break, region records under
//! substitution, and the space that follows a use site.

mod common;

use std::sync::Arc;

use flm_core::model::element::element_of;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::{Definition, RecursionLimit};
use flm_core::lang::{structural, Flm};
use techy::core::constructs::UnresolvableCommand;
use techy::core::node::{validate_tree, NodeRef, NodeTree};
use techy::error::DiagnosticInfo;
use techy::latexlike::ParagraphBreakSpec;
use techy::source::SourceProvenance;

use common::userdefs::{chars_of, element_names, expanded, recomposed, setup_definitions, Setup};

/// `\foo[opt]{mand}` → `FOO(#1|#2)` in text mode only.
fn foo() -> Definition {
    Definition::macro_("foo", vec![ArgDef::optional("opt"), ArgDef::mandatory("mand")])
        .text("FOO(#1|#2)")
}

/// `\mm` → `MM`, applying in both parsing modes: one template text, compiled
/// under each mode it applies in.
fn mm() -> Definition {
    Definition::macro_("mm", Vec::new()).content("MM")
}

/// The fixture the cases below share.
fn fixture() -> Setup {
    let setup = setup_definitions(vec![
        foo(),
        mm(),
        Definition::macro_("rec", Vec::new()).text("r\\rec"),
        Definition::macro_("twopar", Vec::new()).text("para one\n\npara two"),
        Definition::macro_("wrap", vec![ArgDef::mandatory("x")]).text("W(\\foo{#1})"),
        Definition::macro_("txtonly", Vec::new()).text("T"),
    ]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    setup
}

fn heads(tree: &NodeTree<Flm>) -> Vec<NodeRef<'_, Flm>> {
    tree.root().children().iter().filter(|node| node.chars() == Some("FOO(")).collect()
}

// --- the splice ----------------------------------------------------------------------------

#[test]
fn an_expansion_splices_flat_into_the_sibling_run() {
    let setup = fixture();
    let (tree, diagnostics, _) = expanded(&setup, r"x \foo{B} y", 8);
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    assert_eq!(chars_of(&tree), ["x ", "FOO(", "|", "B", ")", " y"]);
    assert!(tree.root().children().iter().all(|node| node.is_chars()));
}

#[test]
fn expanded_text_nodes_carry_the_text_element() {
    let setup = fixture();
    let (tree, _, _) = expanded(&setup, r"x \foo{B} y", 8);
    assert!(element_names(&tree).iter().all(|name| name == structural::TEXT));
}

#[test]
fn an_expanded_tree_recomposes_to_the_substituted_text() {
    let setup = fixture();
    let (tree, _, _) = expanded(&setup, r"x \foo{B} y", 8);
    assert_eq!(recomposed(&tree), "x FOO(|B) y");
    assert!(validate_tree(&tree).is_ok(), "{:?}", validate_tree(&tree));
}

#[test]
fn a_provided_optional_argument_lands_at_its_placeholder() {
    let setup = fixture();
    let (tree, _, _) = expanded(&setup, r"\foo[A]{B}", 8);
    assert_eq!(recomposed(&tree), "FOO(A|B)");
}

// --- per-use provenance --------------------------------------------------------------------

#[test]
fn each_use_gets_its_own_synthesized_source() {
    let setup = fixture();
    let (tree, _, fragment) = expanded(&setup, r"\foo{A} \foo{B}", 8);
    let heads = heads(&tree);
    assert_eq!(heads.len(), 2);
    assert_eq!(heads[0].span().range(), heads[1].span().range());
    assert!(!heads[0].span().same_source(heads[1].span()));
    assert!(!Arc::ptr_eq(heads[0].span().source(), fragment.source()));
}

#[test]
fn an_expanded_node_chains_provenance_to_its_use_site() {
    let setup = fixture();
    let (tree, _, fragment) = expanded(&setup, r"\foo{A} \foo{B}", 8);
    for (head, use_site) in heads(&tree).iter().zip([r"\foo{A}", r"\foo{B}"]) {
        let chain: Vec<_> = head.span().source().provenance_chain().collect();
        assert_eq!(chain.len(), 2);
        match chain[0] {
            SourceProvenance::Synthesized { description, triggered_at } => {
                assert_eq!(description, "expansion of \\foo");
                assert_eq!(triggered_at.content(), use_site);
                assert!(Arc::ptr_eq(triggered_at.source(), fragment.source()));
            }
            other => panic!("expected a synthesized provenance, got {other:?}"),
        }
        assert!(matches!(chain[1], SourceProvenance::Primary));
    }
}

#[test]
fn the_synthesized_source_holds_the_template_text_at_the_same_offsets() {
    let setup = fixture();
    let (tree, _, _) = expanded(&setup, r"\foo{A}", 8);
    let head = heads(&tree).into_iter().next().unwrap();
    assert_eq!(head.span_content(), "FOO(");
    assert_eq!(head.span().source().content(), "FOO(#1|#2)");
}

#[test]
fn the_synthesized_source_is_labeled_as_the_expansion() {
    // The label is display data for a diagnostic raised at an expanded node; the
    // source has no canonical path, so it keys nothing.
    let setup = fixture();
    let (tree, _, _) = expanded(&setup, r"\foo{A}", 8);
    let head = heads(&tree).into_iter().next().unwrap();
    let origin = head.span().source().origin();
    assert_eq!(origin.synthetic_label(), Some("expansion of \\foo"));
    assert_eq!(origin.canonical_path(), None);
}

// --- argument hygiene ----------------------------------------------------------------------

#[test]
fn argument_nodes_keep_their_document_spans() {
    let setup = fixture();
    let input = r"\foo[opt]{arg}";
    let (tree, _, fragment) = expanded(&setup, input, 8);
    assert_eq!(recomposed(&tree), "FOO(opt|arg)");
    for text in ["opt", "arg"] {
        let node = tree.root().children().iter().find(|node| node.chars() == Some(text)).unwrap();
        assert!(Arc::ptr_eq(node.span().source(), fragment.source()));
        let start = input.find(text).unwrap();
        assert_eq!(node.span().range(), start..start + text.len());
    }
}

#[test]
fn template_nodes_do_not_point_at_the_document() {
    let setup = fixture();
    let (tree, _, fragment) = expanded(&setup, r"\foo[opt]{arg}", 8);
    let head = heads(&tree).into_iter().next().unwrap();
    assert!(!Arc::ptr_eq(head.span().source(), fragment.source()));
}

// --- mode variants -------------------------------------------------------------------------

#[test]
fn a_definition_of_both_modes_expands_in_text_mode() {
    let setup = fixture();
    let (tree, diagnostics, _) = expanded(&setup, r"\mm", 8);
    assert!(diagnostics.is_empty());
    assert_eq!(recomposed(&tree), "MM");
}

#[test]
fn a_definition_of_both_modes_expands_in_math_mode_from_the_same_template() {
    let setup = fixture();
    let (tree, _, _) = expanded(&setup, r"\(\mm\) \mm", 8);
    assert_eq!(recomposed(&tree), r"\(MM\) MM");
}

#[test]
fn a_use_outside_the_definitions_modes_is_an_unknown_callable() {
    // A definition is declared in the parsing modes it applies in and in no
    // other, so its name is simply undefined elsewhere: the parse, not the
    // expansion, answers such a use site.
    let setup = fixture();
    let fragment = common::parse(&setup.lang, r"\(\txtonly\)");
    let tree = fragment.tree();
    let use_site = tree
        .iter_storage_order()
        .find(|node| node.span_content() == r"\txtonly")
        .expect("the use site");
    assert_eq!(element_of(&use_site).name, structural::UNKNOWN_MACRO);
    assert_eq!(fragment.diags().conditions::<UnresolvableCommand>().count(), 1);
}

// --- the recursion bound -------------------------------------------------------------------

#[test]
fn recursion_stops_at_the_limit_with_the_literal_text() {
    let setup = fixture();
    let (tree, _, _) = expanded(&setup, r"\rec", 8);
    assert_eq!(recomposed(&tree), "rrrrrrrr\\rec");
    let (tree, _, _) = expanded(&setup, r"\rec", 3);
    assert_eq!(recomposed(&tree), "rrr\\rec");
}

#[test]
fn the_recovered_invocation_is_a_text_node_in_the_innermost_expansion() {
    let setup = fixture();
    let (tree, _, _) = expanded(&setup, r"\rec", 8);
    let literal = tree.root().children().iter().find(|node| node.chars() == Some("\\rec")).unwrap();
    assert_eq!(element_of(&literal).name, structural::TEXT);
    assert_eq!(literal.span_content(), "\\rec");
    assert_eq!(literal.span().source().provenance_chain().count(), 9);
}

#[test]
fn the_recursion_limit_is_one_typed_diagnostic() {
    let setup = fixture();
    let (_, diagnostics, _) = expanded(&setup, r"\rec", 8);
    assert_eq!(diagnostics.len(), 1);
    let conditions: Vec<&RecursionLimit> = diagnostics.conditions::<RecursionLimit>().collect();
    assert_eq!(conditions.len(), 1);
    assert_eq!(conditions[0].name, "\\rec");
    assert_eq!(conditions[0].limit, 8);
    assert_eq!(diagnostics.iter().next().unwrap().identifier(), RecursionLimit::IDENTIFIER);
}

// --- nested expansion in arguments ---------------------------------------------------------

#[test]
fn a_definition_inside_an_argument_expands_under_its_own_context() {
    let setup = fixture();
    let (tree, diagnostics, fragment) = expanded(&setup, r"\foo{\mm}", 8);
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    assert_eq!(chars_of(&tree), ["FOO(", "|", "MM", ")"]);
    let nested = tree.root().children().iter().find(|node| node.chars() == Some("MM")).unwrap();
    let trigger = nested.span().source().provenance().triggered_at().expect("triggered");
    assert_eq!(trigger.content(), r"\mm");
    assert!(Arc::ptr_eq(trigger.source(), fragment.source()));
}

#[test]
fn a_definition_used_in_a_template_expands_too() {
    let setup = fixture();
    let (tree, _, _) = expanded(&setup, r"\foo{\foo{x}}", 8);
    assert_eq!(recomposed(&tree), "FOO(|FOO(|x))");
}

// --- a paragraph break inside a template ---------------------------------------------------

#[test]
fn a_template_paragraph_break_lands_beside_the_runs_it_will_split() {
    // The splice is flat: the break stands in the root's own child list between
    // the surrounding text runs, with no wrapper group, which is what lets
    // the block-structure pass split the enclosing paragraph later.
    let setup = fixture();
    let (tree, diagnostics, _) = expanded(&setup, r"a \twopar b", 8);
    assert!(diagnostics.is_empty());
    assert_eq!(chars_of(&tree), ["a ", "para one", "<non-chars>", "para two", "b"]);
    let marker = tree.root().child(2).unwrap();
    assert!(Arc::ptr_eq(element_of(&marker), &setup.lang.structural().parbreak));
}

#[test]
fn a_template_paragraph_break_is_the_driver_stamped_marker() {
    let setup = fixture();
    let (tree, _, _) = expanded(&setup, r"a \twopar b", 8);
    let marker = tree.root().child(2).unwrap();
    let spec = marker.spec().expect("a callable spec");
    assert!((&**spec as &dyn std::any::Any).downcast_ref::<ParagraphBreakSpec>().is_some());
    assert_eq!(marker.span_content(), "\n\n");
}

#[test]
fn a_template_paragraph_break_inside_a_group_splices_flat_into_the_group() {
    let setup = fixture();
    let (tree, _, _) = expanded(&setup, r"{a \twopar b}", 8);
    let group = tree.root().child(0).unwrap();
    let pieces: Vec<&str> =
        group.children().iter().map(|node| node.chars().unwrap_or("<non-chars>")).collect();
    assert_eq!(pieces, ["a ", "para one", "<non-chars>", "para two", "b"]);
    let marker = group.child(2).unwrap();
    assert!(Arc::ptr_eq(element_of(&marker), &setup.lang.structural().parbreak));
}

#[test]
fn the_invocations_post_space_disappears_with_it() {
    let setup = fixture();
    let (tree, _, _) = expanded(&setup, r"a \twopar b", 8);
    assert_eq!(recomposed(&tree), "a para one\n\npara twob");
}

// --- region records under substitution -----------------------------------------------------

#[test]
fn a_placeholder_inside_a_nested_argument_keeps_the_region_records_true() {
    let setup = fixture();
    let (tree, diagnostics, _) = expanded(&setup, r"\wrap{a\mm b}", 8);
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    assert_eq!(recomposed(&tree), "W(FOO(|aMMb))");
    assert!(validate_tree(&tree).is_ok(), "{:?}", validate_tree(&tree));
}

#[test]
fn an_empty_argument_at_a_nested_placeholder_empties_the_region() {
    let setup = fixture();
    let (tree, _, _) = expanded(&setup, r"\wrap{}", 8);
    assert_eq!(recomposed(&tree), "W(FOO(|))");
    assert!(validate_tree(&tree).is_ok());
}

// --- the pass over a tree without definitions ---------------------------------------------

#[test]
fn a_tree_without_use_sites_is_carried_over_unchanged() {
    let setup = fixture();
    let (tree, diagnostics, fragment) = expanded(&setup, "plain {text} \\(x\\)", 8);
    assert!(diagnostics.is_empty());
    assert_eq!(recomposed(&tree), "plain {text} \\(x\\)");
    assert_eq!(tree.node_count(), fragment.tree().node_count());
}
