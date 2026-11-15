//! Exercise Grok follow-up, queue, echo and fork rules through core's captured command work.

use std::time::Instant;

use super::command_work::PreparedCommand;
use super::input_contract_test_support::grok_peer;
use super::models::Role;
use super::prompt_commands::{PromptContext, PromptOptions};
use super::state::SessionState;
use crate::backend::{BackendKind, PromptOrigin, RewindForkRequest};
use crate::backend_commands::CommandHandle;

fn session(root: &str) -> SessionState {
    SessionState::new(root.into(), Default::default(), false)
}

fn submit(
    state: &mut SessionState,
    commands: &CommandHandle,
    text: &str,
    force: bool,
    record: bool,
) {
    let prepared = state
        .prepare_turn_text(
            BackendKind::Grok,
            text.into(),
            PromptOptions {
                record_history: record,
                force_start: force,
                origin: PromptOrigin::Human,
            },
            &PromptContext {
                operation: None,
                claude_queue: false,
            },
        )
        .expect("prepare Grok send");
    drive(state, commands, prepared);
}

fn drive(state: &mut SessionState, commands: &CommandHandle, mut prepared: PreparedCommand) {
    while let Some(work) = prepared.work {
        prepared = state
            .finish_backend_work(work.execute(commands))
            .expect("settle original Grok work");
    }
}

#[test]
fn submit_turn_text_sends_grok_follow_up_while_turn_active() {
    let (commands, writes) = grok_peer();
    let mut state = session("thread-1");
    state.set_active_turn_id(Some("turn-1".into()));
    submit(&mut state, &commands, "follow up", false, true);
    assert_eq!(writes.lock().expect("native writes").starts.len(), 1);
    assert_eq!(state.messages().len(), 1);
    assert_eq!(state.messages()[0].text, "follow up");
    assert_eq!(state.active_turn_id().as_deref(), Some("turn-1"));
    assert_eq!(state.status(), "sent turn");
}

#[test]
fn submit_turn_text_echoes_grok_prompt_and_marks_pending_turn() {
    let (commands, writes) = grok_peer();
    let mut state = session("thread-1");
    submit(&mut state, &commands, "hello grok", false, true);
    assert_eq!(writes.lock().expect("native writes").starts.len(), 1);
    assert_eq!(state.messages().len(), 1);
    assert_eq!(state.messages()[0].role, Role::User);
    assert_eq!(state.messages()[0].text, "hello grok");
    assert_eq!(state.active_turn_id().as_deref(), Some("grok-turn-pending"));
}

#[test]
fn submit_turn_text_queues_behind_existing_grok_turns() {
    let (commands, writes) = grok_peer();
    let mut state = session("thread-1");
    state.queue_turn_input("first");
    submit(&mut state, &commands, "second", false, true);
    assert!(writes.lock().expect("native writes").starts.is_empty());
    assert_eq!(state.status(), "queued behind pending Grok turn");
}

#[test]
fn submit_rewind_turn_text_keeps_prefix_and_resubmits() {
    let (commands, writes) = grok_peer();
    let mut state = session("thread-original");
    let first = state.append_message(Role::User, "first");
    state.record_prompt_history("first", Some(state.messages()[first].id));
    state.append_message(Role::Assistant, "reply one");
    let second = state.append_message(Role::User, "second");
    state.record_prompt_history("second", Some(state.messages()[second].id));
    state.append_message(Role::Assistant, "reply two");
    let selected = state
        .input_history()
        .prompt_id(1)
        .expect("selected owner prompt");
    let prepared = state
        .prepare_rewind_text(
            BackendKind::Grok,
            selected,
            "edited second".into(),
            &PromptContext {
                operation: None,
                claude_queue: false,
            },
            false,
        )
        .expect("prepare Grok fork");
    drive(&mut state, &commands, prepared);
    assert_eq!(state.thread_id(), "grok-fork");
    assert_eq!(state.messages().len(), 3);
    assert_eq!(state.messages()[0].text, "first");
    assert_eq!(state.messages()[1].text, "reply one");
    assert_eq!(state.messages()[2].text, "edited second");
    let writes = writes.lock().expect("native writes");
    assert_eq!(
        writes
            .forks
            .iter()
            .map(|(_, request)| request)
            .collect::<Vec<_>>(),
        [&RewindForkRequest {
            keep_turns: 1,
            drop_turns: 1,
            before_turn_id: None,
            before_user_item: None,
        }]
    );
    assert_eq!(writes.starts.len(), 1);
    assert_eq!(writes.starts[0]["input"][0]["text"], "edited second");
}

#[test]
fn submit_dequeued_grok_turn_starts_even_when_more_are_queued() {
    let (commands, writes) = grok_peer();
    let mut state = session("thread-1");
    state.queue_turn_input("second");
    submit(&mut state, &commands, "first", true, false);
    assert_eq!(writes.lock().expect("native writes").starts.len(), 1);
    assert_eq!(state.messages()[0].text, "first");
    assert_eq!(state.active_turn_id().as_deref(), Some("grok-turn-pending"));
    assert_eq!(
        state
            .dequeue_turn_input(Instant::now())
            .expect("remaining")
            .text,
        "second"
    );
}

#[test]
fn submit_turn_text_with_history_false_skips_grok_history() {
    let (commands, _) = grok_peer();
    let mut state = session("thread-1");
    submit(&mut state, &commands, "internal", false, false);
    assert_eq!(state.messages().len(), 1);
    assert_eq!(state.input_history().history_len(), 0);
}
