//! The composer's question to the template engine for the files a compose template ships
//! (`TemplateEngine::artifacts`): an engine that answers the compose template's own files
//! plus one it computes when the template is applied, which the composer names through the
//! program's naming of files, ships beside the entry artifact and hands to `render` by its
//! path; an engine that cannot give its files, whose failure the composer reports; and the
//! method's default, which answers the compose template's own list.

mod common;

use std::sync::{Arc, Mutex};

use flm_core::core::artifact::{ArtifactId, ArtifactNamer};
use flm_core::core::plan::Overrides;
use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry, RegistryBuilder, RegistryInputs};
use flm_core::core::run::Run;
use flm_core::model::options::OptionValues;
use flm_core::stdcore::backends::html;
use flm_core::stdcore::backends::lowering::NAMER_SLOT;
use flm_core::stdcore::features::compose::{
    Accepts, ComposeFeature, ComposeOptions, ENGINES_SLOT, TEMPLATES_SLOT, Template, TemplateArtifact, TemplateChoice,
    TemplateContext, TemplateEngine, stage_name,
};
use flm_core::stdcore::{StdcoreConfig, package};
use serde_json::{Map, Value};
use techy::error::Severity;

use common::html::page as rendered_page;
use common::naming::JobNamer;
use common::stdcore::run_workflow;

/// What the engine `chooser` saw: the paths `artifacts` was called with, and those
/// `render` was called with, once per call.
#[derive(Default)]
struct Seen {
    asked: Vec<Vec<(String, String)>>,
    rendered: Vec<Vec<(String, String)>>,
}

/// A template engine whose compose templates choose their files when they are applied:
/// it answers the compose template's own files plus `output-name`, a text file holding
/// the name of the output the template is applied for, or fails with `fail` when given.
/// Its page lists the paths it is given as `name=path;`, then the content.
struct Chooser {
    seen: Arc<Mutex<Seen>>,
    fail: Option<&'static str>,
}

impl TemplateEngine for Chooser {
    fn name(&self) -> &str {
        "chooser"
    }

    fn artifacts(&self, template: &Template, context: &TemplateContext) -> Result<Vec<TemplateArtifact>, String> {
        self.seen.lock().unwrap().asked.push(context.artifacts.clone());
        if let Some(detail) = self.fail {
            return Err(String::from(detail));
        }
        let mut artifacts = template.artifacts.clone();
        let kind = if context.html { "HTML" } else { "not HTML" };
        artifacts.push(TemplateArtifact::File {
            name: String::from("output-name"),
            extension: String::from("txt"),
            media_type: String::from("text/plain"),
            apply: false,
            bytes: format!("{} ({kind})\n", template.output).into_bytes(),
        });
        Ok(artifacts)
    }

    fn render(&self, _template: &Template, context: &TemplateContext) -> Result<String, String> {
        self.seen.lock().unwrap().rendered.push(context.artifacts.clone());
        let mut page = String::new();
        for (name, path) in &context.artifacts {
            page.push_str(&format!("{name}={path};"));
        }
        let content = context.regions.iter().find(|(name, _)| name == "content").map(|(_, text)| text.as_str());
        page.push_str(content.unwrap_or_default());
        Ok(page)
    }
}

/// The compose template `chosen`, written for the engine `chooser`, which ships the
/// stylesheet `style` of its own.
fn chosen_template() -> Template {
    Template {
        name: String::from("chosen"),
        package: String::from("test"),
        output: String::from("html"),
        engine: String::from("chooser"),
        text: String::new(),
        accepts: Accepts::All,
        requires: Vec::new(),
        config: Map::new(),
        delimiters: None,
        artifacts: vec![TemplateArtifact::File {
            name: String::from("style"),
            extension: String::from("css"),
            media_type: String::from("text/css"),
            apply: false,
            bytes: b"p { margin: 0 }\n".to_vec(),
        }],
        files: None,
    }
}

/// A feature contributing the engine `chooser` and the compose template `chosen`.
struct Choosing {
    seen: Arc<Mutex<Seen>>,
    fail: Option<&'static str>,
}

impl FlmFeature for Choosing {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("test.choosing", "A compose template whose engine chooses its files")
            .docs("the template engine `chooser` and the compose template `chosen`")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let engine = Chooser { seen: Arc::clone(&self.seen), fail: self.fail };
        b.peer_slot::<dyn TemplateEngine>(ENGINES_SLOT, Arc::new(engine))?;
        b.peer_slot::<Template>(TEMPLATES_SLOT, Arc::new(chosen_template()))
    }
}

