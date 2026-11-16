//! Test the actual Grok writer queue after a session retires its queued frame.

use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::json;

use super::InputWriter;
use crate::owned_child::OwnedChild;
use crate::writer_guard_test_support::Sink;

#[test]
fn grok_writer_discards_a_retired_queued_frame_and_keeps_the_connection() {
    let child = Arc::new(Mutex::new(
        OwnedChild::spawn(Command::new("sleep").arg("30")).expect("owned Grok fixture"),
    ));
    let (sink, proof) = Sink::new();
    let writer = InputWriter::start(sink, child.clone());
    proof.run(|frame, permission| writer.enqueue(frame, permission));
    writer.close();
    child
        .lock()
        .expect("owned child")
        .wait()
        .expect("reap fixture");
}

#[test]
fn grok_blocked_pipe_reports_its_writer_stage_without_private_frame_text() {
    let mut command = Command::new("python3");
    command
        .args(["-c", "import time; time.sleep(60)"])
        .stdin(Stdio::piped());
    let mut child = OwnedChild::spawn(&mut command).expect("non-reading native peer");
    let stdin = child.stdin.take().expect("native input pipe");
    let child = Arc::new(Mutex::new(child));
    let writer = InputWriter::start(stdin, child.clone());
    let began = Instant::now();
    let error = writer
        .write(&json!({"text":"PRIVATE_STAGE_TEST".repeat(128 * 1024)}))
        .expect_err("a stalled native pipe must time out")
        .to_string();
    assert!(error.contains("stdin did not finish"));
    assert!(error.contains("ready+"));
    assert!(error.contains("written=pending"));
    assert!(!error.contains("PRIVATE_STAGE_TEST"));
    child
        .lock()
        .expect("owned child")
        .wait()
        .expect("reap fixture");
    assert!(began.elapsed() < Duration::from_secs(4));
}
