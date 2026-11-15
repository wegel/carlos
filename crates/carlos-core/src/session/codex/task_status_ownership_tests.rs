//! Preserve verified ancestry and process-owner fences independently of any local panel.

use std::time::{Duration, Instant};

use serde_json::json;

use super::task_badge;
use super::task_status_test_support::{
    agent, answer, count, native, overview, panel_tick, panel_updated, process, refresh, scope,
    session, tick, wait,
};
use crate::codex_snapshot::CodexSnapshot;
use crate::session::test_backend::TestBackend;

#[test]
fn verified_grandchild_outside_parent_window_keeps_its_badge_and_targeted_refresh_until_root_switch(
) {
    let mut core = session();
    let peer = TestBackend::default();
    let mut grand = agent("grand", "active");
    grand["parentThreadId"] = json!("bridge");
    grand["source"] = json!({"subAgent":{"thread_spawn":{"parent_thread_id":"bridge"}}});
    let mut bridge = agent("bridge", "idle");
    bridge["source"] = json!({"subAgent":{"thread_spawn":{"parent_thread_id":"root"}}});
    refresh(&mut core);
    for (index, result) in [
        json!({"data":[grand],"nextCursor":null}),
        json!({"thread":bridge}),
        json!({"data":[],"nextCursor":null}),
        json!({"data":[process("grand","one").data()],"nextCursor":null}),
    ]
    .into_iter()
    .enumerate()
    {
        panel_tick(&peer, &mut core);
        answer(&peer, &mut core, index, json!({"result":result}));
    }
    assert_eq!(core.codex_panel().agents().len(), 1);
    assert!(task_badge::owns(&core, scope(&core), "bridge"));
    assert!(task_badge::owns(&core, scope(&core), "grand"));
    assert_eq!(count(&core), 2);
    tick(&peer, &mut core, Instant::now() + Duration::from_secs(4));
    assert_eq!(
        peer.calls.lock().expect("native calls")[4].1["threadId"],
        "grand"
    );
    core.adopt_native_thread("new-root".into());
    answer(
        &peer,
        &mut core,
        4,
        json!({"result":{"data":[process("grand","one").data()],"nextCursor":null}}),
    );
    assert_eq!(count(&core), 0);
    assert!(!task_badge::owns(&core, scope(&core), "grand"));
    assert!(core.codex_panel().processes().is_empty());
}

#[test]
fn reconnect_installs_bounded_positive_owners_for_parents_outside_the_display_window() {
    let mut core = session();
    core.close_codex();
    core.set_codex_generation_for_test(2);
    core.set_codex_rebuilding_for_test(true);
    let peer = TestBackend::default();
    let mut snapshot = snapshot();
    let mut grand = agent("grand", "active");
    grand["parentThreadId"] = json!("bridge");
    snapshot.panel = json!({"agents":[grand],"processes":[{"thread":"grand","data":process("grand","one").data()}],
        "verifiedOwners":["bridge","grand"],"older":null});
    *peer.snapshot.lock().expect("reconnect snapshot") = Some(snapshot);
    native(
        &mut core,
        &peer,
        &json!({"method":"carlos/codex/frame","params":{"generation":2,
        "event":{"method":"carlos/codex/ready","params":{"generation":2,"threadId":"root"}}}})
        .to_string(),
    );
    assert!(!core.codex_scope().closed());
    assert!(task_badge::owns(&core, scope(&core), "grand"));
    assert!(task_badge::owns(&core, scope(&core), "bridge"));
    assert_eq!(count(&core), 2);
}

#[test]
fn successful_empty_root_scope_clears_its_old_error_even_when_a_child_scope_fails() {
    let mut core = session();
    let peer = TestBackend::default();
    core.seed_codex_task_error_for_test("root", "Old root error");
    let mut child = agent("child", "active");
    child["source"] = json!({"subAgent":{"thread_spawn":{"parent_thread_id":"root"}}});
    refresh(&mut core);
    for (index, response) in [
        json!({"result":{"data":[child],"nextCursor":null}}),
        json!({"result":{"data":[],"nextCursor":null}}),
        json!({"error":{"code":-32000,"message":"Child registry unavailable"}}),
    ]
    .into_iter()
    .enumerate()
    {
        panel_tick(&peer, &mut core);
        answer(&peer, &mut core, index, response);
    }
    assert!(core.codex_panel().error().is_some());
    assert!(!core.codex_task_status().errors().contains_key("root"));
    assert!(!core.codex_task_status().uncertain().contains("root"));
    assert!(core.codex_task_status().errors()["child"].contains("Child registry unavailable"));
    assert!(!overview(&core).contains("Old root error"));
}

