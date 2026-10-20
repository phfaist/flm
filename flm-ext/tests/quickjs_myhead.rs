//! The second case of step E4 through JavaScript: the fixture `myhead` of
//! `tests/js/myhead/`, the headings sketch reduced to one kind of heading,
//! numbered, listed in the table of contents through a `Sectioning` handler,
//! referenced through a `Referenceable` handler, its number written through a
//! title template compiled against the built language, and its recomposed
//! LaTeX `\section{…}` with its labels; rendered through the HTML page, the
//! text and the recomposed LaTeX with goldens; its setting `format`, and a
//! `format` that does not compile; and the fixture `exercises` loaded beside
//! it, its counter numbered within the headings' counter. The same tests run
//! under `flm-core/parallel`, where the aspect handlers are called on several
//! threads.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;

use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageEntry, PackageFactory};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::aspects::ContentRef;
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::backends::html;
use flm_core::stdcore::features::toc;
use flm_ext::quickjs::{JsHost, JsSettings};
use serde_json::{json, Value};

use common::{latex_of, text_of};

/// The fixture's main module, as `tests/js/myhead/` holds it (its manifest is
/// [`common::js_manifest`]`("main.js")`, the JSON equal to `flmextension.yaml`).
const MYHEAD_MAIN: &str = include_str!("js/myhead/main.js");

/// The main module of the fixture `exercises` (`tests/js/exercises/`).
const EXERCISES_MAIN: &str = include_str!("js/exercises/main.js");

/// The document: two numbered headings with labels, a starred one between
/// them, and a reference to the first.
const DOCUMENT: &str = "\\myhead{Introduction}\\label{sec:intro}

Some text.

\\myhead*{An aside}

More text.

\\myhead{Methods}\\label{sec:methods}

See \\ref{sec:intro} and \\ref{sec:methods}.";

/// The document of the two fixtures together: exercises under two headings.
const WITH_EXERCISES: &str = "\\myhead{First}

\\begin{exercise}\\label{ex:a}
One.
\\end{exercise}

\\begin{exercise}
Two.
\\end{exercise}

\\myhead{Second}

\\begin{exercise}
Three.
\\end{exercise}

See \\ref{ex:a}.";

