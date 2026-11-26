//! Helpers shared by the tests of the `flm` program: what a run under `--json-messages`
//! wrote on standard error, read back as JSON.
#![allow(dead_code)]

use std::process::Output;

use serde_json::Value;

/// Every line of standard error of `output`, a run under `--json-messages`, each parsed
/// as JSON: the header first, then one record per line.
pub fn records(output: &Output) -> Vec<Value> {
    String::from_utf8_lossy(&output.stderr)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|error| panic!("{error}: {line:?}")))
        .collect()
}

/// The diagnostics of `output`, a run under `--json-messages`: the severity and the
/// condition identifier of each, in order, then the record of the counts (`null` when there
/// is none).
pub fn diagnostics(output: &Output) -> (Vec<(String, String)>, Value) {
    let records = records(output);
    let reported = records
        .iter()
        .filter(|record| record["kind"] == "diagnostic")
        .map(|record| (record["severity"].as_str().unwrap().to_owned(), record["id"].as_str().unwrap().to_owned()))
        .collect();
    let summary = records.iter().find(|record| record["kind"] == "summary").cloned().unwrap_or(Value::Null);
    (reported, summary)
}

/// The records of the diagnostics of `output` whose condition is `id`, in order.
pub fn diagnostics_of(output: &Output, id: &str) -> Vec<Value> {
    records(output).into_iter().filter(|record| record["kind"] == "diagnostic" && record["id"] == id).collect()
}

/// The file, the line and the column of the place of `record`.
pub fn place(record: &Value) -> (&str, u64, u64) {
    let place = &record["place"];
    (place["file"].as_str().unwrap_or_default(), place["line"].as_u64().unwrap_or_default(), place["column"].as_u64().unwrap_or_default())
}

/// The pair of a severity and an identifier, as [`diagnostics`] gives them.
pub fn reported(severity: &str, id: &str) -> (String, String) {
    (severity.to_owned(), id.to_owned())
}
