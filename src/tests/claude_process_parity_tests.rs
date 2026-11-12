//! Enumerate task guards, process resets, and compaction notice identity against the old app.

use std::time::Instant;

use serde_json::{json, Value};

use super::claude_context;
use super::claude_notifications::fail_session;
#[path = "claude_process_support.rs"]
mod support;
use super::notifications::handle_server_message_line;
use super::state::AppState;
use crate::session::claude::{NativeState, StopState};
use support::Peer;

fn app() -> AppState {
    let mut app = AppState::new("root".into());
    app.session.set_claude_enabled_for_test(true);
    app
}

fn fact(app: &mut AppState, method: &str, params: Value) {
    let _ = handle_server_message_line(app, &json!({"method":method,"params":params}).to_string());
}

#[test]
fn task_identity_and_membership_errors_keep_their_exact_defaults() {
    let cases = [
        (json!({}), Ok(None)),
        (json!({"task_id":7}), Ok(None)),
        (json!({"task_id":"x".repeat(4097)}), Err("Native task identity exceeds the bounded task view")),
        (json!({"task_id":"x","run_id":"x".repeat(4097)}), Err("Native task identity exceeds the bounded task view")),
        (json!({"subtype":"background_tasks_changed"}), Err("Native task membership has no task list")),
        (json!({"subtype":"background_tasks_changed","tasks":vec![Value::Null;4097]}), Err("Native task membership exceeds 4096 tasks; stop the session to keep task controls bounded")),
    ];
    for (message, expected) in cases {
        let mut app = app();
        assert_eq!(app.session.update_claude_task(&message), expected);
        assert!(app.session.claude().tasks().ids().is_empty());
    }
}

#[test]
fn malformed_membership_retains_earlier_detail_writes_and_the_prior_live_set() {
    let mut app = app();
    app.session
        .update_claude_task(&json!({"subtype":"background_tasks_changed",
        "tasks":[{"task_id":"old","run_id":"a"}]}))
        .expect("old level");
    assert_eq!(
        app.session
            .update_claude_task(&json!({"subtype":"background_tasks_changed",
        "tasks":[{}, {"task_id":"new","run_id":"b"}, {"task_id":"x".repeat(4097)}]})),
        Err("Native task identity exceeds the bounded task view")
    );
    assert_eq!(
        app.session
            .claude()
            .tasks()
            .live()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["old"]
    );
    assert_eq!(
        app.session
            .claude()
            .tasks()
            .data("new")
            .expect("earlier detail")["run_id"],
        "b"
    );
}

#[test]
fn task_runs_compare_utf16_and_missing_runs_never_replace_a_known_run() {
    for (first, second, expected) in [
        ("\u{e000}", "\u{10000}", "first"),
        ("\u{10000}", "\u{e000}", "second"),
    ] {
        let mut app = app();
        for (run, description) in [(first, "first"), (second, "second")] {
            app.session
                .update_claude_task(&json!({"subtype":"task_started", "task_id":"task",
                "run_id":run,"description":description}))
                .expect("run edge");
        }
        app.session
            .update_claude_task(&json!({"subtype":"task_started","task_id":"task",
            "description":"missing"}))
            .expect("ignored missing run");
        assert_eq!(
            app.session.claude().tasks().data("task").expect("task")["description"],
            expected
        );
    }
}

#[test]
fn terminal_edges_accept_one_report_and_unknown_states_keep_the_prior_status() {
    let mut app = app();
    for (message, expected) in [
        (json!({"subtype":"task_started"}), "running"),
        (
            json!({"subtype":"task_updated","patch":{"status":"future"}}),
            "running",
        ),
        (
            json!({"subtype":"task_updated","patch":{"status":"killed"}}),
            "stopped",
        ),
        (
            json!({"subtype":"task_notification","status":"completed","summary":"first"}),
            "stopped",
        ),
        (
            json!({"subtype":"task_notification","status":"failed","summary":"second"}),
            "stopped",
        ),
    ] {
        let mut message = message;
        message["task_id"] = json!("task");
        message["run_id"] = json!("a");
        app.session.update_claude_task(&message).expect("edge");
        let record = &app.session.claude().tasks().records()["task"];
        assert_eq!(super::claude_tasks::status_label(record.status()), expected);
    }
    assert_eq!(
        app.session
            .claude()
            .tasks()
            .data("task")
            .expect("final report")["summary"],
        "first"
    );
}

