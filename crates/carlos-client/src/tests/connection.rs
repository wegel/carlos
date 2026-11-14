//! Check private resume rights, late socket callbacks and counter exhaustion without a transport.

use carlos_proto::commands::{Command, HelloResult};
use carlos_proto::ids::OperationId;

use crate::{Connection, Welcome};

pub(super) fn hello() -> HelloResult {
    HelloResult {
        protocol: carlos_proto::PROTOCOL_VERSION,
        host: "test".into(),
        daemon_version: "test".into(),
        instance: "c".repeat(32).parse().expect("instance"),
        client: "d".repeat(32).parse().expect("client"),
        resumed: false,
        next_operation: OperationId::new(1),
        resume_token: "e".repeat(64).parse().expect("private token"),
    }
}

#[test]
fn reconnect_keeps_unadmitted_counters_and_ignores_old_socket_callbacks() {
    let mut connection = Connection::default();
    let (first, command) = connection.begin("terminal".into()).expect("begin");
    assert!(matches!(command, Command::Hello { resume: None, .. }));
    assert_eq!(
        connection.accept(first, hello()).expect("greet"),
        Welcome::Fresh
    );
    assert_eq!(
        connection.next_operation(first).expect("first op").id.get(),
        1
    );
    assert_eq!(
        connection
            .next_operation(first)
            .expect("second op not yet admitted")
            .id
            .get(),
        2
    );
    connection.disconnect(first);
    assert!(connection.next_operation(first).is_err());
    let (second, command) = connection.begin("terminal".into()).expect("reconnect");
    let Command::Hello {
        resume,
        resume_token,
        instance,
        ..
    } = command
    else {
        panic!("hello");
    };
    assert_eq!(resume, Some(hello().client));
    assert_eq!(resume_token, Some(hello().resume_token));
    assert_eq!(instance, Some(hello().instance));
    let mut resumed = hello();
    resumed.resumed = true;
    resumed.next_operation = OperationId::new(2); // The server saw only the first operation.
    assert_eq!(
        connection.accept(second, resumed).expect("resume"),
        Welcome::Resumed
    );
    connection.disconnect(first);
    assert!(connection.is_ready(second));
    assert!(!connection.is_ready(first));
    assert_eq!(
        connection
            .next_operation(second)
            .expect("never reuse second op")
            .id
            .get(),
        3
    );
}

#[test]
fn a_claimed_resume_cannot_change_its_identity_secret_or_daemon() {
    for field in ["client", "secret", "instance"] {
        let mut connection = Connection::default();
        let (first, _) = connection.begin("terminal".into()).expect("begin");
        connection.accept(first, hello()).expect("greet");
        connection.disconnect(first);
        let (second, _) = connection.begin("terminal".into()).expect("reconnect");
        let mut changed = hello();
        changed.resumed = true;
        match field {
            "client" => changed.client = "f".repeat(32).parse().expect("foreign client"),
            "secret" => changed.resume_token = "f".repeat(64).parse().expect("foreign token"),
            _ => changed.instance = "f".repeat(32).parse().expect("foreign daemon"),
        }
        assert!(connection.accept(second, changed).is_err());
        assert!(!connection.is_ready(second));
    }
}

#[test]
fn exhaustion_never_wraps_and_a_replaced_lease_reports_lost_rights() {
    let mut connection = Connection::default();
    let (first, _) = connection.begin("terminal".into()).expect("begin");
    let mut last = hello();
    last.next_operation = OperationId::new(u64::MAX);
    connection.accept(first, last).expect("greet");
    assert_eq!(
        connection.next_operation(first).expect("last").id.get(),
        u64::MAX
    );
    assert!(connection.next_operation(first).is_err());
    connection.disconnect(first);
    let (second, _) = connection.begin("terminal".into()).expect("reconnect");
    let mut resumed = hello();
    resumed.resumed = true;
    connection.accept(second, resumed).expect("resume");
    assert!(connection.next_operation(second).is_err());
    connection.disconnect(second);
    let (third, _) = connection.begin("terminal".into()).expect("new lease");
    let mut replaced = hello();
    replaced.client = "f".repeat(32).parse().expect("new client");
    assert_eq!(
        connection.accept(third, replaced).expect("expired lease"),
        Welcome::Replaced {
            instance_changed: false
        }
    );
    assert_eq!(
        connection
            .next_operation(third)
            .expect("fresh rights")
            .id
            .get(),
        1
    );
}
