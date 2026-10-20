//! Unified typed options through the pipeline (`flm_core::model::options`): the
//! bindings, the precedence of render over document over default as a stage and
//! as a rewrite sees it, the refusals at each site, the plan key, and a stage
//! whose needs follow a render-bound value.

mod common;

use std::sync::Arc;

use flm_core::model::options::{Bind, BindSite, OptionError, OptionValues};
use flm_core::core::plan::{Overrides, Plan, PlanCx, PlanError, PlanRequest};
use flm_core::core::registry::{BuildError, BuildPhase, CapabilityDef, FlmRegistry, WorkflowDef};
use flm_core::core::run::{Externals, Run, RunError, RunOptions, StageCx};
use flm_core::core::stage::{Effect, Needs, RewriteDecl, Scope, Stage, StageDecl};
use flm_core::core::data::store::StageOut;

use common::pipeline::{
    LOG, Log, doc, doc_with_options, feature, plan, plan_for, provider, registry, registry_error,
    rewrite, run, stage,
};

#[derive(Debug, Default, Hash, PartialEq, Eq, Clone)]
struct Width(u32);

#[derive(Debug, Default, Hash, PartialEq, Eq, Clone)]
struct Depth(u32);

#[derive(Debug, Default, Hash, PartialEq, Eq, Clone)]
struct Margin(u32);

#[derive(Debug, Default, Hash, PartialEq, Eq, Clone)]
struct Dpi(u32);

#[derive(Debug, Default, Hash, PartialEq, Eq, Clone)]
struct Seed(u32);

/// The feature `t`: `Width`, `Depth`, `Margin` either-bound, `Dpi`
/// render-bound, `Seed` registry-bound; a `Document` stage `t.report` writing
/// what it reads of the three either-bound options into the log.
fn options_feature() -> Arc<dyn flm_core::core::registry::FlmFeature> {
    feature("t", |b| {
        b.options::<Width>(Bind::Either)?;
        b.options::<Depth>(Bind::Either)?;
        b.options::<Margin>(Bind::Either)?;
        b.options::<Dpi>(Bind::Render)?;
        b.options::<Seed>(Bind::Registry)?;
        b.table(LOG, "what the stage read")?;
        b.stage(stage(
            StageDecl::new("t.report", Effect::Compute, Scope::Document).writes(LOG),
            Needs::new().option::<Width>().option::<Depth>().option::<Margin>(),
            |cx, out| {
                let log = out.table(LOG).expect("declared");
                log.0.push(format!("width {}", cx.option::<Width>().unwrap().0));
                log.0.push(format!("depth {}", cx.option::<Depth>().unwrap().0));
                log.0.push(format!("margin {}", cx.option::<Margin>().unwrap().0));
            },
        ))
    })
}

fn logged(run: &Run) -> Vec<String> {
    run.table(LOG).unwrap().map(|log: &Log| log.0.clone()).unwrap_or_default()
}

// --- precedence ----------------------------------------------------------------------------------

#[test]
fn a_stage_reads_render_over_document_over_default() {
    let registry = registry(vec![options_feature()]);
    let document = OptionValues::new().with(Width(100)).with(Depth(5));
    let doc = doc_with_options(&registry, &["x"], document);
    let request =
        PlanRequest::new().demand("t.log").render_options(OptionValues::new().with(Width(120)));
    let run = run(&doc, &plan_for(&registry, &request), &RunOptions::new());
    // render beats document; document beats the default; nothing set: the
    // type's default.
    assert_eq!(logged(&run), ["width 120", "depth 5", "margin 0"]);
}

#[test]
fn an_either_bound_option_accepts_both_sites() {
    let registry = registry(vec![options_feature()]);
    let doc = doc_with_options(&registry, &["x"], OptionValues::new().with(Width(1)));
    let request =
        PlanRequest::new().demand("t.log").render_options(OptionValues::new().with(Width(2)));
    let run = run(&doc, &plan_for(&registry, &request), &RunOptions::new());
    assert_eq!(logged(&run)[0], "width 2");
}

// --- the refusals --------------------------------------------------------------------------------

