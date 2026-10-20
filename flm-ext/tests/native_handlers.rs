//! The engine `native` runs every handler kind of the bindings' shared host
//! side: the extension `rows` of `tests/native/rows.rs`, written against the
//! guest client and compiled into this test program, one package per group of
//! the handler rows that `tests/wasm_engine.rs` drives through scripted
//! WebAssembly guests (a rule's body, `points` and the three sectioning
//! methods; a rewrite stage; a citation provider; a template module's
//! description, files and render; a late-initialization hook and a finalizer,
//! replayed by the rehearsal on another thread). Case 1 (`tests/native_case1.rs`)
//! and the stub (`tests/native.rs`) cover the other rows: the stages, the
//! expansion hooks, the lowering rules, `define`, `packages`, `register` and
//! `rehearse`.
//!
//! The package `rows#forms` runs against the real host the scenarios that the
//! guest client's own tests answer with a test hook (`src/guest/registry.rs`)
//! or find no host for (`tests/guest_forms.rs`, a helper's declaration): a
//! helper's declaration whose documentation and modes the host answers (the
//! modes checked; the registry does not expose the documentation), a fallback
//! and a scoped package whose elements carry lent aspects, and a value
//! contributed to a slot of the feature's own. The package `rows#backend` runs
//! a backend whose emit and final stages are the extension's own; the
//! contributions of a citation provider and of a template module to their
//! slots are those of `rows#cite` and `rows#template`.
//!
//! Run also under `--features flm-core/parallel`, where a run's per-tree stages
//! go to the runner's pool.

#![cfg(feature = "native")]

mod common;

#[path = "native/rows.rs"]
mod rows;

use std::sync::Arc;

use flm_core::core::artifact::ArtifactSet;
use flm_core::core::data::table::TableHandle;
use flm_core::core::plan::{workflow_request, Overrides, Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageEntry};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::aspects::{Referenceable, Sectioning};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::{html, text};
use flm_core::stdcore::features::compose::{stage_name, ComposeOptions, TemplateChoice};
use flm_ext::native::{NativeEngine, NativeExtension};
use flm_ext::{ExtensionLoader, MemoryFiles, MemorySource};
use serde_json::json;

use common::{compose_registry, ext_items, registry_with, text_of, toc_entries};

// --- helpers -----------------------------------------------------------------------------------

/// The package `rows#PACKAGE`, from a load of its own, no setting written.
fn package(name: &str) -> PackageEntry {
    let engine = NativeEngine::new([NativeExtension { stem: "rows", manifest: "", compiled: &rows::FLM_EXTENSION }])
        .with_console(Arc::new(|_, _| {}));
    let source = MemorySource::new().with("rows", "rows", json!({ "extension": { "engine": "native" } }), MemoryFiles::new());
    let loader = Arc::new(ExtensionLoader::new(vec![Arc::new(source)]).with_engine(Arc::new(engine)));
    common::factory_of(&loader, &format!("rows#{name}")).build(&PackageSettings::new()).expect("the package builds")
}

/// The registry of the standard package and the package `rows#PACKAGE`.
fn registry(name: &str) -> Arc<FlmRegistry> {
    registry_with([package(name)])
}

