//! The printer: each kind of place a problem is reported at, as text for a person, and the
//! last line's counts; then the same diagnostics and problems as records, the data a program
//! reads, with their byte offsets counted from the top of each file; then the text of the
//! progress line and the kinds of the table of expected durations shipped with the crate.
//! The documents are read from `MemoryFiles`; no test touches the disk.

use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use flm_core::lang::FlmDiagnostic;
use flm_core::progress::{Count, LogLevel, RowId, RowSnapshot, RowState, Scope, Snapshot, Unit};
use flm_core::techy::error::Severity;
use flm_host::files::MemoryFiles;
use flm_host::report::{
    ContentOffsets, ProgressRow, Record, RecordSeverity, counts, diagnostic, problem, progress_line,
    report,
};
use flm_host::session::{Built, Input, Request, build, prepare};
use serde_json::{Value, json};

/// A request for a text render of `doc/paper.flm`.
fn paper() -> Request {
    let mut request = Request::new(Input::File(String::from("doc/paper.flm")));
    request.default_math = flm_host::seed::MathChoice::processor("passthrough");
    request.profile = Some(String::from("text"));
    request
}

/// The build of a text render of `doc/paper.flm` from `files`.
fn built(files: MemoryFiles) -> Built {
    let prepared = prepare(paper(), Arc::new(files), &[], &Scope::none()).expect("prepared");
    build(&prepared, &Scope::none()).expect("built")
}

/// The diagnostics of a text render of `doc/paper.flm` holding `text`.
fn diagnostics(text: &str) -> Vec<FlmDiagnostic> {
    built(MemoryFiles::new().with("doc/paper.flm", text)).diagnostics
}

/// Every error of `diagnostics`, printed.
fn errors(diagnostics: &[FlmDiagnostic]) -> Vec<String> {
    diagnostics.iter().filter(|entry| entry.severity() == Severity::Error).map(diagnostic).collect()
}

#[test]
fn a_problem_in_the_author_s_text_is_printed_at_its_place() {
    let printed = errors(&diagnostics("Hello.\n\nA \\nosuchmacro here.\n"));
    assert_eq!(printed.len(), 1, "{printed:?}");
    let lines: Vec<&str> = printed[0].lines().collect();
    assert!(lines[0].starts_with("error: "), "{printed:?}");
    assert_eq!(lines[1], "  at doc/paper.flm:3:3", "{printed:?}");
    // The problem is in the author's own text: no content made from it.
    assert!(!printed[0].contains("in content made from it"), "{printed:?}");
}

#[test]
fn a_problem_in_content_a_user_definition_made_leads_with_the_author_s_line() {
    let text = "---\nflm:\n  features:\n    userdefs:\n      macros:\n        see: {content: 'see \\ref{nowhere}'}\n---\nFirst line.\n\nThen \\see{} here.\n";
    let printed = errors(&diagnostics(text));
    let expanded = printed
        .iter()
        .find(|entry| entry.contains("in content made from it"))
        .unwrap_or_else(|| panic!("a problem inside the expansion: {printed:?}"));
    let lines: Vec<&str> = expanded.lines().collect();
    // The author's line first: where the definition is used.
    assert_eq!(lines[1], "  at doc/paper.flm:10:6", "{expanded}");
    assert!(lines.last().unwrap().starts_with("  in content made from it: "), "{expanded}");
}

#[test]
fn a_problem_inside_a_copy_shown_elsewhere_lists_where_the_copy_is_shown() {
    let text = "\\section*{See \\ref{nowhere}}\\label{sec:e}\n\nSee \\ref{sec:e}.\n";
    let printed = errors(&diagnostics(text));
    let framed: Vec<&String> = printed.iter().filter(|entry| entry.contains("\n  inside ")).collect();
    assert_eq!(framed.len(), 1, "{printed:?}");
    let lines: Vec<&str> = framed[0].lines().collect();
    assert_eq!(lines[1], "  at doc/paper.flm:1:15", "{}", framed[0]);
    assert!(lines[2].starts_with("  inside doc/paper.flm:3:5: "), "{}", framed[0]);
}