/// The registry of `stdcore` with the composer, the feature `choosing` and the tests'
/// naming of files.
fn registry(seen: &Arc<Mutex<Seen>>, fail: Option<&'static str>) -> Arc<FlmRegistry> {
    let mut entry = package(StdcoreConfig::new());
    entry.features.extend([
        Arc::new(ComposeFeature::new()) as Arc<dyn FlmFeature>,
        Arc::new(Choosing { seen: Arc::clone(seen), fail }),
    ]);
    let mut builder = RegistryBuilder::new();
    builder.use_package(entry);
    builder.use_package(flm_core::math_passthrough::package());
    builder.contribute_to(NAMER_SLOT, Arc::new(JobNamer) as Arc<dyn ArtifactNamer>);
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()))
}

/// Renders a paragraph to HTML through the compose template `chosen`.
fn render(registry: &Arc<FlmRegistry>) -> Run {
    let options = ComposeOptions { template: TemplateChoice::Name(String::from("chosen")), ..ComposeOptions::default() };
    let overrides = Overrides {
        prefer: vec![stage_name("html").into()],
        render_options: OptionValues::new().with(options),
        ..Overrides::none()
    };
    run_workflow(registry, html::WORKFLOW, &["A paragraph.\n"], &overrides)
}

/// A (name, path) pair.
fn pair(name: &str, path: &str) -> (String, String) {
    (String::from(name), String::from(path))
}

#[test]
fn the_composer_names_and_ships_the_files_the_engine_answers() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let run = render(&registry(&seen, None));
    assert!(
        run.diagnostics().iter().all(|d| d.severity() != Severity::Error),
        "{}",
        run.diagnostics().render_all()
    );

    // The engine was asked once, before any path was named; `render` then received the
    // path of each file, in the order the engine answered them (the page is rendered a
    // second time on HTML, for the composer's table, with the same paths).
    let seen = seen.lock().unwrap();
    assert_eq!(seen.asked, [Vec::<(String, String)>::new()]);
    let paths = vec![pair("style", "job-style.css"), pair("output-name", "job-output-name.txt")];
    assert!(!seen.rendered.is_empty());
    for rendered in &seen.rendered {
        assert_eq!(rendered, &paths);
    }
    let page = rendered_page(&run);
    assert!(
        page.starts_with("style=job-style.css;output-name=job-output-name.txt;<div id=\"flm-region-content\">"),
        "{page}"
    );

    // The computed file is shipped beside the entry artifact as the template's own is.
    let set = run.table(html::DOCUMENT).expect("declared").expect("written");
    let shipped = |name: &str| set.get(&ArtifactId::new("compose", name)).unwrap_or_else(|| panic!("compose:{name}"));
    let computed = shipped("output-name");
    assert_eq!(
        (computed.media_type.as_str(), computed.extension.as_deref(), computed.entry),
        ("text/plain", Some("txt"), false)
    );
    assert_eq!(computed.as_bytes(), Some(&b"html (HTML)\n"[..]));
    assert_eq!(shipped("style").as_bytes(), Some(&b"p { margin: 0 }\n"[..]));
    // The entry artifact links to both, in the engine's order.
    let refs: Vec<String> = set.entry().expect("the page").refs.iter().map(ToString::to_string).collect();
    let position = |name: &str| refs.iter().position(|r| r == name).unwrap_or_else(|| panic!("{name} in {refs:?}"));
    assert!(position("compose:style") < position("compose:output-name"), "{refs:?}");
}

#[test]
fn an_engine_that_cannot_give_its_files_fails_the_template() {
    let seen = Arc::new(Mutex::new(Seen::default()));
    let run = render(&registry(&seen, Some("the file `banner.png` cannot be read")));
    let failures: Vec<String> = run
        .diagnostics()
        .iter()
        .filter(|d| d.identifier() == "flm.compose.template-failed")
        .map(|d| d.message().to_string())
        .collect();
    assert_eq!(
        failures,
        ["the compose template `chosen` for the output `html` cannot be applied: the file `banner.png` cannot be read"]
    );
    // Nothing was rendered and the output is not made.
    assert!(seen.lock().unwrap().rendered.is_empty());
    assert!(run.table(html::DOCUMENT).expect("declared").is_none_or(|set| set.is_empty()));
}

/// A template engine that implements only `name` and `render`.
struct Plain;

impl TemplateEngine for Plain {
    fn name(&self) -> &str {
        "plain"
    }

    fn render(&self, template: &Template, _context: &TemplateContext) -> Result<String, String> {
        Ok(template.text.clone())
    }
}

#[test]
fn by_default_an_engine_answers_the_templates_own_files() {
    let template = chosen_template();
    let context = TemplateContext {
        html: true,
        regions: Vec::new(),
        pieces: Vec::new(),
        meta: Value::Object(Map::new()),
        config: Map::new(),
        artifacts: Vec::new(),
    };
    assert_eq!(Plain.artifacts(&template, &context), Ok(template.artifacts.clone()));
}
