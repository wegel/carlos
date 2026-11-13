//! Runs Claude turn-end rules only when the caller handles an ordered fact.

use serde_json::{json, Value};

use super::event_lifecycle::{prompt_ids, Lifecycle};
use super::event_normalizer::fact;

/// Belongs to ClaudeClient's calling thread, never to either pipe reader.
pub(super) struct EventHandler {
    pub(super) lifecycle: Lifecycle,
    generation: u64,
    epoch: u64,
    closed: bool,
    reset_key: Option<String>,
}

impl Default for EventHandler {
    fn default() -> Self {
        Self {
            lifecycle: Lifecycle::default(),
            generation: 1,
            epoch: 0,
            closed: false,
            reset_key: None,
        }
    }
}

impl EventHandler {
    /// Replace native lifecycle facts with the common lines the app applies at this position.
    pub(super) fn handle(&mut self, line: &str) -> Vec<String> {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return vec![line.to_string()];
        };
        let method = value["method"].as_str().unwrap_or("");
        let params = &value["params"];
        if method == "claude/sessionChanged"
            && params["generation"]
                .as_u64()
                .is_some_and(|g| g > self.generation)
        {
            self.generation = params["generation"].as_u64().expect("checked generation");
            self.lifecycle = Lifecycle::default();
            self.epoch = 0;
            self.closed = false;
            self.reset_key = None;
        }
        if params["generation"]
            .as_u64()
            .is_some_and(|g| g != self.generation)
            || params["epoch"].as_u64().is_some_and(|e| e < self.epoch)
            || (self.closed
                && !matches!(
                    method,
                    "claude/promptSubmitted"
                        | "claude/promptRejected"
                        | "claude/error"
                        | "claude/closed"
                        | "claude/sessionStartFailed"
                ))
        {
            // The app also fences ordinary transcript and process facts.
            return if matches!(method, "claude/nativeBind" | "claude/nativeResult") {
                Vec::new()
            } else {
                vec![line.to_string()]
            };
        }
        let mut output = self.apply(method, params, line);
        for line in &mut output {
            let mut emitted: Value = serde_json::from_str(line).expect("internal fact");
            for key in ["generation", "epoch"] {
                if let Some(stamp) = params.get(key) {
                    emitted["params"][key] = stamp.clone();
                }
            }
            *line = emitted.to_string();
        }
        output
    }

    fn apply(&mut self, method: &str, params: &Value, line: &str) -> Vec<String> {
        match method {
            "claude/promptSubmitted" => {
                if let Some(id) = params["uuid"].as_str() {
                    if self.lifecycle.submitted(id).is_err() {
                        // The app's prompt register reports the same bound and closes the helper.
                        self.closed = true;
                        let mut output = vec![line.to_string()];
                        output.extend(self.lifecycle.fail_pending());
                        return output;
                    }
                }
            }
            "claude/promptRejected" => {
                if let Some(id) = params["uuid"].as_str() {
                    self.lifecycle.rejected(id);
                }
            }
            "claude/nativeBind" => return self.bind(params),
            "claude/nativeResult" => return self.result(params),
            "claude/activity" => {
                self.lifecycle.set_idle(params["state"] == "idle");
                let mut output = vec![line.to_string()];
                output.extend(self.lifecycle.completions());
                return output;
            }
            "claude/conversationReset" => self.reset(params),
            _ => {}
        }
        let mut output = vec![line.to_string()];
        if fatal(method, params) {
            self.closed = true;
            output.extend(self.lifecycle.fail_pending());
        }
        output
    }

    fn bind(&mut self, params: &Value) -> Vec<String> {
        let started = self.lifecycle.bind_reply(params);
        let mut output = Vec::new();
        for id in prompt_ids(params) {
            output.push(fact("claude/promptsConsumed", json!({"ids":[id]})));
            if started.contains(&id) {
                output.push(fact("turn/started", json!({"turn":{"id":id}})));
            }
        }
        output
    }

    fn result(&mut self, params: &Value) -> Vec<String> {
        let retired = self.lifecycle.result(params);
        let mut output = vec![fact("claude/result", json!({"error":params["error"]}))];
        if !retired.is_empty() {
            output.push(fact("claude/promptsRetired", json!({"ids":retired})));
        }
        output.extend(self.lifecycle.completions());
        output
    }

    fn reset(&mut self, params: &Value) {
        self.epoch = params["epoch"].as_u64().unwrap_or(self.epoch);
        let key = params["new_conversation_id"].as_str().unwrap_or("");
        if self.reset_key.as_deref() == Some(key) {
            return;
        }
        self.reset_key = Some(key.to_string());
        self.lifecycle
            .reset_prompts(params["user_message_uuid"].as_str());
    }
}

fn fatal(method: &str, params: &Value) -> bool {
    matches!(method, "claude/closed" | "claude/sessionStartFailed")
        || method == "claude/error" && params["error"]["code"] != "unsupported_dialog"
        || method == "claude/controlReply"
            && params["ok"] != true
            && matches!(
                params["requestMethod"].as_str(),
                Some("start" | "sessions.info" | "sessions.read")
            )
}
