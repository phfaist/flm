//! Service slots declared from a script, through the Rust test double: a slot a package declares holds data, any
//! package fills it, a stage reads its values by name; `flm-core`'s own checks
//! of the assembly apply; the route refuses what a slot's name does not take.

mod common;

use std::sync::Arc;

use flm_core::core::registry::{BuildError, UnresolvedPeer};
use flm_core::core::services::ServiceError;
use flm_ext::api::core::registry::{Contributed, FeatureBuilder};
use flm_ext::api::core::services::Arity;
use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_ext::api::stdlib::features::citations::{CitationProvider, CitationRequest, CitationResult, ProviderCx};
use flm_ext::api::stdlib::features::compose::{TemplateContext, TemplateCx, TemplateDescription, TemplateModule};
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::double::{stage, Double};
use common::{ext_items, feature_errors, registry_with, run_demanding, try_registry_with};

fn data(value: Value) -> Contributed {
    Contributed::Data(value)
}

/// A citation provider that serves no key, and a template module that
/// describes nothing: handler objects for the refusals of the route.
struct Idle;

impl CitationProvider for Idle {
    fn name(&self) -> String {
        "bib.idle".to_string()
    }

    fn answer(&self, _: &CitationRequest, _: &dyn ProviderCx) -> Result<CitationResult, ExtError> {
        Ok(Ok(None))
    }
}

impl TemplateModule for Idle {
    fn name(&self) -> String {
        "idle".to_string()
    }

    fn describe(&self, _: &Value) -> Result<TemplateDescription, ExtError> {
        Err(ExtError::Value { detail: "not described".to_string() })
    }

    fn render(&self, _: &TemplateContext, _: &dyn TemplateCx) -> Result<String, ExtError> {
        Ok(String::new())
    }
}

/// The package `bib`: declares the slot `bib.notes` of `arity`, contributes
/// `{note: "a"}` to it, and has a stage `bib.read` that writes to `bib.seen`
/// what `cx.service`, `cx.services` and `cx.services_by_precedence` answer
/// for `slot`.
fn reader(arity: Arity, slot: &'static str) -> Double {
    let read = stage(
        StageDecl::new("bib.read", Effect::Compute, Scope::Document).writes("bib.seen"),
        Needs::new(),
        move |cx, out| {
            let first = cx.service(slot)?;
            let all = cx.services(slot)?;
            let by_precedence = cx.services_by_precedence(slot)?;
            out.table("bib.seen")?.push(json!({ "first": first, "all": all, "by_precedence": by_precedence }))
        },
    );
    Double::new("bib").feature("notes", move |b| {
        b.service_slot("bib.notes", arity, "Notes any package adds.")?;
        b.contribute_to("bib.notes", data(json!({ "note": "a" })))?;
        b.table("bib.seen", "What the stage read.")?;
        b.stage(Arc::clone(&read))
    })
}