#[test]
fn only_exact_terminal_runs_and_ambient_tasks_are_excluded_from_live_activity() {
    for (task, edge, expected) in [
        (json!({"ambient":true}), json!({}), 0),
        (json!({"skip_transcript":true}), json!({}), 0),
        (json!({}), json!({"ambient":true}), 0),
        (json!({}), json!({"skip_transcript":true}), 0),
        (json!({}), json!({"status":"completed"}), 0),
        (json!({}), json!({"status":"completed","run_id":"b"}), 1),
        (json!({}), json!({"status":"future"}), 1),
    ] {
        let mut app = app();
        let mut task = task;
        task["task_id"] = json!("task");
        task["run_id"] = json!("a");
        app.session
            .update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[task]}))
            .expect("level");
        let mut message = json!({"subtype":"task_notification","task_id":"task","run_id":"a"});
        message
            .as_object_mut()
            .expect("object")
            .extend(edge.as_object().expect("edge").clone());
        app.session.update_claude_task(&message).expect("edge");
        assert_eq!(app.session.claude().tasks().active_count(), expected);
    }
}

#[test]
fn stops_send_once_and_ignore_replies_for_old_or_terminal_runs() {
    let mut app = app();
    let peer = Peer::default();
    app.session.stop_claude_task(&peer, "unknown");
    for run in ["a", "b"] {
        app.session
            .update_claude_task(&json!({"subtype":"task_started","task_id":"task","run_id":run}))
            .expect("start");
        app.session.stop_claude_task(&peer, "task");
        app.session.stop_claude_task(&peer, "task");
    }
    assert_eq!(
        *peer.controls.borrow(),
        [
            ("task.stop".into(), json!({"taskId":"task","runId":"a"})),
            ("task.stop".into(), json!({"taskId":"task","runId":"b"})),
        ]
    );
    app.session
        .receive_claude_task_stop("task", Some("a"), Some("old failure".into()));
    assert!(matches!(
        app.session
            .claude()
            .tasks()
            .stops()
            .state("task", Some("b")),
        Some(StopState::Sending)
    ));
    app.session
        .receive_claude_task_stop("task", Some("b"), None);
    assert!(matches!(
        app.session
            .claude()
            .tasks()
            .stops()
            .state("task", Some("b")),
        Some(StopState::Requested)
    ));
    app.session.update_claude_task(&json!({"subtype":"task_notification","task_id":"task","run_id":"b","status":"completed"})).expect("terminal edge");
    app.session
        .receive_claude_task_stop("task", Some("b"), Some("late failure".into()));
    assert!(app
        .session
        .claude()
        .tasks()
        .stops()
        .state("task", Some("b"))
        .is_none());
}

#[test]
fn activity_defaults_and_label_precedence_leave_context_revision_unchanged() {
    for (state, expected, label) in [
        (json!("running"), NativeState::Running, Some("Running")),
        (
            json!("requires_action"),
            NativeState::RequiresAction,
            Some("Requires action"),
        ),
        (json!("future"), NativeState::Idle, None),
        (Value::Null, NativeState::Idle, None),
    ] {
        let mut app = app();
        fact(&mut app, "claude/thinking", json!({"estimated_tokens":42}));
        fact(&mut app, "claude/status", json!({"label":"Temporary"}));
        assert_eq!(
            super::claude_activity::label(app.session.claude().activity()).as_deref(),
            Some("Temporary")
        );
        fact(&mut app, "claude/status", json!({}));
        assert_eq!(
            super::claude_activity::label(app.session.claude().activity()).as_deref(),
            Some("Thinking ~42 tokens")
        );
        fact(&mut app, "claude/activity", json!({"state":state}));
        assert_eq!(app.session.claude().activity().native_state(), expected);
        if expected == NativeState::Running {
            fact(
                &mut app,
                "claude/thinking",
                json!({"estimated_tokens":"invalid"}),
            );
        }
        assert_eq!(
            super::claude_activity::label(app.session.claude().activity()).as_deref(),
            label
        );
        assert_eq!(app.session.claude().activity().context_revision(), 0);
    }
}

