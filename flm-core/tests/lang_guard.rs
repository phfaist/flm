//! The descent guard of `flm_core::lang::guard`: a refusal is a typed condition
//! and no tree, never a panic; the parse entry points enforce the worker-stack
//! contract; and the guard helpers bound every traversal.
//!
//! Where the stack budget starts refusing, measured on a 64 MiB worker thread
//! with the 48 MiB budget: nesting depth about 2 260 in a debug build and about
//! 13 800 in a release build (roughly 3.5 KB of stack per nesting level in
//! release, several times that in debug). The depths below stay far on either
//! side of both figures, so no test depends on the profile.

mod common;

use std::sync::{Arc, Mutex};
use std::thread;

use flm_core::model::doc::FragmentLayout;
use flm_core::lang::guard::{self, is_worker_thread, run_with_worker_stack};
use flm_core::lang::{
    parse_fragment, Flm, FragmentError, GuardPolicy, LangInputs, NestingTooDeep, ParseOptions,
};
use techy::core::node::NodeRef;
use techy::error::DiagnosticInfo;
use techy::latexlike::Latexlike;
use techy::recompose::{ConcatPieces, Recompose, RecomposeContext, RecomposeError, Recomposer};
use techy::serialize::{
    DeserializeContext, DeserializeError, ObjectSerdeDriver, SerdeSession, SerialEntry,
    SerialIndex, SerialValue, SerializeContext, SerializeError, TableHandle,
};
use techy::transform::{Restage, RestageContext, RestageError};
use techy::visit::{VisitContext, VisitFlow, WalkError};

use common::{build, lang, parse, source};

fn nested(depth: usize) -> String {
    format!("{}x{}", "{".repeat(depth), "}".repeat(depth))
}

// --- the parse under the guard --------------------------------------------------------------

#[test]
fn the_depth_limit_refuses_with_the_typed_condition_and_no_tree() {
    let mut inputs = LangInputs::new();
    inputs.guard = GuardPolicy::DepthLimit(3);
    let lang = build(inputs);
    let result = parse_fragment(
        &lang,
        source(&nested(8)),
        FragmentLayout::default(),
        &ParseOptions::default(),
    );
    match result {
        Err(FragmentError::NestingTooDeep(diagnostic)) => {
            assert_eq!(diagnostic.identifier(), NestingTooDeep::IDENTIFIER);
            assert!(diagnostic.data().downcast_ref::<NestingTooDeep>().is_some());
        }
        other => panic!("expected the guard's refusal, got {other:?}"),
    }
}

#[test]
fn shallow_input_parses_under_the_depth_limit() {
    let mut inputs = LangInputs::new();
    inputs.guard = GuardPolicy::DepthLimit(8);
    let lang = build(inputs);
    let fragment = parse(&lang, &nested(2));
    assert!(fragment.diags().is_empty());
}

#[test]
fn deeply_nested_input_parses_under_the_stack_budget() {
    // 1 000 levels: below the debug refusal depth (see the module docs).
    let depth = 1000;
    let text = nested(depth);
    let lang = lang(Vec::new());
    let count = run_with_worker_stack(|| parse(&lang, &text).tree().node_count()).unwrap();
    assert!(count > depth);
}

#[test]
fn absurd_nesting_is_refused_under_the_stack_budget_without_a_crash() {
    // 400 000 levels: about 29 times the release refusal depth and 180 times the
    // debug one (see the module docs) — not a tuned number, a margin.
    let text = nested(400_000);
    let lang = lang(Vec::new());
    let refused = run_with_worker_stack(|| {
        let result = parse_fragment(
            &lang,
            source(&text),
            FragmentLayout::default(),
            &ParseOptions::default(),
        );
        matches!(result, Err(FragmentError::NestingTooDeep(_)))
    })
    .unwrap();
    assert!(refused);
}

#[test]
fn the_guard_policy_is_reported_by_the_built_language() {
    let mut inputs = LangInputs::new();
    inputs.guard = GuardPolicy::DepthLimit(5);
    assert_eq!(build(inputs).guard(), GuardPolicy::DepthLimit(5));
    assert_eq!(build(LangInputs::new()).guard(), GuardPolicy::StackBudget);
}

