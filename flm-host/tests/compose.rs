//! The composer and the templates, in memory: the templates `simple` and `none` on the three
//! outputs, the identifiers and the table of the regions on HTML, the pieces a template
//! accepts, a template chosen by each source of settings, a template's own settings and
//! those it declares, a template another package contributes, and the table of contents
//! placed by `simple`. No test touches the disk.

use std::sync::Arc;

use flm_core::core::registry::{BuildError, FeatureBuilder, FeatureInfo, FlmFeature, PackageEntry, PackageFactory};
use flm_core::core::settings::{MARKS_KEYWORD, SettingsDecl};
use flm_core::techy::error::DiagnosticInfo;
use flm_core::model::names::Name;
use flm_core::progress::Scope;
use flm_host::files::MemoryFiles;
use flm_host::output::{MemorySink, NamedFile, Sink};
use flm_host::packages::ExtensionFolder;
use flm_host::report::{ContentOffsets, Record};
use flm_host::seed::front_matter_schema;
use flm_host::session::{Built, Input, PrepareErrorKind, Request, build, prepare};
use flm_host::settings::{CommandLineSetting, UnknownSetting};
use flm_core::stdlib::features::compose::{
    self, Accepts, ComposedKind, ComposedRegions, HTML_REGIONS, TEMPLATES_SLOT, Template, region_id,
};
use serde_json::{Map, Value};

const PAPER: &str = "---\ntitle: A \\emph{fine} paper\nauthor: [Ann, Bob]\ndate: Today\nkeywords: [a, b]\n---\n\
Hello world\\footnote{A note.}, with \\(x\\).\n";

fn request(format: &str, options: &[&str]) -> Request {
    let mut request = Request::new(Input::File(String::from("doc/paper.flm")));
    request.default_math = flm_host::seed::MathChoice::processor("passthrough");
    request.profile = Some(String::from(format));
    request.options = options.iter().map(|text| CommandLineSetting::new(*text, format!("-O {text}"))).collect();
    request
}

/// Builds `request` over one file holding `text`, with the compiled packages `compiled`, and
/// gives the run and the text of the entry the sink received.
///
/// A document with errors is written too, with the diagnostics beside it: the cited works
/// of these tests have no source of details, which is an error of the document.
fn render_with(text: &str, mut request: Request, compiled: &[PackageFactory]) -> (Built, String) {
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    request.continue_with_errors = true;
    let files = MemoryFiles::new().with("doc/paper.flm", text);
    let prepared = prepare(request, Arc::new(files), compiled, &Scope::none()).expect("prepared");
    let built = build(&prepared, &Scope::none()).expect("built");
    assert!(
        built.diagnostics.iter().all(|d| !d.identifier().starts_with("flm.compose")),
        "{:?}",
        built.diagnostics
    );
    let received = sink.files();
    let entry = received.values().find(|file| file.entry).expect("an output");
    let entry = String::from_utf8(entry.bytes.clone()).unwrap();
    (built, entry)
}

fn render(text: &str, format: &str, options: &[&str]) -> (Built, String) {
    let (built, entry) = render_with(text, request(format, options), &[]);
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    (built, entry)
}

// --- test 20: HTML ---------------------------------------------------------------------------

#[test]
fn the_simple_page_holds_the_metadata_the_content_the_pieces_the_styles_and_the_scripts() {
    let (_, page) = render(PAPER, "html", &[]);
    assert!(page.starts_with("<!DOCTYPE html>"), "{page}");
    assert!(page.contains("<title>A fine paper</title>"), "{page}");
    let at = |needle: &str| page.find(needle).unwrap_or_else(|| panic!("{needle} in {page}"));
    let header = at("<header");
    let title = at("id=\"flm-region-flm.metadata.title\"");
    let authors = at("id=\"flm-region-flm.metadata.author\"");
    let date = at("id=\"flm-region-flm.metadata.date\"");
    let content = at("id=\"flm-region-content\"");
    let notes = at("id=\"flm-region-flm.xnotes.list.footnote\"");
    assert!(header < title && title < authors && authors < date && date < content && content < notes);
    assert!(at("<style") < header);
    assert!(at("<script") > notes);
    assert!(page.contains("<em"), "the title is FLM: {page}");
}

#[test]
fn the_none_template_gives_the_content_and_the_pieces_alone() {
    let (_, text) = render(PAPER, "html", &["compose.template=none"]);
    assert!(!text.contains("<html") && !text.contains("<style") && !text.contains("<script"), "{text}");
    assert!(text.starts_with("<div id=\"flm-region-flm.metadata.title\">"), "{text}");
    assert!(text.contains("<div id=\"flm-region-content\">"), "{text}");
    assert!(text.contains("<div id=\"flm-region-flm.xnotes.list.footnote\">"), "{text}");
}

#[test]
fn every_region_placed_carries_its_identifier_and_the_table_holds_its_html() {
    for template in ["simple", "none"] {
        let option = format!("compose.template={template}");
        let (built, page) = render(PAPER, "html", &[option.as_str()]);
        let table = built.run.table(HTML_REGIONS).unwrap().expect("the composer's table");
        assert!(table.get(&region_id("content")).is_some());
        for region in &table.regions {
            match region.kind {
                ComposedKind::Html => {
                    let placed = format!("<div id=\"{}\">\n{}\n</div>", region.id, region.text);
                    assert!(page.contains(&placed), "{} in {page}", region.id);
                }
                // The page's title is the one region of words.
                ComposedKind::Words => assert_eq!(region.id, region_id("title")),
            }
        }
        let title = table.get(&region_id("title")).expect("the title's words");
        assert_eq!((title.kind, title.text.as_str()), (ComposedKind::Words, "A fine paper"));
    }
}

