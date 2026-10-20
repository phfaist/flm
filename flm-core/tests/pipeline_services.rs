//! Service slots and two-round registration (`flm_core::core::services`): slot
//! arity, contribution order, the roster errors, the embedder's contributions,
//! what a running stage reads, and finalizers deriving entities from every
//! feature's contributions.

mod common;

use std::sync::Arc;

use flm_core::core::registry::{
    BuildError, BuildPhase, FeatureBuilder, Finalizer, HOST_FEATURE, RegistryBuilder,
    RegistryInputs, RoundTwoAction,
};
use flm_core::core::services::{Arity, ServiceError, SlotTarget};
use flm_core::core::stage::{Effect, Needs, Scope, StageDecl};

use common::pipeline::{
    LOG, Log, doc, feature, plan, provider, registry, registry_error, run, stage,
};
use flm_core::core::run::RunOptions;

/// The endnotes-shaped slot value: plain data contributed by several features.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Category(String);

/// A behavior-shaped slot value.
trait Greeter: Send + Sync {
    fn greet(&self) -> String;
}

struct Hello(&'static str);

impl Greeter for Hello {
    fn greet(&self) -> String {
        format!("hello from {}", self.0)
    }
}

fn service_error(error: flm_core::core::registry::RegistryBuildError) -> ServiceError {
    assert_eq!(error.phase, BuildPhase::Collect);
    assert_eq!(error.errors.len(), 1, "one error expected:\n{}", error.render());
    match error.errors.into_iter().next().unwrap() {
        BuildError::Service(error) => error,
        other => panic!("unexpected error: {other:?}"),
    }
}

// --- slots and contributions ------------------------------------------------------------------

#[test]
fn a_single_slot_refuses_a_second_filler() {
    let error = service_error(registry_error(vec![
        feature("t.a", |b| {
            b.service_slot::<String>("t.toc-builder", Arity::Single, "the one builder")?;
            b.contribute_to("t.toc-builder", Arc::new("builtin".to_string()))
        }),
        feature("t.b", |b| b.contribute_to("t.toc-builder", Arc::new("fancy".to_string()))),
    ]));
    match error {
        ServiceError::SingleSlotRefilled { slot, first, second } => {
            assert_eq!(slot, "t.toc-builder");
            assert_eq!(first, "t.a");
            assert_eq!(second, "t.b");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn ordered_many_keeps_registration_order_with_prepends_in_front() {
    let registry = registry(vec![
        feature("t.a", |b| {
            b.service_slot::<String>("t.hooks", Arity::OrderedMany, "hooks")?;
            b.contribute_to("t.hooks", Arc::new("a1".to_string()))?;
            b.contribute_first_to("t.hooks", Arc::new("a-first".to_string()))
        }),
        feature("t.b", |b| {
            b.contribute_to("t.hooks", Arc::new("b1".to_string()))?;
            b.contribute_first_to("t.hooks", Arc::new("b-first".to_string()))?;
            b.contribute_to("t.hooks", Arc::new("b2".to_string()))
        }),
    ]);
    // Prepends first, in their own registration order; then the appends, in
    // theirs (a prepend never jumps ahead of an earlier prepend).
    let hooks: Vec<&str> =
        registry.services().services_named::<String>("t.hooks").map(String::as_str).collect();
    assert_eq!(hooks, ["a-first", "b-first", "a1", "b1", "b2"]);
    assert_eq!(registry.services().count("t.hooks"), 5);
}

#[test]
fn contributing_to_an_undeclared_slot_lists_the_roster() {
    let error = service_error(registry_error(vec![
        feature("t.a", |b| {
            b.service_slot::<String>("t.hooks", Arity::OrderedMany, "hooks")?;
            b.service_slot::<u32>("t.limits", Arity::Single, "limits")
        }),
        feature("t.b", |b| b.contribute_to("t.hookz", Arc::new("oops".to_string()))),
    ]));
    match error {
        ServiceError::UndeclaredSlot { feature, target, roster } => {
            assert_eq!(feature, "t.b");
            assert_eq!(target, SlotTarget::ByName("t.hookz".into()));
            assert_eq!(roster, ["t.hooks", "t.limits"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// --- peer contributions ---------------------------------------------------------------------

/// The identifiers of a registry's build diagnostics.
fn build_diagnostic_ids(
    registry: &flm_core::core::registry::FlmRegistry,
) -> Vec<&str> {
    registry.build_diagnostics().iter().map(|d| d.identifier()).collect()
}

/// Every peer contribution of a registry as (feature, slot, resolved).
fn peer_slots(registry: &flm_core::core::registry::FlmRegistry) -> Vec<(&str, &str, bool)> {
    registry
        .peer_slots()
        .iter()
        .map(|entry| (entry.owner.as_str(), entry.slot.as_str(), entry.resolved))
        .collect()
}

#[test]
fn a_peer_contribution_fills_a_slot_another_feature_declares() {
    // `t.b` does not own `t.hooks` and says so; the slot is there, so the
    // value lands in it exactly as an ordinary contribution would, after the
    // owner's own.
    let registry = registry(vec![
        feature("t.a", |b| {
            b.service_slot::<String>("t.hooks", Arity::OrderedMany, "hooks")?;
            b.contribute_to("t.hooks", Arc::new("a1".to_string()))
        }),
        feature("t.b", |b| b.peer_slot("t.hooks", Arc::new("b-peer".to_string()))),
    ]);
    let hooks: Vec<&str> =
        registry.services().services_named::<String>("t.hooks").map(String::as_str).collect();
    assert_eq!(hooks, ["a1", "b-peer"]);
    assert_eq!(peer_slots(&registry), [("t.b", "t.hooks", true)]);
    assert!(registry.build_diagnostics().is_empty());
}

#[test]
fn a_peer_contribution_to_a_slot_nobody_declares_is_dropped_with_a_note() {
    // The same feature without the slot's owner: the build succeeds, the value
    // is nowhere, and the registry says which slot went unresolved.
    let registry =
        registry(vec![feature("t.b", |b| b.peer_slot("t.hooks", Arc::new("b-peer".to_string())))]);
    assert_eq!(registry.services().count("t.hooks"), 0);
    assert!(registry.services().service::<String>().is_none());
    assert_eq!(peer_slots(&registry), [("t.b", "t.hooks", false)]);
    assert_eq!(build_diagnostic_ids(&registry), ["flm.registry.unresolved-peer"]);
    assert!(!registry.build_diagnostics().has_errors());
    assert_eq!(
        registry.build_diagnostics().iter().next().expect("the note").message(),
        "feature `t.b` refers to the slot `t.hooks` and expects another feature to declare it, but no feature of this registry declares it; the value contributed to it is dropped"
    );
}

#[test]
fn several_peer_contributions_to_one_absent_slot_are_one_note() {
    // A slot holds many values, so a feature may offer it several; offering
    // them to a slot nobody declares is one mistake, and one Note. Every
    // value keeps its own record all the same.
    let registry = registry(vec![feature("t.b", |b| {
        b.peer_slot("t.hooks", Arc::new("first".to_string()))?;
        b.peer_slot("t.hooks", Arc::new("second".to_string()))?;
        b.peer_slot("t.styles", Arc::new("other slot".to_string()))
    })]);
    assert_eq!(
        build_diagnostic_ids(&registry),
        ["flm.registry.unresolved-peer", "flm.registry.unresolved-peer"]
    );
    let messages: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message()).collect();
    assert!(messages[0].contains("refers to the slot `t.hooks`"), "{}", messages[0]);
    assert!(messages[1].contains("refers to the slot `t.styles`"), "{}", messages[1]);
    assert_eq!(
        peer_slots(&registry),
        [("t.b", "t.hooks", false), ("t.b", "t.hooks", false), ("t.b", "t.styles", false)]
    );
}

#[test]
fn a_peer_contribution_of_the_wrong_type_is_refused_like_any_other() {
    // Saying the slot belongs to a peer excuses an absent slot, nothing else:
    // a slot that is there is filled under the usual rules.
    let error = service_error(registry_error(vec![
        feature("t.a", |b| {
            b.service_slot::<Category>("t.categories", Arity::OrderedMany, "categories")
        }),
        feature("t.b", |b| b.peer_slot("t.categories", Arc::new("oops".to_string()))),
    ]));
    match error {
        ServiceError::TypeMismatch { feature, slot, .. } => {
            assert_eq!(feature, "t.b");
            assert_eq!(slot, "t.categories");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_peer_contribution_to_a_single_slot_already_filled_is_refused() {
    let error = service_error(registry_error(vec![
        feature("t.a", |b| {
            b.service_slot::<String>("t.toc-builder", Arity::Single, "the one builder")?;
            b.contribute_to("t.toc-builder", Arc::new("builtin".to_string()))
        }),
        feature("t.b", |b| b.peer_slot("t.toc-builder", Arc::new("fancy".to_string()))),
    ]));
    match error {
        ServiceError::SingleSlotRefilled { slot, first, second } => {
            assert_eq!(slot, "t.toc-builder");
            assert_eq!(first, "t.a");
            assert_eq!(second, "t.b");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_required_slot_left_unfilled_fails_the_build() {
    let error = service_error(registry_error(vec![feature("t.a", |b| {
        b.service_slot::<String>("t.toc-builder", Arity::Single, "the one builder")?;
        b.require_service_named("t.toc-builder")
    })]));
    match error {
        ServiceError::RequiredSlotUnfilled { slot, required_by } => {
            assert_eq!(slot, "t.toc-builder");
            assert_eq!(required_by, "t.a");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_required_slot_filled_by_any_feature_builds() {
    let registry = registry(vec![
        feature("t.a", |b| {
            b.service_slot::<String>("t.toc-builder", Arity::Single, "the one builder")?;
            b.require_service::<String>()
        }),
        feature("t.b", |b| b.contribute(Arc::new("fancy".to_string()))),
    ]);
    assert_eq!(registry.services().service::<String>().map(String::as_str), Some("fancy"));
}

#[test]
fn a_contribution_of_the_wrong_type_is_refused() {
    let error = service_error(registry_error(vec![feature("t.a", |b| {
        b.service_slot::<Category>("t.categories", Arity::OrderedMany, "categories")?;
        b.contribute_to("t.categories", Arc::new("footnote".to_string()))
    })]));
    match error {
        ServiceError::TypeMismatch { feature, slot, declared, contributed } => {
            assert_eq!(feature, "t.a");
            assert_eq!(slot, "t.categories");
            assert_eq!(declared.short_name(), "Category");
            assert_eq!(contributed.short_name(), "String");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn slot_declaration_and_contribution_are_order_independent_within_round_one() {
    // The contributing feature registers before the declaring one: assembly
    // is validated after the whole round.
    let registry = registry(vec![
        feature("t.addendum", |b| b.contribute(Arc::new(Category("addendum".into())))),
        feature("t.endnotes", |b| {
            b.service_slot::<Category>("t.categories", Arity::OrderedMany, "categories")?;
            b.contribute(Arc::new(Category("footnote".into())))
        }),
    ]);
    let names: Vec<&str> =
        registry.services().services::<Category>().map(|c| c.0.as_str()).collect();
    assert_eq!(names, ["addendum", "footnote"]);
}

#[test]
fn a_contribution_by_type_is_ambiguous_when_two_slots_carry_it() {
    let error = service_error(registry_error(vec![feature("t.a", |b| {
        b.service_slot::<String>("t.one", Arity::Single, "one")?;
        b.service_slot::<String>("t.two", Arity::Single, "two")?;
        b.contribute(Arc::new("which?".to_string()))
    })]));
    match error {
        ServiceError::AmbiguousSlot { feature, ty, slots } => {
            assert_eq!(feature, "t.a");
            assert_eq!(ty.short_name(), "String");
            assert_eq!(slots, ["t.one", "t.two"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn two_features_declaring_one_slot_is_an_error_naming_both() {
    let error = service_error(registry_error(vec![
        feature("t.a", |b| b.service_slot::<String>("t.hooks", Arity::OrderedMany, "hooks")),
        feature("t.b", |b| b.service_slot::<String>("t.hooks", Arity::OrderedMany, "hooks")),
    ]));
    match error {
        ServiceError::DuplicateSlot { slot, first, second } => {
            assert_eq!(slot, "t.hooks");
            assert_eq!(first, "t.a");
            assert_eq!(second, "t.b");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn the_embedder_contributes_through_the_registry_builder() {
    let mut builder = RegistryBuilder::new();
    builder.feature(feature("t.a", |b| {
        b.service_slot::<dyn Greeter>("t.greeter", Arity::Single, "the greeter")
    }));
    builder.contribute::<dyn Greeter>(Arc::new(Hello("the host")));
    let registry = builder.build(&RegistryInputs::new()).expect("the host fills the slot");
    assert_eq!(
        registry.services().service::<dyn Greeter>().map(Greeter::greet),
        Some("hello from the host".to_string())
    );
    // A second host contribution to a single slot is attributed to the host.
    let mut builder = RegistryBuilder::new();
    builder.feature(feature("t.a", |b| {
        b.service_slot::<dyn Greeter>("t.greeter", Arity::Single, "the greeter")
    }));
    builder.contribute::<dyn Greeter>(Arc::new(Hello("one")));
    builder.contribute_to::<dyn Greeter>("t.greeter", Arc::new(Hello("two")));
    let error = builder.build(&RegistryInputs::new()).expect_err("twice");
    match service_error(error) {
        ServiceError::SingleSlotRefilled { first, second, .. } => {
            assert_eq!(first, HOST_FEATURE);
            assert_eq!(second, HOST_FEATURE);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// --- what a running stage reads ------------------------------------------------------------------

/// A registry whose stage `t.greet` writes the greetings it reads into the log.
fn greeting_registry(fill: bool) -> Arc<flm_core::core::registry::FlmRegistry> {
    registry(vec![feature("t", move |b| {
        b.service_slot::<dyn Greeter>("t.greeters", Arity::OrderedMany, "greeters")?;
        if fill {
            b.contribute::<dyn Greeter>(Arc::new(Hello("one")))?;
            b.contribute::<dyn Greeter>(Arc::new(Hello("two")))?;
        }
        b.table(LOG, "what the stage saw")?;
        b.stage(stage(
            StageDecl::new("t.greet", Effect::Compute, Scope::Document).writes(LOG),
            Needs::new(),
            |cx, out| {
                let log = out.table(LOG).expect("declared");
                log.0.push(match cx.service::<dyn Greeter>() {
                    Some(greeter) => greeter.greet(),
                    None => "nobody".to_string(),
                });
                log.0.extend(cx.services::<dyn Greeter>().map(Greeter::greet));
            },
        ))
    })])
}

#[test]
fn a_stage_reads_the_slots_contents_in_assembled_order() {
    let registry = greeting_registry(true);
    let run = run(&doc(&registry, &["x"]), &plan(&registry, &["t.log"]), &RunOptions::new());
    let log: &Log = run.table(LOG).unwrap().expect("written");
    assert_eq!(log.0, ["hello from one", "hello from one", "hello from two"]);
}

#[test]
fn an_unfilled_slot_reads_none_at_run_time() {
    // Not requiring a slot means a scheduled stage sees `None` and reports its
    // own diagnostic; the run itself never fails.
    let registry = greeting_registry(false);
    let run = run(&doc(&registry, &["x"]), &plan(&registry, &["t.log"]), &RunOptions::new());
    let log: &Log = run.table(LOG).unwrap().expect("written");
    assert_eq!(log.0, ["nobody"]);
}

// --- two-round registration ----------------------------------------------------------------------

struct DeriveStages;

impl Finalizer for DeriveStages {
    fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        // Round two reads the *assembled* slot: every feature's contributions
        // are in, including a feature registered after this one.
        let names: Vec<String> =
            b.services().expect("round two").services::<Category>().map(|c| c.0.clone()).collect();
        for name in names {
            b.stage(provider(&format!("t.endnotes.{name}"), &[], &[]))?;
        }
        Ok(())
    }
}

#[test]
fn a_finalizer_derives_entities_from_every_features_contributions() {
    let registry = registry(vec![
        feature("t.endnotes", |b| {
            b.service_slot::<Category>("t.categories", Arity::OrderedMany, "categories")?;
            b.contribute(Arc::new(Category("footnote".into())))?;
            b.stage(provider("t.endnotes.mark", &[], &[]))?;
            b.finalizer(Arc::new(DeriveStages))
        }),
        feature("t.addendum", |b| {
            b.contribute(Arc::new(Category("addendum".into())))?;
            b.stage(provider("t.addendum.collect", &[], &[]))
        }),
    ]);
    // Round-one entities in feature order, then the round-two derivations,
    // attributed to the finalizing feature — not to the contributor.
    let stages: Vec<(String, String)> =
        registry.stages().iter().map(|s| (s.name.to_string(), s.owner.to_string())).collect();
    assert_eq!(
        stages,
        [
            ("t.endnotes.mark".to_string(), "t.endnotes".to_string()),
            ("t.addendum.collect".to_string(), "t.addendum".to_string()),
            ("t.endnotes.footnote".to_string(), "t.endnotes".to_string()),
            ("t.endnotes.addendum".to_string(), "t.endnotes".to_string()),
        ]
    );
}

struct LateContribution;

impl Finalizer for LateContribution {
    fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.contribute(Arc::new(Category("too-late".into())))
    }
}

#[test]
fn a_finalizer_cannot_contribute_to_a_slot() {
    let error = registry_error(vec![feature("t.endnotes", |b| {
        b.service_slot::<Category>("t.categories", Arity::OrderedMany, "categories")?;
        b.finalizer(Arc::new(LateContribution))
    })]);
    assert_eq!(error.phase, BuildPhase::Finalize);
    assert_eq!(error.errors.len(), 1, "{}", error.render());
    match &error.errors[0] {
        BuildError::ContributionAfterAssembly { feature, action } => {
            assert_eq!(feature, "t.endnotes");
            assert_eq!(*action, RoundTwoAction::Contribute);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

struct LatePeerContribution;

impl Finalizer for LatePeerContribution {
    fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.peer_slot("t.categories", Arc::new(Category("late".to_string())))
    }
}

#[test]
fn a_finalizer_cannot_make_a_peer_contribution_either() {
    // Slots are assembled between the two rounds, so a peer contribution is
    // as late in round two as an ordinary one.
    let error = registry_error(vec![feature("t.endnotes", |b| {
        b.service_slot::<Category>("t.categories", Arity::OrderedMany, "categories")?;
        b.finalizer(Arc::new(LatePeerContribution))
    })]);
    assert_eq!(error.phase, BuildPhase::Finalize);
    match &error.errors[0] {
        BuildError::ContributionAfterAssembly { feature, action } => {
            assert_eq!(feature, "t.endnotes");
            assert_eq!(*action, RoundTwoAction::Contribute);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

struct LateSlot;

impl Finalizer for LateSlot {
    fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.service_slot::<String>("t.late", Arity::Single, "declared too late")
    }
}

#[test]
fn a_finalizer_cannot_declare_a_slot() {
    let error = registry_error(vec![feature("t", |b| b.finalizer(Arc::new(LateSlot)))]);
    assert_eq!(error.phase, BuildPhase::Finalize);
    assert!(
        matches!(&error.errors[0], BuildError::ContributionAfterAssembly { action, .. }
            if *action == RoundTwoAction::DeclareSlot),
        "{}",
        error.render()
    );
}

struct FromA;

impl Finalizer for FromA {
    fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.stage(provider("t.from-a", &[], &[]))
    }
}

struct FromB;

impl Finalizer for FromB {
    fn finalize(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        b.stage(provider("t.from-b", &[], &[]))
    }
}

#[test]
fn finalizers_run_in_feature_order() {
    let registry = registry(vec![
        feature("t.b", |b| b.finalizer(Arc::new(FromB))),
        feature("t.a", |b| b.finalizer(Arc::new(FromA))),
    ]);
    let names: Vec<String> = registry.stages().iter().map(|s| s.name.to_string()).collect();
    assert_eq!(names, ["t.from-b", "t.from-a"]);
}
