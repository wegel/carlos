//! Show selected queue mode and a short acknowledgement after an explicit steer switch.

use std::time::{Duration, Instant};

use super::state::AppState;

const NOTICE: Duration = Duration::from_secs(2);

/// A mode change is UI feedback only; prompt classification is captured separately at dispatch.
pub(super) fn changed(app: &mut AppState, queue: bool, now: Instant) {
    app.view.viewport.steer_notice_until = (!queue).then_some(now + NOTICE);
}

/// Let an explicit panel shortcut change the same composer mode.
pub(super) fn toggle(app: &mut AppState, now: Instant) {
    let queue = if app.session.claude().enabled() {
        app.view.claude_queue = !app.view.claude_queue;
        app.view.claude_queue
    } else {
        app.view.codex_queue.mode = !app.view.codex_queue.mode;
        app.view.codex_queue.mode
    };
    changed(app, queue, now);
}

/// Keep the ordinary default quiet and show queue because it changes Enter.
pub(super) fn label(app: &AppState, now: Instant) -> Option<&'static str> {
    if app.rewind_mode() || !(app.session.claude().enabled() || app.session.codex_scope().enabled())
    {
        return None;
    }
    let queue = if app.session.claude().enabled() {
        app.view.claude_queue
    } else {
        app.view.codex_queue.mode
    };
    if queue {
        Some("Queue")
    } else if app
        .view
        .viewport
        .steer_notice_until
        .is_some_and(|until| until > now)
    {
        Some("Steer")
    } else {
        None
    }
}

/// Erase the notice even if no native work or keyboard event causes another draw.
pub(super) fn tick(app: &mut AppState, now: Instant) -> bool {
    if app
        .view
        .viewport
        .steer_notice_until
        .is_some_and(|until| until <= now)
    {
        app.view.viewport.steer_notice_until = None;
        return true;
    }
    false
}

/// Wake a quiet UI only while an explicit mode notice still has to disappear.
pub(super) fn wait(app: &AppState, now: Instant) -> Option<Duration> {
    app.view
        .viewport
        .steer_notice_until
        .map(|until| until.saturating_duration_since(now))
}
