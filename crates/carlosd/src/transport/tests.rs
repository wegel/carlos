//! Prove queue reserves, in-flight charges and socket failure bounds at the real transport edge.

use std::os::unix::net::UnixStream;
use std::sync::atomic::Ordering;
use std::sync::{mpsc, Arc};
use std::time::Duration;

use carlos_proto::commands::{Command, Request, RequestId, RpcVersion};
use carlos_proto::context_defaults::ContextChoice;
use carlos_proto::ids::{SessionId, SettingsRevision};

use super::ingress::Ingress;
use super::outbox::{Frame, Kind, Outbox};
use super::{wire, PeerSocket};

#[test]
fn shared_queue_discard_does_not_release_the_active_write_charge() {
    let outbox = Outbox::default();
    let session = session();
    let bytes: Arc<[u8]> = vec![b'x'; 65536].into();
    for _ in 0..crate::limits::SHARED_FRAMES {
        assert!(outbox.push(Frame {
            bytes: bytes.clone(),
            kind: Kind::Shared(session.clone())
        }));
    }
    let active = outbox.take().expect("writer takes one charged frame");
    assert!(!outbox.push(Frame {
        bytes: bytes.clone(),
        kind: Kind::Shared(session.clone())
    }));
    outbox.discard(|kind| matches!(kind, Kind::Shared(_)));
    let large: Arc<[u8]> = vec![b'x'; crate::limits::SHARED_BYTES].into();
    assert!(!outbox.push(Frame {
        bytes: large.clone(),
        kind: Kind::Shared(session.clone())
    }));
    outbox.finish(&active);
    assert!(outbox.push(Frame {
        bytes: large,
        kind: Kind::Shared(session)
    }));
}

#[test]
fn small_shared_frames_leave_count_room_for_replies_and_replacement_snapshots() {
    let outbox = Outbox::default();
    let session = session();
    let bytes: Arc<[u8]> = b"frame\n".as_slice().into();
    for _ in 0..crate::limits::SHARED_FRAMES {
        assert!(outbox.push(Frame {
            bytes: bytes.clone(),
            kind: Kind::Shared(session.clone())
        }));
    }
    let active = outbox.take().expect("active shared write still counts");
    assert!(!outbox.push(Frame {
        bytes: bytes.clone(),
        kind: Kind::Shared(session.clone())
    }));
    assert!(outbox.push(Frame {
        bytes: bytes.clone(),
        kind: Kind::Snapshot(session.clone())
    }));
    for _ in 1..crate::limits::OUT_FRAMES - crate::limits::SHARED_FRAMES {
        assert!(outbox.push(Frame {
            bytes: bytes.clone(),
            kind: Kind::Reply
        }));
    }
    assert!(!outbox.push(Frame {
        bytes: bytes.clone(),
        kind: Kind::Reply
    }));
    outbox.finish(&active);
    assert!(outbox.push(Frame {
        bytes,
        kind: Kind::Reply
    }));
}

#[test]
fn input_reserves_leave_control_capacity_without_reordering_counters() {
    let (wake, _) = mpsc::sync_channel(1);
    let ingress = Ingress::new(wake);
    let operation = carlos_proto::submissions::ClientOperation {
        client: "11111111111111111111111111111111".parse().expect("client"),
        id: carlos_proto::ids::OperationId::new(1).expect("nonzero"),
    };
    for id in 0..64 {
        assert!(ingress.push(
            1,
            request(
                id,
                Command::SetRalph {
                    session: session(),
                    operation: operation.clone(),
                    on: false
                }
            ),
            100
        ));
    }
    assert!(!ingress.push(
        1,
        request(
            64,
            Command::SetRalph {
                session: session(),
                operation: operation.clone(),
                on: false
            }
        ),
        100
    ));
    let turn = "11111111111111111111111111111111".parse().expect("turn");
    assert!(ingress.push(
        1,
        request(
            65,
            Command::Interrupt {
                session: session(),
                operation,
                turn
            }
        ),
        100
    ));
    for id in (0..64).chain([65]) {
        assert_eq!(
            ingress.pop().expect("retained FIFO command").request.id,
            RequestId::Integer(id)
        );
    }
}