#[test]
fn complete_empty_f2_scope_retires_older_targeted_read_without_retry_after_late_response() {
    let mut core = session();
    let peer = TestBackend::default();
    let now = Instant::now();
    let old = process("root", "old");
    let data = old.data().clone();
    core.push_codex_process_for_test(old);
    panel_updated(&mut core);
    tick(&peer, &mut core, now + Duration::from_secs(4));
    assert_eq!(peer.calls.lock().expect("native calls").len(), 1);
    core.clear_codex_processes_for_test();
    core.set_codex_process_scope_for_test("root".into(), None);
    panel_updated(&mut core);
    answer(
        &peer,
        &mut core,
        0,
        json!({"error":{"message":"obsolete read failed"}}),
    );
    assert!(wait(&core, now + Duration::from_secs(21)).is_none());
    assert!(!tick(&peer, &mut core, now + Duration::from_secs(21)));
    assert_eq!(peer.calls.lock().expect("native calls").len(), 1);
    assert!(core.codex_task_status().errors().is_empty());
    assert!(core.codex_panel().processes().is_empty());
    answer(
        &peer,
        &mut core,
        0,
        json!({"result":{"data":[data],"nextCursor":null}}),
    );
    assert_eq!(count(&core), 0);
}

#[test]
fn f2_snapshot_without_an_owners_scope_leaves_its_targeted_read_actionable() {
    let mut core = session();
    let peer = TestBackend::default();
    let now = Instant::now();
    core.push_codex_process_for_test(process("root", "one"));
    panel_updated(&mut core);
    tick(&peer, &mut core, now + Duration::from_secs(4));
    core.set_codex_process_scope_for_test("other-owner".into(), None);
    panel_updated(&mut core);
    answer(
        &peer,
        &mut core,
        0,
        json!({"result":{"data":[],"nextCursor":null}}),
    );
    assert!(core.codex_panel().processes().is_empty());
    assert!(wait(&core, now + Duration::from_secs(4)).is_none());
}

#[test]
fn failed_f2_scope_retires_older_targeted_success_without_erasing_uncertainty() {
    let mut core = session();
    let peer = TestBackend::default();
    let now = Instant::now();
    core.push_codex_process_for_test(process("root", "one"));
    panel_updated(&mut core);
    tick(&peer, &mut core, now + Duration::from_secs(4));
    core.set_codex_process_scope_for_test("root".into(), Some("Newer registry failure".into()));
    panel_updated(&mut core);
    answer(
        &peer,
        &mut core,
        0,
        json!({"result":{"data":[process("root","one").data()],"nextCursor":null}}),
    );
    assert_eq!(count(&core), 0);
    assert!(core.codex_task_status().uncertain().contains("root"));
    assert_eq!(
        core.codex_task_status().errors()["root"],
        "Newer registry failure"
    );
    assert!(wait(&core, now + Duration::from_secs(21)).is_none());
    assert!(!tick(&peer, &mut core, now + Duration::from_secs(21)));
    assert_eq!(peer.calls.lock().expect("native calls").len(), 1);
}

fn snapshot() -> CodexSnapshot {
    CodexSnapshot {
        thread: json!({"id":"root","status":{"type":"idle"},"turns":[{"id":"turn",
            "status":"inProgress","items":[{"id":"answer","type":"agentMessage","text":"root answer"}]}]}),
        runtime: json!({"result":{"thread":{"id":"root"},"model":"a","reasoningEffort":"low"}})
            .to_string(),
        catalog: json!({"data":[{"id":"a","model":"a","displayName":"a","isDefault":true}]}),
        queue: Some(json!([])),
        goal: Some(json!({"goal":null})),
        panel: json!({"agents":[],"processes":[],"older":null}),
    }
}
