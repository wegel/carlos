//! Apply native Claude quota facts through the session owner.

use std::time::Instant;

use serde_json::Value;

use super::ClaudePhase;
use crate::backend::{BackendClient, BackendKind};
use crate::session::state::SessionState;

impl SessionState {
    /// Request optional quota only while the native helper is ready.
    pub fn tick_claude_quota(&mut self, client: &dyn BackendClient, now: Instant) {
        if client.kind() == BackendKind::Claude && self.claude().phase() == ClaudePhase::Ready {
            self.claude.quota.tick(client, now);
        }
    }

    /// Merge a native stream report into its named windows.
    pub fn claude_quota_event(&mut self, info: &Value) {
        self.claude.quota.event(info);
    }

    /// Apply an optional quota report without replacing newer stream facts.
    pub fn claude_quota_optional(&mut self, result: &Value, revision: u64) {
        self.claude.quota.optional(result, revision);
    }
}
