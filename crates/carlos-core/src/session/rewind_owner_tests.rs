//! Keep asynchronous edit drafts and private identities ahead of early helper facts.

use carlos_proto::submissions::ClientOperation;
use serde_json::json;

use super::claude::receive_update::{ClaudeEffect, ClaudeUpdate};
use super::command_work::{BackendCall, BackendCompletion, BackendWork, PreparedStatus};
use super::models::Role;
use super::prompt_commands::PromptContext;
use super::state::SessionState;
use crate::backend::BackendKind;

fn operation(counter: u64) -> ClientOperation {
    ClientOperation {
        client: "a".repeat(32).parse().expect("client"),
        id: counter.to_string().parse().expect("counter"),
    }
}

fn pending() -> (SessionState, BackendWork) {
    let mut state = SessionState::new("root".into(), Default::default(), false);
    state.enable_claude();
    state.append_message(Role::User, "original");
    let id = state.messages()[0].id;
    state.record_prompt_history("original", Some(id));
    state.record_prompt_turn(id, "native-prompt");
    let work = state
        .prepare_rewind_text(
            BackendKind::Claude,
            id,
            "edited".into(),
            &PromptContext {
                operation: Some(operation(1)),
                claude_queue: false,
            },
            false,
        )
        .expect("prepare edit")
        .work
        .expect("fork work");
    (state, work)
}

#[test]
fn a_prepared_claude_edit_owns_its_draft_and_cancels_its_fork_when_replaced() {
    let (mut state, work) = pending();
    assert_eq!(state.claude_edit_prompt_for_test(), Some("edited"));
    let BackendCall::Fork { context, .. } = work.call else {
        panic!("fork");
    };
    assert!(context.liveness.check().is_ok());
    state.begin_claude_edit_for("new edit".into(), Some(operation(2)));
    assert!(context.liveness.check().is_err());
    assert_eq!(state.claude_edit_prompt_for_test(), Some("new edit"));
}

#[test]
fn old_or_malformed_fork_facts_cannot_fail_or_replace_a_new_edit() {
    let (mut state, _work) = pending();
    state.begin_claude_edit_for("new edit".into(), Some(operation(2)));
    for method in [
        "claude/forkFailed",
        "claude/sessionStartFailed",
        "claude/sessionChanged",
    ] {
        for identity in [json!(operation(1)), json!("invalid operation")] {
            let mut update = ClaudeUpdate::default();
            state.receive_named_claude(method, &json!({"operation":identity,"generation":2,"sessionId":"foreign","message":"old failure"}), &mut update);
            assert!(update.effects.is_empty());
            assert_eq!(state.thread_id(), "root");
            assert_eq!(state.claude_edit_prompt_for_test(), Some("new edit"));
        }
    }
}

#[test]
fn a_forks_early_helper_switch_cannot_make_its_write_ack_restart_the_edit() {
    let (mut state, work) = pending();
    state.switch_claude_session(2, "new-native", &mut ClaudeUpdate::default());
    let completion = BackendCompletion {
        guard: work.guard,
        finish: work.finish,
        result: Ok(json!({"result":{"claudeForkPending":true}})),
    };
    let prepared = state
        .finish_backend_work(completion)
        .expect("own async acknowledgement");
    assert!(matches!(prepared.status, PreparedStatus::Awaiting));
    assert!(prepared.work.is_none());
    assert_eq!(state.thread_id(), "new-native");
    assert_eq!(state.claude_edit_prompt_for_test(), Some("edited"));
}

#[test]
fn admitting_an_async_fork_does_not_claim_the_branch_was_adopted() {
    let (mut state, work) = pending();
    let prepared = state
        .finish_backend_work(BackendCompletion {
            guard: work.guard,
            finish: work.finish,
            result: Ok(json!({"result":{"claudeForkPending":true}})),
        })
        .expect("admitted asynchronous request");
    assert!(matches!(prepared.status, PreparedStatus::Awaiting));
    assert!(!prepared
        .update
        .changes
        .effects
        .iter()
        .any(|effect| { matches!(effect, ClaudeEffect::EditAdopted(_)) }));
    let failure = state
        .receive_claude_message(
            "claude/forkFailed",
            &json!({"operation":operation(1),"message":"saved session disappeared"}),
        )
        .expect("matching failure");
    assert!(!failure
        .effects
        .iter()
        .any(|effect| matches!(effect, ClaudeEffect::EditAdopted(_))));
    assert_eq!(state.thread_id(), "root");
}

#[test]
fn only_the_matching_new_helper_switch_reports_adoption_for_its_edit() {
    let (mut state, _work) = pending();
    for facts in [
        json!({"operation":operation(2),"generation":2,"sessionId":"foreign"}),
        json!({"operation":operation(1),"generation":0,"sessionId":"old"}),
    ] {
        let mut update = ClaudeUpdate::default();
        state.receive_named_claude("claude/sessionChanged", &facts, &mut update);
        assert!(update.effects.is_empty());
    }
    let mut update = ClaudeUpdate::default();
    state.receive_named_claude(
        "claude/sessionChanged",
        &json!({"operation":operation(1),"generation":2,"sessionId":"adopted"}),
        &mut update,
    );
    assert_eq!(state.thread_id(), "adopted");
    assert!(update
        .effects
        .iter()
        .any(|effect| matches!(effect, ClaudeEffect::EditAdopted(id) if id == &operation(1))));
}
