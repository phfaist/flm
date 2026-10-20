//! Service slots, handler objects and host services written in JavaScript
//! (E6, plan sections 3.4, 4.2 and 7.3), through small scripts: a slot a
//! script declares and fills across two packages, read by a stage; a citation
//! provider answering through `answer_all` or `answer` alone; a template
//! module describing itself, shipping files and rendering; the `files`
//! service in a stage, a provider and a template; the rehearsal on another
//! thread finding the provider and the template by name, and refusing a name
//! a second run of `register` does not declare; the name rule; the package
//! object's `host` list.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;

use flm_core::core::artifact::{ArtifactId, ArtifactNamer, ArtifactRef, NamingScope, RelPath};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::lowering::NAMER_SLOT;
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{FlmFeature, FlmRegistry, PackageEntry, PackageFactory, RegistryBuildError, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::backends::{html, text};
use flm_core::stdcore::features::compose::{stage_name, ComposeFeature, ComposeOptions, TemplateChoice};
use flm_core::stdcore::{self, StdcoreConfig};
use flm_ext::host::HostService;
use flm_ext::quickjs::{JsHost, JsSettings};
use flm_ext::{ExtensionLoader, MemoryFiles};
use serde_json::{json, Value};

use common::{ext_items, factory_of, js_extension, js_manifest, load_problems, registry_with, try_registry_with, MemoryHostFiles};

// --- helpers -----------------------------------------------------------------------------------

/// The loader of the extension `wc` of the files `files` (its main module
/// `main.js`), offering the `files` service `files` when given.
fn loader(files: &[(&str, &str)], service: Option<&Arc<MemoryHostFiles>>) -> Arc<ExtensionLoader> {
    let source = js_extension("wc", files, js_manifest("main.js"));
    let loader = ExtensionLoader::new(vec![Arc::new(source)]).with_js(JsHost::new(JsSettings::default()));
    Arc::new(match service {
        Some(service) => loader.with_host_service(Arc::clone(service) as Arc<dyn HostService>),
        None => loader,
    })
}

/// The package `entry` of `loader`, built with the configuration `config`
/// of its feature `feature` (none when `config` is `null`).
fn built(loader: &Arc<ExtensionLoader>, entry: &str, feature: &str, config: Value) -> PackageEntry {
    let factory: PackageFactory = factory_of(loader, entry);
    let mut settings = PackageSettings::new();
    if let Value::Object(config) = config {
        let decl = factory.settings(feature).expect("the feature declares settings");
        settings.insert(decl.read(&config).expect("the settings read"));
    }
    factory.build(&settings).expect("the package builds")
}

/// The document's folder of the tests.
fn the_document_s_folder() -> Arc<MemoryHostFiles> {
    let files = MemoryFiles::new()
        .with_text("doc.txt", "from the document")
        .with_text("works.json", r#"{ "a": "Alpha", "b": "Beta", "c": "Gamma" }"#)
        .with_text("note.txt", "a note\n")
        .with_bytes("banner.png", b"PNG");
    Arc::new(MemoryHostFiles::new(files))
}

/// `f` run on a thread marked as a worker of `flm-core`, on which a
/// sequential run executes its stages itself.
fn on_worker<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    flm_core::lang::guard::run_with_worker_stack(f).expect("the worker thread starts")
}

/// `f` run on a marked worker thread of a new thread, which has no engine
/// instance yet.
fn on_other_thread<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn_scoped(scope, || on_worker(f))
            .expect("the thread starts")
            .join()
            .expect("the thread runs")
    })
}

