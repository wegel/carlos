//! Retire a captured control at writer dispatch while keeping the native connection healthy.

#![cfg(unix)]

use std::os::unix::net::UnixListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};
use tungstenite::Message;

use crate::backend::BackendClient;
use crate::codex_connection::CodexClient;
use crate::dispatch_guard::DispatchGuard;

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn a_tokenized_control_retired_at_the_writer_returns_its_original_context_without_native_io() {
    let directory = tempfile::tempdir().expect("native fixture");
    let path = directory.path().join("control.sock");
    let listener = UnixListener::bind(&path).expect("listen");
    let server = thread::spawn(move || serve(listener));
    let mut client = CodexClient::start(Some(path)).expect("real native adapter");
    let events = client.take_events_rx().expect("native events");
    let checks = Arc::new(AtomicUsize::new(0));
    let checked = checks.clone();
    let commands = client
        .capture_commands()
        .expect("capture")
        .guarded(DispatchGuard::new(move || {
            checked.fetch_add(1, Ordering::SeqCst) == 0
        }))
        .expect("guard");
    let context = json!({"operation":"goal","epoch":7,"token":19,"timeoutMs":1000});
    commands
        .request_control_with_context(
            "thread/goal/set",
            json!({"threadId":"root","status":"active"}),
            context.clone(),
        )
        .expect("enqueued");
    let line = events.recv_timeout(TIMEOUT).expect("retired control reply");
    let frame: Value = serde_json::from_str(&line).expect("reply");
    assert_eq!(frame["method"], "carlos/codex/frame");
    let reply = &frame["params"]["event"];
    assert_eq!(
        checks.load(Ordering::SeqCst),
        2,
        "enqueue passed, writer retired"
    );
    assert_eq!(reply["method"], "carlos/codex/controlReply");
    assert_eq!(reply["params"]["requestContext"], context);
    assert!(reply["params"]["response"]["error"]["message"]
        .as_str()
        .expect("error")
        .contains("retired"));
    client.acknowledge_event(line.len());
    let inspected = client
        .capture_commands()
        .expect("healthy transport")
        .call("test/inspect", json!({}), TIMEOUT)
        .expect("inspect");
    assert_eq!(
        serde_json::from_str::<Value>(&inspected).expect("reply")["result"],
        json!(["test/inspect"])
    );
    client.stop();
    server.join().expect("peer stopped");
}

#[test]
fn a_callback_error_retired_at_the_writer_never_closes_a_healthy_native_connection() {
    let directory = tempfile::tempdir().expect("native fixture");
    let path = directory.path().join("error.sock");
    let listener = UnixListener::bind(&path).expect("listen");
    let server = thread::spawn(move || serve(listener));
    let mut client = CodexClient::start(Some(path)).expect("real native adapter");
    let checks = Arc::new(AtomicUsize::new(0));
    let checked = checks.clone();
    let commands = client
        .capture_commands()
        .expect("capture")
        .guarded(DispatchGuard::new(move || {
            checked.fetch_add(1, Ordering::SeqCst) == 0
        }))
        .expect("guard");
    let error = commands
        .respond_error(&json!("old-card"), -32602, "rejected")
        .expect_err("retired before write");
    assert!(error.to_string().contains("retired"));
    assert_eq!(checks.load(Ordering::SeqCst), 2);
    let inspected = client
        .capture_commands()
        .expect("healthy transport")
        .call("test/inspect", json!({}), TIMEOUT)
        .expect("inspect");
    assert_eq!(
        serde_json::from_str::<Value>(&inspected).expect("reply")["result"],
        json!(["test/inspect"])
    );
    client.stop();
    server.join().expect("peer stopped");
}

fn serve(listener: UnixListener) {
    let stream = listener.accept().expect("accept").0;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .expect("bounded peer");
    let mut socket = tungstenite::accept(stream).expect("native handshake");
    let mut log = Vec::new();
    while let Ok(message) = socket.read() {
        if !message.is_text() {
            continue;
        }
        let request: Value =
            serde_json::from_str(message.to_text().expect("text")).expect("request");
        log.push(request["method"].as_str().unwrap_or("callback").to_owned());
        if request.get("method").is_none() {
            continue;
        }
        let result = if request["method"] == "test/inspect" {
            json!(log)
        } else {
            json!({})
        };
        socket
            .send(Message::Text(
                json!({"id":request["id"],"result":result})
                    .to_string()
                    .into(),
            ))
            .expect("reply");
    }
}
