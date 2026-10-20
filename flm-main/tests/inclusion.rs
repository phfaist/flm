//! `\input` through the host: files found relative to the file holding the command, named
//! by the path that gives, their front matter read, their problems reported where they
//! are, and every included file on the list of files read. No test touches the disk.

#[path = "support/files.rs"]
mod support;

use std::path::PathBuf;
use std::sync::Arc;

use flm_core::techy::error::Severity;
use flm_main::files::MemoryFiles;
use flm_main::output::{MemorySink, Sink};
use flm_main::report;
use flm_main::session::{build, prepare, Built, Input, Request};

use support::Written;

fn request(path: &str) -> Request {
    let mut request = Request::new(Input::File(PathBuf::from(path)));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.profile = Some(String::from("text"));
    request
}

fn run(files: MemoryFiles, path: &str) -> Built {
    let prepared = prepare(request(path), Arc::new(files), &[]).expect("prepared");
    build(&prepared).expect("built")
}

/// The document at `path` built to text, and the text written.
fn rendered(files: MemoryFiles, path: &str) -> (Built, String) {
    let sink = Arc::new(MemorySink::new());
    let mut request = request(path);
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    let prepared = prepare(request, Arc::new(files), &[]).expect("prepared");
    let built = build(&prepared).expect("built");
    let received = sink.files();
    let entry = received.values().find(|file| file.entry).expect("an output");
    (built, String::from_utf8(entry.bytes.clone()).expect("UTF-8"))
}

fn printed(built: &Built) -> Vec<String> {
    built.diagnostics.iter().map(report::diagnostic).collect()
}

fn errors(built: &Built) -> Vec<String> {
    built
        .diagnostics
        .iter()
        .filter(|d| d.severity() == Severity::Error)
        .map(|d| d.identifier().to_string())
        .collect()
}

// --- acceptance 7: paths ----------------------------------------------------------------

#[test]
fn a_file_in_a_subfolder_includes_its_neighbor_and_every_name_is_the_resolved_path() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", "Main \\input{parts/one.flm}\n")
        .document("doc/parts/one.flm", "one \\input{two.flm}\n")
        .document("doc/parts/two.flm", "two \\nosuchmacro\n");
    let built = run(files, "doc/paper.flm");
    assert_eq!(errors(&built), ["core.specs.unresolvable-command"]);
    let report = printed(&built).join("\n");
    assert!(report.contains("doc/parts/two.flm:1:5"), "{report}");
    assert!(report.contains("doc/parts/one.flm:1:5"), "{report}");
    assert!(report.contains("doc/paper.flm:1:6"), "{report}");
    let read: Vec<PathBuf> = built.files_read.iter().map(|read| read.path.clone()).collect();
    for name in ["doc/paper.flm", "doc/parts/one.flm", "doc/parts/two.flm"] {
        assert!(read.contains(&PathBuf::from(name)), "{read:?}");
    }
}

#[test]
fn a_reference_upward_is_folded_into_one_name() {
    let files = MemoryFiles::new()
        .document("doc/parts/one.flm", "\\input{../common.flm}\n")
        .document("doc/common.flm", "common \\nosuchmacro\n")
        .document("doc/paper.flm", "\\input{parts/one.flm}\n");
    let built = run(files, "doc/paper.flm");
    let report = printed(&built).join("\n");
    assert!(report.contains("doc/common.flm:1:8"), "{report}");
}

// --- acceptance 8: where a problem is reported --------------------------------------------

#[test]
fn a_problem_on_a_line_of_an_included_file_with_a_front_matter_is_reported_at_that_line() {
    let files = MemoryFiles::new()
        .document("paper.flm", "Hello \\input{part.flm}\n")
        .document("part.flm", "---\ntitle: A part\n---\n\nThe \\nosuchmacro.\n");
    let built = run(files, "paper.flm");
    let report = printed(&built).join("\n");
    assert!(report.contains("part.flm:5:5"), "{report}");
    assert!(report.contains("paper.flm:1:7"), "{report}");
}

#[test]
fn a_front_matter_that_sets_what_only_the_primary_file_may_set_is_reported_there() {
    let files = MemoryFiles::new()
        .document("paper.flm", "Hello \\input{part.flm}\n")
        .faulty("part.flm", "---\nflm:\n  use: [other]\n---\nwords\n");
    let built = run(files, "paper.flm");
    assert_eq!(errors(&built), ["flm.input.front-matter"]);
    let report = printed(&built).join("\n");
    assert!(report.contains("part.flm:3:8"), "{report}");
}

#[test]
fn a_front_matter_that_is_not_yaml_is_reported_and_the_content_brought_in() {
    let files = MemoryFiles::new()
        .document("paper.flm", "Hello \\input{part.flm}\n")
        .faulty("part.flm", "---\n: : [\n---\nwords\n");
    let built = run(files, "paper.flm");
    assert_eq!(errors(&built), ["flm.input.front-matter"]);
}