/// The run of `workflow` over the document `source`, sequential (on the
/// calling thread when it is a marked worker), with `overrides`.
fn run_with(registry: &Arc<FlmRegistry>, workflow: &str, overrides: &Overrides, source: &str) -> Run {
    let source = Arc::new(FlmSource::new(source).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Arc::new(Plan::for_workflow(registry, workflow, overrides).expect("a plan"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::sequential()).expect("the run executes");
    run
}

/// The text output of `source` and the run's diagnostics, sequential.
fn text_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = run_with(registry, text::WORKFLOW, &Overrides::none(), source);
    let out = run.table(text::TEXT_OUTPUT).expect("declared").map(|o| o.text.clone()).unwrap_or_default();
    (out, common::diagnostics(&run))
}

/// The registry build's error, as text.
fn build_error(packages: impl IntoIterator<Item = PackageEntry>) -> String {
    match try_registry_with(packages) {
        Ok(_) => panic!("the registry builds"),
        Err(error) => error.to_string(),
    }
}

// --- a script's own service slot ---------------------------------------------------------------

/// The extension `wc`: the package `_` declares the slot `wc.notes`, fills it
/// and reads it from a stage (in the assembled order and in the order of
/// precedence), with what it reads through `files`; the package `extra` fills
/// it, once first.
const NOTES: &str = r#"
import { StageDecl, Effect, Scope } from "flm:core/stage";
import { Arity } from "flm:core/services";
import { FilesScope } from "flm:host";

export default function (ext) {
  ext.package("_", { host: ["files"], features: { notes: { register(b) {
    b.service_slot("wc.notes", Arity.OrderedMany, "notes, each `{ note }`");
    b.contribute_to("wc.notes", { note: "own" });
    b.table("wc.seen", "what the stage saw");
    b.stage({
      decl: () => new StageDecl("wc.see", Effect.Compute, Scope.Document).writes("wc.seen"),
      run(cx, out) {
        const files = cx.host("files");
        out.table("wc.seen").push({
          first: cx.service("wc.notes"),
          all: cx.services("wc.notes"),
          by_precedence: cx.services_by_precedence("wc.notes"),
          undeclared: [cx.service("wc.none"), cx.services("wc.none")],
          extension: files.read_text(FilesScope.Extension, "data.txt"),
          document: files.read_text("document", "doc.txt"),
          missing: files.read_text("document", "missing.txt"),
        });
      },
    });
  } } } });
  ext.package("extra", { features: { extra: { register(b) {
    b.contribute_to("wc.notes", { note: "extra" });
    b.contribute_first_to("wc.notes", { note: "first" });
  } } } });
}
"#;

#[test]
fn a_slot_declared_and_filled_across_two_packages_is_read_by_a_stage() {
    let service = the_document_s_folder();
    let loader = loader(&[("main.js", NOTES), ("data.txt", "from the extension")], Some(&service));
    let registry =
        registry_with([built(&loader, "wc", "notes", Value::Null), built(&loader, "wc#extra", "extra", Value::Null)]);
    let run = common::run_demanding(&registry, &["wc.seen"], &["Text."]);
    assert!(common::diagnostics(&run).is_empty(), "{:?}", common::diagnostics(&run));
    assert_eq!(
        ext_items(&run, "wc.seen"),
        [json!({
            "first": { "note": "first" },
            "all": [{ "note": "first" }, { "note": "own" }, { "note": "extra" }],
            // The value added first stays ahead; then the feature placed last
            // (`extra`, whose package is used after `wc`'s) comes first.
            "by_precedence": [{ "note": "first" }, { "note": "extra" }, { "note": "own" }],
            "undeclared": [null, []],
            "extension": "from the extension",
            "document": "from the document",
            "missing": null,
        })]
    );
    // The stage read the document's file as its package, through the host.
    let calls = service.calls.lock().expect("the calls");
    let seen: Vec<(&str, &str, &Value)> =
        calls.iter().map(|(caller, method, args)| (caller.package.as_str(), method.as_str(), &args["path"])).collect();
    assert_eq!(seen, [("_", "read_text", &json!("doc.txt")), ("_", "read_text", &json!("missing.txt"))]);
}

// --- citation providers ------------------------------------------------------------------------

/// The extension `wc`: two citation providers, `wc.all` answering the keys
/// its document file `works.json` holds in one call, and `wc.one` answering
/// every key one by one.
const PROVIDERS: &str = r#"
import { PROVIDERS_SLOT } from "flm:stdcore/features/citations";

const all = {
  name: "wc.all",
  answer_all(requests, cx) {
    const works = JSON.parse(cx.host("files").read_text("document", "works.json"));
    return requests.map(({ key }) =>
      key in works ? { entry: cx.build_entry([{ emphasized: [{ text: works[key] }] }]), identity: key } : null);
  },
};

const one = {
  name: "wc.one",
  answer(request, cx) {
    return { entry: cx.parse_entry(`\\textbf{${request.key}} one`, "one"), warnings: ["one by one"] };
  },
};

export default function (ext) {
  ext.package("_", { host: ["files"], features: { bib: { register(b) {
    b.peer_slot(PROVIDERS_SLOT, all);
    b.contribute_to(PROVIDERS_SLOT, one);
  } } } });
}
"#;

/// The registry of the standard package and the providers' extension.
fn providers_registry() -> Arc<FlmRegistry> {
    let service = the_document_s_folder();
    registry_with([built(&loader(&[("main.js", PROVIDERS)], Some(&service)), "wc", "bib", Value::Null)])
}

/// The text of the document citing four keys, with its diagnostics.
const CITING: &str = "A \\cite{a}. B \\cite{b,c}. C \\cite{d}.";

/// What [`CITING`] renders to, the three keys of the file answered in one
/// call, the fourth by the provider of `answer` alone.
const CITED: &str =
    "A\u{a0}[1]. B\u{a0}[2,3]. C\u{a0}[4].\n\nReferences\n----------\n\n[1] Alpha\n\n[2] Beta\n\n[3] Gamma\n\n[4] d one\n";

#[test]
fn a_provider_answers_three_keys_in_one_call_and_another_through_answer_alone() {
    let registry = providers_registry();
    let (out, diagnostics) = on_worker(|| text_of(&registry, CITING));
    assert_eq!(out, CITED);
    assert_eq!(diagnostics, ["flm.citations.provider-warning: one by one"]);
}

#[test]
fn the_rehearsal_on_another_thread_finds_the_providers_by_name() {
    // Built on this thread; run on a new thread, whose engine instance
    // rehearses `register` and finds both objects by their names.
    let registry = providers_registry();
    let (out, diagnostics) = on_other_thread(|| text_of(&registry, CITING));
    assert_eq!(out, CITED);
    assert_eq!(diagnostics, ["flm.citations.provider-warning: one by one"]);
}

/// The extension `wc` whose feature `bib` contributes a provider named after
/// how many times `register` ran on the engine instance (`wc.p0`, `wc.p1`,
/// …), with a configuration `x`.
const COUNTING_PROVIDER: &str = r#"
import { PROVIDERS_SLOT } from "flm:stdcore/features/citations";

let runs = 0;

export default function (ext) {
  ext.package("_", { features: { bib: {
    settings: { config: { docs: "x", schema: { type: "object", properties: { x: { type: "string" } } } } },
    register(b) {
      b.peer_slot(PROVIDERS_SLOT, { name: `wc.p${runs++}`, answer(request, cx) { return { entry: cx.build_entry([{ text: "w" }]) }; } });
    },
  } } });
}
"#;

#[test]
fn a_provider_a_second_run_of_register_does_not_declare_fails_each_key() {
    let loader = loader(&[("main.js", COUNTING_PROVIDER)], None);
    let diagnostics = on_worker(|| {
        // On one instance: `register` runs for the first registry (`wc.p0`),
        // then for the second (`wc.p1`); the first registry's run rehearses
        // its configuration again, which declares `wc.p2`.
        let first = registry_with([built(&loader, "wc", "bib", json!({ "x": "1" }))]);
        let _second = registry_with([built(&loader, "wc", "bib", json!({ "x": "2" }))]);
        text_of(&first, "A \\cite{k}.").1
    });
    let detail = "flm.ext.handler-failed: the citation provider `wc.p0` of the feature `bib` of the package `_` of the \
                  extension `wc` failed: the citation provider `wc.p0` must be contributed by `register` of the feature \
                  `bib` of the package `_` of the extension `wc`";
    assert_eq!(
        diagnostics.first().map(String::as_str),
        Some(format!("flm.citations.provider-failed: the bibliography entry of the key `k` could not be retrieved: {detail}").as_str())
    );
}

#[test]
fn a_provider_of_the_wrong_shape_or_given_to_the_wrong_slot_is_refused() {
    let register = |body: &str| {
        let main = format!(
            "import {{ PROVIDERS_SLOT }} from \"flm:stdcore/features/citations\";\n\
             export default function (ext) {{ ext.package(\"_\", {{ features: {{ bib: {{ register(b) {{ {body} }} }} }} }}); }}\n"
        );
        build_error([built(&loader(&[("main.js", main.as_str())], None), "wc", "bib", Value::Null)])
    };
    let cases = [
        (
            "b.peer_slot(PROVIDERS_SLOT, { answer() {} });",
            "a citation provider contributed to `flm.citations.providers` by the package `_` of the extension `wc` must \
             be an object with `name` and `answer` or `answer_all`",
        ),
        (
            "b.peer_slot(PROVIDERS_SLOT, { name: \"wc.x\", render() {} });",
            "a citation provider contributed to `flm.citations.providers` by the package `_` of the extension `wc` must \
             be an object with `name` and `answer` or `answer_all`",
        ),
        // The slot's name decides (S22): any object given to the providers'
        // slot is a citation provider, one without its methods refused.
        (
            "b.peer_slot(PROVIDERS_SLOT, { name: \"wc.x\" });",
            "a citation provider contributed to `flm.citations.providers` by the package `_` of the extension `wc` must \
             be an object with `name` and `answer` or `answer_all`",
        ),
        (
            "b.peer_slot(PROVIDERS_SLOT, \"wc.x\");",
            "the service slot `flm.citations.providers` takes a citation provider (an object with `name` and `answer` \
             or `answer_all`), and the package `_` of the extension `wc` contributed data",
        ),
        (
            "b.service_slot(\"wc.notes\", \"single\", \"\"); b.contribute_to(\"wc.notes\", { name: \"wc.x\", answer() {} });",
            "the service slot `wc.notes` takes data, and the package `_` of the extension `wc` contributed an object \
             with methods",
        ),
        (
            "b.service_slot(\"wc.notes\", \"single\", \"\"); b.contribute_to(\"wc.notes\", { note: \"x\", show() {} });",
            "the service slot `wc.notes` takes data, and the package `_` of the extension `wc` contributed an object \
             with methods",
        ),
        // A second provider under one name in one `register` (S23).
        (
            "const p = { name: \"wc.x\", answer() { return null; } }; b.peer_slot(PROVIDERS_SLOT, p); \
             b.peer_slot(PROVIDERS_SLOT, { ...p });",
            "the citation provider `wc.x` must be contributed once by `register` of the feature `bib` of the package \
             `_` of the extension `wc`",
        ),
        (
            "b.contribute_first_to(PROVIDERS_SLOT, { name: \"wc.x\", answer() {} });",
            "a citation provider or a template module is contributed with `contribute_to` or `peer_slot`",
        ),
    ];
    for (body, text) in cases {
        let error = register(body);
        assert!(error.contains(text), "{body}: {error}");
    }
}

// --- the name rule -----------------------------------------------------------------------------

#[test]
fn a_service_slot_and_a_citation_provider_are_named_inside_the_extension() {
    let register = |body: &str| {
        let main = format!(
            "import {{ PROVIDERS_SLOT }} from \"flm:stdcore/features/citations\";\n\
             export default function (ext) {{ ext.package(\"_\", {{ features: {{ bib: {{ register(b) {{ {body} }} }} }} }}); }}\n"
        );
        build_error([built(&loader(&[("main.js", main.as_str())], None), "wc", "bib", Value::Null)])
    };
    let slot = register("b.service_slot(\"notes\", \"ordered-many\", \"\");");
    assert!(slot.contains("flm.ext.name-outside-extension"), "{slot}");
    assert!(
        slot.contains("the service slot `notes` declared by the package `_` of the extension `wc` must begin with `wc.`"),
        "{slot}"
    );
    let provider = register("b.peer_slot(PROVIDERS_SLOT, { name: \"bib.x\", answer() { return null; } });");
    assert!(provider.contains("flm.ext.name-outside-extension"), "{provider}");
    assert!(
        provider.contains(
            "the citation provider `bib.x` declared by the package `_` of the extension `wc` must begin with `wc.`"
        ),
        "{provider}"
    );
}

// --- template modules --------------------------------------------------------------------------

/// Names the page `job.EXT` and every other file `job-NAME.EXT`.
struct JobNamer;

impl ArtifactNamer for JobNamer {
    fn url_of(&self, artifact: &ArtifactRef<'_>, _scope: &NamingScope) -> RelPath {
        let extension = artifact.extension.unwrap_or("bin");
        if artifact.entry {
            RelPath::new(format!("job.{extension}"))
        } else {
            RelPath::new(format!("job-{}.{extension}", artifact.id.name))
        }
    }
}

/// The extension `wc`: the template module `tiny` (outside the stem rule, as
/// a compose template's name is), its heading from the feature's
/// configuration; it ships a stylesheet it computes, the extension's
/// `logo.svg` and, when its settings name one, a banner from the document's
/// folder; its page holds the extension's partial `head.html` and the
/// document's `note.txt`.
const TINY: &str = r#"
import { TEMPLATES_SLOT } from "flm:stdcore/features/compose";

const tiny = {
  name: "tiny",
  describe(settings) {
    return { output: "html", accepts: "all", requires: [], config: { heading: settings.heading ?? "Tiny", color: "teal", banner: null } };
  },
  files(data, cx) {
    const files = [
      { name: "style", extension: "css", media_type: "text/css", text: `h1 { color: ${data.config.color}; }\n` },
      { name: "logo", file: "logo.svg", scope: "extension" },
    ];
    if (data.config.banner !== null) files.push({ name: "banner", file: data.config.banner, scope: "document" });
    return files;
  },
  render(data, cx) {
    const files = cx.host("files");
    const head = files.read_text("extension", "head.html") ?? "";
    const note = files.read_text("document", "note.txt") ?? "";
    const paths = Object.fromEntries(data.artifacts);
    const regions = Object.fromEntries(data.regions);
    return `${head}<h1>${data.config.heading}</h1>\n${note}style=${paths.style};logo=${paths.logo};banner=${paths.banner}\n${regions.content}`;
  },
};

export default function (ext) {
  ext.package("_", { host: ["files"], features: { tinytpl: {
    settings: { config: { docs: "The heading.", schema: { type: "object", properties: { heading: { type: "string" } } } } },
    register(b, settings) { b.peer_slot(TEMPLATES_SLOT, tiny); },
  } } });
}
"#;

/// The registry of the standard package with the composer, `package`, and
/// the tests' naming of files.
fn try_compose_registry(package: PackageEntry) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    let mut standard = stdcore::package(StdcoreConfig::new());
    standard.features.push(Arc::new(ComposeFeature::new()) as Arc<dyn FlmFeature>);
    let mut builder = RegistryBuilder::new();
    builder.use_package(standard);
    builder.use_package(flm_core::math_passthrough::package());
    builder.use_package(package);
    builder.contribute_to(NAMER_SLOT, Arc::new(JobNamer) as Arc<dyn ArtifactNamer>);
    builder.build(&RegistryInputs::new())
}

