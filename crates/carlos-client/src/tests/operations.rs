//! Preserve exact draft ownership across shared rows, private reply races and lost retry rights.

use carlos_proto::ids::OperationId;
use carlos_proto::submissions::{
    ClientOperation, OperationResult, OperationState, RecoveryKind, SubmittedText,
};

use super::{entry, entry_id, session};
use crate::{LocalText, Operations};

fn operation(client: &str, id: u64) -> ClientOperation {
    ClientOperation {
        client: client.repeat(32).parse().expect("client"),
        id: OperationId::new(id).expect("operation"),
    }
}

fn draft(text: &str) -> Option<LocalText> {
    Some(LocalText {
        kind: RecoveryKind::Prompt,
        text: text.into(),
    })
}

fn result(operation: &ClientOperation, state: OperationState) -> OperationResult {
    OperationResult {
        operation: operation.clone(),
        session: Some(session()),
        state,
    }
}

#[test]
fn private_success_notes_keep_partial_native_results_without_accepting_oversized_data() {
    let mut operations = Operations::default();
    let own = operation("c", 1);
    operations
        .begin(own.clone(), Some(session()), None)
        .expect("submit settings");
    assert!(operations
        .apply(result(
            &own,
            OperationState::Accepted {
                entry: None,
                notice: Some("界".repeat(1366)),
            }
        ))
        .is_err());
    assert!(matches!(
        operations.get(&own).expect("kept submission").state,
        OperationState::Pending
    ));
    let notice = "Future settings acknowledged; live turn is no longer available";
    let accepted = result(
        &own,
        OperationState::Accepted {
            entry: None,
            notice: Some(notice.into()),
        },
    );
    assert!(operations.apply(accepted.clone()).expect("scoped success"));
    assert_eq!(
        operations.get(&own).expect("private result").state,
        accepted.state
    );
    assert!(!operations.apply(accepted).expect("duplicate"));
}

#[test]
fn early_shared_rows_and_late_admission_replies_never_duplicate_another_clients_echo() {
    let mut operations = Operations::default();
    let own = operation("c", 1);
    let other = operation("d", 1);
    operations
        .begin(own.clone(), Some(session()), draft("own"))
        .expect("submit");
    let mut row = entry(20, "other");
    row.operation = Some(other);
    assert!(!operations
        .observe_entry(&session(), &row)
        .expect("foreign row ignored"));
    assert!(operations.get(&own).expect("own").needs_echo());
    row.operation = Some(own.clone());
    row.text = "own".into();
    assert!(operations
        .observe_entry(&session(), &row)
        .expect("own shared row"));
    assert!(!operations.get(&own).expect("own").needs_echo());
    operations
        .apply(result(
            &own,
            OperationState::Accepted {
                entry: Some(entry_id(20)),
                notice: None,
            },
        ))
        .expect("accepted");
    assert!(!operations
        .apply(result(&own, OperationState::Pending))
        .expect("late admission"));
    assert!(matches!(
        operations.get(&own).expect("own").state,
        OperationState::Accepted { .. }
    ));
    assert!(!operations
        .observe_entry(&session(), &row)
        .expect("same row again"));
    row.id = entry_id(21);
    assert!(operations.observe_entry(&session(), &row).is_err());
}

#[test]
fn recovery_requires_the_exact_operation_and_original_text_before_changing_any_record() {
    let mut operations = Operations::default();
    let own = operation("c", 1);
    operations
        .begin(own.clone(), Some(session()), draft("héllo\n🌍"))
        .expect("submit");
    for (origin, text) in [
        (operation("d", 1), "héllo\n🌍"),
        (own.clone(), "different text"),
    ] {
        assert!(operations
            .apply(result(
                &own,
                OperationState::Rejected {
                    message: "failed".into(),
                    recovery: Some(SubmittedText {
                        operation: Some(origin),
                        text: text.into()
                    }),
                }
            ))
            .is_err());
        assert!(operations.get(&own).expect("retained").needs_echo());
    }
    let final_result = result(
        &own,
        OperationState::Uncertain {
            message: "socket closed after a possible write".into(),
            recovery: Some(SubmittedText {
                operation: Some(own.clone()),
                text: "héllo\n🌍".into(),
            }),
        },
    );
    assert!(operations
        .apply(final_result.clone())
        .expect("private recovery"));
    assert!(!operations.apply(final_result).expect("duplicate recovery"));
    assert!(!operations.get(&own).expect("failed").needs_echo());
    assert_eq!(
        operations.get(&own).expect("retained").text(),
        Some("héllo\n🌍")
    );
    operations.forget(&own);
    assert!(!operations
        .apply(result(&own, OperationState::Pending))
        .expect("handled late reply"));
}

#[test]
fn lost_rights_keep_old_text_uncertain_and_leave_a_new_clients_work_untouched() {
    let mut operations = Operations::default();
    let old = operation("c", 1);
    let new = operation("d", 1);
    operations
        .begin(
            old.clone(),
            Some(session()),
            Some(LocalText {
                kind: RecoveryKind::Edit,
                text: "edited".into(),
            }),
        )
        .expect("edit");
    operations
        .begin(new.clone(), Some(session()), draft("new prompt"))
        .expect("new rights");
    assert!(!operations.get(&old).expect("edit").needs_echo());
    operations.lose_rights(&old.client);
    let OperationState::Uncertain {
        recovery: Some(recovery),
        ..
    } = &operations.get(&old).expect("old work").state
    else {
        panic!("uncertain recovery");
    };
    assert_eq!(recovery.operation, Some(old.clone()));
    assert_eq!(recovery.text, "edited");
    assert!(operations.get(&new).expect("new work").needs_echo());
    assert_eq!(operations.iter().count(), 2);
}

#[test]
fn full_private_text_and_count_budgets_reject_new_work_without_discarding_old_drafts() {
    let mut operations = Operations::default();
    let text = "x".repeat(512 * 1024);
    for id in 1..=4 {
        operations
            .begin(operation("c", id), Some(session()), draft(&text))
            .expect("reserved recovery");
    }
    assert!(operations
        .begin(operation("c", 5), Some(session()), draft("another"))
        .is_err());
    assert_eq!(
        operations
            .get(&operation("c", 1))
            .expect("old draft")
            .text(),
        Some(text.as_str())
    );
    operations.forget(&operation("c", 1));
    operations
        .begin(operation("c", 5), Some(session()), draft("another"))
        .expect("released bytes");
    let mut operations = Operations::default();
    for id in 1..=128 {
        operations
            .begin(operation("c", id), None, None)
            .expect("bounded metadata");
    }
    assert!(operations.begin(operation("c", 129), None, None).is_err());
}
