//! Unscoped archive outcomes cannot adopt conversation rows or lose their private retry identity.

use carlos_proto::commands::Command;
use carlos_proto::ids::OperationId;
use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState};
use serde_json::json;

use super::client::{attached, command, greet, reply};
use super::session;
use crate::operations::{LocalText, Operations};

#[test]
fn archive_reconnect_reads_the_original_unscoped_result_without_replaying_a_write() {
    let (mut client, connection) = attached();
    let outgoing = client
        .submit(connection, |operation| Command::SavedArchive {
            operation,
            scope: "a".repeat(32).parse().expect("scope"),
            choice: "b".repeat(32).parse().expect("choice"),
        })
        .expect("explicit archive");
    let Command::SavedArchive { operation, .. } = command(&outgoing).command else {
        panic!("archive");
    };
    reply(
        &mut client,
        &outgoing,
        json!({"operation":operation,"state":{"kind":"pending"}}),
    );
    assert!(client
        .operations()
        .get(&operation)
        .expect("archive")
        .session
        .is_none());
    client.disconnect(connection);
    let (_, effects) = greet(&mut client, true, "d");
    assert!(effects
        .outgoing
        .iter()
        .any(|outgoing| matches!(command(outgoing).command, Command::Operation { .. })));
    assert!(effects
        .outgoing
        .iter()
        .all(|outgoing| !matches!(command(outgoing).command, Command::SavedArchive { .. })));
}

#[test]
fn a_session_mutation_requires_its_exact_session_and_archive_requires_no_session() {
    let operation = ClientOperation {
        client: "a".repeat(32).parse().expect("client"),
        id: OperationId::new(1).expect("operation"),
    };
    let pending = |session| OperationResult {
        operation: operation.clone(),
        session,
        rewound: false,
        state: OperationState::Pending,
    };
    let mut scoped = Operations::default();
    scoped
        .begin(operation.clone(), Some(session()), None::<LocalText>)
        .expect("session mutation");
    assert!(scoped.apply(pending(None)).is_err());
    let mut opening = Operations::default();
    opening
        .begin(operation.clone(), None, None)
        .expect("new/open mutation");
    assert!(opening.apply(pending(None)).is_err());
    assert!(opening
        .apply(pending(Some(session())))
        .expect("real logical address"));
    let mut archive = Operations::default();
    archive.begin_unscoped(operation.clone()).expect("archive");
    assert!(archive.apply(pending(Some(session()))).is_err());
    assert!(!archive.apply(pending(None)).expect("same pending outcome"));
}
