//! Check native task level/edge ordering, terminal details, and bounded run history.

use serde_json::json;

use super::claude_tasks::{ClaudeTasks, TaskStatus};

#[test]
fn late_same_run_edges_cannot_replace_terminal_details_or_resurrect_activity() {
    let mut tasks = ClaudeTasks::default();
    tasks.update(&json!({"subtype":"task_started","task_id":"worker","run_id":"002","description":"initial"})).expect("start");
    tasks.update(&json!({"subtype":"task_notification","task_id":"worker","run_id":"002","status":"completed",
        "summary":"authoritative final report","output_file":"/tmp/native-output"})).expect("finish");
    for subtype in ["task_started", "task_progress", "task_updated"] {
        tasks.update(&json!({"subtype":subtype,"task_id":"worker","run_id":"002","summary":"old progress",
            "description":"late old description","patch":{"status":"running"}})).expect("late edge");
    }
    assert!(tasks.records["worker"].status == TaskStatus::Completed);
    assert_eq!(
        tasks.records["worker"].data["summary"],
        "authoritative final report"
    );
    assert_eq!(tasks.records["worker"].data["description"], "initial");
    assert_eq!(tasks.active_count(), 0);
}

#[test]
fn level_replaces_membership_and_ambient_watchers_do_not_count_as_activity() {
    let mut tasks = ClaudeTasks::default();
    tasks
        .update(&json!({"subtype":"background_tasks_changed","tasks":[
        {"task_id":"worker","run_id":"002","description":"work"},
        {"task_id":"watcher","run_id":"001","ambient":true}]}))
        .expect("level");
    assert_eq!(tasks.active_count(), 1);
    tasks
        .update(&json!({"subtype":"background_tasks_changed","tasks":[]}))
        .expect("empty level");
    assert_eq!(tasks.active_count(), 0);
    tasks
        .update(&json!({"subtype":"task_started","task_id":"worker","run_id":"001"}))
        .expect("old start");
    assert_eq!(tasks.active_count(), 0);
    assert_eq!(tasks.records["worker"].data["run_id"], "002");
}

#[test]
fn cache_churn_cannot_forget_a_newer_live_run_or_allow_an_older_level_to_replace_it() {
    let mut tasks = ClaudeTasks::default();
    tasks.update(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"long-lived","run_id":"new"}]})).expect("long-lived level");
    for index in 0..4200 {
        tasks.update(&json!({"subtype":"task_notification","task_id":format!("ended-{index}"),"run_id":"one","status":"completed"})).expect("bounded churn");
    }
    tasks
        .update(&json!({"subtype":"task_started","task_id":"long-lived","run_id":"aaa"}))
        .expect("late edge");
    tasks.update(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"long-lived","run_id":"aaa"}]})).expect("late old level");
    assert_eq!(tasks.live["long-lived"]["run_id"], "new");
    assert_eq!(tasks.active_count(), 1);
    assert!(tasks.records.len() <= 256);
}

#[test]
fn new_run_opens_after_terminal_run_and_old_missing_or_late_run_frames_stay_ignored() {
    let mut tasks = ClaudeTasks::default();
    tasks.update(&json!({"subtype":"task_notification","task_id":"worker","run_id":"A","status":"stopped"})).expect("old run");
    tasks.update(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"worker","run_id":"B","description":"resumed"}]})).expect("resumed level");
    tasks.update(&json!({"subtype":"task_started","task_id":"worker","run_id":"B","description":"new run"})).expect("resumed start");
    tasks.update(&json!({"subtype":"task_notification","task_id":"worker","run_id":"A","status":"failed"})).expect("late old finish");
    tasks
        .update(&json!({"subtype":"task_progress","task_id":"worker","description":"missing run"}))
        .expect("missing old run");
    assert!(tasks.records["worker"].status == TaskStatus::Running);
    assert_eq!(tasks.records["worker"].data["description"], "new run");
}

#[test]
fn level_only_task_keeps_one_stop_request_and_ack_across_detail_cache_churn() {
    let mut tasks = ClaudeTasks::default();
    tasks.update(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"old-live","run_id":"z"}]})).expect("live task");
    assert_eq!(tasks.begin_stop("old-live"), Some(Some("z".to_string())));
    for index in 0..300 {
        tasks
            .update(
                &json!({"subtype":"task_started","task_id":format!("task-{index}"),"run_id":"z"}),
            )
            .expect("cache churn");
    }
    assert!(!tasks.records.contains_key("old-live"));
    assert_eq!(tasks.begin_stop("old-live"), None);
    tasks.stop_reply("old-live", Some("z"), None);
    assert!(matches!(
        tasks.stops.state("old-live", Some("z")),
        Some(super::claude_task_stops::StopState::Requested)
    ));
    tasks.update(&json!({"subtype":"task_notification","task_id":"old-live","run_id":"z","status":"stopped"})).expect("native stop");
    assert!(tasks.stops.state("old-live", Some("z")).is_none());
}

#[test]
fn running_foreground_task_can_be_stopped_without_fabricating_background_membership() {
    let mut tasks = ClaudeTasks::default();
    tasks.update(&json!({"subtype":"task_started","task_id":"foreground","run_id":"a","is_backgrounded":false})).expect("foreground");
    assert!(tasks.live.is_empty());
    assert!(tasks.can_stop("foreground"));
    assert_eq!(tasks.begin_stop("foreground"), Some(Some("a".to_string())));
    assert_eq!(tasks.active_count(), 0);
}

#[test]
fn opening_pins_default_selection_through_membership_change_completion_and_detail_eviction() {
    let mut tasks = ClaudeTasks::default();
    tasks.update(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"alpha","run_id":"a"},{"task_id":"beta","run_id":"a"}]})).expect("two tasks");
    tasks.open();
    assert_eq!(tasks.selected_id(), Some("alpha"));
    tasks.update(&json!({"subtype":"task_notification","task_id":"alpha","run_id":"a","status":"completed"})).expect("alpha ends");
    tasks.update(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"beta","run_id":"a"}]})).expect("beta remains");
    assert_eq!(tasks.selected_id(), Some("alpha"));
    assert_eq!(tasks.begin_stop("alpha"), None);
    for index in 0..300 {
        tasks
            .update(
                &json!({"subtype":"task_started","task_id":format!("later-{index}"),"run_id":"a"}),
            )
            .expect("detail churn");
    }
    assert_eq!(tasks.selected.as_deref(), Some("alpha"));
    assert_eq!(tasks.selected_id(), None);
    tasks.open();
    assert_eq!(tasks.selected_id(), None, "reopening cannot retarget x");
}