/// What `bib.read` wrote for a registry of `packages`.
fn seen(packages: Vec<Double>) -> Vec<Value> {
    let registry = registry_with(packages.into_iter().map(Double::package));
    let run = run_demanding(&registry, &["bib.seen"], &["x"]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    ext_items(&run, "bib.seen")
}

#[test]
fn a_slot_declared_by_one_package_and_filled_by_another_is_read_in_the_assembled_order() {
    let other = Double::new("other").feature("more", |b| {
        b.contribute_to("bib.notes", data(json!({ "note": "b" })))?;
        b.contribute_first_to("bib.notes", data(json!({ "note": "first" })))
    });
    assert_eq!(
        seen(vec![reader(Arity::OrderedMany, "bib.notes"), other]),
        [json!({
            "first": { "note": "first" },
            "all": [{ "note": "first" }, { "note": "a" }, { "note": "b" }],
            "by_precedence": [{ "note": "first" }, { "note": "b" }, { "note": "a" }],
        })]
    );
}

#[test]
fn between_features_the_value_of_the_feature_placed_last_takes_precedence() {
    let other = Double::new("other").feature("more", |b| {
        b.contribute_to("bib.notes", data(json!({ "note": "b" })))?;
        b.contribute_to("bib.notes", data(json!({ "note": "c" })))
    });
    assert_eq!(
        seen(vec![reader(Arity::OrderedMany, "bib.notes"), other]),
        [json!({
            "first": { "note": "b" },
            "all": [{ "note": "a" }, { "note": "b" }, { "note": "c" }],
            "by_precedence": [{ "note": "b" }, { "note": "c" }, { "note": "a" }],
        })]
    );
}

#[test]
fn a_slot_no_feature_declares_reads_as_nothing() {
    assert_eq!(
        seen(vec![reader(Arity::OrderedMany, "bib.absent")]),
        [json!({ "first": null, "all": [], "by_precedence": [] })]
    );
}

#[test]
fn a_single_slot_refilled_fails_the_build() {
    let other = Double::new("other").feature("more", |b| b.contribute_to("bib.notes", data(json!("b"))));
    let error = try_registry_with([reader(Arity::Single, "bib.notes").package(), other.package()])
        .expect_err("the build fails");
    assert!(
        matches!(error.errors.as_slice(), [BuildError::Service(ServiceError::SingleSlotRefilled { slot, first, second })]
            if *slot == "bib.notes" && *first == "notes" && *second == "more"),
        "{error:?}"
    );
}

#[test]
fn a_peer_contribution_to_an_absent_slot_is_dropped_with_one_note() {
    let peer = Double::new("other").feature("more", |b| {
        b.peer_slot("absent.notes", data(json!(1)))?;
        b.peer_slot("absent.notes", data(json!(2)))
    });
    let registry = registry_with([peer.package()]);
    assert_eq!(registry.build_diagnostics().iter().count(), 1);
    let notes: Vec<&str> = registry.build_diagnostics().conditions::<UnresolvedPeer>().map(|peer| peer.name.as_str()).collect();
    assert_eq!(notes, ["absent.notes"]);
}

#[test]
fn a_peer_contribution_to_a_declared_slot_fills_it() {
    let other = Double::new("other").feature("more", |b| b.peer_slot("bib.notes", data(json!({ "note": "peer" }))));
    assert_eq!(
        seen(vec![reader(Arity::OrderedMany, "bib.notes"), other]),
        // The feature placed last, `more`, takes precedence.
        [json!({
            "first": { "note": "peer" },
            "all": [{ "note": "a" }, { "note": "peer" }],
            "by_precedence": [{ "note": "peer" }, { "note": "a" }],
        })]
    );
}

#[test]
fn a_data_value_to_a_trait_valued_slot_fails_the_build_with_type_mismatch() {
    let wrong = Double::new("other").feature("more", |b| b.contribute_to("flm.artifacts.namer", data(json!("x"))));
    let error = try_registry_with([wrong.package()]).expect_err("the build fails");
    assert!(
        matches!(error.errors.as_slice(), [BuildError::Service(ServiceError::TypeMismatch { feature, slot, .. })]
            if *feature == "more" && *slot == "flm.artifacts.namer"),
        "{error:?}"
    );
}

#[test]
fn reading_a_slot_whose_values_are_not_data_is_unsupported() {
    let read = stage(
        StageDecl::new("bib.read", Effect::Compute, Scope::Document).writes("bib.seen"),
        Needs::new(),
        |cx, out| {
            let error = cx.services("flm.artifacts.namer").expect_err("not data");
            let first = cx.service("flm.artifacts.namer").expect_err("not data");
            assert_eq!(first, error);
            out.table("bib.seen")?.push(json!({ "kind": error.kind() }))
        },
    );
    let double = Double::new("bib").feature("notes", move |b| {
        b.table("bib.seen", "")?;
        b.stage(Arc::clone(&read))
    });
    let registry = registry_with([double.package()]);
    let run = run_demanding(&registry, &["bib.seen"], &["x"]);
    assert_eq!(ext_items(&run, "bib.seen"), [json!({ "kind": "unsupported" })]);
}

/// The identifier the error of the feature `notes` begins with, when
/// `register` fails the registry build with that one error.
fn refusal(register: impl Fn(&mut dyn FeatureBuilder) -> Result<(), ExtError> + Send + Sync + 'static) -> String {
    let double = Double::new("bib").feature("notes", register);
    let found = feature_errors(&try_registry_with([double.package()]).expect_err("the build fails"));
    match found.as_slice() {
        [(feature, detail)] if feature == "notes" => detail.split_once(": ").map_or(detail.clone(), |(id, _)| id.to_string()),
        other => panic!("one error of the feature `notes`, not {other:?}"),
    }
}

#[test]
fn a_slot_named_outside_the_stem_is_refused() {
    assert_eq!(refusal(|b| b.service_slot("notes", Arity::Single, "")), "flm.ext.name-outside-extension");
}

#[test]
fn data_contributed_to_a_handler_slot_is_refused() {
    assert_eq!(refusal(|b| b.contribute_to("flm.citations.providers", data(json!({})))), "flm.ext.handler-failed");
    assert_eq!(refusal(|b| b.peer_slot("flm.compose.templates", data(json!({})))), "flm.ext.handler-failed");
}

#[test]
fn a_handler_object_contributed_to_a_data_slot_or_first_is_refused() {
    assert_eq!(
        refusal(|b| b.contribute_to("bib.notes", Contributed::CitationProvider(Arc::new(Idle)))),
        "flm.ext.handler-failed"
    );
    assert_eq!(
        refusal(|b| b.contribute_first_to("flm.compose.templates", Contributed::TemplateModule(Arc::new(Idle)))),
        "flm.ext.handler-failed"
    );
}

#[test]
fn data_that_cannot_cross_is_refused() {
    assert_eq!(refusal(|b| b.contribute_to("bib.notes", data(json!(0.5)))), "flm.ext.handler-failed");
}
