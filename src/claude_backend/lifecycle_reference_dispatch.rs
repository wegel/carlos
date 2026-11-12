//! Projects main's app handlers onto the ordered named facts used by differential tests.

use serde_json::{json, Value};

use super::lifecycle_reference::Reference;

#[derive(Default)]
pub(super) struct ReferenceDriver {
    pub(super) state: Reference,
    generation: u64,
    epoch: u64,
    closed: bool,
    reset_key: Option<String>,
}

impl ReferenceDriver {
    pub(super) fn new() -> Self {
        Self {
            generation: 1,
            ..Self::default()
        }
    }

    /// main:claude_notifications::handle/handle_controller/handle_frame generation fences.
    pub(super) fn handle(&mut self, line: &str) -> Vec<String> {
        let value: Value = serde_json::from_str(line).expect("test JSON");
        let method = value["method"].as_str().expect("method");
        let params = &value["params"];
        if method == "claude/sessionChanged"
            && params["generation"]
                .as_u64()
                .is_some_and(|g| g > self.generation)
        {
            *self = Self {
                generation: params["generation"].as_u64().expect("generation"),
                ..Self::default()
            };
        }
        if params["generation"]
            .as_u64()
            .is_some_and(|g| g != self.generation)
            || params["epoch"].as_u64().is_some_and(|e| e < self.epoch)
            // main still accepts local prompt facts and terminal notices after closing the query.
            || (self.closed && !matches!(method,
                "claude/promptSubmitted" | "claude/promptRejected" | "claude/error" | "claude/closed" | "claude/sessionStartFailed"
            ))
        {
            return if matches!(method, "claude/nativeBind" | "claude/nativeResult") {
                Vec::new()
            } else {
                vec![line.to_string()]
            };
        }
        let output = self.apply(method, params, line);
        output
            .into_iter()
            .map(|line| {
                let mut emitted: Value = serde_json::from_str(&line).expect("fact");
                for key in ["generation", "epoch"] {
                    if let Some(stamp) = params.get(key) {
                        emitted["params"][key] = stamp.clone();
                    }
                }
                emitted.to_string()
            })
            .collect()
    }

    fn apply(&mut self, method: &str, params: &Value, line: &str) -> Vec<String> {
        match method {
            // main:claude_notifications::accept_prompt and promptRejected branch.
            "claude/promptSubmitted" => {
                let id = params["uuid"].as_str().expect("UUID");
                self.state.submitted(id);
                self.state
                    .prompts
                    .entries
                    .entry(id.to_string())
                    .or_default();
            }
            "claude/promptRejected" => {
                let id = params["uuid"].as_str().expect("UUID");
                self.state.rejected(id);
                self.state.prompts.entries.remove(id);
            }
            "claude/nativeBind" => return self.bind(params),
            "claude/nativeResult" => return self.result(params),
            // main:claude_sdk_events::receive session_state_changed and finish_results.
            "claude/activity" => {
                self.state.set_idle(params["state"] == "idle");
                let mut output = vec![line.to_string()];
                output.extend(self.state.completions());
                return output;
            }
            "claude/conversationReset" => self.reset(params),
            _ => {}
        }
        let mut output = vec![line.to_string()];
        // main:claude_notifications::handle_event/fail_session; control_replies::failed.
        if matches!(method, "claude/closed" | "claude/sessionStartFailed")
            || method == "claude/error" && params["error"]["code"] != "unsupported_dialog"
            || method == "claude/controlReply"
                && params["ok"] != true
                && matches!(
                    params["requestMethod"].as_str(),
                    Some("start" | "sessions.info" | "sessions.read")
                )
        {
            self.closed = true;
            output.extend(self.state.fail_pending());
        }
        output
    }

    /// main:claude_sdk_events::main_items and claude_prompts::consume (prompt UI omitted).
    fn bind(&mut self, params: &Value) -> Vec<String> {
        let mut native = params.clone();
        if params["init"] == true {
            native["type"] = json!("system");
            native["subtype"] = json!("init");
        }
        self.state.bind_reply(&native);
        let mut output = Vec::new();
        for id in super::lifecycle_reference::prompt_ids(params) {
            output.push(record("claude/promptsConsumed", json!({"ids":[id]})));
            if let Some(prompt) = self.state.prompts.entries.get_mut(&id) {
                let start = !prompt.consumed && !prompt.clear;
                prompt.consumed = true;
                if start {
                    output.push(record("turn/started", json!({"turn":{"id":id}})));
                }
            }
        }
        output
    }

    /// main:claude_sdk_events::receive result branch and finish_results, in source order.
    fn result(&mut self, params: &Value) -> Vec<String> {
        let before: Vec<String> = self.state.prompts.entries.keys().cloned().collect();
        self.state.result(params);
        let mut output = vec![record("claude/result", json!({"error":params["error"]}))];
        let retired: Vec<String> = before
            .into_iter()
            .filter(|id| !self.state.prompts.entries.contains_key(id))
            .collect();
        if !retired.is_empty() {
            output.push(record("claude/promptsRetired", json!({"ids":retired})));
        }
        output.extend(self.state.completions());
        output
    }

    /// main:claude_reset::receive, including its duplicate-key guard and cursor reset.
    fn reset(&mut self, params: &Value) {
        self.epoch = params["epoch"].as_u64().unwrap_or(self.epoch);
        let key = params["new_conversation_id"].as_str().unwrap_or("");
        if self.reset_key.as_deref() == Some(key) {
            return;
        }
        self.reset_key = Some(key.to_string());
        self.state
            .reset_prompts(params["user_message_uuid"].as_str());
        self.state.translation = Default::default();
    }
}

fn record(method: &str, params: Value) -> String {
    json!({"method":method,"params":params}).to_string()
}
