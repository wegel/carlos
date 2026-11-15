//! Retire old controls when a new owner reuses the same logical and native session addresses.

use carlos_proto::session::Backend;
use serde_json::json;

use crate::session::models::Role;
use crate::session::state::SessionState;

#[test]
fn replacement_owner_retires_equal_codex_settings_goal_and_queue_references() {
    let mut before = codex();
    let replacement = codex();
    assert_eq!(before.session_id(), replacement.session_id());
    assert_eq!(before.thread_id(), replacement.thread_id());
    assert_ne!(before.settings_revision(), replacement.settings_revision());
    assert_ne!(before.goal_revision(), replacement.goal_revision());
    assert_ne!(before.queue_revision(), replacement.queue_revision());
    let old_entry = before.queue_entry_id(&before.codex_queue.entries[0]);
    assert!(replacement.queue_entry(&old_entry).is_none());
    assert_ne!(
        old_entry,
        replacement.queue_entry_id(&replacement.codex_queue.entries[0])
    );
    let revision = before.queue_revision();
    before.publish();
    before.append_message(Role::Assistant, "unrelated root text");
    before.publish();
    assert_eq!(
        before.queue_revision(),
        revision,
        "the same owner keeps its scope"
    );
}

#[test]
fn replacement_helper_owner_cannot_reuse_a_prior_task_or_run_identity() {
    let before = claude();
    let replacement = claude();
    let old = before.task_page(None, None, 1).expect("old task");
    let current = replacement
        .task_page(None, None, 1)
        .expect("same native task");
    assert_eq!(before.session_id(), replacement.session_id());
    assert_eq!(old.tasks[0].label, current.tasks[0].label);
    assert_ne!(old.tasks[0].id, current.tasks[0].id);
    assert_ne!(old.tasks[0].run, current.tasks[0].run);
    assert!(replacement
        .task_detail(&current.revision, &old.tasks[0].id)
        .is_err());
}

fn base(backend: Backend) -> SessionState {
    let mut state = SessionState::new("same-native".into(), Default::default(), false);
    state.events.id = "a".repeat(32).parse().expect("same logical session");
    state.set_client_identity(backend, "host".into());
    state
}

fn codex() -> SessionState {
    let mut state = base(Backend::Codex);
    state.enable_codex();
    state.configure_codex_connection(1, None);
    state.codex_queue.entries =
        vec![json!({"id":"same-native-queue-entry","input":[{"type":"text","text":"same text"}]})];
    state
}

fn claude() -> SessionState {
    let mut state = base(Backend::Claude);
    state.enable_claude();
    state.set_claude_generation_for_test(1);
    state
        .update_claude_task(
            &json!({"subtype":"task_started","task_id":"same-task","run_id":"same-run"}),
        )
        .expect("native task");
    state
}
