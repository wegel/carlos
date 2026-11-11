//! Keep task counts fresh without hidden full-panel reads, stale replies or idle polling.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::codex_question_support::{app, question, receive, Peer};
use crate::app::codex_panel_state::Process;
use crate::app::codex_task_status::{observed, panel_updated, terminated, tick, wait};
use crate::app::state::AppState;
use crate::app::task_badge::count;

/// Build one child whose source is controlled by the test.
pub(super) fn agent(id: &str, state: &str) -> Value {
    json!({"id":id,"parentThreadId":"root","status":{"type":state,"activeFlags":[]}})
}
fn own(app: &mut AppState) {
    app.codex.panel.ownership = crate::app::codex_panel_state::VerifiedOwners::new(
        &app.thread_id,
        app.codex.epoch,
        app.codex
            .panel
            .agents
            .iter()
            .map(|agent| agent["id"].as_str().expect("agent ID").to_string()),
    )
    .expect("verified test owners");
}
/// Build one exact native terminal identity.
pub(super) fn process(owner: &str, id: &str) -> Process {
    Process {
        thread: owner.into(),
        data: json!({"processId":id,"itemId":format!("item-{id}"),"command":"sleep 30","cwd":"/tmp"}),
    }
}
/// Return one targeted control through the shared notification entrypoint.
pub(super) fn answer(peer: &Peer, app: &mut AppState, index: usize, response: Value) {
    receive(
        app,
        json!({"method":"carlos/codex/controlReply","params":{
        "requestMethod":peer.controls.borrow()[index].0,"requestParams":peer.controls.borrow()[index].1,
        "requestContext":peer.contexts.borrow()[index],"response":response}}),
    );
}
fn hint(app: &mut AppState, id: &str) {
    observed(
        app,
        "item/started",
        &json!({"threadId":"root","item":{"type":"commandExecution","id":format!("item-{id}"),"processId":id}}),
    );
}

#[test]
fn task_counts_include_verified_active_or_waiting_children_but_not_old_idle_or_foreign_rows() {
    let mut app = app();
    app.codex.panel.agents = vec![
        agent("running", "active"),
        agent("idle", "idle"),
        agent("error", "systemError"),
        agent("gone", "notLoaded"),
    ];
    own(&mut app);
    app.codex.panel.processes = vec![
        process("root", "one"),
        process("running", "two"),
        process("foreign", "ignore"),
    ];
    assert_eq!(count(&app), 4);
    receive(&mut app, question(json!("child-question"), "idle", false));
    assert_eq!(count(&app), 5);
    receive(&mut app, question(json!("root-question"), "root", false));
    assert_eq!(count(&app), 5, "root callbacks are not background tasks");
    receive(
        &mut app,
        json!({"method":"serverRequest/resolved","params":{"requestId":"child-question","threadId":"idle"}}),
    );
    assert_eq!(count(&app), 4);
    receive(
        &mut app,
        json!({"method":"turn/completed","params":{"threadId":"running","turn":{"id":"done","status":"completed"}}}),
    );
    assert_eq!(
        count(&app),
        2,
        "the settled agent and its uncertain old process no longer count"
    );
    receive(
        &mut app,
        json!({"method":"thread/status/changed","params":{"threadId":"running","status":{"type":"active","activeFlags":[]}}}),
    );
    assert_eq!(
        count(&app),
        3,
        "a positively new active state clears the old terminal turn"
    );
}

#[test]
fn no_startup_reads_and_no_idle_timer_remain_after_a_foreground_command_has_no_background_process()
{
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    assert!(!tick(&peer, &mut app, now));
    assert!(wait(&app, now).is_none());
    hint(&mut app, "foreground");
    assert!(app.codex.task_status.errors.is_empty());
    assert_eq!(count(&app), 0);
    assert!(tick(&peer, &mut app, now));
    assert_eq!(
        peer.controls.borrow()[0].0,
        "thread/backgroundTerminals/list"
    );
    assert!(wait(&app, now).expect("pending read deadline") > Duration::ZERO);
    answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"data":[],"nextCursor":null}}),
    );
    assert!(wait(&app, now).is_none());
    assert!(!tick(&peer, &mut app, now + Duration::from_secs(60)));
    assert_eq!(peer.controls.borrow().len(), 1);
}

