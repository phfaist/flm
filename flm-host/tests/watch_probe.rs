//! The probe of the crate the preview serves with: a stream of events held open through
//! `tiny_http` delivers each event to the client as soon as it is written, before the
//! stream ends.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

/// The upper bound of every wait of this test; the test waits for what it expects and
/// fails only past it.
const BOUND: Duration = Duration::from_secs(20);

#[test]
fn an_event_written_to_an_open_stream_reaches_the_client_at_once() {
    let server = tiny_http::Server::http("127.0.0.1:0").expect("the server listens");
    let address = server.server_addr().to_ip().expect("an IP address");
    let (send, events) = mpsc::channel::<String>();
    let serving = thread::spawn(move || {
        let request = server.recv().expect("a request");
        let mut writer = request.into_writer();
        writer
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\n\r\n")
            .expect("the head is written");
        writer.flush().expect("the head is sent");
        for event in events {
            writer.write_all(format!("data: {event}\n\n").as_bytes()).expect("the event is written");
            writer.flush().expect("the event is sent");
        }
    });

    let mut client = TcpStream::connect(address).expect("the client connects");
    client.set_read_timeout(Some(BOUND)).expect("a read bound");
    client.write_all(b"GET /events HTTP/1.1\r\nHost: localhost\r\nAccept: text/event-stream\r\n\r\n").expect("sent");
    let mut lines = BufReader::new(client);
    let mut line = String::new();
    lines.read_line(&mut line).expect("the status line");
    assert_eq!(line.trim_end(), "HTTP/1.1 200 OK");
    loop {
        line.clear();
        lines.read_line(&mut line).expect("a header line");
        if line.trim_end().is_empty() {
            break;
        }
    }
    // Each event is read before the next is sent: the stream stays open, and nothing waits
    // for a buffer to fill.
    for index in 0..3 {
        send.send(format!("update {index}")).expect("the server takes the event");
        line.clear();
        lines.read_line(&mut line).expect("the event arrives within the bound");
        assert_eq!(line.trim_end(), format!("data: update {index}"));
        line.clear();
        lines.read_line(&mut line).expect("the blank line ending the event");
        assert_eq!(line.trim_end(), "");
    }
    drop(send);
    serving.join().expect("the server thread ends");
}
