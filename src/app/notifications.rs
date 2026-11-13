//! Keep keyboard and animation helpers beside immediate client effects of backend messages.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use carlos_core::session::actions::ServerRequestAction;
use carlos_core::session::common_update::CommonUpdate;
use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

use super::state::AppState;
use crate::theme::ANIMATION_STEP_MS;

// --- Key helpers ---

pub(super) fn is_ctrl_char(code: KeyCode, modifiers: KeyModifiers, ch: char) -> bool {
    matches!(code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&ch))
        && modifiers.contains(KeyModifiers::CONTROL)
}

pub(super) fn is_perf_toggle_key(code: KeyCode, modifiers: KeyModifiers) -> bool {
    matches!(code, KeyCode::F(8)) || is_ctrl_char(code, modifiers, 'p')
}

// --- Animation ---

pub(super) fn animation_tick() -> u128 {
    animation_tick_for_step(ANIMATION_STEP_MS)
}

/// Share a single-cell spinner at half the scanner's frame rate.
pub(super) fn spinner_frame() -> &'static str {
    const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    FRAMES[((animation_tick() / 2) % FRAMES.len() as u128) as usize]
}

pub(super) fn animation_tick_for_step(step_ms: u128) -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() / step_ms.max(1))
        .unwrap_or(0)
}

pub(super) fn working_animation_step_ms() -> u128 {
    ANIMATION_STEP_MS
}

pub(super) fn animation_poll_timeout() -> Duration {
    animation_poll_timeout_for_step(working_animation_step_ms())
}

pub(super) fn animation_poll_timeout_for_step(step_ms: u128) -> Duration {
    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let step_ms = step_ms.max(1);
    let rem = step_ms - (now_ms % step_ms);
    Duration::from_millis(rem.max(1) as u64)
}

pub(super) fn kitt_head_index(width: usize, tick: u128) -> usize {
    if width <= 1 {
        return 0;
    }

    let span = (width - 1) as u128;
    let cycle = span * 2;
    if cycle == 0 {
        return 0;
    }

    let phase = tick % cycle;
    if phase <= span {
        phase as usize
    } else {
        (cycle - phase) as usize
    }
}

pub(super) fn is_key_press_like(kind: KeyEventKind) -> bool {
    matches!(kind, KeyEventKind::Press | KeyEventKind::Repeat)
}

pub(crate) fn handle_server_message_line(
    app: &mut AppState,
    line: &str,
) -> Option<ServerRequestAction> {
    let context = super::backend_updates::context(&app.session, &app.view);
    let update = app.session.handle_backend_line(line, &context);
    app.apply_backend_update(update)
}

#[cfg(test)]
pub(super) fn handle_notification_line(app: &mut AppState, line: &str) {
    let _ = handle_server_message_line(app, line);
}

impl AppState {
    /// Consume one common-message report before handling another terminal event.
    pub(super) fn apply_common_update(
        &mut self,
        update: CommonUpdate,
    ) -> Option<ServerRequestAction> {
        if let Some(changed) = update.thread {
            self.apply_thread_change(changed);
        }
        if let Some(approval) = update.approval {
            if approval.clear_extra_lines {
                self.view.approval_hints.clear_extra_lines(approval.current);
            }
            self.drop_retired_approval_hints(approval.retired);
        }
        if let Some(cleared) = update.cleared {
            self.apply_cleared_approval(cleared);
        }
        if update.clear_quota_countdown {
            self.view.quota_next_draw = None;
        }
        self.apply_item_changes(update.items);
        update.action
    }
}
