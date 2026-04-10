//! Assistant snapshot output synthesis for the Claude CLI backend.

use serde_json::{json, Map, Value};

use super::types::{
    begin_claude_message, ensure_claude_turn_started, ClaudeToolCall, ClaudeTranslationState,
    TranslateOutput, should_hide_claude_tool_transcript,
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
    if state.current_message_has_content_blocks {
        return;
    }

    let Some(content) = message.get("content").and_then(Value::as_array) else {
        return;
    };
    if content.is_empty() {
        return;
    }

    let had_live_turn = state.current_turn_id.is_some();
    ensure_claude_turn_started(state, out);
    if !had_live_turn || state.current_message_seq == 0 {
        begin_claude_message(state);
    } else if state.current_message_has_content_blocks {
        return;
    }

    let mut emitted_any = false;
    for (index, part) in content.iter().enumerate() {
        match part.get("type").and_then(Value::as_str) {
            Some("text") => emitted_any |= emit_text_snapshot(state, part, index, out),
            Some("tool_use") => emitted_any |= emit_tool_use_snapshot(state, part, index, out),
            _ => {}
        }
    }

    if emitted_any {
        state.current_message_has_content_blocks = true;
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
    let item_id = format!("claude-msg-{}-{}", state.current_message_seq, index);
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
        .unwrap_or_else(|| format!("claude-tool-{}-{}", state.current_message_seq, index));
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