#[test]
fn a_changed_title_changes_the_title_s_regions_and_not_the_skeleton_of_a_page_without_its_words() {
    let composed = |text: &str| {
        let (built, _) = render(text, "html", &[]);
        built.run.table(HTML_REGIONS).unwrap().unwrap().clone()
    };
    let first = composed(PAPER);
    let again = composed(PAPER);
    assert_eq!(first.skeleton, again.skeleton);
    let changed = composed(&PAPER.replace("A \\emph{fine} paper", "Another paper"));
    assert_eq!(first.skeleton, changed.skeleton);
    let title = region_id("flm.metadata.title");
    assert_ne!(first.get(&title), changed.get(&title));
    assert_ne!(first.get(&region_id("title")), changed.get(&region_id("title")));
    assert_eq!(first.get(&region_id("content")), changed.get(&region_id("content")));
    assert!(!first.skeleton.contains("fine") && !first.skeleton.contains("Hello"), "{}", first.skeleton);
}

#[test]
fn a_formula_s_script_changes_the_skeleton() {
    let skeleton = |text: &str| {
        let (built, _) = render(text, "html", &[]);
        built.run.table(HTML_REGIONS).unwrap().unwrap().skeleton.clone()
    };
    let plain = skeleton("Hello.\n");
    let with_formula = skeleton("Hello \\(x\\).\n");
    assert!(!plain.contains("<script") && with_formula.contains("<script"), "{with_formula}");
    assert_ne!(plain, with_formula);
    assert!(with_formula.contains("<style"), "the stylesheets stay in the skeleton: {with_formula}");
    let (built, _) = render("Hello.\n", "html", &["html.lang=en"]);
    let skeleton = &built.run.table(HTML_REGIONS).unwrap().unwrap().skeleton;
    assert!(skeleton.contains("<html lang=\"en\">"), "{skeleton}");
}

// --- test 21, the part that needs no TeX: `none` on the recomposed LaTeX ------------------------

#[test]
fn the_none_template_of_the_recomposed_latex_is_the_body_the_library_makes() {
    let (_, simple) = render(PAPER, "latex-recomposed", &[]);
    let (_, none) = render(PAPER, "latex-recomposed", &["compose.template=none"]);
    assert!(simple.starts_with("\\documentclass{article}"), "{simple}");
    assert!(simple.contains("\\title{A \\emph{fine} paper}") && simple.contains("\\author{Ann \\and Bob}"), "{simple}");
    // The library's fixed skeleton writes the body after one comment line naming the style
    // file; `none` is that body.
    let fixed = format!("---\nflm: {{features: {{compose: false}}}}\n{}", &PAPER[4..]);
    let (_, skeleton) = render(&fixed, "latex-recomposed", &[]);
    let (comment, body) = skeleton.split_once('\n').unwrap();
    assert!(comment.starts_with("% flm: style file "), "{skeleton}");
    assert_eq!(none, body);
    assert!(simple.contains(body), "{simple}");
}

// --- test 22: plain text ---------------------------------------------------------------------

#[test]
fn simple_and_none_on_plain_text_are_the_backend_s_text() {
    let (_, simple) = render(PAPER, "text", &[]);
    let (_, none) = render(PAPER, "text", &["compose.template=none"]);
    assert!(simple.starts_with("A fine paper\n\nAnn\n\nBob\n\nToday\n\nHello world"), "{simple}");
    assert_eq!(simple, none);
}

// --- test 23: a piece the template does not accept is not built ----------------------------------

#[test]
fn a_piece_the_template_does_not_accept_is_not_built() {
    let stages = |format: &str| {
        let files = MemoryFiles::new().with("doc/paper.flm", PAPER);
        let prepared = prepare(request(format, &[]), Arc::new(files), &[], &Scope::none()).expect("prepared");
        prepared.plan().stage_names().iter().map(|name| name.to_string()).collect::<Vec<_>>()
    };
    let builder = "flm.xnotes.build-list.footnote";
    // The recomposed LaTeX's `simple` accepts the pieces of the metadata alone.
    assert!(!stages("latex-recomposed").iter().any(|name| name == builder));
    assert!(stages("text").iter().any(|name| name == builder));
}

// --- tests 24 and 27: a template another package contributes ---------------------------------

/// A compiled package `marked` contributing, for each output, the template `marked`, which
/// prints each region between neutral markers, and which accepts every piece.
fn marked_package() -> PackageFactory {
    struct Marked;
    impl FlmFeature for Marked {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new("marked", "A template printing each region between markers")
        }
        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            for output in ["html", "text", "latex-recomposed"] {
                let template = Template {
                    name: String::from("marked"),
                    package: String::from("marked"),
                    output: String::from(output),
                    engine: String::from("minijinja"),
                    text: String::from(
                        "{% for name, text in regions|items %}=== begin {{ name }} ===\n{{ text }}\n=== end {{ name }} ===\n{% endfor %}\
                         === metadata {{ metadata.keywords|join(\",\") }} ===\n",
                    ),
                    source: None,
                    accepts: Accepts::All,
                    requires: vec![Name::from("flm.metadata.title")],
                    config: Map::new(),
                    settings: None,
                    delimiters: None,
                    artifacts: Vec::new(),
                };
                b.contribute_to::<Template>(TEMPLATES_SLOT, Arc::new(template))?;
            }
            Ok(())
        }
    }
    PackageFactory::new("marked", |_| Ok(PackageEntry::new("marked").feature(Arc::new(Marked))))
        .unwrap()
        .uses("std")
}

