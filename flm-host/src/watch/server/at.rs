//! The request of the constructs at a place: `GET` of [`AT`](super::AT), answered from the
//! document snapshot of the output served
//! ([`Processed::at`](crate::watch::snapshot::Processed::at)), each construct with its help
//! page as the program's services give it.

use serde_json::{Value, json};
use tiny_http::Request;

use super::Shared;
use super::http::{QueryError, query_value, reply};
use crate::session::Prepared;
use crate::watch::snapshot::Construct;

/// Answers a request for the constructs at a place, given by the query's parameters `file`
/// and `start`: see the [module documentation](super), "The constructs at a place".
pub(super) fn at(shared: &Shared, request: Request) {
    let place = parameter(request.url(), "file").and_then(|file| {
        let start = parameter(request.url(), "start")?;
        start.parse::<usize>().map(|start| (file, start)).map_err(|_| {
            format!("Invalid parameter ‘start’ in the query: ‘{start}’ is not a byte offset, a whole number from 0")
        })
    });
    let (file, start) = match place {
        Ok(place) => place,
        Err(reason) => {
            reply(request, 400, Some("text/plain"), None, reason.as_bytes());
            return;
        }
    };
    let (processed, prepared) = {
        let state = shared.state();
        (state.snapshot.processed.clone(), state.prepared.clone())
    };
    let Some(processed) = processed else {
        let reason = "No output is served yet";
        reply(request, 404, Some("text/plain"), None, reason.as_bytes());
        return;
    };
    let at = match processed.at(&file, start) {
        Ok(at) => at,
        Err(error) => {
            reply(request, 404, Some("text/plain"), None, error.to_string().as_bytes());
            return;
        }
    };
    let answer = json!({
        "file": file,
        "start": start,
        "line": at.line,
        "column": at.column,
        "constructs": at.constructs.iter().map(|construct| with_help(shared, prepared.as_deref(), construct)).collect::<Vec<_>>(),
    });
    reply(request, 200, Some("application/json"), None, answer.to_string().as_bytes());
}

/// `construct` as JSON ([`Construct::to_json`]), with the field `help`: the path of its
/// help page in the environment of `prepared`, the last preparation that succeeded, as the
/// program's help service gives it; `null` when it gives none.
fn with_help(shared: &Shared, prepared: Option<&Prepared>, construct: &Construct) -> Value {
    let help = prepared.and_then(|prepared| shared.services.help()?.construct_page(prepared, construct));
    let mut value = construct.to_json();
    if let Value::Object(fields) = &mut value {
        fields.insert(String::from("help"), json!(help));
    }
    value
}

/// The value of the parameter `name` in the query of `url` ([`query_value`]); `Err` gives
/// why the query does not give one, as the answer's text.
fn parameter(url: &str, name: &str) -> Result<String, String> {
    query_value(url, name).map_err(|error| match error {
        QueryError::Missing => format!("Missing parameter ‘{name}’ in the query: the place is given as ‘?file=NAME&start=OFFSET’"),
        QueryError::Duplicate => format!("Duplicate parameter ‘{name}’ in the query: a request reads the constructs at one place"),
        QueryError::NotText(_) => format!("Invalid parameter ‘{name}’ in the query: it is not percent-encoded UTF-8 text"),
    })
}
