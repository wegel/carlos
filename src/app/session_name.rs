//! Read session names and save explicit rename commands without sending chat messages.

use carlos_core::backend::{BackendClient, BackendKind};
use serde_json::Value;

use super::AppState;

impl AppState {
    /// Reset only local controls named by an accepted native thread change.
    pub(super) fn apply_thread_change(
        &mut self,
        changed: carlos_core::session::thread_commands::ThreadChanged,
    ) {
        for retired in changed.callbacks {
            super::codex_callbacks::apply_retired(self, retired);
        }
        if changed.codex_reset {
            self.view.codex_callbacks.focused = false;
            self.view.codex_panel = Default::default();
            self.view.codex_goals = Default::default();
            let mode = self.view.codex_queue.mode;
            self.view.codex_queue = Default::default();
            self.view.codex_queue.mode = mode;
            self.view.codex_settings = Default::default();
        }
    }

    /// Apply a title only when metadata belongs to the current thread and includes a name field.
    pub(super) fn read_thread_name(&mut self, thread: &Value) {
        self.session.observe_thread_name(thread);
    }
}

/// Prefer the most recent start/resume response over an earlier history read.
pub(super) fn read_response_name(app: &mut AppState, response: &str) {
    if let Ok(parsed) = serde_json::from_str::<Value>(response) {
        app.read_thread_name(&parsed["result"]["thread"]);
    }
}

/// Consume `/rename` commands locally; a bare command loads the current name for editing.
pub(super) fn handle_rename_command(
    client: &dyn BackendClient,
    app: &mut AppState,
    text: &str,
) -> bool {
    let text = text.trim();
    let Some(rest) = text.strip_prefix("/rename") else {
        return false;
    };
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return false;
    }
    if matches!(client.kind(), BackendKind::Codex | BackendKind::Claude) && rest.trim().is_empty() {
        app.set_input_text(&format!(
            "/rename {}",
            app.session.thread_name().as_deref().unwrap_or("")
        ));
    } else {
        let update = app.session.prepare_rename(client.kind(), rest.trim());
        super::turn_submit::run_prepared(client, app, update);
    }
    true
}
