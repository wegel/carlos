//! Exercise native snapshot/event races, exact generations and recoverable reconnect failures.

use std::time::Instant;

use serde_json::json;

use super::codex_question_support::question;
use super::codex_reconnect_support::{app, complete, delta, ready, receive, snapshot, Peer};

use carlos_core::session::codex::callbacks::CallbackKey;
use carlos_core::session::codex::goal_state::GoalKnowledge;
use carlos_core::session::models::Role;
use carlos_core::session::models::TurnStatus;

#[test]
fn capture_old_snapshot_then_completion_before_reply_keeps_latest_text_and_terminal_turn() {
    let peer = Peer::default();
    let mut app = app();
    let held = receive(&peer, &mut app, 2, delta(" suffix"));
    receive(&peer, &mut app, 2, complete("prefix suffix"));
    receive(
        &peer,
        &mut app,
        2,
        json!({"method":"turn/completed","params":{"threadId":"root","turn":{"id":"turn","status":"completed"}}}),
    );
    assert!(
        !peer.acknowledged.borrow().contains(&held),
        "held frames stay charged"
    );
    ready(&peer, &mut app, snapshot("prefix", "active"));
    assert!(!app.session.codex_scope().closed());
    assert_eq!(
        app.session
            .messages()
            .iter()
            .filter(|row| row.role == Role::Assistant)
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>(),
        ["prefix suffix"]
    );
    assert_eq!(
        app.session.turn_status_by_id()["turn"],
        TurnStatus::Completed
    );
    assert!(app.session.active_turn_id().is_none());
    assert!(peer.acknowledged.borrow().contains(&held));
    assert_eq!(app.input_text(), "preserve main 日本語");
}

#[test]
fn already_included_delta_never_duplicates_and_late_refresh_cannot_replace_completed_text() {
    let peer = Peer::default();
    let mut app = app();
    receive(&peer, &mut app, 2, delta(" suffix"));
    ready(&peer, &mut app, snapshot("prefix suffix", "active"));
    assert_eq!(app.session.messages()[0].text, "prefix suffix");
    crate::app::codex_reconnect_streams::tick(&peer, &mut app, Instant::now());
    let requests = peer.native.controls.borrow();
    assert_eq!(requests[0].0, "thread/items/read");
    assert_eq!(
        requests[0].1,
        json!({"threadId":"root","turnId":"turn","itemIds":["answer"]})
    );
    drop(requests);
    let context = peer.native.contexts.borrow()[0].clone();
    receive(&peer, &mut app, 2, complete("prefix suffix FINAL"));
    receive(
        &peer,
        &mut app,
        2,
        json!({"method":"carlos/codex/controlReply","params":{
        "requestMethod":"thread/items/read","requestParams":{"threadId":"root","turnId":"turn","itemIds":["answer"]},
        "requestContext":context,"response":{"result":{"data":[{"turnId":"turn","item":{"id":"answer","type":"agentMessage","text":"prefix"}}]}}}}),
    );
    assert_eq!(app.session.messages()[0].text, "prefix suffix FINAL");
}

#[test]
fn old_generation_cannot_resolve_or_revalidate_a_replayed_typed_callback() {
    let peer = Peer::default();
    let mut app = app();
    receive(&peer, &mut app, 2, question(json!(9), "root", false));
    receive(&peer, &mut app, 2, question(json!("9"), "child", true));
    receive(
        &peer,
        &mut app,
        1,
        json!({"method":"serverRequest/resolved","params":{"threadId":"root","requestId":9}}),
    );
    receive(
        &peer,
        &mut app,
        1,
        json!({"method":"carlos/codex/formChecked","params":{"requestId":"9","revision":0,"ok":false,"error":"OLD_ERROR"}}),
    );
    assert_eq!(app.session.codex_callbacks().len(), 2);
    assert!(app
        .view
        .codex_callbacks
        .drafts
        .get(&CallbackKey::new(&json!("9")).expect("typed id"))
        .expect("child callback")
        .unsupported
        .is_none());
    assert!(
        app.session.codex_scope().closed(),
        "answers can be shown before history is ready"
    );
    assert_eq!(
        app.session
            .approval()
            .pending()
            .expect("blocking child")
            .facts()
            .request_id,
        json!("9")
    );
}

