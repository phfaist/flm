//! Plan construction beyond provider selection (`flm_core::core::plan`): the
//! stage order and the batches, the ordering edges, the cycle error naming the
//! cycle, two writers of one table, the rewrites (their total order, the tie
//! error listing the resolving edges, computed reads refused, refinements and
//! marked defaults), `Io` placement, completeness under the selected rules and
//! the accepted holes, the lowering ladder branch by branch, and workflows.

mod common;

use std::sync::Arc;

use flm_core::core::artifact::ArtifactSet;
use flm_core::core::lowering::{DefaultMark, LoweringDecl};
use flm_core::core::plan::{
    LoweringSelection, Overrides, Plan, PlanError, PlanRequest, PlanWarning,
};
use flm_core::core::registry::{
    BackendDef, BuildError, BuildPhase, CapabilityDef, FlmFeature, FlmRegistry, OrderingDef,
    OutputDef, RegistryBuilder, RegistryInputs, WorkflowDef, HOST_FEATURE,
};
use flm_core::stdcore::backends::lowering::{LoweredTrees, LoweringStage, lowered_table_name};
use flm_core::core::stage::{Effect, Needs, RewriteDecl, Scope, StageDecl, TreesTable};
use flm_core::core::data::table::TableName;

use common::macro_def;
use common::pipeline::{
    COUNT, Count, LOG, Log, feature, lowered_cap, noop, output_cap, plan, plan_for, provider,
    register_backend, registry, registry_error, rewrite, rule, toy_registry,
};

fn names(names: &[flm_core::model::names::StageName]) -> Vec<String> {
    names.iter().map(|n| n.to_string()).collect()
}

// --- order and batches ---------------------------------------------------------------------------

/// `scan` provides `t.a`; `num`, `stats` and `check` each require it and provide
/// their own capability; registered in that order.
fn fan_out() -> Arc<FlmRegistry> {
    toy_registry(
        &[("t.a", None, false), ("t.n", None, false), ("t.s", None, false), ("t.c", None, false)],
        &[
            ("t.scan", vec!["t.a"], vec![]),
            ("t.num", vec!["t.n"], vec!["t.a"]),
            ("t.stats", vec!["t.s"], vec!["t.a"]),
            ("t.check", vec!["t.c"], vec!["t.a"]),
        ],
    )
}

#[test]
fn stage_order_is_topological_with_a_registration_order_tie_break() {
    let plan = plan(&fan_out(), &["t.c", "t.s", "t.n"]);
    assert_eq!(names(&plan.stage_names()), ["t.scan", "t.num", "t.stats", "t.check"]);
}

#[test]
fn independent_stages_batch_together() {
    let plan = plan(&fan_out(), &["t.c", "t.s", "t.n"]);
    let batches: Vec<Vec<String>> =
        plan.batch_names().into_iter().map(|batch| names(&batch)).collect();
    assert_eq!(
        batches,
        [vec!["t.scan".to_string()], vec!["t.num".into(), "t.stats".into(), "t.check".into()]]
    );
    assert_eq!(plan.first_io_batch(), None);
}

#[test]
fn a_dependency_is_a_predecessor_edge() {
    let registry = fan_out();
    let plan = plan(&registry, &["t.n"]);
    let scan = registry.stage_idx("t.scan").unwrap();
    let num = registry.stage_idx("t.num").unwrap();
    assert_eq!(plan.predecessors(num).unwrap().iter().copied().collect::<Vec<_>>(), [scan]);
    assert!(plan.predecessors(scan).unwrap().is_empty());
}

// --- ordering edges ------------------------------------------------------------------------------

/// `t.p` provides `t.a`; `t.x` provides `t.b` with the given ordering-only
/// needs; registered `t.x` first so that only an edge can put it second.
fn edged(needs: Needs) -> Arc<FlmRegistry> {
    registry(vec![feature("t", move |b| {
        b.capability(CapabilityDef::new("t.a"))?;
        b.capability(CapabilityDef::new("t.b"))?;
        b.stage(noop(
            StageDecl::new("t.x", Effect::Compute, Scope::Document).provides("t.b"),
            needs.clone(),
        ))?;
        b.stage(provider("t.p", &["t.a"], &[]))
    })])
}

#[test]
fn an_after_edge_orders_without_activating() {
    let registry = edged(Needs::new().after("t.a"));
    // Ordering-only: the provider of `t.a` is not pulled in.
    assert_eq!(names(&plan(&registry, &["t.b"]).stage_names()), ["t.x"]);
    // Both scheduled: the edge puts `t.x` after `t.p` despite registration order.
    assert_eq!(names(&plan(&registry, &["t.a", "t.b"]).stage_names()), ["t.p", "t.x"]);
}

#[test]
fn a_before_edge_orders_the_other_way() {
    let registry = registry(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("t.a"))?;
        b.capability(CapabilityDef::new("t.b"))?;
        b.stage(provider("t.p", &["t.a"], &[]))?;
        b.stage(noop(
            StageDecl::new("t.x", Effect::Compute, Scope::Document).provides("t.b"),
            Needs::new().before("t.a"),
        ))
    })]);
    assert_eq!(names(&plan(&registry, &["t.a", "t.b"]).stage_names()), ["t.x", "t.p"]);
}

#[test]
fn a_third_features_ordering_edge_applies() {
    let registry = registry(vec![
        feature("t", |b| {
            b.capability(CapabilityDef::new("t.a"))?;
            b.capability(CapabilityDef::new("t.b"))?;
            b.stage(provider("t.x", &["t.b"], &[]))?;
            b.stage(provider("t.p", &["t.a"], &[]))
        }),
        feature("t.other", |b| b.ordering(OrderingDef::new("t.x").after("t.a"))),
    ]);
    assert_eq!(names(&plan(&registry, &["t.a", "t.b"]).stage_names()), ["t.p", "t.x"]);
}

