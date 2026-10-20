//! The runner of `flm_core::core::run`: rewrites threaded tree-major and
//! published as tables, a failing rewrite's absent tree, the folds in traversal
//! order, the walk, checks, the error gate before the first `Io` stage,
//! provided implying present, value-shaped tables, parallel output
//! bit-identical to sequential over a skewed workload, stepping, externals and
//! stamps, the chained diagnostics view, and worker threads.

mod common;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use techy::error::{Diagnostic, Severity};

use flm_core::model::addr::TreeId;
use flm_core::model::doc::{Doc, DocContent};
use flm_core::model::options::{Bind, OptionValues};
use flm_core::core::lowering::{DefaultMark, LoweringDecl};
use flm_core::core::plan::{LoweringSelection, Overrides, Plan, PlanRequest, PlanWarning};
use flm_core::core::registry::{CapabilityDef, FlmFeature, FlmRegistry, WorkflowDef};
use flm_core::core::run::{Externals, Run, RunError, RunOptions, Stamp, Step};
use flm_core::core::stage::{
    Effect, Needs, RewriteDecl, RewriteError, Scope, StageDecl, TreesTable,
};
use flm_core::core::data::store::AccessError;
use flm_core::core::data::table::TableName;
use flm_core::lang::guard;

use common::pipeline::{
    COUNT, Count, LOG, Log, Note, Other, Probe, appending, doc, doc_with_options, feature, noop,
    plan, plan_for, probe, record, recorded, registry, reparse, rewrite, run, run_with, snapshot,
    span, stage, text_of,
};
use common::stdcore::identifiers;

const ONE: TableName<TreesTable> = TableName::new("t.one");
const TWO: TableName<TreesTable> = TableName::new("t.two");
const WALK: TableName<Log> = TableName::new("t.walk");
const EXT: TableName<Count> = TableName::new("t.ext");

fn texts(run: &Run, table: TableName<TreesTable>) -> Vec<(u32, String)> {
    let trees: &TreesTable = run.table(table).unwrap().expect("published");
    trees.trees.iter().map(|(id, tree)| (id.index(), text_of(tree))).collect()
}

/// Busy work proportional to `units`, so a parallel map has room to finish
/// out of order.
fn spin(units: usize) {
    let mut acc = 0u64;
    for i in 0..units * 2_000 {
        acc = acc.wrapping_mul(31).wrapping_add(i as u64);
        std::hint::black_box(acc);
    }
}

// --- the rewrites --------------------------------------------------------------------------------

/// Two rewrites — `t.rw1` (appends `+1`, publishes `t.one`; fails on a tree
/// whose text contains `fail_on`) and `t.rw2` (appends `+2`, publishes
/// `t.two`, requires `t.one` — an `after` edge alone would order but never
/// activate the first rewrite) — a `PerTree` counter `t.count` (requires `t.two`;
/// writes `t.count` and `t.log` with each tree's text) and a `Document` walker
/// `t.walker` (writes `t.walk` with the walk's addresses).
fn chain_registry(fail_on: Option<&'static str>) -> Arc<FlmRegistry> {
    registry(vec![feature("t", move |b| {
        b.rewrite_stage(rewrite(
            RewriteDecl::new("t.rw1", "t.one"),
            Needs::new(),
            move |_, tree, cx, _| {
                let text = text_of(tree);
                if fail_on.is_some_and(|needle| text.contains(needle)) {
                    return Err(RewriteError::failed(Note { detail: "refused".into() }, span()));
                }
                Ok(reparse(cx.registry(), &format!("{text}+1"), cx))
            },
        ))?;
        b.rewrite_stage(rewrite(
            RewriteDecl::new("t.rw2", "t.two"),
            Needs::new().requires("t.one"),
            |_, tree, cx, _| Ok(reparse(cx.registry(), &format!("{}+2", text_of(tree)), cx)),
        ))?;
        b.table(COUNT, "one per tree")?;
        b.table(LOG, "each tree's text, in fold order")?;
        b.table(WALK, "the walk's addresses")?;
        b.stage(stage(
            StageDecl::new("t.count", Effect::Compute, Scope::PerTree).writes(COUNT).writes(LOG),
            Needs::new().requires("t.two"),
            |cx, out| {
                let id = cx.tree_id().expect("a per-tree run");
                let text = text_of(&cx.my_tree().expect("present"));
                spin(10 - id.index() as usize % 10);
                out.table(COUNT).unwrap().0 += 1;
                out.table(LOG).unwrap().0.push(format!("{}: {text}", id.index()));
            },
        ))?;
        b.stage(stage(
            StageDecl::new("t.walker", Effect::Compute, Scope::Document).writes(WALK),
            Needs::new().requires("t.two"),
            |cx, out| {
                let walk = out.table(WALK).unwrap();
                for (addr, node) in cx.walk() {
                    if let Some(chars) = node.chars() {
                        walk.0.push(format!("{}:{chars}", addr.tree.index()));
                    }
                }
            },
        ))
    })])
}

