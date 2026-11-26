//! The engine `native`: extensions written against the guest client
//! `flm_ext::guest` and compiled into this test program, run through the
//! bindings' shared host side in the same process. The stub of
//! `tests/native/stub/` (a standalone crate whose `src/lib.rs` is included here
//! as a module) and the second extension of `tests/native/second.rs`: a stage
//! reading an option, calling a host service and pushing items; two extensions
//! in one registry run on several threads; a rule of one extension lowering an
//! element of the other (a nested entry); a request sent by a `define` run
//! inside another extension's request; a request sent from a thread an extension
//! started; a panicking rule; a request sent by the
//! `Drop` of a rule's body, its instance discarded inside another extension's
//! request, and at the thread's end; the loader's rules: a stem the engine does
//! not hold, the identity, the digest, one load for two `use` entries; the
//! writer of a formula's TeX called through the guest client, its parts and
//! holes the library's. Gated on `FLM_TESTS_WASM_GUEST=1` (`common::guest`),
//! the stub built for `wasm32-unknown-unknown` and loaded by the engine `wasm`
//! gives the same items as the native run.
//!
//! Run also under `--features flm-core/parallel`, where a run's per-tree stages
//! go to the runner's pool.

#![cfg(feature = "native")]

mod common;

#[path = "native/second.rs"]
mod second;
#[path = "native/stub/src/lib.rs"]
mod stub;

use std::sync::{Arc, Mutex};

use flm_core::core::lowering::DefaultMark;
use flm_core::core::plan::{Overrides, Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageFactory};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::options::OptionValues;
use flm_core::model::element::element_of;
use flm_core::stdlib::backends::text;
use flm_core::stdlib::features::math::{lines as core_lines, tex as core_tex, ENVIRONMENT};
use flm_ext::guest::{CompiledExtension, NamedExtension};
use flm_ext::host::{HostAnswer, HostCaller, HostService, HostServiceError};
use flm_ext::native::NativeEngine;
use flm_ext::{Engine, ExtensionIdentity, ExtensionLoader, Manifest, MemoryFiles, MemorySource};
use serde_json::{json, Value};

use common::{ext_items, registry_with};

// --- helpers -----------------------------------------------------------------------------------

/// The table of the stub's stage.
const TALLY: &str = "stub.tally";

/// The table of the stub's TeX of the formulas.
const TEX: &str = "stub.tex";

/// Formulas holding text commands, at the top and inside a group, a comment, and an
/// environment of three lines, one labeled, one tagged, whose line ends the newline
/// rule reduces.
const FORMULAS: [&str; 2] = [
    "Let \\(x + \\text{if } y\\) and \\(\\alpha%%c\nb\\), \\[{\\mbox{all} z}\\].",
    "\\begin{align}\n  a \\textbf{b} \\label{eq:a}\n  \\\\\n  c \\tag{T}\n  \\\\ d\n\\end{align}",
];

/// The table of code of the extensions this test program compiles in.
static EXTENSIONS: &[NamedExtension] = &[
    NamedExtension { stem: "stub", compiled: &stub::FLM_EXTENSION },
    NamedExtension { stem: "second", compiled: &second::FLM_EXTENSION },
];

/// A source answering each of `stems` with the manifest `engine: native` and no file.
fn source(stems: &[&str]) -> MemorySource {
    let manifest = json!({ "extension": { "engine": "native" } });
    stems.iter().fold(MemorySource::new(), |source, stem| source.with(stem, stem, manifest.clone(), MemoryFiles::new()))
}

/// The host service `probe`: records each call (the caller, the method, the arguments) and
/// answers twice the `words` it is given.
#[derive(Default)]
struct Probe {
    calls: Mutex<Vec<Value>>,
}

impl Probe {
    /// The calls received, sorted by their text (the per-tree stages run in any order).
    fn calls(&self) -> Vec<Value> {
        let mut calls = self.calls.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).clone();
        calls.sort_by_key(Value::to_string);
        calls
    }
}