#[test]
fn a_document_value_at_a_render_only_binding_is_refused_at_run_construction() {
    let registry = registry(vec![options_feature()]);
    let doc = doc_with_options(&registry, &["x"], OptionValues::new().with(Dpi(150)));
    let error = Run::new(doc, plan(&registry, &["t.log"]), Externals::none())
        .expect_err("`Dpi` is render-bound");
    match error {
        RunError::DocumentOption(OptionError::BindingRefused { site, option, declared, owner, .. }) => {
            assert_eq!(site, BindSite::Document);
            assert_eq!(option, "Dpi");
            assert_eq!(declared, Bind::Render);
            assert_eq!(owner, "t");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_render_value_at_a_document_only_binding_is_refused_at_plan_build() {
    let registry = registry(vec![feature("t", |b| {
        b.options::<Depth>(Bind::Document)?;
        b.capability(CapabilityDef::new("t.x"))?;
        b.stage(provider("t.p", &["t.x"], &[]))
    })]);
    let request =
        PlanRequest::new().demand("t.x").render_options(OptionValues::new().with(Depth(4)));
    let error = Plan::build(&registry, &request).expect_err("`Depth` is document-bound");
    match error {
        PlanError::RenderOption(OptionError::BindingRefused { site, option, declared, .. }) => {
            assert_eq!(site, BindSite::Render);
            assert_eq!(option, "Depth");
            assert_eq!(declared, Bind::Document);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

/// A feature declaring the document-only `Depth`, with a workflow `report`
/// that sets `render` as its render option values.
fn document_only_feature(render: OptionValues) -> Arc<dyn flm_core::core::registry::FlmFeature> {
    feature("t", move |b| {
        b.options::<Depth>(Bind::Document)?;
        b.capability(CapabilityDef::new("t.x"))?;
        b.stage(provider("t.p", &["t.x"], &[]))?;
        b.workflow(WorkflowDef::new("report").demand("t.x").render_options(render.clone()))
    })
}

#[test]
fn a_workflows_render_value_at_a_document_only_binding_names_the_workflow() {
    // Set by the workflow itself: the registry build refuses it.
    let error = registry_error(vec![document_only_feature(OptionValues::new().with(Depth(4)))]);
    assert!(
        error.to_string().contains(
            "a value is set on the render request of workflow `report` for the option type \
             `Depth` of feature `t`, but feature `t` declares that values of this option type may be set only on the document"
        ),
        "{error}"
    );
    // Set by the caller's overrides of the workflow: the plan build refuses it.
    let registry = registry(vec![document_only_feature(OptionValues::new())]);
    let overrides =
        Overrides { render_options: OptionValues::new().with(Depth(4)), ..Overrides::none() };
    let error = Plan::for_workflow(&registry, "report", &overrides).expect_err("document-bound");
    match &error {
        PlanError::RenderOption(OptionError::BindingRefused { workflow: Some(w), .. }) => {
            assert_eq!(w, "report");
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert!(
        error.to_string().starts_with(
            "a value is set on the render request of workflow `report` for the option type \
             `Depth` of feature `t`, but"
        ),
        "{error}"
    );
    // A hand-built request keeps the place as the subject.
    let request =
        PlanRequest::new().demand("t.x").render_options(OptionValues::new().with(Depth(4)));
    let error = Plan::build(&registry, &request).expect_err("document-bound");
    assert!(
        error.to_string().starts_with("a value is set on the render request for the option type"),
        "{error}"
    );
}

#[test]
fn a_registry_bound_value_is_refused_at_both_sites() {
    let registry = registry(vec![options_feature()]);
    let doc = doc_with_options(&registry, &["x"], OptionValues::new().with(Seed(7)));
    let at_document = Run::new(doc, plan(&registry, &["t.log"]), Externals::none()).err();
    assert!(matches!(
        at_document,
        Some(RunError::DocumentOption(OptionError::BindingRefused {
            declared: Bind::Registry,
            ..
        }))
    ));
    let request =
        PlanRequest::new().demand("t.log").render_options(OptionValues::new().with(Seed(7)));
    let at_render = Plan::build(&registry, &request).err();
    assert!(matches!(
        at_render,
        Some(PlanError::RenderOption(OptionError::BindingRefused { declared: Bind::Registry, .. }))
    ));
}

#[test]
fn a_value_for_an_undeclared_option_type_lists_the_declared_types() {
    let registry = registry(vec![feature("t", |b| {
        b.options::<Width>(Bind::Either)?;
        b.options::<Depth>(Bind::Either)?;
        b.capability(CapabilityDef::new("t.x"))?;
        b.stage(provider("t.p", &["t.x"], &[]))
    })]);
    let request =
        PlanRequest::new().demand("t.x").render_options(OptionValues::new().with(Margin(1)));
    match Plan::build(&registry, &request).expect_err("`Margin` is not declared") {
        PlanError::RenderOption(OptionError::UnknownOption { site, option, roster, .. }) => {
            assert_eq!(site, BindSite::Render);
            assert_eq!(option, "Margin");
            assert_eq!(roster, ["Depth", "Width"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_read_of_a_registry_bound_option_is_refused_at_plan_build() {
    let registry = registry(vec![feature("t", |b| {
        b.options::<Seed>(Bind::Registry)?;
        b.capability(CapabilityDef::new("t.x"))?;
        b.stage(stage(
            StageDecl::new("t.p", Effect::Compute, Scope::Document).provides("t.x"),
            Needs::new().option::<Seed>(),
            |_, _| {},
        ))
    })]);
    // Needs are resolved for every registered stage, so no demand is needed.
    match Plan::build(&registry, &PlanRequest::new()).expect_err("registry-bound") {
        PlanError::RegistryBoundRead { error: OptionError::RegistryBoundRead { stage, option, owner }, .. } => {
            assert_eq!(stage, "t.p");
            assert_eq!(option, "Seed");
            assert_eq!(owner, "t");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_needs_naming_an_undeclared_option_type_is_a_plan_error() {
    let registry = registry(vec![feature("t", |b| {
        b.options::<Width>(Bind::Either)?;
        b.capability(CapabilityDef::new("t.x"))?;
        b.stage(stage(
            StageDecl::new("t.p", Effect::Compute, Scope::Document).provides("t.x"),
            Needs::new().option::<Depth>(),
            |_, _| {},
        ))
    })]);
    match Plan::build(&registry, &PlanRequest::new()).expect_err("`Depth` is not declared") {
        PlanError::NeedsUndeclared { stage, kind, name, roster, .. } => {
            assert_eq!(stage, "t.p");
            assert_eq!(kind, "option type");
            assert_eq!(name, "Depth");
            assert_eq!(roster, ["Width"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_undeclared_option_read_is_a_typed_error_at_run_time() {
    let registry = registry(vec![feature("t", |b| {
        b.options::<Width>(Bind::Either)?;
        b.options::<Depth>(Bind::Either)?;
        b.table(LOG, "what the stage saw")?;
        b.stage(stage(
            StageDecl::new("t.p", Effect::Compute, Scope::Document).writes(LOG),
            Needs::new().option::<Width>(),
            |cx, out| {
                let error = cx.option::<Depth>().expect_err("not declared");
                out.table(LOG).unwrap().0.push(format!("{error:?}"));
                match error {
                    OptionError::UndeclaredRead { stage, option, declared } => {
                        assert_eq!(stage, "t.p");
                        assert_eq!(option, "Depth");
                        assert_eq!(declared, ["Width"]);
                    }
                    other => panic!("unexpected error: {other:?}"),
                }
            },
        ))
    })]);
    let run = run(&doc(&registry, &["x"]), &plan(&registry, &["t.log"]), &RunOptions::new());
    assert_eq!(logged(&run).len(), 1);
}

#[test]
fn two_features_declaring_one_option_type_is_an_error_naming_both() {
    let error = registry_error(vec![
        feature("t.a", |b| b.options::<Width>(Bind::Either)),
        feature("t.b", |b| b.options::<Width>(Bind::Document)),
    ]);
    assert_eq!(error.phase, BuildPhase::Validate);
    match &error.errors[0] {
        BuildError::Duplicate { kind, name, first, second } => {
            assert_eq!(*kind, "option type");
            assert_eq!(name, "Width");
            assert_eq!(first, "t.a");
            assert_eq!(second, "t.b");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

// --- rewrites and the plan key -------------------------------------------------------------------

#[test]
fn a_rewrite_reads_options_of_any_binding() {
    // Step-1 bookkeeping (4): `RewriteCx::option` over declared reads of any
    // binding, document- and render-bound alike.
    let registry = registry(vec![feature("t", |b| {
        b.options::<Width>(Bind::Either)?;
        b.options::<Depth>(Bind::Document)?;
        b.table(LOG, "what the rewrite saw")?;
        b.rewrite_stage(rewrite(
            RewriteDecl::new("t.rw", "t.rewritten"),
            Needs::new().option::<Width>().option::<Depth>(),
            |_, tree, cx, diags| {
                let seen = format!(
                    "{} {}",
                    cx.option::<Width>().unwrap().0,
                    cx.option::<Depth>().unwrap().0
                );
                diags.push(techy::error::Diagnostic::note(
                    common::pipeline::Note { detail: seen },
                    common::pipeline::span(),
                ));
                Ok(Arc::clone(tree))
            },
        ))
    })]);
    let doc = doc_with_options(&registry, &["x"], OptionValues::new().with(Depth(3)));
    let request =
        PlanRequest::new().demand("t.rewritten").render_options(OptionValues::new().with(Width(9)));
    let run = run(&doc, &plan_for(&registry, &request), &RunOptions::new());
    let seen: Vec<String> = run
        .pipeline_diagnostics()
        .conditions::<common::pipeline::Note>()
        .map(|n| n.detail.clone())
        .collect();
    assert_eq!(seen, ["9 3"]);
}

#[test]
fn render_option_values_enter_the_plan_key() {
    let registry = registry(vec![options_feature()]);
    let with = |width: u32| {
        plan_for(
            &registry,
            &PlanRequest::new()
                .demand("t.log")
                .render_options(OptionValues::new().with(Width(width))),
        )
        .key()
    };
    assert_eq!(with(1), with(1));
    assert_ne!(with(1), with(2));
}

#[test]
fn a_workflows_render_values_are_overridden_per_type_by_the_host() {
    let registry = registry(vec![
        options_feature(),
        feature("t.wf", |b| {
            b.workflow(
                WorkflowDef::new("report")
                    .demand("t.log")
                    .render_options(OptionValues::new().with(Width(30)).with(Depth(2))),
            )
        }),
    ]);
    let overrides =
        Overrides { render_options: OptionValues::new().with(Width(40)), ..Overrides::none() };
    let plan = Arc::new(Plan::for_workflow(&registry, "report", &overrides).expect("plan"));
    let run = run(&doc(&registry, &["x"]), &plan, &RunOptions::new());
    // The host's `Width` replaces the bundle's; the bundle's `Depth` stays.
    assert_eq!(logged(&run), ["width 40", "depth 2", "margin 0"]);
}

// --- needs that follow the render request --------------------------------------------------------

/// A stage requiring `t.inspected` only when the render-bound `Strict` value is
/// set. Any stage may vary its inputs with the render request, but only through
/// `PlanCx`.
struct Publish;

#[derive(Debug, Default, Hash, Clone, Copy)]
struct Strict(bool);

impl Stage for Publish {
    fn decl(&self) -> StageDecl {
        StageDecl::new("t.publish", Effect::Compute, Scope::Document).provides("t.published")
    }

    fn needs(&self, cx: &PlanCx<'_>) -> Needs {
        match cx.render_option::<Strict>() {
            Some(Strict(true)) => Needs::new().requires("t.inspected"),
            _ => Needs::new(),
        }
    }

    fn run(&self, _cx: &StageCx<'_>, _out: &mut StageOut<'_>) {}
}

fn publish_registry() -> Arc<FlmRegistry> {
    registry(vec![feature("t", |b| {
        b.options::<Strict>(Bind::Render)?;
        b.capability(CapabilityDef::new("t.published"))?;
        b.capability(CapabilityDef::new("t.inspected"))?;
        b.stage(Arc::new(Publish))?;
        b.stage(provider("t.inspect", &["t.inspected"], &[]))
    })])
}

#[test]
fn a_stages_needs_may_follow_a_render_bound_value() {
    let registry = publish_registry();
    let relaxed = plan_for(&registry, &PlanRequest::new().demand("t.published"));
    assert_eq!(relaxed.scheduled_names(), ["t.publish"]);
    let strict = plan_for(
        &registry,
        &PlanRequest::new()
            .demand("t.published")
            .render_options(OptionValues::new().with(Strict(true))),
    );
    assert_eq!(strict.scheduled_names(), ["t.inspect", "t.publish"]);
    assert_ne!(relaxed.key(), strict.key());
}
