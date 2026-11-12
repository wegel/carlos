//! Keeps native quota buckets separate and ignores expired or unavailable readings.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::backend::BackendClient;
use crate::session::usage::WeeklyUsage;
use crate::session::usage::{unix_now, RateWindow, WEEK_MINUTES};

/// Retains account windows without interpreting session token totals as quota usage.
#[derive(Default)]
#[cfg_attr(test, derive(Debug))]
pub(crate) struct ClaudeQuota {
    windows: BTreeMap<String, QuotaWindow>,
    revision: u64,
    next_read: Option<Instant>,
}

/// Keeps a model-specific bucket distinct from the shared weekly bucket.
#[cfg_attr(test, derive(Debug))]
pub(crate) struct QuotaWindow {
    label: String,
    used: f64,
    reset: i64,
    revision: u64,
}

impl ClaudeQuota {
    /// Request optional account quota without waiting for it.
    pub(in crate::session) fn tick(&mut self, client: &dyn BackendClient, now: Instant) {
        if self.next_read.is_none_or(|deadline| deadline <= now) {
            self.next_read = Some(now + Duration::from_secs(300));
            let _ =
                client.request_native_control("usage.optional", json!({"revision":self.revision}));
        }
    }

    /// Read the next optional backend deadline.
    pub(crate) fn wait(&self, now: Instant) -> Option<Duration> {
        self.next_read
            .map(|deadline| deadline.saturating_duration_since(now))
    }

    /// Borrow retained windows without granting writes.
    pub(crate) fn windows(&self) -> &BTreeMap<String, QuotaWindow> {
        &self.windows
    }

    /// Merge stable stream fractions only into the bucket that native Claude named.
    pub(in crate::session) fn event(&mut self, info: &Value) {
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
    pub(in crate::session) fn optional(&mut self, result: &Value, revision: u64) {
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
    pub(crate) fn indicator(&self) -> Option<WeeklyUsage> {
        let window = self.windows.get("seven_day")?;
        RateWindow::new(
            window.used.round() as u64,
            Some(WEEK_MINUTES),
            Some(window.reset),
        )
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

impl QuotaWindow {
    /// Borrow the native window label.
    pub(crate) fn label(&self) -> &str {
        &self.label
    }
    /// Read the percentage used in this window.
    pub(crate) fn used(&self) -> f64 {
        self.used
    }
    /// Read the native reset timestamp.
    pub(crate) fn reset(&self) -> i64 {
        self.reset
    }
}
