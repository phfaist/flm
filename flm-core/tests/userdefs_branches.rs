//! The conditional families of the user-definition template language
//! (`flm_core::lang::rewrites::userdefs`) over a definition that declares a
//! default argument value, and a conditional written inside a conditional's
//! branch.
//!
//! A declared default is bound before the conditionals run, so an argument the
//! use site left out still has a value and is not blank; the tests below say
//! so at each family.

mod common;

use flm_core::lang::mint::{ArgDef, ArgStdKind};
use flm_core::lang::rewrites::userdefs::Definition;

use common::userdefs::{Setup, expanded, recomposed, setup_definitions};

fn with(definitions: Vec<Definition>) -> Setup {
    let setup = setup_definitions(definitions);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    setup
}

/// The text `input` expands to, the expansion reported clean.
fn text(setup: &Setup, input: &str) -> String {
    let (tree, diagnostics, _) = expanded(setup, input, 8);
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    recomposed(&tree)
}

/// `\greet[greeting]{name}` over `template`, the greeting defaulting to
/// `Hello`.
fn greet(template: &str) -> Definition {
    Definition::macro_("greet", vec![ArgDef::optional("greeting"), ArgDef::mandatory("name")])
        .text(template)
        .default("greeting", "Hello")
}

// --- the no-value family --------------------------------------------------------------------

// Ported from Python FLM's test/test_feature_substmacros.py::TestFeatureSubstMacros::test_subst_IfNoValueTF.
#[test]
fn if_no_value_takes_the_value_branch_for_an_argument_a_default_fills() {
    let setup = with(vec![greet(r"\IfNoValueTF{#1}{Hello, [#2]!}{#1, [#2].}")]);
    assert_eq!(text(&setup, r"\greet{Albert}"), "Hello, [Albert].");
    assert_eq!(text(&setup, r"\greet[Howdy]{Albert}"), "Howdy, [Albert].");
    assert_eq!(text(&setup, r"\greet[]{Albert}"), ", [Albert].");
}

// Ported from Python FLM's test/test_feature_substmacros.py::TestFeatureSubstMacros::test_subst_IfNoValueT_IfNoValueF.
#[test]
fn the_one_branch_no_value_forms_select_on_the_same_test() {
    let setup =
        with(vec![greet(r"Test: \IfNoValueF{#{greeting}}{#1: }\IfNoValueT{#1}{Hello, }[#2]!")]);
    assert_eq!(text(&setup, r"\greet{Albert}"), "Test: Hello: [Albert]!");
    assert_eq!(text(&setup, r"\greet[Howdy]{Albert}"), "Test: Howdy: [Albert]!");
}

// --- the value family, beside a star argument --------------------------------------------------

// Ported from Python FLM's test/test_feature_substmacros.py::TestFeatureSubstMacros::test_subst_IfValue.
#[test]
fn if_value_reads_the_default_of_one_argument_and_the_use_site_of_another() {
    let setup = with(vec![
        Definition::macro_(
            "greet",
            vec![ArgDef::new(ArgStdKind::Star), ArgDef::optional("greeting"), ArgDef::mandatory("name")],
        )
        .text(r"Test: \IfValueTF{#{greeting}}{G}{NOG} \IfValueT{#1}{T-G}-\IfValueF{#1}{F-NOG}")
        .default("greeting", "Hello"),
    ]);
    // `#1` is the star, written or not; the greeting has the default's value
    // whether or not the use site wrote one.
    assert_eq!(text(&setup, r"\greet*[Hi]{Albert}"), "Test: G T-G-");
    assert_eq!(text(&setup, r"\greet{Albert}"), "Test: G -F-NOG");
    assert_eq!(text(&setup, r"\greet*[]{Albert}"), "Test: G T-G-");
}

// --- the blank family ---------------------------------------------------------------------------

// Ported from Python FLM's test/test_feature_substmacros.py::TestFeatureSubstMacros::test_subst_ifblank_notblank.
#[test]
fn a_blank_test_reads_through_a_default_and_the_argument_written_at_the_use_site() {
    let setup = with(vec![greet(concat!(
        r"Test: \ifblank{#1}{1BLK}{1NOBLK:#1} \ifblank{#2}{2BLK}{2NOBLK:#2} ",
        r"\notblank{#1}{1NOBLK:#1}{1BLK} \notblank{#2}{2NOBLK:#2}{2BLK}"
    ))]);
    assert_eq!(
        text(&setup, r"\greet[Hi]{Albert}"),
        "Test: 1NOBLK:Hi 2NOBLK:Albert 1NOBLK:Hi 2NOBLK:Albert"
    );
    assert_eq!(
        text(&setup, r"\greet{Albert}"),
        "Test: 1NOBLK:Hello 2NOBLK:Albert 1NOBLK:Hello 2NOBLK:Albert"
    );
    assert_eq!(text(&setup, r"\greet{}"), "Test: 1NOBLK:Hello 2BLK 1NOBLK:Hello 2BLK");
}

// --- a conditional inside a conditional's branch -------------------------------------------------

// Ported from Python FLM's test/test_feature_substmacros.py::TestFeatureSubstMacros::test_subst_notblank_nested.
#[test]
fn a_conditional_inside_a_branch_is_evaluated_when_that_branch_is_selected() {
    let setup = with(vec![greet(concat!(
        r"Test: \notblank{#1}{\notblank{#2}{1NOBLK:#1,2NOBLK:#2}{1NOBLK:#1,2BLK}}",
        r"{\notblank{#2}{1BLK,2NOBLK:#2}{1BLK,2BLK}}"
    ))]);
    assert_eq!(text(&setup, r"\greet[Hi]{Albert}"), "Test: 1NOBLK:Hi,2NOBLK:Albert");
    assert_eq!(text(&setup, r"\greet[]{Albert}"), "Test: 1BLK,2NOBLK:Albert");
    assert_eq!(text(&setup, r"\greet[Hi]{}"), "Test: 1NOBLK:Hi,2BLK");
    assert_eq!(text(&setup, r"\greet[]{}"), "Test: 1BLK,2BLK");
}
