//! The feature `compose` of the library (`flm_core::stdlib::features::compose`): a
//! compose template read from data, the composer applying a `content-only` template that
//! another feature contributes, the contest a registry holding the composer meets when
//! no plan request prefers it (why the feature is not among the standard features), a
//! registry without the feature, which assembles each output as the backend's final
//! stage does, and a piece declared off by default that a compose template requires,
//! which the selection of pieces places (`select_pieces`).

mod common;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::sync::Arc;

use flm_core::core::plan::{Overrides, Plan, PlanError};
use flm_core::core::registry::{
    BuildError, DefContent, FeatureBuilder, FeatureChain, FeatureInfo, FlmFeature, FlmRegistry,
    PackageEntry, RegistryBuilder, RegistryInputs,
};
use flm_core::model::names::Name;
use flm_core::model::options::OptionValues;
use flm_core::stdlib::backends::html::{self, DefinitionOverride, HtmlRenderOptions};
use flm_core::stdlib::backends::pieces::{self, PiecesRenderOptions};
use flm_core::stdlib::features::compose::{
    Accepts, CONTENT_ONLY, ComposeFeature, ComposeOptions, Delimiters, ENGINES_SLOT, HTML_REGIONS,
    TEMPLATES_SLOT, Template, TemplateArtifact, TemplateChoice, TemplateEngine, TemplateFailed,
    TemplateShipper, region_id, select_pieces, served_output, stage_name,
};
use flm_core::stdlib::{StdlibConfig, add_features};
use serde_json::{Map, Value, json};

use common::html::{assert_golden, page as rendered_page};
use common::naming::JobNamer;
use flm_core::core::artifact::{ArtifactData, ArtifactId, ArtifactNamer, RelPath};
use flm_core::core::run::Run;
use flm_core::stdlib::backends::lowering::NAMER_SLOT;
use flm_core::stdlib::features::compose::TemplateContext;
use techy::error::Severity;
use common::pieces::{Content, TestFeature, TestPiece};
use common::stdlib::{assert_only_unchecked_output, registry as stdlib_registry, run_workflow};

// --- a compose template read from data ------------------------------------------------------

/// A reader of the files a description lists that finds none.
fn no_files(_path: &str) -> Result<Option<Vec<u8>>, String> {
    Ok(None)
}

/// What a reader of a compose template's text returns: the text, `None` for no file.
type TextRead = Result<Option<String>, String>;

/// A reader of a compose template's text that finds no file.
fn no_text(_path: &str) -> TextRead {
    Ok(None)
}

/// A reader of a compose template's text that finds `text` at `page.html`, and no other
/// file.
fn page_at(text: &'static str) -> impl Fn(&str) -> TextRead {
    move |path| Ok((path == "page.html").then(|| String::from(text)))
}

/// What a folder reader returns: the folder's files by relative path, `None` for no folder.
type FolderRead = Result<Option<Vec<(String, Vec<u8>)>>, String>;

/// A reader of the folders a description lists that finds none.
fn no_folders(_path: &str) -> FolderRead {
    Ok(None)
}

/// The places of the files a description lists, which have none.
fn no_place(_path: &str) -> Option<String> {
    None
}

/// A reader of the paths of the defaults that keeps each as written.
fn as_written(path: &str) -> Result<String, String> {
    Ok(path.to_string())
}

