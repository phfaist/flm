//! The answers of the server, written by hand: the status line, the headers every answer
//! carries, and the body; and the reading of a parameter of a request's query.

use std::io::Write;

use tiny_http::{Method, Request};

/// The value of the header `Content-Type` for the media type `media_type`, with the
/// character set UTF-8 added to a text type that has none: every text a build writes is
/// UTF-8. `None` for a media type that cannot be written in a header.
fn content_type(media_type: &str) -> Option<String> {
    if !media_type.chars().all(|character| character.is_ascii_graphic() || character == ' ') {
        return None;
    }
    if media_type.starts_with("text/") && !media_type.contains("charset") {
        Some(format!("{media_type}; charset=utf-8"))
    } else {
        Some(media_type.to_owned())
    }
}

/// The words that follow the status `status` in an answer's first line.
fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        204 => "No Content",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        409 => "Conflict",
        428 => "Precondition Required",
        500 => "Internal Server Error",
        501 => "Not Implemented",
        503 => "Service Unavailable",
        _ => "",
    }
}

/// Answers `request` with the status `status` and the body `body` of the media type
/// `media_type` (no body for `HEAD`), not to be kept by the browser, and closes the
/// connection after it; `served_by`, when given, is the value of the header
/// `Server-Timing` ([`super::output::served_by`]).
///
/// The answer is written by hand, as the stream's head is, to say `Connection: close`,
/// which `tiny_http` does not let an answer say, so that a browser never sends a request on
/// a connection a dropped server keeps without answering.
pub(super) fn reply(request: Request, status: u16, media_type: Option<&str>, served_by: Option<&str>, body: &[u8]) {
    reply_with(request, status, media_type, served_by.map(|value| ("Server-Timing", value)).as_slice(), body);
}

/// Answers `request` as [`reply`] does, with the headers `headers` (each a name and a
/// value) added after `Content-Type`.
pub(super) fn reply_with(request: Request, status: u16, media_type: Option<&str>, headers: &[(&str, &str)], body: &[u8]) {
    let head_only = *request.method() == Method::Head;
    let mut head = format!("HTTP/1.1 {status} {}\r\nCache-Control: no-store\r\nConnection: close\r\n", reason(status));
    if status != 204 {
        head.push_str(&format!("Content-Length: {}\r\n", body.len()));
    }
    if let Some(value) = media_type.and_then(content_type) {
        head.push_str(&format!("Content-Type: {value}\r\n"));
    }
    for (name, value) in headers {
        head.push_str(&format!("{name}: {value}\r\n"));
    }
    head.push_str("\r\n");
    let mut writer = request.into_writer();
    let _ = writer
        .write_all(head.as_bytes())
        .and_then(|()| if head_only { Ok(()) } else { writer.write_all(body) })
        .and_then(|()| writer.flush());
}

/// Why the query of a request does not give the value of a parameter ([`query_value`]).
#[derive(Debug, PartialEq)]
pub(super) enum QueryError {
    /// The query does not give the parameter.
    Missing,
    /// The query gives the parameter more than once.
    Duplicate,
    /// The parameter's value, as written (each `+` read as a space), does not
    /// percent-decode to UTF-8 text.
    NotText(String),
}

/// The value of the parameter `name` in the query of `url` (after `?`, before `#`), each `+`
/// read as a space and percent-decoded as UTF-8 text.
pub(super) fn query_value(url: &str, name: &str) -> Result<String, QueryError> {
    let query = url.split_once('?').map(|(_, query)| query).unwrap_or_default();
    let query = query.split('#').next().unwrap_or_default();
    let prefix = format!("{name}=");
    let mut values = query.split('&').filter_map(|pair| pair.strip_prefix(prefix.as_str()));
    let value = values.next().ok_or(QueryError::Missing)?;
    if values.next().is_some() {
        return Err(QueryError::Duplicate);
    }
    let value = value.replace('+', " ");
    match percent_encoding::percent_decode_str(&value).decode_utf8() {
        Ok(decoded) => Ok(decoded.into_owned()),
        Err(_) => Err(QueryError::NotText(value)),
    }
}
