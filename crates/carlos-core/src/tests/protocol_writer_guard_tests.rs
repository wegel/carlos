//! Exercise retirement after admission to the actual Codex native write queue.

use std::process::Command;
use std::time::{Duration, Instant};

use super::InputWriter;
use crate::protocol_events::EventQueue;
use crate::protocol_state::ClientState;
use crate::writer_guard_test_support::Sink;

#[test]
fn codex_writer_discards_a_retired_queued_frame_and_keeps_the_connection() {
    let child = Command::new("sleep")
        .arg("30")
        .spawn()
        .expect("owned native process fixture");
    let (events, _receiver) = EventQueue::new();
    let state = ClientState::new(child, events);
    let (sink, proof) = Sink::new();
    let writer = InputWriter::start(Box::new(sink), state.clone());
    proof.run(|frame, permission| {
        writer.enqueue_guarded(
            frame.clone(),
            Instant::now() + Duration::from_secs(5),
            &state,
            permission,
        )
    });
    state.close("fixture finished", false);
}
