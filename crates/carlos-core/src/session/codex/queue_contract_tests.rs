//! Keep queue acknowledgements, rich inputs and rewind guards at their native Core owner.

use serde_json::json;

use super::queue::QueueUpdate;
use super::queue_contract_support::{add, call, entry, reply, session, submit};
use super::queue_edit::{operate, save_edit};
use super::queue_receive;
use super::queue_state::editable_text;
use crate::backend::BackendKind;
use crate::session::backend_test_support::context;
use crate::session::backend_update::BackendChange;
use crate::session::command_work::PreparedStatus;
use crate::session::prompt_commands::PromptContext;
use crate::session::test_backend::TestBackend;

#[test]
fn queued_human_enters_history_only_after_native_consumption_and_duplicate_sends_are_blocked() {
    let mut state = session();
    let peer = TestBackend::default();
    let scope = state.codex_scope();
    let mut update = QueueUpdate::default();
    let params = add("queued 日本語");
    assert!(submit(&peer, &mut state, &mut update, "queued 日本語"));
    assert!(state.messages().is_empty());
    assert_eq!(state.input_history().history_len(), 0);
    assert!(!submit(
        &peer,
        &mut state,
        &mut update,
        "must not duplicate"
    ));
    assert_eq!(peer.calls.lock().expect("calls").len(), 1);
    assert_eq!(call(&peer, 0).0, "thread/queue/add");
    assert_eq!(call(&peer, 0).1, params);
    queue_receive::reply(
        &mut state,
        scope,
        &mut update,
        &reply(
            &peer,
            0,
            json!({"result":{"queuedSubmission":{"id":"native-queue-1",
            "clientUserMessageId":params["clientUserMessageId"],"input":params["input"]}}}),
        ),
    );
    assert!(state.messages().is_empty());
    assert_eq!(state.input_history().history_len(), 0);
    let consumed = json!({"method":"item/completed","params":{"threadId":"root",
        "turnId":"consumed-turn","item":{"id":"native-user-item",
        "clientId":params["clientUserMessageId"],"type":"userMessage","content":params["input"]}}});
    for _ in 0..2 {
        state.handle_backend_line(&consumed.to_string(), &context(&state));
    }
    assert_eq!(
        state
            .messages()
            .iter()
            .filter(|row| row.text == "queued 日本語")
            .count(),
        1
    );
    assert_eq!(state.input_history().history_len(), 1);
    assert!(!state.codex_queue().pending());
}

#[test]
fn queue_failure_and_eof_return_submitted_text_once_without_an_automatic_retry() {
    let mut state = session();
    let peer = TestBackend::default();
    let scope = state.codex_scope();
    let mut update = QueueUpdate::default();
    assert!(submit(&peer, &mut state, &mut update, "submitted first"));
    let rejected = reply(
        &peer,
        0,
        json!({"error":{"code":-32000,"message":"queue rejected"}}),
    );
    queue_receive::reply(&mut state, scope, &mut update, &rejected);
    assert_eq!(update.recovery.take().as_deref(), Some("submitted first"));
    queue_receive::reply(&mut state, scope, &mut update, &rejected);
    assert!(update.recovery.is_none());
    assert!(!state.codex_queue().pending());
    assert_eq!(state.codex_queue().error(), Some("queue rejected"));
    assert_eq!(peer.calls.lock().expect("calls").len(), 1);
    assert!(submit(&peer, &mut state, &mut update, "keep after EOF"));
    let closed = state.handle_backend_line(
        &json!({"method":"carlos/codex/closed","params":{"message":"native EOF"}}).to_string(),
        &context(&state),
    );
    let BackendChange::Closed(closed) = closed.change else {
        panic!("native EOF report")
    };
    assert_eq!(closed.recovery.as_deref(), Some("keep after EOF"));
    assert!(state.codex_scope().closed());
    assert!(!state.codex_queue().pending());
    assert!(state.take_codex_queue_recovery().is_none());
    assert_eq!(peer.calls.lock().expect("calls").len(), 2);
}

