//! Provider selection by forced deduction (`flm_core::core::plan`): scenarios
//! A, B and C, the graphics worked example, refinement implication, marked
//! defaults, collections, externals, the request's roster errors, and
//! declaration-order independence pinned byte-identically against a golden.

mod common;

use std::sync::Arc;

use flm_core::model::names::CapName;
use flm_core::core::plan::{Plan, PlanError, PlanRequest, PlanWarning};
use flm_core::core::registry::{CapabilityDef, CollectionDef, FlmRegistry};
use flm_core::core::data::table::TableName;

use common::pipeline::{
    Count, ToyCap, ToyProvider, feature, golden_path, plan_for, provider, registry, toy_registry,
};
use flm_core::dump::golden::assert_text_golden;

fn request(demands: &[&str], prefer: &[&str]) -> PlanRequest {
    let mut request = PlanRequest::new();
    for demand in demands {
        request = request.demand(*demand);
    }
    for stage in prefer {
        request = request.prefer(*stage);
    }
    request
}

fn build(
    registry: &Arc<FlmRegistry>,
    demands: &[&str],
    prefer: &[&str],
) -> Result<Plan, PlanError> {
    Plan::build(registry, &request(demands, prefer))
}

fn excluded(plan: &Plan) -> Vec<(String, String, String)> {
    plan.excluded()
        .iter()
        .map(|e| (e.stage.to_string(), e.excluded_by.to_string(), e.over.to_string()))
        .collect()
}

// --- scenario fixtures -----------------------------------------------------------------------

fn scenario_a() -> Arc<FlmRegistry> {
    toy_registry(
        &[("X", None, false), ("Y", None, false)],
        &[("P", vec!["X", "Y"], vec![]), ("Q", vec!["Y"], vec![])],
    )
}

fn scenario_b() -> Arc<FlmRegistry> {
    toy_registry(
        &[("X", None, false), ("Y", None, false)],
        &[("P", vec!["X", "Y"], vec![]), ("Q", vec!["Y"], vec![]), ("R", vec!["X"], vec![])],
    )
}

fn scenario_c() -> Arc<FlmRegistry> {
    toy_registry(
        &[("X", None, false), ("Y", None, false), ("Z", None, false)],
        &[("P", vec!["X", "Y"], vec![]), ("Q", vec!["Z", "Y"], vec![])],
    )
}

// --- A: automatic co-commitment ----------------------------------------------------------------

#[test]
fn scenario_a_one_forced_stage_supplies_both_capabilities() {
    let plan = build(&scenario_a(), &["X", "Y"], &[]).expect("no conflict");
    // X has exactly one candidate (P), so P commits; that excludes Q over the
    // Y they share, and Y is then supplied by P without any preference.
    assert_eq!(plan.scheduled_names(), ["P"]);
    assert_eq!(plan.commit_order(), ["P"]);
    assert_eq!(excluded(&plan), [("Q".to_string(), "P".to_string(), "Y".to_string())]);
}

// --- B: exclusion narrows a contest ------------------------------------------------------------

#[test]
fn scenario_b_preferring_the_narrow_stage_schedules_two() {
    let plan = build(&scenario_b(), &["X", "Y"], &["R"]).expect("no conflict");
    // Both X and Y start contested; preferring R commits R, which excludes P
    // over X — and that narrows Y to Q alone, which commits by propagation.
    assert_eq!(plan.scheduled_names(), ["Q", "R"]);
    assert_eq!(plan.commit_order(), ["R", "Q"]);
    assert_eq!(excluded(&plan), [("P".to_string(), "R".to_string(), "X".to_string())]);
}

#[test]
fn scenario_b_preferring_the_wide_stage_schedules_it_alone() {
    let plan = build(&scenario_b(), &["X", "Y"], &["P"]).expect("no conflict");
    assert_eq!(plan.scheduled_names(), ["P"]);
    assert_eq!(
        excluded(&plan),
        [
            ("Q".to_string(), "P".to_string(), "Y".to_string()),
            ("R".to_string(), "P".to_string(), "X".to_string())
        ]
    );
}

