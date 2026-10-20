//! The user-definition expansion pass as a pure function, and the definition
//! shapes around it (`flm_core::lang::rewrites::userdefs`): expansion sets,
//! environments and specials, defaults, an uncompiled definition, block roles,
//! a third-party `Expand` hook over the facade, the guard refusal, and the
//! worker-stack contract.

mod common;

use std::sync::{Arc, OnceLock};
use std::thread;

use flm_core::model::aspects::{block_role_of, BlockRole};
use flm_core::model::element::{element_of, ElementSpec};
use flm_core::lang::mint::{body_def_of, ArgDef, BodyLayout, CallableKind, ElementDef, SyntaxDef};
use flm_core::lang::rewrites::userdefs::{
    compile_template, expand, BadDefinition, BadPlaceholder, CompiledTemplate, Definition,
    DefinitionProblem, Expand, ExpandCx, ExpandError, ExpandFailure, ExpandOptions, ExpandPolicy,
    ExpandSet, Expander, Expansion, PlaceholderBindings, PlaceholderProblem, PlaceholderTarget,
    PlaceholderValue, PlaceholderVocabulary, RecursionLimit, TemplateSpec, UserDefinition,
};
use flm_core::lang::{
    build_language, BuiltLang, Flm, FlmCallableType, FlmDiagnostics, GuardPolicy, LangInputs,
    NestingTooDeep,
};
use techy::core::node::{NodeRef, NodeSlice, NodeTree};
use techy::error::{Diagnostic, DiagnosticInfo, Severity};
use techy::latexlike::Mode;

use common::userdefs::{
    chars_of, expanded, expanded_set, options, recomposed, setup, setup_definitions, Setup,
    OWNER,
};
use common::{callables, element, parse};

// --- expansion sets ------------------------------------------------------------------------

fn policies() -> Vec<Definition> {
    vec![
        Definition::macro_("a", Vec::new()).text("A").expand(ExpandPolicy::Always),
        Definition::macro_("p", Vec::new()).text("P").expand(ExpandPolicy::PerPlan),
    ]
}

#[test]
fn the_always_set_expands_only_always_definitions() {
    let setup = setup_definitions(policies());
    let (tree, _, _) = expanded_set(&setup, r"\a\p", 8, ExpandSet::Always);
    assert_eq!(recomposed(&tree), "A\\p");
}

#[test]
fn the_per_plan_set_expands_only_per_plan_definitions() {
    let setup = setup_definitions(policies());
    let (tree, _, _) = expanded_set(&setup, r"\a\p", 8, ExpandSet::PerPlan);
    assert_eq!(recomposed(&tree), "\\aP");
}

#[test]
fn the_both_set_expands_every_definition() {
    let setup = setup_definitions(policies());
    let (tree, _, _) = expanded_set(&setup, r"\a\p", 8, ExpandSet::Both);
    assert_eq!(recomposed(&tree), "AP");
}

#[test]
fn a_kept_use_site_keeps_its_node_and_element() {
    let setup = setup_definitions(policies());
    let (tree, _, fragment) = expanded_set(&setup, r"\p", 8, ExpandSet::Always);
    let kept = tree.root().child(0).unwrap();
    let parsed = fragment.tree().root().child(0).unwrap();
    assert!(Arc::ptr_eq(element_of(&kept), element_of(&parsed)));
}

// --- environments and specials -------------------------------------------------------------

