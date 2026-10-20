//! The host from a file to named files, in memory: a document read from `MemoryFiles`, its
//! front matter and settings combined, its profile and its workflow chosen, its output
//! handed to a `MemorySink`, and the error gate. No test touches the disk.

use std::path::PathBuf;
use std::sync::Arc;

use flm_main::files::MemoryFiles;
use flm_main::output::{MemorySink, Naming, Sink};
use flm_core::core::registry::{
    BuildError, FeatureBuilder, FeatureInfo, FlmFeature, OutputDef, PackageEntry, PackageFactory, WorkflowDef,
};
use flm_core::techy::error::DiagnosticInfo;
use flm_main::seed::{MathChoice, Place};
use flm_main::session::{
    Built, GivenList, GivenMath, Input, PrepareErrorKind, Request, build, document_packages, prepare,
};
use flm_main::packages::ExtensionFolder;
use flm_main::settings::{CommandLineSetting, Source};
use flm_core::model::doc::FragmentLayout;
use flm_core::stdcore::features::compose;
use flm_core::stdcore::features::headings::{HeadingNumbering, HeadingsConfig};
use flm_core::stdcore::features::meta::DocMeta;

/// The command-line setting `text`, labeled as `-O` writes it.
fn option(text: &str) -> CommandLineSetting {
    CommandLineSetting::new(text, format!("-O {text}"))
}

/// A request for the file `paper.flm` of `files`, giving the profile `profile` when given.
fn request(profile: Option<&str>) -> Request {
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = MathChoice::processor("passthrough");
    request.profile = profile.map(str::to_string);
    request
}

/// A request giving no profile and the output `format`.
fn format_alone(format: &str) -> Request {
    let mut request = request(None);
    request.format = Some(String::from(format));
    request
}

fn run(files: MemoryFiles, request: Request) -> Built {
    let prepared = prepare(request, Arc::new(files), &[]).expect("prepared");
    build(&prepared).expect("built")
}

/// Builds `request` over `files` with a `MemorySink`, and gives the run and the text of
/// the output's entry the sink received.
fn rendered(files: MemoryFiles, mut request: Request) -> (Built, String) {
    let sink = Arc::new(MemorySink::new());
    request.sink = Some(Arc::clone(&sink) as Arc<dyn Sink>);
    let built = run(files, request);
    let received = sink.files();
    let entry = received.values().find(|file| file.entry).expect("an output");
    (built, String::from_utf8(entry.bytes.clone()).unwrap())
}

fn text(files: MemoryFiles, request: Request) -> String {
    rendered(files, request).1
}

fn one_file(text: &str) -> MemoryFiles {
    MemoryFiles::new().with("doc/paper.flm", text)
}

// --- the initial list ------------------------------------------------------------------------

/// The names of the packages that `request` uses for the document `text`, as *prepare*
/// resolves them.
fn packages_of(text: &str, request: Request) -> Vec<String> {
    prepare(request, Arc::new(one_file(text)), &[]).expect("prepared").packages().to_vec()
}

/// The problems that stop `request` for the document `text` at finding its packages.
fn package_problems(text: &str, request: Request) -> Vec<flm_main::session::Problem> {
    let error = prepare(request, Arc::new(one_file(text)), &[]).err().expect("a problem");
    assert_eq!(error.kind, PrepareErrorKind::Packages, "{error:?}");
    error.problems
}

/// The request for `doc/paper.flm` whose initial list given on the command line is `names`.
fn given(names: &[&str]) -> Request {
    let mut request = request(Some("text"));
    request.initial_use = Some(GivenList::new(names.iter().copied(), "the option `--initial-use`"));
    request
}

#[test]
fn the_initial_list_is_std_by_default_and_comes_before_the_document_s_use() {
    // flm-main's own default is `std` and no math processor; these tests' requests choose
    // `passthrough`, whose package follows the initial list.
    let library = Request::new(Input::Text(String::new()));
    assert_eq!(library.default_initial_use.packages, ["std"]);
    assert_eq!(library.default_math, MathChoice::NoProcessor);
    assert_eq!(request(None).default_initial_use.packages, ["std"]);
    let all = ["stdcore", "stdmain", "std", "math-passthrough"];
    assert_eq!(packages_of("Hello.\n", request(Some("text"))), all);
    // The document's `use` comes after; a package met already is not used twice.
    assert_eq!(packages_of("---\nflm: {use: [stdcore]}\n---\nHello.\n", request(Some("text"))), all);
}

#[test]
fn the_front_matter_s_initial_use_replaces_the_host_s_list_exactly() {
    let text = "---\nflm:\n  initial_use: [stdcore]\n  use: [stdmain]\n---\nHello.\n";
    assert_eq!(packages_of(text, request(Some("text"))), ["stdcore", "math-passthrough", "stdmain"]);
    // An empty list uses no package before the math processor's and those of `use`.
    let text = "---\nflm:\n  initial_use: []\n  use: [stdcore]\n---\nHello.\n";
    assert_eq!(packages_of(text, request(Some("text"))), ["stdcore", "math-passthrough"]);
}

#[test]
fn the_command_line_s_initial_list_wins_then_the_front_matter_s_then_the_host_s() {
    let written = "---\nflm:\n  initial_use: [stdcore]\n---\nHello.\n";
    // The command line's list wins over the front matter's and the host's.
    let mut request = given(&["stdcore", "stdmain"]);
    request.default_initial_use = GivenList::new(["nosuch"], "the host's list");
    assert_eq!(packages_of(written, request.clone()), ["stdcore", "stdmain", "math-passthrough"]);
    // Without it, the front matter's wins over the host's.
    request.initial_use = None;
    assert_eq!(packages_of(written, request.clone()), ["stdcore", "math-passthrough"]);
    // Without either, the host's applies: its unknown entry is in no file.
    // Its message opens with where the list comes from.
    let problems = package_problems("Hello.\n", request);
    assert_eq!((problems[0].file.as_deref(), problems[0].place), (None, None), "{problems:?}");
    assert!(
        problems[0].detail.starts_with("in the host's list, `nosuch` must name a package compiled into the program"),
        "{problems:?}"
    );
    let problems = package_problems("Hello.\n", given(&["nosuch"]));
    assert_eq!((problems[0].file.as_deref(), problems[0].place), (None, None), "{problems:?}");
    assert!(problems[0].detail.starts_with("in the option `--initial-use`, `nosuch` must name"), "{problems:?}");
    // The program's own default list says so.
    let mut program = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    program.default_initial_use.packages = vec![String::from("nosuch")];
    let problems = package_problems("Hello.\n", program);
    assert!(
        problems[0].detail.starts_with("in the program's default list of packages, `nosuch` must name"),
        "{problems:?}"
    );
}

#[test]
fn an_unknown_package_of_initial_use_is_reported_at_its_entry() {
    let problems = package_problems("---\nflm:\n  initial_use: [std, nosuch]\n---\nHello.\n", request(Some("text")));
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].file.as_deref(), Some("doc/paper.flm"));
    assert_eq!(problems[0].place, Some(flm_main::seed::Place { line: 3, column: 22 }));
    assert_eq!(
        problems[0].detail,
        "`nosuch` must name a package compiled into the program (`math-passthrough`, `std`, `stdcore`, `stdmain`) \
         or an extension \
         found in a folder of the search path, which is empty"
    );
}