// --- the worker-stack contract -----------------------------------------------------------

#[test]
fn a_deep_parse_on_a_default_stack_thread_is_never_an_abort() {
    // 5 000 levels on a thread with Rust's default 2 MiB stack: above the debug
    // refusal depth and below the release one, so the outcome differs by profile
    // — a tree or a refusal — while the absence of a crash does not.
    let lang = lang(Vec::new());
    let text = nested(5_000);
    let outcome = thread::spawn(move || {
        let result = parse_fragment(
            &lang,
            source(&text),
            FragmentLayout::default(),
            &ParseOptions::default(),
        );
        matches!(result, Ok(_) | Err(FragmentError::NestingTooDeep(_)))
    })
    .join()
    .unwrap();
    assert!(outcome);
}

#[test]
fn too_deep_input_on_a_default_stack_thread_is_refused() {
    let lang = lang(Vec::new());
    let text = nested(400_000);
    let refused = thread::spawn(move || {
        let result = parse_fragment(
            &lang,
            source(&text),
            FragmentLayout::default(),
            &ParseOptions::default(),
        );
        matches!(result, Err(FragmentError::NestingTooDeep(_)))
    })
    .join()
    .unwrap();
    assert!(refused);
}

#[test]
fn the_parse_entry_point_leaves_the_calling_thread_unmarked() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, "a");
    assert_eq!(fragment.tree().node_count(), 2);
    assert!(!is_worker_thread());
}

// --- the traversal helpers -------------------------------------------------------------------

#[test]
fn the_walker_helper_refuses_under_the_depth_limit() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, &nested(8));
    let mut visitor = |_node: NodeRef<'_, Flm>, _cx: &VisitContext<'_, Flm>| VisitFlow::Descend;
    let result =
        guard::walker(&mut visitor, GuardPolicy::DepthLimit(3)).walk(fragment.tree().root());
    assert!(matches!(result, Err(WalkError::DescentLimitExceeded { .. })));
}

#[test]
fn the_walker_helper_walks_a_shallow_tree() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, "a{b}");
    let mut visited = 0usize;
    let mut visitor = |_node: NodeRef<'_, Flm>, _cx: &VisitContext<'_, Flm>| {
        visited += 1;
        VisitFlow::Descend
    };
    guard::walker(&mut visitor, GuardPolicy::DepthLimit(8)).walk(fragment.tree().root()).unwrap();
    assert_eq!(visited, fragment.tree().node_count());
}

#[test]
fn the_restager_helper_refuses_under_the_depth_limit() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, &nested(8));
    let mut visitor = |_node: NodeRef<'_, Flm>, _cx: &mut RestageContext<'_, Flm, (), ()>| {
        Ok::<Restage<()>, ()>(Restage::Descend(()))
    };
    let result = guard::restager(&mut visitor, GuardPolicy::DepthLimit(3)).restage(fragment.tree());
    assert!(matches!(result, Err(RestageError::DescentLimitExceeded { .. })));
}

#[test]
fn the_restager_helper_restages_a_shallow_tree() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, "a{b}");
    let mut visitor = |_node: NodeRef<'_, Flm>, _cx: &mut RestageContext<'_, Flm, (), ()>| {
        Ok::<Restage<()>, ()>(Restage::Descend(()))
    };
    let tree = guard::restager(&mut visitor, GuardPolicy::DepthLimit(8))
        .restage(fragment.tree())
        .unwrap();
    assert_eq!(tree.node_count(), fragment.tree().node_count());
}

/// A recomposer flattening every node to its text.
struct Flatten;

impl Recomposer<Flm, ()> for Flatten {
    type State = ();
    type Piece = String;
    type Error = ();

    fn recompose_node(
        &mut self,
        node: NodeRef<'_, Flm>,
        _state: &(),
        _cx: &mut RecomposeContext<'_, Flm>,
    ) -> Result<Recompose<String, ()>, ()> {
        Ok(match node.chars() {
            Some(text) => Recompose::Emit(text.to_string()),
            None => Recompose::Concat(ConcatPieces::children()),
        })
    }
}

