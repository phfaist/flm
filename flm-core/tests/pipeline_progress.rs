//! A run on the progress sheet in `flm_core::core::run`: reporting changes
//! nothing that is computed, the rows of the rewrites and of every scheduled
//! stage, a cancellation that discards the step under way, and value catch-up,
//! which adds no row.

mod common;

use std::sync::Arc;

use techy::error::Diagnostic;

use flm_core::core::registry::{CapabilityDef, FlmRegistry};
use flm_core::core::run::{Externals, Run, RunOptions, RunOutcome};
use flm_core::core::stage::{Effect, Needs, Scope, StageDecl};
use flm_core::progress::{Kind, Observer, RowSnapshot, RowState, Sheet, Snapshot};
use flm_core::stdlib::features::refs::UnresolvedRefs;

use common::pipeline::{
    self, COUNT, LOG, Log, MARKS, Other, Probe, appending, doc, feature, plan, probe,
    record, recorded, registry, span, stage, text_of, tree,
};

const BUILD: Kind = Kind::new("test.build");
const ITEMS: Kind = Kind::new("test.items");

/// A sheet with no observer and no log sink, and its root row.
fn new_sheet() -> (Sheet, flm_core::progress::Task) {
    Sheet::start(&BUILD, None, Observer::default(), None)
}

/// Runs `plan` over `doc` to the end, reporting under `root`'s scope when
/// there is one.
fn run_on(
    doc: &Arc<flm_core::model::doc::Doc>,
    plan: &Arc<flm_core::core::plan::Plan>,
    root: Option<&flm_core::progress::Task>,
    options: &RunOptions,
) -> (Run, RunOutcome) {
    let mut run = Run::new(Arc::clone(doc), Arc::clone(plan), Externals::none()).unwrap();
    if let Some(root) = root {
        run = run.with_progress(root.scope());
    }
    let outcome = run.execute(options).unwrap();
    (run, outcome)
}

/// The one row of `snapshot` of kind `kind`.
fn row<'s>(snapshot: &'s Snapshot, kind: &str) -> &'s RowSnapshot {
    let mut rows = snapshot.rows.iter().filter(|row| row.kind == kind);
    let found = rows.next().unwrap_or_else(|| panic!("a row of kind `{kind}`"));
    assert!(rows.next().is_none(), "one row of kind `{kind}`");
    found
}

/// A rewrite `t.rw`, the document-scope check `t.check`, the per-tree stage
/// `t.boom` (an Error on a tree whose text holds `boom`), and the
/// document-scope `Io` stage `t.io`, which records that it ran.
fn gate_registry(ran: Probe) -> Arc<FlmRegistry> {
    registry(vec![feature("t", move |b| {
        b.rewrite_stage(appending("t.rw", "t.rewritten", "+"))?;
        b.capability(CapabilityDef::new("t.checked"))?;
        b.capability(CapabilityDef::new("t.boomed"))?;
        b.capability(CapabilityDef::new("t.written"))?;
        b.stage(stage(
            StageDecl::new("t.check", Effect::Check, Scope::Document).provides("t.checked"),
            Needs::new(),
            |_, _| {},
        ))?;
        b.stage(stage(
            StageDecl::new("t.boom", Effect::Compute, Scope::PerTree).provides("t.boomed"),
            Needs::new().requires("t.rewritten"),
            |cx, out| {
                if text_of(&cx.my_tree().unwrap()).contains("boom") {
                    out.diags.push(Diagnostic::error(Other { detail: "boom".into() }, span()));
                }
            },
        ))?;
        let ran = Arc::clone(&ran);
        b.stage(stage(
            StageDecl::new("t.io", Effect::Io, Scope::Document).provides("t.written"),
            Needs::new(),
            move |_, _| record(&ran, "io ran"),
        ))
    })])
}

#[test]
fn a_sheet_changes_nothing_that_is_computed_on_either_path() {
    use common::spine::{FEATURES_DOCUMENT, SESSION06_DOCUMENT, overrides, snapshot, text_plan};
    let registry = common::spine::registry();
    let mut texts = vec![FEATURES_DOCUMENT.to_string(), SESSION06_DOCUMENT.to_string()];
    texts.extend((0..6).map(|i| format!("Tree {i} with \\emph{{some}} \\(x_{i}\\) content.")));
    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
    let doc = pipeline::doc(&registry, &texts);
    let plan = text_plan(&registry, &overrides(Some(40), UnresolvedRefs::Allow));

    let (bare, _) = run_on(&doc, &plan, None, &RunOptions::sequential());
    let expected = snapshot(&bare);
    for options in [RunOptions::sequential(), RunOptions::new()] {
        let (_sheet, root) = new_sheet();
        let (reported, outcome) = run_on(&doc, &plan, Some(&root), &options);
        assert_eq!(outcome, RunOutcome::Finished);
        assert_eq!(snapshot(&reported), expected, "with a sheet, {options:?}");
        let (unreported, _) = run_on(&doc, &plan, None, &options);
        assert_eq!(snapshot(&unreported), expected, "without a sheet, {options:?}");
    }
}

