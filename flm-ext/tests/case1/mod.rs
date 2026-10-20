//! Case 1 written in Rust against the guest client: the assertions about the
//! extension `wordcount` of `tests/wasm/wordcount/` (the twin of
//! `tests/js/wordcount/`) and the failing guest of `tests/wasm/failing/` that
//! hold for both forms of one extension crate. `tests/wasm_case1.rs` runs them
//! on the crates built for `wasm32-unknown-unknown` and loaded by the engine
//! `wasm`; `tests/native_case1.rs` runs them on the same sources compiled into
//! its test program and loaded by the engine `native`. Each test file gives a
//! [`Loads`], which loads one of the two extensions; the assertions here are
//! written once.
//!
//! Case 1's text, HTML and tables equal byte for byte the JavaScript case's
//! goldens (read from `tests/goldens/quickjs/` and compared, never written); its
//! three settings, the hook's recovery, the rehearsal on a worker thread; the
//! failing guest's hook answering an error, and the diagnostic of its hook that
//! panics (whose text each engine words its own way).

use std::path::Path;
use std::sync::Arc;

use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageFactory};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::{FeatureSettings, PackageSettings};
use flm_core::dump::golden::{check_json_golden, check_text_golden, GoldenMode};
use flm_core::dump::json::from_json;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::options::OptionValues;
use flm_ext::adapter::table::ExtTable;
use flm_ext::adapter::values::diagnostic_value_to_json;
use serde_json::{json, Value};

use crate::common::{self, html_of, registry_with, text_of};

/// How a test file loads the two extensions of the case.
pub trait Loads {
    /// The factory of the package `_` of the extension `stem` (`wordcount` or
    /// `failing`), from a load of its own.
    fn factory(&self, stem: &str) -> PackageFactory;
}

// --- helpers -----------------------------------------------------------------------------------

/// The document of the JavaScript case (`tests/quickjs_case1.rs`).
pub const DOCUMENT: &str = "\\flmname{} counts words. \\sayhi{World}\n\n\\shout{listen} to this \\todo{check the numbers}.\n\n\\section{Two}\nSecond paragraph, \\shout{quietly}.";

/// The table of word counts.
pub const WORDS: &str = "wordcount.words";

/// The table of markers.
pub const TODOS: &str = "wordcount.todos";