#[test]
fn an_environment_definition_substitutes_its_body() {
    let setup = setup_definitions(vec![
        Definition::environment("box", vec![ArgDef::optional("title")])
            .text("[#1|#{body}]"),
    ]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let (tree, diagnostics, _) = expanded(&setup, "\\begin{box}[T]content\\end{box}", 8);
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    assert_eq!(recomposed(&tree), "[T|content]");
}

#[test]
fn a_specials_definition_expands_at_its_trigger() {
    let setup = setup_definitions(vec![Definition::specials("~", Vec::new()).text("<nbsp>")]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let (tree, _, _) = expanded(&setup, "a~b", 8);
    assert_eq!(recomposed(&tree), "a<nbsp>b");
}

#[test]
fn an_environment_definitions_body_layout_follows_its_block_role() {
    let block =
        Definition::environment("blk", Vec::new()).text("#{body}").block_role(BlockRole::Block);
    let inline = Definition::environment("inl", Vec::new()).text("#{body}");
    let setup = setup_definitions(vec![block, inline]);
    let fragment = parse(&setup.lang, "\\begin{blk}x\\end{blk}\\begin{inl}y\\end{inl}");
    let nodes = callables(&fragment);
    assert_eq!(body_def_of(&nodes[0]).unwrap().layout, BodyLayout::Blocks);
    assert_eq!(body_def_of(&nodes[1]).unwrap().layout, BodyLayout::Inline);
}

#[test]
fn a_definitions_element_carries_the_declared_block_role() {
    let setup = setup_definitions(vec![
        Definition::macro_("blk", Vec::new()).text("B").block_role(BlockRole::Block),
    ]);
    let fragment = parse(&setup.lang, r"\blk");
    let node = callables(&fragment).into_iter().next().unwrap();
    assert_eq!(block_role_of(&node), BlockRole::Block);
}

#[test]
fn an_expanded_block_role_definition_still_splices_flat() {
    // The declared role governs a kept use site only (the structure pass reads
    // it there); an expanded one is replaced by its template's nodes, spliced
    // into the sibling run with no wrapper carrying the role.
    let setup = setup_definitions(vec![
        Definition::macro_("blk", Vec::new()).text("B").block_role(BlockRole::Block),
    ]);
    let (tree, _, _) = expanded(&setup, r"x \blk y", 8);
    assert_eq!(chars_of(&tree), ["x ", "B", "y"]);
}

// --- defaults ------------------------------------------------------------------------------

#[test]
fn an_absent_argument_takes_its_default_value() {
    let setup = setup_definitions(vec![
        Definition::macro_("g", vec![ArgDef::optional("who")])
            .text("hi #1")
            .default("who", "world"),
    ]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let (tree, _, _) = expanded(&setup, r"\g", 8);
    assert_eq!(recomposed(&tree), "hi world");
    let (tree, _, _) = expanded(&setup, r"\g[you]", 8);
    assert_eq!(recomposed(&tree), "hi you");
}

#[test]
fn a_default_value_may_refer_to_another_argument() {
    let setup = setup_definitions(vec![
        Definition::macro_("g", vec![ArgDef::optional("a"), ArgDef::mandatory("b")])
            .text("#1/#2")
            .default("1", "(#2)"),
    ]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let (tree, _, _) = expanded(&setup, r"\g{x}", 8);
    assert_eq!(recomposed(&tree), "(x)/x");
}

#[test]
fn a_numbered_default_key_follows_the_argument_number_offset() {
    // One numbering everywhere: under an offset of one, `#1` and the default
    // keyed "1" name the same argument — the second declared one.
    let setup = setup_definitions(vec![
        Definition::macro_("g", vec![ArgDef::optional("a"), ArgDef::optional("b")])
            .text("<#1>")
            .argument_number_offset(1)
            .default("1", "dflt"),
    ]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let (tree, _, _) = expanded(&setup, r"\g", 8);
    assert_eq!(recomposed(&tree), "<dflt>");
    let (tree, _, _) = expanded(&setup, r"\g[x][y]", 8);
    assert_eq!(recomposed(&tree), "<y>");
}

#[test]
fn a_named_default_key_names_the_same_argument_under_an_offset() {
    let setup = setup_definitions(vec![
        Definition::macro_("g", vec![ArgDef::optional("a"), ArgDef::optional("b")])
            .text("<#1>")
            .argument_number_offset(1)
            .default("b", "dflt"),
    ]);
    assert!(setup.build_diagnostics.is_empty(), "{:?}", setup.build_diagnostics);
    let (tree, _, _) = expanded(&setup, r"\g", 8);
    assert_eq!(recomposed(&tree), "<dflt>");
}

#[test]
fn a_default_values_nodes_chain_provenance_to_the_use_site() {
    let setup = setup_definitions(vec![
        Definition::macro_("g", vec![ArgDef::optional("a")]).text("#1").default("a", "dflt"),
    ]);
    let (tree, _, fragment) = expanded(&setup, r"\g", 8);
    let node = tree.root().child(0).unwrap();
    assert_eq!(node.chars(), Some("dflt"));
    let trigger = node.span().source().provenance().triggered_at().expect("triggered");
    assert!(Arc::ptr_eq(trigger.source(), fragment.source()));
    assert_eq!(trigger.content(), r"\g");
}

// --- an uncompiled definition -------------------------------------------------------------

#[test]
fn a_use_of_an_uncompiled_definition_recovers_as_literal_text() {
    let value = UserDefinition::new(Definition::macro_("u", Vec::new()).text("U"));
    let mut inputs = LangInputs::new();
    inputs.elements.push(value.element_def(OWNER, "u.u"));
    let lang = build_language(inputs);
    let fragment = parse(&lang, r"\u");
    let mut diagnostics = FlmDiagnostics::new();
    let tree = expand(
        fragment.tree(),
        &lang,
        &ExpandOptions { recursion_limit: 8, guard: lang.guard() },
        ExpandSet::Both,
        &mut diagnostics,
    )
    .unwrap();
    assert_eq!(recomposed(&tree), r"\u");
    let conditions: Vec<&BadDefinition> = diagnostics.conditions::<BadDefinition>().collect();
    assert_eq!(conditions.len(), 1);
    assert_eq!(conditions[0].problem, DefinitionProblem::NotCompiled);
}

// --- the declaration shapes ---------------------------------------------------------------

#[test]
fn the_element_declaration_of_a_specials_lists_no_names() {
    let value = UserDefinition::new(Definition::specials("~", Vec::new()).text("x"));
    let def = value.element_def(OWNER, "u.nbsp");
    assert!(def.names.is_empty());
    assert_eq!(def.syntax.kind(), Some(CallableKind::Specials));
}

#[test]
fn an_overriding_definition_declares_an_override() {
    let value =
        UserDefinition::new(Definition::macro_("emph", Vec::new()).content("x").overriding());
    let override_def = value.override_def(OWNER).expect("an override");
    assert_eq!(override_def.kind, CallableKind::Macro);
    assert_eq!(override_def.name, "emph");
    let plain = UserDefinition::new(Definition::macro_("emph", Vec::new()).text("x"));
    assert!(plain.override_def(OWNER).is_none());
}

#[test]
fn an_overriding_definition_replaces_another_features_definition() {
    let other = common::macro_def("t", "emph", 1);
    let setup = setup(
        // The replaced declaration is text-only; this one applies in both modes,
        // which covers it: an override narrower than what it replaces leaves the
        // rest standing.
        vec![Definition::macro_("emph", vec![ArgDef::mandatory("a")])
            .content("<#1>")
            .overriding()],
        vec![other],
    );
    let (tree, _, _) = expanded(&setup, r"\emph{x}", 8);
    assert_eq!(recomposed(&tree), "<x>");
}

#[test]
fn a_definition_finds_its_argument_by_name_or_number() {
    let definition = Definition::macro_("f", vec![ArgDef::optional("a"), ArgDef::mandatory("b")]);
    assert_eq!(definition.argument_index("b"), Some(1));
    assert_eq!(definition.argument_index("1"), Some(0));
    assert_eq!(definition.argument_index("3"), None);
    assert_eq!(definition.argument_index("0"), None);
}

// --- a third-party hook over the façade ------------------------------------------------------

/// An extension's own `Expand` implementation: a template with a hook-answered
/// placeholder family (`#{stamp}`, `#[…]{echo}`, `#[…]{quote}`) over the macro
/// `\st{a}[o]`; its compiled template, its stamp tree and the compiled default
/// value of the optional argument are filled in after the language is built, as
/// a feature's late-init hook would.
struct Stamped {
    template: OnceLock<CompiledTemplate>,
    /// The content `#{stamp}` stands for.
    stamp: OnceLock<Arc<NodeTree<Flm>>>,
    /// The default value of the optional argument `o` (`#2`), when a test gives one.
    default: OnceLock<CompiledTemplate>,
    echoes: Vec<String>,
}

struct StampBindings<'a> {
    hook: &'a Stamped,
    site: NodeRef<'a, Flm>,
}

impl PlaceholderBindings for StampBindings<'_> {
    fn value<'a>(
        &'a self,
        target: &PlaceholderTarget,
        substitution: Option<NodeSlice<'_, Flm>>,
    ) -> PlaceholderValue<'a> {
        let stamp = self.hook.stamp.get();
        match target {
            PlaceholderTarget::Extra(name) if name == "stamp" => match stamp {
                Some(stamp) => PlaceholderValue::Tree(stamp),
                None => PlaceholderValue::Unresolvable,
            },
            PlaceholderTarget::Extra(name) if name == "echo" => {
                // The substitution argument's nodes are the template's own.
                let echoed = substitution
                    .and_then(|nodes| nodes.source_text().map(String::from))
                    .is_some_and(|text| self.hook.echoes.contains(&text));
                match stamp {
                    Some(stamp) if echoed => PlaceholderValue::Tree(stamp),
                    _ => PlaceholderValue::Unresolvable,
                }
            }
            // The substitution argument itself, handed back as the value.
            PlaceholderTarget::Extra(name) if name == "quote" => PlaceholderValue::Substitution,
            PlaceholderTarget::Argument(index) => match self.site.argument_content_nodes(*index) {
                Some(content) => PlaceholderValue::Provided(content),
                None => match self.hook.default.get() {
                    Some(default) if *index == 1 => PlaceholderValue::Default(default),
                    _ => PlaceholderValue::NoValue,
                },
            },
            _ => PlaceholderValue::Unresolvable,
        }
    }
}

impl Expand for Stamped {
    fn expand(
        &self,
        node: NodeRef<'_, Flm>,
        cx: &mut ExpandCx<'_>,
        xp: &mut dyn Expander,
    ) -> Result<Expansion, ExpandError> {
        let template = self.template.get().ok_or_else(|| ExpandError::Hook {
            detail: String::from("not compiled"),
        })?;
        let bindings = StampBindings { hook: self, site: node };
        Ok(Expansion::Emit(xp.instantiate(cx, node, template, &bindings)?))
    }
}

/// What `#{stamp}` stands for in a test.
enum Stamp {
    /// A tree parsed from plain text (`STAMP`).
    Text,
    /// A compiled template's tree, holding a placeholder node (`P#1`).
    WithPlaceholder,
}

fn stamped_language(template: &str) -> (BuiltLang, Arc<Stamped>) {
    stamped_language_with(template, None, Stamp::Text)
}

/// The language over `\st` with the given template, the default value of its
/// optional argument (when given) and the content of `#{stamp}`.
fn stamped_language_with(
    template: &str,
    default: Option<&str>,
    stamp: Stamp,
) -> (BuiltLang, Arc<Stamped>) {
    let hook = Arc::new(Stamped {
        template: OnceLock::new(),
        stamp: OnceLock::new(),
        default: OnceLock::new(),
        echoes: vec![String::from("yes")],
    });
    let hook_dyn: Arc<dyn Expand> = hook.clone();
    let spec = Arc::new(ElementSpec::new("x.stamped").with::<dyn Expand>(hook_dyn));
    let mut inputs = LangInputs::new();
    inputs.elements.push(ElementDef::new(
        "x",
        ["st"],
        SyntaxDef::Macro { args: vec![ArgDef::mandatory("a"), ArgDef::optional("o")] },
        spec,
    ));
    let lang = build_language(inputs);
    assert!(lang.is_sound());
    // Late init, against the finished language: the compiled template, the
    // stamp tree and the default value, as a feature's late-init hook fills them.
    let vocabulary = PlaceholderVocabulary::new(2)
        .extra("stamp", false)
        .extra("echo", true)
        .extra("quote", true);
    let compiled = |text: &str, origin_label: &str| {
        compile_template(
            &TemplateSpec {
                text,
                mode: Mode::Text,
                vocabulary: &vocabulary,
                offset: 0,
                definition: "\\st",
                label: "expansion of \\st",
                origin_label,
                user_definition: true,
            },
            &lang,
        )
        .unwrap_or_else(|diagnostics| panic!("{diagnostics:?}"))
    };
    let stamp_tree = match stamp {
        Stamp::Text => Arc::new(parse(&lang, "STAMP").tree().annotate(|_| ())),
        Stamp::WithPlaceholder => Arc::clone(compiled("P#1", "stamp of \\st").tree()),
    };
    let _ = hook.stamp.set(stamp_tree);
    if let Some(text) = default {
        let _ = hook.default.set(compiled(text, "default value of #2 of \\st"));
    }
    let _ = hook.template.set(compiled(template, "template of \\st"));
    (lang, hook)
}

fn expand_with(lang: &BuiltLang, text: &str) -> (Arc<NodeTree<Flm>>, FlmDiagnostics) {
    let fragment = parse(lang, text);
    let mut diagnostics = FlmDiagnostics::new();
    let tree = expand(
        fragment.tree(),
        lang,
        &ExpandOptions { recursion_limit: 8, guard: lang.guard() },
        ExpandSet::Both,
        &mut diagnostics,
    )
    .unwrap();
    (tree, diagnostics)
}

#[test]
fn a_hook_answered_placeholder_splices_the_hooks_tree() {
    let (lang, _hook) = stamped_language("[#1:#{stamp}]");
    let (tree, diagnostics) = expand_with(&lang, r"\st{x}");
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    assert_eq!(recomposed(&tree), "[x:STAMP]");
}

#[test]
fn a_hook_receives_the_substitution_argument() {
    let (lang, _hook) = stamped_language("#[yes]{echo}");
    let (tree, diagnostics) = expand_with(&lang, r"\st{x}");
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    assert_eq!(recomposed(&tree), "STAMP");
}

#[test]
fn an_unresolvable_placeholder_recovers_the_use_site_as_literal_text() {
    let (lang, _hook) = stamped_language("#[no]{echo}");
    let (tree, diagnostics) = expand_with(&lang, r"a \st{x} b");
    assert_eq!(recomposed(&tree), r"a \st{x} b");
    let bad: Vec<&BadPlaceholder> = diagnostics.conditions::<BadPlaceholder>().collect();
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].reference, "echo");
    assert_eq!(bad[0].reason, PlaceholderProblem::Unresolvable);
}

