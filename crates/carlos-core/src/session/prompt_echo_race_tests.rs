//! Bind an edited Codex prompt exactly once when its native echo races the worker result.

use serde_json::json;

use super::backend_test_support::context;
use super::input_contract_test_support::rewind_peer;
use super::prompt_commands::{PromptContext, PromptOptions};
use super::state::SessionState;
use crate::backend::{BackendKind, PromptOrigin};

fn echo(session: &mut SessionState) {
    for frame in [
        json!({"method":"turn/started","params":{"threadId":"root","turn":{"id":"native-turn","status":"inProgress"}}}),
        json!({"method":"item/completed","params":{"threadId":"root","turnId":"native-turn","item":{
            "id":"native-user","type":"userMessage","content":[{"type":"text","text":"edited second"}]}}}),
    ] {
        session.handle_backend_message(&frame, &context(session));
    }
}

#[test]
fn an_edited_prompt_keeps_one_bound_history_record_in_both_native_reply_orders() {
    for (echo_first, previous_same_text) in
        [(true, false), (false, false), (true, true), (false, true)]
    {
        let mut session = SessionState::new("root".into(), Default::default(), false);
        session.enable_codex();
        session.enable_native_history();
        if previous_same_text {
            let previous = session.append_message(super::models::Role::User, "edited second");
            let row = session.messages()[previous].id;
            session.record_prompt_history("edited second", Some(row));
            session.record_prompt_turn(row, "previous-turn");
        }
        let (commands, writes) = rewind_peer();
        let prepared = session
            .prepare_prompt(
                BackendKind::Codex,
                "edited second".into(),
                PromptOptions {
                    record_history: true,
                    force_start: true,
                    origin: PromptOrigin::Human,
                },
                &PromptContext {
                    operation: None,
                    claude_queue: false,
                },
                true,
            )
            .expect("edited send after checked fork");
        let completion = prepared
            .work
            .expect("guarded prompt write")
            .execute(&commands);
        if echo_first {
            echo(&mut session);
        }
        session
            .finish_backend_work(completion)
            .expect("original command settles");
        if !echo_first {
            echo(&mut session);
        }
        assert_bound_history(&session, previous_same_text);
        assert_eq!(writes.lock().expect("one native send").starts.len(), 1);
    }
}

fn assert_bound_history(session: &SessionState, previous: bool) {
    let history = session.input_history();
    let selected = usize::from(previous);
    assert_eq!(history.history_len(), selected + 1);
    let row = history
        .entry_id(selected)
        .expect("native echo binds its prompt");
    assert_eq!(history.turn_id(row), Some("native-turn"));
    assert_eq!(history.text(selected), "edited second");
    if previous {
        let earlier = history
            .entry_id(0)
            .expect("prior identical prompt stays bound");
        assert_ne!(earlier, row);
        assert_eq!(history.turn_id(earlier), Some("previous-turn"));
    }
    assert_eq!(
        session
            .messages()
            .iter()
            .filter(|row| row.text == "edited second")
            .count(),
        selected + 1
    );
}

#[test]
fn a_failed_edited_send_retires_its_unbound_record_and_recovers_the_original_text() {
    let mut session = SessionState::new("root".into(), Default::default(), false);
    session.enable_codex();
    session.enable_native_history();
    let (commands, writes) = rewind_peer();
    let commands = commands
        .guarded(crate::dispatch_guard::DispatchGuard::new(|| false))
        .expect("captured retired writer");
    let prepared = session
        .prepare_prompt(
            BackendKind::Codex,
            "edited second".into(),
            PromptOptions {
                record_history: true,
                force_start: true,
                origin: PromptOrigin::Human,
            },
            &PromptContext {
                operation: None,
                claude_queue: false,
            },
            true,
        )
        .expect("checked fork already adopted");
    let completed = prepared
        .work
        .expect("original prompt attempt")
        .execute(&commands);
    let finished = session
        .finish_backend_work(completed)
        .expect("failed write settles");
    assert_eq!(session.input_history().history_len(), 0);
    assert!(writes.lock().expect("native trace").starts.is_empty());
    assert!(finished.update.changes.effects.iter().any(|effect| matches!(effect,
        super::claude::receive_update::ClaudeEffect::Submission(carlos_proto::submissions::SubmissionOutcome::Recovered { draft, .. }) if draft.text == "edited second")));
}
