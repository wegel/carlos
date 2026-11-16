//! Preserve stop admission when ordinary writes and urgent native controls both fill up.

use std::sync::mpsc;

use carlos_proto::commands::{Command, Request, RequestId, RpcVersion, SessionOperation};
use carlos_proto::ids::SessionId;
use carlos_proto::submissions::ClientOperation;

use super::ingress::Ingress;

#[test]
fn full_write_and_control_ingress_still_accepts_a_stop_in_original_counter_order() {
    let (wake, _) = mpsc::sync_channel(1);
    let ingress = Ingress::new(wake);
    let session: SessionId = "a".repeat(32).parse().expect("session");
    let operation = ClientOperation {
        client: "b".repeat(32).parse().expect("client"),
        id: "1".parse().expect("counter"),
    };
    for id in 0..64 {
        assert!(ingress.push(
            1,
            request(
                id,
                Command::Send {
                    session: session.clone(),
                    operation: operation.clone(),
                    text: "held".into(),
                    policy: Default::default(),
                }
            ),
            100
        ));
    }
    for id in 64..96 {
        assert!(ingress.push(
            1,
            request(
                id,
                Command::Reconnect {
                    session: session.clone(),
                    operation: operation.clone()
                }
            ),
            100
        ));
    }
    assert!(!ingress.push(
        1,
        request(
            96,
            Command::Reconnect {
                session: session.clone(),
                operation: operation.clone()
            }
        ),
        100
    ));
    assert!(ingress.push(
        1,
        request(97, Command::Stop(SessionOperation { session, operation })),
        100
    ));
    for expected in (0..96).chain([97]) {
        assert_eq!(
            ingress.pop().expect("FIFO").request.id,
            RequestId::Integer(expected)
        );
    }
    assert!(ingress.pop().is_none());
}

fn request(id: i64, command: Command) -> Request {
    Request {
        jsonrpc: RpcVersion::V2,
        id: RequestId::Integer(id),
        command,
    }
}

#[test]
fn full_other_ingress_classes_leave_a_bounded_local_operator_reserve() {
    let (wake, _) = mpsc::sync_channel(1);
    let ingress = Ingress::new(wake);
    let target = SessionOperation {
        session: "a".repeat(32).parse().expect("session"),
        operation: ClientOperation {
            client: "b".repeat(32).parse().expect("client"),
            id: "1".parse().expect("counter"),
        },
    };
    let ordinary = Command::Send {
        session: target.session.clone(),
        operation: target.operation.clone(),
        text: "held".into(),
        policy: Default::default(),
    };
    let control = Command::Reconnect {
        session: target.session.clone(),
        operation: target.operation.clone(),
    };
    for (command, count) in [
        (ordinary, 64),
        (control, 32),
        (Command::Watch {}, 128),
        (Command::Stop(target), 16),
    ] {
        for _ in 0..count {
            assert!(ingress.push(1, request(1, command.clone()), 100));
        }
        assert!(!ingress.push(1, request(1, command), 100));
    }
    for _ in 0..16 {
        assert!(ingress.push(2, request(2, Command::DeviceList {}), 100));
    }
    assert!(!ingress.push(2, request(2, Command::DeviceList {}), 100));
    assert!(!ingress.is_empty());
    for _ in 0..256 {
        ingress.pop().expect("all admitted commands remain queued");
    }
    assert!(ingress.is_empty());
}