#[test]
fn std_adds_nothing_to_stdcore_and_stdmain() {
    let text = |front: &str| format!("---\ntitle: A title\n{front}---\nHello \\emph{{world}}.\\footnote{{A note.}}\n");
    let by_default = rendered(one_file(&text("")), request(Some("html"))).1;
    let without_std =
        rendered(one_file(&text("flm: {initial_use: [stdcore, stdmain]}\n")), request(Some("html"))).1;
    assert_eq!(without_std, by_default);
}

// --- the math processor ----------------------------------------------------------------------

/// The request for `doc/paper.flm` whose math processor chosen on the command line is
/// `choice`, and whose host's processor is the unknown `nohost`.
fn given_math(choice: MathChoice) -> Request {
    let mut request = request(Some("text"));
    request.default_math = MathChoice::processor("nohost");
    request.math = Some(GivenMath::new(choice, "the option `--math`"));
    request
}

/// The names of the packages that `request` finds for the document `text`, without
/// building a registry.
fn found(text: &str, request: Request) -> Vec<String> {
    let found = document_packages(request, Arc::new(one_file(text)), &[]).expect("found");
    found.factories.iter().map(|factory| factory.name().to_string()).collect()
}

#[test]
fn the_math_processor_s_package_follows_the_initial_list_of_each_source() {
    let all = ["stdcore", "stdmain", "std", "math-passthrough"];
    // After the host's list, the front matter's and the command line's.
    assert_eq!(packages_of("Hello.\n", request(Some("text"))), all);
    let written = "---\nflm:\n  initial_use: [stdcore, stdmain]\n  use: [std]\n---\nHello.\n";
    assert_eq!(packages_of(written, request(Some("text"))), ["stdcore", "stdmain", "math-passthrough", "std"]);
    assert_eq!(packages_of("Hello.\n", given(&["stdmain"])), ["stdcore", "stdmain", "math-passthrough"]);
    // A written `initial_use` and a written `math` both apply.
    let mut library = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    library.profile = Some(String::from("text"));
    let both = "---\nflm:\n  initial_use: [stdcore, stdmain]\n  math: passthrough\n---\nHello.\n";
    assert_eq!(packages_of(both, library), ["stdcore", "stdmain", "math-passthrough"]);
}

#[test]
fn the_command_line_s_math_processor_wins_then_the_front_matter_s_then_the_host_s() {
    let all = ["stdcore", "stdmain", "std", "math-passthrough"];
    // The command line's choice wins over the front matter's unknown `nowritten` and the
    // host's unknown `nohost`.
    let written = "---\nflm:\n  math: nowritten\n---\nHello.\n";
    assert_eq!(packages_of(written, given_math(MathChoice::processor("passthrough"))), all);
    // Without it, the front matter's wins over the host's.
    let mut request = given_math(MathChoice::NoProcessor);
    request.math = None;
    assert_eq!(packages_of("---\nflm:\n  math: passthrough\n---\nHello.\n", request.clone()), all);
    // Without either, the host's applies.
    let problems = package_problems("Hello.\n", request);
    assert!(problems[0].detail.starts_with("in the program's default choice of math processor, `math-nohost` must name"), "{problems:?}");
}

#[test]
fn the_choice_of_no_processor_adds_no_package() {
    // The command line's `false` wins over the host's processor: no package is added.
    let none = given_math(MathChoice::NoProcessor);
    assert_eq!(found("Hello.\n", none.clone()), ["stdcore", "stdmain", "std"]);
    // A document without formulas then runs without error.
    let built = run(one_file("Hello.\n"), none);
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    // The front matter's `false`, with a processor listed under `use`.
    let written = "---\nflm:\n  math: false\n  use: [math-passthrough]\n---\nHello.\n";
    assert_eq!(packages_of(written, request(Some("text"))), ["stdcore", "stdmain", "std", "math-passthrough"]);
    assert_eq!(found("---\nflm:\n  math: false\n---\nHello.\n", request(Some("text"))), ["stdcore", "stdmain", "std"]);
}

#[test]
fn an_unknown_math_processor_is_reported_where_it_is_chosen() {
    // At the value of the front matter's `math`.
    let problems = package_problems("---\nflm:\n  math: nosuch\n---\nHello.\n", request(Some("text")));
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].file.as_deref(), Some("doc/paper.flm"));
    assert_eq!(problems[0].place, Some(Place { line: 3, column: 9 }));
    assert_eq!(
        problems[0].detail,
        "`math-nosuch` must name a package compiled into the program (`math-passthrough`, `std`, `stdcore`, \
         `stdmain`) or an extension found in a folder of the search path, which is empty"
    );
    // A choice given on the command line, and the host's, are in no file: the message opens
    // with where the choice comes from.
    let problems = package_problems("Hello.\n", given_math(MathChoice::processor("nosuch")));
    assert_eq!((problems[0].file.as_deref(), problems[0].place), (None, None), "{problems:?}");
    assert!(problems[0].detail.starts_with("in the option `--math`, `math-nosuch` must name a package"), "{problems:?}");
    let mut host = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    host.default_math = MathChoice::processor("nosuch");
    let problems = package_problems("Hello.\n", host);
    assert_eq!((problems[0].file.as_deref(), problems[0].place), (None, None), "{problems:?}");
    assert!(
        problems[0].detail.starts_with("in the program's default choice of math processor, `math-nosuch` must name"),
        "{problems:?}"
    );
}

#[test]
fn a_value_of_math_that_is_not_a_processor_s_name_stops_at_the_front_matter() {
    for (value, column) in [("a/b", 9), ("true", 9), ("[mathjax]", 9)] {
        let text = format!("---\nflm:\n  math: {value}\n---\nHello.\n");
        let error = prepare(request(Some("text")), Arc::new(one_file(&text)), &[]).err().expect("a problem");
        assert_eq!(error.kind, PrepareErrorKind::FrontMatter, "{error:?}");
        assert_eq!(error.problems[0].place, Some(Place { line: 3, column }), "{error:?}");
    }
}

// --- test 8: the seed ---------------------------------------------------------------------

#[test]
fn a_problem_in_the_content_is_placed_at_its_line_in_the_file() {
    let built = run(one_file("---\ntitle: x\n---\n\nHello \\nosuchmacro.\n"), request(Some("text")));
    let error = built.diagnostics.iter().find(|d| d.severity() == flm_core::techy::error::Severity::Error).unwrap();
    let printed = flm_main::report::diagnostic(error);
    assert!(printed.contains("doc/paper.flm:5:"), "{printed}");
}

#[test]
fn the_files_read_hold_the_primary_file() {
    let built = run(one_file("Hello."), request(Some("text")));
    assert!(built.files_read.iter().all(|read| read.found && read.path == std::path::Path::new("doc/paper.flm")));
    assert!(!built.files_read.is_empty());
}

#[test]
fn a_missing_primary_file_is_a_problem_of_the_input() {
    let error = prepare(request(None), Arc::new(MemoryFiles::new()), &[]).err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Input);
}

