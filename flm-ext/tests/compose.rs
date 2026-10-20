//! A compose template written as a module (E6, plan sections 2.5, 3.1 and
//! 3.3), through the Rust test double: the module describes itself from the
//! feature's configuration, the adapter contributes its template and its own
//! engine, the composer asks the module for the files it ships (a computed
//! one, one of the extension's folder, one of the document's folder) and
//! hands their paths to `render`; a failing module, a file not there, and a
//! description or a list of files of the wrong shape are reported.

mod common;

use std::sync::{Arc, Mutex};

use flm_core::core::artifact::{ArtifactId, ArtifactNamer, ArtifactRef, NamingScope, RelPath};
use flm_core::core::plan::{Overrides, Plan};
use flm_core::core::registry::{FlmFeature, FlmRegistry, PackageEntry, RegistryBuildError, RegistryBuilder, RegistryInputs};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::html;
use flm_core::stdcore::backends::lowering::NAMER_SLOT;
use flm_core::stdcore::features::compose::{ComposeFeature, ComposeOptions, TemplateChoice, TemplateEngine, ENGINES_SLOT as CORE_ENGINES_SLOT, stage_name};
use flm_core::stdcore::{self, StdcoreConfig};
use flm_ext::adapter::values::{handler_named, template_context_to_value, template_description_from_value, template_files_from_value};
use flm_ext::api::core::registry::Contributed;
use flm_ext::api::host::{FilesScope, HostHandle};
use flm_ext::api::stdcore::features::compose::{
    Accepts, TemplateContext, TemplateCx, TemplateDescription, TemplateFile, TemplateModule, TEMPLATES_SLOT,
};
use flm_ext::engine::{ConfigReport, SettingsReport};
use flm_ext::host::HostService;
use flm_ext::{ExtError, MemoryFiles};
use serde_json::{json, Value};

use common::double::Double;
use common::MemoryHostFiles;

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

/// What the module `tiny` was handed.
#[derive(Default)]
struct Seen {
    /// The settings `describe` received.
    described: Vec<Value>,
    /// `data` as a binding receives it, at `files` and at `render`.
    files: Vec<Value>,
    rendered: Vec<Value>,
}

/// The template module `tiny` of the plan's fixture, in Rust: a heading from
/// the configuration, a stylesheet it computes, a logo of the extension's
/// folder, a banner of the document's folder when the template's settings
/// name one, a partial read from the extension's folder. `fail` makes
/// `render` fail.
struct Tiny {
    seen: Arc<Mutex<Seen>>,
    fail: bool,
}

impl TemplateModule for Tiny {
    fn name(&self) -> String {
        "tiny".to_string()
    }

    fn describe(&self, settings: &Value) -> Result<TemplateDescription, ExtError> {
        self.seen.lock().expect("seen").described.push(settings.clone());
        let heading = settings.get("heading").cloned().unwrap_or_else(|| json!("Tiny"));
        let config = json!({ "heading": heading, "color": "teal", "banner": null });
        let Value::Object(config) = config else { unreachable!() };
        Ok(TemplateDescription { output: "html".to_string(), accepts: Accepts::All, requires: Vec::new(), config })
    }

    fn files(&self, data: &TemplateContext, _cx: &dyn TemplateCx) -> Result<Vec<TemplateFile>, ExtError> {
        self.seen.lock().expect("seen").files.push(template_context_to_value(data)?);
        let color = data.config["color"].as_str().unwrap_or_default();
        let mut files = vec![
            TemplateFile::Content {
                name: "style".to_string(),
                extension: "css".to_string(),
                media_type: "text/css".to_string(),
                text: format!("h1 {{ color: {color}; }}\n"),
            },
            TemplateFile::File {
                name: "logo".to_string(),
                path: "templates/html/logo.svg".to_string(),
                scope: FilesScope::Extension,
                media_type: None,
            },
        ];
        if let Some(banner) = data.config["banner"].as_str() {
            files.push(TemplateFile::File {
                name: "banner".to_string(),
                path: banner.to_string(),
                scope: FilesScope::Document,
                media_type: None,
            });
        }
        Ok(files)
    }