const CITED: &str = "---\ntitle: A paper\nkeywords: [a, b]\nflm: {use: [marked]}\n---\n\
Hello\\footnote{A note.} and \\cite{arxiv:1234.5678}.\n";

#[test]
fn a_template_printing_each_region_between_markers_shows_the_regions() {
    for format in ["text", "html", "latex-recomposed"] {
        let (_, text) = render_with(CITED, request(format, &["compose.template=marked"]), &[marked_package()]);
        assert!(text.contains("=== begin content ==="), "{format}: {text}");
        assert!(text.contains("=== metadata a,b ==="), "{format}: {text}");
        // The text backend lays the pieces out in its one region, the content.
        if format != "text" {
            assert!(text.contains("=== begin flm.metadata.title ==="), "{format}: {text}");
            assert!(text.contains("=== begin flm.xnotes.list.footnote ==="), "{format}: {text}");
            assert!(text.contains("=== begin flm.citations.bibliography ==="), "{format}: {text}");
        }
    }
}

#[test]
fn a_template_another_package_contributes_is_found_by_its_name() {
    let (_, text) = render_with(CITED, request("text", &["compose.template=marked"]), &[marked_package()]);
    assert!(text.starts_with("=== begin content ==="), "{text}");
}

/// A compiled package `worded` contributing the HTML template `worded`, which writes the
/// title piece's plain words in a `<meta>` tag and places the content.
fn worded_package() -> PackageFactory {
    struct Worded;
    impl FlmFeature for Worded {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new("worded", "A template writing the title's words in the head")
        }
        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            let template = Template {
                name: String::from("worded"),
                package: String::from("worded"),
                output: String::from("html"),
                engine: String::from("minijinja"),
                text: String::from(
                    "<html><head><meta name=\"description\" content=\"{{ pieces[\"flm.metadata.title\"].words }}\">\
                     </head><body>{{ regions[\"content\"] }}</body></html>\n",
                ),
                source: None,
                accepts: Accepts::Only(vec![Name::from("flm.metadata.title")]),
                requires: vec![Name::from("flm.metadata.title")],
                settings: None,
                config: Map::new(),
                delimiters: None,
                artifacts: Vec::new(),
            };
            b.contribute_to::<Template>(TEMPLATES_SLOT, Arc::new(template))?;
            Ok(())
        }
    }
    PackageFactory::new("worded", |_| Ok(PackageEntry::new("worded").feature(Arc::new(Worded))))
        .unwrap()
        .uses("std")
}

#[test]
fn a_piece_s_words_a_template_writes_are_in_the_skeleton() {
    let composed = |title: &str| {
        let text = format!("---\ntitle: {title}\nflm: {{use: [worded]}}\n---\nHello.\n");
        let (built, page) = render_with(&text, request("html", &["compose.template=worded"]), &[worded_package()]);
        assert!(!built.has_errors(), "{:?}", built.diagnostics);
        assert!(page.contains(&format!("content=\"{title}\"")), "{page}");
        built.run.table(HTML_REGIONS).unwrap().unwrap().clone()
    };
    let first = composed("A paper");
    let changed = composed("Another paper");
    assert!(first.skeleton.contains("content=\"A paper\""), "{}", first.skeleton);
    assert_ne!(first.skeleton, changed.skeleton);
    let ids = |table: &ComposedRegions| {
        table.regions.iter().map(|region| region.id.clone()).collect::<Vec<_>>()
    };
    assert_eq!(ids(&first), ids(&changed));
    assert_eq!(first.get(&region_id("content")), changed.get(&region_id("content")));
}

// --- test 25: the template chosen by each source --------------------------------------------

fn chosen(front_matter: &str, options: &[&str]) -> String {
    let text = format!("---\ntitle: T\n{front_matter}---\nHello.\n");
    render(&text, "html", options).1
}

#[test]
fn the_template_is_chosen_by_the_command_line_over_the_front_matter() {
    let simple = |page: &str| page.starts_with("<!DOCTYPE html>");
    assert!(simple(&chosen("", &[])));
    assert!(!simple(&chosen("flm: {features: {compose: {template: none}}}\n", &[])));
    assert!(!simple(&chosen("flm: {features: {compose: {template: {html: none}}}}\n", &[])));
    assert!(simple(&chosen("flm: {features: {compose: {template: {text: none}}}}\n", &[])));
    assert!(simple(&chosen("flm: {features: {compose: {template: none}}}\n", &["compose.template=simple"])));
    assert!(!simple(&chosen("", &["compose.template={html: none}"])));
}

#[test]
fn a_template_no_name_matches_stops_the_work() {
    let files = MemoryFiles::new().with("doc/paper.flm", "Hello.\n");
    let error = prepare(request("html", &["compose.template=nosuch"]), Arc::new(files), &[], &Scope::none())
        .err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Template);
}

#[test]
fn switching_compose_standard_off_leaves_no_template_of_std() {
    // `compose` stays, with its engine `content-only`; `std`'s templates and `minijinja`
    // are gone, so the default choice `simple` names no template for the output.
    let text = "---\nflm: {features: {compose-standard: false}}\n---\nHello.\n";
    let files = MemoryFiles::new().with("doc/paper.flm", text);
    let error = prepare(request("html", &[]), Arc::new(files), &[], &Scope::none()).err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Template);
}

