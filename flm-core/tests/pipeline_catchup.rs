//! Value catch-up in `flm_core::core::run`: a consumer catching a synthesized
//! tree up runs the plan's active rewrites over it, then the `PerTree` stages
//! of its own dependency closure over the value alone, reads the results as an
//! overlay, and never touches the main store. Also the numbering of values, the
//! typed failures, and the condition reported at the access site when an entry
//! is missing.

mod common;

use std::sync::Arc;

use techy::error::Diagnostic;

use flm_core::model::addr::TreeId;
use flm_core::model::doc::FragmentLayout;
use flm_core::model::names::{CapName, ElementName};
use flm_core::core::registry::FlmRegistry;
use flm_core::core::run::{CatchUpError, MissingEntry, RunOptions, StageCx};
use flm_core::core::stage::{Effect, Needs, RewriteDecl, RewriteError, Scope, StageDecl};
use flm_core::core::data::store::{StageOut, StoreError};
use flm_core::core::data::table::TableName;
use flm_core::lang::guard::NestingTooDeep;

use common::pipeline::{
    FACT, LOG, Log, MARKS, Marks, Note, Probe, doc, feature, plan, probe, record, recorded,
    registry, reparse, rewrite, run, span, stage, text_of, tree,
};

const DOCS: TableName<Log> = TableName::new("t.docs");
const OTHER: TableName<Marks> = TableName::new("t.other");

type Consumer = Arc<dyn Fn(&StageCx<'_>, &mut StageOut<'_>) + Send + Sync>;

/// The catch-up fixture: the rewrite `t.rw` (appends `+`; fails on `bad`;
/// refuses `deep` as a nesting refusal; notes the layout it was given), the
/// `PerTree` stage `t.mark` (requires `t.rewritten`; writes `t.marks`: node
/// address → text, and notes how many nodes the walk shows), the `PerTree`
/// stage `t.other` (writes `t.other`, read by nobody), the `Document` stage
/// `t.docfact` (writes `t.docs`), the `PerTree` stage `t.fact` (writes the
/// document-fact table `t.fact`), and the `Document` consumer `t.consume`
/// (reads `t.marks` and `t.docs`, plus `t.fact` when `reads_fact`; writes
/// `t.log`) whose body the test supplies. `seen` counts the runs of the
/// stages other than the consumer.
fn catchup_registry(seen: Probe, reads_fact: bool, consumer: Consumer) -> Arc<FlmRegistry> {
    registry(vec![feature("t", move |b| {
        b.rewrite_stage(rewrite(
            RewriteDecl::new("t.rw", "t.rewritten"),
            Needs::new(),
            |_, tree, cx, diags| {
                let text = text_of(tree);
                if text.contains("bad") {
                    return Err(RewriteError::failed(Note { detail: "bad".into() }, span()));
                }
                if text.contains("deep") {
                    let refusal =
                        Diagnostic::error(NestingTooDeep { detail: "too deep".into() }, span());
                    return Err(RewriteError::NestingTooDeep(refusal));
                }
                diags.push(Diagnostic::note(
                    Note { detail: format!("layout {:?}", cx.layout()) },
                    span(),
                ));
                Ok(reparse(cx.registry(), &format!("{text}+"), cx))
            },
        ))?;
        b.table(MARKS, "node address → text")?;
        b.table(OTHER, "read by nobody")?;
        b.table(DOCS, "a document-scope log")?;
        b.table(FACT, "a document fact per tree")?;
        b.table(LOG, "what the consumer saw")?;
        let mark_seen = Arc::clone(&seen);
        b.stage(stage(
            StageDecl::new("t.mark", Effect::Compute, Scope::PerTree).writes(MARKS),
            Needs::new().requires("t.rewritten"),
            move |cx, out| {
                record(&mark_seen, format!("mark {}", cx.tree_id().unwrap()));
                let tree = cx.my_tree().expect("a per-tree run has a tree");
                let marks = out.table(MARKS).expect("declared");
                for node in tree.root().descendants() {
                    if let Some(chars) = node.chars() {
                        marks.0.insert(
                            cx.addr(&node).expect("a node of this tree"),
                            chars.to_string(),
                        );
                    }
                }
                let walked = cx.walk().count();
                out.diags.push(Diagnostic::note(Note { detail: format!("walk {walked}") }, span()));
            },
        ))?;
        let other_seen = Arc::clone(&seen);
        b.stage(stage(
            StageDecl::new("t.other", Effect::Compute, Scope::PerTree).writes(OTHER),
            Needs::new(),
            move |cx, _| record(&other_seen, format!("other {}", cx.tree_id().unwrap())),
        ))?;
        let doc_seen = Arc::clone(&seen);
        b.stage(stage(
            StageDecl::new("t.docfact", Effect::Compute, Scope::Document).writes(DOCS),
            Needs::new(),
            move |_, out| {
                record(&doc_seen, "docfact");
                out.table(DOCS).unwrap().0.push("document".into());
            },
        ))?;
        b.stage(stage(
            StageDecl::new("t.fact", Effect::Compute, Scope::PerTree).writes(FACT),
            Needs::new(),
            |_, out| out.table(FACT).unwrap().0 += 1,
        ))?;
        let consumer = Arc::clone(&consumer);
        let mut needs = Needs::new().reads(MARKS).reads(DOCS);
        if reads_fact {
            needs = needs.reads(FACT);
        }
        b.stage(stage(
            StageDecl::new("t.consume", Effect::Compute, Scope::Document).writes(LOG),
            needs,
            move |cx, out| consumer(cx, out),
        ))
    })])
}

