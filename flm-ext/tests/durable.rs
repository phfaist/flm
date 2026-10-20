//! Durable handles (plan E4, section 6.2, design (A)): the template language
//! and the compiled templates a feature's handlers keep in the adapter's
//! durable table, one per feature per registration; and the templates a
//! rule instantiates (`instantiate`, `ValueBindings`, `DefinitionBindings`)
//! and splices. Through the Rust façades and through the JSON double.

mod common;

use std::sync::{Arc, Mutex};

use flm_core::core::registry::PackageEntry;
use flm_core::stdcore::backends::pres;
use flm_ext::adapter::durable::{DurableKind, DurableTable, DurableToken};
use flm_ext::api::core::lowering::{DefaultMark, Lower, LowerCx, Lowerer, Lowering, LoweringDecl, LoweringRule};
use flm_ext::api::lang::mint::{ArgDef, ElementDef, Mode, SyntaxDef};
use flm_ext::api::lang::rewrites::userdefs::{
    compile_template_with, instantiate, Definition, DefinitionBindings, PlaceholderVocabulary,
    TemplateLanguage, TemplateSpec, UserDefinition, ValueBindings,
};
use flm_ext::api::model::element::ElementSpec;
use flm_ext::api::techy::node::Node;
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::double::Double;
use common::json_double::{module, JsonDouble};
use common::{registry_with, text_of};

/// The function of a rule's body.
type BodyFn = dyn Fn(&dyn Node<'_>, &mut dyn LowerCx, &mut dyn Lowerer) -> Result<Lowering, ExtError> + Send + Sync;

/// A rule's body written as a closure.
struct LowerFn(Box<BodyFn>);

impl Lower for LowerFn {
    fn lower<'c>(&self, node: &'c dyn Node<'c>, cx: &'c mut dyn LowerCx, lw: &'c mut dyn Lowerer) -> Result<Lowering, ExtError> {
        (self.0)(node, cx, lw)
    }
}

/// The body `f`.
fn body(
    f: impl Fn(&dyn Node<'_>, &mut dyn LowerCx, &mut dyn Lowerer) -> Result<Lowering, ExtError> + Send + Sync + 'static,
) -> Arc<dyn Lower> {
    Arc::new(LowerFn(Box::new(f)))
}

/// The package `ex`, whose feature `titles` declares `\extitle{}` (the
/// element `ex.title`) and its rule, of body `body`.
fn titles(body: Arc<dyn Lower>) -> PackageEntry {
    Double::new("ex")
        .feature("titles", move |b| {
            b.element(ElementDef::new(
                b.feature(),
                ["extitle"],
                SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] },
                ElementSpec::new("ex.title"),
            ))?;
            b.lowering_rule(LoweringRule::new(
                "ex.title.lower.default",
                "ex.title",
                LoweringDecl::new().produces(pres::SPAN),
                Arc::clone(&body),
                DefaultMark::Everywhere,
            ))
        })
        .package()
}

/// A text-mode template of `text`, with the extra placeholder `number`.
fn spec(text: &str) -> TemplateSpec {
    TemplateSpec {
        text: text.to_string(),
        mode: Mode::Text,
        vocabulary: PlaceholderVocabulary::new(0).extra("number", false),
        definition: "the title of an exercise".to_string(),
        label: "title of an exercise".to_string(),
        origin_label: "the title format of ex".to_string(),
        user_definition: false,
        offset: 0,
    }
}

/// The title `Exercise #{number}` with the number `3`, instantiated at
/// `node` and spliced: the number is itself a template instantiated with no
/// value, whose root node the bindings take. Answers the title template.
fn splice_title(node: &dyn Node<'_>, cx: &mut dyn LowerCx, lw: &mut dyn Lowerer) -> Result<(Vec<flm_ext::api::core::lowering::Staged>, DurableToken), ExtError> {
    let (title, number) = {
        let lang = lw.lang();
        let language = TemplateLanguage::new(&*lang)?;
        (compile_template_with(&spec("Exercise #{number}"), &*lang, &language)?, compile_template_with(&spec("3"), &*lang, &language)?)
    };
    let number = instantiate(node, &number, &ValueBindings::new())?;
    let bindings = ValueBindings::new().with("number", &*number)?;
    let tree = instantiate(node, &title, &bindings)?;
    Ok((lw.splice(cx, &*tree)?, title.token()))
}