impl HostService for Probe {
    fn name(&self) -> &str {
        "probe"
    }

    fn call(&self, caller: &HostCaller, method: &str, args: &Value, _: &flm_core::progress::Scope) -> Result<HostAnswer, HostServiceError> {
        let call = json!({ "extension": caller.extension.stem, "package": caller.package, "method": method, "args": args });
        self.calls.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).push(call);
        Ok(HostAnswer::Data(json!({ "twice": args["words"].as_u64().unwrap_or(0) * 2 })))
    }
}

/// A loader over `source` with the engine `native` holding [`EXTENSIONS`], offering `probe`.
fn loader(probe: &Arc<Probe>, source: MemorySource) -> Arc<ExtensionLoader> {
    let engine = NativeEngine::new(EXTENSIONS.iter().copied());
    Arc::new(
        ExtensionLoader::new(vec![Arc::new(source)])
            .with_engine(Arc::new(engine))
            .with_host_service(Arc::clone(probe) as Arc<dyn HostService>),
    )
}

/// The registry of the standard package and the packages `_` of `stems`, loaded by `loader`.
fn registry(loader: &Arc<ExtensionLoader>, stems: &[&str]) -> Arc<FlmRegistry> {
    registry_with(stems.iter().map(|stem| {
        common::factory_of(loader, stem).build(&PackageSettings::new()).expect("the package builds")
    }))
}

/// The render options setting the stub's tag to `tag`, read with the feature's declaration.
fn tagged(factory: &PackageFactory, tag: &str) -> OptionValues {
    let decl = factory.settings("stub").expect("the feature declares settings");
    let Value::Object(map) = json!({ "tag": tag }) else { unreachable!() };
    decl.read(&map).expect("the settings read").render_options().clone()
}

