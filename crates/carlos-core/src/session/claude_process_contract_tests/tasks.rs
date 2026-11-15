//! Keep exact native task bounds, run comparisons, terminal edges and stop targets.

use serde_json::{json, Value};

use super::{app, operation, stop};
use crate::session::claude::{StopState, TaskStatus};

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
        assert_eq!(app.update_claude_task(&message), expected);
        assert!(app.claude().tasks().ids().is_empty());
    }
}

#[test]
fn malformed_membership_retains_earlier_detail_writes_and_the_prior_live_set() {
    let mut app = app();
    app.update_claude_task(&json!({"subtype":"background_tasks_changed",
        "tasks":[{"task_id":"old","run_id":"a"}]}))
        .expect("old level");
    assert_eq!(
        app.update_claude_task(&json!({"subtype":"background_tasks_changed",
        "tasks":[{}, {"task_id":"new","run_id":"b"}, {"task_id":"x".repeat(4097)}]})),
        Err("Native task identity exceeds the bounded task view")
    );
    assert_eq!(
        app.claude()
            .tasks()
            .live()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["old"]
    );
    assert_eq!(
        app.claude().tasks().data("new").expect("earlier detail")["run_id"],
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
            app.update_claude_task(&json!({"subtype":"task_started", "task_id":"task",
                "run_id":run,"description":description}))
                .expect("run edge");
        }
        app.update_claude_task(&json!({"subtype":"task_started","task_id":"task",
            "description":"missing"}))
            .expect("ignored missing run");
        assert_eq!(
            app.claude().tasks().data("task").expect("task")["description"],
            expected
        );
    }
}

#[test]
fn terminal_edges_accept_one_report_and_unknown_states_keep_the_prior_status() {
    let mut app = app();
    for (message, expected, label) in [
        (
            json!({"subtype":"task_started"}),
            TaskStatus::Running,
            "running",
        ),
        (
            json!({"subtype":"task_updated","patch":{"status":"future"}}),
            TaskStatus::Running,
            "running",
        ),
        (
            json!({"subtype":"task_updated","patch":{"status":"killed"}}),
            TaskStatus::Stopped,
            "stopped",
        ),
        (
            json!({"subtype":"task_notification","status":"completed","summary":"first"}),
            TaskStatus::Stopped,
            "stopped",
        ),
        (
            json!({"subtype":"task_notification","status":"failed","summary":"second"}),
            TaskStatus::Stopped,
            "stopped",
        ),
    ] {
        let mut message = message;
        message["task_id"] = json!("task");
        message["run_id"] = json!("a");
        app.update_claude_task(&message).expect("edge");
        let record = &app.claude().tasks().records()["task"];
        assert_eq!(record.status(), expected);
        let page = app
            .task_page(None, None, 32)
            .expect("actual shared task state");
        assert_eq!(page.tasks[0].state, label);
    }
    assert_eq!(
        app.claude().tasks().data("task").expect("final report")["summary"],
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
        app.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[task]}))
            .expect("level");
        let mut message = json!({"subtype":"task_notification","task_id":"task","run_id":"a"});
        message
            .as_object_mut()
            .expect("object")
            .extend(edge.as_object().expect("edge").clone());
        app.update_claude_task(&message).expect("edge");
        assert_eq!(app.claude().tasks().active_count(), expected);
    }
}

#[test]
fn stops_send_once_and_ignore_replies_for_old_or_terminal_runs() {
    let mut app = app();
    assert!(app
        .task_page(None, None, 32)
        .expect("no task target")
        .tasks
        .is_empty());
    let mut captured = Vec::new();
    for (counter, run) in [(1, "a"), (2, "b")] {
        app.update_claude_task(&json!({"subtype":"task_started","task_id":"task","run_id":run}))
            .expect("start");
        let work = stop(&mut app, "task", counter);
        captured.push(work.claim.target.request());
        let task = app
            .task_page(None, None, 32)
            .expect("task page")
            .tasks
            .remove(0);
        assert!(app
            .prepare_task_stop(
                operation(counter + 10),
                task.id,
                task.run.expect("native run")
            )
            .is_err());
    }
    assert_eq!(
        captured,
        [
            ("task.stop", json!({"taskId":"task","runId":"a"})),
            ("task.stop", json!({"taskId":"task","runId":"b"}))
        ]
    );
    app.receive_claude_task_stop("task", Some("a"), Some("old failure".into()));
    assert!(matches!(
        app.claude().tasks().stops().state("task", Some("b")),
        Some(StopState::Sending)
    ));
    app.receive_claude_task_stop("task", Some("b"), None);
    assert!(matches!(
        app.claude().tasks().stops().state("task", Some("b")),
        Some(StopState::Requested)
    ));
    app.update_claude_task(
        &json!({"subtype":"task_notification","task_id":"task","run_id":"b","status":"completed"}),
    )
    .expect("terminal edge");
    app.receive_claude_task_stop("task", Some("b"), Some("late failure".into()));
    assert!(app
        .claude()
        .tasks()
        .stops()
        .state("task", Some("b"))
        .is_none());
}

#[test]
fn immediate_stop_failure_keeps_the_run_retryable_without_a_transcript_error() {
    let mut app = app();
    app.update_claude_task(&json!({"subtype":"task_started","task_id":"task"}))
        .expect("unknown native run");
    app.update_claude_task(&json!({"subtype":"task_started","task_id":"known","run_id":"a"}))
        .expect("known native run");
    let (unknown, known_run) = {
        let rows = app.task_rows();
        let unknown = rows
            .iter()
            .find(|row| row.native_id() == "task")
            .expect("unknown native run");
        let known = rows
            .iter()
            .find(|row| row.native_id() == "known")
            .expect("known native run");
        assert!(unknown.run(&app).is_none());
        (unknown.id(&app), known.run(&app).expect("known proof"))
    };
    assert!(app
        .prepare_task_stop(operation(1), unknown, known_run)
        .is_err());
    // The native registry still handles optional runs, but the public writer never sends null-run controls.
    assert_eq!(app.claude.tasks.begin_stop("task"), Some(None));
    app.receive_claude_task_stop("task", None, Some("injected stop failure".into()));
    match app.claude().tasks().stops().state("task", None) {
        Some(StopState::Failed(error)) => assert_eq!(error, "injected stop failure"),
        _ => panic!("exact native run keeps its failure"),
    }
    assert!(app.messages().is_empty());
    assert_eq!(app.claude.tasks.begin_stop("task"), Some(None));
    assert!(matches!(
        app.claude().tasks().stops().state("task", None),
        Some(StopState::Sending)
    ));
}