#[test]
fn a_problem_with_a_command_line_setting_names_the_words_the_user_wrote() {
    let text = "---\nflm: {features: {compose: {template: simple}}}\n---\nHello.\n";
    let files = MemoryFiles::new().with("doc/paper.flm", text);
    let mut request = request("html", &[]);
    request.options.push(CommandLineSetting::new("compose.template='nosuch'", "-t nosuch"));
    let error = prepare(request, Arc::new(files), &[], &Scope::none()).err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Template);
    let problem = &error.problems[0];
    // Placed at the command line, not at the front matter's value it replaced.
    assert_eq!((problem.file.as_deref(), problem.place), (None, None));
    assert!(problem.detail.contains("-t nosuch"), "{}", problem.detail);
}

#[test]
fn a_required_piece_nobody_provides_fails_the_plan() {
    let text = "---\nflm: {use: [marked], features: {metadata: false}}\n---\nHello.\n";
    let files = MemoryFiles::new().with("doc/paper.flm", text);
    let error = prepare(request("text", &["compose.template=marked"]), Arc::new(files), &[marked_package()], &Scope::none())
        .err()
        .unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Plan);
}

// --- test 26: a template's own settings --------------------------------------------------------

/// A package contributing, for plain text, the template `greeting`, whose own settings are
/// `greeting` and `name` with their defaults.
fn greeting_package() -> PackageFactory {
    struct Greeting;
    impl FlmFeature for Greeting {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new("greeting", "A template with settings of its own")
        }
        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            let mut config = Map::new();
            config.insert(String::from("greeting"), Value::from("Hello"));
            config.insert(String::from("name"), Value::from("world"));
            let template = Template {
                name: String::from("greeting"),
                package: String::from("greeting"),
                output: String::from("text"),
                engine: String::from("minijinja"),
                text: String::from("{{ config.greeting }}, {{ config.name }}!\n"),
                source: None,
                accepts: Accepts::All,
                settings: None,
                requires: Vec::new(),
                config,
                delimiters: None,
                artifacts: Vec::new(),
            };
            b.contribute_to::<Template>(TEMPLATES_SLOT, Arc::new(template))
        }
    }
    PackageFactory::new("greeting", |_| Ok(PackageEntry::new("greeting").feature(Arc::new(Greeting))))
        .unwrap()
        .uses("std")
}

#[test]
fn template_config_reaches_the_template_over_its_defaults() {
    let text = "---\nflm: {use: [greeting]}\n---\nHello.\n";
    let greet = |options: &[&str]| {
        let mut all = vec!["compose.template=greeting"];
        all.extend_from_slice(options);
        render_with(text, request("text", &all), &[greeting_package()]).1
    };
    assert_eq!(greet(&[]), "Hello, world!\n");
    assert_eq!(greet(&["compose.template_config={text: {greeting: {name: Ann}}}"]), "Hello, Ann!\n");
    // The settings are given per output: those of another output's `greeting` do not reach it.
    assert_eq!(greet(&["compose.template_config={html: {greeting: {name: Ann}}}"]), "Hello, world!\n");
}

// --- the page's title and language ------------------------------------------------------------

#[test]
fn the_render_options_give_the_page_s_language_and_title_under_the_simple_template() {
    let untitled = "Hello.\n";
    let (_, page) = render(untitled, "html", &[]);
    assert!(page.contains("<html>") && page.contains("<title>FLM document</title>"), "{page}");
    let (_, page) = render(untitled, "html", &["html.lang=en", "html.title=My <page>"]);
    assert!(page.contains("<html lang=\"en\">"), "{page}");
    assert!(page.contains("<title>My &lt;page&gt;</title>"), "{page}");
}

#[test]
fn the_document_s_title_is_the_page_s_title_under_both_assemblers() {
    let paper = PAPER.replace("A \\emph{fine} paper", "Cats & \\emph{dogs}");
    let fixed = format!("---\nflm: {{features: {{compose: false}}}}\n{}", &paper[4..]);
    for text in [paper.as_str(), fixed.as_str()] {
        let (_, page) = render(text, "html", &["html.title=Unused"]);
        assert!(page.contains("<title>Cats &amp; dogs</title>"), "{page}");
    }
}

// --- the table of contents under the simple template -----------------------------------------

/// A document with a title, a section holding a subsection holding a subsubsection, and a
/// second section, whose front matter places the table of contents `flm.toc` with the
/// depth 2.
const TOC_PAPER: &str = "---\ntitle: A paper\nflm: {features: {pieces: {include: [flm.toc]}, toc: {depth: 2}}}\n---\n\
\\section{One}\n\nHello.\n\n\\subsection{Two}\n\nMore.\n\n\\subsubsection{Three}\n\nDeeper.\n\n\\section{Four}\n\nEnd.\n";

/// `TOC_PAPER` without the include: the same settings otherwise, no table of contents.
fn toc_paper_without_the_include() -> String {
    TOC_PAPER.replace("pieces: {include: [flm.toc]}, ", "")
}

/// The part of `page` from `<body>` on.
fn body(page: &str) -> &str {
    &page[page.find("<body>").expect("a body")..]
}