#[test]
fn an_unresolvable_placeholder_inside_a_conditional_branch_recovers_the_use_site() {
    // The placeholder sits one driver level down (a selected branch is restaged
    // through the substitution pass): still the use site's recovery.
    let (lang, _hook) = stamped_language("\\IfValueTF{#1}{#[no]{echo}}{-}");
    let (tree, diagnostics) = expand_with(&lang, r"\st{x}");
    assert_eq!(recomposed(&tree), r"\st{x}");
    let bad: Vec<&BadPlaceholder> = diagnostics.conditions::<BadPlaceholder>().collect();
    assert_eq!(bad.len(), 1);
    assert_eq!(bad[0].reference, "echo");
}

#[test]
fn an_unresolvable_placeholder_inside_a_default_value_recovers_the_use_site() {
    let (lang, _hook) = stamped_language_with("[#2]", Some("#[no]{echo}"), Stamp::Text);
    let (tree, diagnostics) = expand_with(&lang, r"\st{x}");
    assert_eq!(recomposed(&tree), r"\st{x}");
    assert_eq!(diagnostics.conditions::<BadPlaceholder>().count(), 1);
}

#[test]
fn a_compiled_template_handed_back_by_a_hook_is_instantiated_in_place() {
    // `PlaceholderValue::Default` is how a hook has a compiled template's own
    // placeholders resolved (here the default value of the absent `#2`).
    let (lang, _hook) = stamped_language_with("[#2]", Some("d:#1"), Stamp::Text);
    let (tree, diagnostics) = expand_with(&lang, r"\st{x}");
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    assert_eq!(recomposed(&tree), "[d:x]");
}