#[test]
fn a_problem_in_the_front_matter_is_printed_at_its_line() {
    let files = MemoryFiles::new().with("doc/paper.flm", "---\ntitle: A title\nauthor: {affiliation: A}\n---\nText.\n");
    let error = prepare(Request::new(Input::File(String::from("doc/paper.flm"))), Arc::new(files), &[], &Scope::none())
        .expect_err("a problem of the front matter");
    let printed = problem(&error.problems[0]);
    assert!(printed.starts_with("error: "), "{printed}");
    assert!(printed.ends_with("\n  at doc/paper.flm:3:9"), "{printed}");
}

#[test]
fn the_last_line_counts_the_errors_and_the_warnings() {
    assert_eq!(counts(0, 0), "0 errors, 0 warnings");
    assert_eq!(counts(1, 1), "1 error, 1 warning");
    assert_eq!(counts(2, 3), "2 errors, 3 warnings");
    let found = diagnostics("A \\nosuchmacro and \\nosuchmacro.\n");
    let printed = report(&found);
    assert_eq!(printed.lines().last(), Some("2 errors, 0 warnings"), "{printed}");
    // Every diagnostic is printed, the two alike included.
    assert_eq!(printed.matches("error: ").count(), 2, "{printed}");
}

// --- records -----------------------------------------------------------------------------

/// Every diagnostic of `built` as a record, as JSON, with the build's content offsets.
fn records(built: &Built) -> Vec<Value> {
    built
        .diagnostics
        .iter()
        .map(|entry| serde_json::to_value(Record::of_diagnostic(entry, &built.content_offsets)).expect("JSON"))
        .collect()
}

/// The message of the first Error of `built`.
fn first_message(built: &Built) -> String {
    built.diagnostics.iter().find(|entry| entry.severity() == Severity::Error).expect("an error").message()
}

#[test]
fn a_diagnostic_with_a_frame_records_its_place_and_the_frame_s() {
    let built = built(MemoryFiles::new().with("doc/paper.flm", "\\section*{See \\ref{nowhere}}\\label{sec:e}\n\nSee \\ref{sec:e}.\n"));
    let mut framed: Vec<Value> = records(&built).into_iter().filter(|record| record["frames"] != json!([])).collect();
    // A frame's title is text for a person: the record holds one, and its words are not
    // compared.
    for frame in framed.iter_mut().flat_map(|record| record["frames"].as_array_mut().expect("frames")) {
        assert!(frame["title"].is_string(), "{frame}");
        frame.as_object_mut().expect("a frame").remove("title");
    }
    let expected = json!({
        "kind": "diagnostic",
        "severity": "error",
        "id": "flm.refs.bad-label",
        "data": {"text": "nowhere"},
        "message": first_message(&built),
        "place": {"file": "doc/paper.flm", "offset": 14, "line": 1, "column": 15, "end": {"offset": 27, "line": 1, "column": 28}},
        "frames": [{
            "place": {"file": "doc/paper.flm", "offset": 47, "line": 3, "column": 5, "end": {"offset": 58, "line": 3, "column": 16}}
        }]
    });
    assert_eq!(framed, [expected]);
}

#[test]
fn a_diagnostic_in_content_a_user_definition_made_records_the_label_of_that_content() {
    // A front matter of 97 bytes; the use of `\see` is on line 10, at byte 18 of the content.
    let text = "---\nflm:\n  features:\n    userdefs:\n      macros:\n        see: {content: 'see \\ref{nowhere}'}\n---\nFirst line.\n\nThen \\see{} here.\n";
    let built = built(MemoryFiles::new().with("doc/paper.flm", text));
    let expanded: Vec<Value> = records(&built).into_iter().filter(|record| record.get("made_from").is_some()).collect();
    let expected = json!({
        "kind": "diagnostic",
        "severity": "error",
        "id": "flm.refs.bad-label",
        "data": {"text": "nowhere"},
        "message": first_message(&built),
        "place": {"file": "doc/paper.flm", "offset": 115, "line": 10, "column": 6, "end": {"offset": 119, "line": 10, "column": 10}},
        "frames": [],
        "made_from": {"label": "expansion of \\see"}
    });
    assert_eq!(expanded, [expected]);
}

