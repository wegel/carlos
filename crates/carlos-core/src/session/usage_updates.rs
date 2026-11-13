//! Apply quota messages and read timers through the session that owns them.

use std::time::Instant;

use serde_json::Value;

use super::rate_limits::RateLimits;
use super::state::SessionState;
use crate::backend::BackendClient;

impl SessionState {
    /// Borrow the quota facts and backend deadlines without permitting writes.
    pub fn rate_limits(&self) -> &RateLimits {
        &self.rate_limits
    }

    /// Expire outstanding quota reads and request the next snapshot when due.
    pub fn tick_rate_limits(&mut self, client: &dyn BackendClient, now: Instant) {
        if self.backend_reads_ready() {
            self.rate_limits.tick(client, now);
        }
    }

    /// Consume the reply for the current quota read, if it matches.
    pub fn rate_limits_response(&mut self, value: &Value) -> bool {
        self.rate_limits.response(value)
    }

    /// Merge a quota notification without replacing omitted window fields.
    pub fn update_rate_limits(&mut self, snapshot: &Value) {
        self.rate_limits.update(snapshot);
    }

    /// Forget account facts and any pending read after an account change.
    pub fn clear_rate_limits(&mut self) {
        self.rate_limits.clear();
    }
}