#[test]
fn the_layout_and_the_parsing_of_the_front_matter_are_used() {
    let built = run(
        one_file("---\nflm:\n  parsing: {dollar_math: true}\n  layout: inline\n---\n$x$ and y\n"),
        request(Some("text")),
    );
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
}

// --- test 10: the metadata, through the host ----------------------------------------------

const META: &str = "---\ntitle: On \\emph{flexible} markup\nauthor: [Ada Lovelace, Grace Hopper]\ndate: 2026\nkeywords: [markup]\n---\nThe content.\n";

#[test]
fn the_metadata_of_the_front_matter_is_shown_before_the_content() {
    let (built, output) = rendered(one_file(META), request(Some("text")));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    let at = |needle: &str| output.find(needle).unwrap_or_else(|| panic!("`{needle}` in:\n{output}"));
    assert!(at("On flexible markup") < at("Ada Lovelace, Grace Hopper"), "{output}");
    assert!(at("Grace Hopper") < at("2026"), "{output}");
    assert!(at("2026") < at("The content."), "{output}");
    assert!(!output.contains("keywords"), "{output}");
}

#[test]
fn a_problem_in_the_title_is_printed_at_its_line_in_the_file() {
    let built = run(one_file("---\ntitle: A \\nosuchmacro title\n---\nThe content.\n"), request(Some("text")));
    let error = built.diagnostics.iter().find(|d| d.severity() == flm_core::techy::error::Severity::Error).unwrap();
    let printed = flm_main::report::diagnostic(error);
    assert!(printed.contains("doc/paper.flm:2:10"), "{printed}");
}

#[test]
fn a_problem_in_a_quoted_title_is_printed_at_the_column_an_editor_shows() {
    for title in ["\"A \\\\nosuchmacro title\"", "'A \\nosuchmacro title'"] {
        let built = run(one_file(&format!("---\ntitle: {title}\n---\nThe content.\n")), request(Some("text")));
        let error = built.diagnostics.iter().find(|d| d.severity() == flm_core::techy::error::Severity::Error).unwrap();
        let printed = flm_main::report::diagnostic(error);
        assert!(printed.contains("doc/paper.flm:2:11"), "{title}: {printed}");
    }
}

#[test]
fn a_title_that_is_not_a_text_is_a_problem_of_the_front_matter() {
    let error = prepare(request(Some("text")), Arc::new(one_file("---\ntitle: [a, b]\n---\nText.\n")), &[])
        .err()
        .unwrap();
    assert_eq!(error.kind, PrepareErrorKind::FrontMatter);
    assert_eq!(error.problems[0].place.map(|place| place.line), Some(2));
}

#[test]
fn with_the_feature_switched_off_the_metadata_is_not_shown() {
    let (built, output) = rendered(
        one_file("---\ntitle: A title\nflm:\n  features:\n    meta: false\n---\nThe content.\n"),
        request(Some("text")),
    );
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    assert!(!output.contains("A title"));
}

// --- test 9: the settings -----------------------------------------------------------------

const NOTE: &str = "Text\\footnote{A note.} more.\n";

#[test]
fn the_program_s_defaults_then_the_front_matter_then_the_command_line() {
    // The program's defaults mark a footnote with a letter.
    let defaults = text(one_file(NOTE), request(Some("text")));
    assert!(defaults.contains("[a]") || defaults.contains("a]") || defaults.contains("ᵃ"), "{defaults}");
    // The front matter replaces the kinds whole.
    let front = "---\nflm:\n  features:\n    xnotes:\n      kinds:\n        footnote:\n          words: {base: {one: \"Note~\", other: \"Notes~\"}}\n          commands: [footnote]\n          title: Remarks\n---\n";
    let written = text(one_file(&format!("{front}{NOTE}")), request(Some("text")));
    assert!(written.contains("Remarks"), "{written}");
    // The command line outranks the front matter.
    let mut command = request(Some("text"));
    command.options.push(option(
        "xnotes.kinds={footnote: {words: {base: {one: N~, other: Ns~}}, commands: [footnote], title: Asides}}",
    ));
    let over = text(one_file(&format!("{front}{NOTE}")), command);
    assert!(over.contains("Asides") && !over.contains("Remarks"), "{over}");
}

#[test]
fn a_key_of_each_binding_is_set_from_the_command_line() {
    let mut command = request(Some("text"));
    command.options.push(option("text.width=20"));
    command.options.push(option("numbering.counters={flm.headings.section: {initial: 4}}"));
    command.options.push(option(
        "xnotes.kinds={footnote: {words: {base: {one: N~, other: Ns~}}, commands: [footnote], title: Asides}}",
    ));
    let document = "\\section{A long heading}\n\nOne two three four five six seven eight.\\footnote{A.}\n";
    let (built, output) = rendered(one_file(document), command);
    assert!(output.lines().all(|line| line.chars().count() <= 20), "{output}");
    let numbering = built.run.doc().options().get::<flm_core::stdcore::features::numbering::NumberingOptions>();
    assert_eq!(numbering.unwrap().counters.values().next().unwrap().initial, Some(4));
    assert!(output.contains("Asides"), "{output}");
}

#[test]
fn a_key_bound_per_document_is_seen_on_the_output() {
    let mut command = request(Some("text"));
    command.options.push(option("numbering.counters={flm.theorems.theorem: {initial: 4}}"));
    let output = text(one_file("\\begin{theorem}\nTrue.\n\\end{theorem}\n"), command);
    // The theorem's word and number are joined by a non-breaking space.
    assert!(output.replace('\u{a0}', " ").contains("Theorem 5"), "{output}");
}

#[test]
fn a_key_no_feature_declares_is_a_problem_of_the_settings() {
    let error = prepare(request(None), Arc::new(one_file("---\nflm:\n  features:\n    text: {wdth: 3}\n---\nx")), &[])
        .err()
        .unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Settings);
    assert_eq!(error.problems[0].place.map(|place| place.line), Some(4));
}

// --- test 11: the workflow ----------------------------------------------------------------

fn workflow_of(front: &str, request: Request) -> String {
    let prepared = prepare(request, Arc::new(one_file(&format!("{front}Hello."))), &[]).unwrap();
    prepared.workflow().to_string()
}

#[test]
fn html_when_no_workflow_is_specified() {
    assert_eq!(workflow_of("", request(None)), "html");
}

/// The text of the one problem of the error that preparing `text` with `request` stops
/// with, a problem of the workflow.
fn workflow_problem(text: &str, request: Request) -> String {
    let error = prepare(request, Arc::new(one_file(text)), &[]).err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Workflow);
    assert_eq!(error.problems.len(), 1);
    error.problems[0].detail.clone()
}

