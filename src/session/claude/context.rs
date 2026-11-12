//! Refresh root-context snapshots without clearing known counts during ordinary work.

use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::activity::NativeState;
use super::state::ClaudePhase;
use crate::backend::BackendClient;
use crate::session::context_usage::ContextUsage;
use crate::session::state::SessionState;

const INTERVAL: Duration = Duration::from_secs(1);
const STALE_AFTER: Duration = Duration::from_secs(5);

/// Keep one native read outstanding, including when a slow reply outlives its UI deadline.
#[derive(Default)]
#[cfg_attr(test, derive(Debug))]
pub(in crate::session) struct ClaudeContext {
    pending: Option<Read>,
    next_read: Option<Instant>,
    sequence: u64,
    pub(in crate::session) fresh: bool,
    model: Option<String>,
}

#[cfg_attr(test, derive(Debug))]
struct Read {
    token: u64,
    sent: Instant,
}

impl SessionState {
    /// Request another snapshot without invalidating the last known native count.
    pub(crate) fn refresh_claude_context(&mut self) {
        self.claude.context.fresh = false;
    }

    /// Fence old reads only when the native context or its window actually changes.
    pub(crate) fn invalidate_claude_context(&mut self) {
        self.claude.activity.context_revision += 1;
        self.set_context_usage(None);
        self.claude.context.next_read = None;
        self.refresh_claude_context();
    }

    /// Send at most one optional summary read; neither typing nor chat waits for it.
    pub(crate) fn tick_claude_context(
        &mut self,
        client: &dyn BackendClient,
        now: Instant,
        enabled: bool,
    ) -> bool {
        if !self.claude_context_available(enabled) {
            return false;
        }
        let model = self.runtime_settings().current_model().clone();
        let mut redraw = false;
        if self.claude.context.model != model {
            redraw = self.context_usage().is_some();
            self.claude.context.model = model;
            self.invalidate_claude_context();
        }
        if self.claude.context.fresh
            && !self.claude_context_working()
            && self.claude.context.pending.is_none()
        {
            return redraw;
        }
        let context = &mut self.claude.context;
        if let Some(read) = &context.pending {
            let stale = now.saturating_duration_since(read.sent) >= STALE_AFTER;
            let changed = stale && context.fresh;
            context.fresh &= !stale;
            return redraw || changed;
        }
        if context.next_read.is_some_and(|deadline| deadline > now) {
            return redraw;
        }
        context.sequence += 1;
        context.pending = Some(Read {
            token: context.sequence,
            sent: now,
        });
        context.next_read = Some(now + INTERVAL);
        let params = json!({"revision":self.claude.activity.context_revision,"contextToken":context.sequence});
        if client.request_native_control("context", params).is_err() {
            context.pending = None;
            context.next_read = Some(now + STALE_AFTER);
            redraw |= context.fresh;
            context.fresh = false;
        }
        redraw
    }

    /// Wake idle work for reads and stale-value markers, without polling a stuck request forever.
    pub(crate) fn claude_context_wait(&self, now: Instant, enabled: bool) -> Option<Duration> {
        if !self.claude_context_available(enabled) {
            return None;
        }
        let context = &self.claude.context;
        if context.fresh && !self.claude_context_working() && context.pending.is_none() {
            return None;
        }
        let deadline = if let Some(read) = &context.pending {
            let deadline = read.sent + STALE_AFTER;
            (deadline > now).then_some(deadline)?
        } else {
            context.next_read.unwrap_or(now)
        };
        Some(deadline.saturating_duration_since(now))
    }

    /// Settle even an obsolete read, then apply only counts for the current context revision.
    pub(crate) fn receive_claude_context(&mut self, frame: &Value, request: &Value) {
        let context = &mut self.claude.context;
        if let Some(token) = request["contextToken"].as_u64() {
            if !context
                .pending
                .as_ref()
                .is_some_and(|read| read.token == token)
            {
                return;
            }
        }
        context.pending = None;
        if request["revision"]
            .as_u64()
            .is_some_and(|requested| requested != self.claude.activity.context_revision)
        {
            context.next_read = None;
            return;
        }
        context.next_read = Some(
            Instant::now()
                + if frame["ok"] == true {
                    INTERVAL
                } else {
                    STALE_AFTER
                },
        );
        let result = &frame["result"];
        let max = result["rawMaxTokens"]
            .as_u64()
            .or_else(|| result["maxTokens"].as_u64());
        if frame["ok"] == true {
            if let (Some(used), Some(max)) =
                (result["totalTokens"].as_u64(), max.filter(|max| *max > 0))
            {
                self.context_usage = Some(ContextUsage { used, max });
                context.fresh = true;
                return;
            }
        }
        context.fresh = false;
    }

    fn claude_context_available(&self, enabled: bool) -> bool {
        enabled
            && self.claude.phase == ClaudePhase::Ready
            && self.claude.activity.transient.as_deref() != Some("Compacting")
    }

    fn claude_context_working(&self) -> bool {
        self.active_turn_id().is_some() || self.claude.activity.state != NativeState::Idle
    }
}