/// The golden file `name` of the JavaScript case, under `tests/goldens/quickjs/`.
fn javascript_golden(name: &str) -> String {
    format!("{}/tests/goldens/quickjs/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// `text` equal to the JavaScript case's golden `name`, compared and never written (a
/// difference is a finding, never a new golden).
fn assert_javascript_text(name: &str, text: &str) {
    if let Err(error) = check_text_golden(Path::new(&javascript_golden(name)), text, GoldenMode::Compare) {
        panic!("{error}");
    }
}

/// `value` equal to the JavaScript case's JSON golden `name`, compared and never written.
fn assert_javascript_json(name: &str, value: &Value) {
    let serial = from_json(&value.to_string()).expect("the value is JSON");
    if let Err(error) = check_json_golden(Path::new(&javascript_golden(name)), &serial, GoldenMode::Compare) {
        panic!("{error}");
    }
}

/// The registry of the standard package and the package of the extension `stem`, no
/// setting written.
pub fn registry_of(loads: &dyn Loads, stem: &str) -> Arc<FlmRegistry> {
    registry_with([loads.factory(stem).build(&PackageSettings::new()).expect("the package builds")])
}

/// The settings of case 1's feature read from `data` with its declaration.
fn wordcount_settings(factory: &PackageFactory, data: Value) -> FeatureSettings {
    let decl = factory.settings("wordcount").expect("the feature declares settings");
    let Value::Object(map) = data else { panic!("not a map") };
    decl.read(&map).expect("the settings read")
}

/// The document of one tree per text of `texts`.
pub fn doc(registry: &Arc<FlmRegistry>, texts: &[&str], options: OptionValues) -> Arc<Doc> {
    let mut content = DocContent::new();
    for text in texts {
        let source = Arc::new(FlmSource::new(*text).with_origin(FlmOrigin::none()));
        let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
            .expect("the tree parses");
        content.place(Arc::new(fragment));
    }
    Arc::new(Doc::with_options(content, options))
}

/// The run of a plan demanding `demands` over a document of one tree per text of `texts`,
/// sequential (the stages run on the calling thread when it is a marked worker).
pub fn run_demanding(registry: &Arc<FlmRegistry>, demands: &[&str], texts: &[&str]) -> Run {
    let request = demands.iter().fold(PlanRequest::new(), |request, demand| request.demand(*demand));
    let plan = Arc::new(Plan::build(registry, &request).expect("the plan builds"));
    let mut run = Run::new(doc(registry, texts, OptionValues::new()), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::sequential()).expect("the run executes");
    run
}

/// The items of the extension's table `name` after `run`.
pub fn items(run: &Run, name: &str) -> Vec<Value> {
    run.table(TableHandle::<ExtTable>::new(name))
        .expect("the table is declared")
        .map(|table| table.items().to_vec())
        .unwrap_or_default()
}

/// The data of the first diagnostic of `run` whose identifier is `id`, as JSON.
pub fn data_of(run: &Run, id: &str) -> Value {
    run.diagnostics()
        .iter()
        .find(|d| d.identifier() == id)
        .map(|d| diagnostic_value_to_json(&d.data().serializable_data()))
        .unwrap_or(Value::Null)
}

/// `f` run on a thread marked as a worker of `flm-core`, on which a run executes its
/// stages itself.
pub fn on_worker<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    flm_core::lang::guard::run_with_worker_stack(f).expect("the worker thread starts")
}

// --- case 1: the JavaScript case's goldens ------------------------------------------------------

/// Case 1 renders through the text workflow as the JavaScript case does.
pub fn case_one_renders_through_the_text_workflow(loads: &dyn Loads) {
    let registry = registry_of(loads, "wordcount");
    let (text, diagnostics) = text_of(&registry, DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_javascript_text("case1.txt", &text);
    // The hook agrees with WP-4's golden for the same text, as the JavaScript case's does.
    let (text, diagnostics) = text_of(&registry, "Say \\shout{hey there} and \\shout{you}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let shout = format!("{}/tests/goldens/shout.txt", env!("CARGO_MANIFEST_DIR"));
    if let Err(error) = check_text_golden(Path::new(&shout), &text, GoldenMode::Compare) {
        panic!("{error}");
    }
}

/// Case 1 renders through the HTML workflow as the JavaScript case does.
pub fn case_one_renders_through_the_html_workflow(loads: &dyn Loads) {
    let (html, diagnostics) = html_of(&registry_of(loads, "wordcount"), DOCUMENT);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_javascript_text("case1.html", &html);
}

/// Case 1's tables project as the JavaScript case's do.
pub fn case_one_s_tables_project_as_under_javascript(loads: &dyn Loads) {
    let registry = registry_of(loads, "wordcount");
    let run = run_demanding(&registry, &[WORDS, TODOS], &[DOCUMENT, "A \\todo{last} one."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    let mut words = items(&run, WORDS);
    words.sort_by_key(|item| item["tree"].as_str().map(str::to_string));
    assert_javascript_json("case1-words.json", &Value::Array(words));
    assert_javascript_json("case1-todos.json", &Value::Array(items(&run, TODOS)));
}

/// Case 1's registry builds without a diagnostic.
pub fn case_one_s_registry_builds_without_a_diagnostic(loads: &dyn Loads) {
    let registry = registry_of(loads, "wordcount");
    let diagnostics: Vec<String> = registry.build_diagnostics().iter().map(|d| d.message().to_string()).collect();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

/// The hook of `\shout` given a command recovers the use site with its own diagnostic.
pub fn a_shout_of_a_command_recovers_the_use_site_with_the_guest_s_diagnostic(loads: &dyn Loads) {
    let registry = registry_of(loads, "wordcount");
    let run = common::run(&registry, flm_core::stdcore::backends::text::WORKFLOW, "Say \\shout{\\emph{x}}.");
    assert_eq!(common::diagnostics(&run), ["wordcount.shout.not-characters: the argument of \\shout must be plain characters"]);
    let (text, _) = text_of(&registry, "Say \\shout{\\emph{x}}.");
    assert!(text.contains("\\shout{\\emph{x}}"), "{text}");
}

/// A run on the thread that built the registry and a run on another thread, whose
/// instance rehearses the feature first, give the same tables.
pub fn the_rehearsal_declares_the_same_objects(loads: &dyn Loads) {
    let texts = [DOCUMENT, "A \\todo{last} one."];
    let tables = |run: &Run| {
        let mut words = items(run, WORDS);
        words.sort_by_key(|item| item["tree"].as_str().map(str::to_string));
        (words, items(run, TODOS))
    };
    // Built and run on one marked worker thread: the stages run on the instance
    // `register` ran on.
    let factory = loads.factory("wordcount");
    let same = on_worker(|| {
        let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
        tables(&run_demanding(&registry, &[WORDS, TODOS], &texts))
    });
    // Built on this thread, run on another marked worker thread: the feature is
    // rehearsed on that thread's instance first.
    let registry = registry_of(loads, "wordcount");
    let shared = Arc::clone(&registry);
    let other = std::thread::Builder::new()
        .stack_size(64 << 20)
        .spawn(move || on_worker(|| tables(&run_demanding(&shared, &[WORDS, TODOS], &texts))))
        .expect("the thread starts")
        .join()
        .expect("the thread runs");
    assert_eq!(other, same);
    assert_eq!(same.0.len(), 2, "{same:?}");
    assert_eq!(same.1, [json!({ "tree": "tree#0", "text": "check the numbers" }), json!({ "tree": "tree#1", "text": "last" })]);
}

// --- the settings ------------------------------------------------------------------------------

/// `register` receives the feature's configuration; a hook run on a worker thread,
/// whose instance rehearses `register`, sees the same.
pub fn register_receives_the_configuration_and_a_hook_on_a_worker_sees_the_same(loads: &dyn Loads) {
    let factory = loads.factory("wordcount");
    let mut settings = PackageSettings::new();
    settings.insert(wordcount_settings(&factory, json!({ "shout_suffix": "!" })));
    let registry = registry_with([factory.build(&settings).expect("the package builds")]);
    // Built on this thread; the text workflow runs the hook on a run's own worker
    // thread, whose instance rehearses `register` with the same configuration.
    let (text, diagnostics) = text_of(&registry, "Say \\shout{hey}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Say HEY!.\n");
    // Another build of the same factory with another configuration, run on a marked
    // worker thread that already ran the first build's hook.
    let mut other = PackageSettings::new();
    other.insert(wordcount_settings(&factory, json!({ "shout_suffix": "?" })));
    let second = registry_with([factory.build(&other).expect("the package builds")]);
    let texts = on_worker(|| {
        let first = text_of(&registry, "\\shout{a}").0;
        let then = text_of(&second, "\\shout{a}").0;
        let again = text_of(&registry, "\\shout{a}").0;
        (first, then, again)
    });
    assert_eq!(texts, ("A!\n".to_string(), "A?\n".to_string(), "A!\n".to_string()));
}

/// The count stage reads the render option and the document option.
pub fn the_count_stage_reads_both_options(loads: &dyn Loads) {
    let factory = loads.factory("wordcount");
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
    let render = wordcount_settings(&factory, json!({ "min_length": 5 })).render_options().clone();
    let document = wordcount_settings(&factory, json!({ "report_counts": true })).document_options().clone();
    let run_with = |document: OptionValues, render: OptionValues| {
        let request = PlanRequest::new().demand(WORDS).render_options(render);
        let plan = Arc::new(Plan::build(&registry, &request).expect("the plan builds"));
        let mut run =
            Run::new(doc(&registry, &["One two three seven eleven."], document), plan, Externals::none()).expect("a run");
        run.execute(&RunOptions::sequential()).expect("the run executes");
        run
    };
    // Nothing written: every word counts, and no note.
    let run = run_with(OptionValues::new(), OptionValues::new());
    assert_eq!(items(&run, WORDS), [json!({ "tree": "tree#0", "words": 5 })]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    // Both written: the words of five characters or more, and the note.
    let run = run_with(document, render);
    assert_eq!(items(&run, WORDS), [json!({ "tree": "tree#0", "words": 3 })]);
    assert_eq!(common::diagnostics(&run), ["wordcount.count: the tree `tree#0` holds 3 words of at least 5 characters"]);
}

/// The count stage's capability joins `flm.validated`, so that the count runs with every
/// standard workflow, and its Note appears under the plain text and HTML workflows.
pub fn the_count_s_note_appears_under_the_plain_workflows(loads: &dyn Loads) {
    let factory = loads.factory("wordcount");
    let registry = registry_with([factory.build(&PackageSettings::new()).expect("the package builds")]);
    let document = wordcount_settings(&factory, json!({ "report_counts": true })).document_options().clone();
    for workflow in [flm_core::stdcore::backends::text::WORKFLOW, flm_core::stdcore::backends::html::WORKFLOW] {
        let plan = Arc::new(
            Plan::for_workflow(&registry, workflow, &flm_core::core::plan::Overrides::none()).expect("the plan builds"),
        );
        let doc = doc(&registry, &["One two three seven eleven."], document.clone());
        let mut run = Run::new(doc, plan, Externals::none()).expect("a run");
        run.execute(&RunOptions::sequential()).expect("the run executes");
        assert_eq!(
            common::diagnostics(&run),
            ["wordcount.count: the tree `tree#0` holds 5 words of at least 1 characters"],
            "{workflow}"
        );
    }
}

// --- the failing guest -------------------------------------------------------------------------

/// The hook of `\fails` answers an error of its own: the handler's failure carries it,
/// and the use site is recovered.
pub fn a_hook_answering_an_error_fails_with_the_guest_s_error(loads: &dyn Loads) {
    let registry = registry_of(loads, "failing");
    let run = common::run(&registry, flm_core::stdcore::backends::text::WORKFLOW, "Say \\fails{hey}.");
    assert_eq!(
        common::diagnostics(&run),
        ["flm.ext.handler-failed: the expansion hook of `failing.macro.fails` failed: the hook is not ready"]
    );
    assert_eq!(data_of(&run, "flm.ext.handler-failed")["data"], json!({ "name": "NotReady" }));
    let (text, _) = text_of(&registry, "Say \\fails{hey}.");
    assert_eq!(text, "Say \\fails{hey}.\n");
}

/// The beginning of the diagnostic of the hook of `\panics`, as the expansion path words
/// every binding's failure of a hook; each engine adds its own words for the panic.
pub const PANIC_PREFIX: &str = "flm.ext.handler-failed: the expansion hook of `failing.macro.panics` failed: the expansion \
                                hook of the element `failing.macro.panics` of the feature `failing` of the package `_` of \
                                the extension `failing` must run without a ";

/// The diagnostics of a run of `\panics` and the data of its failure, for the engine's own
/// assertions.
pub fn the_panicking_hook_s_failure(loads: &dyn Loads) -> (Vec<String>, Value) {
    let registry = registry_of(loads, "failing");
    let run = common::run(&registry, flm_core::stdcore::backends::text::WORKFLOW, "Say \\panics{hey}.");
    (common::diagnostics(&run), data_of(&run, "flm.ext.handler-failed")["data"].clone())
}
