//! The declarations the registry a host builds from the `stdcore` package
//! retains (`FlmRegistry::declarations`): every command its language
//! registers, `\section` with its arguments, the scoped package of list
//! items with the bodies that make it available; and the argument roles of
//! the standard features: the eight roles, each declared once by its feature
//! with a description, and the arguments that carry each.

mod common;

use std::collections::BTreeSet;

use flm_core::core::registry::{ArgumentRef, FlmRegistry, ScopedPackageUse};
use flm_core::lang::mint::{CallableKind, ElementDef, SyntaxDef};
use flm_core::core::registry::{RegistryBuilder, RegistryInputs};
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::userdefs::{UserdefsConfig, UserdefsData};

use common::stdcore::registry;

/// Each argument that has `role`, written as `command/argument` (the
/// argument's name, prefixed by `\field/` for an argument of a tack-on field),
/// once each, in name order.
fn carriers(registry: &FlmRegistry, role: &str) -> Vec<String> {
    let uses: Vec<ArgumentRef<'_>> = registry.declarations().role_uses(role);
    let written: BTreeSet<String> = uses
        .iter()
        .map(|place| {
            let command = match place.declaration.names.first() {
                Some(name) => format!("\\{name}"),
                None => place.declaration.element.name.to_string(),
            };
            let field = place.field.map(|field| format!("\\{}/", field.name)).unwrap_or_default();
            let argument = place.arg.name.as_deref().unwrap_or("?");
            format!("{command}/{field}{argument}")
        })
        .collect();
    written.into_iter().collect()
}

// --- the retained declarations -----------------------------------------------------------

fn arg_names(def: &ElementDef) -> Vec<Option<&str>> {
    def.syntax.args().iter().map(|arg| arg.name.as_deref()).collect()
}

#[test]
fn the_standard_registry_retains_every_command_its_language_registers() {
    let registry = registry(StdcoreConfig::default());
    let declarations = registry.declarations();
    for (symbol, owner) in registry.lang().symbols() {
        let found = declarations.named(symbol.kind, &symbol.name);
        let last = found.last().unwrap_or_else(|| panic!("no declaration of {symbol}"));
        assert_eq!(&last.owner, owner, "{symbol}");
    }
    let mut commands: Vec<(CallableKind, &str)> = declarations
        .elements()
        .iter()
        .filter_map(|def| def.syntax.kind().map(|kind| (kind, def)))
        .flat_map(|(kind, def)| match &def.syntax {
            SyntaxDef::Specials { trigger, .. } => vec![(kind, trigger.as_str())],
            _ => def.names.iter().map(|name| (kind, name.as_str())).collect(),
        })
        .collect();
    commands.sort();
    commands.dedup();
    assert_eq!(commands.len(), registry.lang().symbols().len());
}

#[test]
fn the_standard_section_declaration_is_found_with_its_arguments() {
    let registry = registry(StdcoreConfig::default());
    let section = registry.declarations().named(CallableKind::Macro, "section");
    assert_eq!(section.len(), 1);
    assert_eq!(section[0].owner, "headings");
    let args = arg_names(section[0]);
    assert!(args.contains(&Some("title")), "{args:?}");
    assert_eq!(args.last(), Some(&Some("fields")));
    assert_eq!(section[0].docs.is_empty(), !cfg!(feature = "docs"));
}

#[test]
fn the_standard_list_items_package_is_found_and_named_by_both_lists() {
    let registry = registry(StdcoreConfig::default());
    let declarations = registry.declarations();
    let items = declarations.scoped_package("flm.lists.items").expect("declared by lists");
    assert_eq!(items.owner, "lists");
    assert!(items.elements.iter().any(|def| def.names.iter().any(|name| name == "item")));
    let users: Vec<&str> = declarations
        .scoped_package_uses("flm.lists.items")
        .iter()
        .filter(|place| matches!(place, ScopedPackageUse::Body { .. }))
        .map(|place| place.declaration().names[0].as_str())
        .collect();
    assert!(users.contains(&"enumerate") && users.contains(&"itemize"), "{users:?}");
}

// --- the argument roles ------------------------------------------------------------------

