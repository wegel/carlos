//! Restore only local editors and transcript caches after native snapshot adoption.

use serde_json::json;

use super::state::AppState;
use crate::codex_snapshot::CodexSnapshot;

/// Capture an unsubmitted editor before the session checks and adopts native facts.
pub(super) fn install(
    app: &mut AppState,
    snapshot: CodexSnapshot,
    active: Option<String>,
) -> anyhow::Result<()> {
    let editor = app.view.runtime_settings.show_model_settings.then(|| {
        json!({"model":app.model_settings_model_value(),
        "effort":app.model_settings_effort_value(),"summary":app.model_settings_summary_value()})
    });
    let installed =
        crate::session::codex::reconnect_install::install(&mut app.session, snapshot, active)?;
    app.reset_history_view();
    app.view.quota_next_draw = None;
    app.apply_item_changes(installed.history);
    if let Some(editor) = editor {
        app.view.codex_settings.rejected = Some(editor);
        super::codex_settings_draft::restore_editor(app);
    }
    app.view.codex_queue.error = None;
    app.view.codex_panel.detail = false;
    super::codex_goals::apply(app, installed.goal);
    Ok(())
}
