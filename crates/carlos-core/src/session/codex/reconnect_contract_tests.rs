//! Preserve native replay, partial-output and snapshot failure behavior without AppState.

use std::sync::atomic::Ordering;
use std::time::Instant;

use serde_json::json;

use super::reconnect_contract_support::{
    complete, delta, question, ready, receive, session, snapshot,
};
use crate::session::codex::goal_state::GoalKnowledge;
use crate::session::models::{Role, TurnStatus};
use crate::session::test_backend::TestBackend;
#[test]
fn capture_old_snapshot_then_completion_before_reply_keeps_latest_text_and_terminal_turn() {
    let peer = TestBackend::default();
    let mut state = session();
    let (held, _) = receive(&peer, &mut state, 2, delta(" suffix"));
    receive(&peer, &mut state, 2, complete("prefix suffix"));
    receive(
        &peer,
        &mut state,
        2,
        json!({"method":"turn/completed","params":{"threadId":"root","turn":{"id":"turn","status":"completed"}}}),
    );
    assert!(
        !peer
            .acknowledgements
            .lock()
            .expect("native charges")
            .contains(&held),
        "held frames stay charged"
    );
    ready(&peer, &mut state, snapshot("prefix", "active"));
    assert!(!state.codex_scope().closed());
    assert_eq!(
        state
            .messages()
            .iter()
            .filter(|row| row.role == Role::Assistant)
            .map(|row| row.text.as_str())
            .collect::<Vec<_>>(),
        ["prefix suffix"]
    );
    assert_eq!(state.turn_status_by_id()["turn"], TurnStatus::Completed);
    assert!(state.active_turn_id().is_none());
    assert!(peer
        .acknowledgements
        .lock()
        .expect("native charges")
        .contains(&held));
}

#[test]
fn already_included_delta_never_duplicates_and_late_refresh_cannot_replace_completed_text() {
    let peer = TestBackend::default();
    let mut state = session();
    receive(&peer, &mut state, 2, delta(" suffix"));
    ready(&peer, &mut state, snapshot("prefix suffix", "active"));
    assert_eq!(state.messages()[0].text, "prefix suffix");
    super::reconnect_streams::tick(&peer, &mut state, Instant::now());
    let requests = peer.calls.lock().expect("native controls");
    assert_eq!(requests[0].0, "thread/items/read");
    assert_eq!(
        requests[0].1,
        json!({"threadId":"root","turnId":"turn","itemIds":["answer"]})
    );
    drop(requests);
    let context = peer.calls.lock().expect("native context")[0].2.clone();
    receive(&peer, &mut state, 2, complete("prefix suffix FINAL"));
    receive(
        &peer,
        &mut state,
        2,
        json!({"method":"carlos/codex/controlReply","params":{
        "requestMethod":"thread/items/read","requestParams":{"threadId":"root","turnId":"turn","itemIds":["answer"]},
        "requestContext":context,"response":{"result":{"data":[{"turnId":"turn","item":{"id":"answer","type":"agentMessage","text":"prefix"}}]}}}}),
    );
    assert_eq!(state.messages()[0].text, "prefix suffix FINAL");
}

#[test]
fn old_generation_cannot_resolve_or_revalidate_a_replayed_typed_callback() {
    let peer = TestBackend::default();
    let mut state = session();
    receive(&peer, &mut state, 2, question(json!(9), "root", false));
    receive(&peer, &mut state, 2, question(json!("9"), "child", true));
    receive(
        &peer,
        &mut state,
        1,
        json!({"method":"serverRequest/resolved","params":{"threadId":"root","requestId":9}}),
    );
    receive(
        &peer,
        &mut state,
        1,
        json!({"method":"carlos/codex/formChecked","params":{"requestId":"9","revision":0,"ok":false,"error":"OLD_ERROR"}}),
    );
    assert_eq!(state.codex_callbacks().len(), 2);
    assert!(state
        .codex_callbacks()
        .get(&json!("9"))
        .expect("child callback")
        .schema_error
        .is_none());
    assert!(
        state.codex_scope().closed(),
        "answers can be shown before history is ready"
    );
    let child = state
        .codex_callbacks()
        .get(&json!("9"))
        .expect("typed blocking child");
    assert!(child.blocking);
    assert_eq!(child.params["threadId"], "child");
    assert!(
        !state
            .codex_callbacks()
            .get(&json!(9))
            .expect("numeric root")
            .blocking
    );
}

#[test]
fn shared_disconnect_keeps_native_running_status_without_inventing_failure() {
    let peer = TestBackend::default();
    let mut state = session();
    state.set_codex_socket_for_test(Some("/tmp/own-test.sock".into()));
    state.set_codex_rebuilding_for_test(false);
    state.reopen_codex();
    state.record_turn_started("native-still-running");
    receive(
        &peer,
        &mut state,
        2,
        json!({"method":"carlos/codex/closed","params":{"message":"proxy exited"}}),
    );
    assert!(state.codex_scope().closed());
    assert!(state.active_turn_id().is_none());
    assert_eq!(
        state.turn_status_by_id()["native-still-running"],
        TurnStatus::InProgress
    );
    assert!(state
        .messages()
        .last()
        .expect("visible cause")
        .text
        .contains("connection lost"));
}

#[test]
fn owned_idle_metadata_does_not_restore_a_stale_rollout_turn() {
    let peer = TestBackend::default();
    let mut state = session();
    ready(
        &peer,
        &mut state,
        snapshot("old interrupted output", "idle"),
    );
    assert!(state.active_turn_id().is_none());
    assert!(!state.codex_scope().closed());
}

