//! Verifies shutdown after stdout closes while the helper remains alive on real pipes.

use std::time::{Duration, Instant};

use serde_json::json;

use super::event_eof_support::{assert_reaped, EofPeer, FullQueue};
use super::event_race_support::{notify, through, transport, Pause, DEADLINE};
use super::event_test_support::assert_eof;
use crate::backend::BackendClient;

#[test]
fn eof_before_stderr_byte_overflow_stops_live_helper() {
    check_stderr(FullQueue::Bytes, b'S', "stderr after EOF");
}

#[test]
fn eof_before_stderr_frame_overflow_stops_live_helper() {
    check_stderr(FullQueue::Frames, b'S', "stderr after EOF");
}

#[test]
fn eof_before_stderr_protocol_error_stops_live_helper_on_overflow() {
    check_stderr(
        FullQueue::Frames,
        b'P',
        "protocol error: invalid controller frame",
    );
}

fn check_stderr(limit: FullQueue, signal: u8, diagnostic: &str) {
    for eof_delivered in [false, true] {
        check_stderr_order(limit, signal, diagnostic, eof_delivered);
    }
}

fn check_stderr_order(limit: FullQueue, signal: u8, diagnostic: &str, eof_delivered: bool) {
    let mut peer = EofPeer::start();
    let transport = transport(&peer.client);
    let eof = prepare_eof(&mut peer, eof_delivered);
    limit.fill(&peer);
    let deferred = notify(&transport.events.hooks, "terminal_deferred");
    let started = Instant::now();
    peer.signal(signal);
    deferred
        .recv_timeout(DEADLINE)
        .expect("stderr overflow requests shutdown");
    assert!(transport
        .state
        .wait_for_shutdown_timeout(Duration::from_millis(3500)));
    assert!(started.elapsed() < Duration::from_millis(3500));
    assert_reaped(&transport);
    if let Some(eof) = eof {
        eof.resume();
    }
    limit.drain(&peer);
    assert_stderr_facts(&peer, diagnostic, eof_delivered);
    drop(peer.client);
    assert_reaped(&transport);
    assert!(transport.workers.is_cancelled());
}

fn prepare_eof(peer: &mut EofPeer, delivered: bool) -> Option<Pause> {
    let eof = peer.close_stdout_with_pending_call();
    if !delivered {
        return Some(eof);
    }
    let transport = transport(&peer.client);
    let done = notify(&transport.events.hooks, "reader_done");
    eof.resume();
    done.recv_timeout(DEADLINE)
        .expect("EOF facts queued before stderr");
    let facts = through(&peer.client, &peer.receiver, "turn/completed");
    assert_eq!(facts.len(), 2);
    assert_eq!(
        facts[0]["params"]["error"]["message"],
        "Claude controller stdout closed unexpectedly"
    );
    assert_eq!(
        facts[1]["params"]["turn"],
        json!({"id":peer.id,"status":"failed"})
    );
    assert!(!transport.state.shutdown_requested());
    assert!(transport
        .child
        .lock()
        .expect("child")
        .try_wait()
        .expect("live after EOF")
        .is_none());
    None
}

fn assert_stderr_facts(peer: &EofPeer, diagnostic: &str, eof_delivered: bool) {
    let until = if eof_delivered {
        "claude/error"
    } else {
        "turn/completed"
    };
    let facts = through(&peer.client, &peer.receiver, until);
    assert_eq!(facts.len(), if eof_delivered { 1 } else { 2 });
    assert_eq!(facts[0]["method"], "claude/error");
    assert_eq!(
        facts[0]["params"]["error"]["message"],
        format!("Claude controller: {diagnostic}")
    );
    if !eof_delivered {
        assert_eq!(
            facts[1]["params"]["turn"],
            json!({"id":peer.id,"status":"failed"})
        );
        assert_eof(&peer.client, &peer.receiver);
    }
}

#[test]
fn eof_before_explicit_stop_reaps_child_and_preserves_final_order() {
    let mut peer = EofPeer::start();
    let transport = transport(&peer.client);
    let eof = peer.close_stdout_with_pending_call();
    let started = Instant::now();
    peer.client.stop();
    assert!(started.elapsed() < Duration::from_millis(3500));
    assert_reaped(&transport);
    eof.resume();
    let facts = through(&peer.client, &peer.receiver, "turn/completed");
    assert_eq!(facts.len(), 2);
    assert_eq!(
        facts[0]["params"]["error"]["message"],
        "Claude controller stdout closed unexpectedly"
    );
    assert_eq!(
        facts[1]["params"]["turn"],
        json!({"id":peer.id,"status":"failed"})
    );
    drop(peer.client);
    assert!(transport.workers.is_cancelled());
    assert_reaped(&transport);
}

#[test]
fn eof_before_client_drop_cancels_senders_with_retained_byte_saturated_receiver() {
    check_drop(FullQueue::Bytes);
}

#[test]
fn eof_before_client_drop_cancels_senders_with_retained_frame_saturated_receiver() {
    check_drop(FullQueue::Frames);
}

#[test]
fn eof_before_client_drop_requests_shutdown_and_cancels_senders() {
    let mut peer = EofPeer::start();
    let transport = transport(&peer.client);
    let eof = peer.close_stdout_with_pending_call();
    let reader_done = notify(&transport.events.hooks, "reader_done");
    eof.resume();
    reader_done
        .recv_timeout(DEADLINE)
        .expect("EOF facts queued");
    assert!(
        !transport.state.shutdown_requested(),
        "EOF alone must not stop the child"
    );
    let started = Instant::now();
    drop(peer.client);
    assert!(started.elapsed() < Duration::from_millis(3500));
    assert_reaped(&transport);
    assert!(transport.workers.is_cancelled());
}

fn check_drop(limit: FullQueue) {
    let mut peer = EofPeer::start();
    let transport = transport(&peer.client);
    let eof = peer.close_stdout_with_pending_call();
    limit.fill(&peer);
    let deferred = notify(&transport.events.hooks, "terminal_deferred");
    let finished = notify(&transport.events.hooks, "finished");
    eof.resume();
    deferred
        .recv_timeout(DEADLINE)
        .expect("EOF facts wait for the consumer");
    let started = Instant::now();
    drop(peer.client);
    assert!(started.elapsed() < Duration::from_millis(3500));
    assert_reaped(&transport);
    finished
        .recv_timeout(DEADLINE)
        .expect("cancelled sender exits without consumer acknowledgements");
    assert!(
        transport.workers.is_cancelled(),
        "queue senders cancelled despite retained receiver"
    );
}

#[test]
fn eof_peer_keeps_control_socket_when_source_descriptor_is_100() {
    let mut peer = EofPeer::start_with_descriptor(true);
    let transport = transport(&peer.client);
    let eof = peer.close_stdout_with_pending_call();
    let done = notify(&transport.events.hooks, "reader_done");
    eof.resume();
    done.recv_timeout(DEADLINE).expect("EOF facts queued");
    peer.signal(b'S');
    // The helper has closed stdout, so stop it without spending the EOF grace.
    transport.emitter.stop();
    drop(peer.client);
    assert_reaped(&transport);
}
