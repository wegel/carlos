//! Apply history reports to this terminal after the session adopts native records.

use anyhow::Result;
use serde_json::Value;

use super::state::AppState;

/// Load checked native history and reset local viewport/cursor state.
pub(crate) fn load_history_from_start_or_resume(app: &mut AppState, response: &str) -> Result<()> {
    let changes = crate::session::item_history::load_history_from_start_or_resume(
        &mut app.session,
        response,
    )?;
    if !changes.transcript.is_empty() {
        app.reset_history_view();
    }
    app.apply_item_changes(changes);
    Ok(())
}

/// Replace same-thread rows after all native metadata has been checked.
pub(super) fn replace_history_from_thread(app: &mut AppState, thread: &Value) -> Result<()> {
    let changes =
        crate::session::item_history::replace_history_from_thread(&mut app.session, thread)?;
    app.reset_history_view();
    app.apply_item_changes(changes);
    Ok(())
}

#[cfg(test)]
pub(super) fn append_history_from_thread(app: &mut AppState, thread: &Value) {
    let changes =
        crate::session::item_history::append_history_from_thread(&mut app.session, thread);
    app.apply_item_changes(changes);
}
