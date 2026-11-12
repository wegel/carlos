//! Check native task level/edge ordering, terminal details, and bounded run history.

use serde_json::json;

use super::state::AppState;
use crate::session::claude::{StopState, TaskStatus};

#[test]
fn late_same_run_edges_cannot_replace_terminal_details_or_resurrect_activity() {
    let mut app = AppState::new("tasks".into());
    app.session.update_claude_task(&json!({"subtype":"task_started","task_id":"worker","run_id":"002","description":"initial"})).expect("start");
    app.session.update_claude_task(&json!({"subtype":"task_notification","task_id":"worker","run_id":"002","status":"completed",
        "summary":"authoritative final report","output_file":"/tmp/native-output"})).expect("finish");
    for subtype in ["task_started", "task_progress", "task_updated"] {
        app.session.update_claude_task(&json!({"subtype":subtype,"task_id":"worker","run_id":"002","summary":"old progress",
            "description":"late old description","patch":{"status":"running"}})).expect("late edge");
    }
    assert!(app.session.claude().tasks().records()["worker"].status() == TaskStatus::Completed);
    assert_eq!(
        app.session.claude().tasks().records()["worker"].data()["summary"],
        "authoritative final report"
    );
    assert_eq!(
        app.session.claude().tasks().records()["worker"].data()["description"],
        "initial"
    );
    assert_eq!(app.session.claude().tasks().active_count(), 0);
}

#[test]
fn level_replaces_membership_and_ambient_watchers_do_not_count_as_activity() {
    let mut app = AppState::new("tasks".into());
    app.session
        .update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[
        {"task_id":"worker","run_id":"002","description":"work"},
        {"task_id":"watcher","run_id":"001","ambient":true}]}))
        .expect("level");
    assert_eq!(app.session.claude().tasks().active_count(), 1);
    app.session
        .update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[]}))
        .expect("empty level");
    assert_eq!(app.session.claude().tasks().active_count(), 0);
    app.session
        .update_claude_task(&json!({"subtype":"task_started","task_id":"worker","run_id":"001"}))
        .expect("old start");
    assert_eq!(app.session.claude().tasks().active_count(), 0);
    assert_eq!(
        app.session.claude().tasks().records()["worker"].data()["run_id"],
        "002"
    );
}

#[test]
fn cache_churn_cannot_forget_a_newer_live_run_or_allow_an_older_level_to_replace_it() {
    let mut app = AppState::new("tasks".into());
    app.session.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"long-lived","run_id":"new"}]})).expect("long-lived level");
    for index in 0..4200 {
        app.session.update_claude_task(&json!({"subtype":"task_notification","task_id":format!("ended-{index}"),"run_id":"one","status":"completed"})).expect("bounded churn");
    }
    app.session
        .update_claude_task(
            &json!({"subtype":"task_started","task_id":"long-lived","run_id":"aaa"}),
        )
        .expect("late edge");
    app.session.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"long-lived","run_id":"aaa"}]})).expect("late old level");
    assert_eq!(
        app.session.claude().tasks().live()["long-lived"]["run_id"],
        "new"
    );
    assert_eq!(app.session.claude().tasks().active_count(), 1);
    assert!(app.session.claude().tasks().records().len() <= 256);
}

#[test]
fn new_run_opens_after_terminal_run_and_old_missing_or_late_run_frames_stay_ignored() {
    let mut app = AppState::new("tasks".into());
    app.session.update_claude_task(&json!({"subtype":"task_notification","task_id":"worker","run_id":"A","status":"stopped"})).expect("old run");
    app.session.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"worker","run_id":"B","description":"resumed"}]})).expect("resumed level");
    app.session.update_claude_task(&json!({"subtype":"task_started","task_id":"worker","run_id":"B","description":"new run"})).expect("resumed start");
    app.session.update_claude_task(&json!({"subtype":"task_notification","task_id":"worker","run_id":"A","status":"failed"})).expect("late old finish");
    app.session
        .update_claude_task(
            &json!({"subtype":"task_progress","task_id":"worker","description":"missing run"}),
        )
        .expect("missing old run");
    assert!(app.session.claude().tasks().records()["worker"].status() == TaskStatus::Running);
    assert_eq!(
        app.session.claude().tasks().records()["worker"].data()["description"],
        "new run"
    );
}