#[test]
fn live_processes_refresh_by_owner_then_stop_polling_when_native_membership_becomes_empty() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    app.codex.panel.processes.push(process("root", "one"));
    panel_updated(&mut app);
    assert!(!tick(&peer, &mut app, now));
    assert!(tick(&peer, &mut app, now + Duration::from_secs(4)));
    assert_eq!(peer.controls.borrow()[0].1["threadId"], "root");
    assert!(!peer
        .controls
        .borrow()
        .iter()
        .any(|(method, _)| method == "thread/list"));
    answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"data":[],"nextCursor":null}}),
    );
    assert_eq!(count(&app), 0);
    assert!(app.codex.panel.processes.is_empty());
    assert!(wait(&app, now + Duration::from_secs(4)).is_none());
}

#[test]
fn terminal_observation_and_newer_f2_snapshot_fence_older_targeted_replies() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    let old = process("root", "one");
    let data = old.data.clone();
    app.codex.panel.processes.push(old);
    hint(&mut app, "one");
    tick(&peer, &mut app, now);
    terminated(&mut app, "root", "one", Some("item-one"));
    answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"data":[data.clone()],"nextCursor":null}}),
    );
    assert_eq!(count(&app), 0);
    assert!(app.codex.panel.processes.is_empty());
    tick(&peer, &mut app, now + Duration::from_secs(4));
    app.codex.panel.processes = vec![process("root", "new")];
    app.codex.panel.process_scopes.insert("root".into(), None);
    panel_updated(&mut app);
    answer(
        &peer,
        &mut app,
        1,
        json!({"result":{"data":[data],"nextCursor":null}}),
    );
    assert_eq!(app.codex.panel.processes.len(), 1);
    assert_eq!(app.codex.panel.processes[0].data["processId"], "new");
}

#[test]
fn errors_keep_the_reason_in_f2_and_stop_automatic_retry_until_new_native_work() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    app.codex.panel.agents.push(agent("child", "active"));
    own(&mut app);
    app.codex.panel.selected = Some(crate::app::codex_panel_state::PanelId::Agent(
        "child".into(),
    ));
    app.codex.panel.processes.push(process("root", "one"));
    hint(&mut app, "one");
    tick(&peer, &mut app, now);
    answer(
        &peer,
        &mut app,
        0,
        json!({"error":{"code":-32000,"message":"Fixture process registry unavailable"}}),
    );
    assert_eq!(
        count(&app),
        1,
        "the active child remains, but the uncertain process is excluded"
    );
    assert!(crate::app::codex_panel_text::body(&app)
        .join("\n")
        .contains("Fixture process registry unavailable"));
    assert!(wait(&app, now + Duration::from_secs(20)).is_none());
    assert!(!tick(&peer, &mut app, now + Duration::from_secs(20)));
    assert_eq!(peer.controls.borrow().len(), 1);
    hint(&mut app, "one");
    tick(&peer, &mut app, now + Duration::from_secs(21));
    answer(
        &peer,
        &mut app,
        1,
        json!({"result":{"data":[],"nextCursor":null}}),
    );
    assert!(app.codex.task_status.errors.is_empty());
}

