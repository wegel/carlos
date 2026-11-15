//! Keep targeted native task reads bounded and preserve each owner error without terminal state.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::task_status_test_support::{
    agent, answer, count, feed, hint, observed, overview, own, panel_updated, process, session,
    terminated, tick, wait,
};
use crate::session::test_backend::TestBackend;

#[test]
fn task_counts_include_verified_active_or_waiting_children_but_not_old_idle_or_foreign_rows() {
    let mut core = session();
    core.set_codex_agents_for_test(vec![
        agent("running", "active"),
        agent("idle", "idle"),
        agent("error", "systemError"),
        agent("gone", "notLoaded"),
    ]);
    own(&mut core);
    core.set_codex_processes_for_test(vec![
        process("root", "one"),
        process("running", "two"),
        process("foreign", "ignore"),
    ]);
    assert_eq!(count(&core), 4);
    core.accept_codex_callback(
        "item/tool/requestUserInput",
        &question("child-question", "idle"),
    );
    assert_eq!(count(&core), 5);
    core.accept_codex_callback(
        "item/tool/requestUserInput",
        &question("root-question", "root"),
    );
    assert_eq!(count(&core), 5, "root callbacks are not background tasks");
    core.resolve_codex_callback(&json!({"requestId":"child-question","threadId":"idle"}));
    assert_eq!(count(&core), 4);
    feed(
        &mut core,
        "turn/completed",
        json!({"threadId":"running","turn":{"id":"done","status":"completed"}}),
    );
    assert_eq!(
        count(&core),
        2,
        "the settled agent and its uncertain old process no longer count"
    );
    feed(
        &mut core,
        "thread/status/changed",
        json!({"threadId":"running","status":{"type":"active","activeFlags":[]}}),
    );
    assert_eq!(
        count(&core),
        3,
        "a positively new active state clears the old terminal turn"
    );
}

#[test]
fn no_startup_reads_and_no_idle_timer_remain_after_a_foreground_command_has_no_background_process()
{
    let mut core = session();
    let peer = TestBackend::default();
    let now = Instant::now();
    assert!(!tick(&peer, &mut core, now));
    assert!(wait(&core, now).is_none());
    hint(&mut core, "foreground");
    assert!(core.codex_task_status().errors().is_empty());
    assert_eq!(count(&core), 0);
    assert!(tick(&peer, &mut core, now));
    assert_eq!(
        peer.calls.lock().expect("native calls")[0].0,
        "thread/backgroundTerminals/list"
    );
    assert!(wait(&core, now).expect("pending read deadline") > Duration::ZERO);
    answer(
        &peer,
        &mut core,
        0,
        json!({"result":{"data":[],"nextCursor":null}}),
    );
    assert!(wait(&core, now).is_none());
    assert!(!tick(&peer, &mut core, now + Duration::from_secs(60)));
    assert_eq!(peer.calls.lock().expect("native calls").len(), 1);
}

#[test]
fn live_processes_refresh_by_owner_then_stop_polling_when_native_membership_becomes_empty() {
    let mut core = session();
    let peer = TestBackend::default();
    let now = Instant::now();
    core.push_codex_process_for_test(process("root", "one"));
    panel_updated(&mut core);
    assert!(!tick(&peer, &mut core, now));
    assert!(tick(&peer, &mut core, now + Duration::from_secs(4)));
    assert_eq!(
        peer.calls.lock().expect("native calls")[0].1["threadId"],
        "root"
    );
    assert!(!peer
        .calls
        .lock()
        .expect("native calls")
        .iter()
        .any(|(method, _, _)| method == "thread/list"));
    answer(
        &peer,
        &mut core,
        0,
        json!({"result":{"data":[],"nextCursor":null}}),
    );
    assert_eq!(count(&core), 0);
    assert!(core.codex_panel().processes().is_empty());
    assert!(wait(&core, now + Duration::from_secs(4)).is_none());
}

#[test]
fn terminal_observation_and_newer_f2_snapshot_fence_older_targeted_replies() {
    let mut core = session();
    let peer = TestBackend::default();
    let now = Instant::now();
    let old = process("root", "one");
    let data = old.data().clone();
    core.push_codex_process_for_test(old);
    hint(&mut core, "one");
    tick(&peer, &mut core, now);
    terminated(&mut core, "root", "one", Some("item-one"));
    answer(
        &peer,
        &mut core,
        0,
        json!({"result":{"data":[data.clone()],"nextCursor":null}}),
    );
    assert_eq!(count(&core), 0);
    assert!(core.codex_panel().processes().is_empty());
    tick(&peer, &mut core, now + Duration::from_secs(4));
    core.set_codex_processes_for_test(vec![process("root", "new")]);
    core.set_codex_process_scope_for_test("root".into(), None);
    panel_updated(&mut core);
    answer(
        &peer,
        &mut core,
        1,
        json!({"result":{"data":[data],"nextCursor":null}}),
    );
    assert_eq!(core.codex_panel().processes().len(), 1);
    assert_eq!(core.codex_panel().processes()[0].data()["processId"], "new");
}

