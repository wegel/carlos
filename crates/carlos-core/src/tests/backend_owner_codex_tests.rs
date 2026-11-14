//! Exercise captured Codex connections against a deterministic native Unix peer.
#![cfg(unix)]

use std::os::unix::net::UnixListener;
use std::sync::mpsc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};
use tungstenite::Message;

use crate::backend::{BackendClient, BackendKind, PromptOrigin};
use crate::codex_connection::CodexClient;
use crate::session::prompt_commands::{PromptContext, PromptOptions};
use crate::session::state::SessionState;

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn codex_fork_checks_permission_after_its_native_validation_read() {
    let directory = tempfile::tempdir().expect("private socket");
    let path = directory.path().join("fork.sock");
    let listener = UnixListener::bind(&path).expect("listen");
    let live = Arc::new(AtomicBool::new(true));
    let server_live = live.clone();
    let server = thread::spawn(move || serve_fork_guard(listener, server_live));
    let mut client = CodexClient::start(Some(path)).expect("native adapter");
    let commands = client
        .capture_commands()
        .expect("captured")
        .guarded(crate::dispatch_guard::DispatchGuard::from_flag(live))
        .expect("guarded");
    let request = crate::backend::RewindForkRequest {
        keep_turns: 0,
        drop_turns: 1,
        before_turn_id: Some("live".into()),
        before_user_item: Some(crate::backend::RewindUserMessage {
            item_id: "user".into(),
            text: "original".into(),
            position: 0,
        }),
    };
    let error = commands
        .fork_from_rewind("root", request, TIMEOUT)
        .expect_err("retired before fork write");
    assert!(error.to_string().contains("retired"), "{error}");
    let response = client
        .capture_commands()
        .expect("healthy native connection")
        .call("test/inspect", json!({}), TIMEOUT)
        .expect("inspect");
    let response: Value = serde_json::from_str(&response).expect("reply");
    assert_eq!(
        response["result"],
        json!(["thread/items/list", "test/inspect"])
    );
    client.stop();
    server.join().expect("peer stops");
}

