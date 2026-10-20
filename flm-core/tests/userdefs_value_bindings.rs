//! Instantiating a compiled template outside the expand pass
//! (`flm_core::lang::rewrites::userdefs::instantiate`) with values a feature
//! computed (`ValueBindings`): a held name gives its tree, a name not held has
//! no value, and an argument or the body cannot be resolved.

mod common;

use std::sync::Arc;

use flm_core::lang::flm::Flm;
use flm_core::lang::guard::{on_worker_thread, GuardPolicy};
use flm_core::lang::rewrites::userdefs::{
    compile_template_with, instantiate, CompiledTemplate, ExpandError, PlaceholderBindings,
    PlaceholderTarget, PlaceholderValue, PlaceholderVocabulary, TemplateLanguage, TemplateSpec,
    ValueBindings,
};
use flm_core::lang::{build_language, BuiltLang, LangInputs};
use techy::core::node::NodeTree;
use techy::latexlike::Mode;

/// The language of the tests: no element of its own.
fn language() -> BuiltLang {
    let lang = build_language(LangInputs::new());
    assert!(lang.is_sound(), "{:?}", lang.errors());
    lang
}

/// `text` compiled in text mode against `lang`, with the arguments and extra
/// names of `vocabulary`.
fn compiled(lang: &BuiltLang, text: &str, vocabulary: &PlaceholderVocabulary) -> CompiledTemplate {
    let language = TemplateLanguage::new(lang).expect("the template language mints");
    let spec = TemplateSpec {
        text,
        mode: Mode::Text,
        vocabulary,
        offset: 0,
        definition: "test",
        label: "a template of the test",
        origin_label: "a template of the test",
        user_definition: false,
    };
    compile_template_with(&spec, lang, &language).unwrap_or_else(|errors| panic!("{text}: {errors:?}"))
}

/// The text of a tree: its characters nodes, concatenated.
fn text_of(tree: &NodeTree<Flm>) -> String {
    tree.root().descendants().filter_map(|node| node.chars()).collect()
}

/// `template` instantiated with `bindings` on a worker thread, at the site of
/// the template's own root, under the stack budget.
fn instantiated(template: &CompiledTemplate, bindings: &ValueBindings) -> Result<NodeTree<Flm>, ExpandError> {
    let site = template.tree().root().span().clone();
    on_worker_thread(|| instantiate(&site, template, bindings, GuardPolicy::StackBudget))
        .expect("the worker thread starts")
}

#[test]
fn a_held_name_gives_its_tree_and_a_name_not_held_has_no_value() {
    let lang = language();
    let number = Arc::clone(common::parse(&lang, "3.2").tree());
    let vocabulary = PlaceholderVocabulary::new(0).extra("number", false).extra("title", false);
    let template = compiled(&lang, r"No. #{number}\IfNoValueTF{#{title}}{ (untitled)}{ (titled)}", &vocabulary);
    let bindings = ValueBindings::new().with("number", number);
    let instance = instantiated(&template, &bindings).expect("the template instantiates");
    assert_eq!(text_of(&instance), "No. 3.2 (untitled)");
}

#[test]
fn the_bindings_answer_by_target() {
    let lang = language();
    let tree = Arc::clone(common::parse(&lang, "x").tree());
    let bindings = ValueBindings::new().with("number", tree);
    let extra = |name: &str| PlaceholderTarget::Extra(name.to_string());
    assert!(matches!(bindings.value(&extra("number"), None), PlaceholderValue::Tree(_)));
    assert!(matches!(bindings.value(&extra("title"), None), PlaceholderValue::NoValue));
    assert!(matches!(bindings.value(&PlaceholderTarget::Argument(0), None), PlaceholderValue::Unresolvable));
    assert!(matches!(bindings.value(&PlaceholderTarget::Body, None), PlaceholderValue::Unresolvable));
}

#[test]
fn an_argument_placeholder_cannot_be_resolved() {
    let lang = language();
    let number = Arc::clone(common::parse(&lang, "3").tree());
    let vocabulary = PlaceholderVocabulary::new(1).extra("number", false);
    let template = compiled(&lang, "#{number}: #1", &vocabulary);
    let error = instantiated(&template, &ValueBindings::new().with("number", number))
        .expect_err("`#1` has no value");
    assert_eq!(error.bad_placeholder(), Some("1"), "{error:?}");
}

#[test]
fn a_name_given_twice_keeps_the_later_tree() {
    let lang = language();
    let first = Arc::clone(common::parse(&lang, "1").tree());
    let second = Arc::clone(common::parse(&lang, "2").tree());
    let vocabulary = PlaceholderVocabulary::new(0).extra("number", false);
    let template = compiled(&lang, "#{number}", &vocabulary);
    let bindings = ValueBindings::new().with("number", first).with("number", second);
    assert_eq!(text_of(&instantiated(&template, &bindings).unwrap()), "2");
}