#[test]
fn a_template_read_from_data_keeps_every_field() {
    let description = json!({
        "name": "paper",
        "output": "latex-recomposed",
        "engine": "host",
        "main": "paper.tex",
        "accepts": ["flm.metadata.title", "flm.metadata.author"],
        "requires": ["flm.metadata.title"],
        "settings": {
            "type": "object",
            "properties": {
                "class": { "type": "string", "default": "article" },
                "options": { "type": "array", "default": ["11pt"], "x-flm-marks": ["markup"] },
            },
        },
        "delimiters": {
            "block": ["<%", "%>"],
            "variable": ["<<", ">>"],
            "comment": ["<#", "#>"],
        },
    });
    let settings = description["settings"].clone();
    let text = |path: &str| Ok((path == "paper.tex").then(|| String::from("<<regions.content>>\n")));
    let template =
        Template::from_data("mine", description, &text, &no_files, &no_folders, &|path| Some(format!("ext+mine:{path}")), &as_written)
            .unwrap();
    let mut config = Map::new();
    config.insert(String::from("class"), json!("article"));
    config.insert(String::from("options"), json!(["11pt"]));
    let pair = |a: &str, b: &str| (String::from(a), String::from(b));
    assert_eq!(
        template,
        Template {
            name: String::from("paper"),
            package: String::from("mine"),
            output: String::from("latex-recomposed"),
            engine: String::from("host"),
            text: String::from("<<regions.content>>\n"),
            source: Some(String::from("ext+mine:paper.tex")),
            accepts: Accepts::Only(vec![Name::from("flm.metadata.title"), Name::from("flm.metadata.author")]),
            requires: vec![Name::from("flm.metadata.title")],
            config,
            settings: Some(settings),
            delimiters: Some(Delimiters {
                block: pair("<%", "%>"),
                variable: pair("<<", ">>"),
                comment: pair("<#", "#>"),
            }),
            artifacts: Vec::new(),
        }
    );

    // The optional keys left out: nothing required, no settings, the usual delimiters.
    let bare = Template::from_data(
        "mine",
        json!({ "name": "bare", "output": "html", "engine": CONTENT_ONLY, "accepts": "all" }),
        &no_text,
        &no_files,
        &no_folders,
        &no_place,
        &as_written,
    )
    .unwrap();
    assert_eq!(bare.accepts, Accepts::All);
    assert!(bare.requires.is_empty() && bare.config.is_empty() && bare.delimiters.is_none());
    assert_eq!(bare.settings, Some(json!({ "type": "object", "properties": {} })));
    assert_eq!(bare.text, "");
}

#[test]
fn a_template_reads_its_text_once_from_the_file_main_gives() {
    let asked = RefCell::new(Vec::new());
    let text = |path: &str| -> TextRead {
        asked.borrow_mut().push(String::from(path));
        Ok(Some(String::from("<main>{{ regions.content }}</main>")))
    };
    let description =
        json!({ "name": "paper", "output": "html", "engine": "host", "main": "pages/paper.html", "accepts": "all" });
    let template = Template::from_data("mine", description, &text, &no_files, &no_folders, &no_place, &as_written).unwrap();
    assert_eq!(template.text, "<main>{{ regions.content }}</main>");
    // Asked once, for the path as the description writes it.
    assert_eq!(*asked.borrow(), ["pages/paper.html"]);

    // Without `main`, the text is empty and the reader is not asked.
    asked.borrow_mut().clear();
    let description = json!({ "name": "bare", "output": "html", "engine": "host", "accepts": "all" });
    let bare = Template::from_data("mine", description, &text, &no_files, &no_folders, &no_place, &as_written).unwrap();
    assert_eq!(bare.text, "");
    assert!(asked.borrow().is_empty());
}

#[test]
fn a_text_the_reader_cannot_give_is_refused() {
    let refused = |text: &dyn Fn(&str) -> TextRead| {
        let description = json!({ "name": "paper", "output": "html", "engine": "host", "main": "page.html", "accepts": "all" });
        Template::from_data("mine", description, text, &no_files, &no_folders, &no_place, &as_written).is_err()
    };
    // No file at `main`.
    assert!(refused(&no_text));
    // The reader's own failure.
    assert!(refused(&|_| Err(String::from("permission denied"))));
}

#[test]
fn a_description_with_an_unknown_key_or_a_bad_accepts_is_refused() {
    let refused =
        |description: Value| Template::from_data("mine", description, &no_text, &no_files, &no_folders, &no_place, &as_written).is_err();
    assert!(refused(json!({ "name": "paper", "output": "html", "engine": "host", "accepts": "all", "colour": "red" })));
    assert!(refused(json!({ "name": "paper", "output": "html", "accepts": "all" })));
    assert!(refused(json!(["html"])));
    assert!(refused(json!({ "name": "paper", "output": "html", "engine": "host", "accepts": "some" })));
    assert!(refused(json!({ "name": "paper", "output": "html", "engine": "host", "accepts": 3 })));
}

#[test]
fn a_description_without_a_name_is_refused() {
    let description = json!({ "output": "html", "engine": "host", "accepts": "all" });
    let refused = Template::from_data("mine", description, &no_text, &no_files, &no_folders, &no_place, &as_written).unwrap_err();
    assert!(refused.contains("`name`"), "{refused}");
}

// --- the composer in a registry -------------------------------------------------------------

/// A feature of another package that contributes the compose template `bare`, for HTML,
/// applied by the engine `content-only`, through `peer_slot`.
struct BareTemplate;

