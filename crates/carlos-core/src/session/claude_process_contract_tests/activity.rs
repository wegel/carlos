//! Preserve native activity fields, compaction attempt identity and context read fencing.

use std::time::Instant;

use serde_json::{json, Value};

use super::{app, fact};
use crate::session::claude::{NativeState, StopState};

#[test]
fn activity_defaults_and_label_precedence_leave_context_revision_unchanged() {
    for (state, expected) in [
        (json!("running"), NativeState::Running),
        (json!("requires_action"), NativeState::RequiresAction),
        (json!("future"), NativeState::Idle),
        (Value::Null, NativeState::Idle),
    ] {
        let mut app = app();
        fact(&mut app, "claude/thinking", json!({"estimated_tokens":42}));
        fact(&mut app, "claude/status", json!({"label":"Temporary"}));
        assert_eq!(app.claude().activity().transient(), Some("Temporary"));
        assert_eq!(app.claude().activity().thinking(), Some(42));
        fact(&mut app, "claude/status", json!({}));
        assert!(app.claude().activity().transient().is_none());
        assert_eq!(app.claude().activity().thinking(), Some(42));
        fact(&mut app, "claude/activity", json!({"state":state}));
        assert_eq!(app.claude().activity().native_state(), expected);
        if expected == NativeState::Running {
            fact(
                &mut app,
                "claude/thinking",
                json!({"estimated_tokens":"invalid"}),
            );
        }
        assert!(app.claude().activity().thinking().is_none());
        assert_eq!(app.claude().activity().context_revision(), 0);
    }
}

#[test]
fn compaction_reuses_attempts_until_a_new_boundary_or_start_after_any_result() {
    let mut app = app();
    app.set_claude_generation_for_test(7);
    let messages = [
        (json!({"status":"compacting"}), 1, Some("Compacting")),
        (json!({"status":"compacting"}), 1, Some("Compacting")),
        (json!({"compact_result":"future"}), 1, None),
        (json!({"status":"compacting"}), 2, Some("Compacting")),
        (json!({"subtype":"compact_boundary","uuid":"x"}), 2, None),
        (json!({"subtype":"compact_boundary","uuid":"x"}), 2, None),
        (json!({"subtype":"compact_boundary"}), 3, None),
        (
            json!({"subtype":"compact_boundary","uuid":"y","compact_result":"failed"}),
            3,
            None,
        ),
    ];
    for (index, (message, rows, transient)) in messages.into_iter().enumerate() {
        app.receive_claude_compaction(&message);
        assert_eq!(app.messages().len(), rows);
        assert_eq!(app.claude().activity().context_revision(), index as u64 + 1);
        assert_eq!(app.claude().activity().transient(), transient);
    }
    assert!(app.has_item_mapping(&app.backend_entry_key("compact:7:1")));
    assert!(app.has_item_mapping(&app.backend_entry_key("compact:7:2")));
    assert!(app.has_item_mapping(&app.backend_entry_key("compact:7:3")));
    assert_eq!(
        app.messages()[2].text,
        "Claude could not compact context: unknown error"
    );
}

#[test]
fn observing_a_changed_model_fences_the_read_before_tick_without_duplicate_bumps() {
    let mut app = app();
    app.observe_claude_model("old");
    let request = app
        .prepare_claude_context(Instant::now())
        .1
        .expect("first native read");
    app.observe_claude_model("new");
    assert_eq!(app.claude().activity().context_revision(), 2);
    assert_eq!(app.claude().observed_model(), Some("new"));
    app.receive_claude_context(
        &json!({"ok":true,"result":{"totalTokens":99,"maxTokens":100}}),
        &request,
    );
    assert!(app.context_usage().is_none());
    app.observe_claude_model("new");
    assert_eq!(app.claude().activity().context_revision(), 2);
    let next = app
        .prepare_claude_context(Instant::now())
        .1
        .expect("fresh native read");
    assert_eq!(next, json!({"revision":2,"contextToken":2}));
}

#[test]
fn clear_retains_process_work_but_failure_only_clears_activity_and_live_membership() {
    let mut app = app();
    app.update_claude_task(
        &json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"task","run_id":"a"}]}),
    )
    .expect("level");
    app.begin_claude_task_stop_for_test("task");
    fact(&mut app, "claude/activity", json!({"state":"running"}));
    app.receive_claude_reset(
        &json!({"new_conversation_id":"clear"}),
        &mut Default::default(),
    );
    assert_eq!(app.claude().tasks().active_count(), 1);
    assert_eq!(app.claude().activity().native_state(), NativeState::Running);
    assert_eq!(app.claude().activity().context_revision(), 1);
    assert!(matches!(
        app.claude().tasks().stops().state("task", Some("a")),
        Some(StopState::Sending)
    ));
    app.fail_claude(&mut Default::default());
    assert_eq!(app.claude().tasks().active_count(), 0);
    assert!(app.claude().tasks().data("task").is_some());
    assert!(matches!(
        app.claude().tasks().stops().state("task", Some("a")),
        Some(StopState::Sending)
    ));
    assert_eq!(app.claude().activity().context_revision(), 0);
}
