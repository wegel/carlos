//! Preserve exact approval and fork/send requests through core's typed prepare/finish boundaries.

use std::sync::mpsc;
use std::time::{Duration, Instant};

use carlos_proto::commands::ApprovalAnswer;
use carlos_proto::session::Decision;
use carlos_proto::submissions::{ClientOperation, OperationState};
use serde_json::json;

use super::approval_control_test_support::Peer;
use super::approval_state::{ApprovalRequestDraft, ApprovalRequestKind};
use super::input_contract_test_support::rewind_peer;
use super::models::Role;
use super::prompt_commands::PromptContext;
use super::ralph::RalphConfig;
use super::state::SessionState;
use crate::backend::{BackendKind, RewindForkRequest};
use crate::engine::{EngineCommand, EngineHandle, EngineReply};

#[test]
fn pending_claude_exit_plan_accept_uses_approval_path_not_chat_input() {
    let peer = Peer::new(BackendKind::Claude);
    let mut session = SessionState::new("thread-1".into(), Default::default(), false);
    let request = json!({"backend":"claude","kind":"exitPlanMode","toolUseId":"toolu_1"});
    let retired = session.set_pending_approval(ApprovalRequestDraft {
        request_id: request.clone(),
        method: "claude/exitPlan/requestApproval".into(),
        kind: ApprovalRequestKind::ClaudeExitPlanMode,
        title: "Approve Claude plan".into(),
        detail_lines: vec!["Do the work".into()],
        requested_permissions: None,
        can_accept_for_session: false,
        can_decline: true,
        can_cancel: false,
    });
    assert!(retired.is_empty());
    let id = session
        .approval()
        .pending()
        .expect("offered plan")
        .entry_id();
    let work = session
        .prepare_engine_answer(
            &peer,
            id,
            ApprovalAnswer::Decision {
                decision: Decision::Once,
            },
        )
        .expect("typed native answer");
    session
        .finish_engine_control(&peer, work.execute(&peer.commands))
        .expect("accepted original answer");
    assert!(session.approval().pending().is_none());
    assert!(session.messages().is_empty());
    assert_eq!(session.input_history().history_len(), 0);
    assert_eq!(
        *peer.calls.lock().expect("native answers"),
        [(request, json!({"decision":"accept"}))]
    );
}

#[test]
fn submit_rewind_turn_text_forks_codex_thread_before_resubmitting() {
    let (commands, writes) = rewind_peer();
    let mut session = SessionState::new("thread-original".into(), Default::default(), false);
    let first = session.append_message(Role::User, "first");
    session.record_prompt_history("first", Some(session.messages()[first].id));
    session.append_message(Role::Assistant, "reply one");
    let second = session.append_message(Role::User, "second");
    session.record_prompt_history("second", Some(session.messages()[second].id));
    session.append_message(Role::Assistant, "reply two");
    let selected = session
        .input_history()
        .prompt_id(1)
        .expect("selected owner prompt");
    let mut prepared = session
        .prepare_rewind_text(
            BackendKind::Codex,
            selected,
            "edited second".into(),
            &PromptContext {
                operation: None,
                claude_queue: false,
            },
            false,
        )
        .expect("prepare fork");
    while let Some(work) = prepared.work {
        prepared = session
            .finish_backend_work(work.execute(&commands))
            .expect("original completion");
    }
    assert_eq!(session.thread_id(), "thread-fork");
    assert_eq!(session.messages().len(), 2);
    assert_eq!(session.messages()[0].text, "first");
    assert_eq!(session.messages()[1].text, "reply one");
    let indices: Vec<_> = (0..session.input_history().history_len())
        .map(|index| {
            session
                .input_history()
                .entry_id(index)
                .and_then(|id| session.messages().position(id))
        })
        .collect();
    assert_eq!(indices, [Some(0), None]);
    let writes = writes.lock().expect("native writes");
    assert_eq!(
        writes.forks,
        [(
            "thread-original".into(),
            RewindForkRequest {
                keep_turns: 1,
                drop_turns: 1,
                before_turn_id: None,
                before_user_item: None
            }
        )]
    );
    assert_eq!(writes.starts.len(), 1);
    assert_eq!(writes.starts[0]["threadId"], "thread-fork");
    assert_eq!(writes.starts[0]["input"][0]["text"], "edited second");
}

#[test]
fn typed_ralph_stop_discards_a_pending_continuation_without_native_work() {
    let mut session = SessionState::new("thread-1".into(), Default::default(), false);
    session.enable_ralph_mode(RalphConfig {
        prompt_path: ".agents/ralph-prompt.md".into(),
        base_prompt: "base".into(),
        done_marker: "@@COMPLETE@@".into(),
        blocked_marker: "@@BLOCKED@@".into(),
        waiting_marker: "@@AWAITING_HUMAN@@".into(),
        continuation_prompt: "continue".into(),
    });
    session.queue_ralph_continuation("continue");
    // A captured current turn keeps the continuation pending independently of elapsed wall time.
    session.set_active_turn_id(Some("held-native-turn".into()));
    let before = session.publish().snapshot(1);
    assert!(before.info.ralph);
    assert!(
        before
            .info
            .status
            .expect("activity facts")
            .ralph
            .continuation_pending
    );
    let peer = Peer::new(BackendKind::Claude);
    let calls = peer.calls.clone();
    let (_native, events) = mpsc::channel();
    let (owner, _notices) = EngineHandle::spawn(session, Box::new(peer), events.into());
    let operation = ClientOperation {
        client: "a".repeat(32).parse().expect("client"),
        id: "1".parse().expect("operation"),
    };
    let result = request(
        &owner,
        EngineCommand::SetRalph {
            operation: operation.clone(),
            on: false,
            options: None,
        },
    );
    let accepted = wait_stop(&owner, &operation);
    let EngineReply::Snapshot(after) = request(&owner, EngineCommand::Snapshot { limit: 1 }) else {
        panic!("snapshot")
    };
    owner.shutdown();
    owner.join().expect("owner stops");
    assert!(matches!(result, EngineReply::Operation(_)));
    assert!(accepted);
    assert!(!after.info.ralph);
    assert!(
        !after
            .info
            .status
            .expect("activity facts")
            .ralph
            .continuation_pending
    );
    assert!(calls.lock().expect("native calls").is_empty());
}

fn request(owner: &EngineHandle, command: EngineCommand) -> EngineReply {
    owner
        .request(command)
        .expect("bounded admission")
        .recv_timeout(Duration::from_secs(5))
        .expect("responsive owner")
}

fn wait_stop(owner: &EngineHandle, operation: &ClientOperation) -> bool {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let EngineReply::Outcomes(results) = request(owner, EngineCommand::Outcomes) else {
            panic!("private outcomes")
        };
        if let Some(result) = results.iter().find(|result| {
            &result.operation == operation
                && !matches!(
                    result.state,
                    OperationState::Pending | OperationState::Queued
                )
        }) {
            return matches!(result.state, OperationState::Accepted { .. });
        }
        assert!(Instant::now() < deadline, "original Ralph stop settles");
        std::thread::yield_now();
    }
}
