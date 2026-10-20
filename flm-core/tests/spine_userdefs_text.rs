//! User definitions through the whole pipeline to the text backend
//! (`flm_core::stdcore::features::userdefs`): what each kind of definition
//! prints, the markup a template carries, and the blocks an expansion standing
//! alone between two paragraphs leaves.

mod common;

use std::sync::Arc;

use flm_core::model::addr::TreeId;
use flm_core::model::element::element_of;
use flm_core::core::registry::FlmRegistry;
use flm_core::lang::mint::ArgDef;
use flm_core::lang::rewrites::userdefs::Definition;
use flm_core::stdcore::backends::pres;
use flm_core::stdcore::features::refs::UnresolvedRefs;
use flm_core::stdcore::features::userdefs::UserdefsConfig;
use flm_core::stdcore::StdcoreConfig;

use common::lowering::lowered;
use common::spine::{overrides, render, text_at};
use common::stdcore::registry;

/// The registry a host builds over the standard package with `definitions`.
fn registry_over(definitions: Vec<Definition>) -> Arc<FlmRegistry> {
    registry(StdcoreConfig::new().userdefs(UserdefsConfig::new().definitions(definitions)))
}

/// The text `document` prints as, never wrapped.
fn text(definitions: Vec<Definition>, document: &str) -> String {
    text_at(&registry_over(definitions), document, None)
}

// --- one definition of each kind, printed --------------------------------------------------

// Ported from Python FLM's test/test_feature_substmacros.py::TestSubstMacrosRenderers::test_simple_text_renderer.
#[test]
fn a_macro_prints_its_template() {
    let mymacro = Definition::macro_("mymacro", Vec::new()).text("[my macro content]");
    assert_eq!(text(vec![mymacro], r"Test \mymacro."), "Test [my macro content].\n");
}

// Ported from Python FLM's test/test_feature_substmacros.py::TestSubstMacrosRenderers::test_args_text_renderer.
#[test]
fn a_macro_prints_its_arguments_and_the_defaults_of_the_absent_ones() {
    let mymacro =
        Definition::macro_("mymacro", vec![ArgDef::optional("greeting"), ArgDef::mandatory("name")])
            .text(r"\textbf{#{greeting}}, \emph{#2}")
            .default("greeting", "Salut");
    assert_eq!(text(vec![mymacro], r"\mymacro{Albert}"), "Salut, Albert\n");
}

// Ported from Python FLM's test/test_feature_substmacros.py::TestSubstMacrosRenderers::test_specials_text_renderer.
#[test]
fn a_specials_prints_its_template_where_its_trigger_stood() {
    let at = Definition::specials("@", Vec::new()).text("[at-sign]");
    assert_eq!(text(vec![at], "user@domain"), "user[at-sign]domain\n");
}

// Ported from Python FLM's test/test_feature_substmacros.py::TestSubstMacrosRenderers::test_env_text_renderer.
#[test]
fn an_environment_prints_its_template_around_its_body() {
    let myenv = Definition::environment("myenv", Vec::new()).text("[[[#{body}]]]");
    assert_eq!(
        text(vec![myenv], "Test \\begin{myenv}Hello.\\end{myenv}"),
        "Test [[[Hello.]]]\n"
    );
}

// --- a template's own markup is live ---------------------------------------------------------

// Ported from Python FLM's test/test_feature_substmacros.py::TestFeatureSubstMacros::test_simple_wflm.
#[test]
fn a_templates_markup_is_live_where_the_use_site_stands() {
    let mymacro =
        Definition::macro_("mymacro", Vec::new()).text(r"[my \textit{macro} content]");
    assert_eq!(
        text(vec![mymacro], r"Test \textbf{macro content: \emph{a \mymacro{} b}}."),
        "Test macro content: a [my macro content] b.\n"
    );
}

// --- an expansion standing alone between two paragraphs ---------------------------------------

// Ported from Python FLM's test/test_feature_substmacros.py::TestFeatureSubstMacros::test_with_arguments_blck.
#[test]
fn an_expansion_standing_alone_between_two_paragraphs_is_a_paragraph_of_its_own() {
    let mymacro =
        Definition::macro_("mymacro", vec![ArgDef::optional("greeting"), ArgDef::mandatory("name")])
            .text(r"#1, \emph{#2}!")
            .default("greeting", "Hello");
    let document = "Block 1\n\n\\mymacro{Albert}\n\nBlock 3\n";
    let registry = registry_over(vec![mymacro]);
    assert_eq!(
        text_at(&registry, document, None),
        "Block 1\n\nHello, Albert!\n\nBlock 3\n"
    );
    let run = render(&registry, &[document], &overrides(None, UnresolvedRefs::Error));
    let paragraphs = lowered(&run, TreeId::tree(0))
        .tree
        .iter_storage_order()
        .filter(|node| element_of(node).name == pres::PAR)
        .count();
    assert_eq!(paragraphs, 3);
}