#[test]
fn a_third_party_hook_is_bounded_by_the_facade() {
    // The hook checks nothing itself: the façade's depth bound recovers the
    // innermost use site — its literal text on the instance's own source — with
    // the typed condition, and the tree is there.
    let (lang, _hook) = stamped_language("<\\st{#1}>");
    let (tree, diagnostics) = expand_with(&lang, r"\st{x}");
    assert_eq!(recomposed(&tree), format!("{}\\st{{#1}}{}", "<".repeat(8), ">".repeat(8)));
    let limits: Vec<&RecursionLimit> = diagnostics.conditions::<RecursionLimit>().collect();
    assert_eq!(limits.len(), 1);
    assert_eq!(limits[0].name, "\\st");
    assert_eq!(limits[0].limit, 8);
}

#[test]
fn a_placeholder_node_in_a_hooks_tree_never_survives_the_pass() {
    // Hook content is finished content: a placeholder in it is bound to nothing,
    // so the use site recovers and no `Placeholder`-typed callable is left in the
    // expanded tree.
    let (lang, _hook) = stamped_language_with("#{stamp}", None, Stamp::WithPlaceholder);
    let (tree, diagnostics) = expand_with(&lang, r"\st{x}");
    let placeholders = tree
        .root()
        .descendants()
        .filter(|node| node.callable_type() == Some(FlmCallableType::Placeholder))
        .count();
    assert_eq!(placeholders, 0);
    assert_eq!(recomposed(&tree), r"\st{x}");
    assert_eq!(diagnostics.conditions::<BadPlaceholder>().count(), 1);
}

