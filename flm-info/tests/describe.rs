//! The query API over the standard environment of the library: the registry of `stdcore`
//! and `math-passthrough` with a plan for the workflow `text`, as `flm-core`'s own tests
//! build it.
//!
//! The views are compared with JSON goldens under `tests/goldens/describe/`, written as
//! indented JSON text and compared byte for byte (`assert_text_golden`): the views hold
//! JSON Schema fragments, whose `$ref` and `$defs` keys a techy serialized value cannot
//! hold, so the JSON form of `assert_json_golden` does not apply. Rewrite them with
//! `UPDATE_GOLDEN=1` after reading the diff; they change whenever a documentation text of
//! the standard features changes.

use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::mint::CallableKind;
use flm_core::stdcore::{self, StdcoreConfig};
use flm_info::env::{ArgumentView, Hit};
use flm_info::{Environment, summary_of};
use serde::Serialize;

/// The standard environment, with the plan of the workflow `text` in view.
fn environment() -> Environment {
    let mut builder = RegistryBuilder::new();
    builder.use_package(stdcore::package(StdcoreConfig::new()));
    builder.use_package(flm_core::math_passthrough::package());
    let registry = builder.build(&RegistryInputs::new()).expect("the standard registry builds");
    let plan = Plan::for_workflow(&registry, "text", &Overrides::none()).expect("the plan of `text`");
    let factories = vec![
        stdcore::factory().expect("the factory of stdcore"),
        flm_core::math_passthrough::factory().expect("the factory of math-passthrough"),
    ];
    Environment::new(registry, Some(Arc::new(plan)), factories)
}

/// Compares `value`, written as indented JSON, with the golden file `name`.
fn golden(name: &str, value: &impl Serialize) {
    let text = serde_json::to_string_pretty(value).expect("a view is written as JSON") + "\n";
    assert_text_golden(format!("{}/tests/goldens/describe/{name}", env!("CARGO_MANIFEST_DIR")), &text);
}

#[test]
fn the_headings_feature_is_a_golden() {
    let env = environment();
    let feature = env.feature("headings").expect("the headings feature");
    assert_eq!(feature.package.as_deref(), Some("stdcore"));
    assert!(!feature.elements.is_empty());
    golden("feature-headings.json", &feature);
}

#[test]
fn the_section_macro_is_a_golden() {
    let env = environment();
    let section = env.command(CallableKind::Macro, "section");
    assert_eq!(section.last().map(|view| view.feature.as_str()), Some("headings"));
    golden("command-macro-section.json", &section);
}

#[test]
fn the_ref_macro_with_its_argument_role_is_a_golden() {
    let env = environment();
    let views = env.command(CallableKind::Macro, "ref");
    let view = views.last().expect("`\\ref` is declared");
    assert!(
        view.arguments.iter().any(|argument| argument.role.as_deref() == Some("flm.refs.target")),
        "{:?}",
        view.arguments
    );
    golden("command-macro-ref.json", &views);
}

#[test]
fn the_verbatimcode_environment_with_its_raw_body_is_a_golden() {
    let env = environment();
    let views = env.command(CallableKind::Environment, "verbatimcode");
    let view = views.last().expect("`verbatimcode` is declared");
    assert_eq!(view.kind, Some(CallableKind::Environment));
    let body = view.body.as_ref().expect("an environment has a body");
    assert!(body.reader.is_some(), "{body:?}");
    golden("command-environment-verbatimcode.json", &views);
}

#[test]
fn a_special_character_sequence_is_a_golden_and_found_by_its_trigger() {
    let env = environment();
    let views = env.command(CallableKind::Specials, "~");
    let view = views.last().expect("`~` is declared");
    assert_eq!((view.kind, view.trigger.as_deref()), (Some(CallableKind::Specials), Some("~")));
    assert!(view.names.is_empty(), "{:?}", view.names);
    golden("command-specials-tilde.json", &views);
    // A name without the backslash finds the specials whose trigger it is; with the
    // backslash, it finds only macros and environments.
    let is_specials = |hit: &Hit| matches!(hit, Hit::Command { kind: CallableKind::Specials, name, .. } if name == "~");
    assert!(env.lookup("~").iter().any(is_specials), "{:?}", env.lookup("~"));
    assert!(!env.lookup("\\~").iter().any(is_specials), "{:?}", env.lookup("\\~"));
}

#[test]
fn a_declaration_without_syntax_is_synthesized() {
    let env = environment();
    let synthesized: Vec<_> = env.features().iter().flat_map(|feature| env.commands(&feature.name).synthesized).collect();
    assert!(!synthesized.is_empty());
    for view in &synthesized {
        assert!(view.synthesized && view.kind.is_none() && view.modes.is_empty() && view.arguments.is_empty(), "{view:?}");
    }
    // The number of a heading, which the heading's lowering rule inserts.
    let number = env
        .commands("headings")
        .synthesized
        .into_iter()
        .find(|view| view.element.name == "flm.heading.number")
        .expect("the headings feature declares the heading number without syntax");
    let json = serde_json::to_value(&number).expect("a command view is JSON");
    assert_eq!(json["synthesized"], serde_json::json!(true));
    assert!(json.get("kind").is_none(), "{json}");
    golden("command-synthesized-heading-number.json", &number);
}