#[test]
fn malformed_envelopes_get_bounded_standard_errors_and_encodes_stop_at_the_cap() {
    assert_eq!(wire::decode(b"{").expect_err("bad JSON").error.code, -32700);
    assert_eq!(
        wire::decode(br#"{"jsonrpc":"2.0","method":"native/call","id":1,"params":{}}"#)
            .expect_err("no tunnel")
            .error
            .code,
        -32601
    );
    let huge_id = serde_json::json!({"jsonrpc":"2.0","id":"x".repeat(300),"method":"sessions/watch","params":{}});
    assert!(wire::decode(&serde_json::to_vec(&huge_id).expect("JSON"))
        .expect_err("id cap")
        .id
        .is_none());
    assert!(wire::encode(&"x".repeat(crate::limits::FRAME_BYTES)).is_err());
}

#[test]
fn every_typed_fixture_request_passes_the_real_daemon_method_and_field_checks() {
    let fixture: serde_json::Value =
        serde_json::from_str(include_str!("../../../carlos-proto/tests/fixtures/v1.json"))
            .expect("fixture");
    for value in fixture["requests"].as_array().expect("requests") {
        let bytes = serde_json::to_vec(value).expect("encode");
        let request = wire::decode(&bytes).expect("typed command must reach manager");
        assert_eq!(serde_json::to_value(request).expect("reencode"), *value);
    }
}

#[test]
fn an_unread_socket_hits_the_write_deadline_without_blocking_its_manager() {
    let (writer, _unread) = UnixStream::pair().expect("local socket pair");
    socket2::SockRef::from(&writer)
        .set_send_buffer_size(4096)
        .expect("force a blocked write before a full frame fits");
    let (wake, wake_rx) = mpsc::sync_channel(1);
    let peer =
        PeerSocket::start(1, writer, Ingress::new(wake.clone()), wake).expect("socket workers");
    assert!(peer.outbox.push(Frame {
        bytes: vec![b'x'; crate::limits::FRAME_BYTES].into(),
        kind: Kind::Reply
    }));
    wake_rx
        .recv_timeout(Duration::from_secs(2))
        .expect("writer fails at its deadline");
    assert!(peer.closed.load(Ordering::SeqCst));
}

fn session() -> SessionId {
    "11111111111111111111111111111111".parse().expect("session")
}
fn request(id: i64, command: Command) -> Request {
    Request {
        jsonrpc: RpcVersion::V2,
        id: RequestId::Integer(id),
        command,
    }
}

#[test]
fn settings_writes_share_the_ordinary_budget_and_leave_reads_available() {
    let (wake, _) = mpsc::sync_channel(1);
    let ingress = Ingress::new(wake);
    let operation = carlos_proto::submissions::ClientOperation {
        client: "11111111111111111111111111111111".parse().expect("client"),
        id: carlos_proto::ids::OperationId::new(1).expect("counter"),
    };
    let expected: SettingsRevision = "a".repeat(64).parse().expect("settings revision");
    for id in 0..64 {
        assert!(ingress.push(
            1,
            request(
                id,
                Command::ToolLabelsSet {
                    session: session(),
                    operation: operation.clone(),
                    expected: expected.clone(),
                    enabled: false,
                    save_default: true,
                }
            ),
            100
        ));
    }
    assert!(!ingress.push(
        1,
        request(
            64,
            Command::SettingsSet {
                session: session(),
                operation,
                expected,
                change: Default::default(),
                save_default: true,
            }
        ),
        100
    ));
    assert!(ingress.push(
        1,
        request(
            65,
            Command::SettingsRead {
                session: session(),
                expected: None,
                offset: 0,
                limit: Some(1),
            }
        ),
        100
    ));
}

#[test]
fn context_writes_cannot_consume_the_reserved_read_budget() {
    let (wake, _) = mpsc::sync_channel(1);
    let ingress = Ingress::new(wake);
    let operation = carlos_proto::submissions::ClientOperation {
        client: "11111111111111111111111111111111".parse().expect("client"),
        id: carlos_proto::ids::OperationId::new(1).expect("counter"),
    };
    for id in 0..65 {
        let admitted = ingress.push(
            1,
            request(
                id,
                Command::ContextSet {
                    session: session(),
                    operation: operation.clone(),
                    choice: ContextChoice::Tokens(500_000),
                },
            ),
            100,
        );
        assert_eq!(admitted, id < 64);
    }
    assert!(ingress.push(
        1,
        request(66, Command::ContextRead { session: session() }),
        100
    ));
}
