//! Registration and the factory (plan A, WP-2): a scripted package described
//! by the double becomes a package factory; its features declare elements,
//! fallbacks, overrides, tables, stages, rules and service requirements; the
//! name rule and the digest hold; the definitions' templates compile when the
//! registry is built.

mod common;

use std::sync::Arc;

use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::rewrites::userdefs::Expand as CoreExpand;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::FragmentLayout;
use flm_core::stdcore;
use flm_ext::adapter::table::ExtTable;
use flm_ext::api::core::registry::CallableKind;
use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_ext::api::lang::mint::{ArgDef, ElementDef, FallbackDef, FallbackKind, Mode, SyntaxDef};
use flm_ext::api::lang::rewrites::userdefs::{Definition, ExpandPolicy, Expansion, UserDefinition};
use flm_ext::api::model::element::ElementSpec;
use flm_ext::api::stdcore::features::userdefs::{element_name_under, expand_in_tree_rule_for};
use flm_ext::{ExtError, PackageReport};
use serde_json::Value;

use common::double::{expand, stage, Double};
use common::{registry_with, try_registry_with};

/// The text of the error of a registry build holding the standard package and
/// `double`'s package; the build must fail.
fn build_error(double: Double) -> String {
    try_registry_with([double.package()]).expect_err("the build fails").to_string()
}

/// The element names of the nodes of `text` parsed by `registry`'s language,
/// in document order.
fn element_names(registry: &flm_core::core::registry::FlmRegistry, text: &str) -> Vec<String> {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("a parse");
    fragment.tree().root().descendants().map(|node| node.ext().element.name.to_string()).collect()
}

// --- the factory -------------------------------------------------------------------------

#[test]
fn the_factory_lists_the_package_its_features_and_what_it_uses() {
    let factory = Double::new("wc").feature("count", |_| Ok(())).feature("check", |_| Ok(())).uses("other").factory();
    assert_eq!(factory.name().as_str(), "wc#_");
    let features: Vec<&str> = factory.features().iter().map(|f| f.name.as_str()).collect();
    assert_eq!(features, ["count", "check"]);
    assert!(factory.features().iter().all(|f| f.settings.is_none()));
    assert_eq!(factory.used().iter().map(|n| n.as_str()).collect::<Vec<_>>(), ["other"]);
}

#[test]
fn a_feature_switched_off_is_left_out() {
    let factory = Double::new("wc").feature("count", |_| Ok(())).feature("check", |_| Ok(())).factory();
    let mut settings = PackageSettings::new();
    settings.switch_off("check");
    let entry = factory.build(&settings).expect("the package");
    assert_eq!(entry.name.as_str(), "wc#_");
    let features: Vec<String> = entry.features.iter().map(|f| f.info().name.to_string()).collect();
    assert_eq!(features, ["count"]);
}

#[test]
fn a_report_with_a_feature_without_a_name_is_refused() {
    let double = Double::new("wc").feature("count", |_| Ok(())).describe_with(|d| d.features[0].name = String::new());
    let errors = double.try_factory().expect_err("an empty name");
    assert_eq!(
        errors.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["every feature of the package `_` of the extension `wc` must have a name"]
    );
}

#[test]
fn a_report_with_two_features_of_one_name_is_refused() {
    let double = Double::new("wc").feature("count", |_| Ok(())).describe_with(|d: &mut PackageReport| {
        let again = d.features[0].clone();
        d.features.push(again);
    });
    let errors = double.try_factory().expect_err("a duplicate");
    assert_eq!(
        errors.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["each feature of the package `_` of the extension `wc` must have a name of its own; `count` is used twice"]
    );
}

// --- registration ------------------------------------------------------------------------

#[test]
fn a_registry_with_a_scripted_package_builds_without_a_diagnostic() {
    let double = Double::new("wc").feature("count", |b| b.table("wc.words", "The words of each tree."));
    let registry = registry_with([double.package()]);
    assert!(registry.has_feature("count"));
    let diagnostics: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let table = registry.table_named("wc.words").expect("the table is declared");
    assert!(table.def.ty.is::<ExtTable>());
    assert_eq!(table.def.owner.as_str(), "count");
}

#[test]
fn a_name_outside_the_package_prefix_fails_the_build_for_an_element() {
    let double = Double::new("wc").feature("count", |b| {
        b.element(ElementDef::new("", ["shout"], SyntaxDef::Macro { args: Vec::new() }, ElementSpec::new("shout")))
    });
    let error = build_error(double);
    assert!(
        error.contains(
            "flm.ext.name-outside-extension: the element `shout` declared by the package `_` of the extension `wc` must begin with `wc.`"
        ),
        "{error}"
    );
}

#[test]
fn a_name_outside_the_package_prefix_fails_the_build_for_a_table() {
    let error = build_error(Double::new("wc").feature("count", |b| b.table("words", "")));
    assert!(
        error.contains("flm.ext.name-outside-extension: the table `words` declared by the package `_` of the extension `wc` must begin with `wc.`"),
        "{error}"
    );
}