#[test]
fn a_run_ends_the_rows_of_its_rewrites_and_of_every_scheduled_stage() {
    let demands = ["t.checked", "t.boomed", "t.written"];

    let ran = probe();
    let registry = gate_registry(Arc::clone(&ran));
    let (sheet, root) = new_sheet();
    let document = doc(&registry, &["a", "b", "c"]);
    let (_run, outcome) =
        run_on(&document, &plan(&registry, &demands), Some(&root), &RunOptions::new());
    assert_eq!(outcome, RunOutcome::Finished);
    let snapshot = sheet.snapshot();
    for kind in ["flm.run.rewrites", "t.check", "t.boom", "t.io"] {
        assert_eq!(row(&snapshot, kind).state, RowState::Finished, "{kind}");
    }
    let count = row(&snapshot, "t.boom").count.expect("a per-tree stage counts its trees");
    assert_eq!((count.done, count.total), (3, 3));
    assert_eq!(row(&snapshot, "t.check").count, None);

    // A run stopped at the error gate: the `Io` stage's row is not run.
    let ran = probe();
    let registry = gate_registry(Arc::clone(&ran));
    let (sheet, root) = new_sheet();
    let document = doc(&registry, &["a", "boom"]);
    let (_run, outcome) =
        run_on(&document, &plan(&registry, &demands), Some(&root), &RunOptions::new());
    assert_eq!(outcome, RunOutcome::HaltedAtGate);
    let snapshot = sheet.snapshot();
    assert_eq!(row(&snapshot, "t.boom").state, RowState::Finished);
    assert_eq!(row(&snapshot, "t.io").state, RowState::NotRun);
    assert!(recorded(&ran).is_empty(), "the Io stage did not run");
}

#[test]
fn a_cancellation_discards_the_step_under_way_and_keeps_the_earlier_ones() {
    let ran = probe();
    let (sheet, root) = new_sheet();
    let registry = {
        let ran = Arc::clone(&ran);
        let sheet = sheet.clone();
        registry(vec![feature("t", move |b| {
            b.table(LOG, "what the first stage wrote")?;
            b.table(COUNT, "one per tree, before the cancellation")?;
            b.capability(CapabilityDef::new("t.later"))?;
            b.stage(stage(
                StageDecl::new("t.first", Effect::Compute, Scope::Document).writes(LOG),
                Needs::new(),
                |_, out| out.table(LOG).unwrap().0.push("first".into()),
            ))?;
            let sheet = sheet.clone();
            let cancelling = Arc::clone(&ran);
            b.stage(stage(
                StageDecl::new("t.cancel", Effect::Compute, Scope::PerTree).writes(COUNT),
                Needs::new().reads(LOG),
                move |_, out| {
                    record(&cancelling, "cancel ran");
                    out.table(COUNT).unwrap().0 += 1;
                    sheet.cancel();
                },
            ))?;
            let ran = Arc::clone(&ran);
            b.stage(stage(
                StageDecl::new("t.later", Effect::Compute, Scope::Document).provides("t.later"),
                Needs::new().reads(COUNT),
                move |_, _| record(&ran, "later ran"),
            ))
        })])
    };
    let document = doc(&registry, &["a", "b"]);
    let (run, outcome) =
        run_on(&document, &plan(&registry, &["t.later"]), Some(&root), &RunOptions::sequential());

    assert_eq!(outcome, RunOutcome::Cancelled);
    assert_eq!(run.outcome(), Some(RunOutcome::Cancelled));
    assert!(run.table(COUNT).unwrap().is_none(), "nothing of the cancelled batch is folded");
    let first: &Log = run.table(LOG).unwrap().expect("the earlier batch stays readable");
    assert_eq!(first.0, ["first"]);
    assert_eq!(recorded(&ran), ["cancel ran"], "the second tree and the later stage are skipped");
    let snapshot = sheet.snapshot();
    assert!(snapshot.cancelled);
    assert_eq!(row(&snapshot, "t.first").state, RowState::Finished);
    assert_eq!(row(&snapshot, "t.cancel").state, RowState::Failed);
    assert_eq!(row(&snapshot, "t.later").state, RowState::NotRun);
}

#[test]
fn the_stages_run_over_a_synthesized_value_add_no_row() {
    let seen = probe();
    let marking = Arc::clone(&seen);
    let registry = registry(vec![feature("t", move |b| {
        b.rewrite_stage(appending("t.rw", "t.rewritten", "+"))?;
        b.table(MARKS, "written by the per-tree stage")?;
        b.table(LOG, "what the consumer saw")?;
        let marking = Arc::clone(&marking);
        b.stage(stage(
            StageDecl::new("t.mark", Effect::Compute, Scope::PerTree).writes(MARKS),
            Needs::new().requires("t.rewritten"),
            move |cx, _| {
                let items = cx.progress().child(&ITEMS, None).start();
                record(&marking, format!("mark {}", cx.tree_id().unwrap()));
                items.finish();
            },
        ))?;
        b.stage(stage(
            StageDecl::new("t.consume", Effect::Compute, Scope::Document).writes(LOG),
            Needs::new().reads(MARKS),
            |cx, out| {
                let caught = cx.resolve_value(&tree(cx.registry(), "v")).expect("caught up");
                out.table(LOG).unwrap().0.push(text_of(&caught.tree));
            },
        ))
    })]);
    let (sheet, root) = new_sheet();
    let document = doc(&registry, &["doc"]);
    let (run, _) =
        run_on(&document, &plan(&registry, &["t.log"]), Some(&root), &RunOptions::sequential());

    let log: &Log = run.table(LOG).unwrap().expect("written");
    assert_eq!(log.0, ["v+"], "the value was brought up to date");
    assert_eq!(recorded(&seen).len(), 2, "the per-tree stage ran over the tree and the value");
    let snapshot = sheet.snapshot();
    let kinds: Vec<&str> = snapshot.rows.iter().map(|row| row.kind.as_str()).collect();
    assert_eq!(kinds, ["test.build", "flm.run.rewrites", "t.mark", "t.consume", "test.items"]);
    let count = row(&snapshot, "t.mark").count.expect("a per-tree stage counts its trees");
    assert_eq!((count.done, count.total), (1, 1));
    assert_eq!(row(&snapshot, "test.items").parent, Some(row(&snapshot, "t.mark").id));
}
