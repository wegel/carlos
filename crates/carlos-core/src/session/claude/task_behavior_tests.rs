//! Preserve native task ordering, bounded caches and exact stop registry behavior without a terminal.

use serde_json::json;

use crate::session::claude::{StopState, TaskStatus};
use crate::session::state::SessionState;
use crate::session::test_backend::TestBackend;

#[test]
fn late_same_run_edges_cannot_replace_terminal_details_or_resurrect_activity() {
    let mut core = SessionState::new("tasks".into(), Default::default(), false);
    core.update_claude_task(&json!({"subtype":"task_started","task_id":"worker","run_id":"002","description":"initial"})).expect("start");
    core.update_claude_task(&json!({"subtype":"task_notification","task_id":"worker","run_id":"002","status":"completed",
        "summary":"authoritative final report","output_file":"/tmp/native-output"})).expect("finish");
    for subtype in ["task_started", "task_progress", "task_updated"] {
        core.update_claude_task(
            &json!({"subtype":subtype,"task_id":"worker","run_id":"002","summary":"old progress",
            "description":"late old description","patch":{"status":"running"}}),
        )
        .expect("late edge");
    }
    assert!(core.claude().tasks().records()["worker"].status() == TaskStatus::Completed);
    assert_eq!(
        core.claude().tasks().records()["worker"].data()["summary"],
        "authoritative final report"
    );
    assert_eq!(
        core.claude().tasks().records()["worker"].data()["description"],
        "initial"
    );
    assert_eq!(core.claude().tasks().active_count(), 0);
    assert_eq!(core.claude.tasks.begin_stop("worker"), None);
}

#[test]
fn level_replaces_membership_and_ambient_watchers_do_not_count_as_activity() {
    let mut core = SessionState::new("tasks".into(), Default::default(), false);
    core.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[
        {"task_id":"worker","run_id":"002","description":"work"},
        {"task_id":"watcher","run_id":"001","ambient":true}]}))
        .expect("level");
    assert_eq!(core.claude().tasks().active_count(), 1);
    core.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[]}))
        .expect("empty level");
    assert_eq!(core.claude().tasks().active_count(), 0);
    core.update_claude_task(&json!({"subtype":"task_started","task_id":"worker","run_id":"001"}))
        .expect("old start");
    assert_eq!(core.claude().tasks().active_count(), 0);
    assert_eq!(
        core.claude().tasks().records()["worker"].data()["run_id"],
        "002"
    );
}

#[test]
fn cache_churn_cannot_forget_a_newer_live_run_or_allow_an_older_level_to_replace_it() {
    let mut core = SessionState::new("tasks".into(), Default::default(), false);
    core.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"long-lived","run_id":"new"}]})).expect("long-lived level");
    for index in 0..4200 {
        core.update_claude_task(&json!({"subtype":"task_notification","task_id":format!("ended-{index}"),"run_id":"one","status":"completed"})).expect("bounded churn");
    }
    core.update_claude_task(
        &json!({"subtype":"task_started","task_id":"long-lived","run_id":"aaa"}),
    )
    .expect("late edge");
    core.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"long-lived","run_id":"aaa"}]})).expect("late old level");
    assert_eq!(core.claude().tasks().live()["long-lived"]["run_id"], "new");
    assert_eq!(core.claude().tasks().active_count(), 1);
    assert!(core.claude().tasks().records().len() <= 256);
}

#[test]
fn new_run_opens_after_terminal_run_and_old_missing_or_late_run_frames_stay_ignored() {
    let mut core = SessionState::new("tasks".into(), Default::default(), false);
    core.update_claude_task(
        &json!({"subtype":"task_notification","task_id":"worker","run_id":"A","status":"stopped"}),
    )
    .expect("old run");
    core.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"worker","run_id":"B","description":"resumed"}]})).expect("resumed level");
    core.update_claude_task(
        &json!({"subtype":"task_started","task_id":"worker","run_id":"B","description":"new run"}),
    )
    .expect("resumed start");
    core.update_claude_task(
        &json!({"subtype":"task_notification","task_id":"worker","run_id":"A","status":"failed"}),
    )
    .expect("late old finish");
    core.update_claude_task(
        &json!({"subtype":"task_progress","task_id":"worker","description":"missing run"}),
    )
    .expect("missing old run");
    assert!(core.claude().tasks().records()["worker"].status() == TaskStatus::Running);
    assert_eq!(
        core.claude().tasks().records()["worker"].data()["description"],
        "new run"
    );
}