/// The document of one tree per text of `texts`.
fn doc(registry: &Arc<FlmRegistry>, texts: &[&str]) -> Arc<Doc> {
    let mut content = DocContent::new();
    for text in texts {
        let source = Arc::new(FlmSource::new(*text).with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .expect("the tree parses");
        content.place(Arc::new(fragment));
    }
    Arc::new(Doc::new(content))
}

/// The run of the plan `request` asks for, over a document of one tree per text of `texts`,
/// with `options`.
fn run(registry: &Arc<FlmRegistry>, request: PlanRequest, texts: &[&str], options: &RunOptions) -> Run {
    let plan = Arc::new(Plan::build(registry, &request).expect("the plan builds"));
    let mut run = Run::new(doc(registry, texts), plan, Externals::none()).expect("a run");
    run.execute(options).expect("the run executes");
    run
}

/// The items of the table `name` after `run`, sorted by tree.
fn sorted_items(run: &Run, name: &str) -> Vec<Value> {
    let mut items = ext_items(run, name);
    items.sort_by_key(|item| item["tree"].as_str().map(str::to_string));
    items
}

/// The plain text of `source` and the run's diagnostics, the run sequential on a worker
/// thread of `flm-core` (so that every handler call runs on that one thread), with `then`
/// called on that thread afterwards.
fn text_on_one_thread<T: Send>(registry: &Arc<FlmRegistry>, source: &str, then: impl FnOnce() -> T + Send) -> (String, Vec<String>, T) {
    flm_core::lang::guard::run_with_worker_stack(|| {
        let plan = Arc::new(Plan::for_workflow(registry, text::WORKFLOW, &Overrides::none()).expect("a plan"));
        let mut run = Run::new(doc(registry, &[source]), plan, Externals::none()).expect("a run");
        run.execute(&RunOptions::sequential()).expect("the run executes");
        let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
        (out, common::diagnostics(&run), then())
    })
    .expect("the worker thread starts")
}

// --- one extension compiled in ------------------------------------------------------------------

#[test]
fn the_stub_s_stage_reads_its_option_calls_the_host_service_and_pushes_items() {
    let probe = Arc::new(Probe::default());
    let loader = loader(&probe, source(&["stub"]));
    let factory = common::factory_of(&loader, "stub");
    let registry = registry(&loader, &["stub"]);
    let request = PlanRequest::new().demand(TALLY).render_options(tagged(&factory, "blue"));
    let run = run(&registry, request, &["One two three.", "Four five."], &RunOptions::new());
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(
        sorted_items(&run, TALLY),
        [
            json!({ "tree": "tree#0", "words": 3, "tag": "blue", "probe": { "twice": 6 } }),
            json!({ "tree": "tree#1", "words": 2, "tag": "blue", "probe": { "twice": 4 } }),
        ]
    );
    let call = |tree: &str, words: u64| {
        json!({ "extension": "stub", "package": "_", "method": "tally", "args": { "tree": tree, "words": words, "tag": "blue" } })
    };
    assert_eq!(probe.calls(), [call("tree#0", 3), call("tree#1", 2)]);
}

/// The rules of `math-passthrough` the stub registers through the guest client under
/// its own names, each with the library's element, declaration and description, and
/// the mark the stub gives.
#[test]
fn the_stub_registers_math_passthrough_s_rules_under_its_own_names() {
    let registry = registry(&loader(&Arc::new(Probe::default()), source(&["stub"])), &["stub"]);
    let rule = |name: &str| registry.rules().iter().find(|entry| entry.rule.name.as_str() == name).expect("registered");
    for (ours, library) in [
        (stub::MATH_RULE, flm_core::math_passthrough::MATH_RULE),
        (stub::ENVIRONMENT_RULE, flm_core::math_passthrough::ENVIRONMENT_RULE),
    ] {
        let (ours, library) = (rule(ours), rule(library));
        assert_eq!(ours.owner.as_str(), "stub");
        assert_eq!(ours.rule.default, DefaultMark::Never);
        assert_eq!((&ours.rule.element, &ours.rule.docs), (&library.rule.element, &library.rule.docs));
        assert_eq!(format!("{:?}", ours.rule.decl), format!("{:?}", library.rule.decl));
    }
}

#[test]
fn the_stub_s_option_left_unset_reads_as_the_stage_s_default() {
    let probe = Arc::new(Probe::default());
    let loader = loader(&probe, source(&["stub"]));
    let registry = registry(&loader, &["stub"]);
    let run = run(&registry, PlanRequest::new().demand(TALLY), &["Just one tree here."], &RunOptions::new());
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(sorted_items(&run, TALLY), [json!({ "tree": "tree#0", "words": 4, "tag": "untagged", "probe": { "twice": 8 } })]);
}

#[test]
fn a_stem_the_engine_does_not_hold_is_refused() {
    let probe = Arc::new(Probe::default());
    let loader = loader(&probe, source(&["absent"]));
    assert_eq!(common::load_problems(&loader, "absent").len(), 1);
}

/// The engine finds an extension by its stem; the identity is the source's, as for every
/// engine.
#[test]
fn the_engine_finds_the_extension_by_its_stem_under_the_source_s_identity() {
    let probe = Arc::new(Probe::default());
    let manifest = json!({ "extension": { "engine": "native" } });
    let source = MemorySource::new().with("stub", "compiled-in:stub", manifest, MemoryFiles::new());
    let loader = loader(&probe, source);
    let factory = common::factory_of(&loader, "stub");
    assert_eq!(factory.name().as_str(), "compiled-in:stub#_");
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
    let run = run(&registry, PlanRequest::new().demand(TALLY), &["Two words."], &RunOptions::new());
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(sorted_items(&run, TALLY).len(), 1);
}

/// The engine's part of the digest is the engine's name, the version of `flm-ext`, and the
/// extension crate's name and version (here the stub's code under another crate's name and
/// version: an extension included in this test program reports the crate `flm-ext`).
#[test]
fn the_engine_s_digest_input_is_its_name_and_the_versions_of_flm_ext_and_of_the_crate() {
    let digest = |compiled: &'static CompiledExtension| {
        let engine = NativeEngine::new([NamedExtension { stem: "stub", compiled }]);
        let identity = ExtensionIdentity { identity: "stub".to_string(), stem: "stub".to_string(), name: "stub".to_string() };
        let manifest = Manifest::read("stub", &json!({ "extension": { "engine": "native" } })).expect("the manifest reads");
        let Ok(loaded) = engine.load(&identity, &manifest, Box::new(MemoryFiles::new())) else { panic!("the extension loads") };
        loaded.runtime.digest_input()
    };
    let version = env!("CARGO_PKG_VERSION");
    assert_eq!(digest(&stub::FLM_EXTENSION), format!("flm-ext native {version}\0flm-ext\0{version}\0").into_bytes());
    let other: &'static CompiledExtension =
        Box::leak(Box::new(CompiledExtension::new(stub::FLM_EXTENSION.define(), "other-crate", "9.9.9")));
    assert_eq!(digest(other), format!("flm-ext native {version}\0other-crate\09.9.9\0").into_bytes());
}