#[test]
fn two_compiles_of_one_specification_answer_one_entry_and_the_language_is_built_once() {
    static SEEN: Mutex<Vec<(DurableToken, DurableToken, DurableToken, DurableToken)>> = Mutex::new(Vec::new());
    let rule = body(|_node, _cx, lw| {
        let lang = lw.lang();
        let language = TemplateLanguage::new(&*lang)?;
        let again = TemplateLanguage::new(&*lang)?;
        let first = compile_template_with(&spec("Exercise #{number}"), &*lang, &language)?;
        let second = compile_template_with(&spec("Exercise #{number}"), &*lang, &again)?;
        SEEN.lock().expect("the lock").push((language.token(), again.token(), first.token(), second.token()));
        Ok(Lowering::Emit(Vec::new()))
    });
    let registry = registry_with([titles(rule)]);
    let (_, diagnostics) = text_of(&registry, "\\extitle{} and \\extitle{}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let seen = SEEN.lock().expect("the lock").clone();
    assert_eq!(seen.len(), 2, "two calls");
    for (language, again, first, second) in &seen {
        assert_eq!(language, again, "one template language per table");
        assert_eq!(language.kind, DurableKind::TemplateLanguage);
        assert_eq!(first, second, "one entry for two compiles of one specification");
        assert_eq!(first.kind, DurableKind::CompiledTemplate);
        assert_eq!(first.table, language.table);
    }
    assert_eq!(seen[0], seen[1], "the second call answers the first call's tokens");
}

#[test]
fn a_rule_splices_the_root_node_instantiate_answers() {
    let registry = registry_with([titles(body(|node, cx, lw| Ok(Lowering::Emit(splice_title(node, cx, lw)?.0))))]);
    let (text, diagnostics) = text_of(&registry, "A \\extitle{} b.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A Exercise 3 b.\n");
}

#[test]
fn a_token_kept_in_module_scope_resolves_in_a_later_call_and_is_stale_under_another_registry() {
    static KEPT: Mutex<Option<DurableToken>> = Mutex::new(None);
    static FAILED: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let package = || {
        titles(body(|node, cx, lw| {
            let kept = *KEPT.lock().expect("the lock");
            match kept {
                None => {
                    let (staged, token) = splice_title(node, cx, lw)?;
                    *KEPT.lock().expect("the lock") = Some(token);
                    Ok(Lowering::Emit(staged))
                }
                Some(token) => match lw.durable_template(token) {
                    Ok(template) => {
                        let number = {
                            let lang = lw.lang();
                            let language = TemplateLanguage::new(&*lang)?;
                            compile_template_with(&spec("4"), &*lang, &language)?
                        };
                        let number = instantiate(node, &number, &ValueBindings::new())?;
                        let tree = instantiate(node, &template, &ValueBindings::new().with("number", &*number)?)?;
                        Ok(Lowering::Emit(lw.splice(cx, &*tree)?))
                    }
                    Err(error) => {
                        FAILED.lock().expect("the lock").push(error.to_string());
                        Err(error)
                    }
                },
            }
        }))
    };
    // The first call compiles and keeps the token; the second resolves it.
    let first = registry_with([package()]);
    let (text, diagnostics) = text_of(&first, "\\extitle{} and \\extitle{}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Exercise 3 and Exercise 4.\n");
    // Under another registry, built again through `ExtFeature::register`, the
    // kept token names a table of the first registry.
    let second = registry_with([package()]);
    let (_, diagnostics) = text_of(&second, "\\extitle{}.");
    let failed = FAILED.lock().expect("the lock").clone();
    assert_eq!(
        failed,
        ["the `compiled_template` object 1 must be one the feature `titles` of the package `_` of the extension `ex` made \
          under the registry in use"]
    );
    assert!(diagnostics.iter().any(|text| text.contains("must be one the feature `titles`")), "{diagnostics:?}");
}

#[test]
fn an_index_the_table_does_not_hold_and_an_entry_of_another_kind_are_stale() {
    static ANSWERS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
    let rule = body(|_node, _cx, lw| {
        let token = {
            let lang = lw.lang();
            let language = TemplateLanguage::new(&*lang)?;
            compile_template_with(&spec("x"), &*lang, &language)?.token()
        };
        let past = lw.durable_template(DurableToken { index: 99, ..token }).err().expect("stale");
        let other = lw.durable_template(DurableToken { kind: DurableKind::TemplateLanguage, ..token }).err().expect("stale");
        assert!(matches!(past, ExtError::StaleDurable { .. }));
        ANSWERS.lock().expect("the lock").push((past.to_string(), other.to_string()));
        Ok(Lowering::Emit(Vec::new()))
    });
    let registry = registry_with([titles(rule)]);
    let _ = text_of(&registry, "\\extitle{}");
    let answers = ANSWERS.lock().expect("the lock").clone();
    assert_eq!(answers.len(), 1);
    assert_eq!(
        answers[0].0,
        "the `compiled_template` object 99 must be one the feature `titles` of the package `_` of the extension `ex` made \
         under the registry in use"
    );
    assert!(answers[0].1.starts_with("the `template_language` object 1 must be one"), "{}", answers[0].1);
}

#[test]
fn a_token_resolves_on_two_threads_at_once() {
    let registry = registry_with([]);
    let table = DurableTable::new("ex", "_", "titles");
    let vocabulary = PlaceholderVocabulary::new(0).extra("number", false);
    let spec = flm_core::lang::rewrites::userdefs::TemplateSpec {
        text: "Exercise #{number}",
        mode: Mode::Text,
        vocabulary: &vocabulary,
        offset: 0,
        definition: "the title of an exercise",
        label: "title of an exercise",
        origin_label: "title of an exercise",
        user_definition: false,
    };
    let (token, _) = table.template(&spec, registry.lang()).expect("the template compiles");
    let tokens: Vec<DurableToken> = std::thread::scope(|scope| {
        let threads: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    let again = table.template(&spec, registry.lang()).expect("kept").0;
                    table.compiled_template(token).expect("valid on this thread").token();
                    again
                })
            })
            .collect();
        threads.into_iter().map(|thread| thread.join().expect("the thread finishes")).collect()
    });
    assert_eq!(tokens, [token, token]);
    assert_eq!(table.len(), 2, "the language and one template");
}