#[test]
fn shared_disconnect_keeps_native_running_status_without_inventing_failure() {
    let peer = Peer::default();
    let mut app = app();
    app.session
        .set_codex_socket_for_test(Some("/tmp/own-test.sock".into()));
    app.session.set_codex_rebuilding_for_test(false);
    app.session.reopen_codex();
    app.record_turn_started("native-still-running");
    receive(
        &peer,
        &mut app,
        2,
        json!({"method":"carlos/codex/closed","params":{"message":"proxy exited"}}),
    );
    assert!(app.session.codex_scope().closed());
    assert!(app.session.active_turn_id().is_none());
    assert_eq!(
        app.session.turn_status_by_id()["native-still-running"],
        TurnStatus::InProgress
    );
    assert!(app
        .session
        .messages()
        .last()
        .expect("visible cause")
        .text
        .contains("connection lost"));
    assert_eq!(app.input_text(), "preserve main 日本語");
}

#[test]
fn owned_idle_metadata_does_not_restore_a_stale_rollout_turn() {
    let peer = Peer::default();
    let mut app = app();
    ready(&peer, &mut app, snapshot("old interrupted output", "idle"));
    assert!(app.session.active_turn_id().is_none());
    assert!(!app.session.codex_scope().closed());
}

#[test]
fn malformed_goal_fails_before_transcript_and_snapshots_are_replaced() {
    let peer = Peer::default();
    let mut app = app();
    app.append_message(Role::Assistant, "KEEP_OLD_HISTORY");
    app.session
        .set_codex_queue_entries_for_test(vec![json!({"id":"keep-queue"})]);
    let mut bad = snapshot("DO_NOT_INSTALL", "idle");
    bad.goal = Some(json!({}));
    ready(&peer, &mut app, bad);
    assert!(app.session.codex_scope().closed());
    assert_eq!(app.session.messages()[0].text, "KEEP_OLD_HISTORY");
    assert_eq!(app.session.codex_queue().entries()[0]["id"], "keep-queue");
    assert_eq!(*peer.aborted.borrow(), 1);
    assert_eq!(app.input_text(), "preserve main 日本語");
}

#[test]
fn ambiguous_goal_events_require_a_fresh_read_before_ralph_is_allowed() {
    let peer = Peer::default();
    let mut app = app();
    receive(
        &peer,
        &mut app,
        2,
        json!({"method":"thread/goal/cleared","params":{"threadId":"root"}}),
    );
    ready(&peer, &mut app, snapshot("same", "idle"));
    assert_eq!(
        app.session.codex_goals().knowledge(),
        GoalKnowledge::Unknown
    );
    assert!(app.session.codex_goals().last_read().is_none());
}

#[test]
fn due_quota_does_not_write_or_spin_before_ready_then_refreshes_once() {
    let peer = Peer::default();
    let mut app = app();
    let now = Instant::now();
    app.tick_rate_limits(&peer, now - std::time::Duration::from_secs(400));
    peer.native.controls.borrow_mut().clear();
    assert!(!crate::app::input::tick_rate_limits(&peer, &mut app, now));
    assert!(crate::app::input::idle_wait(&app, now).is_none());
    assert!(peer.native.controls.borrow().is_empty());
    ready(&peer, &mut app, snapshot("fresh", "idle"));
    app.session.set_codex_runtime_refresh_for_test(false);
    assert!(crate::app::input::tick_rate_limits(&peer, &mut app, now));
    crate::app::input::tick_rate_limits(&peer, &mut app, now);
    assert_eq!(peer.native.controls.borrow().len(), 1);
    assert_eq!(
        peer.native.controls.borrow()[0].0,
        "account/rateLimits/read"
    );
    assert!(
        crate::app::input::idle_wait(&app, now).expect("future quota timer")
            > std::time::Duration::ZERO
    );
}

