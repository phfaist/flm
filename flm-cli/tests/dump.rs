//! The smoke test of the `flm dump` command: the binary parses a file, runs the
//! `text` workflow over `flm_core::stdcore::package` and prints the requested
//! view — the parsed tree, the frozen tree, the lowered tree, a table, an
//! output — as JSON, the wire view as one line; the document's diagnostics go
//! to standard error; and a usage error or an unknown name fails with a
//! non-zero exit status. Under `--workflow latex-recomposed` the same views
//! show the LaTeX backend: its lowered tree of construct elements, its
//! safe-label table, its per-tree record, and its output of two artifacts.
//! Under `--workflow html` they show the HTML backend: its lowered tree of
//! shared presentation elements, its per-tree record of HTML and required
//! definitions, the definitions table — the manifest a bundler works from —
//! and its output of one artifact, the page.

use std::path::PathBuf;
use std::process::{Command, Output};

/// A small document: a labeled heading and a paragraph referencing it.
const DOCUMENT: &str =
    "\\section{Intro}\\label{sec:intro}\n\nHello \\emph{world}, see \\ref{sec:intro}.\n";

/// A scratch file holding the document, removed on drop.
struct Scratch {
    path: PathBuf,
}

impl Scratch {
    fn new(name: &str) -> Scratch {
        Scratch::holding(name, DOCUMENT)
    }

    /// A scratch file holding `text` instead of [`DOCUMENT`].
    fn holding(name: &str, text: &str) -> Scratch {
        let mut path = std::env::temp_dir();
        path.push(format!("flm-main-dump-{}-{name}.flm", std::process::id()));
        std::fs::write(&path, text).expect("the scratch file is written");
        Scratch { path }
    }

    fn arg(&self) -> &str {
        self.path.to_str().expect("a UTF-8 path")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Runs `flm dump` with `args`.
fn flm_dump(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_flm"))
        .arg("dump")
        .args(args)
        .output()
        .expect("the binary runs")
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("UTF-8 output")
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("UTF-8 output")
}

/// The output parsed as JSON; a failure shows the text.
fn json(output: &Output) -> serde_json::Value {
    let text = stdout(output);
    serde_json::from_str(&text).unwrap_or_else(|error| panic!("not JSON ({error}):\n{text}"))
}

/// The bytes of the one entry artifact of an `--output` view, as text.
///
/// An artifact's bytes render as the reserved one-entry object `{"$bytes":
/// "<base64>"}`, so they are read back through the library's own reader —
/// `flm_core::dump::from_json`, which turns that object into a byte string
/// again — rather than through a second base64 decoder written here.
fn page_bytes(output: &Output) -> String {
    use flm_core::techy::serialize::SerialValue;

    fn key<'v>(value: &'v SerialValue, name: &str) -> &'v SerialValue {
        match value {
            SerialValue::Map(entries) => entries
                .iter()
                .find(|(k, _)| k == name)
                .map(|(_, v)| v)
                .unwrap_or_else(|| panic!("no key `{name}` in {value:?}")),
            other => panic!("not a map: {other:?}"),
        }
    }

    let text = stdout(output);
    let value = flm_core::dump::from_json(&text)
        .unwrap_or_else(|error| panic!("not JSON ({error}):\n{text}"));
    let set = key(&value, "flm.output.html.document");
    let SerialValue::List(artifacts) = key(set, "artifacts") else { panic!("no artifact list") };
    let entries: Vec<&SerialValue> = artifacts
        .iter()
        .filter(|a| matches!(key(a, "entry"), SerialValue::Bool(true)))
        .collect();
    assert_eq!(entries.len(), 1, "exactly one entry artifact");
    match key(entries[0], "data") {
        SerialValue::Bytes(bytes) => {
            String::from_utf8(bytes.clone()).expect("the page is UTF-8")
        }
        other => panic!("the artifact's data is not a byte string: {other:?}"),
    }
}