#[test]
fn malformed_goal_fails_before_transcript_and_snapshots_are_replaced() {
    let peer = TestBackend::default();
    let mut state = session();
    state.append_message(Role::Assistant, "KEEP_OLD_HISTORY");
    state.set_codex_queue_entries_for_test(vec![json!({"id":"keep-queue"})]);
    let mut bad = snapshot("DO_NOT_INSTALL", "idle");
    bad.goal = Some(json!({}));
    ready(&peer, &mut state, bad);
    assert!(state.codex_scope().closed());
    assert_eq!(state.messages()[0].text, "KEEP_OLD_HISTORY");
    assert_eq!(state.codex_queue().entries()[0]["id"], "keep-queue");
    assert_eq!(peer.aborted.load(Ordering::SeqCst), 1);
}

#[test]
fn ambiguous_goal_events_require_a_fresh_read_before_ralph_is_allowed() {
    let peer = TestBackend::default();
    let mut state = session();
    receive(
        &peer,
        &mut state,
        2,
        json!({"method":"thread/goal/cleared","params":{"threadId":"root"}}),
    );
    ready(&peer, &mut state, snapshot("same", "idle"));
    assert_eq!(state.codex_goals().knowledge(), GoalKnowledge::Unknown);
    assert!(state.codex_goals().last_read().is_none());
}

#[test]
fn empty_command_completion_retires_partial_reads_and_fetches_complete_native_output() {
    let peer = TestBackend::default();
    let mut state = session();
    let mut current = snapshot("unused", "active");
    current.thread["turns"][0]["items"][0] = json!({"id":"answer","type":"commandExecution","command":"pwd","aggregatedOutput":"prefix"});
    receive(
        &peer,
        &mut state,
        2,
        json!({"method":"item/commandExecution/outputDelta","params":{"threadId":"root","turnId":"turn","itemId":"answer","delta":" tail"}}),
    );
    ready(&peer, &mut state, current);
    super::reconnect_streams::tick(&peer, &mut state, Instant::now());
    let old = peer.calls.lock().expect("native context")[0].2.clone();
    receive(
        &peer,
        &mut state,
        2,
        json!({"method":"item/completed","params":{"threadId":"root","turnId":"turn","item":{"id":"answer","type":"commandExecution","command":"pwd","exitCode":0}}}),
    );
    super::reconnect_streams::tick(&peer, &mut state, Instant::now());
    let final_context = peer.calls.lock().expect("native context")[1].2.clone();
    assert_ne!(old["token"], final_context["token"]);
    let response = |context, text| {
        json!({"method":"carlos/codex/controlReply","params":{
        "requestContext":context,"requestParams":{"threadId":"root","turnId":"turn","itemIds":["answer"]},
        "response":{"result":{"data":[{"turnId":"turn","completedAtMs":1,"item":{"id":"answer","type":"commandExecution","command":"pwd","aggregatedOutput":text,"exitCode":0}}]}}}})
    };
    receive(&peer, &mut state, 2, response(old, "prefix"));
    receive(&peer, &mut state, 2, response(final_context, "prefix tail"));
    assert!(state.messages()[0].text.contains("prefix tail"));
    assert!(super::reconnect_streams::wait(&state, Instant::now()).is_none());
}

#[test]
fn duplicate_close_preserves_lost_child_state_and_failed_ready_clears_dead_callbacks() {
    let peer = TestBackend::default();
    let mut state = session();
    state.set_codex_socket_for_test(Some("/tmp/fixture.sock".into()));
    receive(&peer, &mut state, 2, question(json!(0), "child", true));
    let closed =
        json!({"method":"carlos/codex/closed","params":{"message":"first transport cause"}});
    receive(&peer, &mut state, 2, closed.clone());
    let count = state.messages().len();
    receive(&peer, &mut state, 2, closed);
    assert_eq!(state.messages().len(), count);
    assert_eq!(state.codex_connection().unreplayed_children(), 1);
    state.set_codex_generation_for_test(3);
    state.set_codex_rebuilding_for_test(true);
    receive(&peer, &mut state, 3, question(json!(0), "root", true));
    let mut malformed = snapshot("must not replace old history", "idle");
    malformed.goal = Some(json!({}));
    *peer.snapshot.lock().expect("native snapshot") = Some(malformed);
    receive(
        &peer,
        &mut state,
        3,
        json!({"method":"carlos/codex/ready","params":{"generation":3,"threadId":"root"}}),
    );
    assert!(state.codex_scope().closed());
    assert!(state.codex_callbacks().is_empty());
    assert!(state.approval().pending().is_none());
    assert_eq!(state.codex_connection().unreplayed_children(), 1);
    receive(
        &peer,
        &mut state,
        3,
        json!({"method":"carlos/codex/closed","params":{}}),
    );
    assert_eq!(state.codex_connection().unreplayed_children(), 1);
    state.set_codex_generation_for_test(4);
    state.set_codex_rebuilding_for_test(true);
    *peer.snapshot.lock().expect("native snapshot") =
        Some(snapshot("restored native history", "idle"));
    receive(
        &peer,
        &mut state,
        4,
        json!({"method":"carlos/codex/ready","params":{"generation":4,"threadId":"root"}}),
    );
    assert!(!state.codex_scope().closed());
    assert!(state
        .messages()
        .iter()
        .any(|message| message.text.contains("unanswered child requests")));
    assert_eq!(state.codex_connection().unreplayed_children(), 0);
}
