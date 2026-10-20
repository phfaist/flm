//! The conditional family of the user-definition template language
//! (`flm_core::lang::rewrites::userdefs`), evaluated at each use site: the
//! three tests a template can write — provided, value, blank — their branch
//! forms, and their interplay with default argument values.

mod common;

use flm_core::lang::mint::{ArgDef, ArgStdKind};
use flm_core::lang::rewrites::userdefs::Definition;

use common::userdefs::{expanded, recomposed, setup_definitions, Setup};

/// `\opt[a]` with the three no-value/value forms.
fn opt() -> Definition {
    Definition::macro_("opt", vec![ArgDef::optional("a")]).text(concat!(
        "<\\IfNoValueTF{#1}{none}{some:#1}|\\IfValueT{#1}{V}",
        "|\\IfNoValueF{#1}{F}|\\IfValueF{#1}{nv}>"
    ))
}

fn with(definitions: Vec<Definition>) -> Setup {
    let setup = setup_definitions(definitions);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    setup
}

fn text(setup: &Setup, input: &str) -> String {
    let (tree, diagnostics, _) = expanded(setup, input, 8);
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    recomposed(&tree)
}

// --- no value / value ----------------------------------------------------------------------

#[test]
fn an_absent_optional_argument_has_no_value() {
    let setup = with(vec![opt()]);
    assert_eq!(text(&setup, r"\opt"), "<none|||nv>");
}

#[test]
fn a_provided_optional_argument_has_a_value() {
    let setup = with(vec![opt()]);
    assert_eq!(text(&setup, r"\opt[x]"), "<some:x|V|F|>");
}

#[test]
fn an_empty_optional_argument_still_has_a_value() {
    let setup = with(vec![opt()]);
    assert_eq!(text(&setup, r"\opt[]"), "<some:|V|F|>");
}

#[test]
fn an_absent_argument_with_a_default_has_a_value() {
    let setup = with(vec![
        Definition::macro_("d", vec![ArgDef::optional("a")])
            .text("\\IfNoValueTF{#1}{none}{some:#1}")
            .default("a", "dflt"),
    ]);
    assert_eq!(text(&setup, r"\d"), "some:dflt");
}

#[test]
fn a_literal_test_argument_counts_as_a_value() {
    let setup =
        with(vec![Definition::macro_("l", Vec::new()).text("\\IfNoValueTF{lit}{none}{some}")]);
    assert_eq!(text(&setup, r"\l"), "some");
}

// --- provided (the boolean family and its plain-named synonyms) ----------------------------

#[test]
fn a_provided_star_is_boolean_true() {
    let setup = with(vec![
        Definition::macro_("s", vec![ArgDef::new(ArgStdKind::Star)])
            .text("\\IfBooleanTF{#1}{star}{nostar}\\IfBooleanT{#1}{!}\\IfBooleanF{#1}{?}"),
    ]);
    assert_eq!(text(&setup, r"\s*"), "star!");
}

#[test]
fn an_absent_star_is_boolean_false() {
    let setup = with(vec![
        Definition::macro_("s", vec![ArgDef::new(ArgStdKind::Star)])
            .text("\\IfBooleanTF{#1}{star}{nostar}\\IfBooleanT{#1}{!}\\IfBooleanF{#1}{?}"),
    ]);
    assert_eq!(text(&setup, r"\s x"), "nostar?x");
}

/// The plain-named spelling of the provided test, over an optional argument.
fn provided() -> Definition {
    Definition::macro_("p", vec![ArgDef::optional("a")])
        .text("<\\IfProvidedTF{#1}{yes}{no}|\\IfProvidedT{#1}{T}|\\IfProvidedF{#1}{F}>")
}

#[test]
fn if_provided_is_true_when_the_use_site_wrote_the_argument() {
    let setup = with(vec![provided()]);
    assert_eq!(text(&setup, r"\p[x]"), "<yes|T|>");
}

#[test]
fn if_provided_is_false_when_the_argument_is_absent() {
    let setup = with(vec![provided()]);
    assert_eq!(text(&setup, r"\p"), "<no||F>");
}

#[test]
fn if_provided_is_false_although_a_default_gives_the_argument_a_value() {
    // A deliberate deviation from the Python implementation: the default is
    // bound first, so the value test holds where the provided test does not.
    let setup = with(vec![
        Definition::macro_("d", vec![ArgDef::optional("a")])
            .text("<\\IfProvidedTF{#1}{written}{unwritten}|\\IfValueTF{#1}{value}{novalue}>")
            .default("a", "dflt"),
    ]);
    assert_eq!(text(&setup, r"\d"), "<unwritten|value>");
}

