//! Describe weekly allowance left and compare it with time left until the reset.

use std::time::{SystemTime, UNIX_EPOCH};

use ratatui::style::Color;
use serde_json::Value;

use crate::theme::{COLOR_DIFF_REMOVE, COLOR_GUTTER_USER, COLOR_TOOL_EDIT};

pub(super) const WEEK_MINUTES: i64 = 7 * 24 * 60;

/// Keep a quota window even when an update omits its duration or reset time.
#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct RateWindow {
    pub(super) used: u64,
    pub(super) duration: Option<i64>,
    pub(super) resets_at: Option<i64>,
}

impl RateWindow {
    /// Merge a valid window; a changed duration cannot inherit an old reset time.
    pub(super) fn merge(previous: Option<Self>, value: &Value) -> Option<Self> {
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
    pub(super) fn with_previous(self, previous: Option<Self>) -> Self {
        let previous = previous.filter(|p| self.duration.is_none() || self.duration == p.duration);
        Self {
            duration: self.duration.or(previous.and_then(|p| p.duration)),
            resets_at: self.resets_at.or(previous.and_then(|p| p.resets_at)),
            ..self
        }
    }

    /// Hide expired windows until the server supplies a fresh quota snapshot.
    pub(super) fn indicator(self, now: i64) -> Option<WeeklyIndicator> {
        if self.duration != Some(WEEK_MINUTES) {
            return None;
        }
        let remaining_seconds = self.resets_at?.checked_sub(now)?;
        if remaining_seconds <= 0 {
            return None;
        }
        let remaining = 100 - self.used;
        let time_left = (remaining_seconds as f64 / (WEEK_MINUTES * 60) as f64).min(1.0);
        let color = if remaining == 0 {
            COLOR_DIFF_REMOVE
        } else if remaining as f64 >= time_left * 100.0 {
            COLOR_GUTTER_USER
        } else if remaining as f64 >= time_left * 75.0 {
            COLOR_TOOL_EDIT
        } else {
            COLOR_DIFF_REMOVE
        };
        Some(WeeklyIndicator {
            percent: format!("{remaining}%"),
            countdown: format!("{:.1}d", remaining_seconds as f64 / 86_400.0),
            color,
        })
    }
}

/// Keep the percentage's pace color separate from the dim reset countdown.
pub(super) struct WeeklyIndicator {
    pub(super) percent: String,
    pub(super) countdown: String,
    pub(super) color: Color,
}

/// Read wall-clock seconds for the backend's Unix reset timestamps.
pub(super) fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs().min(i64::MAX as u64) as i64)
}