#[test]
fn an_edge_on_a_parent_capability_follows_the_active_refinement() {
    let registry = registry(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("t.r"))?;
        b.capability(CapabilityDef::new("t.r.fast").refines("t.r"))?;
        b.capability(CapabilityDef::new("t.r.slow").refines("t.r"))?;
        b.capability(CapabilityDef::new("t.b"))?;
        b.stage(noop(
            StageDecl::new("t.x", Effect::Compute, Scope::Document).provides("t.b"),
            Needs::new().after("t.r"),
        ))?;
        b.stage(provider("t.fast", &["t.r.fast"], &[]))?;
        b.stage(provider("t.slow", &["t.r.slow"], &[]))
    })]);
    assert_eq!(names(&plan(&registry, &["t.r.slow", "t.b"]).stage_names()), ["t.slow", "t.x"]);
    assert_eq!(names(&plan(&registry, &["t.r.fast", "t.b"]).stage_names()), ["t.fast", "t.x"]);
}

// --- cycles and writers --------------------------------------------------------------------------

fn ping_pong() -> Arc<FlmRegistry> {
    toy_registry(
        &[("t.ping", None, false), ("t.pong", None, false), ("t.other", None, false)],
        &[
            ("t.pong-stage", vec!["t.pong"], vec!["t.ping"]),
            ("t.ping-stage", vec!["t.ping"], vec!["t.pong"]),
            ("t.other-stage", vec!["t.other"], vec![]),
        ],
    )
}