/// Two `use` entries reaching one native extension load it once: one instance is created
/// on the loading thread (its `define` runs once), and the registry build there uses it.
#[test]
fn two_use_entries_of_one_native_extension_load_it_once() {
    let before = second::defines_on_this_thread();
    let probe = Arc::new(Probe::default());
    let loader = loader(&probe, source(&["second"]));
    let first = common::factory_of(&loader, "second");
    let again = common::factory_of(&loader, "second#_");
    assert_eq!(first.name(), again.name());
    let registry = registry_with([again.build(&PackageSettings::new()).expect("the package builds")]);
    assert!(registry.build_diagnostics().is_empty());
    assert_eq!(second::defines_on_this_thread(), before + 1);
}

// --- two extensions compiled in ------------------------------------------------------------------

/// The texts of eight trees, of 1 to 8 words.
fn eight_trees() -> Vec<String> {
    (1..=8).map(|words| vec!["word"; words].join(" ")).collect()
}

/// What the two stages write for [`eight_trees`]: the stub's tally and the second's count.
fn expected_for_eight_trees() -> (Vec<Value>, Vec<Value>) {
    let tally = (1..=8_u64)
        .map(|words| json!({ "tree": format!("tree#{}", words - 1), "words": words, "tag": "untagged", "probe": { "twice": words * 2 } }))
        .collect();
    let count = (1..=8_u64).map(|words| json!({ "tree": format!("tree#{}", words - 1), "characters": words * 5 - 1 })).collect();
    (tally, count)
}

/// Both extensions in one registry, their per-tree stages run on several threads. The loader
/// loads both before the registry build, so on the loading thread the stub's `register` runs
/// after the second's `define`: with one state shared by the two, it would find the second's
/// packages and fail.
#[test]
fn two_extensions_keep_their_own_state_on_several_threads() {
    let probe = Arc::new(Probe::default());
    let loader = loader(&probe, source(&["stub", "second"]));
    let registry = registry(&loader, &["stub", "second"]);
    let texts = eight_trees();
    let texts: Vec<&str> = texts.iter().map(String::as_str).collect();
    let expected = expected_for_eight_trees();
    let tables = |run: &Run| {
        assert!(common::diagnostics(run).is_empty(), "{:?}", common::diagnostics(run));
        (sorted_items(run, TALLY), sorted_items(run, second::COUNT))
    };
    let request = || PlanRequest::new().demand(TALLY).demand(second::COUNT);
    // One run, its per-tree stages on the runner's pool under `flm-core/parallel`.
    assert_eq!(tables(&run(&registry, request(), &texts, &RunOptions::new())), expected);
    // Four runs at once, each on a thread of its own.
    std::thread::scope(|scope| {
        let runs: Vec<_> = (0..4).map(|_| scope.spawn(|| tables(&run(&registry, request(), &texts, &RunOptions::new())))).collect();
        for handle in runs {
            assert_eq!(handle.join().expect("the run's thread ends"), expected);
        }
    });
}