/// The run of `request` over the document `source`.
fn run(registry: &Arc<FlmRegistry>, request: &PlanRequest, source: &str) -> Run {
    let source = Arc::new(FlmSource::new(source).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Arc::new(Plan::build(registry, request).expect("the plan builds"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The names of the elements of the nodes `text` parses to.
fn element_names(registry: &FlmRegistry, text: &str) -> Vec<String> {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("a parse");
    fragment.tree().root().descendants().map(|node| node.ext().element.name.to_string()).collect()
}

/// The build diagnostics of `registry`, rendered.
fn build_diagnostics(registry: &FlmRegistry) -> Vec<String> {
    registry.build_diagnostics().iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect()
}

// --- the handler rows --------------------------------------------------------------------------

#[test]
fn rows_3_9_and_10_a_rule_s_body_points_and_sectioning() {
    let registry = registry("box");
    assert!(build_diagnostics(&registry).is_empty(), "{:?}", build_diagnostics(&registry));
    let (text, diagnostics) = text_of(&registry, "\\rowsbox{Alpha}\\label{ex:one} See \\ref{ex:one}.");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(text, "Alpha See Alpha.\n");
    // The aspects declared with the element: level 2, the first argument as the title.
    assert_eq!(toc_entries(&registry, "\\rowsbox{Intro} text \\rowsbox{More}"), [(2, true), (2, true)]);
    // The aspect `attach` added: level 3, the trait's default title (none).
    assert_eq!(toc_entries(&registry, "\\rowspart{Intro}"), [(3, false)]);
}

#[test]
fn row_4_a_rewrite_stage_restages() {
    let registry = registry("rewrite");
    let respelled = |demanded: bool| {
        let request = workflow_request(&registry, text::WORKFLOW, &Overrides::none()).expect("the workflow");
        let request = if demanded { request.demand(rows::RESPELLED) } else { request };
        let ran = run(&registry, &request, "A colour.");
        assert!(common::diagnostics(&ran).is_empty(), "{:?}", common::diagnostics(&ran));
        ran.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default()
    };
    assert_eq!(respelled(true), "A color.\n");
    // Without its table demanded, the rewrite stage does not run.
    assert_eq!(respelled(false), "A colour.\n");
}

#[test]
fn row_5_a_citation_provider_answers_the_requests() {
    let registry = registry("cite");
    let (_, diagnostics) = text_of(&registry, "See \\cite{knuth84}.");
    assert!(diagnostics.iter().any(|d| d.contains(rows::NO_ENTRY)), "{diagnostics:?}");
}

#[test]
fn rows_6_7_and_8_a_template_module_describes_itself_inside_register_then_ships_and_renders() {
    // The description runs inside `register`'s request (a nested entry into the same
    // instance), then the composer ships the module's file and renders its page.
    let registry = compose_registry(package("template")).expect("the registry builds");
    let options = ComposeOptions {
        template: TemplateChoice::Name("tiny".to_string()),
        template_config: [("html".to_string(), [("tiny".to_string(), serde_json::Map::new())].into_iter().collect())]
            .into_iter()
            .collect(),
    };
    let overrides =
        Overrides { prefer: vec![stage_name("html").into()], render_options: OptionValues::new().with(options), ..Overrides::none() };
    let request = workflow_request(&registry, html::WORKFLOW, &overrides).expect("the workflow");
    let ran = run(&registry, &request, "Some text.");
    assert!(common::diagnostics(&ran).is_empty(), "{:?}", common::diagnostics(&ran));
    let set = ran.table(html::DOCUMENT).expect("declared").cloned().unwrap_or_default();
    let page = set.entry().and_then(|entry| entry.as_bytes()).map(|bytes| String::from_utf8_lossy(bytes).into_owned());
    assert_eq!(page.as_deref(), Some("<html>tiny tiny-style.css</html>\n"));
    let style = set.iter().find(|artifact| !artifact.entry).and_then(|artifact| artifact.as_bytes());
    assert_eq!(style, Some(&b"h1 {}\n"[..]));
}

#[test]
fn rows_11_and_12_a_late_initialization_hook_and_a_finalizer_and_their_rehearsal_on_a_worker_thread() {
    let registry = registry("hooks");
    assert_eq!(build_diagnostics(&registry), ["rows.late: the language is built"]);
    // The stage the finalizer declared runs on a worker thread, whose own instance runs
    // `define`, then the rehearsal (`register`, then the finalizer), then the stage.
    let items = flm_core::lang::guard::run_with_worker_stack(|| {
        let ran = run(&registry, &PlanRequest::new().demand(rows::LATE), "One tree.");
        assert!(common::diagnostics(&ran).is_empty(), "{:?}", common::diagnostics(&ran));
        ext_items(&ran, rows::LATE)
    })
    .expect("the worker thread starts");
    assert_eq!(items, [json!({ "filled": true })]);
}

// --- the values holding handler objects, against the host ----------------------------------------

#[test]
fn the_aspects_of_an_element_s_declaration_a_fallback_and_a_scoped_package_reach_the_host() {
    let forms = registry("forms");
    assert!(build_diagnostics(&forms).is_empty(), "{:?}", build_diagnostics(&forms));
    let aspects = |name: &str| {
        let element = forms.element_named(name).unwrap_or_else(|| panic!("the element `{name}` is registered"));
        (element.has_aspect::<dyn Referenceable>(), element.has_aspect::<dyn Sectioning>())
    };
    assert_eq!(aspects("rows.loose"), (true, false));
    assert_eq!(aspects("rows.item"), (false, true));
    // An environment that nothing defines, in text mode, is the fallback's element.
    assert!(element_names(&forms, "\\begin{nosuch}x\\end{nosuch}").iter().any(|name| name == "rows.loose"));
    let boxed = registry("box");
    let aspects = |name: &str| {
        let element = boxed.element_named(name).unwrap_or_else(|| panic!("the element `{name}` is registered"));
        (element.has_aspect::<dyn Referenceable>(), element.has_aspect::<dyn Sectioning>())
    };
    assert_eq!(aspects("rows.box"), (true, true));
    assert_eq!(aspects("rows.part"), (false, true));
}

#[test]
fn a_helper_s_declaration_sends_its_documentation_and_its_modes_to_the_host() {
    // `docs` and `modes` are requests the host answers with its declaration (in the guest
    // client's own tests, with no host, they answer `Unsupported`): the feature's
    // `register` runs to its end and the registry builds cleanly. Of their effects only the
    // modes can be observed, since the registry does not expose a declaration's
    // documentation: the declaration is in math mode only.
    let registry = registry("forms");
    assert!(build_diagnostics(&registry).is_empty(), "{:?}", build_diagnostics(&registry));
    assert!(element_names(&registry, "\\(\\rowsgreet\\)").iter().any(|name| name == "rows.macro.rowsgreet"));
    assert!(!element_names(&registry, "\\rowsgreet").iter().any(|name| name == "rows.macro.rowsgreet"));
}

#[test]
fn a_backend_s_own_emit_and_final_stages_write_its_output() {
    let registry = registry("backend");
    let output = registry.output("rows").expect("the output `rows`");
    assert_eq!((output.def.entry.extension.as_str(), output.def.entry.media_type.as_str()), ("txt", "text/plain"));
    let ran = run(&registry, &PlanRequest::new().demand(rows::DOCUMENT), "One tree.");
    assert!(common::diagnostics(&ran).is_empty(), "{:?}", common::diagnostics(&ran));
    let set = ran.table(TableHandle::<ArtifactSet>::new(rows::DOCUMENT)).expect("declared").cloned().unwrap_or_default();
    let entry = set.entry().and_then(|entry| entry.as_bytes()).map(|bytes| String::from_utf8_lossy(bytes).into_owned());
    assert_eq!(entry.as_deref(), Some("emitted, the lowered trees read: true"));
}

#[test]
fn a_value_contributed_first_to_a_slot_is_read_back_by_a_stage() {
    let registry = registry("forms");
    let ran = run(&registry, &PlanRequest::new().demand(rows::READ), "One tree.");
    assert!(common::diagnostics(&ran).is_empty(), "{:?}", common::diagnostics(&ran));
    assert_eq!(ext_items(&ran, rows::READ), [json!({ "values": [{ "k": [1, 2] }] })]);
}
