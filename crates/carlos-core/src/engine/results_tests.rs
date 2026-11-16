//! Keep a batch of returned human drafts in the originating device's send order.

use carlos_proto::ids::{ClientId, OperationId, SessionId};
use carlos_proto::submissions::{ClientOperation, SubmittedText};

use super::Results;
use crate::engine::mailbox::Mailbox;
use crate::engine::EngineCommand;

#[test]
fn failed_drafts_return_in_numeric_send_order_and_release_their_original_permits() {
    let session: SessionId = "a".repeat(32).parse().expect("session");
    let client: ClientId = "b".repeat(32).parse().expect("private origin");
    let (mailbox, receiver) = Mailbox::new(64, 65536);
    let mut results = Results::default();
    let operations: Vec<_> = (1..=64)
        .map(|id| ClientOperation {
            client: client.clone(),
            id: OperationId::new(id).expect("positive operation counter"),
        })
        .collect();
    for operation in &operations {
        let _reply = mailbox
            .request(EngineCommand::Info)
            .expect("original admission");
        results.admit(
            &session,
            operation.clone(),
            receiver.recv().expect("permit").permit,
        );
    }
    assert_eq!(
        results
            .collect()
            .into_iter()
            .map(|result| result.operation)
            .collect::<Vec<_>>(),
        operations
    );
    assert!(
        mailbox.request(EngineCommand::Info).is_err(),
        "pending results keep admission charged"
    );
    for operation in operations.iter().rev() {
        results.recover(
            SubmittedText {
                operation: Some(operation.clone()),
                text: format!("human {}", operation.id.get()),
            },
            "native closed",
            true,
        );
    }
    let returned = results.collect();
    assert_eq!(
        returned
            .iter()
            .map(|result| &result.operation)
            .collect::<Vec<_>>(),
        operations.iter().collect::<Vec<_>>()
    );
    assert!(
        results.collect().is_empty(),
        "each returned draft is reported once"
    );
    assert!(
        mailbox.request(EngineCommand::Info).is_ok(),
        "terminal results release the exact permits"
    );
}
