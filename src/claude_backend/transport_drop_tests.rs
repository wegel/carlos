//! Checks client-drop latency while a grandchild keeps each helper pipe open.

use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::event_eof_support::assert_reaped;
use super::transport_drop_support::PipePeer;
use super::transport_state::SHUTDOWN_TIMEOUT;

#[test]
fn drop_detaches_retained_stdout() {
    check_drop("stdout", false);
}

#[test]
fn drop_detaches_retained_stderr() {
    check_drop("stderr", false);
}

#[test]
fn drop_detaches_full_stdin_retained_by_grandchild() {
    check_drop("stdin", false);
}

#[test]
fn drop_detaches_retained_stdout_with_full_byte_budget() {
    check_drop("stdout", true);
}

#[test]
fn drop_detaches_retained_stderr_with_full_byte_budget() {
    check_drop("stderr", true);
}

#[test]
fn drop_detaches_full_stdin_retained_by_grandchild_with_full_byte_budget() {
    check_drop("stdin", true);
}

#[test]
fn ordinary_drop_keeps_main_latency() {
    check_drop("ordinary", false);
}

fn check_drop(pipe: &str, full_budget: bool) {
    let mut peer = PipePeer::start(pipe, full_budget);
    if pipe != "ordinary" {
        peer.exit_helper(pipe);
    }
    let client = peer.client.take().expect("client");
    let (done_tx, done) = mpsc::channel();
    let dropping = thread::spawn(move || {
        let started = Instant::now();
        drop(client);
        done_tx.send(started.elapsed()).expect("drop result");
    });
    // Main normally needs one 10 ms reaper tick; allow a fraction of its existing grace.
    let bound = SHUTDOWN_TIMEOUT / 10;
    let result = done.recv_timeout(bound);
    // Release all pipes even if an unbounded join regresses, before asserting or joining.
    peer.cleanup();
    dropping.join().expect("drop released by fixture cleanup");
    let elapsed: Duration = result.expect("drop must return while the grandchild holds its pipe");
    eprintln!(
        "drop_latency pipe={pipe} full_budget={full_budget} us={}",
        elapsed.as_micros()
    );
    assert!(elapsed < bound, "drop took {elapsed:?}; bound is {bound:?}");
    assert_reaped(&peer.transport);
    assert!(peer.transport.workers.is_cancelled());
}