#[test]
fn compaction_reuses_attempts_until_a_new_boundary_or_start_after_any_result() {
    let mut app = app();
    app.session.set_claude_generation_for_test(7);
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
        super::claude_notices::compact(&mut app, &message);
        assert_eq!(app.session.messages().len(), rows);
        assert_eq!(
            app.session.claude().activity().context_revision(),
            index as u64 + 1
        );
        assert_eq!(app.session.claude().activity().transient(), transient);
    }
    assert!(app.has_item_mapping("compact:7:1"));
    assert!(app.has_item_mapping("compact:7:2"));
    assert!(app.has_item_mapping("compact:7:3"));
    assert_eq!(
        app.session.messages()[2].text,
        "Claude could not compact context: unknown error"
    );
}

#[test]
fn observing_a_changed_model_fences_the_read_before_tick_without_duplicate_bumps() {
    let mut app = app();
    let peer = Peer::default();
    super::claude_models::observe_model(&mut app, "old");
    claude_context::tick(&peer, &mut app, Instant::now());
    let request = peer.controls.borrow()[0].1.clone();
    app.session.observe_claude_model("new");
    assert_eq!(app.session.claude().activity().context_revision(), 2);
    assert_eq!(app.session.claude().observed_model(), Some("new"));
    claude_context::receive(
        &mut app,
        &json!({"ok":true,"result":{"totalTokens":99,"maxTokens":100}}),
        &request,
    );
    assert!(app.session.context_usage().is_none());
    super::claude_models::observe_model(&mut app, "new");
    assert_eq!(app.session.claude().activity().context_revision(), 2);
    claude_context::tick(&peer, &mut app, Instant::now());
    assert_eq!(
        peer.controls.borrow()[1].1,
        json!({"revision":2,"contextToken":2})
    );
}

#[test]
fn clear_retains_process_work_but_failure_only_clears_activity_and_live_membership() {
    let mut app = app();
    app.session.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"task","run_id":"a"}]})).expect("level");
    app.session.begin_claude_task_stop_for_test("task");
    fact(&mut app, "claude/activity", json!({"state":"running"}));
    app.view.claude_tasks.selected = Some("task".into());
    app.set_input_text("keep this draft");
    super::claude_reset::receive(&mut app, &json!({"new_conversation_id":"clear"}));
    assert_eq!(app.session.claude().tasks().active_count(), 1);
    assert_eq!(
        app.session.claude().activity().native_state(),
        NativeState::Running
    );
    assert_eq!(app.session.claude().activity().context_revision(), 1);
    assert!(matches!(
        app.session
            .claude()
            .tasks()
            .stops()
            .state("task", Some("a")),
        Some(StopState::Sending)
    ));
    assert_eq!(app.input_text(), "keep this draft");
    fail_session(&mut app);
    assert_eq!(app.session.claude().tasks().active_count(), 0);
    assert!(app.session.claude().tasks().data("task").is_some());
    assert!(matches!(
        app.session
            .claude()
            .tasks()
            .stops()
            .state("task", Some("a")),
        Some(StopState::Sending)
    ));
    assert_eq!(app.session.claude().activity().context_revision(), 0);
    assert_eq!(app.view.claude_tasks.selected.as_deref(), Some("task"));
}

#[test]
fn immediate_stop_failure_keeps_the_run_retryable_without_a_transcript_error() {
    let mut app = app();
    let peer = Peer::default();
    app.session
        .update_claude_task(&json!({"subtype":"task_started","task_id":"task"}))
        .expect("run without id");
    peer.fail_control.set(true);
    app.session.stop_claude_task(&peer, "task");
    match app.session.claude().tasks().stops().state("task", None) {
        Some(StopState::Failed(error)) => assert_eq!(error, "injected stop failure"),
        _ => panic!("send failure must remain attached to this run"),
    }
    assert!(app.session.messages().is_empty());
    peer.fail_control.set(false);
    app.session.stop_claude_task(&peer, "task");
    assert_eq!(
        *peer.controls.borrow(),
        [
            ("task.stop".into(), json!({"taskId":"task","runId":null})),
            ("task.stop".into(), json!({"taskId":"task","runId":null})),
        ]
    );
    assert!(matches!(
        app.session.claude().tasks().stops().state("task", None),
        Some(StopState::Sending)
    ));
}