#[test]
fn rewrites_run_tree_major_in_plan_order_and_publish_their_trees() {
    let registry = chain_registry(None);
    let run = run(&doc(&registry, &["a", "b"]), &plan(&registry, &["t.count"]), &RunOptions::new());
    assert_eq!(texts(&run, ONE), [(0, "a+1".to_string()), (1, "b+1".to_string())]);
    assert_eq!(texts(&run, TWO), [(0, "a+1+2".to_string()), (1, "b+1+2".to_string())]);
    // The last rewrite's output is the frozen tree.
    assert_eq!(text_of(run.frozen_tree(TreeId::tree(0)).unwrap()), "a+1+2");
    assert_eq!(run.frozen_trees().count(), 2);
}

#[test]
fn a_failing_rewrite_makes_its_tree_absent_downstream_with_one_error() {
    let registry = chain_registry(Some("b"));
    let doc = doc(&registry, &["a", "b", "c"]);
    let run = run(&doc, &plan(&registry, &["t.count", "t.walk"]), &RunOptions::new());
    // Absent from the failing rewrite's table and every later one; the
    // frozen tree is absent; per-tree stages never ran for it; the document
    // stage ran and saw the gap.
    let one: Vec<u32> = texts(&run, ONE).into_iter().map(|(id, _)| id).collect();
    assert_eq!(one, [0, 2]);
    let two: Vec<u32> = texts(&run, TWO).into_iter().map(|(id, _)| id).collect();
    assert_eq!(two, [0, 2]);
    assert!(run.frozen_tree(TreeId::tree(1)).is_none());
    assert_eq!(run.table(COUNT).unwrap().unwrap().0, 2);
    let walk: &Log = run.table(WALK).unwrap().unwrap();
    assert_eq!(walk.0, ["0:a+1+2", "2:c+1+2"]);
    // Exactly one Error diagnostic — the rewrite's own — and no second one.
    assert_eq!(identifiers(&run), ["t.note"]);
    assert_eq!(run.diagnostics().error_count(), 1);
}

#[test]
fn an_earlier_rewrites_table_keeps_the_tree_a_later_one_dropped() {
    let registry = registry(vec![feature("t", |b| {
        b.rewrite_stage(appending("t.rw1", "t.one", "+1"))?;
        b.rewrite_stage(rewrite(
            RewriteDecl::new("t.rw2", "t.two"),
            Needs::new().requires("t.one"),
            |_, tree, _, _| {
                if text_of(tree).contains('b') {
                    return Err(RewriteError::failed(Note { detail: "refused".into() }, span()));
                }
                Ok(Arc::clone(tree))
            },
        ))
    })]);
    let run = run(&doc(&registry, &["a", "b"]), &plan(&registry, &["t.two"]), &RunOptions::new());
    let one: Vec<u32> = texts(&run, ONE).into_iter().map(|(id, _)| id).collect();
    let two: Vec<u32> = texts(&run, TWO).into_iter().map(|(id, _)| id).collect();
    assert_eq!(one, [0, 1]);
    assert_eq!(two, [0]);
}

#[test]
fn per_tree_pieces_fold_in_traversal_order() {
    let registry = chain_registry(None);
    let texts: Vec<String> = (0..12).map(|i| "x".repeat(i % 4 + 1)).collect();
    let borrowed: Vec<&str> = texts.iter().map(String::as_str).collect();
    let doc = doc(&registry, &borrowed);
    let plan = plan(&registry, &["t.count"]);
    for options in [RunOptions::new(), RunOptions::sequential()] {
        let run = run(&doc, &plan, &options);
        let log: &Log = run.table(LOG).unwrap().unwrap();
        let order: Vec<u32> =
            log.0.iter().map(|line| line.split(':').next().unwrap().parse().unwrap()).collect();
        assert_eq!(order, (0..12).collect::<Vec<u32>>());
    }
}

#[test]
fn a_document_stage_walks_every_present_tree_in_order() {
    let registry = chain_registry(None);
    let run = run(&doc(&registry, &["a", "b"]), &plan(&registry, &["t.walk"]), &RunOptions::new());
    let walk: &Log = run.table(WALK).unwrap().unwrap();
    assert_eq!(walk.0, ["0:a+1+2", "1:b+1+2"]);
}

#[test]
fn a_value_shaped_table_absorbs_by_summing() {
    let registry = chain_registry(None);
    let run =
        run(&doc(&registry, &["a", "b", "c"]), &plan(&registry, &["t.count"]), &RunOptions::new());
    assert_eq!(run.table(COUNT).unwrap().unwrap().0, 3);
}

