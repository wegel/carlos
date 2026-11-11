//! Exercise real local sockets, WebSocket framing and stable reconnect ownership without inference.
#![cfg(unix)]

use std::os::unix::net::UnixListener;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::protocol::frame::coding::{Data, OpCode};
use tungstenite::protocol::frame::Frame;
use tungstenite::{Message, WebSocket};

use crate::backend::BackendClient;
use crate::codex_connection::CodexClient;

fn endpoint() -> (tempfile::TempDir, UnixListener) {
    let directory = tempfile::tempdir().expect("isolated local socket");
    let listener = UnixListener::bind(directory.path().join("rpc.sock")).expect("listen");
    (directory, listener)
}

fn event(receiver: &mpsc::Receiver<String>, method: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let line = receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("bounded native event");
        let frame: Value = serde_json::from_str(&line).expect("generation envelope");
        if frame["params"]["event"]["method"] == method {
            return frame;
        }
    }
}

#[test]
fn fragmented_messages_ping_and_unknown_additive_fields_survive_the_local_codec() {
    let (directory, listener) = endpoint();
    let (release, wait) = mpsc::channel();
    let server = thread::spawn(move || {
        let mut socket =
            tungstenite::accept(listener.accept().expect("accept").0).expect("upgrade");
        socket
            .send(Message::Ping(vec![1, 2, 3].into()))
            .expect("ping");
        socket
            .send(Message::Frame(Frame::message(
                b"{\"method\":\"future/additive\",\"params\":".to_vec(),
                OpCode::Data(Data::Text),
                false,
            )))
            .expect("first fragment");
        socket
            .send(Message::Frame(Frame::message(
                b"{\"unknown\":\"kept\"}}".to_vec(),
                OpCode::Data(Data::Continue),
                true,
            )))
            .expect("second fragment");
        assert_eq!(
            socket.read().expect("automatic pong"),
            Message::Pong(vec![1, 2, 3].into())
        );
        wait.recv_timeout(Duration::from_secs(3))
            .expect("test release");
    });
    let mut client =
        CodexClient::start(Some(directory.path().join("rpc.sock"))).expect("local attach");
    let events = client.take_events_rx().expect("stable events");
    let frame = event(&events, "future/additive");
    assert_eq!(frame["params"]["generation"], 1);
    assert_eq!(frame["params"]["event"]["params"]["unknown"], "kept");
    client.acknowledge_event(frame.to_string().len());
    client.stop();
    release.send(()).expect("release");
    server.join().expect("fixture ended");
}

#[test]
fn oversized_aggregate_fragments_fail_waiting_work_and_close_only_the_local_connection() {
    let (directory, listener) = endpoint();
    let server = thread::spawn(move || {
        let mut socket =
            tungstenite::accept(listener.accept().expect("accept").0).expect("upgrade");
        let request: Value =
            serde_json::from_str(socket.read().expect("request").to_text().expect("text"))
                .expect("JSON");
        let body = json!({"id":request["id"],"result":"x".repeat(9 * 1024 * 1024)}).to_string();
        for (index, opcode) in [Data::Text, Data::Continue, Data::Continue]
            .into_iter()
            .enumerate()
        {
            if socket
                .send(Message::Frame(Frame::message(
                    body.as_bytes()[index * (body.len() / 3)..if index == 2 {
                        body.len()
                    } else {
                        (index + 1) * (body.len() / 3)
                    }]
                        .to_vec(),
                    OpCode::Data(opcode),
                    index == 2,
                )))
                .is_err()
            {
                break;
            }
        }
    });
    let mut client =
        CodexClient::start(Some(directory.path().join("rpc.sock"))).expect("local attach");
    let events = client.take_events_rx().expect("events");
    let error = client
        .call("model/list", json!({}), Duration::from_secs(3))
        .expect_err("nine MiB fragmented message is bounded");
    assert!(error.to_string().contains("exceeds 8 MiB"), "{error}");
    assert_eq!(
        event(&events, "carlos/codex/closed")["params"]["generation"],
        1
    );
    let started = Instant::now();
    client.stop();
    assert!(started.elapsed() < Duration::from_millis(500));
    server.join().expect("fixture ended");
}

#[test]
fn a_blocked_socket_writer_and_stalled_handshake_do_not_block_stop() {
    for upgrade in [false, true] {
        let (directory, listener) = endpoint();
        let (release, wait) = mpsc::channel();
        let server = thread::spawn(move || {
            let stream = listener.accept().expect("accept").0;
            let mut raw = Some(stream);
            let _socket =
                upgrade.then(|| tungstenite::accept(raw.take().expect("stream")).expect("upgrade"));
            let _ = wait.recv_timeout(Duration::from_secs(3));
        });
        let client = CodexClient::start(Some(directory.path().join("rpc.sock")))
            .expect("attach returns before handshake");
        if upgrade {
            assert!(client
                .call(
                    "thread/queue/add",
                    json!({"input":"x".repeat(7*1024*1024)}),
                    Duration::from_millis(100)
                )
                .is_err());
        }
        let started = Instant::now();
        client.stop();
        assert!(started.elapsed() < Duration::from_millis(500));
        release.send(()).expect("release");
        server.join().expect("fixture ended");
    }
}

