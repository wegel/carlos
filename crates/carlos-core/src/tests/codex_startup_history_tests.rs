//! Read saved turns only for existing native threads, without inferring absence from an error.

use std::os::unix::net::UnixListener;
use std::thread;

use serde_json::{json, Value};
use tungstenite::Message;

use super::{endpoint, event, reply};
use crate::backend::BackendClient;
use crate::codex_connection::CodexClient;
use crate::codex_snapshot::{BootstrapHistory, CodexSnapshot};
use crate::protocol_params::initialize_client;

fn serve(listener: UnixListener, saved: Option<Value>) -> Vec<String> {
    let mut socket = tungstenite::accept(listener.accept().expect("native connection").0)
        .expect("native WebSocket");
    let mut methods = Vec::new();
    while let Ok(Message::Text(raw)) = socket.read() {
        let request: Value = serde_json::from_str(&raw).expect("native request");
        let method = request["method"].as_str().expect("method");
        methods.push(method.to_owned());
        let result = match method {
            "initialize" => json!({}),
            "initialized" => continue,
            "thread/turns/list" if saved.is_some() => {
                json!({"data": saved, "nextCursor": null})
            }
            "thread/turns/list" => {
                socket
                    .send(Message::Text(
                        json!({"id":request["id"],"error":{
                    "code":-32600,"message":"thread is not materialized yet"}})
                        .to_string()
                        .into(),
                    ))
                    .expect("unmaterialized history error");
                continue;
            }
            "model/list" => json!({"data":[],"nextCursor":null}),
            "thread/queue/list" | "thread/list" | "thread/backgroundTerminals/list" => {
                json!({"data":[],"nextCursor":null})
            }
            "thread/goal/get" => json!({"goal":null}),
            "thread/read" => json!({"thread":{"id":"fresh","turns":[],
                "status":{"type":"idle"},"model":"native","reasoningEffort":"low"}}),
            other => panic!("unexpected native work: {other}"),
        };
        reply(&mut socket, &request, result);
    }
    methods
}

fn bootstrap(
    history: BootstrapHistory,
    turns: Value,
    ready: bool,
) -> (Option<CodexSnapshot>, Value, Vec<String>) {
    let (directory, listener) = endpoint();
    let saved = turns
        .as_array()
        .filter(|turns| !turns.is_empty())
        .map(|_| turns.clone());
    let server = thread::spawn(move || serve(listener, saved));
    let mut client =
        CodexClient::start(Some(directory.path().join("rpc.sock"))).expect("native attach");
    let receiver = client.take_events_rx().expect("ordered events");
    initialize_client(&client).expect("completed native handshake before bootstrap");
    let runtime = json!({"result":{"thread":{"id":"fresh","turns":turns,
        "status":{"type":"idle"}},"model":"native","reasoningEffort":"low"}})
    .to_string();
    client
        .bootstrap("fresh".into(), runtime, history)
        .expect("start async snapshot");
    let frame = event(
        &receiver,
        if ready {
            "carlos/codex/ready"
        } else {
            "carlos/codex/closed"
        },
    );
    let snapshot = client.take_reconnect_snapshot(1);
    client.stop();
    (
        snapshot,
        frame,
        server.join().expect("native fixture finished"),
    )
}

#[test]
fn new_thread_reaches_ready_without_reading_history_before_the_first_user_message() {
    let (snapshot, _, methods) = bootstrap(BootstrapHistory::NewThread, json!([]), true);
    let snapshot = snapshot.expect("complete fresh snapshot");
    assert_eq!(snapshot.thread["id"], "fresh");
    assert_eq!(snapshot.thread["turns"], json!([]));
    assert_eq!(snapshot.queue, Some(json!([])));
    assert_eq!(snapshot.goal, Some(json!({"goal":null})));
    assert!(methods
        .iter()
        .all(|method| !matches!(method.as_str(), "thread/turns/list" | "thread/items/list")));
    assert!(methods.iter().any(|method| method == "thread/read"));
}

#[test]
fn existing_thread_history_errors_cannot_turn_into_a_successful_empty_snapshot() {
    let (snapshot, frame, methods) = bootstrap(BootstrapHistory::ExistingThread, json!([]), false);
    assert!(snapshot.is_none());
    assert!(frame.to_string().contains("not materialized"));
    assert_eq!(methods, ["initialize", "initialized", "thread/turns/list"]);
}

#[test]
fn a_start_response_with_materialized_turns_still_reads_every_saved_turn() {
    let turns = json!([{"id":"materialized","status":"completed","items":[{
        "id":"saved-answer","type":"agentMessage","text":"Keep this exact native answer"}]}]);
    let (snapshot, _, methods) = bootstrap(BootstrapHistory::NewThread, turns.clone(), true);
    assert_eq!(
        snapshot.expect("complete materialized snapshot").thread["turns"],
        turns
    );
    assert!(methods.iter().any(|method| method == "thread/turns/list"));
    assert!(methods.iter().any(|method| method == "thread/read"));
}