#[test]
fn stages_run_on_worker_threads_under_both_paths() {
    // Every batch runs on a marked worker thread — the pool's threads, or the
    // guard's check-then-run on the sequential path.
    let seen = probe();
    let registry = {
        let seen = Arc::clone(&seen);
        registry(vec![feature("t", move |b| {
            let rewrite_seen = Arc::clone(&seen);
            b.rewrite_stage(rewrite(
                RewriteDecl::new("t.rw", "t.one"),
                Needs::new(),
                move |_, tree, _, _| {
                    record(&rewrite_seen, format!("rewrite {}", guard::is_worker_thread()));
                    Ok(Arc::clone(tree))
                },
            ))?;
            b.capability(CapabilityDef::new("t.a"))?;
            let stage_seen = Arc::clone(&seen);
            b.stage(stage(
                StageDecl::new("t.p", Effect::Compute, Scope::Document).provides("t.a"),
                Needs::new().requires("t.one"),
                move |_, _| record(&stage_seen, format!("stage {}", guard::is_worker_thread())),
            ))
        })])
    };
    for options in [RunOptions::new(), RunOptions::sequential()] {
        run(&doc(&registry, &["x"]), &plan(&registry, &["t.a"]), &options);
    }
    assert_eq!(recorded(&seen), ["rewrite true", "stage true", "rewrite true", "stage true"]);
    assert!(!guard::is_worker_thread(), "the test thread stays unmarked");
}

// --- checks and the error gate -------------------------------------------------------------------