#[test]
fn a_rule_of_one_extension_lowers_an_element_of_the_other_in_a_nested_entry() {
    let probe = Arc::new(Probe::default());
    let loader = loader(&probe, source(&["stub", "second"]));
    let registry = registry(&loader, &["stub", "second"]);
    // The stub's rule lowers the second's element, whose rule lowers the stub's again.
    let (text, diagnostics, ()) =
        text_on_one_thread(&registry, "\\stubbox{a \\secondmark{b \\stubbox{c} d} e} f", || ());
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "<a [second:b <c> d] e> f\n");
}

/// The kind of the refusal of the second's request (the media type of `pdf`) sent while no
/// handler call of the second runs.
const SECOND_REFUSED: &str = "handler";

/// The kind of the answer to a request of the guest client that no host answers.
const UNANSWERED: &str = "unsupported";

/// The second's instance on the worker thread is created inside the stub's request
/// `lower_nodes` (the stub's rule runs first there), so the second's `define` runs while the
/// stub's call is the innermost one on the thread and its table is in use. The request that
/// `define` sends is refused as a request sent outside a handler call of the second, and the
/// stub's table is not touched (the frames' invariant 2).
#[test]
fn a_request_of_a_define_run_inside_another_extension_s_request_is_refused() {
    let probe = Arc::new(Probe::default());
    let loader = loader(&probe, source(&["stub", "second"]));
    let registry = registry(&loader, &["stub", "second"]);
    let (text, diagnostics, answers) =
        text_on_one_thread(&registry, "\\stubbox{a \\secondmark{b} c}", second::define_answers_on_this_thread);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "<a [second:b] c>\n");
    // `define` ran inside the stub's request, and its request was refused.
    assert_eq!(answers, [(true, SECOND_REFUSED.to_string())]);
}

/// The second's rule panics inside the stub's request `lower_nodes`; its instance on the
/// worker thread is discarded there, while the stub's state is the thread's current one and
/// the stub's table is in use. The rule's body sends a request when it is dropped with the
/// instance: the request is the second's, refused as a request sent outside a handler call of
/// the second, and the stub's table is not touched (the frames' invariant 2).
#[test]
fn a_request_of_a_drop_run_inside_another_extension_s_request_is_refused() {
    let probe = Arc::new(Probe::default());
    let loader = loader(&probe, source(&["stub", "second"]));
    let registry = registry(&loader, &["stub", "second"]);
    let (text, diagnostics, answers) =
        text_on_one_thread(&registry, "\\stubbox{a \\secondpanic{now} c}", second::drop_answers_on_this_thread);
    assert_eq!(common::ids(&diagnostics), ["flm.ext.handler-failed"]);
    assert!(diagnostics[0].ends_with(second::PANIC), "{diagnostics:?}");
    assert!(text.starts_with("<a ") && text.ends_with(" c>\n"), "{text:?}");
    // The rule's body was dropped inside the stub's request, and its request was refused.
    assert_eq!(answers, [(true, SECOND_REFUSED.to_string())]);
}

/// When a thread ends, the engine drops the native instances left on it, while the thread's
/// thread-locals are being destroyed: here the stub's and the second's, which the load keeps
/// on the loading thread for the registry build. The `Drop` of the second's rule body sends a
/// request then, which must reach no destroyed thread-local (the request would panic) and no
/// host: it answers that the guest client has no host to send it to.
#[test]
fn the_instances_left_on_a_thread_are_dropped_when_it_ends() {
    /// The key under which the drops on the loading thread record their answers.
    const KEY: u64 = 1;
    let ended = std::thread::spawn(|| {
        second::record_drops_under(KEY);
        let probe = Arc::new(Probe::default());
        let loader = loader(&probe, source(&["stub", "second"]));
        let registry = registry(&loader, &["stub", "second"]);
        let (text, diagnostics, ()) = text_on_one_thread(&registry, "\\secondmark{x}", || ());
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
        text
    })
    .join();
    assert_eq!(ended.expect("the thread ends"), "[second:x]\n");
    let answers = second::drop_answers_under(KEY);
    assert_eq!(answers, [(false, UNANSWERED.to_string())]);
}