impl FlmFeature for BareTemplate {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("test.bare-template", "A compose template").docs("the compose template `bare`")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let template = Template::from_data(
            "test",
            json!({ "name": "bare", "output": "html", "engine": CONTENT_ONLY, "accepts": "all" }),
            &no_text,
            &no_files,
            &no_folders,
            &no_place,
            &as_written,
        )
        .expect("the description reads");
        b.peer_slot::<Template>(TEMPLATES_SLOT, Arc::new(template))
    }
}

/// The registry of the standard features with the features `features` added to it.
fn registry_with(features: Vec<Arc<dyn FlmFeature>>) -> Arc<FlmRegistry> {
    let mut entry = add_features(PackageEntry::new("std"), StdlibConfig::new());
    entry.features.extend(features);
    let mut builder = RegistryBuilder::new();
    builder.use_package(entry);
    builder.use_package(flm_core::math_passthrough::package());
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| {
        panic!("the registry build failed in phase {}:\n{}", error.phase, error.render())
    })
}

#[test]
fn the_composer_applies_a_content_only_template_another_feature_contributes() {
    let registry = registry_with(vec![Arc::new(ComposeFeature::new()), Arc::new(BareTemplate)]);
    let engines: Vec<&str> =
        registry.services().services_named::<dyn TemplateEngine>(ENGINES_SLOT).map(|engine| engine.name()).collect();
    assert_eq!(engines, [CONTENT_ONLY]);
    let templates: Vec<&str> =
        registry.services().services_named::<Template>(TEMPLATES_SLOT).map(|t| t.name.as_str()).collect();
    assert_eq!(templates, ["bare"]);

    let options = ComposeOptions { template: TemplateChoice::Name(String::from("bare")), ..ComposeOptions::default() };
    let overrides = Overrides {
        prefer: vec![stage_name("html").into()],
        render_options: OptionValues::new().with(options),
        ..Overrides::none()
    };
    let run = run_workflow(&registry, html::WORKFLOW, &["A paragraph with \\emph{emphasis}.\n"], &overrides);
    assert!(
        run.diagnostics().iter().all(|d| !d.identifier().starts_with("flm.compose")),
        "{}",
        run.diagnostics().render_all()
    );

    // The entry artifact is the content alone, in the element that carries its name.
    let page = rendered_page(&run);
    let opening = format!("<div id=\"{}\">\n", region_id("content"));
    assert!(page.starts_with(&opening), "{page}");
    assert!(page.ends_with("</div>\n"), "{page}");
    assert!(page.contains("<em"), "{page}");
    assert!(!page.contains("<html") && !page.contains("<!DOCTYPE"), "{page}");

    // The composer's table holds the content under its identifier.
    let composed = run.table(HTML_REGIONS).expect("the table is declared").expect("the table is written");
    let content = composed.get(&region_id("content")).expect("the content");
    assert!(content.text.contains("<em"), "{}", content.text);
}

/// A template engine of the tests' own, `marking`, which writes its marker alone.
struct Marking(&'static str);

impl TemplateEngine for Marking {
    fn name(&self) -> &str {
        "marking"
    }

    fn render(
        &self,
        _template: &Template,
        _context: &TemplateContext,
        _shipper: &Arc<dyn TemplateShipper>,
    ) -> Result<String, String> {
        Ok(format!("marked by {}", self.0))
    }
}

/// A feature `name` contributing the engine `marking` with the marker `marker`,
/// and, where `template` says so, the compose template `marked` written in it.
fn marking_feature(name: &'static str, marker: &'static str, template: bool) -> Arc<dyn FlmFeature> {
    common::pipeline::feature(name, move |b| {
        b.peer_slot::<dyn TemplateEngine>(ENGINES_SLOT, Arc::new(Marking(marker)))?;
        if template {
            let description = json!({ "name": "marked", "output": "html", "engine": "marking", "accepts": "all" });
            let template = Template::from_data("test", description, &no_text, &no_files, &no_folders, &no_place, &as_written)
                .expect("the description reads");
            b.peer_slot::<Template>(TEMPLATES_SLOT, Arc::new(template))?;
        }
        Ok(())
    })
}

#[test]
fn of_two_engines_of_one_name_the_composer_uses_the_one_of_the_feature_placed_last() {
    let registry_of = |chains: &[&str]| {
        let mut entry = add_features(PackageEntry::new("std"), StdlibConfig::new());
        entry.features.extend([
            Arc::new(ComposeFeature::new()) as Arc<dyn FlmFeature>,
            marking_feature("test.marking-a", "a", true),
            marking_feature("test.marking-b", "b", false),
        ]);
        let mut builder = RegistryBuilder::new();
        builder.use_package(entry);
        builder.use_package(flm_core::math_passthrough::package());
        for text in chains {
            builder.feature_order(FeatureChain::parse(text, "test").expect("a chain"));
        }
        builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()))
    };
    let options = ComposeOptions { template: TemplateChoice::Name(String::from("marked")), ..ComposeOptions::default() };
    let overrides = Overrides {
        prefer: vec![stage_name("html").into()],
        render_options: OptionValues::new().with(options),
        ..Overrides::none()
    };
    let page = |chains: &[&str]| {
        let run = run_workflow(&registry_of(chains), html::WORKFLOW, &["A paragraph.\n"], &overrides);
        assert!(
            run.diagnostics().iter().all(|d| !d.identifier().starts_with("flm.compose")),
            "{}",
            run.diagnostics().render_all()
        );
        rendered_page(&run)
    };
    // `test.marking-b` is placed last.
    assert_eq!(page(&[]), "marked by b");
    // The chain places `test.marking-a` last.
    assert_eq!(page(&["test.marking-b < test.marking-a"]), "marked by a");
}

