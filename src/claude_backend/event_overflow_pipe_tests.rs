//! Checks helper shutdown before deferred terminal facts wait on frame or byte limits.

use serde_json::{json, Value};

use super::event_queue::MAX_EVENT_BYTES;
use super::event_race_support::{notify, peer, through, transport, DEADLINE};
use super::event_test_support::{assert_eof, records, submit};
use super::transport::ControllerTransport;

#[test]
fn stderr_byte_overflow_stops_helper_before_deferring_final_facts() {
    check_overflow(true);
}

#[test]
fn stderr_frame_overflow_stops_helper_before_deferring_final_facts() {
    check_overflow(false);
}

fn check_overflow(bytes: bool) {
    let (client, receiver) = peer();
    let id = submit(&client);
    records(&client, &receiver);
    let transport = transport(&client);
    let deferred = notify(&transport.events.hooks, "terminal_deferred");
    let eof = notify(&transport.events.hooks, "reader_eof");
    let finished = notify(&transport.events.hooks, "finished");
    let count = if bytes { 1 } else { 1024 };
    let filler = if bytes {
        "x".repeat(MAX_EVENT_BYTES)
    } else {
        "filler".to_string()
    };
    for _ in 0..count {
        client
            .events_tx
            .send(filler.clone())
            .expect("fill event queue");
    }
    // A correlated synchronous reply cannot compete with stderr for the full event queue.
    let _ = transport.call("stderr", json!({}), DEADLINE);
    deferred
        .recv_timeout(DEADLINE)
        .expect("terminal worker was deferred");
    eof.recv_timeout(DEADLINE)
        .expect("bounded shutdown closes helper stdout");
    assert_stopped(&transport);
    for _ in 0..count {
        let line = receiver.recv_timeout(DEADLINE).expect("queued filler");
        assert_eq!(line, filler);
        client.events_tx.handled(line.len());
    }
    let actual = through(&client, &receiver, "turn/completed");
    let methods: Vec<&str> = actual
        .iter()
        .map(|line| line["method"].as_str().expect("method"))
        .collect();
    assert_eq!(methods, ["claude/error", "turn/completed"]);
    assert!(actual[0]["params"]["error"]["message"]
        .as_str()
        .expect("error")
        .contains("deterministic stderr failure"));
    assert_eq!(
        actual[1]["params"]["turn"],
        json!({"id":id,"status":"failed"})
    );
    finished
        .recv_timeout(DEADLINE)
        .expect("batch permit released after final facts");
    assert_eof(&client, &receiver);
    assert!(receiver.try_iter().all(|line| {
        let value: Value = serde_json::from_str(&line).expect("JSON");
        value["method"] != "turn/completed"
    }));
}

fn assert_stopped(transport: &ControllerTransport) {
    assert!(transport.state.is_closed());
    assert_eq!(
        transport
            .call("barrier", json!({}), DEADLINE)
            .expect_err("later calls must fail")
            .to_string(),
        "Claude controller is closed"
    );
    assert!(
        transport
            .child
            .lock()
            .expect("child")
            .wait()
            .expect("reap helper")
            .code()
            != Some(0)
    );
    assert!(
        transport.normalizer.try_lock().is_ok(),
        "terminal sender released native state"
    );
}
