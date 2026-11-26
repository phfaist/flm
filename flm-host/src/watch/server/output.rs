//! The output of the last build, served by path, and the waiting page served at `/` before
//! any build has given an output.

use std::sync::Arc;

use flm_core::stdlib::backends::html::escape::escape_text_into;
use tiny_http::{Method, Request};

use super::Shared;
use super::http::reply;
use crate::stdpkg::preview::{SCRIPT_TEXT, STYLE_TEXT};

/// Answers a request outside the server's own addresses, at the address path `path`: a file
/// of the output the server holds (with the header `Server-Timing` identifying the server
/// and the build: [`served_by`]), the waiting page at `/` before any output, or "not found".
pub(super) fn serve(shared: &Shared, request: Request, path: &str) {
    if !matches!(request.method(), Method::Get | Method::Head) {
        reply(request, 405, None, None, b"");
        return;
    }
    let output = shared.state().output.as_ref().map(|(build, output)| (*build, Arc::clone(output)));
    let Some(name) = file_name(path) else {
        reply(request, 404, None, None, b"");
        return;
    };
    match (&output, name.as_str()) {
        (Some((build, output)), name) => {
            let file = if name.is_empty() { output.entry() } else { output.files.get(name) };
            match file {
                Some(file) => {
                    let served_by = served_by(&shared.server, *build);
                    reply(request, 200, Some(&file.media_type), Some(&served_by), &file.bytes)
                }
                None => reply(request, 404, None, None, b""),
            }
        }
        (None, "") => reply(request, 200, Some("text/html"), None, waiting_page(&shared.document).as_bytes()),
        (None, _) => reply(request, 404, None, None, b""),
    }
}

/// The value of the header `Server-Timing` on an answer that carries a file of the output
/// of the build `build`: the metrics `flm-server` and `flm-build`, whose descriptions are
/// the server's identifier and the build's number. A page reads it from the answer that
/// carried it (the navigation entry's `serverTiming` in the browser), and so learns which
/// server and which build it shows without inferring them from the stream's first
/// `hello`; the bytes of the answer stay the build's.
pub(super) fn served_by(server: &str, build: u64) -> String {
    format!("flm-server;desc=\"{server}\", flm-build;desc=\"{build}\"")
}

/// The path, relative to the output, of the file that the address path `path` designates:
/// `path` without its leading `/`, its percent-encoded bytes decoded; `None` when they do
/// not make UTF-8 text. The empty path designates the entry file.
fn file_name(path: &str) -> Option<String> {
    decoded(path.strip_prefix('/').unwrap_or(path))
}

/// `path` with its percent-encoded bytes decoded; `None` when they do not make UTF-8 text.
pub(super) fn decoded(path: &str) -> Option<String> {
    percent_encoding::percent_decode_str(path).decode_utf8().ok().map(|name| name.into_owned())
}

/// The page the server serves at `/` before a build has given an output, for the document
/// `document`: it says, in a few words and with the document's name, that the preview waits
/// for the document, and carries the page script and its stylesheet, written into it (the
/// page is the server's own, not a build's). The script loads the document's page once a
/// build gives one.
fn waiting_page(document: &str) -> String {
    let mut escaped = String::new();
    escape_text_into(&mut escaped, document);
    let document = escaped;
    format!(
        "<!DOCTYPE html>\n<html data-flm-preview-waiting>\n<head>\n<meta charset=\"utf-8\">\n\
         <title>FLM preview: {document}</title>\n<style>\n{STYLE_TEXT}</style>\n</head>\n<body>\n\
         <div class=\"flm-preview-waiting\">\n<h1>FLM preview</h1>\n\
         <p>Waiting for <code>{document}</code></p>\n</div>\n\
         <script>\n{SCRIPT_TEXT}</script>\n</body>\n</html>\n"
    )
}
