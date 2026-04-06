//! Assistant snapshot output synthesis for the Claude CLI backend.

use serde_json::{json, Map, Value};

use super::types::{
    begin_claude_message, ensure_claude_turn_started, ClaudeToolCall, ClaudeTranslationState,
    TranslateOutput,
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
            Some("text") => {
                let Some(text) = part
                    .get("text")
                    .and_then(Value::as_str)
                    .map(str::to_string)
                    .filter(|text| !text.trim().is_empty())
                else {
                    continue;
                };
                let item_id = format!("claude-msg-{}-{}", state.current_message_seq, index);
                out.lines.push(
                    json!({
                        "method": "item/completed",
                        "params": {
                            "item": {
                                "id": item_id,
                                "type": "agentMessage",
                                "text": text,
                            }
                        }
                    })
                    .to_string(),
                );
                emitted_any = true;
            }
            Some("tool_use") => {
                let item_id = part
                    .get("id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned)
                    .unwrap_or_else(|| {
                        format!("claude-tool-{}-{}", state.current_message_seq, index)
                    });
                let name = part
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("Tool")
                    .to_string();
                let input = part.get("input").cloned().unwrap_or_else(|| json!({}));
                state.tool_calls.insert(
                    item_id.clone(),
                    ClaudeToolCall {
                        name: name.clone(),
                        input: input.clone(),
                    },
                );
                out.lines.push(
                    json!({
                        "method": "item/completed",
                        "params": {
                            "item": {
                                "id": item_id,
                                "type": "toolCall",
                                "tool": name,
                                "name": name,
                                "input": input,
                            }
                        }
                    })
                    .to_string(),
                );
                emitted_any = true;
            }
            _ => {}
        }
    }

    if emitted_any {
        state.current_message_has_content_blocks = true;
    }
}
