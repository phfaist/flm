//! The printer: each kind of place a problem is reported at, as text for a person, and the
//! last line's counts; then the same diagnostics and problems as records, the data a program
//! reads, with their byte offsets counted from the top of each file. The documents are read
//! from `MemoryFiles`; no test touches the disk.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use flm_core::lang::FlmDiagnostic;
use flm_core::techy::error::Severity;
use flm_main::files::MemoryFiles;
use flm_main::report::{ContentOffsets, Record, RecordSeverity, counts, diagnostic, problem, report};
use flm_main::session::{Built, Input, Request, build, prepare};
use serde_json::{Value, json};

/// A request for a text render of `doc/paper.flm`.
fn paper() -> Request {
    let mut request = Request::new(Input::File(PathBuf::from("doc/paper.flm")));
    request.default_math = flm_main::seed::MathChoice::processor("passthrough");
    request.profile = Some(String::from("text"));
    request
}

/// The build of a text render of `doc/paper.flm` from `files`.
fn built(files: MemoryFiles) -> Built {
    let prepared = prepare(paper(), Arc::new(files), &[]).expect("prepared");
    build(&prepared).expect("built")
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
    let files = MemoryFiles::new().with("doc/paper.flm", "---\ntitle: A title\nauthor: {name: A}\n---\nText.\n");
    let error = prepare(Request::new(Input::File(PathBuf::from("doc/paper.flm"))), Arc::new(files), &[])
        .err()
        .expect("a problem of the front matter");
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
    let framed: Vec<Value> = records(&built).into_iter().filter(|record| record["frames"] != json!([])).collect();
    let expected = json!({
        "kind": "diagnostic",
        "severity": "error",
        "id": "flm.refs.bad-label",
        "data": {"text": "nowhere"},
        "message": first_message(&built),
        "place": {"file": "doc/paper.flm", "offset": 14, "line": 1, "column": 15, "end": {"offset": 27, "line": 1, "column": 28}},
        "frames": [{
            "place": {"file": "doc/paper.flm", "offset": 47, "line": 3, "column": 5, "end": {"offset": 58, "line": 3, "column": 16}},
            "title": "a copy of this content, shown here"
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
    let prepared = prepare(paper(), Arc::new(files), &[]).expect("prepared");
    let warning = prepared.diagnostics().first().expect("a warning");
    let record = serde_json::to_value(Record::of_diagnostic(warning, &ContentOffsets::new())).expect("JSON");
    assert_eq!(record["severity"], "warning");
    assert_eq!(record["id"], "flm.host.unknown-feature-switched-off");
    assert_eq!(record["place"], json!({"file": "doc/paper.flm", "line": 4, "column": 20}));
}

#[test]
fn a_problem_records_its_message_and_its_place_when_known() {
    let files = MemoryFiles::new().with("doc/paper.flm", "---\ntitle: A title\nauthor: {name: A}\n---\nText.\n");
    let error = prepare(paper(), Arc::new(files), &[]).err().expect("a problem of the front matter");
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
    let error = prepare(paper(), Arc::new(files), &[]).err().expect("a problem of the front matter");
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
    let line = Record::Log { text: String::from("read doc/paper.flm") };
    assert_eq!(serde_json::to_value(line).unwrap(), json!({"kind": "log", "text": "read doc/paper.flm"}));
    let meta = serde_json::to_value(Record::meta()).unwrap();
    assert_eq!(meta, json!({"kind": "diagnostics", "flm": env!("CARGO_PKG_VERSION"), "format": 1}));
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
    for kind in ["diagnostic", "problem", "summary", "build", "log"] {
        assert!(schema.contains(&format!("\"{kind}\"")), "{kind}: {schema}");
    }
    for key in ["made_from", "frames", "offset", "column"] {
        assert!(schema.contains(key), "{key}: {schema}");
    }
}