/// `t.check` (a `Check`, warns once), `t.boom` (a `Compute` raising an Error
/// when a tree's text contains `boom`), and `t.io` (an `Io` stage recording
/// that it ran) — every one demandable on its own.
fn gate_registry(ran: Probe) -> Arc<FlmRegistry> {
    registry(vec![feature("t", move |b| {
        b.capability(CapabilityDef::new("t.checked"))?;
        b.capability(CapabilityDef::new("t.boomed"))?;
        b.capability(CapabilityDef::new("t.written"))?;
        b.stage(stage(
            StageDecl::new("t.check", Effect::Check, Scope::Document).provides("t.checked"),
            Needs::new(),
            |_, out| out.diags.push(Diagnostic::warning(Note { detail: "checked".into() }, span())),
        ))?;
        b.stage(stage(
            StageDecl::new("t.boom", Effect::Compute, Scope::PerTree).provides("t.boomed"),
            Needs::new(),
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
fn a_check_stage_reports_diagnostics_only() {
    let registry = gate_registry(probe());
    let run = run(&doc(&registry, &["x"]), &plan(&registry, &["t.checked"]), &RunOptions::new());
    let diagnostics: Vec<(Severity, String)> =
        run.diagnostics().iter().map(|d| (d.severity(), d.identifier().to_string())).collect();
    assert_eq!(diagnostics, [(Severity::Warning, "t.note".to_string())]);
    assert!(run.computed().is_empty());
    assert!(!run.diagnostics().has_errors());
}

#[test]
fn the_io_stage_runs_when_no_error_was_recorded() {
    let ran = probe();
    let registry = gate_registry(Arc::clone(&ran));
    let run = run(
        &doc(&registry, &["x"]),
        &plan(&registry, &["t.written", "t.boomed"]),
        &RunOptions::new(),
    );
    assert_eq!(recorded(&ran), ["io ran"]);
    assert!(!run.halted_at_gate());
}

#[test]
fn the_error_gate_ends_the_run_before_the_first_io_stage() {
    let ran = probe();
    let registry = gate_registry(Arc::clone(&ran));
    let doc = doc(&registry, &["boom"]);
    let plan = plan(&registry, &["t.written", "t.boomed"]);
    let mut run = Run::new(doc, plan, Externals::none()).unwrap();
    let outcome = run.execute(&RunOptions::new()).unwrap();
    assert!(outcome.halted_at_gate);
    assert!(run.halted_at_gate());
    assert!(run.is_done());
    assert!(recorded(&ran).is_empty(), "the Io stage must not run");
    assert_eq!(identifiers(&run), ["t.other"]);
}

#[test]
fn continue_with_errors_keeps_the_gate_open() {
    let ran = probe();
    let registry = gate_registry(Arc::clone(&ran));
    let options = RunOptions { continue_with_errors: true, ..RunOptions::new() };
    let run =
        run(&doc(&registry, &["boom"]), &plan(&registry, &["t.written", "t.boomed"]), &options);
    assert_eq!(recorded(&ran), ["io ran"]);
    assert!(!run.halted_at_gate());
    assert!(run.diagnostics().has_errors());
}

#[test]
fn a_parse_error_closes_the_gate_too() {
    let ran = probe();
    let registry = gate_registry(Arc::clone(&ran));
    let doc = doc(&registry, &[r"\begin{nope}x\end{nope}"]);
    assert!(doc.content().fragment(TreeId::tree(0)).unwrap().diags().has_errors());
    let run = run(&doc, &plan(&registry, &["t.written"]), &RunOptions::new());
    assert!(run.halted_at_gate());
    assert!(recorded(&ran).is_empty());
}

#[test]
fn a_warning_does_not_close_the_gate() {
    let ran = probe();
    let registry = gate_registry(Arc::clone(&ran));
    let run = run(
        &doc(&registry, &["x"]),
        &plan(&registry, &["t.written", "t.checked"]),
        &RunOptions::new(),
    );
    assert_eq!(recorded(&ran), ["io ran"]);
    assert!(run.diagnostics().any_recorded());
}

// --- provided ⇒ present, reads -----------------------------------------------------------------

#[test]
fn a_declared_write_that_produced_nothing_installs_an_empty_table() {
    let registry = registry(vec![feature("t", |b| {
        b.table(COUNT, "a count")?;
        b.stage(noop(
            StageDecl::new("t.silent", Effect::Compute, Scope::PerTree).writes(COUNT),
            Needs::new(),
        ))
    })]);
    let run = run(&doc(&registry, &["x"]), &plan(&registry, &["t.count"]), &RunOptions::new());
    assert_eq!(run.table(COUNT).unwrap().unwrap().0, 0);
}

#[test]
fn an_optional_read_of_an_unprovided_table_reads_none() {
    let registry = registry(vec![feature("t", |b| {
        b.table(COUNT, "a count")?;
        b.table(LOG, "never written")?;
        b.stage(stage(
            StageDecl::new("t.opt", Effect::Compute, Scope::Document).writes(COUNT),
            Needs::new().reads_opt(LOG),
            |cx, out| {
                assert!(cx.get(LOG).unwrap().is_none(), "not provided reads None");
                out.table(COUNT).unwrap().0 = 1;
            },
        ))
    })]);
    let run = run(&doc(&registry, &["x"]), &plan(&registry, &["t.count"]), &RunOptions::new());
    assert_eq!(run.table(COUNT).unwrap().unwrap().0, 1);
    assert!(run.table(LOG).unwrap().is_none());
}

#[test]
fn an_undeclared_read_at_run_time_is_a_typed_error_naming_stage_and_table() {
    let registry = registry(vec![feature("t", |b| {
        b.table(COUNT, "a count")?;
        b.table(LOG, "a log")?;
        b.stage(stage(
            StageDecl::new("t.greedy", Effect::Compute, Scope::Document).writes(COUNT),
            Needs::new(),
            |cx, out| {
                match cx.get(LOG) {
                    Err(AccessError::UndeclaredRead { stage, table, declared }) => {
                        assert_eq!(stage, "t.greedy");
                        assert_eq!(table, "t.log");
                        assert!(declared.is_empty());
                    }
                    other => panic!("unexpected: {other:?}"),
                }
                match out.table(LOG) {
                    Err(AccessError::UndeclaredWrite { stage, table, declared }) => {
                        assert_eq!(stage, "t.greedy");
                        assert_eq!(table, "t.log");
                        assert_eq!(declared, ["t.count"]);
                    }
                    other => panic!("unexpected: {other:?}"),
                }
                out.table(COUNT).unwrap().0 = 1;
            },
        ))
    })]);
    let run = run(&doc(&registry, &["x"]), &plan(&registry, &["t.count"]), &RunOptions::new());
    assert_eq!(run.table(COUNT).unwrap().unwrap().0, 1, "the stage ran to its end");
}

#[test]
fn the_host_reads_a_table_by_key() {
    let registry = chain_registry(None);
    let run = run(&doc(&registry, &["a"]), &plan(&registry, &["t.count"]), &RunOptions::new());
    assert!(run.table(COUNT).unwrap().is_some());
    assert!(run.table(WALK).unwrap().is_none(), "declared, not scheduled: not provided");
    assert!(matches!(
        run.table(TableName::<Log>::new("t.count")),
        Err(AccessError::TypeMismatch { table, .. }) if table == "t.count"
    ));
    assert!(matches!(
        run.table(TableName::<Log>::new("t.nope")),
        Err(AccessError::UndeclaredRead { table, .. }) if table == "t.nope"
    ));
}

#[test]
fn document_stages_run_once_and_per_tree_stages_once_per_tree() {
    let seen = probe();
    let registry = {
        let seen = Arc::clone(&seen);
        registry(vec![feature("t", move |b| {
            b.capability(CapabilityDef::new("t.a"))?;
            b.capability(CapabilityDef::new("t.b"))?;
            let per_tree = Arc::clone(&seen);
            b.stage(stage(
                StageDecl::new("t.per-tree", Effect::Compute, Scope::PerTree).provides("t.a"),
                Needs::new(),
                move |cx, _| {
                    record(&per_tree, format!("per-tree {}", cx.tree_id().unwrap().index()))
                },
            ))?;
            let document = Arc::clone(&seen);
            b.stage(stage(
                StageDecl::new("t.document", Effect::Compute, Scope::Document).provides("t.b"),
                Needs::new().after("t.a"),
                move |cx, _| record(&document, format!("document {:?}", cx.tree_id())),
            ))
        })])
    };
    run(&doc(&registry, &["a", "b"]), &plan(&registry, &["t.a", "t.b"]), &RunOptions::sequential());
    assert_eq!(recorded(&seen), ["per-tree 0", "per-tree 1", "document None"]);
}

// --- determinism ---------------------------------------------------------------------------------

/// Twenty-four trees of wildly uneven size, so the parallel map is far from
/// uniform.
fn skewed_texts(seed: usize) -> Vec<String> {
    (0..24)
        .map(|i| {
            let words = (i * 7 + seed) % 11 + 1;
            (0..words).map(|j| format!("w{i}-{j}")).collect::<Vec<_>>().join(" ")
        })
        .collect()
}

fn skewed_doc(registry: &FlmRegistry, seed: usize) -> Arc<Doc> {
    let texts = skewed_texts(seed);
    let borrowed: Vec<&str> = texts.iter().map(String::as_str).collect();
    doc(registry, &borrowed)
}

#[test]
fn parallel_output_is_bit_identical_to_sequential_over_a_skewed_workload() {
    let registry = chain_registry(None);
    let plan = plan(&registry, &["t.count", "t.walk"]);
    let doc = skewed_doc(&registry, 0);
    let sequential = snapshot(&run(&doc, &plan, &RunOptions::sequential()));
    let parallel = snapshot(&run(&doc, &plan, &RunOptions::new()));
    assert_eq!(parallel, sequential, "parallel output must be bit-identical");
    // Again, to catch anything that merely happened to agree once.
    assert_eq!(snapshot(&run(&doc, &plan, &RunOptions::new())), parallel);
    assert!(parallel.contains("t.log = "), "the snapshot is not vacuous:\n{parallel}");
}

#[test]
fn a_plan_is_a_reusable_value_across_documents_and_runs() {
    let registry = chain_registry(None);
    let plan = plan(&registry, &["t.count"]);
    let first = skewed_doc(&registry, 0);
    let second = skewed_doc(&registry, 3);
    let a = snapshot(&run(&first, &plan, &RunOptions::new()));
    let b = snapshot(&run(&second, &plan, &RunOptions::new()));
    let a_again = snapshot(&run(&first, &plan, &RunOptions::new()));
    assert_eq!(a, a_again, "same plan, same document, same bytes");
    assert_ne!(a, b, "different documents really differ");
}

#[test]
fn stepping_is_observationally_equivalent_to_execute() {
    let registry = chain_registry(None);
    let plan = plan(&registry, &["t.count", "t.walk"]);
    let doc = skewed_doc(&registry, 1);
    let whole = snapshot(&run(&doc, &plan, &RunOptions::new()));
    let mut stepped = Run::new(Arc::clone(&doc), Arc::clone(&plan), Externals::none()).unwrap();
    let mut steps = 0;
    while stepped.step(&RunOptions::new()).unwrap() == Step::More {
        steps += 1;
    }
    // The rewrites, then one step per batch; stepping past the end is a no-op.
    assert_eq!(steps, plan.batches().len());
    assert_eq!(stepped.step(&RunOptions::new()).unwrap(), Step::Done);
    assert_eq!(snapshot(&stepped), whole);
}

#[derive(Default)]
struct Concurrency {
    live: AtomicUsize,
    high: AtomicUsize,
}

impl Concurrency {
    fn enter(&self) {
        let now = self.live.fetch_add(1, Ordering::SeqCst) + 1;
        self.high.fetch_max(now, Ordering::SeqCst);
    }

    fn leave(&self) {
        self.live.fetch_sub(1, Ordering::SeqCst);
    }

    /// Waits (bounded) for a second stage to join, so a high-water mark of two
    /// is evidence rather than luck.
    fn rendezvous(&self) {
        let deadline = Instant::now() + Duration::from_millis(500);
        while self.high.load(Ordering::SeqCst) < 2 && Instant::now() < deadline {
            std::hint::spin_loop();
        }
    }
}

/// Two independent `Document` stages sharing a batch, each observing how many
/// stages are in flight.
fn concurrency_registry(observer: Arc<Concurrency>) -> Arc<FlmRegistry> {
    registry(vec![feature("t", move |b| {
        b.capability(CapabilityDef::new("t.a"))?;
        b.capability(CapabilityDef::new("t.b"))?;
        for (name, capability) in [("t.slow-a", "t.a"), ("t.slow-b", "t.b")] {
            let observer = Arc::clone(&observer);
            b.stage(stage(
                StageDecl::new(name, Effect::Compute, Scope::Document).provides(capability),
                Needs::new(),
                move |_, _| {
                    observer.enter();
                    observer.rendezvous();
                    observer.leave();
                },
            ))?;
        }
        Ok(())
    })])
}

#[test]
fn the_sequential_path_runs_one_stage_at_a_time() {
    let observer = Arc::new(Concurrency::default());
    let registry = concurrency_registry(Arc::clone(&observer));
    let plan = plan(&registry, &["t.a", "t.b"]);
    assert_eq!(plan.batches().len(), 1, "independent stages share a batch");
    run(&doc(&registry, &["x"]), &plan, &RunOptions::sequential());
    assert_eq!(observer.high.load(Ordering::SeqCst), 1);
}

/// Meaningful only with the feature on; the runner's pool has one thread per
/// processor — or as many as `RAYON_NUM_THREADS` says, which rayon's default
/// honors — so a pool of one thread cannot show two stages in flight.
#[cfg(feature = "parallel")]
#[test]
fn independent_document_stages_run_concurrently_under_parallel() {
    let observer = Arc::new(Concurrency::default());
    let registry = concurrency_registry(Arc::clone(&observer));
    run(&doc(&registry, &["x"]), &plan(&registry, &["t.a", "t.b"]), &RunOptions::new());
    let cpus = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let threads = std::env::var("RAYON_NUM_THREADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|threads| *threads > 0)
        .unwrap_or(cpus);
    if cpus >= 2 && threads >= 2 {
        assert_eq!(observer.high.load(Ordering::SeqCst), 2, "both stages were in flight at once");
    }
}

// --- externals and stamps ------------------------------------------------------------------------

/// The external table `t.ext` (a count) read by `t.reader`, which writes what
/// it read into the log; with `rewrite`, a rewrite reading the `Width` option.
fn externals_registry(with_rewrite: bool) -> Arc<FlmRegistry> {
    registry(vec![feature("t", move |b| {
        b.options::<Width>(Bind::Either)?;
        b.external_table(EXT, "the corpus count: install `t.ext` from the scan run")?;
        b.table(COUNT, "an ordinary count")?;
        b.table(LOG, "what the reader saw")?;
        if with_rewrite {
            b.rewrite_stage(rewrite(
                RewriteDecl::new("t.rw", "t.rewritten"),
                Needs::new().option::<Width>(),
                |_, tree, _, _| Ok(Arc::clone(tree)),
            ))?;
        }
        b.stage(stage(
            StageDecl::new("t.reader", Effect::Compute, Scope::Document).writes(LOG),
            Needs::new().reads(EXT),
            |cx, out| {
                let count = cx.get(EXT).unwrap().expect("installed");
                out.table(LOG).unwrap().0.push(format!("ext {}", count.0));
            },
        ))
    })])
}

#[derive(Debug, Default, Hash, Clone)]
struct Width(u32);

#[test]
fn a_missing_install_is_a_typed_error_naming_table_and_docs() {
    let registry = externals_registry(false);
    let plan = plan(&registry, &["t.log"]);
    assert_eq!(plan.external_names().iter().map(|n| n.to_string()).collect::<Vec<_>>(), ["t.ext"]);
    match Run::new(doc(&registry, &["x"]), plan, Externals::none()).expect_err("missing") {
        RunError::MissingExternal { table, docs } => {
            assert_eq!(table, "t.ext");
            assert_eq!(docs.contains("install `t.ext`"), cfg!(feature = "docs"));
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_unstamped_install_is_read_by_the_consumer() {
    let registry = externals_registry(false);
    let externals = Externals::none().install_unstamped(EXT, Count(7));
    let run = run_with(
        &doc(&registry, &["x"]),
        &plan(&registry, &["t.log"]),
        externals,
        &RunOptions::new(),
    );
    let log: &Log = run.table(LOG).unwrap().unwrap();
    assert_eq!(log.0, ["ext 7"]);
}

#[test]
fn an_install_of_an_unknown_table_lists_the_external_roster() {
    let registry = externals_registry(false);
    let externals =
        Externals::none().install_unstamped(TableName::<Count>::new("t.nope"), Count(1));
    match Run::new(doc(&registry, &["x"]), plan(&registry, &["t.log"]), externals).err().unwrap() {
        RunError::UnknownExternal { table, roster } => {
            assert_eq!(table, "t.nope");
            assert_eq!(roster, ["t.ext"]);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_install_into_a_table_a_stage_provides_is_refused() {
    let registry = externals_registry(false);
    let externals =
        Externals::none().install_unstamped(EXT, Count(1)).install_unstamped(COUNT, Count(1));
    match Run::new(doc(&registry, &["x"]), plan(&registry, &["t.log"]), externals).err().unwrap() {
        RunError::NotExternal { table } => assert_eq!(table, "t.count"),
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn an_install_of_the_wrong_type_is_refused_naming_both_types() {
    let registry = externals_registry(false);
    let externals =
        Externals::none().install_unstamped(TableName::<Log>::new("t.ext"), Log::default());
    match Run::new(doc(&registry, &["x"]), plan(&registry, &["t.log"]), externals).err().unwrap() {
        RunError::ExternalType { table, declared, installed } => {
            assert_eq!(table, "t.ext");
            assert_eq!(declared, "Count");
            assert_eq!(installed, "Log");
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn a_matching_stamp_is_accepted_and_a_mismatch_names_both_sides() {
    let registry = externals_registry(true);
    let plan = plan(&registry, &["t.log", "t.rewritten"]);
    let doc = doc(&registry, &["x"]);
    // The consuming run's stamp, learned from a run over the same plan.
    let probe = Run::new(
        Arc::clone(&doc),
        Arc::clone(&plan),
        Externals::none().install_unstamped(EXT, Count(1)),
    )
    .unwrap();
    let stamp = probe.stamp();
    assert_eq!(stamp.digest, registry.digest());
    assert_eq!(stamp.active_rewrites.iter().map(|n| n.to_string()).collect::<Vec<_>>(), ["t.rw"]);
    let stamped = Externals::none().install(EXT, Count(2), stamp.clone());
    let run = run_with(&doc, &plan, stamped, &RunOptions::new());
    assert_eq!(run.table(LOG).unwrap().unwrap().0, ["ext 2"]);
    let stale = Stamp { digest: stamp.digest.wrapping_add(1), ..stamp.clone() };
    let externals = Externals::none().install(EXT, Count(2), stale.clone());
    match Run::new(doc, plan, externals).err().unwrap() {
        RunError::StampMismatch { table, installed, expected } => {
            assert_eq!(table, "t.ext");
            assert_eq!(installed, stale);
            assert_eq!(expected, stamp);
        }
        other => panic!("unexpected error: {other:?}"),
    }
}

#[test]
fn the_stamp_follows_the_active_rewrites_and_the_options_they_read() {
    let registry = externals_registry(true);
    let stamp_under = |demands: &[&str], width: u32| {
        let doc = doc_with_options(&registry, &["x"], OptionValues::new().with(Width(width)));
        let externals = Externals::none().install_unstamped(EXT, Count(1));
        Run::new(doc, plan(&registry, demands), externals).unwrap().stamp()
    };
    // The rewrite reads `Width`: its value enters the stamp.
    assert_ne!(
        stamp_under(&["t.log", "t.rewritten"], 1),
        stamp_under(&["t.log", "t.rewritten"], 2)
    );
    // No active rewrite: the options are irrelevant to the scan.
    assert_eq!(stamp_under(&["t.log"], 1), stamp_under(&["t.log"], 2));
    assert_ne!(stamp_under(&["t.log"], 1), stamp_under(&["t.log", "t.rewritten"], 1));
}

// --- the chained diagnostics view ----------------------------------------------------------------

#[test]
fn the_chained_view_lists_parse_diagnostics_in_traversal_order_then_the_pipelines() {
    let registry = gate_registry(probe());
    let doc = doc(&registry, &["ok", r"\begin{nope}x\end{nope}"]);
    let options = RunOptions { continue_with_errors: true, ..RunOptions::new() };
    let run = run(&doc, &plan(&registry, &["t.checked"]), &options);
    let severities: Vec<Severity> = run.diagnostics().iter().map(|d| d.severity()).collect();
    assert_eq!(severities, [Severity::Error, Severity::Warning]);
    assert_eq!(run.diagnostics().retained_len(), 2);
    assert_eq!(run.diagnostics().error_count(), 1);
    assert!(run.diagnostics().has_errors());
    assert_eq!(run.pipeline_diagnostics().len(), 1, "the pipeline sink holds only its own");
}

#[test]
fn a_fragment_placed_twice_has_one_set_of_parse_diagnostics() {
    let registry = chain_registry(None);
    let fragment = Arc::new(common::parse(registry.lang(), r"\begin{nope}x\end{nope}"));
    let mut content = DocContent::new();
    content.place(Arc::clone(&fragment));
    content.place(fragment);
    let doc = Arc::new(Doc::new(content));
    let run = run(&doc, &plan(&registry, &["t.count"]), &RunOptions::new());
    assert_eq!(run.table(COUNT).unwrap().unwrap().0, 2, "two trees");
    assert_eq!(run.diagnostics().retained_len(), 1, "one fragment's diagnostics");
}

#[test]
fn a_preference_that_cannot_be_honored_leads_the_runs_diagnostics_as_a_warning() {
    let registry = common::pipeline::toy_registry(
        &[("X", None, false), ("Y", None, false)],
        &[("P", vec!["X", "Y"], vec![]), ("Q", vec!["Y"], vec![])],
    );
    let plan = plan_for(&registry, &PlanRequest::new().demand("X").demand("Y").prefer("Q"));
    let run = run(&doc(&registry, &["x"]), &plan, &RunOptions::new());
    let first = run.diagnostics().iter().next().expect("the warning");
    assert_eq!(first.identifier(), "flm.run.ignored-preference");
    assert_eq!(first.severity(), Severity::Warning);
    assert_eq!(
        first.message(),
        "the plan request prefers the stage `Q`, but that stage cannot be scheduled: the scheduled stage `P` also provides `Y`, and only one scheduled stage may provide each capability"
    );
}

#[test]
fn a_workflows_preference_that_cannot_be_honored_names_the_workflow() {
    let registry = registry(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("X"))?;
        b.capability(CapabilityDef::new("Y"))?;
        b.stage(common::pipeline::provider("P", &["X", "Y"], &[]))?;
        b.stage(common::pipeline::provider("Q", &["Y"], &[]))?;
        b.workflow(WorkflowDef::new("w").demand("X").demand("Y").prefer("Q"))
    })]);
    let plan = Arc::new(Plan::for_workflow(&registry, "w", &Overrides::none()).expect("plan"));
    assert!(matches!(
        plan.warnings(),
        [PlanWarning::IgnoredPreference { workflow: Some(w), .. }] if w == "w"
    ));
    let run = run(&doc(&registry, &["x"]), &plan, &RunOptions::new());
    let first = run.diagnostics().iter().next().expect("the warning");
    assert_eq!(first.identifier(), "flm.run.ignored-preference");
    assert!(
        first.message().starts_with("the plan request for workflow `w` prefers the stage `Q`, but"),
        "{}",
        first.message()
    );
}

/// The toy payload a peer element declaration names.
#[derive(Debug)]
struct BoxPayload;

impl flm_core::lang::ext::PresPayload for BoxPayload {
    fn project(&self) -> techy::serialize::SerialValue {
        techy::serialize::SerialValue::Null
    }
}

/// A feature owning `t.foo` and one rule for it, the default on the backend
/// `html` and producing `t.pres.box`, both declared as peer declarations. In a
/// registry without `html` the rule is set aside; in one with it the rule takes
/// part, but belongs to `html` alone.
fn html_rule_feature() -> Arc<dyn FlmFeature> {
    feature("t.rule", |b| {
        b.element(common::macro_def("t", "foo", 0))?;
        b.peer_backend("html", "the backend the rule is the default on")?;
        b.peer_element::<BoxPayload>("t.pres.box", "the box the rule stages")?;
        b.lowering_rule(common::pipeline::rule(
            "t.foo.lower.html",
            "t.foo",
            LoweringDecl::new().produces("t.pres.box"),
            DefaultMark::OnBackends(vec!["html".into()]),
        ))
    })
}

/// The request that renders `text` accepting `t.foo` as a hole and preferring
/// the `html` rule for it, which this backend cannot honor either way.
fn prefers_the_html_rule() -> PlanRequest {
    PlanRequest::new()
        .demand(common::pipeline::lowered_cap("text"))
        .unsupported("text", "t.foo")
        .lowering("text", LoweringSelection::new().prefer("t.foo.lower.html"))
}

#[test]
fn a_preference_for_a_set_aside_rule_leads_the_runs_diagnostics_as_a_warning() {
    // No feature registers `html` or `t.pres.box`, so the rule is set aside and
    // the preference naming it cannot be honored.
    let registry = registry(vec![
        html_rule_feature(),
        feature("t.text", |b| common::pipeline::register_backend(b, "text", &[])),
    ]);
    let run = run(
        &doc(&registry, &["x"]),
        &plan_for(&registry, &prefers_the_html_rule()),
        &RunOptions::new(),
    );
    let first = run.diagnostics().iter().next().expect("the warning");
    assert_eq!(first.identifier(), "flm.run.preferred-rule-set-aside");
    assert_eq!(first.severity(), Severity::Warning);
    assert_eq!(
        first.message(),
        "the plan request prefers the lowering rule `t.foo.lower.html` on backend `text`, but that rule cannot be used: it \
         depends on backend `html`, element `t.pres.box`, which no registered feature declares"
    );
}

#[test]
fn a_preference_for_a_rule_of_another_backend_leads_the_runs_diagnostics_as_a_warning() {
    // Here `html` and `t.pres.box` are registered, so the rule takes part — on
    // `html`. On `text` it is no candidate, and the preference says so.
    let registry = registry(vec![
        html_rule_feature(),
        feature("t.text", |b| common::pipeline::register_backend(b, "text", &[])),
        feature("t.html", |b| {
            b.presentation_element::<BoxPayload>("t.pres.box", "a box")?;
            common::pipeline::register_backend(b, "html", &["t.pres.box"])
        }),
    ]);
    assert_eq!(registry.set_aside_rules().count(), 0);
    let run = run(
        &doc(&registry, &["x"]),
        &plan_for(&registry, &prefers_the_html_rule()),
        &RunOptions::new(),
    );
    let first = run.diagnostics().iter().next().expect("the warning");
    assert_eq!(first.identifier(), "flm.run.preferred-rule-not-candidate");
    assert_eq!(first.severity(), Severity::Warning);
    assert_eq!(
        first.message(),
        "the plan request prefers the lowering rule `t.foo.lower.html` on backend `text`, but that rule is marked as the default on other backends only, which restricts it to those backends"
    );
}

#[test]
fn the_pipeline_sink_honors_the_retention_cap() {
    let registry = registry(vec![feature("t", |b| {
        b.capability(CapabilityDef::new("t.noisy"))?;
        b.stage(stage(
            StageDecl::new("t.noise", Effect::Compute, Scope::PerTree).provides("t.noisy"),
            Needs::new(),
            |_, out| out.diags.push(Diagnostic::error(Note { detail: "noise".into() }, span())),
        ))
    })]);
    let options = RunOptions { max_diagnostics: Some(2), ..RunOptions::new() };
    let run = run(&doc(&registry, &["a", "b", "c"]), &plan(&registry, &["t.noisy"]), &options);
    assert_eq!(run.diagnostics().retained_len(), 2, "retained");
    assert_eq!(run.diagnostics().error_count(), 3, "counted, suppressed included");
}