#[test]
fn a_cycle_among_the_selected_stages_is_a_typed_error_naming_the_cycle() {
    match Plan::build(&ping_pong(), &PlanRequest::new().demand("t.ping")).expect_err("a cycle") {
        PlanError::Cycle { stages, .. } => {
            // Closed, in dependency order, rotated to its name-least member.
            assert_eq!(names(&stages), ["t.ping-stage", "t.pong-stage", "t.ping-stage"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_cycle_needs_a_demand_to_become_real() {
    // The registry accepts the pair; only a demand reaching it makes the cycle.
    assert_eq!(names(&plan(&ping_pong(), &["t.other"]).stage_names()), ["t.other-stage"]);
}

#[test]
fn two_writers_of_one_table_are_mutually_exclusive_by_atomicity() {
    // One writer per table and one provider per capability are one rule — a
    // written table provides its capability, so committing
    // one writer excludes the other, and demanding both is the ordinary
    // atomicity error rather than a separate two-writers failure.
    let registry = registry(vec![feature("t", |b| {
        b.table(COUNT, "a count")?;
        b.capability(CapabilityDef::new("t.a"))?;
        b.capability(CapabilityDef::new("t.b"))?;
        b.stage(noop(
            StageDecl::new("t.first", Effect::Compute, Scope::Document)
                .provides("t.a")
                .writes(COUNT),
            Needs::new(),
        ))?;
        b.stage(noop(
            StageDecl::new("t.second", Effect::Compute, Scope::Document)
                .provides("t.b")
                .writes(COUNT),
            Needs::new(),
        ))
    })]);
    match Plan::build(&registry, &PlanRequest::new().demand("t.a").demand("t.b"))
        .expect_err("exclusive")
    {
        PlanError::Unsatisfiable { capability, blocked, .. } => {
            assert_eq!(capability, "t.b");
            assert_eq!(blocked.len(), 1);
            assert_eq!(blocked[0].stage, "t.second");
            assert_eq!(blocked[0].excluded_by, "t.first");
            assert_eq!(blocked[0].over, "t.count");
        }
        other => panic!("unexpected error: {other:?}"),
    }
    // Demanding the table's capability alone is a contest between the writers.
    assert!(matches!(
        Plan::build(&registry, &PlanRequest::new().demand("t.count")),
        Err(PlanError::Contested { .. })
    ));
    assert_eq!(names(&plan(&registry, &["t.a"]).stage_names()), ["t.first"]);
}

// --- rewrites ------------------------------------------------------------------------------------

fn identity_rewrite(
    name: &str,
    publishes: &str,
    needs: Needs,
) -> Arc<dyn flm_core::core::stage::RewriteStage> {
    rewrite(RewriteDecl::new(name, publishes), needs, |_, tree, _, _| Ok(Arc::clone(tree)))
}

#[test]
fn active_rewrites_are_totally_ordered_by_their_edges() {
    let registry = registry(vec![feature("t", |b| {
        b.rewrite_stage(identity_rewrite("t.second", "t.two", Needs::new().after("t.one")))?;
        b.rewrite_stage(identity_rewrite("t.first", "t.one", Needs::new()))
    })]);
    let plan = plan(&registry, &["t.one", "t.two"]);
    assert_eq!(names(&plan.rewrite_names()), ["t.first", "t.second"]);
    assert!(plan.stage_names().is_empty());
}

#[test]
fn a_rewrite_is_active_only_when_demanded_or_required() {
    let registry = registry(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("t.a"))?;
        b.rewrite_stage(identity_rewrite("t.rw", "t.rewritten", Needs::new()))?;
        b.stage(provider("t.p", &["t.a"], &[]))?;
        b.stage(provider("t.q", &["t.b"], &["t.rewritten"]))?;
        b.capability(CapabilityDef::new("t.b"))
    })]);
    assert!(plan(&registry, &["t.a"]).rewrites().is_empty());
    assert_eq!(names(&plan(&registry, &["t.b"]).rewrite_names()), ["t.rw"]);
}

#[test]
fn unordered_active_rewrites_are_a_typed_tie_listing_the_resolving_edges() {
    let registry = registry(vec![feature("t", |b| {
        b.rewrite_stage(identity_rewrite("t.first", "t.one", Needs::new()))?;
        b.rewrite_stage(identity_rewrite("t.second", "t.two", Needs::new()))
    })]);
    match Plan::build(&registry, &PlanRequest::new().demand("t.one").demand("t.two"))
        .expect_err("a tie")
    {
        PlanError::RewriteTie { active, first, second, resolving, .. } => {
            assert_eq!(names(&active), ["t.first", "t.second"]);
            assert_eq!(first, "t.first");
            assert_eq!(second, "t.second");
            assert_eq!(
                resolving,
                [
                    OrderingDef::new("t.second").after("t.one"),
                    OrderingDef::new("t.first").after("t.two"),
                ]
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_contributed_edge_resolves_a_rewrite_tie() {
    let registry = registry(vec![
        feature("t", |b| {
            b.rewrite_stage(identity_rewrite("t.first", "t.one", Needs::new()))?;
            b.rewrite_stage(identity_rewrite("t.second", "t.two", Needs::new()))
        }),
        feature("t.order", |b| b.ordering(OrderingDef::new("t.first").after("t.two"))),
    ]);
    assert_eq!(
        names(&plan(&registry, &["t.one", "t.two"]).rewrite_names()),
        ["t.second", "t.first"]
    );
}

#[test]
fn a_rewrite_declaring_a_computed_read_is_refused() {
    let registry = registry(vec![feature("t", |b| {
        b.table(COUNT, "a count")?;
        b.rewrite_stage(identity_rewrite("t.rw", "t.rewritten", Needs::new().reads(COUNT)))
    })]);
    match Plan::build(&registry, &PlanRequest::new()).expect_err("no computed data") {
        PlanError::RewriteReads { stage, table, .. } => {
            assert_eq!(stage, "t.rw");
            assert_eq!(table, "t.count");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_rewrite_requiring_computed_data_is_refused() {
    let registry = registry(vec![feature("t", |b| {
        b.table(COUNT, "a count")?;
        b.stage(noop(
            StageDecl::new("t.counter", Effect::Compute, Scope::PerTree).writes(COUNT),
            Needs::new(),
        ))?;
        b.rewrite_stage(identity_rewrite("t.rw", "t.rewritten", Needs::new().requires("t.count")))
    })]);
    match Plan::build(&registry, &PlanRequest::new().demand("t.rewritten")).expect_err("computed") {
        PlanError::RewriteNeedsComputed { stage, capability, provider, .. } => {
            assert_eq!(stage, "t.rw");
            assert_eq!(capability, "t.count");
            assert_eq!(provider, "t.counter");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

/// The blocks shape: `t.structure` (a rewrite publishing `t.blocks.structured`,
/// refining `t.blocks`, the default) against the marker `t.raw` (providing
/// `t.blocks.raw`, refining `t.blocks`).
fn blocks_registry() -> Arc<FlmRegistry> {
    registry(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("t.blocks"))?;
        b.capability(CapabilityDef::new("t.blocks.raw").refines("t.blocks"))?;
        b.rewrite_stage(rewrite(
            RewriteDecl::new("t.structure", "t.blocks.structured").refines("t.blocks").default(),
            Needs::new(),
            |_, tree, _, _| Ok(Arc::clone(tree)),
        ))?;
        b.stage(provider("t.raw", &["t.blocks.raw"], &[]))
    })])
}

#[test]
fn a_rewrites_trees_table_refines_its_declared_parent() {
    let registry = blocks_registry();
    let structured = registry.capability_named("t.blocks.structured").expect("declared");
    assert_eq!(structured.refines, registry.cap_id("t.blocks"));
    assert!(structured.default);
    assert!(structured.table.is_some());
}

#[test]
fn the_marked_default_rewrite_answers_the_parent_demand() {
    let plan = plan(&blocks_registry(), &["t.blocks"]);
    assert_eq!(names(&plan.rewrite_names()), ["t.structure"]);
    assert!(plan.stage_names().is_empty());
}

#[test]
fn demanding_the_marker_freezes_the_parsed_tree() {
    let plan = plan(&blocks_registry(), &["t.blocks.raw"]);
    assert!(plan.rewrites().is_empty());
    assert_eq!(names(&plan.stage_names()), ["t.raw"]);
}

#[test]
fn demanding_both_alternatives_is_the_atomicity_error() {
    let request = PlanRequest::new().demand("t.blocks.structured").demand("t.blocks.raw");
    match Plan::build(&blocks_registry(), &request).expect_err("exclusive") {
        PlanError::Unsatisfiable { capability, blocked, .. } => {
            assert_eq!(capability, "t.blocks.structured");
            assert_eq!(blocked.len(), 1);
            assert_eq!(blocked[0].stage, "t.structure");
            assert_eq!(blocked[0].excluded_by, "t.raw");
            assert_eq!(blocked[0].over, "t.blocks");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_compute_stage_may_read_a_rewrites_trees_table() {
    let registry = registry(vec![feature("t", |b| {
        b.rewrite_stage(identity_rewrite("t.rw", "t.rewritten", Needs::new()))?;
        b.table(LOG, "a log")?;
        b.stage(noop(
            StageDecl::new("t.peek", Effect::Compute, Scope::Document).writes(LOG),
            Needs::new().reads(TableName::<TreesTable>::new("t.rewritten")),
        ))
    })]);
    let plan = plan(&registry, &["t.log"]);
    assert_eq!(names(&plan.rewrite_names()), ["t.rw"]);
    let peek = registry.stage_idx("t.peek").unwrap();
    let reads = &plan.manifest(peek).unwrap().reads;
    assert_eq!(
        reads.get("t.rewritten").unwrap().producer.as_deref().map(|n| n.as_str()),
        Some("t.rw")
    );
}

// --- Io placement --------------------------------------------------------------------------------

/// An `Io` stage `t.write` registered first, then two independent `Compute`
/// stages; with `dependent`, a third `Compute` stage requiring the `Io`
/// stage's capability.
fn io_registry(dependent: bool) -> Arc<FlmRegistry> {
    registry(vec![feature("t", move |b| {
        b.capability(CapabilityDef::new("t.written"))?;
        b.capability(CapabilityDef::new("t.a"))?;
        b.capability(CapabilityDef::new("t.b"))?;
        b.capability(CapabilityDef::new("t.c"))?;
        b.stage(noop(
            StageDecl::new("t.write", Effect::Io, Scope::Document).provides("t.written"),
            Needs::new(),
        ))?;
        b.stage(provider("t.pa", &["t.a"], &[]))?;
        b.stage(provider("t.pb", &["t.b"], &[]))?;
        if dependent {
            b.stage(provider("t.pc", &["t.c"], &["t.written"]))?;
        }
        Ok(())
    })])
}

#[test]
fn io_stages_come_after_every_pure_stage() {
    let plan = plan(&io_registry(false), &["t.written", "t.a", "t.b"]);
    assert_eq!(names(&plan.stage_names()), ["t.pa", "t.pb", "t.write"]);
}

#[test]
fn batches_never_mix_io_with_pure_stages() {
    let plan = plan(&io_registry(false), &["t.written", "t.a", "t.b"]);
    let batches: Vec<Vec<String>> =
        plan.batch_names().into_iter().map(|batch| names(&batch)).collect();
    assert_eq!(
        batches,
        [vec!["t.pa".to_string(), "t.pb".to_string()], vec!["t.write".to_string()]]
    );
    assert_eq!(plan.first_io_batch(), Some(1));
}

#[test]
fn an_io_product_read_by_a_pure_stage_is_a_typed_error() {
    match Plan::build(&io_registry(true), &PlanRequest::new().demand("t.c"))
        .expect_err("Io product")
    {
        PlanError::IoProductReadByPure { reader, io_stage, .. } => {
            assert_eq!(reader, "t.pc");
            assert_eq!(io_stage, "t.write");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_io_stage_may_read_another_ios_product() {
    let registry = registry(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("t.written"))?;
        b.capability(CapabilityDef::new("t.uploaded"))?;
        b.stage(noop(
            StageDecl::new("t.upload", Effect::Io, Scope::Document).provides("t.uploaded"),
            Needs::new().requires("t.written"),
        ))?;
        b.stage(noop(
            StageDecl::new("t.write", Effect::Io, Scope::Document).provides("t.written"),
            Needs::new(),
        ))
    })]);
    let plan = plan(&registry, &["t.uploaded"]);
    assert_eq!(names(&plan.stage_names()), ["t.write", "t.upload"]);
    assert_eq!(plan.first_io_batch(), Some(0));
}

// --- what a rule's optional read schedules --------------------------------------------------

/// The backend of the three registries below. Its lowering stage is the
/// library's own, which is what turns the selected rules' reads into the
/// plan's, so these tests exercise the real scheduling rule.
const REAL_BACKEND: &str = "t-real";

/// The element `t.foo`, one rule for it that reads the table `t.count`
/// optionally and produces the native `t.pres`, and the backend `t-real` built
/// around [`LoweringStage`]. The table is another feature's, so the rule's
/// feature declares it as a peer table; the three registries below differ only
/// in who declares that table.
fn optional_reader_feature() -> Arc<dyn FlmFeature> {
    feature("t.reader", |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.element(macro_def("t", "pres", 0))?;
        b.table_named::<LoweredTrees>(
            lowered_table_name(&REAL_BACKEND.into()),
            "the lowered trees of the backend",
        )?;
        b.peer_table::<Count>(COUNT.name(), "the count another feature owns")?;
        b.lowering_rule(rule(
            "t.foo.lower.default",
            "t.foo",
            LoweringDecl::new().produces("t.pres").reads_opt(COUNT),
            DefaultMark::Everywhere,
        ))?;
        b.capability(CapabilityDef::new(output_cap(REAL_BACKEND)))?;
        let native: Vec<String> = flm_core::lang::structural::NAMES
            .iter()
            .chain(["t.pres"].iter())
            .map(|element| element.to_string())
            .collect();
        b.backend(BackendDef::new(
            REAL_BACKEND,
            native,
            Arc::new(LoweringStage::new(REAL_BACKEND)),
            noop(
                StageDecl::new("t.real.emit", Effect::Compute, Scope::Document)
                    .provides(output_cap(REAL_BACKEND)),
                Needs::new().requires(lowered_table_name(&REAL_BACKEND.into())),
            ),
            noop(
                StageDecl::new("t.real.final", Effect::Compute, Scope::Document),
                Needs::new().requires(output_cap(REAL_BACKEND)),
            ),
        ))
    })
}

/// A plan of the backend above, demanding its lowered trees.
fn real_plan(registry: &Arc<FlmRegistry>) -> Arc<Plan> {
    plan_for(
        registry,
        &PlanRequest::new().demand(lowered_table_name(&REAL_BACKEND.into())),
    )
}

/// How the plan reads `t.count` in the backend's lowering stage: whether the
/// read is optional, and which stage is scheduled to write the table.
fn count_read(plan: &Plan, registry: &FlmRegistry) -> Option<(bool, Option<String>)> {
    let idx = registry.stage_idx("flm.t-real.lower").expect("the lowering stage");
    let manifest = plan.manifest(idx).expect("its manifest");
    manifest
        .reads
        .get(COUNT.name())
        .map(|read| (read.optional, read.producer.as_ref().map(ToString::to_string)))
}

#[test]
fn an_optional_read_of_a_written_table_schedules_the_stage_that_writes_it() {
    // The table's owner is registered and a stage writes it: the rule's
    // optional read is passed on as a required read, so the writer is
    // scheduled and the rule finds the values.
    let registry = registry(vec![
        optional_reader_feature(),
        feature("t.writer", |b| {
            b.table(COUNT, "the count")?;
            b.stage(noop(
                StageDecl::new("t.count.write", Effect::Compute, Scope::Document).writes(COUNT),
                Needs::new(),
            ))
        }),
    ]);
    let plan = real_plan(&registry);
    assert!(
        names(&plan.stage_names()).contains(&"t.count.write".to_string()),
        "the writer is scheduled: {:?}",
        names(&plan.stage_names())
    );
    assert_eq!(count_read(&plan, &registry), Some((false, Some("t.count.write".to_string()))));
}

#[test]
fn an_optional_read_of_a_host_installed_table_stays_optional() {
    // Nothing can schedule a table the host installs when a run is
    // constructed, so the read stays optional and the plan asks the host for
    // nothing.
    let registry = registry(vec![
        optional_reader_feature(),
        feature("t.host", |b| {
            b.external_table(COUNT, "installed when a run is constructed")?;
            Ok(())
        }),
    ]);
    let plan = real_plan(&registry);
    assert_eq!(count_read(&plan, &registry), Some((true, None)));
    assert!(plan.external_names().is_empty(), "{:?}", plan.external_names());
}

#[test]
fn an_optional_read_of_a_table_nobody_declares_is_left_out_of_the_needs() {
    // The peer table stays unresolved: the stage does not name it, the plan is
    // built all the same, and the rule runs and sees the table as absent.
    let registry = registry(vec![optional_reader_feature()]);
    let plan = real_plan(&registry);
    assert_eq!(count_read(&plan, &registry), None);
    let selected = plan.selection(REAL_BACKEND).expect("the backend is active");
    assert!(selected.rule_for("t.foo").is_some());
}

// --- completeness and the lowering ladder --------------------------------------------------------

/// A registry with the backend `text` (native: the structural elements plus
/// `t.pres`), a source element `t.foo`, and the given rules and holes.
fn lowering_registry(
    rules: Vec<flm_core::core::lowering::LoweringRule>,
    workflow_holes: &'static [&'static str],
) -> Arc<FlmRegistry> {
    registry(vec![feature("t", move |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.element(macro_def("t", "pres", 0))?;
        b.element(macro_def("t", "mid", 0))?;
        b.capability(CapabilityDef::new("t.resolved"))?;
        b.stage(provider("t.resolve", &["t.resolved"], &[]))?;
        register_backend(b, "text", &["t.pres"])?;
        register_backend(b, "other", &["t.pres"])?;
        for rule in &rules {
            b.lowering_rule(rule.clone())?;
        }
        let mut workflow = WorkflowDef::new("text").demand(lowered_cap("text"));
        for hole in workflow_holes {
            workflow = workflow.unsupported("text", *hole);
        }
        b.workflow(workflow)
    })])
}

fn text_request() -> PlanRequest {
    PlanRequest::new().demand(lowered_cap("text"))
}

fn produces(element: &str) -> LoweringDecl {
    LoweringDecl::new().produces(element)
}

#[test]
fn an_element_neither_native_nor_ruled_is_incomplete() {
    let registry = lowering_registry(vec![], &[]);
    match Plan::build(&registry, &text_request()).expect_err("incomplete") {
        PlanError::Incomplete { backend, missing, .. } => {
            assert_eq!(backend, "text");
            assert_eq!(
                missing.iter().map(|e| e.to_string()).collect::<Vec<_>>(),
                ["t.foo", "t.mid"]
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_accepted_hole_excuses_an_element() {
    let registry = lowering_registry(vec![], &[]);
    let request = text_request().unsupported("text", "t.foo").unsupported("text", "t.mid");
    let plan = plan_for(&registry, &request);
    assert_eq!(plan.active_backends().iter().map(|b| b.to_string()).collect::<Vec<_>>(), ["text"]);
}

#[test]
fn a_rule_reaching_the_native_set_makes_an_element_complete() {
    let registry = lowering_registry(
        vec![
            rule("t.foo.lower.default", "t.foo", produces("t.pres"), DefaultMark::Everywhere),
            rule("t.mid.lower.default", "t.mid", produces("t.pres"), DefaultMark::Everywhere),
        ],
        &[],
    );
    let plan = plan_for(&registry, &text_request());
    let selected = plan.selection("text").expect("selected for");
    let name =
        |element: &str| registry.rule(selected.rule_for(element).unwrap()).rule.name.to_string();
    assert_eq!(name("t.foo"), "t.foo.lower.default");
    assert!(selected.rule_for("t.pres").is_none(), "a native element takes no rule");
}

#[test]
fn reachability_is_transitive_through_produces() {
    // `t.foo` produces `t.mid`, which produces the native `t.pres`.
    let registry = lowering_registry(
        vec![
            rule("t.foo.lower.default", "t.foo", produces("t.mid"), DefaultMark::Everywhere),
            rule("t.mid.lower.default", "t.mid", produces("t.pres"), DefaultMark::Everywhere),
        ],
        &[],
    );
    plan_for(&registry, &text_request());
}

#[test]
fn a_rule_producing_an_unreachable_element_leaves_its_element_incomplete() {
    let registry = lowering_registry(
        vec![rule("t.foo.lower.default", "t.foo", produces("t.mid"), DefaultMark::Everywhere)],
        &["t.mid"],
    );
    // `t.mid` is an accepted hole for its own occurrences, but `t.foo`'s rule
    // produces it and a hole is not a lowering: `t.foo` stays unreachable.
    match Plan::for_workflow(&registry, "text", &Overrides::none()).expect_err("incomplete") {
        PlanError::Incomplete { missing, .. } => {
            assert_eq!(missing.iter().map(|e| e.to_string()).collect::<Vec<_>>(), ["t.foo"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_produces_cycle_is_a_typed_error() {
    let registry = lowering_registry(
        vec![
            rule("t.foo.lower.default", "t.foo", produces("t.mid"), DefaultMark::Everywhere),
            rule("t.mid.lower.default", "t.mid", produces("t.foo"), DefaultMark::Everywhere),
        ],
        &[],
    );
    match Plan::build(&registry, &text_request()).expect_err("a cycle") {
        PlanError::ProducesCycle { backend, cycle, .. } => {
            assert_eq!(backend, "text");
            assert_eq!(cycle.iter().map(|e| e.to_string()).collect::<Vec<_>>(), ["t.foo", "t.mid"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn the_ladder_prefers_the_backend_default_over_the_general_default() {
    let registry = lowering_registry(
        vec![
            rule("t.foo.lower.general", "t.foo", produces("t.pres"), DefaultMark::Everywhere),
            rule(
                "t.foo.lower.text",
                "t.foo",
                produces("t.pres"),
                DefaultMark::OnBackends(vec!["text".into()]),
            ),
            rule("t.mid.lower.default", "t.mid", produces("t.pres"), DefaultMark::Everywhere),
        ],
        &[],
    );
    let plan = plan_for(&registry, &text_request());
    let chosen = |backend: &str| {
        let selected = plan.selection(backend).unwrap();
        registry.rule(selected.rule_for("t.foo").unwrap()).rule.name.to_string()
    };
    assert_eq!(chosen("text"), "t.foo.lower.text");
    // The other backend has no specific default: the general one.
    assert_eq!(chosen("other"), "t.foo.lower.general");
}

#[test]
fn a_rule_marked_for_one_backend_is_no_candidate_on_another_even_as_the_only_one() {
    // The mark says where the rule belongs: `t.foo` has one rule, written for
    // the backend `other`, so on `text` the element has no rule at all and the
    // completeness check reports it.
    let registry = lowering_registry(
        vec![
            rule(
                "t.foo.lower.other",
                "t.foo",
                produces("t.pres"),
                DefaultMark::OnBackends(vec!["other".into()]),
            ),
            rule("t.mid.lower.default", "t.mid", produces("t.pres"), DefaultMark::Everywhere),
        ],
        &[],
    );
    match Plan::build(&registry, &text_request()).expect_err("incomplete on text") {
        PlanError::Incomplete { backend, missing, .. } => {
            assert_eq!(backend, "text");
            assert_eq!(missing.iter().map(|e| e.to_string()).collect::<Vec<_>>(), ["t.foo"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_preference_for_a_rule_of_another_backend_is_a_warning_and_the_ladder_goes_on() {
    let registry = lowering_registry(
        vec![
            rule("t.foo.lower.general", "t.foo", produces("t.pres"), DefaultMark::Everywhere),
            rule(
                "t.foo.lower.other",
                "t.foo",
                produces("t.pres"),
                DefaultMark::OnBackends(vec!["other".into()]),
            ),
            rule("t.mid.lower.default", "t.mid", produces("t.pres"), DefaultMark::Everywhere),
        ],
        &[],
    );
    let request =
        text_request().lowering("text", LoweringSelection::new().prefer("t.foo.lower.other"));
    let plan = plan_for(&registry, &request);
    // The preferred rule belongs to `other`, so the general rule is chosen and
    // the caller is told that what it asked for could not happen.
    let selected = plan.selection("text").unwrap();
    assert_eq!(registry.rule(selected.rule_for("t.foo").unwrap()).rule.name, "t.foo.lower.general");
    assert_eq!(
        plan.warnings().to_vec(),
        vec![PlanWarning::PreferredRuleNotCandidate {
            backend: "text".into(),
            rule: "t.foo.lower.other".into(),
            workflow: None
        }]
    );
    assert_eq!(
        plan.warnings()[0].to_string(),
        "the plan request prefers the lowering rule `t.foo.lower.other` on backend `text`, but that rule is marked as the default on other backends only, which restricts it to those backends"
    );
}

#[test]
fn the_ladder_takes_the_unique_candidate_without_a_default() {
    let registry = lowering_registry(
        vec![
            rule("t.foo.lower.only", "t.foo", produces("t.pres"), DefaultMark::Never),
            rule("t.mid.lower.default", "t.mid", produces("t.pres"), DefaultMark::Everywhere),
        ],
        &[],
    );
    let plan = plan_for(&registry, &text_request());
    let selected = plan.selection("text").unwrap();
    assert_eq!(registry.rule(selected.rule_for("t.foo").unwrap()).rule.name, "t.foo.lower.only");
}

#[test]
fn a_rule_contest_without_default_or_preference_is_a_typed_error() {
    let registry = lowering_registry(
        vec![
            rule("t.foo.lower.b", "t.foo", produces("t.pres"), DefaultMark::Never),
            rule("t.foo.lower.a", "t.foo", produces("t.pres"), DefaultMark::Never),
            rule("t.mid.lower.default", "t.mid", produces("t.pres"), DefaultMark::Everywhere),
        ],
        &[],
    );
    match Plan::build(&registry, &text_request()).expect_err("contested") {
        PlanError::RuleContest { backend, element, candidates, .. } => {
            assert_eq!(backend, "text");
            assert_eq!(element, "t.foo");
            assert_eq!(
                candidates.iter().map(|r| r.to_string()).collect::<Vec<_>>(),
                ["t.foo.lower.a", "t.foo.lower.b"]
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_preference_settles_a_rule_contest() {
    let registry = lowering_registry(
        vec![
            rule("t.foo.lower.b", "t.foo", produces("t.pres"), DefaultMark::Never),
            rule("t.foo.lower.a", "t.foo", produces("t.pres"), DefaultMark::Never),
            rule("t.mid.lower.default", "t.mid", produces("t.pres"), DefaultMark::Everywhere),
        ],
        &[],
    );
    let request = text_request().lowering("text", LoweringSelection::new().prefer("t.foo.lower.b"));
    let plan = plan_for(&registry, &request);
    let selected = plan.selection("text").unwrap();
    assert_eq!(registry.rule(selected.rule_for("t.foo").unwrap()).rule.name, "t.foo.lower.b");
}

#[test]
fn a_rule_contest_on_an_inactive_backend_is_no_error() {
    let registry = lowering_registry(
        vec![
            rule("t.foo.lower.b", "t.foo", produces("t.pres"), DefaultMark::Never),
            rule("t.foo.lower.a", "t.foo", produces("t.pres"), DefaultMark::Never),
        ],
        &[],
    );
    // Nothing demands a backend: the contest is kept aside, the plan builds.
    let plan = plan(&registry, &["t.resolved"]);
    assert!(plan.active_backends().is_empty());
    assert!(plan.selection("text").is_none());
}

#[test]
fn a_preference_overrides_a_native_claim() {
    let registry = lowering_registry(
        vec![
            rule("t.pres.lower.alt", "t.pres", produces("t.pres"), DefaultMark::Never),
            rule("t.foo.lower.default", "t.foo", produces("t.pres"), DefaultMark::Everywhere),
            rule("t.mid.lower.default", "t.mid", produces("t.pres"), DefaultMark::Everywhere),
        ],
        &[],
    );
    // Without a preference the native claim stands and the rule is unused.
    let plain = plan_for(&registry, &text_request());
    assert!(plain.selection("text").unwrap().rule_for("t.pres").is_none());
    // A preference for the alternative rule overrides the native claim — and
    // since the rule produces the element itself, completeness fails: the
    // override is real.
    let request =
        text_request().lowering("text", LoweringSelection::new().prefer("t.pres.lower.alt"));
    match Plan::build(&registry, &request).expect_err("the native claim is gone") {
        PlanError::ProducesCycle { cycle, .. } => {
            assert_eq!(cycle.iter().map(|e| e.to_string()).collect::<Vec<_>>(), ["t.pres"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_selected_rules_requires_join_the_lowering_stages_needs() {
    let registry = lowering_registry(
        vec![
            rule(
                "t.foo.lower.default",
                "t.foo",
                produces("t.pres").requires("t.resolved"),
                DefaultMark::Everywhere,
            ),
            rule("t.mid.lower.default", "t.mid", produces("t.pres"), DefaultMark::Everywhere),
        ],
        &[],
    );
    let plan = plan_for(&registry, &text_request());
    // The rule's requirement activated the resolver behind the lowering stage.
    assert_eq!(names(&plan.stage_names()), ["t.resolve", "t.text.lower"]);
}

#[test]
fn the_lowering_selection_enters_the_plan_key() {
    let registry = lowering_registry(
        vec![
            rule("t.foo.lower.b", "t.foo", produces("t.pres"), DefaultMark::Everywhere),
            rule("t.foo.lower.a", "t.foo", produces("t.pres"), DefaultMark::Never),
            rule("t.mid.lower.default", "t.mid", produces("t.pres"), DefaultMark::Everywhere),
        ],
        &[],
    );
    let plain = plan_for(&registry, &text_request()).key();
    let preferred = plan_for(
        &registry,
        &text_request().lowering("text", LoweringSelection::new().prefer("t.foo.lower.a")),
    )
    .key();
    let hole = plan_for(&registry, &text_request().unsupported("text", "t.mid")).key();
    assert_ne!(plain, preferred);
    assert_ne!(plain, hole);
}

#[test]
fn request_names_are_checked_against_the_roster() {
    let registry = lowering_registry(vec![], &[]);
    let unknown_backend = text_request().lowering("nope", LoweringSelection::new());
    assert!(matches!(
        Plan::build(&registry, &unknown_backend),
        Err(PlanError::UnknownBackend { .. })
    ));
    let unknown_rule = text_request().lowering("text", LoweringSelection::new().prefer("t.nope"));
    assert!(matches!(Plan::build(&registry, &unknown_rule), Err(PlanError::UnknownRule { .. })));
    let unknown_element = text_request().unsupported("text", "t.nope");
    assert!(matches!(
        Plan::build(&registry, &unknown_element),
        Err(PlanError::UnknownElement { .. })
    ));
}

// --- workflows -----------------------------------------------------------------------------------

const DOCUMENT: TableName<ArtifactSet> = TableName::new("t.output.text.document");
const OUTPUT: TableName<Log> = TableName::new("t.output.text");

/// A registry with the `text` backend whose final stage writes the artifact
/// set, the output `text` over it, the workflow `text` (outputs `[text]`) and
/// a workflow `draft` extending it with a hole and a preference.
fn workflow_registry() -> Arc<FlmRegistry> {
    registry(vec![workflow_feature()])
}

fn workflow_feature() -> Arc<dyn FlmFeature> {
    feature("t", |b| {
        b.element(macro_def("t", "foo", 0))?;
        b.element(macro_def("t", "pres", 0))?;
        b.capability(CapabilityDef::new("t.resolved"))?;
        b.capability(CapabilityDef::new("t.resolved.cheap").refines("t.resolved"))?;
        b.capability(CapabilityDef::new("t.resolved.full").refines("t.resolved").default())?;
        b.stage(provider("t.resolve-cheap", &["t.resolved.cheap"], &[]))?;
        b.stage(provider("t.resolve-full", &["t.resolved.full"], &[]))?;
        b.table(DOCUMENT, "the text output")?;
        b.table(OUTPUT, "the emitted text")?;
        b.capability(CapabilityDef::new(lowered_cap("text")))?;
        let lowered = lowered_cap("text");
        let native: Vec<String> = flm_core::lang::structural::NAMES
            .iter()
            .map(|n| n.to_string())
            .chain(["t.pres".to_string()])
            .collect();
        b.backend(flm_core::core::registry::BackendDef::new(
            "text",
            native,
            noop(
                StageDecl::new("t.text.lower", Effect::Compute, Scope::PerTree)
                    .provides(lowered.clone()),
                Needs::new().requires("t.resolved"),
            ),
            noop(
                StageDecl::new("t.text.emit", Effect::Compute, Scope::Document).writes(OUTPUT),
                Needs::new().requires(lowered),
            ),
            noop(
                StageDecl::new("t.text.final", Effect::Compute, Scope::Document).writes(DOCUMENT),
                Needs::new().reads(OUTPUT),
            ),
        ))?;
        b.output(OutputDef::new(
            "text",
            "t.output.text.document",
            "t.output.text.document",
            "text/plain",
            "txt",
        ))?;
        b.workflow(WorkflowDef::new("text").output("text"))?;
        b.workflow(
            WorkflowDef::new("draft")
                .extends("text")
                .prefer("t.resolve-cheap")
                .unsupported("text", "t.foo"),
        )
    })
}

#[test]
fn fields_registered_for_a_workflow_change_it_or_define_it() {
    let fields = feature("fields", |b| {
        b.workflow_fields(WorkflowDef::new("text").prefer("t.resolve-cheap").unsupported("text", "t.foo"))?;
        b.workflow_fields(WorkflowDef::new("text").docs("Changed."))?;
        b.workflow_fields(WorkflowDef::new("fresh").extends("text"))
    });
    // Registered before the workflow they change, the fields still apply to it.
    let registry = registry(vec![fields, workflow_feature()]);
    let text = &registry.workflow("text").expect("one workflow `text`").def;
    assert_eq!(text.outputs.iter().map(|o| o.to_string()).collect::<Vec<_>>(), ["text"]);
    #[cfg(feature = "docs")]
    assert_eq!(text.docs, "Changed.");
    let plan = Plan::for_workflow(&registry, "text", &Overrides::none()).expect("the hole is accepted");
    assert_eq!(names(&plan.stage_names())[0], "t.resolve-cheap");
    assert_eq!(registry.workflow("fresh").expect("defined").def.extends.as_deref().map(|n| n.as_str()), Some("text"));
}

#[test]
fn fields_the_host_registers_for_a_workflow_apply_after_the_features_fields() {
    let fields = feature("fields", |b| b.workflow_fields(WorkflowDef::new("text").docs("A feature's.")));
    let mut builder = RegistryBuilder::new();
    builder.feature(fields).feature(workflow_feature());
    builder
        .workflow_fields(WorkflowDef::new("text").docs("The host's.").prefer("t.resolve-cheap"))
        .workflow_fields(WorkflowDef::new("fresh").extends("text"));
    let registry = builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    });
    // Over a feature's workflow: the host's fields replace the feature's and
    // keep the fields the host did not declare.
    let text = registry.workflow("text").expect("one workflow `text`");
    #[cfg(feature = "docs")]
    assert_eq!(text.def.docs, "The host's.");
    assert_eq!(text.def.outputs.iter().map(|o| o.to_string()).collect::<Vec<_>>(), ["text"]);
    assert_eq!(text.owner.as_str(), "t");
    // A name no feature registers: the fields are the workflow, the host's.
    let fresh = registry.workflow("fresh").expect("defined");
    assert_eq!(fresh.def.extends.as_deref().map(|n| n.as_str()), Some("text"));
    assert_eq!(fresh.owner.as_str(), HOST_FEATURE);
}

#[test]
fn a_workflow_turns_its_outputs_into_demands_and_activates_the_backend() {
    let registry = workflow_registry();
    // The `text` workflow accepts no hole, so `t.foo` makes it incomplete: the
    // output's demand did reach the backend.
    match Plan::for_workflow(&registry, "text", &Overrides::none()).expect_err("incomplete") {
        PlanError::Incomplete { backend, missing, .. } => {
            assert_eq!(backend, "text");
            assert_eq!(missing.iter().map(|e| e.to_string()).collect::<Vec<_>>(), ["t.foo"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    let with_hole = Overrides {
        unsupported: [("text".into(), vec!["t.foo".into()])].into_iter().collect(),
        ..Overrides::none()
    };
    let plan = Plan::for_workflow(&registry, "text", &with_hole).expect("the hole excuses it");
    assert_eq!(
        plan.demands().iter().map(|c| c.to_string()).collect::<Vec<_>>(),
        ["t.output.text.document"]
    );
    assert_eq!(
        names(&plan.stage_names()),
        ["t.resolve-full", "t.text.lower", "t.text.emit", "t.text.final"]
    );
    assert_eq!(plan.active_backends().iter().map(|b| b.to_string()).collect::<Vec<_>>(), ["text"]);
}

#[test]
fn a_workflow_extends_another_taking_the_ancestors_outputs() {
    let registry = workflow_registry();
    let plan = Plan::for_workflow(&registry, "draft", &Overrides::none())
        .expect("the hole is in the bundle");
    // The outputs come from `text`; the preference and the hole are `draft`'s.
    assert_eq!(
        names(&plan.stage_names()),
        ["t.resolve-cheap", "t.text.lower", "t.text.emit", "t.text.final"]
    );
}

#[test]
fn the_registry_gives_a_workflow_s_chain_root_first_and_its_fields_combined_along_it() {
    let registry = workflow_registry();
    let chain: Vec<String> =
        registry.workflow_chain("draft").expect("draft").iter().map(|def| def.name.to_string()).collect();
    assert_eq!(chain, ["text", "draft"]);
    let combined = registry.combined_workflow("draft").expect("draft");
    assert_eq!(combined.name.to_string(), "draft");
    assert_eq!(combined.outputs, registry.workflow("text").unwrap().def.outputs);
    assert_eq!(combined.prefer, registry.workflow("draft").unwrap().def.prefer);
    assert!(registry.workflow_chain("nope").is_none() && registry.combined_workflow("nope").is_none());
}

#[test]
fn host_overrides_rank_ahead_of_the_bundles_preferences() {
    let registry = workflow_registry();
    let overrides = Overrides { prefer: vec!["t.resolve-full".into()], ..Overrides::none() };
    let plan = Plan::for_workflow(&registry, "draft", &overrides).expect("plan");
    assert_eq!(names(&plan.stage_names())[0], "t.resolve-full");
    assert_eq!(
        plan.request().prefer.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
        ["t.resolve-full", "t.resolve-cheap"]
    );
}

#[test]
fn an_error_of_a_workflows_plan_names_the_workflow() {
    let overrides = Overrides { prefer: vec!["t.nope".into()], ..Overrides::none() };
    match Plan::for_workflow(&workflow_registry(), "draft", &overrides).expect_err("unknown stage") {
        error @ PlanError::UnknownPreference { .. } => {
            assert!(matches!(&error, PlanError::UnknownPreference { workflow: Some(w), .. } if w == "draft"));
            assert!(
                error.to_string().starts_with(
                    "the plan request for workflow `draft` prefers the stage `t.nope`, but"
                ),
                "{error}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_error_of_a_hand_built_request_names_the_plan_request() {
    let request = PlanRequest::new().demand(output_cap("text")).prefer("t.nope");
    match Plan::build(&workflow_registry(), &request).expect_err("unknown stage") {
        error @ PlanError::UnknownPreference { workflow: None, .. } => {
            assert!(
                error.to_string().starts_with("the plan request prefers the stage `t.nope`, but"),
                "{error}"
            );
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_unknown_workflow_lists_the_roster() {
    match Plan::for_workflow(&workflow_registry(), "nope", &Overrides::none()).expect_err("unknown")
    {
        PlanError::UnknownWorkflow { workflow, roster } => {
            assert_eq!(workflow, "nope");
            assert_eq!(roster, ["draft", "text"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_workflow_listing_an_unknown_output_is_a_registry_error() {
    let error =
        registry_error(vec![feature("t", |b| b.workflow(WorkflowDef::new("w").output("nope")))]);
    assert_eq!(error.phase, BuildPhase::Validate);
    match &error.errors[0] {
        BuildError::Undeclared { kind, name, .. } => {
            assert_eq!(*kind, "output");
            assert_eq!(name, "nope");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_extends_cycle_is_a_registry_error_naming_the_cycle() {
    let error = registry_error(vec![feature("t", |b| {
        b.workflow(WorkflowDef::new("b").extends("a"))?;
        b.workflow(WorkflowDef::new("a").extends("b"))
    })]);
    match &error.errors[0] {
        BuildError::ExtendsCycle { cycle } => {
            assert_eq!(cycle.iter().map(|w| w.to_string()).collect::<Vec<_>>(), ["a", "b"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_workflows_render_options_are_validated_at_registry_build() {
    #[derive(Debug, Default, Hash)]
    struct Width(u32);
    let error = registry_error(vec![feature("t", |b| {
        b.workflow(
            WorkflowDef::new("w")
                .render_options(flm_core::model::options::OptionValues::new().with(Width(1))),
        )
    })]);
    assert!(
        matches!(&error.errors[0], BuildError::Feature { feature, .. } if feature == "t"),
        "{}",
        error.render()
    );
}