#[test]
fn a_rule_instantiates_a_variant_s_template_with_the_use_site_s_bindings() {
    // `\exboxed{text}`, whose template is `[#1]`, registered so that its
    // templates are compiled; the rule of `\extitle{content}` instantiates
    // that template with the bindings of its own use site's argument.
    let definition = UserDefinition::new(Definition::macro_("exboxed", vec![ArgDef::mandatory("text")]).content("[#1]"));
    let held = definition.clone();
    let rule = body(move |node, cx, lw| {
        let variant = held.variant_for_use(node)?;
        let bindings = DefinitionBindings::new(node, &*variant);
        let tree = instantiate(node, &*variant.template(), &*bindings)?;
        Ok(Lowering::Emit(lw.splice(cx, &*tree)?))
    });
    let package = Double::new("ex")
        .feature("titles", move |b| {
            b.lowering_rule(flm_ext::api::stdcore::features::userdefs::expand_in_tree_rule_for("ex.boxed", definition.definition()))?;
            b.element(definition.element_def(&b.feature(), "ex.boxed"))?;
            b.element(ElementDef::new(
                b.feature(),
                ["extitle"],
                SyntaxDef::Macro { args: vec![ArgDef::mandatory("content")] },
                ElementSpec::new("ex.title"),
            ))?;
            b.lowering_rule(LoweringRule::new(
                "ex.title.lower.default",
                "ex.title",
                LoweringDecl::new().produces(pres::SPAN),
                Arc::clone(&rule),
                DefaultMark::Everywhere,
            ))
        })
        .package();
    let registry = registry_with([package]);
    let (text, diagnostics) = text_of(&registry, "A \\extitle{x} b.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A [x] b.\n");
}

#[test]
fn value_bindings_take_the_root_node_of_a_tree_alone() {
    static REFUSED: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let rule = body(|node, _cx, _lw| {
        let refused = ValueBindings::new().with("number", node).expect_err("a node of the document");
        REFUSED.lock().expect("the lock").push(refused.to_string());
        Ok(Lowering::Emit(Vec::new()))
    });
    let registry = registry_with([titles(rule)]);
    let _ = text_of(&registry, "\\extitle{}");
    assert_eq!(*REFUSED.lock().expect("the lock"), ["the value of the placeholder `number` must be the root node of a tree"]);
}

// --- through the dispatcher ------------------------------------------------------------------

/// The package `ex` through the JSON double: `\extitle{}` and a rule whose
/// body is `lower`, given the handles of the node, the context and the
/// lowerer.
fn json_titles(
    lower: impl for<'t, 'x> Fn(&mut common::json_double::Js<'t, 'x>, &Value, &Value, &Value) -> Result<Value, ExtError>
    + Send
    + Sync
    + Clone
    + 'static,
) -> PackageEntry {
    JsonDouble::new("ex")
        .feature("titles", move |js, b| {
            js.call(
                b,
                "element",
                json!([{
                    "owner": "titles",
                    "names": ["extitle"],
                    "syntax": { "macro": { "args": [{ "name": "content", "code": "{" }] } },
                    "element": { "name": "ex.title" },
                }]),
            )?;
            let lower = lower.clone();
            let lower = js.lend_lower(move |js, node, cx, lw| lower(js, node, cx, lw));
            let decl = json!({ "produces": ["flm.pres.span"] });
            let rule = js.call(&module("core/lowering"), "LoweringRule::new", json!(["ex.title.lower.default", "ex.title", decl, lower, "everywhere"]))?;
            js.call(b, "lowering_rule", json!([rule]))?;
            Ok(())
        })
        .package()
}

