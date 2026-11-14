//! Keep private recovery and read wakeups independent from an oversized shared page.

use std::time::Duration;

use carlos_proto::ids::Sequence;
use carlos_proto::submissions::{ClientOperation, OperationState};

use super::{EngineCommand, EngineHandle, EngineReply};
use crate::grok_backend::receiver_test_support;
use crate::session::models::Role;
use crate::session::state::SessionState;

const TIMEOUT: Duration = Duration::from_secs(5);

#[test]
fn an_oversized_shared_page_does_not_hide_a_private_result_or_create_read_feedback() {
    let (client, events) = receiver_test_support::client(Vec::new());
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.append_message(Role::Assistant, "x".repeat(8 * 1024 * 1024));
    let (owner, _notices) = EngineHandle::spawn(session, Box::new(client), events);
    let operation = ClientOperation {
        client: "a".repeat(32).parse().expect("client"),
        id: "1".parse().expect("counter"),
    };
    request(
        &owner,
        EngineCommand::SetRalph {
            operation: operation.clone(),
            on: false,
        },
    );
    let EngineReply::Updates(update) = request(
        &owner,
        EngineCommand::Updates {
            since: Sequence::new(0),
        },
    ) else {
        panic!("manager batch");
    };
    assert_eq!(
        update.changes.err().expect("page budget is explicit").code,
        -32011
    );
    assert!(update
        .outcomes
        .iter()
        .any(|result| result.operation == operation
            && matches!(result.state, OperationState::Accepted { .. })));
    owner.take_updates();
    assert!(matches!(
        request(&owner, EngineCommand::Info),
        EngineReply::Info { .. }
    ));
    assert!(
        !owner.take_updates(),
        "completing a read wakes the manager without manufacturing another fact batch"
    );
    owner.shutdown();
    owner.join().expect("owner stops");
}

#[test]
fn a_large_ralph_file_fails_before_it_can_fill_an_automatic_prompt_queue() {
    let root = tempfile::tempdir().expect("prompt fixture");
    let path = root.path().join("prompt.md");
    std::fs::File::create(&path)
        .expect("prompt file")
        .set_len(8 * 1024 * 1024 + 1)
        .expect("oversized source");
    let error =
        crate::session::ralph::load_ralph_config(root.path(), path.to_str(), None, None, None)
            .expect_err("bounded file read");
    assert!(error.to_string().contains("exceeds eight MiB"));
}

fn request(owner: &EngineHandle, command: EngineCommand) -> EngineReply {
    owner
        .request(command)
        .expect("bounded admission")
        .recv_timeout(TIMEOUT)
        .expect("responsive owner")
}