#[test]
fn a_registry_with_the_composer_and_no_preference_is_contested() {
    let registry = registry_with(vec![Arc::new(ComposeFeature::new()), Arc::new(BareTemplate)]);
    match Plan::for_workflow(&registry, html::WORKFLOW, &Overrides::none()) {
        Err(PlanError::Contested { candidates, .. }) => {
            let candidates: Vec<String> = candidates.iter().map(ToString::to_string).collect();
            assert!(candidates.contains(&stage_name("html")), "{candidates:?}");
            assert!(candidates.contains(&String::from("flm.html.final.singlefile")), "{candidates:?}");
        }
        Err(other) => panic!("unexpected error: {other}"),
        Ok(_) => panic!("the plan was built with two stages providing the page"),
    }
}

#[test]
fn a_registry_without_the_compose_feature_assembles_each_output_as_before() {
    // The registry of the standard features alone, which does not hold the feature: no composer, and
    // the page is the one `html_pipeline.rs`'s `a_small_document_is_a_page` pins, under the
    // same render options (every stylesheet by a local address).
    let registry = stdlib_registry(StdlibConfig::new());
    assert!(registry.services().services_named::<Template>(TEMPLATES_SLOT).next().is_none());
    let mut definitions = BTreeMap::new();
    for style in html::defs::stylesheets() {
        let id = style.id.to_string();
        let stem = id.rsplit('.').next().expect("an identifier has a last part");
        definitions.insert(
            Name::from(id.as_str()),
            DefinitionOverride::Parts(vec![DefContent::Url(format!("css/{stem}.css"))]),
        );
    }
    let options = HtmlRenderOptions { definitions, ..HtmlRenderOptions::default() };
    let overrides = Overrides { render_options: OptionValues::new().with(options), ..Overrides::none() };
    let plan = Plan::for_workflow(&registry, html::WORKFLOW, &overrides).expect("one stage provides the page");
    assert!(plan.scheduled_names().iter().all(|name| !name.starts_with("flm.compose.")));
    let run = run_workflow(
        &registry,
        html::WORKFLOW,
        &["\\section{A title}\n\nA paragraph with \\emph{emphasis} and \\verbcode|code|.\n"],
        &overrides,
    );
    assert_golden("document.page.html", &rendered_page(&run));
}

// --- files a compose template ships ---------------------------------------------------------

/// A template engine of the tests' own, `subst`: it writes the text with each place
/// `{{ regions.NAME }}`, `{{ config.KEY }}` and `{{ artifacts.NAME }}` replaced by its
/// value, and refuses a place whose value nothing provides.
struct SubstEngine;

impl TemplateEngine for SubstEngine {
    fn name(&self) -> &str {
        "subst"
    }