#[test]
fn a_name_outside_the_package_prefix_fails_the_build_for_a_stage() {
    let error = build_error(Double::new("wc").feature("count", |b| {
        b.stage(stage(StageDecl::new("count-words", Effect::Compute, Scope::PerTree), Needs::new(), |_, _| Ok(())))
    }));
    assert!(error.contains("flm.ext.name-outside-extension"), "{error}");
    assert!(error.contains("the stage `count-words`"), "{error}");
    let error = build_error(Double::new("wc").feature("count", |b| {
        b.table("wc.words", "")?;
        let decl = StageDecl::new("wc.count", Effect::Compute, Scope::PerTree).writes("wc.words").provides("counted");
        b.stage(stage(decl, Needs::new(), |_, _| Ok(())))
    }));
    assert!(error.contains("the capability `counted`"), "{error}");
}

#[test]
fn a_failing_register_handler_fails_the_build_with_handler_failed() {
    let error = build_error(Double::new("wc").feature("count", |_| {
        Err(ExtError::Handler { detail: "TypeError: x is undefined".to_string(), data: Value::Null })
    }));
    assert!(error.contains("`count`"), "{error}");
    assert!(error.contains("flm.ext.handler-failed: TypeError: x is undefined"), "{error}");
}

/// The feature `defs` of the package `wc`: one definition, `\shout{…}`,
/// registered with its element and its rule, under `policy`.
fn shout(content: &'static str, policy: ExpandPolicy) -> Double {
    Double::new("wc").feature("defs", move |b| {
        let definition = Definition::macro_("shout", vec![ArgDef::mandatory("text")]).content(content).expand(policy);
        let name = element_name_under("wc", &definition);
        let definition = UserDefinition::new(definition);
        b.lowering_rule(expand_in_tree_rule_for(&name, definition.definition()))?;
        b.element(definition.element_def(&b.feature(), &name))
    })
}

