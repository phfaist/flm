//! The engine of `flm dump`, called as a library: what it gives, in order, and its
//! failures with their statuses. The `flm` program's own tests (`flm-cli/tests/dump.rs`)
//! check the command end to end.

use flm_info::dump::{At, Printed, Request, Subject, View, dump};

/// Runs the engine over `text` and returns the diagnostics texts and the views it gave,
/// or its error.
fn run(request: &Request, text: &str) -> Result<(Vec<String>, Vec<String>), flm_info::dump::Error> {
    let mut diagnostics = Vec::new();
    let mut views = Vec::new();
    dump(request, text.to_string(), "doc.flm", &mut |printed| match printed {
        Printed::Diagnostics(text) => diagnostics.push(text.to_string()),
        Printed::View(text) => views.push(text.to_string()),
    })?;
    Ok((diagnostics, views))
}

#[test]
fn the_parsed_tree_is_given_once_as_json() {
    let (diagnostics, views) = run(&Request::new(Subject::Tree(At::Parsed)), "Hello \\emph{world}.").expect("dumped");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(views.len(), 1);
    serde_json::from_str::<serde_json::Value>(&views[0]).expect("the view is JSON");
    let mut wire = Request::new(Subject::Tree(At::Parsed));
    wire.view = View::Wire;
    let (_, views) = run(&wire, "Hello.").expect("dumped");
    assert!(!views[0].contains('\n'), "the wire view is one line");
}

#[test]
fn the_frozen_tree_and_a_table_come_from_a_run() {
    let (_, views) = run(&Request::new(Subject::Tree(At::Frozen)), "Hello.").expect("dumped");
    assert_eq!(views.len(), 1);
    let (_, views) = run(&Request::new(Subject::Output(String::from("text"))), "Hello.").expect("dumped");
    assert!(views[0].contains("\"flm.output.text"), "{}", views[0]);
}

#[test]
fn a_document_s_diagnostics_are_given_before_its_view() {
    let (diagnostics, views) = run(&Request::new(Subject::Tree(At::Parsed)), "A \\nosuchmacro here.").expect("dumped");
    assert_eq!(diagnostics.len(), 1, "{diagnostics:?}");
    assert!(diagnostics[0].contains("doc.flm"), "{}", diagnostics[0]);
    assert_eq!(views.len(), 1);
}

#[test]
fn the_failures_carry_their_statuses() {
    let error = run(&Request::new(Subject::Table(String::from("no.such.table"))), "Hello.").expect_err("an unknown table");
    assert_eq!(error.status, 1);
    assert!(error.message.starts_with("the table `no.such.table` must be one of the tables: "), "{}", error.message);
    let error = run(&Request::new(Subject::Tree(At::Lowered(String::from("nope")))), "Hello.").expect_err("an unknown backend");
    assert_eq!(error.status, 1);
    let mut unknown = Request::new(Subject::Tree(At::Frozen));
    unknown.workflow = String::from("no-such-workflow");
    let error = run(&unknown, "Hello.").expect_err("an unknown workflow");
    assert!(error.message.starts_with("the workflow `no-such-workflow` cannot be run: "), "{}", error.message);
}