    fn render(
        &self,
        template: &Template,
        context: &TemplateContext,
        _shipper: &Arc<dyn TemplateShipper>,
    ) -> Result<String, String> {
        let mut out = String::new();
        let mut rest = template.text.as_str();
        while let Some(at) = rest.find("{{") {
            out.push_str(&rest[..at]);
            let (place, after) = rest[at + 2..].split_once("}}").ok_or("a place is not closed")?;
            let place = place.trim();
            let value = match place.split_once('.') {
                Some(("regions", name)) => context.regions.iter().find(|(n, _)| n == name).map(|(_, text)| text.clone()),
                Some(("config", key)) => context.config.get(key).map(|value| match value {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                }),
                Some(("artifacts", name)) => context.artifacts.iter().find(|(n, _)| n == name).map(|(_, path)| path.clone()),
                _ => None,
            };
            out.push_str(&value.ok_or_else(|| format!("the value `{place}` is undefined"))?);
            rest = after;
        }
        out.push_str(rest);
        Ok(out)
    }
}

/// The files of the package that ships the compose template `shipping`, by path.
fn shipped_files(path: &str) -> Result<Option<Vec<u8>>, String> {
    Ok(match path {
        "fancy.css" => Some(b"p { margin: 0 }\n".to_vec()),
        "theme.css" => Some(b"body { color: {{ config.color }}; background: url({{ artifacts.logo }}) }\n".to_vec()),
        "img/logo.png" => Some(vec![0x89, b'P', b'N', b'G', 0xff, 0xfe]),
        "notes" | "data.weird" => Some(b"x".to_vec()),
        "broken.css" => return Err(String::from("permission denied")),
        _ => None,
    })
}

/// The folders of the package that ships the compose template `shipping`, by path: each
/// file by its path relative to the folder, in the order the reader gives.
fn shipped_folders(path: &str) -> FolderRead {
    Ok(match path {
        "icons" => Some(vec![
            (String::from("check.svg"), b"<svg>check</svg>".to_vec()),
            (String::from("more/cross.svg"), b"<svg>cross</svg>".to_vec()),
            (String::from("more/data.weird"), b"x".to_vec()),
        ]),
        "empty" => Some(Vec::new()),
        "locked" => return Err(String::from("the shipped folder `locked` cannot be read: permission denied")),
        _ => None,
    })
}

/// The description of the compose template `shipping`: a copied stylesheet, an applied
/// stylesheet and a binary logo, and a page linking the three, at `page.html`.
fn shipping_description() -> Value {
    json!({
        "name": "shipping",
        "output": "html",
        "engine": "subst",
        "main": "page.html",
        "accepts": "all",
        "settings": { "type": "object", "properties": { "color": { "type": "string", "default": "teal" } } },
        "artifacts": {
            "style": { "file": "fancy.css" },
            "theme": { "file": "theme.css", "template": true },
            "logo": { "file": "img/logo.png" },
        },
    })
}

/// A feature contributing the engine `subst` and the compose template `shipping` of the
/// text `text` and the description `description`.
struct Shipping {
    text: &'static str,
    description: fn() -> Value,
}

impl FlmFeature for Shipping {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("test.shipping", "A compose template shipping files").docs("the compose template `shipping`")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let template = Template::from_data("test", (self.description)(), &page_at(self.text), &shipped_files, &shipped_folders, &no_place, &as_written)
            .expect("the description reads");
        b.peer_slot::<dyn TemplateEngine>(ENGINES_SLOT, Arc::new(SubstEngine))?;
        b.peer_slot::<Template>(TEMPLATES_SLOT, Arc::new(template))
    }
}

/// The page of the compose template `shipping`.
const SHIPPING_PAGE: &str = "<link href=\"{{ artifacts.style }}\"><link href=\"{{ artifacts.theme }}\">\
                             <img src=\"{{ artifacts.logo }}\">{{ regions.content }}";

/// The registry of the standard features with the composer and the feature `shipping` of the page
/// `text`, and the tests' naming service where `namer` says so.
fn shipping_registry(text: &'static str, namer: bool) -> Arc<FlmRegistry> {
    shipping_registry_of(text, shipping_description, namer)
}

/// The registry of [`shipping_registry`], the compose template `shipping` read from the
/// description `description`.
fn shipping_registry_of(text: &'static str, description: fn() -> Value, namer: bool) -> Arc<FlmRegistry> {
    let mut entry = add_features(PackageEntry::new("std"), StdlibConfig::new());
    entry.features.extend([Arc::new(ComposeFeature::new()) as Arc<dyn FlmFeature>, Arc::new(Shipping { text, description })]);
    let mut builder = RegistryBuilder::new();
    builder.use_package(entry);
    builder.use_package(flm_core::math_passthrough::package());
    if namer {
        builder.contribute_to(NAMER_SLOT, Arc::new(JobNamer) as Arc<dyn ArtifactNamer>);
    }
    builder.build(&RegistryInputs::new()).unwrap_or_else(|error| panic!("{}", error.render()))
}

