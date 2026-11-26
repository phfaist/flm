//! The requests of the help pages: `GET` of [`HELP_INDEX`](super::HELP_INDEX) and of
//! [`HELP_PAGE`](super::HELP_PAGE), answered by the program's help service
//! ([`Help`](crate::watch::services::Help)) for the last preparation that succeeded.

use std::sync::Arc;

use serde_json::json;
use tiny_http::Request;

use super::{Shared, environment_of};
use super::http::{QueryError, query_value, reply};
use crate::session::Prepared;
use crate::watch::services::{Help, HelpEntry, HelpError};

/// Answers a request for the index of the help pages: see the [module
/// documentation](super), "The help pages".
pub(super) fn index(shared: &Shared, request: Request) {
    let Some((help, prepared)) = service(shared) else {
        refuse(shared, request);
        return;
    };
    match help.index(&prepared) {
        Ok(pages) => {
            let answer = json!({
                "environment": environment_of(&prepared),
                "pages": pages.iter().map(HelpEntry::to_json).collect::<Vec<_>>(),
            });
            reply(request, 200, Some("application/json"), None, answer.to_string().as_bytes());
        }
        Err(error) => failed(request, &error),
    }
}

/// Answers a request for one help page, given by the query's parameter `path`: see the
/// [module documentation](super), "The help pages".
pub(super) fn page(shared: &Shared, request: Request) {
    let path = match query_value(request.url(), "path") {
        Ok(path) => path,
        Err(error) => {
            let reason = match error {
                QueryError::Missing => "Missing parameter ‘path’ in the query: the page is given as ‘?path=PATH’",
                QueryError::Duplicate => "Duplicate parameter ‘path’ in the query: a request reads one page",
                QueryError::NotText(_) => "Invalid parameter ‘path’ in the query: it is not percent-encoded UTF-8 text",
            };
            reply(request, 400, Some("text/plain"), None, reason.as_bytes());
            return;
        }
    };
    let Some((help, prepared)) = service(shared) else {
        refuse(shared, request);
        return;
    };
    let answer = match help.page(&prepared, &path) {
        Ok(page) => json!({ "path": path, "page": page.to_json(), "nearest": [] }),
        Err(HelpError::NoPage { nearest, .. }) => {
            let nearest: Vec<_> = nearest.iter().map(|(path, title)| json!({ "path": path, "title": title })).collect();
            json!({ "path": path, "page": null, "nearest": nearest })
        }
        Err(error) => {
            failed(request, &error);
            return;
        }
    };
    reply(request, 200, Some("application/json"), None, answer.to_string().as_bytes());
}

/// The help service and the last preparation that succeeded, when the program gives the
/// service and a preparation succeeded.
fn service(shared: &Shared) -> Option<(&dyn Help, Arc<Prepared>)> {
    let help = shared.services.help()?;
    let prepared = shared.state().prepared.clone()?;
    Some((help, prepared))
}

/// Answers a request of the help pages that cannot be answered: `501` when the program
/// gives no help pages, `404` when no preparation of the document has succeeded yet.
fn refuse(shared: &Shared, request: Request) {
    if shared.services.help().is_none() {
        reply(request, 501, Some("text/plain"), None, "Missing help pages in this preview".as_bytes());
    } else {
        let reason = "Missing help pages: the document has not been prepared yet";
        reply(request, 404, Some("text/plain"), None, reason.as_bytes());
    }
}

/// Answers `500` with the text of `error`.
fn failed(request: Request, error: &HelpError) {
    reply(request, 500, Some("text/plain"), None, error.to_string().as_bytes());
}
