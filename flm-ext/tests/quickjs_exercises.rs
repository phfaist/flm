//! The first case of step E4 through JavaScript: the fixture `exercises` of
//! `tests/js/exercises/`, a numbered, referenceable environment with a
//! counter of its own, a heading line, a command legal inside it alone and
//! the recomposed LaTeX written as the `theorems` feature writes its kinds,
//! rendered through the HTML page, the text and the recomposed LaTeX with
//! goldens; its package `intoc` listing the exercises in the table of
//! contents; and a `points` handler that throws. The same tests run under
//! `flm-core/parallel`, where the aspect handlers are called on several
//! threads.

#![cfg(feature = "quickjs")]

mod common;

use std::sync::Arc;

use flm_core::core::plan::{Plan, PlanRequest};
use flm_core::core::registry::{FlmRegistry, PackageEntry};
use flm_core::core::run::{Externals, Run, RunOptions};
use flm_core::core::settings::PackageSettings;
use flm_core::dump::golden::assert_text_golden;
use flm_core::lang::{parse_fragment, FlmOrigin, FlmSource, ParseOptions};
use flm_core::model::aspects::ContentRef;
use flm_core::model::doc::{Doc, DocContent, FragmentLayout};
use flm_core::stdcore::backends::{html, latex_recomposed};
use flm_core::stdcore::features::toc;
use flm_ext::quickjs::{JsHost, JsSettings};

use common::{latex_of, text_of};

/// The fixture's files, as `tests/js/exercises/` holds them (its manifest is
/// [`common::js_manifest`]`("main.js")`, the JSON equal to `flmextension.yaml`).
const EXERCISES_MAIN: &str = include_str!("js/exercises/main.js");

/// The document: two exercises with titles and labels, one without either,
/// a `\hint` inside the first, two references, and a `\hint` outside every
/// exercise (an unknown command there).
const DOCUMENT: &str = "\\begin{exercise}[A title]\\label{ex:first}
Solve it. \\hint{Think of the first case.}
\\end{exercise}

\\begin{exercise}[Another title]\\label{ex:second}
Solve this one too.
\\end{exercise}

\\begin{exercise}
An exercise without a title.
\\end{exercise}

See \\ref{ex:first} and \\ref[noprefix]{ex:second}.

Outside: \\hint{not here}.";

/// The diagnostic of the `\hint` outside every exercise.
const UNKNOWN_HINT: &str = "core.specs.unresolvable-command";

/// The golden file `name` under `tests/goldens/quickjs/`.
fn golden(name: &str) -> String {
    format!("{}/tests/goldens/quickjs/{name}", env!("CARGO_MANIFEST_DIR"))
}

/// The packages `entries` of the fixture (`exercises`, `exercises#intoc`),
/// its main module's text changed by `edit`, built from empty settings.
fn exercises_edited(edit: impl Fn(&str) -> String, entries: &[&str]) -> Vec<PackageEntry> {
    let main = edit(EXERCISES_MAIN);
    let loader = common::js_loader(
        &JsHost::new(JsSettings::default()),
        common::js_extension("exercises", &[("main.js", main.as_str())], common::js_manifest("main.js")),
    );
    entries
        .iter()
        .map(|entry| common::factory_of(&loader, entry).build(&PackageSettings::new()).expect("the package builds"))
        .collect()
}

/// The registry of the standard package and the fixture's packages `entries`.
fn registry_of(entries: &[&str]) -> Arc<FlmRegistry> {
    common::registry_with(exercises_edited(str::to_string, entries))
}