/// Renders a paragraph to HTML through the compose template `shipping`.
fn render_shipping(registry: &Arc<FlmRegistry>) -> Run {
    let options = ComposeOptions { template: TemplateChoice::Name(String::from("shipping")), ..ComposeOptions::default() };
    let overrides = Overrides {
        prefer: vec![stage_name("html").into()],
        render_options: OptionValues::new().with(options),
        ..Overrides::none()
    };
    run_workflow(registry, html::WORKFLOW, &["A paragraph.\n"], &overrides)
}

/// The compose template's failures the run reports.
fn template_failures(run: &Run) -> Vec<TemplateFailed> {
    run.diagnostics().iter().filter_map(|d| d.data().downcast_ref::<TemplateFailed>()).cloned().collect()
}

/// A failure of the compose template `template` for the output `html`, with `detail`.
fn failed(template: &str, detail: &str) -> TemplateFailed {
    TemplateFailed { template: Some(template.into()), output: "html".into(), detail: detail.into() }
}

/// The name, extension, media type, application and bytes of the file `artifact`, which
/// must be a file and not a folder.
fn file_of(artifact: &TemplateArtifact) -> (&str, &str, &str, bool, &[u8]) {
    match artifact {
        TemplateArtifact::File { name, extension, media_type, apply, bytes, .. } => {
            (name, extension, media_type, *apply, bytes)
        }
        TemplateArtifact::Folder { name, .. } => panic!("`{name}` is a folder"),
    }
}

#[test]
fn a_template_reads_its_files_with_their_extensions_and_media_types() {
    let template = Template::from_data("test", shipping_description(), &page_at(SHIPPING_PAGE), &shipped_files, &shipped_folders, &no_place, &as_written).unwrap();
    let read: Vec<(&str, &str, &str, bool)> = template
        .artifacts
        .iter()
        .map(|a| {
            let (name, extension, media_type, apply, _) = file_of(a);
            (name, extension, media_type, apply)
        })
        .collect();
    // In the order of the description.
    assert_eq!(
        read,
        [("style", "css", "text/css", false), ("theme", "css", "text/css", true), ("logo", "png", "image/png", false)]
    );
    assert_eq!(file_of(&template.artifacts[2]).4, shipped_files("img/logo.png").unwrap().unwrap());
    // A media type the description gives is kept.
    let given = Template::from_data(
        "test",
        json!({ "name": "given", "output": "html", "engine": "subst", "accepts": "all",
                "artifacts": { "data": { "file": "data.weird", "media_type": "application/x-weird" } } }),
        &no_text,
        &shipped_files,
        &shipped_folders,
        &no_place,
        &as_written,
    )
    .unwrap();
    let (_, extension, media_type, _, _) = file_of(&given.artifacts[0]);
    assert_eq!((extension, media_type), ("weird", "application/x-weird"));
}

#[test]
fn the_composer_names_applies_and_writes_the_files_a_template_ships() {
    let run = render_shipping(&shipping_registry(SHIPPING_PAGE, true));
    assert!(
        run.diagnostics().iter().all(|d| d.severity() != Severity::Error),
        "{}",
        run.diagnostics().render_all()
    );
    let set = run.table(html::DOCUMENT).expect("declared").expect("written");
    let page = rendered_page(&run);
    assert!(
        page.starts_with(
            "<link href=\"job-style.css\"><link href=\"job-theme.css\"><img src=\"job-logo.png\"><div id=\"flm-region-content\">"
        ),
        "{page}"
    );
    let shipped = |name: &str| set.get(&ArtifactId::new("compose", name)).unwrap_or_else(|| panic!("compose:{name}"));
    let described = |name: &str| {
        let artifact = shipped(name);
        (artifact.media_type.as_str(), artifact.extension.as_deref(), artifact.entry)
    };
    assert_eq!(described("style"), ("text/css", Some("css"), false));
    assert_eq!(described("theme"), ("text/css", Some("css"), false));
    assert_eq!(described("logo"), ("image/png", Some("png"), false));
    assert_eq!(shipped("style").as_bytes(), Some(&b"p { margin: 0 }\n"[..]));
    assert_eq!(shipped("theme").as_bytes(), Some(&b"body { color: teal; background: url(job-logo.png) }\n"[..]));
    assert_eq!(shipped("logo").as_bytes(), shipped_files("img/logo.png").unwrap().as_deref());
    // The page links to each.
    let refs: Vec<String> = set.entry().expect("the page").refs.iter().map(ToString::to_string).collect();
    for name in ["compose:style", "compose:theme", "compose:logo"] {
        assert!(refs.iter().any(|r| r == name), "{name} in {refs:?}");
    }
}

