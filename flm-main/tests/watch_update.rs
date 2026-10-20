//! The comparison of two builds (the module `watch::update`), over real builds of documents
//! held in memory. It performs no input or output, so these tests pass with the crate's
//! default features off.

use std::path::PathBuf;
use std::sync::Arc;

use flm_core::techy::error::Severity;
use flm_main::files::MemoryFiles;
use flm_main::output::{MemorySink, NamedFile, Naming, Sink};
use flm_main::packages::ExtensionFolder;
use flm_main::report::Reported;
use flm_main::session::{Input, Request, build, prepare};
use flm_core::stdcore::features::compose::region_id;
use flm_main::watch::build_settings;
use flm_main::watch::update::{BuildOutput, PageChange, Update, compare};

/// The document `doc/paper.flm` holding `text`.
fn files(text: &str) -> MemoryFiles {
    MemoryFiles::new().with("doc/paper.flm", text)
}

/// Builds `doc/paper.flm` of `files` as the preview does — through its errors, with the
/// settings the loop adds to every build, the output handed over in memory — and gives the
/// output, or `None` when the build gave none, and the problems.
fn output(files: MemoryFiles) -> (Option<BuildOutput>, Vec<Reported>) {
    output_of(files, None, "paper.html")
}

/// As [`output`], for the workflow `workflow` when given (with the extensions of `defs/`),
/// under the name `name`.
fn output_of(files: MemoryFiles, workflow: Option<&str>, name: &str) -> (Option<BuildOutput>, Vec<Reported>) {
    let sink = Arc::new(MemorySink::new());
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.naming = Naming::File { name: name.to_string() };
    if let Some(workflow) = workflow {
        request.profile = Some(workflow.to_string());
        request.extension_folders = vec![ExtensionFolder { path: PathBuf::from("defs"), required: false }];
    }
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    request.continue_with_errors = true;
    request.options.extend(build_settings());
    let prepared = match prepare(request, Arc::new(files), &[]) {
        Ok(prepared) => prepared,
        Err(error) => return (None, error.problems.iter().map(Reported::of_problem).collect()),
    };
    let built = build(&prepared).expect("built");
    let problems = built.diagnostics.iter().map(Reported::of_diagnostic).collect();
    let received = sink.files();
    if received.is_empty() {
        return (None, problems);
    }
    let leading = prepared.outputs()[0].clone();
    (Some(BuildOutput::of_build(&built.run, &leading, received.into_values())), problems)
}

/// The update from the document `before` to the document `after`.
fn update(before: &str, after: &str) -> Update {
    let (before, _) = output(files(before));
    let (after, problems) = output(files(after));
    Update::new(2, before.as_ref(), after.as_ref(), problems)
}

/// The identifiers of the regions an update names.
fn named(update: &Update) -> Vec<String> {
    match &update.page {
        PageChange::Regions(regions) => regions.iter().map(|region| region.id.clone()).collect(),
        other => panic!("not regions: {other:?}"),
    }
}

const DOC: &str = "---\ntitle: Notes\n---\nHello\\footnote{A first note.}.\n\nA second paragraph.\n";

/// A named file of `bytes` at `path`.
fn file(path: &str, entry: bool, bytes: &[u8]) -> NamedFile {
    NamedFile {
        path: path.to_string(),
        output: String::from("html"),
        entry,
        media_type: String::from("image/png"),
        bytes: bytes.to_vec(),
    }
}

// --- acceptance 1: nothing changed --------------------------------------------------------

#[test]
fn two_builds_of_the_same_files_give_an_update_that_names_nothing() {
    let update = update(DOC, DOC);
    assert_eq!(update.page, PageChange::Same);
    assert!(update.files.is_empty());
    assert!(update.changes_nothing());
    assert_eq!(update.to_json()["page"], "same");
}

// --- acceptance 2: a region changed --------------------------------------------------------

#[test]
fn a_change_inside_the_content_names_the_content_region_alone_with_its_new_html() {
    let update = update(DOC, &DOC.replace("A second paragraph.", "A second, longer paragraph."));
    assert_eq!(named(&update), vec![region_id("content")]);
    let PageChange::Regions(regions) = &update.page else { unreachable!() };
    assert!(regions[0].text.contains("A second, longer paragraph."), "{}", regions[0].text);
    assert!(update.files.is_empty());
    let json = update.to_json();
    assert_eq!(json["page"], "regions");
    assert_eq!(json["regions"][0]["id"], "flm-region-content");
    assert_eq!(json["regions"][0]["kind"], "html");
}

#[test]
fn a_change_to_a_footnotes_text_names_the_region_of_the_list_of_notes() {
    let update = update(DOC, &DOC.replace("A first note.", "A first note, mended."));
    let named = named(&update);
    assert!(named.contains(&region_id("flm.xnotes.list.footnote")), "{named:?}");
    assert!(!named.contains(&region_id("flm.meta.title")), "{named:?}");
}