fn reply(socket: &mut WebSocket<std::os::unix::net::UnixStream>, request: &Value, result: Value) {
    socket
        .send(Message::Text(
            json!({"id":request["id"],"result":result})
                .to_string()
                .into(),
        ))
        .expect("reply");
}

fn serve_snapshot(listener: UnixListener, count: usize) {
    let mut first =
        tungstenite::accept(listener.accept().expect("first attach").0).expect("first upgrade");
    first.close(None).expect("first connection closes");
    let mut socket = tungstenite::accept(listener.accept().expect("reattach").0).expect("upgrade");
    while let Ok(Message::Text(raw)) = socket.read() {
        let request: Value = serde_json::from_str(&raw).expect("JSON request");
        let params = &request["params"];
        let thread = json!({"id":"current-fork","turns":[],"historyMode":"paginated","status":{"type":"idle"},"model":"fixture-b","reasoningEffort":"low"});
        let result = match request["method"].as_str().expect("method") {
            "initialized" => continue,
            "initialize" => json!({}),
            "thread/resume" | "thread/read" => {
                assert_eq!(params["threadId"], "current-fork");
                json!({"thread":thread})
            }
            "thread/turns/list" => {
                json!({"data":[{"id":"turn","status":"completed","itemsView":"notLoaded","items":[]}],"nextCursor":null})
            }
            "thread/items/list" => {
                let index = params["cursor"]
                    .as_str()
                    .unwrap_or("0")
                    .parse::<usize>()
                    .expect("cursor");
                json!({"data":[{"turnId":"turn","item":{"type":"agentMessage","id":format!("item-{index}"),"text":"x".repeat(1024*1024)}}],"nextCursor":(index+1<count).then(||(index+1).to_string())})
            }
            "model/list" => {
                json!({"data":[{"id":"fixture-b","model":"fixture-b","displayName":"B"}],"nextCursor":null})
            }
            "thread/goal/get" => {
                socket.send(Message::Text(json!({"id":request["id"],"error":{"code":-32600,"message":"goals feature is disabled"}}).to_string().into())).expect("disabled optional goal");
                continue;
            }
            "thread/queue/list" | "thread/list" | "thread/backgroundTerminals/list" => {
                json!({"data":[],"nextCursor":null})
            }
            other => panic!("unexpected mutation/read: {other}"),
        };
        reply(&mut socket, &request, result);
    }
}

#[test]
fn reconnect_hands_off_large_history_once_without_serializing_it_into_the_ui_stream() {
    let (directory, listener) = endpoint();
    let server = thread::spawn(move || serve_snapshot(listener, 20));
    let mut client = CodexClient::start(Some(directory.path().join("rpc.sock"))).expect("attach");
    let events = client.take_events_rx().expect("one stable receiver");
    event(&events, "carlos/codex/closed");
    let generation = client
        .reconnect("current-fork".into())
        .expect("async reconnect");
    let ready = event(&events, "carlos/codex/ready");
    assert!(
        ready.to_string().len() < 1024,
        "ready is an ordered ticket, not history JSON"
    );
    assert_eq!(ready["params"]["generation"], generation);
    let snapshot = client
        .take_reconnect_snapshot(generation)
        .expect("one complete slot");
    assert!(snapshot.thread.to_string().len() > 16 * 1024 * 1024);
    assert!(
        snapshot.goal.is_none(),
        "explicit disabled goal does not fail reconnect"
    );
    assert!(client.take_reconnect_snapshot(generation).is_none());
    client.stop();
    server.join().expect("fixture ended");
}

#[test]
fn a_large_full_turn_response_stays_off_the_socket_event_stream() {
    let (directory, listener) = endpoint();
    let (release, wait) = mpsc::channel();
    let server = thread::spawn(move || {
        let mut socket =
            tungstenite::accept(listener.accept().expect("accept").0).expect("upgrade");
        let request: Value =
            serde_json::from_str(socket.read().expect("request").to_text().expect("text"))
                .expect("JSON");
        socket
            .send(Message::Text(
                json!({"id":request["id"],"result":{"data":"x".repeat(9 * 1024 * 1024)}})
                    .to_string()
                    .into(),
            ))
            .expect("large page");
        wait.recv_timeout(Duration::from_secs(5))
            .expect("test release");
    });
    let mut client =
        CodexClient::start(Some(directory.path().join("rpc.sock"))).expect("local attach");
    let events = client.take_events_rx().expect("events");
    let response = client
        .call(
            "thread/turns/list",
            json!({"itemsView":"full"}),
            Duration::from_secs(4),
        )
        .expect("correlated large page");
    assert!(response.len() > 8 * 1024 * 1024);
    assert!(
        events.try_recv().is_err(),
        "history reply must not reach UI"
    );
    client.stop();
    release.send(()).expect("release");
    server.join().expect("fixture ended");
}