#[test]
fn a_substitution_argument_handed_back_as_the_value_carries_this_uses_source() {
    // The substitution nodes are the template's own, so they are re-sourced
    // like the rest of the template.
    let (lang, _hook) = stamped_language("#[yes]{quote}");
    let (tree, diagnostics) = expand_with(&lang, r"\st{x}");
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    assert_eq!(recomposed(&tree), "yes");
    let node = tree.root().child(0).unwrap();
    assert_eq!(node.span().source().origin().synthetic_label(), Some("expansion of \\st"));
    let trigger = node.span().source().provenance().triggered_at().expect("triggered");
    assert_eq!(trigger.content(), r"\st{x}");
}

// --- a hook that declines ---------------------------------------------------------------------

/// A condition an extension's hook records at a use site it keeps.
#[derive(Debug, Clone, PartialEq, Eq, DiagnosticInfo)]
#[diagnostic(id = "x.kept", message = "‘{name}’ kept as written")]
struct Kept {
    name: String,
}

/// An extension's hook that keeps every use site (`Expansion::Default`): the
/// pass carries the node over and descends into its children. With `record`
/// set, it reports `Kept` at the node first, through the façade.
struct Declining {
    record: bool,
}

impl Expand for Declining {
    fn expand(
        &self,
        node: NodeRef<'_, Flm>,
        _cx: &mut ExpandCx<'_>,
        xp: &mut dyn Expander,
    ) -> Result<Expansion, ExpandError> {
        if self.record {
            let name = node.name().unwrap_or("").to_string();
            xp.diagnose(Diagnostic::warning(Kept::new(name), node.span().clone()));
        }
        Ok(Expansion::Default)
    }
}

