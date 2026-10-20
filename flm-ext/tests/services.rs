//! Service slots declared from a script (E6, plan sections 2.1, 3.1, 4.1),
//! through the Rust test double: a slot a package declares holds data, any
//! package fills it, a stage reads its values by name; `flm-core`'s own checks
//! of the assembly apply; the route refuses what a slot's name does not take.

mod common;

use std::sync::Arc;

use flm_ext::api::core::registry::{Contributed, FeatureBuilder};
use flm_ext::api::core::services::Arity;
use flm_ext::api::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_ext::api::stdcore::features::citations::{CitationProvider, CitationRequest, CitationResult, ProviderCx};
use flm_ext::api::stdcore::features::compose::{TemplateContext, TemplateCx, TemplateDescription, TemplateModule};
use flm_ext::ExtError;
use serde_json::{json, Value};

use common::double::{stage, Double};
use common::{ext_items, registry_with, run_demanding, try_registry_with};

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
fn a_single_slot_refilled_fails_the_build_with_flm_core_s_text() {
    let other = Double::new("other").feature("more", |b| b.contribute_to("bib.notes", data(json!("b"))));
    let error = try_registry_with([reader(Arity::Single, "bib.notes").package(), other.package()])
        .expect_err("the build fails");
    assert_eq!(
        error.render(),
        "service slot `bib.notes` takes at most one value, but feature `notes` contributes one and feature `more` \
         contributes another"
    );
}

#[test]
fn a_peer_contribution_to_an_absent_slot_is_dropped_with_one_note() {
    let peer = Double::new("other").feature("more", |b| {
        b.peer_slot("absent.notes", data(json!(1)))?;
        b.peer_slot("absent.notes", data(json!(2)))
    });
    let registry = registry_with([peer.package()]);
    let notes: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert_eq!(notes.len(), 1, "{notes:?}");
    assert!(notes[0].contains("absent.notes"), "{notes:?}");
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
    assert_eq!(
        error.render(),
        "a value contributed to service slot `flm.artifacts.namer` must have the slot's type `ArtifactNamer`, but \
         feature `more` contributes a `ExtService`"
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
            out.table("bib.seen")?.push(json!({ "kind": error.kind(), "text": error.to_string() }))
        },
    );
    let double = Double::new("bib").feature("notes", move |b| {
        b.table("bib.seen", "")?;
        b.stage(Arc::clone(&read))
    });
    let registry = registry_with([double.package()]);
    let run = run_demanding(&registry, &["bib.seen"], &["x"]);
    assert_eq!(
        ext_items(&run, "bib.seen"),
        [json!({
            "kind": "unsupported",
            "text": "reading the service slot `flm.artifacts.namer`, whose values are not data, is not supported yet",
        })]
    );
}

/// The error `register` fails with: the text of the registry build's error.
fn refusal(register: impl Fn(&mut dyn FeatureBuilder) -> Result<(), ExtError> + Send + Sync + 'static) -> String {
    let double = Double::new("bib").feature("notes", register);
    try_registry_with([double.package()]).expect_err("the build fails").render()
}

#[test]
fn a_slot_named_outside_the_stem_is_refused() {
    assert_eq!(
        refusal(|b| b.service_slot("notes", Arity::Single, "")),
        "feature `notes`: flm.ext.name-outside-extension: the service slot `notes` declared by \
         the package `_` of the extension `bib` must begin with `bib.`"
    );
}

#[test]
fn data_contributed_to_a_handler_slot_is_refused() {
    assert_eq!(
        refusal(|b| b.contribute_to("flm.citations.providers", data(json!({})))),
        "feature `notes`: flm.ext.handler-failed: `contribute_to`: the service slot \
         `flm.citations.providers` takes a citation provider (an object with `name` and `answer` or `answer_all`), \
         and the package `_` of the extension `bib` contributed data"
    );
    assert_eq!(
        refusal(|b| b.peer_slot("flm.compose.templates", data(json!({})))),
        "feature `notes`: flm.ext.handler-failed: `peer_slot`: the service slot \
         `flm.compose.templates` takes a compose template module (an object with `name`, `describe` and `render`), \
         and the package `_` of the extension `bib` contributed data"
    );
}

#[test]
fn a_handler_object_contributed_to_a_data_slot_or_first_is_refused() {
    assert_eq!(
        refusal(|b| b.contribute_to("bib.notes", Contributed::CitationProvider(Arc::new(Idle)))),
        "feature `notes`: flm.ext.handler-failed: `contribute_to`: the service slot `bib.notes` \
         takes data, and the package `_` of the extension `bib` contributed an object with methods; the slots that \
         take an object are `flm.citations.providers` (a citation provider) and `flm.compose.templates` (a \
         template module)"
    );
    assert_eq!(
        refusal(|b| b.contribute_first_to("flm.compose.templates", Contributed::TemplateModule(Arc::new(Idle)))),
        "feature `notes`: flm.ext.handler-failed: `contribute_first_to`: the value must be data; a \
         citation provider or a template module is contributed with `contribute_to` or `peer_slot`"
    );
}

#[test]
fn data_that_cannot_cross_is_refused() {
    let text = refusal(|b| b.contribute_to("bib.notes", data(json!(0.5))));
    assert!(text.starts_with("feature `notes`: flm.ext.handler-failed: "), "{text}");
    assert!(text.contains("0.5"), "{text}");
}
