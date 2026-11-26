//! The stream of messages: server-sent events, `hello` first on every connection, then each
//! message the server sends to every open stream.

use std::io::Write;
use std::sync::Arc;
use std::sync::mpsc::channel;

use serde_json::{Value, json};
use tiny_http::Request;

use super::Shared;

/// One server-sent event: its name, and its data on one line.
pub(super) fn frame(event: &str, data: &Value) -> String {
    format!("event: {event}\ndata: {data}\n\n")
}

/// Opens a stream of messages over `request`'s connection: the head of the answer, then
/// `hello`, then, while a build runs, the last `progress` message sent for it, then every
/// message sent to the clients, on a thread of the stream's own; once
/// the server stops, the end of the stream.
///
/// The answer is written by hand ([`super::http::reply`] says why): an answer of `tiny_http`
/// whose body is a stream keeps the messages in its buffers until the body ends. Its body
/// comes in chunks, one per message, so that the browser is told where it ends, and a page
/// whose stream ended connects again.
pub(super) fn stream(shared: &Arc<Shared>, request: Request) {
    let (send, messages) = channel::<String>();
    {
        // The viewer settings are held until the stream is among the clients, so that a
        // change made meanwhile reaches it ([`Shared::viewer`] gives the order of the locks).
        let viewer = shared.viewer();
        let mut state = shared.state();
        let output_build = state.output.as_ref().map(|(build, _)| *build);
        let mut hello = json!({
            "server": shared.server,
            "document": shared.document,
            "build": output_build,
            "building": state.building,
            "help": shared.services.help().is_some(),
        });
        if let Value::Object(fields) = &mut hello {
            fields.extend(viewer.fields(&shared.editors));
            fields.extend(state.snapshot.fields());
            fields.extend(state.served());
        }
        // The browser waits a second, not its default three, before it connects again.
        let _ = send.send(format!("retry: 1000\n{}", frame("hello", &hello)));
        // A stream opened while a build runs gets the last progress text sent for it: the
        // server sends the text again only once it changes.
        if let (true, Some(text)) = (state.building, &state.told.text) {
            let _ = send.send(frame("progress", &json!({ "text": text })));
        }
        state.clients.push(send);
    }
    let mut writer = request.into_writer();
    let head = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\n\
                 Connection: close\r\nTransfer-Encoding: chunked\r\n\r\n";
    if writer.write_all(head).and_then(|()| writer.flush()).is_err() {
        return;
    }
    for message in messages {
        let chunk = format!("{:x}\r\n{message}\r\n", message.len());
        if writer.write_all(chunk.as_bytes()).and_then(|()| writer.flush()).is_err() {
            return;
        }
    }
    // The server stopped: the last chunk, which ends the body.
    let _ = writer.write_all(b"0\r\n\r\n").and_then(|()| writer.flush());
}