/// A request sent from a thread the extension started finds no instance's state there:
/// no host answers it.
#[test]
fn a_request_from_a_thread_the_extension_started_is_not_answered() {
    let probe = Arc::new(Probe::default());
    let loader = loader(&probe, source(&["second"]));
    let registry = registry(&loader, &["second"]);
    let (text, diagnostics, ()) = text_on_one_thread(&registry, "\\secondthread{spawned}", || ());
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "spawned\n");
    assert_eq!(second::thread_answers_for("spawned"), [UNANSWERED]);
}

#[test]
fn a_panic_fails_its_handler_call_and_the_next_call_runs_on_a_fresh_instance() {
    let probe = Arc::new(Probe::default());
    let loader = loader(&probe, source(&["stub", "second"]));
    let registry = registry(&loader, &["stub", "second"]);
    let (text, diagnostics, (defines, dropped)) =
        text_on_one_thread(&registry, "\\secondpanic{now} then \\secondpanic{fine}.", || {
            (second::defines_on_this_thread(), second::drop_answers_on_this_thread())
        });
    assert_eq!(common::ids(&diagnostics), ["flm.ext.handler-failed"]);
    assert!(diagnostics[0].ends_with(second::PANIC), "{diagnostics:?}");
    assert!(text.ends_with(" then fine.\n"), "{text:?}");
    // The panic discarded the thread's instance: `\secondpanic{fine}` ran on a fresh one,
    // which ran `define` again. The discarded instance's rule body sent a request when it was
    // dropped, outside any handler call: refused.
    assert_eq!(defines, 2);
    assert_eq!(dropped, [(false, SECOND_REFUSED.to_string())]);
    // The next document runs.
    let (text, diagnostics, ()) = text_on_one_thread(&registry, "\\secondmark{x}", || ());
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "[second:x]\n");
}

// --- the writer of a formula's TeX through the guest client --------------------------------

