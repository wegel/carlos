//! Settle correlated Claude controls without treating acknowledgments as turn completion.

use serde_json::{json, Value};

use super::receive_update::ClaudeUpdate;
use crate::session::actions::ServerRequestAction;
use crate::session::state::SessionState;

impl SessionState {
    /// Apply the existing request-specific success and failure rules.
    pub(crate) fn receive_claude_control(
        &mut self,
        frame: &Value,
        request: Option<&str>,
        context: &Value,
        update: &mut ClaudeUpdate,
    ) {
        if frame["ok"] != true {
            self.failed_claude_control(frame, request, context, update);
            return;
        }
        match request {
            Some(method @ ("sessions.info" | "sessions.read")) => {
                match self.claude_history_control(method, &frame["result"]) {
                    Ok((next, params)) => {
                        if method == "sessions.info" {
                            self.observe_name(frame["result"]["customTitle"].as_str());
                            self.set_status("Reading Claude history");
                        }
                        update.action = Some(ServerRequestAction::ClaudeControl {
                            method: next.into(),
                            params,
                        });
                    }
                    Err(error) => {
                        self.record_claude_error(&json!({"message":error.to_string()}));
                        self.fail_claude(update);
                    }
                }
            }
            Some("start" | "capabilities") => {
                self.receive_claude_capabilities(&frame["result"], update);
                update.action = Some(ServerRequestAction::ApplyClaudeSettings);
            }
            Some("task.stop") => {
                if let Some(id) = context["taskId"].as_str() {
                    self.receive_claude_task_stop(id, context["runId"].as_str(), None);
                }
            }
            Some("usage.optional") => self
                .claude_quota_optional(&frame["result"], context["revision"].as_u64().unwrap_or(0)),
            Some("context") => self.receive_claude_context(frame, context),
            _ => {}
        }
    }

    fn failed_claude_control(
        &mut self,
        frame: &Value,
        request: Option<&str>,
        context: &Value,
        update: &mut ClaudeUpdate,
    ) {
        if request == Some("task.stop") {
            if let Some(id) = context["taskId"].as_str() {
                self.receive_claude_task_stop(
                    id,
                    context["runId"].as_str(),
                    Some(
                        frame["error"]["message"]
                            .as_str()
                            .unwrap_or("Stop failed")
                            .to_string(),
                    ),
                );
            }
            return;
        }
        if request == Some("context") {
            self.receive_claude_context(frame, context);
            return;
        }
        if request == Some("usage.optional") {
            return;
        }
        self.record_claude_error(&frame["error"]);
        if matches!(request, Some("start" | "sessions.info" | "sessions.read")) {
            self.fail_claude(update);
        }
    }
}