#[test]
fn errors_keep_the_reason_in_f2_and_stop_automatic_retry_until_new_native_work() {
    let mut core = session();
    let peer = TestBackend::default();
    let now = Instant::now();
    core.push_codex_agent_for_test(agent("child", "active"));
    own(&mut core);
    core.push_codex_process_for_test(process("root", "one"));
    hint(&mut core, "one");
    tick(&peer, &mut core, now);
    answer(
        &peer,
        &mut core,
        0,
        json!({"error":{"code":-32000,"message":"Fixture process registry unavailable"}}),
    );
    assert_eq!(
        count(&core),
        1,
        "the active child remains, but the uncertain process is excluded"
    );
    assert!(overview(&core).contains("Fixture process registry unavailable"));
    assert!(wait(&core, now + Duration::from_secs(20)).is_none());
    assert!(!tick(&peer, &mut core, now + Duration::from_secs(20)));
    assert_eq!(peer.calls.lock().expect("native calls").len(), 1);
    hint(&mut core, "one");
    tick(&peer, &mut core, now + Duration::from_secs(21));
    answer(
        &peer,
        &mut core,
        1,
        json!({"result":{"data":[],"nextCursor":null}}),
    );
    assert!(core.codex_task_status().errors().is_empty());
}

#[test]
fn cache_limit_failure_preserves_details_but_excludes_uncertain_counts_and_exposes_reason() {
    let mut core = session();
    let peer = TestBackend::default();
    let now = Instant::now();
    core.push_codex_agent_for_test(agent("child", "idle"));
    own(&mut core);
    core.set_codex_processes_for_test(
        (0..512)
            .map(|index| process("child", &index.to_string()))
            .collect(),
    );
    hint(&mut core, "new");
    tick(&peer, &mut core, now);
    answer(
        &peer,
        &mut core,
        0,
        json!({"result":{"data":[process("root","new").data()],"nextCursor":null}}),
    );
    assert_eq!(core.codex_panel().processes().len(), 512);
    assert!(!core
        .codex_panel()
        .processes()
        .iter()
        .any(|row| row.thread() == "root"));
    assert!(core.codex_task_status().uncertain().contains("root"));
    assert!(overview(&core).contains("512-record/8 MiB bound"));
}

#[test]
fn old_root_epoch_and_generation_replies_cannot_install_process_rows() {
    for field in ["root", "epoch", "generation"] {
        let mut core = session();
        let peer = TestBackend::default();
        hint(&mut core, "one");
        tick(&peer, &mut core, Instant::now());
        peer.calls.lock().expect("native calls")[0].2[field] = if field == "root" {
            json!("old-root")
        } else {
            json!(99)
        };
        answer(
            &peer,
            &mut core,
            0,
            json!({"result":{"data":[process("root","one").data()],"nextCursor":null}}),
        );
        assert!(core.codex_panel().processes().is_empty());
    }
}

#[test]
fn repeated_native_hints_respect_the_per_owner_interval_and_do_not_spin_while_waiting() {
    let mut core = session();
    let peer = TestBackend::default();
    let now = Instant::now();
    hint(&mut core, "one");
    tick(&peer, &mut core, now);
    answer(
        &peer,
        &mut core,
        0,
        json!({"result":{"data":[],"nextCursor":null}}),
    );
    for _ in 0..10 {
        hint(&mut core, "one");
        tick(&peer, &mut core, now + Duration::from_millis(100));
    }
    assert_eq!(peer.calls.lock().expect("native calls").len(), 1);
    assert!(wait(&core, now + Duration::from_millis(100)).expect("rate limit") > Duration::ZERO);
    tick(&peer, &mut core, now + Duration::from_secs(3));
    assert_eq!(peer.calls.lock().expect("native calls").len(), 2);
    hint(&mut core, "one");
    assert!(wait(&core, now + Duration::from_secs(3)).expect("pending deadline") > Duration::ZERO);
}

#[test]
fn actual_targeted_timeout_retains_old_rows_but_clears_counts_and_stops_idle_retry() {
    let mut core = session();
    let peer = TestBackend::default();
    let now = Instant::now();
    core.push_codex_process_for_test(process("root", "one"));
    hint(&mut core, "one");
    tick(&peer, &mut core, now);
    assert!(tick(&peer, &mut core, now + Duration::from_secs(21)));
    assert_eq!(core.codex_panel().processes().len(), 1);
    assert_eq!(count(&core), 0);
    assert!(core.codex_task_status().uncertain().contains("root"));
    assert!(overview(&core).contains("refresh timed out"));
    assert!(wait(&core, now + Duration::from_secs(21)).is_none());
    assert!(!tick(&peer, &mut core, now + Duration::from_secs(60)));
    assert_eq!(peer.calls.lock().expect("native calls").len(), 1);
}

#[test]
fn another_owners_success_does_not_erase_a_failed_owners_reason_or_uncertainty() {
    let mut core = session();
    let peer = TestBackend::default();
    let now = Instant::now();
    core.push_codex_agent_for_test(agent("child", "active"));
    own(&mut core);
    hint(&mut core, "one");
    tick(&peer, &mut core, now);
    answer(
        &peer,
        &mut core,
        0,
        json!({"error":{"code":-32000,"message":"Root registry unavailable"}}),
    );
    observed(
        &mut core,
        "item/started",
        &json!({"threadId":"child","item":{"type":"commandExecution","id":"child-call","processId":"child-process"}}),
    );
    tick(&peer, &mut core, now);
    answer(
        &peer,
        &mut core,
        1,
        json!({"result":{"data":[],"nextCursor":null}}),
    );
    assert!(core.codex_task_status().errors().contains_key("root"));
    assert!(core.codex_task_status().uncertain().contains("root"));
    assert!(overview(&core).contains("Root registry unavailable"));
}

fn question(id: &str, owner: &str) -> Value {
    json!({"id":id,"params":{"threadId":owner,"turnId":"request-turn","itemId":"question-tool",
        "isBlocking":false,"questions":[{"id":"native-q-7","header":"Native question",
        "question":"Do not use this prose as the answer key","isSecret":false,"options":null}]}})
}
