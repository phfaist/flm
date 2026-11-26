//! The composer's question to the template engine for the files a compose template ships
//! (`TemplateEngine::artifacts`): an engine that answers the compose template's own files
//! plus one it computes when the template is applied, which the composer names through the
//! program's naming of files, ships beside the entry artifact and hands to `render` by its
//! path; an engine that cannot give its files, whose failure the composer reports; and the
//! method's default, which answers the compose template's own list. And an engine that
//! ships a file while it applies a compose template (`TemplateShipper`).

mod common;

use std::sync::{Arc, Mutex};

use flm_core::core::artifact::{ArtifactId, ArtifactNamer};
use flm_core::core::plan::Overrides;
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, FlmRegistry, PackageEntry, RegistryBuilder, RegistryInputs,
};
use flm_core::core::run::Run;
use flm_core::model::digest::Fnv1a64;
use flm_core::model::options::OptionValues;
use flm_core::stdlib::backends::html;
use flm_core::stdlib::backends::lowering::NAMER_SLOT;
use flm_core::stdlib::features::compose::{
    Accepts, ComposeFeature, ComposeOptions, ENGINES_SLOT, ShippedFile, TEMPLATES_SLOT, Template, TemplateArtifact,
    TemplateChoice, TemplateContext, TemplateEngine, TemplateFailed, TemplateShipper, stage_name,
};
use flm_core::stdlib::{StdlibConfig, add_features};
use serde_json::{Map, Value};
use techy::error::Severity;

use common::html::page as rendered_page;
use common::naming::JobNamer;
use common::stdlib::run_workflow;

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

    fn artifacts(
        &self,
        template: &Template,
        context: &TemplateContext,
        _shipper: &Arc<dyn TemplateShipper>,
    ) -> Result<Vec<TemplateArtifact>, String> {
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
            source: None,
        });
        Ok(artifacts)
    }

    fn render(
        &self,
        _template: &Template,
        context: &TemplateContext,
        _shipper: &Arc<dyn TemplateShipper>,
    ) -> Result<String, String> {
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
        source: None,
        accepts: Accepts::All,
        requires: Vec::new(),
        config: Map::new(),
        settings: None,
        delimiters: None,
        artifacts: vec![TemplateArtifact::File {
            name: String::from("style"),
            extension: String::from("css"),
            media_type: String::from("text/css"),
            apply: false,
            bytes: b"p { margin: 0 }\n".to_vec(),
            source: None,
        }],
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

/// The registry of the standard features with the composer, the feature `choosing` and the tests'
/// naming of files.
fn registry(seen: &Arc<Mutex<Seen>>, fail: Option<&'static str>) -> Arc<FlmRegistry> {
    registry_with(Arc::new(Choosing { seen: Arc::clone(seen), fail }))
}

/// The registry of the standard features with the composer, the feature `feature` and the
/// tests' naming of files.
fn registry_with(feature: Arc<dyn FlmFeature>) -> Arc<FlmRegistry> {
    let mut entry = add_features(PackageEntry::new("std"), StdlibConfig::new());
    entry.features.extend([Arc::new(ComposeFeature::new()) as Arc<dyn FlmFeature>, feature]);
    let mut builder = RegistryBuilder::new();
    builder.use_package(entry);
    builder.use_package(flm_core::math_passthrough::package());
    builder.contribute_to(NAMER_SLOT, Arc::new(JobNamer) as Arc<dyn ArtifactNamer>);
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()))
}

/// Renders a paragraph to HTML through the compose template `chosen`.
fn render(registry: &Arc<FlmRegistry>) -> Run {
    render_through(registry, "chosen")
}