// --- acceptance 3 through the host: parsing settings ---------------------------------------

#[test]
fn the_parsing_settings_of_an_included_file_are_read_over_those_of_the_including_file() {
    let files = MemoryFiles::new()
        .document("paper.flm", "---\nflm:\n  parsing: {comment_prefix: ';;'}\n---\n\\input{part.flm} after ;; \\nosuchmacro gone\n")
        .document("part.flm", "---\nflm:\n  parsing: {dollar_math: true}\n---\n$x$ ;; \\nosuchmacro hidden\n");
    let built = run(files, "paper.flm");
    assert_eq!(errors(&built), Vec::<String>::new(), "{:?}", printed(&built));
}

#[test]
fn a_file_without_settings_of_its_own_hands_on_those_it_inherited() {
    // `a.flm` writes `;;` as its comment start, `b.flm` writes nothing, `c.flm` adds `$`
    // math: in `c.flm` both hold, and in `b.flm` the first does.
    let files = MemoryFiles::new()
        .document("paper.flm", "\\input{a.flm}\n")
        .document("a.flm", "---\nflm:\n  parsing: {comment_prefix: ';;'}\n---\n\\input{b.flm}\n")
        .document("b.flm", "\\input{c.flm} ;; \\nosuchmacro in b\n")
        .document("c.flm", "---\nflm:\n  parsing: {dollar_math: true}\n---\n$x$ ;; \\nosuchmacro in c\n");
    let built = run(files, "paper.flm");
    assert_eq!(errors(&built), Vec::<String>::new(), "{:?}", printed(&built));
}

// --- acceptance 9: a file that names no file ----------------------------------------------

#[test]
fn a_reference_that_names_no_file_is_reported_with_the_path_looked_for() {
    let files = MemoryFiles::new().document("doc/paper.flm", "\\input{missing.flm}\n");
    let built = run(files, "doc/paper.flm");
    assert_eq!(errors(&built), ["flm.input.unreadable"]);
    assert!(printed(&built)[0].contains("doc/missing.flm"), "{:?}", printed(&built));
    assert!(built.files_read.iter().any(|read| !read.found && read.path == std::path::Path::new("doc/missing.flm")));
}

// --- acceptance 12: nothing loops -----------------------------------------------------------

#[test]
fn a_file_that_includes_itself_is_reported_and_an_empty_file_or_a_front_matter_alone_brings_nothing() {
    let files = MemoryFiles::new()
        .document("paper.flm", "A \\input{self.flm} \\input{empty.flm} \\input{only.flm} B\n")
        .document("self.flm", "\\input{self.flm}\n")
        .document("empty.flm", "")
        .document("only.flm", "---\ntitle: nothing\n---\n");
    let built = run(files, "paper.flm");
    assert_eq!(errors(&built), ["flm.input.includes-itself"]);
}

// --- acceptance 4 and 8 through the host: a heading input ----------------------------------

#[test]
fn a_file_declaring_a_heading_opens_with_its_title_under_the_command_named() {
    let files = MemoryFiles::new()
        .document("doc/paper.flm", "Intro, see \\ref{sec:part}.\n\n\\input{part.flm}\n")
        .document(
            "doc/part.flm",
            "---\ntitle: The \\emph{Part}\nflm:\n  input_as: {heading: section, label: 'sec:part'}\n---\nThe part's text.\n",
        );
    let (built, text) = rendered(files, "doc/paper.flm");
    assert_eq!(errors(&built), Vec::<String>::new(), "{:?}", printed(&built));
    let heading = text.find("The Part\n========").unwrap_or_else(|| panic!("{text}"));
    assert!(text.find("Intro, see The Part.").is_some_and(|at| at < heading), "{text}");
    assert!(text[heading..].contains("text."), "{text}");
}

#[test]
fn a_problem_in_the_title_of_a_heading_input_is_reported_at_its_line_in_the_front_matter() {
    let files = MemoryFiles::new()
        .document("paper.flm", "\\input{part.flm}\n")
        .faulty("part.flm", "---\nflm:\n  input_as: {heading: section}\ntitle: A \\nosuchmacro title\n---\nText.\n");
    let built = run(files, "paper.flm");
    assert_eq!(errors(&built), ["core.specs.unresolvable-command"]);
    let report = printed(&built).join("\n");
    // `title: ` puts the title at column 8, so the command stands at column 10.
    assert!(report.contains("part.flm:4:10"), "{report}");
}

#[test]
fn a_bad_input_as_is_reported_in_the_front_matter() {
    let files = MemoryFiles::new()
        .document("paper.flm", "\\input{part.flm}\n")
        .faulty("part.flm", "---\nflm:\n  input_as: {heading: section, level: 2}\n---\nText.\n");
    let built = run(files, "paper.flm");
    assert_eq!(errors(&built), ["flm.input.front-matter"]);
    assert!(printed(&built).join("\n").contains("part.flm:3:"), "{:?}", printed(&built));
}
