//! Seed exact native queue fixtures without exposing mutable session fields.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::queue_state::QueueOperation;
use crate::session::state::SessionState;

impl SessionState {
    pub fn set_codex_queue_entries_for_test(&mut self, entries: Vec<Value>) {
        self.codex_queue.entries = entries;
    }
    pub fn set_codex_queue_known_for_test(&mut self, known: bool) {
        self.codex_queue.known = known;
    }
    pub fn set_codex_queue_pending_for_test(&mut self, pending: bool) {
        self.codex_queue.operation = pending.then(|| QueueOperation {
            token: 1,
            method: "thread/queue/add".into(),
            params: json!({}),
            submitted: None,
        });
    }
    pub fn age_codex_queue_read_for_test(&mut self, age: Duration) {
        self.codex_queue.read.as_mut().expect("snapshot").started =
            Instant::now().checked_sub(age).expect("prior instant");
    }
}