#[test]
fn a_change_to_the_title_names_the_title_and_its_words() {
    let update = update(DOC, &DOC.replace("title: Notes", "title: Other notes"));
    let named = named(&update);
    assert!(named.contains(&region_id("flm.meta.title")), "{named:?}");
    assert!(named.contains(&region_id("title")), "{named:?}");
    assert!(!named.contains(&region_id("content")), "{named:?}");
    let PageChange::Regions(regions) = &update.page else { unreachable!() };
    let words = regions.iter().find(|region| region.id == region_id("title")).unwrap();
    assert_eq!(update.to_json()["regions"].as_array().unwrap().iter().find(|r| r["id"] == "flm-region-title").unwrap()["kind"], "words");
    assert_eq!(words.text, "Other notes");
}

// --- acceptance 3: the page loaded anew ----------------------------------------------------

#[test]
fn the_first_formula_of_a_document_asks_for_the_page_to_be_loaded_anew() {
    let update = update(DOC, &DOC.replace("A second paragraph.", "A second paragraph, \\(x^2\\)."));
    assert_eq!(update.page, PageChange::Reload);
    assert_eq!(update.to_json()["page"], "reload");
}

#[test]
fn the_first_mark_of_an_error_and_the_mending_of_the_last_give_the_regions_they_lie_in() {
    // The preview's page requires the stylesheet of the marks of errors whether the
    // document has such a mark or not, so its head stays the same.
    let broken = DOC.replace("A second paragraph.", "A second \\nosuchmacro paragraph.");
    assert_eq!(named(&update(DOC, &broken)), vec![region_id("content")]);
    assert_eq!(named(&update(&broken, DOC)), vec![region_id("content")]);
}

#[test]
fn a_change_of_the_templates_own_markup_asks_for_the_page_to_be_loaded_anew() {
    let other = DOC.replace("title: Notes\n", "title: Notes\nflm:\n  features:\n    compose:\n      template: none\n");
    let update = update(DOC, &other);
    assert_eq!(update.page, PageChange::Reload);
}

#[test]
fn a_region_that_appears_or_goes_asks_for_the_page_to_be_loaded_anew() {
    let without = DOC.replace("\\footnote{A first note.}", "");
    assert_eq!(update(DOC, &without).page, PageChange::Reload);
    assert_eq!(update(&without, DOC).page, PageChange::Reload);
}

#[test]
fn the_first_build_asks_for_the_page_to_be_loaded() {
    let (now, _) = output(files(DOC));
    let (page, files) = compare(None, now.as_ref());
    assert_eq!(page, PageChange::Reload);
    assert!(files.is_empty(), "the page's own file is not among the files: {files:?}");
}

// --- acceptance 4: the files beside the page -----------------------------------------------

#[test]
fn a_file_beside_the_page_is_named_when_its_bytes_changed_appeared_or_went() {
    let (page, _) = output(files(DOC));
    let mut before = page.unwrap();
    let mut after = before.clone();
    before.files.insert(String::from("paper.same.png"), file("paper.same.png", false, b"same"));
    after.files.insert(String::from("paper.same.png"), file("paper.same.png", false, b"same"));
    before.files.insert(String::from("paper.plot.png"), file("paper.plot.png", false, b"old"));
    after.files.insert(String::from("paper.plot.png"), file("paper.plot.png", false, b"new"));
    before.files.insert(String::from("plot-0011aabb.png"), file("plot-0011aabb.png", false, b"one"));
    after.files.insert(String::from("plot-2233ccdd.png"), file("plot-2233ccdd.png", false, b"two"));
    let update = Update::new(2, Some(&before), Some(&after), Vec::new());
    assert_eq!(update.page, PageChange::Same);
    assert_eq!(update.files, vec!["paper.plot.png", "plot-0011aabb.png", "plot-2233ccdd.png"]);
    assert_eq!(update.to_json()["files"][0], "paper.plot.png");
}

#[test]
fn the_entry_of_an_output_that_is_no_page_is_one_of_the_files() {
    let output = |bytes: &[u8]| BuildOutput {
        page: None,
        leading: String::from("html"),
        files: [(String::from("paper.txt"), file("paper.txt", true, bytes))].into(),
        references: Default::default(),
    };
    let (before, after) = (output(b"A"), output(b"B"));
    assert_eq!(compare(Some(&before), Some(&after)), (PageChange::Same, vec![String::from("paper.txt")]));
    assert_eq!(compare(Some(&before), Some(&before)), (PageChange::Same, Vec::new()));
}