#[test]
fn a_value_nothing_provides_is_the_engines_error() {
    let run = render_shipping(&shipping_registry("{{ artifacts.missing }}", true));
    assert_eq!(template_failures(&run), [failed("shipping", "the value `artifacts.missing` is undefined")]);
    assert!(run.table(html::DOCUMENT).expect("declared").is_none_or(|set| set.is_empty()));
}

#[test]
fn without_a_naming_service_a_template_shipping_files_cannot_be_applied() {
    let run = render_shipping(&shipping_registry(SHIPPING_PAGE, false));
    let found: Vec<(Option<String>, String)> =
        template_failures(&run).into_iter().map(|failure| (failure.template, failure.output)).collect();
    assert_eq!(found, [(Some(String::from("shipping")), String::from("html"))]);
}

#[test]
fn a_file_a_template_cannot_ship_is_refused() {
    let refused = |engine: &str, artifact: Value| {
        let description =
            json!({ "name": "t", "output": "html", "engine": engine, "accepts": "all", "artifacts": { "a": artifact } });
        Template::from_data("test", description, &no_text, &shipped_files, &shipped_folders, &no_place, &as_written).is_err()
    };
    assert!(refused("subst", json!({ "file": "missing.css" })));
    assert!(refused("subst", json!({ "file": "broken.css" })));
    assert!(refused("subst", json!({ "file": "notes" })));
    assert!(refused("subst", json!({ "file": "data.weird" })));
    assert!(refused(CONTENT_ONLY, json!({ "file": "fancy.css", "template": true })));
    assert!(refused("subst", json!({ "file": "img/logo.png", "template": true })));
    assert!(refused("subst", json!({ "file": "fancy.css", "colour": "red" })));
}

// --- a folder a compose template ships ------------------------------------------------------

/// The description of the compose template `shipping` shipping the folder `icons` beside
/// a stylesheet, its page at `page.html`.
fn folder_description() -> Value {
    json!({
        "name": "shipping",
        "output": "html",
        "engine": "subst",
        "main": "page.html",
        "accepts": "all",
        "artifacts": {
            "style": { "file": "fancy.css" },
            "icons": { "dir": "icons" },
        },
    })
}

#[test]
fn a_template_reads_a_folder_through_its_folder_reader() {
    let template = Template::from_data("test", folder_description(), &page_at(SHIPPING_PAGE), &shipped_files, &shipped_folders, &no_place, &as_written).unwrap();
    assert_eq!(template.artifacts.iter().map(TemplateArtifact::name).collect::<Vec<_>>(), ["style", "icons"]);
    assert_eq!(
        template.artifacts[1],
        TemplateArtifact::Folder { name: String::from("icons"), files: shipped_folders("icons").unwrap().unwrap() }
    );
}

#[test]
fn the_composer_ships_a_folder_as_one_artifact_and_gives_its_path() {
    let page = "<link href=\"{{ artifacts.style }}\"><img src=\"{{ artifacts.icons }}/check.svg\">{{ regions.content }}";
    let run = render_shipping(&shipping_registry_of(page, folder_description, true));
    assert!(
        run.diagnostics().iter().all(|d| d.severity() != Severity::Error),
        "{}",
        run.diagnostics().render_all()
    );
    // The page sees the folder's path, with no trailing slash.
    let text = rendered_page(&run);
    assert!(text.starts_with("<link href=\"job-style.css\"><img src=\"job-icons/check.svg\">"), "{text}");
    let set = run.table(html::DOCUMENT).expect("declared").expect("written");
    let icons = set.get(&ArtifactId::new("compose", "icons")).expect("compose:icons");
    // The folder's files, in the order the reader gave; no media type, no extension.
    let expected: Vec<(RelPath, Vec<u8>)> = shipped_folders("icons")
        .unwrap()
        .unwrap()
        .into_iter()
        .map(|(path, bytes)| (RelPath::new(path), bytes))
        .collect();
    assert_eq!(icons.data, ArtifactData::Files(expected));
    assert_eq!((icons.media_type.as_str(), icons.extension.as_deref(), icons.entry), ("", None, false));
    let refs: Vec<String> = set.entry().expect("the page").refs.iter().map(ToString::to_string).collect();
    assert!(refs.iter().any(|r| r == "compose:icons"), "{refs:?}");
}