#[test]
fn the_simple_page_places_the_table_of_contents_between_the_header_and_the_content() {
    let (_, page) = render(TOC_PAPER, "html", &[]);
    let at = |needle: &str| page.find(needle).unwrap_or_else(|| panic!("{needle} in {page}"));
    let header_end = at("</header>");
    let region = at("<div id=\"flm-region-flm.toc\">");
    let nav = at("<nav class=\"flm-pres-block flm-toc\" role=\"doc-toc\" data-flm-kind=\"toc\">");
    let content = at("<div id=\"flm-region-content\">");
    assert!(at("<main") < at("<header>"), "the header is inside the content element: {page}");
    assert!(header_end < region && region < nav && nav < content, "{page}");
    assert_eq!(page.matches("<nav").count(), 1, "{page}");
    let nav_text = &page[nav..page.find("</nav>").unwrap()];
    assert!(nav_text.contains("href=\"#sec--One\"") && nav_text.contains(">One</a>"), "{nav_text}");
    assert!(nav_text.contains("href=\"#sec--Two\"") && nav_text.contains(">Two</a>"), "{nav_text}");
    assert!(nav_text.contains("href=\"#sec--Four\""), "{nav_text}");
    // The depth 2 of the front matter leaves the subsubsection out.
    assert!(!nav_text.contains("Three"), "{nav_text}");
    // The header holds the metadata alone.
    assert!(!page[..header_end].contains("flm-region-flm.toc"), "{page}");
}

#[test]
fn without_the_include_the_simple_page_has_no_table_of_contents_and_is_otherwise_unchanged() {
    let (_, with) = render(TOC_PAPER, "html", &[]);
    let (_, without) = render(&toc_paper_without_the_include(), "html", &[]);
    assert!(!without.contains("<nav") && !without.contains("flm-region-flm.toc"), "{without}");
    // The body of the page with the table of contents, its region taken out, is the body of
    // the page without it. (The head differs by the table of contents' stylesheet.)
    let region_start = with.find("<div id=\"flm-region-flm.toc\">").unwrap();
    let region_end = region_start + with[region_start..].find("</nav>\n</div>\n").unwrap() + "</nav>\n</div>\n".len();
    let taken_out = format!("{}{}", &with[..region_start], &with[region_end..]);
    assert_eq!(body(&taken_out), body(&without));
}

#[test]
fn the_render_request_s_depth_replaces_the_document_s() {
    let (_, page) = render(TOC_PAPER, "html", &["toc.depth=1"]);
    let nav = page.find("<nav").unwrap_or_else(|| panic!("a table of contents in {page}"));
    let nav_text = &page[nav..page.find("</nav>").unwrap()];
    assert!(nav_text.contains("href=\"#sec--One\"") && nav_text.contains("href=\"#sec--Four\""), "{nav_text}");
    assert!(!nav_text.contains("Two") && !nav_text.contains("Three"), "{nav_text}");
    assert_eq!(nav_text.matches("<ul").count(), 1, "{nav_text}");
}

// --- the fixed skeleton stays the library's -------------------------------------------------

#[test]
fn switching_the_composer_off_gives_the_library_s_fixed_skeleton() {
    let (_, page) = render(&format!("---\nflm: {{features: {{compose: false}}}}\n{}", &PAPER[4..]), "html", &[]);
    assert!(page.contains("<main class=\"flm-content\" id=\"flm-content\">"), "{page}");
    assert!(!page.contains("flm-region-"), "{page}");
}

// --- the set the composer writes -------------------------------------------------------------

/// A package whose one stage writes, into the page's artifact table, a file marked as the
/// output's entry: a set with two entries once the composer adds the page. The `graphics`
/// feature, the table's usual writer, is switched off by the document that uses it, and the
/// package's workflow `second-entry`, the page's with the stage's capability demanded, has the
/// stage run.
fn second_entry_package() -> PackageFactory {
    use flm_core::core::artifact::{Artifact, ArtifactId, ArtifactSet};
    use flm_core::core::data::store::StageOut;
    use flm_core::core::plan::PlanCx;
    use flm_core::core::registry::{CapabilityDef, WorkflowDef};
    use flm_core::core::run::StageCx;
    use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
    use flm_core::stdlib::backends::html;

    struct WriteEntry;
    impl Stage for WriteEntry {
        fn decl(&self) -> StageDecl {
            StageDecl::new("second-entry.write", Effect::Compute, Scope::Document)
                .provides("second-entry.files")
                .writes(html::ARTIFACTS)
        }
        fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
            Needs::new()
        }
        fn run(&self, _cx: &StageCx<'_>, out: &mut StageOut<'_>) {
            let set: &mut ArtifactSet = out.table(html::ARTIFACTS).expect("declared");
            let other = Artifact::bytes(ArtifactId::new("second-entry", "page"), "text/html", b"<p>other</p>".to_vec());
            set.insert(other.as_entry()).expect("the table's only entry");
        }
    }
    struct SecondEntry;
    impl FlmFeature for SecondEntry {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new("second-entry", "A second entry written beside the page")
        }
        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            b.peer_table::<ArtifactSet>(html::ARTIFACTS.name(), "the page's artifact table")?;
            b.capability(CapabilityDef::new("second-entry.files"))?;
            b.stage(Arc::new(WriteEntry))?;
            b.workflow(
                WorkflowDef::new("second-entry")
                    .extends(html::WORKFLOW)
                    .demand(flm_core::stdlib::VALIDATED)
                    .demand("second-entry.files"),
            )
        }
    }
    PackageFactory::new("second-entry", |_| Ok(PackageEntry::new("second-entry").feature(Arc::new(SecondEntry))))
        .unwrap()
        .uses("std")
}