/// The registry of the standard package and the fixture's package `_`.
fn registry() -> Arc<FlmRegistry> {
    registry_of(&["exercises"])
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

/// The style file of the recomposed LaTeX of `source`: every artifact but the
/// entry, joined.
fn latex_style_of(registry: &Arc<FlmRegistry>, source: &str) -> String {
    let run = common::run(registry, latex_recomposed::WORKFLOW, source);
    run.table(latex_recomposed::DOCUMENT)
        .expect("declared")
        .map(|set| {
            let entry = set.entry();
            set.iter()
                .filter(|artifact| !entry.is_some_and(|entry| std::ptr::eq(entry, *artifact)))
                .filter_map(|artifact| artifact.as_bytes().map(|b| String::from_utf8_lossy(b).into_owned()))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

/// Asserts that `diagnostics` holds the one report of the `\hint` outside
/// every exercise.
fn assert_only_the_unknown_hint(diagnostics: &[String]) {
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].starts_with(UNKNOWN_HINT) && diagnostics[0].contains("hint"), "{diagnostics:?}");
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
    let registry = registry_of(&["exercises", "exercises#intoc"]);
    let found: Vec<String> = registry.build_diagnostics().iter().map(|d| format!("{}: {}", d.identifier(), d.message())).collect();
    assert!(found.is_empty(), "{found:?}");
}

#[test]
fn the_fixture_renders_the_html_page() {
    let (page, diagnostics) = page_of(&registry(), DOCUMENT);
    assert_only_the_unknown_hint(&diagnostics);
    // The page's body, without the standard stylesheets of its head.
    let body = &page[page.find("<body").expect("a body")..];
    assert_text_golden(golden("exercises.page-body.html"), body);
}

#[test]
fn the_fixture_renders_the_text() {
    let (text, diagnostics) = text_of(&registry(), DOCUMENT);
    assert_only_the_unknown_hint(&diagnostics);
    // The text wraps its lines; the words are compared.
    let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
    for expected in [
        "Exercise 1 (A title)",
        "Hint: Think of the first case.",
        "Exercise 2 (Another title)",
        "Exercise 3 An exercise without a title.",
        "See Exercise 1 and 2.",
    ] {
        assert!(words.contains(expected), "{expected:?} in {words:?}");
    }
    assert_text_golden(golden("exercises.txt"), &text);
}

#[test]
fn the_fixture_renders_the_recomposed_latex_with_its_preamble_piece() {
    let registry = registry();
    let (latex, diagnostics) = latex_of(&registry, DOCUMENT);
    assert_only_the_unknown_hint(&diagnostics);
    assert!(latex.contains("\\begin{exercise}[A title]\\flmLabels{\\label{ref1}}"), "{latex}");
    assert!(latex.contains("\\end{exercise}") && latex.contains("See \\flmRef{ref1} and \\flmRef[noprefix]{ref2}."), "{latex}");
    assert_text_golden(golden("exercises.tex"), &latex);
    // The style file holds the piece the LaTeX rule requires.
    let style = latex_style_of(&registry, DOCUMENT);
    assert!(style.contains("\\newtheorem{exercise}{Exercise}"), "{style}");
}

// --- the table of contents ---------------------------------------------------------------------

#[test]
fn with_the_package_intoc_the_titled_exercises_are_listed_at_level_three() {
    let found = entries(&registry_of(&["exercises", "exercises#intoc"]), DOCUMENT);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(found.iter().all(|(level, title)| *level == 3 && matches!(title, Some(ContentRef::ArgumentNamed(name)) if name.as_str() == "title")), "{found:?}");
    // Without it, the exercises are not listed.
    assert!(entries(&registry(), DOCUMENT).is_empty());
}

// --- a failing handler -------------------------------------------------------------------------

#[test]
fn a_points_handler_that_throws_leaves_the_exercise_without_a_point() {
    let registry = common::registry_with(exercises_edited(
        |main| main.replace("function points(node) {", "function points(node) {\n  throw new Error(\"no points today\");"),
        &["exercises"],
    ));
    let (text, diagnostics) = text_of(&registry, "\\begin{exercise}[A title]\\label{ex:first}\nSolve it.\n\\end{exercise}\n\nSee \\ref{ex:first}.");
    // Nothing is reported where the handler fails (the aspect has no place to report
    // to); the reference is the references feature's unresolved-reference Error, and
    // the heading has no number.
    assert_eq!(diagnostics, ["flm.refs.unresolved: the reference points at the label `ex:first`, which is not defined"]);
    assert!(text.contains("Exercise (A title)"), "{text}");
    assert!(text.contains("Solve it."), "{text}");
}