#[test]
fn if_provided_and_if_boolean_are_the_same_test() {
    let setup = with(vec![
        Definition::macro_("s", vec![ArgDef::new(ArgStdKind::Star)])
            .text("\\IfProvidedTF{#1}{star}{nostar}\\IfBooleanTF{#1}{star}{nostar}"),
    ]);
    assert_eq!(text(&setup, r"\s*"), "starstar");
    assert_eq!(text(&setup, r"\s x"), "nostarnostarx");
}

// --- blank / not blank ---------------------------------------------------------------------

#[test]
fn a_whitespace_only_argument_is_not_blank() {
    // The Python implementation's rule, in `flm/feature/substmacros.py`: the
    // blankness test filters comments, not whitespace.
    let setup = with(vec![
        Definition::macro_("b", vec![ArgDef::mandatory("a")])
            .text("\\ifblank{#1}{blank}{full}/\\notblank{#1}{full}{blank}"),
    ]);
    assert_eq!(text(&setup, "\\b{ }"), "full/full");
}

#[test]
fn a_comment_only_argument_is_blank() {
    let setup = with(vec![
        Definition::macro_("b", vec![ArgDef::mandatory("a")])
            .text("\\ifblank{#1}{blank}{full}/\\notblank{#1}{full}{blank}"),
    ]);
    assert_eq!(text(&setup, "\\b{%%c\n}"), "blank/blank");
}

#[test]
fn an_empty_argument_is_blank() {
    let setup = with(vec![
        Definition::macro_("b", vec![ArgDef::mandatory("a")])
            .text("\\ifblank{#1}{blank}{full}"),
    ]);
    assert_eq!(text(&setup, r"\b{}"), "blank");
}

#[test]
fn a_literal_group_as_the_test_argument_is_not_blank() {
    // A superset over the Python implementation, which refuses any test
    // argument that is not a placeholder reference; here it is content, hence
    // not blank.
    let setup =
        with(vec![Definition::macro_("g", Vec::new()).text("\\ifblank{{}}{blank}{full}")]);
    assert_eq!(text(&setup, r"\g"), "full");
}

#[test]
fn an_argument_with_text_is_not_blank() {
    let setup = with(vec![
        Definition::macro_("b", vec![ArgDef::mandatory("a")])
            .text("\\ifblank{#1}{blank}{full}/\\notblank{#1}{full}{blank}"),
    ]);
    assert_eq!(text(&setup, r"\b{x}"), "full/full");
}

#[test]
fn an_absent_argument_is_blank() {
    let setup = with(vec![
        Definition::macro_("b", vec![ArgDef::optional("a")]).text("\\ifblank{#1}{blank}{full}"),
    ]);
    assert_eq!(text(&setup, r"\b"), "blank");
}

#[test]
fn a_blank_test_reads_through_a_default_value() {
    // The default is bound first, so the test sees the
    // default's content: a comment-only default is blank …
    let setup = with(vec![
        Definition::macro_("b", vec![ArgDef::optional("a")])
            .text("\\ifblank{#1}{blank}{full}")
            .default("a", "%%c\n"),
    ]);
    assert_eq!(text(&setup, r"\b"), "blank");
}

#[test]
fn a_whitespace_only_default_value_is_not_blank() {
    // … while a whitespace-only default is content, like whitespace written at
    // the use site.
    let setup = with(vec![
        Definition::macro_("b", vec![ArgDef::optional("a")])
            .text("\\ifblank{#1}{blank}{full}")
            .default("a", "  "),
    ]);
    assert_eq!(text(&setup, r"\b"), "full");
}

#[test]
fn a_blank_test_over_literal_text_beside_a_placeholder_is_not_blank() {
    let setup = with(vec![
        Definition::macro_("b", vec![ArgDef::optional("a")]).text("\\ifblank{x#1}{blank}{full}"),
    ]);
    assert_eq!(text(&setup, r"\b"), "full");
}

// --- the selected branch is itself substituted ---------------------------------------------

#[test]
fn a_selected_branch_substitutes_its_own_placeholders() {
    let setup = with(vec![
        Definition::macro_("p", vec![ArgDef::optional("a"), ArgDef::mandatory("b")])
            .text("\\IfValueTF{#1}{[#1:#2]}{[#2]}"),
    ]);
    assert_eq!(text(&setup, r"\p[x]{y}"), "[x:y]");
    assert_eq!(text(&setup, r"\p{y}"), "[y]");
}

#[test]
fn the_unselected_branch_leaves_nothing_behind() {
    let setup = with(vec![
        Definition::macro_("p", vec![ArgDef::optional("a")]).text("a\\IfValueT{#1}{X}b"),
    ]);
    assert_eq!(text(&setup, r"\p"), "ab");
}

#[test]
fn a_definition_used_in_a_selected_branch_expands() {
    let setup = with(vec![
        Definition::macro_("inner", Vec::new()).text("I"),
        Definition::macro_("p", vec![ArgDef::optional("a")]).text("\\IfValueTF{#1}{\\inner}{-}"),
    ]);
    assert_eq!(text(&setup, r"\p[x]"), "I");
}