/// The place of the first record of a render of `files` whose condition is an unknown
/// command.
fn unknown_command_place(files: MemoryFiles) -> Value {
    let found = records(&built(files));
    let record = found.iter().find(|record| record["id"] == "core.specs.unresolvable-command").expect("an unknown command");
    record["place"].clone()
}

#[test]
fn a_byte_offset_counts_from_the_top_of_the_file_past_its_front_matter() {
    // The first byte of the content of a file with a three-line front matter: the front
    // matter's byte length is its offset, and the line where the content starts its line.
    let front_matter = "---\ntitle: One\ndate: Two\nsubject: Three\n---\n";
    let text = format!("{front_matter}\\nosuchmacro here.\n");
    let place = unknown_command_place(MemoryFiles::new().with("doc/paper.flm", &text));
    assert_eq!(place["offset"], json!(front_matter.len()));
    assert_eq!(place["offset"], json!(44));
    assert_eq!(place["line"], json!(6));
    assert_eq!(place["column"], json!(1));
    // Without a front matter, the content is the whole file.
    let place = unknown_command_place(MemoryFiles::new().with("doc/paper.flm", "\\nosuchmacro here.\n"));
    assert_eq!(place, json!({"file": "doc/paper.flm", "offset": 0, "line": 1, "column": 1, "end": {"offset": 13, "line": 1, "column": 14}}));
    // A file included with `\input` counts past its own front matter, of 22 bytes.
    let files = MemoryFiles::new()
        .with("doc/paper.flm", "Hello \\input{part.flm} end.\n")
        .with("doc/part.flm", "---\ntitle: A part\n---\n\\nosuchmacro.\n");
    let place = unknown_command_place(files);
    assert_eq!(place, json!({"file": "doc/part.flm", "offset": 22, "line": 4, "column": 1, "end": {"offset": 34, "line": 4, "column": 13}}));
}

/// Every place a record of `record` holds: its leading position, its frames' places and
/// the place in the content the library made.
fn places(record: &Value) -> Vec<Value> {
    let mut found = vec![record["place"].clone()];
    found.extend(record["frames"].as_array().expect("frames").iter().map(|frame| frame["place"].clone()));
    found.extend(record.get("made_from").cloned());
    found
}

#[test]
fn a_problem_in_a_text_of_the_front_matter_records_its_line_and_column_and_no_byte_offset() {
    // The document's title is parsed from the front matter: its places have no byte offset,
    // and their columns count bytes (`é` is two).
    for (title, column) in [("See", 12), ("Café", 14)] {
        let text = format!("---\ntitle: {title} \\nosuchmacro here\n---\nHello.\n");
        let found = records(&built(MemoryFiles::new().with("doc/paper.flm", &text)));
        let record = found.iter().find(|record| record["id"] == "core.specs.unresolvable-command").expect("an unknown command");
        assert_eq!(record["place"], json!({"file": "doc/paper.flm", "line": 2, "column": column}), "{title}");
        assert!(places(record).iter().all(|place| place.get("offset").is_none()), "{record}");
    }
    // The title of a file included with `\input` that opens with a heading: the heading is
    // parsed from a text the library makes, at the title's place in the front matter.
    let files = MemoryFiles::new()
        .with("doc/paper.flm", "Hello.\n\n\\input{part.flm}\n")
        .with("doc/part.flm", "---\nflm:\n  input_as: {heading: section}\ntitle: A \\nosuchmacro title\n---\nText.\n");
    let found = records(&built(files));
    let record = found.iter().find(|record| record["id"] == "core.specs.unresolvable-command").expect("an unknown command");
    assert_eq!(record["place"], json!({"file": "doc/part.flm", "line": 4, "column": 10}));
    // The frames in the heading have no byte offset either; the last frame, the `\input`
    // command in the including file's content, has one.
    let (heading, including): (Vec<Value>, Vec<Value>) =
        places(record).into_iter().partition(|place| place["file"] == "doc/part.flm");
    assert!(heading.len() > 1 && heading.iter().all(|place| place.get("offset").is_none() && place.get("end").is_none()), "{record}");
    assert_eq!(including, [json!({"file": "doc/paper.flm", "offset": 8, "line": 3, "column": 1, "end": {"offset": 24, "line": 3, "column": 17}})]);
}

