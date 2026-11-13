//! Build prompt choices from accepted session defaults and native inheritance.

use serde_json::Value;

use crate::backend::{BackendClient, BackendKind};
use crate::protocol_params::params_turn_start;

use super::state::SessionState;

impl SessionState {
    /// Omit cached native choices unless the server requires legacy turn overrides.
    pub(crate) fn new_turn_params(&self, client: &dyn BackendClient, text: &str) -> Value {
        let (model, effort, summary) =
            if client.kind() == BackendKind::Codex && self.codex_scope().enabled() {
                if self.codex_settings().legacy_overrides() {
                    let choices = self.codex_runtime_choices();
                    (
                        choices["model"].as_str().map(str::to_string),
                        choices["effort"].as_str().map(str::to_string),
                        choices["summary"].as_str().map(str::to_string),
                    )
                } else {
                    (None, None, None)
                }
            } else {
                self.next_turn_runtime_settings()
            };
        let (model, effort) = if client.kind() == BackendKind::Claude {
            self.claude_prompt_choices(model, effort)
        } else {
            (model, effort)
        };
        params_turn_start(
            self.thread_id(),
            text,
            model.as_deref(),
            effort.as_deref(),
            summary.as_deref(),
        )
    }
}