/// The golden file `name` under `tests/goldens/quickjs/`.
fn golden(name: &str) -> String {
    format!("{}/tests/goldens/quickjs/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The factory of the package `_` of the extension `locator` whose main
/// module is `main`.
fn factory(locator: &str, main: &str) -> PackageFactory {
    let loader = common::js_loader(
        &JsHost::new(JsSettings::default()),
        common::js_extension(locator, &[("main.js", main)], common::js_manifest("main.js")),
    );
    common::factory_of(&loader, locator)
}

/// The package of `myhead` built with the feature's settings `data`.
fn myhead_with(data: Value) -> PackageEntry {
    let factory = factory("myhead", MYHEAD_MAIN);
    let mut settings = PackageSettings::new();
    let Value::Object(map) = data else { panic!("not a map") };
    if !map.is_empty() {
        let decl = factory.settings("myhead").expect("the feature declares settings");
        settings.insert(decl.read(&map).expect("the settings read"));
    }
    factory.build(&settings).expect("the package builds")
}

/// The registry of the standard package and `myhead` with the settings `data`.
fn registry_with_settings(data: Value) -> Arc<FlmRegistry> {
    common::registry_with([myhead_with(data)])
}

/// The registry of the standard package and `myhead`.
fn registry() -> Arc<FlmRegistry> {
    registry_with_settings(json!({}))
}

/// The registry of the standard package, `myhead` and `exercises`, the
/// exercises' counter numbered within the headings' counter: the fixture
/// `exercises` as written, its counter declaration given
/// `.within(Within.parent("myhead.heading"))`.
fn registry_with_exercises() -> Arc<FlmRegistry> {
    let declaration = "new CounterDecl(COUNTER, STYLE)";
    assert!(EXERCISES_MAIN.contains(declaration));
    let exercises = EXERCISES_MAIN
        .replace(declaration, "new CounterDecl(COUNTER, STYLE).within(Within.parent(\"myhead.heading\"))")
        .replace("FormatOptions, PrefixWords }", "FormatOptions, PrefixWords, Within }");
    let exercises = factory("exercises", &exercises).build(&PackageSettings::new()).expect("the package builds");
    common::registry_with([myhead_with(json!({})), exercises])
}

/// The build diagnostics of `registry`, rendered.
fn build_diagnostics(registry: &FlmRegistry) -> Vec<String> {
    registry.build_diagnostics().iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect()
}

/// The HTML page of `source`, and the diagnostics.
fn page_of(registry: &Arc<FlmRegistry>, source: &str) -> (String, Vec<String>) {
    let run = common::run(registry, html::WORKFLOW, source);
    let page = run
        .table(html::DOCUMENT)
        .expect("declared")
        .and_then(|set| set.entry().and_then(|entry| entry.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned())))
        .unwrap_or_default();
    (page, common::diagnostics(&run))
}

/// The words of `text`, joined by single spaces (the text output wraps its
/// lines).
fn words(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The table of contents' entries of `text`, as `(level, title)`.
fn entries(registry: &Arc<FlmRegistry>, text: &str) -> Vec<(u32, Option<ContentRef>)> {
    let source = Arc::new(FlmSource::new(text).with_origin(FlmOrigin::none()));
    let fragment = parse_fragment(registry.lang(), source, FragmentLayout::default(), &ParseOptions::default())
        .expect("the document parses");
    let mut content = DocContent::new();
    content.place(Arc::new(fragment));
    let plan = Plan::build(registry, &PlanRequest::new().demand(toc::ENTRIES.name())).expect("a plan");
    let mut run = Run::new(Arc::new(Doc::new(content)), Arc::new(plan), Externals::none()).expect("a run");
    run.execute(&RunOptions::new()).expect("the run executes");
    run.table(toc::ENTRIES)
        .expect("declared")
        .map(|table| table.entries.iter().map(|entry| (entry.level, entry.title.clone())).collect())
        .unwrap_or_default()
}

// --- the fixture through the three outputs -----------------------------------------------------

#[test]
fn the_fixture_s_registry_builds_without_a_diagnostic() {
    assert_eq!(build_diagnostics(&registry()), Vec::<String>::new());
    assert_eq!(build_diagnostics(&registry_with_exercises()), Vec::<String>::new());
}

#[test]
fn the_fixture_renders_the_html_page() {
    let (page, diagnostics) = page_of(&registry(), DOCUMENT);
    assert_eq!(diagnostics, Vec::<String>::new());
    // The page's body, without the standard stylesheets of its head.
    let body = &page[page.find("<body").expect("a body")..];
    // The number through the title template, the anchors from the labels and,
    // for the starred heading, from its title.
    assert!(body.contains("id=\"sec-intro\" data-flm-heading=\"myhead\" data-flm-level=\"1\" data-flm-toc=\"true\"><span class=\"flm-pres-span myhead-number\">1. </span>Introduction</h1>"), "{body}");
    assert!(body.contains("id=\"myhead--An_aside\" data-flm-heading=\"myhead\" data-flm-level=\"1\" data-flm-toc=\"false\">An aside</h1>"), "{body}");
    assert!(body.contains("<a class=\"flm-pres-link\" href=\"#sec-intro\" data-flm-kind=\"ref\">1</a>"), "{body}");
    assert_text_golden(golden("myhead.page-body.html"), body);
}

#[test]
fn the_fixture_renders_the_text() {
    let (text, diagnostics) = text_of(&registry(), DOCUMENT);
    assert_eq!(diagnostics, Vec::<String>::new());
    let words = words(&text);
    for expected in ["Introduction", "An aside", "Methods", "See 1 and 2."] {
        assert!(words.contains(expected), "{expected:?} in {words:?}");
    }
    assert_text_golden(golden("myhead.txt"), &text);
}

#[test]
fn the_fixture_renders_the_recomposed_latex() {
    let (latex, diagnostics) = latex_of(&registry(), DOCUMENT);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert!(latex.contains("\\section{Introduction}\\flmLabels{\\label{ref1}}"), "{latex}");
    assert!(latex.contains("\\section*{An aside}\n"), "{latex}");
    assert!(latex.contains("\\section{Methods}\\flmLabels{\\label{ref2}}"), "{latex}");
    assert!(latex.contains("See \\flmRef{ref1} and \\flmRef{ref2}."), "{latex}");
    assert_text_golden(golden("myhead.tex"), &latex);
}

// --- the table of contents ---------------------------------------------------------------------

#[test]
fn the_numbered_headings_are_listed_in_the_table_of_contents() {
    let found = entries(&registry(), DOCUMENT);
    // The two numbered headings, at level 1, under their titles; the starred one is absent.
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found.iter().all(|(level, title)| *level == 1 && matches!(title, Some(ContentRef::ArgumentNamed(name)) if name.as_str() == "title")), "{found:?}");
}

// --- the setting `format` ----------------------------------------------------------------------

#[test]
fn the_setting_format_changes_the_number_line() {
    let registry = registry_with_settings(json!({ "format": "Part~#{number}: " }));
    assert_eq!(build_diagnostics(&registry), Vec::<String>::new());
    let (page, diagnostics) = page_of(&registry, DOCUMENT);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert!(page.contains("<span class=\"flm-pres-span myhead-number\">Part&#xA0;1: </span>Introduction</h1>"), "{page}");
    assert!(page.contains("<span class=\"flm-pres-span myhead-number\">Part&#xA0;2: </span>Methods</h1>"), "{page}");
    let (text, _) = text_of(&registry, DOCUMENT);
    assert!(text.contains("Part\u{a0}1:Introduction") && text.contains("Part\u{a0}2:Methods"), "{text:?}");
}

#[test]
fn a_format_that_does_not_compile_is_reported_once_and_the_headings_have_no_number() {
    let registry = registry_with_settings(json!({ "format": "#{nope}. " }));
    let found = build_diagnostics(&registry);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].starts_with("myhead.bad-format: the setting `format` of myhead must be a template of the number: "), "{found:?}");
    assert!(found[0].contains("#nope"), "{found:?}");
    // The run reports nothing more; the headings are written with their titles alone.
    let (page, diagnostics) = page_of(&registry, DOCUMENT);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert!(!page.contains("myhead-number"), "{page}");
    assert!(page.contains("data-flm-toc=\"true\">Introduction</h1>"), "{page}");
    let (text, diagnostics) = text_of(&registry, DOCUMENT);
    assert_eq!(diagnostics, Vec::<String>::new());
    assert!(text.starts_with("Introduction\n"), "{text:?}");
}

// --- the two fixtures together -----------------------------------------------------------------

#[test]
fn with_exercises_the_exercises_are_numbered_within_the_headings() {
    let registry = registry_with_exercises();
    let (text, diagnostics) = text_of(&registry, WITH_EXERCISES);
    assert_eq!(diagnostics, Vec::<String>::new());
    let words = words(&text);
    for expected in ["Exercise 1.1 One.", "Exercise 1.2 Two.", "Exercise 2.1 Three.", "See Exercise 1.1."] {
        assert!(words.contains(expected), "{expected:?} in {words:?}");
    }
    assert_text_golden(golden("myhead.with-exercises.txt"), &text);
}