#[test]
fn the_composer_reports_a_second_entry_beside_the_page_it_makes() {
    let text = "---\nflm: {use: [second-entry], features: {graphics: false}}\n---\nHello.\n";
    let mut request = request("html", &[]);
    request.profile = Some(String::from("second-entry"));
    let (built, page) = render_with(text, request, &[second_entry_package()]);
    // Switching `graphics` off leaves a Note from a feature that names its command.
    let errors: Vec<&str> = built
        .diagnostics
        .iter()
        .filter(|d| d.severity() == flm_core::techy::error::Severity::Error)
        .map(|d| d.identifier())
        .collect();
    assert_eq!(errors, ["flm.artifacts.second-entry"], "{:?}", built.diagnostics);
    assert!(page.contains("Hello."), "{page}");
    assert!(page.contains("flm-region-"), "{page}");
}

// --- an output made from another output -------------------------------------------------------

/// A package `copied` whose output `copied` is made from `latex-rendered`: its stage copies
/// the main file of `latex-rendered` as its own, and its workflow `copied` asks for that
/// output alone. The package also ships the `latex-rendered` compose template `marked`,
/// which writes a marker line before the content.
fn copied_package() -> PackageFactory {
    use flm_core::core::artifact::{Artifact, ArtifactId, ArtifactSet};
    use flm_core::core::data::store::StageOut;
    use flm_core::core::data::table::TableName;
    use flm_core::core::plan::PlanCx;
    use flm_core::core::registry::{OutputDef, WorkflowDef};
    use flm_core::core::run::StageCx;
    use flm_core::core::stage::{Effect, Needs, Scope, Stage, StageDecl};
    use flm_core::stdlib::backends::latex_rendered;

    const DOCUMENT: TableName<ArtifactSet> = TableName::new("copied.document");

    struct Copy;
    impl Stage for Copy {
        fn decl(&self) -> StageDecl {
            StageDecl::new("copied.copy", Effect::Compute, Scope::Document).writes(DOCUMENT)
        }
        fn needs(&self, _cx: &PlanCx<'_>) -> Needs {
            Needs::new().reads(latex_rendered::DOCUMENT)
        }
        fn run(&self, cx: &StageCx<'_>, out: &mut StageOut<'_>) {
            let Some(entry) = cx.get(latex_rendered::DOCUMENT).expect("read").and_then(ArtifactSet::entry) else {
                return;
            };
            let bytes = entry.as_bytes().expect("bytes").to_vec();
            let copy = Artifact::bytes(ArtifactId::new("copied", "document"), "application/x-latex", bytes);
            out.table(DOCUMENT).expect("declared").insert(copy.as_entry()).expect("the only entry");
        }
    }
    struct Copied;
    impl FlmFeature for Copied {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new("copied", "An output made from the files of latex-rendered")
        }
        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            b.peer_table::<ArtifactSet>(latex_rendered::DOCUMENT.name(), "the files of latex-rendered")?;
            b.table(DOCUMENT, "the copy")?;
            b.output(
                OutputDef::new("copied", "copied.document", "copied.document", "application/x-latex", "tex")
                    .made_from(latex_rendered::OUTPUT),
            )?;
            b.stage(Arc::new(Copy))?;
            b.workflow(WorkflowDef::new("copied").extends(latex_rendered::WORKFLOW).output("copied"))?;
            let template = Template {
                name: String::from("marked"),
                package: String::from("copied"),
                output: String::from(latex_rendered::OUTPUT),
                engine: String::from("minijinja"),
                text: String::from("%% marked\n{{ regions[\"content\"] }}\n"),
                source: None,
                settings: None,
                accepts: Accepts::All,
                requires: Vec::new(),
                config: Map::new(),
                delimiters: None,
                artifacts: Vec::new(),
            };
            b.contribute_to::<Template>(TEMPLATES_SLOT, Arc::new(template))?;
            Ok(())
        }
    }
    PackageFactory::new("copied", |_| Ok(PackageEntry::new("copied").feature(Arc::new(Copied))))
        .unwrap()
        .uses("std")
}

const COPIED: &str = "---\nflm: {use: [copied]}\n---\nHello.\n";

#[test]
fn the_compose_template_applies_to_the_output_the_requested_output_is_made_from() {
    let copied = |options: &[&str]| {
        let (built, text) = render_with(COPIED, request("copied", options), &[copied_package()]);
        assert!(!built.has_errors(), "{:?}", built.diagnostics);
        text
    };
    // The default compose template, `simple`, makes a whole LaTeX document.
    let simple = copied(&[]);
    assert!(simple.contains("\\documentclass") && !simple.contains("%% marked"), "{simple}");
    // One name for every output, as `-t marked` gives it, and a map by output.
    for option in ["compose.template=marked", "compose.template={latex-rendered: marked}"] {
        let marked = copied(&[option]);
        assert!(marked.starts_with("%% marked\n") && marked.contains("Hello."), "{option}: {marked}");
    }
    // A map that sets the template of another output leaves `latex-rendered` at `simple`.
    let other = copied(&["compose.template={html: none}"]);
    assert!(other.contains("\\documentclass"), "{other}");
}

#[test]
fn a_compose_template_missing_for_the_output_made_from_stops_the_work() {
    let files = MemoryFiles::new().with("doc/paper.flm", COPIED);
    let error = prepare(request("copied", &["compose.template=nosuch"]), Arc::new(files), &[copied_package()], &Scope::none())
        .err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Template);
    assert_eq!(error.problems.len(), 1, "one problem: {:?}", error.problems);
}

// --- the files a compose template reads, by a path with its root ------------------------------