#[test]
fn the_parsed_tree_is_printed_as_its_projection() {
    let file = Scratch::new("parsed");
    let output = flm_dump(&[file.arg()]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value = json(&output);
    let text = value.to_string();
    assert!(text.contains("\"flm.heading\""), "{text}");
    assert!(text.contains("\"flm.ref\""), "{text}");
    assert!(!text.contains("\"flm.paragraph\""), "a parsed tree holds no paragraph: {text}");
}

#[test]
fn the_frozen_tree_holds_the_structured_paragraphs() {
    let file = Scratch::new("frozen");
    let output = flm_dump(&[file.arg(), "--at", "frozen"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = json(&output).to_string();
    assert!(text.contains("\"flm.paragraph\""), "{text}");
}

#[test]
fn the_lowered_tree_holds_the_presentation_elements() {
    let file = Scratch::new("lowered");
    let output = flm_dump(&[file.arg(), "--at", "lowered:text"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = json(&output).to_string();
    assert!(text.contains("\"flm.pres.block-heading\""), "{text}");
    assert!(text.contains("\"flm.pres.link\""), "{text}");
    assert!(text.contains("\"#sec-intro\""), "{text}");
}

#[test]
fn the_wire_view_is_one_json_line() {
    let file = Scratch::new("wire");
    let output = flm_dump(&[file.arg(), "--view", "wire"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = stdout(&output);
    assert_eq!(text.lines().count(), 1, "{text}");
    serde_json::from_str::<serde_json::Value>(text.trim()).expect("a JSON line");
}

#[test]
fn a_table_is_printed_under_its_name() {
    let file = Scratch::new("table");
    let output = flm_dump(&[file.arg(), "--table", "flm.labels"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value = json(&output);
    let labels = value.get("flm.labels").expect("the table under its name");
    assert!(labels.to_string().contains("sec:intro"), "{labels}");
}

#[test]
fn the_counter_table_names_each_counter_in_full() {
    // Equations are numbered by default; the counter is the mathematics
    // feature's, named under its prefix. The items of an `enumerate` count in
    // the lists feature's counter, one scoped instance per list: the nested
    // item's record names its own list as its instance and carries the
    // enclosing item's value, in the enclosing list's instance, as its
    // ancestor.
    let file = Scratch::holding(
        "counters",
        "\\begin{equation}\n  x \\label{eq:x}\n\\end{equation}\n\n\
         \\begin{enumerate}\n\\item a\n\\begin{enumerate}\n\\item b\n\\end{enumerate}\n\
         \\end{enumerate}\n\nSee \\ref{eq:x}.\n",
    );
    let output = flm_dump(&[file.arg(), "--table", "flm.counters"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value = json(&output);
    let counters = value.get("flm.counters").expect("the table under its name");
    let records: Vec<&serde_json::Value> =
        counters.as_object().expect("a map from point to record").values().collect();
    let mut names: Vec<&str> = records
        .iter()
        .filter_map(|record| record.get("counter").and_then(|counter| counter.as_str()))
        .collect();
    names.sort();
    assert_eq!(names, ["flm.lists.enumerate", "flm.lists.enumerate", "flm.math.equation"], "{counters}");
    let items: Vec<&&serde_json::Value> = records
        .iter()
        .filter(|record| record.get("counter").and_then(|c| c.as_str()) == Some("flm.lists.enumerate"))
        .collect();
    assert!(items.iter().all(|record| record.get("instance").is_some()), "{counters}");
    let nested = items
        .iter()
        .find(|record| record.get("within").is_some_and(|a| !a.as_array().is_some_and(Vec::is_empty)))
        .unwrap_or_else(|| panic!("the nested item's record: {counters}"));
    let ancestor = &nested.get("within").and_then(|a| a.as_array()).expect("a list")[0];
    assert_eq!(ancestor.get("counter").and_then(|c| c.as_str()), Some("flm.lists.enumerate"));
    assert!(ancestor.get("instance").is_some(), "{counters}");
    assert_ne!(ancestor.get("instance"), nested.get("instance"), "{counters}");
}

#[test]
fn the_table_of_xnotes_records_each_footnote_of_the_standard_package() {
    // The standard package lists the xnotes feature: a document's footnotes
    // are collected by kind, each with its point and its mark's point.
    let file = Scratch::holding(
        "xnotes",
        "One.\\footnote{a} Two.\\footnote[\\label{fn:b}]{b} See \\ref{fn:b}.\n",
    );
    let output = flm_dump(&[file.arg(), "--table", "flm.xnotes"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value = json(&output);
    let xnotes = value.get("flm.xnotes").expect("the table under its name");
    let footnotes = xnotes
        .get("footnote")
        .and_then(|records| records.as_array())
        .unwrap_or_else(|| panic!("the records of the kind `footnote`: {xnotes}"));
    assert_eq!(footnotes.len(), 2, "{xnotes}");
    for record in footnotes {
        assert!(record.get("point").is_some() && record.get("mark").is_some(), "{xnotes}");
    }
}

#[test]
fn a_definition_s_point_is_printed_with_its_labels() {
    // A defined term's definition is one reference point: the label its term
    // computes, one per alias, then the labels written for it.
    let file = Scratch::holding(
        "defterm",
        "\\begin{defterm}{Markov  chain}\\label{topic:mc}\\aliasterm{Markov chains}\nA process.\n\\end{defterm}\n",
    );
    let output = flm_dump(&[file.arg(), "--table", "flm.refpoints"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value = json(&output);
    let points = value.get("flm.refpoints").and_then(|table| table.as_object()).expect("the table");
    let point = points.values().next().expect("one point");
    assert_eq!(point.get("kind").and_then(|kind| kind.as_str()), Some("defterm"), "{point}");
    assert_eq!(
        point.get("labels"),
        Some(&serde_json::json!(["defterm:Markov chain", "defterm:Markov chains", "topic:mc"])),
        "{point}"
    );
    assert_eq!(point.get("anchor").and_then(|anchor| anchor.as_str()), Some("from-first-label"));
}

#[test]
fn an_output_is_printed_as_its_artifact_set() {
    let file = Scratch::new("output");
    let output = flm_dump(&[file.arg(), "--output", "text", "--width", "20"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value = json(&output);
    let set = value.get("flm.output.text.document").expect("the output's table");
    let artifacts = set.get("artifacts").and_then(|a| a.as_array()).expect("the artifacts");
    assert_eq!(artifacts.len(), 1);
    assert_eq!(artifacts[0].get("id").and_then(|id| id.as_str()), Some("text:document"));
    assert_eq!(artifacts[0].get("media_type").and_then(|m| m.as_str()), Some("text/plain"));
}

#[test]
fn a_documents_diagnostics_go_to_standard_error() {
    let file = Scratch::new("diagnostics");
    let output = flm_dump(&[file.arg(), "--at", "frozen", "--workflow", "check"]);
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stderr(&output).is_empty(), "a clean document: {}", stderr(&output));
}

#[test]
fn an_unknown_table_fails_naming_the_known_ones() {
    let file = Scratch::new("unknown");
    let output = flm_dump(&[file.arg(), "--table", "nothing.here"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("flm.labels"), "{}", stderr(&output));
}

#[test]
fn a_workflow_beside_the_parsed_tree_is_a_usage_error() {
    let file = Scratch::new("parsed-workflow");
    let output = flm_dump(&[file.arg(), "--workflow", "nope"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("--at parsed"), "{}", stderr(&output));
    assert!(stdout(&output).is_empty());
}

#[test]
fn an_unknown_backend_fails_naming_the_active_ones() {
    let file = Scratch::new("unknown-backend");
    let output = flm_dump(&[file.arg(), "--at", "lowered:nosuch"]);
    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(message.contains("`nosuch`"), "{message}");
    assert!(message.contains("the backends that the workflow `text` uses: `text`"), "{message}");
}

#[test]
fn a_missing_file_argument_is_a_usage_error() {
    let output = flm_dump(&[]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("usage:"), "{}", stderr(&output));
}

// --- the `latex-recomposed` workflow -------------------------------------------------------

#[test]
fn the_latex_output_is_printed_as_its_two_artifacts() {
    let file = Scratch::new("latex-output");
    let output =
        flm_dump(&[file.arg(), "--workflow", "latex-recomposed", "--output", "latex-recomposed"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value = json(&output);
    let set = value.get("flm.output.latex-recomposed.document").expect("the output's table");
    let artifacts = set.get("artifacts").and_then(|a| a.as_array()).expect("the artifacts");
    // The document references a heading, so it needs `zref-clever` and the
    // hooks, and the style file is written beside the recomposed LaTeX.
    let ids: Vec<&str> = artifacts.iter().filter_map(|a| a.get("id")?.as_str()).collect();
    // The style file's identity is `latex-recomposed:STEM.HASH`, its stem and the
    // hash of its content.
    assert_eq!(ids.len(), 2, "{ids:?}");
    assert_eq!(ids[0], "latex-recomposed:document");
    assert!(ids[1].starts_with("latex-recomposed:flmrecomposed."), "{ids:?}");
    assert_eq!(artifacts[0].get("entry").and_then(|e| e.as_bool()), Some(true));
    assert_eq!(artifacts[1].get("entry").and_then(|e| e.as_bool()), Some(false));
    for artifact in artifacts {
        assert_eq!(
            artifact.get("media_type").and_then(|m| m.as_str()),
            Some("application/x-latex")
        );
    }
    assert!(stderr(&output).is_empty(), "a clean document: {}", stderr(&output));
}

#[test]
fn the_latex_output_of_a_document_needing_no_style_file_is_the_recomposed_latex_alone() {
    // Plain text and `\emph` need no package and no definition piece, so no
    // style file is written and the set holds the recomposed LaTeX only.
    let file = Scratch::holding("latex-output-plain", "Plain text with \\emph{emphasis}.\n");
    let output =
        flm_dump(&[file.arg(), "--workflow", "latex-recomposed", "--output", "latex-recomposed"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value = json(&output);
    let set = value.get("flm.output.latex-recomposed.document").expect("the output's table");
    let artifacts = set.get("artifacts").and_then(|a| a.as_array()).expect("the artifacts");
    let ids: Vec<&str> = artifacts.iter().filter_map(|a| a.get("id")?.as_str()).collect();
    assert_eq!(ids, ["latex-recomposed:document"]);
    assert_eq!(artifacts[0].get("entry").and_then(|e| e.as_bool()), Some(true));
    assert!(stderr(&output).is_empty(), "a clean document: {}", stderr(&output));
}

#[test]
fn the_lowered_tree_of_the_latex_backend_holds_the_construct_elements() {
    let file = Scratch::new("latex-lowered");
    let output = flm_dump(&[
        file.arg(),
        "--workflow",
        "latex-recomposed",
        "--at",
        "lowered:latex-recomposed",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = json(&output).to_string();
    assert!(text.contains("\"flm.pres.latex.macro\""), "{text}");
    assert!(text.contains("\"flmHeading\""), "{text}");
    assert!(text.contains("\"flmRef\""), "{text}");
    assert!(!text.contains("\"flm.pres.link\""), "no shared-vocabulary link here: {text}");
}

#[test]
fn the_safe_labels_table_maps_the_label_and_its_key_both_ways() {
    let file = Scratch::new("safe-labels");
    let output =
        flm_dump(&[file.arg(), "--workflow", "latex-recomposed", "--table", "flm.labels.safe"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value = json(&output);
    let domain = value
        .get("flm.labels.safe")
        .and_then(|t| t.get("ref"))
        .expect("the table under its name, the `ref` domain");
    assert_eq!(
        domain.get("labels").and_then(|l| l.get("sec:intro")).and_then(|n| n.as_str()),
        Some("ref1")
    );
    assert_eq!(
        domain.get("names").and_then(|n| n.get("ref1")).and_then(|l| l.as_str()),
        Some("sec:intro")
    );
}

#[test]
fn the_per_tree_latex_record_shows_what_the_tree_declares_needs_and_requires() {
    let file = Scratch::new("latex-fragments");
    let output = flm_dump(&[
        file.arg(),
        "--workflow",
        "latex-recomposed",
        "--table",
        "flm.output.latex-recomposed",
    ]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value = json(&output);
    let trees = value
        .get("flm.output.latex-recomposed")
        .and_then(|t| t.as_array())
        .expect("one entry per tree");
    assert_eq!(trees.len(), 1);
    let record = &trees[0];
    assert!(record.get("text").and_then(|t| t.as_str()).is_some_and(|t| t.contains("\\flmRef{ref1}")));
    // Plain ASCII: nothing declared, no preamble chunk needed.
    assert_eq!(record.get("declared").and_then(|d| d.as_object()).map(|d| d.len()), Some(0));
    assert_eq!(record.get("chunks").and_then(|c| c.as_array()).map(|c| c.len()), Some(0));
    let requires = record.get("requires").and_then(|r| r.as_array()).expect("the requirements");
    assert!(requires.iter().any(|r| r.as_str() == Some("flm.latex.def.refs")), "{requires:?}");
}

#[test]
fn an_output_the_chosen_workflow_does_not_write_names_that_workflow() {
    // `--output latex-recomposed` under the default `text` workflow: the
    // registry declares the table, but the text plan never writes it.
    let file = Scratch::new("latex-under-text");
    let output = flm_dump(&[file.arg(), "--output", "latex-recomposed"]);
    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(message.contains("the workflow `text`"), "{message}");
    assert!(message.contains("--workflow"), "{message}");
    assert!(stdout(&output).is_empty());
}

// --- the `html` workflow ------------------------------------------------------------------

#[test]
fn the_html_output_is_the_page_as_one_artifact() {
    let file = Scratch::new("html-output");
    let output = flm_dump(&[file.arg(), "--workflow", "html", "--output", "html"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value = json(&output);
    let set = value.get("flm.output.html.document").expect("the output's table");
    let artifacts = set.get("artifacts").and_then(|a| a.as_array()).expect("the artifacts");
    let ids: Vec<&str> = artifacts.iter().filter_map(|a| a.get("id")?.as_str()).collect();
    assert_eq!(ids, ["html:document"]);
    assert_eq!(artifacts[0].get("entry").and_then(|e| e.as_bool()), Some(true));
    assert_eq!(artifacts[0].get("media_type").and_then(|m| m.as_str()), Some("text/html"));
    // The bytes are a whole page, rendered as base64 the way every artifact's
    // bytes are; [`page_bytes`] reads them back.
    let page = page_bytes(&output);
    assert!(page.starts_with("<!DOCTYPE html>"), "{page}");
    assert!(page.contains("<main class=\"flm-content\" id=\"flm-content\">"), "{page}");
    assert!(page.contains("<h1 class=\"flm-pres-block--heading\" id=\"sec-intro\""), "{page}");
    // No render option is set from the command line, so the stylesheets the
    // document needs are written into the page itself and the title is the
    // backend's default.
    assert!(page.contains("<title>FLM document</title>"), "{page}");
    assert!(page.contains("/* flm.html.style.core */"), "{page}");
    assert!(stderr(&output).is_empty(), "a clean document: {}", stderr(&output));
}

#[test]
fn the_lowered_tree_of_the_html_backend_holds_the_presentation_elements() {
    let file = Scratch::new("html-lowered");
    let output = flm_dump(&[file.arg(), "--workflow", "html", "--at", "lowered:html"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let text = json(&output).to_string();
    // The shared presentation vocabulary, not a backend's own constructs: the
    // HTML backend claims no element of its own for a heading or a link.
    assert!(text.contains("\"flm.pres.block-heading\""), "{text}");
    assert!(text.contains("\"flm.pres.link\""), "{text}");
    assert!(!text.contains("\"flm.pres.latex.macro\""), "no LaTeX constructs here: {text}");
}

#[test]
fn the_per_tree_html_record_holds_the_html_and_what_the_tree_requires() {
    let file = Scratch::new("html-fragments");
    let output = flm_dump(&[file.arg(), "--workflow", "html", "--table", "flm.output.html"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value = json(&output);
    let trees = value.get("flm.output.html").and_then(|t| t.as_array()).expect("one per tree");
    assert_eq!(trees.len(), 1);
    let record = &trees[0];
    let html = record
        .get("segments")
        .and_then(|s| s.as_array())
        .and_then(|s| s.first()?.get("html")?.as_str())
        .expect("the tree's HTML");
    assert!(html.contains("<p class=\"flm-pres-par\">"), "{html}");
    // What the tree needs beside its content, by identifier: the emitter
    // required a stylesheet for each element it wrote. This document holds no
    // formula, so the formula renderer is not among them.
    let requires: Vec<&str> =
        record.get("requires").and_then(|r| r.as_array()).expect("the requirements")
            .iter().filter_map(|r| r.as_str()).collect();
    assert!(requires.contains(&"flm.html.style.core"), "{requires:?}");
    assert!(requires.contains(&"flm.html.style.headings"), "{requires:?}");
    assert!(!requires.contains(&"flm.html.mathjax"), "{requires:?}");
}

#[test]
fn the_definitions_table_is_the_manifest_of_what_the_page_needs() {
    // The document holds a formula, so the formula renderer is required beside
    // the stylesheets: this is the manifest an external bundler or a site
    // generator works from.
    let file =
        Scratch::holding("html-definitions", "A formula \\(x^2\\).\n\nA second paragraph.\n");
    let output =
        flm_dump(&[file.arg(), "--workflow", "html", "--table", "flm.html.definitions"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let value = json(&output);
    let rows = value.get("flm.html.definitions").and_then(|t| t.as_array()).expect("the rows");
    let ids: Vec<&str> = rows.iter().filter_map(|r| r.get("id")?.as_str()).collect();
    assert_eq!(ids, ["flm.html.style.core", "flm.html.style.math-passthrough", "flm.html.style.page",
                     "flm.html.mathjax"]);
    let mathjax = rows.last().expect("the last row");
    assert_eq!(mathjax.get("kind").and_then(|k| k.as_str()), Some("script"));
    assert_eq!(mathjax.get("owner").and_then(|o| o.as_str()), Some("html"));
    assert_eq!(mathjax.get("provided_by_host").and_then(|p| p.as_bool()), Some(false));
    // Two parts in order: the configuration as text, then the program's
    // address. Nothing here fetches the address.
    let parts = mathjax.get("parts").and_then(|p| p.as_array()).expect("the parts");
    assert_eq!(parts.len(), 2);
    assert!(parts[0].get("inline").and_then(|i| i.as_str()).is_some_and(|t| t.contains("MathJax")));
    assert!(parts[1].get("url").and_then(|u| u.as_str()).is_some_and(|u| u.starts_with("https://")));
}