#[test]
fn the_format_selects_an_output_of_the_specified_workflow_and_never_chooses_one() {
    let mut specified = request(Some("text"));
    specified.format = Some(String::from("text"));
    assert_eq!(workflow_of("", specified), "text");
    // The output alone runs the workflow `html`, which must provide it.
    assert_eq!(workflow_of("", format_alone("html")), "html");
    // One message, whoever chose the workflow: here no workflow is specified.
    assert_eq!(
        workflow_problem("x", format_alone("text")),
        "the workflow `html` does not provide the format `text`; it provides `html`"
    );
    assert_eq!(
        workflow_problem("x", format_alone("nosuch")),
        "the workflow `html` does not provide the format `nosuch`; it provides `html`"
    );
    // The request specifies the workflow.
    let mut specified = request(Some("text"));
    specified.format = Some(String::from("html"));
    assert_eq!(
        workflow_problem("x", specified),
        "the workflow `text` does not provide the format `html`; it provides `text`"
    );
    let mut specified = request(Some("check"));
    specified.format = Some(String::from("text"));
    assert_eq!(
        workflow_problem("x", specified),
        "the workflow `check` does not provide the format `text`; it provides none"
    );
}

#[test]
fn the_default_profile_runs_whatever_the_format() {
    let front = "---\nflm: {default_profile: text}\n---\n";
    assert_eq!(workflow_of(front, request(None)), "text");
    assert_eq!(workflow_of(front, format_alone("text")), "text");
    assert_eq!(workflow_of(front, request(Some("latex-recomposed"))), "latex-recomposed");
    // A default that does not provide the output is not passed over; the message is the
    // one for a workflow the request specifies.
    assert_eq!(
        workflow_problem(&format!("{front}x"), format_alone("html")),
        "the workflow `text` does not provide the format `html`; it provides `text`"
    );
    assert_eq!(
        workflow_problem("---\nflm: {default_profile: check}\n---\nx", format_alone("html")),
        "the workflow `check` does not provide the format `html`; it provides none"
    );
}

#[test]
fn a_default_profile_written_as_a_map_is_a_problem_of_the_front_matter() {
    let text = "---\nflm:\n  default_profile: {_: latex-recomposed, html: html}\n---\nx";
    let error = prepare(request(None), Arc::new(one_file(text)), &[]).err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::FrontMatter);
    assert_eq!(error.problems[0].place.map(|place| place.line), Some(3));
}

#[test]
fn without_a_workflow_html_and_no_profile_given_there_is_no_workflow_to_run() {
    workflow_problem("---\nflm: {features: {html: false}}\n---\nx", request(None));
}

#[test]
fn a_name_that_matches_nothing_is_a_problem_of_the_workflow() {
    workflow_problem("x", request(Some("nosuch")));
    let error = prepare(request(None), Arc::new(one_file("---\nflm: {default_profile: nope}\n---\nx")), &[])
        .err()
        .unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Workflow);
    assert_eq!(error.problems[0].place.map(|place| place.line), Some(2));
}

// --- the feature `write`, switched off ------------------------------------------------

const WRITE_OFF: &str = "---\nflm:\n  features: {write: false}\n---\nHello.\n";

#[test]
fn a_request_that_writes_nothing_reads_write_switched_off() {
    let built = run(one_file(WRITE_OFF), request(Some("text")));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
}

#[test]
fn a_request_that_writes_cannot_start_with_write_switched_off() {
    let sink = Arc::new(MemorySink::new());
    let error = prepare(writing("text", &sink), Arc::new(one_file(WRITE_OFF)), &[]).err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Plan);
    assert_eq!(error.problems[0].place.map(|place| place.line), Some(3));
}

#[test]
fn settings_written_for_write_are_read_the_same_whether_or_not_the_request_writes() {
    let front = one_file("---\nflm:\n  features: {write: {x: 1}}\n---\nHello.\n");
    let silent = prepare(request(Some("text")), Arc::new(front.clone()), &[]).err().unwrap();
    let sink = Arc::new(MemorySink::new());
    let writing = prepare(writing("text", &sink), Arc::new(front), &[]).err().unwrap();
    // A diagnostic has no equality; the two errors carry none.
    assert!(silent.diagnostics.is_empty() && writing.diagnostics.is_empty());
    assert_eq!((silent.kind, &silent.problems, &silent.files_read), (writing.kind, &writing.problems, &writing.files_read));
    assert_eq!(silent.kind, PrepareErrorKind::Settings);
}

// --- the profile ------------------------------------------------------------------------

/// A request for `doc/paper.flm` giving the profile `profile` when given, its search path the
/// folder `defs`.
fn profiled(profile: Option<&str>) -> Request {
    let mut request = request(profile);
    request.extension_folders = vec![ExtensionFolder { path: PathBuf::from("defs"), required: false }];
    request
}

/// The extension `profile-draft`, declaring the workflow `draft`.
const PROFILE_DRAFT: &str = "flm:\n  workflows:\n    draft:\n      extends: text\n";

#[test]
fn a_profile_s_extension_is_used_and_its_workflow_runs() {
    let files = one_file("Hello.").with("defs/profile-draft.yaml", PROFILE_DRAFT);
    let prepared = prepare(profiled(Some("draft")), Arc::new(files.clone()), &[]).expect("prepared");
    assert_eq!(prepared.profile_package(), Some("profile-draft"));
    assert!(prepared.packages().last().is_some_and(|name| name.contains("profile-draft")), "{:?}", prepared.packages());
    assert_eq!(prepared.workflow(), "draft");
    assert_eq!(text(files, profiled(Some("draft"))), "Hello.\n");
}

#[test]
fn without_its_extension_a_profile_runs_the_workflow_a_package_in_use_declares() {
    let files = one_file("---\nflm:\n  use: [both]\n---\nHello.").with("defs/both.yaml", BOTH);
    let prepared = prepare(profiled(Some("both")), Arc::new(files), &[]).expect("prepared");
    assert_eq!((prepared.workflow(), prepared.profile_package()), ("both", None));
}

#[test]
fn a_profile_with_neither_its_extension_nor_its_workflow_is_an_error() {
    let error = prepare(profiled(Some("nosuch")), Arc::new(one_file("Hello.")), &[]).err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Workflow);
}

#[test]
fn a_profile_s_extension_that_declares_no_workflow_of_its_name_is_an_error() {
    let files = one_file("Hello.").with("defs/profile-draft.yaml", "extension: {description: No workflow.}\n");
    let error = prepare(profiled(Some("draft")), Arc::new(files), &[]).err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Workflow);
}

#[test]
fn a_profile_s_extension_that_cannot_be_read_is_an_error() {
    let files = one_file("Hello.").with("defs/profile-draft.yaml", "flm: [not valid\n");
    let error = prepare(profiled(Some("draft")), Arc::new(files), &[]).err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Packages);
}

#[test]
fn the_front_matter_s_default_profile_loads_its_extension_and_another_profile_does_not() {
    let files = one_file("---\nflm: {default_profile: draft}\n---\nHello.").with("defs/profile-draft.yaml", PROFILE_DRAFT);
    let prepared = prepare(profiled(None), Arc::new(files.clone()), &[]).expect("prepared");
    assert_eq!((prepared.workflow(), prepared.profile_package()), ("draft", Some("profile-draft")));
    let prepared = prepare(profiled(Some("text")), Arc::new(files), &[]).expect("prepared");
    assert_eq!((prepared.workflow(), prepared.profile_package()), ("text", None));
}

