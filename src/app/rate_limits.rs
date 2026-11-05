//! Refresh Codex quota snapshots without waiting for network replies in the UI loop.

use std::time::{Duration, Instant};

use serde_json::Value;

use super::weekly_usage::{unix_now, RateWindow, WeeklyIndicator};
use crate::backend::{BackendClient, BackendKind};

const REFRESH_INTERVAL: Duration = Duration::from_secs(300);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
const DISPLAY_INTERVAL: Duration = Duration::from_secs(60);

/// Track the shared Codex bucket, an outstanding read, and the next refresh times.
#[derive(Default)]
pub(super) struct RateLimits {
    windows: [Option<RateWindow>; 2],
    during_read: [Option<RateWindow>; 2],
    pending: Option<(String, Instant)>,
    next_read: Option<Instant>,
    next_draw: Option<Instant>,
}

impl RateLimits {
    /// Queue a small JSON-RPC request and leave the reader to deliver its reply.
    pub(super) fn tick(&mut self, client: &dyn BackendClient, now: Instant) -> bool {
        if client.kind() != BackendKind::Codex {
            return false;
        }
        let redraw = self.next_draw.is_none_or(|deadline| deadline <= now);
        if redraw {
            self.next_draw = Some(now + DISPLAY_INTERVAL);
        }
        if self
            .pending
            .as_ref()
            .is_some_and(|(_, deadline)| *deadline <= now)
        {
            self.pending = None;
        }
        if self.pending.is_none() && self.next_read.is_none_or(|deadline| deadline <= now) {
            self.next_read = Some(now + REFRESH_INTERVAL);
            if let Ok(Some(id)) = client.request_rate_limits() {
                self.during_read = [None; 2];
                self.pending = Some((id, now + REQUEST_TIMEOUT));
            }
        }
        redraw
    }

    /// Wake idle sessions for countdown changes, request deadlines, and quota refreshes.
    pub(super) fn wait(&self, now: Instant) -> Option<Duration> {
        self.next_draw
            .into_iter()
            .chain(self.next_read)
            .chain(self.pending.as_ref().map(|(_, deadline)| *deadline))
            .min()
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    /// Consume only the asynchronous reply that belongs to the current read.
    pub(super) fn response(&mut self, value: &Value) -> bool {
        let Some((id, _)) = self.pending.as_ref() else {
            return false;
        };
        if value.get("id").and_then(Value::as_str) != Some(id.as_str()) {
            return false;
        }
        self.pending = None;
        if let Some(result) = value.get("result") {
            let updates = std::mem::take(&mut self.during_read);
            self.windows = [None; 2];
            if let Some(snapshot) = result
                .get("rateLimitsByLimitId")
                .and_then(|m| m.get("codex"))
            {
                self.update(snapshot);
            } else if let Some(snapshot) = result.get("rateLimits") {
                self.update(snapshot);
            }
            for (window, update) in self.windows.iter_mut().zip(updates) {
                if let Some(update) = update {
                    *window = Some(update.with_previous(*window));
                }
            }
        }
        true
    }

    /// Merge sparse updates for Codex; other model-specific buckets stay separate.
    pub(super) fn update(&mut self, snapshot: &Value) {
        if snapshot
            .get("limitId")
            .and_then(Value::as_str)
            .is_some_and(|id| id != "codex")
        {
            return;
        }
        for (index, name) in ["primary", "secondary"].into_iter().enumerate() {
            if let Some(window) = snapshot.get(name).filter(|window| !window.is_null()) {
                if let Some(merged) = RateWindow::merge(self.windows[index], window) {
                    self.windows[index] = Some(merged);
                    if self.pending.is_some() {
                        self.during_read[index] =
                            RateWindow::merge(self.during_read[index], window);
                    }
                }
            }
        }
    }

    /// Discard account-specific values and any in-flight reply after an auth change.
    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }

    /// Find a seven-day window by its duration rather than by primary/secondary order.
    pub(super) fn indicator(&self) -> Option<WeeklyIndicator> {
        self.indicator_at(unix_now())
    }

    /// Use an explicit clock when checking countdown and pace calculations.
    pub(super) fn indicator_at(&self, now: i64) -> Option<WeeklyIndicator> {
        self.windows
            .iter()
            .flatten()
            .find_map(|window| window.indicator(now))
    }
}

#[cfg(test)]
#[path = "../tests/weekly_usage_tests.rs"]
mod tests;
