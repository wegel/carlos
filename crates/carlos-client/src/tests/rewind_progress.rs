//! Keep an operation's adopted fork across a later failure and an older admission response.

use carlos_proto::submissions::{ClientOperation, OperationResult, OperationState, RecoveryKind};

use super::session;
use crate::{LocalText, Operations};

#[test]
fn a_private_fork_fact_survives_the_later_send_failure_without_replaying_a_mutation() {
    let operation = operation();
    let mut operations = Operations::default();
    operations
        .begin(
            operation.clone(),
            Some(session()),
            Some(LocalText {
                kind: RecoveryKind::Edit,
                text: "edited original".into(),
            }),
        )
        .expect("one explicit edit");
    let result = |rewound, state| OperationResult {
        operation: operation.clone(),
        session: Some(session()),
        rewound,
        state,
    };
    assert!(operations
        .apply(result(true, OperationState::Pending))
        .expect("fork adopted"));
    assert!(!operations
        .apply(result(false, OperationState::Pending))
        .expect("late admission"));
    assert!(operations
        .apply(result(
            true,
            OperationState::Rejected {
                message: "edited prompt send failed".into(),
                recovery: None,
            }
        ))
        .expect("later failure"));
    let record = operations.get(&operation).expect("original private result");
    assert!(record.rewound);
    assert_eq!(record.text(), Some("edited original"));
}

#[test]
fn an_ordinary_prompt_cannot_claim_to_have_adopted_a_fork() {
    let operation = operation();
    let mut operations = Operations::default();
    operations
        .begin(
            operation.clone(),
            Some(session()),
            Some(LocalText {
                kind: RecoveryKind::Prompt,
                text: "ordinary human".into(),
            }),
        )
        .expect("ordinary prompt");
    assert!(operations
        .apply(OperationResult {
            operation,
            session: Some(session()),
            rewound: true,
            state: OperationState::Pending,
        })
        .is_err());
}

fn operation() -> ClientOperation {
    ClientOperation {
        client: "a".repeat(32).parse().expect("client"),
        id: carlos_proto::ids::OperationId::new(1).expect("operation"),
    }
}
