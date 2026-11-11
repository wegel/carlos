//! Keeps native quota buckets separate and ignores expired or unavailable readings.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use super::weekly_usage::{unix_now, RateWindow, WeeklyIndicator, WEEK_MINUTES};
use crate::backend::{BackendClient, BackendKind};

/// Retains account windows without interpreting session token totals as quota usage.
#[derive(Default)]
pub(super) struct ClaudeQuota {
    pub(super) windows: BTreeMap<String, QuotaWindow>,
    pub(super) revision: u64,
    next_read: Option<Instant>,
    next_draw: Option<Instant>,
}

/// Keeps a model-specific bucket distinct from the shared weekly bucket.
pub(super) struct QuotaWindow {
    pub(super) label: String,
    pub(super) used: f64,
    pub(super) reset: i64,
    revision: u64,
}

impl ClaudeQuota {
    /// Read optional account quota in the background; neither prompts nor startup await it.
    pub(super) fn tick(&mut self, client: &dyn BackendClient, now: Instant, ready: bool) -> bool {
        if client.kind() != BackendKind::Claude || !ready {
            return false;
        }
        let redraw = self.next_draw.is_none_or(|deadline| deadline <= now);
        if redraw {
            self.next_draw = Some(now + Duration::from_secs(60));
        }
        if self.next_read.is_none_or(|deadline| deadline <= now) {
            self.next_read = Some(now + Duration::from_secs(300));
            let _ =
                client.request_native_control("usage.optional", json!({"revision":self.revision}));
        }
        redraw
    }

    /// Wake the idle UI when quota countdowns or the next optional read need attention.
    pub(super) fn wait(&self, now: Instant) -> Option<Duration> {
        self.next_draw
            .into_iter()
            .chain(self.next_read)
            .min()
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    /// Merge stable stream fractions only into the bucket that native Claude named.
    pub(super) fn event(&mut self, info: &Value) {
        self.revision += 1;
        if let Some(key) = info["rateLimitType"].as_str().filter(|key| valid_key(key)) {
            self.fraction(key, info);
        }
        if let Some(windows) = info["unifiedWindows"].as_object() {
            for (key, window) in windows {
                if valid_key(key) {
                    self.fraction(key, window);
                }
            }
        }
    }

    /// Apply a sparse optional report without replacing newer stream windows with an older read.
    pub(super) fn optional(&mut self, result: &Value, revision: u64) {
        if result["report"]["rate_limits_available"] == false {
            self.windows.retain(|_, window| window.revision > revision);
            return;
        }
        if let Some(windows) = result["windows"].as_array() {
            for window in windows {
                let Some(key) = window["key"].as_str() else {
                    continue;
                };
                if self
                    .windows
                    .get(key)
                    .is_some_and(|current| current.revision > revision)
                {
                    continue;
                }
                if let (Some(used), Some(reset)) =
                    (window["usedPercent"].as_f64(), window["resetsAt"].as_i64())
                {
                    self.insert(
                        key,
                        window["label"].as_str().unwrap_or(key),
                        used,
                        reset,
                        revision,
                    );
                }
            }
        }
    }

    /// Reuse Carlos's weekly pace color only for the shared seven-day bucket.
    pub(super) fn indicator(&self) -> Option<WeeklyIndicator> {
        let window = self.windows.get("seven_day")?;
        RateWindow {
            used: window.used.round() as u64,
            duration: Some(WEEK_MINUTES),
            resets_at: Some(window.reset),
        }
        .indicator(unix_now())
    }

    fn fraction(&mut self, key: &str, value: &Value) {
        if let (Some(fraction), Some(reset)) =
            (value["utilization"].as_f64(), value["resetsAt"].as_i64())
        {
            if (0.0..=1.0).contains(&fraction) {
                self.insert(
                    key,
                    &key.replace('_', " "),
                    fraction * 100.0,
                    reset,
                    self.revision,
                );
            }
        }
    }

    fn insert(&mut self, key: &str, label: &str, used: f64, reset: i64, revision: u64) {
        if key.len() > 4096
            || label.len() > 4096
            || !used.is_finite()
            || !(0.0..=100.0).contains(&used)
            || reset <= unix_now()
        {
            return;
        }
        if !self.windows.contains_key(key) && self.windows.len() >= 128 {
            return;
        }
        self.windows.insert(
            key.to_string(),
            QuotaWindow {
                label: label.to_string(),
                used,
                reset,
                revision,
            },
        );
    }
}

fn valid_key(key: &str) -> bool {
    matches!(
        key,
        "five_hour"
            | "seven_day"
            | "seven_day_opus"
            | "seven_day_sonnet"
            | "seven_day_overage_included"
            | "overage"
    )
}