    fn render(&self, data: &TemplateContext, cx: &dyn TemplateCx) -> Result<String, ExtError> {
        self.seen.lock().expect("seen").rendered.push(template_context_to_value(data)?);
        if self.fail {
            return Err(ExtError::Handler { detail: "TypeError: regions is undefined".to_string(), data: Value::Null });
        }
        let HostHandle::Files(files) = cx.host("files")? else { panic!("`files` is the files handle") };
        let head = files.read_text(FilesScope::Extension, "templates/html/head.html")?.unwrap_or_default();
        let paths: Vec<String> = data.artifacts.iter().map(|(name, path)| format!("{name}={path}")).collect();
        let content = data.regions.iter().find(|(name, _)| name == "content").map(|(_, text)| text.as_str());
        Ok(format!("{head}<h1>{}</h1>\n{}\n{}\n", data.config["heading"].as_str().unwrap_or_default(), paths.join(";"), content.unwrap_or_default()))
    }
}

/// The extension's own files.
fn the_extension_s_files() -> MemoryFiles {
    MemoryFiles::new()
        .with_text("templates/html/head.html", "<meta charset=\"utf-8\">\n")
        .with_text("templates/html/logo.svg", "<svg/>")
}

/// The document's folder.
fn the_document_s_folder() -> Arc<MemoryHostFiles> {
    Arc::new(MemoryHostFiles::new(MemoryFiles::new().with_text("logo.png", "PNG")))
}

/// The package `tiny` (extension `tiny`) contributing `module` with
/// `peer_slot`, declaring and offered the `files` service, its feature
/// declaring a configuration.
fn tiny(module: Arc<dyn TemplateModule>) -> Double {
    let report = SettingsReport {
        config: Some(ConfigReport { docs: String::new(), schema: json!({ "type": "object", "properties": { "heading": {} } }) }),
        ..SettingsReport::default()
    };
    Double::new("tiny")
        .host("files")
        .offering(the_document_s_folder() as Arc<dyn HostService>)
        .files(the_extension_s_files())
        .configured("tiny", move |b, _| b.peer_slot(TEMPLATES_SLOT, Contributed::TemplateModule(Arc::clone(&module))))
        .settings("tiny", report)
}

/// The package built with the feature's configuration `config`.
fn built(double: Double, config: Value) -> PackageEntry {
    let factory = double.factory();
    let mut settings = PackageSettings::new();
    let Value::Object(config) = config else { panic!("a map") };
    let decl = factory.settings("tiny").expect("the feature declares settings");
    settings.insert(decl.read(&config).expect("the settings read"));
    factory.build(&settings).expect("the package builds")
}

/// The registry of the standard package with the composer, `package`, and
/// the tests' naming of files.
fn try_registry(package: PackageEntry) -> Result<Arc<FlmRegistry>, RegistryBuildError> {
    let mut standard = stdcore::package(StdcoreConfig::new());
    standard.features.push(Arc::new(ComposeFeature::new()) as Arc<dyn FlmFeature>);
    let mut builder = RegistryBuilder::new();
    builder.use_package(standard);
    builder.use_package(flm_core::math_passthrough::package());
    builder.use_package(package);
    builder.contribute_to(NAMER_SLOT, Arc::new(JobNamer) as Arc<dyn ArtifactNamer>);
    builder.build(&RegistryInputs::new())
}

