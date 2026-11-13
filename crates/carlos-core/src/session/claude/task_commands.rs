//! Apply native task edges and send stops for an explicitly chosen task.

use serde_json::{json, Value};

use crate::backend::BackendClient;
use crate::session::state::SessionState;

impl SessionState {
    /// Replace native membership or apply an edge for its newest run.
    pub fn update_claude_task(&mut self, message: &Value) -> Result<Option<String>, &'static str> {
        self.claude.tasks.update(message)
    }

    /// Verify the chosen target and send at most one stop per native run.
    pub fn stop_claude_task(&mut self, client: &dyn BackendClient, id: &str) {
        let Some(run) = self.claude.tasks.begin_stop(id) else {
            return;
        };
        if let Err(error) =
            client.request_native_control("task.stop", json!({"taskId":id,"runId":run}))
        {
            self.claude
                .tasks
                .stop_reply(id, run.as_deref(), Some(error.to_string()));
        }
    }

    /// Settle only an acknowledgement for the current nonterminal run.
    pub fn receive_claude_task_stop(&mut self, id: &str, run: Option<&str>, error: Option<String>) {
        self.claude.tasks.stop_reply(id, run, error);
    }

    /// Dismiss the previous stop error when the user chooses a target.
    pub fn dismiss_claude_task_error(&mut self) {
        self.claude.tasks.control_error = None;
    }
}
