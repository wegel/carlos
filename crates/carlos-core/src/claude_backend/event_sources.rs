//! Attaches native evidence to common items without treating generated counters as identity.

use std::collections::HashMap;

use serde_json::{json, Value};

use super::types::ClaudeTranslationState;

/// Annotate an item only when Claude supplied a native message or tool identity.
pub(super) fn attach_sources(state: &ClaudeTranslationState, message: &Value, line: &mut Value) {
    let Some(id) = line["params"]["item"]["id"]
        .as_str()
        .or_else(|| line["params"]["itemId"].as_str())
    else {
        return;
    };
    let kind = line["params"]["item"]["type"].as_str();
    let key = if kind == Some("toolCall")
        && state.tool_calls.get(id).is_some_and(|call| call.native_id)
    {
        Some(json!({"kind":"tool","id":id,"part":"call"}))
    } else if matches!(kind, Some("agentMessage" | "reasoning"))
        || line["params"]["delta"].is_string()
    {
        assistant_source(state, message, id)
    } else {
        result_source(message, id)
    };
    if let Some(mut key) = key {
        key["sessionId"] = message["session_id"]
            .as_str()
            .or(state.session_id.as_deref())
            .map_or(Value::Null, |id| json!(id));
        line["params"]["sourceKey"] = key;
    }
}

fn assistant_source(state: &ClaudeTranslationState, message: &Value, id: &str) -> Option<Value> {
    let prefix = format!(
        "claude-msg-{}-{}-",
        state.stream_instance_seq, state.current_message_seq
    );
    let index = id.strip_prefix(&prefix)?.parse::<usize>().ok()?;
    let native = state
        .current_message_id
        .as_deref()
        .or_else(|| {
            (message["type"] == "assistant")
                .then(|| message["uuid"].as_str())
                .flatten()
        })?
        .trim();
    // A UUID identifies this snapshot, rather than every snapshot in the generated message.
    let block = if state.current_message_id.is_none()
        && message["message"]["content"]
            .as_array()
            .is_some_and(|parts| parts.len() == 1)
    {
        0
    } else {
        index
    };
    (!native.is_empty()).then(|| json!({"kind":"assistant","id":native,"block":block}))
}

fn result_source(message: &Value, id: &str) -> Option<Value> {
    message["message"]["content"]
        .as_array()?
        .iter()
        .find_map(|part| {
            let native = part["tool_use_id"].as_str().filter(|id| !id.is_empty())?;
            (part["type"] == "tool_result" && id == format!("{native}:result"))
                .then(|| json!({"kind":"tool","id":native,"part":"result"}))
        })
}

/// Tracks native block positions across Claude's one-block assistant history records.
#[derive(Default)]
pub(super) struct HistorySources {
    message: Option<String>,
    next_block: usize,
    records: HashMap<String, usize>,
}

impl HistorySources {
    /// Advance only on native assistant records; generated display counters remain separate.
    pub(super) fn begin_record(&mut self, message: &Value) -> usize {
        let native = message["message"]["id"]
            .as_str()
            .filter(|id| !id.trim().is_empty())
            .or_else(|| message["uuid"].as_str())
            .map(str::trim)
            .filter(|id| !id.is_empty());
        if native != self.message.as_deref() || native.is_none() {
            self.next_block = 0;
            self.records.clear();
        }
        self.message = native.map(str::to_string);
        if let Some(base) = message["uuid"].as_str().and_then(|id| self.records.get(id)) {
            return *base;
        }
        let count = message["message"]["content"].as_array().map_or(0, Vec::len);
        let base = if count == 1 { self.next_block } else { 0 };
        self.next_block = self.next_block.max(base + count);
        if let Some(uuid) = message["uuid"].as_str() {
            self.records.insert(uuid.to_string(), base);
        }
        base
    }
}