/// Runs the fixture over a one-tree document with the given consumer body and
/// returns the consumer's log and the probe's record.
fn consume(reads_fact: bool, consumer: Consumer) -> (Vec<String>, Vec<String>) {
    let seen = probe();
    let registry = catchup_registry(Arc::clone(&seen), reads_fact, consumer);
    let mut demands = vec!["t.log", "t.other"];
    if reads_fact {
        demands.push("t.fact");
    }
    let run = run(&doc(&registry, &["doc"]), &plan(&registry, &demands), &RunOptions::sequential());
    let log: &Log = run.table(LOG).unwrap().expect("written");
    (log.0.clone(), recorded(&seen))
}

fn marks_of(marks: &Marks) -> Vec<(TreeId, String)> {
    marks.0.iter().map(|(addr, text)| (addr.tree, text.clone())).collect()
}

#[test]
fn a_value_is_rewritten_by_the_active_rewrites() {
    let (log, _) = consume(
        false,
        Arc::new(|cx, out| {
            let value = tree(cx.registry(), "v");
            let caught = cx.resolve_value(&value).expect("caught up");
            out.table(LOG).unwrap().0.push(text_of(&caught.tree));
        }),
    );
    assert_eq!(log, ["v+"]);
}

#[test]
fn the_per_tree_closure_re_enters_over_the_value_alone() {
    let (log, seen) = consume(
        false,
        Arc::new(|cx, out| {
            let caught = cx.resolve_value(&tree(cx.registry(), "v")).expect("caught up");
            let scratch =
                caught.scratch.get_as::<Marks>(cx.registry().table_id("t.marks").unwrap()).unwrap();
            out.table(LOG).unwrap().0.push(format!("{:?}", marks_of(scratch)));
            // The main store's table holds the document's entries only.
            let main = cx.get(MARKS).unwrap().unwrap();
            out.table(LOG).unwrap().0.push(format!("{:?}", marks_of(main)));
        }),
    );
    assert_eq!(
        log,
        [
            format!("{:?}", [(TreeId::value(0), "v+".to_string())]),
            format!("{:?}", [(TreeId::tree(0), "doc+".to_string())]),
        ]
    );
    // `t.mark` ran for the document tree, then once more over the value;
    // `t.other` (outside the closure) and `t.docfact` (`Document` scope) ran
    // for the document only.
    assert_eq!(seen, ["mark tree#0", "other tree#0", "docfact", "mark value#0"]);
}

#[test]
fn the_overlay_reads_the_values_entries_before_the_main_store() {
    let (log, _) = consume(
        false,
        Arc::new(|cx, out| {
            let caught = cx.resolve_value(&tree(cx.registry(), "v")).expect("caught up");
            let overlay = caught.view(cx).get(MARKS).unwrap().unwrap();
            out.table(LOG).unwrap().0.push(format!("{:?}", marks_of(overlay)));
        }),
    );
    assert_eq!(log, [format!("{:?}", [(TreeId::value(0), "v+".to_string())])]);
}

#[test]
fn values_are_numbered_from_zero_per_invocation_and_addressable() {
    let (log, _) = consume(
        false,
        Arc::new(|cx, out| {
            let first = cx.resolve_value(&tree(cx.registry(), "a")).expect("caught up");
            let second = cx.resolve_value(&tree(cx.registry(), "b")).expect("caught up");
            let log = out.table(LOG).unwrap();
            log.0.push(format!("{} {}", first.id, second.id));
            assert_eq!((first.id, second.id), (TreeId::value(0), TreeId::value(1)));
            // The value's tree is reachable by id and its nodes have addresses
            // in the value's id.
            let held = cx.tree(first.id).expect("held by the context");
            assert!(Arc::ptr_eq(&held, &first.tree));
            let addr = cx.addr(&held.root()).expect("a node of a caught-up value");
            log.0.push(addr.tree.to_string());
            assert!(cx.tree(TreeId::value(7)).is_none());
            // A node of a tree the run never saw has no address.
            let stray = tree(cx.registry(), "stray");
            assert!(cx.addr(&stray.root()).is_none());
        }),
    );
    assert_eq!(
        log,
        [format!("{} {}", TreeId::value(0), TreeId::value(1)), TreeId::value(0).to_string(),]
    );
}

