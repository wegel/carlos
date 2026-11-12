//! Apply native Claude quota facts through the session owner.

use std::time::Instant;

use serde_json::Value;

use crate::backend::BackendClient;
use crate::session::state::SessionState;

impl SessionState {
    /// Request optional quota when the app has checked the native helper is ready.
    pub(crate) fn tick_claude_quota(&mut self, client: &dyn BackendClient, now: Instant) {
        self.claude.quota.tick(client, now);
    }

    /// Merge a native stream report into its named windows.
    pub(crate) fn claude_quota_event(&mut self, info: &Value) {
        self.claude.quota.event(info);
    }

    /// Apply an optional quota report without replacing newer stream facts.
    pub(crate) fn claude_quota_optional(&mut self, result: &Value, revision: u64) {
        self.claude.quota.optional(result, revision);
    }
}
