//! Prove the actual owner stays readable and answers controls before a real Codex send settles.
#![cfg(unix)]

use std::os::unix::net::UnixListener;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use carlos_proto::commands::{ApprovalAnswer, SendPolicy};
use carlos_proto::session::Snapshot;
use carlos_proto::submissions::{ClientOperation, OperationState};
use serde_json::{json, Value};
use tungstenite::Message;

use super::{EngineCommand, EngineHandle, EngineReply};
use crate::backend::BackendClient;
use crate::codex_connection::CodexClient;
use crate::session::state::SessionState;

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn owner_keeps_reads_controls_and_private_results_live_while_send_waits() {
    let directory = tempfile::tempdir().expect("private peer directory");
    let path = directory.path().join("native.sock");
    let listener = UnixListener::bind(&path).expect("native listener");
    let (log, native_log) = mpsc::channel();
    let peer = thread::spawn(move || serve(listener, log));
    let mut client = CodexClient::start(Some(path.clone())).expect("real native adapter");
    let events = client.take_events_rx().expect("native events");
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session.configure_codex_connection(1, Some(path));
    let (engine, notices) = EngineHandle::spawn(session, Box::new(client), events);
    send(&engine, 1, "first");
    assert_eq!(next_send(&native_log), "turn/start");
    send(&engine, 2, "second");
    let snapshot = wait_snapshot(&engine, &notices);
    assert_eq!(snapshot.entries[0].text, "fact while send waits");
    assert!(
        native_log.try_recv().is_err(),
        "second ordinary send must wait"
    );
    let turn = snapshot.info.current_turn.expect("public current turn");
    request(
        &engine,
        EngineCommand::Interrupt {
            operation: operation(3),
            turn,
        },
    );
    let card = snapshot.approvals[0].id.clone();
    answer(&engine, 4, &card);
    answer(&engine, 5, &card);
    let mut controls = [
        native_log
            .recv_timeout(TIMEOUT)
            .expect("first urgent control"),
        native_log
            .recv_timeout(TIMEOUT)
            .expect("second urgent control"),
    ];
    controls.sort();
    assert_eq!(controls, ["answer", "turn/interrupt"]);
    assert_eq!(next_send(&native_log), "turn/steer");
    let outcomes = wait_results(&engine, &notices);
    assert!(outcomes
        .iter()
        .any(|result| result.operation == operation(1)
            && matches!(result.state, OperationState::Accepted { .. })));
    assert!(outcomes
        .iter()
        .any(|result| result.operation == operation(5)
            && matches!(result.state, OperationState::Rejected { .. })));
    assert!(!outcomes
        .iter()
        .any(|result| result.operation.client != operation(1).client));
    engine.shutdown();
    engine.join().expect("owner stops");
    peer.join().expect("only attached socket closes");
}

fn answer(engine: &EngineHandle, id: u64, card: &carlos_proto::ids::EntryId) {
    request(
        engine,
        EngineCommand::Approve {
            operation: operation(id),
            card: card.clone(),
            answer: ApprovalAnswer::Decision {
                decision: carlos_proto::session::Decision::Once,
            },
        },
    );
}

fn operation(id: u64) -> ClientOperation {
    ClientOperation {
        client: "11111111111111111111111111111111".parse().expect("client"),
        id: carlos_proto::ids::OperationId::new(id).expect("counter"),
    }
}

fn request(engine: &EngineHandle, command: EngineCommand) -> EngineReply {
    engine
        .request(command)
        .expect("bounded admission")
        .recv_timeout(TIMEOUT)
        .expect("responsive owner")
}

fn send(engine: &EngineHandle, id: u64, text: &str) {
    assert!(matches!(
        request(
            engine,
            EngineCommand::Send {
                operation: operation(id),
                text: text.into(),
                policy: SendPolicy::Steer
            }
        ),
        EngineReply::Operation(_)
    ));
}

fn wait_snapshot(engine: &EngineHandle, notices: &mpsc::Receiver<()>) -> Snapshot {
    let deadline = Instant::now() + TIMEOUT;
    loop {
        let EngineReply::Snapshot(snapshot) =
            request(engine, EngineCommand::Snapshot { limit: 100 })
        else {
            panic!("snapshot");
        };
        if !snapshot.approvals.is_empty() && !snapshot.entries.is_empty() {
            return *snapshot;
        }
        notices
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("native facts");
    }
}

fn wait_results(
    engine: &EngineHandle,
    notices: &mpsc::Receiver<()>,
) -> Vec<carlos_proto::submissions::OperationResult> {
    let deadline = Instant::now() + TIMEOUT;
    let mut results = Vec::new();
    loop {
        let EngineReply::Outcomes(outcomes) = request(engine, EngineCommand::Outcomes) else {
            panic!("private outcomes");
        };
        results.extend(outcomes.into_iter().filter(|result| {
            !matches!(
                result.state,
                OperationState::Pending | OperationState::Queued
            )
        }));
        if results
            .iter()
            .any(|result| result.operation == operation(2))
            && results
                .iter()
                .any(|result| result.operation == operation(5))
        {
            return results;
        }
        notices
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("private completion");
    }
}

fn next_send(log: &mpsc::Receiver<String>) -> String {
    log.recv_timeout(TIMEOUT).expect("native command")
}

fn serve(listener: UnixListener, log: mpsc::Sender<String>) {
    let stream = listener.accept().expect("accept native connection").0;
    stream
        .set_read_timeout(Some(TIMEOUT))
        .expect("bound native peer");
    let mut socket = tungstenite::accept(stream).expect("native WebSocket");
    let mut held = None;
    let mut interrupted = false;
    let mut answered = false;
    while let Ok(message) = socket.read() {
        if !message.is_text() {
            continue;
        }
        let request: Value =
            serde_json::from_str(message.to_text().expect("text")).expect("native JSON");
        if request["id"] == "card" {
            assert!(!answered, "a card can be answered only once");
            assert_eq!(request["result"], json!({"decision":"accept"}));
            answered = true;
            log.send("answer".into()).expect("record answer");
        } else {
            let method = request["method"].as_str().expect("native method");
            if method == "turn/start" {
                held = Some(request["id"].clone());
                log.send(method.into()).expect("record held send");
                initial_facts(&mut socket);
                continue;
            }
            if method == "turn/interrupt" {
                interrupted = true;
                log.send(method.into()).expect("record interrupt");
            }
            if method == "turn/steer" {
                assert!(answered && interrupted);
                log.send(method.into()).expect("record second send");
            }
            write(&mut socket, json!({"id":request["id"],"result":{}}));
        }
        if interrupted && answered {
            if let Some(id) = held.take() {
                write(
                    &mut socket,
                    json!({"id":id,"result":{"turn":{"id":"live"}}}),
                );
            }
        }
    }
}

fn initial_facts(socket: &mut tungstenite::WebSocket<std::os::unix::net::UnixStream>) {
    for event in [
        json!({"method":"turn/started","params":{"threadId":"root","turn":{"id":"live"}}}),
        json!({"method":"item/agentMessage/delta","params":{"threadId":"root","itemId":"message","turnId":"live","delta":"fact while send waits"}}),
        json!({"id":"card","method":"item/commandExecution/requestApproval","params":{"threadId":"root","turnId":"live","itemId":"tool","command":"true"}}),
    ] {
        write(socket, event);
    }
}

fn write(socket: &mut tungstenite::WebSocket<std::os::unix::net::UnixStream>, value: Value) {
    socket
        .send(Message::Text(value.to_string().into()))
        .expect("native peer write");
}