#[test]
fn empty_command_completion_retires_partial_reads_and_fetches_complete_native_output() {
    let peer = Peer::default();
    let mut app = app();
    let mut current = snapshot("unused", "active");
    current.thread["turns"][0]["items"][0] = json!({"id":"answer","type":"commandExecution","command":"pwd","aggregatedOutput":"prefix"});
    receive(
        &peer,
        &mut app,
        2,
        json!({"method":"item/commandExecution/outputDelta","params":{"threadId":"root","turnId":"turn","itemId":"answer","delta":" tail"}}),
    );
    ready(&peer, &mut app, current);
    crate::app::codex_reconnect_streams::tick(&peer, &mut app, Instant::now());
    let old = peer.native.contexts.borrow()[0].clone();
    receive(
        &peer,
        &mut app,
        2,
        json!({"method":"item/completed","params":{"threadId":"root","turnId":"turn","item":{"id":"answer","type":"commandExecution","command":"pwd","exitCode":0}}}),
    );
    crate::app::codex_reconnect_streams::tick(&peer, &mut app, Instant::now());
    let final_context = peer.native.contexts.borrow()[1].clone();
    assert_ne!(old["token"], final_context["token"]);
    let response = |context, text| {
        json!({"method":"carlos/codex/controlReply","params":{
        "requestContext":context,"requestParams":{"threadId":"root","turnId":"turn","itemIds":["answer"]},
        "response":{"result":{"data":[{"turnId":"turn","completedAtMs":1,"item":{"id":"answer","type":"commandExecution","command":"pwd","aggregatedOutput":text,"exitCode":0}}]}}}})
    };
    receive(&peer, &mut app, 2, response(old, "prefix"));
    receive(&peer, &mut app, 2, response(final_context, "prefix tail"));
    assert!(app.session.messages()[0].text.contains("prefix tail"));
    assert!(crate::app::codex_reconnect_streams::wait(&app, Instant::now()).is_none());
}

#[test]
fn duplicate_close_preserves_lost_child_state_and_failed_ready_clears_dead_callbacks() {
    let peer = Peer::default();
    let mut app = app();
    app.session
        .set_codex_socket_for_test(Some("/tmp/fixture.sock".into()));
    receive(&peer, &mut app, 2, question(json!(0), "child", true));
    let closed =
        json!({"method":"carlos/codex/closed","params":{"message":"first transport cause"}});
    receive(&peer, &mut app, 2, closed.clone());
    let count = app.session.messages().len();
    receive(&peer, &mut app, 2, closed);
    assert_eq!(app.session.messages().len(), count);
    assert_eq!(app.session.codex_connection().unreplayed_children(), 1);
    app.session.set_codex_generation_for_test(3);
    app.session.set_codex_rebuilding_for_test(true);
    receive(&peer, &mut app, 3, question(json!(0), "root", true));
    let mut malformed = snapshot("must not replace old history", "idle");
    malformed.goal = Some(json!({}));
    *peer.snapshot.borrow_mut() = Some(malformed);
    receive(
        &peer,
        &mut app,
        3,
        json!({"method":"carlos/codex/ready","params":{"generation":3,"threadId":"root"}}),
    );
    assert!(app.session.codex_scope().closed());
    assert!(app.session.codex_callbacks().is_empty());
    assert!(app.session.approval().pending().is_none());
    assert_eq!(app.session.codex_connection().unreplayed_children(), 1);
    assert_eq!(app.input_text(), "preserve main 日本語");
    receive(
        &peer,
        &mut app,
        3,
        json!({"method":"carlos/codex/closed","params":{}}),
    );
    assert_eq!(app.session.codex_connection().unreplayed_children(), 1);
    app.session.set_codex_generation_for_test(4);
    app.session.set_codex_rebuilding_for_test(true);
    *peer.snapshot.borrow_mut() = Some(snapshot("restored native history", "idle"));
    receive(
        &peer,
        &mut app,
        4,
        json!({"method":"carlos/codex/ready","params":{"generation":4,"threadId":"root"}}),
    );
    assert!(!app.session.codex_scope().closed());
    assert!(app
        .session
        .messages()
        .iter()
        .any(|message| message.text.contains("unanswered child requests")));
    assert_eq!(app.session.codex_connection().unreplayed_children(), 0);
    assert_eq!(app.input_text(), "preserve main 日本語");
}
