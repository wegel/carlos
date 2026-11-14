//! Drive shared and named JSON handlers through the terminal-free session owner.

use serde_json::Value;

use super::backend_update::{BackendChange, BackendContext, BackendUpdate};
use super::claude::receive_update::ClaudeUpdate;
use super::state::SessionState;

impl SessionState {
    /// Apply one normalized backend line with the existing temporary native turn scope.
    pub fn handle_backend_line(
        &mut self,
        line: &str,
        context: &BackendContext<'_>,
    ) -> BackendUpdate {
        let Ok(parsed) = serde_json::from_str::<Value>(line) else {
            return BackendUpdate::new(BackendChange::None);
        };
        self.handle_backend_message(&parsed, context)
    }

    /// Apply an already parsed frame without copying or serializing its JSON tree.
    pub(crate) fn handle_backend_message(
        &mut self,
        parsed: &Value,
        context: &BackendContext<'_>,
    ) -> BackendUpdate {
        let previous =
            self.replace_codex_item_turn(parsed["params"]["turnId"].as_str().map(str::to_string));
        let update = self.receive_backend_message(parsed, context);
        self.replace_codex_item_turn(previous);
        update
    }

    /// Route an assembled Claude fact again, discarding its action at the old caller boundary.
    pub fn finish_claude_update(
        &mut self,
        mut update: ClaudeUpdate,
        context: &BackendContext<'_>,
    ) -> BackendUpdate {
        let nested = update
            .history_frame
            .take()
            .map(|frame| Box::new(self.handle_backend_message(&frame, context)));
        BackendUpdate {
            change: BackendChange::Claude(update),
            nested,
        }
    }

    fn receive_backend_message(
        &mut self,
        parsed: &Value,
        context: &BackendContext<'_>,
    ) -> BackendUpdate {
        if self.codex_scope().closed() || self.rate_limits_response(parsed) {
            return BackendUpdate::new(BackendChange::None);
        }
        let Some(method) = parsed["method"].as_str() else {
            return BackendUpdate::new(BackendChange::None);
        };
        if self.codex_scope().enabled() {
            if let Some(update) = self.route_codex_callback(method, parsed, context.panel) {
                return BackendUpdate::new(BackendChange::Codex(update));
            }
        }
        if super::codex::panel_events::event(self, context.panel, method, &parsed["params"]) {
            return BackendUpdate::new(BackendChange::None);
        }
        let Some(params) = parsed.get("params").and_then(Value::as_object) else {
            return BackendUpdate::new(BackendChange::None);
        };
        if method == "carlos/grok/ready" || method == "carlos/grok/closed" {
            return self.handle_grok_lifecycle(method, &parsed["params"]);
        }
        if method == "carlos/codex/closed" {
            return BackendUpdate::new(
                self.close_codex_native(params, "")
                    .map(BackendChange::Closed)
                    .unwrap_or(BackendChange::None),
            );
        }
        if let Some(update) = self.receive_claude_message(method, &parsed["params"]) {
            return self.finish_claude_update(update, context);
        }
        BackendUpdate::new(BackendChange::Common(
            self.handle_common_backend_message(parsed),
        ))
    }
}
