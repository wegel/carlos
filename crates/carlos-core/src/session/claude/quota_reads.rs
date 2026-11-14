//! Bind optional quota reads to their original native root, helper generation and five-minute deadline.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::quota::ClaudeQuota;
use crate::backend::BackendKind;
use crate::session::state::SessionState;
use crate::session::task_digest::hash_text;

/// One optional report cannot overwrite a different native root or a newer scheduled read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct QuotaReadKey {
    number: u64,
    scope: blake3::Hash,
}

impl QuotaReadKey {
    pub(crate) fn from_context(context: &Value) -> Option<Self> {
        Some(Self {
            number: context["quotaRead"].as_u64()?,
            scope: blake3::Hash::from_hex(context["quotaScope"].as_str()?).ok()?,
        })
    }
}

impl ClaudeQuota {
    pub(super) fn prepare(&mut self, now: Instant, scope: blake3::Hash) -> Option<Value> {
        if self.next_read.is_some_and(|deadline| deadline > now) {
            return None;
        }
        self.next_read = Some(now + Duration::from_secs(300));
        self.issued = self.issued.checked_add(1)?;
        let key = QuotaReadKey {
            number: self.issued,
            scope,
        };
        self.pending = Some(key);
        Some(
            json!({"revision":self.revision,"quotaRead":key.number,"quotaScope":key.scope.to_hex().as_str()}),
        )
    }

    pub(super) fn request_is_current(&self, key: QuotaReadKey, scope: blake3::Hash) -> bool {
        self.pending == Some(key) && key.scope == scope
    }

    pub(super) fn finish_read(&mut self, key: QuotaReadKey, scope: blake3::Hash) -> bool {
        let current = self.request_is_current(key, scope);
        self.fail_read(key);
        current
    }

    pub(super) fn fail_read(&mut self, key: QuotaReadKey) {
        if self.pending == Some(key) {
            self.pending = None;
        }
    }

    pub(super) fn retire_scope(&mut self, scope: blake3::Hash) {
        if self.pending.is_some_and(|key| key.scope != scope) {
            self.pending = None;
        }
    }
}

pub(super) fn scope(session: &SessionState) -> blake3::Hash {
    let mut hash = blake3::Hasher::new();
    hash.update(b"carlos.claude.quota.read.v1");
    hash_text(&mut hash, session.session_id().as_str());
    hash_text(&mut hash, session.thread_id());
    let (epoch, generation) = session.command_connection(BackendKind::Claude);
    hash.update(&epoch.to_le_bytes());
    hash.update(&generation.to_le_bytes());
    hash.update(&[u8::from(session.events.backend_closed)]);
    hash.finalize()
}