/// Builds `doc/paper.flm` of `files` to HTML, with the extensions of the folder `defs` of
/// `files`, and gives the text of the entry and every file the sink received.
fn render_with_extensions(files: MemoryFiles) -> (String, Vec<NamedFile>) {
    let (_, page, received) = build_with_extensions(files);
    (page, received)
}

/// Builds `doc/paper.flm` of `files` as [`render_with_extensions`] does, and gives the run
/// too.
fn build_with_extensions(files: MemoryFiles) -> (Built, String, Vec<NamedFile>) {
    let sink = Arc::new(MemorySink::new());
    let mut request = request("html", &[]);
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    request.extension_folders = vec![ExtensionFolder { path: String::from("defs"), required: false }];
    let prepared = prepare(request, Arc::new(files), &[], &Scope::none()).expect("prepared");
    let built = build(&prepared, &Scope::none()).expect("built");
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    let received: Vec<NamedFile> = sink.files().into_values().collect();
    let entry = received.iter().find(|file| file.entry).expect("an output");
    let page = String::from_utf8(entry.bytes.clone()).unwrap();
    (built, page, received)
}

#[test]
fn a_package_names_its_own_files_in_the_settings_of_the_compose_template_of_another() {
    // `basetemplate` applies the text setting `style` with `eval` and ships the file of
    // the path setting `logo`; `mytemplate`, which uses it, writes the text with its own
    // root and the path bare; the document uses `mytemplate` and writes a third setting.
    let base = "name: base\noutput: html\nengine: minijinja\nmain: base.html\naccepts: all\n\
                settings:\n  type: object\n  properties:\n    style: {type: string, default: ''}\n    \
                logo: {type: string, default: 'ext+basetemplate:logo.png', x-flm-marks: [path]}\n    \
                note: {type: string, default: ''}\n";
    let page = "<style>{{ eval(config.style, html=false) }}</style><img src=\"{{ include_artifact_url(config.logo) }}\">\
                <p>{{ config.note }}</p>{{ regions.content }}\n";
    let mine = "flm:\n  use: [basetemplate]\n  features:\n    compose:\n      template: {html: base}\n      \
                template_config:\n        html:\n          base:\n            \
                style: '{{ include_raw(\"ext+mytemplate:style.css\") }}'\n            \
                logo: logo.png\n";
    let doc = "---\nflm:\n  use: [mytemplate]\n  features:\n    compose:\n      template_config:\n        html:\n          \
               base:\n            note: From the document\n---\nHello.\n";
    let files = MemoryFiles::new()
        .with("doc/paper.flm", doc)
        .with("defs/basetemplate/flmextension.yaml", "flm: {}\n")
        .with("defs/basetemplate/templates/html/base/flmtemplate.yaml", base)
        .with("defs/basetemplate/templates/html/base/base.html", page)
        .with("defs/basetemplate/logo.png", "the logo of basetemplate")
        .with("defs/mytemplate/flmextension.yaml", mine)
        .with("defs/mytemplate/style.css", "p { color: teal; }")
        .with("defs/mytemplate/logo.png", "the logo of mytemplate");
    let (page, received) = render_with_extensions(files);
    assert!(page.starts_with("<style>p { color: teal; }</style><img src=\""), "{page}");
    assert!(page.contains("<p>From the document</p>"), "{page}");
    let linked = page.split_once("<img src=\"").and_then(|(_, rest)| rest.split_once('"')).map(|(name, _)| name);
    let linked = linked.unwrap_or_else(|| panic!("{page}"));
    let logo = received.iter().find(|file| file.path.ends_with(linked)).unwrap_or_else(|| panic!("{linked}: {received:?}"));
    assert_eq!(logo.bytes, b"the logo of mytemplate");
}

#[test]
fn a_compose_template_extends_a_text_of_another_extension_read_in_that_text_s_folder() {
    let child = "name: child\noutput: html\nengine: minijinja\nmain: child.html\naccepts: all\n";
    let files = MemoryFiles::new()
        .with("doc/paper.flm", "---\nflm:\n  use: [child]\n  features:\n    compose: {template: {html: child}}\n---\nHello.\n")
        .with("defs/child/flmextension.yaml", "flm:\n  use: [parent]\n")
        .with("defs/child/templates/html/child/flmtemplate.yaml", child)
        .with(
            "defs/child/templates/html/child/child.html",
            "{% extends \"ext+parent:templates/html/frame/frame.html\" %}{% block body %}[{% include \"part.html\" %}]{% endblock %}",
        )
        .with("defs/child/templates/html/child/part.html", "the child's part")
        .with("defs/parent/flmextension.yaml", "flm: {}\n")
        .with(
            "defs/parent/templates/html/frame/frame.html",
            "<header>{% include \"partials/top.html\" %}</header>{% block body %}{% endblock %}\n",
        )
        .with("defs/parent/templates/html/frame/partials/top.html", "the parent's top")
        .with("defs/parent/templates/html/frame/part.html", "the parent's part");
    let (page, _) = render_with_extensions(files);
    // The extended text reads its partial in its own folder; the block of the child, its
    // own.
    assert_eq!(page, "<header>the parent's top</header>[the child's part]");
}

// --- the settings a compose template declares -------------------------------------------------

