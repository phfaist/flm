//! The viewer settings as the server holds them, and the request to change them: `POST` of
//! [`VIEWER`](super::VIEWER).

use std::io::Read;

use serde_json::{Map, Value, json};
use tiny_http::Request;

use super::Shared;
use super::http::reply;
use super::stream::frame;
use crate::watch::editor::{Chosen, Editors};
use crate::watch::viewer::{ViewerError, ViewerFile};

/// The largest body, in bytes, of a request to [`VIEWER`](super::VIEWER); a longer body is
/// answered with `400`.
const VIEWER_BODY_LIMIT: u64 = 4 * 1024;

/// The viewer settings as the server holds them: the file and what it gives, and whether
/// the program's command line still chooses the editor.
pub(super) struct Viewer {
    /// The file of the viewer settings, and the settings in effect.
    pub(super) file: ViewerFile,
    /// Whether the editor the program's command line asks for still counts over the file's:
    /// until a page chooses an editor.
    pub(super) preferred: bool,
}

impl Viewer {
    /// The viewer settings read from `file`, with the editor of the command line counting.
    pub(super) fn new(file: ViewerFile) -> Viewer {
        Viewer { file, preferred: true }
    }

    /// The editor a place opens in, among `editors` ([`Editors::choose`]): the one the
    /// command line asks for while it counts and while it is available, else the one of the
    /// viewer settings.
    pub(super) fn chosen<'e>(&'e self, editors: &'e Editors) -> Option<Chosen<'e>> {
        let settings = self.file.settings();
        let preferred = editors.preferred().filter(|_| self.preferred);
        let asked: Vec<&str> = preferred.into_iter().chain(settings.editor.as_deref()).collect();
        editors.choose(&asked, settings.editor_command.as_ref())
    }

    /// The names of the editors a place can be opened in: those of [`Editors::listed`] that
    /// are available.
    pub(super) fn names(&self, editors: &Editors) -> Vec<&'static str> {
        editors.listed().into_iter().filter(|listed| listed.available).map(|listed| listed.name).collect()
    }

    /// The fields that `hello` and the message `viewer` give of the editors and the viewer
    /// settings: `editors`, every editor of [`Editors::listed`], each
    /// `{"name", "label", "available"}`; and `viewer`, the viewer settings
    /// ([`Viewer::to_json`]), whose editor depends on the editor command of the file, which a
    /// change reads again.
    pub(super) fn fields(&self, editors: &Editors) -> Map<String, Value> {
        let listed: Vec<Value> = editors
            .listed()
            .into_iter()
            .map(|listed| json!({ "name": listed.name, "label": listed.label, "available": listed.available }))
            .collect();
        let mut fields = Map::new();
        fields.insert(String::from("editors"), Value::Array(listed));
        fields.insert(String::from("viewer"), self.to_json(editors));
        fields
    }

    /// The viewer settings as the pages read them, in the field `viewer` of `hello` and of the
    /// message `viewer`: `editor`, the name of the editor a place opens in (`null` when there is none); every
    /// other setting but the editor command
    /// ([`ViewerSettings::to_json`](crate::watch::viewer::ViewerSettings::to_json)); and
    /// `unreadable`, why the file cannot be read, on one line, or `null`.
    pub(super) fn to_json(&self, editors: &Editors) -> Value {
        let mut fields = Map::new();
        fields.insert(String::from("editor"), json!(self.chosen(editors).map(|chosen| chosen.name())));
        fields.extend(self.file.settings().to_json());
        fields.insert(String::from("unreadable"), json!(self.file.unreadable().map(ViewerError::to_string)));
        Value::Object(fields)
    }
}

/// Answers a request to change the viewer settings: see the [module
/// documentation](super), "The viewer settings".
pub(super) fn change(shared: &Shared, mut request: Request) {
    let mut body = Vec::new();
    let read = request.as_reader().take(VIEWER_BODY_LIMIT + 1).read_to_end(&mut body);
    let changes = read.ok().filter(|_| body.len() as u64 <= VIEWER_BODY_LIMIT).and_then(|_| match serde_json::from_slice(&body) {
        Ok(Value::Object(changes)) => Some(changes),
        _ => None,
    });
    let Some(changes) = changes else {
        let reason = format!("Invalid body: the viewer settings to change must be a JSON object of at most {VIEWER_BODY_LIMIT} bytes");
        reply(request, 400, Some("text/plain"), None, reason.as_bytes());
        return;
    };
    let mut viewer = shared.viewer();
    let names = viewer.names(&shared.editors);
    match viewer.file.change(&changes, &names) {
        Ok(()) => {
            if changes.contains_key("editor") {
                viewer.preferred = false;
            }
            // Sent before the viewer settings are let go, so that the pages are told every
            // change in its order and a stream that opens meanwhile misses none.
            let message = frame("viewer", &Value::Object(viewer.fields(&shared.editors)));
            shared.state().send(&message);
            drop(viewer);
            reply(request, 204, None, None, b"");
        }
        Err(error) => {
            let status = match error {
                ViewerError::Refused(_) => 400,
                ViewerError::Unreadable { .. } => 409,
                ViewerError::Unwritable { .. } => 500,
            };
            reply(request, status, Some("text/plain"), None, error.to_string().as_bytes());
        }
    }
}
