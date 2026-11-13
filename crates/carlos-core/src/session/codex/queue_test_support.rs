//! Seed exact native queue fixtures without exposing mutable session fields.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::queue_state::QueueOperation;
use crate::session::state::SessionState;

impl SessionState {
    /// Replace the fixture queue with accepted native entries.
    pub fn set_codex_queue_entries_for_test(&mut self, entries: Vec<Value>) {
        self.codex_queue.entries = entries;
    }
    /// Set whether the fixture has received the native queue.
    pub fn set_codex_queue_known_for_test(&mut self, known: bool) {
        self.codex_queue.known = known;
    }
    /// Mark whether the fixture has a native queue change in progress.
    pub fn set_codex_queue_pending_for_test(&mut self, pending: bool) {
        self.codex_queue.operation = pending.then(|| QueueOperation {
            token: 1,
            method: "thread/queue/add".into(),
            params: json!({}),
            submitted: None,
        });
    }
    /// Age the fixture queue request so deadline handling can run immediately.
    pub fn age_codex_queue_read_for_test(&mut self, age: Duration) {
        self.codex_queue.read.as_mut().expect("snapshot").started =
            Instant::now().checked_sub(age).expect("prior instant");
    }
}
