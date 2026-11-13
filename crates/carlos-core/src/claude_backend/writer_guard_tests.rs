//! Test the actual Claude writer queue after a session retires its queued frame.

use super::InputWriter;
use crate::claude_backend::transport_state::TransportState;
use crate::claude_backend::transport_workers::TransportWorkers;
use crate::writer_guard_test_support::Sink;
use std::sync::Arc;

#[test]
fn claude_writer_discards_a_retired_queued_frame_and_keeps_the_connection() {
    let (sink, proof) = Sink::new();
    let writer = InputWriter::start(
        sink,
        None,
        Arc::new(TransportState::default()),
        Arc::new(TransportWorkers::default()),
    );
    proof.run(|frame, permission| writer.enqueue(frame, permission));
    writer.close();
}
