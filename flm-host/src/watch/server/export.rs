//! The requests of the export: `GET` of [`EXPORT`](super::EXPORT), the name and the command
//! lines of an export; `POST` of [`EXPORT`](super::EXPORT), an export carried out by the
//! watch loop ([`Handle::export`](crate::watch::Handle::export)); and `POST` of
//! [`EXPORT_OPEN`](super::EXPORT_OPEN), the main file of the last export saved opened
//! through the program's services
//! ([`Services::open_file`](crate::watch::services::Services::open_file)).

use std::io::{Cursor, Read, Write};

use serde_json::{Value, json};
use tiny_http::Request;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

use super::Shared;
use super::http::{QueryError, query_value, reply, reply_with};
use crate::files::system::to_system;
use crate::output::NamedFile;
use crate::watch::export::{ExportAction, ExportError, ExportOrder, Exported};
use crate::watch::services::OpenFileError;

/// The largest body, in bytes, of a request to [`EXPORT`](super::EXPORT); a longer body is
/// answered with `400`.
const EXPORT_BODY_LIMIT: u64 = 4 * 1024;

/// Answers a request for the name and the command lines of the export of a build type,
/// given by the query's parameter `profile`: see the [module documentation](super),
/// "Exporting the document".
pub(super) fn describe(shared: &Shared, request: Request) {
    let profile = match query_value(request.url(), "profile") {
        Ok(profile) if !profile.is_empty() => profile,
        Ok(_) | Err(QueryError::Missing) => {
            let reason = "Missing parameter ‘profile’ in the query: the build type is given as ‘?profile=NAME’";
            reply(request, 400, Some("text/plain"), None, reason.as_bytes());
            return;
        }
        Err(QueryError::Duplicate) => {
            let reason = "Duplicate parameter ‘profile’ in the query: a request reads one build type";
            reply(request, 400, Some("text/plain"), None, reason.as_bytes());
            return;
        }
        Err(QueryError::NotText(_)) => {
            let reason = "Invalid parameter ‘profile’ in the query: it is not percent-encoded UTF-8 text";
            reply(request, 400, Some("text/plain"), None, reason.as_bytes());
            return;
        }
    };
    match shared.handle.export_name(profile.as_str()) {
        Ok(name) => {
            let command = |continuing| shared.services.export_command(&profile, &name, continuing);
            let answer = json!({
                "profile": profile,
                "name": name,
                "command": command(false),
                "command_continuing": command(true),
            });
            reply(request, 200, Some("application/json"), None, answer.to_string().as_bytes());
        }
        Err(error) => refuse(request, &error),
    }
}

/// Answers a request to export the document: the watch loop builds it, and the answer names
/// the files saved, or carries the file or the archive to download, or says why nothing was
/// written. See the [module documentation](super), "Exporting the document".
pub(super) fn export(shared: &Shared, mut request: Request) {
    let mut body = Vec::new();
    let read = request.as_reader().take(EXPORT_BODY_LIMIT + 1).read_to_end(&mut body);
    let Some(order) = read.ok().filter(|_| body.len() as u64 <= EXPORT_BODY_LIMIT).and_then(|_| order_of(&body)) else {
        let reason = "The body must be a JSON object of at most 4 KiB with a build type (‘profile’), an action \
                      (‘save’ or ‘download’) and ‘continue_with_errors’ (true or false)";
        reply(request, 400, Some("text/plain"), None, reason.as_bytes());
        return;
    };
    match shared.handle.export(order) {
        Ok(Exported::Saved { name, files, main }) => {
            shared.state().exported = Some(main);
            let answer = json!({ "name": name, "files": files });
            reply(request, 200, Some("application/json"), None, answer.to_string().as_bytes());
        }
        Ok(Exported::Download { name, stem, files }) => download(request, &name, &stem, &files),
        Err(error) => refuse(request, &error),
    }
}

