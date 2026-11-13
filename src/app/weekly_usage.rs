//! Format weekly allowance facts and map their pace to terminal colors.

use carlos_core::session::usage::{Pace, WeeklyUsage};
use ratatui::style::Color;

use crate::theme::{COLOR_DIFF_REMOVE, COLOR_GUTTER_USER, COLOR_TOOL_EDIT};

/// Keep the percentage's pace color separate from the dim reset countdown.
pub(super) struct WeeklyIndicator {
    pub(super) percent: String,
    pub(super) countdown: String,
    pub(super) color: Color,
}

impl From<WeeklyUsage> for WeeklyIndicator {
    fn from(usage: WeeklyUsage) -> Self {
        Self {
            percent: format!("{}%", usage.remaining()),
            countdown: format!("{:.1}d", usage.remaining_seconds() as f64 / 86_400.0),
            color: match usage.pace() {
                Pace::OnTrack => COLOR_GUTTER_USER,
                Pace::Close => COLOR_TOOL_EDIT,
                Pace::Behind => COLOR_DIFF_REMOVE,
            },
        }
    }
}
