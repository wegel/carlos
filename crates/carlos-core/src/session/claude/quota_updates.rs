//! Apply native Claude quota facts through the session owner.

use std::time::Instant;

use serde_json::Value;

use super::quota_reads;
use super::{ClaudePhase, QuotaReadKey};
use crate::backend::{BackendClient, BackendKind};
use crate::session::state::SessionState;

impl SessionState {
    /// Request optional quota only while the native helper is ready.
    pub fn tick_claude_quota(&mut self, client: &dyn BackendClient, now: Instant) {
        if client.kind() == BackendKind::Claude {
            if let Some(params) = self.prepare_claude_quota(now) {
                let key = QuotaReadKey::from_context(&params).expect("prepared quota read");
                if client
                    .request_native_control("usage.optional", params)
                    .is_err()
                {
                    self.fail_claude_quota_request(key);
                }
            }
        }
    }

    /// Prepare the existing optional account read without waiting for a native pipe writer.
    pub fn prepare_claude_quota(&mut self, now: Instant) -> Option<Value> {
        let scope = quota_reads::scope(self);
        self.claude.quota.retire_scope(scope);
        (self.claude.phase() == ClaudePhase::Ready)
            .then(|| self.claude.quota.prepare(now, scope))
            .flatten()
    }

    pub(crate) fn claude_quota_request_current(&self, key: QuotaReadKey) -> bool {
        self.claude
            .quota
            .request_is_current(key, quota_reads::scope(self))
    }

    pub(crate) fn fail_claude_quota_request(&mut self, key: QuotaReadKey) {
        self.claude.quota.fail_read(key);
    }

    pub(super) fn receive_claude_quota_report(&mut self, frame: &Value, context: &Value) {
        let Some(key) = QuotaReadKey::from_context(context) else {
            return;
        };
        let scope = quota_reads::scope(self);
        if !self.claude.quota.finish_read(key, scope) {
            return;
        }
        if frame["ok"] == true {
            self.claude_quota_optional(&frame["result"], context["revision"].as_u64().unwrap_or(0));
        }
    }

    /// Merge a native stream report into its named windows.
    pub fn claude_quota_event(&mut self, info: &Value) {
        self.task_overview_changed();
        self.claude.quota.event(info);
    }

    /// Apply an optional quota report without replacing newer stream facts.
    pub fn claude_quota_optional(&mut self, result: &Value, revision: u64) {
        self.task_overview_changed();
        self.claude.quota.optional(result, revision);
    }
}