// --- C: exclusivity conflict --------------------------------------------------------------------

#[test]
fn scenario_c_shared_side_capability_makes_the_demand_unsatisfiable() {
    let error = build(&scenario_c(), &["X", "Z"], &[]).expect_err("P and Q cannot coexist");
    // X forces P; P excludes Q over the Y they share; Z is then unprovidable.
    match error {
        PlanError::Unsatisfiable { capability, blocked, .. } => {
            assert_eq!(capability, "Z");
            assert_eq!(blocked.len(), 1);
            assert_eq!(blocked[0].stage, "Q");
            assert_eq!(blocked[0].excluded_by, "P");
            assert_eq!(blocked[0].over, "Y");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_demand_nothing_provides_is_unsatisfiable_with_an_empty_blocked_list() {
    let registry =
        toy_registry(&[("X", None, false), ("orphan", None, false)], &[("P", vec!["X"], vec![])]);
    let error = build(&registry, &["X", "orphan"], &[]).expect_err("nothing provides `orphan`");
    match error {
        PlanError::Unsatisfiable { capability, blocked, .. } => {
            assert_eq!(capability, "orphan");
            assert!(blocked.is_empty());
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// --- the graphics worked example ----------------------------------------------------------------

const COLLECTED: &str = "flm.graphics.collected";
const RESOLVED: &str = "flm.graphics.resolved";
const RESOLVED_DRAFT: &str = "flm.graphics.resolved.draft";
const RESOLVED_FINAL: &str = "flm.graphics.resolved.final";
const INSPECTED: &str = "flm.graphics.inspected";

fn graphics_caps() -> Vec<ToyCap> {
    vec![
        (COLLECTED, None, false),
        (RESOLVED, None, false),
        (RESOLVED_DRAFT, Some(RESOLVED), false),
        (RESOLVED_FINAL, Some(RESOLVED), false),
        (INSPECTED, None, false),
    ]
}

fn graphics_stages() -> Vec<ToyProvider> {
    vec![
        ("collect", vec![COLLECTED], vec![]),
        ("resolve-draft", vec![RESOLVED_DRAFT], vec![COLLECTED]),
        ("resolve-final", vec![RESOLVED_FINAL], vec![COLLECTED]),
        ("inspect", vec![INSPECTED], vec![RESOLVED_FINAL]),
        ("inspect-exotic", vec![INSPECTED], vec![RESOLVED_FINAL]),
    ]
}

fn graphics() -> Arc<FlmRegistry> {
    toy_registry(&graphics_caps(), &graphics_stages())
}

#[test]
fn refinement_implies_the_ancestors() {
    let registry = graphics();
    let closure: Vec<String> = registry
        .stage_named("resolve-final")
        .unwrap()
        .provides
        .iter()
        .map(|id| registry.capability(*id).name.to_string())
        .collect();
    assert_eq!(closure, [RESOLVED, RESOLVED_FINAL]);
}

#[test]
fn graphics_plan_print_follows_the_preference_all_the_way_down() {
    let plan =
        build(&graphics(), &[INSPECTED], &["inspect-exotic"]).expect("the preference settles it");
    assert_eq!(plan.scheduled_names(), ["collect", "inspect-exotic", "resolve-final"]);
    // The preference first, then everything it forces.
    assert_eq!(plan.commit_order(), ["inspect-exotic", "resolve-final", "collect"]);
    assert_eq!(
        excluded(&plan),
        [
            ("inspect".to_string(), "inspect-exotic".to_string(), INSPECTED.to_string()),
            ("resolve-draft".to_string(), "resolve-final".to_string(), RESOLVED.to_string()),
        ]
    );
}

#[test]
fn graphics_plan_print_default_is_contested() {
    let error = build(&graphics(), &[INSPECTED], &[]).expect_err("no preference");
    match error {
        PlanError::Contested { capability, candidates, .. } => {
            assert_eq!(capability, INSPECTED);
            assert_eq!(candidates, ["inspect", "inspect-exotic"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn graphics_plan_preview_stops_at_the_draft() {
    let plan = build(&graphics(), &[RESOLVED_DRAFT], &[]).expect("only one draft resolver");
    assert_eq!(plan.scheduled_names(), ["collect", "resolve-draft"]);
    assert_eq!(plan.commit_order(), ["resolve-draft", "collect"]);
    // resolve-final is out because it also provides the abstract `resolved`.
    assert_eq!(
        excluded(&plan),
        [("resolve-final".to_string(), "resolve-draft".to_string(), RESOLVED.to_string())]
    );
}

#[test]
fn graphics_plan_abstract_satisfies_resolved_by_implication() {
    let plan =
        build(&graphics(), &[RESOLVED, INSPECTED], &["inspect"]).expect("no extra preference");
    // `inspect` (the only preference) forces resolve-final, whose implied
    // `resolved` satisfies the abstract demand — and excludes resolve-draft.
    assert_eq!(plan.scheduled_names(), ["collect", "inspect", "resolve-final"]);
    assert_eq!(plan.commit_order(), ["inspect", "resolve-final", "collect"]);
    assert_eq!(
        excluded(&plan),
        [
            ("inspect-exotic".to_string(), "inspect".to_string(), INSPECTED.to_string()),
            ("resolve-draft".to_string(), "resolve-final".to_string(), RESOLVED.to_string()),
        ]
    );
}

#[test]
fn graphics_plan_incoherent_is_refused() {
    let error = build(&graphics(), &[RESOLVED_DRAFT, INSPECTED], &["inspect"])
        .expect_err("draft and final");
    // The draft demand is forced first (one candidate), which excludes
    // resolve-final over `resolved`; the preferred inspector then needs
    // `resolved.final` and nothing can provide it.
    match error {
        PlanError::Unsatisfiable { capability, blocked, .. } => {
            assert_eq!(capability, RESOLVED_FINAL);
            assert_eq!(blocked.len(), 1);
            assert_eq!(blocked[0].stage, "resolve-final");
            assert_eq!(blocked[0].excluded_by, "resolve-draft");
            assert_eq!(blocked[0].over, RESOLVED);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_abstract_requires_is_satisfied_by_a_refined_provider() {
    let mut caps = graphics_caps();
    caps.push(("flm.graphics.reported", None, false));
    let mut stages = graphics_stages();
    stages.push(("report", vec!["flm.graphics.reported"], vec![RESOLVED]));
    let registry = toy_registry(&caps, &stages);
    let plan = build(&registry, &["flm.graphics.reported", RESOLVED_FINAL], &[])
        .expect("resolved is implied");
    assert_eq!(plan.scheduled_names(), ["collect", "report", "resolve-final"]);
    // `report`'s plain `resolved` never needed a preference: the demanded
    // `resolved.final` forced resolve-final, whose implication covers it.
    assert_eq!(plan.commit_order(), ["report", "resolve-final", "collect"]);
}

// --- declaration-order independence -------------------------------------------------------------

fn graphics_requests() -> Vec<(&'static str, PlanRequest)> {
    vec![
        ("abstract", request(&[RESOLVED, INSPECTED], &["inspect"])),
        ("incoherent", request(&[RESOLVED_DRAFT, INSPECTED], &["inspect"])),
        ("preview", request(&[RESOLVED_DRAFT], &[])),
        ("print", request(&[INSPECTED], &["inspect-exotic"])),
        ("print-default", request(&[INSPECTED], &[])),
    ]
}

/// A structural rendering of the errors the graphics example raises — the
/// variant and its fields, never the message text.
fn render_error(error: &PlanError) -> String {
    let names = |names: &[flm_core::model::names::StageName]| {
        names.iter().map(|n| n.as_str()).collect::<Vec<_>>().join(", ")
    };
    match error {
        PlanError::Unsatisfiable { capability, blocked, .. } => {
            let blocked: Vec<String> = blocked
                .iter()
                .map(|b| format!("{}/{}/{}", b.stage, b.excluded_by, b.over))
                .collect();
            format!("unsatisfiable cap={capability} blocked=[{}]", blocked.join(", "))
        }
        PlanError::Contested { capability, candidates, .. } => {
            format!("contested cap={capability} candidates=[{}]", names(candidates))
        }
        other => panic!("the graphics example raises no other error: {other:?}"),
    }
}

fn render_all(registry: &Arc<FlmRegistry>) -> String {
    let mut out = String::new();
    for (name, request) in graphics_requests() {
        out.push_str(&format!("plan {name}\n"));
        match Plan::build(registry, &request) {
            Ok(plan) => {
                for line in plan.render().lines() {
                    out.push_str(&format!("  {line}\n"));
                }
            }
            Err(error) => out.push_str(&format!("  error: {}\n", render_error(&error))),
        }
    }
    out
}

fn graphics_permuted(cap_order: &[usize], stage_order: &[usize]) -> Arc<FlmRegistry> {
    let caps = graphics_caps();
    let stages = graphics_stages();
    let caps: Vec<ToyCap> = cap_order.iter().map(|i| caps[*i]).collect();
    let stages: Vec<ToyProvider> = stage_order.iter().map(|i| stages[*i].clone()).collect();
    toy_registry(&caps, &stages)
}

#[test]
fn results_and_errors_are_byte_identical_across_declaration_orders() {
    let permutations: [(&[usize], &[usize]); 5] = [
        (&[0, 1, 2, 3, 4], &[0, 1, 2, 3, 4]),
        (&[4, 3, 2, 1, 0], &[4, 3, 2, 1, 0]),
        (&[2, 0, 4, 1, 3], &[1, 4, 0, 3, 2]),
        (&[1, 3, 0, 2, 4], &[3, 0, 2, 4, 1]),
        (&[3, 4, 1, 0, 2], &[2, 1, 4, 0, 3]),
    ];
    // Some permutations declare `resolved.draft` before its parent: forward
    // references are legal, the roster is a set.
    let reference = render_all(&graphics());
    for (cap_order, stage_order) in permutations {
        let rendered = render_all(&graphics_permuted(cap_order, stage_order));
        assert_eq!(rendered, reference, "permutation {cap_order:?} / {stage_order:?}");
    }
    assert_text_golden(golden_path("graphics.txt"), &reference);
}

// --- marked defaults -----------------------------------------------------------------------------

fn defaults_registry(default_a: bool, default_b: bool) -> Arc<FlmRegistry> {
    toy_registry(
        &[
            ("t.x", None, false),
            ("t.x.a", Some("t.x"), default_a),
            ("t.x.b", Some("t.x"), default_b),
        ],
        &[("t.pa", vec!["t.x.a"], vec![]), ("t.pb", vec!["t.x.b"], vec![])],
    )
}

#[test]
fn a_marked_default_settles_a_contest_among_refinements() {
    let plan =
        build(&defaults_registry(true, false), &["t.x"], &[]).expect("the default settles it");
    assert_eq!(plan.scheduled_names(), ["t.pa"]);
    assert_eq!(excluded(&plan), [("t.pb".to_string(), "t.pa".to_string(), "t.x".to_string())]);
}

#[test]
fn a_preference_beats_the_marked_default() {
    let plan =
        build(&defaults_registry(true, false), &["t.x"], &["t.pb"]).expect("the preference wins");
    assert_eq!(plan.scheduled_names(), ["t.pb"]);
}

#[test]
fn a_contest_without_default_or_preference_is_a_typed_error() {
    let error =
        build(&defaults_registry(false, false), &["t.x"], &[]).expect_err("nothing breaks the tie");
    match error {
        PlanError::Contested { capability, candidates, .. } => {
            assert_eq!(capability, "t.x");
            assert_eq!(candidates, ["t.pa", "t.pb"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn the_default_walk_descends_a_contested_default() {
    // `t.x` defaults to `t.x.a`, itself contested between two refinements of
    // which `t.x.a.1` is the default: the walk reaches the unique provider.
    let registry = toy_registry(
        &[
            ("t.x", None, false),
            ("t.x.a", Some("t.x"), true),
            ("t.x.b", Some("t.x"), false),
            ("t.x.a.1", Some("t.x.a"), true),
            ("t.x.a.2", Some("t.x.a"), false),
        ],
        &[
            ("t.p1", vec!["t.x.a.1"], vec![]),
            ("t.p2", vec!["t.x.a.2"], vec![]),
            ("t.pb", vec!["t.x.b"], vec![]),
        ],
    );
    let plan = build(&registry, &["t.x"], &[]).expect("the default chain settles it");
    assert_eq!(plan.scheduled_names(), ["t.p1"]);
}

// --- warnings, self-exclusion -------------------------------------------------------------------

#[test]
fn a_preference_for_an_excluded_stage_is_a_warning() {
    // X forces P, which excludes Q; preferring Q afterwards changes nothing.
    let plan =
        build(&scenario_a(), &["X", "Y"], &["Q"]).expect("the preference is ignored, not fatal");
    assert_eq!(plan.scheduled_names(), ["P"]);
    assert_eq!(
        plan.warnings().to_vec(),
        vec![PlanWarning::IgnoredPreference {
            stage: "Q".into(),
            excluded_by: "P".into(),
            over: "Y".into(),
            workflow: None
        }]
    );
}

#[test]
fn a_stage_that_would_exclude_its_own_provider_is_refused() {
    // `t.s` provides `t.x.b` and requires the sibling `t.x.a`: committing it
    // would exclude every provider of `t.x.a` over their shared parent.
    let registry = toy_registry(
        &[("t.x", None, false), ("t.x.a", Some("t.x"), false), ("t.x.b", Some("t.x"), false)],
        &[("t.pa", vec!["t.x.a"], vec![]), ("t.s", vec!["t.x.b"], vec!["t.x.a"])],
    );
    let error = build(&registry, &[], &[]).expect_err("demand-independent");
    match error {
        PlanError::SelfExclusion { stage, requires, provides, .. } => {
            assert_eq!(stage, "t.s");
            assert_eq!(requires, "t.x.a");
            assert_eq!(provides, "t.x.b");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_stage_requiring_the_parent_of_what_it_provides_is_refused() {
    // Rejected unless the two are identical: the requirement could only ever be
    // met by the stage itself.
    let registry = toy_registry(
        &[("t.x", None, false), ("t.x.b", Some("t.x"), false)],
        &[("t.s", vec!["t.x.b"], vec!["t.x"])],
    );
    match build(&registry, &["t.x.b"], &[]).expect_err("not identical") {
        PlanError::SelfExclusion { stage, requires, provides, .. } => {
            assert_eq!(stage, "t.s");
            assert_eq!(requires, "t.x");
            assert_eq!(provides, "t.x.b");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

/// The one carve-out of the self-exclusion rule: the required capability is
/// itself among the stage's direct provides.
#[test]
fn a_stage_requiring_exactly_what_it_provides_is_fine() {
    let registry = toy_registry(
        &[("t.x", None, false), ("t.x.b", Some("t.x"), false)],
        &[("t.s", vec!["t.x.b"], vec!["t.x.b"])],
    );
    let plan = build(&registry, &["t.x.b"], &[]).expect("identical");
    assert_eq!(plan.scheduled_names(), ["t.s"]);
}

#[test]
fn the_self_exclusion_error_names_the_name_least_pair() {
    // Two offending requirements, declared in the reverse of name order.
    let registry = toy_registry(
        &[
            ("t.x", None, false),
            ("t.x.a", Some("t.x"), false),
            ("t.x.b", Some("t.x"), false),
            ("t.y", None, false),
            ("t.y.a", Some("t.y"), false),
            ("t.y.b", Some("t.y"), false),
        ],
        &[("t.s", vec!["t.y.b", "t.x.b"], vec!["t.y.a", "t.x.a"])],
    );
    match build(&registry, &[], &[]).expect_err("two offending pairs") {
        PlanError::SelfExclusion { requires, provides, .. } => {
            assert_eq!(requires, "t.x.a");
            assert_eq!(provides, "t.x.b");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// --- collections and externals -------------------------------------------------------------------

/// Capabilities `t.a`, `t.b` in the collection `t.all`, each provided; with
/// `requirer`, a stage `t.wants-all` requiring the collection itself.
fn collection_registry(requirer: bool) -> Arc<FlmRegistry> {
    registry(vec![feature("t", move |b| {
        b.capability(CapabilityDef::new("t.a"))?;
        b.capability(CapabilityDef::new("t.b"))?;
        b.capability(CapabilityDef::new("t.c"))?;
        b.collection(CollectionDef::new("t.all"))?;
        b.collection_member("t.all", "t.a")?;
        b.collection_member("t.all", "t.b")?;
        b.stage(provider("t.pa", &["t.a"], &[]))?;
        b.stage(provider("t.pb", &["t.b"], &[]))?;
        if requirer {
            b.stage(provider("t.wants-all", &["t.c"], &["t.all"]))?;
        }
        Ok(())
    })])
}

#[test]
fn demanding_a_collection_demands_its_members() {
    let plan = build(&collection_registry(false), &["t.all"], &[]).expect("members provided");
    assert_eq!(plan.scheduled_names(), ["t.pa", "t.pb"]);
    assert_eq!(plan.demands().iter().map(|c| c.to_string()).collect::<Vec<_>>(), ["t.a", "t.b"]);
}

#[test]
fn a_stage_requiring_a_collection_is_a_typed_error() {
    // Needs are resolved for every registered stage, whatever the demands.
    let error = build(&collection_registry(true), &["t.a"], &[])
        .expect_err("collections are demand-side only");
    match error {
        PlanError::CollectionRequired { stage, collection, .. } => {
            assert_eq!(stage, "t.wants-all");
            assert_eq!(collection, "t.all");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_external_tables_capability_is_satisfied_by_install_and_recorded() {
    let registry = registry(vec![feature("t", |b| {
        b.external_table(TableName::<Count>::new("t.ext"), "install the corpus count")?;
        b.capability(flm_core::core::registry::CapabilityDef::new("t.read"))?;
        b.stage(provider("t.reader", &["t.read"], &["t.ext"]))?;
        Ok(())
    })]);
    let plan = build(&registry, &["t.read"], &[]).expect("an external needs no provider");
    assert_eq!(plan.scheduled_names(), ["t.reader"]);
    assert_eq!(plan.external_names(), [CapName::from("t.ext")]);
}

// --- the request's roster errors -----------------------------------------------------------------

#[test]
fn an_unknown_demand_lists_capabilities_and_collections() {
    let error = build(&collection_registry(false), &["t.nope"], &[]).expect_err("undeclared");
    match error {
        PlanError::UnknownDemand { name, roster, .. } => {
            assert_eq!(name, "t.nope");
            assert_eq!(roster, ["t.a", "t.all", "t.b", "t.c"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_unknown_preferred_stage_lists_the_stage_roster() {
    let error = build(&scenario_a(), &["X"], &["U"]).expect_err("no stage `U`");
    match error {
        PlanError::UnknownPreference { stage, roster, .. } => {
            assert_eq!(stage, "U");
            assert_eq!(roster, ["P", "Q"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_needs_reference_to_an_undeclared_capability_lists_the_roster() {
    let registry = toy_registry(
        &[("t.a", None, false), ("t.b", None, false)],
        &[("t.s", vec!["t.a"], vec!["t.nope"])],
    );
    let error = build(&registry, &["t.b"], &[]).expect_err("needs are checked for every stage");
    match error {
        PlanError::NeedsUndeclared { stage, kind, name, roster, .. } => {
            assert_eq!(stage, "t.s");
            assert_eq!(kind, "capability");
            assert_eq!(name, "t.nope");
            assert_eq!(roster, ["t.a", "t.b"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// --- the plan key --------------------------------------------------------------------------------

#[test]
fn the_plan_key_follows_the_request() {
    let registry = scenario_b();
    let same = plan_for(&registry, &request(&["X", "Y"], &["R"]));
    let again = plan_for(&registry, &request(&["X", "Y"], &["R"]));
    let other = plan_for(&registry, &request(&["X", "Y"], &["P"]));
    assert_eq!(same.key(), again.key());
    assert_ne!(same.key(), other.key());
}