#[test]
fn a_scripted_definition_declares_an_element_under_the_helper_s_name() {
    let registry = registry_with([shout("<#1>", ExpandPolicy::PerPlan).package()]);
    let diagnostics: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert!(registry.element_named("wc.macro.shout").is_some());
    assert!(element_names(&registry, "\\shout{hey}").iter().any(|name| name == "wc.macro.shout"));
    let (text, diagnostics) = common::text_of(&registry, "A \\shout{hey}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "A <hey>.\n");
}

#[test]
fn a_registry_holding_scripted_definitions_without_userdefs_does_not_build() {
    let mut settings = PackageSettings::new();
    settings.switch_off("userdefs");
    let standard = stdcore::factory().expect("the factory").build(&settings).expect("the package");
    let mut builder = RegistryBuilder::new();
    builder.use_package(standard);
    builder.use_package(shout("<#1>", ExpandPolicy::Always).package());
    let error = builder.build(&RegistryInputs::new()).expect_err("the definitions need userdefs");
    assert!(error.to_string().contains("flm.userdefs.always-expanded"), "{error}");
}

#[test]
fn a_scripted_fallback_answers_an_unknown_macro_in_text_mode() {
    let double = Double::new("wc").feature("loose", |b| {
        b.fallback(FallbackDef::new([Mode::Text], FallbackKind::Macro, ElementSpec::new("wc.unknown")))
    });
    let registry = registry_with([double.package()]);
    let names = element_names(&registry, "A \\nosuchmacro here.");
    assert!(names.iter().any(|name| name == "wc.unknown"), "{names:?}");
    let error = build_error(Double::new("wc").feature("loose", |b| {
        b.fallback(FallbackDef::new([Mode::Text], FallbackKind::Macro, ElementSpec::new("unknown")))
    }));
    assert!(error.contains("the element `unknown`"), "{error}");
}

#[test]
fn a_scripted_definition_marked_overriding_replaces_a_standard_macro() {
    let double = Double::new("wc").feature("defs", |b| {
        let definition = Definition::macro_("emph", vec![ArgDef::mandatory("text")]).content("[#1]").overriding();
        let name = element_name_under("wc", &definition);
        let definition = UserDefinition::new(definition);
        b.override_callable(CallableKind::Macro, "emph")?;
        b.lowering_rule(expand_in_tree_rule_for(&name, definition.definition()))?;
        b.element(definition.element_def(&b.feature(), &name))
    });
    let registry = registry_with([double.package()]);
    let names = element_names(&registry, "\\emph{x}");
    assert!(names.iter().any(|name| name == "wc.macro.emph"), "{names:?}");
}

#[test]
fn requiring_an_unfilled_service_slot_fails_the_build() {
    let error = build_error(Double::new("wc").feature("count", |b| b.require_service("wc.spell-checker")));
    assert!(error.contains("wc.spell-checker"), "{error}");
}

#[test]
fn digest_input_changes_the_registry_digest() {
    let double = |bytes: &[u8]| Double::new("wc").feature("count", |b| b.table("wc.words", "")).digest(bytes);
    let a = registry_with([double(b"version a").package()]);
    let a_again = registry_with([double(b"version a").package()]);
    let b = registry_with([double(b"version b").package()]);
    assert_eq!(a.digest(), a_again.digest());
    assert_ne!(a.digest(), b.digest());
    let fed = registry_with([Double::new("wc")
        .feature("count", |b| {
            b.table("wc.words", "")?;
            b.digest_input(b"more")
        })
        .digest(b"version a")
        .package()]);
    assert_ne!(a.digest(), fed.digest());
}

#[test]
fn the_late_init_hook_reports_a_template_that_does_not_parse() {
    let registry = registry_with([shout("\\begin{x}", ExpandPolicy::PerPlan).package()]);
    // The compilation reports the parse's own conditions, in a source named
    // after the definition's template.
    let reported: Vec<(String, Option<String>)> = registry
        .build_diagnostics()
        .iter()
        .map(|d| (d.identifier().to_string(), d.span().source().origin().synthetic_label().map(str::to_string)))
        .collect();
    let expected = ("core.environments.missing-terminator".to_string(), Some("template of \\shout (text mode)".to_string()));
    assert!(reported.contains(&expected), "{reported:?}");
}

// --- the reads of a stage ------------------------------------------------------------------

/// The package `wc` of case 1: a table, a stage writing it from the
/// structured trees, and a stage reading it (and writing a table of its own,
/// whose name is what a plan demands).
fn counting(report_reads: &'static str) -> Double {
    Double::new("wc").feature("count", move |b| {
        b.table("wc.words", "The words of each tree.")?;
        b.table("wc.reported", "The report.")?;
        let count = StageDecl::new("wc.count", Effect::Compute, Scope::PerTree).writes("wc.words");
        b.stage(stage(count, Needs::new().reads("flm.blocks.structured"), |_, _| Ok(())))?;
        let report = StageDecl::new("wc.report", Effect::Compute, Scope::Document).writes("wc.reported");
        b.stage(stage(report, Needs::new().reads(report_reads), |_, _| Ok(())))
    })
}

#[test]
fn a_stage_reads_its_own_table_and_another_feature_s_trees_by_name() {
    let registry = registry_with([counting("wc.words").package()]);
    let plan = Plan::build(&registry, &PlanRequest::new().demand("wc.reported")).expect("the plan builds");
    let scheduled: Vec<String> = plan.scheduled_names().iter().map(|n| n.to_string()).collect();
    assert!(scheduled.iter().any(|n| n == "wc.count"), "{scheduled:?}");
    assert!(scheduled.iter().any(|n| n == "wc.report"), "{scheduled:?}");
}

#[test]
fn a_read_of_an_undeclared_table_fails_the_plan() {
    let registry = registry_with([counting("wc.nothing").package()]);
    let error = Plan::build(&registry, &PlanRequest::new().demand("wc.reported")).expect_err("undeclared");
    assert!(error.to_string().contains("wc.nothing"), "{error}");
}

// --- the WP-2 review's test gaps (plan A, A30) -------------------------------------------------

#[test]
fn a_helper_made_declaration_with_a_hook_registers_with_the_hook() {
    let double = Double::new("wc").feature("defs", |b| {
        let definition = Definition::macro_("shout", vec![ArgDef::mandatory("text")]).content("<#1>");
        let name = element_name_under("wc", &definition);
        let definition = UserDefinition::new(definition);
        b.lowering_rule(expand_in_tree_rule_for(&name, definition.definition()))?;
        let hook = expand(ExpandPolicy::Always, |_, _, _| Ok(Expansion::Default));
        b.element(definition.element_def_with(&b.feature(), &name, hook))
    });
    let registry = registry_with([double.package()]);
    let diagnostics: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let element = registry.element_named("wc.macro.shout").expect("the element is registered");
    // The element's `Expand` aspect is the extension's hook (its policy), not
    // the definition itself (whose policy is the default, per plan).
    let aspect = element.aspect::<dyn CoreExpand>().expect("the element carries an expansion hook");
    assert_eq!(aspect.policy(), ExpandPolicy::Always);
}

#[test]
fn a_helper_made_declaration_outside_the_package_prefix_fails_the_build() {
    let error = build_error(Double::new("wc").feature("defs", |b| {
        let definition = UserDefinition::new(Definition::macro_("shout", vec![ArgDef::mandatory("text")]).content("!"));
        b.element(definition.element_def(&b.feature(), "shout"))
    }));
    assert!(
        error.contains(
            "flm.ext.name-outside-extension: the element `shout` declared by the package `_` of the extension `wc` must begin with `wc.`"
        ),
        "{error}"
    );
}

#[test]
fn the_extension_s_digest_counts_in_the_registry_digest() {
    // The digest the loader computes once per extension (the manifest, then
    // the engine's bytes) is what every feature feeds; the double's stands
    // for it.
    let double = |bytes: &[u8]| Double::new("wc").feature("count", |b| b.table("wc.words", "")).digest(bytes);
    let a = registry_with([double(b"version a").package()]);
    let again = registry_with([double(b"version a").package()]);
    let b = registry_with([double(b"version b").package()]);
    assert_eq!(a.digest(), again.digest());
    assert_ne!(a.digest(), b.digest());
}
