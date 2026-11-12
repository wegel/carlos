//! Reproduces review schedules through real pipes, reader threads, and the production forwarder.

use std::sync::mpsc::Receiver;
use std::sync::Arc;
use std::thread;

use serde_json::json;

use super::event_forwarder_support::{apply, completions, forward, mark, peer, ready, through};
use super::event_queue::MAX_EVENT_BYTES;
use super::event_race_support::{notify, transport, DEADLINE};
use super::event_test_support::submit;
use super::transport::ControllerTransport;
use super::ClaudeClient;
use crate::backend::{BackendClient, EventReceiver};
use crate::event::UiEvent;

#[test]
fn forwarded_result_idle_reset_reply_preserves_completion() {
    check_later("reset");
}

#[test]
fn forwarded_result_idle_second_prompt_reply_preserves_completion() {
    check_later("second");
}

fn check_later(later: &str) {
    for idle_first in [false, true] {
        for accepted in [false, true] {
            let pair = if idle_first {
                "idle,result"
            } else {
                "result,idle"
            };
            let (client, receiver) = peer(&format!("{pair},{later},reply"), accepted);
            let ui = forward(receiver);
            let id = ready(&client, &ui);
            if later == "second" {
                submit(&client);
                mark(&client, "submitted");
                through(&client, &ui, "submitted");
            }
            let reply = client.call("turn/interrupt", json!({}), DEADLINE);
            assert_eq!(reply.is_ok(), accepted);
            mark(&client, "done");
            let output = through(&client, &ui, "done");
            assert_eq!(
                completions(&output),
                [json!({"id":id,"status":if accepted {"interrupted"} else {"completed"}})]
            );
            let completed = output
                .iter()
                .position(|f| f["method"] == "turn/completed")
                .expect("completion");
            let after = output
                .iter()
                .position(|f| {
                    f["method"]
                        == if later == "reset" {
                            "claude/conversationReset"
                        } else {
                            "turn/started"
                        }
                })
                .expect("later fact");
            assert!(
                completed < after,
                "completion keeps its native stream position"
            );
        }
    }
}

#[test]
fn forwarded_reader_ahead_interrupt_captures_handled_prompt() {
    let (client, receiver) = peer("reply", true);
    let ui = forward(receiver);
    let id = ready(&client, &ui);
    client
        .helper_call("queued", json!({}), DEADLINE)
        .expect("reader has read result and idle");
    // Both lines may already be in ui; neither has reached the handler-owned lifecycle.
    client
        .call("turn/interrupt", json!({}), DEADLINE)
        .expect("accepted interrupt");
    mark(&client, "done");
    assert_eq!(
        completions(&through(&client, &ui, "done")),
        [json!({"id":id,"status":"interrupted"})]
    );
}

#[test]
fn forwarded_result_rejected_reply_idle_preserves_success() {
    let (client, receiver) = peer("result,reply,idle", false);
    let ui = forward(receiver);
    let id = ready(&client, &ui);
    client
        .call("turn/interrupt", json!({}), DEADLINE)
        .expect_err("rejected interrupt");
    mark(&client, "done");
    assert_eq!(
        completions(&through(&client, &ui, "done")),
        [json!({"id":id,"status":"completed"})]
    );
}

#[test]
fn forwarded_stderr_byte_limit_with_unanswered_interrupt_stops_helper() {
    check_overflow(true);
}

#[test]
fn forwarded_stderr_frame_limit_with_unanswered_interrupt_stops_helper() {
    check_overflow(false);
}

