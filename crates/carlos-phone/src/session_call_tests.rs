//! Prove exact private prompts and checked session admission through the exported phone API.

use std::slice::from_ref;

use crate::test_support::{connected, hello, request, subscribed, token, SESSION};
use crate::{PhoneAvailability, PhoneBackend, PhoneError, PhoneOperationState, PhoneSendMode};

#[test]
fn a_failed_partial_write_preserves_private_text_and_never_replays_the_prompt() {
    let client = subscribed("ready");
    let text = "private exact\n  prompt 🦀";
    let submission = client
        .send_prompt(SESSION.into(), text.into(), PhoneSendMode::Queue)
        .expect("send");
    let command = request(&submission.frame);
    assert_eq!(command["method"], "session/send");
    assert_eq!(command["params"]["text"], text);
    assert_eq!(command["params"]["policy"], "queue");
    assert!(submission.operation.needs_echo);
    let batch = client
        .write_failed(
            submission.frame.generation,
            submission.frame.request_id,
            true,
        )
        .expect("write failed");
    assert!(!batch.ready);
    assert_eq!(
        batch.operations.as_slice(),
        from_ref(&submission.operation.key)
    );
    let retained = client
        .operation(submission.operation.key.clone())
        .expect("lookup")
        .expect("retained");
    assert_eq!(retained.text.as_deref(), Some(text));
    assert!(matches!(
        retained.state,
        PhoneOperationState::Uncertain { .. }
    ));
    let greeting = client.begin("phone".into(), token()).expect("reconnect");
    let recovery = client
        .receive(greeting.generation, hello(greeting.request_id, true))
        .expect("resume");
    assert!(recovery.ready);
    assert!(recovery
        .outgoing
        .iter()
        .all(|frame| request(frame)["method"] != "session/send"));
    assert!(recovery
        .outgoing
        .iter()
        .any(|frame| request(frame)["method"] == "operation/read"));
    assert_eq!(
        client
            .operation(submission.operation.key)
            .expect("lookup")
            .expect("retained")
            .text
            .as_deref(),
        Some(text)
    );
    assert!(connected().operations().expect("other device").is_empty());
}

#[test]
fn starting_or_disconnected_facts_do_not_admit_early_prompts() {
    let client = subscribed("starting");
    assert!(matches!(
        client.catalog().expect("catalog")[0].activity.availability,
        PhoneAvailability::Starting
    ));
    assert!(matches!(
        client.send_prompt(SESSION.into(), "draft".into(), PhoneSendMode::Queue),
        Err(PhoneError::State { .. })
    ));
    assert!(client.operations().expect("private work").is_empty());
    let ready = subscribed("ready");
    ready.disconnect(1).expect("disconnect");
    assert!(matches!(
        ready.send_prompt(SESSION.into(), "private draft".into(), PhoneSendMode::Queue),
        Err(PhoneError::State { .. })
    ));
    assert_eq!(ready.catalog().expect("cached catalog")[0].id, SESSION);
    assert!(ready.operations().expect("no submission").is_empty());
}

#[test]
fn creation_uses_an_explicit_host_directory_and_retains_one_original_operation() {
    let client = connected();
    assert!(matches!(
        client.create_session(PhoneBackend::Claude, "  ".into()),
        Err(PhoneError::InvalidInput { .. })
    ));
    assert!(client.operations().expect("no invalid create").is_empty());
    let create = client
        .create_session(PhoneBackend::Grok, "/project with spaces".into())
        .expect("create");
    let command = request(&create.frame);
    assert_eq!(command["method"], "session/create");
    assert_eq!(command["params"]["backend"], "grok");
    assert_eq!(command["params"]["cwd"], "/project with spaces");
    assert_eq!(client.operations().expect("one operation").len(), 1);
    let failed = client
        .write_failed(create.frame.generation, create.frame.request_id, false)
        .expect("not written");
    assert!(!failed.ready);
    assert!(matches!(
        client
            .operation(create.operation.key)
            .expect("lookup")
            .expect("retained")
            .state,
        PhoneOperationState::Rejected { .. }
    ));
}