/// The registry holding the template module `tiny`, its heading `Notes`.
fn tiny_registry() -> Arc<FlmRegistry> {
    let service = the_document_s_folder();
    let files = [("main.js", TINY), ("head.html", "<meta charset=\"utf-8\">\n"), ("logo.svg", "<svg/>")];
    try_compose_registry(built(&loader(&files, Some(&service)), "wc", "tinytpl", json!({ "heading": "Notes" })))
        .expect("the registry builds")
}

/// Renders a paragraph to HTML through the template `template`, its
/// settings `config`, sequential.
fn render(registry: &Arc<FlmRegistry>, template: &str, config: Value) -> Run {
    let Value::Object(config) = config else { panic!("a map") };
    let options = ComposeOptions {
        template: TemplateChoice::Name(template.to_string()),
        template_config: [("html".to_string(), [(template.to_string(), config)].into_iter().collect())].into_iter().collect(),
    };
    let overrides = Overrides {
        prefer: vec![stage_name("html").into()],
        render_options: OptionValues::new().with(options),
        ..Overrides::none()
    };
    run_with(registry, html::WORKFLOW, &overrides, "A paragraph.\n")
}

/// The bytes of the file `compose:NAME` of a run's HTML output, as text.
fn shipped(run: &Run, name: &str) -> Option<String> {
    let set = run.table(html::DOCUMENT).expect("declared")?;
    let artifact = set.get(&ArtifactId::new("compose", name))?;
    Some(String::from_utf8_lossy(artifact.as_bytes()?).into_owned())
}