#[test]
fn attachments_and_text_spans_keep_their_exact_native_input_when_started_without_editing() {
    let mut attached = entry("attached", "text");
    attached["input"]
        .as_array_mut()
        .expect("inputs")
        .push(json!({"type":"image","url":"https://example.test/image"}));
    let mut spans = entry("span", "token");
    spans["input"][0]["text_elements"] =
        json!([{"byteRange":{"start":0,"end":5},"placeholder":"token"}]);
    for native in [attached, spans] {
        let mut state = session();
        let peer = TestBackend::default();
        let scope = state.codex_scope();
        state.set_codex_queue_entries_for_test(vec![native.clone()]);
        state.set_codex_queue_known_for_test(true);
        assert!(editable_text(&native).is_none());
        let page = state.queue_page(0, 32);
        assert!(page.entries[0].text_bytes.is_none());
        operate(
            &peer,
            &mut state,
            scope,
            &mut QueueUpdate::default(),
            native["id"].as_str(),
            "start",
        )
        .expect("rich input may still start");
        assert_eq!(call(&peer, 0).0, "thread/queue/start");
        assert_eq!(
            call(&peer, 0).1,
            json!({"threadId":"root","queuedSubmissionId":native["id"]})
        );
        assert_eq!(state.codex_queue().entries(), [native]);
    }
}

#[test]
fn an_external_edit_or_consumption_rejects_the_old_identity_before_any_native_write() {
    let mut state = session();
    let peer = TestBackend::default();
    let scope = state.codex_scope();
    let original = entry("a", "first")["input"].clone();
    state.set_codex_queue_entries_for_test(vec![entry("a", "first"), entry("b", "second")]);
    state.set_codex_queue_known_for_test(true);
    let revision = state.queue_page(0, 32).queue.revision;
    state.set_codex_queue_entries_for_test(vec![
        entry("a", "external new value"),
        entry("b", "second"),
    ]);
    state.invalidate_codex_queue();
    state.set_codex_queue_known_for_test(true);
    assert_ne!(state.queue_page(0, 32).queue.revision, revision);
    let mut update = QueueUpdate::default();
    let error = save_edit(
        &peer,
        &mut state,
        scope,
        &mut update,
        "a",
        &original,
        "independent draft".into(),
    )
    .expect_err("stale edit rejected");
    assert!(error.contains("this draft is preserved"));
    assert!(peer.calls.lock().expect("calls").is_empty());
    state.set_codex_queue_entries_for_test(vec![entry("b", "second")]);
    assert!(operate(&peer, &mut state, scope, &mut update, Some("a"), "delete").is_err());
    assert!(peer.calls.lock().expect("calls").is_empty());
    operate(&peer, &mut state, scope, &mut update, Some("b"), "delete")
        .expect("only explicitly reselected entry can be deleted");
    assert_eq!(
        call(&peer, 0).1,
        json!({"threadId":"root","queuedSubmissionId":"b"})
    );
}

#[test]
fn a_fork_cannot_discard_an_unsettled_queue_submission_or_an_unrecovered_draft() {
    for recovery in [false, true] {
        let mut state = session();
        let peer = TestBackend::default();
        if !recovery {
            assert!(submit(
                &peer,
                &mut state,
                &mut QueueUpdate::default(),
                "pending queue input"
            ));
        }
        let selected = state.transcript_allocate_card();
        let prepared = state
            .prepare_rewind_text(
                BackendKind::Codex,
                selected,
                "edited previous prompt".into(),
                &PromptContext {
                    operation: None,
                    claude_queue: false,
                },
                recovery,
            )
            .expect("queue guard rejects before work");
        assert!(prepared.work.is_none());
        assert!(matches!(prepared.status, PreparedStatus::Failed(_)));
        assert_eq!(state.thread_id(), "root");
        assert_eq!(state.codex_queue().pending(), !recovery);
        assert_eq!(
            peer.calls.lock().expect("calls").len(),
            usize::from(!recovery)
        );
        assert!(state
            .messages()
            .last()
            .expect("explicit queue warning")
            .text
            .contains("Finish or recover the Codex queue request"));
    }
}