#[test]
fn the_standard_features_declare_eight_roles_with_a_description() {
    let registry = registry(StdcoreConfig::default());
    let roles: Vec<(&str, &str)> =
        registry.argument_roles().iter().map(|role| (role.name.as_str(), role.owner.as_str())).collect();
    let mut sorted = roles.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        [
            ("flm.citations.key", "citations"),
            ("flm.defterm.defined-term", "defterm"),
            ("flm.defterm.target", "defterm"),
            ("flm.graphics.path", "graphics"),
            ("flm.href.address", "href"),
            ("flm.input.path", "input"),
            ("flm.refs.defined-label", "refs"),
            ("flm.refs.target", "refs"),
        ]
    );
    for role in registry.argument_roles() {
        assert_eq!(role.docs.is_empty(), !cfg!(feature = "docs"), "{}", role.name);
        assert!(role.name.starts_with(&format!("flm.{}.", role.owner)), "{}", role.name);
    }
    // Every peer declaration of a role resolves in the standard registry.
    assert!(registry.peer_argument_roles().iter().all(|peer| peer.resolved));
}

#[test]
fn the_references_roles_are_on_the_targets_and_on_every_label() {
    let registry = registry(StdcoreConfig::default());
    assert_eq!(carriers(&registry, "flm.refs.target"), ["\\eqref/target", "\\hyperref/target", "\\ref/target"]);
    let labels = carriers(&registry, "flm.refs.defined-label");
    for expected in ["\\section/\\label/target", "\\defterm/\\label/target", "\\label/target"] {
        assert!(labels.iter().any(|written| written == expected), "{expected} in {labels:?}");
    }
    // The `\label` field of a heading, a list item, a theorem and a term
    // definition; the `\label` command of the scoped packages of the
    // references feature, of floats and of equation lines.
    let owners: BTreeSet<String> = registry
        .declarations()
        .role_uses("flm.refs.defined-label")
        .iter()
        .map(|place| place.declaration.owner.to_string())
        .collect();
    let owners: Vec<&str> = owners.iter().map(String::as_str).collect();
    assert_eq!(owners, ["defterm", "floats", "headings", "lists", "math", "refs", "theorems"]);
}

#[test]
fn the_other_roles_are_on_the_arguments_they_describe() {
    let registry = registry(StdcoreConfig::default());
    assert_eq!(
        carriers(&registry, "flm.citations.key"),
        ["\\R/\\cite/keys", "\\cite/\\cite/keys", "\\cite/keys"]
    );
    assert_eq!(
        carriers(&registry, "flm.defterm.defined-term"),
        ["\\defterm/\\aliasterm/term", "\\defterm/term"]
    );
    assert_eq!(carriers(&registry, "flm.defterm.target"), ["\\term/ref"]);
    assert_eq!(carriers(&registry, "flm.graphics.path"), ["\\includegraphics/path"]);
    assert_eq!(carriers(&registry, "flm.input.path"), ["\\input/reference"]);
    assert_eq!(carriers(&registry, "flm.href.address"), ["\\email/address", "\\href/address", "\\url/address"]);
}

/// The standard registry whose `userdefs` settings are `settings`, as data.
fn with_userdefs(settings: serde_json::Value) -> Result<std::sync::Arc<FlmRegistry>, String> {
    let data: UserdefsData = serde_json::from_value(settings).expect("userdefs settings");
    let mut builder = RegistryBuilder::new();
    builder.use_package(flm_core::stdcore::package(StdcoreConfig::default().userdefs(UserdefsConfig::from(data))));
    builder.use_package(flm_core::math_passthrough::package());
    builder.build(&RegistryInputs::new()).map_err(|error| error.render())
}

#[test]
fn a_user_definition_written_as_data_states_a_role_another_feature_declares() {
    let registry = with_userdefs(serde_json::json!({
        "macros": { "seeref": { "args": [{ "code": "{", "name": "key", "role": "flm.refs.target" }], "content": "see #1" } },
    }))
    .expect("the registry builds");
    let found = registry.declarations().named(CallableKind::Macro, "seeref");
    assert_eq!(found[0].owner, "userdefs");
    assert_eq!(found[0].syntax.args()[0].role.as_ref().map(|name| name.as_str()), Some("flm.refs.target"));
    let error = with_userdefs(serde_json::json!({
        "macros": { "seeref": { "args": [{ "code": "{", "name": "key", "role": "flm.refs.targt" }], "content": "see #1" } },
    }))
    .expect_err("a role no feature declares is refused");
    assert!(
        error.contains(
            "feature `userdefs`: the argument `key` of `\\seeref` has the argument role `flm.refs.targt`, which no \
             registered feature declares"
        ),
        "{error}"
    );
}
