//! The request to open a place of the document's files in an editor: `POST` of
//! [`OPEN`](super::OPEN).

use std::io::Read;

use serde_json::{Value, json};
use tiny_http::Request;

use super::http::reply;
use super::{Opening, Shared};
use crate::watch::editor::{OpenedPlace, Refusal, Spot, Started};
use crate::watch::snapshot::Processed;

/// The largest body, in bytes, of a request to [`OPEN`](super::OPEN); a longer body is
/// answered with `400`.
const OPEN_BODY_LIMIT: u64 = 64 * 1024;

/// What became of a request to open a place.
enum Answer {
    /// The editor started.
    Opened(Started, Opening),
    /// The place was located, but no editor could be opened: the place, `PATH:LINE:COLUMN`,
    /// and why.
    Unopened { place: String, reason: String },
    /// The place is not one to open: the status and the message of the answer.
    Refused(u16, String),
}

/// Answers a request to open a place in the editor: see the [module documentation](super).
pub(super) fn open(shared: &Shared, mut request: Request) {
    let mut body = Vec::new();
    let read = request.as_reader().take(OPEN_BODY_LIMIT + 1).read_to_end(&mut body);
    let place = read.ok().filter(|_| body.len() as u64 <= OPEN_BODY_LIMIT).and_then(|_| place_of(&body));
    let answer = match place {
        Some((file, spot)) => open_place(shared, &file, spot),
        None => Answer::Refused(
            400,
            format!(
                "Invalid place to open: the body must be a JSON object of at most {} KiB with ‘file’ and either \
                 ‘start’ or ‘line’ and ‘column’",
                OPEN_BODY_LIMIT / 1024
            ),
        ),
    };
    match answer {
        Answer::Opened(started, opened) => {
            reply(request, 204, None, None, b"");
            (shared.openings)(opened);
            if let Err(failure) = started.wait() {
                (shared.openings)(Opening::Failed(failure));
            }
        }
        Answer::Unopened { place, reason } => {
            let body = json!({ "place": place, "reason": reason }).to_string();
            reply(request, 200, Some("application/json"), None, body.as_bytes());
            (shared.openings)(Opening::Unopened { place, reason });
        }
        Answer::Refused(status, reason) => {
            reply(request, status, Some("text/plain"), None, reason.as_bytes());
            (shared.openings)(Opening::Refused { reason });
        }
    }
}

/// The place a request's body gives: `{"file": NAME, "start": OFFSET}` or `{"file": NAME,
/// "line": LINE, "column": COLUMN}`, each number a whole number (the line and the column
/// from 1), and no other field; `None` when the body is not that.
fn place_of(body: &[u8]) -> Option<(String, Spot)> {
    let Value::Object(fields) = serde_json::from_slice(body).ok()? else {
        return None;
    };
    let number = |key: &str| fields.get(key).and_then(Value::as_u64).and_then(|number| usize::try_from(number).ok());
    let file = fields.get("file")?.as_str()?.to_owned();
    let spot = match (fields.len(), number("start"), number("line"), number("column")) {
        (2, Some(start), None, None) => Spot::Offset(start),
        (3, None, Some(line), Some(column)) if line > 0 && column > 0 => Spot::LineColumn { line, column },
        _ => return None,
    };
    Some((file, spot))
}

/// Opens the place `spot` of the source named `file` in the editor of the viewer settings
/// ([`Editors::open_place`](crate::watch::editor::Editors::open_place)).
fn open_place(shared: &Shared, file: &str, spot: Spot) -> Answer {
    let (places, read) = {
        let state = shared.state();
        (state.snapshot.processed.clone(), state.read.clone())
    };
    let places = places.as_deref().map(Processed::places);
    let opened = {
        let viewer = shared.viewer();
        shared.editors.open_place(places, &read, file, spot, viewer.chosen(&shared.editors))
    };
    match opened {
        Ok(OpenedPlace { started, line, column }) => {
            let opened = Opening::Opened { editor: started.name().to_owned(), file: file.to_string(), line, column };
            Answer::Opened(started, opened)
        }
        Err(refusal) => {
            if let Some(place) = refusal.place() {
                return Answer::Unopened { place: place.to_string(), reason: refusal.to_string() };
            }
            let status = match refusal {
                Refusal::NotRead { .. } => 403,
                Refusal::PastEnd { .. } => 404,
                _ => 500,
            };
            Answer::Refused(status, refusal.to_string())
        }
    }
}
