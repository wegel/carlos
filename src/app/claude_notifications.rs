//! Pass normalized Claude frames to the session and apply its local reports.

use super::state::AppState;

/// Recover submitted text and apply only returned local editor changes.
#[cfg(test)]
pub(super) fn fail_session(app: &mut AppState) {
    let mut update = Default::default();
    app.session.fail_claude(&mut update);
    let _ = app.apply_claude_update(update);
}
