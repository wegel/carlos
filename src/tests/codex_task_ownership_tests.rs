//! Preserve verified lineage facts even when an ancestor falls outside the retained F2 window.

use std::time::{Duration, Instant};

use serde_json::json;

use super::codex_question_support::{app, Peer};
use super::codex_task_status_tests::{agent, answer, process};
use crate::app::codex_task_status::tick;
use crate::app::task_badge::count;

#[test]
fn verified_grandchild_outside_parent_window_keeps_its_badge_and_targeted_refresh_until_root_switch(
) {
    let mut app = app();
    let peer = Peer::default();
    let mut grand = agent("grand", "active");
    grand["parentThreadId"] = json!("bridge");
    grand["source"] = json!({"subAgent":{"thread_spawn":{"parent_thread_id":"bridge"}}});
    let mut bridge = agent("bridge", "idle");
    bridge["source"] = json!({"subAgent":{"thread_spawn":{"parent_thread_id":"root"}}});
    crate::app::codex_panel::refresh(&mut app, false);
    for (index, result) in [
        json!({"data":[grand],"nextCursor":null}),
        json!({"thread":bridge}),
        json!({"data":[],"nextCursor":null}),
        json!({"data":[process("grand","one").data()],"nextCursor":null}),
    ]
    .into_iter()
    .enumerate()
    {
        crate::app::codex_panel::tick(&peer, &mut app);
        answer(&peer, &mut app, index, json!({"result":result}));
    }
    assert_eq!(app.session.codex_panel().agents().len(), 1);
    assert!(crate::app::task_badge::owns(&app, "bridge"));
    assert!(crate::app::task_badge::owns(&app, "grand"));
    assert_eq!(count(&app), 2);
    tick(&peer, &mut app, Instant::now() + Duration::from_secs(4));
    assert_eq!(peer.controls.borrow()[4].1["threadId"], "grand");
    app.set_thread_id("new-root");
    answer(
        &peer,
        &mut app,
        4,
        json!({"result":{"data":[process("grand","one").data()],"nextCursor":null}}),
    );
    assert_eq!(count(&app), 0);
    assert!(!crate::app::task_badge::owns(&app, "grand"));
    assert!(app.session.codex_panel().processes().is_empty());
}

#[test]
fn reconnect_installs_bounded_positive_owners_for_parents_outside_the_display_window() {
    let mut app = super::codex_reconnect_support::app();
    let peer = super::codex_reconnect_support::Peer::default();
    let mut snapshot = super::codex_reconnect_support::snapshot("root answer", "idle");
    let mut grand = agent("grand", "active");
    grand["parentThreadId"] = json!("bridge");
    snapshot.panel = json!({"agents":[grand],"processes":[{"thread":"grand","data":process("grand","one").data()}],
        "verifiedOwners":["bridge","grand"],"older":null});
    super::codex_reconnect_support::ready(&peer, &mut app, snapshot);
    assert!(!app.codex.closed);
    assert!(crate::app::task_badge::owns(&app, "grand"));
    assert!(crate::app::task_badge::owns(&app, "bridge"));
    assert_eq!(count(&app), 2);
}

#[test]
fn successful_empty_root_scope_clears_its_old_error_even_when_a_child_scope_fails() {
    let mut app = app();
    let peer = Peer::default();
    app.session
        .seed_codex_task_error_for_test("root", "Old root error");
    let mut child = agent("child", "active");
    child["source"] = json!({"subAgent":{"thread_spawn":{"parent_thread_id":"root"}}});
    crate::app::codex_panel::refresh(&mut app, false);
    for (index, response) in [
        json!({"result":{"data":[child],"nextCursor":null}}),
        json!({"result":{"data":[],"nextCursor":null}}),
        json!({"error":{"code":-32000,"message":"Child registry unavailable"}}),
    ]
    .into_iter()
    .enumerate()
    {
        crate::app::codex_panel::tick(&peer, &mut app);
        answer(&peer, &mut app, index, response);
    }
    assert!(app.session.codex_panel().error().is_some());
    assert!(!app
        .session
        .codex_task_status()
        .errors()
        .contains_key("root"));
    assert!(!app.session.codex_task_status().uncertain().contains("root"));
    assert!(
        app.session.codex_task_status().errors()["child"].contains("Child registry unavailable")
    );
    assert!(!crate::app::codex_panel_text::body(&app)
        .join("\n")
        .contains("Old root error"));
}

#[test]
fn complete_empty_f2_scope_retires_older_targeted_read_without_retry_after_late_response() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    let old = process("root", "old");
    let data = old.data().clone();
    app.session.push_codex_process_for_test(old);
    crate::app::tests::codex_panel_support::panel_updated(&mut app);
    tick(&peer, &mut app, now + Duration::from_secs(4));
    assert_eq!(peer.controls.borrow().len(), 1);
    app.session.clear_codex_processes_for_test();
    app.session
        .set_codex_process_scope_for_test("root".into(), None);
    crate::app::tests::codex_panel_support::panel_updated(&mut app);
    answer(
        &peer,
        &mut app,
        0,
        json!({"error":{"message":"obsolete read failed"}}),
    );
    assert!(crate::app::codex_task_status::wait(&app, now + Duration::from_secs(21)).is_none());
    assert!(!tick(&peer, &mut app, now + Duration::from_secs(21)));
    assert_eq!(peer.controls.borrow().len(), 1);
    assert!(app.session.codex_task_status().errors().is_empty());
    assert!(app.session.codex_panel().processes().is_empty());
    answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"data":[data],"nextCursor":null}}),
    );
    assert_eq!(count(&app), 0);
}

#[test]
fn f2_snapshot_without_an_owners_scope_leaves_its_targeted_read_actionable() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    app.session
        .push_codex_process_for_test(process("root", "one"));
    crate::app::tests::codex_panel_support::panel_updated(&mut app);
    tick(&peer, &mut app, now + Duration::from_secs(4));
    app.session
        .set_codex_process_scope_for_test("other-owner".into(), None);
    crate::app::tests::codex_panel_support::panel_updated(&mut app);
    answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"data":[],"nextCursor":null}}),
    );
    assert!(app.session.codex_panel().processes().is_empty());
    assert!(crate::app::codex_task_status::wait(&app, now + Duration::from_secs(4)).is_none());
}

#[test]
fn failed_f2_scope_retires_older_targeted_success_without_erasing_uncertainty() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    app.session
        .push_codex_process_for_test(process("root", "one"));
    crate::app::tests::codex_panel_support::panel_updated(&mut app);
    tick(&peer, &mut app, now + Duration::from_secs(4));
    app.session
        .set_codex_process_scope_for_test("root".into(), Some("Newer registry failure".into()));
    crate::app::tests::codex_panel_support::panel_updated(&mut app);
    answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"data":[process("root","one").data()],"nextCursor":null}}),
    );
    assert_eq!(count(&app), 0);
    assert!(app.session.codex_task_status().uncertain().contains("root"));
    assert_eq!(
        app.session.codex_task_status().errors()["root"],
        "Newer registry failure"
    );
    assert!(crate::app::codex_task_status::wait(&app, now + Duration::from_secs(21)).is_none());
    assert!(!tick(&peer, &mut app, now + Duration::from_secs(21)));
    assert_eq!(peer.controls.borrow().len(), 1);
}