#[test]
fn a_document_fact_piece_from_a_re_entered_stage_is_a_typed_error() {
    let (log, _) = consume(
        true,
        Arc::new(|cx, out| {
            let error = cx.resolve_value(&tree(cx.registry(), "v")).expect_err("refused");
            match error {
                CatchUpError::Store(StoreError::DocumentFactInValue { stage, table }) => {
                    out.table(LOG).unwrap().0.push(format!("{stage} {table}"));
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }),
    );
    assert_eq!(log, ["t.fact t.fact"]);
}

#[test]
fn a_rewrite_failing_over_the_value_is_reported_with_its_stage() {
    let (log, _) = consume(
        false,
        Arc::new(|cx, out| {
            match cx.resolve_value(&tree(cx.registry(), "bad")).expect_err("refused") {
                CatchUpError::Rewrite { stage, diagnostic } => {
                    out.table(LOG).unwrap().0.push(format!("{stage} {}", diagnostic.identifier()));
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }),
    );
    assert_eq!(log, ["t.rw t.note"]);
}

#[test]
fn a_nesting_refusal_is_re_reported_as_synth_nesting_too_deep() {
    let (log, _) = consume(
        false,
        Arc::new(|cx, out| {
            match cx.resolve_value(&tree(cx.registry(), "deep")).expect_err("refused") {
                CatchUpError::NestingTooDeep(diagnostic) => {
                    out.table(LOG).unwrap().0.push(diagnostic.identifier().to_string());
                }
                other => panic!("unexpected error: {other:?}"),
            }
        }),
    );
    assert_eq!(log, ["flm.synth.nesting-too-deep"]);
}

#[test]
fn a_missing_entry_condition_names_table_producer_and_element() {
    let (log, _) = consume(
        false,
        Arc::new(|cx, out| {
            let diagnostic =
                cx.missing_entry(&CapName::from("t.marks"), &ElementName::from("t.foo"), span());
            assert_eq!(diagnostic.identifier(), "flm.run.missing-entry");
            let mut sink = flm_core::lang::FlmDiagnostics::new();
            sink.push(diagnostic);
            let condition = sink.conditions::<MissingEntry>().next().expect("the condition");
            out.table(LOG)
                .unwrap()
                .0
                .push(format!("{} {} {}", condition.table, condition.producer, condition.element));
            // A table with no scheduled writer names nobody.
            let unwritten =
                cx.missing_entry(&CapName::from("t.docs"), &ElementName::from("t.foo"), span());
            sink.push(unwritten);
            let producers: Vec<String> =
                sink.conditions::<MissingEntry>().map(|c| c.producer.clone()).collect();
            out.table(LOG).unwrap().0.push(producers.join(","));
        }),
    );
    assert_eq!(log, ["t.marks t.mark t.foo", "t.mark,t.docfact"]);
}

#[test]
fn catch_up_diagnostics_are_returned_to_the_consumer_and_stay_out_of_the_run() {
    let seen = probe();
    let registry = catchup_registry(
        seen,
        false,
        Arc::new(|cx, out| {
            let caught = cx.resolve_value(&tree(cx.registry(), "v")).expect("caught up");
            let notes: Vec<String> =
                caught.diags.conditions::<Note>().map(|note| note.detail.clone()).collect();
            out.table(LOG).unwrap().0.extend(notes);
        }),
    );
    let run =
        run(&doc(&registry, &["doc"]), &plan(&registry, &["t.log", "t.other"]), &RunOptions::new());
    let log: &Log = run.table(LOG).unwrap().unwrap();
    // The rewrite's note (the default layout), then the re-entered stage's
    // note: the walk it saw is the document's, not the value's.
    assert_eq!(log.0, ["layout InlineOrBlocks", "walk 2"]);
    // Nothing of the catch-up reached the run's own sink beyond the document
    // runs' notes.
    let run_notes: Vec<String> =
        run.pipeline_diagnostics().conditions::<Note>().map(|note| note.detail.clone()).collect();
    assert_eq!(run_notes, ["layout InlineOrBlocks", "walk 2"]);
}

#[test]
fn a_layout_given_at_catch_up_reaches_the_rewrites() {
    let (log, _) = consume(
        false,
        Arc::new(|cx, out| {
            let caught =
                cx.resolve_value_with(&tree(cx.registry(), "v"), FragmentLayout::Blocks).unwrap();
            let notes: Vec<String> =
                caught.diags.conditions::<Note>().map(|n| n.detail.clone()).collect();
            out.table(LOG).unwrap().0.push(notes[0].clone());
        }),
    );
    assert_eq!(log, ["layout Blocks"]);
}