/// Renders a paragraph to HTML through the template `tiny`, its settings
/// `config`.
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
    let source = Arc::new(FlmSource::new("A paragraph.\n").with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Arc::new(Plan::for_workflow(registry, html::WORKFLOW, &overrides).expect("a plan"));
    let mut run = Run::new(Arc::new(Doc::new(content)), plan, Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run
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

#[test]
fn a_template_module_describes_itself_ships_its_files_and_renders_with_their_paths() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let module = Arc::new(Tiny { seen: Arc::clone(&seen), fail: false });
    let registry = try_registry(built(tiny(module), json!({ "heading": "Notes" }))).expect("the registry builds");

    // One template and one engine of its own, named after the template's
    // package and name.
    let template = registry
        .services()
        .services_named::<flm_core::stdcore::features::compose::Template>(TEMPLATES_SLOT)
        .find(|template| template.name == "tiny")
        .expect("the template");
    assert_eq!((template.package.as_str(), template.engine.as_str()), ("tiny#_", "tiny#_:tiny"));
    assert_eq!(template.config["heading"], json!("Notes"));
    assert!(template.text.is_empty() && template.artifacts.is_empty() && template.files.is_none());
    let engines: Vec<String> =
        registry.services().services_named::<dyn TemplateEngine>(CORE_ENGINES_SLOT).map(|e| e.name().to_string()).collect();
    assert!(engines.contains(&"tiny#_:tiny".to_string()), "{engines:?}");

    let run = render(&registry, json!({ "banner": "logo.png" }));
    let diagnostics = common::diagnostics(&run);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        page(&run).expect("the page"),
        "<meta charset=\"utf-8\">\n<h1>Notes</h1>\nstyle=job-style.css;logo=job-logo.svg;banner=job-banner.png\n\
         <div id=\"flm-region-content\">\nA paragraph. \n</div>\n"
    );
    assert_eq!(shipped(&run, "style").as_deref(), Some("h1 { color: teal; }\n"));
    assert_eq!(shipped(&run, "logo").as_deref(), Some("<svg/>"));
    assert_eq!(shipped(&run, "banner").as_deref(), Some("PNG"));
    let set = run.table(html::DOCUMENT).expect("declared").expect("written");
    let media = |name: &str| set.get(&ArtifactId::new("compose", name)).map(|a| a.media_type.clone());
    assert_eq!(
        (media("style"), media("logo"), media("banner")),
        (Some("text/css".to_string()), Some("image/svg+xml".to_string()), Some("image/png".to_string()))
    );

    // The logo and the banner were read as bytes, in the two scopes of the
    // `files` service (ruling S12). `describe` received the configuration; `files` the data without the
    // paths, `render` with them, as a binding receives them.
    let seen = seen.lock().expect("seen");
    assert_eq!(seen.described, [json!({ "heading": "Notes" })]);
    let files = &seen.files[0];
    assert_eq!(files["artifacts"], json!([]));
    assert_eq!(files["config"], json!({ "heading": "Notes", "color": "teal", "banner": "logo.png" }));
    assert_eq!(files["html"], json!(true));
    let rendered = seen.rendered.last().expect("rendered");
    assert_eq!(
        rendered["artifacts"],
        json!([["style", "job-style.css"], ["logo", "job-logo.svg"], ["banner", "job-banner.png"]])
    );
    assert!(rendered["regions"].as_array().expect("a list").iter().any(|pair| pair[0] == "content"), "{rendered}");
    assert!(rendered["pieces"].is_array(), "{rendered}");
}

/// The Errors of a run through `module`, with the template's settings
/// `config`.
fn errors(module: Arc<dyn TemplateModule>, config: Value) -> Vec<String> {
    let registry = try_registry(built(tiny(module), json!({}))).expect("the registry builds");
    let run = render(&registry, config);
    common::diagnostics(&run)
}

#[test]
fn a_module_that_fails_fails_the_template() {
    let module = Arc::new(Tiny { seen: Arc::default(), fail: true });
    assert_eq!(
        errors(module, json!({})),
        ["flm.compose.template-failed: the compose template `tiny` for the output `html` cannot be applied: \
          flm.ext.handler-failed: the template module `tiny` of the feature `tiny` of the package `_` of the extension \
          `tiny` failed: TypeError: regions is undefined"]
    );
}

#[test]
fn a_shipped_file_that_is_not_there_fails_the_template() {
    let module = Arc::new(Tiny { seen: Arc::default(), fail: false });
    assert_eq!(
        errors(module, json!({ "banner": "absent.png" })),
        ["flm.compose.template-failed: the compose template `tiny` for the output `html` cannot be applied: \
          flm.ext.handler-failed: the template module `tiny` of the feature `tiny` of the package `_` of the extension \
          `tiny` failed: the file `absent.png` it ships from the document's folder is not there"]
    );
}

/// A module that speaks JSON, as a binding does: its description and its
/// files are read from data with the conversions of `values.rs`.
struct Speaking {
    description: Value,
    files: Value,
}

impl TemplateModule for Speaking {
    fn name(&self) -> String {
        "tiny".to_string()
    }

    fn describe(&self, _: &Value) -> Result<TemplateDescription, ExtError> {
        template_description_from_value(&self.description, &handler_named("template module", "tiny", "tiny", "_", "tiny"))
    }

    fn files(&self, _: &TemplateContext, _: &dyn TemplateCx) -> Result<Vec<TemplateFile>, ExtError> {
        template_files_from_value(&self.files)
    }

    fn render(&self, _: &TemplateContext, _: &dyn TemplateCx) -> Result<String, ExtError> {
        Ok("page".to_string())
    }
}

#[test]
fn a_description_of_the_wrong_shape_fails_the_registration() {
    for description in [json!({ "output": "html", "accepts": "some" }), json!({ "output": "html" }), json!({ "output": "html", "accepts": "all", "engine": "x" })] {
        let module = Arc::new(Speaking { description, files: json!([]) });
        let error = try_registry(built(tiny(module), json!({}))).expect_err("the build fails");
        assert_eq!(
            error.render(),
            "feature `tiny`: flm.ext.handler-failed: the description the template module `tiny` of the feature `tiny` of \
             the package `_` of the extension `tiny` answers must be `{ output, accepts, requires, config }`, with \
             `accepts` `all` or a list of piece names"
        );
    }
}