/// `\keep{a}` (the declining hook) beside the definition `\t` → `T`.
fn declining_setup(record: bool) -> Setup {
    let hook: Arc<dyn Expand> = Arc::new(Declining { record });
    let spec = Arc::new(ElementSpec::new("x.keep").with::<dyn Expand>(hook));
    let keep = ElementDef::new(
        "x",
        ["keep"],
        SyntaxDef::Macro { args: vec![ArgDef::mandatory("a")] },
        spec,
    );
    setup(vec![Definition::macro_("t", Vec::new()).text("T")], vec![keep])
}

#[test]
fn a_hook_that_declines_keeps_its_node_and_the_pass_descends_into_its_children() {
    let setup = declining_setup(false);
    let (tree, diagnostics, fragment) = expanded(&setup, r"\keep{\t}", 8);
    assert!(diagnostics.is_empty(), "{}", diagnostics.render_all());
    assert_eq!(recomposed(&tree), r"\keep{T}");
    let kept = tree.root().child(0).unwrap();
    let parsed = fragment.tree().root().child(0).unwrap();
    assert!(Arc::ptr_eq(element_of(&kept), element_of(&parsed)));
}

#[test]
fn a_hook_may_record_a_diagnostic_without_recovering_anything() {
    let setup = declining_setup(true);
    let (tree, diagnostics, _) = expanded(&setup, r"\keep{x}", 8);
    assert_eq!(recomposed(&tree), r"\keep{x}");
    let kept: Vec<&Kept> = diagnostics.conditions::<Kept>().collect();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].name, "keep");
    assert_eq!(diagnostics.iter().next().unwrap().severity(), Severity::Warning);
}

// --- the guard and the worker-stack contract ----------------------------------------------