#[test]
fn the_files_read_include_where_the_profile_s_extension_would_be() {
    let prepared = prepare(profiled(Some("text")), Arc::new(one_file("Hello.")), &[]).expect("prepared");
    let probed = prepared.files_read().into_iter().any(|read| read.path == std::path::Path::new("defs/profile-text.yaml") && !read.found);
    assert!(probed, "{:?}", prepared.files_read());
}

// --- the packages the request adds ----------------------------------------------------------

#[test]
fn the_request_s_packages_come_after_the_front_matter_s_use_and_before_the_profile_s() {
    let extension = "extension: {description: An extension.}\n";
    let files = one_file("---\nflm: {use: [one]}\n---\nHello.")
        .with("defs/one.yaml", extension)
        .with("defs/two.yaml", extension)
        .with("defs/three.yaml", extension)
        .with("defs/profile-draft.yaml", PROFILE_DRAFT);
    let mut request = profiled(Some("draft"));
    request.uses = Some(GivenList::new(["three", "two"], "the option `--use`"));
    let prepared = prepare(request, Arc::new(files.clone()), &[]).expect("prepared");
    let packages = prepared.packages();
    let last = &packages[packages.len().saturating_sub(4)..];
    assert_eq!(last, ["defs/one.yaml#_", "defs/three.yaml#_", "defs/two.yaml#_", "defs/profile-draft.yaml#_"], "{packages:?}");
    // An entry that names no package stops *prepare*.
    let mut request = profiled(Some("draft"));
    request.uses = Some(GivenList::new(["nosuch"], "the option `--use`"));
    let error = prepare(request, Arc::new(files), &[]).err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Packages);
}

/// A compiled package `plain`, whose feature adds the output `plain` (the text backend's
/// artifact set under another name, which no workflow is named after) and the workflows
/// `names`, each delivering it.
fn plain_package(names: &'static [&'static str]) -> PackageFactory {
    struct Plain(&'static [&'static str]);
    impl FlmFeature for Plain {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new("plain", "The output `plain`")
        }
        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            let table = flm_core::stdcore::backends::text::TEXT_DOCUMENT.name();
            b.output(OutputDef::new("plain", table, table, "text/plain", "txt"))?;
            for name in self.0 {
                b.workflow(WorkflowDef::new(*name).output("plain"))?;
            }
            Ok(())
        }
    }
    PackageFactory::new("plain", move |_| Ok(PackageEntry::new("plain").feature(Arc::new(Plain(names)))))
        .unwrap()
        .uses("std")
}

/// Prepares the document that uses the package `package` with `request`.
fn prepared_with(package: PackageFactory, request: Request) -> Result<flm_main::session::Prepared, flm_main::session::PrepareError> {
    let files = one_file("---\nflm: {use: [plain]}\n---\nHello.\n");
    prepare(request, Arc::new(files), &[package])
}

#[test]
fn an_output_that_only_another_workflow_delivers_is_a_problem_of_the_workflow() {
    let mut named = request(Some("only"));
    named.format = Some(String::from("plain"));
    assert_eq!(prepared_with(plain_package(&["only"]), named).expect("prepared").outputs(), ["plain"]);
    let error = prepared_with(plain_package(&["first", "second"]), format_alone("plain")).err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Workflow);
    assert_eq!(error.problems[0].detail, "the workflow `html` does not provide the format `plain`; it provides `html`");
}

/// A compiled package `peering`, whose feature declares as a peer a scoped package that no
/// feature defines: the registry is built, with a Note among its build diagnostics.
fn peering_package() -> PackageFactory {
    struct Peering;
    impl FlmFeature for Peering {
        fn info(&self) -> FeatureInfo {
            FeatureInfo::new("peering", "A peer declaration that nothing resolves")
        }
        fn register(&self, b: &mut FeatureBuilder<'_>) -> Result<(), BuildError> {
            b.peer_scoped_package("nosuch", "A package that no feature defines")
        }
    }
    PackageFactory::new("peering", |_| Ok(PackageEntry::new("peering").feature(Arc::new(Peering))))
        .unwrap()
        .uses("std")
}

#[test]
fn a_preparation_that_stops_after_the_registry_carries_its_build_diagnostics() {
    let files = one_file("---\nflm: {use: [peering]}\n---\nHello.\n");
    let mut named = request(None);
    named.profile = Some(String::from("nosuch"));
    let error = prepare(named, Arc::new(files), &[peering_package()]).err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Workflow);
    let peer = flm_core::core::registry::UnresolvedPeer::IDENTIFIER;
    assert!(error.diagnostics.iter().any(|d| d.identifier() == peer), "{:?}", error.diagnostics);
}

// --- test 18: a plan that does not build -------------------------------------------------

#[test]
fn a_plan_that_does_not_build_is_a_problem_of_the_plan() {
    let files = one_file("---\nflm:\n  features: {pieces: false}\n---\nHello.\n");
    let error = prepare(request(Some("text")), Arc::new(files), &[]).err().unwrap();
    assert_eq!(error.kind, PrepareErrorKind::Plan);
}

#[test]
fn a_document_without_formulas_runs_with_its_processing_feature_switched_off() {
    // The `math` feature processes no formula itself: without
    // `math-passthrough`, no workflow demands the check of the mathematics, so
    // a document without formulas runs without error.
    let files = one_file("---\nflm: {features: {math-passthrough: false}}\n---\nNo formula.\n");
    let built = run(files, request(Some("html")));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
}

// --- test 12: the output ------------------------------------------------------------------

const REFERENCED: &str = "\\section{Intro}\\label{sec:intro}\n\nSee \\ref{sec:intro}.\n";

/// A request for the workflow `workflow`, writing into `sink`.
fn writing(workflow: &str, sink: &Arc<MemorySink>) -> Request {
    let mut request = request(Some(workflow));
    request.naming = Naming::File { name: format!("paper.{}", if workflow == "html" { "html" } else { "tex" }) };
    request.sink = Some(Arc::clone(sink) as Arc<dyn Sink>);
    request
}

#[test]
fn a_memory_sink_receives_the_named_files() {
    let sink = Arc::new(MemorySink::new());
    run(one_file("Hello."), writing("html", &sink));
    let files = sink.files();
    assert_eq!(files.keys().collect::<Vec<_>>(), ["paper.html"]);
    assert!(String::from_utf8_lossy(&files["paper.html"].bytes).contains("Hello."));
}

#[test]
fn the_style_file_keeps_its_own_path_and_goes_when_no_longer_made() {
    let sink = Arc::new(MemorySink::new());
    run(one_file(REFERENCED), writing("latex-recomposed", &sink));
    assert_eq!(sink.files().len(), 2, "{:?}", sink.files().keys().collect::<Vec<_>>());
    assert!(sink.files().contains_key("paper.tex"));
    run(one_file("Plain text."), writing("latex-recomposed", &sink));
    assert_eq!(sink.files().keys().collect::<Vec<_>>(), ["paper.tex"]);
}

// --- test 13: the gate --------------------------------------------------------------------

#[test]
fn a_document_with_an_error_writes_nothing() {
    let sink = Arc::new(MemorySink::new());
    let built = run(one_file("Hello \\nosuchmacro."), writing("html", &sink));
    assert!(built.has_errors());
    assert!(sink.files().is_empty());
}