/// The parts `parts` of the library as the stub writes them.
fn core_parts(parts: &[core_tex::TexPart<'_>]) -> Value {
    Value::Array(
        parts
            .iter()
            .map(|part| match part {
                core_tex::TexPart::Tex(text) => json!({ "tex": text }),
                core_tex::TexPart::Hole(hole) => {
                    json!({ "hole": { "command": hole.command, "source": hole.node.span_content() } })
                }
            })
            .collect(),
    )
}

/// The items of `stub.tex` the library answers for the trees of `run`, in tree order.
fn library_tex(run: &Run) -> Vec<Value> {
    let mut items = Vec::new();
    for (_, tree) in run.frozen_trees() {
        for node in tree.root().descendants() {
            let element = element_of(&node).name.as_str().to_string();
            if element == flm_core::lang::structural::MATH {
                items.push(json!({ "group": core_parts(&core_tex::group_tex(&node)) }));
            } else if element == ENVIRONMENT {
                let mut lines = Vec::new();
                let mut after_line_end = false;
                for line in core_lines::equation_lines(&node) {
                    let tex = core_tex::line_tex(&line, after_line_end);
                    after_line_end = tex.ends_line();
                    lines.push(json!({ "parts": core_parts(&tex.parts), "trailing": tex.trailing }));
                }
                items.push(json!({ "lines": lines }));
            }
        }
    }
    items
}

/// The items of `stub.tex` the stub's stage pushes for [`FORMULAS`] through `loader`,
/// sorted by their text (the per-tree stages run in any order), and the library's items
/// for the same trees, sorted alike.
fn tex_items(loader: &Arc<ExtensionLoader>) -> (Vec<Value>, Vec<Value>) {
    let registry = registry(loader, &["stub"]);
    let run = run(&registry, PlanRequest::new().demand(TEX), &FORMULAS, &RunOptions::new());
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let mut items = ext_items(&run, TEX);
    let mut expected = library_tex(&run);
    items.sort_by_key(Value::to_string);
    expected.sort_by_key(Value::to_string);
    (items, expected)
}

#[test]
fn the_stub_s_tex_of_each_formula_through_the_guest_client_is_the_library_s() {
    let probe = Arc::new(Probe::default());
    let (items, expected) = tex_items(&loader(&probe, source(&["stub"])));
    assert_eq!(items, expected);
    let hole = |command: &str, source: &str| json!({ "hole": { "command": command, "source": source } });
    let first = json!({ "group": [{ "tex": "x + " }, hole("text", "\\text{if }"), { "tex": " y" }] });
    let comment = json!({ "group": [{ "tex": "\\alpha b" }] });
    let nested = json!({ "group": [{ "tex": "{" }, hole("mbox", "\\mbox{all}"), { "tex": " z}" }] });
    for item in [&first, &comment, &nested] {
        assert!(items.contains(item), "{item} in {items:?}");
    }
    let lines = items.iter().find(|item| item.get("lines").is_some()).expect("the environment");
    assert_eq!(lines["lines"].as_array().map(Vec::len), Some(3), "{lines}");
    assert_eq!(lines["lines"][0]["parts"][1], hole("textbf", "\\textbf{b}"), "{lines}");
}

// --- the same stub, built for WebAssembly ------------------------------------------------------

/// The stub built for `wasm32-unknown-unknown` (once, `FLM_TESTS_WASM_GUEST=1`) and loaded by
/// the engine `wasm` gives the items and the service calls of the native run, and the items
/// of the formulas' TeX.
#[cfg(feature = "wasm")]
#[test]
fn the_stub_built_for_webassembly_gives_the_items_of_the_native_run() {
    use std::path::Path;

    use flm_ext::wasm::{WasmHost, WasmSettings};

    let flm_ext = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(bytes) = common::guest::module_in(flm_ext, &flm_ext.join("tests/native/stub"), "stub") else { return };
    let texts = ["One two three.", "Four five.", "Six."];
    let items_and_calls = |loader: &Arc<ExtensionLoader>, probe: &Probe| {
        let factory = common::factory_of(loader, "stub");
        let registry = registry(loader, &["stub"]);
        let request = PlanRequest::new().demand(TALLY).render_options(tagged(&factory, "green"));
        let run = run(&registry, request, &texts, &RunOptions::new());
        assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
        (sorted_items(&run, TALLY), probe.calls())
    };
    let native_probe = Arc::new(Probe::default());
    let native = items_and_calls(&loader(&native_probe, source(&["stub"])), &native_probe);
    let wasm_probe = Arc::new(Probe::default());
    let files = MemoryFiles::new().with_bytes("stub.wasm", &bytes);
    let wasm_source = MemorySource::new().with("stub", "stub", common::wasm_manifest("./stub.wasm"), files);
    let wasm_loader = Arc::new(
        ExtensionLoader::new(vec![Arc::new(wasm_source)])
            .with_wasm(WasmHost::new(WasmSettings::default()))
            .with_host_service(Arc::clone(&wasm_probe) as Arc<dyn HostService>),
    );
    let wasm = items_and_calls(&wasm_loader, &wasm_probe);
    assert_eq!(native.0.len(), 3);
    assert_eq!(native, wasm);
    // The writer of a formula's TeX, its parts and holes across the binding.
    let (native_tex, expected) = tex_items(&loader(&Arc::new(Probe::default()), source(&["stub"])));
    let (wasm_tex, _) = tex_items(&wasm_loader);
    assert_eq!(native_tex, expected);
    assert_eq!(wasm_tex, native_tex);
}
