//! Keep keyboard and animation helpers beside immediate client effects of backend messages.

use std::time::{SystemTime, UNIX_EPOCH};

use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};

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
