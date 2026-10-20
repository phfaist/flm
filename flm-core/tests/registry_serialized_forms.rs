//! The serialized forms of the plain-data registry and plan types
//! (`flm_core::core::registry`, "Serialized forms"): the JSON each writes,
//! the fields that hold behavior left out, the enumerations as lowercase
//! words, and a whole plan of the standard registry written without error.

mod common;

use serde_json::json;

use flm_core::core::plan::{Excluded, Overrides, PlanWarning};
use flm_core::core::registry::{
    ArgumentRoleDef, CapabilityDef, FeatureInfo, LoweringSelection, OutputDef, PeerName, WorkflowDef,
};
use flm_core::core::stage::{Effect, Needs, RewriteDecl, Scope, StageDecl};
use flm_core::lang::mint::{BodyLayout, CallableKind, Symbol};
use flm_core::model::options::{Bind, OptionDecl, OptionRef};
use flm_core::stdcore::StdcoreConfig;
use flm_core::stdcore::features::refs::{LABELS, Labels};

use common::stdcore::{plan, registry};

fn written(value: impl serde::Serialize) -> serde_json::Value {
    serde_json::to_value(value).expect("every plain-data type serializes")
}

/// A documentation text as the registry keeps it: the text under the cargo
/// feature `docs`, and an empty text without it.
fn docs(text: &str) -> &str {
    if cfg!(feature = "docs") { text } else { "" }
}

#[test]
fn a_stage_declaration_writes_its_fields_and_the_name_of_each_table() {
    let decl = StageDecl::new("t.scan", Effect::Compute, Scope::PerTree)
        .provides("t.scanned")
        .writes(LABELS)
        .docs("Scans.");
    let value = written(&decl);
    assert_eq!(value["name"], "t.scan");
    assert_eq!(value["effect"], "compute");
    assert_eq!(value["scope"], "per-tree");
    assert_eq!(value["provides"], json!(["t.scanned"]));
    assert_eq!(value["docs"], docs("Scans."));
    let table = &value["writes"][0];
    assert_eq!(table["name"], "flm.labels");
    assert_eq!(table["type_name"], std::any::type_name::<Labels>());
    assert!(table.get("type_id").is_none());
    assert_eq!(written(Effect::Io), "io");
    assert_eq!(written(Scope::Document), "document");
    assert_eq!(
        written(RewriteDecl::new("t.expand", "t.expanded").refines("t.done").default()),
        json!({"name": "t.expand", "publishes": "t.expanded", "refines": "t.done", "default": true, "docs": ""})
    );
    let needs = written(Needs::new().requires("t.a").reads_opt(LABELS));
    assert_eq!(needs["requires"], json!(["t.a"]));
    assert_eq!(needs["reads_opt"][0]["name"], "flm.labels");
}

#[test]
fn the_registry_declarations_write_their_fields() {
    assert_eq!(
        written(FeatureInfo::new("refs", "References").docs("Labels.").version("1")),
        json!({"name": "refs", "title": "References", "docs": docs("Labels."), "version": "1"})
    );
    assert_eq!(
        written(CapabilityDef::new("t.b").refines("t.a").default()),
        json!({"name": "t.b", "docs": "", "refines": "t.a", "default": true, "unprovided": null})
    );
    assert_eq!(
        written(ArgumentRoleDef::new("t.key", "A key.")),
        json!({"name": "t.key", "docs": docs("A key.")})
    );
    let output = written(OutputDef::new("t.out", "t.done", "t.files", "text/plain", "txt"));
    assert_eq!(output["entry"], json!({"media_type": "text/plain", "extension": "txt"}));
    let mut workflow = WorkflowDef::new("t.flow");
    workflow.lowering.insert("text".into(), LoweringSelection::new().prefer("t.rule"));
    let value = written(&workflow);
    assert_eq!(value["name"], "t.flow");
    assert_eq!(value["lowering"], json!({"text": {"prefer": ["t.rule"]}}));
    assert!(value.get("render_options").is_none());
}

#[test]
fn an_option_type_is_written_as_its_name() {
    let decl = OptionDecl::named("t.width", "t", Bind::Render).docs("The width.");
    assert_eq!(written(&decl), json!({"key": "t.width", "bind": "render", "owner": "t", "docs": docs("The width.")}));
    assert_eq!(written(OptionRef::named("t.width")), "t.width");
    assert_eq!(written(Bind::Either), "either");
}

#[test]
fn names_kinds_warnings_and_exclusions_have_one_spelling_each() {
    assert_eq!(written(PeerName::ScopedPackage("t.pkg".into())), json!({"kind": "scoped package", "name": "t.pkg"}));
    assert_eq!(written(PeerName::ArgumentRole("t.key".into())), json!({"kind": "argument role", "name": "t.key"}));
    // Every kind is written as `PeerName::kind` spells it, the word the
    // condition `flm.registry.unresolved-peer` carries in its data.
    let peers = [
        PeerName::Backend("t.out".into()),
        PeerName::Element("t.box".into()),
        PeerName::Table("t.data".into()),
        PeerName::Slot("t.slot".into()),
        PeerName::ScopedPackage("t.pkg".into()),
        PeerName::ElementType("t.box".into()),
        PeerName::ArgumentRole("t.key".into()),
    ];
    for peer in &peers {
        assert_eq!(written(peer), json!({"kind": peer.kind(), "name": peer.name().as_str()}));
    }
    assert_eq!(written(BodyLayout::FromRole), "from-role");
    assert_eq!(written(CallableKind::Specials), "specials");
    assert_eq!(
        written(Symbol { kind: CallableKind::Macro, name: String::from("section") }),
        json!({"kind": "macro", "name": "section"})
    );
    let warning = PlanWarning::IgnoredPreference {
        stage: "t.b".into(),
        excluded_by: "t.a".into(),
        over: "t.done".into(),
        workflow: None,
    };
    assert_eq!(
        written(&warning),
        json!({"kind": "ignored-preference", "stage": "t.b", "excluded_by": "t.a", "over": "t.done", "workflow": null})
    );
    let excluded = Excluded { stage: "t.b".into(), excluded_by: "t.a".into(), over: "t.done".into() };
    assert_eq!(written(&excluded), json!({"stage": "t.b", "excluded_by": "t.a", "over": "t.done"}));
}

#[test]
fn every_stage_plan_of_a_standard_plan_is_written() {
    let registry = registry(StdcoreConfig::default());
    let plan = plan(&registry, "html", &Overrides::default());
    assert!(!plan.manifests().is_empty());
    for (idx, stage) in plan.manifests() {
        let value = written(stage);
        assert_eq!(value["idx"], json!(idx.0));
        assert_eq!(value["manifest"]["decl"]["name"], registry.stage(*idx).name.as_str());
        assert!(value["reads"].is_object() && value["writes"].is_object());
        assert!(value["requires"].is_array() && value["options"].is_array());
    }
}