#[test]
fn the_scoped_package_of_list_items_is_a_golden() {
    let env = environment();
    let package = env.scoped_package("flm.lists.items").expect("the scoped package of list items");
    assert!(package.commands.macros.iter().any(|command| command.names.iter().any(|name| name == "item")));
    assert!(package.available_in.iter().any(|site| site.command == "itemize" && site.part == "body"));
    assert!(package.commands.macros.iter().all(|command| command.within.as_deref() == Some("flm.lists.items")));
    golden("scoped-package-flm.lists.items.json", &package);
}

#[test]
fn the_settings_of_html_are_a_golden() {
    let env = environment();
    let settings = env.settings("html").expect("the html feature takes settings");
    assert!(!settings.keys.is_empty());
    golden("settings-html.json", &settings);
}

#[test]
fn the_text_workflow_with_its_plan_is_a_golden() {
    let env = environment();
    let workflow = env.workflow("text").expect("the workflow `text`");
    let plan = workflow.plan.as_ref().expect("the plan in view is the plan of `text`");
    assert!(plan.stages.iter().any(|stage| stage.rewrite));
    assert!(plan.lowering.iter().any(|lowering| lowering.backend == "text"));
    // The plan in view belongs to `text` alone.
    assert!(env.workflow("html").expect("the workflow `html`").plan.is_none());
    golden("workflow-text.json", &workflow);
}

#[test]
fn the_namespaces_are_a_golden() {
    let env = environment();
    let namespaces = env.namespaces();
    let names: Vec<&str> = namespaces.iter().map(|namespace| namespace.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "feature",
            "package",
            "element",
            "capability",
            "table",
            "collection",
            "stage",
            "lowering-rule",
            "backend",
            "output",
            "workflow",
            "option-type",
            "service-slot",
            "scoped-package",
            "presentation-element",
            "piece",
            "style",
            "script",
            "settings-key",
            "argument-role",
        ]
    );
    assert!(namespaces.iter().all(|namespace| !namespace.what.is_empty()));
    assert_eq!(env.namespace("stage"), namespaces.iter().find(|namespace| namespace.name == "stage").cloned());
    assert!(env.namespace("registry").is_none());
    golden("namespaces.json", &namespaces);
}

#[test]
fn a_summary_is_the_first_paragraph_trimmed() {
    assert_eq!(summary_of("A cell, in the next column.\n\nThe placement is given in brackets."), "A cell, in the next column.");
    assert_eq!(summary_of("  One paragraph\non two lines.  "), "One paragraph\non two lines.");
    assert_eq!(summary_of("\n \t\nAfter blank lines.\n   \nNot this."), "After blank lines.");
    assert_eq!(summary_of("First.\r\n\r\nSecond."), "First.");
    assert_eq!(summary_of(""), "");
    assert_eq!(summary_of("\n\n"), "");
}

#[test]
fn a_table_that_is_also_a_capability_gives_two_hits() {
    let env = environment();
    let table = env.registry().tables()[0].def.name.to_string();
    let hits = env.lookup(&table);
    let namespaces: Vec<&str> = hits
        .iter()
        .filter_map(|hit| match hit {
            Hit::Entity { namespace, .. } => Some(namespace.as_str()),
            Hit::Command { .. } => None,
        })
        .collect();
    assert!(namespaces.contains(&"table") && namespaces.contains(&"capability"), "{hits:?}");
    assert_eq!(hits.len(), 2, "{hits:?}");
}

#[test]
fn a_command_is_found_with_and_without_the_backslash() {
    let env = environment();
    let with = env.lookup("\\section");
    let without = env.lookup("section");
    assert!(!with.is_empty());
    assert_eq!(with, without);
    for hit in &with {
        let Hit::Command { kind, name, feature, within, .. } = hit else { panic!("not a command: {hit:?}") };
        assert_eq!((*kind, name.as_str(), feature.as_str(), within.as_ref()), (CallableKind::Macro, "section", "headings", None));
    }
    // A scoped package's command is found, with its package.
    let item = env.lookup("\\item");
    assert!(item.iter().any(|hit| matches!(hit, Hit::Command { within: Some(within), .. } if within == "flm.lists.items")), "{item:?}");
    // A stage is found by its name; nothing has an unknown name.
    assert!(env.lookup("flm.refs.resolve").iter().any(|hit| matches!(hit, Hit::Entity { namespace, .. } if namespace == "stage")));
    assert!(env.lookup("no.such.name").is_empty());
}

