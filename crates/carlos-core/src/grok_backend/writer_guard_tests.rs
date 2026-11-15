//! Test the actual Grok writer queue after a session retires its queued frame.

use std::process::Command;
use std::sync::{Arc, Mutex};

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