#[test]
fn continuing_with_errors_writes() {
    let sink = Arc::new(MemorySink::new());
    let mut request = writing("html", &sink);
    request.continue_with_errors = true;
    let built = run(one_file("Hello \\nosuchmacro."), request);
    assert!(built.has_errors());
    assert_eq!(sink.files().len(), 1);
}

#[test]
fn warnings_alone_write() {
    let sink = Arc::new(MemorySink::new());
    let mut request = writing("html", &sink);
    request.options.push(option("pieces.include=[nosuch.piece]"));
    let built = run(one_file("Hello."), request);
    assert!(!built.has_errors());
    assert!(built.diagnostics.iter().any(|d| d.severity() == flm_core::techy::error::Severity::Warning));
    assert_eq!(sink.files().len(), 1);
}

// --- every output of the workflow -----------------------------------------------------------

/// An extension `both` declaring the workflow `both`, which delivers the outputs `html` and
/// `text`. (A workflow delivering `html` and `latex-recomposed` cannot be planned: the
/// recomposed LaTeX lowers the tree as parsed, the page the tree laid out in paragraphs.)
const BOTH: &str = "flm:\n  workflows:\n    both:\n      extends: html\n      outputs: [html, text]\n";

/// The document `doc/paper.flm` holding `text` and using the extension `both` of `defs/`.
fn using_both(text: &str) -> MemoryFiles {
    MemoryFiles::new()
        .with("doc/paper.flm", &format!("---\nflm:\n  use: [both]\n---\n{text}"))
        .with("defs/both.yaml", BOTH)
}

/// A request for the workflow `both`, with the output `format` when given, writing into
/// `sink` under the name `paper.html`.
fn both_request(format: Option<&str>, sink: &Arc<MemorySink>) -> Request {
    let mut request = request(Some("both"));
    request.format = format.map(str::to_string);
    request.extension_folders = vec![ExtensionFolder { path: PathBuf::from("defs"), required: false }];
    request.naming = Naming::File { name: String::from("paper.html") };
    request.sink = Some(Arc::clone(sink) as Arc<dyn Sink>);
    request
}

/// The names of the outputs whose capability the plan of `prepared` demands.
fn demanded(prepared: &flm_main::session::Prepared) -> Vec<String> {
    prepared
        .registry()
        .outputs()
        .filter(|output| prepared.plan().demands().contains(&output.def.capability))
        .map(|output| output.def.name.to_string())
        .collect()
}

#[test]
fn a_request_produces_every_output_of_its_workflow() {
    let sink = Arc::new(MemorySink::new());
    let prepared = prepare(both_request(None, &sink), Arc::new(using_both("Hello.")), &[]).expect("prepared");
    assert_eq!(prepared.outputs(), ["html", "text"]);
    assert_eq!(demanded(&prepared), ["html", "text"]);
    let prefer = &prepared.plan().request().prefer;
    for output in ["html", "text"] {
        let stage = compose::stage_name(output);
        assert!(prefer.iter().any(|name| name.as_str() == stage.as_str()), "{output}: {prefer:?}");
    }
    let built = build(&prepared).expect("built");
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    // One delivery holds both outputs' files, named with one stem.
    let files = sink.files();
    assert_eq!(files.keys().collect::<Vec<_>>(), ["paper.html", "paper.txt"]);
    assert_eq!((files["paper.html"].output.as_str(), files["paper.txt"].output.as_str()), ("html", "text"));
    assert!(files.values().all(|file| file.entry));
    assert_eq!(files["paper.txt"].bytes, b"Hello.\n");
}

#[test]
fn a_format_narrows_the_request_to_one_output_of_its_workflow() {
    let sink = Arc::new(MemorySink::new());
    let prepared = prepare(both_request(Some("text"), &sink), Arc::new(using_both("Hello.")), &[]).expect("prepared");
    assert_eq!(prepared.outputs(), ["text"]);
    assert_eq!(demanded(&prepared), ["text"]);
    build(&prepared).expect("built");
    // The stem drops only the extension of an output the request produces.
    assert_eq!(sink.files().keys().collect::<Vec<_>>(), ["paper.html.txt"]);
}

#[test]
fn a_workflow_without_outputs_produces_none() {
    let prepared = prepare(request(None), Arc::new(one_file("---\nflm: {default_profile: check}\n---\nHi.\n")), &[]);
    assert!(prepared.expect("prepared").outputs().is_empty());
}

// --- a page and LaTeX for LuaLaTeX from one plan ---------------------------------------------

/// An extension `pagepaper` declaring the workflow `pagepaper`, which delivers the outputs
/// `html` and `latex-rendered`: both consume the tree laid out in paragraphs, so that one
/// plan serves both.
const PAGE_PAPER: &str =
    "flm:\n  workflows:\n    pagepaper:\n      extends: html\n      outputs: [html, latex-rendered]\n";

/// The files of the document `doc/paper.flm` holding `text` under the front matter's
/// settings `front` (lines below `flm:`), using the extension `pagepaper` of `defs/`.
fn using_page_paper(front: &str, text: &str) -> MemoryFiles {
    MemoryFiles::new()
        .with("doc/paper.flm", &format!("---\nflm:\n  use: [pagepaper]\n{front}---\n{text}\n"))
        .with("defs/pagepaper.yaml", PAGE_PAPER)
}

/// A request for the workflow `pagepaper`, writing into `sink` under the name `paper.html`,
/// with the command-line settings `options`.
fn page_paper_request(sink: &Arc<MemorySink>, options: &[&str]) -> Request {
    let mut request = both_request(None, sink);
    request.profile = Some(String::from("pagepaper"));
    request.extension_folders = vec![ExtensionFolder { path: PathBuf::from("defs"), required: false }];
    request.options.extend(options.iter().copied().map(option));
    request
}

#[test]
fn a_page_and_latex_rendered_share_one_plan_and_one_delivery() {
    let sink = Arc::new(MemorySink::new());
    let files = using_page_paper("", "\\section{Intro}\n\nHello.");
    let prepared = prepare(page_paper_request(&sink, &[]), Arc::new(files), &[]).expect("prepared");
    assert_eq!(prepared.outputs(), ["html", "latex-rendered"]);
    assert_eq!(demanded(&prepared), ["html", "latex-rendered"]);
    let built = build(&prepared).expect("built");
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    // One delivery: the page, the LaTeX document and its style file, named with one stem.
    let files = sink.files();
    let names: Vec<&String> = files.keys().collect();
    assert_eq!(names.len(), 3, "{names:?}");
    assert_eq!(files["paper.html"].output.as_str(), "html");
    assert_eq!(files["paper.tex"].output.as_str(), "latex-rendered");
    assert!(files["paper.html"].entry && files["paper.tex"].entry);
    let style = names.iter().find(|name| name.ends_with(".sty")).expect("the style file");
    assert!(style.starts_with("paper-flmx."), "{style}");
    assert!(!files[style.as_str()].entry);
    let tex = String::from_utf8(files["paper.tex"].bytes.clone()).unwrap();
    let package = style.trim_end_matches(".sty");
    assert!(tex.contains(&format!("\\usepackage{{{package}}}")), "{tex}");
    assert!(tex.contains("Hello."), "{tex}");
}