/// The written form of every named argument of the standard registry holds the argument's
/// name, except a form with no place for content: a marker (`*`, `!`) and embellishments
/// (`^{…}_{…}`). A tack-on argument is written as its fields, whose arguments are checked in
/// turn.
#[test]
fn every_named_argument_is_written_with_its_name() {
    fn check(command: &str, argument: &ArgumentView, checked: &mut usize, wrong: &mut Vec<String>) {
        if !argument.fields.is_empty() {
            for field in &argument.fields {
                for inner in &field.arguments {
                    check(&format!("{command} \\{}", field.name), inner, checked, wrong);
                }
            }
            return;
        }
        let Some(name) = &argument.name else { return };
        let marker = argument.written.chars().count() == 1;
        let embellishments = argument.written.contains("{…}");
        if marker || embellishments {
            return;
        }
        *checked += 1;
        if !argument.written.contains(name.as_str()) {
            wrong.push(format!("{command}: `{name}` written `{}`", argument.written));
        }
    }
    let env = environment();
    let (mut checked, mut wrong) = (0, Vec::new());
    for feature in env.features() {
        let mut groups = vec![env.commands(&feature.name)];
        groups.extend(env.scoped_packages(&feature.name).into_iter().map(|package| package.commands));
        for group in groups {
            for command in group.macros.iter().chain(&group.environments).chain(&group.specials) {
                let label = command.names.first().or(command.trigger.as_ref()).cloned().unwrap_or_default();
                for argument in &command.arguments {
                    check(&label, argument, &mut checked, &mut wrong);
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
    assert!(checked > 50, "{checked} named arguments");
    // A verbatim argument between a prescribed pair, and a signature made of several forms.
    let written = |kind: CallableKind, name: &str| -> Vec<String> {
        let view = env.command(kind, name).pop().unwrap_or_else(|| panic!("the command `{name}`"));
        view.arguments.iter().map(|argument| argument.written.clone()).collect()
    };
    assert_eq!(written(CallableKind::Macro, "url"), ["{address}"]);
    assert_eq!(written(CallableKind::Macro, "href"), ["{address}", "{text}"]);
}

#[test]
fn the_feature_inventory_lists_what_the_feature_declares() {
    let env = environment();
    let refs = env.feature("refs").expect("the refs feature");
    let namespaces: Vec<&str> = refs.declares.iter().map(|declared| declared.namespace.as_str()).collect();
    assert!(namespaces.contains(&"stage") && namespaces.contains(&"argument-role"), "{namespaces:?}");
    assert!(refs.declares.iter().all(|declared| declared.entries.iter().all(|entry| entry.owner.as_deref() == Some("refs"))));
    // Every value lists the commands written for it; the `\label` field is a command of
    // the field element.
    let headings = env.feature("headings").expect("the headings feature");
    assert!(headings.elements.iter().flat_map(|ty| &ty.values).any(|value| value.commands.iter().any(|command| command.name == "section")));
    assert!(env.features().len() == env.registry().features().len());
    assert!(env.feature("no-such-feature").is_none());
}

#[test]
fn the_packages_list_their_features() {
    let env = environment();
    let packages = env.packages();
    assert_eq!(packages.iter().map(|package| package.name.as_str()).collect::<Vec<_>>(), ["stdcore", "math-passthrough"]);
    assert!(packages[0].features.iter().any(|feature| feature == "headings"));
}

#[test]
fn the_parsing_settings_hold_the_registry_s_configuration() {
    let env = environment();
    let parsing = env.parsing_settings();
    let names: Vec<&str> = parsing.keys.iter().map(|key| key.name.as_str()).collect();
    assert_eq!(names, ["comments", "comment_prefix", "dollar_math"]);
    assert_eq!(parsing.keys[1].value, serde_json::json!("%%"));
    assert!(parsing.keys.iter().all(|key| key.description.is_some()));
}

#[test]
fn a_feature_without_settings_and_an_unknown_one_have_no_settings_view() {
    let env = environment();
    assert!(env.settings("no-such-feature").is_none());
    let without: Vec<String> = stdcore::factory()
        .expect("the factory of stdcore")
        .features()
        .iter()
        .filter(|feature| feature.settings.is_none())
        .map(|feature| feature.name.to_string())
        .collect();
    if let Some(feature) = without.first() {
        assert!(env.settings(feature).is_none());
    }
}

#[test]
fn the_schema_of_the_description_is_a_json_schema() {
    let schema = flm_info::describe::schema();
    assert!(schema.is_object());
    assert_eq!(schema["$schema"], flm_info::describe::JSON_SCHEMA_VERSION);
    let defs = schema["$defs"].as_object().expect("the definitions");
    for name in ["PackageView", "FeatureView", "CommandView", "ScopedPackageView", "SettingsView", "WorkflowView", "NamespaceView", "Hit"] {
        assert!(defs.contains_key(name), "{name}");
    }
    // A key that the JSON form leaves out when it has nothing to show is not required.
    let required: Vec<&str> = defs["CommandView"]["required"]
        .as_array()
        .expect("the required keys of a command view")
        .iter()
        .filter_map(serde_json::Value::as_str)
        .collect();
    assert!(required.contains(&"feature") && required.contains(&"names") && required.contains(&"element"), "{required:?}");
    for optional in ["kind", "docs", "summary", "available_in", "synthesized"] {
        assert!(!required.contains(&optional), "{optional} in {required:?}");
    }
    // Every view of the standard environment is written as JSON.
    let env = environment();
    for feature in env.features() {
        serde_json::to_value(&feature).expect("a feature view is JSON");
        serde_json::to_value(env.commands(&feature.name)).expect("a commands view is JSON");
    }
}