#[test]
fn a_diagnostic_placed_by_its_line_and_column_alone_records_no_byte_offset() {
    // `false` for a feature that no package in use has: a warning at the place where the
    // value is written, in the front matter, with no text of the file behind it.
    let files = MemoryFiles::new().with("doc/paper.flm", "---\nflm:\n  features:\n    nosuchfeature: false\n---\nHello.\n");
    let prepared = prepare(paper(), Arc::new(files), &[], &Scope::none()).expect("prepared");
    let warning = prepared.diagnostics().first().expect("a warning");
    let record = serde_json::to_value(Record::of_diagnostic(warning, &ContentOffsets::new())).expect("JSON");
    assert_eq!(record["severity"], "warning");
    assert_eq!(record["id"], "flm.host.unknown-feature");
    assert_eq!(record["place"], json!({"file": "doc/paper.flm", "line": 4, "column": 20}));
}

#[test]
fn a_problem_records_its_message_and_its_place_when_known() {
    let files = MemoryFiles::new().with("doc/paper.flm", "---\ntitle: A title\nauthor: {affiliation: A}\n---\nText.\n");
    let error = prepare(paper(), Arc::new(files), &[], &Scope::none()).expect_err("a problem of the front matter");
    let record = serde_json::to_value(Record::of_problem(&error.problems[0])).expect("JSON");
    assert_eq!(
        record,
        json!({
            "kind": "problem",
            "severity": "error",
            "message": error.problems[0].detail,
            "place": {"file": "doc/paper.flm", "line": 3, "column": 9}
        })
    );
    // The column counts bytes: `é` is two.
    let files = MemoryFiles::new().with("doc/paper.flm", "---\nflm: {features: {}, éé: 1}\n---\nText.\n");
    let error = prepare(paper(), Arc::new(files), &[], &Scope::none()).expect_err("a problem of the front matter");
    let record = serde_json::to_value(Record::of_problem(&error.problems[0])).expect("JSON");
    assert_eq!(record["place"], json!({"file": "doc/paper.flm", "line": 2, "column": 27}));
    // A problem with no file and no place records none.
    let mut nowhere = error.problems[0].clone();
    nowhere.file = None;
    nowhere.place = None;
    let record = serde_json::to_value(Record::of_problem(&nowhere)).expect("JSON");
    assert_eq!(record, json!({"kind": "problem", "severity": "error", "message": nowhere.detail}));
}

#[test]
fn the_counts_a_build_s_start_a_line_and_the_header_record_as_data() {
    let found = diagnostics("A \\nosuchmacro and \\nosuchmacro.\n");
    assert_eq!(serde_json::to_value(Record::summary(&found)).unwrap(), json!({"kind": "summary", "errors": 2, "warnings": 0}));
    let started = UNIX_EPOCH + Duration::from_millis(1_500);
    assert_eq!(serde_json::to_value(Record::build(3, started)).unwrap(), json!({"kind": "build", "number": 3, "time": 1500}));
    let text = String::from("read doc/paper.flm");
    let line = Record::Log { level: LogLevel::Info, source: String::from("flm"), text };
    assert_eq!(
        serde_json::to_value(line).unwrap(),
        json!({"kind": "log", "level": "info", "source": "flm", "text": "read doc/paper.flm"})
    );
    let meta = serde_json::to_value(Record::meta()).unwrap();
    assert_eq!(meta, json!({"kind": "messages", "flm": env!("CARGO_PKG_VERSION"), "format": 1}));
    // One record is one line of JSON.
    let text = serde_json::to_string(&Record::of_diagnostic(&found[0], &ContentOffsets::new())).unwrap();
    assert!(!text.contains('\n'), "{text}");
}