/// An image of one gray pixel, in the format `format`.
#[cfg(feature = "graphics")]
fn one_pixel(format: image::ImageFormat) -> Vec<u8> {
    let picture = image::DynamicImage::ImageRgba8(image::RgbaImage::from_pixel(1, 1, image::Rgba([128; 4])));
    let mut bytes = Vec::new();
    picture.write_to(&mut std::io::Cursor::new(&mut bytes), format).unwrap();
    bytes
}

#[cfg(feature = "graphics")]
#[test]
fn an_image_copied_for_both_outputs_is_delivered_once() {
    let sink = Arc::new(MemorySink::new());
    let mut files = using_page_paper(
        "  features:\n    graphics:\n      global: true\n",
        "A \\includegraphics{plot.png}.",
    );
    files.insert("doc/plot.png", one_pixel(image::ImageFormat::Png));
    let options = ["graphics.strategy.html=copy", "graphics.strategy.latex-rendered=copy"];
    let built = run(files, page_paper_request(&sink, &options));
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    let files = sink.files();
    let copies: Vec<&String> = files.keys().filter(|name| name.ends_with(".png")).collect();
    assert_eq!(copies.len(), 1, "{:?}", files.keys().collect::<Vec<_>>());
    let copy = copies[0];
    assert!(copy.starts_with("paper-plot-"), "{copy}");
    let page = String::from_utf8(files["paper.html"].bytes.clone()).unwrap();
    let tex = String::from_utf8(files["paper.tex"].bytes.clone()).unwrap();
    assert!(page.contains(&format!("src=\"{copy}\"")), "{page}");
    assert!(tex.contains(copy.as_str()), "{tex}");
}

#[cfg(feature = "graphics")]
#[test]
fn the_program_s_defaults_convert_a_gif_image_into_png_for_latex_rendered() {
    let sink = Arc::new(MemorySink::new());
    let mut files = MemoryFiles::new().with(
        "doc/paper.flm",
        "---\nflm:\n  features:\n    graphics:\n      global: true\n---\nA \\includegraphics{plot.gif}.\n",
    );
    files.insert("doc/plot.gif", one_pixel(image::ImageFormat::Gif));
    let mut request = writing("latex-rendered", &sink);
    request.options.push(option("graphics.strategy.latex-rendered=copy"));
    let built = run(files, request);
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    let files = sink.files();
    let copy = files.keys().find(|name| name.ends_with(".png")).expect("the converted copy");
    assert!(!files.keys().any(|name| name.ends_with(".gif")), "{:?}", files.keys().collect::<Vec<_>>());
    let tex = String::from_utf8(files["paper.tex"].bytes.clone()).unwrap();
    assert!(tex.contains(copy.as_str()), "{tex}");
}

// --- what *prepare* exposes ------------------------------------------------------------------

/// A document whose front matter writes metadata, a layout and a setting.
const EXPOSED: &str =
    "---\ntitle: A paper\nflm:\n  layout: inline\n  features:\n    text: {width: 50}\n    headings: {numbering: all-numbered}\n---\nHello.\n";

#[test]
fn prepare_exposes_the_front_matter_the_layout_the_search_path_and_the_document_s_options() {
    let folder = ExtensionFolder { path: PathBuf::from("ext"), required: false };
    let mut command = request(Some("text"));
    command.extension_folders = vec![folder.clone()];
    let prepared = prepare(command, Arc::new(one_file(EXPOSED)), &[]).expect("prepared");
    assert_eq!(
        prepared.front_matter_text(),
        Some("title: A paper\nflm:\n  layout: inline\n  features:\n    text: {width: 50}\n    headings: {numbering: all-numbered}\n")
    );
    assert_eq!(prepared.layout(), FragmentLayout::Inline);
    assert_eq!(prepared.search_path(), [folder]);
    let meta = prepared.document_options().get::<DocMeta>().expect("the metadata");
    assert_eq!(meta.title.as_ref().map(|title| title.text.as_str()), Some("A paper"));
    // Every package in use has the settings it was built from, and a setting written in the
    // front matter reaches the configuration of the package that holds its feature.
    assert!(prepared.packages().iter().all(|name| prepared.package_settings().contains_key(name.as_str())));
    let numbering: Vec<HeadingNumbering> = prepared
        .package_settings()
        .values()
        .filter_map(|settings| settings.config::<HeadingsConfig>("headings"))
        .map(|config| config.numbering)
        .collect();
    assert_eq!(numbering, [HeadingNumbering::AllNumbered]);
    // The request's layout replaces the front matter's; a file without a front matter has
    // no front matter text.
    let mut command = request(Some("text"));
    command.layout = Some(FragmentLayout::Blocks);
    let prepared = prepare(command, Arc::new(one_file("Hello.\n")), &[]).expect("prepared");
    assert_eq!(prepared.front_matter_text(), None);
    assert_eq!(prepared.layout(), FragmentLayout::Blocks);
}

#[test]
fn prepare_exposes_each_setting_s_resulting_value_with_where_it_was_written() {
    let prepared = prepare(request(Some("text")), Arc::new(one_file(EXPOSED)), &[]).expect("prepared");
    let width = prepared.settings().value("text", "width").expect("written");
    assert_eq!(width.value, serde_json::json!(50));
    assert_eq!(width.origin.source, Source::FrontMatter(String::from("doc/paper.flm")));
    assert_eq!(width.origin.place.map(|place| place.line), Some(6));
    // The command line's value wins over the front matter's, and keeps its origin.
    let mut command = request(Some("text"));
    command.options.push(option("text.width=72"));
    let prepared = prepare(command, Arc::new(one_file(EXPOSED)), &[]).expect("prepared");
    let width = prepared.settings().value("text", "width").expect("written");
    assert_eq!(width.value, serde_json::json!(72));
    assert_eq!(width.origin.source, Source::CommandLine(1, String::from("-O text.width=72")));
    assert_eq!(width.origin.place, None);
}

// --- the packages of a document whose front matter has other problems ------------------------

/// A front matter that chooses the packages (`initial_use`, `math`, `use`,
/// `default_profile`), with `metadata`, more root keys, before `flm` and `rest`, more keys,
/// under it; then a line of content.
fn choosing(metadata: &str, rest: &str) -> String {
    format!(
        "---\n{metadata}flm:\n  initial_use: [stdcore, stdmain]\n  math: passthrough\n  use: [std]\n  default_profile: mine\n{rest}---\nHello.\n"
    )
}

/// The compiled package `profile-mine`: the package of the profile `mine`.
fn profile_mine() -> PackageFactory {
    PackageFactory::new("profile-mine", |_| Ok(PackageEntry::new("profile-mine"))).expect("a package factory")
}

/// The names of the packages that `document_packages` finds for the document `text`, the
/// program carrying the package `profile-mine`.
fn found_with_profile(text: &str) -> Vec<String> {
    let found = document_packages(request(None), Arc::new(one_file(text)), &[profile_mine()])
        .unwrap_or_else(|error| panic!("found for {text:?}: {:?}", error.problems));
    found.factories.iter().map(|factory| factory.name().to_string()).collect()
}