#[test]
fn a_guard_refusal_is_the_typed_nesting_condition_and_no_tree() {
    let setup = setup_definitions(vec![
        Definition::macro_("t", Vec::new()).text("T"),
    ]);
    let fragment = parse(&setup.lang, "{{{{{{x}}}}}}");
    let mut diagnostics = FlmDiagnostics::new();
    let result = expand(
        fragment.tree(),
        &setup.lang,
        &ExpandOptions { recursion_limit: 8, guard: GuardPolicy::DepthLimit(3) },
        ExpandSet::Both,
        &mut diagnostics,
    );
    match result {
        Err(ExpandFailure::NestingTooDeep(diagnostic)) => {
            assert_eq!(diagnostic.identifier(), NestingTooDeep::IDENTIFIER);
        }
        other => panic!("expected the guard's refusal, got {other:?}"),
    }
}

#[test]
fn shallow_input_expands_under_the_depth_limit() {
    let setup = setup_definitions(vec![
        Definition::macro_("t", Vec::new()).text("T"),
    ]);
    let fragment = parse(&setup.lang, r"{\t}");
    let mut diagnostics = FlmDiagnostics::new();
    let tree = expand(
        fragment.tree(),
        &setup.lang,
        &ExpandOptions { recursion_limit: 8, guard: GuardPolicy::DepthLimit(16) },
        ExpandSet::Both,
        &mut diagnostics,
    )
    .unwrap();
    assert_eq!(recomposed(&tree), "{T}");
}

#[test]
fn a_deep_expansion_on_a_default_stack_thread_is_never_an_abort() {
    // 1 000 nested groups around a use site (below the parse's refusal depth in
    // both profiles — the language layer's guard tests record it), expanded from
    // a thread with Rust's default 2 MiB stack: the pass runs itself on a worker
    // thread, so the outcome is a tree or a typed refusal, never a crash.
    let setup = setup_definitions(vec![
        Definition::macro_("t", Vec::new()).text("T"),
    ]);
    let text = format!("{}\\t{}", "{".repeat(1_000), "}".repeat(1_000));
    let outcome = thread::spawn(move || {
        let fragment = parse(&setup.lang, &text);
        let mut diagnostics = FlmDiagnostics::new();
        let result =
            expand(fragment.tree(), &setup.lang, &options(&setup, 8), ExpandSet::Both, &mut diagnostics);
        match result {
            Ok(tree) => recomposed(&tree).contains('T'),
            Err(ExpandFailure::NestingTooDeep(_)) => true,
            Err(other) => panic!("unexpected failure: {other}"),
        }
    })
    .join()
    .expect("no abort");
    assert!(outcome);
}

#[test]
fn the_pass_runs_on_a_snippet_tree() {
    // A tree that is not a document's: parsed on its own, expanded on its own.
    let setup = setup_definitions(vec![
        Definition::macro_("t", Vec::new()).text("T"),
    ]);
    let snippet = parse(&setup.lang, r"\t\t");
    let mut diagnostics = FlmDiagnostics::new();
    let tree = expand(snippet.tree(), &setup.lang, &options(&setup, 8), ExpandSet::Both, &mut diagnostics)
        .unwrap();
    assert_eq!(recomposed(&tree), "TT");
}

#[test]
fn expanding_twice_gives_equal_results() {
    let setup = setup_definitions(vec![
        Definition::macro_("t", vec![ArgDef::mandatory("a")]).text("<#1>"),
    ]);
    let (first, _, _) = expanded(&setup, r"\t{x}\t{y}", 8);
    let (second, _, _) = expanded(&setup, r"\t{x}\t{y}", 8);
    assert_eq!(recomposed(&first), recomposed(&second));
    assert_eq!(first.node_count(), second.node_count());
}

/// A language-level sanity check of the element the feature registers: the
/// element value is the definition, reachable by downcast.
#[test]
fn the_elements_value_is_the_definition() {
    let value = UserDefinition::new(Definition::macro_("t", Vec::new()).text("T"));
    let spec = value.element("u.t");
    let held = spec.value_as::<UserDefinition>().expect("the definition as the value");
    assert_eq!(held.definition().name, "t");
    assert!(spec.has_aspect::<dyn Expand>());
    let _ = element("unused");
}