/// Renders a paragraph to HTML through the compose template `template`.
fn render_through(registry: &Arc<FlmRegistry>, template: &str) -> Run {
    let options = ComposeOptions { template: TemplateChoice::Name(String::from(template)), ..ComposeOptions::default() };
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
    let failures: Vec<TemplateFailed> =
        run.diagnostics().iter().filter_map(|d| d.data().downcast_ref::<TemplateFailed>()).cloned().collect();
    assert_eq!(
        failures,
        [TemplateFailed {
            template: Some(String::from("chosen")),
            output: String::from("html"),
            detail: String::from("the file `banner.png` cannot be read"),
        }]
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

    fn render(
        &self,
        template: &Template,
        _context: &TemplateContext,
        _shipper: &Arc<dyn TemplateShipper>,
    ) -> Result<String, String> {
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
        metadata: Value::Object(Map::new()),
        config: Map::new(),
        artifacts: Vec::new(),
    };
    let shipper: Arc<dyn TemplateShipper> = Arc::new(Unused);
    assert_eq!(Plain.artifacts(&template, &context, &shipper), Ok(template.artifacts.clone()));
}

/// A handle that the default list of files never calls.
struct Unused;

impl TemplateShipper for Unused {
    fn warn(&self, detail: String) {
        panic!("the default list of files reports no warning: {detail}");
    }

    fn ship(&self, _file: ShippedFile) -> Result<String, String> {
        panic!("the default list of files ships nothing")
    }
}

// --- a file shipped while the template is applied ----------------------------------------------

/// The bytes of the stylesheet that the engine `shipping` ships.
const NOTES: &[u8] = b"h1 { color: teal }\n";

/// A template engine that ships the stylesheet `notes.css` from `render`, records the path
/// each shipping returned, and writes that path and the content.
struct Shipping {
    returned: Arc<Mutex<Vec<String>>>,
}

impl TemplateEngine for Shipping {
    fn name(&self) -> &str {
        "shipping"
    }

    fn render(
        &self,
        _template: &Template,
        context: &TemplateContext,
        shipper: &Arc<dyn TemplateShipper>,
    ) -> Result<String, String> {
        let file = ShippedFile {
            stem: String::from("notes"),
            extension: String::from("css"),
            media_type: String::from("text/css"),
            bytes: NOTES.to_vec(),
        };
        let path = shipper.ship(file)?;
        self.returned.lock().unwrap().push(path.clone());
        let content = context.regions.iter().find(|(name, _)| name == "content").map(|(_, text)| text.as_str());
        Ok(format!("{path};{}", content.unwrap_or_default()))
    }
}

/// A feature contributing the engine `shipping` and the compose template `shipping`, which
/// lists no file of its own.
struct ShippingFeature {
    returned: Arc<Mutex<Vec<String>>>,
}

impl FlmFeature for ShippingFeature {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("test.shipping", "A compose template whose engine ships a file as it applies it")
            .docs("the template engine `shipping` and the compose template `shipping`")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let engine = Shipping { returned: Arc::clone(&self.returned) };
        b.peer_slot::<dyn TemplateEngine>(ENGINES_SLOT, Arc::new(engine))?;
        let template = Template {
            name: String::from("shipping"),
            engine: String::from("shipping"),
            artifacts: Vec::new(),
            ..chosen_template()
        };
        b.peer_slot::<Template>(TEMPLATES_SLOT, Arc::new(template))
    }
}

#[test]
fn a_file_shipped_from_render_is_shipped_once_under_its_digest_and_the_entry_refers_to_it() {
    let returned = Arc::new(Mutex::new(Vec::new()));
    let run = render_through(&registry_with(Arc::new(ShippingFeature { returned: Arc::clone(&returned) })), "shipping");
    assert!(
        run.diagnostics().iter().all(|d| d.severity() != Severity::Error),
        "{}",
        run.diagnostics().render_all()
    );

    // The identity: the stem, then eight hexadecimal digits of a digest of the bytes and
    // the media type.
    let mut digest = Fnv1a64::new();
    digest.write_bytes(NOTES);
    digest.write_str("text/css");
    let name = format!("notes-{:08x}", digest.value() >> 32);
    let id = ArtifactId::new("compose", name.as_str());

    // Every call (the page is rendered twice on HTML) returned the naming service's path.
    let returned = returned.lock().unwrap();
    assert_eq!(returned.len(), 2, "{returned:?}");
    let path = format!("job-{name}.css");
    assert!(returned.iter().all(|returned| *returned == path), "{returned:?}");
    assert!(rendered_page(&run).starts_with(&format!("{path};")));

    // The artifact set holds the file once, with its bytes and media type.
    let set = run.table(html::DOCUMENT).expect("declared").expect("written");
    let shipped: Vec<_> = set.iter().filter(|artifact| artifact.id.owner.as_str() == "compose").collect();
    assert_eq!(shipped.len(), 1, "{shipped:?}");
    assert_eq!(shipped[0].id, id);
    assert_eq!(
        (shipped[0].media_type.as_str(), shipped[0].extension.as_deref(), shipped[0].as_bytes()),
        ("text/css", Some("css"), Some(NOTES))
    );
    // The entry artifact refers to it.
    assert!(set.entry().expect("the page").refs.contains(&id));
}
