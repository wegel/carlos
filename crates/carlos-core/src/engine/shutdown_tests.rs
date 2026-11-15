//! Keep final closure and private results readable after the owner leaves its command loop.

use std::sync::mpsc;
use std::time::Duration;

use carlos_proto::commands::SendPolicy;
use carlos_proto::ids::Sequence;
use carlos_proto::session::Availability;
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::{EngineCommand, EngineHandle, EngineReply};
use crate::session::models::Role;
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn explicit_shutdown_returns_closed_facts_after_deferred_native_cleanup() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.set_active_turn_id(Some("native-turn".into()));
    let owner = owner(session);
    let operation = operation();
    request(
        &owner,
        EngineCommand::SetRalph {
            operation: operation.clone(),
            on: false,
            options: None,
        },
    );
    request(&owner, EngineCommand::Info);
    let stop_request = owner.stop_session(Sequence::new(0)).expect("first stop");
    assert!(owner.stop_session(Sequence::new(0)).is_err());
    let EngineReply::Stopped {
        update: final_state,
        native,
    } = stop_request.recv_timeout(TIMEOUT).expect("final state")
    else {
        panic!("shutdown update");
    };
    assert!(native.is_ok());
    assert_eq!(final_state.info.availability, Availability::Closed);
    assert!(final_state.info.current_turn.is_none());
    assert!(!final_state.info.ralph);
    assert!(final_state
        .outcomes
        .iter()
        .any(|result| result.operation == operation
            && matches!(result.state, OperationState::Accepted { .. })));
    owner.join().expect("native and worker cleanup completed");
}

#[test]
fn oversized_shared_history_does_not_hide_final_private_draft_recovery() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.append_message(Role::ToolCall, "x".repeat(8 * 1024 * 1024));
    let owner = owner(session);
    let operation = operation();
    request(
        &owner,
        EngineCommand::Send {
            operation: operation.clone(),
            text: "private original 🦊".into(),
            policy: SendPolicy::Steer,
        },
    );
    request(&owner, EngineCommand::Info);
    let stop_request = owner
        .stop_session(Sequence::new(0))
        .expect("capture final outcome");
    let EngineReply::Stopped {
        update: final_state,
        native,
    } = stop_request.recv_timeout(TIMEOUT).expect("final state")
    else {
        panic!("shutdown update");
    };
    assert!(native.is_ok());
    assert!(
        final_state.seq > Sequence::default(),
        "the final cursor survives a page error"
    );
    assert_eq!(
        final_state.changes.err().expect("oversized page").code,
        -32011
    );
    let result = final_state
        .outcomes
        .iter()
        .find(|result| result.operation == operation)
        .expect("private result");
    let (OperationState::Rejected {
        recovery: Some(draft),
        ..
    }
    | OperationState::Uncertain {
        recovery: Some(draft),
        ..
    }) = &result.state
    else {
        panic!("unsupported fixture send retains its draft");
    };
    assert_eq!(draft.text, "private original 🦊");
    assert_eq!(draft.operation.as_ref(), Some(&operation));
    owner.join().expect("owner joined after final read");
}

fn owner(session: SessionState) -> EngineHandle {
    let backend = TestBackend {
        deferred: true,
        ..Default::default()
    };
    let (_sender, receiver) = mpsc::channel();
    EngineHandle::spawn(session, Box::new(backend), receiver.into()).0
}

fn operation() -> ClientOperation {
    ClientOperation {
        client: "a".repeat(32).parse().expect("client"),
        id: "1".parse().expect("counter"),
    }
}

fn request(owner: &EngineHandle, command: EngineCommand) -> EngineReply {
    owner
        .request(command)
        .expect("admitted request")
        .recv_timeout(TIMEOUT)
        .expect("owner reply")
}