#[test]
fn cache_limit_failure_preserves_details_but_excludes_uncertain_counts_and_exposes_reason() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    app.codex.panel.agents.push(agent("child", "idle"));
    own(&mut app);
    app.codex.panel.processes = (0..512)
        .map(|index| process("child", &index.to_string()))
        .collect();
    hint(&mut app, "new");
    tick(&peer, &mut app, now);
    answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"data":[process("root","new").data],"nextCursor":null}}),
    );
    assert_eq!(app.codex.panel.processes.len(), 512);
    assert!(!app
        .codex
        .panel
        .processes
        .iter()
        .any(|row| row.thread == "root"));
    assert!(app.codex.task_status.uncertain.contains("root"));
    assert!(crate::app::codex_panel_text::body(&app)
        .join("\n")
        .contains("512-record/8 MiB bound"));
}

#[test]
fn old_root_epoch_and_generation_replies_cannot_install_process_rows() {
    for field in ["root", "epoch", "generation"] {
        let mut app = app();
        let peer = Peer::default();
        hint(&mut app, "one");
        tick(&peer, &mut app, Instant::now());
        peer.contexts.borrow_mut()[0][field] = if field == "root" {
            json!("old-root")
        } else {
            json!(99)
        };
        answer(
            &peer,
            &mut app,
            0,
            json!({"result":{"data":[process("root","one").data],"nextCursor":null}}),
        );
        assert!(app.codex.panel.processes.is_empty());
    }
}

#[test]
fn repeated_native_hints_respect_the_per_owner_interval_and_do_not_spin_while_waiting() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    hint(&mut app, "one");
    tick(&peer, &mut app, now);
    answer(
        &peer,
        &mut app,
        0,
        json!({"result":{"data":[],"nextCursor":null}}),
    );
    for _ in 0..10 {
        hint(&mut app, "one");
        tick(&peer, &mut app, now + Duration::from_millis(100));
    }
    assert_eq!(peer.controls.borrow().len(), 1);
    assert!(wait(&app, now + Duration::from_millis(100)).expect("rate limit") > Duration::ZERO);
    tick(&peer, &mut app, now + Duration::from_secs(3));
    assert_eq!(peer.controls.borrow().len(), 2);
    hint(&mut app, "one");
    assert!(wait(&app, now + Duration::from_secs(3)).expect("pending deadline") > Duration::ZERO);
}

#[test]
fn actual_targeted_timeout_retains_old_rows_but_clears_counts_and_stops_idle_retry() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    app.codex.panel.processes.push(process("root", "one"));
    hint(&mut app, "one");
    tick(&peer, &mut app, now);
    assert!(tick(&peer, &mut app, now + Duration::from_secs(21)));
    assert_eq!(app.codex.panel.processes.len(), 1);
    assert_eq!(count(&app), 0);
    assert!(app.codex.task_status.uncertain.contains("root"));
    assert!(crate::app::codex_panel_text::body(&app)
        .join("\n")
        .contains("refresh timed out"));
    assert!(wait(&app, now + Duration::from_secs(21)).is_none());
    assert!(!tick(&peer, &mut app, now + Duration::from_secs(60)));
    assert_eq!(peer.controls.borrow().len(), 1);
}

#[test]
fn another_owners_success_does_not_erase_a_failed_owners_reason_or_uncertainty() {
    let mut app = app();
    let peer = Peer::default();
    let now = Instant::now();
    app.codex.panel.agents.push(agent("child", "active"));
    own(&mut app);
    hint(&mut app, "one");
    tick(&peer, &mut app, now);
    answer(
        &peer,
        &mut app,
        0,
        json!({"error":{"code":-32000,"message":"Root registry unavailable"}}),
    );
    observed(
        &mut app,
        "item/started",
        &json!({"threadId":"child","item":{"type":"commandExecution","id":"child-call","processId":"child-process"}}),
    );
    tick(&peer, &mut app, now);
    answer(
        &peer,
        &mut app,
        1,
        json!({"result":{"data":[],"nextCursor":null}}),
    );
    assert!(app.codex.task_status.errors.contains_key("root"));
    assert!(app.codex.task_status.uncertain.contains("root"));
    assert!(crate::app::codex_panel_text::body(&app)
        .join("\n")
        .contains("Root registry unavailable"));
}