#[test]
fn level_only_task_keeps_one_stop_request_and_ack_across_detail_cache_churn() {
    let mut app = AppState::new("tasks".into());
    app.session.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"old-live","run_id":"z"}]})).expect("live task");
    assert_eq!(
        app.session.begin_claude_task_stop_for_test("old-live"),
        Some(Some("z".to_string()))
    );
    for index in 0..300 {
        app.session
            .update_claude_task(
                &json!({"subtype":"task_started","task_id":format!("task-{index}"),"run_id":"z"}),
            )
            .expect("cache churn");
    }
    assert!(!app
        .session
        .claude()
        .tasks()
        .records()
        .contains_key("old-live"));
    assert_eq!(
        app.session.begin_claude_task_stop_for_test("old-live"),
        None
    );
    app.session
        .receive_claude_task_stop("old-live", Some("z"), None);
    assert!(matches!(
        app.session
            .claude()
            .tasks()
            .stops()
            .state("old-live", Some("z")),
        Some(StopState::Requested)
    ));
    app.session.update_claude_task(&json!({"subtype":"task_notification","task_id":"old-live","run_id":"z","status":"stopped"})).expect("native stop");
    assert!(app
        .session
        .claude()
        .tasks()
        .stops()
        .state("old-live", Some("z"))
        .is_none());
}

#[test]
fn running_foreground_task_can_be_stopped_without_fabricating_background_membership() {
    let mut app = AppState::new("tasks".into());
    app.session.update_claude_task(&json!({"subtype":"task_started","task_id":"foreground","run_id":"a","is_backgrounded":false})).expect("foreground");
    assert!(app.session.claude().tasks().live().is_empty());
    assert!(app.session.claude().tasks().can_stop("foreground"));
    assert_eq!(
        app.session.begin_claude_task_stop_for_test("foreground"),
        Some(Some("a".to_string()))
    );
    assert_eq!(app.session.claude().tasks().active_count(), 0);
}

#[test]
fn opening_pins_default_selection_through_membership_change_completion_and_detail_eviction() {
    let mut app = AppState::new("tasks".into());
    app.session.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"alpha","run_id":"a"},{"task_id":"beta","run_id":"a"}]})).expect("two tasks");
    app.claude.tasks.open(app.session.claude().tasks());
    assert_eq!(
        app.claude.tasks.selected_id(app.session.claude().tasks()),
        Some("alpha")
    );
    app.session.update_claude_task(&json!({"subtype":"task_notification","task_id":"alpha","run_id":"a","status":"completed"})).expect("alpha ends");
    app.session.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"beta","run_id":"a"}]})).expect("beta remains");
    assert_eq!(
        app.claude.tasks.selected_id(app.session.claude().tasks()),
        Some("alpha")
    );
    assert_eq!(app.session.begin_claude_task_stop_for_test("alpha"), None);
    for index in 0..300 {
        app.session
            .update_claude_task(
                &json!({"subtype":"task_started","task_id":format!("later-{index}"),"run_id":"a"}),
            )
            .expect("detail churn");
    }
    assert_eq!(app.claude.tasks.selected.as_deref(), Some("alpha"));
    assert_eq!(
        app.claude.tasks.selected_id(app.session.claude().tasks()),
        None
    );
    app.claude.tasks.open(app.session.claude().tasks());
    assert_eq!(
        app.claude.tasks.selected_id(app.session.claude().tasks()),
        None,
        "reopening cannot retarget x"
    );
}