/// The page of a run's HTML output.
fn page(run: &Run) -> Option<String> {
    let set = run.table(html::DOCUMENT).expect("declared")?;
    Some(String::from_utf8_lossy(set.entry()?.as_bytes()?).into_owned())
}

/// The page `tiny` renders with the banner `banner.png`.
const TINY_PAGE: &str = "<meta charset=\"utf-8\">\n<h1>Notes</h1>\na note\nstyle=job-style.css;logo=job-logo.svg;\
                         banner=job-banner.png\n<div id=\"flm-region-content\">\nA paragraph. \n</div>";

/// The page, the shipped files and the diagnostics of a render of `tiny`.
fn tiny_render(registry: &Arc<FlmRegistry>) -> (Option<String>, [Option<String>; 3], Vec<String>) {
    let run = render(registry, "tiny", json!({ "banner": "banner.png" }));
    (page(&run), [shipped(&run, "style"), shipped(&run, "logo"), shipped(&run, "banner")], common::diagnostics(&run))
}

#[test]
fn a_template_module_describes_itself_ships_its_files_and_renders() {
    let registry = tiny_registry();
    let template = registry
        .services()
        .services_named::<flm_core::stdcore::features::compose::Template>(flm_core::stdcore::features::compose::TEMPLATES_SLOT)
        .find(|template| template.name == "tiny")
        .expect("the template");
    assert_eq!((template.package.as_str(), template.engine.as_str()), ("wc#_", "wc#_:tiny"));
    assert_eq!(template.config["heading"], json!("Notes"));
    let (page, files, diagnostics) = on_worker(|| tiny_render(&registry));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(page.as_deref(), Some(TINY_PAGE));
    assert_eq!(files, [Some("h1 { color: teal; }\n".to_string()), Some("<svg/>".to_string()), Some("PNG".to_string())]);
}