/// Answers a request to open the main file of the last export saved, through the program's
/// services: see the [module documentation](super), "Exporting the document".
pub(super) fn open(shared: &Shared, request: Request) {
    let Some(main) = shared.state().exported.clone() else {
        let reason = "Missing export to open: no export has been saved";
        reply(request, 404, Some("text/plain"), None, reason.as_bytes());
        return;
    };
    let opened = to_system(&main)
        .map_err(|error| OpenFileError::Failed(format!("Failed to open ‘{main}’: {error}")))
        .and_then(|system| shared.services.open_file(&system));
    match opened {
        Ok(()) => reply(request, 204, None, None, b""),
        Err(OpenFileError::NotOffered) => {
            reply(request, 501, Some("text/plain"), None, OpenFileError::NotOffered.to_string().as_bytes());
        }
        Err(error) => reply(request, 500, Some("text/plain"), None, error.to_string().as_bytes()),
    }
}

/// The export a request's body orders, `{"profile": NAME, "action": "save" | "download",
/// "continue_with_errors": BOOL}`, the name a text that is not empty; `None` when the body
/// is not that.
fn order_of(body: &[u8]) -> Option<ExportOrder> {
    let value: Value = serde_json::from_slice(body).ok()?;
    let profile = value.get("profile")?.as_str().filter(|profile| !profile.is_empty())?;
    let action = match value.get("action")?.as_str()? {
        "save" => ExportAction::SaveAsSibling,
        "download" => ExportAction::Download,
        _ => return None,
    };
    let continue_with_errors = value.get("continue_with_errors")?.as_bool()?;
    Some(ExportOrder { profile: profile.to_owned(), action, continue_with_errors })
}

/// Answers a request of the export that wrote nothing, the reason on the first line and the
/// build's log, when there is one, after a blank line: `428` for a document with errors,
/// the one refusal that an export continuing with errors lifts; `409` for an export that
/// the document or the build type does not allow otherwise and for a failed build; `500`
/// for files that could not be written; `503` once the watch loop has ended.
fn refuse(request: Request, error: &ExportError) {
    let status = match error {
        ExportError::DocumentErrors { .. } => 428,
        ExportError::NotWritten { .. } => 500,
        ExportError::Ended => 503,
        _ => 409,
    };
    let mut text = error.to_string();
    if let Some(log) = error.log() {
        text.push_str("\n\n");
        text.push_str(log);
    }
    reply(request, status, Some("text/plain"), None, text.as_bytes());
}

/// Answers a download: the one file of an export as it is, else a zip archive of its files
/// named `STEM.zip`, each file at its path; with the header `Content-Disposition` that has
/// the browser save it under its name.
fn download(request: Request, name: &str, stem: &str, files: &[NamedFile]) {
    if let [file] = files {
        let saved = file.path.rsplit('/').next().unwrap_or(name);
        let disposition = disposition(saved);
        reply_with(request, 200, Some(&file.media_type), &[("Content-Disposition", &disposition)], &file.bytes);
        return;
    }
    match archive(files) {
        Ok(bytes) => {
            let disposition = disposition(&format!("{stem}.zip"));
            reply_with(request, 200, Some("application/zip"), &[("Content-Disposition", &disposition)], &bytes);
        }
        Err(error) => {
            let reason = format!("Failed to write the archive of ‘{name}’: {error}");
            reply(request, 500, Some("text/plain"), None, reason.as_bytes());
        }
    }
}

/// The zip archive of `files`, each at its path, stored without compression.
fn archive(files: &[NamedFile]) -> zip::result::ZipResult<Vec<u8>> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    for file in files {
        writer.start_file(file.path.as_str(), options)?;
        writer.write_all(&file.bytes)?;
    }
    Ok(writer.finish()?.into_inner())
}

/// The characters of a file name that the header `Content-Disposition` writes
/// percent-encoded: every one but the letters, the digits and `- . _ ~`.
const ENCODED: &percent_encoding::AsciiSet =
    &percent_encoding::NON_ALPHANUMERIC.remove(b'-').remove(b'.').remove(b'_').remove(b'~');

/// The value of the header `Content-Disposition` that has the browser save a file under the
/// name `name`: the name percent-encoded as UTF-8, which every browser reads.
fn disposition(name: &str) -> String {
    let encoded = percent_encoding::utf8_percent_encode(name, ENCODED);
    format!("attachment; filename*=UTF-8''{encoded}")
}