#[test]
fn a_record_s_severity_is_spelled_as_the_projection_spells_it() {
    for severity in [Severity::Error, Severity::Warning, Severity::Note] {
        assert_eq!(serde_json::to_value(RecordSeverity::from(severity)).unwrap(), json!(severity.to_string()));
    }
}

#[test]
fn the_schema_of_a_record_describes_each_kind() {
    let schema = serde_json::to_string(&schemars::schema_for!(Record)).unwrap();
    for kind in ["diagnostic", "problem", "summary", "preview-link", "build", "log", "progress"] {
        assert!(schema.contains(&format!("\"{kind}\"")), "{kind}: {schema}");
    }
    for key in ["made_from", "frames", "offset", "column", "level", "source", "fraction", "row", "total", "done"] {
        assert!(schema.contains(key), "{key}: {schema}");
    }
    // A progress record and the row it shows, as data.
    let row = ProgressRow {
        kind: String::from("flm.graphics.convert"),
        title: Some(String::from("Converting graphics")),
        label: Some(String::from("figure-3.pdf")),
        count: Some(Count { unit: Unit::Items, total: 4, done: 1 }),
    };
    let text = String::from("50% Converting graphics figure-3.pdf 1/4");
    let record = Record::Progress { fraction: 0.5, text, row: Some(row) };
    assert_eq!(
        serde_json::to_value(record).unwrap(),
        json!({
            "kind": "progress",
            "fraction": 0.5,
            "text": "50% Converting graphics figure-3.pdf 1/4",
            "row": {
                "kind": "flm.graphics.convert",
                "title": "Converting graphics",
                "label": "figure-3.pdf",
                "count": {"unit": "items", "total": 4, "done": 1},
            },
        })
    );
    let shown = Record::Progress { fraction: 0.0, text: String::from("0%"), row: None };
    assert_eq!(serde_json::to_value(shown).unwrap(), json!({"kind": "progress", "fraction": 0.0, "text": "0%"}));
}

/// A row of a snapshot written by hand: `id` under `parent`, of the kind `kind` with the
/// title `title`, labeled `label`, in the state `state`, with the count `count`.
fn row(
    id: u64,
    parent: Option<u64>,
    kind: &'static str,
    title: Option<&'static str>,
    label: Option<&'static str>,
    state: RowState,
    count: Option<Count>,
) -> RowSnapshot {
    RowSnapshot {
        id: RowId(id),
        parent: parent.map(RowId),
        kind: kind.into(),
        title: title.map(Into::into),
        label: label.map(Into::into),
        state,
        count,
        started: None,
        ended: None,
    }
}

#[test]
fn the_progress_line_shows_the_deepest_running_row_that_has_a_title() {
    let fetch = Some(Count { unit: Unit::Items, total: 12, done: 3 });
    let snapshot = Snapshot {
        cancelled: false,
        rows: vec![
            row(0, None, "test.build", None, None, RowState::Running, None),
            row(1, Some(0), "flm.host.run", Some("Processing document"), None, RowState::Running, None),
            row(2, Some(1), "flm.citations.fetch", Some("Fetching citation data"), Some("refs"), RowState::Running, fetch),
            // At the same depth, declared after: passed over.
            row(3, Some(1), "test.other", Some("Converting graphics"), Some("figure.pdf"), RowState::Running, None),
            // Deeper, with no title: passed over.
            row(4, Some(2), "test.request", None, Some("arXiv:2101.00001"), RowState::Running, None),
            // Deeper, with a title, but finished.
            row(5, Some(2), "test.done", Some("Reading document"), Some("done.flm"), RowState::Finished, None),
        ],
    };
    let text = progress_line(&snapshot, 0.42);
    assert!(text.starts_with("42%"), "{text}");
    for shown in ["Fetching citation data", "refs", "3/12"] {
        assert!(text.contains(shown), "{shown}: {text}");
    }
    for passed in ["Processing document", "Converting graphics", "figure.pdf", "arXiv:2101.00001", "done.flm"] {
        assert!(!text.contains(passed), "{passed}: {text}");
    }
    // Below 100 while the root row runs, whatever the fraction.
    assert!(progress_line(&snapshot, 1.0).starts_with("99%"));
    // The record shows the same row.
    let Record::Progress { row: Some(shown), .. } = Record::progress(&snapshot, 0.42) else {
        panic!("a progress record with a row");
    };
    assert_eq!(shown.kind, "flm.citations.fetch");
    assert_eq!(shown.count, fetch);
}