#[test]
fn the_rehearsal_on_another_thread_finds_the_template_module_by_name() {
    let registry = tiny_registry();
    let (page, files, diagnostics) = on_other_thread(|| tiny_render(&registry));
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(page.as_deref(), Some(TINY_PAGE));
    assert_eq!(files[0].as_deref(), Some("h1 { color: teal; }\n"));
}

/// The extension `wc` whose feature contributes a template module named
/// after how many times `register` ran on the engine instance (`t0`, `t1`,
/// …), with a configuration `x`.
const COUNTING_TEMPLATE: &str = r#"
import { TEMPLATES_SLOT } from "flm:stdcore/features/compose";

let runs = 0;

export default function (ext) {
  ext.package("_", { features: { tinytpl: {
    settings: { config: { docs: "x", schema: { type: "object", properties: { x: { type: "string" } } } } },
    register(b) {
      b.peer_slot(TEMPLATES_SLOT, {
        name: `t${runs++}`,
        describe() { return { output: "html", accepts: "all" }; },
        render(data) { return "page"; },
      });
    },
  } } });
}
"#;

#[test]
fn a_template_module_a_second_run_of_register_does_not_declare_fails_the_template() {
    let loader = loader(&[("main.js", COUNTING_TEMPLATE)], None);
    let diagnostics = on_worker(|| {
        let first = try_compose_registry(built(&loader, "wc", "tinytpl", json!({ "x": "1" }))).expect("the registry");
        let _second = try_compose_registry(built(&loader, "wc", "tinytpl", json!({ "x": "2" }))).expect("the registry");
        common::diagnostics(&render(&first, "t0", json!({})))
    });
    assert_eq!(
        diagnostics,
        ["flm.compose.template-failed: the compose template `t0` for the output `html` cannot be applied: \
          flm.ext.handler-failed: the template module `t0` of the feature `tinytpl` of the package `_` of the extension \
          `wc` failed: the template module `t0` must be contributed by `register` of the feature `tinytpl` of the \
          package `_` of the extension `wc`"]
    );
}