#[test]
fn a_leading_output_that_is_no_page_is_compared_as_a_file_and_gives_no_regions() {
    // The workflow `pair` leads with the text and also produces the page: the preview
    // serves `paper.txt`, so a change of the text names it, and the page's regions are not
    // the served file's.
    let pair = |text: &str| {
        MemoryFiles::new()
            .with("doc/paper.flm", &format!("---\nflm:\n  use: [pair]\n---\n{text}"))
            .with("defs/pair.yaml", "flm:\n  workflows:\n    pair:\n      extends: html\n      outputs: [text, html]\n")
    };
    let (before, _) = output_of(pair("A first paragraph.\n"), Some("pair"), "paper.txt");
    let (after, _) = output_of(pair("A second paragraph.\n"), Some("pair"), "paper.txt");
    let (before, after) = (before.expect("an output"), after.expect("an output"));
    assert_eq!(after.leading, "text");
    assert_eq!(after.entry().map(|file| file.path.as_str()), Some("paper.txt"));
    assert_eq!(after.page, None);
    let (page, files) = compare(Some(&before), Some(&after));
    assert_eq!(page, PageChange::Same);
    assert!(files.contains(&String::from("paper.txt")), "{files:?}");
}

#[test]
fn a_leading_output_in_latex_rendered_serves_its_document_and_its_style_file() {
    // The workflow `paper` leads with the LaTeX for LuaLaTeX and also produces the page: the
    // preview serves `paper.tex` at `/` (the leading output's entry) and the style file
    // beside it by its path, and the page's regions are not the served file's.
    let paper = |text: &str| {
        MemoryFiles::new()
            .with("doc/paper.flm", &format!("---\nflm:\n  use: [paper]\n---\n{text}"))
            .with("defs/paper.yaml", "flm:\n  workflows:\n    paper:\n      extends: html\n      outputs: [latex-rendered, html]\n")
    };
    let (output, problems) = output_of(paper("\\section{Intro}\n\nA paragraph.\n"), Some("paper"), "paper.tex");
    let output = output.expect("an output");
    assert!(problems.is_empty(), "{problems:?}");
    assert_eq!(output.leading, "latex-rendered");
    assert_eq!(output.page, None);
    let entry = output.entry().expect("the leading output's entry");
    assert_eq!(entry.path, "paper.tex");
    let tex = String::from_utf8(entry.bytes.clone()).unwrap();
    let style = output.files.keys().find(|path| path.ends_with(".sty")).expect("the style file");
    assert!(style.starts_with("paper-flmx."), "{style}");
    assert_eq!(output.files[style].output, "latex-rendered");
    assert!(!output.files[style].entry);
    assert!(tex.contains(&format!("\\usepackage{{{}}}", style.trim_end_matches(".sty"))), "{tex}");
}

// --- acceptance 5: a document with problems ------------------------------------------------

#[test]
fn a_document_with_an_error_gives_the_changed_regions_and_the_problems_with_their_places() {
    let broken = DOC.replace("A second paragraph.", "A second \\nosuchmacro paragraph.");
    let update = update(DOC, &broken);
    assert_eq!(named(&update), vec![region_id("content")]);
    let PageChange::Regions(regions) = &update.page else { unreachable!() };
    assert!(regions[0].text.contains("paragraph"), "the rest of the paragraph is shown");
    let error = update.problems.iter().find(|problem| problem.text.contains("nosuchmacro")).expect("an error");
    assert_eq!(error.severity, Severity::Error);
    assert!(error.text.starts_with("error: "), "{}", error.text);
    let position = error.position.as_ref().expect("a position");
    assert_eq!(position.file.as_deref(), Some("doc/paper.flm"));
    assert_eq!(position.line_column.map(|(line, _)| line), Some(6));
    let bytes = error.bytes.clone().expect("a range of bytes");
    let content = broken.split("---\n").nth(2).unwrap();
    assert_eq!(content[bytes.clone()].trim_end(), "\\nosuchmacro");
    let json = &update.to_json()["problems"].as_array().unwrap().iter().find(|p| p["text"].as_str().unwrap().contains("nosuchmacro")).unwrap().clone();
    assert_eq!(json["severity"], "error");
    assert_eq!(json["file"], "doc/paper.flm");
    assert_eq!(json["line"], 6);
    assert_eq!(json["start"], bytes.start);
    assert_eq!(json["end"], bytes.end);
}

#[test]
fn a_build_that_gives_no_page_says_so_and_carries_why() {
    let (before, _) = output(files(DOC));
    let (now, problems) = output(files("---\ntitle: [unclosed\n---\nHello.\n"));
    assert!(now.is_none());
    let update = Update::new(2, before.as_ref(), now.as_ref(), problems);
    assert_eq!(update.page, PageChange::Absent);
    assert!(update.files.is_empty());
    assert_eq!(update.problems.len(), 1);
    assert_eq!(update.problems[0].severity, Severity::Error);
    assert_eq!(update.problems[0].position.as_ref().and_then(|position| position.file.as_deref()), Some("doc/paper.flm"));
    assert_eq!(update.to_json()["page"], "absent");
    assert_eq!(update.to_json()["problems"][0]["start"], serde_json::Value::Null);
}