#[test]
fn level_only_task_keeps_one_stop_request_and_ack_across_detail_cache_churn() {
    let mut core = SessionState::new("tasks".into(), Default::default(), false);
    core.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"old-live","run_id":"z"}]})).expect("live task");
    assert_eq!(
        core.claude.tasks.begin_stop("old-live"),
        Some(Some("z".to_string()))
    );
    for index in 0..300 {
        core.update_claude_task(
            &json!({"subtype":"task_started","task_id":format!("task-{index}"),"run_id":"z"}),
        )
        .expect("cache churn");
    }
    assert!(!core.claude().tasks().records().contains_key("old-live"));
    assert_eq!(core.claude.tasks.begin_stop("old-live"), None);
    core.receive_claude_task_stop("old-live", Some("z"), None);
    assert!(matches!(
        core.claude().tasks().stops().state("old-live", Some("z")),
        Some(StopState::Requested)
    ));
    core.update_claude_task(&json!({"subtype":"task_notification","task_id":"old-live","run_id":"z","status":"stopped"})).expect("native stop");
    assert!(core
        .claude()
        .tasks()
        .stops()
        .state("old-live", Some("z"))
        .is_none());
}

#[test]
fn running_foreground_task_can_be_stopped_without_fabricating_background_membership() {
    let mut core = SessionState::new("tasks".into(), Default::default(), false);
    core.update_claude_task(&json!({"subtype":"task_started","task_id":"foreground","run_id":"a","is_backgrounded":false})).expect("foreground");
    assert!(core.claude().tasks().live().is_empty());
    assert!(core.claude().tasks().can_stop("foreground"));
    assert_eq!(
        core.claude.tasks.begin_stop("foreground"),
        Some(Some("a".to_string()))
    );
    assert_eq!(core.claude().tasks().active_count(), 0);
}

#[test]
fn detail_bound_evicts_terminal_rows_before_older_running_rows() {
    let mut core = SessionState::new("root".into(), Default::default(), false);
    core.update_claude_task(&json!({"subtype":"task_started","task_id":"old-running"}))
        .expect("old work");
    core.update_claude_task(
        &json!({"subtype":"task_notification","task_id":"terminal","status":"completed"}),
    )
    .expect("finished work");
    for index in 0..255 {
        core.update_claude_task(
            &json!({"subtype":"task_started","task_id":format!("task-{index}")}),
        )
        .expect("detail churn");
    }
    assert_eq!(core.claude().tasks().records().len(), 256);
    assert!(core.claude().tasks().data("terminal").is_none());
    assert!(core.claude().tasks().data("old-running").is_some());
    core.update_claude_task(&json!({"subtype":"task_started","task_id":"newest"}))
        .expect("one more row");
    assert!(core.claude().tasks().data("old-running").is_none());
    assert!(core.claude().tasks().data("newest").is_some());
}

#[test]
fn an_oversized_detail_is_evicted_without_changing_live_membership() {
    let mut core = SessionState::new("root".into(), Default::default(), false);
    core.update_claude_task(
        &json!({"subtype":"background_tasks_changed","tasks":[{"task_id":"live","run_id":"a"}]}),
    )
    .expect("live work");
    core.update_claude_task(&json!({"subtype":"task_started","task_id":"oversized","description":"x".repeat(8*1024*1024)})).expect("oversized edge");
    assert_eq!(core.claude().tasks().records().len(), 0);
    assert_eq!(core.claude().tasks().ids(), ["live"]);
    assert_eq!(core.claude().tasks().active_count(), 1);
    assert_eq!(
        core.claude().tasks().data("live").expect("level fallback")["run_id"],
        "a"
    );
}

#[test]
fn full_stop_registry_reports_one_error_without_sending_an_overflow_stop() {
    let mut core = SessionState::new("root".into(), Default::default(), false);
    let tasks: Vec<_> = (0..4096)
        .map(|index| json!({"task_id":format!("task-{index}"),"run_id":"a"}))
        .collect();
    core.update_claude_task(&json!({"subtype":"background_tasks_changed","tasks":tasks}))
        .expect("full live set");
    for index in 0..4096 {
        assert_eq!(
            core.claude.tasks.begin_stop(&format!("task-{index}")),
            Some(Some("a".into()))
        );
    }
    core.update_claude_task(&json!({"subtype":"task_started","task_id":"overflow","run_id":"a"}))
        .expect("foreground work");
    let peer = TestBackend::default();
    core.stop_claude_task(&peer, "overflow");
    assert_eq!(
        core.claude().tasks().control_error(),
        Some("Too many unanswered native task stops; wait for native task events")
    );
    assert!(peer.calls.lock().expect("native calls").is_empty());
}