#[test]
fn the_recomposer_helper_refuses_under_the_depth_limit() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, &nested(8));
    let mut flatten = Flatten;
    let result =
        guard::recomposer(&mut flatten, GuardPolicy::DepthLimit(3)).recompose(fragment.tree(), ());
    assert!(matches!(result, Err(RecomposeError::DescentLimitExceeded { .. })));
}

#[test]
fn the_recomposer_helper_composes_a_shallow_tree() {
    let lang = lang(Vec::new());
    let fragment = parse(&lang, "a{b}c");
    let mut flatten = Flatten;
    let text =
        guard::recomposer(&mut flatten, GuardPolicy::DepthLimit(8)).recompose(fragment.tree(), ());
    assert_eq!(text.unwrap(), "abc");
}

// --- the serialization session helper ---------------------------------------------------

/// A chain of objects, each referring to the next: interning one interns the
/// whole chain, one descent per link.
struct Ring {
    next: Option<Arc<Ring>>,
}

techy::serial_index! {
    struct RingIndex;
}

/// The rings table's driver, holding its own handle so it can intern the next
/// ring (set right after registration).
struct RingDriver {
    handle: Arc<Mutex<Option<TableHandle<RingDriver>>>>,
}

impl ObjectSerdeDriver<Latexlike> for RingDriver {
    type Object = Ring;
    type Index = RingIndex;

    fn table_name(&self) -> &'static str {
        "rings"
    }

    fn homogeneous_identifier(&self) -> Option<&'static str> {
        Some("test.ring")
    }

    fn serialize_object(
        &self,
        ring: &Arc<Ring>,
        cx: &mut SerializeContext<'_, Latexlike>,
    ) -> Result<SerialEntry, SerializeError> {
        let handle = self.handle.lock().unwrap().expect("the handle is set after registration");
        let data = match &ring.next {
            Some(next) => {
                let index = cx.intern(handle, next)?;
                SerialValue::Index { table: index.table(), index: index.index() }
            }
            None => SerialValue::Null,
        };
        Ok(SerialEntry { identifier: "test.ring".into(), data })
    }

    fn deserialize_object(
        &self,
        _entry: &SerialEntry,
        _cx: &mut DeserializeContext<'_, Latexlike>,
    ) -> Result<Arc<Ring>, DeserializeError> {
        Err(DeserializeError::failed("rings are written only"))
    }
}

fn ring_chain(length: usize) -> Arc<Ring> {
    let mut head = Arc::new(Ring { next: None });
    for _ in 0..length {
        head = Arc::new(Ring { next: Some(head) });
    }
    head
}

fn ring_session(policy: GuardPolicy) -> (SerdeSession<Latexlike>, TableHandle<RingDriver>) {
    let mut session = guard::serde_session(SerdeSession::<Latexlike>::empty(), policy);
    let shared = Arc::new(Mutex::new(None));
    let handle = session.register_table(RingDriver { handle: Arc::clone(&shared) }).unwrap();
    *shared.lock().unwrap() = Some(handle);
    (session, handle)
}

/// Whether the descent guard's refusal is the cause of `error` (the session
/// wraps a driver's failure per table).
fn descent_refused(error: &SerializeError) -> bool {
    let mut current = error;
    loop {
        match current {
            SerializeError::DescentLimitExceeded { .. } => return true,
            SerializeError::InTable { cause, .. } => current = &**cause,
            _ => return false,
        }
    }
}

#[test]
fn the_serde_session_helper_refuses_under_the_depth_limit() {
    let (mut session, handle) = ring_session(GuardPolicy::DepthLimit(4));
    let error = session.intern(handle, &ring_chain(20)).unwrap_err();
    assert!(descent_refused(&error), "{error:?}");
}

#[test]
fn the_serde_session_helper_interns_a_short_chain() {
    let (mut session, handle) = ring_session(GuardPolicy::DepthLimit(8));
    session.intern(handle, &ring_chain(3)).unwrap();
}
