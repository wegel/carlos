//! Check native task cache bounds and the stop error dismissed by local selection.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde_json::json;

use super::state::AppState;

#[path = "claude_process_support.rs"]
mod support;
use support::Peer;

#[test]
fn detail_bound_evicts_terminal_rows_before_older_running_rows() {
    let mut app = AppState::new("root".into());
    app.session
        .update_claude_task(&json!({"subtype":"task_started","task_id":"old-running"}))
        .expect("old work");
    app.session
        .update_claude_task(
            &json!({"subtype":"task_notification","task_id":"terminal","status":"completed"}),
        )
        .expect("finished work");
    for index in 0..255 {
        app.session
            .update_claude_task(
                &json!({"subtype":"task_started","task_id":format!("task-{index}")}),
            )
            .expect("detail churn");
    }
    assert_eq!(app.session.claude().tasks().records().len(), 256);
    assert!(app.session.claude().tasks().data("terminal").is_none());
    assert!(app.session.claude().tasks().data("old-running").is_some());
    app.session
        .update_claude_task(&json!({"subtype":"task_started","task_id":"newest"}))
        .expect("one more row");
    assert!(app.session.claude().tasks().data("old-running").is_none());
    assert!(app.session.claude().tasks().data("newest").is_some());
}

#[test]
fn an_oversized_detail_is_evicted_without_changing_live_membership() {
    let mut app = AppState::new("root".into());
    app.session.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"live","run_id":"a"}]})).expect("live work");
    app.session.update_claude_task(&json!({"subtype":"task_started","task_id":"oversized","description":"x".repeat(8*1024*1024)})).expect("oversized edge");
    assert_eq!(app.session.claude().tasks().records().len(), 0);
    assert_eq!(app.session.claude().tasks().ids(), ["live"]);
    assert_eq!(app.session.claude().tasks().active_count(), 1);
    assert_eq!(
        app.session
            .claude()
            .tasks()
            .data("live")
            .expect("level fallback")["run_id"],
        "a"
    );
}

#[test]
fn full_stop_registry_sets_one_control_error_until_the_user_selects_a_target() {
    let mut app = AppState::new("root".into());
    let tasks: Vec<_> = (0..4096)
        .map(|index| json!({"task_id":format!("task-{index}"),"run_id":"a"}))
        .collect();
    app.session
        .update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":tasks}))
        .expect("full live set");
    for index in 0..4096 {
        assert_eq!(
            app.session
                .begin_claude_task_stop_for_test(&format!("task-{index}")),
            Some(Some("a".into()))
        );
    }
    app.session
        .update_claude_task(&json!({"subtype":"task_started","task_id":"overflow","run_id":"a"}))
        .expect("foreground work");
    let peer = Peer::default();
    app.session.stop_claude_task(&peer, "overflow");
    assert_eq!(
        app.session.claude().tasks().control_error(),
        Some("Too many unanswered native task stops; wait for native task events")
    );
    assert!(peer.controls.borrow().is_empty());
    app.claude.tasks.open(app.session.claude().tasks());
    super::claude_task_view::key(
        &peer,
        &mut app,
        KeyEvent::new(KeyCode::Down, KeyModifiers::NONE),
    );
    assert!(app.session.claude().tasks().control_error().is_none());
    assert_eq!(app.claude.tasks.scroll, 0);
}
