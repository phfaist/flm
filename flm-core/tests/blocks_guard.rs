//! The block-structure pass under the descent guard and the worker-stack
//! contract (`flm_core::lang::rewrites::blocks`, `flm_core::lang::guard`):
//! content nested too deeply yields the typed condition and no tree, never a
//! crash.

mod common;

use std::thread;

use flm_core::model::doc::FragmentLayout;
use flm_core::lang::guard::{is_worker_thread, run_with_worker_stack};
use flm_core::lang::rewrites::blocks::{
    BlocksConfig, StructureError, structure, structure_with_guard,
};
use flm_core::lang::{
    FlmDiagnostics, FragmentError, GuardPolicy, NestingTooDeep, ParseOptions, parse_fragment,
};
use techy::error::DiagnosticInfo;

use common::blocks::toy_lang;
use common::source;

fn nested(depth: usize) -> String {
    format!("{}x{}", "{".repeat(depth), "}".repeat(depth))
}

#[test]
fn the_depth_limit_refuses_with_the_typed_condition_and_no_tree() {
    let lang = toy_lang();
    let fragment = common::parse(&lang, &nested(8));
    let mut diags = FlmDiagnostics::new();
    let result = structure_with_guard(
        fragment.tree(),
        FragmentLayout::Blocks,
        &BlocksConfig::default(),
        GuardPolicy::DepthLimit(3),
        &mut diags,
    );
    match result {
        Err(error @ StructureError::NestingTooDeep { .. }) => {
            let diagnostic = error.diagnostic(fragment.tree());
            assert_eq!(diagnostic.identifier(), NestingTooDeep::IDENTIFIER);
            assert!(diagnostic.data().downcast_ref::<NestingTooDeep>().is_some());
        }
        other => panic!("expected the guard's refusal, got {other:?}"),
    }
}

#[test]
fn a_shallow_tree_structures_under_the_depth_limit() {
    let lang = toy_lang();
    let fragment = common::parse(&lang, &nested(2));
    let mut diags = FlmDiagnostics::new();
    let out = structure_with_guard(
        fragment.tree(),
        FragmentLayout::Blocks,
        &BlocksConfig::default(),
        GuardPolicy::DepthLimit(16),
        &mut diags,
    )
    .unwrap();
    assert!(out.node_count() >= fragment.tree().node_count());
}

#[test]
fn a_deeply_nested_tree_structures_under_the_stack_budget() {
    // 1 000 levels: below the parse's debug refusal depth (see `lang_guard`).
    let depth = 1000;
    let lang = toy_lang();
    let text = nested(depth);
    let count = run_with_worker_stack(|| {
        let fragment = common::parse(&lang, &text);
        let mut diags = FlmDiagnostics::new();
        let out = structure(
            fragment.tree(),
            FragmentLayout::Blocks,
            &BlocksConfig::default(),
            &mut diags,
        )
        .unwrap();
        out.node_count()
    })
    .unwrap();
    assert!(count > depth);
}

#[test]
fn a_deep_pass_on_a_default_stack_thread_is_never_an_abort() {
    // 5 000 levels on a thread with Rust's default 2 MiB stack: above the parse's
    // debug refusal depth and below its release one, so the parse itself may be
    // refused; whatever parses is structured, and the outcome is a tree or a
    // refusal — never a crash.
    let lang = toy_lang();
    let text = nested(5_000);
    let outcome = thread::spawn(move || {
        let parsed = parse_fragment(
            &lang,
            source(&text),
            FragmentLayout::default(),
            &ParseOptions::default(),
        );
        let fragment = match parsed {
            Ok(fragment) => fragment,
            Err(FragmentError::NestingTooDeep(_)) => return true,
            Err(other) => panic!("unexpected parse failure: {other}"),
        };
        let mut diags = FlmDiagnostics::new();
        let result = structure(
            fragment.tree(),
            FragmentLayout::Blocks,
            &BlocksConfig::default(),
            &mut diags,
        );
        matches!(result, Ok(_) | Err(StructureError::NestingTooDeep { .. }))
    })
    .join()
    .unwrap();
    assert!(outcome);
}

#[test]
fn a_thousand_levels_structure_from_a_default_stack_thread() {
    // 1 000 levels parse in both profiles; from a 2 MiB thread the pass runs on
    // a worker thread it spawns itself, and yields the tree.
    let lang = toy_lang();
    let text = nested(1_000);
    let count = thread::spawn(move || {
        let fragment = common::parse(&lang, &text);
        let mut diags = FlmDiagnostics::new();
        structure(
            fragment.tree(),
            FragmentLayout::Blocks,
            &BlocksConfig::default(),
            &mut diags,
        )
        .unwrap()
        .node_count()
    })
    .join()
    .unwrap();
    assert!(count > 1_000);
}

#[test]
fn the_pass_leaves_the_calling_thread_unmarked() {
    let lang = toy_lang();
    let fragment = common::parse(&lang, "a");
    let mut diags = FlmDiagnostics::new();
    structure(
        fragment.tree(),
        FragmentLayout::Blocks,
        &BlocksConfig::default(),
        &mut diags,
    )
    .unwrap();
    assert!(!is_worker_thread());
}

#[test]
fn the_pass_types_are_send_and_sync() {
    fn assert_send_sync<T: Send + Sync>() {}
    assert_send_sync::<BlocksConfig>();
    assert_send_sync::<StructureError>();
}