fn serve_fork_guard(listener: UnixListener, live: Arc<AtomicBool>) {
    let stream = listener.accept().expect("accept").0;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .expect("bounded peer");
    let mut socket = tungstenite::accept(stream).expect("handshake");
    let mut log = Vec::new();
    while let Ok(message) = socket.read() {
        if !message.is_text() {
            continue;
        }
        let request: Value =
            serde_json::from_str(message.to_text().expect("text")).expect("request");
        let method = request["method"].as_str().expect("method");
        log.push(method.to_owned());
        let result = match method {
            "thread/items/list" => {
                live.store(false, Ordering::SeqCst);
                json!({"data":[{"turnId":"live","item":{"id":"user","type":"userMessage","content":[{"type":"text","text":"original"}]}}],"nextCursor":null})
            }
            "thread/fork" => json!({"thread":{"id":"fork","turns":[]}}),
            "test/inspect" => json!(log),
            _ => json!({}),
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

#[test]
fn real_codex_owner_reads_interrupts_and_answers_before_prompt_reply() {
    let directory = tempfile::tempdir().expect("private socket");
    let path = directory.path().join("peer.sock");
    let listener = UnixListener::bind(&path).expect("native listener");
    let server = thread::spawn(move || serve(listener));
    let mut client = CodexClient::start(Some(path)).expect("real native adapter");
    let events = client.take_events_rx().expect("owner events");
    let commands = client.capture_commands().expect("capture");
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.configure_codex_connection(1, None);
    let work = prepare(&mut state);
    let worker = thread::spawn(move || work.execute(&commands));
    let line = events
        .recv_timeout(TIMEOUT)
        .expect("native fact before reply");
    let frame: Value = serde_json::from_str(&line).expect("native frame");
    state.handle_backend_line(
        &frame["params"]["event"].to_string(),
        &state.backend_context(),
    );
    client.acknowledge_event(line.len());
    assert_eq!(
        state.publish().snapshot(100).entries[0].text,
        "fact during wait"
    );
    assert!(!worker.is_finished());
    let commands = client.capture_commands().expect("controls");
    commands
        .call(
            "turn/interrupt",
            json!({"threadId":"root","turnId":"live"}),
            TIMEOUT,
        )
        .expect("interrupt before reply");
    commands
        .respond(&json!("card"), json!({"decision":"accept"}))
        .expect("answer before reply");
    let log: Value = serde_json::from_str(
        &commands
            .call("test/inspect", json!({}), TIMEOUT)
            .expect("native log"),
    )
    .expect("JSON");
    assert_eq!(
        log["result"],
        json!(["turn/start", "turn/interrupt", "answer", "test/inspect"])
    );
    assert!(!worker.is_finished());
    commands
        .call("test/release", json!({}), TIMEOUT)
        .expect("release");
    state
        .finish_backend_work(worker.join().expect("worker"))
        .expect("settle");
    client.stop();
    server.join().expect("native peer stopped");
}

fn prepare(state: &mut SessionState) -> crate::session::command_work::BackendWork {
    state
        .prepare_turn_text(
            BackendKind::Codex,
            "held".into(),
            PromptOptions {
                record_history: true,
                force_start: false,
                origin: PromptOrigin::Human,
            },
            &PromptContext {
                operation: None,
                claude_queue: false,
            },
        )
        .expect("prepare")
        .work
        .expect("work")
}

fn serve(listener: UnixListener) {
    let stream = listener.accept().expect("accept").0;
    stream.set_read_timeout(Some(TIMEOUT)).expect("bound peer");
    let mut socket = tungstenite::accept(stream).expect("native handshake");
    let mut held = None;
    let mut log = Vec::new();
    while let Ok(message) = socket.read() {
        if !message.is_text() {
            continue;
        }
        let request: Value = serde_json::from_str(message.to_text().expect("text")).expect("JSON");
        if request["id"] == "card" {
            assert_eq!(request["result"], json!({"decision":"accept"}));
            log.push("answer".to_string());
            continue;
        }
        let method = request["method"].as_str().expect("method");
        log.push(method.to_owned());
        if method == "turn/start" {
            held = Some(request["id"].clone());
            let fact = json!({"method":"item/agentMessage/delta","params":{"threadId":"root","itemId":"live","delta":"fact during wait"}});
            socket
                .send(Message::Text(fact.to_string().into()))
                .expect("native fact");
            let card = json!({"id":"card","method":"item/commandExecution/requestApproval","params":{"threadId":"root","turnId":"live","itemId":"tool","command":"true"}});
            socket
                .send(Message::Text(card.to_string().into()))
                .expect("native approval");
            continue;
        }
        if method == "test/release" {
            let reply =
                json!({"id":held.take().expect("held request"),"result":{"turn":{"id":"live"}}});
            socket
                .send(Message::Text(reply.to_string().into()))
                .expect("release");
        }
        let result = if method == "test/inspect" {
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
            .expect("control reply");
    }
}

#[test]
fn stopped_real_codex_handle_rejects_dispatch_without_writing() {
    let directory = tempfile::tempdir().expect("private socket");
    let path = directory.path().join("peer.sock");
    let listener = UnixListener::bind(&path).expect("listen");
    let (release, wait) = mpsc::channel();
    let (ready, handshake) = mpsc::channel();
    let server = thread::spawn(move || {
        let stream = listener.accept().expect("accept").0;
        let mut socket = tungstenite::accept(stream).expect("handshake");
        ready.send(()).expect("handshake ready");
        wait.recv_timeout(TIMEOUT).expect("owner stopped");
        while let Ok(message) = socket.read() {
            assert!(!message.is_text(), "no stale command");
        }
    });
    let mut client = CodexClient::start(Some(path)).expect("native adapter");
    handshake.recv_timeout(TIMEOUT).expect("handshake complete");
    let commands = client.capture_commands().expect("capture old generation");
    client.stop();
    assert!(commands.call("turn/start", json!({}), TIMEOUT).is_err());
    release.send(()).expect("release");
    server.join().expect("peer stopped");
}
