//! Hold a newer body reply until that socket has received the facts identifying its version.

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::time::Duration;

use carlos_core::engine::{EngineChanges, EngineFrame, EngineReply};
use carlos_proto::commands::RequestId;
use carlos_proto::ids::{Sequence, SessionId};
use serde_json::{json, Value};

use super::super::body::{Cursor, Format};
use super::super::state::{Interest, OutputWatch, Read, ReadKind};
use super::super::Manager;

#[test]
fn source_and_output_replies_wait_for_their_shared_entry_version() {
    for source in [true, false] {
        for watch in [true, false] {
            check_order(source, watch, false);
        }
    }
}

#[test]
fn direct_body_read_without_a_session_stream_does_not_wait_for_one() {
    let root = tempfile::tempdir().expect("private state");
    let guard = File::create(root.path().join("guard")).expect("state guard");
    let mut manager = Manager::new(root.path().into(), None, guard).expect("manager");
    let (server, terminal) = UnixStream::pair().expect("socket pair");
    terminal
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("read bound");
    manager.accept(server);
    let session: SessionId = "a".repeat(32).parse().expect("session");
    enqueue(&mut manager, &session, body_reply(true), true, false);
    manager.drain_reads();
    assert!(manager.reads.is_empty());
    assert_eq!(manager.read_bytes, 0);
    assert_eq!(line(&mut BufReader::new(terminal))["id"], 7);
}

#[test]
fn stale_body_errors_wait_for_their_replacement_facts_too() {
    for source in [true, false] {
        for watch in [true, false] {
            check_order(source, watch, true);
        }
    }
}

fn check_order(source: bool, watch: bool, failed: bool) {
    let root = tempfile::tempdir().expect("private state");
    let guard = File::create(root.path().join("guard")).expect("state guard");
    let mut manager = Manager::new(root.path().into(), None, guard).expect("manager");
    let (server, terminal) = UnixStream::pair().expect("socket pair");
    terminal
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("read bound");
    manager.accept(server);
    let session: SessionId = "a".repeat(32).parse().expect("session");
    manager.peers.get_mut(&1).expect("peer").sessions.insert(
        session.clone(),
        Interest {
            seq: Sequence::new(1),
            syncing: false,
            generation: 1,
        },
    );
    enqueue(
        &mut manager,
        &session,
        body_result(source, failed),
        source,
        watch,
    );
    manager.drain_reads();
    assert_eq!(
        manager.reads.len(),
        1,
        "body must wait for its numbered facts"
    );
    assert_eq!(manager.read_bytes, 65536, "waiting bytes remain charged");
    let event = json!({"jsonrpc":"2.0","method":"session/event","params":{
        "session":session,"seq":"2","event":{"kind":"entry_updated","entry":{
            "id":entry(),"kind":"answer","text":"replacement","done":true,"tool":null,
            "source":{"revision":"2","bytes":"11"},"operation":null
        }}
    }});
    let mut bytes = serde_json::to_vec(&event).expect("shared entry frame");
    bytes.push(b'\n');
    manager.deliver_changes(
        1,
        &session,
        &EngineChanges::Replay {
            frames: vec![EngineFrame {
                seq: Sequence::new(2),
                bytes: bytes.into(),
            }],
            seq: Sequence::new(2),
        },
    );
    manager.drain_reads();
    assert!(manager.reads.is_empty());
    assert_eq!(manager.read_bytes, 0);
    let mut terminal = BufReader::new(terminal);
    assert_eq!(line(&mut terminal), event);
    assert_eq!(line(&mut terminal)["id"], 7);
}

fn body_result(source: bool, failed: bool) -> EngineReply {
    if failed {
        EngineReply::BodyFailure {
            error: carlos_proto::commands::RpcError {
                code: -32012,
                message: "output body replaced; restart without a cursor".into(),
            },
            seq: Sequence::new(2),
        }
    } else {
        body_reply(source)
    }
}

fn enqueue(
    manager: &mut Manager,
    session: &SessionId,
    reply: EngineReply,
    source: bool,
    watch: bool,
) {
    let format = if source {
        Format::Source
    } else {
        Format::Normalized
    };
    let kind = if watch {
        manager.peers.get_mut(&1).expect("peer").outputs.insert(
            (session.clone(), entry().parse().expect("entry"), format),
            OutputWatch {
                cursor: if source {
                    Cursor::Source(None)
                } else {
                    Cursor::Normalized(None)
                },
                generation: 9,
                reading: true,
                dirty: false,
                done: false,
            },
        );
        ReadKind::OutputFirst {
            id: RequestId::Integer(7),
            entry: entry().parse().expect("entry"),
            format,
            generation: 9,
        }
    } else {
        ReadKind::Rpc(RequestId::Integer(7))
    };
    let (send, receive) = mpsc::channel();
    send.send(reply).expect("completed body");
    manager.read_bytes = 65536;
    manager.reads.push(Read {
        peer: 1,
        session: session.clone(),
        kind,
        reply: receive,
        ready: None,
        reserved: 65536,
    });
}

fn body_reply(source: bool) -> EngineReply {
    if source {
        EngineReply::Source(serde_json::from_value(json!({
            "entry":entry(),"source":{"revision":"2","bytes":"11"},"offset":"0",
            "from":null,"next":"new-end","text":"replacement","has_more":false,"done":true,"seq":"2"
        })).expect("source chunk"))
    } else {
        EngineReply::Output(serde_json::from_value(json!({
            "entry":entry(),"revision":"2","from":null,"next":"new-end",
            "segments":[{"kind":"plain","text":"replacement"}],"has_more":false,"done":true,"seq":"2"
        })).expect("output chunk"))
    }
}

fn entry() -> String {
    format!("{}-0000000000000001", "b".repeat(32))
}

fn line(terminal: &mut BufReader<UnixStream>) -> Value {
    let mut line = String::new();
    terminal.read_line(&mut line).expect("ordered wire frame");
    serde_json::from_str(&line).expect("JSON frame")
}
