//! The argument layer of a user definition
//! (`flm_core::lang::rewrites::userdefs`): what an argument written empty
//! substitutes, how a default keyed by an argument's number reaches a named
//! argument, an argument beside an environment's body, and a specials taking
//! arguments.

mod common;

use flm_core::lang::mint::ArgDef;
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

/// `\greet[greeting]{name}`, the greeting defaulting to `Salut`, the default
/// keyed by the argument's declared name.
fn greet_by_name() -> Definition {
    Definition::macro_("greet", vec![ArgDef::optional("greeting"), ArgDef::mandatory("name")])
        .text("[#{greeting}], [#2]!")
        .default("greeting", "Salut")
}

// --- a written argument, empty or not --------------------------------------------------------

// Ported from Python FLM's test/test_feature_substmacros.py::TestFeatureSubstMacros::test_with_arguments_3.
#[test]
fn an_argument_written_empty_substitutes_as_nothing_and_not_as_its_default() {
    let setup = with(vec![greet_by_name()]);
    assert_eq!(text(&setup, r"\greet[]{Albert}"), "[], [Albert]!");
}

#[test]
fn an_argument_left_out_substitutes_its_default() {
    let setup = with(vec![greet_by_name()]);
    assert_eq!(text(&setup, r"\greet{Albert}"), "[Salut], [Albert]!");
}

// Ported from Python FLM's test/test_feature_substmacros.py::TestFeatureSubstMacros::test_with_arguments_4.
#[test]
fn a_default_keyed_by_an_arguments_number_reaches_the_named_argument() {
    let setup = with(vec![
        Definition::macro_("greet", vec![ArgDef::optional("greeting"), ArgDef::mandatory("name")])
            .text("[#1], [#2]!")
            .default("1", "Salut"),
    ]);
    assert_eq!(text(&setup, r"\greet{Albert}"), "[Salut], [Albert]!");
    assert_eq!(text(&setup, r"\greet[]{Albert}"), "[], [Albert]!");
}

// --- an argument beside an environment's body ------------------------------------------------

// Ported from Python FLM's test/test_feature_substmacros.py::TestFeatureSubstMacros::test_env_arg.
#[test]
fn an_environment_substitutes_an_argument_beside_its_body() {
    let setup = with(vec![
        Definition::environment("myenv", vec![ArgDef::mandatory("kind")])
            .text("[#1][[[#{body}]]]"),
    ]);
    assert_eq!(
        text(&setup, "Test \\begin{myenv}{ccc}Hello.\\end{myenv}"),
        "Test [ccc][[[Hello.]]]"
    );
}

// --- a specials taking arguments ---------------------------------------------------------------

// Ported from Python FLM's test/test_feature_substmacros.py::TestFeatureSubstMacros::test_specials_arg.
#[test]
fn a_specials_takes_arguments_and_their_defaults() {
    let setup = with(vec![
        Definition::specials("!", vec![ArgDef::optional("arg1"), ArgDef::mandatory("arg2")])
            .text("[exclamation mark, #{arg1}, #{arg2}]")
            .default("1", "|-|"),
    ]);
    assert_eq!(text(&setup, "Test!+ Hello."), "Test[exclamation mark, |-|, +] Hello.");
    assert_eq!(
        text(&setup, "Test![hi]{x} Hello."),
        "Test[exclamation mark, hi, x] Hello."
    );
}
