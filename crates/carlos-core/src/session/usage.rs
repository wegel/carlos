//! Describe weekly allowance left and compare it with time left until the reset.

use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::Value;

pub const WEEK_MINUTES: i64 = 7 * 24 * 60;

/// Keep a quota window even when an update omits its duration or reset time.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub struct RateWindow {
    used: u64,
    duration: Option<i64>,
    resets_at: Option<i64>,
}

impl RateWindow {
    /// Keep the backend values without inferring absent reset facts.
    pub fn new(used: u64, duration: Option<i64>, resets_at: Option<i64>) -> Self {
        Self {
            used,
            duration,
            resets_at,
        }
    }

    /// Read the reported percentage after the existing clamp.
    #[cfg(any(test, feature = "test-support"))]
    pub fn used(&self) -> u64 {
        self.used
    }

    /// Merge a valid window; a changed duration cannot inherit an old reset time.
    pub fn merge(previous: Option<Self>, value: &Value) -> Option<Self> {
        let used = value.get("usedPercent")?.as_u64()?.min(100);
        Some(
            Self {
                used,
                duration: value.get("windowDurationMins").and_then(Value::as_i64),
                resets_at: value.get("resetsAt").and_then(Value::as_i64),
            }
            .with_previous(previous),
        )
    }

    /// Fill omitted fields without borrowing a reset time from a different window.
    pub(crate) fn with_previous(self, previous: Option<Self>) -> Self {
        let previous = previous.filter(|p| self.duration.is_none() || self.duration == p.duration);
        Self {
            duration: self.duration.or(previous.and_then(|p| p.duration)),
            resets_at: self.resets_at.or(previous.and_then(|p| p.resets_at)),
            ..self
        }
    }

    /// Hide expired windows until the server supplies a fresh quota snapshot.
    pub fn indicator(self, now: i64) -> Option<WeeklyUsage> {
        if self.duration != Some(WEEK_MINUTES) {
            return None;
        }
        let remaining_seconds = self.resets_at?.checked_sub(now)?;
        if remaining_seconds <= 0 {
            return None;
        }
        let remaining = 100 - self.used;
        let time_left = (remaining_seconds as f64 / (WEEK_MINUTES * 60) as f64).min(1.0);
        let pace = if remaining == 0 {
            Pace::Behind
        } else if remaining as f64 >= time_left * 100.0 {
            Pace::OnTrack
        } else if remaining as f64 >= time_left * 75.0 {
            Pace::Close
        } else {
            Pace::Behind
        };
        Some(WeeklyUsage {
            remaining,
            remaining_seconds,
            pace,
        })
    }
}

/// Compare the allowance left with time left in the seven-day window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pace {
    OnTrack,
    Close,
    Behind,
}

/// Describe the current weekly allowance without terminal colors or layout.
pub struct WeeklyUsage {
    remaining: u64,
    remaining_seconds: i64,
    pace: Pace,
}

impl WeeklyUsage {
    /// Read the percentage still available.
    pub fn remaining(&self) -> u64 {
        self.remaining
    }

    /// Read the seconds before the backend reset timestamp.
    pub fn remaining_seconds(&self) -> i64 {
        self.remaining_seconds
    }

    /// Read the allowance's pace relative to the remaining week.
    pub fn pace(&self) -> Pace {
        self.pace
    }
}

/// Read wall-clock seconds for the backend's Unix reset timestamps.
pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs().min(i64::MAX as u64) as i64)
}