/// The names of the stages that a preparation of `text`, a document in an HTML render,
/// registers, with the math processor `math`, the request offering the extensions shipped
/// with FLM.
#[cfg(feature = "js")]
fn stage_names(text: &str, math: &str) -> Vec<String> {
    let mut request = Request::new(Input::File(String::from("paper.flm")));
    request.bundled = flm_host::bundled::Bundled::new(flm_extensions::EXTENSIONS);
    #[cfg(feature = "native")]
    {
        request.bundled = request.bundled.with_code(flm_extensions::COMPILED_EXTENSIONS);
    }
    request.default_math = flm_host::seed::MathChoice::processor(math);
    request.profile = Some(String::from("html"));
    let files = MemoryFiles::new().with("paper.flm", text);
    let prepared = prepare(request, Arc::new(files), &[], &Scope::none()).expect("prepared");
    prepared.registry().stages().iter().map(|stage| stage.name.to_string()).collect()
}

#[cfg(all(feature = "js", feature = "citations", feature = "fs"))]
#[test]
fn every_kind_of_the_default_table_is_a_kind_constant_or_a_stage() {
    let table = flm_host::report::expected_durations();
    assert!(table.iter().count() > 0, "the shipped table is read");
    let mut known: Vec<String> = [
        &flm_host::session::PREPARE_KIND,
        &flm_host::session::BUILD_KIND,
        &flm_host::session::PACKAGES_KIND,
        &flm_host::session::REGISTRY_KIND,
        &flm_host::session::PLAN_KIND,
        &flm_host::session::PARSE_KIND,
        &flm_host::session::RUN_KIND,
        &flm_host::output::FILES_KIND,
        &flm_host::citations::FETCH_KIND,
        &flm_host::host_programs::RUN_KIND,
        &flm_core::core::run::REWRITES_KIND,
        &flm_core::stdlib::features::graphics::IMAGES_KIND,
        &flm_core::stdlib::features::graphics::CONVERT_KIND,
    ]
    .iter()
    .map(|kind| kind.name().to_string())
    .collect();
    // The kind that the script of the bundled extension `math-mathjax` declares.
    let render = flm_extensions::EXTENSIONS
        .iter()
        .find(|extension| extension.stem == "math-mathjax")
        .and_then(|extension| extension.file("render.js"))
        .expect("render.js");
    let script = std::str::from_utf8(render.bytes).unwrap();
    let formulas = "math-mathjax.formulas";
    assert!(script.contains(&format!("const FORMULAS_KIND = \"{formulas}\";")), "{formulas}");
    known.push(formulas.to_string());
    // The stages of `std`, and of the packages shipped with the host that a build uses.
    known.extend(stage_names("---\nflm:\n  features:\n    math-mathjax:\n      mode: mathml\n---\n\\(x\\)\n", "mathjax"));
    known.extend(stage_names("\\(x\\)\n", "mathjax"));
    known.extend(stage_names("---\nflm:\n  use: [letter]\n---\nDear reader.\n", "passthrough"));
    for (kind, _) in table.iter() {
        assert!(known.iter().any(|name| name == kind), "{kind} is no kind of the workspace");
    }
}