#[test]
fn a_folder_a_template_cannot_ship_is_refused() {
    let read = |artifact: Value| {
        let description =
            json!({ "name": "t", "output": "html", "engine": "subst", "accepts": "all", "artifacts": { "a": artifact } });
        Template::from_data("test", description, &no_text, &shipped_files, &shipped_folders, &no_place, &as_written).unwrap_err()
    };
    // Each is refused: `read` fails the test on a template that reads.
    read(json!({ "dir": "empty" }));
    read(json!({ "dir": "missing" }));
    // The folder reader's text (the test's own) is returned as it is.
    assert_eq!(read(json!({ "dir": "locked" })), "the shipped folder `locked` cannot be read: permission denied");
    read(json!({ "file": "fancy.css", "dir": "icons" }));
    read(json!({ "dir": "icons", "template": false }));
    read(json!({ "dir": "icons", "media_type": "image/svg+xml" }));
    read(json!({ "dir": "icons", "template": true, "media_type": "image/svg+xml" }));
    read(json!({ "template": true }));
}

// --- a required piece is placed ----------------------------------------------------------------

/// A feature of another package that contributes the compose template `needs`, for HTML,
/// applied by the engine `content-only`, which requires the piece `required`.
struct RequiringTemplate {
    required: String,
}

impl FlmFeature for RequiringTemplate {
    fn info(&self) -> FeatureInfo {
        FeatureInfo::new("test.requiring-template", "A compose template requiring a piece")
            .docs("the compose template `needs`")
    }

    fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
        let template = Template::from_data(
            "test",
            json!({ "name": "needs", "output": "html", "engine": CONTENT_ONLY, "accepts": "all", "requires": [self.required] }),
            &no_text,
            &no_files,
            &no_folders,
            &no_place,
            &as_written,
        )
        .expect("the description reads");
        b.peer_slot::<Template>(TEMPLATES_SLOT, Arc::new(template))
    }
}

#[test]
fn a_piece_off_by_default_that_the_compose_template_requires_is_placed() {
    let piece = TestPiece::before("contents", Content::Source("Contents.")).off();
    let registry = registry_with(vec![
        Arc::new(ComposeFeature::new()),
        Arc::new(RequiringTemplate { required: piece.name() }),
        Arc::new(TestFeature { pieces: vec![piece.clone()] }),
    ]);
    let template = registry
        .services()
        .services_named::<Template>(TEMPLATES_SLOT)
        .find(|template| template.name == "needs")
        .expect("the template is contributed");
    let served = served_output(&registry, "html").expect("the composer serves the HTML output");

    // The selection of pieces includes the required piece beside setting what the HTML
    // backend may place.
    let mut selection = PiecesRenderOptions::default();
    select_pieces(&registry, template, served, &mut selection);
    assert!(selection.include.contains(&Name::from(piece.name())), "{:?}", selection.include);
    assert!(
        selection.backends.get(&served.backend).is_some_and(|names| names.contains(&Name::from(piece.name()))),
        "{:?}",
        selection.backends
    );

    // Rendered under that selection, the HTML backend places the piece.
    let placed = |selection: PiecesRenderOptions| -> Vec<String> {
        let compose = ComposeOptions { template: TemplateChoice::Name(String::from("needs")), ..ComposeOptions::default() };
        let overrides = Overrides {
            prefer: vec![stage_name("html").into()],
            render_options: OptionValues::new().with(compose).with(selection),
            ..Overrides::none()
        };
        let run = run_workflow(&registry, html::WORKFLOW, &["Body.\n"], &overrides);
        assert_only_unchecked_output(&run);
        let lowered = run
            .table(&pieces::lowered_pieces_table(&html::BACKEND.into()))
            .expect("declared")
            .expect("written");
        lowered.pieces.iter().map(|piece| piece.name.to_string()).collect()
    };
    assert_eq!(placed(selection), [piece.name()]);

    // Without the required piece among the included ones, the builder runs (the composer
    // reads the piece) and the backend places nothing: the gap the selection closes.
    let mut accepted_only = PiecesRenderOptions::default();
    accepted_only.backends.insert(served.backend.clone(), [Name::from(piece.name())].into_iter().collect());
    assert!(placed(accepted_only).is_empty());
}

