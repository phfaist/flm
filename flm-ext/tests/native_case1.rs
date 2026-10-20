//! Case 1 compiled into the program: the extension `wordcount` of
//! `tests/wasm/wordcount/` and the failing guest of `tests/wasm/failing/`,
//! whose sources (`src/lib.rs`, written against `flm_ext::guest`) this test
//! program includes as modules, loaded through the engine `native`. It runs the
//! assertions of `tests/case1/`, which `tests/wasm_case1.rs` runs on the same
//! crates built for WebAssembly: the JavaScript case's goldens, the settings, the
//! hook's recovery, the rehearsal on a worker thread, the hook answering an
//! error. Here also the failure of the hook that panics, as the engine `native`
//! words it. The WebAssembly case's limit tests (the stage that loops, the stage
//! that grows its memory) have no counterpart: the engine `native` sets no limit.

#![cfg(feature = "native")]

mod case1;
mod common;

#[path = "wasm/failing/src/lib.rs"]
mod failing;
#[path = "wasm/wordcount/src/lib.rs"]
mod wordcount;

use std::sync::Arc;

use flm_core::core::registry::PackageFactory;
use flm_ext::native::{NativeEngine, NativeExtension};
use flm_ext::{ExtensionLoader, MemoryFiles, MemorySource};
use serde_json::json;

use case1::{Loads, PANIC_PREFIX};

/// The two extensions compiled into this test program, each loaded by a loader of its
/// own whose source answers its stem with the manifest `engine: native`.
struct NativeCase;

impl Loads for NativeCase {
    fn factory(&self, stem: &str) -> PackageFactory {
        let held = [
            NativeExtension { stem: "wordcount", manifest: "", compiled: &wordcount::FLM_EXTENSION },
            NativeExtension { stem: "failing", manifest: "", compiled: &failing::FLM_EXTENSION },
        ];
        let engine = NativeEngine::new(held).with_console(Arc::new(|_, _| {}));
        let manifest = json!({ "extension": { "engine": "native" } });
        let source = MemorySource::new().with(stem, stem, manifest, MemoryFiles::new());
        let loader = Arc::new(ExtensionLoader::new(vec![Arc::new(source)]).with_engine(Arc::new(engine)));
        common::factory_of(&loader, stem)
    }
}

#[test]
fn case_one_renders_through_the_text_workflow() {
    case1::case_one_renders_through_the_text_workflow(&NativeCase);
}

#[test]
fn case_one_renders_through_the_html_workflow() {
    case1::case_one_renders_through_the_html_workflow(&NativeCase);
}

#[test]
fn case_one_s_tables_project_as_under_javascript() {
    case1::case_one_s_tables_project_as_under_javascript(&NativeCase);
}

#[test]
fn case_one_s_registry_builds_without_a_diagnostic() {
    case1::case_one_s_registry_builds_without_a_diagnostic(&NativeCase);
}

#[test]
fn a_shout_of_a_command_recovers_the_use_site_with_the_guest_s_diagnostic() {
    case1::a_shout_of_a_command_recovers_the_use_site_with_the_guest_s_diagnostic(&NativeCase);
}

#[test]
fn the_rehearsal_declares_the_same_objects() {
    case1::the_rehearsal_declares_the_same_objects(&NativeCase);
}

#[test]
fn register_receives_the_configuration_and_a_hook_on_a_worker_sees_the_same() {
    case1::register_receives_the_configuration_and_a_hook_on_a_worker_sees_the_same(&NativeCase);
}

#[test]
fn the_count_stage_reads_both_options() {
    case1::the_count_stage_reads_both_options(&NativeCase);
}

#[test]
fn the_count_s_note_appears_under_the_plain_workflows() {
    case1::the_count_s_note_appears_under_the_plain_workflows(&NativeCase);
}

#[test]
fn a_hook_answering_an_error_fails_with_the_guest_s_error() {
    case1::a_hook_answering_an_error_fails_with_the_guest_s_error(&NativeCase);
}

/// The panic unwinds to the engine, which fails the hook's call with the panic's message
/// (no trap natively, and no place: the engine installs no panic hook).
#[test]
fn a_hook_that_panics_fails_with_the_panic_s_message() {
    let (diagnostics, data) = case1::the_panicking_hook_s_failure(&NativeCase);
    assert_eq!(diagnostics, [format!("{PANIC_PREFIX}panic: the hook is not ready either")]);
    assert_eq!(data, json!({ "panic": "the hook is not ready either" }));
}