/// The extension `basetemplate`, whose compose template `base` for HTML declares the
/// settings `settings` (YAML, indented by four spaces) and writes `page`, and the document
/// `doc` beside it.
fn declaring(settings: &str, page: &str, doc: &str) -> MemoryFiles {
    let base = format!(
        "name: base\noutput: html\nengine: minijinja\nmain: base.html\naccepts: all\n\
         settings:\n  type: object\n  properties:\n{settings}"
    );
    MemoryFiles::new()
        .with("doc/paper.flm", doc)
        .with("defs/basetemplate/flmextension.yaml", "flm: {}\n")
        .with("defs/basetemplate/templates/html/base/flmtemplate.yaml", base.as_str())
        .with("defs/basetemplate/templates/html/base/base.html", page)
}

#[test]
fn a_key_that_a_compose_template_does_not_declare_gives_one_warning_and_does_not_reach_it() {
    let doc = "---\nflm:\n  use: [basetemplate]\n  features:\n    compose:\n      template: {html: base}\n      \
               template_config:\n        html:\n          base:\n            accentt: navy\n---\nHello.\n";
    let files = declaring("    accent: {type: string, default: teal}\n", "{{ config.accent }} {{ 'accentt' in config }}", doc);
    let (built, page, _) = build_with_extensions(files);
    assert_eq!(page, "teal False");
    let warnings: Vec<_> =
        built.diagnostics.iter().filter(|d| d.identifier() == UnknownSetting::IDENTIFIER).collect();
    assert_eq!(warnings.len(), 1, "{:?}", built.diagnostics);
    let record = serde_json::to_value(Record::of_diagnostic(warnings[0], &ContentOffsets::new())).expect("JSON");
    assert_eq!(record["severity"], "warning");
    assert_eq!(record["place"]["file"], "doc/paper.flm");
    // The place of the write that holds the key: the value of `template_config`.
    assert_eq!(record["place"]["line"], 8);
}

#[test]
fn an_object_setting_marked_merge_object_key_values_combines_a_package_and_the_front_matter_key_by_key() {
    let settings = "    colors:\n      type: object\n      default: {text: black, link: blue}\n      \
                    x-flm-marks: [merge-object-key-values]\n";
    let page = "{{ config.colors.text }} {{ config.colors.link }} {{ config.colors.rule }}";
    let doc = "---\nflm:\n  use: [mytemplate]\n  features:\n    compose:\n      template_config:\n        html:\n          \
               base:\n            colors: {text: navy}\n---\nHello.\n";
    let mine = "flm:\n  use: [basetemplate]\n  features:\n    compose:\n      template: {html: base}\n      \
                template_config:\n        html:\n          base:\n            colors: {link: green, rule: gray}\n";
    let files = declaring(settings, page, doc).with("defs/mytemplate/flmextension.yaml", mine);
    let (_, page, _) = build_with_extensions(files);
    assert_eq!(page, "navy green gray");
}

#[test]
fn the_schema_of_a_front_matter_holds_the_settings_of_the_compose_templates_with_their_marks() {
    struct Shipping;
    impl FlmFeature for Shipping {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new("shipping", "A template with declared settings")
        }
        fn settings(&self) -> Option<SettingsDecl> {
            Some(compose::declare_settings(SettingsDecl::new("shipping"), &[shipped()]).unwrap())
        }
        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            b.contribute_to::<Template>(TEMPLATES_SLOT, Arc::new(shipped()))
        }
    }
    fn shipped() -> Template {
        let settings = serde_json::json!({
            "type": "object",
            "properties": {
                "logo": { "type": "string", "default": "", "description": "The logo.", "x-flm-marks": ["path"] },
                "colors": { "type": "object", "default": {}, "x-flm-marks": ["merge-object-key-values"] },
            },
        });
        let config = settings["properties"].as_object().unwrap().iter().map(|(k, v)| (k.clone(), v["default"].clone())).collect();
        Template {
            name: String::from("shipped"),
            package: String::from("shipping"),
            output: String::from("html"),
            engine: String::from("minijinja"),
            text: String::new(),
            source: None,
            accepts: Accepts::All,
            requires: Vec::new(),
            config,
            settings: Some(settings),
            delimiters: None,
            artifacts: Vec::new(),
        }
    }
    let shipping = PackageFactory::new("shipping", |_| Ok(PackageEntry::new("shipping").feature(Arc::new(Shipping)))).unwrap();
    let std = flm_host::stdpkg::factory(flm_host::stdpkg::StdParts::new(Arc::new(flm_host::output::Naming::Folder))).unwrap();
    let schema = front_matter_schema(&[&std, &shipping], &[]);
    let outputs = &schema["$defs"]["compose"]["properties"]["template_config"]["properties"];
    let shipped = &outputs["html"]["properties"]["shipped"]["properties"];
    assert_eq!(shipped["logo"][MARKS_KEYWORD], serde_json::json!(["path"]));
    assert_eq!(shipped["logo"]["description"], "The logo.");
    assert_eq!(shipped["colors"][MARKS_KEYWORD], serde_json::json!(["merge-object-key-values"]));
    // The program's own compose templates are there too, with their defaults.
    assert_eq!(outputs["html"]["properties"]["blogpost"]["properties"]["theme"]["default"], "light");
    assert_eq!(outputs["latex-rendered"]["properties"]["article"]["properties"]["documentclass"]["default"], "article");
}

#[test]
fn the_settings_of_a_compose_template_that_no_package_in_use_ships_are_accepted() {
    let doc = "---\nflm:\n  features:\n    compose:\n      template_config:\n        html:\n          \
               nosuch: {anything: 1}\n---\nHello.\n";
    let (built, _) = render(doc, "html", &[]);
    assert!(built.diagnostics.iter().all(|d| d.identifier() != UnknownSetting::IDENTIFIER), "{:?}", built.diagnostics);
}