#[test]
fn a_list_of_files_of_the_wrong_shape_fails_the_template() {
    let description = json!({ "output": "html", "accepts": ["flm.toc"], "requires": [], "config": {} });
    let prefix = "flm.compose.template-failed: the compose template `tiny` for the output `html` cannot be applied: \
                  flm.ext.handler-failed: the template module `tiny` of the feature `tiny` of the package `_` of the \
                  extension `tiny` failed: ";
    let cases = [
        (
            json!([{ "name": "style", "text": "x" }]),
            "each file it ships must be `{ name, extension, media_type, text }` or `{ name, file, \
             scope, media_type }`",
        ),
        (json!([{ "name": "logo", "file": "logo.svg", "scope": "home" }]), "the scope of a file read must be `extension` or `document`, not `home`"),
        (
            json!([{ "name": "logo", "file": "templates/html/logo", "scope": "extension" }]),
            "the file `templates/html/logo` it ships from the extension's folder must have an extension",
        ),
        (
            json!([{ "name": "logo", "file": "absent.svg", "scope": "extension" }]),
            "the file `absent.svg` it ships from the extension's folder is not a file of the extension",
        ),
    ];
    for (files, detail) in cases {
        let module = Arc::new(Speaking { description: description.clone(), files });
        assert_eq!(errors(module, json!({})), [format!("{prefix}{detail}")]);
    }
}

#[test]
fn a_module_s_files_are_read_through_a_declared_files_service_alone() {
    let module: Arc<dyn TemplateModule> = Arc::new(Tiny { seen: Arc::default(), fail: false });
    let undeclared = Double::new("tiny")
        .files(the_extension_s_files())
        .feature("tiny", move |b| b.peer_slot(TEMPLATES_SLOT, Contributed::TemplateModule(Arc::clone(&module))));
    let registry = try_registry(undeclared.package()).expect("the registry builds");
    let run = render(&registry, json!({}));
    assert_eq!(
        common::diagnostics(&run),
        ["flm.compose.template-failed: the compose template `tiny` for the output `html` cannot be applied: \
          flm.ext.handler-failed: the template module `tiny` of the feature `tiny` of the package `_` of the extension \
          `tiny` failed: the package `_` of the extension `tiny` must declare the host service `files` in its `host` \
          list for the module to ship the file `templates/html/logo.svg` from the extension's folder"]
    );
}

#[test]
fn a_module_without_a_name_or_given_to_the_wrong_slot_is_refused() {
    struct Nameless;
    impl TemplateModule for Nameless {
        fn name(&self) -> String {
            String::new()
        }
        fn describe(&self, _: &Value) -> Result<TemplateDescription, ExtError> {
            unreachable!("refused before")
        }
        fn render(&self, _: &TemplateContext, _: &dyn TemplateCx) -> Result<String, ExtError> {
            unreachable!("refused before")
        }
    }
    let refusal = |slot: &'static str, module: Arc<dyn TemplateModule>| {
        let double = Double::new("tiny").feature("tiny", move |b| b.contribute_to(slot, Contributed::TemplateModule(Arc::clone(&module))));
        try_registry(double.package()).expect_err("the build fails").render()
    };
    assert_eq!(
        refusal(TEMPLATES_SLOT, Arc::new(Nameless)),
        "feature `tiny`: flm.ext.handler-failed: `contribute_to`: a template module contributed to \
         `flm.compose.templates` by the package `_` of the extension `tiny` must be an object with `name`, `describe` \
         and `render`"
    );
    assert_eq!(
        refusal("flm.citations.providers", Arc::new(Speaking { description: json!({}), files: json!([]) })),
        "feature `tiny`: flm.ext.handler-failed: `contribute_to`: the service slot `flm.citations.providers` takes a \
         citation provider (an object with `name` and `answer` or `answer_all`), and the package `_` of the extension \
         `tiny` contributed a template module"
    );
}

#[test]
fn the_registry_digest_follows_the_module_s_description() {
    let digest = |heading: &str| {
        let module = Arc::new(Tiny { seen: Arc::default(), fail: false });
        try_registry(built(tiny(module), json!({ "heading": heading }))).expect("the registry builds").digest()
    };
    assert_eq!(digest("A"), digest("A"));
    assert_ne!(digest("A"), digest("B"));
}