#[test]
fn a_template_module_of_the_wrong_shape_is_refused() {
    let main = "import { TEMPLATES_SLOT } from \"flm:stdcore/features/compose\";\n\
                export default function (ext) { ext.package(\"_\", { features: { tinytpl: { register(b) {\n\
                b.peer_slot(TEMPLATES_SLOT, { name: \"tiny\", render() { return \"\"; } }); } } } }); }\n";
    let error = match try_compose_registry(built(&loader(&[("main.js", main)], None), "wc", "tinytpl", Value::Null)) {
        Ok(_) => panic!("the registry builds"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains(
            "a template module contributed to `flm.compose.templates` by the package `_` of the extension `wc` must be \
             an object with `name`, `describe` and `render`"
        ),
        "{error}"
    );
}

#[test]
fn a_second_template_module_under_one_name_is_refused() {
    let module = "{ name: \"tiny\", describe() { return { output: \"html\", accepts: \"all\" }; }, render() { return \"\"; } }";
    let main = format!(
        "import {{ TEMPLATES_SLOT }} from \"flm:stdcore/features/compose\";\n\
         export default function (ext) {{ ext.package(\"_\", {{ features: {{ tinytpl: {{ register(b) {{\n\
         b.peer_slot(TEMPLATES_SLOT, {module}); b.peer_slot(TEMPLATES_SLOT, {module}); }} }} }} }}); }}\n"
    );
    let loader = loader(&[("main.js", main.as_str())], None);
    let error = match try_compose_registry(built(&loader, "wc", "tinytpl", Value::Null)) {
        Ok(_) => panic!("the registry builds"),
        Err(error) => error.to_string(),
    };
    assert!(
        error.contains(
            "the template module `tiny` must be contributed once by `register` of the feature `tinytpl` of the \
             package `_` of the extension `wc`"
        ),
        "{error}"
    );
}

#[test]
fn a_template_module_s_page_must_be_a_text() {
    let main = "import { TEMPLATES_SLOT } from \"flm:stdcore/features/compose\";\n\
                export default function (ext) { ext.package(\"_\", { features: { tinytpl: { register(b) {\n\
                b.peer_slot(TEMPLATES_SLOT, { name: \"tiny\", describe() { return { output: \"html\", accepts: \"all\" }; }, \
                render() { return 7; } }); } } } }); }\n";
    let registry =
        try_compose_registry(built(&loader(&[("main.js", main)], None), "wc", "tinytpl", Value::Null)).expect("the registry");
    let diagnostics = on_worker(|| common::diagnostics(&render(&registry, "tiny", json!({}))));
    assert_eq!(
        diagnostics,
        ["flm.compose.template-failed: the compose template `tiny` for the output `html` cannot be applied: \
          flm.ext.handler-failed: the template module `tiny` of the feature `tinytpl` of the package `_` of the \
          extension `wc` failed: the answer of `render` must be a text"]
    );
}

/// A `register` that catches an exhausted allocation, contributes a
/// template module (whose `describe` runs as a nested call on the same
/// instance) and rethrows what it caught.
const CAUGHT_THEN_NESTED: &str = r#"
import { TEMPLATES_SLOT } from "flm:stdcore/features/compose";

export default function (ext) {
  ext.package("_", { features: { tinytpl: { register(b) {
    let caught = "nothing";
    let a = [];
    try { for (;;) a.push(new Array(100000)); } catch (e) { caught = e; }
    a = null;
    b.peer_slot(TEMPLATES_SLOT, {
      name: "tiny",
      describe() { return { output: "html", accepts: "all" }; },
      render() { return ""; },
    });
    throw caught;
  } } } });
}
"#;

#[test]
fn a_nested_call_keeps_the_memory_limit_the_outer_call_reached() {
    let limit = 4usize << 20;
    let source = js_extension("wc", &[("main.js", CAUGHT_THEN_NESTED)], js_manifest("main.js"));
    let settings = JsSettings { memory_limit: Some(limit), ..JsSettings::default() };
    let loader = Arc::new(ExtensionLoader::new(vec![Arc::new(source)]).with_js(JsHost::new(settings)));
    let error = match try_compose_registry(built(&loader, "wc", "tinytpl", Value::Null)) {
        Ok(_) => panic!("the registry builds"),
        Err(error) => error.to_string(),
    };
    let expected = format!(
        "`register` of the feature `tinytpl` of the package `_` of the extension `wc` must stay within the memory \
         limit of {limit} bytes"
    );
    assert!(error.contains(&expected), "{error}");
}

// --- the package object's `host` --------------------------------------------------------------

#[test]
fn host_must_be_a_list_of_texts() {
    let main = "export default function (ext) { ext.package(\"_\", { host: \"files\" }); }";
    let problems = load_problems(&loader(&[("main.js", main)], None), "wc");
    assert!(
        problems.iter().any(|problem| problem.contains("`host` of the package `_` of the extension `wc` must be a list of texts")),
        "{problems:?}"
    );
}

/// The registry digest of the extension `wc` whose package declares `host`,
/// the host offering `files`.
fn digest_declaring(host: &str) -> String {
    let main = format!(
        "export default function (ext) {{ ext.package(\"_\", {{ host: {host}, features: {{ f: {{ register(b) {{}} }} }} }}); }}"
    );
    let service = the_document_s_folder();
    let loader = loader(&[("main.js", main.as_str())], Some(&service));
    let registry = registry_with([built(&loader, "wc", "f", Value::Null)]);
    format!("{:?}", registry.digest())
}

#[test]
fn the_host_list_reaches_the_report_and_the_digest() {
    // The report holds the list: a service the host lacks fails the load.
    let main = "export default function (ext) { ext.package(\"_\", { host: [\"files\", \"programs\"] }); }";
    let service = the_document_s_folder();
    assert_eq!(
        load_problems(&loader(&[("main.js", main)], Some(&service)), "wc"),
        ["the package `_` of the extension `wc` declares the host service `programs` (`host`), which this program does \
          not offer; it offers `files`"]
    );
    // The declaration enters the digest.
    assert_eq!(digest_declaring("[\"files\"]"), digest_declaring("[\"files\"]"));
    assert_ne!(digest_declaring("[\"files\"]"), digest_declaring("[]"));
}

// --- the S15 data rule -------------------------------------------------------------------------

#[test]
fn a_decimal_number_in_a_template_s_data_is_named_by_its_key_path() {
    let registry = tiny_registry();
    let diagnostics = on_worker(|| common::diagnostics(&render(&registry, "tiny", json!({ "ratio": 1.5 }))));
    assert_eq!(
        diagnostics,
        ["flm.compose.template-failed: the compose template `tiny` for the output `html` cannot be applied: \
          flm.ext.handler-failed: the template module `tiny` of the feature `tinytpl` of the package `_` of the \
          extension `wc` failed: `config.ratio` of the data of a compose template must be an integer, not `1.5`"]
    );
}
