//! The fixture `tests/js/tinytpl/` (E6, plan section 7.2): a compose
//! template written as a JavaScript module (`templates/html/tiny.js`), its
//! heading from the feature's configuration, shipping a stylesheet it
//! computes and, when the template's settings name one, a banner copied from
//! the document's folder, and reading the partial `templates/html/head.html`
//! from the extension's folder. Loaded through the js engine with a `files`
//! service over memory ([`MemoryHostFiles`]), in a registry with the composer
//! (registered as `flm-main`'s standard package registers it) and a naming of
//! files after the job `tiny`; the template chosen and its settings given
//! through the render option [`ComposeOptions`]. The page and the stylesheet
//! are goldens; a module whose `render` throws (D14) and a banner that is not
//! there (D7) are reported by the composer.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;

use flm_core::core::artifact::{ArtifactId, ArtifactNamer, ArtifactRef, NamingScope, RelPath};
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{FlmFeature, FlmRegistry, PackageEntry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::html;
use flm_core::stdcore::backends::lowering::NAMER_SLOT;
use flm_core::stdcore::features::compose::{stage_name, ComposeFeature, ComposeOptions, Template, TemplateChoice, TEMPLATES_SLOT};
use flm_core::stdcore::features::meta::{DocMeta, MetaText};
use flm_core::stdcore::{self, StdcoreConfig};
use flm_ext::host::HostService;
use flm_ext::quickjs::{JsHost, JsSettings};
use flm_ext::{ExtensionLoader, MemoryFiles};
use serde_json::{json, Value};

use common::{factory_of, MemoryHostFiles};

// --- helpers -----------------------------------------------------------------------------------

/// The fixture's template module, as `tests/js/tinytpl/templates/html/tiny.js`
/// holds it.
const TINY_JS: &str = include_str!("js/tinytpl/templates/html/tiny.js");

/// The fixture's files other than the template module, as `tests/js/tinytpl/`
/// holds them (its manifest is [`manifest`], the JSON equal to
/// `flmextension.yaml`).
const TINYTPL_FILES: [(&str, &str); 2] = [
    ("main.js", include_str!("js/tinytpl/main.js")),
    ("templates/html/head.html", include_str!("js/tinytpl/templates/html/head.html")),
];

/// The fixture's manifest, `tests/js/tinytpl/flmextension.yaml` as JSON.
fn manifest() -> Value {
    json!({ "extension": {
        "description": "A tiny page written as a JavaScript module",
        "engine": "js",
        "js": { "main": "main.js" },
    } })
}

/// A PNG image of one gray pixel, the banner of the document's folder (the
/// bytes `flm-main/tests/watch.rs`'s `png(1)` writes, here as data, since
/// this crate has no PNG encoder).
const LOGO_PNG: [u8; 67] = [
    0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, 0x00, 0x00, 0x00,
    0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x00, 0x00, 0x00, 0x00, 0x3a, 0x7e, 0x9b, 0x55, 0x00, 0x00, 0x00, 0x0a, 0x49,
    0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0x68, 0x00, 0x00, 0x00, 0x82, 0x00, 0x81, 0x77, 0xcd, 0x72, 0xb6, 0x00, 0x00,
    0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
];

/// The document's body (its title `Tiny notes` is set as the document's
/// metadata, [`render`]).
const DOCUMENT: &str = "A paragraph of \\emph{tiny} notes.\n";

/// The golden file `name` under `tests/goldens/`.
fn golden(name: &str) -> String {
    format!("{}/tests/goldens/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// Names files as the `flm` program does for the job `tiny`
/// (`flm-main/src/output.rs`): the page `tiny.EXT`, every other file
/// `tiny-NAME.EXT`.
struct TinyNamer;

impl ArtifactNamer for TinyNamer {
    fn url_of(&self, artifact: &ArtifactRef<'_>, _scope: &NamingScope) -> RelPath {
        let extension = artifact.extension.unwrap_or("bin");
        if artifact.entry {
            RelPath::new(format!("tiny.{extension}"))
        } else {
            RelPath::new(format!("tiny-{}.{extension}", artifact.id.name))
        }
    }
}

/// The document's folder: `logo.png`.
fn folder() -> Arc<MemoryHostFiles> {
    Arc::new(MemoryHostFiles::new(MemoryFiles::new().with_bytes("logo.png", &LOGO_PNG)))
}

/// The package `_` of the fixture, its template module's text `tiny_js`,
/// built with the feature's configuration `config`, the document's folder
/// `service`.
fn package(service: &Arc<MemoryHostFiles>, tiny_js: &str, config: Value) -> PackageEntry {
    let files = TINYTPL_FILES
        .iter()
        .fold(MemoryFiles::new(), |held, (path, text)| held.with_text(path, text))
        .with_text("templates/html/tiny.js", tiny_js);
    let source = flm_ext::MemorySource::new().with("tinytpl", "tinytpl", manifest(), files);
    let loader = Arc::new(
        ExtensionLoader::new(vec![Arc::new(source)])
            .with_js(JsHost::new(JsSettings::default()))
            .with_host_service(Arc::clone(service) as Arc<dyn HostService>),
    );
    let factory = factory_of(&loader, "tinytpl");
    let mut settings = PackageSettings::new();
    if let Value::Object(config) = config {
        let decl = factory.settings("tinytpl").expect("the feature declares settings");
        settings.insert(decl.read(&config).expect("the settings read"));
    }
    factory.build(&settings).expect("the package builds")
}

/// The registry of the standard package with the composer (as `flm-main`'s
/// standard package adds it), `package`, and [`TinyNamer`].
fn registry(package: PackageEntry) -> Arc<FlmRegistry> {
    let mut standard = stdcore::package(StdcoreConfig::new());
    standard.features.push(Arc::new(ComposeFeature::new()) as Arc<dyn FlmFeature>);
    let mut builder = RegistryBuilder::new();
    builder.use_package(standard);
    builder.use_package(flm_core::math_passthrough::package());
    builder.use_package(package);
    builder.contribute_to(NAMER_SLOT, Arc::new(TinyNamer) as Arc<dyn ArtifactNamer>);
    builder.build(&RegistryInputs::new()).expect("the registry builds")
}

/// The registry of the fixture as written, its heading `Notes`.
fn tiny_registry(service: &Arc<MemoryHostFiles>) -> Arc<FlmRegistry> {
    registry(package(service, TINY_JS, json!({ "heading": "Notes" })))
}

/// `f` run on a thread marked as a worker of `flm-core`.
fn on_worker<T: Send>(f: impl FnOnce() -> T + Send) -> T {
    flm_core::lang::guard::run_with_worker_stack(f).expect("the worker thread starts")
}

/// The HTML run of [`DOCUMENT`] titled `Tiny notes`, the template `tiny`
/// chosen with the settings `config` through the render option
/// [`ComposeOptions`] (what `-t tiny` and the front matter's `features:
/// { compose: { template_config: { tiny: … } } }` set).
fn render(registry: &Arc<FlmRegistry>, config: Value) -> Run {
    let Value::Object(config) = config else { panic!("a map") };
    let options = ComposeOptions {
        template: TemplateChoice::Name("tiny".to_string()),
        template_config: [("html".to_string(), [("tiny".to_string(), config)].into_iter().collect())].into_iter().collect(),
    };
    let overrides = Overrides {
        prefer: vec![stage_name("html").into()],
        render_options: OptionValues::new().with(options),
        ..Overrides::none()
    };
    let source = Arc::new(FlmSource::new(DOCUMENT).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let meta = DocMeta::default().title(MetaText::new("Tiny notes"));
    let plan = Arc::new(Plan::for_workflow(registry, html::WORKFLOW, &overrides).expect("a plan"));
    let doc = Doc::with_options(content, OptionValues::new().with(meta));
    let mut run = Run::new(Arc::new(doc), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
}

/// The page of a run's HTML output.
fn page(run: &Run) -> Option<String> {
    let set = run.table(html::DOCUMENT).expect("declared")?;
    Some(String::from_utf8_lossy(set.entry()?.as_bytes()?).into_owned())
}

/// The bytes of the shipped file `compose:NAME` of a run's HTML output.
fn shipped(run: &Run, name: &str) -> Option<Vec<u8>> {
    let set = run.table(html::DOCUMENT).expect("declared")?;
    Some(set.get(&ArtifactId::new("compose", name))?.as_bytes()?.to_vec())
}

/// The names of the files of a run's HTML output other than the page, in
/// the set's order (by identity).
fn shipped_names(run: &Run) -> Vec<String> {
    let Some(set) = run.table(html::DOCUMENT).expect("declared") else { return Vec::new() };
    let entry = set.entry().map(|entry| entry.id.clone());
    set.iter().filter(|artifact| Some(&artifact.id) != entry.as_ref()).map(|artifact| artifact.id.to_string()).collect()
}

// --- the page of section 7.2 -------------------------------------------------------------------

#[test]
fn the_template_module_is_contributed_with_its_description() {
    let registry = tiny_registry(&folder());
    let template = registry
        .services()
        .services_named::<Template>(TEMPLATES_SLOT)
        .find(|template| template.name == "tiny")
        .expect("the template `tiny`");
    assert_eq!(
        (template.package.as_str(), template.output.as_str(), template.engine.as_str()),
        ("tinytpl#_", "html", "tinytpl#_:tiny")
    );
    assert_eq!(Value::Object(template.config.clone()), json!({ "heading": "Notes", "color": "teal", "banner": null }));
}

#[test]
fn the_page_has_its_heading_its_stylesheet_its_banner_and_the_partial() {
    let service = folder();
    let registry = tiny_registry(&service);
    let run = on_worker(|| render(&registry, json!({ "banner": "logo.png" })));
    let diagnostics = common::diagnostics(&run);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let page = page(&run).expect("the page");
    assert_text_golden(golden("tinytpl.html"), &page);
    // The heading from the configuration, the partial's line, the two files
    // named after the job beside the page.
    assert!(page.contains("<h1>Notes</h1>\n"), "{page}");
    assert!(page.contains("<head>\n<meta charset=\"utf-8\">\n<title>"), "{page}");
    assert!(page.contains("<link rel=\"stylesheet\" href=\"tiny-style.css\">"), "{page}");
    assert!(page.contains("<img src=\"tiny-banner.png\" alt=\"\">"), "{page}");
    // The stylesheet the module computes; the banner copied from the
    // document's folder.
    let style = String::from_utf8(shipped(&run, "style").expect("the stylesheet")).expect("text");
    assert_text_golden(golden("tinytpl-style.css"), &style);
    assert_eq!(shipped(&run, "banner").as_deref(), Some(&LOGO_PNG[..]));
    assert_eq!(shipped_names(&run), ["compose:banner", "compose:style"]);
    // The banner was read from the document's folder through the host.
    let calls = service.calls.lock().expect("the calls");
    let seen: Vec<(&str, &str, &Value)> =
        calls.iter().map(|(caller, method, args)| (caller.package.as_str(), method.as_str(), args)).collect();
    assert_eq!(seen, [("_", "read_bytes", &json!({ "scope": "document", "path": "logo.png" }))]);
}

#[test]
fn without_a_banner_and_a_heading_the_page_has_the_defaults() {
    let registry = registry(package(&folder(), TINY_JS, Value::Null));
    let run = on_worker(|| render(&registry, json!({})));
    let diagnostics = common::diagnostics(&run);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    let page = page(&run).expect("the page");
    assert!(page.contains("<h1>Tiny</h1>\n<div"), "{page}");
    assert!(!page.contains("<img"), "{page}");
    assert_eq!(shipped_names(&run), ["compose:style"]);
}

// --- failures ----------------------------------------------------------------------------------

#[test]
fn a_template_module_whose_render_throws_fails_the_template() {
    let throwing = TINY_JS.replacen("render(data, cx) {", "render(data, cx) {\n    throw new Error(\"no page today\");", 1);
    assert_ne!(throwing, TINY_JS);
    let registry = registry(package(&folder(), &throwing, json!({ "heading": "Notes" })));
    let run = on_worker(|| render(&registry, json!({})));
    assert_eq!(
        common::diagnostics(&run),
        ["flm.compose.template-failed: the compose template `tiny` for the output `html` cannot be applied: \
          flm.ext.handler-failed: the template module `tiny` of the feature `tinytpl` of the package `_` of the \
          extension `tinytpl` failed: no page today"]
    );
    assert_eq!(page(&run), None);
}

#[test]
fn a_banner_that_is_not_in_the_document_s_folder_fails_the_template() {
    let registry = tiny_registry(&folder());
    let run = on_worker(|| render(&registry, json!({ "banner": "missing.png" })));
    assert_eq!(
        common::diagnostics(&run),
        ["flm.compose.template-failed: the compose template `tiny` for the output `html` cannot be applied: \
          flm.ext.handler-failed: the template module `tiny` of the feature `tinytpl` of the package `_` of the \
          extension `tinytpl` failed: the file `missing.png` it ships from the document's \
          folder is not there"]
    );
}
