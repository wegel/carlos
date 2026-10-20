//! Regression tests for terminal turns, repeated starts, and Claude interrupts.

use serde_json::json;

use super::{handle_notification_line, AppState, Role};
use crate::app::models::TurnStatus;
use crate::claude_backend::claude_interrupted_turn_notification;

#[test]
fn replayed_start_cannot_reopen_a_terminal_turn() {
    for terminal_status in ["completed", "interrupted", "failed"] {
        let mut app = AppState::new("thread-root".to_string());
        notify_turn(&mut app, "turn/started", "turn-done", "inProgress");
        notify_turn(&mut app, "turn/completed", "turn-done", terminal_status);
        let completed_status = app.status.clone();

        notify_turn(&mut app, "turn/started", "turn-done", "inProgress");

        assert_eq!(app.active_turn_id, None);
        assert_eq!(app.status, completed_status);
        assert!(app.turn_status_by_id["turn-done"].is_terminal());
    }
}

#[test]
fn replayed_old_start_cannot_replace_a_newer_active_turn() {
    let mut app = AppState::new("thread-root".to_string());
    notify_turn(&mut app, "turn/completed", "turn-old", "completed");
    notify_turn(&mut app, "turn/started", "turn-new", "inProgress");

    notify_turn(&mut app, "turn/started", "turn-old", "inProgress");

    assert_eq!(app.active_turn_id.as_deref(), Some("turn-new"));
    assert_eq!(app.turn_status_by_id["turn-old"], TurnStatus::Completed);
}

#[test]
fn repeated_start_preserves_the_current_turn_message_boundary() {
    let mut app = AppState::new("thread-root".to_string());
    notify_turn(&mut app, "turn/started", "turn-live", "inProgress");
    app.append_message(Role::Assistant, "already streamed");

    notify_turn(&mut app, "turn/started", "turn-live", "inProgress");

    assert_eq!(app.turn_start_message_idx, Some(0));
}

#[test]
fn completion_with_in_progress_status_cannot_end_a_turn() {
    let mut app = AppState::new("thread-root".to_string());
    notify_turn(&mut app, "turn/started", "turn-live", "inProgress");

    notify_turn(&mut app, "turn/completed", "turn-live", "inProgress");

    assert_eq!(app.active_turn_id.as_deref(), Some("turn-live"));
}

#[test]
fn claude_interrupt_ends_the_requested_turn_once() {
    for turn_id in ["claude-stream-1-turn-1", "claude-turn-pending"] {
        let mut app = AppState::new("thread-root".to_string());
        notify_turn(&mut app, "turn/started", turn_id, "inProgress");
        let completion = claude_interrupted_turn_notification(&json!({
            "threadId": "thread-root", "turnId": turn_id
        }))
        .expect("build Claude interrupt completion");

        handle_notification_line(&mut app, &completion);
        handle_notification_line(&mut app, &completion);

        assert_eq!(app.active_turn_id, None);
        assert_eq!(app.status, "turn interrupted");
        assert_eq!(app.messages.len(), 1);
    }
}

#[test]
fn late_claude_interrupt_cannot_end_a_newer_turn() {
    let mut app = AppState::new("thread-root".to_string());
    notify_turn(&mut app, "turn/started", "claude-turn-new", "inProgress");
    let completion = claude_interrupted_turn_notification(&json!({
        "threadId": "thread-root", "turnId": "claude-turn-old"
    }))
    .expect("build delayed Claude interrupt completion");

    handle_notification_line(&mut app, &completion);

    assert_eq!(app.active_turn_id.as_deref(), Some("claude-turn-new"));
}

#[test]
fn claude_interrupt_requires_thread_and_turn_ids() {
    for params in [
        json!({}),
        json!({"threadId": "thread-root"}),
        json!({"turnId": "turn-live"}),
    ] {
        assert!(claude_interrupted_turn_notification(&params).is_err());
    }
}

fn notify_turn(app: &mut AppState, method: &str, turn_id: &str, status: &str) {
    let line = json!({
        "method": method,
        "params": {"threadId": "thread-root", "turn": {"id": turn_id, "status": status}}
    })
    .to_string();
    handle_notification_line(app, &line);
}