#[test]
fn the_packages_are_found_although_the_rest_of_the_front_matter_has_problems() {
    let expected = ["stdcore", "stdmain", "math-passthrough", "std", "profile-mine"];
    for (metadata, rest) in [
        ("", ""),
        // A setting of the wrong type, which only the reading of the settings finds.
        ("", "  features:\n    html: {title: 5}\n"),
        // `features` that is not a map, a key that a document does not write under `flm`, a
        // layout that does not exist.
        ("", "  features: 5\n"),
        ("", "  nosuchkey: 1\n"),
        ("", "  layout: sideways\n"),
        // Metadata of the wrong shape.
        ("author: {name: A}\n", ""),
    ] {
        let text = choosing(metadata, rest);
        assert_eq!(found_with_profile(&text), expected, "{text}");
    }
    // *Prepare* reads the whole front matter, and stops.
    let text = choosing("", "  nosuchkey: 1\n");
    let error = prepare(request(None), Arc::new(one_file(&text)), &[profile_mine()]).err().expect("a problem");
    assert_eq!(error.kind, PrepareErrorKind::FrontMatter);
}

#[test]
fn the_packages_are_not_found_when_the_front_matter_is_not_yaml_or_a_key_that_decides_them_is_wrong() {
    for text in [
        "---\nflm: [\n---\nHello.\n",
        "---\nflm:\n  use: 3\n---\nHello.\n",
        "---\nflm:\n  use: [std, 2]\n---\nHello.\n",
        "---\nflm:\n  initial_use: {a: b}\n---\nHello.\n",
        "---\nflm:\n  math: 5\n---\nHello.\n",
        "---\nflm:\n  default_profile: [a]\n---\nHello.\n",
    ] {
        let error = document_packages(request(None), Arc::new(one_file(text)), &[])
            .err()
            .unwrap_or_else(|| panic!("a problem for {text:?}"));
        assert_eq!(error.kind, PrepareErrorKind::FrontMatter, "{text}");
        // The same problems as *prepare* reports, at the same places.
        let strict = prepare(request(None), Arc::new(one_file(text)), &[]).err().expect("a problem");
        assert_eq!(error.problems, strict.problems, "{text}");
    }
}

/// A document whose front matter writes a setting, and an entry of a key merged entry by
/// entry.
const IN_EFFECT: &str =
    "---\nflm:\n  features:\n    text: {width: 50}\n    userdefs:\n      macros:\n        hello: {content: Hello}\n---\nHello.\n";

#[test]
fn prepare_lists_the_settings_in_effect_with_where_each_was_written() {
    let prepared = prepare(request(Some("text")), Arc::new(one_file(IN_EFFECT)), &[]).expect("prepared");
    let in_effect = prepared.settings_in_effect();
    let front_matter = Source::FrontMatter(String::from("doc/paper.flm"));
    let width = &in_effect["text"].keys["width"];
    assert_eq!(width.value, serde_json::json!(50));
    assert_eq!(width.origin.source, front_matter);
    assert_eq!(width.origin.place.map(|place| place.line), Some(4));
    // The entry the front matter adds is in the key's value, with the front matter as its
    // origin.
    let macros = &in_effect["userdefs"].keys["macros"];
    assert_eq!(macros.value["hello"], serde_json::json!({ "content": "Hello" }));
    let hello = &macros.entry_origins["hello"];
    assert_eq!(hello.source, front_matter);
    assert_eq!(hello.place.map(|place| place.line), Some(7));
    // Every feature listed is one of a package in use.
    assert!(in_effect.keys().all(|feature| {
        prepared.factories().iter().any(|factory| factory.features().iter().any(|entry| entry.name == feature.as_str()))
    }));
}

// --- the manifests of the extensions in use --------------------------------------------------

/// A data extension, beside the document in `_flmextensions`, whose manifest writes a
/// description and an author.
const NARROW: &str = "extension:\n  description: Narrow text.\n  author: Ada Lovelace\nflm:\n  features:\n    text:\n      width: 20\n";

/// The document `text` with the extension `narrow` in the folder `doc/_flmextensions`, and a
/// request for it that searches that folder.
fn with_narrow(text: &str) -> (MemoryFiles, Request) {
    let files = one_file(text).with("doc/_flmextensions/narrow.yaml", NARROW);
    let mut command = request(Some("text"));
    command.extension_folders = vec![ExtensionFolder { path: PathBuf::from("doc/_flmextensions"), required: false }];
    (files, command)
}

#[test]
fn prepare_keeps_the_manifest_of_each_extension_in_use() {
    let (files, command) = with_narrow("---\nflm:\n  use: [narrow]\n---\nHello.\n");
    let prepared = prepare(command, Arc::new(files), &[]).expect("prepared");
    let identity = "doc/_flmextensions/narrow.yaml";
    assert!(prepared.packages().iter().any(|name| *name == format!("{identity}#_")), "{:?}", prepared.packages());
    let manifest = prepared.manifest(identity).expect("the manifest of the extension in use");
    assert_eq!(manifest.description.as_deref(), Some("Narrow text."));
    assert_eq!(manifest.author.as_deref(), Some("Ada Lovelace"));
    assert_eq!(manifest.engine, "data");
    // A compiled package has no manifest; nor has an extension found but not in use.
    assert!(prepared.manifest("std").is_none());
    let (files, command) = with_narrow("Hello.\n");
    let prepared = prepare(command, Arc::new(files), &[]).expect("prepared");
    assert!(prepared.manifest(identity).is_none());
}

#[cfg(feature = "bundled-extensions")]
#[test]
fn prepare_keeps_the_manifest_of_a_bundled_extension_in_use() {
    let text = "---\nflm:\n  use: [lecturenotes]\n---\nHello.\n";
    let prepared = prepare(request(Some("text")), Arc::new(one_file(text)), &[]).expect("prepared");
    let manifest = prepared.manifest("bundled:data/lecturenotes").expect("the bundled extension's manifest");
    assert!(manifest.description.as_deref().is_some_and(|text| text.starts_with("The lecture-notes page")), "{manifest:?}");
    assert_eq!(manifest.engine, "data");
}

/// The manifest kept for an extension written in Rust and compiled into the program is the
/// one the loader read: its engine is `native`, the engine that runs it, although the file
/// specifies `wasm`. The extension's package declares the host service `programs`, which the
/// request offers (nothing is run).
#[cfg(all(feature = "bundled-extensions", feature = "fs"))]
#[test]
fn prepare_keeps_the_engine_native_for_an_extension_compiled_in_rust() {
    let mut command = request(Some("pdf-latex"));
    command.offer_programs = true;
    let prepared = prepare(command, Arc::new(one_file("Hello.\n")), &[]).expect("prepared");
    let identity = "bundled:rust/profile-pdf-latex";
    assert_eq!(prepared.profile_package(), Some("profile-pdf-latex"));
    assert!(prepared.packages().iter().any(|name| *name == format!("{identity}#_")), "{:?}", prepared.packages());
    let manifest = prepared.manifest(identity).expect("the manifest of the profile's extension");
    assert_eq!(manifest.engine, "native");
    assert!(manifest.description.is_some(), "{manifest:?}");
}
