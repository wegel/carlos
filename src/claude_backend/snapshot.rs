//! Assistant snapshot output synthesis for the Claude CLI backend.

use serde_json::{json, Map, Value};

use super::types::{
    begin_claude_message, claude_message_item_id, claude_tool_item_id, ensure_claude_turn_started,
    should_hide_claude_tool_transcript, ClaudeToolCall, ClaudeTranslationState, TranslateOutput,
};

pub(super) fn synthesize_assistant_snapshot(
    state: &mut ClaudeTranslationState,
    root: &Map<String, Value>,
    out: &mut TranslateOutput,
) {
    let Some(message) = root.get("message").and_then(Value::as_object) else {
        return;
    };
    if message.get("role").and_then(Value::as_str) != Some("assistant") {
        return;
    }
    let message_id = message
        .get("id")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|message_id| !message_id.is_empty());
    let Some(content) = message.get("content").and_then(Value::as_array) else {
        return;
    };
    if content.is_empty() {
        return;
    }

    ensure_claude_turn_started(state, out);
    if state.current_message_seq == 0 || message_id != state.current_message_id.as_deref() {
        begin_claude_message(state, message_id);
    }
    let key = root
        .get("uuid")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| serde_json::to_string(content).unwrap_or_default());
    if !state.snapshot_keys.insert(key) {
        return;
    }
    emit_snapshot_parts(state, content, out);
}

fn emit_snapshot_parts(
    state: &mut ClaudeTranslationState,
    content: &[Value],
    out: &mut TranslateOutput,
) {
    for (position, part) in content.iter().enumerate() {
        let kind = part.get("type").and_then(Value::as_str).unwrap_or("");
        let index = if content.len() > 1 {
            position
        } else {
            state
                .streamed_blocks
                .iter()
                .filter(|(index, streamed)| {
                    **index >= state.next_snapshot_block && streamed.as_str() == kind
                })
                .map(|(index, _)| *index)
                .min()
                .unwrap_or(state.next_snapshot_block)
        };
        state.next_snapshot_block = state.next_snapshot_block.max(index + 1);
        if state.streamed_blocks.contains_key(&index) {
            continue;
        }
        match kind {
            "text" => {
                emit_text_snapshot(state, part, index, out);
            }
            "thinking" => {
                if let Some(text) = part
                    .get("thinking")
                    .and_then(Value::as_str)
                    .filter(|text| !text.is_empty())
                {
                    super::thinking::thinking_completed(
                        claude_message_item_id(state, index),
                        text.to_string(),
                        out,
                    );
                }
            }
            "tool_use" => {
                emit_tool_use_snapshot(state, part, index, out);
            }
            _ => {}
        }
    }
}

fn emit_text_snapshot(
    state: &ClaudeTranslationState,
    part: &Value,
    index: usize,
    out: &mut TranslateOutput,
) -> bool {
    let Some(text) = part
        .get("text")
        .and_then(Value::as_str)
        .map(str::to_string)
        .filter(|t| !t.trim().is_empty())
    else {
        return false;
    };
    let item_id = claude_message_item_id(state, index);
    out.lines.push(
        json!({
            "method": "item/completed",
            "params": { "item": { "id": item_id, "type": "agentMessage", "text": text } }
        })
        .to_string(),
    );
    true
}

fn emit_tool_use_snapshot(
    state: &mut ClaudeTranslationState,
    part: &Value,
    index: usize,
    out: &mut TranslateOutput,
) -> bool {
    let item_id = part
        .get("id")
        .and_then(Value::as_str)
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| claude_tool_item_id(state, index));
    let name = part
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("Tool")
        .to_string();
    let input = part.get("input").cloned().unwrap_or_else(|| json!({}));
    let hidden = should_hide_claude_tool_transcript(&name, &input);
    state.tool_calls.insert(
        item_id.clone(),
        ClaudeToolCall {
            name: name.clone(),
            input: input.clone(),
        },
    );
    if hidden {
        return false;
    }
    out.lines.push(
        json!({
            "method": "item/completed",
            "params": {
                "item": {
                    "id": item_id, "type": "toolCall",
                    "tool": name, "name": name, "input": input,
                }
            }
        })
        .to_string(),
    );
    true
}
