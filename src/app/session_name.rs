//! Read session names and save explicit rename commands without sending chat messages.

use std::time::Duration;

use serde_json::{Map, Value};

use super::{AppState, Role};
use crate::backend::{BackendClient, BackendKind};
use crate::protocol_params::{extract_result_object, params_thread_set_name};

impl AppState {
    /// Clear the previous session's name when the client switches thread IDs.
    pub(super) fn set_thread_id(&mut self, thread_id: impl Into<String>) {
        let thread_id = thread_id.into();
        if *self.session.thread_id() != thread_id {
            if self.session.codex_scope().enabled() {
                super::codex_callbacks::reset(self);
                self.view.codex_panel = Default::default();
                self.session.reset_codex_panel();
                self.session.reset_codex_task_status();
                self.view.codex_goals = Default::default();
                self.session.reset_codex_goals();
                self.session.advance_codex_epoch();
                let mode = self.view.codex_queue.mode;
                self.view.codex_queue = Default::default();
                self.session.reset_codex_queue();
                self.view.codex_queue.mode = mode;
                self.view.codex_settings = Default::default();
                self.session.reset_codex_settings();
            }
            self.session.set_thread_name(None);
        }
        self.session.set_thread_id(thread_id);
    }

    /// Keep a single-line, terminal-safe name for display and inline editing.
    pub(super) fn set_thread_name(&mut self, name: Option<&str>) {
        self.session.observe_name(name);
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

/// Apply external renames without changing the draft or transcript.
pub(super) fn handle_name_updated(app: &mut AppState, params: &Map<String, Value>) {
    if params.get("threadId").and_then(Value::as_str) == Some(app.session.thread_id().as_str()) {
        app.set_thread_name(params.get("threadName").and_then(Value::as_str));
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
    if !matches!(client.kind(), BackendKind::Codex | BackendKind::Claude) {
        report_rename_error(
            app,
            "this backend does not support renaming sessions",
            false,
        );
    } else if rest.trim().is_empty() {
        app.set_input_text(&format!(
            "/rename {}",
            app.session.thread_name().as_deref().unwrap_or("")
        ));
    } else if rest.trim().chars().any(char::is_control) {
        report_rename_error(
            app,
            "use a single-line session name without control characters",
            false,
        );
    } else {
        save_name(client, app, rest.trim());
    }
    true
}

fn save_name(client: &dyn BackendClient, app: &mut AppState, name: &str) {
    let result = client
        .call(
            "thread/name/set",
            params_thread_set_name(&app.session.thread_id(), name),
            Duration::from_secs(10),
        )
        .and_then(|response| extract_result_object(&response));
    match result {
        Ok(_) => {
            app.set_thread_name(Some(name));
            app.clear_input();
            app.set_status("session renamed");
        }
        Err(error) => report_rename_error(app, &error.to_string(), true),
    }
}

fn report_rename_error(app: &mut AppState, message: &str, backend: bool) {
    let message = format!("Could not rename session: {message}");
    if backend {
        app.set_status(message.clone());
    } else {
        app.set_hint(message.clone());
    }
    app.append_message(Role::System, message);
}
