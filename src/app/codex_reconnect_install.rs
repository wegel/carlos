//! Restore only local editors and transcript caches after native snapshot adoption.

use super::state::AppState;
use carlos_core::session::codex::frame_install::NativeInstalled;

impl AppState {
    /// Restore the exact unsent choices captured before the session adopted its new catalog.
    pub(super) fn apply_native_install(&mut self, installed: NativeInstalled) {
        self.reset_history_view();
        self.view.quota_next_draw = None;
        self.apply_item_changes(installed.facts.history);
        if let Some(editor) = installed.editor {
            self.view.codex_settings.rejected = Some(editor);
            super::codex_settings_draft::restore_editor(self);
        }
        self.view.codex_queue.error = None;
        self.view.codex_panel.detail = false;
        super::codex_goals::apply(self, installed.facts.goal);
    }
}