/// A template specification in its data form.
fn spec_value(text: &str) -> Value {
    json!({
        "text": text, "mode": "text",
        "vocabulary": { "arity": 0, "extras": [{ "name": "number", "substitution": false }] },
        "definition": "the title of an exercise", "label": "title of an exercise",
        "origin_label": "the title format of ex",
    })
}

#[test]
fn the_dispatcher_compiles_keeps_and_instantiates_a_template() {
    static KEPT: Mutex<Option<Value>> = Mutex::new(None);
    let registry = registry_with([json_titles(|js, node, cx, lw| {
        let userdefs = module("lang/rewrites/userdefs");
        let lang = js.call(lw, "lang", json!([]))?;
        assert_eq!(lang["@handle"]["kind"], json!("built_lang"));
        let language = js.call(&userdefs, "TemplateLanguage::new", json!([lang]))?;
        assert_eq!(language["@durable"]["kind"], json!("template_language"));
        let kept = KEPT.lock().expect("the lock").clone();
        let title = match kept {
            // The token kept from the first call, passed as it was answered.
            Some(token) => token,
            None => {
                let title = js.call(&userdefs, "compile_template_with", json!([spec_value("Exercise #{number}"), lang, language]))?;
                let again = js.call(&userdefs, "compile_template_with", json!([spec_value("Exercise #{number}"), lang, language]))?;
                assert_eq!(title, again, "one token for one specification");
                *KEPT.lock().expect("the lock") = Some(title.clone());
                title
            }
        };
        assert_eq!(title["@durable"]["kind"], json!("compiled_template"));
        let number = js.call(&userdefs, "compile_template_with", json!([spec_value("3"), lang, language]))?;
        let empty = js.call(&userdefs, "ValueBindings::new", json!([]))?;
        let number = js.call(&userdefs, "instantiate", json!([node, number, empty]))?;
        let bindings = js.call(&empty, "with", json!(["number", number]))?;
        assert_eq!(bindings["@handle"]["kind"], json!("value_bindings"));
        let tree = js.call(&userdefs, "instantiate", json!([node, title, bindings]))?;
        let staged = js.call(lw, "splice", json!([cx, tree]))?;
        Ok(json!({ "emit": staged }))
    })]);
    let (text, diagnostics) = text_of(&registry, "\\extitle{} and \\extitle{}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Exercise 3 and Exercise 3.\n");
}

#[test]
fn the_dispatcher_refuses_a_stale_token_a_bad_specification_and_a_node_of_the_document() {
    static ANSWERS: Mutex<Vec<String>> = Mutex::new(Vec::new());
    let registry = registry_with([json_titles(|js, node, _cx, lw| {
        let userdefs = module("lang/rewrites/userdefs");
        let lang = js.call(lw, "lang", json!([]))?;
        let language = js.call(&userdefs, "TemplateLanguage::new", json!([lang]))?;
        let title = js.call(&userdefs, "compile_template_with", json!([spec_value("x"), lang, language]))?;
        let mut stale = title.clone();
        stale["@durable"]["index"] = json!(99);
        let empty = js.call(&userdefs, "ValueBindings::new", json!([]))?;
        let mut answers = ANSWERS.lock().expect("the lock");
        let mut push = |answer: Result<Value, ExtError>| answers.push(answer.err().map(|error| error.to_string()).unwrap_or_default());
        push(js.call(&userdefs, "instantiate", json!([node, stale, empty])));
        push(js.call(&userdefs, "compile_template_with", json!([{ "text": "x" }, lang, language])));
        push(js.call(&userdefs, "compile_template_with", json!([spec_value("#{other}"), lang, language])));
        push(js.call(&empty, "with", json!(["number", node])));
        Ok(json!("default"))
    })]);
    let _ = text_of(&registry, "\\extitle{}");
    let answers = ANSWERS.lock().expect("the lock").clone();
    assert_eq!(
        answers[0],
        "the `compiled_template` object 99 must be one the feature `titles` of the package `_` of the extension `ex` made \
         under the registry in use"
    );
    assert_eq!(answers[1], "`compile_template_with`: a template specification must have the field `mode`");
    assert!(answers[2].contains("other"), "{}", answers[2]);
    assert_eq!(answers[3], "`with`: the value of the placeholder `number` must be the root node of a tree");
}