fn check_overflow(bytes: bool) {
    let (client, receiver) = peer("barrier", false);
    let id = submit(&client);
    super::event_test_support::records(&client, &receiver);
    let transport = transport(&client);
    let (receiver, filler, count) = thread::scope(|scope| {
        let observed = transport.clone();
        let witness = scope.spawn(move || saturate_unanswered(observed, receiver, bytes));
        client
            .call("turn/interrupt", json!({}), DEADLINE)
            .expect_err("EOF rejects unanswered call");
        witness.join().expect("shutdown witness")
    });
    assert!(transport.pending.lock().expect("requests").is_empty());
    assert_eq!(
        client
            .call("turn/interrupt", json!({}), DEADLINE)
            .expect_err("stopped helper")
            .to_string(),
        "Claude controller is closed"
    );
    let ui = forward(receiver);
    let mut output = Vec::new();
    for _ in 0..2 {
        let UiEvent::ServerLine(line) = ui.recv_timeout(DEADLINE).expect("queued native fact")
        else {
            panic!("expected native fact");
        };
        output.extend(apply(&client, &line));
    }
    drain_filler(&client, &ui, count, &filler);
    let UiEvent::ServerLine(line) = ui.recv_timeout(DEADLINE).expect("terminal failure") else {
        panic!("expected failure");
    };
    output.extend(apply(&client, &line));
    assert_eq!(output.last().expect("failure")["method"], "claude/error");
    assert_eq!(
        completions(&output),
        [json!({"id":id,"status":"completed"})]
    );
    assert!(
        transport
            .child
            .lock()
            .expect("child")
            .wait()
            .expect("helper exited")
            .code()
            != Some(0)
    );
}

fn saturate_unanswered(
    transport: Arc<ControllerTransport>,
    receiver: EventReceiver,
    bytes: bool,
) -> (EventReceiver, String, usize) {
    // The pipe marker proves the interrupt is pending before result and idle are read.
    let line = receiver
        .recv_timeout(DEADLINE)
        .expect("interrupt pending marker");
    let marker: serde_json::Value = serde_json::from_str(&line).expect("marker");
    assert_eq!(marker["params"]["callbackId"], "interrupt-pending");
    transport.events.handled(line.len());
    transport
        .call("queued", json!({}), DEADLINE)
        .expect("reader queued result and idle");
    let deferred = notify(&transport.events.hooks, "terminal_deferred");
    let eof = notify(&transport.events.hooks, "reader_eof");
    let filler = if bytes {
        "x".repeat(MAX_EVENT_BYTES - transport.events.charged_bytes())
    } else {
        "filler".to_string()
    };
    let count = if bytes { 1 } else { 1022 }; // Result and idle occupy the other two slots.
    for _ in 0..count {
        transport.events.send(filler.clone()).expect("fill queue");
    }
    let _ = transport.call("stderr", json!({}), DEADLINE);
    deferred
        .recv_timeout(DEADLINE)
        .expect("overflow starts bounded shutdown");
    eof.recv_timeout(DEADLINE)
        .expect("unanswered interrupt helper stops");
    assert!(transport.state.is_closed());
    (receiver, filler, count)
}

fn drain_filler(client: &ClaudeClient, ui: &Receiver<UiEvent>, count: usize, filler: &str) {
    for _ in 0..count {
        let UiEvent::ServerLine(line) = ui.recv_timeout(DEADLINE).expect("forwarded filler") else {
            panic!("expected server line");
        };
        assert_eq!(line, filler);
        client.acknowledge_event(line.len());
    }
}

#[test]
fn forwarded_4096_interrupts_after_eof_retain_no_requests() {
    let (client, receiver) = peer("", false);
    let ui = forward(receiver);
    ready(&client, &ui);
    let transport = transport(&client);
    let eof = notify(&transport.events.hooks, "reader_eof");
    client
        .request_native_control("exit", json!({}))
        .expect("exit");
    eof.recv_timeout(DEADLINE).expect("reader EOF");
    let UiEvent::ServerLine(line) = ui.recv_timeout(DEADLINE).expect("EOF fact") else {
        panic!("expected EOF");
    };
    apply(&client, &line);
    for _ in 0..4096 {
        assert!(client.call("turn/interrupt", json!({}), DEADLINE).is_err());
    }
    assert!(transport.pending.lock().expect("requests").is_empty());
    assert!(client
        .handler
        .borrow_mut()
        .lifecycle
        .completions()
        .is_empty());
}
