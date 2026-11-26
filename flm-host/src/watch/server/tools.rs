//! The files of the tools page, embedded in the program compressed with gzip, and served at
//! [`TOOLS`](super::TOOLS) and under [`ASSETS`](super::ASSETS).

use std::io::Read;

use flate2::read::GzDecoder;
use tiny_http::{Method, Request};

use super::http::{reply, reply_with};
use super::output::decoded;

// The table `FILES` that the crate's build script writes: each file under the folder
// `watch/client/tools/` of the crate's sources, by its path relative to that folder.
include!(concat!(env!("OUT_DIR"), "/watch_tools.rs"));

/// The path, among the files of the tools page, of the page itself.
const INDEX: &str = "index.html";

/// The media type and the compressed bytes of the file of the tools page at `path`, if
/// there is one.
fn file(path: &str) -> Option<(&'static str, &'static [u8])> {
    FILES.iter().find(|(name, _, _)| *name == path).map(|(_, media_type, bytes)| (*media_type, *bytes))
}

/// Answers a request for the tools page itself.
pub(super) fn serve_index(request: Request) {
    serve(request, INDEX);
}

/// Answers a request for the file of the tools page at the address path `path` under
/// [`ASSETS`](super::ASSETS) (the part after that prefix, percent-encoded).
pub(super) fn serve_asset(request: Request, path: &str) {
    match decoded(path) {
        Some(name) => serve(request, &name),
        None => reply(request, 404, None, None, b""),
    }
}

/// Answers a request for the file of the tools page at `path`: as it is embedded, with
/// `Content-Encoding: gzip`, when the request accepts gzip, and decompressed otherwise; `404`
/// for a path that is not one of the embedded files.
fn serve(request: Request, path: &str) {
    if !matches!(request.method(), Method::Get | Method::Head) {
        reply(request, 405, None, None, b"");
        return;
    }
    let Some((media_type, compressed)) = file(path) else {
        reply(request, 404, None, None, b"");
        return;
    };
    let vary = ("Vary", "Accept-Encoding");
    if accepts_gzip(&request) {
        reply_with(request, 200, Some(media_type), &[("Content-Encoding", "gzip"), vary], compressed);
        return;
    }
    let mut bytes = Vec::new();
    match GzDecoder::new(compressed).read_to_end(&mut bytes) {
        Ok(_) => reply_with(request, 200, Some(media_type), &[vary], &bytes),
        // The build script wrote the compressed bytes: they decompress unless the program is
        // damaged.
        Err(error) => {
            let reason = format!("Failed to decompress the file ‘{path}’ of the tools page: {error}");
            reply(request, 500, Some("text/plain"), None, reason.as_bytes());
        }
    }
}

/// Whether `request` accepts an answer compressed with gzip: its header `Accept-Encoding`
/// lists `gzip` (or `x-gzip`), or `*` without listing gzip, with a weight (`q`) other than
/// zero.
fn accepts_gzip(request: &Request) -> bool {
    let mut gzip = None;
    let mut any = None;
    let values = request.headers().iter().filter(|header| header.field.equiv("Accept-Encoding"));
    for coding in values.flat_map(|header| header.value.as_str().split(',')) {
        let mut parts = coding.split(';');
        let name = parts.next().unwrap_or_default().trim().to_ascii_lowercase();
        let weight = parts
            .filter_map(|parameter| parameter.trim().strip_prefix("q=").or_else(|| parameter.trim().strip_prefix("Q=")))
            .find_map(|value| value.trim().parse::<f32>().ok())
            .unwrap_or(1.0);
        match name.as_str() {
            "gzip" | "x-gzip" => gzip = Some(weight > 0.0),
            "*" => any = Some(weight > 0.0),
            _ => {}
        }
    }
    gzip.or(any).unwrap_or(false)
}

