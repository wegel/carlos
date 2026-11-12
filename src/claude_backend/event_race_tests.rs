//! Exercises interleavings hidden by synchronous app fixtures, using real pipes and queues.

use std::thread;

use serde_json::json;

use super::event_queue::MAX_EVENT_BYTES;
use super::event_race_support::{
    assert_blocks, notify, pause_once, peer, receive, request, through, transport, DEADLINE,
};
use super::event_test_support::{records, submit};
use crate::backend::BackendClient;

#[test]
fn rejected_interrupt_resolves_before_consecutive_result_and_idle() {
    let (client, receiver) = peer();
    let id = submit(&client);
    records(&client, &receiver);
    let result = client.call("turn/interrupt", json!({}), DEADLINE);
    let actual = through(&client, &receiver, "turn/completed");
    assert!(result.is_err());
    let completed = actual.last().expect("completion");
    assert_eq!(
        completed["params"]["turn"],
        json!({"id":id,"status":"completed"})
    );
    assert_eq!(
        actual
            .iter()
            .filter(|line| line["method"] == "turn/completed")
            .count(),
        1
    );
}

#[test]
fn stderr_failure_cannot_split_an_accepted_stdout_batch() {
    let (client, receiver) = peer();
    let id = submit(&client);
    records(&client, &receiver);
    let transport = transport(&client);
    let batch = pause_once(&transport.events.hooks, "batch_line");
    let failure_ready = notify(&transport.events.hooks, "failure_ready");
    request(&client, "burst");
    batch.wait();
    request(&client, "stderr");
    failure_ready
        .recv_timeout(DEADLINE)
        .expect("stderr reader reached emitter");
    let unlocked = transport.normalizer.try_lock().is_ok();
    batch.resume();
    let actual = through(&client, &receiver, "turn/completed");
    assert!(
        unlocked,
        "stdout sender must release native state before sending"
    );
    assert_blocks(&actual);
    let failure = actual
        .iter()
        .position(|line| line["method"] == "claude/error")
        .expect("failure");
    assert!(
        actual[..failure]
            .iter()
            .filter(|line| line["method"] == "item/completed")
            .count()
            == 900
    );
    assert_eq!(
        actual.last().expect("completion")["params"]["turn"],
        json!({"id":id,"status":"failed"})
    );
}

#[test]
fn history_continuation_waits_for_the_whole_stdout_batch() {
    let (client, receiver) = peer();
    client
        .request_native_control("sessions.read", json!({"sessionId":client.session_id()}))
        .expect("history page");
    assert_eq!(
        receive(&client.events_tx, &receiver)["method"],
        "claude/historyFactPart"
    );
    client
        .helper_call("barrier", json!({}), DEADLINE)
        .expect("history batch has released its permit");
    let transport = transport(&client);
    let batch = pause_once(&transport.events.hooks, "batch_line");
    let history_ready = notify(&transport.events.hooks, "history_ready");
    request(&client, "burst");
    batch.wait();
    thread::scope(|scope| {
        let history = scope.spawn(|| transport.emitter.continue_history(None));
        history_ready
            .recv_timeout(DEADLINE)
            .expect("history caller reached emitter");
        batch.resume();
        let actual = through(&client, &receiver, "claude/historyFactPart");
        let result = history.join().expect("history caller");
        result.expect("continuation");
        assert_blocks(&actual);
    });
}

#[test]
fn eof_delivery_releases_normalizer_before_the_queue_send() {
    let (client, receiver) = peer();
    let id = submit(&client);
    records(&client, &receiver);
    let transport = transport(&client);
    let finish = pause_once(&transport.events.hooks, "finish");
    request(&client, "exit");
    finish.wait();
    let unlocked = transport.normalizer.try_lock().is_ok();
    finish.resume();
    let actual = through(&client, &receiver, "turn/completed");
    assert!(
        unlocked,
        "EOF must release the normalizer before terminal queue delivery"
    );
    assert_eq!(
        actual.last().expect("completion")["params"]["turn"],
        json!({"id":id,"status":"failed"})
    );
}

#[test]
fn receiver_drop_cancels_final_delivery_waiting_on_a_full_byte_budget() {
    let (client, receiver) = peer();
    submit(&client);
    records(&client, &receiver);
    let transport = transport(&client);
    let waiting = notify(&transport.events.hooks, "budget_wait");
    let finished = notify(&transport.events.hooks, "finished");
    client
        .events_tx
        .send("x".repeat(MAX_EVENT_BYTES))
        .expect("fill byte budget");
    request(&client, "exit");
    waiting
        .recv_timeout(DEADLINE)
        .expect("final delivery waits for bytes");
    drop(receiver);
    finished
        .recv_timeout(DEADLINE)
        .expect("receiver drop releases terminal worker");
}

#[test]
fn deferred_failure_keeps_native_closed_notice_after_final_completion() {
    let (client, receiver) = peer();
    let id = submit(&client);
    records(&client, &receiver);
    let transport = transport(&client);
    let finish = pause_once(&transport.events.hooks, "finish");
    let waiting = notify(&transport.events.hooks, "budget_wait");
    client
        .events_tx
        .send("x".repeat(MAX_EVENT_BYTES))
        .expect("fill budget");
    client
        .helper_call("stderr", json!({}), DEADLINE)
        .expect("peer wrote stderr");
    finish.wait();
    client
        .helper_call("ignored", json!({}), DEADLINE)
        .expect("ignored SDK frame cannot block a correlated reply behind terminal delivery");
    let closed_ready = notify(&transport.events.hooks, "route_ready");
    request(&client, "closed");
    closed_ready
        .recv_timeout(DEADLINE)
        .expect("stdout reader received closed notice");
    finish.resume();
    waiting
        .recv_timeout(DEADLINE)
        .expect("terminal worker waits for bytes");
    assert!(client
        .helper_call("ignored", json!({}), DEADLINE)
        .expect_err("overflow stopped the helper")
        .to_string()
        .contains("closed"));
    // A closed history caller must leave the consumer free to release the full byte budget.
    request(&client, "history.continue");
    let charged = receiver
        .recv_timeout(DEADLINE)
        .expect("charged queue contents");
    assert_eq!(charged.len(), MAX_EVENT_BYTES);
    client.acknowledge_event(charged.len());
    let actual = through(&client, &receiver, "claude/closed");
    let methods: Vec<&str> = actual
        .iter()
        .map(|line| line["method"].as_str().expect("method"))
        .collect();
    assert_eq!(methods, ["claude/error", "turn/completed", "claude/closed"]);
    assert_eq!(
        actual[1]["params"]["turn"],
        json!({"id":id,"status":"failed"})
    );
    assert_eq!(actual[2]["params"]["reason"], "start_failed");
}
