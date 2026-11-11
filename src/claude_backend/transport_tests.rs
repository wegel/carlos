//! Deterministic pipe and event-budget checks without a Claude account or model.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;
use std::sync::atomic::Ordering;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde_json::json;

use super::event_queue::EventSender;
use super::helper::native_on_path;
use super::transport::ControllerTransport;

#[test]
fn stuck_helper_stdin_cannot_block_request_or_shutdown_forever() {
    let (sender, _receiver) = mpsc::sync_channel(16);
    let mut command = Command::new("python3");
    command.args([
        "-c",
        "import signal,time; signal.signal(signal.SIGTERM,signal.SIG_IGN); time.sleep(60)",
    ]);
    let transport = ControllerTransport::start_command(command, EventSender::new(sender))
        .expect("start stuck helper");
    let started = Instant::now();
    let error = transport
        .request("hello", json!({ "text": "x".repeat(2 * 1024 * 1024) }))
        .expect_err("stuck pipe must fail");
    assert!(error.to_string().contains("stdin did not finish"));
    assert!(transport.stopped.load(Ordering::SeqCst));
    transport.stop();
    assert!(
        started.elapsed() < Duration::from_secs(4),
        "stuck stdin blocked shutdown"
    );
}

#[test]
fn slow_ui_stops_the_helper_and_settles_unanswered_requests() {
    let (sender, receiver) = mpsc::sync_channel(1024);
    let events = EventSender::new(sender);
    let mut command = Command::new("python3");
    command.args(["-c", concat!(
        "import json,sys,time\n", "sys.stdin.readline()\n",
        "for i in range(20):\n",
        " print(json.dumps({'v':1,'type':'event','event':'sdk','message':{'type':'system','text':'x'*900000}}),flush=True)\n",
        "time.sleep(60)\n")]);
    let transport =
        ControllerTransport::start_command(command, events.clone()).expect("start flooding helper");
    let started = Instant::now();
    let error = transport
        .call("start", json!({}), Duration::from_secs(5))
        .expect_err("overflow must settle pending reply");
    assert!(error.to_string().contains("closed before replying"));
    assert!(started.elapsed() < Duration::from_secs(3));
    transport.stop();
    let queued: Vec<String> = receiver.try_iter().collect();
    assert!(queued
        .iter()
        .any(|line| line.contains("backlog overflowed")));
    assert!(queued.iter().map(String::len).sum::<usize>() < 9 * 1024 * 1024);
    for line in queued {
        events.handled(line.len());
    }
}

#[test]
fn moving_events_to_an_intermediate_queue_does_not_release_their_byte_budget() {
    let (sender, receiver) = mpsc::sync_channel(16);
    let events = EventSender::new(sender);
    let frame = "x".repeat(1024 * 1024);
    let mut intermediate = Vec::new();
    for _ in 0..8 {
        events.send(frame.clone()).expect("queue event");
        intermediate.push(receiver.recv().expect("forward event"));
    }
    assert!(events.send("another event".to_string()).is_err());
    events.handled(intermediate.pop().expect("queued event").len());
    events
        .send("another event".to_string())
        .expect("UI consumption releases budget");
}

#[test]
fn native_path_lookup_skips_a_nonexecutable_shadow() {
    let root = tempfile::tempdir().expect("temporary PATH");
    let first = root.path().join("first");
    let second = root.path().join("second");
    fs::create_dir_all(&first).expect("first folder");
    fs::create_dir_all(&second).expect("second folder");
    fs::write(first.join("claude"), "not executable").expect("shadow file");
    fs::write(second.join("claude"), "#!/bin/sh\nexit 0\n").expect("executable");
    fs::set_permissions(second.join("claude"), fs::Permissions::from_mode(0o755))
        .expect("executable mode");
    let path = std::env::join_paths([first, second.clone()]).expect("PATH");
    assert_eq!(
        native_on_path(&path).expect("native CLI"),
        second.join("claude")
    );
}

#[test]
fn an_unanswered_control_stops_the_session_instead_of_assuming_it_was_not_applied() {
    let (sender, _receiver) = mpsc::sync_channel(16);
    let mut command = Command::new("python3");
    command.args([
        "-c",
        "import sys,time; sys.stdin.readline(); time.sleep(60)",
    ]);
    let transport = ControllerTransport::start_command(command, EventSender::new(sender))
        .expect("start quiet helper");
    let error = transport
        .call(
            "model.set",
            json!({ "model": "test" }),
            Duration::from_millis(50),
        )
        .expect_err("missing reply");
    assert!(error.to_string().contains("may already have been accepted"));
    assert!(transport.stopped.load(Ordering::SeqCst));
    transport.stop();
}

#[test]
fn oversized_input_is_rejected_before_writing_and_leaves_the_session_usable() {
    let (sender, _receiver) = mpsc::sync_channel(16);
    let mut command = Command::new("python3");
    command.args(["-c", concat!("import sys,json\n", "for line in sys.stdin:\n",
        " request=json.loads(line)\n",
        " print(json.dumps({'v':1,'type':'reply','id':request['id'],'ok':True,'result':{}}),flush=True)\n",
        " if request['method']=='shutdown': break\n")]);
    let transport = ControllerTransport::start_command(command, EventSender::new(sender))
        .expect("start echo helper");
    let error = transport
        .request("prompt", json!({ "text": "x".repeat(4 * 1024 * 1024) }))
        .expect_err("oversized frame");
    assert!(error.to_string().contains("exceeds 4 MiB"));
    assert!(!transport.stopped.load(Ordering::SeqCst));
    transport
        .call("hello", json!({}), Duration::from_secs(1))
        .expect("valid request after rejected input");
    transport.stop();
}
